//! `maru-core` — pure domain types and traits for the maru profile manager.
//!
//! See [GENESIS §6](https://github.com/itsgg/maru/blob/main/GENESIS.md) for
//! the full type and trait surface. Concrete types land per the
//! `/autopilot` task list as Phase 1 progresses.
#![forbid(unsafe_code)]

mod adapter;
mod adapter_types;
mod context;
mod diagnostic;
mod environment;
mod harness;
mod plan;
mod profile_name;
mod seed;

pub use adapter::HarnessAdapter;
pub use adapter_types::{AdapterError, Detection, ValidationReport};
pub use context::ProfileContext;
pub use diagnostic::{Diagnostic, Level};
pub use environment::{Environment, FakeEnvironment, SystemEnvironment};
pub use harness::{HarnessId, UnknownHarness};
pub use plan::ActivationPlan;
pub use profile_name::{InvalidName, ProfileName};
pub use seed::{MergeStrategy, SeedFile};
