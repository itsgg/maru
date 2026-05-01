//! `maru schema` — emit a stable JSON schema describing the on-disk
//! `state.toml` and every `--json` output. GENESIS §8.
//!
//! v1 is hand-rolled (no `schemars` dep yet). It documents every field
//! the CLI emits. CI snapshot-tests this output to detect drift.

use anyhow::Result;
use serde_json::json;

pub fn run() -> Result<()> {
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://maru.dev/schema/v1",
        "title": "maru v1 schema",
        "description": "On-disk state.toml shape + every CLI --json output. Bumping this requires bumping schema_version per GENESIS §10.",
        "version": 1,
        "definitions": {
            "ProfileEntry": {
                "type": "object",
                "required": ["created_at", "last_used_at", "harnesses", "ever_activated"],
                "properties": {
                    "created_at": {"type": "string", "format": "date-time"},
                    "last_used_at": {"type": "string", "format": "date-time"},
                    "harnesses": {
                        "type": "array",
                        "items": {"type": "string", "enum": ["claude", "codex", "gemini"]}
                    },
                    "ever_activated": {"type": "boolean"}
                }
            },
            "State": {
                "type": "object",
                "required": ["schema_version"],
                "properties": {
                    "schema_version": {"type": "integer", "const": 1},
                    "profiles": {
                        "type": "object",
                        "additionalProperties": {"$ref": "#/definitions/ProfileEntry"}
                    },
                    "defaults": {
                        "type": "object",
                        "properties": {
                            "profile": {"type": ["string", "null"]}
                        }
                    }
                }
            },
            "ListOutput": {
                "type": "object",
                "properties": {
                    "profiles": {"type": "array", "items": {"type": "object"}},
                    "active": {"type": ["string", "null"]},
                    "default": {"type": ["string", "null"]}
                }
            },
            "DryRunOutput": {
                "type": "object",
                "properties": {
                    "profile": {"type": "string"},
                    "harness": {"type": "string"},
                    "env": {"type": "array"},
                    "args_prefix": {"type": "array"},
                    "diagnostics": {"type": "array"}
                }
            }
        }
    });
    let s = serde_json::to_string_pretty(&schema)?;
    println!("{s}");
    Ok(())
}
