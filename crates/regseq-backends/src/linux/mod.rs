//! Linux kernel C code generation backend.
//!
//! Contains the Linux-specific `LinuxBackendPlan` and planner that converts
//! a portable `DriverLoweringPlan` into Linux realization decisions, plus
//! the emitter that produces the generated artifact set.

pub mod body_index;
pub mod conventions;
mod emit_module;
mod emit_regs;
pub(crate) mod emit_semantic;
pub mod emitter;
pub mod plan;
pub mod planner;
