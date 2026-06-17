//! Atomicity construct M3 heavy-ops host oracle (T11/T12/T13): the `fetch_*` /
//! `swap` RMW family, `compare_exchange` (`Result<T, T>` commit/contention), and
//! the standalone `atomic::fence`, each through the certified six-stage generic_c
//! path. Driverless + L2-free (D05/D11), so the bind stage is a no-op.
//!
//! `BLESS=1` writes each fixture's `golden.c` + `expected.stdout` from a clean
//! bridge+emit+run. The three fixtures run SEQUENTIALLY in this single test (one
//! rustc per fixture, never concurrent — the interner-isolation discipline).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::{manifest, pipeline, toolchain};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn cc() -> &'static Path {
    Path::new("cc")
}

fn atomic_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_atomic")
}

/// (fixture name, golden.c substrings, expected.stdout lines) per M3 fixture.
fn cases() -> Vec<(&'static str, Vec<&'static str>, Vec<&'static str>)> {
    vec![
        (
            "atomic-rmw",
            vec!["regseq_atomic_rmw(0, 32,"],
            vec![
                "ATM add cell=0 w=32 ord=Relaxed val=0x5 prior=0x0",
                "ATM add cell=0 w=32 ord=AcqRel val=0x3 prior=0x5",
                "ATM or cell=0 w=32 ord=Release val=0xf0 prior=0x8",
                "ATM and cell=0 w=32 ord=Acquire val=0xf prior=0xf8",
                "ATM xor cell=0 w=32 ord=SeqCst val=0xff prior=0x8",
                "ATM swap cell=0 w=32 ord=SeqCst val=0x2a prior=0xf7",
            ],
        ),
        (
            "atomic-cas",
            vec!["regseq_atomic_cmpxchg(0, 32,"],
            vec![
                "ATM cmpxchg cell=0 w=32 succ=AcqRel fail=Acquire expected=0xa desired=0x14 observed=0xa outcome=Ok",
                "ATM store cell=0 w=32 ord=Release val=0xa",
                "ATM cmpxchg cell=0 w=32 succ=SeqCst fail=Relaxed expected=0x63 desired=0x1e observed=0xa outcome=Err",
                "ATM add cell=0 w=32 ord=Relaxed val=0xa prior=0xa",
            ],
        ),
        (
            "atomic-fence",
            vec!["regseq_atomic_fence("],
            vec![
                "ATM store cell=0 w=32 ord=Relaxed val=0x1",
                "ATM fence ord=Release",
                "ATM load cell=0 w=32 ord=Relaxed => 0x1",
                "ATM fence ord=SeqCst",
            ],
        ),
    ]
}

#[test]
fn atomic_m3_heavy_ops_six_stage_oracle() {
    let fixtures = manifest::discover(&atomic_corpus()).expect("discover atomic corpus");
    let bless = std::env::var_os("BLESS").is_some();

    for (name, golden_gates, stdout_gates) in cases() {
        let fixture = fixtures
            .iter()
            .find(|f| f.manifest.name == name)
            .unwrap_or_else(|| panic!("{name} fixture discovered"));

        if bless {
            let options = pipeline::backend_options_for(fixture).expect("backend options");
            let mut model = match try_bridge_fixture_path(&fixture.input) {
                BridgeOutcome::Ok(m) => m,
                other => panic!("[{name}] bless bridge failed: {other:?}"),
            };
            let identity = pipeline::bind_l2_platform(&mut model, fixture).expect("L2 bind (no-op)");
            let context = regseq_backends::BackendContext::new(options, identity);
            let emit = regseq_backends::emit_generic_c_for_entry_with_context(
                &model,
                &fixture.manifest.entry,
                &context,
            )
            .unwrap_or_else(|e| panic!("[{name}] emit must succeed: {e}"));
            let run = toolchain::compile_and_run(&emit.tu, &emit.shim, cc())
                .unwrap_or_else(|e| panic!("[{name}] compile + run must succeed: {e}"));
            std::fs::write(&fixture.golden_c, &emit.tu).expect("write golden.c");
            std::fs::write(&fixture.expected_stdout, &run.stdout).expect("write expected.stdout");
            continue;
        }

        let result = pipeline::run_fixture(fixture, cc());
        assert!(
            result.is_green(),
            "[{name}] must pass all six stages: {:?}",
            result.verdict
        );

        let golden = std::fs::read_to_string(&fixture.golden_c).expect("read golden.c");
        for needle in &golden_gates {
            assert!(golden.contains(needle), "[{name}] golden must contain `{needle}`");
        }
        let stdout = std::fs::read_to_string(&fixture.expected_stdout).expect("read expected.stdout");
        for line in &stdout_gates {
            assert!(
                stdout.contains(line),
                "[{name}] trace must carry `{line}`; got:\n{stdout}"
            );
        }
        // Fence placement (T13): the fences sit between the store and the load,
        // and after the load — an ordering assertion beyond mere presence.
        if name == "atomic-fence" {
            let pos = |s: &str| stdout.find(s).unwrap_or_else(|| panic!("[{name}] missing `{s}`"));
            assert!(
                pos("ATM store cell=0 w=32 ord=Relaxed val=0x1")
                    < pos("ATM fence ord=Release")
                    && pos("ATM fence ord=Release") < pos("ATM load cell=0 w=32 ord=Relaxed => 0x1")
                    && pos("ATM load cell=0 w=32 ord=Relaxed => 0x1") < pos("ATM fence ord=SeqCst"),
                "[{name}] fence placement must be store < fence(Release) < load < fence(SeqCst); got:\n{stdout}"
            );
        }
    }
}
