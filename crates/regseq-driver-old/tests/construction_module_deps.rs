//! Voided pending MonoRust + RegseqModel redesign.
//! See docs/discussions/compiler-refactoring/test-audit.md for the classification rationale
//! and the retirement criterion.
#![cfg(feature = "post-monorust-tests")]

//! Per-module dependency-set derivation tests (regseq-model-53).
//!
//! Exercises [`derive_module_dependencies`] on:
//! - Hand-built minimal models that isolate each contribution path
//!   (DriverStruct resources, TypeDef fields, function params,
//!   return types, locals, Place projections, IntrinsicCall
//!   terminators).
//! - A PL011-like hand-built driver model that covers the full
//!   "after Pass 1 + Pass 2 + derivation" shape the spec calls out.
//! - Determinism and bottom-up module-tree recursion.
//! - A rustc-harness-driven Pass 1 + Pass 2 + derivation smoke
//!   test that verifies the integration wiring through `build_body`.

#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, Once};

use rustc_hir::def_id::DefId;
use rustc_middle::ty::TyCtxt;

use regseq_driver_old::construction::{
    build_body, build_pass1_output, derive_module_dependencies, ConstantIndex, DerefCollapseLookup,
    ExtractionRegistries, FunctionIndex, IntrinsicLookup, LogArgUnwrap, OrderedMarkers,
    Pass1Output, PinStateRegistry, RegisterIndex, TryMarkers, TypeDefIndex,
};
use regseq_driver_old::extract::abstraction_types::AbstractionTypeRegistry;
use regseq_driver_old::extract::framework_types::FrameworkTypeRegistry;
use regseq_driver_old::extract::intrinsic_registry::IntrinsicRegistry;
use regseq_model_old::identity::{
    BasicBlockId, DeclId, FunctionId, LocalId, ModuleId, ResourceId, SourceOrigin, Symbol,
    SymbolInterner, TypeDefId,
};
use regseq_model_old::ir::{
    BasicBlock, BasicBlockSourceMap, Body, BodySourceMap, IntrinsicMetadata, LocalDecl, Mutability,
    Place, Projection, RegseqAbstraction, RegseqIntrinsic, RegseqType, ScalarType, Terminator,
    TerminatorKind,
};
use regseq_model_old::model::{
    Attrs, ClockDecl, ClockResource, DriverIdentity, DriverStruct, FieldDef, Function,
    InterruptDecl, InterruptResource, MmioDecl, MmioResource, Module, ModuleKind, Param, ParamKind,
    PinCtrlDecl, PinCtrlResource, RegseqModel, ResetDecl, ResetResource, Shape, SyncKind,
    SyncResource, TypeDef, TypeDefClassification, TypeDefKind,
};

// ---------------------------------------------------------------------------
// Model builders
// ---------------------------------------------------------------------------

fn intern(interner: &mut SymbolInterner, s: &str) -> Symbol {
    interner.intern(s)
}

fn empty_module(interner: &mut SymbolInterner, name: &str, id: u32, kind: ModuleKind) -> Module {
    let name_sym = intern(interner, name);
    Module {
        id: ModuleId::from(DeclId::new(id)),
        name: name_sym,
        kind,
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
    }
}

fn single_origin() -> SourceOrigin {
    SourceOrigin::CompilerGenerated
}

fn body_with_terminators(terminators: Vec<TerminatorKind>, locals: Vec<RegseqType>) -> Body {
    let local_decls: Vec<LocalDecl> = locals
        .into_iter()
        .map(|ty| LocalDecl {
            ty,
            mutable: false,
            name: None,
        })
        .collect();
    let mut basic_blocks: Vec<BasicBlock> = Vec::new();
    let mut block_sources: Vec<BasicBlockSourceMap> = Vec::new();
    for term in terminators {
        basic_blocks.push(BasicBlock {
            statements: vec![],
            terminator: Terminator { kind: term },
        });
        block_sources.push(BasicBlockSourceMap {
            statements: vec![],
            terminator: single_origin(),
        });
    }
    let locals_origins: Vec<SourceOrigin> =
        (0..local_decls.len()).map(|_| single_origin()).collect();
    Body {
        local_decls,
        arg_count: 0,
        basic_blocks,
        source_map: BodySourceMap {
            body_span: single_origin(),
            locals: locals_origins,
            blocks: block_sources,
        },
        ordered_regions: vec![],
    }
}

fn empty_body_with_return() -> Body {
    body_with_terminators(vec![TerminatorKind::Return], vec![RegseqType::Unit])
}

fn new_function(
    interner: &mut SymbolInterner,
    fn_id: u32,
    name: &str,
    params: Vec<Param>,
    return_type: RegseqType,
    body: Body,
) -> Function {
    let name_sym = intern(interner, name);
    Function {
        id: FunctionId::from(DeclId::new(fn_id)),
        name: name_sym,
        params,
        return_type,
        body,
        attrs: Attrs::default(),
    }
}

fn abstraction_type_def(
    interner: &mut SymbolInterner,
    id: u32,
    name: &str,
    a: RegseqAbstraction,
) -> TypeDef {
    let name_sym = intern(interner, name);
    TypeDef {
        id: TypeDefId::from(DeclId::new(id)),
        name: name_sym,
        kind: TypeDefKind::Struct { fields: vec![] },
        classification: TypeDefClassification::Abstraction(a),
        attrs: Attrs::default(),
    }
}

fn user_type_def(interner: &mut SymbolInterner, id: u32, name: &str) -> TypeDef {
    let name_sym = intern(interner, name);
    TypeDef {
        id: TypeDefId::from(DeclId::new(id)),
        name: name_sym,
        kind: TypeDefKind::Struct { fields: vec![] },
        classification: TypeDefClassification::UserDefined,
        attrs: Attrs::default(),
    }
}

// ---------------------------------------------------------------------------
// Minimal case #1: TypeDef with no abstraction-classified fields.
// Both sets must be empty after derivation (spec test plan bullet 2).
// ---------------------------------------------------------------------------

#[test]
fn minimal_module_with_plain_type_def_produces_empty_sets() {
    let mut interner = SymbolInterner::new();

    let mut root = empty_module(&mut interner, "root", 1, ModuleKind::Normal);
    // A UserDefined type with only scalar fields contributes nothing.
    let f0_name = intern(&mut interner, "x");
    let f1_name = intern(&mut interner, "y");
    let mut ud = user_type_def(&mut interner, 2, "Plain");
    ud.kind = TypeDefKind::Struct {
        fields: vec![
            FieldDef {
                name: f0_name,
                ty: RegseqType::Scalar(ScalarType::U32),
            },
            FieldDef {
                name: f1_name,
                ty: RegseqType::Bool,
            },
        ],
    };
    root.type_defs.push(ud);

    let mut model = RegseqModel {
        symbols: interner,
        source_locations: BTreeMap::new(),
        root_module: root,
        format_version: 1,
    };

    derive_module_dependencies(&mut model);

    assert!(model.root_module.abstractions_used.is_empty());
    assert!(model.root_module.intrinsics_used.is_empty());
}

// ---------------------------------------------------------------------------
// Minimal case #2: Function with &Clock param and zero IntrinsicCalls
// produces abstractions_used = {Clock}, intrinsics_used = {}.
// Covers the spec test plan bullet 3.
// ---------------------------------------------------------------------------

#[test]
fn function_with_clock_ref_param_and_no_intrinsic_calls_picks_up_clock_only() {
    let mut interner = SymbolInterner::new();
    let mut root = empty_module(&mut interner, "root", 1, ModuleKind::Normal);

    // The Clock abstraction type lives in the same module.
    let clock_td = abstraction_type_def(&mut interner, 2, "Clock", RegseqAbstraction::Clock);
    let clock_id = clock_td.id;
    root.type_defs.push(clock_td);

    // A free function `fn use_clock(c: &Clock)` whose body has no
    // IntrinsicCall terminator.
    let clk_name = intern(&mut interner, "c");
    let body = empty_body_with_return();
    let func = new_function(
        &mut interner,
        3,
        "use_clock",
        vec![Param {
            kind: ParamKind::Regular,
            name: clk_name,
            ty: RegseqType::Ref(Box::new(RegseqType::Defined(clock_id)), Mutability::Shared),
        }],
        RegseqType::Unit,
        body,
    );
    root.free_functions.push(func);

    let mut model = RegseqModel {
        symbols: interner,
        source_locations: BTreeMap::new(),
        root_module: root,
        format_version: 1,
    };

    derive_module_dependencies(&mut model);

    let mut expected: BTreeSet<RegseqAbstraction> = BTreeSet::new();
    expected.insert(RegseqAbstraction::Clock);
    assert_eq!(model.root_module.abstractions_used, expected);
    assert!(model.root_module.intrinsics_used.is_empty());
}

// ---------------------------------------------------------------------------
// PL011-like driver module fixture: driver struct carries every resource
// category, and synthetic bodies invoke the canonical intrinsics the
// task-spec acceptance list names. Verifies the containment-style
// assertions from the spec test plan bullet 1 end-to-end (minus the
// rustc pipeline, which the integration smoke test at the bottom
// exercises separately).
// ---------------------------------------------------------------------------

struct Pl011Build {
    model: RegseqModel,
    driver_path: ModulePath,
}

struct ModulePath(Vec<usize>);

impl ModulePath {
    fn resolve<'a>(&self, root: &'a Module) -> &'a Module {
        let mut cur = root;
        for &idx in &self.0 {
            cur = &cur.submodules[idx];
        }
        cur
    }
}

fn build_pl011_like_model() -> Pl011Build {
    let mut interner = SymbolInterner::new();

    let mut root = empty_module(&mut interner, "crate_root", 1, ModuleKind::Normal);
    let mut driver = empty_module(&mut interner, "driver", 2, ModuleKind::Driver);

    // Abstraction TypeDefs under the driver module.
    let mmio_td = abstraction_type_def(&mut interner, 10, "Mmio", RegseqAbstraction::Mmio);
    let clock_td = abstraction_type_def(&mut interner, 11, "Clock", RegseqAbstraction::Clock);
    let interrupt_td =
        abstraction_type_def(&mut interner, 12, "Interrupt", RegseqAbstraction::Interrupt);
    let pinctrl_td = abstraction_type_def(&mut interner, 13, "PinCtrl", RegseqAbstraction::PinCtrl);
    let reset_td = abstraction_type_def(&mut interner, 14, "Reset", RegseqAbstraction::Reset);

    let mmio_id = mmio_td.id;
    let _clock_id = clock_td.id;
    let _interrupt_id = interrupt_td.id;
    let _pinctrl_id = pinctrl_td.id;
    let _reset_id = reset_td.id;

    driver.type_defs.push(mmio_td);
    driver.type_defs.push(clock_td);
    driver.type_defs.push(interrupt_td);
    driver.type_defs.push(pinctrl_td);
    driver.type_defs.push(reset_td);

    // The driver struct itself (as a TypeDef).
    let drv_type_id = TypeDefId::from(DeclId::new(20));
    let drv_name = intern(&mut interner, "Pl011");
    let drv_identity = DriverIdentity {
        name: intern(&mut interner, "pl011-fixture"),
        compatible: vec![],
        bus: None,
    };

    let regs_field = intern(&mut interner, "regs");
    let clk_field = intern(&mut interner, "clk");
    let rst_field = intern(&mut interner, "rst");
    let pins_field = intern(&mut interner, "pins");
    let irq_field = intern(&mut interner, "irq");

    let regs_id = intern(&mut interner, "regs");
    let clk_id = intern(&mut interner, "uart-clk");
    let rst_id = intern(&mut interner, "uart-reset");
    let pins_id = intern(&mut interner, "uart-pins");
    let irq_id = intern(&mut interner, "uart-irq");

    let register_block_id = regseq_model_old::identity::RegisterBlockId::from(DeclId::new(99));

    let driver_struct = DriverStruct {
        type_id: drv_type_id,
        name: drv_name,
        identity: drv_identity,
        device_class: None,
        pm_info: None,
        platform_metadata: None,
        clocks: vec![ClockResource {
            id: ResourceId::from(DeclId::new(30)),
            field_name: clk_field,
            shape: Shape::Single,
            decl: ClockDecl { clock_id: clk_id },
            binding: None,
        }],
        resets: vec![ResetResource {
            id: ResourceId::from(DeclId::new(31)),
            field_name: rst_field,
            shape: Shape::Optional,
            decl: ResetDecl { reset_id: rst_id },
            binding: None,
        }],
        pinctrl: vec![PinCtrlResource {
            id: ResourceId::from(DeclId::new(32)),
            field_name: pins_field,
            shape: Shape::Single,
            decl: PinCtrlDecl {
                pinctrl_id: pins_id,
            },
            binding: None,
        }],
        interrupts: vec![InterruptResource {
            id: ResourceId::from(DeclId::new(33)),
            field_name: irq_field,
            shape: Shape::Single,
            decl: InterruptDecl {
                interrupt_id: irq_id,
            },
            binding: None,
        }],
        mmio: vec![MmioResource {
            id: ResourceId::from(DeclId::new(34)),
            field_name: regs_field,
            shape: Shape::Single,
            decl: MmioDecl {
                mmio_id: regs_id,
                register_block: register_block_id,
            },
            binding: None,
        }],
        sync: vec![],
        properties: vec![],
        user_data: vec![],
    };
    driver.driver = Some(driver_struct);

    // Also include a TypeDef entry for the driver struct so the
    // "TypeDef field scan" reaches its fields too. The struct has no
    // abstraction-classified fields on its own in this fixture -- the
    // abstractions appear via the DriverStruct resource lists.
    let drv_type = TypeDef {
        id: drv_type_id,
        name: drv_name,
        kind: TypeDefKind::Struct { fields: vec![] },
        classification: TypeDefClassification::UserDefined,
        attrs: Attrs::default(),
    };
    driver.type_defs.push(drv_type);

    // Synthesize a driver impl block with methods that carry
    // IntrinsicCall terminators for every intrinsic the acceptance
    // list names, plus one Return terminator so the bodies are
    // well-formed.
    let method_intrinsics = [
        RegseqIntrinsic::RegisterRead,
        RegseqIntrinsic::RegisterReadField,
        RegseqIntrinsic::RegisterWrite,
        RegseqIntrinsic::RegisterWriteField,
        RegseqIntrinsic::ClockEnable,
        RegseqIntrinsic::ClockDisable,
        RegseqIntrinsic::BusyWait,
        RegseqIntrinsic::PinCtrlApply,
        RegseqIntrinsic::LogInfo,
    ];

    let mut methods: Vec<Function> = Vec::new();
    for (i, intr) in method_intrinsics.iter().enumerate() {
        let name = format!("m{}", i);
        let metadata = match intr {
            RegseqIntrinsic::LogInfo => Some(IntrinsicMetadata::Log {
                template: "hello".to_string(),
                placeholders: vec![],
            }),
            _ => None,
        };
        let terminators = vec![
            TerminatorKind::IntrinsicCall {
                intrinsic: *intr,
                args: vec![],
                metadata,
                destination: Place {
                    local: LocalId(0),
                    projection: vec![],
                },
                target: BasicBlockId(1),
                error_target: None,
            },
            TerminatorKind::Return,
        ];
        let locals = vec![RegseqType::Unit];
        let body = body_with_terminators(terminators, locals);
        methods.push(new_function(
            &mut interner,
            50 + i as u32,
            &name,
            vec![],
            RegseqType::Unit,
            body,
        ));
    }
    driver
        .driver_impl_blocks
        .push(regseq_model_old::model::ImplBlock {
            target_type: drv_type_id,
            trait_name: None,
            methods,
            associated_constants: vec![],
        });

    // Also include a Function on the driver that takes `&Mmio<()>` so
    // the Mmio abstraction is reachable via param-type scan (in
    // addition to the DriverStruct.mmio entry).
    let bar_name = intern(&mut interner, "bar");
    let helper_body = empty_body_with_return();
    let helper_fn = new_function(
        &mut interner,
        70,
        "helper",
        vec![Param {
            kind: ParamKind::Regular,
            name: bar_name,
            ty: RegseqType::Ref(Box::new(RegseqType::Defined(mmio_id)), Mutability::Shared),
        }],
        RegseqType::Unit,
        helper_body,
    );
    driver.free_functions.push(helper_fn);

    root.submodules.push(driver);

    let model = RegseqModel {
        symbols: interner,
        source_locations: BTreeMap::new(),
        root_module: root,
        format_version: 1,
    };

    Pl011Build {
        model,
        driver_path: ModulePath(vec![0]),
    }
}

#[test]
fn pl011_like_driver_module_contains_required_abstractions_and_intrinsics() {
    let Pl011Build {
        mut model,
        driver_path,
    } = build_pl011_like_model();

    derive_module_dependencies(&mut model);

    let driver_module = driver_path.resolve(&model.root_module);

    for expected in [
        RegseqAbstraction::Mmio,
        RegseqAbstraction::Clock,
        RegseqAbstraction::Interrupt,
        RegseqAbstraction::PinCtrl,
        RegseqAbstraction::Reset,
    ] {
        assert!(
            driver_module.abstractions_used.contains(&expected),
            "driver module must contain abstraction {expected:?}, got {:?}",
            driver_module.abstractions_used,
        );
    }

    for expected in [
        RegseqIntrinsic::RegisterRead,
        RegseqIntrinsic::RegisterReadField,
        RegseqIntrinsic::RegisterWrite,
        RegseqIntrinsic::RegisterWriteField,
        RegseqIntrinsic::ClockEnable,
        RegseqIntrinsic::ClockDisable,
        RegseqIntrinsic::BusyWait,
        RegseqIntrinsic::PinCtrlApply,
        RegseqIntrinsic::LogInfo,
    ] {
        assert!(
            driver_module.intrinsics_used.contains(&expected),
            "driver module must contain intrinsic {expected:?}, got {:?}",
            driver_module.intrinsics_used,
        );
    }
}

// ---------------------------------------------------------------------------
// Sibling Normal module has its own restricted set (spec test plan bullet 1
// sub-bullet 3). Build a root with one Driver submodule (populated) and
// one Normal sibling whose only content is a scalar helper; the sibling
// must come back with empty sets.
// ---------------------------------------------------------------------------

#[test]
fn normal_sibling_module_has_its_own_restricted_empty_set() {
    let mut interner = SymbolInterner::new();
    let mut root = empty_module(&mut interner, "root", 1, ModuleKind::Normal);

    let mut driver = empty_module(&mut interner, "drv", 2, ModuleKind::Driver);
    driver.type_defs.push(abstraction_type_def(
        &mut interner,
        3,
        "Clock",
        RegseqAbstraction::Clock,
    ));
    driver.driver = Some(DriverStruct {
        type_id: TypeDefId::from(DeclId::new(4)),
        name: intern(&mut interner, "Drv"),
        identity: DriverIdentity {
            name: intern(&mut interner, "drv"),
            compatible: vec![],
            bus: None,
        },
        device_class: None,
        pm_info: None,
        platform_metadata: None,
        clocks: vec![],
        resets: vec![],
        pinctrl: vec![],
        interrupts: vec![],
        mmio: vec![],
        sync: vec![SyncResource {
            id: ResourceId::from(DeclId::new(5)),
            field_name: intern(&mut interner, "lk"),
            sync_kind: SyncKind::Mutex,
            payload_type: Some(RegseqType::Scalar(ScalarType::U32)),
        }],
        properties: vec![],
        user_data: vec![],
    });

    let mut sibling = empty_module(&mut interner, "util", 10, ModuleKind::Normal);
    let body = empty_body_with_return();
    let x = intern(&mut interner, "x");
    sibling.free_functions.push(new_function(
        &mut interner,
        11,
        "identity",
        vec![Param {
            kind: ParamKind::Regular,
            name: x,
            ty: RegseqType::Scalar(ScalarType::U32),
        }],
        RegseqType::Scalar(ScalarType::U32),
        body,
    ));

    root.submodules.push(driver);
    root.submodules.push(sibling);

    let mut model = RegseqModel {
        symbols: interner,
        source_locations: BTreeMap::new(),
        root_module: root,
        format_version: 1,
    };

    derive_module_dependencies(&mut model);

    // Driver-side: Mutex abstraction flows in from the sync resource.
    let drv = &model.root_module.submodules[0];
    assert!(drv.abstractions_used.contains(&RegseqAbstraction::Mutex));

    // Sibling Normal module: nothing to pick up.
    let sibling_out = &model.root_module.submodules[1];
    assert!(sibling_out.abstractions_used.is_empty());
    assert!(sibling_out.intrinsics_used.is_empty());
}

// ---------------------------------------------------------------------------
// Bottom-up recursion: a parent module's set does NOT include its
// submodules' sets (spec Section 2 non-aggregation rule).
// ---------------------------------------------------------------------------

#[test]
fn parent_module_sets_do_not_aggregate_submodule_sets() {
    let mut interner = SymbolInterner::new();
    let mut root = empty_module(&mut interner, "root", 1, ModuleKind::Normal);

    let mut child = empty_module(&mut interner, "child", 2, ModuleKind::Normal);
    let clock_td = abstraction_type_def(&mut interner, 3, "Clock", RegseqAbstraction::Clock);
    let clock_id = clock_td.id;
    child.type_defs.push(clock_td);
    let body = body_with_terminators(
        vec![
            TerminatorKind::IntrinsicCall {
                intrinsic: RegseqIntrinsic::ClockEnable,
                args: vec![],
                metadata: None,
                destination: Place {
                    local: LocalId(0),
                    projection: vec![],
                },
                target: BasicBlockId(1),
                error_target: None,
            },
            TerminatorKind::Return,
        ],
        vec![RegseqType::Unit],
    );
    let c = intern(&mut interner, "c");
    child.free_functions.push(new_function(
        &mut interner,
        4,
        "enable",
        vec![Param {
            kind: ParamKind::Regular,
            name: c,
            ty: RegseqType::Ref(Box::new(RegseqType::Defined(clock_id)), Mutability::Shared),
        }],
        RegseqType::Unit,
        body,
    ));

    root.submodules.push(child);

    let mut model = RegseqModel {
        symbols: interner,
        source_locations: BTreeMap::new(),
        root_module: root,
        format_version: 1,
    };

    derive_module_dependencies(&mut model);

    // Parent must be empty despite child carrying Clock + ClockEnable.
    assert!(model.root_module.abstractions_used.is_empty());
    assert!(model.root_module.intrinsics_used.is_empty());

    // Child must reflect its own content.
    let child_out = &model.root_module.submodules[0];
    assert!(child_out
        .abstractions_used
        .contains(&RegseqAbstraction::Clock));
    assert!(child_out
        .intrinsics_used
        .contains(&RegseqIntrinsic::ClockEnable));
}

// ---------------------------------------------------------------------------
// Determinism: two successive derivations on identical models produce
// identical iteration orders for both sets.
// ---------------------------------------------------------------------------

#[test]
fn derivation_produces_deterministic_iteration_order_across_runs() {
    let once = build_pl011_like_model().model;
    let twice = build_pl011_like_model().model;
    let mut once = once;
    let mut twice = twice;
    derive_module_dependencies(&mut once);
    derive_module_dependencies(&mut twice);

    let once_abs: Vec<RegseqAbstraction> = once.root_module.submodules[0]
        .abstractions_used
        .iter()
        .copied()
        .collect();
    let twice_abs: Vec<RegseqAbstraction> = twice.root_module.submodules[0]
        .abstractions_used
        .iter()
        .copied()
        .collect();
    assert_eq!(
        once_abs, twice_abs,
        "BTreeSet iteration order must be deterministic across runs"
    );
    let once_intr: Vec<RegseqIntrinsic> = once.root_module.submodules[0]
        .intrinsics_used
        .iter()
        .copied()
        .collect();
    let twice_intr: Vec<RegseqIntrinsic> = twice.root_module.submodules[0]
        .intrinsics_used
        .iter()
        .copied()
        .collect();
    assert_eq!(once_intr, twice_intr);
}

// ---------------------------------------------------------------------------
// Place.projection Field.ty scan: an otherwise plain body whose only
// abstraction reference lives inside a Place.projection Field.ty on a
// statement's Assign destination must be picked up.
// ---------------------------------------------------------------------------

#[test]
fn place_projection_field_ty_scan_catches_abstraction_references() {
    let mut interner = SymbolInterner::new();
    let mut root = empty_module(&mut interner, "root", 1, ModuleKind::Normal);

    let clock_td = abstraction_type_def(&mut interner, 2, "Clock", RegseqAbstraction::Clock);
    let clock_id = clock_td.id;
    root.type_defs.push(clock_td);

    // Construct an Assign whose destination place carries a Field
    // projection whose `ty` is `Defined(Clock)`.
    use regseq_model_old::ir::{Rvalue, Statement};
    let field_name = intern(&mut interner, "uart_clk");
    let dest = Place {
        local: LocalId(0),
        projection: vec![Projection::Field {
            index: 0,
            name: Some(field_name),
            ty: RegseqType::Defined(clock_id),
        }],
    };
    let assign = Statement {
        kind: StatementKind::Assign(
            dest,
            Rvalue::Use(regseq_model_old::ir::Operand::Literal(
                regseq_model_old::ir::LiteralValue::U32(0),
            )),
        ),
    };

    use regseq_model_old::ir::StatementKind;
    let bb = BasicBlock {
        statements: vec![assign],
        terminator: Terminator {
            kind: TerminatorKind::Return,
        },
    };
    let body = Body {
        local_decls: vec![LocalDecl {
            ty: RegseqType::Unit,
            mutable: false,
            name: None,
        }],
        arg_count: 0,
        basic_blocks: vec![bb],
        source_map: BodySourceMap {
            body_span: single_origin(),
            locals: vec![single_origin()],
            blocks: vec![BasicBlockSourceMap {
                statements: vec![single_origin()],
                terminator: single_origin(),
            }],
        },
        ordered_regions: vec![],
    };

    root.free_functions.push(new_function(
        &mut interner,
        3,
        "scan_projection",
        vec![],
        RegseqType::Unit,
        body,
    ));

    let mut model = RegseqModel {
        symbols: interner,
        source_locations: BTreeMap::new(),
        root_module: root,
        format_version: 1,
    };

    derive_module_dependencies(&mut model);

    assert!(model
        .root_module
        .abstractions_used
        .contains(&RegseqAbstraction::Clock));
}

// ---------------------------------------------------------------------------
// rustc harness: Pass 1 + Pass 2 + derivation smoke test on an inline
// specimen that declares a `PinCtrl` abstraction + driver struct with
// a PinCtrl resource, and a free function invoking PinCtrl::apply.
// Proves the derivation wires through the real extraction pipeline.
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

fn with_specimen<R, F>(source: &str, test_name: &str, f: F) -> R
where
    R: Send + 'static,
    F: for<'tcx> FnOnce(TyCtxt<'tcx>, &Pass1Output) -> R + Send + 'static,
{
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

    let out: Arc<Mutex<Option<R>>> = Arc::new(Mutex::new(None));
    let callback_cell: Arc<Mutex<Option<Box<dyn FnOnce(TyCtxt<'_>, &Pass1Output) -> R + Send>>>> =
        Arc::new(Mutex::new(Some(Box::new(f))));

    struct Callbacks<R: Send + 'static> {
        out: Arc<Mutex<Option<R>>>,
        callback: Arc<Mutex<Option<Box<dyn FnOnce(TyCtxt<'_>, &Pass1Output) -> R + Send>>>>,
    }

    impl<R: Send + 'static> rustc_driver::Callbacks for Callbacks<R> {
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
            let pass1 =
                build_pass1_output(tcx, &registries).expect("Pass 1 must succeed on this specimen");
            let f = self
                .callback
                .lock()
                .unwrap()
                .take()
                .expect("callback taken once");
            let result = f(tcx, &pass1);
            *self.out.lock().unwrap() = Some(result);
            rustc_driver::Compilation::Stop
        }
    }

    let mut callbacks = Callbacks::<R> {
        out: out.clone(),
        callback: callback_cell,
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
        "-Zmir-opt-level=0".to_string(),
        "-Coverflow-checks=off".to_string(),
        "-Cdebug-assertions=off".to_string(),
    ];

    rustc_driver::run_compiler(&args, &mut callbacks);

    let result = out
        .lock()
        .unwrap()
        .take()
        .expect("callback must run in after_analysis");

    std::fs::remove_dir_all(&test_dir).ok();

    result
}

fn find_impl_fn_def_id<'tcx>(tcx: TyCtxt<'tcx>, self_ty: &str, method: &str) -> DefId {
    for item_id in tcx
        .hir_module_items(rustc_hir::def_id::LocalModDefId::CRATE_DEF_ID)
        .free_items()
    {
        let item = tcx.hir_item(item_id);
        if let rustc_hir::ItemKind::Impl(impl_block) = &item.kind {
            let impl_owner = item.owner_id.def_id.to_def_id();
            let impl_ty = tcx.type_of(impl_owner).instantiate_identity();
            if format!("{impl_ty}") != self_ty {
                continue;
            }
            for ii in impl_block.items {
                let impl_item = tcx.hir_impl_item(*ii);
                if let rustc_hir::ImplItemKind::Fn(_, _) = &impl_item.kind {
                    if impl_item.ident.name.as_str() == method {
                        return impl_item.owner_id.def_id.to_def_id();
                    }
                }
            }
        }
    }
    panic!("method `{self_ty}::{method}` not found in specimen");
}

const PL011_LIKE_SPECIMEN: &str = r#"
#![no_std]

pub struct Clock;

pub trait PinState {}

pub struct PinCtrl {
    _opaque: (),
}

impl PinCtrl {
    pub fn apply<S: PinState>(&self) {
        loop {}
    }
}

#[rseq::pin_state(id = "default")]
pub struct DefaultState;
impl PinState for DefaultState {}

#[rseq::driver(name = "pl011-like")]
pub struct Drv {
    #[rseq::clock(id = "apb-clk")]
    pub clk: Clock,
    #[rseq::pinctrl(id = "uart-pins")]
    pub pins: PinCtrl,
}

impl Drv {
    pub fn apply_default_pins(pinctrl: &PinCtrl) {
        pinctrl.apply::<DefaultState>()
    }
}

pub mod util {
    pub fn identity(x: u32) -> u32 {
        x
    }
}
"#;

/// Locate a mutable function in the module tree by [`FunctionId`].
fn find_function_mut(model: &mut RegseqModel, id: FunctionId) -> &mut Function {
    fn walk_mut(m: &mut Module, id: FunctionId) -> Option<&mut Function> {
        for f in &mut m.free_functions {
            if f.id == id {
                return Some(f);
            }
        }
        for b in &mut m.impl_blocks {
            for f in &mut b.methods {
                if f.id == id {
                    return Some(f);
                }
            }
        }
        for b in &mut m.trait_impls {
            for f in &mut b.methods {
                if f.id == id {
                    return Some(f);
                }
            }
        }
        for b in &mut m.driver_impl_blocks {
            for f in &mut b.methods {
                if f.id == id {
                    return Some(f);
                }
            }
        }
        for sub in &mut m.submodules {
            if let Some(found) = walk_mut(sub, id) {
                return Some(found);
            }
        }
        None
    }
    walk_mut(&mut model.root_module, id).expect("FunctionId must exist in the model")
}

/// Borrow the single Driver-kind module in a model tree. Panics if
/// there is not exactly one Driver module.
fn find_driver_module(root: &Module) -> &Module {
    fn collect<'a>(m: &'a Module, out: &mut Vec<&'a Module>) {
        if m.kind == ModuleKind::Driver {
            out.push(m);
        }
        for s in &m.submodules {
            collect(s, out);
        }
    }
    let mut found: Vec<&Module> = Vec::new();
    collect(root, &mut found);
    assert_eq!(
        found.len(),
        1,
        "expected exactly one Driver module in the tree, found {}",
        found.len()
    );
    found[0]
}

/// Borrow a specific Normal submodule by name, searching the whole tree.
fn find_normal_module_named<'a>(
    root: &'a Module,
    name_str: &str,
    interner: &SymbolInterner,
) -> &'a Module {
    fn walk<'a>(m: &'a Module, name_str: &str, interner: &SymbolInterner) -> Option<&'a Module> {
        if m.kind == ModuleKind::Normal && interner.resolve(m.name) == name_str {
            return Some(m);
        }
        for s in &m.submodules {
            if let Some(found) = walk(s, name_str, interner) {
                return Some(found);
            }
        }
        None
    }
    walk(root, name_str, interner)
        .unwrap_or_else(|| panic!("normal module named `{name_str}` not found"))
}

#[test]
fn pass1_pass2_derivation_smoke_test_through_rustc_harness() {
    let (driver_abstractions, driver_intrinsics, util_abstractions, util_intrinsics) =
        with_specimen(
            PL011_LIKE_SPECIMEN,
            "construction_module_deps_pl011_like",
            |tcx, pass1| {
                // Seed an intrinsic lookup for the local `apply` method.
                let apply = find_impl_fn_def_id(tcx, "PinCtrl", "apply");
                let intrinsic_lookup =
                    IntrinsicLookup::from_entries(tcx, [(apply, "pinctrl_apply")].into_iter())
                        .expect("canonical name must parse");

                // Build the body for Drv::apply_default_pins which
                // invokes the recognized PinCtrl::apply intrinsic.
                let apply_default = find_impl_fn_def_id(tcx, "Drv", "apply_default_pins");
                let fn_id = *pass1
                    .cross_refs
                    .functions
                    .get(&apply_default)
                    .expect("apply_default_pins FunctionId must be present");

                let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
                let constants = ConstantIndex::new(&pass1.cross_refs.constants);
                let functions = FunctionIndex::new(&pass1.cross_refs.functions);
                let abstraction_registry = AbstractionTypeRegistry::default();
                let deref_lookup = DerefCollapseLookup::build(tcx, &abstraction_registry)
                    .expect("Deref lang item resolves");
                let ordered_markers = OrderedMarkers::default();
                let try_markers = TryMarkers::default();
                let log_arg = LogArgUnwrap::default();
                let mut interner = SymbolInterner::new();
                let body = build_body(
                    tcx,
                    apply_default,
                    fn_id,
                    &type_def_index,
                    &constants,
                    &functions,
                    &intrinsic_lookup,
                    &deref_lookup,
                    &ordered_markers,
                    &try_markers,
                    &log_arg,
                    &RegisterIndex::default(),
                    &pass1.cross_refs.register_blocks,
                    &abstraction_registry,
                    &pass1.cross_refs.pin_state_lookup,
                    &mut interner,
                )
                .expect("build_body must succeed on apply_default_pins");

                // Reassemble a mutable model and attach the translated body.
                let mut model = pass1.model.clone();
                {
                    let f = find_function_mut(&mut model, fn_id);
                    f.body = body;
                }

                // Run the derivation under test: this is the Pass 3
                // step that populates module.abstractions_used /
                // module.intrinsics_used from the fully-populated
                // model state.
                derive_module_dependencies(&mut model);

                let driver_module = find_driver_module(&model.root_module);
                let driver_abs: BTreeSet<RegseqAbstraction> =
                    driver_module.abstractions_used.iter().copied().collect();
                let driver_intr: BTreeSet<RegseqIntrinsic> =
                    driver_module.intrinsics_used.iter().copied().collect();

                let util = find_normal_module_named(&model.root_module, "util", &model.symbols);
                let util_abs: BTreeSet<RegseqAbstraction> =
                    util.abstractions_used.iter().copied().collect();
                let util_intr: BTreeSet<RegseqIntrinsic> =
                    util.intrinsics_used.iter().copied().collect();

                (driver_abs, driver_intr, util_abs, util_intr)
            },
        );

    // Driver module: Clock from DriverStruct.clocks, PinCtrl from
    // DriverStruct.pinctrl, PinCtrlApply from the attached body.
    assert!(
        driver_abstractions.contains(&RegseqAbstraction::Clock),
        "driver module must contain Clock via DriverStruct.clocks, got {:?}",
        driver_abstractions,
    );
    assert!(
        driver_abstractions.contains(&RegseqAbstraction::PinCtrl),
        "driver module must contain PinCtrl via DriverStruct.pinctrl, got {:?}",
        driver_abstractions,
    );
    assert!(
        driver_intrinsics.contains(&RegseqIntrinsic::PinCtrlApply),
        "driver module must contain PinCtrlApply via the attached body, got {:?}",
        driver_intrinsics,
    );

    // The Normal sibling `util` carries only a scalar helper -- its
    // sets must remain empty (parent's sets do NOT aggregate child's,
    // and children carry only what their own items reference).
    assert!(
        util_abstractions.is_empty(),
        "normal sibling module `util` must have empty abstractions_used, got {:?}",
        util_abstractions,
    );
    assert!(
        util_intrinsics.is_empty(),
        "normal sibling module `util` must have empty intrinsics_used, got {:?}",
        util_intrinsics,
    );
}
