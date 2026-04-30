# Changelog

All notable changes to `maru` will be documented in this file.

The format is based on [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Initial repository scaffold: GENESIS.md (normative spec), README, CLAUDE.md, AGENTS.md, CONTRIBUTING.md, SECURITY.md.
- Cargo workspace skeleton with `[workspace.lints]` (deny-dangerous / warn-safety-relevant / pedantic baseline) and shared `[workspace.dependencies]`.
- Quality gates: `lefthook.yml` (pre-commit, pre-push, commit-msg), `deny.toml`, `clippy.toml`, `rustfmt.toml`, `_typos.toml`, `audit.toml`, `.markdownlint.yaml`, `.prettierrc`.
- CI: `.github/workflows/ci.yml` covering fmt, clippy, typos, deny (matrix-split), audit, MSRV, machete, test (3-OS matrix), bench (Phase-1-gated).
- Claude Code workflow: `.claude/{settings.json, hooks, skills, agents, rules}` including `/check`, `/next`, `/research`, `/spike`, `/genesis-check`, `/cut-phase`, `/autopilot` skills and `genesis-validator`, `rust-test-runner`, `rust-reviewer` subagents.
- Specs and decisions scaffolding: `specs/TEMPLATE.md`, `docs/decisions/0000-template.md`.

### Notes

- No releases yet. First tag will be `phase-1-complete` per GENESIS §14.
