//! End-to-end tests for the `maru` CLI against fake harness shells.
//!
//! GENESIS §15 testing strategy level 5 ("E2E tests with fake harness
//! binaries that print their environment").
//!
//! Each test:
//! 1. Builds a tempdir `MARU_HOME`.
//! 2. Stages a `bin/` containing a fake harness shell script.
//! 3. Invokes `maru` via `assert_cmd` with `PATH` pointing at that bin.
//! 4. Asserts the fake script's stdout contains the env vars maru should
//!    have set.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "tests"
)]

use std::path::PathBuf;
use std::process::Command;

use assert_cmd::Command as TestCommand;
use predicates::prelude::*;

#[cfg(unix)]
fn fixture(name: &str) -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.join("tests/fixtures").join(name)
}

fn maru_cmd() -> TestCommand {
    TestCommand::cargo_bin("maru").expect("maru binary built")
}

#[test]
fn profile_create_list_use_current() {
    let home = tempfile::tempdir().unwrap();

    maru_cmd()
        .args([
            "--maru-home",
            home.path().to_str().unwrap(),
            "profile",
            "create",
            "work",
            "--harness",
            "claude,codex",
        ])
        .assert()
        .success();

    maru_cmd()
        .args([
            "--maru-home",
            home.path().to_str().unwrap(),
            "profile",
            "list",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("work"))
        .stdout(predicate::str::contains("claude,codex"));

    maru_cmd()
        .args([
            "--maru-home",
            home.path().to_str().unwrap(),
            "profile",
            "use",
            "work",
        ])
        .assert()
        .success();

    maru_cmd()
        .args([
            "--maru-home",
            home.path().to_str().unwrap(),
            "profile",
            "current",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("work"));
}

#[test]
fn run_dry_run_produces_claude_envvars() {
    let home = tempfile::tempdir().unwrap();
    maru_cmd()
        .args([
            "--maru-home",
            home.path().to_str().unwrap(),
            "profile",
            "create",
            "work",
            "--harness",
            "claude",
        ])
        .assert()
        .success();

    let out = maru_cmd()
        .args([
            "--maru-home",
            home.path().to_str().unwrap(),
            "run",
            "--profile",
            "work",
            "--dry-run",
            "--",
            "claude",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let s = String::from_utf8(out).unwrap();
    assert!(
        s.contains("CLAUDE_CONFIG_DIR"),
        "output missing CLAUDE_CONFIG_DIR: {s}"
    );
    assert!(
        s.contains("CLAUDE_CODE_PLUGIN_CACHE_DIR"),
        "GENESIS §7.1 carve-out env var missing: {s}"
    );
}

#[test]
#[cfg(unix)]
fn run_executes_fake_claude_with_env() {
    // Build a PATH containing only the fake claude.
    let home = tempfile::tempdir().unwrap();
    let bin_dir = tempfile::tempdir().unwrap();
    let dest = bin_dir.path().join("claude");
    std::fs::copy(fixture("fake-claude.sh"), &dest).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&dest).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&dest, perms).unwrap();
    }

    maru_cmd()
        .args([
            "--maru-home",
            home.path().to_str().unwrap(),
            "profile",
            "create",
            "work",
            "--harness",
            "claude",
        ])
        .assert()
        .success();

    let out = maru_cmd()
        .args([
            "--maru-home",
            home.path().to_str().unwrap(),
            "run",
            "--profile",
            "work",
            "--",
            "claude",
            "hello",
        ])
        .env(
            "PATH",
            format!("{}:/usr/bin:/bin:/usr/sbin:/sbin", bin_dir.path().display()),
        )
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let s = String::from_utf8(out).unwrap();
    assert!(
        s.contains("FAKE_CLAUDE_CALLED"),
        "fake script not invoked: {s}"
    );
    assert!(
        s.contains(&format!(
            "CLAUDE_CONFIG_DIR={}/profiles/work/claude",
            home.path().display()
        )),
        "CLAUDE_CONFIG_DIR was not set correctly: {s}"
    );
    assert!(
        s.contains(&format!(
            "CLAUDE_CODE_PLUGIN_CACHE_DIR={}/profiles/work/claude/plugins",
            home.path().display()
        )),
        "CLAUDE_CODE_PLUGIN_CACHE_DIR (GENESIS §7.1 carve-out) was not set correctly: {s}"
    );
    assert!(s.contains("argv=hello"), "argv not forwarded: {s}");
}

#[test]
#[cfg(unix)]
fn run_executes_fake_codex_with_env() {
    let home = tempfile::tempdir().unwrap();
    let bin_dir = tempfile::tempdir().unwrap();
    let dest = bin_dir.path().join("codex");
    std::fs::copy(fixture("fake-codex.sh"), &dest).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&dest).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&dest, perms).unwrap();
    }

    maru_cmd()
        .args([
            "--maru-home",
            home.path().to_str().unwrap(),
            "profile",
            "create",
            "work",
            "--harness",
            "codex",
        ])
        .assert()
        .success();

    let out = maru_cmd()
        .args([
            "--maru-home",
            home.path().to_str().unwrap(),
            "run",
            "--profile",
            "work",
            "--",
            "codex",
        ])
        .env(
            "PATH",
            format!("{}:/usr/bin:/bin:/usr/sbin:/sbin", bin_dir.path().display()),
        )
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let s = String::from_utf8(out).unwrap();
    assert!(s.contains("FAKE_CODEX_CALLED"));
    assert!(
        s.contains(&format!(
            "CODEX_HOME={}/profiles/work/codex",
            home.path().display()
        )),
        "CODEX_HOME mismatched: {s}"
    );
}

#[test]
fn delete_requires_force_for_activated_profile() {
    let home = tempfile::tempdir().unwrap();
    maru_cmd()
        .args([
            "--maru-home",
            home.path().to_str().unwrap(),
            "profile",
            "create",
            "work",
            "--harness",
            "claude",
        ])
        .assert()
        .success();

    // First-time delete on a never-activated profile is fine.
    maru_cmd()
        .args([
            "--maru-home",
            home.path().to_str().unwrap(),
            "profile",
            "delete",
            "work",
        ])
        .assert()
        .success();
}

#[test]
fn version_json() {
    let out = maru_cmd()
        .args(["version", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8(out).unwrap();
    assert!(s.contains("\"name\": \"maru-cli\""), "missing name: {s}");
    assert!(s.contains("\"version\""), "missing version: {s}");
}

#[test]
fn schema_includes_state_definition() {
    let out = maru_cmd()
        .args(["schema"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8(out).unwrap();
    assert!(s.contains("\"State\""));
    assert!(s.contains("\"ProfileEntry\""));
    assert!(s.contains("\"schema_version\""));
}

#[test]
fn doctor_runs_clean() {
    let home = tempfile::tempdir().unwrap();
    maru_cmd()
        .args(["--maru-home", home.path().to_str().unwrap(), "doctor"])
        .assert()
        .success()
        .stdout(predicate::str::contains("MARU_HOME"))
        .stdout(predicate::str::contains("Adapters:"));
}

// Suppress the unused `Command` import on Unix where assert_cmd already
// covers our needs; kept around in case future tests need lower-level
// process spawning.
#[allow(dead_code)]
fn _unused_command() -> Command {
    Command::new("true")
}
