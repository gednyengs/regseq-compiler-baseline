# Stage 7 -- Discussion Record

Distilled record of the design dialogue producing the
stage-7 `Rvalue` vocabulary spec. Each entry captures the
question raised, key options considered, the decision (or
current lean for open decisions), the reasoning chain, and
cross-references to `spec-snapshot.md` (the spec) and
`review-log.md` (the audit trail; created on first review
round).

Status of decisions in this file mirrors `spec-snapshot.md`'s
"Rvalue vocabulary" section (§21) plus adjacent additions to
§18.2, plus `status.md` "Open decisions" / "Locked decisions"
tables.

D-tag convention: prefixed `D-7-K` form per `../README.md`
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

### D-7-1 -- `Rvalue` core vocabulary [LOCKED]

**Question.** What is the closed `Rvalue` value shape that
the bridge fills from rustc's `mir::Rvalue<'tcx>` and that
downstream consumers (stages 8-9 body content, codegen) read
as the RHS-expression surface of MIR?

**Key options.**

1. **Option A -- mirror rustc's 13-variant shape verbatim.**
   Adopt rustc's `mir::Rvalue<'tcx>` structurally (Use,
   Repeat, Ref with Region, ThreadLocalRef, RawPtr, Cast,
   BinaryOp with Box pair, UnaryOp, Discriminant, Aggregate
   with full AggregateKind, ShallowInitBox, CopyForDeref,
   WrapUnsafeBinder). Carry every variant; preserve every
   borrowck / NLL / runtime-pre-lowering carrier.
2. **Option B -- 11-variant closed surface.** Drop only the
   two cleanup-pass-discharged variants (`CopyForDeref` and
   `ShallowInitBox`); preserve `ThreadLocalRef` and
   `WrapUnsafeBinder`. Reject-on-encounter via D-7-3 for the
   discharged pair.
3. **Option C -- 9-variant closed surface.** Drop the four
   constructs that cannot reach a well-formed driver
   `optimized_mir`: `CopyForDeref`, `ShallowInitBox`
   (cleanup-pass-discharged), `ThreadLocalRef` (DSL-surface
   incompatibility with Linux-kernel-module / Zephyr driver
   targets), `WrapUnsafeBinder` (nightly-experimental + zero
   driver-corpus precedent + no C-shape; type-level
   rejection of `UnsafeBinder` in §15.1 cascades to
   the rvalue level by composition).

**Decision.** Option C (the parent decision; per-aspect
sub-decisions pinned in D-7-1.1 through D-7-1.8):

```rust
pub enum Rvalue {
    Use(Operand),
    Repeat(Operand, u64),
    Ref(BorrowKind, Place),
    RawPtr(RawPtrKind, Place),
    Cast(CastKind, Operand, TyId),
    BinaryOp(BinOp, Operand, Operand),
    UnaryOp(UnOp, Operand),
    Discriminant(Place),
    Aggregate(AggregateKind, Box<[Operand]>),
}
```

Plus bridge-side translation rules folded into §15
(extending stage-6 D-6-1.2's bridge-lowering pattern to
`RawPtrKind::FakeForPtrMetadata` and to the various rustc-
internal annotations dropped per sub-decisions D-7-1.4,
D-7-1.5, D-7-1.6, D-7-1.8). Reject-on-encounter rules for
the four dropped rustc variants plus the cleanup-pass-
discharged BorrowKind / CastKind / AggregateKind sub-
variants land in D-7-3.

**Reasoning.** Three forces converge on Option C:

1. **Closed-vocabulary discipline grounded in cleanup-pass
   evidence.** rustc's required runtime-lowering passes
   discharge two variants: `EraseDerefTemps` eliminates
   `CopyForDeref` (`rustc_mir_transform/src/lib.rs:658`),
   and `ElaborateBoxDerefs` eliminates `ShallowInitBox`
   (`rustc_mir_transform/src/lib.rs:659`). Both `is_required:
   true` -- run regardless of opt-level. Both are also
   `bug!()`-rejected by codegen at `rustc_codegen_ssa/src/mir/rvalue.rs:712-713`.
   This is twice-confirmed evidence: a well-formed
   `optimized_mir` body cannot contain either variant.
   Carrying them as RegseqModel `Rvalue` arms would violate
   the closed-vocabulary methodology rule by parking
   variants the bridge will never produce.
2. **DSL-surface incompatibility excludes ThreadLocalRef.**
   `Rvalue::ThreadLocalRef(DefId)` is generated only by
   source-level `#[thread_local]` static items, gated on
   `#![feature(thread_local)]`. The two driver target form-
   factors (Linux kernel module, Zephyr device driver) do
   not provide TLS infrastructure to driver code: Linux
   kernel modules use `DEFINE_PER_CPU` (exposed through
   rust-for-linux bindings, not `#[thread_local]`); Zephyr
   exposes thread-local-like storage via `k_thread`
   structures (also not Rust TLS). A driver author who
   writes `#[thread_local]` produces source whose
   `optimized_mir` cannot link or run. The "No defensive
   fallbacks" methodology rule applies: do not carry a
   variant whose only producer is source that cannot ship.
3. **Type-level rejection cascades to WrapUnsafeBinder.**
   Stage 3 already rejects `UnsafeBinder` at type
   translation (`spec.md:4583`, §15.1 type-level rejection
   table: "unstable for-binders feature"). `Rvalue::WrapUnsafeBinder(Operand, Ty)`
   produces a value of an `UnsafeBinder<...>` type --
   precisely the type stage 3 rejects. By composition with
   stage 3, the variant is unreachable in valid IR. This is
   the strongest grounding available: a hard type-level wall
   rather than a soft pass-discharge or a source-level claim.

For per-variant payload shapes, sub-enum subsets, storage
choices, and per-site invariants, see sub-decisions
D-7-1.1 through D-7-1.8 (variant set + payload), D-7-2
(storage), D-7-3 (invariants), and D-7-K (renderer).

**Methodology rules invoked.**

- "Closed `Rvalue` vocabulary" (`../memory.md` §2 stage-7-
  specific principle) -- the closed-vocabulary discipline
  that bounds the variant set. This is a stage-7
  specialization of the README §2 "core is opaque" rule's
  "items outside the recognized vocabulary are rejected"
  pattern, applied at the Rvalue surface.
- "No defensive fallbacks" -- do not carry variants whose
  producers cannot reach the bridge (cleanup-pass-discharged,
  type-level-rejected, target-form-factor-incompatible).
- "Cleanup-pass artefacts are reject-on-encounter, not
  variants" (`../memory.md` §2 stage-7-specific principle)
  -- the discharged pair lives in D-7-3 rejection rules,
  not in the enum.
- "Compositional behavior beats special-case flags" --
  WrapUnsafeBinder drop falls out of stage 3's UnsafeBinder
  type rejection (`spec.md:4583`) plus stage 7's variant-
  cardinality rule; no parallel flag.
- "Re-derive from evidence; do not import baggage" --
  rustc's 13-variant shape carries multiple variants whose
  rationale (early-MIR optimisation hooks, async runtime
  concerns) does not transfer to RegseqModel's `optimized_mir`-
  consuming, `#![no_std]`-driver-targeting context.
- "C semantics with information preservation" -- preserve
  what rustc gives us for free for the surviving 9 variants
  (Operand sub-content, TyId references, BorrowKind /
  RawPtrKind / CastKind / AggregateKind discriminators) and
  drop borrowck / NLL / Miri-internal annotations that have
  no C-shape consequence.

**Prior-art evidence.**

- `prior-art/rustc.md` §1 (13-variant Rvalue ground truth);
  §11 (CopyForDeref / ShallowInitBox / WrapUnsafeBinder
  doc-side and consumer-side evidence); §13 (cleanup-pass
  whitelist context: which discharges are required and
  which are opt-level-gated); §15 (D-7-K candidate mapping).
- `prior-art/mrustc.md` §1 (15-variant peer surface; no
  CopyForDeref / ShallowInitBox / WrapUnsafeBinder analogs --
  C-targeting peer that already lacks these constructs);
  §11 (per-D-tag mapping).
- `prior-art/charon.md` §1 (11-variant LLBC peer surface;
  validates that 11 is workable for a verification-friendly
  downstream); §11 (translation-pass evidence: Charon
  rejects ThreadLocalRef and WrapUnsafeBinder at translation
  time, validating Option C's drop targets); §14 (D-7-K
  mapping).

**Worked example.** A typical driver function

```rust
fn read_status(reg: &mut Reg) -> u32 {
    let raw: *const u32 = &raw const reg.status;
    let v = unsafe { core::ptr::read_volatile(raw) };
    v & 0x0F
}
```

produces MIR `Rvalue`s spanning:

- `Rvalue::RawPtr(RawPtrKind::Const, Place::from(reg.status))`
  (the `&raw const` expression),
- `Rvalue::Use(Operand::Move(_2))` (where `_2` holds the
  intrinsic call result),
- `Rvalue::BinaryOp(BinOp::BitAnd, Operand::Copy(_3),
  Operand::Constant(0x0F))` (the masking operation).

All three reach stage 7 as variants of the 9-variant closed
surface; no `CopyForDeref`, no `ThreadLocalRef`, no
`ShallowInitBox`, no `WrapUnsafeBinder` is produced by this
driver code in the first place. The closed surface matches
the producible IR exactly.

**Cross-refs.**

- `spec-snapshot.md` §21 (Rvalue vocabulary), §21.6 (bridge
  translation table for §15), §21.5 (rejection rules).
- Sub-decisions D-7-1.1 through D-7-1.8.

---

### D-7-1.1 -- Rvalue variant cardinality [LOCKED]

**Question.** Should the RegseqModel `Rvalue` enum carry
all 13 rustc variants, 11 (drop only cleanup-pass-
discharged), or 9 (drop additionally `ThreadLocalRef` and
`WrapUnsafeBinder`)?

**Key options.** Subsumed by parent D-7-1's options
(Option A = 13 verbatim, Option B = 11 + reject 2,
Option C = 9 + reject 4). This sub-decision pins the
variant *cardinality*; D-7-3 pins the rejection rules for
the dropped variants.

**Decision.** 9 variants:

```rust
pub enum Rvalue {
    Use(Operand),
    Repeat(Operand, u64),
    Ref(BorrowKind, Place),
    RawPtr(RawPtrKind, Place),
    Cast(CastKind, Operand, TyId),
    BinaryOp(BinOp, Operand, Operand),
    UnaryOp(UnOp, Operand),
    Discriminant(Place),
    Aggregate(AggregateKind, Box<[Operand]>),
}
```

The four rustc variants `CopyForDeref`, `ShallowInitBox`,
`ThreadLocalRef`, and `WrapUnsafeBinder` are dropped from
the IR; rejection-on-encounter rules land in D-7-3.

**Reasoning.** Each drop has its own evidence basis:

| Variant | Evidence basis |
| ------- | -------------- |
| `CopyForDeref(Place)` | Eliminated by required `EraseDerefTemps` runtime-lowering pass (`rustc_mir_transform/src/lib.rs:658`); codegen-side `bug!("CopyForDeref in codegen")` (`rustc_codegen_ssa/src/mir/rvalue.rs:712`); twice-confirmed unreachable in `optimized_mir`. |
| `ShallowInitBox(Operand, Ty)` | Eliminated by required `ElaborateBoxDerefs` runtime-lowering pass (`rustc_mir_transform/src/lib.rs:659`); codegen-side `bug!("ShallowInitBox in codegen")` (`rustc_codegen_ssa/src/mir/rvalue.rs:713`); twice-confirmed unreachable. |
| `ThreadLocalRef(DefId)` | DSL-surface claim: source-level `#[thread_local]` requires TLS infrastructure not provided by Linux-kernel-module / Zephyr-driver target form-factors. A driver author who writes `#[thread_local]` produces source that cannot link or run. Per "No defensive fallbacks," the bridge does not carry the variant. |
| `WrapUnsafeBinder(Operand, Ty)` | §15.1 Type-level rejection table (`spec.md:4583`) already rejects `UnsafeBinder` at type translation (`unstable for-binders feature`). The variant produces a value of an `UnsafeBinder<...>` type -- precisely the type stage 3's translation rules reject. By composition with stage 3, the variant is unreachable in valid IR. |

The 9-variant set is the largest reachable closed surface
that compose-derives from the cleanup-pass + type-system +
target-form-factor evidence. Each dropped variant has at
least one strong-evidence ground; the cleanup-pass-
discharged pair has two; `WrapUnsafeBinder` has the
strongest (type-system rejection cascade).

**Methodology rules invoked.** "Closed `Rvalue` vocabulary"
(`../memory.md` §2 stage-7-specific principle: the smallest
variant set covering the reachable content; specializes
the README §2 "core is opaque" rule's "items outside the
recognized vocabulary are rejected" pattern at the Rvalue
surface). "No defensive fallbacks" (`../README.md` §2
methodology rule: do not handle the unhandleable).
"Cleanup-pass artefacts are reject-on-encounter, not
variants" (`../memory.md` §2 stage-7-specific corollary).
"Compositional behavior beats special-case flags"
(`../README.md` §2 methodology rule: WrapUnsafeBinder drop
falls out of §15.1 type-level rejection plus stage-7's
variant-cardinality rule).

**Prior-art evidence.**

- `prior-art/rustc.md` §11 (codegen rejection sites),
  §13 (cleanup-pass whitelist context).
- `prior-art/mrustc.md` §1 (peer C-targeting compiler --
  has none of CopyForDeref / ShallowInitBox / WrapUnsafeBinder;
  has BORROW unified for Ref + RawPtr; absence of Coroutine
  variants tracks the same `#![no_std]` driver-corpus
  reality).
- `prior-art/charon.md` §11 (translation-pass rejection
  matrix: CopyForDeref lowered to plain Copy, ShallowInitBox
  preserved structurally but never reached in
  optimized-MIR-derived input, ThreadLocalRef rejected,
  WrapUnsafeBinder rejected). Validates Option C's reject-
  list against an independent peer making similar choices.

**Worked example.** A driver function whose only structural
oddity is taking the address of an array element

```rust
fn first_byte(buf: &[u8; 16]) -> u8 { buf[0] }
```

produces an `optimized_mir` body containing
`Rvalue::Use(Operand::Copy(Place { local: _1, projection:
[Deref, ConstantIndex { offset: 0, ... }] }))` -- a plain
`Use` plus a 2-element place projection. None of
`CopyForDeref`, `ShallowInitBox`, `ThreadLocalRef`, or
`WrapUnsafeBinder` appears. The 9-variant surface admits
this body without any defensive arm.

By contrast, hand-crafted MIR or `optimized_mir` from a
crate that uses `#[thread_local] static FOO: u32 = 0;` would
emit `Rvalue::ThreadLocalRef(<def_id of FOO>)`. The bridge
rejects with an `unsupported_construct!` diagnostic per
D-7-3, naming "thread-local statics not supported in driver
code (target form-factor lacks TLS infrastructure)."

**Cross-refs.** `spec-snapshot.md` §21.1 (variant set),
§21.5 (rejection rules); D-7-1 parent; D-7-3 (rejection
rule list).

---

### D-7-1.2 -- `Use(Operand)` payload [LOCKED]

**Question.** What does the `Use` variant carry?

**Key options.**

1. **Option A -- direct embed.** `Use(Operand)` carries the
   stage-6 `Operand` by value.
2. **Option B -- collapse Use into Operand-as-Rvalue.**
   Eliminate the variant; allow `Operand` to appear
   wherever an `Rvalue` is expected.

**Decision.** Option A. Direct embed of stage-6 `Operand`:

```rust
Use(Operand),
```

**Reasoning.** Option B would require the type system to
treat `Operand` and `Rvalue` interchangeably at every
`Statement::Assign(Place, Rvalue)` site -- a structural
divergence from rustc's MIR with no compensating benefit.
The variant tag disambiguates "this rvalue is a pure
operand-load" from "this rvalue is a computation;" backend
lowering and pretty-printing both want the disambiguation.

Codegen at `rustc_codegen_ssa/src/mir/rvalue.rs:653` is pure
delegation: `mir::Rvalue::Use(ref operand) =>
self.codegen_operand(bx, operand)`. mrustc preserves the
variant as `RValue::Use(LValue)` (`src/mir/mir.hpp:644-650`,
per `prior-art/mrustc.md` §1). Charon preserves it as
`Rvalue::Use(Operand)` (`prior-art/charon.md` §2). Three
peers converge on direct embed.

**Methodology rules invoked.** "C semantics with
information preservation" (preserve the rustc-given variant
tag).

**Prior-art evidence.** `prior-art/rustc.md` §2;
`prior-art/mrustc.md` §1 (RValue::Use);
`prior-art/charon.md` §2.

**Worked example.** `let x = y;` produces
`StatementKind::Assign(_x, Rvalue::Use(Operand::Copy(_y)))`
in rustc MIR; translates to
`Statement::Assign(_x, Rvalue::Use(Operand::Copy(_y)))` in
RegseqModel -- direct shape-preserving translation.

**Cross-refs.** `spec-snapshot.md` §21.1 (Use variant
docstring).

---

### D-7-1.3 -- `Repeat(Operand, count)` count form [LOCKED]

**Question.** How is the array length carried -- as a
direct `u64`, or as a `ConstId` reference into stage 4's
`ConstInterner`?

**Key options.**

1. **Option A -- direct `u64`.** `Repeat(Operand, u64)`.
   Compositional with stage 3's
   `Ty::Array(elem_ty, n: u64)` canonical-form rule
   (`spec.md:4525`).
2. **Option B -- `ConstId`.** `Repeat(Operand, ConstId)`.
   Resolve through stage-4 `ConstInterner`; ConstId would
   resolve to a `Const::Uint`.

**Decision.** Option A:

```rust
Repeat(Operand, u64),
```

**Reasoning.** Three concrete arguments:

1. **Compositional with stage 3's `Ty::Array` length.**
   Stage 3's translation table (`spec.md:4525`) lowers
   rustc `Array(t, n)` to `Ty::Array(translate(t), n)` with
   `n` as post-mono `u64`. The same value (an array length)
   should be stored the same way at the rvalue level;
   maintaining two storage forms for one value would be a
   compositional break.
2. **Type-system const, not value-system const.** rustc's
   `Repeat(Operand<'tcx>, ty::Const<'tcx>)` carries a
   *type-const* (`ty::Const`) -- the same kind that
   parameterises `Ty::Array`. After monomorphisation,
   type-consts for array lengths are concrete non-negative
   integers; `u64` is sufficient. The stage-4
   `ConstInterner` handles *value-consts* (`mir::Const`),
   a different kind. Routing array lengths through the
   value-const interner would conflate the two systems.
3. **No interning payoff.** `ConstId` indirection adds
   value when a value carries cross-build identity (F4-2
   InstanceKey participation, structural-aggregate
   children) or when content uniqueness has multiple
   downstream consumers (D-6-2 reasoning). A bare `u64` is
   structurally trivial and has no analogous consumer.

**Methodology rules invoked.** "Compositional behavior
beats special-case flags" (one storage form per concept;
do not introduce parallel flag-style markers); "Re-derive
from evidence; do not import baggage" (rustc's `ty::Const`
carries genericity machinery dead post-mono; do not import).

**Prior-art evidence.** `prior-art/rustc.md` §3 (type-const
post-mono is concrete u64; cross-stage anchor to stage 3
`Ty::Array` canonical form); `prior-art/mrustc.md` §9
(`SizedArray.count is HIR::ArraySize, not Constant` --
peer-validated separation of array-length-const from
value-const); `prior-art/charon.md` §10 (Charon stores the
count as a `TyConst` analog separate from value `Operand`s,
peer-validated).

**Worked example.** `[0u32; 16]` produces rustc MIR
`Rvalue::Repeat(Operand::Constant(box ConstOperand { ...
const_: Const::Val(0u32) }), <ty::Const wrapping 16>)`.
Translation:

```text
mir::Rvalue::Repeat(elem, len_const)
  -> elem -> translate_operand(elem) -> Operand::Constant(<intern Const::Uint(U32, 0)>)
  -> len_const -> evaluate as concrete u64 -> 16u64
  -> Rvalue::Repeat(Operand::Constant(...), 16u64)
```

The result is a uniform `u64` count that mirrors the same
value in the destination type `Ty::Array(elem_ty, 16u64)`.

**Cross-refs.** `spec-snapshot.md` §21.1 (Repeat variant
docstring), §21.6 (bridge translation -- type-const
evaluation rule).

---

### D-7-1.4 -- `Ref(BorrowKind, Place)` payload + BorrowKind subset [LOCKED]

**Question.** Two woven sub-questions:
(1) Does `Ref` carry rustc's `Region<'tcx>` slot? (2) What
is the closed `BorrowKind` subset?

**Key options.** For sub-question 1: keep the Region slot
or drop it. For sub-question 2:

1. **Option A -- 2-variant subset (`SharedBorrow` /
   `UniqueBorrow`).** Drop `Fake` (cleanup-discharged) and
   the `MutBorrowKind` sub-discriminator (codegen-collapsed).
2. **Option B -- 3-variant: preserve `TwoPhaseBorrow`.**
   Carry the auto-ref distinction (method-call auto-ref).
3. **Option C -- 3-variant: preserve `ClosureCapture`.**
   Carry the closure-internal distinction.

**Decision.** Drop the Region slot; 2-variant `BorrowKind`:

```rust
Ref(BorrowKind, Place),

pub enum BorrowKind {
    SharedBorrow,
    UniqueBorrow,
}
```

The discriminator values match stage 3's
`Ty::Ref(translate(t), SharedBorrow|UniqueBorrow)` enum
(`spec.md:4527-4528`, where the discriminator type is
`Provenance` per `spec.md:2458`).

**Phase B realization.** stage-3 already defines
`Provenance { SharedBorrow, UniqueBorrow }` at
`spec.md:2458` with content identical to the abstract
`BorrowKind` proposed here. `spec-snapshot.md` §21.3.1
reuses the stage-3 `Provenance` enum directly; the
realized payload is `Ref(Provenance, Place)`. The
abstract `BorrowKind` name in this audit-trail entry is
an alias for stage-3's `Provenance`; no parallel
sibling-named enum is introduced. This is a
"compositional behavior beats special-case flags"
realization (`../README.md` §2 methodology rule).

**Reasoning.** Two threads:

**Region slot drop.** Stage 3 already pins region erasure
at the type level: `Ty::Ref` carries no region slot; §17
(`spec.md:4846` -- the §17 cross-cutting note) states "no
region material survives in `Ty::Ref` for any type that
enters the IR." The rvalue-level region drop follows from
the type-level rule by composition; no parallel justification
needed. Codegen confirms by ignoring the slot:
`rustc_codegen_ssa/src/mir/rvalue.rs:506-511` patterns
`(_, bk, place)` and synthesises the result type with
`tcx.lifetimes.re_erased`. Charon and mrustc both drop
region from their rvalue-Ref analogs (`prior-art/charon.md`
§3; `prior-art/mrustc.md` §1's BORROW variant has no
region).

**BorrowKind subset.** Three independent collapses:

1. **`Fake` is cleanup-discharged.** The required
   `CleanupPostBorrowck` pass (`is_required: true`) NOPs
   every `Rvalue::Ref(_, BorrowKind::Fake(_), _)` assignment
   statement at `rustc_mir_transform/src/cleanup_post_borrowck.rs:33-43`.
   Cannot reach `optimized_mir`.
2. **`MutBorrowKind` sub-discriminator is codegen-collapsed.**
   `rustc_codegen_ssa/src/mir/rvalue.rs:506-511` invokes
   `bk.to_mutbl_lossy()`, which collapses all `Mut { kind:
   ... }` flavours (`Default`, `TwoPhaseBorrow`,
   `ClosureCapture`) to a single `Mutability::Mut`. The
   sub-discriminator carries no information that affects
   what the C backend emits.
3. **`TwoPhaseBorrow` is borrowck-internal.** The variant
   captures borrowck's "reservation-then-activation" model;
   once borrowck has accepted the body, the borrow has its
   full mutable kind. After borrowck (which runs before
   `optimized_mir`), the distinction has no operational
   meaning. **`ClosureCapture` is closure-internal.** Stage
   14 will lift closures to structs+methods (per
   `../README.md` "C semantics with information
   preservation"). The `ClosureCapture` mut-flavour is an
   annotation on closure-environment field borrows that
   doesn't survive lifting -- closure capture borrows
   become plain `&`/`&mut` of struct fields.

The 2-variant subset is the smallest closed set covering the
reachable post-cleanup content. Stage 3 already chose this
discriminator at the type level (via `Provenance`); stage 7
matches at the rvalue level for consistency.

**Methodology rules invoked.** "Compositional behavior
beats special-case flags" (`../README.md` §2: region
erasure cascades from stage-3 type-level rule; reuse
`Provenance` discriminator naming/content). "Cleanup-pass
artefacts are reject-on-encounter, not variants"
(`../memory.md` §2 stage-7-specific corollary: `Fake`
lives in D-7-3, not in the enum). "C semantics with
information preservation" (`../README.md` §2: the
`MutBorrowKind` sub-discriminator and the `Region` slot
have no C-shape consequence -- drop them).

**Prior-art evidence.**

- `prior-art/rustc.md` §4 (Ref payload, BorrowKind 3-variant
  enum, MutBorrowKind 3-variant sub-enum, FakeBorrowKind
  closures-only origin, codegen `bk.to_mutbl_lossy()`
  collapse, CleanupPostBorrowck Fake-NOP discharge).
- `prior-art/mrustc.md` §1 (mrustc's BORROW variant: `type:
  HIR::BorrowType`, `is_raw: bool`, `val: LValue` -- no
  region, no FakeBorrowKind analog; peer-validates region
  drop and Fake non-existence at the C-target level).
- `prior-art/charon.md` §3 (Charon's `Ref { place, kind,
  ptr_metadata }`; `kind: BorrowKind` is `Shared / Mut /
  TwoPhaseMut / Shallow / UniqueImmutable`; `Shallow` is the
  Fake counterpart -- Charon preserves it because LLBC
  serves verification semantics where the borrow kind
  affects soundness, RegseqModel does not).

**Worked example.** `let r = &mut buf[0];` produces rustc
MIR `Rvalue::Ref(<region '0>, BorrowKind::Mut { kind:
MutBorrowKind::Default }, Place { local: _buf, projection:
[ConstantIndex { offset: 0, ... }] })`. Translation:

```text
mir::Rvalue::Ref(_region, BorrowKind::Mut { kind: _ }, place)
  -> drop _region (stage-3 region-erasure cascade)
  -> collapse Mut { kind: _ } to UniqueBorrow
       (codegen-validated by bk.to_mutbl_lossy())
  -> place -> translate_place(place)
  -> Rvalue::Ref(BorrowKind::UniqueBorrow, Place { local: _buf,
       projection: [ConstantIndex { offset: 0, ... }] })
```

A `Rvalue::Ref(_, BorrowKind::Fake(_), _)` would never
reach this translation: required `CleanupPostBorrowck` has
NOPed the containing statement before `optimized_mir`. If
hand-crafted IR carried it, D-7-3 rejects with an
`unsupported_construct!` diagnostic naming "fake borrow
NOPed by required CleanupPostBorrowck."

**Cross-refs.** `spec-snapshot.md` §21.1 (Ref variant
docstring), §21.3.1 (Provenance reuse note), §21.6 (bridge
translation -- region-drop + MutBorrowKind collapse +
Provenance reuse).

---

### D-7-1.5 -- `RawPtr(RawPtrKind, Place)` payload + RawPtrKind subset [LOCKED]

**Question.** What is the closed `RawPtrKind` subset, and
how does the bridge handle rustc's `FakeForPtrMetadata`
(which is *not* cleanup-pass-discharged and can survive into
`optimized_mir`)?

**Key options.**

1. **Option A -- 3-variant: preserve `FakeForPtrMetadata`.**
   Closes the rustc set. Carries an annotation that has no
   C-shape consequence; downstream consumers ignore.
2. **Option B -- 2-variant + bridge-side collapse to `Not`.**
   `Const`/`Mut`. Bridge translation rule:
   `FakeForPtrMetadata -> Const` (matching rustc codegen's
   own `to_mutbl_lossy()` mapping at
   `rustc_middle/src/mir/statement.rs:994-1003`, which uses
   `Mutability::Not` as a `*const` approximation since "we
   have no type corresponding to a fake borrow").
3. **Option C -- 2-variant + bridge-side collapse to
   `Mut`.** Treat the syntax.rs:189-197 doc comment
   "operationally equivalent to `RawPtrKind::Mut`" as the
   collapse target. Diverges from rustc codegen.

**Decision.** Option B:

```rust
RawPtr(RawPtrKind, Place),

pub enum RawPtrKind {
    Const,
    Mut,
}
```

Bridge translation rule (filed in §15 / §21.6):
`mir::RawPtrKind::FakeForPtrMetadata -> RawPtrKind::Const`.

**Phase B realization.** stage-3 already defines
`Mutability { Not, Mut }` at `spec.md:2461` with content
conceptually identical to the abstract 2-variant
`RawPtrKind`. `spec-snapshot.md` §21.3.2 reuses the
stage-3 `Mutability` enum directly; the realized payload
is `RawPtr(Mutability, Place)`. The mapping from rustc's
3-variant `RawPtrKind` to stage-3 `Mutability` is:
`RawPtrKind::Const -> Mutability::Not`,
`RawPtrKind::Mut -> Mutability::Mut`,
`RawPtrKind::FakeForPtrMetadata -> Mutability::Not` (the
bridge collapse rule above; matches rustc codegen's
`to_mutbl_lossy()` at `statement.rs:1001`). The abstract
`RawPtrKind` name in this audit-trail entry is an alias
for stage-3's `Mutability` with the documented mapping;
no parallel sibling-named enum is introduced. This is a
"compositional behavior beats special-case flags"
realization (`../README.md` §2 methodology rule).

**Reasoning.** `FakeForPtrMetadata` is reachable in driver
code: source-level constructs like
`<*const [T]>::len()` and `<*const dyn Trait>::metadata()`
generate `Rvalue::RawPtr(RawPtrKind::FakeForPtrMetadata,
Place)` to read pointer metadata without dereferencing the
data. Unlike `BorrowKind::Fake`, `FakeForPtrMetadata` is
NOT discharged by `CleanupPostBorrowck`; it survives.
Reject-on-encounter would reject valid driver code.

The bridge collapse target (`Not`) matches rustc codegen's
own choice. `RawPtrKind::to_mutbl_lossy()` at
`rustc_middle/src/mir/statement.rs:994-1003` maps
`FakeForPtrMetadata` to `Mutability::Not` ("we have no type
corresponding to a fake borrow, so use `*const` as an
approximation," per the inline comment at
`statement.rs:999-1000`); rustc codegen invokes this
mapping at `rustc_codegen_ssa/src/mir/rvalue.rs:513-515`
when synthesising the RawPtr result type via
`Ty::new_ptr(tcx, ty, kind.to_mutbl_lossy())`. The
syntax.rs:189-197 doc comment "operationally equivalent
to `RawPtrKind::Mut`, but will never incur a retag"
describes Miri retag semantics (a stack-borrows /
tree-borrows abstract-machine concept); rustc's actual
codegen behaviour is the `to_mutbl_lossy()` mapping to
`Not`. Documentation and codegen disagree; the codegen
rule is the operational ground truth and is what
RegseqModel mirrors.

Charon also collapses `FakeForPtrMetadata` to `Shared` /
read-only (see `prior-art/charon.md` §4); peer-validates
that read-only is the correct collapse target when the
output language has no fake-borrow representation.
RegseqModel adopts the rustc-codegen target (`Not` /
`*const`), which agrees with Charon's choice.

The bridge-lowering pattern matches stage-6 D-6-1.2:
when an rvalue/operand variant carries information whose
only meaning is in a domain we do not model (Miri
abstract-machine retags here; per-crate-graph session
flags in D-6-1.2), collapse at bridge time to the
documented operationally-equivalent form.

**Methodology rules invoked.** "C semantics with
information preservation" (preserve mutability as a C-shape
qualifier; drop the retag-internal sub-discriminator);
"Re-derive from evidence; do not import baggage" (rustc's
3-variant enum carries Miri infrastructure not mirrored in
RegseqModel; collapse rather than carry); "Compositional
behavior beats special-case flags" (the bridge-lowering
pattern is the same pattern as D-6-1.2 -- one mechanism for
"this carries information meaningful only in a domain we
do not model").

**Prior-art evidence.**

- `prior-art/rustc.md` §6 (RawPtr payload + RawPtrKind
  3-variant enum + FakeForPtrMetadata "operationally
  equivalent to Mut" doc-comment).
- `prior-art/mrustc.md` §1 (mrustc's BORROW variant uses
  `is_raw: bool` to distinguish ref-vs-raw-ptr; the
  `BorrowType` carries `Shared`/`Unique`/`Owned`-style
  values not Const/Mut; mutability is implicit in the
  borrow type. Validates the 2-variant collapse target -- a
  C-targeting peer also collapses the discriminator to
  Mut/Const-equivalent).
- `prior-art/charon.md` §4 (Charon's `RawPtrKind` 3-variant
  enum: `Mut`/`Const`/`FakeForPtrMetadata`; translation rule
  collapses `FakeForPtrMetadata` to `Shared` for
  verification reasons).

**Worked example.** `(buf as *const [u8]).len()` produces
rustc MIR

```text
_2 = &raw const (*_buf);
_3 = (_2 as &[u8]); ; conceptual unsizing
_4 = Len(_3);
```

After elaboration, the metadata-extraction may surface as
`Rvalue::RawPtr(RawPtrKind::FakeForPtrMetadata,
Place::from(_buf))` followed by a metadata projection.
Translation:

```text
mir::Rvalue::RawPtr(RawPtrKind::FakeForPtrMetadata, place)
  -> collapse FakeForPtrMetadata -> Mutability::Not
  -> place -> translate_place(place)
  -> Rvalue::RawPtr(Mutability::Not, translate_place(place))
```

The C backend lowers identically to a `*const` raw-ptr
construction (matching rustc codegen's own choice via
`to_mutbl_lossy()`); downstream stage-14 code that consumes
the metadata reads it via the place's `Ty::Ptr`
discriminator (stage 3, §10) plus `Ty::Slice`/`Ty::Dynamic`
pointee-shape information.

**Cross-refs.** `spec-snapshot.md` §21.1 (RawPtr variant
docstring), §21.3.2 (Mutability reuse note), §21.6.2
(FakeForPtrMetadata collapse rule).

---

### D-7-1.6 -- `Cast(CastKind, Operand, TyId)` payload + CastKind subset [LOCKED]

**Question.** What is the closed `CastKind` subset
(including the `PointerCoercion` sub-enum), and how are
rustc's diagnostic-only / structurally-duplicated
annotations handled?

**Key options.** Several sub-questions woven together:

1. **CastKind cardinality.** All 11 rustc variants?
   Curated subset?
2. **PointerCoercion cardinality.** All 6 sub-variants?
   Or drop the cleanup-discharged pair?
3. **CoercionSource handling.** Preserve the diagnostic-
   only `AsCast` / `Implicit` annotation, or drop?
4. **`hir::Safety` on `ReifyFnPointer` / `ClosureFnPointer`.**
   Preserve, or drop as redundant with destination
   `Ty::FnPtr`'s signature?

**Decision.** 11-variant `CastKind` + 4-variant
`PointerCoercion` sub-enum. Drop `CoercionSource` and
`hir::Safety` from `ReifyFnPointer` / `ClosureFnPointer`.

```rust
Cast(CastKind, Operand, TyId),

pub enum CastKind {
    PointerExposeProvenance,
    PointerWithExposedProvenance,
    PointerCoercion(PointerCoercion),
    IntToInt,
    FloatToInt,
    FloatToFloat,
    IntToFloat,
    PtrToPtr,
    FnPtrToPtr,
    Transmute,
    Subtype,
}

pub enum PointerCoercion {
    ReifyFnPointer,
    UnsafeFnPointer,
    ClosureFnPointer,
    Unsize,
}
```

Bridge translation rules (filed in §15 / §21.6):

- `CastKind::PointerCoercion(MutToConstPointer | ArrayToPointer, _)`
  -> reject (cleanup-discharged via D-7-3).
- `CastKind::PointerCoercion(p, _coercion_source)` -> drop
  `CoercionSource`.
- `PointerCoercion::ReifyFnPointer(_safety)` /
  `ClosureFnPointer(_safety)` -> drop `hir::Safety`.

**Reasoning.** Four threads:

1. **CastKind cardinality.** All 11 rustc variants are
   reachable in `optimized_mir`:
   `PointerExposeProvenance` / `PointerWithExposedProvenance`
   (stable address-exposure semantics);
   `PointerCoercion` (function pointer reification, unsizing);
   `IntToInt` / `FloatToInt` / `FloatToFloat` / `IntToFloat`
   / `PtrToPtr` / `FnPtrToPtr` (arithmetic and pointer
   conversions); `Transmute` (`mem::transmute` survives);
   `Subtype` (added by `Subtyper` runtime-lowering pass at
   `rustc_mir_transform/src/lib.rs:644`). Drivers exercise
   most of these; aggressive curation would reject valid
   code.
2. **PointerCoercion cardinality.** Rustc's 6-variant
   `PointerCoercion` (`rustc_middle/src/ty/adjustment.rs:11-40`)
   has two cleanup-discharged members: `MutToConstPointer`
   and `ArrayToPointer` are rewritten to `CastKind::PtrToPtr`
   by required `CleanupPostBorrowck`
   (`rustc_mir_transform/src/cleanup_post_borrowck.rs:44-60`).
   Codegen also rejects them with `bug!` at
   `rustc_codegen_ssa/src/mir/rvalue.rs:448-452`. Twice-
   confirmed unreachable -- drop, send to D-7-3 rejection.
   The remaining 4 (`ReifyFnPointer`, `UnsafeFnPointer`,
   `ClosureFnPointer`, `Unsize`) survive and have C-shape
   consequences (function-pointer reification, fat-pointer
   unsizing).
3. **CoercionSource drop.** `rustc_middle/src/mir/syntax.rs:1531-1536`
   defines `CoercionSource` as a 2-variant enum
   (`AsCast` / `Implicit`). Its only purpose is diagnostic
   -- to distinguish "user wrote `x as T`" from "compiler
   inserted the coercion." Codegen at
   `rustc_codegen_ssa/src/mir/rvalue.rs:408-447` ignores it
   entirely. The diagnostic value is recoverable at
   bridge-rejection time via the containing Statement's
   `SourceInfo` span (stage 8); operand-internal
   provenance tags add no information stage-8 spans don't
   already carry.
4. **`hir::Safety` drop from ReifyFnPointer / ClosureFnPointer.**
   The variant carries the *source* fn-item's safety
   (`Safe` vs `Unsafe`). The result type (the destination
   `Ty<'tcx>` carried in `Cast`'s third slot) is a
   `Ty::FnPtr` whose `FnSig` already encodes the
   *destination* safety (per stage 3 §14: `FnSig` content
   includes `FnAbi` and safety). Source-safety and
   destination-safety are equivalent for these PointerCoercion
   variants -- the cast preserves safety. Carrying both is
   structural duplication; the type-system has the answer
   via `Ty::FnPtr(FnSigId)`.

The 11+4 surface is the smallest closed set covering the
reachable post-cleanup content; the two annotation drops
(`CoercionSource`, `hir::Safety`) follow the D-6-1.3 pattern
of dropping carriers whose only meaning is in a domain we
do not separately model (diagnostic origin; type-system
duplication).

**Methodology rules invoked.** "Closed `Rvalue`
vocabulary" (`../memory.md` §2 stage-7-specific principle:
smallest reachable set; specializes README §2 "core is
opaque" rule's "items outside the recognized vocabulary
are rejected" pattern). "Cleanup-pass artefacts are
reject-on-encounter, not variants" (`../memory.md` §2
stage-7-specific corollary: `MutToConstPointer` /
`ArrayToPointer` -> D-7-3). "Compositional behavior beats
special-case flags" (`../README.md` §2: `hir::Safety` is
in the destination `Ty::FnPtr`; do not duplicate).
"Re-derive from evidence; do not import baggage"
(`../README.md` §2: `CoercionSource` is rustc-internal
diagnostic provenance not load-bearing post-bridge).

**Prior-art evidence.**

- `prior-art/rustc.md` §7 (CastKind 11-variant enum;
  PointerCoercion 6-variant enum; `CoercionSource`
  diagnostic-only doc; `MutToConstPointer` / `ArrayToPointer`
  CleanupPostBorrowck discharge + codegen `bug!`).
- `prior-art/mrustc.md` §4 (mrustc's `Cast` carries no
  `CastKind` discriminator -- a structurally different
  choice; the cast operation is implicit in the destination
  type. Validates that some peers go *flatter* than
  RegseqModel's 11+4 surface; we keep CastKind because the
  operation kind affects backend codegen paths).
- `prior-art/charon.md` §5 (Charon folds `Cast` into
  `UnaryOp::Cast(...)` and has 6 effective CastKind variants;
  structurally divergent from RegseqModel's choice but
  validates that closed-CastKind subsets are workable).

**Worked example.** `let f: fn(u32) -> u32 = my_fn;`
produces rustc MIR `Rvalue::Cast(CastKind::PointerCoercion(
PointerCoercion::ReifyFnPointer(Safety::Safe),
CoercionSource::Implicit), Operand::Constant(<my_fn DefId>),
<destination Ty: fn(u32) -> u32>)`. Translation:

```text
mir::Rvalue::Cast(CastKind::PointerCoercion(pc, _coercion_source),
                  operand, dest_ty)
  -> drop CoercionSource
  -> match pc:
       PointerCoercion::ReifyFnPointer(_safety) -> drop hir::Safety
       PointerCoercion::ClosureFnPointer(_safety) -> drop hir::Safety
       PointerCoercion::UnsafeFnPointer / Unsize -> as-is
       PointerCoercion::MutToConstPointer / ArrayToPointer -> reject (D-7-3)
  -> dest_ty -> translate_ty -> TyId
  -> Rvalue::Cast(CastKind::PointerCoercion(PointerCoercion::ReifyFnPointer),
                  translate_operand(operand), TyId)
```

A would-be `Cast(CastKind::PointerCoercion(MutToConstPointer,
_), _, _)` -- which CleanupPostBorrowck has already rewritten
to `PtrToPtr` before `optimized_mir` -- triggers D-7-3
rejection if hand-crafted IR includes it.

**Cross-refs.** `spec-snapshot.md` §21.1 (Cast variant
docstring), §21.3.3 (CastKind / PointerCoercion sub-enums),
§21.6.3 (bridge translation -- CoercionSource drop, hir::Safety
drop, PointerCoercion::{MutToConstPointer,ArrayToPointer}
rejection).

---

### D-7-1.7 -- `BinaryOp` + `UnaryOp` payloads + operand-pair storage [LOCKED]

**Question.** Three sub-questions: (1) `BinOp` variant set,
(2) `UnOp` variant set, (3) `BinaryOp` operand-pair storage
(`Box<(Operand, Operand)>` vs flat).

**Key options.** For the 26-variant `BinOp` and 3-variant
`UnOp`, the question is curate vs preserve. For storage:

1. **Option A -- flat: `BinaryOp(BinOp, Operand, Operand)`.**
   Charon precedent. Larger enum; zero allocation per
   BinaryOp.
2. **Option B -- boxed: `BinaryOp(BinOp, Box<(Operand,
   Operand)>)`.** rustc precedent. Smaller enum; one extra
   allocation per BinaryOp.

**Decision.** Preserve all 26 `BinOp` variants and all 3
`UnOp` variants verbatim. Flat operand-pair storage:

```rust
BinaryOp(BinOp, Operand, Operand),
UnaryOp(UnOp, Operand),

pub enum BinOp {
    // 26 variants verbatim from rustc:
    Add, AddUnchecked, AddWithOverflow,
    Sub, SubUnchecked, SubWithOverflow,
    Mul, MulUnchecked, MulWithOverflow,
    Div, Rem,
    BitXor, BitAnd, BitOr,
    Shl, ShlUnchecked,
    Shr, ShrUnchecked,
    Eq, Lt, Le, Ne, Ge, Gt,
    Cmp,
    Offset,
}

pub enum UnOp {
    Not,
    Neg,
    PtrMetadata,
}
```

**Reasoning.** Three threads:

**BinOp preservation.** Each of rustc's 26 variants
(`rustc_middle/src/mir/syntax.rs:1615-1703`) maps to a
distinct C lowering or runtime helper:

- Wrapping arith (`Add`/`Sub`/`Mul`/`Div`/`Rem`) -> C
  operator with explicit unsigned-wrapping for unsigned
  types.
- UB-on-overflow (`AddUnchecked`/`SubUnchecked`/`MulUnchecked`,
  emitted by `unchecked_add`/`unchecked_sub`/etc.
  intrinsics) -> C operator (UB-on-overflow lowering is a
  stage-14 backend concern, not a stage-7 spec concern).
- Overflow-detecting (`AddWithOverflow`/`SubWithOverflow`/
  `MulWithOverflow`, emitted by overflow-check lowering;
  return `(T, bool)`) -> C-helper-style intrinsic returning
  a 2-field struct.
- Bitwise (`BitXor`/`BitAnd`/`BitOr`) -> C operator.
- Shifts (`Shl`/`ShlUnchecked`/`Shr`/`ShrUnchecked`) -> C
  operator with masking per `RHS.rem_euclid(LHS::BITS)`
  semantics.
- Comparisons (`Eq`/`Lt`/`Le`/`Ne`/`Ge`/`Gt`) -> C operator.
- Three-way comparison (`Cmp`, returns
  `core::cmp::Ordering` as `i8`) -> conditional ladder.
- Pointer offset (`Offset`, `<*const T>::offset` semantics)
  -> C pointer arithmetic.

Each has a stage-14 backend lowering; collapsing any pair
(e.g., merging `Add` and `AddUnchecked`) would lose
information rustc paid to compute. Preserve verbatim.

**UnOp preservation.** All 3 variants reach `optimized_mir`:
`Not` (logical inversion `!`), `Neg` (arithmetic negation
`-`), `PtrMetadata` (extracts metadata from a fat pointer).
`PtrMetadata` is `MirPhase::Runtime`-only
(`rustc_middle/src/mir/syntax.rs:1609`); `optimized_mir` IS
Runtime; reachable. Each has a direct C lowering. Preserve.

**Pair storage (flat over boxed).** rustc's
`Box<(Operand<'tcx>, Operand<'tcx>)>` is a size-budget
optimisation rustc needs and we don't:

- rustc's `static_assert_size!(Rvalue<'_>, 40)` at
  `rustc_middle/src/mir/syntax.rs:1752` reflects rustc's
  body-iteration access pattern (multiple compiler passes
  over the same body; cache pressure is real).
- RegseqModel constructs bodies once at bridge time and
  consumes them once at codegen time; per-Rvalue size is
  not load-bearing at our scale.
- Charon stores `BinaryOp(BinOp, Operand, Operand)` flat
  (`prior-art/charon.md` §6); validates that flat works at
  production scale for a verification frontend with similar
  body-construction-then-consumption access pattern.
- Pattern-match ergonomics: `Rvalue::BinaryOp(op, lhs, rhs)
  =>` is cleaner than `Rvalue::BinaryOp(op, box (lhs, rhs))
  =>`. Stage-14 backend code reads better.

After stage 6's `Operand::Constant` shrinkage to bare
`ConstId` (D-6-1.3), stage-7's `Operand` is dominated by
`Place` (~24 bytes). Two operands flat ~48 bytes payload +
tag puts the BinaryOp variant around 56 bytes; with Box,
the variant is around 16 bytes but the Rvalue enum's max
variant is dominated by other variants (e.g.,
`Cast(CastKind, Operand, TyId)` ~32-36 bytes) so the
Rvalue's overall enum size goes from ~40 (boxed) to ~56
(flat). At hundreds of Rvalues per body and bodies
accessed twice, the total memory delta is in the noise.

The size-budget micro-optimisation rustc needs does not
apply to RegseqModel; the allocation cost rustc pays
(one Box per BinaryOp) is the cost of the optimisation.
Stage 6's "drop Box where possible" pattern (D-6-1.3 dropped
`Box<ConstOperand>` to inline `ConstId`) extends here.

**Methodology rules invoked.** "C semantics with information
preservation" (preserve every BinOp / UnOp variant -- each
informs distinct C lowering); "Re-derive from evidence; do
not import baggage" (rustc's Box is a rustc-internal size-
budget concern; do not import); "Compositional behavior
beats special-case flags" (the flat-storage choice extends
D-6-1.3's pattern of "inline when possible").

**Prior-art evidence.**

- `prior-art/rustc.md` §8 (BinaryOp Box-pair, BinOp 26-variant
  enum, size-assert), §9 (UnaryOp, UnOp 3-variant enum,
  PtrMetadata Runtime-phase qualifier).
- `prior-art/mrustc.md` §5 (mrustc's `BinOp` operands stored
  flat; peer-validated -- no Box at the C-target level), §6
  (`UniOp` operand: LValue, not Param -- a different
  structural choice we don't follow), §7 (mrustc's `eBinOp`:
  21 variants, simpler than rustc -- mrustc lacks
  `*WithOverflow` / `*Unchecked` because the C target
  doesn't model overflow-detecting arith at the IR level;
  validates that 26 is reasonable for a richer-arith C
  target where overflow checks matter).
- `prior-art/charon.md` §6 (Charon's `BinaryOp(BinOp, Operand,
  Operand)` flat; UnOp `Not` / `Neg` / `Cast` -- Charon
  folds Cast into UnOp, structurally divergent; preserves
  flat-pair storage).

**Worked example.** `a.wrapping_add(b)` produces rustc MIR
`Rvalue::BinaryOp(BinOp::Add, box (Operand::Copy(_a),
Operand::Copy(_b)))`. Translation:

```text
mir::Rvalue::BinaryOp(op, box (lhs, rhs))
  -> Rvalue::BinaryOp(op, translate_operand(lhs), translate_operand(rhs))
  -> Rvalue::BinaryOp(BinOp::Add, Operand::Copy(_a), Operand::Copy(_b))
```

The Box is unwrapped at bridge time; downstream stages see
flat operand positions.

**Cross-refs.** `spec-snapshot.md` §21.1 (BinaryOp / UnaryOp
variant docstrings), §21.3.4 / §21.3.5 (BinOp / UnOp
sub-enums), §21.6 (bridge translation -- Box unwrap rule).

---

### D-7-1.8 -- `Discriminant` + `Aggregate` payloads + `AggregateKind` subset [LOCKED]

**Question.** Three threads woven: (1) `Discriminant`
payload, (2) `Aggregate` operand-vec storage, (3)
`AggregateKind` subset including the Adt-encoding
question, the Coroutine-variant fate, and the
`UserTypeAnnotationIndex` slot.

**Key options.** Major options on the AggregateKind shape:

1. **Option A -- single Adt variant, rustc-shaped.**
   `Adt(TyId, VariantIdx, Option<FieldIdx>)`. rustc's
   five-slot encoding minus `(DefId, GenericArgsRef)`
   collapse and `UserTypeAnnotationIndex` drop. Carries
   possibly-invalid combinations (e.g., `Enum(TyId, 0,
   Some(field))` is structurally representable but
   semantically invalid).
2. **Option B -- three variants matching stage-3
   NominalKind.** `Struct(TyId)` / `Enum(TyId, VariantIdx)`
   / `Union(TyId, FieldIdx)`. Compositional with stage 3's
   `NominalKind { Struct, Enum, Union, FnDef, Closure }`
   (`spec.md:2896`) and the parallel `Ty::Struct` /
   `Ty::Enum` / `Ty::Union` translation table
   (`spec.md:4555-4557`). No representable invalid states.

**Decision.** Discriminant: direct embed. Aggregate
operand storage: `Box<[Operand]>` (matches stage-5 PlaceElem
storage convention). AggregateKind: 7-variant subset under
**Option B** (three-way Adt split), with `Tuple(TyId)` for
symmetric type-naming across all 7 variants:

```rust
Discriminant(Place),
Aggregate(AggregateKind, Box<[Operand]>),

pub enum AggregateKind {
    Array(TyId),                       // destination array type Ty::Array(elem, n)
    Tuple(TyId),                       // destination tuple type Ty::Tuple { fields, layout }
    Struct(TyId),                      // TyId resolves to Ty::Struct
    Enum(TyId, VariantIdx),            // TyId resolves to Ty::Enum; active variant
    Union(TyId, FieldIdx),             // TyId resolves to Ty::Union; active field
    Closure(TyId),                     // TyId resolves to Ty::Closure (identity view per §13.1)
    RawPtr(TyId, Mutability),          // pointee TyId; mutability ties to Ty::Ptr discriminator
}
```

Bridge translation rules (filed in §15 / §21.6):

- `Adt(did, vidx, args, _user_ty, active_field)` ->
  - struct (per `tcx.adt_def(did).is_struct()`):
    `Struct(<TyId of Ty::Struct{translate(did), translate(args), layout}>)`
  - enum (per `is_enum()`): `Enum(<TyId>, vidx)`
  - union (per `is_union()`): `Union(<TyId>, active_field.unwrap())`
  Drop the `Option<UserTypeAnnotationIndex>` slot.
- `Closure(did, args)` -> `Closure(<TyId of
  Ty::Closure(translate(did), translate(args))>)` --
  identity view, per §13.1's two-views design.
- `Coroutine(...)` / `CoroutineClosure(...)` -> reject
  (D-7-3); double-grounded by §15.1 type-level rejection
  (`spec.md:4581`) plus StateTransform discharge.
- `Tuple` -> `Tuple(translate(rvalue.ty(body, tcx)))`.
  rustc's `Aggregate(Tuple, ops)` does not carry a type
  slot; the bridge obtains the destination tuple type via
  `rvalue.ty(body, tcx)` (rustc's MIR Rvalue type-of
  helper). Translation routes through stage-3
  `Ty::Tuple { fields, layout }` interning where
  `fields = elem_tys` and `layout` is sourced from
  `tcx.layout_of` per §15 (parallel to stage-3's tuple
  translation rule at `spec.md:4530`).

**Reasoning.** Five threads, organised by structural area:

1. **Discriminant direct.** `Rvalue::Discriminant(Place)`
   reads the discriminant of an enum place and returns it
   as an integer of type `discriminant_ty`. The payload is
   trivially a `Place`; no curation.

2. **Operand-vec storage.** rustc's `IndexVec<FieldIdx,
   Operand>` is a `Vec` with a typed index; the FieldIdx
   typing is purely cosmetic. Stage-5 D-5-1 chose
   `Box<[PlaceElem]>` for projection storage (immutable
   IR; saves the `Vec` capacity word). Stage 7 follows
   the same convention for AggregateKind operands.

3. **Adt three-way split (Option B).** rustc's encoding
   (`rustc_middle/src/mir/syntax.rs:1545-1550`) is
   "three-states-in-five-slots":
   - Struct: `VariantIdx == FIRST_VARIANT (0)`,
     `active_field == None`.
   - Enum: `VariantIdx == <active variant>`,
     `active_field == None`.
   - Union: `VariantIdx == FIRST_VARIANT (0)`,
     `active_field == Some(<field>)`.
   The five-slot single-variant encoding admits
   structurally-representable-but-semantically-invalid
   combinations (e.g., `Adt(<enum TyId>, 0, Some(field))`).
   Stage-3's type taxonomy already separates the three
   nominal kinds: `Ty::Struct { item, args, layout }` /
   `Ty::Enum { item, args, layout }` / `Ty::Union { item,
   args, layout }` (`spec.md:4555-4557`); the
   `NominalKind` enum (`spec.md:2896`: `Struct, Enum,
   Union, FnDef, Closure`) is the canonical discriminator.
   The three-way `AggregateKind` split aligns 1:1 with the
   type-side discriminator -- compositional with stage 3,
   no representable invalid states.

4. **Adt collapse via TyId.** rustc's
   `Adt(DefId, VariantIdx, GenericArgsRef, ...)` carries a
   `(DefId, GenericArgsRef)` pair that identifies a
   monomorphised Adt. Stage 3's `Ty::Struct`/`Enum`/`Union`
   already store `(item, args, layout)` for the same
   monomorphised Adt; both encodings carry the *same*
   information. Replacing the `(DefId, GenericArgsRef)` pair
   with a `TyId` resolving to `Ty::Struct` / `Ty::Enum` /
   `Ty::Union` composes through stage-3's type interner
   without information loss. The Adt's `AdtFlags` cache
   (`spec.md:1213-1304`) is also reachable through the type
   interner. Stage-3's `find_nominal(NominalKind, item,
   args)` lookup (`spec.md:3306, 4968, 5188`) is the bridge
   construction primitive that produces the TyId in the
   first place; AggregateKind references the result.

5. **Closure identity view.** §13.1 (`spec.md:3231,
   3237-3304`) pins each closure as TWO `Ty` entries
   sharing one synthesised Item:
   - `Ty::Closure(item, args)` -- closure-type-identity
     view (canonical form).
   - `Ty::Struct(synthesised_item, args, layout)` -- the
     parallel captures-struct layout view, reached via
     `find_nominal(NominalKind::Struct, item, args)`.
   AggregateKind::Closure references the *identity view*
   (`Ty::Closure`); the layout view is reachable for
   codegen via the parallel-lookup at stage-14 backend
   time. This preserves "what kind of construction is
   this" in the AggregateKind tag while the type-system
   handles the layout-view duplication.

6. **Coroutine drop (double-grounded).** §15.1 type-level
   rejection table (`spec.md:4581`) already rejects
   `Coroutine` / `CoroutineClosure` / `CoroutineWitness`
   at type translation: "async / coroutine state machines;
   outside `#![no_std]` driver DSL." The corresponding
   AggregateKind variants would construct values of
   `Ty::Coroutine`/etc. -- types stage 3 rejects. By
   composition, the AggregateKind variants are unreachable
   in valid IR. Independently, the `StateTransform`
   runtime-lowering pass (`rustc_mir_transform/src/lib.rs:660`)
   rewrites coroutine bodies into state-machine struct
   construction; `rustc_middle/src/mir/syntax.rs:1457-1458`
   states "After coroutine lowering, `Coroutine` aggregate
   kinds are disallowed too." Two independent paths to
   unreachability -- the type-level rejection is the
   stronger, harder wall.

7. **UserTypeAnnotationIndex drop (D-6-1.3 precedent).**
   rustc's `AggregateKind::Adt(_, _, _, Option<UserTypeAnnotationIndex>,
   _)` 4th slot is an NLL/borrowck-only carrier. Required
   `CleanupPostBorrowck` clears the body-level annotation
   table (`body.user_type_annotations.raw.clear()`,
   `cleanup_post_borrowck.rs:81-85`); the codegen consumer
   (`rustc_codegen_ssa/src/mir/rvalue.rs:670-672`) ignores
   the slot. Same drop pattern as D-6-1.3 (which dropped
   `user_ty` from `ConstOperand`).

8. **Tuple(TyId) for symmetry.** rustc's `AggregateKind::Tuple`
   is the only zero-payload variant; the tuple's destination
   type is reconstructable from operand types but doing so
   at the C backend means running type-of inference at
   lowering time. Carrying `TyId` makes the destination type
   explicit. Stage 3's `Ty::Tuple { fields, layout }` is
   interned (where `fields = elem_tys` is the translated
   element list and `layout` is sourced from `tcx.layout_of`
   per `spec.md:4530`); the bridge can supply the TyId at
   translation time. This
   makes the AggregateKind surface uniformly "every variant
   names its destination TyId" -- a structural symmetry
   that matches stage-7's compositional preferences.

**Methodology rules invoked.** "Compositional behavior
beats special-case flags" (`../README.md` §2: three-way
Adt split via stage-3 NominalKind; closure identity view
via §13.1; Coroutine drop via §15.1 type-level rejection).
"Closed `Rvalue` vocabulary" (`../memory.md` §2
stage-7-specific principle: smallest reachable set; no
Coroutine/CoroutineClosure). "C semantics with
information preservation" (`../README.md` §2: carry TyId
for every construction; drop UserTypeAnnotationIndex
which has no C-shape consequence). "Re-derive from
evidence; do not import baggage" (`../README.md` §2:
rustc's three-states-in-five-slots Adt encoding is
rustc-internal; re-derive cleaner).

**Prior-art evidence.**

- `prior-art/rustc.md` §10 (Discriminant payload, Aggregate
  payload, AggregateKind 7-variant enum, size_asserts),
  §13 (cleanup-pass whitelist context for
  UserTypeAnnotationIndex clearing).
- `prior-art/mrustc.md` §10 (mrustc's aggregate kinds split
  per-shape: `MakeTuple`/`MakeArray`/`MakeStruct`/`MakeEnumVariant`
  -- a peer that already adopted the per-shape split, validating
  Option B; mrustc has no closure-construction at this level
  because mrustc's closures are pre-lifted; mrustc has no
  Coroutine analog).
- `prior-art/charon.md` §9 (Charon's `AggregateKind`:
  3-variant peer surface -- `Aggregate { kind, fields }`
  with `kind: AggregateKind = Adt | Array | Closure`;
  Charon's Adt covers Struct/Enum/Union via the Ty's
  NominalKind discriminator. Validates that closed
  AggregateKind subsets are workable; structurally less
  granular than RegseqModel's 7-variant choice).

**Worked example.** A struct construction
`Foo { x: 1, y: 2 }`:

```text
mir::Rvalue::Aggregate(box AggregateKind::Adt(<Foo did>,
                            FIRST_VARIANT, <args>, None, None),
                       [Operand::Constant(<1>), Operand::Constant(<2>)])
  -> dispatch on tcx.adt_def(<Foo did>).is_struct() -> true
  -> look up TyId of Ty::Struct{<translated did>, <translated args>, layout}
       via find_nominal(NominalKind::Struct, item, args)
  -> drop UserTypeAnnotationIndex (cleared by CleanupPostBorrowck)
  -> drop Option<FieldIdx> active_field (None for struct)
  -> Rvalue::Aggregate(AggregateKind::Struct(TyId),
       Box::new([Operand::Constant(<1>), Operand::Constant(<2>)]))
```

A closure expression `|x| x + cap`:

```text
mir::Rvalue::Aggregate(box AggregateKind::Closure(<closure did>, <args>),
                       [Operand::Copy(<cap_local>)])
  -> look up TyId of Ty::Closure(<translated did>, <translated args>)
       (identity view per §13.1)
  -> Rvalue::Aggregate(AggregateKind::Closure(TyId),
       Box::new([Operand::Copy(<cap_local>)]))
```

The captures-struct layout view (`Ty::Struct` per §13.1) is
reachable for codegen at stage 14 via
`find_nominal(NominalKind::Struct, item, args)`.

A would-be coroutine construction
`AggregateKind::Coroutine(<async_block_did>, <args>)`
triggers D-7-3 rejection if hand-crafted IR includes it;
in practice, both §15.1 type-level rejection
(`spec.md:4581`) and `StateTransform` runtime-lowering
remove it before `optimized_mir`.

**Cross-refs.** `spec-snapshot.md` §21.1 (Discriminant /
Aggregate variant docstrings), §21.3.6 (AggregateKind
sub-enum), §21.6.4 (bridge translation -- Adt three-way
dispatch, UserTypeAnnotationIndex drop, Coroutine
rejection, Closure identity-view selection, Tuple TyId
materialisation), §13.1 (closure-family two-views design;
referenced for the identity-view rule).

---

### D-7-2 -- Rvalue storage and interning [LOCKED]

**Question.** Where does an `Rvalue` live in the model --
inline at use sites, or via an `RvalueId` indirection into
a top-level interner?

**Key options.**

1. **Option A -- inline at use sites.** `Rvalue` is
   embedded by value into stage-8 `Statement::Assign(Place,
   Rvalue)` and stage-9 terminator payloads (transitively).
   No `RvalueId`, no interner.
2. **Option B -- `RvalueId` interner.** Mirror stage 4's
   `Const` interning; introduce `RvalueInterner` and
   reference rvalues by `RvalueId`.

**Decision.** Option A. Rvalues live inline at use sites;
no `RvalueId`; no rvalue-level interner.

**Reasoning.** Two concrete arguments against interning:

1. **No downstream consumer of rvalue-content-uniqueness.**
   `Const` interning exists because `Const`-content-identity
   matters for downstream consumers (stage-3
   `GenericArg::Const(ConstId)` participates in `Ty`-identity;
   stage-9 instance-key construction reads `ConstId`s; F4-2
   cross-build identity threads through them). Rvalues have
   no analogous consumer -- no IR structure consumes "two
   rvalue uses are content-equivalent."
2. **Heterogeneous, larger payloads.** Rvalue variants are
   structurally heterogeneous and larger than Operand (the
   max payload is `BinaryOp(BinOp, Operand, Operand)` after
   D-7-1.7's flat-pair choice, ~56 bytes). Interning's
   overhead (a hash lookup at construction; ID dereference
   at use) buys nothing without a content-uniqueness
   consumer.

This is the same reasoning as D-5-2 (places) and D-6-2
(operands). Stages 5, 6, 7 all conclude inline; the chain
is consistent.

**Methodology rules invoked.** "Compiler-theory-first"
(interning is a deduplication optimisation; without a
deduplication consumer, it's premature abstraction); "Eager
translation" (Q2) does not require interning; it requires
fully-owned IR, which inline-by-value satisfies trivially.

**Prior-art evidence.** rustc's MIR has no rvalue interner
(`prior-art/rustc.md` §1 and §14). `rustc_public` follows
the same pattern (`prior-art/rustc.md` §12). mrustc inlines
at use sites (`prior-art/mrustc.md` §1). Charon has a
generic `HashConsed<T>` infrastructure but does not apply
it to Rvalue (`prior-art/charon.md` §1, §11). Four-out-of-
four peers store inline.

**Worked example.** A function with two consecutive
statements `_2 = _1 + 1` and `_3 = _1 + 1` produces MIR with
two `Rvalue::BinaryOp(BinOp::Add, Operand::Copy(_1),
Operand::Constant(<1>))` rvalue slots. Under Option B, both
would resolve to the same `RvalueId` via interning -- a
single deduplication step saving ~56 bytes per repeat.
Under Option A (this decision), each `StatementKind::Assign`
carries its own `Rvalue::BinaryOp(...)` inline. The "saved"
~56 bytes per duplicate is dwarfed by the cost of a
parallel `RvalueInterner` table, the `RvalueId` type and
serde derives, and the indirection at every use site -- all
to deduplicate something no downstream consumer asks
about. Inline storage is strictly cheaper.

F4-2 cross-build identity is OUT of scope for Rvalue (per
D-7-3 part D): rvalue is body-local content; the F4-2
instance-ordering canonicalisation reads `(InstanceKey)`,
not `(BodyContents)`.

**Cross-refs.** `spec-snapshot.md` §21.4 (storage and
identity), parallel to §19.2 (Place storage) and §20.2
(Operand storage).

---

### D-7-3 -- Rvalue-site invariants [LOCKED]

**Question.** What invariants does the bridge enforce at
rvalue construction? Specifically: which dropped variants
and discharged-but-representable sub-discriminator values
are rejected, and what positive well-formedness invariants
hold?

**Key options.** Whether to enforce per-variant rejection
rules at bridge time (Option A) or defer to stage 11 walk
(Option B). Since the dropped variants do not exist in the
RegseqModel `Rvalue` enum, "rejection" is structural by
type (the bridge's `translate_rvalue` returns
`BridgeError::UnsupportedConstruct` rather than
constructing a non-existent variant). Sub-discriminator
rejection (Fake / ArrayToPointer / MutToConstPointer /
Coroutine kinds) is enforced at bridge translation time.

**Decision.** Bridge-time rejection for all dropped
constructs (Part A). Positive invariants specified at
stage 7 (Part B); walk-enforcement implementation deferred
to stage 11's body-level traversal (Part C; same split as
D-5-2 places). F4 / F4-2 cross-build identity OUT of
scope (Part D).

### Part A: Rejection rules (cleanup-pass-discharged or unreachable constructs)

| Construct | Rejection ground | Source |
| --------- | ---------------- | ------ |
| `mir::Rvalue::CopyForDeref(_)` | Eliminated by required `EraseDerefTemps` runtime-lowering pass (listed at `rustc_mir_transform/src/lib.rs:658`; `is_required(): true` at `rustc_mir_transform/src/erase_deref_temps.rs:38`); codegen `bug!()`s (`rustc_codegen_ssa/src/mir/rvalue.rs:712`) | D-7-1.1 |
| `mir::Rvalue::ShallowInitBox(_, _)` | Eliminated by required `ElaborateBoxDerefs` runtime-lowering pass (listed at `rustc_mir_transform/src/lib.rs:659`; `is_required(): true` at `rustc_mir_transform/src/elaborate_box_derefs.rs:224`); codegen `bug!()`s (`rustc_codegen_ssa/src/mir/rvalue.rs:713`) | D-7-1.1 |
| `mir::Rvalue::ThreadLocalRef(_)` | DSL-surface claim: `#[thread_local]` cannot produce a working driver on Linux/Zephyr targets; "No defensive fallbacks" rule | D-7-1.1 |
| `mir::Rvalue::WrapUnsafeBinder(_, _)` | §15.1 Type-level rejection table (`spec.md:4583`) rejects `UnsafeBinder` at type translation; rvalue-level rejection cascades by composition | D-7-1.1 |
| `mir::Rvalue::Ref(_, BorrowKind::Fake(_), _)` | NOPed by required `CleanupPostBorrowck` (NOP code at `rustc_mir_transform/src/cleanup_post_borrowck.rs:33-43`; `is_required(): true` at `rustc_mir_transform/src/cleanup_post_borrowck.rs:88`) | D-7-1.4 |
| `mir::Rvalue::Cast(CastKind::PointerCoercion(MutToConstPointer \| ArrayToPointer, _), _, _)` | Rewritten to `PtrToPtr` by required `CleanupPostBorrowck` (rewrite code at `rustc_mir_transform/src/cleanup_post_borrowck.rs:44-60`; `is_required(): true` at `rustc_mir_transform/src/cleanup_post_borrowck.rs:88`) | D-7-1.6 |
| `mir::Rvalue::Aggregate(box AggregateKind::Coroutine(_, _) \| CoroutineClosure(_, _), _)` | Type-level rejection in §15.1 (`spec.md:4581`) plus `StateTransform` runtime-lowering discharge | D-7-1.8 |

Diagnostic per row: `unsupported_construct!` style, naming
the discharging mechanism so the driver author sees a
useful error message (e.g., "thread-local statics not
supported in driver code (target form-factor lacks TLS
infrastructure)" for ThreadLocalRef).

### Part B: Positive invariants (well-formedness rules; walk-enforcement deferred to stage 11)

- **I-Rvalue-Operand-Wellformed.** Every embedded
  `Operand` slot satisfies stage-6 D-6-3 invariants
  (inherited via §20.x I-Operand-Place-Validity at
  `spec.md:5833`). Every embedded `Place` slot satisfies
  stage-5 D-5-2 invariants (inherited via §19.7 Place
  validation at `spec.md:5527`).
- **I-Rvalue-TyId-Interned.** Every `TyId` slot (in
  `Cast`, `AggregateKind::Array` / `Tuple` / `Struct` /
  `Enum` / `Union` / `Closure` / `RawPtr`) resolves through
  stage-3's type interner (the same primitive that
  `find_nominal(NominalKind::Struct, item, args)` uses at
  `spec.md:3306, 4968, 5188`).
- **I-Rvalue-Aggregate-Operand-Count.** Number of
  operands matches the destination type's structural shape:
  - `Array(TyId)`: count == `n` for `Ty::Array(_, n)`.
  - `Tuple(TyId)`: count == `fields.len()` for
    `Ty::Tuple { fields, layout }`.
  - `Struct(TyId)`: count == struct field count.
  - `Enum(TyId, VariantIdx)`: count == named-variant field
    count.
  - `Union(TyId, FieldIdx)`: count == 1 (unions construct
    exactly one active field).
  - `Closure(TyId)`: count == captures-struct field count
    (reached via parallel `Ty::Struct` view per §13.1).
  - `RawPtr(TyId, Mutability)`: count == 2 (`(data_ptr,
    metadata)` -- per `rustc_middle/src/mir/syntax.rs:1561-1566`,
    even thin pointers carry both operands; the unit
    metadata for thin pointers is still required).
- **I-Rvalue-Repeat-Count-Wellformed.** `Repeat(Operand,
  count)` count is `u64`; trivially non-negative
  (unsigned).
- **I-Rvalue-VariantIdx-Wellformed.** `Enum(TyId,
  VariantIdx)`'s VariantIdx is in range `[0, variant_count)`
  of the enum (parallel to D-5-1's PlaceElem::Downcast(VariantIdx)
  invariant).
- **I-Rvalue-FieldIdx-Wellformed.** `Union(TyId,
  FieldIdx)`'s FieldIdx is in range `[0, field_count)` of
  the union.

### Part C: Walk-implementation placement

Rules at stage 7 (this section); walk implementation at
stage 11 (with the body-walker that visits
`Statement::Assign(Place, Rvalue)`). Same split as D-5-2
for Place validation (rules at stage 5; walk at stage 11).

### Part D: F4 / F4-2 cross-build identity -- OUT of scope

Rvalue is body-local content; does not participate in
cross-build identity. The F4-2 instance ordering
canonicalises `(InstanceKey)`, not `(BodyContents)`. Same
scope reduction as D-5-2 and D-6-3.

**Reasoning.** Each rejection rule has an explicit
discharge path documented in the prior-art file referenced;
the table is a consolidation, not a new claim. Positive
invariants follow the principle "enforce structural
well-formedness; trust upstream-discharged semantic
invariants" -- rustc has already discharged type-coherence
checks (drop-elaboration, NLL, borrowck) before
`optimized_mir`; the bridge enforces what is structurally
visible in the IR (operand count, index bounds, TyId
interner-membership) and inherits what stages 5/6 already
enforce. Walk-implementation placement at stage 11 mirrors
stage-5's split: rules can be specified standalone, but the
walker that exercises them needs `Body` machinery (stage
11). F4-2 OUT-of-scope mirrors stages 5/6 (body-local
content; no cross-build identity consumer).

**Methodology rules invoked.** "No defensive fallbacks"
(do not re-check what rustc has discharged); "Compositional
behavior beats special-case flags" (`../README.md` §2:
Rvalue invariants inherit from D-5-2 / D-6-3 transitively
for embedded Place / Operand slots; stage 7 adds only
structural shape invariants). "Cleanup-pass artefacts are
reject-on-encounter, not variants" (`../memory.md` §2
stage-7-specific corollary: the rejection table
implements this principle).

**Prior-art evidence.**

- `prior-art/rustc.md` §11 (codegen rejection sites for
  CopyForDeref / ShallowInitBox), §13 (cleanup-pass
  whitelist context: which discharges are required); §15
  D-7-10 mapping.
- `prior-art/mrustc.md` §11 D-7-3 (mrustc has no analogous
  rejection chain because mrustc lacks the upstream
  borrowck / cleanup-pass infrastructure that produces these
  constructs in the first place; not directly comparable).
- `prior-art/charon.md` §11 (Charon's translation-pass
  rejection matrix: `CopyForDeref` lowered to plain Copy;
  `ShallowInitBox` preserved structurally but never reached
  in optimized-MIR-derived input; `ThreadLocalRef` rejected;
  `WrapUnsafeBinder` rejected; Coroutine variants rejected).
  Validates the rejection-list structure against an
  independent peer.

**Worked example.** Hand-crafted IR carrying a fake borrow:

```text
StatementKind::Assign(_2,
    Rvalue::Ref(<region>, BorrowKind::Fake(FakeBorrowKind::Shallow), <place>))
```

Bridge translation:

```text
translate_rvalue(mir::Rvalue::Ref(_, BorrowKind::Fake(_), _))
  -> emit BridgeError::UnsupportedConstruct {
       construct: "Rvalue::Ref(BorrowKind::Fake(_))",
       discharge: "NOPed by required CleanupPostBorrowck",
       reference: "rustc_mir_transform/src/cleanup_post_borrowck.rs:33-43",
     }
```

The driver author sees a structured diagnostic naming both
the construct and the rustc mechanism that should have
discharged it; the IR builds to a rejected state rather
than carrying a representable but invalid variant.

A well-formed Aggregate construction:

```text
Rvalue::Aggregate(AggregateKind::Struct(<TyId of Foo>),
                  Box::new([Operand::Constant(<1>), Operand::Constant(<2>)]))
```

passes I-Rvalue-Aggregate-Operand-Count if `Foo` has 2
fields; passes I-Rvalue-TyId-Interned if `<TyId of Foo>`
resolves through the type interner; passes
I-Rvalue-Operand-Wellformed by inheritance from D-6-3 for
each operand.

**Cross-refs.** `spec-snapshot.md` §21.5 (rejection rules
+ positive invariants), §21.5.3 (walk-implementation
deferral note); §19.7 (inherited Place validation); §20.5
(inherited I-Operand-Place-Validity).

---

### D-7-K -- `pretty.rs::render_rvalue` deferral [LOCKED]

**Question.** Where does `pretty.rs::render_rvalue` land
-- stage 7 (alongside the Rvalue vocabulary), stage 11
(alongside `render_place` and `render_operand`), or stage
14 (alongside `render_const` and the renderer for the
remaining pieces)?

**Key options.**

1. **Option A -- stage 7 (immediate).** Land
   `render_rvalue` alongside the variant set. Requires
   sub-renderers `render_place` (stage 11 per D-5-K) and
   `render_operand` (stage 11 per D-6-K) and `render_const`
   (stage 14 per D-4-K) to be in place; they are not.
2. **Option B -- stage 11 (latest first-batch
   dependency).** Co-locate with `render_place` (D-5-K) and
   `render_operand` (D-6-K). The `Cast` / `Aggregate` arms
   may late-bind to `render_const` if needed for
   constant-payload sub-rendering, but most arms use
   only `render_place` / `render_operand`.
3. **Option C -- stage 14 (latest dependency overall).**
   Wait for `render_const` (stage 14) before landing.
   Maximally deferred.

**Decision.** Option B (stage 11). Filed as outbound
expectation in `stage-11/expectations.md` "From stage 7"
alongside the existing D-5-K `render_place` and D-6-K
`render_operand` deferrals.

**Reasoning.** Per-variant rendering input dependency:

| Variant | Rendering input | Stage of input |
| ------- | --------------- | -------------- |
| `Use(o)` | `render_operand(o, body)` | 11 |
| `Repeat(o, n)` | `render_operand(o, body)` + literal `u64` | 11 |
| `Ref(prov, p)` | `&` / `&mut` keyword (per `Provenance` discriminator) + `render_place(p, body)` | 11 |
| `RawPtr(mutbl, p)` | `&raw const` / `&raw mut` keyword (per `Mutability` discriminator) + `render_place(p, body)` | 11 |
| `Cast(ck, o, ty)` | `render_operand(o, body)` + CastKind name + `render_ty(ty)` | 11 (operand), 13 (ty) |
| `BinaryOp(op, l, r)` | BinOp name + `render_operand(l, body)` + `render_operand(r, body)` | 11 |
| `UnaryOp(op, o)` | UnOp name + `render_operand(o, body)` | 11 |
| `Discriminant(p)` | `discriminant(` + `render_place(p, body)` + `)` | 11 |
| `Aggregate(kind, ops)` | AggregateKind dispatch + per-element `render_operand` + per-variant field/variant names resolved directly from `kind`'s `TyId` via the §12 type interner (no head-local lookup needed -- every `AggregateKind` variant carries the destination TyId in its payload). What forces deferral to stage 11 is the operand-rendering subtree, not field-name resolution. | 11 |

The dominant input dependency is stage 11 (`render_place`
and `render_operand` both deferred there). The `Cast` arm's
`render_ty` is stage 13 (per D-3 §13 deferred D14 -- Ty
renderer at stage 13); `render_rvalue` late-binds to
`render_ty` once stage 13 fills it. The load-bearing
first-batch dependency is stage 11; the Cast arm becomes
fully functional once stage 13 lands.

`render_rvalue` could land at stage 11 or stage 13. Stage
11 is chosen because:

1. `render_place` and `render_operand` both land at stage
   11 (D-5-K, D-6-K). Co-locating `render_rvalue` keeps the
   rendering surface coherent.
2. The Cast arm's `render_ty` dependency can late-bind
   (stage 11's `render_rvalue` calls into a stage-13-filled
   `render_ty` once stage 13 lands; the call is statically
   valid as soon as both are in place; tests for the Cast
   arm gate on stage 13).
3. Snapshot tests covering `Rvalue`-bearing body content
   (every `Statement::Assign` and most `Terminator`
   payloads) become available at stage 11 (when bodies
   materialise), blocking on `render_rvalue` rather than
   waiting for stage 13.

This mirrors `render_place`'s and `render_operand`'s
dependency analyses (D-5-K, D-6-K): land at the latest of
the *first batch* of dependencies that unblock testing,
with later-binding arms calling later-landing dependencies.

**Methodology rules invoked.** "Renderer style is decided
deliberately" (stage-7-specific principle from
`memory.md` §2; deferral target chosen by dependency-chain
analysis, not by default).

**Prior-art evidence.** D-5-K and D-6-K precedents:
`render_place` and `render_operand` deferred to stage 11
with the same dependency-on-Body-materialisation argument.
`prior-art/charon.md` -- Charon's pretty-printer renders
rvalues with keyword-prefix style for Ref/RawPtr (`& <p>`,
`&raw const <p>`) and operator names for BinaryOp/UnaryOp;
one valid prior-art point for stage 11's eventual
`render_rvalue`.

**Worked example.** Rendering `Rvalue::BinaryOp(BinOp::Add,
Operand::Copy(_1), Operand::Constant(<5>))` requires three
pieces:

- BinOp name: `+` (literal mapping; self-contained).
- `render_operand(Operand::Copy(_1), body)`: returns
  `copy _1` (when the place has no projections; stage 11
  via `render_place`).
- `render_operand(Operand::Constant(<5>), body)`: returns
  `const 5_i32` (when the const resolves to `Const::Int(I32,
  5)`; stage 11's `render_operand` calls into stage-14
  `render_const`).

Full output: `copy _1 + const 5_i32`. At stage 11 where
`render_rvalue` lands, the Use / Repeat / Ref / RawPtr /
BinaryOp / UnaryOp / Discriminant / Aggregate arms work
end-to-end via `render_place` / `render_operand` plus the
§12 type-interner lookups for AggregateKind field/variant
names (each variant carries the destination TyId in its
payload). The Cast arm late-binds to stage-13 `render_ty`
for full destination-type rendering and becomes fully
functional once stage 13 lands; tests for the Cast arm
gate on stage 13.

**Cross-refs.** `spec-snapshot.md` §21.8 (rendering
deferral), §18.2 (per-method ownership table -- updated
with `render_rvalue` row). Outbound:
`stage-11/expectations.md` "From stage 7" (filed at stage-7
close per the merger contract).
