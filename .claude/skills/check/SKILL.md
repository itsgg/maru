---
name: check
description: Run the full local quality gate (fmt, clippy, nextest, doc-tests, deny, typos, machete) and report a tight summary. Use before declaring a task done, before opening a PR, or after any non-trivial change.
disable-model-invocation: true
allowed-tools: Bash, Read
---

# /check — full local quality gate

Run every check that gates a PR, in the same order CI runs them. Report a single tight summary with PASS / FAIL per stage.

## Procedure

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo nextest run --workspace --all-features
cargo test --doc --workspace --all-features
cargo deny check
typos
cargo machete
```

If `cargo nextest` is not installed, fall back to `cargo test --workspace --all-features`. If `typos` or `cargo machete` aren't installed, mark them SKIPPED with a one-line install hint, do NOT fail the run.

## Output

```
[PASS] fmt
[PASS] clippy
[FAIL] nextest — 2 failures (see below)
[SKIP] doc-tests (gated on nextest)
[PASS] deny
[PASS] typos
[PASS] machete

Summary: 6 PASS, 1 FAIL, 1 SKIP

<failure details, max 30 lines>
```

If everything passes, end with `Ready to commit.` If anything fails, do NOT propose fixes — just report. The parent agent decides what to do.
