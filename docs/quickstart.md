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

## 2. Authenticate Claude (per-profile)

```sh
maru profile login work             # runs `claude setup-token`; paste the printed token when prompted; saves to <profile>/claude/oauth_token
```

This is the maru-recommended way to give a profile its own Claude credentials. The adapter exports the saved token via `CLAUDE_CODE_OAUTH_TOKEN` at activation, which bypasses Claude's shared-Keychain problem and gives each profile real OAuth isolation. See [`adapters/claude.md`](adapters/claude.md#per-profile-oauth-token-keychain-bypass) for details. Codex authenticates on first launch (step 3); Gemini too.

## 3. Activate it

```sh
maru profile use work
claude        # uses the work-profile OAuth token from step 2
codex         # first launch prompts for OAuth; tokens land under <profile>/codex/
```

Whatever you log into now lives under `$MARU_HOME/profiles/work/`. The shim sets `CLAUDE_CONFIG_DIR` and `CODEX_HOME` before exec'ing the real binary.

## 4. Add a second profile

```sh
maru profile create personal --harness claude,codex
maru profile login personal             # log in with a different Claude account
maru profile use personal
claude                                  # uses the personal-profile token
```

You now have two fully isolated profiles. Switch with `maru profile use <name>`. Claude credentials are isolated per-profile via per-token-file env vars; logging out from one profile no longer affects the other.

## 5. Verify

```sh
maru profile list
maru profile current

# Per-call override without changing the active profile:
MARU_PROFILE=work claude
```

## 6. Inspect what would happen

```sh
maru run --profile work --dry-run -- claude
```

Note: the first arg after `--` is the harness binary name (`claude`, `codex`, or `gemini`); `maru run` uses argv[0] dispatch the same way the shim does. Without it you'll get a clap error.

Prints the JSON activation plan (env vars, args prefix, diagnostics) without exec'ing anything. Useful when something feels wrong.

## What to read next

- **[GENESIS §7](../GENESIS.md)** — the per-harness adapter contracts. Important if you've heard rumors about the carve-outs.
- **[`limitations.md`](limitations.md)** — what maru does and doesn't cover, especially around IDE extension hosts.
- Per-harness specifics: [`adapters/claude.md`](adapters/claude.md), [`adapters/codex.md`](adapters/codex.md), [`adapters/gemini.md`](adapters/gemini.md).
