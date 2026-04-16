//! Unified RegseqModel types.
//!
//! This module hosts the rustc-independent data model that consolidates
//! declaration, binding, and lowering facts produced by the extraction
//! pipeline. Types are added incrementally as Phase 2 of the regseq-model
//! execution plan progresses.

pub mod constant;
pub mod framework;
pub mod register;
pub mod type_def;

pub use constant::Constant;
pub use framework::{FrameworkTypeKind, TypeDefClassification};
pub use register::{
    AccessKind, BitfieldDef, NamedValue, RegisterBlock, RegisterBlockAttributes,
    RegisterBlockField, RegisterDescriptor,
};
pub use type_def::{AttrEntry, AttrValue, Attrs, FieldDef, TypeDef, TypeDefKind, VariantDef};
