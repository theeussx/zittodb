//! User settings — small JSON file, atomic writes (spec §37, §70).

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, ErrorCode};
use super::{ensure_private_dir, ensure_private_file};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// "dark" | "light" | "system"
    pub theme: String,
    /// "auto" | "pt-BR" | "en-US"
    pub language: String,
    /// "normal" | "low" | "ultra"  (spec §15, §40)
    pub performance_mode: String,
    /// Manual tool paths (None = auto-detect)
    pub adb_path: Option<String>,
    pub scrcpy_path: Option<String>,
    pub fastboot_path: Option<String>,
    /// Storage locations (None = platform default)
    pub screenshot_dir: Option<String>,
    pub recording_dir: Option<String>,
    pub download_dir: Option<String>,
    /// Optional device auto-refresh (seconds). None = off (default,
    /// to keep polling out of the way — spec §5).
    pub auto_refresh_secs: Option<u32>,
    /// Logcat visible-line cap (spec §29).
    pub logcat_max_lines: u32,
    /// Keep the local audit log (spec §54).
    pub audit_enabled: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: "system".into(),
            language: "auto".into(),
            performance_mode: "normal".into(),
            adb_path: None,
            scrcpy_path: None,
            fastboot_path: None,
            screenshot_dir: None,
            recording_dir: None,
            download_dir: None,
            auto_refresh_secs: None,
            logcat_max_lines: 5000,
            audit_enabled: true,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), AppError> {
        if !matches!(self.theme.as_str(), "dark" | "light" | "system") {
            return Err(AppError::new(
                ErrorCode::InvalidArgument,
                format!("invalid theme: {}", self.theme),
            ));
        }
        if !matches!(self.language.as_str(), "auto" | "pt-BR" | "en-US") {
            return Err(AppError::new(
                ErrorCode::InvalidArgument,
                format!("invalid language: {}", self.language),
            ));
        }
        if !matches!(self.performance_mode.as_str(), "normal" | "low" | "ultra") {
            return Err(AppError::new(
                ErrorCode::InvalidArgument,
                format!("invalid performance mode: {}", self.performance_mode),
            ));
        }
        if let Some(s) = self.auto_refresh_secs {
            if s < 2 || s > 300 {
                return Err(AppError::new(
                    ErrorCode::InvalidArgument,
                    format!("auto_refresh_secs out of range: {s}"),
                ));
            }
        }
        if self.logcat_max_lines < 200 || self.logcat_max_lines > 100_000 {
            return Err(AppError::new(
                ErrorCode::InvalidArgument,
                format!("logcat_max_lines out of range: {}", self.logcat_max_lines),
            ));
        }
        Ok(())
    }
}

pub struct SettingsStore {
    path: PathBuf,
    current: Mutex<Settings>,
}

impl SettingsStore {
    pub fn open(dir: &std::path::Path) -> SettingsStore {
        ensure_private_dir(dir);
        let path = dir.join("settings.json");
        ensure_private_file(&path);
        let loaded = load_from(&path).unwrap_or_default();
        SettingsStore {
            path,
            current: Mutex::new(loaded),
        }
    }

    /// For tests.
    pub fn at(path: PathBuf) -> SettingsStore {
        if let Some(parent) = path.parent() {
            ensure_private_dir(parent);
        }
        ensure_private_file(&path);
        let loaded = load_from(&path).unwrap_or_default();
        SettingsStore {
            path,
            current: Mutex::new(loaded),
        }
    }

    pub fn load(&self) -> Settings {
        self.current.lock().unwrap().clone()
    }

    /// Validates and atomically persists.
    pub fn save(&self, new: &Settings) -> Result<(), AppError> {
        new.validate()?;
        let tmp = self.path.with_extension("json.tmp");
        fs::write(
            &tmp,
            serde_json::to_string_pretty(new)
                .map_err(|e| AppError::new(ErrorCode::Unexpected, format!("serialize: {e}")))?,
        )?;
        ensure_private_file(&tmp);
        fs::rename(&tmp, &self.path)?;
        ensure_private_file(&self.path);
        *self.current.lock().unwrap() = new.clone();
        Ok(())
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

fn load_from(path: &std::path::Path) -> Option<Settings> {
    let s = fs::read_to_string(path).ok()?;
    serde_json::from_str(&s).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_safe() {
        let s = Settings::default();
        assert!(s.validate().is_ok());
        assert_eq!(s.performance_mode, "normal");
        assert_eq!(s.logcat_max_lines, 5000);
        assert!(s.auto_refresh_secs.is_none()); // no polling by default
        assert!(s.audit_enabled);
    }

    #[test]
    fn rejects_bad_values() {
        let mut s = Settings::default();
        s.theme = "neon".into();
        assert!(s.validate().is_err());
        s = Settings::default();
        s.performance_mode = "turbo".into();
        assert!(s.validate().is_err());
        s = Settings::default();
        s.auto_refresh_secs = Some(1);
        assert!(s.validate().is_err());
        s = Settings::default();
        s.logcat_max_lines = 10;
        assert!(s.validate().is_err());
    }

    #[test]
    fn save_and_reload() {
        let dir = std::env::temp_dir().join(format!("zittodb-set-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let store = SettingsStore::at(dir.join("settings.json"));
        let mut s = Settings::default();
        s.theme = "dark".into();
        s.performance_mode = "low".into();
        store.save(&s).unwrap();

        let reloaded = SettingsStore::at(dir.join("settings.json"));
        assert_eq!(reloaded.load().theme, "dark");
        assert_eq!(reloaded.load().performance_mode, "low");

        // corrupt file must not kill the app: falls back to defaults
        fs::write(dir.join("settings.json"), "{ not json").unwrap();
        let reloaded = SettingsStore::at(dir.join("settings.json"));
        assert_eq!(reloaded.load().theme, "system");
        let _ = fs::remove_dir_all(&dir);
    }
}
