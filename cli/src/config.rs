use crate::known_tools::extract_shorthand;
use crate::unknown_tools::{augment_binary_fields, parse_unknown};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;

#[derive(Debug, Clone)]
pub struct TlkConfig {
    pub tools: Vec<Tool>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Tool {
    pub name: String,
    pub version: String,
    #[serde(default = "default_kind")]
    pub kind: ToolKind,
    /// URL template. Supports {version}, {os}, {arch}
    pub source: String,
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub binary: Option<String>,
    #[serde(default)]
    pub install_dir: Option<String>,
    /// Per-OS templates (keys: linux, mac, windows). Supports {version} and {arch}.
    #[serde(default)]
    pub per_os: Option<PerOsSources>,
    /// Per OS+Arch templates. Allows fine grained override.
    #[serde(default)]
    pub per_os_arch: Option<PerOsArchSources>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct PerOsSources {
    #[serde(default)]
    pub linux: Option<String>,
    #[serde(default, rename = "mac")]
    pub mac: Option<String>,
    #[serde(default)]
    pub windows: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct PerOsArchSources {
    #[serde(default)]
    pub linux: Option<ArchSources>,
    #[serde(default, rename = "mac")]
    pub mac: Option<ArchSources>,
    #[serde(default)]
    pub windows: Option<ArchSources>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ArchSources {
    #[serde(default)]
    pub amd64: Option<String>,
    #[serde(default)]
    pub arm64: Option<String>,
    // Accept synonyms users might prefer
    #[serde(default, rename = "x86_64")]
    pub x86_64: Option<String>,
    #[serde(default, rename = "aarch64")]
    pub aarch64: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum ToolKind {
    Archive,
    Direct,
}

fn default_kind() -> ToolKind {
    ToolKind::Archive
}

impl TlkConfig {
    pub fn load(path: &str) -> Option<Self> {
        let data = fs::read_to_string(path);
        let data = match data {
            Ok(d) => d,
            Err(_) => return None,
        };
        let value: toml::Value = match data.parse::<toml::Value>() {
            Ok(v) => v,
            Err(_) => {
                if data.trim_start().starts_with('{') {
                    if let Ok(repaired) = repair_inline_root(&data) {
                        std::fs::write(path, &repaired).ok();

                        let repaired_file = repaired
                            .parse::<toml::Value>()
                            .with_context(|| "parsing tlk.toml after repair");
                        if let Ok(v) = repaired_file {
                            v
                        } else {
                            return None;
                        }
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            }
        };
        // Unknown (user-provided) tools from [[tools]] entries
        let tools = parse_unknown(&value);
        let Ok(mut tools) = tools else { return None };
        let explicit_names: HashSet<String> = tools.iter().map(|t| t.name.clone()).collect();
        // Known shorthand single-line entries (terraform = "1.2.3")
        let mut shorthand = extract_shorthand(&value, &explicit_names);
        tools.append(&mut shorthand);
        augment_binary_fields(&mut tools);

        Some(TlkConfig { tools })
    }
}

impl Tool {
    /// Produce a concrete source template (still with {version},{os},{arch} placeholders potentially) after applying per_os/per_os_arch overrides.
    pub fn effective_source_template(&self, os: &str, arch: &str) -> String {
        // Priority: per_os_arch specific > per_os > source
        if let Some(per) = &self.per_os_arch {
            let os_entry = match os {
                "linux" => per.linux.as_ref(),
                "darwin" => per.mac.as_ref(),
                "macos" => per.mac.as_ref(),
                "windows" => per.windows.as_ref(),
                _ => None,
            };
            if let Some(arch_sources) = os_entry {
                let candidate = match arch {
                    "amd64" | "x86_64" => {
                        arch_sources.amd64.as_ref().or(arch_sources.x86_64.as_ref())
                    }
                    "arm64" | "aarch64" => arch_sources
                        .arm64
                        .as_ref()
                        .or(arch_sources.aarch64.as_ref()),
                    other => {
                        // Try dynamic field names not directly represented
                        if other == "x86_64" {
                            arch_sources.x86_64.as_ref()
                        } else {
                            None
                        }
                    }
                };
                if let Some(tpl) = candidate {
                    return tpl.clone();
                }
            }
        }
        if let Some(per) = &self.per_os {
            let candidate = match os {
                "linux" => per.linux.as_ref(),
                "darwin" => per.mac.as_ref(),
                "macos" => per.mac.as_ref(),
                "windows" => per.windows.as_ref(),
                _ => None,
            };
            if let Some(tpl) = candidate {
                return tpl.clone();
            }
        }
        self.source.clone()
    }
}

// placeholder helpers available in known_tools

// Attempt to repair a root-level inline table dumped form like:
// { key = "v", tools = [{ k = "v" }] }
// into standard multi-line TOML accepted by our loader.
fn repair_inline_root(src: &str) -> Result<String> {
    let s = src.trim();
    if !(s.starts_with('{') && s.ends_with('}')) {
        return Err(anyhow::anyhow!("not inline root"));
    }
    let inner = &s[1..s.len() - 1];
    let parts = split_top_level(inner);
    let mut shorthand = Vec::new();
    let mut tools_segment: Option<String> = None;
    for p in parts {
        let p = p.trim();
        if p.starts_with("tools") {
            tools_segment = Some(p.to_string());
        } else if !p.is_empty() {
            shorthand.push(p.to_string());
        }
    }
    let mut out = String::new();
    for kv in shorthand {
        out.push_str(kv.trim());
        out.push('\n');
    }
    if let Some(seg) = tools_segment {
        if let Some(arr_start) = seg.find('[') {
            // tools = [ ... ]
            let arr = seg[arr_start..].trim();
            if arr.starts_with('[') && arr.ends_with(']') {
                let arr_inner = &arr[1..arr.len() - 1];
                let tables = split_inline_tables(arr_inner);
                for t in tables {
                    out.push_str("\n[[tools]]\n");
                    let kvs = split_top_level(&t);
                    for kv in kvs {
                        let kv = kv.trim();
                        if !kv.is_empty() {
                            out.push_str(kv);
                            out.push('\n');
                        }
                    }
                }
            }
        }
    }
    Ok(out)
}

fn split_top_level(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut depth_brace = 0usize; // {}
    let mut depth_bracket = 0usize; // []
    let mut in_str = false;
    let mut prev = '\0';
    for c in s.chars() {
        if in_str {
            cur.push(c);
            if c == '"' && prev != '\\' {
                in_str = false;
            }
        } else {
            match c {
                '"' => {
                    in_str = true;
                    cur.push(c);
                }
                '{' => {
                    depth_brace += 1;
                    cur.push(c);
                }
                '}' => {
                    if depth_brace > 0 {
                        depth_brace -= 1;
                    }
                    cur.push(c);
                }
                '[' => {
                    depth_bracket += 1;
                    cur.push(c);
                }
                ']' => {
                    if depth_bracket > 0 {
                        depth_bracket -= 1;
                    }
                    cur.push(c);
                }
                ',' if depth_brace == 0 && depth_bracket == 0 => {
                    parts.push(cur.trim().to_string());
                    cur.clear();
                }
                _ => cur.push(c),
            }
        }
        prev = c;
    }
    if !cur.trim().is_empty() {
        parts.push(cur.trim().to_string());
    }
    parts
}

fn split_inline_tables(s: &str) -> Vec<String> {
    // expects sequence like { a = "b" } , { ... }
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0usize;
    let mut in_str = false;
    let mut prev = '\0';
    for c in s.chars() {
        if in_str {
            cur.push(c);
            if c == '"' && prev != '\\' {
                in_str = false;
            }
            prev = c;
            continue;
        }
        match c {
            '"' => {
                in_str = true;
                cur.push(c);
            }
            '{' => {
                depth += 1;
                cur.push(c);
            }
            '}' => {
                if depth > 0 {
                    depth -= 1;
                }
                cur.push(c);
                if depth == 0 {
                    out.push(
                        cur.trim()
                            .trim_start_matches('{')
                            .trim_end_matches('}')
                            .trim()
                            .to_string(),
                    );
                    cur.clear();
                }
            }
            _ => {
                if !(depth == 0 && c == ',') {
                    cur.push(c);
                }
            }
        }
        prev = c;
    }
    out
}
