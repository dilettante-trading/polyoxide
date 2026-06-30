use polyoxide_core::{
    HttpClient, HttpClientBuilder, RateLimiter, RetryConfig, DEFAULT_POOL_SIZE, DEFAULT_TIMEOUT_MS,
};

use crate::{
    account::{Account, Credentials},
    api::{
        account::AccountApi, auth::Auth, notifications::Notifications, orders::OrderResponse,
        rewards::Rewards, Health, Markets, Orders,
    },
    core::chain::Chain,
    error::ClobError,
    request::{AuthMode, Request},
    types::*,
    utils::{
        calculate_market_order_amounts, calculate_market_price, calculate_order_amounts,
        generate_salt,
    },
};
use alloy::primitives::{Address, B256};
#[cfg(feature = "gamma")]
use polyoxide_gamma::Gamma;

const DEFAULT_BASE_URL: &str = "https://clob.polymarket.com";

/// CLOB (Central Limit Order Book) trading client for Polymarket.
///
/// Provides authenticated order creation, signing, and submission, plus read-only
/// market data and order book access. Use [`Clob::public()`] for unauthenticated
/// read-only access, or [`Clob::builder()`] for full trading capabilities.
#[derive(Clone)]
pub struct Clob {
    pub(crate) http_client: HttpClient,
    pub(crate) chain_id: u64,
    pub(crate) signature_type: SignatureType,
    /// Builder-attribution code stamped into the V2 order `builder` field. Defaults to
    /// [`B256::ZERO`] (no attribution).
    pub(crate) builder_code: B256,
    pub(crate) account: Option<Account>,
    #[cfg(feature = "gamma")]
    pub(crate) gamma: Gamma,
}

impl Clob {
    /// Create a new CLOB client with default configuration
    pub fn new(
        private_key: impl Into<String>,
        credentials: Credentials,
    ) -> Result<Self, ClobError> {
        Self::builder(private_key, credentials)?.build()
    }

    /// Create a new public CLOB client (read-only)
    pub fn public() -> Self {
        ClobBuilder::new().build().unwrap() // unwrap safe because default build never fails
    }

    /// Create a new CLOB client builder with required authentication
    pub fn builder(
        private_key: impl Into<String>,
        credentials: Credentials,
    ) -> Result<ClobBuilder, ClobError> {
        let account = Account::new(private_key, credentials)?;
        Ok(ClobBuilder::new().with_account(account))
    }

    /// Create a new CLOB client from an Account
    pub fn from_account(account: Account) -> Result<Self, ClobError> {
        ClobBuilder::new().with_account(account).build()
    }

    /// Get a reference to the account
    pub fn account(&self) -> Option<&Account> {
        self.account.as_ref()
    }

    /// Get markets namespace
    pub fn markets(&self) -> Markets {
        Markets {
            http_client: self.http_client.clone(),
            chain_id: self.chain_id,
        }
    }

    /// Get health namespace for latency and health checks
    pub fn health(&self) -> Health {
        Health {
            http_client: self.http_client.clone(),
            chain_id: self.chain_id,
        }
    }

    /// Get orders namespace
    pub fn orders(&self) -> Result<Orders, ClobError> {
        let account = self
            .account
            .as_ref()
            .ok_or_else(|| ClobError::validation("Account required for orders API"))?;

        Ok(Orders {
            http_client: self.http_client.clone(),
            wallet: account.wallet().clone(),
            credentials: account.credentials().clone(),
            signer: account.signer().clone(),
            chain_id: self.chain_id,
        })
    }

    /// Get account API namespace
    pub fn account_api(&self) -> Result<AccountApi, ClobError> {
        let account = self
            .account
            .as_ref()
            .ok_or_else(|| ClobError::validation("Account required for account API"))?;

        Ok(AccountApi {
            http_client: self.http_client.clone(),
            wallet: account.wallet().clone(),
            credentials: account.credentials().clone(),
            signer: account.signer().clone(),
            chain_id: self.chain_id,
            signature_type: self.signature_type,
        })
    }

    /// Get notifications namespace
    pub fn notifications(&self) -> Result<Notifications, ClobError> {
        let account = self
            .account
            .as_ref()
            .ok_or_else(|| ClobError::validation("Account required for notifications API"))?;

        Ok(Notifications {
            http_client: self.http_client.clone(),
            wallet: account.wallet().clone(),
            credentials: account.credentials().clone(),
            signer: account.signer().clone(),
            chain_id: self.chain_id,
            signature_type: self.signature_type,
        })
    }

    /// Get rewards namespace for liquidity reward operations
    pub fn rewards(&self) -> Result<Rewards, ClobError> {
        let account = self
            .account
            .as_ref()
            .ok_or_else(|| ClobError::validation("Account required for rewards API"))?;

        Ok(Rewards {
            http_client: self.http_client.clone(),
            wallet: account.wallet().clone(),
            credentials: account.credentials().clone(),
            signer: account.signer().clone(),
            chain_id: self.chain_id,
            signature_type: self.signature_type,
        })
    }

    /// Get auth namespace for API key management
    pub fn auth(&self) -> Result<Auth, ClobError> {
        let account = self
            .account
            .as_ref()
            .ok_or_else(|| ClobError::validation("Account required for auth API"))?;

        Ok(Auth {
            http_client: self.http_client.clone(),
            wallet: account.wallet().clone(),
            credentials: account.credentials().clone(),
            signer: account.signer().clone(),
            chain_id: self.chain_id,
        })
    }

    /// Create an unsigned order from parameters
    pub async fn create_order(
        &self,
        params: &CreateOrderParams,
        options: Option<PartialCreateOrderOptions>,
    ) -> Result<Order, ClobError> {
        let account = self
            .account
            .as_ref()
            .ok_or_else(|| ClobError::validation("Account required to create order"))?;

        params.validate()?;

        // Fetch market metadata (neg_risk and tick_size)
        let (neg_risk, tick_size) = self.get_market_metadata(&params.token_id, options).await?;

        // Calculate amounts
        let (maker_amount, taker_amount) =
            calculate_order_amounts(params.price, params.size, params.side, tick_size);

        // Resolve maker address
        let signature_type = params.signature_type.unwrap_or_default();
        if signature_type == SignatureType::Poly1271 {
            return Err(ClobError::validation(
                "Poly1271 (EIP-1271) signing is not yet supported; use EOA/PolyProxy/PolyGnosisSafe",
            ));
        }
        let maker = self
            .resolve_maker_address(params.funder, signature_type, account)
            .await?;

        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);

        // Build order
        Ok(Self::build_order_v2(
            params.token_id.clone(),
            maker,
            account.address(),
            maker_amount,
            taker_amount,
            params.side,
            signature_type,
            neg_risk,
            params.expiration,
            self.builder_code,
            B256::ZERO,
            timestamp_ms,
        ))
    }

    /// Create an unsigned market order from parameters
    pub async fn create_market_order(
        &self,
        params: &MarketOrderArgs,
        options: Option<PartialCreateOrderOptions>,
    ) -> Result<Order, ClobError> {
        let account = self
            .account
            .as_ref()
            .ok_or_else(|| ClobError::validation("Account required to create order"))?;

        if !params.amount.is_finite() {
            return Err(ClobError::validation(
                "Amount must be finite (no NaN or infinity)",
            ));
        }
        if params.amount <= 0.0 {
            return Err(ClobError::validation(format!(
                "Amount must be positive, got {}",
                params.amount
            )));
        }
        if let Some(p) = params.price {
            if !p.is_finite() || p <= 0.0 || p > 1.0 {
                return Err(ClobError::validation(format!(
                    "Price must be finite and between 0.0 and 1.0, got {}",
                    p
                )));
            }
        }

        // Fetch market metadata (neg_risk and tick_size)
        let (neg_risk, tick_size) = self.get_market_metadata(&params.token_id, options).await?;

        // Determine price
        let price = if let Some(p) = params.price {
            p
        } else {
            // Fetch orderbook and calculate price
            let book = self
                .markets()
                .order_book(params.token_id.clone())
                .send()
                .await?;

            let levels = match params.side {
                OrderSide::Buy => book.asks,
                OrderSide::Sell => book.bids,
            };

            calculate_market_price(&levels, params.amount, params.side)
                .ok_or_else(|| ClobError::validation("Not enough liquidity to fill market order"))?
        };

        // Calculate amounts
        let (maker_amount, taker_amount) =
            calculate_market_order_amounts(params.amount, price, params.side, tick_size);

        // Resolve maker address
        let signature_type = params.signature_type.unwrap_or_default();
        if signature_type == SignatureType::Poly1271 {
            return Err(ClobError::validation(
                "Poly1271 (EIP-1271) signing is not yet supported; use EOA/PolyProxy/PolyGnosisSafe",
            ));
        }
        let maker = self
            .resolve_maker_address(params.funder, signature_type, account)
            .await?;

        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);

        // Build order with expiration set to 0 for market orders
        Ok(Self::build_order_v2(
            params.token_id.clone(),
            maker,
            account.address(),
            maker_amount,
            taker_amount,
            params.side,
            signature_type,
            neg_risk,
            Some(0),
            self.builder_code,
            B256::ZERO,
            timestamp_ms,
        ))
    }
    /// Sign an order using the configured account's EIP-712 signer.
    pub async fn sign_order(&self, order: &Order) -> Result<SignedOrder, ClobError> {
        let account = self
            .account
            .as_ref()
            .ok_or_else(|| ClobError::validation("Account required to sign order"))?;
        account.sign_order(order, self.chain_id).await
    }

    // Helper methods for order creation

    /// Fetch market metadata (neg_risk and tick_size) for a token
    async fn get_market_metadata(
        &self,
        token_id: &str,
        options: Option<PartialCreateOrderOptions>,
    ) -> Result<(bool, TickSize), ClobError> {
        // Fetch or use provided neg_risk status
        let neg_risk = if let Some(neg_risk) = options.and_then(|o| o.neg_risk) {
            neg_risk
        } else {
            let neg_risk_resp = self.markets().neg_risk(token_id.to_string()).send().await?;
            neg_risk_resp.neg_risk
        };

        // Fetch or use provided tick size
        let tick_size = if let Some(tick_size) = options.and_then(|o| o.tick_size) {
            tick_size
        } else {
            let tick_size_resp = self
                .markets()
                .tick_size(token_id.to_string())
                .send()
                .await?;
            let tick_size_val = tick_size_resp
                .minimum_tick_size
                .parse::<f64>()
                .map_err(|e| {
                    ClobError::validation(format!("Invalid minimum_tick_size field: {}", e))
                })?;
            TickSize::try_from(tick_size_val)?
        };

        Ok((neg_risk, tick_size))
    }

    /// Resolve the maker address based on funder and signature type
    async fn resolve_maker_address(
        &self,
        funder: Option<Address>,
        signature_type: SignatureType,
        account: &Account,
    ) -> Result<Address, ClobError> {
        if let Some(funder) = funder {
            Ok(funder)
        } else if signature_type.is_proxy() {
            #[cfg(feature = "gamma")]
            {
                // Fetch proxy from Gamma
                let profile = self
                    .gamma
                    .user()
                    .get(account.address().to_string())
                    .send()
                    .await
                    .map_err(|e| {
                        ClobError::service(format!("Failed to fetch user profile: {}", e))
                    })?;

                profile
                    .proxy
                    .ok_or_else(|| {
                        ClobError::validation(format!(
                            "Signature type {:?} requires proxy, but none found for {}",
                            signature_type,
                            account.address()
                        ))
                    })?
                    .parse::<Address>()
                    .map_err(|e| {
                        ClobError::validation(format!(
                            "Invalid proxy address format from Gamma: {}",
                            e
                        ))
                    })
            }
            #[cfg(not(feature = "gamma"))]
            {
                Err(ClobError::validation(format!(
                    "Signature type {:?} requires the `gamma` feature to resolve proxy address; \
                     enable `polyoxide-clob/gamma` or provide an explicit `funder` address",
                    signature_type
                )))
            }
        } else {
            Ok(account.address())
        }
    }

    /// Build a V2 [`Order`] struct from the provided parameters.
    ///
    /// `timestamp_ms` supplies the order's uniqueness nonce (Unix ms); `builder` carries the
    /// builder-attribution code and `metadata` an opaque caller tag (both default to
    /// [`B256::ZERO`]). `expiration` travels on the wire for GTD orders but is not signed.
    #[allow(clippy::too_many_arguments)]
    fn build_order_v2(
        token_id: String,
        maker: Address,
        signer: Address,
        maker_amount: String,
        taker_amount: String,
        side: OrderSide,
        signature_type: SignatureType,
        neg_risk: bool,
        expiration: Option<u64>,
        builder: B256,
        metadata: B256,
        timestamp_ms: u128,
    ) -> Order {
        Order {
            salt: generate_salt(),
            maker,
            signer,
            token_id,
            maker_amount,
            taker_amount,
            side,
            expiration: expiration.unwrap_or(0).to_string(),
            signature_type,
            timestamp: timestamp_ms.to_string(),
            metadata,
            builder,
            neg_risk,
        }
    }

    /// Post multiple signed orders (up to 15)
    pub async fn post_orders(
        &self,
        orders: &[SignedOrderPayload],
    ) -> Result<Vec<OrderResponse>, ClobError> {
        let account = self
            .account
            .as_ref()
            .ok_or_else(|| ClobError::validation("Account required to post orders"))?;

        let auth = AuthMode::L2 {
            address: account.address(),
            credentials: account.credentials().clone(),
            signer: account.signer().clone(),
        };

        let payload: Vec<_> = orders
            .iter()
            .map(|o| {
                serde_json::json!({
                    "order": o.order,
                    "owner": account.credentials().key,
                    "orderType": o.order_type,
                    "postOnly": o.post_only,
                })
            })
            .collect();

        Request::post(
            self.http_client.clone(),
            "/orders".to_string(),
            auth,
            self.chain_id,
        )
        .body(&payload)?
        .send()
        .await
    }

    /// Post a signed order
    pub async fn post_order(
        &self,
        signed_order: &SignedOrder,
        order_type: OrderKind,
        post_only: bool,
    ) -> Result<OrderResponse, ClobError> {
        let account = self
            .account
            .as_ref()
            .ok_or_else(|| ClobError::validation("Account required to post order"))?;

        let auth = AuthMode::L2 {
            address: account.address(),
            credentials: account.credentials().clone(),
            signer: account.signer().clone(),
        };

        // Create the payload wrapping the signed order
        let payload = serde_json::json!({
            "order": signed_order,
            "owner": account.credentials().key,
            "orderType": order_type,
            "postOnly": post_only,
        });

        Request::post(
            self.http_client.clone(),
            "/order".to_string(),
            auth,
            self.chain_id,
        )
        .body(&payload)?
        .send()
        .await
    }

    /// Create, sign, and post an order (convenience method)
    pub async fn place_order(
        &self,
        params: &CreateOrderParams,
        options: Option<PartialCreateOrderOptions>,
    ) -> Result<OrderResponse, ClobError> {
        let order = self.create_order(params, options).await?;
        let signed_order = self.sign_order(&order).await?;
        self.post_order(&signed_order, params.order_type, params.post_only)
            .await
    }

    /// Create, sign, and post a market order (convenience method)
    pub async fn place_market_order(
        &self,
        params: &MarketOrderArgs,
        options: Option<PartialCreateOrderOptions>,
    ) -> Result<OrderResponse, ClobError> {
        let order = self.create_market_order(params, options).await?;
        let signed_order = self.sign_order(&order).await?;

        let order_type = params.order_type.unwrap_or(OrderKind::Fok);
        // Market orders are usually FOK

        self.post_order(&signed_order, order_type, false) // Market orders cannot be post_only
            .await
    }
}

/// Parameters for creating an order
#[derive(Debug, Clone)]
pub struct CreateOrderParams {
    pub token_id: String,
    pub price: f64,
    pub size: f64,
    pub side: OrderSide,
    pub order_type: OrderKind,
    pub post_only: bool,
    pub expiration: Option<u64>,
    pub funder: Option<Address>,
    pub signature_type: Option<SignatureType>,
}

impl CreateOrderParams {
    /// Validate price and size are finite and within expected ranges.
    pub fn validate(&self) -> Result<(), ClobError> {
        if !self.price.is_finite() || !self.size.is_finite() {
            return Err(ClobError::validation(
                "Price and size must be finite (no NaN or infinity)",
            ));
        }
        if self.price <= 0.0 || self.price > 1.0 {
            return Err(ClobError::validation(format!(
                "Price must be between 0.0 and 1.0, got {}",
                self.price
            )));
        }
        if self.size <= 0.0 {
            return Err(ClobError::validation(format!(
                "Size must be positive, got {}",
                self.size
            )));
        }
        Ok(())
    }
}

/// Payload for batch order submission via [`Clob::post_orders`]
#[derive(Debug, Clone)]
pub struct SignedOrderPayload {
    pub order: SignedOrder,
    pub order_type: OrderKind,
    pub post_only: bool,
}

/// Builder for CLOB client
pub struct ClobBuilder {
    base_url: String,
    timeout_ms: u64,
    pool_size: usize,
    chain: Chain,
    signature_type: SignatureType,
    builder_code: B256,
    account: Option<Account>,
    #[cfg(feature = "gamma")]
    gamma: Option<Gamma>,
    retry_config: Option<RetryConfig>,
    max_concurrent: Option<usize>,
}

impl ClobBuilder {
    /// Create a new builder with default configuration
    pub fn new() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_string(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            pool_size: DEFAULT_POOL_SIZE,
            chain: Chain::PolygonMainnet,
            signature_type: SignatureType::Eoa,
            builder_code: B256::ZERO,
            account: None,
            #[cfg(feature = "gamma")]
            gamma: None,
            retry_config: None,
            max_concurrent: None,
        }
    }

    /// Set account for the client
    pub fn with_account(mut self, account: Account) -> Self {
        self.account = Some(account);
        self
    }

    /// Set base URL for the API
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Set request timeout in milliseconds
    pub fn timeout_ms(mut self, timeout: u64) -> Self {
        self.timeout_ms = timeout;
        self
    }

    /// Set connection pool size
    pub fn pool_size(mut self, size: usize) -> Self {
        self.pool_size = size;
        self
    }

    /// Set chain
    pub fn chain(mut self, chain: Chain) -> Self {
        self.chain = chain;
        self
    }

    /// Set the account signature type used to derive the on-chain address for
    /// authenticated read endpoints (balances, notifications, rewards).
    ///
    /// Defaults to [`SignatureType::Eoa`]. Set this to [`SignatureType::PolyProxy`]
    /// or [`SignatureType::PolyGnosisSafe`] when the API credentials belong to a
    /// Polymarket proxy / Gnosis Safe wallet, so the server resolves the correct
    /// address rather than the bare EOA.
    pub fn signature_type(mut self, signature_type: SignatureType) -> Self {
        self.signature_type = signature_type;
        self
    }

    /// Set the builder-attribution code stamped into the `builder` field of every V2 order
    /// created by this client.
    ///
    /// Defaults to [`B256::ZERO`] (no attribution). Set this to the 32-byte code issued to
    /// your integration to attribute order flow (and any associated builder fees).
    pub fn builder_code(mut self, code: B256) -> Self {
        self.builder_code = code;
        self
    }

    /// Set Gamma client
    #[cfg(feature = "gamma")]
    pub fn gamma(mut self, gamma: Gamma) -> Self {
        self.gamma = Some(gamma);
        self
    }

    /// Set retry configuration for 429 responses
    pub fn with_retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = Some(config);
        self
    }

    /// Set the maximum number of concurrent in-flight requests.
    ///
    /// Default: 8. Prevents Cloudflare 1015 errors from request bursts.
    pub fn max_concurrent(mut self, max: usize) -> Self {
        self.max_concurrent = Some(max);
        self
    }

    /// Build the CLOB client
    pub fn build(self) -> Result<Clob, ClobError> {
        let mut builder = HttpClientBuilder::new(&self.base_url)
            .timeout_ms(self.timeout_ms)
            .pool_size(self.pool_size)
            .with_rate_limiter(RateLimiter::clob_default())
            .with_max_concurrent(self.max_concurrent.unwrap_or(8));
        if let Some(config) = self.retry_config {
            builder = builder.with_retry_config(config);
        }
        let http_client = builder.build()?;

        #[cfg(feature = "gamma")]
        let gamma = if let Some(gamma) = self.gamma {
            gamma
        } else {
            polyoxide_gamma::Gamma::builder()
                .timeout_ms(self.timeout_ms)
                .pool_size(self.pool_size)
                .build()
                .map_err(|e| {
                    ClobError::service(format!("Failed to build default Gamma client: {}", e))
                })?
        };

        Ok(Clob {
            http_client,
            chain_id: self.chain.chain_id(),
            signature_type: self.signature_type,
            builder_code: self.builder_code,
            account: self.account,
            #[cfg(feature = "gamma")]
            gamma,
        })
    }
}

impl Default for ClobBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_order_v2_sets_builder_and_timestamp() {
        let code = alloy::primitives::B256::from([0x11u8; 32]);
        let order = Clob::build_order_v2(
            "100".to_string(),
            alloy::primitives::Address::ZERO,
            alloy::primitives::Address::ZERO,
            "1000000".to_string(),
            "5000000".to_string(),
            OrderSide::Buy,
            SignatureType::PolyProxy,
            false,
            Some(0),
            code,
            alloy::primitives::B256::ZERO,
            1_700_000_000_000,
        );
        assert_eq!(order.builder, code);
        assert_eq!(order.metadata, alloy::primitives::B256::ZERO);
        assert_eq!(order.timestamp, "1700000000000");
        assert_eq!(order.expiration, "0");
    }

    #[test]
    fn test_builder_custom_max_concurrent() {
        let builder = ClobBuilder::new().max_concurrent(16);
        assert_eq!(builder.max_concurrent, Some(16));
    }

    #[tokio::test]
    async fn test_default_concurrency_limit_is_8() {
        let clob = Clob::public();
        let mut permits = Vec::new();
        for _ in 0..8 {
            permits.push(clob.http_client.acquire_concurrency().await);
        }
        assert!(permits.iter().all(|p| p.is_some()));

        let result = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            clob.http_client.acquire_concurrency(),
        )
        .await;
        assert!(
            result.is_err(),
            "9th permit should block with default limit of 8"
        );
    }

    #[test]
    fn test_builder_custom_retry_config() {
        let config = RetryConfig {
            max_retries: 5,
            initial_backoff_ms: 1000,
            max_backoff_ms: 30_000,
        };
        let builder = ClobBuilder::new().with_retry_config(config);
        let config = builder.retry_config.unwrap();
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.initial_backoff_ms, 1000);
    }

    fn make_params(price: f64, size: f64) -> CreateOrderParams {
        CreateOrderParams {
            token_id: "test".to_string(),
            price,
            size,
            side: OrderSide::Buy,
            order_type: OrderKind::Gtc,
            post_only: false,
            expiration: None,
            funder: None,
            signature_type: None,
        }
    }

    #[test]
    fn test_validate_rejects_nan_price() {
        let params = make_params(f64::NAN, 100.0);
        let err = params.validate().unwrap_err();
        assert!(err.to_string().contains("finite"));
    }

    #[test]
    fn test_validate_rejects_nan_size() {
        let params = make_params(0.5, f64::NAN);
        let err = params.validate().unwrap_err();
        assert!(err.to_string().contains("finite"));
    }

    #[test]
    fn test_validate_rejects_infinite_price() {
        let params = make_params(f64::INFINITY, 100.0);
        let err = params.validate().unwrap_err();
        assert!(err.to_string().contains("finite"));
    }

    #[test]
    fn test_validate_rejects_infinite_size() {
        let params = make_params(0.5, f64::INFINITY);
        let err = params.validate().unwrap_err();
        assert!(err.to_string().contains("finite"));
    }

    #[test]
    fn test_validate_rejects_neg_infinity_size() {
        let params = make_params(0.5, f64::NEG_INFINITY);
        let err = params.validate().unwrap_err();
        assert!(err.to_string().contains("finite"));
    }

    #[test]
    fn test_validate_rejects_price_out_of_range() {
        let params = make_params(1.5, 100.0);
        let err = params.validate().unwrap_err();
        assert!(err.to_string().contains("between 0.0 and 1.0"));
    }

    #[test]
    fn test_validate_rejects_zero_price() {
        let params = make_params(0.0, 100.0);
        let err = params.validate().unwrap_err();
        assert!(err.to_string().contains("between 0.0 and 1.0"));
    }

    #[test]
    fn test_validate_rejects_negative_size() {
        let params = make_params(0.5, -10.0);
        let err = params.validate().unwrap_err();
        assert!(err.to_string().contains("positive"));
    }

    #[test]
    fn test_validate_accepts_valid_params() {
        let params = make_params(0.5, 100.0);
        assert!(params.validate().is_ok());
    }

    #[test]
    fn test_validate_accepts_boundary_price() {
        // Price exactly 1.0 should be valid
        let params = make_params(1.0, 100.0);
        assert!(params.validate().is_ok());
    }
}
