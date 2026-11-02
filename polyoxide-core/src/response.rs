//! The response an [`HttpTransport`](crate::HttpTransport) hands the send
//! loop: an owned head and a lazy body.

use std::fmt;

use bytes::Bytes;
use http::{HeaderMap, StatusCode};
use polyoxide_rt::BoxFuture;
use serde::de::DeserializeOwned;
use url::Url;

use crate::error::ApiError;
use crate::transport::TransportError;

/// A response, as a transport answers one: the status, headers and URL are
/// here, and the body is read on demand.
///
/// The send loop decides on the head, releases the attempt's permit, and only
/// then reads an error's body, so a slow body holds no slot. Reading consumes
/// the response: [`bytes`](Self::bytes), [`text`](Self::text) and
/// [`json`](Self::json) each return an [`ApiError`], a transport failure
/// ([`ApiError::Transport`]) or, for the last, a body that does not parse
/// ([`ApiError::Serialization`]).
pub struct Response {
    status: StatusCode,
    headers: HeaderMap,
    url: Url,
    body: BoxFuture<Result<Bytes, TransportError>>,
}

impl Response {
    /// A response with its head and a future that reads its body.
    ///
    /// `url` is the URL the response answers, after any redirect the
    /// transport followed.
    pub fn new(
        status: StatusCode,
        headers: HeaderMap,
        url: Url,
        body: BoxFuture<Result<Bytes, TransportError>>,
    ) -> Self {
        Self {
            status,
            headers,
            url,
            body,
        }
    }

    /// The status.
    pub fn status(&self) -> StatusCode {
        self.status
    }

    /// The headers.
    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    /// The URL the response answers.
    pub fn url(&self) -> &Url {
        &self.url
    }

    /// Read the body whole.
    ///
    /// # Errors
    ///
    /// [`ApiError::Transport`] when the body could not be read.
    pub async fn bytes(self) -> Result<Bytes, ApiError> {
        self.body.await.map_err(ApiError::from)
    }

    /// Read the body as text: UTF-8, with each invalid sequence replaced by
    /// `U+FFFD`.
    ///
    /// # Errors
    ///
    /// [`ApiError::Transport`] when the body could not be read.
    pub async fn text(self) -> Result<String, ApiError> {
        let bytes = self.bytes().await?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Read the body and parse it as JSON.
    ///
    /// # Errors
    ///
    /// [`ApiError::Transport`] when the body could not be read, and
    /// [`ApiError::Serialization`] when it does not parse as `T`.
    pub async fn json<T: DeserializeOwned>(self) -> Result<T, ApiError> {
        let bytes = self.bytes().await?;
        serde_json::from_slice(&bytes).map_err(ApiError::from)
    }
}

/// Written by hand: the body is a future, which prints nothing useful, and a
/// header's value (a token, a cookie) must never reach a log line.
impl fmt::Debug for Response {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let header_names: Vec<&str> = self.headers.keys().map(|name| name.as_str()).collect();
        f.debug_struct("Response")
            .field("status", &self.status)
            .field("url", &self.url.as_str())
            .field("headers", &header_names)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use http::HeaderValue;
    use polyoxide_venue::{Class, Classify};

    use super::*;
    use crate::transport::TransportKind;

    fn url() -> Url {
        Url::parse("https://api.example.test/v1/rows").unwrap()
    }

    /// A 200 carrying `body`, with one header.
    fn answering(body: &'static [u8]) -> Response {
        let mut headers = HeaderMap::new();
        headers.insert("x-served-by", HeaderValue::from_static("test"));
        Response::new(
            StatusCode::OK,
            headers,
            url(),
            Box::pin(async move { Ok(Bytes::from_static(body)) }),
        )
    }

    /// A 200 whose body never arrives.
    fn cut_off() -> Response {
        Response::new(
            StatusCode::OK,
            HeaderMap::new(),
            url(),
            Box::pin(async {
                Err::<Bytes, _>(TransportError::new(TransportKind::Body, "cut off"))
            }),
        )
    }

    #[test]
    fn the_head_reads_without_the_body() {
        let response = answering(b"{}");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.url(), &url());
        assert_eq!(
            response
                .headers()
                .get("x-served-by")
                .and_then(|value| value.to_str().ok()),
            Some("test")
        );
    }

    #[tokio::test]
    async fn bytes_text_and_json_read_the_same_body() {
        const BODY: &[u8] = br#"{"rows":[1,2]}"#;
        assert_eq!(
            answering(BODY).bytes().await.unwrap(),
            Bytes::from_static(BODY)
        );
        assert_eq!(answering(BODY).text().await.unwrap(), r#"{"rows":[1,2]}"#);
        assert_eq!(
            answering(BODY).json::<serde_json::Value>().await.unwrap(),
            serde_json::json!({"rows": [1, 2]})
        );
    }

    #[tokio::test]
    async fn text_replaces_invalid_utf8() {
        assert_eq!(answering(b"h\xFFi").text().await.unwrap(), "h\u{FFFD}i");
    }

    #[tokio::test]
    async fn a_body_that_is_not_json_is_a_serialization_error_of_the_decode_class() {
        let err = answering(b"not json")
            .json::<serde_json::Value>()
            .await
            .unwrap_err();
        assert!(matches!(err, ApiError::Serialization(_)), "{err:?}");
        assert_eq!(err.class(), Class::Decode);
    }

    #[tokio::test]
    async fn a_body_that_fails_to_arrive_is_the_transport_error_whichever_way_it_is_read() {
        for err in [
            cut_off().bytes().await.unwrap_err(),
            cut_off().text().await.unwrap_err(),
            cut_off().json::<serde_json::Value>().await.unwrap_err(),
        ] {
            assert!(
                matches!(&err, ApiError::Transport(e) if e.kind() == TransportKind::Body),
                "{err:?}"
            );
            assert_eq!(err.class(), Class::Network);
        }
    }

    #[test]
    fn debug_names_the_headers_and_never_their_values() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", HeaderValue::from_static("Bearer s3cr3t"));
        let response = Response::new(
            StatusCode::OK,
            headers,
            url(),
            Box::pin(async { Ok(Bytes::new()) }),
        );
        let debug = format!("{response:?}");
        assert!(debug.contains("authorization"), "{debug}");
        assert!(!debug.contains("s3cr3t"), "{debug}");
        assert!(debug.contains("200"), "{debug}");
    }
}
