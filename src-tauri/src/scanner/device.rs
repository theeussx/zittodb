//! Device scan: rules over a [`DeviceSnapshot`].
//!
//! Split in two on purpose:
//! * [`collect_device`] reads the device with allowlisted READ-ONLY operations
//!   only (it never changes anything on the device);
//! * [`evaluate_device`] is a pure function over the snapshot, so every rule is
//!   unit-tested without a device.
//!
//! The rules only look at what the device reports through `getprop`,
//! `dumpsys battery`, `df` and `getenforce`. They do not inspect installed
//! apps, kernel integrity or network exposure beyond adbd's TCP port.

use std::collections::BTreeMap;

use crate::adb::client::{AdbClient, TIMEOUT_DEVICE, TIMEOUT_PROPS};
use crate::adb::operations::DeviceOperation;
use crate::adb::parse::{self, BatteryInfo, DiskUsage};
use crate::devices::{ConnectionKind, DeviceState};
use crate::error::AppError;

use super::model::{
    CheckResult, CheckStatus, Confidence, Evidence, Finding, Lang, ScanTarget, Severity,
};

#[derive(Debug, Clone)]
pub struct DeviceSnapshot {
    pub serial: String,
    pub state: DeviceState,
    pub connection: ConnectionKind,
    pub props: BTreeMap<String, String>,
    pub battery: Option<BatteryInfo>,
    pub data_disk: Option<DiskUsage>,
    pub selinux: Option<String>,
    /// Collection failures by data source: `props`, `battery`, `disk`, `selinux`.
    pub errors: BTreeMap<String, String>,
}

/// (check id, English title, Portuguese title)
pub const DEVICE_CHECKS: &[(&str, &str, &str)] = &[
    (
        "dev.connection",
        "ADB connection and authorization",
        "Conexão e autorização ADB",
    ),
    (
        "dev.adb_tcp",
        "adbd listening on TCP",
        "adbd escutando em TCP",
    ),
    (
        "dev.boot_state",
        "Bootloader / verified boot state",
        "Estado do bootloader / boot verificado",
    ),
    ("dev.debuggable", "Debuggable build", "Build depurável"),
    (
        "dev.patch_age",
        "Security patch age",
        "Idade do patch de segurança",
    ),
    ("dev.selinux", "SELinux mode", "Modo do SELinux"),
    (
        "dev.storage",
        "Free storage on /data",
        "Armazenamento livre em /data",
    ),
    (
        "dev.battery",
        "Battery health and temperature",
        "Saúde e temperatura da bateria",
    ),
    ("dev.scrcpy", "scrcpy requirements", "Requisitos do scrcpy"),
];

pub fn check_title(id: &str, lang: Lang) -> String {
    DEVICE_CHECKS
        .iter()
        .find(|(cid, _, _)| *cid == id)
        .map(|(_, en, pt)| lang.pick(en, pt))
        .unwrap_or_else(|| id.to_string())
}

struct Ctx {
    lang: Lang,
    subject: String,
    findings: Vec<Finding>,
    checks: Vec<CheckResult>,
}

impl Ctx {
    fn finding(&mut self, rule: &str, sev: Severity, conf: Confidence, category: &str) -> usize {
        self.findings.push(Finding::new(
            rule,
            ScanTarget::Device,
            &self.subject,
            sev,
            conf,
            category,
        ));
        self.findings.len() - 1
    }

    fn done(&mut self, id: &str, status: CheckStatus, detail: Option<String>) {
        self.checks.push(CheckResult {
            id: id.to_string(),
            title: check_title(id, self.lang),
            status,
            detail,
        });
    }

    fn passed_or_found(&mut self, id: &str, found: bool) {
        self.done(
            id,
            if found {
                CheckStatus::Found
            } else {
                CheckStatus::Passed
            },
            None,
        );
    }
}

// ---------------------------------------------------------------------------
// date helpers (no chrono dependency)
// ---------------------------------------------------------------------------

/// Days since 1970-01-01 for a civil date (proleptic Gregorian).
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// `YYYY-MM-DD` → days since epoch. `None` for anything else.
pub fn parse_patch_date(s: &str) -> Option<i64> {
    let mut it = s.trim().split('-');
    let y: i64 = it.next()?.parse().ok()?;
    let m: i64 = it.next()?.parse().ok()?;
    let d: i64 = it.next()?.parse().ok()?;
    if it.next().is_some()
        || !(1970..=2200).contains(&y)
        || !(1..=12).contains(&m)
        || !(1..=31).contains(&d)
    {
        return None;
    }
    Some(days_from_civil(y, m, d))
}

fn prop<'a>(s: &'a DeviceSnapshot, k: &str) -> Option<&'a str> {
    s.props.get(k).map(|v| v.trim()).filter(|v| !v.is_empty())
}

/// `dumpsys battery` prints the numeric `BatteryManager.BATTERY_HEALTH_*`
/// constant on most builds; some print text. Returns a label for problems.
fn battery_health_problem(raw: &str) -> Option<&'static str> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "3" | "overheat" => Some("overheat"),
        "4" | "dead" => Some("dead"),
        "5" | "over voltage" | "over_voltage" => Some("over voltage"),
        "6" | "unspecified failure" | "unspecified_failure" => Some("unspecified failure"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// evaluation
// ---------------------------------------------------------------------------

pub fn evaluate_device(
    s: &DeviceSnapshot,
    now_unix: i64,
    lang: Lang,
) -> (Vec<Finding>, Vec<CheckResult>) {
    let mut c = Ctx {
        lang,
        subject: s.serial.clone(),
        findings: Vec::new(),
        checks: Vec::new(),
    };

    // --- connection -------------------------------------------------------
    match s.state {
        DeviceState::Connected => c.done("dev.connection", CheckStatus::Passed, None),
        DeviceState::Unauthorized
        | DeviceState::Offline
        | DeviceState::Recovery
        | DeviceState::Other => {
            let (rule, sev, state_txt) = match s.state {
                DeviceState::Unauthorized => ("DEV-UNAUTHORIZED", Severity::Medium, "unauthorized"),
                DeviceState::Offline => ("DEV-OFFLINE", Severity::Low, "offline"),
                _ => ("DEV-NOT-READY", Severity::Low, "not in normal mode"),
            };
            let i = c.finding(rule, sev, Confidence::High, "connectivity");
            let f = &mut c.findings[i];
            f.title = match s.state {
                DeviceState::Unauthorized => lang.pick(
                    "Device has not authorized this computer",
                    "Dispositivo não autorizou este computador",
                ),
                DeviceState::Offline => lang.pick("Device is offline", "Dispositivo está offline"),
                _ => lang.pick(
                    "Device is not in normal ADB mode",
                    "Dispositivo não está no modo ADB normal",
                ),
            };
            f.detection = lang.pick(
                "State reported by `adb devices -l`",
                "Estado informado por `adb devices -l`",
            );
            f.evidence
                .push(Evidence::new("adb devices -l", "state", state_txt));
            f.impact = lang.pick(
                "Every other device check was skipped: nothing can be read from the device in this state.",
                "Todas as outras verificações do dispositivo foram puladas: nada pode ser lido neste estado.",
            );
            f.recommendation = match s.state {
                DeviceState::Unauthorized => lang.pick(
                    "Unlock the device and accept the RSA key dialog (\"Allow USB debugging\"). If it never appears, revoke USB debugging authorizations in Developer options and reconnect.",
                    "Desbloqueie o aparelho e aceite o diálogo da chave RSA (\"Permitir depuração USB\"). Se ele não aparecer, revogue as autorizações de depuração USB nas Opções do desenvolvedor e reconecte.",
                ),
                DeviceState::Offline => lang.pick(
                    "Reconnect the cable or network, then run `adb kill-server` and try again.",
                    "Reconecte o cabo ou a rede, depois execute `adb kill-server` e tente novamente.",
                ),
                _ => lang.pick(
                    "Boot the device normally (recovery/sideload/bootloader modes expose no shell).",
                    "Inicie o aparelho normalmente (modos recovery/sideload/bootloader não expõem shell).",
                ),
            };
            c.done("dev.connection", CheckStatus::Found, None);
            let reason = lang.pick(
                "device is not connected and authorized",
                "dispositivo não está conectado e autorizado",
            );
            for (id, _, _) in DEVICE_CHECKS.iter().skip(1) {
                c.done(id, CheckStatus::Skipped, Some(reason.clone()));
            }
            return (c.findings, c.checks);
        }
    }

    // --- props-based checks ------------------------------------------------
    if let Some(err) = s.errors.get("props") {
        for id in [
            "dev.adb_tcp",
            "dev.boot_state",
            "dev.debuggable",
            "dev.patch_age",
            "dev.scrcpy",
        ] {
            c.done(id, CheckStatus::Error, Some(err.clone()));
        }
    } else {
        eval_adb_tcp(&mut c, s);
        eval_boot_state(&mut c, s);
        eval_debuggable(&mut c, s);
        eval_patch(&mut c, s, now_unix);
        eval_scrcpy(&mut c, s);
    }

    eval_selinux(&mut c, s);
    eval_storage(&mut c, s);
    eval_battery(&mut c, s);

    // Keep the check order stable (as declared).
    let order = |id: &str| {
        DEVICE_CHECKS
            .iter()
            .position(|(cid, _, _)| *cid == id)
            .unwrap_or(usize::MAX)
    };
    c.checks.sort_by_key(|ch| order(&ch.id));
    (c.findings, c.checks)
}

fn eval_adb_tcp(c: &mut Ctx, s: &DeviceSnapshot) {
    let lang = c.lang;
    let keys = ["service.adb.tcp.port", "persist.adb.tcp.port"];
    let hits: Vec<(&str, &str)> = keys
        .iter()
        .filter_map(|k| prop(s, k).map(|v| (*k, v)))
        .filter(|(_, v)| v.parse::<u32>().map(|p| p > 0).unwrap_or(false))
        .collect();
    if hits.is_empty() {
        c.passed_or_found("dev.adb_tcp", false);
        return;
    }
    let i = c.finding(
        "DEV-ADB-TCP",
        Severity::Medium,
        Confidence::High,
        "exposure",
    );
    let f = &mut c.findings[i];
    f.title = lang.pick(
        "adbd is listening on a TCP port",
        "adbd está escutando em uma porta TCP",
    );
    f.detection = lang.pick(
        "Read `service.adb.tcp.port` / `persist.adb.tcp.port` with getprop",
        "Leitura de `service.adb.tcp.port` / `persist.adb.tcp.port` via getprop",
    );
    for (k, v) in hits {
        f.evidence.push(Evidence::new("getprop", k, v));
    }
    f.impact = lang.pick(
        "Hosts on the same network can attempt to connect to adbd. The RSA authorization dialog still applies, but the service is reachable and the traffic is not encrypted in this legacy mode.",
        "Hosts na mesma rede podem tentar conectar ao adbd. O diálogo de autorização RSA continua valendo, mas o serviço fica acessível e o tráfego não é criptografado neste modo legado.",
    );
    f.recommendation = lang.pick(
        "When you no longer need it, go back to USB with `adb usb`, or switch to Wireless debugging (Android 11+), which uses pairing.",
        "Quando não precisar mais, volte ao USB com `adb usb` ou use a Depuração sem fio (Android 11+), que usa pareamento.",
    );
    f.reversible = Some(true);
    c.passed_or_found("dev.adb_tcp", true);
}

fn eval_boot_state(c: &mut Ctx, s: &DeviceSnapshot) {
    let lang = c.lang;
    let state = prop(s, "ro.boot.verifiedbootstate");
    let locked = prop(s, "ro.boot.flash.locked");
    let vb_dev = prop(s, "ro.boot.vbmeta.device_state");
    if state.is_none() && locked.is_none() && vb_dev.is_none() {
        c.done(
            "dev.boot_state",
            CheckStatus::Skipped,
            Some(lang.pick(
                "this device does not expose boot-state properties",
                "este dispositivo não expõe propriedades de estado de boot",
            )),
        );
        return;
    }
    let unlocked = locked == Some("0") || vb_dev == Some("unlocked") || state == Some("orange");
    let red = state == Some("red");
    let yellow = state == Some("yellow");
    if !(unlocked || red || yellow) {
        c.passed_or_found("dev.boot_state", false);
        return;
    }
    let sev = if red {
        Severity::High
    } else if unlocked {
        Severity::Medium
    } else {
        Severity::Low
    };
    let i = c.finding(
        "DEV-BOOTLOADER-UNLOCKED",
        sev,
        Confidence::High,
        "integrity",
    );
    let f = &mut c.findings[i];
    f.title = if red {
        lang.pick(
            "Verified boot reports a verification failure",
            "Boot verificado reporta falha de verificação",
        )
    } else if unlocked {
        lang.pick("Bootloader is unlocked", "Bootloader está desbloqueado")
    } else {
        lang.pick(
            "Device boots with a custom verified-boot key",
            "Dispositivo inicia com uma chave de boot verificado personalizada",
        )
    };
    f.detection = lang.pick(
        "Read boot properties with getprop",
        "Leitura das propriedades de boot via getprop",
    );
    for (k, v) in [
        ("ro.boot.verifiedbootstate", state),
        ("ro.boot.flash.locked", locked),
        ("ro.boot.vbmeta.device_state", vb_dev),
    ] {
        if let Some(v) = v {
            f.evidence.push(Evidence::new("getprop", k, v));
        }
    }
    f.impact = lang.pick(
        "The system image is not protected by the vendor's verified-boot chain: a modified or malicious system could boot without warning. Expected on development devices.",
        "A imagem do sistema não é protegida pela cadeia de boot verificado do fabricante: um sistema modificado ou malicioso poderia iniciar sem aviso. Esperado em aparelhos de desenvolvimento.",
    );
    f.recommendation = lang.pick(
        "If this is not a development device, relock the bootloader (this wipes the device and may require the stock firmware).",
        "Se não for um aparelho de desenvolvimento, bloqueie novamente o bootloader (isso apaga o aparelho e pode exigir o firmware original).",
    );
    f.reversible = Some(false);
    c.passed_or_found("dev.boot_state", true);
}

fn eval_debuggable(c: &mut Ctx, s: &DeviceSnapshot) {
    let lang = c.lang;
    match prop(s, "ro.debuggable") {
        None => c.done(
            "dev.debuggable",
            CheckStatus::Skipped,
            Some(lang.pick("ro.debuggable not exposed", "ro.debuggable não exposto")),
        ),
        Some("1") => {
            let i = c.finding(
                "DEV-DEBUGGABLE-BUILD",
                Severity::Medium,
                Confidence::High,
                "integrity",
            );
            let f = &mut c.findings[i];
            f.title = lang.pick(
                "System build is debuggable (eng/userdebug)",
                "Build do sistema é depurável (eng/userdebug)",
            );
            f.detection = lang.pick(
                "Read `ro.debuggable` with getprop",
                "Leitura de `ro.debuggable` via getprop",
            );
            f.evidence
                .push(Evidence::new("getprop", "ro.debuggable", "1"));
            if let Some(t) = prop(s, "ro.build.type") {
                f.evidence
                    .push(Evidence::new("getprop", "ro.build.type", t));
            }
            f.impact = lang.pick(
                "Debuggable builds relax platform protections (for example adbd may run with elevated rights). Normal on emulators and engineering devices; unexpected on a consumer device.",
                "Builds depuráveis relaxam proteções da plataforma (por exemplo, o adbd pode rodar com privilégios elevados). Normal em emuladores e aparelhos de engenharia; inesperado em aparelho de consumo.",
            );
            f.recommendation = lang.pick(
                "Use a `user` build for anything that handles real data.",
                "Use um build `user` para qualquer coisa que lide com dados reais.",
            );
            c.passed_or_found("dev.debuggable", true);
        }
        Some(_) => c.passed_or_found("dev.debuggable", false),
    }
}

fn eval_patch(c: &mut Ctx, s: &DeviceSnapshot, now_unix: i64) {
    let lang = c.lang;
    let Some(raw) = prop(s, "ro.build.version.security_patch") else {
        c.done(
            "dev.patch_age",
            CheckStatus::Skipped,
            Some(lang.pick(
                "security patch level not exposed",
                "nível do patch de segurança não exposto",
            )),
        );
        return;
    };
    let Some(patch_day) = parse_patch_date(raw) else {
        c.done(
            "dev.patch_age",
            CheckStatus::Error,
            Some(format!(
                "{}: {raw}",
                lang.pick(
                    "unrecognised date format",
                    "formato de data não reconhecido"
                )
            )),
        );
        return;
    };
    let age = now_unix.div_euclid(86_400) - patch_day;
    let sev = if age > 730 {
        Severity::High
    } else if age > 365 {
        Severity::Medium
    } else {
        c.passed_or_found("dev.patch_age", false);
        return;
    };
    let i = c.finding("DEV-PATCH-OLD", sev, Confidence::Medium, "patching");
    let f = &mut c.findings[i];
    f.title = lang.pick(
        "Android security patch level is old",
        "Nível do patch de segurança do Android está antigo",
    );
    f.detection = lang.pick(
        "Compared `ro.build.version.security_patch` with today's date",
        "Comparação de `ro.build.version.security_patch` com a data de hoje",
    );
    f.evidence.push(Evidence::new(
        "getprop",
        "ro.build.version.security_patch",
        raw,
    ));
    f.evidence
        .push(Evidence::new("computed", "age_days", age.to_string()));
    f.impact = lang.pick(
        "Vulnerabilities fixed in later monthly bulletins may be present. This is an age check only: no specific CVE was identified, and updates delivered through Google Play system updates are not reflected in this property.",
        "Vulnerabilidades corrigidas em boletins mensais posteriores podem estar presentes. É apenas uma checagem de idade: nenhuma CVE específica foi identificada, e atualizações entregues por atualizações do sistema via Google Play não aparecem nesta propriedade.",
    );
    f.recommendation = lang.pick(
        "Check for a system update. If the vendor no longer ships updates, avoid handling sensitive data on this device.",
        "Verifique se há atualização do sistema. Se o fabricante não envia mais atualizações, evite lidar com dados sensíveis neste aparelho.",
    );
    c.passed_or_found("dev.patch_age", true);
}

fn eval_scrcpy(c: &mut Ctx, s: &DeviceSnapshot) {
    let lang = c.lang;
    let Some(sdk) = prop(s, "ro.build.version.sdk").and_then(|v| v.parse::<u32>().ok()) else {
        c.done(
            "dev.scrcpy",
            CheckStatus::Skipped,
            Some(lang.pick("SDK level not exposed", "nível de SDK não exposto")),
        );
        return;
    };
    // Requirements from the scrcpy README: API 21 minimum, audio on API 30+.
    let (title, impact) = if sdk < 21 {
        (
            lang.pick(
                "scrcpy does not support this Android version",
                "scrcpy não suporta esta versão do Android",
            ),
            lang.pick(
                "scrcpy requires Android 5.0 (API 21) or newer; mirroring will not work.",
                "O scrcpy exige Android 5.0 (API 21) ou superior; o espelhamento não funcionará.",
            ),
        )
    } else if sdk < 30 {
        (
            lang.pick("scrcpy audio forwarding is unavailable", "Encaminhamento de áudio do scrcpy indisponível"),
            lang.pick("Audio forwarding needs Android 11 (API 30) or newer; video and control still work.", "O encaminhamento de áudio exige Android 11 (API 30) ou superior; vídeo e controle continuam funcionando."),
        )
    } else {
        c.passed_or_found("dev.scrcpy", false);
        return;
    };
    let i = c.finding(
        "DEV-SCRCPY-LIMITS",
        Severity::Info,
        Confidence::High,
        "compatibility",
    );
    let f = &mut c.findings[i];
    f.title = title;
    f.detection = lang.pick(
        "Read `ro.build.version.sdk` with getprop",
        "Leitura de `ro.build.version.sdk` via getprop",
    );
    f.evidence.push(Evidence::new(
        "getprop",
        "ro.build.version.sdk",
        sdk.to_string(),
    ));
    f.impact = impact;
    f.recommendation = lang.pick(
        "Use the mirroring without audio (--no-audio) or update the device.",
        "Use o espelhamento sem áudio (--no-audio) ou atualize o aparelho.",
    );
    c.passed_or_found("dev.scrcpy", true);
}

fn eval_selinux(c: &mut Ctx, s: &DeviceSnapshot) {
    let lang = c.lang;
    if let Some(err) = s.errors.get("selinux") {
        c.done("dev.selinux", CheckStatus::Error, Some(err.clone()));
        return;
    }
    let Some(mode) = s
        .selinux
        .as_deref()
        .map(str::trim)
        .filter(|m| !m.is_empty())
    else {
        c.done(
            "dev.selinux",
            CheckStatus::Skipped,
            Some(lang.pick(
                "getenforce returned nothing",
                "getenforce não retornou nada",
            )),
        );
        return;
    };
    match mode.to_ascii_lowercase().as_str() {
        "permissive" => {
            let i = c.finding(
                "DEV-SELINUX-PERMISSIVE",
                Severity::High,
                Confidence::High,
                "integrity",
            );
            let f = &mut c.findings[i];
            f.title = lang.pick(
                "SELinux is in permissive mode",
                "SELinux está em modo permissivo",
            );
            f.detection = lang.pick("Output of `getenforce`", "Saída de `getenforce`");
            f.evidence.push(Evidence::new("getenforce", "mode", mode));
            f.impact = lang.pick(
                "SELinux policy violations are logged but not blocked, which removes a major isolation layer between apps and the system.",
                "Violações da política do SELinux são registradas mas não bloqueadas, o que remove uma camada importante de isolamento entre apps e sistema.",
            );
            f.recommendation = lang.pick(
                "Restore enforcing mode (`setenforce 1` needs root) or reflash a build that boots enforcing.",
                "Restaure o modo enforcing (`setenforce 1` exige root) ou regrave um build que inicie em enforcing.",
            );
            f.reversible = Some(true);
            c.passed_or_found("dev.selinux", true);
        }
        "enforcing" => c.passed_or_found("dev.selinux", false),
        other => c.done(
            "dev.selinux",
            CheckStatus::Skipped,
            Some(format!(
                "{}: {other}",
                lang.pick("unrecognised output", "saída não reconhecida")
            )),
        ),
    }
}

fn eval_storage(c: &mut Ctx, s: &DeviceSnapshot) {
    let lang = c.lang;
    if let Some(err) = s.errors.get("disk") {
        c.done("dev.storage", CheckStatus::Error, Some(err.clone()));
        return;
    }
    let Some(d) = s.data_disk.as_ref().filter(|d| d.total_mb > 0) else {
        c.done(
            "dev.storage",
            CheckStatus::Skipped,
            Some(lang.pick(
                "could not read /data usage",
                "não foi possível ler o uso de /data",
            )),
        );
        return;
    };
    let pct = d.avail_mb * 100 / d.total_mb;
    let sev = if pct < 5 {
        Severity::High
    } else if pct < 10 || d.avail_mb < 1024 {
        Severity::Medium
    } else {
        c.passed_or_found("dev.storage", false);
        return;
    };
    let i = c.finding("DEV-LOW-STORAGE", sev, Confidence::High, "capacity");
    let f = &mut c.findings[i];
    f.title = lang.pick("Little free space on /data", "Pouco espaço livre em /data");
    f.detection = lang.pick("Output of `df -m /data`", "Saída de `df -m /data`");
    f.evidence.push(Evidence::new(
        "df -m /data",
        "total_mb",
        d.total_mb.to_string(),
    ));
    f.evidence.push(Evidence::new(
        "df -m /data",
        "avail_mb",
        d.avail_mb.to_string(),
    ));
    f.evidence
        .push(Evidence::new("computed", "avail_percent", pct.to_string()));
    f.impact = lang.pick(
        "Installs, updates and app writes can fail, and the system may become slow or unstable.",
        "Instalações, atualizações e gravações de apps podem falhar, e o sistema pode ficar lento ou instável.",
    );
    f.recommendation = lang.pick(
        "Free space (large files, unused apps, caches) before installing or testing.",
        "Libere espaço (arquivos grandes, apps sem uso, caches) antes de instalar ou testar.",
    );
    c.passed_or_found("dev.storage", true);
}

fn eval_battery(c: &mut Ctx, s: &DeviceSnapshot) {
    let lang = c.lang;
    if let Some(err) = s.errors.get("battery") {
        c.done("dev.battery", CheckStatus::Error, Some(err.clone()));
        return;
    }
    let Some(b) = s.battery.as_ref() else {
        c.done(
            "dev.battery",
            CheckStatus::Skipped,
            Some(lang.pick(
                "battery data not available",
                "dados da bateria indisponíveis",
            )),
        );
        return;
    };
    let mut found = false;
    if let Some(problem) = b.health.as_deref().and_then(battery_health_problem) {
        found = true;
        let i = c.finding(
            "DEV-BATTERY-HEALTH",
            Severity::Medium,
            Confidence::Medium,
            "hardware",
        );
        let f = &mut c.findings[i];
        f.title = lang.pick(
            "Battery reports a health problem",
            "Bateria reporta problema de saúde",
        );
        f.detection = lang.pick(
            "`health` field of `dumpsys battery`",
            "Campo `health` de `dumpsys battery`",
        );
        f.evidence.push(Evidence::new(
            "dumpsys battery",
            "health",
            b.health.clone().unwrap_or_default(),
        ));
        f.evidence
            .push(Evidence::new("computed", "interpretation", problem));
        f.impact = lang.pick(
            "Unexpected shutdowns or throttling can corrupt test runs and, in rare cases, indicate a safety risk.",
            "Desligamentos ou limitação de desempenho inesperados podem invalidar testes e, em casos raros, indicar risco de segurança.",
        );
        f.recommendation = lang.pick(
            "Stop charging and let the device cool down; have the battery checked or replaced.",
            "Interrompa a carga e deixe o aparelho esfriar; faça a bateria ser avaliada ou trocada.",
        );
    }
    if let Some(t) = b.temperature.filter(|t| *t >= 450) {
        found = true;
        let i = c.finding(
            "DEV-BATTERY-HOT",
            Severity::Low,
            Confidence::Medium,
            "hardware",
        );
        let f = &mut c.findings[i];
        f.title = lang.pick(
            "Battery temperature is high",
            "Temperatura da bateria está alta",
        );
        f.detection = lang.pick(
            "`temperature` field of `dumpsys battery` (tenths of °C)",
            "Campo `temperature` de `dumpsys battery` (décimos de °C)",
        );
        f.evidence.push(Evidence::new(
            "dumpsys battery",
            "temperature",
            t.to_string(),
        ));
        f.evidence.push(Evidence::new(
            "computed",
            "celsius",
            format!("{:.1}", t as f32 / 10.0),
        ));
        f.impact = lang.pick(
            "Heuristic threshold (45 °C): sustained heat can throttle the CPU and skew performance measurements.",
            "Limite heurístico (45 °C): calor prolongado pode reduzir o desempenho da CPU e distorcer medições.",
        );
        f.recommendation = lang.pick(
            "Let the device cool before benchmarking; avoid charging while stress testing.",
            "Deixe o aparelho esfriar antes de medir desempenho; evite carregar durante testes de estresse.",
        );
    }
    c.passed_or_found("dev.battery", found);
}

// ---------------------------------------------------------------------------
// collection (read-only)
// ---------------------------------------------------------------------------

fn run_op(client: &AdbClient<'_>, serial: &str, op: DeviceOperation) -> Result<String, String> {
    let timeout = match op {
        DeviceOperation::GetProps => TIMEOUT_PROPS,
        _ => TIMEOUT_DEVICE,
    };
    op.validate().map_err(|e| e.to_string())?;
    let args = op
        .to_adb_args(Some(serial))
        .map_err(|e: AppError| e.to_string())?;
    let out = client.run(args, timeout).map_err(|e| e.to_string())?;
    if out.timed_out {
        return Err("timeout".into());
    }
    if !out.success() {
        let msg = out.stderr.trim();
        return Err(if msg.is_empty() {
            format!("exit code {}", out.exit_code)
        } else {
            msg.to_string()
        });
    }
    Ok(out.text())
}

/// Reads the device with read-only allowlisted operations.
pub fn collect_device(adb: &str, serial: &str) -> Result<DeviceSnapshot, AppError> {
    crate::security::validate_serial(serial)?;
    let devices = crate::devices::list_devices(adb)?;
    // A serial that is not listed is NOT "offline": it is not connected at all.
    let dev = devices.iter().find(|d| d.serial == serial).ok_or_else(|| {
        AppError::new(
            crate::error::ErrorCode::NoDevice,
            format!("device {serial} is not connected"),
        )
    })?;
    let (state, connection) = (dev.state, dev.connection);

    let mut snap = DeviceSnapshot {
        serial: serial.to_string(),
        state,
        connection,
        props: BTreeMap::new(),
        battery: None,
        data_disk: None,
        selinux: None,
        errors: BTreeMap::new(),
    };
    if state != DeviceState::Connected {
        return Ok(snap);
    }

    let client = AdbClient::new(adb);
    match run_op(&client, serial, DeviceOperation::GetProps) {
        Ok(t) => snap.props = parse::parse_getprop(&t),
        Err(e) => {
            snap.errors.insert("props".into(), e);
        }
    }
    match run_op(&client, serial, DeviceOperation::GetBattery) {
        Ok(t) => snap.battery = Some(parse::parse_dumpsys_battery(&t)),
        Err(e) => {
            snap.errors.insert("battery".into(), e);
        }
    }
    match run_op(
        &client,
        serial,
        DeviceOperation::GetDiskUsage {
            path: "/data".into(),
        },
    ) {
        Ok(t) => snap.data_disk = parse::parse_df(&t, "/data"),
        Err(e) => {
            snap.errors.insert("disk".into(), e);
        }
    }
    match run_op(&client, serial, DeviceOperation::GetSelinuxMode) {
        Ok(t) => snap.selinux = Some(t.trim().to_string()),
        Err(e) => {
            snap.errors.insert("selinux".into(), e);
        }
    }
    Ok(snap)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_792_000_000; // 2026-10-14

    fn snap() -> DeviceSnapshot {
        let mut props = BTreeMap::new();
        for (k, v) in [
            ("ro.build.version.security_patch", "2026-09-05"),
            ("ro.build.version.sdk", "34"),
            ("ro.debuggable", "0"),
            ("ro.boot.verifiedbootstate", "green"),
            ("ro.boot.flash.locked", "1"),
        ] {
            props.insert(k.to_string(), v.to_string());
        }
        DeviceSnapshot {
            serial: "S1".into(),
            state: DeviceState::Connected,
            connection: ConnectionKind::Usb,
            props,
            battery: Some(parse::parse_dumpsys_battery(
                "level: 80\nscale: 100\nhealth: 2\ntemperature: 310\n",
            )),
            data_disk: Some(DiskUsage {
                total_mb: 100_000,
                used_mb: 40_000,
                avail_mb: 60_000,
                path: "/data".into(),
            }),
            selinux: Some("Enforcing".into()),
            errors: BTreeMap::new(),
        }
    }

    fn rules(s: &DeviceSnapshot) -> Vec<String> {
        evaluate_device(s, NOW, Lang::En)
            .0
            .into_iter()
            .map(|f| f.rule_id)
            .collect()
    }

    fn status(s: &DeviceSnapshot, id: &str) -> CheckStatus {
        evaluate_device(s, NOW, Lang::En)
            .1
            .into_iter()
            .find(|c| c.id == id)
            .unwrap()
            .status
    }

    #[test]
    fn date_math_is_correct() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1), 11_017);
        assert_eq!(days_from_civil(2026, 10, 14), NOW / 86_400);
        assert_eq!(
            parse_patch_date("2026-09-05"),
            Some(days_from_civil(2026, 9, 5))
        );
        for bad in [
            "",
            "2026-13-01",
            "2026-00-10",
            "26-01-01",
            "2026-01",
            "2026-01-01-01",
            "abcd-ef-gh",
        ] {
            assert_eq!(parse_patch_date(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_clean_device_has_no_findings_and_every_check_ran() {
        let (f, c) = evaluate_device(&snap(), NOW, Lang::En);
        assert!(f.is_empty(), "{f:?}");
        assert_eq!(c.len(), DEVICE_CHECKS.len());
        assert!(c.iter().all(|x| x.status == CheckStatus::Passed), "{c:?}");
    }

    #[test]
    fn unauthorized_device_skips_everything_else_and_says_so() {
        let mut s = snap();
        s.state = DeviceState::Unauthorized;
        let (f, c) = evaluate_device(&s, NOW, Lang::En);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].rule_id, "DEV-UNAUTHORIZED");
        assert_eq!(
            c.iter()
                .filter(|x| x.status == CheckStatus::Skipped)
                .count(),
            DEVICE_CHECKS.len() - 1
        );
    }

    #[test]
    fn patch_age_thresholds() {
        let mut s = snap();
        s.props.insert(
            "ro.build.version.security_patch".into(),
            "2025-12-01".into(),
        ); // ~317 d
        assert!(!rules(&s).contains(&"DEV-PATCH-OLD".to_string()));
        s.props.insert(
            "ro.build.version.security_patch".into(),
            "2025-06-01".into(),
        ); // ~500 d
        let (f, _) = evaluate_device(&s, NOW, Lang::En);
        let p = f.iter().find(|x| x.rule_id == "DEV-PATCH-OLD").unwrap();
        assert_eq!(p.severity, Severity::Medium);
        assert_eq!(
            p.confidence,
            Confidence::Medium,
            "an age check is never high confidence"
        );
        s.props.insert(
            "ro.build.version.security_patch".into(),
            "2023-01-01".into(),
        );
        let (f, _) = evaluate_device(&s, NOW, Lang::En);
        assert_eq!(
            f.iter()
                .find(|x| x.rule_id == "DEV-PATCH-OLD")
                .unwrap()
                .severity,
            Severity::High
        );
    }

    #[test]
    fn unparsable_patch_is_an_error_not_a_pass() {
        let mut s = snap();
        s.props
            .insert("ro.build.version.security_patch".into(), "soon".into());
        assert_eq!(status(&s, "dev.patch_age"), CheckStatus::Error);
    }

    #[test]
    fn adb_tcp_only_flags_a_positive_port() {
        let mut s = snap();
        s.props.insert("service.adb.tcp.port".into(), "".into());
        s.props.insert("persist.adb.tcp.port".into(), "0".into());
        assert!(!rules(&s).contains(&"DEV-ADB-TCP".to_string()));
        s.props.insert("service.adb.tcp.port".into(), "5555".into());
        let (f, _) = evaluate_device(&s, NOW, Lang::En);
        let x = f.iter().find(|x| x.rule_id == "DEV-ADB-TCP").unwrap();
        assert_eq!(x.evidence[0].value, "5555");
        assert_eq!(x.reversible, Some(true));
    }

    #[test]
    fn bootloader_and_debuggable_and_selinux_rules() {
        let mut s = snap();
        s.props.insert("ro.boot.flash.locked".into(), "0".into());
        s.props
            .insert("ro.boot.verifiedbootstate".into(), "orange".into());
        s.props.insert("ro.debuggable".into(), "1".into());
        s.selinux = Some("Permissive\n".into());
        let r = rules(&s);
        for want in [
            "DEV-BOOTLOADER-UNLOCKED",
            "DEV-DEBUGGABLE-BUILD",
            "DEV-SELINUX-PERMISSIVE",
        ] {
            assert!(r.contains(&want.to_string()), "{want} in {r:?}");
        }
    }

    #[test]
    fn missing_properties_are_skipped_not_passed() {
        let mut s = snap();
        s.props.remove("ro.boot.verifiedbootstate");
        s.props.remove("ro.boot.flash.locked");
        assert_eq!(status(&s, "dev.boot_state"), CheckStatus::Skipped);
        s.selinux = None;
        assert_eq!(status(&s, "dev.selinux"), CheckStatus::Skipped);
    }

    #[test]
    fn collection_errors_are_reported_as_errors() {
        let mut s = snap();
        s.errors.insert("props".into(), "timeout".into());
        s.errors.insert("disk".into(), "exit code 1".into());
        assert_eq!(status(&s, "dev.debuggable"), CheckStatus::Error);
        assert_eq!(status(&s, "dev.storage"), CheckStatus::Error);
    }

    #[test]
    fn storage_thresholds() {
        let mut s = snap();
        s.data_disk = Some(DiskUsage {
            total_mb: 100_000,
            used_mb: 92_000,
            avail_mb: 8_000,
            path: "/data".into(),
        });
        let (f, _) = evaluate_device(&s, NOW, Lang::En);
        assert_eq!(
            f.iter()
                .find(|x| x.rule_id == "DEV-LOW-STORAGE")
                .unwrap()
                .severity,
            Severity::Medium
        );
        s.data_disk = Some(DiskUsage {
            total_mb: 100_000,
            used_mb: 97_000,
            avail_mb: 3_000,
            path: "/data".into(),
        });
        let (f, _) = evaluate_device(&s, NOW, Lang::En);
        assert_eq!(
            f.iter()
                .find(|x| x.rule_id == "DEV-LOW-STORAGE")
                .unwrap()
                .severity,
            Severity::High
        );
        s.data_disk = Some(DiskUsage {
            total_mb: 8_000,
            used_mb: 7_500,
            avail_mb: 500,
            path: "/data".into(),
        });
        assert!(
            rules(&s).contains(&"DEV-LOW-STORAGE".to_string()),
            "<1 GiB free is flagged even above 5 %"
        );
    }

    #[test]
    fn battery_rules() {
        let mut s = snap();
        s.battery = Some(parse::parse_dumpsys_battery(
            "level: 50\nscale: 100\nhealth: 3\ntemperature: 478\n",
        ));
        let r = rules(&s);
        assert!(r.contains(&"DEV-BATTERY-HEALTH".to_string()));
        assert!(r.contains(&"DEV-BATTERY-HOT".to_string()));
        s.battery = Some(parse::parse_dumpsys_battery(
            "health: 2\ntemperature: 449\n",
        ));
        assert!(rules(&s).is_empty());
    }

    #[test]
    fn scrcpy_limits_follow_the_documented_requirements() {
        let mut s = snap();
        s.props.insert("ro.build.version.sdk".into(), "29".into());
        assert!(rules(&s).contains(&"DEV-SCRCPY-LIMITS".to_string()));
        s.props.insert("ro.build.version.sdk".into(), "30".into());
        assert!(!rules(&s).contains(&"DEV-SCRCPY-LIMITS".to_string()));
    }

    #[test]
    fn texts_follow_the_requested_language_and_ids_do_not_change() {
        let mut s = snap();
        s.selinux = Some("Permissive".into());
        let (en, _) = evaluate_device(&s, NOW, Lang::En);
        let (pt, _) = evaluate_device(&s, NOW, Lang::Pt);
        assert_eq!(en[0].id, pt[0].id);
        assert_ne!(en[0].title, pt[0].title);
        assert!(pt[0].title.contains("SELinux"));
    }
}
