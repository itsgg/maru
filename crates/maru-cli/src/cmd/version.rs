//! `maru version` — package version + build metadata.

use anyhow::Result;
use clap::Args;
use serde::Serialize;

#[derive(Debug, Args)]
pub struct VersionArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Serialize)]
struct VersionOutput {
    name: &'static str,
    version: &'static str,
    rust_version: &'static str,
}

pub fn run(args: VersionArgs) -> Result<()> {
    // The package name is `maru-cli` (the crate); the binary is `maru`.
    // User-facing output uses the binary name for consistency with
    // `maru --version` (clap default) and the `which maru` they typed.
    let out = VersionOutput {
        name: "maru",
        version: env!("CARGO_PKG_VERSION"),
        rust_version: env!("CARGO_PKG_RUST_VERSION"),
    };
    if args.json {
        let s = serde_json::to_string_pretty(&out)?;
        println!("{s}");
    } else {
        println!("{} {}", out.name, out.version);
    }
    Ok(())
}
