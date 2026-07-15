#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::Path;

use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn model() -> regseq_model::RegseqModel {
    let input = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/_network/network-full/input.rs");
    match try_bridge_fixture_path(&input) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("network-full bridge failed: {other:?}"),
    }
}

fn linux_model() -> regseq_model::RegseqModel {
    let mut model = model();
    model.device_lifecycle.drivers.values_mut().next().unwrap().ops =
        regseq_model::LifecycleOps::default();
    let interrupt = model.interrupt.drivers.values_mut().next().unwrap();
    interrupt.resources[0].binding.shared = false;
    model
}

fn zephyr_model() -> regseq_model::RegseqModel {
    let mut model = model();
    let compatible = model.strings.intern("regseq,netfull");
    let driver = model.device_lifecycle.drivers.values_mut().next().unwrap();
    driver.descriptor.bus_type = None;
    driver.descriptor.match_set = vec![regseq_model::MatchEntry::Devicetree {
        compatible,
    }];
    driver.ops = regseq_model::LifecycleOps::default();
    model.interrupt.drivers.clear();
    model.dma.drivers.clear();
    model
        .network_interface
        .drivers
        .values_mut()
        .next()
        .unwrap()
        .ops
        .set_published = None;
    model
}

#[test]
fn network_linux_lowers_packet_descriptor_index_and_lifecycle_ownership() {
    let tu = regseq_backends::emit_linux_c(&linux_model())
        .expect("emit Linux NIM")
        .tu;
    for needle in [
        "struct net_device_ops",
        ".ndo_open =",
        ".ndo_stop =",
        ".ndo_start_xmit =",
        ".ndo_get_stats64 =",
        "struct netfull_tx_descriptor",
        "tx_producer",
        "tx_consumer",
        "dma_map_single(",
        "dma_unmap_single(",
        "pcim_enable_device(pdev)",
        "dma_set_mask_and_coherent(&pdev->dev, DMA_BIT_MASK(32))",
        "NETDEV_TX_BUSY; /* rejection: caller retains packet */",
        "NETDEV_TX_OK; /* accepted: binding owns reclamation */",
        "dev_consume_skb_any(packet)",
        "register_netdev(priv->netdev)",
        "unregister_netdev(priv->netdev)",
        "request_irq(pdev->irq",
        "free_irq(pdev->irq, priv)",
        "cancel_work_sync(&priv->__rseq_work_0)",
    ] {
        assert!(tu.contains(needle), "missing Linux NIM proof {needle}");
    }
    let initialize = tu.find("priv->tx_ring = dma_alloc_coherent").unwrap();
    let publish = tu.find("register_netdev(priv->netdev)").unwrap();
    assert!(initialize < publish, "publication preceded initialization");

    let remove = tu.find("static void netfull_remove").unwrap();
    let teardown = &tu[remove..];
    let drain = teardown.find("cancel_work_sync").unwrap();
    let mask = teardown.find("disable_irq").unwrap();
    let withdraw = teardown.find("unregister_netdev").unwrap();
    assert!(drain < mask && mask < withdraw, "teardown is not D1 -> D2 -> withdraw");
    assert!(!tu.contains("packet_completed"));
    assert!(!tu.contains("packet_canceled"));
}

#[test]
fn network_zephyr_lowers_native_capabilities_and_rejects_runtime_publication() {
    let tu = regseq_backends::emit_zephyr_c(&zephyr_model())
        .expect("emit Zephyr NIM")
        .tu;
    for needle in [
        "net_if_up(iface)",
        "net_if_down(iface)",
        "net_if_send_data(iface, packet)",
        "net_recv_data(iface, packet)",
        "net_if_carrier_on(iface)",
        "net_if_dormant_on(iface)",
        "net_if_get_by_iface(iface)",
        "net_if_get_mtu(iface)",
        "net_if_set_mtu(iface, mtu)",
        "net_if_get_link_addr(iface)",
        "no packet completion is fabricated",
    ] {
        assert!(tu.contains(needle), "missing Zephyr NIM proof {needle}");
    }

    let error = regseq_backends::emit_zephyr_c(&model())
        .expect_err("runtime publication must remain absent on Zephyr");
    assert!(error
        .to_string()
        .contains("runtime publication is unsupported on Zephyr"));
}

#[test]
fn network_source_forbids_backend_mechanisms_in_portable_table() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../regseq-model/src/network.rs");
    let text = std::fs::read_to_string(root).unwrap();
    for forbidden in [
        "DescriptorRing",
        "ProducerIndex",
        "ConsumerIndex",
        "PollBudget",
        "Napi",
        "DmaMapping",
        "IrqBinding",
        "WorkQueue",
    ] {
        assert!(!text.contains(forbidden), "portable NIM source contains {forbidden}");
    }
}
