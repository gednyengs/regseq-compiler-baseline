#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::sync::{Arc, Mutex, Once};

use regseq_driver::extract::{extract_driver_identities, DriverIdentity, DriverIdentityError};

type ExtractionResult = Result<Vec<DriverIdentity>, DriverIdentityError>;

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
            *self.result.lock().unwrap() = Some(extract_driver_identities(tcx));
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

#[test]
fn extract_valid_driver_identity() {
    let source = r#"
        #[rseq::driver(name = "pl011-uart", compatible = ["arm,pl011", "arm,sbsa-uart"])]
        #[rseq::device_class(serial)]
        pub struct Pl011;
    "#;

    let identities = compile_and_extract(source, "valid").unwrap();
    assert_eq!(identities.len(), 1);
    assert_eq!(identities[0].driver_name, "pl011-uart");
    assert_eq!(identities[0].compatible, vec!["arm,pl011", "arm,sbsa-uart"]);
    assert_eq!(identities[0].device_class, "serial");
}

#[test]
fn reject_missing_driver_name() {
    let source = r#"
        #[rseq::driver(compatible = ["arm,pl011"])]
        #[rseq::device_class(serial)]
        pub struct Pl011;
    "#;

    let result = compile_and_extract(source, "missing_name");
    assert_eq!(result, Err(DriverIdentityError::MissingDriverName));
}

#[test]
fn reject_missing_compatible() {
    let source = r#"
        #[rseq::driver(name = "pl011-uart")]
        #[rseq::device_class(serial)]
        pub struct Pl011;
    "#;

    let result = compile_and_extract(source, "missing_compat");
    assert_eq!(result, Err(DriverIdentityError::MissingCompatible));
}

#[test]
fn reject_missing_device_class() {
    let source = r#"
        #[rseq::driver(name = "pl011-uart", compatible = ["arm,pl011"])]
        pub struct Pl011;
    "#;

    let result = compile_and_extract(source, "missing_class");
    assert_eq!(result, Err(DriverIdentityError::MissingDeviceClass));
}

#[test]
fn reject_duplicate_driver_attribute() {
    let source = r#"
        #[rseq::driver(name = "a", compatible = ["a"])]
        #[rseq::driver(name = "b", compatible = ["b"])]
        #[rseq::device_class(serial)]
        pub struct Pl011;
    "#;

    let result = compile_and_extract(source, "duplicate");
    assert_eq!(
        result,
        Err(DriverIdentityError::DuplicateAttribute("driver".into()))
    );
}

#[test]
fn reject_malformed_driver_attribute() {
    let source = r#"
        #[rseq::driver("not-a-key-value")]
        #[rseq::device_class(serial)]
        pub struct Pl011;
    "#;

    let result = compile_and_extract(source, "malformed");
    assert!(matches!(
        result,
        Err(DriverIdentityError::MalformedAttribute(_))
    ));
}
