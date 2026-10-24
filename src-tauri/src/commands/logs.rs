//! Logcat commands (spec §29–30).
//!
//! The backend streams lines; the frontend keeps a bounded window
//! (settings.logcat_max_lines, default 5 000) so memory stays flat.

use std::sync::Arc;

use tauri::AppHandle;
use tauri::Emitter;
use tauri::State;

use crate::error::AppError;
use crate::processes::validate_logcat_spec;

use super::AppState;

/// Starts a logcat stream. Emits `logcat-line`: `{ "session", "line" }`.
///
/// `spec` is an optional validated tag filter (e.g. "Camera" or "Camera:E").
#[tauri::command]
pub fn logcat_start(
    app: AppHandle,
    state: State<'_, AppState>,
    serial: String,
    spec: Option<String>,
) -> Result<String, AppError> {
    let st = state.inner().clone();
    let adb = st.adb()?;
    let id = st.logcat_mgr.next_id();

    if let Some(s) = &spec {
        if !validate_logcat_spec(s)? {
            return Err(crate::error::AppError::new(
                crate::error::ErrorCode::InvalidArgument,
                format!("invalid logcat spec: {s}"),
            ));
        }
    }

    let emitter = app.clone();
    let id_for_event = id.clone();
    let on_line = move |line: String| {
        let _ = emitter.emit(
            "logcat-line",
            serde_json::json!({ "session": id_for_event, "line": line }),
        );
    };

    let spec_ref = spec.as_deref();
    st.logcat_mgr
        .open(&id, &adb, &serial, spec_ref, Arc::new(on_line))?;
    st.log
        .info(&format!("logcat session {id} started for {serial}"));
    Ok(id)
}

#[tauri::command]
pub fn logcat_stop(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let st = state.inner().clone();
    st.logcat_mgr.stop(&id)
}

#[tauri::command]
pub fn logcat_list(state: State<'_, AppState>) -> Result<Vec<String>, AppError> {
    let st = state.inner().clone();
    Ok(st.logcat_mgr.list())
}

/// Extensions accepted for exported text (logs, reports, session dumps).
/// Restricting them prevents the webview from creating files such as
/// `~/.config/autostart/x.desktop` or shell rc files through this command.
const EXPORT_EXTENSIONS: &[&str] = &["log", "txt", "md", "json", "html"];
/// Upper bound for one exported text file.
pub const MAX_EXPORT_BYTES: usize = 64 * 1024 * 1024;

/// Writes `content` to a NEW local file. Never overwrites, never creates
/// parent directories, never follows a pre-existing symlink at the target
/// (`create_new` uses O_EXCL).
pub fn write_new_text_file(path: &str, content: &str) -> Result<std::path::PathBuf, AppError> {
    use std::io::Write;

    crate::security::validate_local_path(path)?;
    let p = std::path::Path::new(path);
    if !p.is_absolute() {
        return Err(AppError::new(
            crate::error::ErrorCode::InvalidPath,
            "export path must be absolute",
        ));
    }
    let ext_ok = p
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| EXPORT_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false);
    if !ext_ok {
        return Err(AppError::new(
            crate::error::ErrorCode::InvalidArgument,
            format!(
                "extension not allowed; use one of: {}",
                EXPORT_EXTENSIONS.join(", ")
            ),
        ));
    }
    if content.len() > MAX_EXPORT_BYTES {
        return Err(AppError::new(
            crate::error::ErrorCode::InvalidArgument,
            format!("content too large ({} bytes)", content.len()),
        ));
    }
    match p.parent() {
        Some(dir) if dir.is_dir() => {}
        _ => {
            return Err(AppError::new(
                crate::error::ErrorCode::InvalidPath,
                "destination folder does not exist",
            ))
        }
    }
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(p)
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                AppError::new(
                    crate::error::ErrorCode::FileExists,
                    format!("file exists: {}", p.display()),
                )
            } else {
                AppError::from(e)
            }
        })?;
    f.write_all(content.as_bytes())?;
    Ok(p.to_path_buf())
}

/// Saves the user-selected log text to a NEW local file (explicit user action).
#[tauri::command]
pub fn save_log_file(path: String, content: String) -> Result<(), AppError> {
    write_new_text_file(&path, &content).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("zittodb-logs-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn writes_new_file_once() {
        let d = tmp("once");
        let f = d.join("a.log");
        write_new_text_file(f.to_str().unwrap(), "hello").unwrap();
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "hello");
        let e = write_new_text_file(f.to_str().unwrap(), "other").unwrap_err();
        assert_eq!(e.code, ErrorCode::FileExists);
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "hello");
    }

    #[test]
    fn rejects_relative_bad_extension_and_missing_dir() {
        let d = tmp("rej");
        assert_eq!(
            write_new_text_file("a.log", "x").unwrap_err().code,
            ErrorCode::InvalidPath
        );
        let e = write_new_text_file(d.join("x.desktop").to_str().unwrap(), "x").unwrap_err();
        assert_eq!(e.code, ErrorCode::InvalidArgument);
        let e = write_new_text_file(d.join("no/such/x.log").to_str().unwrap(), "x").unwrap_err();
        assert_eq!(e.code, ErrorCode::InvalidPath);
    }

    #[cfg(unix)]
    #[test]
    fn does_not_follow_symlink_target() {
        let d = tmp("sym");
        let victim = d.join("victim.txt");
        std::fs::write(&victim, "keep").unwrap();
        let link = d.join("link.log");
        std::os::unix::fs::symlink(&victim, &link).unwrap();
        assert!(write_new_text_file(link.to_str().unwrap(), "pwn").is_err());
        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "keep");
    }
}
