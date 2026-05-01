//! Helpers for human-vs-JSON output. GENESIS §8: "All output to stderr
//! unless `--json`, in which case the JSON document goes to stdout and
//! human messages to stderr."

use anyhow::Result;
use serde::Serialize;

/// Print a value either as pretty-printed JSON to stdout (when `json`)
/// or via the supplied human formatter to stdout.
pub fn emit<T: Serialize>(value: &T, json: bool, human: impl FnOnce(&T)) -> Result<()> {
    if json {
        let s = serde_json::to_string_pretty(value)?;
        println!("{s}");
    } else {
        human(value);
    }
    Ok(())
}
