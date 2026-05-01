//! `maru profile ...` subcommand surface. GENESIS §8.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};
use maru_core::{HarnessId, ProfileName};
use maru_store::{
    ProfileEntry,
    active::{read as read_active, write as write_active},
    copy::copy_filtered,
    profile_dirs::{apply_seeds, ensure_profile_dirs, profile_subdir},
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
    /// Write `.maru` in the cwd to pin a profile to this directory.
    Pin(PinArgs),
    /// Delete `.maru` in the cwd.
    Unpin,
    /// Clone a profile (excludes credentials per GENESIS §8 deny-list).
    Clone(CloneArgs),
    /// Export a profile as a tarball (excludes credentials).
    Export(ExportArgs),
    /// Import a previously-exported tarball into a new profile.
    Import(ImportArgs),
    /// Copy an existing `~/.<harness>` directory into a new maru profile.
    ImportExisting(ImportExistingArgs),
    /// Authenticate a profile against a harness's auth provider and save
    /// a per-profile credential the activation plan can export. Currently
    /// supports Claude (runs `claude setup-token` and writes the resulting
    /// OAuth token to `<profile>/claude/oauth_token`).
    Login(LoginArgs),
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

#[derive(Debug, Args)]
pub struct PinArgs {
    pub name: String,
    /// Overwrite an existing `.maru` even if it points elsewhere.
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct CloneArgs {
    /// Source profile name.
    pub from: String,
    /// Destination profile name.
    pub to: String,
}

#[derive(Debug, Args)]
pub struct ExportArgs {
    pub name: String,
    /// Output path for the tarball (e.g. `/tmp/work.tar.gz`).
    #[arg(long)]
    pub to: PathBuf,
}

#[derive(Debug, Args)]
pub struct ImportArgs {
    /// Path to a tarball previously written by `maru profile export`.
    pub from: PathBuf,
    /// Optional override for the resulting profile name.
    /// Defaults to the name embedded in the tarball.
    #[arg(long)]
    pub name: Option<String>,
}

#[derive(Debug, Args)]
pub struct ImportExistingArgs {
    /// Which harness to import (`claude`, `codex`, or `gemini`).
    #[arg(long)]
    pub harness: HarnessId,
    /// Profile name for the imported state.
    #[arg(long, default_value = "imported")]
    pub name: String,
    /// Override the source dir (default: `~/.<harness>`).
    #[arg(long)]
    pub from: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct LoginArgs {
    /// Profile to authenticate (must already exist).
    pub name: String,
    /// Which harness to log in to. Currently only `claude` is supported.
    #[arg(long, default_value = "claude")]
    pub harness: HarnessId,
    /// Read the token from stdin instead of running `claude setup-token`
    /// interactively. Useful for piping (`claude setup-token | tail -1
    /// | maru profile login work --stdin`) or for users who prefer to
    /// generate the token separately and paste it.
    #[arg(long)]
    pub stdin: bool,
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
        ProfileCmd::Pin(args) => pin(args),
        ProfileCmd::Unpin => unpin(),
        ProfileCmd::Clone(args) => clone(ctx, args),
        ProfileCmd::Export(args) => export(ctx, args),
        ProfileCmd::Import(args) => import(ctx, args),
        ProfileCmd::ImportExisting(args) => import_existing(ctx, args),
        ProfileCmd::Login(args) => login(ctx, args),
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

// ───── pin / unpin ──────────────────────────────────────────────────────

const PIN_FILE: &str = ".maru";

fn pin(args: PinArgs) -> Result<()> {
    let name = ProfileName::new(args.name)
        .map_err(|e| CliError::user(format!("invalid profile name: {e}")))?;
    let cwd = std::env::current_dir().context("get cwd")?;
    let pin_path = cwd.join(PIN_FILE);

    if pin_path.exists() && !args.force {
        let existing = std::fs::read_to_string(&pin_path).unwrap_or_default();
        let existing_first_line = existing.lines().next().unwrap_or("").trim();
        if existing_first_line != name.as_str() {
            bail!(CliError::user(format!(
                "{} already pins {:?}; pass --force to overwrite",
                pin_path.display(),
                existing_first_line
            )));
        }
        // Idempotent rewrite: same name, no-op except touch.
    }

    std::fs::write(&pin_path, format!("{}\n", name.as_str()))
        .with_context(|| format!("write {}", pin_path.display()))?;
    eprintln!(
        "maru: pinned {:?} via {}",
        name.as_str(),
        pin_path.display()
    );
    Ok(())
}

fn unpin() -> Result<()> {
    let cwd = std::env::current_dir().context("get cwd")?;
    let pin_path = cwd.join(PIN_FILE);
    if !pin_path.exists() {
        eprintln!("maru: no .maru in {} (nothing to do)", cwd.display());
        return Ok(());
    }
    std::fs::remove_file(&pin_path).with_context(|| format!("remove {}", pin_path.display()))?;
    eprintln!("maru: removed {}", pin_path.display());
    Ok(())
}

// ───── clone ───────────────────────────────────────────────────────────

fn clone(ctx: &CliContext, args: CloneArgs) -> Result<()> {
    let from =
        ProfileName::new(args.from).map_err(|e| CliError::user(format!("invalid `from`: {e}")))?;
    let to = ProfileName::new(args.to).map_err(|e| CliError::user(format!("invalid `to`: {e}")))?;

    let state = read_state(ctx.maru_home()).context("read state.toml")?;
    let from_entry = state
        .profiles
        .get(from.as_str())
        .ok_or_else(|| CliError::user(format!("no such profile {:?}", from.as_str())))?;
    if state.profiles.contains_key(to.as_str()) {
        bail!(CliError::user(format!(
            "destination profile {:?} already exists",
            to.as_str()
        )));
    }

    let from_root = ctx.profile_root(from.as_str());
    let to_root = ctx.profile_root(to.as_str());

    let mut total_copied = 0_usize;
    let mut total_excluded = 0_usize;
    for harness in &from_entry.harnesses {
        let from_dir = from_root.join(profile_subdir(*harness));
        let to_dir = to_root.join(profile_subdir(*harness));
        let report = copy_filtered(&from_dir, &to_dir, *harness).context("copy profile dir")?;
        total_copied = total_copied.saturating_add(report.copied);
        total_excluded = total_excluded.saturating_add(report.excluded);
    }

    insert_profile(
        ctx.maru_home(),
        &to,
        ProfileEntry::new(from_entry.harnesses.clone()),
    )
    .context("register new profile")?;

    eprintln!(
        "maru: cloned {:?} -> {:?} ({} files, {} excluded credentials)",
        from.as_str(),
        to.as_str(),
        total_copied,
        total_excluded,
    );
    Ok(())
}

// ───── export / import ──────────────────────────────────────────────────

fn export(ctx: &CliContext, args: ExportArgs) -> Result<()> {
    use std::io::Write;

    let name = ProfileName::new(args.name)
        .map_err(|e| CliError::user(format!("invalid profile name: {e}")))?;

    let state = read_state(ctx.maru_home()).context("read state.toml")?;
    let entry = state
        .profiles
        .get(name.as_str())
        .ok_or_else(|| CliError::user(format!("no such profile {:?}", name.as_str())))?;
    let from_root = ctx.profile_root(name.as_str());

    // Stage filtered copy in a tempdir, then tar it up.
    let staging = tempfile::tempdir().context("create staging tempdir")?;
    let staging_root = staging.path().join(name.as_str());
    let mut total_excluded = 0_usize;
    for harness in &entry.harnesses {
        let from_dir = from_root.join(profile_subdir(*harness));
        let to_dir = staging_root.join(profile_subdir(*harness));
        let report =
            copy_filtered(&from_dir, &to_dir, *harness).context("filtered copy for export")?;
        total_excluded = total_excluded.saturating_add(report.excluded);
    }

    // Write a manifest so import can reconstruct the profile.
    let manifest = ExportManifest {
        version: 1,
        name: name.as_str().to_owned(),
        harnesses: entry
            .harnesses
            .iter()
            .map(|h| h.as_str().to_owned())
            .collect(),
    };
    let manifest_text = serde_json::to_string_pretty(&manifest)?;
    std::fs::write(staging_root.join(".maru-manifest.json"), manifest_text)
        .context("write manifest")?;

    // Tar.gz the staging dir.
    if let Some(parent) = args.to.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let f =
        std::fs::File::create(&args.to).with_context(|| format!("create {}", args.to.display()))?;
    let gz = flate2::write::GzEncoder::new(f, flate2::Compression::default());
    let mut tar_builder = tar::Builder::new(gz);
    tar_builder
        .append_dir_all(name.as_str(), &staging_root)
        .context("append profile to tarball")?;
    let gz = tar_builder.into_inner().context("finalize tar")?;
    let mut f = gz.finish().context("flush gzip")?;
    f.flush().context("flush archive")?;

    eprintln!(
        "maru: exported {:?} to {} ({} excluded credentials)",
        name.as_str(),
        args.to.display(),
        total_excluded,
    );
    Ok(())
}

fn import(ctx: &CliContext, args: ImportArgs) -> Result<()> {
    let archive_file =
        std::fs::File::open(&args.from).with_context(|| format!("open {}", args.from.display()))?;
    let gz = flate2::read::GzDecoder::new(archive_file);
    let mut tar = tar::Archive::new(gz);

    let staging = tempfile::tempdir().context("create staging tempdir")?;
    tar.unpack(staging.path()).context("unpack archive")?;

    // Find the single top-level directory (the profile name).
    let entries: Vec<_> = std::fs::read_dir(staging.path())
        .context("scan staging")?
        .filter_map(std::result::Result::ok)
        .collect();
    let profile_dir = entries
        .iter()
        .find(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(std::fs::DirEntry::path)
        .ok_or_else(|| {
            CliError::user("archive contains no top-level profile directory".to_owned())
        })?;

    let manifest_path = profile_dir.join(".maru-manifest.json");
    let manifest_text = std::fs::read_to_string(&manifest_path).with_context(|| {
        format!(
            "read manifest at {} (was the archive produced by `maru profile export`?)",
            manifest_path.display()
        )
    })?;
    let manifest: ExportManifest =
        serde_json::from_str(&manifest_text).context("parse manifest")?;
    if manifest.version != 1 {
        bail!(CliError::user(format!(
            "unsupported manifest version: {}",
            manifest.version
        )));
    }

    let target_name = args.name.unwrap_or(manifest.name);
    let target = ProfileName::new(target_name)
        .map_err(|e| CliError::user(format!("invalid target profile name: {e}")))?;

    let harnesses: Vec<HarnessId> = manifest
        .harnesses
        .iter()
        .filter_map(|s| s.parse().ok())
        .collect();
    let dst_root = ctx.profile_root(target.as_str());
    if dst_root.exists() {
        bail!(CliError::user(format!(
            "destination profile dir already exists: {}",
            dst_root.display()
        )));
    }

    // Move the staged dir into the profiles tree (minus the manifest).
    std::fs::create_dir_all(&dst_root).with_context(|| format!("create {}", dst_root.display()))?;
    for entry in std::fs::read_dir(&profile_dir).context("scan profile_dir")? {
        let entry = entry?;
        let name = entry.file_name();
        if name == ".maru-manifest.json" {
            continue;
        }
        let src = entry.path();
        let dst = dst_root.join(&name);
        std::fs::rename(&src, &dst).or_else(|_| {
            // cross-fs move falls back to copy
            copy_dir_recursive(&src, &dst)
        })?;
    }

    insert_profile(ctx.maru_home(), &target, ProfileEntry::new(harnesses))
        .context("register imported profile")?;

    eprintln!(
        "maru: imported profile {:?} from {}",
        target.as_str(),
        args.from.display()
    );
    Ok(())
}

fn import_existing(ctx: &CliContext, args: ImportExistingArgs) -> Result<()> {
    let target =
        ProfileName::new(args.name).map_err(|e| CliError::user(format!("invalid name: {e}")))?;

    let env = maru_core::SystemEnvironment::new();
    let home = maru_core::Environment::home_dir(&env)
        .ok_or_else(|| CliError::env("could not resolve home dir"))?;
    let default_src = home.join(format!(".{}", args.harness.as_str()));
    let src = args.from.unwrap_or(default_src);

    if !src.exists() {
        bail!(CliError::user(format!(
            "source path {} does not exist",
            src.display()
        )));
    }

    let dst_root = ctx.profile_root(target.as_str());
    let dst_dir = dst_root.join(profile_subdir(args.harness));

    let stats =
        copy_filtered(&src, &dst_dir, args.harness).context("copy existing harness state")?;

    insert_profile(
        ctx.maru_home(),
        &target,
        ProfileEntry::new(vec![args.harness]),
    )
    .context("register imported profile")?;

    eprintln!(
        "maru: imported {} into profile {:?} ({} files, {} excluded credentials)",
        src.display(),
        target.as_str(),
        stats.copied,
        stats.excluded,
    );
    if stats.excluded > 0 {
        eprintln!("maru: excluded files (you must re-authenticate to populate them):");
        for p in stats.excluded_paths.iter().take(5) {
            eprintln!("       - {}", p.display());
        }
        if stats.excluded_paths.len() > 5 {
            eprintln!(
                "       ... and {} more",
                stats.excluded_paths.len().saturating_sub(5)
            );
        }
    }
    Ok(())
}

fn login(ctx: &CliContext, args: LoginArgs) -> Result<()> {
    if args.harness != HarnessId::Claude {
        bail!(CliError::user(
            "`maru profile login` currently supports only --harness claude. \
             Codex and Gemini stash their tokens differently; track via the \
             carve-outs in docs/limitations.md.",
        ));
    }

    let name =
        ProfileName::new(args.name).map_err(|e| CliError::user(format!("invalid name: {e}")))?;

    // Profile must already exist; this command is the second step after
    // `maru profile create`, not a profile-creation shortcut.
    let state = read_state(ctx.maru_home()).context("read state.toml")?;
    if !state.profiles.contains_key(name.as_str()) {
        bail!(CliError::user(format!(
            "no such profile {:?} — run `maru profile create {} --harness claude` first",
            name.as_str(),
            name.as_str()
        )));
    }

    let token_path = ctx
        .profile_root(name.as_str())
        .join(profile_subdir(HarnessId::Claude))
        .join("oauth_token");
    if let Some(parent) = token_path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }

    let token = if args.stdin {
        read_token_from_stdin().context("read token from stdin")?
    } else {
        run_claude_setup_token_and_capture(ctx, &name)?
    };
    if token.is_empty() {
        bail!(CliError::user("no token captured — aborting"));
    }

    write_token_file(&token_path, &token).context("write oauth_token")?;
    eprintln!(
        "maru: saved Claude OAuth token for profile {:?} → {}",
        name.as_str(),
        token_path.display()
    );
    eprintln!(
        "maru: subsequent `claude` invocations under {:?} will authenticate with this token \
         (CLAUDE_CODE_OAUTH_TOKEN), bypassing the macOS Keychain.",
        name.as_str()
    );
    Ok(())
}

/// Read a token from stdin: take the last non-empty trimmed line, so
/// `claude setup-token | maru profile login work --stdin` works (the
/// last line of `claude setup-token`'s stdout is the token).
fn read_token_from_stdin() -> Result<String> {
    use std::io::Read;
    let mut buf = String::new();
    std::io::stdin().read_to_string(&mut buf)?;
    Ok(buf
        .lines()
        .map(str::trim)
        .rfind(|l| !l.is_empty())
        .unwrap_or_default()
        .to_owned())
}

/// Run `claude setup-token` with full stdio inheritance, then prompt
/// the user to paste the token printed at the end.
///
/// Critical: bypass the maru shim. If we let PATH resolution find
/// `claude`, the shim intercepts (it owns `<MARU_HOME>/bin/claude`
/// prepended to PATH) and exports the active profile's
/// `CLAUDE_CODE_OAUTH_TOKEN` env var into the child — which means
/// `setup-token` inherits a (possibly stale or broken) token and
/// either skips the OAuth flow or hangs trying to validate it. We
/// resolve the *real* claude binary by walking PATH and skipping the
/// shim dir, then explicitly strip every `CLAUDE_*` env var we
/// control so `setup-token` runs in a clean environment.
///
/// Earlier versions piped stdout to auto-extract the token; that
/// broke the TUI (it detects the pipe and re-renders the splash
/// repeatedly) and reliably grabbed the hint line `Use this token
/// by setting: export CLAUDE_CODE_OAUTH_TOKEN=<token>` rather than
/// the actual token. The current flow is full stdio inheritance,
/// OAuth runs normally, user pastes the token afterwards.
fn run_claude_setup_token_and_capture(ctx: &CliContext, profile: &ProfileName) -> Result<String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let env = maru_core::SystemEnvironment::new();
    let shim_dir = ctx.shim_install_dir();
    let real_claude = maru_core::Environment::which_skipping(&env, "claude", &shim_dir)
        .ok_or_else(|| {
            CliError::user(
                "could not find a real `claude` binary on PATH outside the maru shim dir. \
                 Install Claude Code first (e.g. `npm i -g @anthropic-ai/claude-code` or \
                 the official installer) and re-run.",
            )
        })?;

    eprintln!(
        "maru: launching `{} setup-token` for profile {:?} (bypassing the maru shim).",
        real_claude.display(),
        profile.as_str()
    );
    eprintln!(
        "maru: complete the OAuth flow in your browser; `setup-token` will \
         print the token at the end. Copy it, then paste here when prompted."
    );
    eprintln!();

    let status = Command::new(&real_claude)
        .arg("setup-token")
        // Strip the shim's redirection env vars so setup-token runs
        // against the user's normal Claude Code config (not against a
        // per-profile dir or with a stale per-profile token).
        .env_remove("CLAUDE_CONFIG_DIR")
        .env_remove("CLAUDE_CODE_PLUGIN_CACHE_DIR")
        .env_remove("CLAUDE_CODE_OAUTH_TOKEN")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("spawn {} setup-token", real_claude.display()))?;

    if !status.success() {
        bail!(CliError::user(format!(
            "`claude setup-token` exited with status {status}"
        )));
    }

    eprintln!();
    eprint!("maru: paste the OAuth token printed above and press Enter: ");
    std::io::stderr().flush().ok();

    let mut buf = String::new();
    std::io::stdin()
        .read_line(&mut buf)
        .context("read pasted token from stdin")?;
    let token = buf.trim().to_owned();
    if token.is_empty() {
        bail!(CliError::user(
            "no token entered — aborting (re-run with --stdin to pipe a token directly)"
        ));
    }
    Ok(token)
}

/// Write `token` to `path` with mode 0600 on Unix.
///
/// `OpenOptionsExt::mode` only applies on creation, so when the file
/// already exists with looser perms (e.g. `0644`), truncating preserves
/// them. Explicitly `chmod` after the write to enforce the intent
/// regardless of pre-existing mode.
fn write_token_file(path: &std::path::Path, token: &str) -> std::io::Result<()> {
    use std::fs::OpenOptions;
    use std::io::Write;
    let mut opts = OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(path)?;
    f.write_all(token.as_bytes())?;
    f.write_all(b"\n")?;
    drop(f);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    if src.is_dir() {
        std::fs::create_dir_all(dst)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let s = entry.path();
            let d = dst.join(entry.file_name());
            copy_dir_recursive(&s, &d)?;
        }
    } else {
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(src, dst)?;
    }
    Ok(())
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct ExportManifest {
    version: u32,
    name: String,
    harnesses: Vec<String>,
}
