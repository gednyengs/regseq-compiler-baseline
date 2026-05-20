//! regseq-driver-56 Test Plan B1 + B2: static-dispatch trait-method
//! callee canonicalization.
//!
//! Bridges a fixture defining a trait, a concrete impl, and a top-level
//! fn that invokes the trait method on a concrete receiver, then asserts:
//!
//!   B1. Exactly two source-derived Function Items (the impl method and
//!       the caller); no Function Item is keyed on a trait DECLARATION
//!       DefId (predicate: `trait_of_assoc == Some(_) && impl_of_assoc
//!       == None`). Exactly two Function Instances (one per Function
//!       Item), with `body == Some(_)` on every source-defined fn. The
//!       caller body's `Terminator::Call` resolves to the impl method's
//!       Instance.
//!
//!   B2. Pretty-printer rendering of the caller body contains the impl
//!       method's DefPath at the call site (terminal `m` segment under
//!       a `{impl#N}` ancestor) and contains no body-less function
//!       declaration whose def_path traces to the trait declaration.

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

use regseq_driver::bridge::{run_bridge, BridgeInput};
use regseq_model::{
    CallCallee, DefPathData, InstanceKind, ItemId, PrettyPrinter, RegseqModel, TerminatorKind,
};
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
    std::env::temp_dir().join(format!("regseq-bridge-trait-method-{crate_name}-{nanos}"))
}

/// Per-Function-Item record of `tcx.trait_of_assoc` and
/// `tcx.impl_of_assoc`. Used by B1 to assert that no surviving
/// source-derived Function Item is keyed on a trait DECLARATION DefId.
struct TraitAssocPredicate {
    rendered: String,
    is_trait_decl: bool,
}

/// Bridge `source` and return both the resulting `RegseqModel` and a
/// per-Function-Item predicate vector capturing
/// `(trait_of_assoc.is_some() && impl_of_assoc.is_none())` for every
/// source-derived Function Item. The predicates are computed inside
/// the rustc callback because `TyCtxt<'tcx>` cannot escape the
/// after-analysis lifetime.
fn compile_with_bridge(crate_name: &str, source: &str) -> (RegseqModel, Vec<TraitAssocPredicate>) {
    let _lock = COMPILER_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    init_rustc();

    let test_dir = fresh_test_dir(crate_name);
    std::fs::create_dir_all(&test_dir).unwrap();
    let src_path = test_dir.join("lib.rs");
    std::fs::write(&src_path, source).unwrap();

    let result: Arc<Mutex<Option<(RegseqModel, Vec<TraitAssocPredicate>)>>> =
        Arc::new(Mutex::new(None));

    struct Callbacks {
        result: Arc<Mutex<Option<(RegseqModel, Vec<TraitAssocPredicate>)>>>,
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
            tcx: TyCtxt<'tcx>,
        ) -> rustc_driver::Compilation {
            let input = BridgeInput::new(tcx);
            let model = run_bridge(input).expect("bridge should run");
            let predicates = collect_trait_assoc_predicates(tcx, &model);
            *self.result.lock().unwrap() = Some((model, predicates));
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

fn collect_trait_assoc_predicates(
    tcx: TyCtxt<'_>,
    model: &RegseqModel,
) -> Vec<TraitAssocPredicate> {
    // Pre-compute (local_hash, stable_crate_id) -> DefId map for every
    // local DefId; we look each Item's recorded DefPathHash up in this
    // map to recover its rustc DefId without depending on the surface
    // hash-to-defid lookup.
    let mut hash_to_def_id: std::collections::HashMap<(u64, u64), rustc_hir::def_id::DefId> =
        std::collections::HashMap::new();
    for local_def_id in tcx.iter_local_def_id() {
        let did = local_def_id.to_def_id();
        let hash = tcx.def_path_hash(did);
        hash_to_def_id.insert(
            (hash.local_hash().as_u64(), hash.stable_crate_id().as_u64()),
            did,
        );
    }

    let mut out = Vec::new();
    for (idx, item) in model.items.iter().enumerate() {
        if !matches!(item.kind, regseq_model::ItemKind::Function) {
            continue;
        }
        if idx >= model.item_def_paths.len() {
            continue;
        }
        let hash = model.item_def_paths[ItemId::from_u32(idx as u32)];
        let rendered = model.strings.resolve(item.def_path.rendered).to_string();
        let Some(did) = hash_to_def_id
            .get(&(hash.local_hash, hash.stable_crate_id.0))
            .copied()
        else {
            // Non-local Item (e.g., upstream crate). Skip; per spec
            // section 8.1 the trait-DECLARATION rejection predicate
            // applies to source-derived Function Items in the primary
            // crate.
            continue;
        };
        let trait_of = tcx.trait_of_assoc(did).is_some();
        let impl_of = tcx.impl_of_assoc(did).is_some();
        out.push(TraitAssocPredicate {
            rendered,
            is_trait_decl: trait_of && !impl_of,
        });
    }
    out
}

const STATIC_DISPATCH_FIXTURE: &str = r#"#![no_std]
pub trait T { fn m(&self) -> u32; }
pub struct S;
impl T for S { fn m(&self) -> u32 { 42 } }
#[no_mangle]
pub extern "C" fn caller(s: &S) -> u32 { s.m() }
"#;

#[test]
fn b1_static_dispatch_call_resolves_to_impl_method_instance() {
    let (model, predicates) = compile_with_bridge("regseq_driver_56_b1", STATIC_DISPATCH_FIXTURE);

    // Exactly two source-derived Function Items: the impl method and
    // the caller. Filter out any Items not produced from source code
    // (synthetic shims carry a non-`None` `item_synthetic_kinds`
    // entry).
    let mut source_function_items: Vec<ItemId> = Vec::new();
    for (idx, item) in model.items.iter().enumerate() {
        if !matches!(item.kind, regseq_model::ItemKind::Function) {
            continue;
        }
        let item_id = ItemId::from_u32(idx as u32);
        if idx < model.item_synthetic_kinds.len()
            && !matches!(
                model.item_synthetic_kinds[item_id],
                regseq_model::SyntheticItemKind::None
            )
        {
            continue;
        }
        source_function_items.push(item_id);
    }
    assert_eq!(
        source_function_items.len(),
        2,
        "expected exactly 2 source-derived Function Items (impl-method + caller), got {}: {:?}",
        source_function_items.len(),
        source_function_items
            .iter()
            .map(|i| model.strings.resolve(model.items[*i].def_path.rendered))
            .collect::<Vec<_>>(),
    );

    // No source-derived Function Item is keyed on a trait DECLARATION
    // DefId (`trait_of_assoc == Some(_) && impl_of_assoc == None`).
    for predicate in &predicates {
        assert!(
            !predicate.is_trait_decl,
            "Function Item {:?} is keyed on a trait DECLARATION DefId (trait_of_assoc=Some, impl_of_assoc=None) -- regseq-driver-56 spec section 8.1 forbids this",
            predicate.rendered,
        );
    }

    // Exactly two Function Instances; every source-derived Function
    // Instance has `body == Some(_)`.
    let function_instances: Vec<&regseq_model::Instance> = model
        .instances
        .iter()
        .filter(|inst| matches!(inst.kind, InstanceKind::Function(_)))
        .collect();
    assert_eq!(
        function_instances.len(),
        2,
        "expected exactly 2 Function Instances, got {}",
        function_instances.len(),
    );
    for inst in &function_instances {
        let Some(function) = inst.kind.as_function() else {
            unreachable!()
        };
        assert!(
            function.body.is_some(),
            "source-defined Function Instance {} has body == None (spec section 9.4 reserves body: None for foreign declarations)",
            model.strings.resolve(inst.emission_name),
        );
    }

    // The caller's body contains a Terminator::Call whose callee is
    // `CallCallee::Direct(_)` resolving to the impl method's Instance.
    let caller_instance = function_instances
        .iter()
        .find(|inst| {
            let rendered = model.strings.resolve(inst.emission_name);
            rendered.ends_with("::caller")
        })
        .expect("caller Function Instance must be present");
    let caller_function = caller_instance.kind.as_function().unwrap();
    let body = caller_function
        .body
        .as_ref()
        .expect("caller Function Instance must have a body");

    let mut saw_impl_method_call = false;
    for block in body.basic_blocks.iter() {
        let TerminatorKind::Call { callee, .. } = &block.terminator.kind else {
            continue;
        };
        let CallCallee::Direct(callee_inst_id) = callee else {
            continue;
        };
        let callee_inst = &model.instances[*callee_inst_id];
        let callee_item = &model.items[callee_inst.item];
        // Resolved impl-method DefPath: terminal segment `m` (the
        // method name) with a `DefPathData::Impl` ancestor.
        let Some(last) = callee_item.def_path.segments.last() else {
            continue;
        };
        let leaf_name = match last.data {
            DefPathData::ValueNs(id) | DefPathData::TypeNs(id) => model.strings.resolve(id),
            _ => continue,
        };
        if leaf_name != "m" {
            continue;
        }
        let has_impl_ancestor = callee_item
            .def_path
            .segments
            .iter()
            .any(|s| matches!(s.data, DefPathData::Impl));
        if has_impl_ancestor {
            saw_impl_method_call = true;
            break;
        }
    }
    assert!(
        saw_impl_method_call,
        "caller body's Terminator::Call must resolve to the impl-method Instance (terminal `m` segment with `{{impl#N}}` ancestor); rendered: {}",
        PrettyPrinter::new(&model).render(),
    );
}

#[test]
fn b2_pretty_printer_renders_resolved_def_path_at_call_site() {
    let (model, _predicates) = compile_with_bridge("regseq_driver_56_b2", STATIC_DISPATCH_FIXTURE);

    let rendered = PrettyPrinter::new(&model).render();

    // The rendered caller body contains the impl method's DefPath at
    // the call site: terminal `m(` opening under a `{impl#` segment.
    assert!(
        rendered.contains("{impl#"),
        "rendered model must contain at least one `{{impl#N}}` def-path segment from the resolved impl method; rendered:\n{rendered}",
    );
    assert!(
        rendered.contains("}::m("),
        "rendered model must contain a call to the resolved impl method's `m(`; rendered:\n{rendered}",
    );

    // No body-less Function declaration whose def_path traces to the
    // trait declaration. Per spec.md section 9.4 `body: None` is
    // reserved for foreign declarations; the trait-method DefId
    // surfacing as a body-less Function Instance is the regseq-driver-56
    // bug being fixed. After the fix the only body-less Function
    // declaration legitimate in this fixture is the foreign-decl
    // surface (none here), so `T::m(...);` must not appear.
    assert!(
        !rendered.contains("T::m("),
        "rendered model must not contain a body-less `T::m(...)` Function declaration (trait declaration DefId leak per regseq-driver-56); rendered:\n{rendered}",
    );
}
