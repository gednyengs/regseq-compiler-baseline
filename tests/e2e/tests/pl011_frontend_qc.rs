//! Caution-focused frontend QC for the accepted PL011 polling-first slice.
//!
//! Reads the checked-in raw frontend artifact and asserts on the caution areas
//! flagged by the compiler collateral: firmware-property handling, optional
//! resource declarations, callback-family/context classification, and
//! register-access evidence for the real polling path.
//!
//! This QC stays at the raw frontend level -- no normalized model, no backend
//! lowering, no lifecycle-generated preamble assumptions.

use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn load_raw_artifact() -> serde_json::Value {
    let path = workspace_root().join("tests/fixtures/pl011-spec-slice/expected/raw-frontend.json");
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read raw artifact at {}: {e}", path.display()));
    serde_json::from_str(&content).expect("raw artifact is valid JSON")
}

// ---------------------------------------------------------------------------
// Driver identity
// ---------------------------------------------------------------------------

#[test]
fn qc_driver_identity_is_pl011_spec_slice() {
    let artifact = load_raw_artifact();
    assert_eq!(artifact["driver_name"], "pl011-spec-slice");
    assert_eq!(artifact["target_os"], "linux");

    let identity = &artifact["identity"];
    assert_eq!(identity["driver_name"], "pl011-spec-slice");
    assert_eq!(identity["device_class"], "serial");
    assert_eq!(identity["compatible"][0], "arm,pl011");
}

// ---------------------------------------------------------------------------
// Firmware-property declarations (accepted with caution)
// ---------------------------------------------------------------------------

#[test]
fn qc_property_reg_io_width_is_required_with_default() {
    let artifact = load_raw_artifact();
    let fields = artifact["firmware_properties"]["fields"]
        .as_array()
        .expect("firmware_properties.fields should be an array");

    let prop = fields
        .iter()
        .find(|f| f["property_id"] == "reg-io-width")
        .expect("reg-io-width property should be present");

    assert_eq!(prop["required"], true, "reg-io-width should be required");
    assert_eq!(
        prop["default_literal"], "4",
        "reg-io-width should declare default = 4"
    );
    assert_eq!(prop["field_type"], "u32");
    assert_eq!(prop["driver_type"], "Pl011SpecSlice");
}

#[test]
fn qc_property_current_speed_is_optional_no_default() {
    let artifact = load_raw_artifact();
    let fields = artifact["firmware_properties"]["fields"]
        .as_array()
        .expect("firmware_properties.fields should be an array");

    let prop = fields
        .iter()
        .find(|f| f["property_id"] == "current-speed")
        .expect("current-speed property should be present");

    assert_eq!(prop["required"], false, "current-speed should be optional");
    assert!(
        prop["default_literal"].is_null(),
        "current-speed should have no default"
    );
    assert_eq!(prop["field_type"], "Option<u32>");
}

#[test]
fn qc_property_lint_flags_required_with_default() {
    let artifact = load_raw_artifact();
    let lints = artifact["firmware_properties"]["lints"]
        .as_array()
        .expect("firmware_properties.lints should be an array");

    let lint = lints
        .iter()
        .find(|l| l["property_id"] == "reg-io-width")
        .expect("RequiredWithDefault lint for reg-io-width should be present");

    assert_eq!(lint["kind"], "RequiredWithDefault");
    assert_eq!(lint["driver_type"], "Pl011SpecSlice");
}

// ---------------------------------------------------------------------------
// Optional resource declarations
// ---------------------------------------------------------------------------

#[test]
fn qc_clock_resource_is_optional() {
    let artifact = load_raw_artifact();
    let clock_fields = artifact["clock"]["fields"]
        .as_array()
        .expect("clock.fields should be an array");

    assert_eq!(clock_fields.len(), 1, "expected exactly one clock field");
    let clock = &clock_fields[0];
    assert_eq!(clock["field_name"], "clock");
    assert_eq!(clock["clock_id"], "uart_clk");
    assert_eq!(
        clock["required"], false,
        "clock should be optional (Option<Clock>)"
    );
}

#[test]
fn qc_reset_resource_is_optional() {
    let artifact = load_raw_artifact();
    let reset_fields = artifact["reset"]["fields"]
        .as_array()
        .expect("reset.fields should be an array");

    assert_eq!(reset_fields.len(), 1, "expected exactly one reset field");
    let reset = &reset_fields[0];
    assert_eq!(reset["field_name"], "reset");
    assert_eq!(
        reset["required"], false,
        "reset should be optional (Option<ResetLine>)"
    );
}

#[test]
fn qc_pinctrl_resource_is_optional_with_id() {
    let artifact = load_raw_artifact();
    let pinctrl_fields = artifact["pinctrl"]["fields"]
        .as_array()
        .expect("pinctrl.fields should be an array");

    assert_eq!(
        pinctrl_fields.len(),
        1,
        "expected exactly one pinctrl field"
    );
    let pinctrl = &pinctrl_fields[0];
    assert_eq!(pinctrl["field_name"], "pinctrl");
    assert_eq!(pinctrl["pinctrl_id"], "uart_pins");
    assert_eq!(
        pinctrl["required"], false,
        "pinctrl should be optional (Option<PinCtrl>)"
    );
}

// ---------------------------------------------------------------------------
// Callback-family and callback-context classification
// ---------------------------------------------------------------------------

#[test]
fn qc_lifecycle_callback_family_captured() {
    let artifact = load_raw_artifact();
    let lifecycle = artifact["trait_callbacks"]["lifecycle_impls"]
        .as_array()
        .expect("lifecycle_impls should be an array");

    assert_eq!(lifecycle.len(), 1);
    assert_eq!(lifecycle[0]["driver_type"], "Pl011SpecSlice");
    assert_eq!(
        lifecycle[0]["context"], "F",
        "Lifecycle should be framework-managed (F)"
    );
}

#[test]
fn qc_serial_callback_family_captured() {
    let artifact = load_raw_artifact();
    let serial = artifact["trait_callbacks"]["serial_impls"]
        .as_array()
        .expect("serial_impls should be an array");

    assert_eq!(serial.len(), 1);
    assert_eq!(serial[0]["driver_type"], "Pl011SpecSlice");
    assert_eq!(
        serial[0]["context"], "S",
        "Serial should be subsystem-serialized (S)"
    );
}

#[test]
fn qc_no_serial_console_or_pm_callbacks() {
    let artifact = load_raw_artifact();
    let console = artifact["trait_callbacks"]["serial_console_impls"]
        .as_array()
        .expect("serial_console_impls should be an array");
    let pm = artifact["trait_callbacks"]["power_management_impls"]
        .as_array()
        .expect("power_management_impls should be an array");

    assert!(console.is_empty(), "no SerialConsole in the PL011 slice");
    assert!(pm.is_empty(), "no PowerManagement in the PL011 slice");
}

// ---------------------------------------------------------------------------
// Register-access raw evidence for the polling path
// ---------------------------------------------------------------------------

fn wrapper_sites(artifact: &serde_json::Value) -> Vec<(String, String)> {
    artifact["register_access"]["wrapper_call_sites"]
        .as_array()
        .expect("wrapper_call_sites should be an array")
        .iter()
        .map(|s| {
            (
                s["method"].as_str().unwrap().to_string(),
                s["containing_fn"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

#[test]
fn qc_baud_rate_register_access_evidence() {
    let artifact = load_raw_artifact();
    let sites = wrapper_sites(&artifact);

    // set_baud_rate writes to IBRD and FBRD
    let baud_writes: Vec<_> = sites
        .iter()
        .filter(|(method, func)| method == "write" && func == "set_baud_rate")
        .collect();
    assert!(
        baud_writes.len() >= 2,
        "expected at least 2 write sites in set_baud_rate (IBRD + FBRD), found {}",
        baud_writes.len()
    );
}

#[test]
fn qc_line_control_register_access_evidence() {
    let artifact = load_raw_artifact();
    let sites = wrapper_sites(&artifact);

    let lcr_writes: Vec<_> = sites
        .iter()
        .filter(|(method, func)| method == "write" && func == "configure_line_control")
        .collect();
    assert!(
        !lcr_writes.is_empty(),
        "expected write site in configure_line_control (LCR_H)"
    );
}

#[test]
fn qc_data_register_access_evidence() {
    let artifact = load_raw_artifact();
    let sites = wrapper_sites(&artifact);

    // tx_byte writes to DR
    let tx_writes: Vec<_> = sites
        .iter()
        .filter(|(method, func)| method == "write" && func == "tx_byte")
        .collect();
    assert!(!tx_writes.is_empty(), "expected write site in tx_byte (DR)");

    // rx_char reads from DR (and FR)
    let rx_reads: Vec<_> = sites
        .iter()
        .filter(|(method, func)| method == "read" && func == "rx_char")
        .collect();
    assert!(
        !rx_reads.is_empty(),
        "expected read site in rx_char (DR/FR)"
    );
}

#[test]
fn qc_flag_register_access_evidence() {
    let artifact = load_raw_artifact();
    let sites = wrapper_sites(&artifact);

    // tx_ready reads FR
    let fr_reads: Vec<_> = sites
        .iter()
        .filter(|(method, func)| method == "read" && func == "tx_ready")
        .collect();
    assert!(!fr_reads.is_empty(), "expected read site in tx_ready (FR)");

    // tx_empty reads FR (BUSY flag)
    let tx_empty_reads: Vec<_> = sites
        .iter()
        .filter(|(method, func)| method == "read" && func == "tx_empty")
        .collect();
    assert!(
        !tx_empty_reads.is_empty(),
        "expected read site in tx_empty (FR)"
    );
}

#[test]
fn qc_v1_fifo_flush_register_access_evidence() {
    let artifact = load_raw_artifact();
    let sites = wrapper_sites(&artifact);

    // flush_rx_fifo reads DR/FR to drain the FIFO
    let flush_rx_reads: Vec<_> = sites
        .iter()
        .filter(|(method, func)| {
            (method == "read" || method == "modify") && func == "flush_rx_fifo"
        })
        .collect();
    assert!(
        !flush_rx_reads.is_empty(),
        "expected register access in flush_rx_fifo"
    );

    // flush_tx_fifo modifies LCR_H (FEN toggle)
    let flush_tx_mods: Vec<_> = sites
        .iter()
        .filter(|(method, func)| method == "modify" && func == "flush_tx_fifo")
        .collect();
    assert!(
        !flush_tx_mods.is_empty(),
        "expected modify site in flush_tx_fifo (LCR_H)"
    );
}

#[test]
fn qc_v1_send_break_register_access_evidence() {
    let artifact = load_raw_artifact();
    let sites = wrapper_sites(&artifact);

    // send_break modifies LCR_H (BRK bit)
    let brk_mods: Vec<_> = sites
        .iter()
        .filter(|(method, func)| method == "modify" && func == "send_break")
        .collect();
    assert!(
        !brk_mods.is_empty(),
        "expected modify site in send_break (LCR_H)"
    );
}

// ---------------------------------------------------------------------------
// Register block and MMIO surface
// ---------------------------------------------------------------------------

#[test]
fn qc_register_block_pl011registers_present() {
    let artifact = load_raw_artifact();
    let blocks = artifact["register_access"]["register_blocks"]
        .as_array()
        .expect("register_blocks should be an array");

    let pl011 = blocks
        .iter()
        .find(|b| b["name"] == "Pl011Registers")
        .expect("Pl011Registers register block should be present");

    let field_names: Vec<&str> = pl011["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["name"].as_str().unwrap())
        .collect();

    assert!(field_names.contains(&"dr"), "register block should have dr");
    assert!(field_names.contains(&"fr"), "register block should have fr");
    assert!(
        field_names.contains(&"ibrd"),
        "register block should have ibrd"
    );
    assert!(
        field_names.contains(&"fbrd"),
        "register block should have fbrd"
    );
    assert!(
        field_names.contains(&"lcr_h"),
        "register block should have lcr_h"
    );
    assert!(field_names.contains(&"cr"), "register block should have cr");
}

#[test]
fn qc_mmio_field_links_to_register_block() {
    let artifact = load_raw_artifact();
    let mmio = artifact["register_access"]["mmio_fields"]
        .as_array()
        .expect("mmio_fields should be an array");

    assert_eq!(mmio.len(), 1);
    assert_eq!(mmio[0]["field_name"], "regs");
    assert_eq!(mmio[0]["mmio_id"], "uart_regs");
    assert_eq!(mmio[0]["register_block_type"], "Pl011Registers");
}

// ---------------------------------------------------------------------------
// Register descriptor facts
// ---------------------------------------------------------------------------

#[test]
fn qc_register_descriptors_cover_polling_subset() {
    let artifact = load_raw_artifact();
    let descriptors = artifact["register_access"]["register_descriptors"]
        .as_array()
        .expect("register_descriptors should be an array");

    let module_names: Vec<&str> = descriptors
        .iter()
        .map(|d| d["register_module"].as_str().unwrap())
        .collect();

    for expected in &[
        "DR_REG", "FR_REG", "IBRD_REG", "FBRD_REG", "LCRH_REG", "CR_REG",
    ] {
        assert!(
            module_names.contains(expected),
            "expected register descriptor module {expected}"
        );
    }
}

#[test]
fn qc_cr_reg_descriptor_has_uarten_txe_rxe() {
    let artifact = load_raw_artifact();
    let descriptors = artifact["register_access"]["register_descriptors"]
        .as_array()
        .unwrap();

    let cr = descriptors
        .iter()
        .find(|d| d["register_module"] == "CR_REG")
        .expect("CR_REG descriptor should be present");

    let field_names: Vec<&str> = cr["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["field_name"].as_str().unwrap())
        .collect();

    assert!(field_names.contains(&"UARTEN"));
    assert!(field_names.contains(&"TXE"));
    assert!(field_names.contains(&"RXE"));
}

#[test]
fn qc_fr_reg_descriptor_has_busy_field() {
    let artifact = load_raw_artifact();
    let descriptors = artifact["register_access"]["register_descriptors"]
        .as_array()
        .unwrap();

    let fr = descriptors
        .iter()
        .find(|d| d["register_module"] == "FR_REG")
        .expect("FR_REG descriptor should be present");

    let field_names: Vec<&str> = fr["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["field_name"].as_str().unwrap())
        .collect();

    assert!(
        field_names.contains(&"BUSY"),
        "FR_REG should have BUSY field for tx_empty"
    );
}

#[test]
fn qc_lcrh_reg_descriptor_has_brk_field() {
    let artifact = load_raw_artifact();
    let descriptors = artifact["register_access"]["register_descriptors"]
        .as_array()
        .unwrap();

    let lcrh = descriptors
        .iter()
        .find(|d| d["register_module"] == "LCRH_REG")
        .expect("LCRH_REG descriptor should be present");

    let field_names: Vec<&str> = lcrh["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["field_name"].as_str().unwrap())
        .collect();

    assert!(
        field_names.contains(&"BRK"),
        "LCRH_REG should have BRK field for send_break"
    );
}
