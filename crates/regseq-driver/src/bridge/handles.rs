//! Phase-2 opaque item handles and final `ItemId` remapping.

use std::collections::HashMap;

use regseq_model::ItemId;

/// Opaque handle assigned before final F4-deterministic `ItemId`s exist.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ItemHandle(u32);

impl ItemHandle {
    /// Returns the raw handle index for deterministic tests and diagnostics.
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

/// Phase-local table that records `ItemHandle -> ItemId` after Phase 4.
#[derive(Clone, Debug, Default)]
pub struct HandleTable {
    next_handle: u32,
    /// Final mapping populated during Phase 4 assignment.
    pub handle_to_item_id: HashMap<ItemHandle, ItemId>,
}

impl HandleTable {
    /// Creates an empty handle table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocates a fresh Phase-2 item handle.
    pub fn fresh_handle(&mut self) -> ItemHandle {
        let handle = ItemHandle(self.next_handle);
        self.next_handle = self
            .next_handle
            .checked_add(1)
            .expect("bridge item handle space exhausted");
        handle
    }

    /// Records the final `ItemId` assigned to a Phase-2 handle.
    pub fn bind_final_item_id(&mut self, handle: ItemHandle, item_id: ItemId) {
        self.handle_to_item_id.insert(handle, item_id);
    }

    /// Resolves a Phase-2 handle to its final `ItemId`, when assigned.
    pub fn resolve(&self, handle: ItemHandle) -> Option<ItemId> {
        self.handle_to_item_id.get(&handle).copied()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn handle_table_resolves_unique_dense_item_ids() {
        let mut table = HandleTable::new();
        let handles = (0..4).map(|_| table.fresh_handle()).collect::<Vec<_>>();

        for (idx, handle) in handles.iter().copied().enumerate() {
            table.bind_final_item_id(handle, ItemId::from_u32(idx as u32));
        }

        let resolved = handles
            .iter()
            .map(|handle| table.resolve(*handle).expect("handle should resolve"))
            .collect::<Vec<_>>();
        let unique = resolved.iter().copied().collect::<HashSet<_>>();

        assert_eq!(unique.len(), resolved.len());
        assert_eq!(
            resolved,
            vec![
                ItemId::from_u32(0),
                ItemId::from_u32(1),
                ItemId::from_u32(2),
                ItemId::from_u32(3),
            ],
        );
    }
}
