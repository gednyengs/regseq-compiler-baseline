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
//!
//! The zephyr_c backend (F-1 independent lowering; `zephyr_c` module) emits
//! an idiomatic Zephyr driver TU + generated DT binding yaml. Its
//! module-local error type is re-exported as `ZephyrEmitError` (same naming
//! convention as the linux family).
//!
//! The `capability` module owns the ratified per-OS capability table
//! (os-capability-disposition unit): the closed disposition enum, the row
//! consts the OS backends cite, and the per-target `ConformanceReport`
//! every OS-backend emit carries.
//!
//! The `match_audit` module owns the mixed-match-set audit
//! (spec-convergence-pl011 D01/D08): the `MatchAudit` struct the report's
//! `ignored_match_entries` section carries, kept structurally separate from
//! the capability table (whose domain stays lifecycle slots only).

pub mod capability;
mod generic_c;
mod linux_c;
pub mod match_audit;
mod zephyr_c;

pub use capability::{ConformanceReport, SlotEntry};
pub use match_audit::MatchAudit;

pub use linux_c::{emit_linux_c, EmitError as LinuxEmitError, LinuxEmit};

pub use zephyr_c::{emit_zephyr_c, EmitError as ZephyrEmitError, ZephyrEmit};

pub use generic_c::{
    emit_generic_c, emit_generic_c_for_entry, emit_generic_c_for_entry_with,
    emit_generic_c_for_entry_with_context, emit_generic_c_with, emit_generic_c_with_context,
    entry_c_symbol, full_helper_prelude, generate_main_c, BackendContext, BackendOptions,
    EmitError, EntryEmit, EntryShim, EntrySymbol, LifecycleScenario,
};
