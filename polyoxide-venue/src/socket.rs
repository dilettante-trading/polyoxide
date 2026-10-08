//! The socket half of the class table: close codes and handshake statuses.
//!
//! A transport error's own variants are matched in each socket crate, since
//! this crate depends on no socket library. Those matches follow one table:
//! an I/O error (a TLS EOF included), a protocol error and a closed connection
//! are [`Network`](Class::Network); a bad URL, a malformed handshake request,
//! a TLS configuration error, an attack-attempt detection, a capacity limit
//! and a use after close are [`InvalidRequest`](Class::InvalidRequest); a
//! handshake refused with an HTTP status goes through
//! [`class_for_handshake_status`]; anything else is
//! [`Network`](Class::Network).

use std::sync::Arc;

use crate::{class_for_status, Class};

/// The class of a socket the server closed, from the close frame's code.
///
/// | Close code | Class |
/// | --- | --- |
/// | 1002, 1003, 1007, 1008, 1009, 1010, 4000–4999 | [`VenueRefusal`](Class::VenueRefusal), with the code in decimal |
/// | everything else, and no code at all | [`Network`](Class::Network) |
///
/// The `VenueRefusal` codes say the server objected to what it was sent: a
/// protocol violation, an unacceptable or oversized message, a policy breach,
/// or an application's own reason. Every other code, 1000, 1001, 1006 and
/// 1011–1013 among them, says the connection ended without the request being
/// at fault.
pub fn class_for_close_code(code: Option<u16>) -> Class {
    match code {
        Some(code @ (1002 | 1003 | 1007..=1010 | 4000..=4999)) => Class::VenueRefusal {
            code: Some(Arc::from(code.to_string())),
        },
        _ => Class::Network,
    }
}

/// The class of a WebSocket upgrade the server refused with an HTTP status.
///
/// The status rule ([`class_for_status`]), with any status outside 400–599 a
/// [`Decode`](Class::Decode): the server answered, but not with an upgrade.
pub fn class_for_handshake_status(status: u16) -> Class {
    class_for_status(status).unwrap_or(Class::Decode)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refusal(code: &str) -> Class {
        Class::VenueRefusal {
            code: Some(Arc::from(code)),
        }
    }

    #[test]
    fn every_close_code_row() {
        let rows = [
            // The codes the table names.
            (Some(1000), Class::Network),
            (Some(1001), Class::Network),
            (Some(1006), Class::Network),
            (Some(1011), Class::Network),
            (Some(1012), Class::Network),
            (Some(1013), Class::Network),
            (Some(1002), refusal("1002")),
            (Some(1003), refusal("1003")),
            (Some(1007), refusal("1007")),
            (Some(1008), refusal("1008")),
            (Some(1009), refusal("1009")),
            (Some(1010), refusal("1010")),
            (Some(4000), refusal("4000")),
            (Some(4404), refusal("4404")),
            (Some(4999), refusal("4999")),
            // The ones it does not.
            (None, Class::Network),
            (Some(1004), Class::Network),
            (Some(1005), Class::Network),
            (Some(1014), Class::Network),
            (Some(1015), Class::Network),
            (Some(2999), Class::Network),
            (Some(3000), Class::Network),
            (Some(3999), Class::Network),
            (Some(5000), Class::Network),
        ];
        for (code, class) in rows {
            assert_eq!(class_for_close_code(code), class, "close code {code:?}");
        }
    }

    #[test]
    fn a_handshake_status_follows_the_status_rule() {
        let rows = [
            (101, Class::Decode),
            (200, Class::Decode),
            (301, Class::Decode),
            (400, Class::VenueRefusal { code: None }),
            (401, Class::Unauthorized),
            (403, Class::Unauthorized),
            (404, Class::VenueRefusal { code: None }),
            (418, Class::Restricted),
            (429, Class::RateLimited { retry_after: None }),
            (451, Class::Restricted),
            (502, Class::Unavailable { code: None }),
            (503, Class::Unavailable { code: None }),
        ];
        for (status, class) in rows {
            assert_eq!(class_for_handshake_status(status), class, "status {status}");
        }
    }
}
