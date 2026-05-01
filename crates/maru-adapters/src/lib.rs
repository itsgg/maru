//! `maru-adapters` — per-harness implementations of [`HarnessAdapter`].
//!
//! Phase 1 ships Claude and Codex per GENESIS §7.1 / §7.2. Gemini lands
//! in Phase 2 (GENESIS §14).
//!
//! Each adapter is stateless and `Send + Sync`, so a single instance per
//! harness lives in the registry built by [`registry`].
#![forbid(unsafe_code)]

pub mod claude;
pub mod codex;

pub use claude::ClaudeAdapter;
pub use codex::CodexAdapter;

use maru_core::{HarnessAdapter, HarnessId};

/// Build the v1 adapter registry: Claude + Codex.
///
/// Returns boxed trait objects so the CLI / shim can address all
/// adapters through one slice. Each returned adapter is stateless;
/// callers should reuse the slice for the process lifetime.
#[must_use]
pub fn v1_adapters() -> Vec<Box<dyn HarnessAdapter>> {
    vec![Box::new(ClaudeAdapter), Box::new(CodexAdapter)]
}

/// Look up an adapter by [`HarnessId`].
///
/// Returns `None` if the harness isn't supported in this build (e.g.
/// Gemini in v1, before the Phase 2 implementation lands).
#[must_use]
pub fn adapter_for(harness: HarnessId) -> Option<Box<dyn HarnessAdapter>> {
    match harness {
        HarnessId::Claude => Some(Box::new(ClaudeAdapter)),
        HarnessId::Codex => Some(Box::new(CodexAdapter)),
        HarnessId::Gemini => None,
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
    use super::{adapter_for, v1_adapters};
    use maru_core::HarnessId;

    #[test]
    fn v1_includes_claude_and_codex() {
        let adapters = v1_adapters();
        let ids: Vec<HarnessId> = adapters.iter().map(|a| a.id()).collect();
        assert!(ids.contains(&HarnessId::Claude));
        assert!(ids.contains(&HarnessId::Codex));
        assert!(!ids.contains(&HarnessId::Gemini));
        assert_eq!(adapters.len(), 2);
    }

    #[test]
    fn adapter_for_known_returns_some() {
        assert!(adapter_for(HarnessId::Claude).is_some());
        assert!(adapter_for(HarnessId::Codex).is_some());
    }

    #[test]
    fn adapter_for_gemini_is_none_in_v1() {
        assert!(adapter_for(HarnessId::Gemini).is_none());
    }
}
