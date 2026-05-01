# dist exit plan

If `axodotdev/cargo-dist` (the `dist` binary) becomes unmaintained, this is how we replicate the workflow with vanilla GitHub Actions.

The current state of `dist`: actively shipping monthly as of early 2026 (latest pinned in `dist-workspace.toml`), but the `axo.dev` domain has visibly contracted (root domain shows a "for sale" placeholder). We monitor at every release; this document is the fallback.

## Strategy

Replace the single `dist`-driven `release.yml` with:

1. **Build matrix** — one `runs-on` per target, each invoking `cargo build --release` with cross-compilation enabled where needed.
2. **Asset packaging** — tarballs for Unix targets (`tar czf maru-<version>-<target>.tar.gz target/<target>/release/maru target/<target>/release/maru-shim`), zip for Windows.
3. **Release publication** — `softprops/action-gh-release@v2` uploads each artifact to a GitHub Release, opens it for the tag.
4. **Homebrew formula PR** — a small script generates the SHA-256 sums and opens a PR against the tap repo using `peter-evans/create-pull-request`.
5. **Scoop manifest update** — same shape, against the scoop bucket repo.
6. **Self-update** — `maru update` already speaks GitHub Releases JSON (no dist-specific protocol); it continues working unchanged.

## Skeleton workflow

```yaml
name: Release (post-dist)

on:
  push:
    tags:
      - "v[0-9]+.[0-9]+.[0-9]+"

permissions:
  contents: write

jobs:
  build:
    strategy:
      fail-fast: false
      matrix:
        include:
          - { target: x86_64-unknown-linux-gnu,   os: ubuntu-22.04 }
          - { target: aarch64-unknown-linux-gnu,  os: ubuntu-22.04, cross: true }
          - { target: aarch64-apple-darwin,       os: macos-14 }
          - { target: x86_64-apple-darwin,        os: macos-14 }
          - { target: x86_64-pc-windows-msvc,     os: windows-latest }
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - name: Install cross
        if: matrix.cross
        run: cargo install cross --locked
      - name: Build
        run: |
          if [[ "${{ matrix.cross }}" == "true" ]]; then
            cross build --release --target ${{ matrix.target }} --bin maru
            cross build --release --target ${{ matrix.target }} --bin maru-shim
          else
            cargo build --release --target ${{ matrix.target }} --bin maru
            cargo build --profile shim --target ${{ matrix.target }} --bin maru-shim
          fi
      - name: Package
        shell: bash
        run: |
          name="maru-${{ github.ref_name }}-${{ matrix.target }}"
          mkdir -p dist/$name
          cp LICENSE-APACHE LICENSE-MIT README.md CHANGELOG.md dist/$name/
          if [[ "${{ matrix.os }}" == "windows-latest" ]]; then
            cp target/${{ matrix.target }}/release/maru.exe dist/$name/
            cp target/shim/maru-shim.exe dist/$name/
            (cd dist && 7z a -tzip $name.zip $name/)
          else
            cp target/${{ matrix.target }}/release/maru dist/$name/
            cp target/shim/maru-shim dist/$name/
            (cd dist && tar czf $name.tar.gz $name/)
          fi
          (cd dist && shasum -a 256 *.tar.gz *.zip > $name.sha256 2>/dev/null || true)
      - uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.target }}
          path: dist/

  release:
    needs: [build]
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v4
      - uses: actions/download-artifact@v4
        with:
          path: dist/
          merge-multiple: true
      - uses: softprops/action-gh-release@v2
        with:
          files: dist/**/*
          generate_release_notes: true
```

Plus separate jobs to PR the Homebrew tap update and Scoop manifest. Both repos own a `Formula/maru.rb` / `bucket/maru.json` that points at the GitHub Release URL.

## What we lose

- Auto-PR'd winget submissions (we'd file by hand each release).
- macOS notarization automation (we'd use `notarytool` via a small shell script).
- Code-signing automation (we'd add a step using the user-provided cert).

All of these are recoverable. The point of this doc is to ensure no lock-in.

## Trigger conditions

Switch to the fallback when **any** of:

1. `dist` hasn't shipped a release in 90 days.
2. A critical bug in `dist` blocks our release for >2 weeks.
3. axodotdev publicly announces the project is unmaintained.

Until then, prefer `dist`. It's better at all of this than we are.
