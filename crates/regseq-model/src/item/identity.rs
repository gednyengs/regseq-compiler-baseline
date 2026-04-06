//! Core identity types for the Layer 2 item hierarchy.
//!
//! `CanonicalPath` is the shared cross-reference identity between
//! `DeclarationModel` (Layer 1) and the Item Hierarchy (Layer 2).
//! `ItemId` is the efficient intra-hierarchy reference.
//! `ItemStore` is the canonical owner of all item payloads.

use serde::{Deserialize, Serialize};

use super::types::Item;

/// Canonical item path derived from rustc DefId during extraction.
///
/// Shared cross-reference identity between DeclarationModel and Item Hierarchy.
/// Fully qualified, stable within a compilation, and unambiguous across modules,
/// impls, and types.
///
/// Normalization rules:
/// - `::` as the segment separator
/// - impl blocks rendered explicitly (`<impl Trait for Type>` or `<impl Type>`)
/// - field paths extend the parent item path with `::field_name`
/// - no backend-specific mangling or stable-name rewriting in the identity itself
///
/// `stable_name` (on `ConstantDef`) is derived from `CanonicalPath`; it is not
/// the identity.
///
/// Examples:
/// - `"pl011_full_spec::driver::Pl011FullSpec"` (driver struct)
/// - `"pl011_full_spec::driver::Pl011FullSpec::init"` (trait method)
/// - `"pl011_full_spec::driver::Pl011FullSpec::DEFAULT_UART_CLOCK_HZ"` (constant)
/// - `"pl011_full_spec::registers::Pl011Registers"` (register block)
/// - `"pl011_full_spec::driver::<impl Serial for Pl011FullSpec>"` (impl)
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CanonicalPath(pub String);

impl CanonicalPath {
    /// Create a new canonical path.
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    /// Return the path as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for CanonicalPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Unique item identifier within a Regseq item hierarchy.
///
/// Resolves through `ItemStore`. Not shared with DeclarationModel;
/// cross-layer identity uses `CanonicalPath`.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ItemId(pub u32);

/// Canonical storage for all item-level entities in the Item Hierarchy.
///
/// Items are addressed by `ItemId`. `Module.items`, `ImplDef.methods`, and
/// `ImplDef.constants` all reference into this store. The store is the
/// single owner of item payloads -- items are never duplicated across
/// multiple containers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemStore {
    items: Vec<Item>,
}

impl ItemStore {
    /// Create an empty item store.
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Look up an item by its identifier.
    pub fn get(&self, id: ItemId) -> Option<&Item> {
        self.items.get(id.0 as usize)
    }

    /// Insert an item and return its assigned identifier.
    pub fn push(&mut self, item: Item) -> ItemId {
        let id = ItemId(self.items.len() as u32);
        self.items.push(item);
        id
    }

    /// Return the number of items in the store.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Return whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl Default for ItemStore {
    fn default() -> Self {
        Self::new()
    }
}
