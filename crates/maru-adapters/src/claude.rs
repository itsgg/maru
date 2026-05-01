//! Claude Code adapter.
//!
//! GENESIS §7.1. Mechanism: `CLAUDE_CONFIG_DIR` env var redirection,
//! plus `CLAUDE_CODE_PLUGIN_CACHE_DIR` emitted unconditionally to address
//! upstream issue #15071 (plugin marketplaces dir hardcoded).
//!
//! Linux/WSL credential gate (#47661): on Linux/WSL2 without a Keychain,
//! `claude` falls through to reading `~/.claude/.credentials.json` even
//! when `CLAUDE_CONFIG_DIR` is set, silently authenticating as the wrong
//! account. The adapter emits `Diagnostic { level: Error }` if the
//! offending file exists; the shim treats this as a fatal pre-exec block
//! per GENESIS §11.

use std::path::Path;

use maru_core::{
    ActivationPlan, AdapterError, Detection, Diagnostic, Environment, HarnessAdapter, HarnessId,
    ProfileContext, ValidationReport,
};

/// `maru-adapters::claude::ClaudeAdapter`.
#[derive(Debug, Default)]
pub struct ClaudeAdapter;

impl ClaudeAdapter {
    /// New adapter instance. Stateless.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl HarnessAdapter for ClaudeAdapter {
    fn id(&self) -> HarnessId {
        HarnessId::Claude
    }

    fn binary_names(&self) -> &'static [&'static str] {
        &["claude"]
    }

    fn profile_subdir(&self) -> &'static Path {
        Path::new("claude")
    }

    fn detect(&self, env: &dyn Environment) -> Detection {
        env.which_skipping("claude", Path::new("/.this-path-never-exists"))
            .map_or(Detection::NotFound, Detection::Found)
    }

    fn plan(
        &self,
        ctx: &ProfileContext<'_>,
        env: &dyn Environment,
    ) -> Result<ActivationPlan, AdapterError> {
        if !ctx.profile_root.is_absolute() {
            return Err(AdapterError::NotAbsolute {
                path: ctx.profile_root.to_path_buf(),
            });
        }

        let mut plan = ActivationPlan::new()
            .with_env(
                "CLAUDE_CONFIG_DIR",
                ctx.profile_root.join("claude").into_os_string(),
            )
            .with_env(
                "CLAUDE_CODE_PLUGIN_CACHE_DIR",
                ctx.profile_root.join("claude/plugins").into_os_string(),
            );

        // Linux/WSL credential gate (#47661). Detect the silent-fallthrough
        // condition and emit a fatal diagnostic if it would activate.
        if let Some(diag) = linux_wsl_credential_gate(ctx, env) {
            plan = plan.with_diagnostic(diag);
        }

        Ok(plan)
    }

    fn validate(&self, profile_dir: &Path) -> ValidationReport {
        if !profile_dir.exists() {
            return ValidationReport::new();
        }
        if !profile_dir.is_dir() {
            return ValidationReport::new().with(Diagnostic::error(format!(
                "{} exists but is not a directory",
                profile_dir.display()
            )));
        }
        // Phase 1: deeper validation (e.g. .credentials.json existence
        // for an "ever_activated" profile) lands when the live-smoke
        // nightly job is online. For now, an existing dir is OK.
        ValidationReport::new()
    }
}

/// Returns a fatal diagnostic if the Claude Linux/WSL credential gate is
/// tripped (per GENESIS §7.1 "Linux/WSL2 credential isolation gate").
///
/// Conditions for tripping:
/// - target OS is Linux or WSL2
/// - `~/.claude/.credentials.json` exists
/// - secret-service / keyring is unavailable (proxied here by the absence
///   of `DBUS_SESSION_BUS_ADDRESS`; a stronger check would attempt a
///   real D-Bus connection but that's out of v1 scope)
fn linux_wsl_credential_gate(
    ctx: &ProfileContext<'_>,
    env: &dyn Environment,
) -> Option<Diagnostic> {
    if !is_linux_or_wsl() {
        return None;
    }
    let creds = ctx.home_dir.join(".claude").join(".credentials.json");
    if !creds.exists() {
        return None;
    }
    if env.var("DBUS_SESSION_BUS_ADDRESS").is_some() {
        // A keyring is plausible; assume Claude Code can use it.
        return None;
    }

    Some(
        Diagnostic::error(format!(
            "Linux/WSL credential gate: {} exists and would silently override the per-profile credentials in your active maru profile (Claude Code issue #47661).",
            creds.display()
        ))
        .with_help(format!(
            "Move it aside to enable profile isolation:\n  mv {} {}.maru-bak",
            creds.display(),
            creds.display()
        )),
    )
}

#[cfg(target_os = "linux")]
const fn is_linux_or_wsl() -> bool {
    true
}

#[cfg(not(target_os = "linux"))]
const fn is_linux_or_wsl() -> bool {
    false
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests"
)]
mod tests {
    use super::{ClaudeAdapter, linux_wsl_credential_gate};
    #[cfg(target_os = "linux")]
    use maru_core::Level;
    use maru_core::{
        AdapterError, Detection, FakeEnvironment, HarnessAdapter, HarnessId, ProfileContext,
        ProfileName,
    };
    use std::path::{Path, PathBuf};

    fn ctx<'a>(name: &'a ProfileName, root: &'a Path, home: &'a Path) -> ProfileContext<'a> {
        ProfileContext {
            profile_name: name,
            profile_root: root,
            harness: HarnessId::Claude,
            home_dir: home,
            project_pin: None,
        }
    }

    #[test]
    fn metadata() {
        let a = ClaudeAdapter;
        assert_eq!(a.id(), HarnessId::Claude);
        assert_eq!(a.binary_names(), &["claude"]);
        assert_eq!(a.profile_subdir(), Path::new("claude"));
    }

    #[test]
    fn plan_emits_both_env_vars_unconditionally() {
        let a = ClaudeAdapter;
        let name = ProfileName::new("work").unwrap();
        let root = PathBuf::from("/maru/profiles/work");
        let home = PathBuf::from("/Users/test"); // macOS-shaped to avoid gate
        let env = FakeEnvironment::new();
        let plan = a.plan(&ctx(&name, &root, &home), &env).unwrap();

        let keys: Vec<_> = plan
            .env
            .iter()
            .map(|(k, _)| k.to_string_lossy().into_owned())
            .collect();
        assert!(
            keys.iter().any(|k| k == "CLAUDE_CONFIG_DIR"),
            "must emit CLAUDE_CONFIG_DIR; got {keys:?}"
        );
        assert!(
            keys.iter().any(|k| k == "CLAUDE_CODE_PLUGIN_CACHE_DIR"),
            "must emit CLAUDE_CODE_PLUGIN_CACHE_DIR (GENESIS §7.1 carve-out); got {keys:?}"
        );

        let cfg = plan
            .env
            .iter()
            .find(|(k, _)| k == "CLAUDE_CONFIG_DIR")
            .map(|(_, v)| v)
            .unwrap();
        assert_eq!(cfg, "/maru/profiles/work/claude");

        let plugin = plan
            .env
            .iter()
            .find(|(k, _)| k == "CLAUDE_CODE_PLUGIN_CACHE_DIR")
            .map(|(_, v)| v)
            .unwrap();
        assert_eq!(plugin, "/maru/profiles/work/claude/plugins");
    }

    #[test]
    fn plan_rejects_relative_root() {
        let a = ClaudeAdapter;
        let name = ProfileName::new("work").unwrap();
        let root = PathBuf::from("relative");
        let home = PathBuf::from("/Users/test");
        let env = FakeEnvironment::new();
        let err = a.plan(&ctx(&name, &root, &home), &env).unwrap_err();
        assert!(matches!(err, AdapterError::NotAbsolute { .. }));
    }

    #[test]
    fn detect_finds_binary() {
        let a = ClaudeAdapter;
        let env = FakeEnvironment::new().with_path_entry("/usr/local/bin", ["claude"]);
        assert!(a.detect(&env).is_found());
    }

    #[test]
    fn detect_returns_notfound() {
        let a = ClaudeAdapter;
        let env = FakeEnvironment::new();
        assert!(matches!(a.detect(&env), Detection::NotFound));
    }

    #[test]
    fn validate_flags_non_directory() {
        let a = ClaudeAdapter;
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let r = a.validate(tmp.path());
        assert!(r.has_errors());
    }

    #[test]
    fn validate_fresh_dir_is_clean() {
        let a = ClaudeAdapter;
        let r = a.validate(Path::new("/nonexistent-path-for-test"));
        assert!(r.diagnostics.is_empty());
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn linux_credential_gate_trips_when_creds_exist_and_no_dbus() {
        let dir = tempfile::tempdir().unwrap();
        let claude_dir = dir.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        std::fs::write(claude_dir.join(".credentials.json"), b"{}").unwrap();

        let name = ProfileName::new("work").unwrap();
        let root = PathBuf::from("/maru/profiles/work");
        let env = FakeEnvironment::new(); // no DBUS_SESSION_BUS_ADDRESS
        let c = ctx(&name, &root, dir.path());
        let diag = linux_wsl_credential_gate(&c, &env).unwrap();
        assert_eq!(diag.level, Level::Error);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn linux_credential_gate_clear_when_dbus_present() {
        let dir = tempfile::tempdir().unwrap();
        let claude_dir = dir.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        std::fs::write(claude_dir.join(".credentials.json"), b"{}").unwrap();

        let name = ProfileName::new("work").unwrap();
        let root = PathBuf::from("/maru/profiles/work");
        let env =
            FakeEnvironment::new().with_var("DBUS_SESSION_BUS_ADDRESS", "unix:abstract=/tmp/dbus");
        let c = ctx(&name, &root, dir.path());
        assert!(linux_wsl_credential_gate(&c, &env).is_none());
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn linux_credential_gate_clear_when_creds_absent() {
        let dir = tempfile::tempdir().unwrap();
        let name = ProfileName::new("work").unwrap();
        let root = PathBuf::from("/maru/profiles/work");
        let env = FakeEnvironment::new();
        let c = ctx(&name, &root, dir.path());
        assert!(linux_wsl_credential_gate(&c, &env).is_none());
    }

    #[test]
    #[cfg(not(target_os = "linux"))]
    fn non_linux_never_trips_gate() {
        let dir = tempfile::tempdir().unwrap();
        // Even with the offending file present, non-Linux should not gate.
        let claude_dir = dir.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        std::fs::write(claude_dir.join(".credentials.json"), b"{}").unwrap();

        let name = ProfileName::new("work").unwrap();
        let root = PathBuf::from("/maru/profiles/work");
        let env = FakeEnvironment::new();
        let c = ctx(&name, &root, dir.path());
        assert!(linux_wsl_credential_gate(&c, &env).is_none());
    }
}
