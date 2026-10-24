//! File manager commands (spec §27–28).

use std::path::Path;

use tauri::AppHandle;
use tauri::Emitter;
use tauri::State;

use crate::adb::operations::DeviceOperation;
use crate::error::{AppError, ErrorCode};
use crate::processes::{remote_file_size, TransferEvent};

use super::{
    audit_basename, audit_event, blocking, execute_op, join, transfer_audit_result, AppState,
    OpResult,
};

/// Lists a device directory (validated absolute path).
#[tauri::command]
pub async fn file_list(
    state: State<'_, AppState>,
    serial: Option<String>,
    path: String,
) -> Result<Vec<crate::adb::parse::FileEntry>, AppError> {
    let st = state.inner().clone();
    let adb = st.adb()?;
    let serial = match serial {
        Some(s) => {
            crate::security::validate_serial(&s)?;
            s
        }
        None => {
            let devices = crate::devices::list_devices(&adb)?;
            crate::devices::resolve_serial(None, &devices)?
        }
    };
    crate::security::validate_device_path(&path)?;
    let op = DeviceOperation::ListDir { path };
    let args = op.to_adb_args(Some(&serial))?;
    let h = blocking(move || {
        let out = crate::processes::run_captured(&adb, &args, crate::adb::client::TIMEOUT_DEVICE)?;
        if !out.success() {
            return Err(AppError::from_process(ErrorCode::ProcessFailed, &adb, &out));
        }
        Ok(crate::adb::parse::parse_ls(&out.text()))
    });
    join(h).await
}

#[tauri::command]
pub async fn file_mkdir(
    state: State<'_, AppState>,
    serial: Option<String>,
    path: String,
) -> Result<OpResult, AppError> {
    let st = state.inner().clone();
    execute_op(st, serial, DeviceOperation::Mkdir { path }, None).await
}

#[tauri::command]
pub async fn file_rename(
    state: State<'_, AppState>,
    serial: Option<String>,
    from: String,
    to: String,
) -> Result<OpResult, AppError> {
    let st = state.inner().clone();
    execute_op(st, serial, DeviceOperation::Rename { from, to }, None).await
}

/// rm -rf — requires the typed word APAGAR (spec §53).
#[tauri::command]
pub async fn file_delete(
    state: State<'_, AppState>,
    serial: Option<String>,
    path: String,
    confirmation: String,
) -> Result<OpResult, AppError> {
    let st = state.inner().clone();
    execute_op(
        st,
        serial,
        DeviceOperation::Delete { path },
        Some(confirmation),
    )
    .await
}

/// Starts an upload. Returns a job id; progress arrives via
/// `file-progress` events. `adb push` has no native progress: the UI shows an
/// indeterminate bar (we do not fake percentages).
#[tauri::command]
pub fn file_push(
    app: AppHandle,
    state: State<'_, AppState>,
    serial: String,
    local: String,
    remote: Option<String>,
) -> Result<String, AppError> {
    let st = state.inner().clone();
    let adb = st.adb()?;
    crate::security::validate_serial(&serial)?;
    crate::security::validate_local_path(&local)?;

    let local_path = Path::new(&local);
    let remote = match remote {
        Some(r) => r,
        None => {
            let name = local_path
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| "file".into());
            format!("/sdcard/{name}")
        }
    };

    let emitter = app.clone();
    let audit = st.audit.clone();
    let audit_serial = serial.clone();
    let audit_cmd = format!("adb -s {serial} push {} {remote}", audit_basename(&local));
    let on_event = move |e: TransferEvent| {
        if let Some(result) = transfer_audit_result(&e.status, &e.message) {
            audit_event(
                &audit,
                Some(audit_serial.clone()),
                "push",
                audit_cmd.clone(),
                result,
            );
        }
        let _ = emitter.emit("file-progress", e);
    };

    st.transfers
        .start_push(&adb, &serial, local_path, &remote, Box::new(on_event))
}

/// Starts a download. Progress is real: the local file size is polled
/// against the remote size.
#[tauri::command]
pub async fn file_pull(
    app: AppHandle,
    state: State<'_, AppState>,
    serial: String,
    remote: String,
    local_dir: Option<String>,
) -> Result<String, AppError> {
    let st = state.inner().clone();
    let adb = st.adb()?;
    crate::security::validate_serial(&serial)?;
    crate::security::validate_device_path(&remote)?;

    let dir = match local_dir {
        Some(d) => {
            crate::security::validate_local_path(&d)?;
            std::path::PathBuf::from(d)
        }
        None => super::media::download_dir(&st),
    };
    std::fs::create_dir_all(&dir)?;
    let name = remote
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("file");
    let local = dir.join(name);
    if local.exists() {
        return Err(AppError::new(
            ErrorCode::FileExists,
            format!("file exists: {}", local.display()),
        ));
    }

    let size = {
        // Cloned for the blocking thread: `adb`/`serial`/`remote` are still
        // needed below to start the transfer.
        let adb = adb.clone();
        let serial = serial.clone();
        let remote = remote.clone();
        let h = blocking(move || remote_file_size(&adb, &serial, &remote));
        join(h).await?
    };

    let emitter = app.clone();
    let audit = st.audit.clone();
    let audit_serial = serial.clone();
    let audit_cmd = format!("adb -s {serial} pull {remote} {}", audit_basename(name));
    let on_event = move |e: TransferEvent| {
        if let Some(result) = transfer_audit_result(&e.status, &e.message) {
            audit_event(
                &audit,
                Some(audit_serial.clone()),
                "pull",
                audit_cmd.clone(),
                result,
            );
        }
        let _ = emitter.emit("file-progress", e);
    };

    let id = st
        .transfers
        .start_pull(&adb, &serial, &remote, &local, size, Box::new(on_event))?;
    Ok(id)
}

/// Cancels a transfer (kills the adb child, spec §45).
#[tauri::command]
pub fn transfer_cancel(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let st = state.inner().clone();
    st.transfers.cancel(&id)
}
