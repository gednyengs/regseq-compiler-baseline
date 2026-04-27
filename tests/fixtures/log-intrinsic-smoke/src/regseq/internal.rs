//! Internal compiler marker functions.
//!
//! These are NOT `#[rseq::intrinsic]` operations. They are plain functions
//! recognized by DefId during IR construction. They exist solely so that
//! paired `ordered_region_begin()`/`ordered_region_end()` calls appear as
//! `Call` terminators in MIR, enabling OrderedRegion detection in Phase 5.

/// Compiler marker: beginning of an ordered region.
/// Not a real intrinsic -- exists only for IR constructor detection.
#[inline(never)]
pub fn ordered_region_begin() {}

/// Compiler marker: end of an ordered region.
/// Not a real intrinsic -- exists only for IR constructor detection.
#[inline(never)]
pub fn ordered_region_end() {}
