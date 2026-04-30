//! [`ProfileContext`] — what an adapter's `plan()` sees.
//!
//! GENESIS §6 line 173. Borrowed everywhere because adapters are pure and
//! the caller (the shim) owns the underlying data.

use std::path::Path;

use crate::{HarnessId, ProfileName};

/// What the shim hands an adapter when computing an activation plan.
///
/// All fields are borrowed: adapters are pure functions over data, and the
/// shim is the single owner of the inputs.
///
/// GENESIS §6 line 173.
#[derive(Debug, Clone, Copy)]
pub struct ProfileContext<'a> {
    /// The active profile's identifier.
    pub profile_name: &'a ProfileName,

    /// `$MARU_HOME/profiles/<name>/`. Absolute path.
    ///
    /// Adapters concatenate their own subdir under this (e.g.
    /// `profile_root.join("claude")`).
    pub profile_root: &'a Path,

    /// Which harness is being activated.
    pub harness: HarnessId,

    /// The user's real home directory, in case the adapter needs to detect
    /// or warn about something under `$HOME` (e.g., the Linux/WSL Claude
    /// credential gate from GENESIS §7.1).
    pub home_dir: &'a Path,

    /// Directory containing a `.maru` project-pin, if one was found by
    /// walking up from `cwd`. `None` means the active profile came from
    /// `MARU_PROFILE` env or `active.txt`.
    pub project_pin: Option<&'a Path>,
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests"
)]
mod tests {
    use super::ProfileContext;
    use crate::{HarnessId, ProfileName};
    use std::path::Path;

    #[test]
    fn construct_and_borrow() {
        let name = ProfileName::new("work").unwrap();
        let root = Path::new("/var/maru/profiles/work");
        let home = Path::new("/Users/gg");
        let pin = Path::new("/Users/gg/Work/GG/maru");

        let ctx = ProfileContext {
            profile_name: &name,
            profile_root: root,
            harness: HarnessId::Claude,
            home_dir: home,
            project_pin: Some(pin),
        };

        assert_eq!(ctx.profile_name.as_str(), "work");
        assert_eq!(ctx.profile_root, root);
        assert_eq!(ctx.harness, HarnessId::Claude);
        assert_eq!(ctx.home_dir, home);
        assert_eq!(ctx.project_pin, Some(pin));
    }

    #[test]
    fn project_pin_is_optional() {
        let name = ProfileName::new("personal").unwrap();
        let root = Path::new("/p");
        let home = Path::new("/h");
        let ctx = ProfileContext {
            profile_name: &name,
            profile_root: root,
            harness: HarnessId::Codex,
            home_dir: home,
            project_pin: None,
        };
        assert!(ctx.project_pin.is_none());
    }
}
