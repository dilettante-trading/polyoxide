use thiserror::Error;

/// Core API error types shared across Polyoxide clients
#[derive(Error, Debug)]
pub enum ApiError {
    /// HTTP request failed
    #[error("API error: {status} - {message}")]
    Api { status: u16, message: String },

    /// Authentication failed (401/403)
    #[error("Authentication failed: {0}")]
    Authentication(String),

    /// Request validation failed (400)
    #[error("Validation error: {0}")]
    Validation(String),

    /// Rate limit exceeded (429)
    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),

    /// Request timeout
    #[error("Request timeout")]
    Timeout,

    /// Network error
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    /// JSON serialization/deserialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// URL parsing error
    #[error("URL error: {0}")]
    Url(#[from] url::ParseError),
}

impl ApiError {
    /// Create error from HTTP response
    pub async fn from_response(response: reqwest::Response) -> Self {
        let status = response.status().as_u16();

        // Get the raw response text first for debugging
        let body_text = response.text().await.unwrap_or_default();
        tracing::debug!("API error response body: {}", body_text);

        let message = serde_json::from_str::<serde_json::Value>(&body_text)
            .ok()
            .and_then(|v| {
                v.get("error")
                    .or(v.get("message"))
                    .and_then(|m| m.as_str())
                    .map(String::from)
            })
            .unwrap_or_else(|| body_text.clone());

        match status {
            401 | 403 => Self::Authentication(message),
            400 => Self::Validation(message),
            429 => Self::RateLimit(message),
            408 => Self::Timeout,
            _ => Self::Api { status, message },
        }
    }

    /// Whether re-sending the same request could plausibly produce a different result.
    ///
    /// Intended as the canonical input to a caller's retry policy, so consumers do
    /// not have to re-derive retriability from status codes or error prose.
    ///
    /// Retriable: rate limits, timeouts, connection failures, `425 Too Early`
    /// (Polymarket's matching engine restarting), and any 5xx. Not retriable:
    /// authentication failures, validation failures, and local encode/decode errors —
    /// all of which are deterministic for a given request.
    ///
    /// Note this describes the *error*, not the *operation*: a retriable error on a
    /// non-idempotent request (order placement) still needs caller-side judgement
    /// about whether resubmitting is safe.
    pub fn is_retriable(&self) -> bool {
        match self {
            // 425 Too Early is the matching engine restarting; 5xx is a server fault.
            // Both are documented upstream as "retry with exponential backoff".
            Self::Api { status, .. } => *status == 425 || *status >= 500,
            Self::RateLimit(_) | Self::Timeout => true,
            Self::Network(e) => e.is_timeout() || e.is_connect(),
            Self::Authentication(_) | Self::Validation(_) => false,
            Self::Serialization(_) | Self::Url(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_error_carries_message() {
        let err = ApiError::RateLimit("too many requests, retry after 5s".to_string());
        let display = format!("{}", err);
        assert!(
            display.contains("too many requests"),
            "RateLimit display should contain the message: {}",
            display
        );
    }

    #[test]
    fn test_rate_limit_error_display_format() {
        let err = ApiError::RateLimit("slow down".to_string());
        assert_eq!(format!("{}", err), "Rate limit exceeded: slow down");
    }

    // ── is_retriable ────────────────────────────────────────────

    #[test]
    fn test_retriable_transient_failures() {
        assert!(ApiError::RateLimit("slow down".into()).is_retriable());
        assert!(ApiError::Timeout.is_retriable());
        for status in [500u16, 502, 503, 504] {
            assert!(
                ApiError::Api {
                    status,
                    message: String::new()
                }
                .is_retriable(),
                "{status} should be retriable"
            );
        }
    }

    #[test]
    fn test_retriable_425_too_early() {
        // Polymarket returns 425 while the matching engine restarts and documents
        // it as "retry with exponential backoff".
        assert!(ApiError::Api {
            status: 425,
            message: String::new()
        }
        .is_retriable());
    }

    #[test]
    fn test_not_retriable_deterministic_failures() {
        assert!(!ApiError::Validation("bad payload".into()).is_retriable());
        assert!(!ApiError::Authentication("Invalid API key".into()).is_retriable());
        for status in [404u16, 409, 418] {
            assert!(
                !ApiError::Api {
                    status,
                    message: String::new()
                }
                .is_retriable(),
                "{status} should not be retriable"
            );
        }
    }

    #[test]
    fn test_not_retriable_local_encode_decode_failures() {
        let json_err = serde_json::from_str::<String>("not json").unwrap_err();
        assert!(!ApiError::Serialization(json_err).is_retriable());
        let url_err = url::Url::parse("://bad").unwrap_err();
        assert!(!ApiError::Url(url_err).is_retriable());
    }
}
