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

use regseq_driver::bridge::body::{translate_terminator, BodyCtxt};
use regseq_driver::bridge::{run_bridge, BridgeCtxt, BridgeInput};
use regseq_model::{AttrData, DefPathData, IntrinsicKind, ItemId, LogLevel, RegseqModel};
use rustc_hir::def_id::{LocalDefId, LocalModDefId};
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
    std::env::temp_dir().join(format!("regseq-bridge-diag-log-{crate_name}-{nanos}"))
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
            // body materialisation (newly active after audit finding
            // regseq-driver-42-4 introduced `construct_function_instances`)
            // does not trip on StorageLive/StorageDead rejections at
            // the statement level.
            config.opts.unstable_opts.mir_opt_level = Some(1);
            config
                .opts
                .unstable_opts
                .crate_attr
                .push("allow(unused, non_snake_case)".to_string());
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

fn translate_calls<'tcx>(tcx: TyCtxt<'tcx>, cx: &mut BridgeCtxt<'tcx>, caller: &str) {
    let body = tcx.optimized_mir(find_fn_def_id(tcx, caller).to_def_id());
    // Pre-walk every call terminator so the callee FnDef types are
    // interned (and their Item handles fresh) before invoking
    // `flush_pending_items_for_tests`. The flush binds final ItemIds
    // so the lookup_or_create_direct_instance path in
    // translate_call_callee resolves callee Items through real
    // ItemIds rather than placeholder ones (audit finding 42-4).
    for block in body.basic_blocks.iter() {
        let Some(term) = &block.terminator else {
            continue;
        };
        if let rustc_middle::mir::TerminatorKind::Call { func, .. } = &term.kind {
            let func_ty = func.ty(&body.local_decls, tcx);
            let _ = regseq_driver::bridge::ty::translate_ty(cx, func_ty);
        }
    }
    regseq_driver::bridge::flush_pending_items_for_tests(cx).unwrap();
    regseq_driver::bridge::synth::rekey_synthesis_in_phase5(cx).unwrap();
    let mut body_cx = BodyCtxt::new(&body.local_decls);
    let mut statements: Vec<regseq_model::Statement> = Vec::new();
    for terminator in body
        .basic_blocks
        .iter()
        .filter_map(|block| block.terminator.clone())
    {
        let _ = translate_terminator(cx, &mut body_cx, &mut statements, terminator).unwrap();
    }
}

fn source_with_calls(calls: &str) -> String {
    source_with_extra_items("", calls)
}

fn source_with_extra_items(extra_items: &str, calls: &str) -> String {
    format!(
        r#"
#![feature(register_tool)]
#![register_tool(rseq)]

pub mod regseq {{
    pub mod log {{
        pub mod intrinsics {{
            pub enum LogArg<'a> {{
                Str(&'a str),
            }}

            #[inline(never)]
            #[rseq::intrinsic("log-debug")]
            pub fn log_debug(_fmt: &'static str, _args: &[LogArg<'_>]) {{ unimplemented!() }}

            #[inline(never)]
            #[rseq::intrinsic("log-info")]
            pub fn log_info(_fmt: &'static str, _args: &[LogArg<'_>]) {{ unimplemented!() }}

            #[inline(never)]
            #[rseq::intrinsic("log-warn")]
            pub fn log_warn(_fmt: &'static str, _args: &[LogArg<'_>]) {{ unimplemented!() }}

            #[inline(never)]
            #[rseq::intrinsic("log-error")]
            pub fn log_error(_fmt: &'static str, _args: &[LogArg<'_>]) {{ unimplemented!() }}
        }}

        macro_rules! debug {{
            ($fmt:literal $(, $arg:expr)* $(,)?) => {{
                $crate::regseq::log::intrinsics::log_debug($fmt, &[])
            }};
        }}
        pub(crate) use debug;

        macro_rules! info {{
            ($fmt:literal $(, $arg:expr)* $(,)?) => {{
                $crate::regseq::log::intrinsics::log_info($fmt, &[])
            }};
        }}
        pub(crate) use info;

        macro_rules! log_warn {{
            ($fmt:literal $(, $arg:expr)* $(,)?) => {{
                $crate::regseq::log::intrinsics::log_warn($fmt, &[])
            }};
        }}
        pub(crate) use log_warn as warn;

        macro_rules! error {{
            ($fmt:literal $(, $arg:expr)* $(,)?) => {{
                $crate::regseq::log::intrinsics::log_error($fmt, &[])
            }};
        }}
        pub(crate) use error;
    }}
}}

pub fn regular_function() {{}}

{extra_items}

#[no_mangle]
pub extern "C" fn caller() {{
    {calls}
}}
"#
    )
}

fn item_log_levels(model: &RegseqModel, leaf: &str) -> Vec<LogLevel> {
    let Some((idx, _item)) = model.items.iter().enumerate().find(|(_, item)| {
        let Some(segment) = item.def_path.segments.last() else {
            return false;
        };
        let DefPathData::ValueNs(name) = segment.data else {
            return false;
        };
        model.strings.resolve(name) == leaf
    }) else {
        return Vec::new();
    };
    model.domain_attrs.item_attrs[ItemId::from_u32(idx as u32)]
        .iter()
        .filter_map(|attr| match attr {
            AttrData::Intrinsic(IntrinsicKind::Log(level)) => Some(level.clone()),
            // A non-log intrinsic (mmio / poll) is not a log level; skip it.
            AttrData::Intrinsic(_) => None,
            // The framework-type marker is not a log level; skip it.
            AttrData::FrameworkType(_) => None,
            // Interrupt-construct markers (#6) are not log levels; skip them.
            AttrData::InterruptResource { .. } => None,
            AttrData::IsrBinding { .. } => None,
            // Atomicity field markers (#7) are not log levels; skip them.
            AttrData::Atomic { .. } => None,
        })
        .collect()
}

fn item_log_levels_after_run_bridge(calls: &str) -> Vec<(String, Vec<LogLevel>)> {
    with_bridge(
        "bridge_diag_log_phase2",
        &source_with_calls(calls),
        |tcx, _cx| {
            let model = run_bridge(BridgeInput::new(tcx)).expect("bridge should run");
            [
                "log_debug",
                "log_info",
                "log_warn",
                "log_error",
                "regular_function",
            ]
            .into_iter()
            .map(|leaf| (leaf.to_string(), item_log_levels(&model, leaf)))
            .collect::<Vec<_>>()
        },
    )
}

fn levels_for<'a>(levels: &'a [(String, Vec<LogLevel>)], leaf: &str) -> &'a [LogLevel] {
    levels
        .iter()
        .find_map(|(name, levels)| (name == leaf).then_some(levels.as_slice()))
        .unwrap_or(&[])
}

#[test]
fn recognises_debug_log_macro_call() {
    let levels = item_log_levels_after_run_bridge(r#"regseq::log::debug!("debug");"#);
    assert_eq!(levels_for(&levels, "log_debug"), &[LogLevel::Debug]);
    assert!(levels_for(&levels, "log_info").is_empty());
    assert!(levels_for(&levels, "log_warn").is_empty());
    assert!(levels_for(&levels, "log_error").is_empty());
}

#[test]
fn recognises_info_log_macro_call() {
    let levels = item_log_levels_after_run_bridge(r#"regseq::log::info!("info");"#);
    assert_eq!(levels_for(&levels, "log_info"), &[LogLevel::Info]);
    assert!(levels_for(&levels, "log_debug").is_empty());
    assert!(levels_for(&levels, "log_warn").is_empty());
    assert!(levels_for(&levels, "log_error").is_empty());
}

#[test]
fn recognises_warn_log_macro_call() {
    let levels = item_log_levels_after_run_bridge(r#"regseq::log::warn!("warn");"#);
    assert_eq!(levels_for(&levels, "log_warn"), &[LogLevel::Warn]);
    assert!(levels_for(&levels, "log_debug").is_empty());
    assert!(levels_for(&levels, "log_info").is_empty());
    assert!(levels_for(&levels, "log_error").is_empty());
}

#[test]
fn recognises_error_log_macro_call() {
    let levels = item_log_levels_after_run_bridge(r#"regseq::log::error!("error");"#);
    assert_eq!(levels_for(&levels, "log_error"), &[LogLevel::Error]);
    assert!(levels_for(&levels, "log_debug").is_empty());
    assert!(levels_for(&levels, "log_info").is_empty());
    assert!(levels_for(&levels, "log_warn").is_empty());
}

#[test]
fn ignores_non_log_call() {
    let levels = item_log_levels_after_run_bridge("regular_function();");
    assert!(levels_for(&levels, "regular_function").is_empty());
    assert!(levels_for(&levels, "log_debug").is_empty());
    assert!(levels_for(&levels, "log_info").is_empty());
    assert!(levels_for(&levels, "log_warn").is_empty());
    assert!(levels_for(&levels, "log_error").is_empty());
}

#[test]
fn ignores_unannotated_function_named_like_log_intrinsic() {
    let source = source_with_extra_items("pub fn log_debug() {}", "log_debug();");
    let levels = with_bridge("bridge_diag_log_unannotated_name", &source, |tcx, cx| {
        translate_calls(tcx, cx, "caller");
        vec![(
            "log_debug".to_string(),
            item_log_levels(&cx.model, "log_debug"),
        )]
    });

    assert!(levels_for(&levels, "log_debug").is_empty());
}

#[test]
fn run_bridge_phase2_recognises_log_intrinsic() {
    let levels = item_log_levels_after_run_bridge(r#"regseq::log::warn!("warn");"#);

    assert_eq!(levels_for(&levels, "log_warn"), &[LogLevel::Warn]);
}

#[test]
fn recognises_multiple_severities_on_distinct_callees() {
    let levels = item_log_levels_after_run_bridge(
        r#"
        regseq::log::debug!("debug");
        regseq::log::info!("info");
        regseq::log::warn!("warn");
        regseq::log::error!("error");
        "#,
    );

    assert_eq!(levels_for(&levels, "log_debug"), &[LogLevel::Debug]);
    assert_eq!(levels_for(&levels, "log_info"), &[LogLevel::Info]);
    assert_eq!(levels_for(&levels, "log_warn"), &[LogLevel::Warn]);
    assert_eq!(levels_for(&levels, "log_error"), &[LogLevel::Error]);
}
