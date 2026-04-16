//! Identity types for the RegseqModel.
//!
//! Provides interned string handles ([`Symbol`], [`FileId`]) and the
//! [`SymbolInterner`] that backs them.

mod symbol;

pub use symbol::{FileId, Symbol, SymbolInterner};
