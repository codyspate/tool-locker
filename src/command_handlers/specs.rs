// Spec & version range canonicalization utilities
pub fn canonicalize_spec(
    user_spec: Option<&str>,
    resolved: &str,
    exact_flag: bool,
    latest_flag: bool,
) -> String {
    if let Some(spec) = user_spec {
        let trimmed = spec.trim();
        if trimmed.is_empty() {
            return resolved.to_string();
        }
        let has_range_tokens = ["^", "~", "*", "x", "X", "||", "-", ">", "<", "="]
            .iter()
            .any(|t| trimmed.contains(t));
        if has_range_tokens { return normalize_partials(trimmed); }
        if semver::Version::parse(trimmed).is_ok() {
            return if exact_flag { trimmed.to_string() } else { caret_for_version(trimmed) };
        }
        let dot_count = trimmed.chars().filter(|c| *c == '.').count();
        if trimmed.chars().all(|c| c.is_ascii_digit() || c == '.') {
            if dot_count == 0 { return format!("{}.x", trimmed); }
            else if dot_count == 1 { return format!("{}.x", trimmed); }
        }
        return trimmed.to_string();
    }
    if exact_flag { return resolved.to_string(); }
    if latest_flag { return caret_for_version(resolved); }
    caret_for_version(resolved)
}

fn normalize_partials(spec: &str) -> String { spec.to_string() }

fn caret_for_version(ver: &str) -> String {
    if semver::Version::parse(ver).is_err() { return ver.to_string(); }
    format!("^{}", ver)
}

pub fn canonicalize_spec_logging(
    path: &str,
    name: &str,
    user_spec: Option<&str>,
    resolved_version: &str,
    exact_flag: bool,
    latest_flag: bool,
) -> anyhow::Result<()> {
    use std::fs;
    let raw = fs::read_to_string(path).unwrap_or_default();
    let mut root: toml::Value = raw.parse().unwrap_or_else(|_| toml::Value::Table(toml::Table::new()));
    let to_store = canonicalize_spec(user_spec, resolved_version, exact_flag, latest_flag);
    if let toml::Value::Table(tbl) = &mut root {
        if let Some(existing) = tbl.get_mut(name) { if existing.is_str() { *existing = toml::Value::String(to_store.clone()); } } else {
            let mut updated=false;
            if let Some(arr)=tbl.get_mut("tools") { if let toml::Value::Array(items)=arr { for it in items.iter_mut() { if let toml::Value::Table(t)=it { if t.get("name").and_then(|v| v.as_str())==Some(name) { t.insert("version".into(), toml::Value::String(to_store.clone())); updated=true; break; } } } } }
            if !updated { tbl.insert(name.to_string(), toml::Value::String(to_store.clone())); }
        }
    }
    let serialized = toml::to_string_pretty(&root)?;
    fs::write(path, serialized)?;
    Ok(())
}
