# Stage 1 RegseqModel Spec Snapshot

> **Snapshot at close of stage 1.** Frozen historical artifact
> preserving stage 1's contribution at its close, including the
> full audit-shape decision text. The current RegseqModel spec
> lives in top-level [`../spec.md`](../spec.md).

The foundational table-and-id scaffolding that every later stage of
RegseqModel rides on. Specifies five components: ID types, the string
interner, the source-file table and span representation, the DefPath
form, and the top-level `RegseqModel` struct.

This document is the spec; `research.md` is the evidence base
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
- Domain extensions (stage 14).
- Bridge translation logic (separate document, post-spec).

Stage 1 forward-declares ID types like `TyId`, `ConstId`, `BodyId`,
`ItemId` so other stages and downstream consumers can reference them.
What those IDs *index into* is filled in by the corresponding later
stage. The top-level `RegseqModel` struct accordingly contains
`IndexVec<TyId, Ty>`, `IndexVec<ConstId, Const>`, and similar fields
whose element types are stage-N concerns.

## Rust -> C lowering reference

RegseqModel is a C-shaped IR. The bridge translates Rust-side
constructs into C-friendly normalized fields whose shape is
dictated by downstream stakeholder needs (domain analyses + the
C codegen backend). For each construct, ask what it carries
semantically, who consumes it, and what shape serves them best --
then translate. Don't pass annotations through as opaque
source-side metadata. Where rustc has done the work (layout
resolution, monomorphization, region erasure), store the result,
not the input.

| Rust construct | RegseqModel shape | Notes |
|---|---|---|
| Generics | Monomorphized; each instantiation is a distinct `Instance` | Q2 + this stage |
| Lifetimes | Erased | rustc's `erasing_regions` |
| Closures | Already MIR-desugared to struct + method; mirrored as such | Stage 11/12 |
| `Drop` terminator | Lowered to `Call` targeting drop-glue `Instance` | Stage 9 |
| Trait objects | Vtable as Static `Instance` of mixed-aggregate type matching rustc's `tcx.vtable_entries` sequence: `MetadataDropInPlace` (fn ptr) + `MetadataSize` + `MetadataAlign` + per-method fn pointers + supertrait vtable pointers (`TraitVPtr`) + `Vacant` slots for non-dispatchable fns. See `rustc-excerpts.md` section 9. Dynamic dispatch is indirect call through the loaded fn ptr. | Stage 9, 12, 13 |
| `&T` | `Ty::Ref(TyId, Provenance::SharedBorrow)` -> `const T*` (drops `const` if pointee transitively contains `UnsafeCell`, per stage-3 D7/D8) | Stage 3 |
| `&mut T` | `Ty::Ref(TyId, Provenance::UniqueBorrow)` -> `T*`; codegen MAY emit `T* restrict` only in declaration contexts where unique-borrow provenance is preserved (function parameters and locals with no escape into stored state, returns, or raw-pointer reborrows). Codegen decides per-context; the type itself does not bake `restrict`. | Stage 3 / 9 |
| `*const T` / `*mut T` | `Ty::Ptr(TyId, Mutability::Not \| Mut)` -> `const T*` / `T*`. Raw provenance is encoded by the variant tag (`Ty::Ptr` vs `Ty::Ref`), not by an enum value -- there is no `Provenance::Raw`. | Stage 3; no `restrict` (raw pointers may alias) |
| `Result`, `Option`, ADTs | Tagged unions or sum-of-product structs via `Ty::Enum` | Stage 3, 4 |
| `match`, `?` | Already MIR-lowered to control flow | Stages 8, 9 |
| `impl` blocks | Lifted; not represented as Items (see "Lifting `impl` items") | This stage |
| `repr(C)`, `repr(packed)`, etc. | **Resolved layout** stored on `Ty::Struct`, not the annotation | Stage 3; bridge calls `tcx.layout_of` |
| `extern "C"` etc. | `Abi` enum on `FnSig` | Stage 11 |
| `#[no_mangle]`, `#[export_name]` | `Linkage` enum on `Item` | Stage 12 |
| `#[inline]`, `#[cold]` | `InlineHint` enum on `Item` | Stage 12 |
| `unsafe fn` | `is_unsafe: bool` on `Item` (consumed by analyses, not codegen) | Stage 12 |
| `const fn` | Likely not represented (drop after const-eval consumes it) | Confirm with a real driver case |
| `async fn` | Reject if encountered (no_std drivers don't use async) | n/a |
| `global_asm!` | Reject if encountered (driver code uses inline asm at fn level, not module level) | n/a |
| `Vec`, `String`, `Box`, other std | **N/A -- not usable in regseq specs (`no_std`, no `alloc`)** | n/a |
| `#[rseq::*]` attributes | Structured `DomainAttr` on `Item` / `Instance` | Stage 14 |

This table is descriptive, not exhaustive: it pins the
already-decided lowerings and serves as a check on future stage
work. New rows are added as new constructs are settled.

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
`Idx` impl, derives for `Copy`, `Clone`, `Eq`, `PartialEq`, `Ord`,
`PartialOrd`, `Hash`, `Debug`, and an associated `pub const ZERO:
Self = Self::from_u32(0);` constant for the index-0 sentinel
pattern (used e.g. as `SourceFileId::ZERO`, `StringId::ZERO`; see
"Synthesized source-file sentinel" below). The implementation may
borrow rustc's macro shape (in the `rustc_index_macros` crate) or
hand-roll one; the IR-side stability contract is that all IDs
satisfy `Idx + Serialize + Deserialize + Hash + Ord` and expose
the `ZERO` const.

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
| `FnSigId` | u32 | `RegseqModel.sigs` | stage 11 (forward) |
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

Stage 14 (domain extensions) may introduce additional IDs (e.g.,
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

## Lifting `impl` items

`impl` blocks are source-level scopes, not emittable units.
RegseqModel does not represent them as Items. Their contents lift
to the top-level `items` / `instances` tables:

- **Inherent impl methods** (`impl Foo { fn bar(&self) }`):
  lifted to top-level Function Items with an explicit receiver
  parameter. `&self` becomes `&Foo` after substitution.
- **Trait impl methods** (`impl Trait for Foo { fn bar() }`):
  same lifting. The trait association survives only via DefPath
  (`crate::Foo::{impl#0 as Trait}::bar`) and -- for dynamic
  dispatch only -- via vtables synthesized as Static Instances.
- **Associated constants**: lifted to top-level Const Items.
- **Associated types**: erased. All uses are substituted at
  monomorphization (`<Foo as Iterator>::Item` -> the resolved
  type). They never appear as Items.
- **Trait declarations**: erased. Static dispatch resolves
  directly to the implementing function's Instance; dynamic
  dispatch goes through a vtable. The trait itself has no
  emittable counterpart.
- **Default trait method bodies**: a default body in a trait
  declaration (`trait Foo { fn bar() { ... default ... } }`) is
  owned by the trait, not by any impl. Its body lives at the
  trait's DefPath (e.g., `crate::Foo::bar`); a single Function
  Item is created there, just like for inherent fns. When `impl
  Foo for X` does not override `bar`, the use-site `<X as
  Foo>::bar()` resolves to an `Instance { item: <trait::bar
  Item>, generic_args: [Self=X, ...] }` whose body is the
  default body monomorphized for the substitution. No impl-side
  Item is created for the inherited default.

Consequences:

- `ItemKind` (stage 12) has six variants -- `Function`, `Static`,
  `Const`, `Struct`, `Enum`, `Union` -- mirroring `InstanceKind`.
  No `Impl`, `Trait`, or `AssocType` variants.
- `DefPathData` keeps its full 19-variant inventory. `Impl`
  remains a path-segment variant for source-level traceback; it
  is not an Item kind.
- The method-vs-function distinction does not survive lifting.
  Consumers see regular functions with whatever first parameter
  the source declared.

## Instances: monomorphized items

Per Q2 the bridge bakes monomorphization. **Instance is the
primary monomorphized unit in RegseqModel.** Every emittable
construct -- functions, statics, consts, structs, enums, unions
-- appears in `instances` as a fully-resolved monomorphization.
Generic forms live in `items` only as the alphabet for
source-level traceback (DefPath identity, attributes, span);
generic items themselves are not emitted.

Downstream stakeholders -- domain analyses and the C codegen
backend -- traverse `instances` and treat each as one C-level
entity. They do not iterate `items` for emission. The
`(item, generic_args)` back-reference exists for diagnostics,
custom naming strategies, source navigation, and cross-build
identity, not for emission.

### `GenericArg`

```rust
pub enum GenericArg {
    Type(TyId),
    Const(ConstId),
    // Lifetimes: erased post-monomorphization, not represented.
}

pub type GenericArgs = Vec<GenericArg>;
```

Lifetimes are erased at the bridge (rustc's `erasing_regions`
step). They never appear in `GenericArgs`.

### `Instance`

```rust
pub struct Instance {
    pub item:         ItemId,         // origin: source-level item, always present
    pub generic_args: GenericArgs,    // origin: resolved args; empty for non-generic items
    pub symbol_name:  StringId,       // default mangled name from `tcx.symbol_name`
    pub span:         Span,           // source span; synthesized => sentinel (see below)
    pub kind:         InstanceKind,
}

newtype_index! { pub struct InstanceId {} }
```

Common fields:

- `item`: back-reference to the source-level Item. Always present;
  every rustc instance variant carries a `DefId`, so an originating
  item exists for every Instance. For Instances whose origin is a
  recognized `core` lang-item (e.g., `drop_in_place` for drop
  glue), the `Item` entry is metadata-only: DefPath, kind, span,
  attributes -- captured by the bridge from rustc-side metadata,
  with no body in our `bodies` table. This is consistent with the
  "core is opaque" methodology rule (we don't walk core MIR);
  per-monomorphization `Instances` of these origins still have
  rustc-generated bodies (e.g., the per-type drop-glue MIR), which
  do appear in `bodies`.
- `generic_args`: the resolved type/const args used for this
  instantiation. Empty for non-generic items.
- `symbol_name`: the bridge-provided default symbol name,
  derived from rustc's `tcx.symbol_name(instance)` (see
  `rustc-excerpts.md` section 8). Format follows rustc's
  standard mangling scheme; uniqueness across `instances` is
  guaranteed by the bridge by construction (rustc's mangler
  factors in the full `InstanceKind`, args, and instantiating
  crate). Consumers wanting a different naming strategy may
  walk `(item.def_path, generic_args)` and synthesize their
  own. **Caveat**: `(item.def_path, generic_args)` is *not*
  unique when rustc shims share a `DefId` (e.g., `Item(foo)`
  vs. `ReifyShim(foo)` -- see `rustc-excerpts.md` section 7).
  Custom names that may encounter shim-distinguished Instances
  must include `InstanceId` or the bridge-provided
  `symbol_name` as a disambiguating suffix to avoid collisions.
- `span`: source location. For bridge-synthesized items (drop
  glue, vtable shims, closure-capture structs that have no source
  location), this is `Span { file: SourceFileId::SYNTHESIZED,
  lo: 0, hi: 0 }` -- the synthesized sentinel.

Origin is just `(item, generic_args)`. There is no separate
`Origin` enum exposed in the public type; the two fields *are*
the origin from the consumer's point of view.

The bridge interns instances during construction. The interning
key is a **Regseq-level instance key** that distinguishes both
the Regseq `InstanceKind` variant *and*, for `Function` instances,
the rustc-side shim discriminator. Two call sites that produce
the same Regseq instance key share an `InstanceId`.

```text
RegseqInstanceKey = (
    regseq_kind_rank:    u8,    // Regseq InstanceKind variant rank (table below)
    item:                ItemId,
    generic_args_key:    ArgsKey,
    fn_shim_disambig:    u8,    // 0 for non-Function variants;
                                // for Function: rustc InstanceKind rank (table below)
)
```

**Why both ranks**: Regseq's `InstanceKind` covers six emission
kinds (Function/Static/Const/Struct/Enum/Union); rustc's
`InstanceKind` is a *callable*-only taxonomy with shim variants
(`Item`, `DropGlue`, `VTableShim`, `ReifyShim`, `FnPtrShim`,
`Virtual`, `ClosureOnceShim`, etc.) that can share a `DefId`.
The `regseq_kind_rank` separates the six emission kinds; the
`fn_shim_disambig` further separates rustc shims within the
`Function` Regseq kind. Without the second discriminator,
`Item(foo)` and `ReifyShim(foo)` would collapse into one
`Instance` despite having distinct semantics, bodies, and
symbol names.

The shim discriminator is **not** exposed as a public field on
`Instance`. It survives only:

- in `symbol_name` (`tcx.symbol_name(instance)` derives from the
  full rustc instance identity, so distinct rustc instances get
  distinct mangled names);
- in `body` (each rustc instance gets its own MIR body);
- in the F4-2 sort key (Regseq instance key as defined above).

The interning HashMap is transient bridge state.

#### `regseq_instance_kind_rank` table

Stable rank for the Regseq `InstanceKind` variant. Append-only
(new variants get new ranks at the end; existing ranks are
immutable):

```rust
fn regseq_instance_kind_rank(kind: &InstanceKind) -> u8 {
    match kind {
        InstanceKind::Function { .. } => 0,
        InstanceKind::Static   { .. } => 1,
        InstanceKind::Const    { .. } => 2,
        InstanceKind::Struct   { .. } => 3,
        InstanceKind::Enum     { .. } => 4,
        InstanceKind::Union    { .. } => 5,
    }
}
```

#### `rustc_instance_kind_sort_rank` table

Stable rank for rustc's `ty::InstanceKind` variant, used as
`fn_shim_disambig` when the Regseq kind is `Function`. Pinned
for the toolchain `nightly-2026-02-01-aarch64-apple-darwin`
(see `rustc-excerpts.md` section 7 for the variant inventory).
Append-only:

```rust
fn rustc_instance_kind_sort_rank(kind: &rustc_ty::InstanceKind<'_>) -> u8 {
    match kind {
        InstanceKind::Item(..)                    => 0,
        InstanceKind::Intrinsic(..)               => 1,
        InstanceKind::VTableShim(..)              => 2,
        InstanceKind::ReifyShim(..)               => 3,
        InstanceKind::FnPtrShim(..)               => 4,
        InstanceKind::Virtual(..)                 => 5,
        InstanceKind::ClosureOnceShim { .. }      => 6,
        InstanceKind::ConstructCoroutineInClosureShim { .. } => 7,
        InstanceKind::ThreadLocalShim(..)         => 8,
        InstanceKind::FutureDropPollShim(..)      => 9,
        InstanceKind::DropGlue(..)                => 10,
        InstanceKind::CloneShim(..)               => 11,
        InstanceKind::FnPtrAddrShim(..)           => 12,
        InstanceKind::AsyncDropGlueCtorShim(..)   => 13,
        InstanceKind::AsyncDropGlue(..)           => 14,
    }
}
```

For non-`Function` Regseq kinds (Static/Const/Struct/Enum/Union),
`fn_shim_disambig` is unconditionally `0` -- they have no
rustc-shim discrimination to encode.

There is no `is_root` flag on `Instance`. Roots (e.g.,
`#[no_mangle]` exports for codegen, analysis-specific reachability
roots) are consumer-derived predicates over `instance.item`'s
attributes, ABI, and visibility -- which live on `Item`
(stage 12).

### `InstanceKind`

```rust
pub enum InstanceKind {
    /// Function instantiation. `sig` is the substituted
    /// signature for this monomorphization (always present, even
    /// for foreign declarations where there is no body). `body`
    /// is `None` for foreign declarations whose bodies are not
    /// in our crate set (e.g., `extern "C" { fn foo(); }`); C
    /// codegen still has the prototype via `sig`.
    Function { sig: FnSigId, body: Option<BodyId> },

    /// Static. Covers both `static` (immutable) and `static mut`
    /// (mutable). The mutability flag lives on the source `Item`
    /// (stage 12, `ItemKind::Static { mutability, .. }`); C
    /// codegen reads it via `instance.item` to decide whether to
    /// emit `const` qualification on the global. `init` is the
    /// const-evaluated initializer.
    Static { ty: TyId, init: ConstId },

    /// Const item.
    Const { ty: TyId, value: ConstId },

    /// Nominal struct. `layout` points at the corresponding
    /// `Ty::Struct` for field structure.
    Struct { layout: TyId },

    /// Nominal enum. `layout` points at `Ty::Enum`.
    Enum { layout: TyId },

    /// Nominal C-style union. `layout` points at `Ty::Union`.
    Union { layout: TyId },
}
```

Each variant carries only the IDs needed to find kind-specific
rich data in the appropriate stage's table:

- `Function.sig` -> `sigs` (stage 11). The substituted `FnSig`
  for this monomorphization. Always present, including for
  foreign declarations (`body: None`). C codegen reads the
  prototype from `sig` regardless of body presence; analyses
  read it for call-shape information.
- `Function.body` -> `bodies` (stage 11). Monomorphized MIR
  body. `None` for foreign declarations whose bodies are not in
  our crate set.
- `Static.ty` / `Const.ty` -> `types` (stage 3).
- `Static.init` / `Const.value` -> `consts` (stage 4).
- `Struct.layout` / `Enum.layout` / `Union.layout` -> `types`
  (stage 3 will define `Ty::Struct { fields }`,
  `Ty::Enum { variants }`, `Ty::Union { fields }`).

Stage 14 may extend `InstanceKind` with domain-specific variants
(e.g., an `Isr` subkind of `Function`). For stage 1, the six
variants above are the closed set.

### Where `InstanceId` references appear

Call sites in MIR (stage 9 terminators) carry `InstanceId`
identifying the callee. This includes:

- Regular function calls.
- Direct dispatch on trait methods (resolved at monomorphization).
- Drop-glue calls: stage 9 lowers `Drop` terminators to regular
  `Call` terminators targeting the drop-glue `Instance` for the
  type. There is no `Drop` variant in stage-9 terminators.

Dynamic dispatch through trait objects becomes an indirect call
through a vtable. The vtable is a synthesized Static `Instance`
(see "Bridge-synthesized Items" above) whose value is a mixed
aggregate matching rustc's `tcx.vtable_entries(trait_ref)`
sequence: drop-glue fn pointer, size, alignment, then
per-trait-method fn pointers, supertrait vtable pointers, and
`Vacant` slots for non-dispatchable methods (see
`rustc-excerpts.md` section 9). Both the drop-glue slot and each
method fn pointer reference synthesized **vtable shim** Function
Items (per stage-12 expectations, section C: a drop-in-place
shim per `(Trait, ConcreteImpl)` plus a method shim per
`(Trait, ConcreteImpl, method)`); each shim takes an erased
receiver (`*const ()` or `*mut ()`), casts to the concrete
receiver, and tail-calls the lifted impl-method `Instance` (or
concrete `drop_in_place::<SelfTy>` for the drop slot). The shims
bridge the type mismatch between the vtable slot's erased-
receiver function-pointer signature and the concrete-receiver
signature on the lifted impl method or rustc's `DropGlue`
Instance. The precise struct layout (`Ty`) for the vtable's
mixed-aggregate value is defined in stage 9/12 when vtable
lowering is implemented; stage 1 reserves the policy.

Pretty-printer rendering follows rustc's MIR-dump convention:
`crate::module::Item::<u32>` for `Instance { item: my_func,
generic_args: [Type(u32_ty)], ... }`.

### Bridge-synthesized Items

Some Instances arise from constructs the bridge synthesizes
rather than from source-level definitions. The principal case
is **trait-object vtables**: when MIR has a `dyn Trait` value,
the bridge synthesizes a Static `Instance` whose value is the
vtable struct. This Static needs an `Item`; rustc has no
source-level `DefId` for the vtable itself.

Bridge-synthesized Items live in `items` alongside source-level
items, with these construction rules:

- **`def_path`**: synthetic, under a reserved canonical form.
  For vtable items, the rendered form is `<vtable for `Trait`
  for `SelfTy`>` (rendering pinned by the pretty-printer). The
  internal `DefPathData` segment representation is pinned in
  stage 9/12 when vtable lowering is implemented; stage 1
  reserves the synthetic-item slot in `items`. **Note**:
  `DefPathData` is a closed 19-variant enum mirroring rustc.
  If stage 9/12 needs a *new* `DefPathData` variant for
  synthetic vtable scopes (rather than overloading an existing
  variant), that is a stage-1 revision: the variant is
  appended to `DefPathData`, the `def_path_data_sort_rank`
  table receives a new append-only entry (rank 19), and stage
  9/12's vtable lowering uses the new variant. Spec-mutability
  rule applies (Codex/external review of the variant addition
  before merge).
- **`def_path_hash`**: derived deterministically from the
  synthetic components. For vtable items: `hash((trait_def_path_hash,
  self_ty_canonical_hash))`. Stable across builds.
- **`crate_id`**: the `crate_id` of the crate that triggered
  the synthesis (typically the primary crate or the
  regseq-aware crate that constructed the trait object).

Construction invariant 2 (`item_def_paths` mirrors `items`)
extends to synthetic items: `item_def_paths[item_id]` for a
synthetic item is the bridge-derived `DefPathHash` defined
above.

Synthetic items are conceptually owned by the bridge; they
appear in all the same tables as source-level items so
consumers can walk `instances` uniformly without special-casing.

### Synthesized source-file sentinel

`source_files[0]` (i.e., `SourceFileId::ZERO`) is reserved for
the synthesized-source sentinel. The bridge populates it as the
first entry of the `source_files` table during construction:

```rust
SourceFile {
    path:        strings.intern("<synthesized>"),
    content:     None,
    line_starts: vec![0],
}
```

Real source files start at `SourceFileId(1)`. Spans on
bridge-synthesized items use `Span { file: SourceFileId::SYNTHESIZED,
lo: 0, hi: 0 }`. Consumers don't need `Option<Span>` checks: a
synthesized span resolves through `Span::slice` to `None`
naturally because the sentinel's `content` is `None`.

`SourceFileId::SYNTHESIZED` is a documentary alias for
`SourceFileId::ZERO` (the macro-generated index-0 constant from
`newtype_index!`):

```rust
impl SourceFileId {
    /// Sentinel for bridge-synthesized spans (no source location).
    /// Equal to `SourceFileId::ZERO`; the named form is preferred
    /// at use sites for readability.
    pub const SYNTHESIZED: SourceFileId = SourceFileId::ZERO;
}
```

This mirrors `StringId::ZERO == ""` -- both are stable index-0
sentinels reserved by construction.

## Top-level `RegseqModel`

The owned root struct returned by `bridge::extract(tcx) ->
RegseqModel`. All other tables hang off it.

```rust
pub struct CrateInfo {
    pub name:            StringId,        // e.g., "pl011-spec", "regseq"
    pub stable_crate_id: StableCrateId,   // rustc's StableCrateId (u64 wrapper)
}

pub struct RegseqModel {
    // Crate inventory (one entry per crate the model contains items from).
    pub crates:        IndexVec<CrateId, CrateInfo>,
    pub primary_crate: CrateId,                          // points at the spec crate

    // Interning tables (stage 1).
    // `source_files[0]` is the synthesized-source sentinel.
    pub strings:      StringInterner,
    pub source_files: IndexVec<SourceFileId, SourceFile>,

    // Type / const tables (stages 3-4).
    pub types:  IndexVec<TyId,    Ty>,
    pub consts: IndexVec<ConstId, Const>,

    // Source-level item alphabet (stage 12). Holds generic-form
    // definitions (DefPath, kind, attributes, span). NOT the
    // emission table; emission walks `instances`.
    // `item_def_paths` is the F4 cross-build-identity side table.
    pub items:          IndexVec<ItemId, Item>,
    pub item_def_paths: IndexVec<ItemId, DefPathHash>,

    // Function signatures (stage 11). Every `InstanceKind::Function`
    // has an entry; foreign declarations have `body: None` but still
    // have a sig.
    pub sigs:   IndexVec<FnSigId, FnSig>,

    // Bodies for body-carrying Instances (stage 11).
    pub bodies: IndexVec<BodyId, Body>,

    // The primary monomorphized table. Iteration target for
    // domain analyses and C codegen.
    pub instances: IndexVec<InstanceId, Instance>,
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
   `FnSigId`, `SourceFileId`, `StringId` that appears anywhere
   in the model resolves to a valid index in its table. In
   particular: every `Function.sig: FnSigId` resolves to a valid
   entry in `sigs`, including for foreign declarations. This is
   a debug-build assertion.
2. *`item_def_paths` mirrors `items`.* `item_def_paths.len() ==
   items.len()`, and for every valid `item_id`,
   `item_def_paths[item_id]` is a stable identity hash for
   `items[item_id]`:
   - For **source-level Items** (the common case):
     `item_def_paths[item_id]` is rustc's own `DefPathHash` of
     the originating `DefId`.
   - For **bridge-synthesized Items** (see "Bridge-synthesized
     Items"; principal case is trait-object vtables):
     `item_def_paths[item_id]` is the bridge-derived stable
     hash defined in that section (e.g., for vtable items:
     `hash((trait_def_path_hash, self_ty_canonical_hash))`).
   Both forms are stable across builds and globally unique.
   Stage 12 specifies the placement of `def_path` on `Item` but
   inherits this consistency rule.
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
7. *No `'tcx` references.* The model is fully owned and `'static`-
   compatible; serialization round-trips losslessly.
8. *Synthesized source-file sentinel.* `source_files[0]` is the
   synthesized sentinel: `path` interned as `"<synthesized>"`,
   `content: None`, `line_starts: vec![0]`. Real source files
   start at `SourceFileId(1)`. Bridge-synthesized Instances have
   `span.file == SourceFileId::SYNTHESIZED`.
9. *Instance closure-completeness.* The bridge computes the
   monomorphization closure from a settled root set: the full
   rustc mono-item closure for the primary crate plus all
   regseq-aware dependency crates (per Q6's
   `[package.metadata.regseq]` discovery), plus all synthesized
   artifacts those mono-items require (drop glue, vtable shims,
   etc., per rustc's `MonoItem` collection logic). The set is
   **bridge-determined and not consumer-influenced**; downstream
   consumers filter the resulting `instances` table for their
   own analysis-specific or codegen-specific roots. Every
   `InstanceId` referenced anywhere in the model resolves to a
   valid entry in `instances`; dangling `InstanceId`s are
   debug-assertion failures.
10. *Body presence matches kind.* For every `Instance` with
    `kind: InstanceKind::Function { body: Some(b) }`, `b`
    resolves to a valid `Body`. `body: None` is reserved for
    foreign declarations whose bodies are not in our crate set;
    the bridge does not synthesize empty bodies.

Invariants outside debug builds are enforced by construction; the
bridge produces the model in one pass and writes only valid indices.

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

### F4-2: Instance ordering

Instances are sorted at construction by the same `RegseqInstanceKey`
the bridge uses for interning (defined in "Instance" above):

```text
(regseq_kind_rank:  u8,        // 0..5; see `regseq_instance_kind_rank`
 item_id:           ItemId,    // already F4-sorted
 generic_args_key:  ArgsKey,
 fn_shim_disambig:  u8)        // 0 for non-Function; rustc kind rank for Function
```

`ArgsKey` is a deterministic fingerprint of `generic_args`:

- Type args compared by structural `Ty` representation
  (canonical form pinned in stage 3).
- Const args compared by canonical const-eval result (pinned in
  stage 4).
- Lifetimes erased; not part of the key.

Two builds of the same source produce identical
`RegseqInstanceKey` tuples, so `InstanceId` assignments are
stable across builds the same way `ItemId` assignments are.

The two rank tables (`regseq_instance_kind_rank` and
`rustc_instance_kind_sort_rank`) are pinned in the "Instance"
section above and are append-only. The `ArgsKey` form is
defined in stage 3 once `Ty`'s canonical-form rule is pinned;
stage 1 reserves the rule and the sort obligation.

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
| `Instance` | object `{ "item": u32, "generic_args": [...], "symbol_name": u32, "span": {...}, "kind": {...} }` | derive |
| `InstanceKind` | externally-tagged enum (serde default), e.g. `{"Function":{"sig":2,"body":7}}` / `{"Function":{"sig":2,"body":null}}` (foreign decl) / `{"Static":{"ty":3,"init":12}}` / `{"Struct":{"layout":4}}` | derive |
| `GenericArg` | externally-tagged enum, e.g. `{"Type":3}` / `{"Const":12}` | derive |
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

- `research.md` -- the evidence base; F1-F4 decisions and
  prior-art cluster summaries.
- `architecture.md` Q1 (two-crate split), Q2 (eager translation /
  internal IDs / self-describing IR), Q3 (single-level IR), Q5
  (insta + textual pretty-printer + serde JSON), Q6 (multi-crate
  strategy), Q7 (intrinsic recognition).
- `README.md` -- methodology rules, especially "no defensive
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
