//! Shared compiler configuration types for the Regseq toolchain.
//!
//! This crate is intentionally rustc-independent so configuration and model
//! types can be reused across `cargo-regseq`, drivers, analyses, and backends.

pub mod emit;
pub mod options;
pub mod param;
pub mod platform;
pub mod toolchain;
pub mod types;

pub use emit::{EmitParseError, EmitSpec};
pub use options::CompilerOptions;
pub use param::{validate_dotted_ident, ParamParseError, ParamScope, RegseqParam};
pub use platform::{
    parse_platform_bindings_path, parse_platform_bindings_str, parse_platform_file_path,
    parse_platform_file_str, InterruptBindingSpec, MmioBindingSpec, PlatformBindings,
    PlatformFile, PlatformIdentityRaw, PlatformParseError,
};
pub use toolchain::CC_FLAGS;
pub use types::{ColorChoice, DumpStage, LogLevel, ParseEnumError};
