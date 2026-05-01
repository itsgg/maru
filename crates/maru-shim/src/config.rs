//! Hand-rolled config readers for `state.toml` and `active.txt`.
//!
//! GENESIS §9: the shim must compile without `serde` / `toml`. These
//! readers parse just enough to find the `[defaults].profile` value and
//! the active profile name from `active.txt`. Both files are tiny and
//! stable.

use std::path::{Path, PathBuf};

/// Resolve `$MARU_HOME` per GENESIS §3.
///
/// - `MARU_HOME` env var if set.
/// - Otherwise: `$XDG_DATA_HOME/maru` on Linux, `~/Library/Application
///   Support/maru` on macOS, `%LOCALAPPDATA%\maru` on Windows.
///
/// Uses `BaseDirs::data_local_dir().join("maru")` — that gives the
/// spec-correct path on every platform. (The earlier
/// `ProjectDirs::from("dev","maru","maru")` produced `dev.maru.maru` on
/// macOS, contradicting GENESIS §3.)
pub fn maru_home() -> Option<PathBuf> {
    if let Some(v) = std::env::var_os("MARU_HOME")
        && !v.is_empty()
    {
        return Some(PathBuf::from(v));
    }
    let dirs = directories::BaseDirs::new()?;
    Some(dirs.data_local_dir().join("maru"))
}

/// Read the first non-empty trimmed line of `<maru_home>/active.txt`.
/// Returns `None` if the file is missing, empty, or its first line is
/// not a syntactically valid profile name (the resolution chain falls
/// through to defaults in that case).
pub fn read_active_txt(maru_home: &Path) -> Option<String> {
    let path = maru_home.join("active.txt");
    let text = std::fs::read_to_string(path).ok()?;
    let line = text.lines().next()?.trim();
    if line.is_empty() || !crate::is_valid_profile_name(line) {
        return None;
    }
    Some(line.to_owned())
}

/// Read `[defaults].profile` from `<maru_home>/state.toml`. Returns
/// `None` if the file is missing, the table is missing, or the value
/// is not a valid profile name.
///
/// This is a deliberately tiny TOML subset: we only handle
///
///   [defaults]
///   profile = "name"
///
/// and tolerate whitespace + comments. We do NOT validate the full
/// `state.toml` schema — that's the job of `maru-store::state::read`.
pub fn read_default_profile(maru_home: &Path) -> Option<String> {
    let path = maru_home.join("state.toml");
    let text = std::fs::read_to_string(path).ok()?;
    extract_default_profile(&text)
}

fn extract_default_profile(text: &str) -> Option<String> {
    let mut in_defaults = false;
    for raw in text.lines() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if let Some(table) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            in_defaults = table.trim() == "defaults";
            continue;
        }
        if !in_defaults {
            continue;
        }
        if let Some(rest) = line.strip_prefix("profile") {
            let rest = rest.trim_start();
            let rest = rest.strip_prefix('=')?.trim_start();
            let value = parse_string_value(rest)?;
            if crate::is_valid_profile_name(&value) {
                return Some(value);
            }
            return None;
        }
    }
    None
}

fn strip_comment(line: &str) -> &str {
    let mut in_str = false;
    let mut escape = false;
    for (i, c) in line.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        match c {
            '\\' if in_str => escape = true,
            '"' => in_str = !in_str,
            '#' if !in_str => return &line[..i],
            _ => {}
        }
    }
    line
}

fn parse_string_value(s: &str) -> Option<String> {
    let s = s.trim();
    let inner = s.strip_prefix('"')?.strip_suffix('"')?;
    // Bare unescape: support \" and \\ which are the only escapes a
    // profile name could legally contain. Profile names are ASCII per
    // GENESIS §6 line 168, so we don't need full TOML unescape.
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next()? {
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                _ => return None,
            }
        } else {
            out.push(c);
        }
    }
    Some(out)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests"
)]
mod tests {
    use super::{
        extract_default_profile, parse_string_value, read_active_txt, read_default_profile,
        strip_comment,
    };

    #[test]
    fn active_txt_returns_none_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_active_txt(dir.path()).is_none());
    }

    #[test]
    fn active_txt_returns_first_line() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("active.txt"), "work\nignored\n").unwrap();
        assert_eq!(read_active_txt(dir.path()).as_deref(), Some("work"));
    }

    #[test]
    fn active_txt_returns_none_for_invalid_name() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("active.txt"), "-bad\n").unwrap();
        assert!(read_active_txt(dir.path()).is_none());
    }

    #[test]
    fn active_txt_returns_none_when_empty() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("active.txt"), "   \n").unwrap();
        assert!(read_active_txt(dir.path()).is_none());
    }

    #[test]
    fn extract_finds_simple_value() {
        let toml = "schema_version = 1\n[defaults]\nprofile = \"work\"\n";
        assert_eq!(extract_default_profile(toml).as_deref(), Some("work"));
    }

    #[test]
    fn extract_handles_whitespace() {
        let toml = "[defaults]\n   profile   =   \"personal\"   \n";
        assert_eq!(extract_default_profile(toml).as_deref(), Some("personal"));
    }

    #[test]
    fn extract_handles_comments() {
        let toml = "# top comment\n[defaults] # also ok\nprofile = \"work\" # inline\n";
        assert_eq!(extract_default_profile(toml).as_deref(), Some("work"));
    }

    #[test]
    fn extract_returns_none_without_defaults_table() {
        let toml = "schema_version = 1\n[profiles.work]\nharnesses = [\"claude\"]\n";
        assert!(extract_default_profile(toml).is_none());
    }

    #[test]
    fn extract_skips_other_tables() {
        let toml = "[profiles.work]\nprofile = \"not-this\"\n[defaults]\nprofile = \"this\"\n";
        assert_eq!(extract_default_profile(toml).as_deref(), Some("this"));
    }

    #[test]
    fn extract_rejects_invalid_name() {
        let toml = "[defaults]\nprofile = \"-bad\"\n";
        assert!(extract_default_profile(toml).is_none());
    }

    #[test]
    fn parse_string_value_basic() {
        assert_eq!(parse_string_value("\"hello\"").as_deref(), Some("hello"));
        assert_eq!(
            parse_string_value("  \"world\"  ").as_deref(),
            Some("world")
        );
    }

    #[test]
    fn parse_string_value_escapes() {
        assert_eq!(parse_string_value("\"a\\\"b\"").as_deref(), Some(r#"a"b"#));
    }

    #[test]
    fn parse_string_value_rejects_unterminated() {
        assert!(parse_string_value("\"hello").is_none());
        assert!(parse_string_value("hello").is_none());
    }

    #[test]
    fn strip_comment_in_string() {
        assert_eq!(strip_comment("profile = \"a#b\""), "profile = \"a#b\"");
    }

    #[test]
    fn strip_comment_outside_string() {
        assert_eq!(strip_comment("key = \"v\" # comment"), "key = \"v\" ");
    }

    #[test]
    fn read_default_profile_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("state.toml"),
            "schema_version = 1\n[defaults]\nprofile = \"personal\"\n",
        )
        .unwrap();
        assert_eq!(
            read_default_profile(dir.path()).as_deref(),
            Some("personal")
        );
    }
}
