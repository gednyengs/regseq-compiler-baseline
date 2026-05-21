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

use regseq_driver::bridge::{run_bridge, BridgeError, BridgeInput as DriverBridgeInput};
use regseq_model::{InvariantViolation, PrettyPrinter, RegseqModel};

/// Allow-listed `-Z` flag names that [`BuildOverrides::z_flags`]
/// entries may carry. Any `-Z` value outside this list causes the
/// override-aware helper workhorse to panic, per the D1 proposal at
/// `.harness/audits/2026-05-21-post-f4-triage.md:349-415`. The
/// allow-list is intentionally narrow; widening it is a separate
/// task with its own approval gate.
pub const Z_FLAG_ALLOWLIST: &[&str] = &["mir-emit-retag"];

/// Per-fixture build-flag overrides for the bridge helper.
///
/// All fields default to None / empty; the default value preserves
/// the helper's existing hard-wired behavior. The override surface
/// is intentionally narrow: only the two flags this proposal
/// authorizes (`overflow_checks` and the `-Z mir-emit-retag` flag)
/// are exposed here. Adding any further flag is a separate task
/// with its own approval gate per the F68 audit document's D1
/// proposal at `.harness/audits/2026-05-21-post-f4-triage.md:349-415`.
#[derive(Debug, Clone, Default)]
pub struct BuildOverrides {
    /// Overrides `config.opts.cg.overflow_checks` in the helper's
    /// `Callbacks::config` block. `None` preserves the existing
    /// hard-wired `Some(false)`. `Some(true)` re-enables runtime
    /// overflow checks, which is the documented requirement for
    /// TM-006-C `AssertKind::OverflowNeg` per
    /// `.harness/expectations/regseq-driver-67.md:264-278`.
    pub overflow_checks: Option<bool>,
    /// Allow-listed `-Z` flag names to append to the rustc args
    /// vec as `-Z <flag>` after the helper's existing
    /// `-C panic=abort`. Allow-list (initial drop):
    ///   - `"mir-emit-retag"`: exposes
    ///     `mir::StatementKind::Retag` to the bridge for RJ-006
    ///     per `.harness/expectations/regseq-driver-67.md:353-373`.
    ///
    /// Any value outside [`Z_FLAG_ALLOWLIST`] causes the helper
    /// workhorse to panic with the documented diagnostic. Adding
    /// values to the allow-list is a separate task per the
    /// No-Defensive-Fallbacks policy.
    pub z_flags: Vec<String>,
}

/// Typed result of a fixture compile + bridge + validate sweep.
///
/// `Ok` means BOTH bridge AND `model.validate()` succeeded.
/// Distinct rejection / validator-failure / rustc-rejected variants
/// are exposed so downstream tooling (specifically the
/// bridge-coverage-runner per
/// `docs/discussions/regseq-model/bridge-coverage-runner-strategy.md`,
/// phase F0) can classify outcomes without parsing error strings.
///
/// This is the typed surface introduced by regseq-driver-63 (F0
/// of the bridge-coverage-runner strategy). Existing callers
/// continue to use the legacy panicking [`bridge_fixture`] helper.
#[derive(Debug)]
pub enum BridgeOutcome {
    /// Bridge succeeded AND `model.validate()` succeeded.
    Ok(RegseqModel),
    /// Bridge succeeded but `model.validate()` failed. The model
    /// is returned alongside the violation so the caller can
    /// pretty-print or inspect.
    ValidatorFailed {
        /// Bridge-constructed model that failed validation.
        model: RegseqModel,
        /// Typed validator violation per `regseq_model::InvariantViolation`.
        violation: InvariantViolation,
    },
    /// Bridge returned a typed [`BridgeError`] (e.g.,
    /// `DslRejection`, `NonRigidTy`, `Phase4`, ...).
    Rejected(BridgeError),
    /// Rustc exited before `after_analysis` was reached -- either
    /// rustc rejected the input or the compile produced no result
    /// for an unknown reason. `stderr` is the rustc subprocess
    /// stderr, captured by the bridge-coverage runner. In this
    /// F0 helper, `stderr` is always the empty string; the
    /// runner adds subprocess-based stderr capture in F1
    /// (per strategy doc section 6.3).
    RustcRejected {
        /// Captured rustc stderr, or the empty string in this
        /// F0 helper.
        stderr: String,
    },
}

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
    /// Enum-layout round-trip fixture exercising every `EnumShape`
    /// arm (Empty / Single ordinary / Single fieldless `repr(int)` /
    /// Multiple Direct / Multiple Niche). Used by regseq-driver-54
    /// to lock the spec-contract `LayoutKind::Enum` shape through
    /// both `validate` and `render`.
    pub const ENUM_ROUND_TRIP: &str = "enum_round_trip";
    /// Instance-construction completeness fixture exercising every
    /// in-scope source kind (`static` / `static mut` / `const` /
    /// `struct` / generic `struct` / `enum` / `union`) plus the
    /// synth-`VTableType` path. Used by regseq-driver-55 to lock
    /// the Phase-5 Instance-construction passes + the
    /// Item-vs-Instance coverage validator through both `validate`
    /// and `render`.
    pub const INSTANCE_COMPLETENESS: &str = "instance_completeness";
    /// Static-dispatch trait-method callee-canonicalization fixture.
    /// Trait + concrete impl + caller that invokes the trait method
    /// on a concrete receiver. Used by regseq-driver-56 to lock the
    /// spec section 8.1 call-site canonicalization contract through
    /// both `validate` and `render`.
    pub const TRAIT_METHOD_RESOLUTION: &str = "trait_method_resolution";
    /// Foreign-decl rendering fixture: an `extern "C"` block with one
    /// foreign function and one foreign static, plus a source-defined
    /// `entry` fn. Used by regseq-driver-59 to lock the
    /// `#[foreign]\n...;` rendering shapes per pretty-printer.md
    /// Specimens 113 (foreign function) and 120 (foreign static).
    pub const FOREIGN_RENDERING: &str = "foreign_rendering";
    /// Default trait method body coverage fixture: a trait with one
    /// method that has a default body, a struct + empty impl, and a
    /// caller. Used by regseq-driver-59 to lock the Change H contract
    /// at spec.md section 8.1 -- the default body must surface as an
    /// `ItemKind::Function` Item at the trait's DefPath.
    pub const TRAIT_DEFAULT_METHOD: &str = "trait_default_method";
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
    match try_bridge_fixture(name) {
        BridgeOutcome::Ok(model) => model,
        BridgeOutcome::ValidatorFailed { violation, .. } => {
            panic!("bridge produced invalid RegseqModel for fixture {name:?}: {violation:?}");
        }
        BridgeOutcome::Rejected(err) => {
            panic!("bridge failed on fixture {name:?}: {err}");
        }
        BridgeOutcome::RustcRejected { .. } => {
            panic!(
                "bridge produced no result for fixture {name:?}: rustc invocation completed without reaching after_analysis"
            )
        }
    }
}

/// Typed entry point for fixtures already shipped under
/// `crates/test-ir-fixtures/fixtures/<name>.rs`.
///
/// Mirrors [`bridge_fixture`] but returns a [`BridgeOutcome`]
/// instead of panicking on non-OK paths. Panics ONLY when the
/// fixture name is unknown or the fixture file is unreadable
/// (those are caller-bugs, not bridge outcomes).
pub fn try_bridge_fixture(name: &str) -> BridgeOutcome {
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
    try_compile_and_bridge(name, &path)
}

/// Typed entry point for arbitrary source paths. Used by the
/// bridge-coverage runner to drive snippets that do NOT live in
/// the `test-ir-fixtures/fixtures/` directory.
///
/// Derives a diagnostic name from the file stem (used in panic
/// messages and the temp-dir name) and a rustc-valid synthetic
/// crate name from the same stem via [`sanitize_crate_name`].
/// The runner's snippet naming convention uses hyphenated row IDs
/// (e.g., `TY-001.rs`); rustc's `--crate-name` rejects hyphens, so
/// the helper sanitizes the stem before passing it through.
pub fn try_bridge_fixture_path(path: &Path) -> BridgeOutcome {
    try_bridge_fixture_path_with_overrides(path, &BuildOverrides::default())
}

/// Typed entry point for arbitrary source paths with caller-supplied
/// build-flag overrides. Mirrors [`try_bridge_fixture_path`] in
/// its diagnostic-name + crate-name derivation; the `overrides`
/// argument additionally threads the codegen flag + `-Z` flag
/// surface through to the rustc invocation per the D1 proposal at
/// `.harness/audits/2026-05-21-post-f4-triage.md:349-415`.
///
/// Passing `&BuildOverrides::default()` preserves the existing
/// hard-wired helper behavior bit-for-bit; that is how the
/// non-overriding entry points call into the shared workhorse.
pub fn try_bridge_fixture_path_with_overrides(
    path: &Path,
    overrides: &BuildOverrides,
) -> BridgeOutcome {
    let diagnostic_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("<anonymous>");
    let crate_name = sanitize_crate_name(diagnostic_name);
    try_compile_and_bridge_with_crate_name(diagnostic_name, &crate_name, path, overrides)
}

/// Map a free-form fixture stem to a rustc-valid crate name by
/// replacing every non-(ascii-alphanumeric-or-underscore) byte
/// with `_`, and prepending `s_` if the result starts with a
/// digit. Idempotent on already-valid names. Returns `"snippet"`
/// for empty input.
pub(crate) fn sanitize_crate_name(stem: &str) -> String {
    let mut out: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if out.is_empty() {
        return "snippet".to_string();
    }
    if out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert_str(0, "s_");
    }
    out
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

/// Compile a fixture source in-process with bridge-enabled rustc
/// callbacks and return a typed [`BridgeOutcome`]. The diagnostic
/// name and crate name coincide for fixture-name callers (existing
/// fixtures under `crates/test-ir-fixtures/fixtures/` use
/// rustc-valid stems).
fn try_compile_and_bridge(name: &str, src: &Path) -> BridgeOutcome {
    try_compile_and_bridge_with_crate_name(name, name, src, &BuildOverrides::default())
}

/// Compile a source file with a caller-supplied `crate_name`
/// passed to rustc's `--crate-name` flag. `diagnostic_name` is used
/// for the temp-dir name only; it may differ from `crate_name`
/// when the path-based entry point sanitizes a hyphenated stem.
/// `overrides` carries optional per-call codegen-flag and `-Z`
/// flag overrides; `&BuildOverrides::default()` preserves the
/// helper's existing hard-wired behavior.
fn try_compile_and_bridge_with_crate_name(
    diagnostic_name: &str,
    crate_name: &str,
    src: &Path,
    overrides: &BuildOverrides,
) -> BridgeOutcome {
    // Defense-in-depth allow-list enforcement: panic before any rustc
    // setup so an unrecognized override never reaches the compiler.
    // The runner's matrix loader is the first line of defense and
    // catches this at load time per the F68 D1 proposal; this panic
    // ensures direct programmatic callers cannot smuggle an
    // unrecognized flag past the loader.
    for flag in overrides.z_flags.iter() {
        if !Z_FLAG_ALLOWLIST.contains(&flag.as_str()) {
            panic!(
                "unrecognized -Z override {value:?}; allow-list = {allowlist:?}",
                value = flag,
                allowlist = Z_FLAG_ALLOWLIST,
            );
        }
    }

    let _lock = COMPILER_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    init_rustc_logger();

    let sysroot = host_sysroot();
    let out_dir = tempdir_for(diagnostic_name);

    let mut args = vec![
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
        crate_name.to_string(),
        "--out-dir".to_string(),
        out_dir
            .to_str()
            .expect("temporary out_dir path is not valid UTF-8")
            .to_string(),
        // Phase-1a body materialisation expects panic=abort cleanup-
        // block pruning per `bridge/body/mod.rs`. Without this flag
        // fixtures that emit MIR Drop / closure-call cleanup blocks
        // (anything past trivial signatures) retain unwind successors
        // that trip the body translator's panic=abort assertion.
        // Mirrors the production `RegseqCallbacks` shape consumed by
        // `cargo-regseq` and the in-tree `bridge_synth` / vtable
        // end-to-end harnesses.
        "-C".to_string(),
        "panic=abort".to_string(),
    ];

    // Append allow-listed `-Z` flags in their stored order so
    // diagnostic reproducibility tracks the caller's input ordering.
    for flag in overrides.z_flags.iter() {
        args.push("-Z".to_string());
        args.push(flag.clone());
    }

    let result: Arc<Mutex<Option<Result<RegseqModel, BridgeError>>>> = Arc::new(Mutex::new(None));

    struct Callbacks {
        result: Arc<Mutex<Option<Result<RegseqModel, BridgeError>>>>,
        overrides: BuildOverrides,
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
            config.opts.cg.overflow_checks = match self.overrides.overflow_checks {
                Some(v) => Some(v),
                None => Some(false),
            };
            config.opts.cg.debug_assertions = Some(false);
            config.opts.debug_assertions = false;
        }

        fn after_analysis<'tcx>(
            &mut self,
            _compiler: &rustc_interface::interface::Compiler,
            tcx: rustc_middle::ty::TyCtxt<'tcx>,
        ) -> rustc_driver::Compilation {
            let input = DriverBridgeInput::new(tcx);
            let outcome = run_bridge(input);
            *self.result.lock().expect("result mutex poisoned") = Some(outcome);
            rustc_driver::Compilation::Stop
        }
    }

    let mut callbacks = Callbacks {
        result: result.clone(),
        overrides: overrides.clone(),
    };
    rustc_driver::run_compiler(&args, &mut callbacks);

    let outcome = result.lock().expect("result mutex poisoned").take();
    let _ = std::fs::remove_dir_all(&out_dir);

    let mut model = match outcome {
        None => {
            return BridgeOutcome::RustcRejected {
                stderr: String::new(),
            };
        }
        Some(Err(err)) => {
            return BridgeOutcome::Rejected(err);
        }
        Some(Ok(model)) => model,
    };

    let bridge_input = regseq_model::BridgeInput::matching(&model);
    match model.validate(&bridge_input) {
        Ok(()) => BridgeOutcome::Ok(model),
        Err(violation) => BridgeOutcome::ValidatorFailed { model, violation },
    }
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
            fixture_names::ENUM_ROUND_TRIP,
            fixture_names::INSTANCE_COMPLETENESS,
            fixture_names::TRAIT_METHOD_RESOLUTION,
            fixture_names::FOREIGN_RENDERING,
            fixture_names::TRAIT_DEFAULT_METHOD,
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

    /// `sanitize_crate_name` replaces hyphens with underscores so
    /// the bridge-coverage runner's hyphenated row-id snippet
    /// stems (e.g. `TY-001`) lift to rustc-valid `--crate-name`
    /// arguments. Covers spec Change D item 4.
    #[test]
    fn sanitize_crate_name_rewrites_hyphen() {
        assert_eq!(sanitize_crate_name("TY-001"), "TY_001");
    }

    /// `sanitize_crate_name` prepends `s_` when the input begins
    /// with a digit (rustc rejects digit-leading crate names).
    /// Covers spec Change D item 4.
    #[test]
    fn sanitize_crate_name_prepends_s_on_digit_leading() {
        assert_eq!(sanitize_crate_name("123foo"), "s_123foo");
    }

    /// `sanitize_crate_name` is idempotent on already-valid
    /// names. Covers spec Change D item 4.
    #[test]
    fn sanitize_crate_name_idempotent_on_valid_name() {
        assert_eq!(sanitize_crate_name("no_op"), "no_op");
    }
}
