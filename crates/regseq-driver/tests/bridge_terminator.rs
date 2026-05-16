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
use regseq_model::{CallCallee, SyntheticItemKind, Terminator, TerminatorKind};
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
            config.opts.unstable_opts.mir_opt_level = Some(0);
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
    let mut body_cx = BodyCtxt::new(&body.local_decls);
    body.basic_blocks
        .iter()
        .filter_map(|block| block.terminator.clone())
        .map(|term| translate_terminator(cx, &mut body_cx, term).unwrap())
        .collect()
}

fn rustc_drop_translations<'tcx>(
    tcx: TyCtxt<'tcx>,
    cx: &mut BridgeCtxt<'tcx>,
    name: &str,
) -> Vec<Terminator> {
    let body = tcx.optimized_mir(find_fn_def_id(tcx, name).to_def_id());
    let mut body_cx = BodyCtxt::new(&body.local_decls);
    body.basic_blocks
        .iter()
        .filter_map(|block| block.terminator.clone())
        .filter(|term| matches!(term.kind, mir::TerminatorKind::Drop { .. }))
        .map(|term| translate_terminator(cx, &mut body_cx, term).unwrap())
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
        let mut body_cx = BodyCtxt::new(&body.local_decls);
        let unreachable = mir::Terminator {
            source_info,
            kind: mir::TerminatorKind::Unreachable,
        };
        let translated = translate_terminator(cx, &mut body_cx, unreachable).unwrap();
        assert!(matches!(translated.kind, TerminatorKind::Unreachable));

        let false_edge = mir::Terminator {
            source_info,
            kind: mir::TerminatorKind::FalseEdge {
                real_target: mir::BasicBlock::from_u32(0),
                imaginary_target: mir::BasicBlock::from_u32(0),
            },
        };
        let err = translate_terminator(cx, &mut body_cx, false_edge)
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
        let one = rustc_drop_translations(tcx, cx, "one_drop");
        assert_eq!(one.len(), 1);
        let TerminatorKind::Call {
            callee: CallCallee::Direct(first_callee),
            args,
            target: Some(_),
            ..
        } = &one[0].kind
        else {
            panic!("drop should collapse to a direct call with a normal target");
        };
        assert_eq!(args.len(), 1);
        assert!(cx
            .pending_items
            .iter()
            .any(|item| matches!(item.synthetic_kind, SyntheticItemKind::DropGlue { .. })));

        let two = rustc_drop_translations(tcx, cx, "two_drops");
        assert!(two.len() >= 2);
        let callees = two
            .iter()
            .map(|term| match &term.kind {
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
