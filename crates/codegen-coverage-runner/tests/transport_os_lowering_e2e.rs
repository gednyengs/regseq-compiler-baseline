#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::Path;

use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn model() -> regseq_model::RegseqModel {
    let input = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/_transport/transport-admission/input.rs");
    match try_bridge_fixture_path(&input) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("transport-admission bridge failed: {other:?}"),
    }
}

fn linux_model() -> regseq_model::RegseqModel {
    let mut model = model();
    let driver = model.device_lifecycle.drivers.values_mut().next().unwrap();
    driver.descriptor.bus_type = Some(regseq_model::BusType::Amba);
    driver.descriptor.match_set = vec![regseq_model::MatchEntry::Amba {
        id: 0x0004_10d3,
        mask: 0x000f_ffff,
    }];
    model
}

fn assert_absent(tu: &str) {
    for forbidden in [
        "reserve_slot",
        "publish_command",
        "harvest_command",
        "command_complete",
        "command_cancel",
        "correlation_id",
        "mbox_client_txdone",
        "tx_done",
        "register_callback",
    ] {
        assert!(!tu.contains(forbidden), "native output fabricated {forbidden}");
    }
}

#[test]
fn transport_linux_is_nonblocking_native_admission() {
    let tu = regseq_backends::emit_linux_c(&linux_model())
        .expect("emit Linux transport")
        .tu;
    assert!(tu.contains("#include <linux/mailbox_client.h>"));
    assert!(tu.contains("__endpoint->client->tx_block"));
    assert!(tu.contains("mbox_send_message(__endpoint->channel"));
    assert!(tu.contains("__reason = -EINVAL"));
    assert!(tu.contains("if (__reason >= 0)"));
    assert_absent(&tu);
}

#[test]
fn transport_zephyr_is_native_admission_only() {
    let tu = regseq_backends::emit_zephyr_c(&model())
        .expect("emit Zephyr transport")
        .tu;
    assert!(tu.contains("#include <zephyr/drivers/mbox.h>"));
    assert!(tu.contains("const struct mbox_dt_spec *__endpoint"));
    assert!(tu.contains("mbox_send(__endpoint->dev, __endpoint->channel_id"));
    assert!(tu.contains("if (__reason == 0)"));
    assert_absent(&tu);
}
