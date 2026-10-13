//! Shared by `live_api.rs`, whose holders probe selects on it, and
//! `holders_shape.rs`, which pins it offline.

/// Whether `condition_id` has the shape `GET /holders` validates `market`
/// against: `0x` followed by exactly 64 hex digits.
///
/// The check runs before any lookup, and a value that fails it is reported as
/// `required query param 'market' not provided` — the *missing*-parameter
/// message, not a malformed-value one. A caller reading that error has no way
/// to tell it sent a bad id rather than none.
pub fn is_hash64(condition_id: &str) -> bool {
    condition_id.len() == 66
        && condition_id.starts_with("0x")
        && condition_id[2..].bytes().all(|b| b.is_ascii_hexdigit())
}
