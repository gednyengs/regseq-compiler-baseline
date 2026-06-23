//! dma-management M3 fix-pass span-emit gc-shape tests (dma-management construct).
//!
//! Two emit-path checks using the lightweight whole-TU `emit_generic_c`:
//!   * `element_size != 1` span lowering — the addr/len arithmetic
//!     (`dev_base + offset*element_size`, `len = (end-start)*element_size`) is
//!     exercised with a non-1 element stride (the PL011 golden only covers
//!     element_size=1, so the multiply was never actually tested — Codex coverage
//!     gap, NON-BLOCKING).
//!   * a `DmaRegionSpan` endpoint the backend CANNOT statically recover (a span
//!     stored in a struct field and passed via the field projection) must FAIL
//!     LOUD (`EmitError::Unsupported`), NOT silently fall back to a `0/0` transfer
//!     (BLOCKING #1).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::PathBuf;

use regseq_backends::emit_generic_c;
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

/// A temp `.rs` file removed on drop.
struct TempSnippet {
    path: PathBuf,
}

impl TempSnippet {
    fn new(label: &str, src: &str) -> TempSnippet {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir()
            .join(format!("ccr-dma-span-{label}-{}-{nanos}.rs", std::process::id()));
        std::fs::write(&path, src).expect("must write temp snippet");
        TempSnippet { path }
    }
}

impl Drop for TempSnippet {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

const LIFECYCLE_SURFACE: &str = include_str!("../fixtures/_lifecycle_surface.rs");

/// The DMA author surface for these tests: a `DmaRegion<T>` with `span`, a
/// `DmaRegionSpan` endpoint type + `TransferEndpoint` overload, a `DmaChannel`
/// with `configure`/`stage_transfer`/`start_transfer`, and a 16-byte `#[repr(C)]`
/// element type (`Desc16`) so an element_size=16 span exercises the stride
/// multiply.
fn dma_surface() -> String {
    "
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Desc16 { pub a: u64, pub b: u64 }

pub struct DmaRegion<T> { _t: core::marker::PhantomData<T> }
impl<T> DmaRegion<T> {
    pub const fn new() -> Self { Self { _t: core::marker::PhantomData } }
    #[inline(never)] #[rseq::intrinsic(\"dma_region_span\")]
    pub fn span(&self, _range: core::ops::Range<usize>) -> DmaRegionSpan { loop {} }
}
pub struct DmaRegionSpan { _a: u64, _l: u64 }
pub trait TransferEndpoint {}
impl TransferEndpoint for DmaRegionSpan {}

pub struct DmaChannel { _id: u32 }
impl DmaChannel {
    pub const fn new() -> Self { Self { _id: 0 } }
    #[inline(never)] #[rseq::intrinsic(\"dma_configure\")]
    pub fn configure(&self) {}
    #[inline(never)] #[must_use] #[rseq::intrinsic(\"dma_stage_transfer\")]
    pub fn stage_transfer(&self, _e: impl TransferEndpoint) -> Result<(), RegseqError> { loop {} }
    #[inline(never)] #[must_use] #[rseq::intrinsic(\"dma_start_transfer\")]
    pub fn start_transfer(&self) -> Result<(), RegseqError> { loop {} }
}
"
    .to_string()
}

const RX_FIELDS: &str = "#[rseq::dma_channel(id = \"rx\", direction = \"device_to_host\")] pub rx: DmaChannel, \
     #[rseq::dma_region(id = \"buf\", element_size = 16, element_count = 64)] pub buf: DmaRegion<Desc16>";

fn source(init_body: &str, extra_impl: &str) -> String {
    format!(
        "#![feature(register_tool)]\n#![register_tool(rseq)]\n\
         {LIFECYCLE_SURFACE}\n{}\n\
         #[rseq::driver(name = \"d\", compatible = [\"acme,d\"], bus = \"platform\")]\n\
         pub struct D {{ {RX_FIELDS} }}\n\
         impl Lifecycle for D {{\n    fn on_init(&mut self) -> Result<(), RegseqError> {{\n        {init_body}\n        Ok(())\n    }}\n}}\n\
         {extra_impl}\n\
         pub fn make() -> D {{ loop {{}} }}\npub fn entry() {{}}\n",
        dma_surface()
    )
}

#[test]
fn span_element_size_16_lowers_stride_multiply() {
    // NON-BLOCKING coverage gap: `buf.span(2..4)` on a DmaRegion<Desc16>
    // (element_size=16) must lower to `dev_base("buf") + 2*16` and `len = (4-2)*16
    // = 32` — exercising the stride MULTIPLY (the PL011 golden's element_size=1
    // hides it).
    let snippet =
        TempSnippet::new("es16", &source("self.rx.stage_transfer(self.buf.span(2..4))?;", ""));
    let model = match try_bridge_fixture_path(&snippet.path) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("the element_size=16 span source must bridge, got {other:?}"),
    };
    let tu = emit_generic_c(&model).expect("emit must succeed for the element_size=16 span");
    assert!(
        tu.contains("regseq_dma_stage_transfer(\"rx\", (regseq_dma_region_dev_base(\"buf\") + 2ull * 16ull), 32ull, 0)"),
        "the span must lower addr = dev_base + 2*16 and len = (4-2)*16 = 32 (the stride multiply); got:\n{tu}"
    );
}

#[test]
fn unrecoverable_span_endpoint_fails_loud() {
    // BLOCKING #1: a `DmaRegionSpan` endpoint stored in a struct field and passed
    // via the field projection is NOT statically recoverable by the backend
    // (`span_endpoint_local` doesn't chase field projections). Because the endpoint
    // TYPE is `DmaRegionSpan`, emit MUST fail loud (Unsupported) — NOT silently
    // emit a 0/0 transfer.
    let extra = "pub struct Holder { pub s: DmaRegionSpan }";
    let init = "let h = Holder { s: self.buf.span(0..4) };\n\
                self.rx.stage_transfer(h.s)?;";
    let snippet = TempSnippet::new("field", &source(init, extra));
    let model = match try_bridge_fixture_path(&snippet.path) {
        // The bridge recognizes the `span()` call fine (the const range is valid);
        // the FAIL-LOUD happens at EMIT (the backend cannot recover the field-stored
        // span endpoint). A bridge reject here is also acceptable, but we expect Ok.
        BridgeOutcome::Ok(model) => model,
        other => panic!("the field-stored span source must bridge to emit, got {other:?}"),
    };
    let err = emit_generic_c(&model)
        .expect_err("an unrecoverable DmaRegionSpan endpoint must FAIL LOUD at emit, not emit 0/0");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("could not statically recover the DmaRegionSpan endpoint"),
        "emit must fail loud naming the unrecoverable span endpoint (not a silent 0/0); got: {msg}"
    );
}
