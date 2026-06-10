//! Regseq codegen backends (Linux, Zephyr). Generic-C backend entry point.
//!
//! The generic-C backend lowers a post-mono [`regseq_model::RegseqModel`] to
//! a single C translation unit in `generic-c-backend.md` section 15 five-bank
//! order. The implementation lives in the `generic_c` module; the public
//! surface (`emit_generic_c` + `EmitError`) is re-exported here so the
//! harness import path (`regseq_backends::emit_generic_c`) is unchanged.
//!
//! The linux_c backend (F-1 independent lowering; `linux_c` module) emits an
//! idiomatic Linux kernel module + Kbuild fragment. Its module-local error
//! type is re-exported as `LinuxEmitError` (the name `EmitError` at the crate
//! root stays the generic-C type for the unchanged harness import path).

mod generic_c;
mod linux_c;

pub use linux_c::{emit_linux_c, EmitError as LinuxEmitError, LinuxEmit};

pub use generic_c::{
    emit_generic_c, emit_generic_c_for_entry, emit_generic_c_for_entry_with,
    emit_generic_c_for_entry_with_context, emit_generic_c_with, emit_generic_c_with_context,
    entry_c_symbol, full_helper_prelude, generate_main_c, BackendContext, BackendOptions, EmitError,
    EntryEmit, EntryShim, EntrySymbol, LifecycleScenario,
};
