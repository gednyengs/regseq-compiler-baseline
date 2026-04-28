# Rustc Excerpts

Curated quotations from the pinned rustc-src checkout, gathered for
the stage-1 review. The reviewer can verify the spec's
rustc-internal claims first-hand against this file without needing
filesystem access to the rustc-src tree.

**Pinned toolchain**: `nightly-2026-02-01-aarch64-apple-darwin`.
**Source root**: `<toolchain>/lib/rustlib/rustc-src/rust/compiler/`.
All file paths below are relative to that root. Quotations are
verbatim except for elisions marked `...`.

This document is reference material; treat it as authoritative for
any rustc-internal claim the spec depends on.

---

## 1. ID conventions

### `rustc_index::Idx` trait

`rustc_index/src/idx.rs:6-24`:

```rust
/// Represents some newtyped `usize` wrapper.
///
/// Purpose: avoid mixing indexes for different bitvector domains.
pub trait Idx: Copy + 'static + Eq + PartialEq + Debug + Hash {
    fn new(idx: usize) -> Self;

    fn index(self) -> usize;

    #[inline]
    fn increment_by(&mut self, amount: usize) {
        *self = self.plus(amount);
    }

    #[inline]
    #[must_use = "Use `increment_by` if you wanted to update the index in-place"]
    fn plus(self, amount: usize) -> Self {
        Self::new(self.index() + amount)
    }
}
```

The Regseq spec mirrors this trait conceptually for its own ID
types. Note `Idx::new(usize)` is *not* `const`; the spec
requires a separate inherent `pub const fn from_u32(u32) -> Self`
for use in const contexts (e.g., `LocalId::RETURN_PLACE`).

### `newtype_index!` usage examples

`rustc_span/src/def_id.rs:18-22`:

```rust
rustc_index::newtype_index! {
    #[orderable]
    #[debug_format = "crate{}"]
    pub struct CrateNum {}
}
```

`rustc_span/src/def_id.rs:207-218`:

```rust
rustc_index::newtype_index! {
    /// A DefIndex is an index into the hir-map for a crate, identifying a
    /// particular definition. It should really be considered an interned
    /// shorthand for a particular DefPath.
    #[orderable]
    #[debug_format = "DefIndex({})"]
    pub struct DefIndex {
        /// The crate root is always assigned index 0 by the AST Map code,
        /// thanks to `NodeCollector::new`.
        const CRATE_DEF_INDEX = 0;
    }
}
```

The macro produces a `u32`-backed newtype with `Idx` impl, niche
optimization, and optional debug format and ordinal markers. The
Regseq spec proposes a hand-rolled analogue.

---

## 2. `Symbol` and the global interner

### `Symbol` definition

`rustc_span/src/symbol.rs:2783-2799`:

```rust
/// An interned UTF-8 string.
///
/// Internally, a `Symbol` is implemented as an index, and all operations
/// (including hashing, equality, and ordering) operate on that index. The use
/// of `rustc_index::newtype_index!` means that `Option<Symbol>` only takes up 4 bytes,
/// because `rustc_index::newtype_index!` reserves the last 256 values for tagging purposes.
///
/// Note that `Symbol` cannot directly be a `rustc_index::newtype_index!` because it
/// implements `fmt::Debug`, `Encodable`, and `Decodable` in special ways.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Symbol(SymbolIndex);

// Used within both `Symbol` and `ByteSymbol`.
rustc_index::newtype_index! {
    #[orderable]
    struct SymbolIndex {}
}
```

### `Symbol::as_str` (the lifetime-fiction transmute)

`rustc_span/src/symbol.rs:2814-2826`:

```rust
/// Access the underlying string. This is a slowish operation because it
/// requires locking the symbol interner.
///
/// Note that the lifetime of the return value is a lie. It's not the same
/// as `&self`, but actually tied to the lifetime of the underlying
/// interner. Interners are long-lived, and there are very few of them, and
/// this function is typically used for short-lived things, so in practice
/// it works out ok.
pub fn as_str(&self) -> &str {
    with_session_globals(|session_globals| unsafe {
        std::mem::transmute::<&str, &str>(session_globals.symbol_interner.get_str(*self))
    })
}
```

### `Symbol::hash_stable` (hashes content, not index)

`rustc_span/src/symbol.rs:2879-2900`:

```rust
impl<CTX> HashStable<CTX> for Symbol {
    #[inline]
    fn hash_stable(&self, hcx: &mut CTX, hasher: &mut StableHasher) {
        self.as_str().hash_stable(hcx, hasher);
    }
}

impl<CTX> ToStableHashKey<CTX> for Symbol {
    type KeyType = String;
    #[inline]
    fn to_stable_hash_key(&self, _: &CTX) -> String {
        self.as_str().to_string()
    }
}

impl StableCompare for Symbol {
    const CAN_USE_UNSTABLE_SORT: bool = true;

    fn stable_cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}
```

`SymbolIndex` is allocation-order, not stable across runs. Any
stable hash, comparison, or sort key on `Symbol` must go through
`as_str()`. This is the basis for the spec's F4 rule: "the
comparator does not compare on `Symbol` index... it compares on
string content via `Symbol::as_str()`."

### Interner storage

`rustc_span/src/symbol.rs:2946-2960`:

```rust
// Interner used for both `Symbol`s and `ByteSymbol`s. If a string and a byte
// string with identical contents (e.g. "foo" and b"foo") are both interned,
// only one copy will be stored and the resulting `Symbol` and `ByteSymbol`
// will have the same index.
pub(crate) struct Interner(Lock<InternerInner>);

// The `&'static [u8]`s in this type actually point into the arena.
//
// This type is private to prevent accidentally constructing more than one
// `Interner` on the same thread, which makes it easy to mix up `Symbol`s
// between `Interner`s.
struct InternerInner {
    arena: DroplessArena,
    byte_strs: FxIndexSet<&'static [u8]>,
}
```

Append-only interner with arena-backed string storage and an
`FxIndexSet` for lookup. The Regseq `StringInterner` mirrors the
shape (storage `Vec` + lookup `HashMap`), with `Arc<str>` instead
of arena-pointer trickery to keep the IR `'static`.

---

## 3. Spans, source files, byte positions

### `BytePos` (global, 32-bit)

`rustc_span/src/lib.rs:2651-2669`:

```rust
impl_pos! {
    /// A byte offset.
    ///
    /// Keep this small (currently 32-bits), as AST contains a lot of them.
    #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
    pub struct BytePos(pub u32);

    /// A byte offset relative to file beginning.
    #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
    pub struct RelativeBytePos(pub u32);

    /// A character offset.
    ///
    /// Because of multibyte UTF-8 characters, a byte offset
    /// is not equivalent to a character offset. The [`SourceMap`] will convert [`BytePos`]
    /// values to `CharPos` values as necessary.
    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
    pub struct CharPos(pub usize);
}
```

`BytePos` is the *global* byte offset across the entire `SourceMap`,
not per-file. The Regseq spec resolves rustc spans to file-local
offsets at the bridge by subtracting `SourceFile.start_pos`.
`RelativeBytePos` is rustc's own per-file form, used internally for
similar reasons.

### `SpanData`

`rustc_span/src/lib.rs:669-684`:

```rust
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
#[derive_where(PartialOrd, Ord)]
pub struct SpanData {
    pub lo: BytePos,
    pub hi: BytePos,
    /// Information about where the macro came from, if this piece of
    /// code was created by a macro expansion.
    #[derive_where(skip)]
    // `SyntaxContext` does not implement `Ord`.
    // The other fields are enough to determine in-file order.
    pub ctxt: SyntaxContext,
    #[derive_where(skip)]
    // `LocalDefId` does not implement `Ord`.
    // The other fields are enough to determine in-file order.
    pub parent: Option<LocalDefId>,
}
```

`SpanData` is the unpacked form. The packed `Span` (in
`span_encoding.rs`) is 8 bytes with 4 layout variants. Regseq
stores spans in the unpacked form, dropping `ctxt` and `parent`
(both rustc-internal and not needed downstream of the bridge).

### `SourceFile`

`rustc_span/src/lib.rs:1871-1905`:

```rust
/// A single source in the [`SourceMap`].
pub struct SourceFile {
    /// The name of the file that the source came from. Source that doesn't
    /// originate from files has names between angle brackets by convention
    /// (e.g., `<anon>`).
    pub name: FileName,
    /// The complete source code.
    pub src: Option<Arc<String>>,
    /// The source code's hash.
    pub src_hash: SourceFileHash,
    /// Used to enable cargo to use checksums to check if a crate is fresh rather
    /// than mtimes. This might be the same as `src_hash`, and if the requested algorithm
    /// is identical we won't compute it twice.
    pub checksum_hash: Option<SourceFileHash>,
    /// The external source code (used for external crates, which will have a `None`
    /// value as `self.src`.
    pub external_src: FreezeLock<ExternalSource>,
    /// The start position of this source in the `SourceMap`.
    pub start_pos: BytePos,
    /// The byte length of this source after normalization.
    pub normalized_source_len: RelativeBytePos,
    /// The byte length of this source before normalization.
    pub unnormalized_source_len: u32,
    /// Locations of lines beginnings in the source code.
    pub lines: FreezeLock<SourceFileLines>,
    /// Locations of multi-byte characters in the source code.
    pub multibyte_chars: Vec<MultiByteChar>,
    /// Locations of characters removed during normalization.
    pub normalized_pos: Vec<NormalizedPos>,
    /// A hash of the filename & crate-id, used for uniquely identifying source
    /// files within the crate graph and for speeding up hashing in incremental
    /// compilation.
    pub stable_id: StableSourceFileId,
    /// Indicates which crate this `SourceFile` was imported from.
    pub cnum: CrateNum,
}
```

The spec's `SourceFile` keeps `name` (as `path: StringId`), `src`
(as `content: Option<Arc<str>>`), `lines` (as `line_starts:
Vec<u32>`). Multi-byte / normalization tables are deferred (see
`stage-1-vocabulary.md` "Open items").

### `SourceMap::lookup_byte_offset` (global -> file-local)

`rustc_span/src/source_map.rs:1062-1075`:

```rust
/// For a global `BytePos`, computes the local offset within the containing `SourceFile`.
pub fn lookup_byte_offset(&self, bpos: BytePos) -> SourceFileAndBytePos {
    let idx = self.lookup_source_file_idx(bpos);
    let sf = Arc::clone(&(*self.files.borrow().source_files)[idx]);
    let offset = bpos - sf.start_pos;
    SourceFileAndBytePos { sf, pos: offset }
}

/// Returns the index of the [`SourceFile`] (in `self.files`) that contains `pos`.
/// This index is guaranteed to be valid for the lifetime of this `SourceMap`,
/// since `source_files` is a `MonotonicVec`
pub fn lookup_source_file_idx(&self, pos: BytePos) -> usize {
    self.files.borrow().source_files.partition_point(|x| x.start_pos <= pos) - 1
}
```

The bridge uses this to convert global rustc spans to file-local
RegseqModel spans.

### `StableSourceFileId`

`rustc_span/src/lib.rs:2080-2099`:

```rust
#[derive(Copy, Clone, Hash, PartialEq, Eq, ..., HashStable_Generic, ...)]
pub struct StableSourceFileId(Hash128);

impl StableSourceFileId {
    fn from_filename_in_current_crate(filename: &FileName) -> Self {
        Self::from_filename_and_stable_crate_id(filename, None)
    }

    pub fn from_filename_for_export(
        filename: &FileName,
        local_crate_stable_crate_id: StableCrateId,
    ) -> Self {
        Self::from_filename_and_stable_crate_id(filename, Some(local_crate_stable_crate_id))
    }
    ...
}
```

The 128-bit hash of filename plus crate-id. The Regseq spec uses
this as the bridge-side `HashMap` key for mapping rustc
`SourceFile`s to RegseqModel `SourceFileId`s.

---

## 4. `DefId`, `DefIndex`, `CrateNum`, `LocalDefId`

### `DefId`

`rustc_span/src/def_id.rs:220-245`:

```rust
/// A `DefId` identifies a particular *definition*, by combining a crate
/// index and a def index.
///
/// You can create a `DefId` from a `LocalDefId` using `local_def_id.to_def_id()`.
#[derive(Clone, PartialEq, Eq, Copy)]
// On below-64 bit systems we can simply use the derived `Hash` impl
#[cfg_attr(not(target_pointer_width = "64"), derive(Hash))]
#[repr(C)]
#[rustc_pass_by_value]
// We guarantee field order. Note that the order is essential here, see below why.
pub struct DefId {
    // cfg-ing the order of fields so that the `DefIndex` which is high entropy always ends up in
    // the lower bits no matter the endianness. This allows the compiler to turn that `Hash` impl
    // into a direct call to `u64::hash(_)`.
    #[cfg(not(all(target_pointer_width = "64", target_endian = "big")))]
    pub index: DefIndex,
    pub krate: CrateNum,
    #[cfg(all(target_pointer_width = "64", target_endian = "big"))]
    pub index: DefIndex,
}

// To ensure correctness of incremental compilation,
// `DefId` must not implement `Ord` or `PartialOrd`.
// See https://github.com/rust-lang/rust/issues/90317.
impl !Ord for DefId {}
impl !PartialOrd for DefId {}
```

The `!Ord` for `DefId` is the basis for the spec's F4 determinism
rule: any sort over rustc DefIds must use a stable key
(`DefPathHash` or path strings), not the raw `DefId` ordering.

### `LocalDefId`

`rustc_span/src/def_id.rs:344-353`:

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalDefId {
    pub local_def_index: DefIndex,
}

// To ensure correctness of incremental compilation,
// `LocalDefId` must not implement `Ord` or `PartialOrd`.
// See https://github.com/rust-lang/rust/issues/90317.
impl !Ord for LocalDefId {}
impl !PartialOrd for LocalDefId {}
```

Same `!Ord` rule applies.

---

## 5. `DefPath`, `DefKey`, `DefPathData`

### `DefKey` and `DisambiguatedDefPathData`

`rustc_hir/src/definitions.rs:127-204`:

```rust
/// A unique identifier that we can use to lookup a definition
/// precisely. It combines the index of the definition's parent (if
/// any) with a `DisambiguatedDefPathData`.
#[derive(Copy, Clone, PartialEq, Debug, Encodable, BlobDecodable)]
pub struct DefKey {
    /// The parent path.
    pub parent: Option<DefIndex>,

    /// The identifier of this node.
    pub disambiguated_data: DisambiguatedDefPathData,
}

impl DefKey {
    pub(crate) fn compute_stable_hash(&self, parent: DefPathHash) -> DefPathHash {
        let mut hasher = StableHasher::new();

        // The new path is in the same crate as `parent`, and will contain the stable_crate_id.
        // Therefore, we only need to include information of the parent's local hash.
        parent.local_hash().hash(&mut hasher);

        let DisambiguatedDefPathData { ref data, disambiguator } = self.disambiguated_data;

        std::mem::discriminant(data).hash(&mut hasher);
        if let Some(name) = data.hashed_symbol() {
            // Get a stable hash by considering the symbol chars rather than
            // the symbol index.
            name.as_str().hash(&mut hasher);
        }

        disambiguator.hash(&mut hasher);
        ...
    }
}

/// A pair of `DefPathData` and an integer disambiguator. The integer is
/// normally `0`, but in the event that there are multiple defs with the
/// same `parent` and `data`, we use this field to disambiguate
/// between them. ...
#[derive(Copy, Clone, PartialEq, Debug, Encodable, BlobDecodable)]
pub struct DisambiguatedDefPathData {
    pub data: DefPathData,
    pub disambiguator: u32,
}

impl DisambiguatedDefPathData {
    pub fn as_sym(&self, verbose: bool) -> Symbol {
        match self.data.name() {
            DefPathDataName::Named(name) => {
                if verbose && self.disambiguator != 0 {
                    Symbol::intern(&format!("{}#{}", name, self.disambiguator))
                } else {
                    name
                }
            }
            DefPathDataName::Anon { namespace } => {
                if let DefPathData::AnonAssocTy(method) = self.data {
                    Symbol::intern(&format!("{}::{{{}#{}}}", method, namespace, self.disambiguator))
                } else {
                    Symbol::intern(&format!("{{{}#{}}}", namespace, self.disambiguator))
                }
            }
        }
    }
}
```

The structured DefPath form. Note `compute_stable_hash` hashes the
discriminant of `DefPathData` plus `name.as_str()` (string content,
not `SymbolIndex`) plus disambiguator -- the same recipe the
Regseq F4 sort comparator follows.

### `DefPath`

`rustc_hir/src/definitions.rs:206-213`:

```rust
#[derive(Clone, Debug, Encodable, Decodable)]
pub struct DefPath {
    /// The path leading from the crate root to the item.
    pub data: Vec<DisambiguatedDefPathData>,

    /// The crate root this path is relative to.
    pub krate: CrateNum,
}
```

`DefPath` carries the crate (`krate`) plus the path segments.
The Regseq spec's `DefPath` mirrors this structure, with crate
identity moved to the model root rather than per-item (since one
RegseqModel may contain items from multiple crates per Q6).

### `DefPathData` enum (full 19 variants)

`rustc_hir/src/definitions.rs:272-321`:

```rust
/// New variants should only be added in synchronization with `enum DefKind`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Encodable, BlobDecodable)]
pub enum DefPathData {
    // Root: these should only be used for the root nodes, because
    // they are treated specially by the `def_path` function.
    /// The crate root (marker).
    CrateRoot,

    // Different kinds of items and item-like things:
    /// An impl.
    Impl,
    /// An `extern` block.
    ForeignMod,
    /// A `use` item.
    Use,
    /// A global asm item.
    GlobalAsm,
    /// Something in the type namespace.
    TypeNs(Symbol),
    /// Something in the value namespace.
    ValueNs(Symbol),
    /// Something in the macro namespace.
    MacroNs(Symbol),
    /// Something in the lifetime namespace.
    LifetimeNs(Symbol),
    /// A closure expression.
    Closure,

    // Subportions of items:
    /// Implicit constructor for a unit or tuple-like struct or enum variant.
    Ctor,
    /// A constant expression (see `{ast,hir}::AnonConst`).
    AnonConst,
    /// A constant expression created during AST->HIR lowering..
    LateAnonConst,
    /// A fresh anonymous lifetime created by desugaring elided lifetimes.
    DesugaredAnonymousLifetime,
    /// An existential `impl Trait` type node.
    /// Argument position `impl Trait` have a `TypeNs` with their pretty-printed name.
    OpaqueTy,
    /// Used for remapped captured lifetimes in an existential `impl Trait` type node.
    OpaqueLifetime(Symbol),
    /// An anonymous associated type from an RPITIT. The symbol refers to the name of the method
    /// that defined the type.
    AnonAssocTy(Symbol),
    /// A synthetic body for a coroutine's by-move body.
    SyntheticCoroutineBody,
    /// Additional static data referred to by a static.
    NestedStatic,
}
```

The Regseq spec's `DefPathData` mirrors this exactly (19
variants), with `Symbol` payloads mapped through `StringId`.

---

## 6. `DefPathHash` and `StableCrateId`

### `DefPathHash`

`rustc_span/src/def_id.rs:52-117`:

```rust
/// A `DefPathHash` is a fixed-size representation of a `DefPath` that is
/// stable across crate and compilation session boundaries. It consists of two
/// separate 64-bit hashes. The first uniquely identifies the crate this
/// `DefPathHash` originates from (see [StableCrateId]), and the second
/// uniquely identifies the corresponding `DefPath` within that crate. Together
/// they form a unique identifier within an entire crate graph.
///
/// ...
///
/// `DefPathHash` uses 64-bit hashes for both the crate-id part and the
/// crate-internal part ...
#[derive(Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[derive(HashStable_Generic, Encodable, Decodable)]
pub struct DefPathHash(pub Fingerprint);

impl DefPathHash {
    /// Returns the [StableCrateId] identifying the crate this [DefPathHash]
    /// originates from.
    #[inline]
    pub fn stable_crate_id(&self) -> StableCrateId {
        StableCrateId(self.0.split().0)
    }

    /// Returns the crate-local part of the [DefPathHash].
    #[inline]
    pub fn local_hash(&self) -> Hash64 {
        self.0.split().1
    }

    /// Builds a new [DefPathHash] with the given [StableCrateId] and
    /// `local_hash`, where `local_hash` must be unique within its crate.
    #[inline]
    pub fn new(stable_crate_id: StableCrateId, local_hash: Hash64) -> DefPathHash {
        DefPathHash(Fingerprint::new(stable_crate_id.0, local_hash))
    }
}
```

The Regseq `DefPathHash` mirrors `(StableCrateId, Hash64)` directly
as named fields rather than storing the opaque `Fingerprint` blob,
for unambiguous JSON serialization.

### `StableCrateId`

`rustc_span/src/def_id.rs:143-145`:

```rust
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[derive(Hash, HashStable_Generic, Encodable, BlobDecodable)]
pub struct StableCrateId(pub(crate) Hash64);
```

64-bit stable crate identifier, computed from crate name plus
`-Cmetadata` arguments. The Regseq spec mirrors this as a
transparent `pub struct StableCrateId(pub u64);` (the regseq-model
crate has zero rustc deps per Q1).

`StableCrateId::new`, `def_id.rs:147-159`:

```rust
impl StableCrateId {
    /// Computes the stable ID for a crate with the given name and
    /// `-Cmetadata` arguments.
    pub fn new(
        crate_name: Symbol,
        is_exe: bool,
        mut metadata: Vec<String>,
        cfg_version: &'static str,
    ) -> StableCrateId {
        let mut hasher = StableHasher::new();
        // We must hash the string text of the crate name, not the id, as the id is not stable
        // across builds.
        crate_name.as_str().hash(&mut hasher);
        ...
    }
    ...
}
```

The "hash by string content, not by SymbolIndex" rule appears
again in StableCrateId's own derivation.

---

## 7. `Instance` and `InstanceKind`

### `Instance` definition

`rustc_middle/src/ty/instance.rs:33-38`:

```rust
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, TyEncodable, TyDecodable)]
#[derive(HashStable, Lift, TypeFoldable, TypeVisitable)]
pub struct Instance<'tcx> {
    pub def: InstanceKind<'tcx>,
    pub args: GenericArgsRef<'tcx>,
}
```

The doc-comment above the struct (`rustc_middle/src/ty/instance.rs:25-29`):

```rust
/// A monomorphized `InstanceKind`.
///
/// Monomorphization happens on-the-fly and no monomorphized MIR is ever created. Instead, this type
/// simply couples a potentially generic `InstanceKind` with some args, and codegen and const eval
/// will do all required instantiations as they run.
```

The Regseq spec uses a richer interning key than rustc's
`(InstanceKind, args)`, defined in `stage-1-vocabulary.md` under
"Instance" as `RegseqInstanceKey`:

```text
(regseq_kind_rank:  u8,        // 0..5; Regseq InstanceKind variant
 item:              ItemId,
 generic_args_key:  ArgsKey,
 fn_shim_disambig:  u8)        // 0 for non-Function;
                               // for Function: rustc InstanceKind rank
```

The Regseq key separates the six Regseq emission kinds
(Function/Static/Const/Struct/Enum/Union) and additionally
distinguishes rustc shim variants within the `Function` kind --
because rustc's `InstanceKind` taxonomy is callable-only and
its variants can share `DefId`s (see `def_id()` below).

### `InstanceKind` enum

`rustc_middle/src/ty/instance.rs:62-185`. The full set of
rustc-side variants (each carrying a `DefId` and possibly extra
data):

```rust
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[derive(TyEncodable, TyDecodable, HashStable, TypeFoldable, TypeVisitable, Lift)]
pub enum InstanceKind<'tcx> {
    /// A user-defined callable item.
    /// Includes `fn` items, closures, coroutines.
    Item(DefId),

    /// An intrinsic `fn` item (with `#[rustc_intrinsic]`).
    Intrinsic(DefId),

    /// `<T as Trait>::method` where `method` receives unsizeable
    /// `self: Self`.
    VTableShim(DefId),

    /// `fn()` pointer where the function itself cannot be turned
    /// into a pointer. Used for virtual calls and `#[track_caller]`
    /// reification.
    ReifyShim(DefId, Option<ReifyReason>),

    /// `<fn() as FnTrait>::call_*`.
    FnPtrShim(DefId, Ty<'tcx>),

    /// Dynamic dispatch to `<dyn Trait as Trait>::fn`.
    Virtual(DefId, usize),

    /// `<[FnMut/Fn closure] as FnOnce>::call_once`.
    ClosureOnceShim { call_once: DefId, track_caller: bool },

    /// `<[FnMut/Fn coroutine-closure] as FnOnce>::call_once`.
    ConstructCoroutineInClosureShim {
        coroutine_closure_def_id: DefId,
        receiver_by_ref: bool,
    },

    /// Compiler-generated accessor for thread locals.
    ThreadLocalShim(DefId),

    /// Proxy shim for async drop of future.
    FutureDropPollShim(DefId, Ty<'tcx>, Ty<'tcx>),

    /// `core::ptr::drop_in_place::<T>`.
    DropGlue(DefId, Option<Ty<'tcx>>),

    /// Compiler-generated `<T as Clone>::clone` implementation.
    CloneShim(DefId, Ty<'tcx>),

    /// Compiler-generated `<T as FnPtr>::addr` implementation.
    FnPtrAddrShim(DefId, Ty<'tcx>),

    /// `core::future::async_drop::async_drop_in_place::<'_, T>`.
    AsyncDropGlueCtorShim(DefId, Ty<'tcx>),

    /// `core::future::async_drop::async_drop_in_place::<'_, T>::{closure}`.
    AsyncDropGlue(DefId, Ty<'tcx>),
}
```

### `InstanceKind::def_id()` (every variant carries a `DefId`)

`rustc_middle/src/ty/instance.rs:242-265`:

```rust
impl<'tcx> InstanceKind<'tcx> {
    #[inline]
    pub fn def_id(self) -> DefId {
        match self {
            InstanceKind::Item(def_id)
            | InstanceKind::VTableShim(def_id)
            | InstanceKind::ReifyShim(def_id, _)
            | InstanceKind::FnPtrShim(def_id, _)
            | InstanceKind::Virtual(def_id, _)
            | InstanceKind::Intrinsic(def_id)
            | InstanceKind::ThreadLocalShim(def_id)
            | InstanceKind::ClosureOnceShim { call_once: def_id, track_caller: _ }
            | ty::InstanceKind::ConstructCoroutineInClosureShim {
                coroutine_closure_def_id: def_id,
                receiver_by_ref: _,
            }
            | InstanceKind::DropGlue(def_id, _)
            | InstanceKind::CloneShim(def_id, _)
            | InstanceKind::FnPtrAddrShim(def_id, _)
            | InstanceKind::FutureDropPollShim(def_id, _, _)
            | InstanceKind::AsyncDropGlue(def_id, _)
            | InstanceKind::AsyncDropGlueCtorShim(def_id, _) => def_id,
        }
    }
    ...
}
```

This grounds the spec's claim that *every* `Instance` has an
originating source-level item: rustc's `def_id()` is total over
`InstanceKind`. The Regseq spec uses this in the `Instance.item`
field's "always present" rule.

Note that distinct variants can share a `DefId` (e.g.,
`Item(foo)` and `ReifyShim(foo, _)` both have `def_id == foo`).
This is the basis for the spec's interning rule: keying by
`(item, generic_args)` alone is insufficient when shims can
share `DefId`s; the key must include the full `InstanceKind`
discriminator.

## 8. Symbol names: `tcx.symbol_name`

### `SymbolName` type

`rustc_middle/src/ty/mod.rs:2191-2201`:

```rust
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, TyEncodable, HashStable)]
pub struct SymbolName<'tcx> {
    /// `&str` gives a consistent ordering, which ensures reproducible builds.
    pub name: &'tcx str,
}

impl<'tcx> SymbolName<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>, name: &str) -> SymbolName<'tcx> {
        SymbolName { name: tcx.arena.alloc_str(name) }
    }
}
```

A `SymbolName` is a `&'tcx str` interned in the type-context
arena. It is `Ord`-comparable for reproducible-build ordering.

### `tcx.symbol_name` query

`rustc_symbol_mangling/src/lib.rs:124-148`:

```rust
pub fn provide(providers: &mut Providers) {
    *providers = Providers { symbol_name: symbol_name_provider, ..*providers };
}

// The `symbol_name` query provides the symbol name for calling a given
// instance from the local crate. In particular, it will also look up the
// correct symbol name of instances from upstream crates.
fn symbol_name_provider<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> ty::SymbolName<'tcx> {
    let symbol_name = compute_symbol_name(tcx, instance, || {
        // This closure determines the instantiating crate for instances that
        // need an instantiating-crate-suffix for their symbol name, in order
        // to differentiate between local copies.
        if is_generic(instance) {
            instance.upstream_monomorphization(tcx).unwrap_or(LOCAL_CRATE)
        } else {
            LOCAL_CRATE
        }
    });

    ty::SymbolName::new(tcx, &symbol_name)
}
```

`tcx.symbol_name(instance)` is the canonical rustc-side query
that produces the mangled name for an `Instance`. It accounts
for the full `InstanceKind` (shims and base items get distinct
names by construction) and the `args`. The Regseq bridge calls
this and interns the resulting string into `StringInterner` to
populate `Instance.symbol_name`.

The doc-comment in `compute_symbol_name`
(`rustc_symbol_mangling/src/lib.rs:160-175`) shows the full
derivation pulls `instance.def_id()` and `instance.args` plus
codegen attributes, instantiating crate, and any
`#[rustc_std_internal_symbol]` overrides. The result is a
`String` which `SymbolName::new` interns.

## 9. Vtable entries: `VtblEntry` and `tcx.vtable_entries`

`rustc_middle/src/ty/vtable.rs:12-26`:

```rust
#[derive(Clone, Copy, PartialEq, HashStable)]
pub enum VtblEntry<'tcx> {
    /// destructor of this type (used in vtable header)
    MetadataDropInPlace,
    /// layout size of this type (used in vtable header)
    MetadataSize,
    /// layout align of this type (used in vtable header)
    MetadataAlign,
    /// non-dispatchable associated function that is excluded from trait object
    Vacant,
    /// dispatchable associated function
    Method(Instance<'tcx>),
    /// pointer to a separate supertrait vtable, can be used by trait upcasting coercion
    TraitVPtr(TraitRef<'tcx>),
}
```

A trait-object vtable is **not** just a struct of fn pointers.
Each entry can be:

- `MetadataDropInPlace` -- a fn pointer to drop glue for the
  concrete `Self` type.
- `MetadataSize` -- the size of the concrete type, encoded inline
  as a usize-shaped slot.
- `MetadataAlign` -- the alignment of the concrete type, encoded
  inline as a usize-shaped slot.
- `Vacant` -- a slot for a non-dispatchable associated function
  (excluded from the trait object). Encoded as a null fn ptr in
  codegen.
- `Method(Instance)` -- a fn pointer to the resolved method
  Instance.
- `TraitVPtr(TraitRef)` -- a pointer to a supertrait's vtable
  (for upcasting).

The Regseq spec models a vtable as a synthesized Static
`Instance` whose value is a struct matching this entry sequence
(metadata header, then per-method fn pointers, then any
supertrait vtable pointers). Stage 9/12 will pin the precise
struct layout; stage 1 reserves the policy.
