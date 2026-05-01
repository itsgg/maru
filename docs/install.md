# Install

`maru` ships as two binaries — `maru` (the CLI) and `maru-shim` (the hot path that intercepts `claude` / `codex` / `gemini` invocations). Both must be on PATH before you run `maru install`.

The installation steps below put both binaries in the same directory; `maru install` then writes per-harness symlinks that dispatch through `maru-shim`.

## Prerequisites

- One or more of the supported harness CLIs already on PATH: [`claude`](https://docs.claude.com/en/docs/claude-code), [`codex`](https://developers.openai.com/codex), [`gemini`](https://github.com/google-gemini/gemini-cli).
- Building from source additionally requires Rust 1.95.0 (the toolchain pinned in `rust-toolchain.toml`).

## Homebrew (macOS, Linux)

```sh
brew install itsgg/maru/maru-cli itsgg/maru/maru-shim
maru install
```

Pulls from the [`itsgg/homebrew-maru`](https://github.com/itsgg/homebrew-maru) tap. The two formulas are updated automatically on each release.

## curl one-liners (macOS, Linux)

```sh
curl -sSL https://github.com/itsgg/maru/releases/latest/download/maru-cli-installer.sh | sh
curl -sSL https://github.com/itsgg/maru/releases/latest/download/maru-shim-installer.sh | sh
maru install
```

Each installer detects your platform, downloads the matching tarball from the latest GitHub Release, and unpacks the binary into `$CARGO_HOME/bin` (defaults to `~/.cargo/bin`).

## PowerShell (Windows)

```powershell
iwr https://github.com/itsgg/maru/releases/latest/download/maru-cli-installer.ps1 | iex
iwr https://github.com/itsgg/maru/releases/latest/download/maru-shim-installer.ps1 | iex
maru install
```

Same idea as the curl installers, but for Windows. Drops both binaries into `%CARGO_HOME%\bin`.

## Scoop (Windows) — not yet available

The `itsgg/scoop-maru` bucket is reserved but no manifests are published yet. dist 0.31.0 doesn't auto-publish to Scoop on release; we'll add it once the bucket has manifests. Track at [docs/notes/phase-4-handoff.md](https://github.com/itsgg/maru/blob/main/docs/notes/phase-4-handoff.md).

Use the PowerShell installer above for now.

## winget (Windows) — not yet available

dist 0.31.0 doesn't auto-submit to `microsoft/winget-pkgs`; submission requires a manual `wingetcreate` step per release. Track at [docs/notes/phase-4-handoff.md](https://github.com/itsgg/maru/blob/main/docs/notes/phase-4-handoff.md).

Use the PowerShell installer above for now.

## Build from source

```sh
git clone https://github.com/itsgg/maru
cd maru
cargo build --release
./target/release/maru install
```

`cargo build --release` produces both binaries under `target/release/`:

- **`maru`** — the manager binary you'll invoke directly (`maru profile create ...`, `maru doctor`, etc.).
- **`maru-shim`** — the hot-path shim. Symlinks named `claude`/`codex`/`gemini` dispatch through this binary by reading `argv[0]`.

When you run `./target/release/maru install`, `maru` finds `maru-shim` next to itself in `target/release/` automatically.

> The toolchain pin in `rust-toolchain.toml` is the build toolchain; the workspace MSRV (`Cargo.toml [workspace.package].rust-version`) is `1.85` for end-user `cargo install`.

## What `maru install` does

1. Locates `maru-shim` (looks adjacent to the running `maru`, then on PATH).
2. Creates `$MARU_HOME/bin/` and symlinks `claude`, `codex`, `gemini` into it pointing at `maru-shim` (Unix). On Windows, writes `.cmd` shims and copies the shim binary under each harness name.
3. Appends a managed block to your shell rc (`~/.bashrc`, `~/.zshrc`, or `~/.config/fish/config.fish`). The PATH entry is platform-specific:

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
   # PowerShell rc editing is not yet implemented; add %LOCALAPPDATA%\maru\bin
   # to your PATH manually (System Properties → Environment Variables, or
   # `setx PATH "%LOCALAPPDATA%\maru\bin;%PATH%"`).
   ```

4. The managed block is delimited by sentinel comments. Re-running `maru install` rewrites the block in-place; it never duplicates and refuses to overwrite a block you've hand-edited inside the markers.

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

`uninstall` without `--purge` preserves your profiles so you can re-`install` later without losing state. Removing the `maru` and `maru-shim` binaries themselves is up to your package manager (`brew uninstall`, etc.).

## Skip the shell rc edit

```sh
maru install --no-shell-rc
```

You'll need to add `$MARU_HOME/bin` to PATH yourself.

## Troubleshooting

- **`could not locate the maru-shim binary`** → you installed `maru` but not `maru-shim`. Install both packages (see the steps above) and re-run `maru install`.
- **`maru: error: ...not found on PATH`** → run `maru doctor` to see what's missing.
- **`brew install`-installed claude not picked up by the shim** → the shim's job is to BE `claude` on your PATH. After `maru install`, `which claude` should return `$MARU_HOME/bin/claude`. If it returns the brew path, your shell rc edit didn't take effect — open a new terminal.
- **VS Code / Cursor extension still uses the old config** → expected. The Anthropic Claude VS Code extension and the Codex VS Code extension don't inherit env from your shell rc. See [`limitations.md`](limitations.md).
