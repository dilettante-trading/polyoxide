//! The unified client: one CLOB, Gamma and Data API client, built from one
//! account, with `clob`, `gamma` and `data` all on (Story 5.7 moved it here
//! from the `polyoxide` umbrella).

use std::time::Duration;

use polyoxide_venue::{Class, Classify};

use crate::clob::{Account, Chain, Clob, ClobBuilder, ClobError};
use crate::data::{DataApi, DataApiBuilder, DataError};
use crate::gamma::{Gamma, GammaError};

/// Why [`PolymarketBuilder::build`] failed: the error of the client that could
/// not be built.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum BuildError {
    /// CLOB API error
    #[error("CLOB error: {0}")]
    Clob(#[from] ClobError),

    /// Gamma API error
    #[error("Gamma error: {0}")]
    Gamma(#[from] GammaError),

    /// Data API error
    #[error("Data error: {0}")]
    Data(#[from] DataError),
}

/// Each variant delegates to the error it wraps.
impl Classify for BuildError {
    fn class(&self) -> Class {
        match self {
            Self::Clob(err) => err.class(),
            Self::Gamma(err) => err.class(),
            Self::Data(err) => err.class(),
        }
    }

    fn is_fault(&self) -> bool {
        match self {
            Self::Clob(err) => err.is_fault(),
            Self::Gamma(err) => err.is_fault(),
            Self::Data(err) => err.is_fault(),
        }
    }

    fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::Clob(err) => Classify::retry_after(err),
            Self::Gamma(err) => Classify::retry_after(err),
            Self::Data(err) => Classify::retry_after(err),
        }
    }
}

/// Unified Polymarket client
#[derive(Clone)]
pub struct Polymarket {
    /// CLOB (trading) API client
    pub clob: Clob,
    /// Gamma (market data) API client
    pub gamma: Gamma,
    /// Data API client (user positions, trades, activity)
    pub data: DataApi,
}

impl Polymarket {
    /// Create a new client from an Account
    pub fn new(account: Account) -> Result<Self, BuildError> {
        PolymarketBuilder::new(account).build()
    }

    /// Create a new builder with an Account
    pub fn builder(account: Account) -> PolymarketBuilder {
        PolymarketBuilder::new(account)
    }
}

/// Builder for Polymarket client
pub struct PolymarketBuilder {
    clob_base_url: Option<String>,
    gamma_base_url: Option<String>,
    data_base_url: Option<String>,
    timeout_ms: Option<u64>,
    chain: Option<Chain>,
    account: Account,
}

impl PolymarketBuilder {
    fn new(account: Account) -> Self {
        Self {
            clob_base_url: None,
            gamma_base_url: None,
            data_base_url: None,
            timeout_ms: None,
            chain: None,
            account,
        }
    }

    /// Set CLOB base URL
    pub fn clob_base_url(mut self, url: impl Into<String>) -> Self {
        self.clob_base_url = Some(url.into());
        self
    }

    /// Set Gamma base URL
    pub fn gamma_base_url(mut self, url: impl Into<String>) -> Self {
        self.gamma_base_url = Some(url.into());
        self
    }

    /// Set Data API base URL
    pub fn data_base_url(mut self, url: impl Into<String>) -> Self {
        self.data_base_url = Some(url.into());
        self
    }

    /// Set request timeout in milliseconds
    pub fn timeout_ms(mut self, timeout: u64) -> Self {
        self.timeout_ms = Some(timeout);
        self
    }

    /// Set chain
    pub fn chain(mut self, chain: Chain) -> Self {
        self.chain = Some(chain);
        self
    }

    /// Build the Polymarket client
    pub fn build(self) -> Result<Polymarket, BuildError> {
        // Build Gamma client
        let mut gamma_builder = Gamma::builder();

        if let Some(url) = self.gamma_base_url {
            gamma_builder = gamma_builder.base_url(url);
        }
        if let Some(timeout) = self.timeout_ms {
            gamma_builder = gamma_builder.timeout_ms(timeout);
        }

        let gamma = gamma_builder.build()?;

        // Build Data API client
        let mut data_builder = DataApiBuilder::default();

        if let Some(url) = self.data_base_url {
            data_builder = data_builder.base_url(url);
        }
        if let Some(timeout) = self.timeout_ms {
            data_builder = data_builder.timeout_ms(timeout);
        }

        let data = data_builder.build()?;

        // Build CLOB client, which reads public trades through the Data client
        let mut clob_builder = ClobBuilder::new()
            .with_account(self.account)
            .data(data.clone());

        if let Some(url) = self.clob_base_url {
            clob_builder = clob_builder.base_url(url);
        }
        if let Some(timeout) = self.timeout_ms {
            clob_builder = clob_builder.timeout_ms(timeout);
        }
        if let Some(chain) = self.chain {
            clob_builder = clob_builder.chain(chain);
        }

        let clob = clob_builder.build()?;

        Ok(Polymarket { clob, gamma, data })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clob::Credentials;
    use polyoxide_core::reqwest::header::{HeaderMap, HeaderValue, RETRY_AFTER};
    use polyoxide_core::reqwest::StatusCode;
    use polyoxide_core::{ApiError, ErrorResponse};

    // Anvil's first well-known key: any valid key builds the client.
    const TEST_KEY: &str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

    fn builder() -> PolymarketBuilder {
        let credentials = Credentials {
            key: "key".into(),
            secret: "c2VjcmV0".into(),
            passphrase: "passphrase".into(),
        };
        Polymarket::builder(Account::new(TEST_KEY, credentials).expect("a valid test key"))
    }

    /// A response with `status`, an empty body and, when given, `Retry-After`.
    fn response(status: u16, retry_after: Option<&'static str>) -> ApiError {
        let mut headers = HeaderMap::new();
        if let Some(seconds) = retry_after {
            headers.insert(RETRY_AFTER, HeaderValue::from_static(seconds));
        }
        ErrorResponse::new(StatusCode::from_u16(status).unwrap(), headers, "").into()
    }

    #[test]
    fn a_build_failure_names_its_client_and_classifies_as_its_error() {
        // A base URL that does not parse fails the one client it is set on,
        // whose error is an `InvalidRequest`.
        let failures = [
            builder().clob_base_url("not a url").build().err(),
            builder().gamma_base_url("not a url").build().err(),
            builder().data_base_url("not a url").build().err(),
        ];
        let [clob, gamma, data] = failures.map(|err| err.expect("a base URL that does not parse"));
        assert!(matches!(clob, BuildError::Clob(_)), "{clob:?}");
        assert!(matches!(gamma, BuildError::Gamma(_)), "{gamma:?}");
        assert!(matches!(data, BuildError::Data(_)), "{data:?}");

        // Each variant delegates, so an error that is not an `InvalidRequest`
        // keeps its own class, fault and wait.
        let wait = Some(Duration::from_secs(7));
        let rows = [
            (clob, Class::InvalidRequest, true, None),
            (gamma, Class::InvalidRequest, true, None),
            (data, Class::InvalidRequest, true, None),
            (
                BuildError::from(ClobError::FakUnmatched {
                    message: "no orders found to match with FAK order".into(),
                }),
                Class::VenueRefusal { code: None },
                false,
                None,
            ),
            (
                BuildError::from(ClobError::from(response(429, Some("7")))),
                Class::RateLimited { retry_after: wait },
                true,
                wait,
            ),
            (
                BuildError::from(GammaError::from(response(503, None))),
                Class::Unavailable { code: None },
                true,
                None,
            ),
            (
                BuildError::from(DataError::Pagination("stuck".into())),
                Class::Decode,
                true,
                None,
            ),
        ];
        for (err, class, fault, retry_after) in rows {
            assert_eq!(err.class(), class, "{err:?}");
            assert_eq!(err.is_fault(), fault, "{err:?}");
            assert_eq!(Classify::retry_after(&err), retry_after, "{err:?}");
            assert_eq!(err.is_retriable(), class.is_retriable(), "{err:?}");
        }
    }
}
