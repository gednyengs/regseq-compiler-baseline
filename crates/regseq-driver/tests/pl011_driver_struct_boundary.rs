//! Boundary validation test: source-to-extraction-03
//!
//! Validates that the authored PL011 driver-struct declaration facts survive
//! at the compiler-side extraction boundary. Uses a reduced inline specimen
//! whose fidelity against the real fixture source is verified as a precondition.

#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::sync::{Arc, Mutex, Once};

// Extraction surfaces used for validation.
#[path = "../src/extract/driver_identity.rs"]
mod driver_identity;

#[path = "../src/extract/identity_enrichment_types.rs"]
mod identity_enrichment_types;

#[path = "../src/extract/identity_enrichment.rs"]
mod identity_enrichment;

#[path = "../src/extract/callback_context.rs"]
mod callback_context;

#[path = "../src/extract/register_access.rs"]
mod register_access;

#[path = "../src/extract/interrupts.rs"]
mod interrupts;

#[path = "../src/extract/clock.rs"]
mod clock;

#[path = "../src/extract/reset.rs"]
mod reset;

#[path = "../src/extract/pinctrl.rs"]
mod pinctrl;

#[path = "../src/extract/property.rs"]
mod property;

use clock::{extract_clock_surface, ClockSurface};
use driver_identity::{extract_driver_identities, DriverIdentity, DriverIdentityError};
use identity_enrichment::{extract_identity_enrichment, IdentityEnrichmentSurface};
use identity_enrichment_types::*;
use interrupts::{extract_interrupt_surface, InterruptSurface};
use pinctrl::{extract_pinctrl_surface, PinCtrlSurface};
use property::{extract_property_surface, PropertySurface};
use register_access::{extract_register_access_surface, RegisterAccessSurface};
use reset::{extract_reset_surface, ResetSurface};

// ---------------------------------------------------------------------------
// Shared compiler harness
// ---------------------------------------------------------------------------

static INIT: Once = Once::new();
static COMPILER_LOCK: Mutex<()> = Mutex::new(());

fn init_rustc() {
    INIT.call_once(|| {
        let early_dcx =
            rustc_session::EarlyDiagCtxt::new(rustc_session::config::ErrorOutputType::default());
        rustc_driver::init_rustc_env_logger(&early_dcx);
    });
}

fn get_sysroot() -> String {
    let output = std::process::Command::new("rustc")
        .arg("--print")
        .arg("sysroot")
        .output()
        .expect("failed to run rustc --print sysroot");
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

/// All extraction surfaces collected in a single compiler pass.
struct AllSurfaces {
    driver_identities: Result<Vec<DriverIdentity>, DriverIdentityError>,
    identity_enrichment: Result<IdentityEnrichmentSurface, IdentityEnrichmentError>,
    register_access: Result<RegisterAccessSurface, register_access::RegisterAccessError>,
    interrupt: Result<InterruptSurface, interrupts::InterruptError>,
    clock: Result<ClockSurface, clock::ClockError>,
    reset: Result<ResetSurface, reset::ResetError>,
    pinctrl: Result<PinCtrlSurface, pinctrl::PinCtrlError>,
    property: Result<PropertySurface, property::PropertyError>,
}

fn compile_and_extract_all(source: &str, test_name: &str) -> AllSurfaces {
    let _lock = COMPILER_LOCK.lock().unwrap();
    init_rustc();

    let test_dir = std::env::temp_dir()
        .join("regseq-driver-tests")
        .join(test_name);
    if test_dir.exists() {
        std::fs::remove_dir_all(&test_dir).unwrap();
    }
    std::fs::create_dir_all(&test_dir).unwrap();

    let src_path = test_dir.join("test_input.rs");
    std::fs::write(&src_path, source).unwrap();

    let sysroot = get_sysroot();

    let result: Arc<Mutex<Option<AllSurfaces>>> = Arc::new(Mutex::new(None));

    struct ExtractCallbacks {
        result: Arc<Mutex<Option<AllSurfaces>>>,
    }

    impl rustc_driver::Callbacks for ExtractCallbacks {
        fn config(&mut self, config: &mut rustc_interface::interface::Config) {
            config.opts.unstable_opts.no_codegen = true;
            config
                .opts
                .unstable_opts
                .crate_attr
                .push("feature(register_tool)".to_string());
            config
                .opts
                .unstable_opts
                .crate_attr
                .push("register_tool(rseq)".to_string());
        }

        fn after_analysis<'tcx>(
            &mut self,
            _compiler: &rustc_interface::interface::Compiler,
            tcx: rustc_middle::ty::TyCtxt<'tcx>,
        ) -> rustc_driver::Compilation {
            let surfaces = AllSurfaces {
                driver_identities: extract_driver_identities(tcx),
                identity_enrichment: extract_identity_enrichment(tcx),
                register_access: extract_register_access_surface(tcx),
                interrupt: extract_interrupt_surface(tcx),
                clock: extract_clock_surface(tcx),
                reset: extract_reset_surface(tcx),
                pinctrl: extract_pinctrl_surface(tcx),
                property: extract_property_surface(tcx),
            };
            *self.result.lock().unwrap() = Some(surfaces);
            rustc_driver::Compilation::Stop
        }
    }

    let mut callbacks = ExtractCallbacks {
        result: result.clone(),
    };

    let args = vec![
        "rustc".to_string(),
        src_path.to_str().unwrap().to_string(),
        "--sysroot".to_string(),
        sysroot,
        "--edition".to_string(),
        "2021".to_string(),
        "--crate-type".to_string(),
        "lib".to_string(),
        "--crate-name".to_string(),
        "pl011_full_spec".to_string(),
        "--out-dir".to_string(),
        test_dir.to_str().unwrap().to_string(),
    ];

    rustc_driver::run_compiler(&args, &mut callbacks);

    let extracted = result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis should have been called");

    std::fs::remove_dir_all(&test_dir).ok();

    extracted
}

// ---------------------------------------------------------------------------
// Reduced inline specimen
// ---------------------------------------------------------------------------

/// Reduced inline specimen of the PL011 driver struct. Preserves exactly the
/// struct declaration, all `#[rseq::*]` attributes, field names, and field
/// types relevant to the driver-struct check contract. Method bodies and
/// helper functions are omitted because they are not in scope for this check.
///
/// Specimen fidelity against the real fixture source
/// (`tests/fixtures/pl011-full-spec/src/driver.rs`) is verified by the
/// `specimen_fidelity` test below.
const PL011_DRIVER_STRUCT_SPECIMEN: &str = r#"
pub mod driver {
    pub struct Mmio<T> { _phantom: core::marker::PhantomData<T> }
    pub struct Interrupt;
    pub struct Clock;
    pub struct ResetLine;
    pub struct PinCtrl;

    pub mod registers {
        #[rseq::register_block]
        pub struct Pl011Registers {
            pub dr: u32,
        }
    }

    use registers::Pl011Registers;

    #[rseq::driver(name = "pl011-full-spec", compatible = ["arm,pl011"], bus = "amba")]
    #[rseq::device_class(serial)]
    pub struct Pl011FullSpec {
        #[rseq::mmio(id = "uart_regs")]
        pub regs: Mmio<Pl011Registers>,

        #[rseq::property(id = "current-speed", default = 115200)]
        pub current_speed: u32,

        #[rseq::property(id = "fifo-size", default = 32)]
        pub fifo_size: u32,

        #[rseq::interrupt(id = "uart_irq")]
        pub irq: Interrupt,

        #[rseq::clock(id = "uart_clk")]
        pub clock: Option<Clock>,

        pub reset: Option<ResetLine>,

        #[rseq::pinctrl(id = "uart_pins")]
        pub pinctrl: Option<PinCtrl>,

        pending_tx: Option<u8>,
    }
}
"#;

// ---------------------------------------------------------------------------
// Specimen fidelity verification
// ---------------------------------------------------------------------------

/// Read the real fixture source file and extract the exact lines constituting
/// the authored driver struct declaration (from `#[rseq::driver` through the
/// closing `}`). This is used to verify specimen fidelity.
fn read_fixture_driver_struct_lines() -> Vec<String> {
    let fixture_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/pl011-full-spec/src/driver.rs"
    );
    let content =
        std::fs::read_to_string(fixture_path).expect("fixture driver.rs should be readable");
    content.lines().map(|l| l.to_string()).collect()
}

/// Extract the struct declaration block from the fixture source.
fn extract_fixture_struct_block(lines: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    let mut in_struct = false;
    let mut brace_depth = 0i32;

    for line in lines {
        let trimmed = line.trim();
        if !in_struct {
            if trimmed.starts_with("#[rseq::driver") {
                in_struct = true;
                result.push(trimmed.to_string());
            }
            continue;
        }

        result.push(trimmed.to_string());

        for ch in trimmed.chars() {
            if ch == '{' {
                brace_depth += 1;
            } else if ch == '}' {
                brace_depth -= 1;
                if brace_depth == 0 {
                    return result;
                }
            }
        }
    }

    result
}

#[test]
fn specimen_fidelity() {
    // Verify that every authored fact from the check contract matches the
    // real fixture source.
    let lines = read_fixture_driver_struct_lines();
    let struct_block = extract_fixture_struct_block(&lines);
    let block_text = struct_block.join("\n");

    // Fidelity assertion 1: driver attribute
    assert!(
        block_text.contains(
            r#"#[rseq::driver(name = "pl011-full-spec", compatible = ["arm,pl011"], bus = "amba")]"#
        ),
        "fixture must contain exact driver attribute; got:\n{block_text}"
    );

    // Fidelity assertion 2: device class
    assert!(
        block_text.contains("#[rseq::device_class(serial)]"),
        "fixture must contain exact device_class attribute"
    );

    // Fidelity assertion 3: struct name
    assert!(
        block_text.contains("pub struct Pl011FullSpec"),
        "fixture must contain struct Pl011FullSpec"
    );

    // Fidelity assertion 4: regs field
    assert!(
        block_text.contains(r#"#[rseq::mmio(id = "uart_regs")]"#),
        "fixture must contain mmio attribute for regs"
    );
    assert!(
        block_text.contains("pub regs: Mmio<Pl011Registers>"),
        "fixture must contain regs field with type Mmio<Pl011Registers>"
    );

    // Fidelity assertion 5: irq field
    assert!(
        block_text.contains(r#"#[rseq::interrupt(id = "uart_irq")]"#),
        "fixture must contain interrupt attribute for irq"
    );
    assert!(
        block_text.contains("pub irq: Interrupt"),
        "fixture must contain irq field"
    );

    // Fidelity assertion 6: clock field
    assert!(
        block_text.contains(r#"#[rseq::clock(id = "uart_clk")]"#),
        "fixture must contain clock attribute"
    );
    assert!(
        block_text.contains("pub clock: Option<Clock>"),
        "fixture must contain clock field with Option<Clock>"
    );

    // Fidelity assertion 7: reset field
    assert!(
        block_text.contains("pub reset: Option<ResetLine>"),
        "fixture must contain reset field with Option<ResetLine>"
    );

    // Fidelity assertion 8: pinctrl field
    assert!(
        block_text.contains(r#"#[rseq::pinctrl(id = "uart_pins")]"#),
        "fixture must contain pinctrl attribute"
    );
    assert!(
        block_text.contains("pub pinctrl: Option<PinCtrl>"),
        "fixture must contain pinctrl field with Option<PinCtrl>"
    );

    // Fidelity assertion 9: current_speed property
    assert!(
        block_text.contains(r#"#[rseq::property(id = "current-speed", default = 115200)]"#),
        "fixture must contain current_speed property attribute"
    );
    assert!(
        block_text.contains("pub current_speed: u32"),
        "fixture must contain current_speed field"
    );

    // Fidelity assertion 10: fifo_size property
    assert!(
        block_text.contains(r#"#[rseq::property(id = "fifo-size", default = 32)]"#),
        "fixture must contain fifo_size property attribute"
    );
    assert!(
        block_text.contains("pub fifo_size: u32"),
        "fixture must contain fifo_size field"
    );

    // Fidelity assertion 11: pending_tx user field
    assert!(
        block_text.contains("pending_tx: Option<u8>"),
        "fixture must contain pending_tx field"
    );

    // Fidelity assertion 12: pending_tx must NOT carry any rseq attribute
    // Verify by checking the line immediately before `pending_tx`.
    let pending_tx_idx = struct_block
        .iter()
        .position(|l| l.contains("pending_tx:"))
        .expect("pending_tx line should exist");
    if pending_tx_idx > 0 {
        let prev_line = &struct_block[pending_tx_idx - 1];
        assert!(
            !prev_line.contains("#[rseq::"),
            "pending_tx must not be preceded by an rseq attribute; got: {prev_line}"
        );
    }

    // Fidelity assertion 13: reset field must NOT carry an rseq attribute
    // (join key is Field name, not Properties.id)
    let reset_idx = struct_block
        .iter()
        .position(|l| l.contains("pub reset: Option<ResetLine>"))
        .expect("reset line should exist");
    if reset_idx > 0 {
        let prev_line = &struct_block[reset_idx - 1];
        assert!(
            !prev_line.contains("#[rseq::"),
            "reset field must not carry an rseq attribute; got: {prev_line}"
        );
    }

    // Fidelity assertion 14: specimen matches all authored field names
    let specimen = PL011_DRIVER_STRUCT_SPECIMEN;
    for field_name in &[
        "regs",
        "current_speed",
        "fifo_size",
        "irq",
        "clock",
        "reset",
        "pinctrl",
        "pending_tx",
    ] {
        assert!(
            specimen.contains(field_name),
            "specimen must contain field {field_name}"
        );
    }
}

// ---------------------------------------------------------------------------
// Check Group 1: Driver identity (name, compatible, bus)
// ---------------------------------------------------------------------------

#[test]
fn check_group_1_driver_identity() {
    let surfaces =
        compile_and_extract_all(PL011_DRIVER_STRUCT_SPECIMEN, "driver_struct_boundary_cg1");

    let identities = surfaces
        .driver_identities
        .expect("driver identity extraction must succeed");
    assert_eq!(
        identities.len(),
        1,
        "exactly one driver identity must be extracted"
    );

    let id = &identities[0];

    // Check 1a: name preserved
    assert_eq!(
        id.driver_name, "pl011-full-spec",
        "driver name must be 'pl011-full-spec'"
    );

    // Check 1b: compatible preserved
    assert_eq!(
        id.compatible,
        vec!["arm,pl011"],
        "compatible must be exactly [\"arm,pl011\"]"
    );

    // Check 1c: bus preserved
    assert_eq!(
        id.bus,
        Some("amba".to_string()),
        "bus must be Some(\"amba\")"
    );
}

// ---------------------------------------------------------------------------
// Check Group 2: Device class
// ---------------------------------------------------------------------------

#[test]
fn check_group_2_device_class() {
    let surfaces =
        compile_and_extract_all(PL011_DRIVER_STRUCT_SPECIMEN, "driver_struct_boundary_cg2");

    let identities = surfaces
        .driver_identities
        .expect("driver identity extraction must succeed");
    assert_eq!(identities.len(), 1);

    // Check 2: device class is serial
    assert_eq!(
        identities[0].device_class, "serial",
        "device class must be 'serial'"
    );
}

// ---------------------------------------------------------------------------
// Check Group 3: Resource fields
// ---------------------------------------------------------------------------

#[test]
fn check_group_3_resources() {
    let surfaces =
        compile_and_extract_all(PL011_DRIVER_STRUCT_SPECIMEN, "driver_struct_boundary_cg3");

    // -- Identity enrichment: field-level rseq classification --
    let enrichment = surfaces
        .identity_enrichment
        .expect("identity enrichment extraction must succeed");

    let driver_struct = enrichment
        .structs
        .iter()
        .find(|s| s.is_driver)
        .expect("driver struct must be found in enrichment");

    // Verify canonical path of driver struct.
    // tcx.def_path_str for local items does not include the crate prefix.
    // The enrichment surface records the path as produced by def_path_str.
    assert_eq!(
        driver_struct.canonical_path.0, "driver::Pl011FullSpec",
        "driver struct canonical path must match def_path_str output"
    );

    // Check 3a: regs -- mmio resource
    let regs_field = driver_struct
        .fields
        .iter()
        .find(|f| f.name == "regs")
        .expect("regs field must exist");
    assert_eq!(regs_field.canonical_path.0, "driver::Pl011FullSpec::regs");
    assert_eq!(
        regs_field.rseq_kind,
        Some(RseqFieldKind::Mmio {
            id: "uart_regs".to_string()
        }),
        "regs must be classified as Mmio with id uart_regs"
    );

    // Check 3b: irq -- interrupt resource
    let irq_field = driver_struct
        .fields
        .iter()
        .find(|f| f.name == "irq")
        .expect("irq field must exist");
    assert_eq!(irq_field.canonical_path.0, "driver::Pl011FullSpec::irq");
    assert_eq!(
        irq_field.rseq_kind,
        Some(RseqFieldKind::Interrupt {
            id: "uart_irq".to_string()
        }),
        "irq must be classified as Interrupt with id uart_irq"
    );

    // Check 3c: clock -- clock resource
    let clock_field = driver_struct
        .fields
        .iter()
        .find(|f| f.name == "clock")
        .expect("clock field must exist");
    assert_eq!(clock_field.canonical_path.0, "driver::Pl011FullSpec::clock");
    assert_eq!(
        clock_field.rseq_kind,
        Some(RseqFieldKind::Clock {
            id: "uart_clk".to_string()
        }),
        "clock must be classified as Clock with id uart_clk"
    );

    // Check 3d: reset -- reset resource (no rseq attribute, join key is Field name)
    let reset_field = driver_struct
        .fields
        .iter()
        .find(|f| f.name == "reset")
        .expect("reset field must exist");
    assert_eq!(reset_field.canonical_path.0, "driver::Pl011FullSpec::reset");
    assert_eq!(
        reset_field.rseq_kind, None,
        "reset must have no rseq attribute (join key is Field name)"
    );

    // Check 3e: pinctrl -- pinctrl resource
    let pinctrl_field = driver_struct
        .fields
        .iter()
        .find(|f| f.name == "pinctrl")
        .expect("pinctrl field must exist");
    assert_eq!(
        pinctrl_field.canonical_path.0,
        "driver::Pl011FullSpec::pinctrl"
    );
    assert_eq!(
        pinctrl_field.rseq_kind,
        Some(RseqFieldKind::PinCtrl {
            id: "uart_pins".to_string()
        }),
        "pinctrl must be classified as PinCtrl with id uart_pins"
    );

    // -- Register access surface: mmio field details --
    let reg_access = surfaces
        .register_access
        .expect("register access extraction must succeed");
    let mmio_regs = reg_access
        .mmio_fields
        .iter()
        .find(|m| m.field_name == "regs")
        .expect("regs must appear in mmio_fields");
    assert_eq!(
        mmio_regs.mmio_id,
        Some("uart_regs".to_string()),
        "mmio_id for regs must be uart_regs"
    );
    assert_eq!(
        mmio_regs.register_block_type,
        Some("Pl011Registers".to_string()),
        "register_block_type for regs must be Pl011Registers"
    );

    // -- Interrupt surface: irq resource --
    let interrupt = surfaces
        .interrupt
        .expect("interrupt extraction must succeed");
    let irq_res = interrupt
        .resources
        .iter()
        .find(|r| r.field_name == "irq")
        .expect("irq must appear in interrupt resources");
    assert_eq!(
        irq_res.interrupt_id, "uart_irq",
        "interrupt_id for irq must be uart_irq"
    );
    assert_eq!(
        irq_res.shape,
        interrupts::InterruptFieldShape::Single,
        "irq shape must be Single (required)"
    );

    // -- Clock surface: clock field --
    let clock_surface = surfaces.clock.expect("clock extraction must succeed");
    let clock_res = clock_surface
        .fields
        .iter()
        .find(|f| f.field_name == "clock")
        .expect("clock must appear in clock fields");
    assert_eq!(clock_res.clock_id, "uart_clk", "clock_id must be uart_clk");
    assert!(
        !clock_res.required,
        "clock must be optional (Option<Clock>)"
    );

    // -- Reset surface: reset field --
    let reset_surface = surfaces.reset.expect("reset extraction must succeed");
    let reset_res = reset_surface
        .fields
        .iter()
        .find(|f| f.field_name == "reset")
        .expect("reset must appear in reset fields");
    assert!(
        !reset_res.required,
        "reset must be optional (Option<ResetLine>)"
    );

    // -- PinCtrl surface: pinctrl field --
    let pinctrl_surface = surfaces.pinctrl.expect("pinctrl extraction must succeed");
    let pinctrl_res = pinctrl_surface
        .fields
        .iter()
        .find(|f| f.field_name == "pinctrl")
        .expect("pinctrl must appear in pinctrl fields");
    assert_eq!(
        pinctrl_res.pinctrl_id,
        Some("uart_pins".to_string()),
        "pinctrl_id must be uart_pins"
    );
    assert!(
        !pinctrl_res.required,
        "pinctrl must be optional (Option<PinCtrl>)"
    );
}

// ---------------------------------------------------------------------------
// Check Group 4: Property fields
// ---------------------------------------------------------------------------

#[test]
fn check_group_4_properties() {
    let surfaces =
        compile_and_extract_all(PL011_DRIVER_STRUCT_SPECIMEN, "driver_struct_boundary_cg4");

    let prop_surface = surfaces.property.expect("property extraction must succeed");

    // Check 4a: current_speed property
    let cs = prop_surface
        .fields
        .iter()
        .find(|f| f.field_name == "current_speed")
        .expect("current_speed must appear in property fields");
    assert_eq!(cs.property_id, "current-speed", "property id must match");
    assert_eq!(cs.field_type, "u32", "current_speed type must be u32");
    assert_eq!(
        cs.default_literal,
        Some("115200".to_string()),
        "current_speed default must be 115200"
    );
    assert!(cs.required, "current_speed must be required (bare u32)");

    // Check 4b: fifo_size property
    let fs = prop_surface
        .fields
        .iter()
        .find(|f| f.field_name == "fifo_size")
        .expect("fifo_size must appear in property fields");
    assert_eq!(fs.property_id, "fifo-size", "property id must match");
    assert_eq!(fs.field_type, "u32", "fifo_size type must be u32");
    assert_eq!(
        fs.default_literal,
        Some("32".to_string()),
        "fifo_size default must be 32"
    );
    assert!(fs.required, "fifo_size must be required (bare u32)");

    // Identity enrichment cross-check for properties
    let enrichment = surfaces
        .identity_enrichment
        .expect("identity enrichment must succeed");
    let driver_struct = enrichment
        .structs
        .iter()
        .find(|s| s.is_driver)
        .expect("driver struct must exist");

    let cs_field = driver_struct
        .fields
        .iter()
        .find(|f| f.name == "current_speed")
        .expect("current_speed field in enrichment");
    assert_eq!(
        cs_field.rseq_kind,
        Some(RseqFieldKind::Property {
            id: "current-speed".to_string()
        }),
        "current_speed must be classified as Property in enrichment"
    );
    assert_eq!(
        cs_field.canonical_path.0,
        "driver::Pl011FullSpec::current_speed"
    );

    let fs_field = driver_struct
        .fields
        .iter()
        .find(|f| f.name == "fifo_size")
        .expect("fifo_size field in enrichment");
    assert_eq!(
        fs_field.rseq_kind,
        Some(RseqFieldKind::Property {
            id: "fifo-size".to_string()
        }),
        "fifo_size must be classified as Property in enrichment"
    );
    assert_eq!(
        fs_field.canonical_path.0,
        "driver::Pl011FullSpec::fifo_size"
    );
}

// ---------------------------------------------------------------------------
// Check Group 5: User field (pending_tx)
// ---------------------------------------------------------------------------

#[test]
fn check_group_5_user_field_not_reclassified() {
    let surfaces =
        compile_and_extract_all(PL011_DRIVER_STRUCT_SPECIMEN, "driver_struct_boundary_cg5");

    // Enrichment: pending_tx has no rseq classification
    let enrichment = surfaces
        .identity_enrichment
        .expect("identity enrichment must succeed");
    let driver_struct = enrichment
        .structs
        .iter()
        .find(|s| s.is_driver)
        .expect("driver struct must exist");

    let pending_tx = driver_struct
        .fields
        .iter()
        .find(|f| f.name == "pending_tx")
        .expect("pending_tx field must exist in enrichment");
    assert_eq!(
        pending_tx.rseq_kind, None,
        "pending_tx must have no rseq classification (None)"
    );
    assert_eq!(
        pending_tx.canonical_path.0,
        "driver::Pl011FullSpec::pending_tx"
    );

    // Closed-world evidence: pending_tx must NOT appear in any resource or
    // property surface.

    // Not in mmio_fields
    let reg_access = surfaces
        .register_access
        .expect("register access must succeed");
    assert!(
        !reg_access
            .mmio_fields
            .iter()
            .any(|m| m.field_name == "pending_tx"),
        "pending_tx must not appear in mmio_fields"
    );

    // Not in interrupt resources
    let interrupt = surfaces
        .interrupt
        .expect("interrupt extraction must succeed");
    assert!(
        !interrupt
            .resources
            .iter()
            .any(|r| r.field_name == "pending_tx"),
        "pending_tx must not appear in interrupt resources"
    );

    // Not in clock fields
    let clock_surface = surfaces.clock.expect("clock extraction must succeed");
    assert!(
        !clock_surface
            .fields
            .iter()
            .any(|f| f.field_name == "pending_tx"),
        "pending_tx must not appear in clock fields"
    );

    // Not in reset fields
    let reset_surface = surfaces.reset.expect("reset extraction must succeed");
    assert!(
        !reset_surface
            .fields
            .iter()
            .any(|f| f.field_name == "pending_tx"),
        "pending_tx must not appear in reset fields"
    );

    // Not in pinctrl fields
    let pinctrl_surface = surfaces.pinctrl.expect("pinctrl extraction must succeed");
    assert!(
        !pinctrl_surface
            .fields
            .iter()
            .any(|f| f.field_name == "pending_tx"),
        "pending_tx must not appear in pinctrl fields"
    );

    // Not in property fields
    let prop_surface = surfaces.property.expect("property extraction must succeed");
    assert!(
        !prop_surface
            .fields
            .iter()
            .any(|f| f.field_name == "pending_tx"),
        "pending_tx must not appear in property fields"
    );
}
