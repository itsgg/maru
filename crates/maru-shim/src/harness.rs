//! Harness identification and per-harness env emission.
//!
//! GENESIS §7. The shim hand-codes env emission rather than depending on
//! `maru-adapters` (which pulls in serde via maru-core; the shim must
//! compile without serde per GENESIS §9).

use std::ffi::OsString;
use std::path::Path;

use crate::ShimError;

/// One enum variant per harness this shim dispatches to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Harness {
    /// Anthropic Claude Code (`claude`).
    Claude,
    /// OpenAI Codex CLI (`codex`).
    Codex,
    /// Google Gemini CLI (`gemini`). Phase 2.
    Gemini,
}

impl Harness {
    /// Match `argv[0]`'s basename to a harness. Returns `None` if the
    /// shim was invoked under an unknown name.
    pub fn from_basename(basename: &std::ffi::OsStr) -> Option<Self> {
        match basename.to_string_lossy().as_ref() {
            "claude" => Some(Self::Claude),
            "codex" => Some(Self::Codex),
            "gemini" => Some(Self::Gemini),
            _ => None,
        }
    }

    /// Real binary name to look up on PATH.
    pub const fn binary_name(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Gemini => "gemini",
        }
    }

    /// Env vars to emit for the given profile root.
    ///
    /// GENESIS §7.1 (Claude): `CLAUDE_CONFIG_DIR` AND
    /// `CLAUDE_CODE_PLUGIN_CACHE_DIR` (both unconditional, both absolute).
    /// §7.2 (Codex): `CODEX_HOME`. §7.3 (Gemini): `GEMINI_CLI_HOME`
    /// (Phase 2).
    pub fn env_for_profile(self, profile_root: &Path) -> Vec<(&'static str, OsString)> {
        match self {
            Self::Claude => vec![
                (
                    "CLAUDE_CONFIG_DIR",
                    profile_root.join("claude").into_os_string(),
                ),
                (
                    "CLAUDE_CODE_PLUGIN_CACHE_DIR",
                    profile_root.join("claude/plugins").into_os_string(),
                ),
            ],
            Self::Codex => vec![("CODEX_HOME", profile_root.join("codex").into_os_string())],
            Self::Gemini => vec![(
                "GEMINI_CLI_HOME",
                profile_root.join("gemini").into_os_string(),
            )],
        }
    }
}

/// Linux/WSL Claude credential gate (GENESIS §7.1, issue #47661).
///
/// Tripped when `target_os` is Linux/WSL2 AND `~/.claude/.credentials.json`
/// exists AND `DBUS_SESSION_BUS_ADDRESS` is unset (a proxy for "no
/// keyring available"). Returns `Some(ShimError)` to abort the launch.
///
/// On non-Linux platforms always returns `None` (the gate doesn't apply).
pub fn linux_wsl_credential_gate(home_dir: &Path) -> Option<ShimError> {
    if !is_linux_or_wsl() {
        return None;
    }
    let creds = home_dir.join(".claude").join(".credentials.json");
    if !creds.exists() {
        return None;
    }
    if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some() {
        return None;
    }
    Some(ShimError::gate(
        format!(
            "Linux/WSL credential gate: {} exists and would silently override the per-profile credentials in your active maru profile (Claude Code issue #47661).",
            creds.display()
        ),
        format!(
            "Move it aside to enable profile isolation:\n         mv {} {}.maru-bak",
            creds.display(),
            creds.display()
        ),
    ))
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
    clippy::indexing_slicing,
    reason = "tests"
)]
mod tests {
    use super::Harness;
    use std::ffi::OsStr;
    use std::path::PathBuf;

    #[test]
    fn from_basename_dispatches() {
        assert_eq!(
            Harness::from_basename(OsStr::new("claude")),
            Some(Harness::Claude)
        );
        assert_eq!(
            Harness::from_basename(OsStr::new("codex")),
            Some(Harness::Codex)
        );
        assert_eq!(
            Harness::from_basename(OsStr::new("gemini")),
            Some(Harness::Gemini)
        );
    }

    #[test]
    fn from_basename_unknown_returns_none() {
        assert!(Harness::from_basename(OsStr::new("bash")).is_none());
        assert!(Harness::from_basename(OsStr::new("aider")).is_none());
        assert!(Harness::from_basename(OsStr::new("")).is_none());
    }

    #[test]
    fn binary_name_matches_basename() {
        for h in [Harness::Claude, Harness::Codex, Harness::Gemini] {
            assert_eq!(
                Harness::from_basename(OsStr::new(h.binary_name())),
                Some(h),
                "round-trip {h:?}"
            );
        }
    }

    #[test]
    fn claude_emits_both_env_vars() {
        let root = PathBuf::from("/maru/profiles/work");
        let env = Harness::Claude.env_for_profile(&root);
        assert_eq!(env.len(), 2);
        let keys: Vec<&str> = env.iter().map(|(k, _)| *k).collect();
        assert!(keys.contains(&"CLAUDE_CONFIG_DIR"));
        assert!(keys.contains(&"CLAUDE_CODE_PLUGIN_CACHE_DIR"));

        let cfg = env
            .iter()
            .find(|(k, _)| *k == "CLAUDE_CONFIG_DIR")
            .map(|(_, v)| v.clone())
            .unwrap();
        assert_eq!(cfg, "/maru/profiles/work/claude");
        let plugin = env
            .iter()
            .find(|(k, _)| *k == "CLAUDE_CODE_PLUGIN_CACHE_DIR")
            .map(|(_, v)| v.clone())
            .unwrap();
        assert_eq!(plugin, "/maru/profiles/work/claude/plugins");
    }

    #[test]
    fn codex_emits_one_env_var() {
        let root = PathBuf::from("/maru/profiles/work");
        let env = Harness::Codex.env_for_profile(&root);
        assert_eq!(env.len(), 1);
        assert_eq!(env[0].0, "CODEX_HOME");
        assert_eq!(env[0].1, "/maru/profiles/work/codex");
    }

    #[test]
    fn gemini_emits_one_env_var() {
        let root = PathBuf::from("/maru/profiles/work");
        let env = Harness::Gemini.env_for_profile(&root);
        assert_eq!(env.len(), 1);
        assert_eq!(env[0].0, "GEMINI_CLI_HOME");
        assert_eq!(env[0].1, "/maru/profiles/work/gemini");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn linux_gate_trips_when_creds_exist_no_dbus() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
        std::fs::write(dir.path().join(".claude").join(".credentials.json"), b"{}").unwrap();
        // Manually clear DBUS_SESSION_BUS_ADDRESS.
        // SAFETY: tests serialize on this var via #[serial] when needed;
        // here we accept the race in single-threaded test exec.
        #[allow(unsafe_code, reason = "test setup")]
        unsafe {
            std::env::remove_var("DBUS_SESSION_BUS_ADDRESS");
        }
        assert!(super::linux_wsl_credential_gate(dir.path()).is_some());
    }

    #[test]
    #[cfg(not(target_os = "linux"))]
    fn non_linux_never_trips() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
        std::fs::write(dir.path().join(".claude").join(".credentials.json"), b"{}").unwrap();
        assert!(super::linux_wsl_credential_gate(dir.path()).is_none());
    }
}
