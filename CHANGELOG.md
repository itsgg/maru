# Changelog

All notable changes to `maru` will be documented in this file.

The format is based on [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `maru update` self-update subcommand (Phase 4 task 4.7). Queries GitHub Releases API for the latest tag, compares against the compiled-in version, and atomically replaces the running binary via `self_replace`. `--check` prints the comparison and exits without side effects. Implementation: `crates/maru-cli/src/cmd/update.rs`. Adds `ureq` and `self-replace` workspace deps with GENESIS §13 justifications.
- `.github/workflows/live-smoke.yml`: nightly job (04:00 UTC) running `scripts/live-smoke.sh` to exercise real `claude` / `codex` / `gemini` binaries against `CLAUDE_CONFIG_DIR` / `CODEX_HOME` / `GEMINI_CLI_HOME` redirections plus the Linux/WSL Claude credential gate. Gated on the `LIVE_SMOKE_ENABLED` repo Variable so it stays a no-op until the runner is provisioned. On failure, opens a tracking issue labelled `autopilot:live-smoke-failure`. Implements GENESIS §15 level 9; covers Phase 0 spike checks 0.2, 0.3, 0.7 deferred under `docs/spike-results.md`.
- Initial repository scaffold: GENESIS.md (normative spec), README, CLAUDE.md, AGENTS.md, CONTRIBUTING.md, SECURITY.md.
- Cargo workspace skeleton with `[workspace.lints]` (deny-dangerous / warn-safety-relevant / pedantic baseline) and shared `[workspace.dependencies]`.
- Quality gates: `lefthook.yml` (pre-commit, pre-push, commit-msg), `deny.toml`, `clippy.toml`, `rustfmt.toml`, `_typos.toml`, `audit.toml`, `.markdownlint.yaml`, `.prettierrc`.
- CI: `.github/workflows/ci.yml` covering fmt, clippy, typos, deny (matrix-split), audit, MSRV, machete, test (3-OS matrix), bench (Phase-1-gated).
- Claude Code workflow: `.claude/{settings.json, hooks, skills, agents, rules}` including `/check`, `/next`, `/research`, `/spike`, `/genesis-check`, `/cut-phase`, `/autopilot` skills and `genesis-validator`, `rust-test-runner`, `rust-reviewer` subagents.
- Specs and decisions scaffolding: `specs/TEMPLATE.md`, `docs/decisions/0000-template.md`.

### Notes

- No releases yet. First tag will be `phase-1-complete` per GENESIS §14.
- Autonomous implementation mode: `/loop /autopilot` runs phases end-to-end with auto-merge. See [ADR 0002](docs/decisions/0002-autonomous-implementation-mode.md) and [docs/notes/handoff.md](docs/notes/handoff.md).
