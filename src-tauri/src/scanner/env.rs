//! Environment scan: the computer's adb / fastboot / scrcpy setup.
//!
//! Same split as the device scan: [`collect_env`] gathers facts (runs
//! `adb version`, `adb server-status`, inspects PATH and file modes) and
//! [`evaluate_env`] is a pure function over the [`EnvSnapshot`].

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::time::Duration;

use crate::adb::client::AdbClient;
use crate::adb::ToolStatus;

use super::model::{
    CheckResult, CheckStatus, Confidence, Evidence, Finding, Lang, ScanTarget, Severity,
};

/// First platform-tools release with ADB Wi-Fi 2.0 (Android developer docs).
pub const ADB_WIFI2_MIN_VERSION: &str = "37.0.0";

#[derive(Debug, Clone)]
pub struct ToolInfo {
    pub name: String,
    pub found: bool,
    pub path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AdbCandidate {
    pub path: String,
    pub platform_version: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ServerStatus {
    /// Not attempted (e.g. adb missing).
    NotChecked(String),
    /// adb ran but does not support `server-status` (older platform-tools).
    Unsupported(String),
    Reported(BTreeMap<String, String>),
}

#[derive(Debug, Clone)]
pub struct EnvSnapshot {
    pub tools: Vec<ToolInfo>,
    pub adb_platform_version: Option<String>,
    pub adb_version_error: Option<String>,
    pub server_status: ServerStatus,
    pub env_vars: BTreeMap<String, String>,
    pub adb_candidates: Vec<AdbCandidate>,
    /// Unix permission bits of the adb binary (`None` on other platforms).
    pub adb_mode: Option<u32>,
}

pub const ENV_CHECKS: &[(&str, &str, &str)] = &[
    (
        "env.adb",
        "adb availability and version",
        "Disponibilidade e versão do adb",
    ),
    (
        "env.fastboot",
        "fastboot availability",
        "Disponibilidade do fastboot",
    ),
    (
        "env.scrcpy",
        "scrcpy availability",
        "Disponibilidade do scrcpy",
    ),
    (
        "env.adb_server",
        "adb server status (mDNS)",
        "Status do servidor adb (mDNS)",
    ),
    (
        "env.android_serial",
        "ANDROID_SERIAL environment variable",
        "Variável de ambiente ANDROID_SERIAL",
    ),
    (
        "env.adb_duplicates",
        "Multiple adb binaries on PATH",
        "Vários binários adb no PATH",
    ),
    (
        "env.adb_permissions",
        "adb binary permissions",
        "Permissões do binário adb",
    ),
];

fn check_title(id: &str, lang: Lang) -> String {
    ENV_CHECKS
        .iter()
        .find(|(cid, _, _)| *cid == id)
        .map(|(_, en, pt)| lang.pick(en, pt))
        .unwrap_or_else(|| id.to_string())
}

// ---------------------------------------------------------------------------
// parsing helpers
// ---------------------------------------------------------------------------

/// Extracts `34.0.5` from the `Version 34.0.5-10900879` line of `adb version`.
pub fn parse_adb_platform_version(text: &str) -> Option<String> {
    let line = text
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("Version "))?;
    let token = line
        .trim_start_matches("Version ")
        .split_whitespace()
        .next()?;
    // Debian-style epoch ("1:34.0.4-...") and build suffix ("-10900879").
    let token = token.rsplit(':').next().unwrap_or(token);
    let ver = token.split('-').next().unwrap_or(token);
    if ver
        .split('.')
        .next()
        .is_some_and(|p| p.parse::<u32>().is_ok())
    {
        Some(ver.to_string())
    } else {
        None
    }
}

/// Parses the `key: value` lines of `adb server-status`.
pub fn parse_server_status(text: &str) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    for line in text.lines() {
        if let Some((k, v)) = line.split_once(':') {
            let k = k.trim();
            if !k.is_empty() && !k.contains(' ') {
                m.insert(k.to_string(), v.trim().trim_matches('"').to_string());
            }
        }
    }
    m
}

/// Numeric dotted-version comparison (`34.0.5` < `37.0.0`).
pub fn cmp_version(a: &str, b: &str) -> Ordering {
    let parse = |s: &str| -> Vec<u64> {
        s.split('.')
            .map(|p| {
                p.chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .parse()
                    .unwrap_or(0)
            })
            .collect()
    };
    let (pa, pb) = (parse(a), parse(b));
    for i in 0..pa.len().max(pb.len()) {
        let o = pa.get(i).unwrap_or(&0).cmp(pb.get(i).unwrap_or(&0));
        if o != Ordering::Equal {
            return o;
        }
    }
    Ordering::Equal
}

// ---------------------------------------------------------------------------
// evaluation
// ---------------------------------------------------------------------------

struct Ctx {
    lang: Lang,
    findings: Vec<Finding>,
    checks: Vec<CheckResult>,
}

impl Ctx {
    fn finding(
        &mut self,
        rule: &str,
        subject: &str,
        sev: Severity,
        conf: Confidence,
        cat: &str,
    ) -> usize {
        self.findings.push(Finding::new(
            rule,
            ScanTarget::Environment,
            subject,
            sev,
            conf,
            cat,
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
}

fn tool<'a>(s: &'a EnvSnapshot, name: &str) -> Option<&'a ToolInfo> {
    s.tools.iter().find(|t| t.name == name)
}

pub fn evaluate_env(s: &EnvSnapshot, lang: Lang) -> (Vec<Finding>, Vec<CheckResult>) {
    let mut c = Ctx {
        lang,
        findings: Vec::new(),
        checks: Vec::new(),
    };
    let adb_found = tool(s, "adb").map(|t| t.found).unwrap_or(false);

    // --- adb ---------------------------------------------------------------
    if !adb_found {
        let i = c.finding(
            "ENV-ADB-MISSING",
            "adb",
            Severity::High,
            Confidence::High,
            "tooling",
        );
        let f = &mut c.findings[i];
        f.title = lang.pick("adb was not found", "O adb não foi encontrado");
        f.detection = lang.pick(
            "Manual override → PATH → SDK directories (the same lookup the app uses)",
            "Caminho manual → PATH → pastas do SDK (a mesma busca que o app usa)",
        );
        f.evidence
            .push(Evidence::new("tool detection", "adb.found", "false"));
        f.impact = lang.pick(
            "Without adb the device features (shell, logs, files, apps) cannot work.",
            "Sem o adb, os recursos de dispositivo (shell, logs, arquivos, apps) não funcionam.",
        );
        f.recommendation = lang.pick(
            "Install the Android SDK Platform Tools, or set the adb path in Settings.",
            "Instale o Android SDK Platform Tools ou informe o caminho do adb em Configurações.",
        );
        c.done("env.adb", CheckStatus::Found, None);
    } else if let Some(v) = &s.adb_platform_version {
        if cmp_version(v, ADB_WIFI2_MIN_VERSION) == Ordering::Less {
            let i = c.finding(
                "ENV-ADB-VERSION-OLD",
                "adb",
                Severity::Info,
                Confidence::High,
                "tooling",
            );
            let f = &mut c.findings[i];
            f.title = lang.pick(
                &format!("adb platform-tools {v} predates ADB Wi-Fi 2.0"),
                &format!("platform-tools {v} do adb é anterior ao ADB Wi-Fi 2.0"),
            );
            f.detection = lang.pick(
                "Parsed the `Version` line of `adb version`",
                "Leitura da linha `Version` de `adb version`",
            );
            f.evidence
                .push(Evidence::new("adb version", "platform_tools", v.clone()));
            f.evidence.push(Evidence::new(
                "reference",
                "wifi2_min",
                ADB_WIFI2_MIN_VERSION,
            ));
            f.impact = lang.pick(
                "Informational: automatic wireless connection (ADB Wi-Fi 2.0, Android 17+) needs platform-tools 37.0.0 or newer. USB and classic wireless debugging are unaffected.",
                "Informativo: a conexão sem fio automática (ADB Wi-Fi 2.0, Android 17+) exige platform-tools 37.0.0 ou superior. USB e a depuração sem fio clássica não são afetados.",
            );
            f.recommendation = lang.pick(
                "Update the SDK Platform Tools if you need ADB Wi-Fi 2.0.",
                "Atualize o SDK Platform Tools se precisar do ADB Wi-Fi 2.0.",
            );
            f.reversible = Some(true);
            c.done("env.adb", CheckStatus::Found, None);
        } else {
            c.done("env.adb", CheckStatus::Passed, None);
        }
    } else {
        c.done(
            "env.adb",
            CheckStatus::Error,
            Some(s.adb_version_error.clone().unwrap_or_else(|| {
                lang.pick(
                    "could not parse `adb version`",
                    "não foi possível interpretar `adb version`",
                )
            })),
        );
    }

    // --- optional tools --------------------------------------------------------
    for (name, rule, check, fix_en, fix_pt, use_en, use_pt) in [
        (
            "fastboot",
            "ENV-FASTBOOT-MISSING",
            "env.fastboot",
            "Install the Android SDK Platform Tools (it ships fastboot).",
            "Instale o Android SDK Platform Tools (ele inclui o fastboot).",
            "The Fastboot module is unavailable.",
            "O módulo Fastboot fica indisponível.",
        ),
        (
            "scrcpy",
            "ENV-SCRCPY-MISSING",
            "env.scrcpy",
            "Install scrcpy from your package manager or from its official repository.",
            "Instale o scrcpy pelo gerenciador de pacotes ou pelo repositório oficial.",
            "Screen mirroring and recording are unavailable.",
            "Espelhamento e gravação de tela ficam indisponíveis.",
        ),
    ] {
        if tool(s, name).map(|t| t.found).unwrap_or(false) {
            c.done(check, CheckStatus::Passed, None);
        } else {
            let i = c.finding(rule, name, Severity::Info, Confidence::High, "tooling");
            let f = &mut c.findings[i];
            f.title = lang.pick(
                &format!("{name} was not found"),
                &format!("{name} não foi encontrado"),
            );
            f.detection = lang.pick(
                "Manual override → PATH → SDK directories",
                "Caminho manual → PATH → pastas do SDK",
            );
            f.evidence.push(Evidence::new(
                "tool detection",
                &format!("{name}.found"),
                "false",
            ));
            f.impact = lang.pick(use_en, use_pt);
            f.recommendation = lang.pick(fix_en, fix_pt);
            c.done(check, CheckStatus::Found, None);
        }
    }

    // --- adb server status -------------------------------------------------------
    match &s.server_status {
        ServerStatus::NotChecked(why) => {
            c.done("env.adb_server", CheckStatus::Skipped, Some(why.clone()))
        }
        ServerStatus::Unsupported(why) => c.done(
            "env.adb_server",
            CheckStatus::Skipped,
            Some(format!(
                "{}: {why}",
                lang.pick(
                    "`adb server-status` is not supported by this adb",
                    "`adb server-status` não é suportado por este adb"
                )
            )),
        ),
        ServerStatus::Reported(m) => {
            if m.get("mdns_enabled").map(|v| v == "false").unwrap_or(false) {
                let i = c.finding(
                    "ENV-ADB-MDNS-OFF",
                    "adb",
                    Severity::Low,
                    Confidence::High,
                    "connectivity",
                );
                let f = &mut c.findings[i];
                f.title = lang.pick(
                    "adb mDNS discovery is disabled",
                    "A descoberta mDNS do adb está desativada",
                );
                f.detection = lang.pick(
                    "`mdns_enabled` field of `adb server-status`",
                    "Campo `mdns_enabled` de `adb server-status`",
                );
                f.evidence
                    .push(Evidence::new("adb server-status", "mdns_enabled", "false"));
                f.impact = lang.pick(
                    "adb will not discover wireless-debugging devices automatically; you must connect them by address.",
                    "O adb não descobrirá aparelhos de depuração sem fio automaticamente; será preciso conectar pelo endereço.",
                );
                f.recommendation = lang.pick(
                    "Set ADB_MDNS=1, then run `adb kill-server` and `adb start-server`.",
                    "Defina ADB_MDNS=1 e execute `adb kill-server` e `adb start-server`.",
                );
                f.reversible = Some(true);
                c.done("env.adb_server", CheckStatus::Found, None);
            } else if m.contains_key("mdns_enabled") {
                c.done("env.adb_server", CheckStatus::Passed, None);
            } else {
                c.done(
                    "env.adb_server",
                    CheckStatus::Skipped,
                    Some(lang.pick(
                        "`mdns_enabled` not present in the output",
                        "`mdns_enabled` ausente na saída",
                    )),
                );
            }
        }
    }

    // --- ANDROID_SERIAL --------------------------------------------------------------
    match s
        .env_vars
        .get("ANDROID_SERIAL")
        .filter(|v| !v.trim().is_empty())
    {
        Some(v) => {
            let i = c.finding(
                "ENV-ANDROID-SERIAL-SET",
                "ANDROID_SERIAL",
                Severity::Low,
                Confidence::High,
                "configuration",
            );
            let f = &mut c.findings[i];
            f.title = lang.pick(
                "ANDROID_SERIAL is set in the environment",
                "ANDROID_SERIAL está definido no ambiente",
            );
            f.detection = lang.pick(
                "Read the process environment",
                "Leitura do ambiente do processo",
            );
            f.evidence
                .push(Evidence::new("env", "ANDROID_SERIAL", v.clone()));
            f.impact = lang.pick(
                "adb commands that do not pass `-s` are directed to this device. ZittoDB always targets a device explicitly, but external adb/scrcpy runs from the same environment may hit the wrong device.",
                "Comandos adb sem `-s` são direcionados a este aparelho. O ZittoDB sempre escolhe o dispositivo explicitamente, mas execuções externas de adb/scrcpy no mesmo ambiente podem atingir o aparelho errado.",
            );
            f.recommendation = lang.pick(
                "Unset it (`unset ANDROID_SERIAL`) unless you set it on purpose.",
                "Remova-o (`unset ANDROID_SERIAL`), a menos que o tenha definido de propósito.",
            );
            f.reversible = Some(true);
            c.done("env.android_serial", CheckStatus::Found, None);
        }
        None => c.done("env.android_serial", CheckStatus::Passed, None),
    }

    // --- duplicates on PATH ------------------------------------------------------------
    if !adb_found {
        c.done(
            "env.adb_duplicates",
            CheckStatus::Skipped,
            Some(lang.pick("adb not found", "adb não encontrado")),
        );
    } else if s.adb_candidates.len() > 1 {
        let mut versions: Vec<&str> = s
            .adb_candidates
            .iter()
            .filter_map(|a| a.platform_version.as_deref())
            .collect();
        versions.sort_unstable();
        versions.dedup();
        let differ = versions.len() > 1;
        let (sev, conf) = if differ {
            (Severity::Medium, Confidence::High)
        } else {
            (Severity::Info, Confidence::Medium)
        };
        let i = c.finding("ENV-ADB-MULTIPLE", "adb", sev, conf, "configuration");
        let f = &mut c.findings[i];
        f.title = if differ {
            lang.pick(
                "Different adb versions are installed on PATH",
                "Há versões diferentes do adb no PATH",
            )
        } else {
            lang.pick(
                "More than one adb binary is on PATH",
                "Há mais de um binário adb no PATH",
            )
        };
        f.detection = lang.pick(
            "Looked for an `adb` executable in every PATH directory and ran `adb version` on each",
            "Busca de um executável `adb` em cada pasta do PATH e execução de `adb version` em cada um",
        );
        for a in &s.adb_candidates {
            f.evidence.push(Evidence::new(
                "PATH",
                &a.path,
                a.platform_version.clone().unwrap_or_else(|| "?".into()),
            ));
        }
        f.impact = lang.pick(
            "When client and server versions differ, adb kills and restarts the server, which can drop connections of other tools (IDEs, scrcpy) in the middle of work.",
            "Quando as versões de cliente e servidor diferem, o adb encerra e reinicia o servidor, o que pode derrubar conexões de outras ferramentas (IDEs, scrcpy) no meio do trabalho.",
        );
        f.recommendation = lang.pick(
            "Keep a single platform-tools install first on PATH, or point ZittoDB to the one you want in Settings.",
            "Mantenha uma única instalação do platform-tools à frente no PATH ou aponte o ZittoDB para a desejada em Configurações.",
        );
        c.done("env.adb_duplicates", CheckStatus::Found, None);
    } else {
        c.done("env.adb_duplicates", CheckStatus::Passed, None);
    }

    // --- adb binary permissions ------------------------------------------------------------
    match s.adb_mode {
        None => c.done(
            "env.adb_permissions",
            CheckStatus::Skipped,
            Some(lang.pick(
                "not applicable on this platform or adb not found",
                "não se aplica a esta plataforma ou adb não encontrado",
            )),
        ),
        Some(mode) if mode & 0o022 != 0 => {
            let path = tool(s, "adb")
                .and_then(|t| t.path.clone())
                .unwrap_or_default();
            let i = c.finding(
                "ENV-ADB-WRITABLE",
                "adb",
                Severity::Medium,
                Confidence::High,
                "integrity",
            );
            let f = &mut c.findings[i];
            f.title = lang.pick(
                "The adb binary is writable by group or others",
                "O binário do adb é gravável por grupo ou outros",
            );
            f.detection = lang.pick(
                "Unix permission bits of the resolved adb file",
                "Bits de permissão Unix do arquivo adb resolvido",
            );
            f.evidence.push(Evidence::new("stat", "path", path));
            f.evidence.push(Evidence::new(
                "stat",
                "mode",
                format!("{:04o}", mode & 0o7777),
            ));
            f.impact = lang.pick(
                "Another local user or process could replace adb with a malicious program that ZittoDB (and you) would then run.",
                "Outro usuário ou processo local poderia substituir o adb por um programa malicioso que o ZittoDB (e você) executaria.",
            );
            f.recommendation = lang.pick(
                "Remove the write bit for group/others (`chmod go-w <path>`) or reinstall the tools in a directory you own.",
                "Remova a permissão de escrita de grupo/outros (`chmod go-w <caminho>`) ou reinstale as ferramentas em uma pasta sua.",
            );
            f.reversible = Some(true);
            c.done("env.adb_permissions", CheckStatus::Found, None);
        }
        Some(_) => c.done("env.adb_permissions", CheckStatus::Passed, None),
    }

    let order = |id: &str| {
        ENV_CHECKS
            .iter()
            .position(|(cid, _, _)| *cid == id)
            .unwrap_or(usize::MAX)
    };
    c.checks.sort_by_key(|ch| order(&ch.id));
    (c.findings, c.checks)
}

// ---------------------------------------------------------------------------
// collection
// ---------------------------------------------------------------------------

#[cfg(unix)]
fn is_executable_file(p: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata()
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable_file(p: &std::path::Path) -> bool {
    p.is_file()
}

#[cfg(unix)]
fn file_mode(p: &str) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p).ok().map(|m| m.permissions().mode())
}

#[cfg(not(unix))]
fn file_mode(_p: &str) -> Option<u32> {
    None
}

/// Every distinct `adb` executable found in the given PATH value (max 6).
pub fn find_adb_on_path(path_var: &std::ffi::OsStr) -> Vec<String> {
    let name = if cfg!(windows) { "adb.exe" } else { "adb" };
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for dir in std::env::split_paths(path_var) {
        let cand = dir.join(name);
        if !is_executable_file(&cand) {
            continue;
        }
        let canon = std::fs::canonicalize(&cand).unwrap_or(cand.clone());
        if seen.insert(canon) {
            out.push(cand.to_string_lossy().to_string());
        }
        if out.len() >= 6 {
            break;
        }
    }
    out
}

pub fn collect_env(tools: &[ToolStatus], path_var: Option<&std::ffi::OsStr>) -> EnvSnapshot {
    let adb_path = tools
        .iter()
        .find(|t| t.name == "adb" && t.found)
        .and_then(|t| t.path.clone());
    let mut snap = EnvSnapshot {
        tools: tools
            .iter()
            .map(|t| ToolInfo {
                name: t.name.clone(),
                found: t.found,
                path: t.path.clone(),
            })
            .collect(),
        adb_platform_version: None,
        adb_version_error: None,
        server_status: ServerStatus::NotChecked("adb not found".into()),
        env_vars: BTreeMap::new(),
        adb_candidates: Vec::new(),
        adb_mode: None,
    };
    if let Ok(v) = std::env::var("ANDROID_SERIAL") {
        snap.env_vars.insert("ANDROID_SERIAL".into(), v);
    }
    let Some(adb) = adb_path else {
        return snap;
    };

    snap.adb_mode = file_mode(&adb);
    let client = AdbClient::new(&adb);
    match client.run_ok(vec!["version".into()], Duration::from_secs(5)) {
        Ok(t) => {
            snap.adb_platform_version = parse_adb_platform_version(&t);
            if snap.adb_platform_version.is_none() {
                snap.adb_version_error = Some("no `Version` line in `adb version` output".into());
            }
        }
        Err(e) => snap.adb_version_error = Some(e.to_string()),
    }
    snap.server_status = match client.run(vec!["server-status".into()], Duration::from_secs(5)) {
        Ok(out) if out.success() && !out.timed_out => {
            ServerStatus::Reported(parse_server_status(&out.text()))
        }
        Ok(out) => ServerStatus::Unsupported(if out.timed_out {
            "timeout".to_string()
        } else {
            out.stderr
                .trim()
                .lines()
                .next()
                .unwrap_or("non-zero exit")
                .to_string()
        }),
        Err(e) => ServerStatus::Unsupported(e.to_string()),
    };
    if let Some(p) = path_var {
        for cand in find_adb_on_path(p) {
            let platform_version = AdbClient::new(&cand)
                .run_ok(vec!["version".into()], Duration::from_secs(5))
                .ok()
                .and_then(|t| parse_adb_platform_version(&t));
            snap.adb_candidates.push(AdbCandidate {
                path: cand,
                platform_version,
            });
        }
    }
    snap
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tools(adb: bool, fastboot: bool, scrcpy: bool) -> Vec<ToolInfo> {
        [("adb", adb), ("fastboot", fastboot), ("scrcpy", scrcpy)]
            .into_iter()
            .map(|(n, f)| ToolInfo {
                name: n.into(),
                found: f,
                path: f.then(|| format!("/opt/{n}")),
            })
            .collect()
    }

    fn healthy() -> EnvSnapshot {
        EnvSnapshot {
            tools: tools(true, true, true),
            adb_platform_version: Some("37.0.1".into()),
            adb_version_error: None,
            server_status: ServerStatus::Reported(parse_server_status(
                "version: \"37.0.1\"\nmdns_enabled: true\nmdns_backend: LIBADBMDNS\n",
            )),
            env_vars: BTreeMap::new(),
            adb_candidates: vec![AdbCandidate {
                path: "/opt/adb".into(),
                platform_version: Some("37.0.1".into()),
            }],
            adb_mode: Some(0o755),
        }
    }

    #[test]
    fn parses_adb_version_variants() {
        assert_eq!(
            parse_adb_platform_version("Android Debug Bridge version 1.0.41\nVersion 34.0.5-10900879\nInstalled as /usr/bin/adb\n").as_deref(),
            Some("34.0.5")
        );
        assert_eq!(
            parse_adb_platform_version("Version 1:34.0.4-debian\n").as_deref(),
            Some("34.0.4")
        );
        assert_eq!(
            parse_adb_platform_version("Android Debug Bridge version 1.0.41\n"),
            None
        );
        assert_eq!(parse_adb_platform_version("Version abc\n"), None);
    }

    #[test]
    fn parses_server_status_lines() {
        let m = parse_server_status("version: \"37.0.0\"\nmdns_enabled: false\nlog_absolute_path: /tmp/a b/adb.log\nnot a kv line\n");
        assert_eq!(m.get("version").map(String::as_str), Some("37.0.0"));
        assert_eq!(m.get("mdns_enabled").map(String::as_str), Some("false"));
        assert_eq!(
            m.get("log_absolute_path").map(String::as_str),
            Some("/tmp/a b/adb.log")
        );
        assert_eq!(m.len(), 3);
    }

    #[test]
    fn version_comparison_is_numeric() {
        assert_eq!(cmp_version("34.0.5", "37.0.0"), Ordering::Less);
        assert_eq!(cmp_version("37.0.0", "37.0.0"), Ordering::Equal);
        assert_eq!(
            cmp_version("9.0.0", "37.0.0"),
            Ordering::Less,
            "9 < 37 numerically, not lexically"
        );
        assert_eq!(cmp_version("37", "37.0.1"), Ordering::Less);
        assert_eq!(cmp_version("40.1.0", "37.9.9"), Ordering::Greater);
    }

    #[test]
    fn healthy_environment_has_no_findings() {
        let (f, c) = evaluate_env(&healthy(), Lang::En);
        assert!(f.is_empty(), "{f:?}");
        assert_eq!(c.len(), ENV_CHECKS.len());
        assert!(c.iter().all(|x| x.status == CheckStatus::Passed), "{c:?}");
    }

    #[test]
    fn missing_adb_is_high_and_skips_dependent_checks() {
        let mut s = healthy();
        s.tools = tools(false, true, true);
        s.adb_mode = None;
        s.server_status = ServerStatus::NotChecked("adb not found".into());
        let (f, c) = evaluate_env(&s, Lang::En);
        assert_eq!(f.len(), 1);
        assert_eq!(
            (f[0].rule_id.as_str(), f[0].severity),
            ("ENV-ADB-MISSING", Severity::High)
        );
        for id in [
            "env.adb_server",
            "env.adb_duplicates",
            "env.adb_permissions",
        ] {
            assert_eq!(
                c.iter().find(|x| x.id == id).unwrap().status,
                CheckStatus::Skipped,
                "{id}"
            );
        }
    }

    #[test]
    fn optional_tools_are_info_only() {
        let mut s = healthy();
        s.tools = tools(true, false, false);
        let (f, _) = evaluate_env(&s, Lang::En);
        assert_eq!(f.len(), 2);
        assert!(f.iter().all(|x| x.severity == Severity::Info));
    }

    #[test]
    fn old_platform_tools_is_informational_and_mentions_the_threshold() {
        let mut s = healthy();
        s.adb_platform_version = Some("34.0.5".into());
        let (f, _) = evaluate_env(&s, Lang::En);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].severity, Severity::Info);
        assert!(f[0].evidence.iter().any(|e| e.value == "37.0.0"));
    }

    #[test]
    fn unparsable_version_is_an_error_check_not_a_pass() {
        let mut s = healthy();
        s.adb_platform_version = None;
        s.adb_version_error = Some("boom".into());
        let c = evaluate_env(&s, Lang::En).1;
        let adb = c.iter().find(|x| x.id == "env.adb").unwrap();
        assert_eq!(
            (adb.status, adb.detail.as_deref()),
            (CheckStatus::Error, Some("boom"))
        );
    }

    #[test]
    fn server_status_unsupported_is_skipped_and_mdns_off_is_low() {
        let mut s = healthy();
        s.server_status = ServerStatus::Unsupported("unknown command".into());
        let c = evaluate_env(&s, Lang::En).1;
        assert_eq!(
            c.iter().find(|x| x.id == "env.adb_server").unwrap().status,
            CheckStatus::Skipped
        );
        s.server_status = ServerStatus::Reported(parse_server_status("mdns_enabled: false\n"));
        let (f, _) = evaluate_env(&s, Lang::En);
        assert_eq!(
            (f[0].rule_id.as_str(), f[0].severity),
            ("ENV-ADB-MDNS-OFF", Severity::Low)
        );
    }

    #[test]
    fn android_serial_and_duplicates_and_permissions() {
        let mut s = healthy();
        s.env_vars.insert("ANDROID_SERIAL".into(), "ABC".into());
        s.adb_candidates.push(AdbCandidate {
            path: "/usr/bin/adb".into(),
            platform_version: Some("34.0.4".into()),
        });
        s.adb_mode = Some(0o775);
        let (f, _) = evaluate_env(&s, Lang::En);
        let by = |r: &str| f.iter().find(|x| x.rule_id == r).unwrap();
        assert_eq!(by("ENV-ANDROID-SERIAL-SET").severity, Severity::Low);
        assert_eq!(by("ENV-ADB-MULTIPLE").severity, Severity::Medium);
        assert_eq!(by("ENV-ADB-WRITABLE").severity, Severity::Medium);
        assert!(by("ENV-ADB-WRITABLE")
            .evidence
            .iter()
            .any(|e| e.value == "0775"));
        // same version in both places → informational only
        s.adb_candidates[1].platform_version = Some("37.0.1".into());
        let (f, _) = evaluate_env(&s, Lang::En);
        assert_eq!(
            f.iter()
                .find(|x| x.rule_id == "ENV-ADB-MULTIPLE")
                .unwrap()
                .severity,
            Severity::Info
        );
    }

    #[cfg(unix)]
    #[test]
    fn finds_distinct_adb_binaries_on_path() {
        use std::os::unix::fs::PermissionsExt;
        let base = std::env::temp_dir().join(format!("zittodb-path-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (a, b, c) = (base.join("a"), base.join("b"), base.join("c"));
        for d in [&a, &b, &c] {
            std::fs::create_dir_all(d).unwrap();
        }
        for d in [&a, &b] {
            let f = d.join("adb");
            std::fs::write(&f, "#!/bin/sh\n").unwrap();
            std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        // not executable → ignored
        std::fs::write(c.join("adb"), "x").unwrap();
        std::fs::set_permissions(c.join("adb"), std::fs::Permissions::from_mode(0o644)).unwrap();
        // symlink to the same file → deduplicated
        std::os::unix::fs::symlink(a.join("adb"), base.join("adb")).unwrap();

        let path = std::env::join_paths([&a, &b, &c, &base]).unwrap();
        let found = find_adb_on_path(&path);
        assert_eq!(found.len(), 2, "{found:?}");
        std::fs::remove_dir_all(&base).ok();
    }
}
