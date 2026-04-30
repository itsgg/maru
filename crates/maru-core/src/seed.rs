//! [`SeedFile`] — adapter-supplied initial config files.
//!
//! GENESIS §6 line 226 + §11 "`ensure_profile_dirs`". Seeds are written
//! exactly ONCE per (profile, harness) pair, at profile creation. They are
//! never written during activation — that's the whole point of the
//! per-profile seeding model.

use std::path::PathBuf;

/// How a [`SeedFile`] interacts with any pre-existing file at its path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeStrategy {
    /// Write only if the target file does not exist. Never overwrites.
    OverwriteIfMissing,
    /// Parse the existing file as TOML, shallow-merge the seed contents
    /// (also TOML) at the top-level table only, and write the result. A
    /// user-set value on a key that the seed would set is preserved.
    TomlMergeShallow,
}

/// A single file an adapter wants written into a fresh profile dir.
///
/// GENESIS §6 line 226.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SeedFile {
    /// Path relative to the per-profile harness dir
    /// (e.g. `$MARU_HOME/profiles/<name>/codex/`).
    pub path: PathBuf,
    /// File contents to write, verbatim or merged per [`Self::merge`].
    pub contents: String,
    /// Merge strategy when the file already exists.
    pub merge: MergeStrategy,
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests"
)]
mod tests {
    use super::{MergeStrategy, SeedFile};
    use std::path::PathBuf;

    #[test]
    fn seed_file_round_trips_through_serde() {
        let s = SeedFile {
            path: PathBuf::from("config.toml"),
            contents: "[auth]\nstorage = \"file\"\n".to_owned(),
            merge: MergeStrategy::TomlMergeShallow,
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: SeedFile = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
        assert!(json.contains("\"toml_merge_shallow\""));
    }

    #[test]
    fn merge_strategy_variants() {
        let a = MergeStrategy::OverwriteIfMissing;
        let b = MergeStrategy::TomlMergeShallow;
        assert_ne!(a, b);
        let json = serde_json::to_string(&a).unwrap();
        assert_eq!(json, "\"overwrite_if_missing\"");
    }
}
