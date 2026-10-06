//! Local persistence (spec §36, §54, §65).
//!
//! Deliberately tiny: plain JSON + JSONL in the user's config dir.
//! No database, no sync, no cloud (spec §65 anti-overengineering rule).

pub mod audit;
pub mod history;
pub mod settings;

use std::path::PathBuf;

/// Creates a directory that may contain device serials, package names or
/// other user-local metadata with owner-only permissions on Unix.
pub(crate) fn ensure_private_dir(path: &std::path::Path) {
    if std::fs::create_dir_all(path).is_ok() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700));
        }
    }
}

/// Restricts a persisted file to its owner on Unix. On other platforms the
/// platform's normal application-data ACLs remain in effect.
pub(crate) fn ensure_private_file(path: &std::path::Path) {
    #[cfg(unix)]
    if path.exists() {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
}

#[derive(Debug, Clone)]
pub struct AppDirs {
    pub config: PathBuf,
    pub data: PathBuf,
    pub log: PathBuf,
}

/// Resolves the app's local directories (Linux:
/// `~/.config/app.zittodb.desktop`, `~/.local/share/app.zittodb.desktop`).
///
/// Falls back to `./.zittodb` when HOME is unavailable (headless tests).
pub fn app_dirs() -> AppDirs {
    let dirs = match directories::ProjectDirs::from("app", "zittodb", "desktop") {
        Some(pd) => AppDirs {
            config: pd.config_dir().to_path_buf(),
            data: pd.data_dir().to_path_buf(),
            log: pd.data_dir().join("logs"),
        },
        None => {
            let base = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let base = base.join(".zittodb");
            AppDirs {
                config: base.clone(),
                data: base.clone(),
                log: base.join("logs"),
            }
        }
    };
    ensure_private_dir(&dirs.config);
    ensure_private_dir(&dirs.data);
    ensure_private_dir(&dirs.log);
    dirs
}
