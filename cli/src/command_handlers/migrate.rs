use anyhow::{Context, Result};
use crate::lock::{LockFile, to_locked_entry};
use crate::config::TlkConfig;
use crate::installer::render_source;
use crate::installer::compute_installed_digest;

// Regenerate tlk.lock with latest schema and cross-platform sources
pub fn migrate_lock(cfg: &TlkConfig, path: &str) -> Result<()> {
    let Some(existing) = LockFile::load(path)? else {
        println!("No {path} found; nothing to migrate.");
        return Ok(());
    };
    println!("Existing lock schema: {:?}", existing.schema);
    // Rebuild entries from current config to ensure consistency (v3 map schema)
    use std::collections::HashMap;
    let mut map = HashMap::new();
    for t in &cfg.tools {
        let digest = compute_installed_digest(t).ok();
        let tpl = t.effective_source_template(crate::known_tools::placeholder_os(), crate::known_tools::placeholder_arch());
        let (exact, requested) = normalize_version(&t.version);
        let rendered = render_source(t).replace(&t.version, &exact);
        let (name, entry) = to_locked_entry(&t.name, &exact, requested.as_deref(), &rendered, &tpl, &t.sha256, digest);
        map.insert(name, entry);
    }
    let lf = LockFile::new(map);
    lf.save(path).with_context(|| format!("writing migrated lock {path}"))?;
    println!("Migrated {path} to schema {:?}", lf.schema);
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
