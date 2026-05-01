//! [`Diagnostic`] — structured advisory output from adapters.
//!
//! GENESIS §6 line 232. The shim turns `Diagnostic::Error` into an exit-3
//! abort (per GENESIS §9 algorithm step 5). `Warn` prints to stderr and
//! continues. `Info` is suppressed unless `MARU_LOG=debug`.

/// Severity of a [`Diagnostic`].
///
/// Order is meaningful: `Error > Warn > Info`. The shim halts on `Error`,
/// prints `Warn` to stderr, and suppresses `Info` unless verbose logging
/// is enabled.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    /// Informational. Suppressed unless `MARU_LOG=debug`.
    Info,
    /// Warning. Printed to stderr; activation continues.
    Warn,
    /// Error. Activation aborts with exit code 3 (per GENESIS §9 / §11).
    Error,
}

/// One diagnostic emitted by an adapter's `plan()` or `validate()`.
///
/// GENESIS §6 line 220.
///
/// # Examples
///
/// ```
/// use maru_core::{Diagnostic, Level};
///
/// let d = Diagnostic::error("Linux/WSL credential gate tripped")
///     .with_help("Move ~/.claude/.credentials.json aside or delete it.");
/// assert_eq!(d.level, Level::Error);
/// assert!(d.help.is_some());
/// ```
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Diagnostic {
    /// Severity.
    pub level: Level,
    /// Human-readable summary.
    pub message: String,
    /// Optional help text — typically next-step instructions for the user.
    pub help: Option<String>,
}

impl Diagnostic {
    /// Construct an `Info`-level diagnostic.
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            level: Level::Info,
            message: message.into(),
            help: None,
        }
    }

    /// Construct a `Warn`-level diagnostic.
    pub fn warn(message: impl Into<String>) -> Self {
        Self {
            level: Level::Warn,
            message: message.into(),
            help: None,
        }
    }

    /// Construct an `Error`-level diagnostic.
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            level: Level::Error,
            message: message.into(),
            help: None,
        }
    }

    /// Attach help text. Replaces any prior help.
    #[must_use]
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
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
    use super::{Diagnostic, Level};

    #[test]
    fn level_ordering() {
        assert!(Level::Info < Level::Warn);
        assert!(Level::Warn < Level::Error);
    }

    #[test]
    fn builders() {
        let d = Diagnostic::info("hello");
        assert_eq!(d.level, Level::Info);
        assert_eq!(d.message, "hello");
        assert!(d.help.is_none());

        let d = Diagnostic::warn("careful").with_help("do this");
        assert_eq!(d.level, Level::Warn);
        assert_eq!(d.help.as_deref(), Some("do this"));

        let d = Diagnostic::error("nope");
        assert_eq!(d.level, Level::Error);
    }

    #[test]
    fn with_help_replaces() {
        let d = Diagnostic::warn("x").with_help("first").with_help("second");
        assert_eq!(d.help.as_deref(), Some("second"));
    }

    #[test]
    fn serde_round_trip() {
        let d = Diagnostic::error("boom").with_help("try harder");
        let json = serde_json::to_string(&d).unwrap();
        let back: Diagnostic = serde_json::from_str(&json).unwrap();
        assert_eq!(back, d);
        assert!(json.contains("\"error\""));
    }
}
