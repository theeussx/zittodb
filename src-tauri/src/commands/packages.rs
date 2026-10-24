//! Package manager commands (spec §18–19).

use tauri::State;

use crate::adb::operations::DeviceOperation;
use crate::error::{AppError, ErrorCode};

use super::{blocking, execute_op, join, run_readonly, AppState, OpResult};

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageRow {
    pub name: String,
    pub version: Option<String>,
    pub version_code: Option<u64>,
    pub uid: Option<String>,
    pub path: Option<String>,
    pub is_system: bool,
    pub is_disabled: bool,
}

/// Lists all packages with metadata (one `dumpsys package` pass + two small
/// `pm list` calls — no per-app round trips, spec §5).
#[tauri::command]
pub async fn list_packages(
    state: State<'_, AppState>,
    serial: Option<String>,
) -> Result<Vec<PackageRow>, AppError> {
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
    let h = blocking(move || {
        let all = crate::devices::list_all_packages(&adb, &serial)?;
        let tp = crate::devices::third_party_packages(&adb, &serial)?;
        let dis = crate::devices::disabled_packages(&adb, &serial)?;
        Ok(all
            .into_iter()
            .map(|p| PackageRow {
                is_system: !tp.contains(&p.name),
                is_disabled: dis.contains(&p.name),
                name: p.name,
                version: p.version_name,
                version_code: p.version_code,
                uid: p.uid,
                path: p.code_path,
            })
            .collect::<Vec<_>>())
    });
    join(h).await
}

#[tauri::command]
pub async fn package_info(
    state: State<'_, AppState>,
    serial: Option<String>,
    pkg: String,
) -> Result<crate::adb::parse::PackageMeta, AppError> {
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
    crate::security::validate_package(&pkg)?;
    let h = blocking(move || crate::devices::get_package_dump(&adb, &serial, &pkg));
    join(h).await
}

/// Package actions (spec §19). Destructive ones require the typed word.
#[tauri::command]
pub async fn package_action(
    state: State<'_, AppState>,
    serial: Option<String>,
    pkg: String,
    action: PackageAction,
    user: u32,
    confirmation: Option<String>,
) -> Result<OpResult, AppError> {
    let st = state.inner().clone();
    crate::security::validate_package(&pkg)?;

    let op = match action {
        PackageAction::Open => DeviceOperation::OpenPackage { pkg },
        PackageAction::Enable => DeviceOperation::EnablePackage { pkg },
        PackageAction::Disable => DeviceOperation::DisablePackage { pkg, user },
        PackageAction::UninstallForUser => DeviceOperation::UninstallForUser { pkg, user },
        PackageAction::ClearData => DeviceOperation::ClearPackageData { pkg },
        PackageAction::Reinstall => DeviceOperation::ReinstallExisting { pkg },
    };
    execute_op(st, serial, op, confirmation).await
}

#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageAction {
    Open,
    Enable,
    Disable,
    UninstallForUser,
    ClearData,
    Reinstall,
}

/// Extracts a package APK to the local machine (pm path + adb pull).
#[tauri::command]
pub async fn package_extract(
    state: State<'_, AppState>,
    serial: Option<String>,
    pkg: String,
    dest_dir: Option<String>,
) -> Result<String, AppError> {
    let st = state.inner().clone();
    crate::security::validate_package(&pkg)?;
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

    // 1. Locate the APK on the device.
    let apks = run_readonly(
        st.clone(),
        Some(serial.clone()),
        DeviceOperation::PackagePath { pkg: pkg.clone() },
    )
    .await?;
    let apk = apks
        .lines()
        .filter_map(|l| l.trim().strip_prefix("package:"))
        .map(|s| s.trim().to_string())
        .find(|s| !s.ends_with("split_config.*.apk"))
        .ok_or_else(|| {
            AppError::new(ErrorCode::ProcessFailed, format!("no APK found for {pkg}"))
        })?;
    // The path comes from the DEVICE's output: validate it like any input so a
    // hostile or odd reply (e.g. "-x") can never become an adb option.
    crate::security::validate_device_path(&apk)?;

    // 2. Pull it.
    let dir = match dest_dir {
        Some(d) => {
            crate::security::validate_local_path(&d)?;
            std::path::PathBuf::from(d)
        }
        None => super::media::download_dir(&st),
    };
    std::fs::create_dir_all(&dir)?;
    let base = std::path::Path::new(&apk)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| format!("{pkg}.apk"));
    let dest = dir.join(base);
    if dest.exists() {
        return Err(AppError::new(
            ErrorCode::FileExists,
            format!("file exists: {}", dest.display()),
        ));
    }

    let dest_s = dest.to_string_lossy().to_string();
    let audit_serial = serial.clone();
    let audit_cmd = format!(
        "adb -s {serial} pull {apk} {}",
        super::audit_basename(&dest_s)
    );
    let h = blocking(move || {
        let out = crate::processes::run_captured(
            &adb,
            &["-s".into(), serial, "pull".into(), apk, dest_s],
            crate::adb::client::TIMEOUT_INSTALL,
        )?;
        if !out.success() {
            return Err(crate::error::AppError::from_process(
                crate::error::ErrorCode::ProcessFailed,
                &adb,
                &out,
            ));
        }
        Ok(())
    });
    let res = join(h).await;
    let result = match &res {
        Ok(()) => "ok".to_string(),
        Err(e) => format!("error:{}", e.code.as_str()),
    };
    super::audit_event(
        &st.audit,
        Some(audit_serial),
        "package_extract",
        audit_cmd,
        result,
    );
    res?;
    Ok(dest.to_string_lossy().to_string())
}
