//! regseq-driver-60 Test Plan B4: generic fn body materializes
//! against the Function Instance's concrete `generic_args`.
//!
//! Bridges a fixture exposing a generic `id<T: Copy>(x: T) -> T`
//! called as `id::<u32>(7)` and asserts that:
//!   - `run_bridge` returns `Ok(_)` (no `BridgeError::NonRigidTy`
//!     leaks out of the body materializer for the substituted body).
//!   - A Function Instance exists keyed on the `id` Item with
//!     `generic_args == [Type(u32)]`.
//!   - That Instance's `body == Some(_)`.
//!   - At least one `LocalDecl` in the materialized body resolves
//!     to the concrete `Ty::Uint(UintKind::U32)` row, proving the
//!     substitution carried by
//!     `Instance.instantiate_mir_and_normalize_erasing_regions` was
//!     applied before the local-decl walk in
//!     `crates/regseq-driver/src/bridge/body/materialise.rs`.
//!
//! The Regseq `Ty` enum has no `Param` variant; the negative form
//! of the regseq-driver-60 fix is "no `BridgeError::NonRigidTy {
//! kind: Param, .. }` raised during `run_bridge`" rather than "no
//! `Ty::Param` Ty in the model" (per
//! `crates/regseq-driver/src/bridge/ty.rs:275`).

#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, Once};
use std::time::{SystemTime, UNIX_EPOCH};

use regseq_driver::bridge::{run_bridge, BridgeInput};
use regseq_model::{
    FunctionInstanceFlavor, GenericArg, InstanceKind, ItemId, ItemKind, RegseqModel,
    SyntheticItemKind, Ty, UintKind,
};

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
    std::env::temp_dir().join(format!("regseq-driver-60-{crate_name}-{nanos}"))
}

fn compile_with_bridge(crate_name: &str, source: &str) -> RegseqModel {
    let _lock = COMPILER_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    init_rustc();

    let test_dir = fresh_test_dir(crate_name);
    std::fs::create_dir_all(&test_dir).unwrap();
    let src_path = test_dir.join("lib.rs");
    std::fs::write(&src_path, source).unwrap();

    let result: Arc<Mutex<Option<RegseqModel>>> = Arc::new(Mutex::new(None));

    struct Callbacks {
        result: Arc<Mutex<Option<RegseqModel>>>,
    }

    impl rustc_driver::Callbacks for Callbacks {
        fn config(&mut self, config: &mut rustc_interface::interface::Config) {
            config.opts.unstable_opts.no_codegen = true;
            config.opts.unstable_opts.mir_opt_level = Some(0);
            config.opts.unstable_opts.always_encode_mir = true;
            config.opts.unstable_opts.mir_enable_passes = regseq_driver::regseq_mir_passes();
            config.opts.cg.overflow_checks = Some(false);
            config.opts.cg.debug_assertions = Some(false);
            config.opts.debug_assertions = false;
        }

        fn after_analysis<'tcx>(
            &mut self,
            _compiler: &rustc_interface::interface::Compiler,
            tcx: rustc_middle::ty::TyCtxt<'tcx>,
        ) -> rustc_driver::Compilation {
            let input = BridgeInput::new(tcx);
            let model = run_bridge(input).expect("bridge should run");
            *self.result.lock().unwrap() = Some(model);
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
        "-C".to_string(),
        "panic=abort".to_string(),
    ];

    let mut callbacks = Callbacks {
        result: result.clone(),
    };
    rustc_driver::run_compiler(&args, &mut callbacks);
    let output = result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis should produce a bridge result");
    std::fs::remove_dir_all(Path::new(&test_dir)).ok();
    output
}

fn find_function_item(model: &RegseqModel, leaf: &str) -> ItemId {
    let want = format!("::{leaf}");
    for (idx, item) in model.items.iter().enumerate() {
        if !matches!(item.kind, ItemKind::Function) {
            continue;
        }
        let rendered = model.strings.resolve(item.def_path.rendered);
        if rendered.ends_with(&want)
            && matches!(
                model.item_synthetic_kinds[ItemId::from_u32(idx as u32)],
                SyntheticItemKind::None
            )
        {
            return ItemId::from_u32(idx as u32);
        }
    }
    panic!(
        "no source Function Item ending in `::{leaf}` found; items: {:?}",
        model
            .items
            .iter()
            .map(|it| model.strings.resolve(it.def_path.rendered))
            .collect::<Vec<_>>()
    );
}

#[test]
fn generic_fn_body_materializes_against_instance_args() {
    let model = compile_with_bridge(
        "generic_fn_body",
        r#"#![no_std]
pub fn id<T: Copy>(x: T) -> T { x }
pub fn entry() -> u32 { id::<u32>(7) }
"#,
    );

    let id_item = find_function_item(&model, "id");

    // The bridge surfaces the generic `id<T>` Item via the eager
    // HIR enumeration. Its Function Instance is created reactively
    // by the call site `id::<u32>(7)` in `entry` with concrete
    // args `[Type(u32)]`. Pre-regseq-driver-60 the body materializer
    // would have failed `translate_ty` on the polymorphic
    // `local_decls[i].ty == TyKind::Param(0)` row.
    let id_instances: Vec<&regseq_model::Instance> = model
        .instances
        .iter()
        .filter(|inst| inst.item == id_item && matches!(inst.kind, InstanceKind::Function(_)))
        .collect();
    assert_eq!(
        id_instances.len(),
        1,
        "expected exactly one Function Instance for generic fn `id`; got {}",
        id_instances.len(),
    );
    let id_instance = id_instances[0];

    // Generic args are `[Type(u32)]` -- the resolved monomorphization.
    assert_eq!(
        id_instance.generic_args.len(),
        1,
        "Function Instance for `id` must carry exactly one generic arg (concrete `u32`); got {:?}",
        id_instance.generic_args,
    );
    let GenericArg::Type(arg_ty_id) = id_instance.generic_args[0] else {
        panic!(
            "Function Instance for `id` must carry GenericArg::Type; got {:?}",
            id_instance.generic_args[0]
        );
    };
    assert!(
        matches!(model.types.resolve(arg_ty_id), Ty::Uint(UintKind::U32)),
        "Function Instance for `id` must be keyed on `u32`; got {:?}",
        model.types.resolve(arg_ty_id),
    );

    let function_data = id_instance
        .kind
        .as_function()
        .expect("`id` Instance must be a Function");
    let flavor = function_data.flavor;
    assert!(
        matches!(flavor, FunctionInstanceFlavor::Item),
        "Function Instance for `id` must have flavor Item; got {flavor:?}",
    );

    // Per regseq-driver-60: the body materializer applies
    // `Instance.generic_args` to the polymorphic MIR via
    // `instantiate_mir_and_normalize_erasing_regions`, so
    // `body == Some(_)` and its local_decls reference concrete Tys.
    let body = function_data
        .body
        .as_ref()
        .expect("Function Instance for `id` must have body == Some(_) post-materialization");
    assert!(
        !body.local_decls.is_empty(),
        "Function Instance body for `id` must have at least one LocalDecl",
    );

    // At least one LocalDecl must resolve to the concrete `u32`
    // row; presence of any `Ty::Uint(UintKind::U32)` LocalDecl
    // proves the substitution applied. The generic body's
    // local_decls would carry `TyKind::Param(0)` slots (return
    // place + arg) that resolve to `u32` after substitution.
    let has_concrete_u32 = body
        .local_decls
        .iter()
        .any(|local| matches!(model.types.resolve(local.ty), Ty::Uint(UintKind::U32)));
    assert!(
        has_concrete_u32,
        "Function Instance body for `id` must have at least one `Ty::Uint(UintKind::U32)` LocalDecl after substitution; got: {:?}",
        body.local_decls
            .iter()
            .map(|l| model.types.resolve(l.ty))
            .collect::<Vec<_>>(),
    );
}
