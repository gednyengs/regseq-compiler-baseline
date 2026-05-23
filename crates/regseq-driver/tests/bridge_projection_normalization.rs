//! Per regseq-driver-70 (A1 from the F68 post-F4 triage): the bridge's
//! type-translation entry normalizes two authorized associated-type
//! projection families to their concrete monomorphic types before the
//! `TyKind::Alias(..)` short-circuit at
//! `crates/regseq-driver/src/bridge/ty.rs` fires.
//!
//! Tests in this file cover Test Plan B2 of the task spec:
//!   1. Slice `<[T] as Pointee>::Metadata` normalizes to `usize`.
//!   2. Sized `<T as Pointee>::Metadata` normalizes to `()`.
//!   3. Fn-family `<fn(..) as FnOnce<Args>>::Output` normalizes to
//!      the function pointer's return type.
//!   4. An unauthorized projection (custom trait associated type)
//!      continues to short-circuit as
//!      `NonRigidTy { kind: NonRigidTyKind::Alias, .. }`.
//!   5. Dyn-trait `<dyn Tr as Pointee>::Metadata` continues to
//!      reject (regression sentinel: A1 must not normalize the dyn
//!      case).
//!
//! Tests 1, 2, and 5 are body-shaped: they compile a function that
//! exercises the projection and walk the body's `local_decls` to
//! drive `translate_ty`, mirroring the bridge's reactive path in
//! `bridge/body/materialise.rs`. The body-shape coverage is then
//! complemented by a direct `Ty::new_projection` construction on
//! the same projection family to pin the expected normalized
//! shape (`usize` / `()` / rejection) regardless of whether rustc's
//! own MIR pipeline elides the projection at this `mir_opt_level=0`
//! configuration. Test 3 (Fn-family Output over a function pointer
//! Self) is direct-Ty only: the body-shaped form would require
//! monomorphizing `caller<F: Fn()>` with `F = fn()`, which in turn
//! routes through the bridge's `Fn::call` FnDef translation -- a
//! separate downstream concern unrelated to A1's node-local
//! projection normalization. Test 4 (unauthorized projection)
//! extracts the projection from a polymorphic fn signature, which
//! is the natural shape for the unauthorized case.

#![feature(rustc_private)]

extern crate rustc_abi;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, Once};
use std::time::{SystemTime, UNIX_EPOCH};

use regseq_driver::bridge::ty::translate_ty;
use regseq_driver::bridge::{BridgeCtxt, BridgeError, BridgeInput, NonRigidTyKind};
use regseq_model::{Ty, UintKind};
use rustc_hir::def_id::{LocalDefId, LocalModDefId};
use rustc_middle::ty::{GenericArg, TyCtxt};

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
    std::env::temp_dir().join(format!("regseq-bridge-pnorm-{crate_name}-{nanos}"))
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
                "allow(unused, non_snake_case, nonstandard_style, improper_ctypes, private_interfaces, internal_features)"
                    .to_string(),
            );
            config
                .opts
                .unstable_opts
                .crate_attr
                .push("feature(rustc_attrs)".to_string());
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

fn find_trait_def_id(tcx: TyCtxt<'_>, name: &str) -> LocalDefId {
    for item_id in tcx
        .hir_module_items(LocalModDefId::CRATE_DEF_ID)
        .free_items()
    {
        let item = tcx.hir_item(item_id);
        if let rustc_hir::ItemKind::Trait(.., ident, _, _, _) = &item.kind {
            if ident.name.as_str() == name {
                return item.owner_id.def_id;
            }
        }
    }
    panic!("trait `{name}` not found");
}

/// Build a `<self_ty as core::ptr::Pointee>::Metadata` projection
/// `Ty<'tcx>` by interning a fresh `Alias(Projection, ..)` node with
/// the `metadata_type` lang-item DefId. Used to pin the expected
/// normalized shape of `Pointee::Metadata` regardless of whether the
/// compiled body-shaped snippet's MIR pipeline elides the projection
/// at `mir_opt_level=0` (the bridge's pipeline applies an additional
/// `instantiate_mir_and_normalize_erasing_regions` pass on materialised
/// instances per `bridge/body/materialise.rs:288-294`; the helper
/// under test runs at type-translation time on whatever Ty reaches
/// `translate_erased_ty`).
fn build_pointee_metadata_projection<'tcx>(
    tcx: TyCtxt<'tcx>,
    self_ty: rustc_middle::ty::Ty<'tcx>,
) -> rustc_middle::ty::Ty<'tcx> {
    let metadata_def_id =
        tcx.require_lang_item(rustc_hir::LangItem::Metadata, rustc_span::DUMMY_SP);
    rustc_middle::ty::Ty::new_projection(tcx, metadata_def_id, [GenericArg::from(self_ty)])
}

/// Build a `<self_ty as FnOnce<args_ty>>::Output` projection
/// `Ty<'tcx>` by interning a fresh `Alias(Projection, ..)` node with
/// the `fn_once_output` lang-item DefId. The two generic args correspond
/// to the `FnOnce<Args: Tuple>` trait's `Self` and `Args` parameters.
fn build_fn_once_output_projection<'tcx>(
    tcx: TyCtxt<'tcx>,
    self_ty: rustc_middle::ty::Ty<'tcx>,
    args_tuple_ty: rustc_middle::ty::Ty<'tcx>,
) -> rustc_middle::ty::Ty<'tcx> {
    let fn_once_output =
        tcx.require_lang_item(rustc_hir::LangItem::FnOnceOutput, rustc_span::DUMMY_SP);
    rustc_middle::ty::Ty::new_projection(
        tcx,
        fn_once_output,
        [GenericArg::from(self_ty), GenericArg::from(args_tuple_ty)],
    )
}

#[test]
fn slice_pointee_metadata_normalizes_to_usize() {
    // Body-shaped: compile a simple fn that uses
    // `core::ptr::metadata::<[u32]>(p)` and walk its `local_decls`
    // through `translate_ty`. Before A1, the projection in the
    // call's return-temp local rejected with NonRigidTy(Alias);
    // after A1 every local translates without error.
    let source = r#"
#![feature(ptr_metadata)]
pub fn meta_slice(p: *const [u32]) -> usize {
    core::ptr::metadata(p)
}
"#;

    with_bridge("bridge_pnorm_slice", source, |tcx, cx| {
        let body = tcx.optimized_mir(find_fn_def_id(tcx, "meta_slice").to_def_id());
        for (idx, local) in body.local_decls.iter().enumerate() {
            translate_ty(cx, local.ty).unwrap_or_else(|err| {
                panic!(
                    "meta_slice local {} (ty `{}`) failed to translate after A1: {:?}",
                    idx, local.ty, err
                )
            });
        }

        // Direct: pin the expected normalization target for the
        // `<[u32] as Pointee>::Metadata` projection (regardless of
        // MIR pipeline elision in the body above).
        let u32_ty = tcx.types.u32;
        let slice_u32 = rustc_middle::ty::Ty::new_slice(tcx, u32_ty);
        let projection = build_pointee_metadata_projection(tcx, slice_u32);
        let ty_id = translate_ty(cx, projection)
            .expect("slice Pointee::Metadata projection should normalize to usize via A1");
        let resolved = cx.model.types.resolve(ty_id);
        assert_eq!(
            resolved,
            &Ty::Uint(UintKind::Usize),
            "slice Pointee::Metadata should normalize to usize"
        );
    });
}

#[test]
fn sized_pointee_metadata_normalizes_to_unit_tuple() {
    // Body-shaped: compile a simple fn that uses
    // `core::ptr::metadata::<u32>(p)` and walk its `local_decls`
    // through `translate_ty`. After A1 every local translates
    // without error.
    let source = r#"
#![feature(ptr_metadata)]
pub fn meta_sized(p: *const u32) -> () {
    core::ptr::metadata(p)
}
"#;

    with_bridge("bridge_pnorm_sized", source, |tcx, cx| {
        let body = tcx.optimized_mir(find_fn_def_id(tcx, "meta_sized").to_def_id());
        for (idx, local) in body.local_decls.iter().enumerate() {
            translate_ty(cx, local.ty).unwrap_or_else(|err| {
                panic!(
                    "meta_sized local {} (ty `{}`) failed to translate after A1: {:?}",
                    idx, local.ty, err
                )
            });
        }

        // Direct: pin the expected normalization target for the
        // sized `<u32 as Pointee>::Metadata` projection.
        let u32_ty = tcx.types.u32;
        let projection = build_pointee_metadata_projection(tcx, u32_ty);
        let ty_id = translate_ty(cx, projection)
            .expect("sized Pointee::Metadata projection should normalize to () via A1");
        let resolved = cx.model.types.resolve(ty_id);
        match resolved {
            Ty::Tuple { fields, .. } if fields.is_empty() => {}
            other => {
                panic!("sized Pointee::Metadata should normalize to unit tuple, got {other:?}")
            }
        }
    });
}

#[test]
fn fn_ptr_fn_once_output_normalizes_to_return_ty() {
    // Direct-Ty only: the body-shaped form would require monomorphizing
    // `caller<F: Fn()>` with `F = fn()`, which in turn routes through
    // the bridge's `Fn::call` FnDef sig translation -- a downstream
    // concern unrelated to A1's node-local projection normalization.
    // Constructing the projection directly is the most precise way
    // to exercise the FnOnceOutput arm of the helper.
    let source = "pub fn _anchor() {}";

    with_bridge("bridge_pnorm_fnptr", source, |tcx, cx| {
        let unit_ty = tcx.types.unit;
        let fn_sig = rustc_middle::ty::Binder::dummy(rustc_middle::ty::FnSig {
            inputs_and_output: tcx.mk_type_list(&[unit_ty]),
            c_variadic: false,
            safety: rustc_hir::Safety::Safe,
            abi: rustc_abi::ExternAbi::Rust,
        });
        let fn_ptr_ty = rustc_middle::ty::Ty::new_fn_ptr(tcx, fn_sig);

        let projection = build_fn_once_output_projection(tcx, fn_ptr_ty, unit_ty);

        let ty_id = translate_ty(cx, projection).expect(
            "<fn() as FnOnce<()>>::Output projection should normalize to the fn-ptr return type via A1",
        );
        let resolved = cx.model.types.resolve(ty_id);
        match resolved {
            Ty::Tuple { fields, .. } if fields.is_empty() => {}
            other => panic!(
                "Fn-family Output projection over fn() should normalize to unit tuple, got {other:?}"
            ),
        }
    });
}

#[test]
fn unauthorized_projection_continues_to_short_circuit_as_alias() {
    // Sig-shaped: extract the polymorphic projection from a
    // declared fn signature. A1's helper must NOT normalize the
    // custom-trait associated-type projection; the bridge must
    // continue to short-circuit as
    // `NonRigidTy { kind: NonRigidTyKind::Alias, .. }`.
    let source = r#"
pub trait Tr {
    type Out;
}

pub fn f<T: Tr>() -> <T as Tr>::Out {
    loop {}
}
"#;

    with_bridge("bridge_pnorm_unauth", source, |tcx, cx| {
        let f_def_id = find_fn_def_id(tcx, "f").to_def_id();
        let sig = tcx.fn_sig(f_def_id).skip_binder().skip_binder();
        let output = sig.output();

        let result = translate_ty(cx, output);
        match result {
            Err(BridgeError::NonRigidTy { kind, .. }) => {
                assert!(
                    matches!(kind, NonRigidTyKind::Alias),
                    "unauthorized projection should reject with NonRigidTyKind::Alias, got {kind:?}"
                );
            }
            other => panic!(
                "expected NonRigidTy {{ kind: Alias }} on custom-trait projection, got {other:?}"
            ),
        }
    });
}

#[test]
fn dyn_trait_pointee_metadata_still_rejects_after_a1() {
    // Regression sentinel: A1 deliberately does NOT normalize
    // `<dyn Tr as Pointee>::Metadata`. The exact rejection variant
    // depends on rustc's machinery -- per F4 evidence at
    // `.harness/expectations/regseq-driver-67.md:243-244`, the
    // dyn case may reject with `Foreign` on
    // `core::ptr::metadata::VTable` (rustc partially normalizes the
    // dyn projection to a `DynMetadata` whose interior contains a
    // `NonNull<VTable>` pointer; the bridge's `TyKind::RawPtr`
    // Foreign check at `bridge/ty.rs:103-108` then fires). With the
    // raw projection (constructed directly here) the bridge falls
    // through the unauthorized-alias branch and rejects with
    // `NonRigidTy`. Both variants are acceptable.
    let source = r#"
pub trait Tr {}
"#;

    with_bridge("bridge_pnorm_dyn", source, |tcx, cx| {
        let tr_def_id = find_trait_def_id(tcx, "Tr").to_def_id();
        let principal =
            rustc_middle::ty::ExistentialTraitRef::new_from_args(tcx, tr_def_id, tcx.mk_args(&[]));
        let predicate = rustc_middle::ty::ExistentialPredicate::Trait(principal);
        let predicates =
            tcx.mk_poly_existential_predicates(&[rustc_middle::ty::Binder::dummy(predicate)]);
        let dyn_tr_ty = rustc_middle::ty::Ty::new_dynamic(tcx, predicates, tcx.lifetimes.re_erased);

        let projection = build_pointee_metadata_projection(tcx, dyn_tr_ty);

        let result = translate_ty(cx, projection);
        match result {
            Err(BridgeError::NonRigidTy { .. }) | Err(BridgeError::Foreign { .. }) => {}
            other => panic!(
                "dyn-Pointee::Metadata must still reject (NonRigidTy or Foreign), got {other:?}"
            ),
        }
    });
}
