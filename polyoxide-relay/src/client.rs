use crate::account::BuilderAccount;
use crate::config::{get_contract_config, AuthConfig, BuilderConfig, ContractConfig};
use crate::deposit_wallet::DepositWalletCall;
use crate::error::RelayError;
use crate::session_signers::{
    SessionSignerAuthorization, SessionSignerAuthorizationResponse, SessionSignerRevocation,
    SessionSignerRevocationResponse,
};
use crate::types::{
    ExecuteParams, GaslessTransaction, NonceResponse, RelayerApiKey, RelayerTransaction,
    SafeTransaction, SafeTx, SubmitResponse, WalletType,
};
use crate::wallet::WalletKind;
use alloy::hex;
use alloy::network::TransactionBuilder;
use alloy::primitives::{address, keccak256, Address, Bytes, B256, U256};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::TransactionRequest;
use alloy::sol_types::{Eip712Domain, SolCall, SolStruct, SolValue};
use polyoxide_core::{
    retry_after_header, DepositWalletRole, HttpClient, HttpClientBuilder, RateLimiter, RetryConfig,
    SessionSignerScope,
};
use serde::Serialize;
use std::time::{Duration, Instant};
use url::Url;

// Safe/Proxy wallet operation types
const CALL_OPERATION: u8 = 0;
const DELEGATE_CALL_OPERATION: u8 = 1;

/// py-sdk's redemption target for a Deposit Wallet on a CTF market.
const COLLATERAL_ADAPTER: Address = address!("AdA100Db00Ca00073811820692005400218FcE1f");
/// py-sdk's redemption target for a Deposit Wallet on a neg-risk market.
const NEG_RISK_COLLATERAL_ADAPTER: Address = address!("adA2005600Dec949baf300f4C6120000bDB6eAab");

/// The error both legacy redemption entry points return on a Deposit Wallet client,
/// which has to name the market's neg-risk flag to pick the adapter.
fn deposit_wallet_redemption_refused() -> RelayError {
    RelayError::Api(
        "a Deposit Wallet redemption needs the market's neg-risk flag: use \
         RelayClient::submit_deposit_wallet_redemption"
            .to_string(),
    )
}

// Proxy wallet call type for ProxyTransaction struct
const PROXY_CALL_TYPE_CODE: u8 = 1;

// multiSend(bytes) function selector
const MULTISEND_SELECTOR: [u8; 4] = [0x8d, 0x80, 0xff, 0x0a];

/// `GET /deployed` response body, shared by [`RelayClient::get_deployed`] and
/// [`RelayClient::get_deployed_typed`].
#[derive(serde::Deserialize)]
struct DeployedResponse {
    deployed: bool,
}

// ── Relay submission request bodies ─────────────────────────────────

#[derive(Serialize)]
struct SafeSigParams {
    #[serde(rename = "gasPrice")]
    gas_price: String,
    operation: String,
    #[serde(rename = "safeTxnGas")]
    safe_tx_gas: String,
    #[serde(rename = "baseGas")]
    base_gas: String,
    #[serde(rename = "gasToken")]
    gas_token: String,
    #[serde(rename = "refundReceiver")]
    refund_receiver: String,
}

#[derive(Serialize)]
struct SafeSubmitBody {
    #[serde(rename = "type")]
    type_: String,
    from: String,
    to: String,
    #[serde(rename = "proxyWallet")]
    proxy_wallet: String,
    data: String,
    signature: String,
    #[serde(rename = "signatureParams")]
    signature_params: SafeSigParams,
    value: String,
    nonce: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<String>,
}

#[derive(Serialize)]
struct ProxySigParams {
    #[serde(rename = "relayerFee")]
    relayer_fee: String,
    #[serde(rename = "gasLimit")]
    gas_limit: String,
    #[serde(rename = "gasPrice")]
    gas_price: String,
    #[serde(rename = "relayHub")]
    relay_hub: String,
    relay: String,
}

#[derive(Serialize)]
struct ProxySubmitBody {
    #[serde(rename = "type")]
    type_: String,
    from: String,
    to: String,
    #[serde(rename = "proxyWallet")]
    proxy_wallet: String,
    data: String,
    signature: String,
    #[serde(rename = "signatureParams")]
    signature_params: ProxySigParams,
    nonce: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<String>,
}

#[derive(Serialize)]
struct DepositWalletCallBody {
    target: String,
    value: String,
    data: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DepositWalletParamsBody {
    deposit_wallet: String,
    deadline: String,
    calls: Vec<DepositWalletCallBody>,
}

#[derive(Serialize)]
struct DepositWalletSubmitBody {
    #[serde(rename = "type")]
    type_: String,
    from: String,
    to: String,
    nonce: String,
    signature: String,
    /// Always present, as py-sdk's `build_deposit_wallet_payload` sends it: `""` by
    /// default, at most [`DEPOSIT_WALLET_METADATA_MAX_CHARS`] characters.
    metadata: String,
    #[serde(rename = "depositWalletParams")]
    deposit_wallet_params: DepositWalletParamsBody,
}

/// py-sdk's `_METADATA_MAX_LENGTH`: the longest `metadata` a Deposit Wallet submission
/// may carry, in characters.
const DEPOSIT_WALLET_METADATA_MAX_CHARS: usize = 500;

/// The `metadata` a Deposit Wallet submission sends: `""` when none is given, refused
/// past [`DEPOSIT_WALLET_METADATA_MAX_CHARS`]. Counts characters (code points), as
/// Python's `len` does, not bytes.
fn deposit_wallet_metadata(metadata: Option<String>) -> Result<String, RelayError> {
    let metadata = metadata.unwrap_or_default();
    let chars = metadata.chars().count();
    if chars > DEPOSIT_WALLET_METADATA_MAX_CHARS {
        return Err(RelayError::Api(format!(
            "metadata must be at most {DEPOSIT_WALLET_METADATA_MAX_CHARS} characters, got {chars}"
        )));
    }
    Ok(metadata)
}

/// The session-signer authorization route, which accepts only Builder HMAC auth.
const SESSION_SIGNER_AUTHORIZATIONS: &str = "v1/session-signers/authorizations";

/// Refuse a relayer API key for a route that accepts only Builder HMAC. `path` is
/// the route with its leading slash, as it appears in the error. Shared by
/// `post_json` and the conveniences that must refuse before their own I/O, so the
/// wording cannot drift between them.
fn refuse_relayer_api_key(auth: &AuthConfig, path: &str) -> Result<(), RelayError> {
    if matches!(auth, AuthConfig::RelayerApiKey(_)) {
        return Err(RelayError::Api(format!(
            "{path} requires Builder HMAC auth; configure the client with BuilderConfig"
        )));
    }
    Ok(())
}

/// Whether a trading approval is an ERC-20 `approve` or an ERC-1155 `setApprovalForAll`.
#[derive(Debug, Clone, Copy)]
enum ApprovalKind {
    Erc20,
    Erc1155,
}

/// py-sdk's `_required_trading_approvals` on Polygon mainnet, in py-sdk's order: the
/// kind, the token contract called, and the spender or operator approved. ERC-20
/// approvals are for the maximum uint256; ERC-1155 approvals set `true`.
const POLYGON_TRADING_APPROVALS: [(ApprovalKind, Address, Address); 17] = {
    use ApprovalKind::{Erc1155, Erc20};
    let pusd = address!("C011a7E12a19f7B1f670d46F03B03f3342E82DFB");
    let ctf = address!("4D97DCd97eC945f40cF65F87097ACe5EA0476045");
    let position_manager = address!("006F54F7f9A22e0000CC2AB60031000000ae9fEF");
    let standard_exchange = address!("E111180000d2663C0091e4f400237545B87B996B");
    let neg_risk_exchange = address!("e2222d279d744050d28e00520010520000310F59");
    let collateral_adapter = COLLATERAL_ADAPTER;
    let neg_risk_collateral_adapter = NEG_RISK_COLLATERAL_ADAPTER;
    let protocol_v2_router = address!("12121212006e4CD160D18e3f00711DA5c3372600");
    let exchange_v3 = address!("e3333700cA9d93003F00f0F71f8515005F6c00Aa");
    let perps_deposit = address!("DCa4af75705dbB50f62437045afF9921947917d2");
    let auto_redeem_operator = address!("a1200000d0002264C9a1698e001292D00E1b00af");
    let binary_module = address!("1000008dD9001B968442c1000017eaE6E0dA00Ba");
    let neg_risk_module = address!("200000900045e3B6259600682756002200028933");
    [
        (Erc20, pusd, standard_exchange),
        (Erc20, pusd, neg_risk_exchange),
        (Erc20, pusd, collateral_adapter),
        (Erc20, pusd, neg_risk_collateral_adapter),
        (Erc20, pusd, protocol_v2_router),
        (Erc20, pusd, exchange_v3),
        (Erc20, pusd, perps_deposit),
        (Erc1155, ctf, standard_exchange),
        (Erc1155, ctf, neg_risk_exchange),
        (Erc1155, ctf, collateral_adapter),
        (Erc1155, ctf, neg_risk_collateral_adapter),
        (Erc1155, ctf, auto_redeem_operator),
        (Erc1155, ctf, binary_module),
        (Erc1155, ctf, neg_risk_module),
        (Erc1155, position_manager, protocol_v2_router),
        (Erc1155, position_manager, exchange_v3),
        (Erc1155, position_manager, auto_redeem_operator),
    ]
};

/// Client for submitting gasless transactions through Polymarket's relayer service.
///
/// Supports Safe, Proxy and Deposit Wallet types. Handles EIP-712 transaction signing,
/// nonce management, and multi-send batching automatically.
#[derive(Debug, Clone)]
pub struct RelayClient {
    http_client: HttpClient,
    chain_id: u64,
    account: Option<BuilderAccount>,
    auth: Option<AuthConfig>,
    contract_config: ContractConfig,
    wallet_type: WalletType,
    deposit_wallet: Option<Address>,
    deposit_wallet_role: DepositWalletRole,
}

impl RelayClient {
    /// Create a new Relay client with authentication
    pub fn new(
        private_key: impl Into<String>,
        config: Option<BuilderConfig>,
    ) -> Result<Self, RelayError> {
        let account = BuilderAccount::new(private_key, config)?;
        Self::builder()?.with_account(account).build()
    }

    /// Create a new Relay client builder
    pub fn builder() -> Result<RelayClientBuilder, RelayError> {
        RelayClientBuilder::new()
    }

    /// Create a new Relay client builder pulling settings from environment
    pub fn default_builder() -> Result<RelayClientBuilder, RelayError> {
        Ok(RelayClientBuilder::default())
    }

    /// Create a new Relay client from a BuilderAccount
    pub fn from_account(account: BuilderAccount) -> Result<Self, RelayError> {
        Self::builder()?.with_account(account).build()
    }

    /// Returns the signer's Ethereum address, or `None` if no account is configured.
    pub fn address(&self) -> Option<Address> {
        self.account.as_ref().map(|a| a.address())
    }

    /// Send a GET request with retry-on-429 logic.
    ///
    /// Handles rate limiting, retries with exponential backoff, and error
    /// responses. Returns the successful response for the caller to parse.
    async fn get_with_retry(&self, path: &str, url: &Url) -> Result<reqwest::Response, RelayError> {
        let mut attempt = 0u32;
        loop {
            let _permit = self.http_client.acquire_concurrency().await;
            self.http_client.acquire_rate_limit(path, None).await;
            let resp = self.http_client.client.get(url.clone()).send().await?;
            let retry_after = retry_after_header(&resp);
            self.http_client
                .note_rate_limited(resp.status(), retry_after.as_deref());

            if let Some(backoff) =
                self.http_client
                    .should_retry(resp.status(), attempt, retry_after.as_deref())
            {
                attempt += 1;
                tracing::warn!(
                    "Retriable status {} on {}, retry {} after {}ms",
                    resp.status(),
                    path,
                    attempt,
                    backoff.as_millis()
                );
                drop(_permit);
                tokio::time::sleep(backoff).await;
                continue;
            }

            if !resp.status().is_success() {
                let text = resp.text().await?;
                return Err(RelayError::Api(format!("{} failed: {}", path, text)));
            }

            return Ok(resp);
        }
    }

    /// The auth this client submits under: the account's own config if it has one,
    /// otherwise the one given to [`RelayClientBuilder::with_auth`].
    fn auth(&self) -> Result<&AuthConfig, RelayError> {
        if let Some(auth) = &self.auth {
            return Ok(auth);
        }
        if self.account.is_none() {
            return Err(RelayError::Api(
                "Account missing - cannot authenticate request. Configure an account via RelayClientBuilder::with_account or ::relayer_api_key, or auth via ::with_auth.".to_string(),
            ));
        }
        Err(RelayError::Api(
            "No authentication configured - provide BuilderConfig or RelayerApiKeyConfig when creating the BuilderAccount, or configure auth via RelayClientBuilder::with_auth".to_string(),
        ))
    }

    /// Produce GET auth headers, enforcing per-endpoint auth-scheme allow-lists.
    fn authed_get_headers(
        &self,
        path: &str,
        allow_builder: bool,
        allow_relayer_api_key: bool,
    ) -> Result<reqwest::header::HeaderMap, RelayError> {
        match self.auth()? {
            AuthConfig::Builder(cfg) => {
                if !allow_builder {
                    return Err(RelayError::Api(format!(
                        "{} requires Relayer API Key auth; configure the client with relayer_api_key()",
                        path
                    )));
                }
                cfg.generate_relayer_v2_headers("GET", path, None)
                    .map_err(RelayError::Api)
            }
            AuthConfig::RelayerApiKey(cfg) => {
                if !allow_relayer_api_key {
                    return Err(RelayError::Api(format!(
                        "{} requires Builder HMAC auth; configure the client with BuilderConfig",
                        path
                    )));
                }
                cfg.generate_headers().map_err(RelayError::Api)
            }
        }
    }

    /// Send an authenticated GET request with retry-on-429 logic.
    async fn get_with_retry_authed(
        &self,
        path: &str,
        url: &Url,
        allow_builder: bool,
        allow_relayer_api_key: bool,
    ) -> Result<reqwest::Response, RelayError> {
        let mut attempt = 0u32;
        loop {
            let _permit = self.http_client.acquire_concurrency().await;
            self.http_client.acquire_rate_limit(path, None).await;

            // Regenerate auth headers each attempt so HMAC timestamps stay fresh.
            let headers = self.authed_get_headers(path, allow_builder, allow_relayer_api_key)?;
            let resp = self
                .http_client
                .client
                .get(url.clone())
                .headers(headers)
                .send()
                .await?;

            let retry_after = retry_after_header(&resp);
            self.http_client
                .note_rate_limited(resp.status(), retry_after.as_deref());
            if let Some(backoff) =
                self.http_client
                    .should_retry(resp.status(), attempt, retry_after.as_deref())
            {
                attempt += 1;
                tracing::warn!(
                    "Retriable status {} on {}, retry {} after {}ms",
                    resp.status(),
                    path,
                    attempt,
                    backoff.as_millis()
                );
                drop(_permit);
                tokio::time::sleep(backoff).await;
                continue;
            }

            if !resp.status().is_success() {
                let text = resp.text().await?;
                return Err(RelayError::Api(format!("{} failed: {}", path, text)));
            }

            return Ok(resp);
        }
    }

    /// Measure the round-trip time (RTT) to the Relay API.
    ///
    /// Makes a GET request to the API base URL and returns the latency.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use polyoxide_relay::RelayClient;
    ///
    /// # async fn example() -> Result<(), polyoxide_relay::RelayError> {
    /// let client = RelayClient::builder()?.build()?;
    /// let latency = client.ping().await?;
    /// println!("API latency: {}ms", latency.as_millis());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn ping(&self) -> Result<Duration, RelayError> {
        let url = self.http_client.base_url.clone();
        let start = Instant::now();
        let _resp = self.get_with_retry("/", &url).await?;
        Ok(start.elapsed())
    }

    /// Fetch the current transaction nonce for an address from the relayer.
    pub async fn get_nonce(&self, address: Address) -> Result<u64, RelayError> {
        let url = self.http_client.base_url.join(&format!(
            "nonce?address={}&type={}",
            address,
            self.wallet_type.as_str()
        ))?;
        let resp = self.get_with_retry("/nonce", &url).await?;
        let data = resp.json::<NonceResponse>().await?;
        Ok(data.nonce)
    }

    /// Query the full record of a previously submitted relay transaction.
    pub async fn get_transaction(
        &self,
        transaction_id: &str,
    ) -> Result<RelayerTransaction, RelayError> {
        let url = self
            .http_client
            .base_url
            .join(&format!("transaction?id={}", transaction_id))?;
        let resp = self.get_with_retry("/transaction", &url).await?;
        resp.json::<RelayerTransaction>().await.map_err(Into::into)
    }

    /// List the most recent relayer transactions owned by the authenticated user.
    ///
    /// Accepts either Builder HMAC auth ([`AuthConfig::Builder`]) or static Relayer
    /// API Key auth ([`AuthConfig::RelayerApiKey`]); the client will use whichever
    /// is configured on its [`BuilderAccount`].
    ///
    /// Returns an error if no account / auth is configured.
    ///
    /// See `GET /transactions` in `docs/specs/relay/openapi.yaml`.
    pub async fn list_transactions(&self) -> Result<Vec<RelayerTransaction>, RelayError> {
        let url = self.http_client.base_url.join("transactions")?;
        let resp = self
            .get_with_retry_authed("/transactions", &url, true, true)
            .await?;
        resp.json::<Vec<RelayerTransaction>>()
            .await
            .map_err(Into::into)
    }

    /// List all relayer API keys owned by the authenticated address.
    ///
    /// Requires static Relayer API Key auth ([`AuthConfig::RelayerApiKey`]);
    /// returns an error if the client is configured with Builder HMAC auth
    /// (per the OpenAPI spec, this endpoint does not accept Builder HMAC).
    ///
    /// See `GET /relayer/api/keys` in `docs/specs/relay/openapi.yaml`.
    pub async fn list_relayer_api_keys(&self) -> Result<Vec<RelayerApiKey>, RelayError> {
        let url = self.http_client.base_url.join("relayer/api/keys")?;
        let resp = self
            .get_with_retry_authed("/relayer/api/keys", &url, false, true)
            .await?;
        resp.json::<Vec<RelayerApiKey>>().await.map_err(Into::into)
    }

    /// Check whether a Safe wallet has been deployed on-chain.
    pub async fn get_deployed(&self, safe_address: Address) -> Result<bool, RelayError> {
        let url = self
            .http_client
            .base_url
            .join(&format!("deployed?address={}", safe_address))?;
        let resp = self.get_with_retry("/deployed", &url).await?;
        let data = resp.json::<DeployedResponse>().await?;
        Ok(data.deployed)
    }

    /// Check whether a wallet of the given type is deployed (`GET /deployed?type=`).
    ///
    /// The published spec lists [`WalletType::Safe`] and [`WalletType::DepositWallet`];
    /// py-sdk also sends [`WalletType::Proxy`], and so does [`Self::resolve_wallet`].
    pub async fn get_deployed_typed(
        &self,
        wallet: Address,
        wallet_type: WalletType,
    ) -> Result<bool, RelayError> {
        let url = self.http_client.base_url.join(&format!(
            "deployed?address={}&type={}",
            wallet,
            wallet_type.as_str()
        ))?;
        let resp = self.get_with_retry("/deployed", &url).await?;
        Ok(resp.json::<DeployedResponse>().await?.deployed)
    }

    /// Fetch the next nonce for `owner`'s wallet of `wallet_type`
    /// (`GET /v1/account/transactions/params`).
    ///
    /// This is the v1 route the Deposit Wallet batch needs; [`RelayClient::get_nonce`]
    /// stays on the legacy `/nonce` route for Safe and Proxy.
    pub async fn get_execute_params(
        &self,
        owner: Address,
        wallet_type: WalletType,
    ) -> Result<u64, RelayError> {
        let url = self.http_client.base_url.join(&format!(
            "v1/account/transactions/params?address={}&type={}",
            owner,
            wallet_type.as_str()
        ))?;
        let resp = self
            .get_with_retry("/v1/account/transactions/params", &url)
            .await?;
        Ok(resp.json::<ExecuteParams>().await?.nonce)
    }

    /// Poll a submitted transaction (`GET /v1/account/transactions/{id}`).
    ///
    /// Stop polling once [`crate::types::TransactionState::is_terminal`] is true. The
    /// venue may take up to five minutes to move a session-signer authorization out of
    /// `STATE_NEW`.
    pub async fn get_gasless_transaction(
        &self,
        transaction_id: &str,
    ) -> Result<GaslessTransaction, RelayError> {
        let mut url = self.http_client.base_url.join("v1/account/transactions/")?;
        url.path_segments_mut()
            .map_err(|_| RelayError::Api("base URL cannot be a base".into()))?
            .pop_if_empty()
            .push(transaction_id);
        let resp = self
            .get_with_retry("/v1/account/transactions", &url)
            .await?;
        resp.json::<GaslessTransaction>().await.map_err(Into::into)
    }

    /// Find which account wallet `owner` has deployed.
    ///
    /// Derives the beacon and UUPS Deposit Wallets, the Safe and the Proxy, asks
    /// `/deployed` for each, and returns the one that exists, or `None` when nothing
    /// is deployed. More than one deployed wallet is an error rather than a guess.
    /// The Proxy is asked with `type=PROXY`, as py-sdk does, although the published
    /// spec omits it; see [`WalletKind`]. Costs up to four requests against the
    /// relay bucket, so callers should cache the answer rather than calling this on
    /// every use. A candidate whose contracts the chain's `ContractConfig` does not
    /// name is skipped rather than refused, so on Amoy only the Safe is asked.
    pub async fn resolve_wallet(&self, owner: Address) -> Result<Option<WalletKind>, RelayError> {
        let cfg = &self.contract_config;
        let mut candidates: Vec<(WalletKind, WalletType)> = Vec::new();
        if cfg.deposit_wallet_factory.is_some() && cfg.deposit_wallet_beacon.is_some() {
            candidates.push((
                WalletKind::DepositWallet(crate::wallet::derive_deposit_wallet_beacon(owner, cfg)?),
                WalletType::DepositWallet,
            ));
        }
        if cfg.deposit_wallet_factory.is_some() && cfg.deposit_wallet_implementation.is_some() {
            candidates.push((
                WalletKind::DepositWallet(crate::wallet::derive_deposit_wallet_uups(owner, cfg)?),
                WalletType::DepositWallet,
            ));
        }
        candidates.push((
            WalletKind::Safe(crate::wallet::derive_safe(owner, cfg)),
            WalletType::Safe,
        ));
        if cfg.proxy_factory.is_some() && cfg.proxy_implementation.is_some() {
            candidates.push((
                WalletKind::Proxy(crate::wallet::derive_proxy(owner, cfg)?),
                WalletType::Proxy,
            ));
        }

        let mut found = Vec::new();
        for (kind, wallet_type) in candidates {
            if self.get_deployed_typed(kind.address(), wallet_type).await? {
                found.push(kind);
            }
        }
        match found.as_slice() {
            [] => Ok(None),
            [one] => Ok(Some(*one)),
            many => Err(RelayError::Api(format!(
                "owner {owner} has more than one deployed wallet: {many:?}"
            ))),
        }
    }

    fn derive_safe_address(&self, owner: Address) -> Address {
        crate::wallet::derive_safe(owner, &self.contract_config)
    }

    /// Derive the expected Safe wallet address for the configured account via CREATE2.
    pub fn get_expected_safe(&self) -> Result<Address, RelayError> {
        let account = self.account.as_ref().ok_or(RelayError::MissingSigner)?;
        Ok(self.derive_safe_address(account.address()))
    }

    fn derive_proxy_wallet(&self, owner: Address) -> Result<Address, RelayError> {
        crate::wallet::derive_proxy(owner, &self.contract_config)
    }

    /// Derive the expected Proxy wallet address for the configured account via CREATE2.
    pub fn get_expected_proxy_wallet(&self) -> Result<Address, RelayError> {
        let account = self.account.as_ref().ok_or(RelayError::MissingSigner)?;
        self.derive_proxy_wallet(account.address())
    }

    /// Get relay payload for PROXY wallets (returns relay address and nonce)
    pub async fn get_relay_payload(&self, address: Address) -> Result<(Address, u64), RelayError> {
        #[derive(serde::Deserialize)]
        struct RelayPayload {
            address: String,
            #[serde(deserialize_with = "crate::types::deserialize_nonce")]
            nonce: u64,
        }

        let url = self
            .http_client
            .base_url
            .join(&format!("relay-payload?address={}&type=PROXY", address))?;
        let resp = self.get_with_retry("/relay-payload", &url).await?;
        let data = resp.json::<RelayPayload>().await?;
        let relay_address: Address = data
            .address
            .parse()
            .map_err(|e| RelayError::Api(format!("Invalid relay address: {}", e)))?;
        Ok((relay_address, data.nonce))
    }

    /// Create the proxy struct hash for signing (EIP-712 style but with specific fields)
    #[allow(clippy::too_many_arguments)]
    fn create_proxy_struct_hash(
        &self,
        from: Address,
        to: Address,
        data: &[u8],
        tx_fee: U256,
        gas_price: U256,
        gas_limit: U256,
        nonce: u64,
        relay_hub: Address,
        relay: Address,
    ) -> [u8; 32] {
        let mut message = Vec::new();

        // "rlx:" prefix
        message.extend_from_slice(b"rlx:");
        // from address (20 bytes)
        message.extend_from_slice(from.as_slice());
        // to address (20 bytes) - This must be the ProxyFactory address
        message.extend_from_slice(to.as_slice());
        // data (raw bytes)
        message.extend_from_slice(data);
        // txFee as 32-byte big-endian
        message.extend_from_slice(&tx_fee.to_be_bytes::<32>());
        // gasPrice as 32-byte big-endian
        message.extend_from_slice(&gas_price.to_be_bytes::<32>());
        // gasLimit as 32-byte big-endian
        message.extend_from_slice(&gas_limit.to_be_bytes::<32>());
        // nonce as 32-byte big-endian
        message.extend_from_slice(&U256::from(nonce).to_be_bytes::<32>());
        // relayHub address (20 bytes)
        message.extend_from_slice(relay_hub.as_slice());
        // relay address (20 bytes)
        message.extend_from_slice(relay.as_slice());

        keccak256(&message).into()
    }

    /// Encode proxy transactions into calldata for the proxy wallet
    fn encode_proxy_transaction_data(&self, txns: &[SafeTransaction]) -> Vec<u8> {
        // ProxyTransaction struct: (uint8 typeCode, address to, uint256 value, bytes data)
        // Function selector for proxy(ProxyTransaction[])
        // IMPORTANT: Field order must match the ABI exactly!
        alloy::sol! {
            struct ProxyTransaction {
                uint8 typeCode;
                address to;
                uint256 value;
                bytes data;
            }
            function proxy(ProxyTransaction[] txns);
        }

        let proxy_txns: Vec<ProxyTransaction> = txns
            .iter()
            .map(|tx| ProxyTransaction {
                typeCode: PROXY_CALL_TYPE_CODE,
                to: tx.to,
                value: tx.value,
                data: tx.data.clone(),
            })
            .collect();

        // Encode the function call: proxy([ProxyTransaction, ...])
        let call = proxyCall { txns: proxy_txns };
        call.abi_encode()
    }

    fn create_safe_multisend_transaction(&self, txns: &[SafeTransaction]) -> SafeTransaction {
        if txns.len() == 1 {
            return txns[0].clone();
        }

        let mut encoded_txns = Vec::new();
        for tx in txns {
            // Packed: [uint8 operation, address to, uint256 value, uint256 data_len, bytes data]
            let mut packed = Vec::new();
            packed.push(tx.operation);
            packed.extend_from_slice(tx.to.as_slice());
            packed.extend_from_slice(&tx.value.to_be_bytes::<32>());
            packed.extend_from_slice(&U256::from(tx.data.len()).to_be_bytes::<32>());
            packed.extend_from_slice(&tx.data);
            encoded_txns.extend_from_slice(&packed);
        }

        let mut data = MULTISEND_SELECTOR.to_vec();

        // Use alloy to encode `(bytes)` tuple.
        let multisend_data = (Bytes::from(encoded_txns),).abi_encode();
        data.extend_from_slice(&multisend_data);

        SafeTransaction {
            to: self.contract_config.safe_multisend,
            operation: DELEGATE_CALL_OPERATION,
            data: data.into(),
            value: U256::ZERO,
        }
    }

    fn split_and_pack_sig_safe(&self, sig: alloy::primitives::Signature) -> String {
        // Alloy's v() returns a boolean y_parity: false = 0, true = 1
        // For Safe signatures, v must be adjusted: 0/1 + 31 = 31/32
        let v_raw = if sig.v() { 1u8 } else { 0u8 };
        let v = v_raw + 31;

        // Pack r, s, v
        let mut packed = Vec::new();
        packed.extend_from_slice(&sig.r().to_be_bytes::<32>());
        packed.extend_from_slice(&sig.s().to_be_bytes::<32>());
        packed.push(v);

        format!("0x{}", hex::encode(packed))
    }

    fn split_and_pack_sig_proxy(&self, sig: alloy::primitives::Signature) -> String {
        // For Proxy signatures, use standard v value: 27 or 28
        let v = if sig.v() { 28u8 } else { 27u8 };

        // Pack r, s, v
        let mut packed = Vec::new();
        packed.extend_from_slice(&sig.r().to_be_bytes::<32>());
        packed.extend_from_slice(&sig.s().to_be_bytes::<32>());
        packed.push(v);

        format!("0x{}", hex::encode(packed))
    }

    /// Sign and submit transactions through the relayer with default gas settings.
    pub async fn execute(
        &self,
        transactions: Vec<SafeTransaction>,
        metadata: Option<String>,
    ) -> Result<SubmitResponse, RelayError> {
        self.execute_with_gas(transactions, metadata, None).await
    }

    /// Sign and submit transactions through the relayer with an optional gas limit override.
    ///
    /// For Safe wallets, transactions are batched via MultiSend. For Proxy wallets,
    /// they are encoded into the proxy's calldata format. For Deposit Wallets, they
    /// are signed as one EIP-712 `Batch` of CALLs (DELEGATECALL is refused), and
    /// `gas_limit` is ignored because a Deposit Wallet submission carries none.
    pub async fn execute_with_gas(
        &self,
        transactions: Vec<SafeTransaction>,
        metadata: Option<String>,
        gas_limit: Option<u64>,
    ) -> Result<SubmitResponse, RelayError> {
        if transactions.is_empty() {
            return Err(RelayError::Api("No transactions to execute".into()));
        }
        match self.wallet_type {
            WalletType::Safe => self.execute_safe(transactions, metadata).await,
            WalletType::Proxy => self.execute_proxy(transactions, metadata, gas_limit).await,
            WalletType::DepositWallet => self.execute_deposit_wallet(transactions, metadata).await,
        }
    }

    async fn execute_safe(
        &self,
        transactions: Vec<SafeTransaction>,
        metadata: Option<String>,
    ) -> Result<SubmitResponse, RelayError> {
        let account = self.account.as_ref().ok_or(RelayError::MissingSigner)?;
        let from_address = account.address();

        let safe_address = self.derive_safe_address(from_address);

        if !self.get_deployed(safe_address).await? {
            return Err(RelayError::Api(format!(
                "Safe {} is not deployed",
                safe_address
            )));
        }

        let nonce = self.get_nonce(from_address).await?;

        let aggregated = self.create_safe_multisend_transaction(&transactions);

        let safe_tx = SafeTx {
            to: aggregated.to,
            value: aggregated.value,
            data: aggregated.data,
            operation: aggregated.operation,
            safeTxGas: U256::ZERO,
            baseGas: U256::ZERO,
            gasPrice: U256::ZERO,
            gasToken: Address::ZERO,
            refundReceiver: Address::ZERO,
            nonce: U256::from(nonce),
        };

        let domain = Eip712Domain {
            name: None,
            version: None,
            chain_id: Some(U256::from(self.chain_id)),
            verifying_contract: Some(safe_address),
            salt: None,
        };

        let struct_hash = safe_tx.eip712_signing_hash(&domain);
        let signature = account
            .signer()
            .sign_message(struct_hash.as_slice())
            .await
            .map_err(|e| RelayError::Signer(e.to_string()))?;
        let packed_sig = self.split_and_pack_sig_safe(signature);

        let body = SafeSubmitBody {
            type_: "SAFE".to_string(),
            from: from_address.to_string(),
            to: safe_tx.to.to_string(),
            proxy_wallet: safe_address.to_string(),
            data: safe_tx.data.to_string(),
            signature: packed_sig,
            signature_params: SafeSigParams {
                gas_price: "0".to_string(),
                operation: safe_tx.operation.to_string(),
                safe_tx_gas: "0".to_string(),
                base_gas: "0".to_string(),
                gas_token: Address::ZERO.to_string(),
                refund_receiver: Address::ZERO.to_string(),
            },
            value: safe_tx.value.to_string(),
            nonce: nonce.to_string(),
            metadata,
        };

        self._post_request("submit", &body).await
    }

    async fn execute_proxy(
        &self,
        transactions: Vec<SafeTransaction>,
        metadata: Option<String>,
        gas_limit: Option<u64>,
    ) -> Result<SubmitResponse, RelayError> {
        let account = self.account.as_ref().ok_or(RelayError::MissingSigner)?;
        let from_address = account.address();

        let proxy_wallet = self.derive_proxy_wallet(from_address)?;
        let relay_hub = self
            .contract_config
            .relay_hub
            .ok_or_else(|| RelayError::Api("Relay hub not configured".to_string()))?;
        let proxy_factory = self
            .contract_config
            .proxy_factory
            .ok_or_else(|| RelayError::Api("Proxy factory not configured".to_string()))?;

        // Get relay payload (relay address + nonce)
        let (relay_address, nonce) = self.get_relay_payload(from_address).await?;

        // Encode all transactions into proxy calldata
        let encoded_data = self.encode_proxy_transaction_data(&transactions);

        // Constants for proxy transactions
        let tx_fee = U256::ZERO;
        let gas_price = U256::ZERO;
        let gas_limit = U256::from(gas_limit.unwrap_or(10_000_000u64));

        // The "to" field must be proxy_factory per the Python relayer client reference.
        let struct_hash = self.create_proxy_struct_hash(
            from_address,
            proxy_factory,
            &encoded_data,
            tx_fee,
            gas_price,
            gas_limit,
            nonce,
            relay_hub,
            relay_address,
        );

        // Sign the struct hash with EIP191 prefix
        let signature = account
            .signer()
            .sign_message(&struct_hash)
            .await
            .map_err(|e| RelayError::Signer(e.to_string()))?;
        let packed_sig = self.split_and_pack_sig_proxy(signature);

        let body = ProxySubmitBody {
            type_: "PROXY".to_string(),
            from: from_address.to_string(),
            to: proxy_factory.to_string(),
            proxy_wallet: proxy_wallet.to_string(),
            data: format!("0x{}", hex::encode(&encoded_data)),
            signature: packed_sig,
            signature_params: ProxySigParams {
                relayer_fee: "0".to_string(),
                gas_limit: gas_limit.to_string(),
                gas_price: "0".to_string(),
                relay_hub: relay_hub.to_string(),
                relay: relay_address.to_string(),
            },
            nonce: nonce.to_string(),
            metadata,
        };

        self._post_request("submit", &body).await
    }

    fn deposit_wallet_factory(&self) -> Result<Address, RelayError> {
        self.contract_config.deposit_wallet_factory.ok_or_else(|| {
            RelayError::Api("Deposit Wallets are not supported on this chain".to_string())
        })
    }

    fn configured_deposit_wallet(&self) -> Result<Address, RelayError> {
        self.deposit_wallet.ok_or_else(|| {
            RelayError::Api(
                "no Deposit Wallet configured: call RelayClientBuilder::deposit_wallet(address)"
                    .to_string(),
            )
        })
    }

    /// Unix seconds now plus [`crate::deposit_wallet::DEFAULT_BATCH_DEADLINE_SECS`].
    fn default_deadline() -> u64 {
        polyoxide_core::current_timestamp() + crate::deposit_wallet::DEFAULT_BATCH_DEADLINE_SECS
    }

    /// The batch as EIP-712 JSON for an external signer (`eth_signTypedData_v4`).
    ///
    /// Pair with [`RelayClient::submit_deposit_wallet_batch_from`]. `nonce` comes from
    /// [`RelayClient::get_execute_params`] with [`WalletType::DepositWallet`], queried
    /// for the EOA that will sign (the owner or the session key).
    pub fn deposit_wallet_batch_typed_data(
        &self,
        wallet: Address,
        calls: &[DepositWalletCall],
        nonce: u64,
        deadline: u64,
    ) -> serde_json::Value {
        crate::deposit_wallet::batch_typed_data(self.chain_id, wallet, calls, nonce, deadline)
    }

    /// Submit a batch signed elsewhere, naming the signer explicitly.
    ///
    /// `from` is the EOA that produced `signature` (the owner, or a session key whose
    /// signature is already wrapped in the session-signer envelope). Works with
    /// [`RelayClientBuilder::with_auth`] and no account.
    ///
    /// `metadata` is sent as `""` when `None`, as py-sdk does, and refused before any
    /// I/O past 500 characters (py-sdk's cap).
    #[allow(clippy::too_many_arguments)]
    pub async fn submit_deposit_wallet_batch_from(
        &self,
        from: Address,
        wallet: Address,
        calls: &[DepositWalletCall],
        nonce: u64,
        deadline: u64,
        signature: &str,
        metadata: Option<String>,
    ) -> Result<SubmitResponse, RelayError> {
        let metadata = deposit_wallet_metadata(metadata)?;
        let body = DepositWalletSubmitBody {
            type_: WalletType::DepositWallet.as_str().to_string(),
            from: from.to_string(),
            to: self.deposit_wallet_factory()?.to_string(),
            nonce: nonce.to_string(),
            signature: signature.to_string(),
            metadata,
            deposit_wallet_params: DepositWalletParamsBody {
                deposit_wallet: wallet.to_string(),
                deadline: deadline.to_string(),
                calls: calls
                    .iter()
                    .map(|c| DepositWalletCallBody {
                        target: c.target.to_string(),
                        value: c.value.to_string(),
                        data: format!("0x{}", hex::encode(&c.data)),
                    })
                    .collect(),
            },
        };
        self._post_request("submit", &body).await
    }

    /// Submit a batch signed elsewhere by this client's account.
    pub async fn submit_deposit_wallet_batch(
        &self,
        wallet: Address,
        calls: &[DepositWalletCall],
        nonce: u64,
        deadline: u64,
        signature: &str,
        metadata: Option<String>,
    ) -> Result<SubmitResponse, RelayError> {
        let from = self
            .account
            .as_ref()
            .ok_or(RelayError::MissingSigner)?
            .address();
        self.submit_deposit_wallet_batch_from(
            from, wallet, calls, nonce, deadline, signature, metadata,
        )
        .await
    }

    /// Sign a batch with the account's key, applying the session-signer envelope for a
    /// session-key role, and return the hex signature.
    async fn sign_deposit_wallet_batch(
        &self,
        wallet: Address,
        calls: &[DepositWalletCall],
        nonce: u64,
        deadline: u64,
    ) -> Result<String, RelayError> {
        let account = self.account.as_ref().ok_or(RelayError::MissingSigner)?;
        let digest =
            crate::deposit_wallet::batch_digest(self.chain_id, wallet, calls, nonce, deadline);
        let sig = account
            .signer()
            .sign_hash(&digest)
            .await
            .map_err(|e| RelayError::Signer(e.to_string()))?;
        let bytes = match self.deposit_wallet_role {
            DepositWalletRole::Owner => sig.as_bytes().to_vec(),
            DepositWalletRole::SessionKey => {
                crate::deposit_wallet::wrap_session_signer(account.address(), &sig.as_bytes())
            }
        };
        Ok(format!("0x{}", hex::encode(bytes)))
    }

    async fn execute_deposit_wallet(
        &self,
        transactions: Vec<SafeTransaction>,
        metadata: Option<String>,
    ) -> Result<SubmitResponse, RelayError> {
        let wallet = self.configured_deposit_wallet()?;
        // Fail on an unsupported chain or oversized metadata before fetching a nonce or
        // signing.
        self.deposit_wallet_factory()?;
        let metadata = Some(deposit_wallet_metadata(metadata)?);
        let account = self.account.as_ref().ok_or(RelayError::MissingSigner)?;
        let calls = transactions
            .into_iter()
            .map(|tx| {
                if tx.operation != CALL_OPERATION {
                    return Err(RelayError::Api(
                        "a Deposit Wallet batch supports CALL only, not DELEGATECALL".to_string(),
                    ));
                }
                Ok(DepositWalletCall {
                    target: tx.to,
                    value: tx.value,
                    data: tx.data,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let nonce = self
            .get_execute_params(account.address(), WalletType::DepositWallet)
            .await?;
        let deadline = Self::default_deadline();
        let signature = self
            .sign_deposit_wallet_batch(wallet, &calls, nonce, deadline)
            .await?;
        self.submit_deposit_wallet_batch(wallet, &calls, nonce, deadline, &signature, metadata)
            .await
    }

    /// The one-call batch that authorizes `session_signer` until `valid_until`.
    fn authorize_session_signer_calls(
        wallet: Address,
        session_signer: Address,
        valid_until: u64,
    ) -> Vec<DepositWalletCall> {
        vec![DepositWalletCall {
            target: wallet,
            value: U256::ZERO,
            data: crate::deposit_wallet::authorize_session_signer_calldata(
                session_signer,
                valid_until,
            )
            .into(),
        }]
    }

    /// The one-call batch that revokes `session_signer`.
    fn revoke_session_signer_calls(
        wallet: Address,
        session_signer: Address,
    ) -> Vec<DepositWalletCall> {
        vec![DepositWalletCall {
            target: wallet,
            value: U256::ZERO,
            data: crate::deposit_wallet::revoke_session_signer_calldata(session_signer).into(),
        }]
    }

    /// Build the authorization batch for an external signer, with `valid_until`
    /// computed as now + [`crate::SESSION_KEY_LIFETIME_SECS`] (the only lifetime the
    /// venue accepts). Returns the typed data to sign and the request to submit with
    /// the signature. Validates `scopes` before doing anything else.
    pub fn authorize_session_signer_typed_data(
        &self,
        wallet: Address,
        session_signer: Address,
        scopes: Vec<SessionSignerScope>,
        nonce: u64,
        deadline: u64,
    ) -> Result<(serde_json::Value, SessionSignerAuthorization), RelayError> {
        let valid_until =
            polyoxide_core::current_timestamp() + crate::deposit_wallet::SESSION_KEY_LIFETIME_SECS;
        self.authorize_session_signer_typed_data_with_valid_until(
            wallet,
            session_signer,
            scopes,
            valid_until,
            nonce,
            deadline,
        )
    }

    /// [`RelayClient::authorize_session_signer_typed_data`] with an explicit
    /// `valid_until`. The venue rejects lifetimes other than
    /// [`crate::SESSION_KEY_LIFETIME_SECS`] from now; this exists for tests and for
    /// the day the venue relaxes that.
    pub fn authorize_session_signer_typed_data_with_valid_until(
        &self,
        wallet: Address,
        session_signer: Address,
        scopes: Vec<SessionSignerScope>,
        valid_until: u64,
        nonce: u64,
        deadline: u64,
    ) -> Result<(serde_json::Value, SessionSignerAuthorization), RelayError> {
        crate::session_signers::validate_scopes(&scopes)?;
        if session_signer == Address::ZERO {
            return Err(RelayError::Api(
                "session signer must not be the zero address".into(),
            ));
        }
        let calls = Self::authorize_session_signer_calls(wallet, session_signer, valid_until);
        let typed = self.deposit_wallet_batch_typed_data(wallet, &calls, nonce, deadline);
        Ok((
            typed,
            SessionSignerAuthorization {
                wallet_address: wallet,
                session_signer_address: session_signer,
                scopes,
                valid_until,
                nonce,
                deadline,
            },
        ))
    }

    /// `POST /v1/session-signers/authorizations` with an owner signature produced
    /// elsewhere.
    ///
    /// Builder HMAC auth only: a client with a relayer API key is refused before any
    /// I/O. `idempotency_key` is trimmed and sent as `Idempotency-Key`; reuse it when
    /// retrying the same request. The request waits up to
    /// [`crate::SESSION_SIGNER_REQUEST_TIMEOUT`], because the venue broadcasts the
    /// batch before it answers.
    pub async fn submit_session_signer_authorization(
        &self,
        request: &SessionSignerAuthorization,
        signature: &str,
        idempotency_key: &str,
    ) -> Result<SessionSignerAuthorizationResponse, RelayError> {
        self.post_json(
            SESSION_SIGNER_AUTHORIZATIONS,
            &request.body(signature),
            Self::idempotency_headers(idempotency_key)?,
            false,
            Some(crate::session_signers::SESSION_SIGNER_REQUEST_TIMEOUT),
        )
        .await
    }

    /// Build the revocation batch for an external signer. Returns the typed data to
    /// sign and the request to submit with the signature.
    pub fn revoke_session_signer_typed_data(
        &self,
        wallet: Address,
        session_signer: Address,
        nonce: u64,
        deadline: u64,
    ) -> (serde_json::Value, SessionSignerRevocation) {
        let calls = Self::revoke_session_signer_calls(wallet, session_signer);
        let typed = self.deposit_wallet_batch_typed_data(wallet, &calls, nonce, deadline);
        (
            typed,
            SessionSignerRevocation {
                wallet_address: wallet,
                session_signer_address: session_signer,
                nonce,
                deadline,
            },
        )
    }

    /// `POST /v1/session-signers/revocations` with an owner signature produced elsewhere.
    ///
    /// Accepts Builder HMAC auth or a relayer API key, as py-sdk does; only the
    /// authorization route is restricted to Builder HMAC. `idempotency_key` is
    /// trimmed and sent as `Idempotency-Key`; reuse it when retrying the same
    /// request. The request waits up to [`crate::SESSION_SIGNER_REQUEST_TIMEOUT`].
    ///
    /// The venue answers once the key is fenced out of the registry; the on-chain
    /// revocation and the cancel-all of its open orders follow asynchronously.
    pub async fn submit_session_signer_revocation(
        &self,
        request: &SessionSignerRevocation,
        signature: &str,
        idempotency_key: &str,
    ) -> Result<SessionSignerRevocationResponse, RelayError> {
        self.post_json(
            "v1/session-signers/revocations",
            &request.body(signature),
            Self::idempotency_headers(idempotency_key)?,
            true,
            Some(crate::session_signers::SESSION_SIGNER_REQUEST_TIMEOUT),
        )
        .await
    }

    /// Authorize `session_signer` with this client's account (the owner's key) and
    /// configured Deposit Wallet: fetch the nonce, sign, submit. The idempotency key
    /// is a fresh UUID; use the two-step API to retry with the same one.
    ///
    /// Refused before any I/O for a client built with
    /// [`DepositWalletRole::SessionKey`]: session signers are managed by the owner.
    /// Also refused before any I/O for a client authenticated with a relayer API
    /// key: the authorization route accepts only Builder HMAC.
    pub async fn authorize_session_signer(
        &self,
        session_signer: Address,
        scopes: Vec<SessionSignerScope>,
    ) -> Result<SessionSignerAuthorizationResponse, RelayError> {
        let (wallet, owner) = self.session_signer_owner_context()?;
        refuse_relayer_api_key(self.auth()?, &format!("/{SESSION_SIGNER_AUTHORIZATIONS}"))?;
        // Validate before fetching a nonce; the typed-data call below repeats it.
        crate::session_signers::validate_scopes(&scopes)?;
        let nonce = self
            .get_execute_params(owner, WalletType::DepositWallet)
            .await?;
        let deadline = Self::default_deadline();
        let (_, request) = self.authorize_session_signer_typed_data(
            wallet,
            session_signer,
            scopes,
            nonce,
            deadline,
        )?;
        let calls =
            Self::authorize_session_signer_calls(wallet, session_signer, request.valid_until);
        let signature = self
            .sign_deposit_wallet_batch(wallet, &calls, nonce, deadline)
            .await?;
        self.submit_session_signer_authorization(&request, &signature, &Self::new_idempotency_key())
            .await
    }

    /// Revoke `session_signer` with this client's account (the owner's key) and
    /// configured Deposit Wallet: fetch the nonce, sign, submit. The idempotency key
    /// is a fresh UUID; use the two-step API to retry with the same one.
    ///
    /// Refused before any I/O for a client built with
    /// [`DepositWalletRole::SessionKey`]: session signers are managed by the owner.
    pub async fn revoke_session_signer(
        &self,
        session_signer: Address,
    ) -> Result<SessionSignerRevocationResponse, RelayError> {
        let (wallet, owner) = self.session_signer_owner_context()?;
        let nonce = self
            .get_execute_params(owner, WalletType::DepositWallet)
            .await?;
        let deadline = Self::default_deadline();
        let (_, request) =
            self.revoke_session_signer_typed_data(wallet, session_signer, nonce, deadline);
        let calls = Self::revoke_session_signer_calls(wallet, session_signer);
        let signature = self
            .sign_deposit_wallet_batch(wallet, &calls, nonce, deadline)
            .await?;
        self.submit_session_signer_revocation(&request, &signature, &Self::new_idempotency_key())
            .await
    }

    /// The wallet and owner address for the session-signer conveniences, refusing a
    /// session-key role, a missing wallet, an unsupported chain or a missing account.
    fn session_signer_owner_context(&self) -> Result<(Address, Address), RelayError> {
        if self.deposit_wallet_role == DepositWalletRole::SessionKey {
            return Err(RelayError::Api(
                "session signers are managed by the wallet owner, not a session key".into(),
            ));
        }
        let wallet = self.configured_deposit_wallet()?;
        self.deposit_wallet_factory()?;
        let owner = self
            .account
            .as_ref()
            .ok_or(RelayError::MissingSigner)?
            .address();
        Ok((wallet, owner))
    }

    /// The `Idempotency-Key` header map, with the key trimmed; a blank key is refused.
    fn idempotency_headers(
        idempotency_key: &str,
    ) -> Result<reqwest::header::HeaderMap, RelayError> {
        let idempotency_key = idempotency_key.trim();
        if idempotency_key.is_empty() {
            return Err(RelayError::Api("idempotency key must not be empty".into()));
        }
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Idempotency-Key",
            reqwest::header::HeaderValue::from_str(idempotency_key)
                .map_err(|e| RelayError::Api(format!("invalid idempotency key: {e}")))?,
        );
        Ok(headers)
    }

    /// A UUID v4 string for `Idempotency-Key`.
    fn new_idempotency_key() -> String {
        // Avoid a uuid dependency: 16 random bytes formatted 8-4-4-4-12 with the
        // version and variant nibbles set.
        let mut b = [0u8; 16];
        rand::RngCore::fill_bytes(&mut rand::rng(), &mut b);
        b[6] = (b[6] & 0x0f) | 0x40;
        b[8] = (b[8] & 0x3f) | 0x80;
        let h = hex::encode(b);
        format!(
            "{}-{}-{}-{}-{}",
            &h[0..8],
            &h[8..12],
            &h[12..16],
            &h[16..20],
            &h[20..32]
        )
    }

    /// Every approval py-sdk considers a fully approved Deposit Wallet to hold: 7 ERC-20
    /// `approve` calls for the maximum uint256 on pUSD, then 10 ERC-1155
    /// `setApprovalForAll(operator, true)` calls on the Conditional Tokens contract and
    /// the position manager, in py-sdk's order. The spenders are the standard and
    /// neg-risk exchanges, both collateral adapters, the V2 router, exchange V3, the
    /// perps deposit contract, the auto-redeem operator and the binary and neg-risk
    /// modules.
    ///
    /// Sign and submit the result as one batch (one relayer submit), or convert each
    /// call to a `SafeTransaction` with `operation: 0` for [`RelayClient::execute`].
    ///
    /// The addresses are Polygon mainnet's; any other chain is an error.
    pub fn deposit_wallet_trading_approvals(&self) -> Result<Vec<DepositWalletCall>, RelayError> {
        use crate::deposit_wallet::{
            erc1155_set_approval_for_all_calldata, erc20_approve_calldata,
        };
        if self.chain_id != 137 {
            return Err(RelayError::Api(format!(
                "trading approvals are only known for Polygon mainnet (137), not chain {}",
                self.chain_id
            )));
        }
        Ok(POLYGON_TRADING_APPROVALS
            .iter()
            .map(|&(kind, target, spender)| DepositWalletCall {
                target,
                value: U256::ZERO,
                data: match kind {
                    ApprovalKind::Erc20 => erc20_approve_calldata(spender, U256::MAX),
                    ApprovalKind::Erc1155 => erc1155_set_approval_for_all_calldata(spender, true),
                }
                .into(),
            })
            .collect())
    }

    /// The single `redeemPositions` call a Deposit Wallet redemption batch carries, as
    /// py-sdk builds it: pUSD collateral and the root parent collection, sent to the
    /// collateral adapter, or to the neg-risk collateral adapter when `neg_risk`.
    fn deposit_wallet_redemption_call(
        condition_id: B256,
        index_sets: &[U256],
        neg_risk: bool,
    ) -> DepositWalletCall {
        DepositWalletCall {
            target: if neg_risk {
                NEG_RISK_COLLATERAL_ADAPTER
            } else {
                COLLATERAL_ADAPTER
            },
            value: U256::ZERO,
            data: crate::deposit_wallet::redeem_positions_calldata(
                address!("C011a7E12a19f7B1f670d46F03B03f3342E82DFB"),
                condition_id,
                index_sets,
            )
            .into(),
        }
    }

    /// The redemption batch for a Deposit Wallet, for an external signer.
    ///
    /// Redeems `condition_id`'s `index_sets` with pUSD as collateral (the Deposit Wallet
    /// collateral, not the legacy USDC that Safe and Proxy redemptions use). py-sdk
    /// sends this to the collateral adapter, or the neg-risk collateral adapter for a
    /// neg-risk market, never to the Conditional Tokens contract directly; `neg_risk`
    /// is the market's flag (`GET /neg-risk?token_id=` on the CLOB, or gamma's
    /// `negRisk`). Protocol-V2 markets (router `redeem`) are not supported.
    ///
    /// Returns the typed data and the calls to pass to
    /// [`RelayClient::submit_redemption_with_signature`]. `nonce` comes from
    /// [`RelayClient::get_execute_params`] with [`WalletType::DepositWallet`].
    ///
    /// The addresses are Polygon mainnet's. This function cannot fail, so it builds
    /// the batch on any chain; submitting it on a chain without a Deposit Wallet
    /// factory is refused by [`RelayClient::submit_deposit_wallet_batch_from`].
    pub fn redeem_typed_data(
        &self,
        wallet: Address,
        condition_id: B256,
        index_sets: &[U256],
        neg_risk: bool,
        nonce: u64,
        deadline: u64,
    ) -> (serde_json::Value, Vec<DepositWalletCall>) {
        let calls = vec![Self::deposit_wallet_redemption_call(
            condition_id,
            index_sets,
            neg_risk,
        )];
        (
            self.deposit_wallet_batch_typed_data(wallet, &calls, nonce, deadline),
            calls,
        )
    }

    /// Submit a redemption batch signed elsewhere by this client's account.
    ///
    /// Pair with [`RelayClient::redeem_typed_data`]. Sends `metadata: ""`, py-sdk's
    /// default for a Deposit Wallet submission; py-sdk's own `redeem_positions` labels
    /// it `Redeem positions for condition <id>` instead, which the batch signature does
    /// not cover.
    pub async fn submit_redemption_with_signature(
        &self,
        wallet: Address,
        calls: &[DepositWalletCall],
        nonce: u64,
        deadline: u64,
        signature: &str,
    ) -> Result<SubmitResponse, RelayError> {
        self.submit_deposit_wallet_batch(wallet, calls, nonce, deadline, signature, None)
            .await
    }

    /// Redeem `condition_id` through this client's Deposit Wallet, signed by its
    /// account and submitted as one batch. `neg_risk` selects the adapter, as
    /// py-sdk does. With `estimate_gas`, simulates the adapter call from the wallet
    /// first as an early revert check; the submission itself carries no gas limit.
    ///
    /// Redeems both outcomes (index sets `[1, 2]`, py-sdk's binary index sets) with
    /// pUSD as collateral, sent to the collateral adapter, or to the neg-risk
    /// collateral adapter when `neg_risk`, never to the Conditional Tokens contract
    /// directly. `neg_risk` is the market's flag (`GET /neg-risk?token_id=` on the CLOB,
    /// or gamma's `negRisk`). Protocol-V2 markets (router `redeem`) are not supported.
    /// Sends `metadata: ""`, where py-sdk's `redeem_positions` defaults to
    /// `Redeem positions for condition <id>`; the batch signature does not cover it.
    /// See [`RelayClient::redeem_typed_data`] for an external signer.
    pub async fn submit_deposit_wallet_redemption(
        &self,
        condition_id: B256,
        neg_risk: bool,
        estimate_gas: bool,
    ) -> Result<SubmitResponse, RelayError> {
        let call = Self::deposit_wallet_redemption_call(
            condition_id,
            &[U256::from(1), U256::from(2)],
            neg_risk,
        );
        if estimate_gas {
            self.estimate_deposit_wallet_redemption_gas(&call).await?;
        }
        let tx = SafeTransaction {
            to: call.target,
            value: call.value,
            data: call.data,
            operation: CALL_OPERATION,
        };
        self.execute_deposit_wallet(vec![tx], None).await
    }

    /// Simulate a Deposit Wallet redemption call from the configured wallet, returning
    /// the gas limit with relayer overhead and a safety buffer, as
    /// `estimate_redemption_gas` does for Safe and Proxy.
    async fn estimate_deposit_wallet_redemption_gas(
        &self,
        call: &DepositWalletCall,
    ) -> Result<u64, RelayError> {
        let wallet = self.configured_deposit_wallet()?;
        self.estimate_call_gas(wallet, call.target, call.data.clone())
            .await
    }

    /// Ask the configured RPC node to simulate `from` calling `to` with `input`, and
    /// return that cost plus relayer execution overhead and a 20% safety buffer.
    async fn estimate_call_gas(
        &self,
        from: Address,
        to: Address,
        input: Bytes,
    ) -> Result<u64, RelayError> {
        let provider = ProviderBuilder::new().connect_http(
            self.contract_config
                .rpc_url
                .parse()
                .map_err(|e| RelayError::Api(format!("Invalid RPC URL: {}", e)))?,
        );
        let tx = TransactionRequest::default()
            .with_from(from)
            .with_to(to)
            .with_input(input);
        let inner_gas_used = provider
            .estimate_gas(tx)
            .await
            .map_err(|e| RelayError::Api(format!("Gas estimation failed: {}", e)))?;
        let relayer_overhead: u64 = 50_000;
        Ok((inner_gas_used + relayer_overhead) * 120 / 100)
    }

    /// Estimate gas required for a redemption transaction.
    ///
    /// Returns the estimated gas limit with relayer overhead and safety buffer included.
    /// Uses the default RPC URL configured for the current chain. The simulated call
    /// redeems against USDC on the Conditional Tokens contract, matching what
    /// [`RelayClient::submit_gasless_redemption`] sends.
    ///
    /// Safe and Proxy only. A [`WalletType::DepositWallet`] client is refused before
    /// any I/O: its redemption target depends on the market's neg-risk flag, so use
    /// [`RelayClient::submit_deposit_wallet_redemption`] with `estimate_gas`.
    ///
    /// # Arguments
    ///
    /// * `condition_id` - The condition ID to redeem
    /// * `index_sets` - The index sets to redeem
    ///
    /// # Example
    ///
    /// ```no_run
    /// use polyoxide_relay::{RelayClient, BuilderAccount, BuilderConfig, WalletType};
    /// use alloy::primitives::U256;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let builder_config = BuilderConfig::new(
    ///     "key".to_string(),
    ///     "secret".to_string(),
    ///     None,
    /// );
    /// let account = BuilderAccount::new("0x...", Some(builder_config))?;
    /// let client = RelayClient::builder()?
    ///     .with_account(account)
    ///     .wallet_type(WalletType::Proxy)
    ///     .build()?;
    ///
    /// let condition_id = [0u8; 32];
    /// let index_sets = vec![U256::from(1)];
    /// let estimated_gas = client
    ///     .estimate_redemption_gas(condition_id, index_sets)
    ///     .await?;
    /// println!("Estimated gas: {}", estimated_gas);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn estimate_redemption_gas(
        &self,
        condition_id: [u8; 32],
        index_sets: Vec<U256>,
    ) -> Result<u64, RelayError> {
        // 1. The wallet the call is simulated from; a Deposit Wallet is refused.
        let from = match self.wallet_type {
            WalletType::Proxy => self.get_expected_proxy_wallet()?,
            WalletType::Safe => self.get_expected_safe()?,
            WalletType::DepositWallet => return Err(deposit_wallet_redemption_refused()),
        };

        // 2. Define the redemption interface
        alloy::sol! {
            function redeemPositions(address collateral, bytes32 parentCollectionId, bytes32 conditionId, uint256[] indexSets);
        }

        // 3. Setup constants: USDC on Polygon, the Safe and Proxy collateral.
        let collateral =
            Address::parse_checksummed("0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174", None)
                .map_err(|e| RelayError::Api(format!("Invalid collateral address: {}", e)))?;
        let ctf_exchange =
            Address::parse_checksummed("0x4D97DCd97eC945f40cF65F87097ACe5EA0476045", None)
                .map_err(|e| RelayError::Api(format!("Invalid CTF exchange address: {}", e)))?;
        let parent_collection_id = [0u8; 32];

        // 4. Encode the redemption calldata
        let call = redeemPositionsCall {
            collateral,
            parentCollectionId: parent_collection_id.into(),
            conditionId: condition_id.into(),
            indexSets: index_sets,
        };
        let redemption_calldata = Bytes::from(call.abi_encode());

        // 5. Simulate it exactly as the wallet will execute it, plus relayer overhead
        // and a 20% safety buffer.
        self.estimate_call_gas(from, ctf_exchange, redemption_calldata)
            .await
    }

    /// Submit a gasless CTF position redemption without gas estimation.
    ///
    /// Safe and Proxy only: redeems against USDC on the Conditional Tokens contract. A
    /// [`WalletType::DepositWallet`] client is refused before any I/O; use
    /// [`RelayClient::submit_deposit_wallet_redemption`], which takes the market's
    /// neg-risk flag to pick the collateral adapter py-sdk sends to.
    pub async fn submit_gasless_redemption(
        &self,
        condition_id: [u8; 32],
        index_sets: Vec<alloy::primitives::U256>,
    ) -> Result<SubmitResponse, RelayError> {
        self.submit_gasless_redemption_with_gas_estimation(condition_id, index_sets, false)
            .await
    }

    /// Submit a gasless CTF position redemption, optionally estimating gas first.
    ///
    /// When `estimate_gas` is true, simulates the redemption against the configured
    /// RPC endpoint to determine a safe gas limit before submission.
    ///
    /// Safe and Proxy only: redeems against USDC on the Conditional Tokens contract. A
    /// [`WalletType::DepositWallet`] client is refused before any I/O; use
    /// [`RelayClient::submit_deposit_wallet_redemption`], which takes the market's
    /// neg-risk flag to pick the collateral adapter py-sdk sends to.
    pub async fn submit_gasless_redemption_with_gas_estimation(
        &self,
        condition_id: [u8; 32],
        index_sets: Vec<alloy::primitives::U256>,
        estimate_gas: bool,
    ) -> Result<SubmitResponse, RelayError> {
        if self.wallet_type == WalletType::DepositWallet {
            return Err(deposit_wallet_redemption_refused());
        }

        // 1. Define the specific interface for redemption
        alloy::sol! {
            function redeemPositions(address collateral, bytes32 parentCollectionId, bytes32 conditionId, uint256[] indexSets);
        }

        // 2. Setup Constants
        // USDC on Polygon: the Safe and Proxy collateral
        let collateral =
            Address::parse_checksummed("0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174", None)
                .map_err(|e| RelayError::Api(format!("Invalid address: {}", e)))?;
        // CTF Exchange Address on Polygon
        let ctf_exchange =
            Address::parse_checksummed("0x4D97DCd97eC945f40cF65F87097ACe5EA0476045", None)
                .map_err(|e| RelayError::Api(format!("Invalid address: {}", e)))?;
        let parent_collection_id = [0u8; 32];

        // 3. Encode the Calldata
        let call = redeemPositionsCall {
            collateral,
            parentCollectionId: parent_collection_id.into(),
            conditionId: condition_id.into(),
            indexSets: index_sets.clone(),
        };
        let data = call.abi_encode();

        // 4. Estimate gas if requested
        let gas_limit = if estimate_gas {
            Some(
                self.estimate_redemption_gas(condition_id, index_sets.clone())
                    .await?,
            )
        } else {
            None
        };

        // 5. Construct the SafeTransaction
        let tx = SafeTransaction {
            to: ctf_exchange,
            value: U256::ZERO,
            data: data.into(),
            operation: CALL_OPERATION,
        };

        // 6. Use the execute_with_gas method
        // This handles Nonce fetching, EIP-712 Signing, and Relayer submission.
        self.execute_with_gas(vec![tx], None, gas_limit).await
    }

    async fn _post_request<T: Serialize>(
        &self,
        endpoint: &str,
        body: &T,
    ) -> Result<SubmitResponse, RelayError> {
        self.post_json(
            endpoint,
            body,
            reqwest::header::HeaderMap::new(),
            true,
            None,
        )
        .await
    }

    /// POST `body` as JSON to `endpoint` under the client's auth, retrying on 429.
    ///
    /// `extra_headers` are inserted after the auth headers on every attempt, so a
    /// same-named header would overwrite an auth header: callers must not pass auth
    /// header names. When `allow_relayer_api_key` is false, a client configured with
    /// a relayer API key is refused before any I/O. `timeout`, when set, replaces the
    /// client-level timeout for each attempt; `None` keeps the client default.
    async fn post_json<B: Serialize, T: serde::de::DeserializeOwned>(
        &self,
        endpoint: &str,
        body: &B,
        extra_headers: reqwest::header::HeaderMap,
        allow_relayer_api_key: bool,
        timeout: Option<Duration>,
    ) -> Result<T, RelayError> {
        let url = self.http_client.base_url.join(endpoint)?;
        let body_str = serde_json::to_string(body)?;
        let path = format!("/{}", endpoint);
        let auth = self.auth()?;
        if !allow_relayer_api_key {
            refuse_relayer_api_key(auth, &path)?;
        }
        let mut attempt = 0u32;

        loop {
            let _permit = self.http_client.acquire_concurrency().await;
            self.http_client
                .acquire_rate_limit(&path, Some(&reqwest::Method::POST))
                .await;

            // Generate fresh auth headers each attempt (timestamps stay current)
            let mut headers = auth
                .generate_relayer_v2_headers("POST", url.path(), Some(&body_str))
                .map_err(RelayError::Api)?;
            for (name, value) in &extra_headers {
                headers.insert(name.clone(), value.clone());
            }

            headers.insert(
                reqwest::header::CONTENT_TYPE,
                reqwest::header::HeaderValue::from_static("application/json"),
            );

            let mut request = self
                .http_client
                .client
                .post(url.clone())
                .headers(headers)
                .body(body_str.clone());
            if let Some(timeout) = timeout {
                request = request.timeout(timeout);
            }
            let resp = request.send().await?;

            let status = resp.status();
            let retry_after = retry_after_header(&resp);
            tracing::debug!("Response status for {}: {}", endpoint, status);
            self.http_client
                .note_rate_limited(status, retry_after.as_deref());

            if let Some(backoff) =
                self.http_client
                    .should_retry(status, attempt, retry_after.as_deref())
            {
                attempt += 1;
                tracing::warn!(
                    "Retriable status {} on {}, retry {} after {}ms",
                    status,
                    endpoint,
                    attempt,
                    backoff.as_millis()
                );
                drop(_permit);
                tokio::time::sleep(backoff).await;
                continue;
            }

            if !status.is_success() {
                let text = resp.text().await?;
                tracing::error!(
                    "Request to {} failed with status {}: {}",
                    endpoint,
                    status,
                    polyoxide_core::truncate_for_log(&text)
                );
                return Err(RelayError::Api(format!("Request failed: {}", text)));
            }

            let response_text = resp.text().await?;

            // Try to deserialize
            return serde_json::from_str(&response_text).map_err(|e| {
                tracing::error!(
                    "Failed to decode response from {}: {}. Raw body: {}",
                    endpoint,
                    e,
                    polyoxide_core::truncate_for_log(&response_text)
                );
                RelayError::SerdeJson(e)
            });
        }
    }
}

/// Builder for configuring a [`RelayClient`].
///
/// Defaults to Polygon mainnet (chain ID 137) with the production relayer URL.
/// Use [`Default::default()`] to also read `RELAYER_URL` and `CHAIN_ID` from the environment.
pub struct RelayClientBuilder {
    base_url: String,
    chain_id: u64,
    account: Option<BuilderAccount>,
    auth: Option<AuthConfig>,
    wallet_type: WalletType,
    deposit_wallet: Option<Address>,
    deposit_wallet_role: DepositWalletRole,
    retry_config: Option<RetryConfig>,
    max_concurrent: Option<usize>,
}

impl Default for RelayClientBuilder {
    fn default() -> Self {
        let relayer_url = std::env::var("RELAYER_URL")
            .unwrap_or_else(|_| "https://relayer-v2.polymarket.com/".to_string());
        let chain_id = std::env::var("CHAIN_ID")
            .unwrap_or("137".to_string())
            .parse::<u64>()
            .unwrap_or(137);

        Self::new()
            .expect("default URL is valid")
            .url(&relayer_url)
            .expect("default URL is valid")
            .chain_id(chain_id)
    }
}

impl RelayClientBuilder {
    /// Create a new builder with default settings (Polygon mainnet, production relayer URL).
    pub fn new() -> Result<Self, RelayError> {
        let mut base_url = Url::parse("https://relayer-v2.polymarket.com")?;
        if !base_url.path().ends_with('/') {
            base_url.set_path(&format!("{}/", base_url.path()));
        }

        Ok(Self {
            base_url: base_url.to_string(),
            chain_id: 137,
            account: None,
            auth: None,
            wallet_type: WalletType::default(),
            deposit_wallet: None,
            deposit_wallet_role: DepositWalletRole::Owner,
            retry_config: None,
            max_concurrent: None,
        })
    }

    /// Set the target chain ID (default: 137 for Polygon mainnet).
    pub fn chain_id(mut self, chain_id: u64) -> Self {
        self.chain_id = chain_id;
        self
    }

    /// Set a custom relayer API base URL.
    pub fn url(mut self, url: &str) -> Result<Self, RelayError> {
        let mut base_url = Url::parse(url)?;
        if !base_url.path().ends_with('/') {
            base_url.set_path(&format!("{}/", base_url.path()));
        }
        self.base_url = base_url.to_string();
        Ok(self)
    }

    /// Attach a [`BuilderAccount`] for authenticated relay operations.
    pub fn with_account(mut self, account: BuilderAccount) -> Self {
        self.account = Some(account);
        self
    }

    /// Attach relayer API key credentials for authenticated relay operations.
    ///
    /// This is a convenience method that creates a [`BuilderAccount`] with
    /// [`RelayerApiKeyConfig`](crate::RelayerApiKeyConfig) internally. The `private_key` is still required
    /// for EIP-712 transaction signing.
    pub fn relayer_api_key(
        self,
        private_key: impl Into<String>,
        key: String,
        address: String,
    ) -> Result<Self, RelayError> {
        let account = BuilderAccount::with_relayer_api_key(private_key, key, address)?;
        Ok(self.with_account(account))
    }

    /// Authenticate relay submissions without a wallet key.
    ///
    /// For flows where the owner signs typed data out of process and this client
    /// only submits (session-signer authorization under Builder HMAC, or any
    /// `submit_*` call that takes a signature produced elsewhere). An account's own
    /// auth config, if also set, takes precedence over this.
    pub fn with_auth(mut self, auth: AuthConfig) -> Self {
        self.auth = Some(auth);
        self
    }

    /// The Deposit Wallet this client acts for. Required by every Deposit Wallet
    /// execution path; it is not derived, so a mistaken address fails at the relayer
    /// rather than silently targeting a wallet you do not own.
    pub fn deposit_wallet(mut self, wallet: Address) -> Self {
        self.deposit_wallet = Some(wallet);
        self
    }

    /// Whether the account's key is the Deposit Wallet's owner (default) or a session
    /// key; a session key's batch signatures get the session-signer envelope.
    pub fn deposit_wallet_role(mut self, role: DepositWalletRole) -> Self {
        self.deposit_wallet_role = role;
        self
    }

    /// Set the wallet type (default: [`WalletType::Safe`]).
    pub fn wallet_type(mut self, wallet_type: WalletType) -> Self {
        self.wallet_type = wallet_type;
        self
    }

    /// Set retry configuration for 429 responses
    pub fn with_retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = Some(config);
        self
    }

    /// Set the maximum number of concurrent in-flight requests.
    ///
    /// Default: 2. Prevents Cloudflare 1015 errors from request bursts.
    pub fn max_concurrent(mut self, max: usize) -> Self {
        self.max_concurrent = Some(max);
        self
    }

    /// Build the [`RelayClient`].
    ///
    /// Returns an error if the chain ID is unsupported or the base URL is invalid.
    pub fn build(self) -> Result<RelayClient, RelayError> {
        let mut base_url = Url::parse(&self.base_url)?;
        if !base_url.path().ends_with('/') {
            base_url.set_path(&format!("{}/", base_url.path()));
        }

        let contract_config = get_contract_config(self.chain_id)
            .ok_or_else(|| RelayError::Api(format!("Unsupported chain ID: {}", self.chain_id)))?;

        let mut builder = HttpClientBuilder::new(base_url.as_str())
            .with_rate_limiter(RateLimiter::relay_default())
            .with_max_concurrent(self.max_concurrent.unwrap_or(2));
        if let Some(config) = self.retry_config {
            builder = builder.with_retry_config(config);
        }
        let http_client = builder.build()?;

        let auth = self
            .account
            .as_ref()
            .and_then(|a| a.auth_config().cloned())
            .or(self.auth);

        Ok(RelayClient {
            http_client,
            chain_id: self.chain_id,
            account: self.account,
            auth,
            contract_config,
            wallet_type: self.wallet_type,
            deposit_wallet: self.deposit_wallet,
            deposit_wallet_role: self.deposit_wallet_role,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::address;

    #[tokio::test]
    async fn test_ping() {
        let client = RelayClient::builder().unwrap().build().unwrap();
        let result = client.ping().await;
        assert!(result.is_ok(), "ping failed: {:?}", result.err());
    }

    #[tokio::test]
    async fn test_default_concurrency_limit_is_2() {
        let client = RelayClient::builder().unwrap().build().unwrap();
        let mut permits = Vec::new();
        for _ in 0..2 {
            permits.push(client.http_client.acquire_concurrency().await);
        }
        assert!(permits.iter().all(|p| p.is_some()));

        let result = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            client.http_client.acquire_concurrency(),
        )
        .await;
        assert!(
            result.is_err(),
            "3rd permit should block with default limit of 2"
        );
    }

    #[test]
    fn deposit_wallet_trading_approvals_are_py_sdks_full_set_in_order_on_polygon_only() {
        let v: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/session_keys/relay_vectors.json"
        ))
        .unwrap();
        let expected = v["trading_approvals"].as_array().unwrap();
        let client = RelayClient::builder().unwrap().build().unwrap();
        let calls = client.deposit_wallet_trading_approvals().unwrap();
        assert_eq!(calls.len(), expected.len());
        for (i, (call, want)) in calls.iter().zip(expected).enumerate() {
            let target: Address = want["target"].as_str().unwrap().parse().unwrap();
            assert_eq!(call.target, target, "call {i}");
            assert_eq!(
                format!("0x{}", hex::encode(&call.data)),
                want["data"].as_str().unwrap(),
                "call {i}"
            );
            assert!(call.value.is_zero(), "call {i}");
        }

        let amoy = RelayClient::builder()
            .unwrap()
            .chain_id(80002)
            .build()
            .unwrap();
        let err = amoy.deposit_wallet_trading_approvals().unwrap_err();
        assert!(err.to_string().contains("80002"), "{err}");
    }

    #[test]
    fn deposit_wallet_metadata_defaults_to_empty_and_counts_characters_not_bytes() {
        assert_eq!(deposit_wallet_metadata(None).unwrap(), "");
        // 500 two-byte characters are 1000 bytes but within py-sdk's 500-character cap.
        assert_eq!(
            deposit_wallet_metadata(Some("é".repeat(500))).unwrap(),
            "é".repeat(500)
        );
        let err = deposit_wallet_metadata(Some("x".repeat(501))).unwrap_err();
        assert!(err.to_string().contains("at most 500"), "{err}");
    }

    #[test]
    fn idempotency_headers_trim_the_key_and_refuse_a_blank_one() {
        let headers = RelayClient::idempotency_headers("  idem-1\t").unwrap();
        assert_eq!(headers.get("Idempotency-Key").unwrap(), "idem-1");
        for blank in ["", "   "] {
            let err = RelayClient::idempotency_headers(blank)
                .unwrap_err()
                .to_string();
            assert!(err.contains("idempotency key"), "{err}");
        }
    }

    #[test]
    fn new_idempotency_key_is_a_uuid_v4() {
        let a = RelayClient::new_idempotency_key();
        let b = RelayClient::new_idempotency_key();
        assert_ne!(a, b);
        for key in [&a, &b] {
            assert_eq!(key.len(), 36, "{key}");
            let bytes = key.as_bytes();
            for i in [8, 13, 18, 23] {
                assert_eq!(bytes[i], b'-', "{key}");
            }
            assert_eq!(bytes[14], b'4', "version nibble: {key}");
            assert!(b"89ab".contains(&bytes[19]), "variant nibble: {key}");
            assert!(key
                .chars()
                .all(|c| c == '-' || c.is_ascii_digit() || ('a'..='f').contains(&c)));
        }
    }

    #[test]
    fn test_multisend_selector_matches_expected() {
        // multiSend(bytes) selector = keccak256("multiSend(bytes)")[..4] = 0x8d80ff0a
        assert_eq!(MULTISEND_SELECTOR, [0x8d, 0x80, 0xff, 0x0a]);
    }

    #[test]
    fn test_operation_constants() {
        assert_eq!(CALL_OPERATION, 0);
        assert_eq!(DELEGATE_CALL_OPERATION, 1);
        assert_eq!(PROXY_CALL_TYPE_CODE, 1);
    }

    #[test]
    fn test_contract_config_polygon_mainnet() {
        let config = get_contract_config(137);
        assert!(config.is_some(), "should return config for Polygon mainnet");
        let config = config.unwrap();
        assert!(config.proxy_factory.is_some());
        assert!(config.relay_hub.is_some());
        assert!(config.deposit_wallet_factory.is_some());
    }

    #[test]
    fn test_contract_config_amoy_testnet() {
        let config = get_contract_config(80002);
        assert!(config.is_some(), "should return config for Amoy testnet");
        let config = config.unwrap();
        assert!(
            config.proxy_factory.is_none(),
            "proxy not supported on Amoy"
        );
        assert!(
            config.relay_hub.is_none(),
            "relay hub not supported on Amoy"
        );
        assert!(
            config.deposit_wallet_factory.is_none(),
            "deposit wallets not supported on Amoy"
        );
    }

    #[test]
    fn test_contract_config_unknown_chain() {
        assert!(get_contract_config(999).is_none());
    }

    #[test]
    fn test_relay_client_builder_default() {
        let builder = RelayClientBuilder::default();
        assert_eq!(builder.chain_id, 137);
    }

    #[test]
    fn test_builder_custom_retry_config() {
        let config = RetryConfig {
            max_retries: 5,
            initial_backoff_ms: 1000,
            max_backoff_ms: 30_000,
        };
        let builder = RelayClientBuilder::new().unwrap().with_retry_config(config);
        let config = builder.retry_config.unwrap();
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.initial_backoff_ms, 1000);
    }

    // ── Builder ──────────────────────────────────────────────────

    #[test]
    fn test_builder_unsupported_chain() {
        let result = RelayClient::builder().unwrap().chain_id(999).build();
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("Unsupported chain ID"),
            "Expected unsupported chain error, got: {err_msg}"
        );
    }

    #[test]
    fn test_builder_with_wallet_type() {
        let client = RelayClient::builder()
            .unwrap()
            .wallet_type(WalletType::Proxy)
            .build()
            .unwrap();
        assert_eq!(client.wallet_type, WalletType::Proxy);
    }

    #[test]
    fn test_builder_no_account_address_is_none() {
        let client = RelayClient::builder().unwrap().build().unwrap();
        assert!(client.address().is_none());
    }

    #[test]
    fn test_builder_relayer_api_key_attaches_account() {
        let client = RelayClient::builder()
            .unwrap()
            .relayer_api_key(
                "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
                "my-relayer-key".to_string(),
                "0xabc123".to_string(),
            )
            .unwrap()
            .build()
            .unwrap();
        let account = client.account.as_ref().expect("account should be attached");
        assert!(matches!(
            account.auth_config(),
            Some(crate::config::AuthConfig::RelayerApiKey(_))
        ));
    }

    // ── address derivation (CREATE2) ────────────────────────────

    // Well-known test key: anvil/hardhat default #0
    const TEST_KEY: &str = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

    fn test_client_with_account() -> RelayClient {
        let account = crate::BuilderAccount::new(TEST_KEY, None).unwrap();
        RelayClient::builder()
            .unwrap()
            .with_account(account)
            .build()
            .unwrap()
    }

    fn approval_batch_fixture() -> (Address, Vec<DepositWalletCall>, u64, u64, Vec<u8>) {
        let v: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/session_keys/relay_vectors.json"
        ))
        .unwrap();
        let b = &v["approval_batch"];
        let wallet: Address = v["wallet"].as_str().unwrap().parse().unwrap();
        let calls = b["calls"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| DepositWalletCall {
                target: c["target"].as_str().unwrap().parse().unwrap(),
                value: c["value"].as_str().unwrap().parse().unwrap(),
                data: hex::decode(c["data"].as_str().unwrap()).unwrap().into(),
            })
            .collect();
        let nonce = b["nonce"].as_str().unwrap().parse().unwrap();
        let deadline = b["deadline"].as_str().unwrap().parse().unwrap();
        let signature = hex::decode(b["signature"].as_str().unwrap()).unwrap();
        (wallet, calls, nonce, deadline, signature)
    }

    fn client_with_role(role: DepositWalletRole) -> RelayClient {
        let account = crate::BuilderAccount::new(TEST_KEY, None).unwrap();
        RelayClient::builder()
            .unwrap()
            .with_account(account)
            .deposit_wallet_role(role)
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn owner_role_signs_the_batch_bare() {
        let (wallet, calls, nonce, deadline, owner_sig) = approval_batch_fixture();
        let client = client_with_role(DepositWalletRole::Owner);
        let sig = client
            .sign_deposit_wallet_batch(wallet, &calls, nonce, deadline)
            .await
            .unwrap();
        assert_eq!(sig, format!("0x{}", hex::encode(owner_sig)));
    }

    #[tokio::test]
    async fn session_key_role_wraps_the_batch_signature_naming_the_account() {
        // The fixture's `session_signature` names Anvil #1 as the session signer
        // while Anvil #0 signed the inner batch (PROVENANCE.md), so it cannot be
        // matched directly; the bare `signature` bytes are the pin.
        let (wallet, calls, nonce, deadline, owner_sig) = approval_batch_fixture();
        let client = client_with_role(DepositWalletRole::SessionKey);
        let anvil0 = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
        let sig = client
            .sign_deposit_wallet_batch(wallet, &calls, nonce, deadline)
            .await
            .unwrap();
        let expected = crate::deposit_wallet::wrap_session_signer(anvil0, &owner_sig);
        assert_eq!(sig, format!("0x{}", hex::encode(expected)));
    }

    #[test]
    fn expected_wallets_for_anvil0_match_the_fixture() {
        let client = test_client_with_account();
        assert_eq!(
            client.get_expected_proxy_wallet().unwrap(),
            address!("365f0CA36Ae1f641E02fE3B7743673da42A13A70")
        );
        assert_eq!(
            client.get_expected_safe().unwrap(),
            address!("d93B25cb943D14d0d34FBaF01Fc93a0f8b5F6E47")
        );
    }

    #[test]
    fn test_derive_safe_address_deterministic() {
        let client = test_client_with_account();
        let addr1 = client.get_expected_safe().unwrap();
        let addr2 = client.get_expected_safe().unwrap();
        assert_eq!(addr1, addr2);
    }

    #[test]
    fn test_derive_safe_address_nonzero() {
        let client = test_client_with_account();
        let addr = client.get_expected_safe().unwrap();
        assert_ne!(addr, Address::ZERO);
    }

    #[test]
    fn test_derive_proxy_wallet_deterministic() {
        let client = test_client_with_account();
        let addr1 = client.get_expected_proxy_wallet().unwrap();
        let addr2 = client.get_expected_proxy_wallet().unwrap();
        assert_eq!(addr1, addr2);
    }

    #[test]
    fn test_safe_and_proxy_addresses_differ() {
        let client = test_client_with_account();
        let safe = client.get_expected_safe().unwrap();
        let proxy = client.get_expected_proxy_wallet().unwrap();
        assert_ne!(safe, proxy);
    }

    #[test]
    fn test_derive_proxy_wallet_no_account() {
        let client = RelayClient::builder().unwrap().build().unwrap();
        let result = client.get_expected_proxy_wallet();
        assert!(result.is_err());
    }

    #[test]
    fn test_derive_proxy_wallet_amoy_unsupported() {
        let account = crate::BuilderAccount::new(TEST_KEY, None).unwrap();
        let client = RelayClient::builder()
            .unwrap()
            .chain_id(80002)
            .with_account(account)
            .build()
            .unwrap();
        // Amoy has no proxy_factory
        let result = client.get_expected_proxy_wallet();
        assert!(result.is_err());
    }

    // ── signature packing ───────────────────────────────────────

    #[test]
    fn test_split_and_pack_sig_safe_format() {
        let client = test_client_with_account();
        // Create a dummy signature
        let sig = alloy::primitives::Signature::from_scalars_and_parity(
            alloy::primitives::B256::from([1u8; 32]),
            alloy::primitives::B256::from([2u8; 32]),
            false, // v = 0 → Safe adjusts to 31
        );
        let packed = client.split_and_pack_sig_safe(sig);
        assert!(packed.starts_with("0x"));
        // 32 bytes r + 32 bytes s + 1 byte v = 65 bytes = 130 hex chars + "0x" prefix
        assert_eq!(packed.len(), 132);
        // v should be 31 (0x1f) when v() is false
        assert!(packed.ends_with("1f"), "expected v=31(0x1f), got: {packed}");
    }

    #[test]
    fn test_split_and_pack_sig_safe_v_true() {
        let client = test_client_with_account();
        let sig = alloy::primitives::Signature::from_scalars_and_parity(
            alloy::primitives::B256::from([0xAA; 32]),
            alloy::primitives::B256::from([0xBB; 32]),
            true, // v = 1 → Safe adjusts to 32
        );
        let packed = client.split_and_pack_sig_safe(sig);
        // v should be 32 (0x20) when v() is true
        assert!(packed.ends_with("20"), "expected v=32(0x20), got: {packed}");
    }

    #[test]
    fn test_split_and_pack_sig_proxy_format() {
        let client = test_client_with_account();
        let sig = alloy::primitives::Signature::from_scalars_and_parity(
            alloy::primitives::B256::from([1u8; 32]),
            alloy::primitives::B256::from([2u8; 32]),
            false, // v = 0 → Proxy uses 27
        );
        let packed = client.split_and_pack_sig_proxy(sig);
        assert!(packed.starts_with("0x"));
        assert_eq!(packed.len(), 132);
        // v should be 27 (0x1b) when v() is false
        assert!(packed.ends_with("1b"), "expected v=27(0x1b), got: {packed}");
    }

    #[test]
    fn test_split_and_pack_sig_proxy_v_true() {
        let client = test_client_with_account();
        let sig = alloy::primitives::Signature::from_scalars_and_parity(
            alloy::primitives::B256::from([0xAA; 32]),
            alloy::primitives::B256::from([0xBB; 32]),
            true, // v = 1 → Proxy uses 28
        );
        let packed = client.split_and_pack_sig_proxy(sig);
        // v should be 28 (0x1c) when v() is true
        assert!(packed.ends_with("1c"), "expected v=28(0x1c), got: {packed}");
    }

    // ── encode_proxy_transaction_data ───────────────────────────

    #[test]
    fn test_encode_proxy_transaction_data_single() {
        let client = test_client_with_account();
        let txns = vec![SafeTransaction {
            to: Address::ZERO,
            operation: 0,
            data: alloy::primitives::Bytes::from(vec![0xde, 0xad]),
            value: U256::ZERO,
        }];
        let encoded = client.encode_proxy_transaction_data(&txns);
        // Should produce valid ABI-encoded calldata with a 4-byte function selector
        assert!(
            encoded.len() >= 4,
            "encoded data too short: {} bytes",
            encoded.len()
        );
    }

    #[test]
    fn test_encode_proxy_transaction_data_multiple() {
        let client = test_client_with_account();
        let txns = vec![
            SafeTransaction {
                to: Address::ZERO,
                operation: 0,
                data: alloy::primitives::Bytes::from(vec![0x01]),
                value: U256::ZERO,
            },
            SafeTransaction {
                to: Address::ZERO,
                operation: 0,
                data: alloy::primitives::Bytes::from(vec![0x02]),
                value: U256::from(100),
            },
        ];
        let encoded = client.encode_proxy_transaction_data(&txns);
        assert!(encoded.len() >= 4);
        // Multiple transactions should produce longer data than a single one
        let single = client.encode_proxy_transaction_data(&txns[..1]);
        assert!(encoded.len() > single.len());
    }

    #[test]
    fn test_encode_proxy_transaction_data_empty() {
        let client = test_client_with_account();
        let encoded = client.encode_proxy_transaction_data(&[]);
        // Should still produce a valid ABI encoding with empty array
        assert!(encoded.len() >= 4);
    }

    // ── create_safe_multisend_transaction ────────────────────────

    #[test]
    fn test_multisend_single_returns_same() {
        let client = test_client_with_account();
        let tx = SafeTransaction {
            to: Address::from([0x42; 20]),
            operation: 0,
            data: alloy::primitives::Bytes::from(vec![0xAB]),
            value: U256::from(99),
        };
        let result = client.create_safe_multisend_transaction(std::slice::from_ref(&tx));
        assert_eq!(result.to, tx.to);
        assert_eq!(result.value, tx.value);
        assert_eq!(result.data, tx.data);
        assert_eq!(result.operation, tx.operation);
    }

    #[test]
    fn test_multisend_multiple_uses_delegate_call() {
        let client = test_client_with_account();
        let txns = vec![
            SafeTransaction {
                to: Address::from([0x01; 20]),
                operation: 0,
                data: alloy::primitives::Bytes::from(vec![0x01]),
                value: U256::ZERO,
            },
            SafeTransaction {
                to: Address::from([0x02; 20]),
                operation: 0,
                data: alloy::primitives::Bytes::from(vec![0x02]),
                value: U256::ZERO,
            },
        ];
        let result = client.create_safe_multisend_transaction(&txns);
        // Should be a DelegateCall (operation = 1) to the multisend address
        assert_eq!(result.operation, 1);
        assert_eq!(result.to, client.contract_config.safe_multisend);
        assert_eq!(result.value, U256::ZERO);
        // Data should start with multiSend selector: 8d80ff0a
        let data_hex = hex::encode(&result.data);
        assert!(
            data_hex.starts_with("8d80ff0a"),
            "Expected multiSend selector, got: {}",
            &data_hex[..8.min(data_hex.len())]
        );
    }
}
