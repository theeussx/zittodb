//! Local audit log of interface-initiated operations (spec §25, §54).
//!
//! Rules:
//! - only operations built by the allowlist are logged (their argv is not
//!   sensitive by construction);
//! - shell session input is NEVER logged (it may contain secrets);
//! - file contents and tool output are NEVER logged;
//! - the user can clear the history at any time.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use super::{ensure_private_dir, ensure_private_file};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntry {
    pub ts: i64,
    pub device: Option<String>,
    pub action: String,
    pub command: String,
    pub result: String, // "ok" | "error:<CODE>"
    pub undo: Option<UndoRef>,
}

/// Pointer to how the operation could be reverted (spec §23).
/// `None` / absent = no guaranteed reversal.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoRef {
    pub action: String, // "enable_package" | "reinstall_existing"
    pub package: String,
}

const MAX_LINES: usize = 5000;

pub struct AuditLog {
    path: PathBuf,
    enabled: Mutex<bool>,
}

impl AuditLog {
    pub fn open(dir: &std::path::Path, enabled: bool) -> AuditLog {
        ensure_private_dir(dir);
        let path = dir.join("audit.jsonl");
        ensure_private_file(&path);
        AuditLog {
            path,
            enabled: Mutex::new(enabled),
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        *self.enabled.lock().unwrap() = enabled;
    }

    pub fn append(&self, entry: AuditEntry) {
        if !*self.enabled.lock().unwrap() {
            return;
        }
        let Ok(json) = serde_json::to_string(&entry) else {
            return;
        };
        let Ok(mut file) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        else {
            return;
        };
        ensure_private_file(&self.path);
        let _ = writeln!(file, "{json}");
        self.trim();
    }

    /// Keeps the file bounded (spec §29 principle, applied to the audit log).
    fn trim(&self) {
        let Ok(content) = fs::read_to_string(&self.path) else {
            return;
        };
        let lines: Vec<&str> = content.lines().collect();
        if lines.len() <= MAX_LINES {
            return;
        }
        let keep = &lines[lines.len() - MAX_LINES..];
        let joined: String = keep.iter().map(|l| format!("{l}\n")).collect();
        fs::write(&self.path, joined).ok();
        ensure_private_file(&self.path);
    }

    /// Most recent first.
    pub fn read(&self, limit: Option<usize>) -> Vec<AuditEntry> {
        let Ok(content) = fs::read_to_string(&self.path) else {
            return Vec::new();
        };
        let mut entries: Vec<AuditEntry> = content
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        entries.reverse();
        if let Some(n) = limit {
            entries.truncate(n);
        }
        entries
    }

    pub fn clear(&self) {
        fs::write(&self.path, "").ok();
        ensure_private_file(&self.path);
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(i: i64) -> AuditEntry {
        AuditEntry {
            ts: i,
            device: Some("S1".into()),
            action: "disable_package".into(),
            command: "adb -s S1 shell pm disable-user --user 0 com.x".into(),
            result: "ok".into(),
            undo: Some(UndoRef {
                action: "enable_package".into(),
                package: "com.x".into(),
            }),
        }
    }

    #[test]
    fn append_and_read_reversed() {
        let dir = std::env::temp_dir().join(format!("zittodb-audit-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let log = AuditLog::open(&dir, true);
        log.append(entry(1));
        log.append(entry(2));
        let v = log.read(None);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].ts, 2);
        assert_eq!(v[1].ts, 1);
        log.clear();
        assert!(log.read(None).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn disabled_log_writes_nothing() {
        let dir = std::env::temp_dir().join(format!("zittodb-audit2-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let log = AuditLog::open(&dir, false);
        log.append(entry(1));
        assert!(!log.path().exists() || log.read(None).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_lines_are_skipped() {
        let dir = std::env::temp_dir().join(format!("zittodb-audit3-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let log = AuditLog::open(&dir, true);
        log.append(entry(1));
        fs::OpenOptions::new()
            .append(true)
            .open(log.path())
            .map(|mut f| {
                let _ = writeln!(f, "not json");
            })
            .ok();
        log.append(entry(3));
        let v = log.read(None);
        assert_eq!(v.len(), 2);
        let _ = fs::remove_dir_all(&dir);
    }
}
