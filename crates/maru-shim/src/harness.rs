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
    /// §7.2 (Codex): `CODEX_HOME`. §7.3 (Gemini): `GEMINI_CLI_HOME`.
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
/// `vars` is a callback that returns environment-variable values. The
/// production caller passes a closure over [`std::env::var_os`]; tests
/// pass an in-memory map so they don't have to mutate the real process
/// env (which is racy when tests run in parallel).
///
/// On non-Linux platforms always returns `None` (the gate doesn't apply).
pub fn linux_wsl_credential_gate<F>(home_dir: &Path, vars: F) -> Option<ShimError>
where
    F: Fn(&str) -> Option<OsString>,
{
    if !is_linux_or_wsl() {
        return None;
    }
    let creds = home_dir.join(".claude").join(".credentials.json");
    if !creds.exists() {
        return None;
    }
    if vars("DBUS_SESSION_BUS_ADDRESS").is_some() {
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

/// Gemini keychain-isolation warning (GENESIS §7.3).
///
/// When the user has set `GEMINI_FORCE_ENCRYPTED_FILE_STORAGE` to a
/// truthy value, Gemini stores OAuth tokens in a single shared keychain
/// service name (`gemini-cli-oauth`), at which point per-profile
/// isolation breaks. The shim emits this as a warning to stderr; it is
/// **not** a fatal gate (the user may know what they're doing).
///
/// Returns `Some((message, help))` if the warning should fire,
/// `None` otherwise. `vars` is a callback returning env-var values, as
/// in [`linux_wsl_credential_gate`].
pub fn gemini_keychain_warning<F>(vars: F) -> Option<(String, String)>
where
    F: Fn(&str) -> Option<OsString>,
{
    let raw = vars("GEMINI_FORCE_ENCRYPTED_FILE_STORAGE")?;
    if !is_truthy(&raw) {
        return None;
    }
    Some((
        "GEMINI_FORCE_ENCRYPTED_FILE_STORAGE is set; Gemini OAuth tokens go to a single shared keychain entry (`gemini-cli-oauth`), breaking per-profile isolation."
            .to_owned(),
        "Unset the variable to restore profile isolation:\n         unset GEMINI_FORCE_ENCRYPTED_FILE_STORAGE"
            .to_owned(),
    ))
}

fn is_truthy(v: &OsString) -> bool {
    let s = v.to_string_lossy();
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
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
    clippy::indexing_slicing,
    reason = "tests"
)]
mod tests {
    use super::Harness;
    use std::ffi::{OsStr, OsString};
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
        assert_eq!(cfg, root.join("claude").as_os_str());
        let plugin = env
            .iter()
            .find(|(k, _)| *k == "CLAUDE_CODE_PLUGIN_CACHE_DIR")
            .map(|(_, v)| v.clone())
            .unwrap();
        assert_eq!(plugin, root.join("claude/plugins").as_os_str());
    }

    #[test]
    fn codex_emits_one_env_var() {
        let root = PathBuf::from("/maru/profiles/work");
        let env = Harness::Codex.env_for_profile(&root);
        assert_eq!(env.len(), 1);
        assert_eq!(env[0].0, "CODEX_HOME");
        assert_eq!(env[0].1, root.join("codex").as_os_str());
    }

    #[test]
    fn gemini_emits_one_env_var() {
        let root = PathBuf::from("/maru/profiles/work");
        let env = Harness::Gemini.env_for_profile(&root);
        assert_eq!(env.len(), 1);
        assert_eq!(env[0].0, "GEMINI_CLI_HOME");
        assert_eq!(env[0].1, root.join("gemini").as_os_str());
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn linux_gate_trips_when_creds_exist_no_dbus() {
        // No `unsafe`, no real-process-env mutation: the gate takes a
        // closure that returns env var values. Tests pass an empty
        // closure so DBUS_SESSION_BUS_ADDRESS reads as None.
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
        std::fs::write(dir.path().join(".claude").join(".credentials.json"), b"{}").unwrap();
        let no_vars = |_: &str| -> Option<OsString> { None };
        assert!(super::linux_wsl_credential_gate(dir.path(), no_vars).is_some());
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn linux_gate_clear_when_dbus_present() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
        std::fs::write(dir.path().join(".claude").join(".credentials.json"), b"{}").unwrap();
        let with_dbus = |k: &str| -> Option<OsString> {
            if k == "DBUS_SESSION_BUS_ADDRESS" {
                Some(OsString::from("unix:abstract=/tmp/dbus"))
            } else {
                None
            }
        };
        assert!(super::linux_wsl_credential_gate(dir.path(), with_dbus).is_none());
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn linux_gate_clear_when_creds_absent() {
        let dir = tempfile::tempdir().unwrap();
        let no_vars = |_: &str| -> Option<OsString> { None };
        assert!(super::linux_wsl_credential_gate(dir.path(), no_vars).is_none());
    }

    #[test]
    #[cfg(not(target_os = "linux"))]
    fn non_linux_never_trips() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
        std::fs::write(dir.path().join(".claude").join(".credentials.json"), b"{}").unwrap();
        let no_vars = |_: &str| -> Option<OsString> { None };
        assert!(super::linux_wsl_credential_gate(dir.path(), no_vars).is_none());
    }

    #[test]
    fn gemini_keychain_warning_fires_when_truthy() {
        let on = |k: &str| -> Option<OsString> {
            if k == "GEMINI_FORCE_ENCRYPTED_FILE_STORAGE" {
                Some(OsString::from("true"))
            } else {
                None
            }
        };
        let warn = super::gemini_keychain_warning(on).expect("warning should fire on `true`");
        assert!(warn.0.contains("GEMINI_FORCE_ENCRYPTED_FILE_STORAGE"));
        assert!(warn.0.contains("gemini-cli-oauth"));
        assert!(warn.1.contains("Unset"));
    }

    #[test]
    fn gemini_keychain_warning_silent_when_unset_or_falsy() {
        let none = |_: &str| -> Option<OsString> { None };
        assert!(super::gemini_keychain_warning(none).is_none());

        let off = |k: &str| -> Option<OsString> {
            if k == "GEMINI_FORCE_ENCRYPTED_FILE_STORAGE" {
                Some(OsString::from("false"))
            } else {
                None
            }
        };
        assert!(super::gemini_keychain_warning(off).is_none());

        let zero = |k: &str| -> Option<OsString> {
            if k == "GEMINI_FORCE_ENCRYPTED_FILE_STORAGE" {
                Some(OsString::from("0"))
            } else {
                None
            }
        };
        assert!(super::gemini_keychain_warning(zero).is_none());
    }

    #[test]
    fn gemini_keychain_warning_truthy_variants() {
        for v in ["1", "true", "TRUE", "yes", "on"] {
            let f = move |k: &str| -> Option<OsString> {
                if k == "GEMINI_FORCE_ENCRYPTED_FILE_STORAGE" {
                    Some(OsString::from(v))
                } else {
                    None
                }
            };
            assert!(
                super::gemini_keychain_warning(f).is_some(),
                "{v:?} should be truthy"
            );
        }
    }
}
