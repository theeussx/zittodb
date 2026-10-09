//! Fastboot module (spec §52–53).
//!
//! Separate from the ADB device flow. Destructive operations
//! (flash / erase / unlock / lock) require a typed confirmation word —
//! a single click is never enough.

use serde::{Deserialize, Serialize};

use crate::adb::client::AdbClient;
use crate::adb::operations::RebootTarget;
use crate::error::AppError;
use crate::processes::Captured;
use crate::security::{validate_getvar, validate_local_path, validate_partition, validate_serial};
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FastbootDevice {
    pub serial: String,
    pub state: String,
}

pub fn list_devices(fastboot: &str) -> Result<Vec<FastbootDevice>, AppError> {
    let client = AdbClient::new(fastboot);
    let out = client.run_ok(vec!["devices".into()], Duration::from_secs(15))?;
    Ok(parse_devices(&out))
}

/// `fastboot devices` lines: `SERIAL  STATE` (state may contain spaces).
pub fn parse_devices(text: &str) -> Vec<FastbootDevice> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("<") {
            // skip warnings like "< waiting for device >"
            continue;
        }
        let mut parts = line.splitn(2, char::is_whitespace);
        let serial = parts.next().unwrap_or("").to_string();
        let state = parts.next().unwrap_or("").trim().to_string();
        if serial.is_empty() {
            continue;
        }
        out.push(FastbootDevice { serial, state });
    }
    out
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case", tag = "op")]
pub enum FastbootOperation {
    Devices,
    Reboot {
        target: RebootTarget,
    },
    Getvar {
        var: String,
    },
    /// `fastboot flash <partition> <file>`
    Flash {
        partition: String,
        file: String,
    },
    /// `fastboot erase <partition>`
    Erase {
        partition: String,
    },
    /// `fastboot oem unlock` — requires bootloader unlocked state on device.
    Unlock,
    /// `fastboot oem lock`
    Lock,
}

impl FastbootOperation {
    pub fn name(&self) -> &'static str {
        use FastbootOperation as O;
        match self {
            O::Devices => "devices",
            O::Reboot { .. } => "reboot",
            O::Getvar { .. } => "getvar",
            O::Flash { .. } => "flash",
            O::Erase { .. } => "erase",
            O::Unlock => "unlock",
            O::Lock => "lock",
        }
    }

    /// flash/erase/unlock/lock can destroy data (spec §53).
    pub fn is_destructive(&self) -> bool {
        matches!(
            self,
            FastbootOperation::Flash { .. }
                | FastbootOperation::Erase { .. }
                | FastbootOperation::Unlock
                | FastbootOperation::Lock
        )
    }

    pub fn required_confirmation(&self) -> Option<&'static str> {
        match self {
            FastbootOperation::Flash { .. } => Some("FLASHAR"),
            FastbootOperation::Erase { .. } => Some("APAGAR"),
            FastbootOperation::Unlock | FastbootOperation::Lock => Some("APAGAR"),
            _ => None,
        }
    }

    pub fn validate(&self) -> Result<(), AppError> {
        use FastbootOperation as O;
        match self {
            O::Flash { partition, file } => {
                validate_partition(partition)?;
                validate_local_path(file)
            }
            O::Erase { partition } => validate_partition(partition),
            O::Getvar { var } => validate_getvar(var),
            _ => Ok(()),
        }
    }

    pub fn to_args(&self) -> Result<Vec<String>, AppError> {
        self.to_args_for_serial(None)
    }

    pub fn to_args_for_serial(&self, serial: Option<&str>) -> Result<Vec<String>, AppError> {
        self.validate()?;
        let prefix = match serial {
            Some(serial) => {
                validate_serial(serial)?;
                vec!["-s".into(), serial.into()]
            }
            None => Vec::new(),
        };
        use FastbootOperation as O;
        let operation = match self {
            O::Devices => vec!["devices".into()],
            O::Reboot { target } => {
                if *target == RebootTarget::System {
                    vec!["reboot".into()]
                } else {
                    vec!["reboot".into(), target.as_str().into()]
                }
            }
            O::Getvar { var } => vec!["getvar".into(), var.clone()],
            O::Flash { partition, file } => vec!["flash".into(), partition.clone(), file.clone()],
            O::Erase { partition } => vec!["erase".into(), partition.clone()],
            O::Unlock => vec!["oem".into(), "unlock".into()],
            O::Lock => vec!["oem".into(), "lock".into()],
        };
        Ok(prefix.into_iter().chain(operation).collect())
    }

    pub fn run(&self, fastboot: &str) -> Result<Captured, AppError> {
        self.run_on_device(fastboot, None)
    }

    pub fn run_on_device(
        &self,
        fastboot: &str,
        serial: Option<&str>,
    ) -> Result<Captured, AppError> {
        let client = AdbClient::new(fastboot);
        let timeout = match self {
            FastbootOperation::Flash { .. } => Duration::from_secs(600),
            FastbootOperation::Erase { .. } => Duration::from_secs(120),
            _ => Duration::from_secs(30),
        };
        client.run(self.to_args_for_serial(serial)?, timeout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_devices() {
        let v = parse_devices(
            "ABC123    FASTBOOT\n\nDEF456    loader (waiting)\n< waiting for device >\n",
        );
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].serial, "ABC123");
        assert_eq!(v[0].state, "FASTBOOT");
        assert_eq!(v[1].state, "loader (waiting)");
    }

    #[test]
    fn destructive_flags() {
        assert!(FastbootOperation::Erase {
            partition: "cache".into()
        }
        .is_destructive());
        assert!(FastbootOperation::Flash {
            partition: "boot".into(),
            file: "/tmp/boot.img".into()
        }
        .is_destructive());
        assert!(FastbootOperation::Unlock.is_destructive());
        assert!(!FastbootOperation::Reboot {
            target: RebootTarget::System
        }
        .is_destructive());
        assert!(!FastbootOperation::Getvar {
            var: "product".into()
        }
        .is_destructive());

        assert_eq!(
            FastbootOperation::Erase {
                partition: "cache".into()
            }
            .required_confirmation(),
            Some("APAGAR")
        );
        assert_eq!(
            FastbootOperation::Flash {
                partition: "boot".into(),
                file: "/tmp/x".into()
            }
            .required_confirmation(),
            Some("FLASHAR")
        );
    }

    #[test]
    fn argv_and_validation() {
        let op = FastbootOperation::Flash {
            partition: "boot".into(),
            file: "/tmp/boot.img".into(),
        };
        assert_eq!(
            op.to_args().unwrap(),
            vec!["flash", "boot", "/tmp/boot.img"]
        );

        let bad = FastbootOperation::Erase {
            partition: "boot;reboot".into(),
        };
        assert!(bad.to_args().is_err());

        let bad = FastbootOperation::Getvar { var: "-v".into() };
        assert!(bad.to_args().is_err());
    }

    #[test]
    fn selected_serial_is_included_in_argv() {
        let op = FastbootOperation::Getvar {
            var: "product".into(),
        };
        assert_eq!(
            op.to_args_for_serial(Some("FAKEFB01")).unwrap(),
            vec!["-s", "FAKEFB01", "getvar", "product"]
        );
    }
}
