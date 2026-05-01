# Install

Pick the channel that matches your platform. After installing the binary by **any** method, run `maru install` once to wire the shim symlinks into your PATH (see [Install shims onto PATH](#install-shims-onto-path) below).

## Prerequisites

- One or more of the supported harness CLIs already on PATH: [`claude`](https://docs.claude.com/en/docs/claude-code), [`codex`](https://developers.openai.com/codex), [`gemini`](https://github.com/google-gemini/gemini-cli).
- Building from source additionally requires Rust 1.95.0 (the toolchain pinned in `rust-toolchain.toml`).

## Homebrew (macOS, Linux)

```sh
brew install itsgg/maru/maru
```

Pulls from the [`itsgg/homebrew-maru`](https://github.com/itsgg/homebrew-maru) tap. The formula is updated automatically on each release.

## curl one-liner (macOS, Linux)

```sh
curl -sSL https://github.com/itsgg/maru/releases/latest/download/maru-installer.sh | sh
```

Detects your platform, downloads the matching tarball from the latest GitHub Release, verifies its checksum, and unpacks `maru` + `maru-shim` into `~/.local/bin` (or the closest equivalent).

## PowerShell (Windows)

```powershell
iwr https://github.com/itsgg/maru/releases/latest/download/maru-installer.ps1 | iex
```

Same idea as the curl installer, but for Windows. Drops the binaries into `%LOCALAPPDATA%\maru\bin`.

## Scoop (Windows)

```powershell
scoop bucket add maru https://github.com/itsgg/scoop-maru
scoop install maru
```

The Scoop bucket is maintained per-release: the manifest is updated when a new tag ships. If `scoop install` reports a stale version, run `scoop update` first.

## winget (Windows)

```powershell
winget install itsgg.maru
```

The winget manifest is submitted to `microsoft/winget-pkgs` after each release; expect a small lag between a fresh tag and the manifest going live.

## Build from source

```sh
git clone https://github.com/itsgg/maru
cd maru
cargo build --release
```

Two binaries land under `target/release/`:

- **`maru`** — the manager binary you'll invoke directly (`maru profile create ...`, `maru doctor`, etc.).
- **`maru-shim`** — the hot-path shim. Symlinks named `claude`/`codex`/`gemini` dispatch through this binary by reading `argv[0]`.

> The toolchain pin in `rust-toolchain.toml` is the build toolchain; the workspace MSRV (`Cargo.toml [workspace.package].rust-version`) is `1.85` for end-user `cargo install`.

## Install shims onto PATH

```sh
./target/release/maru install
```

This:

1. Creates `$MARU_HOME/bin/` and symlinks `claude`, `codex`, `gemini` into it pointing at `maru-shim` (Unix). On Windows, writes `.cmd` shims and copies the shim binary under each harness name.
2. Appends a managed block to your shell rc (`~/.bashrc`, `~/.zshrc`, or `~/.config/fish/config.fish`). The PATH entry is platform-specific:

   **macOS:**

   ```sh
   # >>> maru managed block (do not edit) >>>
   export PATH="$HOME/Library/Application Support/maru/bin:$PATH"
   # <<< maru managed block <<<
   ```

   **Linux:**

   ```sh
   # >>> maru managed block (do not edit) >>>
   export PATH="$XDG_DATA_HOME/maru/bin:$PATH"   # typically $HOME/.local/share/maru/bin
   # <<< maru managed block <<<
   ```

   **Windows:**

   ```powershell
   # The installer writes %LOCALAPPDATA%\maru\bin onto the user PATH via the registry;
   # there is no shell rc edit on Windows.
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
