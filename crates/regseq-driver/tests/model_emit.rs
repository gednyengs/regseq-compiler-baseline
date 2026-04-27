//! Voided pending MonoRust + RegseqModel redesign.
//! See docs/discussions/compiler-refactoring/test-audit.md for the classification rationale
//! and the retirement criterion.
#![cfg(feature = "post-monorust-tests")]

#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::sync::{Mutex, Once};

use regseq_config::{ColorChoice, CompilerOptions, EmitSpec, LogLevel, TargetOs};
use regseq_driver::RegseqCallbacks;
use serde_json::Value;

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

fn find_driver_module(module: &Value) -> Option<&Value> {
    if module
        .get("kind")
        .and_then(|kind| kind.get("kind"))
        .and_then(Value::as_str)
        == Some("Driver")
    {
        return Some(module);
    }

    module
        .get("submodules")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find_map(find_driver_module)
}

#[test]
fn emit_model_writes_regseq_model_json() {
    const SOURCE: &str = r#"
#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

pub struct Clock;
pub struct ResetLine;
pub struct Interrupt;
pub struct Mmio<T> { _marker: core::marker::PhantomData<T> }
pub struct ReadWrite<T, R> { _v: T, _r: core::marker::PhantomData<R> }
pub enum IrqResult { Handled, NotHandled }

#[allow(non_snake_case)]
pub mod DR_REG {
    pub struct Register;
}

#[repr(C)]
#[rseq::register_block(endian = "little", access_width = 32)]
pub struct MyBlock {
    pub dr: ReadWrite<u32, DR_REG::Register>,
}

#[rseq::driver(name = "resource-drv", compatible = ["acme,fixture"])]
pub struct ResourceDriver {
    #[rseq::clock(id = "core_clk")]
    pub clk: Clock,
    #[rseq::reset(id = "core_rst")]
    pub rst: Option<ResetLine>,
    #[rseq::property(id = "sample-rate", vendor_prefix = "acme")]
    pub sample_rate: u32,
    #[rseq::interrupt(id = "core_irq")]
    pub irq: Interrupt,
    #[rseq::mmio(id = "regs")]
    pub regs: Mmio<MyBlock>,
}

impl ResourceDriver {
    #[rseq::isr]
    pub fn on_irq(&mut self) -> IrqResult { IrqResult::Handled }
}
"#;

    let _lock = COMPILER_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    init_rustc();

    let test_dir = std::env::temp_dir()
        .join("regseq-driver-tests")
        .join("emit_model_writes_regseq_model_json");
    if test_dir.exists() {
        std::fs::remove_dir_all(&test_dir).unwrap();
    }
    std::fs::create_dir_all(&test_dir).unwrap();

    let src_path = test_dir.join("test_input.rs");
    std::fs::write(&src_path, SOURCE).unwrap();

    let platform_path = test_dir.join("platform.toml");
    std::fs::write(&platform_path, "").unwrap();

    let output_dir = test_dir.join("out");
    let options = CompilerOptions {
        target_os: TargetOs::Linux,
        platform_file: platform_path,
        output_dir: output_dir.clone(),
        emit: vec![EmitSpec::Model { path: None }],
        check: false,
        add_analysis: vec![],
        skip_analysis: vec![],
        deny: vec![],
        allow: vec![],
        params: vec![],
        log_level: LogLevel::Warn,
        dump_stage: None,
        color: ColorChoice::Auto,
    };

    let args = vec![
        "rustc".to_string(),
        src_path.to_str().unwrap().to_string(),
        "--sysroot".to_string(),
        get_sysroot(),
        "--edition".to_string(),
        "2021".to_string(),
        "--crate-type".to_string(),
        "lib".to_string(),
        "--crate-name".to_string(),
        "emit_model_specimen".to_string(),
        "--out-dir".to_string(),
        output_dir.to_str().unwrap().to_string(),
    ];

    let mut callbacks = RegseqCallbacks::new(options);
    rustc_driver::run_compiler(&args, &mut callbacks);

    let model_path = output_dir.join("drivers/resource-drv.model.json");
    let json = std::fs::read_to_string(&model_path).expect("model JSON should be emitted");
    let value: Value = serde_json::from_str(&json).expect("model JSON should deserialize");
    let driver_module = find_driver_module(&value["root_module"]).expect("driver module present");
    let driver = &driver_module["driver"];

    assert_eq!(driver["identity"]["name"], "resource-drv");
    assert_eq!(driver["clocks"].as_array().unwrap().len(), 1);
    assert_eq!(driver["resets"].as_array().unwrap().len(), 1);
    assert_eq!(driver["interrupts"].as_array().unwrap().len(), 1);
    assert_eq!(driver["mmio"].as_array().unwrap().len(), 1);
    assert_eq!(driver["properties"].as_array().unwrap().len(), 1);

    std::fs::remove_dir_all(&test_dir).ok();
}
