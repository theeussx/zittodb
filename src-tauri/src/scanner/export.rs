//! JSON and Markdown export of a [`ScanReport`].

use super::model::{CheckStatus, Lang, ScanReport, Severity};

pub fn to_json(report: &ScanReport) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(report)
}

fn md_cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

fn fmt_ts(ts: i64) -> String {
    // Civil UTC date-time without extra dependencies.
    let days = ts.div_euclid(86_400);
    let secs = ts.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02} UTC",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

pub fn to_markdown(r: &ScanReport) -> String {
    let lang = Lang::from_tag(&r.lang);
    let t = |en: &str, pt: &str| lang.pick(en, pt);
    let mut o = String::new();

    o.push_str(&format!(
        "# {}\n\n",
        t("ZittoDB scan report", "Relatório de varredura do ZittoDB")
    ));
    o.push_str(&format!(
        "- **{}:** `{}`\n",
        t("Scan", "Varredura"),
        r.scan_id
    ));
    o.push_str(&format!(
        "- **{}:** {}\n",
        t("Target", "Alvo"),
        r.target.as_str()
    ));
    if let Some(s) = &r.subject {
        o.push_str(&format!("- **{}:** `{}`\n", t("Subject", "Assunto"), s));
    }
    o.push_str(&format!(
        "- **{}:** {}\n",
        t("Started", "Início"),
        fmt_ts(r.started_at)
    ));
    o.push_str(&format!(
        "- **{}:** {}\n\n",
        t("Finished", "Fim"),
        fmt_ts(r.finished_at)
    ));

    o.push_str(&format!("## {}\n\n", t("Summary", "Resumo")));
    o.push_str(&format!(
        "- {}: **{}**\n",
        t("Findings", "Achados"),
        r.summary.total
    ));
    for sev in [
        Severity::Critical,
        Severity::High,
        Severity::Medium,
        Severity::Low,
        Severity::Info,
    ] {
        if let Some(n) = r.summary.by_severity.get(sev.as_str()) {
            o.push_str(&format!("  - {}: {}\n", sev.as_str(), n));
        }
    }
    o.push_str(&format!(
        "- {}: {} · {}: {}\n\n",
        t("Checks evaluated", "Verificações avaliadas"),
        r.summary.checks_evaluated,
        t("not evaluated", "não avaliadas"),
        r.summary.checks_not_evaluated
    ));
    o.push_str(&format!(
        "> {}\n\n",
        t(
            "No findings does not mean secure: see the checks that were not evaluated and the limitations below.",
            "Nenhum achado não significa seguro: veja as verificações não avaliadas e as limitações abaixo."
        )
    ));

    o.push_str(&format!("## {}\n\n", t("Checks", "Verificações")));
    o.push_str(&format!(
        "| {} | {} | {} |\n|---|---|---|\n",
        t("Check", "Verificação"),
        t("Status", "Status"),
        t("Detail", "Detalhe")
    ));
    for c in &r.checks {
        let st = match c.status {
            CheckStatus::Passed => t("no finding", "sem achado"),
            CheckStatus::Found => t("finding", "achado"),
            CheckStatus::Skipped => t("NOT evaluated (skipped)", "NÃO avaliada (pulada)"),
            CheckStatus::Error => t("NOT evaluated (error)", "NÃO avaliada (erro)"),
        };
        o.push_str(&format!(
            "| {} | {} | {} |\n",
            md_cell(&c.title),
            st,
            md_cell(c.detail.as_deref().unwrap_or(""))
        ));
    }
    o.push('\n');

    o.push_str(&format!("## {}\n\n", t("Findings", "Achados")));
    if r.findings.is_empty() {
        o.push_str(&format!("{}\n\n", t("No findings.", "Nenhum achado.")));
    }
    for f in &r.findings {
        o.push_str(&format!(
            "### [{}] {} — {}\n\n",
            f.severity.as_str().to_uppercase(),
            f.title,
            f.id
        ));
        o.push_str(&format!(
            "- **{}:** `{}` · **{}:** {} · **{}:** {}\n",
            t("Rule", "Regra"),
            f.rule_id,
            t("Confidence", "Confiança"),
            f.confidence.as_str(),
            t("Category", "Categoria"),
            f.category
        ));
        o.push_str(&format!(
            "- **{}:** `{}`\n",
            t("Subject", "Assunto"),
            f.subject
        ));
        o.push_str(&format!(
            "- **{}:** {}\n",
            t("Detection", "Detecção"),
            f.detection
        ));
        if !f.evidence.is_empty() {
            o.push_str(&format!("- **{}:**\n", t("Evidence", "Evidências")));
            for e in &f.evidence {
                o.push_str(&format!(
                    "  - `{}` · `{}` = `{}`\n",
                    e.source,
                    e.key,
                    e.value.replace('`', "'")
                ));
            }
        }
        o.push_str(&format!("- **{}:** {}\n", t("Impact", "Impacto"), f.impact));
        o.push_str(&format!(
            "- **{}:** {}\n",
            t("Recommendation", "Recomendação"),
            f.recommendation
        ));
        if let Some(rev) = f.reversible {
            o.push_str(&format!(
                "- **{}:** {}\n",
                t("Reversible", "Reversível"),
                if rev {
                    t("yes", "sim")
                } else {
                    t("no", "não")
                }
            ));
        }
        o.push('\n');
    }

    o.push_str(&format!(
        "## {}\n\n",
        t("Limitations of this scan", "Limitações desta varredura")
    ));
    for l in &r.limitations {
        o.push_str(&format!("- {l}\n"));
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::model::*;

    fn sample() -> ScanReport {
        let mut f = Finding::new(
            "DEV-X",
            ScanTarget::Device,
            "S1",
            Severity::High,
            Confidence::High,
            "integrity",
        );
        f.title = "Pipe | and `tick`".into();
        f.evidence.push(Evidence::new("getprop", "k", "a`b"));
        let checks = vec![
            CheckResult {
                id: "a".into(),
                title: "A".into(),
                status: CheckStatus::Found,
                detail: None,
            },
            CheckResult {
                id: "b".into(),
                title: "B".into(),
                status: CheckStatus::Skipped,
                detail: Some("no prop".into()),
            },
        ];
        ScanReport {
            schema_version: SCAN_SCHEMA_VERSION,
            scan_id: "scan-1".into(),
            target: ScanTarget::Device,
            subject: Some("S1".into()),
            started_at: 1_792_000_000,
            finished_at: 1_792_000_005,
            lang: "en-US".into(),
            summary: summarize(&[f.clone()], &checks),
            checks,
            findings: vec![f],
            limitations: vec!["only what the device reports".into()],
        }
    }

    #[test]
    fn json_round_trips() {
        let r = sample();
        let back: ScanReport = serde_json::from_str(&to_json(&r).unwrap()).unwrap();
        assert_eq!(back, r);
    }

    #[test]
    fn markdown_states_not_evaluated_checks_and_limitations() {
        let md = to_markdown(&sample());
        assert!(md.contains("NOT evaluated (skipped)"));
        assert!(md.contains("No findings does not mean secure"));
        assert!(md.contains("[HIGH]"));
        assert!(md.contains("only what the device reports"));
        assert!(md.contains("2026-10-1"), "timestamp rendered: {md}");
        assert!(md.contains("a'b"), "backticks in evidence are neutralised");
    }

    #[test]
    fn markdown_follows_the_language() {
        let mut r = sample();
        r.lang = "pt-BR".into();
        assert!(to_markdown(&r).contains("Relatório de varredura"));
    }

    #[test]
    fn timestamps_are_formatted_in_utc() {
        assert_eq!(fmt_ts(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(fmt_ts(1_792_000_000), "2026-10-14 17:46:40 UTC");
    }
}
