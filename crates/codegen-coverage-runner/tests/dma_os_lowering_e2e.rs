#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::manifest;
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn fixture(name: &str) -> manifest::Fixture {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_dma");
    manifest::discover(&dir)
        .expect("discover DMA corpus")
        .into_iter()
        .find(|fixture| fixture.manifest.name == name)
        .unwrap_or_else(|| panic!("missing DMA fixture {name}"))
}

fn model(name: &str) -> regseq_model::RegseqModel {
    let fixture = fixture(name);
    match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("{name} bridge failed: {other:?}"),
    }
}

fn linux_channel_model() -> regseq_model::RegseqModel {
    let mut model = model("dma-channel-pl011");
    let driver = model.device_lifecycle.drivers.values_mut().next().unwrap();
    driver.descriptor.bus_type = Some(regseq_model::BusType::Amba);
    driver.descriptor.match_set = vec![regseq_model::MatchEntry::Amba {
        id: 0x0004_1011,
        mask: 0x000f_ffff,
    }];
    driver.ops = regseq_model::LifecycleOps::default();
    model
}

fn zephyr_channel_model() -> regseq_model::RegseqModel {
    let mut model = model("dma-channel-pl011");
    model.device_lifecycle.drivers.values_mut().next().unwrap().ops =
        regseq_model::LifecycleOps::default();
    model
}

fn linux_buffer_model() -> regseq_model::RegseqModel {
    let mut model = model("dma-buffer-e1000");
    let driver = model.device_lifecycle.drivers.values_mut().next().unwrap();
    driver.descriptor.bus_type = Some(regseq_model::BusType::Pci);
    // The locked buffer scenario is the Generic-C core-shim oracle. This test
    // exercises the independently generated native DMA binding, not that host
    // scenario, so do not make Linux own Generic-C's bodyless core helpers.
    driver.ops = regseq_model::LifecycleOps::default();
    model
}

#[test]
fn dma_linux_buffer_and_channel_use_native_owned_lifetimes() {
    let buffer = regseq_backends::emit_linux_c(&linux_buffer_model())
        .expect("Linux DmaBuffer lowering");
    assert!(buffer.tu.contains("dma_alloc_coherent("));
    assert!(buffer.tu.contains("dma_free_coherent("));
    assert!(buffer.tu.contains("_dma_buffer_cpu_address("));
    assert!(buffer.tu.contains("_dma_buffer_transfer_address("));
    assert!(buffer.tu.contains("_dma_buffer_length("));
    assert!(buffer.tu.contains("_dma_buffer_set_valid_length("));
    assert_eq!(buffer.tu.matches("dma_free_coherent(").count(), 1);
    assert!(buffer.tu.contains("buffer->cpu_address = NULL"));
    assert!(!buffer.tu.contains("DmaRegion"));

    let mut minimal_buffer = model("dma-independent-lifetimes");
    let driver = minimal_buffer
        .device_lifecycle
        .drivers
        .values_mut()
        .next()
        .unwrap();
    driver.descriptor.bus_type = Some(regseq_model::BusType::Amba);
    driver.descriptor.match_set = vec![regseq_model::MatchEntry::Amba {
        id: 0x0004_1011,
        mask: 0x000f_ffff,
    }];
    driver.ops = regseq_model::LifecycleOps::default();
    let minimal_buffer = regseq_backends::emit_linux_c(&minimal_buffer)
        .expect("Linux capability-gated DmaBuffer lowering");
    assert!(!minimal_buffer.tu.contains("_dma_buffer_transfer_address("));
    assert!(!minimal_buffer.tu.contains("_dma_buffer_set_valid_length("));

    let channel = regseq_backends::emit_linux_c(&linux_channel_model())
        .expect("Linux async channel lowering");
    for needle in [
        "dma_request_chan(",
        "_dma_channel_prepare(",
        "dmaengine_submit(",
        "owned->prepared = NULL; /* ownership passed to the engine */",
        "dma_async_issue_pending(",
        "dmaengine_tx_status(",
        "dmaengine_terminate_async(",
        "dmaengine_synchronize(",
        "owned->prepared || owned->submitted || owned->active",
        "(owned->terminated && !owned->synchronized)",
        "dma_release_channel(",
    ] {
        assert!(channel.tu.contains(needle), "missing Linux DMA shape {needle}");
    }
    let terminate = channel.tu.find("dmaengine_terminate_async(").unwrap();
    let synchronize = channel.tu.find("dmaengine_synchronize(").unwrap();
    let release = channel.tu.find("dma_release_channel(").unwrap();
    assert!(terminate < synchronize && synchronize < release);
    let configure = channel.tu.find("_dma_channel_configure(").unwrap();
    let prepare = channel.tu.find("_dma_channel_prepare(").unwrap();
    let submit = channel.tu.find("_dma_channel_submit(").unwrap();
    let activate = channel.tu.find("_dma_channel_activate(").unwrap();
    assert!(configure < prepare && prepare < submit && submit < activate);
    assert!(!channel.tu.contains("callback ="));
    assert!(!channel.tu.contains("canceled"));
}

#[test]
fn dma_zephyr_preserves_buffer_absence_and_lowers_channel() {
    let error = regseq_backends::emit_zephyr_c(&model("dma-buffer-e1000"))
        .expect_err("Zephyr has no owned DMA-buffer allocator");
    assert!(error.to_string().contains("DmaBuffer owned allocation is unsupported"));

    let channel = regseq_backends::emit_zephyr_c(&zephyr_channel_model())
        .expect("Zephyr async channel lowering");
    for needle in [
        "<zephyr/drivers/dma.h>",
        "dma_request_channel(",
        "dma_config(",
        "dma_start(",
        "dma_get_status(",
        "dma_stop(",
        "owned->prepared || owned->submitted || owned->active",
        "(owned->terminated && !owned->synchronized)",
        "dma_release_channel(",
    ] {
        assert!(channel.tu.contains(needle), "missing Zephyr DMA shape {needle}");
    }
    assert!(channel.tu.contains("dma_request_channel(controller, filter_param)"));
    assert!(!channel.tu.contains("dma_alloc_coherent"));
    assert!(!channel.tu.contains("__nocache"));
    assert!(!channel.tu.contains("callback ="));
}

#[test]
fn dma_forbidden_old_surface_is_absent_from_new_implementation() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    for path in [
        root.join("regseq-model/src/dma.rs"),
        root.join("regseq-backends/src/linux_c/dma.rs"),
        root.join("regseq-backends/src/zephyr_c/dma.rs"),
    ] {
        let text = std::fs::read_to_string(&path).unwrap();
        for forbidden in ["DmaStreamBuffer", "element_access", "stage_transfer", "dma_mmio_store_order"] {
            assert!(!text.contains(forbidden), "{} contains {forbidden}", path.display());
        }
    }
    let bridge = std::fs::read_to_string(root.join("regseq-driver/src/bridge/dma.rs")).unwrap();
    assert!(bridge.contains("EXCLUDED_TRAITS"));
    assert!(bridge.contains("DmaStreamBuffer"));
    assert!(!bridge.contains("element_access"));
    assert!(!bridge.contains("stage_transfer"));
    assert!(!bridge.contains("dma_mmio_store_order"));
}
