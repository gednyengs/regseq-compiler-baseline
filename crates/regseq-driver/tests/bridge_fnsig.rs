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

use regseq_driver::bridge::fnsig::{compute_cabi_lowering, translate_fn_sig};
use regseq_driver::bridge::{BridgeCtxt, BridgeError, BridgeInput, CAbiTypePosition};
use regseq_model::{CAbiShape, FnAbi, SubstReason, Ty, UintKind};
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
    std::env::temp_dir().join(format!("regseq-bridge-fnsig-{crate_name}-{nanos}"))
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
                .push("allow(unused, improper_ctypes_definitions)".to_string());
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

fn direct_positions(lowering: &regseq_model::CAbiLowering) -> bool {
    lowering
        .inputs
        .iter()
        .all(|input| matches!(input, CAbiShape::Direct))
        && matches!(lowering.output, CAbiShape::Direct)
}

#[test]
fn translates_function_signatures_and_records_c_abi_lowering() {
    let source = r#"
#[repr(transparent)]
pub struct Handle(pub u32);

pub fn add(a: u32, b: u32) -> u32 { a + b }
pub extern "C" fn cfn(x: i32) -> i32 { x }
pub fn u() {}
pub fn r(p: &u32) -> u32 { *p }
pub extern "C" fn handle(h: Handle) -> Handle { h }
pub extern "C" fn handle_cb(_: extern "C" fn(Handle) -> Handle) {}
"#;

    with_bridge("bridge_fnsig_positive", source, |tcx, cx| {
        let add = translate_fn_sig(
            cx,
            tcx.fn_sig(find_fn_def_id(tcx, "add").to_def_id())
                .instantiate_identity(),
        )
        .unwrap();
        let add_sig = cx.model.sigs.resolve(add);
        assert_eq!(add_sig.inputs.len(), 2);
        assert_eq!(add_sig.abi, FnAbi::Rust);
        assert!(direct_positions(&compute_cabi_lowering(cx, add)));
        assert_eq!(cx.model.sig_c_abi_lowerings[add], None);

        let cfn = translate_fn_sig(
            cx,
            tcx.fn_sig(find_fn_def_id(tcx, "cfn").to_def_id())
                .instantiate_identity(),
        )
        .unwrap();
        let cfn_sig = cx.model.sigs.resolve(cfn);
        assert_eq!(cfn_sig.inputs.len(), 1);
        assert_eq!(cfn_sig.abi, FnAbi::C);
        assert!(direct_positions(
            cx.model.sig_c_abi_lowerings[cfn].as_ref().unwrap()
        ));

        let unit = translate_fn_sig(
            cx,
            tcx.fn_sig(find_fn_def_id(tcx, "u").to_def_id())
                .instantiate_identity(),
        )
        .unwrap();
        let unit_sig = cx.model.sigs.resolve(unit);
        assert!(unit_sig.inputs.is_empty());
        assert!(matches!(
            cx.model.types.resolve(unit_sig.output),
            Ty::Tuple { fields, .. } if fields.is_empty()
        ));

        let reference = translate_fn_sig(
            cx,
            tcx.fn_sig(find_fn_def_id(tcx, "r").to_def_id())
                .instantiate_identity(),
        )
        .unwrap();
        let reference_sig = cx.model.sigs.resolve(reference);
        assert!(matches!(
            cx.model.types.resolve(reference_sig.inputs[0]),
            Ty::Ref(_, _)
        ));
        assert!(direct_positions(&compute_cabi_lowering(cx, reference)));

        let handle = translate_fn_sig(
            cx,
            tcx.fn_sig(find_fn_def_id(tcx, "handle").to_def_id())
                .instantiate_identity(),
        )
        .unwrap();
        let handle_sig = cx.model.sigs.resolve(handle);
        let handle_input = handle_sig.inputs[0];
        let handle_output = handle_sig.output;
        let lowering = cx.model.sig_c_abi_lowerings[handle].as_ref().unwrap();
        let CAbiShape::Substituted {
            emit_as: input_emit_as,
            reason: SubstReason::TransparentChain,
        } = lowering.inputs[0]
        else {
            panic!("transparent input should be substituted");
        };
        assert_ne!(input_emit_as, handle_input);
        assert_eq!(
            cx.model.types.resolve(input_emit_as),
            &Ty::Uint(UintKind::U32)
        );
        let CAbiShape::Substituted {
            emit_as: output_emit_as,
            reason: SubstReason::TransparentChain,
        } = lowering.output
        else {
            panic!("transparent output should be substituted");
        };
        assert_ne!(output_emit_as, handle_output);
        assert_eq!(
            cx.model.types.resolve(output_emit_as),
            &Ty::Uint(UintKind::U32)
        );

        let handle_cb = translate_fn_sig(
            cx,
            tcx.fn_sig(find_fn_def_id(tcx, "handle_cb").to_def_id())
                .instantiate_identity(),
        )
        .unwrap();
        let handle_cb_sig = cx.model.sigs.resolve(handle_cb);
        let Ty::FnPtr(nested_sig) = cx.model.types.resolve(handle_cb_sig.inputs[0]) else {
            panic!("callback input should translate as a function pointer");
        };
        let nested_lowering = cx.model.sig_c_abi_lowerings[*nested_sig].as_ref().unwrap();
        assert!(matches!(
            nested_lowering.inputs[0],
            CAbiShape::Substituted {
                reason: SubstReason::TransparentChain,
                ..
            }
        ));
    });
}

#[test]
fn rejects_non_admissible_c_abi_positions() {
    let source = r#"
pub extern "C" fn bad_char(_: char) {}
"#;

    with_bridge("bridge_fnsig_bad_cabi", source, |tcx, cx| {
        let err = translate_fn_sig(
            cx,
            tcx.fn_sig(find_fn_def_id(tcx, "bad_char").to_def_id())
                .instantiate_identity(),
        )
        .unwrap_err();
        assert!(matches!(err, BridgeError::UnsupportedCAbiType { .. }));
    });
}

#[test]
fn rejects_never_inputs_for_all_abis() {
    let source = r#"
#![feature(never_type)]
pub fn never_input(_: !) {}
"#;

    with_bridge("bridge_fnsig_never_input", source, |tcx, cx| {
        let err = translate_fn_sig(
            cx,
            tcx.fn_sig(find_fn_def_id(tcx, "never_input").to_def_id())
                .instantiate_identity(),
        )
        .unwrap_err();
        assert!(matches!(err, BridgeError::UnsupportedNeverInput { .. }));
    });
}

#[test]
fn rejects_repr_c_aggregate_with_bad_field() {
    let source = r#"
#[repr(C)]
pub struct Bad {
    c: char,
}

pub extern "C" fn bad_aggregate(_: Bad) {}
"#;

    with_bridge("bridge_fnsig_bad_aggregate", source, |tcx, cx| {
        let err = translate_fn_sig(
            cx,
            tcx.fn_sig(find_fn_def_id(tcx, "bad_aggregate").to_def_id())
                .instantiate_identity(),
        )
        .unwrap_err();
        assert!(matches!(err, BridgeError::UnsupportedCAbiType { .. }));
    });
}

#[test]
fn rejects_substantive_hrtb_through_type_translation() {
    let source = r#"
pub fn hrtb(_: for<'a> fn(&'a u32, &'a u32)) {}
"#;

    with_bridge("bridge_fnsig_hrtb", source, |tcx, cx| {
        let err = translate_fn_sig(
            cx,
            tcx.fn_sig(find_fn_def_id(tcx, "hrtb").to_def_id())
                .instantiate_identity(),
        )
        .unwrap_err();
        assert!(matches!(err, BridgeError::SubstantiveHrtb { .. }));
    });
}

fn assert_unsupported_cabi(err: BridgeError, expected: CAbiTypePosition) {
    match err {
        BridgeError::UnsupportedCAbiType { position, .. } => {
            assert_eq!(position, expected, "wrong CAbi rejection position");
        }
        other => panic!("expected UnsupportedCAbiType, got {other:?}"),
    }
}

#[test]
fn rejects_unit_input_at_c_boundary() {
    let source = r#"
pub extern "C" fn unit_arg(_: ()) {}
"#;

    with_bridge("bridge_fnsig_unit_input", source, |tcx, cx| {
        let err = translate_fn_sig(
            cx,
            tcx.fn_sig(find_fn_def_id(tcx, "unit_arg").to_def_id())
                .instantiate_identity(),
        )
        .unwrap_err();
        assert_unsupported_cabi(err, CAbiTypePosition::Input(0));
    });
}

#[test]
fn rejects_array_input_at_c_boundary() {
    let source = r#"
pub extern "C" fn array_arg(_: [u32; 4]) {}
"#;

    with_bridge("bridge_fnsig_array_input", source, |tcx, cx| {
        let err = translate_fn_sig(
            cx,
            tcx.fn_sig(find_fn_def_id(tcx, "array_arg").to_def_id())
                .instantiate_identity(),
        )
        .unwrap_err();
        assert_unsupported_cabi(err, CAbiTypePosition::Input(0));
    });
}

#[test]
fn rejects_rust_abi_fnptr_field_at_c_boundary() {
    let source = r#"
#[repr(C)]
pub struct WithRustFn {
    pub f: fn(),
}

pub extern "C" fn with_rust_fn(_: WithRustFn) {}
"#;

    with_bridge("bridge_fnsig_field_rust_fnptr", source, |tcx, cx| {
        let err = translate_fn_sig(
            cx,
            tcx.fn_sig(find_fn_def_id(tcx, "with_rust_fn").to_def_id())
                .instantiate_identity(),
        )
        .unwrap_err();
        assert_unsupported_cabi(err, CAbiTypePosition::Field);
    });
}

#[test]
fn rejects_nested_never_output_at_field_position() {
    let source = r#"
#[repr(C)]
pub struct WithDiverging {
    pub f: extern "C" fn() -> !,
}

pub extern "C" fn with_diverging(_: WithDiverging) {}
"#;

    with_bridge("bridge_fnsig_field_never_output", source, |tcx, cx| {
        let err = translate_fn_sig(
            cx,
            tcx.fn_sig(find_fn_def_id(tcx, "with_diverging").to_def_id())
                .instantiate_identity(),
        )
        .unwrap_err();
        assert_unsupported_cabi(err, CAbiTypePosition::Field);
    });
}

#[test]
fn rejects_unsized_str_pointee_at_c_boundary() {
    let source = r#"
pub extern "C" fn raw_str(_: *const str) {}
"#;

    with_bridge("bridge_fnsig_raw_str", source, |tcx, cx| {
        let err = translate_fn_sig(
            cx,
            tcx.fn_sig(find_fn_def_id(tcx, "raw_str").to_def_id())
                .instantiate_identity(),
        )
        .unwrap_err();
        assert_unsupported_cabi(err, CAbiTypePosition::Input(0));
    });
}

#[test]
fn rejects_unsized_slice_pointee_at_c_boundary() {
    let source = r#"
pub extern "C" fn raw_slice(_: *const [u8]) {}
"#;

    with_bridge("bridge_fnsig_raw_slice", source, |tcx, cx| {
        let err = translate_fn_sig(
            cx,
            tcx.fn_sig(find_fn_def_id(tcx, "raw_slice").to_def_id())
                .instantiate_identity(),
        )
        .unwrap_err();
        assert_unsupported_cabi(err, CAbiTypePosition::Input(0));
    });
}

#[test]
fn rejects_unsized_dyn_pointee_at_c_boundary() {
    let source = r#"
pub trait Marker {}
pub extern "C" fn raw_dyn(_: *const dyn Marker) {}
"#;

    with_bridge("bridge_fnsig_raw_dyn", source, |tcx, cx| {
        let err = translate_fn_sig(
            cx,
            tcx.fn_sig(find_fn_def_id(tcx, "raw_dyn").to_def_id())
                .instantiate_identity(),
        )
        .unwrap_err();
        assert_unsupported_cabi(err, CAbiTypePosition::Input(0));
    });
}

#[test]
fn nested_fnptr_input_inherits_field_position() {
    let source = r#"
#[repr(C)]
pub struct WithBadCb {
    pub f: extern "C" fn(char),
}

pub extern "C" fn with_bad_cb(_: WithBadCb) {}
"#;

    with_bridge("bridge_fnsig_nested_inherits_field", source, |tcx, cx| {
        let err = translate_fn_sig(
            cx,
            tcx.fn_sig(find_fn_def_id(tcx, "with_bad_cb").to_def_id())
                .instantiate_identity(),
        )
        .unwrap_err();
        assert_unsupported_cabi(err, CAbiTypePosition::Field);
    });
}

#[test]
fn rejects_repr_c_zst_at_input_position() {
    let source = r#"
#[repr(C)]
pub struct Empty;

pub extern "C" fn empty_arg(_: Empty) {}
"#;

    with_bridge("bridge_fnsig_repr_c_zst", source, |tcx, cx| {
        let err = translate_fn_sig(
            cx,
            tcx.fn_sig(find_fn_def_id(tcx, "empty_arg").to_def_id())
                .instantiate_identity(),
        )
        .unwrap_err();
        assert_unsupported_cabi(err, CAbiTypePosition::Input(0));
    });
}
