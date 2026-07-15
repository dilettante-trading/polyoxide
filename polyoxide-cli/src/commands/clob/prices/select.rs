use std::path::Path;

use color_eyre::eyre::{Context, Result};

use crate::commands::clob::prices::types::Target;

/// Deduplicate token ids into `Target`s, preserving first-seen order.
#[allow(dead_code)] // used by the download orchestration in a later task
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
#[allow(dead_code)] // used by the download orchestration in a later task
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
#[allow(dead_code)] // used by the download orchestration in a later task
pub fn parse_clob_token_ids(raw: &str) -> Result<Vec<String>> {
    serde_json::from_str::<Vec<String>>(raw)
        .with_context(|| format!("parsing clob_token_ids: {raw}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

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
