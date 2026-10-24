//! Scan history: one JSON file per scan in `<data>/scans/`.
//!
//! * ids are validated before touching the filesystem (no path traversal);
//! * writes are atomic (temp file + rename) and owner-only on Unix;
//! * only the newest [`MAX_SCANS`] reports are kept.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{AppError, ErrorCode};
use crate::scanner::model::{ScanReport, ScanTarget};

use super::{ensure_private_dir, ensure_private_file};

pub const MAX_SCANS: usize = 50;

/// `scan-<unix seconds>-<4 hex>`; nothing else is ever turned into a path.
pub fn valid_scan_id(id: &str) -> bool {
    let Some(rest) = id.strip_prefix("scan-") else {
        return false;
    };
    let Some((ts, suffix)) = rest.split_once('-') else {
        return false;
    };
    (1..=12).contains(&ts.len())
        && ts.bytes().all(|b| b.is_ascii_digit())
        && suffix.len() == 4
        && suffix
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScanSummary {
    pub scan_id: String,
    pub target: ScanTarget,
    pub subject: Option<String>,
    pub started_at: i64,
    pub lang: String,
    pub total: u32,
    pub by_severity: std::collections::BTreeMap<String, u32>,
    pub checks_not_evaluated: u32,
}

pub struct ScanStore {
    dir: PathBuf,
}

impl ScanStore {
    pub fn open(data_dir: &Path) -> ScanStore {
        let dir = data_dir.join("scans");
        ensure_private_dir(&dir);
        ScanStore { dir }
    }

    fn path_for(&self, id: &str) -> Result<PathBuf, AppError> {
        if !valid_scan_id(id) {
            return Err(AppError::new(ErrorCode::InvalidArgument, "invalid scan id"));
        }
        Ok(self.dir.join(format!("{id}.json")))
    }

    pub fn save(&self, report: &ScanReport) -> Result<(), AppError> {
        let path = self.path_for(&report.scan_id)?;
        let json = serde_json::to_string(report)
            .map_err(|e| AppError::new(ErrorCode::Unexpected, e.to_string()))?;
        let tmp = self.dir.join(format!(".{}.tmp", report.scan_id));
        std::fs::write(&tmp, json)?;
        ensure_private_file(&tmp);
        std::fs::rename(&tmp, &path)?;
        self.prune();
        Ok(())
    }

    pub fn get(&self, id: &str) -> Result<ScanReport, AppError> {
        let path = self.path_for(id)?;
        let text = std::fs::read_to_string(&path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                AppError::new(ErrorCode::FileNotFound, "unknown scan")
            } else {
                AppError::from(e)
            }
        })?;
        serde_json::from_str(&text)
            .map_err(|e| AppError::new(ErrorCode::Unexpected, format!("corrupt scan file: {e}")))
    }

    pub fn delete(&self, id: &str) -> Result<(), AppError> {
        let path = self.path_for(id)?;
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Newest first. Unreadable or corrupt files are skipped, never fatal.
    pub fn list(&self) -> Vec<ScanSummary> {
        let mut out: Vec<ScanSummary> = self
            .ids()
            .into_iter()
            .filter_map(|id| self.get(&id).ok())
            .map(|r| ScanSummary {
                scan_id: r.scan_id,
                target: r.target,
                subject: r.subject,
                started_at: r.started_at,
                lang: r.lang,
                total: r.summary.total,
                by_severity: r.summary.by_severity,
                checks_not_evaluated: r.summary.checks_not_evaluated,
            })
            .collect();
        out.sort_by(|a, b| {
            b.started_at
                .cmp(&a.started_at)
                .then_with(|| b.scan_id.cmp(&a.scan_id))
        });
        out
    }

    fn ids(&self) -> Vec<String> {
        std::fs::read_dir(&self.dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .filter_map(|e| e.file_name().to_str().map(str::to_string))
                    .filter_map(|n| n.strip_suffix(".json").map(str::to_string))
                    .filter(|id| valid_scan_id(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn prune(&self) {
        let list = self.list();
        for s in list.into_iter().skip(MAX_SCANS) {
            let _ = self.delete(&s.scan_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::model::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("zittodb-scans-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn report(id: &str, ts: i64) -> ScanReport {
        ScanReport {
            schema_version: SCAN_SCHEMA_VERSION,
            scan_id: id.into(),
            target: ScanTarget::Device,
            subject: Some("S1".into()),
            started_at: ts,
            finished_at: ts + 1,
            lang: "en-US".into(),
            checks: vec![],
            findings: vec![],
            summary: Summary::default(),
            limitations: vec![],
        }
    }

    #[test]
    fn ids_cannot_escape_the_directory() {
        for ok in ["scan-1792000000-00ab", "scan-1-ffff"] {
            assert!(valid_scan_id(ok), "{ok}");
        }
        for bad in [
            "",
            "scan-",
            "../scan-1-aaaa",
            "scan-1-aaaa/../../x",
            "scan-1-AAAA",
            "scan-1-aaa",
            "scan-1-aaaaa",
            "scan-x-aaaa",
            "scan-1792000000000000-aaaa",
            "scan-1-aaaa.json",
            "/etc/passwd",
        ] {
            assert!(!valid_scan_id(bad), "{bad}");
        }
        let s = ScanStore::open(&tmp("trav"));
        assert_eq!(
            s.get("../../etc/passwd").unwrap_err().code,
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            s.delete("../x").unwrap_err().code,
            ErrorCode::InvalidArgument
        );
    }

    #[test]
    fn save_get_list_delete_round_trip() {
        let s = ScanStore::open(&tmp("rt"));
        s.save(&report("scan-100-aaaa", 100)).unwrap();
        s.save(&report("scan-200-bbbb", 200)).unwrap();
        assert_eq!(s.get("scan-100-aaaa").unwrap().started_at, 100);
        let l = s.list();
        assert_eq!(
            l.iter().map(|x| x.scan_id.as_str()).collect::<Vec<_>>(),
            ["scan-200-bbbb", "scan-100-aaaa"]
        );
        s.delete("scan-100-aaaa").unwrap();
        s.delete("scan-100-aaaa").unwrap(); // idempotent
        assert_eq!(s.list().len(), 1);
        assert_eq!(
            s.get("scan-100-aaaa").unwrap_err().code,
            ErrorCode::FileNotFound
        );
    }

    #[test]
    fn keeps_only_the_newest_reports() {
        let s = ScanStore::open(&tmp("prune"));
        for i in 0..(MAX_SCANS as i64 + 5) {
            s.save(&report(&format!("scan-{}-0000", 1000 + i), 1000 + i))
                .unwrap();
        }
        let l = s.list();
        assert_eq!(l.len(), MAX_SCANS);
        assert_eq!(l[0].started_at, 1000 + MAX_SCANS as i64 + 4);
        assert!(s.get("scan-1000-0000").is_err(), "oldest was pruned");
    }

    #[test]
    fn corrupt_files_are_skipped_in_the_listing() {
        let d = tmp("corrupt");
        let s = ScanStore::open(&d);
        s.save(&report("scan-100-aaaa", 100)).unwrap();
        std::fs::write(d.join("scans").join("scan-300-cccc.json"), "{not json").unwrap();
        assert_eq!(s.list().len(), 1);
        assert_eq!(
            s.get("scan-300-cccc").unwrap_err().code,
            ErrorCode::Unexpected
        );
    }

    #[cfg(unix)]
    #[test]
    fn files_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let d = tmp("perm");
        let s = ScanStore::open(&d);
        s.save(&report("scan-100-aaaa", 100)).unwrap();
        let mode = std::fs::metadata(d.join("scans").join("scan-100-aaaa.json"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o077, 0, "mode {mode:o}");
    }
}
