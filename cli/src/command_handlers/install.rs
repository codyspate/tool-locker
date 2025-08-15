use crate::config::TlkConfig;
use crate::{installer, ops, versioning};
use anyhow::Result;

pub struct InstallArgs<'a> {
    pub write_lock: bool,
    pub locked: bool,
    pub no_verify: bool,
    pub specs: &'a [String],
    pub exact: bool,
    pub config_path: &'a str,
    pub cfg: Option<&'a TlkConfig>,
}

pub fn run_install(args: InstallArgs) -> Result<()> {
    if args.locked && !args.specs.is_empty() {
        anyhow::bail!("--locked cannot be combined with specs");
    }
    if args.locked {
        if args.write_lock {
            eprintln!("Note: --locked implies no lock updates; ignoring implied write");
        }
        ops::install_locked("tlk.lock", args.cfg)?;
        return Ok(());
    }
    if args.specs.is_empty() {
        if !args.write_lock && !args.no_verify {
            installer::verify_lockfile(args.cfg, "tlk.lock")?;
        }
        installer::install_all(args.cfg)?;
        if args.write_lock {
            installer::write_lockfile(args.cfg, "tlk.lock")?;
        }
        return Ok(());
    }
    // Resolve requested known tools (currently only known-tool path supported for multi-spec)
    let mut resolved = Vec::new();
    for spec in args.specs {
        let (name, ver_opt) = parse_spec(spec)?;
        let per_spec_latest = ver_opt.is_none() || ver_opt.as_deref() == Some("latest");
        let version = if let Some(v) = &ver_opt {
            if v == "latest" {
                versioning::fetch_latest(&name)?
            } else {
                resolve_version(&name, v)?
            }
        } else {
            versioning::fetch_latest(&name)?
        };
        let tool = crate::known_tools::build_known_tool(&name, &version)?;
        resolved.push((tool, per_spec_latest, ver_opt.clone()));
    }
    // Parallel install
    let tools_only: Vec<_> = resolved.iter().map(|(t, _, _)| t.clone()).collect();
    let results = installer::install_tools_parallel(&tools_only);
    // Report using earlier collected metadata
    for (tool, per_spec_latest, _) in &resolved {
        let success = results
            .iter()
            .find(|(n, _)| n == &tool.name)
            .map(|(_, r)| r.is_ok())
            .unwrap_or(false);
        if success {
            if !args.exact && !per_spec_latest {
                println!("Installed {} {} (non-exact)", tool.name, tool.version);
            } else {
                println!("Installed {} {}", tool.name, tool.version);
            }
        }
    }
    // Lock update in batch
    if args.write_lock {
        for (tool, per_spec_latest, original_spec) in &resolved {
            let name = tool.name.clone();
            if let Err(e) = ops::write_single_lock(tool) {
                eprintln!("Warning: failed to update lock for {}: {e}", name);
            } else if let Err(e) = crate::command_handlers::specs::canonicalize_spec_logging(
                args.config_path,
                &name,
                if *per_spec_latest {
                    None
                } else {
                    original_spec.as_deref()
                },
                &tool.version,
                args.exact,
                *per_spec_latest,
            ) {
                eprintln!("Warning: failed to update config for {}: {e}", name);
            }
        }
    }
    Ok(())
}

fn parse_spec(spec: &str) -> anyhow::Result<(String, Option<String>)> {
    if let Some((n, v)) = spec.split_once('@') {
        Ok((n.to_string(), Some(v.to_string())))
    } else {
        Ok((spec.to_string(), None))
    }
}

// build_known_tool moved to known_tools::build_known_tool

fn resolve_version(name: &str, spec: &str) -> anyhow::Result<String> {
    if semver::Version::parse(spec).is_ok() {
        return Ok(spec.to_string());
    }
    let all = versioning::fetch_all_versions(name)?;
    if spec.contains("||") {
        let mut best: Option<semver::Version> = None;
        for clause in spec.split("||") {
            let clause = clause.trim();
            if clause.is_empty() {
                continue;
            }
            if let Ok(vs) = resolve_version(name, clause) {
                if let Ok(ver) = semver::Version::parse(&vs) {
                    if best.as_ref().map(|b| ver > *b).unwrap_or(true) {
                        best = Some(ver);
                    }
                }
            }
        }
        if let Some(v) = best {
            return Ok(v.to_string());
        }
    }
    if spec.contains('-') && spec.contains(' ') {
        if let Some((a, b)) = spec.split_once('-') {
            let (a, b) = (a.trim(), b.trim());
            if !a.is_empty() && !b.is_empty() {
                let tr = format!(">={a} <={b}");
                if let Ok(v) = resolve_version(name, &tr) {
                    return Ok(v);
                }
            }
        }
    }
    let mut normalized = spec.replace('*', "x");
    if semver::Version::parse(&normalized).is_err() {
        if normalized.chars().filter(|c| *c == '.').count() == 1
            && !normalized.contains('x')
            && !normalized.contains('^')
            && !normalized.contains('~')
        {
            normalized.push_str(".x");
        } else if normalized.chars().filter(|c| *c == '.').count() == 0
            && normalized.chars().all(|c| c.is_ascii_digit())
        {
            normalized.push_str(".x");
        }
    }
    if let Ok(req) = semver::VersionReq::parse(&normalized) {
        for v in &all {
            if req.matches(v) {
                return Ok(v.to_string());
            }
        }
    }
    for v in &all {
        if v.to_string().starts_with(spec) {
            return Ok(v.to_string());
        }
    }
    Err(anyhow::anyhow!(
        "cannot resolve version spec '{spec}' for {name}"
    ))
}
