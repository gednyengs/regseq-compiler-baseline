# Stage 8 -- Discussion Record

Distilled record of the design dialogue producing the
stage-8 `Statement` vocabulary spec. Each entry captures
the question raised, key options considered, the decision
(or current lean for open decisions), the reasoning chain,
and cross-references to `spec-snapshot.md` (the spec) and
`review-log.md` (the audit trail; created on first review
round).

Status of decisions in this file mirrors `spec-snapshot.md`'s
"Statement vocabulary" section (§22) plus adjacent additions
to §18.2, plus `status.md` "Open decisions" / "Locked
decisions" tables.

D-tag convention: prefixed `D-8-K` form per `../README.md`
§5.2.

Status tags:
- `[LOCKED]` -- audit-shape recorded; pending Codex review.
- `[LOCKED]` -- closed by an approved lockability decision
  (APPROVE, or MINOR-NIT-only stable per the stage-5
  diminishing-returns precedent).
- `[REOPENED]` -- a later sub-decision or sibling D-tag
  forced a revision; superseded entry kept for history.
- `[DEFERRED to stage N]` -- moved to a later stage's scope.

---

## 1. Decision discussions

### D-8-1 -- `Statement` core vocabulary [LOCKED]

**Question.** What is the closed `Statement` value shape
that the bridge fills from rustc's `mir::StatementKind<'tcx>`
and that downstream consumers (stage 10 basic blocks --
`BasicBlock.statements: Vec<Statement>`; stage 11 bodies;
stage 14 backends) read as the per-block sub-terminator
instruction sequence?

**Key options.**

1. **Option A -- mirror rustc verbatim (13 variants).**
   Carry every `mir::StatementKind<'tcx>` arm: `Assign`,
   `FakeRead`, `SetDiscriminant`, `StorageLive`,
   `StorageDead`, `Retag`, `PlaceMention`,
   `AscribeUserType`, `Coverage`, `Intrinsic`,
   `ConstEvalCounter`, `Nop`, `BackwardIncompatibleDropHint`.
2. **Option B -- mirror rustc_public (12 variants).**
   Drop only `BackwardIncompatibleDropHint` (which is
   semantically a Nop).
3. **Option C -- the closed minimum-reachable surface (3
   variants).** Drop every variant that is cleanup-pass-
   discharged or out-of-pipeline under our compilation
   contract (`mir_opt_level=0` + cleanup-pass whitelist
   per `crates/regseq-driver/src/callbacks.rs:25-66`).
   Surviving variants: `Assign`, `SetDiscriminant`,
   `Intrinsic`.
4. **Option D -- mrustc-tighter (2 variants).** Drop
   `SetDiscriminant` (fold into `Rvalue::EnumVariant`)
   and `Intrinsic` (lift to `Terminator::Call` per
   mrustc's `CallTarget::Intrinsic{name, params}`).

**Decision.** Option C. Closed 3-variant
`StatementKind`:

```rust
pub enum StatementKind {
    Assign(Place, Rvalue),
    SetDiscriminant(Place, VariantIdx),
    Intrinsic(NonDivergingIntrinsic),
}
```

The 10 dropped distinct rustc variants
(`FakeRead`, `AscribeUserType`,
`BackwardIncompatibleDropHint`, `Coverage`,
`Assign(_, Rvalue::Ref(_, BorrowKind::Fake, _))`,
`PlaceMention`, `Nop`, `StorageLive`, `StorageDead`,
`Retag`, `ConstEvalCounter`) are reject-on-encounter
at the bridge with diagnostics naming the discharging
mechanism. Sub-decisions D-8-1.1 -- D-8-1.4 specialize
this composite into per-aspect commitments.

**Reasoning.** Option A admits unreachable
constructs into the IR, defeating the bridge's job of
canonicalising rustc's wide construction surface into
the post-required-cleanup-pass-discharge surface.
Option B keeps cleanup-pass-discharged constructs that
cannot reach our optimized_mir input -- all 9 variants
beyond Assign / SetDiscriminant / Intrinsic are
strip-target by required passes (CleanupPostBorrowck
NOPs FakeRead, AscribeUserType,
BackwardIncompatibleDropHint, Coverage markers,
Fake-borrow Assigns; RemovePlaceMention strips
PlaceMention and Nop; RemoveStorageMarkers
[force-on via `mir_enable_passes` whitelist] strips
StorageLive / StorageDead / Nop; AddRetag /
CtfeLimit / InstrumentCoverage are out-of-pipeline).
Option D's mrustc-tighter folding (SetDiscriminant
into Rvalue::EnumVariant; Intrinsic to Call) imposes
a structural transformation rustc itself does not
perform: real driver state-machine code emits in-place
tag-writes (`*self = State::Configured`) that the
optimized_mir actually contains as `SetDiscriminant`
statements; folding them into atomic-construction
Rvalues misrepresents what we receive. Lifting
non-diverging intrinsics to Calls breaks rustc's
explicit choice ("avoids adding a new block and
terminator for simple intrinsics" per
`rustc_middle/src/mir/syntax.rs:448-451`) and pollutes
Q7's intrinsic-recognition namespace
(`#[rseq::intrinsic("NAME")]` recognised at Calls;
mixing rustc's NonDivergingIntrinsic into the same
Call space muddles the recognizer).

Option C is the largest reachable closed surface that
compose-derives from the cleanup-pass + build-flag +
pipeline-driver evidence. Each dropped variant has a
named, evidence-based discharge mechanism (D-8-4 Part
A enumerates them with citations).

**Methodology rules invoked.** "Closed `Statement`
vocabulary" (`../memory.md` §2 stage-8-specific
principle: smallest variant set covering the
reachable content; specializes the README §2 "core is
opaque" rule's "items outside the recognized
vocabulary are rejected" pattern at the Statement
surface). "No defensive fallbacks"
(`../README.md` §2 methodology rule: do not handle
the unhandleable). "Cleanup-pass artefacts are
reject-on-encounter, not variants"
(`../memory.md` §2 stage-8-specific corollary).
"Out-of-pipeline constructs are reject-on-encounter,
not variants" (`../memory.md` §2 stage-8-specific
corollary covering build-flag-gated variants).
"Compositional behavior beats special-case flags"
(`../README.md` §2 methodology rule: the cleanup-
discharge cascade composes with stage-3 type
rejection plus pipeline-driver pass whitelist; no
parallel side-channel needed).

**Prior-art evidence.**

- `prior-art/rustc.md` §1-3 (full 13-variant surface
  + `Statement` wrapper struct + cleanup-pass
  discharge mechanics: CleanupPostBorrowck NOP rules,
  RemovePlaceMention / RemoveStorageMarkers retain
  filters, `is_required` vs `is_enabled` taxonomy,
  pass-manager override semantics).
- `prior-art/rustc_public.md` §1-3 (12-variant
  reference; documents the BackwardIncompatibleDropHint
  drop precedent and the bridge's per-variant
  treatment; closest existing owned-IR analog).
- `prior-art/mrustc.md` §1 (8-variant peer-narrowest
  surface as Option D evidence: SetDiscriminant
  folded; intrinsics lifted to Call).
- `prior-art/charon.md` §1-3 (9-variant ULLBC: Assume
  fans out to Assert; CopyNonOverlapping is a
  dedicated boxed Statement variant; DOES preserve
  StorageLive / StorageDead [Charon's input is pre-
  cleanup MIR; ours is post]; Statement-level
  rejection is absent in Charon -- absorb-or-drop
  pattern).

**Worked example.** A driver function body with one
of each retained variant and one of the discharged
variants:

```rust
enum State { Idle, Configured }
fn advance(s: &mut State, p: *mut u32, q: *const u32) {
    let _scratch = 42_u32;     // generates a temporary local
    *s = State::Configured;
    unsafe {
        core::intrinsics::assume(!p.is_null());
        core::ptr::copy_nonoverlapping(q, p, 1);
    }
}
```

After cleanup-pass discharge, the optimized_mir body
contains:

- An `Assign(Place, Rvalue)` for `_scratch = 42`.
- A `SetDiscriminant((*_1), VariantIdx(1))` for the
  `*s = State::Configured` write.
- An `Intrinsic(NonDivergingIntrinsic::Assume(...))`
  for the assume call.
- An `Intrinsic(NonDivergingIntrinsic::CopyNonOverlapping(...))`
  for the copy_nonoverlapping call.

None of `FakeRead` / `AscribeUserType` /
`StorageLive` / `StorageDead` / `Nop` /
`PlaceMention` / `Retag` / `ConstEvalCounter` /
`Coverage` / `BackwardIncompatibleDropHint` appears.
The 3-variant surface admits this body without any
defensive arm. By contrast, hand-crafted MIR with
`#[thread_local]` on a static would inject a non-
applicable construct at the rvalue level (caught by
§21 D-7); with `-Z mir-emit-retag` would inject
`Retag` (caught by §22.5.1 row 10).

**Cross-refs.** `spec-snapshot.md` §22.1 (variant
set), §22.5 (rejection rules + invariants); D-8-1.1
through D-8-1.4 (per-variant payload sub-decisions);
D-8-3 (wrapper struct); D-8-4 (rejection table +
positive invariants).

---

### D-8-1.1 -- Statement variant cardinality [LOCKED]

**Question.** Should the RegseqModel `StatementKind`
enum carry all 13 rustc variants, 12 (drop only
`BackwardIncompatibleDropHint`), 3 (drop everything
cleanup-discharged + out-of-pipeline), or 2 (drop also
`SetDiscriminant` and `Intrinsic` per mrustc)?

**Key options.** Subsumed by parent D-8-1's options
(Option A = 13 verbatim, Option B = 12 + reject 1,
Option C = 3 + reject 10, Option D = 2 + reject 11).
This sub-decision pins the variant *cardinality*;
D-8-4 pins the rejection rules for the dropped
variants; D-8-1.2 -- D-8-1.4 pin the per-variant
payload shapes for the kept variants.

**Decision.** 3 variants:

```rust
pub enum StatementKind {
    Assign(Place, Rvalue),
    SetDiscriminant(Place, VariantIdx),
    Intrinsic(NonDivergingIntrinsic),
}
```

The 10 dropped distinct rustc variants are reject-on-
encounter; details in D-8-4 (rejection table).

**Reasoning.** Each drop has its own evidence basis,
with the discharge mechanism cited per row:

| Variant | Evidence basis |
| ------- | -------------- |
| `FakeRead(_, _)` | NOPed by required `CleanupPostBorrowck` (`rustc_mir_transform/src/cleanup_post_borrowck.rs:40`; `is_required(): true` at line 88); resulting Nop swept by required `RemovePlaceMention` (`remove_place_mention.rs:18-20`) and whitelisted `RemoveStorageMarkers` (`remove_storage_markers.rs:18-20`). Documented "Disallowed after drop elaboration" (`rustc_middle/src/mir/syntax.rs:357`). |
| `AscribeUserType(_, _)` | NOPed at `cleanup_post_borrowck.rs:33`; user-type-ascriptions cleared at `cleanup_post_borrowck.rs:81-85`. "Disallowed after drop elaboration" (`rustc_middle/src/mir/syntax.rs:426`). |
| `BackwardIncompatibleDropHint { .. }` | NOPed at `cleanup_post_borrowck.rs:41`. Lint-only marker semantically equivalent to Nop (`rustc_middle/src/mir/syntax.rs:462-463`); rustc_public drops the variant (`rustc_public/src/mir/body.rs:476-490`). |
| `Coverage(BlockMarker | SpanMarker)` | NOPed at `cleanup_post_borrowck.rs:35-39`. Mid-pipeline coverage markers; not needed after `InstrumentCoverage`. |
| `Assign(_, Rvalue::Ref(_, BorrowKind::Fake, _))` | Whole Assign NOPed at `cleanup_post_borrowck.rs:34`. Compositional with stage-7 rvalue rejection of `BorrowKind::Fake` (§21.5.1 row 5). |
| `PlaceMention(_)` | Removed by required `RemovePlaceMention` (`remove_place_mention.rs:18`; `is_required: true` at line 24; `is_enabled` returns `!sess.opts.unstable_opts.mir_preserve_ub` which is off by default). Pure `let _ = PLACE;` desugar artefact. |
| `Nop` | Swept by both `RemovePlaceMention` (`remove_place_mention.rs:18`) and `RemoveStorageMarkers` (`remove_storage_markers.rs:20`). Only transient post-`make_nop`; not in optimized_mir. |
| `StorageLive(_)` | Removed by `RemoveStorageMarkers` (`remove_storage_markers.rs:18`). The pass's `is_enabled = mir_opt_level > 0 && !sess.emit_lifetime_markers()` (`remove_storage_markers.rs:10-12`) would gate it off at our default `mir_opt_level=0`, BUT the bridge driver flips it on via `mir_enable_passes` (`crates/regseq-driver/src/callbacks.rs:27` registers `("RemoveStorageMarkers", true)`). The pass-manager override at `rustc_mir_transform/src/pass_manager.rs:227-238` consults `mir_enable_passes` ahead of `is_enabled`, so the gate is bypassed. **The OPEN QUESTION raised in `memory.md` §2 is RESOLVED by this trace: the pass DOES run.** |
| `StorageDead(_)` | Same as `StorageLive(_)`. |
| `Retag(_, _)` | Only emitted by `AddRetag` (`add_retag.rs:85, 118, 177`); `is_enabled` requires `-Z mir-emit-retag`. The bridge driver does not enable that flag. Out-of-pipeline. |
| `ConstEvalCounter` | Only injected by `CtfeLimit` (`ctfe_limit.rs:36`) for const-eval-bound bodies (`mir_for_ctfe`). Driver bodies are run-time bodies (`optimized_mir`); CtfeLimit does not run on them. |
| `Coverage(non-marker variants)` | Only emitted by `InstrumentCoverage` under `-C instrument-coverage`. Driver builds do not enable coverage instrumentation. |

The 3-variant set is the largest reachable closed
surface that compose-derives from the cleanup-pass +
required-pass-discharge + build-flag-gating evidence
under our compilation contract. Each dropped variant
has at least one strong-evidence ground.

The mrustc-tighter alternative (2 variants -- drop
SetDiscriminant + Intrinsic) is rejected because:
- mrustc folds SetDiscriminant into
  `RValue::EnumVariant` because mrustc only
  constructs whole enums, never tag-writes in place;
  this does not match what optimized_mir for real
  driver code contains (state-machine transitions
  emit `SetDiscriminant` for in-place tag updates,
  not full reconstructions).
- mrustc lifts intrinsics to `Terminator::Call`
  because mrustc has no Statement-level intrinsic
  variant; rustc explicitly chose Statement for non-
  diverging cases ("avoids adding a new block and
  terminator for simple intrinsics" per
  `rustc_middle/src/mir/syntax.rs:448-451`); lifting
  to Call inflates basic-block count for code that
  is structurally straight-line, AND collides with
  Q7's regseq DSL intrinsic recognition at Call
  sites (the recognizer would see rustc's Assume /
  CopyNonOverlapping mixed with regseq DSL
  semantic intrinsics).

**Methodology rules invoked.** "Closed `Statement`
vocabulary"; "No defensive fallbacks"; "Cleanup-pass
artefacts are reject-on-encounter, not variants";
"Out-of-pipeline constructs are reject-on-encounter,
not variants"; "Compositional behavior beats
special-case flags". (Same set as D-8-1's parent
audit; specialised here at the cardinality decision.)

**Prior-art evidence.**

- `prior-art/rustc.md` §3 (per-variant cleanup-
  discharge trace), §10 (the rejection-point table
  used as a template for D-8-4 Part A), §13
  (`is_required` / `is_enabled` semantics in the
  pass manager).
- `prior-art/rustc_public.md` §1 (12-variant baseline:
  the BackwardIncompatibleDropHint drop precedent),
  §6 (preservation of FakeRead / AscribeUserType
  motivated by pre-runtime consumers' use cases --
  RegseqModel inputs are post-runtime).
- `prior-art/mrustc.md` §1 (8-variant peer-narrowest
  as Option D evidence; the SetDiscriminant fold and
  Intrinsic-to-Call lift discussed under D-8-1's
  rejection of Option D).
- `prior-art/charon.md` §1, §2 (9-variant ULLBC: the
  Assume-to-Assert lift and CopyNonOverlapping-as-
  dedicated-Statement-variant fanout discussed under
  D-8-1.4's rejection of fanout).

**Worked example.** Same as D-8-1 parent's worked
example (the `advance` function with `*s =
State::Configured` plus assume + copy_nonoverlapping
unsafe block).

**Cross-refs.** `spec-snapshot.md` §22.1 (variant
definition), §22.1.3 (variants dropped vs rustc),
§22.5.1 (rejection rules); D-8-1 (parent); D-8-4
(rejection table + invariants).

---

### D-8-1.2 -- `Assign(Place, Rvalue)` payload [LOCKED]

**Question.** What does the `Assign` variant carry?

**Key options.**

1. **Option A -- direct flat tuple `(Place, Rvalue)`.**
   Inline place and rvalue; no Box.
2. **Option B -- mirror rustc `Box<(Place, Rvalue)>`.**
   Box the payload tuple to maintain a 16-byte
   `StatementKind` size budget.
3. **Option C -- struct with named fields
   `Assign { place, rvalue }`.** Stylistic alternative
   to A.

**Decision.** Option A. Direct flat tuple:

```rust
Assign(Place, Rvalue),
```

Plus the type-equality invariant `place_ty(p) ==
rvalue_ty(rv)` modulo lifetime erasure as
I-Statement-Assign-Type-Equality (D-8-4 Part B-1;
walk-enforcement deferred to stage 11). Plus the
compositional rule that drop-suppression is the
explicit Drop terminator's job at stage 9 -- no
drop-suppression flag on the variant.

**Reasoning.** Option B's Box exists in rustc to
maintain the 16-byte `StatementKind` size assert at
`rustc_middle/src/mir/syntax.rs:1753`. RegseqModel
has no comparable size budget at the statement
surface; rustc's Box is purely a memory-layout
optimization rustc needs and we don't, exactly
mirroring stage-7 D-7-1.7's flat-`BinaryOp` decision
(rejected rustc's `Box<(Operand, Operand)>` for the
same reason). rustc_public's
`Assign(Place, Rvalue)` (`rustc_public/src/mir/body.rs:478`)
is the direct precedent; mrustc's flat
`Assign { dst, src }` and Charon's
`Assign(Place, Rvalue)` confirm the peer pattern.

Option C (struct-with-named-fields) is rejected for
consistency with stage-7's tuple-for-2-3-field
precedent (D-7-1.4 `Ref(Provenance, Place)`, D-7-1.5
`RawPtr(Mutability, Place)`). The two fields
(Place / Rvalue) are not order-confusable -- they
have distinct types -- so named fields add nothing
beyond stylistic verbosity. (Compare D-8-1.4's
`CopyNonOverlapping{src, dst, count}` where named
fields ARE correctness-essential because all three
operands have type `Operand` and order confusion
would silently produce wrong-direction copy bugs.)

The type-equality invariant is rustc's well-formedness
rule at `rustc_middle/src/mir/syntax.rs:324`
("Assignments in which the types of the place and
rvalue differ are not well-formed"); pinned by
rustc's validator
(`rustc_mir_transform/src/validate.rs:1467-1582`).
RegseqModel does not duplicate the type at the
variant level (no `assign_ty: TyId` slot); the type
is recoverable from `place_ty(lhs)` (§19's
type-coherence walk) and `rvalue_ty(rhs)` (§21's
per-variant type-recovery rules). The invariant
lives in §22.5.2 / D-8-4 Part B-1 with walk-
enforcement deferred to stage 11.

The drop-suppression non-flag is settled by rustc
semantics: `Assign` does NOT call drop on the
previous LHS value
(`rustc_middle/src/mir/syntax.rs:312-317`); drop is
the explicit `Drop` terminator's job at stage 9. No
parallel `is_drop_assign` flag needed -- compositional
with stage-9 drop terminator placement.

**Methodology rules invoked.** "Compositional
behavior beats special-case flags" (no
`is_drop_assign` flag; drop falls out of stage-9
terminator placement). "C semantics with information
preservation" (preserve rustc's variant meaning;
strip the rustc-specific Box-budget artefact).

**Prior-art evidence.**

- `prior-art/rustc.md` §4 (`Assign` payload + the
  type-equality invariant + `Box` size-budget
  rationale).
- `prior-art/rustc_public.md` §3 (flat
  `Assign(Place, Rvalue)`; no Box; same flat shape).
- `prior-art/mrustc.md` §1 (flat
  `Assign { dst, src }`; flat shape; struct-with-
  named-fields stylistic choice that doesn't apply
  to RegseqModel's tuple precedent).
- `prior-art/charon.md` §2 (flat `Assign(Place,
  Rvalue)`; flat shape).

**Worked example.** `let y = x + 1;` produces an
`Assign(_2, BinaryOp(Add, Move(_1), Constant(c1)))`
where `place_ty(_2)` resolves to `Ty::Uint(U32)` (from
the local-decl) and `rvalue_ty(BinaryOp(Add, Move(_1),
Constant(c1)))` also resolves to `Ty::Uint(U32)` (per
stage-7 BinOp type-recovery rules). I-Statement-
Assign-Type-Equality holds.

**Cross-refs.** `spec-snapshot.md` §22.2.1 (payload),
§22.5.2 (I-Statement-Assign-Type-Equality); D-8-1
(parent); D-8-4 (Part B invariants).

---

### D-8-1.3 -- `SetDiscriminant(Place, VariantIdx)` payload [LOCKED]

**Question.** What does the `SetDiscriminant` variant
carry?

**Key options.**

1. **Option A -- flat tuple `(Place, VariantIdx)`.**
   Inline; no Box; no struct fields.
2. **Option B -- mirror rustc `{ place: Box<Place>,
   variant_index: VariantIdx }`.** Box the place;
   struct with named fields.
3. **Option C -- struct with flat place, named
   fields `{ place: Place, variant_index: VariantIdx
   }`.** rustc_public's choice.
4. **Option D -- mrustc fold into `Rvalue::EnumVariant`.**
   Drop the variant entirely; require atomic full-
   enum construction via Rvalue.

**Decision.** Option A. Flat tuple:

```rust
SetDiscriminant(Place, VariantIdx),
```

Plus the variant_index in-range invariant
I-Statement-SetDiscriminant-VariantIdx (D-8-4 Part
B-2; reads `Ty::Enum.layout`'s `LayoutKind::Enum.shape`
cases per §11.1). Plus the target-type-must-be-Ty::Enum
invariant I-Statement-SetDiscriminant-Target (D-8-4
Part B-3), compositional with the §10 `Ty` taxonomy
(direct `Ty::Enum { item, args, layout }` match; not
via a `Ty::Adt` umbrella).

**Reasoning.** Option B's Box exists for rustc's 16-
byte size assert; not inherited (same reasoning as
D-8-1.2). Option C (struct-with-flat-place) is a
stylistic question -- the project leans tuple per
stage-7 precedent (D-7-1.4 `Ref(Provenance, Place)`,
D-7-1.5 `RawPtr(Mutability, Place)`, D-7-1.8
`Aggregate(AggregateKind, Box<[Operand]>)`); 2-field
payloads with distinct types use tuple. Option D is
rejected at D-8-1's parent level (mrustc's
atomic-construction model misrepresents what
optimized_mir actually contains for state-machine
driver code: in-place tag-writes are real and
SetDiscriminant captures them).

The target-type rule:
- Rustc's validator at
  `rustc_mir_transform/src/validate.rs:1551-1567`
  allows `place_ty(p).kind() ∈ { ty::Adt(..),
  ty::Coroutine(..), ty::Alias(ty::Opaque, ..) }`.
- `ty::Coroutine(..)` is rejected at stage 3 §15.1;
  the variant cannot reach optimized_mir with a
  coroutine-typed target.
- `ty::Alias(ty::Opaque, ..)` is normalized away by
  the required `PostAnalysisNormalize` pass
  (`rustc_mir_transform/src/lib.rs:642`); opaque
  types resolve to their underlying type before
  optimized_mir.
- `ty::Adt(..)` corresponds to RegseqModel's three
  direct Adt-shaped Ty variants `Ty::Struct{..}`,
  `Ty::Enum{..}`, `Ty::Union{..}` per §10. Only
  `Ty::Enum` has a discriminant to write;
  `Ty::Struct` and `Ty::Union` have no tag location.

Bridge rule: `place_ty(p) = Ty::Enum { item, args,
layout }` per §10. Other shapes (`Ty::Struct{..}`,
`Ty::Union{..}`, `Ty::Tuple{..}`, primitive types,
references, raw pointers, function types, etc.)
reject at the bridge with a diagnostic citing the
§10 `Ty` variant taxonomy. Compositional with
stage-3 `Ty` taxonomy; not a parallel
`target_kind: Enum` flag.

The variant carries no redundant Item / Layout slot;
the target type is fully recoverable from
`place_ty(p)` via stage-3 / stage-5 / §10 vocabulary
at render / codegen time (matches stage-7
Aggregate's TyId-lookup-via-§12-type-interner
pattern). The variant_index in-range check uses
`Ty::Enum`'s `LayoutKind::Enum.shape` cases per
§11.1: `EnumShape::Empty -> 0`,
`EnumShape::Single { .. } -> 1`,
`EnumShape::Multiple { variants, .. } ->
variants.len()`.

**Methodology rules invoked.** "Compositional
behavior beats special-case flags" (no `target_kind`
flag; the target-type rule falls out of stage-3
NominalKind taxonomy).

**Prior-art evidence.**

- `prior-art/rustc.md` §5 (`SetDiscriminant` payload
  + the target-type validator rule at
  `validate.rs:1467-1582`).
- `prior-art/rustc_public.md` §3 (struct-with-flat-
  place precedent for Option C).
- `prior-art/charon.md` §2 (flat tuple
  `SetDiscriminant(Place, VariantId)` precedent for
  Option A).
- `prior-art/mrustc.md` §1 (the fold-into-
  EnumVariant Option D evidence, rejected).

**Worked example.** `*s = State::Configured;` where
`State` has variants `Idle, Configured, Running`
produces `SetDiscriminant((*_1), VariantIdx(1))`.
- `place_ty((*_1)) = Ty::Enum { item: <ItemId of
  State>, args: [], layout: <Layout of State> }`
  (B-3 satisfied via the direct `Ty::Enum` match).
- `VariantIdx(1) < 3`: the bound check reads
  `Ty::Enum`'s `LayoutKind::Enum.shape ->
  EnumShape::Multiple { variants, .. } ->
  variants.len() == 3`; `1 < 3` (B-2 satisfied).

By contrast, hand-crafted MIR with a struct target
`SetDiscriminant(_2 /* type Foo struct */, _)` would
have `place_ty(_2) = Ty::Struct{..}`, which fails
B-3 at the bridge with a diagnostic citing "target of
SetDiscriminant must be Ty::Enum; got Ty::Struct."

**Cross-refs.** `spec-snapshot.md` §22.2.2 (payload),
§22.5.2 (I-Statement-SetDiscriminant-VariantIdx,
I-Statement-SetDiscriminant-Target); D-8-1 (parent);
D-8-4 (Part B invariants).

---

### D-8-1.4 -- `Intrinsic(NonDivergingIntrinsic)` payload [LOCKED]

**Question.** What does the `Intrinsic` variant carry,
and how is the `NonDivergingIntrinsic` sub-enum shaped?

**Key options.**

1. **Option A -- nested flat
   `Intrinsic(NonDivergingIntrinsic)` with sub-enum
   `Assume(Operand) | CopyNonOverlapping(CopyNonOverlapping)`
   and named-fields struct
   `CopyNonOverlapping { src, dst, count }`.** All
   flat (no Box at any level).
2. **Option B -- mirror rustc
   `Intrinsic(Box<NonDivergingIntrinsic>)`** with same
   nested sub-enum. Box at the Intrinsic arm.
3. **Option C -- fan out into top-level Statement
   variants** `Assume(Operand)` /
   `CopyNonOverlapping(CopyNonOverlapping)`. Charon-
   ish (Charon actually transforms Assume to Assert
   and gives CopyNonOverlapping its own boxed
   Statement variant).
4. **Option D -- lift to `Terminator::Call` per
   mrustc.** No Statement-level intrinsic variant.

**Decision.** Option A. Nested flat:

```rust
Intrinsic(NonDivergingIntrinsic),

pub enum NonDivergingIntrinsic {
    Assume(Operand),
    CopyNonOverlapping(CopyNonOverlapping),
}

pub struct CopyNonOverlapping {
    pub src: Operand,
    pub dst: Operand,
    pub count: Operand,
}
```

Plus per-arm operand-type invariants:
I-Statement-Intrinsic-Assume-Bool (D-8-4 Part B-4),
I-Statement-Intrinsic-CopyNonOverlapping-Types (D-8-4
Part B-5).

**Reasoning.** Option D is rejected at D-8-1's parent
level (collides with Q7 intrinsic-recognition
namespace at Calls; inflates basic-block count for
straight-line code; rustc explicitly chose Statement
to avoid this per `rustc_middle/src/mir/syntax.rs:448-451`).

Option B's Box exists for rustc's 16-byte size assert;
not inherited (same reasoning as D-8-1.2 / D-8-1.3).

Option C (fanout) is rejected on three grounds:
1. **Semantic grouping**: rustc's
   `NonDivergingIntrinsic` sub-enum has meaning -- both
   arms are rustc-internal hint primitives that don't
   diverge. Fanning out loses the grouping.
2. **Closed-surface size**: fanout would expand the
   Statement closed surface from 3 to 4 variants
   without semantic gain.
3. **Future-proofing**: rustc may add new
   `NonDivergingIntrinsic` arms (this is a rustc
   extension point). With nested, future arms land as
   sub-enum extensions (a small blast radius); with
   fanout, they would land as Statement-enum changes
   (the spec's `StatementKind` would have to grow,
   touching many more sites).

Charon's actual transformation (Assume -> Assert;
CopyNonOverlapping -> dedicated boxed Statement) is
specifically a *transformation of meaning* for
verification artefacts (Assert with
`on_failure: UndefinedBehavior` is a verification-
friendly shape; we don't want UB labels on rustc's
boolean hints). Not appropriate for our pipeline.

`CopyNonOverlapping`'s named-fields struct is **the one
correctness-essential named-fields use** in stage 8.
All three fields are `Operand`, so a tuple shape
`(Operand, Operand, Operand)` would be order-
confusable: `memcpy(dst, src, count)` and
`copy_nonoverlapping(src, dst, count)` swap arguments
1 and 2; a tuple-shape RegseqModel could silently
produce wrong-direction copies at the bridge or
codegen sites if a maintainer reads tuple positions in
the wrong order. Named fields prevent this.

(Compare D-8-1.3's `SetDiscriminant(Place, VariantIdx)`:
two fields, distinct types, not order-confusable;
tuple is correct.)

The per-arm operand-type invariants (B-4, B-5) are
rustc's validator pins; pinned at
`rustc_mir_transform/src/validate.rs:1467-1582`. Walk-
enforcement deferred to stage 11.

**Methodology rules invoked.** "Closed `Statement`
vocabulary" (nested keeps the surface at 3; fanout
would expand to 4). "Compositional behavior beats
special-case flags" (no fanout-flag at the Statement
level). "C semantics with information preservation"
(preserve rustc's NonDivergingIntrinsic semantic
group; strip rustc-specific Box-budget artefacts).

**Prior-art evidence.**

- `prior-art/rustc.md` §6 (`Intrinsic` payload +
  `NonDivergingIntrinsic` definition + the
  `CopyNonOverlapping` struct + the per-variant
  validator pins).
- `prior-art/rustc_public.md` §3 (nested flat
  precedent: `Intrinsic(NonDivergingIntrinsic)` with
  same sub-enum + struct shapes; no Box).
- `prior-art/mrustc.md` §1 (Option D evidence:
  intrinsics flow through `Terminator::Call`; no
  Statement-level intrinsic variant).
- `prior-art/charon.md` §2 (Option C evidence: fan
  out with transformation; rejected for semantic
  grouping + future-proofing reasons).

**Worked example.** `assume(_5)` produces
`Intrinsic(NonDivergingIntrinsic::Assume(Move(_5)))`
where `operand_ty(Move(_5)) == Ty::Bool` (B-4
satisfied).

`copy_nonoverlapping(q, p, n)` produces
`Intrinsic(NonDivergingIntrinsic::CopyNonOverlapping(
CopyNonOverlapping {
  src: Move(_2),  // *const u32
  dst: Move(_1),  // *mut u32
  count: Move(_3) // usize
}))` where:
- `operand_ty(Move(_2)) = Ty::Ptr(u32_ty_id, Mutability::Not)`
  (raw pointer; one of the deref-able pointer-like
  types accepted under the `builtin_deref(true)`
  predicate at
  `rustc_mir_transform/src/validate.rs:1521`),
- `operand_ty(Move(_1)) = Ty::Ptr(u32_ty_id, Mutability::Mut)`
  (raw pointer; same predicate at line 1531),
- `pointee(operand_ty(_2)) == pointee(operand_ty(_1))
  == u32_ty_id` (B-5 pointee-equality satisfied;
  validator's analog at
  `validate.rs:1542`),
- `operand_ty(Move(_3)) == Ty::Uint(UintKind::Usize)`
  (B-5 count-type satisfied; validator's analog at
  `validate.rs:1547`).

B-5 satisfied. Stage-14 codegen emits
`memcpy(_1, _2, _3 * sizeof(uint32_t))`. Note the
src/dst swap between the rustc input
`copy_nonoverlapping(q=_2, p=_1, n=_3)` and the C
output `memcpy(dst=_1, src=_2, ...)` -- the named-
fields struct prevents accidental order confusion at
the codegen site.

References (`Ty::Ref(_, _)`) are also admitted by
B-5 in the post-translation surface; rustc's
validator additionally accepts `Box<T>` via
`builtin_deref(true)` per
`rustc_middle/src/mir/syntax.rs:496`, but RegseqModel
narrows the admissible set: stage-3 §15.1 rejects
`Box<T>` at type translation
(`spec.md:4546` -- "alloc construct outside the
no-alloc driver DSL"), so a Box-typed operand cannot
reach a stage-8 `CopyNonOverlapping` site. The B-5
admissible set is therefore the strict subset of
rustc's `builtin_deref(true)` accept-list that
survives the §15.1 cascade: `Ty::Ptr` and `Ty::Ref`.
Driver code overwhelmingly uses raw pointers at the
`copy_nonoverlapping` call site (the intrinsic
signature takes `*const T` / `*mut T`); references
are admitted when rustc emits them through deref
coercions.

**Cross-refs.** `spec-snapshot.md` §22.2.3 (Intrinsic
payload), §22.3 (sub-enum + sub-struct), §22.5.2
(I-Statement-Intrinsic-Assume-Bool,
I-Statement-Intrinsic-CopyNonOverlapping-Types);
D-8-1 (parent); D-8-4 (Part B invariants).

---

### D-8-2 -- Statement storage and interning [LOCKED]

**Question.** Does `Statement` participate in any
top-level interning? Is there a `StatementId` handle?
Does Statement participate in cross-build identity (F4
/ F4-2)?

**Key options.**

1. **Option A -- inline storage, no interning.** Body-
   local content; lives inline in basic-block
   storage; no `StatementId`; F4 / F4-2 cross-build
   identity out of scope.
2. **Option B -- intern Statements with `StatementId`.**
   Hash-cons repeated identical statements; expose a
   handle.

**Decision.** Option A. No Statement interning.

**Reasoning.** Compositional with stage-5 D-5-2,
stage-6 D-6-2, stage-7 D-7-2 -- Place, Operand, and
Rvalue are all body-local with no interning. Statement
is body-local content built from Place + Rvalue +
Operand + VariantIdx + LocalId, all of which are
either body-local or already interned at their own
level (TyId, LocalId via stage-1's interners). There
is no payload at the Statement level that warrants
its own intern-table.

Every prior-art peer stores statements inline:
- rustc: `BasicBlockData.statements: Vec<Statement>`.
- rustc_public:
  `BasicBlock.statements: Vec<Statement>`
  (`rustc_public/src/mir/body.rs:133-137`); `Tables<'tcx, B>`
  interns 8 families (DefIds, AllocIds, Spans, Types,
  Instances, TyConsts, MirConsts, Layouts) -- not
  Statements; `Statement` and `StatementKind`
  notably do NOT derive `Hash`
  (`rustc_public_bridge/src/lib.rs:45-54`).
- mrustc: `BasicBlock::statements: std::vector<Statement>`.
- Charon: `Vec<Statement>` with positional
  `(BlockId, usize)` identity (LLBC adds
  `StatementId` only after CFG reconstruction;
  ULLBC -- the analog of our `optimized_mir`-level IR
  -- has no Statement interning).

Hash-consing repeated statements is a marginal-gain
optimization for runtime memory; the structural-
equality semantic stays simpler without it. Stage-1's
F4-2 cross-build identity discipline canonicalises
`(InstanceKey)`, not body-content.

Storage container choice (`Vec<Statement>` vs
`Box<[Statement]>`) is a stage-10 (`BasicBlock`)
decision, not stage-8's; the `Statement` surface
introduces no top-level Box indirection beyond what
individual variant payloads need internally (none
do, after D-8-1.2 -- D-8-1.4 flat decisions).

**Methodology rules invoked.** "Compositional behavior
beats special-case flags" (no `StatementId`; identity
falls out of structural equality + body-local scope).

**Prior-art evidence.**

- `prior-art/rustc.md` §3.5 (no statement-interner;
  Statement::source_info span identity is per-build).
- `prior-art/rustc_public.md` §4, §11 (Tables families
  exclude Statement; Statement does not derive Hash).
- `prior-art/mrustc.md` §3 (std::vector storage, no
  interner).
- `prior-art/charon.md` §4 (inline Vec<Statement> with
  positional identity; no interner at ULLBC level).

**Worked example.** Two assign statements that happen
to be structurally identical
(`Assign(_2, Use(Constant(c1)))` appearing twice in
two different basic blocks) both live inline at their
respective `BasicBlock.statements` vector positions;
no shared backing handle. Equality at construction
time is structural; F4-2 cross-build canonicalisation
does not see them.

**Cross-refs.** `spec-snapshot.md` §22.4 (identity and
storage); D-5-2 (Place inline); D-6-2 (Operand
inline); D-7-2 (Rvalue inline).

---

### D-8-3 -- `Statement` wrapper struct shape [LOCKED]

**Question.** What metadata does the `Statement`
wrapper struct carry around the `StatementKind`
payload? What span / scope / debuginfo fields are
preserved or dropped?

**Key options.**

1. **Option A -- minimum-span wrapper:
   `{ kind, span: Span }`.** Drop `source_info.scope`
   and `debuginfos`.
2. **Option B -- mirror rustc:
   `{ source_info: SourceInfo, kind, debuginfos:
   StmtDebugInfos }`.** Preserve everything.
3. **Option C -- mrustc-style zero-metadata: just the
   `StatementKind`.** Drop the wrapper; spans go to
   diagnostics only and are discarded.
4. **Option D -- Charon-style fidelity wrapper:
   `{ span, kind, comments_before: Vec<String> }`.**
   Add a comments-preservation field for source-
   fidelity output.

**Decision.** Option A. Minimum-span wrapper:

```rust
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span,
}
```

Field order `kind, span` matches rustc_public
(`rustc_public/src/mir/body.rs:470-474`).

The body-level `var_debug_info: Vec<VarDebugInfo>`
analog (named-local mapping per
`rustc_middle/src/mir/mod.rs:264, 1235-1257`) is a
distinct, stage-11 concern -- not on the Statement
wrapper. Outbound expectation to stage 11: preserve
named-local mapping at the body level (see "Outbound
expectations" below).

**Reasoning.** Option B preserves Rust-level
debuginfo material that has no C-shape meaning:
- `source_info.scope` indexes into `body.source_scopes`
  for DWARF lexical-scope display. RegseqModel emits
  C, not DWARF; the C backend produces C source which
  gets compiled by clang/gcc with their own debug-
  info pipelines. Carrying scope is dead weight.
- `debuginfos: StmtDebugInfos` carries per-statement
  DWARF instrumentation hints (e.g., `AssignRef(Local,
  Place)` for tracking reference assignments to
  ssa-locals; `InvalidAssign(Local)` for poison
  markers; defined at
  `rustc_middle/src/mir/statement.rs:21, 1015-1077`).
  These are translated by codegen to LLVM debug
  intrinsics. RegseqModel does not emit DWARF; the
  field is rejected on stage-1 C-shape preservation
  grounds (carried as the project-wide stance against
  Rust-level debuginfo storage).

Option C (no wrapper) is rejected because spans are
load-bearing for diagnostic emission: when the bridge
rejects a construct, the diagnostic must cite the
source location. Stage-6 D-6-1.3 already deferred
operand-level spans to "the containing Statement's
SourceInfo at stage 8" (`stage-6/spec-snapshot.md:5681`,
:5745); without a Statement-level span, that fallback
breaks. mrustc's no-span discipline is too aggressive
for our diagnostic story.

Option D's `comments_before` is Charon-specific (Aeneas
formal-verification artefacts want source-comment
fidelity); RegseqModel emits C, not formal-verification
artefacts. No analog in any other peer; rejected.

The span-only choice (drop scope) follows from:
- Stage-6 forward ref needs span only;
- Stage-1 §5.3 already pinned `Span` (12 bytes,
  inline, no `SpanId` interning; source-file +
  byte-range -- exactly what diagnostics need).

The `debuginfos` drop follows from:
- No DWARF emission target;
- Stage-1 spec rejected debuginfo storage;
- mrustc, rustc_public both confirm the drop is
  viable for their respective targets.

**Critical clarification: named-local preservation is
NOT lost by this drop.** The user-source variable name
mapping (e.g., "local _3 corresponds to user variable
`temp`") lives in rustc's body-level
`var_debug_info: Vec<VarDebugInfo>`
(`rustc_middle/src/mir/mod.rs:264, 1235-1257`) -- a
separate field on `Body`, NOT on `Statement`. That is
a stage-11 design concern. Stage 1's debuginfo
rejection covered DWARF-emission instrumentation
(StmtDebugInfos) but did NOT preempt name-mapping at
the Body level, which is distinct semantic content
needed for output readability (Q5 pretty-printer +
generated-C reviewer experience). Outbound
expectation filed to stage 11.

**Methodology rules invoked.** "C semantics with
information preservation" (preserve C-shape-relevant
information; strip rustc-specific debuginfo
infrastructure that has no C analog). "Drop
NLL/borrowck-only carriers" (`debuginfos`
StmtDebugInfo arms include borrowck-side
instrumentation hints; same discipline as stage-3 +
stage-6 +stage-7 dropping `UserTypeAnnotationIndex`).

**Prior-art evidence.**

- `prior-art/rustc.md` §3.1, §3.2, §3.3 (`Statement`
  wrapper struct + `SourceInfo`'s scope field +
  `StmtDebugInfos` semantics).
- `prior-art/rustc_public.md` §2 (the Option-A
  precedent: drop scope and debuginfos; keep span;
  closest existing analog).
- `prior-art/mrustc.md` §4 (Option C precedent: no
  wrapper at all; spans go to diagnostics only).
- `prior-art/charon.md` §3 (Option D evidence:
  `comments_before` for Aeneas; rejected).

**Worked example.** A bridge translation of a single
statement:

```text
mir::Statement {
    source_info: SourceInfo {
        span: rustc_span(file="driver.rs", lo=120, hi=137),
        scope: SourceScope(3),
    },
    kind: StatementKind::Assign(box (place, rvalue)),
    debuginfos: StmtDebugInfos(vec![]),
}
```

translates to:

```text
Statement {
    kind: StatementKind::Assign(translate_place(place), translate_rvalue(rvalue)),
    span: Span { file: <interned src/driver.rs>, lo: 120, hi: 137 },
}
```

The `scope: SourceScope(3)` and `debuginfos: vec![]`
fields are dropped at translation. The `span` carries
the source location for any diagnostic the bridge
might raise about this statement (e.g., a B-1 type-
equality violation downstream).

**Outbound expectations** (to file in
`stage-11/expectations.md` "From stage 8" at merger
close):

> **Named-local preservation.** Stage 11's `Body` /
> `LocalDecls` design must preserve user-source
> variable names (rustc's body-level
> `var_debug_info: Vec<VarDebugInfo>` per
> `[rustc_middle/src/mir/mod.rs:264, 1235-1257]`) for:
> (a) pretty-printer readability per Q5;
> (b) generated-C readability so reviewers see
> `temp = ...;` not `_3 = ...;`;
> (c) downstream debugger / linter usability on the
> emitted C. Stage 1 spec rejected DWARF-emission
> instrumentation (the `StmtDebugInfos` per-statement
> hints) but did NOT preempt name-mapping at the Body
> level, which is distinct semantic content.
> Recommended provision:
> `LocalDecl.debug_name: Option<StringId>` populated
> from rustc's `var_debug_info`, scoped within the
> body, used by `render_local` / `render_place` and
> by the C-backend identifier emission policy.

**Cross-refs.** `spec-snapshot.md` §22.1.1 (wrapper
struct definition + the named-local note); D-8-K
(consolidated outbound expectations bundle to file
at merger close).

---

### D-8-4 -- Statement-site invariants [LOCKED]

**Question.** What rejection rules and positive
invariants apply at the Statement site? Where does
walk-implementation live? Does Statement participate
in cross-build identity?

**Key options.** Five-part registry. The shape is
mostly synthesis (Parts A and B accumulate from D-8-
1.1 -- D-8-1.4); the open dimensions are:
- (a) Part A row ordering / grouping.
- (b) Part B explicit-vs-implicit treatment of
  inherited Place / Rvalue invariants.
- (c) Part C walk-implementation phase placement.
- (d) Part D cross-build identity.
- (e) Part E peer comparison level of detail.

**Decision.** Five-part registry as described in
spec-snapshot.md §22.5:

- **Part A**: 12-row rejection table grouped by
  discharge mechanism (Group 1 CleanupPostBorrowck-
  NOPed: FakeRead, AscribeUserType,
  BackwardIncompatibleDropHint,
  Coverage(BlockMarker | SpanMarker), Fake-borrow
  Assigns; Group 2 always-running cleanup-pass
  strip: PlaceMention, Nop, StorageLive, StorageDead;
  Group 3 build-flag-gated: Retag, ConstEvalCounter,
  Coverage(non-marker)). Each row cites the
  responsible pass / absent flag with file:line.
- **Part B**: 8 positive invariants, with B-0a /
  B-0b / B-0c inherited invariants explicit (rather
  than implicit-by-composition):
  - B-0a I-Statement-Place-Wellformed (inherits
    §19.7 from D-5-2).
  - B-0b I-Statement-Rvalue-Wellformed (inherits
    §21.5.2 from D-7-3, including the §21.5.1 7-row
    rejection table transitively). Applies to
    operands embedded in rvalues reachable through
    `Assign`'s payload.
  - B-0c I-Statement-Operand-Wellformed (inherits
    §20.5 directly). Applies to operands embedded in
    `Intrinsic` payloads (Assume's operand,
    CopyNonOverlapping's src/dst/count operands)
    that do NOT pass through an enclosing rvalue;
    distinct from B-0b.
  - B-1 I-Statement-Assign-Type-Equality (D-8-1.2).
  - B-2 I-Statement-SetDiscriminant-VariantIdx
    (D-8-1.3).
  - B-3 I-Statement-SetDiscriminant-Target (D-8-1.3).
  - B-4 I-Statement-Intrinsic-Assume-Bool (D-8-1.4).
  - B-5 I-Statement-Intrinsic-CopyNonOverlapping-Types
    (D-8-1.4).
- **Part C**: walk-implementation deferred to stage
  11. Same split as §19.7 / §21.5.3 walk-impl
  deferral. Compositional with B-0a / B-0b: the
  stage-11 visitor walks Place / Rvalue / Statement
  invariants in a single pass.
- **Part D**: F4 / F4-2 cross-build identity OUT of
  scope (body-local content per D-8-2; same scope
  reduction as D-5-2 / D-6-2 / D-7-2).
- **Part E**: peer comparison informational. rustc 3
  invariant clusters; rustc_public none; Charon none;
  mrustc fatal MIR_BUG. RegseqModel sides with rustc
  + mrustc on validator discipline.

**Reasoning.** Part A's grouping by discharge
mechanism makes the rejection table readable: a
consumer trying to understand "why is StorageLive
dropped" reads Group 2 and sees the
RemoveStorageMarkers + pass-manager-override trace
in one place; mixing cleanup-NOPed and build-flag-
gated rows would obscure the discharge ordering.

Part B's B-0a / B-0b explicit-rather-than-implicit
treatment is a documentation-clarity choice: the
spec is more legible when inherited invariants are
listed rather than implied by walk-traversal
composition. The cost is duplicate text (also lives
in §19.7 / §21.5.2); the benefit is that a stage-11
walker implementer can read §22.5.2 and see the full
list of rules to apply at every Statement, in one
place.

Part C's walk-impl-at-stage-11 deferral matches the
D-5-2 / D-7-3 precedent: rules at the owning stage,
walk implementation at the body-level (§3.3 Phase B
body validation). Implementing a stage-8-local
visitor would have to be retrofitted at stage 11
when bodies materialise.

Part D restates the cross-build identity scope-
reduction for the registry's completeness.

Part E's peer-comparison level of detail (4 rows,
one per peer, with one-line behaviour summaries)
matches stage-7 D-7-3's Part C peer-comparison
table.

**Methodology rules invoked.** "Compositional
behavior beats special-case flags" (no parallel
"validator discipline level" flag; the walk-impl
falls out of stage-11 body validation). "Spec is
mutable" (Part B's explicit B-0a / B-0b listings
duplicate text from §19.7 / §21.5.2 -- accepted as
documentation cost; if maintenance burden becomes
real, switch to implicit-by-composition).

**Prior-art evidence.**

- `prior-art/rustc.md` §3.7 (validator pinpoints for
  the three Statement-level invariant clusters at
  `validate.rs:1467-1582`).
- `prior-art/rustc_public.md` §5 (no construction-
  site invariants; pre-runtime consumer accommodation).
- `prior-art/charon.md` §4 (no Statement-level
  rejection; absorb-or-drop pattern).
- `prior-art/mrustc.md` §6, §7 (two-layer validator
  with fatal `MIR_BUG`; the stricter discipline that
  RegseqModel matches).

**Worked example.** A bridge walk of a stage-11 body
visiting `BasicBlock.statements[i]` for each i:

```text
1. Match the rustc statement against §22.5.1 (Part A).
   - If rustc_kind matches a row, raise a diagnostic.
2. Else, dispatch on RegseqModel `kind`:
   - For `Assign(p, rv)`:
     - Apply B-0a (visit `p` per §19.7).
     - Apply B-0b (visit `rv` per §21.5.2 + §21.5.1).
     - Apply B-1 (`place_ty(p) == rvalue_ty(rv)`).
   - For `SetDiscriminant(p, vi)`:
     - Apply B-0a (visit `p`).
     - Apply B-2 (`vi < |variants(target_enum)|`).
     - Apply B-3 (`place_ty(p) = Ty::Enum { item,
       args, layout }` per §10).
   - For `Intrinsic(NonDivergingIntrinsic::Assume(op))`:
     - Apply B-0c (the §20.5 operand-site invariants
       apply directly to `op`; this is NOT B-0b
       because the operand is not embedded in an
       enclosing rvalue).
     - Apply B-4 (`operand_ty(op) == Ty::Bool`).
   - For `Intrinsic(NonDivergingIntrinsic::CopyNonOverlapping(c))`:
     - Apply B-0c to `c.src`, `c.dst`, `c.count`
       (each is a Statement-level operand; §20.5
       applies directly).
     - Apply B-5 (the deref-able-pointee + pointee-
       equality + count-as-usize rules; cite
       `validate.rs:1517-1549`).
```

Single-pass traversal; B-0a / B-0b handled inline as
the visitor recurses into Place / Rvalue / Operand
content.

**Cross-refs.** `spec-snapshot.md` §22.5 (full
registry); D-8-1.1 (Part A discharge-mechanism trace);
D-8-1.2 (B-1); D-8-1.3 (B-2, B-3); D-8-1.4 (B-4, B-5).

---

### D-8-K -- `pretty.rs::render_statement` deferral [LOCKED]

**Question.** Should `render_statement` be implemented
at stage 8 alongside the closed surface, or deferred
to stage 11?

**Key options.**

1. **Option A -- defer to stage 11.** Match the
   D-5-K / D-6-K / D-7-K pattern. Stage-8 spec
   defines the variants; stage-11 fills the renderer
   alongside `render_place`, `render_operand`,
   `render_rvalue`.
2. **Option B -- implement at stage 8 with stub
   renderers for embedded payloads.** Synthesise
   placeholder `render_place` / `render_operand` /
   `render_rvalue` calls; replace at stage 11.

**Decision.** Option A. Defer to stage 11.

**Reasoning.** Every stage-8 variant payload routes
through stages 5/6/7 renderers transitively:
- `Assign(Place, Rvalue)` needs `render_place` (D-5-K
  -> stage 11) + `render_rvalue` (D-7-K -> stage 11).
- `SetDiscriminant(Place, VariantIdx)` needs
  `render_place` (D-5-K -> stage 11) + variant-name
  lookup direct from `place_ty(p)`'s
  `LayoutKind::Enum.shape` per the §11.1 cases
  (`EnumShape::Single { variant, .. } -> variant.name`;
  `EnumShape::Multiple { variants, .. } ->
  variants[vidx].name`). No late-binding to later
  stages required because the §10 / §11 vocabulary is
  already pinned at construction time.
- `Intrinsic(NonDivergingIntrinsic)` needs
  `render_operand` (D-6-K -> stage 11) for the
  embedded operands in both arms.

Every variant's renderer is gated on at least one
D-X-K -> stage-11 deferral. Implementing Option B at
stage 8 would require synthesising stub renderers for
Place / Rvalue / Operand -- discarded work when stage
11's actual renderers land. Co-locating
`render_statement` with the rest of the body-
rendering subtree at stage 11 keeps the rendering
surface coherent and unblocks body-content snapshot
tests when bodies materialise.

The stage-7 D-7-K precedent applies verbatim: same
deferral pattern, same operand-rendering subtree
driver, same stage-11 outbound expectation.

**Methodology rules invoked.** "Renderer style is
decided deliberately" (`../memory.md` §2 stage-8-
specific corollary). "Compositional behavior beats
special-case flags" (the deferral falls out of
embedded-payload renderers; no parallel
`render_at_stage_8` flag).

**Prior-art evidence.**

- `prior-art/rustc_public.md` §11 (`pretty_statement`
  exists at `pretty.rs:90-126` with `// FIX-ME: Make
  them pretty.` fall-throughs for AscribeUserType,
  Coverage, Intrinsic; partial implementation that
  would need completion at our level).
- `prior-art/charon.md` §27 (full FmtWithCtx impl
  with MIR-style surface; a stage-8-equivalent
  renderer is feasible at this stage, but Charon's
  body-context infrastructure ships alongside).
- `prior-art/mrustc.md` §13 (full per-variant
  `Statement::operator<<` at
  `src/mir/mir.cpp:689-722`; ships alongside
  body-context infrastructure).

**Worked example.** A stage-11 body-content snapshot
test for the §22.7.1 `add_one` example would render:

```text
bb0: {
    _2 = const 1_u32;
    _3 = Add(move _1, move _2);
    _0 = move _3;
    return;
}
```

`render_statement(_2 = const 1_u32)` calls into
`render_place(_2)` (D-5-K -> stage 11) and
`render_rvalue(Use(Constant(c1)))` (D-7-K -> stage
11; transitively `render_operand(Constant(c1))` and
`render_const(c1)` -- D-6-K + D-4-K -> stage 11 +
stage 14). The full call graph terminates only after
stage 11 (and stage 14 for `render_const`).

**Outbound expectations** (to file in
`stage-11/expectations.md` "From stage 8" at merger
close, as a three-item bundle):

1. **`render_statement` deferral.** Implement at
   stage 11 alongside `render_place` (D-5-K),
   `render_operand` (D-6-K), `render_rvalue`
   (D-7-K). Driver: every variant's payload either
   embeds a Place (D-5-K-deferred), an Rvalue
   carrying Operands (D-7-K + D-6-K-deferred), or
   Operands directly (D-6-K-deferred).
2. **Statement walk implementation** (from D-8-4
   Part C). Body-level visitor walks
   `BasicBlock.statements`, applies §22.5.1
   rejection table + §22.5.2 positive invariants in
   one body-level pass; compositional with stage-5
   Place walk + stage-7 Rvalue walk inheritance.
3. **Named-local preservation** (from D-8-3).
   `LocalDecl.debug_name: Option<StringId>`
   populated from rustc's body-level `var_debug_info`
   per `rustc_middle/src/mir/mod.rs:264, 1235-1257`,
   used by `render_local` / `render_place` /
   C-backend identifier emission.

**Cross-refs.** `spec-snapshot.md` §22.8 (rendering
deferral); §18.2 per-method ownership table update
for `render_statement` lands at merger close;
D-8-3 (named-local preservation); D-8-4 (walk-impl
deferral).

---

## 2. Outbound expectations summary

To file in `stage-11/expectations.md` "From stage 8"
at merger close:

1. `render_statement` deferral (D-8-K).
2. Statement walk implementation (D-8-4 Part C).
3. Named-local preservation (D-8-3).

Top-level `spec.md` §18.2 per-method ownership table
update for `render_statement` (with stage-11 owner)
lands at merger close.

No earlier-stage decisions reopened by stage 8. No
cross-stage friction surfaced during Phase A.
