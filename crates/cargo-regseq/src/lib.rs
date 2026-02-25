//! Library entry points for the `cargo-regseq` cargo subcommand.

use clap::Parser;
use thiserror::Error;

pub mod cli;
pub mod toolchain;

use crate::cli::CargoCli;

/// Error type for `cargo-regseq`.
#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Validation(String),

    #[error("{0}")]
    Toolchain(String),

    #[error("{0}")]
    Invoke(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Runs the `cargo-regseq` command orchestration.
pub fn run() -> Result<(), Error> {
    let CargoCli::Regseq(cli) = CargoCli::parse();
    cli.validate()?;

    let options = cli.into_compiler_options();
    let options_json = serde_json::to_string_pretty(&options)
        .map_err(|error| Error::Validation(format!("failed to serialize options: {error}")))?;
    eprintln!("regseq options: {options_json}");

    Ok(())
}
