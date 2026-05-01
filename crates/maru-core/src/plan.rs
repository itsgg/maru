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
    clippy::arithmetic_side_effects,
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

    // ---- proptest: ActivationPlan invariants (GENESIS §15 level 2) ----

    use crate::Level;
    use proptest::collection::vec;
    use proptest::prelude::{Just, ProptestConfig, Strategy, prop_assert_eq, prop_oneof, proptest};
    use std::ffi::OsString;

    fn level_strategy() -> impl Strategy<Value = Level> {
        prop_oneof![Just(Level::Info), Just(Level::Warn), Just(Level::Error),]
    }

    fn diagnostic_strategy() -> impl Strategy<Value = Diagnostic> {
        (level_strategy(), ".{0,32}", proptest::option::of(".{0,32}")).prop_map(
            |(level, message, help)| Diagnostic {
                level,
                message,
                help,
            },
        )
    }

    fn env_pair_strategy() -> impl Strategy<Value = (OsString, OsString)> {
        ("[A-Z_]{1,16}", ".{0,32}").prop_map(|(k, v)| (OsString::from(k), OsString::from(v)))
    }

    fn plan_strategy() -> impl Strategy<Value = ActivationPlan> {
        (
            vec(env_pair_strategy(), 0..8),
            vec(".{0,16}", 0..4).prop_map(|v| v.into_iter().map(OsString::from).collect()),
            vec(diagnostic_strategy(), 0..8),
        )
            .prop_map(|(env, args_prefix, diagnostics)| ActivationPlan {
                env,
                args_prefix,
                diagnostics,
            })
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 64, .. ProptestConfig::default() })]

        /// `has_errors()` is `true` iff at least one diagnostic is `Level::Error`.
        #[test]
        fn prop_has_errors_iff_any_error_diagnostic(plan in plan_strategy()) {
            let any_error = plan.diagnostics.iter().any(|d| d.level == Level::Error);
            prop_assert_eq!(plan.has_errors(), any_error);
        }

        /// Cloning a plan yields a `PartialEq`-equal plan.
        ///
        /// `ActivationPlan` doesn't implement `PartialEq` directly, so we
        /// compare each public field for structural equality.
        #[test]
        fn prop_clone_is_structurally_equal(plan in plan_strategy()) {
            let copy = plan.clone();
            prop_assert_eq!(&plan.env, &copy.env);
            prop_assert_eq!(&plan.args_prefix, &copy.args_prefix);
            prop_assert_eq!(&plan.diagnostics, &copy.diagnostics);
        }

        /// `with_env` is append-only: the resulting `env` length is exactly one
        /// greater than before, and the appended pair is the last entry.
        #[test]
        fn prop_with_env_appends(plan in plan_strategy(), pair in env_pair_strategy()) {
            let before_len = plan.env.len();
            let (key, value) = pair;
            let next = plan.with_env(key.clone(), value.clone());
            prop_assert_eq!(next.env.len(), before_len + 1);
            let last = next.env.last().expect("just appended");
            prop_assert_eq!(&last.0, &key);
            prop_assert_eq!(&last.1, &value);
        }

        /// `with_diagnostic` is append-only: length grows by exactly 1 and the
        /// appended diagnostic is last.
        #[test]
        fn prop_with_diagnostic_appends(plan in plan_strategy(), d in diagnostic_strategy()) {
            let before_len = plan.diagnostics.len();
            let next = plan.with_diagnostic(d.clone());
            prop_assert_eq!(next.diagnostics.len(), before_len + 1);
            prop_assert_eq!(next.diagnostics.last(), Some(&d));
        }
    }
}
