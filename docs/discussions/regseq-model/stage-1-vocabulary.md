# Stage 1: Vocabulary Specification

The foundational table-and-id scaffolding that every later stage of
RegseqModel rides on. Specifies five components: ID types, the string
interner, the source-file table and span representation, the DefPath
form, and the top-level `RegseqModel` struct.

This document is the spec; `stage-1-research.md` is the evidence base
behind the choices. F1-F4 from that document are taken as given here
and instantiated as concrete type shapes.

Type signatures below are sketches in Rust syntax. They pin shape and
intent; field names, derive lists, and minor ergonomic details are at
the implementation's discretion. Anything the implementation diverges
from materially should be reflected back into this document.

## Scope

In scope:

- ID types and their conventions.
- The `StringInterner`.
- The `SourceFile` table and the `Span` type.
- The `DefPath` form (segments, data kind, disambiguator) and its
  rendering.
- The `Instance` form and `InstanceId`.
- The top-level `RegseqModel` struct (forward-declares tables that
  later stages populate).
- Determinism rules (F4).
- Serialization conventions.
- Pretty-printer interface (sketch).

Out of scope (deferred to later stages):

- The `Ty` type (stage 3).
- The `Const` type (stage 4).
- Place / Operand / Rvalue / Statement / Terminator / BasicBlock /
  Body shapes (stages 5-11).
- The `Item` enum and Item-kind classification (stage 12).
- Domain extensions (stage 13).
- Bridge translation logic (separate document, post-spec).

Stage 1 forward-declares ID types like `TyId`, `ConstId`, `BodyId`,
`ItemId` so other stages and downstream consumers can reference them.
What those IDs *index into* is filled in by the corresponding later
stage. The top-level `RegseqModel` struct accordingly contains
`IndexVec<TyId, Ty>`, `IndexVec<ConstId, Const>`, and similar fields
whose element types are stage-N concerns.

## ID conventions

### The `Idx` trait

All IDs implement a small `Idx` trait that gives bidirectional
conversion to/from `usize`:

```rust
pub trait Idx: Copy + Eq + Hash + Ord + Debug + 'static {
    fn new(idx: usize) -> Self;
    fn index(self) -> usize;
}
```

This mirrors `rustc_index::Idx`. `Ord` is required (typed IDs are
comparable for stable iteration order in dumps). Unlike rustc's
`DefId` -- which deliberately omits `Ord` to discourage incremental-
order-dependent code -- our typed IDs are post-bridge and assigned in
deterministic order, so `Ord` is sound.

`'static` makes the existing owned/`'tcx`-free IR invariant (see the
"No `'tcx` references" model invariant later in this document, and Q2
in `architecture.md`) explicit at the trait boundary. All stage-1 IDs
are `u32`-backed and naturally `'static`; the bound costs nothing.
rustc's `rustc_index::Idx` carries the same bound (rustc additionally
omits `Ord`; we keep it for stable dump ordering).

### Backing representation

All stage-1 IDs are `u32` underneath, with niche optimization so that
`Option<Id>` is 4 bytes (rustc-style top-value reservation; the exact
mechanism is the implementation's choice). The implementation may use
`NonZeroU32` shifted by one, or a `NonMaxU32` newtype, or rustc's
explicit top-256 reservation.

Rationale: 4 billion items / types / consts is far beyond any
realistic driver crate. `u16` would be too tight (basic blocks of
large bodies routinely exceed 16K). `u32` is the rustc convention and
serializes to JSON cheaply.

### The `newtype_index!` analogue

A macro generates each typed ID. Conceptually:

```rust
newtype_index! {
    pub struct ItemId {}
}
```

expands to a struct with a `u32` (or niche-equivalent) field, an
`Idx` impl, and derives for `Copy`, `Clone`, `Eq`, `PartialEq`, `Ord`,
`PartialOrd`, `Hash`, `Debug`. The implementation may borrow rustc's
macro shape (in the `rustc_index_macros` crate) or hand-roll one;
the IR-side stability contract is just that all IDs satisfy `Idx +
Serialize + Deserialize + Hash + Ord`.

`Serialize` / `Deserialize` must not be field-derived (i.e., a bare
`#[derive(Serialize, Deserialize)]` on the niche-bearing struct), nor
`#[serde(transparent)]` over the niche-bearing inner field -- both
leak the internal niche-encoded representation to the wire. Each ID
newtype implements serde with the on-wire shape `u32 = Idx::index()`
via one of the following routes:

1. *Hand-rolled `impl Serialize` / `impl Deserialize`.* Direct control
   of error reporting; niche-violating input produces a clean
   `D::Error::custom(...)`. Self-contained per ID.
2. *`#[serde(into = "u32", try_from = "u32")]`* plus `From<Id> for u32`
   and `TryFrom<u32, Error = E> for Id`, where `E: Display` (e.g., a
   small `IdRangeError`) and the error message identifies the niche
   violation. The infallible `from = "u32"` shape is *not* acceptable
   -- delegating to `from_u32` panics on niche violations, which serde
   surfaces poorly.

Either route must:

- Round-trip the *logical index* through the wire, never the niche-
  encoded internal representation.
- Reject niche-violating values (e.g., `u32::MAX` if the niche
  reserves the top value) at deserialize time with a clear error.

Bounds-against-table validation happens later as a model-level
invariant, not a serde concern.

In addition to the trait method `Idx::new(usize)`, every ID newtype
exposes an inherent `pub const fn from_u32(idx: u32) -> Self` for use
in const contexts (the trait method is not `const`). All const ID
values in this spec (e.g., `LocalId::RETURN_PLACE`) go through
`from_u32`.

`Display` is *not* required on the ID itself (the pretty-printer
renders IDs with their kind prefix, e.g., `Item/7`, `bb3`, `_2`).

### `IndexVec<I, T>`

The canonical dense storage indexed by a typed ID. Mirrors rustc's
`IndexVec`. Whether we vendor rustc's implementation or write a thin
wrapper is the implementation's call; the spec requires only:

- `IndexVec<I: Idx, T>::push(value: T) -> I` (returns the assigned
  index).
- `IndexVec<I, T>::iter()`, `iter_mut()`, indexing by `I`.
- Length-as-`I::index` semantics (so iterating yields IDs in order).
- `Serialize` / `Deserialize` that produce a flat array of `T` on the
  wire (no per-element key). Implementable via `#[serde(transparent)]`
  on a single inner `Vec<T>` field with `PhantomData<I>` skipped, or
  via custom impl. The phantom-data type parameter must not surface in
  the JSON shape.

## Stage-1 ID inventory

The IDs introduced or named at stage 1, with their referents:

| ID | Width | Indexes into | Defined in |
|---|---|---|---|
| `CrateId` | u32 | `RegseqModel.crates` | stage 1 |
| `StringId` | u32 | `RegseqModel.strings` (the interner) | stage 1 |
| `SourceFileId` | u32 | `RegseqModel.source_files` | stage 1 |
| `ItemId` | u32 | `RegseqModel.items` | stage 12 (forward) |
| `BodyId` | u32 | `RegseqModel.bodies` | stage 11 (forward) |
| `TyId` | u32 | `RegseqModel.types` | stage 3 (forward) |
| `ConstId` | u32 | `RegseqModel.consts` | stage 4 (forward) |
| `InstanceId` | u32 | `RegseqModel.instances` | stage 1 |
| `LocalId` | u32 | per-`Body` `local_decls` | stage 11 (body-local) |
| `BlockId` | u32 | per-`Body` `basic_blocks` | stage 10 (body-local) |

Body-local IDs (`LocalId`, `BlockId`) are scoped to a single `Body`;
they are not unique across the whole RegseqModel. Crate-level IDs
(everything else) are unique within the RegseqModel.

### Body-local ID constants

By rustc convention, two body-local IDs are reserved:

```rust
impl LocalId {
    pub const RETURN_PLACE: LocalId = LocalId::from_u32(0);
}

impl BlockId {
    pub const START_BLOCK: BlockId = BlockId::from_u32(0);
}
```

Stage 11 (body) and stage 10 (basic blocks) restate this with the
behavior: `LocalId(0)` is the implicit return slot of every body, and
`BlockId(0)` is the entry block.

### IDs added at later stages

Stage 13 (domain extensions) may introduce additional IDs (e.g.,
`RegisterId`, `IsrId`). They follow the same conventions and live in
the same single namespace per kind.

## StringInterner

### Type

```rust
pub struct StringInterner {
    storage: Vec<Arc<str>>,
    lookup:  HashMap<Arc<str>, StringId>,
}
```

Two collections rather than one because we want amortized-O(1)
`resolve(StringId) -> &str` (vector indexing) and amortized-O(1)
`intern(&str) -> StringId` (hash lookup). `Arc<str>` lets `lookup`
keys and `storage` entries share allocations.

### Methods

```rust
impl StringInterner {
    pub fn new() -> Self;             // pre-interns the empty string at StringId(0)
    pub fn intern(&mut self, s: &str) -> StringId;
    pub fn resolve(&self, id: StringId) -> &str;
    pub fn len(&self) -> usize;
    pub fn iter(&self) -> impl Iterator<Item = (StringId, &str)>;
}
```

### Invariants

1. *Append-only.* No removal API. Once a `StringId` is assigned, the
   string at that index never changes. This matches rustc's interner
   posture; removal would invalidate every IR node holding that
   `StringId`.
2. *`StringId(0)` is the empty string.* Pre-interned by `new()`. Used
   as a sentinel where a name is absent (e.g., `DefPathSegment` for
   variants without a symbol). Documented as a stable contract.
3. *Hash by content for cross-session stability.* The interner's
   numeric IDs are allocation-order; if a RegseqModel artifact is
   ever serialized and compared across runs, equality is by content,
   not by `StringId` value. Pretty-printer comparisons and the F4
   sort comparator both go through `resolve`.

### Serialization

`StringInterner` implements `Serialize` / `Deserialize` manually
(derive cannot produce the desired shape because it would emit the
`lookup` HashMap alongside `storage`). The on-wire shape is a flat
JSON array of strings -- only `storage`, in order. On deserialize,
the implementation reads the array, populates `storage`, and rebuilds
`lookup` by re-interning each string in order. Round-trip preserves
all `StringId` values.

Deserialize-time validation:

1. Index 0 must be the empty string (matches the `StringId(0) == ""`
   invariant).
2. No duplicate entries (the interner is content-unique by
   construction; a duplicate in the input is a corrupted artifact).

Either failure surfaces as a clear deserialization error, not a
defensive fallback (per methodology).

## Source files and spans

### `SourceFile`

```rust
pub struct SourceFile {
    pub path:        StringId,         // file path (interned)
    pub content:     Option<Arc<str>>, // None for upstream-crate sources we do not own
    pub line_starts: Vec<u32>,         // file-local byte offsets of line starts; line N is at line_starts[N - 1] (1-based externally; the table itself is zero-indexed)
}
```

Per F3:

- `content` is populated for crates Regseq extracts (the spec crate,
  the `regseq` facade, any regseq-aware crate per Q6). It is `None`
  for upstream-crate sources whose spans we surface but whose source
  text we do not embed. The bridge decides population based on the
  cargo-metadata recognition rule.
- `line_starts` is computed from `content` when content is present.
  When content is `None`, `line_starts` is still populated (rustc
  provides line-start info on `SourceFile` independent of whether we
  hold the source text), enabling `path:line:col` rendering without
  source-line highlighting.

`Arc<str>` over `Arc<String>` because the content is immutable and
we want no extra heap indirection. Cloning is `O(1)` for sharing
across pretty-printer / diagnostic paths.

### `Position`

The resolved form of a byte offset:

```rust
pub struct Position {
    pub line: u32,        // 1-based
    pub col:  u32,        // 1-based byte column
}
```

Stage-1 `Position` is byte-column. Display-column (accounting for
multi-byte and full-width characters) is deferred. When stage-1 spec
work touches non-ASCII source in earnest, add a `display_col: u32`
field and a `multibyte_chars: Vec<MultiByteChar>` table on
`SourceFile`. For driver source code (overwhelmingly ASCII), byte-col
matches display-col.

### `Span`

```rust
pub struct Span {
    pub file: SourceFileId,
    pub lo:   u32,   // file-local byte offset, inclusive
    pub hi:   u32,   // file-local byte offset, exclusive
}
```

Per F1:

- 12 bytes total (4 + 4 + 4).
- `[lo, hi)` end-exclusive convention.
- `lo` and `hi` are *file-local* byte offsets into the
  `SourceFile.content` buffer (not rustc's global `BytePos` across
  the whole `SourceMap`).

Per F2: `Span` values appear inline in IR nodes, not behind a
`SpanId`. There is no span-deduplication table in the IR.

### Resolution methods

```rust
impl SourceFile {
    pub fn position(&self, byte: u32) -> Position;       // returns 1-based line + 1-based col
    pub fn line_text(&self, line: u32) -> Option<&str>;  // 1-based line index; matches Position
}

impl Span {
    pub fn start(&self, files: &IndexVec<SourceFileId, SourceFile>) -> Position;
    pub fn end(&self,   files: &IndexVec<SourceFileId, SourceFile>) -> Position;
    pub fn slice<'a>(&self, files: &'a IndexVec<SourceFileId, SourceFile>) -> Option<&'a str>;
}
```

`SourceFile::position` is a binary search over `line_starts`,
returning a `Position` with 1-based line and 1-based byte column.
`SourceFile::line_text` takes a 1-based line number to match
`Position`; returns `None` when `content` is `None` or when the line
is out of range, otherwise returns the slice between consecutive
line starts *excluding* the line terminator (whether `\n` or `\r\n`;
the implementation strips the terminator regardless of platform).
`Span::slice` returns `None` when `content` is `None`.

### Bridge invariants

The bridge populates `Span` from rustc's `SpanData` by:

1. Resolving `SpanData.lo: BytePos` (global) to a `SourceFileAndBytePos
   { sf: Arc<SourceFile>, pos: file_local_offset }` via
   `tcx.sess.source_map().lookup_byte_offset(SpanData.lo)`. Call the
   resulting `SourceFile` reference `sf`. The `pos` field is the
   file-local byte offset for `lo`.
2. Computing `file_local_hi = SpanData.hi - sf.start_pos` (using `sf`
   selected in step 1, *not* a fresh lookup of `hi`). Both endpoints
   must be interpreted relative to the same source file: a fresh
   `lookup_byte_offset(SpanData.hi)` could partition into a different
   `SourceFile` at a file boundary, since rustc's `SourceMap`
   partitions on `start_pos <= pos` and an end-exclusive `hi` lying
   exactly on the next file's `start_pos` would resolve to the next
   file. The bridge must use one source file per span.
3. Mapping the rustc `SourceFile` to a RegseqModel `SourceFileId` via
   the bridge's transient `HashMap<rustc_span::StableSourceFileId,
   SourceFileId>`. `StableSourceFileId` (a Hash128 of filename plus
   crate-id) is rustc's blessed unique identifier for a `SourceFile`
   within the crate graph; it is already computed and stored in the
   `stable_id` field on every `SourceFile`. We do not key on
   `Arc<SourceFile>` pointer identity (less semantic, fragile to
   rustc's internal Arc-cloning) nor on `FileName` (could collide
   across crates with same-named files).
4. Writing `Span { file: source_file_id, lo: file_local_lo, hi:
   file_local_hi }`.

Validation (debug assertions):

- `lo <= hi`.
- `hi - sf.start_pos <= sf.unnormalized_source_len` (i.e., `hi` lies
  within `sf`'s byte range; an `hi` that overflows the file is a sign
  the span crosses a file boundary, which the bridge must reject).
- When `SourceFile.content` is `Some(c)`: `hi <= c.len()` (file-local).
- The `file` index is valid (`< source_files.len()`).

A failed validation is a bridge bug and surfaces as a clear error;
no defensive fallback (per methodology).

## DefPath form

### Goal

A structured per-item identifier that:

- Distinguishes items in different crates (Q6 puts items from
  multiple crates into one RegseqModel).
- Distinguishes sibling impls or anon-consts via disambiguator.
- Renders cleanly to `crate::module::Item` strings for human display.
- Supports the F4 deterministic sort comparator.

### `DefPathData`

A closed enum mirroring rustc's `DefPathData`, mapped through our
interner:

```rust
pub enum DefPathData {
    // Root marker
    CrateRoot,

    // Items and item-like things
    Impl,                       // inherent or trait impl
    ForeignMod,                 // `extern "C"` block
    Use,                        // `use` item
    GlobalAsm,                  // `global_asm!`
    TypeNs(StringId),           // type, module
    ValueNs(StringId),          // function, const, static
    MacroNs(StringId),
    LifetimeNs(StringId),
    Closure,                    // closure expression

    // Subportions of items
    Ctor,                       // implicit tuple-struct / unit-variant ctor
    AnonConst,                  // const expr in source (array length, etc.)
    LateAnonConst,              // const expr created during AST->HIR lowering
    DesugaredAnonymousLifetime, // elided-lifetime placeholder
    OpaqueTy,                   // existential `impl Trait` node
    OpaqueLifetime(StringId),   // RPITIT captured lifetime
    AnonAssocTy(StringId),      // RPITIT anonymous associated type
    SyntheticCoroutineBody,     // coroutine by-move body
    NestedStatic,               // additional static data referred to by a static
}
```

Variants without a name (`Impl`, `Closure`, `AnonConst`,
`LateAnonConst`, `DesugaredAnonymousLifetime`, `OpaqueTy`,
`SyntheticCoroutineBody`, `NestedStatic`, `Ctor`, plus the root
markers) carry no `StringId`; the segment's identity comes from its
parent path plus the disambiguator.

*Closed-vocabulary scope.* The vocabulary is closed against the
*pinned toolchain* (see `rustc-references.md`). The 19 variants above
mirror `rustc_hir::definitions::DefPathData` exactly as of pinned
rustc; any `Symbol` payload in the rustc enum maps to `StringId` here.
A toolchain bump that adds a variant is a methodology event: the
agent making the bump adds the variant explicitly (and any new
sort-rank entry per the F4 table), with a Codex review of the new
variant's semantic placement.

The vocabulary does *not* claim to enumerate every variant the
project should accept at the source-construct level. Restrictions
like "no async functions in driver specs" or "no RPITIT" are
bridge-level / stage-12 concerns about which *items* are allowed,
not DefPath-vocabulary concerns about which *path segments* are
recognized. The bridge rejects unknown variants (no defensive
fallback per methodology), but inclusion of a variant in the
vocabulary is not an endorsement of every Rust source construct
that produces it.

### `DefPathSegment`

```rust
pub struct DefPathSegment {
    pub data:          DefPathData,
    pub disambiguator: u32,
}
```

`disambiguator` matches rustc's `DisambiguatedDefPathData.disambiguator`
field. For most segments it is `0`; for sibling `Impl` or `AnonConst`
in the same scope, it disambiguates.

### `DefPath`

```rust
pub struct DefPath {
    pub crate_id: CrateId,                // which crate this path lives in
    pub segments: Vec<DefPathSegment>,
    pub rendered: StringId,               // canonical verbose rendering, eagerly populated
}
```

`crate_id` is the per-item crate identity. Since Q6 places items
from multiple crates into one RegseqModel (the spec crate, the
`regseq` facade, regseq-aware deps), each `DefPath` must record
which crate it originates in. The matching `CrateInfo`
(name + `StableCrateId`) lives in `RegseqModel.crates[crate_id]`.

`segments` is the structured form, used by the bridge's F4 sort
comparator and by any future analysis that wants to pattern-match on
path shape.

`rendered` is the eagerly-computed canonical `crate::module::Item`
string. The rendering is *always verbose*: every segment with
`disambiguator != 0` carries a `#N` suffix, and every anonymous
namespace (`Impl`, `Closure`, `AnonConst`, `LateAnonConst`,
`OpaqueTy`, `SyntheticCoroutineBody`, etc.) is rendered as
`{namespace#N}` where `namespace` is the `DefPathData` variant name
in lowercase (matching rustc's
`DisambiguatedDefPathData::as_sym(verbose=true)` convention from
`rustc_hir/src/definitions.rs:185-204`). The crate prefix is the
resolved `crates[crate_id].name` (no leading `::`).

Construction invariant: `rendered` is the canonical verbose
rendering of `(crates[crate_id].name, segments)`. Two RegseqModels
for the same source must produce byte-identical `rendered` strings
for corresponding items; the bridge must not pick non-verbose mode
selectively. The pretty-printer reads `rendered` for display;
consumers get human-readable identifiers without having to walk
segments or resolve the crates table.

### `DefPathHash`

```rust
pub struct DefPathHash {
    pub stable_crate_id: StableCrateId,  // u64 wrapper, mirrored at the model root
    pub local_hash:      u64,
}
```

Mirrors rustc's `DefPathHash = (StableCrateId, Hash64)` exactly.
Stable across builds; computed by the bridge from rustc's
`tcx.def_path_hash(def_id)` (extracting the `StableCrateId` and the
local 64-bit hash) and stored in the side table `item_def_paths` (per
F4). Splitting the 128-bit fingerprint into named fields makes the
JSON shape unambiguous (`{ "stable_crate_id": u64, "local_hash":
u64 }`) and matches the rustc structure rather than treating it as an
opaque blob.

### Where `DefPath` lives

`DefPath` is part of `Item` (stage 12). Stage 1 specifies the type
and the rendering convention; stage 12 places it on the `Item`
struct.

The F4 cross-build identity table `item_def_paths: IndexVec<ItemId,
DefPathHash>` lives at the `RegseqModel` level (stage 1).

## Monomorphization: `Instance`

Per Q2, the bridge bakes monomorphization. Each call to a generic
function in MIR is resolved to a specific `(item, generic-args)`
tuple. The IR carries this as an `Instance`.

### `GenericArg`

```rust
pub enum GenericArg {
    Type(TyId),
    Const(ConstId),
    // Lifetime: erased post-monomorphization, not represented
}

pub type GenericArgs = Vec<GenericArg>;
```

Lifetimes are erased at monomorphization (rustc's
`erasing_regions` step at the bridge). They do not appear in
`GenericArgs`.

### `Instance`

```rust
pub struct Instance {
    pub item: ItemId,
    pub args: GenericArgs,
}

newtype_index! { pub struct InstanceId {} }
```

The bridge interns instances during construction so that two call
sites referring to the same `(item, args)` tuple share an
`InstanceId`. The interning HashMap is transient bridge state (not
in the IR); the IR holds `IndexVec<InstanceId, Instance>`.

Pretty-printer rendering follows rustc's MIR-dump convention:
`crate::module::Item::<u32>` for `Instance { item: my_func,
args: [Type(u32_ty)] }`.

### Where `InstanceId` references appear

Call sites in MIR carry `InstanceId` (not `ItemId`) when the callee
is a regular function. Calls to drop glue, virtual methods through
trait objects, or domain intrinsics use other forms; those are
stage-12 / stage-13 concerns and reuse `InstanceId` where
appropriate.

## Top-level `RegseqModel`

The owned root struct returned by `bridge::extract(tcx) ->
RegseqModel`. All other tables hang off it.

```rust
pub struct CrateInfo {
    pub name:            StringId,        // e.g., "pl011-spec", "regseq"
    pub stable_crate_id: StableCrateId,   // rustc's StableCrateId (u64 wrapper)
}

pub struct RegseqModel {
    // Crate inventory (one entry per crate the model contains items from)
    pub crates:        IndexVec<CrateId, CrateInfo>,
    pub primary_crate: CrateId,                          // points at the spec crate

    // Interning tables (stage 1)
    pub strings:      StringInterner,
    pub source_files: IndexVec<SourceFileId, SourceFile>,

    // Type / const tables (stages 3-4)
    pub types:  IndexVec<TyId,    Ty>,
    pub consts: IndexVec<ConstId, Const>,

    // Item / body / instance tables (stages 11-12)
    pub items:          IndexVec<ItemId, Item>,
    pub item_def_paths: IndexVec<ItemId, DefPathHash>,   // F4 cross-build identity
    pub bodies:         IndexVec<BodyId, Body>,
    pub instances:      IndexVec<InstanceId, Instance>,

    // Roots
    pub entry_points: Vec<ItemId>,
}
```

`StableCrateId` is rustc's 64-bit type (`rustc_span::def_id::StableCrateId`).
Since `regseq-model` has zero rustc deps (Q1), the type is mirrored
into our crate as a transparent `pub struct StableCrateId(pub u64);`
wrapper. The bridge populates it from rustc's `tcx.stable_crate_id`.

The crates table holds one `CrateInfo` per crate that contributes
items to the model. Per Q6 this includes the spec crate, the
`regseq` facade, and any regseq-aware dep declared via
`[package.metadata.regseq]`. `primary_crate` selects the spec crate
within the table (the entry point for downstream emit specs). The
relationship between `CrateId` (sequential u32 IR addressing,
this-build-only) and `StableCrateId` (cross-build stable identity,
stored on each `CrateInfo`) parallels the `ItemId` /
`item_def_paths[ItemId]` relationship for items.

### Construction invariants

The bridge guarantees:

1. *Tables are populated before they are referenced.* Every `CrateId`,
   `TyId`, `ConstId`, `ItemId`, `BodyId`, `InstanceId`,
   `SourceFileId`, `StringId` that appears anywhere in the model
   resolves to a valid index in its table. This is a debug-build
   assertion.
2. *`item_def_paths` mirrors `items`.* `item_def_paths.len() ==
   items.len()`, and for every valid `item_id`,
   `item_def_paths[item_id]` is the rustc `DefPathHash` corresponding
   to `items[item_id].def_path` (rustc's own `DefPathHash` of the
   originating `DefId`). Stage 12 specifies the placement of
   `def_path` on `Item` but inherits this consistency rule.
3. *Items are sorted at construction.* Per F4, items are placed in
   `items` (and `item_def_paths`) in the order produced by the F4
   sort comparator. Two builds of the same source produce identical
   `ItemId` assignments.
4. *`primary_crate` is valid and corresponds to the spec crate.*
   `primary_crate.index() < crates.len()`, and
   `crates[primary_crate].stable_crate_id ==
   bridge_input.spec_crate_stable_id`. The bridge picks
   `primary_crate` deterministically (the crate the bridge was
   invoked on; no fallback if it is missing).
5. *`crates[c].stable_crate_id` is unique across `c`.* No two
   `CrateInfo` entries share a `StableCrateId`. Same-named crates
   with different stable IDs are distinct entries; same stable ID
   means same crate.
6. *Every `DefPath.crate_id` resolves to a valid entry.* For every
   item, `item.def_path.crate_id.index() < crates.len()`. Items
   from upstream crates (per Q6 cross-crate handling) appear in the
   `crates` table just like the primary crate.
7. *`entry_points` references items in `primary_crate`.* The
   reachable items the bridge identified as entry points (functions
   called from outside the spec crate, e.g., the regseq-aware
   driver init). Stage 12 specifies the classification rule; stage
   1 just reserves the field and pins the
   `def_path.crate_id == primary_crate` constraint.
8. *No `'tcx` references.* The model is fully owned and `'static`-
   compatible; serialization round-trips losslessly.

Invariants outside debug builds are enforced by construction; the
bridge produces the model in one pass and writes only valid indices.

### Roots / entry points

`entry_points: Vec<ItemId>` lists the items the bridge identified as
entry points for downstream emit specs. Stage 12 specifies the
classification rule; stage 1 just defines the field.

For phase 2 (device pressure), the entry-point set will be the
driver-init / IRQ-handler / register-access functions that the C
backend must emit. For phase 1 linguistic snippets, it is whatever
top-level item the test drives.

### Design rationale: local IDs vs stable identity

The model uses two parallel ID schemes for things that need both
cheap addressing inside the IR and stable cross-build identity:

| Concern | Local addressing (cheap, per-build) | Stable cross-build identity |
|---|---|---|
| Items | `ItemId(u32)` indexes `items` | `DefPathHash` stored at `item_def_paths[ItemId]` (parallel side table) |
| Crates | `CrateId(u32)` indexes `crates` | `StableCrateId(u64)` stored on each `CrateInfo` (embedded field) |

The local IDs (`ItemId`, `CrateId`) keep IR field sizes small (4
bytes per reference) and serialization compact. They are *not*
stable across builds: two RegseqModels for the same source can have
identical `DefPath`s mapped to different sequential IDs if the
reachable set differs (incremental builds, partial extraction). A
build-internal value uses these directly.

The stable forms (`DefPathHash`, `StableCrateId`) are needed at
boundaries the local IDs cannot serve: cross-build diff alignment
("which item in build B corresponds to `ItemId(7)` in build A?"),
cross-graph item identity (rustc's own `DefPathHash` is
`(StableCrateId, Hash64)`; both halves are stored), and any
artifact consumed across incremental rebuilds. A consumer that
needs to compare across runs hashes by these fields, never by
local indices.

The two schemes are stored differently because their cardinalities
differ. There is exactly one `StableCrateId` per crate, so
embedding it as a `CrateInfo` field is cheaper than a separate
`IndexVec<CrateId, StableCrateId>`. For items, the side-table form
(`item_def_paths`) is used because `Item` itself (stage 12) carries
heavier data; consumers that want path-hash without paying for the
full `Item` load read the side table directly.

## Determinism (F4 in detail)

### The sort comparator

The bridge sorts items before assigning `ItemId`s. The comparator
operates on a temporary `(rustc::DefId, structured_def_path,
crate_info)` list and produces a sorted order, after which
`ItemId(0..n)` are assigned in that order.

Sort key per item:

```text
(crate_name:           &str,            // crates[item.def_path.crate_id].name (resolved)
 crate_stable_id:      StableCrateId,   // crates[item.def_path.crate_id].stable_crate_id
 segments:             Vec<SegmentKey>)

SegmentKey = (data_kind:    u8 (stable rank from def_path_data_sort_rank),
              symbol_text:  &str,    // empty when DefPathData carries no name
              disambiguator: u32)
```

The crate-level prefix comes from the crates table entry that the
item's `DefPath.crate_id` points at. Note this means the crates
table itself must already be populated before the items sort runs;
the bridge orders crates first (sorted by `(name, stable_crate_id)`
the same way) and assigns `CrateId`s, then sorts items.

Comparison:

1. `crate_name` (lexicographic).
2. `crate_stable_id` (`StableCrateId` numeric value, tiebreak for
   crates sharing a name).
3. `segments` (lexicographic; per-segment as below).
4. Per segment: `data_kind` (stable rank table, see below), then
   `symbol_text` (lexicographic), then `disambiguator` (numeric).

*Stable rank table.* The `data_kind: u8` is *not* `DefPathData as u8`
(which depends on enum layout and could shift as variants are
reordered) but a stable rank assigned by an explicit
`def_path_data_sort_rank` function. The rank is fixed by this spec
and is not allowed to drift. Adding a new variant during a toolchain
bump means appending a new rank at the end of the existing
assignments, never re-ranking earlier entries.

```rust
fn def_path_data_sort_rank(data: &DefPathData) -> u8 {
    match data {
        DefPathData::CrateRoot                  => 0,
        DefPathData::Impl                       => 1,
        DefPathData::ForeignMod                 => 2,
        DefPathData::Use                        => 3,
        DefPathData::GlobalAsm                  => 4,
        DefPathData::TypeNs(_)                  => 5,
        DefPathData::ValueNs(_)                 => 6,
        DefPathData::MacroNs(_)                 => 7,
        DefPathData::LifetimeNs(_)              => 8,
        DefPathData::Closure                    => 9,
        DefPathData::Ctor                       => 10,
        DefPathData::AnonConst                  => 11,
        DefPathData::LateAnonConst              => 12,
        DefPathData::DesugaredAnonymousLifetime => 13,
        DefPathData::OpaqueTy                   => 14,
        DefPathData::OpaqueLifetime(_)          => 15,
        DefPathData::AnonAssocTy(_)             => 16,
        DefPathData::SyntheticCoroutineBody     => 17,
        DefPathData::NestedStatic               => 18,
    }
}
```

The comparator runs in the bridge with the rustc `Symbol` interner
in scope: `symbol_text` is `Symbol::as_str()` for the duration of
the sort. After sort and ItemId assignment, the same names are
re-interned through the RegseqModel `StringInterner` and stored on
the corresponding `Item`s.

The comparator does *not* compare on `Symbol` index, `SymbolIndex`,
or any allocation-order numeric ID. Allocation-order numeric IDs
(rustc's `SymbolIndex`, our `StringId`) are not deterministic across
runs.

### Cross-build identity

`item_def_paths: IndexVec<ItemId, DefPathHash>` lets a consumer
align two RegseqModels for the same source by hash. Two builds will
produce identical sequential `ItemId`s only if their reachable item
sets are identical; for incremental/partial builds where the
reachable set differs, `DefPathHash` is the stable identifier that
maps "this `ItemId` in build A" to "that `ItemId` in build B."

`DefPathHash` identifies the *path*, not the item's body content.
Detecting body changes between builds is a separate question that
would need a body fingerprint; out of scope for stage 1.

## Serialization conventions

All stage-1 types are `Serialize` / `Deserialize` via serde. Most
field-derive directly. IDs follow the rules in the `Idx` section
(hand-written impls or serde conversion attributes); `StringInterner`
uses a hand-written impl per its section. JSON is the structured
format (Q5).

### Per-type conventions

| Type | JSON shape | Mechanism |
|---|---|---|
| ID newtypes (`CrateId`, `StringId`, `SourceFileId`, `ItemId`, ...) | bare `u32` (logical `Idx::index()`) | hand-written `impl` or serde conversion attributes (`#[serde(into = "u32", try_from = "u32")]`); niche-range validation; default field derive and `#[serde(transparent)]` are not acceptable |
| `StringInterner` | flat string array (the `storage` field) | custom `Serialize` / `Deserialize` with deserialize-time validation (index 0 is empty, no duplicates) |
| `IndexVec<I, T>` | flat array of `T` (no per-element key, no phantom-data field) | `#[serde(transparent)]` over inner `Vec<T>` with `PhantomData<I>` skipped, or custom impl |
| `CrateInfo` | object with `name` (StringId), `stable_crate_id` (u64) | derive |
| `SourceFile` | object with `path` (StringId), `content` (string or null), `line_starts` (array of u32) | derive |
| `Span` | object `{ "file": u32, "lo": u32, "hi": u32 }` | derive |
| `DefPath` | object with `crate_id` (CrateId), `segments` (array), `rendered` (StringId) | derive |
| `DefPathHash` | object `{ "stable_crate_id": u64, "local_hash": u64 }` | derive |
| `StableCrateId` | bare u64 | `#[serde(transparent)]` |
| `Instance` | object `{ "item": u32, "args": [...] }` | derive |
| `RegseqModel` | top-level object with all fields | derive |

JSON consumers reconstruct the interner lookup map on load (per the
`StringInterner` deserialization rules).

### Pretty-printer conventions

The textual MIR-inspired pretty-printer (Q5) renders IDs with kind-
prefixed sigils:

| ID | Sigil |
|---|---|
| `ItemId` | `Item/N` |
| `BodyId` | `Body/N` (rendered as the path of the owning item when in scope) |
| `TyId` | rendered as the type itself (`u32`, `*const u8`, ...) |
| `ConstId` | rendered as the const value |
| `LocalId` | `_N` (zero-based; `_0` is `RETURN_PLACE`) |
| `BlockId` | `bbN` |
| `SourceFileId` | rendered as the file path |
| `StringId` | rendered as the resolved string |
| `Span` | `path:line:col: line:col` (matches rustc's MIR-dump form) |
| `InstanceId` | `Item::<args>` (path of the item plus generic args) |

These conventions are pretty-printer concerns, not type-system
concerns; they live in `pretty.rs` (or per-module `Display` impls)
in the regseq-model crate.

## Pretty-printer interface (sketch)

The pretty-printer is the primary serialization (Q5). Per-type
`Display` impls are insufficient because rendering requires context
(the model's tables for ID resolution). The shape is a separate
`PrettyPrinter` that holds a reference to the model:

```rust
pub struct PrettyPrinter<'a> {
    model: &'a RegseqModel,
    // optional: indentation level, span-rendering mode, etc.
}

impl<'a> PrettyPrinter<'a> {
    pub fn new(model: &'a RegseqModel) -> Self;
    pub fn render(&self) -> String;
    // per-type render methods used by stages 3-13:
    pub fn render_ty(&self, ty: TyId) -> String;
    pub fn render_span(&self, span: Span) -> String;
    pub fn render_def_path(&self, item: ItemId) -> String;
    // ...
}
```

Stages 3-13 add `render_*` methods as their types come online. Stage
1 pins the shape and the conventions table above.

## Open items

Sub-decisions that surfaced during stage-1 spec work and are
deferred:

- *Path representation under `--remap-path-prefix`.* The bridge
  stores `SourceFile.path` as rustc reports it (post-remap). Spec'd
  in the construction document.
- *Multibyte / display-col handling.* Stage 1 ships byte-col only.
  When non-ASCII source becomes load-bearing (likely never in driver
  code, but possible via macro-generated identifiers or
  documentation strings), add a `display_col` field on `Position`
  and a `multibyte_chars` table on `SourceFile`. Migration is
  additive.
- *`BodyId` vs `ItemId`.* One body per item is the rustc convention
  (a function's `optimized_mir`). Whether `BodyId == ItemId`
  numerically (so the IR has one less ID to track) or whether
  bodies get their own sequential IDs is a stage-11 question.
  Stage 1 reserves both type names.

## References

- `stage-1-research.md` -- the evidence base; F1-F4 decisions and
  prior-art cluster summaries.
- `architecture.md` Q1 (two-crate split), Q2 (eager translation /
  internal IDs / self-describing IR), Q3 (single-level IR), Q5
  (insta + textual pretty-printer + serde JSON), Q6 (multi-crate
  strategy), Q7 (intrinsic recognition).
- `memory.md` -- methodology rules, especially "no defensive
  fallbacks" and "compiler-theory-first."
- rustc-internal landmarks (against the pinned toolchain):
  - `compiler/rustc_index/src/idx.rs` -- the `Idx` trait.
  - `compiler/rustc_index/src/vec.rs` -- `IndexVec<I, T>`.
  - `compiler/rustc_span/src/symbol.rs` -- the symbol interner.
  - `compiler/rustc_span/src/lib.rs` -- `Span`, `SourceFile`,
    `SourceMap`, `BytePos`.
  - `compiler/rustc_hir/src/definitions.rs` -- `DefKey`,
    `DefPathData`.
  - `compiler/rustc_span/src/def_id.rs` -- `DefPathHash`,
    `StableCrateId`.
