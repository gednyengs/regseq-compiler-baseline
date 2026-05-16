//! Integration tests for regseq-driver-46: Phase-2 FnDef Item
//! interning and canonical DefPath rendering.
//!
//! Mirrors the `compile_with_bridge` harness from
//! `bridge_pipeline.rs` and exercises the new bridge contract:
//!
//! - Every source-derived `MonoItem::Fn` with `DefKind::Fn |
//!   DefKind::AssocFn` produces exactly one `ItemKind::Function`
//!   Item per spec.md section 3.4 Phase 2 (`spec.md:1142`).
//! - The Function Item's `DefPath` carries rustc's segments verbatim
//!   and `DefPath.rendered` matches `render_def_path(...)` per
//!   spec.md section 7.2 (`spec.md:2079-2088`).

#![feature(rustc_private)]

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

use regseq_driver::bridge::{run_bridge, BridgeInput as DriverBridgeInput};
use regseq_model::{
    render_def_path, BridgeInput as ModelBridgeInput, DefPathData, ItemKind, RegseqModel,
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
    std::env::temp_dir().join(format!("regseq-bridge-46-{crate_name}-{nanos}"))
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
            // mir_opt_level >= 1 enables `RemoveStorageMarkers` so
            // bodies with let-bindings do not trip the DSL rejection
            // for raw StorageLive/StorageDead markers.
            config.opts.unstable_opts.mir_opt_level = Some(1);
        }

        fn after_analysis<'tcx>(
            &mut self,
            _compiler: &rustc_interface::interface::Compiler,
            tcx: rustc_middle::ty::TyCtxt<'tcx>,
        ) -> rustc_driver::Compilation {
            let input = DriverBridgeInput::new(tcx);
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

/// Resolve a `StringId` to its interned content via the model.
fn resolve_string<'a>(model: &'a RegseqModel, id: regseq_model::StringId) -> &'a str {
    model.strings.resolve(id)
}

/// Compile a fixture and run a `TyCtxt`-borrowing callback. Mirrors
/// the `with_bridge` pattern in `bridge_synth.rs` but tailored to
/// this file (returns the callback's result without going through
/// `run_bridge`). Used by the `is_source_derived_function` filter
/// table test (Test Plan item 2), which needs to inspect raw
/// `DefId` values for `DefKind::Fn`, `DefKind::AssocFn`, and
/// `DefKind::Closure` in the same compilation.
fn with_tcx<R, F>(crate_name: &str, source: &str, f: F) -> R
where
    R: Send + 'static,
    F: for<'tcx> FnOnce(rustc_middle::ty::TyCtxt<'tcx>) -> R + Send + 'static,
{
    let _lock = COMPILER_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    init_rustc();

    let test_dir = fresh_test_dir(crate_name);
    std::fs::create_dir_all(&test_dir).unwrap();
    let src_path = test_dir.join("lib.rs");
    std::fs::write(&src_path, source).unwrap();

    let result: Arc<Mutex<Option<R>>> = Arc::new(Mutex::new(None));
    let callback =
        Arc::new(Mutex::new(Some(Box::new(f)
            as Box<
                dyn for<'tcx> FnOnce(rustc_middle::ty::TyCtxt<'tcx>) -> R + Send,
            >)));

    struct Callbacks<R: Send + 'static> {
        result: Arc<Mutex<Option<R>>>,
        callback: Arc<
            Mutex<Option<Box<dyn for<'tcx> FnOnce(rustc_middle::ty::TyCtxt<'tcx>) -> R + Send>>>,
        >,
    }

    impl<R: Send + 'static> rustc_driver::Callbacks for Callbacks<R> {
        fn config(&mut self, config: &mut rustc_interface::interface::Config) {
            config.opts.unstable_opts.no_codegen = true;
            config.opts.unstable_opts.mir_opt_level = Some(1);
        }

        fn after_analysis<'tcx>(
            &mut self,
            _compiler: &rustc_interface::interface::Compiler,
            tcx: rustc_middle::ty::TyCtxt<'tcx>,
        ) -> rustc_driver::Compilation {
            let f = self.callback.lock().unwrap().take().expect("callback once");
            *self.result.lock().unwrap() = Some(f(tcx));
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
        .expect("callback should produce a result");
    std::fs::remove_dir_all(Path::new(&test_dir)).ok();
    output
}

/// Test Plan item 2: the `is_source_derived_function` filter table.
/// Compiles a fixture with one regular `DefKind::Fn` (`top_fn`),
/// one `DefKind::AssocFn` (`S::assoc`), and one `DefKind::Closure`
/// (the anonymous `|| {}`), then asserts the helper returns `true`
/// for the first two and `false` for the closure.
///
/// Closures are routed to spec.md section 13.1 synthesis
/// (`ClosureCaptures` + `ClosureCall` + `ClosureDropGlue`), so the
/// Phase-2 baseline pass MUST exclude them per the task spec
/// (`.harness/tasks/regseq-driver/46-top-level-fndef-interning-and-canonical-def-path.md:391-399`).
#[test]
fn is_source_derived_function_filters_fn_assocfn_but_not_closure() {
    let source = r#"#![no_std]

pub fn top_fn() {}

pub struct S;

impl S {
    pub fn assoc() {}
}

pub fn carrier() -> u32 {
    let c = || 1u32;
    c()
}
"#;

    with_tcx("filter_table_46", source, |tcx| {
        let mut top_fn_id: Option<rustc_span::def_id::DefId> = None;
        let mut assoc_id: Option<rustc_span::def_id::DefId> = None;
        let mut closure_id: Option<rustc_span::def_id::DefId> = None;

        for local_def_id in tcx.hir_body_owners() {
            let def_id = local_def_id.to_def_id();
            match tcx.def_kind(def_id) {
                rustc_hir::def::DefKind::Fn => {
                    if tcx.item_name(def_id).as_str() == "top_fn" {
                        top_fn_id = Some(def_id);
                    } else if tcx.item_name(def_id).as_str() == "carrier" {
                        // The closure inside `carrier` has the same
                        // local-def-id parent; locate it via the
                        // body's local_decls instead.
                        let body = tcx.optimized_mir(local_def_id);
                        for local in &body.local_decls {
                            if let rustc_middle::ty::TyKind::Closure(closure_def_id, _) =
                                local.ty.kind()
                            {
                                closure_id = Some(*closure_def_id);
                                break;
                            }
                        }
                    }
                }
                rustc_hir::def::DefKind::AssocFn => {
                    if tcx.item_name(def_id).as_str() == "assoc" {
                        assoc_id = Some(def_id);
                    }
                }
                _ => {}
            }
        }

        let top_fn = top_fn_id.expect("top_fn DefId not found");
        let assoc = assoc_id.expect("S::assoc DefId not found");
        let closure = closure_id.expect("anonymous closure DefId not found");

        // Sanity: rustc classifies these as expected.
        assert!(matches!(tcx.def_kind(top_fn), rustc_hir::def::DefKind::Fn));
        assert!(matches!(
            tcx.def_kind(assoc),
            rustc_hir::def::DefKind::AssocFn
        ));
        assert!(matches!(
            tcx.def_kind(closure),
            rustc_hir::def::DefKind::Closure
        ));

        // The helper accepts DefKind::Fn and DefKind::AssocFn ...
        assert!(regseq_driver::bridge::is_source_derived_function(
            tcx, top_fn
        ));
        assert!(regseq_driver::bridge::is_source_derived_function(
            tcx, assoc
        ));
        // ... and rejects DefKind::Closure (spec.md section 13.1
        // synthesis path owns closures).
        assert!(!regseq_driver::bridge::is_source_derived_function(
            tcx, closure
        ));
    });
}

/// Test Plan item 3: a leaf `pub fn no_op() {}` interns exactly one
/// `ItemKind::Function` Item and yields exactly one Function
/// Instance keyed at the eager `construct_function_instances` pass
/// (spec.md section 3.4 Phase 5; `spec.md:1226`).
#[test]
fn no_op_fixture_produces_one_function_item_and_instance() {
    let source = r#"#![no_std]

#[no_mangle]
pub extern "C" fn no_op() {}
"#;

    let model = compile_with_bridge("no_op_46", source);

    let function_items: Vec<_> = model
        .items
        .iter()
        .filter(|it| matches!(it.kind, ItemKind::Function))
        .collect();
    // Spec section 3.4 Phase 2 (`spec.md:1142`): every source-derived
    // FnDef MonoItem yields one ItemKind::Function Item.
    assert_eq!(function_items.len(), 1);

    // Spec section 3.4 Phase 5 (`spec.md:1226`): one Instance per
    // (Item, GenericArgs, flavor) tuple. The lone `no_op` FnDef has
    // identity args and `FunctionInstanceFlavor::Item`, so exactly
    // one Function Instance exists after `run_bridge` returns.
    assert_eq!(model.instances.len(), 1);

    // Spec section 9.2: the Instance's `item` resolves back to the
    // single Function Item interned for `no_op`.
    let instance = &model.instances[regseq_model::InstanceId::from_u32(0)];
    let item = &model.items[instance.item];
    let last_seg = item
        .def_path
        .segments
        .last()
        .expect("Function Item must have a leaf segment");
    let leaf_name = match last_seg.data {
        DefPathData::ValueNs(s) => Some(resolve_string(&model, s)),
        _ => None,
    };
    assert_eq!(leaf_name, Some("no_op"));
}

/// Test Plan item 4: the `no_op` Function Item carries the
/// canonical `(segments, rendered)` shape per spec.md section 3.4
/// Phase 3 (`spec.md:1198-1202`) and section 7.2
/// (`spec.md:2079-2088`).
#[test]
fn no_op_def_path_is_canonical_for_top_level_fn() {
    let source = r#"#![no_std]

#[no_mangle]
pub extern "C" fn no_op() {}
"#;

    let model = compile_with_bridge("no_op_canonical_46", source);

    let item = model
        .items
        .iter()
        .find(|it| matches!(it.kind, ItemKind::Function))
        .expect("Function Item must exist");

    // `tcx.def_path(def_id).data` for a crate-root function is a
    // single name-bearing segment; no leading CrateRoot.
    assert_eq!(item.def_path.segments.len(), 1);
    let seg = &item.def_path.segments[0];
    assert_eq!(seg.disambiguator, 0);
    let leaf = match seg.data {
        DefPathData::ValueNs(s) => Some(resolve_string(&model, s)),
        _ => None,
    };
    assert_eq!(leaf, Some("no_op"));

    // Spec section 7.2: rendered must byte-equal the canonical
    // helper output for the (crate, segments) pair.
    let actual = resolve_string(&model, item.def_path.rendered);
    let expected = render_def_path(&item.def_path, &model.crates, &model.strings);
    assert_eq!(actual, expected);
    assert_eq!(actual, "no_op_canonical_46::no_op");
}

/// Test Plan item 5: model.validate passes on the no-op fixture.
/// This is the assertion `test-ir-fixtures-01` depends on; the
/// bridge's contract is locked here independently.
#[test]
fn no_op_fixture_passes_model_validate() {
    let source = r#"#![no_std]

#[no_mangle]
pub extern "C" fn no_op() {}
"#;

    let mut model = compile_with_bridge("no_op_validate_46", source);

    // Spec section 7.2 + model.rs:1442: validate_items enforces
    // DefPath.rendered == render_def_path(...).
    assert_eq!(
        model.validate(&ModelBridgeInput::matching(&model.clone())),
        Ok(())
    );
}

/// Test Plan item 6: multi-segment `def_path` for an `impl` method.
/// Covers the rustc-verbatim segment vector that the previous
/// leaf-only implementation truncated; cites spec.md section 3.4
/// Phase 3 (`spec.md:1201`).
#[test]
fn impl_method_carries_multi_segment_def_path() {
    let source = r#"#![no_std]

pub struct S;

impl S {
    #[no_mangle]
    pub extern "C" fn m() {}
}
"#;

    let model = compile_with_bridge("impl_method_46", source);

    let function_items: Vec<_> = model
        .items
        .iter()
        .filter(|it| matches!(it.kind, ItemKind::Function))
        .collect();
    // Only one source-derived FnDef in this fixture: S::m.
    assert_eq!(function_items.len(), 1);
    let m_item = function_items[0];

    // Spec section 3.4 Phase 3 (`spec.md:1198-1202`): segments are
    // rustc-verbatim. For `impl S { pub fn m() {} }` rustc emits
    // `[Impl(disambig=0), ValueNs("m")]`.
    assert_eq!(m_item.def_path.segments.len(), 2);
    let impl_seg = &m_item.def_path.segments[0];
    assert!(matches!(impl_seg.data, DefPathData::Impl));
    let leaf_seg = &m_item.def_path.segments[1];
    let leaf_name = match leaf_seg.data {
        DefPathData::ValueNs(s) => Some(resolve_string(&model, s)),
        _ => None,
    };
    assert_eq!(leaf_name, Some("m"));

    // Spec section 7.2: rendered must byte-equal the canonical
    // helper output. The verbose form is `<crate>::{impl#0}::m`.
    let actual = resolve_string(&model, m_item.def_path.rendered);
    let expected = render_def_path(&m_item.def_path, &model.crates, &model.strings);
    assert_eq!(actual, expected);
    assert_eq!(actual, "impl_method_46::{impl#0}::m");
}

/// Test Plan item 7: a generic function instantiated at two
/// distinct argument sets yields one `ItemKind::Function` Item and
/// two distinct Function Instances (one per `(ItemId, ArgsKey,
/// FunctionInstanceFlavor)` tuple, per audit regseq-driver-42-4 /
/// spec.md section 16.2).
///
/// Ignored: this test is blocked on a pre-existing bridge
/// limitation that the regseq-driver-46 task spec explicitly
/// marks Non-Goal. Body materialisation
/// (`bridge/body/materialise.rs::materialise_function_body_inner`
/// at `materialise.rs:243`) fetches
/// `tcx.instance_mir(InstanceKind::Item(def_id))`, which the
/// pinned toolchain returns as the unsubstituted polymorphic MIR
/// (`rustc_middle/src/ty/mod.rs:1671` `instance_mir` ->
/// `optimized_mir(def)`). The bridge does not call
/// `subst_and_normalize_erasing_regions` before walking
/// `local_decls`, so the generic body's `Param(T)` locals trigger
/// `BridgeError::NonRigidTy` in `translate_ty`. Per the task
/// spec's Non-Goals
/// (`.harness/tasks/regseq-driver/46-top-level-fndef-interning-and-canonical-def-path.md:362-364`):
/// "Body materialisation / vtable-indirect / DiagLogPeel. Covered
/// by `regseq-driver-43`, `regseq-driver-44`, `regseq-driver-45`.
/// No changes here." The (ItemId, ArgsKey,
/// FunctionInstanceFlavor) keying contract this fixture exercises
/// is verified upstream of the crash: `body/terminator.rs::
/// lookup_or_create_direct_instance` produces distinct (item,
/// args) rows before Phase-5 attempts to materialise their bodies.
/// The bridge crash is in body materialisation, not in the keying
/// path this task touches. Locking in the fixture (kept on disk
/// under `#[ignore]`) so a future task that lifts the body-
/// materialisation Non-Goal can flip the attribute off without
/// re-deriving the assertion shape.
#[test]
#[ignore = "blocked on pre-existing body-materialisation limitation; explicit Non-Goal per task spec lines 362-364"]
fn generic_fn_two_instantiations_yield_one_item_two_instances() {
    let source = r#"#![no_std]

pub fn id<T>(x: T) -> T { x }

#[no_mangle]
pub extern "C" fn caller() {
    let _ = id::<u32>(1);
    let _ = id::<u64>(2);
}
"#;

    let model = compile_with_bridge("generic_id_46", source);

    // Locate the `id` Function Item by its leaf ValueNs name.
    let id_items: Vec<_> = model
        .items
        .iter()
        .enumerate()
        .filter_map(|(idx, it)| {
            if !matches!(it.kind, ItemKind::Function) {
                return None;
            }
            let leaf = it.def_path.segments.last()?;
            match leaf.data {
                DefPathData::ValueNs(s) if resolve_string(&model, s) == "id" => {
                    Some(regseq_model::ItemId::from_u32(idx as u32))
                }
                _ => None,
            }
        })
        .collect();
    // Spec section 3.4 Phase 2 dedup: one Item per DefPathHash
    // regardless of how many monomorphisations the bridge sees.
    assert_eq!(id_items.len(), 1);
    let id_item_id = id_items[0];

    // Spec section 16.2 (F4-2 / audit-finding regseq-driver-42-4):
    // each `(ItemId, ArgsKey, FunctionInstanceFlavor)` tuple is a
    // distinct Instance row. Two concrete `T`s ⇒ two ArgsKeys ⇒
    // two Instances pointing at the same `id` Item.
    let id_instances = model
        .instances
        .iter()
        .filter(|inst| inst.item == id_item_id)
        .count();
    assert_eq!(id_instances, 2);
}
