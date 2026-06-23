#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// The dma-management M3 PL011 slave-channel golden (PLAN T26). A PL011-style
// driver declares a slave `DmaChannel` (RX, `device_to_host`) + a coherent
// `DmaRegion<u8>` RX buffer, then in `on_init`:
//   configure() → stage_transfer(rx_buf.span(0..32)) → start_transfer()
// consuming each fallible `Result` with `?` (the T17 fallibility lint). A
// `#[rseq::dma_completion(channel = "rx")]` handler binds the framework completion
// callback (registration-shape only; the host model NEVER fires it, D06).
//
// This is the FIRST channel golden that COMPILES + RUNS. The trace asserts the
// full M3 channel path in order:
//   alloc_coherent (region) → request_channel → register_completion →
//   configure → stage_transfer (REAL span addr/len) → start_transfer →
//   (teardown) release_channel → free_coherent (LIFO)
//
// The span lowers to the real device address
// `regseq_dma_region_dev_base("rx_buf") + 0*1` and len `32*1 = 32` (M3/T26 —
// replaces the M2 0/0 placeholder). `request_channel`/`release_channel` are
// compiler-emitted lifecycle (NOT author calls). generic-c only, Tier-1,
// candidate-prototype.

include!("../../_lifecycle_surface.rs");

// ---------------------------------------------------------------------------
// DMA-management author surface (dma-management construct, M3 channel + span +
// completion). The typed `DmaRegion<T>` / `DmaChannel` resources (detected by
// type NAME), the A2 + element-access ops, the `span` endpoint + the
// `TransferEndpoint` overload, and the 5-op channel transport — each tagged with
// its namespaced `#[rseq::intrinsic("dma_*")]` facade.
// ---------------------------------------------------------------------------
pub struct DmaRegion<T> {
    _t: core::marker::PhantomData<T>,
}
impl<T> DmaRegion<T> {
    pub const fn new() -> Self {
        Self { _t: core::marker::PhantomData }
    }
    /// The region's DMA-visible base address (A2 addressing).
    #[inline(never)]
    #[rseq::intrinsic("dma_device_addr")]
    pub fn device_addr(&self) -> u64 {
        loop {}
    }
    /// The declared element cardinality (A2 introspection).
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

/// The span endpoint type (M3/T09) + the `TransferEndpoint` overload (C06):
/// `stage_transfer` accepts `&DmaStreamBuffer | DmaRegionSpan`.
pub struct DmaRegionSpan {
    _addr: u64,
    _len: u64,
}
pub trait TransferEndpoint {}
impl TransferEndpoint for DmaRegionSpan {}

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
    /// Stage a transfer of an endpoint (`DmaRegionSpan` here, via the
    /// `TransferEndpoint` overload, C06). Fallible.
    #[inline(never)]
    #[must_use]
    #[rseq::intrinsic("dma_stage_transfer")]
    pub fn stage_transfer(&self, _endpoint: impl TransferEndpoint) -> Result<(), RegseqError> {
        loop {}
    }
    /// Issue/submit a staged transfer. Fallible.
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
}

/// The completion payload type (M3/T09(b), GRILL C05): the registration marker
/// the handler receives. Never constructed/delivered this cycle (D06).
pub struct DmaCompletion {
    pub status: u32,
}

// ---------------------------------------------------------------------------
// The PL011 slave driver: amba-matched, a coherent RX buffer + an RX slave
// channel. A slave channel does NOT set `dma_present` (T05 is region-bus-master
// only), so no `set_master`. The region's alloc_coherent/free_coherent +
// the channel's request_channel/release_channel frame the device lifetime.
// ---------------------------------------------------------------------------
#[rseq::driver(name = "pl011dma", bus = "amba",
    amba = [
    { id = 0x00041011, mask = 0x000fffff },
    ],
    compatible = ["regseq,pl011dma"]
)]
pub struct Pl011Dma {
    #[rseq::dma_channel(id = "rx", direction = "device_to_host")]
    rx: DmaChannel,
    #[rseq::dma_region(id = "rx_buf", element_size = 1, element_count = 64)]
    rx_buf: DmaRegion<u8>,
}

impl Pl011Dma {
    /// The DMA-framework completion callback (M3/T18, D06/C05): bound to the `rx`
    /// channel; registration-shape only — the host model NEVER fires it this
    /// cycle. The body is device-class logic (here a trivial Ok).
    #[rseq::dma_completion(channel = "rx")]
    pub fn rx_complete(&mut self, _result: DmaCompletion) -> Result<(), RegseqError> {
        Ok(())
    }
}

impl Lifecycle for Pl011Dma {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        // Configure the channel (declaration/L2-derived), stage the RX transfer
        // into the first 32 bytes of the coherent buffer (the span endpoint
        // lowers to the real device address + len, M3/T26), then start it.
        // Each fallible Result is consumed with `?` (the T17 fallibility lint).
        self.rx.configure();
        self.rx.stage_transfer(self.rx_buf.span(0..32))?;
        self.rx.start_transfer()?;
        Ok(())
    }

    fn on_deinit(&mut self) -> Result<(), RegseqError> {
        // Quiesce the channel. The compiler-managed release_channel + free_coherent
        // postambles run after this (LIFO), trace-only in the oracle.
        self.rx.stop();
        Ok(())
    }
}

// No `regseq_main`: the generic-c backend synthesizes `main`; the manifest's
// `lifecycle_scenario = "orderly"` drives the run.
