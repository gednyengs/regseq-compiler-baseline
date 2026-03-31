#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::collections::HashSet;
use std::sync::{Arc, Mutex, Once};

#[path = "../src/extract/synchronization.rs"]
mod synchronization;

use synchronization::*;

type ExtractionResult = Result<SynchronizationSurface, SynchronizationError>;

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

fn compile_and_extract(source: &str, test_name: &str, isr_names: &[&str]) -> ExtractionResult {
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
    let isr_set: HashSet<String> = isr_names.iter().map(|s| s.to_string()).collect();

    struct ExtractCallbacks {
        result: Arc<Mutex<Option<ExtractionResult>>>,
        isr_handler_names: HashSet<String>,
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
            *self.result.lock().unwrap() = Some(extract_synchronization_surface(
                tcx,
                &self.isr_handler_names,
            ));
            rustc_driver::Compilation::Stop
        }
    }

    let mut callbacks = ExtractCallbacks {
        result: result.clone(),
        isr_handler_names: isr_set,
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
    use core::ops::{Deref, DerefMut};

    pub struct Mutex<T> { _t: PhantomData<T> }
    pub struct SpinLock<T> { _t: PhantomData<T> }
    pub struct IrqSpinLock<T> { _t: PhantomData<T> }
    pub struct Completion { _p: () }

    pub struct Guard<'a, T> { _m: PhantomData<&'a mut T> }
    impl<T> Deref for Guard<'_, T> {
        type Target = T;
        fn deref(&self) -> &T { unimplemented!() }
    }
    impl<T> DerefMut for Guard<'_, T> {
        fn deref_mut(&mut self) -> &mut T { unimplemented!() }
    }

    impl<T> Mutex<T> {
        pub fn lock(&self) -> Guard<'_, T> { unimplemented!() }
    }
    impl<T> SpinLock<T> {
        pub fn lock(&self) -> Guard<'_, T> { unimplemented!() }
    }
    impl<T> IrqSpinLock<T> {
        pub fn lock(&self) -> Guard<'_, T> { unimplemented!() }
    }

    pub struct Duration { us: u64 }
    impl Duration {
        pub const fn from_ms(ms: u64) -> Self { Self { us: ms * 1_000 } }
    }

    #[derive(Debug)]
    pub enum RegseqError { Timeout }

    impl Completion {
        pub fn wait(&self, _timeout: Duration) -> Result<(), RegseqError> { unimplemented!() }
        pub fn signal(&self) { unimplemented!() }
        pub fn reset(&self) { unimplemented!() }
    }

    pub struct Interrupt;
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum IrqResult { Handled, NotHandled, DeferWork }
"#;

// ---------------------------------------------------------------------------
// Test 1: Recognize synchronization field types and payload types
// ---------------------------------------------------------------------------

#[test]
fn recognize_sync_field_types_and_payloads() {
    let source = format!(
        r#"
        {preamble}

        pub struct CallbackState {{
            pub counter: u32,
        }}

        pub struct Device {{
            pub mtx: Mutex<CallbackState>,
            pub spin: SpinLock<u32>,
            pub irq_spin: IrqSpinLock<CallbackState>,
            pub done: Completion,
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "sync_fields", &[]).unwrap();

    assert_eq!(surface.sync_fields.len(), 4);

    let mtx = surface
        .sync_fields
        .iter()
        .find(|f| f.field_name == "mtx")
        .unwrap();
    assert_eq!(mtx.kind, SyncFieldKind::Mutex);
    assert_eq!(mtx.payload_type, Some("CallbackState".into()));

    let spin = surface
        .sync_fields
        .iter()
        .find(|f| f.field_name == "spin")
        .unwrap();
    assert_eq!(spin.kind, SyncFieldKind::SpinLock);

    let irq_spin = surface
        .sync_fields
        .iter()
        .find(|f| f.field_name == "irq_spin")
        .unwrap();
    assert_eq!(irq_spin.kind, SyncFieldKind::IrqSpinLock);
    assert_eq!(irq_spin.payload_type, Some("CallbackState".into()));

    let done = surface
        .sync_fields
        .iter()
        .find(|f| f.field_name == "done")
        .unwrap();
    assert_eq!(done.kind, SyncFieldKind::Completion);
    assert_eq!(done.payload_type, None);
}

// ---------------------------------------------------------------------------
// Test 2: Recognize wrapper-method call sites
// ---------------------------------------------------------------------------

#[test]
fn recognize_sync_call_sites() {
    let source = format!(
        r#"
        {preamble}

        pub struct State {{ pub val: u32 }}

        pub struct Device {{
            pub mtx: Mutex<State>,
            pub done: Completion,
        }}

        impl Device {{
            pub fn do_work(&self) {{
                let _guard = self.mtx.lock();
                self.done.signal();
            }}

            pub fn wait_done(&self) {{
                let _ = self.done.wait(Duration::from_ms(100));
            }}

            pub fn restart(&self) {{
                self.done.reset();
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "sync_call_sites", &[]).unwrap();

    assert_eq!(surface.lock_operations.len(), 1);
    assert_eq!(surface.lock_operations[0].lock_kind, SyncFieldKind::Mutex);
    assert_eq!(surface.lock_operations[0].containing_fn, "do_work");

    assert_eq!(surface.completion_operations.len(), 3);
    assert!(surface
        .completion_operations
        .iter()
        .any(|o| o.method == CompletionMethod::Signal && o.containing_fn == "do_work"));
    assert!(surface
        .completion_operations
        .iter()
        .any(|o| o.method == CompletionMethod::Wait && o.containing_fn == "wait_done"));
    assert!(surface
        .completion_operations
        .iter()
        .any(|o| o.method == CompletionMethod::Reset && o.containing_fn == "restart"));
}

// ---------------------------------------------------------------------------
// Test 3: Reject Mutex/SpinLock lock in ISR context
// ---------------------------------------------------------------------------

#[test]
fn reject_mutex_lock_in_isr() {
    let source = format!(
        r#"
        {preamble}

        pub struct State {{ pub val: u32 }}

        pub struct Device {{
            #[rseq::interrupt(id = "irq")]
            pub irq: Interrupt,
            pub mtx: Mutex<State>,
        }}

        impl Device {{
            #[rseq::isr]
            pub fn handle_irq(&mut self) -> IrqResult {{
                let _guard = self.mtx.lock();
                IrqResult::Handled
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let result = compile_and_extract(&source, "mutex_in_isr", &["handle_irq"]);
    assert_eq!(
        result,
        Err(SynchronizationError::NonIsrSafeLockInIsr(
            "handle_irq".into()
        ))
    );
}

#[test]
fn reject_spinlock_lock_in_isr() {
    let source = format!(
        r#"
        {preamble}

        pub struct State {{ pub val: u32 }}

        pub struct Device {{
            #[rseq::interrupt(id = "irq")]
            pub irq: Interrupt,
            pub spin: SpinLock<State>,
        }}

        impl Device {{
            #[rseq::isr]
            pub fn handle_irq(&mut self) -> IrqResult {{
                let _guard = self.spin.lock();
                IrqResult::Handled
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let result = compile_and_extract(&source, "spinlock_in_isr", &["handle_irq"]);
    assert_eq!(
        result,
        Err(SynchronizationError::NonIsrSafeLockInIsr(
            "handle_irq".into()
        ))
    );
}

// ---------------------------------------------------------------------------
// Test 4: Reject Completion::wait in ISR context
// ---------------------------------------------------------------------------

#[test]
fn reject_completion_wait_in_isr() {
    let source = format!(
        r#"
        {preamble}

        pub struct Device {{
            #[rseq::interrupt(id = "irq")]
            pub irq: Interrupt,
            pub done: Completion,
        }}

        impl Device {{
            #[rseq::isr]
            pub fn handle_irq(&mut self) -> IrqResult {{
                let _ = self.done.wait(Duration::from_ms(100));
                IrqResult::Handled
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let result = compile_and_extract(&source, "wait_in_isr", &["handle_irq"]);
    assert_eq!(
        result,
        Err(SynchronizationError::CompletionWaitInIsr(
            "handle_irq".into()
        ))
    );
}

// ---------------------------------------------------------------------------
// Test 5: Guard-scope metadata available for later analysis
// ---------------------------------------------------------------------------

#[test]
fn guard_scope_metadata_available() {
    let source = format!(
        r#"
        {preamble}

        pub struct State {{ pub val: u32 }}

        pub struct Device {{
            pub irq_lock: IrqSpinLock<State>,
            pub mtx: Mutex<State>,
        }}

        impl Device {{
            pub fn thread_work(&self) {{
                let _g = self.mtx.lock();
            }}

            pub fn isr_work(&self) {{
                let _g = self.irq_lock.lock();
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "guard_scopes", &[]).unwrap();

    assert_eq!(surface.guard_scopes.len(), 2);
    assert!(surface
        .guard_scopes
        .iter()
        .any(|g| g.lock_kind == SyncFieldKind::Mutex && g.containing_fn == "thread_work"));
    assert!(surface
        .guard_scopes
        .iter()
        .any(|g| g.lock_kind == SyncFieldKind::IrqSpinLock && g.containing_fn == "isr_work"));
}

// IrqSpinLock in ISR is accepted
#[test]
fn irqspinlock_in_isr_accepted() {
    let source = format!(
        r#"
        {preamble}

        pub struct State {{ pub val: u32 }}

        pub struct Device {{
            #[rseq::interrupt(id = "irq")]
            pub irq: Interrupt,
            pub irq_lock: IrqSpinLock<State>,
        }}

        impl Device {{
            #[rseq::isr]
            pub fn handle_irq(&mut self) -> IrqResult {{
                let _guard = self.irq_lock.lock();
                IrqResult::Handled
            }}
        }}
    "#,
        preamble = FIXTURE_PREAMBLE
    );

    let surface = compile_and_extract(&source, "irqspinlock_in_isr", &["handle_irq"]).unwrap();
    assert_eq!(surface.lock_operations.len(), 1);
    assert_eq!(
        surface.lock_operations[0].lock_kind,
        SyncFieldKind::IrqSpinLock
    );
}
