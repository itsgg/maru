# AGENTS.md

This is a copy of [CLAUDE.md](./CLAUDE.md) for non-Anthropic agents (Codex CLI, Gemini CLI, Aider, etc.). The two files are kept in sync; if you find drift, treat CLAUDE.md as canonical and open a PR to align AGENTS.md.

See [CLAUDE.md](./CLAUDE.md) for:

- Project at a glance
- Commands
- Conventions (errors, logging, style, commits, branching)
- What lives where
- Doing tasks
- When in doubt

The only differences:

- The `/spike`, `/genesis-check`, `/cut-phase` skills and the `genesis-validator`, `rust-test-runner` subagents are Claude Code-specific. Other agents must invoke their equivalents manually.
- `.claude/hooks/` runs only inside Claude Code. Other agents are expected to run the local gate manually before declaring a task done:

  ```sh
  cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets --all-features -- -D warnings && \
  cargo nextest run --workspace --all-features && \
  cargo deny check && typos && cargo machete
  ```

Read [GENESIS.md](./GENESIS.md) before writing any code. It is the normative spec; this file and CLAUDE.md are the conventions layer on top.
