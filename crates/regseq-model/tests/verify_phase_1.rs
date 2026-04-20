//! Integration tests for the post-Phase-1 verifier (task
//! regseq-model-55).
//!
//! The happy-path builder ([`build_minimal_ok`]) constructs a minimal
//! valid Phase-1 model programmatically. The spec explicitly calls out
//! that this task must NOT depend on any golden from another task:
//! task 54 committed only per-function Body goldens, and task 60 is
//! the first to land a model-level golden.

use std::collections::{BTreeMap, BTreeSet};

use regseq_model::identity::{
    DeclId, FileId, FunctionId, ModuleId, PropertyId, RegisterBlockId, ResourceId, SourceLocation,
    SourceOrigin, SymbolInterner, TypeDefId,
};
use regseq_model::ir::{
    BasicBlock, BasicBlockSourceMap, Body, BodySourceMap, LocalDecl, RegseqType, ScalarType,
    Terminator, TerminatorKind,
};
use regseq_model::model::driver::{DriverIdentity, DriverStruct};
use regseq_model::model::framework::TypeDefClassification;
use regseq_model::model::function::{Function, Param, ParamKind};
use regseq_model::model::module::{Module, ModuleKind};
use regseq_model::model::regseq_model::RegseqModel;
use regseq_model::model::resources::{
    ClockBinding, ClockDecl, ClockResource, PropertyField, Shape,
};
use regseq_model::model::type_def::{Attrs, TypeDef, TypeDefKind};
use regseq_model::verify::{verify_phase_1, VerifierErrorKind, VerifierLevel};

// --- DeclId allocation for the minimal fixture ----------------------
//
// The happy-path model uses a fixed, small set of DeclIds. Tests that
// mutate the model pick higher ids to avoid collision.
const ROOT_MODULE_ID: u32 = 1;
const DRIVER_MODULE_ID: u32 = 2;
const DRIVER_TYPE_ID: u32 = 3;
const DRIVER_FUNCTION_ID: u32 = 4;
const MINIMAL_CLOCK_RESOURCE_ID: u32 = 5;

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

/// Build a minimal valid Phase-1 model.
///
/// One Driver module `drv` with a `MyDriver` struct that carries one
/// clock resource (`binding == None`, the Phase-1 invariant) and zero
/// other resources / properties / user-data fields. One registered
/// free function (`init`) lives on the root module so the declaration
/// walker has a function to inspect. Every allocated `DeclId` has an
/// entry in `source_locations`.
///
/// The clock resource is present specifically so the
/// [`rejects_phase_1_model_with_populated_clock_binding`] test can
/// mutate `binding` from `None` to `Some(..)` on an otherwise-valid
/// Phase-1 model -- the brief's required proof that the post-Phase-1
/// invariant is enforced (see
/// `.harness/spec-batches/regseqmodel-phase-6.md` lines 156-158 and
/// the task spec's test plan lines 200-205).
fn build_minimal_ok() -> RegseqModel {
    let mut symbols = SymbolInterner::new();
    let file = symbols.intern_file("drv/lib.rs");

    let drv_type_sym = symbols.intern("MyDriver");
    let drv_identity_sym = symbols.intern("my-driver");
    let clk_field_sym = symbols.intern("apb_clk");
    let clk_id_sym = symbols.intern("apb");

    let function_body_origin = origin(file, 20);
    let func = Function {
        id: FunctionId::from(DeclId::new(DRIVER_FUNCTION_ID)),
        name: symbols.intern("init"),
        params: vec![],
        return_type: RegseqType::Unit,
        body: non_placeholder_body(function_body_origin),
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
            id: ResourceId::from(DeclId::new(MINIMAL_CLOCK_RESOURCE_ID)),
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
        properties: vec![],
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
    source_locations.insert(DeclId::new(MINIMAL_CLOCK_RESOURCE_ID), origin(file, 40));

    RegseqModel {
        symbols,
        source_locations,
        root_module,
        format_version: 1,
    }
}

// ---------------------------------------------------------------------
// Happy path
// ---------------------------------------------------------------------

#[test]
fn happy_path_minimal_valid_phase_1_model_passes() {
    let model = build_minimal_ok();
    assert!(verify_phase_1(&model).is_ok());
}

// ---------------------------------------------------------------------
// Negative: missing driver module
// ---------------------------------------------------------------------

#[test]
fn rejects_model_with_zero_driver_modules() {
    let mut model = build_minimal_ok();
    model.root_module.submodules.clear();

    let err = verify_phase_1(&model).unwrap_err();
    assert_eq!(err.level, VerifierLevel::Module);
    assert_eq!(err.kind, VerifierErrorKind::MissingDriverModule);
}

// ---------------------------------------------------------------------
// Negative: multiple driver modules
// ---------------------------------------------------------------------

#[test]
fn rejects_model_with_two_driver_modules() {
    let mut model = build_minimal_ok();

    // Append a second Driver-kind module with a fresh DeclId.
    let second_id = DeclId::new(100);
    let name = model.symbols.intern("drv2");
    let second = Module {
        id: ModuleId::from(second_id),
        name,
        kind: ModuleKind::Driver,
        submodules: vec![],
        constants: vec![],
        free_functions: vec![],
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
    let file = model.symbols.intern_file("drv/lib.rs");
    model.source_locations.insert(second_id, origin(file, 100));
    model.root_module.submodules.push(second);

    let err = verify_phase_1(&model).unwrap_err();
    assert_eq!(err.level, VerifierLevel::Module);
    assert_eq!(
        err.kind,
        VerifierErrorKind::MultipleDriverModules { count: 2 }
    );
}

// ---------------------------------------------------------------------
// Negative: duplicate item name within a module
// ---------------------------------------------------------------------

#[test]
fn rejects_module_with_duplicate_item_name() {
    let mut model = build_minimal_ok();

    let duplicate_name = model.symbols.intern("init");
    let file = model.symbols.intern_file("drv/lib.rs");
    let second_fn_id = DeclId::new(101);
    let second = Function {
        id: FunctionId::from(second_fn_id),
        name: duplicate_name,
        params: vec![],
        return_type: RegseqType::Unit,
        body: non_placeholder_body(origin(file, 30)),
        attrs: Attrs::default(),
    };
    model.root_module.free_functions.push(second);
    model
        .source_locations
        .insert(second_fn_id, origin(file, 30));

    let err = verify_phase_1(&model).unwrap_err();
    assert_eq!(err.level, VerifierLevel::Module);
    match err.kind {
        VerifierErrorKind::DuplicateItemName { module, name } => {
            assert_eq!(module, "crate_root");
            assert_eq!(name, "init");
        }
        other => panic!("expected DuplicateItemName, got {other:?}"),
    }
}

// ---------------------------------------------------------------------
// Negative: Phase-1 leakage (THIS IS THE BRIEF'S REQUIRED NEGATIVE
// TEST -- see .harness/spec-batches/regseqmodel-phase-6.md line 156-158)
// ---------------------------------------------------------------------

#[test]
fn rejects_phase_1_model_with_populated_clock_binding() {
    // The brief's required negative test (see
    // `.harness/spec-batches/regseqmodel-phase-6.md:156-158` and the
    // task spec's Test Plan at lines 200-205): take a valid Phase-1
    // model and mutate an existing clock resource's `binding` from
    // `None` to `Some(...)`. This is the proof that the post-Phase-1
    // invariant is enforced by the verifier rather than by
    // convention.
    let mut model = build_minimal_ok();
    assert!(
        verify_phase_1(&model).is_ok(),
        "sanity: starting model is valid"
    );

    let clock = model
        .root_module
        .submodules
        .iter_mut()
        .find(|m| m.kind == ModuleKind::Driver)
        .and_then(|m| m.driver.as_mut())
        .and_then(|d| d.clocks.first_mut())
        .expect("minimal valid model carries exactly one clock resource");
    assert!(
        clock.binding.is_none(),
        "sanity: minimal model starts with binding == None"
    );
    clock.binding = Some(ClockBinding {
        default_frequency: 24_000_000,
    });

    let err = verify_phase_1(&model).unwrap_err();
    assert_eq!(err.level, VerifierLevel::DriverStruct);
    match err.kind {
        VerifierErrorKind::UnexpectedBindingPopulated { resource, slot } => {
            assert_eq!(resource, DeclId::new(MINIMAL_CLOCK_RESOURCE_ID));
            assert_eq!(slot, "clock");
        }
        other => panic!("expected UnexpectedBindingPopulated, got {other:?}"),
    }
}

#[test]
fn rejects_phase_1_model_with_populated_property_bound_value() {
    use regseq_model::ir::LiteralValue;

    let mut model = build_minimal_ok();

    let prop_id = DeclId::new(120);
    let prop_field = model.symbols.intern("clock_frequency");
    let prop_symbol = model.symbols.intern("clock-frequency");
    let file = model.symbols.intern_file("drv/lib.rs");

    let driver_module = model
        .root_module
        .submodules
        .iter_mut()
        .find(|m| m.kind == ModuleKind::Driver)
        .expect("minimal model has a Driver module");
    driver_module
        .driver
        .as_mut()
        .expect("Driver module carries a DriverStruct")
        .properties
        .push(PropertyField {
            id: PropertyId::from(prop_id),
            field_name: prop_field,
            field_type: ScalarType::U32,
            property_id: prop_symbol,
            default_value: None,
            vendor_prefix: None,
            required: true,
            bound_value: Some(LiteralValue::U32(24_000_000)),
        });
    model.source_locations.insert(prop_id, origin(file, 50));

    let err = verify_phase_1(&model).unwrap_err();
    assert_eq!(err.level, VerifierLevel::DriverStruct);
    match err.kind {
        VerifierErrorKind::UnexpectedPropertyValuePopulated { property } => {
            assert_eq!(property, prop_id);
        }
        other => panic!("expected UnexpectedPropertyValuePopulated, got {other:?}"),
    }
}

// ---------------------------------------------------------------------
// Negative: unresolved TypeDefId
// ---------------------------------------------------------------------

#[test]
fn rejects_function_param_referencing_undeclared_type_def_id() {
    let mut model = build_minimal_ok();

    let dangling = TypeDefId::from(DeclId::new(999));
    let arg_name = model.symbols.intern("opaque");

    let func = model
        .root_module
        .free_functions
        .first_mut()
        .expect("minimal model has a free function");
    func.params.push(Param {
        kind: ParamKind::Regular,
        name: arg_name,
        ty: RegseqType::Defined(dangling),
    });

    let err = verify_phase_1(&model).unwrap_err();
    assert_eq!(err.level, VerifierLevel::CrossItem);
    match err.kind {
        VerifierErrorKind::UnresolvedTypeDefId {
            referencing,
            target,
        } => {
            assert_eq!(referencing, DeclId::new(DRIVER_FUNCTION_ID));
            assert_eq!(target, dangling);
        }
        other => panic!("expected UnresolvedTypeDefId, got {other:?}"),
    }
}

// ---------------------------------------------------------------------
// Negative: Phase-5 Pass-1 placeholder body leaked into Phase 1
// ---------------------------------------------------------------------

#[test]
fn rejects_function_with_phase_5_pass_1_placeholder_body() {
    let mut model = build_minimal_ok();

    // Phase 5 Pass 1 produces a skeleton body: exactly one basic block
    // whose only instruction is `TerminatorKind::Unreachable`. Pass 2
    // overwrites it with the real CFG. A function still carrying that
    // skeleton when Phase 1 completes is a boundary violation.
    let func = model
        .root_module
        .free_functions
        .first_mut()
        .expect("minimal model has a free function");
    func.body.basic_blocks = vec![BasicBlock {
        statements: vec![],
        terminator: Terminator {
            kind: TerminatorKind::Unreachable,
        },
    }];

    let err = verify_phase_1(&model).unwrap_err();
    assert_eq!(err.level, VerifierLevel::Module);
    match err.kind {
        VerifierErrorKind::DeclarationFieldUnpopulated { decl_id, field } => {
            assert_eq!(decl_id, DeclId::new(DRIVER_FUNCTION_ID));
            assert_eq!(field, "body");
        }
        other => panic!("expected DeclarationFieldUnpopulated, got {other:?}"),
    }
}

// ---------------------------------------------------------------------
// Negative: unresolved RegisterDescriptorId in a register block
// ---------------------------------------------------------------------

#[test]
fn rejects_register_block_field_referencing_undeclared_descriptor() {
    use regseq_model::identity::RegisterDescriptorId;
    use regseq_model::model::register::{
        AccessKind, RegisterBlock, RegisterBlockAttributes, RegisterBlockField,
    };

    let mut model = build_minimal_ok();
    let file = model.symbols.intern_file("drv/lib.rs");
    let block_name = model.symbols.intern("Regs");
    let endian = model.symbols.intern("little");
    let field_name = model.symbols.intern("dr");

    let block_id = DeclId::new(140);
    let dangling = RegisterDescriptorId::from(DeclId::new(777));

    let driver_module = model
        .root_module
        .submodules
        .iter_mut()
        .find(|m| m.kind == ModuleKind::Driver)
        .expect("minimal model has a driver submodule");
    driver_module.register_blocks.push(RegisterBlock {
        id: RegisterBlockId::from(block_id),
        name: block_name,
        block_attributes: RegisterBlockAttributes {
            endian,
            access_width: 32,
        },
        fields: vec![RegisterBlockField {
            name: field_name,
            access_kind: AccessKind::ReadWrite,
            descriptor: Some(dangling),
            byte_offset: 0,
            array_count: None,
        }],
    });
    model.source_locations.insert(block_id, origin(file, 80));

    let err = verify_phase_1(&model).unwrap_err();
    assert_eq!(err.level, VerifierLevel::CrossItem);
    match err.kind {
        VerifierErrorKind::UnresolvedRegisterDescriptorId {
            referencing,
            target,
        } => {
            assert_eq!(referencing, block_id);
            assert_eq!(target, dangling);
        }
        other => panic!("expected UnresolvedRegisterDescriptorId, got {other:?}"),
    }
}

// ---------------------------------------------------------------------
// Negative: CompilerGenerated source origin for a user-sourced DeclId
// ---------------------------------------------------------------------

#[test]
fn rejects_compiler_generated_source_origin_for_user_sourced_decl() {
    let mut model = build_minimal_ok();
    let target = DeclId::new(DRIVER_TYPE_ID);
    model
        .source_locations
        .insert(target, SourceOrigin::CompilerGenerated);

    let err = verify_phase_1(&model).unwrap_err();
    assert_eq!(err.level, VerifierLevel::CrossItem);
    match err.kind {
        VerifierErrorKind::MissingSourceLocation { decl_id } => {
            assert_eq!(decl_id, target);
        }
        other => panic!("expected MissingSourceLocation, got {other:?}"),
    }
}

// ---------------------------------------------------------------------
// Negative: unresolved RegisterBlockId (MMIO resource)
// ---------------------------------------------------------------------

#[test]
fn rejects_mmio_resource_referencing_undeclared_register_block_id() {
    use regseq_model::model::resources::{MmioDecl, MmioResource};

    let mut model = build_minimal_ok();

    let mmio_res_id = DeclId::new(130);
    let dangling = RegisterBlockId::from(DeclId::new(888));
    let field = model.symbols.intern("regs");
    let mmio_sym = model.symbols.intern("uart_regs");
    let file = model.symbols.intern_file("drv/lib.rs");

    let driver = model
        .root_module
        .submodules
        .iter_mut()
        .find(|m| m.kind == ModuleKind::Driver)
        .and_then(|m| m.driver.as_mut())
        .expect("minimal model has a driver struct");
    driver.mmio.push(MmioResource {
        id: ResourceId::from(mmio_res_id),
        field_name: field,
        shape: Shape::Single,
        decl: MmioDecl {
            mmio_id: mmio_sym,
            register_block: dangling,
        },
        binding: None,
    });
    model.source_locations.insert(mmio_res_id, origin(file, 60));

    let err = verify_phase_1(&model).unwrap_err();
    assert_eq!(err.level, VerifierLevel::CrossItem);
    match err.kind {
        VerifierErrorKind::UnresolvedRegisterBlockId {
            referencing,
            target,
        } => {
            assert_eq!(referencing, mmio_res_id);
            assert_eq!(target, dangling);
        }
        other => panic!("expected UnresolvedRegisterBlockId, got {other:?}"),
    }
}

// ---------------------------------------------------------------------
// Negative: missing SourceLocation
// ---------------------------------------------------------------------

#[test]
fn rejects_model_with_decl_id_missing_from_source_locations() {
    let mut model = build_minimal_ok();

    // Remove the source location for the driver type -- a DeclId
    // that is declared but has no source_locations entry.
    let removed = DeclId::new(DRIVER_TYPE_ID);
    model.source_locations.remove(&removed);

    let err = verify_phase_1(&model).unwrap_err();
    assert_eq!(err.level, VerifierLevel::CrossItem);
    match err.kind {
        VerifierErrorKind::MissingSourceLocation { decl_id } => {
            assert_eq!(decl_id, removed);
        }
        other => panic!("expected MissingSourceLocation, got {other:?}"),
    }
}

// ---------------------------------------------------------------------
// Determinism: identical first error across runs
// ---------------------------------------------------------------------

#[test]
fn verify_phase_1_returns_identical_error_across_two_runs() {
    let mut model = build_minimal_ok();

    // Introduce TWO violations: a duplicate item name AND an
    // unresolved TypeDefId. Fail-fast means we should always see the
    // module-level violation first (module rules run before
    // cross-item), and that violation must be byte-identical across
    // runs.
    let duplicate_name = model.symbols.intern("init");
    let file = model.symbols.intern_file("drv/lib.rs");
    let second_fn_id = DeclId::new(150);
    model.root_module.free_functions.push(Function {
        id: FunctionId::from(second_fn_id),
        name: duplicate_name,
        params: vec![Param {
            kind: ParamKind::Regular,
            name: model.symbols.intern("opaque"),
            ty: RegseqType::Defined(TypeDefId::from(DeclId::new(999))),
        }],
        return_type: RegseqType::Unit,
        body: non_placeholder_body(origin(file, 70)),
        attrs: Attrs::default(),
    });
    model
        .source_locations
        .insert(second_fn_id, origin(file, 70));

    let first = verify_phase_1(&model).unwrap_err();
    let second = verify_phase_1(&model).unwrap_err();
    assert_eq!(first, second, "fail-fast output must be deterministic");
    // Sanity-check the ordering: module-level rule fires before
    // cross-item so the first error is the duplicate.
    assert_eq!(first.level, VerifierLevel::Module);
    assert!(matches!(
        first.kind,
        VerifierErrorKind::DuplicateItemName { .. }
    ));
}
