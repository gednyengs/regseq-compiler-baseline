//! Shared Rust IR fixtures + bridge helpers for phase-1a end-to-end
//! integration tests.
//!
//! This crate is a workspace-internal test-support crate (per the
//! `test-support-crate` rule in `.harness/README.md`). It ships the
//! canonical phase-1a Rust source fixtures (under `fixtures/`) plus a
//! helper API that downstream integration tests use to drive the
//! `regseq-driver` bridge against each fixture and obtain the
//! resulting [`RegseqModel`] for assertion.
//!
//! The helper runs `rustc` in-process against the fixture file via
//! `rustc_driver::run_compiler`, then invokes
//! [`regseq_driver::bridge::run_bridge`] inside the after-analysis
//! callback to construct the `RegseqModel`. Per spec section 25.7 the
//! bridge runs the full five-phase pipeline before returning.

#![warn(missing_docs)]
#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, Once};

use regseq_driver::bridge::{run_bridge, BridgeInput as DriverBridgeInput};
use regseq_model::{PrettyPrinter, RegseqModel};

/// Names of the canonical phase-1a fixtures shipped by this crate.
/// Each name maps to a `.rs` source file under `fixtures/`.
pub mod fixture_names {
    /// `fn no_op() {}` -- the minimal smoke fixture.
    pub const NO_OP: &str = "no_op";
    /// Calls `regseq::log::{debug,info,warn,error}!(...)` once each
    /// with one `u32` argument. Used by P1a-E2E-02.
    pub const DIAG_LOGGING: &str = "diag_logging";
    /// `dyn Reader` trait-object call site. Used by P1a-E2E-03.
    pub const TRAIT_OBJECT: &str = "trait_object";
    /// `|x| x + 1` closure expression. Used by P1a-E2E-03.
    pub const CLOSURE: &str = "closure";
}

/// Drive the phase-1a `regseq-driver` bridge against the named
/// fixture under `crates/test-ir-fixtures/fixtures/<name>.rs` and
/// return the constructed `RegseqModel`. Panics with a descriptive
/// message if the fixture name is unknown, the fixture file is
/// missing, the bridge errors, or the bridge produces an
/// `InvariantViolation` from `RegseqModel::validate`.
///
/// The helper compiles the fixture under the workspace's pinned
/// rustc toolchain (per `rust-toolchain.toml`) using the `regseq-driver`
/// binary entry point and parses the bridge's serialised output back
/// into an in-memory `RegseqModel`. See `bridge_fixture_with_options`
/// for the full configuration surface.
pub fn bridge_fixture(name: &str) -> RegseqModel {
    let path = fixture_path(name);
    let metadata = std::fs::metadata(&path).unwrap_or_else(|err| {
        panic!(
            "fixture {name:?} is not shipped by test-ir-fixtures (expected at {}): {err}",
            path.display()
        )
    });
    if !metadata.is_file() {
        panic!(
            "fixture {name:?} entry at {} is not a regular file",
            path.display()
        );
    }
    compile_and_bridge(name, &path)
}

/// Pretty-print the model with default `PrettyPrintOptions`
/// (`include_spans: false`, `include_debug_names: false`) per
/// `pretty-printer.md` §4.1 default mode. Convenience wrapper for
/// integration-test snapshot assertions.
pub fn render_default(model: &RegseqModel) -> String {
    PrettyPrinter::new(model).render()
}

/// Absolute path to a fixture's `.rs` source file.
fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(format!("{name}.rs"))
}

// Rustc state lives in process-global statics; the rustc logger may
// only be initialised once and `run_compiler` is not safe to invoke
// concurrently from multiple threads. The helper serialises bridge
// invocations behind a process-wide mutex so cargo's parallel test
// runner cannot trigger reentrant compilation.
static RUSTC_LOGGER_INIT: Once = Once::new();
static COMPILER_LOCK: Mutex<()> = Mutex::new(());

fn init_rustc_logger() {
    RUSTC_LOGGER_INIT.call_once(|| {
        let early_dcx =
            rustc_session::EarlyDiagCtxt::new(rustc_session::config::ErrorOutputType::default());
        rustc_driver::init_rustc_env_logger(&early_dcx);
    });
}

/// Resolve the host rustc sysroot by shelling out to the active
/// `rustc` binary (per `rust-toolchain.toml`).
fn host_sysroot() -> String {
    let output = Command::new("rustc")
        .arg("--print")
        .arg("sysroot")
        .output()
        .expect("failed to spawn `rustc --print sysroot`");
    if !output.status.success() {
        panic!(
            "`rustc --print sysroot` failed: status={:?} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stderr),
        );
    }
    String::from_utf8(output.stdout)
        .expect("rustc sysroot output is not valid UTF-8")
        .trim()
        .to_string()
}

/// Compile the fixture source in-process with bridge-enabled rustc
/// callbacks and return the resulting `RegseqModel`.
fn compile_and_bridge(name: &str, src: &Path) -> RegseqModel {
    let _lock = COMPILER_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    init_rustc_logger();

    let sysroot = host_sysroot();
    let out_dir = tempdir_for(name);

    let args = vec![
        "rustc".to_string(),
        src.to_str()
            .expect("fixture path is not valid UTF-8")
            .to_string(),
        "--sysroot".to_string(),
        sysroot,
        "--edition".to_string(),
        "2021".to_string(),
        "--crate-type".to_string(),
        "rlib".to_string(),
        "--crate-name".to_string(),
        name.to_string(),
        "--out-dir".to_string(),
        out_dir
            .to_str()
            .expect("temporary out_dir path is not valid UTF-8")
            .to_string(),
    ];

    let result: Arc<Mutex<Option<Result<RegseqModel, String>>>> = Arc::new(Mutex::new(None));

    struct Callbacks {
        result: Arc<Mutex<Option<Result<RegseqModel, String>>>>,
    }

    impl rustc_driver::Callbacks for Callbacks {
        fn config(&mut self, config: &mut rustc_interface::interface::Config) {
            // Match `RegseqCallbacks::config` shape (see
            // `crates/regseq-driver/src/callbacks.rs`): no codegen,
            // mir_opt_level=0 + always_encode_mir so the bridge sees
            // every body, the regseq pass whitelist, and runtime
            // checks disabled.
            config.opts.unstable_opts.no_codegen = true;
            config.opts.unstable_opts.mir_opt_level = Some(0);
            config.opts.unstable_opts.always_encode_mir = true;
            config.opts.unstable_opts.mir_enable_passes = regseq_driver::regseq_mir_passes();
            config.opts.cg.overflow_checks = Some(false);
            config.opts.cg.debug_assertions = Some(false);
            config.opts.debug_assertions = false;
        }

        fn after_analysis<'tcx>(
            &mut self,
            _compiler: &rustc_interface::interface::Compiler,
            tcx: rustc_middle::ty::TyCtxt<'tcx>,
        ) -> rustc_driver::Compilation {
            let input = DriverBridgeInput::new(tcx);
            let outcome = match run_bridge(input) {
                Ok(model) => Ok(model),
                Err(err) => Err(format!("{err}")),
            };
            *self.result.lock().expect("result mutex poisoned") = Some(outcome);
            rustc_driver::Compilation::Stop
        }
    }

    let mut callbacks = Callbacks {
        result: result.clone(),
    };
    rustc_driver::run_compiler(&args, &mut callbacks);

    let outcome = result
        .lock()
        .expect("result mutex poisoned")
        .take()
        .unwrap_or_else(|| {
            panic!(
                "bridge produced no result for fixture {name:?}: rustc invocation completed without reaching after_analysis"
            )
        });

    let _ = std::fs::remove_dir_all(&out_dir);

    let mut model = match outcome {
        Ok(model) => model,
        Err(message) => {
            panic!("bridge failed on fixture {name:?}: {message}")
        }
    };

    let bridge_input = regseq_model::BridgeInput::matching(&model);
    if let Err(err) = model.validate(&bridge_input) {
        panic!("bridge produced invalid RegseqModel for fixture {name:?}: {err:?}");
    }
    model
}

/// Unique temp directory for a fixture compile, keyed by the fixture
/// name and an os-level timestamp to avoid races between parallel
/// test runs (cargo executes each `tests/*.rs` in its own process,
/// but the same fixture may also be exercised by multiple tests).
fn tempdir_for(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("test-ir-fixtures-{name}-{nanos}"));
    std::fs::create_dir_all(&dir).expect("failed to create fixture temp dir");
    dir
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
    }

    /// Each named fixture resolves to a readable, non-empty file
    /// under `fixtures/`. Covers Test Plan item 3 (fixture files
    /// present and well-formed).
    #[test]
    fn fixture_files_present_and_well_formed() {
        for name in [
            fixture_names::NO_OP,
            fixture_names::DIAG_LOGGING,
            fixture_names::TRAIT_OBJECT,
            fixture_names::CLOSURE,
        ] {
            let path = fixture_dir().join(format!("{name}.rs"));
            let metadata = std::fs::metadata(&path).unwrap_or_else(|err| {
                panic!("fixture {name} missing at {}: {err}", path.display())
            });
            assert!(
                metadata.is_file(),
                "fixture {name} at {} must be a file",
                path.display()
            );
            assert!(
                metadata.len() > 0,
                "fixture {name} at {} must be non-empty",
                path.display()
            );
        }
    }

    /// Invoking the helper with a name that does not match any
    /// shipped fixture panics with a message that names the missing
    /// fixture. Covers Test Plan item 4 (helper failure path is
    /// descriptive).
    #[test]
    fn bridge_fixture_unknown_name_panics_with_descriptive_message() {
        let missing_name = "does_not_exist";
        let result = std::panic::catch_unwind(|| {
            bridge_fixture(missing_name);
        });
        let payload = result.expect_err("bridge_fixture must panic on unknown fixture name");
        let message = payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| {
                payload
                    .downcast_ref::<&'static str>()
                    .map(|s| s.to_string())
            })
            .unwrap_or_default();
        assert!(
            message.contains(missing_name),
            "panic message must name the missing fixture; got: {message:?}"
        );
    }
}
