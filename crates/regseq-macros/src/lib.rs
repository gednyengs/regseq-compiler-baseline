//! Procedural macro crate for Regseq helper macros.
//!
//! Provides `register_bitfields!` for register layouts and any future helper
//! macros needed by driver specifications.
//!
//! `#[rseq::...]` tool attributes are **not** provided by this crate. They are
//! an inert custom tool-attribute namespace registered by the compiler wrapper
//! (`regseq-driver`), so the attributes remain available to HIR extraction
//! without requiring proc-macro expansion.

use proc_macro::TokenStream;

mod register_bitfields;

/// Declare register bitfield layouts for typed register access.
///
/// Generates per-register modules with marker types, field descriptors, and
/// named value constants compatible with the `regseq` facade register support
/// types (`Field`, `FieldValue`, `ReadOnly`, `ReadWrite`, etc.).
///
/// # Syntax
///
/// ```text
/// register_bitfields! [
///     <width>,
///     REG_NAME [
///         FIELD_NAME OFFSET(<n>) NUMBITS(<n>) [
///             VALUE_NAME = <expr>,
///             ...
///         ]
///     ],
///     ...
/// ];
/// ```
#[proc_macro]
pub fn register_bitfields(input: TokenStream) -> TokenStream {
    let parsed = syn::parse_macro_input!(input as register_bitfields::RegisterBitfields);
    parsed.expand().into()
}
