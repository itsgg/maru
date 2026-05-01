//! `maru-store` — profile DB, atomic writes, file locking.
//!
//! GENESIS §10 / §11. Owns the on-disk layout under `$MARU_HOME` and
//! every write to `state.toml` / `active.txt`. Read-only operations are
//! lock-free; writes go through advisory `flock` (Unix) / `LockFileEx`
//! (Windows) on `<maru_home>/.lock`.
#![forbid(unsafe_code)]

pub mod active;
pub mod atomic;
pub mod copy;
pub mod deny_list;
pub(crate) mod lock;
pub mod profile_dirs;
pub mod resolve;
pub mod scrub;
pub mod snapshot;
pub mod state;
pub mod time;

pub use profile_dirs::{apply_seed, apply_seeds, ensure_profile_dirs, profile_subdir};
pub use resolve::{PinLookup, Resolved, Source, resolve, walk_for_pin};
pub use snapshot::snapshot_state;
pub use state::{CURRENT_SCHEMA_VERSION, Defaults, ProfileEntry, State};
pub use time::{epoch_to_iso8601, now_iso8601};

use std::path::PathBuf;

/// Errors `maru-store` can produce.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Filesystem I/O error.
    #[error("{operation} at {path:?}: {source}")]
    Io {
        /// What we were trying to do.
        operation: String,
        /// Affected path.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: std::io::Error,
    },

    /// Failed to decode an on-disk file.
    #[error("decode {path:?}: {message}")]
    Decode {
        /// File we were decoding.
        path: PathBuf,
        /// Diagnostic.
        message: String,
    },

    /// Failed to encode data to write.
    #[error("encode: {message}")]
    Encode {
        /// Diagnostic.
        message: String,
    },

    /// Tried to insert a profile that already exists in `state.toml`.
    #[error("profile {name:?} already exists")]
    ProfileExists {
        /// The duplicate name.
        name: String,
    },

    /// A `.maru` file was found, but its first line does not parse as a
    /// valid [`maru_core::ProfileName`]. GENESIS §12 mandates that this
    /// is a hard error, not a silent fall-through to `active.txt`.
    #[error(
        ".maru at {path:?} contains invalid profile name {line:?}; refusing to silently fall back"
    )]
    InvalidPin {
        /// Path to the offending `.maru` file.
        path: PathBuf,
        /// The raw first line that failed to parse.
        line: String,
    },
}
