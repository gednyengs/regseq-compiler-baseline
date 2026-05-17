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

use regseq_driver::bridge::synth::vtable::canonical_vacant_fn_ptr_ty;
use regseq_driver::bridge::synth::{
    rekey_synthesis_in_phase5, synthesise_closure, synthesise_drop_glue, synthesise_vtable,
};
use regseq_driver::bridge::{run_bridge, BridgeCtxt, BridgeInput};
use regseq_model::{
    ArgsKey, CAbiShape, Const, DefPathHash, FnAbi, InstanceKind, ItemId, PrincipalTraitKey,
    ProjectionBindingsKey, StableCrateId, SyntheticItemKind, Ty, UintKind,
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
            // Mirror the production `RegseqCallbacks::config` MIR pass
            // whitelist so source-derived FnDef Items interned by the
            // regseq-driver-46 Phase-2 pass have `RemoveStorageMarkers`
            // applied before Phase-5 body materialisation walks the
            // bodies; otherwise let-binding fixtures trip the DSL
            // rejection for raw StorageLive/StorageDead markers.
            config.opts.unstable_opts.mir_enable_passes = regseq_driver::regseq_mir_passes();
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
        // Bridge expects panic=abort cleanup-block pruning to remove
        // is_cleanup=true blocks before Phase-5 body materialisation.
        // The regseq-driver-46 Phase-2 pass interns source-derived
        // FnDef Items as `ItemKind::Function` and now their bodies
        // get materialised; without panic=abort, fixtures with Drop /
        // closures retain cleanup blocks that trip the body
        // translator's panic=abort assertion.
        "-C".to_string(),
        "panic=abort".to_string(),
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
    // The `c(0)` call keeps the closure local live across the
    // `SimplifyLocals-final` MIR pass in
    // `regseq_driver::regseq_mir_passes()`; without it the unused
    // closure local is elided and `first_closure_args` cannot
    // recover the closure args from the body's local_decls.
    let source = r#"
pub fn f() {
    let c = |x: u32| x + 1;
    let _ = c(0);
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
        // Synthetic test path produces vtable_type + vtable_instance
        // + drop_shim (no method shims; per audit findings the
        // hardcoded `vec![(3, None)]` is removed).
        assert_eq!(cx.pending_items.len(), 3);

        assign_pending(cx);
        rekey_synthesis_in_phase5(cx).unwrap();

        let vtable_item = cx.model.vtables.get(&principal).copied().unwrap();
        assert_eq!(vtable_item, cx.handles.resolve(first.vtable_type).unwrap());
        assert!(matches!(
            &cx.model.item_synthetic_kinds[vtable_item],
            SyntheticItemKind::VTableType { .. }
        ));
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
    assert!(model.items.iter().any(|item| {
        model
            .strings
            .resolve(item.def_path.rendered)
            .contains("drop_glue")
    }));
}

#[test]
fn vtable_method_shims_carry_distinct_def_paths_and_match_entry_indices() {
    // Two-method trait. `tcx.vtable_entries` produces a per-method
    // `VtblEntry::Method` slot after the leading drop/size/align
    // triple; per audit findings every method shim's
    // `vtable_entry_index` must match that slot index and every
    // shim's `method_def_path` must be the source method's def-path
    // (no shared `principal.trait_path` fallback).
    let source = r#"
pub trait T {
    fn first(&self);
    fn second(&self);
}
impl T for u32 {
    fn first(&self) {}
    fn second(&self) {}
}
pub fn f() {
    let x = 0u32;
    let t = &x as &dyn T;
    t.first();
    t.second();
}
"#;

    let model = compile_model("bridge_synth_multi_method", source);

    let mut method_shims: Vec<(u32, DefPathHash)> = model
        .item_synthetic_kinds
        .iter()
        .filter_map(|kind| match kind {
            SyntheticItemKind::VTableMethodShim {
                vtable_entry_index,
                method_def_path,
                ..
            } => Some((*vtable_entry_index, *method_def_path)),
            _ => None,
        })
        .collect();
    method_shims.sort_by_key(|entry| entry.0);

    assert_eq!(method_shims.len(), 2);
    assert!(
        method_shims[0].0 >= 3,
        "method slots follow the leading triple"
    );
    assert!(method_shims[1].0 > method_shims[0].0);
    assert_ne!(
        method_shims[0].1, method_shims[1].1,
        "every method shim has a distinct DefPathHash; principal.trait_path fallback is forbidden"
    );
}

#[test]
fn vtable_for_trivial_drop_impl_writes_null_pointer_in_drop_slot() {
    // `u32` has trivial Drop; per audit findings the
    // MetadataDropInPlace slot must hold a null pointer cell
    // (Const::NullPtr), not a drop-shim Reloc, and no
    // VTableDropShim Item should be synthesised.
    let source = r#"
pub trait T {
    fn m(&self);
}
impl T for u32 {
    fn m(&self) {}
}
pub fn f() {
    let x = 0u32;
    let t = &x as &dyn T;
    t.m();
}
"#;

    let model = compile_model("bridge_synth_trivial_drop", source);

    assert!(
        !model
            .item_synthetic_kinds
            .iter()
            .any(|kind| matches!(kind, SyntheticItemKind::VTableDropShim { .. })),
        "trivial Drop impl must not synthesise a VTableDropShim Item"
    );

    let init_const_id = model
        .instances
        .iter()
        .find_map(|instance| {
            if let InstanceKind::Static { init: Some(c), .. } = &instance.kind {
                let kind = &model.item_synthetic_kinds[instance.item];
                if matches!(kind, SyntheticItemKind::VTableInstance { .. }) {
                    return Some(*c);
                }
            }
            None
        })
        .expect("VTableInstance Static must exist");

    let init = model.consts.resolve(init_const_id);
    let Const::Aggregate(fields) = init else {
        panic!("VTableInstance init must be Const::Aggregate");
    };
    let drop_slot = model.consts.resolve(fields[0]);
    assert!(
        matches!(drop_slot, Const::NullPtr(_)),
        "MetadataDropInPlace slot must be a null pointer cell for trivial Drop, got {drop_slot:?}"
    );
}

#[test]
fn vtable_for_supertrait_holds_reloc_in_trait_vptr_slot() {
    // A trait with multiple supertraits forces rustc's
    // `prepare_vtable_segments` to emit at least one TraitVPtr entry:
    // sibling supertraits of an already-emitted segment cannot share
    // a vtable prefix, so they get explicit back-pointers per
    // `rustc_trait_selection/src/traits/vtable.rs`. The audit
    // requires that this slot resolves to a supertrait vtable
    // instance, not uninit/null.
    let source = r#"
pub trait SuperA {
    fn a(&self);
}
pub trait SuperB {
    fn b(&self);
}
pub trait Sub: SuperA + SuperB {
    fn d(&self);
}
impl SuperA for u32 {
    fn a(&self) {}
}
impl SuperB for u32 {
    fn b(&self) {}
}
impl Sub for u32 {
    fn d(&self) {}
}
pub fn f() {
    let x = 0u32;
    let t = &x as &dyn Sub;
    t.d();
}
"#;

    let model = compile_model("bridge_synth_supertrait", source);

    // Locate the Sub principal vs the Super principal by counting
    // VTableInstance Items: Sub's vtable contains a TraitVPtr slot
    // pointing at Super's vtable, so the model has TWO VTableInstance
    // Items per impl_ty.
    let vtable_instances: Vec<(regseq_model::ItemId, regseq_model::ConstId)> = model
        .instances
        .iter()
        .filter_map(|instance| {
            if let InstanceKind::Static { init: Some(c), .. } = &instance.kind {
                let kind = &model.item_synthetic_kinds[instance.item];
                if matches!(kind, SyntheticItemKind::VTableInstance { .. }) {
                    return Some((instance.item, *c));
                }
            }
            None
        })
        .collect();
    assert!(
        vtable_instances.len() >= 2,
        "should synthesise vtable instances for both Sub and at least one supertrait, got {}",
        vtable_instances.len()
    );

    // Compute the synthetic def-path-hash sets so we can identify
    // which init aggregate slot is the supertrait Reloc.
    let vtable_instance_def_paths: std::collections::HashSet<regseq_model::DefPathHash> =
        vtable_instances
            .iter()
            .map(|(item, _)| model.item_def_paths[*item])
            .collect();

    let mut found_supertrait_slot = false;
    for (_, init_const_id) in &vtable_instances {
        let Const::Aggregate(fields) = model.consts.resolve(*init_const_id) else {
            panic!("VTableInstance init must be Const::Aggregate");
        };
        for field in fields {
            if let Const::Reloc(instance_ref) = model.consts.resolve(*field) {
                if instance_ref.regseq_kind_rank == 1
                    && vtable_instance_def_paths.contains(&instance_ref.def_path_hash)
                {
                    found_supertrait_slot = true;
                }
            }
        }
    }
    assert!(
        found_supertrait_slot,
        "TraitVPtr slot must hold a Reloc to a supertrait VTableInstance Static"
    );
}

#[test]
fn vtable_items_and_instances_are_f4_and_f4_2_ordered_post_phase4() {
    // After Phase 4 + Phase 5 the model.items table must be sorted
    // by F4 (`<regseq_synth>` synthetic items follow source items in
    // the comparator) and model.instances must be F4-2 sorted: the
    // VTableInstance Statics (rank 1) appear after every Function
    // instance (rank 0). This task does not push Function instances
    // (regseq-driver-42 owns that), so the Static-only suffix is
    // trivially F4-2 conformant; the Items table check is the
    // load-bearing assertion here.
    let source = r#"
pub trait T {
    fn m(&self);
}
impl T for u32 {
    fn m(&self) {}
}
pub fn f() {
    let x = 0u32;
    let t = &x as &dyn T;
    t.m();
}
"#;

    let model = compile_model("bridge_synth_f4_ordering", source);

    // Per spec section 16.1, model.items must be sorted by the
    // canonical F4 comparator. Use the model's own
    // `compare_items_f4` to avoid re-deriving the byte-key order.
    for window in model.items.iter().collect::<Vec<_>>().windows(2) {
        let ordering = model.compare_items_f4(window[0], window[1]);
        assert!(
            !ordering.is_gt(),
            "model.items must be F4-sorted at Phase 4 entry per spec section 16.1; got {:?} between consecutive items",
            ordering
        );
    }

    // F4-2 instance ordering: walk `model.instances` and assert
    // ranks are non-decreasing.
    let mut last_rank: i32 = -1;
    for instance in model.instances.iter() {
        let rank = regseq_model::regseq_instance_kind_rank(&instance.kind) as i32;
        assert!(
            rank >= last_rank,
            "model.instances must be F4-2 sorted; saw rank {rank} after {last_rank}"
        );
        last_rank = rank;
    }
}

#[test]
fn final_sort_orders_function_before_static_instances() {
    // Audit-finding HIGH-40-2: `model.sort_instances_by_f4_2`
    // must order Function instances (rank 0) before Static
    // instances (rank 1) regardless of which Phase-5 sub-pass
    // pushed each first. Force a model with at least one of each
    // shape and confirm the post-sort order.
    use regseq_model::{
        regseq_instance_kind_rank, Const, ConstId, FunctionInstanceData, FunctionInstanceFlavor,
        Instance, InstanceKind, Mutability, Span, Ty, UNIT_LAYOUT,
    };

    let mut model = regseq_model::RegseqModel::with_primary("f4_2", regseq_model::StableCrateId(1));
    // Two minimal Items: one Function, one Static. Both share the
    // primary crate so they only diverge on kind for the F4-2
    // comparator's leading discriminator.
    let func_name = model.strings.intern("the_fn");
    let static_name = model.strings.intern("the_static");
    let render_fn = model.strings.intern("test::the_fn");
    let render_static = model.strings.intern("test::the_static");
    let item_fn = model.items.push(regseq_model::Item {
        def_path: regseq_model::DefPath {
            crate_id: model.primary_crate,
            segments: vec![regseq_model::DefPathSegment {
                data: regseq_model::DefPathData::ValueNs(func_name),
                disambiguator: 0,
            }],
            rendered: render_fn,
        },
        kind: regseq_model::ItemKind::Function,
        span: Span::dummy(),
        linkage: regseq_model::Linkage::Default,
        link_section: None,
        inline_hint: regseq_model::InlineHint::None,
        is_unsafe: false,
        is_phantom_data: false,
        is_unsafe_cell: false,
        is_manually_drop: false,
        opaque_handle_eligible: false,
    });
    let item_static = model.items.push(regseq_model::Item {
        def_path: regseq_model::DefPath {
            crate_id: model.primary_crate,
            segments: vec![regseq_model::DefPathSegment {
                data: regseq_model::DefPathData::ValueNs(static_name),
                disambiguator: 0,
            }],
            rendered: render_static,
        },
        kind: regseq_model::ItemKind::Static {
            mutability: Mutability::Not,
        },
        span: Span::dummy(),
        linkage: regseq_model::Linkage::Default,
        link_section: None,
        inline_hint: regseq_model::InlineHint::None,
        is_unsafe: false,
        is_phantom_data: false,
        is_unsafe_cell: false,
        is_manually_drop: false,
        opaque_handle_eligible: false,
    });
    model.item_def_paths.push(regseq_model::DefPathHash {
        stable_crate_id: regseq_model::StableCrateId(1),
        local_hash: 1,
    });
    model.item_def_paths.push(regseq_model::DefPathHash {
        stable_crate_id: regseq_model::StableCrateId(1),
        local_hash: 2,
    });
    model
        .item_synthetic_kinds
        .push(regseq_model::SyntheticItemKind::None);
    model
        .item_synthetic_kinds
        .push(regseq_model::SyntheticItemKind::None);
    model.domain_attrs.item_attrs.push(Vec::new());
    model.domain_attrs.item_attrs.push(Vec::new());

    // Push Static FIRST, Function SECOND. The pre-sort order is
    // [Static, Function]; F4-2 demands [Function, Static].
    let unit_ty = model.types.intern(Ty::Tuple {
        fields: Vec::new(),
        layout: UNIT_LAYOUT,
    });
    let zst_const = model.consts.intern(Const::ZST);
    model.instances.push(Instance {
        item: item_static,
        generic_args: Vec::new(),
        emission_name: render_static,
        span: Span::dummy(),
        kind: InstanceKind::Static {
            ty: unit_ty,
            init: Some(zst_const),
        },
    });
    let fn_sig = model.sigs.intern(regseq_model::FnSig {
        inputs: Vec::new(),
        output: unit_ty,
        abi: regseq_model::FnAbi::Rust,
        c_variadic: false,
        safety: regseq_model::Safety::Safe,
    });
    model.instances.push(Instance {
        item: item_fn,
        generic_args: Vec::new(),
        emission_name: render_fn,
        span: Span::dummy(),
        kind: InstanceKind::Function(FunctionInstanceData {
            fn_sig,
            body: None,
            flavor: FunctionInstanceFlavor::Item,
            virtual_idx: None,
        }),
    });

    let _ = ConstId::from_u32(0);
    assert_eq!(model.instances.len(), 2);
    assert_eq!(
        regseq_instance_kind_rank(&model.instances[regseq_model::InstanceId::from_u32(0)].kind),
        1,
        "pre-sort: Static is first"
    );
    assert_eq!(
        regseq_instance_kind_rank(&model.instances[regseq_model::InstanceId::from_u32(1)].kind),
        0,
        "pre-sort: Function is second"
    );

    model.sort_instances_by_f4_2();

    assert_eq!(
        regseq_instance_kind_rank(&model.instances[regseq_model::InstanceId::from_u32(0)].kind),
        0,
        "post-sort: Function must precede Static (F4-2 rank 0)"
    );
    assert_eq!(
        regseq_instance_kind_rank(&model.instances[regseq_model::InstanceId::from_u32(1)].kind),
        1,
        "post-sort: Static must follow Function (F4-2 rank 1)"
    );
}

#[test]
fn canonical_vacant_fn_ptr_ty_populates_c_abi_lowering_side_table() {
    // Per spec.md section 13.2 (lines 4633-4641) the canonical vacant
    // FnPtr signature is `extern "C" fn(*mut ()) -> ()`. Per spec.md
    // section 3.3 step 5 every `FnAbi::C` FnSig must carry a
    // `Some(CAbiLowering)` entry in the parallel
    // `sig_c_abi_lowerings` side table; this test guards the audit-
    // fix in regseq-driver-50 that wires
    // `bridge::fnsig::compute_cabi_lowering` into
    // `canonical_vacant_fn_ptr_ty` so the side table stays in sync
    // with `sigs`.
    let source = "";
    with_bridge(
        "bridge_synth_canonical_vacant_fn_ptr",
        source,
        |_tcx, cx| {
            let fn_ptr_ty = canonical_vacant_fn_ptr_ty(cx);
            let Ty::FnPtr(sig_id) = cx.model.types.resolve(fn_ptr_ty) else {
                panic!("canonical_vacant_fn_ptr_ty must return a Ty::FnPtr");
            };
            let sig_id = *sig_id;
            let sig = cx.model.sigs.resolve(sig_id).clone();
            assert_eq!(sig.abi, FnAbi::C, "canonical vacant FnSig must be FnAbi::C");
            assert!(
            sig_id.index() < cx.model.sig_c_abi_lowerings.len(),
            "sig_c_abi_lowerings side table must have a slot for every interned FnSig per spec.md section 3.3 step 5"
        );
            let lowering = cx.model.sig_c_abi_lowerings[sig_id]
            .clone()
            .expect("canonical vacant FnSig must carry Some(CAbiLowering) per spec.md section 3.3 step 5");
            assert_eq!(
                lowering.inputs.len(),
                1,
                "canonical vacant FnSig has exactly one input (`*mut ()`)"
            );
            assert!(
                matches!(lowering.inputs[0], CAbiShape::Direct),
                "the `*mut ()` input is leaf-admissible (Direct) per spec.md section 14.5"
            );
            assert!(
                matches!(lowering.output, CAbiShape::Direct),
                "the unit output is leaf-admissible (Direct) per spec.md section 14.5"
            );
            let output_ty = cx.model.types.resolve(sig.output);
            assert!(
                matches!(output_ty, Ty::Tuple { fields, .. } if fields.is_empty()),
                "canonical vacant FnSig output is unit"
            );
        },
    );
}
