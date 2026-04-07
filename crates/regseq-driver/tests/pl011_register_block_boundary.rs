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

type RaResult = Result<RegisterAccessSurface, RegisterAccessError>;
type IeResult = Result<IdentityEnrichmentSurface, IdentityEnrichmentError>;

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
    register_access: RegisterAccessSurface,
    identity_enrichment: IdentityEnrichmentSurface,
}

fn compile_and_extract(source: &str, test_name: &str) -> BoundaryResult {
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

    let ra_result: Arc<Mutex<Option<RaResult>>> = Arc::new(Mutex::new(None));
    let ie_result: Arc<Mutex<Option<IeResult>>> = Arc::new(Mutex::new(None));

    struct ExtractCallbacks {
        ra: Arc<Mutex<Option<RaResult>>>,
        ie: Arc<Mutex<Option<IeResult>>>,
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
            *self.ra.lock().unwrap() = Some(extract_register_access_surface(tcx));
            *self.ie.lock().unwrap() = Some(extract_identity_enrichment(tcx));
            rustc_driver::Compilation::Stop
        }
    }

    let mut callbacks = ExtractCallbacks {
        ra: ra_result.clone(),
        ie: ie_result.clone(),
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

    let ra = ra_result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis should have run (register access)")
        .expect("register access extraction failed");
    let ie = ie_result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis should have run (identity enrichment)")
        .expect("identity enrichment extraction failed");

    std::fs::remove_dir_all(&test_dir).ok();

    BoundaryResult {
        register_access: ra,
        identity_enrichment: ie,
    }
}

// ---------------------------------------------------------------------------
// Inline fixture mirroring PL011 Pl011Registers authored layout.
//
// Preserves: struct name, field names, field order, field count, reserved
// regions, block-level #[rseq::register_block(endian, access_width)] attribute,
// wrapper-type structure (ReadWrite/ReadOnly/WriteOnly/Reserved), register
// descriptor module names, and a driver struct for identity enrichment.
//
// Omits: register_bitfields! macro expansion internals (field/named-value
// constants) because those are a separate extraction family not in scope for
// the register-block boundary check.
// ---------------------------------------------------------------------------

const PL011_FIXTURE: &str = r#"
    use core::marker::PhantomData;

    pub struct ReadWrite<T, R> { _v: T, _r: PhantomData<R> }
    pub struct ReadOnly<T, R> { _v: T, _r: PhantomData<R> }
    pub struct WriteOnly<T, R> { _v: T, _r: PhantomData<R> }
    pub struct Reserved<T> { _v: T }
    pub struct Mmio<T> { _t: PhantomData<T> }

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

    #[rseq::driver(name = "pl011-boundary-test")]
    pub struct BoundaryTestDriver {
        #[rseq::mmio(id = "uart_regs")]
        pub regs: Mmio<Pl011Registers>,
    }
"#;

/// Authored field names in exact declared order (22 registers + 3 reserved).
const EXPECTED_FIELDS: &[&str] = &[
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
// Boundary validation test
// ---------------------------------------------------------------------------

#[test]
fn pl011_register_block_extraction_boundary() {
    let result = compile_and_extract(PL011_FIXTURE, "pl011_register_block_boundary");

    let ra = &result.register_access;
    let ie = &result.identity_enrichment;

    // -------------------------------------------------------------------
    // Check 1: Register block identity
    // -------------------------------------------------------------------
    assert_eq!(
        ra.register_blocks.len(),
        1,
        "Check 1: exactly one register block extracted"
    );
    let block = &ra.register_blocks[0];
    assert_eq!(
        block.name, "Pl011Registers",
        "Check 1: register block name matches authored identity"
    );

    let rb_struct = ie
        .structs
        .iter()
        .find(|s| s.name == "Pl011Registers")
        .expect("Check 1: Pl011Registers found in identity enrichment");
    assert!(
        rb_struct.is_register_block,
        "Check 1: identity enrichment marks struct as register_block"
    );

    // -------------------------------------------------------------------
    // Check 2: Canonical path preservation
    // -------------------------------------------------------------------
    assert!(
        rb_struct.canonical_path.0 == "Pl011Registers"
            || rb_struct.canonical_path.0.ends_with("::Pl011Registers"),
        "Check 2: canonical path preserves Pl011Registers identity, got: {}",
        rb_struct.canonical_path.0
    );

    // -------------------------------------------------------------------
    // Check 3: Block-level properties (endian, access_width) -- FAIL
    //
    // RegisterBlockDecl has no block-level endian or access_width fields.
    // The authored #[rseq::register_block(endian = "little", access_width = 32)]
    // is used only as a marker to detect the struct. The attribute arguments
    // are not parsed into the extraction record.
    //
    // Per-field #[rseq::register] attrs do not exist on the authored source,
    // so all per-field metadata is None.
    // -------------------------------------------------------------------
    for field in &block.fields {
        assert!(
            field.access_width.is_none(),
            "Check 3: field '{}' has no access_width (block-level attr not parsed)",
            field.name,
        );
        assert!(
            field.endian.is_none(),
            "Check 3: field '{}' has no endian (block-level attr not parsed)",
            field.name,
        );
    }

    // -------------------------------------------------------------------
    // Check 4: Reserved region names survive
    //
    // Reserved field names are present. Byte offsets and array element
    // counts are NOT preserved in RegisterFieldDecl (no offset or type
    // fields). Without block-level access_width (Check 3 gap), exact byte
    // ranges cannot be reconstructed from the extraction record alone.
    // -------------------------------------------------------------------
    let field_names: Vec<&str> = block.fields.iter().map(|f| f.name.as_str()).collect();
    assert!(
        field_names.contains(&"_reserved0"),
        "Check 4: _reserved0 present"
    );
    assert!(
        field_names.contains(&"_reserved1"),
        "Check 4: _reserved1 present"
    );
    assert!(
        field_names.contains(&"_reserved2"),
        "Check 4: _reserved2 present"
    );

    // -------------------------------------------------------------------
    // Check 5: Complete inventory in authored order
    // -------------------------------------------------------------------
    assert_eq!(
        field_names.as_slice(),
        EXPECTED_FIELDS,
        "Check 5: field inventory matches authored order exactly"
    );

    // -------------------------------------------------------------------
    // Check 6: Per-register four facts
    //
    // Struct field name: preserved (Check 5).
    // Register name (e.g., DR_REG): NOT in RegisterFieldDecl.
    // Byte offset: NOT in RegisterFieldDecl.
    // Access kind (RW/R/W): NOT in RegisterFieldDecl.
    //
    // All metadata fields are None for every field, confirming no
    // per-register metadata beyond the name survives.
    // -------------------------------------------------------------------
    for field in &block.fields {
        assert!(
            field.access_width.is_none()
                && field.endian.is_none()
                && field.side_effects.is_none()
                && field.hw_updates.is_none(),
            "Check 6: field '{}' carries no register metadata",
            field.name,
        );
    }

    // -------------------------------------------------------------------
    // Check 7: No insertions, removals, renames, or reordering
    // -------------------------------------------------------------------
    assert_eq!(
        block.fields.len(),
        EXPECTED_FIELDS.len(),
        "Check 7: no registers inserted or removed"
    );

    // -------------------------------------------------------------------
    // Check 8: No invented properties
    //
    // Block-level endian and access_width are dropped (Check 3).
    // No per-field properties are invented -- all metadata is None.
    // -------------------------------------------------------------------
    for field in &block.fields {
        assert!(
            field.side_effects.is_none() && field.hw_updates.is_none(),
            "Check 8: field '{}' has no invented properties",
            field.name,
        );
    }
}
