//! Item Hierarchy: Layer 2 item-indexed logical source structure.
//!
//! This module defines the Regseq item hierarchy -- the rustc-independent
//! internal model for logical item containment, structural ownership, and
//! canonical cross-reference identity.
//!
//! The item hierarchy coexists with `DeclarationModel` (Layer 1) as a
//! complementary view: construct-family-indexed (obligations) vs
//! item-indexed (structure/semantics). Both are bridged by `CanonicalPath`.
//!
//! Semantic bodies (`SemanticBody`, `Stmt`, `Expr`) are NOT defined here.
//! `FunctionDef` is header-only until the semantic body lowering task.

pub mod attributes;
pub mod constant;
pub mod cross_link;
pub mod function;
pub mod identity;
pub mod impl_def;
pub mod interrupt;
pub mod module;
pub mod type_ref;
pub mod types;

// Re-export core types for convenience.
pub use attributes::{FieldAttribute, ItemAttribute};
pub use constant::{ConstValue, ConstantDef};
pub use cross_link::CrossLinkTable;
pub use function::{
    DeviceCallbackRole, DeviceClassKind, FunctionDef, FunctionKind, FunctionRole, LifecycleRole,
};
pub use identity::{CanonicalPath, ItemId, ItemStore};
pub use impl_def::{ImplDef, TraitRef};
pub use interrupt::InterruptClassDef;
pub use module::Module;
pub use type_ref::{PrimitiveType, TypeRef};
pub use types::{EnumDef, EnumVariantDef, Item, StructDef, StructFieldDef, VariantFieldDef};
