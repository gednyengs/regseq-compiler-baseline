#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::sync::{Arc, Mutex, Once};

#[path = "../src/extract/ordering.rs"]
mod ordering;

use ordering::*;

type ExtractionResult = Result<OrderingSurface, OrderingError>;

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
            *self.result.lock().unwrap() = Some(extract_ordering_surface(tcx));
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

const FIXTURE_PREAMBLE: &str = r#"
    use core::marker::PhantomData;

    pub mod intrinsics {
        pub mod sync {
            #[derive(Debug, Clone, Copy)]
            pub enum BarrierKind {
                Full,
                WriteWrite,
            }

            pub fn memory_barrier(_kind: BarrierKind) {}
        }
    }

    pub struct Field<T, R> { pub mask: T, pub shift: T, _r: PhantomData<R> }
    pub struct FieldValue<T, R> { pub mask: T, pub value: T, _r: PhantomData<R> }

    impl<T, R> Field<T, R> {
        pub const fn new(mask: T, shift: T) -> Self {
            Self { mask, shift, _r: PhantomData }
        }
    }

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
    }

    pub struct Mmio<T> { _t: PhantomData<T> }
    impl<T> core::ops::Deref for Mmio<T> {
        type Target = T;
        fn deref(&self) -> &T { unimplemented!() }
    }

    pub struct FakeRegister;

    pub mod poll {
        pub fn until<C: FnMut() -> bool>(_c: C, _d: u64, _t: u64) -> Result<(), ()> { Ok(()) }
    }
"#;

// ---------------------------------------------------------------------------
// Test 1: Recognize ordered region from expanded ordered! pattern
// ---------------------------------------------------------------------------

#[test]
fn recognize_ordered_region() {
    let source = format!(
        r#"
        {preamble}

        pub struct Regs {{
            pub cr: ReadWrite<u32, FakeRegister>,
        }}

        pub struct Device {{
            pub regs: Mmio<Regs>,
        }}

        impl Device {{
            pub fn ordered_write(&self) {{
                {{
                    intrinsics::sync::memory_barrier(intrinsics::sync::BarrierKind::Full);
                    let __regseq_ordered_result = {{
                        self.regs.cr.write(FieldValue::new(0xFF_u32, 0x42));
                    }};
                    intrinsics::sync::memory_barrier(intrinsics::sync::BarrierKind::Full);
                    __regseq_ordered_result
                }}
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "ordered_region").unwrap();

    assert_eq!(surface.ordered_regions.len(), 1);
    let region = &surface.ordered_regions[0];
    assert_eq!(region.containing_fn, "ordered_write");
}

// ---------------------------------------------------------------------------
// Test 2: Canonical barrier anchors recoverable
// ---------------------------------------------------------------------------

#[test]
fn barrier_anchors_recoverable() {
    let source = format!(
        r#"
        {preamble}

        pub struct Regs {{
            pub cr: ReadWrite<u32, FakeRegister>,
        }}

        pub struct Device {{
            pub regs: Mmio<Regs>,
        }}

        impl Device {{
            pub fn ordered_op(&self) {{
                {{
                    intrinsics::sync::memory_barrier(intrinsics::sync::BarrierKind::Full);
                    let __regseq_ordered_result = {{
                        self.regs.cr.write(FieldValue::new(0xFF_u32, 0x42));
                    }};
                    intrinsics::sync::memory_barrier(intrinsics::sync::BarrierKind::WriteWrite);
                    __regseq_ordered_result
                }}
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "barrier_anchors").unwrap();

    assert_eq!(surface.ordered_regions.len(), 1);
    let region = &surface.ordered_regions[0];
    assert_eq!(region.pre_barrier_kind, "Full");
    assert_eq!(region.post_barrier_kind, "WriteWrite");
}

// ---------------------------------------------------------------------------
// Test 3: Enclosed operations associated with ordered region
// ---------------------------------------------------------------------------

#[test]
fn enclosed_operations_associated() {
    let source = format!(
        r#"
        {preamble}

        pub struct Regs {{
            pub cr: ReadWrite<u32, FakeRegister>,
            pub sr: ReadWrite<u32, FakeRegister>,
        }}

        pub struct Device {{
            pub regs: Mmio<Regs>,
        }}

        impl Device {{
            pub fn ordered_sequence(&self) {{
                {{
                    intrinsics::sync::memory_barrier(intrinsics::sync::BarrierKind::Full);
                    let __regseq_ordered_result = {{
                        self.regs.cr.write(FieldValue::new(0xFF_u32, 0x42));
                        self.regs.sr.modify(FieldValue::new(0xFF_u32, 0x01));
                        let _ = self.regs.cr.read(Field::new(0xFF_u32, 0_u32));
                    }};
                    intrinsics::sync::memory_barrier(intrinsics::sync::BarrierKind::Full);
                    __regseq_ordered_result
                }}
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "enclosed_ops").unwrap();

    assert_eq!(surface.ordered_regions.len(), 1);
    let region = &surface.ordered_regions[0];
    assert_eq!(region.enclosed_wrapper_calls.len(), 3);
    assert!(region.enclosed_wrapper_calls.contains(&"write".to_string()));
    assert!(region
        .enclosed_wrapper_calls
        .contains(&"modify".to_string()));
    assert!(region.enclosed_wrapper_calls.contains(&"read".to_string()));
}

// ---------------------------------------------------------------------------
// Test 4: Malformed ordered pattern rejected
// ---------------------------------------------------------------------------

#[test]
fn reject_unpaired_barrier() {
    let source = format!(
        r#"
        {preamble}

        pub struct Regs {{
            pub cr: ReadWrite<u32, FakeRegister>,
        }}

        pub struct Device {{
            pub regs: Mmio<Regs>,
        }}

        impl Device {{
            pub fn bad_ordered(&self) {{
                {{
                    intrinsics::sync::memory_barrier(intrinsics::sync::BarrierKind::Full);
                    self.regs.cr.write(FieldValue::new(0xFF_u32, 0x42));
                }}
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let result = compile_and_extract(&source, "unpaired_barrier");
    assert_eq!(
        result,
        Err(OrderingError::UnpairedBarrier("bad_ordered".into()))
    );
}
