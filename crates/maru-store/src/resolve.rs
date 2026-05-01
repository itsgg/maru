//! Profile resolution: env > `.maru` > `active.txt` > `[defaults].profile`.
//!
//! GENESIS §10 "Profile-resolution sources" table.

use std::path::{Path, PathBuf};

use maru_core::{Environment, ProfileName};

use crate::{Error, active, state};

/// Source the resolved profile came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// `MARU_PROFILE` env var (non-empty).
    Env,
    /// `.maru` file walked from `cwd`.
    Pin,
    /// `<maru_home>/active.txt` (non-empty single line).
    Active,
    /// `[defaults].profile` in `state.toml`.
    Default,
}

/// One resolved profile name plus its source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved {
    /// The name of the active profile.
    pub name: ProfileName,
    /// Where it came from.
    pub source: Source,
}

/// Outcome of walking up from `cwd` looking for a `.maru` file.
///
/// GENESIS §12: an unknown / invalid profile name in `.maru` is an
/// explicit user-error condition, NOT a silent fall-through to the next
/// resolution source. This 3-state result lets callers distinguish:
///
/// - [`PinLookup::NotFound`] — no `.maru` was found anywhere up the
///   chain. Callers continue resolution with `active.txt` / defaults.
/// - [`PinLookup::Found`] — a `.maru` with a valid profile name. Use it.
/// - [`PinLookup::Invalid`] — a `.maru` exists but its first line does
///   not parse as a [`ProfileName`]. Callers must surface this as a
///   typed error rather than silently continuing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PinLookup {
    /// No `.maru` file in or above `cwd`.
    NotFound,
    /// Valid pin found.
    Found {
        /// The resolved profile name.
        name: ProfileName,
        /// The directory containing the `.maru` file.
        pin_dir: PathBuf,
    },
    /// A `.maru` file exists but its first line is not a valid profile name.
    Invalid {
        /// Path to the offending `.maru` file.
        path: PathBuf,
        /// The raw first line that failed to parse, trimmed.
        line: String,
    },
}

/// Resolve the active profile name following the GENESIS §10 source
/// order.
///
/// `cwd` is the working directory used to walk for a `.maru` file.
/// Pass `None` to skip the project-pin source (the CLI uses this when it
/// shouldn't honor pins, e.g. `maru profile current --no-pin`).
///
/// Returns `Ok(None)` if every source is empty.
///
/// # Errors
///
/// Returns [`Error`] on I/O failures reading `active.txt` or `state.toml`,
/// **or** [`Error::InvalidPin`] if a `.maru` file is present but its
/// first line does not parse as a valid profile name (GENESIS §12: this
/// is a hard error, not a silent fall-through).
pub fn resolve(
    env: &dyn Environment,
    maru_home: &Path,
    cwd: Option<&Path>,
) -> Result<Option<Resolved>, Error> {
    // 1. MARU_PROFILE env var (non-empty).
    if let Some(raw) = env.var("MARU_PROFILE") {
        let raw = raw.to_string_lossy();
        let trimmed = raw.trim();
        if !trimmed.is_empty()
            && let Ok(name) = ProfileName::new(trimmed)
        {
            return Ok(Some(Resolved {
                name,
                source: Source::Env,
            }));
        }
    }

    // 2. .maru file walked from cwd upward, stopping at $HOME or root.
    //    GENESIS §12: an invalid name in .maru is a hard error — never
    //    silently fall through to active.txt.
    if let Some(cwd) = cwd {
        match walk_for_pin(cwd, env.home_dir().as_deref())? {
            PinLookup::Found { name, .. } => {
                return Ok(Some(Resolved {
                    name,
                    source: Source::Pin,
                }));
            }
            PinLookup::Invalid { path, line } => {
                return Err(Error::InvalidPin { path, line });
            }
            PinLookup::NotFound => {}
        }
    }

    // 3. active.txt
    if let Some(name) = active::read(maru_home)? {
        return Ok(Some(Resolved {
            name,
            source: Source::Active,
        }));
    }

    // 4. [defaults].profile
    let s = state::read(maru_home)?;
    if let Some(default) = s.defaults.profile
        && let Ok(name) = ProfileName::new(default)
    {
        return Ok(Some(Resolved {
            name,
            source: Source::Default,
        }));
    }

    Ok(None)
}

/// Walk up from `start` looking for a `.maru` file; stop at `home_dir` or
/// the filesystem root, whichever comes first.
///
/// Returns the first hit's outcome:
/// - [`PinLookup::Found`] — a `.maru` with a valid profile name.
/// - [`PinLookup::Invalid`] — a `.maru` exists but its first line does
///   not parse as a valid profile name. GENESIS §12 mandates this is a
///   hard error, not a silent fall-through.
/// - [`PinLookup::NotFound`] — no `.maru` was found anywhere up to
///   `home_dir` / the filesystem root.
///
/// # Errors
///
/// I/O errors during file-read are surfaced as [`Error::Io`].
pub fn walk_for_pin(start: &Path, home_dir: Option<&Path>) -> Result<PinLookup, Error> {
    let mut current: Option<&Path> = Some(start);
    while let Some(dir) = current {
        let candidate = dir.join(".maru");
        match std::fs::read(&candidate) {
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes);
                let raw_line = text.lines().next().unwrap_or("").trim().to_owned();
                if raw_line.is_empty() {
                    // An empty .maru file is an explicit "no pin"
                    // signal; not invalid, just absent. Treat as not
                    // found so resolution falls through naturally.
                    return Ok(PinLookup::NotFound);
                }
                let result = ProfileName::new(raw_line.clone()).map_or_else(
                    |_| PinLookup::Invalid {
                        path: candidate.clone(),
                        line: raw_line.clone(),
                    },
                    |name| PinLookup::Found {
                        name,
                        pin_dir: dir.to_path_buf(),
                    },
                );
                return Ok(result);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(Error::Io {
                    operation: "read .maru".to_owned(),
                    path: candidate,
                    source,
                });
            }
        }

        // Stop at the user's home dir to avoid leaking outside it.
        if Some(dir) == home_dir {
            break;
        }
        current = dir.parent();
    }
    Ok(PinLookup::NotFound)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests"
)]
mod tests {
    use super::{PinLookup, Source, resolve, walk_for_pin};
    use crate::Error;
    use crate::state::{Defaults, ProfileEntry, State, write as write_state};
    use maru_core::{FakeEnvironment, HarnessId, ProfileName};

    #[test]
    fn returns_none_when_all_sources_empty() {
        let dir = tempfile::tempdir().unwrap();
        let env = FakeEnvironment::new();
        assert!(resolve(&env, dir.path(), None).unwrap().is_none());
    }

    #[test]
    fn env_var_wins_over_active_txt() {
        let dir = tempfile::tempdir().unwrap();
        // active.txt says "personal"
        crate::active::write(dir.path(), Some(&ProfileName::new("personal").unwrap())).unwrap();
        // env says "work"
        let env = FakeEnvironment::new().with_var("MARU_PROFILE", "work");
        let r = resolve(&env, dir.path(), None).unwrap().unwrap();
        assert_eq!(r.name.as_str(), "work");
        assert_eq!(r.source, Source::Env);
    }

    #[test]
    fn empty_env_var_falls_through() {
        let dir = tempfile::tempdir().unwrap();
        crate::active::write(dir.path(), Some(&ProfileName::new("personal").unwrap())).unwrap();
        let env = FakeEnvironment::new().with_var("MARU_PROFILE", "");
        let r = resolve(&env, dir.path(), None).unwrap().unwrap();
        assert_eq!(r.name.as_str(), "personal");
        assert_eq!(r.source, Source::Active);
    }

    #[test]
    fn invalid_env_var_falls_through() {
        let dir = tempfile::tempdir().unwrap();
        crate::active::write(dir.path(), Some(&ProfileName::new("personal").unwrap())).unwrap();
        let env = FakeEnvironment::new().with_var("MARU_PROFILE", "-bad");
        let r = resolve(&env, dir.path(), None).unwrap().unwrap();
        assert_eq!(r.source, Source::Active);
    }

    #[test]
    fn defaults_used_when_active_empty() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = State::default();
        state.profiles.insert(
            "personal".to_owned(),
            ProfileEntry::new(vec![HarnessId::Claude]),
        );
        state.defaults = Defaults {
            profile: Some("personal".to_owned()),
        };
        write_state(dir.path(), &state).unwrap();
        let env = FakeEnvironment::new();
        let r = resolve(&env, dir.path(), None).unwrap().unwrap();
        assert_eq!(r.name.as_str(), "personal");
        assert_eq!(r.source, Source::Default);
    }

    #[test]
    fn pin_walks_up_to_home() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let project = home.join("repo");
        let nested = project.join("src").join("deep");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(project.join(".maru"), "work\n").unwrap();

        let env = FakeEnvironment::new().with_home(home);
        let r = resolve(&env, dir.path(), Some(&nested)).unwrap().unwrap();
        assert_eq!(r.name.as_str(), "work");
        assert_eq!(r.source, Source::Pin);
    }

    #[test]
    fn pin_wins_over_active_but_loses_to_env() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("repo");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(project.join(".maru"), "pinned\n").unwrap();
        crate::active::write(dir.path(), Some(&ProfileName::new("active").unwrap())).unwrap();

        let env = FakeEnvironment::new().with_home(dir.path());
        let r = resolve(&env, dir.path(), Some(&project)).unwrap().unwrap();
        assert_eq!(r.name.as_str(), "pinned");

        let env_w_var = env.with_var("MARU_PROFILE", "via-env");
        let r = resolve(&env_w_var, dir.path(), Some(&project))
            .unwrap()
            .unwrap();
        assert_eq!(r.name.as_str(), "via-env");
        assert_eq!(r.source, Source::Env);
    }

    #[test]
    fn pin_with_invalid_name_returns_typed_error() {
        // GENESIS §12: a `.maru` containing an invalid profile name must
        // surface as a typed error, not silently fall through to
        // active.txt (which would hide the user's intent).
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("repo");
        std::fs::create_dir_all(&project).unwrap();
        let pin_path = project.join(".maru");
        std::fs::write(&pin_path, "-not-valid\n").unwrap();
        // Active profile would otherwise mask the failure if we fell through:
        crate::active::write(dir.path(), Some(&ProfileName::new("active").unwrap())).unwrap();

        let env = FakeEnvironment::new().with_home(dir.path());
        let err = resolve(&env, dir.path(), Some(&project)).unwrap_err();
        match err {
            Error::InvalidPin { path, line } => {
                assert_eq!(path, pin_path);
                assert_eq!(line, "-not-valid");
            }
            other => panic!("expected Error::InvalidPin, got {other:?}"),
        }
    }

    #[test]
    fn walk_for_pin_returns_invalid_for_bad_name() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("repo");
        std::fs::create_dir_all(&project).unwrap();
        let pin_path = project.join(".maru");
        std::fs::write(&pin_path, "-not-valid\n").unwrap();

        let res = walk_for_pin(&project, Some(dir.path())).unwrap();
        match res {
            PinLookup::Invalid { path, line } => {
                assert_eq!(path, pin_path);
                assert_eq!(line, "-not-valid");
            }
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn walk_for_pin_returns_found_for_valid_name() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("repo");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(project.join(".maru"), "work\n").unwrap();

        let res = walk_for_pin(&project, Some(dir.path())).unwrap();
        match res {
            PinLookup::Found { name, pin_dir } => {
                assert_eq!(name.as_str(), "work");
                assert_eq!(pin_dir, project);
            }
            other => panic!("expected Found, got {other:?}"),
        }
    }

    #[test]
    fn walk_for_pin_returns_not_found_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let res = walk_for_pin(dir.path(), Some(dir.path())).unwrap();
        assert_eq!(res, PinLookup::NotFound);
    }

    #[test]
    fn empty_pin_file_treated_as_not_found() {
        // An empty .maru file is a no-op signal, not an error: resolution
        // continues with active.txt / defaults.
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("repo");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(project.join(".maru"), "").unwrap();
        crate::active::write(dir.path(), Some(&ProfileName::new("active").unwrap())).unwrap();

        let env = FakeEnvironment::new().with_home(dir.path());
        let r = resolve(&env, dir.path(), Some(&project)).unwrap().unwrap();
        assert_eq!(r.name.as_str(), "active");
        assert_eq!(r.source, Source::Active);
    }
}
