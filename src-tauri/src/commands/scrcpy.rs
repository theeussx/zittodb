//! scrcpy commands (spec §8, §14, §17).

use std::sync::Arc;

use tauri::AppHandle;
use tauri::Emitter;
use tauri::State;

use crate::adb::ToolKind;
use crate::error::{AppError, ErrorCode};
use crate::processes::{run_captured, spawn_streamed};
use crate::scrcpy::{
    parse_version, supports_android_15, ScrcpyOptions, ScrcpyStatus, MIN_ANDROID_15_VERSION,
};
use crate::security::validate_serial;

use super::{media, AppState};

const SCRCPY_ID: &str = "scrcpy";

fn check_scrcpy_compatibility(scrcpy_bin: &str) -> Result<(), AppError> {
    let output = run_captured(
        scrcpy_bin,
        &["--version".to_string()],
        std::time::Duration::from_secs(5),
    )?;
    let version_text = format!("{} {}", output.text(), output.stderr);
    let Some(version) = parse_version(&version_text) else {
        // Unknown version output should not make an otherwise usable custom
        // build unusable. The process itself will report any real failure.
        return Ok(());
    };
    if supports_android_15(version) {
        return Ok(());
    }
    Err(AppError::new(
        ErrorCode::ScrcpyIncompatible,
        format!(
            "Detected scrcpy {major}.{minor}.{patch}; Android 15 requires scrcpy >= {}.{}.{}. Upgrade from the official release: https://github.com/Genymobile/scrcpy/releases",
            MIN_ANDROID_15_VERSION.0,
            MIN_ANDROID_15_VERSION.1,
            MIN_ANDROID_15_VERSION.2,
            major = version.0,
            minor = version.1,
            patch = version.2,
        ),
    ))
}

fn is_android_15_or_newer(adb_bin: &str, serial: &str) -> bool {
    let args = vec![
        "-s".to_string(),
        serial.to_string(),
        "shell".to_string(),
        "getprop".to_string(),
        "ro.build.version.sdk".to_string(),
    ];
    let Ok(output) = run_captured(adb_bin, &args, std::time::Duration::from_secs(5)) else {
        return false;
    };
    output
        .text()
        .trim()
        .parse::<u32>()
        .is_ok_and(|sdk| sdk >= 35)
}

#[tauri::command]
pub fn scrcpy_start(
    app: AppHandle,
    state: State<'_, AppState>,
    serial: String,
    options: ScrcpyOptions,
) -> Result<ScrcpyStatus, AppError> {
    let st = state.inner().clone();
    let s = st.settings.load();
    validate_serial(&serial)?;
    let scrcpy_bin = st
        .tools
        .require(ToolKind::Scrcpy, s.scrcpy_path.as_deref())?;

    // Do this before spawning the long-running process. Ubuntu/Debian often
    // expose scrcpy 1.25, whose bundled server crashes on Android 15 with
    // SurfaceControl and Clipboard NoSuchMethodException errors.
    if let Ok(adb_bin) = st.tools.require(ToolKind::Adb, s.adb_path.as_deref()) {
        if is_android_15_or_newer(&adb_bin, &serial) {
            check_scrcpy_compatibility(&scrcpy_bin)?;
        }
    }

    if st.registry.is_running(SCRCPY_ID) {
        return Err(AppError::new(
            ErrorCode::AlreadyRunning,
            "scrcpy is already running",
        ));
    }

    let mut opts = options;
    if let Some(preset) = opts.preset.clone() {
        opts.apply_preset(&preset);
    }
    opts.validate()?;
    let args = opts.to_args(&serial)?;

    let recording = opts.record_path.is_some();

    // Stream scrcpy's log lines to the UI (it prints errors like
    // "ERROR: unable to find the device" on stderr).
    let emitter = app.clone();
    let serial_for_event = serial.clone();
    let on_line = move |line: String| {
        let _ = emitter.emit(
            "scrcpy-log",
            serde_json::json!({ "serial": serial_for_event, "line": line }),
        );
    };

    let label = format!("scrcpy -s {serial}");
    let handle = spawn_streamed(&scrcpy_bin, &args, &label, false, Arc::new(on_line))?;
    st.registry.add(SCRCPY_ID, handle);
    st.log.info(&format!("scrcpy started: {}", args.join(" ")));

    Ok(ScrcpyStatus {
        running: true,
        pid: st.registry.status(SCRCPY_ID).1,
        recording,
    })
}

#[tauri::command]
pub fn scrcpy_stop(state: State<'_, AppState>) -> Result<ScrcpyStatus, AppError> {
    let st = state.inner().clone();
    if st.registry.is_running(SCRCPY_ID) {
        st.registry.stop(SCRCPY_ID)?;
        st.log.info("scrcpy stopped");
    }
    Ok(ScrcpyStatus {
        running: false,
        pid: None,
        recording: false,
    })
}

#[tauri::command]
pub fn scrcpy_status(state: State<'_, AppState>) -> Result<ScrcpyStatus, AppError> {
    let st = state.inner().clone();
    let (running, pid) = st.registry.status(SCRCPY_ID);
    let recording = st.registry.get(SCRCPY_ID).map(|_| false).unwrap_or(false);
    Ok(ScrcpyStatus {
        running,
        pid,
        recording,
    })
}

/// Resolves (and creates) the recording directory for the UI to prefill.
#[tauri::command]
pub fn recording_dir_cmd(state: State<'_, AppState>) -> Result<String, AppError> {
    let st = state.inner().clone();
    Ok(media::recording_dir(&st).to_string_lossy().to_string())
}

/// Suggested recording file: never overwrites (spec §16 principle).
#[tauri::command]
pub fn recording_filename(state: State<'_, AppState>) -> Result<String, AppError> {
    let st = state.inner().clone();
    let dir = media::recording_dir(&st);
    std::fs::create_dir_all(&dir)?;
    let ts = chrono::Local::now().format("%Y-%m-%d-%H%M%S");
    let mut name = format!("recording-{ts}.mp4");
    if dir.join(&name).exists() {
        for i in 1..100 {
            let alt = format!("recording-{ts}-{i}.mp4");
            if !dir.join(&alt).exists() {
                name = alt;
                break;
            }
        }
    }
    Ok(dir.join(name).to_string_lossy().to_string())
}
