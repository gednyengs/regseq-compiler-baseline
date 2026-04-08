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

/// All 22 declared register field names in authored offset order.
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

/// Non-reserved register field names only (for register-inventory checks).
const AUTHORED_REGISTER_NAMES: &[&str] = &[
    "dr",
    "rsr_ecr",
    "fr",
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
    "periph_id0",
    "periph_id1",
    "periph_id2",
    "periph_id3",
    "pcell_id0",
    "pcell_id1",
    "pcell_id2",
    "pcell_id3",
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

    // Fidelity assertion 3: all authored field names present in source order
    for field_name in AUTHORED_FIELD_NAMES {
        let pattern = if field_name.starts_with('_') {
            // Reserved fields are private (no pub)
            format!("{field_name}:")
        } else {
            format!("pub {field_name}:")
        };
        assert!(
            fixture_src.contains(&pattern),
            "fixture must contain field `{field_name}`"
        );
    }

    // Fidelity assertion 4: field ordering matches authored order
    let mut last_pos = 0usize;
    for field_name in AUTHORED_FIELD_NAMES {
        let pattern = if field_name.starts_with('_') {
            format!("{field_name}:")
        } else {
            format!("pub {field_name}:")
        };
        let pos = fixture_src[last_pos..]
            .find(&pattern)
            .unwrap_or_else(|| panic!("field `{field_name}` not found after position {last_pos}"));
        last_pos += pos + pattern.len();
    }

    // Fidelity assertion 5: reserved region array sizes
    assert!(
        fixture_src.contains("_reserved0: [Reserved<u32>; 4]"),
        "fixture must have _reserved0 as [Reserved<u32>; 4]"
    );
    assert!(
        fixture_src.contains("_reserved1: Reserved<u32>"),
        "fixture must have _reserved1 as Reserved<u32>"
    );
    assert!(
        fixture_src.contains("_reserved2: [Reserved<u32>; 997]"),
        "fixture must have _reserved2 as [Reserved<u32>; 997]"
    );

    // Fidelity assertion 6: wrapper types match authored access kinds
    // RW registers use ReadWrite, R-only use ReadOnly, W-only use WriteOnly
    assert!(fixture_src.contains("pub dr: ReadWrite<u32, DR_REG::Register>"));
    assert!(fixture_src.contains("pub fr: ReadOnly<u32, FR_REG::Register>"));
    assert!(fixture_src.contains("pub icr: WriteOnly<u32, ICR_REG::Register>"));
    assert!(fixture_src.contains("pub ris: ReadOnly<u32, RIS_REG::Register>"));
    assert!(fixture_src.contains("pub cr: ReadWrite<u32, CR_REG::Register>"));
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

    // The extraction surface uses `tcx.def_path_str(DefId)` which for local
    // items does not include the crate-name prefix. The struct's canonical_path
    // is "registers::Pl011Registers" (source-local), not
    // "pl011_full_spec::registers::Pl011Registers" (fully-qualified).
    //
    // However, the boundary preserves enough evidence to reconstruct the full
    // canonical path exactly:
    // 1. The crate root module is named "pl011_full_spec"
    // 2. The "registers" module is a child of the crate root
    // 3. The struct canonical_path is "registers::Pl011Registers"
    // Combined: pl011_full_spec::registers::Pl011Registers

    // Assertion 1: at least one struct with source-local path
    // "registers::Pl011Registers" exists and is marked as a register block.
    // (The identity enrichment surface may produce duplicate entries when
    // the struct is visited both from the crate-root sweep and from the
    // module recursive walk; this is a known extraction surface property.)
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

    // The RegisterBlockDecl struct has no block-level `endian` or
    // `access_width` fields. The check specification requires both to survive
    // at the extraction boundary. We verify that the extraction surface does
    // NOT carry these properties.
    //
    // Per-field RegisterFieldDecl has access_width/endian but those come from
    // per-field #[rseq::register(...)] attributes, which are NOT present on the
    // PL011 fixture fields. The block-level attribute values are not propagated.
    //
    // Assertion: no field carries block-level endian or access_width because
    // the extraction surface does not propagate block-level properties to
    // fields, and no per-field #[rseq::register] attribute is present.
    for field in &block.fields {
        assert_eq!(
            field.endian, None,
            "field `{}`: no endian should be extracted (block-level attribute not propagated)",
            field.name
        );
        assert_eq!(
            field.access_width, None,
            "field `{}`: no access_width should be extracted (block-level attribute not propagated)",
            field.name
        );
    }

    // The above proves that endian=little and access_width=32 are NOT preserved
    // at the RegisterAccessSurface boundary. This check will be recorded as
    // FAIL in the validation report.
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

    // Reserved fields are extracted by name.
    let reserved_names: Vec<&str> = block
        .fields
        .iter()
        .filter(|f| f.name.starts_with("_reserved"))
        .map(|f| f.name.as_str())
        .collect();

    assert_eq!(
        reserved_names,
        vec!["_reserved0", "_reserved1", "_reserved2"],
        "all three reserved regions must be extracted by name"
    );

    // However, the extraction surface does NOT preserve byte offsets or array
    // sizes for reserved regions. RegisterFieldDecl has no offset or size
    // fields. This means the authored offsets (0x008->0x014, 0x01C->0x01C,
    // 0x04C->0xFDC) cannot be reconstructed from the extraction boundary.
    // This check will be recorded as FAIL in the validation report because
    // the boundary does not carry enough layout data to reconstruct exact gaps.
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
    //
    // Assertion: all 25 fields (22 registers + 3 reserved) are extracted
    // in authored source order.
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

    // (a) Struct field names are preserved. Verify all authored register names.
    let non_reserved: Vec<&str> = block
        .fields
        .iter()
        .filter(|f| !f.name.starts_with("_reserved"))
        .map(|f| f.name.as_str())
        .collect();
    assert_eq!(
        non_reserved, AUTHORED_REGISTER_NAMES,
        "all non-reserved field names must match authored register names"
    );

    // (b) Register name (type descriptor, e.g. DR_REG), byte offset, and
    //     access kind are NOT preserved in RegisterFieldDecl. The struct only
    //     has: name, access_width, endian, side_effects, hw_updates.
    //     None of which carry the register descriptor type name, byte offset,
    //     or read/write/read-write classification.
    //
    // This check will be recorded as FAIL for register name, byte offset,
    // and access kind. Only struct field name is preserved.
    for field in &block.fields {
        if field.name.starts_with("_reserved") {
            continue;
        }
        // Confirm the three missing per-register facts are indeed absent:
        assert_eq!(
            field.access_width, None,
            "field `{}`: access_width not preserved (no per-field #[rseq::register])",
            field.name
        );
        assert_eq!(
            field.endian, None,
            "field `{}`: endian not preserved (no per-field #[rseq::register])",
            field.name
        );
        // RegisterFieldDecl has no `register_type_name`, `byte_offset`, or
        // `access_kind` fields at all. The extraction surface structurally
        // cannot carry these facts.
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

    // Closed-world assertion 3: only one register block exists (no spurious blocks)
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

    // The authored register-block property set is: endian=little, access_width=32.
    // The extraction boundary (RegisterBlockDecl) has NO block-level property
    // fields at all -- only `name` and `fields`.
    //
    // Verify that per-field properties are also absent (since no per-field
    // #[rseq::register] attributes exist on the specimen):
    for field in &block.fields {
        assert_eq!(
            field.access_width, None,
            "no access_width should be invented for `{}`",
            field.name
        );
        assert_eq!(
            field.endian, None,
            "no endian should be invented for `{}`",
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

    // The above confirms:
    // - No properties are invented (good)
    // - But the authored endian=little and access_width=32 are dropped because
    //   RegisterBlockDecl does not carry block-level properties.
    // This check will be recorded as FAIL due to the authored properties being
    // dropped.
}
