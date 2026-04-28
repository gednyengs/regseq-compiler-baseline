//! Voided pending MonoRust + RegseqModel redesign.
//! See docs/discussions/compiler-refactoring/test-audit.md for the classification rationale
//! and the retirement criterion.
#![cfg(feature = "post-monorust-tests")]

//! Integration tests for the non-fail-fast lint channel surfaced by
//! [`verify_phase_1`] / [`verify_phase_2`] (task regseq-model-62).
//!
//! Covers:
//!
//! - Happy path: structurally valid Phase-1 model with no lints
//!   triggered returns `Ok(VerifyReport { lints: [] })`.
//! - One lint per rule: for every [`LintKind`] variant, a minimal
//!   model triggers exactly that rule.
//! - Determinism: two runs on a multi-lint model produce byte-equal
//!   serialized output (covering sort order across levels and
//!   subject DeclIds).
//!
//! Lint pass-through through [`MutationGuard::commit`] (no rollback
//! on lint-only model) is covered in `mutation_api_auto_verify.rs`.

use regseq_model_old::identity::{
    DeclId, FileId, FunctionId, ModuleId, PropertyId, ResourceId, SourceLocation, SourceOrigin,
    SymbolInterner, TypeDefId,
};
use regseq_model_old::ir::{
    BasicBlock, BasicBlockSourceMap, Body, BodySourceMap, LiteralValue, LocalDecl, RegseqType,
    ScalarType, Terminator, TerminatorKind,
};
use regseq_model_old::model::driver::{DriverIdentity, DriverStruct};
use regseq_model_old::model::framework::TypeDefClassification;
use regseq_model_old::model::function::Function;
use regseq_model_old::model::module::{Module, ModuleKind};
use regseq_model_old::model::regseq_model_old::RegseqModel;
use regseq_model_old::model::resources::{ClockDecl, ClockResource, PropertyField, Shape};
use regseq_model_old::model::type_def::{Attrs, TypeDef, TypeDefKind};
use regseq_model_old::verify::{verify_phase_1, LintKind, LintLevel};
use std::collections::{BTreeMap, BTreeSet};

// --- DeclId allocation for the minimal fixture ----------------------

const ROOT_MODULE_ID: u32 = 1;
const DRIVER_MODULE_ID: u32 = 2;
const DRIVER_TYPE_ID: u32 = 3;
const DRIVER_FUNCTION_ID: u32 = 4;
const CLOCK_RESOURCE_ID: u32 = 5;
const PROPERTY_A_ID: u32 = 6;
const PROPERTY_B_ID: u32 = 7;

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
            body_span: source.clone(),
            locals: vec![SourceOrigin::CompilerGenerated],
            blocks: vec![BasicBlockSourceMap {
                statements: vec![],
                terminator: SourceOrigin::CompilerGenerated,
            }],
        },
        ordered_regions: vec![],
    }
}

/// Build a minimal valid Phase-1 model with configurable properties.
fn build_phase_1_model(properties: Vec<PropertyField>) -> RegseqModel {
    let mut symbols = SymbolInterner::new();
    let file = symbols.intern_file("drv/lib.rs");

    let drv_type_sym = symbols.intern("MyDriver");
    let drv_identity_sym = symbols.intern("my-driver");
    let clk_field_sym = symbols.intern("apb_clk");
    let clk_id_sym = symbols.intern("apb");

    let func = Function {
        id: FunctionId::from(DeclId::new(DRIVER_FUNCTION_ID)),
        name: symbols.intern("init"),
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
        clocks: vec![ClockResource {
            id: ResourceId::from(DeclId::new(CLOCK_RESOURCE_ID)),
            field_name: clk_field_sym,
            shape: Shape::Single,
            decl: ClockDecl {
                clock_id: clk_id_sym,
            },
            binding: None,
        }],
        resets: vec![],
        pinctrl: vec![],
        interrupts: vec![],
        mmio: vec![],
        sync: vec![],
        properties,
        user_data: vec![],
    };

    let driver_module = Module {
        id: ModuleId::from(DeclId::new(DRIVER_MODULE_ID)),
        name: symbols.intern("drv"),
        kind: ModuleKind::Driver,
        submodules: vec![],
        constants: vec![],
        free_functions: vec![],
        type_defs: vec![driver_type_def],
        register_descriptors: vec![],
        register_blocks: vec![],
        impl_blocks: vec![],
        driver: Some(driver),
        trait_impls: vec![],
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
        free_functions: vec![func],
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
    source_locations.insert(DeclId::new(DRIVER_FUNCTION_ID), origin(file, 20));
    source_locations.insert(DeclId::new(CLOCK_RESOURCE_ID), origin(file, 40));
    // Cover every property DeclId produced by the caller; the Phase-1
    // cross-item rule requires a source-location entry per DeclId.
    let prop_a_decl = DeclId::new(PROPERTY_A_ID);
    let prop_b_decl = DeclId::new(PROPERTY_B_ID);
    source_locations.insert(prop_a_decl, origin(file, 60));
    source_locations.insert(prop_b_decl, origin(file, 70));

    RegseqModel {
        symbols,
        source_locations,
        root_module,
        format_version: 1,
    }
}

fn property(
    symbols: &mut SymbolInterner,
    id: u32,
    field_name: &str,
    property_id: &str,
    required: bool,
    default_value: Option<LiteralValue>,
) -> PropertyField {
    PropertyField {
        id: PropertyId::from(DeclId::new(id)),
        field_name: symbols.intern(field_name),
        field_type: ScalarType::U32,
        property_id: symbols.intern(property_id),
        default_value,
        vendor_prefix: None,
        required,
        bound_value: None,
    }
}

// ---------------------------------------------------------------------
// Happy path: structurally valid Phase-1 model with no lint rules
// triggered returns Ok(VerifyReport { lints: [] }).
// ---------------------------------------------------------------------

#[test]
fn happy_path_minimal_valid_model_produces_empty_lint_list() {
    let model = build_phase_1_model(vec![]);
    let report = verify_phase_1(&model).expect("structurally valid model");
    assert!(
        report.lints.is_empty(),
        "no lint rules must fire on a minimal valid model: {:?}",
        report.lints
    );
}

#[test]
fn required_property_without_default_does_not_trigger_lint() {
    // A required property with no default is the normal shape and
    // must not trigger the PropertyRequiredWithDefault rule.
    let mut model = build_phase_1_model(vec![]);
    let prop = property(
        &mut model.symbols,
        PROPERTY_A_ID,
        "clock_frequency",
        "clock-frequency",
        true,
        None,
    );
    let driver = model
        .root_module
        .submodules
        .iter_mut()
        .find(|m| m.kind == ModuleKind::Driver)
        .and_then(|m| m.driver.as_mut())
        .expect("driver");
    driver.properties.push(prop);

    let report = verify_phase_1(&model).expect("structurally valid model");
    assert!(report.lints.is_empty());
}

#[test]
fn optional_property_with_default_does_not_trigger_lint() {
    // An optional property (required == false) with a default is
    // fine: the default is consumable when the platform file omits
    // the key. Only required+default is the offending shape.
    let mut model = build_phase_1_model(vec![]);
    let prop = property(
        &mut model.symbols,
        PROPERTY_A_ID,
        "clock_frequency",
        "clock-frequency",
        false,
        Some(LiteralValue::U32(24_000_000)),
    );
    let driver = model
        .root_module
        .submodules
        .iter_mut()
        .find(|m| m.kind == ModuleKind::Driver)
        .and_then(|m| m.driver.as_mut())
        .expect("driver");
    driver.properties.push(prop);

    let report = verify_phase_1(&model).expect("structurally valid model");
    assert!(report.lints.is_empty());
}

// ---------------------------------------------------------------------
// One lint per rule: PropertyRequiredWithDefault.
// ---------------------------------------------------------------------

#[test]
fn property_required_with_default_triggers_exactly_one_lint() {
    let mut model = build_phase_1_model(vec![]);
    let prop = property(
        &mut model.symbols,
        PROPERTY_A_ID,
        "clock_frequency",
        "clock-frequency",
        true,
        Some(LiteralValue::U32(24_000_000)),
    );
    let driver = model
        .root_module
        .submodules
        .iter_mut()
        .find(|m| m.kind == ModuleKind::Driver)
        .and_then(|m| m.driver.as_mut())
        .expect("driver");
    driver.properties.push(prop);

    let report = verify_phase_1(&model).expect("structurally valid model");
    assert_eq!(report.lints.len(), 1, "exactly one lint must fire");
    let finding = &report.lints[0];
    assert_eq!(finding.level, LintLevel::Advisory);
    match &finding.kind {
        LintKind::PropertyRequiredWithDefault { property } => {
            assert_eq!(*property, DeclId::new(PROPERTY_A_ID));
        }
    }
    assert!(
        !finding.details.is_empty(),
        "every lint must carry reviewer-facing details",
    );
}

// ---------------------------------------------------------------------
// Determinism: two runs on a multi-lint model produce byte-identical
// serialized VerifyReport. The fixture uses two lint-triggering
// properties with reversed DeclId ordering so the sort key is
// exercised.
// ---------------------------------------------------------------------

#[test]
fn verify_report_serialization_is_byte_equal_across_runs() {
    let mut model = build_phase_1_model(vec![]);
    // Use B before A in the properties vector so the rule walks them
    // in non-sorted-id order; determinism then depends on the
    // VerifyReport sorter, not on the iteration order of the source.
    let prop_b = property(
        &mut model.symbols,
        PROPERTY_B_ID,
        "baud_rate",
        "baud-rate",
        true,
        Some(LiteralValue::U32(115_200)),
    );
    let prop_a = property(
        &mut model.symbols,
        PROPERTY_A_ID,
        "clock_frequency",
        "clock-frequency",
        true,
        Some(LiteralValue::U32(24_000_000)),
    );
    let driver = model
        .root_module
        .submodules
        .iter_mut()
        .find(|m| m.kind == ModuleKind::Driver)
        .and_then(|m| m.driver.as_mut())
        .expect("driver");
    driver.properties.push(prop_b);
    driver.properties.push(prop_a);

    let run_a = verify_phase_1(&model).expect("structurally valid");
    let run_b = verify_phase_1(&model).expect("structurally valid");

    let json_a = serde_json::to_string(&run_a).expect("serialize run A");
    let json_b = serde_json::to_string(&run_b).expect("serialize run B");
    assert_eq!(
        json_a, json_b,
        "two runs must produce byte-identical VerifyReport serialization",
    );

    // Assert canonical sort order: PROPERTY_A_ID (6) must precede
    // PROPERTY_B_ID (7) regardless of source-side Vec order.
    assert_eq!(run_a.lints.len(), 2);
    let ids: Vec<DeclId> = run_a
        .lints
        .iter()
        .map(|f| match &f.kind {
            LintKind::PropertyRequiredWithDefault { property } => *property,
        })
        .collect();
    assert_eq!(
        ids,
        vec![DeclId::new(PROPERTY_A_ID), DeclId::new(PROPERTY_B_ID)],
        "findings must sort by DeclId, not by source Vec order",
    );
}
