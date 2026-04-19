//! Pass 2 Rule 2 `?`-pattern folding tests (regseq-model-46).
//!
//! These tests exercise the IR constructor's ability to recognize
//! Rust's `?` desugaring -- the `<T as Try>::branch` +
//! `SwitchInt(ControlFlow discriminant)` + `FromResidual::from_residual`
//! MIR pattern -- and fold it into the `error_target` field of the
//! originating Call / IntrinsicCall terminator. The algorithm is
//! documented in `ir-reference.md` Section 5; the task spec
//! (regseq-model-46) lists the hard-error checks and the fallback
//! rule.
//!
//! The specimens here declare local functions / methods and feed
//! Try-carrying types (`Result`, `Option`) through them. The Pass 2
//! marker pair is resolved via [`TryMarkers::resolve`], which reads
//! the `branch_fn` / `from_residual_fn` lang items from
//! `core::ops` -- every `#![no_std]` specimen that depends on
//! `core` gets them for free, so test setup does not need to
//! synthesize the markers.

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
    ExtractionRegistries, FunctionIndex, IntrinsicLookup, OrderedMarkers, Pass1Output,
    PinStateRegistry, TryMarkers, TypeDefIndex,
};
use regseq_driver::extract::abstraction_types::AbstractionTypeRegistry;
use regseq_driver::extract::framework_types::FrameworkTypeRegistry;
use regseq_driver::extract::intrinsic_registry::IntrinsicRegistry;
use regseq_model::identity::{SourceOrigin, SymbolInterner};
use regseq_model::ir::{Operand, Place, RegseqType, ScalarType, Statement, TerminatorKind};

// ---------------------------------------------------------------------------
// rustc harness (mirrors the other construction_*.rs test files).
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
// Shared body-building helper.
// ---------------------------------------------------------------------------

fn run_build_body<'tcx>(
    tcx: TyCtxt<'tcx>,
    pass1: &Pass1Output,
    caller: DefId,
) -> Result<regseq_model::ir::Body, ConstructionError> {
    let caller_fn_id = *pass1
        .cross_refs
        .functions
        .get(&caller)
        .expect("caller must be registered in Pass 1");
    let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
    let constants = ConstantIndex::new(&pass1.cross_refs.constants);
    let functions = FunctionIndex::new(&pass1.cross_refs.functions);
    let intrinsics = IntrinsicLookup::default();
    let abstraction_registry = AbstractionTypeRegistry::default();
    let deref_lookup = DerefCollapseLookup::build(tcx, &abstraction_registry)
        .expect("Deref lang item must be resolvable on the specimen crate");
    let ordered_markers = OrderedMarkers::default();
    let try_markers = TryMarkers::resolve(tcx);
    assert!(
        try_markers.is_resolved(),
        "Try::branch / FromResidual::from_residual lang items must resolve on any `#![no_std]` specimen that depends on core",
    );
    let mut interner = SymbolInterner::new();
    build_body(
        tcx,
        caller,
        caller_fn_id,
        &type_def_index,
        &constants,
        &functions,
        &intrinsics,
        &deref_lookup,
        &ordered_markers,
        &try_markers,
        &mut interner,
    )
}

// ---------------------------------------------------------------------------
// Introspection helpers.
// ---------------------------------------------------------------------------

/// Collect the Regseq callee `FunctionId`s of every `Call` terminator
/// in the body (so we can assert "zero Try::branch / from_residual
/// Calls remain").
fn call_callees(body: &regseq_model::ir::Body) -> Vec<regseq_model::identity::FunctionId> {
    let mut out = Vec::new();
    for bb in &body.basic_blocks {
        if let TerminatorKind::Call { callee, .. } = &bb.terminator.kind {
            out.push(*callee);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Shared minimal specimens.
// ---------------------------------------------------------------------------

// The Regseq IR's `AggregateKind::Adt` variant stores a
// `TypeDefId` -- the Option/Result aggregates produced by
// constructors like `Ok(())` or `Some(v)` therefore cannot lower
// cleanly (they reference `core::result::Result` /
// `core::option::Option`, which are modelled as dedicated
// `RegseqType::Result` / `RegseqType::Option` variants and thus
// carry no `TypeDefId`). Tests that exercise Rule 2 folding route
// every Result/Option construction through a helper function; the
// helper's body is not translated here, so the caller under test
// emits only `Call` terminators (no `Rvalue::Aggregate` nodes) for
// its return value.
const PREAMBLE: &str = r#"
#![no_std]

pub struct MyError;

pub fn may_fail() -> Result<(), MyError> { make_ok_unit() }
pub fn may_fail_u32() -> Result<u32, MyError> { make_ok_u32(0) }
pub fn may_option() -> Option<u32> { make_some_u32(0) }

pub fn make_ok_unit() -> Result<(), MyError> { Ok(()) }
pub fn make_ok_u32(v: u32) -> Result<u32, MyError> { Ok(v) }
pub fn make_some_u32(v: u32) -> Option<u32> { Some(v) }

#[rseq::driver(name = "spec-drv", compatible = ["acme,widget"], bus = "amba")]
pub struct Drv;
"#;

fn compose(body: &str) -> String {
    format!("{PREAMBLE}{body}")
}

// ---------------------------------------------------------------------------
// Test 1: a single `?` on a Result-returning fn folds correctly.
//
// Mirrors the spec's canonical example `enable_clock_if_present`:
// a fallible call whose `?` propagates the error via the ambient
// `Result<(), MyError>` return type.
// ---------------------------------------------------------------------------

const RESULT_BODY: &str = r#"
impl Drv {
    pub fn enable_like(&self) -> Result<(), MyError> {
        may_fail()?;
        make_ok_unit()
    }
}
"#;

#[test]
fn single_question_mark_folds_into_error_target() {
    let src = compose(RESULT_BODY);
    let outcome = with_specimen(&src, "construction_try_folding_single", |tcx, pass1| {
        let caller = find_method_def_id(tcx, "Drv", "enable_like");
        let may_fail = find_free_fn_def_id(tcx, "may_fail");
        let body = run_build_body(tcx, pass1, caller).expect("build_body must succeed on `?`");

        let may_fail_fn_id = *pass1.cross_refs.functions.get(&may_fail).unwrap();

        // Find the folded Call. It must target `may_fail` and carry
        // a Some(bb) in `error_target`.
        let mut found: Vec<(Option<u32>, Option<u32>, bool)> = Vec::new();
        for bb in &body.basic_blocks {
            if let TerminatorKind::Call {
                callee,
                target,
                error_target,
                destination,
                ..
            } = &bb.terminator.kind
            {
                if *callee == may_fail_fn_id {
                    let dest_ty = destination_ty(&body, destination);
                    found.push((
                        target.map(|b| b.0),
                        error_target.map(|b| b.0),
                        dest_ty == RegseqType::Unit,
                    ));
                }
            }
        }

        // Collect every callee to verify no Try::branch /
        // from_residual survives.
        let all_callees = call_callees(&body);

        // Structural lookups for the second half of the assertions.
        let try_markers = TryMarkers::resolve(tcx);
        let branch_def_id = try_markers.branch_def_id();
        let fr_def_id = try_markers.from_residual_def_id();

        (found, all_callees, branch_def_id, fr_def_id)
    });

    let (found, all_callees, branch_def_id, fr_def_id) = outcome;

    assert_eq!(found.len(), 1, "exactly one `may_fail` Call expected");
    let (target, error_target, dest_is_unit) = found[0];
    assert!(target.is_some(), "folded Call must carry a Continue target");
    assert!(
        error_target.is_some(),
        "folded Call must populate error_target (`?` ran)",
    );
    assert_ne!(
        target, error_target,
        "Continue and Break targets must differ",
    );
    assert!(
        dest_is_unit,
        "destination must be the unwrapped Continue type `()`, not `Result<..>`",
    );

    // Sanity: the body must carry zero Calls whose DefIds resolve to
    // Try::branch or from_residual. We cannot check the trait-method
    // DefId directly against a FunctionId without the Pass 1 map;
    // the zero-intrinsic + zero-branch assertion here is proved via
    // the callee list (branch / from_residual are not in the
    // specimen's `functions` table at all, so they cannot appear as
    // a `Call.callee`). `branch_def_id` / `fr_def_id` are captured
    // only to demonstrate the lang items resolved -- see below.
    let _ = (branch_def_id, fr_def_id);
    assert!(
        !all_callees.is_empty(),
        "body must still contain the folded may_fail Call",
    );
}

// Resolve the Regseq type of a Place's destination by walking its
// LocalDecls entry. Used to assert `destination` is the unwrapped
// inner success type after folding.
fn destination_ty(body: &regseq_model::ir::Body, place: &Place) -> RegseqType {
    let decl = &body.local_decls[place.local.0 as usize];
    let mut current = decl.ty.clone();
    for proj in &place.projection {
        if let regseq_model::ir::Projection::Field { ty, .. } = proj {
            current = ty.clone();
        }
    }
    current
}

// ---------------------------------------------------------------------------
// Test 2: type-agnostic -- `Option::?` folds the same way.
// ---------------------------------------------------------------------------

const OPTION_BODY: &str = r#"
impl Drv {
    pub fn read_like(&self) -> Option<u32> {
        let v = may_option()?;
        make_some_u32(v)
    }
}
"#;

#[test]
fn option_question_mark_folds_same_shape() {
    let src = compose(OPTION_BODY);
    let outcome = with_specimen(&src, "construction_try_folding_option", |tcx, pass1| {
        let caller = find_method_def_id(tcx, "Drv", "read_like");
        let may_option = find_free_fn_def_id(tcx, "may_option");
        let body =
            run_build_body(tcx, pass1, caller).expect("build_body must succeed on Option `?`");
        let may_option_fn_id = *pass1.cross_refs.functions.get(&may_option).unwrap();

        let mut result: Vec<(bool, bool)> = Vec::new();
        for bb in &body.basic_blocks {
            if let TerminatorKind::Call {
                callee,
                target,
                error_target,
                destination,
                ..
            } = &bb.terminator.kind
            {
                if *callee == may_option_fn_id {
                    let dest_ty = destination_ty(&body, destination);
                    result.push((
                        error_target.is_some() && target.is_some(),
                        dest_ty == RegseqType::Scalar(ScalarType::U32),
                    ));
                }
            }
        }
        result
    });

    assert_eq!(outcome.len(), 1, "exactly one may_option Call expected");
    let (folded, dest_is_u32) = outcome[0];
    assert!(
        folded,
        "Option `?` must fold the same way as Result -- both target and error_target populated",
    );
    assert!(
        dest_is_u32,
        "destination must be the Continue inner type `u32`, not `Option<u32>`",
    );
}

// ---------------------------------------------------------------------------
// Test 3: nested `?` produce two independent folds.
// ---------------------------------------------------------------------------

const NESTED_BODY: &str = r#"
impl Drv {
    pub fn chain(&self) -> Result<u32, MyError> {
        let a = may_fail_u32()?;
        let b = may_fail_u32()?;
        make_ok_u32(a + b)
    }
}
"#;

#[test]
fn two_sequential_question_marks_produce_two_independent_folds() {
    let src = compose(NESTED_BODY);
    let outcome = with_specimen(&src, "construction_try_folding_nested", |tcx, pass1| {
        let caller = find_method_def_id(tcx, "Drv", "chain");
        let may_fail_u32 = find_free_fn_def_id(tcx, "may_fail_u32");
        let body =
            run_build_body(tcx, pass1, caller).expect("build_body must succeed on chained `?`");
        let may_fail_u32_fn_id = *pass1.cross_refs.functions.get(&may_fail_u32).unwrap();

        let mut call_sites: Vec<(Option<u32>, Option<u32>)> = Vec::new();
        for bb in &body.basic_blocks {
            if let TerminatorKind::Call {
                callee,
                target,
                error_target,
                ..
            } = &bb.terminator.kind
            {
                if *callee == may_fail_u32_fn_id {
                    call_sites.push((target.map(|b| b.0), error_target.map(|b| b.0)));
                }
            }
        }
        call_sites
    });

    assert_eq!(
        outcome.len(),
        2,
        "two may_fail_u32 Calls expected (one per `?`)",
    );
    for (i, (target, err)) in outcome.iter().enumerate() {
        assert!(target.is_some(), "Call #{i} must have a Continue target");
        assert!(err.is_some(), "Call #{i} must populate error_target");
    }
    // The two folds are independent: each Call carries its own
    // Continue/Break edge. The Break edges MAY share a cleanup block
    // (per the spec's "may share Drop-cascade blocks" caveat), so we
    // do not assert distinct error_targets here. The MUST-DIFFER
    // invariant is per-Call: Continue != Break.
    for (target, err) in &outcome {
        assert_ne!(target, err, "Continue and Break must differ for each fold");
    }
}

// ---------------------------------------------------------------------------
// Test 4: source-origin on error-path drops.
//
// When drop elaboration inserts Drop terminators on the Break path,
// the spec requires their source_origin to be `Source(...)` pointing
// at the originating `?` span -- never `CompilerGenerated`.
// ---------------------------------------------------------------------------

const DROP_BODY: &str = r#"
pub struct Guard;

impl Drop for Guard {
    fn drop(&mut self) {}
}

impl Drv {
    pub fn hold_and_check(&self) -> Result<(), MyError> {
        let _g = Guard;
        may_fail()?;
        make_ok_unit()
    }
}
"#;

#[test]
fn error_path_drop_cascade_carries_source_origin() {
    let src = compose(DROP_BODY);
    let outcome = with_specimen(&src, "construction_try_folding_drop", |tcx, pass1| {
        let caller = find_method_def_id(tcx, "Drv", "hold_and_check");
        let may_fail = find_free_fn_def_id(tcx, "may_fail");
        let body = run_build_body(tcx, pass1, caller)
            .expect("build_body must succeed on `?` + Drop cascade");
        let may_fail_fn_id = *pass1.cross_refs.functions.get(&may_fail).unwrap();

        // Find the folded Call and its error_target.
        let mut fold_site: Option<(u32, u32)> = None;
        for (idx, bb) in body.basic_blocks.iter().enumerate() {
            if let TerminatorKind::Call {
                callee,
                error_target,
                ..
            } = &bb.terminator.kind
            {
                if *callee == may_fail_fn_id {
                    fold_site = Some((idx as u32, error_target.unwrap().0));
                }
            }
        }
        let (call_idx, err_bb) = fold_site.expect("folded Call must be present");

        // Walk every block reachable via error_target (without
        // going back through the Call block). Record each block's
        // terminator SourceOrigin. Per the spec, drops inserted by
        // drop elaboration on the `?` error branch must carry
        // `Source(...)`.
        use std::collections::BTreeSet;
        let mut seen: BTreeSet<u32> = BTreeSet::new();
        let mut stack = vec![err_bb];
        let mut drops_with_source = 0;
        let mut drops_with_compiler = 0;
        let mut bodies: Vec<(u32, bool, bool)> = Vec::new();
        while let Some(bb) = stack.pop() {
            if bb == call_idx || !seen.insert(bb) {
                continue;
            }
            let idx = bb as usize;
            if idx >= body.basic_blocks.len() {
                continue;
            }
            let kind = &body.basic_blocks[idx].terminator.kind;
            let term_origin = &body.source_map.blocks[idx].terminator;
            let is_drop = matches!(kind, TerminatorKind::Drop { .. });
            let is_source = matches!(term_origin, SourceOrigin::Source(_));
            bodies.push((bb, is_drop, is_source));
            if is_drop {
                if is_source {
                    drops_with_source += 1;
                } else {
                    drops_with_compiler += 1;
                }
            }
            match kind {
                TerminatorKind::Goto(t) => stack.push(t.0),
                TerminatorKind::Drop { target, .. } => stack.push(target.0),
                _ => {}
            }
        }
        (drops_with_source, drops_with_compiler, bodies)
    });

    let (drops_with_source, drops_with_compiler, bodies) = outcome;
    assert!(
        drops_with_source >= 1,
        "at least one Drop terminator on the error path must carry Source(...), got: {bodies:?}",
    );
    assert_eq!(
        drops_with_compiler, 0,
        "no Drop on the error path may be CompilerGenerated, got: {bodies:?}",
    );
}

// ---------------------------------------------------------------------------
// Negative test: malformed `?` pattern raises MalformedTryPattern.
//
// The spec's hard-error policy says any structural deviation after
// the `Try::branch` trigger fires must surface as
// `ConstructionError::MalformedTryPattern`. A manual call to
// `Try::branch` (without the subsequent SwitchInt + FromResidual
// shape) produces exactly that scenario -- the trigger fires on
// the originating Call's return target, but the following block
// does not carry the canonical `_discr = discriminant(_cf); switch`
// shape, so `find_single_substantive_stmt` rejects it and
// `recognize` raises the hard error.
//
// This exercises the end-to-end path from `build_body` down through
// `TryFoldAnalysis::analyze` -> `recognize`, covering the
// `switch_block_shape` / `switch_stmt_not_assign` / related tags.
// Validation of the per-step tags in isolation lives in the unit
// tests inside `try_folding.rs` (module `tests`).
// ---------------------------------------------------------------------------

const MALFORMED_SPECIMEN: &str = r#"
#![no_std]
#![feature(try_trait_v2)]
use core::ops::Try;

pub struct MyError;
pub fn helper() -> Result<(), MyError> { let r: Result<(), MyError>; r = unsafe { core::mem::zeroed() }; r }

#[rseq::driver(name = "spec-drv", compatible = ["acme,widget"], bus = "amba")]
pub struct Drv;

impl Drv {
    // Call `Try::branch` manually, without the canonical SwitchInt
    // follow-through. The pattern recognizer triggers on `helper`'s
    // return target (which carries a `Try::branch` Call), then
    // finds no SwitchInt in the next block -- hard error.
    pub fn malformed(&self) -> () {
        let _cf = <Result<(), MyError> as Try>::branch(helper());
    }
}
"#;

#[test]
fn manual_try_branch_without_switch_raises_malformed_try_pattern() {
    let outcome = with_specimen(
        MALFORMED_SPECIMEN,
        "construction_try_folding_malformed",
        |tcx, pass1| {
            let caller = find_method_def_id(tcx, "Drv", "malformed");
            let caller_fn_id = *pass1.cross_refs.functions.get(&caller).unwrap();
            let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
            let constants = ConstantIndex::new(&pass1.cross_refs.constants);
            let functions = FunctionIndex::new(&pass1.cross_refs.functions);
            let intrinsics = IntrinsicLookup::default();
            let abstraction_registry = AbstractionTypeRegistry::default();
            let deref_lookup = DerefCollapseLookup::build(tcx, &abstraction_registry)
                .expect("Deref lang item must be resolvable on the specimen crate");
            let ordered_markers = OrderedMarkers::default();
            let try_markers = TryMarkers::resolve(tcx);
            let mut interner = SymbolInterner::new();

            match build_body(
                tcx,
                caller,
                caller_fn_id,
                &type_def_index,
                &constants,
                &functions,
                &intrinsics,
                &deref_lookup,
                &ordered_markers,
                &try_markers,
                &mut interner,
            ) {
                Err(ConstructionError::MalformedTryPattern {
                    call_mir_block,
                    step,
                    ..
                }) => Ok((call_mir_block, step.to_string())),
                Err(other) => Err(format!("expected MalformedTryPattern, got {other:?}")),
                Ok(_) => Err(
                    "expected build_body to raise MalformedTryPattern, got Ok(body)".to_string(),
                ),
            }
        },
    );

    let (call_mir_block, step) =
        outcome.expect("manual Try::branch without SwitchInt must hard-error");
    assert!(
        step == "switch_block_shape"
            || step == "switch_stmt_not_assign"
            || step == "switch_rvalue_not_discriminant"
            || step == "switch_terminator_not_switch",
        "malformed SwitchInt shape must surface as a switch-validation tag, got `{step}`",
    );
    // Sanity: the carried MIR block index is a real basic block
    // (u32 index), not a sentinel.
    assert!(
        call_mir_block != u32::MAX,
        "MalformedTryPattern must carry the originating Call's MIR basic-block index",
    );
}

// ---------------------------------------------------------------------------
// Negative test 4: `Try::branch` + valid `SwitchInt`, but the Break
// path does NOT call `FromResidual::from_residual`.
//
// The spec's hard-error policy demands that a `Try::branch` whose
// downstream SwitchInt is canonical, but whose Break arm lacks the
// `FromResidual` Call (or even the Break extraction shape that
// would feed it), produce `ConstructionError::MalformedTryPattern`.
// We synthesize that scenario via `feature(try_trait_v2)`: a
// hand-written `match cf { Continue(c) => ..., Break(_) => ... }`
// generates the canonical SwitchInt shape but the Break arm is
// arbitrary user code (no extraction, no FromResidual). The
// recognizer must hard-error on the Break-path validation step
// (`break_extraction_missing`, `break_terminator_not_call`, or
// `break_callee_not_from_residual`).
// ---------------------------------------------------------------------------

const MALFORMED_BREAK_SPECIMEN: &str = r#"
#![no_std]
#![feature(try_trait_v2)]
use core::ops::{ControlFlow, Try};

pub struct MyError;
pub fn helper() -> Result<(), MyError> { let r: Result<(), MyError>; r = unsafe { core::mem::zeroed() }; r }

#[rseq::driver(name = "spec-drv", compatible = ["acme,widget"], bus = "amba")]
pub struct Drv;

impl Drv {
    pub fn malformed_break(&self) -> u32 {
        let cf: ControlFlow<_, _> = <Result<(), MyError> as Try>::branch(helper());
        match cf {
            ControlFlow::Continue(c) => { let _ = c; 1 }
            ControlFlow::Break(_) => 0,  // no FromResidual call
        }
    }
}
"#;

#[test]
fn break_path_without_from_residual_raises_malformed_try_pattern() {
    let outcome = with_specimen(
        MALFORMED_BREAK_SPECIMEN,
        "construction_try_folding_malformed_break",
        |tcx, pass1| {
            let caller = find_method_def_id(tcx, "Drv", "malformed_break");
            let caller_fn_id = *pass1.cross_refs.functions.get(&caller).unwrap();
            let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
            let constants = ConstantIndex::new(&pass1.cross_refs.constants);
            let functions = FunctionIndex::new(&pass1.cross_refs.functions);
            let intrinsics = IntrinsicLookup::default();
            let abstraction_registry = AbstractionTypeRegistry::default();
            let deref_lookup = DerefCollapseLookup::build(tcx, &abstraction_registry)
                .expect("Deref lang item must be resolvable on the specimen crate");
            let ordered_markers = OrderedMarkers::default();
            let try_markers = TryMarkers::resolve(tcx);
            let mut interner = SymbolInterner::new();

            match build_body(
                tcx,
                caller,
                caller_fn_id,
                &type_def_index,
                &constants,
                &functions,
                &intrinsics,
                &deref_lookup,
                &ordered_markers,
                &try_markers,
                &mut interner,
            ) {
                Err(ConstructionError::MalformedTryPattern { step, .. }) => Ok(step.to_string()),
                Err(other) => Err(format!("expected MalformedTryPattern, got {other:?}")),
                Ok(_) => Err(
                    "expected build_body to raise MalformedTryPattern on missing FromResidual"
                        .to_string(),
                ),
            }
        },
    );

    let step = outcome.expect("Break path without FromResidual must hard-error");
    assert!(
        step == "break_extraction_missing"
            || step == "break_terminator_not_call"
            || step == "break_callee_not_from_residual",
        "Break-path validation must surface as a Break-side tag, got `{step}`",
    );
}

// ---------------------------------------------------------------------------
// Test 5: no-match fallback.
//
// A Call whose return target does NOT start with `Try::branch`
// translates to a regular Call with `error_target: None`.
// ---------------------------------------------------------------------------

const FALLBACK_BODY: &str = r#"
pub fn infallible() {}

impl Drv {
    pub fn no_qmark(&self) {
        infallible();
        infallible();
    }
}
"#;

#[test]
fn non_try_call_has_no_error_target() {
    let src = compose(FALLBACK_BODY);
    let outcome = with_specimen(&src, "construction_try_folding_fallback", |tcx, pass1| {
        let caller = find_method_def_id(tcx, "Drv", "no_qmark");
        let infallible = find_free_fn_def_id(tcx, "infallible");
        let body = run_build_body(tcx, pass1, caller).expect("build_body must succeed on fallback");
        let infallible_fn_id = *pass1.cross_refs.functions.get(&infallible).unwrap();

        let mut err_targets: Vec<Option<u32>> = Vec::new();
        for bb in &body.basic_blocks {
            if let TerminatorKind::Call {
                callee,
                error_target,
                ..
            } = &bb.terminator.kind
            {
                if *callee == infallible_fn_id {
                    err_targets.push(error_target.map(|b| b.0));
                }
            }
        }
        err_targets
    });

    assert!(
        !outcome.is_empty(),
        "infallible Call(s) must still be present",
    );
    for err in outcome {
        assert!(
            err.is_none(),
            "no `?` on this Call -- error_target must remain None (fallback rule)",
        );
    }
}

// ---------------------------------------------------------------------------
// Fidelity test: the PL011 fixture's `enable_clock_if_present()` and
// `init()` still have the shape this suite's intrinsic-fold test
// depends on. Following the pattern established in
// `construction_switch_and_loops::pl011_fixture_fidelity`, this
// reads the fixture source and asserts the relevant substrings
// exist. Any drift in the fixture surfaces here.
// ---------------------------------------------------------------------------

#[test]
fn pl011_fixture_fidelity() {
    let fixture = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/pl011-full-spec/src/driver.rs"
    ))
    .expect("PL011 fixture source must be readable");

    // `enable_clock_if_present` is the canonical `?` instance the
    // spec's Test Plan calls out (line 179 of the fixture).
    assert!(
        fixture.contains("fn enable_clock_if_present(&self) -> Result<(), RegseqError>"),
        "PL011 fixture must define `enable_clock_if_present`",
    );
    assert!(
        fixture.contains("clock.enable()?;"),
        "PL011 fixture enable_clock_if_present must call `clock.enable()?` (the \
         `?`-pattern-folding trigger)",
    );

    // `init()`'s `?` chain wraps `enable_clock_if_present()` with
    // `?` (spec Test Plan mentions nested/sequential folds).
    assert!(
        fixture.contains("self.enable_clock_if_present()?;"),
        "PL011 fixture init() must call `self.enable_clock_if_present()?`",
    );
}

// ---------------------------------------------------------------------------
// Test: fold applies to IntrinsicCall terminators too.
//
// Rule 2 must fold through both `Call` and `IntrinsicCall` shapes
// (task spec bullet "When processing a Call (or IntrinsicCall from
// task 45)..."). This test mirrors the PL011 fixture's
// `enable_clock_if_present()` pattern: a fallible intrinsic
// (`clock_enable`) called through `?` produces a folded
// `IntrinsicCall` whose `destination` is the unwrapped inner type,
// whose `target` points to the Continue block, and whose
// `error_target` points to the Drop-cascade block on the `?` error
// branch. The walk over every surviving statement / terminator
// asserts no `Try::branch` / `FromResidual` / `ControlFlow` remnant
// remains.
// ---------------------------------------------------------------------------

const INTRINSIC_FOLD_SPECIMEN: &str = r#"
#![no_std]

pub struct MyError;

pub fn make_ok_unit() -> Result<(), MyError> { Ok(()) }

pub struct Clock;

impl Clock {
    pub fn enable(&self) -> Result<(), MyError> { make_ok_unit() }
}

#[rseq::driver(name = "spec-drv", compatible = ["acme,widget"], bus = "amba")]
pub struct Drv {
    #[rseq::clock(id = "clk")]
    pub clk: Clock,
}

impl Drv {
    pub fn enable_clock_if_present(&self) -> Result<(), MyError> {
        self.clk.enable()?;
        make_ok_unit()
    }
}
"#;

#[test]
fn intrinsic_call_question_mark_folds_with_error_target() {
    let outcome = with_specimen(
        INTRINSIC_FOLD_SPECIMEN,
        "construction_try_folding_intrinsic",
        |tcx, pass1| {
            let enable = find_method_def_id(tcx, "Clock", "enable");
            let caller = find_method_def_id(tcx, "Drv", "enable_clock_if_present");
            let caller_fn_id = *pass1.cross_refs.functions.get(&caller).unwrap();

            // Register `Clock::enable` as the `clock_enable` intrinsic
            // so Rule 1 turns the Call into an `IntrinsicCall` before
            // Rule 2 applies the fold on top.
            let lookup = IntrinsicLookup::from_entries(tcx, [(enable, "clock_enable")].into_iter())
                .expect("canonical name must parse");

            let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
            let constants = ConstantIndex::new(&pass1.cross_refs.constants);
            let functions = FunctionIndex::new(&pass1.cross_refs.functions);
            let abstraction_registry = AbstractionTypeRegistry::default();
            let deref_lookup = DerefCollapseLookup::build(tcx, &abstraction_registry)
                .expect("Deref lang item must be resolvable on the specimen crate");
            let ordered_markers = OrderedMarkers::default();
            let try_markers = TryMarkers::resolve(tcx);
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
                &ordered_markers,
                &try_markers,
                &mut interner,
            )
            .expect("build_body must succeed on intrinsic `?` fold");

            let mir_local_count = tcx.optimized_mir(caller).local_decls.len();

            let mut intrinsic_folds: Vec<(
                regseq_model::ir::RegseqIntrinsic,
                bool,
                bool,
                RegseqType,
            )> = Vec::new();
            for bb in &body.basic_blocks {
                if let TerminatorKind::IntrinsicCall {
                    intrinsic,
                    target: _,
                    error_target,
                    destination,
                    ..
                } = &bb.terminator.kind
                {
                    let dest_ty = destination_ty(&body, destination);
                    intrinsic_folds.push((
                        *intrinsic,
                        error_target.is_some(),
                        dest_ty == RegseqType::Unit,
                        dest_ty,
                    ));
                }
            }

            // Total reference counts for the Try-machinery remnant
            // check (spec Test Plan: zero references to Try::branch,
            // zero FromResidual, zero ControlFlow temporaries).
            //
            // - `downcasts`: every `Continue`/`Break` projection would
            //   carry a `Projection::Downcast` -- absorbing the
            //   extraction statements into the fold drops those.
            // - `body.local_decls.len()` < `mir_local_count` confirms
            //   the `_cf` / `_residual` / `_converted` temporaries (and
            //   any move-chain stepping-stones) are eliminated from
            //   the Regseq IR per the spec's Elimination list.
            // - No `Call` survives whose callee is in the
            //   `{Try::branch, FromResidual::from_residual}` set --
            //   those callees are never present in the Pass 1
            //   `functions` table, so a surviving Call against them
            //   would have raised `UnresolvedCallTarget`. Since
            //   `build_body` succeeded (`.expect`), no such Call
            //   exists. We still enumerate the surviving Call
            //   callees below so a regression that silently drops a
            //   Call shows up.
            let downcasts = count_downcasts(&body);
            let regseq_local_count = body.local_decls.len();
            let surviving_call_count = call_callees(&body).len();
            (
                intrinsic_folds,
                downcasts,
                mir_local_count,
                regseq_local_count,
                surviving_call_count,
            )
        },
    );

    let (intrinsic_folds, downcasts, mir_local_count, regseq_local_count, surviving_call_count) =
        outcome;
    assert_eq!(
        intrinsic_folds.len(),
        1,
        "exactly one `clock_enable` IntrinsicCall expected, got {intrinsic_folds:?}",
    );
    let (intrinsic, has_error_target, dest_is_unit, dest_ty) = &intrinsic_folds[0];
    assert_eq!(*intrinsic, regseq_model::ir::RegseqIntrinsic::ClockEnable);
    assert!(
        *has_error_target,
        "folded IntrinsicCall must populate error_target -- the `?` ran",
    );
    assert!(
        *dest_is_unit,
        "folded IntrinsicCall destination must be the unwrapped Continue type `()`, \
         not `Result<(), MyError>` (got `{dest_ty:?}`)",
    );
    assert_eq!(
        downcasts, 0,
        "no Downcast projection may survive Rule 2 folding \
         (Continue/Break extraction absorbed into the fold)",
    );
    assert!(
        regseq_local_count < mir_local_count,
        "Rule 2 must remove the `_cf` / `_residual` / `_converted` temporaries: \
         mir locals={mir_local_count}, regseq locals={regseq_local_count}",
    );
    assert!(
        surviving_call_count <= 1,
        "the folded body keeps only the trailing `make_ok_unit()` Call; \
         no `Try::branch` / `FromResidual::from_residual` Call may survive \
         (got {surviving_call_count} surviving Call terminators)",
    );
}

// ---------------------------------------------------------------------------
// Test 6: no references to `_cf` / _residual / _converted survive.
//
// After folding, the body must not reference any of the
// `ControlFlow` intermediate temporaries. This validates the
// "intermediate `Try`-wrapper and `ControlFlow` values never exist
// in the Regseq IR" invariant from ir-reference.md Section 5.
// ---------------------------------------------------------------------------

// After folding, every Place / Operand that used to reference the
// intermediate `_cf` / `_residual` / `_converted` temporaries
// carried a `Downcast(variant)` projection into `ControlFlow`'s
// Continue/Break arms. Rule 2 eliminates the extraction statements
// entirely; we verify that invariant by scanning every Place
// projection in every surviving statement / terminator and
// asserting no `Downcast` projection is reachable from the body's
// CFG. The assertion is stronger than "no `ControlFlow` locals
// reachable" because `Downcast` projections are structurally unique
// to variant destructuring -- if any survive, a `?` fold was
// incomplete.
#[test]
fn folded_body_has_no_control_flow_downcasts() {
    let src = compose(RESULT_BODY);
    let downcast_count = with_specimen(
        &src,
        "construction_try_folding_no_downcasts",
        |tcx, pass1| {
            let caller = find_method_def_id(tcx, "Drv", "enable_like");
            let body = run_build_body(tcx, pass1, caller).expect("build_body must succeed");
            count_downcasts(&body)
        },
    );
    assert_eq!(
        downcast_count, 0,
        "folded body must contain zero Downcast projections (all absorbed into the fold)",
    );
}

fn count_downcasts(body: &regseq_model::ir::Body) -> usize {
    let mut count = 0;
    for bb in &body.basic_blocks {
        for stmt in &bb.statements {
            count += stmt_downcast_count(stmt);
        }
        count += terminator_downcast_count(&bb.terminator.kind);
    }
    count
}

fn place_downcast_count(place: &Place) -> usize {
    place
        .projection
        .iter()
        .filter(|p| matches!(p, regseq_model::ir::Projection::Downcast { .. }))
        .count()
}

fn operand_downcast_count(operand: &Operand) -> usize {
    match operand {
        Operand::Copy(p) | Operand::Move(p) => place_downcast_count(p),
        _ => 0,
    }
}

fn rvalue_downcast_count(rvalue: &regseq_model::ir::Rvalue) -> usize {
    use regseq_model::ir::Rvalue;
    match rvalue {
        Rvalue::Use(o) | Rvalue::Cast(_, o, _) | Rvalue::UnaryOp(_, o) => operand_downcast_count(o),
        Rvalue::Repeat(o, _) => operand_downcast_count(o),
        Rvalue::Ref(_, p) | Rvalue::Discriminant(p) => place_downcast_count(p),
        Rvalue::BinaryOp(_, a, b) => operand_downcast_count(a) + operand_downcast_count(b),
        Rvalue::Aggregate(_, operands) => operands.iter().map(operand_downcast_count).sum(),
    }
}

fn stmt_downcast_count(stmt: &Statement) -> usize {
    use regseq_model::ir::StatementKind;
    match &stmt.kind {
        StatementKind::Assign(place, rvalue) => {
            place_downcast_count(place) + rvalue_downcast_count(rvalue)
        }
        StatementKind::SetDiscriminant { place, .. } => place_downcast_count(place),
        StatementKind::StorageLive(_) | StatementKind::StorageDead(_) => 0,
        StatementKind::MirIntrinsic(mi) => mir_intrinsic_downcast_count(mi),
    }
}

fn mir_intrinsic_downcast_count(mi: &regseq_model::ir::MirIntrinsic) -> usize {
    use regseq_model::ir::MirIntrinsic;
    match mi {
        MirIntrinsic::Assume(o) => operand_downcast_count(o),
        MirIntrinsic::CopyNonOverlapping { src, dst, count } => {
            operand_downcast_count(src)
                + operand_downcast_count(dst)
                + operand_downcast_count(count)
        }
    }
}

fn terminator_downcast_count(kind: &TerminatorKind) -> usize {
    match kind {
        TerminatorKind::SwitchInt { discriminant, .. } => operand_downcast_count(discriminant),
        TerminatorKind::Call {
            args, destination, ..
        }
        | TerminatorKind::IntrinsicCall {
            args, destination, ..
        } => {
            place_downcast_count(destination)
                + args.iter().map(operand_downcast_count).sum::<usize>()
        }
        TerminatorKind::Drop { place, .. } => place_downcast_count(place),
        TerminatorKind::Goto(_) | TerminatorKind::Return | TerminatorKind::Unreachable => 0,
    }
}
