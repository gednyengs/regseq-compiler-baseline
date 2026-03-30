//! Shared diagnostics engine for the Regseq compiler.
//!
//! Provides diagnostic severity levels, a diagnostic ID system, structured
//! messages with source spans, and emission support. Used by analysis passes,
//! backends, and other compiler components.
//!
//! This crate has no rustc dependency. It uses its own span representation
//! that can be populated from rustc spans by the driver.
