//! `maru doctor` — install + carve-out diagnostics.

use anyhow::Result;
use clap::Args;
use maru_core::Environment;
use serde::Serialize;

use crate::{CliContext, output};

#[derive(Debug, Args)]
pub struct DoctorArgs {
    #[arg(long)]
    pub json: bool,
    /// Reserved for future use; rename + write fixes are not yet wired.
    #[arg(long)]
    pub fix: bool,
}

#[derive(Debug, Serialize)]
struct DoctorOutput {
    maru_home: String,
    shim_install_dir: String,
    shim_install_dir_exists: bool,
    on_path: bool,
    adapters: Vec<AdapterCheck>,
    notes: Vec<String>,
}

#[derive(Debug, Serialize)]
struct AdapterCheck {
    id: String,
    real_binary: Option<String>,
    profile_subdir: String,
}

pub fn run(ctx: &CliContext, args: DoctorArgs) -> Result<()> {
    let env = maru_core::SystemEnvironment::new();
    let shim_dir = ctx.shim_install_dir();
    let path_entries = env.path();
    let on_path = path_entries
        .iter()
        .any(|p| p.canonicalize().ok() == shim_dir.canonicalize().ok());

    let mut adapter_checks = Vec::new();
    for adapter in maru_adapters::v1_adapters() {
        let detection = adapter.detect(&env);
        adapter_checks.push(AdapterCheck {
            id: adapter.id().as_str().to_owned(),
            real_binary: detection.path().map(|p| p.display().to_string()),
            profile_subdir: adapter.profile_subdir().display().to_string(),
        });
    }

    let mut notes = Vec::new();
    if !shim_dir.exists() {
        notes.push(format!(
            "shim install dir {} does not exist; run `maru install` to create",
            shim_dir.display()
        ));
    }
    if !on_path {
        notes.push(format!(
            "{} is not on PATH; shims won't intercept invocations until you re-source your shell rc",
            shim_dir.display()
        ));
    }
    if args.fix {
        notes.push("--fix is reserved for a follow-up; this run made no changes".to_owned());
    }
    notes.push(
        "Phase 1 known limitation: extension hosts (VS Code Anthropic Claude / Codex extensions) do NOT inherit env from shell rc; integrated terminals do".to_owned()
    );
    notes.push(
        "Phase 1 known limitation: GUI-launched IDEs (Dock/Spotlight on macOS, Start Menu on Windows) do NOT inherit shell rc env; use `launchctl setenv` / `setx` if needed".to_owned()
    );

    let out = DoctorOutput {
        maru_home: ctx.maru_home().display().to_string(),
        shim_install_dir: shim_dir.display().to_string(),
        shim_install_dir_exists: shim_dir.exists(),
        on_path,
        adapters: adapter_checks,
        notes,
    };

    output::emit(&out, args.json, |o| {
        println!("MARU_HOME:           {}", o.maru_home);
        println!(
            "Shim install dir:    {} ({})",
            o.shim_install_dir,
            if o.shim_install_dir_exists {
                "exists"
            } else {
                "missing"
            }
        );
        println!("On PATH:             {}", o.on_path);
        println!("\nAdapters:");
        for a in &o.adapters {
            let bin = a.real_binary.as_deref().unwrap_or("(not found on PATH)");
            println!("  {:<10}  real={}  subdir={}", a.id, bin, a.profile_subdir);
        }
        if !o.notes.is_empty() {
            println!("\nNotes:");
            for n in &o.notes {
                println!("  - {n}");
            }
        }
    })
}
