//! Content-unique string interner indexed by `StringId`.
//!
//! Mirrors the rustc interner posture: append-only,
//! amortized-O(1) `intern` and `resolve`, `Arc<str>` shared
//! between `storage` (the index-ordered backing) and `lookup`
//! (the content-keyed dedup table). `StringId::ZERO` is
//! pre-interned to the empty string by `new`, providing a stable
//! sentinel for "no name" callers.
//!
//! Per the stage-1 spec, the JSON wire shape is a flat array of
//! strings -- only `storage`, in order. On deserialize, `lookup`
//! is reconstructed; non-empty index 0 and duplicate entries
//! surface as deserialization errors.

use std::collections::HashMap;
use std::sync::Arc;

use serde::de::Error as DeError;
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::ids::StringId;
use crate::idx::Idx;

#[derive(Clone)]
pub struct StringInterner {
    storage: Vec<Arc<str>>,
    lookup: HashMap<Arc<str>, StringId>,
}

impl StringInterner {
    /// Creates an interner with the empty string pre-interned at
    /// `StringId::ZERO`.
    pub fn new() -> Self {
        let empty: Arc<str> = Arc::from("");
        let mut storage = Vec::with_capacity(1);
        let mut lookup = HashMap::with_capacity(1);
        storage.push(Arc::clone(&empty));
        lookup.insert(empty, StringId::ZERO);
        Self { storage, lookup }
    }

    /// Returns the `StringId` for `s`, allocating and assigning a
    /// fresh ID if `s` is not already interned.
    ///
    /// Panics if assigning a fresh ID would exceed
    /// `StringId::MAX_AS_U32`.
    pub fn intern(&mut self, s: &str) -> StringId {
        if let Some(&id) = self.lookup.get(s) {
            return id;
        }
        let arc: Arc<str> = Arc::from(s);
        let id = <StringId as Idx>::new(self.storage.len());
        self.storage.push(Arc::clone(&arc));
        self.lookup.insert(arc, id);
        id
    }

    /// Returns the string interned at `id`. Panics if `id` is out
    /// of range; under the append-only invariant, valid IDs
    /// produced by this interner remain valid for its lifetime.
    pub fn resolve(&self, id: StringId) -> &str {
        &self.storage[id.index()]
    }

    /// Number of interned strings, including the empty-string sentinel.
    pub fn len(&self) -> usize {
        self.storage.len()
    }

    /// Returns true when the backing storage has no strings.
    ///
    /// A well-formed interner created by `StringInterner::new` is
    /// never empty because it contains the empty-string sentinel.
    pub fn is_empty(&self) -> bool {
        // Under the construction invariants, an interner always
        // contains at least the empty string at index 0; this
        // method is included for parity with collection APIs and
        // returns false for any well-formed interner.
        self.storage.is_empty()
    }

    /// Iterates over interned strings in `StringId` order.
    pub fn iter(&self) -> impl Iterator<Item = (StringId, &str)> + '_ {
        self.storage
            .iter()
            .enumerate()
            .map(|(i, s)| (StringId::from_u32(i as u32), s.as_ref()))
    }
}

impl Default for StringInterner {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for StringInterner {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("StringInterner")
            .field("storage", &self.storage)
            .finish()
    }
}

impl PartialEq for StringInterner {
    /// Equality is content-equality on `storage`. The `lookup` map
    /// is a content-derived index of `storage`, so two interners
    /// with equal `storage` necessarily have equivalent lookup
    /// behavior; comparing the HashMap directly would expose its
    /// non-deterministic iteration order to the equality check.
    fn eq(&self, other: &Self) -> bool {
        self.storage == other.storage
    }
}

impl Eq for StringInterner {}

impl Serialize for StringInterner {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        let mut seq = ser.serialize_seq(Some(self.storage.len()))?;
        for s in &self.storage {
            seq.serialize_element(s.as_ref())?;
        }
        seq.end()
    }
}

impl<'de> Deserialize<'de> for StringInterner {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let entries: Vec<String> = Vec::deserialize(de)?;

        if entries.is_empty() {
            return Err(D::Error::custom(
                "StringInterner is empty (must contain at least the empty string at index 0)",
            ));
        }
        if !entries[0].is_empty() {
            return Err(D::Error::custom(format!(
                "StringInterner index 0 is {:?}, must be the empty string",
                entries[0]
            )));
        }
        if entries.len() as u64 > StringId::MAX_AS_U32 as u64 + 1 {
            return Err(D::Error::custom(format!(
                "StringInterner length {} exceeds StringId::MAX ({})",
                entries.len(),
                StringId::MAX_AS_U32
            )));
        }

        let mut storage = Vec::with_capacity(entries.len());
        let mut lookup = HashMap::with_capacity(entries.len());

        for (i, s) in entries.into_iter().enumerate() {
            let arc: Arc<str> = Arc::from(s);
            if lookup.contains_key(&arc) {
                return Err(D::Error::custom(format!(
                    "StringInterner contains duplicate entry {:?} at index {}",
                    &*arc, i
                )));
            }
            let id = StringId::from_u32(i as u32);
            storage.push(Arc::clone(&arc));
            lookup.insert(arc, id);
        }

        Ok(Self { storage, lookup })
    }
}

#[cfg(test)]
#[path = "string_interner_tests.rs"]
mod tests;
