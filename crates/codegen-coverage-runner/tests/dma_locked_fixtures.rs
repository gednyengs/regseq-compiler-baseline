#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::manifest::{self, Expect};
use codegen_coverage_runner::{pipeline, toolchain};
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_dma")
}

fn fixture(name: &str) -> manifest::Fixture {
    manifest::discover(&corpus())
        .expect("discover DMA corpus")
        .into_iter()
        .find(|fixture| fixture.manifest.name == name)
        .unwrap_or_else(|| panic!("missing DMA fixture {name}"))
}

#[test]
fn locked_dma_positive_frontend_and_generic_c_behavior() {
    for name in [
        "dma-buffer-e1000",
        "dma-channel-pl011",
        "dma-independent-lifetimes",
    ] {
        let fixture = fixture(name);
        let mut model = match try_bridge_fixture_path(&fixture.input) {
            BridgeOutcome::Ok(model) => model,
            other => panic!("{name} bridge failed: {other:?}"),
        };
        let identity = pipeline::bind_l2_platform(&mut model, &fixture).expect("DMA L2 bind");
        if name == "dma-buffer-e1000" {
            let mut referenced_core_shims = model
                .instances
                .iter()
                .filter_map(|instance| {
                    let data = instance.kind.as_function()?;
                    let path = model
                        .strings
                        .resolve(model.items[instance.item].def_path.rendered);
                    (path == "core::ptr::null_mut" || path == "core::cmp::PartialEq::ne")
                        .then_some((path, data.body.is_none()))
                })
                .collect::<Vec<_>>();
            referenced_core_shims.sort_unstable();
            assert_eq!(
                referenced_core_shims,
                vec![
                    ("core::cmp::PartialEq::ne", true),
                    ("core::ptr::null_mut", true)
                ],
                "external core helpers must remain model-bodyless after bridging",
            );
        }
        let context = regseq_backends::BackendContext::new(
            pipeline::backend_options_for(&fixture).expect("backend options"),
            identity,
        );
        let emit = regseq_backends::emit_generic_c_for_entry_with_context(
            &model,
            &fixture.manifest.entry,
            &context,
        )
        .unwrap_or_else(|err| panic!("{name} generic-C emit failed: {err}"));
        assert!(emit.tu.contains("RSEQ-DMA"), "{name} lacks DMA manifest");
        if name == "dma-buffer-e1000" {
            for symbol in ["core_cmp_PartialEq_ne", "core_ptr_null_mut"] {
                let definitions = emit
                    .tu
                    .lines()
                    .filter(|line| {
                        let line = line.trim_start();
                        line.starts_with("static inline ")
                            && line.contains(symbol)
                            && line.ends_with('{')
                    })
                    .count();
                assert_eq!(
                    definitions, 1,
                    "referenced core helper {symbol} must have exactly one TU-local generic-C body",
                );
            }
            assert!(
                emit.tu
                    .lines()
                    .any(|line| line.trim().starts_with("return !")
                        && line.contains("__eq(lhs, rhs);")),
                "PartialEq::ne must delegate to the concrete bodyful eq instance",
            );
            assert!(
                emit.tu.lines().any(|line| {
                    let line = line.trim_start();
                    line.starts_with("static inline ")
                        && line.contains("__eq(")
                        && line.ends_with('{')
                }),
                "the concrete eq used by the TU-local ne shim must also have TU-local linkage",
            );
            assert!(
                !emit.tu.contains("memcmp("),
                "PartialEq::ne must not guess structural equality with memcmp",
            );
        }
        let run = toolchain::compile_and_run(&emit.tu, &emit.shim, Path::new("cc"))
            .unwrap_or_else(|err| panic!("{name} compile/run failed: {err}"));
        assert_eq!(run.exit_code, Some(0), "{name}: {}", run.stderr);
    }
}

#[test]
fn locked_dma_negative_frontend_corpus() {
    let fixtures = manifest::discover(&corpus()).expect("discover DMA corpus");
    let rejects = fixtures
        .iter()
        .filter(|fixture| matches!(fixture.manifest.expect, Expect::Reject))
        .collect::<Vec<_>>();
    assert_eq!(rejects.len(), 4, "the locked DMA negative corpus is exact");
    for fixture in rejects {
        let result = pipeline::run_fixture(fixture, Path::new("cc"));
        assert!(
            result.is_green(),
            "{}: {:?}",
            fixture.manifest.name,
            result.verdict
        );
    }
}

#[test]
fn dma_two_owned_lifetimes_and_absences_are_recorded() {
    let fixture = fixture("dma-independent-lifetimes");
    let model = match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("independent DMA bridge failed: {other:?}"),
    };
    let dma = model.dma.drivers.values().next().expect("one DMA driver");
    assert!(dma.buffer.is_some());
    assert!(dma.channel.is_some());
    assert!(dma.buffer.as_ref().unwrap().ops.transfer_address.is_none());
    assert!(dma.channel.as_ref().unwrap().ops.status.is_none());
    assert!(dma.channel.as_ref().unwrap().ops.on_completion.is_none());
    assert!(dma.channel.as_ref().unwrap().ops.terminate.is_none());
}
