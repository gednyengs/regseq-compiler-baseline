//! Stages 4-6 (the load-bearing proof): drive `compile_and_run` + the
//! dual oracle with hand-written C strings (inline, not committed
//! fixtures). These exercise the `cc` subprocess + hermetic temp dir +
//! capture + oracle end to end against real C -- the machinery tasks
//! b-e depend on, verified before any emit exists.
//!
//! The `rustc_private` anchor is required because the crate under test
//! transitively links the rustc-backed bridge.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::Path;

use codegen_coverage_runner::oracle;
use codegen_coverage_runner::toolchain::{self, EntryShim, RunOutput, ToolchainError};

/// The host `cc`, matching the runner's default.
fn cc() -> &'static Path {
    Path::new("cc")
}

/// A "pass" TU: `regseq_main` prints a computed value. Compiles, runs,
/// exits 0 with empty stderr, and `behavioral_stdout` matches the golden.
#[test]
fn pass_tu_compiles_runs_and_stdout_matches() {
    let tu = "#include <stdio.h>\n\
              void regseq_main(void) {\n\
              \x20\x20\x20\x20int result = 21 + 21;\n\
              \x20\x20\x20\x20printf(\"[INFO] result=%d\\n\", result);\n\
              }\n";
    let run = toolchain::compile_and_run(tu, &EntryShim::unit_void("regseq_main"), cc())
        .expect("pass TU must build/run");

    assert_eq!(run.exit_code, Some(0), "pass TU must exit 0");
    assert_eq!(run.stderr, "", "pass TU stderr must be clean");
    assert_eq!(run.stdout, "[INFO] result=42\n");

    oracle::behavioral_stdout(&run.stdout, "[INFO] result=42\n")
        .expect("stdout must match the golden");
    // A real mismatch is reported, not silently blessed.
    assert!(oracle::behavioral_stdout(&run.stdout, "[INFO] result=43\n").is_err());
}

/// A "panic" TU: prints to stderr then `abort()`s. The process dies by
/// signal (non-zero / `None` exit), and `behavioral_panic` matches the
/// golden stderr.
#[test]
fn panic_tu_aborts_with_stderr_and_nonzero_exit() {
    let tu = "#include <stdio.h>\n\
              #include <stdlib.h>\n\
              void regseq_main(void) {\n\
              \x20\x20\x20\x20fprintf(stderr, \"[PANIC] assertion failed\\n\");\n\
              \x20\x20\x20\x20abort();\n\
              }\n";
    let run = toolchain::compile_and_run(tu, &EntryShim::unit_void("regseq_main"), cc())
        .expect("panic TU must build/run");

    assert_ne!(run.exit_code, Some(0), "panic TU must not exit 0");
    assert_eq!(run.stderr, "[PANIC] assertion failed\n");

    oracle::behavioral_panic(&run, "[PANIC] assertion failed\n")
        .expect("panic stderr must match the golden");
    // Wrong golden stderr is a mismatch; a clean exit would also fail.
    assert!(oracle::behavioral_panic(&run, "[PANIC] other\n").is_err());
}

/// A malformed TU fails compilation as a stage-4
/// `ToolchainError::CompileFailed` carrying the compiler stderr.
#[test]
fn malformed_tu_is_a_compile_failure() {
    let tu = "void regseq_main(void) { this is not valid c }\n";
    let err = toolchain::compile_and_run(tu, &EntryShim::unit_void("regseq_main"), cc())
        .expect_err("malformed TU must fail to compile");
    match err {
        ToolchainError::CompileFailed { stderr } => {
            assert!(!stderr.is_empty(), "compile failure must carry cc stderr");
        }
        other => panic!("expected CompileFailed, got {other:?}"),
    }
}

/// The symbolic `expected.exit` oracle (`behavioral_exit`), string/value in,
/// verdict out: a matching exit with empty stderr passes; a mismatched exit
/// fails; and a nonzero exit WITH stderr fails -- so a `panic=abort` SIGABRT
/// (nonzero + a panic message on stderr) cannot masquerade as a clean errno
/// return.
#[test]
fn behavioral_exit_matches_resolves_and_guards_stderr() {
    let clean_zero = RunOutput {
        stdout: "[INFO] ok\n".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
    };
    oracle::behavioral_exit(&clean_zero, 0).expect("exit 0 with empty stderr must match");

    let clean_errno = RunOutput {
        stdout: String::new(),
        stderr: String::new(),
        exit_code: Some(110),
    };
    oracle::behavioral_exit(&clean_errno, 110)
        .expect("a matching nonzero exit with empty stderr must match");

    // Mismatched exit code fails.
    assert!(
        oracle::behavioral_exit(&clean_errno, 22).is_err(),
        "exit 110 must not satisfy an expected 22"
    );
    assert!(
        oracle::behavioral_exit(&clean_zero, 110).is_err(),
        "exit 0 must not satisfy an expected nonzero errno"
    );

    // Nonzero exit WITH stderr fails the empty-stderr guard (the SIGABRT
    // masquerade defense, requirement R11).
    let aborted = RunOutput {
        stdout: String::new(),
        stderr: "[PANIC] assertion failed\n".to_string(),
        exit_code: Some(134),
    };
    assert!(
        oracle::behavioral_exit(&aborted, 134).is_err(),
        "a nonzero exit with stderr must fail even when the code matches"
    );

    // A signal death (None exit) never satisfies a numeric expectation.
    let signal = RunOutput {
        stdout: String::new(),
        stderr: String::new(),
        exit_code: None,
    };
    assert!(
        oracle::behavioral_exit(&signal, 0).is_err(),
        "a signal-killed process (None exit) must not satisfy any numeric exit"
    );
}

/// `resolve_errno` resolves a real errno identifier on the fixture's own
/// `cc`/libc to a sane numeric (no host numeric is hard-coded; we only assert
/// it lands in the 8-bit exit range), and short-circuits `"0"` to 0 with no
/// probe.
#[test]
fn resolve_errno_resolves_a_symbol_and_short_circuits_zero() {
    assert_eq!(
        toolchain::resolve_errno("0", cc()).expect("\"0\" must resolve"),
        0,
        "the literal \"0\" must resolve to 0 without a probe"
    );

    let n =
        toolchain::resolve_errno("ETIMEDOUT", cc()).expect("ETIMEDOUT must resolve on this libc");
    assert!(
        n > 0 && n < 256,
        "ETIMEDOUT must resolve to a sane 8-bit-fitting positive numeric, got {n}"
    );

    // An identifier the libc does not define fails the probe compile loudly.
    assert!(
        toolchain::resolve_errno("ENOSUCHERRNO_REGSEQ", cc()).is_err(),
        "an undefined errno identifier must fail to resolve, not return garbage"
    );

    // A raw numeric or a C expression is NOT a symbolic identifier: it is
    // rejected up front (never resolved to its own value), so no host-libc
    // numeric can be smuggled into a manifest (C04/R10/I03).
    assert!(
        toolchain::resolve_errno("110", cc()).is_err(),
        "a raw numeric must be rejected, not resolved to itself"
    );
    assert!(
        toolchain::resolve_errno("EINVAL + 1", cc()).is_err(),
        "a C expression must be rejected, not interpolated into the probe"
    );
}

/// `generate_main_c` emits the Doc-2 D3 unit-`void` entry shim for the
/// designated entry name.
#[test]
fn generate_main_c_calls_the_entry() {
    let main_c = toolchain::generate_main_c(&EntryShim::unit_void("regseq_main"));
    assert!(main_c.contains("extern void regseq_main(void);"));
    assert!(main_c.contains("regseq_main();"));
    assert!(main_c.contains("int main(void)"));
    assert!(main_c.contains("return 0;"));
}
