//! Compiler-side boundary validation test for PL011 register-block extraction.
//!
//! Validates the 8 checks defined in:
//!   docs/validation/01-pl011/checks/01-register-block-extraction.md
//!
//! This test compiles an inline source that structurally mirrors the PL011
//! `Pl011Registers` register block and inspects what the register-access
//! extractor actually captures at the compiler-side boundary.

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
// Inline source that mirrors the PL011 register block authored structure
// ---------------------------------------------------------------------------

/// Type stubs and complete register block matching the authored PL011 fixture.
///
/// This source mirrors `tests/fixtures/pl011-full-spec/src/registers.rs`:
/// - Same field names and ordering
/// - Same struct-level `#[rseq::register_block(endian = "little", access_width = 32)]`
/// - Same wrapper types (ReadWrite, ReadOnly, WriteOnly, Reserved)
/// - Same reserved-region structure
///
/// No per-field `#[rseq::register(...)]` attributes are present, matching the
/// authored source exactly.
const PL011_REGISTER_BLOCK_SOURCE: &str = r#"
    use core::marker::PhantomData;

    pub struct ReadWrite<T, R> { _v: T, _r: PhantomData<R> }
    pub struct ReadOnly<T, R> { _v: T, _r: PhantomData<R> }
    pub struct WriteOnly<T, R> { _v: T, _r: PhantomData<R> }
    pub struct Reserved<T> { _v: T }

    #[allow(non_snake_case)] pub mod DR_REG { pub struct Register; }
    #[allow(non_snake_case)] pub mod RSR_ECR_REG { pub struct Register; }
    #[allow(non_snake_case)] pub mod FR_REG { pub struct Register; }
    #[allow(non_snake_case)] pub mod ILPR_REG { pub struct Register; }
    #[allow(non_snake_case)] pub mod IBRD_REG { pub struct Register; }
    #[allow(non_snake_case)] pub mod FBRD_REG { pub struct Register; }
    #[allow(non_snake_case)] pub mod LCRH_REG { pub struct Register; }
    #[allow(non_snake_case)] pub mod CR_REG { pub struct Register; }
    #[allow(non_snake_case)] pub mod IFLS_REG { pub struct Register; }
    #[allow(non_snake_case)] pub mod IMSC_REG { pub struct Register; }
    #[allow(non_snake_case)] pub mod RIS_REG { pub struct Register; }
    #[allow(non_snake_case)] pub mod MIS_REG { pub struct Register; }
    #[allow(non_snake_case)] pub mod ICR_REG { pub struct Register; }
    #[allow(non_snake_case)] pub mod DMACR_REG { pub struct Register; }
    #[allow(non_snake_case)] pub mod PERIPHID0_REG { pub struct Register; }
    #[allow(non_snake_case)] pub mod PERIPHID1_REG { pub struct Register; }
    #[allow(non_snake_case)] pub mod PERIPHID2_REG { pub struct Register; }
    #[allow(non_snake_case)] pub mod PERIPHID3_REG { pub struct Register; }
    #[allow(non_snake_case)] pub mod PCELLID0_REG { pub struct Register; }
    #[allow(non_snake_case)] pub mod PCELLID1_REG { pub struct Register; }
    #[allow(non_snake_case)] pub mod PCELLID2_REG { pub struct Register; }
    #[allow(non_snake_case)] pub mod PCELLID3_REG { pub struct Register; }

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
"#;

/// Authored field names in exact declared order (22 registers + 3 reserved).
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

fn extract_pl011_block() -> RegisterAccessSurface {
    compile_and_extract(PL011_REGISTER_BLOCK_SOURCE, "pl011_register_block_boundary")
        .expect("extraction must succeed for structurally valid register block")
}

// ---------------------------------------------------------------------------
// Check 1: Register block identity preserved as "Pl011Registers"
// ---------------------------------------------------------------------------

#[test]
fn check_1_register_block_identity_preserved() {
    let surface = extract_pl011_block();

    assert_eq!(
        surface.register_blocks.len(),
        1,
        "exactly one register block must be extracted"
    );
    assert_eq!(
        surface.register_blocks[0].name, "Pl011Registers",
        "register block must retain authored identity"
    );
}

// ---------------------------------------------------------------------------
// Check 2: Canonical path evidence
//
// The RegisterAccessSurface stores only the short struct name. Canonical path
// preservation is provided by the IdentityEnrichmentSurface (see
// crates/regseq-driver/src/extract/identity_enrichment.rs:70-80 where
// StructFact.canonical_path is derived from tcx.def_path_str(DefId) for
// structs marked is_register_block). This test verifies the register-access
// surface contribution: the short name matches the authored identity.
// ---------------------------------------------------------------------------

#[test]
fn check_2_short_name_matches_authored_identity() {
    let surface = extract_pl011_block();

    let block = &surface.register_blocks[0];
    assert_eq!(
        block.name, "Pl011Registers",
        "short name must match authored struct ident for canonical path reconstruction"
    );
}

// ---------------------------------------------------------------------------
// Check 3: Block-level properties (endian, access_width) NOT captured
//
// The authored source has:
//   #[rseq::register_block(endian = "little", access_width = 32)]
// But extract_register_block only checks attribute existence via
// has_rseq_attr, not its parameters. RegisterBlockDecl has no block-level
// property fields. This test proves the gap.
// ---------------------------------------------------------------------------

#[test]
fn check_3_block_properties_not_extracted() {
    let surface = extract_pl011_block();

    let block = &surface.register_blocks[0];

    // The authored register block has no per-field #[rseq::register(...)]
    // attributes, so all per-field metadata is None.
    for field in &block.fields {
        assert_eq!(
            field.access_width, None,
            "field '{}': access_width must be None (block-level attribute not parsed into fields)",
            field.name
        );
        assert_eq!(
            field.endian, None,
            "field '{}': endian must be None (block-level attribute not parsed into fields)",
            field.name
        );
    }
}

// ---------------------------------------------------------------------------
// Check 4: Reserved regions present but offsets not captured
//
// Reserved fields appear in the field list but without byte offset or
// array-count data. The extraction boundary does not preserve enough
// information to reconstruct the authored gap ranges:
//   _reserved0: 0x008 -> 0x014
//   _reserved1: 0x01C -> 0x01C
//   _reserved2: 0x04C -> 0xFDC
// ---------------------------------------------------------------------------

#[test]
fn check_4_reserved_fields_present_but_offsets_absent() {
    let surface = extract_pl011_block();

    let block = &surface.register_blocks[0];
    let field_names: Vec<&str> = block.fields.iter().map(|f| f.name.as_str()).collect();

    // Reserved fields are present in the extracted field list.
    assert!(
        field_names.contains(&"_reserved0"),
        "_reserved0 must be present in extracted fields"
    );
    assert!(
        field_names.contains(&"_reserved1"),
        "_reserved1 must be present in extracted fields"
    );
    assert!(
        field_names.contains(&"_reserved2"),
        "_reserved2 must be present in extracted fields"
    );

    // RegisterFieldDecl carries no byte-offset or array-size fields.
    // Confirm that the only per-field metadata available (access_width,
    // endian, side_effects, hw_updates) is all None for reserved fields.
    let reserved_fields: Vec<&RegisterFieldDecl> = block
        .fields
        .iter()
        .filter(|f| f.name.starts_with("_reserved"))
        .collect();

    for rf in &reserved_fields {
        assert_eq!(rf.access_width, None);
        assert_eq!(rf.endian, None);
        assert_eq!(rf.side_effects, None);
        assert_eq!(rf.hw_updates, None);
    }
}

// ---------------------------------------------------------------------------
// Check 5: Complete register inventory in authored order
//
// The check spec says: "a complete extracted field sequence in authored order
// is sufficient for the inventory/order checks even if per-field offset and
// access metadata is not preserved."
// ---------------------------------------------------------------------------

#[test]
fn check_5_complete_inventory_in_authored_order() {
    let surface = extract_pl011_block();

    let block = &surface.register_blocks[0];
    let extracted_names: Vec<&str> = block.fields.iter().map(|f| f.name.as_str()).collect();

    assert_eq!(
        extracted_names.len(),
        AUTHORED_FIELD_NAMES.len(),
        "extracted field count must match authored count ({})",
        AUTHORED_FIELD_NAMES.len()
    );

    assert_eq!(
        extracted_names, AUTHORED_FIELD_NAMES,
        "extracted field sequence must match authored declaration order exactly"
    );
}

// ---------------------------------------------------------------------------
// Check 6: Per-register four-fact tuple incomplete
//
// The check requires: field name, register name, byte offset, access kind.
// Only field name is captured. Register name (e.g. "DR_REG"), byte offset,
// and access kind (RW/R/W) are not present in RegisterFieldDecl.
// ---------------------------------------------------------------------------

#[test]
fn check_6_only_field_name_preserved_per_register() {
    let surface = extract_pl011_block();

    let block = &surface.register_blocks[0];

    // Field name is captured for every field.
    let dr = block.fields.iter().find(|f| f.name == "dr").unwrap();
    assert_eq!(dr.name, "dr");

    // RegisterFieldDecl has no register-type-name field.
    // RegisterFieldDecl has no byte-offset field.
    // RegisterFieldDecl has no access-kind field.
    // The only per-field metadata fields are: access_width, endian,
    // side_effects, hw_updates -- all None for PL011 fields.
    assert_eq!(dr.access_width, None, "no per-field access_width");
    assert_eq!(dr.endian, None, "no per-field endian");
    assert_eq!(dr.side_effects, None, "no per-field side_effects");
    assert_eq!(dr.hw_updates, None, "no per-field hw_updates");

    // Same absence pattern for a read-only register.
    let fr = block.fields.iter().find(|f| f.name == "fr").unwrap();
    assert_eq!(fr.access_width, None);

    // Same absence pattern for a write-only register.
    let icr = block.fields.iter().find(|f| f.name == "icr").unwrap();
    assert_eq!(icr.access_width, None);
}

// ---------------------------------------------------------------------------
// Check 7: No registers inserted, removed, renamed, or reordered
// ---------------------------------------------------------------------------

#[test]
fn check_7_no_insertions_or_removals() {
    let surface = extract_pl011_block();

    let block = &surface.register_blocks[0];
    let extracted_names: Vec<&str> = block.fields.iter().map(|f| f.name.as_str()).collect();

    // Exact count match: no insertions.
    assert_eq!(extracted_names.len(), 25);

    // Every authored name is present.
    for &name in AUTHORED_FIELD_NAMES {
        assert!(
            extracted_names.contains(&name),
            "authored field '{}' must be present in extraction",
            name
        );
    }

    // No extra entries.
    for &name in &extracted_names {
        assert!(
            AUTHORED_FIELD_NAMES.contains(&name),
            "extracted field '{}' must not be an invented insertion",
            name
        );
    }

    // Order preserved.
    assert_eq!(extracted_names, AUTHORED_FIELD_NAMES);
}

// ---------------------------------------------------------------------------
// Check 8: No block-level properties invented or dropped
//
// The authored register-block properties are endian and access_width.
// RegisterBlockDecl has no fields to carry them. This test proves that no
// properties are invented AND that the authored properties are not available.
// ---------------------------------------------------------------------------

#[test]
fn check_8_no_invented_properties_and_authored_properties_dropped() {
    let surface = extract_pl011_block();

    let block = &surface.register_blocks[0];

    // RegisterBlockDecl has exactly two fields: name and fields.
    // No block-level property fields exist in the struct definition
    // (see crates/regseq-driver/src/extract/register_access.rs:22-26).
    // This means no properties are invented.
    // But it also means the authored endian and access_width are dropped.

    // Confirm no per-field metadata was invented for any field.
    for field in &block.fields {
        assert_eq!(
            field.access_width, None,
            "field '{}': no access_width invented",
            field.name
        );
        assert_eq!(
            field.endian, None,
            "field '{}': no endian invented",
            field.name
        );
        assert_eq!(
            field.side_effects, None,
            "field '{}': no side_effects invented",
            field.name
        );
        assert_eq!(
            field.hw_updates, None,
            "field '{}': no hw_updates invented",
            field.name
        );
    }
}
