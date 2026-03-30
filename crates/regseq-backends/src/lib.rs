//! C code generation backend for the Regseq compiler.
//!
//! Provides a backend trait, a backend registry, and per-OS modules for
//! Linux and Zephyr target emission. Linux and Zephyr are separate modules
//! within this crate.

pub mod linux;
pub mod zephyr;
