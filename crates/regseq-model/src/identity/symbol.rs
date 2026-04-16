//! Symbol interning for O(1) string equality and compact storage.
//!
//! All identifiers and file paths in the RegseqModel are interned through
//! a single [`SymbolInterner`]. References use [`Symbol`] (for identifiers)
//! or [`FileId`] (for file paths) -- both are 4-byte `Copy` handles with
//! O(1) equality.

use std::collections::HashMap;

/// An interned string identifier. Copy, Eq, Hash -- all O(1).
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Symbol(u32);

/// An interned file path. Distinct newtype over the same pool.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct FileId(u32);

/// The string intern table.
///
/// Owns a single pool of strings shared by both [`Symbol`] and [`FileId`].
/// The distinct newtypes prevent accidental mixing at call sites.
pub struct SymbolInterner {
    strings: Vec<String>,
    lookup: HashMap<String, u32>,
}

impl std::fmt::Debug for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Symbol({})", self.0)
    }
}

impl std::fmt::Debug for FileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FileId({})", self.0)
    }
}

impl SymbolInterner {
    /// Create an empty interner with no entries.
    pub fn new() -> Self {
        Self {
            strings: Vec::new(),
            lookup: HashMap::new(),
        }
    }

    /// Intern a string, returning its index in the shared pool.
    fn intern_raw(&mut self, s: &str) -> u32 {
        if let Some(&idx) = self.lookup.get(s) {
            return idx;
        }
        let idx: u32 = self
            .strings
            .len()
            .try_into()
            .expect("symbol interner overflow: more than u32::MAX interned strings");
        self.strings.push(s.to_owned());
        self.lookup.insert(s.to_owned(), idx);
        idx
    }

    /// Return the number of interned strings.
    pub fn len(&self) -> usize {
        self.strings.len()
    }

    /// Return `true` if no strings have been interned.
    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }

    /// Intern a string identifier. Returns the existing [`Symbol`] if
    /// the string was already interned.
    pub fn intern(&mut self, s: &str) -> Symbol {
        Symbol(self.intern_raw(s))
    }

    /// Intern a file path. Same pool as [`intern`](Self::intern), but
    /// returns a distinct [`FileId`] type.
    pub fn intern_file(&mut self, path: &str) -> FileId {
        FileId(self.intern_raw(path))
    }

    /// Resolve a [`Symbol`] to its string.
    pub fn resolve(&self, sym: Symbol) -> &str {
        &self.strings[sym.0 as usize]
    }

    /// Resolve a [`FileId`] to its file path string.
    pub fn resolve_file(&self, file: FileId) -> &str {
        &self.strings[file.0 as usize]
    }
}

impl Default for SymbolInterner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intern_returns_same_symbol_on_repeated_calls() {
        let mut interner = SymbolInterner::new();
        let s1 = interner.intern("foo");
        let s2 = interner.intern("foo");
        assert_eq!(s1, s2);
    }

    #[test]
    fn different_strings_produce_different_symbols() {
        let mut interner = SymbolInterner::new();
        let s1 = interner.intern("foo");
        let s2 = interner.intern("bar");
        assert_ne!(s1, s2);
    }

    #[test]
    fn resolve_round_trips() {
        let mut interner = SymbolInterner::new();
        let sym = interner.intern("bar");
        assert_eq!(interner.resolve(sym), "bar");
    }

    #[test]
    fn intern_file_and_resolve_file_round_trip() {
        let mut interner = SymbolInterner::new();
        let fid = interner.intern_file("path/to/file");
        assert_eq!(interner.resolve_file(fid), "path/to/file");
    }

    #[test]
    fn symbol_and_file_id_are_distinct_types() {
        // Symbol and FileId from the same string are backed by the same
        // index but are different types -- this test verifies they compile
        // as distinct types and that both resolve correctly.
        let mut interner = SymbolInterner::new();
        let sym = interner.intern("shared");
        let fid = interner.intern_file("shared");
        // Same underlying index (deduplication), but different types.
        assert_eq!(interner.resolve(sym), "shared");
        assert_eq!(interner.resolve_file(fid), "shared");
    }

    #[test]
    fn empty_interner_has_no_entries() {
        let interner = SymbolInterner::new();
        assert_eq!(interner.len(), 0);
        assert!(interner.is_empty());
    }

    #[test]
    fn debug_shows_raw_index() {
        let mut interner = SymbolInterner::new();
        let sym = interner.intern("hello");
        let fid = interner.intern_file("world");
        assert_eq!(format!("{:?}", sym), "Symbol(0)");
        // "world" may share the pool -- check format, not specific index.
        let dbg = format!("{:?}", fid);
        assert!(dbg.starts_with("FileId("));
    }
}
