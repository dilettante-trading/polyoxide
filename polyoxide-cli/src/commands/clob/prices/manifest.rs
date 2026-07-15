use std::io::Write;
use std::path::Path;

use color_eyre::eyre::{Context, Result};

use crate::commands::clob::prices::types::ManifestRecord;

/// Write the run manifest as JSONL (one record per line) to `path`.
///
/// Written once at end-of-run; resume relies on dataset-file existence, not the
/// manifest, so a crash mid-run simply omits the manifest without breaking a
/// subsequent resume.
pub fn write_manifest(path: &Path, records: &[ManifestRecord]) -> Result<()> {
    if let Some(dir) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("creating manifest dir {}", dir.display()))?;
    }
    let mut file = std::fs::File::create(path)
        .with_context(|| format!("creating manifest {}", path.display()))?;
    for rec in records {
        let line = serde_json::to_string(rec).context("serializing manifest record")?;
        writeln!(file, "{line}")
            .with_context(|| format!("writing manifest record for {}", rec.token_id))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::clob::prices::types::ManifestRecord;

    #[test]
    fn write_manifest_emits_one_json_object_per_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("manifest.jsonl");
        let records = vec![
            ManifestRecord {
                token_id: "111".into(),
                path: "111.csv".into(),
                points: 2,
                first_ts: Some(1700000000),
                last_ts: Some(1700001000),
                status: "ok".into(),
                error: None,
            },
            ManifestRecord {
                token_id: "222".into(),
                path: "222.csv".into(),
                points: 0,
                first_ts: None,
                last_ts: None,
                status: "failed".into(),
                error: Some("boom".into()),
            },
        ];
        write_manifest(&path, &records).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("\"token_id\":\"111\""));
        assert!(lines[1].contains("\"status\":\"failed\""));
    }
}
