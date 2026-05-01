# Quickstart

Five minutes from zero to switching between two profiles.

## 0. Install

See [`install.md`](install.md). Verify:

```sh
maru --version
maru doctor
```

## 1. Create your first profile

```sh
maru profile create work --harness claude,codex
```

This creates `$MARU_HOME/profiles/work/{claude,codex}/` and registers the profile in `state.toml`. No credentials are copied — these are fresh empty config dirs.

## 2. Activate it

```sh
maru profile use work
claude        # first launch will prompt for OAuth — that creds the `work` profile
codex
```

Whatever you log into now lives under `$MARU_HOME/profiles/work/`. The shim sets `CLAUDE_CONFIG_DIR` and `CODEX_HOME` before exec'ing the real binary.

## 3. Add a second profile

```sh
maru profile create personal --harness claude,codex
maru profile use personal
claude        # fresh OAuth flow — different account
```

You now have two fully isolated profiles: separate credentials, history, MCP servers, plugins, settings. Switch with `maru profile use <name>`.

## 4. Verify

```sh
maru profile list
maru profile current

# Per-call override without changing the active profile:
MARU_PROFILE=work claude
```

## 5. Inspect what would happen

```sh
maru run --profile work --dry-run -- claude
```

Prints the JSON activation plan (env vars, args prefix, diagnostics) without exec'ing anything. Useful when something feels wrong.

## What to read next

- **[GENESIS §7](../GENESIS.md)** — the per-harness adapter contracts. Important if you've heard rumors about the carve-outs.
- **[`limitations.md`](limitations.md)** — what maru does and doesn't cover, especially around IDE extension hosts.
- **[`adapters/`](adapters/)** — per-harness specifics (lands per Phase 2).
