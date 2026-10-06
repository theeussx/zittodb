//! Minimal local diagnostic logger.
//!
//! Design rules (spec §60):
//! - INFO/WARNING/ERROR/DEBUG levels only;
//! - never log passwords, tokens, file contents, or command output that may
//!   contain user data — `sanitize` scrubs known sensitive patterns;
//! - plain append-only file, no rotation beyond a simple size cap.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::storage::{ensure_private_dir, ensure_private_file};

const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024; // 2 MB, then start over

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Debug,
    Info,
    Warning,
    Error,
}

impl Level {
    pub fn as_str(&self) -> &'static str {
        match self {
            Level::Debug => "DEBUG",
            Level::Info => "INFO",
            Level::Warning => "WARNING",
            Level::Error => "ERROR",
        }
    }
}

pub struct AppLog {
    file: Mutex<Option<File>>,
}

impl AppLog {
    pub fn open(log_dir: &Path) -> std::io::Result<AppLog> {
        ensure_private_dir(log_dir);
        let path = log_dir.join("zittodb.log");
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        ensure_private_file(&path);
        Ok(AppLog {
            file: Mutex::new(Some(file)),
        })
    }

    /// In-memory-only logger for tests / headless usage.
    pub fn null() -> AppLog {
        AppLog {
            file: Mutex::new(None),
        }
    }

    pub fn log(&self, level: Level, msg: &str) {
        let sanitized = sanitize(msg);
        let mut guard = match self.file.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let Some(file) = guard.as_mut() else {
            return;
        };

        // Simple size cap: if the file grew too large, truncate once.
        if let Ok(md) = file.metadata() {
            if md.len() > MAX_FILE_BYTES {
                use std::io::Seek;
                use std::io::SeekFrom;
                let _ = file.seek(SeekFrom::Start(0));
                let _ = file.set_len(0);
                let _ = file.seek(SeekFrom::Start(0));
            }
        }

        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let _ = writeln!(file, "{ts} {} {sanitized}", level.as_str());
    }

    pub fn debug(&self, msg: &str) {
        self.log(Level::Debug, msg)
    }
    pub fn info(&self, msg: &str) {
        self.log(Level::Info, msg)
    }
    pub fn warning(&self, msg: &str) {
        self.log(Level::Warning, msg)
    }
    pub fn error(&self, msg: &str) {
        self.log(Level::Error, msg)
    }
}

/// Strips control characters and redacts values that look like credentials.
///
/// This is defense-in-depth: the app never *sends* secrets anywhere, but we
/// also never *write* them to disk (spec §60).
fn sanitize(msg: &str) -> String {
    let clean: String = msg
        .chars()
        .filter(|c| *c == '\t' || *c >= ' ')
        .take(4000)
        .collect();

    let words: Vec<&str> = clean.split_whitespace().collect();
    let mut out = String::with_capacity(clean.len());
    let mut i = 0;
    while i < words.len() {
        let w = words[i];
        let lower = w.to_lowercase();
        if let Some((key, _value)) = lower.split_once('=') {
            if matches!(
                key,
                "password" | "passwd" | "token" | "api_key" | "apikey" | "secret"
            ) {
                out.push_str(&format!("{key}= [redacted]"));
                i += 1;
                continue;
            }
        }
        // Also redact key: value style secrets (e.g. `password: xyz`)
        if i + 1 < words.len()
            && matches!(
                lower.trim_end_matches(':'),
                "password" | "passwd" | "token" | "api_key" | "apikey" | "secret"
            )
            && lower.ends_with(':')
        {
            out.push_str(&format!("{w} [redacted]"));
            i += 2;
            continue;
        }
        if i > 0 {
            out.push(' ');
        }
        out.push_str(w);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_passwords() {
        let s = sanitize("login failed: password=supersecret retry");
        assert!(!s.contains("supersecret"));
        assert!(s.contains("[redacted]"));
    }

    #[test]
    fn sanitizes_token_with_colon() {
        let s = sanitize("header token: abc123 done");
        assert!(!s.contains("abc123"));
    }

    #[test]
    fn strips_control_chars() {
        let s = sanitize("line1\x00\x1b[31mred\x07 end");
        assert!(!s.contains('\0'));
        assert!(!s.contains('\x1b'));
    }

    #[test]
    fn keeps_normal_text() {
        let s = sanitize("adb devices: 1 device connected");
        assert_eq!(s, "adb devices: 1 device connected");
    }
}
