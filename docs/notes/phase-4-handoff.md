# Phase 4 — manual setup needed

Phase 4 (distribution) requires a few one-time setup steps the bot can't do on its own. This is the punch list.

## Repos to create

1. **`itsgg/homebrew-maru`** — empty public repo, `main` branch only. dist will write `Formula/maru.rb` here on each release.
2. **`itsgg/scoop-maru`** — empty public repo for the Scoop bucket. Add later when we wire Scoop into dist.

## GitHub Secrets to add to `itsgg/maru`

Settings → Secrets and variables → Actions:

| Secret | Purpose |
| --- | --- |
| `HOMEBREW_TAP_TOKEN` | PAT with `contents:write` on `itsgg/homebrew-maru`. dist uses it to push the formula update. |
| `APPLE_TEAM_ID` | Your Apple Developer team ID (10-char string). |
| `APPLE_NOTARY_USER` | Apple ID email registered with the developer account. |
| `APPLE_NOTARY_PASSWORD` | App-specific password generated at appleid.apple.com. |
| `WINDOWS_CERTIFICATE_BASE64` | Code-signing cert (.pfx) base64-encoded. |
| `WINDOWS_CERTIFICATE_PASSWORD` | Password for the .pfx. |

The first one is required for any release. The notarization + signing ones are optional — without them, releases ship unsigned with a "unidentified developer" warning on macOS and SmartScreen on Windows.

## winget submission

dist as of 0.31.0 doesn't auto-PR to `microsoft/winget-pkgs`. After each release:

1. Wait for the GitHub Release to be live.
2. Run `wingetcreate update --urls <release-url>/maru-<version>-x86_64-pc-windows-msvc.zip --version <version> --submit`.
3. Or hand-craft the manifest and PR to `microsoft/winget-pkgs`.

Track at: <https://github.com/microsoft/winget-pkgs>.

## First release dry-run

```sh
# In the maru working tree:
dist plan
# Verify the planned artifacts look right.

# Cut the first release:
git tag -a v0.1.0-alpha.0 -m "First dist-managed release"
git push origin v0.1.0-alpha.0
# Watch .github/workflows/release.yml run.
```

After the run:

- The GitHub Release page should have one artifact per target per binary (5 tarballs/zips × 2 binaries + checksums + installers + per-package formulas).
- The release also produces per-package `maru-cli.rb` and `maru-shim.rb` formulas. dist would auto-PR these to `itsgg/homebrew-maru` if `HOMEBREW_TAP_TOKEN` is set.
- The user-facing brew tap exposes a hand-maintained single `maru.rb` formula at `itsgg/homebrew-maru/Formula/maru.rb` that uses Homebrew's `resource` feature to pull both binaries. Per-release, regenerate it from the dist-produced `maru-cli.rb` + `maru-shim.rb` (URLs and SHAs change every tag) and push to the tap. See [Updating the brew formula](#updating-the-brew-formula) below.
- `brew install itsgg/maru/maru && maru install` should work end-to-end.

## Updating the brew formula

We hand-maintain `itsgg/homebrew-maru/Formula/maru.rb` (one user-facing formula, both binaries) instead of using the per-package formulas dist generates. To update for a new release:

1. Wait for `release.yml` to finish on the tagged commit.
2. Pull the per-package formulas from the release:
   ```sh
   gh release download <tag> --repo itsgg/maru --pattern '*.rb' -D ./formulas
   ```
3. Open `./formulas/maru-cli.rb` and `./formulas/maru-shim.rb`. The interesting bits are the `version` and the per-platform `url` + `sha256` blocks.
4. Edit `Formula/maru.rb` in the tap repo:
   - bump `version`
   - replace each platform's main `url` + `sha256` (these come from `maru-cli.rb`)
   - replace each platform's resource `url` + `sha256` inside `resource "maru-shim"` (these come from `maru-shim.rb`)
5. Commit and push to `itsgg/homebrew-maru`.

This is per-release manual work. To eliminate it, either (a) set `HOMEBREW_TAP_TOKEN` and accept the two-formula split that dist generates natively, or (b) write a generator that emits the unified `maru.rb` from the dist outputs.

## What to expect from `dist init`

Running `dist init` (interactively) reads `dist-workspace.toml`, asks about installers/targets/signing, and **regenerates `release.yml`** to match. Hand edits there will be overwritten. If `dist init` proposes changes you don't want, edit `dist-workspace.toml` and re-run.

## Fallback

If dist becomes unmaintained, see [`dist-exit-plan.md`](dist-exit-plan.md). The README install one-liners stay the same; only the workflow that produces the artifacts changes.
