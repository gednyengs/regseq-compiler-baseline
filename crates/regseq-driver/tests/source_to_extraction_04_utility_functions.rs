//! source-to-extraction-04: PL011 utility functions authored-to-extraction
//! boundary validation.
//!
//! Validates that the authored local utility-function facts for PL011 survive
//! at the compiler-side boundary between the authored PL011 source and the
//! current Regseq extraction state.
//!
//! Extraction surfaces under test:
//!   - `extract_identity_enrichment(tcx) -> IdentityEnrichmentSurface`
//!     -- inspects `IdentityEnrichmentSurface.free_functions`
//!   - `extract_body_surface(tcx) -> MirBodySurface`
//!     -- inspects `MirBodySurface.functions`
//!
//! Specimen: inline reduced specimen mirroring the three PL011 utility
//! functions. Fidelity is verified against the real fixture source at
//! `tests/fixtures/pl011-full-spec/src/driver.rs`.

#![feature(rustc_private)]
#![feature(box_patterns)]

extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::sync::{Arc, Mutex, Once};

#[path = "../src/extract/identity_enrichment_types.rs"]
mod identity_enrichment_types;

#[path = "../src/extract/identity_enrichment.rs"]
mod identity_enrichment;

#[path = "../src/mir_repr.rs"]
mod mir_repr;

#[path = "../src/extract/mir_extract.rs"]
mod mir_extract;

#[path = "../src/extract/body.rs"]
mod body;

use body::extract_body_surface;
use identity_enrichment::*;
use mir_repr::*;

// ---------------------------------------------------------------------------
// Authored facts from the check specification
// ---------------------------------------------------------------------------

/// Authored utility function names.
const AUTHORED_UTILITY_FN_NAMES: &[&str] = &["field_val", "raw_reg_val", "field_from_raw"];

/// Authored signatures (for fixture-fidelity verification).
const AUTHORED_FIELD_VAL_SIG: &str =
    "fn field_val<R>(field: Field<u32, R>, raw: u32) -> FieldValue<u32, R>";
const AUTHORED_RAW_REG_VAL_SIG: &str =
    "fn raw_reg_val<R>(mask: u32, value: u32) -> FieldValue<u32, R>";
const AUTHORED_FIELD_FROM_RAW_SIG: &str =
    "fn field_from_raw<R>(field: Field<u32, R>, raw: u32) -> u32";

/// Authored body effects (for fixture-fidelity verification).
const AUTHORED_FIELD_VAL_BODY: &str = "FieldValue::new(field.mask, raw << field.shift)";
const AUTHORED_RAW_REG_VAL_BODY: &str = "FieldValue::new(mask, value)";
const AUTHORED_FIELD_FROM_RAW_BODY: &str = "(raw & field.mask) >> field.shift";

/// Fixture source path.
const FIXTURE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/pl011-full-spec/src/driver.rs"
);

/// Read the PL011 fixture driver source.
fn read_fixture_source() -> String {
    std::fs::read_to_string(FIXTURE_PATH).expect("fixture source must be readable")
}

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

/// Both extraction surfaces collected in a single compiler pass.
struct DualExtractionResult {
    identity: Result<IdentityEnrichmentSurface, IdentityEnrichmentError>,
    body: Result<MirBodySurface, body::BodyError>,
}

fn compile_and_extract_dual(source: &str, test_name: &str) -> DualExtractionResult {
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

    type IdentityResult = Result<IdentityEnrichmentSurface, IdentityEnrichmentError>;
    type BodyResult = Result<MirBodySurface, body::BodyError>;

    let identity_result: Arc<Mutex<Option<IdentityResult>>> = Arc::new(Mutex::new(None));
    let body_result: Arc<Mutex<Option<BodyResult>>> = Arc::new(Mutex::new(None));

    struct ExtractCallbacks {
        identity_result: Arc<Mutex<Option<IdentityResult>>>,
        body_result: Arc<Mutex<Option<BodyResult>>>,
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
            *self.identity_result.lock().unwrap() = Some(extract_identity_enrichment(tcx));
            *self.body_result.lock().unwrap() = Some(extract_body_surface(tcx));
            rustc_driver::Compilation::Stop
        }
    }

    let mut callbacks = ExtractCallbacks {
        identity_result: identity_result.clone(),
        body_result: body_result.clone(),
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

    let identity = identity_result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis should have been called (identity)");
    let body = body_result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis should have been called (body)");

    std::fs::remove_dir_all(&test_dir).ok();

    DualExtractionResult { identity, body }
}

// ---------------------------------------------------------------------------
// Reduced inline specimen
// ---------------------------------------------------------------------------

/// Reduced inline specimen that mirrors the three PL011 utility functions.
///
/// The specimen preserves:
/// - all three function names exactly
/// - generic parameter `R` on each function
/// - parameter names and types
/// - return types
/// - body expressions
///
/// A minimal driver struct is included to satisfy body-extraction prerequisites
/// (body extraction requires finding the `#[rseq::driver]` struct).
const SPECIMEN: &str = r#"
pub struct Field<T, R> {
    pub mask: T,
    pub shift: T,
    _marker: core::marker::PhantomData<R>,
}

pub struct FieldValue<T, R> {
    _marker: core::marker::PhantomData<(T, R)>,
}

impl<T, R> FieldValue<T, R> {
    pub fn new(_mask: T, _value: T) -> Self {
        Self { _marker: core::marker::PhantomData }
    }
}

impl<T, R> Field<T, R> {
    pub fn val(&self, _raw: T) -> FieldValue<T, R> {
        FieldValue { _marker: core::marker::PhantomData }
    }
    pub fn read(&self, _raw: T) -> T {
        _raw
    }
}

fn field_val<R>(field: Field<u32, R>, raw: u32) -> FieldValue<u32, R> {
    FieldValue::new(field.mask, raw << field.shift)
}

fn raw_reg_val<R>(mask: u32, value: u32) -> FieldValue<u32, R> {
    FieldValue::new(mask, value)
}

fn field_from_raw<R>(field: Field<u32, R>, raw: u32) -> u32 {
    (raw & field.mask) >> field.shift
}

#[rseq::driver(name = "test-drv", compatible = ["vendor,test"])]
pub struct TestDriver;
"#;

// ---------------------------------------------------------------------------
// Precondition: fixture-source fidelity
// ---------------------------------------------------------------------------

#[test]
fn precondition_fixture_source_contains_authored_utility_functions() {
    let source = read_fixture_source();

    // Verify each authored signature is present in the fixture source.
    assert!(
        source.contains(AUTHORED_FIELD_VAL_SIG),
        "fixture must contain field_val signature: {AUTHORED_FIELD_VAL_SIG}"
    );
    assert!(
        source.contains(AUTHORED_RAW_REG_VAL_SIG),
        "fixture must contain raw_reg_val signature: {AUTHORED_RAW_REG_VAL_SIG}"
    );
    assert!(
        source.contains(AUTHORED_FIELD_FROM_RAW_SIG),
        "fixture must contain field_from_raw signature: {AUTHORED_FIELD_FROM_RAW_SIG}"
    );

    // Verify each authored body expression is present in the fixture source.
    assert!(
        source.contains(AUTHORED_FIELD_VAL_BODY),
        "fixture must contain field_val body: {AUTHORED_FIELD_VAL_BODY}"
    );
    assert!(
        source.contains(AUTHORED_RAW_REG_VAL_BODY),
        "fixture must contain raw_reg_val body: {AUTHORED_RAW_REG_VAL_BODY}"
    );
    assert!(
        source.contains(AUTHORED_FIELD_FROM_RAW_BODY),
        "fixture must contain field_from_raw body: {AUTHORED_FIELD_FROM_RAW_BODY}"
    );
}

// ---------------------------------------------------------------------------
// Precondition: specimen fidelity against fixture source
// ---------------------------------------------------------------------------

#[test]
fn precondition_specimen_fidelity_against_fixture() {
    let source = read_fixture_source();

    // The specimen contains the exact same three function definitions as the
    // fixture. Verify each function name, signature, and body expression appear
    // in both the specimen and the fixture.
    for name in AUTHORED_UTILITY_FN_NAMES {
        assert!(
            SPECIMEN.contains(name),
            "specimen must contain utility function: {name}"
        );
        assert!(
            source.contains(name),
            "fixture must contain utility function: {name}"
        );
    }

    // Verify exact signature text appears in both.
    for sig in &[
        AUTHORED_FIELD_VAL_SIG,
        AUTHORED_RAW_REG_VAL_SIG,
        AUTHORED_FIELD_FROM_RAW_SIG,
    ] {
        assert!(
            SPECIMEN.contains(sig),
            "specimen must contain signature: {sig}"
        );
        assert!(
            source.contains(sig),
            "fixture must contain signature: {sig}"
        );
    }

    // Verify exact body expressions appear in both.
    for body_expr in &[
        AUTHORED_FIELD_VAL_BODY,
        AUTHORED_RAW_REG_VAL_BODY,
        AUTHORED_FIELD_FROM_RAW_BODY,
    ] {
        assert!(
            SPECIMEN.contains(body_expr),
            "specimen must contain body: {body_expr}"
        );
        assert!(
            source.contains(body_expr),
            "fixture must contain body: {body_expr}"
        );
    }
}

// ---------------------------------------------------------------------------
// Check Group 1: identity, canonical path preservation
// ---------------------------------------------------------------------------

#[test]
fn check_group_1_free_function_identity_name_and_canonical_path() {
    let result = compile_and_extract_dual(SPECIMEN, "s2e04_identity");
    let surface = result.identity.expect("identity extraction should succeed");

    // All three utility functions must appear in free_functions by name.
    let fn_names: Vec<&str> = surface
        .free_functions
        .iter()
        .map(|f| f.name.as_str())
        .collect();

    for name in AUTHORED_UTILITY_FN_NAMES {
        assert!(
            fn_names.contains(name),
            "free_functions must contain '{name}'; found: {fn_names:?}"
        );
    }

    // Canonical paths: in an inline single-file specimen the crate name is
    // `test_input` (derived from filename). The functions are at crate root,
    // so canonical paths are `test_input::<fn_name>` or just `<fn_name>`
    // (rustc may omit crate name for root-level items).
    for name in AUTHORED_UTILITY_FN_NAMES {
        let func = surface
            .free_functions
            .iter()
            .find(|f| f.name == *name)
            .unwrap_or_else(|| panic!("function '{name}' not found in free_functions"));
        let cp = &func.canonical_path.0;
        assert!(
            cp == *name || cp.ends_with(&format!("::{name}")),
            "canonical_path for '{name}' should end with the function name; got: {cp}"
        );
    }

    // Exactly three utility functions expected (plus possibly the
    // constructor/helper functions from supporting types, but the check is
    // that all three authored ones are present).
    let authored_count = AUTHORED_UTILITY_FN_NAMES
        .iter()
        .filter(|n| fn_names.contains(*n))
        .count();
    assert_eq!(
        authored_count,
        AUTHORED_UTILITY_FN_NAMES.len(),
        "all three authored utility functions must be present in free_functions"
    );
}

#[test]
fn check_group_1_identity_surface_lacks_generics_params_return_type() {
    // FunctionFact only contains name, canonical_path, module_path.
    // It does NOT carry generics, parameter list, or return type.
    // This test documents that the identity enrichment surface does not
    // preserve those declaration facts.
    let result = compile_and_extract_dual(SPECIMEN, "s2e04_identity_schema");
    let surface = result.identity.expect("identity extraction should succeed");

    for name in AUTHORED_UTILITY_FN_NAMES {
        let func = surface
            .free_functions
            .iter()
            .find(|f| f.name == *name)
            .unwrap_or_else(|| panic!("function '{name}' not found"));

        // FunctionFact has exactly three fields: name, canonical_path, module_path.
        // There are no fields for generics, parameters, or return type.
        // We verify the fields that ARE present:
        assert!(!func.name.is_empty(), "name must be non-empty for '{name}'");
        assert!(
            !func.canonical_path.0.is_empty(),
            "canonical_path must be non-empty for '{name}'"
        );
        assert!(
            !func.module_path.0.is_empty(),
            "module_path must be non-empty for '{name}'"
        );
    }
}

// ---------------------------------------------------------------------------
// Check Group 2: body extraction for free functions
// ---------------------------------------------------------------------------

#[test]
fn check_group_2_body_surface_does_not_contain_free_functions() {
    // The body extraction surface (MirBodySurface) only extracts bodies for
    // roots discovered via impl blocks on the driver type: callbacks, ISRs,
    // and transitively-reachable helper methods. Free functions at module
    // scope are NOT roots and NOT helper candidates (helper candidates are
    // only inherent impl methods on the driver type).
    //
    // Therefore, the three utility free functions should NOT appear in
    // MirBodySurface.functions.
    let result = compile_and_extract_dual(SPECIMEN, "s2e04_body");
    let body = result.body.expect("body extraction should succeed");

    let body_fn_names: Vec<&str> = body.functions.iter().map(|f| f.name.as_str()).collect();

    for name in AUTHORED_UTILITY_FN_NAMES {
        assert!(
            !body_fn_names.contains(name),
            "free function '{name}' should NOT appear in MirBodySurface.functions \
             because free functions are not body-extraction roots; \
             found functions: {body_fn_names:?}"
        );
    }
}

#[test]
fn check_group_2_free_function_body_extraction_root_absence() {
    // This test specifically documents that free functions are absent from
    // body extraction because `discover_roots` in body.rs only scans impl
    // blocks on the `#[rseq::driver]` type. Free functions are outside that
    // scope. This is the root-cause explanation for the body-extraction gap.
    let result = compile_and_extract_dual(SPECIMEN, "s2e04_root_absence");
    let body = result.body.expect("body extraction should succeed");

    // No function in the body surface has kind == FreeFunction.
    let free_fn_count = body
        .functions
        .iter()
        .filter(|f| matches!(f.kind, MirFnKind::FreeFunction))
        .count();
    assert_eq!(
        free_fn_count, 0,
        "body surface should contain zero FreeFunction-kind entries; \
         root discovery only considers impl blocks on the driver type"
    );
}
