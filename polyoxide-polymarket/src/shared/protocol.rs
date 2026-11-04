//! Which Polymarket protocol an asset id belongs to.
//!
//! The protocol is a function of the id alone, as in py-sdk 0.12.0
//! (`_internal/protocol.py`): no lookup is made. Nothing here is public; each
//! module wraps these in its own types and errors.
use alloy::primitives::{FixedBytes, U256};

/// Parses an asset id as py-sdk does: whitespace trimmed, then decimal or a
/// `0x`, `0b` or `0o` prefix (any case), and a value that fits a uint256.
/// Anything else is `None`.
pub(crate) fn parse_position_id(id: &str) -> Option<U256> {
    let text = id.trim();
    let (digits, radix) = match text.get(..2) {
        Some(prefix) if prefix.eq_ignore_ascii_case("0x") => (&text[2..], 16_u32),
        Some(prefix) if prefix.eq_ignore_ascii_case("0b") => (&text[2..], 2),
        Some(prefix) if prefix.eq_ignore_ascii_case("0o") => (&text[2..], 8),
        _ => (text, 10),
    };
    if digits.is_empty() || !digits.chars().all(|c| c.is_digit(radix)) {
        return None;
    }
    U256::from_str_radix(digits, u64::from(radix)).ok()
}

/// Splits a V2 position id into its condition id and outcome index.
///
/// The 32-byte big-endian value's first byte is the module (1 binary, 2
/// neg-risk, 3 combinatorial), its last byte the outcome index (0 or 1), and
/// its first 31 bytes the condition id. `None` for any refusal.
pub(crate) fn decode_v2_outcome_position_id(id: &str) -> Option<(FixedBytes<31>, u8)> {
    let bytes = parse_position_id(id)?.to_be_bytes::<32>();
    if !matches!(bytes[0], 1..=3) || bytes[31] > 1 {
        return None;
    }
    Some((FixedBytes::<31>::from_slice(&bytes[..31]), bytes[31]))
}

/// Whether an id names a Protocol V2 position: py-sdk's `is_v2_position_id`
/// (bits 40 to 103 all clear) **and** `decode_v2_outcome_position_id` both
/// accept it.
///
/// One condition stricter than py-sdk's mask, which also calls toy ids such as
/// `1`, `100` and `123456` V2. Those name no V2 position (their module byte is
/// 0) and are the ids older tests sign with. It agrees with py-sdk on every id
/// a market can carry: a V1 id is a keccak hash, so its bits 40 to 103 are not
/// all clear, and a V2 id has module byte 1 to 3 and outcome byte 0 or 1.
#[cfg(feature = "clob")]
pub(crate) fn is_v2_position_id(id: &str) -> bool {
    let Some(value) = parse_position_id(id) else {
        return false;
    };
    let reserved = U256::from(u64::MAX) << 40;
    (value & reserved) == U256::ZERO && decode_v2_outcome_position_id(id).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::hex;

    const VECTORS: &str = include_str!("../../tests/fixtures/clob/protocol_v2/id_vectors.json");

    #[derive(serde::Deserialize)]
    struct Vectors {
        #[cfg(feature = "clob")]
        ids: Ids,
        position_ids: Vec<Row>,
    }

    #[cfg(feature = "clob")]
    #[derive(serde::Deserialize)]
    struct Ids {
        v1: String,
        v2_binary: String,
        v2_neg_risk: String,
        v2_combo: String,
    }

    /// One input and both of py-sdk's answers about it.
    #[derive(serde::Deserialize)]
    struct Row {
        input: String,
        is_v2_position_id: bool,
        decoded: Option<Decoded>,
    }

    #[derive(serde::Deserialize)]
    struct Decoded {
        condition_id: String,
        outcome_index: u8,
    }

    fn vectors() -> Vectors {
        serde_json::from_str(VECTORS).unwrap()
    }

    #[test]
    fn the_table_holds_the_rows_the_rule_depends_on() {
        // Not a test of production code: it proves the fixture can tell the
        // rule from py-sdk's bare mask, so matching it means something.
        let rows = vectors().position_ids;
        assert!(rows.len() >= 20, "{} rows", rows.len());
        let mask_only = rows
            .iter()
            .filter(|r| r.is_v2_position_id && r.decoded.is_none())
            .count();
        let both = rows
            .iter()
            .filter(|r| r.is_v2_position_id && r.decoded.is_some())
            .count();
        let neither = rows.iter().filter(|r| !r.is_v2_position_id).count();
        // py-sdk's mask refuses it, its decode accepts it: the conjunction
        // makes it not V2 (the row with bit 40 set).
        let decode_only = rows
            .iter()
            .filter(|r| !r.is_v2_position_id && r.decoded.is_some())
            .count();
        assert!(mask_only >= 3, "toy ids 1, 100 and 123456: {mask_only}");
        assert!(both >= 6, "the four ids and their spellings: {both}");
        assert!(neither >= 1, "ids py-sdk calls V1: {neither}");
        assert!(
            decode_only >= 1,
            "mask refuses, decode accepts: {decode_only}"
        );
    }

    #[cfg(feature = "clob")]
    #[test]
    fn the_rule_is_the_conjunction_of_py_sdks_two_answers() {
        for row in vectors().position_ids {
            let expected = row.is_v2_position_id && row.decoded.is_some();
            assert_eq!(is_v2_position_id(&row.input), expected, "{:?}", row.input);
        }
    }

    #[test]
    fn decode_agrees_with_py_sdk_on_every_row() {
        for row in vectors().position_ids {
            let got = decode_v2_outcome_position_id(&row.input);
            match row.decoded {
                Some(decoded) => {
                    let (condition_id, outcome_index) =
                        got.unwrap_or_else(|| panic!("refused {:?}", row.input));
                    assert_eq!(
                        format!("0x{}", hex::encode(condition_id)),
                        decoded.condition_id,
                        "{:?}",
                        row.input
                    );
                    assert_eq!(outcome_index, decoded.outcome_index, "{:?}", row.input);
                }
                None => assert!(got.is_none(), "accepted {:?}", row.input),
            }
        }
    }

    #[cfg(feature = "clob")]
    #[test]
    fn the_four_fixture_ids_classify_as_the_venue_issues_them() {
        let ids = vectors().ids;
        assert!(!is_v2_position_id(&ids.v1), "a CTF id is a keccak hash");
        assert!(is_v2_position_id(&ids.v2_binary));
        assert!(is_v2_position_id(&ids.v2_neg_risk));
        assert!(is_v2_position_id(&ids.v2_combo));
        assert!(parse_position_id(&ids.v1).is_some());
    }

    #[cfg(feature = "clob")]
    #[test]
    fn toy_ids_stay_v1_though_py_sdks_mask_alone_accepts_them() {
        // The divergence the rules files record: py-sdk's mask calls these V2,
        // but their module byte is 0, so they name no V2 position.
        let rows = vectors().position_ids;
        for toy in ["1", "100", "123456"] {
            let row = rows.iter().find(|r| r.input == toy).unwrap();
            assert!(row.is_v2_position_id, "py-sdk's mask on {toy}");
            assert!(row.decoded.is_none(), "py-sdk's decode on {toy}");
            assert!(!is_v2_position_id(toy), "{toy} must stay V1");
        }
    }
}
