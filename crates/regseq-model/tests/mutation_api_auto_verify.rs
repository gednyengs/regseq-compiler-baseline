//! Voided pending MonoRust + RegseqModel redesign.
//! See docs/discussions/compiler-refactoring/test-audit.md for the classification rationale
//! and the retirement criterion.
#![cfg(feature = "post-monorust-tests")]

//! Integration tests for the always-on verification contract of the
//! [`MutationGuard`] API (task `regseq-model-57`).
//!
//! Every test here opens a mutation, performs one targeted change, and
//! asserts either that [`MutationGuard::commit`] succeeds OR that it
//! returns a specific [`VerifierError`] AND the model was rolled back.
//! Tests never poke model fields directly after `begin_mutation` --
//! the whole point of this task is that the guarded surface is the
//! only commit path.

use std::collections::{BTreeMap, BTreeSet};

use regseq_model::identity::{
    DeclId, FileId, FunctionId, LocalId, ModuleId, PropertyId, RegisterBlockId,
    RegisterDescriptorId, ResourceId, SourceLocation, SourceOrigin, SymbolInterner, TypeDefId,
};
use regseq_model::ir::{
    BasicBlock, BasicBlockSourceMap, Body, BodySourceMap, LiteralValue, LocalDecl, Operand, Place,
    RegseqType, Rvalue, ScalarType, Statement, StatementKind, Terminator, TerminatorKind,
};
use regseq_model::model::driver::{DriverIdentity, DriverStruct};
use regseq_model::model::framework::TypeDefClassification;
use regseq_model::model::function::Function;
use regseq_model::model::module::{Module, ModuleKind};
use regseq_model::model::mutation::{MutationGuard, MutationPhase};
use regseq_model::model::register::{
    AccessKind, RegisterBlock, RegisterBlockAttributes, RegisterBlockField, RegisterDescriptor,
};
use regseq_model::model::regseq_model::RegseqModel;
use regseq_model::model::resources::{
    ClockBinding, ClockDecl, ClockResource, MmioBinding, MmioDecl, MmioResource, PropertyField,
    Shape,
};
use regseq_model::model::type_def::{Attrs, TypeDef, TypeDefKind};
use regseq_model::verify::{VerifierErrorKind, VerifierLevel};

// --- Fixture DeclId allocation --------------------------------------

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

/// Build a minimal valid Phase-1 model.
///
/// Same shape as the Phase-1 verifier tests: one Driver module with
/// one unbound clock resource, one `init` free function on the root.
fn build_minimal_phase_1_ok() -> RegseqModel {
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
    source_locations.insert(DeclId::new(CLOCK_RESOURCE_ID), origin(file, 40));

    RegseqModel {
        symbols,
        source_locations,
        root_module,
        format_version: 1,
    }
}

/// Build a minimal valid post-Phase-2 model.
///
/// Same shape as the Phase-2 verifier tests: all bindings and
/// property values populated, one register block with a single
/// ReadWrite descriptor, one `init` function on the root.
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

/// Deterministic signature of a [`RegseqModel`] suitable for rollback
/// byte-equality assertions.
///
/// The [`RegseqModel`] root struct does not implement `Serialize` (its
/// [`SymbolInterner`] field is compared by identity rather than by
/// content), so we cannot run the model through `serde_json`. Instead
/// we use `{:#?}` as a stable stand-in and project over every field
/// on the root struct -- including `symbols` -- so rollback must
/// restore the model byte-for-byte across the full public surface.
///
/// Stability comes from two facts:
/// - All collections on the root render deterministically:
///   `source_locations` is a [`BTreeMap`], the module tree uses
///   `Vec`/[`BTreeSet`]/`Option`, and [`SymbolInterner`] itself is
///   backed by a `Vec<String>` plus a `HashMap<String, u32>`.
/// - The snapshot captured by [`RegseqModel::begin_mutation`] is a
///   plain `Clone` of the live model, which preserves the interner's
///   internal bucket layout and therefore renders the same under
///   `{:#?}`.
fn model_signature(model: &RegseqModel) -> String {
    format!(
        "{:#?}|{:#?}|{:#?}|{}",
        model.symbols, model.root_module, model.source_locations, model.format_version
    )
}

// ---------------------------------------------------------------------
// (1) Commit success path -- Phase-1
// ---------------------------------------------------------------------

#[test]
fn commit_success_phase_1_add_benign_typedef() {
    let mut model = build_minimal_phase_1_ok();
    let file = model.symbols.intern_file("drv/lib.rs");
    let new_tydef_id = DeclId::new(110);
    let counter_sym = model.symbols.intern("Counter");
    model
        .source_locations
        .insert(new_tydef_id, origin(file, 110));

    let root_id = ModuleId::from(DeclId::new(ROOT_MODULE_ID));
    let mut guard = model.begin_mutation(MutationPhase::Phase1);
    guard.modify_module(root_id, |m: &mut Module| {
        m.type_defs.push(TypeDef {
            id: TypeDefId::from(new_tydef_id),
            name: counter_sym,
            kind: TypeDefKind::Struct { fields: vec![] },
            classification: TypeDefClassification::UserDefined,
            attrs: Attrs::default(),
        });
    });
    guard.commit().expect("benign typedef commit succeeds");

    // Mutation stuck: the new typedef is on the root module.
    assert!(
        model
            .root_module
            .type_defs
            .iter()
            .any(|t| t.id == TypeDefId::from(new_tydef_id)),
        "committed typedef must be present"
    );
}

// ---------------------------------------------------------------------
// (2) Commit rejection path -- module level (MultipleDriverModules)
// ---------------------------------------------------------------------

#[test]
fn commit_rejects_multiple_driver_modules_and_rolls_back() {
    let mut model = build_minimal_phase_1_ok();

    // Record the source-location for the second driver BEFORE
    // begin_mutation so it is part of the snapshot and survives
    // rollback along with the rest of the model. Then capture the
    // pre-mutation signature -- we compare against this exact state
    // after the commit fails.
    let second_id = DeclId::new(200);
    let file = model.symbols.intern_file("drv/lib.rs");
    let drv2_name = model.symbols.intern("drv2");
    model.source_locations.insert(second_id, origin(file, 200));
    let pre = model_signature(&model);

    let root_id = ModuleId::from(DeclId::new(ROOT_MODULE_ID));
    let mut guard = model.begin_mutation(MutationPhase::Phase1);
    guard.modify_module(root_id, |m: &mut Module| {
        m.submodules.push(Module {
            id: ModuleId::from(second_id),
            name: drv2_name,
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
        });
    });

    let err = guard.commit().expect_err("two driver modules must reject");
    assert_eq!(err.level, VerifierLevel::Module);
    assert_eq!(
        err.kind,
        VerifierErrorKind::MultipleDriverModules { count: 2 }
    );

    assert_eq!(
        model.root_module.submodules.len(),
        1,
        "rollback must remove the injected second driver module"
    );
    assert_eq!(
        model_signature(&model),
        pre,
        "model rolled back to pre-mutation state"
    );
}

// ---------------------------------------------------------------------
// (3) Commit rejection path -- driver struct level
// ---------------------------------------------------------------------

#[test]
fn commit_rejects_incomplete_resource_attribute_and_rolls_back() {
    let mut model = build_minimal_phase_2_ok();
    let pre_clock_id_sym = model
        .root_module
        .submodules
        .iter()
        .find(|m| m.kind == ModuleKind::Driver)
        .and_then(|m| m.driver.as_ref())
        .expect("minimal model has driver")
        .clocks[0]
        .decl
        .clock_id;

    let empty_sym = model.symbols.intern("");
    let mut guard = model.begin_mutation(MutationPhase::Phase2);
    guard.modify_driver_struct(|d: &mut DriverStruct| {
        d.clocks[0].decl.clock_id = empty_sym;
    });
    let err = guard.commit().expect_err("empty clock_id must be rejected");

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

    // Rollback: clock_id is back to the pre-mutation symbol.
    let post_clock_id_sym = model
        .root_module
        .submodules
        .iter()
        .find(|m| m.kind == ModuleKind::Driver)
        .and_then(|m| m.driver.as_ref())
        .expect("minimal model still has driver after rollback")
        .clocks[0]
        .decl
        .clock_id;
    assert_eq!(
        post_clock_id_sym, pre_clock_id_sym,
        "rollback restored clock_id to the pre-mutation symbol"
    );
}

// ---------------------------------------------------------------------
// (4) Commit rejection path -- body level
// ---------------------------------------------------------------------
//
// The post-Phase-2 verifier runs `cross_item` before the body rule
// (see `.harness/tasks/regseq-model/56-post-phase2-verifier.md`
// lines 173-188). That means a body referencing an undeclared
// `FunctionId` surfaces first through `cross_item` as
// `UnresolvedFunctionId` at the CrossItem level, not as
// `UnresolvedBodyReference` at the Body level.
//
// The only id kind uniquely owned by the body rule is `ConstantId`
// (cross_item does not walk Operand::Constant). We use it here so the
// Body-level test exercises the body rule directly through commit.

#[test]
fn commit_rejects_unresolved_body_constant_reference_and_rolls_back() {
    use regseq_model::identity::{BasicBlockId, ConstantId};

    let mut model = build_minimal_phase_2_ok();
    let pre = model_signature(&model);
    let dangling = ConstantId::from(DeclId::new(999));
    let target_fn = FunctionId::from(DeclId::new(DRIVER_FUNCTION_ID));

    let mut guard = model.begin_mutation(MutationPhase::Phase2);
    guard.modify_body(target_fn, |body: &mut Body| {
        body.basic_blocks = vec![
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
        body.source_map.blocks = vec![
            BasicBlockSourceMap {
                statements: vec![SourceOrigin::CompilerGenerated],
                terminator: SourceOrigin::CompilerGenerated,
            },
            BasicBlockSourceMap {
                statements: vec![],
                terminator: SourceOrigin::CompilerGenerated,
            },
        ];
    });

    let err = guard
        .commit()
        .expect_err("unresolved ConstantId must reject at body level");
    assert_eq!(err.level, VerifierLevel::Body);
    match err.kind {
        VerifierErrorKind::UnresolvedBodyReference { body, decl_id } => {
            assert_eq!(body, target_fn);
            assert_eq!(decl_id, dangling.decl_id());
        }
        other => panic!("expected UnresolvedBodyReference, got {other:?}"),
    }

    assert_eq!(
        model_signature(&model),
        pre,
        "rollback must restore the model byte-for-byte"
    );
}

/// The spec's Test Plan body-level case (task 57 line 176-178) wording
/// calls out a `FunctionId` reference. `verify_phase_2` runs
/// `cross_item` before the body rule (see the Phase-2 verifier
/// rustdoc), so a body referencing a dangling `FunctionId` surfaces
/// through the cross-item bucket as `UnresolvedFunctionId` (CrossItem
/// level). This is still a body-mutation-via-commit rejection with
/// full rollback -- the ordering means the committed error kind
/// differs from the spec's literal wording, but the routing property
/// the spec cares about is preserved (commit rejects, guard rolls
/// back). The `ConstantId` test above exercises the body rule's
/// unique id kind.
#[test]
fn commit_rejects_unresolved_body_function_reference_and_rolls_back() {
    use regseq_model::identity::BasicBlockId;

    let mut model = build_minimal_phase_2_ok();
    let pre = model_signature(&model);
    let dangling_fn = FunctionId::from(DeclId::new(888));
    let target_fn = FunctionId::from(DeclId::new(DRIVER_FUNCTION_ID));

    let mut guard = model.begin_mutation(MutationPhase::Phase2);
    guard.modify_body(target_fn, |body: &mut Body| {
        body.basic_blocks = vec![
            BasicBlock {
                statements: vec![],
                terminator: Terminator {
                    kind: TerminatorKind::Call {
                        callee: dangling_fn,
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
        body.source_map.blocks = vec![
            BasicBlockSourceMap {
                statements: vec![],
                terminator: SourceOrigin::CompilerGenerated,
            },
            BasicBlockSourceMap {
                statements: vec![],
                terminator: SourceOrigin::CompilerGenerated,
            },
        ];
    });

    let err = guard
        .commit()
        .expect_err("body referencing a dangling FunctionId must reject on commit");
    assert_eq!(err.level, VerifierLevel::CrossItem);
    match err.kind {
        VerifierErrorKind::UnresolvedFunctionId {
            referencing,
            target,
        } => {
            assert_eq!(referencing, target_fn.decl_id());
            assert_eq!(target, dangling_fn);
        }
        other => panic!("expected UnresolvedFunctionId, got {other:?}"),
    }

    assert_eq!(
        model_signature(&model),
        pre,
        "rollback must restore the model byte-for-byte"
    );
}

// ---------------------------------------------------------------------
// (5) Commit rejection path -- cross-item level (brief's explicit
// requirement)
// ---------------------------------------------------------------------

#[test]
fn commit_rejects_typedef_with_unresolved_reference_and_rolls_back_byte_equal() {
    // The Phase-6 brief requires a cross-item-level rejection test
    // that proves the cross-item bucket is routed (not just the
    // Module or DriverStruct buckets) AND that rollback is
    // byte-equal (see `.harness/spec-batches/regseqmodel-phase-6.md`
    // lines 163-166 and task spec lines 127-143).
    //
    // The task spec's Implementation Notes paragraph (lines 127-136)
    // prescribes "insert a new TypeDef that references a nonexistent
    // RegisterBlockId" and asserts `UnresolvedRegisterBlockId`. In
    // the current model, TypeDefs carry `RegseqType`-valued field
    // types and do not reference `RegisterBlockId` at all -- the
    // cross_item bucket resolves `TypeDefId`/`FunctionId`/
    // `RegisterBlockId` separately, and the kind produced by walking
    // a TypeDef's fields is `UnresolvedTypeDefId`. We preserve the
    // spec's structural intent (insert a TypeDef, verify cross_item
    // routes the error, prove byte-equal rollback) while asserting
    // the kind the cross_item rule actually produces for a TypeDef
    // mutation.
    //
    // A second, MMIO-based test below covers the spec's named error
    // kind `UnresolvedRegisterBlockId` explicitly.
    let mut model = build_minimal_phase_1_ok();

    // Register the source-location for the injected TypeDef BEFORE
    // begin_mutation so the snapshot captures it; that way the
    // post-rollback model is byte-equal to the pre-mutation signature
    // recorded here, and cross_item's reference-resolution rule fires
    // ahead of the source-location-coverage rule.
    let file = model.symbols.intern_file("drv/lib.rs");
    let new_tydef_id = DeclId::new(150);
    let new_tydef_sym = model.symbols.intern("DanglingUser");
    let dangling_target = TypeDefId::from(DeclId::new(901));
    model
        .source_locations
        .insert(new_tydef_id, origin(file, 150));
    let pre = model_signature(&model);

    let root_id = ModuleId::from(DeclId::new(ROOT_MODULE_ID));
    let mut guard = model.begin_mutation(MutationPhase::Phase1);
    guard.modify_module(root_id, |m: &mut Module| {
        use regseq_model::model::type_def::FieldDef;
        m.type_defs.push(TypeDef {
            id: TypeDefId::from(new_tydef_id),
            name: new_tydef_sym,
            kind: TypeDefKind::Struct {
                fields: vec![FieldDef {
                    name: new_tydef_sym,
                    ty: RegseqType::Defined(dangling_target),
                }],
            },
            classification: TypeDefClassification::UserDefined,
            attrs: Attrs::default(),
        });
    });

    let err = guard
        .commit()
        .expect_err("typedef with unresolved reference must reject at cross-item level");
    assert_eq!(err.level, VerifierLevel::CrossItem);
    match err.kind {
        VerifierErrorKind::UnresolvedTypeDefId {
            referencing,
            target,
        } => {
            assert_eq!(referencing, new_tydef_id);
            assert_eq!(target, dangling_target);
        }
        other => panic!("expected UnresolvedTypeDefId, got {other:?}"),
    }

    assert_eq!(
        model_signature(&model),
        pre,
        "rollback must restore the model byte-for-byte"
    );
}

#[test]
fn commit_rejects_mmio_referencing_unknown_register_block_and_rolls_back_byte_equal() {
    // Companion test to the one above: covers the error kind
    // `UnresolvedRegisterBlockId` named in the task spec's Implementation
    // Notes (lines 134-136). Inserts a new MMIO resource on the driver
    // struct that references an undeclared RegisterBlockId.
    let mut model = build_minimal_phase_1_ok();

    let file = model.symbols.intern_file("drv/lib.rs");
    let new_mmio_res_id = DeclId::new(130);
    let dangling_block = RegisterBlockId::from(DeclId::new(888));
    let new_mmio_field = model.symbols.intern("regs");
    let new_mmio_sym = model.symbols.intern("uart_regs");
    model
        .source_locations
        .insert(new_mmio_res_id, origin(file, 130));
    let pre = model_signature(&model);

    let mut guard = model.begin_mutation(MutationPhase::Phase1);
    guard.modify_driver_struct(|d: &mut DriverStruct| {
        d.mmio.push(MmioResource {
            id: ResourceId::from(new_mmio_res_id),
            field_name: new_mmio_field,
            shape: Shape::Single,
            decl: MmioDecl {
                mmio_id: new_mmio_sym,
                register_block: dangling_block,
            },
            binding: None,
        });
    });

    let err = guard
        .commit()
        .expect_err("unresolved RegisterBlockId must reject at cross-item level");
    assert_eq!(err.level, VerifierLevel::CrossItem);
    match err.kind {
        VerifierErrorKind::UnresolvedRegisterBlockId {
            referencing,
            target,
        } => {
            assert_eq!(referencing, new_mmio_res_id);
            assert_eq!(target, dangling_block);
        }
        other => panic!("expected UnresolvedRegisterBlockId, got {other:?}"),
    }

    assert_eq!(
        model_signature(&model),
        pre,
        "rollback must restore the model byte-for-byte"
    );
}

// ---------------------------------------------------------------------
// (6) Abort without commit -- dropping the guard rolls back
// ---------------------------------------------------------------------

#[test]
fn drop_without_commit_rolls_back() {
    let mut model = build_minimal_phase_1_ok();
    let pre = model_signature(&model);

    let file = model.symbols.intern_file("drv/lib.rs");
    let new_tydef_id = DeclId::new(140);
    let name_sym = model.symbols.intern("Aborted");
    model
        .source_locations
        .insert(new_tydef_id, origin(file, 140));
    let pre_with_source_loc = model_signature(&model);

    let root_id = ModuleId::from(DeclId::new(ROOT_MODULE_ID));
    {
        let mut guard: MutationGuard<'_> = model.begin_mutation(MutationPhase::Phase1);
        guard.modify_module(root_id, |m: &mut Module| {
            m.type_defs.push(TypeDef {
                id: TypeDefId::from(new_tydef_id),
                name: name_sym,
                kind: TypeDefKind::Struct { fields: vec![] },
                classification: TypeDefClassification::UserDefined,
                attrs: Attrs::default(),
            });
        });
        // Drop the guard without calling commit.
    }

    // The guard was dropped, so the type_def mutation was reverted.
    // The source-location insertion was outside the guard, so it
    // survives.
    assert_eq!(
        model_signature(&model),
        pre_with_source_loc,
        "drop without commit must roll back in-guard mutations"
    );

    // Sanity: the in-guard mutation is gone but the pre-guard mutation
    // (source_locations entry) is not. The pre-guard snapshot we took
    // first (without the source_locations entry) is strictly less than
    // post-drop model, proving `pre_with_source_loc` is the correct
    // checkpoint.
    assert_ne!(
        pre, pre_with_source_loc,
        "pre-mutation setup must actually have added the source-location"
    );
}

// ---------------------------------------------------------------------
// (7) Determinism: serialize-before / abort / serialize-after is
// byte-identical
// ---------------------------------------------------------------------

#[test]
fn abort_is_byte_identical_determinism() {
    let mut model = build_minimal_phase_2_ok();

    // Intern the symbol we will use during the guarded mutation
    // BEFORE recording the pre-mutation signature. If we interned it
    // later, the interner pool would be part of the snapshot that
    // begin_mutation captured, and after rollback the interner would
    // include the new entry while `pre` would not -- surfacing as a
    // false determinism failure.
    let empty_sym = model.symbols.intern("");
    let pre = model_signature(&model);

    {
        let mut guard = model.begin_mutation(MutationPhase::Phase2);
        guard.modify_driver_struct(|d: &mut DriverStruct| {
            // Strip the MMIO binding and replace the mmio_id with an
            // empty symbol. Both mutations would be rejected on
            // commit; we abort via drop instead and assert the model
            // is byte-identical to pre-mutation.
            d.mmio[0].binding = None;
            d.mmio[0].decl.mmio_id = empty_sym;
        });
        // Intentionally no commit -- drop rolls back.
    }

    let post = model_signature(&model);
    assert_eq!(pre, post, "open+abort must produce a byte-identical model");
}

// ---------------------------------------------------------------------
// Additional coverage: insert_binding and insert_property_value route
// through the verifier on commit.
// ---------------------------------------------------------------------

#[test]
fn insert_binding_commits_on_phase_2_when_valid() {
    use regseq_model::model::mutation::ResourceBinding;

    // Start from a Phase-2 model whose clock binding is stripped;
    // re-insert it through the guard and assert commit succeeds.
    let mut model = build_minimal_phase_2_ok();

    {
        let mut guard = model.begin_mutation(MutationPhase::Phase2);
        // First remove the binding.
        guard.modify_driver_struct(|d: &mut DriverStruct| {
            d.clocks[0].binding = None;
        });
        // Then put it back via insert_binding.
        guard.insert_binding(
            DeclId::new(CLOCK_RESOURCE_ID),
            ResourceBinding::Clock(ClockBinding {
                default_frequency: 48_000_000,
            }),
        );
        guard.commit().expect("re-inserted binding is valid");
    }

    let post_freq = model
        .root_module
        .submodules
        .iter()
        .find(|m| m.kind == ModuleKind::Driver)
        .and_then(|m| m.driver.as_ref())
        .expect("driver present")
        .clocks[0]
        .binding
        .as_ref()
        .expect("binding committed")
        .default_frequency;
    assert_eq!(post_freq, 48_000_000);
}

#[test]
fn insert_property_value_commits_on_phase_2_when_valid() {
    // Strip the property value inside the guard and re-insert via
    // insert_property_value. Commit must succeed.
    let mut model = build_minimal_phase_2_ok();
    {
        let mut guard = model.begin_mutation(MutationPhase::Phase2);
        guard.modify_driver_struct(|d: &mut DriverStruct| {
            d.properties[0].bound_value = None;
        });
        guard.insert_property_value(DeclId::new(PROPERTY_ID), LiteralValue::U32(12_000_000));
        guard.commit().expect("re-inserted property value is valid");
    }

    let post_value = model
        .root_module
        .submodules
        .iter()
        .find(|m| m.kind == ModuleKind::Driver)
        .and_then(|m| m.driver.as_ref())
        .expect("driver present")
        .properties[0]
        .bound_value
        .clone()
        .expect("bound value committed");
    assert_eq!(post_value, LiteralValue::U32(12_000_000));
}

// ---------------------------------------------------------------------
// (8) Task 62: lint channel pass-through through MutationGuard::commit.
// A mutation that introduces a model triggering a lint (property
// marked required AND carrying a default_value) must commit
// successfully, the lint must surface in the returned VerifyReport,
// and the model must NOT roll back.
// ---------------------------------------------------------------------

#[test]
fn commit_surfaces_lint_findings_without_rollback() {
    use regseq_model::verify::{LintKind, LintLevel};

    let mut model = build_minimal_phase_2_ok();

    // Flip the property into the "required with default" shape, which
    // triggers the `PropertyRequiredWithDefault` lint without violating
    // any Phase-2 structural invariant (bound_value stays populated, so
    // `required` remains satisfied).
    {
        let mut guard = model.begin_mutation(MutationPhase::Phase2);
        guard.modify_driver_struct(|d: &mut DriverStruct| {
            d.properties[0].required = true;
            d.properties[0].default_value = Some(LiteralValue::U32(0));
        });
        let report = guard
            .commit()
            .expect("lint-only model must not trigger rollback");
        assert_eq!(report.lints.len(), 1, "exactly one lint must fire");
        assert_eq!(report.lints[0].level, LintLevel::Advisory);
        match &report.lints[0].kind {
            LintKind::PropertyRequiredWithDefault { property } => {
                assert_eq!(*property, DeclId::new(PROPERTY_ID));
            }
        }
    }

    // Mutation stuck: the default_value we inserted is still present.
    let post_default = model
        .root_module
        .submodules
        .iter()
        .find(|m| m.kind == ModuleKind::Driver)
        .and_then(|m| m.driver.as_ref())
        .expect("driver present")
        .properties[0]
        .default_value
        .clone();
    assert_eq!(post_default, Some(LiteralValue::U32(0)));
}
