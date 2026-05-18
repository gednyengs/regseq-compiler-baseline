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

use regseq_driver::bridge::ty::{erase_regions_or_reject, translate_ty};
use regseq_driver::bridge::{BridgeCtxt, BridgeError, BridgeInput, DslRejectionKind};
use regseq_model::{
    AutoTrait, FloatKind, IntKind, LayoutKind, Mutability, Provenance, SyntheticItemKind, Ty,
    UintKind,
};
use rustc_hir::def::DefKind;
use rustc_hir::def_id::{LocalDefId, LocalModDefId};
use rustc_middle::mir::RETURN_PLACE;
use rustc_middle::ty::{self, Ty as RustcTy, TyCtxt};

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
    std::env::temp_dir().join(format!("regseq-bridge-ty-{crate_name}-{nanos}"))
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

fn find_type_alias_def_id(tcx: TyCtxt<'_>, name: &str) -> LocalDefId {
    for item_id in tcx
        .hir_module_items(LocalModDefId::CRATE_DEF_ID)
        .free_items()
    {
        let item = tcx.hir_item(item_id);
        if let rustc_hir::ItemKind::TyAlias(ident, ..) = &item.kind {
            if ident.name.as_str() == name {
                return item.owner_id.def_id;
            }
        }
    }
    panic!("type alias `{name}` not found");
}

fn param_ty<'tcx>(tcx: TyCtxt<'tcx>, fn_name: &str, index: usize) -> rustc_middle::ty::Ty<'tcx> {
    let body = tcx.optimized_mir(find_fn_def_id(tcx, fn_name).to_def_id());
    let local = body.args_iter().nth(index).expect("parameter should exist");
    body.local_decls[local].ty
}

#[test]
fn translates_primitive_reference_pointer_array_slice_tuple_and_never_rows() {
    let source = r#"
pub fn f(
    a_bool: bool,
    a_char: char,
    a_i32: i32,
    a_u64: u64,
    a_f32: f32,
    a_str: &str,
    a_slice: &[u32],
    a_ref: &u32,
    a_ref_mut: &mut u32,
    a_ptr: *mut u32,
    a_array: [u32; 4],
    a_tuple: (u32, u8),
) -> ! {
    loop {}
}
"#;

    with_bridge("bridge_ty_positive", source, |tcx, cx| {
        let translated = (0..12)
            .map(|idx| {
                let id = translate_ty(cx, param_ty(tcx, "f", idx)).unwrap();
                cx.model.types.resolve(id).clone()
            })
            .collect::<Vec<_>>();

        assert_eq!(translated[0], Ty::Bool);
        assert_eq!(translated[1], Ty::Char);
        assert_eq!(translated[2], Ty::Int(IntKind::I32));
        assert_eq!(translated[3], Ty::Uint(UintKind::U64));
        assert_eq!(translated[4], Ty::Float(FloatKind::F32));
        assert!(matches!(
            translated[5],
            Ty::Ref(_, Provenance::SharedBorrow)
        ));
        let Ty::Ref(str_ty, _) = translated[5] else {
            panic!("&str should translate as Ref");
        };
        assert_eq!(cx.model.types.resolve(str_ty), &Ty::Str);
        assert!(matches!(
            translated[6],
            Ty::Ref(_, Provenance::SharedBorrow)
        ));
        let Ty::Ref(slice_ty, _) = translated[6] else {
            panic!("&[u32] should translate as Ref");
        };
        assert!(matches!(cx.model.types.resolve(slice_ty), Ty::Slice(_)));
        assert!(matches!(
            translated[7],
            Ty::Ref(_, Provenance::SharedBorrow)
        ));
        assert!(matches!(
            translated[8],
            Ty::Ref(_, Provenance::UniqueBorrow)
        ));
        assert!(matches!(translated[9], Ty::Ptr(_, Mutability::Mut)));
        assert!(matches!(translated[10], Ty::Array(_, 4)));
        let Ty::Tuple { fields, layout } = &translated[11] else {
            panic!("tuple row should translate as Ty::Tuple");
        };
        assert_eq!(fields.len(), 2);
        assert!(layout.size >= 4);
        let regseq_model::LayoutKind::Struct {
            fields: layout_fields,
        } = &layout.kind
        else {
            panic!("tuple layout should use struct field layout");
        };
        assert_eq!(layout_fields[0].offset, 0);

        let body = tcx.optimized_mir(find_fn_def_id(tcx, "f").to_def_id());
        let never = translate_ty(cx, body.local_decls[RETURN_PLACE].ty).unwrap();
        assert_eq!(cx.model.types.resolve(never), &Ty::Never);

        let rustc_middle::ty::TyKind::Ref(_, bare_str, _) = param_ty(tcx, "f", 5).kind() else {
            panic!("expected &str rustc type");
        };
        assert!(matches!(
            translate_ty(cx, *bare_str),
            Err(BridgeError::UnsupportedTy { .. })
        ));
        let rustc_middle::ty::TyKind::Ref(_, bare_slice, _) = param_ty(tcx, "f", 6).kind() else {
            panic!("expected slice reference rustc type");
        };
        assert!(matches!(
            translate_ty(cx, *bare_slice),
            Err(BridgeError::UnsupportedTy { .. })
        ));
    });
}

#[test]
fn translates_fn_def_fn_ptr_and_ordinary_dynamic_rows() {
    let source = r#"
pub trait Device {
    fn poll(&self) {}
}

pub fn target() {}
pub fn takes_fn_ptr(_: fn(u32) -> u32) {}
pub fn takes_dynamic(_: &(dyn Device + Send)) {}
pub fn closure_holder() {
    let _c = |x: u32| x + 1;
}
"#;

    with_bridge("bridge_ty_callable_dynamic", source, |tcx, cx| {
        let target = find_fn_def_id(tcx, "target").to_def_id();
        let fn_def_ty = tcx.type_of(target).instantiate_identity();
        let fn_def = translate_ty(cx, fn_def_ty).unwrap();
        assert!(matches!(cx.model.types.resolve(fn_def), Ty::FnDef(_, _)));

        let fn_ptr = translate_ty(cx, param_ty(tcx, "takes_fn_ptr", 0)).unwrap();
        let Ty::FnPtr(sig) = cx.model.types.resolve(fn_ptr) else {
            panic!("fn pointer row should translate as Ty::FnPtr");
        };
        let sig = cx.model.sigs.resolve(*sig);
        assert_eq!(sig.inputs.len(), 1);

        let dyn_ref = translate_ty(cx, param_ty(tcx, "takes_dynamic", 0)).unwrap();
        let Ty::Ref(dynamic, Provenance::SharedBorrow) = cx.model.types.resolve(dyn_ref) else {
            panic!("dynamic parameter should be behind a shared ref");
        };
        let Ty::Dynamic { auto_traits, .. } = cx.model.types.resolve(*dynamic) else {
            panic!("dyn row should translate as Ty::Dynamic");
        };
        assert_eq!(auto_traits, &vec![AutoTrait::Send]);

        let closure_def = tcx
            .hir_body_owners()
            .map(LocalDefId::to_def_id)
            .find(|def_id| matches!(tcx.def_kind(*def_id), DefKind::Closure))
            .expect("closure body should be present");
        let closure_ty = tcx.type_of(closure_def).instantiate_identity();
        let closure = translate_ty(cx, closure_ty).unwrap();
        assert!(matches!(cx.model.types.resolve(closure), Ty::Closure(_, _)));
    });
}

#[test]
fn translates_special_and_generic_adts_with_item_flags() {
    let source = r#"
pub struct S { x: u32 }

pub fn f(
    a_phantom: core::marker::PhantomData<u32>,
    a_cell: core::cell::UnsafeCell<u32>,
    a_drop: core::mem::ManuallyDrop<u32>,
    a_struct: S,
) {}
"#;

    with_bridge("bridge_ty_adts", source, |tcx, cx| {
        let ids = (0..4)
            .map(|idx| translate_ty(cx, param_ty(tcx, "f", idx)).unwrap())
            .collect::<Vec<_>>();
        // Source-derived nominal Items are interned with placeholder
        // ItemIds during Phase 2 (audit-finding F4: no direct
        // `model.items.push` from Phase 2); drive Phase 3 + Phase 4
        // + Phase 5 patching so the Ty entries resolve to real
        // ItemIds before reading them.
        regseq_driver::bridge::flush_pending_items_for_tests(cx).unwrap();
        regseq_driver::bridge::synth::rekey_synthesis_in_phase5(cx).unwrap();
        let items = ids
            .iter()
            .map(|id| match cx.model.types.resolve(*id) {
                Ty::Struct { item, .. } => *item,
                other => panic!("expected struct, got {other:?}"),
            })
            .collect::<Vec<_>>();

        assert!(cx.model.items[items[0]].is_phantom_data);
        assert!(cx.model.items[items[1]].is_unsafe_cell);
        assert!(cx.model.contains_unsafe_cell(ids[1]));
        assert!(cx.model.items[items[2]].is_manually_drop);
        assert!(!cx.model.items[items[3]].is_phantom_data);
        assert!(!cx.model.items[items[3]].is_unsafe_cell);
        assert!(!cx.model.items[items[3]].is_manually_drop);
        let Ty::Struct { layout, .. } = cx.model.types.resolve(ids[3]) else {
            panic!("generic ADT should translate as struct");
        };
        assert!(layout.size >= 4);
    });
}

#[test]
fn rejects_box_atomic_and_c_void_adts() {
    let source = r#"
pub fn boxed(_: Box<u32>) {}
pub fn atomic_bool(_: core::sync::atomic::AtomicBool) {}
pub fn atomic_ptr(_: core::sync::atomic::AtomicPtr<u32>) {}
pub fn atomic_u8(_: core::sync::atomic::AtomicU8) {}
pub fn atomic_u16(_: core::sync::atomic::AtomicU16) {}
pub fn atomic_u32(_: core::sync::atomic::AtomicU32) {}
pub fn atomic_u64(_: core::sync::atomic::AtomicU64) {}
pub fn atomic_usize(_: core::sync::atomic::AtomicUsize) {}
pub fn atomic_i8(_: core::sync::atomic::AtomicI8) {}
pub fn atomic_i16(_: core::sync::atomic::AtomicI16) {}
pub fn atomic_i32(_: core::sync::atomic::AtomicI32) {}
pub fn atomic_i64(_: core::sync::atomic::AtomicI64) {}
pub fn atomic_isize(_: core::sync::atomic::AtomicIsize) {}
pub fn const_void(_: *const core::ffi::c_void) {}
pub fn mut_void(_: *mut core::ffi::c_void) {}
pub fn register_callback(
    cb: extern "C" fn(*mut core::ffi::c_void),
    ctx: *mut core::ffi::c_void,
) {
    let _ = (cb, ctx);
}
"#;

    with_bridge("bridge_ty_reject_adts", source, |tcx, cx| {
        let cases = [
            ("boxed", DslRejectionKind::Box),
            ("atomic_bool", DslRejectionKind::Atomic),
            ("atomic_ptr", DslRejectionKind::Atomic),
            ("atomic_u8", DslRejectionKind::Atomic),
            ("atomic_u16", DslRejectionKind::Atomic),
            ("atomic_u32", DslRejectionKind::Atomic),
            ("atomic_u64", DslRejectionKind::Atomic),
            ("atomic_usize", DslRejectionKind::Atomic),
            ("atomic_i8", DslRejectionKind::Atomic),
            ("atomic_i16", DslRejectionKind::Atomic),
            ("atomic_i32", DslRejectionKind::Atomic),
            ("atomic_i64", DslRejectionKind::Atomic),
            ("atomic_isize", DslRejectionKind::Atomic),
            ("const_void", DslRejectionKind::CVoid),
            ("mut_void", DslRejectionKind::CVoid),
        ];
        for (name, expected) in cases {
            let err = match translate_ty(cx, param_ty(tcx, name, 0)) {
                Ok(id) => panic!("{name} unexpectedly translated to {id:?}"),
                Err(err) => err,
            };
            assert!(matches!(
                err,
                BridgeError::DslRejection { kind, .. } if kind == expected
            ));
        }

        let callback = translate_ty(cx, param_ty(tcx, "register_callback", 0)).unwrap_err();
        assert!(matches!(
            callback,
            BridgeError::DslRejection {
                kind: DslRejectionKind::CVoid,
                ..
            }
        ));
        let ctx = translate_ty(cx, param_ty(tcx, "register_callback", 1)).unwrap_err();
        assert!(matches!(
            ctx,
            BridgeError::DslRejection {
                kind: DslRejectionKind::CVoid,
                ..
            }
        ));
    });
}

#[test]
fn rejects_foreign_dynamic_and_hrtb_rows() {
    let source = r#"
#![feature(extern_types)]
#![feature(auto_traits)]
#![feature(coroutines)]
#![feature(coroutine_trait)]
#![feature(stmt_expr_attributes)]

unsafe extern "C" { type Opaque; }
type Callback = for<'a> fn(&'a u32, &'a u32);
type CleanCallback = for<'a> fn(&'a u32);
type RawCallback = for<'a> fn(*const &'a u32);
type TupleCallback = for<'a> fn((&'a u32,));
// Spec.md section 17.1 lines 6243-6246: `&'a &'b T` is TWO
// immediate-input-ref slots; both bound regions are used once each
// so the binder must be ACCEPTED (audit-finding MEDIUM-37-3
// Codex revision).
type ChainRefCallback = for<'a, 'b> fn(&'a &'b u32);
type Alias = u32;
pub auto trait MyAuto {}
pub trait Trait<T> {}

pub fn foreign(_: *const Opaque) {}
pub fn foreign_ref(_: &Opaque) {}
pub fn dyn_auto(_: &dyn Send) {}
pub fn custom_auto(_: &(dyn core::fmt::Debug + MyAuto)) {}
pub fn generic<T>(_: T) {}
pub fn hrtb(_: Callback) {}
pub fn clean_hrtb(_: CleanCallback) {}
pub fn raw_hrtb(_: RawCallback) {}
pub fn tuple_hrtb(_: TupleCallback) {}
pub fn chain_ref_hrtb(_: ChainRefCallback) {}
pub fn dynamic_hrtb(_: &dyn for<'a> Trait<&'a u32>) {}
pub fn coroutine_holder() {
    let _c = #[coroutine] || {
        yield 1u32;
    };
}
pub async fn async_fn() {}
pub fn coroutine_closure_holder() {
    let _ac = async || 1u32;
}
"#;

    with_bridge("bridge_ty_reject_misc", source, |tcx, cx| {
        let foreign = translate_ty(cx, param_ty(tcx, "foreign", 0)).unwrap_err();
        assert!(matches!(foreign, BridgeError::Foreign { .. }));
        let rustc_middle::ty::TyKind::Ref(_, foreign_pointee, _) =
            param_ty(tcx, "foreign_ref", 0).kind()
        else {
            panic!("expected foreign reference rustc type");
        };
        let foreign_ref = translate_ty(cx, *foreign_pointee).unwrap_err();
        assert!(matches!(foreign_ref, BridgeError::Foreign { .. }));

        let dyn_auto = translate_ty(cx, param_ty(tcx, "dyn_auto", 0)).unwrap_err();
        assert!(matches!(dyn_auto, BridgeError::DynAutoOnly { .. }));

        let custom_auto = translate_ty(cx, param_ty(tcx, "custom_auto", 0)).unwrap_err();
        assert!(matches!(custom_auto, BridgeError::CustomAutoTrait { .. }));

        let param = translate_ty(cx, param_ty(tcx, "generic", 0)).unwrap_err();
        assert!(matches!(
            param,
            BridgeError::NonRigidTy {
                kind: regseq_driver::bridge::NonRigidTyKind::Param,
                ..
            }
        ));

        let synthetic_non_rigid = [
            (
                RustcTy::new_bound(
                    tcx,
                    ty::INNERMOST,
                    ty::BoundTy {
                        var: ty::BoundVar::from_u32(0),
                        kind: ty::BoundTyKind::Anon,
                    },
                ),
                regseq_driver::bridge::NonRigidTyKind::Bound,
            ),
            (
                RustcTy::new_placeholder(
                    tcx,
                    ty::PlaceholderType::new_anon(
                        ty::UniverseIndex::ROOT.next_universe(),
                        ty::BoundVar::from_u32(0),
                    ),
                ),
                regseq_driver::bridge::NonRigidTyKind::Placeholder,
            ),
            (
                RustcTy::new_fresh(tcx, 0),
                regseq_driver::bridge::NonRigidTyKind::Infer,
            ),
            (
                RustcTy::new_alias(
                    tcx,
                    ty::Free,
                    ty::AliasTy::new_from_args(
                        tcx,
                        find_type_alias_def_id(tcx, "Alias").to_def_id(),
                        ty::GenericArgs::empty(),
                    ),
                ),
                regseq_driver::bridge::NonRigidTyKind::Alias,
            ),
        ];
        for (ty, expected) in synthetic_non_rigid {
            let err = translate_ty(cx, ty).unwrap_err();
            assert!(matches!(
                err,
                BridgeError::NonRigidTy { kind, .. } if kind == expected
            ));
        }
        let hrtb = translate_ty(cx, param_ty(tcx, "hrtb", 0)).unwrap_err();
        assert!(matches!(hrtb, BridgeError::SubstantiveHrtb { .. }));
        let clean = erase_regions_or_reject(tcx, param_ty(tcx, "clean_hrtb", 0));
        assert!(clean.is_ok());
        let relational = erase_regions_or_reject(tcx, param_ty(tcx, "hrtb", 0)).unwrap_err();
        assert!(matches!(relational, BridgeError::SubstantiveHrtb { .. }));

        let raw_hrtb = translate_ty(cx, param_ty(tcx, "raw_hrtb", 0)).unwrap_err();
        assert!(matches!(raw_hrtb, BridgeError::SubstantiveHrtb { .. }));

        let tuple_hrtb = translate_ty(cx, param_ty(tcx, "tuple_hrtb", 0)).unwrap_err();
        assert!(matches!(tuple_hrtb, BridgeError::SubstantiveHrtb { .. }));

        // Spec.md section 17.1 lines 6243-6246: `&'a &'b u32` is two
        // immediate-input-ref slots, each bound region used once;
        // must be ACCEPTED (audit-finding MEDIUM-37-3 Codex revision).
        let chain_ref = translate_ty(cx, param_ty(tcx, "chain_ref_hrtb", 0));
        assert!(
            chain_ref.is_ok(),
            "for<'a, 'b> fn(&'a &'b u32) must be accepted per spec section 17.1 lines 6243-6246; got {chain_ref:?}"
        );

        let dynamic_hrtb = translate_ty(cx, param_ty(tcx, "dynamic_hrtb", 0)).unwrap_err();
        assert!(matches!(dynamic_hrtb, BridgeError::SubstantiveHrtb { .. }));

        let coroutine_errors = tcx
            .hir_body_owners()
            .map(LocalDefId::to_def_id)
            .filter(|def_id| matches!(tcx.def_kind(*def_id), DefKind::Closure))
            .filter(|def_id| {
                let ty = tcx.type_of(*def_id).instantiate_identity();
                matches!(translate_ty(cx, ty), Err(BridgeError::Coroutine { .. }))
            })
            .count();
        assert!(coroutine_errors >= 3);

        let pat = tcx.mk_pat(ty::PatternKind::NotNull);
        let pat_ty = RustcTy::new_pat(tcx, tcx.types.usize, pat);
        assert!(matches!(
            translate_ty(cx, pat_ty),
            Err(BridgeError::Pat { .. })
        ));

        let unsafe_binder = RustcTy::new_unsafe_binder(tcx, ty::Binder::dummy(tcx.types.u32));
        assert!(matches!(
            translate_ty(cx, unsafe_binder),
            Err(BridgeError::UnsafeBinder { .. })
        ));

        let rustc_middle::ty::TyKind::Ref(_, bare_dynamic, _) = param_ty(tcx, "dyn_auto", 0).kind()
        else {
            panic!("expected dyn reference rustc type");
        };
        assert!(matches!(
            translate_ty(cx, *bare_dynamic),
            Err(BridgeError::UnsupportedTy { .. })
        ));
    });
}

#[test]
fn rejects_tykind_error_when_it_surfaces_in_post_normalisation_input() {
    // `TyKind::Error` is one of the section 15.1 non-rigid TyKinds.
    // Post-monomorphization it should not appear, but if the bridge
    // is handed an error type by a malformed input it must reject
    // explicitly rather than silently translating it. We synthesise
    // a TyKind::Error by emitting a real diagnostic to obtain an
    // `ErrorGuaranteed`, then constructing `Ty::new_error` with it.
    // We reset the diagnostic counter afterwards so the test
    // harness does not surface the synthetic error as a real
    // compile failure.
    let source = r#"pub fn f(_: u32) {}"#;
    with_bridge("bridge_ty_error_kind", source, |tcx, cx| {
        let dcx = tcx.dcx();
        let guar = dcx.err("synthetic TyKind::Error fixture for bridge rejection test");
        let error_ty = RustcTy::new_error(tcx, guar);
        let err = translate_ty(cx, error_ty).unwrap_err();
        assert!(matches!(
            err,
            BridgeError::NonRigidTy {
                kind: regseq_driver::bridge::NonRigidTyKind::Error,
                ..
            }
        ));
        // Reset the diagnostic state so the fixture does not surface
        // as a real rustc error to the test harness.
        dcx.reset_err_count();
    });
}

#[test]
fn closure_interns_both_ty_closure_and_parallel_ty_struct_view() {
    // Spec section 13.1: every closure interns a `Ty::Closure(item,
    // args)` plus a parallel `Ty::Struct { item, layout }` captures
    // view sharing the same `ItemId`. The mapping is recorded on the
    // model via `closure_captures_struct_ty`.
    let source = r#"
pub fn closure_holder() {
    let captured = 7u32;
    let other = 13u64;
    let _c = move || (captured, other);
}
"#;
    with_bridge("bridge_ty_closure_captures_view", source, |tcx, cx| {
        let closure_def = tcx
            .hir_body_owners()
            .map(LocalDefId::to_def_id)
            .find(|def_id| matches!(tcx.def_kind(*def_id), DefKind::Closure))
            .expect("closure body should be present");
        let closure_ty = tcx.type_of(closure_def).instantiate_identity();
        let closure_id = translate_ty(cx, closure_ty).unwrap();

        let Ty::Closure(closure_item, _) = cx.model.types.resolve(closure_id).clone() else {
            panic!("closure should translate as Ty::Closure");
        };
        let captures_struct_id = *cx
            .model
            .closure_captures_struct_ty
            .get(&closure_id)
            .expect("closure_captures_struct_ty mapping should exist");
        let Ty::Struct {
            item: struct_item,
            layout,
            ..
        } = cx.model.types.resolve(captures_struct_id).clone()
        else {
            panic!("captures view should be Ty::Struct");
        };
        assert_eq!(closure_item, struct_item, "two views share one ItemId");
        let LayoutKind::Struct { fields } = layout.kind else {
            panic!("captures struct layout should be Struct-kind");
        };
        assert_eq!(
            fields.len(),
            2,
            "captures struct should have one field per upvar"
        );
        // Spec section 13.1 lines 4527-4529: fields are positionally
        // named `_0`, `_1`, ... (audit-finding MEDIUM-40-3).
        for (idx, field) in fields.iter().enumerate() {
            let rendered = cx.model.strings.resolve(field.name);
            assert_eq!(
                rendered,
                format!("_{idx}"),
                "captures struct field {idx} must be named `_{idx}`"
            );
        }
        // The captured upvar field types match the closure body's
        // upvar tys.
        let rustc_upvars = if let rustc_middle::ty::TyKind::Closure(_, args) = closure_ty.kind() {
            args.as_closure().upvar_tys().iter().collect::<Vec<_>>()
        } else {
            panic!("closure_ty should be a closure");
        };
        for (field, expected) in fields.iter().zip(rustc_upvars.iter()) {
            let translated = translate_ty(cx, *expected).unwrap();
            assert_eq!(
                field.ty, translated,
                "captures struct field type should match closure upvar"
            );
        }
    });
}

#[test]
fn dynamic_vtable_ty_resolves_to_vtable_type_backed_struct() {
    // Spec section 13.2: `Ty::Dynamic.vtable_ty` must reference a
    // real `Ty::Struct { item, layout, .. }` whose `Item` carries
    // `SyntheticItemKind::VTableType { principal }`. The struct
    // layout carries one pointer-width slot per vtable entry.
    let source = r#"
pub trait Device {
    fn poll(&self) {}
}

pub fn dyn_holder(_: &dyn Device) {}
"#;
    with_bridge("bridge_ty_vtable_type_backed_struct", source, |tcx, cx| {
        let dyn_ref = translate_ty(cx, param_ty(tcx, "dyn_holder", 0)).unwrap();
        let Ty::Ref(dynamic_id, _) = cx.model.types.resolve(dyn_ref).clone() else {
            panic!("dyn parameter should sit behind a reference");
        };
        let Ty::Dynamic { vtable_ty, .. } = cx.model.types.resolve(dynamic_id).clone() else {
            panic!("dynamic should translate as Ty::Dynamic");
        };

        // Per spec section 3.4 D-12-5.3 plus the audit-finding
        // refinement on regseq-driver-40, the VTableType ItemId is
        // assigned at Phase 4 and the placeholder Ty::Struct is
        // patched at Phase 4 deferred resolution. Drive the bridge's
        // Phase-4 + rekey transitions before reading
        // `item_synthetic_kinds`.
        regseq_driver::bridge::flush_pending_items_for_tests(cx).unwrap();
        regseq_driver::bridge::synth::rekey_synthesis_in_phase5(cx).unwrap();

        let Ty::Struct {
            item: vtable_item,
            layout,
            ..
        } = cx.model.types.resolve(vtable_ty).clone()
        else {
            panic!("vtable_ty should be Ty::Struct");
        };
        match &cx.model.item_synthetic_kinds[vtable_item] {
            SyntheticItemKind::VTableType { .. } => {}
            other => panic!("vtable item should be SyntheticItemKind::VTableType, got {other:?}"),
        }
        let LayoutKind::Struct { fields } = layout.kind else {
            panic!("vtable struct layout should be Struct-kind");
        };
        // Common slots (drop, size, align) + poll method = 4 slots.
        assert!(
            fields.len() >= 4,
            "vtable should expose drop/size/align + per-method slots, got {} fields",
            fields.len()
        );
        // All slots are pointer-width.
        let pointer_size = cx.model.target.pointer_width as u64 / 8;
        assert_eq!(layout.size, fields.len() as u64 * pointer_size);
    });
}

#[test]
fn fn_ptr_with_rust_call_abi_is_rejected_at_fnsig_interner_path() {
    // Spec section 14.2: `RustCall` is rejected. The closure-call
    // shim rewrite (section 13.1) un-tuples the trailing args slot
    // and remaps the ABI to `FnAbi::Rust` BEFORE the
    // section-14.2 abi check; a RustCall FnPtr whose shape does
    // not match the closure-call shim (no tupled trailing slot)
    // falls through to the strict `translate_abi` rejection. The
    // FnSig interner path mirrors this: any RustCall FnSig that
    // was not pre-substituted must be rejected with
    // `BridgeError::UnsupportedAbi`. We exercise both paths.
    use regseq_driver::bridge::fnsig::translate_fn_sig;
    use regseq_driver::bridge::ty::translate_ty;
    let source = r#"pub fn dummy() {}"#;
    with_bridge("bridge_ty_rust_call_rejected", source, |tcx, cx| {
        let unit = tcx.types.unit;
        // FnSig interner path: RustCall rejected outright.
        let fn_sig = ty::Binder::dummy(tcx.mk_fn_sig(
            [unit],
            unit,
            false,
            rustc_hir::Safety::Safe,
            rustc_abi::ExternAbi::RustCall,
        ));
        let err = translate_fn_sig(cx, fn_sig).unwrap_err();
        assert!(matches!(err, BridgeError::UnsupportedAbi { .. }));

        // FnPtr translation path: a RustCall FnPtr whose last input
        // is NOT a tuple (i.e., outside the closure-call shim
        // shape) is rejected via `translate_abi`.
        let non_tuple_rust_call_sig = ty::Binder::dummy(tcx.mk_fn_sig(
            [tcx.types.u32],
            tcx.types.u32,
            false,
            rustc_hir::Safety::Safe,
            rustc_abi::ExternAbi::RustCall,
        ));
        let non_tuple_fn_ptr = RustcTy::new_fn_ptr(tcx, non_tuple_rust_call_sig);
        let err = translate_ty(cx, non_tuple_fn_ptr).unwrap_err();
        assert!(matches!(err, BridgeError::UnsupportedAbi { .. }));

        // FnPtr translation path: a RustCall FnPtr with a tupled
        // trailing slot (the closure-call shim shape) is normalised
        // to FnAbi::Rust per spec section 13.1 and translates
        // successfully; it is NOT rejected because the rewrite has
        // already substituted away the RustCall.
        let tuple_input = tcx.mk_ty_from_kind(rustc_middle::ty::TyKind::Tuple(
            tcx.mk_type_list(&[tcx.types.u32, tcx.types.bool]),
        ));
        let tuple_rust_call_sig = ty::Binder::dummy(tcx.mk_fn_sig(
            [tuple_input],
            tcx.types.u32,
            false,
            rustc_hir::Safety::Safe,
            rustc_abi::ExternAbi::RustCall,
        ));
        let tuple_fn_ptr = RustcTy::new_fn_ptr(tcx, tuple_rust_call_sig);
        let translated = translate_ty(cx, tuple_fn_ptr).unwrap();
        let Ty::FnPtr(sig_id) = cx.model.types.resolve(translated).clone() else {
            panic!("RustCall FnPtr with tupled trailing slot should remain Ty::FnPtr");
        };
        let sig = cx.model.sigs.resolve(sig_id);
        assert_eq!(sig.abi, regseq_model::FnAbi::Rust);
        assert_eq!(
            sig.inputs.len(),
            2,
            "trailing tuple slot should be un-tupled"
        );
    });
}

#[test]
fn nominal_struct_field_layout_preserves_source_field_name() {
    // Spec regseq-driver-53 Test Plan B2: bridging
    // `pub struct Counter { pub value: u32 }` must populate the
    // first `FieldLayout.name` with the source field name `"value"`,
    // not the empty interned string.
    let source = r#"
pub struct Counter { pub value: u32 }
pub fn f(_: Counter) {}
"#;
    with_bridge("bridge_ty_nominal_field_name", source, |tcx, cx| {
        let id = translate_ty(cx, param_ty(tcx, "f", 0)).unwrap();
        let Ty::Struct { layout, .. } = cx.model.types.resolve(id).clone() else {
            panic!("Counter should translate as Ty::Struct");
        };
        let LayoutKind::Struct { fields } = layout.kind else {
            panic!("Counter layout should be Struct-kind");
        };
        assert_eq!(fields.len(), 1, "Counter has one field");
        let rendered = cx.model.strings.resolve(fields[0].name);
        assert_eq!(
            rendered, "value",
            "Counter's only field must carry the source name `value`"
        );
    });
}

#[test]
fn tuple_struct_field_layout_uses_rustc_numeric_field_names() {
    // Spec regseq-driver-53 Test Plan B3: bridging
    // `pub struct Pair(pub u32, pub u32)` must populate
    // `FieldLayout.name` with rustc's source-level tuple-struct
    // field names `"0"` and `"1"` (numeric without underscore),
    // not the `_0`/`_1` form which is only for bare tuples.
    let source = r#"
pub struct Pair(pub u32, pub u32);
pub fn f(_: Pair) {}
"#;
    with_bridge("bridge_ty_tuple_struct_field_name", source, |tcx, cx| {
        let id = translate_ty(cx, param_ty(tcx, "f", 0)).unwrap();
        let Ty::Struct { layout, .. } = cx.model.types.resolve(id).clone() else {
            panic!("Pair should translate as Ty::Struct");
        };
        let LayoutKind::Struct { fields } = layout.kind else {
            panic!("Pair layout should be Struct-kind");
        };
        assert_eq!(fields.len(), 2, "Pair has two fields");
        assert_eq!(cx.model.strings.resolve(fields[0].name), "0");
        assert_eq!(cx.model.strings.resolve(fields[1].name), "1");
    });
}

#[test]
fn bare_tuple_field_layout_uses_underscore_prefixed_names() {
    // Spec regseq-driver-53 Test Plan B4: bridging a bare tuple
    // type `(u32, u32)` must populate `FieldLayout.name` with the
    // underscore-prefixed form `"_0"`, `"_1"`, ... per the
    // tuple-field validator contract at
    // `crates/regseq-model/src/layout_validator.rs:419-431`.
    let source = r#"
pub fn f(_: (u32, u32)) {}
"#;
    with_bridge("bridge_ty_bare_tuple_field_name", source, |tcx, cx| {
        let id = translate_ty(cx, param_ty(tcx, "f", 0)).unwrap();
        let Ty::Tuple { layout, .. } = cx.model.types.resolve(id).clone() else {
            panic!("(u32, u32) should translate as Ty::Tuple");
        };
        let LayoutKind::Struct { fields } = layout.kind else {
            panic!("bare tuple layout should be Struct-kind");
        };
        assert_eq!(fields.len(), 2, "(u32, u32) has two fields");
        assert_eq!(cx.model.strings.resolve(fields[0].name), "_0");
        assert_eq!(cx.model.strings.resolve(fields[1].name), "_1");
    });
}
