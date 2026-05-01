# Claude Code adapter

GENESIS §7.1.

## Mechanism

Environment variable redirection. The shim emits **two** env vars unconditionally before exec'ing `claude`:

| Variable | Purpose |
| --- | --- |
| `CLAUDE_CONFIG_DIR` | Per-profile state: credentials, settings, sessions, projects, `.claude.json` |
| `CLAUDE_CODE_PLUGIN_CACHE_DIR` | Per-profile plugin marketplace cache (works around upstream issue #15071) |

Both paths are absolute — Claude Code does not expand `~` ([anthropics/claude-code#519](https://github.com/anthropics/claude-code/issues/519)).

## Profile layout

```
$MARU_HOME/profiles/<name>/claude/
├── .credentials.json    # OAuth tokens (Linux/WSL — see gate below)
├── .claude.json         # session + per-project trust state
├── settings.json
├── projects/
├── sessions/
└── plugins/             # CLAUDE_CODE_PLUGIN_CACHE_DIR target
```

## Linux/WSL credential gate

Per [anthropics/claude-code#47661](https://github.com/anthropics/claude-code/issues/47661): on Linux/WSL2 without a Keychain, `claude` falls through to `~/.claude/.credentials.json` even when `CLAUDE_CONFIG_DIR` is set, silently authenticating as the wrong account.

The adapter detects this combination (Linux + offending file present + `DBUS_SESSION_BUS_ADDRESS` unset) and emits a `Diagnostic::Error`. The shim treats this as a fatal pre-exec block (exit code 3). Fix: `mv ~/.claude/.credentials.json ~/.claude/.credentials.json.maru-bak` and rerun.

## macOS shared-Keychain credential storage (default)

On macOS, Claude Code stores OAuth credentials in the system Keychain under the single shared service `Claude Safe Storage` / account `Claude Key`. **The Keychain entry is not keyed per-`CLAUDE_CONFIG_DIR`**, so credentials are not isolated per maru profile on macOS — logging out from one profile clears the entry that all profiles share.

File state (sessions, projects, settings, plugins, `.claude.json`) IS isolated per profile via `CLAUDE_CONFIG_DIR`; only the OAuth tokens leak across.

See [`limitations.md`](../limitations.md#claude-on-macos-shared-keychain-credential-storage-default) for the workaround pattern. Conceptually upstream-tracked in [#47661](https://github.com/anthropics/claude-code/issues/47661); the structural fix would be a per-config-dir Keychain key.

## Carve-outs (still upstream bugs as of Claude Code 2.1.x)

| Issue | Description | Mitigation |
| --- | --- | --- |
| [#47056](https://github.com/anthropics/claude-code/issues/47056) | `~/.claude/CLAUDE.md` user memory still loaded under `CLAUDE_CONFIG_DIR` | Move/delete the global file; `maru doctor` warns. |
| [#42217](https://github.com/anthropics/claude-code/issues/42217) | MCP `.mcp.json` user-scope not loaded | Edit `<profile>/.claude.json` directly. |
| [#30538](https://github.com/anthropics/claude-code/issues/30538) | VS Code extension host doesn't inherit env | Out of v1 scope; Phase 6 daemon target. |

## Validation

`validate()` returns clean if the profile dir doesn't exist (fresh) or contains a readable directory. Missing `.credentials.json` after first use is acceptable (the user may not have logged in yet).

## Minimum supported version

Claude Code 2.0.42 (the version that started relocating `~/.claude.json` under `CLAUDE_CONFIG_DIR`; see [#3833](https://github.com/anthropics/claude-code/issues/3833)).
