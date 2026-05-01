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
- Phase 0 (#1): spike findings appended to `docs/spike-results.md`; autonomous GENESIS §7.2 correction removing the unverified `[auth] storage = "file"` seed (finding 0.5 disconfirmation).
- Phase 1 (#2): `maru-core` (domain types, `HarnessAdapter` trait, `Environment` trait), `maru-store` (profile DB with atomic writes and file locking), `maru-adapters` (Claude + Codex), `maru-activation` (env application + binary resolution + exec/spawn), `maru-shim` (hot-path argv[0] dispatch), `maru-cli` (clap surface, `install`, `doctor`, `run`), e2e tests against fake harnesses, install/quickstart/limitations docs.
- Phase 2 (#3): `GeminiAdapter` with `GEMINI_CLI_HOME` redirection and `GEMINI_FORCE_ENCRYPTED_FILE_STORAGE` warning; three-harness adapter registry parity; per-adapter docs; `bench-coldstart` CI for shim cold-start.
- Phase 3 (#4): `.maru` project pins walked from `cwd` upward, `profile clone`, `profile export` / `profile import`, credential deny-list scrubber.
- Phase 4 (#5): `dist` (formerly `cargo-dist`) configuration in `dist-workspace.toml`, `release.yml` workflow, `docs.yml` mdBook publish workflow, mdBook scaffolding under `docs/book/`, `docs/notes/phase-4-handoff.md` operator runbook.
- Phase 4 follow-up (#6): removed failing `dist plan` PR trigger from CI.

### Notes

- No releases yet. The first `v0.1.0-alpha.0` tag triggers binary distribution (Homebrew tap, Scoop bucket, winget, signed binaries) per `docs/notes/phase-4-handoff.md`.
- Autonomous implementation mode: `/loop /autopilot` ran phases end-to-end with auto-merge. See [ADR 0002](docs/decisions/0002-autonomous-implementation-mode.md), [ADR 0003](docs/decisions/0003-no-halt-issue-based-mode.md), and [docs/notes/handoff.md](docs/notes/handoff.md) (now archived).
