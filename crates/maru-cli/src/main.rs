//! `maru` — the profile-manager binary. GENESIS §8.
#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "the CLI is the primary user-facing output channel"
)]
// Binary crate: many workspace lints aren't appropriate here.
#![allow(unreachable_pub)]
#![allow(missing_docs)]
#![allow(clippy::missing_docs_in_private_items)]
#![allow(
    clippy::needless_pass_by_value,
    reason = "subcommand handlers consume Args structs by value to avoid lifetime gymnastics in clap derive output"
)]
#![allow(
    clippy::too_many_lines,
    reason = "subcommand handlers prefer a single readable function over micro-decomposition"
)]
#![allow(
    clippy::format_in_format_args,
    clippy::format_collect,
    clippy::format_push_string,
    clippy::map_unwrap_or,
    reason = "ergonomics > pedantic style for the CLI surface"
)]
#![allow(
    clippy::exit,
    clippy::let_underscore_must_use,
    clippy::let_underscore_untyped,
    reason = "process::exit and let-_-init are intentional in main.rs"
)]

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

mod cmd;
mod context;
mod output;

use context::CliContext;

/// `maru` — switch between AI agent profiles.
#[derive(Debug, Parser)]
#[command(name = "maru", version, about, long_about = None)]
struct Cli {
    /// Override `$MARU_HOME` for this invocation.
    #[arg(long, global = true, env = "MARU_HOME")]
    maru_home: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Manage profiles (create, list, use, ...).
    #[command(subcommand)]
    Profile(cmd::profile::ProfileCmd),

    /// Inspect adapters and harness binaries.
    #[command(subcommand)]
    Adapter(cmd::adapter::AdapterCmd),

    /// Run a one-shot command under a specific profile.
    Run(cmd::run::RunArgs),

    /// Diagnose the install: PATH, shims, harness binaries, carve-outs.
    Doctor(cmd::doctor::DoctorArgs),

    /// Install shims and prepend `$MARU_HOME/bin` to PATH (via shell rc).
    Install(cmd::install::InstallArgs),

    /// Remove shims; with `--purge`, also delete `$MARU_HOME`.
    Uninstall(cmd::install::UninstallArgs),

    /// Print the version (with `--json` for machine-readable form).
    Version(cmd::version::VersionArgs),

    /// Emit the stable JSON schema for `--json` outputs and `state.toml`.
    Schema,

    /// Self-update via dist artifacts (Phase 4 task 4.7).
    Update(cmd::update::UpdateArgs),
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    init_tracing();

    let maru_home = match resolve_maru_home(cli.maru_home.clone()) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("maru: {e:#}");
            return ExitCode::from(2);
        }
    };
    let ctx = CliContext::new(maru_home);

    let result = match cli.command {
        Command::Profile(cmd) => cmd::profile::run(&ctx, cmd),
        Command::Adapter(cmd) => cmd::adapter::run(&ctx, cmd),
        Command::Run(args) => cmd::run::run(&ctx, args),
        Command::Doctor(args) => cmd::doctor::run(&ctx, args),
        Command::Install(args) => cmd::install::run_install(&ctx, args),
        Command::Uninstall(args) => cmd::install::run_uninstall(&ctx, args),
        Command::Version(args) => cmd::version::run(args),
        Command::Schema => cmd::schema::run(),
        Command::Update(args) => cmd::update::run(args),
    };

    match result {
        Ok(()) => ExitCode::from(0),
        Err(e) => {
            eprintln!("maru: error: {e:#}");
            // Preserve the GENESIS §8 exit-code scheme via downcast.
            if let Some(ce) = e.downcast_ref::<CliError>() {
                return ExitCode::from(ce.code);
            }
            ExitCode::from(1)
        }
    }
}

fn init_tracing() {
    use tracing_subscriber::{EnvFilter, fmt};
    let filter = EnvFilter::try_from_env("MARU_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = fmt().with_env_filter(filter).with_target(false).try_init();
}

fn resolve_maru_home(override_path: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(p) = override_path {
        return Ok(p);
    }
    let dirs = directories::ProjectDirs::from("dev", "maru", "maru")
        .context("could not derive default $MARU_HOME via `directories`")?;
    Ok(dirs.data_dir().to_path_buf())
}

/// CLI-side error with a GENESIS §8 exit code. Wrap in `anyhow::Error`
/// so `?` works through the call chain.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct CliError {
    pub code: u8,
    pub message: String,
}

impl CliError {
    pub fn user(msg: impl Into<String>) -> Self {
        Self {
            code: 1,
            message: msg.into(),
        }
    }

    pub fn env(msg: impl Into<String>) -> Self {
        Self {
            code: 2,
            message: msg.into(),
        }
    }
}

#[allow(dead_code, reason = "may be used for fail-fast bail-style")]
fn _bail_user(msg: &str) -> ! {
    eprintln!("maru: {msg}");
    std::process::exit(1);
}
