//! Shared compiler configuration types for the Regseq toolchain.
//!
//! This crate is intentionally rustc-independent so configuration and model
//! types can be reused across `cargo-regseq`, drivers, analyses, and backends.

use thiserror::Error;

/// Placeholder error type for `regseq-config`.
#[derive(Debug, Error)]
#[error("regseq-config error")]
pub struct Error;
