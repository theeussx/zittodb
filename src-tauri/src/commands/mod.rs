//! Tauri IPC surface.
//!
//! This is the ONLY way the frontend talks to the system:
//!   frontend → typed command → validation → allowlist op → process
//! There is no `shell` plugin and no generic command runner: the frontend
//! literally cannot execute arbitrary commands (spec §42, §44, §56).

pub mod devices;
pub mod fastboot;
pub mod files;
pub mod logs;
pub mod media;
pub mod operations;
pub mod packages;
pub mod reports;
pub mod scrcpy;
pub mod settings;
pub mod shell;
pub mod tools;

use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;

use crate::adb::client::{AdbClient, TIMEOUT_DEVICE, TIMEOUT_MEDIA, TIMEOUT_PROPS};
use crate::adb::operations::DeviceOperation;
use crate::adb::ToolKind;
use crate::applog::AppLog;
use crate::devices as device_helpers;
use crate::error::{AppError, ErrorCode};
use crate::processes::{self, Captured};
use crate::storage::audit::{AuditEntry, UndoRef};
use crate::storage::settings::SettingsStore;
use crate::storage::AppDirs;

/// Everything shared between commands. Cheap to clone (Arc fields), which
/// keeps async commands `Send` without holding `State<'_` across awaits.
#[derive(Clone)]
pub struct AppState {
    pub dirs: AppDirs,
    pub log: Arc<AppLog>,
    pub settings: Arc<SettingsStore>,
    pub tools: Arc<crate::adb::ToolManager>,
    pub registry: Arc<crate::processes::ProcessRegistry>,
    pub shell_mgr: Arc<crate::processes::ShellManager>,
    pub logcat_mgr: Arc<crate::processes::LogcatManager>,
    pub transfers: Arc<crate::processes::TransferManager>,
    pub audit: Arc<crate::storage::audit::AuditLog>,
    pub history: Arc<crate::storage::history::DeviceHistory>,
    pub coordinator: Arc<device_helpers::Coordinator>,
}

impl AppState {
    pub fn adb(&self) -> Result<String, AppError> {
        let s = self.settings.load();
        self.tools.require(ToolKind::Adb, s.adb_path.as_deref())
    }
}

/// Result of an executed operation.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpResult {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
    pub code: Option<String>,
}

impl OpResult {
    fn from_captured(out: &Captured) -> OpResult {
        let ok = out.success();
        OpResult {
            ok,
            stdout: out.text(),
            stderr: out.stderr.clone(),
            code: if ok {
                None
            } else {
                Some(infer_code(out).as_str().to_string())
            },
        }
    }
}

pub fn infer_code(out: &Captured) -> ErrorCode {
    if out.timed_out {
        return ErrorCode::Timeout;
    }
    let stderr = out.stderr.to_lowercase();
    if stderr.contains("unauthorized") {
        ErrorCode::DeviceUnauthorized
    } else if stderr.contains("no devices/emulators found") || stderr.contains("no devices") {
        ErrorCode::NoDevice
    } else if stderr.contains("offline") {
        ErrorCode::DeviceOffline
    } else {
        ErrorCode::ProcessFailed
    }
}

fn timeout_for(op: &DeviceOperation) -> Duration {
    use DeviceOperation as O;
    match op {
        O::GetProps
        | O::GetKernel
        | O::GetMeminfo
        | O::GetBattery
        | O::GetNetwork
        | O::GetResolv
        | O::GetMac { .. }
        | O::GetDiskUsage { .. }
        | O::ListDir { .. }
        | O::Mkdir { .. }
        | O::Rename { .. }
        | O::Delete { .. }
        | O::ListPackages { .. }
        | O::PackagePath { .. }
        | O::EnablePackage { .. }
        | O::DisablePackage { .. }
        | O::ReinstallExisting { .. }
        | O::ClearPackageData { .. }
        | O::OpenPackage { .. }
        | O::Connect { .. }
        | O::Disconnect { .. }
        | O::Reboot { .. } => TIMEOUT_DEVICE,
        O::PackageDump { .. } => TIMEOUT_PROPS,
        O::UninstallForUser { .. } => TIMEOUT_PROPS,
        O::InstallApk { .. } => Duration::from_secs(600), // large APKs on slow Wi-Fi
        O::Screenshot => TIMEOUT_MEDIA,
        O::Push { .. } | O::Pull { .. } => Duration::from_secs(3600),
    }
}

/// Core executor for a single allowlisted operation.
///
/// `confirmation` must match `op.required_confirmation()` for destructive
/// ops. Audits every execution (spec §54).
pub async fn execute_op(
    st: AppState,
    serial: Option<String>,
    op: DeviceOperation,
    confirmation: Option<String>,
) -> Result<OpResult, AppError> {
    let adb = st.adb()?;
    st.log.info(&format!(
        "op={} serial={:?} destructive={}",
        op.name(),
        serial,
        op.is_destructive()
    ));

    // 1. Confirmation gate for destructive operations (spec §53).
    if op.is_destructive() {
        let required = op
            .required_confirmation()
            .expect("destructive ops define a confirmation word");
        match &confirmation {
            Some(c) if c == required => {}
            _ => {
                return Err(AppError::new(
                    ErrorCode::ConfirmationRequired,
                    format!("type {required} to confirm"),
                ))
            }
        }
    }

    // 2. Serial resolution (never guess with multiple devices, spec §11).
    let serial = if op.requires_serial() {
        let devices = device_helpers::list_devices(&adb)?;
        Some(device_helpers::resolve_serial(serial.as_deref(), &devices)?)
    } else {
        serial
    };

    // 3. Validate + build argv (allowlist, spec §44).
    op.validate()?;
    let args = op.to_adb_args(serial.as_deref())?;
    let described = op.describe(&adb, serial.as_deref()).unwrap_or_default();

    // 4. Serialize conflicting operations per device (spec §46).
    let device_lock = if op.is_destructive() {
        serial.as_ref().map(|s| st.coordinator.lock_for(s))
    } else {
        None
    };
    let guard = device_lock
        .as_ref()
        .map(|lock| lock.lock().unwrap_or_else(|e| e.into_inner()));

    // 5. Execute.
    let result = (|| -> Result<OpResult, AppError> {
        let out = processes::run_captured(&adb, &args, timeout_for(&op))?;
        if out.timed_out {
            return Err(AppError::new(
                ErrorCode::Timeout,
                format!("{} timed out", op.name()),
            ));
        }
        // Reboot makes the device vanish mid-exit: treat "disconnected" as ok.
        if !out.success() && matches!(op, DeviceOperation::Reboot { .. }) {
            let err = out.stderr.to_lowercase();
            if err.contains("offline") || err.contains("not found") || err.contains("closed") {
                return Ok(OpResult {
                    ok: true,
                    stdout: String::new(),
                    stderr: out.stderr.clone(),
                    code: None,
                });
            }
        }
        Ok(OpResult::from_captured(&out))
    })();

    // 6. Audit (no sensitive data: command comes from the allowlist builder).
    let (result_str, undo) = match &result {
        Ok(r) if r.ok => ("ok".to_string(), undo_ref_for(&op)),
        Ok(r) => (
            format!("error:{}", r.code.clone().unwrap_or_default()),
            None,
        ),
        Err(e) => (format!("error:{}", e.code.as_str()), None),
    };
    st.audit.append(AuditEntry {
        ts: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0),
        device: serial.clone(),
        action: op.name().to_string(),
        command: described,
        result: result_str,
        undo,
    });

    let _ = guard; // release explicitly at end of scope
    result
}

/// Reversal reference for reversible operations (spec §23).
pub fn undo_ref_for(op: &DeviceOperation) -> Option<UndoRef> {
    use DeviceOperation as O;
    match op {
        O::DisablePackage { pkg, .. } => Some(UndoRef {
            action: "enable_package".into(),
            package: pkg.clone(),
        }),
        O::UninstallForUser { pkg, .. } => Some(UndoRef {
            action: "reinstall_existing".into(),
            package: pkg.clone(),
        }),
        _ => None,
    }
}

/// Runs a read-only operation and returns stdout (for the various
/// dashboard info commands).
pub async fn run_readonly(
    st: AppState,
    serial: Option<String>,
    op: DeviceOperation,
) -> Result<String, AppError> {
    let adb = st.adb()?;
    let devices = device_helpers::list_devices(&adb)?;
    let serial = if op.requires_serial() {
        Some(device_helpers::resolve_serial(serial.as_deref(), &devices)?)
    } else {
        serial
    };
    op.validate()?;
    let args = op.to_adb_args(serial.as_deref())?;
    let client = AdbClient::new(&adb);
    let out = client.run(args, timeout_for(&op))?;
    if out.timed_out {
        return Err(AppError::new(ErrorCode::Timeout, "timeout"));
    }
    if !out.success() {
        return Err(AppError::from_process(ErrorCode::ProcessFailed, &adb, &out));
    }
    Ok(out.text())
}

/// Convenience for async commands that wrap blocking calls.
pub fn blocking<F, T>(f: F) -> tauri::async_runtime::JoinHandle<Result<T, AppError>>
where
    F: FnOnce() -> Result<T, AppError> + Send + 'static,
    T: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
}

/// Joins a blocking handle, mapping JoinError to AppError.
pub async fn join<T>(
    h: tauri::async_runtime::JoinHandle<Result<T, AppError>>,
) -> Result<T, AppError>
where
    T: Send + 'static,
{
    h.await
        .map_err(|e| AppError::new(ErrorCode::Unexpected, format!("task: {e}")))?
}
