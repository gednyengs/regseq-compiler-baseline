//! C0-DWS native lowering gate. The historical fixture remains a 0-L1/0-L2
//! construct; this test only proves that its recovered model lowers onto each
//! target's existing work API and lifecycle scaffold.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::Path;

use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn bridged_reset() -> regseq_model::RegseqModel {
    let input = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/_deferred_work/deferred-work-reset/input.rs");
    match try_bridge_fixture_path(&input) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("deferred-work-reset bridge failed: {other:?}"),
    }
}

fn linux_model() -> regseq_model::RegseqModel {
    let mut model = bridged_reset();
    let driver = model
        .device_lifecycle
        .drivers
        .values_mut()
        .next()
        .expect("deferred-work-reset driver");
    // Keep the historical DWS fixture byte-exact. Linux's existing scaffold
    // requires a native registration fact, so adapt only this test model with
    // the already-supported AMBA vocabulary before exercising DWS lowering.
    driver.descriptor.bus_type = Some(regseq_model::BusType::Amba);
    driver
        .descriptor
        .match_set
        .push(regseq_model::MatchEntry::Amba {
            id: 0x0009_9001,
            mask: 0x000f_ffff,
        });
    model
}

#[test]
fn deferred_work_uses_native_linux_and_zephyr_work_apis() {
    let linux = regseq_backends::emit_linux_c(&linux_model())
        .expect("deferred work must lower for Linux")
        .tu;
    assert!(linux.contains("#include <linux/workqueue.h>"));
    assert!(linux.contains("struct work_struct __rseq_work_0;"));
    assert!(linux.contains("INIT_WORK(&priv->__rseq_work_0, dwreset_work_0);"));
    assert!(linux.contains("schedule_work(&__rseq_priv->__rseq_work_0);"));
    assert!(linux.contains("cancel_work_sync(&priv->__rseq_work_0);"));
    assert!(linux.contains("container_of(work, struct dwreset_priv, __rseq_work_0)"));
    assert!(
        linux.find("INIT_WORK(&priv->__rseq_work_0").unwrap()
            < linux.find("dwreset_on_init(&priv->inner)").unwrap(),
        "Linux initializes work before authored on_init"
    );
    assert!(
        linux.find("dwreset_on_deinit(&priv->inner)").unwrap_or(0)
            < linux
                .find("cancel_work_sync(&priv->__rseq_work_0)")
                .unwrap(),
        "Linux drains work after authored teardown and before storage release"
    );

    let zephyr = regseq_backends::emit_zephyr_c(&bridged_reset())
        .expect("deferred work must lower for Zephyr")
        .tu;
    assert!(zephyr.contains("#include <zephyr/kernel.h>"));
    assert!(zephyr.contains("struct k_work __rseq_work_0;"));
    assert!(zephyr.contains("k_work_init(&data->__rseq_work_0, dwreset_work_0);"));
    assert!(zephyr.contains("k_work_submit(&__rseq_data->__rseq_work_0);"));
    assert!(zephyr.contains("k_work_cancel_sync(&data->__rseq_work_0, &__sync)"));
    assert!(zephyr.contains("CONTAINER_OF(work, struct dwreset_data, __rseq_work_0)"));
    assert!(
        zephyr.find("k_work_init(&data->__rseq_work_0").unwrap()
            < zephyr.find("dwreset_on_init(&data->inner)").unwrap(),
        "Zephyr initializes work before authored on_init"
    );
}
