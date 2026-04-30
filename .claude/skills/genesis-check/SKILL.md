---
name: genesis-check
description: Validate that current code (or a proposed change) matches GENESIS.md. Use when the user asks to verify spec alignment, check for drift, or before merging a phase PR.
disable-model-invocation: true
allowed-tools: Bash, Read, Grep, Glob
---

# /genesis-check — spec-vs-code drift detector

GENESIS.md is normative. This skill walks the spec section-by-section and flags places where the code disagrees.

## Procedure

1. Read `GENESIS.md` end-to-end. Note especially:
   - §6 (core types and traits) — every public type and trait listed must exist in `crates/maru-core` with the documented signature.
   - §7 (adapter specifications) — each adapter's `plan()` must emit the env vars listed.
   - §9 (shim algorithm) — the shim's `main` must follow the 8-step algorithm.
   - §10 (profile store) — the on-disk layout must match.
   - §13 (dependency budget) — no dep outside the budget appears in any `Cargo.toml`.

2. For each section, run targeted checks:
   - **§6 types**: `rg -n 'pub (struct|enum|trait|fn) (ProfileName|HarnessId|ProfileContext|HarnessAdapter|ActivationPlan|SeedFile|Diagnostic|Environment)' crates/`
   - **§9 forbidden in shim**: `cargo tree -p maru-shim --depth 2 2>/dev/null | grep -E '(serde|serde_json|toml|tokio|anyhow|tracing|reqwest|clap)'` — output should be empty.
   - **§13 forbidden workspace-wide**: `cargo tree --workspace 2>/dev/null | grep -E '(tokio|async-std|reqwest)'` — output should be empty.
   - **§7.1 Claude env vars**: grep adapter source for both `CLAUDE_CONFIG_DIR` and `CLAUDE_CODE_PLUGIN_CACHE_DIR`.
   - **§14 Phase gates**: `git tag --list 'phase-*-complete'` and verify each tagged phase has met its exit criteria.

3. Produce a report grouped by section, with one line per drift item:
   - `[OK]` — code matches spec
   - `[DRIFT]` — code diverges; cite spec line and code location
   - `[N/A]` — section's code not yet implemented (acceptable per current phase)

4. End with a verdict: `READY`, `READY-WITH-NOTES`, or `BLOCKED`.

## When to invoke

- Before tagging a `phase-N-complete`.
- After a large refactor.
- When a reviewer asks "does this match the spec?"
- During Phase 0 if `docs/spike-results.md` recorded any `disconfirmed` finding.

## Out of scope

This skill does not modify code or the spec. If drift is found, propose a fix path; do not implement it.
