//! Identity types for the RegseqModel.
//!
//! Provides interned string handles ([`Symbol`], [`FileId`]), the
//! [`SymbolInterner`] that backs them, typed declaration IDs
//! ([`DeclId`] and its nine wrappers), body-level IDs
//! ([`LocalId`], [`BasicBlockId`]), and source location types
//! ([`SourceLocation`], [`SourceOrigin`]).

mod decl_id;
mod source;
mod symbol;

pub use decl_id::{
    BasicBlockId, ConstantId, DeclId, FunctionId, LocalId, ModuleId, PropertyId, RegisterBlockId,
    RegisterDescriptorId, ResourceId, TypeDefId, UserDataId,
};
pub use source::{SourceLocation, SourceOrigin};
pub use symbol::{FileId, Symbol, SymbolInterner};
