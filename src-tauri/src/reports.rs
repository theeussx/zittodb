//! Stable diagnostic report contract and pure formatters (P0).
//!
//! This module deliberately has no device or filesystem side effects. Collection
//! and secure writing are integration concerns built on top of this DTO.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const REPORT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SectionStatus {
    Ok,
    Unavailable,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReportError {
    pub code: String,
    pub details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReportSection {
    pub status: SectionStatus,
    pub data: Option<Value>,
    pub error: Option<ReportError>,
}

impl ReportSection {
    pub fn ok(data: Value) -> Self {
        Self {
            status: SectionStatus::Ok,
            data: Some(data),
            error: None,
        }
    }

    pub fn unavailable(code: impl Into<String>, details: impl Into<String>) -> Self {
        Self {
            status: SectionStatus::Unavailable,
            data: None,
            error: Some(ReportError {
                code: code.into(),
                details: details.into(),
            }),
        }
    }

    pub fn error(code: impl Into<String>, details: impl Into<String>) -> Self {
        Self {
            status: SectionStatus::Error,
            data: None,
            error: Some(ReportError {
                code: code.into(),
                details: details.into(),
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReportDevice {
    pub serial: Option<String>,
    pub state: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReportTool {
    pub name: String,
    pub found: bool,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticReport {
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,
    pub generated_at: String,
    pub app_version: String,
    pub device: ReportDevice,
    pub tools: Vec<ReportTool>,
    pub sections: BTreeMap<String, ReportSection>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReportPrivacy {
    pub include_serial: bool,
    pub include_network: bool,
}

impl Default for ReportPrivacy {
    fn default() -> Self {
        Self {
            include_serial: false,
            include_network: false,
        }
    }
}

impl DiagnosticReport {
    pub fn new(
        generated_at: impl Into<String>,
        app_version: impl Into<String>,
        serial: Option<String>,
        state: Option<String>,
    ) -> Self {
        Self {
            schema_version: REPORT_SCHEMA_VERSION,
            generated_at: generated_at.into(),
            app_version: app_version.into(),
            device: ReportDevice { serial, state },
            tools: Vec::new(),
            sections: BTreeMap::new(),
            limitations: Vec::new(),
        }
    }

    /// Applies the user's privacy choice in place. The default is conservative.
    pub fn apply_privacy(&mut self, privacy: ReportPrivacy) {
        if !privacy.include_serial {
            self.device.serial = self.device.serial.as_deref().map(mask_serial);
        }
        if !privacy.include_network {
            for (name, section) in &mut self.sections {
                if name == "network" {
                    if let Some(data) = section.data.as_mut() {
                        redact_network_value(data);
                    }
                }
            }
        }
    }
}

pub fn to_json(report: &DiagnosticReport) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(report)
}

pub fn to_markdown(report: &DiagnosticReport) -> String {
    let mut out = String::new();
    out.push_str("# ZittoDB — relatório de diagnóstico\n\n");
    out.push_str(&format!("- **Schema:** {}\n", report.schema_version));
    out.push_str(&format!("- **Gerado em:** {}\n", report.generated_at));
    out.push_str(&format!(
        "- **Versão do ZittoDB:** {}\n",
        report.app_version
    ));
    out.push_str(&format!(
        "- **Dispositivo:** {}\n",
        report.device.serial.as_deref().unwrap_or("indisponível")
    ));
    out.push_str(&format!(
        "- **Estado ADB:** {}\n\n",
        report.device.state.as_deref().unwrap_or("indisponível")
    ));

    out.push_str("## Ferramentas\n\n| Ferramenta | Encontrada | Versão |\n|---|---:|---|\n");
    for tool in &report.tools {
        out.push_str(&format!(
            "| {} | {} | {} |\n",
            tool.name,
            if tool.found { "sim" } else { "não" },
            tool.version.as_deref().unwrap_or("indisponível")
        ));
    }

    out.push_str("\n## Seções\n\n");
    for (name, section) in &report.sections {
        out.push_str(&format!(
            "### {}\n\n- **Status:** {:?}\n",
            name, section.status
        ));
        if let Some(error) = &section.error {
            out.push_str(&format!(
                "- **Erro:** `{}` — {}\n",
                error.code, error.details
            ));
        }
        if let Some(data) = &section.data {
            out.push_str("\n```json\n");
            out.push_str(&serde_json::to_string_pretty(data).unwrap_or_else(|_| "null".into()));
            out.push_str("\n```\n");
        }
        out.push('\n');
    }

    if !report.limitations.is_empty() {
        out.push_str("## Limitações da coleta\n\n");
        for limitation in &report.limitations {
            out.push_str(&format!("- {limitation}\n"));
        }
    }
    out
}

pub fn next_available_filename(
    dir: &Path,
    stem: &str,
    extension: &str,
    exists: impl Fn(&Path) -> bool,
) -> PathBuf {
    let extension = extension.trim_start_matches('.');
    let base = format!("{stem}.{extension}");
    let candidate = dir.join(&base);
    if !exists(&candidate) {
        return candidate;
    }
    for index in 1..=999 {
        let candidate = dir.join(format!("{stem}-{index}.{extension}"));
        if !exists(&candidate) {
            return candidate;
        }
    }
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_else(|_| u128::from(std::process::id()));
    dir.join(format!("{stem}-overflow-{nonce}.{extension}"))
}

fn mask_serial(serial: &str) -> String {
    let tail: String = serial
        .chars()
        .rev()
        .take(4)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    if tail.is_empty() {
        "[redacted]".into()
    } else {
        format!("…{tail}")
    }
}

fn redact_network_value(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for (key, child) in object.iter_mut() {
                let lower = key.to_ascii_lowercase();
                if matches!(lower.as_str(), "ip" | "mac" | "dns" | "gateway")
                    || lower.contains("address")
                {
                    redact_value(child);
                } else {
                    redact_network_value(child);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                redact_network_value(item);
            }
        }
        _ => {}
    }
}

fn redact_value(value: &mut Value) {
    match value {
        Value::Array(items) => {
            for item in items {
                *item = Value::String("[redacted]".into());
            }
        }
        _ => *value = Value::String("[redacted]".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> DiagnosticReport {
        let mut report = DiagnosticReport::new(
            "2026-10-09T18:00:00Z",
            "0.1.6",
            Some("23021RAA2Y".into()),
            Some("connected".into()),
        );
        report.tools.push(ReportTool {
            name: "adb".into(),
            found: true,
            version: Some("1.0.41".into()),
        });
        report.sections.insert(
            "network".into(),
            ReportSection::ok(json!({
                "interfaces": [{"ip": "192.168.1.50", "mac": "aa:bb:cc:dd:ee:ff"}],
                "gateway": "192.168.1.1",
                "dns": ["1.1.1.1"]
            })),
        );
        report.sections.insert(
            "battery".into(),
            ReportSection::unavailable("DEVICE_OFFLINE", "device is offline"),
        );
        report
    }

    #[test]
    fn schema_and_partial_sections_are_stable() {
        let report = sample();
        let json = to_json(&report).expect("serializes");
        assert!(json.contains("\"schemaVersion\": 1"));
        assert!(json.contains("\"status\": \"unavailable\""));
        assert!(json.contains("DEVICE_OFFLINE"));
    }

    #[test]
    fn default_privacy_masks_serial_and_network() {
        let mut report = sample();
        report.apply_privacy(ReportPrivacy::default());
        assert_eq!(report.device.serial.as_deref(), Some("…AA2Y"));
        let network = report.sections["network"]
            .data
            .as_ref()
            .expect("network data");
        assert_eq!(network["gateway"], "[redacted]");
        assert_eq!(network["interfaces"][0]["ip"], "[redacted]");
        assert_eq!(network["dns"][0], "[redacted]");
    }

    #[test]
    fn explicit_privacy_keeps_identifiers() {
        let mut report = sample();
        report.apply_privacy(ReportPrivacy {
            include_serial: true,
            include_network: true,
        });
        assert_eq!(report.device.serial.as_deref(), Some("23021RAA2Y"));
        assert_eq!(
            report.sections["network"].data.as_ref().unwrap()["gateway"],
            "192.168.1.1"
        );
    }

    #[test]
    fn markdown_has_sections_and_no_invented_unavailable_values() {
        let markdown = to_markdown(&sample());
        assert!(markdown.contains("## Seções"));
        assert!(markdown.contains("### battery"));
        assert!(markdown.contains("DEVICE_OFFLINE"));
    }

    #[test]
    fn filename_never_overwrites_and_increments() {
        let dir = Path::new("/tmp");
        let selected = next_available_filename(dir, "report", "json", |_| false);
        assert_eq!(selected, dir.join("report.json"));

        let selected = next_available_filename(dir, "report", "json", |path| {
            matches!(
                path.file_name().and_then(|n| n.to_str()),
                Some("report.json" | "report-1.json")
            )
        });
        assert_eq!(selected, dir.join("report-2.json"));
    }
}
