# Contributing to maru

Thanks for your interest. `maru` is a young, opinionated project — read this once, then [GENESIS.md](./GENESIS.md), then [CLAUDE.md](./CLAUDE.md), and you'll be set.

## Ground rules

- **GENESIS.md is normative.** It describes the architecture and constraints. If your change conflicts with the spec, update the spec first (in a separate PR), then the code.
- **Phases gate work.** See GENESIS §14. Don't start phase _N+1_ until phase _N_ is tagged `phase-N-complete`.
- **Issues before code for non-trivial work.** Open an issue describing the problem before opening a PR. Bug fixes and small docs changes can skip this.
- **One logical change per commit and per PR.** PRs squash-merge.

## Development setup

```sh
# clone
git clone https://github.com/<owner>/maru
cd maru

# pin toolchain (read from rust-toolchain.toml automatically)
rustup show

# install dev tools
cargo install cargo-nextest cargo-deny cargo-machete typos-cli
brew install lefthook   # or: go install github.com/evilmartians/lefthook@latest
lefthook install

# verify
cargo build --workspace
cargo nextest run --workspace --all-features
```

## The local quality gate

Before opening a PR (the same checks run in CI):

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo nextest run --workspace --all-features
cargo test --doc --workspace --all-features
cargo deny check
typos
cargo machete
```

In Claude Code, `/check` runs this for you and reports a summary.

`lefthook install` wires `pre-commit` (fmt, clippy, typos, deny), `pre-push` (test, machete), and `commit-msg` (Conventional Commits) hooks. Don't bypass with `--no-verify`; if a hook fails, fix the underlying issue.

## Code standards

### Errors

- Library crates: `thiserror`, one error enum per module-cluster, `#[from]` for transparent conversions.
- Binary crates (`maru-cli`): `anyhow::Result<T>` + `.context("...")` aggressively.
- The shim (`maru-shim`): hand-rolled minimal error type. **No `anyhow` in the shim.**
- Never `.unwrap()` or `.expect()` outside `#[cfg(test)]`.

### Style

- `rustfmt` config in `rustfmt.toml`. `cargo fmt` before commit.
- `clippy` runs with `-D warnings`. Pedantic enabled at workspace level. Specific allows go in `clippy.toml` with a one-line justification per allow.
- Public API doc-comments include at least one `# Examples` block. `#![deny(missing_docs)]` on `maru-core`.

### Dependencies

- The dependency budget is GENESIS §13. **Adding a dep requires a justification line in the PR description.**
- The shim's tighter list (GENESIS §9) is enforced at code-review time and by `cargo tree` checks.
- Forbidden everywhere: `tokio`, `async-std`, `reqwest`. There is no async or networking in this codebase.

### Logging and secrets

- `tracing` + `tracing-subscriber` in `maru-cli`. Not in the shim.
- **Never log credentials.** Files matching `*.credentials.json`, `auth.json`, `oauth_creds.json`, or values matching the GENESIS §8 deny-list patterns must never appear in tracing call sites. CI greps for these.

## Commit format

[Conventional Commits](https://www.conventionalcommits.org/), enforced by the `commit-msg` lefthook:

```
<type>(<scope>)?: <short description>

<optional body>

<optional footer>
```

Allowed types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert`.

Examples:

- `feat(maru-core): add HarnessAdapter trait`
- `fix(shim): handle empty MARU_PROFILE as unset`
- `docs(genesis): clarify Codex storage seed`

Phase-completion commits are tagged `phase-N-complete`.

## Pull requests

PR titles use the same format and become the squash-commit message.

PR description should contain:

1. **What** — one-paragraph summary.
2. **Why** — link to the issue / GENESIS section.
3. **Spec alignment** — for non-trivial changes, run `/genesis-check` (or invoke the `genesis-validator` subagent) and paste the verdict.
4. **Tests** — what was added or modified.
5. **New deps** — none, or a justified list per GENESIS §13.

CI must be green. Reviewer asks for `approve` once a real engineer has read it.

## Architecture decision records (ADRs)

For decisions that affect architecture beyond what GENESIS spells out, write an ADR:

```sh
cp docs/decisions/0000-template.md docs/decisions/NNNN-<short-name>.md
# fill in, link from your PR
```

Use ADRs for: new cross-cutting patterns, dependency swaps, deprecations, anything you'll want to remember the reasoning for in 6 months.

## Filing issues

- **Bugs**: include `cargo --version`, OS + version, `maru --version`, repro steps, expected vs actual.
- **Feature requests**: describe the user problem, not the solution. Reference the GENESIS section if relevant.
- **Security**: don't open a public issue. See [SECURITY.md](./SECURITY.md).

## Architecture

Spec: [GENESIS.md](./GENESIS.md). The ten-thousand-foot view:

- `maru-core` — pure types and traits, no I/O.
- `maru-store` — profile DB, atomic writes, file locking.
- `maru-adapters` — per-harness logic (Claude, Codex, Gemini).
- `maru-activation` — env application + exec.
- `maru-cli` — the `maru` binary.
- `maru-shim` — the hot-path shim binary.

## License

By contributing, you agree your code is released under the project's dual license: Apache-2.0 OR MIT (see [LICENSE-APACHE](./LICENSE-APACHE) and [LICENSE-MIT](./LICENSE-MIT)). You retain copyright of your contributions.
