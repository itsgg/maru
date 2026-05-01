# Changelog

All notable changes to `maru` will be documented in this file.

The format is based on [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-alpha.6] - 2026-05-01

### Added

- **Per-profile Claude OAuth isolation via `CLAUDE_CODE_OAUTH_TOKEN`.** Claude Code's Keychain entry is not reliably partitioned per `CLAUDE_CONFIG_DIR` across 2.1.x — logging out under one profile clears credentials shared with another. Fix: store a per-profile OAuth token at `<profile>/claude/oauth_token` (mode 0600); the Claude adapter exports it as `CLAUDE_CODE_OAUTH_TOKEN` at activation. Per Claude Code's auth precedence (step 5), env-var tokens win over Keychain — Claude Code never consults the shared entry when the file is present, giving each profile real credential isolation.
- **`maru profile login <name>`** — wraps `claude setup-token`, captures the OAuth token from stdout, writes it to `<profile>/claude/oauth_token`. Use `--stdin` to pipe a pre-generated token (`claude setup-token | maru profile login work --stdin`).
- `oauth_token` added to the GENESIS §8 credential deny-list — never copied by `maru profile clone` / `export` / `import`.

### Fixed

- `write_token_file` now `chmod`s to 0600 explicitly after writing. `OpenOptionsExt::mode` only applies on creation; truncating an existing 0644 file kept its old mode. The 0600 enforcement is now covered by an e2e test that pre-creates the file at 0644 and asserts the post-login mode.

## [0.1.0-alpha.4] - 2026-05-01

### Added

- Single-step install across all channels via `scripts/install.{sh,ps1}` wrappers that drive both per-binary `dist` installers and run `maru install`. Hosted at `raw.githubusercontent.com/itsgg/maru/main/scripts/install.{sh,ps1}` so the URL doesn't change per release.
- Unified `maru` Homebrew formula at `itsgg/homebrew-maru/Formula/maru.rb` — one `brew install itsgg/maru/maru` installs both binaries (the shim arrives via Homebrew's `resource` feature).

### Fixed

- `maru update --check` always reported "no releases published yet" because it queried `https://api.github.com/repos/itsgg/maru/releases/latest`, which excludes prereleases. Switched to the list endpoint (`/releases?per_page=1`); self-update works against alpha tags now. Tests updated.
- `maru version` printed `maru-cli` (the package name) instead of `maru` (the binary name). Hard-coded `"maru"` for the version command's `name` field; e2e test updated.
- `scripts/install.{sh,ps1}` had the same `/releases/latest` 404 bug as `maru update`. Both now query the list endpoint, parse the first tag, and build the download URL from the resolved tag.
- README + `docs/book/src/introduction.md` Status sections claimed "v0.1.0-alpha.0 will trigger binary distribution" — already shipped since alpha.0; refreshed.
- Brief experiment with a `post_install` Gatekeeper pre-warm in the brew formula was reverted: it moved the hang from `maru install` to `brew install` with no explanation, which was strictly worse UX. Caveats and `docs/install.md` now document the macOS first-run latency clearly.

### Documentation

- `docs/install.md` rewritten for single-step install + a "Manual install" fallback for users who want the per-binary `dist` installers directly. Top-of-page heads-up for macOS users about first-run `syspolicy` latency for unsigned binaries.
- `docs/notes/phase-4-handoff.md` documents the per-release process for maintaining the unified brew formula by hand until either `HOMEBREW_TAP_TOKEN` lets dist auto-publish or someone writes a generator.

## [0.1.0-alpha.3] - 2026-05-01

### Fixed

- `$MARU_HOME` default path. The implementation used `directories::ProjectDirs::from("dev","maru","maru")`, which produces `~/Library/Application Support/dev.maru.maru/` on macOS (reverse-DNS style). GENESIS §3 specifies `~/Library/Application Support/maru/`. Both call sites (`crates/maru-cli/src/main.rs` and `crates/maru-shim/src/config.rs`) now use `BaseDirs::data_local_dir().join("maru")`, which gives the spec-correct path on every platform.

## [0.1.0-alpha.2] - 2026-05-01

### Fixed

- `Cargo.toml` `workspace.package.repository` was `https://github.com/gg/maru` (no `itsgg`). `dist` propagates this into the generated Homebrew formulas; `maru-cli.rb` and `maru-shim.rb` shipped in alpha.0 and alpha.1 contained URLs that 404'd. `brew install` would have failed even with `HOMEBREW_TAP_TOKEN` set. Corrected the URL.

## [0.1.0-alpha.1] - 2026-05-01

### Fixed

- Install instructions in README and `docs/install.md` were inaccurate in four ways:
  - Wrong installer filenames (`maru-installer.sh` → actual `maru-cli-installer.sh`, etc.).
  - Wrong brew formula name (`maru` → actual `maru-cli` plus `maru-shim` per dist's per-package output).
  - The two-step install requirement was hidden (`maru` ships as two binary crates; both must be on PATH).
  - Scoop and winget sections claimed working installs but neither is wired in (Scoop bucket is empty; winget isn't auto-submitted by dist 0.31.0).

## [0.1.0-alpha.0] - 2026-05-01

### Added

- First `dist`-managed release. Five platforms: x86_64+aarch64 linux-gnu, x86_64+aarch64 darwin, x86_64 windows-msvc.
- `maru update` self-update subcommand (Phase 4 task 4.7). Queries GitHub Releases API for the latest tag, compares against the compiled-in version, and atomically replaces the running binary via `self_replace`. `--check` prints the comparison and exits without side effects. Implementation: `crates/maru-cli/src/cmd/update.rs`. Adds `ureq` and `self-replace` workspace deps with GENESIS §13 justifications.
- `.github/workflows/live-smoke.yml`: nightly job (04:00 UTC) running `scripts/live-smoke.sh` to exercise real `claude` / `codex` / `gemini` binaries against `CLAUDE_CONFIG_DIR` / `CODEX_HOME` / `GEMINI_CLI_HOME` redirections plus the Linux/WSL Claude credential gate. Gated on the `LIVE_SMOKE_ENABLED` repo Variable. On failure, opens a tracking issue labelled `autopilot:live-smoke-failure`. Implements GENESIS §15 level 9; covers Phase 0 spike checks 0.2, 0.3, 0.7 deferred under `docs/spike-results.md`.
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

- Released as a prerelease for the entire `0.1.0-alpha.*` chain to validate the dist pipeline. Each subsequent alpha bumps for a specific user-blocking fix; see entries above.
- Code-signing and notarization (macOS) plus Windows code-signing are user-blocked on Apple Developer / DigiCert credentials. Until the secrets are provisioned, releases ship unsigned and macOS users see a one-time Gatekeeper online verification on first run (30 s – 2 min).
- Scoop and winget channels are reserved (`itsgg/scoop-maru`, `itsgg.maru`) but not yet wired up — dist 0.31.0 doesn't auto-publish either; both require per-release manifest work.
- Autonomous implementation mode: `/loop /autopilot` ran phases 0–4 end-to-end with auto-merge. See [ADR 0002](docs/decisions/0002-autonomous-implementation-mode.md), [ADR 0003](docs/decisions/0003-no-halt-issue-based-mode.md), and [docs/notes/handoff.md](docs/notes/handoff.md) (now archived).
