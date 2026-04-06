//! Impl block definition and trait reference for the item hierarchy.

use serde::{Deserialize, Serialize};

use crate::declaration::Provenance;

use super::identity::{CanonicalPath, ItemId};
use super::type_ref::TypeRef;

/// An impl block grouping methods on a type.
///
/// Impl is a first-class item for provenance and grouping. Methods within
/// an impl are also reachable as standalone `FunctionDef` items (the impl
/// records ownership, not exclusive containment).
///
/// Associated constants follow the same pattern: they are `ConstantDef`
/// items reachable by `ItemId` through `constants`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImplDef {
    pub id: ItemId,
    pub canonical_path: CanonicalPath,
    /// The type this impl block is for.
    pub self_type: TypeRef,
    /// Trait being implemented, if any. `None` for inherent impls.
    pub trait_ref: Option<TraitRef>,
    /// Method item IDs belonging to this impl.
    pub methods: Vec<ItemId>,
    /// Associated constant item IDs belonging to this impl.
    pub constants: Vec<ItemId>,
    pub provenance: Provenance,
}

/// Reference to a trait in a trait impl.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraitRef {
    /// Short trait name (e.g. "Serial", "Lifecycle").
    pub name: String,
    /// Full path for provenance (e.g. "regseq::serial::Serial").
    pub path: String,
}
