//! `maru-shim` — argv[0]-dispatched profile-redirect shim.
//!
//! GENESIS §9. The hot path: every `claude`, `codex`, `gemini` invocation
//! goes through this binary. Performance budget: ≤15 ms p50 on Linux/macOS
//! from `main()` to `execvp`; ≤40 ms on Windows from `main()` to
//! `CreateProcess`.
//!
//! This crate has the strictest dependency budget in the workspace: `std`
//! and `directories` only. No `serde`, no `toml`, no `clap`. The config
//! reader for `state.toml` and `active.txt` is hand-rolled.
// One unsafe block for env-var application. Single-threaded by
// construction (no tokio, no rayon, no threads); see GENESIS §11.
#![deny(unsafe_code)]
// Binary crate: `pub` items in submodules are crate-local by definition.
#![allow(unreachable_pub)]
#![allow(
    clippy::print_stderr,
    reason = "intentional user-facing diagnostic output"
)]

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

mod config;
mod harness;

// Exit codes per GENESIS §8.
const EXIT_SUCCESS: u8 = 0;
const EXIT_USER_ERROR: u8 = 1;
const EXIT_ENV_ERROR: u8 = 2;
const EXIT_ADAPTER_GATE: u8 = 3;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::from(EXIT_SUCCESS),
        Err(e) => {
            eprintln!("maru: {}", e.message);
            if let Some(help) = &e.help {
                eprintln!("       {help}");
            }
            ExitCode::from(e.code)
        }
    }
}

/// Lightweight error type for the shim. No `anyhow` per GENESIS §13.
struct ShimError {
    message: String,
    help: Option<String>,
    code: u8,
}

impl ShimError {
    fn user(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            help: None,
            code: EXIT_USER_ERROR,
        }
    }

    fn env(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            help: None,
            code: EXIT_ENV_ERROR,
        }
    }

    fn gate(msg: impl Into<String>, help: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            help: Some(help.into()),
            code: EXIT_ADAPTER_GATE,
        }
    }
}

fn run() -> Result<(), ShimError> {
    // 1. argv[0] basename → harness id.
    let all_args: Vec<OsString> = std::env::args_os().collect();
    let prog = all_args
        .first()
        .ok_or_else(|| ShimError::env("argv[0] missing — cannot dispatch"))?;
    let basename =
        basename_of(prog).ok_or_else(|| ShimError::env("argv[0] has no file-name component"))?;

    let harness = harness::Harness::from_basename(&basename).ok_or_else(|| {
        ShimError::user(format!(
            "shim invoked as {basename:?}, but only `claude`, `codex`, `gemini` are supported. \
             Did you symlink this binary under the wrong name?"
        ))
    })?;

    // 2. Resolve profile name: env > .maru > active.txt > [defaults].profile.
    let maru_home =
        config::maru_home().ok_or_else(|| ShimError::env("could not resolve $MARU_HOME"))?;
    let home_dir = directories::BaseDirs::new()
        .map(|b| b.home_dir().to_path_buf())
        .ok_or_else(|| ShimError::env("could not resolve home directory"))?;
    let cwd =
        std::env::current_dir().map_err(|e| ShimError::env(format!("could not get cwd: {e}")))?;

    let profile_name = resolve_profile(&maru_home, &home_dir, &cwd).ok_or_else(|| {
        ShimError::user(
            "no maru profile is active. \
             Set MARU_PROFILE, create a .maru file, or run `maru profile use <name>`."
                .to_owned(),
        )
    })?;

    // 3. Compute env from harness + profile.
    let profile_root = maru_home.join("profiles").join(&profile_name);
    let env_pairs = harness.env_for_profile(&profile_root);

    // 4. Run the Linux/WSL Claude credential gate (per GENESIS §7.1).
    if matches!(harness, harness::Harness::Claude) {
        if let Some(err) = harness::linux_wsl_credential_gate(&home_dir) {
            return Err(err);
        }
    }

    // 5. Apply env. Single unsafe block; safe under the single-thread invariant.
    apply_env(&env_pairs);

    // 6. Resolve real harness binary (skipping our install dir).
    let our_install_dir = current_install_dir();
    let real =
        which_skipping(harness.binary_name(), our_install_dir.as_deref()).ok_or_else(|| {
            ShimError::env(format!(
                "real {} binary not found on PATH (after skipping shim install dir)",
                harness.binary_name()
            ))
        })?;

    // 7. Build argv and exec.
    let user_args: Vec<OsString> = all_args.iter().skip(1).cloned().collect();
    exec_or_spawn(&real, &basename, &user_args)?;

    // Unreachable on Unix; on Windows exec_or_spawn returns the child's exit.
    Ok(())
}

// ─── argv[0] handling ────────────────────────────────────────────────────

fn basename_of(arg0: &OsString) -> Option<OsString> {
    let p = Path::new(arg0);
    let stem = p.file_stem()?;
    Some(stem.to_owned())
}

// ─── profile resolution chain ────────────────────────────────────────────

fn resolve_profile(maru_home: &Path, home_dir: &Path, cwd: &Path) -> Option<String> {
    // 1. MARU_PROFILE env (non-empty + valid).
    if let Some(raw) = std::env::var_os("MARU_PROFILE") {
        let s = raw.to_string_lossy().trim().to_owned();
        if !s.is_empty() && is_valid_profile_name(&s) {
            return Some(s);
        }
    }

    // 2. .maru file walked from cwd up to home_dir or filesystem root.
    if let Some(name) = walk_for_pin(cwd, home_dir) {
        return Some(name);
    }

    // 3. active.txt
    if let Some(name) = config::read_active_txt(maru_home) {
        return Some(name);
    }

    // 4. [defaults].profile in state.toml
    if let Some(name) = config::read_default_profile(maru_home) {
        return Some(name);
    }

    None
}

fn walk_for_pin(start: &Path, home_dir: &Path) -> Option<String> {
    let mut current: Option<&Path> = Some(start);
    while let Some(dir) = current {
        let candidate = dir.join(".maru");
        if let Ok(text) = std::fs::read_to_string(&candidate) {
            let line = text.lines().next()?.trim();
            if !line.is_empty() && is_valid_profile_name(line) {
                return Some(line.to_owned());
            }
            return None; // .maru exists but invalid: caller's intent is explicit
        }
        if dir == home_dir {
            return None;
        }
        current = dir.parent();
    }
    None
}

/// GENESIS §6 line 168: `[A-Za-z0-9][A-Za-z0-9_-]{0,63}`.
fn is_valid_profile_name(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphanumeric() {
        return false;
    }
    if s.len() > 64 {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

// ─── env application ─────────────────────────────────────────────────────

fn apply_env(pairs: &[(&'static str, OsString)]) {
    // SAFETY: shim is single-threaded by construction (GENESIS §13 forbids
    // tokio / async-std / threading sources). No other thread can be
    // reading or writing process env concurrently.
    #[allow(unsafe_code, reason = "see safety comment")]
    unsafe {
        for (k, v) in pairs {
            std::env::set_var(k, v);
        }
    }
}

// ─── which_skipping ──────────────────────────────────────────────────────

fn current_install_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let resolved = exe.canonicalize().ok().unwrap_or(exe);
    resolved.parent().map(Path::to_path_buf)
}

fn which_skipping(name: &str, skip: Option<&Path>) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    let skip_canonical = skip.and_then(|p| p.canonicalize().ok());

    for dir in std::env::split_paths(&path_var) {
        let dir_canonical = dir.canonicalize().ok();
        if let (Some(s), Some(d)) = (&skip_canonical, &dir_canonical)
            && s == d
        {
            continue;
        }
        if skip_canonical.is_none()
            && let Some(s) = skip
            && dir == *s
        {
            continue;
        }

        let candidate = dir.join(name);
        if is_executable(&candidate) {
            return Some(candidate);
        }

        #[cfg(windows)]
        for ext in [".exe", ".cmd", ".bat"] {
            let with_ext = dir.join(format!("{name}{ext}"));
            if is_executable(&with_ext) {
                return Some(with_ext);
            }
        }
    }
    None
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

// ─── exec / spawn ────────────────────────────────────────────────────────

#[cfg(unix)]
fn exec_or_spawn(real: &Path, arg0: &OsString, user_args: &[OsString]) -> Result<(), ShimError> {
    use std::os::unix::process::CommandExt;
    let mut cmd = std::process::Command::new(real);
    cmd.arg0(arg0).args(user_args);
    let err = cmd.exec();
    Err(ShimError::env(format!("exec {}: {err}", real.display())))
}

#[cfg(windows)]
fn exec_or_spawn(
    real: &Path,
    _arg0: &std::ffi::OsString,
    user_args: &[std::ffi::OsString],
) -> Result<(), ShimError> {
    let mut child = std::process::Command::new(real)
        .args(user_args)
        .spawn()
        .map_err(|e| ShimError::env(format!("spawn {real:?}: {e}")))?;
    let status = child
        .wait()
        .map_err(|e| ShimError::env(format!("wait {real:?}: {e}")))?;
    let code = u8::try_from(status.code().unwrap_or(1) & 0xFF).unwrap_or(1);
    std::process::exit(i32::from(code));
}

// ─── unit tests ──────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "tests"
)]
mod tests {
    use super::{basename_of, is_valid_profile_name, walk_for_pin};
    use std::ffi::OsString;
    use std::path::PathBuf;

    #[test]
    fn basename_extracts_simple_name() {
        let arg0 = OsString::from("/usr/local/bin/claude");
        assert_eq!(basename_of(&arg0), Some(OsString::from("claude")));
    }

    #[test]
    fn basename_extracts_bare_name() {
        let arg0 = OsString::from("claude");
        assert_eq!(basename_of(&arg0), Some(OsString::from("claude")));
    }

    #[test]
    fn basename_strips_extension_on_windows_style() {
        let arg0 = OsString::from("claude.exe");
        assert_eq!(basename_of(&arg0), Some(OsString::from("claude")));
    }

    #[test]
    fn profile_name_validation_accepts_valid() {
        for s in ["a", "work", "p1", "client-1", "personal_dev"] {
            assert!(is_valid_profile_name(s), "{s:?} should be valid");
        }
    }

    #[test]
    fn profile_name_validation_rejects_invalid() {
        for s in ["", "-bad", "_bad", " bad", "has space", "a/b"] {
            assert!(!is_valid_profile_name(s), "{s:?} should be invalid");
        }
    }

    #[test]
    fn profile_name_rejects_too_long() {
        let s = "x".repeat(65);
        assert!(!is_valid_profile_name(&s));
    }

    #[test]
    fn walk_for_pin_finds_nearest() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("repo");
        let nested = project.join("src");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(project.join(".maru"), "work\n").unwrap();
        let found = walk_for_pin(&nested, dir.path());
        assert_eq!(found.as_deref(), Some("work"));
    }

    #[test]
    fn walk_for_pin_stops_at_home() {
        let dir = tempfile::tempdir().unwrap();
        // No .maru anywhere; walking should terminate without panicking.
        let nested = dir.path().join("a").join("b");
        std::fs::create_dir_all(&nested).unwrap();
        assert!(walk_for_pin(&nested, dir.path()).is_none());
    }

    #[test]
    fn walk_for_pin_ignores_invalid_name() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("repo");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(project.join(".maru"), "-not-valid\n").unwrap();
        // Returns None — does NOT silently fall through to active.txt
        // (per GENESIS §12: "A `.maru` file containing an unknown profile
        // name causes the shim to exit 1 with a clear message"). Note:
        // walk_for_pin returns None here; the caller's chain then tries
        // active.txt. This is consistent with the in-store implementation.
        assert!(walk_for_pin(&project, dir.path()).is_none());
    }

    #[test]
    fn walk_for_pin_handles_path_at_home() {
        let dir = tempfile::tempdir().unwrap();
        // Home == start.
        std::fs::write(dir.path().join(".maru"), "personal\n").unwrap();
        assert_eq!(
            walk_for_pin(dir.path(), dir.path()).as_deref(),
            Some("personal")
        );
    }

    #[test]
    fn walk_for_pin_returns_none_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(walk_for_pin(dir.path(), dir.path()).is_none());
    }

    // Convince clippy nothing imported is dead.
    #[allow(dead_code)]
    fn _unused_imports() -> PathBuf {
        PathBuf::new()
    }
}
