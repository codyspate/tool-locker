use crate::config::Tool;
use anyhow::{Context, Result};
use crate::known_tools::{detect_arch, detect_os};

pub fn parse_explicit(root: &toml::Value) -> Result<Vec<Tool>> {
    let mut tools = Vec::new();
    if let Some(arr) = root.get("tools").and_then(|t| t.as_array()) {
        for item in arr {
            if let toml::Value::Table(tbl) = item {
                let tool: Tool = tbl.clone().try_into().with_context(|| "parsing tool entry")?;
                tools.push(tool);
            }
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
