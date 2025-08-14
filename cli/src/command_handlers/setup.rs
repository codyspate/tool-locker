use anyhow::Result;

// New setup strategy:
// Instead of creating a global bin dir & editing PATH directly, we simply place an eval line
// into the user's shell profile so dynamic activation (tlk hook) manages PATH per project.
// Supports bash, zsh, fish, PowerShell, and fallback instructions.
pub fn setup_flow(apply: bool) -> Result<()> {
    let eval_line = r#"# tlk dynamic activation
if command -v tlk >/dev/null 2>&1; then
  eval "$(tlk hook)"
fi"#;

    if !apply {
        println!("Add the following to your shell profile (~/.bashrc, ~/.zshrc, ~/.config/fish/config.fish, or PowerShell profile):\n\n{}\n", eval_line);
        return Ok(());
    }

    let mut wrote_any = false;

    #[cfg(windows)]
    {
        // PowerShell profile (current user, current host)
        if let Some(profile) = powershell_profile_path() {
            append_if_missing(&profile, eval_line)?;
            println!(
                "Appended tlk hook eval to PowerShell profile: {}",
                profile.display()
            );
            wrote_any = true;
        }
        // Also try Git Bash / MSYS ~/.bashrc if exists
        if let Some(home) = dirs::home_dir() {
            let bashrc = home.join(".bashrc");
            if bashrc.exists() {
                append_if_missing(&bashrc, eval_line)?;
                println!("Appended tlk hook eval to {}", bashrc.display());
                wrote_any = true;
            }
        }
    }

    #[cfg(not(windows))]
    {
        use std::path::PathBuf;
        let mut candidates: Vec<(PathBuf, &'static str)> = Vec::new();
        if let Some(home) = dirs::home_dir() {
            candidates.push((home.join(".bashrc"), "bash"));
            candidates.push((home.join(".zshrc"), "zsh"));
            candidates.push((home.join(".profile"), "profile"));
            candidates.push((home.join(".bash_profile"), "bash_profile"));
            candidates.push((home.join(".config/fish/config.fish"), "fish"));
        }
        for (path, label) in candidates {
            // Only write to existing shell files except bashrc/zshrc where we'll create if missing.
            let create_ok = matches!(label, "bash" | "zsh");
            if path.exists() || create_ok {
                append_if_missing(&path, eval_line)?;
                println!("Ensured tlk hook eval present in {}", path.display());
                wrote_any = true;
            }
        }
    }

    if !wrote_any {
        println!(
            "Could not locate a shell profile to update automatically. Add manually:\n\n{}\n",
            eval_line
        );
    } else {
        println!(
            "Setup complete. Open a new shell or source your profile to activate tlk dynamic PATH."
        );
    }
    Ok(())
}

#[cfg(windows)]
fn powershell_profile_path() -> Option<std::path::PathBuf> {
    // Try $HOME/Documents/PowerShell/Microsoft.PowerShell_profile.ps1
    use std::path::PathBuf;
    let home = dirs::home_dir()?;
    let candidates = [
        home.join("Documents/PowerShell/Microsoft.PowerShell_profile.ps1"),
        home.join("Documents/WindowsPowerShell/Microsoft.PowerShell_profile.ps1"),
    ];
    for c in candidates {
        if c.parent()
            .map(|p| std::fs::create_dir_all(p).is_ok())
            .unwrap_or(false)
        {
            return Some(c);
        }
    }
    None
}

fn append_if_missing(path: &std::path::Path, snippet: &str) -> Result<()> {
    use std::fs;
    use std::io::{Read, Write};
    let mut existing = String::new();
    if path.exists() {
        fs::File::open(path)?.read_to_string(&mut existing)?;
    }
    if !existing.contains("tlk hook") {
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        if !existing.ends_with('\n') {
            f.write_all(b"\n")?;
        }
        f.write_all(b"\n")?;
        f.write_all(snippet.as_bytes())?;
        f.write_all(b"\n")?;
    }
    Ok(())
}
