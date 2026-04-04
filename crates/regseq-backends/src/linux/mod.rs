//! Linux kernel C code generation backend.
//!
//! Contains the Linux-specific `LinuxBackendPlan` and planner that converts
//! a portable `DriverLoweringPlan` into Linux realization decisions, plus
//! the emitter that produces the generated artifact set.

pub mod c_ast;
pub mod emitter;
pub mod plan;
pub mod planner;
