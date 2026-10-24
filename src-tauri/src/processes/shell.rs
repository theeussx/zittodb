//! Interactive ADB shell sessions (spec §24–25).
//!
//! Security model: the host process is always exactly
//! `adb -s <validated-serial> shell`. User input is written to the child's
//! stdin — it is never concatenated into the host command line, so the
//! frontend cannot inject arguments into adb itself.
//!
//! Multiple sessions are supported; every session is killed on close and on
//! app exit (no orphans).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::error::{AppError, ErrorCode};
use crate::security::validate_serial;

use super::{spawn_streamed, StreamHandle};

/// Maximum size of one command line sent to a shell session.
pub const MAX_SHELL_LINE: usize = 10_000;

/// A shell line must be non-empty text without line breaks or control bytes
/// (tabs are allowed).
pub fn validate_shell_line(line: &str) -> Result<(), AppError> {
    let bad = line.trim().is_empty()
        || line.len() > MAX_SHELL_LINE
        || line.chars().any(|c| c != '\t' && (c.is_control()));
    if bad {
        return Err(AppError::new(
            ErrorCode::InvalidArgument,
            "shell input must be a single non-empty line without control characters",
        ));
    }
    Ok(())
}

pub struct ShellSession {
    pub id: String,
    pub serial: String,
    pub handle: Arc<StreamHandle>,
}

pub struct ShellManager {
    sessions: Mutex<HashMap<String, Arc<StreamHandle>>>,
    serials: Mutex<HashMap<String, String>>,
    counter: Mutex<u32>,
}

impl ShellManager {
    pub fn new() -> ShellManager {
        ShellManager {
            sessions: Mutex::new(HashMap::new()),
            serials: Mutex::new(HashMap::new()),
            counter: Mutex::new(0),
        }
    }

    pub fn next_id(&self) -> String {
        let mut c = self.counter.lock().unwrap();
        *c += 1;
        format!("sh{}", *c)
    }

    /// Opens `adb -s <serial> shell` as a long-lived process.
    pub fn open(
        &self,
        id: &str,
        adb: &str,
        serial: &str,
        on_line: Arc<dyn Fn(String) + Send + Sync + 'static>,
    ) -> Result<Arc<StreamHandle>, AppError> {
        validate_serial(serial)?;
        if self.sessions.lock().unwrap().contains_key(id) {
            return Err(AppError::new(
                ErrorCode::AlreadyRunning,
                format!("session {id} exists"),
            ));
        }

        let args = vec!["-s".to_string(), serial.to_string(), "shell".to_string()];
        let handle = spawn_streamed(adb, &args, &format!("adb -s {serial} shell"), true, on_line)?;

        self.sessions
            .lock()
            .unwrap()
            .insert(id.to_string(), handle.clone());
        self.serials
            .lock()
            .unwrap()
            .insert(id.to_string(), serial.to_string());
        Ok(handle)
    }

    pub fn write(&self, id: &str, data: &str) -> Result<(), AppError> {
        let guard = self.sessions.lock().unwrap();
        let Some(handle) = guard.get(id) else {
            return Err(AppError::new(
                ErrorCode::FileNotFound,
                format!("no shell session '{id}'"),
            ));
        };
        handle.write(data)
    }

    /// Sends ONE command line to the session. The backend owns the line
    /// terminator (the frontend never sends `\n`), which keeps the desktop
    /// backend consistent with the browser dev server and prevents a single
    /// call from smuggling several commands through embedded newlines.
    pub fn write_line(&self, id: &str, line: &str) -> Result<(), AppError> {
        validate_shell_line(line)?;
        self.write(id, &format!("{line}\n"))
    }

    /// Serial a session was opened for.
    pub fn serial_of(&self, id: &str) -> Option<String> {
        self.serials.lock().unwrap().get(id).cloned()
    }

    pub fn close(&self, id: &str) -> Result<(), AppError> {
        let handle = self.sessions.lock().unwrap().remove(id).ok_or_else(|| {
            AppError::new(ErrorCode::FileNotFound, format!("no shell session '{id}'"))
        })?;
        self.serials.lock().unwrap().remove(id);
        handle.stop();
        Ok(())
    }

    pub fn list(&self) -> Vec<ShellSession> {
        let sessions = self.sessions.lock().unwrap();
        let serials = self.serials.lock().unwrap();
        sessions
            .iter()
            .filter_map(|(id, handle)| {
                Some(ShellSession {
                    id: id.clone(),
                    serial: serials.get(id).cloned().unwrap_or_default(),
                    handle: handle.clone(),
                })
            })
            .collect()
    }

    pub fn stop_all(&self) {
        for (_, handle) in self.sessions.lock().unwrap().drain() {
            handle.stop();
        }
        self.serials.lock().unwrap().clear();
    }
}

impl Drop for ShellManager {
    fn drop(&mut self) {
        self.stop_all();
    }
}

impl Default for ShellManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_serial() {
        let m = ShellManager::new();
        let on_line = |_l: String| {};
        let err = m
            .open("sh1", "/bin/echo", "bad;rm -rf /", Arc::new(on_line))
            .expect_err("invalid serial must be rejected");
        assert_eq!(err.code, ErrorCode::InvalidSerial);
    }

    #[test]
    fn rejects_shell_metachars_in_serial() {
        let m = ShellManager::new();
        for bad in ["a b", "a|b", "a&b", "$(x)", "`x`", "a\nb", "a/b"] {
            let on_line = |_l: String| {};
            let err = m
                .open("shX", "/bin/echo", bad, Arc::new(on_line))
                .expect_err("must reject");
            assert_eq!(
                err.code,
                ErrorCode::InvalidSerial,
                "serial {bad:?} accepted?"
            );
        }
    }

    #[test]
    fn closes_sessions() {
        let m = ShellManager::new();
        let on_line = |_l: String| {};
        // A stdin-reading script stands in for `adb shell` (same shape: writes
        // go to stdin). `/bin/cat` cannot be used directly: adb's argv
        // (`-s SERIAL shell`) makes cat treat them as file names and exit.
        let dir = std::env::temp_dir().join(format!("zittodb-shell-sink-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sink = dir.join("fake-adb");
        std::fs::write(&sink, "#!/bin/sh\nexec cat > /dev/null\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&sink, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let h = m
            .open(
                "sh1",
                sink.to_str().unwrap(),
                "192.168.1.50:5555",
                Arc::new(on_line),
            )
            .unwrap();
        m.write("sh1", "hi\n").unwrap();
        assert_eq!(m.list().len(), 1);
        m.close("sh1").unwrap();
        assert_eq!(m.list().len(), 0);
        let err = m.write("sh1", "x").expect_err("gone");
        assert_eq!(err.code, ErrorCode::FileNotFound);
        drop(h);
    }

    #[test]
    fn write_line_appends_exactly_one_newline() {
        let dir = std::env::temp_dir().join(format!("zittodb-shell-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("out.txt");
        let _ = std::fs::remove_file(&out);
        let m = ShellManager::new();
        let on_line = |_l: String| {};
        // `tee` stands in for `adb shell`: it only emits what it receives.
        let tee = dir.join("fake-adb");
        std::fs::write(&tee, format!("#!/bin/sh\nexec cat > '{}'\n", out.display())).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tee, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        m.open("sh1", tee.to_str().unwrap(), "FAKE1", Arc::new(on_line))
            .unwrap();
        m.write_line("sh1", "getprop ro.product.model").unwrap();
        // Wait until the child has consumed the input BEFORE closing (close kills it).
        for _ in 0..100 {
            if std::fs::metadata(&out)
                .map(|m| m.len() > 0)
                .unwrap_or(false)
            {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        m.close("sh1").unwrap();
        assert_eq!(
            std::fs::read_to_string(&out).unwrap(),
            "getprop ro.product.model\n"
        );
    }

    #[test]
    fn write_line_rejects_multiline_and_control_input() {
        for bad in ["", "   ", "a\nb", "a\rb", "a\0b", "ls\x1b[2J"] {
            assert_eq!(
                validate_shell_line(bad).unwrap_err().code,
                ErrorCode::InvalidArgument,
                "{bad:?}"
            );
        }
        assert!(validate_shell_line(&"x".repeat(MAX_SHELL_LINE + 1)).is_err());
        assert!(validate_shell_line("echo\thello").is_ok());
    }
}
