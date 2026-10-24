//! Scanner Center commands.
//!
//! The UI names the target explicitly (`environment` or `device` + serial).
//! Exporting and comparing read the report from the local history, never from
//! data sent by the frontend.

use tauri::State;

use crate::error::{AppError, ErrorCode};
use crate::scanner::model::{diff_reports, ScanDiff, ScanReport, ScanTarget};
use crate::scanner::{self, export};
use crate::storage::scans::ScanSummary;

use super::{audit_event, blocking, join, AppState};

/// Runs a scan, stores it in the history and returns it.
#[tauri::command]
pub async fn scan_run(
    state: State<'_, AppState>,
    target: ScanTarget,
    serial: Option<String>,
    lang: String,
) -> Result<ScanReport, AppError> {
    let st = state.inner().clone();
    let (st2, serial2) = (st.clone(), serial.clone());
    let report = match target {
        ScanTarget::Environment => {
            let settings = st.settings.load();
            let tools = st.tools.detect(
                settings.adb_path.as_deref(),
                settings.scrcpy_path.as_deref(),
                settings.fastboot_path.as_deref(),
            );
            join(blocking(move || {
                let path = std::env::var_os("PATH");
                Ok(scanner::run_environment_scan(
                    &tools,
                    path.as_deref(),
                    &lang,
                ))
            }))
            .await?
        }
        ScanTarget::Device => {
            let serial = serial2.ok_or_else(|| {
                AppError::new(
                    ErrorCode::InvalidArgument,
                    "a device scan needs an explicit serial",
                )
            })?;
            let adb = st2.adb()?;
            join(blocking(move || {
                scanner::run_device_scan(&adb, &serial, &lang)
            }))
            .await?
        }
    };

    if let Err(e) = st.scans.save(&report) {
        st.log.info(&format!(
            "scan history: could not save {}: {}",
            report.scan_id, e.details
        ));
    }
    audit_event(
        &st.audit,
        report.subject.clone(),
        "scan",
        format!(
            "scan {} ({} findings)",
            report.target.as_str(),
            report.summary.total
        ),
        "ok".to_string(),
    );
    Ok(report)
}

#[tauri::command]
pub fn scan_history(state: State<'_, AppState>) -> Vec<ScanSummary> {
    state.scans.list()
}

#[tauri::command]
pub fn scan_get(state: State<'_, AppState>, id: String) -> Result<ScanReport, AppError> {
    state.scans.get(&id)
}

#[tauri::command]
pub fn scan_delete(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    state.scans.delete(&id)
}

/// Text of a stored scan as `json` or `markdown`. Saving it to disk is the
/// UI's job (`save_log_file`: new file only, allowlisted extensions).
#[tauri::command]
pub fn scan_export(
    state: State<'_, AppState>,
    id: String,
    format: String,
) -> Result<String, AppError> {
    export_text(&state.scans.get(&id)?, &format)
}

pub fn export_text(report: &ScanReport, format: &str) -> Result<String, AppError> {
    match format {
        "json" => Ok(export::to_json(report)?),
        "markdown" | "md" => Ok(export::to_markdown(report)),
        other => Err(AppError::new(
            ErrorCode::InvalidArgument,
            format!("unknown export format: {other}"),
        )),
    }
}

/// Compares two stored scans of the SAME target and subject.
#[tauri::command]
pub fn scan_compare(
    state: State<'_, AppState>,
    before: String,
    after: String,
) -> Result<ScanDiff, AppError> {
    compare(&state.scans.get(&before)?, &state.scans.get(&after)?)
}

pub fn compare(before: &ScanReport, after: &ScanReport) -> Result<ScanDiff, AppError> {
    if before.target != after.target || before.subject != after.subject {
        return Err(AppError::new(
            ErrorCode::InvalidArgument,
            "only scans of the same target and subject can be compared",
        ));
    }
    Ok(diff_reports(before, after))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::model::*;

    fn r(target: ScanTarget, subject: Option<&str>) -> ScanReport {
        ScanReport {
            schema_version: SCAN_SCHEMA_VERSION,
            scan_id: "scan-1-aaaa".into(),
            target,
            subject: subject.map(str::to_string),
            started_at: 1,
            finished_at: 2,
            lang: "en".into(),
            checks: vec![],
            findings: vec![],
            summary: Summary::default(),
            limitations: vec![],
        }
    }

    #[test]
    fn export_formats() {
        let rep = r(ScanTarget::Device, Some("S1"));
        assert!(export_text(&rep, "json").unwrap().contains("\"scanId\""));
        assert!(export_text(&rep, "markdown").unwrap().starts_with("# "));
        assert_eq!(
            export_text(&rep, "pdf").unwrap_err().code,
            ErrorCode::InvalidArgument
        );
    }

    #[test]
    fn compare_requires_same_target_and_subject() {
        let a = r(ScanTarget::Device, Some("S1"));
        assert!(compare(&a, &r(ScanTarget::Device, Some("S1"))).is_ok());
        assert!(compare(&a, &r(ScanTarget::Device, Some("S2"))).is_err());
        assert!(compare(&a, &r(ScanTarget::Environment, None)).is_err());
    }
}
