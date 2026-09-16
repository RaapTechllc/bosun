//! `bosunctl` — the Bosun control CLI.
//!
//! M1 scope. Device identity comes from a TOML descriptor or explicit
//! match flags. The default descriptor path is the committed G13 data file;
//! VID/PID are not compiled into this binary.

use anyhow::Result;
use bosunctl::{run, Cli};
use clap::Parser;
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    // Logs go to stderr so stdout stays a clean, pipeable listing.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    run(Cli::parse())
}
