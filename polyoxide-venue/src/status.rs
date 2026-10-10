//! The status-to-class map.

use crate::Class;

/// The class an HTTP error status decides, before any body is read.
///
/// | Status | Class |
/// | --- | --- |
/// | 401, 403 | [`Unauthorized`](Class::Unauthorized) |
/// | 418, 451 | [`Restricted`](Class::Restricted) |
/// | 429 | [`RateLimited`](Class::RateLimited) |
/// | 408, 425, 500–599 | [`Unavailable`](Class::Unavailable) |
/// | any other 400–499 | [`VenueRefusal`](Class::VenueRefusal) |
///
/// `None` for a status outside 400–599, which is not an error status: the
/// caller picks the class, since only it knows what such a status means
/// there (a `0` standing in for a failure that had no status, or a `3xx`
/// reported as an error). The classes come back with no code and no wait; set them with
/// [`Class::with_code`] and [`Class::with_retry_after`].
///
/// A venue overrides a status only where it documents a different meaning,
/// as Binance's firewall does with a `403` that is a [`Restricted`](Class::Restricted).
pub fn class_for_status(status: u16) -> Option<Class> {
    let class = match status {
        401 | 403 => Class::Unauthorized,
        418 | 451 => Class::Restricted,
        429 => Class::RateLimited { retry_after: None },
        408 | 425 | 500..=599 => Class::Unavailable { code: None },
        400..=499 => Class::VenueRefusal { code: None },
        _ => return None,
    };
    Some(class)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_status_row() {
        let refusal = Some(Class::VenueRefusal { code: None });
        let unavailable = Some(Class::Unavailable { code: None });
        let rows = [
            (0, None),
            (100, None),
            (200, None),
            (304, None),
            (399, None),
            (400, refusal.clone()),
            (401, Some(Class::Unauthorized)),
            (403, Some(Class::Unauthorized)),
            (404, refusal.clone()),
            (408, unavailable.clone()),
            (409, refusal.clone()),
            (418, Some(Class::Restricted)),
            (422, refusal.clone()),
            (425, unavailable.clone()),
            (429, Some(Class::RateLimited { retry_after: None })),
            (451, Some(Class::Restricted)),
            (499, refusal),
            (500, unavailable.clone()),
            (503, unavailable.clone()),
            (599, unavailable),
            (600, None),
            (999, None),
        ];
        for (status, class) in rows {
            assert_eq!(class_for_status(status), class, "status {status}");
        }
    }

    #[test]
    fn a_status_never_produces_invalid_request_network_or_decode() {
        for status in 0..=u16::MAX {
            let class = class_for_status(status);
            assert!(
                !matches!(
                    class,
                    Some(Class::InvalidRequest | Class::Network | Class::Decode)
                ),
                "status {status} gave {class:?}"
            );
        }
    }
}
