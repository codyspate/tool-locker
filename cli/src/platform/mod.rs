pub fn platform() -> &'static dyn PlatformOps {
    &ConcretePlatform
}

use anyhow::Result;
use std::path::{Path, PathBuf};

pub trait PlatformOps: Sync + Send {
    fn home_dir(&self) -> Option<PathBuf>;
    fn global_bin_dir(&self) -> Option<PathBuf>;
    fn final_binary_name(&self, base: &str) -> String;
    fn candidate_archive_entry_names(&self, base: &str) -> Vec<String>;
    fn adjust_direct_url(&self, url: &str) -> String;
    fn make_executable(&self, path: &Path) -> Result<()>;
}

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::UNIX_PLATFORM as ConcretePlatform;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::WINDOWS_PLATFORM as ConcretePlatform;
