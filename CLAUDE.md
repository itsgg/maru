# CLAUDE.md — agent conventions for `maru`

This file is your operating manual. Read [GENESIS.md](./GENESIS.md) first — it is the **normative spec**. When the spec and your prior knowledge conflict, the spec wins. When the spec is silent, defer to the rules below, then to idiomatic Rust.

## Project at a glance

- Rust workspace, `~6` crates (see GENESIS §5).
- Edition 2024. MSRV pinned in `Cargo.toml` (`workspace.package.rust-version`).
- Toolchain pinned in `rust-toolchain.toml` (`channel = "1.95.0"`).
- No async, no `tokio`, no networking. The forbidden list in GENESIS §13 is enforced.
- Phase-driven (see GENESIS §14). Phase boundaries auto-merge on green CI; see [ADR 0002](docs/decisions/0002-autonomous-implementation-mode.md).
- Operator runbook: [`docs/notes/handoff.md`](docs/notes/handoff.md).

## Commands

```sh
cargo build --workspace
cargo nextest run --workspace --all-features    # falls back to `cargo test` if nextest absent
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all
cargo deny check
typos                                            # spell-check
cargo machete                                    # unused deps
```

Run the full local gate before declaring a task done:

```sh
cargo fmt --all -- --check && \
cargo clippy --workspace --all-targets --all-features -- -D warnings && \
cargo nextest run --workspace --all-features && \
cargo deny check && typos && cargo machete
```

`lefthook` runs the same checks on `git commit` (fmt, clippy, typos, deny) and on `git push` (test, machete).

## Conventions

### Errors

- Library crates: `thiserror`, one error enum per module-cluster, `#[from]` for transparent conversions.
- Binary crates (`maru-cli`): `anyhow::Result<T>` at function boundaries, `.context("...")` aggressively.
- Shim (`maru-shim`): hand-rolled minimal error enum. **No `anyhow` in the shim.**
- **Never** `.unwrap()` or `.expect()` outside `#[cfg(test)]`. Use `?` or return a typed error.

### Logging

- `tracing` + `tracing-subscriber` in `maru-cli`. **Not in the shim.**
- Default `INFO` for CLI, `WARN` for shim. `MARU_LOG=debug` overrides.
- **Never log secrets**: `.credentials.json`, `auth.json`, `oauth_creds.json`, or any value scrubbed by the §8 deny-list. CI greps tracing call sites for these patterns.

### Style

- `rustfmt` config in `rustfmt.toml` (2024 edition). Run `cargo fmt` before commit (the pre-commit hook also enforces).
- `clippy` runs with `-D warnings`. Pedantic lints enabled at workspace level (see `Cargo.toml [workspace.lints]`); specific allows go in `clippy.toml` with a one-line comment per allow.
- `#![deny(missing_docs)]` on `maru-core`. Public API doc-comments include at least one `# Examples` block.

### Commits

- Conventional Commits: `feat:`, `fix:`, `docs:`, `style:`, `refactor:`, `perf:`, `test:`, `build:`, `ci:`, `chore:`. Scope optional.
- One logical change per commit. PRs squash-merged; PR title becomes the commit message.
- Phase-completion commits tagged `phase-N-complete`.

### Branching

- `main` is always green and releasable.
- Phase work on `phase-N-<short-description>` branches.
- Hotfix branches off `main` only for shipped releases.

## What lives where

- **GENESIS.md** is normative. Anything the spec says explicitly wins over this file.
- **README.md** is for humans landing on the repo. Keep it short.
- **CLAUDE.md** (this file) is conventions for agents.
- **AGENTS.md** is the same content for non-Anthropic agents (kept in sync via a header link).
- **`docs/`** is mdBook source. `docs/notes/<phase-N>-<topic>.md` for reviewer flags. `docs/spike-results.md` for Phase 0 findings.
- **`.claude/`** holds skills, hooks, agents, and shared settings. `settings.local.json` is per-developer and gitignored.

## Doing tasks

1. **Plan-mode for non-trivial work.** If a task touches more than ~3 files or introduces a new public type, propose a plan first. Don't implement until the user agrees.
2. **Implement crates in dependency order:** `maru-core` → `maru-store` → `maru-adapters` → `maru-activation` → `maru-cli` and `maru-shim` in parallel.
3. **At every phase boundary:** open a PR titled `phase-N: ...`, push, and pause for human review.
4. **If a Phase 0 spike check disconfirms an assumption in GENESIS §7, update the spec first** (separate PR), then implement.
5. **Don't add deps not listed in GENESIS §13** without a justification line in the PR.
6. **A feature without docs is not done.** Update `docs/` in the same PR.
7. **The carve-outs in GENESIS §7.1 are upstream bugs, not maru bugs.** When upstream fixes one, delete the corresponding `doctor` warning, the test asserting it, and the row in §7.1 — in that order — in the same PR.

## Implementation workflow

A non-trivial change runs through this loop. Skip steps only when the work is genuinely trivial.

1. **`/research <topic>`** — only if GENESIS is silent or the question is genuinely open. Skip for spec-clear work.
2. **Spec or ADR** — if the change introduces a cross-cutting pattern, copy `specs/TEMPLATE.md` or `docs/decisions/0000-template.md` first.
3. **Issue or TODO** — if not handled by `/autopilot --bootstrap`, capture intent before code.
4. **Implement** — match GENESIS exactly. No deps outside §13 without justification. No `unwrap`/`expect` outside `#[cfg(test)]`.
5. **`/check`** — full local quality gate. Don't proceed past failures; root-cause them.
6. **`genesis-validator`** subagent for non-trivial changes; require `READY` or `READY-WITH-NOTES`.
7. **`rust-reviewer`** subagent before opening a PR; act on `REQUEST-CHANGES` and `BLOCK` verdicts.
8. **Commit** — Conventional Commits, one logical change.
9. **PR** at phase boundaries via `/cut-phase <N>`.

## Skills

| Skill              | When to use                                                             |
| ------------------ | ----------------------------------------------------------------------- |
| `/check`           | Before declaring a task done; before opening a PR.                      |
| `/next`            | Start of session; whenever the user asks "what's next?".                |
| `/research <topic>` | Before designing or implementing when GENESIS is silent.               |
| `/spike <id>`      | Phase 0 verification runner; appends to `docs/spike-results.md`.        |
| `/genesis-check`   | Before tagging a phase or merging a non-trivial PR.                     |
| `/cut-phase <N>`   | Phase boundary: tag + push + PR scaffold.                               |
| `/autopilot`       | Drive the current phase autonomously, one task per call.                |
| `/loop /autopilot` | Hands-off continuous execution; halts on its own at phase boundaries.   |

## Subagents

| Agent              | Purpose                                                                 |
| ------------------ | ----------------------------------------------------------------------- |
| `genesis-validator` | Read-only spec audit. Returns `READY` / `READY-WITH-NOTES` / `BLOCKED`. |
| `rust-reviewer`    | Idiom + safety + perf + architecture review against GENESIS and lints.  |
| `rust-test-runner` | Tight test failure summary (model: haiku).                              |

## Rules

- @.claude/rules/collaboration.md

## When in doubt

Choose the option that best preserves: (a) shim performance, (b) testability of `maru-core`, (c) cross-platform behavior, (d) credential isolation. **In that order.**
