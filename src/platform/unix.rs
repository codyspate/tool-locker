use crate::platform::PlatformOps;
use anyhow::Result;
use std::path::{Path, PathBuf};

pub static UNIX_PLATFORM: Unix = Unix;

pub struct Unix;

impl PlatformOps for Unix {
    fn home_dir(&self) -> Option<PathBuf> {
        std::env::var_os("HOME").map(PathBuf::from)
    }
    fn global_bin_dir(&self) -> Option<PathBuf> {
        self.home_dir().map(|h| h.join(".tlk").join("bin"))
    }
    fn final_binary_name(&self, base: &str) -> String {
        base.to_string()
    }
    fn candidate_archive_entry_names(&self, base: &str) -> Vec<String> {
        vec![base.to_string()]
    }
    fn adjust_direct_url(&self, url: &str) -> String {
        url.to_string()
    }
    fn make_executable(&self, path: &Path) -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms)?;
        Ok(())
    }
}
