use std::io::{self, BufWriter, Write};
use std::path::Path;

use color_eyre::eyre::{Context, Result};
use polyoxide_clob::PriceHistoryPoint;
use serde::Serialize;

use crate::commands::clob::prices::types::OutputFormat;

/// Serializes a market's price points into a byte sink. Separating
/// serialization from the filesystem keeps the format logic unit-testable.
pub trait DatasetWriter {
    fn serialize(
        &self,
        out: &mut dyn Write,
        token_id: &str,
        points: &[PriceHistoryPoint],
    ) -> io::Result<()>;
}

/// CSV with a `token_id,timestamp,price` header.
pub struct CsvWriter;

impl DatasetWriter for CsvWriter {
    fn serialize(
        &self,
        out: &mut dyn Write,
        token_id: &str,
        points: &[PriceHistoryPoint],
    ) -> io::Result<()> {
        writeln!(out, "token_id,timestamp,price")?;
        for p in points {
            writeln!(out, "{token_id},{},{}", p.timestamp, p.price)?;
        }
        Ok(())
    }
}

/// Newline-delimited JSON: one `{"token_id","t","p"}` object per line.
pub struct JsonlWriter;

#[derive(Serialize)]
struct JsonlRow<'a> {
    token_id: &'a str,
    t: i64,
    p: f64,
}

impl DatasetWriter for JsonlWriter {
    fn serialize(
        &self,
        out: &mut dyn Write,
        token_id: &str,
        points: &[PriceHistoryPoint],
    ) -> io::Result<()> {
        for p in points {
            let row = JsonlRow {
                token_id,
                t: p.timestamp,
                p: p.price,
            };
            serde_json::to_writer(&mut *out, &row).map_err(io::Error::from)?;
            out.write_all(b"\n")?;
        }
        Ok(())
    }
}

/// Return the writer for a format. Parquet is handled in a later task.
pub fn writer_for(format: OutputFormat) -> Box<dyn DatasetWriter> {
    match format {
        OutputFormat::Csv => Box::new(CsvWriter),
        OutputFormat::Jsonl => Box::new(JsonlWriter),
        OutputFormat::Parquet => Box::new(CsvWriter), // replaced in a later task
    }
}

/// Atomically write a dataset: serialize into a temp file in the destination
/// directory, then rename it into place. A killed run never leaves a partial
/// file that resume would wrongly skip.
pub fn atomic_write(
    writer: &dyn DatasetWriter,
    path: &Path,
    token_id: &str,
    points: &[PriceHistoryPoint],
) -> Result<()> {
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)
        .with_context(|| format!("creating output dir {}", dir.display()))?;
    let mut tmp = tempfile::NamedTempFile::new_in(dir)
        .with_context(|| format!("creating temp file in {}", dir.display()))?;
    {
        let mut buf = BufWriter::new(tmp.as_file_mut());
        writer
            .serialize(&mut buf, token_id, points)
            .with_context(|| format!("writing dataset to {}", path.display()))?;
        buf.flush()?;
    }
    tmp.as_file_mut().flush()?;
    tmp.persist(path)
        .with_context(|| format!("persisting dataset to {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use polyoxide_clob::PriceHistoryPoint;

    fn points() -> Vec<PriceHistoryPoint> {
        vec![
            PriceHistoryPoint {
                timestamp: 1700000000,
                price: 0.55,
            },
            PriceHistoryPoint {
                timestamp: 1700001000,
                price: 0.60,
            },
        ]
    }

    #[test]
    fn csv_serializes_header_and_rows() {
        let mut buf = Vec::new();
        CsvWriter.serialize(&mut buf, "0xabc", &points()).unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert_eq!(
            text,
            "token_id,timestamp,price\n0xabc,1700000000,0.55\n0xabc,1700001000,0.6\n"
        );
    }

    #[test]
    fn jsonl_serializes_one_object_per_line() {
        let mut buf = Vec::new();
        JsonlWriter.serialize(&mut buf, "0xabc", &points()).unwrap();
        let text = String::from_utf8(buf).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], r#"{"token_id":"0xabc","t":1700000000,"p":0.55}"#);
    }

    #[test]
    fn atomic_write_creates_file_with_contents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("0xabc.csv");
        atomic_write(&CsvWriter, &path, "0xabc", &points()).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("token_id,timestamp,price\n"));
    }

    #[test]
    fn atomic_write_overwrites_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("0xabc.csv");
        std::fs::write(&path, "stale contents").unwrap();
        atomic_write(&CsvWriter, &path, "0xabc", &points()).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("token_id,timestamp,price\n"));
        assert!(!text.contains("stale"));
    }
}
