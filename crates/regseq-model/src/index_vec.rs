//! Dense storage indexed by a typed ID.
//!
//! Mirrors rustc's `IndexVec`: a `Vec<T>` whose indexing uses a
//! typed `Idx` rather than `usize`. `push` returns the assigned ID
//! so callers receive a stable handle without separately tracking
//! the next index.
//!
//! Per the stage-1 spec, the JSON wire shape is a flat array of
//! `T` -- the phantom type parameter is invisible. The
//! `#[serde(transparent)]` attribute on the struct, combined with
//! `#[serde(skip)]` on the `_marker` field, gives that shape via
//! the standard derive.

use core::marker::PhantomData;
use core::ops::{Index, IndexMut};

use serde::{Deserialize, Serialize};

use crate::idx::Idx;

/// Dense `Vec<T>` indexed by a typed ID `I`.
///
/// `PhantomData<fn(&I)>` matches rustc's choice for the same type:
/// it carries the type parameter without affecting drop semantics
/// or auto-trait inference (function pointers are unconditionally
/// `Send + Sync`), and makes the marker contravariant in `I`
/// (variance is moot here because `Idx` requires `'static`).
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct IndexVec<I, T> {
    inner: Vec<T>,
    #[serde(skip)]
    _marker: PhantomData<fn(&I)>,
}

impl<I, T> IndexVec<I, T> {
    /// Returns an empty `IndexVec`. `const`-callable.
    pub const fn new() -> Self {
        Self {
            inner: Vec::new(),
            _marker: PhantomData,
        }
    }

    /// Number of elements stored.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// `true` iff `len() == 0`.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Iterator over `&T` in index order.
    pub fn iter(&self) -> core::slice::Iter<'_, T> {
        self.inner.iter()
    }

    /// Iterator over `&mut T` in index order.
    pub fn iter_mut(&mut self) -> core::slice::IterMut<'_, T> {
        self.inner.iter_mut()
    }
}

impl<I: Idx, T> IndexVec<I, T> {
    /// Append `value`; return the ID assigned to it.
    ///
    /// Panics if the assigned index (the current length, before
    /// the push) cannot be represented by `I` -- i.e., if
    /// `len() > I::MAX_AS_U32`. The check happens via `I::new`
    /// before the underlying `Vec::push`, so on panic the storage
    /// is not modified.
    pub fn push(&mut self, value: T) -> I {
        let idx = I::new(self.inner.len());
        self.inner.push(value);
        idx
    }
}

impl<I: Idx, T> Index<I> for IndexVec<I, T> {
    type Output = T;
    fn index(&self, id: I) -> &T {
        &self.inner[id.index()]
    }
}

impl<I: Idx, T> IndexMut<I> for IndexVec<I, T> {
    fn index_mut(&mut self, id: I) -> &mut T {
        &mut self.inner[id.index()]
    }
}

impl<I, T> Default for IndexVec<I, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<I, T: core::fmt::Debug> core::fmt::Debug for IndexVec<I, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.inner.fmt(f)
    }
}
