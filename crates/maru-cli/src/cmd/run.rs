//! `maru run --profile <name> [--dry-run] -- <cmd> [args...]`

use std::ffi::OsString;

use anyhow::{Context, Result, bail};
use clap::Args;
use maru_core::{Environment, HarnessId, ProfileContext, ProfileName};
use serde::Serialize;

use crate::{CliContext, CliError};

#[derive(Debug, Args)]
pub struct RunArgs {
    /// Profile to activate for this one-shot.
    #[arg(long)]
    pub profile: String,

    /// Print the activation plan as JSON instead of executing.
    #[arg(long)]
    pub dry_run: bool,

    /// The command + args to run, after `--`.
    #[arg(trailing_var_arg = true, required_unless_present = "dry_run")]
    pub command: Vec<OsString>,
}

#[derive(Debug, Serialize)]
struct DryRunOutput {
    profile: String,
    harness: String,
    env: Vec<(String, String)>,
    args_prefix: Vec<String>,
    diagnostics: Vec<DiagOut>,
}

#[derive(Debug, Serialize)]
struct DiagOut {
    level: String,
    message: String,
    help: Option<String>,
}

pub fn run(ctx: &CliContext, args: RunArgs) -> Result<()> {
    let name = ProfileName::new(args.profile.clone())
        .map_err(|e| CliError::user(format!("invalid profile: {e}")))?;

    // The first argv element after `--` selects the harness via its
    // basename (matching the shim's argv[0] dispatch).
    let cmd = args.command.first().ok_or_else(|| {
        CliError::user(
            "missing command. Usage: maru run --profile <name> -- <claude|codex|gemini> [args...]"
                .to_owned(),
        )
    })?;
    let basename = std::path::Path::new(cmd)
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| CliError::user(format!("invalid command {cmd:?}")))?;
    let harness: HarnessId = basename.parse().map_err(|e| {
        CliError::user(format!(
            "command must be a known harness ({basename:?}): {e}"
        ))
    })?;

    let adapter = maru_adapters::adapter_for(harness)
        .ok_or_else(|| CliError::user(format!("no adapter for {}", harness.as_str())))?;

    let profile_root = ctx.profile_root(name.as_str());
    let env = maru_core::SystemEnvironment::new();
    // GENESIS §7.1 Linux Claude gate checks `home_dir.join(...)`; if we
    // silently default to `PathBuf::new()` here, the gate would test the
    // current working directory instead of $HOME, which is a silent
    // misfire. Fail loudly instead.
    let home = env
        .home_dir()
        .ok_or_else(|| CliError::env("could not resolve home directory"))?;
    let profile_ctx = ProfileContext {
        profile_name: &name,
        profile_root: &profile_root,
        harness,
        home_dir: &home,
        project_pin: None,
    };
    let plan = adapter
        .plan(&profile_ctx, &env)
        .context("adapter plan failed")?;

    if args.dry_run {
        let out = DryRunOutput {
            profile: name.as_str().to_owned(),
            harness: harness.as_str().to_owned(),
            env: plan
                .env
                .iter()
                .map(|(k, v)| {
                    (
                        k.to_string_lossy().into_owned(),
                        v.to_string_lossy().into_owned(),
                    )
                })
                .collect(),
            args_prefix: plan
                .args_prefix
                .iter()
                .map(|s| s.to_string_lossy().into_owned())
                .collect(),
            diagnostics: plan
                .diagnostics
                .iter()
                .map(|d| DiagOut {
                    level: format!("{:?}", d.level).to_lowercase(),
                    message: d.message.clone(),
                    help: d.help.clone(),
                })
                .collect(),
        };
        let s = serde_json::to_string_pretty(&out)?;
        println!("{s}");
        return Ok(());
    }

    // Process diagnostics + apply env + exec.
    if let Err(e) = maru_activation::process_diagnostics(&plan.diagnostics) {
        match e {
            maru_activation::Error::Blocked { message, help } => {
                eprintln!("maru: gate: {message}");
                if let Some(h) = help {
                    eprintln!("       {h}");
                }
                bail!(CliError {
                    code: 3,
                    message: "adapter gate blocked activation".to_owned(),
                });
            }
            other => bail!(other),
        }
    }

    maru_activation::apply_env(&plan);

    // Resolve real binary, exec/spawn.
    let our_install_dir = ctx.shim_install_dir();
    let real = maru_activation::resolve_real_binary(
        &env,
        adapter.binary_names().first().copied().unwrap_or(basename),
        &our_install_dir,
    )
    .context("resolve real binary")?;

    let prog_name = std::ffi::OsString::from(basename);
    let tail: Vec<OsString> = args.command.iter().skip(1).cloned().collect();
    let cmd_argv = maru_activation::build_argv(&prog_name, &plan, &tail);

    #[cfg(unix)]
    {
        let _: std::convert::Infallible = match maru_activation::exec_or_spawn(&real, &cmd_argv) {
            Ok(_) => unreachable!(),
            Err(e) => bail!(e),
        };
    }
    #[cfg(windows)]
    {
        let code = maru_activation::exec_or_spawn(&real, &cmd_argv).context("exec_or_spawn")?;
        std::process::exit(code);
    }
}
