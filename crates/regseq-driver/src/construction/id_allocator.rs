//! Sequential [`DeclId`] allocator used during Pass 1 construction.

use regseq_model::identity::DeclId;

/// Monotonically-increasing allocator for [`DeclId`] values. Pass 1
/// creates exactly one allocator and hands out ids in encounter order so
/// that the resulting `source_locations` `BTreeMap` iterates in a
/// deterministic sequence matching the traversal.
#[derive(Debug)]
pub struct DeclIdAllocator {
    next: u32,
}

impl DeclIdAllocator {
    /// Create a fresh allocator that hands out ids starting at 0.
    pub fn new() -> Self {
        Self { next: 0 }
    }

    /// Reserve the next [`DeclId`].
    pub fn alloc(&mut self) -> DeclId {
        let id = DeclId::new(self.next);
        self.next += 1;
        id
    }
}

impl Default for DeclIdAllocator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocates_monotonically_from_zero() {
        let mut a = DeclIdAllocator::new();
        assert_eq!(a.alloc(), DeclId::new(0));
        assert_eq!(a.alloc(), DeclId::new(1));
        assert_eq!(a.alloc(), DeclId::new(2));
    }
}
