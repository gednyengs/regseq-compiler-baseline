# Stage 3 RegseqModel Spec Snapshot

> **Snapshot at close of stage 3.** Frozen historical artifact
> preserving stage 3's contribution at its close, including the
> full audit-shape decision text (Decision / Reasoning /
> Methodology rules invoked / Prior-art evidence / Worked
> example) for every locked decision. The current RegseqModel
> spec lives in top-level [`../spec.md`](../spec.md). D14
> (renderer style) was deferred to stage 13 (type-system
> pretty-printing).

## 0. Document status

This document is the canonical stage-3 type-system spec. Decisions
are locked progressively through discussion; this file is updated
in place as decisions firm up. Each locked decision carries:

1. The decision statement.
2. Reasoning -- why this and not the alternatives.
3. Methodology rules invoked (cross-references to `../README.md`
   "Methodology rules").
4. Prior-art evidence (cross-references to `prior-art/<project>.md`
   files in this folder, which are stage-3-specific primary-source
   research distinct from the stage-0 architectural-decisions
   `../stage-0/prior-art/` summaries).
5. Worked example(s) showing the C lowering each decision drives.

The structure makes individual decisions auditable: a review agent
should be able to read a decision, check the reasoning, verify the
prior-art citation, and confirm the worked example is consistent
with the rest of the IR.

**Currently locked:** D1 (variant set + lang-item recognition + ADT
kind separation + Option/Result handling + rejection list); D2
(layout position); D3 (interning strategy: structural BiMap with
`TyId` newtype); D4 (interning location: `RegseqModel.types:
TyInterner`, mirroring the stage-1 `StringInterner` pattern); D5
(`Dynamic` lowering: `Ty::Dynamic { auto_traits, vtable_ty }`,
fat-pointer codegen-emitted, work distributed across stages 9/11/12);
D6 (closure naming: rustc DefPath verbatim, captures-struct +
1-3 call-method Function Items synthesised by stage 12); D7/D8
(provenance enum on `Ref`; `Ty::Ptr` shape); D9 (volatile on the
intrinsic `InstanceId`, no `Ty` bit; raw `*_volatile` rejected);
D10 (atomics on the regseq-DSL `Atomic<T>` facade with intrinsic
operations; `core::sync::atomic` operational API rejected); D11
(layout content subset, refined by D13); D12 (niche optimization
preservation, now formal via `TagEncoding::Niche`); D13
(per-variant discriminant values: `VariantLayout.discriminant`
and `tag_value` extending D11); D15 (fn-pointer ABI subset:
`FnAbi { Rust, C }`); D16 (`c_void` rejected at bridge in v1).
The S0 framing principle ("no QualType layer," Section 1.12) is
the unifying meta-pattern these decisions commit to.

**Currently open:** None. D14 (renderer style) was originally
reserved at stage 3 but **deferred to stage 13** (type-system
pretty-printing) -- the renderer's input depends on `Ty` content
references that only become fully populated at stage 11
(`FnSig`) and stage 12 (`Item` / DefPath). See
`../stage-13/expectations.md` for the deferred-decision context;
`discussions.md` preserves the historical D14 entry as input.

**Supersedes / interacts with:**
- `stage-3/expectations.md` -- incoming forward refs from stage 1.
  Items here mark each as resolved when the spec lands.
- `stage-3/findings-rustc-draft.md` -- empirical evidence about
  rustc's type system. Cited extensively below.
- `stage-3/prior-art/<project>.md` -- per-project deep type-system
  research. Cited where each project's design informs a decision.
- `stage-1/spec-snapshot.md` -- stage-1 spec; pre-established the
  forward-declared `TyId` and the "Ty::Struct/Enum/Union with
  resolved layout" goal. Stage 3 fills in those forward decls.

---

## 1. The `Ty` enum

### 1.1 Variant set (v1, locked)

The v1 `Ty` enum has **19 variants**:

| Variant | Carries | C-side intent |
| --- | --- | --- |
| `Bool` | -- | `_Bool` (or `uint8_t`) |
| `Char` | -- | `uint32_t` (Unicode scalar value, not C `char`) |
| `Int(IntKind)` | `I8` / `I16` / `I32` / `I64` / `I128` / `Isize` | exact-width `intN_t` / `intptr_t` |
| `Uint(UintKind)` | `U8` / `U16` / `U32` / `U64` / `U128` / `Usize` | exact-width `uintN_t` / `uintptr_t` / `size_t` |
| `Float(FloatKind)` | `F16` / `F32` / `F64` / `F128` | C `float` / `double` / `_Float16` / `_Float128` |
| `Struct(...)` | resolved field list + layout | C `struct` |
| `Enum(...)` | resolved variants + tag encoding + layout | tagged C struct or niche-encoded layout |
| `Union(...)` | resolved field list + layout | C `union` |
| `Str` | -- (marker) | only ever behind `Ref`/`Ptr`; lowered to fat pointer |
| `Array(TyId, u64)` | element type + length | C array `T[N]` |
| `Slice(TyId)` | element type | only ever behind `Ref`/`Ptr`; lowered to fat pointer |
| `Ptr(TyId, Mutability)` | pointee + mutability | `T*` / `const T*` |
| `Ref(TyId, Provenance)` | pointee + borrow flavor | `T*` / `const T*` / `T* restrict` per provenance |
| `FnDef(ItemId, GenericArgs)` | callable item + post-mono args | ZST -- carries the function's identity |
| `FnPtr(FnSigId)` | signature handle (carries `FnAbi`) | C function pointer `T (*)(args)` |
| `Dynamic { auto_traits, vtable_ty }` | auto-trait set + TyId of synthesised vtable `Ty::Struct` | `dyn Trait` -- codegen emits fat pointer typedef when behind `Ref`/`Ptr` |
| `Closure(ItemId, GenericArgs)` | synthesised captures-struct Item + post-mono args | C struct of captures (layout via parallel `Ty::Struct` sharing the same Item) |
| `Never` | -- | uninhabited; codegen branch-elision marker |
| `Tuple { fields: Vec<TyId>, layout: Layout }` | element types + resolved layout (synthetic field names `_0`, `_1`, ...) | anonymous C struct |

The `Provenance` enum, the `FnAbi` enum on `FnSig`, the exact
field-list shape, and layout content are pinned in subsequent
decisions (D7/D8, D11, D15).

**Forward reference: `Ty` enum closure (Q3).** The 19-variant set
above is **closed** at stage 3. Stage 14 domain extensions
(intrinsics, register access, ISR classification, ordered regions,
...) do **not** add new `Ty` variants. Per Q3 (single-level IR,
`architecture.md`), domain extensions land as additional
`InstanceKind` variants, flags on `Item`, or tags on `Instance` /
existing IR nodes -- never as new `Ty` shapes. The set of type
shapes RegseqModel can represent is fixed here; later stages
refine the IR by populating optional fields, attaching tags, or
synthesising Items, not by extending `Ty`.

### 1.2 Decision D1.1 -- Variant scope: rigid post-monomorphization

**Decision.** `Ty` carries only variants that survive the bridge's
mandatory fold of every MIR-borne type through
`instantiate_mir_and_normalize_erasing_regions`. This is the *rigid*
subset of rustc's `TyKind` per `TyKind::is_known_rigid()`, minus
domain-specific rejections (D1.4) and adjustments for the v1 driver
DSL.

The bridge **rejects** any non-rigid `TyKind` it encounters. There
is no "fallback handling" for unexpected variants.

**Reasoning.**

Rustc has done the heavy lifting -- monomorphization substitutes
`Param`/`Bound` away; `normalize_erasing_regions` substitutes
`Alias::*` away; `erase_and_anonymize_regions` rewrites free
regions to `ReErased`. By the time the bridge consumes a fully
folded type, only rigid variants should remain. Per the
methodology rule "store the result, not the input," our `Ty` mirrors
the *result* of rustc's analysis, not the inputs to it.

The non-rigid variants represent intermediate states (typeck,
substitution, inference) that have no meaning at codegen. Carrying
them in our IR would be importing typeck baggage we have no
machinery to interpret.

**Methodology rules invoked.**
- "No defensive fallbacks" -- toolchain pinned; reject what
  shouldn't appear rather than handling speculatively.
- "C semantics with information preservation" -- store rustc's
  resolved result.
- "Re-derive from evidence; do not import baggage."

**Prior-art evidence.**
- Rustc's `TyKind` is 29 variants
  (`stage-3/findings-rustc-draft.md` Theme A.1, table rows
  enumerated lines 31-59).
- Rustc's `is_known_rigid()` partition
  (`stage-3/findings-rustc-draft.md` Theme A.1, citing
  `rustc_type_ir/src/ty_kind.rs:289-322`).
- `rustc_public::TyKind` collapses non-rigid into a 4-wrapper
  (`RigidTy`, `Alias`, `Param`, `Bound`) -- the public API that
  external consumers see uses the same partition.
- Convergent vocabulary size across surveyed *Rust- or LLVM-rich*
  closed-vocabulary projects: Charon 14
  (`prior-art/charon.md` Theme A), CLR 14
  (`prior-art/rustc-codegen-clr.md` Theme A), mrustc 15
  (`TypeData` enumerated at `prior-art/mrustc.md:47-83` --
  `Infer, Diverge, Primitive, Path, Generic, TraitObject,
  ErasedType, Array, Slice, Tuple, Borrow, Pointer, NamedFunction,
  Function, NodeType`), LLVM 22 (`prior-art/llvm-ir.md` Theme A).
  All sit between 14 and 22 closed variants -- this is the band
  our v1 set falls inside.
- Non-comparable lower-level points worth noting:
  *rustc-codegen-c* has only 6 type variants but the project is
  visibly incomplete with `todo!()` in many positions
  (`prior-art/rustc-codegen-c.md` Theme A); its vocabulary is not
  a stable design statement. *CompCert* has 9 front-end
  `Ctypes.type` constructors (`Tvoid`, `Tint`, `Tlong`, `Tfloat`,
  `Tpointer`, `Tarray`, `Tfunction`, `Tstruct`, `Tunion`; per
  `prior-art/compcert.md` Theme A), but C is the source language,
  so the count reflects C's intrinsic simplicity (no
  enums-with-payload, no closures, no `dyn`), not a Rust-IR design
  choice.
- MLIR is a separate data point: its type system is *open* /
  dialect-extensible, not a closed enum
  (`prior-art/mlir.md` Theme A). It contributes the structural
  insight that a fixed-size builtin vocabulary plus extension is a
  viable design, but its absolute variant count is not directly
  comparable.

**Variants that cannot survive the bridge's mandatory fold**
(`instantiate_mir_and_normalize_erasing_regions`):
- `Param`, `Bound`, `Placeholder`, `Infer`, `Error` -- typeck-only
  (`stage-3/findings-rustc-draft.md` Theme E.5).
- `Alias::Projection`, `Alias::Free`, `Alias::Inherent` -- normalized
  away by `normalize_erasing_regions`
  (`stage-3/findings-rustc-draft.md` Theme E.3).
- `Alias::Opaque` -- revealed in `TypingEnv::fully_monomorphized()`,
  which the bridge uses
  (`stage-3/findings-rustc-draft.md` Theme E.3).

After the fold, only rigid variants remain. *Some rigid variants are
also rejected*, but for DSL-scope reasons rather than survivability
(e.g., `Pat`, `UnsafeBinder`, `Foreign`, `Coroutine` family). The
formal, complete rejection list -- non-rigid plus rigid-but-DSL-rejected
-- is in D1.4.

### 1.3 Decision D1.2 -- ADT kind separation

**Decision.** `Ty::Struct`, `Ty::Enum`, and `Ty::Union` are three
sibling variants. There is no unified `Ty::Adt` variant.

**Reasoning.**

The three ADT kinds have *distinct C lowerings*. Struct → C struct
with field offsets. Enum → tagged C struct or niche-encoded layout
(potentially radically different shape; see D1.3). Union → C union.
Codegen always pattern-matches on which kind it has; flattening
removes a layer of indirection.

This also matches the stage-1 spec's forward declarations, which
already named `Ty::Struct { fields }`, `Ty::Enum { variants }`,
`Ty::Union { fields }` as the targets stage 3 must populate
(`stage-3/expectations.md` "Define the layout-pointing Ty
variants").

Below the kind level, three recognized lang-item ADTs (`PhantomData<T>`,
`UnsafeCell<T>`, `ManuallyDrop<T>`) carry their identity as flags on
the *referenced `Item` decl*, not via separate `Ty` variants. See D1.5
for the flag set and recognition mechanism, plus the adjacent lang
items that v1 deliberately does not flag (`Pin<P>`, `MaybeUninit<T>`,
`CVoid`).

**Methodology rules invoked.**
- "C semantics with information preservation" -- different C shapes
  warrant different IR variants.

**Prior-art evidence.**
- rustc_codegen_clr is the closest precedent for our use case
  (Rust → C, one IR, multiple printers): it uses a single `ClassRef`
  but distinguishes via `ClassDef` body shape with explicit overlap-
  vs-non-overlap layout (`prior-art/rustc-codegen-clr.md` Theme A).
  Flattening at the type level is a strict improvement -- saves a
  hop and the kind is statically discoverable.
- CompCert separates struct and union at the type level (`Tstruct`,
  `Tunion`); C doesn't have enums-with-payload but the principle
  matches (`prior-art/compcert.md` Theme A).
- Charon collapses to single `Adt(TypeDeclRef)` with kind on the
  decl (`prior-art/charon.md` Theme A) -- a different choice
  motivated by Charon's verifier focus where structural identity
  through the decl handle is load-bearing. Not our case.
- mrustc uses a single `Path` variant for all composites
  (`prior-art/mrustc.md` Theme A) -- closer to Charon's posture;
  motivated by their shared HIR-MIR-codegen IR.

### 1.4 Decision D1.3 -- `Option<T>` and `Result<T, E>` as ordinary `Ty::Enum`

**Decision.** `Option<T>` and `Result<T, E>` are not first-class
`Ty` variants. They are ordinary `Ty::Enum` values with two variants
each, exactly as a user-defined two-variant enum would be modeled.

This explicitly *rejects* the `regseq-model-old` design choice
which carved out `Option(Box<RegseqType>)` and
`Result(Box<RegseqType>, Box<RegseqType>)` as first-class type
variants (see `crates/regseq-model-old/src/ir/types.rs:18-29`).

**Reasoning.**

Niche optimization is a *layout-level* concern, not a type-level
one. Rustc's `LayoutData` already encodes whether a given enum gets
niche-encoded (via `TagEncoding::Niche`); when we treat
`Option<&u32>` as an ordinary `Ty::Enum`, the niche-optimized layout
falls out of consulting `tcx.layout_of(ty)`. We get the small
representation for free, *exactly* the same as if we had hand-
crafted a special `Ty::Option` variant -- without inventing new
machinery.

The argument also doesn't generalize: if we special-case Option and
Result, why not `core::cmp::Ordering` (also enum), `core::ops::ControlFlow`,
`core::task::Poll`? The principled position is: our `Ty::Enum` plus
proper layout handling subsumes all of these.

Per the methodology rule "C semantics with information preservation,"
rustc's niche-encoding decision *is* the resolved result we should
preserve. Bypassing it via dedicated `Option`/`Result` variants
duplicates the work and creates a divergent path codegen has to
maintain.

The `regseq-model-old` choice predates the architectural
decisions in `architecture.md` (Q1-Q7); it was made in a different
design context. Per the methodology rule "Re-derive from evidence;
do not import baggage," the old-IR choice is reference, not
authority.

**Methodology rules invoked.**
- "Re-derive from evidence; do not import baggage."
- "C semantics with information preservation."
- "Compiler-theory-first" -- niche optimization is the proper
  general mechanism; carving Option/Result special is a narrow
  workaround.

**Prior-art evidence.**
- rustc, mrustc, Charon, rustc_codegen_clr all model
  `Option`/`Result` as ordinary enums. None carve them out.
- LLVM has no enum concept at all -- Rust frontend lowers all enums
  including Option/Result identically (`prior-art/llvm-ir.md`
  Theme A).
- rustc's niche encoding for `Option<&T>`: `LayoutData` returns
  size = pointer size (8 on 64-bit), `TagEncoding::Niche {
  untagged_variant: Some, niche_variants: None..=None, niche_start:
  0 }` (`stage-3/findings-rustc-draft.md` Theme B.4).
- `regseq-model-old`'s first-class `Option`/`Result` is a
  documented anomaly with no surveyed peer
  (`stage-3/findings-rustc-draft.md` cross-reference and
  `crates/regseq-model-old/src/ir/types.rs:18-29`).

**Worked example.** See section 4.4 (`Option<&u32>` niche-encoded
lowering).

### 1.5 Decision D1.4 -- Rejection list

**Decision.** The bridge rejects two distinct categories of types
at construction:

**Category A -- Rigid but DSL-rejected.** These return true from
rustc's `is_known_rigid()` (`stage-3/findings-rustc-draft.md` Theme
A.1), placing them on the same side of the rigid/non-rigid partition
as the variants we accept (Bool/Char/Adt/Ref/.../Tuple). They are
rejected here for v1 driver-DSL scope reasons rather than because
they cannot survive `normalize_erasing_regions`. `UnsafeBinder`
deserves a footnote: it is rigid by the partition but is unstable
and unlikely to surface in optimized MIR per
`stage-3/findings-rustc-draft.md` "Watch-outs and open questions"
W.7; rejection is appropriate regardless. The bridge raises a
clear diagnostic naming the construct and (where applicable) the
canonical alternative.

1. `Foreign(_)` -- the `extern type T` Rust feature.
2. `Box<T>` (i.e., `Adt` flagged `IS_BOX`) -- `alloc` crate
   construct.
3. `Coroutine` / `CoroutineClosure` / `CoroutineWitness` -- async
   and generator state machines.
4. `Pat` -- pattern-newtype types.
5. `UnsafeBinder` -- unstable for-binders feature.

**Category B -- Non-rigid (cannot survive monomorphization).**
These should never appear in MIR after
`instantiate_mir_and_normalize_erasing_regions`. Encountering one
is a bug in the bridge's typing-environment setup or its MIR pass
whitelist; the diagnostic indicates that, not a driver-side issue.

6. `Param` -- type parameter; substituted away by monomorphization
   (`stage-3/findings-rustc-draft.md` Theme E.5).
7. `Bound` -- bound type variable in a `for<>` binder; only inside
   binder scope (`stage-3/findings-rustc-draft.md` Theme E.5).
8. `Placeholder` -- typeck-only
   (`stage-3/findings-rustc-draft.md` Theme E.5).
9. `Infer` -- typeck-only; codegen asserts
   `!instance.args.has_infer()`
   (`stage-3/findings-rustc-draft.md` Theme E.5).
10. `Error` -- error sentinel; absent in successful builds
    (`stage-3/findings-rustc-draft.md` Theme E.5).
11. `Alias::Projection` / `::Free` / `::Inherent` -- normalized
    away by `normalize_erasing_regions`
    (`stage-3/findings-rustc-draft.md` Theme E.3).
12. `Alias::Opaque` -- revealed in `TypingEnv::fully_monomorphized()`,
    which the bridge uses
    (`stage-3/findings-rustc-draft.md` Theme E.3).

**Reasoning per Category-A case.**

- **`Foreign`.** `Foreign(_)` represents the `extern type T` Rust
  feature, an unsized opaque FFI type
  (`stage-3/findings-rustc-draft.md` Theme A.6). None of the
  surveyed driver / Rust-to-target projects
  (`prior-art/rustc-codegen-c.md`, `prior-art/mrustc.md`,
  `prior-art/charon.md`, `prior-art/rustc-codegen-clr.md`) carry a
  dedicated `Foreign` variant in their type IRs. The standard Rust
  idiom for opaque C-side handles is the *opaque struct pattern*
  (`#[repr(C)] struct task_struct { _private: [u8; 0] }`), which
  is structurally an `Adt` and is supported by every project that
  handles ADTs. Bridge rejection points authors to that pattern.

- **`Box`.** `Box<T>` is defined in `alloc::boxed::Box`, not in
  `core`. The regseq DSL surface is "limited subset of `core` (plus
  core traits); no other crates required by the language" (per
  user-confirmed DSL scope rule). Drivers do not pull in `alloc`,
  so `Box<T>` cannot appear in their MIR. The `OwnedBox` lang
  item is registered only when `alloc` is in the dependency graph.
  If the bridge encounters `IS_BOX`-flagged ADTs, this is a DSL
  scope violation, not a driver pattern to support.

- **`Coroutine` family.** `async fn`, `async ||`, and coroutine
  syntax produce these variants. The regseq DSL is `#![no_std]`
  with no runtime facilities (`regseq-background.md:34-38`); driver
  specs do not invoke async. Stage 1 already established this
  position when rejecting `async fn` (`stage-1/spec-snapshot.md:81`).
  The pl011 fixture is sync, consistent with that scope. When a
  future framework target adds an async runtime, this rejection
  can be relaxed; for v1 it stays.

- **`Pat`.** Integer-range pattern types only today
  (`stage-3/findings-rustc-draft.md` Theme A.7). Not in driver use.

- **`UnsafeBinder`.** Requires `feature(unsafe_binders)`. Used for
  self-referential types with lifetimes erased. Should not appear
  in any reasonable driver code.

**Methodology rules invoked.**
- "No defensive fallbacks" -- reject, do not handle.
- DSL scope (user-confirmed): "Regseq DSL uses a limited subset of
  `core`. No other crates."

### 1.6 Decision D1.5 -- Lang-item recognition flags on `Item`

**Decision.** Three lang-item-flagged ADT types are recognized at
the bridge and represented as flags on the *referenced `Item`*
(stage-12 territory), not as separate `Ty` variants:

| Lang item | Kind | Flag name | Codegen impact |
| --- | --- | --- | --- |
| `PhantomData<T>` | Struct | `is_phantom_data` | omit field at C-emit time (0-byte ZST) |
| `UnsafeCell<T>` | Struct | `is_unsafe_cell` | drop `const` qualifier on enclosing references |
| `ManuallyDrop<T>` | Struct | `is_manually_drop` | suppress drop-glue Calls for the wrapped value |

Recognition: the bridge uses rustc's `AdtFlags` predicates
(`adt_def.is_phantom_data()` / `is_unsafe_cell()` /
`is_manually_drop()`) when constructing the `Item` for the ADT.
All three v1 flags are cached in `AdtFlags`
(`stage-3/findings-rustc-draft.md` Theme D.1, citing
`rustc_middle/src/ty/adt.rs:30-63`), so no separate
`tcx.is_lang_item(...)` query is needed for them. Flags are stored
on the `Item`, not duplicated on every `Ty` referencing it.

For lang items that *are* in `AdtFlags` but currently rejected or
unflagged in v1, the bridge uses the `AdtFlags` predicate path
uniformly: `OwnedBox` -> `IS_BOX` (used for D1.4 rejection
diagnostics), `Pin` -> `IS_PIN` (used when Pin recognition is
reintroduced post-v1).

`tcx.is_lang_item(did, LangItem::X)` is reserved for lang items
*not* cached in `AdtFlags` -- specifically `MaybeUninit`, `CVoid`,
`VaList` (`stage-3/findings-rustc-draft.md` Theme D.2). These
require the slower direct query when (and only when) recognition
is needed.

**Reasoning.**

Each flag drives a *specific* and *necessary* C-codegen decision
that no general rule subsumes. Without the flag, the C output is
wrong or unsafe:

- **`PhantomData` omitted** because Rust's `PhantomData<T>` is a 0-byte
  ZST with no runtime presence. C structs require non-zero size
  for any declared field; emitting `phantom_data_t phantom;` adds
  at least 1 byte plus alignment padding, breaking the
  rustc-claimed layout.

- **`UnsafeCell` drops `const`** because `&UnsafeCell<T>` in Rust
  permits mutation through a shared reference (per the `Sync` /
  interior-mutability contract). If we emit the C reference as
  `const T*`, writing through it is C undefined behavior. Note that
  `restrict` is not affected by `UnsafeCell` -- `restrict` is added
  only for `&mut T` (`UniqueBorrow`), and shared `&T` (regardless
  of `UnsafeCell`) does not get `restrict` because shared aliases
  are allowed.

- **`ManuallyDrop` suppresses drop-glue** because its semantic
  contract is "no automatic Drop." If codegen emits drop-glue Calls
  at scope exit for `ManuallyDrop<T>` slots, double-frees or
  drops-on-uninit-memory result. The methodology rule "drop-glue
  calls are regular Calls" applies, but `ManuallyDrop` specifically
  suppresses those Calls.

These flags are *not* `Ty` variants because the *type-level shape*
is unchanged: `PhantomData<T>` is still a Struct, just a flagged
one; `ManuallyDrop<T>` is still a Struct, just a flagged one.
Conflating "shape" and "lang-item identity" would inflate the
variant set without semantic gain.

**Lang items not flagged in v1 (and why).**

- **`Pin<P>`.** No codegen consumer for synchronous driver code
  (move-blocking is enforced pre-MIR by typeck). Per the
  methodology rules "no defensive fallbacks" and "ask who consumes
  it; what shape best serves them," recognizing it without a v1
  consumer is speculative. `Pin<P>` is treated as an ordinary
  `Ty::Struct` for v1. When async support is added (with
  `Coroutine` rejection lifted), `Pin` recognition is reintroduced
  at that time.

- **`MaybeUninit<T>`.** Its drop-suppression is achieved
  structurally (see Section 4.6 worked example):
  `MaybeUninit<T>` is `union { uninit: (), value: ManuallyDrop<T> }`,
  and the existing machinery -- `Ty::Union` does not auto-drop
  fields, plus the inner `is_manually_drop` flag -- is sufficient.
  An additional `is_maybe_uninit` flag would be redundant.

- **`OwnedBox`** (Box) -- not in scope (D1.4).
- **`VaList`** -- variadic args; not in driver use.
- **`CVoid`** (`core::ffi::c_void`) -- not flagged/lowered in v1;
  recognized via `tcx.is_lang_item(did, LangItem::CVoid)` only
  to **reject** at type translation per D16 (Section 1.15).
  Reopens to add an `is_c_void` flag if a future driver fixture
  or stage-14 intrinsic surfaces a stable need for FFI-canonical
  pointer-to-`c_void` lowering.

**Per-flag `AdtFlags` bit (for cross-reference).**

- `is_phantom_data` <- `AdtFlags::IS_PHANTOM_DATA`.
- `is_unsafe_cell` <- `AdtFlags::IS_UNSAFE_CELL`.
- `is_manually_drop` <- `AdtFlags::IS_MANUALLY_DROP`.

(All cited from `stage-3/findings-rustc-draft.md` Theme D.1 and
`rustc_middle/src/ty/adt.rs:30-63`.)

**Methodology rules invoked.**
- "C semantics with information preservation" -- recognize the
  lang-item identity at the bridge (where the rustc query is local)
  and store it, rather than passing the rustc-side annotation
  through and re-querying downstream.
- "No defensive fallbacks" -- only flag types with concrete v1
  consumers; defer the rest until a consumer materializes.
- "Ask who consumes it; what shape best serves them" -- each flag's
  consumer is named (omit field / drop const / suppress drop-glue).

**Prior-art evidence.**
- Rustc's own model: `AdtFlags` bits
  (`stage-3/findings-rustc-draft.md` Theme D.1, citing
  `rustc_middle/src/ty/adt.rs:30-63`). Rustc caches the recognition
  for fast access; we mirror that approach for the three v1 flags
  (PhantomData, UnsafeCell, ManuallyDrop), all of which are in
  `AdtFlags`.
- mrustc recognizes lang items via `StaticTraitResolve::type_is_*`
  predicates at the same Item-decl level
  (`prior-art/mrustc.md` Theme D).
- Charon treats UnsafeCell, PhantomData, MaybeUninit, ManuallyDrop,
  and Pin as ordinary ADTs with no special type-level marker
  (`prior-art/charon.md` Theme D). This supports our "*not* a
  separate `Ty` variant" choice; the further "flag on the `Item`
  decl" storage decision is ours, not Charon's (Charon does not
  flag these on its decl side either).
- rustc_codegen_clr: UnsafeCell is invisible at the type level;
  volatile is per-op (`prior-art/rustc-codegen-clr.md` Theme D).

**Worked examples.** See sections 4.3 (PhantomData omission),
4.5 (UnsafeCell aliasing), 4.6 (MaybeUninit drop suppression --
explained as a structural composition without an explicit flag),
4.7 (ManuallyDrop drop suppression).

### 1.7 Decision D1.6 -- Auxiliary variant decisions

Three small decisions consolidated.

**D1.6.a -- `Str` as marker variant.** `Ty::Str` carries no
payload. Its sole purpose is to let the bridge distinguish
`Ref(Str, _)` (fat pointer with `len`, no NUL terminator) from
`Ptr(Int(I8), _)` (thin pointer to byte, NUL-terminated by C
convention).

*Reasoning.* Rust's `str` is fundamentally different from C
strings: UTF-8 (not implementation-defined), explicitly-lengthed
(not NUL-terminated), DST-only (always behind a pointer). The
codegen distinction is required for correct fat-pointer emission.
See section 4.2 for the worked lowering.

**D1.6.b -- `FnDef` and `FnPtr` distinct.** `Ty::FnDef(ItemId,
GenericArgs)` represents the *anonymous ZST type* of a specific
function name (e.g., `let bar = foo;` gives `bar` the type
`fn() -> i32 {foo}`, a ZST). `Ty::FnPtr(FnSigId)` represents a
function-pointer type (`fn() -> i32`, address-bearing).

*Reasoning.* Conflating them loses the per-call-site
monomorphization key. `FnDef` carries the function's identity --
critical for resolving direct calls. `FnPtr` is opaque -- carries
only signature, no identity.

*Prior-art convergence.* rustc, mrustc, Charon all keep distinct.
rustc_codegen_clr collapses fn-items to `Type::Void` -- the
outlier (`prior-art/rustc-codegen-clr.md` Theme A); the cost is
having to recover identity at codegen time, which we don't want.

**D1.6.c -- `Never` separate variant.** `Ty::Never` represents
Rust's `!` type. Distinct from any uninhabited Adt, distinct from
`c_void`.

*Reasoning.* `!` is uninhabited -- no values exist. Codegen uses
this to elide unreachable arms (e.g., the `Err` branch of
`Result<T, !>`).

C lowering for a function `fn(...) -> !` decomposes into two
*complementary* signals, not equivalents:

- *Return type spelling.* C has no first-class non-returning return
  type, so the syntactic slot becomes `void`. `void` here only
  means "no value to return," not "function never returns."
- *Non-return semantics.* Conveyed by `__attribute__((noreturn))`
  (gcc/clang) on the function declaration. This is the actual
  carrier of the never-returns information.

The full lowering of `fn diverge() -> !` is therefore
`__attribute__((noreturn)) void diverge(void);`. Non-return
positions for `!` (e.g., as an expression) are unreachable by
construction; codegen elides them.

`c_void` is a different concept: an opaque C-level type used in
extern signatures. It is the `CVoid` lang-item ADT -- in the
pinned toolchain a 2-variant `repr(u8)` enum with two hidden
variants (`__variant1`, `__variant2`) that make it inhabited;
lang-item target is `Target::Enum` per
`library/core/src/ffi/mod.rs:49-76` and
`rustc_hir/src/lang_items.rs:279` (cf.
`stage-3/findings-rustc-draft.md` Theme D.2). v1 does *not* flag
it (see D1.5 "Lang items not flagged in v1"); D16 hardens this
deferral into active rejection at the bridge pending a future
`is_c_void` lowering decision.

### 1.8 Decision D7/D8 -- Provenance enum on `Ref`; `Ty::Ptr` shape

**Decision.** Three coupled changes:

1. The `Ty::Ref` variant is `Ref(TyId, Provenance)`. The `Mutability`
   field is dropped; it was 1:1 redundant with the new `Provenance`
   field for borrows.
2. The `Ty::RawPtr` variant is renamed to `Ty::Ptr` and is
   `Ptr(TyId, Mutability)`. No structural change; `Mutability` is
   `rustc_ast_ir::Mutability` (the existing `Not` / `Mut` two-variant
   enum).
3. The `Provenance` enum has exactly two variants:

   ```rust
   pub enum Provenance {
       SharedBorrow,    // source-level &T
       UniqueBorrow,    // source-level &mut T
   }
   ```

   The third advisory variant (`Raw`) that was originally listed as
   the third element of the alphabet does **not** appear in the IR
   enum: raw pointers are encoded by the variant choice (`Ty::Ptr`
   vs `Ty::Ref`), not by a `Provenance` value. Including a vacuous
   `Raw` would be a single-valued field on `Ty::Ptr` -- pure waste.

The bridge translation rule is forced once the IR shape is locked:

```
TyKind::Ref(_region, ty, Mutability::Not)  ->  Ty::Ref(translate(ty), Provenance::SharedBorrow)
TyKind::Ref(_region, ty, Mutability::Mut)  ->  Ty::Ref(translate(ty), Provenance::UniqueBorrow)
TyKind::RawPtr(ty, mutbl)                  ->  Ty::Ptr(translate(ty), mutbl)
```

The `_region` slot in `TyKind::Ref` is **inspected at the parent
level** before being discarded. Post-erasure, the slot is
`ReErased` for free regions (the typical case for MIR-local types
after `tcx.instantiate_mir_and_normalize_erasing_regions(...)`)
and `ReBound` for late-bound regions inside `Binder<FnSig>` /
`Binder<Predicate>` and `Dynamic` existential-predicate scopes
(see `findings-rustc-draft.md:675` and `:695-697`). The bridge's
behaviour is two-pass: HRTB-bearing parents (`Ty::FnPtr` over a
`Binder<FnSig>` containing `ReBound`, `Ty::Dynamic` over
`Binder<Predicate>` containing `ReBound`) are **rejected** at
the parent level per the refinement below ("higher-ranked binders
rejected in v1") -- the bridge raises
`BridgeError::UnsupportedHRTB` *before* any nested `TyKind::Ref`
inside such a binder translates. For non-HRTB parents, every
`TyKind::Ref` the bridge then translates has a `ReErased` free
region, and the slot is discarded with no `RegionKind` recording.
No region survives in `Ty::Ref` for any type that enters the IR.

**Reasoning.**

Rust's mutability information is layered across three independent
axes:
1. **Binding mutability** (`let x` vs `let mut x`) -- a property of
   the binding site, not the type. Lives in MIR `LocalDecl`, not in
   `Ty`.
2. **Reference mutability** (`&T` vs `&mut T`, or `*const T` vs
   `*mut T`) -- carried in the *type*. This is what `TyKind::Ref`'s
   third field and `TyKind::RawPtr`'s second field encode in rustc.
3. **Data mutability of the original owner** -- a property of the
   owner's binding site. Not in `Ty`; partially observable through
   the `is_unsafe_cell` flag on `Item` (interior mutability).

Our IR's `Provenance` and `Mutability` fields target axis 2
exclusively. Two consequences follow:

- **For `Ref`, `Mutability` and `Provenance` carry the same axis-2
  signal** (`Not` <=> `SharedBorrow`, `Mut` <=> `UniqueBorrow`).
  Storing both is redundant. We keep `Provenance` because its
  spelling carries the C-codegen-relevant connotation: "is this
  borrow unique?" -- which is exactly the predicate that controls
  the `restrict` qualifier emission. The bridge's `Mutability`
  enum value is the *input*; the IR stores the *result*. This
  follows the methodology rule "store the result, not the input."

- **For `Ptr`, `Mutability` is the only axis-2 signal that varies**
  (raw pointers always have implicitly-`Raw` provenance with no
  uniqueness contract). A `Provenance` field on `Ptr` would be
  vacuous; including it would be the kind of redundant flag that
  the methodology forbids ("compositional behavior beats special-
  case flags").

The asymmetry between `Ref(_, Provenance)` and `Ptr(_, Mutability)`
is therefore deliberate: it tracks that the two pointer kinds carry
*semantically different* axis-2 information. `Ref`'s axis-2 is a
uniqueness/aliasing contract; `Ptr`'s axis-2 is a source-level
`*const`/`*mut` marker with no aliasing implication.

The rename `RawPtr` -> `Ptr` pairs the two kinds symmetrically by
name (`Ref` for borrows, `Ptr` for raw pointers); the asymmetry is
in the carried payload, not in the variant name.

Advisory bits from rustc's `PointerKind` (`SharedRef.frozen`,
`MutableRef.unpin`, `Box.unpin/global`) are dropped. Per
`rustc_abi/src/lib.rs:2134-2137`: "if the information is wrong, that
can cause UB, but if the information is absent, that must always be
okay." None of the bits has a v1 codegen consumer:

- `frozen` (no `UnsafeCell` in pointee) would let codegen add
  `restrict` on shared `&T`. We don't add `restrict` on shared
  references (the alias contract permits multiple shared borrows of
  the same data). So `frozen` adds nothing.
- `unpin` (no pinned data) is meaningful only when async/`Pin<P>`
  is in scope. Stage-3 D1.4 rejects async (`Coroutine`); `Pin<P>`
  is on the stage-3-deferred list (`status.md`). Moot.
- `global` (Box on global allocator) is moot because Box is in the
  v1 reject list (no heap allocation in driver code).

If a future codegen consumer materializes for any of these bits,
it can be re-introduced as a flag *on the consumer site* (e.g., a
flag on `Item` for the box allocator, mirroring how
`is_unsafe_cell` is stored on `Item`, not on `Ty`).

**Methodology rules invoked.**
- "C semantics with information preservation" -- store the
  resolved axis-2 result; let bridge translate `Mutability` into
  `Provenance` once.
- "No defensive fallbacks" -- toolchain pinned; bridge debug-
  asserts the post-erasure invariant on the region slot rather
  than handling unexpected values.
- "Compositional behavior beats special-case flags" -- raw
  pointer's `Raw` provenance is encoded by the variant choice
  (`Ty::Ptr`); no flag stored.
- "Ask who consumes it" -- advisory bits dropped; no v1 consumer.
- "Don't redo work the rustc compiler has already done" -- bridge
  consumes `Mutability` once; IR stores derived `Provenance`.

**Prior-art evidence.**

- **Charon** (`prior-art/charon.md` Theme A, lines 104-107, 248-
  251). Charon defines `RefKind { Mut, Shared }` and *reuses* the
  same enum for both `Ref(Region, Ty, RefKind)` and
  `RawPtr(Ty, RefKind)`. We diverge: our `Provenance` enum lives
  on `Ref` only; `Ty::Ptr` carries `Mutability` directly.
  Rationale for divergence: Charon's verifier-focused IR collapses
  the borrow-vs-raw distinction at the `RefKind` level (both have
  the same flavor field); for byte-exact C codegen we want the
  variant-level distinction (the `restrict` rule fires for
  borrows only, not raw pointers, and the rule is local on the
  variant tag rather than requiring a join condition).
- **rustc `PointerKind` and `PointeeInfo`** (`findings-rustc-
  draft.md` Theme B.10, lines 405-417). rustc's three-way
  partition (`SharedRef { frozen }`, `MutableRef { unpin }`, `Box
  { unpin, global }`) is the source of the advisory-bits debate.
  Our IR keeps the borrow-flavor distinction (Shared vs Mutable
  vs raw-via-`Ptr`-variant) and drops every advisory bit.
- **rustc `TyKind::Ref` / `TyKind::RawPtr`** (`findings-rustc-
  draft.md` Theme A, lines 42-43, 193-196). rustc carries
  `Mutability` on both; our IR consumes `Mutability` on the
  reference path and stores `Provenance` instead.
- **mrustc** (`prior-art/mrustc.md`, line 461). mrustc keeps no
  per-`TypeRef` flag for interior mutability; the interior-
  mutability check is a separate path-walk
  (`src/hir_typeck/static.cpp:2831-...`). Confirms that the
  axis-3 (data mutability via `UnsafeCell`) decision to keep the
  flag on `Item` rather than on `Ty` is the conventional split.
- **LLVM IR** (`prior-art/llvm-ir.md`). LLVM has typeless `ptr`
  post-opaque-pointer transition: `PointerType` carries only an
  address space (`prior-art/llvm-ir.md:71`), and the textual form
  is `ptr` (`prior-art/llvm-ir.md:389-390`). No mutability/const
  at type level (`prior-art/llvm-ir.md:505`). Confirms that
  aliasing/mutability annotations are attribute-level concerns in
  lower IRs; we keep a higher-level type-system distinction because
  the C codegen target needs it for `const` / `restrict` emission.
- **CompCert** (`prior-art/compcert.md` Theme A). Has `Tpointer ty
  attr` with `attr_volatile` etc., but no borrow-vs-raw
  distinction. CompCert's source language is C; the borrow
  distinction does not exist at C source level, so this is not a
  comparable design choice.

**Worked example.** See Section 4.1 (replacement) and the new
edge-cases subsection 4.1.1.

**Refinement: higher-ranked binders (HRTB) are rejected in v1.**
Region erasure under `erase_and_anonymize_regions` keeps
`ReBound` regions intact (`findings-rustc-draft.md` W.13); free
regions become `ReErased`. HRTB-bearing types -- `for<'a> fn(&'a
T) -> &'a T` (a higher-ranked `Ty::FnPtr`) and `dyn for<'a>
Trait<'a>` (a higher-ranked `Ty::Dynamic`) -- therefore arrive
at the bridge with their binders still present, distinct from
the same-shape non-HRTB types under uniform region erasure.

The same type-identity argument that locks D15's `FnAbi` field
(rustc-distinct types must remain distinct in the IR) applies
to binders. **v1 policy: the bridge rejects HRTB-bearing
`Ty::FnPtr` and `Ty::Dynamic` types** at type translation with
`BridgeError::UnsupportedHRTB { span, ty }`. Driver code does
not need higher-ranked function pointers or higher-ranked
trait-object binders in v1 (no fixture surfaces them; no
stage-14 intrinsic signature uses them); the rejection follows
the D9/D10/D16 rejection-pattern symmetry ("type shapes outside
the recognized vocabulary are rejected"). The IR's region
erasure for non-HRTB types is uniform per the locked rule
above; HRTB types do not enter the IR at all. When a future
driver fixture or stage-14 intrinsic surfaces a stable need for
HRTBs, this rule reopens to either (a) extend `FnSig`
canonicalization to encode binder rank/shape, or (b) add a
higher-ranked variant.

### 1.9 Decision D15 -- fn-pointer ABI subset (`FnAbi` enum)

**Decision.** A new enum `FnAbi` is added to the stage-3 type-system
vocabulary:

```rust
pub enum FnAbi {
    Rust,    // default Rust ABI; rustc's ExternAbi::Rust
    C,       // extern "C"; rustc's ExternAbi::C { unwind: false }
}
```

**Storage.** `FnAbi` is a structural field of `FnSig` (whose full
shape is stage-11 territory; `stage-1/spec-snapshot.md` line 76 reserves
the slot). Stage 3 commits the enum vocabulary; stage 11 embeds
`FnSig::abi: FnAbi`.

**Bridge translation.** The bridge maps rustc's `ExternAbi` (defined
at `rustc_abi/src/extern_abi.rs:20-119`, 28 variants total) as
follows:

```
ExternAbi::Rust                       ->  FnAbi::Rust  (accept)
ExternAbi::C { unwind: false }        ->  FnAbi::C     (accept)
ExternAbi::C { unwind: true }         ->  reject (BridgeError::UnsupportedAbi)
[all non-Rust / non-C ExternAbi variants] ->  reject (BridgeError::UnsupportedAbi)
```

The rejection is **hard**: the bridge raises a diagnostic carrying
the source span and the unsupported ABI name, and refuses to
translate the function (or the `FnSig` referenced by a `FnPtr`/`FnDef`
type). No defensive fallback to `FnAbi::C` for unsupported ABIs.

**Reasoning.**

The `FnAbi` field exists for two distinct reasons, with very
different stakes:

1. **Type-identity preservation (soundness).** Rustc treats
   `fn() -> u32` and `extern "C" fn() -> u32` as **distinct types**
   with no implicit coercion. rustc internally stores this in
   `FnSig.abi`, and `Ty<'tcx>` identity reflects it. If our `FnSig`
   omits the ABI, the bridge maps two rustc-distinct types to one
   IR type, the interner conflates them, and downstream MIR casts
   between the two become vacuous. *This forces an `FnAbi` field
   onto `FnSig`.* The variant set must be at least as wide as the
   set of ABIs that appear in code we accept.

2. **Codegen and downstream decisions.** Active and near-future
   consumers:

   - *Pretty-printer (stage 13's `render_ty`).*
     Round-trippable rendering of `extern "C" fn(...)`;
     consumes the full enum.
   - *Future FFI-validity linter.* "Are this `extern "C"`
     function's argument and return types C-compatible?" --
     deferred consumer; consumes the predicate "is this
     `FnAbi::C`?".
   - *Calling-convention attribute emission.* On Linux x86-64
     (the v1 demo target) all our accepted ABIs collapse to the C
     ABI at the binary level; the C compiler picks the platform
     default. **No-op for v1.** A future cross-platform port would
     wire this up (e.g., `__attribute__((stdcall))` on
     Windows-x86 for `System`).

   **Not a `FnAbi` consumer: symbol mangling.** Symbol mangling
   is governed by separate machinery: `Item.linkage` (the
   `Linkage` enum, locked stage-1 / formalized stage 12, see
   `stage-1/spec-snapshot.md:77`) determines whether
   `#[no_mangle]` / `#[export_name]` apply; the default mangled
   name is `Instance.symbol_name = tcx.symbol_name(...)` (see
   `stage-1/spec-snapshot.md:670-673`). `extern "C" fn foo()`
   without `#[no_mangle]` still receives a mangled symbol --
   the ABI does not control mangling. Routing the
   mangled-vs-unmangled decision through `FnAbi` would be wrong;
   it belongs to the linkage layer, not the type layer.

The minimal variant set for v1 is therefore driven by the
type-identity argument: which ABIs appear in code we accept? For
Linux v1 driver code:

- `Rust`: pervasive (default for any non-extern function).
- `C`: pervasive (FFI to kernel APIs).
- `System`, `Unadjusted`, ...: rare or never in driver code.
  `System` resolves to `C` on Linux. `Unadjusted` is
  compiler-internal.
- `RustCall`: the closure-trait method ABI. Closures are
  accepted (D6, Section 1.11) but `RustCall` does **not** appear
  in the IR -- D6.1 bullet 4 rewrites synthesised closure
  call-method Function Items to `FnAbi::Rust` after the
  receiver/argument tuple normalization D6.1 bullet 2 already
  performs. The post-rewrite `FnAbi::Rust` is correct because
  `RustCall` is *defined* as `Rust` plus tuple-packing of
  receiver and arguments; once the bridge undoes the packing,
  the conventions coincide. This rewrite is the only D15
  exception to hard rejection of non-`Rust`/non-`C` ABIs.

We pick the tight set `{ Rust, C }` and reject everything else.
This obeys methodology rules: "expand vocabulary lazily," "reject
what shouldn't appear; never speculate-handle." Adding `System` /
`Unadjusted` later if a real driver crashes the bridge is a
one-line enum extension plus a matching bridge entry.

**Why `unwind: true` is rejected.** `extern "C-unwind"` (RFC 2945,
stabilized Rust 1.71) permits panic-style unwinding to propagate
across an FFI boundary. Driver code essentially never uses this:
Linux kernels run `panic = "abort"` semantics (panics halt the
machine, no unwinding); Windows kernels bugcheck similarly. The
construct represents a runtime capability drivers don't have.
Mapping `C { unwind: true }` to `FnAbi::C` would silently strip
type-system information -- a defensive fallback. Rejecting
surfaces the problem at the bridge with the source span. If a
future use case for cross-FFI unwinding materializes (e.g., a
user-space Rust library FFI'd to C++ with exceptions), lift the
enum to `{ Rust, C, CUnwind }` or a parameterized
`C { unwind: bool }` variant -- not in v1 scope.

**Methodology rules invoked.**
- "C semantics with information preservation" -- store the
  source-level ABI marker so that pretty-printing round-trips and
  the IR's type identity matches rustc's.
- "No defensive fallbacks" -- bridge hard-rejects unsupported
  ABIs rather than silently mapping to `C`.
- "Reject what shouldn't appear; never speculate-handle" -- all
  non-`Rust` / non-`C { unwind: false }` `ExternAbi` values are
  bridge-rejected, not speculatively handled.
- "Ask who consumes it" -- the variant set is sized to active and
  near-future consumers (pretty-printing, future FFI-validity
  linter, future calling-convention attribute emission), not to
  rustc's full vocabulary. Symbol mangling is *not* a consumer;
  it is governed by `Item.linkage` and `Instance.symbol_name`
  (see the "Not a `FnAbi` consumer: symbol mangling" sub-block
  in Reasoning above).
- "Compiler theory first" -- type-identity preservation is
  non-negotiable; the variant alphabet is bounded by accepted
  input range.

**Prior-art evidence.**

- **rustc `ExternAbi`** (`findings-rustc-draft.md` Theme A.9, lines
  198-230). 28 source-level variants; some carry `unwind: bool`.
  Our `FnAbi` mirrors a subset; the bridge collapses
  `C { unwind: false }` to `FnAbi::C` and rejects all else. This
  is the source-of-truth enumeration we narrow from.
- **Charon** (`prior-art/charon.md` Theme F, lines 1143-1148).
  Charon's `FunSig` does **not** carry an ABI -- "Calling
  conventions, including extern \"C\", must be recovered (or are
  lost)." We diverge: storing the ABI is required for type-
  identity soundness in our IR (rustc-distinct types must remain
  distinct in our model). Charon's verifier-IR posture doesn't
  need this; our byte-exact-C posture does.
- **mrustc** (`prior-art/mrustc.md` line 207, 430). mrustc keeps
  the ABI as a string (`m_abi: RcString`) and includes it in
  type-equality (line 430: `if (te.m_abi != xe.m_abi) return
  false`). Confirms the soundness-driven design (ABI is part of
  type identity) is the conventional choice for direct C-targeting
  Rust compilers.
- **LLVM IR** (`prior-art/llvm-ir.md` line 332). LLVM has multiple
  calling conventions sharing one `FunctionType` -- relaxed type
  identity. LLVM is a typed assembly; calling-convention is a
  per-call-site or per-function attribute, not part of the
  function-pointer type. We diverge because our type-identity
  posture matches rustc (which is stricter than LLVM).
- **rustc_codegen_clr** (`prior-art/rustc-codegen-clr.md:116-118`).
  CLR's `FnPtr(Interned<FnSig>)` carries an interned signature
  whose `FnSig` is `{ inputs: Box<[Type]>, output: Type }` -- no
  calling-convention or varargs marker stored. This is a non-
  comparable counterexample: CLR targets the .NET CIL universe
  rather than rustc-style ABI semantics, and the C printer collapses
  CIL platform types regardless of source ABI; ABI distinctions
  that drive our codegen don't apply. Useful only as a contrast,
  not as transferable evidence.

**Worked example.**

```rust
// Source
fn rust_fn(x: u32) -> u32 { x + 1 }                    // FnAbi::Rust
extern "C" fn c_fn(x: u32) -> u32 { x + 1 }            // FnAbi::C
extern "C-unwind" fn unwind_fn(x: u32) -> u32 { x }    // bridge-rejected
extern "system" fn sys_fn() {}                         // bridge-rejected (v1)

// Function-pointer type
let p1: fn(u32) -> u32           = rust_fn;            // Ty::FnPtr(<sig with Rust>)
let p2: extern "C" fn(u32) -> u32 = c_fn;              // Ty::FnPtr(<sig with C>)
```

Bridge behavior on `unwind_fn`:

```
BridgeError::UnsupportedAbi {
    abi:  "C-unwind",
    span: <span of `extern "C-unwind"`>,
    note: "RegseqModel v1 supports `extern \"Rust\"` and `extern \"C\"` only. \
           If your driver requires C-unwind ABI, file an issue.",
}
```

The function decl is excluded from the model; downstream codegen
never sees it. A `Ty::FnPtr` referencing a rejected signature is
likewise rejected (the bridge cannot construct a valid `FnSigId`).

C-side emission (Linux x86-64 v1 demo). Mangling shown below is
governed by `Item.linkage`, not by `FnAbi` -- the same `FnAbi::C`
function emits a mangled or unmangled symbol depending on the
linkage attribute. `FnAbi` itself is invisible at the C-source
level on Linux x86-64 because all accepted ABIs collapse to the
platform default; no calling-convention attribute is emitted.

```c
// rust_fn:    FnAbi::Rust + default linkage  -- mangled symbol
uint32_t _ZN8my_crate7rust_fn17h<hash>E(uint32_t x);

// c_fn:       FnAbi::C    + default linkage  -- still mangled (linkage drives mangling)
uint32_t _ZN8my_crate4c_fn17h<hash>E(uint32_t x);

// c_fn_export: FnAbi::C   + #[no_mangle] linkage  -- unmangled (NoMangle linkage drives the spelling)
uint32_t c_fn_export(uint32_t x);
```

The `FnAbi` field's active and near-future observable effects are:
(a) type-identity preservation in the IR (interner correctness;
distinct `FnSig`s remain distinct -- this is the v1 load-bearing
soundness reason), (b) the pretty-printer's ability to round-trip
`extern "Rust" fn(...)` vs `extern "C" fn(...)` (stage 13;
see `../stage-13/expectations.md`), (c) the
future FFI-validity linter's "is this `FnAbi::C`?" predicate
(deferred consumer), and (d) future calling-convention attribute
emission when a non-collapsing target is added. A future
cross-platform port keeps the field for emission of
`__attribute__((<conv>))` on targets where the source ABI does
not collapse (e.g., Windows-x86 `System` -> stdcall).

**Stage-11 forward note.** Stage 11 embeds `FnSig::abi: FnAbi` as
a **structural field of `FnSig`**. This is the **only** location
in RegseqModel where the function ABI is stored: no side-channel
table, no bridge-side `HashMap<FnSigId, FnAbi>`, no rustc query
re-issued post-translation. Q2 (eager translation, fully-owned
IR; `architecture.md`) requires that every piece of rustc-side
state needed by downstream consumers is baked into RegseqModel so
`'tcx` does not survive past the bridge call. `FnAbi`-on-`FnSig`
is the type-identity-load-bearing instance of that rule:
omitting it (or storing it elsewhere and reconstructing on
demand) would conflate rustc-distinct types in the interner per
the soundness argument above. The field name is fixed by D15;
stage 11 wires the body of `FnSig` around it.

### 1.10 Decision D5 -- `Dynamic` lowering

D5 has four sub-decisions: D5.1 (predicate content of
`Ty::Dynamic`), D5.2 (where the lowering work happens), D5.3-A
(vtable type contract), D5.3-B (fat-pointer C-shape policy). All
are locked at the stage-3 level; downstream synthesis details are
forwarded to `stage-9/expectations.md`,
`stage-11/expectations.md`, and `stage-12/expectations.md`.

#### 1.10.1 D5.1 -- Predicate content of `Ty::Dynamic`

**Decision.** `Ty::Dynamic` is a struct variant carrying:

```rust
Ty::Dynamic {
    auto_traits: Vec<AutoTrait>,    // sorted; canonical v1 order: Send < Sync < Unpin
    vtable_ty:   TyId,              // -> Ty::Struct (vtable type; details in stage-12 expectations)
}

pub enum AutoTrait { Send, Sync, Unpin }   // closed v1 set
```

The principal trait identity, projection bindings, and method
signatures are *not* fields of `Ty::Dynamic`. They flow through
`vtable_ty`, which resolves to a synthesised `Ty::Struct` whose
Item, DefPath, hash, layout, and field set are pinned in
`stage-12/expectations.md`. Stage-3 commits only to (a) the slot
exists, (b) it is of type `TyId`, (c) it is filled by the bridge
at type-translation time, (d) it resolves to a `Ty::Struct` value.

HRTB binders on `dyn` types (e.g., `dyn for<'a> Trait<&'a T>`)
are **rejected** at the bridge per the D7/D8 refinement (Section
1.8 "Refinement: higher-ranked binders (HRTB) are rejected in
v1"). Region erasure under `erase_and_anonymize_regions` keeps
`ReBound` regions intact (`findings-rustc-draft.md` W.13), so an
HRTB binder *would* leave an IR trace and could collapse rustc-
distinct types under uniform erasure -- the same type-identity
argument that locks D15's `FnAbi` field forbids that. The bridge
therefore raises `BridgeError::UnsupportedHRTB` for HRTB-bearing
`Ty::Dynamic` (and `Ty::FnPtr`). Non-HRTB regions inside an
ordinary `dyn Trait` (the trait reference itself; any non-bound
projection arg) follow the standard region-erasure rule -- they
become `ReErased` and leave no IR trace, so a non-HRTB
`dyn Trait` and its source counterpart yield the same
`Ty::Dynamic` shape per the locked rule.

Custom auto-traits (declared via unstable `feature(auto_traits)`)
are rejected by the bridge with a clear diagnostic. The closed v1
set is `Send` / `Sync` / `Unpin`; widening is a local v2 change
(extend the enum + lift the bridge rejection).

**Reasoning.**

1. *Region erasure as a RegseqModel-wide invariant (with HRTB
   rejection in v1).* No `Ty` variant carries any region slot
   for free regions (see `discussions.md` Section 2.6
   region-erasure primer); free regions become `ReErased` and
   leave no IR trace. HRTB binders, however, survive rustc's
   `erase_and_anonymize_regions` pass per `findings-rustc-draft.md`
   W.13 (rustc retains `ReBound`). The same type-identity
   argument that locks D15's `FnAbi` field forbids collapsing
   HRTB-bearing and non-HRTB function-pointer / trait-object
   types under uniform erasure. v1 policy (per the D7/D8
   refinement, Section 1.8): the bridge **rejects** HRTB-bearing
   `Ty::FnPtr` and `Ty::Dynamic` at type translation with
   `BridgeError::UnsupportedHRTB`. Driver code does not need
   higher-ranked function pointers or trait-object binders;
   rejecting cleanly avoids the binder-encoding question
   entirely. Reopens if a future fixture or stage-14 intrinsic
   surfaces a stable need.
2. *Trait declarations are erased per stage-1.* Stage 1 erases
   trait declarations (`stage-1/spec-snapshot.md:608-611`) and
   associated types (`stage-1/spec-snapshot.md:605-607`); the closed
   six-variant `ItemKind` (`stage-1/spec-snapshot.md:625-627`) does
   not include `Trait` or `AssocType`, so traits and associated
   types have no `ItemId`. The trait identity must therefore flow
   through a different carrier. Stage-1 reserved the
   bridge-synthesised vtable Static Item per `(Trait, SelfTy)`
   (`stage-1/spec-snapshot.md:910-927`) plus an associated
   `def_path_hash` keyed on `(trait_def_path_hash,
   self_ty_canonical_hash)`. Stage-3 commits to a `vtable_ty:
   TyId` link from `Ty::Dynamic` to the (synthesised) vtable
   `Ty::Struct`; the synthesis machinery (vtable type Item,
   vtable instance Item, vtable shim Function Items, the
   `RegseqModel.vtables` dedup table) is detailed in
   `stage-12/expectations.md`.
3. *Auto-traits affect type identity, not C output.* `dyn Driver
   + Send` and `dyn Driver` are distinct Rust types -- the BiMap
   must dedup them as distinct `Ty::Dynamic` entries -- but the
   C-level fat pointer and vtable struct are identical. So
   `auto_traits` must appear in `Ty::Dynamic`'s structural
   identity (Hash + Eq) but does not affect downstream codegen
   shape.
4. *Closed enum vs DefPathHash for auto-traits.*
   `feature(auto_traits)` is unstable; user-defined auto-traits
   do not appear in driver DSL code. The closed-enum
   `AutoTrait::{Send, Sync, Unpin}` covers all v1 cases without
   per-trait DefPathHash machinery. Per the "no defensive
   fallbacks" rule, do not pre-bake user-defined auto-trait
   support.
5. *Canonical ordering for content-equality.* The `auto_traits`
   `Vec` must be sorted at construction so `dyn Driver + Send +
   Sync` and `dyn Driver + Sync + Send` produce identical
   `Ty::Dynamic` content (and dedup via the BiMap). Sort order:
   `Send < Sync < Unpin`, i.e., enum-discriminant order. Trivial
   because the enum is closed.

**Methodology rules invoked.**

- "C semantics with information preservation" -- preserve what C
  output needs (auto-traits affect identity but not codegen
  shape; vtable_ty bridges to all C-relevant content).
- "Don't redo work the rustc compiler has already done" -- type
  correctness for any HRTB obligations is verified pre-MIR by
  rustc.
- "No defensive fallbacks" -- no user-defined-auto-trait support;
  no silent erasure of surviving HRTB binders; rejection is
  explicit and surfaces the source span.
- "Reject what shouldn't appear; never speculate-handle" --
  HRTB-bearing `Ty::FnPtr` and `Ty::Dynamic` are rejected at the
  bridge in v1 (see Section 1.8 refinement) rather than admitted
  with an erasure policy that could collapse rustc-distinct
  types.
- *Region erasure as a RegseqModel-wide invariant* (see
  `discussions.md` Section 2.6) -- free regions erase uniformly;
  HRTB binders are explicitly rejected per the v1 policy above
  rather than silently erased.

**Prior-art evidence.**

- *rustc* (`findings-rustc-draft.md` Theme A.4): rustc's
  `TyKind::Dynamic(BoundExistentialPredicates, Region)` carries
  predicates, projections, auto-traits, and an outer region. We
  diverge on two axes: (a) the *outer* region -- a free region
  in MIR-local positions -- is erased per the wider
  region-erasure invariant, and (b) principal + projections are
  routed through `vtable_ty` rather than carried inline. The
  HRTB binder around the existential predicates (e.g.,
  `for<'a> dyn Trait<&'a T>`) is *not* erased -- v1 rejects
  HRTB-bearing `Ty::Dynamic` at the bridge per the D7/D8
  refinement (Section 1.8) instead, so the question of whether
  to encode the binder does not arise.
- *Charon* (`prior-art/charon.md` F.3): Charon's
  `TyKind::DynTrait(DynPredicate)` similarly carries a binder
  over predicates. Charon retains the binder because its
  verifier consumers want it; for our C-facing consumer we
  instead **reject** HRTB-bearing dyn types (the type-identity
  argument from D15 forbids erasing them, and v1 has no codegen
  consumer that needs the binder).

**Worked example.** See Section 4.9 (`&dyn Driver`).

#### 1.10.2 D5.2 -- Distribution of lowering work

**Decision.** D5 has no monolithic "lowering pass". The work
distributes across four layers, each in its natural stage:

| Layer | Stage | Deliverable |
| --- | --- | --- |
| Type-level: `Ty::Dynamic` shape and `vtable_ty` slot | 3 | `Ty::Dynamic { auto_traits, vtable_ty }` per D5.1 |
| Type-level: vtable `Ty::Struct` synthesis machinery (Item, layout, dedup table) | 12 | Per-(Trait, projections) vtable type Item; per-(Trait, SelfTy) vtable instance Item; vtable shim Function Items in two families -- per-(Trait, ConcreteImpl, method) method shims and per-(Trait, ConcreteImpl) drop-in-place shims, both bridging erased-receiver vtable slots to concrete-receiver targets; `RegseqModel.vtables` dedup table; concrete DefPath / hash rules; layout construction algorithm. See `stage-12/expectations.md`. |
| Statement-level: `Call` terminator shape supporting vtable-indirect dispatch | 9 | Stage-9 picks between two design candidates -- a public `Instance` discriminator addition (parallel to rustc's `Virtual` `InstanceKind`, requiring a stage-1 revision per `stage-1/spec-snapshot.md:747-754` since `fn_shim_disambig` is private) or a widened `Call.callee` shape (`Direct(InstanceId)` / `Indirect(Operand)`). See `stage-9/expectations.md`. |
| Statement-level: dispatch rewriting pass | 11 | Body-level pass that walks `Call` terminators, rewrites `<dyn Trait as Trait>::method` into vtable-indirect form (per stage-9), and recognises `Cast::Unsize` rvalues to trigger stage-12 vtable instance Item registration. See `stage-11/expectations.md`. |
| Codegen: fat-pointer C typedef | downstream | Pattern-match on `Ref(t, _)` / `Ptr(t, _)` where `t` resolves to `Ty::Dynamic`; emit fat-pointer typedef on demand (D5.3-B). |

The original framing (`discussions.md` D5 lean) considered a
Charon-style monolithic "lowering pass"; that framing collapses
because the work splits naturally across these four layers, with
each layer's deliverable owned by the stage that already covers
that domain.

**Reasoning.**

The bridge has all needed information from rustc's `Dynamic`
predicates at type-translation time, so the eager `vtable_ty`
population is bridge work. The vtable `Ty::Struct` synthesis
itself, however, is *Item* synthesis (per stage-1's
"Bridge-synthesized Items" section, `stage-1/spec-snapshot.md:901-942`)
which legitimately belongs to stage 12. Statement-level
rewriting walks bodies, fitting stage 11. Stage 9 owns the
terminator shape question that stage 11's pass produces.
Splitting the work across these stages avoids reclaiming
territory stage-1 reserved for downstream stages.

The fat-pointer C shape is symmetric with `&str` / `&[T]`: both
are unsized-pointee fat pointers in Rust; both lower to
`(data, metadata)` C struct typedefs; both stay as
`Ref(t, _)` (or `Ptr(t, _)`) in the IR with codegen emitting the
typedef on demand. Treating `&dyn Trait` as the third member of
the same family (D5.3-B) avoids special-casing.

**Methodology rules invoked.**

- "C semantics with information preservation" -- `Ty::Dynamic`
  survives in the IR through to codegen, preserving Rust intent
  for diagnostics and pretty-printing (stage 13).
- "Compiler-theory-first" -- distributing work to its natural
  layer (type / body / codegen) rather than inventing a new pass
  framework or reclaiming downstream territory.

**Prior-art evidence.**

- *Charon* (`prior-art/charon.md` F.3): Charon implements the
  body-level dispatch rewriting as a separate pass
  (`transform_dyn_trait_calls`); the type-level vtable synthesis
  happens during translate-types. Our distribution mirrors
  Charon's split, refined by the cross-stage layering imposed by
  stage-1's locks.

**Worked example.** See Section 4.9.

#### 1.10.3 D5.3-A -- Vtable type contract

**Decision.** `Ty::Dynamic.vtable_ty: TyId` resolves to a
synthesised `Ty::Struct` value. The `Ty::Struct`'s Item details
(DefPath segments, rendered form, hash rule, kind, fields,
layout, supertrait pointers) and surrounding machinery
(per-(Trait, SelfTy) vtable Static instance Items per stage-1's
reservation, vtable shim Function Items per
`InstanceKind::VTableShim` precedent, and the
`RegseqModel.vtables` dedup table) are pinned in
`stage-12/expectations.md`.

Stage-3 commits to:
- The `vtable_ty: TyId` slot is populated by the bridge at
  type-translation time when constructing `Ty::Dynamic`; never
  patched post-construction.
- The TyId resolves to a `Ty::Struct` value (i.e., not to
  `Ty::Dynamic`, `Ty::Tuple`, or any non-ADT-shaped variant).
- Two `Ty::Dynamic` values with identical `(auto_traits,
  vtable_ty)` content are equal under the BiMap's structural
  identity (D3).

The `Ty::Struct` carries an `(item: ItemId, args: GenericArgs,
layout: Layout)` triple per D2; the synthesised Item that
populates `item` is itself a stage-12 deliverable. Stage-3
commits only to the contract that some Item exists in
`RegseqModel.items` for which `ty: Ty::Struct(item, args, layout)`
is the vtable type, and that the bridge can mint and intern this
`Ty::Struct` at the time it constructs `Ty::Dynamic`.

**Reasoning.**

1. *Stage-1 reserved the synthesised vtable Item slot.*
   `stage-1/spec-snapshot.md:910-927` established the bridge-synthesised
   vtable Item is per-(Trait, SelfTy), kind=Static, rendered
   form `<vtable for Trait for SelfTy>`, hash =
   `(trait_def_path_hash, self_ty_canonical_hash)`.
   Stage-1:893-895 reserved the precise vtable struct `Ty` for
   "stage 9/12 when vtable lowering is implemented." D5.3-A
   respects that reservation by pinning only the stage-3-side
   contract (the `vtable_ty` slot) and forwarding the synthesis
   details to stage-12.
2. *Two synthesised Items are needed (vtable type + vtable
   instance), per the type-vs-instance split.* The vtable C
   struct (`struct Driver_VTable { ... }`) is per-(Trait,
   projection_bindings); the vtable instance (`static const
   Driver_VTable __vtable_PL011_for_Driver = ...`) is per-(Trait,
   ConcreteImpl/SelfTy). Stage-1's reservation covers the
   instance Item; the *type* Item is a SESSION-A-recognised
   addition forwarded to stage-12 for spec.
3. *Vtable shim Function Items are needed for receiver erasure
   (two families).* Vtable function-pointer fields use erased
   receivers (`*const ()` / `*mut ()`) throughout, but the
   targets they reference have concrete receivers: lifted impl
   methods carry concrete receivers per
   `stage-1/spec-snapshot.md:597-603`, and rustc's `DropGlue`
   Instance is `core::ptr::drop_in_place::<SelfTy>` with
   concrete `*mut SelfTy` receiver per
   `rustc_middle/src/ty/instance.rs:152-157`. The standard
   resolution (per rustc's `InstanceKind::VTableShim`) is
   bridge-synthesised shims that take the erased receiver, cast
   to concrete, and tail-call the concrete target. Two families
   apply: method shims per (Trait, ConcreteImpl, method) for the
   trait method slots, and a drop-in-place shim per (Trait,
   ConcreteImpl) for the drop slot. Stage-12 specifies the
   synthesis (Section C.1 method shims, C.2 drop-in-place
   shim).
4. *Cross-translation-unit references need a stable Item.* The
   `extern const Driver_VTable __vtable_...;` declarations in C
   output need a stable IR identity; both the type and instance
   Items provide that, per stage-1's bridge-synthesised Item
   framework.

**Methodology rules invoked.**

- "Compositional behavior beats special-case flags" -- vtables
  go through the same `Ty::Struct` machinery as any other
  nominal struct; no `Ty::VTable` variant.
- "Compiler-theory-first" -- the per-(Trait, projections) /
  per-(Trait, Impl) split is the standard vtable factoring used
  by rustc, Charon, and CLR; vtable shims for receiver erasure
  follow rustc's `VTableShim` precedent.
- "Don't redo work the rustc compiler has already done" --
  stage-12 will reuse rustc's `tcx.vtable_entries(trait_ref)`
  output for the field set; the bridge does not recompute.

**Prior-art evidence.**

- *Stage-1* (`stage-1/spec-snapshot.md:884-895, :901-942`): defines
  the per-(Trait, SelfTy) vtable Static Item shape, rendered
  form, hash composition, and DefPathData widening procedure.
  D5.3-A defers to this reservation.
- *rustc* (`stage-1/spec-snapshot.md:786-805`): rustc's
  `InstanceKind` taxonomy includes `VTableShim`, `Virtual`,
  `ReifyShim`, `FnPtrShim`, etc. Stage-12 expectations.md
  reflects which of these we need (VTableShim for receiver
  erasure; Virtual handed off to stage-9).
- *rustc_codegen_clr* (`prior-art/rustc-codegen-clr.md` B): the
  fat-pointer pointee class is a synthesised `ClassDef` per
  unsized pointee shape -- precedent for fat-pointer synthesis
  (codegen-emitted in our case per D5.3-B), but CLR's
  `TyKind::Dynamic` returns a `ClassRef` named "Dyn" with no
  fields (`prior-art/rustc-codegen-clr.md:227-228`); CLR
  diverges on the vtable nominalisation question.
- *Charon* (`prior-art/charon.md` F.3): the vtable struct is a
  real `TypeDecl` registered alongside `DynPredicate`. Direct
  precedent for nominal vtable structs.
- *mrustc* (`prior-art/mrustc.md` F): vtable values are stored in
  `static`s of struct types whose layout is computed via rustc's
  layout rules.

**Worked example.** See Section 4.9.

#### 1.10.4 D5.3-B -- Fat-pointer C shape is codegen-emitted

**Decision.** `Ty::Dynamic` survives in the IR through to
codegen. The lowering work in D5.2 introduces only the vtable
`Ty::Struct` (via stage-12's synthesis machinery) and the
`vtable_ty: TyId` slot in `Ty::Dynamic`; it does **not** rewrite
`Ref(t, _)` or `Ptr(t, _)` (where `t` resolves to `Ty::Dynamic`)
to anything else. After bridge construction, `Ty::Dynamic`
appears in the IR exactly where the source Rust used `dyn
Trait`, and codegen handles the fat-pointer materialisation.

C codegen pattern-matches `Ref(t, _)` and `Ptr(t, _)` where `t`
resolves to `Ty::Dynamic`, and emits a fat-pointer typedef
inline. The qualifications on the `data` field follow the locked
D7/D8 shapes (Section 1.8) and the C-qualification policy in
Section 4.1. Four pointer-kind variants exist for `dyn Trait`
pointees:

| Source | IR shape | C `data` qualification |
| --- | --- | --- |
| `&dyn Trait` | `Ref(t_dyn, Provenance::SharedBorrow)` | `const void *data` |
| `&mut dyn Trait` | `Ref(t_dyn, Provenance::UniqueBorrow)` | `void *data` |
| `*const dyn Trait` | `Ptr(t_dyn, Mutability::Not)` | `const void *data` |
| `*mut dyn Trait` | `Ptr(t_dyn, Mutability::Mut)` | `void *data` |

**Trait-object override on D7/D8's UnsafeCell + `restrict`
policy.** Locked D7/D8 specifies two type-aware adjustments to
the baseline pointer qualifications: (i) `const` is dropped from
`Ref(_, SharedBorrow)` when the pointee transitively contains
`UnsafeCell`; (ii) `restrict` is added to `Ref(_, UniqueBorrow)`
in declaration contexts where uniqueness is preserved. Both
adjustments **do not apply** to the `data` field of a
trait-object fat pointer:

- *No `UnsafeCell` const-drop.* The trait-object `data` is
  typed as `void *` because the underlying concrete type is
  erased; codegen cannot inspect the erased pointee for
  `UnsafeCell` content at the trait-object level. The four
  variants in the table above use the borrow-flavor / pointer-
  kind qualifications only. If a codegen consumer materialises
  a need for type-aware const-drop on a specific trait, the
  override is per-`(Trait, projection_bindings)` codegen
  policy, not stage-3 IR content.
- *No `restrict` on the `data` field.* Even for `&mut dyn
  Trait` the data pointer is `void *`; adding `restrict` would
  assert non-aliasing on the erased pointee, which the
  codegen layer has no type-level evidence to support without
  walking the trait-object's vtable to its concrete impl. The
  conservative choice is to omit `restrict` from the
  trait-object's `data` field at the type-erased level. If a
  later analysis pass refines this (e.g., when a virtual call
  is devirtualised to a concrete impl), the codegen layer can
  upgrade to `restrict` post-devirtualisation.

The `vtable` field is unaffected by either policy: it always
points at a `static const <Trait>_VTable`, qualified `const`
because vtables are immutable post-link, with no `UnsafeCell`
or aliasing concerns.

```c
/* Schematic; concrete typedef name derives from codegen's
 * name-mangling rule (see "Typedef-name uniqueness" below). */
typedef struct <fat_pointer_type_name> {
    <const|/**/> void *data;          /* const for SharedBorrow / Ptr(Not); else absent */
    const <Trait_VTable_C_name> *vtable;
} <fat_pointer_type_name>_t;
```

The `vtable` field is always `const`-qualified pointing at a
`const <Trait>_VTable`, since vtables are `static const` in C
output regardless of the enclosing pointer mutability.

**Typedef-name uniqueness.** The fat-pointer typedef name must
distinguish trait objects whose principal trait is the same but
whose projection bindings differ -- e.g., `dyn Iterator<Item =
u32>` vs `dyn Iterator<Item = u8>` are distinct vtable types
(per stage-12's per-`(Trait, projection_bindings)` vtable type
Item) and therefore distinct fat-pointer types in C. The
codegen-side naming rule (which composes the trait identity, a
discriminator for `projection_args`, and the pointer-kind
variant) is part of the codegen layer's name-mangling
responsibility; D5.3-B specifies the *uniqueness obligation*
but not the concrete mangling. Stage-12 produces the vtable
type Item with a discriminating DefPath / hash; codegen derives
the fat-pointer C name from that DefPath plus the pointer-kind
qualifier.

Codegen reaches the vtable struct's C name via
`Ty::Dynamic.vtable_ty`, resolved to the corresponding
`Ty::Struct` whose synthesised Item supplies the C identifier.

**Reasoning.**

1. *Symmetric with `&str` / `&[T]`.* `Ty::Str` and `Ty::Slice(T)`
   already survive in the IR; codegen pattern-matches `Ref(t, _)`
   where `t` resolves to `Ty::Str` and emits `rust_str_t`, and
   similarly for `Ty::Slice(T)` (Section 4.2). `&dyn Trait` is
   the third unsized-pointee fat-pointer shape; handling it the
   same way avoids special-casing in the IR.
2. *Methodology rule "C semantics with information
   preservation."* Keep Rust intent until codegen needs the C
   shape. An IR-level rewrite of `Ref(t_dyn, _)` to
   `Ty::Tuple { fields: [Ptr(unit, _), Ptr(VTable_T, Not)],
   layout: <fat-ptr layout> }` would discard the `Ty::Dynamic`
   marker, costing diagnostics, pretty-printing fidelity
   (stage 13), and analysis precision.
3. *Mutability follows the enclosing pointer.* The same `dyn
   Trait` pointee is reachable through shared, mutable, and raw
   pointer types; codegen distinguishes them by the C
   qualifications on the `data` field, mirroring how Section 4.1
   handles non-`dyn` references.
4. *No new IR variant family needed.* Both options (rewrite vs
   survive) work within the locked D1.1 variant set; the survive
   option does so while keeping `Ty::Dynamic` semantically
   meaningful end-to-end.

**Methodology rules invoked.**

- "C semantics with information preservation" (cited above).
- "Compositional behavior beats special-case flags" -- `&dyn
  Trait` lowers via the same codegen pattern-match as `&str` and
  `&[T]`.
- "No defensive fallbacks" -- no IR-level rewrite "in case
  codegen wants the lowered form pre-baked."

**Prior-art evidence.**

- *Stage-3 existing* (Section 4.2): `&str` and `&[T]` worked
  example shows the precedent for codegen-emitted fat-pointer
  typedefs. The `dyn Driver` case in that section -- originally
  noted as "abstract pending D5; the fat-pointer shape shown is
  the eventual codegen target" -- is resolved by D5.3-B as that
  section foreshadowed.
- *mrustc* (`prior-art/mrustc.md` F): mrustc's `emit_ctype_ptr`
  pattern-matches the metadata kind (`MetadataType::Slice` /
  `::TraitObject` / `::None`) and emits the appropriate fat
  pointer typedef on demand. Codegen-emitted, exactly mirroring
  our approach.

**Worked example.** See Section 4.9.

### 1.11 Decision D6 -- Closure naming and synthesis

D6 has three sub-decisions: D6.1 (Item materialization), D6.2
(DefPath naming), D6.3 (`Ty::Closure` connection to its captures
struct). All three are locked at the stage-3 level. The detailed
mechanics of synthesising the closure-as-struct Item, the call
methods' Function Items, and the optional drop-glue Function Item
are forwarded to `stage-12/expectations.md` (synthesised Item
machinery is stage-12's domain per stage-1's "Bridge-synthesized
Items" framework).

#### 1.11.1 D6.1 -- Item materialization per closure (contract)

**Decision.** For each closure encountered in MIR, the bridge
synthesises:

1. **One captures-struct Item.** `ItemKind::Struct`. Fields = the
   captured upvars (per rustc's `tupled_upvars_ty()`,
   `findings-rustc-draft.md` A.3). Layout from
   `tcx.layout_of(closure_ty)` per D2. DefPath: rustc's closure
   DefPath verbatim (uses `DefPathData::Closure` per
   `stage-1/spec-snapshot.md:472`).
2. **1-3 Function Items for call methods**, depending on the
   closure's `ClosureKind` (`findings-rustc-draft.md` A.3 line
   127). Per the standard Rust trait hierarchy `Fn` : `FnMut` :
   `FnOnce`:
   - `FnOnce` closure: `call_once` only.
   - `FnMut` closure: `call_mut` plus inherited `call_once`.
   - `Fn` closure: `call` plus inherited `call_mut` and
     `call_once`.
   - Each Function Item's signature is substituted from the
     closure's `closure_sig_as_fn_ptr_ty`, with the
     captures-struct as the receiver type.
3. **A drop-glue Function Item iff the captures need dropping.**
   Synthesised by the same machinery rustc/the bridge use for
   any drop-glue (lifted Function Item with `drop_in_place`-like
   shape); not closure-specific.
4. **Call-method ABI is `FnAbi::Rust`.** Rustc's source-level ABI
   for closure call methods is `RustCall` (the closure-trait
   tuple-receiver convention), which D15 (Section 1.9) rejects.
   The bridge **rewrites** synthesised call-method Function Items
   to `FnAbi::Rust` after the receiver/argument tuple
   normalization that D6.1 bullet 2 already performs: the
   captures-struct is the receiver directly (not wrapped in a
   `RustCall` tuple) and call arguments are ordinary positional
   parameters (not packaged into a tuple). Post-rewrite,
   `FnAbi::Rust` is the correct calling convention; type-identity
   is preserved because `RustCall` no longer appears in the IR
   and no two rustc-distinct closures collapse under the
   normalization (the rewrite is the inverse of `RustCall`'s
   tuple-packing definition). This is the only D15 exception,
   justified by the `RustCall` -> `Rust` equivalence under tuple
   unpacking. The drop-glue Function Item from bullet 3 is
   ordinary `FnAbi::Rust` (drop-glue uses the standard Rust ABI,
   not `RustCall`).

`Coroutine` / `CoroutineClosure` / `CoroutineWitness` are
rejected per D1.4.

The detailed *synthesis mechanics* (how the bridge produces
these Items, ItemId assignment, Item field population, hash
construction) are `stage-12/expectations.md` content. Stage-3
commits to the above contract: which Items exist per closure,
what their kinds and DefPaths are, and how `Ty::Closure` connects
to them.

**Reasoning.**

1. *Materialised projections, not abstract trait impls.* Per
   stage-1 (`stage-1/spec-snapshot.md:600-611`) trait declarations
   and impl blocks are erased in our IR; impl methods are lifted
   to top-level Function Items. Closures inherit the same model:
   Fn/FnMut/FnOnce impls are erased; their *projections* (the
   call methods) become Function Items.
2. *Trait hierarchy determines the count.* `Fn` extends `FnMut`
   extends `FnOnce`; a `Fn` closure satisfies all three traits,
   so all three call methods are materialised. Skipping
   inherited methods would mean a `Fn` closure could not be
   coerced to `&dyn FnOnce` (the FnOnce vtable would have no
   `call_once` to point at).
3. *Drop-glue handled uniformly.* No closure-specific drop-glue
   pathway; the bridge's drop-glue machinery applies to closures
   the same as to any struct with non-trivial fields.
4. *Captures-as-fields, not as a hidden tuple.* The captures
   struct's fields are individually named (matching rustc's
   upvar naming, e.g., `_0`, `_1`, ...). The struct's layout is
   computed by rustc and copied into the Item; we do not
   re-derive it.

**Methodology rules invoked.**

- "Don't redo work the rustc compiler has already done" --
  upvar capture analysis, layout computation, ClosureKind
  determination are all rustc's responsibility; we copy the
  results.
- "Compositional behavior beats special-case flags" -- closures
  use the same Item / Instance / Ty::Struct machinery as any
  other ADT, plus standard Function Item machinery for the call
  methods.

**Prior-art evidence.**

- *Charon* (`prior-art/charon.md` F.2,
  `translate_closures.rs`): "we convert closures to a struct
  containing the closure's state (upvars), along with matching
  trait impls and fun decls (e.g. a `Fn` closure will have a
  trait impl for `Fn`, `FnMut` and `FnOnce`, along with 3
  matching method implementations for `call`, `call_mut` and
  `call_once`)." Direct precedent.
- *mrustc* (`prior-art/mrustc.md` F closure section): "Closures:
  by codegen time these are nominal Path types (`closure#0`-style
  names). They are emitted exactly like ordinary structs through
  `emit_struct` -- no special closure path in the C backend."
  Confirms the captures-struct-emitted-as-ordinary-struct
  approach.
- *rustc_codegen_clr* (`prior-art/rustc-codegen-clr.md` B,
  `closure_typedef`): closures are `ClassDef`s with fields
  labelled `f_0`, `f_1`, etc. -- the upvar tuple flattened to
  named fields.
- *rustc* (`findings-rustc-draft.md` A.3,
  `ClosureArgs::split()`): bridge consumes
  `ClosureArgsParts { parent_args, closure_kind_ty,
  closure_sig_as_fn_ptr_ty, tupled_upvars_ty }`. Each piece
  feeds a specific synthesis decision.

**Worked example.** See Section 4.10.

#### 1.11.2 D6.2 -- DefPath naming policy

**Decision.** The bridge reuses rustc's DefPath verbatim for the
captures-struct Item, and constructs a child DefPath for each
call-method Function Item by appending a `ValueNs(StringId)`
segment with `disambiguator = 0`.

For the captures-struct Item: the DefPath comes from
`tcx.def_path(closure_def_id)`, mapped through stage-1's
`DefPathData` translation. Stage-1's `DefPathData::Closure`
variant (`stage-1/spec-snapshot.md:472`) is the anon-namespace
marker; rustc's per-sibling disambiguator (`#0`, `#1`, ...) flows
into the `DisambiguatedDefPathData::disambiguator` field via
stage-1's `DefPathSegment.disambiguator`. Verbose rendering
follows stage-1's `{namespace#N}` rule
(`stage-1/spec-snapshot.md:545-554`).

For call-method Function Items: the bridge appends
`DefPathSegment { data: ValueNs(intern("call" / "call_mut" /
"call_once")), disambiguator: 0 }` to the closure's DefPath.
Rustc itself does not have `DefId`s for these synthetic methods
-- they are the bridge's projection of the implicit
Fn/FnMut/FnOnce impls.

The resulting verbose-rendered paths:

```
crate::module::fn_name::{closure#0}                    // captures struct
crate::module::fn_name::{closure#0}::call_once         // FnOnce projection
crate::module::fn_name::{closure#0}::call_mut          // FnMut projection (if applicable)
crate::module::fn_name::{closure#0}::call              // Fn projection (if applicable)
```

**Reasoning.**

1. *DefPath reuse gives cross-build identity for free.* Rustc's
   DefPaths are stable across builds via `DefPathHash`
   (`stage-1/spec-snapshot.md` F4). Reusing them feeds directly
   into the F4-2 deep canonicalizer (D3 reasoning constraint 1)
   for cross-build determinism.
2. *No new DefPathData variants needed.* The bridge's child
   segments use the existing `ValueNs(StringId)` variant, which
   stage-1 already provides for ordinary fn names. The synthetic
   method names (`"call"` / `"call_mut"` / `"call_once"`) intern
   into `RegseqModel.strings` via the standard
   `StringInterner.intern`. No DefPathData widening for
   closures.
3. *Disambiguation is handled by rustc.* Sibling closures in
   the same scope are disambiguated by rustc's
   `DisambiguatedDefPathData::disambiguator`; the bridge does
   not need its own counter.
4. *Methods do not collide with sibling fns.* The closure's own
   DefPath is the parent; child names `call`, `call_mut`,
   `call_once` are distinct from any user-defined fn in the
   crate, since they live under an anon-namespace
   `{closure#N}` parent.

**Methodology rules invoked.**

- "Don't redo work the rustc compiler has already done" --
  rustc's DefPath construction (including disambiguation)
  is reused verbatim.
- "Compiler-theory-first" -- synthetic method names follow the
  Rust standard library's own naming for the trait methods
  (`Fn::call`, `FnMut::call_mut`, `FnOnce::call_once`).

**Prior-art evidence.**

- *mrustc* (`prior-art/mrustc.md` G.1): closures appear as
  `closure#0`-style paths in the Rust-syntax printer, mirroring
  rustc's own naming convention.
- *Charon* (`prior-art/charon.md` F.2): synthesised
  `TypeDecl`s for closures use names of the form
  `{parent_fn::closure#0}<...>` -- direct rendering of the
  rustc DefPath.
- *Stage-1* (`stage-1/spec-snapshot.md:472, 545-554`):
  `DefPathData::Closure` is the anon-namespace marker;
  `{namespace#N}` is the verbose rendering rule.

**Worked example.** See Section 4.10.

#### 1.11.3 D6.3 -- `Ty::Closure` connection to captures struct

**Decision.** `Ty::Closure(ItemId, GenericArgs)` is a tuple
variant carrying only the closure's source-level identity (Item
+ resolved args). The corresponding captures struct is reached
implicitly through the shared `ItemId`, by structural-identity
lookup in the Ty interner:

```rust
fn captures_struct_of(
    model: &RegseqModel,
    closure_ty: TyId,
) -> Option<TyId> {
    let Ty::Closure(item, args) = model.types.resolve(closure_ty) else { return None };
    let probe = Ty::Struct {
        item:   *item,
        args:   args.clone(),
        layout: Layout::pending(),    // ignored by ADT-shaped find()
    };
    model.types.find(&probe)
}
```

The `find` method (D4) ignores `layout` for ADT-shaped identity
(per D3 nominal/structural split), so the lookup succeeds
regardless of which `layout` placeholder is in the probe.

`Ty::Closure` carries no inline layout; the layout lives on the
parallel `Ty::Struct` entry that shares the same `(ItemId,
GenericArgs)`.

**Reasoning.**

1. *No information loss from omitting an explicit slot.* The
   shared `(ItemId, GenericArgs)` *is* the connection; an
   explicit `captures_ty: TyId` slot would be redundant
   duplication. This contrasts intentionally with
   `Ty::Dynamic.vtable_ty` (D5.1), where the slot is
   information-bearing because the vtable's ItemId is *not* the
   same as anything else in `Ty::Dynamic`'s content.
2. *Two `Ty` entries per closure, one Item.* The closure has two
   `Ty` entries -- `Ty::Closure(item, args)` (identity view) and
   `Ty::Struct(item, args, layout)` (layout-bearing view) --
   sharing one synthesised Item (D6.1). The two views serve
   distinct purposes: `Ty::Closure` for type-level identity in
   fn signatures and Place types; `Ty::Struct` for the
   layout/codegen view. Pretty-printer (stage 13) renders
   `Ty::Closure` differently from `Ty::Struct` to preserve Rust
   intent.
3. *Consistent with stage-1's view.* Stage-1
   (`stage-1/spec-snapshot.md:842, 866-867`) places struct/enum/union
   layouts on `Ty::Struct` / `Ty::Enum` / `Ty::Union`, with
   `InstanceKind::Struct { layout: TyId }` pointing at the
   layout-bearing Ty. Closures fit the same model: their layout
   lives on the captures `Ty::Struct`; their corresponding
   `Instance` (kind: Struct, layout: <captures_TyId>) points at
   it.

**Methodology rules invoked.**

- "Compositional behavior beats special-case flags" -- no
  closure-specific slot; the connection mechanism is the same
  shared-(item, args) pattern used elsewhere.
- "C semantics with information preservation" -- `Ty::Closure`
  preserves the closure-as-callable identity through to codegen;
  `Ty::Struct` preserves the layout. Both views co-exist without
  redundancy.

**Prior-art evidence.**

- *rustc* (`findings-rustc-draft.md` A.3): rustc has only
  `TyKind::Closure(DefId, GenericArgs)` -- not a parallel
  `TyKind::Adt`. We diverge by interning two parallel `Ty`
  entries (Closure + Struct) sharing the Item, because our
  layout-bearing variant set is `Struct/Enum/Union` only (D2)
  and `Ty::Closure` is identity-only.
- *Charon* (`prior-art/charon.md` F.2): closures are
  represented as `TyKind::Adt(<closure ADT ref>)` directly --
  no separate closure variant. We retain `Ty::Closure` (D1.1)
  for pretty-printer fidelity but reach the layout via the
  parallel `Ty::Struct` view, which is structurally what Charon
  does.

**Worked example.** See Section 4.10.

### 1.12 Framing principle -- no QualType layer

Stage 3's locked decisions, taken together, commit to a meta-
pattern that has not yet been named explicitly in earlier
sections: **regseq-model's `Ty` represents the unqualified Rust
type; C-side qualifiers (`const`, `volatile`, `restrict`, and
similar) are emitted at codegen time from per-decl / per-op
information accumulated elsewhere in the IR**. There is no
Clang-style `QualType` wrapper that pairs an unqualified type
with a `QualifierFlags` bitfield, and no `Ty::ConstQualified(TyId)` /
`Ty::Volatile<T>` variant.

Each C qualifier already maps to a distinct, semantically-natural
IR site:

| C qualifier | Where it's derived from in our IR | Source / status |
| --- | --- | --- |
| `const` on a reference (`Ty::Ref`) | `Provenance` on `Ty::Ref(TyId, Provenance)` (locked D7/D8: 2-arg form, Provenance subsumes Mutability for refs) plus the transitive `is_unsafe_cell` flag on `Item` (the flag drops `const` when the referent transitively contains `UnsafeCell`) | D1.1 LOCKED + D1.5 LOCKED + D7/D8 LOCKED (Section 1.8) |
| `const` on a raw pointer (`Ty::Ptr`) | `Mutability` on `Ty::Ptr(TyId, Mutability)` only (`*const T` vs `*mut T`; locked D7/D8 renames `RawPtr` -> `Ptr`); the `is_unsafe_cell` flag does *not* apply to raw pointers per locked D1.5 | D1.1 LOCKED + D7/D8 LOCKED |
| `static` (storage class / linkage) | `Item` decl attributes (stage-12 territory: name mangling and linkage) | stage-1 Item vocabulary LOCKED |
| `restrict` | `Provenance::UniqueBorrow` on `Ty::Ref` in declaration contexts where uniqueness is preserved | D1.1 LOCKED + D7/D8 LOCKED; Section 4.1 worked example shows the projection |
| `volatile` | intrinsic `InstanceId` identity (closed MMIO-intrinsic vocabulary; the `Call` terminator's callee identity is the carrier) | D9 LOCKED (Section 1.13) |

**Why no QualType layer.**

- *Clang has `QualType` because C's type system genuinely treats
  `const int` as a distinct type from `int`* -- C expression-
  result types depend on the qualifier (`*p` for `p: const int*`
  has type `const int`). A C frontend has to model that.
- *Rust's MIR does not have type-level qualifiers in that sense.*
  `const`-on-pointee is encoded by the `*const T` vs `*mut T`
  and `&T` vs `&mut T` distinction (already distinct `Ty`
  variants in our IR via the `Mutability` and `Provenance`
  fields); `volatile` is intrinsic-level, never a type
  qualifier; `static` is an item attribute. Nothing in Rust's
  source-language type system forces a QualType wrapper on our
  IR.
- *Methodology -- "C semantics with information preservation":*
  preserve what rustc actually computed; do not import a
  C-frontend-specific layer that rustc deliberately does not
  use.
- *Methodology -- "compositional behavior beats special-case
  flags":* each qualifier already falls out of existing fields.
  Adding a `qualifiers: QualifierFlags` field on `Ty` would
  duplicate information and create divergence questions ("what
  if QualType says one thing and the per-decl data says
  another?").
- *Methodology -- "compiler-theory-first":* C qualifiers are an
  artifact of C's surface syntax; in the resolved post-mono Rust
  view, the same information is carried at the operation /
  declaration / item level. The IR should mirror the resolved
  structure, not the surface syntax of the target language.

**How future qualifier questions resolve under this frame.**
A new C qualifier (or a new question about an existing one)
should be answered by identifying the IR site that already
carries the necessary information; only if no such site exists
does the question expand to "should we add a new field?", and
even then the field should land at the operation / declaration
level, not on `Ty`. D9 below applies the principle to
`volatile`; D10 applies it to atomicity.

### 1.13 Decision D9 -- Volatile semantics: per-intrinsic-InstanceId, no Ty bit

**Decision.** `Ty` does not carry any volatile annotation. The
regseq-model IR has no `volatile: bool` field at any level --
not on `Ty`, not on a generic load/store `Statement` (stage 9
territory), not on the `Call` terminator. Volatility is encoded
**on the intrinsic `InstanceId`**: MMIO-access intrinsics
recognised by the closed intrinsic vocabulary
(`#[rseq::intrinsic("mmio_read")]`,
`#[rseq::intrinsic("mmio_write")]`, and the `*_field` variants
exemplified in `tests/fixtures/pl011-full-spec/src/regseq/mmio.rs:35-71`)
are the carriers. Stage-13 codegen for those `InstanceId`s emits
volatile-marked C accesses (`*(volatile uint32_t*)addr` and the
write counterpart); the IR carries no IR-level flag.

The bridge **rejects** raw `core::ptr::read_volatile` and
`core::ptr::write_volatile` calls at `Call`-terminator
translation. All MMIO access in the regseq DSL must flow through
the `#[rseq::intrinsic("mmio_*")]` API. This rejection is the
analog of D1.4 Category A applied to *function calls* rather
than *types*: rigid Rust constructs that exist in `core` but
lie outside the regseq DSL surface are rejected at the bridge
with a clear diagnostic. Enforcement lives at stage 9 / stage
13 (the Call-translation and intrinsic-recognition layers
respectively); D9 records the policy here for audit
completeness.

This decision is the application of the Section 1.12 framing
principle ("no QualType layer") to the volatile qualifier.

**Reasoning.**

Stage 3 owns the question "does `Ty` carry a volatile bit?"
The answer is no, by direct application of the Section 1.12
framing principle. Adopting type-level volatile would be a
one-off departure from the pattern that already governs `const`,
`static`, and `restrict`.

Among the three options stage 3 had to disambiguate -- per-type
(CompCert-style `attr_volatile: bool`), per-op-on-Statement
(CLR-style `LdInd { volatile: bool }`), or
per-intrinsic-InstanceId (no IR flag at any level) -- the third
follows from already-locked commitments rather than from
prior-art arbitration:

1. *Q7 + the closed intrinsic vocabulary* already commit to
   `#[rseq::intrinsic("NAME")]`-attribute-driven recognition
   with unknown names rejected. The infrastructure to encode
   "this `Call` is to an MMIO operation" already exists at the
   `InstanceId` level.
2. *The DSL three-mechanism convention* (`regseq-background.md`
   lines 14-27) places MMIO unambiguously in intrinsic territory
   (Intrinsics for operational verbs; Properties for declarative
   adjectives; Traits for structural integration). The pl011
   fixture's `mmio.rs` (lines 35-71) confirms intrinsic-only
   usage as the actual driver pattern, not aspirational design.
3. *DSL surface rule* ("Regseq DSL uses a limited subset of
   `core`. No other crates.") plus the methodology rule "no
   defensive fallbacks" jointly imply rejection of raw
   `core::ptr::{read,write}_volatile`: drivers do not need them
   when the intrinsic API exists; if they appear, that is a
   DSL-scope violation, not a pattern to support. This is the
   same posture D1.4 Category A takes for `Box`, `Coroutine`,
   etc. -- restated for function calls instead of types.
4. *Compositional behavior beats special-case flags.* If
   volatile lives on the `InstanceId`, no `Ty` flag is needed
   and no Statement flag is needed.
5. *Stage-3 footprint becomes minimal and precise.* D9's
   stage-3 lock answers "is there a `Ty` bit?" with a justified
   no, and forwards the enforcement question to the layers
   that already own intrinsic recognition (stage 9 + stage 14).

The CompCert outlier (per-type `attr_volatile`) is justified by
trace-semantics observability: CompCert's verified semantics
must record volatile operations as observable behaviour
(`prior-art/compcert.md` Theme D, citing the CompCert manual
lines 729-746). This rationale does not transfer: we emit C and
have no trace-semantics requirement.

**Methodology rules invoked.**

- "C semantics with information preservation" -- preserve
  rustc's view (no type-level volatile); emit C qualifiers at
  codegen from per-decl / per-op information accumulated
  elsewhere in the IR.
- "Compositional behavior beats special-case flags" -- each C
  qualifier maps to an existing IR site (Section 1.12 table).
- "No defensive fallbacks" -- bridge rejects raw
  `core::ptr::{read,write}_volatile` rather than carrying a
  speculative IR flag for consumers that do not exist in v1.
- "Don't redo work the rustc compiler has already done" --
  rustc routes volatile through intrinsics; we mirror that
  posture.
- "Compiler-theory-first" -- volatile is a memory-effect
  property of an operation, not a type-level shape. Type-level
  volatile is justified only when the IR's semantics must
  observe volatile at the type-system level (CompCert's trace
  semantics); we have no such constraint.

**Prior-art evidence.**

- *LLVM* -- volatile is a per-op flag on load/store
  instructions; the type system does not carry it
  (`prior-art/llvm-ir.md` Theme D, lines 311-336).
- *mrustc* -- volatile is not used in the C output for Rust
  types; appears only as `__asm__ __volatile__` inside
  intrinsic implementations; atomics realised through
  intrinsic helpers (`prior-art/mrustc.md` Theme D, lines
  516-518, 520-536).
- *Charon* -- atomic types and marker types are ordinary
  `Adt(TypeDeclRef)` instances with no type-level markers
  (`prior-art/charon.md` Theme D, lines 609 and 664). Theme D
  does not specifically address volatile -- the absence of a
  volatile marker in Charon's TyKind is suggestive, not
  load-bearing.
- *rustc_codegen_clr* -- per-op boolean flag on
  `LdInd { addr, tpe, volatile: bool }` and
  `StInd(addr, value, type, is_volatile)`; "does not bake any
  'special semantics' into `Type`"
  (`prior-art/rustc-codegen-clr.md` Theme D, lines 328-382).
- *rustc-codegen-c* -- no `const` / `volatile` / `restrict`
  qualifiers anywhere on `CTy`
  (`prior-art/rustc-codegen-c.md` Theme D, lines 224-242).
- *MLIR* -- type traits / parameters / type interfaces; none
  embed volatility into the type itself
  (`prior-art/mlir.md` Theme D, lines 365-405).
- *CompCert (sole outlier)* -- `attr_volatile: bool` on every
  `attr` record in `Ctypes.type`; required because CompCert's
  *trace semantics* must record volatile operations as
  observable behaviour (`prior-art/compcert.md` Theme D, lines
  415-434, citing the CompCert manual).

Rustc's own posture mirrors the convergent group: volatile is
intrinsic-level (`ptr::read_volatile`, `ptr::write_volatile`);
no type variant carries volatile semantics
(`findings-rustc-draft.md` lines 570-572). regseq-model
follows rustc's posture, refined for our DSL surface (raw
volatile rejected; MMIO via intrinsic only).

**DSL evidence.**

- Three-mechanism convention (`../regseq-background.md` lines
  14-27): operational verbs (Intrinsics), declarative adjectives
  (Properties), structural integration (Traits). MMIO is
  category-operational.
- pl011 fixture
  (`tests/fixtures/pl011-full-spec/src/regseq/mmio.rs:35-71`):
  `#[rseq::intrinsic("mmio_read")]`, `mmio_write`,
  `mmio_read_field`, `mmio_write_field` -- specific names; the
  intrinsic methods are DSL stubs lowered by the compiler
  backend.
- Stage 1 Q7 (`stage-1/spec-snapshot.md` lines 869-899;
  `architecture.md` Q7): closed-intrinsic-vocabulary recognition
  via `#[rseq::intrinsic("NAME")]`-attribute on the callee,
  unknown names rejected.
- Stage 14 (`stage-14/expectations.md`): the MMIO intrinsic
  vocabulary entries and Call-translation enforcement of D9's
  `*_volatile` rejection are forward-expectations from D9.

**Worked example.** See Section 4.11.

### 1.14 Decision D10 -- Atomics: regseq-DSL `Atomic<T>` with intrinsic operations; no Ty bit

**Decision.** `Ty` does not carry any atomic annotation. The
regseq-model IR has no `atomic: bool` field at any level -- not
on `Ty`, not on a generic load/store `Statement`, not on the
`Call` terminator. There is no `Ty::Atomic<T>` variant and no
`is_atomic` flag on `Item`. This is direct application of the
Section 1.12 framing principle to the atomic-operation domain,
paralleling D9's resolution for `volatile`.

Atomic *types* in the regseq DSL are ordinary `Ty::Struct`
instances. The DSL provides its own `Atomic<T>` abstraction
(stage-14 territory for the exact shape) -- a `repr(C)` struct
containing an `UnsafeCell<T>` field -- and the bridge sees it as
a plain `Ty::Struct(<regseq::Atomic ItemId>, <args=[T]>)`. The
existing `is_unsafe_cell` flag (D1.5) on the inner field handles
`const`-dropping on enclosing references, exactly as for any
other UnsafeCell-bearing struct; no atomic-specific machinery is
needed at the type level.

Atomic *operations* are stage-14 intrinsic Calls, recognised by
Q7's existing `#[rseq::intrinsic("NAME")]` attribute mechanism.
Memory ordering (`Relaxed` / `Acquire` / `Release` / `AcqRel` /
`SeqCst`) is operation-level metadata -- it survives in the IR
either as an operand on the `Call` to a single
`atomic_<op>(_, order)` intrinsic, or via per-(op × ordering)
`InstanceId` identity (e.g., `atomic_load_acquire`,
`atomic_load_relaxed`, ...). Stage 14 owns the encoding choice;
either form keeps ordering at the operation level and out of
`Ty`.

The bridge rejection has two phases:

- *Type-translation rejection.* The bridge rejects the
  `core::sync::atomic::Atomic*` type family at the point it
  translates an atomic-typed Rust binding (e.g., `&AtomicU32`,
  `*const AtomicBool`, an `AtomicI64` field). Members
  (per `library/core/src/sync/atomic.rs` in the pinned
  toolchain): `AtomicU8`, `AtomicI8`, `AtomicU16`, `AtomicI16`,
  `AtomicU32`, `AtomicI32`, `AtomicU64`, `AtomicI64`,
  `AtomicU128`, `AtomicI128` (the 128-bit variants are gated
  on `target_has_atomic_load_store = "128"` per
  `library/core/src/sync/atomic.rs:3884-3920`; rejection scope
  is the family, not gated by the cfg), `AtomicUsize`,
  `AtomicIsize`, `AtomicBool`, `AtomicPtr<T>`, plus
  `Atomic<T>` -- the unstable generic atomic type alias
  declared with `#[unstable(feature = "generic_atomic", issue =
  "130539")]` at `library/core/src/sync/atomic.rs:340-345`.
  Rejection scope is the family, not conditional on the
  unstable feature gate.

  *Naming clarification.* `core::sync::atomic::Atomic<T>`
  (the unstable generic alias just listed) and the regseq-DSL
  `regseq::Atomic<T>` facade are distinct types living in
  distinct namespaces. The rejection target is *the core
  alias*; driver code that refers to `regseq::Atomic<T>` via
  the regseq DSL imports is the accept path.
- *Call-translation rejection.* The bridge rejects calls to
  the methods on the above types (`.load()`, `.store()`,
  `.fetch_add()`, `.fetch_sub()`, `.fetch_and()`, `.fetch_or()`,
  `.fetch_xor()`, `.swap()`, `.compare_exchange()`,
  `.compare_exchange_weak()`, ...) and to the rustc-internal
  `core::intrinsics::atomic_*` family.

Explicitly *not* in the rejection scope:
`core::sync::atomic::Ordering` (the value-level enum -- a plain
Rust enum that `regseq::Atomic<T>` may re-export or shadow with
its own ordering enum, per stage-14's choice).

*Memory barriers / fences (v1 policy: rejected at bridge).*
The `core::sync::atomic` module also contains free functions
for memory barriers and compiler fences -- notably
`core::sync::atomic::fence(Ordering)` and
`core::sync::atomic::compiler_fence(Ordering)`, plus related
items. These are *not* part of the regseq Atomic construct
surface (which is value-keyed atomic accesses; see
`docs/compiler-collateral/constructs/atomic-memory-access-and-ordering.md`
for the construct-level scoping that explicitly excludes
barrier/fence elements from the Atomic abstraction). **v1
policy: the bridge rejects `core::sync::atomic::fence` and
`core::sync::atomic::compiler_fence` calls at Call translation**,
mirroring D9's `*_volatile` rejection and D10's
`core::sync::atomic::Atomic*` method rejection. Rationale: the
methodology rule "core items outside the recognized vocabulary
are rejected" applies; leaving these calls in an unspecified
state would conflict with Q7's closed-vocabulary discipline (Q7
governs attribute-tagged callees but the same rejection rigor
applies to raw `core::*` calls outside the accepted set). The
rejection lifts when a future barrier/fence construct decision
admits an explicit DSL mechanism (e.g.,
`#[rseq::intrinsic("barrier_*")]` or a construct-level
reject/accept scope). Stage-13 owns enforcement (see
`stage-14/expectations.md`).

All driver atomic access flows through `regseq::Atomic<T>` and
its `#[rseq::intrinsic("atomic_*")]`-flagged methods.

**Reasoning.**

Stage 3 owns the question "does `Ty` carry an atomic bit?"
The answer is no, by direct application of the Section 1.12
framing principle and by parallel construction with D9. The
four sub-questions stage 3 had to disambiguate resolve as
follows:

1. *Substantive lock -- atomic types are ordinary `Ty::Struct`.*
   Atomic types in the Rust stdlib (and, by analogy, in the
   regseq DSL) are structurally `repr(C)` structs containing a
   single `UnsafeCell<T>` field. The C lowering needed (drop
   `const` on enclosing shared references) is already produced
   by the `is_unsafe_cell` flag (D1.5). Adding an `is_atomic`
   flag or a `Ty::Atomic<T>` variant would duplicate information
   and inflate the variant set with no semantic gain.

2. *Ingest policy -- regseq DSL provides `Atomic<T>`; the
   `core::sync::atomic::Atomic*` type family + their methods +
   `core::intrinsics::atomic_*` are rejected at the bridge,
   plus the v1 rejection of `core::sync::atomic::{fence,
   compiler_fence}` (`Ordering` is *not* in the rejection scope
   -- it is a plain value-level enum).* Five architectural
   strands:

   - *Architectural symmetry with D9.* D9 rejected raw
     `core::ptr::{read,write}_volatile` and required
     `#[rseq::intrinsic("mmio_*")]`. Rejecting the
     `core::sync::atomic` operational API makes both volatile
     and atomic follow the same rule.
   - *DSL surface coherence.* Using `regseq::Atomic<T>` instead
     of `core::sync::atomic::AtomicU32` is a one-import-line
     change.
   - *DSL controls vocabulary instead of inheriting it.* With
     `regseq::Atomic<T>`, the DSL pins exactly which atomic
     operations exist; rustc evolution is decoupled from DSL
     surface.
   - *Single bridge recognition mechanism.* Q7's
     `#[rseq::intrinsic("NAME")]` attribute remains the only
     admission path. No parallel path-based recognizer for
     `core::intrinsics::atomic_*` (rustc-internal, no
     `#[rseq::intrinsic]` attribute).
   - *Per-target codegen latitude is unaffected.* Linux
     (`atomic_*` family with `_relaxed`/`_acquire`/`_release`
     suffixes plus an unsuffixed fully-ordered form covering
     SeqCst, with AcqRel conservatively strengthened to
     unsuffixed since Linux has no dedicated AcqRel suffix);
     Zephyr (SeqCst-only public API; codegen must use GCC
     `__atomic_*` builtins to preserve non-SeqCst orderings).
     Stage-13 codegen for the regseq atomic-intrinsic
     InstanceIds picks per target.

3. *Memory ordering -- no stage-3 representation.* Memory
   ordering is operation-level metadata in the IR, surviving
   either as an operand on the `Call` or via per-(op × ordering)
   `InstanceId` identity. Stage 14 picks the encoding shape.

4. *Compositional behavior beats special-case flags.* Atomicity
   on the `InstanceId`; no `Ty` flag, no Statement flag.

**Methodology rules invoked.**

- "C semantics with information preservation" -- preserve
  rustc's view (no type-level atomic).
- "Compositional behavior beats special-case flags" -- the
  existing `is_unsafe_cell` flag (D1.5) handles the type-level
  part.
- "No defensive fallbacks" -- bridge rejects the
  `core::sync::atomic::Atomic*` type family and methods plus
  the rustc-internal `core::intrinsics::atomic_*` family
  (Ordering excluded).
- "Don't redo work the rustc compiler has already done" --
  rustc routes atomicity through intrinsics.
- "Re-derive from evidence; do not import baggage" -- the
  `core::sync::atomic` API is a stdlib design choice oriented
  to general Rust programs; the DSL surface is operational-
  verb-driven, and atomic operations belong in the intrinsic
  vocabulary alongside MMIO.
- "Compiler-theory-first" -- atomicity is a memory-effect
  property of an operation, not a type-level shape.

**Prior-art evidence.**

Across surveyed projects with explicit atomic-handling
evidence: atomic *types* are treated as ordinary structs (with
`UnsafeCell<T>` or equivalent) and atomic *operations* are kept
op-level (intrinsic, method, or per-op metadata).

- *LLVM* -- atomic operations carry an `atomic` modifier and a
  separate memory-ordering metadata field on load/store; type
  system carries no atomicity (`prior-art/llvm-ir.md` Theme D).
- *mrustc* -- atomic operations realised through intrinsic
  helpers; `m_atomics` lives on the target architecture struct,
  not on type (`prior-art/mrustc.md` Theme D, lines 520-536).
- *Charon* -- no type-level markers for atomic types or marker
  types (`prior-art/charon.md` Theme D, lines 609 and 664).
- *rustc_codegen_clr* -- atomicity lowered to method calls
  (`prior-art/rustc-codegen-clr.md` Theme D, lines 328-382).
- *rustc-codegen-c* -- atomic types are ordinary structs;
  atomicity preserved in emitted expressions
  (`prior-art/rustc-codegen-c.md` Theme D, lines 224-242).
- *MLIR* -- general "special semantics off type" pattern via
  type interfaces / parameters / traits
  (`prior-art/mlir.md` Theme D, lines 365-405).
- *CompCert* -- C source language has no atomics; Theme D atomic
  context (lines 436-451) contributes no signal for D10.

Rustc's own posture mirrors the surveyed group: atomic
operations are intrinsics (`core::intrinsics::atomic_*`); atomic
types are ordinary structs containing `UnsafeCell<T>`
(`findings-rustc-draft.md` Theme D).

**Linux/Zephyr C-side evidence.** Both v1 codegen targets reject
C11 `_Atomic` by convention; both keep atomicity at the
operation/function level. Linux: `atomic_t`/`atomic64_t`/
`atomic_long_t`/`refcount_t`; ordering surfaces via suffix
(`_relaxed`/`_acquire`/`_release`/unsuffixed). Zephyr:
`atomic_t` (long, word-sized); SeqCst-only public API; non-SeqCst
preserved via GCC `__atomic_*` builtins. Hard external
corroboration of the operation-level posture.

**Worked example.** See Section 4.12.

### 1.15 Decision D16 -- `c_void` recognition: rejected at bridge in v1

**Decision.** The bridge **rejects** `core::ffi::c_void` at
type translation in v1. No `is_c_void` flag on `Item`; no
special `Ty` variant; no acceptance as an ordinary `Ty::Enum`.
Recognition is via `tcx.is_lang_item(did, LangItem::CVoid)`
(per D1.5's locked recognition-path note); the bridge runs this
query at `Ty::Adt` translation and rejects with a clear
diagnostic if true.

The pinned-toolchain `core::ffi::c_void` is declared with two
hidden `__variant1` / `__variant2` variants
(`library/core/src/ffi/mod.rs:49-76`, `#[unstable(feature =
"c_void_variant", reason = "temporary implementation detail",
issue = "none")]` and `#[doc(hidden)]`) specifically so that
c_void is *inhabited* (not zero-variant uninhabited). Without
recognition, the bridge would translate c_void as an ordinary
2-variant `Ty::Enum` with `repr(u8)` -- structurally
indistinguishable from any user-defined `repr(u8) enum Foo { X,
Y }`. The C lowering of such a translation would emit a 1-byte
struct holding the tag, *not* the FFI-canonical `void *` shape
that c_void's lang-item identity entails. The rejection is
motivated by this missing FFI-canonical-lowering decision
combined with D9 / D10 / D1.4 rejection-pattern symmetry.

The rejection scope is the `c_void` type itself, which
transitively covers pointers to it (`*mut c_void`,
`*const c_void`) -- the bridge rejects when it encounters the
inner pointee type during pointer translation. The diagnostic
recommends the opaque-struct pattern (`#[repr(C)] struct Opaque
{ _private: [u8; 0] }` and `*mut Opaque`) as the canonical
alternative for opaque C-side handles, mirroring D1.4's existing
guidance for `Foreign(_)` rejection.

This is the formal close of the deferral D1.5 / D1.6.c opened:
D1.5 locked "no flag for v1; recognition deferred"; D1.6.c
described `c_void` as the `CVoid` lang-item ADT and forwarded
the substantive lock to D16. (D1.6.c's description has been
updated post-M-2 to match the pinned-toolchain shape -- the
2-variant `repr(u8)` enum with hidden `__variant1` /
`__variant2` variants. The lang-item identity is unchanged; the
rejection rests on lang-item identity plus FFI-lowering policy
regardless of the variant shape.)

**Trigger conditions for re-opening D16 (deferred to post-v1).**

D16 reopens when *either* of the following materialises:

- *A driver fixture surfaces a stable need for FFI-canonical
  `void *` lowering* -- e.g., a Linux/Zephyr driver passing an
  opaque user-data pointer through a kernel callback that
  insists on `void *` typing.
- *A stage-14 intrinsic vocabulary entry's signature uses
  `*mut c_void`* -- e.g., a hypothetical
  `regseq_runtime_alloc(size: usize) -> *mut c_void` style
  intrinsic.

When either materialises, D16 reopens to:
- Lift the bridge rejection.
- Add an `is_c_void` flag on `Item` (analogous to D1.5's flags),
  driven by `tcx.is_lang_item(LangItem::CVoid)`.
- Define the C lowering, distinguishing the two pointer
  mutabilities per locked D7/D8's `Ty::Ptr(TyId, Mutability)`
  shape and Section 1.12's raw-pointer `const`-derivation rule:
  `*mut c_void` -> C `void *`; `*const c_void` -> C `const
  void *`. The exact C-spelling for the const-qualified case is
  part of the reopened D16's decision.

Until that future reopen, the v1 rejection holds.

**Reasoning.**

Three architectural strands carry the decision:

1. *No v1 consumer.* The evidence sweep confirms zero v1 driver
   fixtures use `core::ffi::c_void`; zero planned stage-14
   intrinsic signatures mention it. Per the methodology rule
   "ask who consumes it; what shape best serves them," no v1
   consumer to serve.

2. *Symmetric posture with D9 / D10 / D1.4.* All three
   already-locked rejections share the same structure: a stable
   `core::*` API or feature is rejected at the bridge because
   it lies outside the regseq DSL's operational surface, with a
   diagnostic pointing to the canonical alternative. D16 fits
   this pattern; the diagnostic recommends the opaque-struct
   pattern (the same alternative D1.4 names for opaque C-side
   handles -- there is no D9/D10-style DSL-defined replacement
   because c_void's role is structural, not operational).

3. *The non-rejection alternative does not earn its weight.*
   Translating `c_void` as an ordinary `Ty::Enum` (per the
   pinned toolchain's actual structure: a 2-variant `repr(u8)`
   enum with `EnumShape::Multiple { tag: u8, encoding: Direct,
   variants: [...] }`) admits a type whose stage-14 C lowering
   is structurally indistinguishable from any user-defined
   `repr(u8) enum Foo { X, Y }`: a 1-byte tagged struct. That
   is *not* the FFI-canonical `void *` shape that c_void's
   lang-item identity implies. Stage-13 codegen would either
   need to commit to the FFI-wrong 1-byte struct lowering or
   perform lang-item recognition that the bridge is not
   currently doing. Rejecting in v1 avoids committing to either
   path before D16 can reopen with proper recognition plus an
   FFI-canonical pointer-to-c_void lowering decision.

The decision hardens D1.5's "recognition deferred" into
"actively rejected pending recognition" -- the only honest
interpretation given that an unrecognised `c_void` cannot be
lowered correctly without a recognition-driven C-lowering
decision.

**Methodology rules invoked.**

- "No defensive fallbacks" -- don't speculate-handle a type
  with no v1 consumer that we cannot lower correctly.
- "Ask who consumes it; what shape best serves them" -- no v1
  consumer.
- "Re-derive from evidence; do not import baggage" -- the
  shared D16 lean addressed only the no-flag question; the full
  lock follows from rejection-pattern symmetry plus
  no-consumer evidence.
- "Compositional behavior beats special-case flags" -- the
  rejection mechanism is the same `tcx.is_lang_item(did,
  LangItem::*)`-driven path D1.5 already establishes.
- "DSL surface coherence" -- operational and FFI-typed `core::*`
  constructs are rejected at the bridge.

**Prior-art evidence.**

Surveyed prior-art projects do not directly address c_void
recognition because their target languages handle FFI
differently:

- *rustc itself* declares `c_void` as a lang-item enum at
  `rustc_hir/src/lang_items.rs:279` (Target::Enum,
  GenericRequirement::None; the Target::Enum tag means "is an
  enum," not "is zero-variant" -- the actual c_void declaration
  in `library/core/src/ffi/mod.rs:49-76` carries two hidden
  variants), accessed via `tcx.is_lang_item(did,
  LangItem::CVoid)`. Not in `AdtFlags` -- per
  `findings-rustc-draft.md` Theme D.2 (lines 546-564) and D1.5's
  recognition-path note (Section 1.6 lines 402-406).
- *Charon* (inference, not direct evidence). Charon catalogues
  marker types treated as ordinary ADTs without type-level
  markers (`prior-art/charon.md` Theme D, lines 609 and
  664-678); c_void is not specifically named, so the inference
  is extrapolation, not citation. Charon's verifier-IR posture
  admits ordinary ADTs because Charon operates on abstract type
  structure rather than emitting C; our codegen-target posture
  has stricter emission requirements.
- *mrustc, rustc_codegen_clr, rustc-codegen-c, llvm-ir,
  compcert* do not specifically address c_void recognition.

The closest in-spec precedent is D1.4's rejection of
`Foreign(_)`: same rejection mechanism; same recommended
alternative (opaque-struct pattern); difference is that c_void
is a stable `core` API rather than an unstable Rust feature,
but the v1 absence of a consumer makes the distinction
immaterial.

**Worked example.** See Section 4.15.

---

## 2. Layout

### 2.1 Decision D2 -- Layout position

**Decision.** Each `Ty::Struct`, `Ty::Enum`, `Ty::Union`, and
`Ty::Tuple` carries a `Layout` field inline. The `Layout` is the
*resolved result* of `tcx.layout_of(ty)`, copied by the bridge at
type construction time. Layout is **not** computed by the bridge
or codegen; rustc owns the layout algorithm; we own preserving
the answer. (Tuples reuse the `LayoutKind::Struct` shape with
synthetic field names `_0`, `_1`, ... -- see D11 below; tuples
have no `Item` to attach the layout to, so the layout lives
inline on `Ty::Tuple` itself rather than on a parallel
layout-bearing entry.)

**Reasoning.**

Rustc has already done the layout work: field offsets, padding,
niche optimization, tag encoding, alignment. Per the methodology
rule "C semantics with information preservation," storing the
result is the right move. Recomputing in the bridge would
duplicate work and risk divergence; deferring to codegen would
require codegen to query the bridge's stored types (back-pressure),
or recompute from scratch (no rustc available downstream of bridge).

The "explicit layout in IR" posture is also the right fit for our
"one IR, two printers" case (Linux + Zephyr both C, sharing layout
rules). rustc_codegen_clr is the direct precedent: its `ClassDef`
carries explicit field offsets and the dual CIL/C printers consume
that layout uniformly.

The cost of explicit-layout-in-IR (binding to one ABI's layout)
is what we want anyway -- compiler instances are per-target.

**Methodology rules invoked.**
- "C semantics with information preservation" -- store the
  resolved result, not the input.
- "No defensive fallbacks" -- don't recompute what rustc has
  already computed.

**Prior-art evidence.**
- rustc_codegen_clr commits explicit layout in `ClassDef`
  (`prior-art/rustc-codegen-clr.md` Theme B). Confirmed transferable
  to our Linux/Zephyr split.
- Most other projects keep layout off the type (LLVM/MLIR/CompCert/
  mrustc/Charon) -- valid for their use cases (multi-target IR,
  verifier focus), not ours (single-instance per-target codegen).

**Follow-on (now locked).** D11 fixes the exact subset of
`LayoutData` fields mirrored. See Section 2.2.

### 2.2 Decision D11 -- Layout content subset

**Decision.** The `Layout` carried inline on `Ty::Struct/Enum/Union`
mirrors the *codegen-load-bearing* subset of rustc's `LayoutData`.
Concretely:

```rust
pub struct Layout {
    pub size:        u64,
    pub align:       u64,
    pub pack:        Option<u32>,   // None = not packed; Some(N) = repr(packed(N))
    pub uninhabited: bool,          // Result<T, !>, enum E {}, struct S(!), etc.
    pub kind:        LayoutKind,
}

pub enum LayoutKind {
    Struct { fields: Vec<FieldLayout> },   // for Ty::Struct AND Ty::Tuple (tuples use synthetic field names "_0", "_1", ...)
    Union  { fields: Vec<FieldLayout> },   // for Ty::Union (offsets all 0)
    Enum   { shape:  EnumShape       },    // for Ty::Enum
}

pub struct FieldLayout {
    pub name:   StringId,    // interned via RegseqModel.strings
    pub ty:     TyId,
    pub offset: u64,         // bytes from the parent type's start
}

pub enum EnumShape {
    Empty,                                 // zero-variant uninhabited enum
    Single { variant: VariantLayout },     // single-variant enum
    Multiple {
        tag:      TagScalar,
        encoding: TagEncoding,
        variants: Vec<VariantLayout>,
    },
}

pub struct VariantLayout {
    pub name:         StringId,
    pub fields:       Vec<FieldLayout>,   // offsets relative to the parent's start
    pub uninhabited:  bool,               // enables Result<T, !> Err-arm elision
    pub discriminant: u128,               // D13: Rust-level semantic discriminant
                                          //   (what `Discriminant(place)` returns;
                                          //   from adt_def.discriminants(tcx)).
                                          //   Always present for any VariantLayout
                                          //   in any EnumShape (Single or Multiple);
                                          //   EnumShape::Empty carries no
                                          //   VariantLayout, so the field does not
                                          //   apply there.
    pub tag_value:    Option<u128>,       // D13: layout-level tag bytes
                                          //   (what `SetDiscriminant` writes).
                                          //   - Some(v) for Multiple direct-encoded
                                          //     variants (v == discriminant).
                                          //   - Some(v) for Multiple niche-encoded
                                          //     niched variants (v computed per
                                          //     D12's formula).
                                          //   - None for Multiple niche-encoded
                                          //     untagged variants (the inner data's
                                          //     bit pattern IS the tag).
                                          //   - None for Single variants (no tag
                                          //     scalar exists in the layout).
}

pub struct TagScalar {
    pub offset:      u64,
    pub scalar_ty:   TyId,            // resolved must be Int/Uint/Ptr/Ref
    pub valid_range: WrappingRange,   // always present; see invariant below
}
// Bridge invariant: a `TagScalar` always carries a `valid_range`
// because the bridge populates it from rustc's `Scalar::Initialized
// { value, valid_range }` (`findings-rustc-draft.md` Theme B.6,
// citing `rustc_abi/src/lib.rs:1511-1528`). Rustc's other `Scalar`
// variant `Scalar::Union` has no `valid_range`
// (`rustc_abi/src/lib.rs:1520-1527`). If the bridge encounters
// `Scalar::Union` at the tag position of a `Variants::Multiple`,
// it rejects the layout shape (per "no defensive fallbacks")
// rather than fabricating a `valid_range`. This is a bridge-side
// rejection, not a claim about rustc's emission behaviour.

pub struct WrappingRange {
    pub start: u128,
    pub end:   u128,                  // inclusive; wraps if start > end
}

pub enum TagEncoding {
    Direct,
    Niche {
        untagged_variant: VariantIdx,
        niche_variants:   VariantRange,   // inclusive [start, end]
        niche_start:      u128,
    },
}

newtype_index! { pub struct VariantIdx {} }     // u32 newtype, dense per Vec<VariantLayout>

pub struct VariantRange {
    pub start: VariantIdx,
    pub end:   VariantIdx,    // inclusive
}
```

`pack` is the single field whose information cannot be reconstructed
from the resolved bytes. Every other repr-driven effect (alignment
boost, transparent newtype, repr-int tag width, struct-field
reordering) is observable from `{size, align, offsets, tag.scalar_ty}`
and need not be carried.

**Mapping from rustc's `LayoutData` to our `Layout`.**

| `LayoutData` field | Our `Layout` | Disposition |
| --- | --- | --- |
| `size: Size` | `size: u64` | keep |
| `align: AbiAlign` | `align: u64` | keep (abi-align scalar; we don't need pref-align) |
| `fields: FieldsShape` | flattened into `LayoutKind::{Struct,Union}.fields` and `EnumShape::*.variants[i].fields` | collapse |
| `variants: Variants` | `LayoutKind::Enum { shape: EnumShape }` | collapse |
| `(in `Variants::Multiple`) tag: Scalar` | `EnumShape::Multiple.tag: TagScalar` | keep (offset + scalar type + valid range) |
| `tag_encoding: TagEncoding` | `EnumShape::Multiple.encoding: TagEncoding` | keep, full fidelity (incl. `niche_variants`, `niche_start`) |
| `tag_field: FieldIdx` | folded into `tag.offset` | collapse to direct offset |
| `uninhabited: bool` | `Layout.uninhabited` + per-variant `VariantLayout.uninhabited` | keep both levels |
| `largest_niche: Option<Niche>` | -- | **drop** (parent-only consumer; already baked into the parent's `tag_encoding`) |
| `backend_repr: BackendRepr` | -- | **drop** (Rust calling-convention hint, irrelevant for C signatures) |
| `max_repr_align: Option<Align>` | -- | **drop** (layout-algorithm intermediate) |
| `unadjusted_abi_align: Align` | -- | **drop** (layout-algorithm intermediate) |
| `randomization_seed: Hash64` | -- | **drop** (nightly perf knob) |
| (from `ReprOptions`) `pack: Option<Align>` | `Layout.pack: Option<u32>` | **keep** (only repr escape) |
| (from `ReprOptions`) `align: Option<Align>` | -- | drop (`Layout.align` already reflects the forced value) |
| (from `ReprOptions`) `int: Option<IntegerType>` | -- | drop (visible via `tag.scalar_ty`) |
| (from `ReprOptions`) `IS_C` / `IS_TRANSPARENT` flags | -- | drop -- not needed; the resolved `(size, align, offsets)` are authoritative for C emission. Codegen emits fields in memory order with explicit padding; the C compiler's natural layout matches the resolved bytes (verified by `_Static_assert`). The flags themselves carry no information codegen needs once the bytes are known. |
| (from `ReprOptions`) `IS_SIMD` / `IS_LINEAR` / `IS_SCALABLE` | -- | drop (DSL out of scope; reject at bridge) |
| (from `ReprOptions`) `RANDOMIZE_LAYOUT` / `PASS_INDIRECTLY_*` / `field_shuffle_seed` | -- | drop (perf hints / Rust ABI) |

**Reasoning.**

The choice of subset answers a single question per field: *does
codegen need this to emit correct C?* We keep size/align (typedef +
`_Static_assert` verification), per-field offsets (placement +
padding emission), tag-scalar info (discriminant byte read/write),
tag-encoding fidelity including `niche_variants`/`niche_start`
(byte-exact niched-enum encoding), per-Layout and per-Variant
`uninhabited` flags (branch elision), and `pack` (the one repr knob
that the C compiler cannot reproduce from bytes alone -- it
re-introduces alignment padding unless `__attribute__((packed))`
suppresses it).

We diverge from Charon by retaining full niche fidelity (Charon's
`DiscriminantLayout` carries `untagged_variant` only). Charon is a
verifier-IR producer where abstract reasoning suffices; we emit
byte-exact C, so the formula `tag = (variant_idx -
niche_variants.start).wrapping_add(niche_start)` is load-bearing for
`SetDiscriminant` codegen on any niche-encoded enum with more than
one niched variant.

We also depart from rustc's recursive `Variants::Multiple.variants:
IndexVec<VariantIdx, LayoutData>` (full nested LayoutData per
variant). Each variant's size/align is derivable from
`max(field.offset + sizeof(field.ty))` and the parent's align; the
nested `LayoutData` adds storage and serialization cost without
codegen benefit. Our `VariantLayout` keeps the field offsets and the
per-variant `uninhabited` flag and elides the rest.

`largest_niche` is dropped on the methodology rule "ask who
consumes it." Rustc carries `largest_niche: Option<Niche>` on every
`LayoutData` (`findings-rustc-draft.md` Theme B.1, citing
`rustc_abi/src/lib.rs:2010-2068`); the rustc evidence we have
enumerates the field's presence but does not document its consumer.
**v1 consumer analysis**: no analysis or codegen pass in the
regseq-driver / regseq-analysis / regseq-backends crates currently
reads a child type's `largest_niche` -- enum codegen reads the
*parent's* `tag_encoding: Niche {...}` (a resolved decision rustc
has already made and we mirror byte-exactly), and
`SetDiscriminant` / `Discriminant` lowerings need only the
parent's tag-encoding fields (Section 4.4 worked example).
Charon similarly omits `largest_niche` from its decl-side `Layout`
(`prior-art/charon.md` Theme B at lines 418-435: Charon's layout
is described as "simplified" and "does not include information
about niches"). This is independent corroboration that an
analysis/verifier IR can omit the field; it does not directly
support our byte-exact-codegen claim, which rests on the v1
consumer analysis above and on the parent-side `tag_encoding`
being byte-exactly preserved (D11 + D12). If a future consumer
surfaces (e.g., an analysis pass that proposes alternative
layouts, or a debugging pretty-printer), the bridge can re-query
`tcx.layout_of` -- the field is not preserved in v1.

`pack: Option<u32>` mirrors rustc's `pack: Option<Align>` shape:
`Some(1)` is plain `repr(packed)`; `Some(N>1)` is `repr(packed(N))`.
v1 codegen handles `Some(1)` and rejects `Some(N>1)` with a clear
diagnostic per the "no defensive fallbacks" rule; the IR shape
admits the full set, so future expansion does not require a re-spec.

**Methodology rules invoked.**

- "Don't redo work the rustc compiler has already done" -- every
  field we keep is a *result* of rustc's layout algorithm, not an
  input. `pack` is the same: it is the resolved post-layout flag,
  not the source-side `#[repr(packed)]` attribute.
- "C semantics with information preservation" -- store byte-exact
  results (offsets, niche encoding, tag scalar) so codegen emits
  matching C without re-deriving anything.
- "No defensive fallbacks" -- v1 only handles `pack: None` and
  `pack: Some(1)` natively; other values reject with diagnostic.
  The IR shape is full-fidelity so the consumer narrows, not the
  spec.
- "Compositional behavior beats special-case flags" -- one `pack`
  field; everything else (`align(N)`, `transparent`, `repr(int)`,
  reordering) falls out of size/align/offsets/tag-scalar.
- "Ask who consumes it" -- `largest_niche` has no v1 consumer;
  `BackendRepr` is irrelevant (we emit C function signatures and
  the C compiler picks calling convention).

**Prior-art evidence.**

- *rustc's `LayoutData` and components* -- `findings-rustc-draft.md`
  Theme B (B.1-B.10): `LayoutData` (B.1, lines 240-254), `FieldsShape`
  (B.2), `Variants` (B.3), `TagEncoding` (B.4), `Niche` (B.5),
  `Scalar` (B.6), `BackendRepr` (B.7), `Integer`/`Float` (B.8),
  `ReprOptions`/`ReprFlags` (B.9), `PointerKind`/`PointeeInfo` (B.10).
- *Convergent universal stores* -- `size`, `align`, per-field
  offsets stored by every surveyed Rust-frontend project that does
  aggregate codegen itself: rustc_codegen_clr's `ClassDef` carries
  size + per-field `(type, name, byte_offset)` (`prior-art/rustc-
  codegen-clr.md` Theme B); mrustc's cached `TypeRepr` carries
  size + align + `Field{offset, ty}` (`prior-art/mrustc.md` Theme
  B); Charon's `Layout` carries size/align (Option due to `?Sized`)
  + per-variant `field_offsets` (`prior-art/charon.md` Theme B).
- *Universal drops* -- `BackendRepr`, `randomization_seed`,
  `max_repr_align`, `unadjusted_abi_align` are stored in *no*
  surveyed project (rustc_codegen_clr, mrustc, CompCert, LLVM,
  Charon, rustc_codegen_c).
- *Tag-encoding fidelity divergence* -- rustc carries the full
  triple (`untagged_variant`, `niche_variants`, `niche_start`);
  Charon stores only `untagged_variant` (`prior-art/charon.md`
  Theme B); mrustc encodes via coarser `VariantMode::{Linear,
  Values, NonZero}` (`prior-art/mrustc.md` Theme B); CLR collapses
  to a synthetic ENUM_TAG field with offset collisions
  (`prior-art/rustc-codegen-clr.md` Theme B). We retain rustc's
  full triple because byte-exact C codegen requires it for
  `SetDiscriminant` on multi-niched enums.
- *`pack` precedent* -- mrustc's `HIR::Struct` carries `m_repr` flag
  set with explicit packed mode (`prior-art/mrustc.md` Theme B);
  Charon's `ReprOptions` similarly carries `align_modif` with
  pack/align variants (`prior-art/charon.md` Theme B). CLR/LLVM/
  CompCert/rustc_codegen_c do not carry repr separately.
- *Per-target multiplicity rejected* -- only Charon stores layout
  per-target via `SeqHashMap<TargetTriple, Layout>` (`prior-art/
  charon.md` Theme B). A regseq compiler instance is per-target;
  layout reflects that target. Single-target lock matches all
  other surveyed projects.

**Worked examples.**

- *Niche-encoded enum* -- Section 4.4 (`Option<&u32>`) walks through
  the `TagEncoding::Niche {untagged_variant: Some, niche_variants:
  None..=None, niche_start: 0}` triple in action.
- *Packed struct* -- Section 4.8 (`#[repr(C, packed)]` MMIO header)
  shows `pack: Some(1)` driving `__attribute__((packed))` emission
  with `_Static_assert` verification.

### 2.3 Decision D13 -- Per-variant discriminant values

**Decision.** D13 disambiguates two distinct senses of
"discriminant" and locks each:

- *Discriminant **type*** -- the `repr(<int>)` choice (or
  rustc's default tag-size selection). **Already settled by
  D11**: visible via `TagScalar.scalar_ty: TyId` (resolved to
  `Ty::Int(_)` / `Ty::Uint(_)` / `Ty::Ptr(_, _)` / `Ty::Ref(_,
  _)` per locked D7/D8 plus D11's mapping table). D11's table
  explicitly drops `ReprOptions.int` because the resolved
  scalar type is authoritative. D13 carries forward this lock
  without modification.
- *Per-variant discriminant **values*** -- two separate
  layout-level facts:
  - The Rust-level *semantic discriminant* -- what
    `Discriminant(place)` returns. For `repr(<int>)` enums with
    user-assigned values like `Idle = 0`, `Active = 1`, `Error
    = 0xFF`, `Reset = 0xDEAD`, this is the user-assigned value.
    For non-`repr(<int>)` enums, this is the rustc-resolved
    discriminant value. Sourced from `adt_def.discriminants(tcx)`
    (which returns a `Discr { val: u128, ty: Ty }` per
    `rustc_middle/src/ty/util.rs:36-40` -- one entry per
    variant, never absent).
  - The layout-level *tag bytes* -- the bit pattern that
    `SetDiscriminant` writes to the tag location. For
    direct-encoded variants this equals the semantic
    discriminant; for niche-encoded niched variants it is
    `(variant_idx - niche_variants.start).wrapping_add(niche_start)`
    (resolved per the D12 formula); for niche-encoded untagged
    variants there is no tag write (the inner data's bit
    pattern *is* the tag); for `EnumShape::Single` variants
    there is no tag scalar at all.

**Settled here**: `VariantLayout` carries two new fields,
extending D11's locked Section 2.2 schema -- `discriminant:
u128` (always present per `adt_def.discriminants(tcx)`) and
`tag_value: Option<u128>` (the tag bytes `SetDiscriminant`
writes; `None` when no tag write occurs). The merged D11 schema
in Section 2.2 above shows the two new fields with inline
comments enumerating their values per `EnumShape` case.

`u128` width matches rustc's resolved `Discr.val: u128`; the
bit pattern is unsigned at storage, with signed interpretation
(for `repr(i*)` enums) supplied by `TagScalar.scalar_ty: TyId`
(D11) at codegen time.

The cited rustc sources: `rustc_middle/src/ty/mod.rs:1116-1128`
declares `VariantDef.discr: VariantDiscr`;
`rustc_middle/src/ty/mod.rs:1313-1323` defines the
`VariantDiscr` enum (`Explicit(DefId)` / `Relative(u32)` -- the
source-level form, before evaluation);
`rustc_middle/src/ty/util.rs:36-40` declares the resolved
`Discr { val: u128, ty: Ty }` type that
`adt_def.discriminants(tcx)` returns. The bridge resolves the
source-level `VariantDiscr` to the post-eval `Discr` via the
`AdtDef::discriminants(self, tcx)` iterator
(`compiler/rustc_middle/src/ty/adt.rs:583-600`) at
type-construction time and copies the `val: u128` into the
`discriminant` field.

**Reasoning.**

For `Direct` tag encoding with user-assigned discriminants
(common in driver code -- status codes, error codes, register
values), variant index and discriminant value are
*independent*. Codegen for `SetDiscriminant(Status::Error)` on
`repr(u32) enum Status { Idle = 0, Active = 1, Error = 0xFF,
Reset = 0xDEAD }` must write `0xFF` to the tag location, not
`2` (the variant index). With D11's pre-D13 `VariantLayout` (no
`discriminant` field), codegen had no source of `0xFF` -- the
value lived only in rustc's typeck-level
`adt_def.discriminants(tcx)`, post-bridge unavailable. **Without
D13, driver code that uses `repr(<int>)` enums with arbitrary
user-assigned tag values cannot lower correctly.**

This is not a hypothetical pattern. Driver code routinely uses
`repr(<int>)` enums for status codes, error codes, and register
values where the byte value carries domain semantics (UART
status bit patterns, interrupt vector numbers, power-management
state-machine encodings). The pattern is foundational to MMIO
register interface design.

The substantive options for *where* to put per-variant
discriminants:

1. *On `VariantLayout`* (this lock; two fields). Cohesive:
   discriminant data is intrinsic to a variant's layout
   alongside its fields' offsets and the `uninhabited` flag.
2. *On `EnumShape::Multiple` as parallel `Vec<u128>` /
   `Vec<Option<u128>>`*. Same information, less cohesion.
3. *On the enum's `Item` decl* (stage-12 territory). Crosses
   the stage-3 / stage-12 boundary that D11 deliberately
   respects. Stage-12 owns *decl-level* enum representation;
   stage-3 owns *layout-level* tag bytes.
4. *Recover from variant index + tag encoding.* Fails for
   user-assigned `repr(<int>)`.

Option 1 is the cleanest fit:

- *"Store the resolved result, not the input."* Per-variant
  discriminants are a resolved-by-rustc decision; rustc has
  already evaluated user-source `= EXPR` clauses, applied
  auto-incrementing where omitted, and produced the final
  values.
- *"C semantics with information preservation."* C codegen for
  `SetDiscriminant(...)` needs the tag bytes; C codegen for
  `Discriminant(...)` needs the Rust-level semantic
  discriminant. Both must be available post-bridge.
- *"Don't redo work the rustc compiler has already done."*
  Discriminant evaluation is rustc's responsibility; we copy
  the answers.

**Why the two-field split.** Combining the two senses into a
single `Option<u128>` would conflate `Discriminant(place)`
semantics with `SetDiscriminant` codegen needs:

- For a `repr(u32)` direct-encoded enum, the two coincide
  (a single field would suffice).
- For a niche-encoded enum like `Option<&u32>`, the two senses
  *diverge*: `Discriminant(place)` returns rustc's resolved
  per-variant semantic discriminant (typically the variant
  index); `SetDiscriminant(None)` writes the niche bit pattern
  (`0`) to the inner pointer field; `SetDiscriminant(Some(p))`
  writes `p` itself (no separate tag write). A single-field
  design cannot honestly represent both.

The split into `discriminant: u128` (always present;
`Discriminant(place)`) and `tag_value: Option<u128>` (present
when a tag write occurs; `SetDiscriminant`) tracks rustc's own
data model: rustc keeps `adt_def.discriminants(tcx)` returning
a `Discr` for *every* variant
(`rustc_middle/src/ty/adt.rs:583-600`) while the layout-level
tag write is computed separately per `tag_encoding`
(`rustc_abi/src/lib.rs:1890-1897`). Charon's prior-art
similarly splits: typeck-level `Variant.discriminant` plus
layout-level `VariantLayout.tag: Option<ScalarValue>`.

**Coverage across `EnumShape` variants.**

- *Multiple, direct-encoded*: every variant has `discriminant
  = v` (the resolved `Discr.val: u128`) and `tag_value =
  Some(v)` (equal to the semantic discriminant -- direct
  encoding writes the discriminant verbatim).
- *Multiple, niche-encoded niched variant*: `discriminant =
  rustc_resolved_value` and `tag_value = Some(niche_bit_pattern)`
  (per the D12 formula).
- *Multiple, niche-encoded untagged variant*: `discriminant =
  rustc_resolved_value` (rustc gives every variant a
  discriminant, including the untagged one) and `tag_value =
  None` (no separate tag write).
- *Single*: `discriminant = v` (the sole variant's semantic
  discriminant) and `tag_value = None` (no tag scalar exists).
- *Empty*: no `VariantLayout` exists at all; the D13 fields do
  not apply.

Modest redundancy with D12 for niche-encoded enums:
`niche_variants` / `niche_start` (D12) tell codegen *where* to
write the tag (into the niched field) and *how* the variant
index maps to the bit pattern via the formula; `tag_value =
Some(v)` (D13) carries the resolved bit pattern, baked in for
direct lookup; `discriminant: u128` (D13) carries the Rust-
level semantic discriminant. Each is load-bearing for a
different `SetDiscriminant`/`Discriminant` codegen aspect.

**Methodology rules invoked.**

- "C semantics with information preservation" -- C codegen for
  `SetDiscriminant` and `Discriminant` consumes resolved tag
  bytes; we store the bytes.
- "Don't redo work the rustc compiler has already done" --
  rustc's `adt_def.discriminants(tcx)` is the authoritative
  source.
- "Compositional behavior beats special-case flags" -- two
  fields cover all `EnumShape` cases without
  encoding-mode-specific branches.
- "No defensive fallbacks" -- `discriminant: u128` admits
  exactly the resolved value; `tag_value: Option<u128>` admits
  exactly the two valid states.
- "Re-derive from evidence; do not import baggage" -- the
  pre-D13 lean ("recoverable from layout's tag scalar type")
  was an over-generalisation that didn't hold up against the
  user-assigned `repr(<int>)` case.

**Prior-art evidence.**

- *Charon* -- carries per-variant resolved tag bytes
  explicitly: `VariantLayout.tag: Option<ScalarValue>` with
  `Option` used for the niche-untagged case (`prior-art/charon.md`
  Theme B lines 397-512, specifically the `tag` field at lines
  413-415); also typeck-level `Variant.discriminant` at line
  504. Direct precedent for D13's Option 1.
- *mrustc* -- carries the discriminant *scheme*
  (`VariantMode::{Linear, Values, NonZero}`) on the layout
  (`prior-art/mrustc.md` Theme B lines 295-319), plus typeck-level
  `HIR::Enum` carrying `m_tag_repr`. Per-variant values are
  computed from the scheme + variant index plus rustc-equivalent
  typeck data.
- *rustc_codegen_clr* -- no per-variant discriminant carried on
  the layout; uses CIL's offset-collision-based variant
  encoding (`prior-art/rustc-codegen-clr.md` Theme B lines
  202-212).
- *rustc-codegen-c, llvm-ir, compcert* -- no per-variant
  discriminant storage observed (LLVM has no first-class enum;
  CompCert's source language has no Rust-style sum types;
  rustc-codegen-c's prior-art coverage is incomplete for this
  layout-level detail).

Rustc itself stores per-variant discriminant values at the
typeck level (`VariantDef.discr: VariantDiscr` at
`rustc_middle/src/ty/adt.rs:159`; accessed via
`adt_def.discriminants(tcx)`). Layout-level
`Variants::Multiple` (`rustc_abi/src/lib.rs:1858-1880`) does
*not* duplicate them on each variant's `LayoutData`; the
per-variant tag value is queried separately from typeck. Our
IR is post-bridge and has no live typeck query available, so
we materialise the per-variant value at bridge time.

**Worked examples.** See Section 4.13 (`repr(u32) enum Status`
with user-assigned tags, Direct encoding) and Section 4.14
(`Option<&u32>` niche-encoded with the untagged-variant `None`
case).

---

## 3. Type identity and interning

Types are content-addressed by a dense u32 newtype `TyId`. The
identity machinery is structurally interned and lives inside
`RegseqModel`, mirroring the stage-1 `StringInterner` pattern.

### 3.1 Decision D3 -- Interning strategy: structural BiMap

**Decision.** `Ty` values are interned structurally via a BiMap:
`storage: IndexVec<TyId, Ty>` (the canonical reverse map) plus
`lookup: HashMap<Ty, TyId>` (the forward dedup map). Interning is
content-unique by construction: two structurally-equal `Ty` values
hash and compare equal, so they collide in `lookup` and resolve to
the same `TyId`.

`TyId` is a u32-backed newtype with niche-optimised `Option<TyId>`
of 4 bytes (stage-1 lock at `stage-1/spec-snapshot.md:117-128`; the
*mechanism* -- `NonZeroU32` shifted, `NonMaxU32`, or rustc-style
top-256 reservation -- is left to the implementation). Stage 3
chooses `NonMaxU32` for ergonomic 0-based indexing while preserving
the niche.

```rust
pub struct TyInterner {
    storage: IndexVec<TyId, Ty>,
    lookup:  HashMap<Ty, TyId>,
}
```

`Ty` impls `Hash + Eq + Clone`. **Identity rule -- nominal for
ADT-shaped variants, structural for the rest:**

- **Nominal identity (ADT-shaped variants)**: `Ty::Struct`,
  `Ty::Enum`, `Ty::Union`, `Ty::FnDef`, `Ty::Closure`. Identity is
  determined by `(variant_kind_tag, item: ItemId, args:
  GenericArgs)` -- the discriminating variant tag (Struct vs Enum
  vs Union vs FnDef vs Closure) is part of identity, so
  `Ty::Closure(item, args)` and `Ty::Struct { item, args, layout }`
  for the same closure share `(item, args)` but remain *distinct*
  entries in the BiMap (D6.3 relies on this so the closure-Item
  and the captures-struct-Item views can both be interned). The
  inline `Layout` (or the closure-vs-struct view distinction) is
  *excluded* from `Hash + Eq`. This mirrors rustc's own `AdtDef`
  interning, which
  keys equality and hashing on `did: DefId` only and uses a
  debug-only deep-equality assertion to sanity-check that two
  `AdtDefData` for the same `DefId` agree on every other field
  (`findings-rustc-draft.md` Theme C.2, citing
  `rustc_middle/src/ty/adt.rs:111-147`). Layout is *functionally
  determined* by `(item, args)` post-monomorphization (rustc's
  layout query is deterministic per `(ty, typing_env)`), so
  excluding it from identity is sound; we cache it on the Ty as
  resolved metadata, not as a discriminator. **Bridge-side
  metadata-equality discipline**: when `intern` hits an existing
  entry under the nominal-identity hash, the bridge does *not*
  recompute the layout (the rustc layout query is functionally
  determined and would produce the same answer). In `cfg!(debug_assertions)`,
  the bridge may run a deep-equality assertion -- mirroring
  rustc's own `AdtDefData::eq` debug check -- by re-querying
  `tcx.layout_of` and comparing to the cached `Layout` on hit;
  release builds skip the assertion for performance.

- **Structural identity (most non-ADT variants)**: `Bool`,
  `Char`, `Int`, `Uint`, `Float`, `Str`, `Array`, `Slice`,
  `Ptr`, `Ref`, `FnPtr`, `Dynamic`, `Never`. Identity is
  derived structurally over all fields. Recursion bottoms out
  at primitive variants; nested types reach through `TyId`
  (which has its own trivial `Hash`/`Eq`).
- **Tuple identity (structural-with-layout-excluded)**:
  `Ty::Tuple { fields, layout }`. Identity is determined by
  `(variant_kind_tag, fields)`; the inline `Layout` is *excluded*
  from `Hash + Eq` for the same reason as ADT-shaped variants
  -- layout is functionally determined by `fields` (rustc's
  `tcx.layout_of` is deterministic per `(ty, typing_env)`), so
  including it in identity would duplicate information already
  captured by `fields` and could cause spurious distinct
  entries if two translation paths happen to compute the same
  layout via numerically-different but structurally-equal
  intermediate `FieldLayout` values.

The nominal/structural split lets self-referential ADTs intern
without cycle: `struct Node { next: *mut Node }` is interned by
reserving `TyId(K)` for `Ty::Struct{item: NodeItemId, args: []}`
*before* computing the layout. While computing the layout, the
bridge translates `*mut Node`: it interns the pointer type
`Ty::Ptr(TyId(K), Mutability::Mut)` as `TyId(K+1)` -- the
`Ptr`'s inner pointee field references the just-reserved
`TyId(K)`, terminating the recursion. The `next` field's
`FieldLayout.ty` then carries `TyId(K+1)` (the pointer type),
not `TyId(K)` directly. Finally the layout is patched into
`storage[K]`. Without the reservation rule, "children-before-
parents" interning cannot terminate on self-referential ADTs
(`findings-rustc-draft.md` Theme A.5: rustc's own `AdtDef` does
not embed field types and uses field `DefId`s as the avoidance
mechanism for `struct S { x: Box<S> }`-style recursion, citing
`rustc_middle/src/ty/adt.rs:74-98`). Section 3.3 walks through
the full intern sequence with explicit TyId assignments.

**Reasoning.**

The strategy is forced by five concurrent constraints:

1. *F4-2 ArgsKey uses a deep structural canonicalizer, distinct
   from the BiMap's in-build `Hash + Eq`.* Stage 1 specifies
   `ArgsKey` as a deterministic fingerprint of `generic_args`
   whose **type args are compared by structural `Ty`
   representation, with the canonical form pinned in stage 3**
   (`stage-1/spec-snapshot.md:1254-1257`). We pin the canonical form
   per Ty variant kind:

   - **ADT-shaped variants** (`Ty::Struct`, `Ty::Enum`,
     `Ty::Union`, `Ty::FnDef`, `Ty::Closure`): canonical encoding
     is exactly `(variant_kind_tag, DefPathHash, canonical_args)`.
     The inline `Layout` (or capture struct, etc.) is **not
     traversed** -- it is functionally determined by `(item, args)`
     (rustc's layout query is deterministic per `(ty,
     typing_env)`), so traversing it would (a) duplicate
     information already captured by `DefPathHash + canonical_args`
     and (b) reintroduce the cycle the ADT terminator is meant to
     break (self-referential ADTs whose layout fields point back
     at the parent). `DefPathHash` is the stable cross-build
     identity from stage-1's `item_def_paths` table
     (`stage-1/spec-snapshot.md:1010, :1052-1064, :1231-1236`).
     `canonical_args` is `GenericArgs` recursively canonicalized
     per the per-IR-identifier rules below.
   - **Non-ADT structural variants** (`Ty::Bool`, `Ty::Char`,
     `Ty::Int`, `Ty::Uint`, `Ty::Float`, `Ty::Str`, `Ty::Array`,
     `Ty::Slice`, `Ty::Ptr`, `Ty::Ref`, `Ty::FnPtr`,
     `Ty::Dynamic`, `Ty::Never`, `Ty::Tuple`): canonical encoding
     is the structural recursion over the variant's fields, with
     the per-IR-identifier rules below applied wherever an
     ID-bearing field appears.

   **Per-IR-identifier rules** (apply inside non-ADT structural
   recursion and inside `canonical_args` of ADT-shaped variants;
   *not* inside an ADT's inline `Layout`, which is skipped per
   the ADT rule above):

   - **`TyId`** (in `Ty::Ref`, `Ty::Array`, `Ty::Slice`,
     `Ty::Ptr`, `Ty::Tuple`, `GenericArg::Type`): resolve via
     `model.types.resolve(...)` and recurse on the resolved `Ty`.
     `FieldLayout.ty` and `TagScalar.scalar_ty` live inside
     `Layout` and are not visited.
   - **`ItemId`** (in `Ty::Struct/Enum/Union { item, ... }`,
     `Ty::FnDef(item, ...)`, `Ty::Closure(item, ...)`): replace
     with `DefPathHash` from `model.item_def_paths[item]`. This
     is the cross-build terminator for ADT-shaped variants.
   - **`FnSigId`** (in `Ty::FnPtr`): resolve via
     `model.sigs.resolve(...)` and expand the `FnSig`'s contents.
     The minimum schema for canonicalization is **pinned in
     stage 3** (so that F4-2 is implementable today; stage 11
     may extend the full body-side `FnSig` table without
     revising this minimum):
     - `inputs: Vec<TyId>` -- positional argument types,
       canonicalized recursively via the `TyId` rule above.
     - `output: TyId` -- return type, canonicalized recursively.
     - `abi: FnAbi` -- the locked D15 enum (`Rust` or `C`);
       included in the canonical encoding to preserve type
       identity per D15's soundness argument.
     - `c_variadic: bool` -- whether the signature has a
       trailing `...` C-variadic. Included in the canonical
       encoding (variadic and non-variadic FnPtrs are distinct
       types in rustc; `extern "C" fn(i32, ...)` cannot
       collapse to `extern "C" fn(i32)`).
     - `safety: Safety` -- a 2-variant `enum Safety { Safe,
       Unsafe }` reflecting the `unsafe fn` marker on the
       function pointer. Included in the canonical encoding
       (rustc treats `unsafe fn(T) -> U` and `fn(T) -> U` as
       distinct types).

     The `FnSig` itself is not nominally identified -- its
     content is inlined into the canonical encoding. Stage 11's
     full `FnSig` shape may add fields beyond this minimum
     (e.g., parameter names for diagnostics); those additions
     are ignored by the canonicalizer because they do not
     affect type identity.
   - **`StringId`** (reachable via `Item.def_path` segments;
     `FieldLayout.name` and `VariantLayout.name` reside inside
     `Layout` and are skipped along with the rest of the
     untraversed Layout): when a `StringId` is reached --
     primarily during `DefPathHash` derivation if the bridge
     materialises path content rather than using rustc's
     pre-computed hash -- replace with the resolved string
     content via `model.strings.resolve(...)`. In practice the
     stage-1 `DefPathHash` value is already cross-build-stable
     and is what the canonicalizer terminates at, so explicit
     `StringId` resolution is rarely on the canonicalizer's
     critical path.
   - **`ConstId`** (in `GenericArg::Const`): replace with the
     canonical const-eval result. The exact canonical form is
     pinned in stage 4 (`stage-1/spec-snapshot.md:1258-1259`).

   This deep canonicalizer is **distinct from the BiMap's in-build
   `Hash + Eq`**. The BiMap's equality is *shallow*: it compares
   nested `TyId` integers directly (sound within a build because
   the BiMap's dedup ensures structurally-equal Tys have equal
   TyIds). The F4-2 canonicalizer is *deep*: it walks content, so
   it produces stable encodings *across* builds regardless of
   integer TyId values. The two notions of equality coexist
   without contradiction because they answer different questions:
   "is this Ty already interned?" (shallow, in-build) vs "is this
   Ty's content equivalent to that Ty's content from another
   build?" (deep, cross-build).

   Cross-build properties:
   - `ItemId` and `InstanceId` orderings are stable across builds
     when reachable sets match (`stage-1/spec-snapshot.md:1067-1070,
     :1232-1236, :1262-1264`); F4 / F4-2 sort keys use the deep
     canonicalizer (DefPath-stable for items; deep ArgsKey for
     instances), so cross-build stability does not depend on TyId
     integer stability.
   - `TyId` integer values are stable across builds *as a side
     effect* when the bridge walks instances in F4-2-sorted order
     and translates types in source order (canonical walk
     discipline). Cross-build TyId stability is recommended for
     deterministic snapshot diffs but is not load-bearing for
     F4-2 -- the deep canonicalizer delivers stability regardless.
   - Snapshot tests render `Ty` content via the pretty-printer
     (Section 4); the rendered form does not embed TyId integers,
     so snapshots are stable on content.
2. *No process-global state.* Q1 (regseq-model has zero rustc
   deps) plus general hygiene rule out RwLock-style global hash-
   cons interners (Charon).
3. *Q2 eager translation.* The `'tcx` lifetime does not survive
   past the bridge call. Anything keyed on `rustc::Ty<'tcx>` must
   die with the bridge.
4. *O(1) `Eq`/`Hash` on `TyId`.* `TyId` propagates everywhere
   (places, operands, GenericArgs, FieldLayouts). Deep structural
   equality (mrustc-style refcount) is too expensive at every
   comparison site; pointer-equality on a `Ty<'tcx>`-style
   interned wrapper is per-session and not stable for serialization.
   Dense u32 indices give O(1) at every site.
5. *Dense u32 IDs.* Stage 1 locks `TyId` at u32 with niche-
   optimised `Option<TyId>` (`stage-1/spec-snapshot.md:117-128`),
   leaving the niche mechanism to implementation choice. Stage 3
   selects `NonMaxU32` for `TyId`. Anything our interning
   strategy chooses must produce dense u32 indices.

These five rule out: rustc-style arena interning (per-session
pointer identity, not stable for serialization), Charon-style
global RwLock (process-global), mrustc-style refcount (no O(1)
equality on handles). What remains is the `rustc_codegen_clr`
BiMap precedent, which is the closest match for our "Rust → C,
single IR, multiple printers" use case.

**Construction invariants.**

1. *Append-only post-finalize.* The model is mutable during
   bridge construction (the bridge's `&mut RegseqModel` extends
   to `&mut TyInterner`). Once `bridge::extract` returns,
   consumers hold `&RegseqModel` and `intern`/patch operations
   are rejected at compile time. There is no public removal
   API. Mirrors `StringInterner`'s posture.
2. *Content-unique.* For every `(id_a, id_b)` with `id_a != id_b`,
   `storage[id_a]` and `storage[id_b]` are not identity-equal
   under the nominal/structural identity rule above. Dedup is
   enforced at intern time via `lookup`.
3. *ADT forward references via reservation.* For ADT-shaped
   variants (`Ty::Struct`, `Ty::Enum`, `Ty::Union`, `Ty::FnDef`,
   `Ty::Closure`), the bridge interns by reserving a `TyId`
   first and -- for layout-bearing variants only -- patching the
   resolved Layout afterwards. The interning sequence (per
   `translate_adt` in Section 3.2):
   - Build a probe `Ty` matching the ADT-shaped sibling for the
     current `kind`: `Ty::Struct{item, args, layout: PENDING}`,
     `Ty::Enum{item, args, layout: PENDING}`, or
     `Ty::Union{item, args, layout: PENDING}` for layout-bearing
     kinds; `Ty::FnDef(item, args)` or `Ty::Closure(item, args)`
     for non-layout-bearing kinds.
   - Probe `find(&probe)` under the nominal-identity hash; if hit,
     return the existing `TyId` without recomputing or re-patching.
   - Otherwise: `intern(probe)` to push the placeholder (or, for
     non-layout-bearing kinds, the final form) to `storage`, get
     back `TyId(K)`, with the BiMap inserting the structurally-
     equal key into `lookup`.
   - For layout-bearing kinds (Struct/Enum/Union): recursively
     compute the layout. The recursion may re-encounter the same
     `(item, args)` and short-circuit via the `find` probe in
     `translate_adt`, receiving `TyId(K)` -- the cycle terminates.
     Patch `storage[K].layout = computed_layout`. Because the
     identity hash excludes layout, the `lookup` entry's key
     remains valid; subsequent probes against any
     `Ty::Struct{item, args, ...}` (or matching sibling) continue
     to resolve to `TyId(K)`.
   - For non-layout-bearing kinds (FnDef/Closure): no patch step
     -- the placeholder *is* the final form. (Closure layout
     lives in a separately-synthesised captures struct per D6;
     FnDef is a ZST identity carrier with no layout slot.)
   Non-ADT variants (Bool, Int, Ref, Array, Tuple, etc.) intern
   in one step: structural recursion bottoms out at primitives,
   and the structural identity rule includes all fields, so no
   reservation is needed.
4. *Children before parents (non-ADT only).* For non-ADT
   variants, recursive translation interns inner `TyId`s before
   constructing the outer `Ty`. ADT-shaped variants are exempt
   per invariant 3 (the parent's `TyId` is reserved first, then
   the children compute, possibly referencing the reservation).
5. *Within-build content-addressing.* Within a single bridge
   run, structurally-identical `Ty` values resolve to the same
   `TyId`. This is the dedup property that makes F4-2's
   structural ArgsKey computation cheap (compute once per
   distinct type; comparison reduces to TyId equality).
6. *No dangling references at finalize.* For every `TyId`
   referenced anywhere in the finalized model (in
   `FieldLayout.ty`, `Ty::Ref(ty, ...)`, `Ty::Array(ty, _)`,
   `GenericArgs`, etc.), `id.index() < types.len()`. Every
   PENDING layout has been patched. Debug-build assertion at the
   end of bridge construction.

**Methodology rules invoked.**

- "Compiler-theory-first" -- structural interning is the canonical
  type-identity mechanism in production compilers (rustc,
  rustc_codegen_clr); not a narrow workaround.
- "C semantics with information preservation" -- the structural
  fingerprint *is* the information rustc would compute as type
  identity; we preserve it as a dense ID rather than re-deriving it
  each comparison.
- "No defensive fallbacks" -- no per-build randomization, no
  multi-strategy fallback; one mechanism.

**Prior-art evidence.**

- *rustc* -- `Ty<'tcx>` is `Interned<WithCachedTypeInfo<TyKind>>`
  via `CtxtInterners::type_: InternedSet`; `intern_ty` populates
  the set keyed on `TyKind`'s `Hash`/`PartialEq`
  (`findings-rustc-draft.md` Theme C.1, citing
  `rustc_middle/src/ty/context.rs:921-952, 995-1011`). Per-session
  pointer identity. We mirror the structural-Hash/Eq strategy but
  produce dense IDs instead of pointers, for cross-build
  reproducibility.
- *rustc_codegen_clr* -- direct precedent: `BiMap` with
  `Interned<u32>` newtype handles. Single-IR, multiple-printer
  posture matches ours (`prior-art/rustc-codegen-clr.md` Theme C;
  `discussions.md` D3 entry).
- *rustc_public::Ty* -- newtype index `Ty(usize)` assigned by the
  bridge's `Tables` at population time (`findings-rustc-draft.md`
  Theme C.4). Structurally similar to our shape; the difference is
  rustc_public's bridge is rustc-internal, while ours is the
  external bridge crate.
- *Charon* -- process-global hash-cons via RwLock; ruled out by
  constraint 2 (`prior-art/charon.md` Theme C; `discussions.md`
  D3).
- *mrustc* -- no interning; deep refcount on `TypeRef`; ruled out
  by constraint 4 (`prior-art/mrustc.md` Theme C; `discussions.md`
  D3).

**Worked example.** See section 3.3 (TyInterner construction
walkthrough).

### 3.2 Decision D4 -- Interning location: `RegseqModel.types: TyInterner`

**Decision.** The `TyInterner` lives **inside `RegseqModel`** as the
`types` field, replacing the stage-1 placeholder `IndexVec<TyId,
Ty>`:

```rust
pub struct RegseqModel {
    // ...
    pub strings: StringInterner,    // stage 1
    pub types:   TyInterner,        // stage 3 -- refines IndexVec<TyId, Ty>
    // ...
}
```

This refines the stage-1 placeholder declaration at
`stage-1/spec-snapshot.md:1002`. The exact field type is stage-3's
prerogative per the stage-1 convention "the top-level RegseqModel
struct accordingly contains placeholder fields whose types are
stage-N concerns" (`stage-1/spec-snapshot.md:46-48`).

`TyInterner`'s public surface mirrors `StringInterner` plus a
read-only structural lookup for invariant checks and snapshot
diffing:

```rust
impl TyInterner {
    pub fn new() -> Self;
    pub fn intern(&mut self, ty: Ty) -> TyId;
    pub fn resolve(&self, id: TyId) -> &Ty;
    pub fn find(&self, ty: &Ty) -> Option<TyId>;   // structural-identity lookup; non-mutating
    pub fn len(&self) -> usize;
    pub fn iter(&self) -> impl Iterator<Item = (TyId, &Ty)>;
}
```

`find` returns the existing `TyId` for a structurally-equal `Ty`
without inserting; consumers use it for "is this Ty already in
the model?" checks (e.g., post-build invariant assertions, the
F4-2 ArgsKey comparator dedup-cache). `find` is read-only;
construction uses `intern`. `StringInterner` does not currently
expose a corresponding `find` -- a deferred symmetry to revisit
if a consumer materialises.

Two locations, one responsibility each:

- *Inside `regseq-model`*: `TyInterner` (the BiMap). All structural
  dedup logic lives here. Tests can construct sample models without
  rustc by calling `RegseqModel::new()` followed by
  `model.types.intern(Ty::Bool)` etc.
- *Inside `regseq-driver` (the bridge)*: a translation cache
  `HashMap<rustc::Ty<'tcx>, TyId>`, bridge-local. Avoids
  re-translating the same rustc type within a single bridge run.
  Forced bridge-side by Q1 (regseq-model has zero rustc deps; cannot
  mention `rustc::Ty<'tcx>`); dies with the bridge call per Q2.

The bridge has two translation paths -- a generic structural
path for non-ADT variants, and an ADT-shaped path with
reservation per D3 invariant 3 (Section 3.1 above). The generic
path:
```
// Non-ADT variants: interning happens after recursive translation.
fn translate_ty(&mut self, rustc_ty: rustc::Ty<'tcx>) -> TyId {
    if let Some(&id) = self.ty_cache.get(&rustc_ty) { return id; }
    if is_adt_shaped(rustc_ty) {
        return self.translate_adt(rustc_ty);   // reservation path
    }
    let our_ty = build_non_adt_ty_from_rustc(rustc_ty, self);   // recurses on inner Tys first
    let id = self.model.types.intern(our_ty);
    self.ty_cache.insert(rustc_ty, id);
    id
}
```

The ADT-shaped path uses TyId reservation to break self-reference
cycles. The pseudocode below uses `find` to detect a nominal-
identity hit before reserving, so the layout computation and
`set_layout` only run on fresh insertions; nominal-identity hits
return the existing `TyId` without recomputing layout (consistent
with D3 invariant 3 and the bridge-side metadata-equality
discipline at Section 3.1):

```
// AdtKind picks which sibling Ty variant the bridge constructs.
enum AdtKind { Struct, Enum, Union, FnDef, Closure }

fn translate_adt(&mut self, rustc_ty: rustc::Ty<'tcx>) -> TyId {
    let (kind, item, args) = adt_components(rustc_ty);
    if let Some(&id) = self.ty_cache.get(&rustc_ty) { return id; }
    // Build the probe matching the actual ADT-shaped Ty sibling
    // (D1.2 keeps Struct/Enum/Union as siblings; FnDef and Closure
    // are their own variants).
    let probe = match kind {
        AdtKind::Struct  => Ty::Struct { item, args, layout: Layout::pending() },
        AdtKind::Enum    => Ty::Enum   { item, args, layout: Layout::pending() },
        AdtKind::Union   => Ty::Union  { item, args, layout: Layout::pending() },
        AdtKind::FnDef   => Ty::FnDef(item, args),       // ZST; no Layout
        AdtKind::Closure => Ty::Closure(item, args),     // captures struct layout
    };
    // Nominal-identity probe: do we already have a TyId for this (kind, item, args)?
    if let Some(id) = self.model.types.find(&probe) {
        // Existing entry (possibly mid-construction PENDING for Struct/Enum/
        // Union; see invariant 3). Layout is already patched or being
        // patched up-stack. Don't recompute or re-patch.
        self.ty_cache.insert(rustc_ty, id);
        return id;
    }
    // Fresh reservation: insert the PENDING (or layout-less) placeholder,
    // get a new TyId.
    let id = self.model.types.intern(probe);
    self.ty_cache.insert(rustc_ty, id);
    // Layout-bearing variants compute and patch. Per D2 (Section 2.1)
    // inline Layout lives on Ty::Struct/Enum/Union only; FnDef is a ZST
    // identity carrier with no layout slot; Closure carries no inline
    // layout in v1 (closures become Ty::Closure(ItemId, GenericArgs)
    // referencing a separately-synthesised captures struct per D6 --
    // the captures struct is its own Ty::Struct with its own layout,
    // interned independently).
    if matches!(kind, AdtKind::Struct | AdtKind::Enum | AdtKind::Union) {
        let layout = self.compute_layout(rustc_ty);   // may recurse via translate_adt
                                                      // and circle back through the
                                                      // find probe above
        self.model.types.set_layout(id, layout);
    }
    id
}
```

Bridge-synthesised non-rustc types skip the rustc cache and call
the appropriate intern path directly: non-ADT synthesised types
call `model.types.intern(...)` directly; ADT-shaped synthesised
types (closure captures structs per D6; vtable type structs per
D5 with synthesis machinery in `stage-12/expectations.md`) go
through `translate_adt`'s reservation flow with a synthesised
`ItemId`. Drop-glue is a function decl, not a Ty. The fat-pointer
C shape for `&dyn Trait` is codegen-emitted on demand (D5.3-B),
mirroring the existing handling of `&str` and `&[T]`;
`Ty::Dynamic` survives in the IR through to codegen and is not
rewritten by any bridge or model-level pass. The dedup mechanism
is uniform across rustc-translated and bridge-synthesised paths
-- the BiMap's nominal/structural identity rule decides identity,
not the path that reached it.

**Reasoning.**

The location decision is dominated by the `StringInterner`
precedent at `stage-1/spec-snapshot.md:250-254`, which already
established the BiMap-in-model pattern for content-addressed value
types. Mirroring it gives us:

- *Mutability boundary by borrow-checker.* `intern` requires
  `&mut self`; once the model finalises, consumers hold
  `&RegseqModel`, so `intern` is rejected at compile time. No
  separate "Builder vs Frozen" types are required to enforce
  read-only post-construction; the borrow checker enforces it
  for free.
- *Test ergonomics.* `RegseqModel::new()` + `model.types.intern(...)`
  matches the `model.strings.intern(...)` pattern. Stage-1
  integration tests already construct hand-built models; the same
  pattern extends to types.
- *Serialization symmetry.* Both interners use the same custom
  serde shape: on-wire is `storage` only; `lookup` rebuilds at
  deserialize. One serialization pattern, two consumers.
- *No additional crate boundary.* Putting the BiMap entirely
  bridge-side would require either (a) re-exporting `Ty`'s
  `Hash`/`Eq` somehow or (b) duplicating the dedup machinery
  outside its data type. The model-side location keeps the
  structural-equality logic with the value type that defines it.

The alternative -- "bridge-side only, IR exposes `IndexVec<TyId,
Ty>` post-build" -- was the original lean (`discussions.md` D4) but
loses the `StringInterner` symmetry and pushes content-unique-by-
construction enforcement out to bridge code. The model-side
location is the strictly stronger position once the
`StringInterner` precedent is recognised.

**Methodology rules invoked.**

- "C semantics with information preservation" -- type identity is
  derivable structural information; keep the derivation co-located
  with the type definition.
- "Compiler-theory-first" -- BiMap-in-model is the
  rustc_codegen_clr precedent and the StringInterner-internal
  pattern; not a custom layering.

**Prior-art evidence.**

- *Stage-1 `StringInterner`* -- `stage-1/spec-snapshot.md:250-254`
  defines the precedent: `storage: Vec<Arc<str>>` + `lookup:
  HashMap<Arc<str>, StringId>`, both inside the interner struct,
  the interner inside `RegseqModel.strings`. Custom serde, append-
  only invariants, deserialize-time validation. We replicate the
  shape for `TyInterner.{storage, lookup}` with `IndexVec<TyId, Ty>`
  and `HashMap<Ty, TyId>` respectively.
- *rustc_codegen_clr* -- BiMap embedded in the project's IR
  context, not in a separate bridge layer
  (`prior-art/rustc-codegen-clr.md` Theme C).
- *rustc* -- `CtxtInterners` are part of `TyCtxt`, the top-level
  IR context, not a separate construction pipeline
  (`findings-rustc-draft.md` Theme C.1).

**Serialization.**

`TyInterner` implements `Serialize` / `Deserialize` manually,
mirroring `StringInterner`'s shape. The on-wire form is `storage`
only (a flat array of `Ty` values, indexed by their original
`TyId`); `lookup` is reconstructed at deserialize by direct
insertion -- *not* via the public `intern` API. Round-trip
preserves all `TyId` values: each entry's index in the on-wire
array is the entry's `TyId`, so internal index-based references
(e.g., the recursive-Node trace where `Node` at `TyId(K)`
references `*mut Node` at `TyId(K+1)`) remain valid after load.
Step 3 below (content-uniqueness validation) prevents the
direct-insertion path from ever creating a duplicate `lookup`
key.

Deserialize flow:

1. Read the full `storage` array first. Forward references are
   permitted: ADT layouts may reference TyIds at higher indices
   than the ADT itself (e.g., the self-referential `struct Node {
   next: *mut Node }` worked example produces `Node` at `TyId(K)`
   referencing `*mut Node` at `TyId(K+1)`). We do not require a
   children-before-parents serialization order; ordering is
   determined by the bridge's intern sequence, which uses TyId
   reservation for ADTs.
2. Validate references *after* the full array is loaded. Every
   `TyId` referenced inside any `Ty` (in `Ty::Ref(inner, ...)`,
   `FieldLayout.ty`, `GenericArgs`, etc.) must satisfy
   `id.index() < storage.len()`. This is the dangling-reference
   check, agnostic to serialization order.
3. Validate content-uniqueness under the nominal/structural
   identity rule: no two distinct `TyId`s in `storage` are
   identity-equal (same `(item, args)` for ADT-shaped variants,
   same structural shape for the rest). The interner is
   content-unique by construction; a duplicate in the input is a
   corrupted artifact.
4. Rebuild `lookup` by direct insertion (not via the public
   `intern`): for each `(id, ty)` in `storage.iter_enumerated()`,
   insert `(ty.clone(), id)` into a fresh `lookup`. This
   preserves the original `TyId` values from storage; calling
   `intern` instead would allocate fresh IDs and break the
   index-based references inside each `Ty` (e.g., the recursive-
   ADT example where `Node` at `TyId(K)` references `*mut Node`
   at `TyId(K+1)`).

Any failure surfaces as a clear deserialization error, not a
defensive fallback (per methodology).

**Cross-build determinism.** Two RegseqModels built from the same
source under the pinned toolchain are **content-equivalent**: the
set of distinct `Ty` values is identical, every `Ty`'s structural
shape matches, and every `Layout` matches. `InstanceId` /
`ItemId` ordering is deterministic per F4 / F4-2
(`stage-1/spec-snapshot.md` determinism rules), which compute over
**structural `Ty` content**, not `TyId` integers. `ArgsKey`
deep canonicalization (D3, Section 3.1) is the load-bearing
cross-build comparator -- it walks `Ty` content, terminates
`ItemId` at `DefPathHash`, and resolves all other IR identifiers
to their content. **Integer `TyId` values are NOT guaranteed to
be identical across builds**: the bridge's discovery walk may
intern types in a different order between runs, producing the
same content with different integer assignments. Consumers must
not key on `TyId` integers across builds; the pretty-printer
renders `Ty` content (not `TyId`), so snapshot tests are stable
on content, and `assert_json_snapshot!` round-trips because
storage order is preserved within a single serialization. A
bridge implementation MAY produce reproducible `TyId` integers
as a recommended deterministic property (e.g., by walking
instances in F4-2 order), but no IR contract relies on it;
cross-build identity goes through the deep canonicalizer.
Stability of content is conditional on the pinned toolchain: a
change to rustc's `DefPathHash` algorithm or to const-evaluation
canonicalization (stage 4) requires re-pinning F4-2.

### 3.3 Worked example -- TyInterner construction walkthrough

This walks through interning `&mut u32` -- a small but representative
case (one nested `TyId`, one bridge-side cache hit on the second use).

**Source MIR.** Imagine the bridge encounters:
```rust
let p: &mut u32 = ...;
let q: &mut u32 = ...;    // structurally identical type
```

Both locals carry `rustc::Ty<'tcx>` values produced by rustc. Within a
single rustc session those two `Ty<'tcx>` references are
*pointer-equal* (per `findings-rustc-draft.md` Theme C.1 -- rustc
structurally interns), so the bridge's `ty_cache:
HashMap<rustc::Ty<'tcx>, TyId>` lookup succeeds on the second.

**First encounter (`p`'s type).**

1. `translate_ty(rustc_ty_for_&mut_u32)`:
   - `ty_cache.get(rustc_ty_for_&mut_u32)` → miss.
   - Recurse on the inner type `u32`:
     - `translate_ty(rustc_ty_for_u32)` → miss in cache.
     - No further recursion; build `Ty::Uint(UintKind::U32)`.
     - `model.types.intern(Ty::Uint(UintKind::U32))`:
       - `lookup.get(&Ty::Uint(...))` → miss.
       - `storage.push(Ty::Uint(...))` → returns `TyId(0)`.
       - `lookup.insert(Ty::Uint(...), TyId(0))`.
       - Return `TyId(0)`.
     - `ty_cache.insert(rustc_ty_for_u32, TyId(0))`.
   - Build `Ty::Ref(TyId(0), Provenance::UniqueBorrow)`.
   - `model.types.intern(...)`:
     - `lookup.get(&Ty::Ref(0, Unique))` → miss.
     - `storage.push(...)` → returns `TyId(1)`.
     - `lookup.insert(...)`.
     - Return `TyId(1)`.
   - `ty_cache.insert(rustc_ty_for_&mut_u32, TyId(1))`.
   - Return `TyId(1)`.

After the first encounter:
- `model.types.storage = [Ty::Uint(U32), Ty::Ref(TyId(0), Unique)]`
- `model.types.lookup = {Ty::Uint(U32) → TyId(0), Ty::Ref(...) → TyId(1)}`
- bridge `ty_cache = {rustc_ty_u32 → TyId(0), rustc_ty_&mut_u32 → TyId(1)}`

**Second encounter (`q`'s type).**

1. `translate_ty(rustc_ty_for_&mut_u32)`:
   - `ty_cache.get(rustc_ty_for_&mut_u32)` → **hit** (returns `TyId(1)`).
   - Return `TyId(1)` immediately. No recursion, no `intern` call.

The bridge cache short-circuits the recursive translation. Even if
the cache missed (e.g., a synthesised `Ty` that bypasses the rustc
cache path), the structural `lookup: HashMap<Ty, TyId>` would still
collapse the second `intern(Ty::Ref(TyId(0), Unique))` call to
`TyId(1)` by hash hit. **Two-layer dedup**: the bridge cache
short-circuits rustc-Ty-keyed work; the IR's structural dedup is
the final authority on identity.

**Cross-build property.** Two builds of the same source produce
*content-equivalent* `RegseqModel`s -- the set of distinct `Ty`
values is the same, every `Ty`'s structural shape matches, every
`Layout` matches, and `InstanceId`/`ItemId` ordering is stable per
F4 / F4-2 (which compute over structural Ty content, not `TyId`
integers per `stage-1/spec-snapshot.md:1254-1257`). Integer `TyId`
values may differ across builds depending on the bridge's
discovery walk; the pretty-printer renders `Ty` content (not
`TyId`), so snapshot tests are stable on content. JSON serialization
round-trips because the on-wire shape preserves the storage array
in order, and `lookup` is rebuilt at deserialize.

**Self-referential ADT trace.** For `struct Node { next: *mut Node }`:

1. Bridge encounters `Node` (rustc-side, with monomorphized args).
2. `intern_ty_for_adt(node_def_id, args=[])`:
   - Reservation: push `Ty::Struct{item: NodeItemId, args: [], layout: PENDING}`
     to `storage`, get `TyId(K)`. Insert into `lookup` under
     the nominal-identity key (item + args).
   - Compute layout for `Node`:
     - For field `next: *mut Node`: translate `*mut Node`.
       - Recurses into `intern_ty_for_adt(node_def_id, [])` for
         the inner `Node`.
       - Hits `lookup` under nominal identity → returns `TyId(K)`.
       - Builds `Ty::Ptr(TyId(K), Mutability::Mut)`, interns
         structurally → some `TyId(K+1)`.
       - Returns `TyId(K+1)`.
     - Builds `FieldLayout { name: "next", ty: TyId(K+1), offset: 0 }`.
     - Builds `Layout { size: 8, align: 8, pack: None, uninhabited: false,
       kind: LayoutKind::Struct { fields: [next_field] } }`.
   - Patch `storage[K].layout = computed_layout`.
3. Subsequent encounters of `Node` (via the rustc cache) short-circuit
   to `TyId(K)`. The cycle resolves cleanly.

**Read-only post-build.** Once the bridge returns `RegseqModel`,
consumers hold `&RegseqModel` (analyses, codegen, pretty-printer).
`intern` requires `&mut self`, so it is rejected at compile time.
`resolve(id) -> &Ty` and `iter()` are the consumer-facing surface;
they require only shared access.

---

## 4. Lowering reference (C output examples)

This section is a worked-example catalog: for each non-trivial
`Ty` variant or flag combination, the canonical C lowering is
shown. It serves as a soundness check on the IR (does our
representation carry enough to drive correct codegen?) and as
input to stage 11/12/13 implementers.

Naming conventions in examples:
- `*_t` typedef suffix for any IR-synthesized C type.
- `rust_*` prefix for shape-equivalent types whose Rust origin is
  compiler-built-in (str, slice, dyn).
- Function names follow `<Item>_<method>` pattern (illustrative
  only; final mangling rules are stage-12 territory).

### 4.1 Reference and pointer types

```rust
&u32                  // Ref(U32, SharedBorrow)
&mut u32              // Ref(U32, UniqueBorrow)
*const u32            // Ptr(U32, Not)
*mut u32              // Ptr(U32, Mut)
&UnsafeCell<u32>      // Ref(UnsafeCell<u32>, SharedBorrow); UnsafeCell flag on Item
```

```c
const uint32_t *               // &u32 (SharedBorrow; no restrict because shared aliases are allowed)
uint32_t *restrict             // &mut u32 (UniqueBorrow; codegen MAY add restrict where uniqueness is preserved)
const uint32_t *               // *const u32 (Ptr Not)
uint32_t *                     // *mut u32  (Ptr Mut)
uint32_t *                     // &UnsafeCell<u32> (interior mutability; const dropped, restrict already absent for shared)
```

The `const` qualifier is dropped from `Ref(_, SharedBorrow)` when the
pointee transitively contains `UnsafeCell` (interior mutability). The
`const` qualifier is **kept** on `Ptr(_, Not)` even when the pointee
transitively contains `UnsafeCell`: raw pointers carry no source-level
aliasing contract that interior mutability could undermine, and the
`const` is a verbatim mirror of the source `*const` marker. The
`restrict` qualifier is added to `&mut` references (`UniqueBorrow`)
in declaration contexts where uniqueness is preserved per the
methodology rule (`../README.md` "C semantics with information
preservation"); it is not added to shared `&T` references or to raw
pointers regardless of `UnsafeCell` presence.

### 4.1.1 Bridge translation: edge cases

The bridge translation rule for `Ref` / `Ptr` is uniform; no
pointer-specific recursion logic is required. The pointee is
recursively translated and interned per D4. The table below shows
the rule's behavior on common edge cases:

| Source | IR |
| --- | --- |
| `&str` | `Ty::Ref(Ty::Str, SharedBorrow)` -- fat pointer materialized by codegen, see Section 4.2 |
| `&[T]` | `Ty::Ref(Ty::Slice(translate(T)), SharedBorrow)` |
| `&dyn Trait` | `Ty::Ref(Ty::Dynamic { auto_traits, vtable_ty }, SharedBorrow)` -- `Ty::Dynamic` shape locked by D5 (Section 1.10); fat-pointer C typedef emitted by codegen on demand (D5.3-B). HRTB-bearing `dyn for<'a> Trait<&'a T>` is rejected at the bridge per the D7/D8 refinement above. |
| `&&u32` | `Ty::Ref(Ty::Ref(Ty::Uint(U32), SharedBorrow), SharedBorrow)` |
| `*const *mut u32` | `Ty::Ptr(Ty::Ptr(Ty::Uint(U32), Mut), Not)` |
| `&!` | `Ty::Ref(Ty::Never, SharedBorrow)` -- accepted at type level; codegen may reject when emitted |
| `*const ()` | `Ty::Ptr(Ty::Tuple { fields: [], layout: <unit> }, Not)` (shorthand: empty tuple) |
| `&UnsafeCell<u32>` | `Ty::Ref(<UnsafeCell<u32>>, SharedBorrow)` -- the `is_unsafe_cell` flag on the `UnsafeCell<u32>` `Item` is what triggers the `const`-drop rule |

Invariants the rule guarantees:

1. No `Provenance` value outside `{ SharedBorrow, UniqueBorrow }`
   can result from translating a rustc `TyKind::Ref` (rustc's
   `Mutability` enum is exhaustive).
2. No `Ty::Ref` that enters the IR carries a region. For non-HRTB
   parents the `_region` slot is `ReErased` (free) and is
   discarded; for HRTB-bearing parents (`Ty::FnPtr` over a
   `Binder<FnSig>` containing `ReBound`, `Ty::Dynamic` over a
   `Binder<Predicate>` containing `ReBound`) the parent is
   rejected with `BridgeError::UnsupportedHRTB` *before* any
   nested `TyKind::Ref` translates, so a `ReBound` slot never
   reaches the discard step on an admitted IR type. (See the
   "Refinement: higher-ranked binders (HRTB) are rejected in v1"
   block in Section 1.8.)
3. The translation is **total over non-HRTB inputs**: every
   `TyKind::Ref` / `TyKind::RawPtr` whose enclosing parent is
   accepted is translated without panic or diagnostic. HRTB-
   bearing parents are rejected at parent-level translation per
   invariant 2 above; this is the only rejection path on the
   `Ref`/`Ptr` lowering surface.
4. The translation is deterministic.

### 4.2 Fat-pointer types: `&str`, `&[T]`, `&dyn Trait`

**Rust:**
```rust
let s: &str = "hello";
let xs: &[u32] = &[1, 2, 3];
let d: &dyn Driver = &my_driver;
```

**C:**
```c
typedef struct rust_str { const uint8_t *data; size_t len; } rust_str_t;

typedef struct rust_slice_u32 {
    const uint32_t *data;
    size_t len;
} rust_slice_u32_t;

typedef struct trait_obj_Driver {
    const void *data;
    const Driver_VTable *vtable;
} trait_obj_Driver_t;

static const uint8_t __str_lit_0[] = { 'h', 'e', 'l', 'l', 'o' };  // no NUL
static const rust_str_t HELLO = { __str_lit_0, 5 };

static const uint32_t __arr_lit_0[] = { 1, 2, 3 };
static const rust_slice_u32_t XS = { __arr_lit_0, 3 };
```

The `dyn Trait` lowering follows D5 (Section 1.10): `Ty::Dynamic`
survives in the IR through to codegen, and codegen emits the
fat-pointer typedef on demand for `Ref(t, _)` / `Ptr(t, _)` where
`t` resolves to `Ty::Dynamic`. The shape shown above is the v1
codegen target; see Section 4.9 for the full worked example.

### 4.3 PhantomData omission

**Rust:**
```rust
struct Mmio<T> {
    base: *mut u8,
    _phantom: PhantomData<T>,
}
// Monomorphized to Mmio<u32>: Layout = { size: 8, align: 8, fields: [(0, *mut u8)] }
```

**C:**
```c
typedef struct Mmio_u32 {
    uint8_t *base;
    // PhantomData<u32> field omitted
} Mmio_u32_t;
// sizeof(Mmio_u32_t) == 8
```

The bridge identifies the `_phantom` field's type by its `Item`
flag `is_phantom_data` and elides the field at C-emit time. Without
elision, the C struct would be at least 9 bytes (1 for the empty
struct + alignment padding), breaking the rustc-claimed layout.

### 4.4 Niche-encoded enum: `Option<&u32>`

**Rust:**
```rust
fn f(x: Option<&u32>) -> u32 {
    match x {
        Some(p) => *p,
        None    => 0,
    }
}
```

**Layout** (from `tcx.layout_of(Option<&u32>)`):
- `size = 8`, `align = 8`
- `tag = Scalar { value: Pointer, valid_range: 1..=u64::MAX }`
- `tag_encoding = Niche { untagged_variant: Some, niche_variants: None..=None, niche_start: 0 }`
- `tag_field = 0`

**C:**
```c
typedef struct Option_ref_u32 {
    const uint32_t *p;   // 0 = None; nonzero = Some(p)
} Option_ref_u32_t;
// sizeof == 8

uint32_t f(Option_ref_u32_t x) {
    if (x.p == NULL) {
        return 0;        // None branch
    } else {
        return *x.p;     // Some branch
    }
}

// Construction:
Option_ref_u32_t make_some(const uint32_t *p) { return (Option_ref_u32_t){ p }; }
Option_ref_u32_t make_none(void)              { return (Option_ref_u32_t){ NULL }; }
```

The niche encoding eliminates the explicit tag byte; the layout's
`tag_encoding = Niche` field is what tells codegen to perform the
"if (x.p == NULL) ... else ..." discriminant-read pattern.

### 4.5 UnsafeCell aliasing

**Rust:**
```rust
struct Counter { value: UnsafeCell<u32> }
fn read(c: &Counter) -> u32 { unsafe { *c.value.get() } }
```

**C:**
```c
typedef struct Counter { uint32_t value; } Counter_t;
// Note: NO `const` qualifier on the parameter (UnsafeCell allows mutation through &T).
// `restrict` is not applicable -- shared borrow already has no `restrict`.
uint32_t Counter_read(Counter_t *c) {
    return c->value;
}
```

The presence of `is_unsafe_cell` in the `Counter` struct's transitive
field types tells codegen to drop the `const` qualifier from references
to `Counter`. `restrict` is borrow-flavor-determined (`UniqueBorrow`
only), so the question of dropping it does not arise for `&Counter`
(`SharedBorrow`).

### 4.6 MaybeUninit (drop suppression via Union + ManuallyDrop)

**Rust:**
```rust
let mut buf: MaybeUninit<[u8; 64]> = MaybeUninit::uninit();
fill(&mut buf);
let buf: [u8; 64] = unsafe { buf.assume_init() };
// at scope exit, buf is dropped
```

**C:**
```c
uint8_t buf[64];        // uninitialized (no = {0})
fill(buf);
// `assume_init` is a no-op cast; buf is now usable
// NO drop-glue Call emitted for the MaybeUninit slot
```

`MaybeUninit<T>` is structurally `union { uninit: (), value: ManuallyDrop<T> }`
(verbatim from the pinned toolchain at
`library/core/src/mem/maybe_uninit.rs:345-348`). Its drop-suppression is
the composition of two existing rules: (a) Rust unions do not auto-drop
their fields (a property of the `Ty::Union` variant we already handle);
(b) the inner `ManuallyDrop<T>` is recognized via its own
`is_manually_drop` flag (see D1.5), which suppresses drop-glue for its
wrapped value. No `is_maybe_uninit` flag is required -- the existing
machinery is sufficient. This is why D1.5 does not list `MaybeUninit`.

### 4.7 ManuallyDrop drop suppression

**Rust:**
```rust
struct Holder { lock: ManuallyDrop<Mutex> }
// At Holder's drop, the Mutex is NOT dropped automatically.
```

**C:**
```c
typedef struct Holder { Mutex_t lock; } Holder_t;

void Holder_drop(Holder_t *h) {
    // Other fields' drop-glue Calls go here.
    // h->lock is NOT dropped (ManuallyDrop suppresses).
}
```

The `is_manually_drop` flag on the field's `Item` tells codegen to
skip the drop-glue Call for that field.

### 4.8 Packed struct: `__attribute__((packed))` emission

**Rust:**
```rust
#[repr(C, packed)]
struct UartHeader {
    magic: u8,
    seq:   u32,
    flags: u8,
}
// Layout (size:6, align:1, pack:Some(1)):
//   kind = Struct {
//     fields: [
//       FieldLayout { name: "magic", ty: u8,  offset: 0 },
//       FieldLayout { name: "seq",   ty: u32, offset: 1 },
//       FieldLayout { name: "flags", ty: u8,  offset: 5 },
//     ],
//   }
```

**C:**
```c
typedef struct UartHeader {
    uint8_t  magic;
    uint32_t seq;
    uint8_t  flags;
} __attribute__((packed)) UartHeader_t;

_Static_assert(sizeof(UartHeader_t) == 6,                "UartHeader size mismatch");
_Static_assert(_Alignof(UartHeader_t) == 1,              "UartHeader alignment mismatch");
_Static_assert(offsetof(UartHeader_t, magic) == 0,       "UartHeader.magic offset mismatch");
_Static_assert(offsetof(UartHeader_t, seq) == 1,         "UartHeader.seq offset mismatch");
_Static_assert(offsetof(UartHeader_t, flags) == 5,       "UartHeader.flags offset mismatch");
```

Without `__attribute__((packed))`, the C compiler would naturally
align `seq` to offset 4 (matching `uint32_t`'s 4-byte alignment),
producing `sizeof(UartHeader_t) == 12` -- inconsistent with the
rustc-claimed layout. The `Layout.pack: Some(1)` flag tells codegen
to emit the attribute. The `_Static_assert`s verify the C compiler's
actual layout matches the rustc-resolved offsets; mismatch fails the
build at the C-compile step rather than producing silently-wrong
code.

For `pack: None`, codegen emits no `packed` attribute. For
`pack: Some(N)` with `N > 1`, the bridge rejects with a clear
diagnostic (per "no defensive fallbacks": `repr(packed(N>1))` is
admitted by the IR shape but not by the v1 codegen, since no driver
fixture has surfaced the pattern; revisit when a consumer
materialises).

### 4.9 `&dyn Driver` lowering (D5)

**Rust source.**

```rust
trait Driver {
    fn read(&self) -> u32;
}

struct PL011 { /* fields */ }

impl Driver for PL011 {
    fn read(&self) -> u32 { /* ... */ }
}

fn process(d: &dyn Driver) -> u32 {
    d.read()
}

let pl011 = PL011 { /* ... */ };
let result = process(&pl011);
```

**Stage-3 IR shape (after bridge construction).**

Stage-3's commitment for this example:

- `PL011::read` (the impl method) is lifted to a Function Item
  per `stage-1/spec-snapshot.md:597-603`, with concrete receiver
  `&PL011` (substituted from `&self`).
- The bridge synthesises the vtable type `Ty::Struct` (with
  `vtable_item` per stage-12 expectations) and interns it,
  yielding TyId `t_vtable_Driver`. The exact `vtable_item`
  DefPath, hash, and field set are pinned in
  `stage-12/expectations.md`.
- `Ty::Dynamic` for `dyn Driver`:
  ```
  Ty::Dynamic {
      auto_traits: [],                  // no Send/Sync/Unpin
      vtable_ty:   t_vtable_Driver,
  }
  ```
  Call this TyId `t_dyn_Driver`.
- `&dyn Driver` becomes `Ty::Ref(t_dyn_Driver,
  Provenance::SharedBorrow)` per locked D7/D8 in Section 1.8 (the
  `Mutability` field is dropped -- it was 1:1 redundant with
  `Provenance` for borrows).
- `process`'s signature: `fn(Ref(t_dyn_Driver, SharedBorrow)) ->
  u32`. Stored as a `FnSig` in `model.sigs` indexed by the
  function's `FnSigId`.
- Stage-12 will register the per-(Trait, SelfTy) vtable instance
  Item (kind Static, rendered `<vtable for Driver for PL011>`
  per stage-1's reservation) with vtable shim Function Items in
  two families -- per-(Trait, ConcreteImpl, method) method
  shims plus a per-(Trait, ConcreteImpl) drop-in-place shim --
  populating the initializer; this is forwarded to
  `stage-12/expectations.md`.
- Stage-11 will rewrite `process`'s body to call the loaded
  `vtable->read` indirectly; this is forwarded to
  `stage-11/expectations.md`.

**C output.**

Codegen pattern-matches `Ref(t, _)` where `t` resolves to
`Ty::Dynamic` (per D5.3-B) and emits:

```c
/* Vtable type emitted from stage-12-synthesised vtable Ty::Struct;
   the vtable function-pointer fields use erased receivers. */
typedef struct Driver_VTable {
    void (*drop_in_place)(void *self);
    size_t size;
    size_t align;
    uint32_t (*read)(const void *self);
} Driver_VTable_t;

/* Trait-object typedef per D5.3-B. SharedBorrow gives const-qualified data. */
typedef struct trait_obj_Driver {
    const void *data;
    const Driver_VTable_t *vtable;
} trait_obj_Driver_t;

/* Vtable method shim for PL011's `read` impl (synthesised per stage-12's
   C.1 method-shim rule; bridges the erased-receiver vtable signature to
   the concrete impl method). */
static uint32_t Driver_read__shim_PL011(const void *self) {
    return PL011_read((const PL011_t *)self);
}

/* Vtable drop-in-place shim for PL011 (synthesised per stage-12's C.2
   drop-shim rule; bridges the erased-receiver vtable drop slot to
   concrete `drop_in_place::<PL011>`). */
static void Driver__drop_shim_PL011(void *self) {
    PL011_drop_in_place((PL011_t *)self);
}

/* Vtable instance per stage-1's `<vtable for Driver for PL011>`. Both
   the drop slot and the method slot reference shims, because the vtable
   fn-pointer field types are erased-receiver throughout. */
extern const Driver_VTable_t __vtable_PL011_for_Driver;
const Driver_VTable_t __vtable_PL011_for_Driver = {
    .drop_in_place = Driver__drop_shim_PL011,
    .size          = sizeof(PL011_t),
    .align         = _Alignof(PL011_t),
    .read          = Driver_read__shim_PL011,
};

uint32_t process(trait_obj_Driver_t d) {
    return d.vtable->read(d.data);
}

/* Construction site for &pl011 as &dyn Driver: */
PL011_t pl011 = { /* ... */ };
trait_obj_Driver_t d = {
    .data   = (const void *)&pl011,
    .vtable = &__vtable_PL011_for_Driver,
};
uint32_t result = process(d);
```

The vtable struct typedef is emitted once per `(Trait,
projection_bindings)` (codegen tracks emission state). The
trait-object (fat-pointer) typedef is emitted once per `(Trait,
projection_bindings, pointer-kind)` because the four pointer-kind
variants (`&dyn`, `&mut dyn`, `*const dyn`, `*mut dyn`) carry
different `data`-field qualifications per the table in D5.3-B; a
single typedef cannot represent both `const void *data` and
`void *data` without collision. The vtable instance is one
`static const` per `(Trait, ConcreteImpl)`, with vtable shims in
two families -- one method shim per `(Trait, ConcreteImpl,
method)` plus one drop-in-place shim per `(Trait, ConcreteImpl)`
-- to bridge erased-receiver fn-pointer fields to concrete-receiver
targets (lifted impl methods or `drop_in_place::<SelfTy>`
respectively). The dispatch site loads `d.vtable->read` and
indirect-calls it with `d.data`.

The vtable shim, vtable instance, and stage-9/11 statement-level
mechanics are all pinned in their respective `expectations.md`
files.

### 4.10 Closure lowering (D6)

**Rust source.**

```rust
// A FnOnce-typed closure that consumes a non-Copy capture.
// `Token` has non-trivial Drop, so the synthesised captures-
// struct needs drop-glue. (`#![no_std]`-friendly: no alloc.)

struct Token { handle: u32 }

impl Drop for Token {
    fn drop(&mut self) { /* release the handle; details elided */ }
}

impl Token {
    fn into_handle(self) -> u32 {
        let h = self.handle;
        core::mem::forget(self);  // suppress Drop on consumption
        h
    }
}

fn make_consumer(t: Token) -> impl FnOnce() -> u32 {
    move || t.into_handle()
}

let c = make_consumer(Token { handle: 42 });
let value = c();
```

This closure is unambiguously `FnOnce`-typed: the body
`t.into_handle()` consumes `t` (by value, via `into_handle(self)`),
which forces `ClosureKind::FnOnce` (closures that consume any
capture by value cannot be `FnMut` or `Fn`). For an `Fn`-typed
closure that only reads its captures, all three call methods
would be materialised; for an `FnMut`-typed closure that mutates
without consuming, two would be.

**Stage-3 IR shape (after bridge construction).**

The closure is represented as:

A synthesised captures-struct Item (call its `ItemId` `c0_item`):
- DefPath: `crate::make_consumer::{closure#0}` (rustc DefPath
  verbatim; `DefPathData::Closure` segment with disambiguator
  `0`).
- Item kind: `Struct`.
- Fields: `[t: Ty::Struct(Token_item, [], layout_Token)]`
  (the captured upvar; rustc's `tupled_upvars_ty` resolves to
  `(Token,)`, flattened to one named field).
- Layout: per `tcx.layout_of`.

A synthesised Function Item for `call_once` (call its `ItemId`
`c0_call_once_item`):
- DefPath: `crate::make_consumer::{closure#0}::call_once`
  (parent = closure's DefPath; child segment
  `ValueNs(intern("call_once"))`, disambiguator `0`).
- Item kind: `Function`.
- Signature: `fn(Ty::Closure(c0_item, [])) -> u32` (substituted
  from `closure_sig_as_fn_ptr_ty`; receiver is the
  closure-as-struct *by value*, since FnOnce takes self). ABI
  is `FnAbi::Rust` per D6.1 bullet 4 (the synthesised call
  method's ABI is rewritten from rustc's source-level
  `RustCall` after the receiver/argument tuple normalization).

Because the closure is `FnOnce`-typed only, no `call_mut` or
`call` Function Items are synthesised.

A drop-glue Function Item is synthesised because the captured
`Token` has non-trivial drop. Synthesis details per stage-12
expectations.

`Ty` entries:
- `Ty::Closure(c0_item, [])` -- closure-type-identity view, call
  it `t_closure_c0`.
- `Ty::Struct { item: c0_item, args: [], layout: <closure
  captures struct layout> }` -- captures-struct view with
  layout, call it `t_struct_c0`. Reached from `t_closure_c0` via
  `model.types.find(&Ty::Struct { item: c0_item, args: [],
  layout: _ })`. The two entries are distinct in the BiMap per
  D3's `(variant_kind_tag, item, args)` identity rule (Section
  3.1) -- `Ty::Closure` and `Ty::Struct` differ in their
  variant tag.

`make_consumer`'s signature (after `impl Trait` resolution at
monomorphization): `fn(Ty::Struct(Token_item, [], layout_Token))
-> Ty::Closure(c0_item, [])`. Stored as a `FnSig` in
`model.sigs`.

**C output.**

```c
typedef struct Token {
    uint32_t handle;
} Token_t;

void Token_drop(Token_t *self) {
    /* release the handle; details elided */
}

uint32_t Token_into_handle(Token_t self) {
    uint32_t h = self.handle;
    /* core::mem::forget on the Rust side suppresses Drop;
       no Token_drop call is emitted at the consumption point. */
    return h;
}

typedef struct closure_make_consumer_0 {
    Token_t t;
} closure_make_consumer_0_t;

uint32_t closure_make_consumer_0__call_once(closure_make_consumer_0_t self) {
    return Token_into_handle(self.t);
    /* `self` is consumed by the caller; the call_once signature takes
       it by value, matching FnOnce semantics. */
}

void closure_make_consumer_0__drop(closure_make_consumer_0_t *self) {
    Token_drop(&self->t);
}

closure_make_consumer_0_t make_consumer(Token_t t) {
    return (closure_make_consumer_0_t){ .t = t };
}

closure_make_consumer_0_t c = make_consumer((Token_t){ .handle = 42 });
uint32_t value = closure_make_consumer_0__call_once(c);
/* `c` is consumed by call_once; no separate drop. */
```

The C struct typedef is emitted from `Ty::Struct(c0_item, [],
layout)` (the layout-bearing view); the C function is emitted
from the synthesised `call_once` Function Item; the drop
function from the synthesised drop-glue Function Item. Call
sites in MIR use `Ty::Closure(c0_item, [])` for type identity but
reach through to the corresponding Function Instance for
dispatch (stage-9/11 territory).

For an `FnMut`-typed closure (e.g., `let mut counter = 0; let
inc = move || { counter += 1; counter };`), the synthesis adds a
`call_mut` Function Item; `call_once` is also materialised
(inherited). The receiver type for `call_mut` is `&mut
Ty::Closure(...)`; for `call_once` it is `Ty::Closure(...)` by
value.

For an `Fn`-typed closure, all three call methods are
materialised, with receivers `&self` (`call`), `&mut self`
(`call_mut`), and `self` (`call_once`).

### 4.11 MMIO read via intrinsic (D9)

**Rust (driver-side):**

```rust
#[repr(C)]
struct UartFlagsReg(u32);

// In a real driver, `read::<UartFlagsReg, _>(...)` resolves to
// the intrinsic-flagged method on the MMIO façade
// (per pl011 fixture mmio.rs:35-71). Reduced here for
// readability.
#[rseq::intrinsic("mmio_read")]
fn mmio_read_u32(base: *mut u8, offset: usize) -> u32 {
    todo!("facade stub: lowered by the compiler backend")
}

fn poll_busy(base: *mut u8) -> bool {
    let flags = mmio_read_u32(base, 0x18);
    (flags & (1 << 5)) != 0
}
```

**IR (post-bridge, schematic).** Inside `poll_busy`'s body the
bridge produces a `Call` terminator targeting the `InstanceId`
for `mmio_read_u32` (the intrinsic). The call's operands and
result hold ordinary `TyId`s with no volatile annotation:

```
poll_busy:
  bb0:
    _1 = base                               // Ty: Ptr(U8, Mutability::Mut)
    _2 = const 0x18_usize                   // Ty: Uint(Usize)
    Call(_3 = mmio_read_u32(_1, _2),
         target: bb1)                       // _3 Ty: Uint(U32)
                                            //   ^ NO volatile bit anywhere
                                            //   ^ Call carries no `volatile: bool`
                                            //   ^ The InstanceId for
                                            //     mmio_read_u32 is the carrier
  bb1:
    _4 = BitAnd(_3, const 0x20_u32)         // Ty: Uint(U32)
    _0 = Ne(_4, const 0_u32)                // Ty: Bool
    Return
```

**C lowering (stage-14 emission, preview).**

```c
static inline bool poll_busy(uint8_t *base) {
    /* Stage-13 codegen for the mmio_read_u32 InstanceId emits a
       volatile-cast access. The volatility is a fact about THIS
       InstanceId's emission, not about any Ty in the IR. */
    uint32_t flags = *(volatile uint32_t *)(base + 0x18);
    return (flags & (1u << 5)) != 0;
}
```

**Rejected counter-example (raw `core::ptr::read_volatile`).**

```rust
fn poll_busy_bad(base: *mut u8) -> bool {
    // REJECTED at the bridge (Call-terminator translation,
    // stage 9 / stage 14 enforcement).
    let flags = unsafe {
        core::ptr::read_volatile(base.add(0x18) as *const u32)
    };
    (flags & (1 << 5)) != 0
}
```

The bridge raises a diagnostic naming `core::ptr::read_volatile`
as out-of-DSL-scope and pointing to the
`#[rseq::intrinsic("mmio_*")]` API as the canonical alternative.

**Observation.** No `Ty` in the model carries a volatile bit.
The volatility is recovered at codegen time from the
`InstanceId`'s identity, consistent with the Section 1.12
framing principle.

### 4.12 `Atomic<u32>` with acquire-load (D10)

**Regseq DSL `Atomic<T>` facade (illustrative; exact API is
stage-14 territory).** Mirrors the pl011 fixture's `mmio.rs`
convention -- intrinsic-flagged methods with `todo!()` bodies
that the compiler backend lowers per target:

```rust
// Conceptual sketch -- actual definition in regseq DSL crate.
#[repr(C)]
pub struct Atomic<T> {
    inner: UnsafeCell<T>,
}

pub enum Ordering { Relaxed, Acquire, Release, AcqRel, SeqCst }

impl<T> Atomic<T> {
    #[rseq::intrinsic("atomic_load")]
    pub fn load(&self, _order: Ordering) -> T {
        todo!("facade stub: lowered by the compiler backend")
    }

    #[rseq::intrinsic("atomic_store")]
    pub fn store(&self, _value: T, _order: Ordering) {
        todo!("facade stub: lowered by the compiler backend")
    }
    /* ... */
}
```

**Driver source.**

```rust
use regseq::{Atomic, Ordering};

fn read_count(counter: &Atomic<u32>) -> u32 {
    counter.load(Ordering::Acquire)
}
```

**IR (post-bridge, schematic).**

```
read_count:
  bb0:
    _1 = counter                    // Ty: Ref(Atomic_u32, Provenance::SharedBorrow)
    _2 = const Ordering::Acquire    // Ty: Enum (regseq::Ordering)
    Call(_3 = atomic_load(_1, _2),
         target: bb1)               // _3 Ty: Uint(U32)
                                    //   ^ NO atomic bit anywhere
                                    //   ^ Ty::Struct(regseq::Atomic, [u32])
                                    //     is an ordinary struct with an
                                    //     `is_unsafe_cell` flag on its inner
                                    //     UnsafeCell<u32> field (D1.5 -- which
                                    //     drops `const` on &Atomic<u32>)
                                    //   ^ The InstanceId for atomic_load is
                                    //     the carrier; ordering is an operand
  bb1:
    _0 = _3
    Return
```

**C lowering, target-aware (stage-14 emission, preview).**

The C-side `Atomic<u32>` lowers to a plain struct (no C11
`_Atomic`):

```c
typedef struct Atomic_u32 {
    uint32_t value;   /* the inner field, after UnsafeCell unwrapping per D1.5 */
} Atomic_u32_t;
```

*Linux (preserves Acquire ordering using the canonical Linux
non-RMW idiom for plain memory locations):*

```c
/* NO `const` qualifier on the parameter, because Atomic_u32_t
   transitively contains UnsafeCell<u32> and the locked D1.5
   `is_unsafe_cell` flag drops `const` on enclosing references
   (Section 1.6 D1.5 and Section 4.5 worked example). */
static inline uint32_t read_count(Atomic_u32_t *counter) {
    /* Stage-13 codegen for the atomic_load + Acquire InstanceId
       emits the Linux non-RMW Acquire idiom. */
    return smp_load_acquire(&counter->value);
}
```

Why `smp_load_acquire` rather than `atomic_read_acquire`:
`atomic_read_acquire` takes `const atomic_t *`, where
`atomic_t` is an opaque struct wrapping `int`; type-punning a
`const uint32_t *` to `const atomic_t *` is not valid in
general. The Linux convention for non-RMW Acquire loads on a
plain memory location is `smp_load_acquire`, which works on
plain primitive types.

*Zephyr (Zephyr's public API is SeqCst-only; codegen falls
through to GCC `__atomic_*` builtins to preserve Acquire):*

```c
static inline uint32_t read_count(Atomic_u32_t *counter) {
    return __atomic_load_n(&counter->value, __ATOMIC_ACQUIRE);
}
```

**Rejected counter-example (raw `core::sync::atomic::AtomicU32`).**

```rust
use core::sync::atomic::{AtomicU32, Ordering};

fn read_count_bad(counter: &AtomicU32) -> u32 {
    // REJECTED at the bridge (type-translation phase): AtomicU32
    // is in the core::sync::atomic::Atomic* type-family rejection
    // list. Diagnostic names `regseq::Atomic` as the canonical
    // alternative.
    counter.load(Ordering::Acquire)
}
```

**Observation.** No `Ty` carries an atomic bit.
`Ty::Struct(regseq::Atomic, [u32])` is structurally identical
to any other UnsafeCell-bearing struct; `Ty::Ref(...,
SharedBorrow)` correctly drops `const` via the existing D1.5
machinery. Memory ordering survives at the operation/InstanceId
level.

### 4.13 `repr(u32)` enum with user-assigned tags (D13, Direct encoding)

**Rust:**

```rust
#[repr(u32)]
pub enum Status {
    Idle   = 0,
    Active = 1,
    Error  = 0xFF,
    Reset  = 0xDEAD,
}

fn priority(s: Status) -> u32 {
    match s {
        Status::Idle   => 0,
        Status::Active => 1,
        Status::Error  => 100,
        Status::Reset  => 200,
    }
}
```

**Layout** (from `tcx.layout_of(Status)` plus
`status_adt_def.discriminants(tcx)`):

```text
size = 4, align = 4, pack = None, uninhabited = false
kind = Enum {
    shape: Multiple {
        tag: TagScalar {
            offset:      0,
            scalar_ty:   Uint(U32),
            valid_range: WrappingRange { start: 0, end: 0xDEAD },
        },
        encoding: Direct,
        variants: [
            VariantLayout { name: "Idle",   fields: [], uninhabited: false, discriminant: 0,      tag_value: Some(0)      },
            VariantLayout { name: "Active", fields: [], uninhabited: false, discriminant: 1,      tag_value: Some(1)      },
            VariantLayout { name: "Error",  fields: [], uninhabited: false, discriminant: 0xFF,   tag_value: Some(0xFF)   },
            VariantLayout { name: "Reset",  fields: [], uninhabited: false, discriminant: 0xDEAD, tag_value: Some(0xDEAD) },
        ],
    },
}
```

For Direct-encoded enums, `discriminant` and `tag_value`
coincide on every variant: the bridge copies `discriminant`
from `status_adt_def.discriminants(tcx)` (the resolved
`Discr.val: u128`), and sets `tag_value = Some(discriminant)`
(direct encoding writes the discriminant verbatim). Variant
index (`0..=3`) and discriminant value (`0`, `1`, `0xFF`,
`0xDEAD`) are independent.

**C lowering:**

```c
typedef struct Status {
    uint32_t tag;
} Status_t;

#define STATUS_IDLE   0u
#define STATUS_ACTIVE 1u
#define STATUS_ERROR  0xFFu
#define STATUS_RESET  0xDEADu

uint32_t priority(Status_t s) {
    switch (s.tag) {
        case STATUS_IDLE:   return 0;
        case STATUS_ACTIVE: return 1;
        case STATUS_ERROR:  return 100;
        case STATUS_RESET:  return 200;
    }
}

// SetDiscriminant(Status::Error) -> place->tag = STATUS_ERROR;
//   codegen reads VariantLayout.tag_value = Some(0xFF) and emits
//   0xFF as the tag value, NOT 2 (variant index).
// Discriminant(place: &Status) -> reads place->tag and matches
//   against per-variant VariantLayout.discriminant to produce
//   the Rust-level semantic value.
```

Without `VariantLayout.tag_value`, codegen would have no basis
for emitting `0xFF` -- it would need to maintain a separate
per-build discriminant table or re-query rustc (impossible
post-bridge).

### 4.14 Niche-encoded `Option<&u32>` with discriminant fields (D13)

**Rust:**

```rust
fn f(x: Option<&u32>) -> u32 {
    match x {
        Some(p) => *p,
        None    => 0,
    }
}
```

**Layout** (`tcx.layout_of(Option<&u32>)` plus the D13
discriminant fields; restating Section 4.4 with the new
fields):

```text
size = 8, align = 8, pack = None, uninhabited = false
kind = Enum {
    shape: Multiple {
        tag: TagScalar {
            offset:      0,
            scalar_ty:   Ptr(U32, Mutability::Not),
            valid_range: WrappingRange { start: 1, end: u64::MAX as u128 },
        },
        encoding: Niche {
            untagged_variant: Some,
            niche_variants:   None..=None,
            niche_start:      0,
        },
        variants: [
            // Variant index 0: None. Niched -- inner data forced to the niche.
            VariantLayout {
                name:         "None",
                fields:       [],
                uninhabited:  false,
                discriminant: 0,             // semantic discriminant (variant idx)
                tag_value:    Some(0),       // resolved tag bit pattern (the niche)
            },
            // Variant index 1: Some(p: &u32). Untagged -- the inner pointer IS the tag.
            VariantLayout {
                name:         "Some",
                fields:       [FieldLayout { name: "0", ty: Ref(U32, Provenance::SharedBorrow), offset: 0 }],
                uninhabited:  false,
                discriminant: 1,             // semantic discriminant (variant idx)
                tag_value:    None,          // no separate tag write occurs
            },
        ],
    },
}
```

The split is load-bearing for niche-encoded enums.
`Discriminant(place: &Option<&u32>)` returns `0` for `None`,
`1` for `Some` -- codegen reads the inner pointer, matches
against the niche bit pattern (`0`) to identify `None`, and
produces `discriminant` accordingly. `SetDiscriminant(Option::None)`
writes the niche bit pattern (`tag_value = Some(0)`) into the
inner pointer field. `SetDiscriminant(Option::Some)` is a
**no-op for the tag**: `tag_value = None` means there is no
separate tag write, since the `Some` variant is the untagged
variant whose identity is conveyed by the payload itself.
Construction of `Some(p)` writes `p` through the **normal
payload-assignment path** (an ordinary field-write statement
on the inner pointer), not through `SetDiscriminant`. The two
operations -- `SetDiscriminant(Some)` and the field-write of
`p` -- happen via separate MIR statements in the construction
sequence.

**C lowering** (consistent with Section 4.4):

```c
typedef struct Option_ref_u32 {
    const uint32_t *p;   // 0 = None; nonzero = Some(p)
} Option_ref_u32_t;

uint32_t f(Option_ref_u32_t x) {
    if (x.p == NULL) return 0;        // None: tag value 0 (the niche)
    return *x.p;                      // Some: any non-null pointer
}

// SetDiscriminant(Option::None) -> place->p = (const uint32_t *)0;
//   /* tag_value = Some(0): writes the niche bit pattern. */
// SetDiscriminant(Option::Some) -> /* no-op */
//   /* tag_value = None: untagged variant; payload IS the tag. */
// Construction of Option::Some(p) -> place->p = p;
//   /* Separate MIR field-write statement, NOT a SetDiscriminant. */
// Discriminant(place: &Option<&u32>) -> reads place->p; if NULL produce
//   semantic discriminant 0 (None.discriminant); else 1 (Some.discriminant).
```

The two-field shape correctly distinguishes the two senses of
"discriminant" for niche encoding. The redundancy with D12's
`niche_variants` / `niche_start` is intentional and
load-bearing for different codegen aspects (see D13 reasoning
in Section 2.3).

### 4.15 Rejected `*mut c_void` usage (D16)

**Driver source (rejected counter-example):**

```rust
use core::ffi::c_void;

#[rseq::intrinsic("opaque_callback_register")]
fn register_callback(cb: extern "C" fn(*mut c_void), ctx: *mut c_void) {
    todo!("hypothetical -- not in v1 stage-14 intrinsic vocabulary")
}
```

The bridge rejects this driver code at type translation:

- When translating the `*mut c_void` parameter type, the bridge
  encounters the `c_void` ADT.
- The bridge runs `tcx.is_lang_item(c_void_def_id,
  LangItem::CVoid)`; returns `true`.
- D16's rejection fires: the bridge raises a diagnostic naming
  `core::ffi::c_void` as out-of-DSL-scope in v1, and recommends
  the opaque-struct pattern as the canonical alternative.

**Suggested rewrite (per D1.4 / D16 diagnostic):**

```rust
#[repr(C)]
struct OpaqueCtx {
    _private: [u8; 0],
}

#[rseq::intrinsic("opaque_callback_register")]
fn register_callback(cb: extern "C" fn(*mut OpaqueCtx), ctx: *mut OpaqueCtx) {
    todo!("hypothetical -- not in v1 stage-14 intrinsic vocabulary")
}
```

The opaque-struct pattern is structurally an `Adt` (an ordinary
`Ty::Struct`) and is supported by every project that handles
ADTs. C lowering of `*mut OpaqueCtx` is a forward-declared
struct pointer (`struct OpaqueCtx *`), a valid opaque-handle
shape on both Linux and Zephyr targets. (The
`register_callback` intrinsic itself is hypothetical -- v1
stage-14 has no such intrinsic in its closed vocabulary; the
example is illustrative of the type-translation rejection
only.)

**Observation.** The rejection is at *type translation*, not
at Call translation: any use of `c_void` -- as a parameter
type, return type, struct field type, generic argument, or the
pointee of a raw pointer -- triggers the rejection. Structurally
identical to D1.4's `Foreign(_)` rejection (also
type-translation-phase) and contrasts with D9's `*_volatile`
rejection (Call-translation-phase) and D10's mixed rejection
(type-translation for `Atomic*`, Call-translation for methods).

---

## 5. Open decisions

Status table for the post-D1 decisions (D3-D16). Locked entries
point at the spec section that records the decision; open entries
carry a one-line lean for context. Filled in as discussion
progresses.

| Tag | Subject | Status |
| --- | --- | --- |
| D3 | Interning strategy (structural / arena / global hash-cons) | **LOCKED** -- structural BiMap with `TyId` newtype (Section 3.1) |
| D4 | Interning location (bridge-side / IR-side) | **LOCKED** -- `RegseqModel.types: TyInterner`, mirroring stage-1 `StringInterner` (Section 3.2) |
| D5 | `Dynamic` lowering (abstract vs early-materialized fat ptr) | **LOCKED** -- `Ty::Dynamic { auto_traits, vtable_ty }`; `vtable_ty` resolves to a `Ty::Struct` whose Item details are pinned in `stage-12/expectations.md`; fat-pointer codegen-emitted (D5.3-B); statement-level dispatch rewriting deferred to stage 11 (Section 1.10) |
| D6 | Closure naming (`closure#N` DefPath segment, etc.) | **LOCKED** -- one captures-struct Item + 1-3 call-method Function Items + drop-glue iff needed; rustc DefPath verbatim; `Ty::Closure` and corresponding `Ty::Struct` share an Item; synthesis details in `stage-12/expectations.md` (Section 1.11) |
| D7 | Provenance enum shape (Shared/Unique/Raw + advisory bits?) | **LOCKED** -- two variants `{ SharedBorrow, UniqueBorrow }` on `Ty::Ref`; `Raw` is encoded by the variant choice `Ty::Ptr`; advisory bits dropped (Section 1.8) |
| D8 | `Ty::Ref` carries provenance enum -- exact set | **LOCKED** -- `Ref(TyId, Provenance)`; `Mutability` field dropped (was redundant); `Ty::RawPtr` renamed to `Ty::Ptr` (Section 1.8) |
| D9 | Volatile per-op vs per-type | **LOCKED** -- no `Ty` bit; volatile is carried on the intrinsic `InstanceId` (no IR-level flag at any level); raw `core::ptr::{read,write}_volatile` rejected at the bridge (Section 1.13) |
| D10 | Atomics as intrinsics | **LOCKED** -- no `Ty` bit; atomic types are ordinary `Ty::Struct` with the existing `is_unsafe_cell` flag on inner `UnsafeCell<T>`; atomic operations are stage-14 intrinsics on the regseq-DSL `Atomic<T>` facade; bridge rejects `core::sync::atomic::Atomic*` types (type-translation) and their methods + `core::intrinsics::atomic_*` (Call-translation); `core::sync::atomic::Ordering` (value-level enum) explicitly *not* rejected; barrier/fence free functions in `core::sync::atomic` (`fence`, `compiler_fence`) are **rejected at the bridge in v1** (Call-translation phase) per the same rejection-pattern symmetry; admission of fence/barrier semantics waits on a future construct-level decision (Section 1.14) |
| D11 | Layout content subset of `LayoutData` | **LOCKED** -- subset table at Section 2.2 |
| D12 | Niche optimization preservation | **LOCKED** -- formal via `TagEncoding::Niche` (Section 2.2) |
| D13 | Discriminant level (typeck vs layout; per-variant values) | **LOCKED** -- discriminant *type* settled by D11 (`TagScalar.scalar_ty`); per-variant discriminant *values* added via two new fields on `VariantLayout`: `discriminant: u128` (Rust-level semantic discriminant, always present per `adt_def.discriminants(tcx)`) and `tag_value: Option<u128>` (layout-level tag bytes for `SetDiscriminant`; `None` for niche-encoded untagged variants and for `EnumShape::Single` variants). Refines D11's Section 2.2 schema by two fields (Section 2.3). |
| D14 | (DEFERRED to stage 13 -- type-system pretty-printing; see `../stage-13/expectations.md`) | DEFERRED |
| D15 | fn-pointer ABI subset (`Rust`, `C`, `System`, ...) | **LOCKED** -- `FnAbi { Rust, C }` v1; all other ExternAbi variants (including `C { unwind: true }`) bridge-rejected with `BridgeError::UnsupportedAbi` (Section 1.9) |
| D16 | `c_void` recognition (rejected at bridge in v1) | **LOCKED** -- bridge rejects `core::ffi::c_void` at type translation in v1 (recognition via `tcx.is_lang_item(did, LangItem::CVoid)` per D1.5); diagnostic recommends opaque-struct pattern (per D1.4 precedent for opaque C-side handles); D1.5's "no flag" lock carries forward unchanged. Reopens when a v1 driver fixture or stage-14 intrinsic surfaces a stable need for FFI-canonical pointer-to-c_void lowering (`*mut c_void` -> `void *`; `*const c_void` -> `const void *`) (Section 1.15) |

---

## 6. References

### 6.1 In-repo cross-references

- **Methodology rules.** `../README.md` "Methodology rules"
  section. Each decision above cites the rules invoked.
- **Architectural decisions.** `../architecture.md` Q1-Q7.
- **Stage-1 spec.** `../stage-1/spec-snapshot.md`. Stage 3 fills in
  the `TyId` forward declaration and the `Ty::Struct/Enum/Union`
  scaffolding stage 1 anticipated.
- **Stage-3 incoming forward refs.** `expectations.md` in this
  folder. As D-decisions land, items there are marked resolved.

### 6.2 Stage-3 research files

- `findings-rustc-draft.md` -- empirical evidence from rustc's
  own type system. Themes A (vocabulary), B (layout), C (interning),
  D (lang-item recognition), E (post-monomorphization erasure).
- `prior-art/rustc-codegen-c.md` -- direct C-emitting Rust
  consumer; tiny vocabulary; informs simplicity baseline.
- `prior-art/mrustc.md` -- Rust-to-C (C++); 15-variant TypeData;
  layout-on-decl pattern; fat-pointer C structs; lang-item
  recognition via path identity.
- `prior-art/compcert.md` -- formally verified C compiler;
  three-vocabulary collapse pattern; `attr_volatile` choice we
  declined; `composite` records inform layout structure.
- `prior-art/charon.md` -- Rust MIR consumer for verifiers;
  14-variant TyKind; `BuiltinTy::Box`/`::Str` outlier;
  hash-consed handles.
- `prior-art/llvm-ir.md` -- 22-variant TypeID; opaque-pointer
  migration lesson (don't bake target info into types when it can
  live on operations); DataLayout as separate concern.
- `prior-art/mlir.md` -- 14-variant builtin + open vocabulary;
  Dialect Conversion + TypeConverter pattern (informs eventual
  bridge → C-codegen lowering structure).
- `prior-art/rustc-codegen-clr.md` -- direct precedent for
  "one IR, two printers"; explicit-layout-in-IR posture; informs
  D2 lock.
