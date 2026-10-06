//! Zittodb — Tauri application crate.
//!
//! Responsibility split (spec §42):
//!   frontend: UI, state, UX, formatting
//!   Rust:     process execution, ADB/scrcpy/fastboot, filesystem,
//!             validation, security, persistence

pub mod adb;
mod applog;
pub mod commands;
pub mod devices;
pub mod error;
pub mod fastboot;
pub mod processes;
pub mod scrcpy;
pub mod security;
pub mod storage;

use std::sync::Arc;

use commands::AppState;
use devices::Coordinator;
use processes::{LogcatManager, ProcessRegistry, ShellManager, TransferManager};
use storage::audit::AuditLog;
use storage::history::DeviceHistory;
use storage::settings::SettingsStore;

pub fn run() {
    let dirs = storage::app_dirs();

    let log = Arc::new(applog::AppLog::open(&dirs.log).unwrap_or_else(|e| {
        eprintln!("zittodb: cannot open log: {e}");
        applog::AppLog::null()
    }));

    let settings = Arc::new(SettingsStore::open(&dirs.config));
    let initial = settings.load();

    let audit = Arc::new(AuditLog::open(&dirs.config, initial.audit_enabled));
    let history = Arc::new(DeviceHistory::open(&dirs.data));

    let state = AppState {
        dirs,
        log: log.clone(),
        settings: settings.clone(),
        tools: Arc::new(adb::ToolManager::new()),
        registry: Arc::new(ProcessRegistry::new()),
        shell_mgr: Arc::new(ShellManager::new()),
        logcat_mgr: Arc::new(LogcatManager::new()),
        transfers: Arc::new(TransferManager::new()),
        audit,
        history,
        coordinator: Arc::new(Coordinator::new()),
    };

    log.info(&format!(
        "Zittodb {} starting (theme={}, perf={})",
        env!("CARGO_PKG_VERSION"),
        initial.theme,
        initial.performance_mode
    ));

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            // tools
            commands::tools::detect_tools,
            commands::tools::set_tool_path,
            commands::tools::check_tool_path,
            commands::tools::adb_server_version,
            commands::tools::adb_server_start,
            commands::tools::adb_server_restart,
            // devices
            commands::devices::list_devices,
            commands::devices::get_device_info,
            commands::devices::get_battery,
            commands::devices::get_storage,
            commands::devices::get_network_info,
            commands::devices::get_device_props,
            commands::devices::adb_connect,
            commands::devices::adb_disconnect,
            commands::devices::reboot_device,
            // media
            commands::media::screenshot,
            commands::media::pick_path,
            commands::media::open_path,
            commands::media::copy_image_to_clipboard,
            // scrcpy
            commands::scrcpy::scrcpy_start,
            commands::scrcpy::scrcpy_stop,
            commands::scrcpy::scrcpy_status,
            commands::scrcpy::recording_dir_cmd,
            commands::scrcpy::recording_filename,
            // shell
            commands::shell::shell_open,
            commands::shell::shell_write,
            commands::shell::shell_close,
            commands::shell::shell_close_all,
            commands::shell::shell_list,
            // packages
            commands::packages::list_packages,
            commands::packages::package_info,
            commands::packages::package_action,
            commands::packages::package_extract,
            // files
            commands::files::file_list,
            commands::files::file_mkdir,
            commands::files::file_rename,
            commands::files::file_delete,
            commands::files::file_push,
            commands::files::file_pull,
            commands::files::transfer_cancel,
            // logs
            commands::logs::logcat_start,
            commands::logs::logcat_stop,
            commands::logs::logcat_list,
            commands::logs::save_log_file,
            // operations / builder / debloat
            commands::operations::describe_operation,
            commands::operations::execute_operation,
            commands::operations::execute_batch,
            commands::operations::classify_packages,
            commands::operations::debloat_profiles,
            // fastboot
            commands::fastboot::fastboot_devices,
            commands::fastboot::fastboot_execute,
            // settings / audit / history / app
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::get_audit,
            commands::settings::clear_audit,
            commands::settings::get_device_history,
            commands::settings::clear_device_history,
            commands::settings::update_device_metadata,
            commands::settings::get_app_info,
            commands::settings::get_app_paths,
        ])
        // No orphan processes (spec §45): AppState is dropped with the app and
        // its Drop impls (registry, shell, logcat, transfers) kill every
        // child. (Tauri 2 offers no `Builder::on_exit` hook, so shutdown
        // cleanup relies on these `Drop` impls, which run deterministically.)
        .run(tauri::generate_context!())
        .expect("error while running Zittodb");
}
