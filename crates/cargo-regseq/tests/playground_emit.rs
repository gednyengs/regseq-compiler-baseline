//! End-to-end regression test for the `cargo regseq` -> `cargo
//! check` -> rustc -> bridge pipeline.
//!
//! Locks the `output_types.should_codegen() == true` requirement
//! that audit-fix `018fcaf` introduced into `RegseqCallbacks::config`.
//! Without that override, `cargo regseq --emit model` against any
//! user-defined library crate writes a zero-byte `model.txt`
//! because:
//!
//!   1. `cargo check` passes `--emit=dep-info,metadata` to rustc.
//!   2. `OutputTypes::should_codegen()` returns false for that
//!      emit set
//!      (`rustc_session/src/config.rs:881-892`).
//!   3. `reachable_non_generics_provider` short-circuits to the
//!      empty map
//!      (`rustc_codegen_ssa/src/back/symbol_export.rs:49-52`).
//!   4. `is_reachable_non_generic` returns false for every
//!      ordinary non-entry root.
//!   5. The Lazy `collect_roots` strategy emits zero roots.
//!   6. The bridge's `collect_and_partition_mono_items` query
//!      returns an empty `MonoItem` set.
//!   7. `PrettyPrinter::new(&model).render()` produces an empty
//!      string, `fs::write` truncates `model.txt` to zero bytes.
//!
//! This test builds a tiny playground crate at test time, invokes
//! `cargo-regseq regseq` against it as a subprocess (mirroring how
//! a real user would run `cargo regseq` on PATH), and asserts the
//! resulting `model.txt` is non-empty and contains rendered output
//! for at least one playground function. A regression in the
//! `output_types` override would surface here as a zero-byte
//! `model.txt`.

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

/// Absolute path to the test-built `cargo-regseq` binary. Cargo
/// injects this env var for every binary defined in the current
/// package.
fn cargo_regseq_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_cargo-regseq"))
}

/// Absolute path to the `regseq-driver` binary, located next to
/// `cargo-regseq` in the same target dir. Cargo only injects
/// `CARGO_BIN_EXE_<name>` for binaries from the CURRENT package;
/// `regseq-driver` lives in a sibling crate, so we resolve its path
/// by sibling-lookup against `cargo_regseq_bin()`.
fn regseq_driver_bin() -> PathBuf {
    cargo_regseq_bin()
        .parent()
        .expect("target dir")
        .join("regseq-driver")
}

/// Build `regseq-driver` from source before invoking the test
/// subprocess. Required because Cargo does not auto-build other
/// workspace packages' binaries when running this test crate's
/// tests; `cargo test -p cargo-regseq` would otherwise be unable
/// to find `regseq-driver`.
///
/// Mirrors the e2e pattern at
/// `tests/e2e-old/tests/driver_e2e.rs:35-44`.
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

/// Write a minimal playground crate into `dir`: a `Cargo.toml` with
/// no parent workspace inheritance, a stub `platform.toml`, the
/// pinned `rust-toolchain.toml` copied from the workspace, and a
/// trivial `src/lib.rs` that defines one `pub fn` plus one nominal
/// struct so the rendered model is non-trivial.
fn write_playground(dir: &Path) {
    fs::create_dir_all(dir.join("src")).expect("mkdir src");

    fs::write(
        dir.join("Cargo.toml"),
        // Stand-alone playground: `[workspace]` blocks cargo from
        // inheriting our own workspace's settings (which would
        // pull in workspace-level dependencies the playground
        // doesn't need).
        r#"[package]
name = "regseq-cargo-test-playground"
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

    // Pin the same nightly the rest of the workspace uses so the
    // driver's `rustc_private` linkage matches.
    fs::copy(
        workspace_root().join("rust-toolchain.toml"),
        dir.join("rust-toolchain.toml"),
    )
    .expect("copy rust-toolchain.toml");

    fs::write(
        dir.join("src/lib.rs"),
        // `register_tool(rseq)` is required because the playground
        // does not depend on the `regseq` facade crate; without
        // these attributes, rustc rejects future `#[rseq::*]`
        // attributes at parse time. The body uses no `rseq::*`
        // attributes (we want to lock the empty-attribute case
        // because that's what a fresh user playground looks like
        // before they reach for an `rseq` macro), but the
        // attributes themselves must be in place for the source to
        // parse cleanly under the driver.
        r#"#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

pub fn identity(x: u32) -> u32 {
    x
}

pub struct Counter {
    pub value: u32,
}

impl Counter {
    pub fn new() -> Counter {
        Counter { value: 0 }
    }
}
"#,
    )
    .expect("write src/lib.rs");
}

#[test]
fn cargo_regseq_emits_non_empty_model_for_playground() {
    ensure_driver_built();
    let temp = TempDir::new().expect("mkdir temp");
    let dir = temp.path();
    write_playground(dir);

    // Invoke `cargo-regseq regseq ...` directly rather than
    // through `cargo regseq`: `[command(name = "cargo", bin_name =
    // "cargo")]` in `crates/cargo-regseq/src/cli.rs:14` declares
    // the binary expects argv[1] = "regseq", which is what cargo's
    // subcommand discovery would inject. Going direct avoids
    // having to manipulate PATH for cargo's `cargo-<sub>` lookup.
    let output = Command::new(cargo_regseq_bin())
        .arg("regseq")
        .arg("--platform-file")
        .arg("platform.toml")
        .arg("--emit")
        .arg("model")
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

    let model_path = dir.join("regseq-out").join("model.txt");
    assert!(
        model_path.exists(),
        "model.txt was not created at {}: stdout={stdout} stderr={stderr}",
        model_path.display(),
    );

    let contents = fs::read_to_string(&model_path).expect("read model.txt");
    assert!(
        !contents.is_empty(),
        "model.txt is empty -- the cargo check / --emit=metadata regression is back. \
         stdout={stdout} stderr={stderr}",
    );

    // The rendered model should contain at least one DefPath for
    // the playground's functions. The exact DefPath form is
    // `regseq_cargo_test_playground::<symbol>`, where the crate
    // name underscore-normalises from the Cargo `name` field.
    assert!(
        contents.contains("regseq_cargo_test_playground::identity"),
        "model.txt does not contain the playground's `identity` fn DefPath; \
         this suggests the bridge produced an empty model. Contents:\n{contents}",
    );
}
