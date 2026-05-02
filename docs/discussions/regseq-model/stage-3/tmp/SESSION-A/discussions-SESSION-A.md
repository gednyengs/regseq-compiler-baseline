# SESSION-A discussion addendum -- D5, D6

This document captures the distilled discussion record for stage-3
decisions **D5 (`Dynamic` lowering)** and **D6 (closure naming)**,
in the format used by `discussions.md` Section 1 (Decision
discussions). Plus one primer addition (region erasure as a
RegseqModel-wide invariant) for `discussions.md` Section 2, and
forward-expectations to file in stage 9, 11, and 12's
`expectations.md` files.

The merge session also applies one wording correction to existing
`vocabulary.md` text per Section 5 of `vocabulary-SESSION-A.md`.

SESSION-A scope is **stage-3 commitments only**. Synthesis
details, Call-shape design, and pass implementation are forwarded
to the relevant stages' `expectations.md` files (Section 4
below). Those stages start from SESSION-A's analysis rather than
from scratch.

---

## 0. Splice instructions for the merge session

| SESSION-A block | Lands in canonical document |
| --- | --- |
| Section 1 (D5 entry) | Append to `discussions.md` Section 1 in the existing decision-record format; replaces the current `D5 -- Dynamic lowering [OPEN]` entry. |
| Section 2 (D6 entry) | Append to `discussions.md` Section 1; replaces the current `D6 -- Closure naming [OPEN]` entry. |
| Section 3 (Region-erasure primer) | Insert as new `discussions.md` Section 2.6 after the existing 2.5 (TyInterner pattern). |
| Section 4.1 (stage-9 forward-expectation) | File in `stage-9/expectations.md`. |
| Section 4.2 (stage-11 forward-expectation) | File in `stage-11/expectations.md`. |
| Section 4.3 (stage-12 forward-expectation) | File in `stage-12/expectations.md`. |

---

## 1. D5 -- `Dynamic` lowering [LOCKED]

**Question.** How does `Ty::Dynamic` represent trait objects in
our IR, where does the lowering to fat-pointer C output happen,
and how does work distribute across stages?

**Key options considered.**

*For variant content (D5.1):*
1. Mirror rustc verbatim: `Dynamic(BoundExistentialPredicates,
   Region)` -- full HRTB binder + predicates + outer region.
2. Charon-style `DynPredicate { binder }` -- single binder over
   `ExistentialPredicate` enum.
3. Slimmer flat shape: `{ principal: TraitRef, projections,
   auto_traits, region }` with `principal_item: ItemId`.
4. Most-deferred: `Ty::Dynamic(DynItemId)` opaque, predicates in
   a side table.

*For lowering location (D5.2):*
1. In the bridge (early materialization).
2. In a regseq-model pass (mid).
3. In codegen (late).
4. Distributed across stages (eventually adopted).

*For lowered shape (D5.3):*
- A: vtable type as nominal `Ty::Struct` Item.
- B: fat-pointer rewritten in IR / codegen-only.

**Decision.**

*Variant content (D5.1):*
```rust
Ty::Dynamic {
    auto_traits: Vec<AutoTrait>,    // sorted by enum-discriminant order
    vtable_ty:   TyId,              // -> synthesised vtable Ty::Struct (details in stage-12 expectations)
}

pub enum AutoTrait { Send, Sync, Unpin }   // closed v1 set
```

No `principal`, no `projections`, no `region` slot. HRTB binders
erased uniformly with all other regions. Custom auto-traits
rejected at the bridge.

*Lowering location (D5.2):* Distributed across stages. Stage-3
commits to the `Ty::Dynamic` shape and `vtable_ty: TyId` slot,
populated by the bridge at type-translation time. Stage-9 owns
the post-rewrite Call terminator shape; stage-11 owns the
body-level dispatch-rewriting pass; stage-12 owns the
synthesis details (vtable type Item, vtable instance Item,
vtable shim Function Items, `RegseqModel.vtables` dedup table).
Codegen emits the fat-pointer C typedef on demand for
`Ref(t, _)` / `Ptr(t, _)` where `t` resolves to `Ty::Dynamic`
(per locked D7/D8 at `vocabulary.md:570-601`).

*Lowered shape (D5.3):*
- D5.3-A: `vtable_ty: TyId` resolves to a `Ty::Struct` value;
  the `Ty::Struct`'s Item, DefPath, hash, layout, and surrounding
  synthesis machinery are stage-12 territory.
- D5.3-B: `Ty::Dynamic` survives in the IR through to codegen;
  the fat-pointer C shape `(data*, vtable*)` is emitted inline by
  codegen, mirroring the existing handling of `Ty::Str` and
  `Ty::Slice` per `vocabulary.md` Section 4.2. Pointer
  qualifications follow locked D7/D8 (`vocabulary.md:570-601`)
  and the C-qualification policy in Section 4.1
  (`vocabulary.md:1923-1951`); four pointer-kind variants exist
  (`&dyn Trait`, `&mut dyn Trait`, `*const dyn Trait`,
  `*mut dyn Trait`) per the table in
  `vocabulary-SESSION-A.md` Section 1.4.

**Reasoning.**

*Bottom-up grounding from C semantics.* The C target shape is a
vtable struct + per-impl static const instance + fat pointer.
This matches the Linux/Zephyr `*_operations` driver-dispatch
idiom one-to-one. Once that target is fixed, the IR layering
question reduces to "which IR entities exist for vtable struct
vs vtable instance vs fat pointer" -- and those entities split
naturally across stage 3 (type-level), 12 (Item-level), and
codegen (C output).

*Trait identities cannot use `ItemId`.* Stage-1 erases trait
declarations (`stage-1/vocabulary.md:608-611`), associated types
(`stage-1/vocabulary.md:605-607`), and closes `ItemKind` to six
variants (`stage-1/vocabulary.md:625-627`) excluding any
trait-related kind. So a `principal: TraitRef { item: ItemId,
args }` field as initially considered is structurally
impossible. The trait identity must flow through a different
carrier: stage-1 reserved the bridge-synthesised vtable Static
Item per `(Trait, SelfTy)` (`stage-1/vocabulary.md:910-927`)
keyed by `(trait_def_path_hash, self_ty_canonical_hash)`, and
SESSION-A's design extends that framework with a parallel vtable
type Item per `(Trait, projection_bindings)` (forwarded to
stage-12). What remains in `Ty::Dynamic` is only what isn't
otherwise expressible: auto-traits (which affect type identity
but not codegen) and the `vtable_ty` link.

*Auto-traits affect type identity, not C output.* `dyn Driver +
Send` and `dyn Driver` are distinct Rust types -- the BiMap must
dedup them as distinct `Ty::Dynamic` entries -- but the C-level
fat pointer and vtable struct are identical. The closed
`AutoTrait::{Send, Sync, Unpin}` enum captures all v1 cases;
custom auto-traits are unstable and rejected at the bridge.
Sort order is enum-discriminant order for canonical
content-equality.

*Region erasure.* After a bottom-up audit, no region material
has any C-relevant content in our IR. Type-checking and
borrow-checking happen pre-MIR; HRTB constraints (`for<'a>`)
are typecheck obligations verified before we see the type.
Erasing the binder loses no codegen-relevant information. The
"no regions in RegseqModel" stance applies uniformly without an
HRTB exception. Section 3 of this document records this as a
named methodology principle.

*Stage distribution.* The original framing considered a
Charon-style monolithic "lowering pass" between bridge and
codegen. That framing collapses because each piece of work has
a natural home: type-level vtable `Ty::Struct` synthesis is
*Item* synthesis (stage 12 territory per stage-1's
"Bridge-synthesized Items" framework); statement-level
rewriting walks bodies (stage 11); the post-rewrite Call shape
is a terminator question (stage 9). Distributing the work
respects stage-1's reservations and gives each stage a focused,
implementable deliverable. SESSION-A produces the stage-3
contract plus three forward-expectations (Section 4) so the
downstream stages start with the design analysis rather than
from scratch.

*Fat-pointer symmetry with `&str` / `&[T]`.* All three are
unsized-pointee fat pointers in Rust; all three should stay as
`Ref(t, _)` (or `Ptr(t, _)`) in the IR with codegen
pattern-matching to emit the C typedef. Treating `&dyn Trait`
consistently (D5.3-B) avoids special-casing.

*Bridge-side `vtable_ty` population.* The bridge has all needed
information from rustc's `Dynamic` predicates at
type-translation time. The `vtable_ty: TyId` slot is populated
eagerly inline; the bridge mints (or looks up via stage-12's
`RegseqModel.vtables` dedup table) the vtable type Item, then
interns the corresponding `Ty::Struct`, then constructs
`Ty::Dynamic` with the resulting TyId.

**Cross-refs.**

- `vocabulary-SESSION-A.md` Section 1 (D5 audit-shape spec).
- `vocabulary-SESSION-A.md` Section 3.1 (`&dyn Driver` worked
  example).
- Section 3 of this document (region-erasure primer).
- Section 4 of this document (forward-expectations to stages 9,
  11, 12).
- `findings-rustc-draft.md` Theme A.4 (rustc `Dynamic`), E.6
  (HRTB ReBound preservation), W.13 (free vs bound region
  distinction in rustc).
- `prior-art/charon.md` F.3 (`DynPredicate` / vtable as
  TypeDecl); `prior-art/mrustc.md` F (`TRAITOBJ_PTR` /
  `emit_ctype_ptr`).
- `stage-1/vocabulary.md`:608-611 (trait declaration erasure);
  :605-607 (associated-type erasure); :625-627 (six-variant
  ItemKind); :884-895 (vtable Ty deferred to stage 9/12);
  :901-942 (Bridge-synthesized Items framework, vtable Static
  Item reservation).

---

## 2. D6 -- Closure naming and synthesis [LOCKED]

**Question.** How does the bridge name and materialize closure
types in the IR? What Items are synthesised per closure, what is
their DefPath shape, and how does `Ty::Closure` connect to the
captures struct's layout?

**Key options considered.**

*For Item materialization (D6.1):*
1. One Item per closure (kind = Struct), with captures as
   fields. Fn/FnMut/FnOnce projections as separate Function
   Items.
2. One Item per closure-and-captures combo with multiple facets
   -- doesn't fit the closed six-kind ItemKind.

*For DefPath naming (D6.2):*
1. Reuse rustc's `DefPath` verbatim, including
   `DefPathData::Closure` segments.
2. Synthesise a fresh naming scheme.

*For `Ty::Closure` -> captures connection (D6.3):*
1. Implicit via shared `(ItemId, GenericArgs)`; consumers use
   `model.types.find` to navigate to the parallel `Ty::Struct`.
2. Explicit `captures_ty: TyId` slot in `Ty::Closure`.
3. Single `Ty::Closure` carries layout inline -- re-opens
   D2/D11.

**Decision.**

*D6.1 (Item materialization contract):* Per closure the bridge
synthesises:
- One captures-struct Item (`ItemKind::Struct`, fields = upvars,
  layout from `tcx.layout_of`).
- 1-3 Function Items for call methods, depending on
  `ClosureKind` and the trait hierarchy `Fn` : `FnMut` : `FnOnce`
  (an `Fn` closure materialises all three; `FnMut` two; `FnOnce`
  one).
- Drop-glue Function Item iff the captures need dropping.

The synthesis *mechanics* (how the Items are produced, ItemId
assignment, DefPath construction details) are forwarded to
`stage-12/expectations.md` per Section 4.3 below.

*D6.2 (DefPath naming):* Reuse rustc's DefPath verbatim for the
captures-struct Item (using stage-1's `DefPathData::Closure`
segment, with `{namespace#N}` verbose rendering per
`stage-1/vocabulary.md:545-554`). For call-method Function
Items, the bridge constructs a child DefPath by appending a
`ValueNs(intern("call" / "call_mut" / "call_once"))` segment
with disambiguator `0`. No new `DefPathData` variants needed for
closures.

*D6.3 (`Ty::Closure` -> captures struct):* Implicit, via shared
`(ItemId, GenericArgs)`. `Ty::Closure(item, args)` and
`Ty::Struct { item, args, layout }` share one synthesised Item
(the captures struct Item). Consumers reach the layout-bearing
view via `model.types.find(&Ty::Struct { item, args, layout: _ })`,
which works because ADT-shaped identity ignores the layout
field per D3.

`Coroutine` / `CoroutineClosure` / `CoroutineWitness` are
rejected per D1.4 (out of v1 scope; async/coroutine).

**Reasoning.**

*Materialised projections, not abstract trait impls.* Stage-1
(`stage-1/vocabulary.md:600-611`) erases trait declarations and
impl blocks; impl methods are lifted to top-level Function
Items. Closures inherit the same model: their implicit
Fn/FnMut/FnOnce trait impls are erased; the call methods are
materialised as Function Items.

*Trait hierarchy determines the count.* `Fn` extends `FnMut`
extends `FnOnce`; a `Fn` closure satisfies all three traits, so
all three call methods are materialised. Skipping inherited
methods would mean a `Fn` closure couldn't be coerced to `&dyn
FnOnce`.

*DefPath reuse gives cross-build identity for free.* Rustc's
DefPaths feed the F4-2 deep canonicalizer (D3 reasoning
constraint 1) directly via `DefPathHash`. Inventing a fresh
naming scheme would require parallel cross-build stability
machinery for no benefit.

*Implicit connection avoids redundant duplication.* The shared
`(ItemId, GenericArgs)` *is* the connection; an explicit
`captures_ty: TyId` slot would carry no information not already
derivable. This contrasts intentionally with `Ty::Dynamic.vtable_ty`,
where the slot is information-bearing because the vtable Item is
*not* the same as anything else in `Ty::Dynamic`.

*Two `Ty` views of one Item.* `Ty::Closure` (identity view, no
layout) and `Ty::Struct(...)` (layout-bearing view) co-exist for
the same Item. The split keeps `Ty::Closure` available for
pretty-printing and dispatch-convention purposes (rendering
`{closure#0}`, distinguishing closure call sites from regular
struct value moves) while routing layout queries through the
existing `Ty::Struct` machinery (D2).

*Synthesis mechanics forwarded to stage 12.* The actual bridge
work to mint the closure-as-struct Item, the call-method
Function Items, and the drop-glue Function Item is Item-synthesis
work, fitting stage 12's domain per stage-1's "Bridge-synthesized
Items" framework (`stage-1/vocabulary.md:901-942`). SESSION-A
commits to the surface contract; stage-12 expectations.md
captures the implementation analysis.

**Cross-refs.**

- `vocabulary-SESSION-A.md` Section 2 (D6 audit-shape spec).
- `vocabulary-SESSION-A.md` Section 3.2 (closure worked
  example).
- Section 4.3 of this document (stage-12 forward-expectation,
  including closure synthesis details).
- `findings-rustc-draft.md` Theme A.3 (`ClosureArgs`,
  `ClosureKind`).
- `prior-art/charon.md` F.2 (`translate_closures.rs`);
  `prior-art/mrustc.md` F (closure-as-struct emission);
  `prior-art/rustc-codegen-clr.md` B (`closure_typedef`).
- `stage-1/vocabulary.md`:472 (`DefPathData::Closure`); :545-554
  (verbose anon-namespace rendering); :600-611 (impl-method
  lifting); :625-627 (six-variant `ItemKind`).

---

## 3. Region erasure as a RegseqModel-wide invariant (primer addition)

This subsection lifts a previously-implicit principle to a named
methodology rule, citable from any stage. The merge session
inserts it as `discussions.md` Section 2.6 (after the existing
Section 2.5 TyInterner pattern).

**Statement.** *No `Ty` variant, no IR aggregate, and no Item
field carries any region (lifetime) slot. Regions are typeck and
borrowck artifacts in rustc; the program reaching the bridge has
already been type-checked and borrow-checked; C has no region
analogue; therefore RegseqModel preserves no region material.*

**Where this principle was already implicit.**

- *D1.1 reasoning* (`discussions.md` Section 1, D1.1 entry): "by
  the time the bridge consumes a fully folded type, only rigid
  variants remain" -- the fold runs `erase_and_anonymize_regions`,
  which collapses all *free* regions.
- *D7/D8 / `Ty::Ref` shape* (`vocabulary.md:68` for the variant
  table row, `vocabulary.md:570-601` for the D7/D8 decision):
  `Ref(TyId, Provenance)` -- explicitly no region slot, and
  `Mutability` dropped in favour of the redundant-with-Provenance
  encoding.
- *Stage-1 `GenericArg` enum* (`stage-1/vocabulary.md:655-659`):
  variants are `Type(TyId)` and `Const(ConstId)` only; the
  comment explicitly notes "Lifetimes: erased post-monomorphization,
  not represented."

The principle generalises beyond these specific decisions: it
applies to every IR aggregate that might otherwise carry a
region. Stage 11 (when it expands the `FnSig` shape) and stage 12
(when it expands `Item`) inherit the invariant; the merge session
should ensure those stages' specs do not introduce region slots.

**What this principle pins.**

- *Bound regions* (HRTB binders, e.g. `for<'a>` in `dyn for<'a>
  Trait<&'a T>`) are erased along with free regions. Rustc's
  `erase_and_anonymize_regions` *retains* `ReBound` per
  `findings-rustc-draft.md` Theme E.6 (because rustc has multiple
  consumers with different needs); we go further because for our
  IR -- post-typecheck, C-facing, single consumer family -- the
  binder carries no semantically meaningful information.
- *No exception for any `Ty` variant.* `Ty::Dynamic` had been a
  candidate for an HRTB binder slot; D5 closes that question by
  applying the principle uniformly. Future stages adding new
  Ty variants must respect the same constraint or motivate an
  exception.
- *No exception for `Item` fields, function signatures, or trait
  references.* The principle is RegseqModel-wide, not per-variant.

**Why.**

1. *Type correctness was already verified.* Rustc verifies HRTB
   obligations and lifetime constraints during type-check and
   borrow-check, both pre-MIR. The IR consuming `optimized_mir`
   gets a typecheck-passing program; we don't re-verify.
2. *C has no region analogue.* No qualifier, no syntactic form,
   no RTTI in C corresponds to a Rust region. Carrying regions
   would force codegen to either ignore them (defensive
   redundancy) or re-derive constraints we've already discarded.
3. *Determinism is easier without regions.* Region representations
   in rustc are non-trivial (free vs bound, ReBound tracking,
   the `RegionKind` taxonomy). Excluding them keeps the F4-2
   deep canonicalizer simpler and reduces the bridge's
   normalisation surface.

**Methodology rules invoked.**

- "Don't redo work the rustc compiler has already done."
- "C semantics with information preservation" -- regions are not
  C-semantic information; preserving them would add
  non-C-relevant content.
- "No defensive fallbacks" -- no region-rejection guards "in
  case rustc forgets to erase one"; the bridge fold is
  authoritative.

**Cross-refs.**

- `discussions.md` Section 1 D1.1 (variant-scope reasoning that
  cites region erasure for the generic-form fold);
  `vocabulary.md`:68 (`Ty::Ref` shape); `stage-1/vocabulary.md`:655-659
  (`GenericArg` no-lifetime comment).
- `findings-rustc-draft.md` Theme E.4 (`erase_and_anonymize_regions`);
  E.6 (`ReBound` preserved by that fold; we erase further);
  W.13 (free vs bound region distinction in rustc).

---

## 4. Forward expectations to file

The merge session files the following entries in
`stage-9/expectations.md`, `stage-11/expectations.md`, and
`stage-12/expectations.md` (in the standard expectations format
per `context.md` "Forward expectations live in
`stage-N/expectations.md`"). Each entry seeds the receiving stage
with SESSION-A's design analysis so the stage doesn't restart
from scratch.

### 4.1 Stage 9 -- `Call` terminator shape supporting vtable-indirect dispatch

```
- [ ] **Define Call terminator shape supporting vtable-indirect
  dispatch** (from stage 3 D5).

  *Constraint surfaced by stage 3.* `Ty::Dynamic.vtable_ty: TyId`
  links a trait object to its vtable type. After the stage-11
  body-level rewriting pass (see stage-11/expectations.md) walks
  a `Call` whose callee is `<dyn Trait as Trait>::method`, the
  rewritten Call must dispatch through a vtable function-pointer
  field (loaded at runtime from the receiver fat pointer's
  `vtable` slot). This is *not* an `InstanceId`-pinned dispatch
  in the sense of stage-1/vocabulary.md:873-876: there is no
  single `Instance` whose `InstanceId` identifies the runtime
  callee; the callee is loaded from a field at call-site time.

  *Two design candidates, both requiring some revision:*

  (a) Public Instance discriminator addition. Rustc's
      `InstanceKind::Virtual` exists as a callable variant
      (rank 5 in stage-1/vocabulary.md:786-805) but stage 1
      keeps the public Regseq `InstanceKind` to six emission
      variants only (stage-1/vocabulary.md:820-849), and
      explicitly states that the rustc shim discriminator
      `fn_shim_disambig` is **not** exposed as a public
      Instance field (stage-1/vocabulary.md:747-754) -- it
      survives only in `symbol_name`, `body`, and the F4-2
      sort key. Therefore making vtable-indirect calls
      observable to stage-9/codegen via the existing public
      InstanceKind requires either (a-i) adding a public
      `Virtual` variant to InstanceKind (a stage-1 revision
      that reopens the closed six-variant set per
      stage-1/vocabulary.md:869-871), or (a-ii) promoting
      `fn_shim_disambig` to a public Instance field exposing
      the shim discriminator (also a stage-1 revision). Cost:
      stage-1 spec mutability per the spec-mutability rule
      cited at stage-1/vocabulary.md:919-927.

  (b) Widen `Call.callee` from `InstanceId` to an enum
      `CallCallee::{Direct(InstanceId), Indirect(Operand)}` (or
      similar). The `Indirect(Operand)` carries the loaded vtable
      function pointer. Cost: stage-9-only widening; no stage-1
      revision needed; consumers (codegen, body analyses) must
      handle two callee kinds.

  *Recommended evaluation criteria.* (i) Cost to stage-1's
  locked InstanceKind closure -- option (a) requires a stage-1
  revision; option (b) does not. (ii) Cost to stage-11's pass
  implementation -- which option is simpler to produce?
  (iii) Cost to codegen -- which option is simpler to consume?
  (iv) Consistency with rustc's design -- rustc treats Virtual
  as a variant of `InstanceKind`, suggesting option (a-i) is the
  rustc-aligned path. (v) Future extensibility -- if other
  rustc shim kinds (FnPtrShim, ReifyShim, etc.) eventually need
  to surface, do they fit option (a)'s discriminator/variant
  pattern or option (b)'s indirect-Operand pattern more
  naturally?

  *Cross-refs.*
  - stage-3/tmp/SESSION-A/vocabulary-SESSION-A.md Section 1.2
    (D5.2 distribution table).
  - stage-3/tmp/SESSION-A/discussions-SESSION-A.md Section 1
    (D5 reasoning).
  - stage-1/vocabulary.md:786-805 (rustc InstanceKind
    disambiguator table including Virtual).
  - stage-1/vocabulary.md:820-848 (current six-variant
    InstanceKind).
  - stage-1/vocabulary.md:873-876 (Call sites carry InstanceId).
  - prior-art/charon.md F.3 (Charon's transform_dyn_trait_calls
    rewrites to `(move (*@receiver.ptr_metadata).method)(...)`,
    an indirect call form).
```

### 4.2 Stage 11 -- Body-level dyn-Trait dispatch rewriting pass

```
- [ ] **Implement body-level dyn-Trait dispatch rewriting pass**
  (from stage 3 D5).

  *Stage-3 work already done.* The bridge populates
  `Ty::Dynamic.vtable_ty: TyId` and registers vtable type Items
  (via stage-12's synthesis machinery; see
  stage-12/expectations.md). The model-side dedup table
  `RegseqModel.vtables: HashMap<(DefPathHash, GenericArgs),
  ItemId>` is populated as part of bridge work (stage-12 spec).
  See stage-3/tmp/SESSION-A/vocabulary-SESSION-A.md Section 1.2.

  *Stage-11 deliverable.* A body-level pass that:

  (a) Recognises `Cast::Unsize` rvalues coercing
      `Concrete -> dyn Trait` and triggers stage-12's
      registration of a per-(Trait, ConcreteImpl) vtable
      instance Item (kind=Static, rendered form
      `<vtable for Trait for SelfTy>` per stage-1's reservation
      at stage-1/vocabulary.md:910-927). The pass calls into
      stage-12's vtable-instance synthesis logic; this stage
      walks the body to find the coercion sites.

  (b) Rewrites `TerminatorKind::Call` whose callee is
      `<dyn Trait as Trait>::method` into a vtable-indirect
      dispatch per the shape stage-9 chose (see
      stage-9/expectations.md). The receiver's
      `Ty::Dynamic.vtable_ty` supplies the vtable struct's TyId;
      the method index supplies the field lookup.

  *Cross-stage shape commitments.* Stage 9 owns the post-rewrite
  Call terminator shape -- a public Instance discriminator
  addition (parallel to rustc's Virtual InstanceKind, requiring
  a stage-1 revision since fn_shim_disambig is currently private
  per stage-1/vocabulary.md:747-754) or a widened Call.callee
  shape; see stage-9/expectations.md for the candidate
  comparison. Stage 12 owns:
  - vtable type Item synthesis (kind=Struct, per (Trait,
    projection_bindings)),
  - vtable instance Item synthesis (kind=Static, per (Trait,
    SelfTy), per stage-1's reservation),
  - vtable shim Function Item synthesis (per (Trait,
    ConcreteImpl, method) for receiver erasure),
  - the RegseqModel.vtables dedup table.

  *Charon precedent.* `transform_dyn_trait_calls` in Charon's
  `transform/normalize/` rewrites direct
  `<dyn Trait as Trait>::method(self, args)` calls into vtable
  function-pointer calls of the form
  `(move (*@receiver.ptr_metadata).method)(move (@receiver),
  move (@args))` (prior-art/charon.md F.3). Charon's pass also
  handles vtable instance registration alongside the rewriting.

  *Out of stage-3 scope per D5.2.*

  *Cross-refs.*
  - stage-3/tmp/SESSION-A/vocabulary-SESSION-A.md Section 1.2
    (D5.2 distribution table).
  - stage-9/expectations.md (Call terminator shape decision).
  - stage-12/expectations.md (vtable Item synthesis machinery).
  - prior-art/charon.md F.3.
```

### 4.3 Stage 12 -- Bridge-synthesised Items for vtables and closures

```
- [ ] **Define and synthesise vtable type Items, vtable instance
  Items, vtable shim Function Items, and closure synthesis details**
  (from stage 3 D5 and D6).

  *Stage-3 work already done.* The Ty-level shapes are pinned:
  `Ty::Dynamic { auto_traits, vtable_ty: TyId }` (D5.1);
  `Ty::Closure(ItemId, GenericArgs)` (D1.1 + D6.1 contract);
  the Ty::Closure / Ty::Struct shared-Item connection (D6.3).
  See stage-3/tmp/SESSION-A/vocabulary-SESSION-A.md Sections 1
  and 2.

  *Stage-12 deliverable.* Synthesise the following bridge-synthesised
  Items (extending stage-1's "Bridge-synthesized Items" framework
  at stage-1/vocabulary.md:901-942):

  ## A. Vtable type Items (D5)

  Per `(Trait, projection_bindings)` reachable in the IR (via
  `Ty::Dynamic` construction), synthesise:

  - **One Item.**
    - `kind`: `ItemKind::Struct`.
    - `def_path` rendered form: `<vtable type for `Trait`<projection_args>>`
      (or alternative; pick a form distinct from the per-(Trait,
      SelfTy) instance Item below).
    - `def_path_hash`: `hash((trait_def_path_hash,
      projection_args_canonical_hash))`. Stable across builds.
    - `crate_id`: per stage-1:931-933.
    - DefPathData segment representation: pick concrete encoding
      (overload an existing variant, or add a new
      `DefPathData::VTableType` variant per stage-1's
      anticipated revision procedure at
      stage-1/vocabulary.md:919-927).

  - **One Ty::Struct(item, args, layout).** Interned via
    translate_adt's reservation flow with the synthesised ItemId.
    Layout: synthesised directly from rustc's
    `tcx.vtable_entries(trait_ref)` sequence (drop_glue ptr
    field + size + align + per-method fn-ptr fields + supertrait
    vtable ptrs + Vacant slots for non-dispatchable methods, per
    stage-1/vocabulary.md:884-895). Field types use **erased**
    receivers (`*const ()` / `*mut ()`) per the rustc vtable
    convention.

    *Vacant slot representation.* Each `Vacant` entry from
    rustc's `tcx.vtable_entries` corresponds to a non-dispatchable
    method (e.g., a method with a `Self: Sized` bound). Stage-12
    must represent these as **explicit placeholder fn-pointer
    fields with a fixed null initializer** in the vtable
    instance, *not* as omitted fields. The rationale: stage-11's
    rewriting pass identifies methods by their position in
    `tcx.vtable_entries`, and that position must align with the
    field index in the vtable `Ty::Struct`. Omitting Vacant
    entries would shift indices and break method-index lookup;
    using an explicit placeholder preserves index alignment with
    rustc's vtable layout. The placeholder field type is the
    same erased fn-pointer shape as a normal method entry; the
    instance initialiser sets it to `0` (NULL).

  ## B. Vtable instance Items (D5)

  Per `(Trait, ConcreteImpl)` reachable (via `Cast::Unsize`
  rvalues at coercion sites; stage-11's pass triggers
  registration), synthesise:

  - **One Item per stage-1's reservation at
    stage-1/vocabulary.md:910-927**:
    - `kind`: `ItemKind::Static`.
    - `def_path` rendered form: `<vtable for `Trait` for
      `SelfTy`>` (stage-1's existing reservation).
    - `def_path_hash`: `hash((trait_def_path_hash,
      self_ty_canonical_hash))` (stage-1's existing reservation).
    - DefPathData segment representation: same as the type Item
      above (or distinct -- stage-1 anticipates either choice).

  - **One Instance** with:
    - `InstanceKind::Static { ty: <vtable type Item's
      Ty::Struct's TyId>, init: <ConstId for the const aggregate> }`.
    - `init` references vtable shim Function Item InstanceIds
      (one per method, see C below) plus drop_in_place
      InstanceId plus const size/align values, in the field
      order of the vtable type.

  ## C. Vtable shim Function Items (D5)

  Per `(Trait, ConcreteImpl, method)` (where method ranges over
  the trait's dispatchable methods), synthesise:

  - **One Function Item** (rustc precedent:
    `InstanceKind::VTableShim` per
    stage-1/vocabulary.md:786-805):
    - `kind`: `ItemKind::Function`.
    - `def_path` rendered form: pick a synthetic form (e.g.,
      `<shim for `Trait`::`method` for `SelfTy`>`).
    - Body: takes erased receiver (`*const ()` or `*mut ()`),
      casts to concrete (`&SelfTy` or `&mut SelfTy`), tail-calls
      the lifted impl method (which has concrete receiver per
      stage-1/vocabulary.md:597-603).
    - Signature: matches the vtable type Item's corresponding
      function-pointer field (erased receiver).

  Without these shims, the vtable instance's initializer cannot
  type-check: vtable fn-pointer slots use erased receivers; impl
  methods use concrete receivers. The shim bridges them.

  ## D. RegseqModel.vtables dedup table (D5)

  - **Schema:** `vtables: HashMap<(DefPathHash,
    GenericArgs), ItemId>`. Key components are IR types
    (DefPathHash is a stage-1 concept;
    GenericArgs is stage-1's `Vec<GenericArg>`); zero rustc
    deps (Q1 preserved).
  - **Population:** by the bridge during type translation, when
    `rustc::TyKind::Dynamic` is encountered. The bridge mints
    the vtable type Item (one-shot) and caches its ItemId for
    repeat encounters of the same `(Trait, projection_bindings)`.
  - **Serde:** mark as `#[serde(skip)]` (bridge-only,
    non-serialized); rebuild on deserialize from
    `RegseqModel.items` by walking synthesised vtable type Items
    and re-keying by their (trait_def_path_hash,
    projection_args_canonical_hash). Mirrors how
    `TyInterner.lookup` is rebuilt at deserialize per
    vocabulary.md:1760-1804.

  ## E. Closure captures-struct Items (D6)

  Per closure encountered in MIR, synthesise:

  - **One Item.**
    - `kind`: `ItemKind::Struct`.
    - `def_path`: rustc's closure DefPath verbatim (uses
      `DefPathData::Closure` segment per
      stage-1/vocabulary.md:472).
    - Fields: from rustc's `tupled_upvars_ty()` (per
      findings-rustc-draft.md A.3:125), flattened to named
      fields (`_0`, `_1`, ...).
    - Layout: from `tcx.layout_of(closure_ty)`.

  - **Two Ty entries** for the same Item:
    - `Ty::Closure(item, args)` -- identity view, no inline
      layout (D1.1).
    - `Ty::Struct(item, args, layout)` -- layout-bearing view.
    - Connection: shared `(ItemId, GenericArgs)` per D6.3;
      consumers use `model.types.find` to navigate.

  ## F. Closure call-method Function Items (D6)

  Per closure, synthesise 1-3 Function Items based on
  `ClosureKind` (per findings-rustc-draft.md A.3:127):

  - **`FnOnce`-typed closure** -> `call_once`.
  - **`FnMut`-typed closure** -> `call_mut` + inherited `call_once`.
  - **`Fn`-typed closure** -> `call` + inherited `call_mut` + inherited `call_once`.

  Each Function Item:
  - `kind`: `ItemKind::Function`.
  - `def_path`: closure's DefPath as parent + `ValueNs(intern("call"
    | "call_mut" | "call_once"))` segment with disambiguator 0
    (D6.2).
  - Signature: substituted from `closure_sig_as_fn_ptr_ty`
    (findings-rustc-draft.md A.3:108-114), with the captures
    struct as receiver:
    - `call_once`: receiver by value.
    - `call_mut`: receiver by `&mut`.
    - `call`: receiver by `&`.

  ## G. Closure drop-glue Function Item (D6)

  Iff the captures need dropping (any field has non-trivial
  drop), synthesise a drop-glue Function Item:

  - `kind`: `ItemKind::Function`.
  - Body: drop_in_place semantics for the captures struct.
  - Handled by the same drop-glue machinery used for any other
    struct with non-trivial fields per
    stage-1/vocabulary.md:686-693; not closure-specific.

  *Cross-refs.*
  - stage-3/tmp/SESSION-A/vocabulary-SESSION-A.md (full SESSION-A
    spec).
  - stage-3/tmp/SESSION-A/discussions-SESSION-A.md Section 1
    (D5 reasoning), Section 2 (D6 reasoning).
  - stage-1/vocabulary.md:786-805 (rustc InstanceKind including
    Virtual / VTableShim); :820-868 (current six-variant
    InstanceKind); :884-895 (vtable Ty deferred to stage 9/12);
    :901-942 (Bridge-synthesized Items framework).
  - stage-1/vocabulary.md:597-603 (impl-method lifting with
    concrete receivers; motivates the vtable shim need).
  - stage-1/vocabulary.md:472, 545-554 (DefPathData::Closure and
    verbose anon-namespace rendering); :600-611 (impl-method
    lifting for closure call methods); :655-659 (GenericArg has
    no lifetime variant).
  - findings-rustc-draft.md A.3 (ClosureArgs, ClosureKind), A.4
    (Dynamic).
  - prior-art/charon.md F.2 (closure synthesis), F.3 (vtable
    synthesis).
  - prior-art/mrustc.md F (closures-as-structs, fat pointers).
```

---

## 5. References

- `vocabulary-SESSION-A.md` -- per-decision spec text in canonical
  audit shape, plus the wording correction to `vocabulary.md:1689-1697`
  applied during merge.
- `discussions.md` -- canonical stage-3 discussion record (D1-D4,
  D11, D12 already locked; D7-D10 + D13-D16 still open in other
  parallel SESSIONs).
- `vocabulary.md` -- canonical stage-3 spec.
- `findings-rustc-draft.md` -- primary-source rustc evidence
  (Themes A.3, A.4, E.4, E.6, W.13).
- `prior-art/charon.md` -- Charon dyn-trait + closure handling
  (Themes F.2, F.3).
- `prior-art/mrustc.md` -- mrustc TRAITOBJ_PTR, closure-as-struct
  emission (Theme F).
- `prior-art/rustc-codegen-clr.md` -- CLR FatPtr, closure
  ClassDef (Theme B).
- `stage-1/vocabulary.md` -- stage-1 architectural commitments
  cited above (DefPathData, ItemKind, InstanceKind, Item vs
  Instance distinction, vtable-as-Static-Instance reservation,
  Bridge-synthesized Items framework).
- `context.md` "Methodology rules" -- cited inline.
