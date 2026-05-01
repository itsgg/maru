# maru

> A unified profile manager for AI coding agents.
> **maru** (மாறு) — Tamil for _change_ / _switch_.

`maru` lets you maintain multiple isolated profiles (work / personal / client) for **Claude Code**, **OpenAI Codex CLI**, and **Google Gemini CLI** on one machine, and switch between them with a single command. Credentials, history, MCP servers, plugins, and settings stay separated.

```sh
maru profile create work --harness claude,codex,gemini
maru profile use work
claude        # uses the work Claude Code config
codex         # uses the work Codex config
gemini        # uses the work Gemini config
```

`maru` does not replace any agent's auth or session machinery. It redirects each agent's "where my user state lives" to a per-profile directory via the agents' own supported environment variables (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `GEMINI_CLI_HOME`).

## Status

Phases 0–4 are merged on `main`. Pre-1.0 alpha; the first `v0.1.0-alpha.0` tag triggers binary distribution (see [phase-4-handoff](docs/notes/phase-4-handoff.md)). The full design is in [GENESIS.md](./GENESIS.md), which is the normative source of truth for the implementation.

## Install

```sh
# macOS / Linux (Homebrew)
brew install itsgg/maru/maru

# macOS / Linux (curl)
curl -sSL https://github.com/itsgg/maru/releases/latest/download/maru-installer.sh | sh

# Windows (PowerShell)
iwr https://github.com/itsgg/maru/releases/latest/download/maru-installer.ps1 | iex

# Windows (Scoop)
scoop bucket add maru https://github.com/itsgg/scoop-maru
scoop install maru

# Windows (winget)
winget install itsgg.maru
```

After install, run `maru install` once to wire the shim symlinks into your shell's PATH. Full instructions and a from-source path are in [`docs/book/src/install.md`](docs/book/src/install.md).

## Project structure

```
maru/
├── GENESIS.md              # Normative design document
├── CLAUDE.md / AGENTS.md   # Conventions for AI coding agents working on the repo
├── crates/                 # Workspace members (added in Phase 1)
│   ├── maru-core/          # Domain types, traits, pure logic
│   ├── maru-store/         # Profile DB, atomic writes, file locking
│   ├── maru-adapters/      # Per-harness implementations
│   ├── maru-activation/    # Env application + exec
│   ├── maru-cli/           # The `maru` binary
│   └── maru-shim/          # The hot-path shim binary
└── docs/                   # mdBook source + spike findings
```

## Contributing

This repo is set up to be driven by an autonomous coding agent (Claude Code) with a human reviewer at phase boundaries. Read in order:

1. **[GENESIS.md](./GENESIS.md)** — the normative design. The agent treats this document as truth; if implementation drifts from it, the doc wins.
2. **[CLAUDE.md](./CLAUDE.md)** — conventions, commands, workflow.
3. **[AGENTS.md](./AGENTS.md)** — same conventions, written for non-Anthropic agents.

Quality gates are in `lefthook.yml` (pre-commit / pre-push / commit-msg) and `.github/workflows/ci.yml`. Install hooks once with:

```sh
brew install lefthook   # or: go install github.com/evilmartians/lefthook@latest
lefthook install
```

## License

Apache-2.0 OR MIT (dual-licensed).
