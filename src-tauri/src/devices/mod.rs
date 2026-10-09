//! Device discovery + information (spec §10–13, §31–34).
//!
//! - `adb devices -l` for the list (USB, Wi-Fi, emulators, offline,
//!   unauthorized — all states are surfaced, never hidden).
//! - Information is pulled from the device on demand (no background metric
//!   collection, spec §13).
//! - Ambiguity rule: operations need an explicit serial. When the frontend
//!   passes none, exactly ONE connected device is resolved; otherwise
//!   `AMBIGUOUS_DEVICE` is returned (spec §11).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;

use crate::adb::client::{AdbClient, TIMEOUT_DEVICE, TIMEOUT_DUMP, TIMEOUT_MEDIA, TIMEOUT_PROPS};
use crate::adb::parse;
use crate::error::{AppError, ErrorCode};
use crate::security::validate_serial;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceState {
    Connected,
    Offline,
    Unauthorized,
    Recovery,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionKind {
    Usb,
    Wifi,
    Emulator,
    Unknown,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub serial: String,
    pub state: DeviceState,
    pub model: Option<String>,
    pub product: Option<String>,
    pub device: Option<String>,
    pub connection: ConnectionKind,
    pub is_emulator: bool,
}

pub fn list_devices(adb: &str) -> Result<Vec<Device>, AppError> {
    let client = AdbClient::new(adb);
    let out = client.run_ok(vec!["devices".into(), "-l".into()], Duration::from_secs(10))?;
    Ok(parse::parse_devices_l(&out)
        .into_iter()
        .map(to_device)
        .collect())
}

pub fn to_device(raw: parse::RawDevice) -> Device {
    let state = match raw.state.as_str() {
        "device" => DeviceState::Connected,
        "offline" => DeviceState::Offline,
        "unauthorized" => DeviceState::Unauthorized,
        "recovery" | "sideload" | "bootloader" => DeviceState::Recovery,
        _ => DeviceState::Other,
    };
    let is_emulator = raw.serial.starts_with("emulator-");
    let connection = if is_emulator {
        ConnectionKind::Emulator
    } else if raw.serial.contains(':') {
        ConnectionKind::Wifi
    } else if raw.attrs.contains_key("usb") {
        ConnectionKind::Usb
    } else {
        ConnectionKind::Unknown
    };
    Device {
        serial: raw.serial,
        state,
        model: raw.attrs.get("model").cloned(),
        product: raw.attrs.get("product").cloned(),
        device: raw.attrs.get("device").cloned(),
        connection,
        is_emulator,
    }
}

/// Resolves a serial from the frontend request.
/// `None` means "the selected device" — allowed only when there is exactly
/// one connected device (never guess with several attached).
pub fn resolve_serial(requested: Option<&str>, devices: &[Device]) -> Result<String, AppError> {
    if let Some(s) = requested {
        validate_serial(s)?;
        return Ok(s.to_string());
    }
    let connected: Vec<&Device> = devices
        .iter()
        .filter(|d| d.state == DeviceState::Connected)
        .collect();
    match connected.len() {
        0 => Err(AppError::new(ErrorCode::NoDevice, "no connected device")),
        1 => Ok(connected[0].serial.clone()),
        _ => Err(AppError::new(
            ErrorCode::AmbiguousDevice,
            format!(
                "multiple devices connected ({}) — select one",
                connected
                    .iter()
                    .map(|d| d.serial.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )),
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub serial: String,
    pub model: Option<String>,
    pub manufacturer: Option<String>,
    pub brand: Option<String>,
    pub android_version: Option<String>,
    pub sdk_version: Option<String>,
    pub security_patch: Option<String>,
    pub build_id: Option<String>,
    pub build_display: Option<String>,
    pub kernel: Option<String>,
    pub cpu_abis: Option<String>,
    pub board_platform: Option<String>,
    pub hardware: Option<String>,
    pub product: Option<String>,
    pub device: Option<String>,
    pub total_ram_mb: Option<u64>,
}

pub fn get_device_info(adb: &str, serial: &str) -> Result<DeviceInfo, AppError> {
    validate_serial(serial)?;
    let client = AdbClient::new(adb);

    let props_out = client.run_ok(
        vec!["-s".into(), serial.into(), "shell".into(), "getprop".into()],
        TIMEOUT_PROPS,
    )?;
    let props = parse::parse_getprop(&props_out);
    let get = |k: &str| props.get(k).cloned();

    let kernel = client
        .run_shell(serial, "uname -r")
        .ok()
        .map(|c| c.text().trim().to_string())
        .filter(|s| !s.is_empty());

    let total_ram_mb = client
        .run_shell(serial, "cat /proc/meminfo")
        .ok()
        .and_then(|c| parse::parse_meminfo_total_mb(&c.text()));

    Ok(DeviceInfo {
        serial: serial.to_string(),
        model: get("ro.product.model"),
        manufacturer: get("ro.product.manufacturer"),
        brand: get("ro.product.brand"),
        android_version: get("ro.build.version.release"),
        sdk_version: get("ro.build.version.sdk"),
        security_patch: get("ro.build.version.security_patch"),
        build_id: get("ro.build.id"),
        build_display: get("ro.build.display.id"),
        kernel,
        cpu_abis: get("ro.product.cpu.abilist"),
        board_platform: get("ro.board.platform"),
        hardware: get("ro.hardware"),
        product: get("ro.product.name"),
        device: get("ro.product.device"),
        total_ram_mb,
    })
}

pub fn get_battery(adb: &str, serial: &str) -> Result<parse::BatteryInfo, AppError> {
    validate_serial(serial)?;
    let client = AdbClient::new(adb);
    let out = client.run_shell(serial, "dumpsys battery")?;
    if !out.success() {
        return Err(AppError::from_process(ErrorCode::ProcessFailed, adb, &out));
    }
    Ok(parse::parse_dumpsys_battery(&out.text()))
}

pub fn get_storage(adb: &str, serial: &str) -> Result<parse::DiskUsage, AppError> {
    validate_serial(serial)?;
    let client = AdbClient::new(adb);
    // Android vendors expose shared storage under different aliases. Try
    // both the public-storage paths and /data, keeping the first parseable
    // result. `df -k` is preferred because it is supported by toybox; the
    // final `df -m` fallback handles older vendor shells.
    let candidates = [
        ("df -k /sdcard", "/sdcard"),
        ("df -k /storage/emulated/0", "/storage/emulated/0"),
        ("df -k /data", "/data"),
        ("df -m /sdcard", "/sdcard"),
    ];
    let mut last_output = String::new();
    for (command, path) in candidates {
        let out = client.run_shell(serial, command)?;
        last_output = out.text();
        if out.success() {
            if let Some(usage) = parse::parse_df(&last_output, path) {
                return Ok(usage);
            }
        }
    }
    Err(AppError::new(
        ErrorCode::ProcessFailed,
        format!("could not parse df output: {}", last_output.trim()),
    ))
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkInfo {
    pub interfaces: Vec<parse::NetIface>,
    pub gateway: Option<String>,
    pub dns: Vec<String>,
    pub mac: Option<String>,
}

pub fn get_network_info(adb: &str, serial: &str) -> Result<NetworkInfo, AppError> {
    validate_serial(serial)?;
    let client = AdbClient::new(adb);
    let mut info = NetworkInfo::default();

    if let Ok(out) = client.run_shell(serial, "ip addr show") {
        if out.success() {
            info.interfaces = parse::parse_ip_addr(&out.text());
        }
    }
    if let Ok(out) = client.run_shell(serial, "ip route") {
        if out.success() {
            info.gateway = parse::parse_ip_route(&out.text());
        }
    }
    if let Ok(out) = client.run_shell(serial, "cat /etc/resolv.conf") {
        if out.success() {
            info.dns = parse::parse_resolv(&out.text());
        }
    }
    // MAC: read from the first non-loopback interface (may be blocked).
    if let Some(iface) = info.interfaces.first() {
        let n = iface.name.clone();
        if let Ok(out) = client.run_shell(&serial, &format!("cat /sys/class/net/{n}/address")) {
            if out.success() {
                let mac = out.text().trim().to_string();
                if !mac.is_empty() && mac.len() >= 8 {
                    info.mac = Some(mac);
                }
            }
        }
    }
    Ok(info)
}

/// Raw `getprop` map (Diagnostics view).
pub fn get_props(
    adb: &str,
    serial: &str,
) -> Result<std::collections::BTreeMap<String, String>, AppError> {
    validate_serial(serial)?;
    let client = AdbClient::new(adb);
    let out = client.run_ok(
        vec!["-s".into(), serial.into(), "shell".into(), "getprop".into()],
        TIMEOUT_PROPS,
    )?;
    Ok(parse::parse_getprop(&out))
}

/// Serializes potentially conflicting operations per device (spec §46).
/// Read-only operations are NOT locked; destructive/long ones are.
pub struct Coordinator {
    locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
}

impl Coordinator {
    pub fn new() -> Coordinator {
        Coordinator {
            locks: Mutex::new(HashMap::new()),
        }
    }

    pub fn lock_for(&self, serial: &str) -> Arc<Mutex<()>> {
        let mut map = self.locks.lock().unwrap();
        map.entry(serial.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }
}

impl Default for Coordinator {
    fn default() -> Self {
        Self::new()
    }
}

pub use crate::adb::parse::parse_package_dump;
pub use crate::adb::parse::PackageMeta;

pub fn get_package_dump(adb: &str, serial: &str, pkg: &str) -> Result<PackageMeta, AppError> {
    validate_serial(serial)?;
    crate::security::validate_package(pkg)?;
    let client = AdbClient::new(adb);
    let out = client.run_shell(serial, &format!("dumpsys package '{pkg}'"))?;
    if !out.success() {
        return Err(AppError::from_process(ErrorCode::NoDevice, adb, &out));
    }
    parse_package_dump(&out.text()).ok_or_else(|| {
        AppError::new(
            ErrorCode::ProcessFailed,
            format!("package not found in dumpsys: {pkg}"),
        )
    })
}

/// Full package metadata in a single `dumpsys package` pass (spec §5:
/// one transfer instead of one per app — lazy by design, not polled).
pub fn list_all_packages(
    adb: &str,
    serial: &str,
) -> Result<Vec<crate::adb::parse::PackageMeta>, AppError> {
    validate_serial(serial)?;
    let client = AdbClient::new(adb);
    let out = client.run_ok(
        vec![
            "-s".into(),
            serial.into(),
            "shell".into(),
            "dumpsys package".into(),
        ],
        TIMEOUT_DUMP,
    )?;
    let mut it = crate::adb::parse::package_parser();
    for line in out.lines() {
        it.feed(line);
    }
    Ok(it.finish())
}

pub fn third_party_packages(adb: &str, serial: &str) -> Result<Vec<String>, AppError> {
    validate_serial(serial)?;
    let client = AdbClient::new(adb);
    let out = client.run_shell(serial, "pm list packages -3")?;
    if !out.success() {
        return Err(AppError::from_process(ErrorCode::ProcessFailed, adb, &out));
    }
    Ok(parse::parse_pm_packages(&out.text()))
}

pub fn disabled_packages(adb: &str, serial: &str) -> Result<Vec<String>, AppError> {
    validate_serial(serial)?;
    let client = AdbClient::new(adb);
    let out = client.run_shell(serial, "pm list packages -d")?;
    if !out.success() {
        return Err(AppError::from_process(ErrorCode::ProcessFailed, adb, &out));
    }
    Ok(parse::parse_pm_packages(&out.text()))
}

/// Screenshot: binary stdout from `adb exec-out screencap -p`.
pub fn screenshot_bytes(adb: &str, serial: &str) -> Result<Vec<u8>, AppError> {
    validate_serial(serial)?;
    let client = AdbClient::new(adb);
    let out = client.run(
        vec![
            "-s".into(),
            serial.into(),
            "exec-out".into(),
            "screencap".into(),
            "-p".into(),
        ],
        TIMEOUT_MEDIA,
    )?;
    if !out.success() {
        return Err(AppError::from_process(ErrorCode::ProcessFailed, adb, &out));
    }
    if out.stdout.len() < 8 || &out.stdout[1..4] != b"PNG" {
        return Err(AppError::new(
            ErrorCode::Unexpected,
            "screencap did not return a PNG",
        ));
    }
    Ok(out.stdout)
}

/// `adb connect host:port`
pub fn adb_connect(adb: &str, host: &str, port: u16) -> Result<String, AppError> {
    crate::security::validate_host(host)?;
    let client = AdbClient::new(adb);
    let out = client.run(
        vec!["connect".into(), format!("{host}:{port}")],
        TIMEOUT_DEVICE,
    )?;
    Ok(out.text().trim().to_string())
}

/// `adb disconnect host:port`
pub fn adb_disconnect(adb: &str, host: &str, port: u16) -> Result<String, AppError> {
    crate::security::validate_host(host)?;
    let client = AdbClient::new(adb);
    let out = client.run(
        vec!["disconnect".into(), format!("{host}:{port}")],
        TIMEOUT_DEVICE,
    )?;
    Ok(out.text().trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_state_mapping() {
        let raw = parse::RawDevice {
            serial: "S1".into(),
            state: "device".into(),
            attrs: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("model".into(), "Redmi_Note_12".into());
                m
            },
        };
        let d = to_device(raw);
        assert_eq!(d.state, DeviceState::Connected);
        assert_eq!(d.model.as_deref(), Some("Redmi_Note_12"));
    }

    #[test]
    fn wifi_and_emulator_detection() {
        let raw = parse::RawDevice {
            serial: "192.168.1.10:5555".into(),
            state: "device".into(),
            attrs: std::collections::BTreeMap::new(),
        };
        assert_eq!(to_device(raw).connection, ConnectionKind::Wifi);

        let raw = parse::RawDevice {
            serial: "emulator-5554".into(),
            state: "device".into(),
            attrs: std::collections::BTreeMap::new(),
        };
        let d = to_device(raw);
        assert!(d.is_emulator);
        assert_eq!(d.connection, ConnectionKind::Emulator);
    }

    #[test]
    fn serial_resolution_ambiguity() {
        let d = |serial: &str| Device {
            serial: serial.into(),
            state: DeviceState::Connected,
            model: None,
            product: None,
            device: None,
            connection: ConnectionKind::Usb,
            is_emulator: false,
        };
        // none requested + none connected
        assert!(matches!(
            resolve_serial(None, &[]),
            Err(e) if e.code == ErrorCode::NoDevice
        ));
        // none requested + one connected -> ok
        let one = vec![d("A1")];
        assert_eq!(resolve_serial(None, &one).unwrap(), "A1");
        // none requested + two connected -> ambiguous
        let two = vec![d("A1"), d("B2")];
        assert!(matches!(
            resolve_serial(None, &two),
            Err(e) if e.code == ErrorCode::AmbiguousDevice
        ));
        // explicit serial always wins (validated)
        assert_eq!(resolve_serial(Some("A1"), &two).unwrap(), "A1");
        assert!(resolve_serial(Some("bad serial"), &two).is_err());
    }

    #[test]
    fn coordinator_locks_per_serial() {
        let c = Coordinator::new();
        let l1 = c.lock_for("A");
        let l2 = c.lock_for("A");
        let l3 = c.lock_for("B");
        let g1 = l1.lock().unwrap();
        // same serial: second lock blocks -> try_lock fails
        assert!(l2.try_lock().is_err());
        // different serial: independent
        assert!(l3.try_lock().is_ok());
        drop(g1);
    }
}
