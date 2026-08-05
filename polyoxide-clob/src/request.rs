use std::marker::PhantomData;

use alloy::primitives::Address;
use polyoxide_core::{current_timestamp, request::QueryBuilder, retry_after_header, HttpClient};
use polyoxide_core::{SignerLimiter, TradingRequest};
use reqwest::{Method, Response};
use serde::de::DeserializeOwned;

use crate::{
    account::{Credentials, Signer, Wallet},
    error::ClobError,
};

/// Authentication mode for requests
#[derive(Debug, Clone)]
pub enum AuthMode {
    None,
    L1 {
        wallet: Wallet,
        nonce: u32,
    },
    L2 {
        address: Address,
        credentials: Credentials,
        signer: Signer,
    },
}

/// Generic request builder for CLOB API
pub struct Request<T> {
    pub(crate) http_client: HttpClient,
    pub(crate) path: String,
    pub(crate) method: Method,
    pub(crate) query: Vec<(String, String)>,
    pub(crate) body: Option<serde_json::Value>,
    pub(crate) auth: AuthMode,
    pub(crate) chain_id: u64,
    /// The per-signer trading limiter and this request's shape, when the
    /// request draws on a signer's order or cancel bucket.
    ///
    /// `None` for everything else — market data, auth, rewards — which the
    /// venue does not evaluate against those buckets at all.
    pub(crate) trading: Option<(SignerLimiter, TradingRequest)>,
    pub(crate) _marker: PhantomData<T>,
}

impl<T> Request<T> {
    /// Charge this request against a signer's order or cancel bucket.
    pub(crate) fn trading(mut self, limiter: &SignerLimiter, request: TradingRequest) -> Self {
        self.trading = Some((limiter.clone(), request));
        self
    }
}

impl<T> Request<T> {
    /// Create a new GET request
    pub(crate) fn get(
        http_client: HttpClient,
        path: impl Into<String>,
        auth: AuthMode,
        chain_id: u64,
    ) -> Self {
        Self {
            http_client,
            path: path.into(),
            method: Method::GET,
            query: Vec::new(),
            body: None,
            auth,
            chain_id,
            trading: None,
            _marker: PhantomData,
        }
    }

    /// Create a new POST request
    pub(crate) fn post(
        http_client: HttpClient,
        path: String,
        auth: AuthMode,
        chain_id: u64,
    ) -> Self {
        Self {
            http_client,
            path,
            method: Method::POST,
            query: Vec::new(),
            body: None,
            auth,
            chain_id,
            trading: None,
            _marker: PhantomData,
        }
    }

    /// Create a new DELETE request
    pub(crate) fn delete(
        http_client: HttpClient,
        path: impl Into<String>,
        auth: AuthMode,
        chain_id: u64,
    ) -> Self {
        Self {
            http_client,
            path: path.into(),
            method: Method::DELETE,
            query: Vec::new(),
            body: None,
            auth,
            chain_id,
            trading: None,
            _marker: PhantomData,
        }
    }

    /// Set request body
    pub fn body<B: serde::Serialize + ?Sized>(mut self, body: &B) -> Result<Self, ClobError> {
        self.body = Some(serde_json::to_value(body)?);
        Ok(self)
    }
}

impl<T> QueryBuilder for Request<T> {
    fn add_query(&mut self, key: String, value: String) {
        self.query.push((key, value));
    }
}

impl<T: DeserializeOwned> Request<T> {
    /// Execute the request and deserialize response
    pub async fn send(self) -> Result<T, ClobError> {
        let response = self.send_raw().await?;

        let text = response.text().await?;

        // Deserialize and provide better error context
        serde_json::from_str(&text).map_err(|e| {
            tracing::error!("Deserialization failed: {}", e);
            tracing::error!(
                "Failed to deserialize: {}",
                polyoxide_core::truncate_for_log(&text)
            );
            e.into()
        })
    }

    /// Execute the request and return raw response
    pub async fn send_raw(self) -> Result<Response, ClobError> {
        let url = self.http_client.base_url.join(&self.path)?;

        let http_client = self.http_client;
        let path = self.path;
        let method = self.method;
        let query = self.query;
        let body = self.body;
        let auth = self.auth;
        let chain_id = self.chain_id;
        let trading = self.trading;
        let mut attempt = 0u32;

        loop {
            let _permit = http_client.acquire_concurrency().await;
            http_client.acquire_rate_limit(&path, Some(&method)).await;

            // The per-signer trading bucket is a second, independent limiter:
            // it counts orders where the one above counts requests. Charged
            // inside the retry loop so a retried request pays again, as the
            // venue would charge it again.
            if let Some((limiter, request)) = &trading {
                limiter.acquire(*request).await?;
            }

            // Build the base request — rebuilt each iteration so auth timestamps are fresh
            let mut request = match method {
                Method::GET => http_client.client.get(url.clone()),
                Method::POST => {
                    let mut req = http_client.client.post(url.clone());
                    if let Some(ref body) = body {
                        req = req.header("Content-Type", "application/json").json(body);
                    }
                    req
                }
                Method::DELETE => {
                    let mut req = http_client.client.delete(url.clone());
                    if let Some(ref body) = body {
                        req = req.header("Content-Type", "application/json").json(body);
                    }
                    req
                }
                Method::PUT => {
                    let mut req = http_client.client.put(url.clone());
                    if let Some(ref body) = body {
                        req = req.header("Content-Type", "application/json").json(body);
                    }
                    req
                }
                _ => return Err(ClobError::validation("Unsupported HTTP method")),
            };

            // Add query parameters
            if !query.is_empty() {
                request = request.query(&query);
            }

            // Add authentication headers (fresh timestamp on each attempt)
            request = add_auth_headers(request, &auth, &path, &method, &body, chain_id).await?;

            // Execute request
            let response = request.send().await?;
            let status = response.status();
            let retry_after = retry_after_header(&response);

            // Poly-RateLimit-Tier is the only way to learn the signer's tier,
            // so record it whatever the status — a 429 carries it too, and that
            // is exactly when the correct capacity matters most.
            if let Some((limiter, _)) = &trading {
                limiter.observe(response.headers());
            }

            if let Some(backoff) = http_client.should_retry(status, attempt, retry_after.as_deref())
            {
                attempt += 1;
                tracing::warn!(
                    "Retriable status {} on {}, retry {} after {}ms",
                    status,
                    path,
                    attempt,
                    backoff.as_millis()
                );
                drop(_permit);
                tokio::time::sleep(backoff).await;
                continue;
            }

            tracing::debug!("Response status: {}", status);

            if !status.is_success() {
                let error = ClobError::from_response(response).await;
                tracing::error!("Request failed: {:?}", error);
                return Err(error);
            }

            return Ok(response);
        }
    }
}

/// Add authentication headers based on auth mode (free function for retry loop)
async fn add_auth_headers(
    mut request: reqwest::RequestBuilder,
    auth: &AuthMode,
    path: &str,
    method: &Method,
    body: &Option<serde_json::Value>,
    chain_id: u64,
) -> Result<reqwest::RequestBuilder, ClobError> {
    match auth {
        AuthMode::None => Ok(request),
        AuthMode::L1 { wallet, nonce } => {
            use crate::core::eip712::sign_clob_auth;

            // Fresh timestamp on each attempt to avoid staleness after retry backoff
            let timestamp = current_timestamp();
            let signature = sign_clob_auth(wallet.signer(), chain_id, timestamp, *nonce).await?;

            request = request
                // EIP-55 checksummed, matching py-clob-client, which sends
                // eth_account's `signer.address()`. Note `Display`/`to_string`
                // checksums but `{:?}` lowercases — the L1 path is where this
                // can matter, since the server recovers the address from the
                // signature and compares it against this header.
                //
                // The L2 branch below deliberately still uses `{:?}`: it sends
                // lowercase today and works, so there is no evidence the server
                // is case-sensitive there and no reason to churn a working path.
                // Don't "unify" these without a live check on both.
                .header("POLY_ADDRESS", wallet.address().to_string())
                .header("POLY_SIGNATURE", signature)
                .header("POLY_TIMESTAMP", timestamp.to_string())
                .header("POLY_NONCE", nonce.to_string());

            Ok(request)
        }
        AuthMode::L2 {
            address,
            credentials,
            signer,
        } => {
            let timestamp = current_timestamp();
            let body_str = body.as_ref().map(|b| b.to_string());
            let message =
                Signer::create_message(timestamp, method.as_str(), path, body_str.as_deref());
            let signature = signer.sign(&message)?;

            request = request
                .header("POLY_ADDRESS", format!("{:?}", address))
                .header("POLY_SIGNATURE", signature)
                .header("POLY_TIMESTAMP", timestamp.to_string())
                .header("POLY_API_KEY", &credentials.key)
                .header("POLY_PASSPHRASE", &credentials.passphrase);

            Ok(request)
        }
    }
}
