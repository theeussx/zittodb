//! Scanner Center data model.
//!
//! A scan produces a [`ScanReport`]: the list of [`Finding`]s PLUS the list of
//! checks that were attempted ([`CheckResult`]). The second list matters:
//! `passed` means "looked, found nothing", while `skipped`/`error` mean
//! "NOT evaluated". Absence of findings is never proof of safety.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const SCAN_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Low => "low",
            Severity::Medium => "medium",
            Severity::High => "high",
            Severity::Critical => "critical",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

impl Confidence {
    pub fn as_str(self) -> &'static str {
        match self {
            Confidence::Low => "low",
            Confidence::Medium => "medium",
            Confidence::High => "high",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScanTarget {
    /// The computer: adb / fastboot / scrcpy and their configuration.
    Environment,
    /// One connected Android device.
    Device,
}

impl ScanTarget {
    pub fn as_str(self) -> &'static str {
        match self {
            ScanTarget::Environment => "environment",
            ScanTarget::Device => "device",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FindingStatus {
    Open,
    Resolved,
    Ignored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    /// Evaluated, nothing found.
    Passed,
    /// Evaluated, at least one finding.
    Found,
    /// Not evaluated: the data does not exist on this target.
    Skipped,
    /// Not evaluated: the collection failed.
    Error,
}

/// Language of the human-readable texts in a report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Pt,
    En,
}

impl Lang {
    pub fn from_tag(tag: &str) -> Lang {
        if tag.to_ascii_lowercase().starts_with("pt") {
            Lang::Pt
        } else {
            Lang::En
        }
    }

    pub fn pick(self, en: &str, pt: &str) -> String {
        match self {
            Lang::En => en.to_string(),
            Lang::Pt => pt.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Evidence {
    /// Where the value was read (`getprop`, `dumpsys battery`, `env`, …).
    pub source: String,
    pub key: String,
    pub value: String,
}

impl Evidence {
    pub fn new(source: &str, key: &str, value: impl Into<String>) -> Evidence {
        Evidence {
            source: source.to_string(),
            key: key.to_string(),
            value: value.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    /// Stable across runs: derived from rule + target + subject, NOT from the
    /// evidence values (so a changing number does not look like a new finding).
    pub id: String,
    pub rule_id: String,
    pub title: String,
    pub severity: Severity,
    pub confidence: Confidence,
    pub target: ScanTarget,
    /// Device serial, tool name, … the finding is about.
    pub subject: String,
    pub category: String,
    /// How it was detected (method), so the user can reproduce it.
    pub detection: String,
    pub evidence: Vec<Evidence>,
    pub impact: String,
    pub recommendation: String,
    /// Reserved: no finding offers an automatic fix yet.
    pub action: Option<String>,
    pub reversible: Option<bool>,
    pub status: FindingStatus,
}

impl Finding {
    pub fn new(
        rule_id: &str,
        target: ScanTarget,
        subject: &str,
        severity: Severity,
        confidence: Confidence,
        category: &str,
    ) -> Finding {
        Finding {
            id: finding_id(rule_id, target, subject),
            rule_id: rule_id.to_string(),
            title: String::new(),
            severity,
            confidence,
            target,
            subject: subject.to_string(),
            category: category.to_string(),
            detection: String::new(),
            evidence: Vec::new(),
            impact: String::new(),
            recommendation: String::new(),
            action: None,
            reversible: None,
            status: FindingStatus::Open,
        }
    }
}

/// `ZF-` + 12 hex chars of SHA-256(rule | target | subject).
pub fn finding_id(rule_id: &str, target: ScanTarget, subject: &str) -> String {
    let mut h = Sha256::new();
    h.update(rule_id.as_bytes());
    h.update([0u8]);
    h.update(target.as_str().as_bytes());
    h.update([0u8]);
    h.update(subject.as_bytes());
    let digest = h.finalize();
    let hex: String = digest.iter().take(6).map(|b| format!("{b:02x}")).collect();
    format!("ZF-{hex}")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckResult {
    pub id: String,
    pub title: String,
    pub status: CheckStatus,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub total: u32,
    pub by_severity: BTreeMap<String, u32>,
    pub checks_evaluated: u32,
    pub checks_not_evaluated: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    pub schema_version: u32,
    pub scan_id: String,
    pub target: ScanTarget,
    pub subject: Option<String>,
    pub started_at: i64,
    pub finished_at: i64,
    pub lang: String,
    pub checks: Vec<CheckResult>,
    pub findings: Vec<Finding>,
    pub summary: Summary,
    /// What this scan can NOT tell you.
    pub limitations: Vec<String>,
}

/// Merges findings with the same id (keeps the highest severity and the union
/// of the evidence) and sorts by severity (desc) then id.
pub fn dedupe_and_sort(findings: Vec<Finding>) -> Vec<Finding> {
    let mut out: Vec<Finding> = Vec::new();
    for f in findings {
        if let Some(existing) = out.iter_mut().find(|e| e.id == f.id) {
            if f.severity > existing.severity {
                existing.severity = f.severity;
            }
            for ev in f.evidence {
                if !existing.evidence.contains(&ev) {
                    existing.evidence.push(ev);
                }
            }
        } else {
            out.push(f);
        }
    }
    out.sort_by(|a, b| b.severity.cmp(&a.severity).then_with(|| a.id.cmp(&b.id)));
    out
}

pub fn summarize(findings: &[Finding], checks: &[CheckResult]) -> Summary {
    let mut by_severity: BTreeMap<String, u32> = BTreeMap::new();
    for f in findings {
        *by_severity
            .entry(f.severity.as_str().to_string())
            .or_insert(0) += 1;
    }
    Summary {
        total: findings.len() as u32,
        by_severity,
        checks_evaluated: checks
            .iter()
            .filter(|c| matches!(c.status, CheckStatus::Passed | CheckStatus::Found))
            .count() as u32,
        checks_not_evaluated: checks
            .iter()
            .filter(|c| matches!(c.status, CheckStatus::Skipped | CheckStatus::Error))
            .count() as u32,
    }
}

/// Difference between two reports of the same target/subject, by finding id.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanDiff {
    pub new: Vec<Finding>,
    pub resolved: Vec<Finding>,
    pub changed: Vec<ScanChange>,
    pub unchanged: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanChange {
    pub id: String,
    pub title: String,
    pub from: Severity,
    pub to: Severity,
}

/// `before` → `after`. A finding that disappeared is only reported as
/// `resolved` if its check was actually evaluated in `after`; otherwise we
/// cannot claim it was fixed.
pub fn diff_reports(before: &ScanReport, after: &ScanReport) -> ScanDiff {
    let mut d = ScanDiff::default();
    for f in &after.findings {
        match before.findings.iter().find(|b| b.id == f.id) {
            None => d.new.push(f.clone()),
            Some(b) if b.severity != f.severity => d.changed.push(ScanChange {
                id: f.id.clone(),
                title: f.title.clone(),
                from: b.severity,
                to: f.severity,
            }),
            Some(_) => d.unchanged += 1,
        }
    }
    for b in &before.findings {
        if after.findings.iter().any(|f| f.id == b.id) {
            continue;
        }
        let evaluated_now = after
            .checks
            .iter()
            .find(|c| c.id == check_for_rule(&b.rule_id))
            .map(|c| matches!(c.status, CheckStatus::Passed | CheckStatus::Found))
            .unwrap_or(false);
        if evaluated_now {
            d.resolved.push(b.clone());
        }
    }
    d
}

/// Check id that evaluates a rule: the rule id lower-cased with `-` → `.` is
/// not derivable in general, so rules register it explicitly.
pub fn check_for_rule(rule_id: &str) -> String {
    match rule_id {
        "ENV-ADB-MISSING" | "ENV-ADB-VERSION-OLD" => "env.adb",
        "ENV-FASTBOOT-MISSING" => "env.fastboot",
        "ENV-SCRCPY-MISSING" => "env.scrcpy",
        "ENV-ADB-MDNS-OFF" => "env.adb_server",
        "ENV-ANDROID-SERIAL-SET" => "env.android_serial",
        "ENV-ADB-MULTIPLE" => "env.adb_duplicates",
        "ENV-ADB-WRITABLE" => "env.adb_permissions",
        "DEV-UNAUTHORIZED" | "DEV-OFFLINE" => "dev.connection",
        "DEV-ADB-TCP" => "dev.adb_tcp",
        "DEV-BOOTLOADER-UNLOCKED" => "dev.boot_state",
        "DEV-DEBUGGABLE-BUILD" => "dev.debuggable",
        "DEV-PATCH-OLD" => "dev.patch_age",
        "DEV-SELINUX-PERMISSIVE" => "dev.selinux",
        "DEV-LOW-STORAGE" => "dev.storage",
        "DEV-BATTERY-HEALTH" | "DEV-BATTERY-HOT" => "dev.battery",
        "DEV-SCRCPY-LIMITS" => "dev.scrcpy",
        "DEV-NOT-READY" => "dev.connection",
        other => other,
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(rule: &str, sev: Severity) -> Finding {
        Finding::new(rule, ScanTarget::Device, "S1", sev, Confidence::High, "t")
    }

    #[test]
    fn ids_are_stable_and_depend_on_rule_target_and_subject_only() {
        let a = finding_id("DEV-X", ScanTarget::Device, "S1");
        assert_eq!(a, finding_id("DEV-X", ScanTarget::Device, "S1"));
        assert!(a.starts_with("ZF-") && a.len() == 15);
        assert_ne!(a, finding_id("DEV-X", ScanTarget::Device, "S2"));
        assert_ne!(a, finding_id("DEV-Y", ScanTarget::Device, "S1"));
        assert_ne!(a, finding_id("DEV-X", ScanTarget::Environment, "S1"));
    }

    #[test]
    fn dedupe_merges_evidence_keeps_max_severity_and_sorts() {
        let mut a = f("A", Severity::Low);
        a.evidence.push(Evidence::new("s", "k1", "v1"));
        let mut a2 = f("A", Severity::High);
        a2.evidence.push(Evidence::new("s", "k2", "v2"));
        a2.evidence.push(Evidence::new("s", "k1", "v1"));
        let b = f("B", Severity::Critical);
        let c = f("C", Severity::Info);
        let out = dedupe_and_sort(vec![c, a, b, a2]);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].rule_id, "B");
        assert_eq!(out[1].rule_id, "A");
        assert_eq!(out[1].severity, Severity::High);
        assert_eq!(out[1].evidence.len(), 2);
        assert_eq!(out[2].rule_id, "C");
    }

    fn report(findings: Vec<Finding>, checks: Vec<(&str, CheckStatus)>) -> ScanReport {
        let checks: Vec<CheckResult> = checks
            .into_iter()
            .map(|(id, status)| CheckResult {
                id: id.into(),
                title: id.into(),
                status,
                detail: None,
            })
            .collect();
        ScanReport {
            schema_version: SCAN_SCHEMA_VERSION,
            scan_id: "x".into(),
            target: ScanTarget::Device,
            subject: Some("S1".into()),
            started_at: 0,
            finished_at: 0,
            lang: "en".into(),
            summary: summarize(&findings, &checks),
            checks,
            findings,
            limitations: vec![],
        }
    }

    #[test]
    fn diff_reports_new_changed_resolved_and_unchanged() {
        let before = report(
            vec![
                f("DEV-PATCH-OLD", Severity::Medium),
                f("DEV-LOW-STORAGE", Severity::Medium),
                f("DEV-ADB-TCP", Severity::Medium),
                f("DEV-SELINUX-PERMISSIVE", Severity::High),
            ],
            vec![],
        );
        let after = report(
            vec![
                f("DEV-PATCH-OLD", Severity::High),       // changed
                f("DEV-LOW-STORAGE", Severity::Medium),   // unchanged
                f("DEV-DEBUGGABLE-BUILD", Severity::Low), // new
            ],
            vec![
                ("dev.adb_tcp", CheckStatus::Passed), // evaluated → resolved
                ("dev.selinux", CheckStatus::Error),  // NOT evaluated → not resolved
            ],
        );
        let d = diff_reports(&before, &after);
        assert_eq!(d.new.len(), 1);
        assert_eq!(d.new[0].rule_id, "DEV-DEBUGGABLE-BUILD");
        assert_eq!(d.changed.len(), 1);
        assert_eq!(
            (d.changed[0].from, d.changed[0].to),
            (Severity::Medium, Severity::High)
        );
        assert_eq!(d.unchanged, 1);
        assert_eq!(d.resolved.len(), 1);
        assert_eq!(d.resolved[0].rule_id, "DEV-ADB-TCP");
    }

    #[test]
    fn summary_separates_evaluated_from_not_evaluated_checks() {
        let r = report(
            vec![f("A", Severity::High), f("B", Severity::High)],
            vec![
                ("a", CheckStatus::Found),
                ("b", CheckStatus::Passed),
                ("c", CheckStatus::Skipped),
                ("d", CheckStatus::Error),
            ],
        );
        assert_eq!(r.summary.total, 2);
        assert_eq!(r.summary.by_severity.get("high"), Some(&2));
        assert_eq!(r.summary.checks_evaluated, 2);
        assert_eq!(r.summary.checks_not_evaluated, 2);
    }
}
