//! `maru update` — self-update via dist artifacts. GENESIS §8 + Phase 4 task 4.7.
//!
//! With `--check`: queries the GitHub Releases API for the latest tag and
//! compares it to the compiled-in version. No download, no side effects.
//!
//! Without flags: downloads the platform tarball from the latest release,
//! extracts the new `maru` binary, and atomically replaces the running
//! binary via the `self_replace` crate.
//!
//! All HTTP is blocking via `ureq`. There is no async surface in this crate.
//! See GENESIS §13 for the dependency justification.

use std::io::{IsTerminal, Read};
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow, bail};
use clap::Args;
use serde::Deserialize;

use crate::CliError;

const RELEASES_API_URL: &str = "https://api.github.com/repos/itsgg/maru/releases/latest";
// GitHub recommends a descriptive User-Agent for API requests.
const USER_AGENT: &str = concat!("maru/", env!("CARGO_PKG_VERSION"), " (self-update)");

#[derive(Debug, Args)]
pub struct UpdateArgs {
    /// Print latest vs current and exit 0; do not download.
    #[arg(long)]
    pub check: bool,
    /// Skip the interactive confirmation prompt (required when stdin is not a TTY).
    #[arg(long)]
    pub yes: bool,
}

/// Subset of the GitHub Releases API response we care about.
#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Debug, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

/// Comparison result between latest release and current binary version.
#[derive(Debug, PartialEq, Eq)]
enum VersionCheck {
    UpToDate,
    UpdateAvailable {
        latest: String,
        current: &'static str,
    },
}

pub fn run(args: UpdateArgs) -> Result<()> {
    let body = fetch_latest_release_body()?;
    let release = parse_release(&body).context("parse GitHub /releases/latest response")?;

    let current = env!("CARGO_PKG_VERSION");
    let check = compare_versions(&release.tag_name, current);

    match &check {
        VersionCheck::UpToDate => {
            eprintln!("maru: up to date ({current})");
            return Ok(());
        }
        VersionCheck::UpdateAvailable { latest, .. } => {
            eprintln!("maru: update available: {current} -> {latest}");
        }
    }

    if args.check {
        return Ok(());
    }

    if !args.yes && !std::io::stdin().is_terminal() {
        return Err(CliError::user(
            "refusing to self-update without --yes when stdin is not a TTY",
        )
        .into());
    }
    if !args.yes {
        eprint!("Proceed with update? [y/N] ");
        let mut buf = String::new();
        std::io::stdin()
            .read_line(&mut buf)
            .context("read stdin for confirmation")?;
        if !matches!(buf.trim(), "y" | "Y" | "yes" | "YES") {
            eprintln!("maru: aborted");
            return Ok(());
        }
    }

    let asset = select_asset(&release.assets, target_triple()).ok_or_else(|| {
        anyhow!(
            "no asset matching target {} in release {}",
            target_triple(),
            release.tag_name
        )
    })?;
    eprintln!("maru: downloading {}", asset.browser_download_url);

    let archive_bytes = download_bytes(&asset.browser_download_url)?;
    let new_binary = extract_binary(&asset.name, &archive_bytes)?;
    replace_running_binary(&new_binary)?;

    eprintln!(
        "maru: replaced binary; relaunch to use {}",
        release.tag_name
    );
    Ok(())
}

/// Issue the GET against the GitHub releases API and return the body bytes
/// as a UTF-8 string. Returns a friendly error when there are no releases.
fn fetch_latest_release_body() -> Result<String> {
    let response = ureq::get(RELEASES_API_URL)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/vnd.github+json")
        .call();

    match response {
        Ok(mut r) => {
            let body = r
                .body_mut()
                .read_to_string()
                .context("read GitHub /releases/latest response body")?;
            Ok(body)
        }
        Err(ureq::Error::StatusCode(404)) => Err(CliError::user(
            "no releases published yet for itsgg/maru. \
                 First release ships during Phase 4 wrap-up.",
        )
        .into()),
        Err(e) => Err(e).context("GET https://api.github.com/repos/itsgg/maru/releases/latest"),
    }
}

/// Parse a /releases/latest JSON body. Public-in-crate so tests can reach it.
fn parse_release(body: &str) -> Result<GhRelease, serde_json::Error> {
    serde_json::from_str(body)
}

/// Compare the GitHub tag (e.g. `v0.1.0`) against `env!("CARGO_PKG_VERSION")`.
/// We strip a leading `v` from the tag for the comparison; otherwise it is
/// a plain string match (semver ordering is not needed — GitHub returns the
/// "latest" release directly, so we only ask "is this a different version?").
fn compare_versions(tag: &str, current: &'static str) -> VersionCheck {
    let latest = tag.strip_prefix('v').unwrap_or(tag);
    if latest == current {
        VersionCheck::UpToDate
    } else {
        VersionCheck::UpdateAvailable {
            latest: latest.to_owned(),
            current,
        }
    }
}

/// Find the asset that matches the current build's target triple. dist names
/// archives like `maru-aarch64-apple-darwin.tar.xz`; the comparison here is a
/// substring match because dist's exact suffix varies by platform.
fn select_asset<'a>(assets: &'a [GhAsset], target: &str) -> Option<&'a GhAsset> {
    assets.iter().find(|a| a.name.contains(target))
}

const fn target_triple() -> &'static str {
    // The `dist` archive naming uses the standard rustc target triple.
    // `std::env::consts` doesn't give us the triple directly, so we hand-roll.
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "aarch64-apple-darwin"
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "x86_64-apple-darwin"
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "x86_64-unknown-linux-gnu"
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        "aarch64-unknown-linux-gnu"
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        "x86_64-pc-windows-msvc"
    } else {
        "unknown"
    }
}

/// Download an asset URL into memory. We don't stream-extract because the
/// archives we ship are < 5 MB and an in-memory buffer simplifies retries.
fn download_bytes(url: &str) -> Result<Vec<u8>> {
    let mut r = ureq::get(url)
        .header("User-Agent", USER_AGENT)
        .call()
        .with_context(|| format!("GET {url}"))?;
    let mut bytes = Vec::new();
    r.body_mut()
        .as_reader()
        .read_to_end(&mut bytes)
        .with_context(|| format!("read body of {url}"))?;
    Ok(bytes)
}

/// Extract the `maru` binary out of an archive (.tar.xz / .tar.gz / .zip).
/// Returns the path to the extracted binary inside a `tempfile::TempDir`
/// kept alive for the duration of the replace step.
#[allow(
    clippy::case_sensitive_file_extension_comparisons,
    reason = "asset_name is already ASCII-lowercased; multi-suffix .tar.gz cannot use Path::extension"
)]
fn extract_binary(asset_name: &str, archive_bytes: &[u8]) -> Result<PathBuf> {
    use std::io::Cursor;

    let dir = tempfile::tempdir().context("create tempdir for extracted binary")?;
    let dir_path = dir.keep();

    let cursor = Cursor::new(archive_bytes);
    let lower = asset_name.to_ascii_lowercase();

    if lower.ends_with(".tar.gz") || lower.ends_with(".tgz") {
        // ascii-lowered above; clippy's case-sensitive lint is a false positive here.
        // Note: we do *not* use Path::extension because ".tar.gz" has two extensions.
        let gz = flate2::read::GzDecoder::new(cursor);
        tar::Archive::new(gz)
            .unpack(&dir_path)
            .with_context(|| format!("unpack {asset_name}"))?;
    } else if lower.ends_with(".tar.xz") {
        // dist's default archive on Unix. We don't ship liblzma; tell the user
        // to use the shell installer in that case.
        bail!(
            ".tar.xz extraction not supported by `maru update` yet. \
             Re-run the install one-liner from https://github.com/itsgg/maru#install."
        );
    } else if lower.ends_with(".zip") {
        bail!(
            ".zip extraction not supported by `maru update` yet. \
             Re-run the PowerShell installer from https://github.com/itsgg/maru#install."
        );
    } else {
        bail!("unrecognized archive format: {asset_name}");
    }

    let bin_name = if cfg!(windows) { "maru.exe" } else { "maru" };
    let candidate = walk_for_binary(&dir_path, bin_name)?
        .ok_or_else(|| anyhow!("no `{bin_name}` found in {asset_name}"))?;
    Ok(candidate)
}

fn walk_for_binary(root: &std::path::Path, name: &str) -> Result<Option<PathBuf>> {
    for entry in std::fs::read_dir(root).with_context(|| format!("read {}", root.display()))? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            if let Some(found) = walk_for_binary(&path, name)? {
                return Ok(Some(found));
            }
        } else if path.file_name().and_then(|n| n.to_str()) == Some(name) {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

/// Atomically replace the currently-running binary with the freshly-extracted
/// one. `self_replace` handles the cross-platform dance (Windows file locks,
/// Unix rename-and-spawn).
fn replace_running_binary(new_binary: &std::path::Path) -> Result<()> {
    self_replace::self_replace(new_binary)
        .with_context(|| format!("self-replace from {}", new_binary.display()))?;
    Ok(())
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::match_wildcard_for_single_variants,
    reason = "tests"
)]
mod tests {
    use super::*;

    /// Captured (trimmed) GitHub /releases/latest response. The fields outside
    /// our subset are harmless; serde ignores them.
    const SAMPLE_RELEASE: &str = r#"{
        "url": "https://api.github.com/repos/itsgg/maru/releases/123",
        "tag_name": "v0.2.0",
        "name": "v0.2.0",
        "draft": false,
        "prerelease": false,
        "assets": [
            {
                "name": "maru-aarch64-apple-darwin.tar.gz",
                "browser_download_url": "https://example.invalid/maru-aarch64-apple-darwin.tar.gz",
                "size": 1234
            },
            {
                "name": "maru-x86_64-unknown-linux-gnu.tar.gz",
                "browser_download_url": "https://example.invalid/maru-x86_64-unknown-linux-gnu.tar.gz",
                "size": 1234
            }
        ]
    }"#;

    #[test]
    fn parses_minimal_release_body() {
        let release = parse_release(SAMPLE_RELEASE).expect("parse ok");
        assert_eq!(release.tag_name, "v0.2.0");
        assert_eq!(release.assets.len(), 2);
        assert_eq!(release.assets[0].name, "maru-aarch64-apple-darwin.tar.gz");
    }

    #[test]
    fn compare_reports_up_to_date() {
        // The package version at compile time is the `current` we compare to.
        let current: &'static str = env!("CARGO_PKG_VERSION");
        let tag = format!("v{current}");
        assert_eq!(compare_versions(&tag, current), VersionCheck::UpToDate);
    }

    #[test]
    fn compare_reports_update_available() {
        let result = compare_versions("v9.9.9", "0.0.0");
        match result {
            VersionCheck::UpdateAvailable { latest, current } => {
                assert_eq!(latest, "9.9.9");
                assert_eq!(current, "0.0.0");
            }
            other => panic!("expected UpdateAvailable, got {other:?}"),
        }
    }

    #[test]
    fn compare_strips_v_prefix() {
        // tags without `v` should also match.
        assert_eq!(compare_versions("0.0.0", "0.0.0"), VersionCheck::UpToDate);
    }

    #[test]
    fn select_asset_picks_matching_target() {
        let release = parse_release(SAMPLE_RELEASE).unwrap();
        let pick = select_asset(&release.assets, "x86_64-unknown-linux-gnu").expect("found");
        assert_eq!(pick.name, "maru-x86_64-unknown-linux-gnu.tar.gz");
    }

    #[test]
    fn select_asset_returns_none_for_unknown_target() {
        let release = parse_release(SAMPLE_RELEASE).unwrap();
        assert!(select_asset(&release.assets, "nonsense-triple").is_none());
    }
}
