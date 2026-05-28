//! Regseq codegen backends (Linux, Zephyr). Generic-C backend entry point.
//!
//! The generic-C backend lowers a post-mono [`regseq_model::RegseqModel`] to
//! a single C translation unit in `generic-c-backend.md` section 15 five-bank
//! order. The implementation lives in the [`generic_c`] module; the public
//! surface (`emit_generic_c` + `EmitError`) is re-exported here so the
//! harness import path (`regseq_backends::emit_generic_c`) is unchanged.

mod generic_c;

pub use generic_c::{emit_generic_c, entry_c_symbol, EmitError, EntrySymbol};
