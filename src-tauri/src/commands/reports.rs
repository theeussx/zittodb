//! Diagnostic report collection and secure export.

use std::io::Write;
use std::path::Path;

use serde::Serialize;
use tauri::State;

use crate::adb::ToolStatus;
use crate::devices::{self, ConnectionKind, DeviceState};
use crate::error::{AppError, ErrorCode};
use crate::reports::{
    next_available_filename, DiagnosticReport, ReportError, ReportPrivacy, ReportSection,
    ReportTool, SectionStatus, REPORT_SCHEMA_VERSION,
};
use crate::storage::ensure_private_file;

use super::{blocking, join, media, AppState};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportExport {
    pub report: DiagnosticReport,
    pub json_path: String,
    pub markdown_path: String,
}

#[tauri::command]
pub async fn collect_diagnostic_report(
    state: State<'_, AppState>,
    serial: String,
    privacy: ReportPrivacy,
) -> Result<DiagnosticReport, AppError> {
    let st = state.inner().clone();
    crate::security::validate_serial(&serial)?;
    let h = blocking(move || collect_report(&st, &serial, privacy));
    join(h).await
}

#[tauri::command]
pub fn export_diagnostic_report(
    state: State<'_, AppState>,
    report: DiagnosticReport,
) -> Result<ReportExport, AppError> {
    if report.schema_version != REPORT_SCHEMA_VERSION {
        return Err(AppError::new(
            ErrorCode::InvalidArgument,
            format!(
                "unsupported diagnostic report schema: {}",
                report.schema_version
            ),
        ));
    }

    let st = state.inner().clone();
    let dir = media::download_dir(&st);
    std::fs::create_dir_all(&dir).map_err(|e| {
        AppError::new(
            ErrorCode::InvalidPath,
            format!("cannot create report directory {}: {e}", dir.display()),
        )
    })?;
    if !dir.is_dir() {
        return Err(AppError::new(
            ErrorCode::InvalidPath,
            format!("report destination is not a directory: {}", dir.display()),
        ));
    }

    let stem = format!(
        "zittodb-diagnostic-{}",
        chrono::Local::now().format("%Y-%m-%d-%H%M%S")
    );
    let json_path = next_available_filename(&dir, &stem, "json", Path::exists);
    let markdown_path = next_available_filename(&dir, &stem, "md", Path::exists);
    let json = crate::reports::to_json(&report)
        .map_err(|e| AppError::new(ErrorCode::Unexpected, format!("serialize JSON report: {e}")))?;
    let markdown = crate::reports::to_markdown(&report);

    write_new_file(&json_path, json.as_bytes())?;
    if let Err(error) = write_new_file(&markdown_path, markdown.as_bytes()) {
        let _ = std::fs::remove_file(&json_path);
        return Err(error);
    }

    Ok(ReportExport {
        report,
        json_path: json_path.to_string_lossy().to_string(),
        markdown_path: markdown_path.to_string_lossy().to_string(),
    })
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| {
            let code = if e.kind() == std::io::ErrorKind::PermissionDenied {
                ErrorCode::OperationRejected
            } else if e.kind() == std::io::ErrorKind::AlreadyExists {
                ErrorCode::FileExists
            } else {
                ErrorCode::ProcessFailed
            };
            AppError::new(code, format!("cannot write {}: {e}", path.display()))
        })?;
    file.write_all(bytes)
        .and_then(|_| file.flush())
        .map_err(|e| {
            AppError::new(
                ErrorCode::ProcessFailed,
                format!("write {}: {e}", path.display()),
            )
        })?;
    ensure_private_file(path);
    Ok(())
}

fn collect_report(
    st: &AppState,
    serial: &str,
    privacy: ReportPrivacy,
) -> Result<DiagnosticReport, AppError> {
    let settings = st.settings.load();
    let statuses = st.tools.detect(
        settings.adb_path.as_deref(),
        settings.scrcpy_path.as_deref(),
        settings.fastboot_path.as_deref(),
    );
    let mut report = DiagnosticReport::new(
        chrono::Utc::now().to_rfc3339(),
        env!("CARGO_PKG_VERSION"),
        Some(serial.to_string()),
        None,
    );
    report.tools = statuses.into_iter().map(tool_from_status).collect();

    let adb = match st.adb() {
        Ok(adb) => adb,
        Err(error) => {
            add_unavailable_sections(&mut report, &error);
            report
                .limitations
                .push(format!("ADB indisponível: {}", error.code.as_str()));
            report.apply_privacy(privacy);
            return Ok(report);
        }
    };

    match revalidate_device(&adb, serial) {
        Ok(device) => {
            report.device.state = Some(device_state_name(device.state).into());
            report.sections.insert(
                "connection".into(),
                ReportSection::ok(serde_json::json!({
                    "state": device_state_name(device.state),
                    "connection": connection_name(device.connection),
                    "isEmulator": device.is_emulator
                })),
            );
        }
        Err(error) => {
            report.device.state = Some("unavailable".into());
            report
                .sections
                .insert("connection".into(), section_error(&error));
            add_unavailable_sections(&mut report, &error);
            report.limitations.push(format!(
                "dispositivo não disponível: {}",
                error.code.as_str()
            ));
            report.apply_privacy(privacy);
            return Ok(report);
        }
    }

    let info = collect_after_revalidation(&adb, serial, || devices::get_device_info(&adb, serial));
    match info {
        Ok(info) => {
            report
                .sections
                .insert("properties".into(), section_value(&info));
            report.sections.insert(
                "memory".into(),
                ReportSection::ok(serde_json::json!({ "totalRamMb": info.total_ram_mb })),
            );
        }
        Err(error) => {
            report
                .sections
                .insert("properties".into(), section_error(&error));
            report
                .sections
                .insert("memory".into(), section_error(&error));
        }
    }

    let battery = collect_after_revalidation(&adb, serial, || devices::get_battery(&adb, serial));
    insert_result(&mut report, "battery", battery);

    let storage = collect_after_revalidation(&adb, serial, || devices::get_storage(&adb, serial));
    insert_result(&mut report, "storage", storage);

    let network =
        collect_after_revalidation(&adb, serial, || devices::get_network_info(&adb, serial));
    insert_result(&mut report, "network", network);

    for (name, section) in &report.sections {
        if section.status != SectionStatus::Ok {
            report
                .limitations
                .push(format!("seção {name} não foi coletada completamente"));
        }
    }
    report.apply_privacy(privacy);
    Ok(report)
}

fn collect_after_revalidation<T>(
    adb: &str,
    serial: &str,
    collect: impl FnOnce() -> Result<T, AppError>,
) -> Result<T, AppError> {
    revalidate_device(adb, serial)?;
    collect()
}

fn revalidate_device(adb: &str, serial: &str) -> Result<devices::Device, AppError> {
    let list = devices::list_devices(adb)?;
    let device = list
        .into_iter()
        .find(|device| device.serial == serial)
        .ok_or_else(|| {
            AppError::new(ErrorCode::NoDevice, "selected device is no longer present")
        })?;
    match device.state {
        DeviceState::Connected => Ok(device),
        DeviceState::Offline => Err(AppError::new(
            ErrorCode::DeviceOffline,
            "selected device is offline",
        )),
        DeviceState::Unauthorized => Err(AppError::new(
            ErrorCode::DeviceUnauthorized,
            "selected device is unauthorized",
        )),
        _ => Err(AppError::new(
            ErrorCode::NoDevice,
            "selected device is not ready for diagnostics",
        )),
    }
}

fn insert_result<T: Serialize>(
    report: &mut DiagnosticReport,
    name: &str,
    result: Result<T, AppError>,
) {
    report.sections.insert(
        name.into(),
        result.map_or_else(|error| section_error(&error), |value| section_value(&value)),
    );
}

fn section_value<T: Serialize>(value: &T) -> ReportSection {
    match serde_json::to_value(value) {
        Ok(value) => ReportSection::ok(value),
        Err(error) => ReportSection::error("SERIALIZE_FAILED", error.to_string()),
    }
}

fn section_error(error: &AppError) -> ReportSection {
    ReportSection {
        status: SectionStatus::Error,
        data: None,
        error: Some(ReportError {
            code: error.code.as_str().to_string(),
            details: error.details.clone(),
        }),
    }
}

fn add_unavailable_sections(report: &mut DiagnosticReport, error: &AppError) {
    for name in ["properties", "memory", "battery", "storage", "network"] {
        report.sections.insert(name.into(), section_error(error));
    }
}

fn tool_from_status(status: ToolStatus) -> ReportTool {
    ReportTool {
        name: status.name,
        found: status.found,
        version: status.version,
    }
}

fn device_state_name(state: DeviceState) -> &'static str {
    match state {
        DeviceState::Connected => "connected",
        DeviceState::Offline => "offline",
        DeviceState::Unauthorized => "unauthorized",
        DeviceState::Recovery => "recovery",
        DeviceState::Other => "other",
    }
}

fn connection_name(connection: ConnectionKind) -> &'static str {
    match connection {
        ConnectionKind::Usb => "usb",
        ConnectionKind::Wifi => "wifi",
        ConnectionKind::Emulator => "emulator",
        ConnectionKind::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_new_file_refuses_overwrite() {
        let dir = std::env::temp_dir().join(format!("zittodb-report-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("report.json");
        std::fs::write(&path, "original").unwrap();
        let error = write_new_file(&path, b"replacement").expect_err("must not overwrite");
        assert_eq!(error.code, ErrorCode::FileExists);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "original");
        let _ = std::fs::remove_dir_all(dir);
    }
}
