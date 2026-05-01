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

- The GitHub Release page should have one artifact per target (5 tarballs/zips + checksums).
- `itsgg/homebrew-maru` should have a new commit `Formula/maru.rb`.
- `brew install itsgg/maru/maru` should now work.

## What to expect from `dist init`

Running `dist init` (interactively) reads `dist-workspace.toml`, asks about installers/targets/signing, and **regenerates `release.yml`** to match. Hand edits there will be overwritten. If `dist init` proposes changes you don't want, edit `dist-workspace.toml` and re-run.

## Fallback

If dist becomes unmaintained, see [`dist-exit-plan.md`](dist-exit-plan.md). The README install one-liners stay the same; only the workflow that produces the artifacts changes.
