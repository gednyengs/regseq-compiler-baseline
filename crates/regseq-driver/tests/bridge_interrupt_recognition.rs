//! Bridge tests for device-interrupt recognition + materialisation (T03).
//!
//! Covers: the smallest valid spec flowing whole into the model (R01/R03,
//! C04); all three declaration forms recognized, never narrowed (R02, I05);
//! the mandatory-id error (R01/D08); the bare-ISR exactly-one rule, the D13
//! handler-count discipline, and the C04 signature rejections; and the
//! `IrqResult` arm-reachability facts that drive R09/R11.

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
use regseq_model::{ArmReachability, AttrData, DriverInterrupts, InterruptForm, RegseqModel};
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
    std::env::temp_dir().join(format!("regseq-bridge-irq-{crate_name}-{nanos}"))
}

/// Run the full bridge over `source`; on success apply `f` to the model, on
/// rejection return the rendered `BridgeError` for diagnostic assertions.
fn with_model<R, F>(crate_name: &str, source: &str, f: F) -> Result<R, String>
where
    R: Send + 'static,
    F: FnOnce(&RegseqModel) -> R + Send + 'static,
{
    let _lock = COMPILER_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    init_rustc();

    let test_dir = fresh_test_dir(crate_name);
    std::fs::create_dir_all(&test_dir).unwrap();
    let src_path = test_dir.join("lib.rs");
    std::fs::write(&src_path, source).unwrap();

    let result: Arc<Mutex<Option<Result<R, String>>>> = Arc::new(Mutex::new(None));
    let callback = Arc::new(Mutex::new(Some(
        Box::new(f) as Box<dyn FnOnce(&RegseqModel) -> R + Send>
    )));

    struct Callbacks<R: Send + 'static> {
        result: Arc<Mutex<Option<Result<R, String>>>>,
        callback: Arc<Mutex<Option<Box<dyn FnOnce(&RegseqModel) -> R + Send>>>>,
    }

    impl<R: Send + 'static> rustc_driver::Callbacks for Callbacks<R> {
        fn config(&mut self, config: &mut rustc_interface::interface::Config) {
            config.opts.unstable_opts.no_codegen = true;
            config.opts.unstable_opts.mir_opt_level = Some(1);
            config
                .opts
                .unstable_opts
                .crate_attr
                .push("allow(unused, non_snake_case)".to_string());
        }

        fn after_analysis<'tcx>(
            &mut self,
            _compiler: &rustc_interface::interface::Compiler,
            tcx: TyCtxt<'tcx>,
        ) -> rustc_driver::Compilation {
            let outcome = match run_bridge(BridgeInput::new(tcx)) {
                Ok(model) => {
                    let f = self.callback.lock().unwrap().take().expect("callback once");
                    Ok(f(&model))
                }
                Err(err) => Err(err.to_string()),
            };
            *self.result.lock().unwrap() = Some(outcome);
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
        .expect("after_analysis should produce a result");
    std::fs::remove_dir_all(Path::new(&test_dir)).ok();
    output
}

/// The shared author surfaces (lifecycle trait + RegseqError; Interrupt +
/// IrqResult), included exactly as the fixtures include them.
const LIFECYCLE_SURFACE: &str =
    include_str!("../../codegen-coverage-runner/fixtures/_lifecycle_surface.rs");
const INTERRUPT_SURFACE: &str =
    include_str!("../../codegen-coverage-runner/fixtures/_interrupt_surface.rs");

/// Build a bridgeable crate around a driver/impl snippet.
fn source(driver: &str) -> String {
    format!(
        "#![feature(register_tool)]\n#![register_tool(rseq)]\n\
         {LIFECYCLE_SURFACE}\n{INTERRUPT_SURFACE}\n{driver}\npub fn entry() {{}}\n"
    )
}

/// The single recognized driver's interrupt tables.
fn sole_interrupts(model: &RegseqModel) -> DriverInterrupts {
    assert_eq!(model.interrupt.drivers.len(), 1, "exactly one irq driver");
    model.interrupt.drivers.values().next().unwrap().clone()
}

// --- The smallest valid spec (M1 vertical-slice shape) -------------------------

const SMALLEST: &str = r#"
#[rseq::driver(name = "uartish", compatible = ["acme,uartish"])]
pub struct Uartish {
    #[rseq::interrupt(id = "uart_irq")]
    pub irq: Interrupt,
}

impl Uartish {
    #[rseq::isr]
    pub fn handle_irq(&mut self, ) -> IrqResult {
        IrqResult::Handled
    }
}

pub fn make() -> Uartish { loop {} }
"#;

#[test]
fn smallest_valid_spec_materialises_resource_binding_and_reachability() {
    let (resources, handlers) = with_model("irq_smallest", &source(SMALLEST), |model| {
        let d = sole_interrupts(model);
        let resources: Vec<(String, String, InterruptForm)> = d
            .resources
            .iter()
            .map(|r| {
                (
                    model.strings.resolve(r.name).to_string(),
                    model.strings.resolve(r.id).to_string(),
                    r.form,
                )
            })
            .collect();
        let handlers: Vec<(String, u32, bool, bool)> = d
            .handlers
            .iter()
            .map(|h| {
                (
                    model.strings.resolve(h.method_name).to_string(),
                    h.resource,
                    h.array_bind,
                    h.body.is_some(),
                )
            })
            .collect();
        (resources, handlers)
    })
    .expect("bridge should accept the smallest valid spec");
    assert_eq!(
        resources,
        vec![(
            "irq".to_string(),
            "uart_irq".to_string(),
            InterruptForm::Single
        )]
    );
    assert_eq!(handlers, vec![("handle_irq".to_string(), 0, false, true)]);
}

#[test]
fn smallest_spec_records_field_and_item_attr_markers() {
    let (field_markers, isr_markers) = with_model("irq_markers", &source(SMALLEST), |model| {
        let field_markers: Vec<AttrData> = model
            .domain_attrs
            .field_attrs
            .values()
            .flatten()
            .filter(|a| matches!(a, AttrData::InterruptResource { .. }))
            .cloned()
            .collect();
        let isr_markers: usize = model
            .domain_attrs
            .item_attrs
            .iter()
            .flatten()
            .filter(|a| matches!(a, AttrData::IsrBinding { .. }))
            .count();
        (field_markers, isr_markers)
    })
    .expect("bridge should accept");
    assert_eq!(field_markers.len(), 1);
    assert!(matches!(
        field_markers[0],
        AttrData::InterruptResource {
            form: InterruptForm::Single,
            ..
        }
    ));
    assert_eq!(isr_markers, 1, "exactly one IsrBinding item marker");
}

#[test]
fn handler_arms_prove_constant_return_reachability() {
    // `handle_irq` returns only `Handled`: that arm must be proven reachable,
    // and the never-constructed `NotHandled`/`DeferWork` proven unreachable
    // (the load-bearing R09 fact: no false DeferWork rejection).
    let arms = with_model("irq_arms_const", &source(SMALLEST), |model| {
        sole_interrupts(model).handlers[0].arms
    })
    .expect("bridge should accept");
    assert_eq!(arms.handled, ArmReachability::ProvenReachable);
    assert_eq!(arms.not_handled, ArmReachability::ProvenUnreachable);
    assert_eq!(arms.defer_work, ArmReachability::ProvenUnreachable);
}

#[test]
fn handler_arms_branching_body_stays_unknown_never_proven() {
    let driver = r#"
#[rseq::driver(name = "uartish", compatible = ["acme,uartish"])]
pub struct Uartish {
    #[rseq::interrupt(id = "uart_irq")]
    pub irq: Interrupt,
    pub seen: u32,
}

impl Uartish {
    #[rseq::isr]
    pub fn handle_irq(&mut self) -> IrqResult {
        if self.seen == 0 {
            return IrqResult::NotHandled;
        }
        IrqResult::Handled
    }
}

pub fn make() -> Uartish { loop {} }
"#;
    let arms = with_model("irq_arms_branch", &source(driver), |model| {
        sole_interrupts(model).handlers[0].arms
    })
    .expect("bridge should accept");
    // CONDITIONAL returns are NOT proof (Codex M1 MUST-FIX): a CFG path's
    // branch predicate may be runtime-infeasible, and claiming
    // ProvenReachable would let the R11 shared-line warning be wrongly
    // SUPPRESSED (under-warn). Both conditional arms stay Unknown — the
    // warn-on-unknown floor over-warns instead. The never-constructed
    // DeferWork stays provably unreachable (the bounded may-set excludes it
    // — the R09-load-bearing direction).
    assert_eq!(arms.handled, ArmReachability::Unknown);
    assert_eq!(arms.not_handled, ArmReachability::Unknown);
    assert_eq!(arms.defer_work, ArmReachability::ProvenUnreachable);
}

// --- All three declaration forms (R02) -----------------------------------------

const THREE_FORMS: &str = r#"
#[rseq::driver(name = "manyirq", compatible = ["acme,manyirq"])]
pub struct ManyIrq {
    #[rseq::interrupt(id = "line_a")]
    pub a: Interrupt,
    #[rseq::interrupt(id = "line_b")]
    pub b: Option<Interrupt>,
    #[rseq::interrupt(id = "line_c")]
    pub c: [Interrupt; 3],
}

impl ManyIrq {
    #[rseq::isr(field = "a")]
    pub fn handle_a(&mut self) -> IrqResult {
        IrqResult::Handled
    }
    #[rseq::isr(field = "c")]
    pub fn handle_c(&mut self, index: usize) -> IrqResult {
        if index == 0 { IrqResult::Handled } else { IrqResult::NotHandled }
    }
}

pub fn make() -> ManyIrq { loop {} }
"#;

#[test]
fn all_three_forms_recognized_whole() {
    let forms = with_model("irq_forms", &source(THREE_FORMS), |model| {
        let d = sole_interrupts(model);
        d.resources
            .iter()
            .map(|r| (model.strings.resolve(r.name).to_string(), r.form))
            .collect::<Vec<_>>()
    })
    .expect("bridge should accept all three forms");
    assert_eq!(
        forms,
        vec![
            ("a".to_string(), InterruptForm::Single),
            ("b".to_string(), InterruptForm::Optional),
            ("c".to_string(), InterruptForm::Array { len: 3 }),
        ]
    );
}

#[test]
fn explicit_field_and_array_bind_handlers_resolve() {
    let handlers = with_model("irq_explicit", &source(THREE_FORMS), |model| {
        let d = sole_interrupts(model);
        d.handlers
            .iter()
            .map(|h| {
                (
                    model.strings.resolve(h.method_name).to_string(),
                    h.resource,
                    h.array_bind,
                )
            })
            .collect::<Vec<_>>()
    })
    .expect("bridge should accept");
    assert_eq!(
        handlers,
        vec![
            ("handle_a".to_string(), 0, false),
            ("handle_c".to_string(), 2, true),
        ]
    );
}

// --- Frontend rejections (the T03 C06 subset) ----------------------------------

#[test]
fn missing_id_is_a_frontend_error() {
    let driver = r#"
#[rseq::driver(name = "noid", compatible = ["acme,noid"])]
pub struct NoId {
    #[rseq::interrupt]
    pub irq: Interrupt,
}

impl NoId {
    #[rseq::isr]
    pub fn handle_irq(&mut self) -> IrqResult { IrqResult::Handled }
}

pub fn make() -> NoId { loop {} }
"#;
    let err = with_model("irq_noid", &source(driver), |_| ()).expect_err("must reject");
    assert!(err.contains("IRQ-ERR-MISSING-ID"), "got: {err}");
}

#[test]
fn bare_isr_with_two_resources_is_ambiguous() {
    let driver = r#"
#[rseq::driver(name = "twoirq", compatible = ["acme,twoirq"])]
pub struct TwoIrq {
    #[rseq::interrupt(id = "a")]
    pub a: Interrupt,
    #[rseq::interrupt(id = "b")]
    pub b: Interrupt,
}

impl TwoIrq {
    #[rseq::isr]
    pub fn handle(&mut self) -> IrqResult { IrqResult::Handled }
}

pub fn make() -> TwoIrq { loop {} }
"#;
    let err = with_model("irq_ambig", &source(driver), |_| ()).expect_err("must reject");
    assert!(err.contains("IRQ-ERR-BARE-ISR-AMBIGUOUS"), "got: {err}");
}

#[test]
fn missing_handler_on_non_optional_is_an_error() {
    let driver = r#"
#[rseq::driver(name = "nohandler", compatible = ["acme,nohandler"])]
pub struct NoHandler {
    #[rseq::interrupt(id = "a")]
    pub a: Interrupt,
}

pub fn make() -> NoHandler { loop {} }
"#;
    let err = with_model("irq_nohandler", &source(driver), |_| ()).expect_err("must reject");
    assert!(err.contains("IRQ-ERR-MISSING-HANDLER"), "got: {err}");
}

#[test]
fn optional_resource_without_handler_is_accepted() {
    let driver = r#"
#[rseq::driver(name = "optirq", compatible = ["acme,optirq"])]
pub struct OptIrq {
    #[rseq::interrupt(id = "maybe")]
    pub maybe: Option<Interrupt>,
}

pub fn make() -> OptIrq { loop {} }
"#;
    let forms = with_model("irq_opt_nohandler", &source(driver), |model| {
        sole_interrupts(model)
            .resources
            .iter()
            .map(|r| r.form)
            .collect::<Vec<_>>()
    })
    .expect("Option form without handler is valid (D13)");
    assert_eq!(forms, vec![InterruptForm::Optional]);
}

#[test]
fn duplicate_handlers_on_one_resource_are_rejected() {
    let driver = r#"
#[rseq::driver(name = "dup", compatible = ["acme,dup"])]
pub struct Dup {
    #[rseq::interrupt(id = "a")]
    pub a: Interrupt,
}

impl Dup {
    #[rseq::isr(field = "a")]
    pub fn one(&mut self) -> IrqResult { IrqResult::Handled }
    #[rseq::isr(field = "a")]
    pub fn two(&mut self) -> IrqResult { IrqResult::Handled }
}

pub fn make() -> Dup { loop {} }
"#;
    let err = with_model("irq_dup", &source(driver), |_| ()).expect_err("must reject");
    assert!(err.contains("IRQ-ERR-DUPLICATE-HANDLER"), "got: {err}");
}

#[test]
fn unknown_isr_field_is_rejected() {
    let driver = r#"
#[rseq::driver(name = "unknown", compatible = ["acme,unknown"])]
pub struct Unknown {
    #[rseq::interrupt(id = "a")]
    pub a: Interrupt,
}

impl Unknown {
    #[rseq::isr(field = "nope")]
    pub fn handle(&mut self) -> IrqResult { IrqResult::Handled }
}

pub fn make() -> Unknown { loop {} }
"#;
    let err = with_model("irq_unknown_field", &source(driver), |_| ()).expect_err("must reject");
    assert!(err.contains("IRQ-ERR-UNKNOWN-FIELD"), "got: {err}");
}

#[test]
fn handler_returning_result_is_rejected() {
    let driver = r#"
#[rseq::driver(name = "resultish", compatible = ["acme,resultish"])]
pub struct Resultish {
    #[rseq::interrupt(id = "a")]
    pub a: Interrupt,
}

impl Resultish {
    #[rseq::isr]
    pub fn handle(&mut self) -> Result<(), RegseqError> { Ok(()) }
}

pub fn make() -> Resultish { loop {} }
"#;
    let err = with_model("irq_result_ret", &source(driver), |_| ()).expect_err("must reject");
    assert!(err.contains("IRQ-ERR-ISR-SIGNATURE"), "got: {err}");
}

#[test]
fn handler_with_shared_receiver_is_rejected() {
    let driver = r#"
#[rseq::driver(name = "sharedrecv", compatible = ["acme,sharedrecv"])]
pub struct SharedRecv {
    #[rseq::interrupt(id = "a")]
    pub a: Interrupt,
}

impl SharedRecv {
    #[rseq::isr]
    pub fn handle(&self) -> IrqResult { IrqResult::Handled }
}

pub fn make() -> SharedRecv { loop {} }
"#;
    let err = with_model("irq_shared_recv", &source(driver), |_| ()).expect_err("must reject");
    assert!(err.contains("IRQ-ERR-ISR-SIGNATURE"), "got: {err}");
}

#[test]
fn array_resource_with_non_indexed_handler_is_rejected() {
    let driver = r#"
#[rseq::driver(name = "arrnoidx", compatible = ["acme,arrnoidx"])]
pub struct ArrNoIdx {
    #[rseq::interrupt(id = "lines")]
    pub lines: [Interrupt; 2],
}

impl ArrNoIdx {
    #[rseq::isr(field = "lines")]
    pub fn handle(&mut self) -> IrqResult { IrqResult::Handled }
}

pub fn make() -> ArrNoIdx { loop {} }
"#;
    let err = with_model("irq_arr_noidx", &source(driver), |_| ()).expect_err("must reject");
    assert!(err.contains("IRQ-ERR-ARRAY-BIND-NO-INDEX"), "got: {err}");
}

#[test]
fn index_param_on_single_resource_is_rejected() {
    let driver = r#"
#[rseq::driver(name = "idxsingle", compatible = ["acme,idxsingle"])]
pub struct IdxSingle {
    #[rseq::interrupt(id = "a")]
    pub a: Interrupt,
}

impl IdxSingle {
    #[rseq::isr]
    pub fn handle(&mut self, index: usize) -> IrqResult {
        let _ = index;
        IrqResult::Handled
    }
}

pub fn make() -> IdxSingle { loop {} }
"#;
    let err = with_model("irq_idx_single", &source(driver), |_| ()).expect_err("must reject");
    assert!(err.contains("IRQ-ERR-ISR-SIGNATURE"), "got: {err}");
}

#[test]
fn trait_provided_isr_is_excluded() {
    let driver = r#"
#[rseq::driver(name = "traitisr", compatible = ["acme,traitisr"])]
pub struct TraitIsr {
    #[rseq::interrupt(id = "a")]
    pub a: Interrupt,
}

pub trait IrqOps {
    fn handle(&mut self) -> IrqResult;
}

impl IrqOps for TraitIsr {
    #[rseq::isr]
    fn handle(&mut self) -> IrqResult { IrqResult::Handled }
}

pub fn make() -> TraitIsr { loop {} }
"#;
    let err = with_model("irq_trait_isr", &source(driver), |_| ()).expect_err("must reject");
    assert!(err.contains("IRQ-ERR-EXCLUDED-PRIMITIVE"), "got: {err}");
}

#[test]
fn line_control_methods_on_interrupt_shell_are_rejected() {
    let driver = r#"
impl Interrupt {
    pub fn enable_line(&mut self) {}
}

#[rseq::driver(name = "linectl", compatible = ["acme,linectl"])]
pub struct LineCtl {
    #[rseq::interrupt(id = "a")]
    pub a: Interrupt,
}

impl LineCtl {
    #[rseq::isr]
    pub fn handle(&mut self) -> IrqResult { IrqResult::Handled }
}

pub fn make() -> LineCtl { loop {} }
"#;
    let err = with_model("irq_line_ctl", &source(driver), |_| ()).expect_err("must reject");
    assert!(
        err.contains("IRQ-ERR-DRIVER-CALLABLE-LINE-CONTROL"),
        "got: {err}"
    );
}

// --- The R11 shared-discipline warning, END-TO-END (T08 acceptance) ------------

/// The PLAN's pinned negative: a handler whose `NotHandled` return is
/// SYNTACTICALLY PRESENT in the authored source but unreachable (a
/// constant-false guard). The authored source flows through the REAL
/// pipeline — bridge recognition, arm-reachability analysis, the L2
/// `[interrupt.<id>] shared = true` join — and the warning must still fire:
/// reachability-based semantics, never syntactic presence (warn-on-unknown).
const UNREACHABLE_NOTHANDLED: &str = r#"
#[rseq::driver(name = "sharedwarn", compatible = ["acme,sharedwarn"])]
pub struct SharedWarn {
    #[rseq::interrupt(id = "a")]
    pub a: Interrupt,
}

impl SharedWarn {
    #[rseq::isr]
    pub fn handle(&mut self) -> IrqResult {
        let always: u32 = 1;
        if always == 0 {
            // Syntactically present, never executable.
            return IrqResult::NotHandled;
        }
        IrqResult::Handled
    }
}

pub fn make() -> SharedWarn { loop {} }
"#;

/// Join `[interrupt.a]` (with the given `shared`) onto the bridged model and
/// return the R11 warnings — the same `bind_all` + query path the production
/// callback runs.
fn warnings_with_shared(model: &RegseqModel, shared: bool) -> Vec<String> {
    let mut model = model.clone();
    let toml = format!("[interrupt.a]\nirq_number = 5\nshared = {shared}\n");
    let bindings = regseq_config::parse_platform_bindings_str(&toml).expect("parse platform");
    regseq_l2::bind_all(&mut model, &bindings).expect("bind_all");
    regseq_l2::validate_l2_surface(&model).expect("validate");
    regseq_l2::interrupt::shared_discipline_warnings(&model)
}

#[test]
fn shared_line_with_present_but_unreachable_nothandled_still_warns_end_to_end() {
    let (warnings, not_handled) = with_model(
        "irq_shared_warn",
        &source(UNREACHABLE_NOTHANDLED),
        |model| {
            (
                warnings_with_shared(model, true),
                sole_interrupts(model).handlers[0].arms.not_handled,
            )
        },
    )
    .expect("bridge accepts the spec");
    // Whatever the analysis concluded for the dead arm (folded ->
    // ProvenUnreachable; surviving -> Unknown), it must NOT be proof.
    assert_ne!(not_handled, ArmReachability::ProvenReachable);
    assert_eq!(warnings.len(), 1, "got: {warnings:?}");
    assert!(
        warnings[0].contains("IRQ-WARN-SHARED-NOTHANDLED"),
        "got: {}",
        warnings[0]
    );
}

#[test]
fn unshared_line_with_same_handler_does_not_warn_end_to_end() {
    let warnings = with_model(
        "irq_unshared_nowarn",
        &source(UNREACHABLE_NOTHANDLED),
        |model| warnings_with_shared(model, false),
    )
    .expect("bridge accepts the spec");
    assert!(warnings.is_empty(), "got: {warnings:?}");
}

// --- Drivers without interrupts stay untouched ---------------------------------

#[test]
fn interruptless_driver_has_no_interrupt_tables() {
    let driver = r#"
#[rseq::driver(name = "plain", compatible = ["acme,plain"])]
pub struct PlainDriver {
    pub counter: u32,
}

pub fn make() -> PlainDriver { loop {} }
"#;
    let empty = with_model("irq_none", &source(driver), |model| {
        model.interrupt.drivers.is_empty()
    })
    .expect("bridge should accept");
    assert!(empty);
}
