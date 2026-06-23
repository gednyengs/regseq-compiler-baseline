//! The dma-management M4 streaming-`DmaStreamBuffer` emit gc-shape test
//! (dma-management construct, PLAN T15). A CODEGEN-SHAPE (emitted-C structure)
//! check on the streaming lifecycle, using the lightweight whole-TU
//! `emit_generic_c` path (no platform.toml / no six-stage run).
//!
//! It witnesses M4/T15:
//!   * `dma::stream(buf, flow) -> Result<DmaStreamBuffer>` lowers to a
//!     `regseq_dma_stream(<cpu>, <len>, <dir>)` map call capturing the synthetic
//!     device addr into a body-level `__dma_stream_<n>_dev` triple;
//!   * the buffer methods `acquire_from_device()` / `release_to_device()` lower to
//!     `regseq_dma_acquire_from_device(dev, len)` / `regseq_dma_release_to_device(dev,
//!     len)`, reading the SAME mapped buffer's triple;
//!   * the RAII unmap fires at scope exit — realized as a deterministic
//!     `regseq_dma_unmap(dev, len, dir)` at the function's normal `return` (the
//!     regseq IR carries no `Drop` terminator, so RAII = function-exit cleanup);
//!   * the full lifecycle emits in order: map → acquire → release → unmap.
//!
//! generic-c only, Tier-1, candidate-prototype. The honest claim is "the surface
//! EXPRESSES RAII unmap; generic-c lowers it as deterministic scope-exit cleanup"
//! (NOT behavioral Drop-timing fidelity).

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::PathBuf;

use regseq_backends::emit_generic_c;
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

/// A temp `.rs` file removed on drop (mirrors the channel gc-shape test).
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
            .join(format!("ccr-dma-t15-{label}-{}-{nanos}.rs", std::process::id()));
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

/// A streaming driver: a coherent `DmaRegion<u8>` (so the driver has a DMA
/// resource and the host DMA model + helpers emit) plus an `on_init` that maps a
/// CPU buffer with `dma::stream`, runs the `acquire_from_device` / `release_to_device`
/// ownership transitions, and lets the `DmaStreamBuffer` drop at scope end (RAII
/// unmap). The `dma::stream` `Result` is consumed with `?` (the T17 fallibility lint).
fn streaming_source() -> String {
    format!(
        "#![feature(register_tool)]\n#![register_tool(rseq)]\n\
         {LIFECYCLE_SURFACE}\n\
         pub struct DmaRegion<T> {{ _t: core::marker::PhantomData<T> }}\n\
         impl<T> DmaRegion<T> {{ pub const fn new() -> Self {{ Self {{ _t: core::marker::PhantomData }} }} }}\n\
         pub struct DmaStreamBuffer {{ _addr: u64 }}\n\
         impl DmaStreamBuffer {{\n\
           #[inline(never)] #[rseq::intrinsic(\"dma_device_addr\")]\n\
           pub fn device_addr(&self) -> u64 {{ loop {{}} }}\n\
           #[inline(never)] #[rseq::intrinsic(\"dma_acquire_from_device\")]\n\
           pub fn acquire_from_device(&self) {{}}\n\
           #[inline(never)] #[rseq::intrinsic(\"dma_release_to_device\")]\n\
           pub fn release_to_device(&self) {{}}\n\
         }}\n\
         pub mod dma {{\n\
           use super::{{DmaStreamBuffer, RegseqError}};\n\
           #[inline(never)] #[must_use] #[rseq::intrinsic(\"dma_stream\")]\n\
           pub fn stream(_buf: *mut u8, _flow: u32) -> Result<DmaStreamBuffer, RegseqError> {{ loop {{}} }}\n\
         }}\n\
         #[rseq::driver(name = \"strdma\", compatible = [\"acme,strdma\"], bus = \"platform\")]\n\
         pub struct StrDma {{\n\
           #[rseq::dma_region(id = \"buf\", element_size = 1, element_count = 64)]\n\
           buf: DmaRegion<u8>,\n\
         }}\n\
         impl Lifecycle for StrDma {{\n\
           fn on_init(&mut self) -> Result<(), RegseqError> {{\n\
             let mut backing: u8 = 0;\n\
             let sb = dma::stream(&mut backing as *mut u8, 1u32)?;\n\
             sb.release_to_device();\n\
             sb.acquire_from_device();\n\
             let _a = sb.device_addr();\n\
             Ok(())\n\
           }}\n\
         }}\n\
         pub fn make() -> StrDma {{ loop {{}} }}\npub fn entry() {{}}\n"
    )
}

#[test]
fn streaming_buffer_map_acquire_release_unmap_gc_shape() {
    let snippet = TempSnippet::new("stream", &streaming_source());
    let model = match try_bridge_fixture_path(&snippet.path) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("the streaming source must bridge to Ok, got {other:?}"),
    };
    let tu = emit_generic_c(&model).expect("emit must return Ok for the M4 streaming body");

    if std::env::var_os("DMA_DUMP_TU").is_some() {
        eprintln!("=== EMITTED STREAMING TU ===\n{tu}\n=== END TU ===");
    }

    let find = |needle: &str| -> usize {
        tu.find(needle)
            .unwrap_or_else(|| panic!("emitted C must contain {needle:?}; got:\n{tu}"))
    };

    // The map: `dma::stream` lowers to `regseq_dma_stream(<cpu>, <len>, <dir>)`
    // capturing the dev addr into the body-level triple. dir = the flow (1).
    let map = find("regseq_dma_stream(");
    assert!(
        tu.contains("__dma_stream_0_dev = regseq_dma_stream("),
        "dma::stream must capture the synthetic dev addr into the body-level triple; got:\n{tu}"
    );

    // The ownership transitions read the mapped buffer's tracked triple.
    let release = find("regseq_dma_release_to_device(__dma_stream_0_dev, __dma_stream_0_len)");
    let acquire = find("regseq_dma_acquire_from_device(__dma_stream_0_dev, __dma_stream_0_len)");

    // The RAII unmap fires at scope exit — a deterministic function-exit cleanup
    // (the IR has no Drop terminator). It reads the same triple (dev, len, dir).
    let unmap = find("regseq_dma_unmap(__dma_stream_0_dev, __dma_stream_0_len, __dma_stream_0_dir)");

    // The streaming lifecycle order (author source: map → release → acquire →
    // device_addr; the unmap is LAST, at scope exit): map < release < acquire < unmap.
    assert!(
        map < release && release < acquire && acquire < unmap,
        "streaming lifecycle must emit in order map ({map}) → release ({release}) → acquire \
         ({acquire}) → unmap ({unmap})"
    );

    // The host DMA model (incl. the streaming hooks) is emitted for this driver.
    assert!(
        tu.contains("static uint64_t regseq_dma_stream(")
            && tu.contains("static void regseq_dma_unmap("),
        "the host DMA model's streaming hooks must be present in the TU"
    );

    // `device_addr()` on the buffer reads the tracked dev addr (the SAME triple as
    // the map/acquire/release — not a region device_addr).
    assert!(
        tu.contains("= (uint64_t)__dma_stream_0_dev;")
            || tu.contains("__dma_stream_0_dev;"),
        "buf.device_addr() must read the mapped buffer's tracked dev addr"
    );
}
