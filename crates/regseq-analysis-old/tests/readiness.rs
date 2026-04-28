//! Voided pending MonoRust + RegseqModel redesign.
//! See docs/discussions/compiler-refactoring/test-audit.md for the classification rationale
//! and the retirement criterion.
#![cfg(feature = "post-monorust-tests")]

//! Integration tests for [`regseq_analysis_old::readiness::check_readiness`].
//!
//! Each test constructs a minimal hand-built [`RegseqModel`], runs
//! [`verify_phase_1`] (to produce a real [`VerifyReport`]), passes both
//! to `check_readiness`, and asserts the resulting [`ReadinessReport`]
//! surfaces the expected reviewer-facing observations.

use regseq_analysis_old::readiness::{check_readiness, ReadinessReport, ResourceKind};
use regseq_model_old::identity::{
    DeclId, FileId, FunctionId, ModuleId, PropertyId, RegisterBlockId, ResourceId, SourceLocation,
    SourceOrigin, SymbolInterner, TypeDefId,
};
use regseq_model_old::ir::{
    BasicBlock, BasicBlockSourceMap, Body, BodySourceMap, LiteralValue, LocalDecl, RegseqType,
    ScalarType, Terminator, TerminatorKind,
};
use regseq_model_old::model::driver::{DriverIdentity, DriverStruct};
use regseq_model_old::model::framework::TypeDefClassification;
use regseq_model_old::model::function::Function;
use regseq_model_old::model::impl_block::ImplBlock;
use regseq_model_old::model::module::{Module, ModuleKind};
use regseq_model_old::model::register::{RegisterBlock, RegisterBlockAttributes};
use regseq_model_old::model::regseq_model::RegseqModel;
use regseq_model_old::model::resources::{
    ClockDecl, ClockResource, MmioDecl, MmioResource, PropertyField, Shape,
};
use regseq_model_old::model::type_def::{Attrs, TypeDef, TypeDefKind};
use regseq_model_old::model::CallbackFamily;
use regseq_model_old::verify::{verify_phase_1, LintKind, LintLevel};
use std::collections::{BTreeMap, BTreeSet};

// -------------------------------------------------------------------
// Minimal model builders
// -------------------------------------------------------------------

const ROOT_MODULE_ID: u32 = 1;
const DRIVER_MODULE_ID: u32 = 2;
const DRIVER_TYPE_ID: u32 = 3;
const FREE_FN_ID: u32 = 4;
const CLOCK_RESOURCE_ID: u32 = 5;
const MMIO_RESOURCE_ID: u32 = 6;
const PROPERTY_A_ID: u32 = 7;
const LIFECYCLE_INIT_ID: u32 = 8;
const LIFECYCLE_DEINIT_ID: u32 = 9;
const SERIAL_TX_ID: u32 = 10;
const MMIO_REG_BLOCK_ID: u32 = 11;

fn origin(file: FileId, line: u32) -> SourceOrigin {
    SourceOrigin::Source(SourceLocation {
        file,
        start_line: line,
        start_column: 1,
        end_line: line,
        end_column: 2,
    })
}

fn non_placeholder_body(source: SourceOrigin) -> Body {
    Body {
        local_decls: vec![LocalDecl {
            ty: RegseqType::Unit,
            mutable: false,
            name: None,
        }],
        arg_count: 0,
        basic_blocks: vec![BasicBlock {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::Return,
            },
        }],
        source_map: BodySourceMap {
            body_span: source,
            locals: vec![SourceOrigin::CompilerGenerated],
            blocks: vec![BasicBlockSourceMap {
                statements: vec![],
                terminator: SourceOrigin::CompilerGenerated,
            }],
        },
        ordered_regions: vec![],
    }
}

/// Configuration knobs for building a minimal model that still passes
/// Phase-1 verification.
#[derive(Default)]
struct MinimalModelConfig {
    /// Include one [`ClockResource`] named `apb_clk`.
    with_clock: bool,
    /// Include one [`MmioResource`] named `regs`.
    with_mmio: bool,
    /// Include a [`PropertyField`] that triggers
    /// [`LintKind::PropertyRequiredWithDefault`].
    with_property_required_with_default: bool,
    /// Populate `trait_impls` with a `Lifecycle` (init+deinit) impl.
    with_lifecycle_impl: bool,
    /// Populate `trait_impls` with a `Serial` (tx_byte) impl.
    with_serial_impl: bool,
}

fn build_model(config: MinimalModelConfig) -> RegseqModel {
    let mut symbols = SymbolInterner::new();
    let file = symbols.intern_file("drv/lib.rs");

    let drv_type_sym = symbols.intern("MyDriver");
    let drv_identity_sym = symbols.intern("my-driver");

    let init_fn = Function {
        id: FunctionId::from(DeclId::new(FREE_FN_ID)),
        name: symbols.intern("bootstrap"),
        params: vec![],
        return_type: RegseqType::Unit,
        body: non_placeholder_body(origin(file, 20)),
        attrs: Attrs::default(),
    };

    let driver_type_def = TypeDef {
        id: TypeDefId::from(DeclId::new(DRIVER_TYPE_ID)),
        name: drv_type_sym,
        kind: TypeDefKind::Struct { fields: vec![] },
        classification: TypeDefClassification::UserDefined,
        attrs: Attrs::default(),
    };

    let mut clocks = Vec::new();
    if config.with_clock {
        let clk_field_sym = symbols.intern("apb_clk");
        let clk_id_sym = symbols.intern("apb");
        clocks.push(ClockResource {
            id: ResourceId::from(DeclId::new(CLOCK_RESOURCE_ID)),
            field_name: clk_field_sym,
            shape: Shape::Single,
            decl: ClockDecl {
                clock_id: clk_id_sym,
            },
            binding: None,
        });
    }

    let mut mmio = Vec::new();
    let mut register_blocks: Vec<RegisterBlock> = Vec::new();
    if config.with_mmio {
        let mmio_field_sym = symbols.intern("regs");
        let mmio_id_sym = symbols.intern("uart_regs");
        let reg_block_sym = symbols.intern("UartRegs");
        let endian_sym = symbols.intern("little");
        mmio.push(MmioResource {
            id: ResourceId::from(DeclId::new(MMIO_RESOURCE_ID)),
            field_name: mmio_field_sym,
            shape: Shape::Single,
            decl: MmioDecl {
                mmio_id: mmio_id_sym,
                register_block: RegisterBlockId::from(DeclId::new(MMIO_REG_BLOCK_ID)),
            },
            binding: None,
        });
        register_blocks.push(RegisterBlock {
            id: RegisterBlockId::from(DeclId::new(MMIO_REG_BLOCK_ID)),
            name: reg_block_sym,
            block_attributes: RegisterBlockAttributes {
                endian: endian_sym,
                access_width: 32,
            },
            fields: vec![],
        });
    }

    let mut properties = Vec::new();
    if config.with_property_required_with_default {
        let prop_field_sym = symbols.intern("clock_frequency");
        let prop_id_sym = symbols.intern("clock-frequency");
        properties.push(PropertyField {
            id: PropertyId::from(DeclId::new(PROPERTY_A_ID)),
            field_name: prop_field_sym,
            field_type: ScalarType::U32,
            property_id: prop_id_sym,
            default_value: Some(LiteralValue::U32(24_000_000)),
            vendor_prefix: None,
            required: true,
            bound_value: None,
        });
    }

    let driver = DriverStruct {
        type_id: TypeDefId::from(DeclId::new(DRIVER_TYPE_ID)),
        name: drv_type_sym,
        identity: DriverIdentity {
            name: drv_identity_sym,
            compatible: vec![],
            bus: None,
        },
        device_class: None,
        pm_info: None,
        platform_metadata: None,
        clocks,
        resets: vec![],
        pinctrl: vec![],
        interrupts: vec![],
        mmio,
        sync: vec![],
        properties,
        user_data: vec![],
    };

    let drv_type_id = TypeDefId::from(DeclId::new(DRIVER_TYPE_ID));
    let mut trait_impls: Vec<ImplBlock> = Vec::new();
    if config.with_lifecycle_impl {
        let lifecycle_trait = symbols.intern("Lifecycle");
        let init = Function {
            id: FunctionId::from(DeclId::new(LIFECYCLE_INIT_ID)),
            name: symbols.intern("init"),
            params: vec![],
            return_type: RegseqType::Unit,
            body: non_placeholder_body(origin(file, 80)),
            attrs: Attrs::default(),
        };
        let deinit = Function {
            id: FunctionId::from(DeclId::new(LIFECYCLE_DEINIT_ID)),
            name: symbols.intern("deinit"),
            params: vec![],
            return_type: RegseqType::Unit,
            body: non_placeholder_body(origin(file, 90)),
            attrs: Attrs::default(),
        };
        trait_impls.push(ImplBlock {
            target_type: drv_type_id,
            trait_name: Some(lifecycle_trait),
            methods: vec![init, deinit],
            associated_constants: vec![],
        });
    }
    if config.with_serial_impl {
        let serial_trait = symbols.intern("Serial");
        let tx = Function {
            id: FunctionId::from(DeclId::new(SERIAL_TX_ID)),
            name: symbols.intern("tx_byte"),
            params: vec![],
            return_type: RegseqType::Unit,
            body: non_placeholder_body(origin(file, 110)),
            attrs: Attrs::default(),
        };
        trait_impls.push(ImplBlock {
            target_type: drv_type_id,
            trait_name: Some(serial_trait),
            methods: vec![tx],
            associated_constants: vec![],
        });
    }

    let driver_module = Module {
        id: ModuleId::from(DeclId::new(DRIVER_MODULE_ID)),
        name: symbols.intern("drv"),
        kind: ModuleKind::Driver,
        submodules: vec![],
        constants: vec![],
        free_functions: vec![],
        type_defs: vec![driver_type_def],
        register_descriptors: vec![],
        register_blocks,
        impl_blocks: vec![],
        driver: Some(driver),
        trait_impls,
        driver_impl_blocks: vec![],
        abstractions_used: BTreeSet::new(),
        intrinsics_used: BTreeSet::new(),
    };

    let root_module = Module {
        id: ModuleId::from(DeclId::new(ROOT_MODULE_ID)),
        name: symbols.intern("crate_root"),
        kind: ModuleKind::Normal,
        submodules: vec![driver_module],
        constants: vec![],
        free_functions: vec![init_fn],
        type_defs: vec![],
        register_descriptors: vec![],
        register_blocks: vec![],
        impl_blocks: vec![],
        driver: None,
        trait_impls: vec![],
        driver_impl_blocks: vec![],
        abstractions_used: BTreeSet::new(),
        intrinsics_used: BTreeSet::new(),
    };

    let mut source_locations = BTreeMap::new();
    source_locations.insert(DeclId::new(ROOT_MODULE_ID), origin(file, 1));
    source_locations.insert(DeclId::new(DRIVER_MODULE_ID), origin(file, 2));
    source_locations.insert(DeclId::new(DRIVER_TYPE_ID), origin(file, 10));
    source_locations.insert(DeclId::new(FREE_FN_ID), origin(file, 20));
    if config.with_clock {
        source_locations.insert(DeclId::new(CLOCK_RESOURCE_ID), origin(file, 40));
    }
    if config.with_mmio {
        source_locations.insert(DeclId::new(MMIO_RESOURCE_ID), origin(file, 50));
        source_locations.insert(DeclId::new(MMIO_REG_BLOCK_ID), origin(file, 55));
    }
    if config.with_property_required_with_default {
        source_locations.insert(DeclId::new(PROPERTY_A_ID), origin(file, 60));
    }
    if config.with_lifecycle_impl {
        source_locations.insert(DeclId::new(LIFECYCLE_INIT_ID), origin(file, 80));
        source_locations.insert(DeclId::new(LIFECYCLE_DEINIT_ID), origin(file, 90));
    }
    if config.with_serial_impl {
        source_locations.insert(DeclId::new(SERIAL_TX_ID), origin(file, 110));
    }

    RegseqModel {
        symbols,
        source_locations,
        root_module,
        format_version: 1,
    }
}

// -------------------------------------------------------------------
// Resource field inventory
// -------------------------------------------------------------------

#[test]
fn resource_field_names_surface_clock_and_mmio_entries() {
    let model = build_model(MinimalModelConfig {
        with_clock: true,
        with_mmio: true,
        ..MinimalModelConfig::default()
    });
    let report = verify_phase_1(&model).expect("minimal model must verify");
    let readiness = check_readiness(&model, &report);

    let families: BTreeSet<ResourceKind> = readiness
        .resource_field_names
        .iter()
        .map(|(kind, _)| *kind)
        .collect();
    assert!(families.contains(&ResourceKind::Clock));
    assert!(families.contains(&ResourceKind::Mmio));

    let names: BTreeSet<String> = readiness
        .resource_field_names
        .iter()
        .map(|(_, name)| name.clone())
        .collect();
    assert!(names.contains("apb_clk"));
    assert!(names.contains("regs"));
}

#[test]
fn resource_field_names_empty_when_driver_has_no_fields() {
    let model = build_model(MinimalModelConfig::default());
    let report = verify_phase_1(&model).expect("minimal model must verify");
    let readiness = check_readiness(&model, &report);
    assert!(readiness.resource_field_names.is_empty());
}

// -------------------------------------------------------------------
// Callback inventory
// -------------------------------------------------------------------

#[test]
fn callback_families_and_impls_cover_declared_traits() {
    let model = build_model(MinimalModelConfig {
        with_lifecycle_impl: true,
        with_serial_impl: true,
        ..MinimalModelConfig::default()
    });
    let report = verify_phase_1(&model).expect("minimal model must verify");
    let readiness = check_readiness(&model, &report);

    let expected: BTreeSet<CallbackFamily> = [CallbackFamily::Lifecycle, CallbackFamily::Serial]
        .into_iter()
        .collect();
    assert_eq!(readiness.callback_families_present, expected);

    // Lifecycle has two methods (init, deinit), Serial has one (tx_byte).
    assert_eq!(readiness.callback_impls.len(), 3);
    let lifecycle_count = readiness
        .callback_impls
        .iter()
        .filter(|ci| ci.family == CallbackFamily::Lifecycle)
        .count();
    assert_eq!(lifecycle_count, 2);
    let serial_count = readiness
        .callback_impls
        .iter()
        .filter(|ci| ci.family == CallbackFamily::Serial)
        .count();
    assert_eq!(serial_count, 1);
}

#[test]
fn callback_inventory_empty_when_no_trait_impls_declared() {
    let model = build_model(MinimalModelConfig::default());
    let report = verify_phase_1(&model).expect("minimal model must verify");
    let readiness = check_readiness(&model, &report);
    assert!(readiness.callback_families_present.is_empty());
    assert!(readiness.callback_impls.is_empty());
}

// -------------------------------------------------------------------
// Property lints sourced from the VerifyReport
// -------------------------------------------------------------------

#[test]
fn property_required_with_default_surfaces_as_a_lint() {
    let model = build_model(MinimalModelConfig {
        with_property_required_with_default: true,
        ..MinimalModelConfig::default()
    });
    let report = verify_phase_1(&model).expect("minimal model must verify");
    assert_eq!(
        report.lints.len(),
        1,
        "verify_phase_1 must emit exactly one property lint"
    );

    let readiness = check_readiness(&model, &report);
    assert_eq!(readiness.property_lints.len(), 1);
    let lint = &readiness.property_lints[0];
    assert_eq!(lint.level, LintLevel::Advisory);
    match &lint.kind {
        LintKind::PropertyRequiredWithDefault { property } => {
            assert_eq!(*property, DeclId::new(PROPERTY_A_ID));
        }
    }
}

#[test]
fn property_lints_empty_when_verify_report_has_no_lints() {
    let model = build_model(MinimalModelConfig {
        with_clock: true,
        ..MinimalModelConfig::default()
    });
    let report = verify_phase_1(&model).expect("minimal model must verify");
    assert!(report.lints.is_empty());
    let readiness = check_readiness(&model, &report);
    assert!(readiness.property_lints.is_empty());
}

// -------------------------------------------------------------------
// Determinism
// -------------------------------------------------------------------

#[test]
fn check_readiness_is_deterministic_across_repeated_calls() {
    let model = build_model(MinimalModelConfig {
        with_clock: true,
        with_mmio: true,
        with_lifecycle_impl: true,
        with_serial_impl: true,
        with_property_required_with_default: true,
    });
    let report = verify_phase_1(&model).expect("minimal model must verify");

    let a = check_readiness(&model, &report);
    let b = check_readiness(&model, &report);

    assert_eq!(a.callback_families_present, b.callback_families_present);
    project_impls(&a)
        .iter()
        .zip(project_impls(&b).iter())
        .for_each(|(x, y)| assert_eq!(x, y));
    assert_eq!(a.resource_field_names, b.resource_field_names);
    assert_eq!(a.property_lints, b.property_lints);
}

fn project_impls(report: &ReadinessReport) -> Vec<(CallbackFamily, u32, u32)> {
    report
        .callback_impls
        .iter()
        .map(|ci| {
            (
                ci.family,
                ci.function_id.0.index(),
                ci.driver_type.0.index(),
            )
        })
        .collect()
}
