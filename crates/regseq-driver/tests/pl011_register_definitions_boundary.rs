//! source-to-extraction-02: PL011 register definitions authored->extraction
//! boundary validation.
//!
//! This test validates that every authored per-register definition fact for the
//! PL011 register set is preserved at the compiler-side extraction boundary
//! (RegisterAccessSurface.register_descriptors).

#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::sync::{Arc, Mutex, Once};

#[path = "../src/extract/register_access.rs"]
mod register_access;

use register_access::*;

type ExtractionResult = Result<RegisterAccessSurface, RegisterAccessError>;

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
            *self.result.lock().unwrap() = Some(extract_register_access_surface(tcx));
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
// Inline type stubs
// ---------------------------------------------------------------------------

const FIXTURE_PREAMBLE: &str = r#"
    use core::marker::PhantomData;

    pub struct Field<T, R> { pub mask: T, pub shift: T, _r: PhantomData<R> }
    impl<T, R> Field<T, R> {
        pub const fn new(mask: T, shift: T) -> Self {
            Self { mask, shift, _r: PhantomData }
        }
    }

    pub struct FieldValue<T, R> { pub mask: T, pub value: T, _r: PhantomData<R> }
    impl<T, R> FieldValue<T, R> {
        pub const fn new(mask: T, value: T) -> Self {
            Self { mask, value, _r: PhantomData }
        }
    }
"#;

// ---------------------------------------------------------------------------
// Authored facts from the check specification
// ---------------------------------------------------------------------------

/// Per-register authored facts: (register_name, [(field_name, offset, width, [(nv_name, nv_value)])])
fn authored_register_facts() -> Vec<(&'static str, Vec<AuthoredField>)> {
    vec![
        (
            "DR_REG",
            vec![
                af("OE", 11, 1, &[]),
                af("BE", 10, 1, &[]),
                af("PE", 9, 1, &[]),
                af("FE", 8, 1, &[]),
                af("DATA", 0, 8, &[]),
            ],
        ),
        (
            "RSR_ECR_REG",
            vec![
                af("OE", 3, 1, &[]),
                af("BE", 2, 1, &[]),
                af("PE", 1, 1, &[]),
                af("FE", 0, 1, &[]),
            ],
        ),
        (
            "FR_REG",
            vec![
                af("RI", 8, 1, &[]),
                af("TXFE", 7, 1, &[]),
                af("RXFF", 6, 1, &[]),
                af("TXFF", 5, 1, &[]),
                af("RXFE", 4, 1, &[]),
                af("BUSY", 3, 1, &[]),
                af("DCD", 2, 1, &[]),
                af("DSR", 1, 1, &[]),
                af("CTS", 0, 1, &[]),
            ],
        ),
        ("ILPR_REG", vec![af("ILPDVSR", 0, 8, &[])]),
        ("IBRD_REG", vec![af("BAUD_DIVINT", 0, 16, &[])]),
        ("FBRD_REG", vec![af("BAUD_DIVFRAC", 0, 6, &[])]),
        (
            "LCRH_REG",
            vec![
                af("SPS", 7, 1, &[("DISABLED", 0), ("ENABLED", 1)]),
                af(
                    "WLEN",
                    5,
                    2,
                    &[
                        ("FIVE_BITS", 0b00),
                        ("SIX_BITS", 0b01),
                        ("SEVEN_BITS", 0b10),
                        ("EIGHT_BITS", 0b11),
                    ],
                ),
                af("FEN", 4, 1, &[("DISABLED", 0), ("ENABLED", 1)]),
                af("STP2", 3, 1, &[("ONE_STOP_BIT", 0), ("TWO_STOP_BITS", 1)]),
                af("EPS", 2, 1, &[("ODD", 0), ("EVEN", 1)]),
                af("PEN", 1, 1, &[("DISABLED", 0), ("ENABLED", 1)]),
                af("BRK", 0, 1, &[("DISABLED", 0), ("ENABLED", 1)]),
            ],
        ),
        (
            "CR_REG",
            vec![
                af("CTSEN", 15, 1, &[("DISABLED", 0), ("ENABLED", 1)]),
                af("RTSEN", 14, 1, &[("DISABLED", 0), ("ENABLED", 1)]),
                af("OUT2", 13, 1, &[("INACTIVE", 0), ("ACTIVE", 1)]),
                af("OUT1", 12, 1, &[("INACTIVE", 0), ("ACTIVE", 1)]),
                af("RTS", 11, 1, &[("INACTIVE", 0), ("ACTIVE", 1)]),
                af("DTR", 10, 1, &[("INACTIVE", 0), ("ACTIVE", 1)]),
                af("RXE", 9, 1, &[("DISABLED", 0), ("ENABLED", 1)]),
                af("TXE", 8, 1, &[("DISABLED", 0), ("ENABLED", 1)]),
                af("LBE", 7, 1, &[("DISABLED", 0), ("ENABLED", 1)]),
                af("SIRLP", 2, 1, &[("NORMAL", 0), ("LOW_POWER", 1)]),
                af("SIREN", 1, 1, &[("DISABLED", 0), ("ENABLED", 1)]),
                af("UARTEN", 0, 1, &[("DISABLED", 0), ("ENABLED", 1)]),
            ],
        ),
        (
            "IFLS_REG",
            vec![
                af(
                    "RXIFLSEL",
                    3,
                    3,
                    &[
                        ("ONE_EIGHTH", 0b000),
                        ("ONE_QUARTER", 0b001),
                        ("ONE_HALF", 0b010),
                        ("THREE_QUARTERS", 0b011),
                        ("SEVEN_EIGHTHS", 0b100),
                    ],
                ),
                af(
                    "TXIFLSEL",
                    0,
                    3,
                    &[
                        ("ONE_EIGHTH", 0b000),
                        ("ONE_QUARTER", 0b001),
                        ("ONE_HALF", 0b010),
                        ("THREE_QUARTERS", 0b011),
                        ("SEVEN_EIGHTHS", 0b100),
                    ],
                ),
            ],
        ),
        (
            "IMSC_REG",
            vec![
                af("OEIM", 10, 1, &[]),
                af("BEIM", 9, 1, &[]),
                af("PEIM", 8, 1, &[]),
                af("FEIM", 7, 1, &[]),
                af("RTIM", 6, 1, &[]),
                af("TXIM", 5, 1, &[]),
                af("RXIM", 4, 1, &[]),
                af("DSRMIM", 3, 1, &[]),
                af("DCDMIM", 2, 1, &[]),
                af("CTSMIM", 1, 1, &[]),
                af("RIMIM", 0, 1, &[]),
            ],
        ),
        (
            "RIS_REG",
            vec![
                af("OERIS", 10, 1, &[]),
                af("BERIS", 9, 1, &[]),
                af("PERIS", 8, 1, &[]),
                af("FERIS", 7, 1, &[]),
                af("RTRIS", 6, 1, &[]),
                af("TXRIS", 5, 1, &[]),
                af("RXRIS", 4, 1, &[]),
                af("DSRRMIS", 3, 1, &[]),
                af("DCDRMIS", 2, 1, &[]),
                af("CTSRMIS", 1, 1, &[]),
                af("RIRMIS", 0, 1, &[]),
            ],
        ),
        (
            "MIS_REG",
            vec![
                af("OEMIS", 10, 1, &[]),
                af("BEMIS", 9, 1, &[]),
                af("PEMIS", 8, 1, &[]),
                af("FEMIS", 7, 1, &[]),
                af("RTMIS", 6, 1, &[]),
                af("TXMIS", 5, 1, &[]),
                af("RXMIS", 4, 1, &[]),
                af("DSRMMIS", 3, 1, &[]),
                af("DCDMMIS", 2, 1, &[]),
                af("CTSMMIS", 1, 1, &[]),
                af("RIMMIS", 0, 1, &[]),
            ],
        ),
        (
            "ICR_REG",
            vec![
                af("OEIC", 10, 1, &[]),
                af("BEIC", 9, 1, &[]),
                af("PEIC", 8, 1, &[]),
                af("FEIC", 7, 1, &[]),
                af("RTIC", 6, 1, &[]),
                af("TXIC", 5, 1, &[]),
                af("RXIC", 4, 1, &[]),
                af("DSRMIC", 3, 1, &[]),
                af("DCDMIC", 2, 1, &[]),
                af("CTSMIC", 1, 1, &[]),
                af("RIMIC", 0, 1, &[]),
            ],
        ),
        (
            "DMACR_REG",
            vec![
                af("DMAONERR", 2, 1, &[("DISABLED", 0), ("ENABLED", 1)]),
                af("TXDMAE", 1, 1, &[("DISABLED", 0), ("ENABLED", 1)]),
                af("RXDMAE", 0, 1, &[("DISABLED", 0), ("ENABLED", 1)]),
            ],
        ),
        (
            "PERIPHID0_REG",
            vec![af("PARTNUMBER0", 0, 8, &[("PL011", 0x11)])],
        ),
        (
            "PERIPHID1_REG",
            vec![
                af("DESIGNER0", 4, 4, &[("ARM_LOW", 0x1)]),
                af("PARTNUMBER1", 0, 4, &[("PL011", 0x0)]),
            ],
        ),
        (
            "PERIPHID2_REG",
            vec![
                af(
                    "REVISION",
                    4,
                    4,
                    &[
                        ("R1P0", 0x0),
                        ("R1P1", 0x1),
                        ("R1P3", 0x2),
                        ("R1P4", 0x2),
                        ("R1P5", 0x3),
                    ],
                ),
                af("DESIGNER1", 0, 4, &[("ARM_HIGH", 0x4)]),
            ],
        ),
        (
            "PERIPHID3_REG",
            vec![af("CONFIGURATION", 0, 8, &[("DEFAULT", 0x00)])],
        ),
        (
            "PCELLID0_REG",
            vec![af("VALUE", 0, 8, &[("DEFAULT", 0x0D)])],
        ),
        (
            "PCELLID1_REG",
            vec![af("VALUE", 0, 8, &[("DEFAULT", 0xF0)])],
        ),
        (
            "PCELLID2_REG",
            vec![af("VALUE", 0, 8, &[("DEFAULT", 0x05)])],
        ),
        (
            "PCELLID3_REG",
            vec![af("VALUE", 0, 8, &[("DEFAULT", 0xB1)])],
        ),
    ]
}

struct AuthoredField {
    name: &'static str,
    offset: u32,
    width: u32,
    named_values: Vec<(&'static str, u32)>,
}

fn af(name: &'static str, offset: u32, width: u32, nvs: &[(&'static str, u32)]) -> AuthoredField {
    AuthoredField {
        name,
        offset,
        width,
        named_values: nvs.to_vec(),
    }
}

// ---------------------------------------------------------------------------
// Specimen generation: build inline source that mirrors register_bitfields!
// macro expansion for every in-scope register
// ---------------------------------------------------------------------------

/// Generate a single field sub-module in the register_bitfields! expansion pattern.
fn gen_field_module(_reg_name: &str, field: &AuthoredField) -> String {
    let mask = ((1u64 << field.width) - 1) << field.offset;
    let shift = field.offset;

    let mut s = format!(
        r#"
                #[allow(non_snake_case)]
                pub mod {field_name} {{
                    pub const FIELD: super::super::Field<u32, super::Register> =
                        super::super::Field::new({mask}, {shift});
"#,
        field_name = field.name,
        mask = mask,
        shift = shift
    );

    for (nv_name, nv_value) in &field.named_values {
        let nv_shifted = (*nv_value as u64) << field.offset;
        s.push_str(&format!(
            r#"                    pub const {nv_name}: super::super::FieldValue<u32, super::Register> =
                        super::super::FieldValue::new({mask}, {nv_shifted});
"#,
            nv_name = nv_name,
            mask = mask,
            nv_shifted = nv_shifted
        ));
    }

    s.push_str("                }\n");
    s
}

/// Generate a full register descriptor module.
fn gen_register_module(reg_name: &str, fields: &[AuthoredField]) -> String {
    let mut s = format!(
        r#"
            #[allow(non_snake_case)]
            pub mod {reg_name} {{
                pub struct Register;
"#,
        reg_name = reg_name
    );

    for field in fields {
        s.push_str(&gen_field_module(reg_name, field));
    }

    s.push_str("            }\n");
    s
}

/// Build the complete inline specimen source.
fn build_specimen_source() -> String {
    let facts = authored_register_facts();
    let mut source = String::new();
    source.push_str(FIXTURE_PREAMBLE);

    for (reg_name, fields) in &facts {
        source.push_str(&gen_register_module(reg_name, fields));
    }

    source
}

// ---------------------------------------------------------------------------
// Precondition: specimen fidelity against fixture source
// ---------------------------------------------------------------------------

/// Verify that every authored fact in our inline specimen matches the actual
/// fixture source registers.rs. This reads the fixture file and checks that
/// every register, field, offset, width, and named value appears in the
/// authored source text.
#[test]
fn precondition_specimen_fidelity_against_fixture() {
    let fixture_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/pl011-full-spec/src/registers.rs"
    );
    let fixture_source =
        std::fs::read_to_string(fixture_path).expect("fixture registers.rs must be readable");

    let facts = authored_register_facts();

    // Verify all 22 in-scope registers appear in the fixture
    let expected_regs: Vec<&str> = facts.iter().map(|(name, _)| *name).collect();
    assert_eq!(
        expected_regs.len(),
        22,
        "must have exactly 22 in-scope registers"
    );

    for (reg_name, fields) in &facts {
        // Register module declaration must exist
        assert!(
            fixture_source.contains(&format!("{} [", reg_name))
                || fixture_source.contains(&format!("{reg_name} [")),
            "fixture must contain register declaration for {reg_name}"
        );

        for field in fields {
            // Field declaration must exist with correct OFFSET and NUMBITS
            let offset_pattern = format!(
                "{} OFFSET({}) NUMBITS({})",
                field.name, field.offset, field.width
            );
            assert!(
                fixture_source.contains(&offset_pattern),
                "fixture must contain field '{field_name}' in {reg_name} with \
                 OFFSET({offset}) NUMBITS({width}), looked for: {pattern}",
                field_name = field.name,
                reg_name = reg_name,
                offset = field.offset,
                width = field.width,
                pattern = offset_pattern
            );

            // Named values must match
            for (nv_name, nv_value) in &field.named_values {
                // Format the value as it appears in source: hex, binary, or decimal
                let value_appears = fixture_source_contains_named_value(
                    &fixture_source,
                    reg_name,
                    field.name,
                    nv_name,
                    *nv_value,
                );
                assert!(
                    value_appears,
                    "fixture must contain named value {nv_name} = {nv_value} \
                     for field {field_name} in {reg_name}",
                    nv_name = nv_name,
                    nv_value = nv_value,
                    field_name = field.name,
                    reg_name = reg_name
                );
            }
        }
    }
}

/// Check that a named value appears in the fixture source.
/// The value might be expressed in decimal, hex, or binary notation.
fn fixture_source_contains_named_value(
    source: &str,
    _reg_name: &str,
    _field_name: &str,
    nv_name: &str,
    nv_value: u32,
) -> bool {
    // Try all common representations
    let patterns = [
        format!("{} = {}", nv_name, nv_value),
        format!("{} = 0x{:02X}", nv_name, nv_value),
        format!("{} = 0x{:x}", nv_name, nv_value),
        format!("{} = 0b{:b}", nv_name, nv_value),
        format!("{} = 0b{:03b}", nv_name, nv_value),
        format!("{} = 0b{:02b}", nv_name, nv_value),
    ];
    patterns.iter().any(|p| source.contains(p))
}

// ---------------------------------------------------------------------------
// Main extraction test: all 5 check groups
// ---------------------------------------------------------------------------

#[test]
fn check_group_1_register_identity_preservation() {
    let source = build_specimen_source();
    let surface = compile_and_extract(&source, "reg_defs_cg1").unwrap();

    let facts = authored_register_facts();

    // Every in-scope register must be identifiable by its authored name
    for (reg_name, _) in &facts {
        let found = surface
            .register_descriptors
            .iter()
            .any(|d| d.register_module == *reg_name);
        assert!(
            found,
            "Check Group 1: register '{reg_name}' must be identifiable in \
             RegisterAccessSurface.register_descriptors"
        );
    }

    // Exact count: no extra registers beyond the 22 in scope
    // (the specimen only contains in-scope registers)
    assert_eq!(
        surface.register_descriptors.len(),
        facts.len(),
        "Check Group 1: descriptor count must equal authored register count ({})",
        facts.len()
    );
}

#[test]
fn check_group_2_field_inventory_completeness_and_order() {
    let source = build_specimen_source();
    let surface = compile_and_extract(&source, "reg_defs_cg2").unwrap();

    let facts = authored_register_facts();

    for (reg_name, authored_fields) in &facts {
        let descriptor = surface
            .register_descriptors
            .iter()
            .find(|d| d.register_module == *reg_name)
            .unwrap_or_else(|| panic!("descriptor for {reg_name} must exist"));

        // Field count must match exactly (no insertions or removals)
        assert_eq!(
            descriptor.fields.len(),
            authored_fields.len(),
            "Check Group 2: field count for {reg_name} must match \
             (expected {expected}, got {actual})",
            reg_name = reg_name,
            expected = authored_fields.len(),
            actual = descriptor.fields.len()
        );

        // Field names must appear in authored order
        let extracted_names: Vec<&str> = descriptor
            .fields
            .iter()
            .map(|f| f.field_name.as_str())
            .collect();
        let authored_names: Vec<&str> = authored_fields.iter().map(|f| f.name).collect();
        assert_eq!(
            extracted_names, authored_names,
            "Check Group 2: field order for {reg_name} must match authored order"
        );
    }
}

#[test]
fn check_group_3_field_name_offset_width_preservation() {
    let source = build_specimen_source();
    let surface = compile_and_extract(&source, "reg_defs_cg3").unwrap();

    let facts = authored_register_facts();

    for (reg_name, authored_fields) in &facts {
        let descriptor = surface
            .register_descriptors
            .iter()
            .find(|d| d.register_module == *reg_name)
            .unwrap_or_else(|| panic!("descriptor for {reg_name} must exist"));

        for authored in authored_fields {
            let extracted = descriptor
                .fields
                .iter()
                .find(|f| f.field_name == authored.name)
                .unwrap_or_else(|| {
                    panic!(
                        "Check Group 3: field '{}' must exist in {reg_name}",
                        authored.name
                    )
                });

            assert_eq!(
                extracted.offset,
                authored.offset,
                "Check Group 3: {reg_name}.{field}.offset must be {expected}, got {actual}",
                field = authored.name,
                expected = authored.offset,
                actual = extracted.offset
            );

            assert_eq!(
                extracted.width,
                authored.width,
                "Check Group 3: {reg_name}.{field}.width must be {expected}, got {actual}",
                field = authored.name,
                expected = authored.width,
                actual = extracted.width
            );
        }
    }
}

#[test]
fn check_group_4_named_value_preservation() {
    let source = build_specimen_source();
    let surface = compile_and_extract(&source, "reg_defs_cg4").unwrap();

    let facts = authored_register_facts();

    for (reg_name, authored_fields) in &facts {
        let descriptor = surface
            .register_descriptors
            .iter()
            .find(|d| d.register_module == *reg_name)
            .unwrap_or_else(|| panic!("descriptor for {reg_name} must exist"));

        for authored in authored_fields {
            if authored.named_values.is_empty() {
                continue;
            }

            let extracted = descriptor
                .fields
                .iter()
                .find(|f| f.field_name == authored.name)
                .unwrap_or_else(|| {
                    panic!(
                        "Check Group 4: field '{}' must exist in {reg_name}",
                        authored.name
                    )
                });

            // Exact named-value count (closed-world: no insertions or removals)
            assert_eq!(
                extracted.named_values.len(),
                authored.named_values.len(),
                "Check Group 4: named value count for {reg_name}.{field} must match \
                 (expected {expected}, got {actual})",
                field = authored.name,
                expected = authored.named_values.len(),
                actual = extracted.named_values.len()
            );

            for (nv_name, nv_value) in &authored.named_values {
                let found = extracted.named_values.iter().find(|nv| nv.name == *nv_name);
                let found = found.unwrap_or_else(|| {
                    panic!(
                        "Check Group 4: named value '{nv_name}' must exist in \
                         {reg_name}.{field}",
                        field = authored.name
                    )
                });
                assert_eq!(
                    found.value,
                    *nv_value,
                    "Check Group 4: {reg_name}.{field}.{nv_name} must have value \
                     {expected}, got {actual}",
                    field = authored.name,
                    expected = nv_value,
                    actual = found.value
                );
            }
        }
    }
}

#[test]
fn check_group_5_no_insertions_removals_renames_reorders() {
    let source = build_specimen_source();
    let surface = compile_and_extract(&source, "reg_defs_cg5").unwrap();

    let facts = authored_register_facts();

    // Closed-world register count
    assert_eq!(
        surface.register_descriptors.len(),
        facts.len(),
        "Check Group 5: exact register count must match (no register insertions/removals)"
    );

    // Register names must be an exact set match
    let mut extracted_reg_names: Vec<&str> = surface
        .register_descriptors
        .iter()
        .map(|d| d.register_module.as_str())
        .collect();
    extracted_reg_names.sort();
    let mut authored_reg_names: Vec<&str> = facts.iter().map(|(name, _)| *name).collect();
    authored_reg_names.sort();
    assert_eq!(
        extracted_reg_names, authored_reg_names,
        "Check Group 5: register set must be exact (no insertions, removals, or renames)"
    );

    for (reg_name, authored_fields) in &facts {
        let descriptor = surface
            .register_descriptors
            .iter()
            .find(|d| d.register_module == *reg_name)
            .unwrap();

        // Closed-world field check: exact count and exact order
        let extracted_field_names: Vec<&str> = descriptor
            .fields
            .iter()
            .map(|f| f.field_name.as_str())
            .collect();
        let authored_field_names: Vec<&str> = authored_fields.iter().map(|f| f.name).collect();
        assert_eq!(
            extracted_field_names, authored_field_names,
            "Check Group 5: field inventory for {reg_name} must be exact \
             (no insertions, removals, renames, or reorders)"
        );

        // Closed-world named value check per field
        for authored in authored_fields {
            let extracted = descriptor
                .fields
                .iter()
                .find(|f| f.field_name == authored.name)
                .unwrap();

            let extracted_nv_names: Vec<&str> = extracted
                .named_values
                .iter()
                .map(|nv| nv.name.as_str())
                .collect();
            let authored_nv_names: Vec<&str> = authored
                .named_values
                .iter()
                .map(|(name, _)| *name)
                .collect();

            assert_eq!(
                extracted_nv_names.len(),
                authored_nv_names.len(),
                "Check Group 5: named value count for {reg_name}.{field} must match exactly",
                field = authored.name
            );

            // Named values: check exact set membership
            for nv_name in &authored_nv_names {
                assert!(
                    extracted_nv_names.contains(nv_name),
                    "Check Group 5: named value '{nv_name}' must exist in {reg_name}.{field}",
                    field = authored.name
                );
            }
            for nv_name in &extracted_nv_names {
                assert!(
                    authored_nv_names.contains(nv_name),
                    "Check Group 5: unexpected named value '{nv_name}' in {reg_name}.{field} \
                     (insertion detected)",
                    field = authored.name
                );
            }
        }
    }
}
