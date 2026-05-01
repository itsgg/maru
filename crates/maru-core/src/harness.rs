//! [`HarnessId`] — closed enum of supported AI coding agent CLIs.
//!
//! GENESIS §6 line 165. Adding a new harness means adding a variant here,
//! gating it behind a cargo feature, and writing the corresponding adapter.

use std::fmt;
use std::str::FromStr;

/// The set of harnesses `maru` knows about.
///
/// Closed enum by design: each variant must have an adapter implementation
/// in `maru-adapters`. New harnesses are added in a separate PR with the
/// matching adapter and tests.
///
/// # Examples
///
/// ```
/// use maru_core::HarnessId;
///
/// assert_eq!(HarnessId::Claude.as_str(), "claude");
/// assert_eq!("codex".parse::<HarnessId>().unwrap(), HarnessId::Codex);
/// assert!("aider".parse::<HarnessId>().is_err());
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HarnessId {
    /// Anthropic Claude Code (`claude` CLI).
    Claude,
    /// OpenAI Codex CLI (`codex`).
    Codex,
    /// Google Gemini CLI (`gemini`).
    Gemini,
}

impl HarnessId {
    /// Every variant of [`HarnessId`], in stable declaration order.
    pub const ALL: &'static [Self] = &[Self::Claude, Self::Codex, Self::Gemini];

    /// The canonical lowercase identifier (matches the CLI binary name).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Gemini => "gemini",
        }
    }
}

impl fmt::Display for HarnessId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Returned by [`HarnessId::from_str`] when the input doesn't match any
/// known harness.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("unknown harness {input:?} (expected one of: claude, codex, gemini)")]
pub struct UnknownHarness {
    /// The input string that didn't match.
    pub input: String,
}

impl FromStr for HarnessId {
    type Err = UnknownHarness;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "claude" => Ok(Self::Claude),
            "codex" => Ok(Self::Codex),
            "gemini" => Ok(Self::Gemini),
            other => Err(UnknownHarness {
                input: other.to_owned(),
            }),
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests assert correctness; convenience over discipline"
)]
mod tests {
    use super::{HarnessId, UnknownHarness};

    #[test]
    fn all_variants_listed() {
        assert_eq!(HarnessId::ALL.len(), 3);
        assert!(HarnessId::ALL.contains(&HarnessId::Claude));
        assert!(HarnessId::ALL.contains(&HarnessId::Codex));
        assert!(HarnessId::ALL.contains(&HarnessId::Gemini));
    }

    #[test]
    fn as_str_matches_binary_name() {
        assert_eq!(HarnessId::Claude.as_str(), "claude");
        assert_eq!(HarnessId::Codex.as_str(), "codex");
        assert_eq!(HarnessId::Gemini.as_str(), "gemini");
    }

    #[test]
    fn display_matches_as_str() {
        for id in HarnessId::ALL {
            assert_eq!(id.to_string(), id.as_str());
        }
    }

    #[test]
    fn from_str_round_trips() {
        for id in HarnessId::ALL {
            let parsed: HarnessId = id.as_str().parse().unwrap();
            assert_eq!(parsed, *id);
        }
    }

    #[test]
    fn from_str_rejects_unknown() {
        let err: UnknownHarness = "aider".parse::<HarnessId>().unwrap_err();
        assert_eq!(err.input, "aider");
    }

    #[test]
    fn from_str_is_case_sensitive() {
        // The CLI binary is lowercase. We match exactly.
        assert!("Claude".parse::<HarnessId>().is_err());
        assert!("CLAUDE".parse::<HarnessId>().is_err());
    }

    #[test]
    fn serde_uses_lowercase() {
        let json = serde_json::to_string(&HarnessId::Claude).unwrap();
        assert_eq!(json, "\"claude\"");
        let back: HarnessId = serde_json::from_str("\"codex\"").unwrap();
        assert_eq!(back, HarnessId::Codex);
    }

    #[test]
    fn serde_rejects_unknown() {
        let result: Result<HarnessId, _> = serde_json::from_str("\"aider\"");
        assert!(result.is_err());
    }

    #[test]
    fn const_safety() {
        // ALL is &'static; this proves it's usable in const context.
        const _: &[HarnessId] = HarnessId::ALL;
    }
}
