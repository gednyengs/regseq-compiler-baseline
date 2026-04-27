use std::process::{Command, Stdio};

use regseq_config::{ColorChoice, CompilerOptions};

use crate::toolchain::ToolchainInfo;
use crate::Error;

/// Build the `cargo check` command used to run Regseq compilation.
pub fn build_cargo_command(
    options: &CompilerOptions,
    toolchain: &ToolchainInfo,
    cargo_args: &[String],
    color: ColorChoice,
) -> Result<Command, Error> {
    let options_json = serde_json::to_string(options).map_err(|error| {
        Error::Invoke(format!(
            "failed to serialize CompilerOptions to JSON: {error}"
        ))
    })?;

    let mut command = Command::new("cargo");
    command.arg("check");
    command.env_remove("CARGO_PRIMARY_PACKAGE");
    command.env("RUSTC_WRAPPER", &toolchain.driver);
    command.env("REGSEQ_OPTIONS", options_json);
    command.env("REGSEQ_SYSROOT", &toolchain.sysroot);

    if !cargo_args_contain_color(cargo_args) {
        command.arg("--color");
        command.arg(color.to_string());
    }

    if !cargo_args_contain_target(cargo_args) {
        command.arg("--target");
        command.arg(&toolchain.host);
    }

    command.args(cargo_args);
    Ok(command)
}

/// Invoke cargo with Regseq wrapper environment configured.
pub fn invoke_cargo(
    options: &CompilerOptions,
    toolchain: &ToolchainInfo,
    cargo_args: &[String],
    color: ColorChoice,
) -> Result<(), Error> {
    std::fs::create_dir_all(&options.output_dir)?;

    let mut command = build_cargo_command(options, toolchain, cargo_args, color)?;
    command.stdout(Stdio::inherit());
    command.stderr(Stdio::inherit());

    let status = command
        .status()
        .map_err(|error| Error::Invoke(format!("failed to launch cargo: {error}")))?;

    if status.success() {
        Ok(())
    } else {
        Err(Error::Invoke(format!(
            "cargo check failed with status {status}"
        )))
    }
}

fn cargo_args_contain_color(cargo_args: &[String]) -> bool {
    cargo_args
        .iter()
        .any(|arg| arg == "--color" || arg.starts_with("--color="))
}

fn cargo_args_contain_target(cargo_args: &[String]) -> bool {
    cargo_args
        .iter()
        .any(|arg| arg == "--target" || arg.starts_with("--target="))
}
