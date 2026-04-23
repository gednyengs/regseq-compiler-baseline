//! Integration tests for the post-Phase-2 verifier (task
//! regseq-model-56).
//!
//! The happy-path builder ([`build_minimal_phase_2_ok`]) constructs a
//! minimal valid post-join model: one Driver module with a fully-bound
//! clock, MMIO, and one bound property. Each negative test mutates a
//! single dimension of that fixture so failure modes are exercised in
//! isolation.

use std::collections::{BTreeMap, BTreeSet};

use regseq_model::identity::LocalId;
use regseq_model::identity::{
    DeclId, FileId, FunctionId, ModuleId, PropertyId, RegisterBlockId, RegisterDescriptorId,
    ResourceId, SourceLocation, SourceOrigin, SymbolInterner, TypeDefId, UserDataId,
};
use regseq_model::ir::{
    BasicBlock, BasicBlockSourceMap, Body, BodySourceMap, IntrinsicMetadata, LiteralValue,
    LocalDecl, Mutability, Operand, Place, RegseqIntrinsic, RegseqType, ScalarType, Terminator,
    TerminatorKind,
};
use regseq_model::model::driver::{DriverIdentity, DriverStruct};
use regseq_model::model::framework::TypeDefClassification;
use regseq_model::model::function::{Function, Param, ParamKind};
use regseq_model::model::impl_block::ImplBlock;
use regseq_model::model::module::{Module, ModuleKind};
use regseq_model::model::register::{
    AccessKind, RegisterBlock, RegisterBlockAttributes, RegisterBlockField, RegisterDescriptor,
};
use regseq_model::model::regseq_model::RegseqModel;
use regseq_model::model::resources::{
    ClockBinding, ClockDecl, ClockResource, InterruptBinding, InterruptDecl, InterruptResource,
    MmioBinding, MmioDecl, MmioResource, PropertyField, Shape, UserDataField, UserDataKind,
};
use regseq_model::model::type_def::{AttrEntry, AttrValue, Attrs, TypeDef, TypeDefKind};
use regseq_model::verify::{verify_phase_2, IsrViolationKind, VerifierErrorKind, VerifierLevel};

// --- DeclId allocation ----------------------------------------------

const ROOT_MODULE_ID: u32 = 1;
const DRIVER_MODULE_ID: u32 = 2;
const DRIVER_TYPE_ID: u32 = 3;
const DRIVER_FUNCTION_ID: u32 = 4;
const CLOCK_RESOURCE_ID: u32 = 5;
const MMIO_RESOURCE_ID: u32 = 6;
const PROPERTY_ID: u32 = 7;
const REGISTER_BLOCK_ID: u32 = 8;
const REGISTER_DESC_RW_ID: u32 = 9;

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

/// Build a minimal valid post-Phase-2 model.
///
/// One Driver module `drv` with:
/// - Driver type `MyDriver`.
/// - One required clock resource bound to a frequency.
/// - One MMIO resource bound to a base address, referencing a
///   register block with a single ReadWrite descriptor.
/// - One required property with a bound u32 value.
/// - One free function `init` with a real (non-placeholder) body.
fn build_minimal_phase_2_ok() -> RegseqModel {
    let mut symbols = SymbolInterner::new();
    let file = symbols.intern_file("drv/lib.rs");

    let drv_type_sym = symbols.intern("MyDriver");
    let drv_identity_sym = symbols.intern("my-driver");
    let clk_field_sym = symbols.intern("apb_clk");
    let clk_id_sym = symbols.intern("apb");
    let mmio_field_sym = symbols.intern("regs");
    let mmio_id_sym = symbols.intern("uart_regs");
    let block_name_sym = symbols.intern("Pl011Registers");
    let endian_sym = symbols.intern("little");
    let dr_field_sym = symbols.intern("dr");
    let dr_desc_name = symbols.intern("DR_REG");
    let prop_field_sym = symbols.intern("clock_frequency");
    let prop_id_sym = symbols.intern("clock-frequency");
    let init_name = symbols.intern("init");

    let func = Function {
        id: FunctionId::from(DeclId::new(DRIVER_FUNCTION_ID)),
        name: init_name,
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

    let dr_descriptor = RegisterDescriptor {
        id: RegisterDescriptorId::from(DeclId::new(REGISTER_DESC_RW_ID)),
        name: dr_desc_name,
        access_width: 32,
        fields: vec![],
    };

    let register_block = RegisterBlock {
        id: RegisterBlockId::from(DeclId::new(REGISTER_BLOCK_ID)),
        name: block_name_sym,
        block_attributes: RegisterBlockAttributes {
            endian: endian_sym,
            access_width: 32,
        },
        fields: vec![RegisterBlockField {
            name: dr_field_sym,
            access_kind: AccessKind::ReadWrite,
            descriptor: Some(RegisterDescriptorId::from(DeclId::new(REGISTER_DESC_RW_ID))),
            byte_offset: 0,
            array_count: None,
        }],
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
            binding: Some(ClockBinding {
                default_frequency: 24_000_000,
            }),
        }],
        resets: vec![],
        pinctrl: vec![],
        interrupts: vec![],
        mmio: vec![MmioResource {
            id: ResourceId::from(DeclId::new(MMIO_RESOURCE_ID)),
            field_name: mmio_field_sym,
            shape: Shape::Single,
            decl: MmioDecl {
                mmio_id: mmio_id_sym,
                register_block: RegisterBlockId::from(DeclId::new(REGISTER_BLOCK_ID)),
            },
            binding: Some(MmioBinding {
                base: 0x0900_0000,
                size: 0x1000,
            }),
        }],
        sync: vec![],
        properties: vec![PropertyField {
            id: PropertyId::from(DeclId::new(PROPERTY_ID)),
            field_name: prop_field_sym,
            field_type: ScalarType::U32,
            property_id: prop_id_sym,
            default_value: None,
            vendor_prefix: None,
            required: true,
            bound_value: Some(LiteralValue::U32(24_000_000)),
        }],
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
        register_descriptors: vec![dr_descriptor],
        register_blocks: vec![register_block],
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
    source_locations.insert(DeclId::new(MMIO_RESOURCE_ID), origin(file, 50));
    source_locations.insert(DeclId::new(PROPERTY_ID), origin(file, 60));
    source_locations.insert(DeclId::new(REGISTER_BLOCK_ID), origin(file, 70));
    source_locations.insert(DeclId::new(REGISTER_DESC_RW_ID), origin(file, 80));

    RegseqModel {
        symbols,
        source_locations,
        root_module,
        format_version: 1,
    }
}

fn driver_mut(model: &mut RegseqModel) -> &mut DriverStruct {
    model
        .root_module
        .submodules
        .iter_mut()
        .find(|m| m.kind == ModuleKind::Driver)
        .and_then(|m| m.driver.as_mut())
        .expect("minimal phase-2 model has a Driver submodule with a driver struct")
}

fn driver_module_mut(model: &mut RegseqModel) -> &mut Module {
    model
        .root_module
        .submodules
        .iter_mut()
        .find(|m| m.kind == ModuleKind::Driver)
        .expect("minimal phase-2 model has a Driver submodule")
}

// --------------------------------------------------------------------
// Happy path
// --------------------------------------------------------------------

#[test]
fn happy_path_minimal_valid_phase_2_model_passes() {
    let model = build_minimal_phase_2_ok();
    let res = verify_phase_2(&model);
    assert!(res.is_ok(), "expected ok, got {res:?}");
}

// --------------------------------------------------------------------
// Driver-struct level
// --------------------------------------------------------------------

#[test]
fn rejects_resource_with_missing_attribute() {
    let mut model = build_minimal_phase_2_ok();
    let empty_sym = model.symbols.intern("");
    driver_mut(&mut model).clocks[0].decl.clock_id = empty_sym;

    let err = verify_phase_2(&model).unwrap_err();
    assert_eq!(err.level, VerifierLevel::DriverStruct);
    match err.kind {
        VerifierErrorKind::ResourceAttributeIncomplete {
            resource,
            attribute,
        } => {
            assert_eq!(resource, DeclId::new(CLOCK_RESOURCE_ID));
            assert_eq!(attribute, "clock_id");
        }
        other => panic!("expected ResourceAttributeIncomplete, got {other:?}"),
    }
}

#[test]
fn rejects_two_clocks_with_duplicate_id() {
    let mut model = build_minimal_phase_2_ok();
    let file = model.symbols.intern_file("drv/lib.rs");
    let dup_field = model.symbols.intern("apb_clk_dup");
    let dup_id = model.symbols.intern("apb"); // same as the first clock

    let dup_decl_id = DeclId::new(200);
    driver_mut(&mut model).clocks.push(ClockResource {
        id: ResourceId::from(dup_decl_id),
        field_name: dup_field,
        shape: Shape::Optional,
        decl: ClockDecl { clock_id: dup_id },
        binding: None,
    });
    model
        .source_locations
        .insert(dup_decl_id, origin(file, 200));

    let err = verify_phase_2(&model).unwrap_err();
    assert_eq!(err.level, VerifierLevel::DriverStruct);
    match err.kind {
        VerifierErrorKind::DuplicateResourceId { id, kind } => {
            assert_eq!(id, "apb");
            assert_eq!(kind, "clock");
        }
        other => panic!("expected DuplicateResourceId, got {other:?}"),
    }
}

#[test]
fn rejects_user_data_field_with_non_copy_inner_type() {
    let mut model = build_minimal_phase_2_ok();
    let file = model.symbols.intern_file("drv/lib.rs");
    let ud_field = model.symbols.intern("buffer");
    let ud_decl_id = DeclId::new(210);

    // Slice<U8> stands in for `String` -- the inner type is unsized
    // and therefore not Copy.
    driver_mut(&mut model).user_data.push(UserDataField {
        id: UserDataId::from(ud_decl_id),
        field_name: ud_field,
        kind: UserDataKind::Option {
            inner_type: RegseqType::Slice(Box::new(RegseqType::Scalar(ScalarType::U8))),
        },
    });
    model.source_locations.insert(ud_decl_id, origin(file, 210));

    let err = verify_phase_2(&model).unwrap_err();
    assert_eq!(err.level, VerifierLevel::DriverStruct);
    match err.kind {
        VerifierErrorKind::UserDataFieldNotZeroInitOrOptionCopy { field, .. } => {
            assert_eq!(field, ud_decl_id);
        }
        other => panic!("expected UserDataFieldNotZeroInitOrOptionCopy, got {other:?}"),
    }
}

#[test]
fn accepts_user_data_zero_init_scalar_field() {
    let mut model = build_minimal_phase_2_ok();
    let file = model.symbols.intern_file("drv/lib.rs");
    let ud_field = model.symbols.intern("counter");
    let ud_decl_id = DeclId::new(211);

    driver_mut(&mut model).user_data.push(UserDataField {
        id: UserDataId::from(ud_decl_id),
        field_name: ud_field,
        kind: UserDataKind::ZeroInit {
            inner_type: RegseqType::Scalar(ScalarType::U32),
        },
    });
    model.source_locations.insert(ud_decl_id, origin(file, 211));

    verify_phase_2(&model)
        .expect("ZeroInit { u32 } user-data field must pass Phase-2 verification");
}

#[test]
fn accepts_user_data_zero_init_u8_scalar() {
    accept_zero_init(RegseqType::Scalar(ScalarType::U8), 221);
}

#[test]
fn accepts_user_data_zero_init_u16_scalar() {
    accept_zero_init(RegseqType::Scalar(ScalarType::U16), 222);
}

#[test]
fn accepts_user_data_zero_init_u64_scalar() {
    accept_zero_init(RegseqType::Scalar(ScalarType::U64), 223);
}

#[test]
fn accepts_user_data_zero_init_bool() {
    accept_zero_init(RegseqType::Bool, 224);
}

#[test]
fn accepts_user_data_zero_init_array_of_u32() {
    accept_zero_init(
        RegseqType::Array(Box::new(RegseqType::Scalar(ScalarType::U32)), 4),
        225,
    );
}

#[test]
fn accepts_user_data_zero_init_nested_array() {
    accept_zero_init(
        RegseqType::Array(
            Box::new(RegseqType::Array(
                Box::new(RegseqType::Scalar(ScalarType::U16)),
                2,
            )),
            8,
        ),
        226,
    );
}

#[test]
fn accepts_user_data_zero_init_user_defined_copy_struct() {
    // Register a user-defined struct TypeDef whose fields are all
    // Rule-1 accepted primitives (u32/u32), then plug it into the
    // driver as a `ZeroInit { Defined(TypeDefId) }`. The verifier's
    // Rule-1 walker must accept it because every field is
    // whitelisted.
    let mut model = build_minimal_phase_2_ok();
    let file = model.symbols.intern_file("drv/lib.rs");
    let struct_decl_id = DeclId::new(230);
    let struct_type_id = TypeDefId::from(struct_decl_id);
    let struct_name_sym = model.symbols.intern("Counters");
    let rx_sym = model.symbols.intern("rx");
    let tx_sym = model.symbols.intern("tx");
    let type_def = TypeDef {
        id: struct_type_id,
        name: struct_name_sym,
        kind: TypeDefKind::Struct {
            fields: vec![
                regseq_model::model::FieldDef {
                    name: rx_sym,
                    ty: RegseqType::Scalar(ScalarType::U32),
                },
                regseq_model::model::FieldDef {
                    name: tx_sym,
                    ty: RegseqType::Scalar(ScalarType::U32),
                },
            ],
        },
        classification: TypeDefClassification::UserDefined,
        attrs: Attrs::default(),
    };
    let driver_mod = model
        .root_module
        .submodules
        .iter_mut()
        .find(|m| m.kind == ModuleKind::Driver)
        .expect("driver module");
    driver_mod.type_defs.push(type_def);
    model
        .source_locations
        .insert(struct_decl_id, origin(file, 230));

    let ud_field = model.symbols.intern("counters");
    let ud_decl_id = DeclId::new(231);
    driver_mut(&mut model).user_data.push(UserDataField {
        id: UserDataId::from(ud_decl_id),
        field_name: ud_field,
        kind: UserDataKind::ZeroInit {
            inner_type: RegseqType::Defined(struct_type_id),
        },
    });
    model.source_locations.insert(ud_decl_id, origin(file, 231));

    verify_phase_2(&model)
        .expect("ZeroInit with a user-defined Copy struct of Rule-1 fields must pass verification");
}

fn accept_zero_init(inner: RegseqType, decl_id: u32) {
    let mut model = build_minimal_phase_2_ok();
    let file = model.symbols.intern_file("drv/lib.rs");
    let ud_field = model.symbols.intern("accepted_ud");
    let ud_decl_id = DeclId::new(decl_id);
    driver_mut(&mut model).user_data.push(UserDataField {
        id: UserDataId::from(ud_decl_id),
        field_name: ud_field,
        kind: UserDataKind::ZeroInit { inner_type: inner },
    });
    model
        .source_locations
        .insert(ud_decl_id, origin(file, decl_id));
    verify_phase_2(&model).expect("Rule-1 ZeroInit must pass verification");
}

#[test]
fn accepts_user_data_option_copy_field() {
    let mut model = build_minimal_phase_2_ok();
    let file = model.symbols.intern_file("drv/lib.rs");
    let ud_field = model.symbols.intern("slot");
    let ud_decl_id = DeclId::new(212);

    driver_mut(&mut model).user_data.push(UserDataField {
        id: UserDataId::from(ud_decl_id),
        field_name: ud_field,
        kind: UserDataKind::Option {
            inner_type: RegseqType::Scalar(ScalarType::U32),
        },
    });
    model.source_locations.insert(ud_decl_id, origin(file, 212));

    verify_phase_2(&model).expect("Option { u32 } user-data field must pass Phase-2 verification");
}

#[test]
fn rejects_user_data_zero_init_with_signed_scalar_inner() {
    // Rule-1 Stage-1 whitelist admits only u8/u16/u32/u64/bool;
    // signed integers must be rejected even though they satisfy
    // Rust's `Copy` bound.
    let mut model = build_minimal_phase_2_ok();
    let file = model.symbols.intern_file("drv/lib.rs");
    let ud_field = model.symbols.intern("mismatched");
    let ud_decl_id = DeclId::new(214);

    driver_mut(&mut model).user_data.push(UserDataField {
        id: UserDataId::from(ud_decl_id),
        field_name: ud_field,
        kind: UserDataKind::ZeroInit {
            inner_type: RegseqType::Scalar(ScalarType::I32),
        },
    });
    model.source_locations.insert(ud_decl_id, origin(file, 214));

    let err = verify_phase_2(&model).unwrap_err();
    assert_eq!(err.level, VerifierLevel::DriverStruct);
    match err.kind {
        VerifierErrorKind::UserDataFieldNotZeroInitOrOptionCopy { field, .. } => {
            assert_eq!(field, ud_decl_id);
        }
        other => panic!("expected UserDataFieldNotZeroInitOrOptionCopy, got {other:?}"),
    }
}

#[test]
fn rejects_user_data_zero_init_with_option_inner() {
    // `ZeroInit { Option<...> }` is a kind/inner mismatch: Option
    // user-data belongs to the Rule-2 `Option` variant. The Rule-1
    // whitelist must reject it.
    let mut model = build_minimal_phase_2_ok();
    let file = model.symbols.intern_file("drv/lib.rs");
    let ud_field = model.symbols.intern("opt_mismatched");
    let ud_decl_id = DeclId::new(215);

    driver_mut(&mut model).user_data.push(UserDataField {
        id: UserDataId::from(ud_decl_id),
        field_name: ud_field,
        kind: UserDataKind::ZeroInit {
            inner_type: RegseqType::Option(Box::new(RegseqType::Scalar(ScalarType::U32))),
        },
    });
    model.source_locations.insert(ud_decl_id, origin(file, 215));

    let err = verify_phase_2(&model).unwrap_err();
    assert_eq!(err.level, VerifierLevel::DriverStruct);
    assert!(matches!(
        err.kind,
        VerifierErrorKind::UserDataFieldNotZeroInitOrOptionCopy { .. }
    ));
}

#[test]
fn rejects_user_data_zero_init_with_non_copy_inner() {
    let mut model = build_minimal_phase_2_ok();
    let file = model.symbols.intern_file("drv/lib.rs");
    let ud_field = model.symbols.intern("buffer");
    let ud_decl_id = DeclId::new(213);

    driver_mut(&mut model).user_data.push(UserDataField {
        id: UserDataId::from(ud_decl_id),
        field_name: ud_field,
        kind: UserDataKind::ZeroInit {
            inner_type: RegseqType::Slice(Box::new(RegseqType::Scalar(ScalarType::U8))),
        },
    });
    model.source_locations.insert(ud_decl_id, origin(file, 213));

    let err = verify_phase_2(&model).unwrap_err();
    assert_eq!(err.level, VerifierLevel::DriverStruct);
    match err.kind {
        VerifierErrorKind::UserDataFieldNotZeroInitOrOptionCopy { field, .. } => {
            assert_eq!(field, ud_decl_id);
        }
        other => panic!("expected UserDataFieldNotZeroInitOrOptionCopy, got {other:?}"),
    }
}

// --------------------------------------------------------------------
// Body level: unresolved reference
// --------------------------------------------------------------------
//
// The post-Phase-2 verifier runs cross_item (task 55) before the
// body rule (task 56), per the prescribed bucket order in
// `.harness/tasks/regseq-model/56-post-phase2-verifier.md` lines
// 173-188. That means a body referencing an undeclared `FunctionId`
// at a Call terminator surfaces through `verify_phase_2` as
// `UnresolvedFunctionId` (level CrossItem) since cross_item walks
// bodies for FunctionId/TypeDefId/RegisterBlockId.
//
// The body rule's own `UnresolvedBodyReference` variant is still
// load-bearing: it fires for `ConstantId` references (which
// cross_item has no variant for) and is the rule the always-on
// transformation verifier (task 57) will invoke without going
// through verify_phase_2. We exercise both here:
// - Test against `verify_phase_2` for ConstantId (only the body
//   rule sees this id kind).
// - Test against `rules::body::run` directly for FunctionId,
//   exercising the spec's Test Plan body-level case at line 223-225
//   (`body references a FunctionId not in the model; assert
//   UnresolvedBodyReference`).

#[test]
fn body_rule_directly_rejects_unresolved_function_reference() {
    use regseq_model::identity::BasicBlockId;
    use regseq_model::verify::rules::body;

    let mut model = build_minimal_phase_2_ok();
    let dangling = FunctionId::from(DeclId::new(999));

    let func = model
        .root_module
        .free_functions
        .first_mut()
        .expect("init function present");
    func.body.basic_blocks = vec![
        BasicBlock {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::Call {
                    callee: dangling,
                    args: vec![],
                    destination: Place {
                        local: LocalId(0),
                        projection: vec![],
                    },
                    target: Some(BasicBlockId(1)),
                    error_target: None,
                },
            },
        },
        BasicBlock {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::Return,
            },
        },
    ];
    func.body.source_map.blocks = vec![
        BasicBlockSourceMap {
            statements: vec![],
            terminator: SourceOrigin::CompilerGenerated,
        },
        BasicBlockSourceMap {
            statements: vec![],
            terminator: SourceOrigin::CompilerGenerated,
        },
    ];

    // Call rules::body::run directly so cross_item does not mask the
    // body rule. This is the entry point the always-on transformation
    // verifier from task 57 uses, and it is also the contract the
    // spec's Test Plan body-level case targets.
    let err = body::run(&model).unwrap_err();
    assert_eq!(err.level, VerifierLevel::Body);
    match err.kind {
        VerifierErrorKind::UnresolvedBodyReference { body, decl_id } => {
            assert_eq!(body, FunctionId::from(DeclId::new(DRIVER_FUNCTION_ID)));
            assert_eq!(decl_id, dangling.decl_id());
        }
        other => panic!("expected UnresolvedBodyReference, got {other:?}"),
    }
}

#[test]
fn rejects_body_with_unresolved_constant_reference() {
    use regseq_model::identity::{BasicBlockId, ConstantId};
    use regseq_model::ir::{Rvalue, Statement, StatementKind};

    let mut model = build_minimal_phase_2_ok();
    let dangling = ConstantId::from(DeclId::new(999));

    let func = model
        .root_module
        .free_functions
        .first_mut()
        .expect("init function present");
    func.body.basic_blocks = vec![
        BasicBlock {
            statements: vec![Statement {
                kind: StatementKind::Assign(
                    Place {
                        local: LocalId(0),
                        projection: vec![],
                    },
                    Rvalue::Use(Operand::Constant(dangling)),
                ),
            }],
            terminator: Terminator {
                kind: TerminatorKind::Goto(BasicBlockId(1)),
            },
        },
        BasicBlock {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::Return,
            },
        },
    ];
    func.body.source_map.blocks = vec![
        BasicBlockSourceMap {
            statements: vec![SourceOrigin::CompilerGenerated],
            terminator: SourceOrigin::CompilerGenerated,
        },
        BasicBlockSourceMap {
            statements: vec![],
            terminator: SourceOrigin::CompilerGenerated,
        },
    ];

    let err = verify_phase_2(&model).unwrap_err();
    assert_eq!(err.level, VerifierLevel::Body);
    match err.kind {
        VerifierErrorKind::UnresolvedBodyReference { body, decl_id } => {
            assert_eq!(body, FunctionId::from(DeclId::new(DRIVER_FUNCTION_ID)));
            assert_eq!(decl_id, dangling.decl_id());
        }
        other => panic!("expected UnresolvedBodyReference, got {other:?}"),
    }
}

// --------------------------------------------------------------------
// Body level: read-only register write
// --------------------------------------------------------------------

#[test]
fn rejects_intrinsic_write_to_read_only_register() {
    let mut model = build_minimal_phase_2_ok();

    // Re-classify the existing single block field to ReadOnly.
    let drv_module = driver_module_mut(&mut model);
    drv_module.register_blocks[0].fields[0].access_kind = AccessKind::ReadOnly;

    let func = model
        .root_module
        .free_functions
        .first_mut()
        .expect("init function present");
    func.body.basic_blocks = vec![
        BasicBlock {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::IntrinsicCall {
                    intrinsic: RegseqIntrinsic::RegisterWrite,
                    args: vec![],
                    metadata: Some(IntrinsicMetadata::Register {
                        register_block: RegisterBlockId::from(DeclId::new(REGISTER_BLOCK_ID)),
                        register_offset: 0,
                        access_width: 32,
                        field: None,
                    }),
                    destination: Place {
                        local: LocalId(0),
                        projection: vec![],
                    },
                    target: regseq_model::identity::BasicBlockId(1),
                    error_target: None,
                },
            },
        },
        BasicBlock {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::Return,
            },
        },
    ];
    func.body.source_map.blocks = vec![
        BasicBlockSourceMap {
            statements: vec![],
            terminator: SourceOrigin::CompilerGenerated,
        },
        BasicBlockSourceMap {
            statements: vec![],
            terminator: SourceOrigin::CompilerGenerated,
        },
    ];

    let err = verify_phase_2(&model).unwrap_err();
    assert_eq!(err.level, VerifierLevel::Body);
    match err.kind {
        VerifierErrorKind::ReadOnlyRegisterWrite { body, register } => {
            assert_eq!(body, FunctionId::from(DeclId::new(DRIVER_FUNCTION_ID)));
            assert_eq!(
                register,
                RegisterDescriptorId::from(DeclId::new(REGISTER_DESC_RW_ID))
            );
        }
        other => panic!("expected ReadOnlyRegisterWrite, got {other:?}"),
    }
}

// --------------------------------------------------------------------
// Body level: ISR-context hook plumbing
// --------------------------------------------------------------------

#[test]
fn isr_context_hook_fires_when_synthetic_forbidden_set_is_non_empty() {
    use regseq_model::verify::rules::body::run_with_isr_forbidden;

    let mut model = build_minimal_phase_2_ok();

    // Add an interrupt resource bound to the existing init function
    // (now classified as ISR).
    let file = model.symbols.intern_file("drv/lib.rs");
    let irq_field = model.symbols.intern("uart_irq");
    let irq_id = model.symbols.intern("uart");
    let irq_decl_id = DeclId::new(220);

    driver_mut(&mut model).interrupts.push(InterruptResource {
        id: ResourceId::from(irq_decl_id),
        field_name: irq_field,
        shape: Shape::Single,
        decl: InterruptDecl {
            interrupt_id: irq_id,
        },
        binding: Some(InterruptBinding {
            irq_number: 37,
            trigger_type: None,
            trigger_polarity: None,
            shared: false,
            isr_target: Some(FunctionId::from(DeclId::new(DRIVER_FUNCTION_ID))),
        }),
    });
    model
        .source_locations
        .insert(irq_decl_id, origin(file, 220));

    // Mark the init function ISR-classified so the hook recognizes it.
    let func = model
        .root_module
        .free_functions
        .first_mut()
        .expect("init function present");
    func.attrs = Attrs {
        entries: vec![AttrEntry {
            key: "rseq::isr".to_string(),
            value: AttrValue::Unit,
        }],
    };

    // Replace the body with one that calls the BusyWait intrinsic.
    func.body.basic_blocks = vec![
        BasicBlock {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::IntrinsicCall {
                    intrinsic: RegseqIntrinsic::BusyWait,
                    args: vec![],
                    metadata: None,
                    destination: Place {
                        local: LocalId(0),
                        projection: vec![],
                    },
                    target: regseq_model::identity::BasicBlockId(1),
                    error_target: None,
                },
            },
        },
        BasicBlock {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::Return,
            },
        },
    ];
    func.body.source_map.blocks = vec![
        BasicBlockSourceMap {
            statements: vec![],
            terminator: SourceOrigin::CompilerGenerated,
        },
        BasicBlockSourceMap {
            statements: vec![],
            terminator: SourceOrigin::CompilerGenerated,
        },
    ];

    // Default verify_phase_2 with empty forbidden set: still passes.
    assert!(verify_phase_2(&model).is_ok(), "default policy is empty");

    // Inject a synthetic non-empty forbidden set: the hook fires.
    let err = run_with_isr_forbidden(&model, &[RegseqIntrinsic::BusyWait]).expect_err("hook fires");
    assert_eq!(err.level, VerifierLevel::Body);
    match err.kind {
        VerifierErrorKind::IsrContextViolation { body, kind } => {
            assert_eq!(body, FunctionId::from(DeclId::new(DRIVER_FUNCTION_ID)));
            assert_eq!(kind, IsrViolationKind::Sleep);
        }
        other => panic!("expected IsrContextViolation, got {other:?}"),
    }
}

// --------------------------------------------------------------------
// Phase-2 boundary
// --------------------------------------------------------------------

#[test]
fn rejects_required_clock_with_no_binding() {
    let mut model = build_minimal_phase_2_ok();
    driver_mut(&mut model).clocks[0].binding = None;

    let err = verify_phase_2(&model).unwrap_err();
    assert_eq!(err.level, VerifierLevel::DriverStruct);
    match err.kind {
        VerifierErrorKind::RequiredBindingMissing { resource, slot } => {
            assert_eq!(resource, DeclId::new(CLOCK_RESOURCE_ID));
            assert_eq!(slot, "clock");
        }
        other => panic!("expected RequiredBindingMissing, got {other:?}"),
    }
}

#[test]
fn accepts_optional_reset_resource_with_no_binding() {
    use regseq_model::model::resources::{ResetDecl, ResetResource};
    let mut model = build_minimal_phase_2_ok();

    let file = model.symbols.intern_file("drv/lib.rs");
    let rst_field = model.symbols.intern("optional_rst");
    let rst_id = model.symbols.intern("opt");
    let rst_decl_id = DeclId::new(230);

    driver_mut(&mut model).resets.push(ResetResource {
        id: ResourceId::from(rst_decl_id),
        field_name: rst_field,
        shape: Shape::Optional,
        decl: ResetDecl { reset_id: rst_id },
        binding: None,
    });
    model
        .source_locations
        .insert(rst_decl_id, origin(file, 230));

    assert!(
        verify_phase_2(&model).is_ok(),
        "optional reset with no binding must verify"
    );
}

// --------------------------------------------------------------------
// Phase-2 cross-item: ISR target validation
// --------------------------------------------------------------------

fn add_interrupt_with_isr_target(model: &mut RegseqModel, target: FunctionId) -> DeclId {
    let file = model.symbols.intern_file("drv/lib.rs");
    let irq_field = model.symbols.intern("uart_irq_x");
    let irq_id = model.symbols.intern("uart_x");
    let irq_decl_id = DeclId::new(240);

    driver_mut(model).interrupts.push(InterruptResource {
        id: ResourceId::from(irq_decl_id),
        field_name: irq_field,
        shape: Shape::Single,
        decl: InterruptDecl {
            interrupt_id: irq_id,
        },
        binding: Some(InterruptBinding {
            irq_number: 41,
            trigger_type: None,
            trigger_polarity: None,
            shared: false,
            isr_target: Some(target),
        }),
    });
    model
        .source_locations
        .insert(irq_decl_id, origin(file, 240));
    irq_decl_id
}

#[test]
fn rejects_interrupt_with_unknown_isr_target() {
    let mut model = build_minimal_phase_2_ok();
    let dangling = FunctionId::from(DeclId::new(999));
    let irq_decl_id = add_interrupt_with_isr_target(&mut model, dangling);

    let err = verify_phase_2(&model).unwrap_err();
    assert_eq!(err.level, VerifierLevel::CrossItem);
    match err.kind {
        VerifierErrorKind::UnknownIsrTarget { interrupt, target } => {
            assert_eq!(interrupt, irq_decl_id);
            assert_eq!(target, dangling);
        }
        other => panic!("expected UnknownIsrTarget, got {other:?}"),
    }
}

#[test]
fn rejects_interrupt_bound_to_non_isr_function() {
    let mut model = build_minimal_phase_2_ok();
    // The init function exists but is NOT ISR-classified.
    let target = FunctionId::from(DeclId::new(DRIVER_FUNCTION_ID));
    let irq_decl_id = add_interrupt_with_isr_target(&mut model, target);

    let err = verify_phase_2(&model).unwrap_err();
    assert_eq!(err.level, VerifierLevel::CrossItem);
    match err.kind {
        VerifierErrorKind::NonIsrFunctionBoundAsIsr {
            interrupt,
            target: tgt,
        } => {
            assert_eq!(interrupt, irq_decl_id);
            assert_eq!(tgt, target);
        }
        other => panic!("expected NonIsrFunctionBoundAsIsr, got {other:?}"),
    }
}

#[test]
fn accepts_interrupt_bound_to_isr_classified_function() {
    let mut model = build_minimal_phase_2_ok();

    // Add an ISR-classified method via a driver impl block on
    // MyDriver, and route a new interrupt at it.
    let file = model.symbols.intern_file("drv/lib.rs");
    let isr_fn_id = DeclId::new(250);
    let isr_name = model.symbols.intern("on_irq");
    let self_name = model.symbols.intern("self");
    let driver_type = TypeDefId::from(DeclId::new(DRIVER_TYPE_ID));
    let receiver_ty = RegseqType::Ref(
        Box::new(RegseqType::Defined(driver_type)),
        Mutability::Mutable,
    );

    let isr_function = Function {
        id: FunctionId::from(isr_fn_id),
        name: isr_name,
        params: vec![Param {
            kind: ParamKind::Receiver,
            name: self_name,
            ty: receiver_ty,
        }],
        return_type: RegseqType::Unit,
        body: non_placeholder_body(origin(file, 250)),
        attrs: Attrs {
            entries: vec![AttrEntry {
                key: "rseq::isr".to_string(),
                value: AttrValue::Unit,
            }],
        },
    };

    driver_module_mut(&mut model)
        .driver_impl_blocks
        .push(ImplBlock {
            target_type: driver_type,
            trait_name: None,
            methods: vec![isr_function],
            associated_constants: vec![],
        });
    model.source_locations.insert(isr_fn_id, origin(file, 250));

    let _ = add_interrupt_with_isr_target(&mut model, FunctionId::from(isr_fn_id));

    let res = verify_phase_2(&model);
    assert!(res.is_ok(), "valid ISR binding should verify; got {res:?}");
}
