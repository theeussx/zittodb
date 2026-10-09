//! Fastboot commands (spec §52–53).

use tauri::State;

use crate::adb::ToolKind;
use crate::error::{AppError, ErrorCode};
use crate::fastboot::{self, FastbootOperation};

use super::{blocking, join, AppState, OpResult};

#[tauri::command]
pub async fn fastboot_devices(
    state: State<'_, AppState>,
) -> Result<Vec<fastboot::FastbootDevice>, AppError> {
    let st = state.inner().clone();
    let s = st.settings.load();
    let fastboot = st
        .tools
        .require(ToolKind::Fastboot, s.fastboot_path.as_deref())?;
    let h = blocking(move || fastboot::list_devices(&fastboot));
    join(h).await
}

/// Executes a fastboot operation. Destructive ops (flash/erase/unlock/lock)
/// require the typed confirmation word — one click is never enough.
#[tauri::command]
pub async fn fastboot_execute(
    state: State<'_, AppState>,
    serial: Option<String>,
    op: FastbootOperation,
    confirmation: Option<String>,
) -> Result<OpResult, AppError> {
    let st = state.inner().clone();
    if let Some(serial) = serial.as_deref() {
        crate::security::validate_serial(serial)?;
    }
    let s = st.settings.load();
    let fastboot_bin = st
        .tools
        .require(ToolKind::Fastboot, s.fastboot_path.as_deref())?;

    if op.is_destructive() {
        let required = op
            .required_confirmation()
            .expect("destructive fastboot ops define a confirmation word");
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

    // Hoisted before `op` moves into the blocking thread.
    let op_name = op.name().to_string();
    let args = op.to_args_for_serial(serial.as_deref())?;
    let described = format!("{} {}", fastboot_bin, args.join(" "));
    st.log.info(&format!("fastboot: {op_name}"));

    let serial_for_run = serial.clone();
    let h = blocking(move || op.run_on_device(&fastboot_bin, serial_for_run.as_deref()));
    let out = join(h).await?;

    let (result_str, code) = if out.success() {
        ("ok".to_string(), None)
    } else {
        (
            format!("error:{}", crate::commands::infer_code(&out).as_str()),
            Some(crate::commands::infer_code(&out).as_str().to_string()),
        )
    };

    st.audit.append(crate::storage::audit::AuditEntry {
        ts: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0),
        device: serial,
        action: op_name,
        command: described,
        result: result_str,
        undo: None,
    });

    Ok(OpResult {
        ok: out.success(),
        stdout: out.text(),
        stderr: out.stderr.clone(),
        code,
    })
}
