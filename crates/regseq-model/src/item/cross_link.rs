//! Cross-link resolution table between canonical paths and item identifiers.
//!
//! `CrossLinkTable` is built once during hierarchy construction and used
//! for deterministic cross-layer lookups between DeclarationModel entities
//! and Item Hierarchy items, at both item and field granularity.
//!
//! All lookups use `CanonicalPath`. No short-name matching.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::identity::{CanonicalPath, ItemId};

/// Cross-reference resolution table for the item hierarchy.
///
/// Maps canonical paths to item-level and field-level targets.
/// Built once during hierarchy construction; used for all cross-references
/// during `DriverLoweringPlan` construction and semantic-body lowering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossLinkTable {
    /// Item-level resolution (structs, enums, functions, impls, constants).
    path_to_item: BTreeMap<CanonicalPath, ItemId>,
    /// Field-level resolution (struct fields, enum variant fields).
    /// Maps field canonical path to (owning struct/enum `ItemId`, field index).
    path_to_field: BTreeMap<CanonicalPath, (ItemId, usize)>,
}

impl CrossLinkTable {
    /// Create an empty cross-link table.
    pub fn new() -> Self {
        Self {
            path_to_item: BTreeMap::new(),
            path_to_field: BTreeMap::new(),
        }
    }

    /// Resolve an item-level canonical path to its `ItemId`.
    pub fn resolve_item(&self, path: &CanonicalPath) -> Option<ItemId> {
        self.path_to_item.get(path).copied()
    }

    /// Resolve a field-level canonical path to its owning `ItemId` and field index.
    pub fn resolve_field(&self, path: &CanonicalPath) -> Option<(ItemId, usize)> {
        self.path_to_field.get(path).copied()
    }

    /// Insert an item-level canonical path mapping.
    pub fn insert_item(&mut self, path: CanonicalPath, id: ItemId) {
        self.path_to_item.insert(path, id);
    }

    /// Insert a field-level canonical path mapping.
    pub fn insert_field(&mut self, path: CanonicalPath, owner: ItemId, index: usize) {
        self.path_to_field.insert(path, (owner, index));
    }

    /// Return the number of item-level entries.
    pub fn item_count(&self) -> usize {
        self.path_to_item.len()
    }

    /// Return the number of field-level entries.
    pub fn field_count(&self) -> usize {
        self.path_to_field.len()
    }
}

impl Default for CrossLinkTable {
    fn default() -> Self {
        Self::new()
    }
}
