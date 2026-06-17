//! Bridge tests for serial device-class recognition (construct #7, PLAN T03).
//!
//! Covers: a serial-class driver's op-set flowing into `model.serial` (base
//! `Serial` C04 + optional `SerialRuntimeLineConfig` C03 + optional
//! `SerialInterruptDriven` C01); the support-enum variant maps captured +
//! validated against the closed vocab (I02); the optional capability planes by
//! impl-presence; the required-base diagnostic (SER-ERR-MISSING-BASE); and the
//! construct-#6 serial relaxation (B3): a serial driver with an `Interrupt`
//! resource and NO authored `#[rseq::isr]` bridges (the line handler is the
//! compiler-synthesized serial ISR).

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
use regseq_model::{DriverSerial, RegseqModel};
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
    std::env::temp_dir().join(format!("regseq-bridge-serial-{crate_name}-{nanos}"))
}

/// Run the full bridge over `source`; on success apply `f` to the model, on
/// rejection return the rendered `BridgeError`.
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

const LIFECYCLE_SURFACE: &str =
    include_str!("../../codegen-coverage-runner/fixtures/_lifecycle_surface.rs");
const INTERRUPT_SURFACE: &str =
    include_str!("../../codegen-coverage-runner/fixtures/_interrupt_surface.rs");
const SERIAL_SURFACE: &str =
    include_str!("../../codegen-coverage-runner/fixtures/_serial_surface.rs");

/// Build a bridgeable crate around a driver/impl snippet, with the serial trait
/// surface in scope (trivial-body impls — recognition reads signatures, not
/// bodies, so no register surface is needed here).
fn source(driver: &str) -> String {
    format!(
        "#![feature(register_tool)]\n#![register_tool(rseq)]\n\
         {LIFECYCLE_SURFACE}\n{INTERRUPT_SURFACE}\n{SERIAL_SURFACE}\n{driver}\npub fn entry() {{}}\n"
    )
}

fn sole_serial(model: &RegseqModel) -> DriverSerial {
    assert_eq!(model.serial.drivers.len(), 1, "exactly one serial driver");
    model.serial.drivers.values().next().unwrap().clone()
}

/// The trivial-body base `Serial` impl reused across drivers.
const BASE_IMPL: &str = r#"
impl Serial for S {
    fn port_service_set(&mut self, _scope: PortScope, _on: bool) -> PrevState { PrevState::new(false) }
    fn data_ready_query(&self, _dir: ReadyDir) -> bool { false }
    fn data_unit_write(&mut self, _unit: u32) {}
    fn data_unit_read(&mut self) -> RxDatum { RxDatum::new(0, RxError::empty()) }
}
"#;

const LINE_CONFIG_IMPL: &str = r#"
impl SerialRuntimeLineConfig for S {
    fn line_format_apply(&mut self, _f: LineFormat) -> ConfigResult { ConfigResult::Ok }
}
"#;

const INTERRUPT_IMPL: &str = r#"
impl SerialInterruptDriven for S {
    fn irq_source_set(&mut self, _source: SerialIrqSource, _state: IrqSourceState) {}
    fn irq_cause_decode(&mut self) -> CauseSet { CauseSet::empty() }
}
"#;

// --- the full in-scope surface: all three planes + the B3 relaxation ----------

#[test]
fn full_serial_surface_flows_into_model() {
    let driver = format!(
        r#"
#[rseq::driver(name = "s", compatible = ["acme,s"])]
#[rseq::device_class(serial)]
pub struct S {{
    #[rseq::interrupt(id = "uart_irq")]
    pub irq: Interrupt,
}}
{BASE_IMPL}
{LINE_CONFIG_IMPL}
{INTERRUPT_IMPL}
pub fn make() -> S {{ loop {{}} }}
"#
    );
    let ds = with_model("serial_full", &source(&driver), sole_serial)
        .expect("the full in-scope serial surface must bridge (incl. B3: no authored ISR)");

    // Base C04: all four ops materialized.
    assert!(ds.base.port_service_set.is_some(), "port_service_set body");
    assert!(ds.base.data_ready_query.is_some(), "data_ready_query body");
    assert!(ds.base.data_unit_write.is_some(), "data_unit_write body");
    assert!(ds.base.data_unit_read.is_some(), "data_unit_read body");

    // Optional planes present by impl.
    let lc = ds.line_config.expect("SerialRuntimeLineConfig plane");
    assert!(lc.line_format_apply.is_some(), "line_format_apply body");
    let id = ds.interrupt_driven.expect("SerialInterruptDriven plane");
    assert!(id.irq_source_set.is_some(), "irq_source_set body");
    assert!(id.irq_cause_decode.is_some(), "irq_cause_decode body");

    // CauseSet repr pinned (O2): 5 causes.
    assert_eq!(id.cause_set.causes.len(), 5);

    // Variant maps captured in canonical (declaration) order.
    let v = |b: &regseq_model::EnumBinding| b.variants.iter().map(|x| x.as_u32()).collect::<Vec<_>>();
    assert_eq!(v(ds.types.irq_source.as_ref().unwrap()), vec![0, 1, 2]);
    assert_eq!(v(ds.types.irq_state.as_ref().unwrap()), vec![0, 1]);
    assert_eq!(v(ds.types.config_result.as_ref().unwrap()), vec![0, 1]);
    assert_eq!(v(ds.types.port_scope.as_ref().unwrap()), vec![0, 1, 2]);
    assert_eq!(v(ds.types.ready_dir.as_ref().unwrap()), vec![0, 1]);
    assert_eq!(v(ds.types.data_bits.as_ref().unwrap()), vec![0, 1, 2, 3, 4]);
    assert_eq!(v(ds.types.parity.as_ref().unwrap()), vec![0, 1, 2]);
    assert_eq!(v(ds.types.stop_bits.as_ref().unwrap()), vec![0, 1]);
}

// --- optional planes absent (impl-presence gating) ----------------------------

#[test]
fn base_only_serial_driver_has_no_capability_planes() {
    let driver = format!(
        r#"
#[rseq::driver(name = "s", compatible = ["acme,s"])]
#[rseq::device_class(serial)]
pub struct S {{}}
{BASE_IMPL}
pub fn make() -> S {{ loop {{}} }}
"#
    );
    let ds = with_model("serial_base_only", &source(&driver), sole_serial)
        .expect("a base-only serial driver must bridge");
    assert!(ds.base.port_service_set.is_some());
    assert!(ds.line_config.is_none(), "no SerialRuntimeLineConfig plane");
    assert!(ds.interrupt_driven.is_none(), "no SerialInterruptDriven plane");
    assert!(ds.types.irq_source.is_none(), "no irq_source map without the plane");
}

// --- required base diagnostic: a capability plane without the base ------------

#[test]
fn capability_plane_without_base_impl_is_rejected() {
    // A driver implementing the interrupt plane but NOT the base `Serial` data
    // path is an incoherent surface → SER-ERR-MISSING-BASE.
    let driver = format!(
        r#"
#[rseq::driver(name = "s", compatible = ["acme,s"])]
#[rseq::device_class(serial)]
pub struct S {{}}
{INTERRUPT_IMPL}
pub fn make() -> S {{ loop {{}} }}
"#
    );
    let err = with_model("serial_plane_no_base", &source(&driver), |_| ())
        .expect_err("a capability plane without impl Serial must be rejected");
    assert!(
        err.contains("SER-ERR-MISSING-BASE"),
        "expected SER-ERR-MISSING-BASE, got: {err}"
    );
}

// --- a bare serial tag with NO serial impls is lenient (deferred class) --------

#[test]
fn bare_serial_tag_without_any_impl_is_lenient() {
    // `#[rseq::device_class(serial)]` with no serial trait impls at all is a
    // bare/deferred serial tag: it bridges (the backend keeps the deferred-class
    // fallback) and populates NO serial tables.
    let driver = r#"
#[rseq::driver(name = "s", compatible = ["acme,s"])]
#[rseq::device_class(serial)]
pub struct S {}
pub fn make() -> S { loop {} }
"#;
    let empty = with_model("serial_bare_tag", &source(driver), |model| {
        model.serial.drivers.is_empty()
    })
    .expect("a bare serial tag must bridge (deferred class)");
    assert!(empty, "a bare serial tag populates no serial tables");
}

// --- non-serial driver is untouched (additive) --------------------------------

#[test]
fn non_serial_driver_has_no_serial_tables() {
    let driver = r#"
#[rseq::driver(name = "g", compatible = ["acme,g"])]
pub struct S {}
pub fn make() -> S { loop {} }
"#;
    let empty = with_model("serial_none", &source(driver), |model| {
        model.serial.drivers.is_empty()
    })
    .expect("a non-serial driver must bridge");
    assert!(empty, "a non-serial driver populates no serial tables");
}
