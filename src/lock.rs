use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

// ---------------- New schema (v3) ----------------

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LockFile {
    pub generated: DateTime<Utc>,
    #[serde(default)]
    pub tlk_version: Option<String>,
    #[serde(default)]
    pub schema: Option<u32>,
    /// Map keyed by tool name
    pub tools: HashMap<String, LockedEntry>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LockedEntry {
    pub version: String, // exact resolved version
    #[serde(default)]
    pub requested_version: Option<String>, // original spec if different (range)
    pub source: String,
    #[serde(default)]
    pub source_template: Option<String>,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub sources: Option<HashMap<String, String>>, // platform matrix
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub digest: Option<String>,
}

impl LockFile {
    pub fn new(map: HashMap<String, LockedEntry>) -> Self {
        Self {
            generated: Utc::now(),
            tlk_version: Some(env!("CARGO_PKG_VERSION").to_string()),
            schema: Some(3),
            tools: map,
        }
    }
    pub fn load(path: &str) -> Result<Option<Self>> {
        if !std::path::Path::new(path).exists() {
            return Ok(None);
        }
        let data = fs::read_to_string(path).with_context(|| format!("reading lock file {path}"))?;
        if let Ok(v3) = toml::from_str::<LockFile>(&data) {
            return Ok(Some(v3));
        }
        // Attempt legacy upgrade path
        if let Ok(old) = toml::from_str::<OldLockFile>(&data) {
            let mut map = HashMap::new();
            for t in old.tools {
                map.insert(
                    t.name.clone(),
                    LockedEntry {
                        version: t.version,
                        requested_version: None,
                        source: t.source,
                        source_template: t.source_template,
                        platform: t.platform,
                        sources: t.sources,
                        sha256: t.sha256,
                        digest: t.digest,
                    },
                );
            }
            return Ok(Some(LockFile::new(map)));
        }
        Err(anyhow::anyhow!(
            "unable to parse lock file (unsupported schema)"
        ))
    }
    pub fn save(&self, path: &str) -> Result<()> {
        let mut clone = self.clone();
        clone.tlk_version = Some(env!("CARGO_PKG_VERSION").to_string());
        if clone.schema.is_none() {
            clone.schema = Some(3);
        }
        let toml_str = toml::to_string_pretty(&clone).with_context(|| "serializing lock file")?;
        fs::write(path, toml_str).with_context(|| format!("writing lock file {path}"))?;
        Ok(())
    }
}

// --------------- Legacy schema (v1/v2) ---------------

#[derive(Debug, Serialize, Deserialize, Clone)]
struct OldLockFile {
    pub generated: DateTime<Utc>,
    #[serde(default)]
    pub tlk_version: Option<String>,
    #[serde(default)]
    pub schema: Option<u32>,
    pub tools: Vec<OldLockedTool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct OldLockedTool {
    pub name: String,
    pub version: String,
    pub source: String,
    #[serde(default)]
    pub source_template: Option<String>,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub sources: Option<HashMap<String, String>>,
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub digest: Option<String>,
}

// --------------- Helpers ---------------

pub fn to_locked_entry(
    name: &str,
    exact_version: &str,
    requested_version: Option<&str>,
    rendered_source: &str,
    template: &str,
    sha256: &Option<String>,
    digest: Option<String>,
) -> (String, LockedEntry) {
    let platform_key = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
    let mut sources: HashMap<String, String> = HashMap::new();
    if template.contains("{os}") && template.contains("{arch}") {
        let oss = ["linux", "darwin", "windows"];
        let archs = ["amd64", "arm64"];
        for o in oss.iter() {
            for a in archs.iter() {
                let url = template
                    .replace("{version}", exact_version)
                    .replace("{os}", o)
                    .replace("{arch}", a);
                sources.insert(format!("{o}-{a}"), url);
            }
        }
    }
    (
        name.to_string(),
        LockedEntry {
            version: exact_version.to_string(),
            requested_version: requested_version.map(|s| s.to_string()),
            source: rendered_source.to_string(),
            source_template: Some(template.to_string()),
            platform: Some(platform_key),
            sources: if sources.is_empty() {
                None
            } else {
                Some(sources)
            },
            sha256: sha256.clone(),
            digest,
        },
    )
}
// old schema kept only for upgrade parsing
