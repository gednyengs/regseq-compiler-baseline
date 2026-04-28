//! Voided pending MonoRust + RegseqModel redesign.
//! See docs/discussions/compiler-refactoring/test-audit.md for the classification rationale
//! and the retirement criterion.
#![cfg(feature = "post-monorust-tests")]

//! MIR-to-Regseq-IR mapping helper tests (regseq-model-42).
//!
//! Each test compiles a small inline specimen through a rustc harness,
//! runs Pass 1 to populate [`CrossRefMaps`], then drives the
//! `construction::mir_mapping` helpers against the live MIR of a
//! specific function in the specimen and asserts the returned Regseq
//! IR fragments cover the spec's positive cases and error variants.

#![feature(rustc_private)]
#![feature(box_patterns)]

extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

use std::sync::{Arc, Mutex, Once};

use rustc_hir::def_id::{DefId, LocalDefId};
use rustc_middle::mir;
use rustc_middle::ty::{self, TyCtxt};

use regseq_driver_old::construction::{
    build_pass1_output, map_const, map_operand, map_place, map_rvalue, map_ty, ConstantIndex,
    ConstructionError, ExtractionRegistries, LocalIdMap, MapOperandResult, Pass1Output,
    PinStateRegistry, TypeDefIndex,
};
use regseq_driver_old::extract::abstraction_types::AbstractionTypeRegistry;
use regseq_driver_old::extract::framework_types::FrameworkTypeRegistry;
use regseq_driver_old::extract::intrinsic_registry::IntrinsicRegistry;
use regseq_model_old::identity::{LocalId, SymbolInterner};
use regseq_model_old::ir::{
    BorrowKind, CastKind, LiteralValue, Mutability, Operand, Place, Projection, RegseqType, Rvalue,
    ScalarType,
};

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

/// Invoke a callback `f` inside `after_analysis` with access to a live
/// [`TyCtxt`] plus the Pass 1 [`Pass1Output`]. The harness drives the
/// full rustc pipeline up through analysis, then hands control to
/// `f`, which drives the mapping helpers and asserts on their output.
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
            let pass1 = build_pass1_output(tcx, &registries)
                .expect("Pass 1 must succeed on mir_mapping specimens");
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
        // Match Phase 4's extraction stage: no optimizations so
        // MIR Const::Unevaluated stays unresolved and rvalues /
        // projections don't get collapsed before mapping.
        "-Zmir-opt-level=0".to_string(),
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

// ---------------------------------------------------------------------------
// Traversal helpers
// ---------------------------------------------------------------------------

/// Find the [`LocalDefId`] of a free function by name in the specimen.
fn find_fn_def_id<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> LocalDefId {
    for item_id in tcx
        .hir_module_items(rustc_hir::def_id::LocalModDefId::CRATE_DEF_ID)
        .free_items()
    {
        let item = tcx.hir_item(item_id);
        if let rustc_hir::ItemKind::Fn { ident, .. } = &item.kind {
            if ident.name.as_str() == name {
                return item.owner_id.def_id;
            }
        }
    }
    panic!("function `{name}` not found in specimen");
}

/// Find the [`DefId`] of a free constant by name in the specimen.
fn find_const_def_id<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> DefId {
    for item_id in tcx
        .hir_module_items(rustc_hir::def_id::LocalModDefId::CRATE_DEF_ID)
        .free_items()
    {
        let item = tcx.hir_item(item_id);
        if let rustc_hir::ItemKind::Const(ident, ..) = &item.kind {
            if ident.name.as_str() == name {
                return item.owner_id.def_id.to_def_id();
            }
        }
    }
    panic!("const `{name}` not found in specimen");
}

/// Build a [`LocalIdMap`] that assigns a dense `LocalId(i)` for each
/// MIR `_i`, keyed to the body's `local_decls`.
fn build_local_id_map<'a, 'tcx>(body: &'a mir::Body<'tcx>) -> LocalIdMap<'a, 'tcx> {
    let mut lim = LocalIdMap::new(&body.local_decls);
    for (local, _decl) in body.local_decls.iter_enumerated() {
        lim.insert(local, LocalId::new(local.as_u32()));
    }
    lim
}

// ---------------------------------------------------------------------------
// Positive: map_ty covers every RegseqType variant
// ---------------------------------------------------------------------------

/// Specimen that places one local of each [`RegseqType`] shape in a
/// dedicated function so the test can walk `body.local_decls` and map
/// each one.
const TY_SPECIMEN: &str = r#"
#![no_std]

pub struct Mmio<T> { _m: core::marker::PhantomData<T> }
pub struct Clock;
pub struct Block;

#[repr(C)]
#[rseq::register_block(endian = "little", access_width = 32)]
pub struct BlockRegs { pub x: u32 }

#[rseq::driver(name = "d", compatible = ["a,b"], bus = "amba")]
pub struct Drv {
    #[rseq::mmio(id = "regs")]
    pub regs: Mmio<BlockRegs>,
    #[rseq::clock(id = "c")]
    pub clk: Clock,
}

pub struct MyAdt { pub a: u32, pub b: bool }

/// Every parameter here corresponds to one target RegseqType variant.
pub fn ty_probe(
    a_u8: u8,
    a_u16: u16,
    a_u32: u32,
    a_u64: u64,
    a_usize: usize,
    a_i8: i8,
    a_i16: i16,
    a_i32: i32,
    a_i64: i64,
    a_isize: isize,
    a_bool: bool,
    a_unit: (),
    a_option: core::option::Option<u32>,
    a_result: core::result::Result<u32, u32>,
    a_array: [u32; 4],
    a_slice: &[u32],
    a_ref: &u32,
    a_ref_mut: &mut u32,
    a_defined: MyAdt,
) -> ! {
    loop {}
}
"#;

#[test]
fn map_ty_covers_all_regseq_type_variants() {
    let outcome = with_specimen(TY_SPECIMEN, "mir_mapping_ty_probe", |tcx, pass1| {
        let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);

        let fn_def = find_fn_def_id(tcx, "ty_probe");
        let body = tcx.optimized_mir(fn_def.to_def_id());

        // MIR locals _1.._N are the function parameters in order. MIR
        // `_0` is the return slot.
        let mut param_iter = body.args_iter();

        let expected: &[(&str, RegseqType)] = &[
            ("a_u8", RegseqType::Scalar(ScalarType::U8)),
            ("a_u16", RegseqType::Scalar(ScalarType::U16)),
            ("a_u32", RegseqType::Scalar(ScalarType::U32)),
            ("a_u64", RegseqType::Scalar(ScalarType::U64)),
            ("a_usize", RegseqType::Scalar(ScalarType::Usize)),
            ("a_i8", RegseqType::Scalar(ScalarType::I8)),
            ("a_i16", RegseqType::Scalar(ScalarType::I16)),
            ("a_i32", RegseqType::Scalar(ScalarType::I32)),
            ("a_i64", RegseqType::Scalar(ScalarType::I64)),
            ("a_isize", RegseqType::Scalar(ScalarType::Isize)),
            ("a_bool", RegseqType::Bool),
            ("a_unit", RegseqType::Unit),
            (
                "a_option",
                RegseqType::Option(Box::new(RegseqType::Scalar(ScalarType::U32))),
            ),
            (
                "a_result",
                RegseqType::Result(
                    Box::new(RegseqType::Scalar(ScalarType::U32)),
                    Box::new(RegseqType::Scalar(ScalarType::U32)),
                ),
            ),
            (
                "a_array",
                RegseqType::Array(Box::new(RegseqType::Scalar(ScalarType::U32)), 4),
            ),
            (
                "a_slice",
                RegseqType::Slice(Box::new(RegseqType::Scalar(ScalarType::U32))),
            ),
            (
                "a_ref",
                RegseqType::Ref(
                    Box::new(RegseqType::Scalar(ScalarType::U32)),
                    Mutability::Shared,
                ),
            ),
            (
                "a_ref_mut",
                RegseqType::Ref(
                    Box::new(RegseqType::Scalar(ScalarType::U32)),
                    Mutability::Mutable,
                ),
            ),
        ];

        for (name, expect) in expected {
            let local = param_iter.next().expect("param exhausted");
            let actual = map_ty(tcx, body.local_decls[local].ty, &type_def_index)
                .unwrap_or_else(|e| panic!("{name}: {e:?}"));
            assert_eq!(
                &actual, expect,
                "param `{name}` must map to {:?}, got {:?}",
                expect, actual
            );
        }

        // a_defined: MyAdt → Defined(TypeDefId(MyAdt)). Compare against
        // the exact TypeDefId the Pass 1 cross-ref table recorded for
        // the ADT's DefId, not just the `Defined(..)` shape.
        let my_adt_local = param_iter.next().expect("MyAdt param");
        let adt_ty = body.local_decls[my_adt_local].ty;
        let ty::TyKind::Adt(adt_def, _) = adt_ty.kind() else {
            panic!("MyAdt param should be a ty::Adt");
        };
        let expected_tid = pass1
            .cross_refs
            .type_defs
            .get(&adt_def.did())
            .copied()
            .expect("MyAdt must be recorded in cross_refs.type_defs");
        let actual = map_ty(tcx, adt_ty, &type_def_index).expect("MyAdt must map");
        assert_eq!(
            actual,
            RegseqType::Defined(expected_tid),
            "MyAdt must map to the exact TypeDefId in cross_refs"
        );

        // Return type of the function is Never (!).
        let ret_ty = body.local_decls[mir::Local::from_u32(0)].ty;
        let ret_regseq = map_ty(tcx, ret_ty, &type_def_index).expect("Never must map");
        assert_eq!(ret_regseq, RegseqType::Never);
    });
    let _ = outcome;
}

// ---------------------------------------------------------------------------
// Positive: map_const covers every literal width
// ---------------------------------------------------------------------------

const CONST_SPECIMEN: &str = r#"
#![no_std]

pub struct Mmio<T> { _m: core::marker::PhantomData<T> }
#[repr(C)]
#[rseq::register_block(endian = "little", access_width = 32)]
pub struct BlockRegs { pub x: u32 }

#[rseq::driver(name = "d", compatible = ["a,b"], bus = "amba")]
pub struct Drv {
    #[rseq::mmio(id = "regs")]
    pub regs: Mmio<BlockRegs>,
}

pub const C_U8: u8 = 0xFE;
pub const C_U16: u16 = 0x1234;
pub const C_U32: u32 = 0xDEAD_BEEF;
pub const C_U64: u64 = 0x1122_3344_5566_7788;
pub const C_USIZE: usize = 1024;
pub const C_I8: i8 = -1;
pub const C_I16: i16 = -2;
pub const C_I32: i32 = -100;
pub const C_I64: i64 = -9;
pub const C_ISIZE: isize = -42;
pub const C_BOOL: bool = true;
"#;

#[test]
fn map_const_covers_all_literal_widths() {
    with_specimen(CONST_SPECIMEN, "mir_mapping_const_widths", |tcx, pass1| {
        let cases: &[(&str, LiteralValue)] = &[
            ("C_U8", LiteralValue::U8(0xFE)),
            ("C_U16", LiteralValue::U16(0x1234)),
            ("C_U32", LiteralValue::U32(0xDEAD_BEEF)),
            ("C_U64", LiteralValue::U64(0x1122_3344_5566_7788)),
            ("C_USIZE", LiteralValue::Usize(1024)),
            ("C_I8", LiteralValue::I8(-1)),
            ("C_I16", LiteralValue::I16(-2)),
            ("C_I32", LiteralValue::I32(-100)),
            ("C_I64", LiteralValue::I64(-9)),
            ("C_ISIZE", LiteralValue::Isize(-42)),
            ("C_BOOL", LiteralValue::Bool(true)),
        ];

        for (name, expected) in cases {
            let def_id = find_const_def_id(tcx, name);
            // Evaluate through tcx.const_eval_poly to obtain a scalar, then
            // wrap back into a ty::Const leaf for map_const.
            let value = tcx
                .const_eval_poly(def_id)
                .unwrap_or_else(|e| panic!("const_eval_poly({name}): {e:?}"));
            let scalar = value
                .try_to_scalar_int()
                .unwrap_or_else(|| panic!("{name}: not a scalar-int ConstValue"));
            let ty_const = ty::Const::new_value(
                tcx,
                ty::ValTree::from_scalar_int(tcx, scalar),
                tcx.type_of(def_id).instantiate_identity(),
            );
            let lit = map_const(tcx, ty_const, tcx.type_of(def_id).instantiate_identity())
                .unwrap_or_else(|e| panic!("map_const({name}): {e:?}"));
            assert_eq!(lit, *expected, "literal mapping for {name}");
        }

        // cross_refs.constants must carry each constant after Pass 1 (task
        // 42 extends CrossRefMaps with the constants map).
        assert!(
            cases.iter().all(|(n, _)| pass1
                .cross_refs
                .constants
                .contains_key(&find_const_def_id(tcx, n))),
            "every constant must be indexed in cross_refs.constants",
        );
    });
}

// ---------------------------------------------------------------------------
// Positive: map_operand / map_place / map_rvalue on a body
// ---------------------------------------------------------------------------

const BODY_SPECIMEN: &str = r#"
#![no_std]

pub struct Mmio<T> { _m: core::marker::PhantomData<T> }
#[repr(C)]
#[rseq::register_block(endian = "little", access_width = 32)]
pub struct BlockRegs { pub x: u32 }

#[rseq::driver(name = "d", compatible = ["a,b"], bus = "amba")]
pub struct Drv {
    #[rseq::mmio(id = "regs")]
    pub regs: Mmio<BlockRegs>,
}

pub struct Pair { pub lo: u32, pub hi: u32 }

pub enum Tag { A(u32), B(bool) }

pub const FOO_CONST: u32 = 0x07FF;

/// Exercises: Use(Copy), Use(Move), literal operand, constant operand,
/// Ref, Cast(IntToInt), BinaryOp(Add), UnaryOp(Not), Aggregate(Tuple),
/// Aggregate(Array), Aggregate(Adt), Discriminant, Repeat, Downcast.
///
/// The `match t` arms bind the inner data so MIR emits a
/// `Downcast(variant) + Field` projection rather than just a discriminant
/// read. The indexed assignment `arr[i] = 42` forces an `Index` projection
/// (a bare local index would be const-folded at -Zmir-opt-level=0).
pub fn body_probe(p: Pair, mut arr: [u32; 3], i: usize, t: Tag) -> u32 {
    let a: u32 = 10;
    let b: u32 = 20;
    let _sum = a + b;
    let _not_b: bool = !true;
    let _copied = p.lo;
    let _cast: u64 = _copied as u64;
    let _r: &u32 = &a;
    let _tup: (u32, bool) = (1, true);
    let _arr2: [u32; 3] = [7, 8, 9];
    let _rep: [u32; 4] = [0u32; 4];
    let _pair2 = Pair { lo: 1, hi: 2 };
    let _d = match t {
        Tag::A(v) => v,
        Tag::B(_) => 0u32,
    };
    let _k = FOO_CONST;
    arr[i] = 42;
    _sum
}
"#;

#[test]
fn map_rvalue_use_copy_and_move() {
    with_specimen(BODY_SPECIMEN, "mir_mapping_body_use", |tcx, pass1| {
        let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
        let constant_index = ConstantIndex::new(&pass1.cross_refs.constants);
        let fn_def = find_fn_def_id(tcx, "body_probe");
        let body = tcx.optimized_mir(fn_def.to_def_id());
        let lim = build_local_id_map(body);
        let mut interner = SymbolInterner::new();

        let mut saw_copy = false;
        let mut saw_move = false;
        let mut saw_literal = false;

        for bb in body.basic_blocks.iter() {
            for stmt in &bb.statements {
                if let mir::StatementKind::Assign(box (_place, rvalue)) = &stmt.kind {
                    if let mir::Rvalue::Use(op) = rvalue {
                        let mapped = match map_operand(
                            tcx,
                            op,
                            &lim,
                            &constant_index,
                            &type_def_index,
                            &mut interner,
                        )
                        .expect("operand must map")
                        {
                            MapOperandResult::Mapped(o) => o,
                            MapOperandResult::Drop => continue,
                        };
                        match mapped {
                            Operand::Copy(_) => saw_copy = true,
                            Operand::Move(_) => saw_move = true,
                            Operand::Literal(_) => saw_literal = true,
                            Operand::Constant(_) => { /* seen in separate test */ }
                        }
                    }
                }
            }
        }

        assert!(saw_copy, "body must contain at least one Operand::Copy");
        assert!(saw_move, "body must contain at least one Operand::Move");
        assert!(
            saw_literal,
            "body must contain at least one literal operand"
        );
    });
}

#[test]
fn map_place_field_and_deref_projections_are_named() {
    with_specimen(BODY_SPECIMEN, "mir_mapping_body_place", |tcx, pass1| {
        let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
        let fn_def = find_fn_def_id(tcx, "body_probe");
        let body = tcx.optimized_mir(fn_def.to_def_id());
        let lim = build_local_id_map(body);
        let mut interner = SymbolInterner::new();

        let mut places: Vec<&mir::Place<'_>> = Vec::new();
        for bb in body.basic_blocks.iter() {
            for stmt in &bb.statements {
                if let mir::StatementKind::Assign(box (place, rvalue)) = &stmt.kind {
                    places.push(place);
                    collect_rvalue_places(rvalue, &mut places);
                }
            }
            match &bb.terminator().kind {
                mir::TerminatorKind::Call {
                    destination, args, ..
                } => {
                    places.push(destination);
                    for arg in args.iter() {
                        if let mir::Operand::Copy(p) | mir::Operand::Move(p) = &arg.node {
                            places.push(p);
                        }
                    }
                }
                mir::TerminatorKind::SwitchInt { discr, .. } => {
                    if let mir::Operand::Copy(p) | mir::Operand::Move(p) = discr {
                        places.push(p);
                    }
                }
                _ => {}
            }
        }

        let mut saw_named_field = false;
        let mut saw_deref = false;
        let mut saw_downcast = false;
        let mut saw_index = false;
        for place in places {
            if let Ok(mapped) = map_place(tcx, place, &lim, &type_def_index, &mut interner) {
                scan_projections(
                    &mapped,
                    &mut saw_named_field,
                    &mut saw_deref,
                    &mut saw_downcast,
                    &mut saw_index,
                );
            }
        }

        assert!(
            saw_named_field,
            "expected at least one named Field projection"
        );
        assert!(saw_deref, "expected at least one Deref projection");
        assert!(saw_downcast, "expected at least one Downcast projection");
        assert!(saw_index, "expected at least one Index projection");
    });
}

fn collect_rvalue_places<'tcx, 'b>(
    rv: &'b mir::Rvalue<'tcx>,
    places: &mut Vec<&'b mir::Place<'tcx>>,
) {
    match rv {
        mir::Rvalue::Use(op) | mir::Rvalue::UnaryOp(_, op) | mir::Rvalue::Cast(_, op, _) => {
            if let mir::Operand::Copy(p) | mir::Operand::Move(p) = op {
                places.push(p);
            }
        }
        mir::Rvalue::Repeat(op, _) => {
            if let mir::Operand::Copy(p) | mir::Operand::Move(p) = op {
                places.push(p);
            }
        }
        mir::Rvalue::Ref(_, _, p)
        | mir::Rvalue::Discriminant(p)
        | mir::Rvalue::CopyForDeref(p)
        | mir::Rvalue::RawPtr(_, p) => places.push(p),
        mir::Rvalue::BinaryOp(_, operands) => {
            for op in [&operands.0, &operands.1] {
                if let mir::Operand::Copy(p) | mir::Operand::Move(p) = op {
                    places.push(p);
                }
            }
        }
        mir::Rvalue::Aggregate(_, ops) => {
            for op in ops.iter() {
                if let mir::Operand::Copy(p) | mir::Operand::Move(p) = op {
                    places.push(p);
                }
            }
        }
        _ => {}
    }
}

fn scan_projections(
    p: &Place,
    named: &mut bool,
    deref: &mut bool,
    downcast: &mut bool,
    index: &mut bool,
) {
    for proj in &p.projection {
        match proj {
            Projection::Field { name: Some(_), .. } => *named = true,
            Projection::Deref => *deref = true,
            Projection::Downcast { .. } => *downcast = true,
            Projection::Index(_) => *index = true,
            _ => {}
        }
    }
}

#[test]
fn map_rvalue_covers_core_variants() {
    with_specimen(BODY_SPECIMEN, "mir_mapping_body_rvalue", |tcx, pass1| {
        let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
        let constant_index = ConstantIndex::new(&pass1.cross_refs.constants);
        let fn_def = find_fn_def_id(tcx, "body_probe");
        let body = tcx.optimized_mir(fn_def.to_def_id());
        let lim = build_local_id_map(body);
        let mut interner = SymbolInterner::new();

        let mut saw_use = false;
        let mut saw_repeat = false;
        let mut saw_ref = false;
        let mut saw_cast = false;
        let mut saw_binary = false;
        let mut saw_unary = false;
        let mut saw_discriminant = false;
        let mut saw_aggregate = false;

        for bb in body.basic_blocks.iter() {
            for stmt in &bb.statements {
                if let mir::StatementKind::Assign(box (_place, rvalue)) = &stmt.kind {
                    let mapped = match map_rvalue(
                        tcx,
                        rvalue,
                        &lim,
                        &type_def_index,
                        &constant_index,
                        &mut interner,
                    ) {
                        Ok(r) => r,
                        Err(_) => continue,
                    };
                    match mapped {
                        Rvalue::Use(_) => saw_use = true,
                        Rvalue::Repeat(_, _) => saw_repeat = true,
                        Rvalue::Ref(_, _) => saw_ref = true,
                        Rvalue::Cast(_, _, _) => saw_cast = true,
                        Rvalue::BinaryOp(_, _, _) => saw_binary = true,
                        Rvalue::UnaryOp(_, _) => saw_unary = true,
                        Rvalue::Discriminant(_) => saw_discriminant = true,
                        Rvalue::Aggregate(_, _) => saw_aggregate = true,
                    }
                }
            }
        }

        assert!(saw_use, "Rvalue::Use must appear");
        assert!(saw_repeat, "Rvalue::Repeat must appear (from [0u32; 4])");
        assert!(saw_ref, "Rvalue::Ref must appear");
        assert!(saw_cast, "Rvalue::Cast must appear");
        assert!(saw_binary, "Rvalue::BinaryOp must appear");
        assert!(saw_unary, "Rvalue::UnaryOp must appear");
        assert!(
            saw_discriminant,
            "Rvalue::Discriminant must appear (from `match` on enum)"
        );
        assert!(saw_aggregate, "Rvalue::Aggregate must appear");
    });
}

#[test]
fn map_rvalue_cast_reduces_to_int_to_int() {
    with_specimen(BODY_SPECIMEN, "mir_mapping_body_cast", |tcx, pass1| {
        let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
        let constant_index = ConstantIndex::new(&pass1.cross_refs.constants);
        let fn_def = find_fn_def_id(tcx, "body_probe");
        let body = tcx.optimized_mir(fn_def.to_def_id());
        let lim = build_local_id_map(body);
        let mut interner = SymbolInterner::new();

        let mut saw_int_to_int_to_u64 = false;
        for bb in body.basic_blocks.iter() {
            for stmt in &bb.statements {
                if let mir::StatementKind::Assign(box (_place, rvalue)) = &stmt.kind {
                    if let mir::Rvalue::Cast(_, _, _) = rvalue {
                        if let Ok(Rvalue::Cast(ck, _, ty)) = map_rvalue(
                            tcx,
                            rvalue,
                            &lim,
                            &type_def_index,
                            &constant_index,
                            &mut interner,
                        ) {
                            if matches!(ck, CastKind::IntToInt)
                                && matches!(ty, RegseqType::Scalar(ScalarType::U64))
                            {
                                saw_int_to_int_to_u64 = true;
                            }
                        }
                    }
                }
            }
        }
        assert!(
            saw_int_to_int_to_u64,
            "cast `_copied as u64` must map to IntToInt -> Scalar(U64)"
        );
    });
}

#[test]
fn map_rvalue_ref_reduces_borrow_kind() {
    with_specimen(BODY_SPECIMEN, "mir_mapping_body_ref", |tcx, pass1| {
        let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
        let constant_index = ConstantIndex::new(&pass1.cross_refs.constants);
        let fn_def = find_fn_def_id(tcx, "body_probe");
        let body = tcx.optimized_mir(fn_def.to_def_id());
        let lim = build_local_id_map(body);
        let mut interner = SymbolInterner::new();

        let mut saw_shared = false;
        for bb in body.basic_blocks.iter() {
            for stmt in &bb.statements {
                if let mir::StatementKind::Assign(box (_place, rvalue)) = &stmt.kind {
                    if let mir::Rvalue::Ref(_, _, _) = rvalue {
                        if let Ok(Rvalue::Ref(bk, _)) = map_rvalue(
                            tcx,
                            rvalue,
                            &lim,
                            &type_def_index,
                            &constant_index,
                            &mut interner,
                        ) {
                            if matches!(bk, BorrowKind::Shared) {
                                saw_shared = true;
                            }
                        }
                    }
                }
            }
        }
        assert!(saw_shared, "let _r: &u32 = &a must produce Ref(Shared, _)");
    });
}

#[test]
fn map_operand_resolves_named_constant() {
    with_specimen(BODY_SPECIMEN, "mir_mapping_body_constant", |tcx, pass1| {
        let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
        let constant_index = ConstantIndex::new(&pass1.cross_refs.constants);
        let fn_def = find_fn_def_id(tcx, "body_probe");
        let body = tcx.optimized_mir(fn_def.to_def_id());
        let lim = build_local_id_map(body);
        let mut interner = SymbolInterner::new();

        // FOO_CONST must resolve to the exact ConstantId recorded in
        // `cross_refs.constants` for its DefId.
        let foo_const_did = find_const_def_id(tcx, "FOO_CONST");
        let expected_cid = pass1
            .cross_refs
            .constants
            .get(&foo_const_did)
            .copied()
            .expect("FOO_CONST must be recorded in cross_refs.constants");

        let mut saw_expected_constant_id = false;
        for bb in body.basic_blocks.iter() {
            for stmt in &bb.statements {
                if let mir::StatementKind::Assign(box (_place, mir::Rvalue::Use(op))) = &stmt.kind {
                    if let Ok(MapOperandResult::Mapped(Operand::Constant(cid))) = map_operand(
                        tcx,
                        op,
                        &lim,
                        &constant_index,
                        &type_def_index,
                        &mut interner,
                    ) {
                        if cid == expected_cid {
                            saw_expected_constant_id = true;
                        }
                    }
                }
            }
        }
        assert!(
            saw_expected_constant_id,
            "FOO_CONST usage must map to the exact ConstantId in cross_refs"
        );
    });
}

#[test]
fn map_rvalue_copy_for_deref_becomes_use_copy() {
    // Construct a `Rvalue::CopyForDeref` rvalue directly against a
    // trivial fixture so the assertion is deterministic regardless of
    // MIR pass behavior. The mapping helper must turn it into
    // `Use(Copy(place))` per `ir-reference.md` Section 3.
    const SPEC: &str = r#"
#![no_std]
pub struct Mmio<T> { _m: core::marker::PhantomData<T> }
#[repr(C)]
#[rseq::register_block(endian = "little", access_width = 32)]
pub struct BlockRegs { pub x: u32 }
#[rseq::driver(name = "d", compatible = ["a,b"], bus = "amba")]
pub struct Drv {
    #[rseq::mmio(id = "regs")]
    pub regs: Mmio<BlockRegs>,
}
pub fn plain(x: u32) -> u32 { x }
"#;
    with_specimen(SPEC, "mir_mapping_copy_for_deref", |tcx, pass1| {
        let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
        let constant_index = ConstantIndex::new(&pass1.cross_refs.constants);
        let fn_def = find_fn_def_id(tcx, "plain");
        let body = tcx.optimized_mir(fn_def.to_def_id());
        let lim = build_local_id_map(body);
        let mut interner = SymbolInterner::new();

        // Build the rvalue `CopyForDeref(_1)` -- the parameter local.
        let place = mir::Place {
            local: mir::Local::from_u32(1),
            projection: tcx.mk_place_elems(&[]),
        };
        let rvalue = mir::Rvalue::CopyForDeref(place);
        let mapped = map_rvalue(
            tcx,
            &rvalue,
            &lim,
            &type_def_index,
            &constant_index,
            &mut interner,
        )
        .expect("CopyForDeref must map");
        assert!(
            matches!(mapped, Rvalue::Use(Operand::Copy(_))),
            "CopyForDeref must collapse to Use(Copy), got {mapped:?}"
        );
    });
}

// ---------------------------------------------------------------------------
// Negative: std type not permitted
// ---------------------------------------------------------------------------

const STD_SPECIMEN: &str = r#"
pub struct Mmio<T> { _m: core::marker::PhantomData<T> }
#[repr(C)]
#[rseq::register_block(endian = "little", access_width = 32)]
pub struct BlockRegs { pub x: u32 }

#[rseq::driver(name = "d", compatible = ["a,b"], bus = "amba")]
pub struct Drv {
    #[rseq::mmio(id = "regs")]
    pub regs: Mmio<BlockRegs>,
}

// The signature carries a `std::*` ADT so `map_ty` surfaces
// `StdTypeNotPermitted` on the parameter. The body is intentionally
// a bare literal return -- it must NOT contain any method call on
// `s`, otherwise Pass 1's Rule 3 cross-crate walker (task
// regseq-model-47) fails earlier with `StdFunctionNotPermitted`
// before `map_ty` even gets a chance. The test targets the Pass 2
// `map_ty` guard specifically; the Pass 1 guard has its own test.
pub fn uses_std_string(_s: &std::string::String) -> usize {
    0
}
"#;

#[test]
fn map_ty_errors_on_std_type() {
    with_specimen(STD_SPECIMEN, "mir_mapping_std_error", |tcx, pass1| {
        let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
        let fn_def = find_fn_def_id(tcx, "uses_std_string");
        let body = tcx.optimized_mir(fn_def.to_def_id());
        let param = body.args_iter().next().expect("one param");
        let err = map_ty(tcx, body.local_decls[param].ty, &type_def_index)
            .expect_err("std::string::String must be rejected");
        assert!(
            matches!(err, ConstructionError::StdTypeNotPermitted { .. }),
            "expected StdTypeNotPermitted, got {err:?}"
        );
    });
}

// ---------------------------------------------------------------------------
// Negative: unsupported rustc type (raw pointer, function pointer, closure)
// ---------------------------------------------------------------------------

const UNSUPPORTED_TY_SPECIMEN: &str = r#"
#![no_std]

pub struct Mmio<T> { _m: core::marker::PhantomData<T> }
#[repr(C)]
#[rseq::register_block(endian = "little", access_width = 32)]
pub struct BlockRegs { pub x: u32 }

#[rseq::driver(name = "d", compatible = ["a,b"], bus = "amba")]
pub struct Drv {
    #[rseq::mmio(id = "regs")]
    pub regs: Mmio<BlockRegs>,
}

pub fn wants_raw_ptr(p: *const u32) -> u32 {
    unsafe { *p }
}
"#;

#[test]
fn map_ty_errors_on_raw_pointer() {
    with_specimen(
        UNSUPPORTED_TY_SPECIMEN,
        "mir_mapping_raw_ptr_error",
        |tcx, pass1| {
            let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
            let fn_def = find_fn_def_id(tcx, "wants_raw_ptr");
            let body = tcx.optimized_mir(fn_def.to_def_id());
            let param = body.args_iter().next().expect("one param");
            let err = map_ty(tcx, body.local_decls[param].ty, &type_def_index)
                .expect_err("raw pointer must be rejected");
            assert!(
                matches!(err, ConstructionError::UnsupportedRustType { .. }),
                "expected UnsupportedRustType, got {err:?}"
            );
        },
    );
}

// ---------------------------------------------------------------------------
// Negative: unsupported constant shape and unsupported rvalue
// ---------------------------------------------------------------------------

const UNSUPPORTED_RV_SPECIMEN: &str = r#"
#![no_std]

pub struct Mmio<T> { _m: core::marker::PhantomData<T> }
#[repr(C)]
#[rseq::register_block(endian = "little", access_width = 32)]
pub struct BlockRegs { pub x: u32 }

#[rseq::driver(name = "d", compatible = ["a,b"], bus = "amba")]
pub struct Drv {
    #[rseq::mmio(id = "regs")]
    pub regs: Mmio<BlockRegs>,
}

/// `&raw const _` emits a `Rvalue::RawPtr(Const, place)` MIR node.
pub fn raw_ptr_rvalue(x: u32) -> *const u32 {
    &raw const x
}
"#;

#[test]
fn map_rvalue_errors_on_raw_ptr_rvalue() {
    with_specimen(
        UNSUPPORTED_RV_SPECIMEN,
        "mir_mapping_raw_ptr_rvalue_error",
        |tcx, pass1| {
            let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
            let constant_index = ConstantIndex::new(&pass1.cross_refs.constants);
            let fn_def = find_fn_def_id(tcx, "raw_ptr_rvalue");
            let body = tcx.optimized_mir(fn_def.to_def_id());
            let lim = build_local_id_map(body);
            let mut interner = SymbolInterner::new();

            let mut saw_err = false;
            for bb in body.basic_blocks.iter() {
                for stmt in &bb.statements {
                    if let mir::StatementKind::Assign(box (_, mir::Rvalue::RawPtr(_, _))) =
                        &stmt.kind
                    {
                        let res = map_rvalue(
                            tcx,
                            match &stmt.kind {
                                mir::StatementKind::Assign(box (_, rv)) => rv,
                                _ => unreachable!(),
                            },
                            &lim,
                            &type_def_index,
                            &constant_index,
                            &mut interner,
                        );
                        if matches!(res, Err(ConstructionError::UnsupportedRvalue { .. })) {
                            saw_err = true;
                        }
                    }
                }
            }
            assert!(
                saw_err,
                "&raw const must produce an UnsupportedRvalue from map_rvalue"
            );
        },
    );
}

// ---------------------------------------------------------------------------
// Negative: unsupported projection (OpaqueCast / UnwrapUnsafeBinder)
// ---------------------------------------------------------------------------
//
// `mir::ProjectionElem::OpaqueCast` and `UnwrapUnsafeBinder` are
// generated by rustc features (impl Trait, unsafe binders) that driver
// specs don't use, so naturally-occurring MIR bodies won't contain
// them. We drive the error arms directly by building a `mir::Place`
// whose projection list carries those variants, then calling
// `map_place` and asserting it rejects with
// `UnsupportedProjection`.

#[test]
fn map_place_errors_on_opaque_cast_projection() {
    // Use a plain fixture (a one-local function is enough) and splice
    // an OpaqueCast projection onto the existing place via
    // `mk_place_elems`. This keeps the rustc types well-formed while
    // still triggering the error arm.
    const SPEC: &str = r#"
#![no_std]
pub struct Mmio<T> { _m: core::marker::PhantomData<T> }
#[repr(C)]
#[rseq::register_block(endian = "little", access_width = 32)]
pub struct BlockRegs { pub x: u32 }
#[rseq::driver(name = "d", compatible = ["a,b"], bus = "amba")]
pub struct Drv {
    #[rseq::mmio(id = "regs")]
    pub regs: Mmio<BlockRegs>,
}
pub fn plain(x: u32) -> u32 { x }
"#;
    with_specimen(SPEC, "mir_mapping_opaque_cast_error", |tcx, pass1| {
        let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
        let fn_def = find_fn_def_id(tcx, "plain");
        let body = tcx.optimized_mir(fn_def.to_def_id());
        let lim = build_local_id_map(body);
        let mut interner = SymbolInterner::new();

        // Build a place `_1` followed by a synthetic OpaqueCast step.
        let synth_ty = body.local_decls[mir::Local::from_u32(1)].ty;
        let place = mir::Place {
            local: mir::Local::from_u32(1),
            projection: tcx.mk_place_elems(&[mir::ProjectionElem::OpaqueCast(synth_ty)]),
        };
        let err = map_place(tcx, &place, &lim, &type_def_index, &mut interner)
            .expect_err("OpaqueCast projection must be rejected");
        assert!(
            matches!(err, ConstructionError::UnsupportedProjection { .. }),
            "expected UnsupportedProjection, got {err:?}"
        );

        // Same for UnwrapUnsafeBinder.
        let place = mir::Place {
            local: mir::Local::from_u32(1),
            projection: tcx.mk_place_elems(&[mir::ProjectionElem::UnwrapUnsafeBinder(synth_ty)]),
        };
        let err = map_place(tcx, &place, &lim, &type_def_index, &mut interner)
            .expect_err("UnwrapUnsafeBinder projection must be rejected");
        assert!(
            matches!(err, ConstructionError::UnsupportedProjection { .. }),
            "expected UnsupportedProjection, got {err:?}"
        );
    });
}

// ---------------------------------------------------------------------------
// Negative: unsupported const shape
// ---------------------------------------------------------------------------

#[test]
fn map_const_errors_on_non_scalar_shape() {
    const SPEC: &str = r#"
#![no_std]
pub struct Mmio<T> { _m: core::marker::PhantomData<T> }
#[repr(C)]
#[rseq::register_block(endian = "little", access_width = 32)]
pub struct BlockRegs { pub x: u32 }
#[rseq::driver(name = "d", compatible = ["a,b"], bus = "amba")]
pub struct Drv {
    #[rseq::mmio(id = "regs")]
    pub regs: Mmio<BlockRegs>,
}
pub const PAIR: (u32, u32) = (1, 2);
"#;
    with_specimen(SPEC, "mir_mapping_const_shape_error", |tcx, _pass1| {
        let def_id = find_const_def_id(tcx, "PAIR");
        let typing_env = ty::TypingEnv::fully_monomorphized();
        // Build a ty::Const that wraps the tuple constant's valtree --
        // a `Branch`, not a `Leaf`. `map_const` must reject this shape
        // as `UnsupportedConstShape` because the Regseq `LiteralValue`
        // space only models scalar leaves.
        let uv = ty::UnevaluatedConst::new(def_id, ty::GenericArgs::identity_for_item(tcx, def_id));
        let c = ty::Const::new_unevaluated(tcx, uv);
        let ty = tcx.type_of(def_id).instantiate_identity();
        let err = map_const(tcx, c, ty).expect_err("tuple const must be rejected");
        assert!(
            matches!(err, ConstructionError::UnsupportedConstShape { .. }),
            "expected UnsupportedConstShape, got {err:?}"
        );
        let _ = typing_env;
    });
}

// ---------------------------------------------------------------------------
// Negative: 128-bit integer rejection
// ---------------------------------------------------------------------------

#[test]
fn map_const_errors_on_128_bit_scalar_constant() {
    // A u128 const evaluates to a 128-bit scalar leaf; map_const must
    // still reject it because Regseq's `LiteralValue` has no 128-bit
    // variant (`scalar_to_literal` returns None for U128/I128).
    const SPEC: &str = r#"
#![no_std]
pub struct Mmio<T> { _m: core::marker::PhantomData<T> }
#[repr(C)]
#[rseq::register_block(endian = "little", access_width = 32)]
pub struct BlockRegs { pub x: u32 }
#[rseq::driver(name = "d", compatible = ["a,b"], bus = "amba")]
pub struct Drv {
    #[rseq::mmio(id = "regs")]
    pub regs: Mmio<BlockRegs>,
}
pub const BIG: u128 = 0xAABBCCDD_EEFF0011_22334455_66778899u128;
"#;
    with_specimen(SPEC, "mir_mapping_u128_const_error", |tcx, _pass1| {
        let def_id = find_const_def_id(tcx, "BIG");
        let uv = ty::UnevaluatedConst::new(def_id, ty::GenericArgs::identity_for_item(tcx, def_id));
        let c = ty::Const::new_unevaluated(tcx, uv);
        let ty = tcx.type_of(def_id).instantiate_identity();
        let err = map_const(tcx, c, ty).expect_err("u128 const must be rejected");
        assert!(
            matches!(err, ConstructionError::UnsupportedConstShape { .. }),
            "expected UnsupportedConstShape, got {err:?}"
        );
    });
}

#[test]
fn map_ty_errors_on_128_bit_integer() {
    const SPEC: &str = r#"
#![no_std]
pub struct Mmio<T> { _m: core::marker::PhantomData<T> }
#[repr(C)]
#[rseq::register_block(endian = "little", access_width = 32)]
pub struct BlockRegs { pub x: u32 }
#[rseq::driver(name = "d", compatible = ["a,b"], bus = "amba")]
pub struct Drv {
    #[rseq::mmio(id = "regs")]
    pub regs: Mmio<BlockRegs>,
}
pub fn wants_u128(x: u128) -> u128 { x }
"#;
    with_specimen(SPEC, "mir_mapping_u128_error", |tcx, pass1| {
        let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
        let fn_def = find_fn_def_id(tcx, "wants_u128");
        let body = tcx.optimized_mir(fn_def.to_def_id());
        let param = body.args_iter().next().expect("one param");
        let err = map_ty(tcx, body.local_decls[param].ty, &type_def_index)
            .expect_err("u128 must be rejected");
        assert!(
            matches!(err, ConstructionError::UnsupportedRustType { .. }),
            "expected UnsupportedRustType, got {err:?}"
        );
    });
}

// ---------------------------------------------------------------------------
// Positive: CastKind reductions and AggregateKind reductions
// ---------------------------------------------------------------------------

#[test]
fn map_rvalue_cast_kinds_reduce_correctly() {
    // Construct `mir::Rvalue::Cast` rvalues directly so the assertion
    // is independent of whether rustc emits the cast as
    // `CastKind::PtrToPtr` or `CastKind::PointerCoercion(...)` in a
    // given nightly (both are spec'd to reduce to Regseq's
    // `CastKind::PtrToPtr`). The Transmute arm is driven the same way.
    const SPEC: &str = r#"
#![no_std]
pub struct Mmio<T> { _m: core::marker::PhantomData<T> }
#[repr(C)]
#[rseq::register_block(endian = "little", access_width = 32)]
pub struct BlockRegs { pub x: u32 }
#[rseq::driver(name = "d", compatible = ["a,b"], bus = "amba")]
pub struct Drv {
    #[rseq::mmio(id = "regs")]
    pub regs: Mmio<BlockRegs>,
}
pub fn plain(x: u32) -> u32 { x }
"#;
    with_specimen(SPEC, "mir_mapping_cast_kinds", |tcx, pass1| {
        let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
        let constant_index = ConstantIndex::new(&pass1.cross_refs.constants);
        let fn_def = find_fn_def_id(tcx, "plain");
        let body = tcx.optimized_mir(fn_def.to_def_id());
        let lim = build_local_id_map(body);
        let mut interner = SymbolInterner::new();

        let u32_ty = tcx.types.u32;
        let i32_ty = tcx.types.i32;
        let operand_place = mir::Place {
            local: mir::Local::from_u32(1),
            projection: tcx.mk_place_elems(&[]),
        };
        let operand = mir::Operand::Copy(operand_place);

        // PtrToPtr direct
        let rv_ptr = mir::Rvalue::Cast(mir::CastKind::PtrToPtr, operand.clone(), u32_ty);
        match map_rvalue(
            tcx,
            &rv_ptr,
            &lim,
            &type_def_index,
            &constant_index,
            &mut interner,
        )
        .expect("PtrToPtr cast must map")
        {
            Rvalue::Cast(CastKind::PtrToPtr, _, _) => {}
            other => panic!("expected PtrToPtr, got {other:?}"),
        }

        // PointerCoercion also reduces to PtrToPtr per the spec table.
        let rv_coerce = mir::Rvalue::Cast(
            mir::CastKind::PointerCoercion(
                ty::adjustment::PointerCoercion::MutToConstPointer,
                mir::CoercionSource::AsCast,
            ),
            operand.clone(),
            u32_ty,
        );
        match map_rvalue(
            tcx,
            &rv_coerce,
            &lim,
            &type_def_index,
            &constant_index,
            &mut interner,
        )
        .expect("PointerCoercion cast must map")
        {
            Rvalue::Cast(CastKind::PtrToPtr, _, _) => {}
            other => panic!("PointerCoercion must reduce to PtrToPtr, got {other:?}"),
        }

        // Transmute
        let rv_trans = mir::Rvalue::Cast(mir::CastKind::Transmute, operand.clone(), i32_ty);
        match map_rvalue(
            tcx,
            &rv_trans,
            &lim,
            &type_def_index,
            &constant_index,
            &mut interner,
        )
        .expect("Transmute cast must map")
        {
            Rvalue::Cast(CastKind::Transmute, _, _) => {}
            other => panic!("expected Transmute, got {other:?}"),
        }

        // FnPtrToPtr is explicitly dropped in the IR; verify it errors.
        let rv_fn = mir::Rvalue::Cast(mir::CastKind::FnPtrToPtr, operand, u32_ty);
        let err = map_rvalue(
            tcx,
            &rv_fn,
            &lim,
            &type_def_index,
            &constant_index,
            &mut interner,
        )
        .expect_err("FnPtrToPtr must be rejected as UnsupportedRvalue");
        assert!(
            matches!(err, ConstructionError::UnsupportedRvalue { .. }),
            "expected UnsupportedRvalue, got {err:?}"
        );
    });
}

#[test]
fn map_rvalue_aggregate_kinds_cover_array_tuple_adt() {
    with_specimen(
        BODY_SPECIMEN,
        "mir_mapping_aggregate_kinds",
        |tcx, pass1| {
            let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
            let constant_index = ConstantIndex::new(&pass1.cross_refs.constants);
            let fn_def = find_fn_def_id(tcx, "body_probe");
            let body = tcx.optimized_mir(fn_def.to_def_id());
            let lim = build_local_id_map(body);
            let mut interner = SymbolInterner::new();

            let mut saw_array = false;
            let mut saw_tuple = false;
            let mut saw_adt = false;
            for bb in body.basic_blocks.iter() {
                for stmt in &bb.statements {
                    if let mir::StatementKind::Assign(box (_place, rvalue)) = &stmt.kind {
                        if let mir::Rvalue::Aggregate(_, _) = rvalue {
                            if let Ok(Rvalue::Aggregate(kind, _)) = map_rvalue(
                                tcx,
                                rvalue,
                                &lim,
                                &type_def_index,
                                &constant_index,
                                &mut interner,
                            ) {
                                match kind {
                                    regseq_model_old::ir::AggregateKind::Array(_) => saw_array = true,
                                    regseq_model_old::ir::AggregateKind::Tuple => saw_tuple = true,
                                    regseq_model_old::ir::AggregateKind::Adt { .. } => saw_adt = true,
                                }
                            }
                        }
                    }
                }
            }
            assert!(saw_array, "`[7, 8, 9]` must map to AggregateKind::Array");
            assert!(saw_tuple, "`(1, true)` must map to AggregateKind::Tuple");
            assert!(
                saw_adt,
                "`Pair {{ lo: 1, hi: 2 }}` must map to AggregateKind::Adt"
            );
        },
    );
}
