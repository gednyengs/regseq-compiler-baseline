//! The env-gated L1 kbuild gate (backend-linux-c T11+T12; GRILL C03, C01.2).
//!
//! With `RSEQ_LINUX_KDIR` pointing at the eval campaign's configured kernel
//! build tree (6.18.21 at both campaign pins), the generated drivers —
//! `linux-minimal`, the composed PL011 first-light driver, and the two
//! e1000 drivers (backend-linux-c-e1000 T10) — must compile as out-of-tree
//! modules with ZERO errors
//! (warnings recorded, not gating). Unset ⇒ one visible skip notice per
//! driver naming the variable (the suite stays green on machines with no
//! kernel tree, I02); set-but-invalid ⇒ hard failure (C03).
//!
//! The actual kernel release is recorded in the pass notice (D03).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::Path;

use codegen_coverage_runner::{kbuild, manifest, pipeline};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn fixture(corpus: &str, name: &str) -> manifest::Fixture {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(corpus);
    manifest::discover(&dir)
        .expect("discover corpus")
        .into_iter()
        .find(|f| f.manifest.name == name)
        .unwrap_or_else(|| panic!("{name} fixture discovered"))
}

fn emit(fixture: &manifest::Fixture) -> regseq_backends::LinuxEmit {
    let mut model = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(m) => m,
        other => panic!("bridge failed: {other:?}"),
    };
    pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind");
    regseq_backends::emit_linux_c(&model).expect("emit_linux_c")
}

/// Print to the REAL stderr (bypassing libtest's capture): the C03 skip
/// notice must be visible on a green run, not swallowed.
fn visible(line: String) {
    use std::io::Write;
    let mut err = std::io::stderr().lock();
    let _ = writeln!(err, "{line}");
}

fn gate(label: &str, emit: &regseq_backends::LinuxEmit) {
    match kbuild::kbuild_module(&emit.tu, &emit.kbuild, &emit.module_name) {
        Ok(kbuild::KbuildOutcome::Skipped { notice }) => {
            // C03: the one visible notice; never a silent pass.
            visible(format!("[{label}] {notice}"));
        }
        Ok(kbuild::KbuildOutcome::Built {
            kernel_version,
            warnings,
        }) => {
            visible(format!("[{label}] kbuild PASS against kernel {kernel_version}"));
            if !warnings.trim().is_empty() {
                visible(format!(
                    "[{label}] warnings (recorded, not gating):\n{warnings}"
                ));
            }
        }
        Err(err) => panic!("[{label}] {err}"),
    }
}

#[test]
fn generated_drivers_kbuild_against_the_campaign_tree() {
    // T11: the smallest emission proves the staging + env contract.
    let minimal = fixture("fixtures/_linux", "linux-minimal");
    gate("linux-minimal", &emit(&minimal));

    // T12: the composed PL011 first-light driver is the gating acceptance
    // (C01.2). The composed bridge runs in this same process AFTER
    // minimal's — no snapshot comparison happens here, so interner history
    // is irrelevant.
    let composed = fixture("fixtures/_composed", "pl011-composed")
        .with_binding(manifest::OsBinding::Linux)
        .expect("the converged fixture resolves the linux binding (C05)");
    gate("pl011-composed", &emit(&composed));

    // backend-linux-c-e1000 T10: both generated e1000 drivers — the composed
    // PCI keystone and the induced-failure variant — compile against the
    // same campaign tree (C02 item 2). This is the O3 pin's mechanical
    // backstop: a wrong pcim symbol or include fails compilation here.
    let e1000 = fixture("fixtures/_composed", "e1000-composed")
        .with_binding(manifest::OsBinding::Linux)
        .expect("the converged fixture resolves the linux binding (C05)");
    gate("e1000-composed", &emit(&e1000));
    let e1000fail = fixture("fixtures/_composed", "e1000-initfail")
        .with_binding(manifest::OsBinding::Linux)
        .expect("the converged fixture resolves the linux binding (C05)");
    gate("e1000-initfail", &emit(&e1000fail));
}
