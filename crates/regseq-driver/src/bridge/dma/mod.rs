//! DMA-management construct bridge (M1 coherent-region path).
//!
//! Recognizes the authored coherent-region DMA surface and interns it into
//! `RegseqModel.dma` for the L2 `dma` tenant to join and the backends' DMA
//! lifecycle glue + per-op lowering to consume. Two passes (clock/pinctrl field
//! recognition + atomic body-walk precedent):
//!
//!   * [`materialise_dma_regions`] (Phase 4, after
//!     `device_lifecycle::materialise_drivers`) — `#[rseq::dma_region(id,
//!     element_size, element_count, alignment?)]` field recognition of the
//!     required `DmaRegion<T>` form (the optional/array/ref forms are
//!     recognized-and-rejected), the mandatory-key checks, the unsupported-form +
//!     duplicate-id rejects, the captured element type `T`, and the `dma_present`
//!     bus-master gate (GRILL T05/T06).
//!
//!   * [`recognise_dma_region_sites`] (Phase 5, after body materialisation) —
//!     the coherent-region `element_access` closure-borrow call-sites, recovered
//!     by RECEIVER RESOURCE TYPE (the `DmaRegion` field place) so the generic
//!     `read`/`write`/`update` verbs are split from `Mmio<T>`'s, with each site's
//!     verb / view type `V` / index / lexical extent recorded as an
//!     `ElementAccessFact` (GRILL T06/T08).
//!
//! The coherent-region ops are author intrinsics (tagged
//! `#[rseq::intrinsic("dma_*")]`, recognized by the shared intrinsic pass +
//! `materialise_intrinsics`); the compiler-managed `alloc_coherent`/`free_coherent`
//! lifecycle ops are synthesized from the region *field* in the backend preamble,
//! not author-called. The slave `DmaChannel` + streaming recognition is M2 (T07).

mod layout;
mod recognition;

pub(crate) use layout::verify_dma_layout;
pub(crate) use recognition::{
    is_dma_channel_field, is_dma_region_field, materialise_dma_completions, materialise_dma_regions,
    recognise_dma_region_sites, recognise_dma_streams_and_fallibility,
    validate_stage_transfer_endpoints,
};
