//! `maru-activation` — apply `ActivationPlan` env and exec the real
//! harness binary.
#![allow(
    unsafe_code,
    reason = "std::env::set_var is unsafe (Rust 1.79+); the shim is single-threaded by construction (GENESIS §11)"
)]
#![allow(
    clippy::print_stderr,
    reason = "diagnostics are intentional user-facing output, not log noise"
)]
#![allow(
    clippy::arithmetic_side_effects,
    reason = "argv length arithmetic is bounded by Vec::with_capacity hint"
)]
//!
//! GENESIS §11. Trivial in v1 because `ActivationPlan` is env-only:
//! 1. Walk `plan.diagnostics`. `Error` aborts; `Warn` prints; `Info`
//!    suppressed.
//! 2. Apply each `(key, value)` in `plan.env` via `std::env::set_var`
//!    (single-threaded by construction; see safety note in [`apply_env`]).
//! 3. Resolve the real binary via `which_skipping`.
//! 4. Unix: `execvp`. Windows: `CreateProcess` + wait + propagate exit.
//!
//! There is no transactional rollback — there are no filesystem
//! mutations. If a future adapter requires fs ops, reintroduce `FsOp`
//! and a transactional executor at that point.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use maru_core::{ActivationPlan, Diagnostic, Environment, Level};

/// Errors that can happen during activation.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A `Diagnostic::Error` aborted activation.
    #[error("adapter diagnostic blocked activation: {message}")]
    Blocked {
        /// The blocking diagnostic's message.
        message: String,
        /// Optional help text from the diagnostic.
        help: Option<String>,
    },

    /// `which_skipping` returned no candidate.
    #[error("real {binary} binary not found on PATH (after skipping shim install dir)")]
    BinaryNotFound {
        /// The binary name we were looking for.
        binary: String,
    },

    /// Exec / spawn failed.
    #[error("exec {binary:?}: {source}")]
    Exec {
        /// Path to the binary that failed to exec.
        binary: PathBuf,
        /// Underlying error.
        #[source]
        source: std::io::Error,
    },
}

/// Process-blocking diagnostics in order. Returns:
/// - `Ok(())` if no `Error`-level diagnostic was encountered.
/// - `Err(Error::Blocked { .. })` on the first `Error`.
///
/// `Warn` diagnostics are printed to stderr; `Info` are suppressed
/// unless `MARU_LOG=debug` is set in the environment.
///
/// # Errors
///
/// Returns [`Error::Blocked`] on any `Diagnostic::Error`.
pub fn process_diagnostics(diagnostics: &[Diagnostic]) -> Result<(), Error> {
    let debug = std::env::var_os("MARU_LOG").is_some_and(|v| v == "debug");
    for d in diagnostics {
        match d.level {
            Level::Error => {
                return Err(Error::Blocked {
                    message: d.message.clone(),
                    help: d.help.clone(),
                });
            }
            Level::Warn => {
                eprintln!("maru: warning: {}", d.message);
                if let Some(h) = &d.help {
                    eprintln!("       {h}");
                }
            }
            Level::Info => {
                if debug {
                    eprintln!("maru: info: {}", d.message);
                }
            }
        }
    }
    Ok(())
}

/// Apply env-var bindings to the current process.
///
/// # Safety
///
/// `std::env::set_var` is `unsafe` as of Rust 1.79 because environment
/// mutation is not thread-safe. This function takes a single `unsafe`
/// block over the entire loop. **Callers must ensure no other threads
/// are reading or writing process env concurrently.** The shim is
/// single-threaded by construction (no `tokio`, no `rayon`, no manually
/// spawned threads — see GENESIS §13 forbidden list); env application
/// happens before any potential threading source could be introduced.
///
/// In test contexts (where the test harness may spawn threads), use
/// [`compute_env`] to obtain the bindings without mutating process env.
pub fn apply_env(plan: &ActivationPlan) {
    // SAFETY: Single-threaded by construction. See GENESIS §11 / §13.
    unsafe {
        for (k, v) in &plan.env {
            std::env::set_var(k, v);
        }
    }
}

/// Pure helper: return the env bindings the plan would apply, without
/// mutating the process. Useful in tests and in the
/// `maru run --dry-run` path.
#[must_use]
pub fn compute_env(plan: &ActivationPlan) -> Vec<(OsString, OsString)> {
    plan.env.clone()
}

/// Resolve the absolute path to the real harness binary.
///
/// Wraps [`Environment::which_skipping`] with our error type.
///
/// # Errors
///
/// Returns [`Error::BinaryNotFound`] if no candidate is on PATH.
pub fn resolve_real_binary(
    env: &dyn Environment,
    binary: &str,
    skip: &Path,
) -> Result<PathBuf, Error> {
    env.which_skipping(binary, skip)
        .ok_or_else(|| Error::BinaryNotFound {
            binary: binary.to_owned(),
        })
}

/// Compute the argv to pass to the real binary: arg0 + `plan.args_prefix`
/// + caller's argv tail.
#[must_use]
pub fn build_argv(arg0: &OsStr, plan: &ActivationPlan, tail: &[OsString]) -> Vec<OsString> {
    let mut out = Vec::with_capacity(1 + plan.args_prefix.len() + tail.len());
    out.push(arg0.to_owned());
    out.extend(plan.args_prefix.iter().cloned());
    out.extend(tail.iter().cloned());
    out
}

/// Replace the current process with `real` (Unix) or spawn-and-wait
/// (Windows).
///
/// On success on Unix this never returns. On Windows it returns the
/// child's exit code; on failure it returns an `Error::Exec`.
///
/// # Errors
///
/// Returns [`Error::Exec`] if the underlying syscall / spawn fails.
#[cfg(unix)]
pub fn exec_or_spawn(real: &Path, argv: &[OsString]) -> Result<std::convert::Infallible, Error> {
    use std::os::unix::process::CommandExt;
    let mut cmd = std::process::Command::new(real);
    if let Some((first, rest)) = argv.split_first() {
        cmd.arg0(first);
        cmd.args(rest);
    }
    // exec() returns std::io::Error only on failure (success replaces
    // the process image and never returns).
    let err = cmd.exec();
    Err(Error::Exec {
        binary: real.to_path_buf(),
        source: err,
    })
}

/// Windows: spawn-and-wait. Returns the child's exit code.
///
/// # Errors
///
/// Returns [`Error::Exec`] on spawn or wait failure.
#[cfg(windows)]
pub fn exec_or_spawn(real: &Path, argv: &[OsString]) -> Result<i32, Error> {
    let (_arg0, rest) = argv.split_first().ok_or_else(|| Error::Exec {
        binary: real.to_path_buf(),
        source: std::io::Error::other("argv must contain at least arg0"),
    })?;
    let mut child = std::process::Command::new(real)
        .args(rest)
        .spawn()
        .map_err(|source| Error::Exec {
            binary: real.to_path_buf(),
            source,
        })?;
    let status = child.wait().map_err(|source| Error::Exec {
        binary: real.to_path_buf(),
        source,
    })?;
    Ok(status.code().unwrap_or(1))
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "tests"
)]
mod tests {
    use super::{Error, build_argv, compute_env, process_diagnostics, resolve_real_binary};
    use maru_core::{ActivationPlan, Diagnostic, FakeEnvironment};
    use std::ffi::OsString;
    use std::path::Path;

    #[test]
    fn diagnostics_pass_when_clean() {
        process_diagnostics(&[]).unwrap();
        process_diagnostics(&[Diagnostic::info("hi"), Diagnostic::warn("uh")]).unwrap();
    }

    #[test]
    fn diagnostics_block_on_error() {
        let err = process_diagnostics(&[
            Diagnostic::warn("first"),
            Diagnostic::error("nope").with_help("do this"),
            Diagnostic::info("never reached"),
        ])
        .unwrap_err();
        match err {
            Error::Blocked { message, help } => {
                assert_eq!(message, "nope");
                assert_eq!(help.as_deref(), Some("do this"));
            }
            other => panic!("expected Blocked, got {other:?}"),
        }
    }

    #[test]
    fn compute_env_clones_plan_env() {
        let plan = ActivationPlan::new()
            .with_env("FOO", "bar")
            .with_env("BAZ", "qux");
        let env = compute_env(&plan);
        assert_eq!(env.len(), 2);
        assert_eq!(env[0].0, "FOO");
    }

    #[test]
    fn build_argv_inserts_prefix() {
        let mut plan = ActivationPlan::new();
        plan.args_prefix.push(OsString::from("--inserted"));
        let tail = vec![OsString::from("user-arg")];
        let argv = build_argv(OsString::from("claude").as_os_str(), &plan, &tail);
        assert_eq!(argv.len(), 3);
        assert_eq!(argv.first().expect("0"), "claude");
        assert_eq!(argv.get(1).expect("1"), "--inserted");
        assert_eq!(argv.get(2).expect("2"), "user-arg");
    }

    #[test]
    fn resolve_real_binary_finds_via_env() {
        let env = FakeEnvironment::new().with_path_entry("/usr/local/bin", ["claude"]);
        let p = resolve_real_binary(&env, "claude", Path::new("/our/install")).unwrap();
        assert_eq!(p, std::path::Path::new("/usr/local/bin/claude"));
    }

    #[test]
    fn resolve_real_binary_returns_not_found() {
        let env = FakeEnvironment::new();
        let err = resolve_real_binary(&env, "claude", Path::new("/our/install")).unwrap_err();
        assert!(matches!(err, Error::BinaryNotFound { .. }));
    }
}
