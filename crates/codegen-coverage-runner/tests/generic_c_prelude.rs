//! Behavioral tests for the generic-C backend bank-1 prelude + helper block
//! (regseq-backends-05, Change E / B2 / B3). These live in the runner crate
//! because it owns `toolchain::compile_and_run` -- the backend's host-`cc`
//! verification harness. Each test emits the real `emit_generic_c` prelude +
//! a hand-written C exerciser and asserts runtime behavior (section 16 panic
//! contract, section 11 saturating cast, section 10.1 `*WithOverflow`), plus
//! that the prelude compiles warning-free standalone under the section 7
//! flags.
//!
//! `#![feature(rustc_private)]` + `extern crate rustc_driver;` link the
//! rustc-backed bridge that the runner lib transitively pulls in.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::Path;
use std::process::Command;

use codegen_coverage_runner::oracle;
use codegen_coverage_runner::toolchain;
use regseq_backends::full_helper_prelude;

/// The host `cc`, matching the runner's default.
fn cc() -> &'static Path {
    Path::new("cc")
}

/// The full bank-1 helper library (static prelude + every arithmetic / cast
/// helper). `emit_generic_c` is now demand-driven (plan 10) and would emit no
/// helpers for an empty model, so these tests -- which verify each helper
/// *definition* compiles warning-free and behaves -- exercise the complete
/// library via `full_helper_prelude`.
fn prelude_tu() -> String {
    full_helper_prelude()
}

/// B3: the emitted prelude + helper block compiles warning-free standalone
/// under the pinned section 7 flags. `compile_and_run` discards `cc`'s
/// compile-time stderr on success, so this invokes `cc -fsyntax-only`
/// directly to capture and assert warnings are absent.
#[test]
fn prelude_compiles_with_no_warnings() {
    let tu = prelude_tu();
    let out = Command::new(cc())
        .args([
            "-std=gnu11",
            "-fwrapv",
            "-Wall",
            "-fsyntax-only",
            "-x",
            "c",
            "-",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child
                .stdin
                .take()
                .expect("stdin")
                .write_all(tu.as_bytes())?;
            child.wait_with_output()
        })
        .expect("cc -fsyntax-only must run");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "prelude must compile: {stderr}");
    assert!(
        stderr.is_empty(),
        "prelude must be warning-free, got: {stderr}"
    );
}

/// B2: prelude + an empty entry compiles clean and exits 0 (no stray panic
/// site, no warning fallout that would surface as a runtime fault).
#[test]
fn prelude_with_empty_entry_exits_zero() {
    let tu = format!("{}\nvoid regseq_main(void) {{}}\n", prelude_tu());
    let run = toolchain::compile_and_run(&tu, "regseq_main", cc())
        .expect("prelude + empty main must build/run");
    assert_eq!(run.exit_code, Some(0), "empty entry must exit 0");
    assert_eq!(run.stderr, "", "empty entry must have clean stderr");
}

/// B2 / section 16: `regseq_panic` prints `"panic: <msg>\n"` to stderr and
/// `abort()`s (non-zero / signal exit), keeping the stdout oracle clean.
#[test]
fn regseq_panic_writes_stderr_and_aborts() {
    let tu = format!(
        "{}\nvoid regseq_main(void) {{ regseq_panic(\"boom\"); }}\n",
        prelude_tu()
    );
    let run =
        toolchain::compile_and_run(&tu, "regseq_main", cc()).expect("panic TU must build/run");
    assert_ne!(run.exit_code, Some(0), "regseq_panic must not exit 0");
    assert_eq!(run.stdout, "", "panic must not pollute stdout");
    assert_eq!(run.stderr, "panic: boom\n");
    oracle::behavioral_panic(&run, "panic: boom\n").expect("panic stderr must match");
}

/// B2 / section 11: the saturating `FloatToInt` helper -- `1e30 -> INT32_MAX`,
/// `NaN -> 0`, `-1e30 -> INT32_MIN` (bounds before cast; NaN guard first).
#[test]
fn saturating_f64_to_int32_saturates_and_nan_is_zero() {
    let tu = format!(
        "{}\n\
         void regseq_main(void) {{\n\
         \x20\x20\x20\x20printf(\"%d %d %d\\n\",\n\
         \x20\x20\x20\x20\x20\x20\x20\x20regseq_f64_to_int32_t(1e30),\n\
         \x20\x20\x20\x20\x20\x20\x20\x20regseq_f64_to_int32_t(NAN),\n\
         \x20\x20\x20\x20\x20\x20\x20\x20regseq_f64_to_int32_t(-1e30));\n\
         }}\n",
        prelude_tu()
    );
    let run =
        toolchain::compile_and_run(&tu, "regseq_main", cc()).expect("saturating TU must build/run");
    assert_eq!(run.exit_code, Some(0), "must exit 0: {}", run.stderr);
    // INT32_MAX, 0, INT32_MIN.
    assert_eq!(run.stdout, "2147483647 0 -2147483648\n");
    oracle::behavioral_stdout(&run.stdout, "2147483647 0 -2147483648\n")
        .expect("stdout must match the golden");
}

/// B2 / section 10.1: `*WithOverflow` -- a non-overflowing add (`overflow ==
/// 0`, `v` correct) and an overflowing mul (`overflow == 1`).
#[test]
fn with_overflow_reports_flag_and_value() {
    let tu = format!(
        "{}\n\
         void regseq_main(void) {{\n\
         \x20\x20\x20\x20struct regseq_overflow_int32_t ok = regseq_add_int32_t(2, 3);\n\
         \x20\x20\x20\x20struct regseq_overflow_int32_t bad = regseq_mul_int32_t(INT32_MAX, 2);\n\
         \x20\x20\x20\x20printf(\"%d %d %d\\n\", (int)ok.v, (int)ok.overflow, (int)bad.overflow);\n\
         }}\n",
        prelude_tu()
    );
    let run =
        toolchain::compile_and_run(&tu, "regseq_main", cc()).expect("overflow TU must build/run");
    assert_eq!(run.exit_code, Some(0), "must exit 0: {}", run.stderr);
    // v=5, no-overflow flag 0, overflow flag 1.
    assert_eq!(run.stdout, "5 0 1\n");
}
