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

#[test]
fn security_delete_and_rename_refuse_protected_roots() {
    for path in [
        "/",
        "/sdcard",
        "/sdcard/",
        "//storage//emulated/0",
        "/data",
        "/proc/1",
    ] {
        let op = DeviceOperation::Delete { path: path.into() };
        assert_eq!(
            op.to_adb_args(Some("S1")).err().map(|e| e.code),
            Some(ErrorCode::OperationRejected),
            "Delete {path:?} must be rejected even before confirmation is checked"
        );
    }
    let op = DeviceOperation::Rename {
        from: "/sdcard".into(),
        to: "/sdcard2".into(),
    };
    assert_eq!(
        op.to_adb_args(Some("S1")).err().map(|e| e.code),
        Some(ErrorCode::OperationRejected)
    );
}

#[test]
fn security_delete_inside_protected_roots_still_builds_argv() {
    let op = DeviceOperation::Delete {
        path: "/sdcard/DCIM/Camera/IMG 1.jpg".into(),
    };
    let args = op
        .to_adb_args(Some("S1"))
        .expect("regular file is deletable");
    assert_eq!(args[..3], ["-s", "S1", "shell"]);
    assert_eq!(args[3], "rm -rf '/sdcard/DCIM/Camera/IMG 1.jpg'");
}

// ---------------------------------------------------------------------------
// Scanner Center: real collection path against a fake adb
// ---------------------------------------------------------------------------

fn scanner_fake_adb(tag: &str, extra_cases: &str) -> std::path::PathBuf {
    let id = FAKE_ADB_COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("zittodb-scan-{tag}-{}-{id}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let fake = dir.join("adb");
    let script = format!(
        r#"#!/bin/sh
while [ $# -gt 0 ]; do
  case "$1" in
    -s) shift; shift 2>/dev/null || true ;;
    *) break ;;
  esac
done
cmd="$1"
shift 2>/dev/null || true
case "$cmd" in
  version)
    echo "Android Debug Bridge version 1.0.41"
    echo "Version 34.0.5-10900879"
    echo "Installed as $0"
    ;;
  server-status)
    echo "adb: unknown command server-status" >&2
    exit 1
    ;;
  devices)
    echo "List of devices attached"
    echo "RISKY001       device usb:1-1 product:p device:d model:Risky_Phone transport_id:1"
    echo "UNAUTH123      unauthorized usb:1-2"
    ;;
  shell)
    case "$1" in
      getprop)
        echo "[ro.build.version.security_patch]: [2023-01-01]"
        echo "[ro.build.version.sdk]: [29]"
        echo "[ro.debuggable]: [1]"
        echo "[ro.build.type]: [userdebug]"
        echo "[ro.boot.flash.locked]: [0]"
        echo "[ro.boot.verifiedbootstate]: [orange]"
        echo "[service.adb.tcp.port]: [5555]"
        ;;
      "dumpsys battery")
        echo "Current Battery Service state:"
        echo "  level: 50"
        echo "  scale: 100"
        echo "  health: 3"
        echo "  temperature: 478"
        ;;
      df*)
        echo "Filesystem       1M-blocks  Used Available Use% Mounted on"
        echo "/dev/block/dm-9     100000 97000      3000  97% /data"
        ;;
      getenforce)
        echo "Permissive"
        ;;
      *)
        echo "fake: unhandled shell $1" >&2
        exit 1
        ;;
    esac
    ;;
  {extra_cases}
  *)
    echo "fake: unknown $cmd" >&2
    exit 1
    ;;
esac
exit 0
"#
    );
    std::fs::write(&fake, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    fake
}

#[test]
fn scanner_device_scan_collects_through_the_allowlist_and_reports_evidence() {
    use zittodb_lib::scanner::{self, model::Severity};
    let fake = scanner_fake_adb("dev", "");
    let report = scanner::run_device_scan(fake.to_str().unwrap(), "RISKY001", "en-US").unwrap();

    let rules: Vec<&str> = report.findings.iter().map(|f| f.rule_id.as_str()).collect();
    for want in [
        "DEV-PATCH-OLD",
        "DEV-ADB-TCP",
        "DEV-BOOTLOADER-UNLOCKED",
        "DEV-DEBUGGABLE-BUILD",
        "DEV-SELINUX-PERMISSIVE",
        "DEV-LOW-STORAGE",
        "DEV-BATTERY-HEALTH",
        "DEV-BATTERY-HOT",
        "DEV-SCRCPY-LIMITS",
    ] {
        assert!(rules.contains(&want), "{want} missing in {rules:?}");
    }
    // Highest severity first, every finding carries evidence and a fix.
    assert_eq!(report.findings[0].severity, Severity::High);
    assert!(report
        .findings
        .iter()
        .all(|f| !f.evidence.is_empty() && !f.recommendation.is_empty()));
    // Every declared check was attempted and none was silently dropped.
    assert_eq!(
        report.checks.len(),
        zittodb_lib::scanner::device::DEVICE_CHECKS.len()
    );
    assert_eq!(report.summary.checks_not_evaluated, 0);
    assert!(!report.limitations.is_empty());
    // Same input → same ids (comparable across runs).
    let again = scanner::run_device_scan(fake.to_str().unwrap(), "RISKY001", "pt-BR").unwrap();
    let ids = |r: &scanner::model::ScanReport| {
        r.findings.iter().map(|f| f.id.clone()).collect::<Vec<_>>()
    };
    assert_eq!(ids(&report), ids(&again));
    // And the comparison of a scan with itself has no changes.
    let d = scanner::model::diff_reports(&report, &again);
    assert!(d.new.is_empty() && d.resolved.is_empty() && d.changed.is_empty());
    assert_eq!(d.unchanged as usize, report.findings.len());
}

#[test]
fn scanner_device_scan_unauthorized_and_unknown_serials() {
    use zittodb_lib::scanner;
    let fake = scanner_fake_adb("unauth", "");
    let r = scanner::run_device_scan(fake.to_str().unwrap(), "UNAUTH123", "en-US").unwrap();
    assert_eq!(r.findings.len(), 1);
    assert_eq!(r.findings[0].rule_id, "DEV-UNAUTHORIZED");
    assert_eq!(
        r.summary.checks_evaluated, 1,
        "only the connection check was evaluated"
    );
    assert_eq!(
        r.summary.checks_not_evaluated as usize,
        zittodb_lib::scanner::device::DEVICE_CHECKS.len() - 1
    );
    let err = scanner::run_device_scan(fake.to_str().unwrap(), "GHOST", "en-US").unwrap_err();
    assert_eq!(err.code, ErrorCode::NoDevice);
    let err =
        scanner::run_device_scan(fake.to_str().unwrap(), "bad serial; rm", "en-US").unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidSerial);
}

#[test]
fn scanner_environment_scan_runs_the_real_adb_probes() {
    use zittodb_lib::adb::ToolStatus;
    use zittodb_lib::scanner;
    let fake = scanner_fake_adb("env", "");
    let tools = vec![
        ToolStatus {
            name: "adb".into(),
            found: true,
            path: Some(fake.to_string_lossy().into()),
            version: None,
            source: None,
        },
        ToolStatus {
            name: "fastboot".into(),
            found: false,
            path: None,
            version: None,
            source: None,
        },
        ToolStatus {
            name: "scrcpy".into(),
            found: false,
            path: None,
            version: None,
            source: None,
        },
    ];
    let path = std::env::join_paths([fake.parent().unwrap()]).unwrap();
    let r = scanner::run_environment_scan(&tools, Some(path.as_os_str()), "en-US");
    let rules: Vec<&str> = r.findings.iter().map(|f| f.rule_id.as_str()).collect();
    assert!(rules.contains(&"ENV-ADB-VERSION-OLD"), "{rules:?}"); // 34.0.5 < 37.0.0
    assert!(rules.contains(&"ENV-FASTBOOT-MISSING"));
    assert!(rules.contains(&"ENV-SCRCPY-MISSING"));
    assert!(!rules.contains(&"ENV-ADB-MISSING"));
    // `adb server-status` exited non-zero → reported as NOT evaluated, not as a pass.
    let srv = r.checks.iter().find(|c| c.id == "env.adb_server").unwrap();
    assert_eq!(
        srv.status,
        zittodb_lib::scanner::model::CheckStatus::Skipped
    );
    assert!(srv
        .detail
        .as_deref()
        .unwrap_or("")
        .contains("unknown command"));
}
