use clap::ValueEnum;

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
