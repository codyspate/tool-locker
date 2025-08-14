use crate::cli::Commands;
use crate::command_handlers::{hook, install, setup, uninstall, migrate, diagnose, migrate_config};
use crate::config::TlkConfig;
use anyhow::Result;

pub fn dispatch(cmd: Commands, cfg: &TlkConfig, config_path: &str) -> Result<()> {
    match cmd {
        Commands::Install {
            no_lock,
            locked,
            no_verify,
            specs,
            exact,
        } => {
            let args = install::InstallArgs {
                write_lock: !no_lock,
                locked,
                no_verify,
                specs: &specs,
                exact,
                config_path,
                cfg,
            };
            install::run_install(args)
        }
        Commands::Plan => crate::installer::plan(cfg),
        Commands::List => crate::installer::list(cfg),
        Commands::Verify => crate::installer::verify_lockfile(cfg, "tlk.lock"),
        Commands::Uninstall { names } => {
            if names.is_empty() {
                anyhow::bail!("at least one tool name required");
            }
            for name in names {
                if let Err(e) = uninstall::uninstall_tool(config_path, &name) {
                    eprintln!("Uninstall {} failed: {}", name, e);
                } else {
                    println!("Uninstalled {}", name);
                }
            }
            Ok(())
        }
        Commands::Setup { apply } => setup::setup_flow(apply),
        Commands::Hook { shell } => hook::print_hook(shell.as_deref()),
    Commands::MigrateLock => migrate::migrate_lock(cfg, "tlk.lock"),
    Commands::MigrateConfig => migrate_config::migrate_config(config_path),
    Commands::Diagnose { lock, kind } => {
        match kind.as_str() {
            "missing-platforms" => diagnose::list_missing(&lock),
            other => anyhow::bail!("unknown diagnose kind '{other}'"),
        }
    }
    }
}
