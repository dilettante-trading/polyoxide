use crate::account::BuilderAccount;
use crate::config::{get_contract_config, AuthConfig, BuilderConfig, ContractConfig};
use crate::error::RelayError;
use crate::types::{
    NonceResponse, RelayerApiKey, RelayerTransaction, SafeTransaction, SafeTx, SubmitResponse,
    WalletType,
};
use alloy::hex;
use alloy::network::TransactionBuilder;
use alloy::primitives::{keccak256, Address, Bytes, U256};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::TransactionRequest;
use alloy::signers::Signer;
use alloy::sol_types::{Eip712Domain, SolCall, SolStruct, SolValue};
use polyoxide_core::{retry_after_header, HttpClient, HttpClientBuilder, RateLimiter, RetryConfig};
use serde::Serialize;
use std::time::{Duration, Instant};
use url::Url;

// Safe Init Code Hash from constants.py
const SAFE_INIT_CODE_HASH: &str =
    "2bce2127ff07fb632d16c8347c4ebf501f4841168bed00d9e6ef715ddb6fcecf";

// From Polymarket Relayer Client
const PROXY_INIT_CODE_HASH: &str =
    "d21df8dc65880a8606f09fe0ce3df9b8869287ab0b058be05aa9e8af6330a00b";

// Safe/Proxy wallet operation types
const CALL_OPERATION: u8 = 0;
const DELEGATE_CALL_OPERATION: u8 = 1;

// Proxy wallet call type for ProxyTransaction struct
const PROXY_CALL_TYPE_CODE: u8 = 1;

// multiSend(bytes) function selector
const MULTISEND_SELECTOR: [u8; 4] = [0x8d, 0x80, 0xff, 0x0a];

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

/// Client for submitting gasless transactions through Polymarket's relayer service.
///
/// Supports both Safe and Proxy wallet types. Handles EIP-712 transaction signing,
/// nonce management, and multi-send batching automatically.
#[derive(Debug, Clone)]
pub struct RelayClient {
    http_client: HttpClient,
    chain_id: u64,
    account: Option<BuilderAccount>,
    contract_config: ContractConfig,
    wallet_type: WalletType,
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

            if let Some(backoff) =
                self.http_client
                    .should_retry(resp.status(), attempt, retry_after.as_deref())
            {
                attempt += 1;
                tracing::warn!(
                    "Rate limited (429) on {}, retry {} after {}ms",
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

    /// Produce GET auth headers, enforcing per-endpoint auth-scheme allow-lists.
    fn authed_get_headers(
        &self,
        path: &str,
        allow_builder: bool,
        allow_relayer_api_key: bool,
    ) -> Result<reqwest::header::HeaderMap, RelayError> {
        let account = self.account.as_ref().ok_or_else(|| {
            RelayError::Api(
                "Account missing - cannot authenticate request. Configure an account via RelayClientBuilder::with_account or ::relayer_api_key.".to_string(),
            )
        })?;
        let auth = account.auth_config().ok_or_else(|| {
            RelayError::Api(
                "No authentication configured - provide BuilderConfig or RelayerApiKeyConfig when creating the BuilderAccount".to_string(),
            )
        })?;

        match auth {
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
            if let Some(backoff) =
                self.http_client
                    .should_retry(resp.status(), attempt, retry_after.as_deref())
            {
                attempt += 1;
                tracing::warn!(
                    "Rate limited (429) on {}, retry {} after {}ms",
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
        #[derive(serde::Deserialize)]
        struct DeployedResponse {
            deployed: bool,
        }
        let url = self
            .http_client
            .base_url
            .join(&format!("deployed?address={}", safe_address))?;
        let resp = self.get_with_retry("/deployed", &url).await?;
        let data = resp.json::<DeployedResponse>().await?;
        Ok(data.deployed)
    }

    fn derive_safe_address(&self, owner: Address) -> Address {
        let salt = keccak256(owner.abi_encode());
        let init_code_hash = hex::decode(SAFE_INIT_CODE_HASH).expect("valid hex constant");

        // CREATE2: keccak256(0xff ++ address ++ salt ++ keccak256(init_code))[12..]
        let mut input = Vec::new();
        input.push(0xff);
        input.extend_from_slice(self.contract_config.safe_factory.as_slice());
        input.extend_from_slice(salt.as_slice());
        input.extend_from_slice(&init_code_hash);

        let hash = keccak256(input);
        Address::from_slice(&hash[12..])
    }

    /// Derive the expected Safe wallet address for the configured account via CREATE2.
    pub fn get_expected_safe(&self) -> Result<Address, RelayError> {
        let account = self.account.as_ref().ok_or(RelayError::MissingSigner)?;
        Ok(self.derive_safe_address(account.address()))
    }

    fn derive_proxy_wallet(&self, owner: Address) -> Result<Address, RelayError> {
        let proxy_factory = self.contract_config.proxy_factory.ok_or_else(|| {
            RelayError::Api("Proxy wallet not supported on this chain".to_string())
        })?;

        // Salt = keccak256(encodePacked(["address"], [address]))
        // encodePacked for address uses the 20 bytes directly.
        let salt = keccak256(owner.as_slice());

        let init_code_hash = hex::decode(PROXY_INIT_CODE_HASH).expect("valid hex constant");

        // CREATE2: keccak256(0xff ++ factory ++ salt ++ init_code_hash)[12..]
        let mut input = Vec::new();
        input.push(0xff);
        input.extend_from_slice(proxy_factory.as_slice());
        input.extend_from_slice(salt.as_slice());
        input.extend_from_slice(&init_code_hash);

        let hash = keccak256(input);
        Ok(Address::from_slice(&hash[12..]))
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
    /// they are encoded into the proxy's calldata format.
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

    /// Estimate gas required for a redemption transaction.
    ///
    /// Returns the estimated gas limit with relayer overhead and safety buffer included.
    /// Uses the default RPC URL configured for the current chain.
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
        // 1. Define the redemption interface
        alloy::sol! {
            function redeemPositions(address collateral, bytes32 parentCollectionId, bytes32 conditionId, uint256[] indexSets);
        }

        // 2. Setup constants
        let collateral =
            Address::parse_checksummed("0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174", None)
                .map_err(|e| RelayError::Api(format!("Invalid collateral address: {}", e)))?;
        let ctf_exchange =
            Address::parse_checksummed("0x4D97DCd97eC945f40cF65F87097ACe5EA0476045", None)
                .map_err(|e| RelayError::Api(format!("Invalid CTF exchange address: {}", e)))?;
        let parent_collection_id = [0u8; 32];

        // 3. Encode the redemption calldata
        let call = redeemPositionsCall {
            collateral,
            parentCollectionId: parent_collection_id.into(),
            conditionId: condition_id.into(),
            indexSets: index_sets,
        };
        let redemption_calldata = Bytes::from(call.abi_encode());

        // 4. Get the proxy wallet address
        let proxy_wallet = match self.wallet_type {
            WalletType::Proxy => self.get_expected_proxy_wallet()?,
            WalletType::Safe => self.get_expected_safe()?,
        };

        // 5. Create provider using the configured RPC URL
        let provider = ProviderBuilder::new().connect_http(
            self.contract_config
                .rpc_url
                .parse()
                .map_err(|e| RelayError::Api(format!("Invalid RPC URL: {}", e)))?,
        );

        // 6. Construct a mock transaction exactly as the proxy will execute it
        let tx = TransactionRequest::default()
            .with_from(proxy_wallet)
            .with_to(ctf_exchange)
            .with_input(redemption_calldata);

        // 7. Ask the Polygon node to simulate it and return the base computational cost
        let inner_gas_used = provider
            .estimate_gas(tx)
            .await
            .map_err(|e| RelayError::Api(format!("Gas estimation failed: {}", e)))?;

        // 8. Add relayer execution overhead + a 20% safety buffer
        let relayer_overhead: u64 = 50_000;
        let safe_gas_limit = (inner_gas_used + relayer_overhead) * 120 / 100;

        Ok(safe_gas_limit)
    }

    /// Submit a gasless CTF position redemption without gas estimation.
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
    pub async fn submit_gasless_redemption_with_gas_estimation(
        &self,
        condition_id: [u8; 32],
        index_sets: Vec<alloy::primitives::U256>,
        estimate_gas: bool,
    ) -> Result<SubmitResponse, RelayError> {
        // 1. Define the specific interface for redemption
        alloy::sol! {
            function redeemPositions(address collateral, bytes32 parentCollectionId, bytes32 conditionId, uint256[] indexSets);
        }

        // 2. Setup Constants
        // USDC on Polygon
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
        let url = self.http_client.base_url.join(endpoint)?;
        let body_str = serde_json::to_string(body)?;
        let path = format!("/{}", endpoint);
        let mut attempt = 0u32;

        loop {
            let _permit = self.http_client.acquire_concurrency().await;
            self.http_client
                .acquire_rate_limit(&path, Some(&reqwest::Method::POST))
                .await;

            // Generate fresh auth headers each attempt (timestamps stay current)
            let mut headers = if let Some(account) = &self.account {
                if let Some(auth) = account.auth_config() {
                    auth.generate_relayer_v2_headers("POST", url.path(), Some(&body_str))
                        .map_err(RelayError::Api)?
                } else {
                    return Err(RelayError::Api(
                        "No authentication configured - provide BuilderConfig or RelayerApiKeyConfig when creating the BuilderAccount".to_string(),
                    ));
                }
            } else {
                return Err(RelayError::Api(
                    "Account missing - cannot authenticate request".to_string(),
                ));
            };

            headers.insert(
                reqwest::header::CONTENT_TYPE,
                reqwest::header::HeaderValue::from_static("application/json"),
            );

            let resp = self
                .http_client
                .client
                .post(url.clone())
                .headers(headers)
                .body(body_str.clone())
                .send()
                .await?;

            let status = resp.status();
            let retry_after = retry_after_header(&resp);
            tracing::debug!("Response status for {}: {}", endpoint, status);

            if let Some(backoff) =
                self.http_client
                    .should_retry(status, attempt, retry_after.as_deref())
            {
                attempt += 1;
                tracing::warn!(
                    "Rate limited (429) on {}, retry {} after {}ms",
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
    wallet_type: WalletType,
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
            wallet_type: WalletType::default(),
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
    /// [`RelayerApiKeyConfig`] internally. The `private_key` is still required
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

        Ok(RelayClient {
            http_client,
            chain_id: self.chain_id,
            account: self.account,
            contract_config,
            wallet_type: self.wallet_type,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_hex_constants_are_valid() {
        hex::decode(SAFE_INIT_CODE_HASH).expect("SAFE_INIT_CODE_HASH should be valid hex");
        hex::decode(PROXY_INIT_CODE_HASH).expect("PROXY_INIT_CODE_HASH should be valid hex");
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
