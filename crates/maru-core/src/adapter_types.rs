//! Supporting types for the [`HarnessAdapter`](crate::HarnessAdapter) trait.
//!
//! GENESIS §6 lines 191–198 reference these types. They live here rather
//! than alongside the trait so each can be imported independently and so
//! the file containing the trait stays focused on the trait surface.

use std::path::PathBuf;

use crate::Diagnostic;

/// Result of [`HarnessAdapter::detect`](crate::HarnessAdapter::detect).
///
/// Tells the shim where the real harness binary lives, or that it could not
/// be located on the user's PATH after skipping the shim's own install dir.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Detection {
    /// Found the real binary at this absolute path.
    Found(PathBuf),
    /// No binary by any of the adapter's `binary_names()` is on PATH
    /// (after skipping the shim install dir).
    NotFound,
}

impl Detection {
    /// `true` if a binary was found.
    #[must_use]
    pub const fn is_found(&self) -> bool {
        matches!(self, Self::Found(_))
    }

    /// Borrow the found path, if any.
    #[must_use]
    pub fn path(&self) -> Option<&std::path::Path> {
        match self {
            Self::Found(p) => Some(p),
            Self::NotFound => None,
        }
    }
}

/// Errors an adapter's [`plan()`](crate::HarnessAdapter::plan) can return.
///
/// Distinct from [`Diagnostic`]: a `Diagnostic` is data that flows back to
/// the user via the activation plan; an `AdapterError` is a hard failure
/// the shim treats as a 64+ exit code (per GENESIS §8 / §9).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AdapterError {
    /// The profile root path is not absolute. Adapters require absolute
    /// paths because the harness CLIs do not expand `~` (GENESIS §7.1).
    #[error("profile_root must be absolute, got {path:?}")]
    NotAbsolute {
        /// The non-absolute path the caller provided.
        path: PathBuf,
    },

    /// A required environment value (e.g. `HOME`) was missing.
    #[error("required environment value {name:?} is unset")]
    MissingEnv {
        /// The env var name.
        name: String,
    },

    /// Catch-all for adapter-specific failures. Use sparingly; prefer
    /// adding a typed variant.
    #[error("{message}")]
    Other {
        /// Free-form message.
        message: String,
    },
}

/// Result of [`HarnessAdapter::validate`](crate::HarnessAdapter::validate).
///
/// `validate()` is called by `maru doctor`. It is read-only and never
/// fails — it returns its findings as a list of [`Diagnostic`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ValidationReport {
    /// Diagnostics produced by the validation pass, in emission order.
    pub diagnostics: Vec<Diagnostic>,
}

impl ValidationReport {
    /// An empty report (no findings).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Append one diagnostic.
    #[must_use]
    pub fn with(mut self, diagnostic: Diagnostic) -> Self {
        self.diagnostics.push(diagnostic);
        self
    }

    /// `true` if any diagnostic has level `Error`.
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
    use super::{AdapterError, Detection, ValidationReport};
    use crate::{Diagnostic, Level};
    use std::path::PathBuf;

    #[test]
    fn detection_helpers() {
        let found = Detection::Found(PathBuf::from("/usr/bin/claude"));
        assert!(found.is_found());
        assert_eq!(found.path(), Some(std::path::Path::new("/usr/bin/claude")));

        let nf = Detection::NotFound;
        assert!(!nf.is_found());
        assert!(nf.path().is_none());
    }

    #[test]
    fn adapter_error_display() {
        let e = AdapterError::NotAbsolute {
            path: PathBuf::from("relative"),
        };
        assert!(e.to_string().contains("relative"));

        let e = AdapterError::MissingEnv {
            name: "HOME".to_owned(),
        };
        assert!(e.to_string().contains("HOME"));

        let e = AdapterError::Other {
            message: "boom".to_owned(),
        };
        assert_eq!(e.to_string(), "boom");
    }

    #[test]
    fn validation_report_builder() {
        let r = ValidationReport::new()
            .with(Diagnostic::info("ok"))
            .with(Diagnostic::warn("hmm"));
        assert_eq!(r.diagnostics.len(), 2);
        assert!(!r.has_errors());

        let r = r.with(Diagnostic::error("nope"));
        assert!(r.has_errors());
        assert_eq!(
            r.diagnostics
                .iter()
                .filter(|d| d.level == Level::Error)
                .count(),
            1
        );
    }

    #[test]
    fn validation_report_default_is_empty() {
        let r = ValidationReport::default();
        assert!(r.diagnostics.is_empty());
        assert!(!r.has_errors());
    }
}
