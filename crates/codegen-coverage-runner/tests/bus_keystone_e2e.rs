//! Bus-resource-management (construct #3) KEYSTONE gate — the end-to-end L2
//! proof (Stream-B T3): a real sibling `platform.toml` flows
//! platform.toml → bound `RegseqModel` → emitted C, with NO hand-set
//! `MmioBinding`. The `brm-e1000-keystone` fixture (a PCI-matched driver with a
//! bus-managed MMIO region) passes the six-stage dual oracle, and the verified
//! `golden.c` carries the real `regseq_bus_map_region(0xf0000000ull, 0x20000ull,
//! 0)` while the behavioral stdout shows the based register access at the real
//! mapped address (`@0xf0000008`).
//!
//! This is the harness coherence payoff: the model the backend consumes IS the
//! bound model, so the golden captures L2 binding for free (no separate "did
//! binding occur" assertion). The runner pipeline (`run_fixture`) performs the
//! bind production already does at the post-bridge "1b" seam (T2).
//!
//! SOLE test in this binary (like `lifecycle_e1000_e2e` / `corpus_e2e`): the
//! emit's MIR is sensitive to rustc's process-global interner history, so the
//! byte-snapshot is reproducible only in a dedicated clean process (D17).
//! `BLESS=1` writes `golden.c` + `expected.stdout` from a single clean
//! bind+emit, then returns without also verifying.
//!
//! `#![feature(rustc_private)]` + `extern crate rustc_driver;` link the
//! rustc-backed bridge (stage 1) into the test binary.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline, toolchain};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

/// The host `cc`, matching the runner's default.
fn cc() -> &'static Path {
    Path::new("cc")
}

fn bus_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_bus")
}

#[test]
fn brm_e1000_keystone_maps_the_real_platform_base_end_to_end() {
    let dir = bus_corpus().join("brm-e1000-keystone");
    let fixtures = manifest::discover(&bus_corpus()).expect("discover bus corpus");
    let fixture = fixtures
        .iter()
        .find(|f| f.manifest.name == "brm-e1000-keystone")
        .expect("brm-e1000-keystone fixture discovered");

    // The fixture MUST carry its sibling platform.toml (T1 discovery): without it
    // the binding never resolves and there is no L2 to prove.
    assert!(
        fixture.platform.is_some(),
        "keystone fixture must discover a sibling platform.toml (T1)"
    );

    if std::env::var_os("BLESS").is_some() {
        let options = pipeline::backend_options_for(fixture).expect("backend options");
        let mut model = match try_bridge_fixture_path(&dir.join("input.rs")) {
            BridgeOutcome::Ok(m) => m,
            other => panic!("bless bridge failed: {other:?}"),
        };
        // The bless path applies the IDENTICAL L2 bind + identity resolution
        // run_fixture does (T2/T7), so the blessed golden reflects the BOUND model
        // AND the traced `[platform]` identity (no hand-set binding).
        let identity = pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind (bless)");
        let context = regseq_backends::BackendContext::new(options, identity);
        let emit = regseq_backends::emit_generic_c_for_entry_with_context(
            &model,
            &fixture.manifest.entry,
            &context,
        )
        .expect("emit must succeed");
        let run = toolchain::compile_and_run(&emit.tu, &emit.shim, cc())
            .expect("compile + run must succeed");
        std::fs::write(dir.join("golden.c"), &emit.tu).expect("write golden.c");
        std::fs::write(dir.join("expected.stdout"), &run.stdout).expect("write expected.stdout");
        return;
    }

    let result = pipeline::run_fixture(fixture, cc());
    assert!(
        result.is_green(),
        "brm-e1000-keystone must pass all six stages: {:?}",
        result.verdict
    );

    // KEYSTONE GATE: the emitted C must carry the REAL platform base/size, keyed
    // by the effective BAR — the literal flowed platform.toml → bound model →
    // emitted C with no hand-set `MmioBinding`.
    let golden = std::fs::read_to_string(dir.join("golden.c")).expect("read golden.c");
    assert!(
        golden.contains("regseq_bus_map_region(0xf0000000ull, 0x20000ull, 0);"),
        "golden.c must map the real declared (base, size, bar) from platform.toml"
    );
    // The PCI acquisition preamble (construct #3 M3) precedes the map_region.
    assert!(
        golden.contains("regseq_bus_enable_device();")
            && golden.contains("regseq_bus_claim_regions(0);")
            && golden.contains("regseq_bus_get_handle();"),
        "golden.c must emit the construct-#3 PCI acquisition preamble before map_region"
    );

    // T7 IDENTITY GATE: the `[platform]` identity (os/board/arch) flows
    // platform.toml → resolved `PlatformIdentity` → `BackendContext` → emitted C
    // as a target trace comment — the end-to-end proof that identity reaches the
    // backend (generic-C only traces it; D7). The same single source of truth
    // (D4) future Zephyr/Linux backends select a framework from.
    assert!(
        golden.contains("/* regseq target platform: os=linux board=qemu-pc arch=x86_64 */"),
        "golden.c must carry the [platform] identity trace (os/board/arch) from platform.toml"
    );

    // BEHAVIORAL GATE (dual oracle): `on_init`'s register access routes through the
    // BASED accessor, so the trace shows the real mapped absolute address
    // (base 0xf0000000 + ICR offset 0x08).
    let stdout = std::fs::read_to_string(dir.join("expected.stdout")).expect("read expected.stdout");
    assert!(
        stdout.contains("@0xf0000008"),
        "behavioral stdout must show the based register access at the real mapped \
         address (0xf0000000 + 0x08); got:\n{stdout}"
    );
}
