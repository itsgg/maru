//! [`ActivationPlan`] — what an adapter returns from `plan()`.
//!
//! GENESIS §6 line 217. Pure data; executed by `maru-activation`. The
//! `FsOp` enum from earlier drafts has been removed (GENESIS §6 notes
//! "v1.0 contains no `FsOp` variants; activation is env-only and therefore
//! needs no transactional rollback").

use std::ffi::OsString;

use crate::Diagnostic;

/// The result of an adapter's `plan()`.
///
/// Three fields per GENESIS §6:
/// - `env`: env vars to set before exec.
/// - `args_prefix`: extra arguments to inject before user-supplied argv.
/// - `diagnostics`: advisory output (Info / Warn / Error) — the shim
///   aborts on any `Error` per GENESIS §11.
///
/// # Examples
///
/// ```
/// use maru_core::{ActivationPlan, Diagnostic};
/// use std::ffi::OsString;
///
/// let plan = ActivationPlan {
///     env: vec![
///         (OsString::from("CLAUDE_CONFIG_DIR"), OsString::from("/tmp/x")),
///     ],
///     args_prefix: vec![],
///     diagnostics: vec![Diagnostic::info("activated work profile")],
/// };
///
/// assert_eq!(plan.env.len(), 1);
/// ```
#[derive(Debug, Clone, Default)]
pub struct ActivationPlan {
    /// Environment-variable bindings to apply before exec. Order is
    /// preserved; if the same key appears twice, last write wins.
    pub env: Vec<(OsString, OsString)>,

    /// Arguments to inject before any user-supplied argv. Typically empty.
    pub args_prefix: Vec<OsString>,

    /// Advisory output. The shim halts on any `Diagnostic::Error` (GENESIS
    /// §9 algorithm step 5).
    pub diagnostics: Vec<Diagnostic>,
}

impl ActivationPlan {
    /// An empty plan: no env, no prefix args, no diagnostics.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Append an env var binding.
    #[must_use]
    pub fn with_env(mut self, key: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    /// Append a diagnostic.
    #[must_use]
    pub fn with_diagnostic(mut self, d: Diagnostic) -> Self {
        self.diagnostics.push(d);
        self
    }

    /// `true` if any diagnostic has level `Error`. The shim treats this as
    /// a hard fail (exit code 3 per GENESIS §11).
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| matches!(d.level, crate::Level::Error))
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
    use super::ActivationPlan;
    use crate::Diagnostic;

    #[test]
    fn default_is_empty() {
        let p = ActivationPlan::new();
        assert!(p.env.is_empty());
        assert!(p.args_prefix.is_empty());
        assert!(p.diagnostics.is_empty());
        assert!(!p.has_errors());
    }

    #[test]
    fn builder_chains() {
        let p = ActivationPlan::new()
            .with_env("FOO", "bar")
            .with_env("BAZ", "qux")
            .with_diagnostic(Diagnostic::info("hi"));
        assert_eq!(p.env.len(), 2);
        assert_eq!(p.diagnostics.len(), 1);
        assert!(!p.has_errors());
    }

    #[test]
    fn errors_detected() {
        let p = ActivationPlan::new()
            .with_diagnostic(Diagnostic::warn("nuance"))
            .with_diagnostic(Diagnostic::error("nope"));
        assert!(p.has_errors());
    }
}
