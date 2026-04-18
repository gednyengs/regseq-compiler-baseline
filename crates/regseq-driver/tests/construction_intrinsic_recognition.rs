//! Pass 2 Rule 1 intrinsic recognition tests (regseq-model-45).
//!
//! These tests exercise the IR constructor's ability to turn MIR `Call`
//! terminators whose callee `DefId` is registered as a Regseq intrinsic
//! into [`TerminatorKind::IntrinsicCall`] terminators while leaving
//! unregistered callees as regular [`TerminatorKind::Call`].
//!
//! The production
//! [`IntrinsicRegistry::build`](regseq_driver::extract::intrinsic_registry::IntrinsicRegistry::build)
//! walks the `regseq` facade crate only, and `#[rseq::intrinsic]` is
//! not in Pass 1's `KNOWN_RSEQ_ATTRS` whitelist for local items -- so
//! the specimens here declare plain methods and free functions
//! without that attribute. Tests then seed the Pass 2
//! [`IntrinsicLookup`] directly from an explicit
//! `(DefId, canonical_name)` list via
//! [`IntrinsicLookup::from_entries`]. The translation path exercised
//! under test is identical to the production one; only the registry
//! source differs.

#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

use std::sync::{Arc, Mutex, Once};

use rustc_hir::def_id::DefId;
use rustc_middle::ty::TyCtxt;

use regseq_driver::construction::{
    build_body, build_pass1_output, ConstantIndex, ConstructionError, DerefCollapseLookup,
    ExtractionRegistries, FunctionIndex, IntrinsicLookup, Pass1Output, PinStateRegistry,
    TypeDefIndex,
};
use regseq_driver::extract::abstraction_types::AbstractionTypeRegistry;
use regseq_driver::extract::framework_types::FrameworkTypeRegistry;
use regseq_driver::extract::intrinsic_registry::IntrinsicRegistry;
use regseq_model::identity::SymbolInterner;
use regseq_model::ir::{Operand, Place, RegseqIntrinsic, TerminatorKind};

// ---------------------------------------------------------------------------
// rustc harness (mirrors construction_pass2.rs / construction_switch_and_loops.rs)
// ---------------------------------------------------------------------------

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
    let output = std::process::Command::new("rustc")
        .arg("--print")
        .arg("sysroot")
        .output()
        .expect("failed to run rustc --print sysroot");
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn with_specimen<R, F>(source: &str, test_name: &str, f: F) -> R
where
    R: Send + 'static,
    F: for<'tcx> FnOnce(TyCtxt<'tcx>, &Pass1Output) -> R + Send + 'static,
{
    let _lock = COMPILER_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    init_rustc();

    let test_dir = std::env::temp_dir()
        .join("regseq-driver-tests")
        .join(test_name);
    if test_dir.exists() {
        std::fs::remove_dir_all(&test_dir).unwrap();
    }
    std::fs::create_dir_all(&test_dir).unwrap();

    let src_path = test_dir.join("test_input.rs");
    std::fs::write(&src_path, source).unwrap();

    let sysroot = get_sysroot();

    let out: Arc<Mutex<Option<R>>> = Arc::new(Mutex::new(None));
    let callback_cell: Arc<Mutex<Option<Box<dyn FnOnce(TyCtxt<'_>, &Pass1Output) -> R + Send>>>> =
        Arc::new(Mutex::new(Some(Box::new(f))));

    struct Callbacks<R: Send + 'static> {
        out: Arc<Mutex<Option<R>>>,
        callback: Arc<Mutex<Option<Box<dyn FnOnce(TyCtxt<'_>, &Pass1Output) -> R + Send>>>>,
    }

    impl<R: Send + 'static> rustc_driver::Callbacks for Callbacks<R> {
        fn config(&mut self, config: &mut rustc_interface::interface::Config) {
            config.opts.unstable_opts.no_codegen = true;
            config
                .opts
                .unstable_opts
                .crate_attr
                .push("feature(register_tool)".to_string());
            config
                .opts
                .unstable_opts
                .crate_attr
                .push("register_tool(rseq)".to_string());
            config.opts.unstable_opts.crate_attr.push(
                "allow(unused, non_snake_case, nonstandard_style, redundant_semicolons)"
                    .to_string(),
            );
        }

        fn after_analysis<'tcx>(
            &mut self,
            _compiler: &rustc_interface::interface::Compiler,
            tcx: rustc_middle::ty::TyCtxt<'tcx>,
        ) -> rustc_driver::Compilation {
            let intrinsic = IntrinsicRegistry::build(tcx);
            let framework = FrameworkTypeRegistry::build(tcx);
            let abstraction = AbstractionTypeRegistry::build(tcx);
            let pin_states = PinStateRegistry::build(tcx);
            let registries = ExtractionRegistries {
                intrinsic: &intrinsic,
                framework_types: &framework,
                abstraction_types: &abstraction,
                pin_states: &pin_states,
            };
            let pass1 =
                build_pass1_output(tcx, &registries).expect("Pass 1 must succeed on this specimen");
            let f = self
                .callback
                .lock()
                .unwrap()
                .take()
                .expect("callback taken once");
            let result = f(tcx, &pass1);
            *self.out.lock().unwrap() = Some(result);
            rustc_driver::Compilation::Stop
        }
    }

    let mut callbacks = Callbacks::<R> {
        out: out.clone(),
        callback: callback_cell,
    };

    let args = vec![
        "rustc".to_string(),
        src_path.to_str().unwrap().to_string(),
        "--sysroot".to_string(),
        sysroot,
        "--edition".to_string(),
        "2021".to_string(),
        "--crate-type".to_string(),
        "lib".to_string(),
        "--crate-name".to_string(),
        "specimen".to_string(),
        "--out-dir".to_string(),
        test_dir.to_str().unwrap().to_string(),
        "-Zmir-opt-level=0".to_string(),
        "-Coverflow-checks=off".to_string(),
        "-Cdebug-assertions=off".to_string(),
    ];

    rustc_driver::run_compiler(&args, &mut callbacks);

    let result = out
        .lock()
        .unwrap()
        .take()
        .expect("callback must run in after_analysis");

    std::fs::remove_dir_all(&test_dir).ok();

    result
}

// ---------------------------------------------------------------------------
// HIR lookup helpers
// ---------------------------------------------------------------------------

fn find_free_fn_def_id<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> DefId {
    for item_id in tcx
        .hir_module_items(rustc_hir::def_id::LocalModDefId::CRATE_DEF_ID)
        .free_items()
    {
        let item = tcx.hir_item(item_id);
        if let rustc_hir::ItemKind::Fn { ident, .. } = &item.kind {
            if ident.name.as_str() == name {
                return item.owner_id.def_id.to_def_id();
            }
        }
    }
    panic!("function `{name}` not found in specimen");
}

fn find_method_def_id<'tcx>(tcx: TyCtxt<'tcx>, self_ty: &str, method: &str) -> DefId {
    for item_id in tcx
        .hir_module_items(rustc_hir::def_id::LocalModDefId::CRATE_DEF_ID)
        .free_items()
    {
        let item = tcx.hir_item(item_id);
        if let rustc_hir::ItemKind::Impl(impl_block) = &item.kind {
            let impl_owner = item.owner_id.def_id.to_def_id();
            let impl_ty = tcx.type_of(impl_owner).instantiate_identity();
            if format!("{impl_ty}") != self_ty {
                continue;
            }
            for ii in impl_block.items {
                let impl_item = tcx.hir_impl_item(*ii);
                if let rustc_hir::ImplItemKind::Fn(_, _) = &impl_item.kind {
                    if impl_item.ident.name.as_str() == method {
                        return impl_item.owner_id.def_id.to_def_id();
                    }
                }
            }
        }
    }
    panic!("method `{self_ty}::{method}` not found in specimen");
}

// ---------------------------------------------------------------------------
// Shared specimen: minimal #[rseq::driver] struct and a handful of
// callable items. The specimen does NOT carry a local
// `#[rseq::intrinsic(...)]` attribute -- that attribute is reserved
// for items inside the `regseq` facade crate and is not part of the
// `KNOWN_RSEQ_ATTRS` set Pass 1 accepts on local items.
// Tests construct [`IntrinsicLookup`] directly from the DefIds of
// selected functions to simulate the Phase 4 registry.
// ---------------------------------------------------------------------------

const INTRINSIC_SPECIMEN: &str = r#"
#![no_std]

pub struct Clock;

impl Clock {
    pub fn get_rate(&self) -> u32 { 0 }
}

pub struct PinCtrl;

impl PinCtrl {
    pub fn apply(&self) {}
}

pub fn busy_wait(_us: u64) {}

pub fn plain_helper(x: u32) -> u32 { x + 1 }

#[rseq::driver(name = "spec-drv", compatible = ["acme,widget"], bus = "amba")]
pub struct Drv {
    #[rseq::clock(id = "clk")]
    pub clk: Clock,
    #[rseq::pinctrl(id = "pins")]
    pub pins: PinCtrl,
}

impl Drv {
    pub fn call_method_intrinsic(&self) -> u32 {
        self.clk.get_rate()
    }

    pub fn call_pinctrl_apply(&self) {
        self.pins.apply()
    }

    pub fn call_free_intrinsic(&self) {
        busy_wait(100)
    }

    pub fn call_regular_function(&self) -> u32 {
        plain_helper(42)
    }
}
"#;

// ---------------------------------------------------------------------------
// Test 1: method intrinsic preserves &self as args[0]
// ---------------------------------------------------------------------------

#[test]
fn method_intrinsic_preserves_self_as_first_arg() {
    let outcome = with_specimen(
        INTRINSIC_SPECIMEN,
        "construction_intrinsic_recognition_method",
        |tcx, pass1| {
            let get_rate = find_method_def_id(tcx, "Clock", "get_rate");
            let caller = find_method_def_id(tcx, "Drv", "call_method_intrinsic");
            let caller_fn_id = *pass1
                .cross_refs
                .functions
                .get(&caller)
                .expect("caller must be registered in Pass 1");

            let lookup =
                IntrinsicLookup::from_entries(tcx, [(get_rate, "clock_get_rate")].into_iter())
                    .expect("canonical name must parse");
            assert_eq!(lookup.lookup(get_rate), Some(RegseqIntrinsic::ClockGetRate));

            let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
            let constants = ConstantIndex::new(&pass1.cross_refs.constants);
            let functions = FunctionIndex::new(&pass1.cross_refs.functions);
            let abstraction_registry = AbstractionTypeRegistry::default();
            let deref_lookup = DerefCollapseLookup::build(tcx, &abstraction_registry)
                .expect("Deref lang item must be resolvable on the specimen crate");
            let mut interner = SymbolInterner::new();
            let body = build_body(
                tcx,
                caller,
                caller_fn_id,
                &type_def_index,
                &constants,
                &functions,
                &lookup,
                &deref_lookup,
                &mut interner,
            )
            .expect("build_body must succeed on the caller method");

            // Reduce to just the IntrinsicCall terminator payload so the
            // return value escapes the rustc context.
            let mut found: Vec<(RegseqIntrinsic, usize, Option<Operand>, bool, bool)> = Vec::new();
            for bb in &body.basic_blocks {
                if let TerminatorKind::IntrinsicCall {
                    intrinsic,
                    args,
                    metadata,
                    error_target,
                    ..
                } = &bb.terminator.kind
                {
                    found.push((
                        *intrinsic,
                        args.len(),
                        args.first().cloned(),
                        metadata.is_some(),
                        error_target.is_some(),
                    ));
                }
            }
            found
        },
    );

    assert_eq!(
        outcome.len(),
        1,
        "exactly one IntrinsicCall expected, got {outcome:?}",
    );
    let (intrinsic, arg_count, first_arg, has_metadata, has_error_target) =
        outcome.into_iter().next().unwrap();
    assert_eq!(intrinsic, RegseqIntrinsic::ClockGetRate);
    assert_eq!(arg_count, 1, "&self receiver is preserved as args[0]");

    // The first arg is the `&self.clk` receiver. After drop
    // elaboration at `mir_opt_level = 0`, `&self.clk` is materialized
    // into a dedicated ref local, and the Call site consumes that
    // local -- so `args[0]` is an `Operand::Copy`/`Move` over a Place
    // with no projection. The important properties are: it is a Place
    // operand (not a literal), and the arg arity matches the
    // source-level receiver-only method signature (already checked
    // above via `arg_count == 1`).
    let first = first_arg.expect("intrinsic must carry an &self operand");
    assert!(
        matches!(first, Operand::Copy(_) | Operand::Move(_)),
        "first arg must be a Place operand (Copy/Move), got {first:?}",
    );
    let _place: Place = match first {
        Operand::Copy(p) | Operand::Move(p) => p,
        _ => unreachable!(),
    };

    assert!(
        !has_metadata,
        "task 45 emits metadata: None (tasks 50-52 populate it)",
    );
    assert!(
        !has_error_target,
        "task 45 emits error_target: None (task 46 populates it)",
    );
}

// ---------------------------------------------------------------------------
// Test 1b: pinctrl_apply method intrinsic preserves &self as args[0]
// ---------------------------------------------------------------------------
//
// Mirrors the clock_get_rate method-intrinsic check but exercises the
// PinCtrlApply variant to confirm the translation path is not
// specialized to a single canonical name. Spec Test Plan requires
// coverage of both `clock_get_rate` and `pinctrl_apply` on the
// method-intrinsic axis.

#[test]
fn pinctrl_apply_method_intrinsic_preserves_self_as_first_arg() {
    let outcome = with_specimen(
        INTRINSIC_SPECIMEN,
        "construction_intrinsic_recognition_pinctrl",
        |tcx, pass1| {
            let apply = find_method_def_id(tcx, "PinCtrl", "apply");
            let caller = find_method_def_id(tcx, "Drv", "call_pinctrl_apply");
            let caller_fn_id = *pass1
                .cross_refs
                .functions
                .get(&caller)
                .expect("caller must be registered in Pass 1");

            let lookup = IntrinsicLookup::from_entries(tcx, [(apply, "pinctrl_apply")].into_iter())
                .expect("canonical name must parse");

            let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
            let constants = ConstantIndex::new(&pass1.cross_refs.constants);
            let functions = FunctionIndex::new(&pass1.cross_refs.functions);
            let abstraction_registry = AbstractionTypeRegistry::default();
            let deref_lookup = DerefCollapseLookup::build(tcx, &abstraction_registry)
                .expect("Deref lang item must be resolvable on the specimen crate");
            let mut interner = SymbolInterner::new();
            let body = build_body(
                tcx,
                caller,
                caller_fn_id,
                &type_def_index,
                &constants,
                &functions,
                &lookup,
                &deref_lookup,
                &mut interner,
            )
            .expect("build_body must succeed on the caller method");

            let mut found: Vec<(RegseqIntrinsic, usize, bool, bool, bool)> = Vec::new();
            for bb in &body.basic_blocks {
                if let TerminatorKind::IntrinsicCall {
                    intrinsic,
                    args,
                    metadata,
                    error_target,
                    ..
                } = &bb.terminator.kind
                {
                    let first_is_place = matches!(
                        args.first(),
                        Some(Operand::Copy(_)) | Some(Operand::Move(_)),
                    );
                    found.push((
                        *intrinsic,
                        args.len(),
                        first_is_place,
                        metadata.is_some(),
                        error_target.is_some(),
                    ));
                }
            }
            found
        },
    );

    assert_eq!(
        outcome.len(),
        1,
        "exactly one PinCtrlApply IntrinsicCall expected, got {outcome:?}",
    );
    let (intrinsic, arg_count, first_is_place, has_metadata, has_error_target) = outcome[0];
    assert_eq!(intrinsic, RegseqIntrinsic::PinCtrlApply);
    assert_eq!(arg_count, 1, "&self receiver preserved as args[0]");
    assert!(
        first_is_place,
        "args[0] must be a Place operand carrying the receiver",
    );
    assert!(
        !has_metadata,
        "task 45 emits metadata: None for PinCtrlApply",
    );
    assert!(
        !has_error_target,
        "task 45 emits error_target: None for PinCtrlApply",
    );
}

// ---------------------------------------------------------------------------
// Test 2: free-function intrinsic has no prepended receiver
// ---------------------------------------------------------------------------

#[test]
fn free_function_intrinsic_does_not_prepend_receiver() {
    let outcome = with_specimen(
        INTRINSIC_SPECIMEN,
        "construction_intrinsic_recognition_free_fn",
        |tcx, pass1| {
            let busy_wait = find_free_fn_def_id(tcx, "busy_wait");
            let caller = find_method_def_id(tcx, "Drv", "call_free_intrinsic");
            let caller_fn_id = *pass1
                .cross_refs
                .functions
                .get(&caller)
                .expect("caller must be registered in Pass 1");

            let lookup = IntrinsicLookup::from_entries(tcx, [(busy_wait, "busy_wait")].into_iter())
                .expect("canonical name must parse");

            let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
            let constants = ConstantIndex::new(&pass1.cross_refs.constants);
            let functions = FunctionIndex::new(&pass1.cross_refs.functions);
            let abstraction_registry = AbstractionTypeRegistry::default();
            let deref_lookup = DerefCollapseLookup::build(tcx, &abstraction_registry)
                .expect("Deref lang item must be resolvable on the specimen crate");
            let mut interner = SymbolInterner::new();
            let body = build_body(
                tcx,
                caller,
                caller_fn_id,
                &type_def_index,
                &constants,
                &functions,
                &lookup,
                &deref_lookup,
                &mut interner,
            )
            .expect("build_body must succeed on the caller method");

            let mut found: Vec<(RegseqIntrinsic, usize)> = Vec::new();
            for bb in &body.basic_blocks {
                if let TerminatorKind::IntrinsicCall {
                    intrinsic, args, ..
                } = &bb.terminator.kind
                {
                    found.push((*intrinsic, args.len()));
                }
            }
            found
        },
    );

    assert_eq!(
        outcome.len(),
        1,
        "exactly one IntrinsicCall expected, got {outcome:?}",
    );
    let (intrinsic, arg_count) = outcome[0];
    assert_eq!(intrinsic, RegseqIntrinsic::BusyWait);
    // `busy_wait(100)` has exactly one source-level argument and no
    // synthetic receiver. Pass 2 preserves arg arity verbatim.
    assert_eq!(
        arg_count, 1,
        "free-function intrinsic must not have a synthetic receiver prepended",
    );
}

// ---------------------------------------------------------------------------
// Test 3: unregistered callee falls through to a regular `Call`
// ---------------------------------------------------------------------------

#[test]
fn unregistered_callee_falls_through_to_regular_call() {
    let outcome = with_specimen(
        INTRINSIC_SPECIMEN,
        "construction_intrinsic_recognition_fall_through",
        |tcx, pass1| {
            let caller = find_method_def_id(tcx, "Drv", "call_regular_function");
            let caller_fn_id = *pass1
                .cross_refs
                .functions
                .get(&caller)
                .expect("caller must be registered in Pass 1");

            // Empty registry: even though `clock_get_rate` and
            // `busy_wait` exist on the HIR with intrinsic attributes,
            // they should NOT be recognized here. The caller only
            // invokes `plain_helper`, which is not in the lookup
            // regardless.
            let lookup = IntrinsicLookup::default();

            let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
            let constants = ConstantIndex::new(&pass1.cross_refs.constants);
            let functions = FunctionIndex::new(&pass1.cross_refs.functions);
            let abstraction_registry = AbstractionTypeRegistry::default();
            let deref_lookup = DerefCollapseLookup::build(tcx, &abstraction_registry)
                .expect("Deref lang item must be resolvable on the specimen crate");
            let mut interner = SymbolInterner::new();
            let body = build_body(
                tcx,
                caller,
                caller_fn_id,
                &type_def_index,
                &constants,
                &functions,
                &lookup,
                &deref_lookup,
                &mut interner,
            )
            .expect("build_body must succeed on the caller method");

            let mut call_count = 0;
            let mut intrinsic_count = 0;
            for bb in &body.basic_blocks {
                match &bb.terminator.kind {
                    TerminatorKind::Call { .. } => call_count += 1,
                    TerminatorKind::IntrinsicCall { .. } => intrinsic_count += 1,
                    _ => {}
                }
            }
            (call_count, intrinsic_count)
        },
    );

    let (call_count, intrinsic_count) = outcome;
    assert!(
        call_count >= 1,
        "fall-through path must emit at least one regular Call",
    );
    assert_eq!(
        intrinsic_count, 0,
        "unregistered callee must not produce an IntrinsicCall",
    );
}

// ---------------------------------------------------------------------------
// Test 3b: production-path wiring through IntrinsicLookup::build
// ---------------------------------------------------------------------------
//
// Exercises the production wiring `IntrinsicLookup::build(tcx,
// registries.intrinsic)` used at Pass 2 setup. Since inline specimens
// do not depend on the `regseq` facade crate, the registry walk
// yields zero entries and the resulting lookup is empty -- which
// still drives the fall-through path for every Call terminator. The
// test verifies that `build` succeeds, the lookup is empty, and
// `build_body` operates identically to the hand-built empty lookup.

#[test]
fn intrinsic_lookup_build_integrates_with_extraction_registry() {
    let outcome = with_specimen(
        INTRINSIC_SPECIMEN,
        "construction_intrinsic_recognition_build_wiring",
        |tcx, pass1| {
            // Rebuild the exact same registries the Pass 1 orchestrator
            // consumed, then run them through `IntrinsicLookup::build`
            // (the production entry point).
            let intrinsic_registry = IntrinsicRegistry::build(tcx);
            let lookup = IntrinsicLookup::build(tcx, &intrinsic_registry)
                .expect("IntrinsicLookup::build must succeed over an empty/valid registry");
            let lookup_len = lookup.len();

            let caller = find_method_def_id(tcx, "Drv", "call_regular_function");
            let caller_fn_id = *pass1
                .cross_refs
                .functions
                .get(&caller)
                .expect("caller must be registered in Pass 1");

            let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
            let constants = ConstantIndex::new(&pass1.cross_refs.constants);
            let functions = FunctionIndex::new(&pass1.cross_refs.functions);
            let abstraction_registry = AbstractionTypeRegistry::default();
            let deref_lookup = DerefCollapseLookup::build(tcx, &abstraction_registry)
                .expect("Deref lang item must be resolvable on the specimen crate");
            let mut interner = SymbolInterner::new();
            let body = build_body(
                tcx,
                caller,
                caller_fn_id,
                &type_def_index,
                &constants,
                &functions,
                &lookup,
                &deref_lookup,
                &mut interner,
            )
            .expect("build_body must succeed when threaded with IntrinsicLookup::build output");

            let mut calls = 0;
            let mut intrinsic_calls = 0;
            for bb in &body.basic_blocks {
                match &bb.terminator.kind {
                    TerminatorKind::Call { .. } => calls += 1,
                    TerminatorKind::IntrinsicCall { .. } => intrinsic_calls += 1,
                    _ => {}
                }
            }
            (lookup_len, calls, intrinsic_calls)
        },
    );

    let (lookup_len, calls, intrinsic_calls) = outcome;
    assert_eq!(
        lookup_len, 0,
        "registry walk over a specimen without `regseq` dependency must be empty",
    );
    assert!(
        calls >= 1,
        "production wiring still translates Call terminators for unregistered callees",
    );
    assert_eq!(
        intrinsic_calls, 0,
        "empty registry -> no Call becomes an IntrinsicCall",
    );
}

// ---------------------------------------------------------------------------
// Test 4: unknown canonical name hard-errors at IntrinsicLookup setup
// ---------------------------------------------------------------------------

#[test]
fn unknown_canonical_name_hard_errors_at_setup() {
    let outcome = with_specimen(
        INTRINSIC_SPECIMEN,
        "construction_intrinsic_recognition_unknown_name",
        |tcx, _pass1| {
            // Use any real DefId from the specimen -- we just need
            // something the `ErrorProvenance::for_def_id` call can
            // resolve for the diagnostic.
            let some_def_id = find_free_fn_def_id(tcx, "plain_helper");
            let result =
                IntrinsicLookup::from_entries(tcx, [(some_def_id, "future_intrinsic")].into_iter());
            match result {
                Ok(_) => Err("expected UnknownIntrinsicCanonicalName, got Ok".to_string()),
                Err(ConstructionError::UnknownIntrinsicCanonicalName { canonical, .. }) => {
                    Ok(canonical)
                }
                Err(other) => Err(format!(
                    "expected UnknownIntrinsicCanonicalName, got {other:?}"
                )),
            }
        },
    );

    let canonical = outcome.expect("hand-built registry with unknown canonical must hard-error");
    assert_eq!(
        canonical, "future_intrinsic",
        "error payload must echo back the offending canonical name",
    );
}
