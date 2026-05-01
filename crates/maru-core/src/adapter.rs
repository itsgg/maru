//! [`HarnessAdapter`] — the per-harness logic interface.
//!
//! GENESIS §6 line 191. Each harness (Claude Code, Codex, Gemini) has one
//! `HarnessAdapter` impl in `maru-adapters`. Adapters are pure: they
//! compute plans, never execute them. The activation pipeline executes
//! plans (env application + exec).

use std::path::Path;

use crate::{
    ActivationPlan, AdapterError, Detection, Environment, HarnessId, ProfileContext, SeedFile,
    ValidationReport,
};

/// One adapter per supported harness. Implementations live in
/// `maru-adapters` (one module per harness).
///
/// Adapters are pure functions over data. Anything that touches the real
/// process env or filesystem goes through [`Environment`] (read) or
/// returns an [`ActivationPlan`] / [`SeedFile`] for the executor to apply
/// (write). The activation hot path NEVER calls a method on this trait
/// that has to do filesystem I/O — that's `validate()`'s job, called by
/// `maru doctor` not by the shim.
///
/// `Send + Sync` so adapters can be stored in a registry shared across
/// threads (the CLI uses one) without `Arc`/`Mutex`.
pub trait HarnessAdapter: Send + Sync {
    /// Which harness this adapter implements.
    fn id(&self) -> HarnessId;

    /// CLI binary names this adapter handles. The shim's argv[0] dispatch
    /// matches against these. Typically a single name (`["claude"]`).
    fn binary_names(&self) -> &'static [&'static str];

    /// Where this harness stores per-profile data, relative to
    /// `$MARU_HOME/profiles/<name>/`. E.g. `Path::new("claude")`.
    fn profile_subdir(&self) -> &'static Path;

    /// Detect the real binary on PATH, skipping the shim install dir.
    /// Used by `maru doctor` to surface incomplete installs.
    fn detect(&self, env: &dyn Environment) -> Detection;

    /// Compute the activation plan for the given context. Pure: no I/O,
    /// no env mutation; the `Environment` reference is read-only.
    ///
    /// # Errors
    ///
    /// Returns [`AdapterError`] for hard adapter failures (e.g. profile
    /// root not absolute). Non-fatal advisories belong in
    /// `plan.diagnostics` instead.
    fn plan(
        &self,
        ctx: &ProfileContext<'_>,
        env: &dyn Environment,
    ) -> Result<ActivationPlan, AdapterError>;

    /// Read-only sanity check on a profile dir, called by `maru doctor`.
    /// Never fails; returns findings as a [`ValidationReport`].
    fn validate(&self, profile_dir: &Path) -> ValidationReport;

    /// Files to write into a fresh profile dir for this harness.
    ///
    /// Called exactly once per (profile, harness) pair, at profile
    /// creation. NEVER on the activation hot path. Default returns
    /// `vec![]` — most adapters need no seed.
    #[must_use]
    fn seed(&self, _profile_dir: &Path) -> Vec<SeedFile> {
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
    use super::HarnessAdapter;
    use crate::{
        ActivationPlan, AdapterError, Detection, Environment, FakeEnvironment, HarnessId,
        ProfileContext, ProfileName, ValidationReport,
    };
    use std::path::{Path, PathBuf};

    /// Minimal in-test adapter to verify the trait surface compiles and
    /// can be exercised end-to-end. The real Claude/Codex/Gemini adapters
    /// land in `maru-adapters`.
    #[derive(Debug)]
    struct TestAdapter;

    impl HarnessAdapter for TestAdapter {
        fn id(&self) -> HarnessId {
            HarnessId::Claude
        }

        fn binary_names(&self) -> &'static [&'static str] {
            &["test-bin"]
        }

        fn profile_subdir(&self) -> &'static Path {
            Path::new("test")
        }

        fn detect(&self, env: &dyn Environment) -> Detection {
            env.which_skipping("test-bin", Path::new("/nonexistent"))
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
            Ok(ActivationPlan::new()
                .with_env("TEST_DIR", ctx.profile_root.join("test").into_os_string()))
        }

        fn validate(&self, _profile_dir: &Path) -> ValidationReport {
            ValidationReport::new()
        }
    }

    #[test]
    fn trait_object_is_send_sync() {
        // If this compiles the trait satisfies Send + Sync.
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<TestAdapter>();
        let _boxed: Box<dyn HarnessAdapter> = Box::new(TestAdapter);
    }

    #[test]
    fn adapter_metadata() {
        let a = TestAdapter;
        assert_eq!(a.id(), HarnessId::Claude);
        assert_eq!(a.binary_names(), &["test-bin"]);
        assert_eq!(a.profile_subdir(), Path::new("test"));
    }

    #[test]
    fn detect_uses_environment() {
        let a = TestAdapter;
        let env = FakeEnvironment::new().with_path_entry("/usr/bin", ["test-bin"]);
        let d = a.detect(&env);
        assert!(d.is_found());
        assert_eq!(d.path(), Some(Path::new("/usr/bin/test-bin")));

        let empty = FakeEnvironment::new();
        assert!(!a.detect(&empty).is_found());
    }

    #[test]
    fn plan_returns_env_for_absolute_profile_root() {
        let a = TestAdapter;
        let name = ProfileName::new("work").unwrap();
        let root = std::env::temp_dir().join("maru-test/profiles/work");
        let home = std::env::temp_dir().join("maru-test/home");
        let ctx = ProfileContext {
            profile_name: &name,
            profile_root: &root,
            harness: HarnessId::Claude,
            home_dir: &home,
            project_pin: None,
        };
        let env = FakeEnvironment::new();
        let plan = a.plan(&ctx, &env).unwrap();
        assert_eq!(plan.env.len(), 1);
        let (k, _v) = plan.env.first().expect("one env entry");
        assert_eq!(k, "TEST_DIR");
    }

    #[test]
    fn plan_rejects_relative_profile_root() {
        let a = TestAdapter;
        let name = ProfileName::new("work").unwrap();
        let root = PathBuf::from("relative");
        let home = PathBuf::from("/Users/gg");
        let ctx = ProfileContext {
            profile_name: &name,
            profile_root: &root,
            harness: HarnessId::Claude,
            home_dir: &home,
            project_pin: None,
        };
        let env = FakeEnvironment::new();
        let err = a.plan(&ctx, &env).unwrap_err();
        match err {
            AdapterError::NotAbsolute { path } => assert_eq!(path, root),
            other => panic!("expected NotAbsolute, got {other:?}"),
        }
    }

    #[test]
    fn default_seed_is_empty() {
        let a = TestAdapter;
        assert!(a.seed(Path::new("/anywhere")).is_empty());
    }
}
