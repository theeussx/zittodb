//! Allowlist of ADB operations (spec §44).
//!
//! The frontend never sends shell strings. It sends a typed `DeviceOperation`
//! and this module:
//!   1. validates every user-supplied field (security module),
//!   2. builds the exact `adb` argv vector,
//!   3. renders the same vector for display/copy ("describe"),
//! so what you see is exactly what runs — single source of truth.
//!
//! Device-side commands (after `adb ... shell`) are a single string argument
//! whose user-provided values are POSIX-single-quoted (`quote_shell`).

use serde::{Deserialize, Serialize};

use crate::error::{AppError, ErrorCode};
use crate::security::{
    quote_shell, validate_device_path, validate_host, validate_local_path,
    validate_mutable_device_path, validate_package, validate_serial,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RebootTarget {
    System,
    Bootloader,
    Recovery,
    Sideload,
}

impl RebootTarget {
    pub fn as_str(&self) -> &'static str {
        match self {
            RebootTarget::System => "system",
            RebootTarget::Bootloader => "bootloader",
            RebootTarget::Recovery => "recovery",
            RebootTarget::Sideload => "sideload",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case", tag = "op")]
pub enum DeviceOperation {
    // ---- read-only -------------------------------------------------------
    GetProps,
    GetKernel,
    /// `getenforce`: SELinux mode (Enforcing / Permissive).
    GetSelinuxMode,
    GetMeminfo,
    GetDiskUsage {
        path: String,
    },
    GetBattery,
    GetNetwork,
    GetResolv,
    GetMac {
        iface: String,
    },
    // ---- media -----------------------------------------------------------
    Screenshot,
    // ---- packages ---------------------------------------------------------
    // NOTE: enum-level `rename_all` only renames variants, not struct fields
    // (serde has a separate `rename_all_fields` for that), so the multi-word
    // field below needs its own rule to match the frontend (`thirdParty`).
    #[serde(rename_all = "camelCase")]
    ListPackages {
        third_party: bool,
        disabled: bool,
    },
    PackagePath {
        pkg: String,
    },
    PackageDump {
        pkg: String,
    },
    OpenPackage {
        pkg: String,
    },
    EnablePackage {
        pkg: String,
    },
    DisablePackage {
        pkg: String,
        user: u32,
    },
    UninstallForUser {
        pkg: String,
        user: u32,
    },
    ReinstallExisting {
        pkg: String,
    },
    ClearPackageData {
        pkg: String,
    },
    /// `adb -s S install -r <local apk>` — `-r` = replace existing (updates).
    /// Not destructive per policy (no typed confirmation); still audited.
    InstallApk {
        local: String,
    },
    // ---- files -------------------------------------------------------------
    ListDir {
        path: String,
    },
    Mkdir {
        path: String,
    },
    Rename {
        from: String,
        to: String,
    },
    Delete {
        path: String,
    },
    Push {
        local: String,
        remote: String,
    },
    Pull {
        remote: String,
        local: String,
    },
    // ---- lifecycle ---------------------------------------------------------
    Reboot {
        target: RebootTarget,
    },
    Connect {
        host: String,
        port: u16,
    },
    Disconnect {
        host: String,
        port: u16,
    },
}

impl DeviceOperation {
    /// Short stable name, used in the audit log and in the UI.
    pub fn name(&self) -> &'static str {
        use DeviceOperation as O;
        match self {
            O::GetProps => "get_props",
            O::GetKernel => "get_kernel",
            O::GetSelinuxMode => "get_selinux_mode",
            O::GetMeminfo => "get_meminfo",
            O::GetDiskUsage { .. } => "get_disk_usage",
            O::GetBattery => "get_battery",
            O::GetNetwork => "get_network",
            O::GetResolv => "get_resolv",
            O::GetMac { .. } => "get_mac",
            O::Screenshot => "screenshot",
            O::ListPackages { .. } => "list_packages",
            O::PackagePath { .. } => "package_path",
            O::PackageDump { .. } => "package_dump",
            O::OpenPackage { .. } => "open_package",
            O::EnablePackage { .. } => "enable_package",
            O::DisablePackage { .. } => "disable_package",
            O::UninstallForUser { .. } => "uninstall_for_user",
            O::ReinstallExisting { .. } => "reinstall_existing",
            O::ClearPackageData { .. } => "clear_package_data",
            O::InstallApk { .. } => "install_apk",
            O::ListDir { .. } => "list_dir",
            O::Mkdir { .. } => "mkdir",
            O::Rename { .. } => "rename",
            O::Delete { .. } => "delete",
            O::Push { .. } => "push",
            O::Pull { .. } => "pull",
            O::Reboot { .. } => "reboot",
            O::Connect { .. } => "connect",
            O::Disconnect { .. } => "disconnect",
        }
    }

    /// Operations that need no serial (operate on the adb server itself).
    pub fn requires_serial(&self) -> bool {
        !matches!(
            self,
            DeviceOperation::Connect { .. } | DeviceOperation::Disconnect { .. }
        )
    }

    /// Destructive = data may be lost or the device state changes hard.
    /// These always require an explicit typed confirmation (spec §53).
    pub fn is_destructive(&self) -> bool {
        matches!(
            self,
            DeviceOperation::Delete { .. }
                | DeviceOperation::UninstallForUser { .. }
                | DeviceOperation::ClearPackageData { .. }
                | DeviceOperation::Reboot { .. }
        )
    }

    /// The exact word the user must type to confirm.
    pub fn required_confirmation(&self) -> Option<&'static str> {
        use DeviceOperation as O;
        match self {
            O::Delete { .. } => Some("APAGAR"),
            O::ClearPackageData { .. } => Some("APAGAR"),
            O::UninstallForUser { .. } => Some("REMOVER"),
            O::Reboot { .. } => Some("REINICIAR"),
            _ => None,
        }
    }

    /// Validations for every user-supplied field.
    pub fn validate(&self) -> Result<(), AppError> {
        use DeviceOperation as O;
        match self {
            O::GetDiskUsage { path } | O::ListDir { path } | O::Mkdir { path } => {
                validate_device_path(path)
            }
            // `rm -rf` and `mv` must never target a protected root.
            O::Delete { path } => validate_mutable_device_path(path),
            O::Rename { from, to } => {
                validate_mutable_device_path(from)?;
                validate_device_path(to)
            }
            O::Push { local, remote } => {
                validate_local_path(local)?;
                validate_device_path(remote)
            }
            O::Pull { remote, local } => {
                validate_device_path(remote)?;
                validate_local_path(local)
            }
            O::PackagePath { pkg }
            | O::PackageDump { pkg }
            | O::OpenPackage { pkg }
            | O::EnablePackage { pkg }
            | O::ReinstallExisting { pkg }
            | O::ClearPackageData { pkg }
            | O::DisablePackage { pkg, .. }
            | O::UninstallForUser { pkg, .. } => validate_package(pkg),
            O::InstallApk { local } => validate_local_path(local),
            O::Connect { host, .. } | O::Disconnect { host, .. } => validate_host(host),
            O::GetMac { iface } => {
                if iface.is_empty()
                    || iface.len() > 32
                    || !iface
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                {
                    Err(AppError::new(
                        ErrorCode::InvalidArgument,
                        format!("invalid interface: {iface:?}"),
                    ))
                } else {
                    Ok(())
                }
            }
            _ => Ok(()),
        }
    }

    /// Builds the device-side shell command (single string) when the op needs
    /// one. Values are quoted with `quote_shell`; package names are already
    /// validated to a safe charset.
    fn device_cmd(&self) -> Option<String> {
        use DeviceOperation as O;
        match self {
            O::GetProps => Some("getprop".into()),
            O::GetKernel => Some("uname -r".into()),
            O::GetSelinuxMode => Some("getenforce".into()),
            O::GetMeminfo => Some("cat /proc/meminfo".into()),
            O::GetDiskUsage { path } => Some(format!("df -m {}", quote_shell(path))),
            O::GetBattery => Some("dumpsys battery".into()),
            O::GetNetwork => Some("ip addr show".into()),
            O::GetResolv => Some("cat /etc/resolv.conf".into()),
            O::GetMac { iface } => Some(format!("cat /sys/class/net/{}/address", iface)),
            O::ListPackages {
                third_party,
                disabled,
            } => {
                let mut c = String::from("pm list packages");
                if *third_party {
                    c.push_str(" -3");
                }
                if *disabled {
                    c.push_str(" -d");
                }
                Some(c)
            }
            O::PackagePath { pkg } => Some(format!("pm path {}", quote_shell(pkg))),
            O::PackageDump { pkg } => Some(format!("dumpsys package {}", quote_shell(pkg))),
            O::OpenPackage { pkg } => Some(format!(
                "monkey -p {} -c android.intent.category.LAUNCHER 1",
                quote_shell(pkg)
            )),
            O::EnablePackage { pkg } => Some(format!("pm enable {}", quote_shell(pkg))),
            O::DisablePackage { pkg, user } => Some(format!(
                "pm disable-user --user {} {}",
                user,
                quote_shell(pkg)
            )),
            O::UninstallForUser { pkg, user } => Some(format!(
                "pm uninstall -k --user {} {}",
                user,
                quote_shell(pkg)
            )),
            O::ReinstallExisting { pkg } => {
                Some(format!("pm install-existing {}", quote_shell(pkg)))
            }
            O::ClearPackageData { pkg } => Some(format!("pm clear {}", quote_shell(pkg))),
            O::ListDir { path } => Some(format!("ls -la {}", quote_shell(path))),
            O::Mkdir { path } => Some(format!("mkdir -p {}", quote_shell(path))),
            O::Rename { from, to } => Some(format!("mv {} {}", quote_shell(from), quote_shell(to))),
            O::Delete { path } => Some(format!("rm -rf {}", quote_shell(path))),
            _ => None,
        }
    }

    /// Validates the serial (when the op needs one) and returns an owned copy.
    fn require_serial<'a>(
        serial: Option<&'a str>,
        op_needs_it: bool,
    ) -> Result<Option<String>, AppError> {
        if !op_needs_it {
            return Ok(None);
        }
        let s = serial.ok_or_else(|| {
            AppError::new(ErrorCode::NoDevice, "operation requires a device serial")
        })?;
        validate_serial(s)?;
        Ok(Some(s.to_string()))
    }

    /// Full argv for `adb` (without the executable path).
    ///
    /// Examples:
    ///   GetProps          -> ["-s", "S", "shell", "getprop"]
    ///   Screenshot        -> ["-s", "S", "exec-out", "screencap", "-p"]
    ///   Connect{..}       -> ["connect", "192.168.1.100:5555"]
    pub fn to_adb_args(&self, serial: Option<&str>) -> Result<Vec<String>, AppError> {
        self.validate()?;
        use DeviceOperation as O;

        match self {
            O::Connect { host, port } => Ok(vec!["connect".into(), format!("{host}:{port}")]),
            O::Disconnect { host, port } => Ok(vec!["disconnect".into(), format!("{host}:{port}")]),
            O::Reboot { target } => {
                let s = Self::require_serial(serial, true)?.unwrap();
                Ok(vec![
                    "-s".into(),
                    s,
                    "reboot".into(),
                    target.as_str().into(),
                ])
            }
            O::Screenshot => {
                let s = Self::require_serial(serial, true)?.unwrap();
                Ok(vec![
                    "-s".into(),
                    s,
                    "exec-out".into(),
                    "screencap".into(),
                    "-p".into(),
                ])
            }
            O::Push { local, remote } => {
                let s = Self::require_serial(serial, true)?.unwrap();
                Ok(vec![
                    "-s".into(),
                    s,
                    "push".into(),
                    local.clone(),
                    remote.clone(),
                ])
            }
            O::Pull { remote, local } => {
                let s = Self::require_serial(serial, true)?.unwrap();
                Ok(vec![
                    "-s".into(),
                    s,
                    "pull".into(),
                    remote.clone(),
                    local.clone(),
                ])
            }
            O::InstallApk { local } => {
                let s = Self::require_serial(serial, true)?.unwrap();
                Ok(vec![
                    "-s".into(),
                    s,
                    "install".into(),
                    "-r".into(),
                    local.clone(),
                ])
            }
            O::ListPackages { .. }
            | O::GetProps
            | O::GetKernel
            | O::GetSelinuxMode
            | O::GetMeminfo
            | O::GetDiskUsage { .. }
            | O::GetBattery
            | O::GetNetwork
            | O::GetResolv
            | O::GetMac { .. }
            | O::PackagePath { .. }
            | O::PackageDump { .. }
            | O::OpenPackage { .. }
            | O::EnablePackage { .. }
            | O::DisablePackage { .. }
            | O::UninstallForUser { .. }
            | O::ReinstallExisting { .. }
            | O::ClearPackageData { .. }
            | O::ListDir { .. }
            | O::Mkdir { .. }
            | O::Rename { .. }
            | O::Delete { .. } => {
                let s = serial.ok_or_else(|| {
                    AppError::new(ErrorCode::NoDevice, "operation requires a device serial")
                })?;
                validate_serial(s)?;
                let cmd = self.device_cmd().ok_or_else(|| {
                    AppError::new(ErrorCode::Unexpected, "internal: no device command")
                })?;
                Ok(vec!["-s".into(), s.to_string(), "shell".into(), cmd])
            }
        }
    }

    /// Human-readable one-liner (what the user sees and can copy).
    pub fn describe(&self, adb_path: &str, serial: Option<&str>) -> Result<String, AppError> {
        let args = self.to_adb_args(serial)?;
        let joined = args
            .iter()
            .map(|a| {
                if a.contains(' ') {
                    format!("'{}'", a.replace('\'', r"'\''"))
                } else {
                    a.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        Ok(format!("{adb_path} {joined}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_exact_argv() {
        let op = DeviceOperation::DisablePackage {
            pkg: "com.example.app".into(),
            user: 0,
        };
        let args = op.to_adb_args(Some("23021RAA2Y")).unwrap();
        assert_eq!(
            args,
            vec![
                "-s",
                "23021RAA2Y",
                "shell",
                "pm disable-user --user 0 'com.example.app'"
            ]
        );
    }

    #[test]
    fn screenshot_argv_uses_exec_out() {
        let op = DeviceOperation::Screenshot;
        let args = op.to_adb_args(Some("S1")).unwrap();
        assert_eq!(args, vec!["-s", "S1", "exec-out", "screencap", "-p"]);
    }

    #[test]
    fn connect_has_no_serial() {
        let op = DeviceOperation::Connect {
            host: "192.168.1.100".into(),
            port: 5555,
        };
        let args = op.to_adb_args(None).unwrap();
        assert_eq!(args, vec!["connect", "192.168.1.100:5555"]);
        assert!(!op.requires_serial());
    }

    #[test]
    fn rejects_bad_package_in_op() {
        let op = DeviceOperation::EnablePackage {
            pkg: "com.x; rm -rf /".into(),
        };
        let err = op.to_adb_args(Some("S1")).expect_err("must fail");
        assert_eq!(err.code, ErrorCode::InvalidPackage);
    }

    #[test]
    fn rejects_bad_device_path() {
        let op = DeviceOperation::Delete {
            path: "../../etc".into(),
        };
        let err = op.to_adb_args(Some("S1")).expect_err("must fail");
        assert_eq!(err.code, ErrorCode::InvalidPath);
    }

    #[test]
    fn requires_serial_rule() {
        assert!(DeviceOperation::GetProps.requires_serial());
        assert!(!DeviceOperation::Connect {
            host: "h".into(),
            port: 1
        }
        .requires_serial());
    }

    #[test]
    fn destructive_flags_and_confirmations() {
        assert!(DeviceOperation::Delete {
            path: "/sdcard/x".into()
        }
        .is_destructive());
        assert!(DeviceOperation::UninstallForUser {
            pkg: "com.a.b".into(),
            user: 0
        }
        .is_destructive());
        assert!(DeviceOperation::ClearPackageData {
            pkg: "com.a.b".into()
        }
        .is_destructive());
        assert!(DeviceOperation::Reboot {
            target: RebootTarget::System
        }
        .is_destructive());
        assert!(!DeviceOperation::DisablePackage {
            pkg: "com.a.b".into(),
            user: 0
        }
        .is_destructive());

        assert_eq!(
            DeviceOperation::Delete {
                path: "/sdcard/x".into()
            }
            .required_confirmation(),
            Some("APAGAR")
        );
        assert_eq!(
            DeviceOperation::UninstallForUser {
                pkg: "com.a.b".into(),
                user: 0
            }
            .required_confirmation(),
            Some("REMOVER")
        );
        assert_eq!(
            DeviceOperation::DisablePackage {
                pkg: "com.a.b".into(),
                user: 0
            }
            .required_confirmation(),
            None
        );
    }

    #[test]
    fn describe_matches_argv() {
        let op = DeviceOperation::DisablePackage {
            pkg: "com.example.app".into(),
            user: 0,
        };
        let s = op.describe("/usr/bin/adb", Some("S1")).unwrap();
        assert!(s.starts_with("/usr/bin/adb -s S1 shell "));
        assert!(s.contains("pm disable-user --user 0"));
        assert!(s.contains("com.example.app"));
    }

    #[test]
    fn quoting_for_paths_with_spaces() {
        let op = DeviceOperation::Delete {
            path: "/sdcard/my folder/file name.txt".into(),
        };
        let args = op.to_adb_args(Some("S1")).unwrap();
        let cmd = &args[3];
        assert_eq!(cmd, "rm -rf '/sdcard/my folder/file name.txt'");
    }

    #[test]
    fn push_pull_argv() {
        let op = DeviceOperation::Push {
            local: "/home/user/app.apk".into(),
            remote: "/sdcard/Download/app.apk".into(),
        };
        let args = op.to_adb_args(Some("S1")).unwrap();
        assert_eq!(
            args,
            vec![
                "-s",
                "S1",
                "push",
                "/home/user/app.apk",
                "/sdcard/Download/app.apk"
            ]
        );
    }

    #[test]
    fn install_apk_argv_and_policy() {
        let op = DeviceOperation::InstallApk {
            local: "/home/user/app-release.apk".into(),
        };
        let args = op.to_adb_args(Some("S1")).unwrap();
        assert_eq!(
            args,
            vec!["-s", "S1", "install", "-r", "/home/user/app-release.apk"]
        );
        // Installing is not on the destructive list (reversible via
        // uninstall-for-user); it must NOT require a typed word.
        assert!(!op.is_destructive());
        assert_eq!(op.required_confirmation(), None);

        let bad = DeviceOperation::InstallApk {
            local: "/tmp/app\x00.apk".into(),
        };
        assert!(bad.validate().is_err());
    }
}
