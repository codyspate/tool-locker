use crate::config::TlkConfig;
use anyhow::Result;

pub fn uninstall_tool(config_path: &str, name: &str) -> Result<()> {
    use std::fs;
    let p = crate::platform::platform();
    if let Some(cfg) = TlkConfig::load(config_path) {
        if let Some(tool) = cfg.tools.iter().find(|t| t.name == name) {
            let dir = tool
                .install_dir
                .clone()
                .or_else(|| p.global_bin_dir().map(|g| g.to_string_lossy().to_string()))
                .unwrap_or_else(|| ".tlk/bin".into());
            let filename = p.final_binary_name(name);
            let path = std::path::Path::new(&dir).join(&filename);
            if path.exists() {
                fs::remove_file(&path)
                    .map_err(|e| anyhow::anyhow!("removing binary {:?}: {e}", path))?;
            }
        }
    }
    let local_dir = std::path::Path::new(".tlk/bin");
    for candidate in [p.final_binary_name(name), name.to_string()].into_iter() {
        let path = local_dir.join(&candidate);
        if path.exists() {
            let _ = fs::remove_file(&path);
        }
    }
    remove_from_config(config_path, name)?;
    remove_from_lock("tlk.lock", name)?;
    Ok(())
}

fn remove_from_config(path: &str, name: &str) -> Result<()> {
    use std::fs;
    let data = fs::read_to_string(path)?;
    let mut root: toml::Value = data.parse()?;
    if let toml::Value::Table(tbl) = &mut root {
        if let Some(v) = tbl.get(name) {
            if v.is_str() {
                tbl.remove(name);
            }
        }
        if let Some(arr) = tbl.get_mut("tools") {
            if let toml::Value::Array(items) = arr {
                items.retain(|it| !(it.get("name").and_then(|v| v.as_str()) == Some(name)));
            }
        }
    }
    let serialized = toml::to_string_pretty(&root)?;
    fs::write(path, serialized)?;
    Ok(())
}

fn remove_from_lock(path: &str, name: &str) -> Result<()> {
    if let Some(mut lf) = crate::lock::LockFile::load(path)? {
        lf.tools.remove(name);
        lf.save(path)?;
    }
    Ok(())
}
