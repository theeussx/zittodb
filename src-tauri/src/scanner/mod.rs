//! Scanner Center (spec: real checks with evidence, never canned results).
//!
//! * [`run_environment_scan`] — the computer's adb/fastboot/scrcpy setup;
//! * [`run_device_scan`] — one connected Android device (read-only).
//!
//! Every scan returns the findings AND the list of checks attempted, plus the
//! limitations of that scan type. "No findings" is never "secure".

pub mod device;
pub mod env;
pub mod export;
pub mod model;

use crate::adb::ToolStatus;
use crate::error::AppError;

use model::{
    dedupe_and_sort, summarize, CheckResult, Lang, ScanReport, ScanTarget, SCAN_SCHEMA_VERSION,
};

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn new_scan_id(ts: i64) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    format!("scan-{ts}-{:04x}", nanos & 0xffff)
}

fn assemble(
    target: ScanTarget,
    subject: Option<String>,
    started_at: i64,
    lang: Lang,
    lang_tag: &str,
    findings: Vec<model::Finding>,
    checks: Vec<CheckResult>,
    limitations: Vec<String>,
) -> ScanReport {
    let findings = dedupe_and_sort(findings);
    let summary = summarize(&findings, &checks);
    let _ = lang;
    ScanReport {
        schema_version: SCAN_SCHEMA_VERSION,
        scan_id: new_scan_id(started_at),
        target,
        subject,
        started_at,
        finished_at: now_unix(),
        lang: lang_tag.to_string(),
        checks,
        findings,
        summary,
        limitations,
    }
}

pub fn environment_limitations(lang: Lang) -> Vec<String> {
    vec![
        lang.pick(
            "Covers only adb, fastboot and scrcpy as found by ZittoDB, the `adb` executables on PATH and the ANDROID_SERIAL variable. It does not inspect the rest of the computer (OS patches, other software, malware).",
            "Cobre apenas adb, fastboot e scrcpy como o ZittoDB os encontra, os executáveis `adb` no PATH e a variável ANDROID_SERIAL. Não inspeciona o restante do computador (patches do SO, outros programas, malware).",
        ),
        lang.pick(
            "`adb server-status` may start the adb server if it is not running (the app starts it anyway).",
            "`adb server-status` pode iniciar o servidor adb se ele não estiver rodando (o app já o inicia de qualquer forma).",
        ),
        lang.pick(
            "Version checks compare against documented thresholds only; they do not look up known vulnerabilities in these tools.",
            "As checagens de versão comparam apenas com limites documentados; não consultam vulnerabilidades conhecidas dessas ferramentas.",
        ),
    ]
}

pub fn device_limitations(lang: Lang) -> Vec<String> {
    vec![
        lang.pick(
            "Read-only scan through adb as the `shell` user: it uses getprop, dumpsys battery, df and getenforce. It does not inspect installed apps, permissions, files, network traffic, kernel or firmware integrity.",
            "Varredura somente leitura via adb como usuário `shell`: usa getprop, dumpsys battery, df e getenforce. Não inspeciona apps instalados, permissões, arquivos, tráfego de rede, nem a integridade do kernel ou do firmware.",
        ),
        lang.pick(
            "Everything is what the device reports about itself. A compromised or modified system can report false values.",
            "Tudo é o que o próprio aparelho informa. Um sistema comprometido ou modificado pode informar valores falsos.",
        ),
        lang.pick(
            "Boot-state and battery-health values follow common Android conventions; they were validated against synthetic data in tests, not against physical devices.",
            "Os valores de estado de boot e saúde da bateria seguem convenções comuns do Android; foram validados com dados sintéticos nos testes, não com aparelhos físicos.",
        ),
        lang.pick(
            "The security-patch check measures age only. It does not identify specific vulnerabilities, and system updates delivered through Google Play are not reflected.",
            "A checagem do patch de segurança mede apenas a idade. Não identifica vulnerabilidades específicas, e atualizações do sistema entregues pelo Google Play não aparecem.",
        ),
    ]
}

pub fn run_environment_scan(
    tools: &[ToolStatus],
    path_var: Option<&std::ffi::OsStr>,
    lang_tag: &str,
) -> ScanReport {
    let lang = Lang::from_tag(lang_tag);
    let started = now_unix();
    let snap = env::collect_env(tools, path_var);
    let (findings, checks) = env::evaluate_env(&snap, lang);
    assemble(
        ScanTarget::Environment,
        None,
        started,
        lang,
        lang_tag,
        findings,
        checks,
        environment_limitations(lang),
    )
}

pub fn run_device_scan(adb: &str, serial: &str, lang_tag: &str) -> Result<ScanReport, AppError> {
    let lang = Lang::from_tag(lang_tag);
    let started = now_unix();
    let snap = device::collect_device(adb, serial)?;
    let (findings, checks) = device::evaluate_device(&snap, started, lang);
    Ok(assemble(
        ScanTarget::Device,
        Some(serial.to_string()),
        started,
        lang,
        lang_tag,
        findings,
        checks,
        device_limitations(lang),
    ))
}
