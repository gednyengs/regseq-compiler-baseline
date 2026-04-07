//! Boundary validation: PL011 register-block authored -> extraction.
//!
//! Validates checks 1-8 from
//! `docs/validation/01-pl011/checks/01-register-block-extraction.md`
//! against the compiler-side extraction surfaces (`RegisterAccessSurface`
//! and `IdentityEnrichmentSurface`).
//!
//! Specimen: inline mirror of `tests/fixtures/pl011-full-spec/src/registers.rs`
//! preserving every structural fact required by the checks.

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

type BothSurfaces = (
    Result<RegisterAccessSurface, RegisterAccessError>,
    Result<IdentityEnrichmentSurface, IdentityEnrichmentError>,
);

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

/// Compile the inline specimen and extract both the register-access and
/// identity-enrichment surfaces in a single compiler invocation.
fn compile_and_extract_both(source: &str, test_name: &str, crate_name: &str) -> BothSurfaces {
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

    let result: Arc<Mutex<Option<BothSurfaces>>> = Arc::new(Mutex::new(None));

    struct ExtractCallbacks {
        result: Arc<Mutex<Option<BothSurfaces>>>,
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
            let ra = register_access::extract_register_access_surface(tcx);
            let ie = identity_enrichment::extract_identity_enrichment(tcx);
            *self.result.lock().unwrap() = Some((ra, ie));
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
        crate_name.to_string(),
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
// Inline specimen: faithful mirror of the authored PL011 register block
// ---------------------------------------------------------------------------

const PL011_SPECIMEN: &str = r#"
    use core::marker::PhantomData;

    pub struct Mmio<T> { _t: PhantomData<T> }
    pub struct ReadWrite<T, R> { _v: T, _r: PhantomData<R> }
    pub struct ReadOnly<T, R> { _v: T, _r: PhantomData<R> }
    pub struct WriteOnly<T, R> { _v: T, _r: PhantomData<R> }
    pub struct Reserved<T> { _v: T }

    macro_rules! reg_mod {
        ($($name:ident),* $(,)?) => {
            $(
                #[allow(non_snake_case)]
                pub mod $name { pub struct Register; }
            )*
        }
    }

    reg_mod!(
        DR_REG, RSR_ECR_REG, FR_REG, ILPR_REG, IBRD_REG, FBRD_REG,
        LCRH_REG, CR_REG, IFLS_REG, IMSC_REG, RIS_REG, MIS_REG,
        ICR_REG, DMACR_REG,
        PERIPHID0_REG, PERIPHID1_REG, PERIPHID2_REG, PERIPHID3_REG,
        PCELLID0_REG, PCELLID1_REG, PCELLID2_REG, PCELLID3_REG,
    );

    pub mod registers {
        use super::*;

        #[repr(C)]
        #[rseq::register_block(endian = "little", access_width = 32)]
        pub struct Pl011Registers {
            /// 0x000 - Data Register.
            pub dr: ReadWrite<u32, DR_REG::Register>,

            /// 0x004 - Receive Status / Error Clear Register.
            pub rsr_ecr: ReadWrite<u32, RSR_ECR_REG::Register>,

            /// 0x008..0x014 - Reserved.
            _reserved0: [Reserved<u32>; 4],

            /// 0x018 - Flag Register.
            pub fr: ReadOnly<u32, FR_REG::Register>,

            /// 0x01C - Reserved.
            _reserved1: Reserved<u32>,

            /// 0x020 - IrDA Low-Power Counter Register.
            pub ilpr: ReadWrite<u32, ILPR_REG::Register>,

            /// 0x024 - Integer Baud Rate Register.
            pub ibrd: ReadWrite<u32, IBRD_REG::Register>,

            /// 0x028 - Fractional Baud Rate Register.
            pub fbrd: ReadWrite<u32, FBRD_REG::Register>,

            /// 0x02C - Line Control Register.
            pub lcr_h: ReadWrite<u32, LCRH_REG::Register>,

            /// 0x030 - Control Register.
            pub cr: ReadWrite<u32, CR_REG::Register>,

            /// 0x034 - Interrupt FIFO Level Select Register.
            pub ifls: ReadWrite<u32, IFLS_REG::Register>,

            /// 0x038 - Interrupt Mask Set/Clear Register.
            pub imsc: ReadWrite<u32, IMSC_REG::Register>,

            /// 0x03C - Raw Interrupt Status Register.
            pub ris: ReadOnly<u32, RIS_REG::Register>,

            /// 0x040 - Masked Interrupt Status Register.
            pub mis: ReadOnly<u32, MIS_REG::Register>,

            /// 0x044 - Interrupt Clear Register.
            pub icr: WriteOnly<u32, ICR_REG::Register>,

            /// 0x048 - DMA Control Register.
            pub dmacr: ReadWrite<u32, DMACR_REG::Register>,

            /// 0x04C..0xFDC - Reserved and test-only regions.
            _reserved2: [Reserved<u32>; 997],

            /// 0xFE0 - Peripheral Identification Register 0.
            pub periph_id0: ReadOnly<u32, PERIPHID0_REG::Register>,

            /// 0xFE4 - Peripheral Identification Register 1.
            pub periph_id1: ReadOnly<u32, PERIPHID1_REG::Register>,

            /// 0xFE8 - Peripheral Identification Register 2.
            pub periph_id2: ReadOnly<u32, PERIPHID2_REG::Register>,

            /// 0xFEC - Peripheral Identification Register 3.
            pub periph_id3: ReadOnly<u32, PERIPHID3_REG::Register>,

            /// 0xFF0 - PrimeCell Identification Register 0.
            pub pcell_id0: ReadOnly<u32, PCELLID0_REG::Register>,

            /// 0xFF4 - PrimeCell Identification Register 1.
            pub pcell_id1: ReadOnly<u32, PCELLID1_REG::Register>,

            /// 0xFF8 - PrimeCell Identification Register 2.
            pub pcell_id2: ReadOnly<u32, PCELLID2_REG::Register>,

            /// 0xFFC - PrimeCell Identification Register 3.
            pub pcell_id3: ReadOnly<u32, PCELLID3_REG::Register>,
        }
    }

    #[rseq::driver(name = "pl011")]
    pub struct Pl011Driver {
        pub dummy: u32,
    }
"#;

/// Authored field names in exact authored order (all fields including reserved).
const AUTHORED_ALL_FIELDS: &[&str] = &[
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

/// Authored declared (non-reserved) registers in offset order.
const AUTHORED_DECLARED_REGISTERS: &[&str] = &[
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
// Check 1: register block identity preserved
// ---------------------------------------------------------------------------

#[test]
fn check1_register_block_identity() {
    let (ra, _ie) = compile_and_extract_both(PL011_SPECIMEN, "pl011_rb_check1", "pl011_full_spec");
    let surface = ra.unwrap();

    assert_eq!(
        surface.register_blocks.len(),
        1,
        "exactly one register block expected"
    );
    assert_eq!(surface.register_blocks[0].name, "Pl011Registers");
}

// ---------------------------------------------------------------------------
// Check 2: canonical path preserved
// ---------------------------------------------------------------------------

#[test]
fn check2_canonical_path() {
    let (_ra, ie) = compile_and_extract_both(PL011_SPECIMEN, "pl011_rb_check2", "pl011_full_spec");
    let surface = ie.unwrap();

    let reg_block_struct = surface
        .structs
        .iter()
        .find(|s| s.name == "Pl011Registers")
        .expect("Pl011Registers struct must appear in identity enrichment");

    assert!(
        reg_block_struct.is_register_block,
        "struct must be marked as register_block"
    );

    // def_path_str() returns crate-relative paths for local items.
    // The struct canonical path preserves the module-qualified path.
    assert_eq!(
        reg_block_struct.canonical_path.0, "registers::Pl011Registers",
        "struct canonical path must preserve module-qualified identity"
    );

    // The containing module path is also preserved.
    assert_eq!(
        reg_block_struct.module_path.0, "registers",
        "containing module path must be preserved"
    );

    // The crate root module preserves the crate name, enabling exact
    // reconstruction: crate_name + "::" + canonical_path =
    // "pl011_full_spec::registers::Pl011Registers"
    let crate_root = surface
        .modules
        .iter()
        .find(|m| m.child_items.iter().any(|c| c.0 == "registers"))
        .expect("crate root module must contain the registers module");
    assert_eq!(
        crate_root.canonical_path.0, "pl011_full_spec",
        "crate root must carry the crate name"
    );

    // Verify full canonical path is reconstructable.
    let reconstructed = format!(
        "{}::{}",
        crate_root.canonical_path.0, reg_block_struct.canonical_path.0
    );
    assert_eq!(
        reconstructed, "pl011_full_spec::registers::Pl011Registers",
        "full canonical path must be exactly reconstructable from extraction evidence"
    );
}

// ---------------------------------------------------------------------------
// Check 3: block-level properties (endian, access_width) preservation
// ---------------------------------------------------------------------------

#[test]
fn check3_block_properties_not_extracted() {
    let (ra, _ie) = compile_and_extract_both(PL011_SPECIMEN, "pl011_rb_check3", "pl011_full_spec");
    let surface = ra.unwrap();
    let block = &surface.register_blocks[0];

    // RegisterBlockDecl has no block-level endian or access_width fields.
    // The only property metadata lives on individual RegisterFieldDecl entries
    // via #[rseq::register(...)], which the authored source does not use.
    // Verify that no per-field metadata carries block-level properties either.
    for field in &block.fields {
        assert_eq!(
            field.endian, None,
            "field '{}' should have no endian metadata (no per-field #[rseq::register])",
            field.name
        );
        assert_eq!(
            field.access_width, None,
            "field '{}' should have no access_width metadata (no per-field #[rseq::register])",
            field.name
        );
    }

    // This proves the extraction boundary drops block-level endian=little and
    // access_width=32. RegisterBlockDecl has no fields to carry them.
}

// ---------------------------------------------------------------------------
// Check 4: reserved region offset preservation
// ---------------------------------------------------------------------------

#[test]
fn check4_reserved_regions_names_present_offsets_absent() {
    let (ra, _ie) = compile_and_extract_both(PL011_SPECIMEN, "pl011_rb_check4", "pl011_full_spec");
    let surface = ra.unwrap();
    let block = &surface.register_blocks[0];

    // Reserved field names are captured.
    let reserved_names: Vec<&str> = block
        .fields
        .iter()
        .filter(|f| f.name.starts_with("_reserved"))
        .map(|f| f.name.as_str())
        .collect();
    assert_eq!(
        reserved_names,
        vec!["_reserved0", "_reserved1", "_reserved2"],
        "all three reserved field names must be present in order"
    );

    // RegisterFieldDecl carries no byte-offset, type, or array-length data.
    // Therefore the authored reserved-region ranges cannot be reconstructed:
    //   _reserved0: 0x008 -> 0x014  (array of 4)
    //   _reserved1: 0x01C -> 0x01C  (single)
    //   _reserved2: 0x04C -> 0xFDC  (array of 997)
    //
    // The field names alone survive; the byte ranges do not.
}

// ---------------------------------------------------------------------------
// Check 5: complete register inventory in authored order
// ---------------------------------------------------------------------------

#[test]
fn check5_inventory_complete_and_ordered() {
    let (ra, _ie) = compile_and_extract_both(PL011_SPECIMEN, "pl011_rb_check5", "pl011_full_spec");
    let surface = ra.unwrap();
    let block = &surface.register_blocks[0];

    let extracted_names: Vec<&str> = block.fields.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        extracted_names, AUTHORED_ALL_FIELDS,
        "complete field inventory must match authored order exactly"
    );
}

// ---------------------------------------------------------------------------
// Check 6: per-register four-fact preservation
// ---------------------------------------------------------------------------

#[test]
fn check6_per_register_facts() {
    let (ra, _ie) = compile_and_extract_both(PL011_SPECIMEN, "pl011_rb_check6", "pl011_full_spec");
    let surface = ra.unwrap();
    let block = &surface.register_blocks[0];

    // Fact 1: struct field name -- preserved.
    let declared: Vec<&str> = block
        .fields
        .iter()
        .filter(|f| !f.name.starts_with("_reserved"))
        .map(|f| f.name.as_str())
        .collect();
    assert_eq!(
        declared, AUTHORED_DECLARED_REGISTERS,
        "declared register field names must match"
    );

    // Facts 2-4: register name, byte offset, access kind -- NOT preserved.
    // RegisterFieldDecl has no register_name, offset, or access_kind fields.
    // These facts are dropped at this extraction boundary.
    for field in &block.fields {
        if field.name.starts_with("_reserved") {
            continue;
        }
        // The only metadata available is from #[rseq::register(...)], which
        // the authored source does not use. All per-field metadata is None.
        assert_eq!(field.endian, None);
        assert_eq!(field.access_width, None);
        assert_eq!(field.side_effects, None);
        assert_eq!(field.hw_updates, None);
    }
}

// ---------------------------------------------------------------------------
// Check 7: no insertions, removals, renames, or reordering
// ---------------------------------------------------------------------------

#[test]
fn check7_no_spurious_changes() {
    let (ra, _ie) = compile_and_extract_both(PL011_SPECIMEN, "pl011_rb_check7", "pl011_full_spec");
    let surface = ra.unwrap();
    let block = &surface.register_blocks[0];

    let extracted_names: Vec<&str> = block.fields.iter().map(|f| f.name.as_str()).collect();

    // Exact match proves no insertion, removal, rename, or reorder.
    assert_eq!(
        extracted_names, AUTHORED_ALL_FIELDS,
        "extraction must match authored field list exactly"
    );

    // Only one register block exists -- no extra blocks invented.
    assert_eq!(surface.register_blocks.len(), 1);
}

// ---------------------------------------------------------------------------
// Check 8: no properties invented or dropped
// ---------------------------------------------------------------------------

#[test]
fn check8_block_properties_dropped() {
    let (ra, _ie) = compile_and_extract_both(PL011_SPECIMEN, "pl011_rb_check8", "pl011_full_spec");
    let surface = ra.unwrap();
    let block = &surface.register_blocks[0];

    // RegisterBlockDecl carries only `name` and `fields`.
    // The authored block-level properties (endian=little, access_width=32) are
    // derived from #[rseq::register_block(endian = "little", access_width = 32)]
    // but the extraction only checks attribute existence via has_rseq_attr()
    // and never parses the attribute arguments at the block level.
    //
    // Verify no per-field metadata carries these either:
    let any_endian = block.fields.iter().any(|f| f.endian.is_some());
    let any_access_width = block.fields.iter().any(|f| f.access_width.is_some());
    assert!(!any_endian, "no field should carry endian metadata");
    assert!(
        !any_access_width,
        "no field should carry access_width metadata"
    );

    // This proves the authored register-block properties are dropped at the
    // extraction boundary.
}
