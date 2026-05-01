//! Codex CLI adapter.
//!
//! GENESIS §7.2. Mechanism: `CODEX_HOME` env var redirection. v1 emits
//! no seed (per the autonomous §7.2 correction in commit f259ad8 — the
//! `[auth] storage = "file"` directive in earlier drafts was disconfirmed
//! by Phase 0 spike finding 0.5; auth.json appears to be the macOS default
//! with no required directive). Phase 1 live-smoke nightly CI must verify
//! per-platform behavior on Linux and Windows; if either uses keyring as
//! default, this adapter will reintroduce a per-platform seed.

use std::path::Path;

use maru_core::{
    ActivationPlan, AdapterError, Detection, Diagnostic, Environment, HarnessAdapter, HarnessId,
    ProfileContext, SeedFile, ValidationReport,
};

/// `maru-adapters::codex::CodexAdapter`.
#[derive(Debug, Default)]
pub struct CodexAdapter;

impl CodexAdapter {
    /// New adapter instance. Stateless.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl HarnessAdapter for CodexAdapter {
    fn id(&self) -> HarnessId {
        HarnessId::Codex
    }

    fn binary_names(&self) -> &'static [&'static str] {
        &["codex"]
    }

    fn profile_subdir(&self) -> &'static Path {
        Path::new("codex")
    }

    fn detect(&self, env: &dyn Environment) -> Detection {
        // Skip the shim's own dir; resolved at construction time elsewhere.
        // For now, "skip nothing" since we don't have access to the install
        // dir from the adapter — the shim layer wraps detect with a real
        // skip path. This is fine for `maru doctor`.
        env.which_skipping("codex", Path::new("/.this-path-never-exists"))
            .map_or(Detection::NotFound, Detection::Found)
    }

    fn plan(
        &self,
        ctx: &ProfileContext<'_>,
        _env: &dyn Environment,
    ) -> Result<ActivationPlan, AdapterError> {
        if !ctx.profile_root.is_absolute() {
            return Err(AdapterError::NotAbsolute {
                path: ctx.profile_root.to_path_buf(),
            });
        }
        let codex_home = ctx.profile_root.join("codex");
        Ok(ActivationPlan::new().with_env("CODEX_HOME", codex_home.into_os_string()))
    }

    fn validate(&self, profile_dir: &Path) -> ValidationReport {
        // Profile dir is valid if it does not exist (fresh) or contains
        // config.toml. No content check on config.toml v1 (per §7.2
        // correction; the storage directive is no longer pinned).
        if !profile_dir.exists() {
            return ValidationReport::new();
        }
        if !profile_dir.is_dir() {
            return ValidationReport::new().with(Diagnostic::error(format!(
                "{} exists but is not a directory",
                profile_dir.display()
            )));
        }
        ValidationReport::new()
    }

    fn seed(&self, _profile_dir: &Path) -> Vec<SeedFile> {
        // No seed in v1. See module-level docs.
        Vec::new()
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
    use super::CodexAdapter;
    use maru_core::{
        AdapterError, Detection, FakeEnvironment, HarnessAdapter, HarnessId, ProfileContext,
        ProfileName,
    };
    use std::path::{Path, PathBuf};

    fn ctx<'a>(name: &'a ProfileName, root: &'a Path, home: &'a Path) -> ProfileContext<'a> {
        ProfileContext {
            profile_name: name,
            profile_root: root,
            harness: HarnessId::Codex,
            home_dir: home,
            project_pin: None,
        }
    }

    #[test]
    fn metadata() {
        let a = CodexAdapter;
        assert_eq!(a.id(), HarnessId::Codex);
        assert_eq!(a.binary_names(), &["codex"]);
        assert_eq!(a.profile_subdir(), Path::new("codex"));
    }

    #[test]
    fn plan_emits_codex_home_only() {
        let a = CodexAdapter;
        let name = ProfileName::new("work").unwrap();
        let root = PathBuf::from("/maru/profiles/work");
        let home = PathBuf::from("/Users/test");
        let env = FakeEnvironment::new();
        let plan = a.plan(&ctx(&name, &root, &home), &env).unwrap();

        assert_eq!(plan.env.len(), 1, "Codex emits exactly one env var in v1");
        let (key, value) = plan.env.first().unwrap();
        assert_eq!(key, "CODEX_HOME");
        assert_eq!(value, "/maru/profiles/work/codex");
        assert!(plan.diagnostics.is_empty());
    }

    #[test]
    fn plan_rejects_relative_root() {
        let a = CodexAdapter;
        let name = ProfileName::new("work").unwrap();
        let root = PathBuf::from("relative/root");
        let home = PathBuf::from("/Users/test");
        let env = FakeEnvironment::new();
        let err = a.plan(&ctx(&name, &root, &home), &env).unwrap_err();
        assert!(matches!(err, AdapterError::NotAbsolute { .. }));
    }

    #[test]
    fn detect_finds_binary() {
        let a = CodexAdapter;
        let env = FakeEnvironment::new().with_path_entry("/usr/local/bin", ["codex"]);
        let d = a.detect(&env);
        assert!(d.is_found());
        assert_eq!(d.path(), Some(Path::new("/usr/local/bin/codex")));
    }

    #[test]
    fn detect_returns_notfound() {
        let a = CodexAdapter;
        let env = FakeEnvironment::new();
        assert!(matches!(a.detect(&env), Detection::NotFound));
    }

    #[test]
    fn validate_fresh_dir_is_clean() {
        let a = CodexAdapter;
        let r = a.validate(Path::new("/nonexistent-path-for-test"));
        assert!(r.diagnostics.is_empty());
    }

    #[test]
    fn validate_flags_non_directory() {
        let a = CodexAdapter;
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let r = a.validate(tmp.path());
        assert!(r.has_errors());
    }

    #[test]
    fn seed_is_empty_in_v1() {
        let a = CodexAdapter;
        assert!(a.seed(Path::new("/anywhere")).is_empty());
    }
}
