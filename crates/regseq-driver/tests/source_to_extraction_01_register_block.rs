//! source-to-extraction-01: PL011 register-block extraction boundary validation.
//!
//! This test validates whether the authored register-block facts for
//! `Pl011Registers` survive at the compiler-side boundary between the authored
//! PL011 source and the current Regseq extraction state.
//!
//! Extraction surfaces under test:
//!   - `extract_register_access_surface(tcx) -> RegisterAccessSurface`
//!   - `extract_identity_enrichment(tcx) -> IdentityEnrichmentSurface`
//!
//! Specimen: inline reduced specimen mirroring the PL011 fixture register block.
//! Fidelity is verified against the real fixture source at
//! `tests/fixtures/pl011-full-spec/src/registers.rs`.

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

#[path = "../src/extract/identity_enrichment_types.rs"]
mod identity_enrichment_types;

#[path = "../src/extract/identity_enrichment.rs"]
mod identity_enrichment;

use identity_enrichment::*;
use register_access::*;

// ---------------------------------------------------------------------------
// Authored facts from the check specification
// ---------------------------------------------------------------------------

/// All 25 fields (22 registers + 3 reserved) in authored offset order.
const AUTHORED_FIELD_NAMES: &[&str] = &[
    "dr",
    "rsr_ecr",
    "_reserved0",
    "fr",
    "_reserved1",
    "ilpr",
    "ibrd",
    "fbrd",
    "lcr_h",
    "cr",
    "ifls",
    "imsc",
    "ris",
    "mis",
    "icr",
    "dmacr",
    "_reserved2",
    "periph_id0",
    "periph_id1",
    "periph_id2",
    "periph_id3",
    "pcell_id0",
    "pcell_id1",
    "pcell_id2",
    "pcell_id3",
];

/// Exact authored field declarations from the PL011 fixture register block.
/// These are used to prove reduced-specimen fidelity exhaustively rather than
/// by representative wrapper-type samples.
const AUTHORED_FIELD_DECLS: &[(&str, &str)] = &[
    ("dr", "pub dr: ReadWrite<u32, DR_REG::Register>,"),
    (
        "rsr_ecr",
        "pub rsr_ecr: ReadWrite<u32, RSR_ECR_REG::Register>,",
    ),
    ("_reserved0", "_reserved0: [Reserved<u32>; 4],"),
    ("fr", "pub fr: ReadOnly<u32, FR_REG::Register>,"),
    ("_reserved1", "_reserved1: Reserved<u32>,"),
    ("ilpr", "pub ilpr: ReadWrite<u32, ILPR_REG::Register>,"),
    ("ibrd", "pub ibrd: ReadWrite<u32, IBRD_REG::Register>,"),
    ("fbrd", "pub fbrd: ReadWrite<u32, FBRD_REG::Register>,"),
    ("lcr_h", "pub lcr_h: ReadWrite<u32, LCRH_REG::Register>,"),
    ("cr", "pub cr: ReadWrite<u32, CR_REG::Register>,"),
    ("ifls", "pub ifls: ReadWrite<u32, IFLS_REG::Register>,"),
    ("imsc", "pub imsc: ReadWrite<u32, IMSC_REG::Register>,"),
    ("ris", "pub ris: ReadOnly<u32, RIS_REG::Register>,"),
    ("mis", "pub mis: ReadOnly<u32, MIS_REG::Register>,"),
    ("icr", "pub icr: WriteOnly<u32, ICR_REG::Register>,"),
    ("dmacr", "pub dmacr: ReadWrite<u32, DMACR_REG::Register>,"),
    ("_reserved2", "_reserved2: [Reserved<u32>; 997],"),
    (
        "periph_id0",
        "pub periph_id0: ReadOnly<u32, PERIPHID0_REG::Register>,",
    ),
    (
        "periph_id1",
        "pub periph_id1: ReadOnly<u32, PERIPHID1_REG::Register>,",
    ),
    (
        "periph_id2",
        "pub periph_id2: ReadOnly<u32, PERIPHID2_REG::Register>,",
    ),
    (
        "periph_id3",
        "pub periph_id3: ReadOnly<u32, PERIPHID3_REG::Register>,",
    ),
    (
        "pcell_id0",
        "pub pcell_id0: ReadOnly<u32, PCELLID0_REG::Register>,",
    ),
    (
        "pcell_id1",
        "pub pcell_id1: ReadOnly<u32, PCELLID1_REG::Register>,",
    ),
    (
        "pcell_id2",
        "pub pcell_id2: ReadOnly<u32, PCELLID2_REG::Register>,",
    ),
    (
        "pcell_id3",
        "pub pcell_id3: ReadOnly<u32, PCELLID3_REG::Register>,",
    ),
];

// ---------------------------------------------------------------------------
// Extraction result types
// ---------------------------------------------------------------------------

type RegAccessResult = Result<RegisterAccessSurface, RegisterAccessError>;
type IdentityResult = Result<IdentityEnrichmentSurface, IdentityEnrichmentError>;

struct DualExtractionResult {
    register_access: RegAccessResult,
    identity: IdentityResult,
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

    let result: Arc<Mutex<Option<DualExtractionResult>>> = Arc::new(Mutex::new(None));

    struct ExtractCallbacks {
        result: Arc<Mutex<Option<DualExtractionResult>>>,
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
            let reg = extract_register_access_surface(tcx);
            let id = extract_identity_enrichment(tcx);
            *self.result.lock().unwrap() = Some(DualExtractionResult {
                register_access: reg,
                identity: id,
            });
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
        "--crate-name".to_string(),
        "pl011_full_spec".to_string(),
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
// Inline reduced specimen
//
// Mirrors the PL011 register block from
//   tests/fixtures/pl011-full-spec/src/registers.rs
// with local type stubs replacing `regseq` crate types.
//
// Structural facts preserved:
//   - struct name: Pl011Registers
//   - module name: registers (via mod wrapper)
//   - attribute: #[rseq::register_block(endian = "little", access_width = 32)]
//   - all 25 fields in authored order (22 registers + 3 reserved regions)
//   - field names match authored exactly
//   - reserved fields use array types where the fixture does
//   - wrapper types: ReadWrite, ReadOnly, WriteOnly, Reserved
//
// Facts intentionally NOT modeled (not needed for register-block checks):
//   - register_bitfields! macro expansion (descriptor fields/values)
//   - driver struct, trait impls, method bodies
// ---------------------------------------------------------------------------

const SPECIMEN_SOURCE: &str = r#"
use core::marker::PhantomData;

pub struct ReadWrite<T, R> { _v: T, _r: PhantomData<R> }
pub struct ReadOnly<T, R> { _v: T, _r: PhantomData<R> }
pub struct WriteOnly<T, R> { _v: T, _r: PhantomData<R> }
pub struct Reserved<T> { _v: T }

// Stub register descriptor modules
#[allow(non_snake_case)] mod DR_REG { pub struct Register; }
#[allow(non_snake_case)] mod RSR_ECR_REG { pub struct Register; }
#[allow(non_snake_case)] mod FR_REG { pub struct Register; }
#[allow(non_snake_case)] mod ILPR_REG { pub struct Register; }
#[allow(non_snake_case)] mod IBRD_REG { pub struct Register; }
#[allow(non_snake_case)] mod FBRD_REG { pub struct Register; }
#[allow(non_snake_case)] mod LCRH_REG { pub struct Register; }
#[allow(non_snake_case)] mod CR_REG { pub struct Register; }
#[allow(non_snake_case)] mod IFLS_REG { pub struct Register; }
#[allow(non_snake_case)] mod IMSC_REG { pub struct Register; }
#[allow(non_snake_case)] mod RIS_REG { pub struct Register; }
#[allow(non_snake_case)] mod MIS_REG { pub struct Register; }
#[allow(non_snake_case)] mod ICR_REG { pub struct Register; }
#[allow(non_snake_case)] mod DMACR_REG { pub struct Register; }
#[allow(non_snake_case)] mod PERIPHID0_REG { pub struct Register; }
#[allow(non_snake_case)] mod PERIPHID1_REG { pub struct Register; }
#[allow(non_snake_case)] mod PERIPHID2_REG { pub struct Register; }
#[allow(non_snake_case)] mod PERIPHID3_REG { pub struct Register; }
#[allow(non_snake_case)] mod PCELLID0_REG { pub struct Register; }
#[allow(non_snake_case)] mod PCELLID1_REG { pub struct Register; }
#[allow(non_snake_case)] mod PCELLID2_REG { pub struct Register; }
#[allow(non_snake_case)] mod PCELLID3_REG { pub struct Register; }

pub mod registers {
    use super::*;

    #[repr(C)]
    #[rseq::register_block(endian = "little", access_width = 32)]
    pub struct Pl011Registers {
        pub dr: ReadWrite<u32, DR_REG::Register>,
        pub rsr_ecr: ReadWrite<u32, RSR_ECR_REG::Register>,
        _reserved0: [Reserved<u32>; 4],
        pub fr: ReadOnly<u32, FR_REG::Register>,
        _reserved1: Reserved<u32>,
        pub ilpr: ReadWrite<u32, ILPR_REG::Register>,
        pub ibrd: ReadWrite<u32, IBRD_REG::Register>,
        pub fbrd: ReadWrite<u32, FBRD_REG::Register>,
        pub lcr_h: ReadWrite<u32, LCRH_REG::Register>,
        pub cr: ReadWrite<u32, CR_REG::Register>,
        pub ifls: ReadWrite<u32, IFLS_REG::Register>,
        pub imsc: ReadWrite<u32, IMSC_REG::Register>,
        pub ris: ReadOnly<u32, RIS_REG::Register>,
        pub mis: ReadOnly<u32, MIS_REG::Register>,
        pub icr: WriteOnly<u32, ICR_REG::Register>,
        pub dmacr: ReadWrite<u32, DMACR_REG::Register>,
        _reserved2: [Reserved<u32>; 997],
        pub periph_id0: ReadOnly<u32, PERIPHID0_REG::Register>,
        pub periph_id1: ReadOnly<u32, PERIPHID1_REG::Register>,
        pub periph_id2: ReadOnly<u32, PERIPHID2_REG::Register>,
        pub periph_id3: ReadOnly<u32, PERIPHID3_REG::Register>,
        pub pcell_id0: ReadOnly<u32, PCELLID0_REG::Register>,
        pub pcell_id1: ReadOnly<u32, PCELLID1_REG::Register>,
        pub pcell_id2: ReadOnly<u32, PCELLID2_REG::Register>,
        pub pcell_id3: ReadOnly<u32, PCELLID3_REG::Register>,
    }
}

// Minimal driver stub required by identity enrichment (it expects a
// #[rseq::driver]-annotated struct to exist).
#[rseq::driver(name = "pl011-full-spec", compatible = ["arm,pl011"])]
pub struct Pl011FullSpec {
    pub dummy: u32,
}
"#;

// ---------------------------------------------------------------------------
// Specimen fidelity: verify inline specimen matches real fixture source
// ---------------------------------------------------------------------------

#[test]
fn specimen_fidelity_against_real_fixture() {
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures/pl011-full-spec/src/registers.rs");
    let fixture_src =
        std::fs::read_to_string(&fixture_path).expect("failed to read real fixture registers.rs");

    // Fidelity assertion 1: struct name
    assert!(
        fixture_src.contains("pub struct Pl011Registers"),
        "fixture must declare Pl011Registers"
    );

    // Fidelity assertion 2: register_block attribute with endian and access_width
    assert!(
        fixture_src.contains(r#"#[rseq::register_block(endian = "little", access_width = 32)]"#),
        "fixture must have register_block attribute with endian=little, access_width=32"
    );

    // Fidelity assertion 3: every exact authored field declaration appears in
    // both the real fixture and the reduced specimen.
    for (field_name, field_decl) in AUTHORED_FIELD_DECLS {
        assert!(
            fixture_src.contains(field_decl),
            "fixture must contain exact field declaration for `{field_name}`: {field_decl}"
        );
        assert!(
            SPECIMEN_SOURCE.contains(field_decl),
            "specimen must contain exact field declaration for `{field_name}`: {field_decl}"
        );
    }

    // Fidelity assertion 4: fixture field ordering matches authored order.
    let mut last_fixture_pos = 0usize;
    for (field_name, field_decl) in AUTHORED_FIELD_DECLS {
        let pos = fixture_src[last_fixture_pos..].find(field_decl).unwrap_or_else(|| {
            panic!("fixture declaration `{field_name}` not found after position {last_fixture_pos}")
        });
        last_fixture_pos += pos + field_decl.len();
    }

    // Fidelity assertion 5: specimen field ordering matches authored order.
    let mut last_specimen_pos = 0usize;
    for (field_name, field_decl) in AUTHORED_FIELD_DECLS {
        let pos = SPECIMEN_SOURCE[last_specimen_pos..]
            .find(field_decl)
            .unwrap_or_else(|| {
                panic!(
                    "specimen declaration `{field_name}` not found after position {last_specimen_pos}"
                )
            });
        last_specimen_pos += pos + field_decl.len();
    }

    // Fidelity assertion 6: reserved region declarations and wrapper/access
    // kinds are covered exhaustively by the exact field-declaration checks
    // above. Keep a small sanity check on declaration cardinality so the
    // fidelity proof fails if the authored field table drifts.
    assert_eq!(
        AUTHORED_FIELD_DECLS.len(),
        AUTHORED_FIELD_NAMES.len(),
        "every authored field must have an exact declaration entry"
    );
}

// ---------------------------------------------------------------------------
// Check 1: Register block remains identifiable as Pl011Registers
// ---------------------------------------------------------------------------

#[test]
fn check_1_register_block_identity() {
    let result = compile_and_extract_dual(SPECIMEN_SOURCE, "src_to_ext_01_check_1");
    let surface = result.register_access.unwrap();

    // Assertion: RegisterAccessSurface contains exactly one register block
    // named "Pl011Registers".
    let blocks: Vec<&RegisterBlockDecl> = surface
        .register_blocks
        .iter()
        .filter(|b| b.name == "Pl011Registers")
        .collect();
    assert_eq!(
        blocks.len(),
        1,
        "exactly one register block named Pl011Registers must be extracted"
    );
}

// ---------------------------------------------------------------------------
// Check 2: Canonical path remains pl011_full_spec::registers::Pl011Registers
// ---------------------------------------------------------------------------

#[test]
fn check_2_canonical_path() {
    let result = compile_and_extract_dual(SPECIMEN_SOURCE, "src_to_ext_01_check_2");
    let identity = result.identity.unwrap();

    // Assertion 1: at least one struct with source-local path
    // "registers::Pl011Registers" exists and is marked as a register block.
    let matching: Vec<_> = identity
        .structs
        .iter()
        .filter(|s| s.canonical_path.0 == "registers::Pl011Registers")
        .collect();
    assert!(
        !matching.is_empty(),
        "at least one struct with source-local path \
         registers::Pl011Registers must exist"
    );
    assert!(
        matching.iter().all(|s| s.is_register_block),
        "all instances must be marked as register blocks"
    );

    // Assertion 2: crate root module is named "pl011_full_spec"
    let crate_root = identity
        .modules
        .iter()
        .find(|m| m.name == "pl011_full_spec")
        .expect("crate root module pl011_full_spec must exist");
    assert_eq!(
        crate_root.canonical_path.0, "pl011_full_spec",
        "crate root canonical path must be pl011_full_spec"
    );

    // Assertion 3: "registers" module is a child of crate root
    let registers_module = identity
        .modules
        .iter()
        .find(|m| m.name == "registers")
        .expect("registers module must exist");
    assert!(
        crate_root
            .child_items
            .iter()
            .any(|c| c.0 == registers_module.canonical_path.0),
        "registers module must be a child of crate root"
    );

    // Assertion 4: struct's module_path connects to the registers module
    let first_match = &matching[0];
    assert_eq!(
        first_match.module_path.0, "registers",
        "struct module_path must be 'registers'"
    );

    // These four facts together allow exact reconstruction of the full
    // canonical path: pl011_full_spec::registers::Pl011Registers.
}

// ---------------------------------------------------------------------------
// Check 3: Register-block properties (endian=little, access_width=32)
// ---------------------------------------------------------------------------

#[test]
fn check_3_register_block_properties() {
    let result = compile_and_extract_dual(SPECIMEN_SOURCE, "src_to_ext_01_check_3");
    let surface = result.register_access.unwrap();

    let block = surface
        .register_blocks
        .iter()
        .find(|b| b.name == "Pl011Registers")
        .expect("Pl011Registers block must exist");

    // The property bag must contain the authored endian property.
    let endian = block
        .properties
        .iter()
        .find(|(k, _)| k == "endian")
        .expect("block properties must contain 'endian'");
    assert_eq!(
        endian.1,
        BlockPropertyValue::Str("little".to_string()),
        "block-level endian must be preserved as 'little'"
    );

    // The property bag must contain the authored access_width property.
    let aw = block
        .properties
        .iter()
        .find(|(k, _)| k == "access_width")
        .expect("block properties must contain 'access_width'");
    assert_eq!(
        aw.1,
        BlockPropertyValue::Int(32),
        "block-level access_width must be preserved as 32"
    );
}

// ---------------------------------------------------------------------------
// Check 4: Reserved regions preserve authored offsets
// ---------------------------------------------------------------------------

#[test]
fn check_4_reserved_regions() {
    let result = compile_and_extract_dual(SPECIMEN_SOURCE, "src_to_ext_01_check_4");
    let surface = result.register_access.unwrap();

    let block = surface
        .register_blocks
        .iter()
        .find(|b| b.name == "Pl011Registers")
        .expect("Pl011Registers block must exist");

    let block_aw = block
        .properties
        .iter()
        .find_map(|(k, v)| {
            if k == "access_width" {
                if let BlockPropertyValue::Int(n) = v {
                    Some(*n)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .expect("access_width must be present in block properties");
    let element_size = block_aw / 8;

    // _reserved0: 0x008 -> 0x014 (4 elements)
    let r0 = block
        .fields
        .iter()
        .find(|f| f.name == "_reserved0")
        .expect("_reserved0 must exist");
    assert_eq!(r0.byte_offset, Some(0x008), "_reserved0 start offset");
    let r0_count = r0.array_count.expect("_reserved0 must have array_count");
    assert_eq!(r0_count, 4, "_reserved0 array count");
    let r0_end = r0.byte_offset.unwrap() + (r0_count - 1) * element_size;
    assert_eq!(r0_end, 0x014, "_reserved0 end offset");

    // _reserved1: 0x01C -> 0x01C (single element)
    let r1 = block
        .fields
        .iter()
        .find(|f| f.name == "_reserved1")
        .expect("_reserved1 must exist");
    assert_eq!(r1.byte_offset, Some(0x01C), "_reserved1 start offset");
    assert_eq!(r1.array_count, None, "_reserved1 is not an array");
    let r1_end = r1.byte_offset.unwrap() + (r1.array_count.unwrap_or(1) - 1) * element_size;
    assert_eq!(r1_end, 0x01C, "_reserved1 end offset");

    // _reserved2: 0x04C -> 0xFDC (997 elements)
    let r2 = block
        .fields
        .iter()
        .find(|f| f.name == "_reserved2")
        .expect("_reserved2 must exist");
    assert_eq!(r2.byte_offset, Some(0x04C), "_reserved2 start offset");
    let r2_count = r2.array_count.expect("_reserved2 must have array_count");
    assert_eq!(r2_count, 997, "_reserved2 array count");
    let r2_end = r2.byte_offset.unwrap() + (r2_count - 1) * element_size;
    assert_eq!(r2_end, 0xFDC, "_reserved2 end offset");
}

// ---------------------------------------------------------------------------
// Check 5: Declared register inventory complete and in authored offset order
// ---------------------------------------------------------------------------

#[test]
fn check_5_register_inventory_and_order() {
    let result = compile_and_extract_dual(SPECIMEN_SOURCE, "src_to_ext_01_check_5");
    let surface = result.register_access.unwrap();

    let block = surface
        .register_blocks
        .iter()
        .find(|b| b.name == "Pl011Registers")
        .expect("Pl011Registers block must exist");

    // The check spec says: "a complete extracted field sequence in authored
    // order is sufficient for the inventory/order checks even if per-field
    // offset and access metadata is not preserved."
    let extracted_names: Vec<&str> = block.fields.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        extracted_names, AUTHORED_FIELD_NAMES,
        "extracted field sequence must match authored order exactly"
    );
}

// ---------------------------------------------------------------------------
// Check 6: Per-register fact preservation (field name, register name,
//           byte offset, access kind)
// ---------------------------------------------------------------------------

#[test]
fn check_6_per_register_facts() {
    let result = compile_and_extract_dual(SPECIMEN_SOURCE, "src_to_ext_01_check_6");
    let surface = result.register_access.unwrap();

    let block = surface
        .register_blocks
        .iter()
        .find(|b| b.name == "Pl011Registers")
        .expect("Pl011Registers block must exist");

    // Authored facts: (field_name, register_type_name, byte_offset, access_kind)
    let expected: &[(&str, &str, u32, &str)] = &[
        ("dr", "DR_REG", 0x000, "RW"),
        ("rsr_ecr", "RSR_ECR_REG", 0x004, "RW"),
        ("fr", "FR_REG", 0x018, "R"),
        ("ilpr", "ILPR_REG", 0x020, "RW"),
        ("ibrd", "IBRD_REG", 0x024, "RW"),
        ("fbrd", "FBRD_REG", 0x028, "RW"),
        ("lcr_h", "LCRH_REG", 0x02C, "RW"),
        ("cr", "CR_REG", 0x030, "RW"),
        ("ifls", "IFLS_REG", 0x034, "RW"),
        ("imsc", "IMSC_REG", 0x038, "RW"),
        ("ris", "RIS_REG", 0x03C, "R"),
        ("mis", "MIS_REG", 0x040, "R"),
        ("icr", "ICR_REG", 0x044, "W"),
        ("dmacr", "DMACR_REG", 0x048, "RW"),
        ("periph_id0", "PERIPHID0_REG", 0xFE0, "R"),
        ("periph_id1", "PERIPHID1_REG", 0xFE4, "R"),
        ("periph_id2", "PERIPHID2_REG", 0xFE8, "R"),
        ("periph_id3", "PERIPHID3_REG", 0xFEC, "R"),
        ("pcell_id0", "PCELLID0_REG", 0xFF0, "R"),
        ("pcell_id1", "PCELLID1_REG", 0xFF4, "R"),
        ("pcell_id2", "PCELLID2_REG", 0xFF8, "R"),
        ("pcell_id3", "PCELLID3_REG", 0xFFC, "R"),
    ];

    for &(field_name, reg_type, offset, access) in expected {
        let field = block
            .fields
            .iter()
            .find(|f| f.name == field_name)
            .unwrap_or_else(|| panic!("field `{field_name}` must exist"));

        assert_eq!(
            field.register_type_name.as_deref(),
            Some(reg_type),
            "field `{field_name}`: register_type_name must be {reg_type}"
        );
        assert_eq!(
            field.byte_offset,
            Some(offset),
            "field `{field_name}`: byte_offset must be 0x{offset:03X}"
        );
        assert_eq!(
            field.access_kind.as_deref(),
            Some(access),
            "field `{field_name}`: access_kind must be {access}"
        );
    }
}

// ---------------------------------------------------------------------------
// Check 7: No registers inserted, removed, renamed, or reordered
// ---------------------------------------------------------------------------

#[test]
fn check_7_closed_world_inventory() {
    let result = compile_and_extract_dual(SPECIMEN_SOURCE, "src_to_ext_01_check_7");
    let surface = result.register_access.unwrap();

    let block = surface
        .register_blocks
        .iter()
        .find(|b| b.name == "Pl011Registers")
        .expect("Pl011Registers block must exist");

    // Closed-world assertion 1: exact field count
    assert_eq!(
        block.fields.len(),
        AUTHORED_FIELD_NAMES.len(),
        "field count must match authored count exactly (no insertions or removals)"
    );

    // Closed-world assertion 2: exact sequence equality
    let extracted: Vec<&str> = block.fields.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        extracted, AUTHORED_FIELD_NAMES,
        "field sequence must be identical to authored (no reordering or renaming)"
    );

    // Closed-world assertion 3: only one register block exists
    assert_eq!(
        surface.register_blocks.len(),
        1,
        "exactly one register block must be extracted (no spurious blocks)"
    );
}

// ---------------------------------------------------------------------------
// Check 8: No register-block properties invented or dropped
// ---------------------------------------------------------------------------

#[test]
fn check_8_property_set_integrity() {
    let result = compile_and_extract_dual(SPECIMEN_SOURCE, "src_to_ext_01_check_8");
    let surface = result.register_access.unwrap();

    let block = surface
        .register_blocks
        .iter()
        .find(|b| b.name == "Pl011Registers")
        .expect("Pl011Registers block must exist");

    // Block-level property bag must contain exactly the authored properties.
    let prop_keys: Vec<&str> = block.properties.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(
        prop_keys,
        vec!["endian", "access_width"],
        "property bag must contain exactly the authored property keys in authored order"
    );
    assert_eq!(
        block.properties[0].1,
        BlockPropertyValue::Str("little".to_string()),
        "block endian must be preserved"
    );
    assert_eq!(
        block.properties[1].1,
        BlockPropertyValue::Int(32),
        "block access_width must be preserved"
    );

    // No properties should be invented at the per-field level
    // (per-field #[rseq::register(...)] attributes are absent in PL011).
    for field in &block.fields {
        assert_eq!(
            field.access_width, None,
            "no per-field access_width should be invented for `{}`",
            field.name
        );
        assert_eq!(
            field.endian, None,
            "no per-field endian should be invented for `{}`",
            field.name
        );
        assert_eq!(
            field.side_effects, None,
            "no side_effects should be invented for `{}`",
            field.name
        );
        assert_eq!(
            field.hw_updates, None,
            "no hw_updates should be invented for `{}`",
            field.name
        );
    }
}
