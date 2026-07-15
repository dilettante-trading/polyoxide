use std::path::{Path, PathBuf};

use clap::ValueEnum;
use serde::Serialize;

/// Output format for downloaded datasets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum OutputFormat {
    /// Comma-separated values with a header row.
    #[default]
    Csv,
    /// Newline-delimited JSON objects.
    Jsonl,
    /// Apache Parquet (requires building with the `parquet` feature).
    Parquet,
}

impl OutputFormat {
    /// File extension (without a dot) for this format.
    pub fn extension(self) -> &'static str {
        match self {
            OutputFormat::Csv => "csv",
            OutputFormat::Jsonl => "jsonl",
            OutputFormat::Parquet => "parquet",
        }
    }
}

/// A single market to download, identified by its CLOB token id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub token_id: String,
}

impl Target {
    /// Output file path for this target: `<out_dir>/<token_id>.<ext>`.
    ///
    /// Token ids are decimal integer strings, so they are filesystem-safe as-is.
    pub fn output_path(&self, out_dir: &Path, format: OutputFormat) -> PathBuf {
        out_dir.join(format!("{}.{}", self.token_id, format.extension()))
    }
}

/// One manifest row per considered market. `status` is `ok` | `empty` |
/// `failed` | `skipped`.
#[derive(Debug, Clone, Serialize)]
pub struct ManifestRecord {
    pub token_id: String,
    pub path: String,
    pub points: usize,
    pub first_ts: Option<i64>,
    pub last_ts: Option<i64>,
    pub status: String,
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn extension_matches_format() {
        assert_eq!(OutputFormat::Csv.extension(), "csv");
        assert_eq!(OutputFormat::Jsonl.extension(), "jsonl");
        assert_eq!(OutputFormat::Parquet.extension(), "parquet");
    }

    #[test]
    fn output_path_joins_dir_id_and_extension() {
        let t = Target {
            token_id: "0xabc".into(),
        };
        let p = t.output_path(Path::new("/data/out"), OutputFormat::Csv);
        assert_eq!(p, Path::new("/data/out/0xabc.csv"));
    }

    #[test]
    fn manifest_record_serializes_status_and_nulls() {
        let rec = ManifestRecord {
            token_id: "0xabc".into(),
            path: "out/0xabc.csv".into(),
            points: 0,
            first_ts: None,
            last_ts: None,
            status: "empty".into(),
            error: None,
        };
        let json = serde_json::to_string(&rec).unwrap();
        assert!(json.contains("\"status\":\"empty\""));
        assert!(json.contains("\"error\":null"));
    }
}
