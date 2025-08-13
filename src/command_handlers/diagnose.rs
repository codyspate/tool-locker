use anyhow::{Result, anyhow};
use crate::lock::LockFile;

pub fn list_missing(lock_path: &str) -> Result<()> {
    let Some(lock) = LockFile::load(lock_path)? else { return Err(anyhow!("no lock file at {lock_path}")); };
    let mut missing_total = 0usize;
    for (name, lt) in &lock.tools {
        if let Some(srcs) = &lt.sources {
            let expected_oss = ["linux", "darwin", "windows"];
            let expected_archs = ["amd64", "arm64"];
            let mut missing = Vec::new();
            for o in expected_oss.iter() { for a in expected_archs.iter() { let key = format!("{o}-{a}"); if !srcs.contains_key(&key) { missing.push(key); } } }
            if !missing.is_empty() { missing_total += missing.len(); println!("{name} {} missing: {}", lt.version, missing.join(", ")); }
        } else {
            println!("{name} {} has no sources map (older schema or custom)", lt.version);
            missing_total += 1;
        }
    }
    if missing_total == 0 { println!("All tools have complete platform coverage"); }
    Ok(())
}
