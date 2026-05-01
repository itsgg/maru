//! Credential / token deny-list for `maru profile clone` / `export` /
//! `import-existing`.
//!
//! GENESIS §8: clone and export drop these files entirely (file-level
//! exclusion) before writing the target. The lists are hand-curated per
//! the documented surfaces of each harness.

use std::path::Path;

use maru_core::HarnessId;

/// Returns `true` if `relative_path` (relative to a per-harness profile
/// dir) should be excluded from clone / export / import-existing.
///
/// Match rules:
/// - Exact file-name match against the per-harness exclusion list.
/// - Universal substring match against `keychain` (case-insensitive).
#[must_use]
pub fn is_excluded(harness: HarnessId, relative_path: &Path) -> bool {
    if matches_universal(relative_path) {
        return true;
    }
    let Some(file_name) = relative_path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    excluded_file_names(harness)
        .iter()
        .any(|pat| pat.eq_ignore_ascii_case(file_name))
}

fn matches_universal(path: &Path) -> bool {
    path.components()
        .filter_map(|c| c.as_os_str().to_str())
        .any(|s| s.to_ascii_lowercase().contains("keychain"))
}

/// File names to drop, per harness. From GENESIS §8 deny-list.
#[must_use]
pub const fn excluded_file_names(harness: HarnessId) -> &'static [&'static str] {
    match harness {
        HarnessId::Claude => &[".credentials.json", "credentials.json"],
        HarnessId::Codex => &[
            "auth.json",
            "mcp-oauth-tokens.json",
            "a2a-oauth-tokens.json",
        ],
        HarnessId::Gemini => &[
            "oauth_creds.json",
            "mcp-oauth-tokens.json",
            "a2a-oauth-tokens.json",
            "google_accounts.json",
        ],
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests"
)]
mod tests {
    use super::is_excluded;
    use maru_core::HarnessId;
    use std::path::Path;

    #[test]
    fn claude_credentials_excluded() {
        assert!(is_excluded(
            HarnessId::Claude,
            Path::new(".credentials.json")
        ));
        assert!(is_excluded(
            HarnessId::Claude,
            Path::new("subdir/.credentials.json")
        ));
    }

    #[test]
    fn codex_auth_excluded() {
        assert!(is_excluded(HarnessId::Codex, Path::new("auth.json")));
    }

    #[test]
    fn gemini_oauth_excluded() {
        assert!(is_excluded(
            HarnessId::Gemini,
            Path::new("oauth_creds.json")
        ));
        assert!(is_excluded(
            HarnessId::Gemini,
            Path::new(".gemini/oauth_creds.json")
        ));
    }

    #[test]
    fn keychain_substring_universally_excluded() {
        for h in [HarnessId::Claude, HarnessId::Codex, HarnessId::Gemini] {
            assert!(is_excluded(h, Path::new("keychain.dat")));
            assert!(is_excluded(h, Path::new("foo/keychain-token")));
            assert!(is_excluded(h, Path::new("MacOSKeychain")));
        }
    }

    #[test]
    fn benign_files_not_excluded() {
        assert!(!is_excluded(HarnessId::Claude, Path::new("settings.json")));
        assert!(!is_excluded(HarnessId::Claude, Path::new("CLAUDE.md")));
        assert!(!is_excluded(HarnessId::Codex, Path::new("config.toml")));
        assert!(!is_excluded(HarnessId::Gemini, Path::new("settings.json")));
    }

    #[test]
    fn case_insensitive_match() {
        assert!(is_excluded(
            HarnessId::Claude,
            Path::new(".CREDENTIALS.JSON")
        ));
        assert!(is_excluded(HarnessId::Codex, Path::new("Auth.JSON")));
    }

    #[test]
    fn mcp_token_caches_excluded() {
        assert!(is_excluded(
            HarnessId::Codex,
            Path::new("mcp-oauth-tokens.json")
        ));
        assert!(is_excluded(
            HarnessId::Gemini,
            Path::new("a2a-oauth-tokens.json")
        ));
    }
}
