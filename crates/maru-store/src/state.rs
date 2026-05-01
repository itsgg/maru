//! `state.toml` schema and read/write per GENESIS §10.

use std::collections::BTreeMap;
use std::path::Path;

use maru_core::{HarnessId, ProfileName};

use crate::{Error, atomic, lock, time::now_iso8601};

/// Top-level schema of `state.toml`. Versioned; bumping `schema_version`
/// requires a migration plan documented in an ADR.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct State {
    /// Schema major version. Currently 1; unknown major versions are a
    /// hard error on read.
    pub schema_version: u32,

    /// Per-profile metadata, keyed by profile name.
    #[serde(default)]
    pub profiles: BTreeMap<String, ProfileEntry>,

    /// Defaults applied when no profile is otherwise selected.
    #[serde(default)]
    pub defaults: Defaults,
}

impl Default for State {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            profiles: BTreeMap::new(),
            defaults: Defaults::default(),
        }
    }
}

/// The current schema version this build understands.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// One entry under `[profiles.<name>]`.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProfileEntry {
    /// ISO-8601 UTC timestamp.
    pub created_at: String,
    /// ISO-8601 UTC timestamp; updated on every successful activation.
    pub last_used_at: String,
    /// Harnesses this profile is configured for.
    pub harnesses: Vec<HarnessId>,
    /// True after the first successful activation.
    #[serde(default)]
    pub ever_activated: bool,
}

impl ProfileEntry {
    /// New entry with `created_at = last_used_at = now()`.
    #[must_use]
    pub fn new(harnesses: Vec<HarnessId>) -> Self {
        let now = now_iso8601();
        Self {
            created_at: now.clone(),
            last_used_at: now,
            harnesses,
            ever_activated: false,
        }
    }
}

/// Defaults table. Currently just the fallback profile.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Defaults {
    /// Profile name used when no other source resolves (per GENESIS §10
    /// resolution table). `None` if no default is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
}

/// Read `state.toml` from `<maru_home>/state.toml`, returning a default
/// (empty) state if the file does not exist.
///
/// # Errors
///
/// Returns [`Error`] for I/O failures, TOML parse failures, or unknown
/// major schema versions.
pub fn read(maru_home: &Path) -> Result<State, Error> {
    let path = maru_home.join("state.toml");
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(State::default()),
        Err(source) => {
            return Err(Error::Io {
                operation: "read state.toml".to_owned(),
                path,
                source,
            });
        }
    };

    let text = String::from_utf8(bytes).map_err(|e| Error::Decode {
        path: path.clone(),
        message: format!("state.toml is not valid UTF-8: {e}"),
    })?;

    let state: State = toml::from_str(&text).map_err(|e| Error::Decode {
        path: path.clone(),
        message: format!("state.toml parse error: {e}"),
    })?;

    if state.schema_version != CURRENT_SCHEMA_VERSION {
        return Err(Error::Decode {
            path,
            message: format!(
                "state.toml schema_version {} not supported (this build understands {})",
                state.schema_version, CURRENT_SCHEMA_VERSION
            ),
        });
    }

    Ok(state)
}

/// Write `state` to `<maru_home>/state.toml` atomically, holding the
/// advisory write lock for the duration.
///
/// # Errors
///
/// Returns [`Error`] for lock-acquisition or I/O failures, or TOML
/// serialization failures.
pub fn write(maru_home: &Path, state: &State) -> Result<(), Error> {
    lock::with_write_lock(maru_home, || {
        let text = toml::to_string_pretty(state).map_err(|e| Error::Encode {
            message: format!("state.toml serialize error: {e}"),
        })?;
        let path = maru_home.join("state.toml");
        atomic::write_atomic(&path, text.as_bytes())
    })
}

/// Mutate the on-disk state via a closure.
///
/// Acquires the lock, reads, calls `f`, writes, releases. The closure
/// runs while the lock is held.
///
/// # Errors
///
/// Same as [`read`] and [`write`].
pub fn update<F>(maru_home: &Path, f: F) -> Result<(), Error>
where
    F: FnOnce(&mut State) -> Result<(), Error>,
{
    lock::with_write_lock(maru_home, || {
        let mut state = read(maru_home)?;
        f(&mut state)?;
        let text = toml::to_string_pretty(&state).map_err(|e| Error::Encode {
            message: format!("state.toml serialize error: {e}"),
        })?;
        let path = maru_home.join("state.toml");
        atomic::write_atomic(&path, text.as_bytes())
    })
}

/// Insert a new profile entry. Errors if the name already exists.
///
/// # Errors
///
/// Returns [`Error::ProfileExists`] if `name` is already in the state.
pub fn insert_profile(
    maru_home: &Path,
    name: &ProfileName,
    entry: ProfileEntry,
) -> Result<(), Error> {
    update(maru_home, |state| {
        if state.profiles.contains_key(name.as_str()) {
            return Err(Error::ProfileExists {
                name: name.as_str().to_owned(),
            });
        }
        state.profiles.insert(name.as_str().to_owned(), entry);
        Ok(())
    })
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
        CURRENT_SCHEMA_VERSION, Defaults, ProfileEntry, State, insert_profile, read, write,
    };
    use maru_core::{HarnessId, ProfileName};

    #[test]
    fn read_returns_default_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let s = read(dir.path()).unwrap();
        assert_eq!(s, State::default());
        assert_eq!(s.schema_version, CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn write_then_read_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = State::default();
        state.profiles.insert(
            "work".to_owned(),
            ProfileEntry::new(vec![HarnessId::Claude]),
        );
        state.defaults = Defaults {
            profile: Some("work".to_owned()),
        };
        write(dir.path(), &state).unwrap();
        let back = read(dir.path()).unwrap();
        assert_eq!(back, state);
    }

    #[test]
    fn rejects_unknown_schema_version() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("state.toml"),
            "schema_version = 999\n[profiles]\n[defaults]\n",
        )
        .unwrap();
        let err = read(dir.path()).unwrap_err();
        assert!(format!("{err}").contains("schema_version 999"));
    }

    #[test]
    fn rejects_invalid_toml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("state.toml"), "this is not toml {{").unwrap();
        let err = read(dir.path()).unwrap_err();
        assert!(format!("{err}").contains("parse error"));
    }

    #[test]
    fn insert_profile_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let name = ProfileName::new("work").unwrap();
        let entry = ProfileEntry::new(vec![HarnessId::Claude, HarnessId::Codex]);
        insert_profile(dir.path(), &name, entry.clone()).unwrap();

        let state = read(dir.path()).unwrap();
        assert_eq!(state.profiles.len(), 1);
        assert_eq!(state.profiles.get("work"), Some(&entry));
    }

    #[test]
    fn insert_profile_rejects_duplicate() {
        let dir = tempfile::tempdir().unwrap();
        let name = ProfileName::new("work").unwrap();
        let entry = ProfileEntry::new(vec![HarnessId::Claude]);
        insert_profile(dir.path(), &name, entry.clone()).unwrap();
        let err = insert_profile(dir.path(), &name, entry).unwrap_err();
        assert!(format!("{err}").contains("already exists"));
    }

    #[test]
    fn missing_defaults_in_file_parses_as_empty() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("state.toml"),
            "schema_version = 1\n[profiles]\n",
        )
        .unwrap();
        let s = read(dir.path()).unwrap();
        assert!(s.defaults.profile.is_none());
    }

    #[test]
    fn defaults_table_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let state = State {
            schema_version: CURRENT_SCHEMA_VERSION,
            profiles: std::collections::BTreeMap::new(),
            defaults: Defaults {
                profile: Some("personal".to_owned()),
            },
        };
        write(dir.path(), &state).unwrap();
        let back = read(dir.path()).unwrap();
        assert_eq!(back, state);
    }
}
