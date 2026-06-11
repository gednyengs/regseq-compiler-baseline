//! The model-equivalence strengthener (spec-convergence-pl011 D12/R17,
//! plan decision 6 — O4 resolved as the in-process double bridge).
//!
//! NON-GATING by contract: this test strengthens the one-spec-two-OSes
//! claim from "one input file, two green pipelines" to "one input file,
//! ONE RegseqModel up to the L2 binding literals" — the two OS pipelines
//! diverge only at the backend, not in what the spec MEANS.
//!
//! Mechanics (the interner-safety answer): ONE test process bridges the
//! converged `input.rs` twice — once per platform binding — then
//! normalizes the OS-conditional fields (the `[mmio]` base/size literals;
//! the `[platform]` identity rides the returned `PlatformIdentity`, not
//! the model) and compares the two models with every interned symbol
//! resolved through its OWN model's interner via the model
//! pretty-printer's full render. Two models bridged in one process may
//! assign interner IDs differently (rustc's process-global history), so
//! raw `==` over ID-bearing structs would be unsound — the rendered form
//! is ID-free by construction. NO serialized model-dump format is
//! invented (the rejected alternative; D12's drop bound exists to prevent
//! exactly that plumbing).
//!
//! `#![feature(rustc_private)]` + `extern crate rustc_driver;` link the
//! rustc-backed bridge into the test binary.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn composed_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_composed")
}

/// Bridge + L2-bind the converged fixture under one binding; return the
/// bound model.
fn bound_model(binding: manifest::OsBinding) -> regseq_model::RegseqModel {
    let fixtures = manifest::discover(&composed_corpus()).expect("discover composed corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "pl011-composed")
        .expect("pl011-composed fixture discovered")
        .with_binding(binding)
        .expect("the converged fixture resolves both bindings (C05)");
    let mut model = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(m) => m,
        other => panic!("bridge failed under {binding:?}: {other:?}"),
    };
    pipeline::bind_l2_platform(&mut model, &fixture).expect("L2 bind + validate");
    model
}

#[test]
fn converged_spec_bridges_to_one_model_under_both_bindings() {
    let linux = bound_model(manifest::OsBinding::Linux);
    let mut zephyr = bound_model(manifest::OsBinding::Zephyr);

    // -- normalize the OS-conditional fields (D12): exactly the [mmio]
    //    base/size literals the two platform files cannot share (the traces
    //    carry the base in every event — D02). Everything else must agree.
    {
        let mut linux_bindings: Vec<(Option<u64>, Option<u64>)> = Vec::new();
        for (_, driver) in linux.device_lifecycle.drivers.iter() {
            for field in &driver.handle.resource_fields {
                linux_bindings.push((field.binding.base, field.binding.size));
            }
        }
        assert_eq!(
            linux_bindings,
            vec![(Some(0x0900_0000), Some(0x1000))],
            "the linux binding resolves the one region at the QEMU virt base"
        );
        let mut i = 0;
        for (_, driver) in zephyr.device_lifecycle.drivers.iter_mut() {
            for field in &mut driver.handle.resource_fields {
                assert_eq!(
                    (field.binding.base, field.binding.size),
                    (Some(0x0904_0000), Some(0x1000)),
                    "the zephyr binding resolves the one region at the DUT base"
                );
                (field.binding.base, field.binding.size) = linux_bindings[i];
                i += 1;
            }
        }
        assert_eq!(i, 1, "exactly one resource binding normalized");
    }

    // -- the resolved-string comparison: the full pretty render resolves
    //    every interned ID through its own model's interner, so the
    //    comparison is meaningful across separately-bridged models.
    let linux_render = linux.pretty().render();
    let zephyr_render = zephyr.pretty().render();
    // Vacuity guard: the render must actually carry the spec's substance.
    assert!(
        linux_render.lines().count() > 100
            && linux_render.contains("pl011")
            && linux_render.contains("on_deinit"),
        "the full model render is non-trivial ({} lines)",
        linux_render.lines().count()
    );
    if linux_render != zephyr_render {
        // Surface the first diverging line, not a megabyte dump.
        let first_diff = linux_render
            .lines()
            .zip(zephyr_render.lines())
            .enumerate()
            .find(|(_, (a, b))| a != b);
        panic!(
            "the converged spec must bridge to ONE model under both bindings \
             (after base/size normalization); first divergence: {:?} \
             (linux render {} lines, zephyr render {} lines)",
            first_diff,
            linux_render.lines().count(),
            zephyr_render.lines().count()
        );
    }
}
