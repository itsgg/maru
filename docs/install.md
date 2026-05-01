# Install

Pre-1.0 — installation is from source. Distribution channels (Homebrew tap, Scoop bucket, winget, `curl | sh`) land in Phase 4 per [GENESIS §14](../GENESIS.md).

## Prerequisites

- Rust 1.95.0 (the toolchain pinned in `rust-toolchain.toml`).
- One or more of the supported harness CLIs already on PATH: [`claude`](https://docs.claude.com/en/docs/claude-code), [`codex`](https://developers.openai.com/codex), [`gemini`](https://github.com/google-gemini/gemini-cli).

## Build from source

```sh
git clone https://github.com/itsgg/maru
cd maru
cargo build --release
```

Two binaries land under `target/release/`:

- **`maru`** — the manager binary you'll invoke directly (`maru profile create ...`, `maru doctor`, etc.).
- **`maru-shim`** — the hot-path shim. Symlinks named `claude`/`codex`/`gemini` dispatch through this binary by reading `argv[0]`.

## Install shims onto PATH

```sh
./target/release/maru install
```

This:

1. Creates `$MARU_HOME/bin/` and symlinks `claude`, `codex`, `gemini` into it pointing at `maru-shim` (Unix). On Windows, writes `.cmd` shims and copies the shim binary under each harness name.
2. Appends a managed block to your shell rc (`~/.bashrc`, `~/.zshrc`, or `~/.config/fish/config.fish`):

   ```sh
   # >>> maru managed block (do not edit) >>>
   export PATH="$HOME/Library/Application Support/maru/bin:$PATH"
   # <<< maru managed block <<<
   ```

3. The block is delimited by sentinel comments. Re-running `maru install` rewrites the block in-place; it never duplicates and refuses to overwrite a block you've hand-edited inside the markers.

Reload your shell (or `source ~/.zshrc`) and verify:

```sh
which claude        # should be inside $MARU_HOME/bin
maru doctor         # one-liner status of PATH + adapter detection
```

`$MARU_HOME` defaults to:

| OS      | Path                                  |
| ------- | ------------------------------------- |
| macOS   | `~/Library/Application Support/maru`  |
| Linux   | `$XDG_DATA_HOME/maru` (typically `~/.local/share/maru`) |
| Windows | `%LOCALAPPDATA%\maru`                 |

Set `MARU_HOME=...` to override.

## Uninstall

```sh
maru uninstall            # removes shim dir + the managed block from your shell rc
maru uninstall --purge    # also removes $MARU_HOME (state.toml, profiles, backups)
```

`uninstall` without `--purge` preserves your profiles so you can re-`install` later without losing state.

## Skip the shell rc edit

```sh
maru install --no-shell-rc
```

You'll need to add `$MARU_HOME/bin` to PATH yourself.

## Troubleshooting

- **`maru: error: ...not found on PATH`** → run `maru doctor` to see what's missing.
- **`brew install`-installed claude not picked up by the shim** → the shim's job is to BE `claude` on your PATH. After `maru install`, `which claude` should return `$MARU_HOME/bin/claude`. If it returns the brew path, your shell rc edit didn't take effect — open a new terminal.
- **VS Code / Cursor extension still uses the old config** → expected. The Anthropic Claude VS Code extension and the Codex VS Code extension don't inherit env from your shell rc. See [`limitations.md`](limitations.md).
