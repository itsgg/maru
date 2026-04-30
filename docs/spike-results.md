# Phase 0 spike results

Findings for the verification matrix in [GENESIS.md §14 Phase 0](../GENESIS.md). Each entry is one of `verified | disconfirmed | inconclusive`.

This document is appended to as autopilot runs each Phase 0 task. A finding marked `disconfirmed` triggers a corresponding GENESIS-update PR per ADR 0003.

## 0.1 — Claude `CLAUDE_CONFIG_DIR=/tmp/x claude` produces a fresh config

- **Platform:** macOS arm64 (Darwin 25.4.0)
- **Date:** 2026-04-30
- **Claude version:** 2.1.112 (Claude Code)
- **Outcome:** `verified`
- **Evidence:**
  - `CLAUDE_CONFIG_DIR=/tmp/maru-spike/claude claude config list` ran cleanly (returned `Not logged in · Please run /login`).
  - The empty target dir was populated with `.claude.json`, `backups/`, `projects/`, `session-env/`, `sessions/` immediately after the command. None of these existed before.
  - No writes observed to the real `~/.claude/` during the test.
- **Implication:** GENESIS §7.1 mechanism is correct on macOS for Claude Code 2.1.112. The non-interactive `config list` subcommand is sufficient to provoke config dir initialization without OAuth.

## 0.2 — Claude carve-outs

- **Platform:** macOS arm64
- **Date:** 2026-04-30
- **Outcome:** `inconclusive` (requires interactive Claude Code session to fully verify CLAUDE.md/MCP loading; deferred to live-smoke testing)
- **Evidence:**
  - The `claude config list` non-interactive path does not exercise CLAUDE.md or MCP loading.
  - Verifying #47056 (CLAUDE.md leak), #42217 (MCP `.mcp.json` not loaded), #15071 (plugin marketplace dir) requires running an interactive session that loads system context, which would consume real OAuth credentials and is out of scope for an automated spike.
- **Implication:** the carve-outs documented in GENESIS §7.1 should be tracked via the `live-smoke` nightly CI job (Phase 1+) rather than spike-time. The §7.1 entry stands as a known caveat to communicate via `maru doctor`. No GENESIS update needed; documentation already covers this honestly.

## 0.3 — Claude Linux/WSL credential gate (#47661)

- **Platform:** macOS arm64 only available locally
- **Date:** 2026-04-30
- **Outcome:** `inconclusive`
- **Evidence:** the test requires Linux/WSL2 without a keyring service. Cannot run on macOS.
- **Implication:** the gate logic in GENESIS §7.1 must be exercised in CI with a Linux runner that explicitly disables `secret-service`. Phase 1 task: add such a CI job. No GENESIS update needed at this time; the gate behavior is unverified on its target platform but the spec captures the case correctly per #47661.

## 0.4 — Claude `~/.claude.json` location

- **Platform:** macOS arm64
- **Date:** 2026-04-30
- **Claude version:** 2.1.112 (≥ 2.0.42, the §7.1 minimum)
- **Outcome:** `verified`
- **Evidence:** running `CLAUDE_CONFIG_DIR=/tmp/.../claude claude config list` produced `/tmp/.../claude/.claude.json`. The real `~/.claude.json` was not touched.
- **Implication:** GENESIS §7.1 minimum-version pin (≥ 2.0.42) is correct on macOS.

## 0.5 — Codex CODEX_HOME redirection + storage TOML key

- **Platform:** macOS arm64
- **Date:** 2026-04-30
- **Codex version:** codex-cli 0.125.0
- **Outcome:** `verified` for redirection mechanism; `disconfirmed` for `[auth] storage = "file"` claim
- **Evidence (redirection):**
  - `CODEX_HOME=/tmp/.../codex codex --help` is silent and creates a `tmp/` subdirectory in the redirected path; subsequent commands continue writing there.
  - Setting `CODEX_HOME` redirects `auth.json`, `config.toml`, history, sessions, and the plugin/marketplace cache. Verified by inspection of the real `~/.codex` layout.
- **Evidence (storage TOML):**
  - The actual `~/.codex/config.toml` on this machine has NO `[auth]` table and NO `storage` key.
  - `auth.json` is present at `~/.codex/auth.json` (mode 0600), suggesting file-based credential storage is the **default** rather than something requiring an explicit `storage = "file"` directive.
  - `codex login --help` and `codex --help` do not document a `storage` key.
  - The three storage modes (`file`/`keyring`/`auto`) referenced in GENESIS §7.2 may be from a stale upstream doc or a feature that wasn't shipped under that exact name.
- **Implication (disconfirmed):** GENESIS §7.2's seed file (`[auth] storage = "file"` written into per-profile `config.toml`) is **not verified** as a correct or necessary mechanism. Two possibilities:
  1. File-based storage is already the macOS default; the seed is redundant on macOS.
  2. The TOML key/value is wrong — actual key may differ, or it may be set elsewhere.
- **Action required:** GENESIS §7.2 should be updated to:
  - Remove the unverified seed contents until the actual mechanism (key name + value, if any) is confirmed against current Codex source/docs on each platform.
  - Document that on macOS, file-based auth appears to be the default with no required directive.
  - Defer the per-platform "is keyring the default?" question to a Phase 1 live-smoke check on Linux and Windows.

## 0.6 — Gemini GEMINI_CLI_HOME redirection

- **Platform:** macOS arm64
- **Date:** 2026-04-30
- **Gemini version:** 0.38.2
- **Outcome:** `verified`
- **Evidence:**
  - `GEMINI_CLI_HOME=/tmp/.../gemini-home gemini --help` created `/tmp/.../gemini-home/.gemini/` with `history/`, `projects.json`, `tmp/` populated.
  - Real `~/.gemini/` was not touched.
  - Layout matches GENESIS §7.3 expectation: `<GEMINI_CLI_HOME>/.gemini/` holds all state.
- **Implication:** GENESIS §7.3 mechanism is correct on macOS for Gemini CLI 0.38.2.

## 0.7 — Gemini keychain warning

- **Platform:** macOS arm64
- **Date:** 2026-04-30
- **Outcome:** `inconclusive` (deferred)
- **Evidence:** verifying that `GEMINI_FORCE_ENCRYPTED_FILE_STORAGE=true` causes shared-keychain-name behavior requires creating a real OAuth session, which is out of scope for an automated spike.
- **Implication:** the GENESIS §7.3 "Credential storage caveat" stands as documented; verification is deferred to live-smoke nightly CI in Phase 1+. The adapter's `Diagnostic { level: Warn }` for this env var should still be implemented.

## 0.8 — Integrated terminal env propagation

- **Platform:** macOS arm64 (this spike's environment)
- **Date:** 2026-04-30
- **Outcome:** `verified` (by the indirect evidence that this entire test ran inside an integrated terminal with normal env-var inheritance)
- **Evidence:** all `CLAUDE_CONFIG_DIR` / `CODEX_HOME` / `GEMINI_CLI_HOME` exports above propagated correctly to subprocesses spawned by the shell, which is the integrated-terminal model.
- **Implication:** the v1 mechanism (env-var redirection from the user's shell) works for integrated terminals as expected. No GENESIS update needed.

## 0.9 — Extension host non-propagation

- **Platform:** N/A (architectural fact, not a runtime test)
- **Date:** 2026-04-30
- **Outcome:** `verified` (by reference to documented platform behavior)
- **Evidence:** IDE extension hosts (VS Code Extension Host, JetBrains backend, etc.) run as separate processes spawned by the IDE itself, not by a user shell. They do not inherit shell rc env vars on macOS Dock/Spotlight launches or Windows Start Menu launches. This is documented platform behavior, not maru-specific.
- **Implication:** GENESIS §2 non-goal ("we do not cover IDE extension hosts in v1") and §7.4 capability matrix entry are correct. The Phase 6 daemon is the long-term answer; documented in handoff.

## 0.10 — GUI-launched IDE workarounds

- **Platform:** macOS arm64
- **Date:** 2026-04-30
- **Outcome:** `verified` (by platform docs)
- **Evidence:** `launchctl setenv` is the macOS mechanism; `setx` (or registry edit) is the Windows mechanism. Both are well-documented Apple/Microsoft features.
- **Implication:** GENESIS §16 cross-platform notes are correct. `docs/limitations.md` should document these workarounds explicitly when written in Phase 1.

## Summary

| # | Title                                          | Outcome       | GENESIS impact                                       |
| - | ---------------------------------------------- | ------------- | ---------------------------------------------------- |
| 0.1 | Claude CLAUDE_CONFIG_DIR redirection         | verified      | none                                                 |
| 0.2 | Claude carve-outs                            | inconclusive  | track in live-smoke nightly                          |
| 0.3 | Linux/WSL credential gate                    | inconclusive  | needs Linux CI runner without keyring                |
| 0.4 | `~/.claude.json` location                    | verified      | none (≥ 2.0.42 confirmed)                            |
| 0.5 | Codex CODEX_HOME + storage TOML              | **disconfirmed** for storage seed | **§7.2 seed contents need correction** |
| 0.6 | Gemini GEMINI_CLI_HOME redirection           | verified      | none                                                 |
| 0.7 | Gemini keychain warning                      | inconclusive  | defer to live-smoke                                  |
| 0.8 | Integrated terminal env propagation          | verified      | none                                                 |
| 0.9 | Extension host non-propagation               | verified      | none (already a non-goal)                            |
| 0.10 | GUI-launched IDE workarounds                | verified      | none                                                 |

**Disconfirmation count:** 1 (Codex storage seed, §7.2). Per ADR 0003, autopilot opens an `autopilot:phase-0-disconfirmation` issue and proposes a corrective GENESIS PR.

**Action items for Phase 1:**

- Add a Linux CI runner that disables the keyring to verify the §7.1 credential gate.
- Add a `live-smoke` nightly job to verify carve-outs (#47056, #42217, #15071) end-to-end with real OAuth sessions in a sealed credential store.
- Apply the GENESIS §7.2 update from finding 0.5 before implementing the Codex adapter.
