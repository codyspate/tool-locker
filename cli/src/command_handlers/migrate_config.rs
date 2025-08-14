use anyhow::{Result, Context};
use std::fs;

// Migrate legacy [[tools]] array-of-table style to new [tools.<name>] tables in-place.
// Keeps a backup at tlk.toml.bak
pub fn migrate_config(path: &str) -> Result<()> {
    let data = fs::read_to_string(path).with_context(|| format!("reading {path}"))?;
    let value: toml::Value = data.parse().with_context(|| "parsing toml")?;
    let Some(arr) = value.get("tools").and_then(|v| v.as_array()) else {
        println!("No legacy [[tools]] entries found (nothing to do)");
        return Ok(());
    };
    if arr.is_empty() { println!("Legacy tools array empty (nothing to do)"); return Ok(()); }
    // Build new table structure.
    let mut root = value.clone();
    // Remove old array
    if let Some(table_root) = root.as_table_mut() { table_root.remove("tools"); }
    let mut tools_table = toml::value::Table::new();
    for item in arr {
        if let Some(t) = item.as_table() {
            let name = if let Some(toml::Value::String(n)) = t.get("name") { n.clone() } else {
                println!("Skipping legacy tool missing name field");
                continue;
            };
            let mut cloned = t.clone();
            cloned.remove("name"); // name now in key
            tools_table.insert(name, toml::Value::Table(cloned));
        }
    }
    if let Some(table_root) = root.as_table_mut() {
        table_root.insert("tools".to_string(), toml::Value::Table(tools_table));
    }
    let serialized = toml::to_string_pretty(&root)?;
    let backup = format!("{path}.bak");
    fs::write(&backup, data)?;
    fs::write(path, serialized)?;
    println!("Migrated config to [tools.<name>] syntax (backup at {backup})");
    Ok(())
}
