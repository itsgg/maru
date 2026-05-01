//! `maru profile ...` subcommand surface. GENESIS §8.

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};
use maru_core::{HarnessId, ProfileName};
use maru_store::{
    ProfileEntry,
    active::{read as read_active, write as write_active},
    profile_dirs::{apply_seeds, ensure_profile_dirs},
    snapshot::snapshot_state,
    state::{insert_profile, read as read_state, update as update_state},
};
use serde::Serialize;

use crate::{CliContext, CliError, output};

#[derive(Debug, Subcommand)]
pub enum ProfileCmd {
    /// Create a new profile.
    Create(CreateArgs),
    /// List all profiles.
    List(ListArgs),
    /// Set the currently active profile.
    Use(UseArgs),
    /// Print the currently resolved profile.
    Current(CurrentArgs),
    /// Delete a profile.
    Delete(DeleteArgs),
    /// Set the fallback profile (used when active.txt is empty).
    Default(DefaultArgs),
    /// Rename a profile.
    Rename(RenameArgs),
}

#[derive(Debug, Args)]
pub struct CreateArgs {
    /// Profile name.
    pub name: String,
    /// Comma-separated list of harnesses (claude,codex,gemini).
    #[arg(long, value_delimiter = ',', default_value = "claude")]
    pub harness: Vec<HarnessId>,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct UseArgs {
    pub name: String,
    /// Persist via shell rc (`MARU_PROFILE` export). Default writes
    /// `active.txt` only.
    #[arg(long)]
    pub persist_shell: bool,
}

#[derive(Debug, Args)]
pub struct CurrentArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct DeleteArgs {
    pub name: String,
    /// Force-delete a profile that has been activated before.
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct DefaultArgs {
    pub name: String,
}

#[derive(Debug, Args)]
pub struct RenameArgs {
    pub from: String,
    pub to: String,
}

pub fn run(ctx: &CliContext, cmd: ProfileCmd) -> Result<()> {
    match cmd {
        ProfileCmd::Create(args) => create(ctx, args),
        ProfileCmd::List(args) => list(ctx, args),
        ProfileCmd::Use(args) => use_(ctx, args),
        ProfileCmd::Current(args) => current(ctx, args),
        ProfileCmd::Delete(args) => delete(ctx, args),
        ProfileCmd::Default(args) => default(ctx, args),
        ProfileCmd::Rename(args) => rename(ctx, args),
    }
}

fn create(ctx: &CliContext, args: CreateArgs) -> Result<()> {
    let name = ProfileName::new(args.name.clone())
        .map_err(|e| CliError::user(format!("invalid profile name: {e}")))?;
    let entry = ProfileEntry::new(args.harness.clone());
    insert_profile(ctx.maru_home(), &name, entry).context("insert profile")?;

    let profile_root = ctx.profile_root(name.as_str());
    ensure_profile_dirs(&profile_root, &args.harness).context("create profile dirs")?;

    // Apply per-harness seeds (Phase 1: only Codex would seed, but
    // GENESIS §7.2 is currently `seed = []`).
    for harness in &args.harness {
        if let Some(adapter) = maru_adapters::adapter_for(*harness) {
            let harness_dir = profile_root.join(maru_store::profile_dirs::profile_subdir(*harness));
            let seeds = adapter.seed(&harness_dir);
            if !seeds.is_empty() {
                apply_seeds(&harness_dir, &seeds).context("apply seeds")?;
            }
        }
    }

    let harness_strs: Vec<&'static str> = args.harness.iter().map(|h| h.as_str()).collect();
    eprintln!(
        "maru: created profile {:?} with harnesses {harness_strs:?}",
        name.as_str(),
    );
    Ok(())
}

#[derive(Debug, Serialize)]
struct ListEntry {
    name: String,
    created_at: String,
    last_used_at: String,
    harnesses: Vec<String>,
    ever_activated: bool,
}

#[derive(Debug, Serialize)]
struct ListOutput {
    profiles: Vec<ListEntry>,
    active: Option<String>,
    default: Option<String>,
}

fn list(ctx: &CliContext, args: ListArgs) -> Result<()> {
    let state = read_state(ctx.maru_home()).context("read state.toml")?;
    let active = read_active(ctx.maru_home())
        .ok()
        .flatten()
        .map(|n| n.as_str().to_owned());
    let entries: Vec<ListEntry> = state
        .profiles
        .iter()
        .map(|(name, entry)| ListEntry {
            name: name.clone(),
            created_at: entry.created_at.clone(),
            last_used_at: entry.last_used_at.clone(),
            harnesses: entry
                .harnesses
                .iter()
                .map(|h| h.as_str().to_owned())
                .collect(),
            ever_activated: entry.ever_activated,
        })
        .collect();

    let out = ListOutput {
        profiles: entries,
        active,
        default: state.defaults.profile,
    };

    output::emit(&out, args.json, |o| {
        if o.profiles.is_empty() {
            eprintln!("(no profiles yet — try `maru profile create work --harness claude`)");
            return;
        }
        for p in &o.profiles {
            let marker = match (&o.active, &o.default) {
                (Some(a), _) if *a == p.name => "* ",
                (_, Some(d)) if *d == p.name => "+ ",
                _ => "  ",
            };
            println!(
                "{marker}{:<20}  harnesses={}  last_used={}",
                p.name,
                p.harnesses.join(","),
                p.last_used_at
            );
        }
        if let Some(a) = &o.active {
            eprintln!("\n* = active ({a})");
        }
        if let Some(d) = &o.default {
            eprintln!("+ = default ({d})");
        }
    })
}

fn use_(ctx: &CliContext, args: UseArgs) -> Result<()> {
    let name = ProfileName::new(args.name.clone())
        .map_err(|e| CliError::user(format!("invalid profile name: {e}")))?;
    let state = read_state(ctx.maru_home()).context("read state.toml")?;
    if !state.profiles.contains_key(name.as_str()) {
        bail!(CliError::user(format!(
            "no such profile {:?}",
            name.as_str()
        )));
    }
    write_active(ctx.maru_home(), Some(&name)).context("write active.txt")?;
    eprintln!("maru: active profile is now {:?}", name.as_str());

    if args.persist_shell {
        eprintln!(
            "maru: --persist-shell not yet implemented (will land in a follow-up); active.txt was still written"
        );
    }
    Ok(())
}

#[derive(Debug, Serialize)]
struct CurrentOutput {
    name: Option<String>,
    source: Option<String>,
}

fn current(ctx: &CliContext, args: CurrentArgs) -> Result<()> {
    let env = maru_core::SystemEnvironment::new();
    let cwd = std::env::current_dir().ok();
    let resolved = maru_store::resolve::resolve(&env, ctx.maru_home(), cwd.as_deref())
        .context("resolve profile")?;

    let out = CurrentOutput {
        name: resolved.as_ref().map(|r| r.name.as_str().to_owned()),
        source: resolved.as_ref().map(|r| format!("{:?}", r.source)),
    };
    output::emit(&out, args.json, |o| match (&o.name, &o.source) {
        (Some(n), Some(s)) => println!("{n} (source: {s})"),
        _ => println!("(no active profile)"),
    })
}

fn delete(ctx: &CliContext, args: DeleteArgs) -> Result<()> {
    let name = ProfileName::new(args.name.clone())
        .map_err(|e| CliError::user(format!("invalid profile name: {e}")))?;
    let state = read_state(ctx.maru_home()).context("read state.toml")?;
    let entry = state
        .profiles
        .get(name.as_str())
        .ok_or_else(|| CliError::user(format!("no such profile {:?}", name.as_str())))?;
    if entry.ever_activated && !args.force {
        bail!(CliError::user(format!(
            "profile {:?} has been activated before; pass --force to delete",
            name.as_str()
        )));
    }

    snapshot_state(ctx.maru_home()).context("snapshot before delete")?;

    update_state(ctx.maru_home(), |s| {
        s.profiles.remove(name.as_str());
        if s.defaults.profile.as_deref() == Some(name.as_str()) {
            s.defaults.profile = None;
        }
        Ok(())
    })
    .context("update state.toml")?;

    let profile_dir = ctx.profile_root(name.as_str());
    if profile_dir.exists() {
        std::fs::remove_dir_all(&profile_dir)
            .with_context(|| format!("remove profile dir {}", profile_dir.display()))?;
    }

    // If the deleted profile was active, clear active.txt.
    if let Ok(Some(active)) = read_active(ctx.maru_home())
        && active.as_str() == name.as_str()
    {
        write_active(ctx.maru_home(), None).context("clear active.txt")?;
    }

    eprintln!("maru: deleted profile {:?}", name.as_str());
    Ok(())
}

fn default(ctx: &CliContext, args: DefaultArgs) -> Result<()> {
    let name = ProfileName::new(args.name)
        .map_err(|e| CliError::user(format!("invalid profile name: {e}")))?;
    let state = read_state(ctx.maru_home()).context("read state.toml")?;
    if !state.profiles.contains_key(name.as_str()) {
        bail!(CliError::user(format!(
            "no such profile {:?}",
            name.as_str()
        )));
    }
    update_state(ctx.maru_home(), |s| {
        s.defaults.profile = Some(name.as_str().to_owned());
        Ok(())
    })
    .context("update defaults.profile")?;
    eprintln!("maru: default profile set to {:?}", name.as_str());
    Ok(())
}

fn rename(ctx: &CliContext, args: RenameArgs) -> Result<()> {
    let from = ProfileName::new(args.from)
        .map_err(|e| CliError::user(format!("invalid `from` name: {e}")))?;
    let to =
        ProfileName::new(args.to).map_err(|e| CliError::user(format!("invalid `to` name: {e}")))?;

    let state = read_state(ctx.maru_home()).context("read state.toml")?;
    if !state.profiles.contains_key(from.as_str()) {
        bail!(CliError::user(format!(
            "no such profile {:?}",
            from.as_str()
        )));
    }
    if state.profiles.contains_key(to.as_str()) {
        bail!(CliError::user(format!(
            "destination profile {:?} already exists",
            to.as_str()
        )));
    }

    snapshot_state(ctx.maru_home()).context("snapshot before rename")?;

    // Move the on-disk profile dir.
    let from_dir = ctx.profile_root(from.as_str());
    let to_dir = ctx.profile_root(to.as_str());
    if from_dir.exists() {
        std::fs::rename(&from_dir, &to_dir)
            .with_context(|| format!("rename {} -> {}", from_dir.display(), to_dir.display()))?;
    }

    update_state(ctx.maru_home(), |s| {
        if let Some(entry) = s.profiles.remove(from.as_str()) {
            s.profiles.insert(to.as_str().to_owned(), entry);
        }
        if s.defaults.profile.as_deref() == Some(from.as_str()) {
            s.defaults.profile = Some(to.as_str().to_owned());
        }
        Ok(())
    })
    .context("update state.toml")?;

    // If active, repoint.
    if let Ok(Some(active)) = read_active(ctx.maru_home())
        && active.as_str() == from.as_str()
    {
        write_active(ctx.maru_home(), Some(&to)).context("update active.txt")?;
    }

    eprintln!("maru: renamed {:?} -> {:?}", from.as_str(), to.as_str());
    Ok(())
}
