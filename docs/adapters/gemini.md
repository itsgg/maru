# Google Gemini CLI adapter

GENESIS §7.3.

## Mechanism

Environment variable redirection. The shim emits a single env var:

| Variable | Purpose |
| --- | --- |
| `GEMINI_CLI_HOME` | Per-profile root; Gemini creates a `.gemini/` subdir inside |

Note: the Gemini CLI creates its own `.gemini/` inside `GEMINI_CLI_HOME`, so the actual on-disk layout has one extra level vs. Claude/Codex.

## Profile layout

```
$MARU_HOME/profiles/<name>/gemini/    # GEMINI_CLI_HOME
└── .gemini/                          # Gemini's own state dir
    ├── settings.json
    ├── oauth_creds.json              # OAuth tokens (default storage)
    ├── projects.json
    └── history/
```

## Why env-var, not symlink

Earlier `maru` drafts used a `~/.gemini` symlink swap based on the assumption that env-var redirection was broken. That assumption referenced a non-existent variable name, `GEMINI_CONFIG_DIR`. The supported variable is **`GEMINI_CLI_HOME`** ([docs/reference/configuration.md](https://github.com/google-gemini/gemini-cli/blob/main/docs/reference/configuration.md)), works on macOS / Linux / Windows, and is verified by Phase 0 spike finding 0.6.

## Keychain caveat

By default, Gemini stores OAuth at `<GEMINI_CLI_HOME>/.gemini/oauth_creds.json`. If the user has set `GEMINI_FORCE_ENCRYPTED_FILE_STORAGE=true`, OAuth tokens go to the OS keychain under a single shared service name `gemini-cli-oauth` — which **defeats per-profile isolation** because keychain entries aren't keyed per-`GEMINI_CLI_HOME`.

The adapter detects this env var at activation time and emits a `Diagnostic::Warn` recommending the user unset it. This is **not a fatal gate** (the user may know what they're doing); just a warning surfaced in the shim's stderr and `maru doctor`.

## Validation

Profile dir is valid if it doesn't exist (fresh) or is a directory. The first launch creates `.gemini/` automatically.

## Minimum supported version

Gemini CLI 0.4.0 (the version where `GEMINI_CLI_HOME` semantics stabilized).
