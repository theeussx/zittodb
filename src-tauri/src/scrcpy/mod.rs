//! scrcpy integration (spec §8, §14, §17).
//!
//! We do NOT reimplement screen streaming: scrcpy is a mature, official tool.
//! Zittodb only:
//!   - locates the scrcpy binary,
//!   - builds a validated argv (presets → flags),
//!   - launches/stops it as a managed child process (no orphans),
//!   - reports status and streams its log lines to the UI.
//!
//! Recording (spec §17): scrcpy records with `--record <path>` for the
//! lifetime of the session, so "Gravação" is a launch option and the UI shows
//! the recording state while the session is alive.

use serde::{Deserialize, Serialize};

use crate::error::{AppError, ErrorCode};
use crate::security::{validate_bitrate, validate_local_path, validate_serial};

/// scrcpy 3.2 contains the upstream fixes for Android 15 framework changes.
/// Older binaries (including Ubuntu/Debian's 1.25 package) can fail before
/// the video stream starts with SurfaceControl/Clipboard NoSuchMethodException.
pub const MIN_ANDROID_15_VERSION: (u32, u32, u32) = (3, 2, 0);

pub fn parse_version(text: &str) -> Option<(u32, u32, u32)> {
    let token = text.split_whitespace().find(|part| {
        part.starts_with('v') || part.chars().next().is_some_and(|c| c.is_ascii_digit())
    })?;
    let token = token.trim_start_matches('v');
    let mut parts = token.split('.').map(|part| {
        part.chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse::<u32>()
            .ok()
    });
    Some((
        parts.next()??,
        parts.next().unwrap_or(Some(0))?,
        parts.next().unwrap_or(Some(0))?,
    ))
}

pub fn supports_android_15(version: (u32, u32, u32)) -> bool {
    version >= MIN_ANDROID_15_VERSION
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrcpyOptions {
    /// "low" | "balanced" | "high" | "custom" (None => custom)
    pub preset: Option<String>,
    /// Max width in px (height auto). e.g. 1280
    pub max_size: Option<u32>,
    pub max_fps: Option<u32>,
    /// e.g. "2M"
    pub bitrate: Option<String>,
    /// "auto" | "portrait" | "landscape"
    pub orientation: Option<String>,
    pub turn_screen_off: bool,
    pub audio: bool,
    pub always_on_top: bool,
    pub record_path: Option<String>,
}

impl Default for ScrcpyOptions {
    fn default() -> Self {
        ScrcpyOptions {
            preset: None,
            max_size: None,
            max_fps: None,
            bitrate: None,
            orientation: None,
            turn_screen_off: false,
            audio: false,
            always_on_top: false,
            record_path: None,
        }
    }
}

impl ScrcpyOptions {
    /// Fills in defaults for a preset without overriding explicit values.
    pub fn apply_preset(&mut self, preset: &str) {
        match preset {
            // For weak computers (spec §14): 720p, 30 fps, moderate bitrate.
            "low" => {
                if self.max_size.is_none() {
                    self.max_size = Some(1280);
                }
                if self.max_fps.is_none() {
                    self.max_fps = Some(30);
                }
                if self.bitrate.is_none() {
                    self.bitrate = Some("2M".into());
                }
            }
            "balanced" => {
                if self.max_size.is_none() {
                    self.max_size = Some(1920);
                }
                if self.max_fps.is_none() {
                    self.max_fps = Some(60);
                }
                if self.bitrate.is_none() {
                    self.bitrate = Some("4M".into());
                }
            }
            "high" => {
                // full device resolution, higher fps/bitrate
                if self.max_fps.is_none() {
                    self.max_fps = Some(120);
                }
                if self.bitrate.is_none() {
                    self.bitrate = Some("8M".into());
                }
            }
            _ => {}
        }
    }

    pub fn validate(&self) -> Result<(), AppError> {
        if let Some(size) = self.max_size {
            if !(320..=3840).contains(&size) {
                return Err(AppError::new(
                    ErrorCode::InvalidArgument,
                    format!("max_size out of range: {size}"),
                ));
            }
        }
        if let Some(fps) = self.max_fps {
            if !(1..=240).contains(&fps) {
                return Err(AppError::new(
                    ErrorCode::InvalidArgument,
                    format!("max_fps out of range: {fps}"),
                ));
            }
        }
        if let Some(b) = &self.bitrate {
            validate_bitrate(b)?;
        }
        if let Some(o) = &self.orientation {
            if !matches!(o.as_str(), "auto" | "portrait" | "landscape") {
                return Err(AppError::new(
                    ErrorCode::InvalidArgument,
                    format!("invalid orientation: {o}"),
                ));
            }
        }
        if let Some(r) = &self.record_path {
            validate_local_path(r)?;
        }
        Ok(())
    }

    /// The scrcpy argv (without the executable).
    pub fn to_args(&self, serial: &str) -> Result<Vec<String>, AppError> {
        validate_serial(serial)?;
        let mut a = vec!["-s".to_string(), serial.to_string()];
        if let Some(size) = self.max_size {
            a.push("--max-size".into());
            a.push(size.to_string());
        }
        if let Some(fps) = self.max_fps {
            a.push("--max-fps".into());
            a.push(fps.to_string());
        }
        if let Some(b) = &self.bitrate {
            // scrcpy 3.3 removed the ambiguous --bit-rate alias.
            a.push("--video-bit-rate".into());
            a.push(b.clone());
        }
        // Note: scrcpy follows the device orientation on its own and has no
        // orientation flag in stable releases; `orientation` stays validated
        // and reserved for API compatibility, but is not forwarded.
        let _ = &self.orientation;
        if self.turn_screen_off {
            a.push("--turn-screen-off".into());
        }
        if self.always_on_top {
            a.push("--always-on-top".into());
        }
        if self.audio {
            a.push("--audio".into());
        }
        if let Some(r) = &self.record_path {
            a.push("--record".into());
            a.push(r.clone());
        }
        Ok(a)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrcpyStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub recording: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_low_matches_spec() {
        let mut o = ScrcpyOptions::default();
        o.apply_preset("low");
        assert_eq!(o.max_size, Some(1280));
        assert_eq!(o.max_fps, Some(30));
        assert_eq!(o.bitrate.as_deref(), Some("2M"));
    }

    #[test]
    fn preset_does_not_override_custom_values() {
        let mut o = ScrcpyOptions {
            max_fps: Some(15),
            ..Default::default()
        };
        o.apply_preset("low");
        assert_eq!(o.max_fps, Some(15));
        assert_eq!(o.max_size, Some(1280));
    }

    #[test]
    fn argv_building() {
        let o = ScrcpyOptions {
            max_size: Some(1280),
            max_fps: Some(30),
            bitrate: Some("2M".into()),
            turn_screen_off: true,
            record_path: Some("/tmp/rec.mp4".into()),
            ..Default::default()
        };
        let args = o.to_args("S1").unwrap();
        assert_eq!(
            args,
            vec![
                "-s",
                "S1",
                "--max-size",
                "1280",
                "--max-fps",
                "30",
                "--video-bit-rate",
                "2M",
                "--turn-screen-off",
                "--record",
                "/tmp/rec.mp4",
            ]
        );
    }

    #[test]
    fn validation_rejects_bad_values() {
        let o = ScrcpyOptions {
            max_size: Some(100),
            ..Default::default()
        };
        assert!(o.validate().is_err());

        let o = ScrcpyOptions {
            bitrate: Some("2x".into()),
            ..Default::default()
        };
        assert!(o.validate().is_err());

        let o = ScrcpyOptions {
            orientation: Some("sideways".into()),
            ..Default::default()
        };
        assert!(o.validate().is_err());
    }

    #[test]
    fn serial_is_validated() {
        let o = ScrcpyOptions::default();
        assert!(o.to_args("bad;serial").is_err());
    }

    #[test]
    fn parses_scrcpy_versions() {
        assert_eq!(parse_version("scrcpy 1.25"), Some((1, 25, 0)));
        assert_eq!(parse_version("scrcpy v3.2"), Some((3, 2, 0)));
        assert_eq!(parse_version("scrcpy 4.1"), Some((4, 1, 0)));
        assert_eq!(parse_version("not a version"), None);
    }

    #[test]
    fn android_15_requires_upstream_fix() {
        assert!(!supports_android_15((1, 25, 0)));
        assert!(!supports_android_15((3, 1, 0)));
        assert!(supports_android_15((3, 2, 0)));
        assert!(supports_android_15((4, 1, 0)));
    }
}
