//! Bridge integration for dma-management coherent-region recognition (M1 / T06).
//!
//! A `#[rseq::driver]` struct with a `DmaRegion<T>` field must bridge (the region
//! field is routed away from the device-lifecycle user-data classifier), and the
//! recognized C09 facts must appear in the model: a `DmaRegionFacts` in
//! `dma.drivers` (id + element_size + element_count + captured element type),
//! plus an `AttrData::DmaRegionResource` field marker. A `region.write(i, |v| …)`
//! closure call must produce an `ElementAccessFact` (verb Write + index + lexical
//! extent). A `#[rseq::dma_region]` missing a mandatory `element_size`/
//! `element_count` key must be REJECTED. A bus-master (PCI) region driver must set
//! `dma_present = true`; a non-PCI one must not.
//!
//! CLOSURE-BORROW NOTE (T13, RESOLVED): the canonical D01 element-access surface
//! passes a borrow closure (`region.write(i, |&mut V| …)`). Its monomorphized
//! `Fn::call` shim signature `for<'a> extern "rust-call" fn((&'a mut V,))` is the
//! closure's benign universal-lifetime self-signature, which the general
//! substantive-HRTB guard (spec §17.1) flags because the borrow rides a tuple
//! slot. T13 adds a NARROW carve-out — scoped to a recognized `dma_element_*`
//! call AND matched ONLY to the closure-reified-sig SHAPE (RustCall + single
//! tuple input) — so the REAL borrow surface bridges and the closure body's field
//! stores survive into the IR (the backend inlines them). The global HRTB guard
//! is UNCHANGED for all non-DMA code (`non_dma_borrow_closure_is_still_rejected`).
//! The lexical extent is the call-site terminator span (a PLACEHOLDER-FLAGGED
//! stand-in for the precise closure entry/exit, R-closure-extent).

#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, Once};
use std::time::{SystemTime, UNIX_EPOCH};

use regseq_driver::bridge::{run_bridge, BridgeInput};
use regseq_model::{AttrData, DmaFlow, ElementVerb, RegseqModel};
use rustc_middle::ty::TyCtxt;

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
    let output = Command::new("rustc")
        .arg("--print")
        .arg("sysroot")
        .output()
        .expect("failed to run rustc --print sysroot");
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn fresh_test_dir(crate_name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("regseq-bridge-dma-{crate_name}-{nanos}"))
}

/// Run the full bridge over `source`; on success apply `f` to the model, on
/// rejection return the rendered `BridgeError`.
fn with_model<R, F>(crate_name: &str, source: &str, f: F) -> Result<R, String>
where
    R: Send + 'static,
    F: FnOnce(&RegseqModel) -> R + Send + 'static,
{
    let _lock = COMPILER_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    init_rustc();

    let test_dir = fresh_test_dir(crate_name);
    std::fs::create_dir_all(&test_dir).unwrap();
    let src_path = test_dir.join("lib.rs");
    std::fs::write(&src_path, source).unwrap();

    let result: Arc<Mutex<Option<Result<R, String>>>> = Arc::new(Mutex::new(None));
    let callback = Arc::new(Mutex::new(Some(
        Box::new(f) as Box<dyn FnOnce(&RegseqModel) -> R + Send>
    )));

    struct Callbacks<R: Send + 'static> {
        result: Arc<Mutex<Option<Result<R, String>>>>,
        callback: Arc<Mutex<Option<Box<dyn FnOnce(&RegseqModel) -> R + Send>>>>,
    }

    impl<R: Send + 'static> rustc_driver::Callbacks for Callbacks<R> {
        fn config(&mut self, config: &mut rustc_interface::interface::Config) {
            config.opts.unstable_opts.no_codegen = true;
            config.opts.unstable_opts.mir_opt_level = Some(1);
            config
                .opts
                .unstable_opts
                .crate_attr
                .push("allow(unused, non_snake_case)".to_string());
        }

        fn after_analysis<'tcx>(
            &mut self,
            _compiler: &rustc_interface::interface::Compiler,
            tcx: TyCtxt<'tcx>,
        ) -> rustc_driver::Compilation {
            let outcome = match run_bridge(BridgeInput::new(tcx)) {
                Ok(model) => {
                    let f = self.callback.lock().unwrap().take().expect("callback once");
                    Ok(f(&model))
                }
                Err(err) => Err(err.to_string()),
            };
            *self.result.lock().unwrap() = Some(outcome);
            rustc_driver::Compilation::Stop
        }
    }

    let sysroot = get_sysroot();
    let args = vec![
        "rustc".to_string(),
        src_path.to_str().unwrap().to_string(),
        "--sysroot".to_string(),
        sysroot,
        "--edition".to_string(),
        "2021".to_string(),
        "--crate-type".to_string(),
        "rlib".to_string(),
        "--crate-name".to_string(),
        crate_name.to_string(),
        "-C".to_string(),
        "panic=abort".to_string(),
        "--out-dir".to_string(),
        test_dir.to_str().unwrap().to_string(),
    ];

    let mut callbacks = Callbacks {
        result: result.clone(),
        callback,
    };
    rustc_driver::run_compiler(&args, &mut callbacks);
    let output = result
        .lock()
        .unwrap()
        .take()
        .expect("after_analysis should produce a result");
    std::fs::remove_dir_all(Path::new(&test_dir)).ok();
    output
}

const LIFECYCLE_SURFACE: &str =
    include_str!("../../codegen-coverage-runner/fixtures/_lifecycle_surface.rs");

/// A minimal coherent `DmaRegion<T>` author surface declared inline (the CR-2
/// discipline): the typed shell + the element-access closure verbs + the A2
/// addressing ops, each tagged with its namespaced `#[rseq::intrinsic("dma_*")]`
/// facade. `DmaRegion` is detected by type NAME (the `Mmio`/`Clock` anchor). An
/// E1000-shaped `LegacyTxDesc` descriptor is the element type `T`.
const DMA_SURFACE: &str = r#"
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LegacyTxDesc {
    pub buffer_addr: u64,
    pub length: u16,
    pub cso: u8,
    pub cmd: u8,
    pub status: u8,
    pub css: u8,
    pub special: u16,
}

pub struct DmaRegion<T> {
    _t: core::marker::PhantomData<T>,
}
impl<T> DmaRegion<T> {
    pub const fn new() -> Self {
        Self { _t: core::marker::PhantomData }
    }
    // The canonical D01 surface passes a borrow closure (`FnOnce(&[mut] V)`):
    // `read` hands the closure `&V`, `write`/`update` hand it `&mut V`. Its
    // `for<'a> fn((&'a [mut] V,))` rust-call reified signature is permitted by
    // the narrow, DMA-element-call-scoped closure-reified-sig carve-out (T13);
    // the global substantive-HRTB guard is unchanged. The view type `V` rides
    // the method turbofish.
    #[inline(never)]
    #[rseq::intrinsic("dma_element_read")]
    pub fn read<V, R>(&self, _i: usize, _f: impl FnOnce(&V) -> R) -> R {
        loop {}
    }
    #[inline(never)]
    #[rseq::intrinsic("dma_element_write")]
    pub fn write<V>(&self, _i: usize, _f: impl FnOnce(&mut V)) {}
    #[inline(never)]
    #[rseq::intrinsic("dma_element_update")]
    pub fn update<V>(&self, _i: usize, _f: impl FnOnce(&mut V)) {}
    #[inline(never)]
    #[rseq::intrinsic("dma_device_addr")]
    pub fn device_addr(&self) -> u64 {
        loop {}
    }
    #[inline(never)]
    #[rseq::intrinsic("dma_device_addr_of")]
    pub fn device_addr_of(&self, _i: usize) -> u64 {
        loop {}
    }
    #[inline(never)]
    #[rseq::intrinsic("dma_element_count")]
    pub fn element_count(&self) -> usize {
        loop {}
    }
    /// `region.span(range) -> DmaRegionSpan` (M3/T09): the non-borrowing,
    /// direction-neutral element-indexed span endpoint (GRILL D05/C06).
    #[inline(never)]
    #[rseq::intrinsic("dma_region_span")]
    pub fn span(&self, _range: core::ops::Range<usize>) -> DmaRegionSpan {
        loop {}
    }
}

// The span endpoint type (M3/T09) + the `TransferEndpoint` overload trait (C06):
// `stage_transfer` accepts `&DmaStreamBuffer | DmaRegionSpan`.
pub struct DmaRegionSpan {
    _addr: u64,
    _len: u64,
}
pub trait TransferEndpoint {}
impl TransferEndpoint for DmaRegionSpan {}
impl TransferEndpoint for &DmaStreamBuffer {}

// ---------------------------------------------------------------------------
// Slave `DmaChannel` surface (M2/T07): the 5-op transport. The fallible ops
// (`stage_transfer`/`start_transfer`/`remaining_bytes`) return `Result` and are
// `#[must_use]` (the bare-statement `op();` discard then trips rustc's
// `unused_must_use`; the explicit `let _ = …` discard is caught by the bridge
// fallibility lint, T17). `configure` is parameterless; `stop` is infallible.
// Each op is tagged with its namespaced `#[rseq::intrinsic("dma_*")]` facade.
// ---------------------------------------------------------------------------
pub struct DmaStreamBuffer {
    _addr: u64,
}
impl DmaStreamBuffer {
    #[inline(never)]
    #[rseq::intrinsic("dma_device_addr")]
    pub fn device_addr(&self) -> u64 {
        loop {}
    }
    #[inline(never)]
    #[rseq::intrinsic("dma_acquire_from_device")]
    pub fn acquire_from_device(&self) {}
    #[inline(never)]
    #[rseq::intrinsic("dma_release_to_device")]
    pub fn release_to_device(&self) {}
}

pub mod dma {
    use super::{DmaStreamBuffer, RegseqError};
    /// `dma::stream(buf, flow) -> Result<DmaStreamBuffer>` — streaming-map a CPU
    /// buffer (encodes `dma_mapping_error`). `#[must_use]` Result + the bridge
    /// fallibility lint catch a discarded mapping.
    #[inline(never)]
    #[must_use]
    #[rseq::intrinsic("dma_stream")]
    pub fn stream(_buf: *mut u8, _flow: u32) -> Result<DmaStreamBuffer, RegseqError> {
        loop {}
    }
}

pub struct DmaChannel {
    _id: u32,
}
impl DmaChannel {
    pub const fn new() -> Self {
        Self { _id: 0 }
    }
    /// Parameterless channel configuration (config is declaration/L2-derived).
    #[inline(never)]
    #[rseq::intrinsic("dma_configure")]
    pub fn configure(&self) {}
    /// Stage a transfer of an endpoint (`&DmaStreamBuffer | DmaRegionSpan` via
    /// the `TransferEndpoint` overload, C06). Fallible.
    #[inline(never)]
    #[must_use]
    #[rseq::intrinsic("dma_stage_transfer")]
    pub fn stage_transfer(&self, _endpoint: impl TransferEndpoint) -> Result<(), RegseqError> {
        loop {}
    }
    /// Issue/submit a staged transfer. Fallible (`dma_submit_error => Err`).
    #[inline(never)]
    #[must_use]
    #[rseq::intrinsic("dma_start_transfer")]
    pub fn start_transfer(&self) -> Result<(), RegseqError> {
        loop {}
    }
    /// Terminate a channel's transfer. Infallible.
    #[inline(never)]
    #[rseq::intrinsic("dma_stop")]
    pub fn stop(&self) {}
    /// Query the residue. Fallible (no-meaningful-residue/`-ENOSYS => Err`).
    #[inline(never)]
    #[must_use]
    #[rseq::intrinsic("dma_remaining_bytes")]
    pub fn remaining_bytes(&self) -> Result<usize, RegseqError> {
        loop {}
    }
}

// The completion payload type (M3/T09(b), GRILL C05): the registration marker the
// handler receives. Never constructed/delivered this cycle (D06).
pub struct DmaCompletion {
    pub status: u32,
}

// ---------------------------------------------------------------------------
// M4 / T19 layout-verify view types: a set of reinterpret view types `V` for the
// C03 `region.<verb>::<V>(i, …)` layout verify negative tests.
// ---------------------------------------------------------------------------

// A NON-DmaPod view: a `#[repr(C)]` struct that carries a RAW POINTER field. A
// host pointer is never a device-visible descriptor field, so this view fails the
// `V: DmaPod` clause (T19) even though it is repr(C) and Drop-free.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PtrView {
    pub p: *const u8,
}

// A NON-repr(C) view: a plain `#[repr(Rust)]` struct whose field order/padding the
// ABI does not pin — fails the `V: DmaPod` clause (unpinned aggregate layout).
#[derive(Clone, Copy)]
pub struct RustView {
    pub a: u32,
    pub b: u32,
}

// An OVERSIZED view: a `#[repr(C)]` 32-byte descriptor. Used as `write::<BigView>`
// against a 16-byte `element_size` region to trip `size_of::<V>() <= element_size`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct BigView {
    pub a: u64,
    pub b: u64,
    pub c: u64,
    pub d: u64,
}
"#;

/// A region driver whose `Lifecycle::on_init` body runs `init_body`, on the given
/// `bus` (e.g. `pci`) with the given declared `fields`.
fn region_source(bus: &str, fields: &str, init_body: &str) -> String {
    format!(
        "#![feature(register_tool)]\n#![register_tool(rseq)]\n\
         {LIFECYCLE_SURFACE}\n{DMA_SURFACE}\n\
         #[rseq::driver(name = \"d\", compatible = [\"acme,d\"], bus = \"{bus}\")]\n\
         pub struct D {{ {fields} }}\n\
         impl Lifecycle for D {{\n    fn on_init(&mut self) -> Result<(), RegseqError> {{\n        {init_body}\n        Ok(())\n    }}\n}}\n\
         pub fn make() -> D {{ loop {{}} }}\npub fn entry() {{}}\n"
    )
}

const E1000_REGION_FIELD: &str =
    "#[rseq::dma_region(id = \"tx_ring\", element_size = 16, element_count = 256)] \
     pub tx_ring: DmaRegion<LegacyTxDesc>";

/// A PL011-style slave `DmaChannel` field (device_to_host RX). M2/T07.
const PL011_CHANNEL_FIELD: &str =
    "#[rseq::dma_channel(id = \"rx\", direction = \"device_to_host\")] pub rx: DmaChannel";

#[test]
fn dma_region_field_recognized_with_c09_facts() {
    let (n_drivers, id, element_size, element_count, has_element_type, has_marker) = with_model(
        "dma_region_field",
        &region_source(
            "pci",
            E1000_REGION_FIELD,
            "let _a = self.tx_ring.device_addr();",
        ),
        |model| {
            let n = model.dma.drivers.len();
            let driver = model.dma.drivers.values().next().expect("one dma driver");
            let region = &driver.regions[0];
            let id = model.strings.resolve(region.id).to_string();
            let has_element_type = region
                .element_type
                .map(|t| model.strings.resolve(t).contains("LegacyTxDesc"))
                .unwrap_or(false);
            let has_marker = model.domain_attrs.field_attrs.values().any(|attrs| {
                attrs
                    .iter()
                    .any(|a| matches!(a, AttrData::DmaRegionResource { .. }))
            });
            (
                n,
                id,
                region.element_size,
                region.element_count,
                has_element_type,
                has_marker,
            )
        },
    )
    .expect("a driver with a DmaRegion field must bridge");

    assert_eq!(n_drivers, 1, "exactly one driver has a DMA surface");
    assert_eq!(id, "tx_ring", "the region id is the join key");
    assert_eq!(element_size, 16, "element_size from the attribute");
    assert_eq!(element_count, 256, "element_count from the attribute");
    assert!(has_element_type, "the element type T (LegacyTxDesc) is captured");
    assert!(has_marker, "field_attrs must carry the DmaRegionResource marker");
}

#[test]
fn element_write_closure_recognized_as_access_fact() {
    let (n_accesses, verb, index, view_is_desc, extent_nonempty) = with_model(
        "dma_element_write",
        &region_source(
            "pci",
            E1000_REGION_FIELD,
            "self.tx_ring.write::<LegacyTxDesc>(0, |d: &mut LegacyTxDesc| { d.buffer_addr = 0x2000; });",
        ),
        |model| {
            let driver = model.dma.drivers.values().next().expect("one dma driver");
            let region = &driver.regions[0];
            let fact = region
                .element_accesses
                .first()
                .expect("one element_access fact recognized");
            let view_is_desc = fact
                .view_type
                .map(|v| model.strings.resolve(v).contains("LegacyTxDesc"))
                .unwrap_or(false);
            let extent_nonempty = fact.lexical_extent.span.hi >= fact.lexical_extent.span.lo;
            (
                region.element_accesses.len(),
                fact.verb,
                fact.index,
                view_is_desc,
                extent_nonempty,
            )
        },
    )
    .expect("a region.write(i, |..|) closure call must bridge");

    assert_eq!(n_accesses, 1, "exactly one element_access fact");
    assert_eq!(verb, ElementVerb::Write, "the verb is write (publishes CPU data)");
    assert_eq!(index, Some(0), "the constant index 0 is captured");
    assert!(view_is_desc, "the view type V (::<LegacyTxDesc> turbofish) is captured");
    assert!(extent_nonempty, "the lexical extent (call-site span) is recorded");
}

#[test]
fn device_addr_site_does_not_create_an_access_fact() {
    // The A2 addressing op `device_addr()` lowers from the resource directly; it
    // carries no closure borrow, so it appends NO element_access fact (S04).
    let n_accesses = with_model(
        "dma_device_addr",
        &region_source(
            "pci",
            E1000_REGION_FIELD,
            "let _a = self.tx_ring.device_addr();",
        ),
        |model| {
            model
                .dma
                .drivers
                .values()
                .next()
                .map(|d| d.regions[0].element_accesses.len())
                .unwrap_or(99)
        },
    )
    .expect("a device_addr() call must bridge");
    assert_eq!(n_accesses, 0, "an A2 device_addr site is not an element_access");
}

#[test]
fn pci_region_driver_sets_dma_present() {
    // T05: a bus-master (PCI) driver declaring a coherent region is a DMA master.
    let present = with_model(
        "dma_present_pci",
        &region_source("pci", E1000_REGION_FIELD, ""),
        |model| {
            model
                .device_lifecycle
                .drivers
                .values()
                .next()
                .map(|d| d.descriptor.dma_present)
                .unwrap_or(false)
        },
    )
    .expect("a PCI region driver must bridge");
    assert!(present, "a PCI coherent-region driver sets dma_present = true");
}

#[test]
fn platform_region_driver_does_not_set_dma_present() {
    // T05 narrowness: a non-PCI (platform/slave) region driver must NOT set the
    // bus-master bit.
    let present = with_model(
        "dma_present_platform",
        &region_source("platform", E1000_REGION_FIELD, ""),
        |model| {
            model
                .device_lifecycle
                .drivers
                .values()
                .next()
                .map(|d| d.descriptor.dma_present)
                .unwrap_or(true)
        },
    )
    .expect("a platform region driver must bridge");
    assert!(
        !present,
        "a non-PCI region driver leaves dma_present = false (T05 is bus-master only)"
    );
}

#[test]
fn missing_element_size_key_is_rejected() {
    let err = with_model(
        "dma_missing_size",
        &region_source(
            "pci",
            "#[rseq::dma_region(id = \"tx_ring\", element_count = 256)] \
             pub tx_ring: DmaRegion<LegacyTxDesc>",
            "",
        ),
        |_| (),
    )
    .expect_err("a dma_region missing element_size must reject");
    assert!(
        err.contains("dma-management validation:") && err.contains("element_size"),
        "the reject must render the DMA diagnostic naming element_size; got: {err}"
    );
}

#[test]
fn missing_element_count_key_is_rejected() {
    let err = with_model(
        "dma_missing_count",
        &region_source(
            "pci",
            "#[rseq::dma_region(id = \"tx_ring\", element_size = 16)] \
             pub tx_ring: DmaRegion<LegacyTxDesc>",
            "",
        ),
        |_| (),
    )
    .expect_err("a dma_region missing element_count must reject");
    assert!(
        err.contains("dma-management validation:") && err.contains("element_count"),
        "the reject must render the DMA diagnostic naming element_count; got: {err}"
    );
}

#[test]
fn real_borrow_closure_body_survives_into_ir() {
    use regseq_model::{InstanceKind, StatementKind};
    // The REAL canonical borrow surface: `write(i, |d: &mut V| { d.field = … })`.
    // The closure body's field stores must survive into the IR so the backend can
    // inline them as the structured element access (T13).
    let src = region_source(
        "pci",
        E1000_REGION_FIELD,
        "self.tx_ring.write::<LegacyTxDesc>(0, |d: &mut LegacyTxDesc| { d.buffer_addr = 0x2000; d.length = 0x40; });",
    );
    // The REAL borrow closure must bridge AND the closure body's field stores
    // must survive into the IR (the backend inlines them). Count the
    // field-projected Assign statements in the closure-call shim body.
    let (n_accesses, field_stores) = with_model("dma_borrow_ir", &src, |model| {
        let driver = model.dma.drivers.values().next().expect("one dma driver");
        let n_accesses = driver.regions[0].element_accesses.len();
        // Find the closure-call shim body and count its field-store Assigns.
        let mut field_stores = 0usize;
        for inst in model.instances.iter() {
            let InstanceKind::Function(f) = &inst.kind else { continue };
            let Some(body) = f.body() else { continue };
            let path = model.strings.resolve(model.items[inst.item].def_path.rendered);
            if !path.contains("closure") { continue }
            for bb in body.basic_blocks.iter() {
                for stmt in bb.statements.iter() {
                    if let StatementKind::Assign(dst, _) = &stmt.kind {
                        if dst.projection.iter().any(|p| matches!(p, regseq_model::PlaceElem::Field(..))) {
                            field_stores += 1;
                        }
                    }
                }
            }
        }
        (n_accesses, field_stores)
    })
    .expect("the real borrow closure must bridge");
    assert_eq!(n_accesses, 1, "one element_access fact recognized");
    assert!(
        field_stores >= 2,
        "the closure body's two field stores (buffer_addr, lower) must survive into IR; got {field_stores}"
    );
}

#[test]
fn non_dma_borrow_closure_is_still_rejected() {
    // T13 soundness: the closure-reified-sig carve-out is SCOPED to a recognized
    // `dma_element_*` call. A NON-DMA borrow closure (here passed to a plain
    // generic `apply` that is NOT a DMA intrinsic) must still hit the global
    // substantive-HRTB guard — proof the carve-out did not weaken the check for
    // non-DMA code. The borrow closure's `for<'a> fn((&'a mut u32,))` reified
    // signature is the rejection trigger.
    let src = format!(
        "#![feature(register_tool)]\n#![register_tool(rseq)]\n\
         {LIFECYCLE_SURFACE}\n{DMA_SURFACE}\n\
         #[inline(never)]\n\
         pub fn apply<F: FnOnce(&mut u32)>(mut x: u32, f: F) -> u32 {{ f(&mut x); x }}\n\
         #[rseq::driver(name = \"d\", compatible = [\"acme,d\"], bus = \"pci\")]\n\
         pub struct D {{ {E1000_REGION_FIELD} }}\n\
         impl Lifecycle for D {{\n    fn on_init(&mut self) -> Result<(), RegseqError> {{\n        let _ = apply(7, |v: &mut u32| {{ *v = 9; }});\n        Ok(())\n    }}\n}}\n\
         pub fn make() -> D {{ loop {{}} }}\npub fn entry() {{}}\n"
    );
    let err = with_model("dma_nondma_borrow", &src, |_| ())
        .expect_err("a non-DMA borrow closure must still be rejected by the HRTB guard");
    assert!(
        err.contains("substantive HRTB"),
        "the non-DMA borrow closure must trip the unchanged substantive-HRTB guard; got: {err}"
    );
}

#[test]
fn substantive_rustcall_hrtb_in_dma_body_is_still_rejected() {
    // FIX #1 soundness (Codex UNSOUND finding): the T13 closure-reified-sig
    // carve-out is gated on SHAPE (RustCall + single-tuple-input) AND an
    // untuple-recheck of the §17.1 substantive-HRTB analysis. An author who
    // spells (on nightly via `#![feature(unboxed_closures)]`) a SUBSTANTIVE
    // RustCall-tuple fn-ptr, co-located in a DMA element-access body so the
    // `DmaElementClosureRelaxScope` window is OPEN, must STILL be rejected — the
    // tuple-wrap shape alone must NOT bypass the guard. Two concrete bypass
    // shapes are exercised:
    //   (a) RELATIONAL: `for<'a> extern "rust-call" fn((&'a mut u32, &'a mut u32))`
    //       — untuples to two refs sharing one bound region (condition 4).
    //   (b) OUTPUT-REGION: `for<'a> extern "rust-call" fn((&'a u32,)) -> &'a u32`
    //       — bound region in the OUTPUT (condition 5). The same region also rides
    //       an input ref so rustc accepts the type (an output-only bound region is
    //       a rustc-level E0581 reject before the bridge ever runs); the §17.1
    //       analysis flags the output occurrence as substantive regardless.
    // Each is declared as a fn-ptr-typed local inside `on_init`, which ALSO calls
    // a recognized `dma_element_*` intrinsic (so the relax window is open during
    // that body's materialisation). The benign closure surface stays accepted
    // (`element_write_closure_recognized_as_access_fact`); the carve-out only
    // relaxes the non-substantive untupled form.
    let shapes = [
        // (a) relational: same bound region in two ref slots.
        "for<'a> extern \"rust-call\" fn((&'a mut u32, &'a mut u32))",
        // (b) output-region: bound region in the output (also in one input so the
        // type is well-formed for rustc).
        "for<'a> extern \"rust-call\" fn((&'a u32,)) -> &'a u32",
    ];
    for shape in shapes {
        let init_body = format!(
            // The substantive fn-ptr sig must reach the T13 carve-out site
            // (`translate_generic_args`) as a GENERIC ARG while the relax window is
            // open. We carry it as `PhantomData<F>`: a zero-field marker struct, so
            // its only translation of `F` is through the gated generic-arg path —
            // it has NO field whose layout would re-translate `F` through the
            // non-gated `erase_regions_or_reject` (an enum/struct field would
            // reject the bare fn-ptr there first, masking the carve-out under
            // test). The local is pinned live (and its type translated during the
            // window-OPEN `on_init` body materialisation, NOT a scan-pass `Call`
            // generic arg) by a plain by-ref use, which lowers to a `Ref` rvalue
            // statement rather than a generic call.
            "self.tx_ring.write::<LegacyTxDesc>(0, |d: &mut LegacyTxDesc| {{ d.buffer_addr = 0x2000; }});\n\
             let _bypass: core::marker::PhantomData<{shape}> = core::marker::PhantomData;\n\
             let _live: *const _ = &_bypass;"
        );
        let src = format!(
            "#![feature(register_tool)]\n#![register_tool(rseq)]\n\
             #![feature(unboxed_closures)]\n\
             {LIFECYCLE_SURFACE}\n{DMA_SURFACE}\n\
             #[rseq::driver(name = \"d\", compatible = [\"acme,d\"], bus = \"pci\")]\n\
             pub struct D {{ {E1000_REGION_FIELD} }}\n\
             impl Lifecycle for D {{\n    fn on_init(&mut self) -> Result<(), RegseqError> {{\n        {init_body}\n        Ok(())\n    }}\n}}\n\
             pub fn make() -> D {{ loop {{}} }}\npub fn entry() {{}}\n"
        );
        let err = with_model("dma_bypass_hrtb", &src, |_| ()).expect_err(
            "a SUBSTANTIVE RustCall-tuple HRTB co-located in a DMA body must STILL be rejected",
        );
        assert!(
            err.contains("substantive HRTB"),
            "the substantive RustCall HRTB shape `{shape}` must trip the §17.1 guard \
             via the untuple-recheck even with the relax window open; got: {err}"
        );
    }
}

#[test]
fn element_layout_facts_are_resolved_and_alignment_recovered() {
    // FIX #2: `T`'s ABI layout (size/align + POD) is resolved at recognition, and
    // the omitted `alignment` defaults to `align_of::<T>()` (NOT 1). `LegacyTxDesc`
    // is `#[repr(C)]` u64+u16+u8+u8+u8+u8+u16 = 16 bytes, align 8, no Drop glue.
    let (size, align, is_pod, recovered_alignment) = with_model(
        "dma_layout_facts",
        &region_source(
            "pci",
            E1000_REGION_FIELD,
            "let _a = self.tx_ring.device_addr();",
        ),
        |model| {
            let driver = model.dma.drivers.values().next().expect("one dma driver");
            let region = &driver.regions[0];
            let layout = region.element_layout.expect("element_layout resolved");
            (layout.size, layout.align, layout.is_pod, region.alignment)
        },
    )
    .expect("a driver with a DmaRegion field must bridge");
    assert_eq!(size, 16, "size_of::<LegacyTxDesc>() = 16");
    assert_eq!(align, 8, "align_of::<LegacyTxDesc>() = 8");
    assert!(is_pod, "a Drop-free descriptor is POD (conservative !needs_drop)");
    assert_eq!(
        recovered_alignment,
        Some(8),
        "omitted alignment recovers to align_of::<T>() (NOT 1), GRILL D08/C03"
    );
}

#[test]
fn element_size_smaller_than_t_is_rejected() {
    // FIX #2 declaration-time layout guard: a declared `element_size` smaller than
    // `size_of::<T>()` is a layout mismatch (the stride cannot hold the element).
    // `LegacyTxDesc` is 16 bytes; declaring `element_size = 8` must REJECT.
    let bad_field = "#[rseq::dma_region(id = \"tx_ring\", element_size = 8, element_count = 256)] \
                     pub tx_ring: DmaRegion<LegacyTxDesc>";
    let err = with_model(
        "dma_layout_too_small",
        &region_source("pci", bad_field, "let _a = self.tx_ring.device_addr();"),
        |_| (),
    )
    .expect_err("element_size < size_of::<T>() must be rejected by the layout guard");
    assert!(
        err.contains("DMA-ERR-LAYOUT-MISMATCH"),
        "the too-small element_size must trip the declaration-time layout guard; got: {err}"
    );
}

#[test]
fn non_power_of_two_alignment_is_rejected() {
    // FIX #2 declaration-time layout guard: an explicit non-power-of-two
    // `alignment` is rejected. (24 >= align_of and divides 48, so only the
    // power-of-two check fires.)
    let bad_field =
        "#[rseq::dma_region(id = \"tx_ring\", element_size = 48, element_count = 256, alignment = 24)] \
         pub tx_ring: DmaRegion<LegacyTxDesc>";
    let err = with_model(
        "dma_layout_bad_align",
        &region_source("pci", bad_field, "let _a = self.tx_ring.device_addr();"),
        |_| (),
    )
    .expect_err("a non-power-of-two alignment must be rejected");
    assert!(
        err.contains("DMA-ERR-LAYOUT-MISMATCH"),
        "the non-power-of-two alignment must trip the layout guard; got: {err}"
    );
}

#[test]
fn view_layout_copies_element_layout_for_default_view() {
    // FIX #2: a `write::<LegacyTxDesc>` element-access whose view type equals the
    // declared `T` copies the region's `element_layout` into the per-site fact.
    let (view_size, view_align) = with_model(
        "dma_view_layout",
        &region_source(
            "pci",
            E1000_REGION_FIELD,
            "self.tx_ring.write::<LegacyTxDesc>(0, |d: &mut LegacyTxDesc| { d.buffer_addr = 0x2000; });",
        ),
        |model| {
            let driver = model.dma.drivers.values().next().expect("one dma driver");
            let region = &driver.regions[0];
            let fact = region
                .element_accesses
                .first()
                .expect("one element_access fact");
            let layout = fact.view_layout.expect("view_layout populated");
            (layout.size, layout.align)
        },
    )
    .expect("a region.write closure call must bridge");
    assert_eq!(view_size, 16, "view layout copies T's size (V = T)");
    assert_eq!(view_align, 8, "view layout copies T's align (V = T)");
}

#[test]
fn driver_without_region_field_has_no_dma_surface() {
    let n_drivers = with_model(
        "dma_none",
        &region_source("pci", "pub n: u32", ""),
        |model| model.dma.drivers.len(),
    )
    .expect("a region-less driver must bridge");
    assert_eq!(n_drivers, 0, "no region field => no DMA surface");
}

// ===========================================================================
// M2 / T07 — DmaChannel + DmaStreamBuffer recognition
// ===========================================================================

#[test]
fn dma_channel_field_recognized_with_direction() {
    // A `#[rseq::dma_channel(id, direction)]` field interns a `DmaChannelFacts`
    // (id + direction) and pushes the `AttrData::DmaChannelResource` marker. A
    // slave channel on a PLATFORM bus must NOT set the bus-master `dma_present`.
    let (n_channels, id, direction, has_marker, dma_present) = with_model(
        "dma_channel_field",
        &region_source("platform", PL011_CHANNEL_FIELD, ""),
        |model| {
            let driver = model.dma.drivers.values().next().expect("one dma driver");
            let channel = &driver.channels[0];
            let id = model.strings.resolve(channel.id).to_string();
            let has_marker = model.domain_attrs.field_attrs.values().any(|attrs| {
                attrs
                    .iter()
                    .any(|a| matches!(a, AttrData::DmaChannelResource { .. }))
            });
            let dma_present = model
                .device_lifecycle
                .drivers
                .values()
                .next()
                .map(|d| d.descriptor.dma_present)
                .unwrap_or(true);
            (
                driver.channels.len(),
                id,
                channel.direction,
                has_marker,
                dma_present,
            )
        },
    )
    .expect("a driver with a DmaChannel field must bridge");

    assert_eq!(n_channels, 1, "exactly one channel recognized");
    assert_eq!(id, "rx", "the channel id is the join key");
    assert_eq!(
        direction,
        DmaFlow::DeviceToHost,
        "the declared direction = device_to_host is captured"
    );
    assert!(has_marker, "field_attrs must carry the DmaChannelResource marker");
    assert!(
        !dma_present,
        "a slave DmaChannel must NOT set the bus-master dma_present bit (T05 is region-only)"
    );
}

#[test]
fn channel_transport_sites_resolve_to_the_channel() {
    // The 5 transport ops resolve to the channel resource: their callees classify
    // as `IntrinsicKind::Dma(Channel*)` and the receiver is the marked field. We
    // check the channel is recognized AND the consumed-Result body bridges (the
    // confirming gate is the emit-side receiver recovery, exercised by T14).
    let init = "self.rx.configure();\n\
                self.rx.stop();\n\
                let r = self.rx.start_transfer();\n        r?;";
    let n_channels = with_model(
        "dma_channel_transport",
        &region_source("platform", PL011_CHANNEL_FIELD, init),
        |model| {
            model
                .dma
                .drivers
                .values()
                .next()
                .map(|d| d.channels.len())
                .unwrap_or(0)
        },
    )
    .expect("a driver exercising the channel transport ops must bridge");
    assert_eq!(n_channels, 1, "the channel is recognized");
}

#[test]
fn missing_direction_key_is_rejected() {
    // T07 negative: a `#[rseq::dma_channel]` with no `direction` key must reject
    // (a slave channel's direction is mandatory, S02/D05).
    let err = with_model(
        "dma_channel_no_dir",
        &region_source(
            "platform",
            "#[rseq::dma_channel(id = \"rx\")] pub rx: DmaChannel",
            "",
        ),
        |_| (),
    )
    .expect_err("a dma_channel missing direction must reject");
    assert!(
        err.contains("dma-management validation:") && err.contains("direction"),
        "the reject must name the missing direction; got: {err}"
    );
}

#[test]
fn unknown_direction_token_is_rejected() {
    let err = with_model(
        "dma_channel_bad_dir",
        &region_source(
            "platform",
            "#[rseq::dma_channel(id = \"rx\", direction = \"sideways\")] pub rx: DmaChannel",
            "",
        ),
        |_| (),
    )
    .expect_err("an unknown direction token must reject");
    assert!(
        err.contains("dma-management validation:") && err.contains("direction"),
        "the reject must name the bad direction; got: {err}"
    );
}

#[test]
fn streaming_site_recorded_with_flow_and_lifetime() {
    // T07 streaming: `dma::stream(buf, flow)` records a `DmaStreamFacts` (flow +
    // RAII lifetime) into the owning driver's `streams`. The buffer's Result is
    // consumed (`let b = …?`) so the fallibility lint passes; the flow = 1 =
    // host_to_device.
    let init = "let mut byte: u8 = 0;\n\
                let b = dma::stream(&mut byte as *mut u8, 1u32)?;\n\
                let _addr = b.device_addr();";
    let (n_streams, flow, extent_nonempty) = with_model(
        "dma_stream_site",
        &region_source("platform", PL011_CHANNEL_FIELD, init),
        |model| {
            let driver = model.dma.drivers.values().next().expect("one dma driver");
            let stream = driver.streams.first().expect("one stream fact recorded");
            let extent = stream.lifetime.extent.span;
            (
                driver.streams.len(),
                stream.flow,
                extent.hi >= extent.lo,
            )
        },
    )
    .expect("a dma::stream call must bridge");
    assert_eq!(n_streams, 1, "exactly one stream fact recorded");
    assert_eq!(
        flow,
        DmaFlow::HostToDevice,
        "the flow argument (1 = host_to_device) is captured"
    );
    assert!(extent_nonempty, "the RAII lifetime extent (call-site span) is recorded");
}

// ===========================================================================
// M2 / T17 — fallibility lint (C10/I06)
// ===========================================================================

#[test]
fn discarded_start_transfer_bare_statement_is_rejected() {
    // A bare-statement discard of a fallible op's Result. (Caught by the bridge
    // lint; the `#[must_use]` surface also catches this at rustc level — both
    // reject.) The bridge lint is the load-bearing one here.
    let err = with_model(
        "dma_discard_bare",
        &region_source("platform", PL011_CHANNEL_FIELD, "self.rx.start_transfer();"),
        |_| (),
    )
    .expect_err("a discarded start_transfer() must reject");
    assert!(
        err.contains("DMA-ERR-DISCARDED-RESULT") || err.contains("must_use") || err.contains("unused"),
        "a bare discarded start_transfer must be rejected; got: {err}"
    );
}

#[test]
fn explicit_let_underscore_discard_is_rejected() {
    // THE T17 case: the explicit `let _ = …` discard that Rust's
    // `unused_must_use` does NOT fire on. The BRIDGE fallibility lint must reject
    // it (DMA-ERR-DISCARDED-RESULT).
    let err = with_model(
        "dma_discard_let_underscore",
        &region_source(
            "platform",
            PL011_CHANNEL_FIELD,
            "let _ = self.rx.start_transfer();",
        ),
        |_| (),
    )
    .expect_err("a `let _ = start_transfer()` discard must reject");
    assert!(
        err.contains("DMA-ERR-DISCARDED-RESULT"),
        "the explicit `let _ =` discard must trip the bridge fallibility lint; got: {err}"
    );
}

#[test]
fn consumed_result_passes() {
    // The positive control: a correctly-consumed Result (`let r = …; r?;`) must
    // bridge — the consume (`?` reads `r`) keeps the lint quiet.
    let init = "let r = self.rx.start_transfer();\n        r?;";
    let ok = with_model(
        "dma_consumed_ok",
        &region_source("platform", PL011_CHANNEL_FIELD, init),
        |model| model.dma.drivers.values().next().map(|d| d.channels.len()).unwrap_or(0),
    )
    .expect("a consumed start_transfer() Result must bridge");
    assert_eq!(ok, 1, "the channel is recognized and the consumed Result passes the lint");
}

#[test]
fn discarded_stream_let_underscore_is_rejected() {
    // The streaming op is also in the fallibility set: `let _ = dma::stream(…)`
    // discards the mapping Result and must reject.
    let init = "let mut byte: u8 = 0;\n\
                let _ = dma::stream(&mut byte as *mut u8, 1u32);";
    let err = with_model(
        "dma_discard_stream",
        &region_source("platform", PL011_CHANNEL_FIELD, init),
        |_| (),
    )
    .expect_err("a discarded dma::stream Result must reject");
    assert!(
        err.contains("DMA-ERR-DISCARDED-RESULT"),
        "a `let _ = dma::stream(…)` discard must trip the fallibility lint; got: {err}"
    );
}

#[test]
fn discarded_stop_is_allowed() {
    // `stop()` is INFALLIBLE (returns `()`), so discarding it is NOT a lint
    // violation — the channel still bridges.
    let ok = with_model(
        "dma_stop_ok",
        &region_source("platform", PL011_CHANNEL_FIELD, "self.rx.stop();"),
        |model| model.dma.drivers.values().next().map(|d| d.channels.len()).unwrap_or(0),
    )
    .expect("an infallible stop() must bridge (it is exempt from the fallibility lint)");
    assert_eq!(ok, 1, "stop() is infallible and exempt");
}

#[test]
fn core_mem_drop_discard_is_rejected() {
    // BLOCKING #1: the idiomatic explicit-discard `core::mem::drop(op())` MOVES the
    // Result into the drop sink — a genuine discard the body-wide "any read" scan
    // would otherwise count as consumption. It MUST reject.
    let err = with_model(
        "dma_discard_mem_drop",
        &region_source(
            "platform",
            PL011_CHANNEL_FIELD,
            "core::mem::drop(self.rx.start_transfer());",
        ),
        |_| (),
    )
    .expect_err("a core::mem::drop(start_transfer()) discard must reject");
    assert!(
        err.contains("DMA-ERR-DISCARDED-RESULT"),
        "core::mem::drop(op()) must trip the fallibility lint as a discard sink; got: {err}"
    );
}

#[test]
fn core_mem_forget_discard_is_rejected() {
    // BLOCKING #1: `core::mem::forget(op())` is the other explicit discard sink.
    let err = with_model(
        "dma_discard_mem_forget",
        &region_source(
            "platform",
            PL011_CHANNEL_FIELD,
            "core::mem::forget(self.rx.start_transfer());",
        ),
        |_| (),
    )
    .expect_err("a core::mem::forget(start_transfer()) discard must reject");
    assert!(
        err.contains("DMA-ERR-DISCARDED-RESULT"),
        "core::mem::forget(op()) must trip the fallibility lint as a discard sink; got: {err}"
    );
}

#[test]
fn return_propagation_is_accepted() {
    // BLOCKING #1 false-reject fix: `return self.rx.start_transfer();` propagates
    // the Result as the body's return value — legitimate consumption. The call's
    // destination is the return place, which must NOT be flagged as a discard.
    let ok = with_model(
        "dma_return_prop",
        &region_source(
            "platform",
            PL011_CHANNEL_FIELD,
            "return self.rx.start_transfer();",
        ),
        |model| model.dma.drivers.values().next().map(|d| d.channels.len()).unwrap_or(0),
    )
    .expect("`return self.rx.start_transfer();` is legitimate propagation, must NOT false-reject");
    assert_eq!(ok, 1, "the channel bridges; return-propagation is consumption");
}

#[test]
fn direct_question_mark_propagation_unbound_is_accepted() {
    // BLOCKING #1 control: `self.rx.start_transfer()?;` (the Result not bound to a
    // local, propagated directly with `?`) is legitimate consumption — the `?`
    // desugar reads the call temp (a Discriminant switch). Must NOT reject.
    let ok = with_model(
        "dma_direct_question",
        &region_source(
            "platform",
            PL011_CHANNEL_FIELD,
            "self.rx.start_transfer()?;",
        ),
        |model| model.dma.drivers.values().next().map(|d| d.channels.len()).unwrap_or(0),
    )
    .expect("`self.rx.start_transfer()?;` direct propagation must NOT false-reject");
    assert_eq!(ok, 1, "direct `?` propagation is consumption");
}



// ===========================================================================
// M3 / T09 — span + completion recognition; T20 — anti-form/bounds verify
// ===========================================================================

/// A PL011-style RX field set: a `device_to_host` channel + a coherent
/// `DmaRegion<u8>` (element_size=1) destination buffer. The span endpoint is
/// element-indexed; for `DmaRegion<u8>` elements == bytes.
const PL011_RX_FIELDS: &str = "#[rseq::dma_channel(id = \"rx\", direction = \"device_to_host\")] pub rx: DmaChannel, \
     #[rseq::dma_region(id = \"rx_buf\", element_size = 1, element_count = 64)] pub rx_buf: DmaRegion<u8>";

/// Build a driver with `PL011_RX_FIELDS`, an `on_init` body, and an OPTIONAL extra
/// inherent `impl D { … }` block (e.g. a `#[rseq::dma_completion]` handler).
fn pl011_source(init_body: &str, extra_impl: &str) -> String {
    format!(
        "#![feature(register_tool)]\n#![register_tool(rseq)]\n\
         {LIFECYCLE_SURFACE}\n{DMA_SURFACE}\n\
         #[rseq::driver(name = \"d\", compatible = [\"acme,d\"], bus = \"platform\")]\n\
         pub struct D {{ {PL011_RX_FIELDS} }}\n\
         impl Lifecycle for D {{\n    fn on_init(&mut self) -> Result<(), RegseqError> {{\n        {init_body}\n        Ok(())\n    }}\n}}\n\
         {extra_impl}\n\
         pub fn make() -> D {{ loop {{}} }}\npub fn entry() {{}}\n"
    )
}

#[test]
fn span_endpoint_recognized_with_addr_len_provenance() {
    // T09(a): `region.span(0..8)` records a `DmaRegionSpan` with element_offset=0,
    // element_size=1 (DmaRegion<u8>), len = 8*1 = 8, provenance = "rx_buf".
    let init = "self.rx.configure();\n\
                self.rx.stage_transfer(self.rx_buf.span(0..8))?;";
    let (n_spans, offset, element_size, len, provenance) = with_model(
        "dma_span_recog",
        &pl011_source(init, ""),
        |model| {
            let driver = model.dma.drivers.values().next().expect("one dma driver");
            let span = driver.spans.first().expect("one span recorded");
            (
                driver.spans.len(),
                span.device_addr.element_offset,
                span.device_addr.element_size,
                span.len,
                model.strings.resolve(span.provenance).to_string(),
            )
        },
    )
    .expect("a region.span(range) stage_transfer must bridge");
    assert_eq!(n_spans, 1, "exactly one span endpoint recorded");
    assert_eq!(offset, 0, "element_offset = range.start = 0");
    assert_eq!(element_size, 1, "element_size = 1 (DmaRegion<u8>)");
    assert_eq!(len, 8, "len = range.len() * element_size = 8 * 1");
    assert_eq!(provenance, "rx_buf", "provenance is the source region id");
}

#[test]
fn span_endpoint_nonzero_offset() {
    // T09(a): `region.span(4..10)` → offset=4, len=(10-4)*1=6.
    let init = "self.rx.stage_transfer(self.rx_buf.span(4..10))?;";
    let (offset, len) = with_model(
        "dma_span_offset",
        &pl011_source(init, ""),
        |model| {
            let span = &model.dma.drivers.values().next().unwrap().spans[0];
            (span.device_addr.element_offset, span.len)
        },
    )
    .expect("a nonzero-offset span must bridge");
    assert_eq!(offset, 4, "element_offset = range.start = 4");
    assert_eq!(len, 6, "len = (10-4)*1 = 6");
}

#[test]
fn completion_binding_recognized() {
    // T09(b): `#[rseq::dma_completion(channel = "rx")]` records a CompletionBinding
    // (channel + handler item + DmaCompletion payload marker) AND an
    // AttrData::DmaCompletionBinding on the handler item.
    let extra = "impl D {\n    #[rseq::dma_completion(channel = \"rx\")]\n    \
                 pub fn rx_done(&mut self, _r: DmaCompletion) -> Result<(), RegseqError> { Ok(()) }\n}";
    let (n_completions, channel, has_payload, has_item_marker) = with_model(
        "dma_completion_recog",
        &pl011_source("self.rx.configure();", extra),
        |model| {
            let driver = model.dma.drivers.values().next().expect("one dma driver");
            let binding = driver.completions.first().expect("one completion binding");
            let channel = model.strings.resolve(binding.channel).to_string();
            let has_payload = model.strings.resolve(binding.payload).contains("DmaCompletion");
            let has_item_marker = model.domain_attrs.item_attrs.iter().any(|attrs| {
                attrs.iter().any(|a| matches!(a, AttrData::DmaCompletionBinding { .. }))
            });
            (driver.completions.len(), channel, has_payload, has_item_marker)
        },
    )
    .expect("a #[rseq::dma_completion] handler must bridge");
    assert_eq!(n_completions, 1, "one completion binding recorded");
    assert_eq!(channel, "rx", "the bound channel id is captured");
    assert!(has_payload, "the DmaCompletion payload type name is the registration marker");
    assert!(has_item_marker, "the handler item carries the DmaCompletionBinding marker");
}

#[test]
fn completion_missing_channel_is_rejected() {
    // T09(b) negative: `#[rseq::dma_completion]` with no channel → reject.
    let extra = "impl D {\n    #[rseq::dma_completion()]\n    \
                 pub fn rx_done(&mut self, _r: DmaCompletion) -> Result<(), RegseqError> { Ok(()) }\n}";
    let err = with_model("dma_completion_no_chan", &pl011_source("", extra), |_| ())
        .expect_err("a dma_completion with no channel must reject");
    assert!(
        err.contains("DMA-ERR-COMPLETION-ATTR") && err.contains("channel"),
        "the reject must name the missing channel; got: {err}"
    );
}

#[test]
fn anti_form_bare_region_to_stage_transfer_is_rejected() {
    // T20 anti-form (I07): passing the bare region (a `&DmaRegion<T>`) to
    // stage_transfer instead of `region.span(range)` is a forbidden anti-form.
    // (The `TransferEndpoint` overload accepts the span; the bare region is the
    // forbidden form the bridge must reject.) We pass `self.rx_buf.span` is NOT
    // used — instead the region reference is passed (anti-form). To express the
    // anti-form we add a `TransferEndpoint` impl for `&DmaRegion<u8>` in an extra
    // block so it type-checks, and the bridge anti-form check rejects it.
    let extra = "impl TransferEndpoint for &DmaRegion<u8> {}";
    let init = "self.rx.stage_transfer(&self.rx_buf)?;";
    let err = with_model("dma_anti_form", &pl011_source(init, extra), |_| ())
        .expect_err("a bare stage_transfer(&DmaRegion<T>) must reject");
    assert!(
        err.contains("DMA-ERR-ANTI-FORM") && err.contains("rx_buf"),
        "the reject must name the anti-form region; got: {err}"
    );
}

#[test]
fn span_out_of_bounds_is_rejected() {
    // T20 span-bounds: `region.span(0..100)` on a 64-element region is OOB.
    let init = "self.rx.stage_transfer(self.rx_buf.span(0..100))?;";
    let err = with_model("dma_span_oob", &pl011_source(init, ""), |_| ())
        .expect_err("an out-of-bounds span must reject");
    assert!(
        err.contains("DMA-ERR-SPAN-BOUNDS") && err.contains("out of bounds"),
        "the reject must name the OOB span; got: {err}"
    );
}

#[test]
fn zero_length_span_is_rejected() {
    // T20 span-bounds: `region.span(8..8)` is a zero-length span.
    let init = "self.rx.stage_transfer(self.rx_buf.span(8..8))?;";
    let err = with_model("dma_span_zero", &pl011_source(init, ""), |_| ())
        .expect_err("a zero-length span must reject");
    assert!(
        err.contains("DMA-ERR-SPAN-BOUNDS") && err.contains("zero-length"),
        "the reject must name the zero-length span; got: {err}"
    );
}

// ===========================================================================
// M3 fix-pass — fail-loud span / completion-channel validation
// ===========================================================================

#[test]
fn non_const_span_range_is_rejected() {
    // BLOCKING #1 (M3 fix): a NON-const span range (`span(0..n)` where `n` is a
    // runtime value) is rejected LOUDLY at recognition — recording start=0,len=0
    // would silently degenerate to a zero-length transfer. Only a const range is
    // supported this cycle.
    let init = "let n = self.rx_buf.element_count();\n\
                self.rx.stage_transfer(self.rx_buf.span(0..n))?;";
    let err = with_model("dma_span_nonconst", &pl011_source(init, ""), |_| ())
        .expect_err("a non-const span range must reject (fail loud, not a silent 0/0)");
    assert!(
        err.contains("DMA-ERR-SPAN-BOUNDS") && err.contains("non-const span range"),
        "the reject must name the non-const span range; got: {err}"
    );
}

#[test]
fn completion_unknown_channel_is_rejected() {
    // BLOCKING #2 (M3 fix): a `#[rseq::dma_completion(channel = "typo")]` that
    // names no declared `#[rseq::dma_channel(id = …)]` is a dangling registration
    // and must reject (the only declared channel is "rx").
    let extra = "impl D {\n    #[rseq::dma_completion(channel = \"typo\")]\n    \
                 pub fn rx_done(&mut self, _r: DmaCompletion) -> Result<(), RegseqError> { Ok(()) }\n}";
    let err = with_model("dma_completion_typo", &pl011_source("self.rx.configure();", extra), |_| ())
        .expect_err("a completion binding for an undeclared channel must reject");
    assert!(
        err.contains("DMA-ERR-UNKNOWN-COMPLETION-CHANNEL") && err.contains("typo"),
        "the reject must name the dangling channel; got: {err}"
    );
}

// ===========================================================================
// M4 / T19 — C03 layout verify rule (the real `DmaPod` predicate + per-access
// view clauses). Each negative test is empirically witness-able: it FAILS if the
// corresponding clause in `dma::layout::verify_dma_layout` is neutered. The
// positive `valid_default_view_passes_layout_verify` is the neuter baseline (a
// genuinely-valid view must still bridge, so a check that always-rejects is also
// caught).
// ===========================================================================

#[test]
fn valid_default_view_passes_layout_verify() {
    // The neuter-witness baseline: the E1000 ring (`element_size = 16`,
    // `element_count = 256`) written through `write::<LegacyTxDesc>(0, …)` — a
    // repr(C) 16-byte POD view at a valid index — must PASS every C03 clause. (If
    // the verify rule rejected unconditionally, this would fail.)
    let _ok = with_model(
        "dma_layout_ok",
        &region_source(
            "pci",
            E1000_REGION_FIELD,
            "self.tx_ring.write::<LegacyTxDesc>(0, |d: &mut LegacyTxDesc| { d.buffer_addr = 0x2000; });",
        ),
        |model| model.dma.drivers.len(),
    )
    .expect("a valid repr(C) POD view at a valid index must pass the C03 layout verify");
}

#[test]
fn non_dma_pod_pointer_view_is_rejected() {
    // T19 `V: DmaPod` clause: a reinterpret view `V` that carries a RAW POINTER
    // field is not a sound DMA view (a host pointer is never a device-visible
    // descriptor field), so `write::<PtrView>` must compile-fail naming the
    // `V: DmaPod` clause. Witnesses the real POD predicate (the M1 `!needs_drop`
    // proxy would WRONGLY accept `PtrView` — it has no Drop glue).
    let err = with_model(
        "dma_pod_ptr_view",
        &region_source(
            "pci",
            E1000_REGION_FIELD,
            "self.tx_ring.write::<PtrView>(0, |v: &mut PtrView| { v.p = core::ptr::null(); });",
        ),
        |_| (),
    )
    .expect_err("a reinterpret view carrying a raw pointer must reject (V: DmaPod)");
    assert!(
        err.contains("DMA-ERR-LAYOUT-VERIFY") && err.contains("raw pointer"),
        "the reject must name the V: DmaPod clause + the raw-pointer reason; got: {err}"
    );
}

#[test]
fn non_repr_c_declared_t_is_rejected() {
    // T19 declaration `V: DmaPod` clause over the declared `T`: a `DmaRegion<T>`
    // whose `T` is a plain `#[repr(Rust)]` struct has an unpinned field layout, so
    // the declaration must compile-fail. Witnesses the POD predicate over the
    // declared type (the M1 `!needs_drop` proxy would WRONGLY accept `RustView`).
    let bad_field = "#[rseq::dma_region(id = \"tx_ring\", element_size = 8, element_count = 256)] \
                     pub tx_ring: DmaRegion<RustView>";
    let err = with_model(
        "dma_pod_rust_t",
        &region_source("pci", bad_field, "let _a = self.tx_ring.device_addr();"),
        |_| (),
    )
    .expect_err("a non-repr(C) declared T must reject (declaration V: DmaPod)");
    assert!(
        err.contains("DMA-ERR-LAYOUT-VERIFY") && err.contains("repr(C)"),
        "the reject must name the declaration DmaPod clause + the repr(C) reason; got: {err}"
    );
}

#[test]
fn oversized_reinterpret_view_is_rejected() {
    // T19 `size_of::<V>() <= element_size` clause: a 32-byte `write::<BigView>`
    // reinterpret against a 16-byte `element_size` ring would read past the slot,
    // so it must compile-fail. Witnesses the size clause (a verify that skipped the
    // size check would accept the oversized view).
    let err = with_model(
        "dma_oversized_view",
        &region_source(
            "pci",
            E1000_REGION_FIELD,
            "self.tx_ring.write::<BigView>(0, |v: &mut BigView| { v.a = 1; });",
        ),
        |_| (),
    )
    .expect_err("an oversized reinterpret view must reject (size_of::<V>() <= element_size)");
    assert!(
        err.contains("DMA-ERR-LAYOUT-VERIFY") && err.contains("size_of::<V>()"),
        "the reject must name the size clause; got: {err}"
    );
}

#[test]
fn out_of_bounds_static_index_is_rejected() {
    // T19 `i in [0, element_count)` clause: a `write::<LegacyTxDesc>(999, …)` on a
    // 256-element ring is out of bounds at a statically-known index, so it must
    // compile-fail. Witnesses the index-bound clause (a verify that skipped the
    // bound check would accept index 999).
    let err = with_model(
        "dma_oob_index",
        &region_source(
            "pci",
            E1000_REGION_FIELD,
            "self.tx_ring.write::<LegacyTxDesc>(999, |d: &mut LegacyTxDesc| { d.buffer_addr = 0x2000; });",
        ),
        |_| (),
    )
    .expect_err("an out-of-bounds static index must reject (i in [0, element_count))");
    assert!(
        err.contains("DMA-ERR-LAYOUT-VERIFY") && err.contains("out of bounds"),
        "the reject must name the index-bound clause; got: {err}"
    );
}

// ===========================================================================
// M4 / T22 — diagnostic family + compile-fail oracle corpus. The `DmaDiagnostic`
// family carries a uniform `DMA-ERR-*` code per rejection case (the recognition
// half; the binding half is `DmaL2Error`'s `DMA-L2-*` set). This corpus test
// asserts (a) every variant's `code()` is the prefix of its rendered message (so
// `code()` and the `#[error]` text never drift), and (b) the FULL coded reject set
// is present — the inventory that every rejection case has a coded diagnostic.
// ===========================================================================

#[test]
fn dma_diagnostic_codes_match_messages_and_cover_the_corpus() {
    use regseq_driver::bridge::DmaDiagnostic;

    let s = || "x".to_string();
    // One representative of every `DmaDiagnostic` variant (the full rejection corpus).
    let corpus: Vec<DmaDiagnostic> = vec![
        DmaDiagnostic::MissingKey { driver: s(), field: s(), key: "element_size" },
        DmaDiagnostic::MalformedAttr { driver: s(), field: s(), detail: s() },
        DmaDiagnostic::AttrOnNonRegion { driver: s(), field: s() },
        DmaDiagnostic::UnsupportedForm { driver: s(), field: s(), form: s() },
        DmaDiagnostic::DuplicateId { driver: s(), id: s() },
        DmaDiagnostic::LayoutMismatch { driver: s(), field: s(), detail: s() },
        DmaDiagnostic::ChannelAttr { driver: s(), field: s(), detail: s() },
        DmaDiagnostic::DiscardedResult { op: "dma::stream" },
        DmaDiagnostic::CompletionAttr { method: s(), detail: s() },
        DmaDiagnostic::UnknownCompletionChannel { method: s(), channel: s(), driver: s() },
        DmaDiagnostic::AntiForm { region: s() },
        DmaDiagnostic::DirectionMismatch { channel: s(), direction: s(), detail: s() },
        DmaDiagnostic::SpanBounds { region: s(), detail: s() },
        DmaDiagnostic::LayoutVerify { region: s(), clause: "V: DmaPod (access view)", detail: s() },
    ];

    // (a) Every variant's `code()` is the literal prefix of its rendered message.
    for d in &corpus {
        let msg = d.to_string();
        assert!(
            msg.starts_with(d.code()),
            "DmaDiagnostic::{:?} message must start with its code {:?}; got: {msg}",
            d,
            d.code()
        );
    }

    // (b) The full coded reject set is present (the corpus inventory). Every code is
    // a `DMA-ERR-*`, and the load-bearing M4 additions (LAYOUT-VERIFY, T19) +
    // M2/M3/T17 cases are all covered.
    let codes: std::collections::BTreeSet<&'static str> =
        corpus.iter().map(|d| d.code()).collect();
    for expected in [
        "DMA-ERR-MISSING-KEY",
        "DMA-ERR-MALFORMED-ATTR",
        "DMA-ERR-ATTR-ON-NON-REGION",
        "DMA-ERR-UNSUPPORTED-FORM",
        "DMA-ERR-DUPLICATE-ID",
        "DMA-ERR-LAYOUT-MISMATCH",
        "DMA-ERR-MISSING-DIRECTION",
        "DMA-ERR-DISCARDED-RESULT",
        "DMA-ERR-COMPLETION-ATTR",
        "DMA-ERR-UNKNOWN-COMPLETION-CHANNEL",
        "DMA-ERR-ANTI-FORM",
        "DMA-ERR-DIRECTION-MISMATCH",
        "DMA-ERR-SPAN-BOUNDS",
        "DMA-ERR-LAYOUT-VERIFY",
    ] {
        assert!(codes.contains(expected), "the reject corpus must include {expected}");
        assert!(expected.starts_with("DMA-ERR-"), "every recognition code is a DMA-ERR-* code");
    }
}

// ===========================================================================
// M4 / T19 — Codex milestone-review folds (bit-validity + fail-closed). Each is
// witness-able: it FAILS if the corresponding hardened clause is neutered.
// ===========================================================================

#[test]
fn bool_reinterpret_view_is_rejected() {
    // BLOCKING-1 (bit-validity): a `write::<bool>` reinterpret view is rejected even
    // though `bool` is 1 byte (fits element_size) and Drop-free — only 0x00/0x01 are
    // valid `bool` bit patterns, but a DMA device writes ARBITRARY bytes, so
    // reinterpreting a device-written byte as `bool` is UB. The M1 `!needs_drop`
    // proxy (and the pre-fold predicate, which accepted `bool`) would WRONGLY accept
    // this. DMA-ERR-LAYOUT-VERIFY naming the V: DmaPod clause + the bool reason.
    let err = with_model(
        "dma_bool_view",
        &region_source(
            "pci",
            E1000_REGION_FIELD,
            "self.tx_ring.write::<bool>(0, |v: &mut bool| { *v = false; });",
        ),
        |_| (),
    )
    .expect_err("a `bool` reinterpret view must reject (bit-validity, V: DmaPod)");
    assert!(
        err.contains("DMA-ERR-LAYOUT-VERIFY") && err.contains("bool"),
        "the reject must name the V: DmaPod clause + the bool bit-validity reason; got: {err}"
    );
}

#[test]
fn char_reinterpret_view_is_rejected() {
    // BLOCKING-1 (bit-validity), the `char` companion: `char` must be a valid Unicode
    // scalar, so a device-written 4 bytes reinterpreted as `char` is UB.
    let err = with_model(
        "dma_char_view",
        &region_source(
            "pci",
            E1000_REGION_FIELD,
            "self.tx_ring.write::<char>(0, |v: &mut char| { *v = 'a'; });",
        ),
        |_| (),
    )
    .expect_err("a `char` reinterpret view must reject (bit-validity, V: DmaPod)");
    assert!(
        err.contains("DMA-ERR-LAYOUT-VERIFY") && err.contains("char"),
        "the reject must name the char bit-validity reason; got: {err}"
    );
}

#[test]
fn non_const_element_index_is_rejected() {
    // BLOCKING-2 (non-const index): an `element_access` whose index is a RUNTIME
    // value (here the result of `element_count()`, a non-const local) is rejected —
    // generic-c emits `base + i*element_size` runtime pointer arithmetic with NO
    // runtime bounds guard, so a non-const index cannot be statically bounded
    // (mirrors the M3 non-const-span-range reject). DMA-ERR-LAYOUT-VERIFY.
    let init = "let i = self.tx_ring.element_count();\n        \
                self.tx_ring.write::<LegacyTxDesc>(i, |d: &mut LegacyTxDesc| { d.buffer_addr = 0x2000; });";
    let err = with_model(
        "dma_nonconst_index",
        &region_source("pci", E1000_REGION_FIELD, init),
        |_| (),
    )
    .expect_err("a non-const element index must reject (no runtime bounds guard)");
    assert!(
        err.contains("DMA-ERR-LAYOUT-VERIFY") && err.contains("non-const element index"),
        "the reject must name the non-const element index; got: {err}"
    );
}
