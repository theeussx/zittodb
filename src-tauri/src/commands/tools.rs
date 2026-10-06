//! Tool discovery commands (spec §7–9).

use tauri::State;

use crate::adb::{ToolKind, ToolStatus};
use crate::adb::client::{AdbClient, TIMEOUT_DEVICE};
use crate::error::{AppError, ErrorCode};

use super::AppState;

/// Detects adb/scrcpy/fastboot (manual override → PATH → SDK dirs).
#[tauri::command]
pub fn detect_tools(state: State<'_, AppState>) -> Result<Vec<ToolStatus>, AppError> {
    let st = state.inner().clone();
    let s = st.settings.load();
    let statuses = st.tools.detect(
        s.adb_path.as_deref(),
        s.scrcpy_path.as_deref(),
        s.fastboot_path.as_deref(),
    );
    st.log.info(&format!(
        "detect: adb={} scrcpy={} fastboot={}",
        statuses
            .iter()
            .find(|t| t.name == "adb")
            .map(|t| t.found)
            .unwrap_or(false),
        statuses
            .iter()
            .find(|t| t.name == "scrcpy")
            .map(|t| t.found)
            .unwrap_or(false),
        statuses
            .iter()
            .find(|t| t.name == "fastboot")
            .map(|t| t.found)
            .unwrap_or(false),
    ));
    Ok(statuses)
}

/// Validates and persists a manual tool path, then re-detects.
#[tauri::command]
pub fn set_tool_path(
    state: State<'_, AppState>,
    tool: String,
    path: Option<String>,
) -> Result<Vec<ToolStatus>, AppError> {
    let st = state.inner().clone();
    let kind = match tool.as_str() {
        "adb" => ToolKind::Adb,
        "scrcpy" => ToolKind::Scrcpy,
        "fastboot" => ToolKind::Fastboot,
        _ => {
            return Err(AppError::new(
                crate::error::ErrorCode::InvalidArgument,
                format!("unknown tool: {tool}"),
            ))
        }
    };

    let mut s = st.settings.load();
    st.tools.set_manual(kind, path.as_deref())?;
    match kind {
        ToolKind::Adb => s.adb_path = path,
        ToolKind::Scrcpy => s.scrcpy_path = path,
        ToolKind::Fastboot => s.fastboot_path = path,
    }
    st.settings.save(&s)?;
    st.tools.clear_cache();
    Ok(detect_tools_inner(&st))
}

#[tauri::command]
pub fn check_tool_path(state: State<'_, AppState>, path: String) -> Result<bool, AppError> {
    let _ = state;
    Ok(crate::adb::is_executable_file_check(&path))
}

#[tauri::command]
pub async fn adb_server_version(state: State<'_, AppState>) -> Result<String, AppError> {
    let st = state.inner().clone();
    let adb = st.adb()?;
    super::join(super::blocking(move || AdbClient::new(&adb).run_ok(vec!["version".into()], TIMEOUT_DEVICE))).await
}

#[tauri::command]
pub async fn adb_server_start(state: State<'_, AppState>) -> Result<String, AppError> {
    let st = state.inner().clone();
    let adb = st.adb()?;
    super::join(super::blocking(move || AdbClient::new(&adb).run_ok(vec!["start-server".into()], TIMEOUT_DEVICE))).await
}

#[tauri::command]
pub async fn adb_server_restart(
    state: State<'_, AppState>,
    confirmation: String,
) -> Result<String, AppError> {
    if confirmation != "REINICIAR_ADB" {
        return Err(AppError::new(ErrorCode::ConfirmationRequired, "type REINICIAR_ADB to confirm"));
    }
    let st = state.inner().clone();
    let adb = st.adb()?;
    let result = super::join(super::blocking(move || {
        let client = AdbClient::new(&adb);
        let _ = client.run_ok(vec!["kill-server".into()], TIMEOUT_DEVICE)?;
        client.run_ok(vec!["start-server".into()], TIMEOUT_DEVICE)
    })).await?;
    st.log.warning("ADB server restarted by explicit user action");
    Ok(result)
}

fn detect_tools_inner(st: &AppState) -> Vec<ToolStatus> {
    let s = st.settings.load();
    st.tools.detect(
        s.adb_path.as_deref(),
        s.scrcpy_path.as_deref(),
        s.fastboot_path.as_deref(),
    )
}
