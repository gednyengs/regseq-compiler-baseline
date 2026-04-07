//! Boundary validation: PL011 register-block authored->extraction check.
//!
//! This test validates that the authored register-block facts for
//! `Pl011Registers` are preserved at the compiler-side boundary between the
//! authored PL011 source and the current Regseq extraction state.
//!
//! Validation checks follow:
//! `docs/validation/01-pl011/checks/01-register-block-extraction.md`
//!
//! Extraction surfaces inspected:
//! - `RegisterAccessSurface` (primary declaration surface)
//! - `IdentityEnrichmentSurface` (canonical/module identity surface)

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

use identity_enrichment::{IdentityEnrichmentError, IdentityEnrichmentSurface};
use register_access::{RegisterAccessError, RegisterAccessSurface};

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

struct BoundaryResult {
    register_access: Result<RegisterAccessSurface, RegisterAccessError>,
    identity: Result<IdentityEnrichmentSurface, IdentityEnrichmentError>,
}

fn compile_and_extract_boundary(source: &str, test_name: &str) -> BoundaryResult {
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

    let reg_result: Arc<Mutex<Option<Result<RegisterAccessSurface, RegisterAccessError>>>> =
        Arc::new(Mutex::new(None));
    let id_result: Arc<Mutex<Option<Result<IdentityEnrichmentSurface, IdentityEnrichmentError>>>> =
        Arc::new(Mutex::new(None));

    struct ExtractCallbacks {
        reg_result: Arc<Mutex<Option<Result<RegisterAccessSurface, RegisterAccessError>>>>,
        id_result: Arc<Mutex<Option<Result<IdentityEnrichmentSurface, IdentityEnrichmentError>>>>,
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
            *self.reg_result.lock().unwrap() =
                Some(register_access::extract_register_access_surface(tcx));
            *self.id_result.lock().unwrap() =
                Some(identity_enrichment::extract_identity_enrichment(tcx));
            rustc_driver::Compilation::Stop
        }
    }

    let mut callbacks = ExtractCallbacks {
        reg_result: reg_result.clone(),
        id_result: id_result.clone(),
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

    let register_access = reg_result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis should have been called (register_access)");
    let identity = id_result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis should have been called (identity)");

    std::fs::remove_dir_all(&test_dir).ok();

    BoundaryResult {
        register_access,
        identity,
    }
}

// ---------------------------------------------------------------------------
// Inline specimen: reduced PL011 register block
//
// This specimen preserves every authored structural fact required by
// checks 1-8.  It uses local type stubs for ReadWrite/ReadOnly/WriteOnly/
// Reserved and register-descriptor module stubs.  The crate is compiled
// with --crate-name pl011_full_spec so canonical paths match the fixture.
//
// Specimen fidelity notes:
// - All 22 declared registers are present with authored names, access
//   wrapper types, and register descriptor type references.
// - All 3 reserved regions are present with authored names and array
//   sizes matching the fixture.
// - The #[rseq::register_block(endian = "little", access_width = 32)]
//   attribute exactly mirrors the fixture.
// - Field ordering matches the fixture.
// - The registers module nesting matches the fixture.
// - A minimal driver struct is included to satisfy IdentityEnrichment's
//   NoDriverType requirement.
// ---------------------------------------------------------------------------

const PL011_SPECIMEN: &str = r#"
    use core::marker::PhantomData;

    pub struct ReadWrite<T, R> { _v: T, _r: PhantomData<R> }
    pub struct ReadOnly<T, R>  { _v: T, _r: PhantomData<R> }
    pub struct WriteOnly<T, R> { _v: T, _r: PhantomData<R> }
    pub struct Reserved<T>     { _v: T }

    pub mod registers {
        use super::*;

        #[allow(non_snake_case)] pub mod DR_REG        { pub struct Register; }
        #[allow(non_snake_case)] pub mod RSR_ECR_REG   { pub struct Register; }
        #[allow(non_snake_case)] pub mod FR_REG        { pub struct Register; }
        #[allow(non_snake_case)] pub mod ILPR_REG      { pub struct Register; }
        #[allow(non_snake_case)] pub mod IBRD_REG      { pub struct Register; }
        #[allow(non_snake_case)] pub mod FBRD_REG      { pub struct Register; }
        #[allow(non_snake_case)] pub mod LCRH_REG      { pub struct Register; }
        #[allow(non_snake_case)] pub mod CR_REG        { pub struct Register; }
        #[allow(non_snake_case)] pub mod IFLS_REG      { pub struct Register; }
        #[allow(non_snake_case)] pub mod IMSC_REG      { pub struct Register; }
        #[allow(non_snake_case)] pub mod RIS_REG       { pub struct Register; }
        #[allow(non_snake_case)] pub mod MIS_REG       { pub struct Register; }
        #[allow(non_snake_case)] pub mod ICR_REG       { pub struct Register; }
        #[allow(non_snake_case)] pub mod DMACR_REG     { pub struct Register; }
        #[allow(non_snake_case)] pub mod PERIPHID0_REG { pub struct Register; }
        #[allow(non_snake_case)] pub mod PERIPHID1_REG { pub struct Register; }
        #[allow(non_snake_case)] pub mod PERIPHID2_REG { pub struct Register; }
        #[allow(non_snake_case)] pub mod PERIPHID3_REG { pub struct Register; }
        #[allow(non_snake_case)] pub mod PCELLID0_REG  { pub struct Register; }
        #[allow(non_snake_case)] pub mod PCELLID1_REG  { pub struct Register; }
        #[allow(non_snake_case)] pub mod PCELLID2_REG  { pub struct Register; }
        #[allow(non_snake_case)] pub mod PCELLID3_REG  { pub struct Register; }

        #[repr(C)]
        #[rseq::register_block(endian = "little", access_width = 32)]
        pub struct Pl011Registers {
            pub dr:         ReadWrite<u32, DR_REG::Register>,
            pub rsr_ecr:    ReadWrite<u32, RSR_ECR_REG::Register>,
            _reserved0:     [Reserved<u32>; 4],
            pub fr:         ReadOnly<u32, FR_REG::Register>,
            _reserved1:     Reserved<u32>,
            pub ilpr:       ReadWrite<u32, ILPR_REG::Register>,
            pub ibrd:       ReadWrite<u32, IBRD_REG::Register>,
            pub fbrd:       ReadWrite<u32, FBRD_REG::Register>,
            pub lcr_h:      ReadWrite<u32, LCRH_REG::Register>,
            pub cr:         ReadWrite<u32, CR_REG::Register>,
            pub ifls:       ReadWrite<u32, IFLS_REG::Register>,
            pub imsc:       ReadWrite<u32, IMSC_REG::Register>,
            pub ris:        ReadOnly<u32, RIS_REG::Register>,
            pub mis:        ReadOnly<u32, MIS_REG::Register>,
            pub icr:        WriteOnly<u32, ICR_REG::Register>,
            pub dmacr:      ReadWrite<u32, DMACR_REG::Register>,
            _reserved2:     [Reserved<u32>; 997],
            pub periph_id0: ReadOnly<u32, PERIPHID0_REG::Register>,
            pub periph_id1: ReadOnly<u32, PERIPHID1_REG::Register>,
            pub periph_id2: ReadOnly<u32, PERIPHID2_REG::Register>,
            pub periph_id3: ReadOnly<u32, PERIPHID3_REG::Register>,
            pub pcell_id0:  ReadOnly<u32, PCELLID0_REG::Register>,
            pub pcell_id1:  ReadOnly<u32, PCELLID1_REG::Register>,
            pub pcell_id2:  ReadOnly<u32, PCELLID2_REG::Register>,
            pub pcell_id3:  ReadOnly<u32, PCELLID3_REG::Register>,
        }
    }

    #[rseq::driver(name = "pl011-full-spec", compatible = ["arm,pl011"])]
    pub struct Pl011FullSpec {
        pub dummy: u32,
    }
"#;

// ---------------------------------------------------------------------------
// Authored facts from the check specification
// ---------------------------------------------------------------------------

/// The 22 declared register field names in authored offset order.
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

/// All 25 struct field names (registers + reserved) in authored order.
const AUTHORED_ALL_FIELD_NAMES: &[&str] = &[
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

// ---------------------------------------------------------------------------
// Check 1: Register block identity
// Premise: the register block remains identifiable as Pl011Registers
// ---------------------------------------------------------------------------

#[test]
fn check1_register_block_identity() {
    let result = compile_and_extract_boundary(PL011_SPECIMEN, "check1_identity");
    let reg = result
        .register_access
        .expect("register extraction should succeed");
    let id = result.identity.expect("identity enrichment should succeed");

    // RegisterAccessSurface preserves the block name.
    assert_eq!(
        reg.register_blocks.len(),
        1,
        "exactly one register block expected"
    );
    assert_eq!(reg.register_blocks[0].name, "Pl011Registers");

    // IdentityEnrichmentSurface marks the struct as a register block.
    let rb_struct = id
        .structs
        .iter()
        .find(|s| s.is_register_block)
        .expect("should find a register_block struct");
    assert_eq!(rb_struct.name, "Pl011Registers");
}

// ---------------------------------------------------------------------------
// Check 2: Canonical path preservation
// Premise: canonical path remains pl011_full_spec::registers::Pl011Registers
// ---------------------------------------------------------------------------

#[test]
fn check2_canonical_path() {
    let result = compile_and_extract_boundary(PL011_SPECIMEN, "check2_canonical");
    let id = result.identity.expect("identity enrichment should succeed");

    let rb_struct = id
        .structs
        .iter()
        .find(|s| s.is_register_block)
        .expect("should find a register_block struct");

    // def_path_str returns the source-local path for local items.
    // The struct path is "registers::Pl011Registers" (without crate prefix).
    let path = &rb_struct.canonical_path.0;
    assert_eq!(
        path, "registers::Pl011Registers",
        "canonical path should be the source-local qualified path"
    );

    // The module path references the registers module.
    assert_eq!(
        rb_struct.module_path.0, "registers",
        "module_path should be the registers module"
    );

    // The crate root module carries the crate name, which together with
    // the source-local struct path reconstructs the full canonical path
    // exactly: pl011_full_spec::registers::Pl011Registers.
    let crate_root = &id.modules[0];
    assert_eq!(
        crate_root.name, "pl011_full_spec",
        "crate root module name should be pl011_full_spec"
    );

    // Exact reconstruction: crate_name + "::" + source_local_path.
    let reconstructed = format!("{}::{}", crate_root.name, path);
    assert_eq!(
        reconstructed, "pl011_full_spec::registers::Pl011Registers",
        "full canonical path must be exactly reconstructible"
    );
}

// ---------------------------------------------------------------------------
// Check 3: Register-block properties (endian, access_width)
// Premise: endian = little, access_width = 32
// Expected outcome: FAIL -- block-level attributes are not extracted
// ---------------------------------------------------------------------------

#[test]
fn check3_block_properties_not_extracted() {
    let result = compile_and_extract_boundary(PL011_SPECIMEN, "check3_props");
    let reg = result
        .register_access
        .expect("register extraction should succeed");

    let block = &reg.register_blocks[0];

    // RegisterBlockDecl has no block-level endian or access_width field.
    // The extraction only parses per-field #[rseq::register(...)] attributes.
    // The PL011 specimen uses #[rseq::register_block(endian = "little",
    // access_width = 32)] on the struct, but individual fields carry no
    // #[rseq::register] attributes.
    //
    // Demonstrate the gap: every field reports None for endian and access_width.
    for field in &block.fields {
        assert_eq!(
            field.endian, None,
            "field '{}': endian not extracted from block-level attribute",
            field.name
        );
        assert_eq!(
            field.access_width, None,
            "field '{}': access_width not extracted from block-level attribute",
            field.name
        );
    }
}

// ---------------------------------------------------------------------------
// Check 4: Reserved region offset preservation
// Premise: reserved regions preserve authored offsets
//   _reserved0: 0x008 -> 0x014
//   _reserved1: 0x01C -> 0x01C
//   _reserved2: 0x04C -> 0xFDC
// Expected outcome: FAIL -- no byte offset data in extraction
// ---------------------------------------------------------------------------

#[test]
fn check4_reserved_regions_lack_offset_data() {
    let result = compile_and_extract_boundary(PL011_SPECIMEN, "check4_reserved");
    let reg = result
        .register_access
        .expect("register extraction should succeed");

    let block = &reg.register_blocks[0];

    // Reserved fields ARE captured by name (they are struct fields).
    let reserved_names: Vec<&str> = block
        .fields
        .iter()
        .filter(|f| f.name.starts_with("_reserved"))
        .map(|f| f.name.as_str())
        .collect();
    assert_eq!(
        reserved_names,
        vec!["_reserved0", "_reserved1", "_reserved2"],
        "all three reserved fields should be present in authored order"
    );

    // RegisterFieldDecl carries: name, access_width, endian, side_effects,
    // hw_updates.  There is no byte-offset field, no array-size field, and
    // no type-identity field.  Therefore exact reserved-region byte ranges
    // (0x008..0x014, 0x01C..0x01C, 0x04C..0xFDC) cannot be reconstructed
    // from the extraction surface alone.
    //
    // Demonstrate: none of the four optional metadata fields carries layout
    // information for reserved fields.
    for field in block
        .fields
        .iter()
        .filter(|f| f.name.starts_with("_reserved"))
    {
        assert_eq!(field.access_width, None);
        assert_eq!(field.endian, None);
        assert_eq!(field.side_effects, None);
        assert_eq!(field.hw_updates, None);
    }
}

// ---------------------------------------------------------------------------
// Check 5: Register inventory completeness and ordering
// Premise: declared register inventory remains complete and in authored
//          offset order
// ---------------------------------------------------------------------------

#[test]
fn check5_register_inventory_and_order() {
    let result = compile_and_extract_boundary(PL011_SPECIMEN, "check5_inventory");
    let reg = result
        .register_access
        .expect("register extraction should succeed");

    let block = &reg.register_blocks[0];

    // Extract the register fields (exclude reserved) in extracted order.
    let extracted_registers: Vec<&str> = block
        .fields
        .iter()
        .filter(|f| !f.name.starts_with("_reserved"))
        .map(|f| f.name.as_str())
        .collect();

    // Exact match against the authored 22-register inventory in offset order.
    assert_eq!(
        extracted_registers.as_slice(),
        AUTHORED_REGISTER_NAMES,
        "extracted register inventory must match authored inventory exactly"
    );
}

// ---------------------------------------------------------------------------
// Check 6: Per-register four-fact preservation
// Premise: each declared register preserves struct field name, register
//          name, byte offset, and access kind
// Expected outcome: FAIL -- only struct field name is preserved
// ---------------------------------------------------------------------------

#[test]
fn check6_per_register_facts_incomplete() {
    let result = compile_and_extract_boundary(PL011_SPECIMEN, "check6_per_reg");
    let reg = result
        .register_access
        .expect("register extraction should succeed");

    let block = &reg.register_blocks[0];

    // (a) struct field name: PRESERVED
    let extracted_names: Vec<&str> = block
        .fields
        .iter()
        .filter(|f| !f.name.starts_with("_reserved"))
        .map(|f| f.name.as_str())
        .collect();
    assert_eq!(extracted_names.as_slice(), AUTHORED_REGISTER_NAMES);

    // (b) register name (e.g. DR_REG): NOT PRESERVED
    // RegisterFieldDecl has no field for the type-parameter register
    // descriptor identity.  The register_descriptors list captures
    // descriptor modules separately, but there is no cross-reference
    // linking field "dr" -> descriptor "DR_REG".

    // (c) byte offset: NOT PRESERVED
    // RegisterFieldDecl has no offset field.

    // (d) access kind (ReadWrite/ReadOnly/WriteOnly): NOT PRESERVED
    // RegisterFieldDecl has no access-kind field.

    // Demonstrate that the only per-field information is the four optional
    // metadata fields, all of which are None for PL011 fields (they use
    // block-level, not per-field, attributes).
    for field in block
        .fields
        .iter()
        .filter(|f| !f.name.starts_with("_reserved"))
    {
        assert_eq!(
            field.access_width, None,
            "field '{}': no per-field access_width",
            field.name
        );
        assert_eq!(
            field.endian, None,
            "field '{}': no per-field endian",
            field.name
        );
    }
}

// ---------------------------------------------------------------------------
// Check 7: No register insertion, removal, renaming, or reordering
// Premise: no registers are inserted, removed, renamed, or reordered
// Evidence type: closed-world (exact-length + exhaustive name match)
// ---------------------------------------------------------------------------

#[test]
fn check7_closed_world_field_preservation() {
    let result = compile_and_extract_boundary(PL011_SPECIMEN, "check7_closed");
    let reg = result
        .register_access
        .expect("register extraction should succeed");

    let block = &reg.register_blocks[0];

    // Exact field count: 25 (22 registers + 3 reserved).
    assert_eq!(
        block.fields.len(),
        AUTHORED_ALL_FIELD_NAMES.len(),
        "field count must match authored total exactly"
    );

    // Exhaustive name-by-name match in authored order.
    let extracted_all: Vec<&str> = block.fields.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        extracted_all.as_slice(),
        AUTHORED_ALL_FIELD_NAMES,
        "full field sequence must match authored sequence exactly"
    );
}

// ---------------------------------------------------------------------------
// Check 8: No register-block properties invented or dropped
// Premise: no register-block properties are invented or dropped
// Expected outcome: FAIL -- endian and access_width are dropped
// ---------------------------------------------------------------------------

#[test]
fn check8_block_properties_dropped() {
    let result = compile_and_extract_boundary(PL011_SPECIMEN, "check8_dropped");
    let reg = result
        .register_access
        .expect("register extraction should succeed");

    let block = &reg.register_blocks[0];

    // The authored register-block properties (endian = "little",
    // access_width = 32) are attributes on #[rseq::register_block(...)].
    // The extraction detects the presence of register_block but does NOT
    // parse its arguments.
    //
    // Verify: RegisterBlockDecl exposes only `name` and `fields` -- there
    // are no block-level property fields.  All per-field metadata is None.
    let any_endian = block.fields.iter().any(|f| f.endian.is_some());
    let any_access_width = block.fields.iter().any(|f| f.access_width.is_some());

    assert!(
        !any_endian,
        "no field carries endian (block-level endian is dropped)"
    );
    assert!(
        !any_access_width,
        "no field carries access_width (block-level access_width is dropped)"
    );
}
