//! Shared compiler configuration types for the Regseq toolchain.
//!
//! This crate is intentionally rustc-independent so configuration and model
//! types can be reused across `cargo-regseq`, drivers, analyses, and backends.

pub mod emit;
pub mod options;
pub mod param;
pub mod types;

pub use emit::{EmitParseError, EmitSpec};
pub use options::CompilerOptions;
pub use param::{validate_dotted_ident, ParamParseError, ParamScope, RegseqParam};
pub use types::{ColorChoice, DumpStage, LogLevel, ParseEnumError, TargetOs};
