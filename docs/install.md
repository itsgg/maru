# Install

`maru` ships as two binaries — `maru` (the CLI) and `maru-shim` (the hot path that intercepts `claude` / `codex` / `gemini` invocations). The installers below place both side-by-side and run `maru install` to wire the per-harness shims into `$MARU_HOME/bin`.

> **Heads-up for macOS users.** Until the binaries are notarized (tracked in [`notes/phase-4-handoff.md`](notes/phase-4-handoff.md)), the **first** invocation of `maru` or `maru-shim` on macOS Sequoia (15+) sits for 30 s – 2 min while the system's `syspolicy` daemon does an online verification against Apple's servers. Every run after that is ~5 ms. **It is not hung.** The wait happens once per binary, then never again.

## Prerequisites

- One or more of the supported harness CLIs already on PATH: [`claude`](https://docs.claude.com/en/docs/claude-code), [`codex`](https://developers.openai.com/codex), [`gemini`](https://github.com/google-gemini/gemini-cli).
- Building from source additionally requires Rust 1.95.0 (the toolchain pinned in `rust-toolchain.toml`).

## Homebrew (macOS, Linux)

```sh
brew install itsgg/maru/maru
maru install
```

A single `maru` formula in the [`itsgg/homebrew-maru`](https://github.com/itsgg/homebrew-maru) tap installs both binaries (the shim arrives via a Homebrew `resource`). `maru install` is a separate step because Homebrew formulas don't write to user directories or shell rc — see [What `maru install` does](#what-maru-install-does) below.

## curl one-liner (macOS, Linux)

```sh
curl -sSL https://raw.githubusercontent.com/itsgg/maru/main/scripts/install.sh | sh
```

A thin wrapper at [`scripts/install.sh`](https://github.com/itsgg/maru/blob/main/scripts/install.sh) runs both per-binary installers (`maru-cli-installer.sh`, `maru-shim-installer.sh`) that `dist` produces on each release, drops both binaries into `$CARGO_HOME/bin` (defaults to `~/.cargo/bin`), and runs `maru install`. Pass `--no-shell-rc` to skip the shell rc edit:

```sh
curl -sSL https://raw.githubusercontent.com/itsgg/maru/main/scripts/install.sh | sh -s -- --no-shell-rc
```

If you'd rather invoke the per-binary installers directly (e.g. you don't trust a third-party wrapper script), see the [Manual install](#manual-install) section below.

## PowerShell (Windows)

```powershell
iwr https://raw.githubusercontent.com/itsgg/maru/main/scripts/install.ps1 | iex
```

The Windows equivalent of the curl wrapper. Drops both binaries into `%CARGO_HOME%\bin` and runs `maru install`. To skip the shell rc edit (PowerShell rc editing is not yet implemented anyway), invoke with `-NoShellRc`:

```powershell
& ([scriptblock]::Create((iwr https://raw.githubusercontent.com/itsgg/maru/main/scripts/install.ps1).Content)) -NoShellRc
```

## Scoop (Windows) — not yet available

The `itsgg/scoop-maru` bucket is reserved but no manifests are published yet. dist 0.31.0 doesn't auto-publish to Scoop on release; we'll add it once the bucket has manifests. Track at [docs/notes/phase-4-handoff.md](https://github.com/itsgg/maru/blob/main/docs/notes/phase-4-handoff.md).

Use the PowerShell installer above for now.

## winget (Windows) — not yet available

dist 0.31.0 doesn't auto-submit to `microsoft/winget-pkgs`; submission requires a manual `wingetcreate` step per release. Track at [docs/notes/phase-4-handoff.md](https://github.com/itsgg/maru/blob/main/docs/notes/phase-4-handoff.md).

Use the PowerShell installer above for now.

## Manual install (skip the wrapper)

If you'd rather not run a third-party wrapper script, invoke the per-binary installers `dist` ships with each release directly:

```sh
# macOS / Linux
curl -sSL https://github.com/itsgg/maru/releases/latest/download/maru-cli-installer.sh | sh
curl -sSL https://github.com/itsgg/maru/releases/latest/download/maru-shim-installer.sh | sh
maru install
```

```powershell
# Windows
iwr https://github.com/itsgg/maru/releases/latest/download/maru-cli-installer.ps1 | iex
iwr https://github.com/itsgg/maru/releases/latest/download/maru-shim-installer.ps1 | iex
maru install
```

These are the installers `dist` generates; the curl/PowerShell wrappers above just call both in sequence.

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

- **`maru install` appears to hang for 30 s – 2 min on macOS, then completes** → that's macOS Sequoia's `syspolicy` daemon doing a one-time online verification of an unsigned binary. The Homebrew formula pre-warms this during `brew install`, so subsequent `maru install` runs should be fast (~5 ms). For curl-installed binaries, the first invocation pays the cost. The fix is proper code-signing + notarization — tracked in [`notes/phase-4-handoff.md`](notes/phase-4-handoff.md) (`APPLE_TEAM_ID` / `APPLE_NOTARY_USER` / `APPLE_NOTARY_PASSWORD` secrets).
- **`could not locate the maru-shim binary`** → you installed `maru` but not `maru-shim`. The wrappers and Homebrew formula install both; if you ran the dist installer scripts manually, run both (see [Manual install](#manual-install-skip-the-wrapper)).
- **`maru: error: ...not found on PATH`** → run `maru doctor` to see what's missing.
- **`brew install`-installed claude not picked up by the shim** → the shim's job is to BE `claude` on your PATH. After `maru install`, `which claude` should return `$MARU_HOME/bin/claude`. If it returns the brew path, your shell rc edit didn't take effect — open a new terminal.
- **VS Code / Cursor extension still uses the old config** → expected. The Anthropic Claude VS Code extension and the Codex VS Code extension don't inherit env from your shell rc. See [`limitations.md`](limitations.md).
