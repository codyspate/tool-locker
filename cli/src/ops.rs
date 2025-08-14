use crate::config::{TlkConfig, Tool};
use crate::known_tools::{placeholder_arch, placeholder_os};
use crate::{installer, lock};
use anyhow::Result;

pub fn write_single_lock(tool: &Tool) -> Result<()> {
    use std::collections::HashMap;
    let path = "tlk.lock";
    let mut map = if let Some(existing) = lock::LockFile::load(path)? { existing.tools } else { HashMap::new() };
    let digest = installer::compute_installed_digest(tool).ok();
    let tpl = tool.effective_source_template(placeholder_os(), placeholder_arch());
    let (exact, requested) = normalize_version(&tool.version);
    let rendered = installer::render_source(tool).replace(&tool.version, &exact);
    let (name, entry) = lock::to_locked_entry(&tool.name, &exact, requested.as_deref(), &rendered, &tpl, &tool.sha256, digest);
    map.insert(name, entry);
    let lf = lock::LockFile::new(map);
    lf.save(path)?;
    println!("Updated lock with {} {}", tool.name, exact);
    Ok(())
}

pub fn install_locked(lock_path: &str, cfg: &TlkConfig) -> Result<()> {
    let Some(lock) = lock::LockFile::load(lock_path)? else { anyhow::bail!("no lock file found at {lock_path}"); };
    for (name, lt) in lock.tools.iter() {
        let mut tool = if let Some(t) = cfg.tools.iter().find(|t| &t.name == name) { let mut cloned = t.clone(); cloned.version = lt.version.clone(); cloned } else { crate::known_tools::build_known_tool(name, &lt.version)? };
        let platform_key = format!("{}-{}", placeholder_os(), placeholder_arch());
        if let Some(srcs) = &lt.sources { if let Some(url) = srcs.get(&platform_key) { tool.source = url.clone(); } else { tool.source = lt.source.clone(); } } else { tool.source = lt.source.clone(); }
        installer::install_single(&tool)?;
    }
    Ok(())
}

fn normalize_version(spec: &str) -> (String, Option<String>) {
    if semver::Version::parse(spec.trim()).is_ok() { return (spec.trim().to_string(), None); }
    let mut s = spec.trim().to_string();
    if let Some((first, _)) = s.split_once(' ') { s = first.to_string(); }
    if let Some(pos) = s.find("||") { s = s[..pos].trim().to_string(); }
    let mut trimmed = s.trim_start_matches(['^','~','>','=','<']).to_string();
    trimmed = trimmed.trim_start_matches('=').to_string();
    trimmed = trimmed.trim().to_string();
    if semver::Version::parse(&trimmed).is_ok() { (trimmed, Some(spec.to_string())) } else { (spec.to_string(), None) }
}
