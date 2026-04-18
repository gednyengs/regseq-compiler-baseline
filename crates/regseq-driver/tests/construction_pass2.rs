//! Pass 2 straight-line body construction tests (regseq-model-43).
//!
//! Each test compiles a small inline specimen through a rustc harness,
//! drives Pass 1 to completion, then calls `build_body` on a specific
//! function and asserts the returned [`Body`] matches the spec in
//! `.harness/tasks/regseq-model/43-pass2-straight-line.md`.

#![feature(rustc_private)]
#![feature(box_patterns)]

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
use regseq_model::identity::{FunctionId, SourceOrigin, SymbolInterner};
use regseq_model::ir::{Body, StatementKind, TerminatorKind};
use regseq_model::model::{Function, Module, RegseqModel};

// ---------------------------------------------------------------------------
// Compiler harness
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

/// Run `f` inside `after_analysis` with a live [`TyCtxt`] and the Pass 1
/// [`Pass1Output`]. `f` returns an arbitrary `Send + 'static` result so
/// callers can unpack whatever they need before rustc shuts down.
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
        // Match the extraction stage pinned by ir-reference.md
        // (`mir_drops_elaborated_and_const_checked`, `mir_opt_level = 0`,
        // release-profile semantics). Without `-Coverflow-checks=off`
        // arithmetic operators lower to `AddWithOverflow + Assert` and
        // introduce terminators Pass 2 rejects.
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
// Lookup helpers
// ---------------------------------------------------------------------------

/// Find the [`DefId`] of a method on a named impl-block self-type by
/// walking the HIR. Matches on the impl's self-type rendered via
/// `tcx.def_path_str` (sufficient for these single-file specimens).
fn find_method_def_id<'tcx>(tcx: TyCtxt<'tcx>, self_ty: &str, method: &str) -> DefId {
    for item_id in tcx
        .hir_module_items(rustc_hir::def_id::LocalModDefId::CRATE_DEF_ID)
        .free_items()
    {
        let item = tcx.hir_item(item_id);
        if let rustc_hir::ItemKind::Impl(impl_block) = &item.kind {
            let impl_owner = item.owner_id.def_id.to_def_id();
            let self_ty_middle = tcx.type_of(impl_owner).instantiate_identity();
            if format!("{self_ty_middle}") != self_ty {
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

/// Find the [`DefId`] of a free function by name.
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

/// Locate a mutable reference to a [`Function`] in the model by its
/// [`FunctionId`]. Walks the module tree and every impl-block-like
/// collection. Panics if the function is not present.
fn find_function_mut(model: &mut RegseqModel, id: FunctionId) -> &mut Function {
    fn walk_mut(m: &mut Module, id: FunctionId) -> Option<&mut Function> {
        for f in &mut m.free_functions {
            if f.id == id {
                return Some(f);
            }
        }
        for b in &mut m.impl_blocks {
            for f in &mut b.methods {
                if f.id == id {
                    return Some(f);
                }
            }
        }
        for b in &mut m.trait_impls {
            for f in &mut b.methods {
                if f.id == id {
                    return Some(f);
                }
            }
        }
        for b in &mut m.driver_impl_blocks {
            for f in &mut b.methods {
                if f.id == id {
                    return Some(f);
                }
            }
        }
        for sub in &mut m.submodules {
            if let Some(found) = walk_mut(sub, id) {
                return Some(found);
            }
        }
        None
    }
    walk_mut(&mut model.root_module, id).expect("FunctionId must exist in the model")
}

// ---------------------------------------------------------------------------
// Positive: disable_uart-like straight-line method
// ---------------------------------------------------------------------------

/// Minimal driver-struct boilerplate prepended to every specimen so
/// Pass 1 (which requires exactly one `#[rseq::driver]` struct per
/// crate) succeeds. Tests then drive Pass 2 against specific functions
/// defined after this preamble.
const DRIVER_PREAMBLE: &str = r#"
#![no_std]
#![feature(core_intrinsics)]

pub struct Mmio<T> { _m: core::marker::PhantomData<T> }
pub struct Clock;

#[allow(non_snake_case)]
pub mod DR_REG { pub struct Register; }

pub struct ReadWrite<T, R> { _v: T, _r: core::marker::PhantomData<R> }

#[repr(C)]
#[rseq::register_block(endian = "little", access_width = 32)]
pub struct Block {
    pub dr: ReadWrite<u32, DR_REG::Register>,
}

#[rseq::driver(name = "spec-drv", compatible = ["acme,widget"], bus = "amba")]
pub struct Drv {
    #[rseq::mmio(id = "regs")]
    pub regs: Mmio<Block>,
    #[rseq::clock(id = "clk")]
    pub clk: Clock,
}
"#;

/// Specimen body for the disable_uart-like test: local helpers, a
/// Drop-having type, and a driver method that chains calls through
/// that live local. The Drop-elaborated MIR puts an unwind cleanup
/// chain on each call site so the test exercises cleanup-block
/// dropping in addition to call translation.
const DRIVER_SPECIMEN_BODY: &str = r#"
pub fn helper_a(x: u32) -> u32 { x + 1 }
pub fn helper_b(x: u32, y: u32) -> u32 { x + y }

pub struct DisableGuard { pub n: u32 }
impl Drop for DisableGuard {
    fn drop(&mut self) { let _ = self.n; }
}

impl Drv {
    pub fn disable_uart(&mut self) {
        let _g = DisableGuard { n: 1 };
        let a = helper_a(7);
        let b = helper_b(a, 9);
        let _c = a + b;
    }
}
"#;

fn compose(body: &str) -> String {
    format!("{DRIVER_PREAMBLE}{body}")
}

#[test]
fn disable_uart_like_method_translates_to_straight_line_body() {
    let src = compose(DRIVER_SPECIMEN_BODY);
    let (
        body,
        arg_count_from_model,
        locals_len,
        total_mir_blocks,
        cleanup_dropped,
        fn_id,
        body_after_attach,
    ) = with_specimen(
        &src,
        "construction_pass2_disable_uart_like",
        |tcx, pass1| {
            let def_id = find_method_def_id(tcx, "Drv", "disable_uart");
            let mir_body = tcx.optimized_mir(def_id);
            let locals_len = mir_body.local_decls.len();
            let total_mir_blocks = mir_body.basic_blocks.len();
            let cleanup_dropped = mir_body
                .basic_blocks
                .iter()
                .filter(|d| d.is_cleanup)
                .count();

            let fn_id = *pass1
                .cross_refs
                .functions
                .get(&def_id)
                .expect("disable_uart FunctionId must be present after Pass 1");

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
                def_id,
                fn_id,
                &type_def_index,
                &constants,
                &functions,
                &intrinsics,
                &deref_lookup,
                &mut interner,
            )
            .expect("build_body must succeed on straight-line MIR");

            // Mutate the Pass 1 model to attach the real body for
            // the spec's "attached to the function in the Driver
            // impl block" assertion. We clone the Pass 1 model so
            // the rest of this closure has full freedom; only the
            // outputs surfaced below need to survive rustc shutdown.
            let mut model = pass1.model.clone();
            {
                let function = find_function_mut(&mut model, fn_id);
                function.body = body.clone();
            }
            // Re-retrieve the attached body from the model to
            // confirm the attachment wired through.
            let attached = find_function_ro(&model, fn_id).body.clone();

            let arg_count = find_function_ro(&model, fn_id).body.arg_count;

            (
                body,
                arg_count,
                locals_len,
                total_mir_blocks,
                cleanup_dropped,
                fn_id.0.index(),
                attached,
            )
        },
    );

    let _ = fn_id;

    // (1) arg_count matches `&mut self` -- exactly one argument.
    assert_eq!(body.arg_count, 1, "disable_uart_like has only &mut self");
    assert_eq!(arg_count_from_model, 1);

    // (2) local_decls length matches the MIR body's local count.
    assert_eq!(
        body.local_decls.len(),
        locals_len,
        "local_decls must be 1:1 with MIR locals"
    );

    // (3) Every Call terminator lands as Call { error_target: None, .. }.
    assert!(
        body.basic_blocks
            .iter()
            .any(|bb| matches!(bb.terminator.kind, TerminatorKind::Call { .. })),
        "specimen must include at least one Call"
    );
    for bb in &body.basic_blocks {
        if let TerminatorKind::Call { error_target, .. } = &bb.terminator.kind {
            assert!(
                error_target.is_none(),
                "task 43 always emits `error_target: None`"
            );
        }
    }

    // (4) Source map populated: body_span + one entry per local / per
    // statement / per terminator.
    assert!(
        matches!(body.source_map.body_span, SourceOrigin::Source(_)),
        "body_span must resolve to a real Source origin"
    );
    assert_eq!(body.source_map.locals.len(), body.local_decls.len());
    assert_eq!(body.source_map.blocks.len(), body.basic_blocks.len());
    for (i, block) in body.basic_blocks.iter().enumerate() {
        assert_eq!(
            body.source_map.blocks[i].statements.len(),
            block.statements.len(),
            "per-statement source map must be structurally parallel at block {i}",
        );
        // Every block carries exactly one terminator-origin entry: the
        // field is a single `SourceOrigin`, so length is enforced
        // structurally. Assert that the origin is a real span (non-
        // CompilerGenerated) -- Pass 2 straight-line never synthesizes.
        assert!(
            matches!(
                body.source_map.blocks[i].terminator,
                SourceOrigin::Source(_)
            ),
            "terminator origin at block {i} must be Source(..)",
        );
    }
    for (i, origin) in body.source_map.locals.iter().enumerate() {
        assert!(
            matches!(origin, SourceOrigin::Source(_)),
            "local {i} must have a Source origin",
        );
    }
    for (i, block_sm) in body.source_map.blocks.iter().enumerate() {
        for (j, origin) in block_sm.statements.iter().enumerate() {
            assert!(
                matches!(origin, SourceOrigin::Source(_)),
                "statement {i}:{j} must have a Source origin",
            );
        }
    }

    // (5) Cleanup blocks dropped per ir-reference.md Section 6.
    // Every `is_cleanup` MIR block is either the spec-literal
    // UnwindResume leaf (dropped directly) or an interior block
    // reachable only through unwind edges (which translation
    // strips, rendering it unreachable). So the kept block count
    // must exactly equal the number of non-cleanup MIR blocks --
    // no cleanup block survives.
    assert!(
        cleanup_dropped > 0,
        "specimen must exercise cleanup blocks so the drop rule is actually tested"
    );
    assert_eq!(
        body.basic_blocks.len(),
        total_mir_blocks - cleanup_dropped,
        "kept block count must match non-cleanup MIR block count",
    );
    // Contiguous renumbering: IDs are Vec indices. Check every
    // terminator-target BasicBlockId lands in `[0, N)` so no edge
    // references a dropped block.
    assert!(!body.basic_blocks.is_empty(), "at least one block");
    for bb in &body.basic_blocks {
        match &bb.terminator.kind {
            TerminatorKind::Goto(t) | TerminatorKind::Drop { target: t, .. } => {
                assert!((t.0 as usize) < body.basic_blocks.len());
            }
            TerminatorKind::Call { target, .. } => {
                if let Some(t) = target {
                    assert!((t.0 as usize) < body.basic_blocks.len());
                }
            }
            TerminatorKind::Return | TerminatorKind::Unreachable => {}
            TerminatorKind::SwitchInt { .. } | TerminatorKind::IntrinsicCall { .. } => {
                panic!("specimen must not produce SwitchInt or IntrinsicCall in task 43 scope");
            }
        }
    }

    // (6) Body attached to the disable_uart Function in the Driver's
    // impl block: the clone retrieved from the mutated model equals
    // the body we built.
    assert_eq!(
        body_after_attach, body,
        "attached body must equal the build_body output",
    );
}

fn find_function_ro(model: &RegseqModel, id: FunctionId) -> &Function {
    fn walk<'a>(m: &'a Module, id: FunctionId) -> Option<&'a Function> {
        for f in &m.free_functions {
            if f.id == id {
                return Some(f);
            }
        }
        for b in &m.impl_blocks {
            for f in &b.methods {
                if f.id == id {
                    return Some(f);
                }
            }
        }
        for b in &m.trait_impls {
            for f in &b.methods {
                if f.id == id {
                    return Some(f);
                }
            }
        }
        for b in &m.driver_impl_blocks {
            for f in &b.methods {
                if f.id == id {
                    return Some(f);
                }
            }
        }
        for sub in &m.submodules {
            if let Some(x) = walk(sub, id) {
                return Some(x);
            }
        }
        None
    }
    walk(&model.root_module, id).expect("FunctionId must resolve in model")
}

// ---------------------------------------------------------------------------
// Statement coverage: StorageLive/Dead and Assign retained;
// Nop/FakeRead/etc. dropped.
// ---------------------------------------------------------------------------

const STATEMENT_SPECIMEN_BODY: &str = r#"
pub fn storage_lifecycle(x: u32) -> u32 {
    let a = x + 1;
    let b = a * 2;
    b
}
"#;

#[test]
fn statement_kinds_assign_and_storage_roundtrip() {
    let src = compose(STATEMENT_SPECIMEN_BODY);
    let body = build_free_fn_body(&src, "construction_pass2_statements", "storage_lifecycle");
    let (assigns, storage_live, storage_dead) = count_statement_kinds(&body);
    assert!(assigns >= 1, "specimen has at least one Assign");
    assert!(
        storage_live >= 1 && storage_dead >= 1,
        "straight-line body produces balanced StorageLive/StorageDead pairs",
    );
}

fn count_statement_kinds(body: &Body) -> (usize, usize, usize) {
    let mut assigns = 0;
    let mut live = 0;
    let mut dead = 0;
    for bb in &body.basic_blocks {
        for stmt in &bb.statements {
            match stmt.kind {
                StatementKind::Assign(_, _) => assigns += 1,
                StatementKind::StorageLive(_) => live += 1,
                StatementKind::StorageDead(_) => dead += 1,
                StatementKind::SetDiscriminant { .. } | StatementKind::MirIntrinsic(_) => {}
            }
        }
    }
    (assigns, live, dead)
}

// ---------------------------------------------------------------------------
// Drop terminator: a user type with Drop produces `TerminatorKind::Drop`
// with `unwind`/`replace` stripped.
// ---------------------------------------------------------------------------

const DROP_SPECIMEN_BODY: &str = r#"
pub struct Guard { pub n: u32 }

impl Drop for Guard {
    fn drop(&mut self) { let _ = self.n; }
}

pub fn exercise_drop(n: u32) -> u32 {
    let g = Guard { n };
    let out = g.n + 1;
    out
}
"#;

#[test]
fn drop_terminator_lands_as_drop_with_target_only() {
    let src = compose(DROP_SPECIMEN_BODY);
    let body = build_free_fn_body(&src, "construction_pass2_drop_terminator", "exercise_drop");
    let mut drop_seen = false;
    for bb in &body.basic_blocks {
        if let TerminatorKind::Drop { target, .. } = &bb.terminator.kind {
            drop_seen = true;
            assert!((target.0 as usize) < body.basic_blocks.len());
        }
    }
    assert!(
        drop_seen,
        "Guard's Drop impl must produce at least one Drop terminator",
    );
    // Cleanup blocks dropped: no Unreachable predecessor chain from
    // drop-in-cleanup survives. Check there is at least one Return.
    assert!(body
        .basic_blocks
        .iter()
        .any(|bb| matches!(bb.terminator.kind, TerminatorKind::Return)));
}

// ---------------------------------------------------------------------------
// MirIntrinsic: Assume + CopyNonOverlapping
// ---------------------------------------------------------------------------

const INTRINSIC_SPECIMEN_BODY: &str = r#"
use core::intrinsics;

pub unsafe fn do_assume(flag: bool) {
    intrinsics::assume(flag);
}
"#;

#[test]
fn mir_intrinsic_assume_translates_to_mir_intrinsic_statement() {
    // `assume` lowers to `StatementKind::MirIntrinsic(Assume(_))`.
    //
    // Note: the CopyNonOverlapping sub-variant is not exercised here.
    // Calling it from user code requires raw-pointer-typed locals
    // (`*const T`, `*mut T`), which `map_ty` currently rejects as
    // `UnsupportedRustType`. A fixture for the CopyNonOverlapping path
    // can be added once raw-pointer types have an IR encoding. The
    // translation logic for `CopyNonOverlapping` is still exercised via
    // the unit tests in `crates/regseq-model/src/ir/statement.rs`.
    let src = compose(INTRINSIC_SPECIMEN_BODY);
    let assume_body = build_free_fn_body(&src, "construction_pass2_assume", "do_assume");
    let has_assume = assume_body.basic_blocks.iter().any(|bb| {
        bb.statements.iter().any(|s| {
            matches!(
                s.kind,
                StatementKind::MirIntrinsic(regseq_model::ir::MirIntrinsic::Assume(_))
            )
        })
    });
    assert!(has_assume, "assume must lower to MirIntrinsic::Assume");
}

// ---------------------------------------------------------------------------
// Negative: InlineAsm surfaces UnsupportedTerminator
// ---------------------------------------------------------------------------

const INLINE_ASM_SPECIMEN_BODY: &str = r#"
use core::arch::asm;

pub unsafe fn do_asm() {
    asm!("nop");
}
"#;

#[test]
fn inline_asm_surfaces_unsupported_terminator() {
    let src = compose(INLINE_ASM_SPECIMEN_BODY);
    let outcome = with_specimen(&src, "construction_pass2_inline_asm", |tcx, pass1| {
        let def_id = find_free_fn_def_id(tcx, "do_asm");
        let fn_id = *pass1
            .cross_refs
            .functions
            .get(&def_id)
            .expect("Pass 1 assigns a FunctionId to every free fn");
        let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
        let constants = ConstantIndex::new(&pass1.cross_refs.constants);
        let functions = FunctionIndex::new(&pass1.cross_refs.functions);
        let intrinsics = IntrinsicLookup::default();
        let abstraction_registry = AbstractionTypeRegistry::default();
        let deref_lookup = DerefCollapseLookup::build(tcx, &abstraction_registry)
            .expect("Deref lang item must be resolvable on the specimen crate");
        let mut interner = SymbolInterner::new();
        build_body(
            tcx,
            def_id,
            fn_id,
            &type_def_index,
            &constants,
            &functions,
            &intrinsics,
            &deref_lookup,
            &mut interner,
        )
        .map(|_| ())
    });
    match outcome {
        Err(ConstructionError::UnsupportedTerminator { rendered, .. }) => {
            assert!(
                rendered.contains("InlineAsm"),
                "InlineAsm must surface in the rendered terminator name, got `{rendered}`",
            );
        }
        other => panic!("expected UnsupportedTerminator(InlineAsm), got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Shared helper: build a Body for a free function by name.
// ---------------------------------------------------------------------------

fn build_free_fn_body(source: &str, test_name: &str, fn_name: &str) -> Body {
    let fn_name = fn_name.to_string();
    with_specimen(source, test_name, move |tcx, pass1| {
        let def_id = find_free_fn_def_id(tcx, &fn_name);
        let fn_id = *pass1
            .cross_refs
            .functions
            .get(&def_id)
            .expect("Pass 1 must assign a FunctionId to the target function");
        let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
        let constants = ConstantIndex::new(&pass1.cross_refs.constants);
        let functions = FunctionIndex::new(&pass1.cross_refs.functions);
        let intrinsics = IntrinsicLookup::default();
        let abstraction_registry = AbstractionTypeRegistry::default();
        let deref_lookup = DerefCollapseLookup::build(tcx, &abstraction_registry)
            .expect("Deref lang item must be resolvable on the specimen crate");
        let mut interner = SymbolInterner::new();
        build_body(
            tcx,
            def_id,
            fn_id,
            &type_def_index,
            &constants,
            &functions,
            &intrinsics,
            &deref_lookup,
            &mut interner,
        )
        .expect("build_body must succeed on the free-fn specimen")
    })
}

// ---------------------------------------------------------------------------
// SetDiscriminant statement
// ---------------------------------------------------------------------------
//
// Direct enum construction (`Color::Blue(42)`) lowers to
// `Rvalue::Aggregate(AggregateKind::Adt, [...])` at
// `mir_drops_elaborated_and_const_checked` with `mir_opt_level = 0`,
// not to a `StatementKind::SetDiscriminant`. The latter is emitted
// almost exclusively by coroutine/state-machine transforms, drop
// elaboration's tag-only writes, and `try_trait_v2` desugarings --
// none of which appear through straight-line user code in Pass 2's
// scope.
//
// The translation is a one-liner (`variant_index.as_u32()`), covered
// structurally by the exhaustive `match` in `translate_statement`
// and unit-tested in `crates/regseq-model/src/ir/statement.rs`. A
// dedicated end-to-end test is deferred until task 44 (SwitchInt) or
// task 46 (?-folding) land, both of which naturally surface
// SetDiscriminant through their respective MIR patterns.

// ---------------------------------------------------------------------------
// Unreachable terminator
// ---------------------------------------------------------------------------
//
// `TerminatorKind::Unreachable` is hard to surface from straight-line
// user code at our MIR stage:
//
// - `loop {}` lowers to `Goto(bb0)`, not Unreachable.
// - `panic!()` / `unreachable!()` lower to Call terminators targeting
//   std panic intrinsics, which are cross-crate and resolve to
//   `UnresolvedCallTarget` before the Unreachable ever appears.
// - `core::hint::unreachable_unchecked()` is a wrapper fn -- the
//   MIR emits a Call to it, not an Unreachable terminator.
// - Exhaustive `match` on an uninhabited type synthesizes
//   `SwitchInt -> Unreachable`, but SwitchInt is task 44.
//
// The translation for Unreachable is a one-liner
// (`TerminatorKind::Unreachable`) with no data to misplace, and it is
// reachable as a compile-time-checked arm of the exhaustive
// `match` in `translate_terminator`. A dedicated end-to-end test is
// therefore deferred until task 44 lands SwitchInt, which enables
// surfacing Unreachable through exhaustive-match code paths.

// ---------------------------------------------------------------------------
// Dropped-statement fixture (Nop / FakeRead / Retag / PlaceMention /
// Coverage / ConstEvalCounter / AscribeUserType / BackwardIncompatibleDropHint)
// ---------------------------------------------------------------------------
//
// The Pass 2 spec's Test Plan suggests a fixture that surfaces these
// statements so their `Ok(None)` drop arms are exercised end-to-end.
// However, at `mir_drops_elaborated_and_const_checked` with
// `mir_opt_level = 0` and release-profile semantics:
//
// - `FakeRead` and `AscribeUserType` are disallowed after drop
//   elaboration; they never appear in the MIR Pass 2 reads.
// - `Retag` is only generated under `-Z mir-emit-retag`, which we do
//   not pass.
// - `Coverage` / `ConstEvalCounter` / `BackwardIncompatibleDropHint`
//   require specific rustc flags or edition settings not exercised
//   by the harness.
// - `Nop` is never emitted by the front-end at this phase.
//
// The task spec explicitly permits skipping this fixture "if not
// reachable through normal code, skip and note in the test comment"
// (see the Test Plan section). The drop arms are still structurally
// exercised: the `match` in `translate_statement` is exhaustive, so
// every un-reached arm is a compile-time no-op that would break the
// build if renamed or removed.

// ---------------------------------------------------------------------------
// CopyNonOverlapping sub-variant
// ---------------------------------------------------------------------------
//
// Exercising `MirIntrinsic::CopyNonOverlapping` end-to-end through
// Pass 2 requires function locals typed as raw pointers (`*const T`
// / `*mut T`): MIR builds the intrinsic statement directly from
// pointer-typed operands. `map_ty` rejects raw pointers as
// `UnsupportedRustType` because the Regseq IR has no raw-pointer
// variant yet, so the translation cannot proceed far enough to
// emit the `CopyNonOverlapping` statement. Adding raw-pointer
// support is an IR-level change outside task 43's scope; until
// then the `CopyNonOverlapping` mapping is covered structurally
// by the unit tests in `crates/regseq-model/src/ir/statement.rs`
// and by the `translate_non_diverging_intrinsic` exhaustive match
// in `pass2.rs`.
