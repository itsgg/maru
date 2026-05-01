//! `maru-adapters` — per-harness implementations of [`HarnessAdapter`].
//!
//! Phase 1 shipped Claude + Codex; Phase 2 adds Gemini.
//!
//! Each adapter is stateless and `Send + Sync`, so a single instance per
//! harness lives in the registry built by [`v1_adapters`] /
//! [`adapter_for`].
#![forbid(unsafe_code)]

pub mod claude;
pub mod codex;
pub mod gemini;

pub use claude::ClaudeAdapter;
pub use codex::CodexAdapter;
pub use gemini::GeminiAdapter;

use maru_core::{HarnessAdapter, HarnessId};

/// Build the full adapter registry: Claude + Codex + Gemini.
///
/// Returns boxed trait objects so the CLI / shim can address all
/// adapters through one slice. Each returned adapter is stateless;
/// callers should reuse the slice for the process lifetime.
#[must_use]
pub fn v1_adapters() -> Vec<Box<dyn HarnessAdapter>> {
    vec![
        Box::new(ClaudeAdapter),
        Box::new(CodexAdapter),
        Box::new(GeminiAdapter),
    ]
}

/// Look up an adapter by [`HarnessId`].
#[must_use]
pub fn adapter_for(harness: HarnessId) -> Option<Box<dyn HarnessAdapter>> {
    match harness {
        HarnessId::Claude => Some(Box::new(ClaudeAdapter)),
        HarnessId::Codex => Some(Box::new(CodexAdapter)),
        HarnessId::Gemini => Some(Box::new(GeminiAdapter)),
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
    fn v1_includes_all_three() {
        let adapters = v1_adapters();
        let ids: Vec<HarnessId> = adapters.iter().map(|a| a.id()).collect();
        assert!(ids.contains(&HarnessId::Claude));
        assert!(ids.contains(&HarnessId::Codex));
        assert!(ids.contains(&HarnessId::Gemini));
        assert_eq!(adapters.len(), 3);
    }

    #[test]
    fn adapter_for_all_known_returns_some() {
        assert!(adapter_for(HarnessId::Claude).is_some());
        assert!(adapter_for(HarnessId::Codex).is_some());
        assert!(adapter_for(HarnessId::Gemini).is_some());
    }
}
