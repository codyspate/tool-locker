use crate::platform::PlatformOps;
use anyhow::Result;
use std::path::{Path, PathBuf};

pub static WINDOWS_PLATFORM: Windows = Windows;

pub struct Windows;

impl PlatformOps for Windows {
    fn home_dir(&self) -> Option<PathBuf> { std::env::var_os("USERPROFILE").map(PathBuf::from) }
    fn global_bin_dir(&self) -> Option<PathBuf> { self.home_dir().map(|h| h.join(".tlk").join("bin")) }
    fn final_binary_name(&self, base: &str) -> String { if base.ends_with(".exe") { base.to_string() } else { format!("{base}.exe") } }
    fn candidate_archive_entry_names(&self, base: &str) -> Vec<String> { if base.ends_with(".exe") { vec![base.to_string()] } else { vec![base.to_string(), format!("{base}.exe")] } }
    fn adjust_direct_url(&self, url: &str) -> String { if url.ends_with(".exe") || url.ends_with(".zip") || url.ends_with(".tar.gz") { url.to_string() } else { format!("{url}.exe") } }
    fn make_executable(&self, _path: &Path) -> Result<()> { Ok(()) }
}
