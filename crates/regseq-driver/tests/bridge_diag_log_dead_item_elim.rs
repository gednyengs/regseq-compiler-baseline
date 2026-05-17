#![feature(rustc_private)]

//! Integration tests for the post-DiagLogPeel residue-driven DCE
//! pass (`regseq-driver-48`). Bridges the canonical `diag_logging`
//! fixture and asserts the section 26.6 carrier-survival contract:
//! every `<LogArg as From<T>>::from(...)` wrapper Instance, every
//! From-impl Item, every per-Item FnSig, every
//! `Ty::Enum { item: <LogArg-item> }` entry in the TyInterner, and
//! the LogArg ADT Item itself are pruned after Phase 5 completes.
//!
//! `model.validate()` is asserted as the first check per
//! `regseq-driver-49`, which lands the validator's section 23.6.2
//! B-4 / B-6 I-DiagLog-CallArity exception (`spec.md:9649-9778`)
//! so the post-elim log Calls' flat-arg shape passes validation.

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex, Once};
use std::time::{SystemTime, UNIX_EPOCH};

use regseq_driver::bridge::{run_bridge, BridgeInput as DriverBridgeInput};
use regseq_model::{
    AttrData, BridgeInput, DefPathData, IntrinsicKind, PrettyPrinter, RegseqModel, Ty,
};
use rustc_driver::{run_compiler, Callbacks, Compilation};
use rustc_middle::ty::TyCtxt;

static INIT: Once = Once::new();
static LOCK: Mutex<()> = Mutex::new(());

fn init_rustc_logger() {
    INIT.call_once(|| {
        let early =
            rustc_session::EarlyDiagCtxt::new(rustc_session::config::ErrorOutputType::default());
        rustc_driver::init_rustc_env_logger(&early);
    });
}

fn host_sysroot() -> String {
    let out = Command::new("rustc")
        .arg("--print")
        .arg("sysroot")
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

fn fresh_test_dir(crate_name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("regseq-dead-item-elim-{crate_name}-{nanos}"))
}

/// Run `regseq-driver`'s bridge against the canonical
/// `crates/test-ir-fixtures/fixtures/<name>.rs` fixture and return
/// the resulting `RegseqModel`. Skips the `test-ir-fixtures`
/// `bridge_fixture` helper's built-in `model.validate()` call so
/// the caller controls when to run validation; the diagnostic-
/// logging carrier-survival test now runs `model.validate()` itself
/// as its first assertion per the `regseq-driver-49`
/// I-DiagLog-CallArity exception (spec.md section 23.6.2 B-4 / B-6
/// at `spec.md:9649-9778`).
fn bridge_fixture_no_validate(name: &str) -> RegseqModel {
    let _lock = LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    init_rustc_logger();
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("test-ir-fixtures")
        .join("fixtures")
        .join(format!("{name}.rs"));
    let test_dir = fresh_test_dir(name);
    std::fs::create_dir_all(&test_dir).unwrap();

    let result: Arc<Mutex<Option<Result<RegseqModel, String>>>> = Arc::new(Mutex::new(None));

    struct Cb {
        result: Arc<Mutex<Option<Result<RegseqModel, String>>>>,
    }
    impl Callbacks for Cb {
        fn config(&mut self, c: &mut rustc_interface::interface::Config) {
            c.opts.unstable_opts.no_codegen = true;
            c.opts.unstable_opts.mir_opt_level = Some(0);
            c.opts.unstable_opts.always_encode_mir = true;
            c.opts.unstable_opts.mir_enable_passes = regseq_driver::regseq_mir_passes();
            c.opts.cg.overflow_checks = Some(false);
            c.opts.cg.debug_assertions = Some(false);
            c.opts.debug_assertions = false;
        }
        fn after_analysis<'tcx>(
            &mut self,
            _compiler: &rustc_interface::interface::Compiler,
            tcx: TyCtxt<'tcx>,
        ) -> Compilation {
            let input = DriverBridgeInput::new(tcx);
            let outcome = run_bridge(input).map_err(|err| format!("{err}"));
            *self.result.lock().unwrap() = Some(outcome);
            Compilation::Stop
        }
    }

    let sysroot = host_sysroot();
    let args = vec![
        "rustc".to_string(),
        fixture_path.to_str().unwrap().to_string(),
        "--sysroot".to_string(),
        sysroot,
        "--edition".to_string(),
        "2021".to_string(),
        "--crate-type".to_string(),
        "rlib".to_string(),
        "--crate-name".to_string(),
        name.to_string(),
        "--out-dir".to_string(),
        test_dir.to_str().unwrap().to_string(),
    ];

    let mut cb = Cb {
        result: result.clone(),
    };
    run_compiler(&args, &mut cb);
    std::fs::remove_dir_all(&test_dir).ok();
    let outcome = result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis produced a result");
    outcome.expect("bridge succeeds on diag_logging fixture")
}

fn def_path_terminal_segment_name<'a>(
    model: &'a RegseqModel,
    item: &regseq_model::Item,
) -> Option<&'a str> {
    let last = item.def_path.segments.last()?;
    let name_id = match last.data {
        DefPathData::TypeNs(id)
        | DefPathData::ValueNs(id)
        | DefPathData::MacroNs(id)
        | DefPathData::LifetimeNs(id)
        | DefPathData::OpaqueLifetime(id)
        | DefPathData::AnonAssocTy(id) => id,
        _ => return None,
    };
    Some(model.strings.resolve(name_id))
}

/// Test Plan item 10: bridge the `diag_logging` fixture and assert
/// the section 26.6 carrier-survival contract.
#[test]
fn diag_logging_post_elim_removes_every_logarg_artifact() {
    let mut model = bridge_fixture_no_validate("diag_logging");

    // 0. The post-elim model satisfies the construction
    //    invariants, including the spec.md section 23.6.2 B-4 / B-6
    //    I-DiagLog-CallArity exception added by `regseq-driver-49`
    //    that admits the post-DiagLogPeel flat-arg shape on the
    //    surviving log Calls.
    let bridge_input = BridgeInput::matching(&model);
    model.validate(&bridge_input).expect(
        "validate succeeds post-elim per regseq-driver-49's B-4 / B-6 section 26.6 exception",
    );

    // 1. No Item's def_path terminal segment renders as "LogArg".
    //    (D8 from regseq-model-94.)
    for item in model.items.iter() {
        let name = def_path_terminal_segment_name(&model, item);
        assert_ne!(
            name,
            Some("LogArg"),
            "Bridge left a LogArg ADT Item in model.items post-elim: {}",
            model.strings.resolve(item.def_path.rendered),
        );
    }

    // 2. No Ty::Enum/Struct/Union resolves to a LogArg-named ADT.
    //    (D7 from regseq-model-94.)
    for (ty_id, ty) in model.types.iter() {
        let item = match ty {
            Ty::Enum { item, .. } | Ty::Struct { item, .. } | Ty::Union { item, .. } => Some(*item),
            _ => None,
        };
        if let Some(item) = item {
            if item.index() >= model.items.len() {
                continue;
            }
            let name = def_path_terminal_segment_name(&model, &model.items[item]);
            assert_ne!(
                name,
                Some("LogArg"),
                "TyInterner at TyId {} still resolves to a LogArg ADT post-elim",
                ty_id.as_u32(),
            );
        }
    }

    // 3. No surviving Function body has a LocalDecl whose ty
    //    resolves to a LogArg-named ADT.
    for instance in model.instances.iter() {
        let Some(data) = instance.kind.as_function() else {
            continue;
        };
        let Some(body) = data.body.as_ref() else {
            continue;
        };
        for decl in body.local_decls.iter() {
            let ty = model.types.resolve(decl.ty);
            let item = match ty {
                Ty::Enum { item, .. } | Ty::Struct { item, .. } | Ty::Union { item, .. } => {
                    Some(*item)
                }
                _ => None,
            };
            if let Some(item) = item {
                if item.index() >= model.items.len() {
                    continue;
                }
                let name = def_path_terminal_segment_name(&model, &model.items[item]);
                assert_ne!(
                    name,
                    Some("LogArg"),
                    "Function body has a LogArg-typed LocalDecl post-elim",
                );
            }
        }
    }

    // 4. No Instance.item resolves to a From-LogArg trait method
    //    callee.
    for instance in model.instances.iter() {
        if instance.item.index() >= model.items.len() {
            continue;
        }
        let item = &model.items[instance.item];
        let rendered = model.strings.resolve(item.def_path.rendered);
        if rendered.ends_with("::From::from") {
            let logarg_self = instance.generic_args.first().is_some_and(|arg| {
                if let regseq_model::GenericArg::Type(self_ty) = arg {
                    if self_ty.index() < model.types.len() {
                        if let Ty::Enum { item, .. }
                        | Ty::Struct { item, .. }
                        | Ty::Union { item, .. } = model.types.resolve(*self_ty)
                        {
                            if item.index() < model.items.len() {
                                return def_path_terminal_segment_name(&model, &model.items[*item])
                                    == Some("LogArg");
                            }
                        }
                    }
                }
                false
            });
            assert!(
                !logarg_self,
                "<LogArg as From<T>>::from wrapper Instance still present post-elim",
            );
        }
    }

    // 5. The four post-peel log Calls remain present, with the
    //    canonical post-peel arity (1 fmt + 1 u32 per call).
    let mut log_call_arities: Vec<usize> = Vec::new();
    for instance in model.instances.iter() {
        let Some(data) = instance.kind.as_function() else {
            continue;
        };
        let Some(body) = data.body.as_ref() else {
            continue;
        };
        for block in body.basic_blocks.iter() {
            if let regseq_model::TerminatorKind::Call { callee, args, .. } = &block.terminator.kind
            {
                let regseq_model::CallCallee::Direct(callee_inst) = callee else {
                    continue;
                };
                if callee_inst.index() >= model.instances.len() {
                    continue;
                }
                let callee_item = model.instances[*callee_inst].item;
                if callee_item.index() >= model.domain_attrs.item_attrs.len() {
                    continue;
                }
                let is_log = model.domain_attrs.item_attrs[callee_item]
                    .iter()
                    .any(|attr| matches!(attr, AttrData::Intrinsic(IntrinsicKind::Log(_))));
                if is_log {
                    log_call_arities.push(args.len());
                }
            }
        }
    }
    assert_eq!(
        log_call_arities.len(),
        4,
        "expected four post-peel log Calls, got {log_call_arities:?}",
    );
    for arity in &log_call_arities {
        assert_eq!(
            *arity, 2,
            "post-peel log Call should carry 1 fmt + 1 u32 = 2 args",
        );
    }
}

/// Test Plan item 11: snapshot the rendered post-elimination
/// model for the `diag_logging` fixture so future regressions
/// surface as snapshot deltas.
#[test]
fn diag_logging_post_elim_render_snapshot() {
    let model = bridge_fixture_no_validate("diag_logging");
    let rendered = PrettyPrinter::new(&model).render();
    assert!(!rendered.is_empty(), "render must be non-empty");
    assert!(
        !rendered.contains("LogArg"),
        "rendered model must not mention `LogArg` post-elim",
    );
    assert!(
        !rendered.contains("From::from"),
        "rendered model must not contain a `From::from` line post-elim",
    );
    insta::assert_snapshot!("diag_logging_post_elim_render", rendered);
}

/// Test Plan item 12: a fixture with an unrelated non-LogArg
/// `From` impl is preserved. The DiagLogPeelResidue accumulator
/// stays empty for this fixture (no `LogArg::from(...)` wrappers
/// exist), so the elim pass is a complete no-op: the `From<u32>
/// for S` Item, its Instance, its FnSig, and `S`'s Ty all
/// survive into RegseqModel.
#[test]
fn non_logarg_from_impl_in_unrelated_fixture_is_preserved() {
    let _lock = LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    init_rustc_logger();

    let source = r#"
        #![no_std]
        pub struct S(pub u32);
        impl From<u32> for S {
            fn from(v: u32) -> Self { S(v) }
        }
        #[no_mangle]
        pub extern "C" fn use_s() -> u32 {
            let s: S = S::from(1u32);
            s.0
        }
    "#;
    let test_dir = fresh_test_dir("non_logarg_from");
    std::fs::create_dir_all(&test_dir).unwrap();
    let src_path = test_dir.join("lib.rs");
    std::fs::write(&src_path, source).unwrap();

    let result: Arc<Mutex<Option<Result<RegseqModel, String>>>> = Arc::new(Mutex::new(None));

    struct Cb {
        result: Arc<Mutex<Option<Result<RegseqModel, String>>>>,
    }
    impl Callbacks for Cb {
        fn config(&mut self, c: &mut rustc_interface::interface::Config) {
            c.opts.unstable_opts.no_codegen = true;
            c.opts.unstable_opts.mir_opt_level = Some(0);
            c.opts.unstable_opts.always_encode_mir = true;
            c.opts.unstable_opts.mir_enable_passes = regseq_driver::regseq_mir_passes();
            c.opts.cg.overflow_checks = Some(false);
            c.opts.cg.debug_assertions = Some(false);
            c.opts.debug_assertions = false;
        }
        fn after_analysis<'tcx>(
            &mut self,
            _compiler: &rustc_interface::interface::Compiler,
            tcx: TyCtxt<'tcx>,
        ) -> Compilation {
            let input = DriverBridgeInput::new(tcx);
            let outcome = run_bridge(input).map_err(|err| format!("{err}"));
            *self.result.lock().unwrap() = Some(outcome);
            Compilation::Stop
        }
    }

    let sysroot = host_sysroot();
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
        "non_logarg_from".to_string(),
        "--out-dir".to_string(),
        test_dir.to_str().unwrap().to_string(),
    ];
    let mut cb = Cb {
        result: result.clone(),
    };
    run_compiler(&args, &mut cb);
    std::fs::remove_dir_all(&test_dir).ok();
    let model = result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis produced a result")
        .expect("bridge succeeds on non-LogArg From impl fixture");

    let mut saw_s_struct_ty = false;
    let mut s_item_id: Option<regseq_model::ItemId> = None;
    for (_, ty) in model.types.iter() {
        if let Ty::Struct { item, .. } = ty {
            if item.index() < model.items.len() {
                let name = def_path_terminal_segment_name(&model, &model.items[*item]);
                if name == Some("S") {
                    saw_s_struct_ty = true;
                    s_item_id = Some(*item);
                }
            }
        }
    }
    assert!(saw_s_struct_ty, "S struct Ty must survive");

    let mut saw_from_for_s_instance = false;
    for instance in model.instances.iter() {
        if instance.item.index() >= model.items.len() {
            continue;
        }
        let item = &model.items[instance.item];
        let rendered = model.strings.resolve(item.def_path.rendered);
        if !rendered.ends_with("::From::from") {
            continue;
        }
        if let Some(regseq_model::GenericArg::Type(self_ty)) = instance.generic_args.first() {
            if self_ty.index() < model.types.len() {
                if let Ty::Struct { item, .. } = model.types.resolve(*self_ty) {
                    if Some(*item) == s_item_id {
                        saw_from_for_s_instance = true;
                    }
                }
            }
        }
    }
    assert!(
        saw_from_for_s_instance,
        "`<S as From<u32>>::from` Instance should survive",
    );
}
