use anyhow::Result;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::sync::Mutex;

static VERSION_CACHE: Lazy<Mutex<HashMap<String, Vec<semver::Version>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub fn fetch_latest(name: &str) -> Result<String> {
    let all = fetch_all_versions(name)?;
    all.first()
        .map(|v| v.to_string())
        .ok_or_else(|| anyhow::anyhow!("no versions found for {name}"))
}

pub fn fetch_all_versions(name: &str) -> Result<Vec<semver::Version>> {
    {
        let cache = VERSION_CACHE.lock().unwrap();
        if let Some(list) = cache.get(name) {
            return Ok(list.clone());
        }
    }
    let raw: Vec<String> = match name {
        "terraform" => fetch_hashicorp_list("terraform")?,
        "helm" => fetch_github_list("helm", "helm")?,
        "gh" => fetch_github_list("cli", "cli")?,
        "buf" => fetch_github_list("bufbuild", "buf")?,
        "kubectl" => fetch_github_list("kubernetes", "kubernetes")?,
        _ => return Err(anyhow::anyhow!("version listing unsupported for {name}")),
    };
    let mut parsed: Vec<semver::Version> = raw
        .into_iter()
        .filter_map(|s| semver::Version::parse(&s).ok())
        .collect();
    parsed.sort_by(|a, b| b.cmp(a));
    let mut cache = VERSION_CACHE.lock().unwrap();
    cache.insert(name.to_string(), parsed.clone());
    Ok(parsed)
}

fn fetch_hashicorp_list(tool: &str) -> Result<Vec<String>> {
    let url = format!("https://releases.hashicorp.com/{tool}/");
    let body = reqwest::blocking::get(&url)?.text()?;
    let re = Regex::new(&format!(r"/{tool}/([0-9]+\.[0-9]+\.[0-9]+)/"))?;
    let mut versions = Vec::new();
    for cap in re.captures_iter(&body) {
        versions.push(cap[1].to_string());
    }
    versions.sort();
    versions.dedup();
    Ok(versions)
}

fn fetch_github_list(owner: &str, repo: &str) -> Result<Vec<String>> {
    let url = format!("https://api.github.com/repos/{owner}/{repo}/releases?per_page=100");
    let client = reqwest::blocking::Client::new();
    let resp = client.get(url).header("User-Agent", "tlk").send()?;
    let arr: serde_json::Value = resp.json()?;
    let mut out = Vec::new();
    if let Some(items) = arr.as_array() {
        for it in items {
            if it.get("prerelease").and_then(|v| v.as_bool()).unwrap_or(false) {
                continue;
            }
            if let Some(tag) = it.get("tag_name").and_then(|v| v.as_str()) {
                let norm = tag.trim_start_matches('v').to_string();
                out.push(norm);
            }
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}
