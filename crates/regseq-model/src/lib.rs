//! Rustc-independent internal model for the Regseq compiler.
//!
//! Owns declaration models for validated constructs, platform binding models
//! for Layer 2 integration data, joined lowering inputs, and serialization
//! support for debug artifacts.
//!
//! This crate has no rustc dependency. It is the central data-structure crate
//! for the compiler pipeline, consumed by analysis passes, backends, and
//! the driver's join logic.

pub mod binding;
pub mod declaration;
pub mod identity;
pub mod item;
pub mod joined;
pub mod platform;
pub mod platform_parser;
pub mod properties;
pub mod semantic;
