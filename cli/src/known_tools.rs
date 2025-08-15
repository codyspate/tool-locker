use crate::config::{Tool, ToolKind};
use std::collections::{HashMap, HashSet};

#[derive(Debug)]
pub enum SourceSpec {
    Template(&'static str),
    Custom(fn(&str) -> String), // takes version (already sanitized of leading 'v')
}

#[derive(Debug)]
pub struct KnownToolDef {
    pub kind: ToolKind,
    pub source: SourceSpec,
    pub binary_rel: Option<&'static str>,
}

impl KnownToolDef {
    fn build(&self, name: &str, version: &str) -> Tool {
        let clean_version = version.trim_start_matches('v').to_string();
        let source = match self.source {
            SourceSpec::Template(tpl) => tpl.to_string(),
            SourceSpec::Custom(f) => (f)(&clean_version),
        };
        Tool {
            name: name.to_string(),
            version: clean_version,
            kind: self.kind.clone(),
            source,
            sha256: None,
            binary: self.binary_rel.map(|s| s.to_string()),
            install_dir: None,
            per_os: None,
            per_os_arch: None,
        }
    }
}

pub fn known_tools_map() -> HashMap<&'static str, KnownToolDef> {
    use ToolKind::*;
    fn map_arch_for_node(a: &str) -> &str {
        match a {
            "amd64" => "x64",
            other => other,
        }
    }
    fn map_arch_for_pnpm(a: &str) -> &str {
        match a {
            "amd64" => "x64",
            other => other,
        }
    }
    fn node_source(version: &str) -> String {
        let os = detect_os();
        let arch = map_arch_for_node(detect_arch());
        // Windows uses win & .zip; others tar.gz. For now prefer unix patterns; extend later as needed.
        if os == "windows" {
            format!("https://nodejs.org/dist/v{version}/node-v{version}-win-{arch}.zip")
        } else {
            format!("https://nodejs.org/dist/v{version}/node-v{version}-{os}-{arch}.tar.gz")
        }
    }
    fn pnpm_source(version: &str) -> String {
        let os_raw = detect_os();
        let arch = map_arch_for_pnpm(detect_arch());
        // Map OS to pnpm asset naming
        let os = match os_raw {
            "darwin" => "macos",
            "windows" => "win",
            other => other,
        };
        let ext = if os == "win" { ".exe" } else { "" };
        // Use new standalone binary assets (linux uses "linuxstatic").
        let os_segment = if os == "linux" { "linuxstatic" } else { os };
        format!("https://github.com/pnpm/pnpm/releases/download/v{version}/pnpm-{os_segment}-{arch}{ext}")
    }
    fn just_source(version: &str) -> String {
        let os = detect_os();
        let arch = detect_arch();
        let triple = match (os, arch) {
            ("darwin", "amd64") => "x86_64-apple-darwin",
            ("darwin", "arm64") => "aarch64-apple-darwin",
            ("linux", "amd64") => "x86_64-unknown-linux-musl",
            ("linux", "arm64") => "aarch64-unknown-linux-musl",
            ("windows", "amd64") => "x86_64-pc-windows-msvc",
            // fallback to generic naming (may not exist)
            (o, a) => {
                return format!("https://github.com/casey/just/releases/download/{version}/just-{version}-{a}-{o}.tar.gz");
            }
        };
        let ext = if os == "windows" { "zip" } else { "tar.gz" };
        // just filenames omit the leading 'v' in the asset itself.
        format!("https://github.com/casey/just/releases/download/{version}/just-{version}-{triple}.{ext}")
    }
    fn jq_source(version: &str) -> String {
        // jq uses jq-{os}-{arch}( .exe on windows), version directory is jq-{version}
        // Example: https://github.com/jqlang/jq/releases/download/jq-1.7.1/jq-linux-amd64
        let os = detect_os();
        let arch = detect_arch();
        let os_part = match os {
            "darwin" => "macos",
            other => other,
        };
        let ext = if os == "windows" { ".exe" } else { "" };
        format!(
            "https://github.com/jqlang/jq/releases/download/jq-{version}/jq-{os_part}-{arch}{ext}"
        )
    }
    fn cosign_source(version: &str) -> String {
        // Example: cosign-linux-amd64 at tag v2.x.y
        let os = detect_os();
        let arch = detect_arch();
        let os_part = match os {
            "darwin" => "darwin",
            other => other,
        }; // keep linux/windows unchanged
        let ext = if os == "windows" { ".exe" } else { "" };
        format!("https://github.com/sigstore/cosign/releases/download/v{version}/cosign-{os_part}-{arch}{ext}")
    }
    fn age_source(version: &str) -> String {
        // Assets: age-v1.1.1-darwin-amd64.tar.gz, age-v1.1.1-windows-amd64.zip
        let os = detect_os();
        let arch = detect_arch();
        let ext = if os == "windows" { "zip" } else { "tar.gz" };
        format!("https://github.com/FiloSottile/age/releases/download/v{version}/age-v{version}-{os}-{arch}.{ext}")
    }
    fn moon_source(version: &str) -> String {
        // moon publishes raw binaries (no version segment in filename) per target triple, e.g.:
        //   moon-aarch64-apple-darwin
        //   moon-x86_64-unknown-linux-gnu / -musl
        //   moon-x86_64-pc-windows-msvc.exe
        // Directory/tag still includes 'v{version}'.
        #[cfg(target_env = "musl")]
        const MOON_LIBC: &str = "musl";
        #[cfg(not(target_env = "musl"))]
        const MOON_LIBC: &str = "gnu";
        let os = detect_os();
        let arch = match detect_arch() {
            "amd64" => "x86_64",
            other => other,
        };
        let triple = match os {
            "darwin" => format!("{arch}-apple-darwin"),
            "linux" => format!("{arch}-unknown-linux-{libc}", libc = MOON_LIBC),
            "windows" => format!("{arch}-pc-windows-msvc"),
            other => format!("{arch}-{other}"),
        };
        let ext = if os == "windows" { ".exe" } else { "" };
        format!("https://github.com/moonrepo/moon/releases/download/v{version}/moon-{triple}{ext}")
    }
    HashMap::from([
        ("terraform", KnownToolDef { kind: Archive, source: SourceSpec::Template("https://releases.hashicorp.com/terraform/{version}/terraform_{version}_{os}_{arch}.zip"), binary_rel: Some("terraform") }),
        ("kubectl", KnownToolDef { kind: Direct, source: SourceSpec::Template("https://dl.k8s.io/release/v{version}/bin/{os}/{arch}/kubectl"), binary_rel: None }),
        ("helm", KnownToolDef { kind: Archive, source: SourceSpec::Template("https://get.helm.sh/helm-v{version}-{os}-{arch}.tar.gz"), binary_rel: None }),
        ("gh", KnownToolDef { kind: Archive, source: SourceSpec::Template("https://github.com/cli/cli/releases/download/v{version}/gh_{version}_{os}_{arch}.tar.gz"), binary_rel: None }),
        ("buf", KnownToolDef { kind: Direct, source: SourceSpec::Template("https://github.com/bufbuild/buf/releases/download/v{version}/buf-{os}-{arch}"), binary_rel: None }),
        // Newly added tools
        ("node", KnownToolDef { kind: Archive, source: SourceSpec::Custom(node_source), binary_rel: Some("bin/node") }),
        ("pnpm", KnownToolDef { kind: Direct, source: SourceSpec::Custom(pnpm_source), binary_rel: None }),
        ("yarn", KnownToolDef { kind: Archive, source: SourceSpec::Template("https://github.com/yarnpkg/yarn/releases/download/v{version}/yarn-v{version}.tar.gz"), binary_rel: Some("bin/yarn") }),
        ("just", KnownToolDef { kind: Archive, source: SourceSpec::Custom(just_source), binary_rel: Some("just") }),
        ("jq", KnownToolDef { kind: Direct, source: SourceSpec::Custom(jq_source), binary_rel: None }),
        ("cosign", KnownToolDef { kind: Direct, source: SourceSpec::Custom(cosign_source), binary_rel: None }),
        ("age", KnownToolDef { kind: Archive, source: SourceSpec::Custom(age_source), binary_rel: Some("age") }),
    ("moon", KnownToolDef { kind: Direct, source: SourceSpec::Custom(moon_source), binary_rel: Some("moon") }),
    ])
}

pub fn extract_shorthand(root: &toml::Value, existing: &HashSet<String>) -> Vec<Tool> {
    let mut out = Vec::new();
    let map = known_tools_map();
    if let toml::Value::Table(tbl) = root {
        for (k, v) in tbl {
            if existing.contains(k) {
                continue;
            }
            if let Some(def) = map.get(k.as_str()) {
                if let Some(ver) = v.as_str() {
                    out.push(def.build(k, ver));
                }
            }
        }
    }
    out
}

pub fn build_known_tool(name: &str, version: &str) -> anyhow::Result<Tool> {
    let map = known_tools_map();
    if let Some(def) = map.get(name) {
        Ok(def.build(name, version))
    } else {
        Err(anyhow::anyhow!("unknown known tool '{name}'"))
    }
}

pub fn detect_os() -> &'static str {
    match std::env::consts::OS {
        "macos" => "darwin",
        other => other,
    }
}
pub fn detect_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        other => other,
    }
}
pub fn placeholder_os() -> &'static str {
    detect_os()
}
pub fn placeholder_arch() -> &'static str {
    detect_arch()
}
