//! Library entry points for the `cargo-regseq` cargo subcommand.

use thiserror::Error;

/// Placeholder library error type for `cargo-regseq`.
#[derive(Debug, Error)]
#[error("cargo-regseq failed")]
pub struct Error;

/// Runs the `cargo-regseq` command orchestration.
pub fn run() -> Result<(), Error> {
    Ok(())
}
