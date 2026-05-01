//! `maru adapter ...` — adapter inspection.

use anyhow::Result;
use clap::{Args, Subcommand};
use serde::Serialize;

use crate::{CliContext, output};

#[derive(Debug, Subcommand)]
pub enum AdapterCmd {
    /// List all known adapters in this build.
    List(ListArgs),
    /// Show per-adapter status (real binary path, validation).
    Status(StatusArgs),
}

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct StatusArgs {
    /// Adapter id (claude, codex, gemini).
    pub id: String,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Serialize)]
struct AdapterEntry {
    id: String,
    binary_names: Vec<String>,
    profile_subdir: String,
}

pub fn run(_ctx: &CliContext, cmd: AdapterCmd) -> Result<()> {
    match cmd {
        AdapterCmd::List(args) => list(args),
        AdapterCmd::Status(args) => status(args),
    }
}

fn list(args: ListArgs) -> Result<()> {
    let adapters = maru_adapters::v1_adapters();
    let entries: Vec<AdapterEntry> = adapters
        .iter()
        .map(|a| AdapterEntry {
            id: a.id().as_str().to_owned(),
            binary_names: a.binary_names().iter().map(|s| (*s).to_owned()).collect(),
            profile_subdir: a.profile_subdir().display().to_string(),
        })
        .collect();
    output::emit(&entries, args.json, |list| {
        for e in list {
            println!(
                "{}: bins={}, subdir={}",
                e.id,
                e.binary_names.join(","),
                e.profile_subdir
            );
        }
    })
}

#[derive(Debug, Serialize)]
struct StatusOutput {
    id: String,
    found_at: Option<String>,
}

fn status(args: StatusArgs) -> Result<()> {
    let id: maru_core::HarnessId = args
        .id
        .parse()
        .map_err(|e| crate::CliError::user(format!("unknown harness {:?}: {e}", args.id)))?;
    let adapter = maru_adapters::adapter_for(id)
        .ok_or_else(|| crate::CliError::user(format!("no adapter for {}", id.as_str())))?;
    let env = maru_core::SystemEnvironment::new();
    let detection = adapter.detect(&env);
    let out = StatusOutput {
        id: id.as_str().to_owned(),
        found_at: detection.path().map(|p| p.display().to_string()),
    };
    output::emit(&out, args.json, |o| match &o.found_at {
        Some(p) => println!("{}: found at {}", o.id, p),
        None => println!("{}: not found on PATH", o.id),
    })
}
