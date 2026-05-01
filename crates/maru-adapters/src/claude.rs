//! Claude Code adapter.
//!
//! GENESIS §7.1. Mechanism: `CLAUDE_CONFIG_DIR` env var redirection,
//! plus `CLAUDE_CODE_PLUGIN_CACHE_DIR` emitted unconditionally to address
//! upstream issue #15071 (plugin marketplaces dir hardcoded), plus
//! `CLAUDE_CODE_OAUTH_TOKEN` populated from a per-profile `oauth_token`
//! file when present (see "OAuth token isolation" below).
//!
//! Linux/WSL credential gate (#47661): on Linux/WSL2 without a Keychain,
//! `claude` falls through to reading `~/.claude/.credentials.json` even
//! when `CLAUDE_CONFIG_DIR` is set, silently authenticating as the wrong
//! account. The adapter emits `Diagnostic { level: Error }` if the
//! offending file exists; the shim treats this as a fatal pre-exec block
//! per GENESIS §11.
//!
//! ## OAuth token isolation (per-profile credentials)
//!
//! Claude Code on macOS stores OAuth credentials in the system Keychain.
//! While Claude Code derives the Keychain service name from
//! `CLAUDE_CONFIG_DIR` in some versions, the per-config-dir Keychain
//! isolation has been observed to fail across Claude Code 2.1.x in the
//! wild — logging out of one profile clears credentials shared by other
//! profiles. The reliable, documented escape hatch is the
//! `CLAUDE_CODE_OAUTH_TOKEN` env var (Claude Code authentication
//! precedence step 5; see code.claude.com/docs/en/authentication).
//!
//! When `<profile_root>/claude/oauth_token` exists, the adapter reads it
//! (single-line OAuth token, leading/trailing whitespace stripped) and
//! emits `CLAUDE_CODE_OAUTH_TOKEN`. With the token in env, Claude Code
//! authenticates with it directly and does not consult the Keychain at
//! all — every profile gets its own token, isolated by file. Generate a
//! token per profile with `claude setup-token` (or `maru profile login
//! <name>`) and the adapter handles the rest.
//!
//! The `oauth_token` file is on the §8 credential deny-list so it never
//! leaks via clone / export / import.

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

        // Per-profile OAuth token. See module docs.
        if let Some(token) = read_oauth_token(&ctx.profile_root.join("claude/oauth_token")) {
            plan = plan.with_env("CLAUDE_CODE_OAUTH_TOKEN", token);
        }

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

/// Read the per-profile OAuth token from the given path, returning the
/// trimmed contents as an `OsString` suitable for env-var emission.
///
/// Returns `None` if the file is missing, unreadable, empty, or contains
/// only whitespace. Errors during the read (permissions, I/O) are
/// silently swallowed because this is a soft fallback — a missing or
/// unreadable token file just means "use the Keychain instead", which
/// is the pre-isolation default behavior.
///
/// The token is treated as opaque text: the first line, trimmed of
/// surrounding whitespace, is what we emit. This handles the common
/// pasted-from-`claude setup-token` shape where the user adds a trailing
/// newline.
fn read_oauth_token(path: &Path) -> Option<std::ffi::OsString> {
    let raw = std::fs::read_to_string(path).ok()?;
    let line = raw.lines().next()?.trim();
    if line.is_empty() {
        return None;
    }
    Some(line.to_owned().into())
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

    /// OS-appropriate absolute path. `Path::is_absolute()` is platform-
    /// specific (Windows requires drive letters), so build a path under
    /// the OS-appropriate temp root which always satisfies `is_absolute()`.
    /// We don't create anything on disk — only the path shape matters.
    fn abs(parts: &[&str]) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push("maru-test");
        for part in parts {
            p.push(part);
        }
        p
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
        let root = abs(&["maru", "profiles", "work"]);
        let home = abs(&["Users", "test"]); // macOS-shaped to avoid gate
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
        assert_eq!(cfg, root.join("claude").as_os_str());

        let plugin = plan
            .env
            .iter()
            .find(|(k, _)| k == "CLAUDE_CODE_PLUGIN_CACHE_DIR")
            .map(|(_, v)| v)
            .unwrap();
        assert_eq!(plugin, root.join("claude/plugins").as_os_str());
    }

    #[test]
    fn plan_rejects_relative_root() {
        let a = ClaudeAdapter;
        let name = ProfileName::new("work").unwrap();
        let root = PathBuf::from("relative");
        let home = abs(&["Users", "test"]);
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
    fn plan_emits_oauth_token_when_file_present() {
        let a = ClaudeAdapter;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        std::fs::create_dir_all(root.join("claude")).unwrap();
        std::fs::write(root.join("claude/oauth_token"), "sk-ant-oat-abc123\n").unwrap();

        let name = ProfileName::new("work").unwrap();
        let home = abs(&["Users", "test"]);
        let env = FakeEnvironment::new();
        let plan = a.plan(&ctx(&name, &root, &home), &env).unwrap();

        let token = plan
            .env
            .iter()
            .find(|(k, _)| k == "CLAUDE_CODE_OAUTH_TOKEN")
            .map(|(_, v)| v.clone());
        assert_eq!(
            token.as_deref().and_then(|v| v.to_str()),
            Some("sk-ant-oat-abc123"),
            "trailing newline should be stripped"
        );
    }

    #[test]
    fn plan_omits_oauth_token_when_file_absent() {
        let a = ClaudeAdapter;
        let dir = tempfile::tempdir().unwrap();
        let name = ProfileName::new("work").unwrap();
        let home = abs(&["Users", "test"]);
        let env = FakeEnvironment::new();
        let plan = a.plan(&ctx(&name, dir.path(), &home), &env).unwrap();

        assert!(
            plan.env.iter().all(|(k, _)| k != "CLAUDE_CODE_OAUTH_TOKEN"),
            "no token file → no env var"
        );
    }

    #[test]
    fn plan_omits_oauth_token_for_empty_file() {
        let a = ClaudeAdapter;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        std::fs::create_dir_all(root.join("claude")).unwrap();
        std::fs::write(root.join("claude/oauth_token"), "   \n  \n").unwrap();

        let name = ProfileName::new("work").unwrap();
        let home = abs(&["Users", "test"]);
        let env = FakeEnvironment::new();
        let plan = a.plan(&ctx(&name, &root, &home), &env).unwrap();

        assert!(
            plan.env.iter().all(|(k, _)| k != "CLAUDE_CODE_OAUTH_TOKEN"),
            "blank file → no env var"
        );
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
