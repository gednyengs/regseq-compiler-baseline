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

#[path = "../src/extract/traits.rs"]
mod traits;

use callback_context::CallbackContext;
use traits::*;

type ExtractionResult = Result<TraitSurface, TraitError>;

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
            *self.result.lock().unwrap() = Some(extract_trait_surface(tcx));
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
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum RegseqError {
        InvalidParameter,
        HardwareError,
        GenericFailure,
    }

    pub struct SerialConfig { _private: () }
    pub struct RxChar { _private: () }
"#;

const LIFECYCLE_TRAIT: &str = r#"
    pub trait Lifecycle {
        fn init(&mut self) -> Result<(), RegseqError>;
        fn deinit(&mut self) -> Result<(), RegseqError>;
    }
"#;

const SERIAL_TRAIT: &str = r#"
    pub trait Serial {
        fn port_open(&mut self) -> Result<(), RegseqError>;
        fn port_close(&mut self);
        fn configure(&mut self, config: &SerialConfig) -> Result<(), RegseqError>;
        fn tx_byte(&mut self, byte: u8);
        fn rx_char(&mut self) -> Option<RxChar>;
        fn tx_ready(&mut self) -> bool;
    }
"#;

const SERIAL_CONSOLE_TRAIT: &str = r#"
    pub trait SerialConsole {
        fn console_write(&mut self, text: &str);
    }
"#;

// ---------------------------------------------------------------------------
// Test 1: Lifecycle impls recognized
// ---------------------------------------------------------------------------

#[test]
fn recognize_lifecycle_impl() {
    let source = format!(
        r#"
        {FIXTURE_PREAMBLE}
        {LIFECYCLE_TRAIT}

        pub struct Pl011;

        impl Lifecycle for Pl011 {{
            fn init(&mut self) -> Result<(), RegseqError> {{ Ok(()) }}
            fn deinit(&mut self) -> Result<(), RegseqError> {{ Ok(()) }}
        }}
    "#
    );

    let surface = compile_and_extract(&source, "lifecycle_recognized").unwrap();
    assert_eq!(surface.lifecycle_impls.len(), 1);
    assert_eq!(surface.lifecycle_impls[0].driver_type, "Pl011");
}

// ---------------------------------------------------------------------------
// Test 2: Serial impls recognized
// ---------------------------------------------------------------------------

#[test]
fn recognize_serial_impl() {
    let source = format!(
        r#"
        {FIXTURE_PREAMBLE}
        {SERIAL_TRAIT}

        pub struct Pl011;

        impl Serial for Pl011 {{
            fn port_open(&mut self) -> Result<(), RegseqError> {{ Ok(()) }}
            fn port_close(&mut self) {{}}
            fn configure(&mut self, _config: &SerialConfig) -> Result<(), RegseqError> {{ Ok(()) }}
            fn tx_byte(&mut self, _byte: u8) {{}}
            fn rx_char(&mut self) -> Option<RxChar> {{ None }}
            fn tx_ready(&mut self) -> bool {{ true }}
        }}
    "#
    );

    let surface = compile_and_extract(&source, "serial_recognized").unwrap();
    assert_eq!(surface.serial_impls.len(), 1);
    assert_eq!(surface.serial_impls[0].driver_type, "Pl011");
}

// ---------------------------------------------------------------------------
// Test 3: Optional SerialConsole recognized independently
// ---------------------------------------------------------------------------

#[test]
fn recognize_serial_console_independently() {
    let source = format!(
        r#"
        {FIXTURE_PREAMBLE}
        {SERIAL_CONSOLE_TRAIT}

        pub struct Pl011;

        impl SerialConsole for Pl011 {{
            fn console_write(&mut self, _text: &str) {{}}
        }}
    "#
    );

    let surface = compile_and_extract(&source, "serial_console_independent").unwrap();
    assert_eq!(surface.serial_console_impls.len(), 1);
    assert_eq!(surface.serial_console_impls[0].driver_type, "Pl011");
    assert!(surface.serial_impls.is_empty());
    assert!(surface.lifecycle_impls.is_empty());
}

// ---------------------------------------------------------------------------
// Test 4: Wrong signatures rejected
// ---------------------------------------------------------------------------

#[test]
fn reject_lifecycle_wrong_return_type() {
    let source = format!(
        r#"
        {FIXTURE_PREAMBLE}

        pub trait Lifecycle {{
            fn init(&mut self) -> bool;
            fn deinit(&mut self) -> Result<(), RegseqError>;
        }}

        pub struct Dev;

        impl Lifecycle for Dev {{
            fn init(&mut self) -> bool {{ true }}
            fn deinit(&mut self) -> Result<(), RegseqError> {{ Ok(()) }}
        }}
    "#
    );

    let result = compile_and_extract(&source, "lifecycle_wrong_return");
    assert_eq!(
        result,
        Err(TraitError::InvalidReturnType {
            trait_name: "Lifecycle".into(),
            method: "init".into(),
        })
    );
}

#[test]
fn reject_lifecycle_wrong_receiver() {
    let source = format!(
        r#"
        {FIXTURE_PREAMBLE}

        pub trait Lifecycle {{
            fn init(&self) -> Result<(), RegseqError>;
            fn deinit(&self) -> Result<(), RegseqError>;
        }}

        pub struct Dev;

        impl Lifecycle for Dev {{
            fn init(&self) -> Result<(), RegseqError> {{ Ok(()) }}
            fn deinit(&self) -> Result<(), RegseqError> {{ Ok(()) }}
        }}
    "#
    );

    let result = compile_and_extract(&source, "lifecycle_wrong_receiver");
    assert_eq!(
        result,
        Err(TraitError::InvalidReceiver {
            trait_name: "Lifecycle".into(),
            method: "init".into(),
        })
    );
}

// ---------------------------------------------------------------------------
// Test 5: Callback contexts are F for Lifecycle, S for Serial/SerialConsole
// ---------------------------------------------------------------------------

#[test]
fn callback_contexts_correct() {
    let source = format!(
        r#"
        {FIXTURE_PREAMBLE}
        {LIFECYCLE_TRAIT}
        {SERIAL_TRAIT}
        {SERIAL_CONSOLE_TRAIT}

        pub struct Dev;

        impl Lifecycle for Dev {{
            fn init(&mut self) -> Result<(), RegseqError> {{ Ok(()) }}
            fn deinit(&mut self) -> Result<(), RegseqError> {{ Ok(()) }}
        }}

        impl Serial for Dev {{
            fn port_open(&mut self) -> Result<(), RegseqError> {{ Ok(()) }}
            fn port_close(&mut self) {{}}
            fn configure(&mut self, _config: &SerialConfig) -> Result<(), RegseqError> {{ Ok(()) }}
            fn tx_byte(&mut self, _byte: u8) {{}}
            fn rx_char(&mut self) -> Option<RxChar> {{ None }}
            fn tx_ready(&mut self) -> bool {{ true }}
        }}

        impl SerialConsole for Dev {{
            fn console_write(&mut self, _text: &str) {{}}
        }}
    "#
    );

    let surface = compile_and_extract(&source, "callback_contexts").unwrap();

    assert_eq!(
        surface.lifecycle_impls[0].context,
        CallbackContext::FrameworkManaged
    );
    assert_eq!(surface.lifecycle_impls[0].context.short_label(), "F");

    assert_eq!(
        surface.serial_impls[0].context,
        CallbackContext::SubsystemSerialized
    );
    assert_eq!(surface.serial_impls[0].context.short_label(), "S");

    assert_eq!(
        surface.serial_console_impls[0].context,
        CallbackContext::SubsystemSerialized
    );
    assert_eq!(surface.serial_console_impls[0].context.short_label(), "S");
}

// ---------------------------------------------------------------------------
// Test 6: Per-method Serial return-shape cross-swaps rejected
// ---------------------------------------------------------------------------

#[test]
fn reject_serial_rx_char_returning_bool() {
    let source = format!(
        r#"
        {FIXTURE_PREAMBLE}

        pub trait Serial {{
            fn port_open(&mut self) -> Result<(), RegseqError>;
            fn port_close(&mut self);
            fn configure(&mut self, config: &SerialConfig) -> Result<(), RegseqError>;
            fn tx_byte(&mut self, byte: u8);
            fn rx_char(&mut self) -> bool;
            fn tx_ready(&mut self) -> bool;
        }}

        pub struct Dev;

        impl Serial for Dev {{
            fn port_open(&mut self) -> Result<(), RegseqError> {{ Ok(()) }}
            fn port_close(&mut self) {{}}
            fn configure(&mut self, _config: &SerialConfig) -> Result<(), RegseqError> {{ Ok(()) }}
            fn tx_byte(&mut self, _byte: u8) {{}}
            fn rx_char(&mut self) -> bool {{ false }}
            fn tx_ready(&mut self) -> bool {{ true }}
        }}
    "#
    );

    let result = compile_and_extract(&source, "serial_rx_char_wrong_return");
    assert_eq!(
        result,
        Err(TraitError::InvalidReturnType {
            trait_name: "Serial".into(),
            method: "rx_char".into(),
        })
    );
}

#[test]
fn reject_serial_tx_ready_returning_option() {
    let source = format!(
        r#"
        {FIXTURE_PREAMBLE}

        pub trait Serial {{
            fn port_open(&mut self) -> Result<(), RegseqError>;
            fn port_close(&mut self);
            fn configure(&mut self, config: &SerialConfig) -> Result<(), RegseqError>;
            fn tx_byte(&mut self, byte: u8);
            fn rx_char(&mut self) -> Option<RxChar>;
            fn tx_ready(&mut self) -> Option<RxChar>;
        }}

        pub struct Dev;

        impl Serial for Dev {{
            fn port_open(&mut self) -> Result<(), RegseqError> {{ Ok(()) }}
            fn port_close(&mut self) {{}}
            fn configure(&mut self, _config: &SerialConfig) -> Result<(), RegseqError> {{ Ok(()) }}
            fn tx_byte(&mut self, _byte: u8) {{}}
            fn rx_char(&mut self) -> Option<RxChar> {{ None }}
            fn tx_ready(&mut self) -> Option<RxChar> {{ None }}
        }}
    "#
    );

    let result = compile_and_extract(&source, "serial_tx_ready_wrong_return");
    assert_eq!(
        result,
        Err(TraitError::InvalidReturnType {
            trait_name: "Serial".into(),
            method: "tx_ready".into(),
        })
    );
}
