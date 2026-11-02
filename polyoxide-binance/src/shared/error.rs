//! Binance's one error decode, by status first, shared by every Binance
//! product's error enum.

use std::time::Duration;

use polyoxide_core::{truncate_for_log, ApiError, ErrorResponse};
use polyoxide_venue::{class_for_status, Class};
use serde::Deserialize;

use crate::shared::weight::MAX_COOLDOWN;

/// What an unsuccessful response decodes to. A product's `From<ApiError>`
/// maps each variant one to one onto its own error enum.
#[derive(Debug)]
pub(crate) enum Decoded {
    /// Transport, decoding, or an error body in a shape Binance does not use.
    Api(ApiError),
    /// Binance's `{"code", "msg"}` body: an unknown symbol is `400` with code
    /// `-1121`.
    Venue {
        /// HTTP status.
        status: u16,
        /// Binance's error code, always negative.
        code: i64,
        /// Binance's message, clipped to 512 bytes.
        msg: String,
    },
    /// `429`.
    RateLimited {
        /// The response's `Retry-After`.
        retry_after: Option<Duration>,
    },
    /// `418`: the IP is banned.
    IpBanned {
        /// The response's `Retry-After`. The docs give bans of 2 minutes to 3 days.
        retry_after: Option<Duration>,
        /// The response body, whole: Binance's `-1003` text names when the
        /// ban ends, and the product logs it, clipped.
        body: String,
    },
    /// `451`: Binance does not serve the caller's location.
    RegionBlocked {
        /// The response body, clipped to 512 bytes.
        msg: String,
    },
    /// `403`: Binance's web application firewall refused the request.
    Forbidden {
        /// The response body, clipped to 512 bytes.
        msg: String,
    },
}

#[derive(Deserialize)]
struct VenueBody {
    code: i64,
    msg: String,
}

/// Binance's one decode: an unsuccessful response, by status first.
///
/// `418`, `429`, `451` and `403` go by status alone: their bodies were not
/// observed from a test IP, and the status is what decides what a caller
/// should do. Anything else with Binance's `{code, msg}` body is
/// [`Decoded::Venue`]; any other body stays core's [`ApiError::Response`],
/// its message and body clipped and its `Retry-After` clamped to 3 days, as
/// every arm reads it. Every other [`ApiError`] stays core's.
///
/// A `418`'s body is Binance's `-1003` text, which names when the ban ends.
/// The product's error has no field for it, so the decode hands it over whole
/// and the product logs it at WARN under its own module's target; the send
/// loop warns of the hold itself, naming the path.
pub(crate) fn decode(err: ApiError) -> Decoded {
    match err {
        ApiError::Response(response) => from_response(*response),
        other => Decoded::Api(other),
    }
}

fn from_response(mut response: ErrorResponse) -> Decoded {
    let status = response.status.as_u16();
    let retry_after = retry_after_secs(
        response
            .headers
            .get(polyoxide_core::reqwest::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok()),
    );
    match status {
        418 => Decoded::IpBanned {
            retry_after,
            body: response.body,
        },
        429 => Decoded::RateLimited { retry_after },
        451 => Decoded::RegionBlocked {
            msg: clip(&response.body),
        },
        403 => Decoded::Forbidden {
            msg: clip(&response.body),
        },
        _ => match serde_json::from_str::<VenueBody>(&response.body) {
            Ok(venue) => Decoded::Venue {
                status,
                code: venue.code,
                msg: clip(&venue.msg),
            },
            // Core read the whole body for its `error` or `message`
            // field; only what is kept is clipped. The wait is Binance's
            // reading, clamped as the other arms clamp it.
            Err(_) => {
                response.message = clip(&response.message);
                response.body = clip(&response.body);
                response.retry_after = retry_after;
                Decoded::Api(response.into())
            }
        },
    }
}

/// Parses `Retry-After` with [`polyoxide_venue::parse_retry_after`], clamped
/// to [`MAX_COOLDOWN`]: seconds, whole or fractional, and `None` for zero,
/// negative, non-finite and unparsable values.
pub(crate) fn retry_after_secs(value: Option<&str>) -> Option<Duration> {
    value.and_then(|value| polyoxide_venue::parse_retry_after(value, MAX_COOLDOWN))
}

/// Clips `text` to 512 bytes, as a message or a logged body carries it.
pub(crate) fn clip(text: &str) -> String {
    truncate_for_log(text).into_owned()
}

/// The class of a `{code, msg}` error: the status rule, with Binance's code in
/// decimal. A status that decides nothing is a `Decode`.
pub(crate) fn venue_class(status: u16, code: i64) -> Class {
    class_for_status(status)
        .unwrap_or(Class::Decode)
        .with_code(code.to_string())
}

#[cfg(test)]
mod tests {
    use polyoxide_core::reqwest::header::HeaderMap;
    use polyoxide_core::reqwest::StatusCode;

    use super::*;

    #[test]
    fn a_418_hands_its_product_the_whole_body() {
        let long = "x".repeat(10_000);
        let response = ErrorResponse::new(StatusCode::IM_A_TEAPOT, HeaderMap::new(), &long);
        let Decoded::IpBanned { retry_after, body } = decode(response.into()) else {
            panic!("a 418 is a ban");
        };
        assert_eq!(retry_after, None);
        assert_eq!(body, long);
    }
}
