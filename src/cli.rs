use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    version,
    name = "tlk",
    about = "Tool Locker: manage non-language tool dependencies defined in tlk.toml"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Path to config (defaults to ./tlk.toml)
    #[arg(short, long)]
    pub config: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Install / update declared tools OR one or more known tool specs.
    /// Examples:
    ///   tlk install                     # install all from tlk.toml
    ///   tlk install --lock              # install all + regenerate lock
    ///   tlk install terraform           # install latest terraform
    ///   tlk install terraform@1.7.5     # install specific version
    ///   tlk install terraform@latest helm@latest  # install latest for both
    Install {
        /// Skip writing tlk.lock (by default the lock file is written / updated)
        #[arg(long, alias = "no-lock")]
        no_lock: bool,
        /// Install exactly the versions recorded in tlk.lock (no lock writes; ignores provided specs and config versions)
        #[arg(long)]
        locked: bool,
        /// Skip lock verification (use with caution, only applies when installing all)
        #[arg(long)]
        no_verify: bool,
        /// Known tool specs (name or name@version). If omitted installs all tools from config.
        #[arg(value_name = "SPEC")]
        specs: Vec<String>,
        /// Treat provided version as exact (currently informational)
        #[arg(long)]
        exact: bool,
        // --latest removed; use per-spec @latest instead
    },
    /// Show what would be installed (no changes)
    Plan,
    /// List currently installed versions for declared tools
    List,
    /// Verify tlk.lock against config & installed binaries (no install)
    Verify,
    /// Uninstall one or more tools: removes binary, tlk.toml entries & tlk.lock entries
    Uninstall {
        /// Tool names to uninstall
        #[arg(value_name = "NAME")]
        names: Vec<String>,
    },
    /// One-time setup: create ~/.tlk/bin and optionally add it to PATH
    Setup {
        /// Append export line to shell rc (~/.bashrc or ~/.zshrc); otherwise just print instructions
        #[arg(long)]
        apply: bool,
    },
    /// Emit shell hook script for dynamic project .tlk/bin activation (bash|zsh|fish|powershell)
    Hook {
        /// Shell type (bash|zsh). If omitted, prints a universal script.
        #[arg(long)]
        shell: Option<String>,
    },
    /// Regenerate tlk.lock at latest schema (adds cross-platform sources)
    MigrateLock,
    /// Migrate tlk.toml legacy [[tools]] syntax to [tools.<name>] tables
    MigrateConfig,
    /// Diagnose lock issues (e.g., missing platform entries)
    Diagnose {
        #[arg(long, default_value = "tlk.lock")]
        lock: String,
        #[arg(long, default_value = "missing-platforms")]
        kind: String,
    },
}
