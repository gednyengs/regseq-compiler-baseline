# MonoRust Overview

> **STATUS: HISTORICAL DESIGN REFERENCE**
>
> MonoRust as a separate IR is no longer the planned architecture. The
> Regseq compiler consumes rustc's `optimized_mir` (with
> `mir_opt_level=0`) directly into RegseqModel construction. The
> "post-monomorphization Rust IR" rationale that motivated MonoRust
> is satisfied by `optimized_mir` itself; materializing a separate
> in-memory representation produced no distinct value beyond
> independent testability, which on reflection just moves the same
> testing problem one layer up.
>
> This document is preserved because its inclusion principle
> (Section 4), the Rust-surface taxonomy (Sections 6 and 7), and the
> kept/gone-by-mono/rejected/invisible/dropped-silently classification
> (Sections 7.1-7.4) all still apply -- they describe what RegseqModel
> construction must decide when consuming `optimized_mir`. Only the
> materialization-as-separate-data-structure aspect is dropped.
>
> Where this document refers to "MonoRust extraction" or "the
> MonoRust artifact," read "RegseqModel construction consuming
> optimized_mir." The substantive design content is unchanged.
>
> See `docs/discussions/regseq-model/README.md` for the
> device-independent (linguistic) phase that owns RegseqModel
> specification and the construction pass, and
> `docs/discussions/manual-lowering/README.md` for phase-2 device
> pressure (golden-C authoring for pl011).

## 1. What MonoRust Is

MonoRust is a self-contained, post-monomorphization Rust intermediate
representation produced by the Regseq compiler from rustc's HIR/MIR.
It is the boundary between the rustc-dependent extraction phase and
the rustc-independent rest of the compiler.

Defining properties:

- **Post-monomorphization.** Every type and item that survives into
  MonoRust is concrete. No `TyKind::Param`, no generic items, no
  unresolved associated-type projections, no lifetimes, no
  `impl Trait`, no trait bounds. Each generic source item produces
  one MonoRust entry per concrete instantiation.
- **Faithful to Rust.** MonoRust represents what the Rust source
  actually says (post-substitution), not what the Regseq domain wants
  it to mean. There are no Regseq-specific operations, intrinsics,
  resource categories, or callback families. `#[rseq::*]` attributes
  are carried opaquely as generic attribute bags attached to items and
  attribute-bearing substructures.
- **Rustc-independent value.** A MonoRust artifact is a plain data
  structure with no reference to `TyCtxt`, `Span`, `DefId`, `ty::Ty`,
  or any other rustc surface. Once produced, it can be serialized,
  inspected, and consumed without rustc on the path. Serialization is
  an observability and testing mechanism, not a required compiler
  pipeline boundary: inside the compiler, RegseqModel construction
  consumes the in-memory MonoRust value directly. The producer
  (`regseq-driver`-style code) depends on rustc; the consumer
  (RegseqModel construction, golden-output codegen, analyses) does
  not.
- **No domain knowledge.** MonoRust knows nothing about driver
  structs, register blocks, intrinsics, ordered regions, ISR
  contexts, callback families, or platform binding. That entire
  vocabulary belongs to RegseqModel.
- **Mirrors rustc's structural model.** MonoRust's IR shape -- flat
  per-crate item indexing, modules-as-items, parent pointers,
  span-as-provenance, MIR-style CFG bodies -- follows rustc's
  design. Deviations exist only where MonoRust's use case has a
  load-bearing reason (owned types instead of arena-allocated
  `'tcx` values; `Symbol` for crate identity instead of `CrateNum`
  for self-readability and cross-artifact stability; rejection of
  constructs outside the inclusion principle; per-item-kind typed
  wrappers around the universal `ItemId`). Where rustc has a
  settled answer for a structural question, that's MonoRust's
  default; structural inventions need an explicit justification.

## 2. Position in the Pipeline

```
   rustc HIR/MIR
        |
        |  extraction (rustc-dependent)
        v
   MonoRust artifact
        |
        |  domain lowering (rustc-independent)
        v
   RegseqModel
        |
        |  platform join, analyses
        v
   Backends (Linux C, Zephyr C, ...)
```

MonoRust sits at the seam between two phases that have very different
concerns:

- The **extraction phase** is concerned with Rust semantics:
  monomorphization via `ty::Instance`, type substitution, closure
  resolution, normalization of associated types, walking the
  call graph from the extraction root set through `core` cross-crate
  code, rejecting unsupported Rust constructs (async, inline asm,
  unwinding, etc.). Its output is a faithful Rust subset materialized
  as MonoRust.
- The **domain-lowering phase** is concerned with Regseq semantics:
  recognizing intrinsic calls by attribute lookup, attaching register
  access metadata, folding `?`-patterns, identifying ordered regions,
  classifying ISR handlers, deriving abstraction/intrinsic dependency
  sets, populating resource manifests. Its input is a MonoRust value;
  its output is a `RegseqModel`.

Within the compiler process, the arrow from MonoRust to RegseqModel is
an in-memory handoff. A serialized MonoRust artifact may be emitted
alongside the compilation for inspection, golden tests, debugging, or
external tooling, but RegseqModel construction does not require a
serialize-then-deserialize step.

### 2.1 Extraction roots and reachability

MonoRust extraction is not purely "start at `main` and follow calls."
Driver specs contain declarations that are semantically important even
when no authored Rust body calls them directly; generated OS glue will
eventually call Regseq callbacks and ISR handlers.

The root set therefore has two categories:

- **Declaration roots.** The entry crate's module tree, the driver
  module, driver struct, resource/property/user-data fields and their
  attributes, Regseq trait impls for the driver, driver impl blocks,
  associated constants, register-block macro expansion output, and
  type definitions needed to reconstruct the declaration side of
  RegseqModel.
- **Execution roots.** Regseq trait methods, `#[rseq::isr]` handlers,
  helper methods reachable from those bodies, free functions reachable
  from those bodies, closure bodies reachable through monomorphization,
  and cross-crate `core`/`regseq` bodies reached from those calls.

Extraction also takes the transitive closure of types, constants,
attributes, signatures, trait refs, impl metadata, and body operands
needed by those roots. The exact traversal algorithm is deliberately
left to the extraction design, but the MonoRust artifact must contain
enough information for RegseqModel construction without going back to
rustc.

TODO: specify the precise extraction-root and reachability algorithm.
The algorithm needs to account for both call-graph-discoverable code
and declarations that are only discovered structurally from the driver
spec.

## 3. Why MonoRust Exists

Four motivations, in roughly decreasing order of practical impact:

### 3.1 Codegen bootstrapping

A serializable MonoRust artifact lets driver authors and backend
implementers hand-write golden C/Rust/C++ output corresponding to
what the compiler is *expected* to produce, run those golden outputs
in QEMU, and validate end-to-end behavior before the rest of the
pipeline (RegseqModel construction, analyses, backend codegen) is
fully implemented. This serialized form is optional observability
output; the normal compiler pipeline builds RegseqModel from the
in-memory MonoRust value. The hand-author mentally applies the same
domain transformations the compiler will eventually automate; when the
compiler eventually produces output from the same MonoRust input, it
should match. This de-risks both the IR design and the backend
contract.

### 3.2 Separation of concerns

Monomorphization and domain lowering are fundamentally different
transformations. Tangling them inside a single pass produces an error
taxonomy that mixes Rust-shape failures (`UnsupportedRustType`,
`UnresolvableGenericCall`, `StdTypeNotPermitted`) with domain-shape
failures (`CrossBlockFieldUsage`, `WrongRegisterAccessKind`,
`UnknownIntrinsicCanonicalName`) in the same enum. With MonoRust as a
materialized boundary, each phase has its own diagnostic surface, its
own verifier, and its own testable inputs and outputs.

### 3.3 Independent testability

The rustc-dependent extraction phase requires the full rustc driver
plumbing to test. The rustc-independent rest of the compiler does
not. Once MonoRust is the input shape for RegseqModel construction,
analyses, and backend codegen, those layers can be tested with
hand-built MonoRust artifacts -- no rustc, no `TyCtxt`, no driver
fixture needed.

### 3.4 Frontend optionality

If a future Regseq frontend ever consumes a non-Rust source language
(e.g., a hypothetical embedded DSL or a C frontend with Regseq
attributes), only the extraction phase needs replacing; the rest of
the compiler stays unchanged. This is not a near-term concern but the
boundary is free if MonoRust exists for the other reasons.

## 4. Inclusion Principle

The single rule that determines what MonoRust represents:

> A Rust construct belongs in MonoRust if it could plausibly appear
> in `#![no_std]` driver spec source, or in cross-crate `core` MIR
> reached from one, after monomorphization.

This is broader than what RegseqModel currently accepts. RegseqModel
narrows further; MonoRust is the durable Rust-faithful boundary. When
RegseqModel grows to support new constructs (e.g., closures for
`PollUntil` predicates), MonoRust does not change -- it was already
complete.

The inclusion test cuts in two directions:

- A construct that monomorphizes away (generic parameter, lifetime,
  trait bound, `TyKind::Param`) is not "rejected" -- it is **gone by
  the time MonoRust would represent it**. MonoRust's job is to
  describe what survives substitution, not what existed before.
- A construct that requires assumptions a portable spec cannot make
  (inline asm, async runtime, unwinding) is **rejected** -- MonoRust
  refuses to represent it and extraction errors out.

## 5. Hierarchical Organization

MonoRust's hierarchy follows rustc's: an artifact contains crates,
each crate carries a flat per-crate item list, and modules are
themselves items whose `Module` kind lists their children in source
order. Crate boundaries are first-class.

```
MonoArtifact
├── entry_crate: Symbol
└── crates: BTreeMap<Symbol, MonoCrate>

MonoCrate
├── name: Symbol
├── items: Vec<MonoItem>            // flat per crate, indexed by item_index
└── root_item: ItemId               // points to the crate-root item (kind = Module)

MonoItem
├── item_id: ItemId
├── parent: Option<ItemId>          // None for crate root
├── name: Symbol                    // empty for crate root
├── mono_args: Vec<Symbol>          // generic args of this monomorphization (empty = non-generic)
├── vis: Visibility                 // Public | Restricted(ItemId) of the limiting module
├── source: SourceOrigin            // declaration site
├── attrs: Vec<RawAttr>             // raw `#[...]` attributes, opaque to MonoRust
└── kind: MonoItemKind

MonoItemKind
├── Module(MonoModule)
├── Struct(MonoStruct)
├── Enum(MonoEnum)
├── Union(MonoUnion)
├── Trait(MonoTrait)
├── Impl(MonoImpl)
├── Function(MonoFn)
├── Const(MonoConst)
├── Static(MonoStatic)
└── TyAlias(MonoTyAlias)

MonoModule
└── items: Vec<ItemId>              // children, in source order
```

### 5.1 Crates are first-class

Each crate the spec depends on -- the entry crate (the spec being
compiled) plus `core`, `regseq`, `regseq_macros`, and any
author-provided dep crates -- appears as a sibling `MonoCrate`
entry. Crates are not synthesized as submodules of one another.
The `MonoArtifact.entry_crate` field names which entry in `crates`
is the spec being compiled; everything else is a dependency crate.

Dependency crates have their contents in MonoRust restricted to
items reached transitively from the entry crate's bodies after
monomorphization. The data structure does not pretend a dependency
crate is fully represented.

### 5.2 Source provenance

Every `MonoItem` carries its declaration-site source location
inline:

```rust
pub enum SourceOrigin {
    Source(SourceLocation),
    CompilerGenerated,
}

pub struct SourceLocation {
    pub file: Symbol,           // file path, interned through the artifact's SymbolInterner
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}
```

`SourceOrigin::Source(_)` covers authored items; `CompilerGenerated`
is the explicit sentinel for items the compiler synthesizes (e.g.,
synthetic modules introduced for cross-crate items). Inline on
`MonoItem` rather than a side `BTreeMap<ItemId, SourceOrigin>`
table -- every item has provenance, so there's no missing-entry
case to optimize for, and one struct keeps construction and access
direct.

Each entry in an item's `attrs: Vec<RawAttr>` bag carries its own
`SourceOrigin` for attribute-level diagnostics (so `#[rseq::clock(id
= "ref")]` can be attributed precisely).

Attribute-bearing substructures also carry their own source location
and `attrs: Vec<RawAttr>` where Rust permits attributes. This includes
struct fields, union fields, enum variants, enum variant fields,
function parameters, trait items, impl items, and local declarations.
RegseqModel construction depends on this for field-level Regseq
properties such as `#[rseq::clock(...)]`, `#[rseq::mmio(...)]`,
`#[rseq::property(...)]`, and ISR parameter diagnostics.

Per-MIR-node provenance (per-statement, per-terminator, per-local)
lives on body structures (a body source map paralleling each
function's CFG), not on `MonoItem` itself. That is a body-level
concern, covered separately when the body shape is detailed.

File identity is metadata attached to spans, not part of the
structural hierarchy. Two modules in the same file are still two
distinct `MonoItem` entries; a single module declared with
`#[path = "alt.rs"]` still produces a single `MonoItem` entry
whose source `file` is the chosen path. File boundaries are an
organizational choice in source code that has no semantic effect
on the compiled output.

### 5.3 Item identity

Every entity in MonoRust is referenced through a single typed
identifier:

```rust
pub struct ItemId {
    pub crate_name: Symbol,
    pub item_index: u32,             // dense per crate
}
```

Per-kind typed wrappers distinguish item categories without
introducing parallel id systems:

```rust
pub struct FunctionId(pub ItemId);
pub struct TypeDefId(pub ItemId);
pub struct ConstantId(pub ItemId);
pub struct ModuleId(pub ItemId);
pub struct TraitId(pub ItemId);
// ...
```

This mirrors rustc's `DefId = (CrateNum, DefIndex)`. Two
deviations: `Symbol` for the crate axis (instead of `CrateNum`), and
typed per-kind wrappers (instead of bare `DefId`).

Properties:

- **One id type.** `ItemId` is the universal handle. Modules are
  items, addressed by `ItemId`, the same as functions and types.
  The dual-id ambiguity ("ItemId or ModuleID for this module?")
  does not arise.
- **Self-readable.** `Symbol` resolves to its string through the
  artifact's interner, so a serialized id renders as
  `regseq::core::option::Option::map`-style text without external
  lookups.
- **Cross-artifact stable.** Ids are anchored on the crate's
  `Symbol`, so the same item in two builds of the same crate keeps
  the same id (subject to deterministic local item ordering).
- **Append-only construction.** Allocating a new item appends to
  `crate.items` and yields the next `item_index`. Existing ids
  never shift.
- **O(1) lookup.**
  ```rust
  let item = &artifact.crates[&id.crate_name].items[id.item_index as usize];
  ```
- **Containing module via parent.** `MonoItem.parent: Option<ItemId>`
  reaches the parent. For an authored item, the parent is the
  module that declares it. For a module, the parent is its
  enclosing module (or `None` for the crate root).
- **Source order via `MonoModule.items: Vec<ItemId>`.** A module's
  declarations -- functions, types, sub-modules, constants, and so
  on -- are listed in source order in the parent module's
  `MonoModule.items`. Walking a module's contents in source order
  is `for child_id in &mono_module.items { ... }` followed by an
  O(1) lookup per child.

The flat per-crate `items` Vec gives best cache locality for
crate-wide walks and direct array indexing for ID-based lookup.
Tree-shaped traversals descend by following each module's
`items: Vec<ItemId>` and looking up each child.

TODO: strengthen serialized identity. The current `crate_name:
Symbol` plus dense `item_index` shape is useful for in-memory lookup,
but crate names are not globally unique and dense indices are fragile
across extraction-order changes. A later design discussion should
decide how serialized MonoRust identities encode a stable crate
identity and a stable source/monomorphization identity, such as a
def-path plus monomorphization fingerprint, while preserving dense IDs
for internal lookup.

## 6. Rust Surface In Scope

MonoRust represents the following categories within the hierarchy
above. Subsections cover items, bodies, types, authoring metadata,
and ABI surface; further per-construct detail (full enum
definitions, exhaustive cross-references) lives in dedicated
documents.

TODO: narrow or stratify the accepted Rust surface. The in-scope set
below is intentionally broader than RegseqModel's current C-oriented
surface, but constructs such as floats, raw pointers, function
pointers, closures, trait objects, `transmute`, unions, and SIMD-ish
reprs should not silently become backend commitments. The design
should classify each construct as: accepted by MonoRust and lowerable
to RegseqModel, accepted by MonoRust only so trusted `core`/`regseq`
dependencies can be represented, or rejected for authored driver
specs.

### 6.1 Items

- Modules
- Structs (named-field, tuple, and unit variants)
- Enums (with named, tuple, and unit variants; with explicit and
  implicit discriminants; including `#[repr(...)]` shapes)
- Unions
- Traits (declarations -- methods, associated types, super-traits)
- Inherent and trait impl blocks
- Free and associated functions, including `unsafe fn`,
  `extern fn`, and methods
- Constants and statics, including associated constants
- Type aliases (resolved form -- the alias item is preserved for
  diagnostics; references to it see the resolved underlying type)
- Macro-expanded items in their post-expansion form

Items also carry the modifier set rustc records on them: visibility
(`Public` or `Restricted(ItemId)` of the limiting module), safety
where applicable (`unsafe trait`, `unsafe fn`, `unsafe` field
qualifier on union fields), `IsAuto` for traits, mutability for
statics, default-field-value expressions where present, and
`mono_args: Vec<Symbol>` recording the generic arguments of each
monomorphization (empty for non-generic source items). Per-impl
modifiers like `Constness`, `Defaultness` (specialization), and
`ImplPolarity::Negative` are dropped -- post-monomorphization they
either no longer apply (`Constness`, `Defaultness`) or are out of
scope by the inclusion principle (`Negative` impls).

TODO: clarify "post-monomorphization" for traits. Function bodies and
ADT/function types are represented per concrete instantiation, but
trait declarations are metadata rather than monomorphized executable
items in the same sense. The design needs to decide whether MonoRust
keeps source-like trait declarations for diagnostics/classification,
instantiated trait refs and impl metadata only, or both, and how this
interacts with associated types and supertraits.

### 6.2 Bodies

A function body in MonoRust is a CFG of basic blocks, mirroring the
shape of MIR after the extraction stage selected for MonoRust. The
categories of body content represented:

**Locals.** Local variable declarations with concrete types and
mutability. Index 0 is the return place; indices `1..=arg_count`
are the function arguments; the remainder are user variables and
compiler temporaries.

**Basic blocks.** Each block contains an ordered sequence of
statements followed by exactly one terminator.

**Statements** (kept):
- `Assign(Place, Rvalue)`
- `SetDiscriminant { place, variant_index }` -- write the enum tag
- `StorageLive(Local)` / `StorageDead(Local)` -- stack lifetime
  markers
- `Intrinsic(NonDivergingIntrinsic)` -- two sub-variants:
  `Assume(Operand)` and `CopyNonOverlapping { src, dst, count }`

**Terminators** (kept):
- `Goto(BasicBlock)`
- `SwitchInt { discr: Operand, targets: SwitchTargets }`
- `Call { func: Operand, args, destination, target }`
- `Assert { cond: Operand, expected: bool, msg: AssertMessage,
  target: BasicBlock }`
- `Drop { place, target }` (the unwind / replace / async-future
  fields are not represented)
- `Return`
- `Unreachable`

`Call` is Rust-faithful and has no Regseq-specific `error_target`.
Fallible `Result`-returning calls and the `?` operator remain ordinary
MIR control flow over concrete values; RegseqModel lowering may later
recognize and fold those patterns into domain-level error edges.

The MIR `Assert` terminator (overflow checks, bounds checks, division
preconditions, and related runtime checks) is preserved. It is not
collapsed to the success target, because doing so would erase Rust
semantics before RegseqModel has a chance to decide whether the check
is supported, rejected, or lowered. All `unwind: UnwindAction` fields
on terminators follow the no-unwinding policy; cleanup-block
subdivision does not exist in MonoRust.

**Rvalues** (kept):
- `Use(Operand)`
- `Repeat(Operand, usize)` -- array repeat `[x; N]`, with `N`
  concrete after monomorphization
- `Ref(BorrowKind, Place)` -- region-erased; `BorrowKind` collapses
  to `Shared | Mutable` (the borrow-checker's `Fake(_)` and
  `MutBorrowKind` sub-variants are not surfaced)
- `RawPtr(Mutability, Place)` -- raw pointer creation
  (`&raw const`/`&raw mut`)
- `Cast(CastKind, Operand, MonoType)`
- `BinaryOp(BinOp, Operand, Operand)`
- `UnaryOp(UnOp, Operand)`
- `Discriminant(Place)` -- read enum tag
- `Aggregate(AggregateKind, Vec<Operand>)`

**Cast kinds** (kept): `IntToInt`, `FloatToInt`, `FloatToFloat`,
`IntToFloat`, `PtrToPtr`, `FnPtrToPtr`, `Transmute`,
`PointerExposeProvenance`, `PointerWithExposedProvenance` (the
Strict Provenance API for raw-address handling), and
`PointerCoercion` (with sub-variants `Reify`, `ClosureFnPointer`,
`UnsafeFnPointer`, `MutToConstPointer`, `ArrayToPointer`, `Unsize`,
`DynStar`).

**Aggregate kinds** (kept): `Array(MonoType)`, `Tuple`,
`Adt(TypeDefId, VariantIdx, Option<FieldIdx>)` (the field index is
the active field for union construction), `Closure(FunctionId)`,
`RawPtr(MonoType, Mutability)`.

**Operands** (kept):
- `Copy(Place)`
- `Move(Place)`
- `Constant(ConstValue)` -- a `ConstValue` is either an inline
  literal scalar/aggregate or a reference to a named constant item
  (`ConstantId`). Distinct from `MonoConst`, which is the *item*
  kind for `const` declarations.

The MIR `Operand::RuntimeChecks(_)` variant (which queries session
flags like `UbChecks`/`OverflowChecks`/`ContractChecks`) is
resolved at extraction time to a concrete `Constant(Bool(_))` based
on the build's session flags; it does not appear as a MonoRust
operand kind.

**Places.** A `Local` plus a chain of projections. Projections
kept: `Deref`, `Field(FieldIdx, MonoType)`, `Index(Local)`,
`ConstantIndex { offset, min_length, from_end }`, `Subslice
{ from, to, from_end }`, `Downcast(VariantIdx, Symbol)`.

**Operators.** All MIR `BinOp` variants are kept (`Add`,
`AddUnchecked`, `AddWithOverflow`, `Sub`, `SubUnchecked`,
`SubWithOverflow`, `Mul`, `MulUnchecked`, `MulWithOverflow`,
`Div`, `Rem`, `BitXor`, `BitAnd`, `BitOr`, `Shl`, `ShlUnchecked`,
`Shr`, `ShrUnchecked`, `Eq`, `Ne`, `Lt`, `Le`, `Gt`, `Ge`, `Cmp`,
`Offset`). All `UnOp` variants are kept (`Not`, `Neg`,
`PtrMetadata`).

**Closures.** Closure types and `AggregateKind::Closure` are
in scope. The closure's captures form an ADT; the closure body is
a `MonoFn` reachable through monomorphization.

**Drop semantics.** Explicit `Drop` terminators are represented when
they survive into the selected MIR stage. No coroutine-drop /
async-drop machinery.

TODO: decide Drop semantics before finalizing the body contract.
Arbitrary user-defined `Drop` side effects are a significant semantic
commitment for a driver-spec compiler. The design should decide
whether authored specs reject nontrivial `Drop`, whether MonoRust
represents drop glue as reachable callable bodies, and how any kept
drop behavior maps into RegseqModel.

**Source provenance.** A body source map carries per-statement and
per-terminator `SourceOrigin` values parallel to the basic-block
contents, plus per-local origins parallel to the local declaration
list.

### 6.3 Types

MonoRust's type system mirrors a subset of `ty::TyKind` with all
generics-related variants removed by monomorphization. Every
`MonoType` value is concrete -- no `Param`, no `Alias`, no
unresolved projections.

**Primitives** (kept):

- `Bool`, `Char`, `Never`, `Unit`, `Str`
- `Int(IntKind)` -- `I8`, `I16`, `I32`, `I64`, `I128`, `Isize`
- `Uint(UintKind)` -- `U8`, `U16`, `U32`, `U64`, `U128`, `Usize`
- `Float(FloatKind)` -- `F16`, `F32`, `F64`, `F128`

`Unit` and `Never` are distinct variants (rather than `Tuple([])`
and reached through `!`'s rendering), for clearer pattern-matching
at consumer sites. `Str` is the bare `str` pointee, which appears
behind references in practice.

**Compound** (kept):

- `Adt(TypeDefId)` -- struct, enum, or union, referenced by its
  monomorphic `TypeDefId`. `Option<u32>` and `Option<u64>` are
  distinct `TypeDefId`s, each with its own `MonoItem` entry whose
  `mono_args` records the rendered argument forms for diagnostics.
- `Array(MonoType, u64)` -- `[T; N]` with `N` concrete
- `Slice(MonoType)` -- `[T]`
- `Tuple(Vec<MonoType>)` -- the empty tuple is `Unit`, not
  `Tuple(vec![])`
- `RawPtr(MonoType, Mutability)` -- `*const T` / `*mut T`
- `Ref(MonoType, Mutability)` -- `&T` / `&mut T`, region-erased

**Function-shaped** (kept):

- `FnDef(FunctionId)` -- the anonymous "named function" type
  carried by every function item. The `FunctionId` identifies the
  specific monomorphization, so generic args are implicit (the
  rustc shape `FnDef(DefId, GenericArgs)` is collapsed).
- `FnPtr(FnSig)` -- a function pointer; `FnSig` carries the input
  types, output type, ABI, and safety
- `Closure(FunctionId)` -- closure type. The closure's body and
  capture struct are reachable through the `FunctionId`.

**Trait objects** (kept):

- `Dyn { principal: TraitRef, auto_traits: Vec<TraitId> }` --
  `dyn Trait + AutoTrait + ...`, region-erased. `principal`
  carries the trait id and its monomorphic args; `auto_traits` is
  the (possibly empty) list of auto-trait ids (`Send`, `Sync`,
  `Unpin`, etc.). `TraitRef { trait_id: TraitId, args:
  Vec<MonoType> }` is the monomorphic trait reference.

The `(DefId, GenericArgs)` pair pattern that rustc uses to address
a specific monomorphization (`Adt(AdtDef, GenericArgs)`,
`FnDef(DefId, GenericArgs)`, `Closure(ClosureId, GenericArgs)`) is
collapsed in MonoRust: post-mono, the `TypeDefId` or `FunctionId`
identifies the monomorphization directly, with `MonoItem.mono_args`
recording the rendered argument forms for diagnostics and naming.

### 6.4 Authoring metadata

**Visibility.** Each `MonoItem` carries a `vis: Visibility` field
mirroring rustc's `ty::Visibility<DefId>`:

```rust
pub enum Visibility {
    Public,
    Restricted(ItemId),     // ItemId of the limiting module
}
```

`Public` covers `pub` items; `Restricted(ItemId)` covers
`pub(crate)`, `pub(super)`, `pub(in path::to::module)`, and bare
private items. The `ItemId` always refers to a module:
`pub(crate)` points at the crate root, `pub(super)` at the
parent module, `pub(in path)` at the named module, and bare
private at the enclosing module.

**Attributes.** Each `MonoItem` and each attribute-bearing
substructure carries `attrs: Vec<RawAttr>` -- an opaque attribute bag
with no MonoRust-side interpretation. `RawAttr` captures the
post-expansion HIR attribute structure in a uniform form rich enough
for downstream consumers (RegseqModel construction, golden codegen,
analyses) to interpret semantic attributes (`#[rseq::*]`,
`#[derive(...)]`, `#[repr(...)]`, `#[link_name = "..."]`, etc.)
without re-parsing source text.

```rust
pub struct RawAttr {
    pub path: Vec<Symbol>,           // e.g., ["rseq", "clock"], ["doc"], ["derive"]
    pub args: AttrArgs,
    pub source: SourceOrigin,
}

pub enum AttrArgs {
    /// `#[path]`
    None,
    /// `#[path = literal]`
    Eq(AttrLit),
    /// `#[path(...)]` -- comma-separated argument list
    List(Vec<AttrArg>),
}

pub enum AttrArg {
    /// A bare literal: `"foo"`, `42`, `true`
    Lit(AttrLit),
    /// An identifier: e.g., `Debug` in `#[derive(Debug)]`
    Ident(Symbol),
    /// A path: e.g., `serde::Serialize` in `#[derive(serde::Serialize)]`
    Path(Vec<Symbol>),
    /// A key-value: e.g., `id = "ref"` in `#[rseq::clock(id = "ref")]`
    KeyValue { name: Symbol, value: AttrLit },
    /// An array literal: e.g., `["arm,pl011", "arm,primecell"]`
    Array(Vec<AttrArg>),
    /// A nested attribute-shaped invocation: e.g.,
    /// `cfg(target_arch = "x86_64")`
    Nested { path: Vec<Symbol>, args: Box<AttrArgs> },
}

pub enum AttrLit {
    Str(Symbol),
    Int(i128),
    Uint(u128),
    Float(Symbol),                   // source-text symbol (no f64 precision issues)
    Bool(bool),
    Char(char),
    ByteStr(Vec<u8>),                // `b"..."` literals; rare in attrs
}
```

Both compiler-known parsed attributes (rustc's
`Attribute::Parsed(AttributeKind)`) and tool/custom unparsed
attributes (`Attribute::Unparsed(AttrItem)`) are projected into
this uniform shape during extraction. For Parsed attributes we
extract the structured fields rustc already broke out; for
Unparsed attributes we parse the `Delimited(TokenStream)` form
ourselves into `AttrArg` shape.

Each `RawAttr` carries its own `SourceOrigin` so attribute-level
diagnostics can point precisely (e.g., "the `id` argument of this
`#[rseq::clock]` attribute").

**Source locations.** Each `MonoItem` carries `source: SourceOrigin`
for its declaration site (Section 5.2). Attribute-bearing
substructures carry their own `source` as well, so diagnostics for
field-level resources, enum variants, function parameters, and local
declarations point at the authored construct rather than only at the
enclosing item. Per-MIR-node source provenance lives on body
structures (Section 6.2), not at the item level.

### 6.5 ABI surface

**Function ABI.** Each `MonoFn` and each `FnPtr` type carries an ABI
on its signature:

```rust
pub struct FnSig {
    pub inputs: Vec<MonoType>,
    pub output: MonoType,
    pub abi: Symbol,                  // canonical ABI name: "Rust", "C", "system", ...
    pub safety: Safety,
    pub c_variadic: bool,
}
```

`abi: Symbol` carries the canonical ABI name as a string, mirroring
how `extern "<abi>"` is written in source. MonoRust does not enumerate
rustc's `ExternAbi` (~25 variants, most arch-specific) as a typed
enum: backends match the strings they care about (typically `"Rust"`
and `"C"`), and future ABIs added by rustc do not require MonoRust
enum changes.

The `unwind: bool` qualifier rustc carries on some ABI variants
(`C { unwind: true }`, `System { unwind: true }`, etc.) is dropped --
driver specs do not unwind.

**Type repr.** Each `MonoStruct`, `MonoEnum`, and `MonoUnion` carries
a `repr: ReprOptions` field describing the effective layout choice:

```rust
pub struct ReprOptions {
    pub kind: ReprKind,
    pub int: Option<IntKind>,         // enums: explicit int repr (e.g., #[repr(u8)])
    pub align: Option<u32>,           // bytes; from #[repr(align(N))]
    pub pack: Option<u32>,            // bytes; from #[repr(packed(N))]
}

pub enum ReprKind {
    Default,                          // Rust default layout (field reordering allowed)
    C,                                // #[repr(C)]: source-order, C-compatible layout
    Transparent,                      // #[repr(transparent)]: single-field passthrough
    Simd,                             // #[repr(simd)]
}
```

Combined forms work naturally: `#[repr(C, packed)]` is `kind = C,
pack = Some(1)`; `#[repr(C, u32)]` on an enum is `kind = C,
int = Some(U32)`.

This mirrors rustc's `ReprOptions` minus seldom-used flags
(`IS_LINEAR`, `IS_SCALABLE`, `scalable: ScalableElt`,
`field_shuffle_seed`) that have no driver use case.

**Other ABI-affecting attributes.** Attributes that control external
symbol naming, visibility, and placement -- `#[no_mangle]`,
`#[export_name = "..."]`, `#[link_section = "..."]`, `#[used]` --
flow through the generic `attrs: Vec<RawAttr>` bag (Section 6.4).
MonoRust does not special-case them; backends inspect them as
needed.

**`#[link(...)]` and `#[link_name = "..."]`** are implicitly out of
scope: foreign items (the only valid targets for these attributes)
are rejected (Section 7.2), so the attributes have nothing to
attach to in MonoRust.

## 7. Rust Surface Out of Scope

The following are absent from MonoRust, organized by why they are
absent.

### 7.1 Removed by monomorphization (gone, not rejected)

Item-level:

- Generic parameters on items (`<T>`, `<const N>`, `<'a>`)
- Where-clauses, trait bounds, type bounds, and `impl Trait` in any
  position (return-position, argument-position)
- Higher-ranked types (`for<'a> ...`)

Type-level (`ty::TyKind` variants that do not survive
monomorphization):

- `TyKind::Param` -- a generic type parameter
- `TyKind::Bound` -- HRTB-introduced bound type variable
- `TyKind::Placeholder` -- higher-ranked subtyping placeholder
- `TyKind::Infer` -- type inference variable
- `TyKind::Alias` in all four sub-kinds (`Projection`, `Opaque`,
  `Inherent`, `Free`) -- associated-type projections, opaque-type
  return positions, free type aliases. All resolve to concrete
  types post-monomorphization.

Lifetime-level:

- Lifetimes and regions on references and elsewhere (region-erased)

These constructs are properties of the Rust *source* and *unsubstituted*
MIR. They have no meaning post-monomorphization and would not be
representable in a faithful MonoRust even if we wanted them.

The reference distinction `&T` vs `&mut T` is preserved -- it is
semantically meaningful for codegen even though the lifetime is not.

### 7.2 Rejected by the inclusion principle

These constructs could in principle appear in MIR but are excluded
because they violate the portable-`#![no_std]`-driver constraint. An
extraction encountering one of them is a hard error.

- **Async / coroutines / generators.** No driver runtime, no
  scheduler, no executor. Includes `Yield` and `CoroutineDrop`
  terminators, `AggregateKind::Coroutine`, async functions, async
  blocks, generator blocks, and `CoroutineClosure` aggregate.
- **Inline assembly (`asm!`).** Defeats target portability; spec
  authors use Regseq intrinsics instead (timing, barriers, etc.).
  Includes naked functions.
- **Module-level inline assembly (`global_asm!`).** HIR variant
  `ItemKind::GlobalAsm`. Same rationale as `asm!`.
- **Trait aliases (`trait Foo = Bar + Baz`).** HIR variant
  `ItemKind::TraitAlias`. Currently gated by
  `feature(trait_alias)`; no driver use case under the spec's
  prefer-static-dispatch posture.
- **Foreign items (contents of `extern { ... }` blocks).** HIR
  variants `ForeignItemKind::Fn`, `ForeignItemKind::Static`,
  `ForeignItemKind::Type`. Driver specs do not declare FFI
  surfaces directly -- all OS-side bindings flow through Regseq
  abstractions and intrinsics. Distinct from `extern "C" fn` items
  with bodies, which are still in scope (they are regular `Fn`
  items whose `header.abi` is `"C"`).
- **Negative trait impls (`impl !Trait for Type`).** Captured by
  `ImplPolarity::Negative(_)`. Unstable; no driver use case.
- **Unwinding.** Drivers do not unwind. Cleanup-block subdivision is
  rejected rather than represented as normal control flow. No unwind
  tables, no `panic = "unwind"`.
- **Tail calls.** The `TailCall` terminator. Currently unstable in
  Rust and not used by drivers.
- **Thread-locals.** `Rvalue::ThreadLocalRef(_)` and
  `#[thread_local] static` items. No driver use case; per-CPU /
  per-thread state goes through Regseq abstractions when needed.
- **Heap allocation (`Box`, `alloc::*`).** The `Rvalue::ShallowInitBox`
  rvalue and any reference to the `alloc` crate. Driver specs are
  no-alloc.
- **Unsafe binders.** `Rvalue::WrapUnsafeBinder` and
  `ProjectionElem::UnwrapUnsafeBinder`. Nightly-only feature with
  no driver use case.
- **Pattern types (`TyKind::Pat`).** Experimental nightly feature
  (`feature(pattern_types)`) that restricts a type's valid value
  range; no driver use case.
- **Compiler error placeholders (`TyKind::Error`).** Indicates the
  rustc compilation itself reported an error. Extraction errors
  out if encountered -- there is no valid MonoRust output for an
  ill-typed input.

### 7.3 Invisible at the MIR level

These constructs either (a) desugar to other constructs before
reaching MIR, or (b) appear in HIR as resolution / expansion
scaffolding but produce no MIR. MonoRust does not represent them
because there is no MIR-level signal corresponding to them.

Source-level constructs that desugar before MIR:

- Source-level pattern syntax (`match`, `if let`, `while let`,
  `let else`, `let chains`)
- `try` blocks (the `?` operator survives as a structural pattern in
  MIR, but the `try { ... }` block syntax does not)
- Macro invocations (only their expansion is visible). MonoRust does
  not preserve source macro calls. Regseq macros must therefore expand
  into regular Rust items, fields, attributes, constants, calls, and
  marker functions that leave all breadcrumbs needed by RegseqModel.
- `for` loops (lowered to `loop` + `Iterator::next`)
- Range expressions (lowered to `Range` struct construction)
- Closures' source syntax (lowered to anonymous structs + functions;
  MonoRust represents the lowered form)

HIR scaffolding that produces no MIR:

- **`use` items.** HIR variant `ItemKind::Use`. Paths in MIR are
  already resolved to absolute form, so `use` items carry no
  IR-level information once name resolution has run.
- **Macro definitions (`macro_rules!`, `macro`).** HIR variant
  `ItemKind::Macro`. Macro invocations were expanded before HIR
  lowering; the surviving definition emits no MIR and has no
  downstream consumer.

### 7.4 Compiler-internal MIR artifacts (dropped silently)

These MIR constructs exist to support compiler internals (borrow
checking, code coverage, runtime tracking, panic emission) and have
no semantic effect on the execution MonoRust represents. MonoRust
drops them without comment.

Statements:

- `FakeRead`, `PlaceMention`, `AscribeUserType` (borrow-checker
  hints; "disallowed after drop elaboration")
- `Retag` (Stacked Borrows / Tree Borrows runtime)
- `Coverage` (`-Cinstrument-coverage` instrumentation)
- `ConstEvalCounter`, `Nop` (no semantic effect)
- `BackwardIncompatibleDropHint` (Edition-2024 lint hint;
  semantically `Nop`)

Terminators:

- `FalseEdge`, `FalseUnwind` (borrow-checker artifacts)

Rvalues:

- `CopyForDeref(Place)` (lowered to `Use(Copy(_))` in runtime MIR;
  if encountered, treated as a normal copy)

Cast kinds:

- `Subtype` (subtype-marker cast with no runtime effect)

Place projections:

- `OpaqueCast(Ty)` (deprecated under `-Znext-solver`; produces no
  MonoRust signal)

Borrow-kind sub-variants collapsed:

- `BorrowKind::Fake(FakeBorrowKind::Shallow | Deep)` -- borrow-
  checker artifacts; collapsed to `Shared` if encountered.
- `MutBorrowKind::Default | TwoPhaseBorrow | ClosureCapture` --
  diagnostic-only sub-variants of mutable borrow; collapsed to a
  single `Mutable` classifier.

## 8. Constructs Not Yet Decided

The following items remain open and need explicit resolution before
per-construct details land:

- **THIR.** The typed HIR layer between HIR and MIR is not normally
  consumed for codegen. Whether MonoRust's design needs to consider
  THIR specifically (versus just HIR + MIR) is open.
- **Drop glue elaboration.** Whether MonoRust extracts the
  pre-elaboration MIR (`mir_drops_elaborated_and_const_checked`) or
  the post-elaboration form, and what the implications are for
  representing user-defined `Drop` impls. This needs resolution before
  finalizing MonoRust bodies because arbitrary destructor side effects
  are a significant semantic commitment for a portable driver-spec
  compiler.
