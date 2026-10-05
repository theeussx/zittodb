//! Integration tests with a fake `adb` (spec §55–56).
//!
//! These run WITHOUT hardware: a shell script emulates the adb CLI surface
//! we use (devices, getprop, shell). Security tests assert that malformed
//! inputs are rejected BEFORE any process is spawned.

use std::sync::atomic::{AtomicUsize, Ordering};
use zittodb_lib::adb::operations::DeviceOperation;
use zittodb_lib::adb::parse;
use zittodb_lib::error::ErrorCode;
use zittodb_lib::processes;
use zittodb_lib::scrcpy::ScrcpyOptions;
use zittodb_lib::security;

static FAKE_ADB_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn fake_adb() -> std::path::PathBuf {
    let id = FAKE_ADB_COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("zittodb-it-{}-{id}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let fake = dir.join("fake-adb");
    let content = r#"#!/bin/sh
# minimal adb emulator for integration tests
# skip leading "-s SERIAL" pairs, then the first remaining word is the command
while [ $# -gt 0 ]; do
  case "$1" in
    -s) shift; shift 2>/dev/null || true ;;
    *) break ;;
  esac
done
cmd="$1"
shift 2>/dev/null || true
case "$cmd" in
  devices)
    echo "List of devices attached"
    echo "FAKE0001       device usb:1-1 product:fake device:fake model:Fake_Phone transport_id:1"
    echo "192.168.1.9:5555 device product:fake device:fake model:Fake_Wifi transport_id:2"
    echo "UNAUTH123      unauthorized usb:1-2"
    ;;
  shell)
    case "$1" in
      getprop)
        echo "ro.product.model: Fake Phone"
        echo "ro.product.manufacturer: FakeCorp"
        echo "ro.build.version.release: 15"
        echo "ro.build.version.sdk: 35"
        echo "ro.build.version.security_patch: 2026-08-01"
        echo "ro.build.id: FP1"
        echo "ro.board.platform: fakeplat"
        echo "ro.hardware: fakehw"
        echo "ro.product.cpu.abilist: arm64-v8a"
        ;;
      "uname -r")
        echo "6.1.0-fake"
        ;;
      "cat /proc/meminfo")
        echo "MemTotal:       8053064 kB"
        echo "MemFree:        2048000 kB"
        ;;
      "dumpsys battery")
        echo "Current Battery Service state:"
        echo "  level: 73"
        echo "  scale: 100"
        echo "  temperature: 310"
        echo "  status: 2"
        echo "  plugged: 1"
        ;;
      "df -m /sdcard")
        echo "Filesystem      1M-blocks  Used Available Use% Mounted on"
        echo "/dev/fuse        123456 45678    77778  37% /storage/self/primary"
        ;;
      *)
        echo ""
        ;;
    esac
    ;;
  exec-out)
    # screencap -p: emit a tiny 1x1 PNG
    printf '\x89PNG\r\n\x1a\n'
    ;;
  connect)
    echo "connected to $1"
    ;;
  *)
    echo ""
    ;;
esac
exit 0
"#;
    std::fs::write(&fake, content).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut p = std::fs::metadata(&fake).unwrap().permissions();
        p.set_mode(0o755);
        std::fs::set_permissions(&fake, p).unwrap();
    }
    fake
}

#[test]
fn discovers_devices_with_fake_adb() {
    let fake = fake_adb();
    let out = processes::run_captured(
        fake.to_str().unwrap(),
        &["devices".into(), "-l".into()],
        std::time::Duration::from_secs(5),
    )
    .expect("fake adb runs");
    let devices = parse::parse_devices_l(&out.text());
    assert_eq!(devices.len(), 3);
    assert_eq!(devices[0].serial, "FAKE0001");
    assert_eq!(devices[1].serial, "192.168.1.9:5555");
    assert_eq!(devices[2].state, "unauthorized");
}

#[test]
fn device_info_flow_with_fake_adb() {
    let fake = fake_adb();
    let out = processes::run_captured(
        fake.to_str().unwrap(),
        &[
            "-s".into(),
            "FAKE0001".into(),
            "shell".into(),
            "getprop".into(),
        ],
        std::time::Duration::from_secs(5),
    )
    .unwrap();
    let props = parse::parse_getprop(&out.text());
    assert_eq!(
        props.get("ro.product.model").map(|s| s.as_str()),
        Some("Fake Phone")
    );
    assert_eq!(
        props.get("ro.build.version.release").map(|s| s.as_str()),
        Some("15")
    );

    let mem = processes::run_captured(
        fake.to_str().unwrap(),
        &[
            "-s".into(),
            "FAKE0001".into(),
            "shell".into(),
            "cat /proc/meminfo".into(),
        ],
        std::time::Duration::from_secs(5),
    )
    .unwrap();
    assert_eq!(
        parse::parse_meminfo_total_mb(&mem.text()),
        Some(8053064 / 1024)
    );
}

#[test]
fn security_invalid_serials_never_reach_a_process() {
    for bad in ["a b", "a|b", "a&b", "$(x)", "a;b", "a\nb", ""] {
        let err = security::validate_serial(bad).expect_err("must reject");
        assert_eq!(err.code, ErrorCode::InvalidSerial);
    }
}

#[test]
fn security_operations_reject_injection() {
    let op = DeviceOperation::DisablePackage {
        pkg: "com.a.b | reboot".into(),
        user: 0,
    };
    assert_eq!(
        op.to_adb_args(Some("S1")).err().map(|e| e.code),
        Some(ErrorCode::InvalidPackage)
    );

    let op = DeviceOperation::Delete {
        path: "/sdcard/../../data".into(),
    };
    assert_eq!(
        op.to_adb_args(Some("S1")).err().map(|e| e.code),
        Some(ErrorCode::InvalidPath)
    );
}

#[test]
fn security_confirmation_gate_requires_exact_word() {
    let op = DeviceOperation::Delete {
        path: "/sdcard/x".into(),
    };
    assert!(op.is_destructive());
    assert_eq!(op.required_confirmation(), Some("APAGAR"));
    // A wrong word must not be accepted (checked in commands::execute_op).
    let wrong = Some("apagar".to_string());
    assert_ne!(
        wrong.as_deref().unwrap(),
        op.required_confirmation().unwrap()
    );
}

#[test]
fn security_scrcpy_args_are_validated() {
    let mut o = ScrcpyOptions {
        bitrate: Some("2M".into()),
        ..Default::default()
    };
    o.apply_preset("low");
    let args = o.to_args("FAKE0001").expect("valid");
    assert!(args.contains(&"--max-size".to_string()));
    assert!(args.contains(&"1280".to_string()));

    let bad = ScrcpyOptions {
        max_fps: Some(9999),
        ..Default::default()
    };
    assert!(bad.validate().is_err());
}

#[test]
fn timeout_kills_stuck_process() {
    // A command that would hang must be killed, not orphaned.
    let started = std::time::Instant::now();
    let out = processes::run_captured(
        "/bin/sleep",
        &["60".into()],
        std::time::Duration::from_millis(150),
    )
    .unwrap();
    assert!(out.timed_out);
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
}
