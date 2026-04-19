//! Rule 3 cross-crate function extraction tests (regseq-model-47).
//!
//! Each test compiles a small inline specimen through a rustc harness
//! and invokes `build_pass1_output`. The tests then assert that the
//! constructed [`RegseqModel`] contains the expected cross-crate
//! `Function` entries placed under synthetic module chains rooted at
//! the callee's defining crate name.
//!
//! Coverage follows the task spec's Test Plan:
//!
//! - A specimen whose authored body calls `Option::<T>::map` and
//!   `Option::<T>::unwrap_or` produces `Function` entries for each
//!   callee under the synthetic `core::option::Option` chain.
//! - A specimen that references `std::*` in an authored body raises
//!   `ConstructionError::StdFunctionNotPermitted`.
//! - Reachability bounded: synthetic modules are only created on
//!   demand -- unreferenced core functions are not eagerly
//!   materialized.

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
    build_body, build_pass1_output, build_pass1_output_with_cross_crate_depth, ConstantIndex,
    ConstructionError, DerefCollapseLookup, ExtractionRegistries, FunctionIndex, IntrinsicLookup,
    OrderedMarkers, Pass1Output, PinStateRegistry, TryMarkers, TypeDefIndex,
};
use regseq_driver::extract::abstraction_types::AbstractionTypeRegistry;
use regseq_driver::extract::framework_types::FrameworkTypeRegistry;
use regseq_driver::extract::intrinsic_registry::IntrinsicRegistry;
use regseq_model::identity::SymbolInterner;
use regseq_model::model::{Function, Module, RegseqModel};

type Outcome = Result<Pass1Output, ConstructionError>;

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

fn compile_and_build(source: &str, test_name: &str) -> Outcome {
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

    let result: Arc<Mutex<Option<Outcome>>> = Arc::new(Mutex::new(None));

    struct Callbacks {
        result: Arc<Mutex<Option<Outcome>>>,
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
            let outcome = build_pass1_output(tcx, &registries);
            *self.result.lock().unwrap() = Some(outcome);
            rustc_driver::Compilation::Stop
        }
    }

    let mut callbacks = Callbacks {
        result: result.clone(),
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

    let outcome = result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis must run");

    std::fs::remove_dir_all(&test_dir).ok();

    outcome
}

// ---------------------------------------------------------------------------
// Helpers for walking the constructed model
// ---------------------------------------------------------------------------

/// Find a submodule by a `::`-separated path of names under `root`.
/// Returns `None` if any segment is missing. Used to navigate the
/// synthetic `core::option::Option` / `core::ops::Add` chains the
/// cross-crate extraction creates.
fn find_module_by_path<'a>(model: &'a RegseqModel, path: &[&str]) -> Option<&'a Module> {
    let mut current = &model.root_module;
    for seg in path {
        let next = current
            .submodules
            .iter()
            .find(|m| model.symbols.resolve(m.name) == *seg)?;
        current = next;
    }
    Some(current)
}

/// Find a free function by name in a module.
fn find_free_fn_by_name<'a>(
    model: &RegseqModel,
    module: &'a Module,
    name: &str,
) -> Option<&'a Function> {
    module
        .free_functions
        .iter()
        .find(|f| model.symbols.resolve(f.name) == name)
}

/// Total count of Function entries in the model (free functions and
/// methods under every impl-block-like collection, recursively).
fn count_functions(m: &Module) -> usize {
    let mut total = m.free_functions.len();
    total += m.impl_blocks.iter().map(|b| b.methods.len()).sum::<usize>();
    total += m.trait_impls.iter().map(|b| b.methods.len()).sum::<usize>();
    total += m
        .driver_impl_blocks
        .iter()
        .map(|b| b.methods.len())
        .sum::<usize>();
    for sub in &m.submodules {
        total += count_functions(sub);
    }
    total
}

// ---------------------------------------------------------------------------
// Positive: Option::map and Option::unwrap_or
// ---------------------------------------------------------------------------

const OPTION_METHODS_SPECIMEN: &str = r#"
#![no_std]

pub struct Clock;
pub struct Mmio<T> { _m: core::marker::PhantomData<T> }

#[allow(non_snake_case)]
pub mod DR_REG { pub struct Register; }
pub struct ReadWrite<T, R> { _v: T, _r: core::marker::PhantomData<R> }

#[repr(C)]
#[rseq::register_block(endian = "little", access_width = 32)]
pub struct Block { pub dr: ReadWrite<u32, DR_REG::Register> }

#[rseq::driver(name = "spec-drv", compatible = ["acme,widget"], bus = "amba")]
pub struct Drv {
    #[rseq::mmio(id = "regs")]
    pub regs: Mmio<Block>,
    #[rseq::clock(id = "clk")]
    pub clk: Clock,
}

impl Drv {
    pub fn option_chain(&self, x: Option<u32>) -> u32 {
        x.map(|v| v + 1).unwrap_or(0)
    }
}
"#;

#[test]
fn option_map_and_unwrap_or_are_registered_as_cross_crate_functions() {
    let pass1 = compile_and_build(OPTION_METHODS_SPECIMEN, "cross_crate_option_methods")
        .expect("Pass 1 must succeed on a specimen that only uses `core` cross-crate callees");

    // (1) A synthetic `core` submodule must exist at the root.
    let core_module = find_module_by_path(&pass1.model, &["core"])
        .expect("synthetic `core` module must be created for Option::map/Option::unwrap_or");
    assert!(
        matches!(core_module.kind, regseq_model::model::ModuleKind::Normal),
        "synthetic module kind must be Normal",
    );

    // (2) Navigate down to `core::option::Option`. The exact depth
    // depends on def_path_str canonicalization; walk the chain so
    // the test survives minor formatting differences.
    let option_module = find_module_by_path(&pass1.model, &["core", "option", "Option"])
        .expect("synthetic `core::option::Option` chain must be materialized");

    // (3) `map` and `unwrap_or` must be registered under
    // `core::option::Option` as free functions with non-empty
    // bodies (the placeholder body carries one `Unreachable` block
    // -- that's still "non-empty" structurally; it exists to be
    // replaced by Pass 2).
    let map_fn = find_free_fn_by_name(&pass1.model, option_module, "map")
        .expect("Option::map must be a free function under `core::option::Option`");
    let unwrap_or_fn = find_free_fn_by_name(&pass1.model, option_module, "unwrap_or")
        .expect("Option::unwrap_or must be a free function under `core::option::Option`");

    assert!(
        !map_fn.body.basic_blocks.is_empty(),
        "Option::map body must have at least one basic block (placeholder or translated)",
    );
    assert!(
        !unwrap_or_fn.body.basic_blocks.is_empty(),
        "Option::unwrap_or body must have at least one basic block",
    );

    // (4) Each cross-crate function must appear in cross_refs.functions
    // so Pass 2's FunctionIndex can resolve it without re-walking
    // the HIR.
    assert!(
        pass1
            .cross_refs
            .functions
            .values()
            .any(|id| *id == map_fn.id),
        "Option::map FunctionId must be in cross_refs.functions",
    );
    assert!(
        pass1
            .cross_refs
            .functions
            .values()
            .any(|id| *id == unwrap_or_fn.id),
        "Option::unwrap_or FunctionId must be in cross_refs.functions",
    );

    // (5) Each cross-crate function's DeclId must have a
    // SourceOrigin entry resolved from `tcx.def_span(def_id)`
    // (Source(...) -- the task spec's contract).
    let map_origin = pass1
        .model
        .source_locations
        .get(&map_fn.id.decl_id())
        .expect("Option::map must have a SourceOrigin entry");
    assert!(
        matches!(map_origin, regseq_model::identity::SourceOrigin::Source(_)),
        "cross-crate Option::map span must resolve to Source(..) (got {map_origin:?})",
    );
    let unwrap_origin = pass1
        .model
        .source_locations
        .get(&unwrap_or_fn.id.decl_id())
        .expect("Option::unwrap_or must have a SourceOrigin entry");
    assert!(
        matches!(
            unwrap_origin,
            regseq_model::identity::SourceOrigin::Source(_)
        ),
        "cross-crate Option::unwrap_or span must resolve to Source(..) (got {unwrap_origin:?})",
    );

    // (6) The synthetic `core::option::Option` module chain
    // itself has `SourceOrigin::CompilerGenerated` per the spec.
    let option_module_origin = pass1
        .model
        .source_locations
        .get(&option_module.id.decl_id())
        .expect("synthetic module must have a SourceOrigin entry");
    assert!(
        matches!(
            option_module_origin,
            regseq_model::identity::SourceOrigin::CompilerGenerated
        ),
        "synthetic module must carry CompilerGenerated origin (got {option_module_origin:?})",
    );
}

// ---------------------------------------------------------------------------
// Positive: reusing synthetic module chains across multiple callees
// ---------------------------------------------------------------------------

/// Two callees under the same synthetic parent (e.g., `Option::map`
/// and `Option::unwrap_or`) must share one synthetic `Option` module
/// node, not two. This is the deterministic placement contract from
/// the spec's Implementation Notes: "Reuse existing synthetic module
/// entries by interned name path."
#[test]
fn synthetic_module_chain_is_reused_for_sibling_callees() {
    let pass1 = compile_and_build(OPTION_METHODS_SPECIMEN, "cross_crate_shared_chain")
        .expect("Pass 1 must succeed");

    let core_module =
        find_module_by_path(&pass1.model, &["core"]).expect("synthetic `core` module must exist");
    // Exactly one `option` child.
    let option_count = core_module
        .submodules
        .iter()
        .filter(|m| pass1.model.symbols.resolve(m.name) == "option")
        .count();
    assert_eq!(
        option_count, 1,
        "two core::option::* callees must share one synthetic `core::option` module",
    );

    let option_module =
        find_module_by_path(&pass1.model, &["core", "option"]).expect("`core::option` must exist");
    let option_type_count = option_module
        .submodules
        .iter()
        .filter(|m| pass1.model.symbols.resolve(m.name) == "Option")
        .count();
    assert_eq!(
        option_type_count, 1,
        "two Option::* callees must share one synthetic `core::option::Option` module",
    );
}

// ---------------------------------------------------------------------------
// Negative: std-crate references are rejected
// ---------------------------------------------------------------------------

const STD_CALL_SPECIMEN: &str = r#"
pub struct Clock;
pub struct Mmio<T> { _m: core::marker::PhantomData<T> }

#[allow(non_snake_case)]
pub mod DR_REG { pub struct Register; }
pub struct ReadWrite<T, R> { _v: T, _r: core::marker::PhantomData<R> }

#[repr(C)]
#[rseq::register_block(endian = "little", access_width = 32)]
pub struct Block { pub dr: ReadWrite<u32, DR_REG::Register> }

#[rseq::driver(name = "spec-drv", compatible = ["acme,widget"], bus = "amba")]
pub struct Drv {
    #[rseq::mmio(id = "regs")]
    pub regs: Mmio<Block>,
    #[rseq::clock(id = "clk")]
    pub clk: Clock,
}

impl Drv {
    pub fn uses_std_vec(&self) {
        // `push` is a method on `std::vec::Vec<T>` -- a std-crate
        // item that the Rule 3 walker must reject. Using it from
        // an authored method body triggers
        // `ConstructionError::StdFunctionNotPermitted` at Pass 1.
        let mut v: std::vec::Vec<u8> = std::vec::Vec::new();
        v.push(0);
    }
}
"#;

#[test]
fn std_function_call_is_rejected_by_rule_3() {
    let outcome = compile_and_build(STD_CALL_SPECIMEN, "cross_crate_std_rejection");
    match outcome {
        Err(ConstructionError::StdFunctionNotPermitted { path, caller, .. }) => {
            // `def_path_str` canonicalizes paths, so the rendered
            // form for `Vec::push` may be `std::vec::Vec::<u8>::push`
            // or `alloc::vec::Vec::<u8>::push` depending on the
            // toolchain's re-export posture. Either way, the
            // caller must be the authored method.
            assert!(
                path.contains("push") || path.contains("new") || path.contains("Vec"),
                "rejected path must mention a std/alloc Vec API: got `{path}`",
            );
            assert!(
                caller.contains("uses_std_vec"),
                "caller must identify the authored method: got `{caller}`",
            );
        }
        Err(other) => panic!("expected StdFunctionNotPermitted, got different error: {other:?}",),
        Ok(_) => panic!("Pass 1 must not succeed on a specimen that calls std functions"),
    }
}

// ---------------------------------------------------------------------------
// Positive: a specimen that calls no cross-crate function does not
// materialize any synthetic module
// ---------------------------------------------------------------------------

const NO_CROSS_CRATE_SPECIMEN: &str = r#"
#![no_std]

pub struct Clock;
pub struct Mmio<T> { _m: core::marker::PhantomData<T> }

#[allow(non_snake_case)]
pub mod DR_REG { pub struct Register; }
pub struct ReadWrite<T, R> { _v: T, _r: core::marker::PhantomData<R> }

#[repr(C)]
#[rseq::register_block(endian = "little", access_width = 32)]
pub struct Block { pub dr: ReadWrite<u32, DR_REG::Register> }

#[rseq::driver(name = "spec-drv", compatible = ["acme,widget"], bus = "amba")]
pub struct Drv {
    #[rseq::mmio(id = "regs")]
    pub regs: Mmio<Block>,
    #[rseq::clock(id = "clk")]
    pub clk: Clock,
}

impl Drv {
    pub fn pure_arithmetic(&self, x: u32) -> u32 {
        x + 1
    }
}
"#;

#[test]
fn no_synthetic_module_when_no_cross_crate_call_is_made() {
    let pass1 = compile_and_build(NO_CROSS_CRATE_SPECIMEN, "cross_crate_none_needed")
        .expect("Pass 1 must succeed on a specimen with no cross-crate calls");

    // Root module's submodules must not include a synthetic `core`
    // entry. A pure-arithmetic body has no `Call` terminators, so
    // the cross-crate walker has nothing to register.
    let has_core_submodule = pass1
        .model
        .root_module
        .submodules
        .iter()
        .any(|m| pass1.model.symbols.resolve(m.name) == "core");
    assert!(
        !has_core_submodule,
        "no synthetic `core` module must appear when no cross-crate call is made",
    );
}

// ---------------------------------------------------------------------------
// Extraction is idempotent: running Pass 1 twice on the same source
// produces a structurally equivalent cross-crate function set.
// ---------------------------------------------------------------------------

#[test]
fn cross_crate_extraction_is_deterministic_across_runs() {
    let pass1_a = compile_and_build(OPTION_METHODS_SPECIMEN, "cross_crate_determinism_a")
        .expect("Pass 1 run A must succeed");
    let pass1_b = compile_and_build(OPTION_METHODS_SPECIMEN, "cross_crate_determinism_b")
        .expect("Pass 1 run B must succeed");

    // Total function count must match. The cross-crate walker
    // processes the worklist in a stable order (BFS from an
    // iterator of the seed ids; even if that ordering is
    // implementation-defined, total count is a stable invariant).
    let count_a = count_functions(&pass1_a.model.root_module);
    let count_b = count_functions(&pass1_b.model.root_module);
    assert_eq!(
        count_a, count_b,
        "two independent Pass 1 runs on the same source must register the same total function count",
    );
}

// ---------------------------------------------------------------------------
// Harness with `TyCtxt` access: required to drive Pass 2 `build_body`
// against an extracted cross-crate `DefId`.
// ---------------------------------------------------------------------------

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

    type CallbackCell<R> =
        Arc<Mutex<Option<Box<dyn FnOnce(TyCtxt<'_>, &Pass1Output) -> R + Send>>>>;
    let out: Arc<Mutex<Option<R>>> = Arc::new(Mutex::new(None));
    let callback_cell: CallbackCell<R> = Arc::new(Mutex::new(Some(Box::new(f))));

    struct Callbacks<R: Send + 'static> {
        out: Arc<Mutex<Option<R>>>,
        callback: CallbackCell<R>,
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
            let pass1 = build_pass1_output(tcx, &registries).expect("Pass 1 must succeed");
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
// Trait method resolution via `Instance::resolve`: a facade-crate-like
// `<LocalTy as core::ops::Add>::add` must be registered against the
// impl method's `DefId`, not the core trait method's `DefId`.
// ---------------------------------------------------------------------------
//
// Our specimen here plays both the "facade" and the authored-driver
// roles (there is no second crate in the test harness). The
// `impl Add for FieldValue` is still a local-crate method whose
// resolved `DefId` lives in LOCAL_CRATE, so `Instance::resolve`
// sends the walker through the short-circuit "already-registered
// local function" path. The invariant we assert here is that the
// fallback resolution does not produce an error, and that no
// synthetic module named after the trait (`Add`) ends up under
// `core::ops` -- the trait method's `DefId` must NOT be mistakenly
// treated as the call target.

const TRAIT_IMPL_SPECIMEN: &str = r#"
#![no_std]

use core::ops::Add;

pub struct Clock;
pub struct Mmio<T> { _m: core::marker::PhantomData<T> }

#[allow(non_snake_case)]
pub mod DR_REG { pub struct Register; }
pub struct ReadWrite<T, R> { _v: T, _r: core::marker::PhantomData<R> }

#[repr(C)]
#[rseq::register_block(endian = "little", access_width = 32)]
pub struct Block { pub dr: ReadWrite<u32, DR_REG::Register> }

#[rseq::driver(name = "spec-drv", compatible = ["acme,widget"], bus = "amba")]
pub struct Drv {
    #[rseq::mmio(id = "regs")]
    pub regs: Mmio<Block>,
    #[rseq::clock(id = "clk")]
    pub clk: Clock,
}

#[derive(Copy, Clone)]
pub struct FieldValue(pub u32);

impl Add for FieldValue {
    type Output = FieldValue;
    fn add(self, other: FieldValue) -> FieldValue {
        FieldValue(self.0 + other.0)
    }
}

impl Drv {
    pub fn combines_field_values(&self, x: FieldValue, y: FieldValue) -> FieldValue {
        // Trait-method call: MIR at `mir_opt_level=0` emits
        // `FnDef(<FieldValue as Add>::add, [...])`. Rule 3's
        // `Instance::resolve` must route to the local impl method,
        // not the core trait method.
        x + y
    }
}
"#;

#[test]
fn trait_method_resolves_to_local_impl_not_core_trait_def() {
    let pass1 = compile_and_build(TRAIT_IMPL_SPECIMEN, "cross_crate_trait_resolution")
        .expect("Pass 1 must succeed on a specimen with a local `Add` impl");

    // After resolution, the impl method lives in LOCAL_CRATE and
    // is a regular local function in the Pass 1 walk. Therefore
    // no synthetic `core::ops::Add` or `core::ops` module should
    // exist, because `Instance::resolve` replaces the core trait
    // method's DefId with the local impl's DefId before the
    // walker registers anything.
    let ops_module = find_module_by_path(&pass1.model, &["core", "ops"]);
    assert!(
        ops_module.is_none()
            || ops_module
                .unwrap()
                .submodules
                .iter()
                .all(|m| pass1.model.symbols.resolve(m.name) != "Add"),
        "`Instance::resolve` must route `<FieldValue as Add>::add` to the local impl, so no `core::ops::Add` synthetic module is needed",
    );
}

// ---------------------------------------------------------------------------
// Pass 2 uniformity: `build_body` translates a cross-crate function's
// MIR the same way it translates an authored body.
// ---------------------------------------------------------------------------

#[test]
fn pass2_build_body_does_not_silently_pass_through_a_placeholder() {
    // Pass 2's `build_body` must handle cross-crate `DefId`s
    // through the exact same entry point as authored bodies --
    // no special-casing. A "uniform" driver cannot silently
    // return an unchanged Pass 1 placeholder body; it must
    // either translate the MIR (yielding a structurally
    // different body) or surface a [`ConstructionError`].
    //
    // Core library generic function MIR (e.g., the body of
    // `core::option::Option::<T>::map`) contains `TyKind::Param`
    // types that Pass 2's `map_ty` legitimately rejects via
    // `ConstructionError::UnsupportedRustType`. That Err is
    // proof the cross-crate `DefId` went through the real
    // translation pipeline: a silent skip would have returned
    // `Ok(placeholder)` instead. This test fails ONLY if a
    // cross-crate DefId's `build_body` call returns an
    // `Ok(placeholder)` -- the exact failure mode a non-uniform
    // "skip cross-crate" branch would introduce.
    let (has_cross_crate_candidate, saw_placeholder_ok, saw_real_outcome) = with_specimen(
        OPTION_METHODS_SPECIMEN,
        "cross_crate_pass2_uniform",
        |tcx, pass1| {
            let cross_crate_def_ids: Vec<DefId> = pass1
                .cross_refs
                .functions
                .keys()
                .copied()
                .filter(|d| d.krate != rustc_hir::def_id::LOCAL_CRATE)
                .collect();

            if cross_crate_def_ids.is_empty() {
                return (false, false, false);
            }

            let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
            let constants = ConstantIndex::new(&pass1.cross_refs.constants);
            let functions = FunctionIndex::new(&pass1.cross_refs.functions);
            let intrinsics = IntrinsicLookup::default();
            let abstraction_registry = AbstractionTypeRegistry::default();
            let deref_lookup = DerefCollapseLookup::build(tcx, &abstraction_registry)
                .expect("Deref lang item must be resolvable");
            let ordered_markers = OrderedMarkers::default();
            let try_markers = TryMarkers::default();

            let mut placeholder_ok_seen = false;
            let mut real_outcome_seen = false;
            for def_id in &cross_crate_def_ids {
                if !tcx.is_mir_available(*def_id) {
                    continue;
                }
                let fn_id = *pass1.cross_refs.functions.get(def_id).unwrap();
                let mut interner = SymbolInterner::new();
                let body_result = build_body(
                    tcx,
                    *def_id,
                    fn_id,
                    &type_def_index,
                    &constants,
                    &functions,
                    &intrinsics,
                    &deref_lookup,
                    &ordered_markers,
                    &try_markers,
                    &mut interner,
                );
                match body_result {
                    Ok(body) => {
                        let is_placeholder = body.basic_blocks.len() == 1
                            && body.basic_blocks[0].statements.is_empty()
                            && matches!(
                                body.basic_blocks[0].terminator.kind,
                                regseq_model::ir::TerminatorKind::Unreachable
                            );
                        if is_placeholder {
                            placeholder_ok_seen = true;
                        } else {
                            real_outcome_seen = true;
                        }
                    }
                    Err(_) => {
                        // A `ConstructionError` is proof the
                        // cross-crate DefId went through the
                        // real translation pipeline (the error
                        // is raised by `map_ty` / `map_place` /
                        // `translate_terminator`, not by a
                        // cross-crate bypass).
                        real_outcome_seen = true;
                    }
                }
            }

            (true, placeholder_ok_seen, real_outcome_seen)
        },
    );

    assert!(
        has_cross_crate_candidate,
        "OPTION_METHODS_SPECIMEN must pull in at least one cross-crate function via Option::map / unwrap_or",
    );
    assert!(
        !saw_placeholder_ok,
        "Pass 2 `build_body` must not silently return the Pass 1 placeholder body for a cross-crate DefId",
    );
    assert!(
        saw_real_outcome,
        "at least one cross-crate DefId must produce a real `build_body` outcome (translated body or ConstructionError)",
    );
}

// ---------------------------------------------------------------------------
// Depth guard: exercise the runtime
// `ConstructionError::CrossCrateRecursionLimit` path.
// ---------------------------------------------------------------------------
//
// Synthesizing a 17-deep transitive cross-crate call chain through
// actual `core` MIR is infeasible in a stand-alone specimen:
// `core`'s internal call depth for the methods a driver spec
// reaches is fixed and shallow. We exercise the depth guard
// through the `#[doc(hidden)]` test-only entry point
// `build_pass1_output_with_cross_crate_depth`, which accepts a
// custom `max_cross_crate_depth`. With `max=0`, a single
// cross-crate edge enqueues a callee at depth 1, which exceeds
// the bound on the next worklist iteration -- the runtime error
// path fires, proving the guard is not dead code.

#[test]
fn cross_crate_recursion_limit_raises_when_depth_exceeds_bound() {
    // Any specimen that reaches at least one allowlisted
    // cross-crate callee will do: the shared `OPTION_METHODS_SPECIMEN`
    // calls `Option::map` and `Option::unwrap_or`, so the worklist
    // grows to depth >= 1 after processing the seeded authored
    // body.
    let outcome =
        compile_and_build_with_depth(OPTION_METHODS_SPECIMEN, "cross_crate_depth_guard_fires", 0);
    match outcome {
        Err(ConstructionError::CrossCrateRecursionLimit { depth, .. }) => {
            assert!(
                depth > 0,
                "recorded overflow depth must be strictly greater than the bound (got {depth})",
            );
        }
        Err(other) => panic!(
            "expected CrossCrateRecursionLimit with depth > 0, got different error: {other:?}",
        ),
        Ok(_) => {
            panic!("Pass 1 must surface CrossCrateRecursionLimit when the depth bound is set to 0",)
        }
    }
}

/// Run Pass 1 with a custom cross-crate depth bound. Wraps the
/// `#[doc(hidden)]` test-only entry point so the depth-guard test
/// can exercise the runtime error path.
fn compile_and_build_with_depth(source: &str, test_name: &str, max_depth: u32) -> Outcome {
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

    let result: Arc<Mutex<Option<Outcome>>> = Arc::new(Mutex::new(None));
    let depth_cell: Arc<Mutex<u32>> = Arc::new(Mutex::new(max_depth));

    struct Callbacks {
        result: Arc<Mutex<Option<Outcome>>>,
        depth: Arc<Mutex<u32>>,
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
            let depth = *self.depth.lock().unwrap();
            let outcome = build_pass1_output_with_cross_crate_depth(tcx, &registries, depth);
            *self.result.lock().unwrap() = Some(outcome);
            rustc_driver::Compilation::Stop
        }
    }

    let mut callbacks = Callbacks {
        result: result.clone(),
        depth: depth_cell,
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

    let outcome = result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis must run");

    std::fs::remove_dir_all(&test_dir).ok();

    outcome
}

// ---------------------------------------------------------------------------
// Regseq-facade cross-crate coverage: verify that calls reaching the
// `regseq` facade crate surface as cross-crate functions under a
// synthetic `regseq::*` module chain.
// ---------------------------------------------------------------------------
//
// Compiles a tiny specimen against the real `regseq` and
// `regseq-macros` workspace members (mirroring the pattern used by
// `intrinsic_registry.rs` and the PL011 full-spec harness). The
// specimen's authored method invokes `Mmio::<T>::read`, which is a
// facade-method intrinsic; Rule 3 must place its `DefId` under a
// synthetic `regseq::*` module with a non-empty body and a
// `Source(..)` origin, and the function must be present in
// `cross_refs.functions`.

use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

#[derive(Debug, Clone)]
struct FacadeArtifacts {
    regseq_rlib: PathBuf,
    regseq_macros_dylib: PathBuf,
    deps_dir: PathBuf,
}

fn build_facade_deps() -> FacadeArtifacts {
    let root = workspace_root();
    let output = std::process::Command::new("cargo")
        .args([
            "build",
            "-p",
            "regseq",
            "-p",
            "regseq-macros",
            "--message-format=json",
        ])
        .current_dir(&root)
        .output()
        .expect("failed to run cargo build for regseq deps");

    assert!(
        output.status.success(),
        "cargo build for regseq deps failed:\n{}",
        String::from_utf8_lossy(&output.stderr),
    );

    let stdout = String::from_utf8(output.stdout).expect("cargo output is valid UTF-8");
    let mut regseq_rlib: Option<PathBuf> = None;
    let mut regseq_macros_dylib: Option<PathBuf> = None;

    for line in stdout.lines() {
        let Ok(msg) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if msg.get("reason").and_then(|v| v.as_str()) != Some("compiler-artifact") {
            continue;
        }
        let Some(target) = msg.get("target") else {
            continue;
        };
        let Some(name) = target.get("name").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(filenames) = msg.get("filenames").and_then(|v| v.as_array()) else {
            continue;
        };
        match name {
            "regseq" => {
                for f in filenames {
                    let p = f.as_str().unwrap_or("");
                    if p.ends_with(".rlib") {
                        regseq_rlib = Some(PathBuf::from(p));
                        break;
                    }
                }
            }
            "regseq-macros" | "regseq_macros" => {
                for f in filenames {
                    let p = f.as_str().unwrap_or("");
                    if p.ends_with(".dylib") || p.ends_with(".so") || p.ends_with(".dll") {
                        regseq_macros_dylib = Some(PathBuf::from(p));
                        break;
                    }
                }
            }
            _ => {}
        }
    }

    let regseq_rlib = regseq_rlib.expect("regseq rlib not found in cargo build output");
    let regseq_macros_dylib =
        regseq_macros_dylib.expect("regseq-macros dylib not found in cargo build output");
    let deps_dir = regseq_rlib
        .parent()
        .expect("regseq rlib path must have a parent")
        .to_path_buf();
    FacadeArtifacts {
        regseq_rlib,
        regseq_macros_dylib,
        deps_dir,
    }
}

fn facade_artifacts() -> &'static FacadeArtifacts {
    use std::sync::OnceLock;
    static A: OnceLock<FacadeArtifacts> = OnceLock::new();
    A.get_or_init(build_facade_deps)
}

const REGSEQ_FACADE_SPECIMEN: &str = r#"
#![no_std]

extern crate regseq;

#[rseq::driver(name = "spec-drv", compatible = ["acme,widget"], bus = "amba")]
pub struct Drv {
    #[rseq::clock(id = "clk")]
    pub clk: regseq::Clock,
}

impl Drv {
    pub fn disable_clock(&self) {
        // `regseq::Clock::disable` is an authored method on the
        // real `regseq` facade crate. A `Call` to it in authored
        // MIR forces Rule 3 to register the facade's method
        // `DefId` as a cross-crate function under synthetic
        // `regseq::*`.
        self.clk.disable();
    }
}
"#;

fn compile_and_build_against_facade(source: &str, test_name: &str) -> Outcome {
    let artifacts = facade_artifacts();
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

    let result: Arc<Mutex<Option<Outcome>>> = Arc::new(Mutex::new(None));

    struct Callbacks {
        result: Arc<Mutex<Option<Outcome>>>,
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
            let outcome = build_pass1_output(tcx, &registries);
            *self.result.lock().unwrap() = Some(outcome);
            rustc_driver::Compilation::Stop
        }
    }

    let mut callbacks = Callbacks {
        result: result.clone(),
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
        "--extern".to_string(),
        format!("regseq={}", artifacts.regseq_rlib.display()),
        "--extern".to_string(),
        format!("regseq_macros={}", artifacts.regseq_macros_dylib.display()),
        "-L".to_string(),
        format!("dependency={}", artifacts.deps_dir.display()),
        "-Zmir-opt-level=0".to_string(),
        "-Coverflow-checks=off".to_string(),
        "-Cdebug-assertions=off".to_string(),
    ];

    rustc_driver::run_compiler(&args, &mut callbacks);

    let outcome = result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis must run");

    std::fs::remove_dir_all(&test_dir).ok();

    outcome
}

#[test]
fn regseq_facade_method_call_is_registered_under_synthetic_regseq_chain() {
    // Compiles a minimal specimen against the real `regseq` facade
    // crate. The specimen's authored method calls `Clock::disable`
    // -- a facade method whose `DefId` lives in the `regseq`
    // crate. Rule 3 must place it under a synthetic `regseq::*`
    // module with a non-empty body, a `Source(..)` origin, and an
    // entry in `cross_refs.functions`. This covers the task
    // spec's "required cross-crate function under synthetic
    // `regseq::*`" contract for facade-side callees without
    // depending on the full PL011 fixture (which carries a
    // separate register-block resolution pre-requirement tracked
    // under a different task).
    let pass1 = compile_and_build_against_facade(
        REGSEQ_FACADE_SPECIMEN,
        "cross_crate_regseq_facade_method",
    )
    .expect(
        "Pass 1 must succeed on a specimen that calls a `regseq` facade method against the real facade crate",
    );

    // A synthetic `regseq` module must exist under the root.
    let regseq_module = find_module_by_path(&pass1.model, &["regseq"])
        .expect("synthetic `regseq` module must exist once a facade method is referenced");
    assert!(
        matches!(regseq_module.kind, regseq_model::model::ModuleKind::Normal),
        "synthetic `regseq` module kind must be Normal",
    );

    // Its SourceOrigin must be CompilerGenerated (the synthetic
    // module itself has no authored source).
    let regseq_module_origin = pass1
        .model
        .source_locations
        .get(&regseq_module.id.decl_id())
        .expect("synthetic `regseq` module must have a SourceOrigin entry");
    assert!(
        matches!(
            regseq_module_origin,
            regseq_model::identity::SourceOrigin::CompilerGenerated
        ),
        "synthetic `regseq` module must carry CompilerGenerated origin (got {regseq_module_origin:?})",
    );

    // Walk the synthetic tree looking for a function whose name
    // matches a facade-method identifier. `Clock::disable` is the
    // concrete callee in the specimen; we look for any function
    // whose interned name contains `disable` under `regseq::*` so
    // the test survives def_path_str rendering differences.
    let facade_fn = walk_free_functions(regseq_module)
        .find(|f| pass1.model.symbols.resolve(f.name) == "disable")
        .expect("Rule 3 must register the `regseq::Clock::disable` DefId under `regseq::*`");

    // Its body must have at least one basic block (the Pass 1
    // placeholder structure).
    assert!(
        !facade_fn.body.basic_blocks.is_empty(),
        "facade method body must be non-empty (placeholder or translated)",
    );

    // Its source_locations entry must resolve to `Source(..)`.
    let origin = pass1
        .model
        .source_locations
        .get(&facade_fn.id.decl_id())
        .expect("facade method must have a SourceOrigin entry");
    assert!(
        matches!(origin, regseq_model::identity::SourceOrigin::Source(_)),
        "facade method span must resolve to Source(..) (got {origin:?})",
    );

    // Must be present in cross_refs.functions for Pass 2 lookup.
    assert!(
        pass1
            .cross_refs
            .functions
            .values()
            .any(|id| *id == facade_fn.id),
        "facade method FunctionId must be in cross_refs.functions",
    );
}

/// Recursively iterate every free function under a module tree.
/// Used by the facade test to locate a cross-crate function by
/// name regardless of exact synthetic module placement.
fn walk_free_functions(m: &Module) -> Box<dyn Iterator<Item = &Function> + '_> {
    let top = m.free_functions.iter();
    let subs = m.submodules.iter().flat_map(walk_free_functions);
    Box::new(top.chain(subs))
}
