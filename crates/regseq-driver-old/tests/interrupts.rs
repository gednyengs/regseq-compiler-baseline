#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::sync::{Arc, Mutex, Once};

#[path = "../src/extract/callback_context.rs"]
mod callback_context;

#[path = "../src/extract/interrupts.rs"]
mod interrupts;

use callback_context::CallbackContext;
use interrupts::*;

type ExtractionResult = Result<InterruptSurface, InterruptError>;

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

fn compile_and_extract(source: &str, test_name: &str) -> ExtractionResult {
    let _lock = COMPILER_LOCK.lock().unwrap();
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

    let result: Arc<Mutex<Option<ExtractionResult>>> = Arc::new(Mutex::new(None));

    struct ExtractCallbacks {
        result: Arc<Mutex<Option<ExtractionResult>>>,
    }

    impl rustc_driver::Callbacks for ExtractCallbacks {
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
        }

        fn after_analysis<'tcx>(
            &mut self,
            _compiler: &rustc_interface::interface::Compiler,
            tcx: rustc_middle::ty::TyCtxt<'tcx>,
        ) -> rustc_driver::Compilation {
            *self.result.lock().unwrap() = Some(extract_interrupt_surface(tcx));
            rustc_driver::Compilation::Stop
        }
    }

    let mut callbacks = ExtractCallbacks {
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
        "--out-dir".to_string(),
        test_dir.to_str().unwrap().to_string(),
    ];

    rustc_driver::run_compiler(&args, &mut callbacks);

    let extracted = result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis should have been called");

    std::fs::remove_dir_all(&test_dir).ok();

    extracted
}

const FIXTURE_PREAMBLE: &str = r#"
    pub struct Interrupt;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum IrqResult {
        Handled,
        NotHandled,
        DeferWork,
    }
"#;

// ---------------------------------------------------------------------------
// Test 1: Recognize interrupt resource field shapes
// ---------------------------------------------------------------------------

#[test]
fn recognize_interrupt_single_optional_and_array() {
    let source = format!(
        r#"
        {preamble}

        pub struct Device {{
            #[rseq::interrupt(id = "combined_irq")]
            pub irq: Interrupt,
            #[rseq::interrupt(id = "aux_irq")]
            pub aux: Option<Interrupt>,
            #[rseq::interrupt(id = "channel_irqs")]
            pub channels: [Interrupt; 4],
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "interrupt_shapes").unwrap();

    assert_eq!(surface.resources.len(), 3);

    let single = surface
        .resources
        .iter()
        .find(|r| r.field_name == "irq")
        .unwrap();
    assert_eq!(single.interrupt_id, "combined_irq");
    assert_eq!(single.shape, InterruptFieldShape::Single);

    let optional = surface
        .resources
        .iter()
        .find(|r| r.field_name == "aux")
        .unwrap();
    assert_eq!(optional.interrupt_id, "aux_irq");
    assert_eq!(optional.shape, InterruptFieldShape::Optional);

    let array = surface
        .resources
        .iter()
        .find(|r| r.field_name == "channels")
        .unwrap();
    assert_eq!(array.interrupt_id, "channel_irqs");
    assert_eq!(array.shape, InterruptFieldShape::Array(4));
}

// ---------------------------------------------------------------------------
// Test 2: Recognize all ISR binding forms
// ---------------------------------------------------------------------------

#[test]
fn recognize_isr_binding_forms() {
    let source = format!(
        r#"
        {preamble}

        pub struct SingleDevice {{
            #[rseq::interrupt(id = "irq")]
            pub irq: Interrupt,
        }}

        impl SingleDevice {{
            #[rseq::isr]
            pub fn handle_irq(&mut self) -> IrqResult {{
                IrqResult::Handled
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "isr_bare_autobind").unwrap();
    assert_eq!(surface.isr_bindings.len(), 1);
    assert_eq!(surface.isr_bindings[0].handler_name, "handle_irq");
    assert_eq!(surface.isr_bindings[0].bind_target, IsrBindTarget::Auto);
    assert!(!surface.isr_bindings[0].has_index_param);

    // Explicit field bind
    let source2 = format!(
        r#"
        {preamble}

        pub struct MultiDevice {{
            #[rseq::interrupt(id = "rx_irq")]
            pub rx_irq: Interrupt,
            #[rseq::interrupt(id = "tx_irq")]
            pub tx_irq: Interrupt,
        }}

        impl MultiDevice {{
            #[rseq::isr(rx_irq)]
            pub fn handle_rx(&mut self) -> IrqResult {{
                IrqResult::Handled
            }}

            #[rseq::isr(tx_irq)]
            pub fn handle_tx(&mut self) -> IrqResult {{
                IrqResult::Handled
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface2 = compile_and_extract(&source2, "isr_explicit_bind").unwrap();
    assert_eq!(surface2.isr_bindings.len(), 2);
    let rx = surface2
        .isr_bindings
        .iter()
        .find(|b| b.handler_name == "handle_rx")
        .unwrap();
    assert_eq!(
        rx.bind_target,
        IsrBindTarget::ExplicitField("rx_irq".into())
    );

    // Array bind with index: usize
    let source3 = format!(
        r#"
        {preamble}

        pub struct ArrayDevice {{
            #[rseq::interrupt(id = "channel_irqs")]
            pub channels: [Interrupt; 4],
        }}

        impl ArrayDevice {{
            #[rseq::isr(channels)]
            pub fn handle_channel(&mut self, _index: usize) -> IrqResult {{
                IrqResult::Handled
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface3 = compile_and_extract(&source3, "isr_array_bind").unwrap();
    assert_eq!(surface3.isr_bindings.len(), 1);
    assert_eq!(
        surface3.isr_bindings[0].bind_target,
        IsrBindTarget::ExplicitField("channels".into())
    );
    assert!(surface3.isr_bindings[0].has_index_param);
}

// ---------------------------------------------------------------------------
// Test 3: Structural validation rejects invalid cases
// ---------------------------------------------------------------------------

#[test]
fn reject_ambiguous_bare_autobind() {
    let source = format!(
        r#"
        {preamble}

        pub struct MultiDevice {{
            #[rseq::interrupt(id = "rx_irq")]
            pub rx_irq: Interrupt,
            #[rseq::interrupt(id = "tx_irq")]
            pub tx_irq: Interrupt,
        }}

        impl MultiDevice {{
            #[rseq::isr]
            pub fn handle(&mut self) -> IrqResult {{
                IrqResult::Handled
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let result = compile_and_extract(&source, "ambiguous_bare_isr");
    assert_eq!(result, Err(InterruptError::AmbiguousBareIsr));
}

#[test]
fn reject_array_handler_without_index() {
    let source = format!(
        r#"
        {preamble}

        pub struct ArrayDevice {{
            #[rseq::interrupt(id = "channel_irqs")]
            pub channels: [Interrupt; 4],
        }}

        impl ArrayDevice {{
            #[rseq::isr(channels)]
            pub fn handle_channel(&mut self) -> IrqResult {{
                IrqResult::Handled
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let result = compile_and_extract(&source, "missing_index_param");
    assert!(matches!(result, Err(InterruptError::MissingIndexParam(_))));
}

#[test]
fn reject_wrong_isr_return_type() {
    let source = format!(
        r#"
        {preamble}

        pub struct Device {{
            #[rseq::interrupt(id = "irq")]
            pub irq: Interrupt,
        }}

        impl Device {{
            #[rseq::isr]
            pub fn handle(&mut self) -> bool {{
                true
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let result = compile_and_extract(&source, "wrong_return_type");
    assert!(matches!(
        result,
        Err(InterruptError::InvalidIsrReturnType(_))
    ));
}

// ---------------------------------------------------------------------------
// Test 4: Layer 2 interrupt binding properties reachable
// ---------------------------------------------------------------------------

#[test]
fn layer2_properties_reachable() {
    let source = format!(
        r#"
        {preamble}

        pub struct Device {{
            #[rseq::interrupt(id = "irq", trigger_type = "level", trigger_polarity = "high", priority = "normal", shared = true)]
            pub irq: Interrupt,
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "layer2_properties").unwrap();
    assert_eq!(surface.resources.len(), 1);
    let r = &surface.resources[0];
    assert_eq!(r.layer2.trigger_type, Some("level".into()));
    assert_eq!(r.layer2.trigger_polarity, Some("high".into()));
    assert_eq!(r.layer2.priority, Some("normal".into()));
    assert_eq!(r.layer2.shared, Some(true));
}

// ---------------------------------------------------------------------------
// Test 5: Extracted handler context is H
// ---------------------------------------------------------------------------

#[test]
fn handler_context_is_h() {
    let source = format!(
        r#"
        {preamble}

        pub struct Device {{
            #[rseq::interrupt(id = "irq")]
            pub irq: Interrupt,
        }}

        impl Device {{
            #[rseq::isr]
            pub fn handle_irq(&mut self) -> IrqResult {{
                IrqResult::Handled
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "handler_context").unwrap();
    assert_eq!(surface.isr_bindings.len(), 1);
    assert_eq!(
        surface.isr_bindings[0].context,
        CallbackContext::HandlerManaged
    );
    assert_eq!(surface.isr_bindings[0].context.short_label(), "H");
}
