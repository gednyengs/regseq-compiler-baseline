# SESSION-A vocabulary addendum -- D5, D6

This document captures the Phase-B spec text for stage-3 decisions
**D5 (`Dynamic` lowering)** and **D6 (closure naming)**, in the
canonical audit shape used by `vocabulary.md` (Decision /
Reasoning / Methodology rules invoked / Prior-art evidence /
Worked example). The merge session splices the per-decision
blocks into the corresponding sections of `vocabulary.md`.

SESSION-A scope is **stage-3 commitments only**. Design analysis
that pertains to stages 9, 11, and 12 (vtable shapes, dispatch
rewriting, synthesised-Item synthesis details) is forwarded to
those stages' `expectations.md` files; see
`discussions-SESSION-A.md` Section 4 for the forward-expectation
text and the merge-session filing instructions.

Plus: one wording correction to existing `vocabulary.md` text
(`vocabulary.md:1689-1697`, post-D7/D8-merge line numbers) that
is superseded by D5.3-B, and the
delta to Section 5 (Open decisions table) and Section 1 (variant
table).

---

## 0. Splice instructions for the merge session

| SESSION-A block | Lands in `vocabulary.md` section |
| --- | --- |
| Section 1 (D5: `Ty::Dynamic` content) | Insert as new **Section 1.8** after the current Section 1.7 (D1.6 auxiliary variant decisions). Title: "1.8 Decision D5 -- `Dynamic` lowering". |
| Section 2 (D6: closures) | Insert as new **Section 1.9** after the new Section 1.8. Title: "1.9 Decision D6 -- Closure naming and synthesis". |
| Section 3 (Worked examples) | Insert as Section 4.9 (`&dyn Driver` lowering) and Section 4.10 (closure lowering) after the current Section 4.8. |
| Section 4 (Open decisions table delta) | Replace the D5 and D6 rows in `vocabulary.md` Section 5. |
| Section 5 (Wording correction) | Replace the parenthetical at `vocabulary.md:1689-1697` per the text in this document's Section 5. |

The Section 1 variant table at `vocabulary.md:51-74` also requires
two row edits, captured in this document's Section 4.

The forward-expectations from `discussions-SESSION-A.md` Section
4 are filed by the merge session into
`stage-9/expectations.md`, `stage-11/expectations.md`, and
`stage-12/expectations.md`.

---

## 1. D5 -- `Dynamic` lowering [LOCKED]

D5 has three sub-decisions: D5.1 (predicate content of
`Ty::Dynamic`), D5.2 (where the lowering work happens), D5.3
(IR products of the lowering, split into D5.3-A vtable type
contract and D5.3-B fat-pointer C-shape policy). All four are
locked here at the stage-3 level; downstream synthesis details
are forwarded to `stage-12/expectations.md` per
`discussions-SESSION-A.md` Section 4.

### 1.1 D5.1 -- Predicate content of `Ty::Dynamic`

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
`stage-12/expectations.md` (see `discussions-SESSION-A.md`
Section 4.3). Stage-3 commits only to (a) the slot exists,
(b) it is of type `TyId`, (c) it is filled by the bridge at
type-translation time, (d) it resolves to a `Ty::Struct` value.

HRTB binders (e.g., `dyn for<'a> Trait<&'a T>`) are erased
uniformly along with all other regions. The bridge constructs the
same `Ty::Dynamic` shape regardless of whether the source had an
HRTB binder or not -- the inner `&'a T` becomes `Ty::Ref(T_id,
Provenance::SharedBorrow)` (which has no region slot per locked
D7/D8 at `vocabulary.md:570-601`), and the binder leaves no IR
trace.

Custom auto-traits (declared via unstable `feature(auto_traits)`)
are rejected by the bridge with a clear diagnostic. The closed v1
set is `Send` / `Sync` / `Unpin`; widening is a local v2 change
(extend the enum + lift the bridge rejection).

**Reasoning.**

1. *Region erasure as a RegseqModel-wide invariant.* No `Ty`
   variant carries any region slot (see `discussions-SESSION-A.md`
   Section 3 region-erasure primer). HRTB binders survive in
   rustc's IR per `findings-rustc-draft.md` Theme E.6 because
   rustc has multiple consumers (borrowck, miri, codegen) with
   different needs; for our IR (post-typecheck, C-facing, one
   consumer family) the binder carries no semantically meaningful
   information. Type correctness for HRTB obligations was
   verified pre-MIR by rustc; we don't re-verify.
2. *Trait declarations are erased per stage-1.* Stage 1 erases
   trait declarations (`stage-1/vocabulary.md:608-611`) and
   associated types (`stage-1/vocabulary.md:605-607`); the closed
   six-variant `ItemKind` (`stage-1/vocabulary.md:625-627`) does
   not include `Trait` or `AssocType`, so traits and associated
   types have no `ItemId`. The trait identity must therefore flow
   through a different carrier. Stage-1 reserved the
   bridge-synthesised vtable Static Item per `(Trait, SelfTy)`
   (`stage-1/vocabulary.md:910-927`) plus an associated
   `def_path_hash` keyed on `(trait_def_path_hash,
   self_ty_canonical_hash)`. Stage-3 commits to a `vtable_ty:
   TyId` link from `Ty::Dynamic` to the (synthesised) vtable
   `Ty::Struct`; the synthesis machinery (vtable type Item,
   vtable instance Item, vtable shim Function Items, the
   `RegseqModel.vtables` dedup table) is detailed in
   `stage-12/expectations.md` per the forwarded design analysis.
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
- "Don't redo work the rustc compiler has already done" --
  HRTB obligations were verified pre-MIR; we do not re-verify.
- "No defensive fallbacks" -- no user-defined-auto-trait support;
  no HRTB-rejection guard for hypothetical scenarios; both reject
  cleanly via the closed v1 vocabulary.
- *Region erasure as a RegseqModel-wide invariant* (see
  `discussions-SESSION-A.md` Section 3) -- HRTB binders are part
  of the region material we erase.

**Prior-art evidence.**

- *rustc* (`findings-rustc-draft.md` Theme A.4): rustc's
  `TyKind::Dynamic(BoundExistentialPredicates, Region)` carries
  predicates, projections, auto-traits, and an outer region. We
  diverge by erasing the region (consistent with our wider
  region-erasure invariant) and routing principal+projections
  through `vtable_ty` rather than carrying them inline.
- *Charon* (`prior-art/charon.md` F.3): Charon's
  `TyKind::DynTrait(DynPredicate)` similarly carries a binder
  over predicates. Charon retains the binder because its verifier
  consumers want it; for our C-facing consumer the binder is
  inert.

**Worked example.** See Section 3.1 below (`&dyn Driver`).

### 1.2 D5.2 -- Distribution of lowering work

**Decision.** D5 has no monolithic "lowering pass". The work
distributes across four layers, each in its natural stage:

| Layer | Stage | Deliverable |
| --- | --- | --- |
| Type-level: `Ty::Dynamic` shape and `vtable_ty` slot | 3 (this SESSION) | `Ty::Dynamic { auto_traits, vtable_ty }` per D5.1 |
| Type-level: vtable `Ty::Struct` synthesis machinery (Item, layout, dedup table) | 12 | Per-(Trait, projections) vtable type Item; per-(Trait, SelfTy) vtable instance Item; per-(Trait, ConcreteImpl, method) vtable shim Function Items; `RegseqModel.vtables` dedup table; concrete DefPath / hash rules; layout construction algorithm. See `discussions-SESSION-A.md` Section 4.3. |
| Statement-level: `Call` terminator shape supporting vtable-indirect dispatch | 9 | Stage-9 picks between two design candidates -- a public `Instance` discriminator addition (parallel to rustc's `Virtual` `InstanceKind`, requiring a stage-1 revision per `stage-1/vocabulary.md:747-754` since `fn_shim_disambig` is private) or a widened `Call.callee` shape (`Direct(InstanceId)` / `Indirect(Operand)`). See `discussions-SESSION-A.md` Section 4.1. |
| Statement-level: dispatch rewriting pass | 11 | Body-level pass that walks `Call` terminators, rewrites `<dyn Trait as Trait>::method` into vtable-indirect form (per stage-9), and recognises `Cast::Unsize` rvalues to trigger stage-12 vtable instance Item registration. See `discussions-SESSION-A.md` Section 4.2. |
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
"Bridge-synthesized Items" section, `stage-1/vocabulary.md:901-942`)
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
  for diagnostics and pretty-printing (D14).
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

**Worked example.** See Section 3.1 below.

### 1.3 D5.3-A -- Vtable type contract

**Decision.** `Ty::Dynamic.vtable_ty: TyId` resolves to a
synthesised `Ty::Struct` value. The `Ty::Struct`'s Item details
(DefPath segments, rendered form, hash rule, kind, fields,
layout, supertrait pointers) and surrounding machinery
(per-(Trait, SelfTy) vtable Static instance Items per stage-1's
reservation, vtable shim Function Items per
`InstanceKind::VTableShim` precedent, and the
`RegseqModel.vtables` dedup table) are pinned in
`stage-12/expectations.md` per `discussions-SESSION-A.md`
Section 4.3.

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
   `stage-1/vocabulary.md:910-927` established the bridge-synthesised
   vtable Item is per-(Trait, SelfTy), kind=Static, rendered
   form `<vtable for Trait for SelfTy>`, hash =
   `(trait_def_path_hash, self_ty_canonical_hash)`.
   Stage-1:893-895 reserved the precise vtable struct `Ty` for
   "stage 9/12 when vtable lowering is implemented." SESSION-A's
   D5.3-A respects that reservation by pinning only the
   stage-3-side contract (the `vtable_ty` slot) and forwarding
   the synthesis details to stage-12.
2. *Two synthesised Items are needed (vtable type + vtable
   instance), per the type-vs-instance split.* The vtable C
   struct (`struct Driver_VTable { ... }`) is per-(Trait,
   projection_bindings); the vtable instance (`static const
   Driver_VTable __vtable_PL011_for_Driver = ...`) is per-(Trait,
   ConcreteImpl/SelfTy). Stage-1's reservation covers the
   instance Item; the *type* Item is a SESSION-A-recognised
   addition forwarded to stage-12 for spec.
3. *Vtable shim Function Items are needed for receiver erasure.*
   Vtable function-pointer fields use erased receivers (`*const
   ()` / `*mut ()`); impl methods are lifted with concrete
   receivers per `stage-1/vocabulary.md:597-603`. The standard
   resolution (per rustc's `InstanceKind::VTableShim`) is a
   bridge-synthesised shim per (Trait, ConcreteImpl, method) that
   takes the erased receiver, casts to concrete, and tail-calls
   the impl method. Stage-12 specifies the synthesis.
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

- *Stage-1* (`stage-1/vocabulary.md:884-895, :901-942`): defines
  the per-(Trait, SelfTy) vtable Static Item shape, rendered
  form, hash composition, and DefPathData widening procedure.
  D5.3-A defers to this reservation.
- *rustc* (`stage-1/vocabulary.md:786-805`): rustc's
  `InstanceKind` taxonomy includes `VTableShim`, `Virtual`,
  `ReifyShim`, `FnPtrShim`, etc. Stage-12 expectations.md
  reflects which of these we need (VTableShim for receiver
  erasure; Virtual handed off to stage-9 per `discussions-SESSION-A.md`
  Section 4.1).
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

**Worked example.** See Section 3.1 below.

### 1.4 D5.3-B -- Fat-pointer C shape is codegen-emitted

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
D7/D8 shapes (`vocabulary.md:570-601`) and the C-qualification
policy in Section 4.1 (`vocabulary.md:1923-1951`). Four
pointer-kind variants exist for `dyn Trait` pointees:

| Source | IR shape | C `data` qualification |
| --- | --- | --- |
| `&dyn Trait` | `Ref(t_dyn, Provenance::SharedBorrow)` | `const void *data` |
| `&mut dyn Trait` | `Ref(t_dyn, Provenance::UniqueBorrow)` | `void *data` |
| `*const dyn Trait` | `Ptr(t_dyn, Mutability::Not)` | `const void *data` |
| `*mut dyn Trait` | `Ptr(t_dyn, Mutability::Mut)` | `void *data` |

**Trait-object override on D7/D8's UnsafeCell + `restrict`
policy.** Locked D7/D8 (`vocabulary.md:1941-1950`) specifies two
type-aware adjustments to the baseline pointer qualifications:
(i) `const` is dropped from `Ref(_, SharedBorrow)` when the
pointee transitively contains `UnsafeCell`; (ii) `restrict` is
added to `Ref(_, UniqueBorrow)` in declaration contexts where
uniqueness is preserved. Both adjustments **do not apply** to
the `data` field of a trait-object fat pointer:

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
responsibility; SESSION-A specifies the *uniqueness obligation*
but not the concrete mangling. Stage-12 produces the vtable
type Item with a discriminating DefPath / hash per
`discussions-SESSION-A.md` Section 4.3; codegen derives the
fat-pointer C name from that DefPath plus the pointer-kind
qualifier.

Codegen reaches the vtable struct's C name via
`Ty::Dynamic.vtable_ty`, resolved to the corresponding
`Ty::Struct` whose synthesised Item supplies the C identifier.

**Reasoning.**

1. *Symmetric with `&str` / `&[T]`.* `Ty::Str` and `Ty::Slice(T)`
   already survive in the IR; codegen pattern-matches `Ref(t, _)`
   where `t` resolves to `Ty::Str` and emits `rust_str_t`, and
   similarly for `Ty::Slice(T)` (`vocabulary.md` Section 4.2).
   `&dyn Trait` is the third unsized-pointee fat-pointer shape;
   handling it the same way avoids special-casing in the IR.
2. *Methodology rule "C semantics with information
   preservation."* Keep Rust intent until codegen needs the C
   shape. An IR-level rewrite of `Ref(t_dyn, _)` to
   `Ty::Tuple([Ptr(unit, _), Ptr(VTable_T, Not)])` would
   discard the `Ty::Dynamic` marker, costing diagnostics,
   pretty-printing fidelity (D14), and analysis precision.
3. *Mutability follows the enclosing pointer.* The same `dyn
   Trait` pointee is reachable through shared, mutable, and raw
   pointer types; codegen distinguishes them by the C
   qualifications on the `data` field, mirroring how
   `vocabulary.md` Section 4.1 handles non-`dyn` references.
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

- *Stage-3 existing* (`vocabulary.md` Section 4.2): `&str` and
  `&[T]` worked example shows the precedent for codegen-emitted
  fat-pointer typedefs. The `dyn Driver` case in that section
  ("the dyn Trait lowering is abstract pending D5; the
  fat-pointer shape shown is the eventual codegen target") is
  resolved by D5.3-B as that section foreshadowed.
- *mrustc* (`prior-art/mrustc.md` F): mrustc's `emit_ctype_ptr`
  pattern-matches the metadata kind (`MetadataType::Slice` /
  `::TraitObject` / `::None`) and emits the appropriate fat
  pointer typedef on demand. Codegen-emitted, exactly mirroring
  our approach.

**Worked example.** See Section 3.1 below.

---

## 2. D6 -- Closure naming and synthesis [LOCKED]

D6 has three sub-decisions: D6.1 (Item materialization), D6.2
(DefPath naming), D6.3 (`Ty::Closure` connection to its captures
struct). All three are locked at the stage-3 level. The detailed
mechanics of synthesising the closure-as-struct Item, the call
methods' Function Items, and the optional drop-glue Function Item
are forwarded to `stage-12/expectations.md` per
`discussions-SESSION-A.md` Section 4.3 (synthesised Item
machinery is stage-12's domain per stage-1's "Bridge-synthesized
Items" framework).

### 2.1 D6.1 -- Item materialization per closure (contract)

**Decision.** For each closure encountered in MIR, the bridge
synthesises:

1. **One captures-struct Item.** `ItemKind::Struct`. Fields = the
   captured upvars (per rustc's `tupled_upvars_ty()`,
   `findings-rustc-draft.md` A.3). Layout from
   `tcx.layout_of(closure_ty)` per D2. DefPath: rustc's closure
   DefPath verbatim (uses `DefPathData::Closure` per
   `stage-1/vocabulary.md:472`).
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

`Coroutine` / `CoroutineClosure` / `CoroutineWitness` are
rejected per D1.4.

The detailed *synthesis mechanics* (how the bridge produces
these Items, ItemId assignment, Item field population, hash
construction) are `stage-12/expectations.md` content per
`discussions-SESSION-A.md` Section 4.3. Stage-3 commits to the
above contract: which Items exist per closure, what their kinds
and DefPaths are, and how `Ty::Closure` connects to them.

**Reasoning.**

1. *Materialised projections, not abstract trait impls.* Per
   stage-1 (`stage-1/vocabulary.md:600-611`) trait declarations
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

**Worked example.** See Section 3.2 below.

### 2.2 D6.2 -- DefPath naming policy

**Decision.** The bridge reuses rustc's DefPath verbatim for the
captures-struct Item, and constructs a child DefPath for each
call-method Function Item by appending a `ValueNs(StringId)`
segment with `disambiguator = 0`.

For the captures-struct Item: the DefPath comes from
`tcx.def_path(closure_def_id)`, mapped through stage-1's
`DefPathData` translation. Stage-1's `DefPathData::Closure`
variant (`stage-1/vocabulary.md:472`) is the anon-namespace
marker; rustc's per-sibling disambiguator (`#0`, `#1`, ...) flows
into the `DisambiguatedDefPathData::disambiguator` field via
stage-1's `DefPathSegment.disambiguator`. Verbose rendering
follows stage-1's `{namespace#N}` rule
(`stage-1/vocabulary.md:545-554`).

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
   (`stage-1/vocabulary.md` F4). Reusing them feeds directly
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
- *Stage-1* (`stage-1/vocabulary.md:472, 545-554`):
  `DefPathData::Closure` is the anon-namespace marker;
  `{namespace#N}` is the verbose rendering rule.

**Worked example.** See Section 3.2 below.

### 2.3 D6.3 -- `Ty::Closure` connection to captures struct

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
   layout/codegen view. Pretty-printer (D14) renders
   `Ty::Closure` differently from `Ty::Struct` to preserve Rust
   intent.
3. *Consistent with stage-1's view.* Stage-1
   (`stage-1/vocabulary.md:842, 866-867`) places struct/enum/union
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

**Worked example.** See Section 3.2 below.

---

## 3. Worked examples

### 3.1 `&dyn Driver` lowering (D5)

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
  per `stage-1/vocabulary.md:597-603`, with concrete receiver
  `&PL011` (substituted from `&self`).
- The bridge synthesises the vtable type `Ty::Struct` (with
  `vtable_item` per stage-12 expectations) and interns it,
  yielding TyId `t_vtable_Driver`. The exact `vtable_item`
  DefPath, hash, and field set are pinned in
  `stage-12/expectations.md` per `discussions-SESSION-A.md`
  Section 4.3.
- `Ty::Dynamic` for `dyn Driver`:
  ```
  Ty::Dynamic {
      auto_traits: [],                  // no Send/Sync/Unpin
      vtable_ty:   t_vtable_Driver,
  }
  ```
  Call this TyId `t_dyn_Driver`.
- `&dyn Driver` becomes `Ty::Ref(t_dyn_Driver,
  Provenance::SharedBorrow)` per locked D7/D8 at
  `vocabulary.md:570-601` (the `Mutability` field is dropped --
  it was 1:1 redundant with `Provenance` for borrows).
- `process`'s signature: `fn(Ref(t_dyn_Driver, SharedBorrow)) ->
  u32`. Stored as a `FnSig` in `model.sigs` indexed by the
  function's `FnSigId`.
- Stage-12 will register the per-(Trait, SelfTy) vtable instance
  Item (kind Static, rendered `<vtable for Driver for PL011>`
  per stage-1's reservation) with vtable shim Function Items
  (per `(Trait, ConcreteImpl, method)`) populating the
  initializer; this is forwarded per `discussions-SESSION-A.md`
  Section 4.3.
- Stage-11 will rewrite `process`'s body to call the loaded
  `vtable->read` indirectly; this is forwarded per
  `discussions-SESSION-A.md` Section 4.2.

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

/* Vtable shim for PL011's read impl (synthesised per stage-12's VTableShim
   precedent; bridges the erased-receiver vtable signature to the concrete
   impl method). */
static uint32_t Driver_read__shim_PL011(const void *self) {
    return PL011_read((const PL011_t *)self);
}

/* Vtable instance per stage-1's `<vtable for Driver for PL011>`. */
extern const Driver_VTable_t __vtable_PL011_for_Driver;
const Driver_VTable_t __vtable_PL011_for_Driver = {
    .drop_in_place = PL011_drop_in_place,
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
different `data`-field qualifications per the table in D5.3-B
(`vocabulary-SESSION-A.md` Section 1.4); a single typedef cannot
represent both `const void *data` and `void *data` without
collision. The vtable instance is one `static const` per
`(Trait, ConcreteImpl)`, with vtable shims one per `(Trait,
ConcreteImpl, method)` to bridge erased-receiver fn-pointer
fields to the concrete impl methods. The dispatch site loads
`d.vtable->read` and indirect-calls it with `d.data`.

The vtable shim, vtable instance, and stage-9/11 statement-level
mechanics are all pinned in their respective `expectations.md`
files per `discussions-SESSION-A.md` Section 4.

### 3.2 Closure lowering (D6)

**Rust source.**

```rust
// A FnOnce-typed closure: it consumes the captured `String` by value
// (via `into_bytes`), so it cannot be called more than once.
fn into_bytes_closure(s: String) -> impl FnOnce() -> Vec<u8> {
    move || s.into_bytes()
}

let bs_closure = into_bytes_closure(String::from("hello"));
let bs = bs_closure();
```

This closure is unambiguously `FnOnce`-typed: the body
`s.into_bytes()` consumes `s` (by value), which forces
`ClosureKind::FnOnce` (closures that consume any capture by
value cannot be `FnMut` or `Fn`). For an `Fn`-typed closure that
only reads its captures, all three call methods would be
materialised; for an `FnMut`-typed closure that mutates without
consuming, two would be.

**Stage-3 IR shape (after bridge construction).**

The closure is represented as:

A synthesised captures-struct Item (call its `ItemId` `c0_item`):
- DefPath: `crate::into_bytes_closure::{closure#0}` (rustc
  DefPath verbatim; `DefPathData::Closure` segment with
  disambiguator `0`).
- Item kind: `Struct`.
- Fields: `[s: Ty::Struct(String_item, [], layout_String)]`
  (the captured upvar; rustc's `tupled_upvars_ty` resolves to
  `(String,)`, flattened to one named field).
- Layout: per `tcx.layout_of`.

A synthesised Function Item for `call_once` (call its `ItemId`
`c0_call_once_item`):
- DefPath: `crate::into_bytes_closure::{closure#0}::call_once`
  (parent = closure's DefPath; child segment
  `ValueNs(intern("call_once"))`, disambiguator `0`).
- Item kind: `Function`.
- Signature: `fn(Ty::Closure(c0_item, [])) -> Vec<u8>`
  (substituted from `closure_sig_as_fn_ptr_ty`; receiver is the
  closure-as-struct *by value*, since FnOnce takes self).

Because the closure is `FnOnce`-typed only, no `call_mut` or
`call` Function Items are synthesised.

A drop-glue Function Item is synthesised because the captured
`String` has non-trivial drop. Synthesis details per stage-12
expectations.

`Ty` entries:
- `Ty::Closure(c0_item, [])` -- closure-type-identity view, call
  it `t_closure_c0`.
- `Ty::Struct { item: c0_item, args: [], layout: <closure
  captures struct layout> }` -- captures-struct view with
  layout, call it `t_struct_c0`. Reached from `t_closure_c0` via
  `model.types.find(&Ty::Struct { item: c0_item, args: [],
  layout: _ })`.

`into_bytes_closure`'s signature (after `impl Trait` resolution
at monomorphization): `fn(String) -> Ty::Closure(c0_item, [])`.
Stored as a `FnSig` in `model.sigs`.

**C output.**

```c
typedef struct closure_into_bytes_closure_0 {
    String_t s;
} closure_into_bytes_closure_0_t;

Vec_u8_t closure_into_bytes_closure_0__call_once(closure_into_bytes_closure_0_t self) {
    return String_into_bytes(self.s);
    /* `self` is consumed by the caller; the call_once signature takes
       it by value, matching FnOnce semantics. */
}

void closure_into_bytes_closure_0__drop(closure_into_bytes_closure_0_t *self) {
    String_drop(&self->s);
}

closure_into_bytes_closure_0_t into_bytes_closure(String_t s) {
    return (closure_into_bytes_closure_0_t){ .s = s };
}

closure_into_bytes_closure_0_t bs_closure = into_bytes_closure(String_from("hello"));
Vec_u8_t bs = closure_into_bytes_closure_0__call_once(bs_closure);
/* bs_closure is consumed by call_once; no separate drop. */
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

---

## 4. Section 5 (Open decisions table) delta + Section 1 variant table edits

### 4.1 Section 5 row replacements

In `vocabulary.md` Section 5, replace the D5 and D6 rows:

| Tag | Subject | Status |
| --- | --- | --- |
| D5 | `Dynamic` lowering (abstract vs early-materialized fat ptr) | **LOCKED** -- `Ty::Dynamic { auto_traits, vtable_ty }`; `vtable_ty` resolves to a `Ty::Struct` whose Item details are pinned in `stage-12/expectations.md`; fat-pointer codegen-emitted (D5.3-B); statement-level dispatch rewriting deferred to stage 11 (Section 1.8) |
| D6 | Closure naming (`closure#N` DefPath segment, etc.) | **LOCKED** -- one captures-struct Item + 1-3 call-method Function Items + drop-glue iff needed; rustc DefPath verbatim; `Ty::Closure` and corresponding `Ty::Struct` share an Item; synthesis details in `stage-12/expectations.md` (Section 1.9) |

### 4.2 Section 1 variant table edits

In `vocabulary.md` Section 1.1 (the 19-variant table at lines
51-74), replace the rows for `Dynamic` and `Closure`:

| Variant | Carries | C-side intent |
| --- | --- | --- |
| `Dynamic { auto_traits, vtable_ty }` | auto-trait set + TyId of synthesised vtable `Ty::Struct` | `dyn Trait` -- codegen emits fat pointer typedef when behind `Ref`/`Ptr` |
| `Closure(ItemId, GenericArgs)` | synthesised captures-struct Item + post-mono args | C struct of captures (layout via parallel `Ty::Struct` sharing the same Item) |

---

## 5. Wording correction to existing `vocabulary.md`

In `vocabulary.md:1689-1697` (currently part of Section 3.2 D4
reasoning; line numbers shifted from the pre-D7/D8-merge
1294-1297 by the addition of new Sections 1.8 and 1.9), the
parenthetical:

> "non-ADT synthesised types (fat-pointer structs per D5 -- which
> are non-ADT in our IR because they are concrete
> `Ty::Ptr`/`Ty::Tuple` shapes, not nominal struct decls;
> drop-glue is a function decl, not a Ty)"

is **superseded by D5.3-B**. Replacement text:

> "non-ADT synthesised types call `model.types.intern(...)`
> directly; ADT-shaped synthesised types (closure captures
> structs per D6; vtable type structs per D5 with synthesis
> machinery in stage-12 expectations.md) go through
> `translate_adt`'s reservation flow with a synthesised
> `ItemId`. Drop-glue is a function decl, not a Ty. The
> fat-pointer C shape for `&dyn Trait` is codegen-emitted on
> demand (D5.3-B), mirroring the existing handling of `&str` and
> `&[T]`; `Ty::Dynamic` survives in the IR through to codegen
> and is not rewritten by any bridge or model-level pass."

The merge session applies this textual replacement during D5/D6
splice-in.

---

## 6. References (used in this document)

- `vocabulary.md` -- canonical stage-3 spec (existing locked
  decisions).
- `discussions.md` -- canonical stage-3 discussion record.
- `discussions-SESSION-A.md` -- distilled D5/D6 discussion +
  primer additions + forward-expectations for stages 9, 11, 12.
- `findings-rustc-draft.md` Theme A.3 (closures), A.4 (Dynamic),
  E.6 (HRTB region preservation).
- `prior-art/charon.md` F.2 (closures), F.3 (dyn Trait).
- `prior-art/mrustc.md` F (closures-as-structs, fat pointers).
- `prior-art/rustc-codegen-clr.md` B (`FatPtr<T>` ClassDef,
  `closure_typedef`).
- `stage-1/vocabulary.md`:472 (DefPathData::Closure), 545-554
  (verbose anon-namespace rendering), 597-603 (impl-method
  lifting with concrete receivers), 605-607 (associated-type
  erasure), 608-611 (trait declaration erasure), 625-627
  (six-variant ItemKind), 642-650 ((item, args) as
  back-reference), 670-679 (Instance shape), 786-805 (rustc
  InstanceKind disambiguator table, including VTableShim and
  Virtual), 820-868 (InstanceKind), 884-895 (vtable Ty deferred
  to stage 9/12), 901-942 (Bridge-synthesized Items framework,
  vtable Static Item reservation).
