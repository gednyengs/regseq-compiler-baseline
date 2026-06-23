//! The dma-management M2 PL011-style slave-`DmaChannel` transport gc-shape test
//! (dma-management construct, PLAN T14). A CODEGEN-SHAPE (emitted-C structure)
//! check on the channel transport ops + the compiler-managed channel lifecycle,
//! using the lightweight whole-TU `emit_generic_c` path (no platform.toml / no
//! six-stage run — the M3 PL011 GOLDEN fixture, T26, is the full-run leg).
//!
//! It witnesses M2/T14:
//!   * the compiler-managed `request_channel` PREAMBLE fires (after alloc_coherent,
//!     in the orderly lifecycle), with the channel id + declared direction;
//!   * the 5 transport ops lower to the existing global-singleton host hooks in
//!     author order: configure → stage_transfer → start_transfer → remaining_bytes
//!     → stop (each keyed by the channel id; stage carries the direction);
//!   * the compiler-managed `release_channel` POSTAMBLE fires at teardown (LIFO).
//!
//! M2 BOUNDARY (deliberate): `stage_transfer`'s endpoint is a `DmaStreamBuffer`,
//! but streaming EMIT (incl. the stream device-addr/len materialization) is M4
//! (T15), so the `addr`/`len` args are `0` placeholders this cycle — the channel
//! id + direction are the load-bearing M2 facts asserted here. The
//! `DmaStreamBuffer` is constructed via a plain (non-intrinsic) `new()` so the
//! body needs no stream-map emit.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::PathBuf;

use regseq_backends::emit_generic_c;
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};

/// A temp `.rs` file removed on drop (mirrors `generic_c_bodies`).
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
            .join(format!("ccr-dma-t14-{label}-{}-{nanos}.rs", std::process::id()));
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

/// A PL011-style slave-channel driver: a `DmaChannel` field (device_to_host RX)
/// whose `on_init` exercises all 5 transport ops in order, consuming every
/// fallible Result with `?` (the T17 fallibility lint requires it). The
/// `DmaStreamBuffer` endpoint is built via a plain `new()` (no stream-map emit).
fn channel_source() -> String {
    format!(
        "#![feature(register_tool)]\n#![register_tool(rseq)]\n\
         {LIFECYCLE_SURFACE}\n\
         pub struct DmaStreamBuffer {{ _addr: u64 }}\n\
         impl DmaStreamBuffer {{ pub const fn new() -> Self {{ Self {{ _addr: 0 }} }} }}\n\
         pub struct DmaChannel {{ _id: u32 }}\n\
         impl DmaChannel {{\n\
           pub const fn new() -> Self {{ Self {{ _id: 0 }} }}\n\
           #[inline(never)] #[rseq::intrinsic(\"dma_configure\")]\n\
           pub fn configure(&self) {{}}\n\
           #[inline(never)] #[must_use] #[rseq::intrinsic(\"dma_stage_transfer\")]\n\
           pub fn stage_transfer(&self, _e: &DmaStreamBuffer) -> Result<(), RegseqError> {{ loop {{}} }}\n\
           #[inline(never)] #[must_use] #[rseq::intrinsic(\"dma_start_transfer\")]\n\
           pub fn start_transfer(&self) -> Result<(), RegseqError> {{ loop {{}} }}\n\
           #[inline(never)] #[rseq::intrinsic(\"dma_stop\")]\n\
           pub fn stop(&self) {{}}\n\
           #[inline(never)] #[must_use] #[rseq::intrinsic(\"dma_remaining_bytes\")]\n\
           pub fn remaining_bytes(&self) -> Result<usize, RegseqError> {{ loop {{}} }}\n\
         }}\n\
         #[rseq::driver(name = \"pl011dma\", compatible = [\"arm,pl011\"], bus = \"platform\")]\n\
         pub struct Pl011Dma {{\n\
           #[rseq::dma_channel(id = \"rx\", direction = \"device_to_host\")]\n\
           rx: DmaChannel,\n\
         }}\n\
         impl Lifecycle for Pl011Dma {{\n\
           fn on_init(&mut self) -> Result<(), RegseqError> {{\n\
             let buf = DmaStreamBuffer::new();\n\
             self.rx.configure();\n\
             self.rx.stage_transfer(&buf)?;\n\
             self.rx.start_transfer()?;\n\
             let _n = self.rx.remaining_bytes()?;\n\
             self.rx.stop();\n\
             Ok(())\n\
           }}\n\
         }}\n\
         pub fn make() -> Pl011Dma {{ loop {{}} }}\npub fn entry() {{}}\n"
    )
}

#[test]
fn pl011_channel_transport_and_lifecycle_gc_shape() {
    let snippet = TempSnippet::new("channel", &channel_source());
    let model = match try_bridge_fixture_path(&snippet.path) {
        BridgeOutcome::Ok(model) => model,
        other => panic!("the PL011 channel source must bridge to Ok, got {other:?}"),
    };
    let tu = emit_generic_c(&model).expect("emit must return Ok for the M2 channel body");

    if std::env::var_os("DMA_DUMP_TU").is_some() {
        eprintln!("=== EMITTED CHANNEL TU ===\n{tu}\n=== END TU ===");
    }

    let find = |needle: &str| -> usize {
        tu.find(needle)
            .unwrap_or_else(|| panic!("emitted C must contain {needle:?}; got:\n{tu}"))
    };

    // ----------------------------------------------------------------------
    // Compiler-managed channel lifecycle: the request_channel preamble (id + dir)
    // and the release_channel postamble both fire. (They are synthesized into the
    // scenario `main`, a DIFFERENT TU section from the `on_init` body — so the
    // lifecycle-vs-transport ordering is asserted WITHIN the orderly scenario
    // block below, not by TU-wide string position.) device_to_host = discriminant 0.
    // ----------------------------------------------------------------------
    let request = find("regseq_dma_request_channel(\"rx\", 0)");
    let release = find("regseq_dma_release_channel(\"rx\")");
    // LIFO: within the (whole-TU) synthesized harness the request precedes the
    // release of the same channel (request in the preamble, release at teardown).
    assert!(
        request < release,
        "request_channel ({request}) must precede release_channel ({release}) (LIFO lifecycle)"
    );

    // ----------------------------------------------------------------------
    // Transport ops, in author order, each keyed by the channel id. These all
    // live in the SAME `on_init` body, so their relative TU positions reflect the
    // author source order.
    // ----------------------------------------------------------------------
    let configure = find("regseq_dma_configure(\"rx\")");
    // stage_transfer carries the channel direction (0); addr/len are M2 0
    // placeholders (stream emit is M4/T15).
    let stage = find("regseq_dma_stage_transfer(\"rx\", 0ull, 0ull, 0)");
    let start = find("regseq_dma_start_transfer(\"rx\")");
    let remaining = find("regseq_dma_remaining_bytes(\"rx\", &");
    let stop = find("regseq_dma_stop(\"rx\")");

    // Author transport order: configure → stage → start → remaining → stop.
    assert!(
        configure < stage && stage < start && start < remaining && remaining < stop,
        "transport ops must emit in author order: configure ({configure}) → stage_transfer \
         ({stage}) → start_transfer ({start}) → remaining_bytes ({remaining}) → stop ({stop})"
    );

    // `stop` is infallible (void) — the others (stage/start/remaining) are the
    // fallible ops whose host hooks return an int status (the bridge fallibility
    // lint, T17, guarantees the author consumes it). Sanity: the request_channel
    // direction is the declared device_to_host (0), distinct from a host_to_device
    // (1) channel — proving the declared direction rode through to emit.
    assert!(
        tu.contains("regseq_dma_request_channel(\"rx\", 0)")
            && !tu.contains("regseq_dma_request_channel(\"rx\", 1)"),
        "the request_channel must carry the DECLARED direction (device_to_host = 0)"
    );

    // ----------------------------------------------------------------------
    // BLOCKING #2 fix — the FALLIBLE channel ops construct a REAL Result, not a
    // cast of the host hook's int rc into the Result struct. Assert the sound
    // shape: an `int __dma_rc = <hook>;` capture + an rc==0 Ok / else Err branch.
    // (The Ok/Err arms write the niche/Direct discriminant + payload — verified by
    // the cc -c compile below, which would fail on the old int→struct cast.)
    // ----------------------------------------------------------------------
    assert!(
        tu.contains("int __dma_rc = regseq_dma_stage_transfer(\"rx\", 0ull, 0ull, 0);"),
        "stage_transfer must capture the host hook int rc into __dma_rc (not cast int→Result)"
    );
    assert!(
        tu.contains("int __dma_rc = regseq_dma_start_transfer(\"rx\");"),
        "start_transfer must capture the host hook int rc into __dma_rc"
    );
    assert!(
        tu.contains("int __dma_rc = regseq_dma_remaining_bytes(\"rx\", &__dma_residue);"),
        "remaining_bytes must capture the int rc into __dma_rc and pass the residue out-param"
    );
    // The rc==0 Ok-branch + the dead-but-valid Err-branch are both present.
    assert!(
        tu.contains("if (__dma_rc == 0)") && tu.contains("dma channel fallible Err branch"),
        "the fallible ops must branch on rc==0 (Ok) else construct an Err (real Result)"
    );
    // remaining_bytes writes the residue as the Ok payload (not discarded).
    assert!(
        tu.contains("(size_t)(__dma_residue)"),
        "remaining_bytes must write the residue out-param as the Ok(usize) payload"
    );

    // ----------------------------------------------------------------------
    // Ad-hoc soundness check (coordinator-requested): the emitted channel TU must
    // COMPILE with the system C compiler (`cc -c`). This is NOT a full PL011 golden
    // (that is M3/T26) — just a compile sanity check that the Result construction
    // is valid C (the old int→struct cast would fail to compile / be UB). Skipped
    // gracefully if `cc` is unavailable.
    // ----------------------------------------------------------------------
    let c_path = std::env::temp_dir().join(format!(
        "ccr-dma-t14-compile-{}-{}.c",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::write(&c_path, &tu).expect("write channel TU for cc -c");
    let out = std::process::Command::new("cc")
        .arg("-c")
        .arg("-o")
        .arg("/dev/null")
        .arg(&c_path)
        .output();
    let _ = std::fs::remove_file(&c_path);
    match out {
        Ok(o) => assert!(
            o.status.success(),
            "the emitted channel TU must compile with `cc -c` (BLOCKING #2: real Result, not \
             int→struct cast). cc stderr:\n{}",
            String::from_utf8_lossy(&o.stderr)
        ),
        Err(e) => eprintln!("note: `cc` unavailable, skipping the ad-hoc compile check ({e})"),
    }
}
