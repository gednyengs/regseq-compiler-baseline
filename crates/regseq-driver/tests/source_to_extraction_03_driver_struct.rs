//! source-to-extraction-03: PL011 driver-struct authored-to-extraction
//! boundary validation.
//!
//! Validates that the authored driver-struct declaration facts for PL011 survive
//! at the compiler-side boundary between the authored PL011 source and the
//! current Regseq extraction state.
//!
//! Extraction surfaces under test:
//!   - `extract_driver_identities(tcx) -> Vec<DriverIdentity>`
//!   - `extract_identity_enrichment(tcx) -> IdentityEnrichmentSurface`
//!     -- inspects `structs` and `modules`
//!   - `extract_register_access_surface(tcx) -> RegisterAccessSurface`
//!     -- inspects `mmio_fields`
//!   - `extract_interrupt_surface(tcx) -> InterruptSurface`
//!     -- inspects `resources`
//!   - `extract_clock_surface(tcx) -> ClockSurface`
//!     -- inspects `fields`
//!   - `extract_reset_surface(tcx) -> ResetSurface`
//!     -- inspects `fields`
//!   - `extract_pinctrl_surface(tcx) -> PinCtrlSurface`
//!     -- inspects `fields`
//!   - `extract_property_surface(tcx) -> PropertySurface`
//!     -- inspects `fields`
//!
//! Specimen: inline reduced specimen mirroring the PL011 driver struct
//! declaration. Fidelity is verified against the real fixture source at
//! `tests/fixtures/pl011-full-spec/src/driver.rs`.

#![feature(rustc_private)]
#![feature(box_patterns)]

extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::sync::{Arc, Mutex, Once};

#[path = "../src/extract/driver_identity.rs"]
mod driver_identity;

#[path = "../src/extract/identity_enrichment_types.rs"]
mod identity_enrichment_types;

#[path = "../src/extract/identity_enrichment.rs"]
mod identity_enrichment;

#[path = "../src/extract/register_access.rs"]
mod register_access;

#[path = "../src/extract/callback_context.rs"]
mod callback_context;

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

use clock::*;
use driver_identity::*;
use identity_enrichment::*;
use interrupts::*;
use pinctrl::*;
use property::*;
use register_access::*;
use reset::*;

// ---------------------------------------------------------------------------
// Authored facts from the check specification
// ---------------------------------------------------------------------------

/// Fixture source path.
const FIXTURE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/pl011-full-spec/src/driver.rs"
);

/// Read the PL011 fixture driver source.
fn read_fixture_source() -> String {
    std::fs::read_to_string(FIXTURE_PATH).expect("fixture source must be readable")
}

// --- Driver identity authored facts ---
const AUTHORED_DRIVER_NAME: &str = "pl011-full-spec";
const AUTHORED_COMPATIBLE: &[&str] = &["arm,pl011"];
const AUTHORED_BUS: &str = "amba";
const AUTHORED_DEVICE_CLASS: &str = "serial";
const AUTHORED_STRUCT_NAME: &str = "Pl011FullSpec";
const AUTHORED_STRUCT_CANONICAL_PATH: &str = "pl011_full_spec::driver::Pl011FullSpec";

// --- Resource authored facts ---
const AUTHORED_REGS_FIELD: &str = "regs";
const AUTHORED_REGS_TYPE: &str = "Mmio<Pl011Registers>";
const AUTHORED_REGS_MMIO_ID: &str = "uart_regs";

const AUTHORED_IRQ_FIELD: &str = "irq";
const AUTHORED_IRQ_TYPE: &str = "Interrupt";
const AUTHORED_IRQ_ID: &str = "uart_irq";

const AUTHORED_CLOCK_FIELD: &str = "clock";
const AUTHORED_CLOCK_TYPE: &str = "Option<Clock>";
const AUTHORED_CLOCK_ID: &str = "uart_clk";

const AUTHORED_RESET_FIELD: &str = "reset";
const AUTHORED_RESET_TYPE: &str = "Option<ResetLine>";

const AUTHORED_PINCTRL_FIELD: &str = "pinctrl";
const AUTHORED_PINCTRL_TYPE: &str = "Option<PinCtrl>";
const AUTHORED_PINCTRL_ID: &str = "uart_pins";

// --- Property authored facts ---
const AUTHORED_CURRENT_SPEED_FIELD: &str = "current_speed";
const AUTHORED_CURRENT_SPEED_TYPE: &str = "u32";
const AUTHORED_CURRENT_SPEED_ID: &str = "current-speed";
const AUTHORED_CURRENT_SPEED_DEFAULT: &str = "115200";

const AUTHORED_FIFO_SIZE_FIELD: &str = "fifo_size";
const AUTHORED_FIFO_SIZE_TYPE: &str = "u32";
const AUTHORED_FIFO_SIZE_ID: &str = "fifo-size";
const AUTHORED_FIFO_SIZE_DEFAULT: &str = "32";

// --- User field authored facts ---
const AUTHORED_PENDING_TX_FIELD: &str = "pending_tx";
const AUTHORED_PENDING_TX_TYPE: &str = "Option<u8>";

// --- Authored fixture source fragments for fidelity ---
const AUTHORED_DRIVER_ATTR: &str =
    r#"#[rseq::driver(name = "pl011-full-spec", compatible = ["arm,pl011"], bus = "amba")]"#;
const AUTHORED_DEVICE_CLASS_ATTR: &str = "#[rseq::device_class(serial)]";
const AUTHORED_STRUCT_DECL: &str = "pub struct Pl011FullSpec {";
const AUTHORED_REGS_DECL: &str = r#"#[rseq::mmio(id = "uart_regs")]"#;
const AUTHORED_REGS_FIELD_DECL: &str = "pub regs: Mmio<Pl011Registers>,";
const AUTHORED_IRQ_ATTR: &str = r#"#[rseq::interrupt(id = "uart_irq")]"#;
const AUTHORED_IRQ_FIELD_DECL: &str = "pub irq: Interrupt,";
const AUTHORED_CLOCK_ATTR: &str = r#"#[rseq::clock(id = "uart_clk")]"#;
const AUTHORED_CLOCK_FIELD_DECL: &str = "pub clock: Option<Clock>,";
const AUTHORED_RESET_FIELD_DECL: &str = "pub reset: Option<ResetLine>,";
const AUTHORED_PINCTRL_ATTR: &str = r#"#[rseq::pinctrl(id = "uart_pins")]"#;
const AUTHORED_PINCTRL_FIELD_DECL: &str = "pub pinctrl: Option<PinCtrl>,";
const AUTHORED_PROP_SPEED_ATTR: &str =
    r#"#[rseq::property(id = "current-speed", default = 115200)]"#;
const AUTHORED_PROP_SPEED_FIELD_DECL: &str = "pub current_speed: u32,";
const AUTHORED_PROP_FIFO_ATTR: &str = r#"#[rseq::property(id = "fifo-size", default = 32)]"#;
const AUTHORED_PROP_FIFO_FIELD_DECL: &str = "pub fifo_size: u32,";
const AUTHORED_PENDING_TX_DECL: &str = "pending_tx: Option<u8>,";

// ---------------------------------------------------------------------------
// Compiler harness
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
struct AllExtractionResult {
    driver_identities: Result<Vec<DriverIdentity>, DriverIdentityError>,
    identity_enrichment: Result<IdentityEnrichmentSurface, IdentityEnrichmentError>,
    register_access: Result<RegisterAccessSurface, RegisterAccessError>,
    interrupt: Result<InterruptSurface, InterruptError>,
    clock: Result<ClockSurface, ClockError>,
    reset: Result<ResetSurface, ResetError>,
    pinctrl: Result<PinCtrlSurface, PinCtrlError>,
    property: Result<PropertySurface, PropertyError>,
}

fn compile_and_extract_all(source: &str, test_name: &str) -> AllExtractionResult {
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

    type IdentityResult = Result<Vec<DriverIdentity>, DriverIdentityError>;
    type EnrichmentResult = Result<IdentityEnrichmentSurface, IdentityEnrichmentError>;
    type RegAccessResult = Result<RegisterAccessSurface, RegisterAccessError>;
    type IntResult = Result<InterruptSurface, InterruptError>;
    type ClkResult = Result<ClockSurface, ClockError>;
    type RstResult = Result<ResetSurface, ResetError>;
    type PinResult = Result<PinCtrlSurface, PinCtrlError>;
    type PropResult = Result<PropertySurface, PropertyError>;

    let identity_result: Arc<Mutex<Option<IdentityResult>>> = Arc::new(Mutex::new(None));
    let enrichment_result: Arc<Mutex<Option<EnrichmentResult>>> = Arc::new(Mutex::new(None));
    let reg_access_result: Arc<Mutex<Option<RegAccessResult>>> = Arc::new(Mutex::new(None));
    let int_result: Arc<Mutex<Option<IntResult>>> = Arc::new(Mutex::new(None));
    let clk_result: Arc<Mutex<Option<ClkResult>>> = Arc::new(Mutex::new(None));
    let rst_result: Arc<Mutex<Option<RstResult>>> = Arc::new(Mutex::new(None));
    let pin_result: Arc<Mutex<Option<PinResult>>> = Arc::new(Mutex::new(None));
    let prop_result: Arc<Mutex<Option<PropResult>>> = Arc::new(Mutex::new(None));

    struct ExtractCallbacks {
        identity_result: Arc<Mutex<Option<IdentityResult>>>,
        enrichment_result: Arc<Mutex<Option<EnrichmentResult>>>,
        reg_access_result: Arc<Mutex<Option<RegAccessResult>>>,
        int_result: Arc<Mutex<Option<IntResult>>>,
        clk_result: Arc<Mutex<Option<ClkResult>>>,
        rst_result: Arc<Mutex<Option<RstResult>>>,
        pin_result: Arc<Mutex<Option<PinResult>>>,
        prop_result: Arc<Mutex<Option<PropResult>>>,
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
            *self.identity_result.lock().unwrap() = Some(extract_driver_identities(tcx));
            *self.enrichment_result.lock().unwrap() = Some(extract_identity_enrichment(tcx));
            *self.reg_access_result.lock().unwrap() = Some(extract_register_access_surface(tcx));
            *self.int_result.lock().unwrap() = Some(extract_interrupt_surface(tcx));
            *self.clk_result.lock().unwrap() = Some(extract_clock_surface(tcx));
            *self.rst_result.lock().unwrap() = Some(extract_reset_surface(tcx));
            *self.pin_result.lock().unwrap() = Some(extract_pinctrl_surface(tcx));
            *self.prop_result.lock().unwrap() = Some(extract_property_surface(tcx));
            rustc_driver::Compilation::Stop
        }
    }

    let mut callbacks = ExtractCallbacks {
        identity_result: identity_result.clone(),
        enrichment_result: enrichment_result.clone(),
        reg_access_result: reg_access_result.clone(),
        int_result: int_result.clone(),
        clk_result: clk_result.clone(),
        rst_result: rst_result.clone(),
        pin_result: pin_result.clone(),
        prop_result: prop_result.clone(),
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

    let msg = "after_analysis should have been called";
    let result = AllExtractionResult {
        driver_identities: identity_result.lock().unwrap().take().expect(msg),
        identity_enrichment: enrichment_result.lock().unwrap().take().expect(msg),
        register_access: reg_access_result.lock().unwrap().take().expect(msg),
        interrupt: int_result.lock().unwrap().take().expect(msg),
        clock: clk_result.lock().unwrap().take().expect(msg),
        reset: rst_result.lock().unwrap().take().expect(msg),
        pinctrl: pin_result.lock().unwrap().take().expect(msg),
        property: prop_result.lock().unwrap().take().expect(msg),
    };

    std::fs::remove_dir_all(&test_dir).ok();
    result
}

// ---------------------------------------------------------------------------
// Reduced inline specimen
// ---------------------------------------------------------------------------

/// Reduced inline specimen mirroring the PL011 driver struct declaration.
///
/// The specimen preserves:
/// - `#[rseq::driver(name, compatible, bus)]` attributes
/// - `#[rseq::device_class(serial)]` attribute
/// - struct name `Pl011FullSpec`
/// - all resource fields with their `#[rseq::*]` attributes and Rust types
/// - all property fields with their `#[rseq::property(...)]` attributes
/// - user field `pending_tx: Option<u8>` without any rseq attribute
///
/// Stub types are defined to satisfy rustc type checking without importing
/// the real `regseq` crate.
const SPECIMEN: &str = r#"
pub struct Mmio<T> { _marker: core::marker::PhantomData<T> }
pub struct Pl011Registers;
pub struct Interrupt;
pub struct Clock;
pub struct ResetLine;
pub struct PinCtrl;

pub mod driver {
    use super::*;

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
// Precondition: fixture-source fidelity
// ---------------------------------------------------------------------------

#[test]
fn precondition_fixture_source_contains_authored_driver_struct() {
    let source = read_fixture_source();

    // Verify driver attribute
    assert!(
        source.contains(AUTHORED_DRIVER_ATTR),
        "fixture must contain driver attribute: {AUTHORED_DRIVER_ATTR}"
    );

    // Verify device class attribute
    assert!(
        source.contains(AUTHORED_DEVICE_CLASS_ATTR),
        "fixture must contain device class attribute: {AUTHORED_DEVICE_CLASS_ATTR}"
    );

    // Verify struct declaration
    assert!(
        source.contains(AUTHORED_STRUCT_DECL),
        "fixture must contain struct declaration: {AUTHORED_STRUCT_DECL}"
    );

    // Verify resource field declarations
    assert!(
        source.contains(AUTHORED_REGS_DECL),
        "fixture must contain mmio attribute: {AUTHORED_REGS_DECL}"
    );
    assert!(
        source.contains(AUTHORED_REGS_FIELD_DECL),
        "fixture must contain regs field: {AUTHORED_REGS_FIELD_DECL}"
    );
    assert!(
        source.contains(AUTHORED_IRQ_ATTR),
        "fixture must contain interrupt attribute: {AUTHORED_IRQ_ATTR}"
    );
    assert!(
        source.contains(AUTHORED_IRQ_FIELD_DECL),
        "fixture must contain irq field: {AUTHORED_IRQ_FIELD_DECL}"
    );
    assert!(
        source.contains(AUTHORED_CLOCK_ATTR),
        "fixture must contain clock attribute: {AUTHORED_CLOCK_ATTR}"
    );
    assert!(
        source.contains(AUTHORED_CLOCK_FIELD_DECL),
        "fixture must contain clock field: {AUTHORED_CLOCK_FIELD_DECL}"
    );
    assert!(
        source.contains(AUTHORED_RESET_FIELD_DECL),
        "fixture must contain reset field: {AUTHORED_RESET_FIELD_DECL}"
    );
    assert!(
        source.contains(AUTHORED_PINCTRL_ATTR),
        "fixture must contain pinctrl attribute: {AUTHORED_PINCTRL_ATTR}"
    );
    assert!(
        source.contains(AUTHORED_PINCTRL_FIELD_DECL),
        "fixture must contain pinctrl field: {AUTHORED_PINCTRL_FIELD_DECL}"
    );

    // Verify property declarations
    assert!(
        source.contains(AUTHORED_PROP_SPEED_ATTR),
        "fixture must contain property speed attribute: {AUTHORED_PROP_SPEED_ATTR}"
    );
    assert!(
        source.contains(AUTHORED_PROP_SPEED_FIELD_DECL),
        "fixture must contain current_speed field: {AUTHORED_PROP_SPEED_FIELD_DECL}"
    );
    assert!(
        source.contains(AUTHORED_PROP_FIFO_ATTR),
        "fixture must contain property fifo attribute: {AUTHORED_PROP_FIFO_ATTR}"
    );
    assert!(
        source.contains(AUTHORED_PROP_FIFO_FIELD_DECL),
        "fixture must contain fifo_size field: {AUTHORED_PROP_FIFO_FIELD_DECL}"
    );

    // Verify user field
    assert!(
        source.contains(AUTHORED_PENDING_TX_DECL),
        "fixture must contain pending_tx field: {AUTHORED_PENDING_TX_DECL}"
    );
}

// ---------------------------------------------------------------------------
// Precondition: specimen fidelity against fixture source
// ---------------------------------------------------------------------------

#[test]
fn precondition_specimen_fidelity_against_fixture() {
    let source = read_fixture_source();

    // Every authored fact in the specimen must also appear in the fixture source.
    let specimen_facts = &[
        AUTHORED_DRIVER_ATTR,
        AUTHORED_DEVICE_CLASS_ATTR,
        AUTHORED_STRUCT_DECL,
        AUTHORED_REGS_DECL,
        AUTHORED_REGS_FIELD_DECL,
        AUTHORED_IRQ_ATTR,
        AUTHORED_IRQ_FIELD_DECL,
        AUTHORED_CLOCK_ATTR,
        AUTHORED_CLOCK_FIELD_DECL,
        AUTHORED_RESET_FIELD_DECL,
        AUTHORED_PINCTRL_ATTR,
        AUTHORED_PINCTRL_FIELD_DECL,
        AUTHORED_PROP_SPEED_ATTR,
        AUTHORED_PROP_SPEED_FIELD_DECL,
        AUTHORED_PROP_FIFO_ATTR,
        AUTHORED_PROP_FIFO_FIELD_DECL,
        AUTHORED_PENDING_TX_DECL,
    ];

    for fact in specimen_facts {
        assert!(
            SPECIMEN.contains(fact),
            "specimen must contain authored fact: {fact}"
        );
        assert!(
            source.contains(fact),
            "fixture must contain authored fact: {fact}"
        );
    }
}

// ---------------------------------------------------------------------------
// Check Group 1: driver identity preserves name, compatible, and bus
// ---------------------------------------------------------------------------

#[test]
fn check_group_1_driver_identity_preserves_name_compatible_bus() {
    let result = compile_and_extract_all(SPECIMEN, "s2e03_cg1_identity");
    let identities = result
        .driver_identities
        .expect("driver identity extraction should succeed");

    assert_eq!(
        identities.len(),
        1,
        "exactly one driver identity expected; found {}",
        identities.len()
    );

    let id = &identities[0];

    // driver name
    assert_eq!(
        id.driver_name, AUTHORED_DRIVER_NAME,
        "driver_name must be '{}'; got '{}'",
        AUTHORED_DRIVER_NAME, id.driver_name
    );

    // compatible
    let authored_compat: Vec<String> = AUTHORED_COMPATIBLE.iter().map(|s| s.to_string()).collect();
    assert_eq!(
        id.compatible, authored_compat,
        "compatible must be {:?}; got {:?}",
        authored_compat, id.compatible
    );

    // bus
    assert_eq!(
        id.bus.as_deref(),
        Some(AUTHORED_BUS),
        "bus must be Some('{}'); got {:?}",
        AUTHORED_BUS,
        id.bus
    );
}

// ---------------------------------------------------------------------------
// Check Group 2: device class remains serial
// ---------------------------------------------------------------------------

#[test]
fn check_group_2_device_class_remains_serial() {
    let result = compile_and_extract_all(SPECIMEN, "s2e03_cg2_class");
    let identities = result
        .driver_identities
        .expect("driver identity extraction should succeed");

    assert_eq!(identities.len(), 1);
    let id = &identities[0];

    assert_eq!(
        id.device_class, AUTHORED_DEVICE_CLASS,
        "device_class must be '{}'; got '{}'",
        AUTHORED_DEVICE_CLASS, id.device_class
    );
}

// ---------------------------------------------------------------------------
// Check Group 3: resource field preservation
// ---------------------------------------------------------------------------

#[test]
fn check_group_3_regs_mmio_resource() {
    let result = compile_and_extract_all(SPECIMEN, "s2e03_cg3_regs");

    // -- Register access surface: mmio_fields --
    let reg_surface = result
        .register_access
        .expect("register access extraction should succeed");
    let mmio = reg_surface
        .mmio_fields
        .iter()
        .find(|f| f.field_name == AUTHORED_REGS_FIELD)
        .unwrap_or_else(|| {
            panic!(
                "mmio_fields must contain '{}'; found: {:?}",
                AUTHORED_REGS_FIELD,
                reg_surface
                    .mmio_fields
                    .iter()
                    .map(|f| &f.field_name)
                    .collect::<Vec<_>>()
            )
        });

    // field name
    assert_eq!(mmio.field_name, AUTHORED_REGS_FIELD);

    // mmio_id (join key via Properties.id)
    assert_eq!(
        mmio.mmio_id.as_deref(),
        Some(AUTHORED_REGS_MMIO_ID),
        "mmio_id must be Some('{}'); got {:?}",
        AUTHORED_REGS_MMIO_ID,
        mmio.mmio_id
    );

    // register_block_type: extraction captures the inner generic type name
    assert_eq!(
        mmio.register_block_type.as_deref(),
        Some("Pl011Registers"),
        "register_block_type must be Some('Pl011Registers'); got {:?}",
        mmio.register_block_type
    );

    // Rust type assertion: AUTHORED_REGS_TYPE is "Mmio<Pl011Registers>".
    // The extraction surface captures register_block_type = "Pl011Registers"
    // and presence in mmio_fields proves the wrapper type is Mmio.
    // Verify the authored type is consistent with the extraction encoding.
    assert_eq!(
        AUTHORED_REGS_TYPE,
        format!(
            "Mmio<{}>",
            mmio.register_block_type.as_deref().unwrap_or("")
        ),
        "authored regs type must match extraction's Mmio<register_block_type>"
    );

    // Requiredness is exactly reconstructable here: the extractor only records
    // fields in `mmio_fields` when the HIR field type itself is `Mmio<T>`.
    // An `Option<Mmio<T>>` field would not reach this surface because
    // `is_mmio_type()` only matches a top-level `Mmio` path.

    // -- Identity enrichment surface: field classification --
    let enrichment = result
        .identity_enrichment
        .expect("identity enrichment should succeed");
    let driver_struct = enrichment
        .structs
        .iter()
        .find(|s| s.is_driver)
        .expect("must find driver struct in enrichment");

    let regs_field = driver_struct
        .fields
        .iter()
        .find(|f| f.name == AUTHORED_REGS_FIELD)
        .expect("enrichment must contain regs field");
    assert!(
        matches!(
            &regs_field.rseq_kind,
            Some(identity_enrichment_types::RseqFieldKind::Mmio { id })
            if id == AUTHORED_REGS_MMIO_ID
        ),
        "regs field rseq_kind must be Mmio with id '{}'; got {:?}",
        AUTHORED_REGS_MMIO_ID,
        regs_field.rseq_kind
    );
}

#[test]
fn check_group_3_irq_interrupt_resource() {
    let result = compile_and_extract_all(SPECIMEN, "s2e03_cg3_irq");

    let int_surface = result
        .interrupt
        .expect("interrupt extraction should succeed");
    let irq = int_surface
        .resources
        .iter()
        .find(|r| r.field_name == AUTHORED_IRQ_FIELD)
        .unwrap_or_else(|| {
            panic!(
                "interrupt resources must contain '{}'; found: {:?}",
                AUTHORED_IRQ_FIELD,
                int_surface
                    .resources
                    .iter()
                    .map(|r| &r.field_name)
                    .collect::<Vec<_>>()
            )
        });

    // field name
    assert_eq!(irq.field_name, AUTHORED_IRQ_FIELD);

    // interrupt_id (join key via Properties.id)
    assert_eq!(
        irq.interrupt_id, AUTHORED_IRQ_ID,
        "interrupt_id must be '{}'; got '{}'",
        AUTHORED_IRQ_ID, irq.interrupt_id
    );

    // shape: single (required)
    assert_eq!(
        irq.shape,
        interrupts::InterruptFieldShape::Single,
        "irq shape must be Single (required); got {:?}",
        irq.shape
    );

    // The interrupt surface preserves a weaker structural fact here:
    // `shape == Single`. It does not expose a rendered Rust type string, so the
    // exact authored type `Interrupt` is not directly recoverable from this
    // surface alone.

    // Identity enrichment field classification
    let enrichment = result
        .identity_enrichment
        .expect("identity enrichment should succeed");
    let driver_struct = enrichment
        .structs
        .iter()
        .find(|s| s.is_driver)
        .expect("must find driver struct");
    let irq_field = driver_struct
        .fields
        .iter()
        .find(|f| f.name == AUTHORED_IRQ_FIELD)
        .expect("enrichment must contain irq field");
    assert!(
        matches!(
            &irq_field.rseq_kind,
            Some(identity_enrichment_types::RseqFieldKind::Interrupt { id })
            if id == AUTHORED_IRQ_ID
        ),
        "irq field rseq_kind must be Interrupt with id '{}'; got {:?}",
        AUTHORED_IRQ_ID,
        irq_field.rseq_kind
    );
}

#[test]
fn check_group_3_clock_resource() {
    let result = compile_and_extract_all(SPECIMEN, "s2e03_cg3_clock");

    let clk_surface = result.clock.expect("clock extraction should succeed");
    let clk = clk_surface
        .fields
        .iter()
        .find(|f| f.field_name == AUTHORED_CLOCK_FIELD)
        .unwrap_or_else(|| {
            panic!(
                "clock fields must contain '{}'; found: {:?}",
                AUTHORED_CLOCK_FIELD,
                clk_surface
                    .fields
                    .iter()
                    .map(|f| &f.field_name)
                    .collect::<Vec<_>>()
            )
        });

    // field name
    assert_eq!(clk.field_name, AUTHORED_CLOCK_FIELD);

    // clock_id (join key via Properties.id)
    assert_eq!(
        clk.clock_id, AUTHORED_CLOCK_ID,
        "clock_id must be '{}'; got '{}'",
        AUTHORED_CLOCK_ID, clk.clock_id
    );

    // required: false (Option<Clock>)
    assert!(
        !clk.required,
        "clock must be optional (required == false); got required == true"
    );

    // The clock surface preserves the weaker structural fact `required == false`.
    // It does not expose a rendered Rust type string, so the exact authored
    // type `Option<Clock>` is not directly recoverable from this surface alone.

    // Identity enrichment field classification
    let enrichment = result
        .identity_enrichment
        .expect("identity enrichment should succeed");
    let driver_struct = enrichment
        .structs
        .iter()
        .find(|s| s.is_driver)
        .expect("must find driver struct");
    let clock_field = driver_struct
        .fields
        .iter()
        .find(|f| f.name == AUTHORED_CLOCK_FIELD)
        .expect("enrichment must contain clock field");
    assert!(
        matches!(
            &clock_field.rseq_kind,
            Some(identity_enrichment_types::RseqFieldKind::Clock { id })
            if id == AUTHORED_CLOCK_ID
        ),
        "clock field rseq_kind must be Clock with id '{}'; got {:?}",
        AUTHORED_CLOCK_ID,
        clock_field.rseq_kind
    );
}

#[test]
fn check_group_3_reset_resource() {
    let result = compile_and_extract_all(SPECIMEN, "s2e03_cg3_reset");

    let rst_surface = result.reset.expect("reset extraction should succeed");
    let rst = rst_surface
        .fields
        .iter()
        .find(|f| f.field_name == AUTHORED_RESET_FIELD)
        .unwrap_or_else(|| {
            panic!(
                "reset fields must contain '{}'; found: {:?}",
                AUTHORED_RESET_FIELD,
                rst_surface
                    .fields
                    .iter()
                    .map(|f| &f.field_name)
                    .collect::<Vec<_>>()
            )
        });

    // field name
    assert_eq!(rst.field_name, AUTHORED_RESET_FIELD);

    // required: false (Option<ResetLine>)
    assert!(
        !rst.required,
        "reset must be optional (required == false); got required == true"
    );

    // The reset surface preserves the weaker structural fact `required == false`.
    // It does not expose a rendered Rust type string, so the exact authored
    // type `Option<ResetLine>` is not directly recoverable from this surface alone.

    // Reset uses field-name join key, no id attribute. Verify there is no
    // rseq classification in enrichment (reset has no #[rseq::*] attr).
    let enrichment = result
        .identity_enrichment
        .expect("identity enrichment should succeed");
    let driver_struct = enrichment
        .structs
        .iter()
        .find(|s| s.is_driver)
        .expect("must find driver struct");
    let reset_field = driver_struct
        .fields
        .iter()
        .find(|f| f.name == AUTHORED_RESET_FIELD)
        .expect("enrichment must contain reset field");
    // Reset has no rseq attribute in the authored source, so rseq_kind is None.
    // The reset surface discovers it by type, not by attribute.
    assert!(
        reset_field.rseq_kind.is_none(),
        "reset field rseq_kind must be None (no #[rseq::*] attribute); got {:?}",
        reset_field.rseq_kind
    );
}

#[test]
fn check_group_3_pinctrl_resource() {
    let result = compile_and_extract_all(SPECIMEN, "s2e03_cg3_pinctrl");

    let pin_surface = result.pinctrl.expect("pinctrl extraction should succeed");
    let pin = pin_surface
        .fields
        .iter()
        .find(|f| f.field_name == AUTHORED_PINCTRL_FIELD)
        .unwrap_or_else(|| {
            panic!(
                "pinctrl fields must contain '{}'; found: {:?}",
                AUTHORED_PINCTRL_FIELD,
                pin_surface
                    .fields
                    .iter()
                    .map(|f| &f.field_name)
                    .collect::<Vec<_>>()
            )
        });

    // field name
    assert_eq!(pin.field_name, AUTHORED_PINCTRL_FIELD);

    // pinctrl_id (join key via Properties.id)
    assert_eq!(
        pin.pinctrl_id.as_deref(),
        Some(AUTHORED_PINCTRL_ID),
        "pinctrl_id must be Some('{}'); got {:?}",
        AUTHORED_PINCTRL_ID,
        pin.pinctrl_id
    );

    // required: false (Option<PinCtrl>)
    assert!(
        !pin.required,
        "pinctrl must be optional (required == false); got required == true"
    );

    // The pinctrl surface preserves the weaker structural fact `required == false`.
    // It does not expose a rendered Rust type string, so the exact authored
    // type `Option<PinCtrl>` is not directly recoverable from this surface alone.

    // Identity enrichment field classification
    let enrichment = result
        .identity_enrichment
        .expect("identity enrichment should succeed");
    let driver_struct = enrichment
        .structs
        .iter()
        .find(|s| s.is_driver)
        .expect("must find driver struct");
    let pinctrl_field = driver_struct
        .fields
        .iter()
        .find(|f| f.name == AUTHORED_PINCTRL_FIELD)
        .expect("enrichment must contain pinctrl field");
    assert!(
        matches!(
            &pinctrl_field.rseq_kind,
            Some(identity_enrichment_types::RseqFieldKind::PinCtrl { id })
            if id == AUTHORED_PINCTRL_ID
        ),
        "pinctrl field rseq_kind must be PinCtrl with id '{}'; got {:?}",
        AUTHORED_PINCTRL_ID,
        pinctrl_field.rseq_kind
    );
}

// ---------------------------------------------------------------------------
// Check Group 3: canonical path for driver struct and resource fields
// ---------------------------------------------------------------------------

#[test]
fn check_group_3_canonical_paths() {
    let result = compile_and_extract_all(SPECIMEN, "s2e03_cg3_paths");

    let enrichment = result
        .identity_enrichment
        .expect("identity enrichment should succeed");
    let driver_struct = enrichment
        .structs
        .iter()
        .find(|s| s.is_driver)
        .expect("must find driver struct");

    // The specimen uses crate name `pl011_full_spec` (via --crate-name) and wraps
    // the struct in `mod driver`, so the full canonical path can be reconstructed
    // as `<crate_root>::<def_path_str>`. The crate root module is captured in
    // the enrichment's modules list. We reconstruct and verify the exact authored
    // canonical path `pl011_full_spec::driver::Pl011FullSpec`.
    assert_eq!(
        driver_struct.name, AUTHORED_STRUCT_NAME,
        "struct name must be '{}'; got '{}'",
        AUTHORED_STRUCT_NAME, driver_struct.name
    );

    // Find the crate root module to get the crate name for path reconstruction.
    let crate_root = enrichment
        .modules
        .iter()
        .find(|m| !m.canonical_path.0.contains("::"))
        .expect("must find crate root module in enrichment");
    let crate_name = &crate_root.canonical_path.0;

    // Reconstruct full canonical path: crate_name + "::" + def_path_str path
    let cp = &driver_struct.canonical_path.0;
    let full_canonical_path = format!("{}::{}", crate_name, cp);
    assert_eq!(
        full_canonical_path, AUTHORED_STRUCT_CANONICAL_PATH,
        "reconstructed canonical_path must be exactly '{}'; got '{}'",
        AUTHORED_STRUCT_CANONICAL_PATH, full_canonical_path
    );

    // Verify canonical paths for all resource and property fields: reconstruct
    // the full path as `crate_name::def_path_str` and match exactly against
    // `pl011_full_spec::driver::Pl011FullSpec::<field_name>`.
    let all_fields = &[
        AUTHORED_REGS_FIELD,
        AUTHORED_IRQ_FIELD,
        AUTHORED_CLOCK_FIELD,
        AUTHORED_RESET_FIELD,
        AUTHORED_PINCTRL_FIELD,
        AUTHORED_CURRENT_SPEED_FIELD,
        AUTHORED_FIFO_SIZE_FIELD,
        AUTHORED_PENDING_TX_FIELD,
    ];

    for field_name in all_fields {
        let field = driver_struct
            .fields
            .iter()
            .find(|f| f.name == *field_name)
            .unwrap_or_else(|| {
                panic!(
                    "enrichment must contain field '{}'; found: {:?}",
                    field_name,
                    driver_struct
                        .fields
                        .iter()
                        .map(|f| &f.name)
                        .collect::<Vec<_>>()
                )
            });

        let expected_path = format!("{}::{}", AUTHORED_STRUCT_CANONICAL_PATH, field_name);
        let fcp = &field.canonical_path.0;
        let full_field_path = format!("{}::{}", crate_name, fcp);
        assert_eq!(
            full_field_path, expected_path,
            "canonical_path for field '{}' must be exactly '{}'; got '{}'",
            field_name, expected_path, full_field_path
        );
    }
}

// ---------------------------------------------------------------------------
// Check Group 4: property field preservation
// ---------------------------------------------------------------------------

#[test]
fn check_group_4_current_speed_property() {
    let result = compile_and_extract_all(SPECIMEN, "s2e03_cg4_speed");

    let prop_surface = result.property.expect("property extraction should succeed");
    let speed = prop_surface
        .fields
        .iter()
        .find(|f| f.field_name == AUTHORED_CURRENT_SPEED_FIELD)
        .unwrap_or_else(|| {
            panic!(
                "property fields must contain '{}'; found: {:?}",
                AUTHORED_CURRENT_SPEED_FIELD,
                prop_surface
                    .fields
                    .iter()
                    .map(|f| &f.field_name)
                    .collect::<Vec<_>>()
            )
        });

    // field name
    assert_eq!(speed.field_name, AUTHORED_CURRENT_SPEED_FIELD);

    // property_id
    assert_eq!(
        speed.property_id, AUTHORED_CURRENT_SPEED_ID,
        "property_id must be '{}'; got '{}'",
        AUTHORED_CURRENT_SPEED_ID, speed.property_id
    );

    // field type
    assert_eq!(
        speed.field_type, AUTHORED_CURRENT_SPEED_TYPE,
        "field_type must be '{}'; got '{}'",
        AUTHORED_CURRENT_SPEED_TYPE, speed.field_type
    );

    // default
    assert_eq!(
        speed.default_literal.as_deref(),
        Some(AUTHORED_CURRENT_SPEED_DEFAULT),
        "default must be Some('{}'); got {:?}",
        AUTHORED_CURRENT_SPEED_DEFAULT,
        speed.default_literal
    );

    // required
    assert!(
        speed.required,
        "current_speed must be required (u32, not Option); got required == false"
    );

    // Identity enrichment field classification
    let enrichment = result
        .identity_enrichment
        .expect("identity enrichment should succeed");
    let driver_struct = enrichment
        .structs
        .iter()
        .find(|s| s.is_driver)
        .expect("must find driver struct");
    let speed_field = driver_struct
        .fields
        .iter()
        .find(|f| f.name == AUTHORED_CURRENT_SPEED_FIELD)
        .expect("enrichment must contain current_speed field");
    assert!(
        matches!(
            &speed_field.rseq_kind,
            Some(identity_enrichment_types::RseqFieldKind::Property { id })
            if id == AUTHORED_CURRENT_SPEED_ID
        ),
        "current_speed rseq_kind must be Property with id '{}'; got {:?}",
        AUTHORED_CURRENT_SPEED_ID,
        speed_field.rseq_kind
    );
}

#[test]
fn check_group_4_fifo_size_property() {
    let result = compile_and_extract_all(SPECIMEN, "s2e03_cg4_fifo");

    let prop_surface = result.property.expect("property extraction should succeed");
    let fifo = prop_surface
        .fields
        .iter()
        .find(|f| f.field_name == AUTHORED_FIFO_SIZE_FIELD)
        .unwrap_or_else(|| {
            panic!(
                "property fields must contain '{}'; found: {:?}",
                AUTHORED_FIFO_SIZE_FIELD,
                prop_surface
                    .fields
                    .iter()
                    .map(|f| &f.field_name)
                    .collect::<Vec<_>>()
            )
        });

    // field name
    assert_eq!(fifo.field_name, AUTHORED_FIFO_SIZE_FIELD);

    // property_id
    assert_eq!(
        fifo.property_id, AUTHORED_FIFO_SIZE_ID,
        "property_id must be '{}'; got '{}'",
        AUTHORED_FIFO_SIZE_ID, fifo.property_id
    );

    // field type
    assert_eq!(
        fifo.field_type, AUTHORED_FIFO_SIZE_TYPE,
        "field_type must be '{}'; got '{}'",
        AUTHORED_FIFO_SIZE_TYPE, fifo.field_type
    );

    // default
    assert_eq!(
        fifo.default_literal.as_deref(),
        Some(AUTHORED_FIFO_SIZE_DEFAULT),
        "default must be Some('{}'); got {:?}",
        AUTHORED_FIFO_SIZE_DEFAULT,
        fifo.default_literal
    );

    // required
    assert!(
        fifo.required,
        "fifo_size must be required (u32, not Option); got required == false"
    );

    // Identity enrichment field classification
    let enrichment = result
        .identity_enrichment
        .expect("identity enrichment should succeed");
    let driver_struct = enrichment
        .structs
        .iter()
        .find(|s| s.is_driver)
        .expect("must find driver struct");
    let fifo_field = driver_struct
        .fields
        .iter()
        .find(|f| f.name == AUTHORED_FIFO_SIZE_FIELD)
        .expect("enrichment must contain fifo_size field");
    assert!(
        matches!(
            &fifo_field.rseq_kind,
            Some(identity_enrichment_types::RseqFieldKind::Property { id })
            if id == AUTHORED_FIFO_SIZE_ID
        ),
        "fifo_size rseq_kind must be Property with id '{}'; got {:?}",
        AUTHORED_FIFO_SIZE_ID,
        fifo_field.rseq_kind
    );
}

// ---------------------------------------------------------------------------
// Check Group 5: pending_tx user field not reclassified
// ---------------------------------------------------------------------------

#[test]
fn check_group_5_pending_tx_not_reclassified() {
    let result = compile_and_extract_all(SPECIMEN, "s2e03_cg5_pending_tx");

    // Rust type assertion: verify the authored type constant is consistent
    // with the specimen declaration. The enrichment surface does not capture
    // Rust types for fields, so we verify the type constant matches the
    // specimen source.
    assert!(
        SPECIMEN.contains(&format!(
            "{}: {},",
            AUTHORED_PENDING_TX_FIELD, AUTHORED_PENDING_TX_TYPE
        )),
        "specimen must contain '{}: {},' confirming authored type",
        AUTHORED_PENDING_TX_FIELD,
        AUTHORED_PENDING_TX_TYPE
    );

    // -- Identity enrichment: pending_tx must have rseq_kind == None --
    let enrichment = result
        .identity_enrichment
        .expect("identity enrichment should succeed");
    let driver_struct = enrichment
        .structs
        .iter()
        .find(|s| s.is_driver)
        .expect("must find driver struct");
    let ptx_field = driver_struct
        .fields
        .iter()
        .find(|f| f.name == AUTHORED_PENDING_TX_FIELD)
        .expect("enrichment must contain pending_tx field");
    assert!(
        ptx_field.rseq_kind.is_none(),
        "pending_tx rseq_kind must be None (not reclassified); got {:?}",
        ptx_field.rseq_kind
    );

    // -- Closed-world: pending_tx must NOT appear in any resource surface --

    // mmio
    let reg_surface = result
        .register_access
        .expect("register access extraction should succeed");
    assert!(
        !reg_surface
            .mmio_fields
            .iter()
            .any(|f| f.field_name == AUTHORED_PENDING_TX_FIELD),
        "pending_tx must not appear in mmio_fields"
    );

    // interrupt
    let int_surface = result
        .interrupt
        .expect("interrupt extraction should succeed");
    assert!(
        !int_surface
            .resources
            .iter()
            .any(|r| r.field_name == AUTHORED_PENDING_TX_FIELD),
        "pending_tx must not appear in interrupt resources"
    );

    // clock
    let clk_surface = result.clock.expect("clock extraction should succeed");
    assert!(
        !clk_surface
            .fields
            .iter()
            .any(|f| f.field_name == AUTHORED_PENDING_TX_FIELD),
        "pending_tx must not appear in clock fields"
    );

    // reset
    let rst_surface = result.reset.expect("reset extraction should succeed");
    assert!(
        !rst_surface
            .fields
            .iter()
            .any(|f| f.field_name == AUTHORED_PENDING_TX_FIELD),
        "pending_tx must not appear in reset fields"
    );

    // pinctrl
    let pin_surface = result.pinctrl.expect("pinctrl extraction should succeed");
    assert!(
        !pin_surface
            .fields
            .iter()
            .any(|f| f.field_name == AUTHORED_PENDING_TX_FIELD),
        "pending_tx must not appear in pinctrl fields"
    );

    // property
    let prop_surface = result.property.expect("property extraction should succeed");
    assert!(
        !prop_surface
            .fields
            .iter()
            .any(|f| f.field_name == AUTHORED_PENDING_TX_FIELD),
        "pending_tx must not appear in property fields"
    );
}
