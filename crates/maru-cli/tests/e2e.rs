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

#[cfg(unix)]
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
#[cfg(unix)]
fn run_executes_fake_gemini_with_env() {
    let home = tempfile::tempdir().unwrap();
    let bin_dir = tempfile::tempdir().unwrap();
    let dest = bin_dir.path().join("gemini");
    std::fs::copy(fixture("fake-gemini.sh"), &dest).unwrap();
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
            "gemini",
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
            "gemini",
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
    assert!(s.contains("FAKE_GEMINI_CALLED"));
    assert!(
        s.contains(&format!(
            "GEMINI_CLI_HOME={}/profiles/work/gemini",
            home.path().display()
        )),
        "GEMINI_CLI_HOME mismatched: {s}"
    );
}

#[test]
fn adapter_list_includes_all_three_harnesses() {
    let out = maru_cmd()
        .args(["adapter", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8(out).unwrap();
    assert!(s.contains("claude"));
    assert!(s.contains("codex"));
    assert!(s.contains("gemini"));
}

#[test]
fn pin_writes_dot_maru_in_cwd() {
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

    let work = tempfile::tempdir().unwrap();
    maru_cmd()
        .args([
            "--maru-home",
            home.path().to_str().unwrap(),
            "profile",
            "pin",
            "work",
        ])
        .current_dir(work.path())
        .assert()
        .success();
    let pin = std::fs::read_to_string(work.path().join(".maru")).unwrap();
    assert_eq!(pin.trim(), "work");

    maru_cmd()
        .args([
            "--maru-home",
            home.path().to_str().unwrap(),
            "profile",
            "unpin",
        ])
        .current_dir(work.path())
        .assert()
        .success();
    assert!(!work.path().join(".maru").exists());
}

#[test]
fn clone_excludes_credentials() {
    let home = tempfile::tempdir().unwrap();
    maru_cmd()
        .args([
            "--maru-home",
            home.path().to_str().unwrap(),
            "profile",
            "create",
            "src",
            "--harness",
            "claude",
        ])
        .assert()
        .success();

    let claude_dir = home.path().join("profiles/src/claude");
    std::fs::write(claude_dir.join(".credentials.json"), "SECRET").unwrap();
    // Seed scrubbable fields per GENESIS §8 value-level scrubbing.
    std::fs::write(
        claude_dir.join("settings.json"),
        r#"{"anthropic_api_key":"sk-CLONESECRET","ui":{"theme":"dark"}}"#,
    )
    .unwrap();

    maru_cmd()
        .args([
            "--maru-home",
            home.path().to_str().unwrap(),
            "profile",
            "clone",
            "src",
            "dst",
        ])
        .assert()
        .success();

    let dst_claude = home.path().join("profiles/dst/claude");
    assert!(dst_claude.join("settings.json").exists());
    assert!(
        !dst_claude.join(".credentials.json").exists(),
        ".credentials.json must NOT be present in cloned profile"
    );
    // Value-level scrubbing: settings.json is included, but the API key
    // value is replaced; benign keys preserved.
    let dst_settings = std::fs::read_to_string(dst_claude.join("settings.json")).unwrap();
    assert!(
        !dst_settings.contains("sk-CLONESECRET"),
        "scrubbed value leaked into clone target: {dst_settings}"
    );
    assert!(
        dst_settings.contains("<scrubbed by maru>"),
        "expected scrub placeholder in cloned settings.json: {dst_settings}"
    );
    assert!(
        dst_settings.contains("dark"),
        "benign value lost in clone: {dst_settings}"
    );
}

#[test]
fn export_tarball_omits_credentials() {
    let home = tempfile::tempdir().unwrap();
    maru_cmd()
        .args([
            "--maru-home",
            home.path().to_str().unwrap(),
            "profile",
            "create",
            "src",
            "--harness",
            "claude",
        ])
        .assert()
        .success();

    let claude_dir = home.path().join("profiles/src/claude");
    std::fs::write(claude_dir.join(".credentials.json"), "SECRET").unwrap();
    // Seed both file-level and value-level secrets to exercise both
    // GENESIS §8 layers in one e2e.
    std::fs::write(
        claude_dir.join("settings.json"),
        r#"{"theme":"dark","anthropic_api_key":"sk-EXPORTSECRET"}"#,
    )
    .unwrap();

    let archive = home.path().join("src.tar.gz");
    maru_cmd()
        .args([
            "--maru-home",
            home.path().to_str().unwrap(),
            "profile",
            "export",
            "src",
            "--to",
            archive.to_str().unwrap(),
        ])
        .assert()
        .success();
    assert!(archive.exists());

    let entries = std::process::Command::new("tar")
        .arg("tzf")
        .arg(&archive)
        .output()
        .expect("tar");
    let names = String::from_utf8(entries.stdout).unwrap();
    assert!(
        !names.contains(".credentials.json"),
        "tarball should not contain credentials: {names}"
    );

    maru_cmd()
        .args([
            "--maru-home",
            home.path().to_str().unwrap(),
            "profile",
            "import",
            archive.to_str().unwrap(),
            "--name",
            "imported",
        ])
        .assert()
        .success();
    let imported = home.path().join("profiles/imported/claude");
    assert!(imported.join("settings.json").exists());
    assert!(!imported.join(".credentials.json").exists());
    let imported_settings = std::fs::read_to_string(imported.join("settings.json")).unwrap();
    assert!(
        !imported_settings.contains("sk-EXPORTSECRET"),
        "value-level scrub failed; raw secret leaked through export/import: {imported_settings}"
    );
    assert!(
        imported_settings.contains("<scrubbed by maru>"),
        "expected scrub placeholder in imported settings.json: {imported_settings}"
    );
    assert!(
        imported_settings.contains("dark"),
        "benign theme lost through export/import: {imported_settings}"
    );
}

#[test]
fn delete_succeeds_for_never_activated_profile() {
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

    // Flip ever_activated = true directly in state.toml to simulate
    // a profile that has been activated at least once.
    maru_store::state::update(home.path(), |s| {
        let entry = s.profiles.get_mut("work").expect("work profile exists");
        entry.ever_activated = true;
        Ok(())
    })
    .unwrap();

    // Without --force: must fail with a CliError::user about activation.
    let assert = maru_cmd()
        .args([
            "--maru-home",
            home.path().to_str().unwrap(),
            "profile",
            "delete",
            "work",
        ])
        .assert()
        .failure();
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).unwrap();
    assert!(
        stderr.contains("activated") || stderr.contains("--force"),
        "stderr should mention activation or --force: {stderr}"
    );

    // With --force: succeeds.
    maru_cmd()
        .args([
            "--maru-home",
            home.path().to_str().unwrap(),
            "profile",
            "delete",
            "work",
            "--force",
        ])
        .assert()
        .success();
}

#[test]
fn rename_round_trip_repoints_active_default_and_snapshots() {
    let home = tempfile::tempdir().unwrap();

    // Create profile "from".
    maru_cmd()
        .args([
            "--maru-home",
            home.path().to_str().unwrap(),
            "profile",
            "create",
            "from",
            "--harness",
            "claude",
        ])
        .assert()
        .success();

    // Set active.
    maru_cmd()
        .args([
            "--maru-home",
            home.path().to_str().unwrap(),
            "profile",
            "use",
            "from",
        ])
        .assert()
        .success();

    // Set as default.
    maru_cmd()
        .args([
            "--maru-home",
            home.path().to_str().unwrap(),
            "profile",
            "default",
            "from",
        ])
        .assert()
        .success();

    // Rename from -> to.
    maru_cmd()
        .args([
            "--maru-home",
            home.path().to_str().unwrap(),
            "profile",
            "rename",
            "from",
            "to",
        ])
        .assert()
        .success();

    // profiles/to/ exists; profiles/from/ does not.
    assert!(
        home.path().join("profiles/to").exists(),
        "profiles/to should exist after rename"
    );
    assert!(
        !home.path().join("profiles/from").exists(),
        "profiles/from should not exist after rename"
    );

    // active.txt reads "to".
    let active = std::fs::read_to_string(home.path().join("active.txt")).unwrap();
    assert_eq!(active.trim(), "to", "active.txt should now read 'to'");

    // [defaults].profile is "to".
    let state_text = std::fs::read_to_string(home.path().join("state.toml")).unwrap();
    assert!(
        state_text.contains("profile = \"to\""),
        "state.toml [defaults] should have profile = \"to\": {state_text}"
    );

    // A backup snapshot exists under backups/.
    let backups_dir = home.path().join("backups");
    assert!(backups_dir.exists(), "backups/ directory should exist");
    let mut snapshot_count = 0;
    for entry in std::fs::read_dir(&backups_dir).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() && entry.path().join("state.toml").exists() {
            snapshot_count += 1;
        }
    }
    assert!(
        snapshot_count >= 1,
        "expected at least one state.toml snapshot under backups/"
    );
}

#[test]
fn import_existing_excludes_credentials_and_registers_profile() {
    let home = tempfile::tempdir().unwrap();
    let src = tempfile::tempdir().unwrap();

    // Simulate ~/.claude with one benign + one credential file + nested benign.
    std::fs::write(src.path().join("settings.json"), r#"{"theme":"dark"}"#).unwrap();
    std::fs::write(src.path().join(".credentials.json"), "SECRET").unwrap();
    std::fs::create_dir_all(src.path().join("projects")).unwrap();
    std::fs::write(src.path().join("projects/notes.md"), "hello").unwrap();

    maru_cmd()
        .args([
            "--maru-home",
            home.path().to_str().unwrap(),
            "profile",
            "import-existing",
            "--harness",
            "claude",
            "--name",
            "imported",
            "--from",
            src.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    let dst = home.path().join("profiles/imported/claude");
    assert!(
        dst.join("settings.json").exists(),
        "settings.json should be present in imported profile"
    );
    assert!(
        dst.join("projects/notes.md").exists(),
        "nested benign file should be copied"
    );
    assert!(
        !dst.join(".credentials.json").exists(),
        ".credentials.json must be excluded by deny-list"
    );

    // state.toml registers the imported profile under claude.
    let state_text = std::fs::read_to_string(home.path().join("state.toml")).unwrap();
    assert!(
        state_text.contains("[profiles.imported]"),
        "imported profile should be registered: {state_text}"
    );
    assert!(
        state_text.contains("claude"),
        "claude harness should be recorded: {state_text}"
    );
}

#[test]
fn default_sets_fallback_profile() {
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

    maru_cmd()
        .args([
            "--maru-home",
            home.path().to_str().unwrap(),
            "profile",
            "default",
            "work",
        ])
        .assert()
        .success();

    let state_text = std::fs::read_to_string(home.path().join("state.toml")).unwrap();
    assert!(
        state_text.contains("profile = \"work\""),
        "[defaults].profile should be 'work': {state_text}"
    );
}

#[test]
fn default_rejects_invalid_profile_name() {
    let home = tempfile::tempdir().unwrap();
    let assert = maru_cmd()
        .args([
            "--maru-home",
            home.path().to_str().unwrap(),
            "profile",
            "default",
            "has space",
        ])
        .assert()
        .failure();
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).unwrap();
    assert!(
        stderr.contains("invalid"),
        "stderr should mention invalid name: {stderr}"
    );
}

fn adapter_status_stdout_contains_harness(id: &str) {
    let out = maru_cmd()
        .args(["adapter", "status", id])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8(out).unwrap();
    assert!(
        s.contains(id),
        "human stdout should mention harness id {id}: {s}"
    );
}

fn adapter_status_json_has_id(id: &str) {
    let out = maru_cmd()
        .args(["adapter", "status", id, "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8(out).unwrap();
    let json: serde_json::Value = serde_json::from_str(&s)
        .unwrap_or_else(|e| panic!("adapter status --json was not valid JSON: {e}: {s}"));
    assert_eq!(
        json.get("id").and_then(serde_json::Value::as_str),
        Some(id),
        "json id field should equal {id}: {json}"
    );
    let found = json
        .get("found_at")
        .unwrap_or_else(|| panic!("missing found_at: {json}"));
    assert!(
        found.is_null() || found.is_string(),
        "found_at must be null or string: {found}"
    );
}

#[test]
fn adapter_status_claude() {
    adapter_status_stdout_contains_harness("claude");
    adapter_status_json_has_id("claude");
}

#[test]
fn adapter_status_codex() {
    adapter_status_stdout_contains_harness("codex");
    adapter_status_json_has_id("codex");
}

#[test]
fn adapter_status_gemini() {
    adapter_status_stdout_contains_harness("gemini");
    adapter_status_json_has_id("gemini");
}

#[test]
fn adapter_status_unknown_harness_is_user_error() {
    let assert = maru_cmd()
        .args(["adapter", "status", "aider"])
        .assert()
        .failure();
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).unwrap();
    assert!(
        stderr.contains("unknown harness"),
        "stderr should mention 'unknown harness': {stderr}"
    );
    let code = assert.get_output().status.code().unwrap_or(-1);
    assert_eq!(
        code, 1,
        "CliError::user should exit with code 1, got {code}"
    );
}

#[test]
fn doctor_surfaces_phase_1_limitations() {
    let home = tempfile::tempdir().unwrap();

    // JSON form: `notes` array contains the documented carve-out strings.
    let out = maru_cmd()
        .args([
            "--maru-home",
            home.path().to_str().unwrap(),
            "doctor",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8(out).unwrap();
    let json: serde_json::Value =
        serde_json::from_str(&s).unwrap_or_else(|e| panic!("doctor --json invalid: {e}: {s}"));
    let notes = json
        .get("notes")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("missing notes array: {json}"));
    let joined = notes
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        joined.contains("extension hosts"),
        "notes should mention extension hosts: {joined}"
    );
    assert!(
        joined.contains("GUI-launched IDEs"),
        "notes should mention GUI-launched IDEs: {joined}"
    );

    // Human form: same substrings appear on stdout.
    let out = maru_cmd()
        .args(["--maru-home", home.path().to_str().unwrap(), "doctor"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8(out).unwrap();
    assert!(
        s.contains("extension hosts"),
        "human stdout should mention extension hosts: {s}"
    );
    assert!(
        s.contains("GUI-launched IDEs"),
        "human stdout should mention GUI-launched IDEs: {s}"
    );
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
