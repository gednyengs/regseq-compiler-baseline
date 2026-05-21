//! Typed bridge invocation per strategy section 6.2 (panic capture
//! via `catch_unwind`) + section 6.3 (subprocess rustc pre-flight +
//! in-process bridge run).
//!
//! Algorithm:
//!  1. Shell out to the active `rustc` with `--emit=metadata
//!     --crate-type lib --edition 2021` to capture stderr cleanly.
//!     This catches malformed Rust before the in-process bridge sees
//!     it, lets the runner classify clean rejections vs. ICEs.
//!  2. On rustc success, invoke [`test_ir_fixtures::try_bridge_fixture_path`]
//!     inside `std::panic::catch_unwind` so bridge panics surface as
//!     [`OracleOutcome::Panic`].
//!
//! Strategy section 6.3 names `-Zno-codegen` as the pre-flight
//! flag; F1 prefers `--emit=metadata` because it is a stable rustc
//! flag and produces metadata-only output without exercising
//! `-Z`-unstable paths.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use regseq_driver::bridge::BridgeError;
use regseq_model::{InvariantViolation, RegseqModel};
use test_ir_fixtures::{try_bridge_fixture_path_with_overrides, BridgeOutcome, BuildOverrides};

/// Typed oracle outcome surfaced to the run loop.
///
/// The `Ok` variant boxes [`RegseqModel`] because the model is the
/// largest single payload the bridge can produce; the boxed shape
/// keeps the enum small for the rest of the run loop.
#[derive(Debug)]
pub enum OracleOutcome {
    /// Bridge succeeded AND `model.validate()` succeeded.
    Ok(Box<RegseqModel>),
    /// Bridge succeeded but the validator surfaced a violation.
    ValidatorFailed {
        /// Validator violation.
        violation: InvariantViolation,
    },
    /// Bridge rejected the input with a typed [`BridgeError`].
    Rejected(BridgeError),
    /// Rustc subprocess exited non-zero with a clean (non-ICE)
    /// rejection. Stderr carried no ICE marker.
    RustcCleanReject {
        /// Captured rustc stderr.
        stderr: String,
    },
    /// Rustc subprocess panicked or emitted an internal compiler
    /// error.
    RustcICE {
        /// Captured rustc stderr.
        stderr: String,
    },
    /// Rustc subprocess accepted the input but the in-process
    /// bridge invocation completed without reaching `after_analysis`
    /// (e.g., empty crate). Strategy section 6.3 third sub-class.
    RustcNoAfterAnalysis,
    /// Bridge panicked; captured payload as a string.
    Panic {
        /// Best-effort panic message extracted from the payload.
        message: String,
    },
}

/// Run the two-step oracle against `snippet_path`. The optional
/// `overrides` argument forwards per-row build-flag overrides to
/// the in-process bridge helper via
/// [`try_bridge_fixture_path_with_overrides`] per the D1 proposal at
/// `.harness/audits/2026-05-21-post-f4-triage.md:349-415`. `None`
/// is functionally equivalent to passing `&BuildOverrides::default()`
/// and preserves pre-D1 helper behavior.
///
/// Pre-flight rustc metadata emission is NOT override-aware: the
/// authorized overrides (`overflow_checks` + `-Z mir-emit-retag`)
/// only affect MIR-build downstream of metadata, so the pre-flight
/// classification stays identical regardless of overrides.
pub fn invoke(snippet_path: &Path, overrides: Option<&BuildOverrides>) -> OracleOutcome {
    match rustc_preflight(snippet_path) {
        PreflightOutcome::Ok => {}
        PreflightOutcome::CleanReject { stderr } => {
            return OracleOutcome::RustcCleanReject { stderr };
        }
        PreflightOutcome::Ice { stderr } => {
            return OracleOutcome::RustcICE { stderr };
        }
    }

    let path_owned = snippet_path.to_path_buf();
    let overrides_owned = overrides.cloned().unwrap_or_default();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        try_bridge_fixture_path_with_overrides(&path_owned, &overrides_owned)
    }));
    match result {
        Err(payload) => OracleOutcome::Panic {
            message: panic_message(payload),
        },
        Ok(BridgeOutcome::Ok(model)) => OracleOutcome::Ok(Box::new(model)),
        Ok(BridgeOutcome::ValidatorFailed { violation, .. }) => {
            OracleOutcome::ValidatorFailed { violation }
        }
        Ok(BridgeOutcome::Rejected(err)) => OracleOutcome::Rejected(err),
        Ok(BridgeOutcome::RustcRejected { .. }) => OracleOutcome::RustcNoAfterAnalysis,
    }
}

enum PreflightOutcome {
    Ok,
    CleanReject { stderr: String },
    Ice { stderr: String },
}

fn rustc_preflight(snippet_path: &Path) -> PreflightOutcome {
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| OsStr::new("rustc").into());
    let tmp = preflight_tmp_path(snippet_path);

    let output = Command::new(&rustc)
        .arg("--crate-type")
        .arg("lib")
        .arg("--edition")
        .arg("2021")
        .arg("--emit=metadata")
        .arg("-o")
        .arg(&tmp)
        .arg(snippet_path)
        .output();

    let _ = fs::remove_file(&tmp);

    let output = match output {
        Ok(o) => o,
        Err(err) => {
            return PreflightOutcome::Ice {
                stderr: format!("failed to spawn rustc subprocess: {err}"),
            };
        }
    };

    if output.status.success() {
        return PreflightOutcome::Ok;
    }
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if stderr.contains("internal compiler error") || stderr.contains("thread 'rustc' panicked") {
        PreflightOutcome::Ice { stderr }
    } else {
        PreflightOutcome::CleanReject { stderr }
    }
}

/// Deterministic per-snippet temp path so the runner does not need
/// the `tempfile` crate dep. Snippets are processed sequentially
/// per strategy section 10 Mode A, so the simple shape is safe.
fn preflight_tmp_path(snippet_path: &Path) -> PathBuf {
    let stem = snippet_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("snippet");
    let sanitized: String = stem
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    std::env::temp_dir().join(format!(
        "bridge_coverage_runner_preflight_{sanitized}.rmeta"
    ))
}

/// Standard panic-payload extraction: `&'static str` or `String`,
/// fall back to a generic message.
fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        return (*s).to_string();
    }
    if let Some(s) = payload.downcast_ref::<String>() {
        return s.clone();
    }
    "<unknown panic payload>".to_string()
}
