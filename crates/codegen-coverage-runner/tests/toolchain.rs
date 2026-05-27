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
use codegen_coverage_runner::toolchain::{self, ToolchainError};

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
    let run = toolchain::compile_and_run(tu, "regseq_main", cc()).expect("pass TU must build/run");

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
    let run = toolchain::compile_and_run(tu, "regseq_main", cc()).expect("panic TU must build/run");

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
    let err = toolchain::compile_and_run(tu, "regseq_main", cc())
        .expect_err("malformed TU must fail to compile");
    match err {
        ToolchainError::CompileFailed { stderr } => {
            assert!(!stderr.is_empty(), "compile failure must carry cc stderr");
        }
        other => panic!("expected CompileFailed, got {other:?}"),
    }
}

/// `generate_main_c` emits the Doc-2 D3 unit-`void` entry shim for the
/// designated entry name.
#[test]
fn generate_main_c_calls_the_entry() {
    let main_c = toolchain::generate_main_c("regseq_main");
    assert!(main_c.contains("extern void regseq_main(void);"));
    assert!(main_c.contains("regseq_main();"));
    assert!(main_c.contains("int main(void)"));
    assert!(main_c.contains("return 0;"));
}
