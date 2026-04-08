//! source-to-extraction-06: PL011 driver constants authored->extraction boundary
//! validation.
//!
//! Validates that the authored driver-constant facts for PL011 survive at the
//! compiler-side boundary between the authored PL011 source and the current
//! Regseq extraction state.
//!
//! Extraction surfaces under test:
//! - `IdentityEnrichmentSurface.impl_blocks` (constant identity, type, owner path)
//! - `MirBodySurface.functions` (inlined constant values in function bodies)

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

use identity_enrichment::*;
use mir_repr::*;

// ---------------------------------------------------------------------------
// Shared compiler harness
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

struct ExtractionResult {
    identity: IdentityEnrichmentSurface,
    body: MirBodySurface,
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
            *self.body_result.lock().unwrap() = Some(body::extract_body_surface(tcx));
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
        .expect("after_analysis should have been called")
        .expect("identity enrichment extraction should succeed");

    let body = body_result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis should have been called")
        .expect("body extraction should succeed");

    std::fs::remove_dir_all(&test_dir).ok();

    ExtractionResult { identity, body }
}

// ---------------------------------------------------------------------------
// Inline specimen: reduced PL011 driver constants surface
// ---------------------------------------------------------------------------

/// Reduced specimen covering all authored driver-constant facts:
/// - `ALL_UART_INTERRUPTS_MASK: u32 = 0x07ff`
/// - `RX_INTERRUPT_MASK: u32 = (1 << 4) | (1 << 6)`
/// - `TX_INTERRUPT_MASK: u32 = 1 << 5`
/// - `ERROR_INTERRUPT_MASK: u32 = (1 << 7) | (1 << 8) | (1 << 9) | (1 << 10)`
/// - `EXPLICIT_ISR_CLEAR_MASK: u32 = Self::ERROR_INTERRUPT_MASK`
/// - `DEFAULT_UART_CLOCK_HZ: u32 = 48_000_000`
///
/// Compiled with `--crate-name pl011_full_spec` so that canonical paths match
/// the real PL011 fixture exactly.
///
/// Includes a helper function `use_constants` that references all constants
/// to ensure they appear in MIR body extraction as inlined scalar values.
const SPECIMEN: &str = r#"
    #![allow(dead_code)]

    pub mod driver {
        #[rseq::driver(name = "pl011-full-spec", compatible = ["arm,pl011"])]
        pub struct Pl011FullSpec {
            pub dummy: u32,
        }

        impl Pl011FullSpec {
            const ALL_UART_INTERRUPTS_MASK: u32 = 0x07ff;
            const RX_INTERRUPT_MASK: u32 = (1 << 4) | (1 << 6);
            const TX_INTERRUPT_MASK: u32 = 1 << 5;
            const ERROR_INTERRUPT_MASK: u32 = (1 << 7) | (1 << 8) | (1 << 9) | (1 << 10);
            const EXPLICIT_ISR_CLEAR_MASK: u32 = Self::ERROR_INTERRUPT_MASK;
            const DEFAULT_UART_CLOCK_HZ: u32 = 48_000_000;

            /// Helper that uses all constants so they appear in MIR body extraction.
            fn use_constants(&self) -> [u32; 6] {
                [
                    Self::ALL_UART_INTERRUPTS_MASK,
                    Self::RX_INTERRUPT_MASK,
                    Self::TX_INTERRUPT_MASK,
                    Self::ERROR_INTERRUPT_MASK,
                    Self::EXPLICIT_ISR_CLEAR_MASK,
                    Self::DEFAULT_UART_CLOCK_HZ,
                ]
            }

            #[rseq::isr]
            pub fn handle_irq(&mut self) -> u32 {
                let vals = self.use_constants();
                vals[0]
            }
        }
    }
"#;

// ---------------------------------------------------------------------------
// Authored constant facts (from check spec)
// ---------------------------------------------------------------------------

struct AuthoredConstant {
    name: &'static str,
    ty: &'static str,
    canonical_path: &'static str,
    /// Authored evaluated value.
    evaluated_value: u128,
}

const AUTHORED_CONSTANTS: &[AuthoredConstant] = &[
    AuthoredConstant {
        name: "ALL_UART_INTERRUPTS_MASK",
        ty: "u32",
        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::ALL_UART_INTERRUPTS_MASK",
        evaluated_value: 0x07ff,
    },
    AuthoredConstant {
        name: "RX_INTERRUPT_MASK",
        ty: "u32",
        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::RX_INTERRUPT_MASK",
        evaluated_value: (1 << 4) | (1 << 6),
    },
    AuthoredConstant {
        name: "TX_INTERRUPT_MASK",
        ty: "u32",
        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::TX_INTERRUPT_MASK",
        evaluated_value: 1 << 5,
    },
    AuthoredConstant {
        name: "ERROR_INTERRUPT_MASK",
        ty: "u32",
        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::ERROR_INTERRUPT_MASK",
        evaluated_value: (1 << 7) | (1 << 8) | (1 << 9) | (1 << 10),
    },
    AuthoredConstant {
        name: "EXPLICIT_ISR_CLEAR_MASK",
        ty: "u32",
        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::EXPLICIT_ISR_CLEAR_MASK",
        // Self::ERROR_INTERRUPT_MASK == (1 << 7) | (1 << 8) | (1 << 9) | (1 << 10)
        evaluated_value: (1 << 7) | (1 << 8) | (1 << 9) | (1 << 10),
    },
    AuthoredConstant {
        name: "DEFAULT_UART_CLOCK_HZ",
        ty: "u32",
        canonical_path: "pl011_full_spec::driver::Pl011FullSpec::DEFAULT_UART_CLOCK_HZ",
        evaluated_value: 48_000_000,
    },
];

// ---------------------------------------------------------------------------
// Precondition: specimen fidelity against real fixture source
// ---------------------------------------------------------------------------

/// Reads the real PL011 fixture source and verifies that the specimen's
/// authored facts match the real fixture for every fact in scope.
#[test]
fn precondition_specimen_fidelity() {
    let fixture = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/pl011-full-spec/src/driver.rs"
    ))
    .expect("fixture source must be readable");

    // Verify each authored constant declaration exists in the fixture source
    // exactly as stated in the check specification.

    // Fact 1: ALL_UART_INTERRUPTS_MASK
    assert!(
        fixture.contains("const ALL_UART_INTERRUPTS_MASK: u32 = 0x07ff;"),
        "fixture must contain `const ALL_UART_INTERRUPTS_MASK: u32 = 0x07ff;`"
    );

    // Fact 2: RX_INTERRUPT_MASK
    assert!(
        fixture.contains("const RX_INTERRUPT_MASK: u32 = (1 << 4) | (1 << 6);"),
        "fixture must contain `const RX_INTERRUPT_MASK: u32 = (1 << 4) | (1 << 6);`"
    );

    // Fact 3: TX_INTERRUPT_MASK
    assert!(
        fixture.contains("const TX_INTERRUPT_MASK: u32 = 1 << 5;"),
        "fixture must contain `const TX_INTERRUPT_MASK: u32 = 1 << 5;`"
    );

    // Fact 4: ERROR_INTERRUPT_MASK
    assert!(
        fixture.contains(
            "const ERROR_INTERRUPT_MASK: u32 = (1 << 7) | (1 << 8) | (1 << 9) | (1 << 10);"
        ),
        "fixture must contain `const ERROR_INTERRUPT_MASK: u32 = (1 << 7) | (1 << 8) | (1 << 9) | (1 << 10);`"
    );

    // Fact 5: EXPLICIT_ISR_CLEAR_MASK = Self::ERROR_INTERRUPT_MASK
    assert!(
        fixture.contains("const EXPLICIT_ISR_CLEAR_MASK: u32 = Self::ERROR_INTERRUPT_MASK;"),
        "fixture must contain `const EXPLICIT_ISR_CLEAR_MASK: u32 = Self::ERROR_INTERRUPT_MASK;`"
    );

    // Fact 6: DEFAULT_UART_CLOCK_HZ
    assert!(
        fixture.contains("const DEFAULT_UART_CLOCK_HZ: u32 = 48_000_000;"),
        "fixture must contain `const DEFAULT_UART_CLOCK_HZ: u32 = 48_000_000;`"
    );

    // Fact 7: All constants are inside `impl Pl011FullSpec`
    assert!(
        fixture.contains("impl Pl011FullSpec {"),
        "fixture must contain `impl Pl011FullSpec`"
    );

    // Verify specimen faithfully reproduces all constant declarations
    assert!(
        SPECIMEN.contains("const ALL_UART_INTERRUPTS_MASK: u32 = 0x07ff;"),
        "specimen must contain ALL_UART_INTERRUPTS_MASK declaration"
    );
    assert!(
        SPECIMEN.contains("const RX_INTERRUPT_MASK: u32 = (1 << 4) | (1 << 6);"),
        "specimen must contain RX_INTERRUPT_MASK declaration"
    );
    assert!(
        SPECIMEN.contains("const TX_INTERRUPT_MASK: u32 = 1 << 5;"),
        "specimen must contain TX_INTERRUPT_MASK declaration"
    );
    assert!(
        SPECIMEN.contains(
            "const ERROR_INTERRUPT_MASK: u32 = (1 << 7) | (1 << 8) | (1 << 9) | (1 << 10);"
        ),
        "specimen must contain ERROR_INTERRUPT_MASK declaration"
    );
    assert!(
        SPECIMEN.contains("const EXPLICIT_ISR_CLEAR_MASK: u32 = Self::ERROR_INTERRUPT_MASK;"),
        "specimen must contain EXPLICIT_ISR_CLEAR_MASK declaration"
    );
    assert!(
        SPECIMEN.contains("const DEFAULT_UART_CLOCK_HZ: u32 = 48_000_000;"),
        "specimen must contain DEFAULT_UART_CLOCK_HZ declaration"
    );

    // Verify struct and impl naming matches the full canonical path
    assert!(
        SPECIMEN.contains("pub struct Pl011FullSpec"),
        "specimen must contain `pub struct Pl011FullSpec`"
    );
    assert!(
        SPECIMEN.contains("impl Pl011FullSpec {"),
        "specimen must contain `impl Pl011FullSpec`"
    );
    assert!(
        SPECIMEN.contains("pub mod driver {"),
        "specimen must wrap in `pub mod driver` for canonical path fidelity"
    );
}

// ---------------------------------------------------------------------------
// Check Group 1: constant identity, type, owner path, and value preservation
// ---------------------------------------------------------------------------

/// Check 1a: All six authored constants are present in the identity enrichment
/// surface with correct names.
#[test]
fn check1a_all_constants_present_by_name() {
    let result = compile_and_extract(SPECIMEN, "s2e06_check1a");

    // Find the inherent impl block for Pl011FullSpec
    let inherent = result
        .identity
        .impl_blocks
        .iter()
        .find(|ib| ib.is_inherent && ib.self_type_name == "Pl011FullSpec")
        .expect("inherent impl block for Pl011FullSpec must exist");

    let extracted_names: Vec<&str> = inherent.constants.iter().map(|c| c.name.as_str()).collect();

    for authored in AUTHORED_CONSTANTS {
        assert!(
            extracted_names.contains(&authored.name),
            "constant '{}' must be present in extraction; found: {:?}",
            authored.name,
            extracted_names
        );
    }
}

/// Check 1b: All six authored constants have the correct Rust type (u32).
#[test]
fn check1b_constant_types_preserved() {
    let result = compile_and_extract(SPECIMEN, "s2e06_check1b");

    let inherent = result
        .identity
        .impl_blocks
        .iter()
        .find(|ib| ib.is_inherent && ib.self_type_name == "Pl011FullSpec")
        .expect("inherent impl block for Pl011FullSpec must exist");

    for authored in AUTHORED_CONSTANTS {
        let extracted = inherent
            .constants
            .iter()
            .find(|c| c.name == authored.name)
            .unwrap_or_else(|| panic!("constant '{}' must exist", authored.name));

        assert_eq!(
            extracted.ty, authored.ty,
            "constant '{}' type must be '{}', got '{}'",
            authored.name, authored.ty, extracted.ty
        );
    }
}

/// Check 1c: All six authored constants have canonical paths that, when
/// combined with the crate root module path, exactly reconstruct the full
/// authored canonical paths.
///
/// `tcx.def_path_str()` produces paths without the crate-name prefix.
/// The extraction stores the crate root module path separately
/// (as `pl011_full_spec`). The full authored canonical path is exactly
/// reconstructable as `crate_root_path + "::" + extracted_canonical_path`.
#[test]
fn check1c_canonical_paths_preserved() {
    let result = compile_and_extract(SPECIMEN, "s2e06_check1c");

    // The crate root module path must be "pl011_full_spec"
    let crate_root = result
        .identity
        .modules
        .iter()
        .find(|m| m.name == "pl011_full_spec")
        .expect("crate root module 'pl011_full_spec' must exist");

    assert_eq!(
        crate_root.canonical_path.0, "pl011_full_spec",
        "crate root canonical path must be 'pl011_full_spec'"
    );

    let inherent = result
        .identity
        .impl_blocks
        .iter()
        .find(|ib| ib.is_inherent && ib.self_type_name == "Pl011FullSpec")
        .expect("inherent impl block for Pl011FullSpec must exist");

    for authored in AUTHORED_CONSTANTS {
        let extracted = inherent
            .constants
            .iter()
            .find(|c| c.name == authored.name)
            .unwrap_or_else(|| panic!("constant '{}' must exist", authored.name));

        // The full canonical path is reconstructable by prepending the
        // crate root path to the extraction's canonical path.
        let reconstructed = format!(
            "{}::{}",
            crate_root.canonical_path.0, extracted.canonical_path.0
        );
        assert_eq!(
            reconstructed, authored.canonical_path,
            "constant '{}' reconstructed canonical path must be exactly '{}', \
             got '{}' (extracted: '{}')",
            authored.name, authored.canonical_path, reconstructed, extracted.canonical_path.0
        );
    }
}

/// Check 1d: All six authored constants point back to the owning inherent
/// impl block via `owner_impl_path`.
#[test]
fn check1d_owner_impl_path_preserved() {
    let result = compile_and_extract(SPECIMEN, "s2e06_check1d");

    let inherent = result
        .identity
        .impl_blocks
        .iter()
        .find(|ib| ib.is_inherent && ib.self_type_name == "Pl011FullSpec")
        .expect("inherent impl block for Pl011FullSpec must exist");

    for authored in AUTHORED_CONSTANTS {
        let extracted = inherent
            .constants
            .iter()
            .find(|c| c.name == authored.name)
            .unwrap_or_else(|| panic!("constant '{}' must exist", authored.name));

        assert_eq!(
            extracted.owner_impl_path, inherent.canonical_path,
            "constant '{}' owner_impl_path must match the inherent impl's canonical path; \
             got '{}', expected '{}'",
            authored.name, extracted.owner_impl_path.0, inherent.canonical_path.0
        );
    }
}

/// Check 1e: Closed-world evidence -- exactly six constants exist in the
/// inherent impl block, matching the authored set with no extras and no losses.
#[test]
fn check1e_closed_world_constant_set() {
    let result = compile_and_extract(SPECIMEN, "s2e06_check1e");

    let inherent = result
        .identity
        .impl_blocks
        .iter()
        .find(|ib| ib.is_inherent && ib.self_type_name == "Pl011FullSpec")
        .expect("inherent impl block for Pl011FullSpec must exist");

    assert_eq!(
        inherent.constants.len(),
        AUTHORED_CONSTANTS.len(),
        "expected exactly {} constants in the inherent impl block, got {}; \
         found: {:?}",
        AUTHORED_CONSTANTS.len(),
        inherent.constants.len(),
        inherent
            .constants
            .iter()
            .map(|c| &c.name)
            .collect::<Vec<_>>()
    );

    let authored_names: Vec<&str> = AUTHORED_CONSTANTS.iter().map(|a| a.name).collect();
    let extracted_names: Vec<&str> = inherent.constants.iter().map(|c| c.name.as_str()).collect();

    for name in &authored_names {
        assert!(
            extracted_names.contains(name),
            "authored constant '{}' missing from extraction",
            name
        );
    }
    for name in &extracted_names {
        assert!(
            authored_names.contains(name),
            "unexpected extra constant '{}' in extraction",
            name
        );
    }
}

/// Check 1f: Constant references survive in MIR body as unevaluated named
/// references.
///
/// The `IdentityEnrichmentSurface` does not carry constant values directly.
/// In MIR, associated constants appear as `MirConstValue::Unevaluated { path }`
/// where `path` is the def-path of the constant.
/// This preserves the constant name reference at use sites, but NOT the
/// evaluated literal value.
#[test]
fn check1f_constant_references_in_mir_body() {
    let result = compile_and_extract(SPECIMEN, "s2e06_check1f");

    // Find the use_constants function in the MIR body surface
    let use_constants_fn = result
        .body
        .functions
        .iter()
        .find(|f| f.name == "use_constants")
        .expect("use_constants helper must be extracted in MIR body surface");

    // Collect all unevaluated constant descriptions from the MIR body
    let unevaluated_refs = collect_unevaluated_constant_refs(&use_constants_fn.body);

    // Verify each authored constant appears as a named unevaluated reference.
    // Unevaluated constants now use MirConstValue::Unevaluated { path } where
    // path is the def_path_str (without crate prefix).
    for authored in AUTHORED_CONSTANTS {
        // Strip "pl011_full_spec::" prefix from authored canonical path
        let without_crate = authored
            .canonical_path
            .strip_prefix("pl011_full_spec::")
            .expect("authored canonical path must start with 'pl011_full_spec::'");
        assert!(
            unevaluated_refs.iter().any(|p| p == without_crate),
            "constant '{}' must appear as an unevaluated reference '{}' in MIR body; \
             found: {:?}",
            authored.name,
            without_crate,
            unevaluated_refs
        );
    }
}

/// Check 1g: Constant literal values are preserved as evaluated scalars on
/// the identity enrichment surface via `AssociatedConstantFact.evaluated_value`.
///
/// The extraction evaluates each associated constant via `tcx.const_eval_poly`
/// and records the resulting scalar in the `evaluated_value` field.
#[test]
fn check1g_constant_values_evaluated() {
    let result = compile_and_extract(SPECIMEN, "s2e06_check1g");

    let inherent = result
        .identity
        .impl_blocks
        .iter()
        .find(|ib| ib.is_inherent && ib.self_type_name == "Pl011FullSpec")
        .expect("inherent impl block must exist");

    for authored in AUTHORED_CONSTANTS {
        let extracted = inherent
            .constants
            .iter()
            .find(|c| c.name == authored.name)
            .unwrap_or_else(|| panic!("constant '{}' must exist", authored.name));

        assert_eq!(
            extracted.evaluated_value,
            Some(authored.evaluated_value),
            "constant '{}' evaluated_value must be Some({}), got {:?}",
            authored.name,
            authored.evaluated_value,
            extracted.evaluated_value
        );
    }
}

// ---------------------------------------------------------------------------
// Check Group 2: definitional relationships
// ---------------------------------------------------------------------------

/// Check 2a: Both `EXPLICIT_ISR_CLEAR_MASK` and `ERROR_INTERRUPT_MASK` survive
/// as distinct named constants in the identity enrichment surface and as
/// distinct unevaluated references in the MIR body surface.
#[test]
fn check2a_both_constants_survive_as_distinct_entries() {
    let result = compile_and_extract(SPECIMEN, "s2e06_check2a");

    let inherent = result
        .identity
        .impl_blocks
        .iter()
        .find(|ib| ib.is_inherent && ib.self_type_name == "Pl011FullSpec")
        .expect("inherent impl block must exist");

    // Both constants exist as distinct entries
    let explicit_isr = inherent
        .constants
        .iter()
        .find(|c| c.name == "EXPLICIT_ISR_CLEAR_MASK")
        .expect("EXPLICIT_ISR_CLEAR_MASK must exist");

    let error_mask = inherent
        .constants
        .iter()
        .find(|c| c.name == "ERROR_INTERRUPT_MASK")
        .expect("ERROR_INTERRUPT_MASK must exist");

    // Both are u32 with the same owner
    assert_eq!(explicit_isr.ty, "u32");
    assert_eq!(error_mask.ty, "u32");
    assert_eq!(explicit_isr.owner_impl_path, error_mask.owner_impl_path);

    // Distinct canonical paths
    assert_ne!(
        explicit_isr.canonical_path, error_mask.canonical_path,
        "the two constants must have distinct canonical paths"
    );

    // Both appear as unevaluated references in MIR
    let use_constants_fn = result
        .body
        .functions
        .iter()
        .find(|f| f.name == "use_constants")
        .expect("use_constants must be extracted");

    let unevaluated = collect_unevaluated_constant_refs(&use_constants_fn.body);

    assert!(
        unevaluated
            .iter()
            .any(|d| d.contains("EXPLICIT_ISR_CLEAR_MASK")),
        "EXPLICIT_ISR_CLEAR_MASK must appear as unevaluated ref; found: {:?}",
        unevaluated
    );
    assert!(
        unevaluated
            .iter()
            .any(|d| d.contains("ERROR_INTERRUPT_MASK") && !d.contains("EXPLICIT_ISR_CLEAR_MASK")),
        "ERROR_INTERRUPT_MASK must appear as a separate unevaluated ref; found: {:?}",
        unevaluated
    );
}

/// Check 2b: Inspect the constant-definition surface directly for the
/// `EXPLICIT_ISR_CLEAR_MASK = Self::ERROR_INTERRUPT_MASK` relationship.
///
/// The identity enrichment surface preserves the syntactic definitional
/// relationship via `AssociatedConstantFact.initializer_expr`. For
/// `EXPLICIT_ISR_CLEAR_MASK`, the initializer expression is rendered from
/// HIR and contains a reference to `ERROR_INTERRUPT_MASK`, proving the
/// authored dependency.
#[test]
fn check2b_definitional_relationship_on_definition_surface() {
    let result = compile_and_extract(SPECIMEN, "s2e06_check2b");

    let inherent = result
        .identity
        .impl_blocks
        .iter()
        .find(|ib| ib.is_inherent && ib.self_type_name == "Pl011FullSpec")
        .expect("inherent impl block for Pl011FullSpec must exist");

    let explicit_isr = inherent
        .constants
        .iter()
        .find(|c| c.name == "EXPLICIT_ISR_CLEAR_MASK")
        .expect("EXPLICIT_ISR_CLEAR_MASK must exist");

    let error_mask = inherent
        .constants
        .iter()
        .find(|c| c.name == "ERROR_INTERRUPT_MASK")
        .expect("ERROR_INTERRUPT_MASK must exist");

    // Both exist with correct types and same owner
    assert_eq!(explicit_isr.ty, "u32");
    assert_eq!(error_mask.ty, "u32");
    assert_ne!(
        explicit_isr.canonical_path, error_mask.canonical_path,
        "the two constants must have distinct canonical paths"
    );
    assert_eq!(
        explicit_isr.owner_impl_path, error_mask.owner_impl_path,
        "both constants must share the same owner impl path"
    );

    // The initializer_expr for EXPLICIT_ISR_CLEAR_MASK must reference
    // ERROR_INTERRUPT_MASK, proving the authored definitional relationship.
    let init_expr = explicit_isr
        .initializer_expr
        .as_ref()
        .expect("EXPLICIT_ISR_CLEAR_MASK must have an initializer_expr");

    assert!(
        init_expr.contains("ERROR_INTERRUPT_MASK"),
        "EXPLICIT_ISR_CLEAR_MASK initializer_expr must reference ERROR_INTERRUPT_MASK; \
         got: '{}'",
        init_expr
    );

    // The initializer for ERROR_INTERRUPT_MASK itself should NOT reference
    // EXPLICIT_ISR_CLEAR_MASK (it is the independent definition).
    let error_init = error_mask
        .initializer_expr
        .as_ref()
        .expect("ERROR_INTERRUPT_MASK must have an initializer_expr");

    assert!(
        !error_init.contains("EXPLICIT_ISR_CLEAR_MASK"),
        "ERROR_INTERRUPT_MASK initializer should not reference EXPLICIT_ISR_CLEAR_MASK; \
         got: '{}'",
        error_init
    );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Collect all unevaluated constant reference descriptions from a MIR body.
fn collect_unevaluated_constant_refs(body: &MirBody) -> Vec<String> {
    let mut refs = Vec::new();

    for bb in &body.basic_blocks {
        for stmt in &bb.statements {
            collect_unevaluated_from_statement(stmt, &mut refs);
        }
        if let Some(term) = &bb.terminator {
            collect_unevaluated_from_terminator(&term.kind, &mut refs);
        }
    }

    refs
}

fn collect_unevaluated_from_statement(stmt: &MirStatement, out: &mut Vec<String>) {
    if let MirStatementKind::Assign { rvalue, .. } = &stmt.kind {
        collect_unevaluated_from_rvalue(rvalue, out);
    }
}

fn collect_unevaluated_from_rvalue(rvalue: &MirRvalue, out: &mut Vec<String>) {
    match rvalue {
        MirRvalue::Use { operand } => {
            collect_unevaluated_from_operand(operand, out);
        }
        MirRvalue::BinaryOp { lhs, rhs, .. } => {
            collect_unevaluated_from_operand(lhs, out);
            collect_unevaluated_from_operand(rhs, out);
        }
        MirRvalue::UnaryOp { operand, .. } => {
            collect_unevaluated_from_operand(operand, out);
        }
        MirRvalue::Aggregate { operands, .. } => {
            for op in operands {
                collect_unevaluated_from_operand(op, out);
            }
        }
        MirRvalue::Cast { operand, .. } => {
            collect_unevaluated_from_operand(operand, out);
        }
        _ => {}
    }
}

fn collect_unevaluated_from_operand(operand: &MirOperand, out: &mut Vec<String>) {
    if let MirOperand::Constant { constant } = operand {
        if let MirConstValue::Unevaluated { path } = &constant.value {
            out.push(path.clone());
        }
    }
}

fn collect_unevaluated_from_terminator(term: &MirTerminatorKind, out: &mut Vec<String>) {
    match term {
        MirTerminatorKind::Call { func, args, .. } => {
            collect_unevaluated_from_operand(func, out);
            for arg in args {
                collect_unevaluated_from_operand(arg, out);
            }
        }
        MirTerminatorKind::SwitchInt { discr, .. } => {
            collect_unevaluated_from_operand(discr, out);
        }
        MirTerminatorKind::Assert { cond, .. } => {
            collect_unevaluated_from_operand(cond, out);
        }
        _ => {}
    }
}
