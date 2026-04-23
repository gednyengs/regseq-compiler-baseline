//! Construction Pass 1 specimens (regseq-model-41).
//!
//! Each test compiles a small inline specimen through a rustc harness,
//! invokes `build_declaration_skeleton`, and asserts the structure of
//! the returned [`RegseqModel`]. Assertions are purely structural -- no
//! JSON serialization (task 54 owns that).

#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

use std::sync::{Arc, Mutex, Once};

use regseq_driver::construction::{
    build_declaration_skeleton, ConstructionError, ExtractionRegistries, PinStateRegistry,
};
use regseq_driver::extract::abstraction_types::AbstractionTypeRegistry;
use regseq_driver::extract::framework_types::FrameworkTypeRegistry;
use regseq_driver::extract::intrinsic_registry::IntrinsicRegistry;
use regseq_model::identity::{SourceOrigin, TypeDefId};
use regseq_model::ir::{Mutability, RegseqType};
use regseq_model::model::{AttrValue, ModuleKind, RegseqModel, TypeDefClassification, TypeDefKind};

type ConstructionOutcome = Result<RegseqModel, ConstructionError>;

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

fn compile_and_build(source: &str, test_name: &str) -> ConstructionOutcome {
    let _lock = COMPILER_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
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

    let result: Arc<Mutex<Option<ConstructionOutcome>>> = Arc::new(Mutex::new(None));

    struct Callbacks {
        result: Arc<Mutex<Option<ConstructionOutcome>>>,
    }

    impl rustc_driver::Callbacks for Callbacks {
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
            config.opts.unstable_opts.crate_attr.push(
                "allow(unused, non_snake_case, nonstandard_style, redundant_semicolons)"
                    .to_string(),
            );
        }

        fn after_analysis<'tcx>(
            &mut self,
            _compiler: &rustc_interface::interface::Compiler,
            tcx: rustc_middle::ty::TyCtxt<'tcx>,
        ) -> rustc_driver::Compilation {
            let intrinsic = IntrinsicRegistry::build(tcx);
            let framework = FrameworkTypeRegistry::build(tcx);
            let abstraction = AbstractionTypeRegistry::build(tcx);
            let pin_states = PinStateRegistry::build(tcx);
            let registries = ExtractionRegistries {
                intrinsic: &intrinsic,
                framework_types: &framework,
                abstraction_types: &abstraction,
                pin_states: &pin_states,
            };
            let outcome = build_declaration_skeleton(tcx, &registries);
            *self.result.lock().unwrap() = Some(outcome);
            rustc_driver::Compilation::Stop
        }
    }

    let mut callbacks = Callbacks {
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
        "specimen".to_string(),
        "--out-dir".to_string(),
        test_dir.to_str().unwrap().to_string(),
    ];

    rustc_driver::run_compiler(&args, &mut callbacks);

    let outcome = result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis must run");

    std::fs::remove_dir_all(&test_dir).ok();

    outcome
}

// ---------------------------------------------------------------------------
// Positive specimen: minimal driver exercising every category
// ---------------------------------------------------------------------------

const POSITIVE_SOURCE: &str = r#"
#![no_std]

// --- Framework facade stubs (no real `regseq` crate here) ---
pub struct Clock;
pub struct ResetLine;
pub struct PinCtrl;
pub struct Interrupt;
pub struct Mmio<T> { _marker: core::marker::PhantomData<T> }
pub struct Mutex<T> { _v: T }
pub struct Completion;
pub enum IrqResult { Handled, NotHandled }

pub struct ReadWrite<T, R> { _v: T, _r: core::marker::PhantomData<R> }
pub struct Reserved<T> { _v: T }

// --- Register descriptor module (emulates register_bitfields! expansion) ---
#[allow(non_snake_case)]
pub mod DR_REG {
    pub struct Register;
}

// --- Register block ---
#[repr(C)]
#[rseq::register_block(endian = "little", access_width = 32)]
pub struct MyBlock {
    pub dr: ReadWrite<u32, DR_REG::Register>,
}

// --- Pin state marker ---
#[rseq::pin_state(id = "default")]
pub struct DefaultState;

// --- Top-level constant ---
pub const BAUD_RATE: u32 = 115200;

// --- Free helper function ---
pub fn double(x: u32) -> u32 { x * 2 }

// --- User-defined Copy type for user_data inner field ---
#[derive(Copy, Clone)]
pub struct Stats {
    pub count: u32,
    pub last: u32,
}

// --- Driver struct ---
#[rseq::driver(name = "specimen-drv", compatible = ["acme,widget"], bus = "amba")]
pub struct MyDriver {
    #[rseq::clock(id = "core_clk")]
    pub clk: Clock,
    #[rseq::clock(id = "bus_clks")]
    pub bus_clks: [Clock; 4],
    #[rseq::reset(id = "core_rst")]
    pub rst: Option<ResetLine>,
    #[rseq::pinctrl(id = "pinmux")]
    pub pins: PinCtrl,
    #[rseq::interrupt(id = "core_irq")]
    pub irq: Interrupt,
    #[rseq::mmio(id = "regs")]
    pub regs: Mmio<MyBlock>,
    pub lock: Mutex<u32>,
    #[rseq::property(id = "sample-rate", default = 48000, vendor_prefix = "acme")]
    pub sample_rate: u32,
    pub pending: Option<u8>,
    pub stats: Option<Stats>,
}

// --- Trait impl ---
pub trait Lifecycle { fn init(&mut self); }

impl Lifecycle for MyDriver {
    fn init(&mut self) {}
}

// --- Driver impl block with helper + ISR ---
impl MyDriver {
    pub fn helper(&self) {}

    #[rseq::isr]
    pub fn on_irq(&mut self) -> IrqResult { IrqResult::Handled }
}
"#;

#[test]
fn positive_specimen_constructs_declaration_skeleton() {
    let model = compile_and_build(POSITIVE_SOURCE, "construction_pass1_positive")
        .expect("positive specimen must succeed");

    // Exactly one Driver module.
    let driver_count = count_driver_modules(&model);
    assert_eq!(driver_count, 1, "exactly one Driver module must exist");

    // The driver struct lives on one of the modules.
    let driver_module = find_driver_module(&model).expect("driver module must exist");
    let driver = driver_module
        .driver
        .as_ref()
        .expect("driver module must carry a DriverStruct");

    // Each field category populated (specimen has two clocks: one
    // scalar and one [Clock; 4] array; two user-data fields: pending and
    // stats).
    assert_eq!(driver.clocks.len(), 2, "two clock resources");
    assert_eq!(driver.resets.len(), 1, "one reset resource");
    assert_eq!(driver.pinctrl.len(), 1, "one pinctrl resource");
    assert_eq!(driver.interrupts.len(), 1, "one interrupt resource");
    assert_eq!(driver.mmio.len(), 1, "one mmio resource");
    assert_eq!(driver.sync.len(), 1, "one sync primitive");
    assert_eq!(driver.properties.len(), 1, "one property");
    assert_eq!(driver.user_data.len(), 2, "two user-data fields");

    // All binding slots None.
    assert!(driver.clocks[0].binding.is_none());
    assert!(driver.resets[0].binding.is_none());
    assert!(driver.pinctrl[0].binding.is_none());
    assert!(driver.interrupts[0].binding.is_none());
    assert!(driver.mmio[0].binding.is_none());
    assert!(driver.properties[0].bound_value.is_none());

    // Array shapes come from middle-ty: `bus_clks: [Clock; 4]` must be
    // `Shape::Array(4)`, not `Shape::Array(0)`. Before 41d this defaulted
    // to 0 since HIR-based shape lookup did not evaluate the const.
    let bus_clks = driver
        .clocks
        .iter()
        .find(|c| model.symbols.resolve(c.field_name) == "bus_clks")
        .expect("bus_clks clock resource");
    assert_eq!(
        bus_clks.shape,
        regseq_model::model::Shape::Array(4),
        "Array shape must be derived from the const length, not hardcoded to 0"
    );

    // Sync payloads: Mutex<u32> resolves to Scalar(U32), not a hardcoded
    // placeholder (before 41d this was always U32 by accident, not by
    // middle-ty resolution).
    let lock = &driver.sync[0];
    assert_eq!(
        lock.payload_type,
        Some(RegseqType::Scalar(regseq_model::ir::ScalarType::U32)),
        "Mutex<u32> payload must resolve to Scalar(U32) via middle-ty"
    );

    // Property vendor_prefix: `#[rseq::property(..., vendor_prefix = "acme")]`
    // must surface as `Some(Symbol::"acme")` (this field was never parsed
    // before 41d).
    let prop = &driver.properties[0];
    let vp = prop
        .vendor_prefix
        .expect("vendor_prefix must be captured from property attr");
    assert_eq!(model.symbols.resolve(vp), "acme");

    // `Copy` enforcement is now a real trait query. The specimen's
    // `stats: Option<Stats>` field uses a `#[derive(Copy, Clone)]` struct,
    // which would fail the pre-41d name-whitelist but passes
    // `type_is_copy_modulo_regions`.
    let stats_ud = driver
        .user_data
        .iter()
        .find(|u| model.symbols.resolve(u.field_name) == "stats")
        .expect("stats user_data field");
    match &stats_ud.kind {
        regseq_model::model::UserDataKind::Option { inner_type } => {
            assert!(
                matches!(inner_type, RegseqType::Defined(_)),
                "Stats Option inner_type must resolve to Defined(TypeDefId), got {:?}",
                inner_type
            );
        }
        other => panic!("stats user-data must be Option<Stats>, got {other:?}"),
    }

    // Constants populated.
    assert!(
        count_constants(&model) >= 1,
        "at least one top-level constant must appear"
    );

    // Register structures present.
    assert!(
        count_register_blocks(&model) >= 1,
        "at least one register block"
    );
    assert!(
        count_register_descriptors(&model) >= 1,
        "at least one register descriptor"
    );

    // MMIO resource resolves to the correct RegisterBlockId (the
    // specimen has exactly one block, so the MMIO decl must point at
    // it, not a placeholder id).
    let mmio_block_id = driver.mmio[0].decl.register_block;
    let expected_block_id = collect_register_block_ids(&model)[0];
    assert_eq!(
        mmio_block_id, expected_block_id,
        "MMIO register_block must resolve to the actual RegisterBlockId",
    );

    // source_locations bijection: every allocated DeclId appears as a
    // key and no extras. We approximate "every DeclId" by collecting
    // every id we can reach through the model.
    let reachable = collect_reachable_decl_ids(&model);
    for id in &reachable {
        assert!(
            model.source_locations.contains_key(id),
            "every reachable DeclId must have a source_locations entry: {id:?}"
        );
    }

    // TypeDef.attrs populated for annotated types: the driver struct
    // and the pin-state marker both carry `#[rseq::...]` attributes
    // (the register-block struct is tracked under `register_blocks`,
    // not `type_defs`, so it does not count here).
    let annotated = collect_classifications_with_attrs(&model)
        .into_iter()
        .filter(|(_, attrs_nonempty)| *attrs_nonempty)
        .count();
    assert!(
        annotated >= 2,
        "at least two annotated types should have non-empty Attrs, got {annotated}"
    );

    // Function placeholder bodies (Unreachable terminator, arg_count set
    // from the signature).
    let functions = collect_functions(&model);
    assert!(!functions.is_empty(), "at least one function present");
    for f in &functions {
        assert_eq!(
            f.body.basic_blocks.len(),
            1,
            "Pass 1 placeholder body has exactly one block"
        );
    }

    // Source locations: every assigned DeclId must be `Source(..)` in
    // Pass 1 -- the spec forbids `CompilerGenerated` for Pass 1 outputs
    // (obligation 8).
    let compiler_generated: Vec<_> = model
        .source_locations
        .iter()
        .filter(|(_, o)| matches!(o, SourceOrigin::CompilerGenerated))
        .map(|(id, _)| *id)
        .collect();
    assert!(
        compiler_generated.is_empty(),
        "Pass 1 must not emit CompilerGenerated source origins; found {} such entries: {:?}",
        compiler_generated.len(),
        compiler_generated,
    );

    // Root module type defs include the register block struct + driver
    // struct + helpers; classification should mark no Abstraction or
    // Framework types (the inline facade does not set those attrs).
    let classifications = collect_classifications(&model);
    assert!(
        classifications
            .iter()
            .any(|c| matches!(c, TypeDefClassification::UserDefined)),
        "at least one UserDefined type should exist"
    );

    // Attribute payloads must survive Pass 1 -- task 52 (PinCtrl metadata)
    // and task 43 (ISR lowering) read these structured values back out of
    // the Attrs bag. Before task 41a, every payload collapsed to Unit.
    let default_state_attr = find_type_def_attr(&model, "DefaultState", "rseq::pin_state")
        .expect("DefaultState must carry rseq::pin_state attr");
    assert_eq!(
        keyval_lookup(default_state_attr, "id"),
        Some(&AttrValue::String("default".to_owned())),
        "pin_state id payload must survive as String(\"default\")"
    );

    let driver_type_attr = find_type_def_attr(&model, "MyDriver", "rseq::driver")
        .expect("MyDriver must carry rseq::driver attr");
    assert_eq!(
        keyval_lookup(driver_type_attr, "name"),
        Some(&AttrValue::String("specimen-drv".to_owned())),
        "driver name keyword arg must survive in Attrs bag"
    );
    assert_eq!(
        keyval_lookup(driver_type_attr, "bus"),
        Some(&AttrValue::String("amba".to_owned())),
        "driver bus keyword arg must survive even when a sibling list arg is unparseable"
    );

    let isr_fn = functions
        .iter()
        .find(|f| model.symbols.resolve(f.name) == "on_irq")
        .expect("ISR method on_irq must be present");
    assert!(
        isr_fn.attrs.entries.iter().any(|e| e.key == "rseq::isr"),
        "ISR method must retain rseq::isr attr entry"
    );

    // Locally-defined ADT fields must resolve through the DefId->TypeDefId
    // map and surface as `RegseqType::Defined(TypeDefId)`, not `Unit`. The
    // specimen's `regs: Mmio<MyBlock>` field on the driver struct points to
    // the inline `Mmio<T>` type definition (a local ADT) -- before 41b this
    // collapsed to `Unit` and Pass 2 had no way to walk field types.
    let mmio_tid =
        find_type_def_id(&model, "Mmio").expect("Mmio type definition must exist in the specimen");
    let my_driver_tid =
        find_type_def_id(&model, "MyDriver").expect("MyDriver type definition must exist");
    let my_driver_fields =
        type_def_fields(&model, my_driver_tid).expect("MyDriver must have struct fields");
    let regs_field_ty = my_driver_fields
        .iter()
        .find(|(name, _)| model.symbols.resolve(*name) == "regs")
        .map(|(_, ty)| ty.clone())
        .expect("MyDriver must have a `regs` field");
    assert_eq!(
        regs_field_ty,
        RegseqType::Defined(mmio_tid),
        "regs: Mmio<MyBlock> must resolve to Defined(Mmio TypeDefId)"
    );

    // Register block fields must resolve their descriptor references via
    // `descriptor_map`. Before 41c, `descriptor` was always `None`.
    let my_block =
        find_register_block(&model, "MyBlock").expect("MyBlock register block must exist");
    let dr_field = my_block
        .fields
        .iter()
        .find(|f| model.symbols.resolve(f.name) == "dr")
        .expect("MyBlock.dr field must exist");
    assert!(
        dr_field.descriptor.is_some(),
        "dr field must resolve to a RegisterDescriptorId (was None before 41c)"
    );
    let dr_reg = find_register_descriptor(&model, "DR_REG")
        .expect("DR_REG descriptor must be synthesized from its submodule");
    assert_eq!(
        dr_field.descriptor,
        Some(dr_reg.id),
        "dr field's descriptor ID must match the DR_REG descriptor"
    );

    // Method receivers must resolve to `Ref(Defined(self_tid), Shared)`,
    // not `Unit`. Before 41b, `&self` parameter types collapsed to Unit.
    let helper = functions
        .iter()
        .find(|f| model.symbols.resolve(f.name) == "helper")
        .expect("helper method must be present");
    let recv_ty = &helper.params[0].ty;
    match recv_ty {
        RegseqType::Ref(inner, Mutability::Shared) => {
            assert_eq!(
                **inner,
                RegseqType::Defined(my_driver_tid),
                "helper `&self` receiver must be &MyDriver (Defined({my_driver_tid:?}))",
            );
        }
        other => panic!("helper receiver must be Ref(_, Shared), got {other:?}"),
    }
}

#[test]
fn resource_surfaces_populate_driver_vectors() {
    const SOURCE: &str = r#"
#![no_std]

pub struct Clock;
pub struct ResetLine;
pub struct PinCtrl;
pub struct Interrupt;
pub struct Mmio<T> { _marker: core::marker::PhantomData<T> }
pub struct ReadWrite<T, R> { _v: T, _r: core::marker::PhantomData<R> }
pub enum IrqResult { Handled, NotHandled }

#[allow(non_snake_case)]
pub mod DR_REG {
    pub struct Register;
}

#[repr(C)]
#[rseq::register_block(endian = "little", access_width = 32)]
pub struct MyBlock {
    pub dr: ReadWrite<u32, DR_REG::Register>,
}

#[rseq::driver(name = "resource-drv", compatible = ["acme,fixture"])]
pub struct ResourceDriver {
    #[rseq::clock(id = "core_clk")]
    pub clk: Clock,
    #[rseq::reset(id = "core_rst")]
    pub rst: Option<ResetLine>,
    #[rseq::property(id = "sample-rate", vendor_prefix = "acme")]
    pub sample_rate: u32,
    #[rseq::interrupt(id = "core_irq")]
    pub irq: Interrupt,
    #[rseq::mmio(id = "regs")]
    pub regs: Mmio<MyBlock>,
}

impl ResourceDriver {
    #[rseq::isr]
    pub fn on_irq(&mut self) -> IrqResult { IrqResult::Handled }
}
"#;

    let model = compile_and_build(SOURCE, "construction_pass1_resources")
        .expect("resource specimen must succeed");
    let driver = find_driver_module(&model)
        .and_then(|module| module.driver.as_ref())
        .expect("driver struct present");

    assert_eq!(driver.clocks.len(), 1, "clock surface must populate clocks");
    assert_eq!(driver.resets.len(), 1, "reset surface must populate resets");
    assert_eq!(
        driver.properties.len(),
        1,
        "property surface must populate properties"
    );
    assert_eq!(
        model.symbols.resolve(driver.clocks[0].field_name),
        "clk",
        "clock field name must come from the extraction surface"
    );
    assert_eq!(
        model.symbols.resolve(driver.resets[0].field_name),
        "rst",
        "reset field name must come from the extraction surface"
    );
    assert_eq!(
        model.symbols.resolve(driver.properties[0].field_name),
        "sample_rate",
        "property field name must come from the extraction surface"
    );
}

// ---------------------------------------------------------------------------
// Register descriptor extraction (task 41c)
// ---------------------------------------------------------------------------

/// The existing positive specimen only sketches `pub mod DR_REG { pub struct
/// Register; }` -- the parser returns a descriptor with zero fields. This
/// test enriches a descriptor module to mimic the `register_bitfields!`
/// expansion (field sub-modules each with a `FIELD` const plus named
/// values) so 41c's field extraction + reserved-array-count evaluation
/// is exercised end to end.
#[test]
fn descriptor_extraction_populates_fields_and_named_values() {
    const SRC: &str = r#"
#![no_std]

pub struct FieldDef<T> { _m: core::marker::PhantomData<T> }
impl<T> FieldDef<T> {
    pub const fn new(_mask: u32, _shift: u32) -> Self { Self { _m: core::marker::PhantomData } }
}
pub struct FieldValue<T> { _m: core::marker::PhantomData<T> }
impl<T> FieldValue<T> {
    pub const fn new(_mask: u32, _value: u32) -> Self { Self { _m: core::marker::PhantomData } }
}

pub struct ReadWrite<T, R> { _v: T, _r: core::marker::PhantomData<R> }
pub struct Reserved<T> { _v: T }
pub struct Mmio<T> { _m: core::marker::PhantomData<T> }

#[allow(non_snake_case)]
pub mod CTRL_REG {
    use super::{FieldDef, FieldValue};
    pub struct Register;

    #[allow(non_snake_case)]
    pub mod MODE {
        use super::{FieldDef, FieldValue};
        pub const FIELD: FieldDef<u32> = FieldDef::new(0x3, 0);
        pub const DISABLED: FieldValue<u32> = FieldValue::new(0x3, 0);
        pub const ENABLED: FieldValue<u32> = FieldValue::new(0x3, 1);
    }

    #[allow(non_snake_case)]
    pub mod IE {
        use super::FieldDef;
        pub const FIELD: FieldDef<u32> = FieldDef::new(0x10, 4);
    }
}

#[repr(C)]
#[rseq::register_block(endian = "little", access_width = 32)]
pub struct Block {
    pub ctrl: ReadWrite<u32, CTRL_REG::Register>,
    pub _gap: [Reserved<u32>; 3],
    pub also_ctrl: ReadWrite<u32, CTRL_REG::Register>,
}

#[rseq::driver(name = "desc-drv", compatible = ["acme,desc"], bus = "amba")]
pub struct DescDriver {
    #[rseq::mmio(id = "regs")]
    pub regs: Mmio<Block>,
}
"#;
    let model =
        compile_and_build(SRC, "construction_pass1_descriptors").expect("specimen must build");

    let ctrl_reg = find_register_descriptor(&model, "CTRL_REG")
        .expect("CTRL_REG descriptor must be synthesized");
    assert_eq!(
        ctrl_reg.fields.len(),
        2,
        "CTRL_REG must surface both MODE and IE fields (got {:?})",
        ctrl_reg.fields,
    );

    let mode = ctrl_reg
        .fields
        .iter()
        .find(|f| model.symbols.resolve(f.name) == "MODE")
        .expect("MODE field");
    assert_eq!(mode.offset, 0, "MODE field offset is 0");
    assert_eq!(mode.width, 2, "MODE field width is 2 (mask 0x3 has 2 bits)");
    assert_eq!(
        mode.named_values.len(),
        2,
        "MODE should expose DISABLED + ENABLED named values"
    );
    let enabled = mode
        .named_values
        .iter()
        .find(|nv| model.symbols.resolve(nv.name) == "ENABLED")
        .expect("ENABLED named value");
    assert_eq!(enabled.value, 1, "ENABLED value resolves to 1 (un-shifted)");

    let ie = ctrl_reg
        .fields
        .iter()
        .find(|f| model.symbols.resolve(f.name) == "IE")
        .expect("IE field");
    assert_eq!(ie.offset, 4, "IE field offset");
    assert_eq!(ie.width, 1, "IE field width (mask 0x10 has 1 bit)");

    // Access width is derived from the maximum touched bit (offset 4 +
    // width 1 = 5 -> rounded to 8).
    assert_eq!(
        ctrl_reg.access_width, 8,
        "access_width derived from highest used bit, not hardcoded to 32"
    );

    // Register block fields resolve the same descriptor for both entries.
    let block = find_register_block(&model, "Block").expect("Block must exist");
    let ctrl_fields: Vec<_> = block
        .fields
        .iter()
        .filter(|f| matches!(model.symbols.resolve(f.name), "ctrl" | "also_ctrl"))
        .collect();
    assert_eq!(ctrl_fields.len(), 2);
    for f in &ctrl_fields {
        assert_eq!(
            f.descriptor,
            Some(ctrl_reg.id),
            "both CTRL-typed fields resolve to the same descriptor ID"
        );
    }

    // Reserved array count must be evaluated, not defaulted to 1.
    let gap = block
        .fields
        .iter()
        .find(|f| model.symbols.resolve(f.name) == "_gap")
        .expect("_gap field exists");
    assert_eq!(
        gap.array_count,
        Some(3),
        "[Reserved<u32>; 3] must produce array_count = 3 (was defaulting to 1)"
    );
}

// ---------------------------------------------------------------------------
// MMIO source-order independence
// ---------------------------------------------------------------------------

/// Regression test for 41b's pre-pass ordering: if the driver struct
/// appears in source before its register block, MMIO resolution must still
/// produce a real `RegisterBlockId`, not a `CompilerGenerated` placeholder.
#[test]
fn mmio_resolves_regardless_of_source_order() {
    const SRC: &str = r#"
#![no_std]
pub struct Mmio<T> { _m: core::marker::PhantomData<T> }
pub struct ReadWrite<T, R> { _v: T, _r: core::marker::PhantomData<R> }

#[allow(non_snake_case)]
pub mod STATUS_REG {
    pub struct Register;
}

#[rseq::driver(name = "order-drv", compatible = ["acme,ordered"], bus = "amba")]
pub struct DriverFirst {
    #[rseq::mmio(id = "regs")]
    pub regs: Mmio<BlockLater>,
}

#[repr(C)]
#[rseq::register_block(endian = "little", access_width = 32)]
pub struct BlockLater {
    pub status: ReadWrite<u32, STATUS_REG::Register>,
}
"#;
    let model = compile_and_build(SRC, "mmio_order_driver_first").expect("specimen must build");
    let driver = find_driver_module(&model)
        .expect("driver module exists")
        .driver
        .as_ref()
        .expect("driver struct exists");
    let mmio_block_id = driver.mmio[0].decl.register_block;
    let blocks = collect_register_block_ids(&model);
    assert_eq!(
        blocks.len(),
        1,
        "exactly one register block in the specimen"
    );
    assert_eq!(
        mmio_block_id, blocks[0],
        "MMIO register_block must resolve to the actual RegisterBlockId even when the driver is declared first"
    );
    assert!(
        matches!(
            model.source_locations.get(&mmio_block_id.decl_id()),
            Some(SourceOrigin::Source(_))
        ),
        "MMIO-referenced block must have a Source origin, not CompilerGenerated"
    );
}

// ---------------------------------------------------------------------------
// Negative specimens
// ---------------------------------------------------------------------------

#[test]
fn negative_no_driver_struct() {
    const SRC: &str = r#"
#![no_std]
pub struct NothingHere;
pub const X: u32 = 1;
"#;
    let outcome = compile_and_build(SRC, "construction_pass1_no_driver");
    assert!(matches!(
        outcome,
        Err(ConstructionError::NoDriverStruct { .. })
    ));
}

#[test]
fn negative_unknown_rseq_attribute() {
    // A typo'd or out-of-set `#[rseq::<name>]` on a Pass 1 item must be
    // rejected as `UnknownRseqAttribute`. Previously these were silently
    // preserved in the Attrs bag (the audit flagged this variant as
    // dead code).
    const SRC: &str = r#"
#![no_std]
#[rseq::driver(name = "typo", compatible = ["x"], bus = "amba")]
pub struct Drv {}

#[rseq::typoed_name]
pub struct Typo {}
"#;
    let outcome = compile_and_build(SRC, "construction_pass1_unknown_attr");
    match outcome {
        Err(ConstructionError::UnknownRseqAttribute { attr, .. }) => {
            assert_eq!(attr, "rseq::typoed_name", "unrecognized attr reported");
        }
        other => panic!("expected UnknownRseqAttribute, got {:?}", other),
    }
}

#[test]
fn negative_duplicate_item_name() {
    // A named-field struct and a const may share a name (they live in
    // separate namespaces at the language level), but Pass 1 tracks items
    // by identifier string to guarantee an unambiguous path for
    // downstream lookups.
    const SRC: &str = r#"
#![no_std]
#[rseq::driver(name = "dup", compatible = ["x"], bus = "amba")]
pub struct Dup {}

pub const Dup: u32 = 0;
"#;
    let outcome = compile_and_build(SRC, "construction_pass1_duplicate_name");
    match outcome {
        Err(ConstructionError::DuplicateItemName { name, .. }) => {
            assert_eq!(name, "Dup", "duplicate name reported");
        }
        other => panic!("expected DuplicateItemName, got {:?}", other),
    }
}

#[test]
fn negative_multiple_driver_structs() {
    const SRC: &str = r#"
#![no_std]
#[rseq::driver(name = "a", compatible = ["x"], bus = "amba")]
pub struct A;
#[rseq::driver(name = "b", compatible = ["y"], bus = "amba")]
pub struct B;
"#;
    let outcome = compile_and_build(SRC, "construction_pass1_multiple_drivers");
    match outcome {
        Err(ConstructionError::MultipleDriverStructs { drivers }) => {
            assert_eq!(drivers.len(), 2, "provenance for both driver structs");
        }
        other => panic!("expected MultipleDriverStructs, got {:?}", other),
    }
}

#[test]
fn negative_user_data_not_copy() {
    const SRC: &str = r#"
#![no_std]
pub struct NotCopy { pub v: u32 }

#[rseq::driver(name = "d", compatible = ["x"], bus = "amba")]
pub struct D {
    pub bad: Option<NotCopy>,
}
"#;
    let outcome = compile_and_build(SRC, "construction_pass1_user_data_not_copy");
    assert!(matches!(
        outcome,
        Err(ConstructionError::UserDataNotCopy { .. })
    ));
}

// ---------------------------------------------------------------------------
// Path A (regseq-model-64): Rule-1 zero-init whitelist acceptance tests
// ---------------------------------------------------------------------------

/// Helper: build a one-field driver specimen with the given field type
/// and field name, then compile it through the construction harness.
fn compile_user_data_specimen(
    field_name: &str,
    field_type: &str,
    test_name: &str,
) -> ConstructionOutcome {
    let src = format!(
        r#"
#![no_std]
#[rseq::driver(name = "d", compatible = ["x"], bus = "amba")]
pub struct D {{
    pub {field_name}: {field_type},
}}
"#,
    );
    compile_and_build(&src, test_name)
}

fn assert_accepted_zero_init(
    outcome: ConstructionOutcome,
    field_name: &str,
    expected: &RegseqType,
) {
    let model = outcome.expect("Rule-1 whitelist shape must be accepted");
    let driver = find_driver_module(&model)
        .and_then(|m| m.driver.as_ref())
        .expect("driver struct");
    let field = driver
        .user_data
        .iter()
        .find(|u| model.symbols.resolve(u.field_name) == field_name)
        .expect("user-data field present");
    match &field.kind {
        regseq_model::model::UserDataKind::ZeroInit { inner_type } => {
            assert_eq!(inner_type, expected, "inner_type mismatch");
        }
        other => panic!("expected ZeroInit, got {other:?}"),
    }
}

fn assert_rule1_whitelist_rejected(outcome: ConstructionOutcome) {
    match outcome {
        Err(ConstructionError::UserDataNotZeroInitWhitelisted { .. }) => {}
        other => panic!("expected UserDataNotZeroInitWhitelisted, got {other:?}"),
    }
}

fn assert_user_data_not_copy(outcome: ConstructionOutcome) {
    match outcome {
        Err(ConstructionError::UserDataNotCopy { .. }) => {}
        other => panic!("expected UserDataNotCopy, got {other:?}"),
    }
}

// ----- Positive: scalar arm ------------------------------------------

#[test]
fn rule1_scalar_u8_accepted() {
    let outcome = compile_user_data_specimen("s", "u8", "cp1_rule1_u8");
    assert_accepted_zero_init(
        outcome,
        "s",
        &RegseqType::Scalar(regseq_model::ir::ScalarType::U8),
    );
}

#[test]
fn rule1_scalar_u16_accepted() {
    let outcome = compile_user_data_specimen("s", "u16", "cp1_rule1_u16");
    assert_accepted_zero_init(
        outcome,
        "s",
        &RegseqType::Scalar(regseq_model::ir::ScalarType::U16),
    );
}

#[test]
fn rule1_scalar_u32_accepted() {
    let outcome = compile_user_data_specimen("s", "u32", "cp1_rule1_u32");
    assert_accepted_zero_init(
        outcome,
        "s",
        &RegseqType::Scalar(regseq_model::ir::ScalarType::U32),
    );
}

#[test]
fn rule1_scalar_u64_accepted() {
    let outcome = compile_user_data_specimen("s", "u64", "cp1_rule1_u64");
    assert_accepted_zero_init(
        outcome,
        "s",
        &RegseqType::Scalar(regseq_model::ir::ScalarType::U64),
    );
}

#[test]
fn rule1_scalar_bool_accepted() {
    let outcome = compile_user_data_specimen("s", "bool", "cp1_rule1_bool");
    assert_accepted_zero_init(outcome, "s", &RegseqType::Bool);
}

// ----- Positive: array arm -------------------------------------------

#[test]
fn rule1_array_u32_4_accepted() {
    let outcome = compile_user_data_specimen("a", "[u32; 4]", "cp1_rule1_arr_u32_4");
    let expected = RegseqType::Array(
        Box::new(RegseqType::Scalar(regseq_model::ir::ScalarType::U32)),
        4,
    );
    assert_accepted_zero_init(outcome, "a", &expected);
}

#[test]
fn rule1_array_u8_16_accepted() {
    let outcome = compile_user_data_specimen("a", "[u8; 16]", "cp1_rule1_arr_u8_16");
    let expected = RegseqType::Array(
        Box::new(RegseqType::Scalar(regseq_model::ir::ScalarType::U8)),
        16,
    );
    assert_accepted_zero_init(outcome, "a", &expected);
}

#[test]
fn rule1_array_nested_u16_2x8_accepted() {
    let outcome = compile_user_data_specimen("a", "[[u16; 2]; 8]", "cp1_rule1_arr_nested");
    let inner = RegseqType::Array(
        Box::new(RegseqType::Scalar(regseq_model::ir::ScalarType::U16)),
        2,
    );
    let expected = RegseqType::Array(Box::new(inner), 8);
    assert_accepted_zero_init(outcome, "a", &expected);
}

// ----- Positive: Copy struct arm -------------------------------------

#[test]
fn rule1_copy_struct_accepted() {
    const SRC: &str = r#"
#![no_std]
#[derive(Copy, Clone)]
pub struct Counters { pub rx: u32, pub tx: u32 }

#[rseq::driver(name = "d", compatible = ["x"], bus = "amba")]
pub struct D {
    pub counters: Counters,
}
"#;
    let outcome = compile_and_build(SRC, "cp1_rule1_copy_struct");
    let model = outcome.expect("Rule-1 Copy struct must be accepted");
    let driver = find_driver_module(&model)
        .and_then(|m| m.driver.as_ref())
        .expect("driver struct");
    let field = driver
        .user_data
        .iter()
        .find(|u| model.symbols.resolve(u.field_name) == "counters")
        .expect("counters user-data field present");
    match &field.kind {
        regseq_model::model::UserDataKind::ZeroInit { inner_type } => {
            assert!(
                matches!(inner_type, RegseqType::Defined(_)),
                "Counters inner_type must resolve to Defined(TypeDefId), got {inner_type:?}",
            );
        }
        other => panic!("expected ZeroInit, got {other:?}"),
    }
}

// ----- Positive: nested Copy struct arm ------------------------------

#[test]
fn rule1_nested_copy_struct_accepted() {
    const SRC: &str = r#"
#![no_std]
#[derive(Copy, Clone)]
pub struct Inner { pub x: u8 }
#[derive(Copy, Clone)]
pub struct Outer { pub inner: Inner }

#[rseq::driver(name = "d", compatible = ["x"], bus = "amba")]
pub struct D {
    pub state: Outer,
}
"#;
    let outcome = compile_and_build(SRC, "cp1_rule1_nested_copy_struct");
    outcome.expect("nested Rule-1 Copy struct must be accepted");
}

// ----- Positive: Rule-2 regressions ----------------------------------

#[test]
fn rule2_option_u32_accepted() {
    let outcome = compile_user_data_specimen("o", "Option<u32>", "cp1_rule2_opt_u32");
    let model = outcome.expect("Option<u32> must be accepted");
    let driver = find_driver_module(&model)
        .and_then(|m| m.driver.as_ref())
        .expect("driver struct");
    let field = driver
        .user_data
        .iter()
        .find(|u| model.symbols.resolve(u.field_name) == "o")
        .expect("o user-data field present");
    match &field.kind {
        regseq_model::model::UserDataKind::Option { inner_type } => {
            assert_eq!(
                *inner_type,
                RegseqType::Scalar(regseq_model::ir::ScalarType::U32)
            );
        }
        other => panic!("expected Option, got {other:?}"),
    }
}

#[test]
fn rule2_option_u8_accepted() {
    let outcome = compile_user_data_specimen("o", "Option<u8>", "cp1_rule2_opt_u8");
    outcome.expect("Option<u8> must be accepted");
}

#[test]
fn rule2_option_i32_accepted() {
    // Critical regression check: `i32` is REJECTED by Rule-1
    // whitelist but MUST remain accepted by Rule-2 because `i32` is
    // `Copy`. Confirms the two predicates are not conflated.
    let outcome = compile_user_data_specimen("o", "Option<i32>", "cp1_rule2_opt_i32");
    let model = outcome.expect("Option<i32> must remain accepted by Rule-2");
    let driver = find_driver_module(&model)
        .and_then(|m| m.driver.as_ref())
        .expect("driver struct");
    let field = driver
        .user_data
        .iter()
        .find(|u| model.symbols.resolve(u.field_name) == "o")
        .expect("o user-data field present");
    match &field.kind {
        regseq_model::model::UserDataKind::Option { inner_type } => {
            assert_eq!(
                *inner_type,
                RegseqType::Scalar(regseq_model::ir::ScalarType::I32)
            );
        }
        other => panic!("expected Option, got {other:?}"),
    }
}

// ----- Negative Category A: Copy but not on whitelist ----------------

#[test]
fn rule1_reject_copy_enum() {
    const SRC: &str = r#"
#![no_std]
#[derive(Copy, Clone)]
pub enum E { A, B }

#[rseq::driver(name = "d", compatible = ["x"], bus = "amba")]
pub struct D {
    pub bad: E,
}
"#;
    let outcome = compile_and_build(SRC, "cp1_rule1_reject_enum");
    assert_rule1_whitelist_rejected(outcome);
}

#[test]
fn rule1_reject_copy_union() {
    const SRC: &str = r#"
#![no_std]
#[derive(Copy, Clone)]
pub union U { pub a: u32, pub b: u32 }

#[rseq::driver(name = "d", compatible = ["x"], bus = "amba")]
pub struct D {
    pub bad: U,
}
"#;
    let outcome = compile_and_build(SRC, "cp1_rule1_reject_union");
    assert_rule1_whitelist_rejected(outcome);
}

#[test]
fn rule1_reject_non_zero_u32() {
    const SRC: &str = r#"
#![no_std]
use core::num::NonZeroU32;

#[rseq::driver(name = "d", compatible = ["x"], bus = "amba")]
pub struct D {
    pub bad: NonZeroU32,
}
"#;
    let outcome = compile_and_build(SRC, "cp1_rule1_reject_nonzero_u32");
    assert_rule1_whitelist_rejected(outcome);
}

#[test]
fn rule1_reject_non_null() {
    const SRC: &str = r#"
#![no_std]
use core::ptr::NonNull;

#[rseq::driver(name = "d", compatible = ["x"], bus = "amba")]
pub struct D {
    pub bad: NonNull<u8>,
}
"#;
    let outcome = compile_and_build(SRC, "cp1_rule1_reject_nonnull");
    assert_rule1_whitelist_rejected(outcome);
}

#[test]
fn rule1_reject_static_reference() {
    const SRC: &str = r#"
#![no_std]

#[rseq::driver(name = "d", compatible = ["x"], bus = "amba")]
pub struct D {
    pub bad: &'static u32,
}
"#;
    let outcome = compile_and_build(SRC, "cp1_rule1_reject_static_ref");
    assert_rule1_whitelist_rejected(outcome);
}

#[test]
fn rule1_reject_fn_pointer() {
    const SRC: &str = r#"
#![no_std]

#[rseq::driver(name = "d", compatible = ["x"], bus = "amba")]
pub struct D {
    pub bad: fn() -> (),
}
"#;
    let outcome = compile_and_build(SRC, "cp1_rule1_reject_fn_ptr");
    assert_rule1_whitelist_rejected(outcome);
}

#[test]
fn rule1_reject_raw_pointer() {
    const SRC: &str = r#"
#![no_std]

#[rseq::driver(name = "d", compatible = ["x"], bus = "amba")]
pub struct D {
    pub bad: *mut u8,
}
"#;
    let outcome = compile_and_build(SRC, "cp1_rule1_reject_raw_ptr");
    assert_rule1_whitelist_rejected(outcome);
}

// ----- Negative Category A: non-Copy types (outer Copy check) --------

#[test]
fn rule1_reject_box_not_copy() {
    const SRC: &str = r#"
#![no_std]
extern crate alloc;
use alloc::boxed::Box;

#[rseq::driver(name = "d", compatible = ["x"], bus = "amba")]
pub struct D {
    pub bad: Box<u32>,
}
"#;
    let outcome = compile_and_build(SRC, "cp1_rule1_reject_box");
    assert_user_data_not_copy(outcome);
}

#[test]
fn rule1_reject_rc_not_copy() {
    const SRC: &str = r#"
#![no_std]
extern crate alloc;
use alloc::rc::Rc;

#[rseq::driver(name = "d", compatible = ["x"], bus = "amba")]
pub struct D {
    pub bad: Rc<u32>,
}
"#;
    let outcome = compile_and_build(SRC, "cp1_rule1_reject_rc");
    assert_user_data_not_copy(outcome);
}

#[test]
fn rule1_reject_arc_not_copy() {
    const SRC: &str = r#"
#![no_std]
extern crate alloc;
use alloc::sync::Arc;

#[rseq::driver(name = "d", compatible = ["x"], bus = "amba")]
pub struct D {
    pub bad: Arc<u32>,
}
"#;
    let outcome = compile_and_build(SRC, "cp1_rule1_reject_arc");
    assert_user_data_not_copy(outcome);
}

#[test]
fn rule1_reject_cell_not_copy() {
    const SRC: &str = r#"
#![no_std]
use core::cell::Cell;

#[rseq::driver(name = "d", compatible = ["x"], bus = "amba")]
pub struct D {
    pub bad: Cell<u32>,
}
"#;
    let outcome = compile_and_build(SRC, "cp1_rule1_reject_cell");
    assert_user_data_not_copy(outcome);
}

#[test]
fn rule1_reject_refcell_not_copy() {
    const SRC: &str = r#"
#![no_std]
use core::cell::RefCell;

#[rseq::driver(name = "d", compatible = ["x"], bus = "amba")]
pub struct D {
    pub bad: RefCell<u32>,
}
"#;
    let outcome = compile_and_build(SRC, "cp1_rule1_reject_refcell");
    assert_user_data_not_copy(outcome);
}

#[test]
fn rule1_reject_unsafecell_not_copy() {
    const SRC: &str = r#"
#![no_std]
use core::cell::UnsafeCell;

#[rseq::driver(name = "d", compatible = ["x"], bus = "amba")]
pub struct D {
    pub bad: UnsafeCell<u32>,
}
"#;
    let outcome = compile_and_build(SRC, "cp1_rule1_reject_unsafecell");
    assert_user_data_not_copy(outcome);
}

#[test]
fn rule1_reject_atomic_u32_not_copy() {
    const SRC: &str = r#"
#![no_std]
use core::sync::atomic::AtomicU32;

#[rseq::driver(name = "d", compatible = ["x"], bus = "amba")]
pub struct D {
    pub bad: AtomicU32,
}
"#;
    let outcome = compile_and_build(SRC, "cp1_rule1_reject_atomic_u32");
    assert_user_data_not_copy(outcome);
}

// ----- Negative Category C: out-of-whitelist primitives --------------

#[test]
fn rule1_reject_i8() {
    let outcome = compile_user_data_specimen("bad", "i8", "cp1_rule1_reject_i8");
    assert_rule1_whitelist_rejected(outcome);
}

#[test]
fn rule1_reject_i16() {
    let outcome = compile_user_data_specimen("bad", "i16", "cp1_rule1_reject_i16");
    assert_rule1_whitelist_rejected(outcome);
}

#[test]
fn rule1_reject_i32() {
    let outcome = compile_user_data_specimen("bad", "i32", "cp1_rule1_reject_i32");
    assert_rule1_whitelist_rejected(outcome);
}

#[test]
fn rule1_reject_i64() {
    let outcome = compile_user_data_specimen("bad", "i64", "cp1_rule1_reject_i64");
    assert_rule1_whitelist_rejected(outcome);
}

#[test]
fn rule1_reject_usize() {
    let outcome = compile_user_data_specimen("bad", "usize", "cp1_rule1_reject_usize");
    assert_rule1_whitelist_rejected(outcome);
}

#[test]
fn rule1_reject_isize() {
    let outcome = compile_user_data_specimen("bad", "isize", "cp1_rule1_reject_isize");
    assert_rule1_whitelist_rejected(outcome);
}

#[test]
fn rule1_reject_f32() {
    let outcome = compile_user_data_specimen("bad", "f32", "cp1_rule1_reject_f32");
    assert_rule1_whitelist_rejected(outcome);
}

#[test]
fn rule1_reject_f64() {
    let outcome = compile_user_data_specimen("bad", "f64", "cp1_rule1_reject_f64");
    assert_rule1_whitelist_rejected(outcome);
}

#[test]
fn rule1_reject_char() {
    let outcome = compile_user_data_specimen("bad", "char", "cp1_rule1_reject_char");
    assert_rule1_whitelist_rejected(outcome);
}

#[test]
fn rule1_reject_u128() {
    let outcome = compile_user_data_specimen("bad", "u128", "cp1_rule1_reject_u128");
    assert_rule1_whitelist_rejected(outcome);
}

#[test]
fn rule1_reject_i128() {
    let outcome = compile_user_data_specimen("bad", "i128", "cp1_rule1_reject_i128");
    assert_rule1_whitelist_rejected(outcome);
}

#[test]
fn rule1_reject_tuple() {
    let outcome = compile_user_data_specimen("bad", "(u32, u32)", "cp1_rule1_reject_tuple");
    assert_rule1_whitelist_rejected(outcome);
}

// ----- Negative Category B: generic / projection gate ----------------

#[test]
fn rule1_reject_generic_param() {
    const SRC: &str = r#"
#![no_std]

#[rseq::driver(name = "d", compatible = ["x"], bus = "amba")]
pub struct D<T: Copy> {
    pub bad: T,
}
"#;
    let outcome = compile_and_build(SRC, "cp1_rule1_reject_generic_param");
    assert_rule1_whitelist_rejected(outcome);
}

#[test]
fn rule1_reject_projection() {
    const SRC: &str = r#"
#![no_std]

pub trait HasAssoc { type Assoc: Copy; }

#[rseq::driver(name = "d", compatible = ["x"], bus = "amba")]
pub struct D<T: HasAssoc> {
    pub bad: T::Assoc,
}
"#;
    let outcome = compile_and_build(SRC, "cp1_rule1_reject_projection");
    assert_rule1_whitelist_rejected(outcome);
}

#[test]
fn negative_missing_resource_id() {
    const SRC: &str = r#"
#![no_std]
pub struct Clock;

#[rseq::driver(name = "d", compatible = ["x"], bus = "amba")]
pub struct D {
    #[rseq::clock]
    pub clk: Clock,
}
"#;
    let outcome = compile_and_build(SRC, "construction_pass1_missing_resource_id");
    assert!(matches!(
        outcome,
        Err(ConstructionError::MissingResourceId { kind: "clock", .. })
    ));
}

// ---------------------------------------------------------------------------
// Walk helpers
// ---------------------------------------------------------------------------

fn count_driver_modules(model: &RegseqModel) -> usize {
    fn walk(m: &regseq_model::model::Module) -> usize {
        let self_count = if m.kind == ModuleKind::Driver { 1 } else { 0 };
        self_count + m.submodules.iter().map(walk).sum::<usize>()
    }
    walk(&model.root_module)
}

fn find_driver_module(model: &RegseqModel) -> Option<&regseq_model::model::Module> {
    fn walk(m: &regseq_model::model::Module) -> Option<&regseq_model::model::Module> {
        if m.kind == ModuleKind::Driver {
            return Some(m);
        }
        for sub in &m.submodules {
            if let Some(x) = walk(sub) {
                return Some(x);
            }
        }
        None
    }
    walk(&model.root_module)
}

fn count_constants(model: &RegseqModel) -> usize {
    fn walk(m: &regseq_model::model::Module) -> usize {
        m.constants.len() + m.submodules.iter().map(walk).sum::<usize>()
    }
    walk(&model.root_module)
}

fn count_register_blocks(model: &RegseqModel) -> usize {
    fn walk(m: &regseq_model::model::Module) -> usize {
        m.register_blocks.len() + m.submodules.iter().map(walk).sum::<usize>()
    }
    walk(&model.root_module)
}

fn count_register_descriptors(model: &RegseqModel) -> usize {
    fn walk(m: &regseq_model::model::Module) -> usize {
        m.register_descriptors.len() + m.submodules.iter().map(walk).sum::<usize>()
    }
    walk(&model.root_module)
}

fn collect_functions(model: &RegseqModel) -> Vec<&regseq_model::model::Function> {
    fn walk<'a>(
        m: &'a regseq_model::model::Module,
        out: &mut Vec<&'a regseq_model::model::Function>,
    ) {
        out.extend(m.free_functions.iter());
        for b in &m.impl_blocks {
            out.extend(b.methods.iter());
        }
        for b in &m.trait_impls {
            out.extend(b.methods.iter());
        }
        for b in &m.driver_impl_blocks {
            out.extend(b.methods.iter());
        }
        for sub in &m.submodules {
            walk(sub, out);
        }
    }
    let mut out = Vec::new();
    walk(&model.root_module, &mut out);
    out
}

fn collect_register_block_ids(model: &RegseqModel) -> Vec<regseq_model::identity::RegisterBlockId> {
    fn walk(
        m: &regseq_model::model::Module,
        out: &mut Vec<regseq_model::identity::RegisterBlockId>,
    ) {
        for b in &m.register_blocks {
            out.push(b.id);
        }
        for sub in &m.submodules {
            walk(sub, out);
        }
    }
    let mut out = Vec::new();
    walk(&model.root_module, &mut out);
    out
}

fn collect_reachable_decl_ids(model: &RegseqModel) -> Vec<regseq_model::identity::DeclId> {
    use regseq_model::identity::DeclId;
    fn walk(m: &regseq_model::model::Module, out: &mut Vec<DeclId>) {
        out.push(m.id.decl_id());
        for c in &m.constants {
            out.push(c.id.decl_id());
        }
        for f in &m.free_functions {
            out.push(f.id.decl_id());
        }
        for td in &m.type_defs {
            out.push(td.id.decl_id());
        }
        for rd in &m.register_descriptors {
            out.push(rd.id.decl_id());
        }
        for rb in &m.register_blocks {
            out.push(rb.id.decl_id());
        }
        if let Some(drv) = &m.driver {
            for r in &drv.clocks {
                out.push(r.id.decl_id());
            }
            for r in &drv.resets {
                out.push(r.id.decl_id());
            }
            for r in &drv.pinctrl {
                out.push(r.id.decl_id());
            }
            for r in &drv.interrupts {
                out.push(r.id.decl_id());
            }
            for r in &drv.mmio {
                out.push(r.id.decl_id());
            }
            for r in &drv.sync {
                out.push(r.id.decl_id());
            }
            for p in &drv.properties {
                out.push(p.id.decl_id());
            }
            for u in &drv.user_data {
                out.push(u.id.decl_id());
            }
        }
        for b in &m.impl_blocks {
            for meth in &b.methods {
                out.push(meth.id.decl_id());
            }
            for c in &b.associated_constants {
                out.push(c.id.decl_id());
            }
        }
        for b in &m.trait_impls {
            for meth in &b.methods {
                out.push(meth.id.decl_id());
            }
            for c in &b.associated_constants {
                out.push(c.id.decl_id());
            }
        }
        for b in &m.driver_impl_blocks {
            for meth in &b.methods {
                out.push(meth.id.decl_id());
            }
            for c in &b.associated_constants {
                out.push(c.id.decl_id());
            }
        }
        for sub in &m.submodules {
            walk(sub, out);
        }
    }
    let mut out = Vec::new();
    walk(&model.root_module, &mut out);
    out
}

fn collect_classifications_with_attrs(model: &RegseqModel) -> Vec<(TypeDefClassification, bool)> {
    fn walk(m: &regseq_model::model::Module, out: &mut Vec<(TypeDefClassification, bool)>) {
        for td in &m.type_defs {
            out.push((td.classification.clone(), !td.attrs.entries.is_empty()));
        }
        for sub in &m.submodules {
            walk(sub, out);
        }
    }
    let mut out = Vec::new();
    walk(&model.root_module, &mut out);
    out
}

fn collect_classifications(model: &RegseqModel) -> Vec<TypeDefClassification> {
    fn walk(m: &regseq_model::model::Module, out: &mut Vec<TypeDefClassification>) {
        for td in &m.type_defs {
            out.push(td.classification.clone());
        }
        for sub in &m.submodules {
            walk(sub, out);
        }
    }
    let mut out = Vec::new();
    walk(&model.root_module, &mut out);
    out
}

fn find_type_def_attr<'m>(
    model: &'m RegseqModel,
    type_name: &str,
    attr_key: &str,
) -> Option<&'m AttrValue> {
    fn walk<'m>(
        m: &'m regseq_model::model::Module,
        symbols: &regseq_model::identity::SymbolInterner,
        type_name: &str,
        attr_key: &str,
    ) -> Option<&'m AttrValue> {
        for td in &m.type_defs {
            if symbols.resolve(td.name) == type_name {
                return td
                    .attrs
                    .entries
                    .iter()
                    .find(|e| e.key == attr_key)
                    .map(|e| &e.value);
            }
        }
        for sub in &m.submodules {
            if let Some(v) = walk(sub, symbols, type_name, attr_key) {
                return Some(v);
            }
        }
        None
    }
    walk(&model.root_module, &model.symbols, type_name, attr_key)
}

fn keyval_lookup<'v>(value: &'v AttrValue, key: &str) -> Option<&'v AttrValue> {
    match value {
        AttrValue::KeyValues(entries) => entries.iter().find(|e| e.key == key).map(|e| &e.value),
        _ => None,
    }
}

fn find_type_def_id(model: &RegseqModel, type_name: &str) -> Option<TypeDefId> {
    fn walk(
        m: &regseq_model::model::Module,
        symbols: &regseq_model::identity::SymbolInterner,
        name: &str,
    ) -> Option<TypeDefId> {
        for td in &m.type_defs {
            if symbols.resolve(td.name) == name {
                return Some(td.id);
            }
        }
        for sub in &m.submodules {
            if let Some(id) = walk(sub, symbols, name) {
                return Some(id);
            }
        }
        None
    }
    walk(&model.root_module, &model.symbols, type_name)
}

fn find_register_block<'m>(
    model: &'m RegseqModel,
    name: &str,
) -> Option<&'m regseq_model::model::RegisterBlock> {
    fn walk<'m>(
        m: &'m regseq_model::model::Module,
        symbols: &regseq_model::identity::SymbolInterner,
        name: &str,
    ) -> Option<&'m regseq_model::model::RegisterBlock> {
        for b in &m.register_blocks {
            if symbols.resolve(b.name) == name {
                return Some(b);
            }
        }
        for sub in &m.submodules {
            if let Some(b) = walk(sub, symbols, name) {
                return Some(b);
            }
        }
        None
    }
    walk(&model.root_module, &model.symbols, name)
}

fn find_register_descriptor<'m>(
    model: &'m RegseqModel,
    name: &str,
) -> Option<&'m regseq_model::model::RegisterDescriptor> {
    fn walk<'m>(
        m: &'m regseq_model::model::Module,
        symbols: &regseq_model::identity::SymbolInterner,
        name: &str,
    ) -> Option<&'m regseq_model::model::RegisterDescriptor> {
        for d in &m.register_descriptors {
            if symbols.resolve(d.name) == name {
                return Some(d);
            }
        }
        for sub in &m.submodules {
            if let Some(d) = walk(sub, symbols, name) {
                return Some(d);
            }
        }
        None
    }
    walk(&model.root_module, &model.symbols, name)
}

fn type_def_fields(
    model: &RegseqModel,
    id: TypeDefId,
) -> Option<Vec<(regseq_model::identity::Symbol, RegseqType)>> {
    fn walk(
        m: &regseq_model::model::Module,
        id: TypeDefId,
    ) -> Option<Vec<(regseq_model::identity::Symbol, RegseqType)>> {
        for td in &m.type_defs {
            if td.id == id {
                if let TypeDefKind::Struct { fields } = &td.kind {
                    return Some(fields.iter().map(|f| (f.name, f.ty.clone())).collect());
                }
                return None;
            }
        }
        for sub in &m.submodules {
            if let Some(v) = walk(sub, id) {
                return Some(v);
            }
        }
        None
    }
    walk(&model.root_module, id)
}
