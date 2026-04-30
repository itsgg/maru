# GENESIS.md — `maru`

> A unified profile manager for AI coding agents.
> **maru** (மாறு) — Tamil for _change_ / _switch_.

This document is the canonical plan. It is written for an autonomous coding agent (Claude Code) to execute end-to-end, with a human reviewer in the loop at phase boundaries. It is normative: when the agent must choose between this document and prior knowledge or convention, this document wins. When it is silent, default to idiomatic Rust and tasteful engineering.

---

## 1. Mission

Developers using AI coding agents (Claude Code, OpenAI Codex CLI, Google Gemini CLI) increasingly run multiple agents and multiple accounts (work/personal/client) on the same machine. Each agent stores its full user state — credentials, history, plugins, MCP servers, hooks, settings — under a single global directory. There is no clean way today to switch between configurations across agents.

`maru` is a thin, fast, IDE-independent profile manager that makes this trivial:

```sh
maru profile create work --harness claude,codex,gemini
maru profile use work
claude        # uses the work Claude Code config
codex         # uses the work Codex config
gemini        # uses the work Gemini config
```

`maru` does **not** replace any agent's auth, plugin, or session machinery. It only redirects each agent's view of "where my user state lives" to a per-profile directory.

### Positioning relative to direnv and inner profile features

- **vs. `direnv`**: a `.envrc` with `export CLAUDE_CONFIG_DIR=...` covers the project-pin case for one harness in interactive shells. `maru` adds (a) cross-harness orchestration with one command, (b) IDE-integrated-terminal pickup without per-user shell config, (c) `doctor` for empirical isolation checks, (d) named lifecycle (create/clone/export/import) with a credential deny-list, (e) shim-based dispatch that survives non-interactive shells. Users who already love direnv can layer `.maru` on top of it; the two do not conflict.
- **vs. each harness's own inner profiles** (e.g., Codex `[profiles]`): those are presets within one credential scope. `maru` profiles isolate at the credential / history / MCP / plugin scope. Both can coexist; see §7.2.

## 2. Scope

### In scope (v1.0)

- Three harnesses: **Claude Code**, **OpenAI Codex CLI**, **Google Gemini CLI**.
- macOS, Linux, Windows (native + Git Bash + WSL2).
- CLI surface for full profile lifecycle.
- `argv[0]`-dispatched shim binaries on `PATH` so any process spawning `claude` / `codex` / `gemini` from a shell — including IDE _integrated terminals_ — picks up the active profile automatically.
- Project-pinned profiles via a `.maru` file walked from `cwd` upward.
- Distribution via `dist` (formerly `cargo-dist`): GitHub Releases, Homebrew tap, Scoop bucket, Linux packages, signed/notarized binaries on macOS and Windows.

### Deferred to later versions

- Additional harnesses (Aider, Goose, Cursor CLI, Windsurf CLI, etc.) — adapter pattern is designed to make these drop-in additions.
- GUI (Tauri v2 + Svelte). Architecture must support it from day one via the `maru-core` crate; no implementation in v1.
- System daemon for cross-process active-profile propagation (the only clean fix for IDE _extension hosts_; see §2 non-goals and §18).
- Team/org features (shared profiles, RBAC, audit).

### Non-goals

- We do not store, copy, or proxy credentials. New profile = first-launch login flow for each harness. This is non-negotiable.
- We do not modify any agent's `settings.local.json`, `.codex/config.toml`, or `.gemini/settings.json` in user repos. Project-scope state belongs to the user's repo, not to `maru`.
- We do not provide a wrapper _protocol_ over the agents' stdio. `maru` exec's the real binary and gets out of the way.
- We do not aim to replace each agent's eventual native profile support if and when it ships. We aim to be useful _today_ and to remain useful as the cross-harness aggregation layer afterward.
- **We do not cover IDE _extension hosts_ in v1.** When the agent CLI is spawned by an IDE extension's own background process (rather than by a user shell), our env-var-based mechanism cannot reach it. The terminal-launched case (integrated terminal, plain `claude`/`codex`/`gemini`) is fully covered. The extension-host case is the explicit job of Phase 6 (system daemon) — see §14 and §18.

## 3. Naming and identifiers

- Project name: **maru**.
- Tagline: _"Switch between AI agent profiles like flipping a switch."_
- Manager binary: `maru`.
- Shim binaries (installed by `maru install`): `claude`, `codex`, `gemini` — all symlinks (or `.cmd` shims on Windows) dispatching to the same `maru-shim` executable, which selects an adapter by `argv[0]`.
- State dir env var: `MARU_HOME` (defaults to `$XDG_DATA_HOME/maru` on Linux, `~/Library/Application Support/maru` on macOS, `%LOCALAPPDATA%\maru` on Windows).
- Per-call override env var: `MARU_PROFILE`. An empty string is treated as "unset," not as a profile literally named `""`.
- Project-pin file: `.maru` (single line: profile name; or TOML for richer config later).

## 4. Architecture

```
                ┌────────────────────────────────────────┐
                │  User: `maru profile use work`         │
                │       or sets `.maru` in repo          │
                └──────────────────┬─────────────────────┘
                                   │
                          writes active.txt
                                   │
                                   ▼
            ┌──────────────────────────────────────────┐
            │   $MARU_HOME/active.txt    ← single line │
            │   $MARU_HOME/state.toml    ← profile DB  │
            │   $MARU_HOME/profiles/<name>/{claude,    │
            │       codex,gemini}/                     │
            └──────────────────┬───────────────────────┘
                               │  read by
                               ▼
   ┌─────────────────────────────────────────────────────────┐
   │  maru-shim (installed as `claude`, `codex`, `gemini`)   │
   │  1. argv[0] basename → harness id                       │
   │  2. resolve profile (env > .maru > active.txt)          │
   │  3. ask adapter for ActivationPlan (env vars only)      │
   │  4. apply env to current process                        │
   │  5. execvp() the real binary                            │
   └────────────────────────┬────────────────────────────────┘
                            │
            ┌───────────────┼───────────────┐
            ▼               ▼               ▼
       real claude     real codex      real gemini
   (CLAUDE_CONFIG_DIR  (CODEX_HOME    (GEMINI_CLI_HOME
       redirected)      redirected)    redirected)
```

The shim is the hot path. Everything else (`maru` manager binary, future GUI) is cold-path tooling around it.

All three v1 adapters use **environment-variable redirection only**. No symlink swaps. No filesystem mutations on the activation hot path. This is a deliberate simplification (see §7.3 for the historical reasoning behind dropping the symlink approach for Gemini).

### Layered design

`maru` is a workspace of crates split by I/O boundary, not by feature:

| Crate             | Role                                                  | I/O                          |
| ----------------- | ----------------------------------------------------- | ---------------------------- |
| `maru-core`       | Domain types, `HarnessAdapter` trait, pure logic      | None                         |
| `maru-store`      | Profile DB, atomic writes, file locking, dir creation | Filesystem                   |
| `maru-adapters`   | Per-harness implementations (Claude, Codex, Gemini)   | Filesystem (validation only) |
| `maru-activation` | Applies env from `ActivationPlan`, exec's real binary | Env, exec                    |
| `maru-cli`        | The `maru` binary                                     | All of the above             |
| `maru-shim`       | The shim binary; depends only on `maru-core` (subset) | Filesystem (read-only), exec |
| `maru-gui`        | (deferred) Tauri app over `maru-core`                 | All of the above             |

The `maru-core` crate has no I/O and no `tokio`. It is unit-testable as pure functions over data. This is the single most important architectural rule.

## 5. Workspace layout

```
maru/
├── Cargo.toml                 # workspace manifest
├── rust-toolchain.toml        # pin to stable
├── deny.toml                  # cargo-deny config
├── rustfmt.toml
├── clippy.toml
├── .github/
│   └── workflows/
│       ├── ci.yml             # fmt, clippy, test, deny, hyperfine
│       └── release.yml        # dist
├── docs/                      # mdBook source
├── crates/
│   ├── maru-core/
│   ├── maru-store/
│   ├── maru-adapters/
│   ├── maru-activation/
│   ├── maru-cli/
│   └── maru-shim/
├── xtask/                     # build orchestration
└── tests/
    └── e2e/                   # against real fake-harness binaries
```

Workspace `Cargo.toml` declares shared lints and dependency versions via `[workspace.dependencies]`. Member crates use `workspace = true` for those deps.

## 6. Core types and traits

These are the source of truth. The agent must implement them with these signatures (modulo trivial naming-convention adjustments).

```rust
// crates/maru-core/src/lib.rs

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// A profile's identity. Profiles are created and referenced by name.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ProfileName(String);

impl ProfileName {
    pub fn new(s: impl Into<String>) -> Result<Self, InvalidName>;
    pub fn as_str(&self) -> &str;
}
// Allowed: [A-Za-z0-9][A-Za-z0-9_-]{0,63}
// Empty string is rejected (callers must treat empty MARU_PROFILE as unset).

/// The set of harnesses we know about. Closed enum; new harnesses are
/// added here and gated behind cargo features.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum HarnessId {
    Claude,
    Codex,
    Gemini,
}

/// What the shim needs to know about the active profile when launching
/// a real harness binary.
pub struct ProfileContext<'a> {
    pub profile_name: &'a ProfileName,
    pub profile_root: &'a Path,        // $MARU_HOME/profiles/<name>
    pub harness: HarnessId,
    pub home_dir: &'a Path,            // user's real home
    pub project_pin: Option<&'a Path>, // dir containing .maru, if any
}

/// Adapters compute plans, never execute them.
pub trait HarnessAdapter: Send + Sync {
    fn id(&self) -> HarnessId;
    fn binary_names(&self) -> &'static [&'static str];

    /// Where this harness stores per-profile data, relative to
    /// $MARU_HOME/profiles/<name>/.
    fn profile_subdir(&self) -> &'static Path; // e.g. Path::new("claude")

    /// Detect the real binary on PATH (skipping our own shim location).
    fn detect(&self, env: &dyn Environment) -> Detection;

    /// Pure: compute the activation plan for the given context.
    fn plan(&self, ctx: &ProfileContext<'_>) -> Result<ActivationPlan, AdapterError>;

    /// Read-only sanity check on a profile dir for `maru doctor`.
    fn validate(&self, profile_dir: &Path) -> ValidationReport;

    /// One-time setup written when a profile is first created with
    /// this harness enabled. Pure data; executed by `maru-store`.
    /// Used today only by the Codex adapter to pin file-based auth
    /// storage — see §7.2.
    fn seed(&self, profile_dir: &Path) -> Vec<SeedFile> { Vec::new() }
}

/// Data, not behavior. Executed by `maru-activation`.
#[derive(Debug, Clone, Default)]
pub struct ActivationPlan {
    pub env: Vec<(OsString, OsString)>,
    pub args_prefix: Vec<OsString>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Written once at profile-creation time. Idempotent: writers must
/// merge with any pre-existing user content rather than overwrite.
#[derive(Debug, Clone)]
pub struct SeedFile {
    pub path: PathBuf,         // relative to profile_dir
    pub contents: String,
    pub merge: MergeStrategy,  // OverwriteIfMissing | TomlMergeShallow
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub level: Level,            // Info | Warn | Error
    pub message: String,
    pub help: Option<String>,
}

/// Trait abstraction over env/PATH/which so adapters stay testable.
pub trait Environment {
    fn var(&self, key: &str) -> Option<OsString>;
    fn path(&self) -> Vec<PathBuf>;
    fn which_skipping(&self, name: &str, skip: &Path) -> Option<PathBuf>;
    fn home_dir(&self) -> Option<PathBuf>;
}
```

Errors: `thiserror` in libraries, `anyhow` at binary edges. Library errors are typed; CLI surfaces them with `anyhow::Context` chains and a single `eprintln!` of the chain at exit. The shim has its own minimal error type (no `anyhow`, see §13).

Notes on the simplified `ActivationPlan`:

- v1.0 contains no `FsOp` variants; activation is env-only and therefore needs no transactional rollback.
- `maru-store` ensures `$MARU_HOME/profiles/<name>/<subdir>/` exists before activation; that's a profile-creation concern, not an activation concern.
- `SeedFile` is written exactly once per (profile, harness) pair when the user adds the harness to a profile. It is _never_ written during activation. This separation keeps the shim hot path purely env+exec.
- The `FsOp` enum from earlier drafts has been removed. It will return when a future adapter genuinely needs symlinks or backups; no v1 adapter does.

## 7. Adapter specifications

### 7.1 Claude Code (`maru-adapters::claude`)

- **Mechanism:** environment variable `CLAUDE_CONFIG_DIR`, documented at <https://code.claude.com/docs/en/env-vars>.
- **Profile subdir:** `claude/`.
- **Plan:**
  - `env: [("CLAUDE_CONFIG_DIR", profile_root.join("claude")), ("CLAUDE_CODE_PLUGIN_CACHE_DIR", profile_root.join("claude/plugins"))]` — must be **absolute** paths; the CLI does not expand `~` ([anthropics/claude-code#519](https://github.com/anthropics/claude-code/issues/519)).
  - `args_prefix: []`
  - `seed: []`
- **First-launch behavior:** the real `claude` binary, on seeing an empty `CLAUDE_CONFIG_DIR`, will prompt for OAuth login. This is correct and expected.
- **Validation:** profile dir is valid if it either does not exist (fresh) or contains a readable `.credentials.json` and `settings.json`. Missing `.credentials.json` after first use is a `Warn`, not `Error`.

#### Known carve-outs (what `CLAUDE_CONFIG_DIR` does NOT redirect)

These are real, documented in open Claude Code issues as of April 2026, and must be surfaced by `maru doctor` and the Phase 0 spike:

| Carve-out | Behavior | Issue | Mitigation |
| --- | --- | --- | --- |
| `~/.claude/CLAUDE.md` user memory | Still loaded even with var set | [#47056](https://github.com/anthropics/claude-code/issues/47056) | `doctor` warns; users who want isolated memory must move/delete the global file. |
| MCP `.mcp.json` user-scope file | Top-level `mcpServers` not loaded | [#42217](https://github.com/anthropics/claude-code/issues/42217) | `doctor` warns; users edit `<profile>/.claude.json` directly until upstream fix. |
| Plugin marketplaces directory | Honors a separate var | [#15071](https://github.com/anthropics/claude-code/issues/15071) | The Claude adapter MUST emit `CLAUDE_CODE_PLUGIN_CACHE_DIR=<profile>/claude/plugins` alongside `CLAUDE_CONFIG_DIR` unconditionally — setting it for users with no plugins is harmless. |
| VS Code / JetBrains extension _host_ | Doesn't inherit env from spawning shell | [#30538](https://github.com/anthropics/claude-code/issues/30538) | Out of v1 scope; Phase 6 daemon target. `doctor` reports "extension host coverage = none in v1." |
| macOS Keychain ACL after reboot | Per-profile entries occasionally lose ACL after reboot | [#19456](https://github.com/anthropics/claude-code/issues/19456) | Documented limitation; user re-runs `/login` per profile if hit. |
| `~/.claude.json` location | Pre-2.0.42 hardcoded to `$HOME`; 2.0.42+ relocates under `CLAUDE_CONFIG_DIR` | [#3833](https://github.com/anthropics/claude-code/issues/3833) | Adapter pins minimum supported Claude Code version `>= 2.0.42`; `doctor` checks `claude --version` and refuses older. |

#### Linux / WSL2 credential isolation gate

[anthropics/claude-code#47661](https://github.com/anthropics/claude-code/issues/47661) (open, April 2026, reproduced): on Linux/WSL2 without a Keychain, `claude` falls through to reading `~/.claude/.credentials.json` even when `CLAUDE_CONFIG_DIR` is set, silently authenticating as the wrong account.

The Claude adapter MUST detect this combination during activation and:

1. If `~/.claude/.credentials.json` exists AND target OS is Linux/WSL2 AND `secret-service`/keyring is unavailable: emit a `Diagnostic { level: Error }` with help text directing the user to either move the file (e.g. `mv ~/.claude/.credentials.json ~/.claude/.credentials.json.maru-bak`) or delete it.
2. The shim treats `Diagnostic::Error` as a fatal pre-exec block. Do not exec the real binary while this gate is tripped.
3. `maru doctor` mirrors the same check and offers `maru doctor --fix` to do the rename automatically (one-shot, atomic, with snapshot under `$MARU_HOME/backups/`).

This is the only place v1 touches a path in the user's `$HOME`. It is gated behind a single, narrow OS+condition check, and the operation is rename-not-delete.

### 7.2 OpenAI Codex CLI (`maru-adapters::codex`)

- **Mechanism:** environment variable `CODEX_HOME` (defaults to `~/.codex`), documented at <https://developers.openai.com/codex/config-advanced>.
- **Profile subdir:** `codex/`.
- **Plan:**
  - `env: [("CODEX_HOME", profile_root.join("codex"))]`
  - `args_prefix: []`
  - `seed: [SeedFile { path: "config.toml", contents: <file-storage directive>, merge: TomlMergeShallow }]`
- **Why the seed pins file-based credential storage:** Codex has three credential storage modes — `file`, `keyring`, `auto`. With `keyring` or `auto`, credentials may land in the OS credential store (single-keyed, not per-`CODEX_HOME`), **breaking profile isolation**. The seed pins file-based storage so credentials live in `<CODEX_HOME>/auth.json`. The seed merges shallowly into any pre-existing `config.toml` and never overwrites a user-set value. **Phase 0 verification target:** confirm the exact TOML key/value against the current Codex auth docs (<https://developers.openai.com/codex/auth>) before locking in the seed contents. Do not ship Phase 1 with an unverified key name.
- **Inner profiles are a different abstraction.** Codex's native `[profiles]` table inside `config.toml` (selectable via `codex --profile <name>`) toggles model/sandbox/approval presets within one `CODEX_HOME`. The `CODEX_PROFILE` env-var counterpart is a feature request ([#4432](https://github.com/openai/codex/issues/4432)), not yet shipped — do not reference it as a real mechanism. `maru` profiles isolate at the `CODEX_HOME` level (separate auth, history, MCP servers); inner profiles live within one of them.
- **IDE extension coverage:** the Codex VS Code extension (closed-source, [#5822](https://github.com/openai/codex/issues/5822)) and JetBrains integration both spawn the Codex CLI as a subprocess, so a terminal launched _from_ the IDE that exports `CODEX_HOME` will be inherited by the spawned CLI. **However**, GUI-launched IDEs (Dock/Spotlight on macOS, Start Menu on Windows) do not inherit shell rc env vars. For those, the v1 answer is: launch the IDE from a `maru run`-aware terminal, or wait for Phase 6. `doctor` reports the inheritance status it actually observes.
- **Validation:** profile dir is valid if it either does not exist (fresh) or contains `config.toml` with the file-storage directive intact (warn if missing — the adapter's seed got tampered with or upstream changed the schema).

### 7.3 Google Gemini CLI (`maru-adapters::gemini`)

- **Mechanism:** environment variable **`GEMINI_CLI_HOME`**. This is the supported, documented variable as of April 2026 — see [`docs/reference/configuration.md`](https://github.com/google-gemini/gemini-cli/blob/main/docs/reference/configuration.md) and the implementation in [`packages/core/src/utils/paths.ts`](https://github.com/google-gemini/gemini-cli/blob/main/packages/core/src/utils/paths.ts). It works on macOS, Linux, and Windows. (Earlier `maru` drafts used a `~/.gemini` symlink swap based on the assumption that env-var redirection was broken; that assumption referenced a non-existent variable name, `GEMINI_CONFIG_DIR`. The bug report behind that assumption — [google-gemini/gemini-cli#8248](https://github.com/google-gemini/gemini-cli/issues/8248) — was stale-bot-closed against a variable the product does not have.)
- **Profile subdir:** `gemini/`. Gemini will create its own `.gemini/` directory inside whatever `GEMINI_CLI_HOME` points at. So the resulting layout is `$MARU_HOME/profiles/<name>/gemini/.gemini/{settings.json,oauth_creds.json,...}`.
- **Plan:**
  - `env: [("GEMINI_CLI_HOME", profile_root.join("gemini"))]`
  - `args_prefix: []`
  - `seed: []`
- **Pre-flight:** the adapter does NOT touch the user's `~/.gemini` directory at any point. If users want to migrate an existing `~/.gemini` into a maru profile, they use `maru profile import-existing --harness gemini` (Phase 3), which is an opt-in copy operation, not part of activation.
- **Credential storage caveat:** by default Gemini stores OAuth at `<GEMINI_CLI_HOME>/.gemini/oauth_creds.json` (mode 0600). If the user has set `GEMINI_FORCE_ENCRYPTED_FILE_STORAGE=true`, OAuth tokens go to the OS keychain under a single shared service name `gemini-cli-oauth` — at which point profile isolation breaks because keychain entries aren't keyed by profile. The adapter detects this env var at activation time and emits a `Diagnostic { level: Warn }` recommending the user unset it. This is _not_ a fatal gate (the user may know what they're doing), just a warning.
- **Validation:** profile dir is valid if it either does not exist (fresh) or `<profile_root>/gemini/.gemini/settings.json` exists. MCP token files (`mcp-oauth-tokens.json`), per-account info (`google_accounts.json`), and other state all live under the same dir and follow the env var.

### 7.4 Adapter capabilities matrix

| Capability                           | Claude                              | Codex                | Gemini                                    |
| ------------------------------------ | ----------------------------------- | -------------------- | ----------------------------------------- |
| Env-var redirection                  | ✅ `CLAUDE_CONFIG_DIR`              | ✅ `CODEX_HOME`      | ✅ `GEMINI_CLI_HOME`                      |
| Symlink / fs mutations on activation | ❌ none                             | ❌ none              | ❌ none                                   |
| IDE _integrated terminal_ coverage   | ✅                                  | ✅                   | ✅                                        |
| IDE _extension host_ coverage        | ❌ (#30538)                         | ❌ (GUI launch)      | ❌                                        |
| Credentials in profile dir           | ✅ `.credentials.json` (Linux gate) | ✅ when seed applied | ✅ default; ❌ if `…FORCE_ENCRYPTED…=true` |
| Native inner-profile feature         | ❌                                  | ✅ (different scope) | ❌                                        |
| Minimum supported version            | 2.0.42                              | rust rewrite (2025+) | 0.4.0                                     |

## 8. CLI surface

`clap` v4 derive. Every read-only command supports `--json`.

```
maru profile create <name> [--harness <list>] [--from <existing>]
maru profile list [--json]
maru profile use <name>                  # writes $MARU_HOME/active.txt (default)
maru profile use <name> --persist-shell  # writes a managed `export MARU_PROFILE=<name>` block to the user's shell rc
maru profile default <name>              # writes [defaults].profile in state.toml (fallback when active.txt empty)
maru profile current [--json]
maru profile delete <name> [--force]
maru profile rename <old> <new>
maru profile clone <from> <to>           # excludes credentials and other deny-listed paths
maru profile export <name> --to <path>   # tarball, excludes deny-listed paths
maru profile import <path>
maru profile import-existing --harness <id> [--name <profile>]   # copy ~/.claude etc. into a new profile

maru profile pin <name>                  # writes ./.maru
maru profile unpin                       # deletes ./.maru

maru doctor [--fix]                      # PATH, shim, perms, harness binaries, carve-outs, gates
maru adapter list [--json]
maru adapter status <id> [--json]

maru run --profile <name> -- <cmd> [args...]   # one-shot, no shell mutation
maru run --profile <name> --dry-run -- <cmd>   # prints the ActivationPlan as JSON, does not exec

maru install [--shell bash|zsh|fish|powershell]   # PATH setup, shim symlinks
maru uninstall [--purge]                          # --purge removes $MARU_HOME data dir too
maru update                                       # self-update via dist artifacts
maru version [--json]
maru schema                                       # emit JSON schema for state.toml + --json outputs
```

### CLI behavior rules

- All output to stderr unless `--json`, in which case the JSON document goes to stdout and human messages to stderr.
- Exit codes: `0` success, `1` user error (bad args, unknown profile), `2` environment problem (PATH, perms, missing harness binary), `3` adapter gate tripped (e.g., Linux Claude credential gate from §7.1), `64+` for adapter-specific failures.
- `maru` never prompts unless attached to a TTY. Non-TTY = fail fast with a clear error.
- `maru profile delete` requires `--force` if the profile has ever been activated (state.toml records this).
- `maru profile clone` and `export` apply the credential-safety rules in two stages:
  - **File-level exclusion** (drop the file from the tarball / clone target entirely): per-adapter credential files (Claude `.credentials.json`, Codex `auth.json`, Gemini `oauth_creds.json`), MCP/A2A token caches (`mcp-oauth-tokens.json`, `a2a-oauth-tokens.json`, Codex equivalents), any path matching `*keychain*` (case-insensitive).
  - **Value-level scrubbing** (file is included, but matching nested values are replaced with the literal string `"<scrubbed by maru>"`): in `settings.json` / `config.toml`, any key path containing a segment that case-insensitively matches `(token|api[_-]?key|secret|password|bearer|credential)` has its scalar value replaced. Nested tables are recursed; non-scalar values are dropped.
  - Static deny-list per adapter implements the rules; adding a new adapter requires populating it. Integration tests assert (a) the produced tarball does not contain excluded files and (b) scrubbed-value semantics produce a parseable output with no original secrets.
- `--persist-shell` and bare `use` are not mutually exclusive. The shim resolves profile in this order: `MARU_PROFILE` env (non-empty) > `.maru` walked from `cwd` > `active.txt` (non-empty) > `[defaults].profile` from `state.toml`. `--persist-shell` writes the env var; bare `use` writes `active.txt`; `default` writes the fallback. `--global` is reserved for future cross-user installs and exits non-zero with an explicit "not supported in v1" message.
- `maru schema` emits a stable, versioned JSON schema covering every `--json` output and the on-disk `state.toml` shape. CI snapshot-tests the schema; bumping it requires bumping `schema_version` in §10.

## 9. The shim

The shim is the **only** component on the user's hot path. Every `claude`, `codex`, `gemini` invocation goes through it. Performance and reliability matter more than features.

### Algorithm

```
1. let arg0 = basename(argv[0])
2. let harness = match arg0 { "claude" => Claude, "codex" => Codex, "gemini" => Gemini, _ => fail }
3. let profile_name =
     env("MARU_PROFILE").filter(|s| !s.is_empty())
       .or_else(|| read_project_pin(cwd))   // walk up looking for .maru
       .or_else(|| read_active_txt())       // non-empty single line
       .or_else(|| read_state_defaults())   // [defaults].profile in state.toml
       .ok_or(fail_with_install_hint)
4. let plan = adapter(harness).plan(&ctx)
       // Err(_) is a hard adapter failure → exit 64+ (per §8 codes)
5. for diag in &plan.diagnostics:
       if diag.level == Error: print and exit 3
       if diag.level == Warn:  print to stderr (one-line, no stack)
       if diag.level == Info:  suppressed unless MARU_LOG=debug
6. apply_env(plan.env)                       // see §11 on set_var safety
7. let real = which_skipping(arg0, our_install_dir)?
                                              // our_install_dir resolved from
                                              // canonicalize(current_exe()).parent()
8. unix:    execvp(real, [arg0] ++ plan.args_prefix ++ argv[1..])
   windows: spawn child via CreateProcess with inherited env, wait, exit with child's code
```

The Windows branch deliberately gives up the zero-overhead `execvp` model. Win32 has no in-place process replacement; the alternatives are (a) spawn-and-wait (chosen here — slightly higher RAM, correct exit-code propagation, signals work) or (b) `_execvp` from the CRT (replaces process image but breaks job objects and confuses parent shells). The §9 Windows perf budget (≤ 40ms from `main()` to `CreateProcess`) accounts for the spawn cost; the wait is not counted because it's bounded by the child, not by us.

### Performance budget

- macOS arm64 and Linux x86_64: cold start ≤ **15 ms p50 from `main()` entry to `execvp`**, measured by `hyperfine`. Build fails if exceeded.
- Windows: cold start ≤ **40 ms p50 from `main()` entry to `CreateProcess`** (the spawn call itself; the subsequent wait on the child is not counted). Measured the same way. Windows wall-clock cold start is dominated by image load + Defender + signature verification, which is not under our control. The budget is scoped to in-process work to keep the metric meaningful.
- Binary size ≤ **1 MB** stripped on all platforms.

### Build profile (Cargo)

```toml
[profile.shim]
inherits = "release"
panic = "abort"
lto = "fat"
codegen-units = 1
strip = "symbols"
opt-level = "z"
```

### Dependency budget for the shim

Allowed: `std`, `directories` (or `etcetera` if dep-graph audit prefers it), and a hand-rolled minimal reader for `state.toml` and `active.txt` (one-line-per-field; both files are tiny and stable).

**Forbidden in the shim crate:** `clap`, `serde`, `serde_derive`, `serde_json`, `toml`, `tokio`, `anyhow`, `tracing`, `reqwest`. The shim must compile without these. Anything beyond `std` + `directories`/`etcetera` requires explicit justification in a PR.

### Dispatch via `argv[0]`

Installed via symlinks on Unix and via small `.cmd` shims plus a single PE binary on Windows (since Windows symlinks for executables are messy across cmd/PowerShell/Git Bash). On Windows the shim is invoked by name and reads `argv[0]` directly; the `.cmd` files are needed only to make cmd.exe pick up the right name.

### PATH ordering

`maru install` prepends its shim install dir (`$MARU_HOME/bin` on Unix, `%LOCALAPPDATA%\maru\bin` on Windows) to the user's `PATH`. This must come ahead of any pre-existing `claude`/`codex`/`gemini` install. `maru doctor` validates the ordering and warns if a real binary is encountered before the shim.

## 10. Profile store

```
$MARU_HOME/
├── state.toml                  # source of truth: profiles, defaults, history
├── active.txt                  # one line: <profile-name>; empty = no active
├── bin/                        # shim symlinks live here, prepended to PATH
│   ├── claude
│   ├── codex
│   └── gemini
├── profiles/
│   ├── work/
│   │   ├── claude/             # CLAUDE_CONFIG_DIR target
│   │   ├── codex/              # CODEX_HOME target (with seeded config.toml)
│   │   └── gemini/             # GEMINI_CLI_HOME target; Gemini creates ./.gemini inside
│   └── personal/...
├── backups/
│   └── 2026-04-30T12-00-00Z/   # state.toml snapshots before destructive ops
└── logs/
    └── maru.log                # tracing JSON, rotated at 5MB / 5 files
```

`state.toml` schema (versioned):

```toml
schema_version = 1

[profiles.work]
created_at = "2026-04-30T10:15:00Z"
last_used_at = "2026-04-30T11:42:00Z"
harnesses = ["claude", "codex", "gemini"]
ever_activated = true

[profiles.personal]
# ...

[defaults]
profile = "personal"   # used by the shim when active.txt is empty/missing
```

The `schema_version` field is mandatory. On read, an unknown major version is a hard error; an unknown minor version logs a warning and continues.

**Profile-resolution sources** (used by both the shim and `maru profile current`):

| Source | Set by | Wins against |
| --- | --- | --- |
| `MARU_PROFILE` env var (non-empty) | shell or `--persist-shell` | everything below |
| `.maru` file walked from `cwd` | `maru profile pin` | `active.txt`, defaults |
| `active.txt` (non-empty single line) | `maru profile use <name>` | defaults |
| `[defaults].profile` in `state.toml` | `maru profile default <name>` | nothing |

If all sources are empty, the shim exits with code 1 and prints an install hint.

### Concurrency and atomicity

- All writes to `state.toml` and `active.txt` are write-temp-rename.
- An advisory file lock (`fd-lock`, or `std::fs::File::lock` if MSRV ≥ 1.89) on `$MARU_HOME/.lock` is taken for any write operation. Reads are lock-free.
- Reads of `active.txt` by the shim are explicitly racy: a `maru profile use foo` running while another shim is mid-launch may produce a launch with the old profile. This is by design — fail-soft beats blocking the hot path. Document explicitly in user docs: "the active profile is resolved at the moment your `claude`/`codex`/`gemini` invocation starts."
- Before any destructive op (`delete`, `import` overwriting), `state.toml` is snapshotted under `backups/` with an ISO-8601 timestamp.

## 11. Activation plan execution

The executor (in `maru-activation`) for v1 is trivial because plans are env-only:

1. For each `Diagnostic` in `plan.diagnostics`:
   - `Error` → print and exit non-zero (code 3); do not exec.
   - `Warn` → print one line to stderr; continue.
   - `Info` → suppressed unless `MARU_LOG=debug`.
2. Apply each `(key, value)` in `plan.env`. **Safety:** `std::env::set_var` is `unsafe` as of Rust 1.79 because environment mutation is not thread-safe. The shim is single-threaded by construction (no `tokio`, no `rayon`, no manually-spawned threads — see §13 forbidden list), and env application happens before any potential threading source could be introduced. The shim wraps the loop in a single `unsafe` block with a comment citing this invariant. Any future contributor adding threading to the shim must move env application to a different model.
3. Resolve the real harness binary via `which_skipping`.
4. Unix: `execvp(real, [arg0] ++ plan.args_prefix ++ argv[1..])`. Windows: see §9 algorithm — spawn-and-wait via `CreateProcess`.

There is no transactional rollback because there are no filesystem mutations. If a future adapter requires fs ops, reintroduce `FsOp` and a transactional executor at that point — not before.

`maru-store::ensure_profile_dirs(profile_root, &harnesses)` is a separate concern called from `maru profile create` / `import` / `add-harness`. It does the `mkdir -p` work and applies `SeedFile`s. It is not on the activation hot path.

### `--persist-shell` rc-file write contract

`maru profile use <name> --persist-shell` (and `maru install --shell <flavor>`) write to the user's shell rc using a managed block delimited by sentinel comments:

```sh
# >>> maru managed block (do not edit) >>>
export PATH="$MARU_HOME/bin:$PATH"
export MARU_PROFILE="work"
# <<< maru managed block <<<
```

Writes are idempotent: the manager parses out the existing block (if any) and replaces it. Repeat invocations do not stack. `maru uninstall` removes the block. If a user has hand-edited inside the markers, the manager refuses to overwrite and prints a diff.

## 12. Project-pin file (`.maru`)

Walked from `cwd` upward, stopping at the first `.maru` found or at `$HOME` / filesystem root. Format v1 is one line, the profile name. Format v2 may add TOML keys; the parser treats a non-empty first line as the profile name and ignores the rest for forward compatibility.

`maru profile pin <name>` writes `.maru` in `cwd`. `maru profile unpin` deletes it.

A `.maru` file containing an unknown profile name causes the shim to exit 1 with a clear message, NOT to silently fall back to `active.txt`. (Falling back hides the user's intent.)

## 13. Dependency budget (workspace-wide)

| Crate                           | Used in                                                | Justification                         |
| ------------------------------- | ------------------------------------------------------ | ------------------------------------- |
| `clap` (derive)                 | cli                                                    | Standard CLI parsing                  |
| `serde`, `serde_derive`         | core, store, cli                                       | Data model                            |
| `serde_json`                    | cli                                                    | `--json` output                       |
| `toml`                          | store, cli, adapters                                   | `state.toml`, Codex `config.toml` seed/inspection |
| `thiserror`                     | core, store, adapters, activation                      | Library errors                        |
| `anyhow`                        | cli                                                    | Binary errors                         |
| `tracing`, `tracing-subscriber` | cli                                                    | Structured logs                       |
| `directories` (or `etcetera`)   | core, store, shim                                      | Cross-platform paths. Pick `etcetera` if it materially shrinks the shim dep graph; benchmark in Phase 1. |
| `fd-lock`                       | store                                                  | Cross-platform advisory locks. May be replaced by `std::fs::File::lock` if workspace MSRV is set to ≥ 1.89. |
| `tempfile`                      | store, tests                                           | Atomic writes, test isolation         |
| `which`                         | adapters, cli                                          | Resolve real harness binaries         |
| `tar`, `flate2`                 | cli                                                    | `export`/`import`                     |

Dev-only:

| `assert_cmd`, `predicates`, `insta` | tests | CLI snapshots |
| `proptest`                          | tests | property tests for store atomicity, plan invariants |
| `hyperfine` (external)              | CI    | shim startup benchmark |

**Explicitly forbidden:** `tokio`, `async-std`, `reqwest`, any web framework, any ORM. There is no async or networking in this codebase.

The shim depends only on `std`, `directories` (or `etcetera`), and a hand-rolled reader for `active.txt`/`state.toml`. No `serde`, no `toml`. It has its own minimal error type — no `anyhow`. See §9 forbidden list.

### Distribution toolchain

- `dist` (formerly `cargo-dist`, repo still at <https://github.com/axodotdev/cargo-dist>; binary on disk is `dist`). Latest tested release pinned in `release.yml`.
- **Fallback plan.** The `axo.dev` corporate web presence has visibly contracted as of early 2026 (the root domain currently shows a "for sale" placeholder), even though the OSS project ships monthly. If `dist` becomes unmaintained, the documented exit is a hand-rolled GitHub Actions matrix + `softprops/action-gh-release` for assets, with Homebrew tap and Scoop bucket repos owned by us. Track this in `docs/notes/dist-exit-plan.md` and review at every release.

### GUI stack (Phase 5)

- **Tauri v2** (stable since Oct 2024).
- **Svelte 5 + adapter-static** (SPA mode required: `+layout.ts` with `export const ssr = false`).
- **`tauri-specta` v2**, NOT `ts-rs`. `tauri-specta` generates typed commands AND events end-to-end and traverses the type graph automatically; `ts-rs` does each type individually and is meaningfully more friction inside a Tauri app. Use `ts-rs` only if a non-Tauri TS consumer also exists.

## 14. Phased roadmap with acceptance criteria

Each phase ends with a tagged release on a feature branch and a human review. The agent must not begin a phase until the previous one is signed off.

### Phase 0 — Spike (≤ 5 days)

**Goal:** prove the foundation before writing real code. The §7 carve-outs and gates are NOT optional curiosities — they are the difference between a profile manager that works and one that silently corrupts state. Every check below produces a written finding in `docs/spike-results.md`.

The verification matrix is large (≈8 redirection checks × 4 platforms + ≈10 IDE-coverage checks). Treat the 5 days as a hard cap; if a check is inconclusive, mark it that way and move on rather than rabbit-holing — disconfirmed and inconclusive checks both feed into the doc-update PR before Phase 1.

Per-harness redirection checks (each on macOS, Linux, Windows native, WSL2):

- [ ] **Claude — `CLAUDE_CONFIG_DIR=/tmp/x claude`** produces a fresh Claude config in `/tmp/x` and triggers OAuth on first launch. Document any leakage.
- [ ] **Claude carve-outs**: with `CLAUDE_CONFIG_DIR` set, verify whether `~/.claude/CLAUDE.md` is loaded (#47056), whether project `<repo>/.mcp.json` and the user-scope `mcpServers` table inside `~/.claude.json` are loaded (#42217), whether plugin marketplaces respect `CLAUDE_CODE_PLUGIN_CACHE_DIR` (#15071). Tabulate per platform.
- [ ] **Claude Linux/WSL credential gate** (#47661): on Linux/WSL with no Keychain, confirm the silent fallthrough to `~/.claude/.credentials.json`. Verify the gate logic from §7.1 detects and blocks the case correctly.
- [ ] **Claude `~/.claude.json` location**: confirm relocation to `$CLAUDE_CONFIG_DIR/.claude.json` on the targeted minimum version (≥ 2.0.42).
- [ ] **Codex — `CODEX_HOME=/tmp/y codex`** produces a fresh Codex config. **Verify the exact TOML key/value that pins file-based credential storage** against current Codex auth docs (see §7.2). Confirm that with the directive present, credentials land in `auth.json` (not the OS keychain) on macOS, Linux, Windows.
- [ ] **Gemini — `GEMINI_CLI_HOME=/tmp/z gemini`** produces a fresh `/tmp/z/.gemini/` and triggers OAuth on first launch. **Specifically verify on Windows native**, since this is the platform we previously assumed was broken.
- [ ] **Gemini keychain warning**: with `GEMINI_FORCE_ENCRYPTED_FILE_STORAGE=true`, confirm that profile isolation breaks (single shared service name `gemini-cli-oauth`) and that the adapter's warning fires.

IDE coverage map:

- [ ] Confirm `CLAUDE_CONFIG_DIR` / `CODEX_HOME` / `GEMINI_CLI_HOME` set in the parent shell propagate into VS Code's, JetBrains', and Cursor's _integrated terminals_.
- [ ] Confirm they do NOT propagate to the IDE _extension hosts_ (Anthropic Claude VS Code extension, Codex VS Code extension, JetBrains AI Assistant Codex Agent). Document this as a v1 limitation.
- [ ] Test GUI-launched IDEs (Dock/Spotlight on macOS, Start Menu on Windows): document that env vars from shell rc do not propagate. Note workarounds (`launchctl setenv` on macOS, registry/`setx` on Windows).

Outcome: `docs/spike-results.md` published; design doc updates merged for any disconfirmed assumption.

**Exit criterion:** spike doc reviewed; no blocking surprises; carve-out matrix updated in §7.4.

### Phase 1 — Core MVP: Claude + Codex (3 weeks)

Phase 1 ships with **two adapters** rather than one. Claude alone has the most carve-outs of the three and can warp the architecture toward its quirks; pairing it with Codex (which has the cleanest mechanism) keeps the trait honest.

- [ ] Workspace skeleton, CI (fmt + clippy + test + cargo-deny + hyperfine).
- [ ] `maru-core`: types, `HarnessAdapter` trait, `Environment` trait + real impl, errors. Unit tests for `ProfileName` validation and `ActivationPlan` construction.
- [ ] `maru-store`: `state.toml` r/w, atomic writes, advisory lock, `active.txt`, `ensure_profile_dirs`, `SeedFile` writer with shallow TOML merge. Property tests for atomicity under concurrent writes.
- [ ] `maru-adapters::claude`: trait impl including the Linux/WSL credential gate. Unit tests against fake `Environment`.
- [ ] `maru-adapters::codex`: trait impl including the `[auth] storage = "file"` seed. Unit tests for the seed merge against a non-empty pre-existing config.
- [ ] `maru-activation`: env application + execvp. Tests with a fake exec target that prints its env.
- [ ] `maru-cli`: `profile create | list | use | current | delete | default | rename`, `install`, `uninstall`, `doctor`, `version`, `schema`, `run --dry-run`.
- [ ] `maru-shim`: argv[0] dispatch, profile resolution, env application, exec, diagnostic gate handling. Hyperfine benchmark in CI on macOS arm64, Linux x86_64, Windows.
- [ ] E2E test: against fake `claude` and `codex` shell scripts that print `$CLAUDE_CONFIG_DIR` / `$CODEX_HOME`.
- [ ] `docs/install.md`, `docs/quickstart.md`, `docs/limitations.md` (covering carve-outs and IDE-host gap).

**Exit criterion:**

- `cargo install --path crates/maru-cli` works on macOS, Linux, Windows.
- `maru install`, `maru profile create work --harness claude,codex`, `maru profile use work`, `claude` and `codex` (the shims) all work end-to-end.
- Shim cold-start ≤ 15 ms (Linux/macOS) and ≤ 40 ms (Windows) in CI.
- Linux Claude credential gate verified by an integration test that creates the offending file and asserts the shim exits 3.

### Phase 2 — Gemini adapter + cross-harness polish (1.5 weeks)

- [ ] `maru-adapters::gemini` (env-var only, no fs ops).
- [ ] Adapter registry in `maru-cli`, `--harness` flag accepts comma-separated list.
- [ ] `maru doctor` checks all three harnesses including the Gemini keychain warning and Claude carve-out diagnostics.
- [ ] `maru adapter status <id>` for per-harness diagnostics.
- [ ] E2E tests for all three against fake binaries on all OSes.
- [ ] `docs/adapters/{claude,codex,gemini}.md`.

**Exit criterion:** parity for all three harnesses; `maru profile use work` activates all three correctly on all three OSes; carve-out matrix in `maru doctor` matches §7.4.

### Phase 3 — Project pins, clone, export/import (1 week)

- [ ] `.maru` file walked by shim and CLI; unknown-profile error path.
- [ ] `maru profile pin / unpin / clone / export / import / import-existing`.
- [ ] Credentials + bearer-token deny-list per adapter; integration test verifying tarballs and clone targets are clean.
- [ ] `direnv` integration documented in `docs/direnv.md` (positioning, not bundling).

**Exit criterion:** `cd repo-with-.maru; claude` activates the pinned profile without any user shell state. `maru profile export work --to /tmp/work.tar.gz && tar tzf /tmp/work.tar.gz` shows zero matching deny-list patterns.

### Phase 4 — Distribution (1–2 weeks)

- [ ] `dist` configured for GitHub Releases.
- [ ] Homebrew tap, Scoop bucket, `winget` manifest.
- [ ] macOS notarization, Windows code signing.
- [ ] `maru update` self-update; never replaces a shim binary while a child is running (use `.next` filename + atomic rename on idle).
- [ ] mdBook docs site published.
- [ ] `README.md` with install one-liner per OS, demo gif.
- [ ] `docs/notes/dist-exit-plan.md` filled in with the GitHub-Actions-only fallback workflow.

**Exit criterion:** `brew install <tap>/maru`, `scoop install maru`, `winget install maru`, `curl -sSL ... | sh` all work on a fresh machine.

### Phase 5 — GUI (deferred; 3–4 weeks when scheduled)

Out of scope for v1.0. Architecture must accommodate it: `maru-core` and `maru-store` are already GUI-friendly. When scheduled, build `maru-gui` (Tauri v2 + Svelte 5 + `tauri-specta` v2). No CLI shelling — direct in-process calls.

### Phase 6 — System daemon (optional)

Only if Phase 4 user feedback demands cross-process active-profile propagation to already-open IDEs and to IDE _extension hosts_. Implement as user-level launchd / systemd / Windows Service that maintains a per-user environment binding (`launchctl setenv` on macOS, registry on Windows, systemd user environment on Linux) and refreshes it when `active.txt` changes. This is the only clean v1.x answer to the IDE-extension-host gap noted in §2 and §7.

## 15. Testing strategy

### Levels

1. **Unit tests** in every crate. Adapter `plan()` is pure; tested with a fake `Environment`. Target: ≥ 85% coverage for `maru-core` and adapters.
2. **Property tests** (`proptest`) for `state.toml` atomicity under concurrent writes and for `ActivationPlan` invariants.
3. **Integration tests** for `maru-store` and the `SeedFile` merge against `tempfile`-backed real filesystems.
4. **CLI snapshot tests** (`insta` + `assert_cmd`) for human and `--json` output. Snapshots live in `tests/snapshots/`.
5. **Schema stability tests**: `maru schema` output is snapshotted; bumping requires bumping `schema_version`.
6. **Carve-out detection tests**: e2e tests that explicitly assert known carve-outs (§7.1) are reflected in `maru doctor` output. If upstream fixes one of them, the test fails and we update the doctor logic.
7. **E2E tests** with fake harness binaries (`tests/e2e/fake-claude.sh` etc.) that print their environment. Runs in CI on Linux, macOS, Windows.
8. **Shim startup benchmark** (`hyperfine`) in CI; fails the build above the §9 budget.
9. **Live smoke tests** behind a `--features live-smoke` flag, exercising real `claude` / `codex` / `gemini` binaries. Runs nightly on a self-hosted runner with credentials in a sealed secret store. Not gated on PR.
10. **Deny-list tests**: clone and export commands run against a profile with seeded fake credential files; integration test asserts the output has zero matches against the static deny-list.

### Cross-platform matrix

| OS                  | Toolchain      | Tested in CI |
| ------------------- | -------------- | ------------ |
| macOS 14 (arm64)    | stable         | ✅           |
| macOS 14 (x86_64)   | stable         | ✅           |
| Ubuntu 22.04        | stable + musl  | ✅           |
| Ubuntu 22.04        | stable + glibc | ✅           |
| Windows Server 2022 | stable MSVC    | ✅           |
| Git Bash on Windows | manual + spike | nightly      |
| WSL2 (Ubuntu)       | stable         | nightly      |

## 16. Cross-platform considerations

- **Paths:** always `Path`/`PathBuf`, never `String`. Use `directories::ProjectDirs` (or `etcetera::AppStrategy`) for the state dir, `BaseDirs` for home.
- **Symlinks vs junctions on Windows:** v1 adapters do not require either. The shim is installed via `.cmd` shims wrapping a single PE binary; no Developer Mode needed. If a future adapter brings them back, the planned approach is junctions for dirs (`mklink /J`, no admin needed) but with the explicit understanding that junction _replacement_ is not atomic and needs care.
- **Git Bash:** detect `MSYSTEM`; convert paths via `cygpath -w` only when invoking native Windows binaries from inside Bash. `maru-shim` itself is a native PE.
- **WSL2:** treated as Linux for adapter behavior; specifically subject to the Claude Linux credential gate (§7.1).
- **macOS notarization:** required for distribution; budget 1 day per release for the codesign + notarytool dance until it's automated by `dist`.
- **Linux musl static builds:** for the CLI and shim. Avoids glibc version mismatches.
- **GUI-launched IDE env propagation:** documented in `docs/limitations.md` with platform-specific workarounds (`launchctl setenv` on macOS, `setx` / registry on Windows, systemd user environment on Linux). Phase 6 daemon is the long-term answer.

## 17. Conventions

### Style

- `rustfmt` with default config plus `imports_granularity = "Crate"`, `group_imports = "StdExternalCrate"`.
- `clippy` with `-D warnings` and `-W clippy::pedantic`. Specific allows go in `clippy.toml` with a comment justifying each.
- Public APIs documented; `#![deny(missing_docs)]` on `maru-core`.

### Errors

- Library crates: `thiserror`, one error enum per module-cluster, `#[from]` for transparent conversions.
- Binary crates: `anyhow::Result<T>` at function boundaries; `.context()` aggressively. `main` returns `anyhow::Result<()>`.
- Shim has a hand-rolled error enum and prints a single line on failure; no anyhow.

### Logging

- `tracing` with `tracing-subscriber`. JSON output when `MARU_LOG_FORMAT=json`, human-friendly otherwise.
- Default level `INFO` for `maru-cli`, `WARN` for the shim. Override via `MARU_LOG=debug`.
- Never log credentials or contents of `.credentials.json` / `auth.json` / `oauth_creds.json`. Static lints in CI grep tracing call sites for these strings and for the patterns matched by the §8 deny-list.
- The shim does not emit telemetry of any kind. The CLI does not emit telemetry. There are no network calls in v1.

### Commits

- Conventional Commits: `feat:`, `fix:`, `refactor:`, `docs:`, `test:`, `chore:`. Scope optional.
- One logical change per commit. Phase-completion commits tagged `phase-N-complete`.
- PRs squash-merged; PR title becomes the commit message.

### Branching

- `main` is always green and releasable.
- Phase work on `phase-N-<short-description>` branches.
- Hotfix branches off `main` only for shipped releases.

## 18. Risk register

| Risk                                              | Likelihood | Impact   | Mitigation                                                                                                                   |
| ------------------------------------------------- | ---------- | -------- | ---------------------------------------------------------------------------------------------------------------------------- |
| Anthropic ships native Claude profiles            | High       | Low      | Cross-harness aggregation remains the value prop; `ClaudeAdapter` shrinks but doesn't disappear.                             |
| Codex inner-profile naming confuses users         | Medium     | Medium   | Docs section; `maru doctor` flags collisions. Reserve `maru codex inner-profile` subcommand for explicit access.             |
| **Linux/WSL Claude credential isolation broken** ([#47661](https://github.com/anthropics/claude-code/issues/47661)) | **High**   | **High** | Adapter detects condition and blocks activation with `Diagnostic::Error` (§7.1). Tracked upstream; remove gate when fixed. |
| **IDE extension host doesn't inherit env**        | **High**   | **Medium** | Documented as v1 limitation; integrated-terminal case is fully covered. Phase 6 daemon is the long-term fix. |
| Claude `~/.claude/CLAUDE.md` user memory leaks    | Medium     | Low      | `doctor` warns; `docs/limitations.md` documents.                                                                             |
| Claude MCP `.mcp.json` not loaded under override  | Medium     | Medium   | `doctor` warns; users edit per-profile `.claude.json`. Tracked upstream ([#42217](https://github.com/anthropics/claude-code/issues/42217)). |
| Codex `keyring` storage breaks isolation          | Medium     | High     | Adapter seeds `[auth] storage = "file"` on profile creation; `validate()` warns if missing.                                  |
| Gemini `GEMINI_FORCE_ENCRYPTED_FILE_STORAGE=true` | Low        | High     | Adapter detects and warns at activation; `doctor` flags.                                                                     |
| Gemini changes config layout                      | Medium     | Medium   | Adapter is small; env-var redirection is robust to most internal changes. Pin against a tested `gemini` version range.       |
| Credential leak via export tarball                | Low        | Critical | Static deny-list per adapter; integration test verifies tarballs do not contain credentials, auth files, or bearer tokens.   |
| Self-update corrupts a running shim               | Low        | High     | Update only the manager binary; never replace shim while a child is alive. Use a `.next` filename and atomic rename on idle. |
| `dist` becomes unmaintained                       | Low        | Medium   | Pin `dist` version; renovate via PRs. `docs/notes/dist-exit-plan.md` documents a GitHub-Actions-only fallback.               |
| Pre-existing `claude`/`codex`/`gemini` on PATH ahead of shim | Medium | High | `maru install` prepends shim dir; `doctor` validates ordering and warns. |
| MSRV-sensitive deps (`directories`, `fd-lock`)    | Low        | Low      | Workspace pins MSRV; CI tests at MSRV. Reassess `std::fs::File::lock` (Rust 1.89+) at each toolchain bump. |

## 19. Glossary

- **Harness** — an AI coding agent CLI we wrap (Claude Code, Codex, Gemini).
- **Adapter** — implementation of `HarnessAdapter` for one harness.
- **Profile** — a named bundle of per-harness state directories.
- **Activation plan** — declarative description of what env vars to set to make a profile active for a harness.
- **Seed** — a one-time-written config file (e.g., Codex `[auth] storage = "file"`) emitted by an adapter at profile creation, never at activation.
- **Shim** — the small `maru-shim` binary installed under each harness's name on `PATH`.
- **Project pin** — a `.maru` file in or above `cwd` that overrides the active profile.
- **Inner profile** (Codex-specific) — Codex CLI's own native `[profiles]` feature inside `config.toml`. Distinct from a `maru` profile.
- **Carve-out** — a piece of harness state that does NOT follow our redirection mechanism (e.g., Claude's `~/.claude/CLAUDE.md`). Surfaced by `maru doctor`.
- **Integrated terminal** — the terminal emulator hosted inside an IDE (VS Code, JetBrains, Cursor). Inherits env from the IDE's launch environment _or_ from `maru install` shell rc edits. Covered by v1.
- **Extension host** — the IDE process that runs an extension's code and may spawn agent CLIs itself (e.g., Anthropic's Claude VS Code extension). Does NOT inherit shell rc env. Not covered by v1.

## 20. Appendix — references

### Claude Code
- Env vars (canonical): <https://code.claude.com/docs/en/env-vars>
- Settings: <https://code.claude.com/docs/en/settings>
- `.claude` directory layout: <https://code.claude.com/docs/en/claude-directory>
- SDK secure deployment (relocating `.claude.json`): <https://code.claude.com/docs/en/agent-sdk/secure-deployment>
- Issue [#47661](https://github.com/anthropics/claude-code/issues/47661) — Linux/WSL credential isolation broken
- Issue [#47056](https://github.com/anthropics/claude-code/issues/47056) — `~/.claude/CLAUDE.md` still loaded
- Issue [#42217](https://github.com/anthropics/claude-code/issues/42217) — MCP `.mcp.json` not loaded
- Issue [#30538](https://github.com/anthropics/claude-code/issues/30538) — VS Code extension host ignores var
- Issue [#19456](https://github.com/anthropics/claude-code/issues/19456) — macOS Keychain ACL after reboot
- Issue [#15071](https://github.com/anthropics/claude-code/issues/15071) — plugins/marketplaces dir hardcoded
- Issue [#3833](https://github.com/anthropics/claude-code/issues/3833) — `~/.claude.json` location history
- Issue [#519](https://github.com/anthropics/claude-code/issues/519) — `~` not expanded in `CLAUDE_CONFIG_DIR`

### Codex
- Advanced configuration (canonical): <https://developers.openai.com/codex/config-advanced>
- Authentication: <https://developers.openai.com/codex/auth>
- IDE extension: <https://developers.openai.com/codex/ide>
- `docs/config.md` source: <https://github.com/openai/codex/blob/main/docs/config.md>
- Issue [#4432](https://github.com/openai/codex/issues/4432) — multi-account auth (`CODEX_PROFILE` is proposed here, not shipped)
- Issue [#7971](https://github.com/openai/codex/issues/7971) — IDE extension config-path complaint
- Issue [#5822](https://github.com/openai/codex/issues/5822) — VS Code extension is closed-source

### Gemini
- Configuration reference (canonical): <https://github.com/google-gemini/gemini-cli/blob/main/docs/reference/configuration.md>
- Auth setup: <https://github.com/google-gemini/gemini-cli/blob/main/docs/get-started/authentication.md>
- Hosted docs site: <https://google-gemini.github.io/gemini-cli/>
- Issue [#22657](https://github.com/google-gemini/gemini-cli/issues/22657) — file locking (concurrency hazards)
- Issue [#21691](https://github.com/google-gemini/gemini-cli/issues/21691) — OAuth refresh races
- Issue [#8248](https://github.com/google-gemini/gemini-cli/issues/8248) — historical: `GEMINI_CONFIG_DIR` (a non-existent var) on Windows; superseded by `GEMINI_CLI_HOME`

### Tooling
- `dist` (formerly `cargo-dist`): <https://github.com/axodotdev/cargo-dist>
- `dist` rebrand announcement: <https://blog.axo.dev/2024/10/new-name>
- `directories` crate: <https://docs.rs/directories>
- `etcetera` crate (alternative): <https://docs.rs/etcetera>
- `fd-lock` crate: <https://docs.rs/fd-lock>
- `tauri-specta` v2: <https://github.com/specta-rs/tauri-specta>
- Tauri v2: <https://v2.tauri.app>

---

## Operating instructions for the executing agent

1. Read this entire document before writing any code.
2. Begin with **Phase 0**. Do not skip the spike. Surprises found later are 10× more expensive. Phase 0 must produce `docs/spike-results.md` with one line per check in §14, marked `verified` / `disconfirmed` / `inconclusive`.
3. If a Phase 0 check disconfirms an assumption in §7, open a PR that updates this document _before_ writing any adapter code. The doc is the source of truth; never let code drift ahead of it.
4. Implement crates in dependency order: `maru-core` → `maru-store` → `maru-adapters` → `maru-activation` → `maru-cli` and `maru-shim` in parallel.
5. At every phase boundary, open a PR with the phase tag in the title and pause for human review before continuing.
6. When the spec is silent or ambiguous, choose the option that best preserves: (a) shim performance, (b) testability of `maru-core`, (c) cross-platform behavior, (d) credential isolation. In that order.
7. Do not introduce dependencies not listed in §13 without justification in the PR description.
8. Keep `docs/` updated as you go, not at the end. A feature without docs is not done. `docs/limitations.md` is a first-class artifact, not an afterthought.
9. Anything you'd want to flag to a reviewer goes in `docs/notes/<phase-N>-<topic>.md`.
10. The carve-outs in §7.1 are upstream bugs, not bugs in `maru`. When upstream fixes one, delete the corresponding `doctor` warning, the test that asserts it, and the row in §7.1's carve-out table — in that order — in the same PR.
