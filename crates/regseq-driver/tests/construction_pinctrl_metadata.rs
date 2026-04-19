//! Pass 2 PinCtrl metadata resolution tests (regseq-model-52).
//!
//! These tests drive `build_body` against inline specimens that
//! declare a local `PinCtrl` abstraction, local pin-state marker
//! types annotated with `#[rseq::pin_state(id = "...")]`, and
//! helper functions that invoke `PinCtrl::apply::<State>()`. The
//! tests assert that the resulting `PinCtrlApply` `IntrinsicCall`
//! terminator carries
//! `IntrinsicMetadata::PinCtrl { state_id }` where `state_id`
//! resolves to the interned `Symbol` matching the marker's `id`
//! attribute, and that a marker without `#[rseq::pin_state]`
//! trips `ConstructionError::MissingPinStateAttribute`.

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
use regseq_model::identity::SymbolInterner;
use regseq_model::ir::{IntrinsicMetadata, RegseqIntrinsic, TerminatorKind};

// ---------------------------------------------------------------------------
// rustc harness (mirror of construction_register_metadata.rs)
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
// HIR helpers
// ---------------------------------------------------------------------------

fn find_impl_fn_def_id<'tcx>(tcx: TyCtxt<'tcx>, self_ty: &str, method: &str) -> DefId {
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
    panic!("free function `{name}` not found in specimen");
}

// ---------------------------------------------------------------------------
// Specimen. The facade `PinCtrl::apply<S: PinState>` is emulated by
// hand so the test doesn't need to pull in the real facade crate.
// Intrinsic recognition is seeded via `IntrinsicLookup::from_entries`
// against the local `apply` method DefId (no `#[rseq::intrinsic]`
// attribute on the specimen -- that attribute is reserved for the
// facade's compiler-managed surface).
// ---------------------------------------------------------------------------

const PINCTRL_SPECIMEN: &str = r#"
#![no_std]

pub trait PinState {}

pub struct PinCtrl {
    _opaque: (),
}

impl PinCtrl {
    pub fn apply<S: PinState>(&self) {
        loop {}
    }
}

#[rseq::pin_state(id = "default")]
pub struct DefaultState;
impl PinState for DefaultState {}

#[rseq::pin_state(id = "sleep")]
pub struct SleepState;
impl PinState for SleepState {}

pub fn apply_default_pins(pinctrl: &PinCtrl) {
    pinctrl.apply::<DefaultState>()
}

pub fn apply_sleep_pins(pinctrl: &PinCtrl) {
    pinctrl.apply::<SleepState>()
}

#[rseq::driver(name = "pinctrl-smoke")]
pub struct Drv {}
"#;

// ---------------------------------------------------------------------------
// Specimen with a pin-state marker that lacks `#[rseq::pin_state]`.
// Translating `apply_foo` must trip `MissingPinStateAttribute`.
// ---------------------------------------------------------------------------

const MISSING_ATTR_SPECIMEN: &str = r#"
#![no_std]

pub trait PinState {}

pub struct PinCtrl {
    _opaque: (),
}

impl PinCtrl {
    pub fn apply<S: PinState>(&self) {
        loop {}
    }
}

// Deliberately no `#[rseq::pin_state]`.
pub struct FooState;
impl PinState for FooState {}

pub fn apply_foo(pinctrl: &PinCtrl) {
    pinctrl.apply::<FooState>()
}

#[rseq::driver(name = "pinctrl-missing")]
pub struct Drv {}
"#;

// ---------------------------------------------------------------------------
// Translation helper
// ---------------------------------------------------------------------------

struct ScenarioCtx<'tcx> {
    tcx: TyCtxt<'tcx>,
    intrinsic_lookup: IntrinsicLookup,
    abstraction_registry: AbstractionTypeRegistry,
}

fn build_scenario<'tcx>(tcx: TyCtxt<'tcx>) -> ScenarioCtx<'tcx> {
    let apply = find_impl_fn_def_id(tcx, "PinCtrl", "apply");
    let lookup = IntrinsicLookup::from_entries(tcx, [(apply, "pinctrl_apply")].into_iter())
        .expect("canonical name must parse");
    ScenarioCtx {
        tcx,
        intrinsic_lookup: lookup,
        abstraction_registry: AbstractionTypeRegistry::default(),
    }
}

/// Translate `method_def_id`'s body and return every `PinCtrlApply`
/// IntrinsicCall's metadata, captured as `(intrinsic, state_id_text)`.
fn translate_pinctrl_calls<'tcx>(
    ctx: &ScenarioCtx<'tcx>,
    pass1: &Pass1Output,
    method_def_id: DefId,
) -> Result<Vec<(RegseqIntrinsic, String)>, ConstructionError> {
    let pass1_symbols = &pass1.model.symbols;
    let tcx = ctx.tcx;
    let function_id = *pass1
        .cross_refs
        .functions
        .get(&method_def_id)
        .unwrap_or_else(|| {
            panic!("method DefId {method_def_id:?} missing from cross_refs.functions")
        });
    let deref_lookup = DerefCollapseLookup::build(tcx, &ctx.abstraction_registry)
        .expect("Deref lang item resolves");
    let ordered_markers = OrderedMarkers::default();
    let try_markers = TryMarkers::default();
    let log_arg = LogArgUnwrap::default();
    let type_def_index = TypeDefIndex::new(&pass1.cross_refs.type_defs);
    let constants = ConstantIndex::new(&pass1.cross_refs.constants);
    let functions = FunctionIndex::new(&pass1.cross_refs.functions);
    let mut interner = SymbolInterner::new();
    let body = build_body(
        tcx,
        method_def_id,
        function_id,
        &type_def_index,
        &constants,
        &functions,
        &ctx.intrinsic_lookup,
        &deref_lookup,
        &ordered_markers,
        &try_markers,
        &log_arg,
        &RegisterIndex::default(),
        &pass1.cross_refs.register_blocks,
        &ctx.abstraction_registry,
        &pass1.cross_refs.pin_state_lookup,
        &mut interner,
    )?;

    let mut calls: Vec<(RegseqIntrinsic, String)> = Vec::new();
    for bb in &body.basic_blocks {
        let TerminatorKind::IntrinsicCall {
            intrinsic,
            metadata,
            ..
        } = &bb.terminator.kind
        else {
            continue;
        };
        let Some(IntrinsicMetadata::PinCtrl { state_id }) = metadata else {
            continue;
        };
        // `state_id` is interned in Pass 1's SymbolInterner, so resolve
        // it through the SAME interner that built the PinStateLookup.
        let text = pass1_symbols.resolve(*state_id).to_string();
        calls.push((*intrinsic, text));
    }
    Ok(calls)
}

// ---------------------------------------------------------------------------
// Pass 1 assertion: PinStateLookup populated from the module walk
// ---------------------------------------------------------------------------

#[test]
fn pass1_pin_state_lookup_is_populated() {
    let len = with_specimen(PINCTRL_SPECIMEN, "pcm_pass1_populated", |_tcx, pass1| {
        pass1.cross_refs.pin_state_lookup.len()
    });
    assert!(
        len >= 2,
        "PinStateLookup must contain entries for DefaultState and SleepState, got len={len}",
    );
}

// ---------------------------------------------------------------------------
// DefaultState / SleepState metadata resolution
// ---------------------------------------------------------------------------

#[test]
fn apply_default_pins_carries_default_state_id() {
    let calls = with_specimen(PINCTRL_SPECIMEN, "pcm_apply_default", |tcx, pass1| {
        let ctx = build_scenario(tcx);
        let did = find_free_fn_def_id(tcx, "apply_default_pins");
        translate_pinctrl_calls(&ctx, pass1, did).expect("apply_default_pins translation")
    });
    assert_eq!(
        calls.len(),
        1,
        "apply_default_pins must emit exactly one PinCtrlApply intrinsic call",
    );
    let (intrinsic, state_id) = &calls[0];
    assert_eq!(*intrinsic, RegseqIntrinsic::PinCtrlApply);
    assert_eq!(state_id, "default");
}

#[test]
fn apply_sleep_pins_carries_sleep_state_id() {
    let calls = with_specimen(PINCTRL_SPECIMEN, "pcm_apply_sleep", |tcx, pass1| {
        let ctx = build_scenario(tcx);
        let did = find_free_fn_def_id(tcx, "apply_sleep_pins");
        translate_pinctrl_calls(&ctx, pass1, did).expect("apply_sleep_pins translation")
    });
    assert_eq!(calls.len(), 1);
    let (intrinsic, state_id) = &calls[0];
    assert_eq!(*intrinsic, RegseqIntrinsic::PinCtrlApply);
    assert_eq!(state_id, "sleep");
}

// ---------------------------------------------------------------------------
// Negative: marker without `#[rseq::pin_state]` trips
// MissingPinStateAttribute
// ---------------------------------------------------------------------------

#[test]
fn missing_pin_state_attribute_raises_error() {
    let outcome: Result<(), String> =
        with_specimen(MISSING_ATTR_SPECIMEN, "pcm_missing_attr", |tcx, pass1| {
            let ctx = build_scenario(tcx);
            let did = find_free_fn_def_id(tcx, "apply_foo");
            match translate_pinctrl_calls(&ctx, pass1, did) {
                Ok(_) => Err("expected MissingPinStateAttribute, got Ok".to_string()),
                Err(ConstructionError::MissingPinStateAttribute { marker, .. }) => {
                    assert!(
                        marker.ends_with("FooState"),
                        "error marker path should end with `FooState`, got `{marker}`",
                    );
                    Ok(())
                }
                Err(other) => Err(format!("expected MissingPinStateAttribute, got {other:?}")),
            }
        });
    outcome.unwrap();
}
