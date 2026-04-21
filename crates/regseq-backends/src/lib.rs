//! C code generation backend for the Regseq compiler.
//!
//! Provides a backend trait, a backend registry, and per-OS modules for
//! Linux and Zephyr target emission. Linux and Zephyr are separate modules
//! within this crate.
//!
//! The `lowering` module provides the portable `DriverLoweringPlan` and
//! planner entry point that all OS-specific backends consume.

pub mod c;
pub mod linux;
pub mod lowering;
pub mod zephyr;
