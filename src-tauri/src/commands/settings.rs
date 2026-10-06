//! Settings, audit, device history, app info (spec §36–37, §54).

use tauri::State;

use crate::storage::audit::AuditEntry;
use crate::storage::history::HistoryEntry;
use crate::storage::settings::Settings;

use super::AppState;

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<Settings, super::AppError> {
    let st = state.inner().clone();
    Ok(st.settings.load())
}

#[tauri::command]
pub fn save_settings(
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<Settings, super::AppError> {
    let st = state.inner().clone();
    st.settings.save(&settings)?;
    st.tools.clear_cache();
    st.audit.set_enabled(settings.audit_enabled);
    st.log.info(&format!(
        "settings saved: theme={} lang={} perf={}",
        settings.theme, settings.language, settings.performance_mode
    ));
    Ok(st.settings.load())
}

#[tauri::command]
pub fn get_audit(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> Result<Vec<AuditEntry>, super::AppError> {
    let st = state.inner().clone();
    Ok(st.audit.read(limit.map(|n| n as usize)))
}

#[tauri::command]
pub fn clear_audit(state: State<'_, AppState>) -> Result<(), super::AppError> {
    let st = state.inner().clone();
    st.audit.clear();
    Ok(())
}

#[tauri::command]
pub fn get_device_history(
    state: State<'_, AppState>,
) -> Result<Vec<HistoryEntry>, super::AppError> {
    let st = state.inner().clone();
    Ok(st.history.read())
}

#[tauri::command]
pub fn clear_device_history(state: State<'_, AppState>) -> Result<(), super::AppError> {
    let st = state.inner().clone();
    st.history.clear();
    Ok(())
}

#[tauri::command]
pub fn update_device_metadata(
    state: State<'_, AppState>,
    serial: String,
    alias: Option<String>,
    tags: Vec<String>,
    favorite: bool,
) -> Result<Vec<HistoryEntry>, super::AppError> {
    let st = state.inner().clone();
    st.history.update_metadata(&serial, alias, tags, favorite)?;
    Ok(st.history.read())
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub tauri_version: String,
    pub platform: String,
}

#[tauri::command]
pub fn get_app_info() -> AppInfo {
    AppInfo {
        name: "Zittodb".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        tauri_version: tauri::VERSION.into(),
        platform: std::env::consts::OS.into(),
    }
}

/// Local paths used by the app (shown in Settings → Sobre).
#[tauri::command]
pub fn get_app_paths(state: State<'_, AppState>) -> Result<AppPaths, super::AppError> {
    let st = state.inner().clone();
    Ok(AppPaths {
        config_dir: st.dirs.config.to_string_lossy().to_string(),
        data_dir: st.dirs.data.to_string_lossy().to_string(),
        log_file: st
            .dirs
            .log
            .join("zittodb.log")
            .to_string_lossy()
            .to_string(),
        audit_file: st.audit.path().to_string_lossy().to_string(),
    })
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppPaths {
    pub config_dir: String,
    pub data_dir: String,
    pub log_file: String,
    pub audit_file: String,
}
