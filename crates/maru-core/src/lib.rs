//! `maru-core` — pure domain types and traits for the maru profile manager.
//!
//! See [GENESIS §6](https://github.com/itsgg/maru/blob/main/GENESIS.md) for
//! the full type and trait surface. Concrete types land per the
//! `/autopilot` task list as Phase 1 progresses.
#![forbid(unsafe_code)]

mod profile_name;

pub use profile_name::{InvalidName, ProfileName};
