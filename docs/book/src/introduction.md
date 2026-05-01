# Introduction

`maru` is a unified profile manager for AI coding agents — Claude Code, OpenAI Codex CLI, and Google Gemini CLI. It lets you maintain isolated work / personal / client profiles on one machine and switch between them with one command.

```sh
maru profile create work --harness claude,codex,gemini
maru profile use work
claude        # uses the work Claude Code config
codex         # uses the work Codex config
gemini        # uses the work Gemini config
```

Each agent stores its credentials, history, plugins, MCP servers, and settings under a single global directory. `maru` redirects each agent's view of "where my user state lives" to a per-profile directory using each agent's own supported environment variable: `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `GEMINI_CLI_HOME`. No credential proxying, no scraping, no wrapping the agent's stdio — `maru` exec's the real binary and gets out of the way.

## What's in this book

- **[Install](install.md)** — install from source today; binary distribution lands in v1.0 via Homebrew tap, Scoop bucket, winget, and `curl | sh`.
- **[Quickstart](quickstart.md)** — five minutes from zero to two profiles.
- **[Limitations](limitations.md)** — what `maru` v1 doesn't cover (IDE extension hosts, certain upstream carve-outs).
- **[direnv integration](direnv.md)** — pin profiles to directories alongside `.envrc`.
- **Adapters** — per-harness specifics: env mechanism, profile layout, caveats.

## Status

Phases 0–4 are merged on `main`. Pre-1.0 alpha; the first `v0.1.0-alpha.0` tag triggers binary distribution (see [phase-4-handoff](https://github.com/itsgg/maru/blob/main/docs/notes/phase-4-handoff.md)). The full design is in [GENESIS.md](https://github.com/itsgg/maru/blob/main/GENESIS.md), which is the normative source of truth. Architecture decision records live in [`docs/decisions/`](https://github.com/itsgg/maru/tree/main/docs/decisions).

## Source

[github.com/itsgg/maru](https://github.com/itsgg/maru) — Apache-2.0 OR MIT.
