//! C1-serial native lowering and capability-absence shape gates.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::manifest;
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

fn fixture(name: &str) -> manifest::Fixture {
    let corpus: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_serial");
    manifest::discover(&corpus)
        .expect("discover serial corpus")
        .into_iter()
        .find(|fixture| fixture.manifest.name == name)
        .unwrap_or_else(|| panic!("{name} fixture"))
}

fn model(name: &str) -> regseq_model::RegseqModel {
    let fixture = fixture(name);
    match try_bridge_fixture_path(&fixture.input) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("{name} bridge failed: {other:?}"),
    }
}

fn linux_model(name: &str) -> regseq_model::RegseqModel {
    let mut model = model(name);
    let driver = model
        .device_lifecycle
        .drivers
        .values_mut()
        .next()
        .expect("serial driver");
    driver.descriptor.bus_type = Some(regseq_model::BusType::Amba);
    driver
        .descriptor
        .match_set
        .push(regseq_model::MatchEntry::Amba {
            id: 0x0004_1011,
            mask: 0x000f_ffff,
        });
    model
}

#[test]
fn serial_linux_uses_uart_ops_and_counted_buffer_adapters() {
    let emit =
        regseq_backends::emit_linux_c(&linux_model("serial-port-all")).expect("Linux serial emit");
    let tu = emit.tu;
    assert!(tu.contains("#include <linux/serial_core.h>"));
    assert!(tu.contains("struct uart_port __rseq_uart_port;"));
    assert!(tu.contains("static const struct uart_ops serial_port_all_uart_ops"));
    assert!(tu.contains("static __maybe_unused size_t serial_port_all_serial_transmit_bytes"));
    assert!(tu.contains("SerialTxBuffer buffer = { .data = (u8 *)data, .len = len }"));
    assert!(tu.contains("static __maybe_unused size_t serial_port_all_serial_receive_bytes"));
    assert!(tu.contains(".errors = errors"));
    assert!(tu.contains(".startup = serial_port_all_uart_startup"));
    assert!(tu.contains(".shutdown = serial_port_all_uart_shutdown"));
    assert!(tu.contains(".tx_empty = serial_port_all_uart_tx_empty"));
    assert!(tu.contains(".set_mctrl = serial_port_all_uart_set_mctrl"));
    assert!(tu.contains(".get_mctrl = serial_port_all_uart_get_mctrl"));
    assert!(tu.contains(".break_ctl = serial_port_all_uart_break_ctl"));
    assert!(tu.contains(".set_termios = serial_port_all_uart_set_termios"));
    assert!(tu.contains(".release_port = serial_port_all_uart_release_port"));
    assert!(tu.contains(".request_port = serial_port_all_uart_request_port"));
    assert!(tu.contains(".config_port = serial_port_all_uart_config_port"));
    assert!(tu.contains(
        "static __maybe_unused struct SerialParams serial_port_all_serial_get_config_snapshot"
    ));
    assert!(tu.contains("restoring the prior applied snapshot"));
    assert!(tu.contains("no completion, abort outcome, or device behavior is synthesized"));
}

#[test]
fn serial_linux_mctrl_shadow_tracks_authored_signal_changes() {
    let tu = regseq_backends::emit_linux_c(&linux_model("serial-port-all"))
        .expect("Linux serial emit")
        .tu;
    let probe = tu
        .find("static int serial_port_all_probe")
        .map(|start| &tu[start..])
        .expect("serial probe");
    let seed = probe
        .find("priv->__rseq_mctrl = 0;")
        .expect("mctrl shadow seed");
    let authored_init = probe
        .find("serial_port_all_on_init(&priv->inner)")
        .expect("authored on_init call");
    assert!(
        seed < authored_init,
        "authored on_init must be able to advance the seeded mctrl shadow"
    );

    let setter = tu
        .rfind("static struct SerialStatus serial_port_all_serial_set_signals")
        .map(|start| &tu[start..])
        .expect("authored set_signals definition");
    let setter = &setter[..setter.find("\n}\n").expect("set_signals end")];
    assert!(
        setter.contains("container_of(_1, struct serial_port_all_priv, inner)->__rseq_mctrl"),
        "set_signals does not update the mctrl shadow:\n{setter}"
    );
    assert!(setter.contains("_2.dtr ? TIOCM_DTR : 0"));
    assert!(setter.contains("_2.rts ? TIOCM_RTS : 0"));
}

#[test]
fn serial_zephyr_uses_uart_driver_api_and_preserves_progress_counts() {
    let emit =
        regseq_backends::emit_zephyr_c(&model("serial-port-all")).expect("Zephyr serial emit");
    let tu = emit.tu;
    assert!(tu.contains("#include <zephyr/drivers/uart.h>"));
    assert!(tu.contains("static DEVICE_API(uart, serial_port_all_uart_api)"));
    assert!(tu.contains(".poll_out = serial_port_all_uart_poll_out"));
    assert!(tu.contains(".poll_in = serial_port_all_uart_poll_in"));
    assert!(tu.contains(".fifo_fill = serial_port_all_uart_fifo_fill"));
    assert!(tu.contains(".fifo_read = serial_port_all_uart_fifo_read"));
    assert!(tu.contains("return (int)serial_port_all_serial_transmit"));
    assert!(tu.contains("if (serial_port_all_serial_receive(&data->inner, buffer) != 1)"));
    assert!(tu.contains("data->__rseq_line_errors.overrun |= errors.overrun"));
    assert!(tu.contains("return moved;"));
    assert!(tu.contains(".configure = serial_port_all_uart_configure"));
    assert!(tu.contains(".config_get = serial_port_all_uart_config_get"));
    assert!(tu.contains("&serial_port_all_uart_api"));
    assert!(!tu.contains("RSEQ-DEFERRED-CLASS: device_class \"serial\""));
    assert!(tu.contains("serial-hardware-ops.break-send is unsupported on Zephyr"));
}

#[test]
fn serial_native_bindings_do_not_fabricate_absent_capabilities() {
    let linux = regseq_backends::emit_linux_c(&linux_model("serial-port-minimal"))
        .expect("minimal Linux serial emit")
        .tu;
    assert!(
        linux.contains("static __maybe_unused size_t serial_port_minimal_serial_transmit_bytes")
    );
    for required in [
        ".tx_empty = serial_port_minimal_uart_tx_empty",
        ".set_mctrl = serial_port_minimal_uart_set_mctrl",
        ".get_mctrl = serial_port_minimal_uart_get_mctrl",
        ".stop_tx = serial_port_minimal_uart_stop_tx",
        ".start_tx = serial_port_minimal_uart_start_tx",
        ".stop_rx = serial_port_minimal_uart_stop_rx",
        ".break_ctl = serial_port_minimal_uart_break_ctl",
        ".startup = serial_port_minimal_uart_startup",
        ".shutdown = serial_port_minimal_uart_shutdown",
        ".flush_buffer = serial_port_minimal_uart_flush_buffer",
        ".set_termios = serial_port_minimal_uart_set_termios",
        ".type = serial_port_minimal_uart_type",
        ".release_port = serial_port_minimal_uart_release_port",
        ".request_port = serial_port_minimal_uart_request_port",
        ".config_port = serial_port_minimal_uart_config_port",
        ".verify_port = serial_port_minimal_uart_verify_port",
    ] {
        assert!(
            linux.contains(required),
            "missing serial-core contract op {required}"
        );
    }
    assert!(linux.contains("serial-core requires unsupported CTS/DCD/DSR permanently active"));
    assert!(linux.contains("No authored session transition is required"));
    assert!(linux.contains("devm owns the already-claimed AMBA region"));
    assert!(!linux.contains("RSEQ-SERIAL-CAPABILITY subscribe_events"));
    assert!(!linux.contains("RSEQ-SERIAL-CAPABILITY abort"));

    let zephyr = regseq_backends::emit_zephyr_c(&model("serial-port-minimal"))
        .expect("minimal Zephyr serial emit")
        .tu;
    assert!(zephyr.contains(".poll_out = serial_port_minimal_uart_poll_out"));
    assert!(zephyr.contains(".fifo_fill = serial_port_minimal_uart_fifo_fill"));
    assert!(!zephyr.contains(".poll_in ="));
    assert!(!zephyr.contains(".configure ="));
    assert!(!zephyr.contains(".config_get ="));
    assert!(!zephyr.contains(".irq_tx_complete ="));
    assert!(!zephyr.contains("RSEQ-SERIAL-CAPABILITY subscribe_events"));
    assert!(!zephyr.contains("RSEQ-SERIAL-CAPABILITY abort"));
}
