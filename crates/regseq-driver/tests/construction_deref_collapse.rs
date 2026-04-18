//! Pass 2 Rule 4 deref-collapse tests (regseq-model-48).
//!
//! Recognizes explicit `<Mmio<T> as Deref>::deref` Calls on transport
//! abstraction types (registered through
//! `AbstractionTypeRegistry`) and verifies:
//!
//! 1. The Call is dropped and the destination temporary is spliced
//!    through to the transport receiver Place. Downstream projections
//!    onto the dropped `&Block` temporary gain a synthetic `Deref`
//!    bridge so the resulting chain roots at the receiver's `&Mmio<T>`
//!    Place.
//! 2. `Deref::deref` on a non-transport type (e.g. `Box<u32>`) is left
//!    alone and translates as a regular `Call`.
//! 3. Two consecutive transport-deref Calls on the same receiver are
//!    each collapsed independently, with each destination temporary
//!    re-rooted at its own recorded receiver Place.
//! 4. The lookup mechanism is live-`DefId`-keyed: detection uses the
//!    cached `Deref::deref` `DefId` from `tcx.lang_items()` and the
//!    live `AbstractionTypeRegistry` entries, not path strings.
//!
//! The specimens here declare a local `Mmio<T>` transport struct and
//! implement `core::ops::Deref` on it. The production
//! [`AbstractionTypeRegistry::build`](regseq_driver::extract::abstraction_types::AbstractionTypeRegistry::build)
//! only indexes the `regseq` facade crate, so tests inject a
//! synthetic registry via
//! [`AbstractionTypeRegistry::from_entries`] pointing at the local
//! struct's `DefId`. The translation path exercised under test is
//! identical to the production one; only the registry source differs.

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
    build_body, build_pass1_output, ConstantIndex, DerefCollapseLookup, ExtractionRegistries,
    FunctionIndex, IntrinsicLookup, Pass1Output, PinStateRegistry, TypeDefIndex,
};
use regseq_driver::extract::abstraction_types::AbstractionTypeRegistry;
use regseq_driver::extract::framework_types::FrameworkTypeRegistry;
use regseq_driver::extract::intrinsic_registry::IntrinsicRegistry;
use regseq_model::identity::SymbolInterner;
use regseq_model::ir::{
    BasicBlock, Body, Operand, Place, Projection, Rvalue, StatementKind, TerminatorKind,
};

// ---------------------------------------------------------------------------
// rustc harness
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

/// Locate a local struct item (`pub struct <name> ...`) and return its `DefId`.
fn find_struct_def_id<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> DefId {
    for item_id in tcx
        .hir_module_items(rustc_hir::def_id::LocalModDefId::CRATE_DEF_ID)
        .free_items()
    {
        let item = tcx.hir_item(item_id);
        if let rustc_hir::ItemKind::Struct(ident, _, _) = &item.kind {
            if ident.name.as_str() == name {
                return item.owner_id.def_id.to_def_id();
            }
        }
    }
    panic!("struct `{name}` not found in specimen");
}

// ---------------------------------------------------------------------------
// Specimens
// ---------------------------------------------------------------------------
//
// The transport-deref specimen mirrors the real `regseq` facade's
// shape: a transparent wrapper with `Deref<Target = Regs>` exposing
// the inner block. `call_transport_deref` is the MIR caller used in
// Test 1 -- it explicitly invokes `<Mmio<Regs> as Deref>::deref` and
// reads a field off the result, so Pass 2 must see a `Call` to
// `Deref::deref` followed by a field projection.
//
// The double-deref specimen (`call_double_deref`) fires two
// `<Mmio<Regs> as Deref>::deref` Calls back-to-back on the same
// receiver. Each temporary must be rewritten through its own recorded
// receiver Place.

const TRANSPORT_SPECIMEN: &str = r#"
#![no_std]

use core::ops::Deref;

#[repr(C)]
pub struct Regs {
    pub a: u32,
    pub b: u32,
}

#[repr(transparent)]
pub struct Mmio<T> {
    inner: core::ptr::NonNull<T>,
}

impl<T> Deref for Mmio<T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { self.inner.as_ref() }
    }
}

pub fn call_transport_deref(mmio: &Mmio<Regs>) -> u32 {
    // `Deref::deref` is invoked explicitly via the `*` operator --
    // MIR lowers this to a `Call` to `<Mmio<Regs> as Deref>::deref`
    // at `mir_opt_level = 0`.
    (*mmio).a
}

pub fn call_double_deref(mmio: &Mmio<Regs>) -> u32 {
    // Two `Deref::deref` Calls in one body. Each lands in its own
    // temporary local; each must be collapsed independently, each
    // preserving transport receiver identity through the rewrite.
    // Combine with `|` (primitive bitwise-or) rather than a method
    // call so the body holds no residual `core::num::*` helper Calls
    // that Pass 1 would otherwise leave unresolved through the
    // `FunctionIndex`.
    let x = (*mmio).a;
    let y = (*mmio).b;
    x | y
}

// Pass 1 requires exactly one `#[rseq::driver]` struct per crate. We
// declare a trivial unit-struct so Pass 1 reaches completion; the
// tests themselves never inspect the driver struct.
#[rseq::driver(name = "spec-drv", compatible = ["acme,widget"], bus = "amba")]
pub struct Drv;
"#;

// Non-transport specimen: `Box<u32>` dereferencing via `*`.
// `Box` is a `Deref` implementor but is not a registered transport
// abstraction; the collapse must leave the Call intact. Note: `Box`
// requires `alloc`, which is not available on `#![no_std]` without a
// global allocator. To keep the specimen self-contained, we define a
// minimal `MyBox<T>` that implements `Deref`.

const NON_TRANSPORT_SPECIMEN: &str = r#"
#![no_std]

use core::ops::Deref;

#[repr(transparent)]
pub struct MyBox<T>(core::ptr::NonNull<T>);

impl<T> Deref for MyBox<T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { self.0.as_ref() }
    }
}

pub fn call_mybox_deref(b: &MyBox<u32>) -> u32 {
    *(*b)
}

#[rseq::driver(name = "spec-drv", compatible = ["acme,widget"], bus = "amba")]
pub struct Drv;
"#;

// ---------------------------------------------------------------------------
// Helpers for body inspection
// ---------------------------------------------------------------------------

/// Count transport-`Deref::deref` Calls that would remain in a translated body
/// if collapse did not run. Since the Call's callee is the trait method, the
/// terminator here is always recognized by callee-`DefId` comparison.
///
/// Actually simpler for the test: after `build_body`, count the number of
/// `Call` terminators in the body whose callee `FunctionId` -- in the
/// production path -- would have been `<Mmio as Deref>::deref`. In the
/// Regseq IR layer we don't preserve the source `DefId`, so instead we
/// directly assert: zero regular `Call` terminators remain (for the
/// transport-deref specimen) and every deref dropped translates into a
/// plain `Goto` successor path. The test compares pre/post terminator
/// counts to pin down the collapse effect.
fn count_terminators(body: &Body) -> TerminatorCounts {
    let mut counts = TerminatorCounts::default();
    for bb in &body.basic_blocks {
        match &bb.terminator.kind {
            TerminatorKind::Call { .. } => counts.call += 1,
            TerminatorKind::IntrinsicCall { .. } => counts.intrinsic_call += 1,
            TerminatorKind::Goto(_) => counts.goto += 1,
            TerminatorKind::SwitchInt { .. } => counts.switch_int += 1,
            TerminatorKind::Return => counts.retn += 1,
            TerminatorKind::Unreachable => counts.unreachable += 1,
            TerminatorKind::Drop { .. } => counts.drop += 1,
        }
    }
    counts
}

#[derive(Default, Debug, Clone, Copy)]
struct TerminatorCounts {
    call: usize,
    intrinsic_call: usize,
    goto: usize,
    switch_int: usize,
    retn: usize,
    unreachable: usize,
    drop: usize,
}

/// Check whether any Place reference in `blocks` -- anywhere statements or
/// terminators touch a `Place` -- projects off `local` with a non-empty
/// projection chain. After collapse, downstream uses of a collapsed
/// destination local must not remain rooted at that local.
fn any_place_roots_local(blocks: &[BasicBlock], local: regseq_model::identity::LocalId) -> bool {
    for block in blocks {
        for stmt in &block.statements {
            if statement_roots_local(&stmt.kind, local) {
                return true;
            }
        }
        if terminator_roots_local(&block.terminator.kind, local) {
            return true;
        }
    }
    false
}

fn place_roots_local(p: &Place, local: regseq_model::identity::LocalId) -> bool {
    p.local == local && !p.projection.is_empty()
}

fn operand_roots_local(op: &Operand, local: regseq_model::identity::LocalId) -> bool {
    match op {
        Operand::Copy(p) | Operand::Move(p) => place_roots_local(p, local),
        Operand::Constant(_) | Operand::Literal(_) => false,
    }
}

fn rvalue_roots_local(rv: &Rvalue, local: regseq_model::identity::LocalId) -> bool {
    match rv {
        Rvalue::Use(op)
        | Rvalue::Repeat(op, _)
        | Rvalue::Cast(_, op, _)
        | Rvalue::UnaryOp(_, op) => operand_roots_local(op, local),
        Rvalue::Ref(_, p) | Rvalue::Discriminant(p) => place_roots_local(p, local),
        Rvalue::BinaryOp(_, l, r) => operand_roots_local(l, local) || operand_roots_local(r, local),
        Rvalue::Aggregate(_, ops) => ops.iter().any(|o| operand_roots_local(o, local)),
    }
}

fn statement_roots_local(k: &StatementKind, local: regseq_model::identity::LocalId) -> bool {
    match k {
        StatementKind::Assign(p, rv) => {
            place_roots_local(p, local) || rvalue_roots_local(rv, local)
        }
        StatementKind::SetDiscriminant { place, .. } => place_roots_local(place, local),
        StatementKind::StorageLive(_) | StatementKind::StorageDead(_) => false,
        StatementKind::MirIntrinsic(_) => false,
    }
}

fn terminator_roots_local(k: &TerminatorKind, local: regseq_model::identity::LocalId) -> bool {
    match k {
        TerminatorKind::SwitchInt { discriminant, .. } => operand_roots_local(discriminant, local),
        TerminatorKind::Call {
            args, destination, ..
        }
        | TerminatorKind::IntrinsicCall {
            args, destination, ..
        } => {
            place_roots_local(destination, local)
                || args.iter().any(|a| operand_roots_local(a, local))
        }
        TerminatorKind::Drop { place, .. } => place_roots_local(place, local),
        TerminatorKind::Goto(_) | TerminatorKind::Return | TerminatorKind::Unreachable => false,
    }
}

// ---------------------------------------------------------------------------
// Test 1: transport deref collapse -- Call dropped, place re-rooted
// ---------------------------------------------------------------------------

#[test]
fn transport_deref_call_is_dropped_and_place_reroots_to_receiver() {
    let outcome = with_specimen(
        TRANSPORT_SPECIMEN,
        "construction_deref_collapse_transport",
        |tcx, pass1| {
            let caller = find_free_fn_def_id(tcx, "call_transport_deref");
            let caller_fn_id = *pass1
                .cross_refs
                .functions
                .get(&caller)
                .expect("caller must be registered in Pass 1");

            let mmio_did = find_struct_def_id(tcx, "Mmio");

            let abstraction_registry =
                AbstractionTypeRegistry::from_entries([(mmio_did, "mmio".to_string())]);
            let deref_lookup = DerefCollapseLookup::build(tcx, &abstraction_registry)
                .expect("Deref lang item must be resolvable on the specimen crate");

            // Sanity: the lookup must be live-DefId-keyed (not path
            // strings), and it must see the injected `mmio` entry.
            // `deref_method()` never panics because `build(...)`
            // already succeeded above (it would have returned
            // `Err(NoDerefLangItem)` otherwise).
            let _ = deref_lookup.deref_method();
            assert_eq!(
                deref_lookup.transport_abstraction_count(),
                1,
                "exactly one transport abstraction injected into the registry",
            );

            let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
            let constants = ConstantIndex::new(&pass1.cross_refs.constants);
            let functions = FunctionIndex::new(&pass1.cross_refs.functions);
            let intrinsics = IntrinsicLookup::default();
            let mut interner = SymbolInterner::new();
            let body = build_body(
                tcx,
                caller,
                caller_fn_id,
                &type_def_index,
                &constants,
                &functions,
                &intrinsics,
                &deref_lookup,
                &mut interner,
            )
            .expect("build_body must succeed on the caller function");

            (
                body.clone(),
                body.basic_blocks.clone(),
                body.local_decls.len(),
            )
        },
    );

    let (body, blocks, local_count) = outcome;
    let counts = count_terminators(&body);

    // Zero regular `Call` terminators remain: the sole Call in the source
    // was `<Mmio<Regs> as Deref>::deref`, which the collapse replaced
    // with a `Goto`.
    assert_eq!(
        counts.call, 0,
        "transport deref Call must be dropped; body terminators: {counts:?}",
    );
    // At least one `Goto` was synthesized by the collapse rewrite (the
    // original Call's return target now reached unconditionally).
    assert!(
        counts.goto >= 1,
        "collapse must emit at least one synthetic Goto; got {counts:?}",
    );
    // The body must reach a Return -- the original caller returns `u32`.
    assert!(
        counts.retn >= 1,
        "body must still terminate in Return; got {counts:?}",
    );

    // None of the locals should serve as a destination-root for any
    // remaining Place post-rewrite: `body.basic_blocks` must not
    // contain any projected Place rooted at a would-be collapsed
    // destination. We check this by scanning each local: for each
    // local id, if any terminator still assigned that local *and* the
    // local is referenced with a projection downstream, the rewrite
    // failed. The concrete guarantee we test: the reads after collapse
    // root at a local whose MIR type was `&Mmio<Regs>` (the receiver),
    // not at the dropped `&Regs` temporary. We approximate this by
    // checking that at least one Place in the body has a projection
    // chain of length >= 2 starting with `Deref, Deref, Field`
    // (receiver's built-in Deref for `&Mmio<Regs>` -> Mmio<Regs>,
    // bridge Deref for the transparent boundary, then field access).
    let has_rewritten_chain = blocks.iter().any(|b| {
        b.statements.iter().any(|s| match &s.kind {
            StatementKind::Assign(_, Rvalue::Use(Operand::Copy(p)))
            | StatementKind::Assign(_, Rvalue::Use(Operand::Move(p))) => {
                p.projection.len() >= 3
                    && matches!(p.projection[0], Projection::Deref)
                    && matches!(p.projection[1], Projection::Deref)
                    && matches!(p.projection[2], Projection::Field { .. })
            }
            _ => false,
        })
    });
    assert!(
        has_rewritten_chain,
        "at least one Place must show the [Deref, Deref, Field] rewrite chain; blocks={blocks:#?}",
    );

    // Sanity: locals count should be >= 2 (return place + at least one
    // arg) -- the collapse does not drop MIR locals.
    assert!(local_count >= 2);
}

// ---------------------------------------------------------------------------
// Test 2: non-transport Deref::deref is preserved as a regular Call
// ---------------------------------------------------------------------------

#[test]
fn non_transport_deref_is_preserved_as_regular_call() {
    let outcome = with_specimen(
        NON_TRANSPORT_SPECIMEN,
        "construction_deref_collapse_non_transport",
        |tcx, pass1| {
            let caller = find_free_fn_def_id(tcx, "call_mybox_deref");
            let caller_fn_id = *pass1
                .cross_refs
                .functions
                .get(&caller)
                .expect("caller must be registered in Pass 1");

            // Empty abstraction registry: `MyBox` is NOT registered
            // with any transport abstraction. The collapse must leave
            // the `<MyBox as Deref>::deref` Call alone.
            let abstraction_registry = AbstractionTypeRegistry::from_entries(core::iter::empty());
            let deref_lookup = DerefCollapseLookup::build(tcx, &abstraction_registry)
                .expect("Deref lang item must be resolvable on the specimen crate");
            assert_eq!(deref_lookup.transport_abstraction_count(), 0);

            let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
            let constants = ConstantIndex::new(&pass1.cross_refs.constants);
            let functions = FunctionIndex::new(&pass1.cross_refs.functions);
            let intrinsics = IntrinsicLookup::default();
            let mut interner = SymbolInterner::new();
            // The non-transport caller translates to a regular `Call`
            // for the `Deref::deref` invocation. Since `Deref::deref`'s
            // DefId is typically *not* in Pass 1's FunctionIndex
            // (FunctionIndex only tracks monomorphized local/reachable
            // functions known to Pass 1), the body build may surface a
            // `UnresolvedCallTarget` error. That is itself a
            // pass-through signal -- it means collapse did NOT eat the
            // Call, which is the property we want to verify. We catch
            // either outcome and route to the correct assertion.
            let result = build_body(
                tcx,
                caller,
                caller_fn_id,
                &type_def_index,
                &constants,
                &functions,
                &intrinsics,
                &deref_lookup,
                &mut interner,
            );
            match result {
                Ok(body) => ResultShape::Body(body),
                Err(e) => ResultShape::Error(format!("{e:?}")),
            }
        },
    );

    match outcome {
        ResultShape::Body(body) => {
            let counts = count_terminators(&body);
            // At least one `Call` must survive -- the non-transport
            // Deref::deref Call was not collapsed.
            assert!(
                counts.call >= 1,
                "non-transport Deref::deref must remain as a regular Call; got {counts:?}",
            );
        }
        ResultShape::Error(msg) => {
            // Acceptable: the Call survived to terminator translation,
            // which then could not resolve its callee through the
            // FunctionIndex because Pass 1 only tracks local items.
            // The important guarantee is that the collapse did NOT
            // drop the Call -- which is exactly what surfacing
            // `UnresolvedCallTarget` at `Deref::deref` implies.
            assert!(
                msg.contains("UnresolvedCallTarget") || msg.contains("Deref"),
                "expected UnresolvedCallTarget-shaped failure post-collapse, got {msg}",
            );
        }
    }
}

#[derive(Debug)]
enum ResultShape {
    Body(Body),
    Error(String),
}

// ---------------------------------------------------------------------------
// Test 3: two consecutive transport-deref Calls are collapsed independently
// ---------------------------------------------------------------------------

#[test]
fn two_consecutive_transport_derefs_collapse_independently() {
    let outcome = with_specimen(
        TRANSPORT_SPECIMEN,
        "construction_deref_collapse_double",
        |tcx, pass1| {
            let caller = find_free_fn_def_id(tcx, "call_double_deref");
            let caller_fn_id = *pass1
                .cross_refs
                .functions
                .get(&caller)
                .expect("caller must be registered in Pass 1");

            let mmio_did = find_struct_def_id(tcx, "Mmio");
            let abstraction_registry =
                AbstractionTypeRegistry::from_entries([(mmio_did, "mmio".to_string())]);
            let deref_lookup = DerefCollapseLookup::build(tcx, &abstraction_registry)
                .expect("Deref lang item must be resolvable on the specimen crate");

            let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
            let constants = ConstantIndex::new(&pass1.cross_refs.constants);
            let functions = FunctionIndex::new(&pass1.cross_refs.functions);
            let intrinsics = IntrinsicLookup::default();
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
                &mut interner,
            )
            .expect("build_body must succeed on the double-deref caller")
        },
    );

    let counts = count_terminators(&outcome);

    // Both Calls must be dropped; zero remaining Call terminators.
    assert_eq!(
        counts.call, 0,
        "both transport deref Calls must be dropped; got {counts:?}",
    );

    // Each collapsed Call produces a synthetic Goto replacing its
    // terminator -- we expect at least two Gotos from the collapses
    // themselves (plus whatever MIR emitted natively for the rest of
    // the body).
    assert!(
        counts.goto >= 2,
        "each transport deref Call must yield a synthetic Goto; got {counts:?}",
    );

    // At least two distinct `[Deref, Deref, Field]` chains must appear
    // in the body -- one per collapsed Call.
    let rewritten_chains: usize = outcome
        .basic_blocks
        .iter()
        .flat_map(|b| b.statements.iter())
        .filter(|s| match &s.kind {
            StatementKind::Assign(_, Rvalue::Use(Operand::Copy(p)))
            | StatementKind::Assign(_, Rvalue::Use(Operand::Move(p))) => {
                p.projection.len() >= 3
                    && matches!(p.projection[0], Projection::Deref)
                    && matches!(p.projection[1], Projection::Deref)
                    && matches!(p.projection[2], Projection::Field { .. })
            }
            _ => false,
        })
        .count();
    assert!(
        rewritten_chains >= 2,
        "double-deref body must have at least two rewritten field-access chains; got {rewritten_chains}",
    );
}

// ---------------------------------------------------------------------------
// Test 4: detection is live-DefId-keyed, not path-string-based.
// ---------------------------------------------------------------------------
//
// Builds a transport-deref specimen but injects a transport registry
// that does NOT include the Mmio struct's DefId. The collapse must
// not fire, which (identically to Test 2) surfaces as either a
// surviving Call or an UnresolvedCallTarget error. This proves the
// gate is the registry lookup -- removing the DefId entry disables
// the rule cleanly, regardless of the ADT's name.

#[test]
fn collapse_is_gated_by_registry_entry_not_by_type_name() {
    let outcome = with_specimen(
        TRANSPORT_SPECIMEN,
        "construction_deref_collapse_registry_gate",
        |tcx, pass1| {
            let caller = find_free_fn_def_id(tcx, "call_transport_deref");
            let caller_fn_id = *pass1
                .cross_refs
                .functions
                .get(&caller)
                .expect("caller must be registered in Pass 1");

            // Intentionally empty: no registered abstraction types.
            let abstraction_registry = AbstractionTypeRegistry::from_entries(core::iter::empty());
            let deref_lookup = DerefCollapseLookup::build(tcx, &abstraction_registry)
                .expect("Deref lang item must be resolvable on the specimen crate");
            assert_eq!(
                deref_lookup.transport_abstraction_count(),
                0,
                "registry injected empty, so no transports are visible to the lookup",
            );

            let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
            let constants = ConstantIndex::new(&pass1.cross_refs.constants);
            let functions = FunctionIndex::new(&pass1.cross_refs.functions);
            let intrinsics = IntrinsicLookup::default();
            let mut interner = SymbolInterner::new();
            let result = build_body(
                tcx,
                caller,
                caller_fn_id,
                &type_def_index,
                &constants,
                &functions,
                &intrinsics,
                &deref_lookup,
                &mut interner,
            );
            match result {
                Ok(_) => "ok".to_string(),
                Err(e) => format!("{e:?}"),
            }
        },
    );

    // The collapse was gated off by the empty registry; the Call
    // survived to regular Call translation, which fails because
    // Deref::deref is not in the FunctionIndex. The message must
    // mention UnresolvedCallTarget OR Deref, proving the Call was
    // never eaten.
    assert!(
        outcome.contains("UnresolvedCallTarget") || outcome.contains("Deref"),
        "empty registry must leave the Deref Call intact, got outcome `{outcome}`",
    );
}

// ---------------------------------------------------------------------------
// Test 5: cross-cutting -- constructed transport-deref body has zero
// Call terminators targeting Deref::deref (negative assertion).
// ---------------------------------------------------------------------------
//
// The spec requires a "cross-cutting assertion on the constructed
// PL011 model" that zero Call terminators target
// `<Mmio<T> as Deref>::deref` anywhere in the module. The PL011
// end-to-end harness is not yet stood up in this phase (task 54),
// so we approximate the guarantee on the specimen's multi-body
// module: every function translated under the registered transport
// abstraction has zero Call terminators that could target
// Deref::deref (because the collapse rewrites them into Gotos).

#[test]
fn constructed_module_has_zero_residual_deref_calls() {
    let outcome = with_specimen(
        TRANSPORT_SPECIMEN,
        "construction_deref_collapse_module_wide",
        |tcx, pass1| {
            let mmio_did = find_struct_def_id(tcx, "Mmio");
            let abstraction_registry =
                AbstractionTypeRegistry::from_entries([(mmio_did, "mmio".to_string())]);
            let deref_lookup = DerefCollapseLookup::build(tcx, &abstraction_registry)
                .expect("Deref lang item must be resolvable on the specimen crate");
            let deref_method_str = tcx.def_path_str(deref_lookup.deref_method());

            let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
            let constants = ConstantIndex::new(&pass1.cross_refs.constants);
            let functions = FunctionIndex::new(&pass1.cross_refs.functions);
            let intrinsics = IntrinsicLookup::default();
            let mut interner = SymbolInterner::new();

            // Enumerate every function-bearing item in the specimen's
            // crate root -- free fns AND impl methods (including the
            // `impl<T> Deref for Mmio<T> { fn deref(...) }` block
            // itself). The cross-cutting invariant is:
            //
            // For every body whose MIR we can translate: zero
            // surviving `TerminatorKind::Call` terminators target
            // `Deref::deref`. Bodies that call items outside Pass 1's
            // `FunctionIndex` (e.g. `core::ptr::NonNull::as_ref` in
            // the Mmio deref impl) surface
            // `ConstructionError::UnresolvedCallTarget`; the
            // assertion then tightens to: that error's rendered
            // callee must NOT be `Deref::deref`, because that would
            // mean the collapse missed a transport deref somewhere.
            let mut target_items: Vec<DefId> = Vec::new();
            for item_id in tcx
                .hir_module_items(rustc_hir::def_id::LocalModDefId::CRATE_DEF_ID)
                .free_items()
            {
                let item = tcx.hir_item(item_id);
                match &item.kind {
                    rustc_hir::ItemKind::Fn { .. } => {
                        target_items.push(item.owner_id.def_id.to_def_id());
                    }
                    rustc_hir::ItemKind::Impl(impl_block) => {
                        for ii in impl_block.items {
                            let impl_item = tcx.hir_impl_item(*ii);
                            if matches!(impl_item.kind, rustc_hir::ImplItemKind::Fn(..)) {
                                target_items.push(impl_item.owner_id.def_id.to_def_id());
                            }
                        }
                    }
                    _ => {}
                }
            }
            assert!(
                target_items.len() >= 3,
                "specimen must expose at least 2 free fn callers + 1 impl deref method; got {}",
                target_items.len(),
            );

            let mut residual_calls: usize = 0;
            let mut surviving_deref_refs: Vec<String> = Vec::new();
            let mut translated_bodies: usize = 0;
            let mut skipped_bodies: usize = 0;
            for def in &target_items {
                let Some(fn_id) = pass1.cross_refs.functions.get(def).copied() else {
                    continue;
                };
                let result = build_body(
                    tcx,
                    *def,
                    fn_id,
                    &type_def_index,
                    &constants,
                    &functions,
                    &intrinsics,
                    &deref_lookup,
                    &mut interner,
                );
                match result {
                    Ok(body) => {
                        translated_bodies += 1;
                        residual_calls += count_terminators(&body).call;
                    }
                    Err(e) => {
                        skipped_bodies += 1;
                        let rendered = format!("{e:?}");
                        if rendered.contains(&deref_method_str) {
                            surviving_deref_refs.push(rendered);
                        }
                    }
                }
            }
            (
                residual_calls,
                surviving_deref_refs,
                target_items.len(),
                translated_bodies,
                skipped_bodies,
            )
        },
    );

    let (residual_calls, surviving_deref_refs, fn_count, translated, skipped) = outcome;
    assert!(
        fn_count >= 3,
        "module must expose the two deref callers plus the `Deref` impl's `deref` method; got {fn_count}",
    );
    assert!(
        translated + skipped == fn_count,
        "every enumerated item was accounted for (translated or skipped): \
         translated={translated}, skipped={skipped}, total={fn_count}",
    );
    assert_eq!(
        residual_calls, 0,
        "module-wide ({translated} bodies translated, {skipped} skipped): zero residual Call \
         terminators remain after Rule 4 collapse",
    );
    assert!(
        surviving_deref_refs.is_empty(),
        "no UnresolvedCallTarget error should mention `Deref::deref` -- any such occurrence \
         indicates the collapse missed a transport deref. Offenders: {surviving_deref_refs:?}",
    );
}

// ---------------------------------------------------------------------------
// Sanity: unused helper to keep the `any_place_roots_local` + Drop
// analyzer live even if the mainline tests skip them.
// ---------------------------------------------------------------------------

#[test]
fn helper_any_place_roots_local_agrees_with_trivial_body() {
    use regseq_model::identity::{BasicBlockId, LocalId};
    use regseq_model::ir::{BasicBlock, Terminator};
    let blocks = vec![BasicBlock {
        statements: vec![],
        terminator: Terminator {
            kind: TerminatorKind::Goto(BasicBlockId(0)),
        },
    }];
    assert!(!any_place_roots_local(&blocks, LocalId(0)));
}
