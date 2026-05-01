//! Google Gemini CLI adapter.
//!
//! GENESIS §7.3. Mechanism: `GEMINI_CLI_HOME` env var redirection. The
//! Gemini CLI creates a `.gemini/` subdirectory inside whatever
//! `GEMINI_CLI_HOME` points at, so the per-profile layout is
//! `<profile_root>/gemini/.gemini/{settings.json,oauth_creds.json,...}`.
//!
//! Credential-storage caveat: by default Gemini stores OAuth at
//! `<GEMINI_CLI_HOME>/.gemini/oauth_creds.json`. If the user has set
//! `GEMINI_FORCE_ENCRYPTED_FILE_STORAGE=true`, OAuth tokens go to the
//! OS keychain under a single shared service name `gemini-cli-oauth`,
//! which breaks profile isolation. We emit a `Diagnostic::Warn` if we
//! observe that env var.

use std::path::Path;

use maru_core::{
    ActivationPlan, AdapterError, Detection, Diagnostic, Environment, HarnessAdapter, HarnessId,
    ProfileContext, ValidationReport,
};

/// `maru-adapters::gemini::GeminiAdapter`.
#[derive(Debug, Default)]
pub struct GeminiAdapter;

impl GeminiAdapter {
    /// New adapter instance. Stateless.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl HarnessAdapter for GeminiAdapter {
    fn id(&self) -> HarnessId {
        HarnessId::Gemini
    }

    fn binary_names(&self) -> &'static [&'static str] {
        &["gemini"]
    }

    fn profile_subdir(&self) -> &'static Path {
        Path::new("gemini")
    }

    fn detect(&self, env: &dyn Environment) -> Detection {
        env.which_skipping("gemini", Path::new("/.this-path-never-exists"))
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

        let mut plan = ActivationPlan::new().with_env(
            "GEMINI_CLI_HOME",
            ctx.profile_root.join("gemini").into_os_string(),
        );

        // Keychain caveat: warn if the user enabled the encrypted-file
        // storage path that defeats per-profile isolation.
        if env
            .var("GEMINI_FORCE_ENCRYPTED_FILE_STORAGE")
            .is_some_and(|v| v == "true" || v == "1")
        {
            plan = plan.with_diagnostic(
                Diagnostic::warn(
                    "GEMINI_FORCE_ENCRYPTED_FILE_STORAGE=true is set; OAuth tokens will be stored in the OS keychain under the shared service name `gemini-cli-oauth`, defeating per-profile isolation",
                )
                .with_help("unset the env var (or set it to false) to keep OAuth tokens inside the per-profile dir"),
            );
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
        ValidationReport::new()
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
    use super::GeminiAdapter;
    use maru_core::{
        AdapterError, Detection, FakeEnvironment, HarnessAdapter, HarnessId, Level, ProfileContext,
        ProfileName,
    };
    use std::path::{Path, PathBuf};

    fn ctx<'a>(name: &'a ProfileName, root: &'a Path, home: &'a Path) -> ProfileContext<'a> {
        ProfileContext {
            profile_name: name,
            profile_root: root,
            harness: HarnessId::Gemini,
            home_dir: home,
            project_pin: None,
        }
    }

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
        let a = GeminiAdapter;
        assert_eq!(a.id(), HarnessId::Gemini);
        assert_eq!(a.binary_names(), &["gemini"]);
        assert_eq!(a.profile_subdir(), Path::new("gemini"));
    }

    #[test]
    fn plan_emits_gemini_cli_home_only() {
        let a = GeminiAdapter;
        let name = ProfileName::new("work").unwrap();
        let root = abs(&["maru", "profiles", "work"]);
        let home = abs(&["Users", "test"]);
        let env = FakeEnvironment::new();
        let plan = a.plan(&ctx(&name, &root, &home), &env).unwrap();

        assert_eq!(plan.env.len(), 1);
        let (key, value) = plan.env.first().unwrap();
        assert_eq!(key, "GEMINI_CLI_HOME");
        assert_eq!(value, root.join("gemini").as_os_str());
        assert!(plan.diagnostics.is_empty());
    }

    #[test]
    fn plan_warns_on_keychain_force_env() {
        let a = GeminiAdapter;
        let name = ProfileName::new("work").unwrap();
        let root = abs(&["maru", "profiles", "work"]);
        let home = abs(&["Users", "test"]);
        let env = FakeEnvironment::new().with_var("GEMINI_FORCE_ENCRYPTED_FILE_STORAGE", "true");
        let plan = a.plan(&ctx(&name, &root, &home), &env).unwrap();

        assert_eq!(plan.diagnostics.len(), 1);
        let d = plan.diagnostics.first().unwrap();
        assert_eq!(d.level, Level::Warn);
        assert!(d.help.is_some());
    }

    #[test]
    fn plan_warn_only_on_truthy_value() {
        let a = GeminiAdapter;
        let name = ProfileName::new("work").unwrap();
        let root = abs(&["maru", "profiles", "work"]);
        let home = abs(&["Users", "test"]);
        // false / unset / empty: no warning.
        for v in ["", "false", "0", "no"] {
            let env = FakeEnvironment::new().with_var("GEMINI_FORCE_ENCRYPTED_FILE_STORAGE", v);
            let plan = a.plan(&ctx(&name, &root, &home), &env).unwrap();
            assert!(
                plan.diagnostics.is_empty(),
                "{v:?} should not produce a warning"
            );
        }
    }

    #[test]
    fn plan_rejects_relative_root() {
        let a = GeminiAdapter;
        let name = ProfileName::new("work").unwrap();
        let root = PathBuf::from("relative");
        let home = abs(&["Users", "test"]);
        let env = FakeEnvironment::new();
        let err = a.plan(&ctx(&name, &root, &home), &env).unwrap_err();
        assert!(matches!(err, AdapterError::NotAbsolute { .. }));
    }

    #[test]
    fn detect_finds_binary() {
        let a = GeminiAdapter;
        let env = FakeEnvironment::new().with_path_entry("/usr/local/bin", ["gemini"]);
        let d = a.detect(&env);
        assert!(d.is_found());
    }

    #[test]
    fn detect_returns_notfound() {
        let a = GeminiAdapter;
        let env = FakeEnvironment::new();
        assert!(matches!(a.detect(&env), Detection::NotFound));
    }

    #[test]
    fn validate_fresh_dir_is_clean() {
        let a = GeminiAdapter;
        let r = a.validate(Path::new("/nonexistent-path-for-test"));
        assert!(r.diagnostics.is_empty());
    }

    #[test]
    fn validate_flags_non_directory() {
        let a = GeminiAdapter;
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let r = a.validate(tmp.path());
        assert!(r.has_errors());
    }
}
