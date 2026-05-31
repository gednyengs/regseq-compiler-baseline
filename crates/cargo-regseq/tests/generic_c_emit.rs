//! End-to-end regression test for `cargo regseq --emit generic-c`:
//! `cargo regseq` -> `cargo check` -> rustc -> bridge -> validate ->
//! `regseq_backends::emit_generic_c` -> `regseq-out/generic.c`.
//!
//! Locks the `regseq-driver-87` wiring (the `EmitSpec::GenericC`
//! emit arm). Mirrors the `--emit model` harness in
//! `playground_emit.rs`, but the playground here defines an
//! `extern "C"` entry that logs a computed value, so the emitted C
//! translation unit is meaningful: the test asserts it is a real C
//! TU and, when a host `cc` is available, compiles AND runs it and
//! asserts the diag-log behavioral output (`[INFO] smoke=42`).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

/// Workspace root: `crates/cargo-regseq/Cargo.toml` -> ../../.
fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

/// Absolute path to the test-built `cargo-regseq` binary.
fn cargo_regseq_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_cargo-regseq"))
}

/// Absolute path to the sibling `regseq-driver` binary in the same
/// target dir (Cargo only injects `CARGO_BIN_EXE_<name>` for the
/// current package, so resolve the sibling by hand).
fn regseq_driver_bin() -> PathBuf {
    cargo_regseq_bin()
        .parent()
        .expect("target dir")
        .join("regseq-driver")
}

/// Build `regseq-driver` from source before invoking the subprocess
/// (Cargo does not auto-build sibling packages' binaries for this
/// test crate). Mirrors `playground_emit.rs::ensure_driver_built`.
fn ensure_driver_built() {
    let driver = regseq_driver_bin();
    if driver.exists() {
        return;
    }
    let status = Command::new(env!("CARGO"))
        .args(["build", "-p", "regseq-driver"])
        .current_dir(workspace_root())
        .status()
        .expect("failed to run `cargo build -p regseq-driver`");
    assert!(
        status.success(),
        "`cargo build -p regseq-driver` failed with status {status}",
    );
    assert!(
        driver.exists(),
        "regseq-driver binary still not found at {} after build",
        driver.display(),
    );
}

/// Write a standalone playground crate whose `extern "C"` entry logs
/// `add(20, 22)` through the diag-log intrinsic, so the emitted C is
/// a runnable translation unit with observable stdout.
fn write_playground(dir: &Path) {
    fs::create_dir_all(dir.join("src")).expect("mkdir src");

    fs::write(
        dir.join("Cargo.toml"),
        // `[workspace]` makes this a standalone crate so cargo does
        // not inherit our own workspace settings.
        r#"[package]
name = "regseq-gc-smoke"
version = "0.0.0"
edition = "2021"
publish = false

[workspace]
"#,
    )
    .expect("write Cargo.toml");

    fs::write(
        dir.join("platform.toml"),
        r#"[platform]
name = "playground"
"#,
    )
    .expect("write platform.toml");

    // Pin the same nightly the workspace uses so the driver's
    // `rustc_private` linkage matches.
    fs::copy(
        workspace_root().join("rust-toolchain.toml"),
        dir.join("rust-toolchain.toml"),
    )
    .expect("copy rust-toolchain.toml");

    fs::write(
        dir.join("src/lib.rs"),
        // A self-contained diag-log intrinsic shim (the playground
        // does not depend on the `regseq` facade) plus an
        // `extern "C"` entry that logs a computed value. The
        // `register_tool(rseq)` attrs let rustc accept the
        // `#[rseq::intrinsic(..)]` marker the bridge keys on.
        r#"#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

pub mod regseq {
    pub mod log {
        pub mod intrinsics {
            pub enum LogArg<'a> {
                U32(u32),
                Str(&'a str),
            }
            impl From<u32> for LogArg<'_> {
                fn from(v: u32) -> Self {
                    LogArg::U32(v)
                }
            }
            impl<'a> From<&'a str> for LogArg<'a> {
                fn from(v: &'a str) -> Self {
                    LogArg::Str(v)
                }
            }
            #[inline(never)]
            #[rseq::intrinsic("log-info")]
            pub fn log_info(_fmt: &'static str, _args: &[LogArg<'_>]) {}
        }
        macro_rules! info {
            ($fmt:literal $(, $arg:expr)* $(,)?) => {
                $crate::regseq::log::intrinsics::log_info(
                    $fmt,
                    &[$($crate::regseq::log::intrinsics::LogArg::from($arg)),*],
                )
            };
        }
        pub(crate) use info;
    }
}

#[inline(never)]
fn add(a: u32, b: u32) -> u32 {
    a + b
}

#[no_mangle]
pub extern "C" fn regseq_main() {
    let r = add(20, 22);
    regseq::log::info!("smoke={}", r);
}
"#,
    )
    .expect("write src/lib.rs");
}

/// Locate a host C compiler (`$CC` or `cc` on PATH). Returns `None`
/// when none is available, so the compile/run leg degrades to a
/// skip rather than a spurious failure on a host without a `cc`.
fn host_cc() -> Option<PathBuf> {
    if let Some(cc) = std::env::var_os("CC") {
        return Some(PathBuf::from(cc));
    }
    let out = Command::new("sh")
        .args(["-c", "command -v cc"])
        .output()
        .ok()?;
    if out.status.success() {
        let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !path.is_empty() {
            return Some(PathBuf::from(path));
        }
    }
    None
}

#[test]
fn cargo_regseq_emit_generic_c_compiles_and_runs() {
    ensure_driver_built();
    let temp = TempDir::new().expect("mkdir temp");
    let dir = temp.path();
    write_playground(dir);

    // Invoke `cargo-regseq regseq ... --emit generic-c` directly
    // (argv[1] = "regseq" is what cargo's subcommand discovery would
    // inject; going direct avoids PATH manipulation).
    let output = Command::new(cargo_regseq_bin())
        .arg("regseq")
        .arg("--target-os")
        .arg("linux")
        .arg("--platform-file")
        .arg("platform.toml")
        .arg("--emit")
        .arg("generic-c")
        .env("REGSEQ_DRIVER", regseq_driver_bin())
        .current_dir(dir)
        .output()
        .expect("failed to spawn cargo-regseq");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "cargo-regseq exited with {}: stdout={stdout} stderr={stderr}",
        output.status,
    );

    // The emit lands at `<output_dir>/generic.c` (default output dir
    // is `./regseq-out/`).
    let gen_c = dir.join("regseq-out").join("generic.c");
    assert!(
        gen_c.exists(),
        "generic.c was not created at {}: stdout={stdout} stderr={stderr}",
        gen_c.display(),
    );
    let code = fs::read_to_string(&gen_c).expect("read generic.c");
    assert!(
        !code.is_empty(),
        "generic.c is empty: stdout={stdout} stderr={stderr}",
    );

    // Structural: it is a real generic-C translation unit.
    assert!(
        code.contains("#include <stdint.h>"),
        "generic.c is not a real C TU (missing stdint.h include):\n{code}",
    );
    // The entry lowers to the crate-qualified C symbol
    // `sanitize(emission_name)` -> `regseq_gc_smoke_regseq_main`
    // (NOT bare `regseq_main`); see the codegen entry-symbol
    // resolution.
    assert!(
        code.contains("regseq_gc_smoke_regseq_main(void)"),
        "generic.c does not define the crate-qualified entry symbol:\n{code}",
    );

    // Behavioral: when a host `cc` is available, compile the TU with a
    // wrapper `main` that calls the entry, run it, and assert the
    // diag-log output. The emitted TU is a library (no `main`), so the
    // wrapper supplies one -- mirroring how the codegen-coverage
    // runner links a generated `main.c`.
    let Some(cc) = host_cc() else {
        eprintln!("note: no host `cc` found; skipping the compile/run leg of the generic-c smoke");
        return;
    };

    let main_c = dir.join("driver_main.c");
    fs::write(
        &main_c,
        "extern void regseq_gc_smoke_regseq_main(void);\n\
         int main(void) { regseq_gc_smoke_regseq_main(); return 0; }\n",
    )
    .expect("write driver_main.c");

    let bin = dir.join("gc_smoke_bin");
    let compile = Command::new(&cc)
        .arg("-std=gnu11")
        .arg("-o")
        .arg(&bin)
        .arg(&gen_c)
        .arg(&main_c)
        .output()
        .expect("failed to spawn cc");
    assert!(
        compile.status.success(),
        "cc failed to compile the emitted generic.c: {}",
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new(&bin)
        .output()
        .expect("failed to run the emitted binary");
    assert!(
        run.status.success(),
        "the emitted binary exited non-zero ({}): stderr={}",
        run.status,
        String::from_utf8_lossy(&run.stderr),
    );
    let run_stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        run_stdout.contains("[INFO] smoke=42"),
        "the emitted binary did not log the expected computed value; \
         got stdout={run_stdout:?}",
    );
}
