//! Advisory lock for `state.toml` writes.
//!
//! GENESIS §10: "An advisory file lock (`fd-lock`) on `$MARU_HOME/.lock`
//! is taken for any write operation. Reads are lock-free."
//!
//! Closure-based API because [`fd_lock::RwLock`] returns a guard with a
//! borrowed lifetime — wrapping it in a struct would require self-
//! referential types. The closure scope is the lock scope.

use std::fs::File;
use std::path::Path;

use fd_lock::RwLock;

use crate::Error;

/// Run `f` while the advisory write lock at `<maru_home>/.lock` is held.
///
/// Blocks on lock acquisition. The lock is released when `f` returns,
/// regardless of success or failure.
///
/// # Errors
///
/// Returns [`Error::Io`] if the lock file cannot be created or locked.
/// Errors from `f` are passed through unchanged.
#[allow(
    unreachable_pub,
    reason = "internal-only API; module visibility is pub(crate). The pub-vs-pub(crate) lint pair conflicts here; we resolve by allowing unreachable_pub."
)]
pub fn with_write_lock<F, R>(maru_home: &Path, f: F) -> Result<R, Error>
where
    F: FnOnce() -> Result<R, Error>,
{
    std::fs::create_dir_all(maru_home).map_err(|source| Error::Io {
        operation: "create maru_home for lock".to_owned(),
        path: maru_home.to_path_buf(),
        source,
    })?;
    let lock_path = maru_home.join(".lock");
    let file = File::options()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|source| Error::Io {
            operation: "open lock file".to_owned(),
            path: lock_path.clone(),
            source,
        })?;
    let mut rw = RwLock::new(file);
    let _guard = rw.write().map_err(|source| Error::Io {
        operation: "acquire write lock".to_owned(),
        path: lock_path,
        source,
    })?;
    f()
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests"
)]
mod tests {
    use super::with_write_lock;

    #[test]
    fn runs_closure_under_lock() {
        let dir = tempfile::tempdir().unwrap();
        let result = with_write_lock(dir.path(), || Ok(42_i32)).unwrap();
        assert_eq!(result, 42);
        assert!(dir.path().join(".lock").exists());
    }

    #[test]
    fn release_after_closure_allows_reacquire() {
        let dir = tempfile::tempdir().unwrap();
        with_write_lock(dir.path(), || Ok(())).unwrap();
        // No deadlock — second call succeeds.
        with_write_lock(dir.path(), || Ok(())).unwrap();
    }

    #[test]
    fn creates_maru_home_if_missing() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("sub").join("maru");
        with_write_lock(&nested, || Ok(())).unwrap();
        assert!(nested.join(".lock").exists());
    }

    #[test]
    fn closure_error_passes_through() {
        let dir = tempfile::tempdir().unwrap();
        let err = with_write_lock(dir.path(), || {
            Err::<(), _>(crate::Error::Encode {
                message: "boom".to_owned(),
            })
        })
        .unwrap_err();
        assert!(matches!(err, crate::Error::Encode { .. }));
    }
}
