//! Generic operation execution + Command Builder support (spec §22, §26).

use tauri::State;

use crate::adb::operations::DeviceOperation;
use crate::error::{AppError, ErrorCode};
use crate::security::risk;

use super::{execute_op, AppState, OpResult};

/// Renders the exact command a builder form would run (single source of
/// truth: the same builder that executes also describes, spec §26).
#[tauri::command]
pub fn describe_operation(
    state: State<'_, AppState>,
    op: DeviceOperation,
    serial: Option<String>,
) -> Result<String, AppError> {
    let st = state.inner().clone();
    let adb = st.adb()?;
    op.describe(&adb, serial.as_deref())
}

/// Executes a single allowlisted operation (Command Builder "Executar").
#[tauri::command]
pub async fn execute_operation(
    state: State<'_, AppState>,
    serial: Option<String>,
    op: DeviceOperation,
    confirmation: Option<String>,
) -> Result<OpResult, AppError> {
    let st = state.inner().clone();
    execute_op(st, serial, op, confirmation).await
}

/// Batch execution for debloat (spec §22): one operation template applied to
/// many packages, executed sequentially under the per-device lock, with a
/// full preview-first flow on the frontend.
#[tauri::command]
pub async fn execute_batch(
    state: State<'_, AppState>,
    serial: String,
    op: DeviceOperation,
    packages: Vec<String>,
    confirmation: Option<String>,
) -> Result<BatchResult, AppError> {
    let st = state.inner().clone();
    crate::security::validate_serial(&serial)?;

    // Only package-level ops may be batched (keeps the confirmation model
    // sound: one required word for the whole batch).
    for pkg in &packages {
        crate::security::validate_package(pkg)?;
    }
    if !is_batchable(&op) {
        return Err(AppError::new(
            ErrorCode::OperationRejected,
            format!("operation '{}' cannot be batched", op.name()),
        ));
    }
    if packages.is_empty() {
        return Ok(BatchResult {
            serial,
            results: Vec::new(),
        });
    }

    let mut results = Vec::with_capacity(packages.len());
    for (i, pkg) in packages.iter().enumerate() {
        if risk::classify_package(pkg) == risk::RiskLevel::Critical {
            results.push(BatchItem {
                index: i as u32,
                package: pkg.clone(),
                ok: false,
                code: Some(ErrorCode::OperationRejected.as_str().to_string()),
                message: "critical package is protected from debloat".into(),
            });
            continue;
        }
        let item_op = with_package(&op, pkg);
        let res = execute_op(
            st.clone(),
            Some(serial.clone()),
            item_op,
            confirmation.clone(),
        )
        .await;
        let (ok, code, message) = match res {
            Ok(r) => (r.ok, r.code.clone(), r.stdout),
            Err(e) => (false, Some(e.code.as_str().to_string()), e.details),
        };
        results.push(BatchItem {
            index: i as u32,
            package: pkg.clone(),
            ok,
            code,
            message,
        });
    }
    Ok(BatchResult { serial, results })
}

fn is_batchable(op: &DeviceOperation) -> bool {
    use DeviceOperation as O;
    matches!(
        op,
        O::EnablePackage { .. }
            | O::DisablePackage { .. }
            | O::UninstallForUser { .. }
            | O::ReinstallExisting { .. }
            | O::ClearPackageData { .. }
    )
}

fn with_package(template: &DeviceOperation, pkg: &str) -> DeviceOperation {
    use DeviceOperation as O;
    match template {
        O::EnablePackage { .. } => O::EnablePackage { pkg: pkg.into() },
        O::DisablePackage { user, .. } => O::DisablePackage {
            pkg: pkg.into(),
            user: *user,
        },
        O::UninstallForUser { user, .. } => O::UninstallForUser {
            pkg: pkg.into(),
            user: *user,
        },
        O::ReinstallExisting { .. } => O::ReinstallExisting { pkg: pkg.into() },
        O::ClearPackageData { .. } => O::ClearPackageData { pkg: pkg.into() },
        _ => unreachable!("is_batchable checked"),
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchResult {
    pub serial: String,
    pub results: Vec<BatchItem>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchItem {
    pub index: u32,
    pub package: String,
    pub ok: bool,
    pub code: Option<String>,
    pub message: String,
}

/// Debloat helper: risk classification for the selected packages (spec §20).
#[tauri::command]
pub fn classify_packages(packages: Vec<String>) -> Result<Vec<PackageRisk>, AppError> {
    Ok(packages
        .iter()
        .map(|p| PackageRisk {
            package: p.clone(),
            risk: risk::classify_package(p),
        })
        .collect())
}

#[tauri::command]
pub fn debloat_profiles() -> Vec<risk::DebloatProfile> {
    risk::profiles()
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageRisk {
    pub package: String,
    pub risk: risk::RiskLevel,
}
