//! Shared CLI runtime context.

use std::path::{Path, PathBuf};

/// Runtime state shared across subcommand handlers.
pub struct CliContext {
    maru_home: PathBuf,
}

impl CliContext {
    pub const fn new(maru_home: PathBuf) -> Self {
        Self { maru_home }
    }

    pub fn maru_home(&self) -> &Path {
        &self.maru_home
    }

    pub fn profiles_dir(&self) -> PathBuf {
        self.maru_home.join("profiles")
    }

    pub fn profile_root(&self, name: &str) -> PathBuf {
        self.profiles_dir().join(name)
    }

    pub fn shim_install_dir(&self) -> PathBuf {
        self.maru_home.join("bin")
    }
}
