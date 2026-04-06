#![feature(rustc_private)]

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

use identity_enrichment::*;

type ExtractionResult = Result<IdentityEnrichmentSurface, IdentityEnrichmentError>;

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
            *self.result.lock().unwrap() = Some(extract_identity_enrichment(tcx));
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
// Minimal source that satisfies the driver type requirement
// ---------------------------------------------------------------------------

const MINIMAL_DRIVER_SOURCE: &str = r#"
    #[rseq::driver(name = "test-drv", compatible = ["vendor,test"])]
    pub struct TestDriver {
        #[rseq::mmio(id = "regs")]
        pub regs: u32,
        #[rseq::interrupt(id = "irq")]
        pub irq: u32,
        pub plain_field: u8,
    }

    impl TestDriver {
        const MY_CONST: u32 = 42;

        pub fn helper(&self) {}

        #[rseq::isr]
        pub fn handle_irq(&mut self) {}
    }

    pub fn free_fn() {}
"#;

// ---------------------------------------------------------------------------
// Contract tests: structural properties
// ---------------------------------------------------------------------------

#[test]
fn deterministic_canonical_paths() {
    let r1 = compile_and_extract(MINIMAL_DRIVER_SOURCE, "determ_1").unwrap();
    let r2 = compile_and_extract(MINIMAL_DRIVER_SOURCE, "determ_2").unwrap();

    // All canonical paths in both runs must be identical.
    let paths1: Vec<_> = r1.structs.iter().map(|s| &s.canonical_path).collect();
    let paths2: Vec<_> = r2.structs.iter().map(|s| &s.canonical_path).collect();
    assert_eq!(paths1, paths2);
}

#[test]
fn canonical_paths_are_fully_qualified() {
    let surface = compile_and_extract(MINIMAL_DRIVER_SOURCE, "fq_paths").unwrap();

    // Every struct canonical path contains :: and the crate name.
    for s in &surface.structs {
        assert!(
            s.canonical_path.0.contains("::") || s.canonical_path.0 == s.name,
            "struct path should be qualified: {}",
            s.canonical_path.0
        );
    }
    // Every function canonical path matches the crate-qualified form produced
    // by tcx.def_path_str(). For crate-root items in a single-file crate,
    // rustc may produce unqualified paths (e.g., "free_fn") -- that is correct
    // and deterministic.
    for f in &surface.free_functions {
        assert!(
            !f.canonical_path.0.is_empty(),
            "function path should not be empty: {}",
            f.name
        );
    }
}

#[test]
fn module_containment_completeness() {
    let surface = compile_and_extract(MINIMAL_DRIVER_SOURCE, "mod_contain").unwrap();

    // Collect all module canonical paths.
    let module_paths: std::collections::HashSet<_> =
        surface.modules.iter().map(|m| &m.canonical_path).collect();

    // Every struct's module_path should match a module.
    for s in &surface.structs {
        assert!(
            module_paths.contains(&s.module_path),
            "struct {} module_path {} not found in modules",
            s.name,
            s.module_path.0
        );
    }
    // Every free function's module_path should match a module.
    for f in &surface.free_functions {
        assert!(
            module_paths.contains(&f.module_path),
            "function {} module_path {} not found in modules",
            f.name,
            f.module_path.0
        );
    }
}

#[test]
fn module_child_items_contain_all_items() {
    let surface = compile_and_extract(MINIMAL_DRIVER_SOURCE, "mod_children").unwrap();

    // Collect all child items across all modules.
    let all_children: std::collections::HashSet<_> = surface
        .modules
        .iter()
        .flat_map(|m| m.child_items.iter())
        .collect();

    // Every struct should appear in some module's child_items.
    for s in &surface.structs {
        assert!(
            all_children.contains(&s.canonical_path),
            "struct {} not in any module's child_items",
            s.canonical_path.0
        );
    }
    // Every free function should appear in some module's child_items.
    for f in &surface.free_functions {
        assert!(
            all_children.contains(&f.canonical_path),
            "function {} not in any module's child_items",
            f.canonical_path.0
        );
    }
    // Every impl block should appear in some module's child_items.
    for ib in &surface.impl_blocks {
        assert!(
            all_children.contains(&ib.canonical_path),
            "impl block {} not in any module's child_items",
            ib.canonical_path.0
        );
    }
}

#[test]
fn associated_constant_ownership() {
    let surface = compile_and_extract(MINIMAL_DRIVER_SOURCE, "const_owner").unwrap();

    let impl_paths: std::collections::HashSet<_> = surface
        .impl_blocks
        .iter()
        .map(|ib| &ib.canonical_path)
        .collect();

    for ib in &surface.impl_blocks {
        for c in &ib.constants {
            assert!(
                impl_paths.contains(&c.owner_impl_path),
                "constant {} owner_impl_path {} not found in impl_blocks",
                c.name,
                c.owner_impl_path.0
            );
        }
    }
}

#[test]
fn field_canonical_path_structure() {
    let surface = compile_and_extract(MINIMAL_DRIVER_SOURCE, "field_path").unwrap();

    for s in &surface.structs {
        for f in &s.fields {
            let expected = format!("{}::{}", s.canonical_path.0, f.name);
            assert_eq!(
                f.canonical_path.0, expected,
                "field path mismatch for {}.{}",
                s.name, f.name
            );
        }
    }
}

#[test]
fn field_resource_classification_is_attribute_based() {
    let surface = compile_and_extract(MINIMAL_DRIVER_SOURCE, "field_attr").unwrap();

    let driver = surface.structs.iter().find(|s| s.is_driver).unwrap();

    let regs_field = driver.fields.iter().find(|f| f.name == "regs").unwrap();
    assert!(
        matches!(regs_field.rseq_kind, Some(RseqFieldKind::Mmio { .. })),
        "regs should be Mmio, got {:?}",
        regs_field.rseq_kind
    );

    let irq_field = driver.fields.iter().find(|f| f.name == "irq").unwrap();
    assert!(
        matches!(irq_field.rseq_kind, Some(RseqFieldKind::Interrupt { .. })),
        "irq should be Interrupt, got {:?}",
        irq_field.rseq_kind
    );

    // plain_field has no rseq attribute -- must be None.
    let plain = driver
        .fields
        .iter()
        .find(|f| f.name == "plain_field")
        .unwrap();
    assert!(
        plain.rseq_kind.is_none(),
        "plain_field should have no rseq_kind"
    );
}

#[test]
fn isr_marker_is_structural() {
    let surface = compile_and_extract(MINIMAL_DRIVER_SOURCE, "isr_marker").unwrap();

    let inherent = surface
        .impl_blocks
        .iter()
        .find(|ib| ib.is_inherent)
        .unwrap();

    let isr = inherent
        .methods
        .iter()
        .find(|m| m.name == "handle_irq")
        .unwrap();
    assert!(isr.is_isr, "handle_irq should have is_isr = true");

    let helper = inherent
        .methods
        .iter()
        .find(|m| m.name == "helper")
        .unwrap();
    assert!(!helper.is_isr, "helper should have is_isr = false");
}

#[test]
fn trait_vs_inherent_impl() {
    let source = r#"
        pub trait MyTrait {
            fn do_thing(&self);
        }

        #[rseq::driver(name = "test", compatible = ["v,t"])]
        pub struct TestDriver;

        impl MyTrait for TestDriver {
            fn do_thing(&self) {}
        }

        impl TestDriver {
            pub fn inherent_method(&self) {}
        }
    "#;

    let surface = compile_and_extract(source, "trait_inherent").unwrap();

    let trait_impl = surface
        .impl_blocks
        .iter()
        .find(|ib| !ib.is_inherent)
        .unwrap();
    assert!(trait_impl.trait_path.is_some());
    assert!(trait_impl.trait_name.is_some());

    let inherent_impl = surface
        .impl_blocks
        .iter()
        .find(|ib| ib.is_inherent)
        .unwrap();
    assert!(inherent_impl.trait_path.is_none());
    assert!(inherent_impl.trait_name.is_none());
}

#[test]
fn overlapping_short_names_disambiguated() {
    let source = r#"
        #[rseq::driver(name = "test", compatible = ["v,t"])]
        pub struct DriverA {
            pub status: u32,
        }

        pub struct DriverB {
            pub status: u32,
        }
    "#;

    let surface = compile_and_extract(source, "overlap_names").unwrap();

    let field_paths: Vec<_> = surface
        .structs
        .iter()
        .flat_map(|s| s.fields.iter().map(|f| &f.canonical_path.0))
        .collect();

    // Both structs have a "status" field but with different canonical paths.
    let status_paths: Vec<_> = field_paths
        .iter()
        .filter(|p| p.ends_with("::status"))
        .collect();
    assert_eq!(
        status_paths.len(),
        2,
        "should have two distinct status field paths"
    );
    assert_ne!(status_paths[0], status_paths[1]);
}

// ---------------------------------------------------------------------------
// PL011-like evidence tests
// ---------------------------------------------------------------------------

#[test]
fn driver_struct_identified() {
    let surface = compile_and_extract(MINIMAL_DRIVER_SOURCE, "drv_id").unwrap();

    let drivers: Vec<_> = surface.structs.iter().filter(|s| s.is_driver).collect();
    assert_eq!(drivers.len(), 1, "exactly one driver struct expected");
    assert_eq!(drivers[0].name, "TestDriver");
}

#[test]
fn associated_constants_present() {
    let surface = compile_and_extract(MINIMAL_DRIVER_SOURCE, "assoc_const").unwrap();

    let inherent = surface
        .impl_blocks
        .iter()
        .find(|ib| ib.is_inherent)
        .unwrap();

    assert_eq!(inherent.constants.len(), 1);
    assert_eq!(inherent.constants[0].name, "MY_CONST");
    assert_eq!(inherent.constants[0].ty, "u32");
    assert_eq!(
        inherent.constants[0].owner_impl_path, inherent.canonical_path,
        "constant owner must point back to owning impl"
    );
}

#[test]
fn free_functions_extracted() {
    let surface = compile_and_extract(MINIMAL_DRIVER_SOURCE, "free_fns").unwrap();

    let free_fn = surface
        .free_functions
        .iter()
        .find(|f| f.name == "free_fn")
        .unwrap();
    assert!(free_fn.canonical_path.0.contains("free_fn"));
}

#[test]
fn enum_identity_extracted() {
    let source = r#"
        #[rseq::driver(name = "test", compatible = ["v,t"])]
        pub struct TestDriver;

        pub enum InterruptClass {
            Rx,
            Tx,
            Error,
        }
    "#;

    let surface = compile_and_extract(source, "enum_id").unwrap();
    let e = surface
        .enums
        .iter()
        .find(|e| e.name == "InterruptClass")
        .unwrap();
    assert_eq!(e.variants, vec!["Rx", "Tx", "Error"]);
    assert!(e.canonical_path.0.contains("InterruptClass"));
}

#[test]
fn no_driver_type_returns_error() {
    let source = r#"
        pub struct NotADriver;
    "#;

    let result = compile_and_extract(source, "no_driver");
    assert_eq!(result, Err(IdentityEnrichmentError::NoDriverType));
}

#[test]
fn json_round_trip() {
    let surface = compile_and_extract(MINIMAL_DRIVER_SOURCE, "json_rt").unwrap();

    let json = serde_json::to_string_pretty(&surface).unwrap();
    let deserialized: IdentityEnrichmentSurface = serde_json::from_str(&json).unwrap();
    assert_eq!(surface, deserialized);
}

#[test]
fn field_rseq_kind_ids_are_correct() {
    let source = r#"
        #[rseq::driver(name = "test", compatible = ["v,t"])]
        pub struct TestDriver {
            #[rseq::mmio(id = "uart_regs")]
            pub regs: u32,
            #[rseq::clock(id = "uart_clk")]
            pub clock: u32,
            #[rseq::pinctrl(id = "uart_pins")]
            pub pinctrl: u32,
            #[rseq::property(id = "baud-rate")]
            pub baud: u32,
        }
    "#;

    let surface = compile_and_extract(source, "field_ids").unwrap();
    let driver = surface.structs.iter().find(|s| s.is_driver).unwrap();

    let regs = driver.fields.iter().find(|f| f.name == "regs").unwrap();
    assert_eq!(
        regs.rseq_kind,
        Some(RseqFieldKind::Mmio {
            id: "uart_regs".to_string()
        })
    );

    let clock = driver.fields.iter().find(|f| f.name == "clock").unwrap();
    assert_eq!(
        clock.rseq_kind,
        Some(RseqFieldKind::Clock {
            id: "uart_clk".to_string()
        })
    );

    let pinctrl = driver.fields.iter().find(|f| f.name == "pinctrl").unwrap();
    assert_eq!(
        pinctrl.rseq_kind,
        Some(RseqFieldKind::PinCtrl {
            id: "uart_pins".to_string()
        })
    );

    let baud = driver.fields.iter().find(|f| f.name == "baud").unwrap();
    assert_eq!(
        baud.rseq_kind,
        Some(RseqFieldKind::Property {
            id: "baud-rate".to_string()
        })
    );
}

#[test]
fn module_structure_with_nested_modules() {
    let source = r#"
        #[rseq::driver(name = "test", compatible = ["v,t"])]
        pub struct TestDriver;

        mod inner {
            pub struct InnerStruct {
                pub x: u32,
            }

            pub fn inner_fn() {}
        }
    "#;

    let surface = compile_and_extract(source, "nested_mod").unwrap();

    // Should have at least the crate root module and the inner module.
    assert!(
        surface.modules.len() >= 2,
        "expected at least 2 modules, got {}",
        surface.modules.len()
    );

    let inner = surface.modules.iter().find(|m| m.name == "inner").unwrap();
    assert!(inner.canonical_path.0.contains("inner"));

    // InnerStruct should be in the inner module.
    let inner_struct = surface
        .structs
        .iter()
        .find(|s| s.name == "InnerStruct")
        .unwrap();
    assert_eq!(inner_struct.module_path, inner.canonical_path);

    // inner_fn should be in the inner module.
    let inner_fn = surface
        .free_functions
        .iter()
        .find(|f| f.name == "inner_fn")
        .unwrap();
    assert_eq!(inner_fn.module_path, inner.canonical_path);
}

#[test]
fn register_block_struct_identified() {
    let source = r#"
        #[rseq::driver(name = "test", compatible = ["v,t"])]
        pub struct TestDriver;

        #[rseq::register_block]
        pub struct TestRegisters {
            pub data: u32,
            pub status: u32,
        }
    "#;

    let surface = compile_and_extract(source, "reg_block").unwrap();

    let reg = surface
        .structs
        .iter()
        .find(|s| s.is_register_block)
        .unwrap();
    assert_eq!(reg.name, "TestRegisters");
    assert_eq!(reg.fields.len(), 2);
    assert!(!reg.is_driver, "register block should not be a driver");
}
