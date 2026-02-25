use std::path::{Path, PathBuf};
use std::process::Command;

use crate::Error;

/// Paths detected from the active pinned toolchain and local environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolchainInfo {
    /// Path to the rustc binary for the pinned nightly.
    pub rustc: PathBuf,
    /// Sysroot path for the pinned nightly.
    pub sysroot: PathBuf,
    /// Path to the regseq-driver binary.
    pub driver: PathBuf,
}

impl ToolchainInfo {
    /// Detect toolchain paths required by `cargo-regseq`.
    pub fn detect() -> Result<ToolchainInfo, Error> {
        let rustc = detect_rustc()?;
        let sysroot = detect_sysroot(&rustc)?;
        let driver = detect_driver()?;
        Ok(Self {
            rustc,
            sysroot,
            driver,
        })
    }
}

fn detect_rustc() -> Result<PathBuf, Error> {
    let stdout = run_checked_command(Command::new("rustup").args(["which", "rustc"]), "rustup")?;
    let rustc = PathBuf::from(stdout.trim());

    if !rustc.exists() || !rustc.is_file() {
        return Err(Error::Toolchain(format!(
            "`rustup which rustc` returned '{}', but it does not exist as a file",
            rustc.display()
        )));
    }

    Ok(rustc)
}

fn detect_sysroot(rustc: &Path) -> Result<PathBuf, Error> {
    let stdout = run_checked_command(
        Command::new(rustc).args(["--print", "sysroot"]),
        "rustc --print sysroot",
    )?;
    let sysroot = PathBuf::from(stdout.trim());

    if !sysroot.exists() || !sysroot.is_dir() {
        return Err(Error::Toolchain(format!(
            "`rustc --print sysroot` returned '{}', but it is not an existing directory",
            sysroot.display()
        )));
    }

    Ok(sysroot)
}

fn detect_driver() -> Result<PathBuf, Error> {
    if let Some(path) = std::env::var_os("REGSEQ_DRIVER") {
        let candidate = PathBuf::from(path);
        if candidate.exists() && candidate.is_file() {
            return Ok(candidate);
        }
        return Err(Error::Toolchain(format!(
            "REGSEQ_DRIVER is set to '{}', but that path is not an existing file",
            candidate.display()
        )));
    }

    let sibling = std::env::current_exe()
        .map_err(|error| {
            Error::Toolchain(format!("failed to resolve current executable: {error}"))
        })?
        .with_file_name(driver_binary_name());
    if sibling.exists() && sibling.is_file() {
        return Ok(sibling);
    }

    let output = Command::new("which")
        .arg(driver_binary_name())
        .output()
        .map_err(|error| {
            Error::Toolchain(format!("failed to execute `which regseq-driver`: {error}"))
        })?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let path = stdout.trim();
        if path.is_empty() {
            return Err(Error::Toolchain(
                "`which regseq-driver` succeeded but returned an empty path".to_string(),
            ));
        }
        let candidate = PathBuf::from(path);
        if candidate.exists() && candidate.is_file() {
            return Ok(candidate);
        }
        return Err(Error::Toolchain(format!(
            "`which regseq-driver` returned '{}', but it is not an existing file",
            candidate.display()
        )));
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(Error::Toolchain(format!(
        "could not find '{}' via REGSEQ_DRIVER, next to cargo-regseq ({}), or on PATH via `which`; stderr: {}",
        driver_binary_name(),
        sibling.display(),
        if stderr.is_empty() {
            "<empty>".to_string()
        } else {
            stderr
        }
    )))
}

fn run_checked_command(command: &mut Command, label: &str) -> Result<String, Error> {
    let output = command
        .output()
        .map_err(|error| Error::Toolchain(format!("failed to execute `{label}`: {error}")))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(Error::Toolchain(format!(
            "`{label}` failed with status {}: {}",
            output.status,
            if stderr.is_empty() {
                "<empty>".to_string()
            } else {
                stderr
            }
        )))
    }
}

fn driver_binary_name() -> &'static str {
    if cfg!(windows) {
        "regseq-driver.exe"
    } else {
        "regseq-driver"
    }
}
