use std::path::Path;

use color_eyre::eyre::{Context, Result};
use polyoxide_gamma::Gamma;

use crate::commands::clob::prices::types::Target;

/// Deduplicate token ids into `Target`s, preserving first-seen order.
pub fn dedupe_targets(ids: impl IntoIterator<Item = String>) -> Vec<Target> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for id in ids {
        if seen.insert(id.clone()) {
            out.push(Target { token_id: id });
        }
    }
    out
}

/// Read a newline-delimited token-id file. Blank lines and lines whose first
/// non-whitespace character is `#` are ignored; other lines are trimmed.
pub fn read_ids_file(path: &Path) -> Result<Vec<String>> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("reading token id file {}", path.display()))?;
    Ok(contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(String::from)
        .collect())
}

/// Parse a Gamma `clobTokenIds` field, which is a JSON-encoded string array
/// such as `"[\"111\",\"222\"]"`.
pub fn parse_clob_token_ids(raw: &str) -> Result<Vec<String>> {
    serde_json::from_str::<Vec<String>>(raw)
        .with_context(|| format!("parsing clob_token_ids: {raw}"))
}

/// Gamma market-discovery filters. All fields optional.
///
/// Note: `closed` and `open` both set the same upstream `closed` filter; if both
/// are Some, `open` is applied last and wins.
#[derive(Debug, Clone, Default)]
pub struct DiscoverFilters {
    pub closed: Option<bool>,
    pub open: Option<bool>,
    pub min_volume: Option<f64>,
    pub min_liquidity: Option<f64>,
    pub tag_id: Option<i64>,
    pub limit: Option<u32>,
}

/// Discover token ids via `gamma.markets().list()`, applying the given filters.
///
/// Each returned market's `clob_token_ids` (a JSON-encoded string array) is
/// parsed and flattened. Markets with a missing or unparseable field are
/// skipped with a warning to stderr.
pub async fn discover_targets(gamma: &Gamma, filters: &DiscoverFilters) -> Result<Vec<String>> {
    let mut req = gamma.markets().list();
    if let Some(closed) = filters.closed {
        req = req.closed(closed);
    }
    if let Some(open) = filters.open {
        req = req.open(open);
    }
    if let Some(v) = filters.min_volume {
        req = req.volume_num_min(v);
    }
    if let Some(l) = filters.min_liquidity {
        req = req.liquidity_num_min(l);
    }
    if let Some(tag) = filters.tag_id {
        req = req.tag_id(tag);
    }
    if let Some(limit) = filters.limit {
        req = req.limit(limit);
    }

    let markets = req.send().await.context("gamma market discovery")?;

    let mut ids = Vec::new();
    for market in markets {
        match market.clob_token_ids.as_deref() {
            Some(raw) => match parse_clob_token_ids(raw) {
                Ok(parsed) => ids.extend(parsed),
                Err(e) => eprintln!("warning: skipping market {}: {e}", market.id),
            },
            None => eprintln!("warning: skipping market {} (no clob_token_ids)", market.id),
        }
    }
    Ok(ids)
}

/// Reject token ids that aren't safe to use as a bare filename (they're
/// interpolated directly into an output path).
pub fn validate_token_id(id: &str) -> Result<()> {
    if id.is_empty()
        || id.contains('/')
        || id.contains('\\')
        || id.contains("..")
        || id.contains('\0')
    {
        return Err(color_eyre::eyre::eyre!(
            "invalid token id {id:?}: must be non-empty and contain no path separators or '..'"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn validate_token_id_rejects_path_traversal() {
        assert!(validate_token_id("12345").is_ok());
        assert!(validate_token_id("").is_err());
        assert!(validate_token_id("../etc/passwd").is_err());
        assert!(validate_token_id("a/b").is_err());
        assert!(validate_token_id("a\\b").is_err());
    }

    #[test]
    fn dedupe_preserves_first_seen_order() {
        let targets = dedupe_targets(["b", "a", "b", "c", "a"].into_iter().map(String::from));
        let ids: Vec<&str> = targets.iter().map(|t| t.token_id.as_str()).collect();
        assert_eq!(ids, ["b", "a", "c"]);
    }

    #[test]
    fn read_ids_file_skips_blanks_and_comments() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        writeln!(f, "0xabc").unwrap();
        writeln!(f, "  # a comment").unwrap();
        writeln!(f).unwrap();
        writeln!(f, "  0xdef  ").unwrap();
        let ids = read_ids_file(f.path()).unwrap();
        assert_eq!(ids, ["0xabc", "0xdef"]);
    }

    #[test]
    fn read_ids_file_errors_on_missing_file() {
        let err =
            read_ids_file(std::path::Path::new("/nonexistent/definitely-missing.txt")).unwrap_err();
        assert!(err.to_string().contains("definitely-missing.txt"));
    }

    #[test]
    fn parse_clob_token_ids_parses_json_array() {
        let ids = parse_clob_token_ids(r#"["111","222"]"#).unwrap();
        assert_eq!(ids, ["111", "222"]);
    }

    #[test]
    fn parse_clob_token_ids_rejects_malformed() {
        assert!(parse_clob_token_ids("not json").is_err());
    }
}
