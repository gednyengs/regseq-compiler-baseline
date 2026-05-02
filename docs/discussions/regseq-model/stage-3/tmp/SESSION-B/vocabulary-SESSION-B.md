# SESSION-B Addendum to `vocabulary.md` (D7, D8, D15)

This file holds the SESSION-B contributions to stage 3's `vocabulary.md`,
written in the full audit shape (Decision / Reasoning / Methodology
rules / Prior-art evidence / Worked example) so the merge agent can
lift each block directly into the shared spec without reformatting.

Scope: locks D7, D8, D15 in the open-decisions table (Section 5),
introduces the `Provenance` enum and renames `Ty::RawPtr` to `Ty::Ptr`
in the variant table (Section 1.1), introduces the `FnAbi` enum, and
extends the worked C-lowering examples (Section 4.1) with the bridge
translation rule and an edge-cases table.

---

## 0. Merge instructions for the integration session

These notes coordinate the lift-in. Foreign-quoted references
(rustc's `TyKind::RawPtr`, Charon's `RawPtr` variant, etc.) must
**not** be renamed -- they describe other projects' types.

### 0.1 Rename sweep: `Ty::RawPtr` -> `Ty::Ptr`

The rename pairs `Ty::Ref` (borrow) with `Ty::Ptr` (raw pointer)
symmetrically. SESSION-B applies the rename only to **our-IR**
occurrences. The merge agent must sweep the shared docs:

| File | Our-IR occurrences (rename) | Foreign-quoted (leave as-is) |
| --- | --- | --- |
| `vocabulary.md` | ~13 sites: Section 1 row, Section 4.1 worked example, Section 3 interning walkthrough (lines around 661, 912, 923-924, 965, 977, 1296, 1494), Section 4.4 self-ref ADT example. **Plus, on the `Ref` arity drop**: Section 1.6 D1.6.a (lines 510-514) -- replace `Ref(_, Str, _)` with `Ref(Str, _)` (where the trailing `_` is now a `Provenance` slot) and `RawPtr(Int(I8), _)` with `Ptr(Int(I8), _)` to match the new arity; **Section 3.3 TyInterner construction walkthrough** (lines 1442, 1444, 1452, 1453, 1465) -- replace the 3-tuple `Ty::Ref(TyId(0), Mutability::Mut, Provenance::UniqueBorrow)` and shorthand `Ty::Ref(0, Mut, Unique)` with the new 2-tuple `Ty::Ref(TyId(0), Provenance::UniqueBorrow)` / `Ty::Ref(0, Unique)` throughout the walkthrough. | -- |
| `discussions.md` | ~6 sites: Section 1 variant lists, D5/D7/D8 entries, Section 1 normalization (lines around 33, 205, 303, 401, 472, 908) | -- |
| `review-log.md` | ~6 sites: Round 2 sub-round narratives (lines around 780, 782, 800, 901, 979) | -- |
| `findings-rustc-draft.md` | none | `TyKind::RawPtr` -- rustc's variant name; keep |
| `prior-art/charon.md` | none | `RawPtr` -- Charon's variant name; keep |
| `expectations.md` | open D7/D8 entry at lines 65-69 ("Pointer / reference provenance field" marked `[open]`) -- **mark resolved** to `Provenance := { SharedBorrow, UniqueBorrow }`; the lean's "three-variant baseline" is replaced by the locked two-variant set (with `Raw` encoded by the `Ty::Ptr` variant choice). | -- |

The bridge code (currently `regseq-driver-old/`) and any future
`regseq-driver`/`regseq-model` Rust source are out of scope for this
sweep -- they pattern-match against rustc's symbols, not ours.

### 0.2 Section placements

- **Section 1.1 variant table** -- replace the `RawPtr` and `Ref` rows
  per Section A below.
- **Section 1 new subsections** -- insert two new subsections after the
  existing Section 1.7 (D1.6 auxiliary variant decisions). The merge
  agent assigns the final section numbers (e.g., 1.8 for D7/D8, 1.9
  for D15) depending on integration order with other parallel
  sessions. This addendum uses Section 1.D7-D8 and Section 1.D15 as
  placeholders; rename on lift-in.
- **Section 4.1** -- replace the Rust/C worked-example block per
  Section C below; append the bridge-translation edge-cases table as
  a new subsection 4.1.1 (or whatever sub-numbering convention the
  merge agent prefers).
- **Section 5 open-decisions table** -- replace the D7, D8, D15 rows
  per Section D below.

---

## A. Section 1.1 variant-table updates

Replace these rows of the variant table (lines around 60-73 of the
current `vocabulary.md`):

| Variant | Carries | C-side intent |
| --- | --- | --- |
| `Str` | -- (marker) | only ever behind `Ref`/`Ptr`; lowered to fat pointer |
| `Slice(TyId)` | element type | only ever behind `Ref`/`Ptr`; lowered to fat pointer |
| `Ptr(TyId, Mutability)` | pointee + mutability | `T*` / `const T*` |
| `Ref(TyId, Provenance)` | pointee + borrow flavor | `T*` / `const T*` / `T* restrict` per provenance |
| `FnPtr(FnSigId)` | signature handle (carries `FnAbi`) | C function pointer `T (*)(args)` |

Also replace the closing paragraph (lines 75-76 of the current spec):

> The `Provenance` enum, the `FnAbi` enum on `FnSig`, the exact
> field-list shape, and layout content are pinned in subsequent
> decisions (D7/D8, D11, D15).

(Removed: "The `Mutability`, `Provenance`, ..." -- `Mutability` no
longer appears as a separate field on `Ref`; it remains on `Ptr` as
a pass-through of `rustc_ast_ir::Mutability`.)

---

## B. New Section 1.D7-D8 -- Provenance enum on `Ref`

### 1.D7-D8 Decision D7/D8 -- Provenance enum on `Ref`; `Ty::Ptr` shape

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

The `_region` slot in `TyKind::Ref` is discarded by the bridge
regardless of variant. Post-erasure, the slot is `ReErased` for free
regions (the typical case for MIR-local types after
`tcx.instantiate_mir_and_normalize_erasing_regions(...)`) and
`ReBound` for late-bound regions inside `Binder<FnSig>` /
`Binder<Predicate>` and `Dynamic` existential-predicate scopes (see
`findings-rustc-draft.md:675` and `:695-697`). No region survives
in `Ty::Ref`; the bridge does not record which `RegionKind` variant
the slot held.

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
  is on the stage-3-deferred list (`stage-3-status.md`). Moot.
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

---

## C. Section 4.1 replacement -- Reference and pointer types

Replace the current Section 4.1 body (lines around 1527-1551) with:

````
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
methodology rule (`../context.md` "C semantics with information
preservation"); it is not added to shared `&T` references or to raw
pointers regardless of `UnsafeCell` presence.
````

Then append a new subsection 4.1.1:

````
### 4.1.1 Bridge translation: edge cases

The bridge translation rule for `Ref` / `Ptr` is uniform; no
pointer-specific recursion logic is required. The pointee is
recursively translated and interned per D4. The table below shows
the rule's behavior on common edge cases:

| Source | IR |
| --- | --- |
| `&str` | `Ty::Ref(Ty::Str, SharedBorrow)` -- fat pointer materialized by codegen, see Section 4.2 |
| `&[T]` | `Ty::Ref(Ty::Slice(translate(T)), SharedBorrow)` |
| `&dyn Trait` | `Ty::Ref(Ty::Dynamic(...), SharedBorrow)` -- `Dynamic` shape pending D5 |
| `&&u32` | `Ty::Ref(Ty::Ref(Ty::Uint(U32), SharedBorrow), SharedBorrow)` |
| `*const *mut u32` | `Ty::Ptr(Ty::Ptr(Ty::Uint(U32), Mut), Not)` |
| `&!` | `Ty::Ref(Ty::Never, SharedBorrow)` -- accepted at type level; codegen may reject when emitted |
| `*const ()` | `Ty::Ptr(Ty::Tuple(empty), Not)` |
| `&UnsafeCell<u32>` | `Ty::Ref(<UnsafeCell<u32>>, SharedBorrow)` -- the `is_unsafe_cell` flag on the `UnsafeCell<u32>` `Item` is what triggers the `const`-drop rule |

Invariants the rule guarantees:

1. No `Provenance` value outside `{ SharedBorrow, UniqueBorrow }`
   can result from translating a rustc `TyKind::Ref` (rustc's
   `Mutability` enum is exhaustive).
2. No `Ty::Ref` carries a region; the `_region` slot in
   `TyKind::Ref` is consumed and discarded regardless of whether
   it is `ReErased` (free) or `ReBound` (late-bound, inside a
   binder).
3. The translation is total over rustc's `TyKind::Ref` /
   `TyKind::RawPtr` (no panic, no diagnostic, no rejection -- both
   variants are always accepted).
4. The translation is deterministic.
````

---

## D. Section 5 open-decisions table -- updates

Replace the D7, D8, D15 rows in Section 5 (lines around 1791-1799)
with:

| Tag | Subject | Status |
| --- | --- | --- |
| D7 | Provenance enum shape (Shared/Unique/Raw + advisory bits?) | **LOCKED** -- two variants `{ SharedBorrow, UniqueBorrow }` on `Ty::Ref`; `Raw` is encoded by the variant choice `Ty::Ptr`; advisory bits dropped (Section 1.D7-D8) |
| D8 | `Ty::Ref` carries provenance enum -- exact set | **LOCKED** -- `Ref(TyId, Provenance)`; `Mutability` field dropped (was redundant); `Ty::RawPtr` renamed to `Ty::Ptr` (Section 1.D7-D8) |
| D15 | fn-pointer ABI subset (`Rust`, `C`, `System`, ...) | **LOCKED** -- `FnAbi { Rust, C }` v1; all other ExternAbi variants (including `C { unwind: true }`) bridge-rejected with `BridgeError::UnsupportedAbi` (Section 1.D15) |

---

## E. New Section 1.D15 -- fn-pointer ABI subset

### 1.D15 Decision D15 -- fn-pointer ABI subset (`FnAbi` enum)

**Decision.** A new enum `FnAbi` is added to the stage-3 type-system
vocabulary:

```rust
pub enum FnAbi {
    Rust,    // default Rust ABI; rustc's ExternAbi::Rust
    C,       // extern "C"; rustc's ExternAbi::C { unwind: false }
}
```

**Storage.** `FnAbi` is a structural field of `FnSig` (whose full
shape is stage-11 territory; `stage-1/vocabulary.md` line 76 reserves
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

   - *Pretty-printer (D14).* Round-trippable rendering of
     `extern "C" fn(...)`; consumes the full enum.
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
   `stage-1/vocabulary.md:77`) determines whether
   `#[no_mangle]` / `#[export_name]` apply; the default mangled
   name is `Instance.symbol_name = tcx.symbol_name(...)` (see
   `stage-1/vocabulary.md:670-673`). `extern "C" fn foo()`
   without `#[no_mangle]` still receives a mangled symbol --
   the ABI does not control mangling. Routing the
   mangled-vs-unmangled decision through `FnAbi` would be wrong;
   it belongs to the linkage layer, not the type layer.

The minimal variant set for v1 is therefore driven by the
type-identity argument: which ABIs appear in code we accept? For
Linux v1 driver code:

- `Rust`: pervasive (default for any non-extern function).
- `C`: pervasive (FFI to kernel APIs).
- `System`, `Unadjusted`, `RustCall`, ...: rare or never in driver
  code. `System` resolves to `C` on Linux. `Unadjusted` is
  compiler-internal. `RustCall` is the closure-trait method ABI
  (D6 territory).

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
`extern "Rust" fn(...)` vs `extern "C" fn(...)` (D14), (c) the
future FFI-validity linter's "is this `FnAbi::C`?" predicate
(deferred consumer), and (d) future calling-convention attribute
emission when a non-collapsing target is added. A future
cross-platform port keeps the field for emission of
`__attribute__((<conv>))` on targets where the source ABI does
not collapse (e.g., Windows-x86 `System` -> stdcall).

**Stage-11 forward note.** Stage 11 embeds `FnSig::abi: FnAbi`. The
field name is fixed by D15; stage 11 wires the body of `FnSig`
around it.

---

## F. SESSION-B summary

| D-tag | Status | Vocabulary impact |
| --- | --- | --- |
| D7 | LOCKED | Provenance enum `{ SharedBorrow, UniqueBorrow }`; advisory bits dropped |
| D8 | LOCKED | `Ref(TyId, Provenance)`; `RawPtr` renamed to `Ptr(TyId, Mutability)`; rename sweep flagged for merge agent |
| D15 | LOCKED | `FnAbi { Rust, C }`; bridge hard-rejects all other ExternAbi variants including `C { unwind: true }` |

Three new structural elements introduced:
- `Provenance` enum (Section 1.D7-D8).
- `FnAbi` enum (Section 1.D15).
- Bridge-translation edge-cases table (Section 4.1.1).

Two existing elements modified:
- Section 1.1 variant table: `Ref` and `Ptr` rows replaced.
- Section 4.1 worked example: shape adjusted to drop `Mutability`
  on `Ref` and reflect the rename.
