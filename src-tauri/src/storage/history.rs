//! Device connection history (spec §36).
//!
//! Stores ONLY non-sensitive metadata: model name, last seen time, last
//! state, connection kind. Never file content, never credentials.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::devices::Device;
use super::{ensure_private_dir, ensure_private_file};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct HistoryFile {
    entries: BTreeMap<String, HistoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub serial: String,
    pub model: Option<String>,
    pub last_state: Option<String>,
    pub last_seen_ms: Option<i64>,
    pub connection: Option<String>,
    pub alias: Option<String>,
    pub tags: Vec<String>,
    pub favorite: bool,
}

pub struct DeviceHistory {
    path: PathBuf,
    cache: Mutex<BTreeMap<String, HistoryEntry>>,
}

impl DeviceHistory {
    pub fn open(dir: &std::path::Path) -> DeviceHistory {
        ensure_private_dir(dir);
        let path = dir.join("devices-history.json");
        ensure_private_file(&path);
        let file: HistoryFile = fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        DeviceHistory {
            path,
            cache: Mutex::new(file.entries),
        }
    }

    /// Records a device sighting (called after each `list_devices`).
    pub fn record(&self, devices: &[Device]) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let mut map = self.cache.lock().unwrap();
        for d in devices {
            let e = map.entry(d.serial.clone()).or_insert_with(|| HistoryEntry {
                serial: d.serial.clone(),
                ..Default::default()
            });
            e.model = d.model.clone();
            e.last_state = Some(format!("{:?}", d.state).to_lowercase());
            e.last_seen_ms = Some(now);
            e.connection = Some(format!("{:?}", d.connection).to_lowercase());
        }
        drop(map);
        self.persist();
    }

    pub fn read(&self) -> Vec<HistoryEntry> {
        let mut v: Vec<HistoryEntry> = self.cache.lock().unwrap().values().cloned().collect();
        v.sort_by(|a, b| {
            b.last_seen_ms
                .unwrap_or(0)
                .cmp(&a.last_seen_ms.unwrap_or(0))
        });
        v
    }

    pub fn clear(&self) {
        self.cache.lock().unwrap().clear();
        self.persist();
    }

    pub fn update_metadata(
        &self,
        serial: &str,
        alias: Option<String>,
        tags: Vec<String>,
        favorite: bool,
    ) -> Result<(), crate::error::AppError> {
        crate::security::validate_serial(serial)?;
        if alias.as_ref().is_some_and(|v| v.chars().count() > 80) {
            return Err(crate::error::AppError::new(
                crate::error::ErrorCode::InvalidArgument,
                "alias too long (maximum 80 characters)",
            ));
        }
        if tags.len() > 12 || tags.iter().any(|tag| tag.is_empty() || tag.chars().count() > 32) {
            return Err(crate::error::AppError::new(
                crate::error::ErrorCode::InvalidArgument,
                "invalid tags (maximum 12 tags of 32 characters)",
            ));
        }
        let mut map = self.cache.lock().unwrap();
        let entry = map.entry(serial.to_string()).or_insert_with(|| HistoryEntry {
            serial: serial.to_string(),
            ..Default::default()
        });
        entry.alias = alias.filter(|v| !v.trim().is_empty());
        entry.tags = tags;
        entry.favorite = favorite;
        drop(map);
        self.persist();
        Ok(())
    }

    fn persist(&self) {
        let map = self.cache.lock().unwrap().clone();
        let file = HistoryFile { entries: map };
        if let Ok(json) = serde_json::to_string_pretty(&file) {
            fs::write(&self.path, json).ok();
            ensure_private_file(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::{ConnectionKind, DeviceState};

    #[test]
    fn records_and_clears() {
        let dir = std::env::temp_dir().join(format!("zittodb-hist-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let h = DeviceHistory::open(&dir);
        let d = Device {
            serial: "S1".into(),
            state: DeviceState::Connected,
            model: Some("Test Phone".into()),
            product: None,
            device: None,
            connection: ConnectionKind::Usb,
            is_emulator: false,
        };
        h.record(std::slice::from_ref(&d));
        let v = h.read();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].model.as_deref(), Some("Test Phone"));
        assert!(v[0].last_seen_ms.is_some());
        h.clear();
        assert!(h.read().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
