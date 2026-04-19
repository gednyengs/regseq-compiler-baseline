//! Pass 2 Rule 5 OrderedRegion detection tests (regseq-model-49).
//!
//! These tests exercise the IR constructor's ability to recognize
//! paired `regseq::internal::ordered_region_begin()` /
//! `ordered_region_end()` marker Calls, drop them from the translated
//! CFG, and record each matched pair as an `OrderedRegion { entry,
//! exit }` on `Body.ordered_regions`. They also cover the two
//! pairing error paths (`UnpairedOrderedMarker` for begin/end
//! imbalance) and the facade-lookup error path
//! (`OrderedMarkerLookupFailed`) when the `regseq` crate is loaded
//! but the marker functions cannot be resolved.
//!
//! The production facade resolution path
//! ([`OrderedMarkers::resolve`]) walks the `regseq` facade crate's
//! `internal` module by `DefId`, not by hard-coded path strings. The
//! specimens here don't depend on `regseq` (which would require
//! building the full facade in-process); they instead declare
//! synthetic marker functions locally and wire them into
//! [`OrderedMarkers`] via [`OrderedMarkers::from_def_ids`]. The
//! translation path exercised under test is identical to production;
//! only the marker-discovery step is bypassed.
//!
//! A dedicated test (`resolve_without_regseq_facade_yields_no_markers`)
//! drives the production [`OrderedMarkers::resolve`] against a
//! specimen that does not depend on `regseq` to confirm the
//! no-markers-present path returns an empty `OrderedMarkers` rather
//! than hard-erroring.

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
    ExtractionRegistries, FunctionIndex, IntrinsicLookup, LogArgUnwrap, OrderedMarkers,
    Pass1Output, PinStateRegistry, RegisterIndex, TryMarkers, TypeDefIndex,
};
use regseq_driver::extract::abstraction_types::AbstractionTypeRegistry;
use regseq_driver::extract::framework_types::FrameworkTypeRegistry;
use regseq_driver::extract::intrinsic_registry::IntrinsicRegistry;
use regseq_model::identity::{BasicBlockId, SymbolInterner};
use regseq_model::ir::{Body, TerminatorKind};

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
// SESE reachability check
// ---------------------------------------------------------------------------

/// Collect every block reachable from `entry` via terminator edges
/// without passing through `stop`. The `stop` block itself is
/// excluded from the reachable set (the region is
/// reachability-up-to-exit, not reachability-through-exit).
///
/// Matches the definition in `ir-reference.md` Section 6: a block
/// belongs to an `OrderedRegion { entry, exit }` iff it is reachable
/// from `entry` via any terminator edge path that does not traverse
/// `exit`.
fn reachable_without_crossing(body: &Body, entry: BasicBlockId, stop: BasicBlockId) -> Vec<u32> {
    use std::collections::BTreeSet;
    let mut seen: BTreeSet<u32> = BTreeSet::new();
    let mut stack = vec![entry];
    while let Some(bb) = stack.pop() {
        if bb == stop || !seen.insert(bb.0) {
            continue;
        }
        let idx = bb.0 as usize;
        if idx >= body.basic_blocks.len() {
            continue;
        }
        match &body.basic_blocks[idx].terminator.kind {
            TerminatorKind::Goto(t) => stack.push(*t),
            TerminatorKind::SwitchInt { targets, .. } => {
                for t in &targets.targets {
                    stack.push(*t);
                }
            }
            TerminatorKind::Call {
                target,
                error_target,
                ..
            } => {
                if let Some(t) = target {
                    stack.push(*t);
                }
                if let Some(e) = error_target {
                    stack.push(*e);
                }
            }
            TerminatorKind::IntrinsicCall {
                target,
                error_target,
                ..
            } => {
                stack.push(*target);
                if let Some(e) = error_target {
                    stack.push(*e);
                }
            }
            TerminatorKind::Drop { target, .. } => stack.push(*target),
            TerminatorKind::Return | TerminatorKind::Unreachable => {}
        }
    }
    let mut out: Vec<u32> = seen.into_iter().collect();
    out.sort_unstable();
    out
}

// ---------------------------------------------------------------------------
// Shared specimen preamble: minimal #[rseq::driver] struct + the
// synthetic marker functions that stand in for
// `regseq::internal::ordered_region_begin/end` in these inline tests.
// ---------------------------------------------------------------------------

const PREAMBLE: &str = r#"
#![no_std]

#[inline(never)]
pub fn ordered_region_begin() {}
#[inline(never)]
pub fn ordered_region_end() {}

pub fn work_a() {}
pub fn work_b() {}
pub fn work_c() {}
pub fn work_d() {}

#[rseq::driver(name = "spec-drv", compatible = ["acme,widget"], bus = "amba")]
pub struct Drv;
"#;

fn compose(body: &str) -> String {
    format!("{PREAMBLE}{body}")
}

// ---------------------------------------------------------------------------
// Test 1: pl011-style program_polling_port -- one ordered region
// ---------------------------------------------------------------------------
//
// Structurally mirrors `program_polling_port()` in the PL011 fixture
// (`tests/fixtures/pl011-full-spec/src/driver.rs` line 156): two
// straight-line helper calls outside the `ordered!` block, then a
// sequence of helper calls inside. The `ordered!` macro in the
// fixture expands to `ordered_region_begin() ; ...block... ;
// ordered_region_end()` (see `crates/regseq-macros/src/ordered.rs`),
// which is exactly the pattern this specimen reproduces via inline
// `ordered_region_begin()` / `ordered_region_end()` calls. The
// `pl011_fixture_fidelity` test below asserts the fixture still has
// this shape, so a deviation in either the fixture or the specimen
// will surface here.
//
// After translation the body must:
// - Contain zero Call terminators whose callee resolves to either
//   marker function.
// - Record exactly one `OrderedRegion { entry, exit }`.
// - Have `entry` equal to the block that immediately follows the
//   dropped begin marker (the former begin-block's Goto target) and
//   `exit` equal to the block that immediately follows the dropped
//   end marker.
// - Satisfy the SESE invariant: the region's member set (reachable
//   from `entry` without crossing `exit`) contains every interior
//   block (the blocks that carried `work_c`/`work_d` calls and the
//   end-marker's former owning block) and excludes every exterior
//   block (the blocks that carried `work_a`/`work_b` and the `exit`
//   block itself).

// Specimen mirrors the literal shape produced by `ordered!`'s
// expansion (see `crates/regseq-macros/src/ordered.rs`): a
// `ordered_region_begin()` call, a `let __regseq_ordered_result = {
// ... };` binding that holds the user block, then a
// `ordered_region_end()` call, then the `__regseq_ordered_result`
// identifier. The two prefix calls (`work_a()` / `work_b()`)
// reproduce the two statements `self.disable_uart();
// self.disable_fifo();` that precede `ordered!` in the PL011 fixture
// (line 157-158). `work_c()` / `work_d()` stand in for the bracketed
// sequence of `self.set_baud_rate(...)`, `configure_line_control()`,
// etc. inside `ordered!`.
const PROGRAM_POLLING_PORT_BODY: &str = r#"
impl Drv {
    pub fn program_polling_port(&mut self) {
        work_a();
        work_b();
        ordered_region_begin();
        let __regseq_ordered_result = {
            work_c();
            work_d();
        };
        ordered_region_end();
        __regseq_ordered_result
    }
}
"#;

#[test]
fn program_polling_port_records_single_ordered_region() {
    let src = compose(PROGRAM_POLLING_PORT_BODY);
    let outcome = with_specimen(
        &src,
        "construction_ordered_region_program_polling_port",
        |tcx, pass1| {
            let begin = find_free_fn_def_id(tcx, "ordered_region_begin");
            let end = find_free_fn_def_id(tcx, "ordered_region_end");
            let ordered_markers = OrderedMarkers::from_def_ids(begin, end);
            let try_markers = TryMarkers::default();
            let log_arg = LogArgUnwrap::default();
            let caller = find_method_def_id(tcx, "Drv", "program_polling_port");
            let caller_fn_id = *pass1
                .cross_refs
                .functions
                .get(&caller)
                .expect("caller must be registered in Pass 1");

            // Pre-translation introspection: walk the MIR to find
            // the block that holds the begin-marker Call and the
            // block that holds the end-marker Call, and derive the
            // expected `entry` / `exit` (the target of each marker
            // Call). This gives us the "expected" region bounds
            // independent of the translation pipeline, so the
            // assertion verifies the translation wiring rather than
            // just internal consistency of the produced regions.
            let mir = tcx.optimized_mir(caller);
            let mut cleanup_count = 0usize;
            let mut begin_site: Option<(u32, u32)> = None;
            let mut end_site: Option<(u32, u32)> = None;
            for (mir_bb, data) in mir.basic_blocks.iter_enumerated() {
                if data.is_cleanup {
                    cleanup_count += 1;
                }
                let term = data
                    .terminator
                    .as_ref()
                    .expect("every block has a terminator");
                if let rustc_middle::mir::TerminatorKind::Call { func, target, .. } = &term.kind {
                    if let rustc_middle::mir::Operand::Constant(c) = func {
                        if let rustc_middle::mir::Const::Val(_, ty) = c.const_ {
                            if let rustc_middle::ty::TyKind::FnDef(def_id, _) = ty.kind() {
                                let target_idx = target
                                    .expect("marker Calls must have a return target")
                                    .as_u32();
                                if *def_id == begin {
                                    begin_site = Some((mir_bb.as_u32(), target_idx));
                                } else if *def_id == end {
                                    end_site = Some((mir_bb.as_u32(), target_idx));
                                }
                            }
                        }
                    }
                }
            }
            let begin_site = begin_site.expect("MIR must contain a begin marker Call");
            let end_site = end_site.expect("MIR must contain an end marker Call");

            let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
            let constants = ConstantIndex::new(&pass1.cross_refs.constants);
            let functions = FunctionIndex::new(&pass1.cross_refs.functions);
            let intrinsics = IntrinsicLookup::default();
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
                &intrinsics,
                &deref_lookup,
                &ordered_markers,
                &try_markers,
                &log_arg,
                &RegisterIndex::default(),
                &std::collections::HashMap::new(),
                &AbstractionTypeRegistry::default(),
                &mut interner,
            )
            .expect("build_body must succeed on program_polling_port");

            // Classify every block by the callee of its Call
            // terminator (or "non-call" for Goto/Return blocks). The
            // test asserts block-level membership in the region is
            // consistent with the source-level bracketing and that
            // every block is covered by the partition
            // (entry/exit/outside/inside/etc.).
            let begin_fn_id = pass1.cross_refs.functions.get(&begin).copied();
            let end_fn_id = pass1.cross_refs.functions.get(&end).copied();
            let work_a = find_free_fn_def_id(tcx, "work_a");
            let work_b = find_free_fn_def_id(tcx, "work_b");
            let work_c = find_free_fn_def_id(tcx, "work_c");
            let work_d = find_free_fn_def_id(tcx, "work_d");
            let work_a_fn_id = *pass1.cross_refs.functions.get(&work_a).unwrap();
            let work_b_fn_id = *pass1.cross_refs.functions.get(&work_b).unwrap();
            let work_c_fn_id = *pass1.cross_refs.functions.get(&work_c).unwrap();
            let work_d_fn_id = *pass1.cross_refs.functions.get(&work_d).unwrap();

            let mut call_blocks: Vec<(u32, &'static str)> = Vec::new();
            let mut marker_call_count = 0usize;
            for (idx, bb) in body.basic_blocks.iter().enumerate() {
                if let TerminatorKind::Call { callee, .. } = &bb.terminator.kind {
                    if Some(*callee) == begin_fn_id || Some(*callee) == end_fn_id {
                        marker_call_count += 1;
                        call_blocks.push((idx as u32, "marker"));
                    } else if *callee == work_a_fn_id {
                        call_blocks.push((idx as u32, "work_a"));
                    } else if *callee == work_b_fn_id {
                        call_blocks.push((idx as u32, "work_b"));
                    } else if *callee == work_c_fn_id {
                        call_blocks.push((idx as u32, "work_c"));
                    } else if *callee == work_d_fn_id {
                        call_blocks.push((idx as u32, "work_d"));
                    } else {
                        call_blocks.push((idx as u32, "other"));
                    }
                }
            }

            let total_blocks = body.basic_blocks.len();
            let mir_block_count = mir.basic_blocks.len();
            (
                body,
                marker_call_count,
                call_blocks,
                begin_site,
                end_site,
                cleanup_count,
                total_blocks,
                mir_block_count,
            )
        },
    );

    let (
        body,
        marker_call_count,
        call_blocks,
        begin_site,
        end_site,
        cleanup_count,
        total_blocks,
        mir_block_count,
    ) = outcome;

    assert_eq!(
        marker_call_count, 0,
        "translated body must contain zero marker Calls after Rule 5 (found {call_blocks:?})",
    );
    assert_eq!(
        body.ordered_regions.len(),
        1,
        "exactly one OrderedRegion expected, got {:?}",
        body.ordered_regions,
    );
    let region = body.ordered_regions[0];

    // No-std, no-Drop specimen: the MIR has zero cleanup blocks, so
    // Pass 2's block-map renumbering is the identity on surviving
    // blocks. That means MIR block index `n` maps directly to
    // `BasicBlockId(n)` in the translated body, and we can compare
    // `body.ordered_regions[0]` bit-for-bit against the MIR-derived
    // expected `entry`/`exit`.
    assert_eq!(
        cleanup_count, 0,
        "specimen must not exercise cleanup blocks so renumbering is the identity",
    );
    assert_eq!(
        total_blocks, mir_block_count,
        "translated body must preserve every MIR block (no cleanup drops, identity renumber): \
         mir_blocks={mir_block_count}, translated_blocks={total_blocks}",
    );

    let (begin_owning_mir, begin_target_mir) = begin_site;
    let (end_owning_mir, end_target_mir) = end_site;
    assert_eq!(
        region.entry,
        BasicBlockId(begin_target_mir),
        "region.entry must equal the translated target of the dropped begin Call \
         (MIR begin block bb{begin_owning_mir} -> target bb{begin_target_mir})",
    );
    assert_eq!(
        region.exit,
        BasicBlockId(end_target_mir),
        "region.exit must equal the translated target of the dropped end Call \
         (MIR end block bb{end_owning_mir} -> target bb{end_target_mir})",
    );
    // The former marker-owning blocks must now be live blocks ending
    // in a `Goto` to their respective marker target (the marker Call
    // was dropped, so the block falls through).
    let begin_owning_bb = &body.basic_blocks[begin_owning_mir as usize];
    let end_owning_bb = &body.basic_blocks[end_owning_mir as usize];
    assert_eq!(
        begin_owning_bb.terminator.kind,
        TerminatorKind::Goto(BasicBlockId(begin_target_mir)),
        "begin-owning block's terminator must become Goto(entry)",
    );
    assert_eq!(
        end_owning_bb.terminator.kind,
        TerminatorKind::Goto(BasicBlockId(end_target_mir)),
        "end-owning block's terminator must become Goto(exit)",
    );

    // Full iff SESE check. Compute the reachability-without-crossing
    // set from `entry`, then compute the exterior set (blocks
    // reachable from block 0 without crossing `entry`), and assert
    // the two sets partition the blocks. Every block is either in
    // the region (reachable from entry without crossing exit), is
    // the exit, or is outside the region.
    let members: Vec<u32> = reachable_without_crossing(&body, region.entry, region.exit);
    let exterior_from_zero: Vec<u32> =
        reachable_without_crossing(&body, BasicBlockId(0), region.entry);

    // No overlap: a block is either strictly inside the region or
    // strictly outside it. `exit` is in neither set.
    for bb in &members {
        assert!(
            !exterior_from_zero.contains(bb),
            "block bb{bb} is reachable from entry without exit AND from 0 without entry -- regions overlap",
        );
    }

    // `entry` is itself a member; `exit` is neither a member nor in
    // the exterior set.
    assert!(members.contains(&region.entry.0));
    assert!(!members.contains(&region.exit.0));
    assert!(!exterior_from_zero.contains(&region.exit.0));

    // Classify every Call block by membership. work_a/work_b are
    // exterior; work_c/work_d are interior. This is the "full iff"
    // verification: the region's member set is exactly the
    // interior-call blocks and their connecting machinery (plus the
    // end-owning block which transitions to `exit`).
    let work_a_bb = find_call_block(&call_blocks, "work_a");
    let work_b_bb = find_call_block(&call_blocks, "work_b");
    let work_c_bb = find_call_block(&call_blocks, "work_c");
    let work_d_bb = find_call_block(&call_blocks, "work_d");

    assert!(exterior_from_zero.contains(&work_a_bb));
    assert!(exterior_from_zero.contains(&work_b_bb));
    assert!(!exterior_from_zero.contains(&work_c_bb));
    assert!(!exterior_from_zero.contains(&work_d_bb));

    assert!(!members.contains(&work_a_bb));
    assert!(!members.contains(&work_b_bb));
    assert!(members.contains(&work_c_bb));
    assert!(members.contains(&work_d_bb));
    assert!(members.contains(&end_owning_mir));

    // The only Call terminators reachable from `entry` without
    // crossing `exit` are the two interior helpers in source order.
    let call_blocks_in_region: Vec<u32> = call_blocks
        .iter()
        .filter(|(bb, _)| members.contains(bb))
        .map(|(bb, _)| *bb)
        .collect();
    assert_eq!(
        call_blocks_in_region,
        vec![work_c_bb, work_d_bb],
        "only work_c and work_d remain as Calls inside the region",
    );

    // Full accounting: every translated block is classified as one
    // of:
    //   * a region member (reachable from `entry` without crossing
    //     `exit`)
    //   * exterior-before-entry (reachable from block 0 without
    //     crossing `entry`)
    //   * exit itself (`BasicBlockId == region.exit`)
    //   * reachable-from-exit (the tail of the CFG that follows the
    //     region, e.g., the final Return block)
    // The four categories are mutually exclusive (assertion below)
    // and exhaustive (every block falls in at least one). This is
    // the full-iff direction of the SESE invariant: no block is
    // left unclassified or silently swept into a fallback class.
    let mut after_exit_seen: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
    {
        // Compute blocks reachable from `exit` inclusive. We include
        // `exit` itself in this set to make the mutual-exclusion
        // check treat `exit` uniformly (it is in the after-exit set,
        // nothing else).
        let mut stack = vec![region.exit];
        while let Some(bb) = stack.pop() {
            if !after_exit_seen.insert(bb.0) {
                continue;
            }
            let idx = bb.0 as usize;
            if idx >= body.basic_blocks.len() {
                continue;
            }
            match &body.basic_blocks[idx].terminator.kind {
                TerminatorKind::Goto(t) => stack.push(*t),
                TerminatorKind::SwitchInt { targets, .. } => {
                    for t in &targets.targets {
                        stack.push(*t);
                    }
                }
                TerminatorKind::Call {
                    target,
                    error_target,
                    ..
                } => {
                    if let Some(t) = target {
                        stack.push(*t);
                    }
                    if let Some(e) = error_target {
                        stack.push(*e);
                    }
                }
                TerminatorKind::IntrinsicCall {
                    target,
                    error_target,
                    ..
                } => {
                    stack.push(*target);
                    if let Some(e) = error_target {
                        stack.push(*e);
                    }
                }
                TerminatorKind::Drop { target, .. } => stack.push(*target),
                TerminatorKind::Return | TerminatorKind::Unreachable => {}
            }
        }
    }

    for i in 0..total_blocks as u32 {
        let in_members = members.contains(&i);
        let in_exterior = exterior_from_zero.contains(&i);
        let in_after_exit = after_exit_seen.contains(&i);

        // Classification must be non-empty (block is covered) and
        // mutually exclusive (block falls into exactly one class).
        // `in_after_exit` subsumes `is_exit` since `exit` is in the
        // after-exit reachable set by construction.
        let class_count = (in_members as u32) + (in_exterior as u32) + (in_after_exit as u32);
        assert_eq!(
            class_count, 1,
            "block bb{i} must belong to exactly one of \
             members/exterior/after-exit (got members={in_members} \
             exterior={in_exterior} after_exit={in_after_exit}); \
             members={members:?}, exterior={exterior_from_zero:?}, \
             after_exit={after_exit_seen:?}",
        );
    }
}

/// Helper: locate a call block by label in the classification Vec.
fn find_call_block(call_blocks: &[(u32, &'static str)], label: &str) -> u32 {
    call_blocks
        .iter()
        .find(|(_, l)| *l == label)
        .unwrap_or_else(|| panic!("block with `{label}` call not found in {call_blocks:?}"))
        .0
}

// ---------------------------------------------------------------------------
// PL011 fixture fidelity -- guards the specimen above against
// fixture drift.
// ---------------------------------------------------------------------------

#[test]
fn pl011_fixture_fidelity() {
    let fixture = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/pl011-full-spec/src/driver.rs",
    ))
    .expect("PL011 fixture source must be readable");

    // Locate `program_polling_port` in the fixture and extract its
    // body verbatim (up to the matching closing `}`), then assert
    // that body uses `ordered! { ... }`. This ties the ordered
    // block to `program_polling_port` specifically -- a
    // whole-file `contains(...)` would silently accept the case
    // where `ordered!` is used elsewhere in the fixture but
    // `program_polling_port` is rewritten to drop it.
    let fn_signature = "fn program_polling_port(&mut self, uart_clk_hz: u32, baud_rate: u32) {";
    let fn_idx = fixture
        .find(fn_signature)
        .expect("PL011 fixture must define `fn program_polling_port(&mut self, uart_clk_hz: u32, baud_rate: u32)`");
    let after_sig = &fixture[fn_idx + fn_signature.len()..];
    // Walk the byte stream tracking `{`/`}` balance so we can carve
    // out exactly `program_polling_port`'s body and inspect it.
    let mut depth: i32 = 1;
    let mut end = 0usize;
    for (i, ch) in after_sig.bytes().enumerate() {
        match ch {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    end = i;
                    break;
                }
            }
            _ => {}
        }
    }
    assert!(
        depth == 0,
        "failed to find closing `}}` for program_polling_port body",
    );
    let body = &after_sig[..end];
    assert!(
        body.contains("ordered! {"),
        "`program_polling_port`'s body must use the `ordered!` macro; got:\n{body}",
    );

    // The `ordered!` macro expands to
    //   ::regseq::internal::ordered_region_begin();
    //   let __regseq_ordered_result = { #input };
    //   ::regseq::internal::ordered_region_end();
    //   __regseq_ordered_result
    // (see `crates/regseq-macros/src/ordered.rs`). This fidelity
    // test asserts every part of that shape stays in place -- if
    // the expansion changes, the inline specimen above (which
    // inlines the same sequence) must be revisited. In particular,
    // the `let __regseq_ordered_result = { ... };` wrapper and the
    // trailing identifier must survive, because they govern whether
    // the end marker sits on a distinct MIR block adjacent to the
    // result expression.
    let macro_src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/regseq-macros/src/ordered.rs",
    ))
    .expect("regseq-macros ordered.rs must be readable");
    for expected in [
        "::regseq::internal::ordered_region_begin();",
        "let __regseq_ordered_result = { #input };",
        "::regseq::internal::ordered_region_end();",
        "__regseq_ordered_result",
    ] {
        assert!(
            macro_src.contains(expected),
            "`ordered!` macro expansion must contain `{expected}`; macro source was:\n{macro_src}",
        );
    }
}

// ---------------------------------------------------------------------------
// Test 2: nested regions -- strict containment
// ---------------------------------------------------------------------------

const NESTED_BODY: &str = r#"
impl Drv {
    pub fn nested(&mut self) {
        ordered_region_begin();
        work_a();
        ordered_region_begin();
        work_b();
        work_c();
        ordered_region_end();
        work_d();
        ordered_region_end();
    }
}
"#;

#[test]
fn nested_ordered_regions_produce_strict_containment() {
    let src = compose(NESTED_BODY);
    let outcome = with_specimen(&src, "construction_ordered_region_nested", |tcx, pass1| {
        let begin = find_free_fn_def_id(tcx, "ordered_region_begin");
        let end = find_free_fn_def_id(tcx, "ordered_region_end");
        let ordered_markers = OrderedMarkers::from_def_ids(begin, end);
        let try_markers = TryMarkers::default();
        let log_arg = LogArgUnwrap::default();
        let caller = find_method_def_id(tcx, "Drv", "nested");
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
            &ordered_markers,
            &try_markers,
            &log_arg,
            &RegisterIndex::default(),
            &std::collections::HashMap::new(),
            &AbstractionTypeRegistry::default(),
            &mut interner,
        )
        .expect("build_body must succeed on nested");

        let begin_fn_id = pass1.cross_refs.functions.get(&begin).copied();
        let end_fn_id = pass1.cross_refs.functions.get(&end).copied();
        let mut marker_calls = 0usize;
        for bb in &body.basic_blocks {
            if let TerminatorKind::Call { callee, .. } = &bb.terminator.kind {
                if Some(*callee) == begin_fn_id || Some(*callee) == end_fn_id {
                    marker_calls += 1;
                }
            }
        }
        (body, marker_calls)
    });

    let (body, marker_calls) = outcome;
    assert_eq!(marker_calls, 0, "both marker pairs dropped from CFG");
    assert_eq!(
        body.ordered_regions.len(),
        2,
        "two OrderedRegions expected (inner + outer), got {:?}",
        body.ordered_regions,
    );

    // Pairing order is LIFO: the inner region is recorded first
    // (its `End` pops first), then the outer. This mirrors the
    // `pair_ordered_regions` stack discipline in
    // `construction::ordered_region`.
    let inner = body.ordered_regions[0];
    let outer = body.ordered_regions[1];

    let inner_members = reachable_without_crossing(&body, inner.entry, inner.exit);
    let outer_members = reachable_without_crossing(&body, outer.entry, outer.exit);

    // Strict containment: every inner member is also an outer member.
    for id in &inner_members {
        assert!(
            outer_members.contains(id),
            "inner region member bb{id} must also be an outer region member (inner={inner_members:?}, outer={outer_members:?})",
        );
    }
    // Strictness: the outer region contains at least one block the
    // inner does not (the block(s) between the outer begin and the
    // inner begin, or between the inner end and the outer end).
    let strictly_outer: Vec<u32> = outer_members
        .iter()
        .filter(|id| !inner_members.contains(id))
        .copied()
        .collect();
    assert!(
        !strictly_outer.is_empty(),
        "outer region must strictly contain the inner (found equal sets: {outer_members:?})",
    );
}

// ---------------------------------------------------------------------------
// Test 3: unpaired begin -> UnpairedOrderedMarker
// ---------------------------------------------------------------------------

const UNPAIRED_BEGIN_BODY: &str = r#"
impl Drv {
    pub fn unpaired_begin(&mut self) {
        ordered_region_begin();
        work_a();
    }
}
"#;

#[test]
fn unpaired_begin_marker_hard_errors() {
    let src = compose(UNPAIRED_BEGIN_BODY);
    let outcome = with_specimen(
        &src,
        "construction_ordered_region_unpaired_begin",
        |tcx, pass1| {
            let begin = find_free_fn_def_id(tcx, "ordered_region_begin");
            let end = find_free_fn_def_id(tcx, "ordered_region_end");
            let ordered_markers = OrderedMarkers::from_def_ids(begin, end);
            let try_markers = TryMarkers::default();
            let log_arg = LogArgUnwrap::default();
            let caller = find_method_def_id(tcx, "Drv", "unpaired_begin");
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
                &log_arg,
                &RegisterIndex::default(),
                &std::collections::HashMap::new(),
                &AbstractionTypeRegistry::default(),
                &mut interner,
            ) {
                Err(ConstructionError::UnpairedOrderedMarker { rendered, .. }) => Ok(rendered),
                Ok(_) => Err("expected UnpairedOrderedMarker, got Ok".to_string()),
                Err(other) => Err(format!("expected UnpairedOrderedMarker, got {other:?}")),
            }
        },
    );

    let rendered = outcome.expect("unpaired begin must hard-error");
    assert!(
        rendered.contains("ordered_region_begin"),
        "error payload must name the unpaired begin marker, got `{rendered}`",
    );
    assert!(
        rendered.contains("no matching end"),
        "error payload must describe what is missing, got `{rendered}`",
    );
}

// ---------------------------------------------------------------------------
// Test 4: unpaired end -> UnpairedOrderedMarker
// ---------------------------------------------------------------------------

const UNPAIRED_END_BODY: &str = r#"
impl Drv {
    pub fn unpaired_end(&mut self) {
        work_a();
        ordered_region_end();
    }
}
"#;

#[test]
fn unpaired_end_marker_hard_errors() {
    let src = compose(UNPAIRED_END_BODY);
    let outcome = with_specimen(
        &src,
        "construction_ordered_region_unpaired_end",
        |tcx, pass1| {
            let begin = find_free_fn_def_id(tcx, "ordered_region_begin");
            let end = find_free_fn_def_id(tcx, "ordered_region_end");
            let ordered_markers = OrderedMarkers::from_def_ids(begin, end);
            let try_markers = TryMarkers::default();
            let log_arg = LogArgUnwrap::default();
            let caller = find_method_def_id(tcx, "Drv", "unpaired_end");
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
                &log_arg,
                &RegisterIndex::default(),
                &std::collections::HashMap::new(),
                &AbstractionTypeRegistry::default(),
                &mut interner,
            ) {
                Err(ConstructionError::UnpairedOrderedMarker { rendered, .. }) => Ok(rendered),
                Ok(_) => Err("expected UnpairedOrderedMarker, got Ok".to_string()),
                Err(other) => Err(format!("expected UnpairedOrderedMarker, got {other:?}")),
            }
        },
    );

    let rendered = outcome.expect("unpaired end must hard-error");
    assert!(
        rendered.contains("ordered_region_end"),
        "error payload must name the unpaired end marker, got `{rendered}`",
    );
    assert!(
        rendered.contains("no matching begin"),
        "error payload must describe what is missing, got `{rendered}`",
    );
}

// ---------------------------------------------------------------------------
// Test 5: production OrderedMarkers::resolve path for specimens that
// do not depend on the `regseq` facade crate.
// ---------------------------------------------------------------------------
//
// These inline specimens have no `regseq` dependency, so
// `OrderedMarkers::resolve` must return an empty marker set (both
// DefIds None). This path is what Pass 2 setup uses in production;
// the test confirms it does not hard-error on a legitimately
// facade-less crate. The facade-loaded-but-marker-missing error
// path (`OrderedMarkerLookupFailed`) cannot be exercised through the
// inline harness without building a renamed facade in-process; the
// synthetic marker-lookup unit in src covers that behaviour through
// the `TyCtxt::module_children` walk pattern (a missing child name
// yields `None`, which `resolve` translates into the typed error).

#[test]
fn resolve_without_regseq_facade_yields_no_markers() {
    let src = compose("");
    let resolved_is_present = with_specimen(
        &src,
        "construction_ordered_region_resolve_no_facade",
        |tcx, _pass1| {
            let markers =
                OrderedMarkers::resolve(tcx).expect("resolve must succeed when facade is absent");
            markers.is_resolved()
        },
    );

    assert!(
        !resolved_is_present,
        "specimen without `regseq` dependency must yield no markers",
    );
}

// ---------------------------------------------------------------------------
// Test 6: OrderedMarkerLookupFailed -- facade crate with missing markers.
// ---------------------------------------------------------------------------
//
// Manufactures the facade-loaded-but-marker-missing state by building
// a facade-shaped crate named `regseq` that exports an `internal`
// module without the expected marker functions. This exercises the
// exact resolution path production uses (`find_crate_by_name("regseq")`
// -> walk `internal` module) and drives it to the
// `OrderedMarkerLookupFailed` branch.

#[test]
fn ordered_marker_lookup_failed_on_renamed_facade() {
    // Compile a minimal fake `regseq` crate whose `internal` module
    // has neither `ordered_region_begin` nor `ordered_region_end`.
    // We run a dedicated rustc invocation for it so the specimen
    // below can link to it as `extern crate regseq`.
    let _lock = COMPILER_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    init_rustc();

    let test_name = "construction_ordered_region_lookup_failed";
    let test_dir = std::env::temp_dir()
        .join("regseq-driver-tests")
        .join(test_name);
    if test_dir.exists() {
        std::fs::remove_dir_all(&test_dir).unwrap();
    }
    std::fs::create_dir_all(&test_dir).unwrap();

    let fake_facade_src = r#"
#![no_std]

pub mod internal {
    // Deliberately empty: no ordered_region_begin/end.
}
"#;
    let fake_facade_path = test_dir.join("regseq.rs");
    std::fs::write(&fake_facade_path, fake_facade_src).unwrap();

    let sysroot = get_sysroot();

    // Compile the fake facade as an rlib named `regseq`.
    let facade_status = std::process::Command::new("rustc")
        .arg(&fake_facade_path)
        .arg("--sysroot")
        .arg(&sysroot)
        .arg("--edition")
        .arg("2021")
        .arg("--crate-type")
        .arg("rlib")
        .arg("--crate-name")
        .arg("regseq")
        .arg("--out-dir")
        .arg(&test_dir)
        .status()
        .expect("failed to invoke rustc to build fake facade");
    assert!(
        facade_status.success(),
        "fake facade crate must compile cleanly",
    );

    let specimen_src = r#"
#![no_std]

extern crate regseq;

#[rseq::driver(name = "spec-drv", compatible = ["acme,widget"], bus = "amba")]
pub struct Drv;
"#;
    let specimen_path = test_dir.join("specimen.rs");
    std::fs::write(&specimen_path, specimen_src).unwrap();

    let out: Arc<Mutex<Option<bool>>> = Arc::new(Mutex::new(None));
    let shared = out.clone();

    struct Callbacks {
        out: Arc<Mutex<Option<bool>>>,
    }

    impl rustc_driver::Callbacks for Callbacks {
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
            let result = OrderedMarkers::resolve(tcx);
            let matched = matches!(
                result,
                Err(ConstructionError::OrderedMarkerLookupFailed { .. })
            );
            *self.out.lock().unwrap() = Some(matched);
            rustc_driver::Compilation::Stop
        }
    }

    let mut callbacks = Callbacks { out: shared };
    let args = vec![
        "rustc".to_string(),
        specimen_path.to_str().unwrap().to_string(),
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
        "--extern".to_string(),
        format!(
            "regseq={}",
            test_dir.join("libregseq.rlib").to_str().unwrap()
        ),
        "-L".to_string(),
        test_dir.to_str().unwrap().to_string(),
        "-Zmir-opt-level=0".to_string(),
        "-Coverflow-checks=off".to_string(),
        "-Cdebug-assertions=off".to_string(),
    ];

    rustc_driver::run_compiler(&args, &mut callbacks);

    let matched = out
        .lock()
        .unwrap()
        .take()
        .expect("callback must run in after_analysis");

    std::fs::remove_dir_all(&test_dir).ok();

    assert!(
        matched,
        "OrderedMarkers::resolve must return OrderedMarkerLookupFailed when the facade is present but markers are missing",
    );
}
