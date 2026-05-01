//! Profile-directory and seed-file management.
//!
//! GENESIS §11: "`maru-store::ensure_profile_dirs(profile_root,
//! &harnesses)` is a separate concern called from `maru profile create` /
//! `import` / `add-harness`. It does the `mkdir -p` work and applies
//! `SeedFile`s. It is not on the activation hot path."

use std::path::{Path, PathBuf};

use maru_core::{HarnessId, MergeStrategy, SeedFile};

use crate::Error;

/// Per-harness profile subdirectory under `<profile_root>/`.
///
/// Hardcoded here for v1 because adapters live in a different crate
/// (cycle avoidance). Adding a new harness requires editing this match.
#[must_use]
pub const fn profile_subdir(harness: HarnessId) -> &'static str {
    match harness {
        HarnessId::Claude => "claude",
        HarnessId::Codex => "codex",
        HarnessId::Gemini => "gemini",
    }
}

/// `mkdir -p` the per-harness directories under `profile_root`.
///
/// Idempotent. Creates one subdir per harness in `harnesses`. Returns
/// the list of created (or pre-existing) absolute paths.
///
/// # Errors
///
/// Returns [`Error::Io`] if any directory creation fails.
pub fn ensure_profile_dirs(
    profile_root: &Path,
    harnesses: &[HarnessId],
) -> Result<Vec<PathBuf>, Error> {
    std::fs::create_dir_all(profile_root).map_err(|source| Error::Io {
        operation: "ensure profile_root".to_owned(),
        path: profile_root.to_path_buf(),
        source,
    })?;

    let mut created = Vec::with_capacity(harnesses.len());
    for h in harnesses {
        let dir = profile_root.join(profile_subdir(*h));
        std::fs::create_dir_all(&dir).map_err(|source| Error::Io {
            operation: format!("ensure {} dir", h.as_str()),
            path: dir.clone(),
            source,
        })?;
        created.push(dir);
    }
    Ok(created)
}

/// Apply a single [`SeedFile`] under `<harness_dir>/<seed.path>`.
///
/// `harness_dir` is one of the paths returned by [`ensure_profile_dirs`].
/// The `seed.path` is interpreted relative to it.
///
/// # Errors
///
/// Returns [`Error::Io`] for I/O failures, or [`Error::Encode`] if the
/// shallow TOML merge fails.
pub fn apply_seed(harness_dir: &Path, seed: &SeedFile) -> Result<(), Error> {
    let target = harness_dir.join(&seed.path);

    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|source| Error::Io {
            operation: "create seed parent dir".to_owned(),
            path: parent.to_path_buf(),
            source,
        })?;
    }

    match seed.merge {
        MergeStrategy::OverwriteIfMissing => {
            if target.exists() {
                return Ok(()); // Idempotent: leave existing alone.
            }
            crate::atomic::write_atomic(&target, seed.contents.as_bytes())?;
            Ok(())
        }
        MergeStrategy::TomlMergeShallow => apply_toml_merge_shallow(&target, &seed.contents),
    }
}

/// Apply all seeds for a single harness.
///
/// # Errors
///
/// Surfaces the first per-seed failure.
pub fn apply_seeds(harness_dir: &Path, seeds: &[SeedFile]) -> Result<(), Error> {
    for seed in seeds {
        apply_seed(harness_dir, seed)?;
    }
    Ok(())
}

fn apply_toml_merge_shallow(target: &Path, seed_contents: &str) -> Result<(), Error> {
    let existing_text = match std::fs::read_to_string(target) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(source) => {
            return Err(Error::Io {
                operation: "read existing seed target".to_owned(),
                path: target.to_path_buf(),
                source,
            });
        }
    };

    let existing: toml::Value = if existing_text.trim().is_empty() {
        toml::Value::Table(toml::value::Table::new())
    } else {
        toml::from_str(&existing_text).map_err(|e| Error::Decode {
            path: target.to_path_buf(),
            message: format!("existing TOML parse error: {e}"),
        })?
    };

    let seed: toml::Value = if seed_contents.trim().is_empty() {
        toml::Value::Table(toml::value::Table::new())
    } else {
        toml::from_str(seed_contents).map_err(|e| Error::Encode {
            message: format!("seed TOML parse error: {e}"),
        })?
    };

    let merged = shallow_merge(existing, seed);
    let text = toml::to_string_pretty(&merged).map_err(|e| Error::Encode {
        message: format!("merged TOML serialize error: {e}"),
    })?;
    crate::atomic::write_atomic(target, text.as_bytes())?;
    Ok(())
}

/// Shallow merge: existing top-level keys win; seed keys fill in only
/// what's missing.
///
/// This is the conservative direction — a user's hand-edited value is
/// never overwritten by an adapter seed.
fn shallow_merge(existing: toml::Value, seed: toml::Value) -> toml::Value {
    let (toml::Value::Table(mut e), toml::Value::Table(s)) = (existing, seed) else {
        // If either side isn't a table, the seed cannot merge — keep
        // the existing structure unchanged.
        return toml::Value::Table(toml::value::Table::new());
    };

    for (k, v) in s {
        e.entry(k).or_insert(v);
    }
    toml::Value::Table(e)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests"
)]
mod tests {
    use super::{apply_seed, apply_seeds, ensure_profile_dirs, profile_subdir};
    use maru_core::{HarnessId, MergeStrategy, SeedFile};
    use std::path::PathBuf;

    #[test]
    fn subdirs_match_genesis_layout() {
        assert_eq!(profile_subdir(HarnessId::Claude), "claude");
        assert_eq!(profile_subdir(HarnessId::Codex), "codex");
        assert_eq!(profile_subdir(HarnessId::Gemini), "gemini");
    }

    #[test]
    fn ensure_creates_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("work");
        let made = ensure_profile_dirs(&root, &[HarnessId::Claude, HarnessId::Codex]).unwrap();
        assert_eq!(made.len(), 2);
        assert!(root.join("claude").is_dir());
        assert!(root.join("codex").is_dir());
        assert!(!root.join("gemini").exists());
    }

    #[test]
    fn ensure_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("work");
        ensure_profile_dirs(&root, &[HarnessId::Claude]).unwrap();
        ensure_profile_dirs(&root, &[HarnessId::Claude]).unwrap();
        assert!(root.join("claude").is_dir());
    }

    #[test]
    fn overwrite_if_missing_writes_only_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        let seed = SeedFile {
            path: PathBuf::from("config.toml"),
            contents: "key = \"new\"\n".to_owned(),
            merge: MergeStrategy::OverwriteIfMissing,
        };
        // Pre-existing file is preserved.
        std::fs::write(dir.path().join("config.toml"), "key = \"old\"\n").unwrap();
        apply_seed(dir.path(), &seed).unwrap();
        let final_contents = std::fs::read_to_string(dir.path().join("config.toml")).unwrap();
        assert_eq!(final_contents, "key = \"old\"\n");

        // Missing file: seed writes.
        let dir2 = tempfile::tempdir().unwrap();
        apply_seed(dir2.path(), &seed).unwrap();
        let new_contents = std::fs::read_to_string(dir2.path().join("config.toml")).unwrap();
        assert_eq!(new_contents, "key = \"new\"\n");
    }

    #[test]
    fn toml_merge_shallow_preserves_user_values() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("config.toml"),
            "model = \"user-pinned\"\n[plugins]\nfoo = true\n",
        )
        .unwrap();

        let seed = SeedFile {
            path: PathBuf::from("config.toml"),
            contents: "model = \"adapter-default\"\nstorage = \"file\"\n".to_owned(),
            merge: MergeStrategy::TomlMergeShallow,
        };
        apply_seed(dir.path(), &seed).unwrap();

        let merged = std::fs::read_to_string(dir.path().join("config.toml")).unwrap();
        let parsed: toml::Value = toml::from_str(&merged).unwrap();

        // User's `model` is preserved; seed's `storage` is added.
        assert_eq!(
            parsed.get("model").and_then(toml::Value::as_str),
            Some("user-pinned")
        );
        assert_eq!(
            parsed.get("storage").and_then(toml::Value::as_str),
            Some("file")
        );
        // Existing nested table preserved.
        assert!(parsed.get("plugins").is_some());
    }

    #[test]
    fn toml_merge_shallow_writes_seed_to_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let seed = SeedFile {
            path: PathBuf::from("config.toml"),
            contents: "key = \"value\"\n".to_owned(),
            merge: MergeStrategy::TomlMergeShallow,
        };
        apply_seed(dir.path(), &seed).unwrap();
        let merged = std::fs::read_to_string(dir.path().join("config.toml")).unwrap();
        let parsed: toml::Value = toml::from_str(&merged).unwrap();
        assert_eq!(
            parsed.get("key").and_then(toml::Value::as_str),
            Some("value")
        );
    }

    #[test]
    fn apply_seeds_handles_multiple() {
        let dir = tempfile::tempdir().unwrap();
        let seeds = vec![
            SeedFile {
                path: PathBuf::from("a.toml"),
                contents: "x = 1\n".to_owned(),
                merge: MergeStrategy::OverwriteIfMissing,
            },
            SeedFile {
                path: PathBuf::from("b.toml"),
                contents: "y = 2\n".to_owned(),
                merge: MergeStrategy::OverwriteIfMissing,
            },
        ];
        apply_seeds(dir.path(), &seeds).unwrap();
        assert!(dir.path().join("a.toml").exists());
        assert!(dir.path().join("b.toml").exists());
    }

    #[test]
    fn apply_seed_creates_subdirs() {
        let dir = tempfile::tempdir().unwrap();
        let seed = SeedFile {
            path: PathBuf::from("nested/deep/config.toml"),
            contents: "key = \"value\"\n".to_owned(),
            merge: MergeStrategy::OverwriteIfMissing,
        };
        apply_seed(dir.path(), &seed).unwrap();
        assert!(dir.path().join("nested/deep/config.toml").exists());
    }
}
