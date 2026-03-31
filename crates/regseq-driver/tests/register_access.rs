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
// Common fixture preamble with local type stubs
// ---------------------------------------------------------------------------

const FIXTURE_PREAMBLE: &str = r#"
    use core::marker::PhantomData;

    pub struct Mmio<T> { _t: PhantomData<T> }
    impl<T> core::ops::Deref for Mmio<T> {
        type Target = T;
        fn deref(&self) -> &T { unimplemented!() }
    }

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

    pub struct ReadWrite<T, R> { _v: T, _r: PhantomData<R> }
    impl<T: Default, R> ReadWrite<T, R> {
        pub fn read(&self, _field: Field<T, R>) -> T { Default::default() }
        pub fn write(&self, _v: FieldValue<T, R>) {}
        pub fn modify(&self, _v: FieldValue<T, R>) {}
        pub fn set(&self, _v: FieldValue<T, R>) {}
    }

    pub struct ReadOnly<T, R> { _v: T, _r: PhantomData<R> }
    impl<T: Default, R> ReadOnly<T, R> {
        pub fn read(&self, _field: Field<T, R>) -> T { Default::default() }
    }

    pub struct FakeRegister;
    pub struct FakeThing;
    impl FakeThing {
        pub fn read(&self) -> u32 { 0 }
    }

    pub mod poll {
        pub fn until<C: FnMut() -> bool>(_c: C, _d: u64, _t: u64) -> Result<(), ()> { Ok(()) }
    }
"#;

// ---------------------------------------------------------------------------
// Test 1: Mmio<T> + register-block declaration recognized
// ---------------------------------------------------------------------------

#[test]
fn recognize_mmio_field_and_register_block() {
    let source = format!(
        r#"
        {preamble}

        #[allow(non_snake_case)]
        mod DR_REG {{ pub struct Register; }}

        #[rseq::register_block]
        pub struct Pl011Regs {{
            #[rseq::register(access_width = 32)]
            pub dr: ReadWrite<u32, DR_REG::Register>,
            #[rseq::register(access_width = 32, hw_updates = "volatile")]
            pub fr: ReadOnly<u32, DR_REG::Register>,
        }}

        pub struct Pl011 {{
            pub regs: Mmio<Pl011Regs>,
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "mmio_and_register_block").unwrap();

    // Register block recognized
    assert_eq!(surface.register_blocks.len(), 1);
    let block = &surface.register_blocks[0];
    assert_eq!(block.name, "Pl011Regs");
    assert_eq!(block.fields.len(), 2);
    assert_eq!(block.fields[0].name, "dr");
    assert_eq!(block.fields[0].access_width, Some(32));
    assert_eq!(block.fields[1].name, "fr");
    assert_eq!(block.fields[1].hw_updates, Some("volatile".to_string()));

    // MMIO field recognized
    assert_eq!(surface.mmio_fields.len(), 1);
    let mmio = &surface.mmio_fields[0];
    assert_eq!(mmio.field_name, "regs");
    assert_eq!(mmio.mmio_id, None);
    assert_eq!(mmio.register_block_type, Some("Pl011Regs".to_string()));
}

// ---------------------------------------------------------------------------
// Test 2: poll::until call sites recognized
// ---------------------------------------------------------------------------

#[test]
fn recognize_poll_until_call_sites() {
    let source = format!(
        r#"
        {preamble}

        pub struct Device;

        impl Device {{
            pub fn wait_for_ready(&self) {{
                let _ = poll::until(|| true, 10, 100);
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "poll_until").unwrap();

    assert_eq!(surface.poll_call_sites.len(), 1);
    assert_eq!(surface.poll_call_sites[0].containing_fn, "wait_for_ready");
}

// ---------------------------------------------------------------------------
// Test 3: wrapper method call sites recognized
// ---------------------------------------------------------------------------

#[test]
fn recognize_wrapper_method_call_sites() {
    let source = format!(
        r#"
        {preamble}

            #[allow(non_snake_case)]
            mod REG {{
                pub struct Register;
                pub mod VALUE {{
                    pub const FIELD: super::super::Field<u32, super::Register> =
                        super::super::Field::new(0x1, 0);
                    pub const SET: super::super::FieldValue<u32, super::Register> =
                        super::super::FieldValue::new(0x1, 0x1);
                }}
            }}

        #[rseq::register_block]
        pub struct Regs {{
            pub dr: ReadWrite<u32, REG::Register>,
        }}

        pub struct Device {{
            #[rseq::mmio(id = "regs")]
            pub regs: Mmio<Regs>,
        }}

        impl Device {{
            pub fn example(&self) {{
                let _ = self.regs.dr.read(REG::VALUE::FIELD);
                self.regs.dr.write(REG::VALUE::SET);
                self.regs.dr.modify(REG::VALUE::SET);
                self.regs.dr.set(REG::VALUE::SET);
            }}

            pub fn unrelated(fake: &FakeThing) {{
                let _ = fake.read();
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "wrapper_methods").unwrap();

    assert_eq!(surface.wrapper_call_sites.len(), 4);
    assert!(surface
        .wrapper_call_sites
        .iter()
        .any(|s| s.method == WrapperMethod::Read));
    assert!(surface
        .wrapper_call_sites
        .iter()
        .any(|s| s.method == WrapperMethod::Write));
    assert!(surface
        .wrapper_call_sites
        .iter()
        .any(|s| s.method == WrapperMethod::Modify));
    assert!(surface
        .wrapper_call_sites
        .iter()
        .any(|s| s.method == WrapperMethod::Set));
    assert!(surface
        .wrapper_call_sites
        .iter()
        .all(|s| s.containing_fn == "example"));
}

// ---------------------------------------------------------------------------
// Test 4: register descriptor facts reachable
// ---------------------------------------------------------------------------

#[test]
fn register_descriptor_facts_reachable() {
    let source = format!(
        r#"
        {preamble}

        #[allow(non_snake_case)]
            pub mod FR_REG {{
                pub struct Register;

                #[allow(non_snake_case)]
                pub mod TXFF {{
                    pub const FIELD: super::super::Field<u32, super::Register> =
                        super::super::Field::new(1 << 5, 5);
                }}

                #[allow(non_snake_case)]
                pub mod BUSY {{
                    pub const FIELD: super::super::Field<u32, super::Register> =
                        super::super::Field::new(1 << 3, 3);
                }}
            }}

            #[allow(non_snake_case)]
            pub mod LCRH_REG {{
                pub struct Register;

                #[allow(non_snake_case)]
                pub mod WLEN {{
                    pub const FIELD: super::super::Field<u32, super::Register> =
                        super::super::Field::new(0b11 << 5, 5);
                    pub const WLEN_5: super::super::FieldValue<u32, super::Register> =
                        super::super::FieldValue::new(0b11 << 5, 0 << 5);
                    pub const WLEN_8: super::super::FieldValue<u32, super::Register> =
                        super::super::FieldValue::new(0b11 << 5, 3 << 5);
                }}
            }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "register_descriptors").unwrap();

    assert_eq!(surface.register_descriptors.len(), 2);

    let fr = surface
        .register_descriptors
        .iter()
        .find(|d| d.register_module == "FR_REG")
        .unwrap();
    let txff = fr.fields.iter().find(|f| f.field_name == "TXFF").unwrap();
    assert_eq!(txff.offset, 5);
    assert_eq!(txff.width, 1);

    let busy = fr.fields.iter().find(|f| f.field_name == "BUSY").unwrap();
    assert_eq!(busy.offset, 3);
    assert_eq!(busy.width, 1);

    let lcrh = surface
        .register_descriptors
        .iter()
        .find(|d| d.register_module == "LCRH_REG")
        .unwrap();
    let wlen = lcrh.fields.iter().find(|f| f.field_name == "WLEN").unwrap();
    assert_eq!(wlen.offset, 5);
    assert_eq!(wlen.width, 2);
    assert!(wlen.named_values.iter().any(|v| v.name == "WLEN_5" && v.value == 0));
    assert!(wlen.named_values.iter().any(|v| v.name == "WLEN_8" && v.value == 3));
}

// ---------------------------------------------------------------------------
// Test 5: malformed declarations rejected
// ---------------------------------------------------------------------------

#[test]
fn reject_missing_mmio_id() {
    let source = format!(
        r#"
        {preamble}

        pub struct Regs {{ pub dr: u32 }}

        pub struct Device {{
            #[rseq::mmio(bar = 0)]
            pub regs: Mmio<Regs>,
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let result = compile_and_extract(&source, "missing_mmio_id");
    assert_eq!(result, Err(RegisterAccessError::MissingMmioId));
}

#[test]
fn mmio_attr_id_is_optional_for_typed_field_recognition() {
    let source = format!(
        r#"
        {preamble}

        pub struct Regs {{ pub dr: u32 }}

        pub struct Device {{
            pub regs: Mmio<Regs>,
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "typed_mmio_without_attr").unwrap();
    assert_eq!(surface.mmio_fields.len(), 1);
    assert_eq!(surface.mmio_fields[0].field_name, "regs");
    assert_eq!(surface.mmio_fields[0].mmio_id, None);
    assert_eq!(
        surface.mmio_fields[0].register_block_type,
        Some("Regs".to_string())
    );
}

#[test]
fn reject_malformed_register_attribute() {
    let source = format!(
        r#"
        {preamble}

        #[rseq::register_block]
        pub struct Regs {{
            #[rseq::register("not-key-value")]
            pub dr: u32,
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let result = compile_and_extract(&source, "malformed_register");
    assert!(matches!(
        result,
        Err(RegisterAccessError::MalformedAttribute(_))
    ));
}
