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

use regseq_driver::bridge::synth::{
    rekey_synthesis_in_phase5, synthesise_closure, synthesise_drop_glue, synthesise_vtable,
};
use regseq_driver::bridge::{run_bridge, BridgeCtxt, BridgeInput};
use regseq_model::{
    ArgsKey, DefPathHash, ItemId, PrincipalTraitKey, ProjectionBindingsKey, StableCrateId,
    SyntheticItemKind, Ty, UintKind,
};
use rustc_hir::def_id::LocalModDefId;
use rustc_middle::ty::{self, TyCtxt, TyKind};

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
    std::env::temp_dir().join(format!("regseq-bridge-synth-{crate_name}-{nanos}"))
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
                .push("allow(unused)".to_string());
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

fn compile_model(crate_name: &str, source: &str) -> regseq_model::RegseqModel {
    with_bridge(crate_name, source, |_tcx, cx| {
        let input = BridgeInput::new(cx.input.tcx);
        run_bridge(input).expect("bridge should run")
    })
}

fn find_fn_def_id(tcx: TyCtxt<'_>, name: &str) -> rustc_hir::def_id::LocalDefId {
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

fn first_closure_args<'tcx>(tcx: TyCtxt<'tcx>, fn_name: &str) -> ty::ClosureArgs<TyCtxt<'tcx>> {
    let body = tcx.optimized_mir(find_fn_def_id(tcx, fn_name));
    for local in &body.local_decls {
        if let TyKind::Closure(_, args) = local.ty.kind() {
            return args.as_closure();
        }
    }
    panic!("closure local not found in `{fn_name}`");
}

fn principal(local_hash: u64) -> PrincipalTraitKey {
    PrincipalTraitKey {
        trait_path: DefPathHash {
            stable_crate_id: StableCrateId(0x711A17),
            local_hash,
        },
        trait_args: ArgsKey::EMPTY,
        projection_bindings: ProjectionBindingsKey(0),
    }
}

fn assign_pending(cx: &mut BridgeCtxt<'_>) {
    for pending in cx.pending_items.drain(..) {
        let item_id = cx.model.items.push(pending.item);
        cx.model.item_def_paths.push(pending.def_path_hash);
        cx.model.item_synthetic_kinds.push(pending.synthetic_kind);
        cx.model.domain_attrs.item_attrs.push(Vec::new());
        cx.handles.bind_final_item_id(pending.handle, item_id);
    }
}

fn synthetic_kinds(cx: &BridgeCtxt<'_>) -> Vec<SyntheticItemKind> {
    cx.pending_items
        .iter()
        .map(|pending| pending.synthetic_kind.clone())
        .collect()
}

#[test]
fn closure_synthesis_records_three_family_handles() {
    let source = r#"
pub fn f() {
    let _c = |x: u32| x + 1;
}
"#;

    with_bridge("bridge_synth_closure", source, |tcx, cx| {
        let handles = synthesise_closure(cx, first_closure_args(tcx, "f")).unwrap();

        assert_eq!(cx.closure_synth_cache, vec![handles.clone()]);
        assert_eq!(cx.pending_items.len(), 4);
        assert!(matches!(
            synthetic_kinds(cx).as_slice(),
            [
                SyntheticItemKind::ClosureCaptures { .. },
                SyntheticItemKind::ClosureCall { .. },
                SyntheticItemKind::ClosureCall { .. },
                SyntheticItemKind::ClosureCall { .. }
            ]
        ));
        assert!(handles.drop_glue.is_none());

        assign_pending(cx);
        rekey_synthesis_in_phase5(cx).unwrap();
    });
}

#[test]
fn closure_synthesis_records_drop_glue_when_captures_need_drop() {
    let source = r#"
pub struct S(u32);
impl Drop for S {
    fn drop(&mut self) {}
}
pub fn f() {
    let s = S(0);
    let _c = move || drop(s);
}
"#;

    with_bridge("bridge_synth_closure_drop", source, |tcx, cx| {
        let handles = synthesise_closure(cx, first_closure_args(tcx, "f")).unwrap();

        assert!(matches!(
            synthetic_kinds(cx).as_slice(),
            [
                SyntheticItemKind::ClosureCaptures { .. },
                SyntheticItemKind::ClosureCall { .. },
                SyntheticItemKind::ClosureDropGlue { .. }
            ]
        ));
        assert!(handles.drop_glue.is_some());

        assign_pending(cx);
        rekey_synthesis_in_phase5(cx).unwrap();
    });
}

#[test]
fn vtable_synthesis_dedups_on_principal_and_rekeys_mapping() {
    let source = "";

    with_bridge("bridge_synth_vtable", source, |_tcx, cx| {
        let impl_ty = cx.model.types.intern(Ty::Uint(UintKind::U32));
        let principal = principal(0x7A17);
        let first = synthesise_vtable(cx, principal.clone(), impl_ty).unwrap();
        let second = synthesise_vtable(cx, principal.clone(), impl_ty).unwrap();

        assert_eq!(first, second);
        assert_eq!(cx.vtable_synth_cache.len(), 1);
        // The public `synthesise_vtable` entry has no rustc handle to
        // consult `tcx.vtable_entries`, so it produces only the three
        // family items: VTableType, VTableInstance, VTableDropShim.
        // Method shims are added only by the rustc-aware call site in
        // `bridge::scan_statement_for_synthesis`.
        assert_eq!(cx.pending_items.len(), 3);
        assert!(first.method_shims.is_empty());

        assign_pending(cx);
        rekey_synthesis_in_phase5(cx).unwrap();

        let vtable_item = cx.model.vtables.get(&principal).copied().unwrap();
        assert_eq!(vtable_item, cx.handles.resolve(first.vtable_type).unwrap());
        assert!(matches!(
            &cx.model.item_synthetic_kinds[vtable_item],
            SyntheticItemKind::VTableType { .. }
        ));

        // Phase 5 must materialise the VTableInstance Static Instance
        // with an `init: Some(_)` value per spec.md:1063-1074. The
        // public entry uses a minimal 3-slot common-header init since
        // no `tcx.vtable_entries` data is available.
        let vtable_instance = cx.handles.resolve(first.vtable_instance).unwrap();
        let instance = cx
            .model
            .instances
            .iter()
            .find(|inst| inst.item == vtable_instance)
            .expect("VTableInstance Static should be synthesised");
        match &instance.kind {
            regseq_model::InstanceKind::Static { init, .. } => {
                assert!(init.is_some(), "Static must have init");
            }
            other => panic!("VTableInstance should be Static, got {other:?}"),
        }
    });
}

#[test]
fn vtable_type_dedups_on_principal_across_impl_tys() {
    let source = "";

    with_bridge("bridge_synth_vtable_principal_dedup", source, |_tcx, cx| {
        let u32_ty = cx.model.types.intern(Ty::Uint(UintKind::U32));
        let u64_ty = cx.model.types.intern(Ty::Uint(UintKind::U64));
        let principal = principal(0x7A18);

        synthesise_vtable(cx, principal.clone(), u32_ty).unwrap();
        synthesise_vtable(cx, principal.clone(), u64_ty).unwrap();

        assert_eq!(cx.vtable_type_synth_cache.len(), 1);
        assert_eq!(cx.vtable_synth_cache.len(), 2);
        assert_eq!(
            synthetic_kinds(cx)
                .iter()
                .filter(|kind| matches!(kind, SyntheticItemKind::VTableType { .. }))
                .count(),
            1
        );

        assign_pending(cx);
        rekey_synthesis_in_phase5(cx).unwrap();
        assert!(cx.model.vtables.contains_key(&principal));
    });
}

#[test]
fn drop_glue_synthesis_dedups_on_ty_id() {
    let source = "";

    with_bridge("bridge_synth_drop_glue", source, |_tcx, cx| {
        let ty = cx.model.types.intern(Ty::Uint(UintKind::U32));
        let first = synthesise_drop_glue(cx, ty).unwrap();
        let second = synthesise_drop_glue(cx, ty).unwrap();

        assert_eq!(first, second);
        assert_eq!(cx.drop_glue_synth_cache.len(), 1);
        assert_eq!(cx.pending_items.len(), 1);

        assign_pending(cx);
        rekey_synthesis_in_phase5(cx).unwrap();
        let item = cx.handles.resolve(first).unwrap();
        assert_eq!(item, ItemId::from_u32(0));
    });
}

#[test]
fn phase5_rekey_is_deterministic_across_repeated_runs() {
    let source = "";
    let snapshots = (0..5)
        .map(|_| {
            with_bridge("bridge_synth_rekey_det", source, |_tcx, cx| {
                let impl_ty = cx.model.types.intern(Ty::Uint(UintKind::U32));
                let principal = principal(0xD371);
                let vtable = synthesise_vtable(cx, principal.clone(), impl_ty).unwrap();
                let drop_glue = synthesise_drop_glue(cx, impl_ty).unwrap();
                assign_pending(cx);
                rekey_synthesis_in_phase5(cx).unwrap();
                (
                    cx.model.vtables.get(&principal).copied().unwrap().as_u32(),
                    cx.handles.resolve(vtable.vtable_instance).unwrap().as_u32(),
                    cx.handles.resolve(drop_glue).unwrap().as_u32(),
                    cx.model.item_synthetic_kinds.len(),
                )
            })
        })
        .collect::<Vec<_>>();

    assert!(snapshots.iter().all(|snapshot| *snapshot == snapshots[0]));
}

#[test]
fn run_bridge_synthesises_closure_vtable_and_drop_glue_fixtures() {
    let source = r#"
pub trait T {
    fn m(&self);
}

impl T for u32 {
    fn m(&self) {}
}

pub struct S(u32);
impl Drop for S {
    fn drop(&mut self) {}
}

pub fn closure_fixture() -> impl Fn(u32) -> u32 {
    |x| x + 1
}

pub fn vtable_fixture() {
    let x = 0u32;
    let t = &x as &dyn T;
    t.m();
}

pub fn drop_fixture() {
    let _ = S(0);
}
"#;

    let model = compile_model("bridge_synth_fixtures", source);

    assert!(model
        .item_synthetic_kinds
        .iter()
        .any(|kind| matches!(kind, SyntheticItemKind::ClosureCaptures { .. })));
    assert!(model
        .item_synthetic_kinds
        .iter()
        .any(|kind| matches!(kind, SyntheticItemKind::ClosureCall { .. })));
    assert!(model
        .item_synthetic_kinds
        .iter()
        .any(|kind| matches!(kind, SyntheticItemKind::VTableType { .. })));
    assert!(model
        .item_synthetic_kinds
        .iter()
        .any(|kind| matches!(kind, SyntheticItemKind::VTableMethodShim { .. })));
    assert!(!model.vtables.is_empty());

    // The trait `T` has exactly one dispatchable method `m`, so the
    // vtable_entries sequence is `[DropInPlace, Size, Align, Method]`
    // and the synthesised method-shim's `vtable_entry_index` must be
    // 3 (not slot 0 or any other hardcoded value). This guards
    // against regressions to the prior hardcoded `vec![(3, None)]`
    // that happened to coincidentally pick the right slot for this
    // single-method trait shape.
    let method_shim_slot = model
        .item_synthetic_kinds
        .iter()
        .find_map(|kind| match kind {
            SyntheticItemKind::VTableMethodShim {
                vtable_entry_index,
                ..
            } => Some(*vtable_entry_index),
            _ => None,
        })
        .expect("vtable_fixture should synthesise a VTableMethodShim");
    assert_eq!(method_shim_slot, 3);

    // The VTableInstance Static Instance must carry a structural
    // init value per spec.md:1063-1074 and the slot count must
    // match the four `vtable_entries` slots above.
    let vtable_instance_item = model
        .item_synthetic_kinds
        .iter()
        .enumerate()
        .find_map(|(idx, kind)| match kind {
            SyntheticItemKind::VTableInstance { .. } => {
                Some(regseq_model::ItemId::from_u32(idx as u32))
            }
            _ => None,
        })
        .expect("VTableInstance Item should be synthesised");
    let init_id = model
        .instances
        .iter()
        .find(|inst| inst.item == vtable_instance_item)
        .and_then(|inst| match &inst.kind {
            regseq_model::InstanceKind::Static { init, .. } => *init,
            _ => None,
        })
        .expect("VTableInstance Static must have Some(init)");
    match model.consts.resolve(init_id) {
        regseq_model::Const::Bytes(bytes) => {
            let pointer_size = bytes.len() / 4;
            assert!(pointer_size > 0);
            assert_eq!(bytes.len(), pointer_size * 4);
        }
        other => panic!("VTableInstance init should be Const::Bytes, got {other:?}"),
    }
    assert!(model.items.iter().any(|item| {
        model
            .strings
            .resolve(item.def_path.rendered)
            .contains("drop_glue")
    }));
}
