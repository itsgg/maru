# OpenAI Codex CLI adapter

GENESIS §7.2.

## Mechanism

Environment variable redirection. The shim emits a single env var:

| Variable | Purpose |
| --- | --- |
| `CODEX_HOME` | Per-profile config + auth + history + MCP servers |

## Profile layout

```
$MARU_HOME/profiles/<name>/codex/
├── auth.json         # OAuth + API-key credentials (when storage = file)
├── config.toml       # user config; merged with adapter seed if any
├── history.jsonl
└── sessions/
```

## Credential-storage caveat

Codex documentation references three storage modes — `file`, `keyring`, `auto` — but as of Codex CLI 0.125.0 (Phase 0 spike finding 0.5) there is no documented `[auth] storage` directive in `config.toml`, and `auth.json` appears to be the macOS default with no required directive.

For this reason the adapter currently emits **no seed** (`seed() == vec![]`). If the Phase 1 live-smoke nightly job discovers that Linux or Windows defaults to keyring storage (which would defeat per-profile isolation), the adapter will reintroduce a per-platform seed at that time.

## Inner profiles vs maru profiles

Codex's native `[profiles]` table inside `config.toml` toggles model/sandbox/approval presets within a single `CODEX_HOME`. **maru profiles isolate at the `CODEX_HOME` level** (separate auth, history, MCP servers); Codex inner profiles live within one of them.

Don't confuse:

```sh
maru profile use work          # switch to maru profile "work" (different auth/history)
codex --profile fast-iter      # switch to Codex inner profile "fast-iter" (same auth)
```

## IDE extension coverage

The official Codex VS Code extension is closed-source ([#5822](https://github.com/openai/codex/issues/5822)). Documentation says it "uses the Codex CLI" and shares `~/.codex/config.toml`. Subprocess inheritance means env-var redirection should work for terminal-launched IDEs but is not guaranteed for GUI-launched ones. The extension's [#7971](https://github.com/openai/codex/issues/7971) suggests there may be hardcoded path lookups in some code paths.

## Validation

Profile dir is valid if it doesn't exist (fresh) or is a directory. v1 does not validate `config.toml` schema (the upstream schema is not stable enough yet to fail-loud on).
