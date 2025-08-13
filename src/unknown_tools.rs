use crate::config::Tool;
use anyhow::{Context, Result};
use crate::known_tools::{detect_arch, detect_os};

// Unknown tools are user-declared entries. Supported syntaxes:
// 1) NEW preferred: [tools.foo] version = "1.2.3" source = "..."
// 2) Legacy: [[tools]] name = "foo" version = "1.2.3" source = "..."
pub fn parse_unknown(root: &toml::Value) -> Result<Vec<Tool>> {
    let mut tools = Vec::new();
    if let Some(container) = root.get("tools") {
        if let Some(arr) = container.as_array() { // legacy array-of-tables
            if !arr.is_empty() {
                eprintln!("Warning: legacy [[tools]] syntax detected; consider running 'tlk migrate-config' to upgrade to [tools.<name>] style.");
            }
            for item in arr {
                if let toml::Value::Table(tbl) = item {
                    let tool: Tool = tbl.clone().try_into().with_context(|| "parsing legacy [[tools]] entry")?;
                    tools.push(tool);
                }
            }
        } else if let Some(tbl) = container.as_table() { // new table-of-tables style
            for (name, val) in tbl.iter() {
                if let toml::Value::Table(inner) = val {
                    let mut cloned = inner.clone();
                    // Allow explicit name override but default to key
                    if !cloned.contains_key("name") {
                        cloned.insert("name".to_string(), toml::Value::String(name.to_string()));
                    }
                    let tool: Tool = cloned.try_into().with_context(|| format!("parsing tools.{name}"))?;
                    tools.push(tool);
                }
            }
        }
    }
    // Validation: ensure version & source present & non-empty
    for t in &tools {
        if t.version.trim().is_empty() {
            return Err(anyhow::anyhow!(format!("tool '{}' missing version", t.name)));
        }
        if t.source.trim().is_empty() {
            return Err(anyhow::anyhow!(format!("tool '{}' missing source", t.name)));
        }
    }
    Ok(tools)
}

pub fn augment_binary_fields(tools: &mut [Tool]) {
    for t in tools.iter_mut() {
        if t.binary.is_none() {
            if t.name == "helm" { t.binary = Some(format!("{os}-{arch}/helm", os=detect_os(), arch=detect_arch())); }
            else if t.name == "gh" { t.binary = Some(format!("gh_{v}_{os}_{arch}/bin/gh", v=t.version, os=detect_os(), arch=detect_arch())); }
        }
    }
}
