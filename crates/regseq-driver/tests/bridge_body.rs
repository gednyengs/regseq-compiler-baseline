#![feature(rustc_private)]

extern crate rustc_abi;
extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_index;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, Once};
use std::time::{SystemTime, UNIX_EPOCH};

use regseq_driver::bridge::body::{
    lower_runtime_check_operand, translate_rvalue, translate_statement, BodyCtxt,
};
use regseq_driver::bridge::{BridgeCtxt, BridgeError, BridgeInput, DslRejectionKind};
use regseq_model::{
    AggregateKind, BinOp, NonDivergingIntrinsic, Operand, PlaceElem, Provenance, Rvalue,
    SourceFileId, Statement, StatementKind,
};
use rustc_hir::def_id::{LocalDefId, LocalModDefId};
use rustc_middle::mir;
use rustc_middle::mir::coverage::{BasicCoverageBlock, CoverageKind};
use rustc_middle::ty::{self, TyCtxt};

static INIT: Once = Once::new();
static COMPILER_LOCK: Mutex<()> = Mutex::new(());

fn init_rustc() {
    INIT.call_once(|| {
        let early_dcx =
            rustc_session::EarlyDiagCtxt::new(rustc_session::config::ErrorOutputType::default());
        rustc_driver::init_rustc_env_logger(&early_dcx);
    });
}

#[test]
fn rejects_statement_site_rows_with_row_specific_messages() {
    let source = "pub fn rows() {}\n";

    with_bridge("bridge_body_statement_rejections", source, |tcx, cx| {
        let body = tcx.optimized_mir(find_fn_def_id(tcx, "rows").to_def_id());
        let source_info = body.basic_blocks[mir::START_BLOCK].terminator().source_info;
        let place = || Box::new(mir::Place::from(mir::RETURN_PLACE));
        let mut body_cx = BodyCtxt::new(&body.local_decls);

        let cases = [
            mir::StatementKind::FakeRead(Box::new((
                mir::FakeReadCause::ForMatchGuard,
                mir::Place::from(mir::RETURN_PLACE),
            ))),
            mir::StatementKind::AscribeUserType(
                Box::new((
                    mir::Place::from(mir::RETURN_PLACE),
                    mir::UserTypeProjection {
                        base: ty::UserTypeAnnotationIndex::from_u32(0),
                        projs: Vec::new(),
                    },
                )),
                ty::Variance::Invariant,
            ),
            mir::StatementKind::BackwardIncompatibleDropHint {
                place: place(),
                reason: mir::BackwardIncompatibleDropReason::Edition2024,
            },
            mir::StatementKind::Coverage(CoverageKind::SpanMarker),
            mir::StatementKind::Coverage(CoverageKind::VirtualCounter {
                bcb: BasicCoverageBlock::from_u32(0),
            }),
            mir::StatementKind::PlaceMention(place()),
            mir::StatementKind::Nop,
            mir::StatementKind::StorageLive(mir::RETURN_PLACE),
            mir::StatementKind::StorageDead(mir::RETURN_PLACE),
            mir::StatementKind::Retag(mir::RetagKind::Default, place()),
            mir::StatementKind::ConstEvalCounter,
            mir::StatementKind::Assign(Box::new((
                mir::Place::from(mir::RETURN_PLACE),
                mir::Rvalue::Ref(
                    tcx.lifetimes.re_erased,
                    mir::BorrowKind::Fake(mir::FakeBorrowKind::Shallow),
                    mir::Place::from(mir::RETURN_PLACE),
                ),
            ))),
        ];

        for kind in cases {
            let err = translate_statement(cx, &mut body_cx, mir::Statement::new(source_info, kind))
                .expect_err("statement-site row should reject");
            let BridgeError::DslRejection {
                kind: DslRejectionKind::Statement,
                ty_repr,
            } = err
            else {
                panic!("expected statement DSL rejection, got {err:?}");
            };
            assert!(!ty_repr.is_empty());
            assert_ne!(ty_repr, "unsupported statement");
        }
    });
}

fn get_sysroot() -> String {
    let output = Command::new("rustc")
        .arg("--print")
        .arg("sysroot")
        .output()
        .expect("failed to run rustc --print sysroot");
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn fresh_test_dir(crate_name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("regseq-bridge-body-{crate_name}-{nanos}"))
}

fn with_bridge<R, F>(crate_name: &str, source: &str, f: F) -> R
where
    R: Send + 'static,
    F: for<'tcx> FnOnce(TyCtxt<'tcx>, &mut BridgeCtxt<'tcx>) -> R + Send + 'static,
{
    let _lock = COMPILER_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    init_rustc();

    let test_dir = fresh_test_dir(crate_name);
    std::fs::create_dir_all(&test_dir).unwrap();
    let src_path = test_dir.join("lib.rs");
    std::fs::write(&src_path, source).unwrap();

    let result = Arc::new(Mutex::new(None));
    let callback =
        Arc::new(Mutex::new(Some(Box::new(f)
            as Box<
                dyn for<'tcx> FnOnce(TyCtxt<'tcx>, &mut BridgeCtxt<'tcx>) -> R + Send,
            >)));

    struct Callbacks<R: Send + 'static> {
        result: Arc<Mutex<Option<R>>>,
        callback: Arc<
            Mutex<
                Option<Box<dyn for<'tcx> FnOnce(TyCtxt<'tcx>, &mut BridgeCtxt<'tcx>) -> R + Send>>,
            >,
        >,
    }

    impl<R: Send + 'static> rustc_driver::Callbacks for Callbacks<R> {
        fn config(&mut self, config: &mut rustc_interface::interface::Config) {
            config.opts.unstable_opts.no_codegen = true;
            config.opts.unstable_opts.mir_opt_level = Some(0);
            config.opts.unstable_opts.crate_attr.push(
                "allow(unused, non_snake_case, nonstandard_style, improper_ctypes, private_interfaces)"
                    .to_string(),
            );
        }

        fn after_analysis<'tcx>(
            &mut self,
            _compiler: &rustc_interface::interface::Compiler,
            tcx: TyCtxt<'tcx>,
        ) -> rustc_driver::Compilation {
            let input = BridgeInput::new(tcx);
            let mut cx = BridgeCtxt::new(input);
            let f = self.callback.lock().unwrap().take().expect("callback once");
            *self.result.lock().unwrap() = Some(f(tcx, &mut cx));
            rustc_driver::Compilation::Stop
        }
    }

    let sysroot = get_sysroot();
    let args = vec![
        "rustc".to_string(),
        src_path.to_str().unwrap().to_string(),
        "--sysroot".to_string(),
        sysroot,
        "--edition".to_string(),
        "2021".to_string(),
        "--crate-type".to_string(),
        "rlib".to_string(),
        "--crate-name".to_string(),
        crate_name.to_string(),
        "--out-dir".to_string(),
        test_dir.to_str().unwrap().to_string(),
    ];

    let mut callbacks = Callbacks {
        result: result.clone(),
        callback,
    };
    rustc_driver::run_compiler(&args, &mut callbacks);
    let output = result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis should produce a result");
    std::fs::remove_dir_all(Path::new(&test_dir)).ok();
    output
}

fn find_fn_def_id(tcx: TyCtxt<'_>, name: &str) -> LocalDefId {
    for item_id in tcx
        .hir_module_items(LocalModDefId::CRATE_DEF_ID)
        .free_items()
    {
        let item = tcx.hir_item(item_id);
        if let rustc_hir::ItemKind::Fn { ident, .. } = &item.kind {
            if ident.name.as_str() == name {
                return item.owner_id.def_id;
            }
        }
    }
    panic!("function `{name}` not found");
}

fn translate_fn_statements<'tcx>(
    tcx: TyCtxt<'tcx>,
    cx: &mut BridgeCtxt<'tcx>,
    name: &str,
) -> (Vec<Statement>, Vec<String>) {
    let body = tcx.optimized_mir(find_fn_def_id(tcx, name).to_def_id());
    let mut body_cx = BodyCtxt::new(&body.local_decls);
    let mut statements = Vec::new();
    let mut rejections = Vec::new();

    for stmt in body
        .basic_blocks
        .iter()
        .flat_map(|block| block.statements.iter().cloned())
    {
        match translate_statement(cx, &mut body_cx, stmt) {
            Ok(Some(statement)) => statements.push(statement),
            Ok(None) => {}
            Err(BridgeError::DslRejection {
                kind: DslRejectionKind::Statement,
                ty_repr,
            }) => rejections.push(ty_repr),
            Err(error) => panic!("unexpected body translation error: {error:?}"),
        }
    }

    (statements, rejections)
}

#[test]
fn translates_operand_and_statement_rows_from_mir() {
    let source = r#"
pub struct NeedsDrop(pub u32);
impl Drop for NeedsDrop {
    fn drop(&mut self) {}
}

pub struct PlainNonCopy(pub u32);

pub fn rows(x: u32, y: u32, s: NeedsDrop, p: PlainNonCopy) -> u32 {
    let c = 1u32;
    let copied = x;
    let z = x & y;
    let _moved = s;
    let _plain = p;
    z + c + copied
}
"#;

    with_bridge("bridge_body_rows", source, |tcx, cx| {
        let (statements, rejections) = translate_fn_statements(tcx, cx, "rows");

        assert!(statements.iter().any(|stmt| matches!(
            &stmt.kind,
            StatementKind::Assign(_, Rvalue::Use(Operand::Constant(_)))
        )));
        assert!(statements.iter().any(|stmt| matches!(
            &stmt.kind,
            StatementKind::Assign(_, Rvalue::Use(Operand::Copy(_)))
        )));
        assert!(statements.iter().any(|stmt| matches!(
            &stmt.kind,
            StatementKind::Assign(_, Rvalue::BinaryOp(BinOp::BitAnd, _, _))
        )));
        assert!(statements.iter().any(|stmt| matches!(
            &stmt.kind,
            StatementKind::Assign(_, Rvalue::Use(Operand::Move(_)))
        )));
        assert!(cx.drop_glue_synth_cache.len() >= 2);
        assert!(
            statements
                .iter()
                .any(|stmt| stmt.span.file != SourceFileId::SYNTHESIZED
                    && stmt.span.hi > stmt.span.lo)
        );
        assert!(rejections.iter().any(|msg| msg.contains("StorageLive")));
        assert!(rejections.iter().any(|msg| msg.contains("StorageDead")));
    });
}

#[test]
fn translates_representative_place_and_rvalue_rows_from_mir() {
    let source = r#"
pub struct S {
    pub x: u32,
    pub y: u32,
}

pub enum E {
    A,
    B { v: u32 },
}

pub union U {
    pub a: u32,
}

pub fn rows(s: &S, arr: &[u32; 4], i: usize, e: E, b: bool, n: i32) -> (u32, [u32; 2], *const u32, u64, bool, i32) {
    let field = s.x;
    let field_ref = &s.x;
    let field_again = *field_ref;
    let mut local = field;
    let unique_ref = &mut local;
    let through_unique = *unique_ref;
    let indexed = arr[i];
    let array_lit = [field, indexed];
    let struct_lit = S { x: field, y: indexed };
    let enum_lit = E::B { v: field };
    let union_lit = U { a: indexed };
    let closure_lit = || field;
    let _aggregate_use = (array_lit, struct_lit.x, matches!(enum_lit, E::B { .. }), unsafe { union_lit.a }, closure_lit());
    let repeated = [field; 2];
    let raw = &raw const s.y;
    let casted = indexed as u64;
    let inverted = !b;
    let negated = -n;
    let discr = core::mem::discriminant(&e);
    match e {
        E::B { v } => (v + field_again + through_unique, repeated, raw, casted, inverted, negated),
        E::A => (field_again + through_unique, repeated, raw, casted, inverted, negated),
    }
}
"#;

    with_bridge("bridge_body_places_rvalues", source, |tcx, cx| {
        let (statements, _) = translate_fn_statements(tcx, cx, "rows");
        let mut elems = Vec::new();
        let mut rvalues = Vec::new();

        for statement in &statements {
            if let StatementKind::Assign(place, rvalue) = &statement.kind {
                elems.extend(place.projection.iter().cloned());
                collect_rvalue_places(rvalue, &mut elems);
                rvalues.push(rvalue);
            }
        }

        assert!(elems.iter().any(|elem| matches!(elem, PlaceElem::Deref)));
        assert!(elems
            .iter()
            .any(|elem| matches!(elem, PlaceElem::Field(_, _))));
        assert!(elems.iter().any(|elem| matches!(elem, PlaceElem::Index(_))));
        assert!(elems
            .iter()
            .any(|elem| matches!(elem, PlaceElem::Downcast(_))));
        assert!(rvalues.iter().any(|rv| matches!(rv, Rvalue::Repeat(_, 2))));
        assert!(rvalues.iter().any(|rv| matches!(rv, Rvalue::Ref(_, _))));
        assert!(rvalues
            .iter()
            .any(|rv| matches!(rv, Rvalue::Ref(Provenance::UniqueBorrow, _))));
        assert!(rvalues.iter().any(|rv| matches!(rv, Rvalue::RawPtr(_, _))));
        assert!(rvalues.iter().any(|rv| matches!(rv, Rvalue::Cast(_, _, _))));
        assert!(rvalues.iter().any(|rv| matches!(rv, Rvalue::UnaryOp(_, _))));
        assert!(rvalues
            .iter()
            .any(|rv| matches!(rv, Rvalue::Aggregate(AggregateKind::Tuple(_), _))));
        assert!(rvalues
            .iter()
            .any(|rv| matches!(rv, Rvalue::Aggregate(AggregateKind::Array(_), _))));
        assert!(rvalues
            .iter()
            .any(|rv| matches!(rv, Rvalue::Aggregate(AggregateKind::Struct(_), _))));
        assert!(rvalues
            .iter()
            .any(|rv| matches!(rv, Rvalue::Aggregate(AggregateKind::Enum(_, _), _))));
        assert!(rvalues
            .iter()
            .any(|rv| matches!(rv, Rvalue::Aggregate(AggregateKind::Union(_, _), _))));
        assert!(rvalues
            .iter()
            .any(|rv| matches!(rv, Rvalue::Aggregate(AggregateKind::Closure(_), _))));
    });
}

#[test]
fn translates_runtime_checks_intrinsic_set_discriminant_and_slice_projection_rows() {
    let source = r#"
#![feature(core_intrinsics)]

pub enum E {
    A,
    B(u32),
}

pub fn set_row() -> E {
    let mut e = E::A;
    e = E::B(1);
    e
}

pub unsafe fn assume_row(b: bool) {
    core::intrinsics::assume(b);
}

pub unsafe fn copy_nonoverlapping_row(src: *const u32, dst: *mut u32, count: usize) {
    core::intrinsics::copy_nonoverlapping(src, dst, count);
}

pub fn runtime_checks_row() -> bool {
    core::intrinsics::ub_checks()
}

pub fn discriminant_row(e: E) -> u8 {
    let d = unsafe { core::intrinsics::discriminant_value(&e) };
    d as u8
}

pub fn slice_rows(xs: &[u32]) -> u32 {
    match xs {
        [first, middle @ .., last] => *first + *last + middle.len() as u32,
        _ => 0,
    }
}

pub fn rawptr_aggregate_row(data: *const u32, meta: ()) {
    let _ = (data, meta);
}
"#;

    with_bridge("bridge_body_remaining_rows", source, |tcx, cx| {
        let lowered = lower_runtime_check_operand(
            cx,
            &mut BodyCtxt::empty(),
            rustc_middle::mir::Operand::RuntimeChecks(rustc_middle::mir::RuntimeChecks::UbChecks),
        )
        .unwrap();
        assert!(matches!(lowered, Operand::Constant(_)));

        let body = tcx.optimized_mir(find_fn_def_id(tcx, "set_row").to_def_id());
        let source_info = body.basic_blocks[mir::START_BLOCK]
            .statements
            .first()
            .expect("fixture should have a statement")
            .source_info;
        let set_stmt = mir::Statement::new(
            source_info,
            mir::StatementKind::SetDiscriminant {
                place: Box::new(mir::Place::from(mir::RETURN_PLACE)),
                variant_index: rustc_abi::VariantIdx::from_u32(0),
            },
        );
        let mut body_cx = BodyCtxt::new(&body.local_decls);
        let set_statement = translate_statement(cx, &mut body_cx, set_stmt)
            .unwrap()
            .expect("SetDiscriminant should translate");
        assert!(matches!(
            set_statement.kind,
            StatementKind::SetDiscriminant(_, _)
        ));

        let (assume_statements, _) = translate_fn_statements(tcx, cx, "assume_row");
        assert!(assume_statements
            .iter()
            .any(|stmt| matches!(stmt.kind, StatementKind::Intrinsic(_))));

        let (copy_statements, _) = translate_fn_statements(tcx, cx, "copy_nonoverlapping_row");
        assert!(copy_statements.iter().any(|stmt| matches!(
            &stmt.kind,
            StatementKind::Intrinsic(NonDivergingIntrinsic::CopyNonOverlapping(_))
        )));

        let (runtime_check_statements, _) = translate_fn_statements(tcx, cx, "runtime_checks_row");
        assert!(runtime_check_statements.iter().any(|stmt| matches!(
            &stmt.kind,
            StatementKind::Assign(_, Rvalue::Use(Operand::Constant(_)))
        )));

        let (discriminant_statements, _) = translate_fn_statements(tcx, cx, "discriminant_row");
        assert!(discriminant_statements.iter().any(|stmt| matches!(
            &stmt.kind,
            StatementKind::Assign(_, Rvalue::Discriminant(_))
        )));

        let (slice_statements, _) = translate_fn_statements(tcx, cx, "slice_rows");
        let mut elems = Vec::new();
        for statement in &slice_statements {
            if let StatementKind::Assign(place, rvalue) = &statement.kind {
                elems.extend(place.projection.iter().cloned());
                collect_rvalue_places(rvalue, &mut elems);
            }
        }
        assert!(elems
            .iter()
            .any(|elem| matches!(elem, PlaceElem::ConstantIndex { .. })));
        assert!(elems
            .iter()
            .any(|elem| matches!(elem, PlaceElem::Subslice { .. })));

        let body = tcx.optimized_mir(find_fn_def_id(tcx, "rawptr_aggregate_row").to_def_id());
        let mut body_cx = BodyCtxt::new(&body.local_decls);
        let rawptr = mir::Rvalue::Aggregate(
            Box::new(mir::AggregateKind::RawPtr(
                tcx.types.u32,
                rustc_ast::Mutability::Not,
            )),
            rustc_index::IndexVec::from_raw(vec![
                mir::Operand::Copy(mir::Place::from(mir::Local::from_u32(1))),
                mir::Operand::Copy(mir::Place::from(mir::Local::from_u32(2))),
            ]),
        );
        let translated = translate_rvalue(cx, &mut body_cx, rawptr).unwrap();
        assert!(matches!(
            translated,
            Rvalue::Aggregate(AggregateKind::RawPtr(_, _), _)
        ));
    });
}

fn collect_rvalue_places(rvalue: &Rvalue, elems: &mut Vec<PlaceElem>) {
    match rvalue {
        Rvalue::Use(operand) | Rvalue::Repeat(operand, _) | Rvalue::UnaryOp(_, operand) => {
            collect_operand_places(operand, elems);
        }
        Rvalue::Ref(_, place) | Rvalue::RawPtr(_, place) | Rvalue::Discriminant(place) => {
            elems.extend(place.projection.iter().cloned());
        }
        Rvalue::Cast(_, operand, _) => collect_operand_places(operand, elems),
        Rvalue::BinaryOp(_, left, right) => {
            collect_operand_places(left, elems);
            collect_operand_places(right, elems);
        }
        Rvalue::Aggregate(_, operands) => {
            for operand in operands {
                collect_operand_places(operand, elems);
            }
        }
    }
}

fn collect_operand_places(operand: &Operand, elems: &mut Vec<PlaceElem>) {
    match operand {
        Operand::Copy(place) | Operand::Move(place) => {
            elems.extend(place.projection.iter().cloned());
        }
        Operand::Constant(_) => {}
    }
}
