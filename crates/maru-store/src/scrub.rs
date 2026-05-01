//! Value-level credential scrubbing for `settings.json` and `config.toml`.
//!
//! GENESIS §8 (verbatim): "Value-level scrubbing (file is included, but
//! matching nested values are replaced with the literal string
//! `\"<scrubbed by maru>\"`): in `settings.json` / `config.toml`, any
//! key path containing a segment that case-insensitively matches
//! `(token|api[_-]?key|secret|password|bearer|credential)` has its
//! scalar value replaced. Nested tables are recursed; non-scalar values
//! are dropped."
//!
//! This module pairs with [`crate::deny_list`] (file-level exclusion).
//! Files matching the deny-list never reach scrubbing because they're
//! not copied at all. Files that *are* copied — `settings.json`,
//! `config.toml` — go through this module to redact secrets that may
//! sit nested inside otherwise benign config.

use crate::Error;

/// Sentinel value written in place of a scrubbed scalar.
pub const SCRUBBED_PLACEHOLDER: &str = "<scrubbed by maru>";

/// Scrub a JSON document (typically a Claude `settings.json` or Gemini
/// `settings.json`).
///
/// Keys are matched case-insensitively at each segment; any nested key
/// whose name contains any of the §8 patterns has its value replaced
/// with the [`SCRUBBED_PLACEHOLDER`] sentinel (if scalar) or dropped (if
/// non-scalar / object / array).
///
/// The output is pretty-printed JSON with the same top-level shape,
/// minus offending sub-trees.
///
/// # Errors
///
/// Returns [`Error::Decode`] if `text` is not valid JSON, or
/// [`Error::Encode`] on serialization failure.
pub fn scrub_settings_json(text: &str) -> Result<String, Error> {
    let mut value: serde_json::Value = serde_json::from_str(text).map_err(|e| Error::Decode {
        path: std::path::PathBuf::from("settings.json"),
        message: format!("invalid JSON: {e}"),
    })?;
    scrub_json_value(&mut value);
    serde_json::to_string_pretty(&value).map_err(|e| Error::Encode {
        message: format!("serialize scrubbed JSON: {e}"),
    })
}

/// Scrub a TOML document (typically a Codex `config.toml`).
///
/// Same matching rules as [`scrub_settings_json`], applied to TOML keys
/// at any depth.
///
/// # Errors
///
/// Returns [`Error::Decode`] if `text` is not valid TOML, or
/// [`Error::Encode`] on serialization failure.
pub fn scrub_config_toml(text: &str) -> Result<String, Error> {
    let mut value: toml::Value = text.parse().map_err(|e| Error::Decode {
        path: std::path::PathBuf::from("config.toml"),
        message: format!("invalid TOML: {e}"),
    })?;
    scrub_toml_value(&mut value);
    toml::to_string(&value).map_err(|e| Error::Encode {
        message: format!("serialize scrubbed TOML: {e}"),
    })
}

/// Returns `true` if `key` (a single key segment) case-insensitively
/// contains any of the §8 sensitive patterns.
fn key_is_sensitive(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    if lower.contains("token")
        || lower.contains("secret")
        || lower.contains("password")
        || lower.contains("bearer")
        || lower.contains("credential")
    {
        return true;
    }
    // `api[_-]?key`: "apikey", "api_key", "api-key".
    if lower.contains("apikey") || lower.contains("api_key") || lower.contains("api-key") {
        return true;
    }
    false
}

/// Recursively scrub a JSON value. Sensitive scalars are replaced with
/// [`SCRUBBED_PLACEHOLDER`]; sensitive non-scalars (objects / arrays)
/// are removed entirely.
fn scrub_json_value(v: &mut serde_json::Value) {
    if let serde_json::Value::Object(map) = v {
        let to_drop: Vec<String> = map
            .iter()
            .filter(|(k, val)| key_is_sensitive(k) && !is_json_scalar(val))
            .map(|(k, _)| k.clone())
            .collect();
        for k in to_drop {
            map.remove(&k);
        }
        let to_redact: Vec<String> = map
            .iter()
            .filter(|(k, val)| key_is_sensitive(k) && is_json_scalar(val))
            .map(|(k, _)| k.clone())
            .collect();
        for k in to_redact {
            map.insert(
                k,
                serde_json::Value::String(SCRUBBED_PLACEHOLDER.to_owned()),
            );
        }

        for (_, val) in map.iter_mut() {
            scrub_json_value(val);
        }
        return;
    }
    if let serde_json::Value::Array(items) = v {
        for item in items.iter_mut() {
            scrub_json_value(item);
        }
    }
}

const fn is_json_scalar(v: &serde_json::Value) -> bool {
    matches!(
        v,
        serde_json::Value::Null
            | serde_json::Value::Bool(_)
            | serde_json::Value::Number(_)
            | serde_json::Value::String(_)
    )
}

/// Recursively scrub a TOML value with the same semantics as
/// [`scrub_json_value`].
fn scrub_toml_value(v: &mut toml::Value) {
    if let toml::Value::Table(map) = v {
        let to_drop: Vec<String> = map
            .iter()
            .filter(|(k, val)| key_is_sensitive(k) && !is_toml_scalar(val))
            .map(|(k, _)| k.clone())
            .collect();
        for k in to_drop {
            map.remove(&k);
        }
        let to_redact: Vec<String> = map
            .iter()
            .filter(|(k, val)| key_is_sensitive(k) && is_toml_scalar(val))
            .map(|(k, _)| k.clone())
            .collect();
        for k in to_redact {
            map.insert(k, toml::Value::String(SCRUBBED_PLACEHOLDER.to_owned()));
        }
        for (_, val) in map.iter_mut() {
            scrub_toml_value(val);
        }
        return;
    }
    if let toml::Value::Array(items) = v {
        for item in items.iter_mut() {
            scrub_toml_value(item);
        }
    }
}

const fn is_toml_scalar(v: &toml::Value) -> bool {
    matches!(
        v,
        toml::Value::String(_)
            | toml::Value::Integer(_)
            | toml::Value::Float(_)
            | toml::Value::Boolean(_)
            | toml::Value::Datetime(_)
    )
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
    use super::{SCRUBBED_PLACEHOLDER, key_is_sensitive, scrub_config_toml, scrub_settings_json};

    #[test]
    fn key_pattern_matches_documented_terms() {
        for k in [
            "token",
            "Token",
            "api_key",
            "API-KEY",
            "apikey",
            "secret",
            "PASSWORD",
            "bearer",
            "credential",
            "anthropic_api_key",
            "GitHubToken",
        ] {
            assert!(key_is_sensitive(k), "{k:?} should be sensitive");
        }
    }

    #[test]
    fn key_pattern_does_not_match_benign() {
        for k in ["theme", "ui", "font", "model", "version", "mcpServers"] {
            assert!(!key_is_sensitive(k), "{k:?} should be benign");
        }
    }

    #[test]
    fn json_scrubs_top_level_scalar_value() {
        let input = r#"{ "anthropic_api_key": "sk-deadbeef", "ui": { "theme": "dark" } }"#;
        let out = scrub_settings_json(input).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["anthropic_api_key"], SCRUBBED_PLACEHOLDER);
        assert_eq!(v["ui"]["theme"], "dark");
        assert!(!out.contains("sk-deadbeef"), "raw secret leaked: {out}");
    }

    #[test]
    fn json_scrubs_nested_table_dropped() {
        let input = r#"{ "tokens": { "github": "ghp_abc123" } }"#;
        let out = scrub_settings_json(input).unwrap();
        // `tokens` is a non-scalar matching the pattern → dropped entirely.
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(
            v.get("tokens").is_none(),
            "non-scalar tokens table should be dropped: {v}"
        );
        assert!(!out.contains("ghp_abc123"), "raw token leaked: {out}");
    }

    #[test]
    fn json_scrubs_value_inside_benign_object() {
        // ui is benign; password inside it is sensitive.
        let input = r#"{ "ui": { "theme": "dark", "password": "hunter2" } }"#;
        let out = scrub_settings_json(input).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["ui"]["theme"], "dark");
        assert_eq!(v["ui"]["password"], SCRUBBED_PLACEHOLDER);
        assert!(!out.contains("hunter2"));
    }

    #[test]
    fn json_preserves_benign_keys_verbatim() {
        let input = r#"{ "ui": { "theme": "dark" }, "model": "claude-opus" }"#;
        let out = scrub_settings_json(input).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["ui"]["theme"], "dark");
        assert_eq!(v["model"], "claude-opus");
    }

    #[test]
    fn toml_scrubs_scalar_under_table() {
        let input = "[auth]\napi_key = \"sk-deadbeef\"\n";
        let out = scrub_config_toml(input).unwrap();
        let v: toml::Value = out.parse().unwrap();
        assert_eq!(v["auth"]["api_key"].as_str(), Some(SCRUBBED_PLACEHOLDER));
        assert!(!out.contains("sk-deadbeef"), "raw secret leaked: {out}");
    }

    #[test]
    fn toml_drops_non_scalar_match() {
        // tokens is a table → matched as sensitive non-scalar → dropped.
        let input = "[tokens]\ngithub = \"ghp_abc\"\n[ui]\ntheme = \"dark\"\n";
        let out = scrub_config_toml(input).unwrap();
        let v: toml::Value = out.parse().unwrap();
        assert!(v.get("tokens").is_none(), "tokens table should be dropped");
        assert_eq!(v["ui"]["theme"].as_str(), Some("dark"));
        assert!(!out.contains("ghp_abc"));
    }

    #[test]
    fn toml_preserves_benign_root() {
        let input = "model = \"claude-opus\"\n[ui]\ntheme = \"dark\"\n";
        let out = scrub_config_toml(input).unwrap();
        let v: toml::Value = out.parse().unwrap();
        assert_eq!(v["model"].as_str(), Some("claude-opus"));
        assert_eq!(v["ui"]["theme"].as_str(), Some("dark"));
    }

    #[test]
    fn json_invalid_returns_decode_error() {
        let err = scrub_settings_json("{not valid json").unwrap_err();
        match err {
            crate::Error::Decode { .. } => {}
            other => panic!("expected Decode, got {other:?}"),
        }
    }

    #[test]
    fn toml_invalid_returns_decode_error() {
        let err = scrub_config_toml("not = valid = toml").unwrap_err();
        match err {
            crate::Error::Decode { .. } => {}
            other => panic!("expected Decode, got {other:?}"),
        }
    }
}
