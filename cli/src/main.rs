mod cli;
mod command_handlers;
mod config;
mod installer;
mod known_tools;
mod lock;
mod ops;
mod platform;
mod unknown_tools;
mod versioning;

use anyhow::Result;
use clap::Parser;

use crate::cli::Cli;
use crate::config::TlkConfig;

// CLI definitions moved to cli.rs

fn main() -> Result<()> {
    let cli = Cli::parse();
    let path = cli.config.clone().unwrap_or_else(|| "tlk.toml".to_string());
    let cfg = TlkConfig::load(&path)?;
    command_handlers::dispatch::dispatch(cli.command, &cfg, &path)?;
    Ok(())
}
