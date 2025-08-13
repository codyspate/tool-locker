use crate::config::{TlkConfig, Tool, ToolKind};
use crate::known_tools::{placeholder_arch, placeholder_os};
use crate::lock::{to_locked_entry, LockFile};
use crate::platform::platform;
use anyhow::{anyhow, Context, Result};
use flate2::read::GzDecoder;
use fs_err as fs;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::blocking::Client;
use semver::Version;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Command;
use tar::Archive;
use zip::ZipArchive;

pub fn plan(cfg: &TlkConfig) -> Result<()> {
    for t in &cfg.tools {
        println!("{} {} -> {}", t.name, t.version, t.source);
    }
    Ok(())
}

pub fn list(cfg: &TlkConfig) -> Result<()> {
    for t in &cfg.tools {
        let installed = find_installed_version(t).unwrap_or_else(|_| "<not installed>".to_string());
        println!("{} desired={} installed={}", t.name, t.version, installed);
    }
    Ok(())
}

pub fn install_all(cfg: &TlkConfig) -> Result<()> {
    // Use parallel strategy for speed; fall back to sequential if only one
    if cfg.tools.len() <= 1 {
        return install_all_sequential(cfg);
    }
    let results = install_tools_parallel(&cfg.tools);
    let out = summarize_parallel(results);
    if out.is_ok() {
        refresh_path();
    }
    out
}

fn install_all_sequential(cfg: &TlkConfig) -> Result<()> {
    let m = MultiProgress::new();
    let client = Client::new();
    for t in &cfg.tools {
        let pb = m.add(ProgressBar::new_spinner());
        pb.set_style(ProgressStyle::with_template("{spinner} {msg}").unwrap());
        pb.set_message(format!("Installing {} {}", t.name, t.version));
        if let Err(e) = install_tool(&client, t, Some(&pb)) {
            pb.finish_with_message(format!("{} FAILED: {e}", t.name));
        } else {
            pb.finish_with_message(format!("{} OK", t.name));
        }
    }
    refresh_path();
    Ok(())
}

pub fn install_tools_parallel(tools: &[Tool]) -> Vec<(String, Result<()>)> {
    use std::thread;
    use std::time::Duration;
    let m = MultiProgress::new();
    let style = ProgressStyle::with_template("{spinner} {msg}").unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    for tool in tools {
        let pb = m.add(ProgressBar::new_spinner());
        pb.set_style(style.clone());
        pb.set_message(format!("Starting {} {}", tool.name, tool.version));
        pb.enable_steady_tick(Duration::from_millis(120));
        let tool_clone = tool.clone();
        let txc = tx.clone();
        thread::spawn(move || {
            let client = Client::new();
            pb.set_message(format!(
                "Downloading {} {}",
                tool_clone.name, tool_clone.version
            ));
            let res = install_tool(&client, &tool_clone, Some(&pb));
            match &res {
                Ok(_) => pb.finish_with_message(format!(
                    "Installed {} {}",
                    tool_clone.name, tool_clone.version
                )),
                Err(e) => pb.finish_with_message(format!(
                    "FAILED {} {}: {e}",
                    tool_clone.name, tool_clone.version
                )),
            }
            let _ = txc.send((tool_clone.name.clone(), res));
        });
    }
    drop(tx); // close sending side when workers exit
              // Collect all results; channel closes when all worker threads done
    let mut results = Vec::with_capacity(tools.len());
    for msg in rx.iter() {
        results.push(msg);
    }
    // Give bars a moment to flush final lines
    std::thread::sleep(Duration::from_millis(20));
    results
}

fn summarize_parallel(results: Vec<(String, Result<()>)>) -> Result<()> {
    let mut failures = Vec::new();
    for (name, res) in results {
        if let Err(e) = res {
            failures.push((name, e));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(anyhow!(format!("{} tool(s) failed", failures.len())))
    }
}

pub fn install_single(tool: &Tool) -> Result<()> {
    let client = Client::new();
    let res = install_tool(&client, tool, None);
    if res.is_ok() {
        refresh_path();
    }
    res
}
// Attempt to mimic the hook's PATH adjustment once after installs so newly installed binaries are immediately usable when user did not yet eval the hook.
pub fn refresh_path() {
    // Skip if disabled
    if std::env::var("TLK_NO_AUTO_PATH").is_ok() {
        return;
    }
    if let Ok(mut dir) = std::env::current_dir() {
        loop {
            if dir.join("tlk.toml").exists() {
                let bindir = dir.join(".tlk/bin");
                if bindir.is_dir() {
                    let bindir_s = bindir.to_string_lossy().to_string();
                    let path = std::env::var("PATH").unwrap_or_default();
                    let mut parts: Vec<String> = path.split(':').map(|s| s.to_string()).collect();
                    parts.retain(|p| p != &bindir_s);
                    parts.insert(0, bindir_s.clone());
                    let new_path = parts.join(":");
                    // We can set for this process only; print hint for user shell.
                    std::env::set_var("PATH", &new_path);
                    println!("[tlk] PATH updated (session only). To persist in shell, eval 'eval \"$(tlk hook)\"'");
                }
                break;
            }
            if !dir.pop() {
                break;
            }
        }
    }
}

// Expand placeholders into a concrete URL for locking and downloading
pub fn render_source(tool: &Tool) -> String {
    let os = placeholder_os();
    let arch = placeholder_arch();
    // May still contain placeholders for {os}/{arch}
    let template = tool.effective_source_template(os, arch);
    let base = template
        .replace("{version}", &tool.version)
        .replace("{os}", os)
        .replace("{arch}", arch);
    platform().adjust_direct_url(&base)
}

pub fn write_lockfile(cfg: &TlkConfig, path: &str) -> Result<()> {
    use std::collections::HashMap;
    let mut map: HashMap<String, crate::lock::LockedEntry> = HashMap::new();
    for t in &cfg.tools {
        let digest = compute_installed_digest(t).ok();
        let tpl = t.effective_source_template(placeholder_os(), placeholder_arch());
        // Determine exact version (strip range operators if any)
        let (exact, requested) = normalize_version(&t.version);
        let rendered = render_source(t).replace(&t.version, &exact); // ensure rendered uses exact if different
        let (name, entry) = to_locked_entry(
            &t.name,
            &exact,
            requested.as_deref(),
            &rendered,
            &tpl,
            &t.sha256,
            digest,
        );
        map.insert(name, entry);
    }
    let lf = LockFile::new(map);
    lf.save(path)?;
    println!("Wrote lockfile {path}");
    Ok(())
}

pub fn verify_lockfile(cfg: &TlkConfig, path: &str) -> Result<()> {
    let Some(lock) = LockFile::load(path)? else {
        println!("No {path} present; run 'tlk install' to create it.");
        return Ok(());
    };
    let mut errors = Vec::new();
    for t in &cfg.tools {
        match lock.tools.get(&t.name) {
            None => errors.push(format!("tool '{}' missing from lock", t.name)),
            Some(lt) => {
                // Determine if config version is a range; locked version must satisfy it
                if is_range(&t.version) {
                    if !range_satisfies(&t.version, &lt.version) {
                        errors.push(format!(
                            "tool '{}' locked version {} does not satisfy range {}",
                            t.name, lt.version, t.version
                        ));
                    }
                } else if lt.version != t.version {
                    errors.push(format!(
                        "tool '{}' version mismatch lock={} config={}",
                        t.name, lt.version, t.version
                    ));
                }
                // Template/source check
                if let Some(tpl) = &lt.source_template {
                    let expected = tpl
                        .replace("{version}", &lt.version)
                        .replace("{os}", placeholder_os())
                        .replace("{arch}", placeholder_arch());
                    let rendered = render_source(t).replace(&t.version, &lt.version);
                    if expected != rendered {
                        errors.push(format!("tool '{}' source mismatch", t.name));
                    }
                }
                if let (Some(cfg_sum), Some(lock_sum)) = (&t.sha256, &lt.sha256) {
                    if cfg_sum != lock_sum {
                        errors.push(format!("tool '{}' checksum mismatch", t.name));
                    }
                }
                if let Some(expected_digest) = &lt.digest {
                    if let Ok(actual) = compute_installed_digest(t) {
                        if actual != *expected_digest {
                            errors.push(format!("tool '{}' digest mismatch", t.name));
                        }
                    }
                }
            }
        }
    }
    for name in lock.tools.keys() {
        if !cfg.tools.iter().any(|t| &t.name == name) {
            println!("Warning: lock contains extra tool '{name}' not in config");
        }
    }
    if errors.is_empty() {
        println!("Lock verification passed");
        Ok(())
    } else {
        Err(anyhow::anyhow!(format!(
            "lock verification failed:\n - {}",
            errors.join("\n - ")
        )))
    }
}

// --- Range helpers (kept local to avoid circular dep on main) ---
fn is_range(spec: &str) -> bool {
    if semver::Version::parse(spec.trim()).is_ok() {
        return false;
    }
    let s = spec.trim();
    ["^", "~", "*", "x", "X", "||", "-", ">", "<", "=", " "]
        .iter()
        .any(|t| s.contains(t))
}

fn normalize_version(spec: &str) -> (String, Option<String>) {
    // If spec parses exactly as semver -> already exact
    if semver::Version::parse(spec.trim()).is_ok() {
        return (spec.trim().to_string(), None);
    }
    // Strip common range prefixes (^, ~, >=, <=, >, <)
    let mut s = spec.trim().to_string();
    // take first token if space separated (avoid complex OR ranges)
    if let Some((first, _rest)) = s.split_once(' ') {
        s = first.to_string();
    }
    if let Some(pos) = s.find("||") {
        s = s[..pos].trim().to_string();
    }
    // Remove leading range operators
    let mut trimmed = s.trim_start_matches(['^', '~', '>', '=', '<']).to_string();
    trimmed = trimmed.trim_start_matches('=').to_string();
    trimmed = trimmed.trim().to_string();
    // Cut off trailing constraint characters like "," or ")"
    let cleaned = trimmed
        .trim_matches(|c: char| c == ',' || c == ')')
        .to_string();
    // If still not a valid version fall back to original (will cause verify error later)
    if semver::Version::parse(&cleaned).is_ok() {
        (cleaned, Some(spec.to_string()))
    } else {
        (spec.to_string(), None)
    }
}

fn range_satisfies(range: &str, version: &str) -> bool {
    let v = match semver::Version::parse(version.trim()) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let mut r = range.trim().to_string();
    // normalize wildcards
    r = r.replace('*', "x");
    // hyphen range A - B -> ">=A <=B" (handle with spaces to avoid accidental minus in pre-release)
    if r.contains('-') && r.contains(' ') {
        if let Some((a, b)) = r.split_once('-') {
            let a = a.trim();
            let b = b.trim();
            if !a.is_empty() && !b.is_empty() {
                r = format!(">={a} <={b}");
            }
        }
    }
    if let Ok(req) = semver::VersionReq::parse(&r) {
        return req.matches(&v);
    }
    false
}

fn install_tool(client: &Client, tool: &Tool, pb: Option<&ProgressBar>) -> Result<()> {
    if let Ok(installed) = find_installed_version(tool) {
        if installed == tool.version {
            if let Some(p) = pb {
                p.set_message(format!("{} already at {} (skip)", tool.name, installed));
            } else {
                println!("{} already at {} (skipping)", tool.name, installed);
            }
            return Ok(());
        }
    }
    match tool.kind {
        ToolKind::Archive => install_archive(client, tool, pb),
        ToolKind::Direct => install_direct(client, tool, pb),
    }
}

fn ensure_dir(p: &Path) -> Result<()> {
    fs::create_dir_all(p).with_context(|| format!("creating dir {p:?}"))
}

fn expand_source(tool: &Tool) -> String {
    render_source(tool)
}

fn target_bin_filename(tool: &Tool) -> String {
    platform().final_binary_name(&tool.name)
}

fn install_direct(client: &Client, tool: &Tool, pb: Option<&ProgressBar>) -> Result<()> {
    let url = expand_source(tool);
    if let Some(p) = pb {
        p.set_message(format!("GET {}", tool.name));
    }
    let resp = client
        .get(&url)
        .send()
        .with_context(|| format!("GET {url}"))?;
    if !resp.status().is_success() {
        return Err(anyhow!("download failed {}", resp.status()));
    }
    let bytes = resp.bytes().with_context(|| "reading body")?.to_vec();

    if let Some(expected) = &tool.sha256 {
        verify_sha256(&bytes, expected)?;
    }

    let install_dir = install_dir(tool)?;
    ensure_dir(&install_dir)?;
    let bin_path = install_dir.join(&target_bin_filename(tool));
    fs::write(&bin_path, &bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&bin_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&bin_path, perms)?;
    }
    Ok(())
}

fn install_archive(client: &Client, tool: &Tool, pb: Option<&ProgressBar>) -> Result<()> {
    let url = expand_source(tool);
    if let Some(p) = pb {
        p.set_message(format!("GET {}", tool.name));
    }
    let resp = client
        .get(&url)
        .send()
        .with_context(|| format!("GET {url}"))?;
    if !resp.status().is_success() {
        return Err(anyhow!("download failed {}", resp.status()));
    }
    let bytes = resp.bytes().with_context(|| "reading body")?.to_vec();
    if let Some(expected) = &tool.sha256 {
        verify_sha256(&bytes, expected)?;
    }

    let install_dir = install_dir(tool)?;
    ensure_dir(&install_dir)?;

    // Detect archive type
    let mut extracted = false;
    if url.ends_with(".tar.gz") || url.ends_with(".tgz") {
        if let Some(p) = pb {
            p.set_message(format!("Extract {}", tool.name));
        }
        let tar = GzDecoder::new(&bytes[..]);
        let mut archive = Archive::new(tar);
        let bin_rel = tool.binary.clone().unwrap_or_else(|| tool.name.clone());
        let candidates = platform().candidate_archive_entry_names(&bin_rel);
        for entry in archive.entries()? {
            let mut e = entry?;
            let path = e.path()?;
            if candidates.iter().any(|c| path.ends_with(c)) {
                let bin_path = install_dir.join(&target_bin_filename(tool));
                let mut out = File::create(&bin_path)?;
                std::io::copy(&mut e, &mut out)?;
                chmod_exec(&bin_path)?;
                extracted = true;
            }
        }
    } else if url.ends_with(".zip") {
        if let Some(p) = pb {
            p.set_message(format!("Extract {}", tool.name));
        }
        let cursor = std::io::Cursor::new(&bytes);
        let mut zip = ZipArchive::new(cursor)?;
        let bin_rel = tool.binary.clone().unwrap_or_else(|| tool.name.clone());
        let candidates = platform().candidate_archive_entry_names(&bin_rel);
        for i in 0..zip.len() {
            let mut file = zip.by_index(i)?;
            let name = file.name().to_string();
            if candidates.iter().any(|c| name.ends_with(c)) {
                let bin_path = install_dir.join(&target_bin_filename(tool));
                let mut out = File::create(&bin_path)?;
                std::io::copy(&mut file, &mut out)?;
                chmod_exec(&bin_path)?;
                extracted = true;
            }
        }
    } else {
        return Err(anyhow!("unsupported archive type for {url}"));
    }
    if !extracted {
        return Err(anyhow!(format!(
            "did not find expected binary '{}' inside archive for {}",
            tool.binary.clone().unwrap_or_else(|| tool.name.clone()),
            tool.name
        )));
    }
    Ok(())
}

fn chmod_exec(path: &Path) -> Result<()> {
    platform().make_executable(path)
}

fn verify_sha256(data: &[u8], expected: &str) -> Result<()> {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let digest = hasher.finalize();
    let hex = hex::encode(digest);
    if &hex != expected {
        return Err(anyhow!("checksum mismatch expected {expected} got {hex}"));
    }
    Ok(())
}

fn install_dir(_tool: &Tool) -> Result<PathBuf> {
    Ok(project_root()
        .unwrap_or(std::env::current_dir()?)
        .join(".tlk")
        .join("bin"))
}

fn project_root() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        if dir.join("tlk.toml").exists() {
            return Some(dir);
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

fn find_installed_version(tool: &Tool) -> Result<String> {
    // naive: run --version and parse first semver
    let dir = install_dir(tool)?;
    let bin = dir.join(&target_bin_filename(tool));
    if !bin.exists() {
        // Try without extension for legacy installs
        #[cfg(windows)]
        {
            let legacy = dir.join(&tool.name);
            if legacy.exists() {
                return Ok(extract_version_from_binary(&legacy)?);
            }
        }
        return Err(anyhow!("not installed"));
    }
    extract_version_from_binary(&bin)
}

fn extract_version_from_binary(path: &Path) -> Result<String> {
    let output = Command::new(path).arg("--version").output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    for tok in stdout.split_whitespace() {
        if let Ok(v) = Version::parse(tok.trim_start_matches('v')) {
            return Ok(v.to_string());
        }
    }
    Ok("unknown".into())
}

pub fn compute_installed_digest(tool: &Tool) -> Result<String> {
    let dir = install_dir(tool)?;
    let bin = dir.join(&target_bin_filename(tool));
    if !bin.exists() {
        return Err(anyhow!("not installed"));
    }
    let data = fs::read(&bin)?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    Ok(hex::encode(hasher.finalize()))
}

// platform-specific helpers moved to platform module
