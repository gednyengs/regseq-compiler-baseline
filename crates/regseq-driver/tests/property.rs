#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::sync::{Arc, Mutex, Once};

#[path = "../src/extract/property.rs"]
mod property;

use property::*;

type ExtractionResult = Result<PropertySurface, PropertyError>;

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
            *self.result.lock().unwrap() = Some(extract_property_surface(tcx));
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

// ---------------------------------------------------------------------------
// Test 1: Representative property declarations preserve all metadata
// ---------------------------------------------------------------------------

#[test]
fn recognize_property_declarations() {
    let source = r#"
        pub struct Pl011 {
            #[rseq::property(id = "reg-io-width", default = 4)]
            pub reg_io_width: u32,

            #[rseq::property(id = "current-speed")]
            pub current_speed: Option<u32>,

            #[rseq::property(id = "periphid", vendor_prefix = "arm")]
            pub periph_id: Option<u32>,
        }
    "#;

    let surface = compile_and_extract(source, "property_basic").unwrap();

    assert_eq!(surface.fields.len(), 3);

    let rio = surface
        .fields
        .iter()
        .find(|f| f.field_name == "reg_io_width")
        .unwrap();
    assert_eq!(rio.property_id, "reg-io-width");
    assert!(rio.required);
    assert_eq!(rio.default_literal.as_deref(), Some("4"));
    assert_eq!(rio.vendor_prefix, None);
    assert_eq!(rio.driver_type, "Pl011");

    let speed = surface
        .fields
        .iter()
        .find(|f| f.field_name == "current_speed")
        .unwrap();
    assert_eq!(speed.property_id, "current-speed");
    assert!(!speed.required);
    assert_eq!(speed.default_literal, None);
    assert_eq!(speed.vendor_prefix, None);

    let pid = surface
        .fields
        .iter()
        .find(|f| f.field_name == "periph_id")
        .unwrap();
    assert_eq!(pid.property_id, "periphid");
    assert!(!pid.required);
    assert_eq!(pid.vendor_prefix.as_deref(), Some("arm"));
}

// ---------------------------------------------------------------------------
// Test 2: Duplicate property ids rejected
// ---------------------------------------------------------------------------

#[test]
fn reject_duplicate_property_ids() {
    let source = r#"
        pub struct Dev {
            #[rseq::property(id = "speed")]
            pub speed_a: u32,

            #[rseq::property(id = "speed")]
            pub speed_b: u32,
        }
    "#;

    let result = compile_and_extract(source, "property_dup_id");
    assert!(
        matches!(
            &result,
            Err(PropertyError::DuplicatePropertyId {
                property_id,
                ..
            }) if property_id == "speed"
        ),
        "expected DuplicatePropertyId, got: {result:?}"
    );
}

// ---------------------------------------------------------------------------
// Test 3: Malformed property attributes rejected
// ---------------------------------------------------------------------------

#[test]
fn reject_malformed_attribute_missing_eq() {
    let source = r#"
        pub struct Dev {
            #[rseq::property("not-key-value")]
            pub field: u32,
        }
    "#;

    let result = compile_and_extract(source, "property_malformed");
    assert!(
        matches!(result, Err(PropertyError::MalformedAttribute(_))),
        "expected MalformedAttribute, got: {result:?}"
    );
}

#[test]
fn reject_missing_property_id() {
    let source = r#"
        pub struct Dev {
            #[rseq::property(default = 4)]
            pub field: u32,
        }
    "#;

    let result = compile_and_extract(source, "property_no_id");
    assert!(
        matches!(result, Err(PropertyError::MissingPropertyId(_))),
        "expected MissingPropertyId, got: {result:?}"
    );
}

// ---------------------------------------------------------------------------
// Test 4: Required field + default produces lint, not hard failure
// ---------------------------------------------------------------------------

#[test]
fn required_with_default_produces_lint() {
    let source = r#"
        pub struct Dev {
            #[rseq::property(id = "reg-io-width", default = 4)]
            pub reg_io_width: u32,
        }
    "#;

    let surface = compile_and_extract(source, "property_req_default").unwrap();

    assert_eq!(surface.fields.len(), 1);
    assert!(surface.fields[0].required);
    assert_eq!(surface.fields[0].default_literal.as_deref(), Some("4"));

    assert_eq!(surface.lints.len(), 1);
    assert!(matches!(
        &surface.lints[0],
        PropertyLint::RequiredWithDefault {
            field_name,
            property_id,
            ..
        } if field_name == "reg_io_width" && property_id == "reg-io-width"
    ));
}

// ---------------------------------------------------------------------------
// Test 5: Unrelated field attributes are ignored
// ---------------------------------------------------------------------------

#[test]
fn unrelated_attributes_ignored() {
    let source = r#"
        pub struct Dev {
            #[rseq::property(id = "speed")]
            pub speed: u32,

            #[allow(dead_code)]
            pub unrelated: u32,

            pub plain: u32,
        }
    "#;

    let surface = compile_and_extract(source, "property_unrelated").unwrap();

    assert_eq!(surface.fields.len(), 1);
    assert_eq!(surface.fields[0].field_name, "speed");
}

// ---------------------------------------------------------------------------
// Test 6: Extracted field type is recoverable
// ---------------------------------------------------------------------------

#[test]
fn field_type_is_recoverable() {
    let source = r#"
        pub struct Dev {
            #[rseq::property(id = "width")]
            pub width: u32,

            #[rseq::property(id = "speed")]
            pub speed: Option<u32>,
        }
    "#;

    let surface = compile_and_extract(source, "property_field_type").unwrap();

    let width = surface
        .fields
        .iter()
        .find(|f| f.field_name == "width")
        .unwrap();
    assert!(
        width.field_type.contains("u32"),
        "expected field_type to contain 'u32', got: {}",
        width.field_type
    );
    assert!(width.required);

    let speed = surface
        .fields
        .iter()
        .find(|f| f.field_name == "speed")
        .unwrap();
    assert!(
        speed.field_type.contains("Option") && speed.field_type.contains("u32"),
        "expected field_type to contain 'Option' and 'u32', got: {}",
        speed.field_type
    );
    assert!(!speed.required);
}
