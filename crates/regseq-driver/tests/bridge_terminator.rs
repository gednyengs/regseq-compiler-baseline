#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, Once};
use std::time::{SystemTime, UNIX_EPOCH};

use regseq_driver::bridge::body::{translate_basic_block, translate_terminator, BodyCtxt};
use regseq_driver::bridge::{BridgeCtxt, BridgeError, BridgeInput, DslRejectionKind};
use regseq_model::{
    Body, CallCallee, FunctionInstanceFlavor, Mutability, Rvalue, Statement, StatementKind,
    SyntheticItemKind, Terminator, TerminatorKind, Ty,
};
use rustc_hir::def_id::{LocalDefId, LocalModDefId};
use rustc_middle::mir;
use rustc_middle::ty::TyCtxt;

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
    std::env::temp_dir().join(format!("regseq-bridge-terminator-{crate_name}-{nanos}"))
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
            // mir_opt_level >= 1 enables `RemoveStorageMarkers` so
            // basic-block translation does not trip on
            // StorageLive/StorageDead rejections inside Drop fixture
            // bodies that exercise audit finding 42-3 (the
            // Drop-as-Call structural prelude).
            config.opts.unstable_opts.mir_opt_level = Some(1);
            config
                .opts
                .unstable_opts
                .crate_attr
                .push("allow(unused, unconditional_panic, arithmetic_overflow)".to_string());
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

fn translate_fn_terminators<'tcx>(
    tcx: TyCtxt<'tcx>,
    cx: &mut BridgeCtxt<'tcx>,
    name: &str,
) -> Vec<Terminator> {
    let body = tcx.optimized_mir(find_fn_def_id(tcx, name).to_def_id());
    let mut model_body = Body::default();
    let mut statements: Vec<Statement> = Vec::new();
    body.basic_blocks
        .iter()
        .filter_map(|block| block.terminator.clone())
        .map(|term| {
            let mut body_cx = BodyCtxt::new_with_model_body(&body.local_decls, &mut model_body);
            translate_terminator(cx, &mut body_cx, &mut statements, term).unwrap()
        })
        .collect()
}

/// Translate every basic block's Drop terminator using the
/// basic-block entry point so the structural prelude (RawPtr Assign)
/// is appended to a fresh statement accumulator per call and the
/// returned (statements, terminator) pair models the post-collapse
/// shape per spec section 23.4.
fn rustc_drop_translations<'tcx>(
    tcx: TyCtxt<'tcx>,
    cx: &mut BridgeCtxt<'tcx>,
    name: &str,
) -> Vec<(Vec<Statement>, Terminator)> {
    let body = tcx.optimized_mir(find_fn_def_id(tcx, name).to_def_id());
    let mut model_body = Body::default();
    body.basic_blocks
        .iter()
        .filter_map(|block| block.terminator.clone())
        .filter(|term| matches!(term.kind, mir::TerminatorKind::Drop { .. }))
        .map(|term| {
            let mut statements: Vec<Statement> = Vec::new();
            let mut body_cx = BodyCtxt::new_with_model_body(&body.local_decls, &mut model_body);
            let translated = translate_terminator(cx, &mut body_cx, &mut statements, term).unwrap();
            (statements, translated)
        })
        .collect()
}

#[test]
fn translates_non_drop_terminator_rows_from_mir() {
    let source = r#"
pub fn callee() {}

pub fn rows(x: u32, b: bool) -> u32 {
    callee();
    let y = if b { x + 1 } else { x - 1 };
    match y {
        0 => 1,
        1 => 2,
        _ => y,
    }
}
"#;

    with_bridge("bridge_terminator_rows", source, |tcx, cx| {
        // Pre-seed type translation for every reachable function so
        // pending Items have real ItemIds before the call-callee
        // resolution path reads `model.items[item]`. Production
        // achieves this via Phase-2 + Phase-4; tests must drive both.
        for fn_name in ["callee", "rows"] {
            let body = tcx.optimized_mir(find_fn_def_id(tcx, fn_name).to_def_id());
            for local in body.local_decls.iter() {
                let _ = regseq_driver::bridge::ty::translate_ty(cx, local.ty);
            }
            for block in body.basic_blocks.iter() {
                if let Some(term) = &block.terminator {
                    if let mir::TerminatorKind::Call { func, .. } = &term.kind {
                        let func_ty = func.ty(&body.local_decls, tcx);
                        let _ = regseq_driver::bridge::ty::translate_ty(cx, func_ty);
                    }
                }
            }
        }
        regseq_driver::bridge::flush_pending_items_for_tests(cx).unwrap();

        let terms = translate_fn_terminators(tcx, cx, "rows");

        assert!(terms
            .iter()
            .any(|term| matches!(term.kind, TerminatorKind::Goto(_))));
        assert!(terms
            .iter()
            .any(|term| matches!(term.kind, TerminatorKind::SwitchInt { .. })));
        assert!(terms
            .iter()
            .any(|term| matches!(term.kind, TerminatorKind::Return)));
        assert!(terms.iter().any(|term| matches!(
            term.kind,
            TerminatorKind::Call {
                callee: CallCallee::Direct(_),
                ..
            }
        )));
        assert!(terms
            .iter()
            .any(|term| matches!(term.kind, TerminatorKind::Assert { .. })));
    });
}

#[test]
fn translates_unreachable_and_rejects_false_edge() {
    let source = "pub fn f() {}\n";

    with_bridge("bridge_terminator_reject", source, |tcx, cx| {
        let body = tcx.optimized_mir(find_fn_def_id(tcx, "f").to_def_id());
        let source_info = body.basic_blocks[mir::START_BLOCK].terminator().source_info;
        let mut model_body = Body::default();
        let mut statements: Vec<Statement> = Vec::new();
        let mut body_cx = BodyCtxt::new_with_model_body(&body.local_decls, &mut model_body);
        let unreachable = mir::Terminator {
            source_info,
            kind: mir::TerminatorKind::Unreachable,
        };
        let translated =
            translate_terminator(cx, &mut body_cx, &mut statements, unreachable).unwrap();
        assert!(matches!(translated.kind, TerminatorKind::Unreachable));

        let false_edge = mir::Terminator {
            source_info,
            kind: mir::TerminatorKind::FalseEdge {
                real_target: mir::BasicBlock::from_u32(0),
                imaginary_target: mir::BasicBlock::from_u32(0),
            },
        };
        let err = translate_terminator(cx, &mut body_cx, &mut statements, false_edge)
            .expect_err("FalseEdge should reject");
        assert!(matches!(
            err,
            BridgeError::DslRejection {
                kind: DslRejectionKind::Terminator,
                ..
            }
        ));
    });
}

#[test]
fn collapses_drop_terminators_to_cached_direct_calls() {
    let source = r#"
pub struct S(pub u32);
impl Drop for S {
    fn drop(&mut self) {}
}

pub fn one_drop() {
    let _s = S(0);
}

pub fn two_drops() {
    let _a = S(0);
    let _b = S(1);
}
"#;

    with_bridge("bridge_terminator_drop", source, |tcx, cx| {
        // Drive Phase-3/4 so the drop-glue ItemHandle resolves to a
        // real ItemId before the Drop-as-Call collapse looks it up.
        // The translator otherwise sees the synth handle in
        // `cx.drop_glue_synth_cache` but cannot resolve to a final
        // ItemId without an assignment pass having run.
        let one_drop_fn = find_fn_def_id(tcx, "one_drop");
        let _ = tcx.optimized_mir(one_drop_fn.to_def_id());
        let two_drop_fn = find_fn_def_id(tcx, "two_drops");
        let _ = tcx.optimized_mir(two_drop_fn.to_def_id());
        // Pre-walk the bodies so all Items / drop-glue handles are
        // registered before we invoke Phase 3/4.
        prewalk_for_synthesis(tcx, cx, "one_drop");
        prewalk_for_synthesis(tcx, cx, "two_drops");
        regseq_driver::bridge::flush_pending_items_for_tests(cx).unwrap();
        regseq_driver::bridge::synth::rekey_synthesis_in_phase5(cx).unwrap();

        let one = rustc_drop_translations(tcx, cx, "one_drop");
        assert_eq!(one.len(), 1);
        let (one_stmts, one_term) = &one[0];
        // Audit-finding regseq-driver-42-3: the Drop-as-Call collapse
        // prepends a `Statement::Assign(_t_drop_arg,
        // Rvalue::RawPtr(Mut, place))` to the statement accumulator
        // BEFORE the synthesized Call terminator.
        assert!(matches!(
            one_stmts
                .last()
                .map(|s| &s.kind)
                .expect("Drop collapse should append a RawPtr prelude statement"),
            StatementKind::Assign(_, Rvalue::RawPtr(Mutability::Mut, _))
        ));
        let TerminatorKind::Call {
            callee: CallCallee::Direct(first_callee),
            args,
            destination,
            target: Some(_),
            ..
        } = &one_term.kind
        else {
            panic!("drop should collapse to a direct call with a normal target");
        };
        assert_eq!(args.len(), 1);
        // Destination must be the synthetic unit-typed return slot,
        // NOT `LocalId::RETURN_PLACE`. Per audit-finding 42-3.
        assert_ne!(destination.local, regseq_model::LocalId::RETURN_PLACE);
        assert!(
            cx.pending_items
                .iter()
                .any(|item| matches!(item.synthetic_kind, SyntheticItemKind::DropGlue { .. }))
                || cx
                    .model
                    .item_synthetic_kinds
                    .iter()
                    .any(|kind| matches!(kind, SyntheticItemKind::DropGlue { .. }))
        );

        // Resolved Instance is Function-flavored DropGlue.
        let inst = &cx.model.instances[*first_callee];
        let function = inst
            .kind
            .as_function()
            .expect("drop-glue InstanceId must resolve to a Function Instance");
        assert_eq!(function.flavor, FunctionInstanceFlavor::DropGlue);

        let two = rustc_drop_translations(tcx, cx, "two_drops");
        assert!(two.len() >= 2);
        let callees = two
            .iter()
            .map(|(_, term)| match &term.kind {
                TerminatorKind::Call {
                    callee: CallCallee::Direct(callee),
                    ..
                } => *callee,
                other => panic!("expected drop call, got {other:?}"),
            })
            .collect::<Vec<_>>();
        assert!(callees.iter().all(|callee| callee == first_callee));
    });
}

fn prewalk_for_synthesis<'tcx>(tcx: TyCtxt<'tcx>, cx: &mut BridgeCtxt<'tcx>, name: &str) {
    // Trigger drop-glue synthesis (and any type translation) for
    // every Drop terminator's place type in the function body so the
    // synth caches have entries before the basic-block translator
    // runs. Production drives this via the `run_bridge` Phase-2 walk;
    // tests need to seed the caches manually before the test invokes
    // `flush_pending_items_for_tests`.
    let body = tcx.optimized_mir(find_fn_def_id(tcx, name).to_def_id());
    for block in body.basic_blocks.iter() {
        let Some(term) = &block.terminator else {
            continue;
        };
        if let mir::TerminatorKind::Drop { place, .. } = &term.kind {
            let place_ty = place.ty(&body.local_decls, tcx).ty;
            let Ok(model_ty) = regseq_driver::bridge::ty::translate_ty(cx, place_ty) else {
                continue;
            };
            let _ = regseq_driver::bridge::synth::synthesise_drop_glue(cx, model_ty);
        }
    }
}

#[test]
fn translates_basic_block_statements_and_terminator() {
    let source = "pub fn f() {}\n";

    with_bridge("bridge_basic_block", source, |tcx, cx| {
        let body = tcx.optimized_mir(find_fn_def_id(tcx, "f").to_def_id());
        let mut body_cx = BodyCtxt::new(&body.local_decls);
        let block = translate_basic_block(cx, &mut body_cx, &body.basic_blocks[mir::START_BLOCK])
            .expect("basic block should translate");

        assert_eq!(
            block.statements.len(),
            body.basic_blocks[mir::START_BLOCK].statements.len()
        );
        assert!(matches!(block.terminator.kind, TerminatorKind::Return));
    });
}

#[test]
fn closure_trait_method_call_rewrites_to_synth_closure_call_shim() {
    // Audit-finding F1c (regseq-driver-37 v4 task spec lines 286-299
    // + spec.md section 13.1): every `Fn::call` / `FnMut::call_mut` /
    // `FnOnce::call_once` direct call against a closure captures
    // struct must be lowered to the synth `ClosureCall` Function Item
    // with positional (un-tupled) arguments.
    let source = r#"
pub fn caller() -> u32 {
    let c = |x: u32| x + 1;
    c(7)
}
"#;

    with_bridge("bridge_terminator_closure_call_shim", source, |tcx, cx| {
        // Iterate every reachable function body looking for a call
        // terminator whose callee Instance maps back to a
        // synth-recorded ClosureCall ItemHandle. Pre-translate
        // every call's func type so the late assignment pass can
        // bind handles before the shim rewrite resolves them.
        let mut saw_shim_callee = false;
        let mut saw_positional_args = false;
        for def_id in tcx.hir_body_owners() {
            if !matches!(tcx.def_kind(def_id), rustc_hir::def::DefKind::Fn) {
                continue;
            }
            let body = tcx.optimized_mir(def_id.to_def_id());
            let mut model_body = Body::default();
            let mut statements: Vec<Statement> = Vec::new();
            let mut body_cx = BodyCtxt::new_with_model_body(&body.local_decls, &mut model_body);
            for block in body.basic_blocks.iter() {
                let Some(term) = block.terminator.clone() else {
                    continue;
                };
                if !matches!(term.kind, mir::TerminatorKind::Call { .. }) {
                    continue;
                }
                let translated = match translate_terminator(cx, &mut body_cx, &mut statements, term)
                {
                    Ok(t) => t,
                    Err(_) => continue,
                };
                let TerminatorKind::Call {
                    callee: CallCallee::Direct(instance_id),
                    args,
                    ..
                } = translated.kind
                else {
                    continue;
                };
                // The Direct callee InstanceId post-Codex-fix
                // references a real model.instances entry; resolve
                // back to the Item and check whether that Item is
                // one of the ClosureCall synth handles.
                if instance_id.index() >= cx.model.instances.len() {
                    continue;
                }
                let inst_item = cx.model.instances[instance_id].item;
                let matches_closure_call = cx
                    .closure_synth_cache
                    .iter()
                    .flat_map(|handles| handles.calls.iter().copied())
                    .any(|handle| cx.handles.resolve(handle) == Some(inst_item));
                if matches_closure_call {
                    saw_shim_callee = true;
                    // Spec 13.1: receiver + tuple-flattened
                    // positional args. The single-arg closure
                    // `|x| x + 1` yields receiver + one
                    // positional u32 = 2 args.
                    if args.len() >= 2 {
                        saw_positional_args = true;
                    }
                }
            }
        }
        assert!(
            saw_shim_callee,
            "expected at least one Fn::call/FnMut::call_mut/FnOnce::call_once \
                 terminator to rewrite to a synth ClosureCall callee",
        );
        assert!(
            saw_positional_args,
            "closure-call shim must untuple the trailing args tuple into \
                 positional inputs (receiver + per-tuple-field args)",
        );
        // Cross-check: at least one ClosureCall synth item exists,
        // confirming synth/closure.rs ran and the shim rewrite
        // found its handle. The Codex-review fix may have drained
        // newly synthesized pending items into `model.items` via
        // the late-assignment path, so accept either location.
        let pending_has_closure_call = cx.pending_items.iter().any(|pending| {
            matches!(
                pending.synthetic_kind,
                SyntheticItemKind::ClosureCall { .. }
            )
        });
        let model_has_closure_call = cx
            .model
            .item_synthetic_kinds
            .iter()
            .any(|kind| matches!(kind, SyntheticItemKind::ClosureCall { .. }));
        assert!(
            pending_has_closure_call || model_has_closure_call,
            "synth/closure.rs must register at least one ClosureCall synthetic item"
        );
    });
}

/// Audit finding regseq-driver-42-3: the Drop-as-Call collapse must
/// allocate two synthetic locals on the in-flight model `Body`
/// (`_t_drop_arg: *mut place_ty` and `_t_drop_ret: ()`) and prepend
/// `Statement::Assign(_t_drop_arg, Rvalue::RawPtr(Mut, place))` to
/// the basic block's statement accumulator. The Call's destination
/// is `Place::from(_t_drop_ret)`, NOT `LocalId::RETURN_PLACE`.
#[test]
fn drop_as_call_appends_raw_ptr_prelude_and_allocates_synthetic_locals() {
    let source = r#"
pub struct S(pub u32);
impl Drop for S {
    fn drop(&mut self) {}
}

pub fn one_drop() {
    let _s = S(0);
}
"#;

    with_bridge("bridge_terminator_drop_prelude", source, |tcx, cx| {
        prewalk_for_synthesis(tcx, cx, "one_drop");
        regseq_driver::bridge::flush_pending_items_for_tests(cx).unwrap();
        regseq_driver::bridge::synth::rekey_synthesis_in_phase5(cx).unwrap();

        // Drive a single Drop terminator through the
        // translator using the basic_block entry point so the
        // statements list / mutable Body get the prelude and
        // synthetic locals respectively.
        let body = tcx.optimized_mir(find_fn_def_id(tcx, "one_drop").to_def_id());
        let mut model_body = Body::default();
        let drop_block = body
            .basic_blocks
            .iter()
            .find(|block| {
                matches!(
                    block.terminator.as_ref().map(|t| &t.kind),
                    Some(mir::TerminatorKind::Drop { .. })
                )
            })
            .expect("source should produce a Drop terminator");
        let locals_before = model_body.local_decls.len();
        let translated = {
            let mut body_cx = BodyCtxt::new_with_model_body(&body.local_decls, &mut model_body);
            regseq_driver::bridge::body::translate_basic_block(cx, &mut body_cx, drop_block)
                .expect("Drop translation should succeed")
        };

        // Synthetic-local allocations land in the mutable model
        // Body, not in `rustc_body.local_decls`.
        let locals_after = model_body.local_decls.len();
        assert_eq!(
            locals_after - locals_before,
            2,
            "Drop collapse must allocate _t_drop_arg + _t_drop_ret on the model Body"
        );

        // The first synthetic local is `*mut place_ty` mutable.
        let drop_arg_decl =
            &model_body.local_decls[regseq_model::LocalId::from_u32(locals_before as u32)];
        match cx.model.types.resolve(drop_arg_decl.ty) {
            Ty::Ptr(_, mutability) => {
                assert_eq!(*mutability, Mutability::Mut);
            }
            other => panic!("_t_drop_arg should be *mut _: got {other:?}"),
        }

        // The second synthetic local is the unit return slot.
        let drop_ret_decl =
            &model_body.local_decls[regseq_model::LocalId::from_u32((locals_before + 1) as u32)];
        assert!(matches!(
            cx.model.types.resolve(drop_ret_decl.ty),
            Ty::Tuple { fields, .. } if fields.is_empty()
        ));

        // The last statement in the translated basic block is
        // the structural prelude `_t_drop_arg = &raw mut place`.
        let last_stmt = translated
            .statements
            .last()
            .expect("Drop collapse must emit a RawPtr prelude statement");
        assert!(matches!(
            &last_stmt.kind,
            StatementKind::Assign(_, Rvalue::RawPtr(Mutability::Mut, _))
        ));

        // The terminator's destination is the synthetic unit
        // return slot, not RETURN_PLACE.
        let TerminatorKind::Call {
            callee,
            destination,
            args,
            ..
        } = &translated.terminator.kind
        else {
            panic!("Drop should collapse to TerminatorKind::Call");
        };
        assert_ne!(destination.local, regseq_model::LocalId::RETURN_PLACE);
        assert_eq!(args.len(), 1);

        // The call resolves to a real Function Instance with
        // DropGlue flavor.
        let CallCallee::Direct(instance_id) = callee else {
            panic!("Drop call must be Direct");
        };
        let instance = &cx.model.instances[*instance_id];
        let function = instance
            .kind
            .as_function()
            .expect("drop-glue InstanceId must resolve to InstanceKind::Function");
        assert_eq!(function.flavor, FunctionInstanceFlavor::DropGlue);
        assert!(function.virtual_idx.is_none());
    });
}

/// Audit finding regseq-driver-42-7 + spec section 15.1: the bridge
/// rejects atomic / fence / volatile intrinsic call sites at
/// translation time with `DslRejectionKind::Terminator`.
#[test]
fn rejects_volatile_and_atomic_call_sites_per_section_15_1() {
    let source = r#"
#![allow(internal_features)]
#![feature(core_intrinsics)]

pub unsafe fn calls_volatile_read(p: *const u32) -> u32 {
    core::ptr::read_volatile(p)
}

pub unsafe fn calls_atomic_intrinsic(p: *mut u32) {
    core::intrinsics::atomic_store::<u32, { core::intrinsics::AtomicOrdering::Relaxed }>(p, 7);
}
"#;

    with_bridge("bridge_terminator_section_15_1", source, |tcx, cx| {
        for fn_name in ["calls_volatile_read", "calls_atomic_intrinsic"] {
            let body = tcx.optimized_mir(find_fn_def_id(tcx, fn_name).to_def_id());
            let mut model_body = Body::default();
            let mut statements: Vec<Statement> = Vec::new();
            let mut saw_rejection = false;
            for term in body
                .basic_blocks
                .iter()
                .filter_map(|block| block.terminator.clone())
            {
                if !matches!(term.kind, mir::TerminatorKind::Call { .. }) {
                    continue;
                }
                let mut body_cx = BodyCtxt::new_with_model_body(&body.local_decls, &mut model_body);
                match translate_terminator(cx, &mut body_cx, &mut statements, term) {
                    Err(BridgeError::DslRejection {
                        kind: DslRejectionKind::Terminator,
                        ..
                    }) => saw_rejection = true,
                    other => {
                        panic!("expected DslRejection::Terminator for {fn_name}, got {other:?}")
                    }
                }
            }
            assert!(
                saw_rejection,
                "fn {fn_name} should trigger a section 15.1 call-site rejection"
            );
        }
    });
}
