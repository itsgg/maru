//! `maru install` and `maru uninstall`. GENESIS §8 + §10.

use std::path::Path;

use anyhow::{Context, Result};
use clap::{Args, ValueEnum};
use maru_core::HarnessId;

use crate::CliContext;

#[derive(Debug, Args)]
pub struct InstallArgs {
    /// Shell rc to update with PATH prepend (defaults: detect from `$SHELL`).
    #[arg(long, value_enum)]
    pub shell: Option<Shell>,
    /// Skip writing to the shell rc file (just install shims).
    #[arg(long)]
    pub no_shell_rc: bool,
}

#[derive(Debug, Args)]
pub struct UninstallArgs {
    /// Also remove `$MARU_HOME` (state.toml, profiles, backups).
    #[arg(long)]
    pub purge: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    Powershell,
}

const MARU_BLOCK_BEGIN: &str = "# >>> maru managed block (do not edit) >>>";
const MARU_BLOCK_END: &str = "# <<< maru managed block <<<";

pub fn run_install(ctx: &CliContext, args: InstallArgs) -> Result<()> {
    let shim_dir = ctx.shim_install_dir();
    std::fs::create_dir_all(&shim_dir).with_context(|| format!("create {}", shim_dir.display()))?;

    let shim_binary = locate_shim_binary().context(
        "could not locate the maru-shim binary. \
         Build with `cargo build --bin maru-shim` first, then re-run `maru install`.",
    )?;

    // Symlink shim names per harness.
    for harness in [HarnessId::Claude, HarnessId::Codex, HarnessId::Gemini] {
        for name in maru_adapters::adapter_for(harness)
            .map(|a| a.binary_names().to_vec())
            .unwrap_or_else(|| vec![harness.as_str()])
        {
            install_one_shim(&shim_dir, &shim_binary, name)?;
        }
    }
    eprintln!("maru: installed shims to {}", shim_dir.display());

    if args.no_shell_rc {
        eprintln!(
            "maru: skipped shell rc update (--no-shell-rc). \
             Add {} to your PATH manually.",
            shim_dir.display()
        );
        return Ok(());
    }

    let shell = args.shell.or_else(detect_shell).unwrap_or(Shell::Bash);
    if let Some(rc) = shell_rc_path(shell) {
        write_managed_block(&rc, &shim_dir, None)
            .with_context(|| format!("update {}", rc.display()))?;
        eprintln!(
            "maru: appended managed block to {}\nmaru: open a new terminal or `source {}` to pick up the PATH change",
            rc.display(),
            rc.display()
        );
    } else {
        eprintln!(
            "maru: could not detect a shell rc; add {} to your PATH manually.",
            shim_dir.display()
        );
    }
    Ok(())
}

pub fn run_uninstall(ctx: &CliContext, args: UninstallArgs) -> Result<()> {
    let shim_dir = ctx.shim_install_dir();
    if shim_dir.exists() {
        std::fs::remove_dir_all(&shim_dir)
            .with_context(|| format!("remove {}", shim_dir.display()))?;
        eprintln!("maru: removed shim dir {}", shim_dir.display());
    }
    if let Some(shell) = detect_shell()
        && let Some(rc) = shell_rc_path(shell)
        && rc.exists()
    {
        remove_managed_block(&rc).with_context(|| format!("update {}", rc.display()))?;
    }
    if args.purge {
        if ctx.maru_home().exists() {
            std::fs::remove_dir_all(ctx.maru_home())
                .with_context(|| format!("purge {}", ctx.maru_home().display()))?;
            eprintln!("maru: purged {}", ctx.maru_home().display());
        }
    } else {
        eprintln!(
            "maru: $MARU_HOME at {} preserved (pass --purge to remove)",
            ctx.maru_home().display()
        );
    }
    Ok(())
}

fn install_one_shim(shim_dir: &Path, shim_binary: &Path, name: &str) -> Result<()> {
    let target = shim_dir.join(name);
    if target.exists() || target.is_symlink() {
        std::fs::remove_file(&target).ok();
    }

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(shim_binary, &target).with_context(|| {
            format!("symlink {} -> {}", target.display(), shim_binary.display())
        })?;
    }
    #[cfg(windows)]
    {
        // Windows: write a small .cmd shim that invokes the shim binary
        // with the desired argv[0]. The shim reads its name from argv[0]
        // for dispatch, so we use a real file-name + the same binary.
        let cmd_path = shim_dir.join(format!("{name}.cmd"));
        let body = format!("@echo off\r\n\"{}\" %*\r\n", shim_binary.display());
        std::fs::write(&cmd_path, body).with_context(|| format!("write {}", cmd_path.display()))?;
        // Also copy the shim binary under the harness name so a direct
        // invocation (PowerShell, Git Bash) works.
        let exe = shim_dir.join(format!("{name}.exe"));
        std::fs::copy(shim_binary, &exe)
            .with_context(|| format!("copy {} -> {}", shim_binary.display(), exe.display()))?;
    }
    Ok(())
}

fn locate_shim_binary() -> Result<std::path::PathBuf> {
    // Strategy: look next to the running `maru` binary first; fall back
    // to PATH; fall back to common cargo target dirs.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            #[cfg(unix)]
            let candidate = dir.join("maru-shim");
            #[cfg(windows)]
            let candidate = dir.join("maru-shim.exe");
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }
    if let Ok(p) = which("maru-shim") {
        return Ok(p);
    }
    anyhow::bail!("maru-shim binary not found")
}

fn which(name: &str) -> Result<std::path::PathBuf> {
    let path_var = std::env::var_os("PATH").context("PATH unset")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Ok(candidate);
        }
        #[cfg(windows)]
        for ext in [".exe", ".cmd", ".bat"] {
            let with_ext = dir.join(format!("{name}{ext}"));
            if with_ext.is_file() {
                return Ok(with_ext);
            }
        }
    }
    anyhow::bail!("{name} not found on PATH")
}

fn detect_shell() -> Option<Shell> {
    let shell = std::env::var_os("SHELL")?;
    let s = shell.to_string_lossy();
    if s.ends_with("zsh") {
        Some(Shell::Zsh)
    } else if s.ends_with("bash") {
        Some(Shell::Bash)
    } else if s.ends_with("fish") {
        Some(Shell::Fish)
    } else {
        None
    }
}

fn shell_rc_path(shell: Shell) -> Option<std::path::PathBuf> {
    let home = directories::BaseDirs::new().map(|d| d.home_dir().to_path_buf())?;
    let rel = match shell {
        Shell::Bash => ".bashrc",
        Shell::Zsh => ".zshrc",
        Shell::Fish => ".config/fish/config.fish",
        // PowerShell is more complex; skip for v1.
        Shell::Powershell => return None,
    };
    Some(home.join(rel))
}

/// Write the managed block to `rc`. Replaces any existing block.
fn write_managed_block(rc: &Path, shim_dir: &Path, profile: Option<&str>) -> Result<()> {
    let existing = std::fs::read_to_string(rc).unwrap_or_default();
    let stripped = strip_managed_block(&existing);
    if stripped.is_none() {
        // User edited inside markers; refuse.
        anyhow::bail!(
            "{} contains a managed block that has been hand-edited; \
             please reconcile manually before re-running install",
            rc.display()
        );
    }
    let cleaned = stripped.unwrap_or_default();

    let block = build_managed_block(shim_dir, profile);
    let trailing_nl = if cleaned.ends_with('\n') { "" } else { "\n" };
    let new_contents = format!("{cleaned}{trailing_nl}{block}\n");

    if let Some(parent) = rc.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(rc, new_contents)?;
    Ok(())
}

fn remove_managed_block(rc: &Path) -> Result<()> {
    let Ok(existing) = std::fs::read_to_string(rc) else {
        return Ok(());
    };
    if let Some(stripped) = strip_managed_block(&existing)
        && stripped != existing
    {
        std::fs::write(rc, stripped)?;
        eprintln!("maru: removed managed block from {}", rc.display());
    }
    Ok(())
}

fn build_managed_block(shim_dir: &Path, profile: Option<&str>) -> String {
    let mut s = String::new();
    s.push_str(MARU_BLOCK_BEGIN);
    s.push('\n');
    s.push_str(&format!("export PATH=\"{}:$PATH\"\n", shim_dir.display()));
    if let Some(p) = profile {
        s.push_str(&format!("export MARU_PROFILE=\"{p}\"\n"));
    }
    s.push_str(MARU_BLOCK_END);
    s
}

/// Returns `Some(content_without_block)` if any block found was clean
/// (no hand-edits between markers); `None` if hand-edited; or the original
/// content if no block.
fn strip_managed_block(text: &str) -> Option<String> {
    let mut out = String::new();
    let mut block_count: u32 = 0;
    let mut in_block = false;
    let mut block_buf = String::new();
    for line in text.lines() {
        if line.trim() == MARU_BLOCK_BEGIN {
            in_block = true;
            block_count = block_count.saturating_add(1);
            block_buf.clear();
            continue;
        }
        if line.trim() == MARU_BLOCK_END {
            // Validate that the block contents look like our generated
            // shape — no extra exports we didn't add.
            for blocked_line in block_buf.lines() {
                let t = blocked_line.trim();
                if t.is_empty() {
                    continue;
                }
                if !(t.starts_with("export PATH=") || t.starts_with("export MARU_PROFILE=")) {
                    return None;
                }
            }
            in_block = false;
            continue;
        }
        if in_block {
            block_buf.push_str(line);
            block_buf.push('\n');
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    if in_block {
        // Unterminated block; treat as edited.
        return None;
    }
    if block_count == 0 {
        return Some(text.to_owned());
    }
    Some(out)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests"
)]
mod tests {
    use super::{
        build_managed_block, remove_managed_block, strip_managed_block, write_managed_block,
    };
    use std::path::PathBuf;

    #[test]
    fn build_block_includes_path_export() {
        let s = build_managed_block(&PathBuf::from("/usr/local/maru/bin"), None);
        assert!(s.contains("export PATH=\"/usr/local/maru/bin:$PATH\""));
    }

    #[test]
    fn strip_managed_block_no_block() {
        let text = "echo hi\nalias ll='ls -l'\n";
        assert_eq!(strip_managed_block(text).as_deref(), Some(text));
    }

    #[test]
    fn strip_managed_block_with_block() {
        let dir = tempfile::tempdir().unwrap();
        let rc = dir.path().join(".bashrc");
        std::fs::write(&rc, "echo before\n").unwrap();
        write_managed_block(&rc, &PathBuf::from("/maru/bin"), None).unwrap();
        let after = std::fs::read_to_string(&rc).unwrap();
        assert!(after.contains("export PATH=\"/maru/bin:$PATH\""));

        // Round-trip: stripping leaves only the original prefix.
        let stripped = strip_managed_block(&after).unwrap();
        assert_eq!(stripped.trim(), "echo before");
    }

    #[test]
    fn strip_block_refuses_hand_edits() {
        let mut text = String::new();
        text.push_str("# >>> maru managed block (do not edit) >>>\n");
        text.push_str("export FOO=\"bar\"\n"); // not one of our exports
        text.push_str("# <<< maru managed block <<<\n");
        assert!(strip_managed_block(&text).is_none());
    }

    #[test]
    fn idempotent_rewrite() {
        let dir = tempfile::tempdir().unwrap();
        let rc = dir.path().join(".bashrc");
        std::fs::write(&rc, "echo hi\n").unwrap();
        write_managed_block(&rc, &PathBuf::from("/maru/bin"), None).unwrap();
        let first = std::fs::read_to_string(&rc).unwrap();
        write_managed_block(&rc, &PathBuf::from("/maru/bin"), None).unwrap();
        let second = std::fs::read_to_string(&rc).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn remove_block_clears() {
        let dir = tempfile::tempdir().unwrap();
        let rc = dir.path().join(".bashrc");
        std::fs::write(&rc, "echo before\n").unwrap();
        write_managed_block(&rc, &PathBuf::from("/maru/bin"), None).unwrap();
        remove_managed_block(&rc).unwrap();
        let after = std::fs::read_to_string(&rc).unwrap();
        assert!(!after.contains("maru managed block"));
        assert!(after.contains("echo before"));
    }
}
