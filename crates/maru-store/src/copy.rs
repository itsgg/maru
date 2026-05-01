//! Filtered profile-dir copy.
//!
//! Walks a source dir and copies every regular file to a parallel
//! location under the destination, except files that match the
//! [`crate::deny_list`] for the supplied harness.
//!
//! Used by `maru profile clone` and `maru profile import-existing`.

use std::path::{Path, PathBuf};

use maru_core::HarnessId;

use crate::{Error, atomic, deny_list, scrub};

/// Stats returned by [`copy_filtered`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CopyStats {
    /// Number of files copied.
    pub copied: usize,
    /// Number of files excluded by the deny-list.
    pub excluded: usize,
    /// Files that were excluded (relative to source root). Useful for
    /// surfacing in CLI output.
    pub excluded_paths: Vec<PathBuf>,
}

/// Recursively copy `src` to `dst`, excluding files that match the
/// per-harness deny-list. Symlinks are resolved (their target is
/// copied; symlinks are not preserved).
///
/// `dst` is created (`mkdir -p`) if missing. Existing files at `dst` are
/// overwritten. The intent is that callers pass a fresh destination.
///
/// # Errors
///
/// Returns [`Error::Io`] for any I/O failure walking `src` or writing to
/// `dst`.
pub fn copy_filtered(src: &Path, dst: &Path, harness: HarnessId) -> Result<CopyStats, Error> {
    let mut stats = CopyStats::default();
    if !src.exists() {
        return Ok(stats);
    }
    walk(src, src, dst, harness, &mut stats)?;
    Ok(stats)
}

fn walk(
    root: &Path,
    src: &Path,
    dst: &Path,
    harness: HarnessId,
    stats: &mut CopyStats,
) -> Result<(), Error> {
    let metadata = std::fs::metadata(src).map_err(|source| Error::Io {
        operation: "stat copy source".to_owned(),
        path: src.to_path_buf(),
        source,
    })?;

    if metadata.is_dir() {
        std::fs::create_dir_all(dst).map_err(|source| Error::Io {
            operation: "create destination dir".to_owned(),
            path: dst.to_path_buf(),
            source,
        })?;

        let entries = std::fs::read_dir(src).map_err(|source| Error::Io {
            operation: "read source dir".to_owned(),
            path: src.to_path_buf(),
            source,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| Error::Io {
                operation: "read dir entry".to_owned(),
                path: src.to_path_buf(),
                source,
            })?;
            let entry_src = entry.path();
            let name = entry.file_name();
            let entry_dst = dst.join(&name);
            walk(root, &entry_src, &entry_dst, harness, stats)?;
        }
        return Ok(());
    }

    if metadata.is_file() {
        let relative = src.strip_prefix(root).unwrap_or(src);
        if deny_list::is_excluded(harness, relative) {
            stats.excluded = stats.excluded.saturating_add(1);
            stats.excluded_paths.push(relative.to_path_buf());
            return Ok(());
        }
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent).map_err(|source| Error::Io {
                operation: "create dst parent".to_owned(),
                path: parent.to_path_buf(),
                source,
            })?;
        }
        std::fs::copy(src, dst).map_err(|source| Error::Io {
            operation: "copy file".to_owned(),
            path: src.to_path_buf(),
            source,
        })?;
        // GENESIS §8 value-level scrubbing: settings.json / config.toml
        // get re-written in place after copy, with sensitive scalars
        // replaced and sensitive non-scalar sub-trees dropped.
        if let Err(e) = scrub_in_place(dst) {
            // A scrub failure is fatal — better to leave nothing than
            // a half-redacted secret leak. Best-effort delete the dst
            // before propagating so the caller can't observe a raw copy.
            drop(std::fs::remove_file(dst));
            return Err(e);
        }
        stats.copied = stats.copied.saturating_add(1);
        return Ok(());
    }

    // Symlinks / sockets / fifos: ignore.
    Ok(())
}

/// Apply [`crate::scrub`] in place if `dst` is a `settings.json` or
/// `config.toml`. No-op for any other file. Errors are surfaced; the
/// caller is expected to drop the dst on failure rather than ship a
/// partially-redacted file.
fn scrub_in_place(dst: &Path) -> Result<(), Error> {
    let Some(name) = dst.file_name().and_then(|s| s.to_str()) else {
        return Ok(());
    };
    let scrubbed = match name {
        "settings.json" => {
            let text = std::fs::read_to_string(dst).map_err(|source| Error::Io {
                operation: "read for scrub".to_owned(),
                path: dst.to_path_buf(),
                source,
            })?;
            // Tolerate empty / non-JSON files rather than failing the
            // whole copy: some harnesses ship placeholder settings.json
            // with comments or partial JSON during development.
            match scrub::scrub_settings_json(&text) {
                Ok(s) => s,
                Err(Error::Decode { .. }) => return Ok(()),
                Err(other) => return Err(other),
            }
        }
        "config.toml" => {
            let text = std::fs::read_to_string(dst).map_err(|source| Error::Io {
                operation: "read for scrub".to_owned(),
                path: dst.to_path_buf(),
                source,
            })?;
            match scrub::scrub_config_toml(&text) {
                Ok(s) => s,
                Err(Error::Decode { .. }) => return Ok(()),
                Err(other) => return Err(other),
            }
        }
        _ => return Ok(()),
    };
    atomic::write_atomic(dst, scrubbed.as_bytes())
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "tests"
)]
mod tests {
    use super::copy_filtered;
    use maru_core::HarnessId;

    #[test]
    fn copies_benign_files_excludes_credentials() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();

        std::fs::write(src.path().join("settings.json"), b"{}").unwrap();
        std::fs::write(src.path().join(".credentials.json"), b"SECRET").unwrap();
        std::fs::create_dir_all(src.path().join("projects")).unwrap();
        std::fs::write(src.path().join("projects").join("notes.md"), b"hello").unwrap();

        let stats = copy_filtered(src.path(), dst.path(), HarnessId::Claude).unwrap();

        assert_eq!(stats.copied, 2);
        assert_eq!(stats.excluded, 1);
        assert!(
            stats
                .excluded_paths
                .iter()
                .any(|p| p.ends_with(".credentials.json"))
        );

        assert!(dst.path().join("settings.json").exists());
        assert!(dst.path().join("projects/notes.md").exists());
        assert!(!dst.path().join(".credentials.json").exists());
    }

    #[test]
    fn missing_src_is_noop() {
        let dst = tempfile::tempdir().unwrap();
        let stats = copy_filtered(
            std::path::Path::new("/no/such/source/path"),
            dst.path(),
            HarnessId::Claude,
        )
        .unwrap();
        assert_eq!(stats.copied, 0);
        assert_eq!(stats.excluded, 0);
    }

    #[test]
    fn keychain_files_universally_excluded() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("MyKeychain.dat"), b"k").unwrap();
        std::fs::write(src.path().join("settings.json"), b"{}").unwrap();
        let stats = copy_filtered(src.path(), dst.path(), HarnessId::Codex).unwrap();
        assert_eq!(stats.excluded, 1);
        assert!(!dst.path().join("MyKeychain.dat").exists());
        assert!(dst.path().join("settings.json").exists());
    }

    #[test]
    fn settings_json_value_level_scrubbed_on_copy() {
        // GENESIS §8 value-level scrubbing: settings.json is included in
        // the copy, but matching nested values are replaced with the
        // sentinel placeholder; benign keys are preserved verbatim.
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        let raw = r#"{
            "anthropic_api_key": "sk-deadbeef-leak",
            "ui": { "theme": "dark" },
            "tokens": { "github": "ghp_should_be_dropped" }
        }"#;
        std::fs::write(src.path().join("settings.json"), raw).unwrap();
        let stats = copy_filtered(src.path(), dst.path(), HarnessId::Claude).unwrap();
        assert_eq!(stats.copied, 1);

        let written = std::fs::read_to_string(dst.path().join("settings.json")).unwrap();
        assert!(
            !written.contains("sk-deadbeef-leak"),
            "raw API key leaked: {written}"
        );
        assert!(
            !written.contains("ghp_should_be_dropped"),
            "raw token leaked: {written}"
        );
        let v: serde_json::Value = serde_json::from_str(&written).unwrap();
        assert_eq!(v["anthropic_api_key"], "<scrubbed by maru>");
        assert_eq!(v["ui"]["theme"], "dark");
        assert!(
            v.get("tokens").is_none(),
            "non-scalar tokens table should be dropped: {written}"
        );
    }

    #[test]
    fn config_toml_value_level_scrubbed_on_copy() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        let raw = "[auth]\napi_key = \"sk-codex-leak\"\n[ui]\ntheme = \"dark\"\n";
        std::fs::write(src.path().join("config.toml"), raw).unwrap();
        let stats = copy_filtered(src.path(), dst.path(), HarnessId::Codex).unwrap();
        assert_eq!(stats.copied, 1);

        let written = std::fs::read_to_string(dst.path().join("config.toml")).unwrap();
        assert!(
            !written.contains("sk-codex-leak"),
            "raw secret leaked: {written}"
        );
        let v: toml::Value = written.parse().unwrap();
        assert_eq!(v["auth"]["api_key"].as_str(), Some("<scrubbed by maru>"));
        assert_eq!(v["ui"]["theme"].as_str(), Some("dark"));
    }

    #[test]
    fn scrub_tolerates_invalid_json() {
        // Non-JSON content in a settings.json filename is left as-is.
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("settings.json"), b"not json").unwrap();
        let stats = copy_filtered(src.path(), dst.path(), HarnessId::Claude).unwrap();
        assert_eq!(stats.copied, 1);
        assert_eq!(
            std::fs::read_to_string(dst.path().join("settings.json")).unwrap(),
            "not json"
        );
    }
}
