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
//! functions inside a `mod driver` block to preserve the authored module
//! nesting. Fidelity is verified against the real fixture source at
//! `tests/fixtures/pl011-full-spec/src/driver.rs`.
//!
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

/// Authored canonical path suffixes (module-qualified).
const AUTHORED_FIELD_VAL_PATH_SUFFIX: &str = "driver::field_val";
const AUTHORED_RAW_REG_VAL_PATH_SUFFIX: &str = "driver::raw_reg_val";
const AUTHORED_FIELD_FROM_RAW_PATH_SUFFIX: &str = "driver::field_from_raw";

/// Authored signatures (for fixture-fidelity verification).
const AUTHORED_FIELD_VAL_SIG: &str =
    "fn field_val<R>(field: Field<u32, R>, raw: u32) -> FieldValue<u32, R>";
const AUTHORED_RAW_REG_VAL_SIG: &str =
    "fn raw_reg_val<R>(mask: u32, value: u32) -> FieldValue<u32, R>";
const AUTHORED_FIELD_FROM_RAW_SIG: &str =
    "fn field_from_raw<R>(field: Field<u32, R>, raw: u32) -> u32";

/// Actual body expressions from the fixture source (used for fidelity checks).
/// These are the REAL bodies, NOT the check spec's "exact body effects" which
/// contain transcription errors.
const FIXTURE_FIELD_VAL_BODY: &str = "FieldValue::new(field.mask, raw << field.shift)";
const FIXTURE_RAW_REG_VAL_BODY: &str = "FieldValue::new(mask, value)";
const FIXTURE_FIELD_FROM_RAW_BODY: &str = "(raw & field.mask) >> field.shift";

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
        "--crate-name".to_string(),
        "pl011_full_spec".to_string(),
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

/// Reduced inline specimen that mirrors the three PL011 utility functions
/// inside a `mod driver` block to preserve the authored `driver::` module
/// nesting from the real fixture (`pl011_full_spec::driver::*`).
///
/// The specimen preserves:
/// - `mod driver` module nesting (authored canonical paths use `driver::`)
/// - all three function names exactly
/// - generic parameter `R` on each function
/// - parameter names and types
/// - return types
/// - body expressions matching the fixture source
///
/// The `#[rseq::driver]` struct is placed at crate root (outside the driver
/// module) because body extraction requires it at root discovery scope.
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

mod driver {
    use super::{Field, FieldValue};

    pub fn field_val<R>(field: Field<u32, R>, raw: u32) -> FieldValue<u32, R> {
        FieldValue::new(field.mask, raw << field.shift)
    }

    pub fn raw_reg_val<R>(mask: u32, value: u32) -> FieldValue<u32, R> {
        FieldValue::new(mask, value)
    }

    pub fn field_from_raw<R>(field: Field<u32, R>, raw: u32) -> u32 {
        (raw & field.mask) >> field.shift
    }
}

#[rseq::driver(name = "test-drv", compatible = ["vendor,test"])]
pub struct TestDriver;
"#;

// ---------------------------------------------------------------------------
// Precondition: fixture-source fidelity (signatures)
// ---------------------------------------------------------------------------

#[test]
fn precondition_fixture_source_contains_authored_signatures() {
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
        "fixture must contain field_from_raw signature: \
         {AUTHORED_FIELD_FROM_RAW_SIG}"
    );
}

// ---------------------------------------------------------------------------
// Precondition: fixture-source body fidelity (actual bodies)
// ---------------------------------------------------------------------------

#[test]
fn precondition_fixture_source_contains_actual_body_expressions() {
    let source = read_fixture_source();

    // Verify the REAL body expressions from the fixture source.
    assert!(
        source.contains(FIXTURE_FIELD_VAL_BODY),
        "fixture must contain field_val body: {FIXTURE_FIELD_VAL_BODY}"
    );
    assert!(
        source.contains(FIXTURE_RAW_REG_VAL_BODY),
        "fixture must contain raw_reg_val body: {FIXTURE_RAW_REG_VAL_BODY}"
    );
    assert!(
        source.contains(FIXTURE_FIELD_FROM_RAW_BODY),
        "fixture must contain field_from_raw body: \
         {FIXTURE_FIELD_FROM_RAW_BODY}"
    );
}

// ---------------------------------------------------------------------------
// Precondition: specimen fidelity against fixture source
// ---------------------------------------------------------------------------

#[test]
fn precondition_specimen_fidelity_against_fixture() {
    let source = read_fixture_source();

    // The specimen contains the exact same three function definitions as the
    // fixture. Verify each function name, signature, and body expression
    // appear in both the specimen and the fixture.
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

    // Verify exact body expressions (from fixture, not spec) appear in both.
    for body_expr in &[
        FIXTURE_FIELD_VAL_BODY,
        FIXTURE_RAW_REG_VAL_BODY,
        FIXTURE_FIELD_FROM_RAW_BODY,
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

    // Verify the specimen preserves `mod driver` nesting.
    assert!(
        SPECIMEN.contains("mod driver"),
        "specimen must contain `mod driver` to preserve authored module \
         nesting"
    );
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

    // Canonical paths must preserve enough information to reconstruct the full
    // authored path `pl011_full_spec::driver::<fn_name>` exactly.
    let crate_root = surface
        .modules
        .iter()
        .find(|m| m.name == "pl011_full_spec")
        .expect("crate root module pl011_full_spec must exist");
    assert_eq!(
        crate_root.canonical_path.0, "pl011_full_spec",
        "crate root canonical_path must be exactly 'pl011_full_spec'"
    );

    let driver_module = surface
        .modules
        .iter()
        .find(|m| m.name == "driver")
        .expect("driver module must exist");
    assert!(
        crate_root
            .child_items
            .iter()
            .any(|c| c.0 == driver_module.canonical_path.0),
        "driver module must be a child of the crate root"
    );

    let path_suffixes = [
        ("field_val", AUTHORED_FIELD_VAL_PATH_SUFFIX),
        ("raw_reg_val", AUTHORED_RAW_REG_VAL_PATH_SUFFIX),
        ("field_from_raw", AUTHORED_FIELD_FROM_RAW_PATH_SUFFIX),
    ];

    for (name, expected_suffix) in &path_suffixes {
        let func = surface
            .free_functions
            .iter()
            .find(|f| f.name == *name)
            .unwrap_or_else(|| panic!("function '{name}' not found in free_functions"));
        let cp = &func.canonical_path.0;
        assert_eq!(
            cp, expected_suffix,
            "source-local canonical_path for '{name}' must be '{expected_suffix}'; got: {cp}"
        );
        assert_eq!(
            func.module_path.0, "driver",
            "module_path for '{name}' must be exactly 'driver'; got: {}",
            func.module_path.0
        );
    }

    // Verify all three authored utility functions are present.
    let authored_count = AUTHORED_UTILITY_FN_NAMES
        .iter()
        .filter(|n| fn_names.contains(*n))
        .count();
    assert_eq!(
        authored_count,
        AUTHORED_UTILITY_FN_NAMES.len(),
        "all three authored utility functions must be present in \
         free_functions"
    );
}

// ---------------------------------------------------------------------------
// Check Group 1b: generics, parameter list, and return type preservation
// ---------------------------------------------------------------------------

#[test]
fn check_group_1b_identity_surface_preserves_generics_params_return_type() {
    let result = compile_and_extract_dual(SPECIMEN, "s2e04_identity_sig");
    let surface = result.identity.expect("identity extraction should succeed");

    // field_val: fn field_val<R>(field: Field<u32, R>, raw: u32) -> FieldValue<u32, R>
    let fv = surface
        .free_functions
        .iter()
        .find(|f| f.name == "field_val")
        .expect("field_val must exist in free_functions");
    assert_eq!(fv.generics, vec!["R"], "field_val generics");
    assert_eq!(fv.params.len(), 2, "field_val param count");
    assert_eq!(fv.params[0].name, "field", "field_val param[0] name");
    assert_eq!(fv.params[0].ty, "Field<u32, R>", "field_val param[0] type");
    assert_eq!(fv.params[1].name, "raw", "field_val param[1] name");
    assert_eq!(fv.params[1].ty, "u32", "field_val param[1] type");
    assert_eq!(
        fv.return_type.as_deref(),
        Some("FieldValue<u32, R>"),
        "field_val return type"
    );

    // raw_reg_val: fn raw_reg_val<R>(mask: u32, value: u32) -> FieldValue<u32, R>
    let rrv = surface
        .free_functions
        .iter()
        .find(|f| f.name == "raw_reg_val")
        .expect("raw_reg_val must exist in free_functions");
    assert_eq!(rrv.generics, vec!["R"], "raw_reg_val generics");
    assert_eq!(rrv.params.len(), 2, "raw_reg_val param count");
    assert_eq!(rrv.params[0].name, "mask", "raw_reg_val param[0] name");
    assert_eq!(rrv.params[0].ty, "u32", "raw_reg_val param[0] type");
    assert_eq!(rrv.params[1].name, "value", "raw_reg_val param[1] name");
    assert_eq!(rrv.params[1].ty, "u32", "raw_reg_val param[1] type");
    assert_eq!(
        rrv.return_type.as_deref(),
        Some("FieldValue<u32, R>"),
        "raw_reg_val return type"
    );

    // field_from_raw: fn field_from_raw<R>(field: Field<u32, R>, raw: u32) -> u32
    let ffr = surface
        .free_functions
        .iter()
        .find(|f| f.name == "field_from_raw")
        .expect("field_from_raw must exist in free_functions");
    assert_eq!(ffr.generics, vec!["R"], "field_from_raw generics");
    assert_eq!(ffr.params.len(), 2, "field_from_raw param count");
    assert_eq!(ffr.params[0].name, "field", "field_from_raw param[0] name");
    assert_eq!(
        ffr.params[0].ty, "Field<u32, R>",
        "field_from_raw param[0] type"
    );
    assert_eq!(ffr.params[1].name, "raw", "field_from_raw param[1] name");
    assert_eq!(ffr.params[1].ty, "u32", "field_from_raw param[1] type");
    assert_eq!(
        ffr.return_type.as_deref(),
        Some("u32"),
        "field_from_raw return type"
    );
}

// ---------------------------------------------------------------------------
// MIR body helpers
// ---------------------------------------------------------------------------

fn operand_mentions(op: &MirOperand, substr: &str) -> bool {
    match op {
        MirOperand::Constant { constant } => match &constant.ty {
            MirTy::Fn { path, .. } => path.contains(substr),
            _ => false,
        },
        _ => false,
    }
}

fn operand_is_local(op: &MirOperand, local: u32) -> bool {
    match op {
        MirOperand::Copy { place } | MirOperand::Move { place } => place.local == local,
        MirOperand::Constant { .. } => false,
    }
}

fn place_is_field_projection(place: &MirPlace, base_local: u32, field_index: u32) -> bool {
    place.local == base_local
        && place.projection.iter().any(|proj| {
            matches!(
                proj,
                MirProjectionElem::Field {
                    index,
                    ..
                } if *index == field_index
            )
        })
}

fn operand_is_field_projection(op: &MirOperand, base_local: u32, field_index: u32) -> bool {
    match op {
        MirOperand::Copy { place } | MirOperand::Move { place } => {
            place_is_field_projection(place, base_local, field_index)
        }
        MirOperand::Constant { .. } => false,
    }
}

fn find_binary_op_assign_local<F, G>(
    body: &MirBody,
    expected_op: MirBinOp,
    lhs_matches: F,
    rhs_matches: G,
) -> Option<u32>
where
    F: Fn(&MirOperand) -> bool,
    G: Fn(&MirOperand) -> bool,
{
    for bb in &body.basic_blocks {
        if bb.is_cleanup {
            continue;
        }
        for stmt in &bb.statements {
            let MirStatementKind::Assign { place, rvalue } = &stmt.kind else {
                continue;
            };
            let MirRvalue::BinaryOp { op, lhs, rhs } = rvalue.as_ref() else {
                continue;
            };
            if *op == expected_op && lhs_matches(lhs) && rhs_matches(rhs) {
                return Some(place.local);
            }
        }
    }
    None
}

fn has_call_with_two_args<F, G>(
    body: &MirBody,
    callee_substr: &str,
    arg0_matches: F,
    arg1_matches: G,
) -> bool
where
    F: Fn(&MirOperand) -> bool,
    G: Fn(&MirOperand) -> bool,
{
    body.basic_blocks.iter().any(|bb| {
        if bb.is_cleanup {
            return false;
        }
        let Some(term) = &bb.terminator else {
            return false;
        };
        let MirTerminatorKind::Call { func, args, .. } = &term.kind else {
            return false;
        };
        args.len() == 2
            && operand_mentions(func, callee_substr)
            && arg0_matches(&args[0])
            && arg1_matches(&args[1])
    })
}

fn find_use_assign_local<F>(body: &MirBody, operand_matches: F) -> Option<u32>
where
    F: Fn(&MirOperand) -> bool,
{
    for bb in &body.basic_blocks {
        if bb.is_cleanup {
            continue;
        }
        for stmt in &bb.statements {
            let MirStatementKind::Assign { place, rvalue } = &stmt.kind else {
                continue;
            };
            let MirRvalue::Use { operand } = rvalue.as_ref() else {
                continue;
            };
            if operand_matches(operand) {
                return Some(place.local);
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Check Group 2: body extraction for free functions
// ---------------------------------------------------------------------------

#[test]
fn check_group_2_body_surface_contains_free_functions() {
    let result = compile_and_extract_dual(SPECIMEN, "s2e04_body");
    let body = result.body.expect("body extraction should succeed");

    let body_fn_names: Vec<&str> = body.functions.iter().map(|f| f.name.as_str()).collect();

    // All three utility free functions must appear in MirBodySurface.functions.
    for name in AUTHORED_UTILITY_FN_NAMES {
        assert!(
            body_fn_names.contains(name),
            "free function '{name}' must appear in \
             MirBodySurface.functions; found: {body_fn_names:?}"
        );
    }

    // All three must have kind == FreeFunction.
    for name in AUTHORED_UTILITY_FN_NAMES {
        let func = body
            .functions
            .iter()
            .find(|f| f.name == *name)
            .unwrap_or_else(|| panic!("function '{name}' must be in body surface"));
        assert_eq!(
            func.kind,
            MirFnKind::FreeFunction,
            "{name} must have kind FreeFunction"
        );
    }
}

#[test]
fn check_group_2_field_val_body_proves_authored_facts() {
    // field_val body: FieldValue::new(field.mask, raw << field.shift)
    //   - callee: FieldValue::new
    //   - arg0: field.mask (field access on param 0)
    //   - arg1: raw << field.shift (Shl of param 1 and field.shift)
    let result = compile_and_extract_dual(SPECIMEN, "s2e04_body_fv");
    let body_surface = result.body.expect("body extraction should succeed");

    let func = body_surface
        .functions
        .iter()
        .find(|f| f.name == "field_val")
        .expect("field_val must be in body surface");

    // Verify function has a body with basic blocks.
    assert!(
        !func.body.basic_blocks.is_empty(),
        "field_val body must have basic blocks"
    );
    assert_eq!(
        func.body.arg_count, 2,
        "field_val body must have exactly 2 parameter locals"
    );

    // Param locals are 1 and 2: `field`, `raw`.
    let mask_local = find_use_assign_local(&func.body, |operand| {
        operand_is_field_projection(operand, 1, 0)
    })
    .expect("field_val must copy `field.mask` into a temp local");
    let raw_local = find_use_assign_local(&func.body, |operand| operand_is_local(operand, 2))
        .expect("field_val must copy `raw` into a temp local");
    let shift_local = find_use_assign_local(&func.body, |operand| {
        operand_is_field_projection(operand, 1, 1)
    })
    .expect("field_val must copy `field.shift` into a temp local");

    let shl_local = find_binary_op_assign_local(
        &func.body,
        MirBinOp::Shl,
        |lhs| operand_is_local(lhs, raw_local),
        |rhs| operand_is_local(rhs, shift_local),
    )
    .expect("field_val must compute `raw << field.shift` from copied operand locals");

    assert!(
        has_call_with_two_args(
            &func.body,
            "FieldValue",
            |arg0| operand_is_local(arg0, mask_local),
            |arg1| operand_is_local(arg1, shl_local),
        ),
        "field_val must call FieldValue::new(mask_temp, shl_temp) where those temps come from `field.mask` and `raw << field.shift`"
    );
}

#[test]
fn check_group_2_raw_reg_val_body_proves_authored_facts() {
    // raw_reg_val body: FieldValue::new(mask, value)
    //   - callee: FieldValue::new
    //   - args: direct parameter forwarding (mask, value)
    let result = compile_and_extract_dual(SPECIMEN, "s2e04_body_rrv");
    let body_surface = result.body.expect("body extraction should succeed");

    let func = body_surface
        .functions
        .iter()
        .find(|f| f.name == "raw_reg_val")
        .expect("raw_reg_val must be in body surface");

    assert!(
        !func.body.basic_blocks.is_empty(),
        "raw_reg_val body must have basic blocks"
    );
    let mask_local = find_use_assign_local(&func.body, |operand| operand_is_local(operand, 1))
        .expect("raw_reg_val must copy `mask` into a temp local");
    let value_local = find_use_assign_local(&func.body, |operand| operand_is_local(operand, 2))
        .expect("raw_reg_val must copy `value` into a temp local");
    assert!(
        has_call_with_two_args(
            &func.body,
            "FieldValue",
            |arg0| operand_is_local(arg0, mask_local),
            |arg1| operand_is_local(arg1, value_local),
        ),
        "raw_reg_val must call FieldValue::new(mask_temp, value_temp) where those temps come directly from `mask` and `value`"
    );
}

#[test]
fn check_group_2_field_from_raw_body_proves_authored_facts() {
    // field_from_raw body: (raw & field.mask) >> field.shift
    //   - BitAnd (raw & field.mask)
    //   - Shr (result >> field.shift)
    let result = compile_and_extract_dual(SPECIMEN, "s2e04_body_ffr");
    let body_surface = result.body.expect("body extraction should succeed");

    let func = body_surface
        .functions
        .iter()
        .find(|f| f.name == "field_from_raw")
        .expect("field_from_raw must be in body surface");

    assert!(
        !func.body.basic_blocks.is_empty(),
        "field_from_raw body must have basic blocks"
    );
    assert_eq!(
        func.body.arg_count, 2,
        "field_from_raw body must have exactly 2 parameter locals"
    );

    let raw_local = find_use_assign_local(&func.body, |operand| operand_is_local(operand, 2))
        .expect("field_from_raw must copy `raw` into a temp local");
    let mask_local = find_use_assign_local(&func.body, |operand| {
        operand_is_field_projection(operand, 1, 0)
    })
    .expect("field_from_raw must copy `field.mask` into a temp local");
    let bitand_local = find_binary_op_assign_local(
        &func.body,
        MirBinOp::BitAnd,
        |lhs| operand_is_local(lhs, raw_local),
        |rhs| operand_is_local(rhs, mask_local),
    )
    .expect("field_from_raw must compute `raw & field.mask` from copied operand locals");

    let shift_local = find_use_assign_local(&func.body, |operand| {
        operand_is_field_projection(operand, 1, 1)
    })
    .expect("field_from_raw must copy `field.shift` into a temp local");
    let shr_local = find_binary_op_assign_local(
        &func.body,
        MirBinOp::Shr,
        |lhs| operand_is_local(lhs, bitand_local),
        |rhs| operand_is_local(rhs, shift_local),
    )
    .expect("field_from_raw must compute `(raw & field.mask) >> field.shift`");

    let return_assigns_shr = func.body.basic_blocks.iter().any(|bb| {
        if bb.is_cleanup {
            return false;
        }
        bb.statements.iter().any(|stmt| match &stmt.kind {
            MirStatementKind::Assign { place, rvalue } if place.local == 0 => match rvalue.as_ref()
            {
                MirRvalue::BinaryOp { op, lhs, rhs } => {
                    *op == MirBinOp::Shr
                        && operand_is_local(lhs, bitand_local)
                        && operand_is_local(rhs, shift_local)
                        && place.local == shr_local
                }
                _ => false,
            },
            _ => false,
        })
    });
    assert!(
        return_assigns_shr,
        "field_from_raw return place must be assigned the exact `(raw & field.mask) >> field.shift` binary operation"
    );
}
