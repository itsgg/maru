# Limitations

`maru` v1 is honest about what it does and doesn't cover. The list below is normative; items called out here are tracked in [GENESIS §7.1 / §18](../GENESIS.md) and surfaced by `maru doctor`.

## Covered

- **Terminal-launched harness invocations.** `claude`/`codex`/`gemini` run from any shell — login shell, IDE-integrated terminal, tmux, ssh — pick up the active profile via the `argv[0]`-dispatched shim.
- **Project pins via `.maru` files.** Phase 3.
- **Cross-platform.** macOS, Linux, Windows (native + WSL2). The Claude credential gate (#47661) is enforced by the adapter on Linux without an active D-Bus session (WSL2 is detected as Linux at compile time); the gate trips when `~/.claude/.credentials.json` exists and `DBUS_SESSION_BUS_ADDRESS` is unset (see [`crates/maru-adapters/src/claude.rs`](https://github.com/itsgg/maru/blob/main/crates/maru-adapters/src/claude.rs)).

## NOT covered in v1

### IDE extension hosts

The Anthropic Claude VS Code extension, the Codex VS Code extension, and the JetBrains AI Assistant Codex Agent run code inside the IDE's _extension host_ process. That process is spawned by the IDE itself, not by your user shell, and does NOT inherit env vars set in `~/.zshrc` / `~/.bashrc`.

**What this means:** even with maru installed, those extensions continue to read your real `~/.claude` / `~/.codex` directories.

**Workarounds:**

- For VS Code on macOS: launch with `code .` from a terminal that has the maru env. Dock-launched VS Code does NOT inherit shell rc env.
- For permanent fix: use `launchctl setenv` (macOS) / `setx` or registry (Windows) / systemd user environment (Linux) to set `MARU_PROFILE` system-wide.
- Phase 6 plans a system daemon that handles this cleanly — see [GENESIS §14 Phase 6](../GENESIS.md).

### GUI-launched IDEs

Same root cause: macOS Dock, Windows Start Menu, and Linux app launchers don't read your shell rc. Either launch the IDE from a terminal or set the env via the OS's per-user environment mechanism.

### Concurrent profile switching mid-flight

If you run `maru profile use foo` while another `claude` invocation is mid-startup, the in-flight invocation may complete with the OLD profile (resolution happens at process start, not throughout). This is by design — fail-soft beats blocking the hot path.

### Carve-outs from `CLAUDE_CONFIG_DIR`

Per Claude Code 2.1.x, the following carve-outs are still upstream issues, not maru bugs:

- `~/.claude/CLAUDE.md` user memory still loads even with `CLAUDE_CONFIG_DIR` set ([#47056](https://github.com/anthropics/claude-code/issues/47056)).
- MCP `.mcp.json` user-scope config is not loaded under override ([#42217](https://github.com/anthropics/claude-code/issues/42217)).
- The Anthropic VS Code extension host doesn't honor the env var ([#30538](https://github.com/anthropics/claude-code/issues/30538)).

`maru doctor` flags these as carve-outs. When upstream fixes one, maru drops the corresponding warning in the same PR.

### Claude credential isolation: use `maru profile login` (per-profile OAuth token)

Claude Code's OAuth credentials are stored in the macOS Keychain (or `~/.claude/.credentials.json` on Linux/Windows). The Keychain entry is **not reliably keyed per `CLAUDE_CONFIG_DIR`** across the Claude Code 2.1.x line — logging out from one profile has been observed to clear credentials shared with other profiles.

The fix maru ships: per-profile OAuth tokens via the `CLAUDE_CODE_OAUTH_TOKEN` env var, which is documented as authentication-precedence step 5 in [Claude Code's auth docs](https://code.claude.com/docs/en/authentication) — env-var tokens win over the Keychain. When the env var is set, Claude Code does not consult the Keychain at all.

**Setup:**

```sh
maru profile create work --harness claude
maru profile login work             # wraps `claude setup-token`; pastes the token into <profile>/claude/oauth_token
maru profile use work
claude                              # authenticates with the work-profile token, ignoring Keychain
```

The Claude adapter reads `<profile>/claude/oauth_token` at activation and exports `CLAUDE_CODE_OAUTH_TOKEN`. Each profile keeps its own token; logging out from one no longer affects another. The token file is on the GENESIS §8 deny-list and is never copied by `maru profile clone` / `export` / `import`.

If you'd rather generate the token yourself (for example to keep the OAuth flow in your usual terminal), pipe it in:

```sh
claude setup-token | maru profile login work --stdin
```

Codex and Gemini still rely on file-level isolation (see the keyring caveats below); per-profile OAuth tokens for those harnesses are not yet wired.

### Codex: `keyring` storage mode

If your `~/.codex/config.toml` enables OS-keyring storage for credentials, profile isolation breaks because keyring entries are not keyed per-`CODEX_HOME`. Phase 0 spike finding 0.5 disconfirmed the earlier-planned `[auth] storage = "file"` seed; per-platform behavior will be verified by a live-smoke nightly CI job once the user provisions the necessary credentials infrastructure (tracked in [`notes/phase-4-handoff.md`](notes/phase-4-handoff.md)) before any seed is reintroduced.

### Gemini: `GEMINI_FORCE_ENCRYPTED_FILE_STORAGE=true`

When this env var is set, OAuth tokens go to the OS keychain under a single shared service name `gemini-cli-oauth` — at which point profile isolation breaks. The Gemini adapter (Phase 2) emits a `Diagnostic::Warn` if it observes this env var.

### Phase 0 spike checks deferred to live-smoke

Three Phase 0 verification matrix entries (0.2 Claude carve-outs, 0.3 Linux/WSL credential gate, 0.7 Gemini keychain warning) require interactive sessions or specific runner conditions. They are deferred to a live-smoke nightly CI job that will run against real harness binaries with sealed credentials, once the user provisions the necessary credentials infrastructure (tracked in [`notes/phase-4-handoff.md`](notes/phase-4-handoff.md)).

## When upstream fixes a carve-out

We delete the corresponding `doctor` warning, the test that asserts the carve-out, and the row in [GENESIS §7.1](../GENESIS.md) — in that order, in the same PR. See the operating instructions at the bottom of GENESIS.
