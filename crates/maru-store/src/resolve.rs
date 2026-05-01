//! Profile resolution: env > `.maru` > `active.txt` > `[defaults].profile`.
//!
//! GENESIS §10 "Profile-resolution sources" table.

use std::path::Path;

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
/// Returns [`Error`] on I/O failures reading `active.txt` or `state.toml`.
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
    if let Some(cwd) = cwd
        && let Some(name) = walk_for_pin(cwd, env.home_dir().as_deref())?
    {
        return Ok(Some(Resolved {
            name,
            source: Source::Pin,
        }));
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
/// the filesystem root, whichever comes first. Return the first hit.
///
/// # Errors
///
/// I/O errors during file-read are surfaced; "not found" yields `Ok(None)`.
fn walk_for_pin(start: &Path, home_dir: Option<&Path>) -> Result<Option<ProfileName>, Error> {
    let mut current: Option<&Path> = Some(start);
    while let Some(dir) = current {
        let candidate = dir.join(".maru");
        match std::fs::read(&candidate) {
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes);
                if let Some(line) = text.lines().next()
                    && !line.trim().is_empty()
                    && let Ok(name) = ProfileName::new(line.trim())
                {
                    return Ok(Some(name));
                }
                return Ok(None);
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
    Ok(None)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests"
)]
mod tests {
    use super::{Source, resolve};
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
    fn pin_with_invalid_name_falls_through() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("repo");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(project.join(".maru"), "-not-valid\n").unwrap();
        crate::active::write(dir.path(), Some(&ProfileName::new("active").unwrap())).unwrap();

        let env = FakeEnvironment::new().with_home(dir.path());
        let r = resolve(&env, dir.path(), Some(&project)).unwrap().unwrap();
        assert_eq!(r.name.as_str(), "active");
        assert_eq!(r.source, Source::Active);
    }
}
