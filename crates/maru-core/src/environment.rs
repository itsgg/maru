//! [`Environment`] — abstraction over `std::env`, `PATH`, and `which`.
//!
//! GENESIS §6 line 240. Adapters consume this trait so they remain pure
//! and unit-testable. The real implementation, [`SystemEnvironment`], is
//! the only thing that touches the actual process env.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Read-only view over the process environment, PATH, and `which`-style
/// binary discovery.
///
/// Adapters take `&dyn Environment` so tests can swap in a fake. The shim
/// constructs a [`SystemEnvironment`] once at `main()` and hands it to the
/// activation pipeline.
pub trait Environment: std::fmt::Debug {
    /// Look up an environment variable. Mirrors [`std::env::var_os`].
    fn var(&self, key: &str) -> Option<OsString>;

    /// Parse the user's PATH into a vector of directories.
    fn path(&self) -> Vec<PathBuf>;

    /// Locate `name` on PATH, but skip any entry under `skip`. Used by the
    /// shim to find the *real* harness binary while ignoring its own
    /// install dir (GENESIS §9 algorithm step 7).
    fn which_skipping(&self, name: &str, skip: &Path) -> Option<PathBuf>;

    /// The user's home directory, if any.
    fn home_dir(&self) -> Option<PathBuf>;
}

/// Real implementation of [`Environment`] backed by the process env.
///
/// This is the only thing in `maru-core` that reads from the global
/// environment. Everything else is pure-over-data.
#[derive(Debug, Default)]
pub struct SystemEnvironment {
    _private: (),
}

impl SystemEnvironment {
    /// Create a fresh handle. Cheap; no allocations.
    #[must_use]
    pub const fn new() -> Self {
        Self { _private: () }
    }
}

impl Environment for SystemEnvironment {
    fn var(&self, key: &str) -> Option<OsString> {
        std::env::var_os(key)
    }

    fn path(&self) -> Vec<PathBuf> {
        std::env::var_os("PATH")
            .map(|p| std::env::split_paths(&p).collect())
            .unwrap_or_default()
    }

    fn which_skipping(&self, name: &str, skip: &Path) -> Option<PathBuf> {
        let skip_canonical = skip.canonicalize().ok();
        for dir in self.path() {
            // Skip our own install directory.
            if let Some(ref s) = skip_canonical {
                if dir.canonicalize().ok().as_deref() == Some(s) {
                    continue;
                }
            } else if dir == skip {
                continue;
            }

            let candidate = dir.join(name);
            if is_executable(&candidate) {
                return Some(candidate);
            }

            // On Windows, also check well-known executable extensions.
            #[cfg(windows)]
            for ext in [".exe", ".cmd", ".bat"] {
                let with_ext = dir.join(format!("{name}{ext}"));
                if is_executable(&with_ext) {
                    return Some(with_ext);
                }
            }
        }
        None
    }

    fn home_dir(&self) -> Option<PathBuf> {
        // GENESIS §13 lists `directories` as an allowed dep but the shim
        // bans it. For now, hand-roll: $HOME on Unix, %USERPROFILE% on
        // Windows. Phase 1 may swap to `directories` for maru-core+store
        // (the non-shim crates).
        #[cfg(unix)]
        {
            std::env::var_os("HOME").map(PathBuf::from)
        }
        #[cfg(windows)]
        {
            std::env::var_os("USERPROFILE").map(PathBuf::from)
        }
        #[cfg(not(any(unix, windows)))]
        {
            None
        }
    }
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

/// In-memory test double for [`Environment`].
///
/// Lives in this module (rather than under `#[cfg(test)]`) so adapter
/// tests in other crates can use it without depending on a test-only
/// re-export.
#[derive(Debug, Default, Clone)]
pub struct FakeEnvironment {
    /// Variables to return from `var()`.
    pub vars: std::collections::HashMap<String, OsString>,
    /// PATH directories.
    pub path: Vec<PathBuf>,
    /// Files that `which_skipping` should report as executable, keyed by
    /// the full directory.
    pub executables: std::collections::HashMap<PathBuf, Vec<String>>,
    /// Value returned by `home_dir()`.
    pub home: Option<PathBuf>,
}

impl FakeEnvironment {
    /// New empty fake.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set an env var.
    #[must_use]
    pub fn with_var(mut self, key: &str, value: impl Into<OsString>) -> Self {
        self.vars.insert(key.to_owned(), value.into());
        self
    }

    /// Set the home dir.
    #[must_use]
    pub fn with_home(mut self, home: impl Into<PathBuf>) -> Self {
        self.home = Some(home.into());
        self
    }

    /// Add a PATH entry that contains the listed executable file names.
    #[must_use]
    pub fn with_path_entry(
        mut self,
        dir: impl Into<PathBuf>,
        executables: impl IntoIterator<Item = &'static str>,
    ) -> Self {
        let dir = dir.into();
        self.path.push(dir.clone());
        self.executables
            .insert(dir, executables.into_iter().map(str::to_owned).collect());
        self
    }
}

impl Environment for FakeEnvironment {
    fn var(&self, key: &str) -> Option<OsString> {
        self.vars.get(key).cloned()
    }

    fn path(&self) -> Vec<PathBuf> {
        self.path.clone()
    }

    fn which_skipping(&self, name: &str, skip: &Path) -> Option<PathBuf> {
        for dir in &self.path {
            if dir == skip {
                continue;
            }
            if let Some(execs) = self.executables.get(dir)
                && execs.iter().any(|n| n == name)
            {
                return Some(dir.join(name));
            }
        }
        None
    }

    fn home_dir(&self) -> Option<PathBuf> {
        self.home.clone()
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests"
)]
mod tests {
    use super::{Environment, FakeEnvironment, SystemEnvironment};
    use std::path::PathBuf;

    #[test]
    fn system_env_path_is_non_empty_on_normal_systems() {
        let env = SystemEnvironment::new();
        // PATH should be set on any reasonable test environment.
        assert!(!env.path().is_empty());
    }

    #[test]
    fn fake_env_var_round_trip() {
        let env = FakeEnvironment::new().with_var("FOO", "bar");
        assert_eq!(env.var("FOO").unwrap(), "bar");
        assert!(env.var("BAZ").is_none());
    }

    #[test]
    fn fake_env_home() {
        let env = FakeEnvironment::new().with_home("/Users/test");
        assert_eq!(env.home_dir().unwrap(), PathBuf::from("/Users/test"));
    }

    #[test]
    fn fake_env_which_skipping_finds_in_first_matching_dir() {
        let env = FakeEnvironment::new()
            .with_path_entry("/usr/local/bin", ["claude"])
            .with_path_entry("/opt/our-shim", ["claude"]);
        let real = env
            .which_skipping("claude", std::path::Path::new("/opt/our-shim"))
            .unwrap();
        assert_eq!(real, PathBuf::from("/usr/local/bin/claude"));
    }

    #[test]
    fn fake_env_which_skipping_skips_correctly() {
        let env = FakeEnvironment::new().with_path_entry("/opt/our-shim", ["claude"]);
        // Only entry IS the skipped one; should return None.
        let result = env.which_skipping("claude", std::path::Path::new("/opt/our-shim"));
        assert!(result.is_none());
    }

    #[test]
    fn fake_env_which_returns_none_for_missing_binary() {
        let env = FakeEnvironment::new().with_path_entry("/usr/bin", ["other"]);
        assert!(
            env.which_skipping("claude", std::path::Path::new("/nope"))
                .is_none()
        );
    }
}
