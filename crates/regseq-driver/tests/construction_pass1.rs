//! Construction Pass 1 specimens (regseq-model-41).
//!
//! Each test compiles a small inline specimen through a rustc harness,
//! invokes `build_declaration_skeleton`, and asserts the structure of
//! the returned [`RegseqModel`]. Assertions are purely structural -- no
//! JSON serialization (task 54 owns that).

#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

use std::sync::{Arc, Mutex, Once};

use regseq_driver::construction::{
    build_declaration_skeleton, ConstructionError, ExtractionRegistries, PinStateRegistry,
};
use regseq_driver::extract::abstraction_types::AbstractionTypeRegistry;
use regseq_driver::extract::framework_types::FrameworkTypeRegistry;
use regseq_driver::extract::intrinsic_registry::IntrinsicRegistry;
use regseq_model::identity::SourceOrigin;
use regseq_model::model::{ModuleKind, RegseqModel, TypeDefClassification};

type ConstructionOutcome = Result<RegseqModel, ConstructionError>;

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

fn compile_and_build(source: &str, test_name: &str) -> ConstructionOutcome {
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

    let result: Arc<Mutex<Option<ConstructionOutcome>>> = Arc::new(Mutex::new(None));

    struct Callbacks {
        result: Arc<Mutex<Option<ConstructionOutcome>>>,
    }

    impl rustc_driver::Callbacks for Callbacks {
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
            let intrinsic = IntrinsicRegistry::build(tcx);
            let framework = FrameworkTypeRegistry::build(tcx);
            let abstraction = AbstractionTypeRegistry::build(tcx);
            let pin_states = PinStateRegistry::build(tcx);
            let registries = ExtractionRegistries {
                intrinsic: &intrinsic,
                framework_types: &framework,
                abstraction_types: &abstraction,
                pin_states: &pin_states,
            };
            let outcome = build_declaration_skeleton(tcx, &registries);
            *self.result.lock().unwrap() = Some(outcome);
            rustc_driver::Compilation::Stop
        }
    }

    let mut callbacks = Callbacks {
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
        "specimen".to_string(),
        "--out-dir".to_string(),
        test_dir.to_str().unwrap().to_string(),
    ];

    rustc_driver::run_compiler(&args, &mut callbacks);

    let outcome = result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis must run");

    std::fs::remove_dir_all(&test_dir).ok();

    outcome
}

// ---------------------------------------------------------------------------
// Positive specimen: minimal driver exercising every category
// ---------------------------------------------------------------------------

const POSITIVE_SOURCE: &str = r#"
#![no_std]

// --- Framework facade stubs (no real `regseq` crate here) ---
pub struct Clock;
pub struct ResetLine;
pub struct PinCtrl;
pub struct Interrupt;
pub struct Mmio<T> { _marker: core::marker::PhantomData<T> }
pub struct Mutex<T> { _v: T }
pub struct Completion;

pub struct ReadWrite<T, R> { _v: T, _r: core::marker::PhantomData<R> }
pub struct Reserved<T> { _v: T }

// --- Register descriptor module (emulates register_bitfields! expansion) ---
#[allow(non_snake_case)]
pub mod DR_REG {
    pub struct Register;
}

// --- Register block ---
#[repr(C)]
#[rseq::register_block(endian = "little", access_width = 32)]
pub struct MyBlock {
    pub dr: ReadWrite<u32, DR_REG::Register>,
}

// --- Pin state marker ---
#[rseq::pin_state(id = "default")]
pub struct DefaultState;

// --- Top-level constant ---
pub const BAUD_RATE: u32 = 115200;

// --- Free helper function ---
pub fn double(x: u32) -> u32 { x * 2 }

// --- Driver struct ---
#[rseq::driver(name = "specimen-drv", compatible = ["acme,widget"], bus = "amba")]
pub struct MyDriver {
    #[rseq::clock(id = "core_clk")]
    pub clk: Clock,
    #[rseq::reset(id = "core_rst")]
    pub rst: Option<ResetLine>,
    #[rseq::pinctrl(id = "pinmux")]
    pub pins: PinCtrl,
    #[rseq::interrupt(id = "core_irq")]
    pub irq: Interrupt,
    #[rseq::mmio(id = "regs")]
    pub regs: Mmio<MyBlock>,
    pub lock: Mutex<u32>,
    #[rseq::property(id = "sample-rate", default = 48000)]
    pub sample_rate: u32,
    pub pending: Option<u8>,
}

// --- Trait impl ---
pub trait Lifecycle { fn init(&mut self); }

impl Lifecycle for MyDriver {
    fn init(&mut self) {}
}

// --- Driver impl block with helper + ISR ---
impl MyDriver {
    pub fn helper(&self) {}

    #[rseq::isr]
    pub fn on_irq(&mut self) {}
}
"#;

#[test]
fn positive_specimen_constructs_declaration_skeleton() {
    let model = compile_and_build(POSITIVE_SOURCE, "construction_pass1_positive")
        .expect("positive specimen must succeed");

    // Exactly one Driver module.
    let driver_count = count_driver_modules(&model);
    assert_eq!(driver_count, 1, "exactly one Driver module must exist");

    // The driver struct lives on one of the modules.
    let driver_module = find_driver_module(&model).expect("driver module must exist");
    let driver = driver_module
        .driver
        .as_ref()
        .expect("driver module must carry a DriverStruct");

    // Each field category populated.
    assert_eq!(driver.clocks.len(), 1, "one clock resource");
    assert_eq!(driver.resets.len(), 1, "one reset resource");
    assert_eq!(driver.pinctrl.len(), 1, "one pinctrl resource");
    assert_eq!(driver.interrupts.len(), 1, "one interrupt resource");
    assert_eq!(driver.mmio.len(), 1, "one mmio resource");
    assert_eq!(driver.sync.len(), 1, "one sync primitive");
    assert_eq!(driver.properties.len(), 1, "one property");
    assert_eq!(driver.user_data.len(), 1, "one user-data field");

    // All binding slots None.
    assert!(driver.clocks[0].binding.is_none());
    assert!(driver.resets[0].binding.is_none());
    assert!(driver.pinctrl[0].binding.is_none());
    assert!(driver.interrupts[0].binding.is_none());
    assert!(driver.mmio[0].binding.is_none());
    assert!(driver.properties[0].bound_value.is_none());

    // Constants populated.
    assert!(
        count_constants(&model) >= 1,
        "at least one top-level constant must appear"
    );

    // Register structures present.
    assert!(
        count_register_blocks(&model) >= 1,
        "at least one register block"
    );
    assert!(
        count_register_descriptors(&model) >= 1,
        "at least one register descriptor"
    );

    // MMIO resource resolves to the correct RegisterBlockId (the
    // specimen has exactly one block, so the MMIO decl must point at
    // it, not a placeholder id).
    let mmio_block_id = driver.mmio[0].decl.register_block;
    let expected_block_id = collect_register_block_ids(&model)[0];
    assert_eq!(
        mmio_block_id, expected_block_id,
        "MMIO register_block must resolve to the actual RegisterBlockId",
    );

    // source_locations bijection: every allocated DeclId appears as a
    // key and no extras. We approximate "every DeclId" by collecting
    // every id we can reach through the model.
    let reachable = collect_reachable_decl_ids(&model);
    for id in &reachable {
        assert!(
            model.source_locations.contains_key(id),
            "every reachable DeclId must have a source_locations entry: {id:?}"
        );
    }

    // TypeDef.attrs populated for annotated types: the driver struct
    // and the pin-state marker both carry `#[rseq::...]` attributes
    // (the register-block struct is tracked under `register_blocks`,
    // not `type_defs`, so it does not count here).
    let annotated = collect_classifications_with_attrs(&model)
        .into_iter()
        .filter(|(_, attrs_nonempty)| *attrs_nonempty)
        .count();
    assert!(
        annotated >= 2,
        "at least two annotated types should have non-empty Attrs, got {annotated}"
    );

    // Function placeholder bodies (Unreachable terminator, arg_count set
    // from the signature).
    let functions = collect_functions(&model);
    assert!(!functions.is_empty(), "at least one function present");
    for f in &functions {
        assert_eq!(
            f.body.basic_blocks.len(),
            1,
            "Pass 1 placeholder body has exactly one block"
        );
    }

    // Source locations: every assigned DeclId has an entry; all Source
    // (no CompilerGenerated after a successful walk except the root
    // module fallback).
    let source_count = model
        .source_locations
        .values()
        .filter(|o| matches!(o, SourceOrigin::Source(_)))
        .count();
    assert!(source_count >= 5, "most DeclIds should have Source origin");

    // Root module type defs include the register block struct + driver
    // struct + helpers; classification should mark no Abstraction or
    // Framework types (the inline facade does not set those attrs).
    let classifications = collect_classifications(&model);
    assert!(
        classifications
            .iter()
            .any(|c| matches!(c, TypeDefClassification::UserDefined)),
        "at least one UserDefined type should exist"
    );
}

// ---------------------------------------------------------------------------
// Negative specimens
// ---------------------------------------------------------------------------

#[test]
fn negative_no_driver_struct() {
    const SRC: &str = r#"
#![no_std]
pub struct NothingHere;
pub const X: u32 = 1;
"#;
    let outcome = compile_and_build(SRC, "construction_pass1_no_driver");
    assert!(matches!(outcome, Err(ConstructionError::NoDriverStruct)));
}

#[test]
fn negative_multiple_driver_structs() {
    const SRC: &str = r#"
#![no_std]
#[rseq::driver(name = "a", compatible = ["x"], bus = "amba")]
pub struct A;
#[rseq::driver(name = "b", compatible = ["y"], bus = "amba")]
pub struct B;
"#;
    let outcome = compile_and_build(SRC, "construction_pass1_multiple_drivers");
    match outcome {
        Err(ConstructionError::MultipleDriverStructs { drivers }) => {
            assert_eq!(drivers.len(), 2, "provenance for both driver structs");
        }
        other => panic!("expected MultipleDriverStructs, got {:?}", other),
    }
}

#[test]
fn negative_invalid_user_data_shape() {
    const SRC: &str = r#"
#![no_std]
#[rseq::driver(name = "d", compatible = ["x"], bus = "amba")]
pub struct D {
    pub bad: u32,
}
"#;
    let outcome = compile_and_build(SRC, "construction_pass1_invalid_user_data");
    assert!(matches!(
        outcome,
        Err(ConstructionError::InvalidUserDataShape { .. })
    ));
}

#[test]
fn negative_user_data_not_copy() {
    const SRC: &str = r#"
#![no_std]
pub struct NotCopy { pub v: u32 }

#[rseq::driver(name = "d", compatible = ["x"], bus = "amba")]
pub struct D {
    pub bad: Option<NotCopy>,
}
"#;
    let outcome = compile_and_build(SRC, "construction_pass1_user_data_not_copy");
    assert!(matches!(
        outcome,
        Err(ConstructionError::UserDataNotCopy { .. })
    ));
}

#[test]
fn negative_missing_resource_id() {
    const SRC: &str = r#"
#![no_std]
pub struct Clock;

#[rseq::driver(name = "d", compatible = ["x"], bus = "amba")]
pub struct D {
    #[rseq::clock]
    pub clk: Clock,
}
"#;
    let outcome = compile_and_build(SRC, "construction_pass1_missing_resource_id");
    assert!(matches!(
        outcome,
        Err(ConstructionError::MissingResourceId { kind: "clock", .. })
    ));
}

// ---------------------------------------------------------------------------
// Walk helpers
// ---------------------------------------------------------------------------

fn count_driver_modules(model: &RegseqModel) -> usize {
    fn walk(m: &regseq_model::model::Module) -> usize {
        let self_count = if m.kind == ModuleKind::Driver { 1 } else { 0 };
        self_count + m.submodules.iter().map(walk).sum::<usize>()
    }
    walk(&model.root_module)
}

fn find_driver_module(model: &RegseqModel) -> Option<&regseq_model::model::Module> {
    fn walk(m: &regseq_model::model::Module) -> Option<&regseq_model::model::Module> {
        if m.kind == ModuleKind::Driver {
            return Some(m);
        }
        for sub in &m.submodules {
            if let Some(x) = walk(sub) {
                return Some(x);
            }
        }
        None
    }
    walk(&model.root_module)
}

fn count_constants(model: &RegseqModel) -> usize {
    fn walk(m: &regseq_model::model::Module) -> usize {
        m.constants.len() + m.submodules.iter().map(walk).sum::<usize>()
    }
    walk(&model.root_module)
}

fn count_register_blocks(model: &RegseqModel) -> usize {
    fn walk(m: &regseq_model::model::Module) -> usize {
        m.register_blocks.len() + m.submodules.iter().map(walk).sum::<usize>()
    }
    walk(&model.root_module)
}

fn count_register_descriptors(model: &RegseqModel) -> usize {
    fn walk(m: &regseq_model::model::Module) -> usize {
        m.register_descriptors.len() + m.submodules.iter().map(walk).sum::<usize>()
    }
    walk(&model.root_module)
}

fn collect_functions(model: &RegseqModel) -> Vec<&regseq_model::model::Function> {
    fn walk<'a>(
        m: &'a regseq_model::model::Module,
        out: &mut Vec<&'a regseq_model::model::Function>,
    ) {
        out.extend(m.free_functions.iter());
        for b in &m.impl_blocks {
            out.extend(b.methods.iter());
        }
        for b in &m.trait_impls {
            out.extend(b.methods.iter());
        }
        for b in &m.driver_impl_blocks {
            out.extend(b.methods.iter());
        }
        for sub in &m.submodules {
            walk(sub, out);
        }
    }
    let mut out = Vec::new();
    walk(&model.root_module, &mut out);
    out
}

fn collect_register_block_ids(model: &RegseqModel) -> Vec<regseq_model::identity::RegisterBlockId> {
    fn walk(
        m: &regseq_model::model::Module,
        out: &mut Vec<regseq_model::identity::RegisterBlockId>,
    ) {
        for b in &m.register_blocks {
            out.push(b.id);
        }
        for sub in &m.submodules {
            walk(sub, out);
        }
    }
    let mut out = Vec::new();
    walk(&model.root_module, &mut out);
    out
}

fn collect_reachable_decl_ids(model: &RegseqModel) -> Vec<regseq_model::identity::DeclId> {
    use regseq_model::identity::DeclId;
    fn walk(m: &regseq_model::model::Module, out: &mut Vec<DeclId>) {
        out.push(m.id.decl_id());
        for c in &m.constants {
            out.push(c.id.decl_id());
        }
        for f in &m.free_functions {
            out.push(f.id.decl_id());
        }
        for td in &m.type_defs {
            out.push(td.id.decl_id());
        }
        for rd in &m.register_descriptors {
            out.push(rd.id.decl_id());
        }
        for rb in &m.register_blocks {
            out.push(rb.id.decl_id());
        }
        if let Some(drv) = &m.driver {
            for r in &drv.clocks {
                out.push(r.id.decl_id());
            }
            for r in &drv.resets {
                out.push(r.id.decl_id());
            }
            for r in &drv.pinctrl {
                out.push(r.id.decl_id());
            }
            for r in &drv.interrupts {
                out.push(r.id.decl_id());
            }
            for r in &drv.mmio {
                out.push(r.id.decl_id());
            }
            for r in &drv.sync {
                out.push(r.id.decl_id());
            }
            for p in &drv.properties {
                out.push(p.id.decl_id());
            }
            for u in &drv.user_data {
                out.push(u.id.decl_id());
            }
        }
        for b in &m.impl_blocks {
            for meth in &b.methods {
                out.push(meth.id.decl_id());
            }
            for c in &b.associated_constants {
                out.push(c.id.decl_id());
            }
        }
        for b in &m.trait_impls {
            for meth in &b.methods {
                out.push(meth.id.decl_id());
            }
            for c in &b.associated_constants {
                out.push(c.id.decl_id());
            }
        }
        for b in &m.driver_impl_blocks {
            for meth in &b.methods {
                out.push(meth.id.decl_id());
            }
            for c in &b.associated_constants {
                out.push(c.id.decl_id());
            }
        }
        for sub in &m.submodules {
            walk(sub, out);
        }
    }
    let mut out = Vec::new();
    walk(&model.root_module, &mut out);
    out
}

fn collect_classifications_with_attrs(model: &RegseqModel) -> Vec<(TypeDefClassification, bool)> {
    fn walk(m: &regseq_model::model::Module, out: &mut Vec<(TypeDefClassification, bool)>) {
        for td in &m.type_defs {
            out.push((td.classification.clone(), !td.attrs.entries.is_empty()));
        }
        for sub in &m.submodules {
            walk(sub, out);
        }
    }
    let mut out = Vec::new();
    walk(&model.root_module, &mut out);
    out
}

fn collect_classifications(model: &RegseqModel) -> Vec<TypeDefClassification> {
    fn walk(m: &regseq_model::model::Module, out: &mut Vec<TypeDefClassification>) {
        for td in &m.type_defs {
            out.push(td.classification.clone());
        }
        for sub in &m.submodules {
            walk(sub, out);
        }
    }
    let mut out = Vec::new();
    walk(&model.root_module, &mut out);
    out
}
