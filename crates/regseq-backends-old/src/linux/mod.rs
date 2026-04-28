//! Linux kernel C code generation backend.
//!
//! During the RegseqModel-only rebuild, this module contains the
//! target-native support pieces that still have a direct role:
//! `body_index` maps `FunctionId` to `ir::Body` over a `RegseqModel`,
//! and `conventions` holds Linux kernel C idioms. Driver emission is
//! intentionally not wired into the live compiler path yet.

pub mod body_index;
pub mod conventions;
