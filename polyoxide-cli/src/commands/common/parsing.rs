use std::time::Duration;

use color_eyre::eyre::{bail, Result};
use polyoxide_data::v2::types::ActivityType;

/// One entry of a comma-separated list flag, trimmed.
///
/// Pair it with `value_delimiter = ','` on a `Vec<String>` field: clap splits
/// the list and parses each entry. A value parser that returns the whole
/// `Vec` instead does not fit a `Vec` field, and clap panics at runtime the
/// first time the flag is given.
pub fn parse_list_entry(s: &str) -> Result<String, std::convert::Infallible> {
    Ok(s.trim().to_owned())
}

pub fn parse_duration(s: &str) -> Result<Duration, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("empty duration".to_string());
    }

    let (num, unit) = if let Some(n) = s.strip_suffix("ms") {
        (n, "ms")
    } else if let Some(n) = s.strip_suffix('s') {
        (n, "s")
    } else if let Some(n) = s.strip_suffix('m') {
        (n, "m")
    } else if let Some(n) = s.strip_suffix('h') {
        (n, "h")
    } else {
        // Default to seconds if no unit
        (s, "s")
    };

    let num: u64 = num
        .parse()
        .map_err(|_| format!("invalid number: {}", num))?;

    match unit {
        "ms" => Ok(Duration::from_millis(num)),
        "s" => Ok(Duration::from_secs(num)),
        "m" => Ok(Duration::from_secs(num * 60)),
        "h" => Ok(Duration::from_secs(num * 3600)),
        _ => Err(format!("unknown unit: {}", unit)),
    }
}

/// Parses comma-separated activity types, case-insensitively, against every
/// type the SDK knows (`ActivityType::ALL`).
pub fn parse_activity_types(input: &str) -> Result<Vec<ActivityType>> {
    let mut valid = Vec::new();
    let mut invalid = Vec::new();

    for s in input.split(',') {
        let trimmed = s.trim();
        let wire = trimmed.to_uppercase();
        match ActivityType::ALL.iter().find(|t| t.as_str() == wire) {
            Some(activity_type) => valid.push(activity_type.clone()),
            None => invalid.push(trimmed.to_string()),
        }
    }

    if !invalid.is_empty() {
        let names: Vec<String> = ActivityType::ALL
            .iter()
            .map(|t| t.as_str().to_lowercase())
            .collect();
        bail!(
            "Invalid activity type(s): {}. Valid types: {}",
            invalid.join(", "),
            names.join(", ")
        );
    }

    Ok(valid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_activity_types_valid_single() {
        let result = parse_activity_types("trade").unwrap();
        assert_eq!(result, vec![ActivityType::Trade]);
    }

    #[test]
    fn parse_activity_types_valid_multiple() {
        let result = parse_activity_types("trade,split,merge").unwrap();
        assert_eq!(
            result,
            vec![
                ActivityType::Trade,
                ActivityType::Split,
                ActivityType::Merge
            ]
        );
    }

    #[test]
    fn parse_activity_types_case_insensitive() {
        let result = parse_activity_types("Trade,SPLIT,rEdEeM").unwrap();
        assert_eq!(
            result,
            vec![
                ActivityType::Trade,
                ActivityType::Split,
                ActivityType::Redeem
            ]
        );
    }

    #[test]
    fn parse_activity_types_trims_whitespace() {
        let result = parse_activity_types(" trade , split ").unwrap();
        assert_eq!(result, vec![ActivityType::Trade, ActivityType::Split]);
    }

    #[test]
    fn parse_activity_types_rejects_invalid() {
        let err = parse_activity_types("trade,invalid,split,typo").unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("invalid"),
            "error should list 'invalid': {msg}"
        );
        assert!(msg.contains("typo"), "error should list 'typo': {msg}");
    }

    #[test]
    fn parse_activity_types_all_invalid() {
        let err = parse_activity_types("foo,bar").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("foo"), "error should list 'foo': {msg}");
        assert!(msg.contains("bar"), "error should list 'bar': {msg}");
    }

    #[test]
    fn parse_activity_types_accepts_every_v2_type() {
        let every = ActivityType::ALL
            .iter()
            .map(|t| t.as_str().to_lowercase())
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(parse_activity_types(&every).unwrap(), ActivityType::ALL);
    }

    #[test]
    fn parse_activity_types_accepts_v2_only_types() {
        assert_eq!(
            parse_activity_types("tip,maker_rebate").unwrap(),
            vec![ActivityType::Tip, ActivityType::MakerRebate]
        );
    }

    #[test]
    fn parse_activity_types_error_lists_the_valid_types() {
        let msg = parse_activity_types("nope").unwrap_err().to_string();
        assert!(msg.contains("tip") && msg.contains("deposit"), "{msg}");
    }

    // --- parse_list_entry tests ---

    #[test]
    fn parse_list_entry_trims_whitespace() {
        assert_eq!(parse_list_entry(" 0xa ").unwrap(), "0xa");
    }

    // --- parse_duration tests ---

    #[test]
    fn parse_duration_seconds_with_unit() {
        let d = parse_duration("30s").unwrap();
        assert_eq!(d, Duration::from_secs(30));
    }

    #[test]
    fn parse_duration_seconds_without_unit() {
        let d = parse_duration("45").unwrap();
        assert_eq!(d, Duration::from_secs(45));
    }

    #[test]
    fn parse_duration_milliseconds() {
        let d = parse_duration("500ms").unwrap();
        assert_eq!(d, Duration::from_millis(500));
    }

    #[test]
    fn parse_duration_minutes() {
        let d = parse_duration("5m").unwrap();
        assert_eq!(d, Duration::from_secs(300));
    }

    #[test]
    fn parse_duration_hours() {
        let d = parse_duration("2h").unwrap();
        assert_eq!(d, Duration::from_secs(7200));
    }

    #[test]
    fn parse_duration_trims_whitespace() {
        let d = parse_duration("  10s  ").unwrap();
        assert_eq!(d, Duration::from_secs(10));
    }

    #[test]
    fn parse_duration_zero() {
        let d = parse_duration("0s").unwrap();
        assert_eq!(d, Duration::from_secs(0));
    }

    #[test]
    fn parse_duration_empty_string_errors() {
        let err = parse_duration("").unwrap_err();
        assert_eq!(err, "empty duration");
    }

    #[test]
    fn parse_duration_whitespace_only_errors() {
        let err = parse_duration("   ").unwrap_err();
        assert_eq!(err, "empty duration");
    }

    #[test]
    fn parse_duration_invalid_number_errors() {
        let err = parse_duration("abcs").unwrap_err();
        assert!(err.contains("invalid number"), "got: {err}");
    }

    #[test]
    fn parse_duration_negative_number_errors() {
        let err = parse_duration("-5s").unwrap_err();
        assert!(err.contains("invalid number"), "got: {err}");
    }

    #[test]
    fn parse_duration_float_errors() {
        let err = parse_duration("1.5s").unwrap_err();
        assert!(err.contains("invalid number"), "got: {err}");
    }

    #[test]
    fn parse_activity_types_empty_string_includes_empty_entry() {
        // An empty string produces a single empty-trimmed item which is invalid
        let err = parse_activity_types("").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Invalid activity type"), "got: {msg}");
    }
}
