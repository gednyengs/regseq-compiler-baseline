//! Library entry points for the `cargo-regseq` cargo subcommand.

use clap::Parser;
use thiserror::Error;

pub mod cli;
pub mod invoke;
pub mod toolchain;

use crate::cli::CargoCli;
use crate::invoke::invoke_cargo;
use crate::toolchain::ToolchainInfo;

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

    let cargo_args = cli.cargo_args.clone();
    let color = cli.color;
    let options = cli.into_compiler_options();
    let toolchain = ToolchainInfo::detect()?;

    invoke_cargo(&options, &toolchain, &cargo_args, color)
}
