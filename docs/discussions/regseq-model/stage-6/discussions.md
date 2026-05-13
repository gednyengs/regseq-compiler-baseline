# Stage 6 -- Discussion Record

Distilled record of the design dialogue producing the
stage-6 `Operand` vocabulary spec. Each entry captures the
question raised, key options considered, the decision (or
current lean for open decisions), the reasoning chain, and
cross-references to `spec-snapshot.md` (the spec) and
`review-log.md` (the audit trail; created on first review
round).

Status of decisions in this file mirrors `spec-snapshot.md`'s
"Operand vocabulary" section (§20) plus adjacent additions to
§18.2, plus `status.md` "Open decisions" / "Locked decisions"
tables.

D-tag convention: prefixed `D-6-K` form per `../README.md`
§5.2 (stages 4+ use the new convention from day one; stages
1 and 3 use legacy un-prefixed tags).

Status tags:
- `[DRAFT]` -- audit-shape recorded; pending Codex review.
- `[LOCKED]` -- closed by an approved lockability decision
  (APPROVE, or MINOR-NIT-only stable per the stage-5
  diminishing-returns precedent).
- `[REOPENED]` -- a later sub-decision or sibling D-tag
  forced a revision; superseded entry kept for history.
- `[DEFERRED to stage N]` -- moved to a later stage's scope.

---

## 1. Decision discussions

### D-6-1 -- `Operand` core vocabulary [LOCKED]

**Question.** What is the closed `Operand` value shape that
the bridge fills from rustc's `mir::Operand<'tcx>` and that
downstream consumers (stages 7-9, codegen) read as the
RHS-atom surface of MIR?

**Key options.**

1. **Option A -- mirror rustc's 4-variant shape verbatim.**
   Adopt rustc's `Copy(Place) / Move(Place) / Constant(Box<ConstOperand>) / RuntimeChecks(RuntimeChecks)`
   structurally; preserve the `ConstOperand` wrapper and
   the `RuntimeChecks` 4th variant. Simplest mirror.
2. **Option B -- 3 variants + reject `RuntimeChecks` on
   encounter.** Adopt `Copy(Place) / Move(Place) / Constant(ConstId)`;
   reject `mir::Operand::RuntimeChecks` per the closed-
   vocabulary methodology rule.
3. **Option C -- 3 variants + lower `RuntimeChecks` at
   bridge time.** Adopt `Copy(Place) / Move(Place) / Constant(ConstId)`;
   the bridge intercepts `mir::Operand::RuntimeChecks(checks)`,
   resolves via `checks.value(tcx.sess)`, interns as
   `Const::Bool(b)`, and emits as `Operand::Constant(ConstId)`.
4. **Option D -- collapse `Copy` and `Move` into a single
   `Use(Place)` variant.** Mirror mrustc's pre-`TODO` shape;
   recover the move-vs-copy semantics via type-driven
   validity tracking in a downstream pass.

**Decision.** Option C (the parent decision; per-aspect
sub-decisions pinned in D-6-1.1 through D-6-1.4):

```rust
pub enum Operand {
    Copy(Place),
    Move(Place),
    Constant(ConstId),
}
```

Plus the bridge-side `RuntimeChecks` lowering rule (§20.3.1):
resolve session-flag query at bridge time, intern as
`Const::Bool`, emit as `Operand::Constant(ConstId)`.

**Reasoning.** Three forces converge on Option C:

1. **Closed-vocabulary discipline.** The methodology rule
   ("closed `PlaceElem` vocabulary" / "no defensive
   fallbacks" -- analogous for stage 6) argues for the
   smallest variant set that captures the IR's content.
   `RuntimeChecks` is per-consumer session-derived; per Q4
   the bridge runs inside one rustc invocation per build
   with one consuming `Session` in scope. The value is
   therefore build-pinned at the bridge boundary and
   indistinguishable from a literal bool at the IR level
   for that build.
2. **Source-acceptance.** Reject-on-encounter (Option B) is
   strictest but rejects legitimate driver source that
   uses `cfg!(ub_checks)` etc. -- a needless friction.
   Lower-and-eliminate (Option C) accepts the source while
   keeping the IR vocabulary closed.
3. **Peer evidence.** mrustc has no `RuntimeChecks` analog
   and resolves session-flag-derived values upstream;
   Charon explicitly lowers `RuntimeChecks` to
   `Rvalue::NullaryOp` + `Move(local)` rather than
   rejecting (`prior-art/charon.md` D-6-1 mapping). Both
   peers' choices match Option C's spirit.

For Copy/Move preservation (Option D rejected): see D-6-1.4.

For `ConstId` payload (vs `Box<ConstOperand>`): see D-6-1.3.

**Methodology rules invoked.**

- "Closed vocabulary; unknown rejected" (`../README.md` §2)
  -- the closed-vocabulary discipline that bounds the
  variant set.
- "No defensive fallbacks" -- there is no `RuntimeChecks`
  fallback variant; the bridge handles the rustc form
  deterministically via the lowering rule.
- "Compositional behavior beats special-case flags" --
  `RuntimeChecks` semantics fall out of `Const::Bool` +
  the existing `Operand::Constant` arm; no parallel flag.
- "C semantics with information preservation" -- store
  rustc's resolved form (Copy/Move tag, interned const),
  render in C shape on demand. Drop redundant metadata
  (`user_ty`, use-site `span`).
- "Re-derive from evidence; do not import baggage" --
  rustc's 4-variant shape is mirror-able but the
  `RuntimeChecks` rationale (multi-crate-graph) does not
  apply here; re-derive yields 3 variants + lowering rule.

**Prior-art evidence.**

- `prior-art/rustc.md` §1 (rustc 4-variant Operand), §4
  (RuntimeChecks per-crate session resolution), §5
  (codegen_operand match), §10 (D-6-1 mapping).
- `prior-art/mrustc.md` -- explicit `TODO: Split "Use" into
  "Copy" and "Move"` at `src/mir/mir.hpp:645` (peer regret
  of Option-D-style collapse); no `RuntimeChecks`
  equivalent.
- `prior-art/charon.md` -- 3-variant LLBC `Operand`;
  preserves Copy/Move; lowers `RuntimeChecks` to
  `Rvalue::NullaryOp` + `Move(local)` (peer precedent for
  Option C).

**Worked example.** A function whose body contains
`if cfg!(overflow_checks) { panic!("...") } else { ... }`
expands at MIR construction to a use of
`mir::Operand::RuntimeChecks(OverflowChecks)` as the
discriminant for a SwitchInt terminator. Bridge translation
walks the operand:

```text
mir::Operand::RuntimeChecks(OverflowChecks)
  -> resolve via sess.overflow_checks() -> b: bool
  -> Const::Bool(b)
  -> ConstInterner::intern(Const::Bool(b)) -> id: ConstId
  -> Operand::Constant(id)
```

Downstream stages see `Operand::Constant(id)` referencing
`Const::Bool(true)` (or `false`) -- indistinguishable from
the source having written the literal directly. The
SwitchInt terminator (stage 9) can statically resolve the
branch in any consumer that const-folds.

**Cross-refs.**

- `spec-snapshot.md` §20 (Operand vocabulary), §20.3
  (bridge translation), §20.3.1 (RuntimeChecks lowering
  rule).
- Sub-decisions D-6-1.1 through D-6-1.4.

---

### D-6-1.1 -- `Operand` variant cardinality [LOCKED]

**Question.** Should the RegseqModel `Operand` enum carry
3 variants (`Copy / Move / Constant`) or rustc's 4 (adding
`RuntimeChecks`)?

**Key options.** Subsumed by parent D-6-1's options
(Option A = 4 variants verbatim, Option B = 3 + reject,
Option C = 3 + lower at bridge time). This sub-decision
pins the variant *cardinality*; D-6-1.2 pins the bridge
treatment of the dropped variant.

**Decision.** 3 variants. The rustc `RuntimeChecks` variant
is dropped from the IR; its content is lowered to
`Operand::Constant(ConstId)` at bridge time per D-6-1.2.

**Reasoning.** Per Q4 (`rustc_driver::Callbacks`
integration), the bridge runs inside one rustc invocation
per build with one consuming `Session` in scope. The rustc
multi-consumer rationale for keeping `RuntimeChecks` as a
non-`Const` variant ("value can differ across crates within
a single crate graph", `rustc_middle/src/mir/syntax.rs:1338-1340`)
maps onto our pipeline as "different builds may produce
different lowered consts" -- but within one build the
consuming session is fixed. There is no IR consumer for
whom the "this came from a session flag" provenance carries
non-constant information -- the bool is the same bool at
IR consumer time. Q6 (multi-crate strategy) does not break
this: even when MIR originates from regseq-aware dependency
crates compiled separately, the bridge consumes via
`tcx.optimized_mir(def_id)` against the bridge-host's
session.

**Methodology rules invoked.** "Closed vocabulary" (smallest
variant set covering content); "Re-derive from evidence"
(rustc's rationale doesn't transfer to our single-consumer
pipeline).

**Prior-art evidence.** `prior-art/charon.md` -- LLBC has
3-variant `Operand` (no `RuntimeChecks` at operand level).
`prior-art/mrustc.md` -- no `RuntimeChecks` analog.

**Worked example.** Source-level `cfg!(overflow_checks)`
expands at MIR construction to a `RuntimeChecks(OverflowChecks)`
operand. With Option A (4 variants), the IR carries this as
a 4th `Operand` arm and every consumer (rvalue evaluation,
backend codegen, validation walks) must handle it explicitly.
With this decision (3 variants + bridge-side lowering), the
IR carries it as `Operand::Constant(ConstId)` referencing a
`Const::Bool(b)` -- consumers see a uniform shape across
all literal-bool sites regardless of source provenance.

**Cross-refs.** `spec-snapshot.md` §20.1; D-6-1 parent;
D-6-1.2 (lowering rule).

---

### D-6-1.2 -- `RuntimeChecks` bridge-side lowering rule [LOCKED]

**Question.** Given D-6-1.1 (drop `RuntimeChecks` variant),
how does the bridge handle a source-level
`mir::Operand::RuntimeChecks(checks)` -- reject, silently
skip, or lower?

**Key options.**

1. **Option (a) -- preserve as 4th variant.** Subsumed by
   D-6-1.1's rejection.
2. **Option (b) -- reject on encounter.** Closed-vocabulary
   strict path; bridge raises `unsupported_construct!`
   diagnostic.
3. **Option (c) -- lower at bridge time.** Resolve via
   `checks.value(tcx.sess)`, intern as `Const::Bool`, emit
   as `Operand::Constant(ConstId)`.

**Decision.** Option (c). The bridge:

1. Resolves the boolean: `b = checks.value(tcx.sess)` per
   the `rustc_middle::mir::syntax.rs:1585-1592` resolution
   table.
2. Constructs `Const::Bool(b)`.
3. Interns via `ConstInterner::intern` (§9.7) yielding
   `ConstId`.
4. Emits `Operand::Constant(ConstId)`.

**Reasoning.** Option (b) would reject legitimate driver
source (`cfg!(ub_checks)`, `core::intrinsics::contract_checks()`,
etc.) for a reason invisible to the user (the IR's
closed-vocabulary discipline). Option (c) accepts the
source while preserving the IR-level discipline; the only
cost is a small bridge-side rewrite.

Resolution well-definedness: per Q4
(`rustc_driver::Callbacks`), the bridge runs inside one
rustc invocation per build with one consuming `Session`.
`checks.value(tcx.sess)` resolves against that single
session. Q6 (multi-crate) does not introduce ambiguity at
the bridge boundary: cross-crate MIR consumed via
`tcx.optimized_mir(def_id)` is resolved against the
bridge-host's session, mirroring rustc's standard codegen
behavior (the consumer's session determines `RuntimeChecks`
values, not the producer's).

Const-uniqueness via `ConstIdentityKey` (D-4-2 byte-form
schema) ensures the same boolean value at multiple sites
resolves to the same `ConstId` within a build, preserving
the F4 / F4-2 within-build invariants that downstream stages
rely on. The build-equivalence quad in §12.4 / §16
(equal source, equal pinned toolchain, equal
`RegseqModel.target`, equal bridge options) does not
currently list `ub_checks` / `contract_checks` /
`overflow_checks` among its equality dimensions; two
builds differing only on those flags fall under the quad's
equivalence but produce different lowered consts at
`RuntimeChecks`-derived operand sites. If cross-build
alignment of those operands is required, the appropriate
fix is to widen the quad in §12.4 (out of stage 6's scope).

**Methodology rules invoked.** "C semantics with
information preservation" (resolve at the level rustc has
already done the work); "Compositional behavior beats
special-case flags" (the variant's content composes from
existing `Const::Bool` + `Operand::Constant`).

**Prior-art evidence.** `prior-art/charon.md` -- Charon
lowers rustc `RuntimeChecks` operands to
`Rvalue::NullaryOp` + materialised bool local +
`Operand::Move(local)`. RegseqModel's lowering is *more*
aggressive (directly to `Const::Bool`) because we don't
need to preserve the origin for verification semantics --
the bool is just a constant once evaluated.

**Worked example.** See D-6-1 worked example (the
`cfg!(overflow_checks)` translation chain).

**Cross-refs.** `spec-snapshot.md` §20.3 (translation table)
and §20.3.1 (lowering rule statement).

---

### D-6-1.3 -- `Constant` payload shape [LOCKED]

**Question.** What does the `Constant` variant carry --
rustc's `Box<ConstOperand>` wrapper, an inline `Const`
struct, or a `ConstId` reference into the model's
`ConstInterner`?

**Key options.**

1. **Option A -- preserve `Box<ConstOperand>` verbatim.**
   `Constant(Box<ConstOperand>)` mirroring rustc; carries
   `span`, `user_ty`, `const_` inline.
2. **Option B -- inline `Const` struct.** Drop the box and
   wrapper but keep an inline `Const` (no
   `ConstInterner` reference); `Constant(Const)` directly.
3. **Option C -- `ConstId` reference.** `Constant(ConstId)`;
   resolve via the model's `ConstInterner` (§9.7); drop
   the wrapper entirely.

**Decision.** Option C. The wrapper is dropped entirely:

- `co.const_` flows through `translate_const` (the §9.7
  bridge construction dispatch for `mir::Const<'tcx>` ->
  RegseqModel `Const`) and is interned via
  `ConstInterner::intern` (§9.7).
- `co.user_ty` is dropped. NLL/borrowck-only metadata; dead
  in `optimized_mir`.
- `co.span` is dropped at the operand level. For
  Statement-level uses, the use-site span is naturally
  carried by the containing Statement's `SourceInfo` at
  stage 8. For Call / TailCall arguments, rustc preserves a
  finer-grained per-argument span via `Spanned<Operand>` at
  the Terminator level (`rustc_middle/src/mir/syntax.rs:811-820, 850-857`);
  that span is captured at the Terminator's argument-list
  slot in stage 9, not in the Operand itself.

**Reasoning.** Three drops, each justified independently:

- **`user_ty` drop.** rustc's `ConstOperand.user_ty` is
  documented as "Needed for NLL to impose user-given type
  constraints" (`rustc_middle/src/mir/syntax.rs:1352-1354`).
  NLL runs *before* MIR optimization passes; by
  `optimized_mir` the field's downstream consumer (NLL) has
  already fired. No bridge consumer reads it.
- **`span` drop at the operand level.** For Statement-level
  uses, the use-site source location is in the Statement's
  `SourceInfo` (`rustc_middle/src/mir/statement.rs:14-19`).
  For Call / TailCall arguments, where rustc *does*
  preserve a per-argument span outside the Terminator's
  own `source_info`, that span lives in the
  `Spanned<Operand>` wrapper at the Terminator level
  (`syntax.rs:811-820, 850-857`); stage 9 captures the
  wrapper, not the Operand. Either way the operand-internal
  `span` is redundant.
- **Wrapper drop.** With `user_ty` and `span` gone, the
  wrapper carries only `const_`. A wrapper around a single
  field is dead structure; the bare `ConstId` reference is
  cleaner.

**Methodology rules invoked.** "Re-derive from evidence; do
not import baggage" (rustc's wrapper is for upstream
consumers that have already run); "C semantics with
information preservation" (preserve what carries downstream
information; drop what is redundant or upstream-dead).

**Prior-art evidence.** `prior-art/rustc.md` §2 (wrapper
fields and their consumers); §8 (rustc_public preserves
the wrapper because it is a stable-IR republisher, not
post-`optimized_mir` pruning -- our pruning calculus is
different).

`prior-art/charon.md` -- Charon stores `Operand::Const(Box<ConstantExpr>)`
inline (no `ConstInterner` analog). RegseqModel's
`ConstId`-by-reference is a stage-4-architectural
consequence, not a stage-6-original choice.

**Worked example.** A `let x: u32 = 42;` binding produces
MIR `_1 = const 42_u32`. The rustc-side
`Operand::Constant(box ConstOperand { span: <site>, user_ty: None, const_: Const::Val(...) })`
is translated by the bridge as:

```text
mir::Operand::Constant(box co)
  -> co.const_
       -> translate_const(co.const_) -> Const::Uint(U32, 42)
       -> ConstInterner::intern(Const::Uint(U32, 42)) -> id: ConstId
  -> co.span: dropped (Statement.source_info at stage 8 carries the use-site span)
  -> co.user_ty: dropped (None here; would also be dropped for an
     ascription like `Vec::<u32>::new()` because user_ty is NLL-only)
  -> Operand::Constant(id)
```

A second `_2 = const 42_u32` later in the same body produces
the *same* `ConstId` via `ConstInterner`'s content-uniqueness
(D-4-2 byte-form schema), so the two operand sites share the
same payload identity automatically.

**Cross-refs.** `spec-snapshot.md` §20.1 (Constant variant
docstring), §20.3 (wrapper-strip translation rule).
Outbound: `stage-9/expectations.md` "From stage 6"
(stage 9 must preserve per-argument spans via the
`Spanned<Operand>` wrapper at Call / TailCall Terminator
sites; preservation shape is stage 9's choice).

---

### D-6-1.4 -- Copy/Move preservation [LOCKED]

**Question.** Should the RegseqModel `Operand` preserve
both `Copy(Place)` and `Move(Place)` as distinct variants,
or collapse to a single `Use(Place)` (mrustc-style)?

**Decision.** Preserve both variants distinct.

**Reasoning.** Three independent forces support
preservation:

1. **Backend optionality at zero spec cost.** A C backend
   that wants to specialise (e.g., emit `memcpy` for large
   moves where the source becomes dead, or elide a final
   move into a return slot) needs the variant tag. Even if
   the v1 backend doesn't use the distinction, preserving
   it costs nothing in spec complexity and keeps the
   option open.
2. **Move's distinguishing Call-site semantics.**
   `Operand::Move`'s `Call`-argument in-place passing
   (rustc `syntax.rs:1322-1330`) is semantically distinct
   from `Copy`. A backend that wants to model parameter
   passing precisely (e.g., for ABI verification) needs
   the distinction. Codegen-level operational equivalence
   (`codegen_operand` collapse in
   `rustc_codegen_ssa/src/mir/operand.rs:1046-1056`) does
   not extend to all consumers.
3. **Peer evidence.** mrustc's `Use(LValue)` collapse
   carries an explicit `TODO: Split "Use" into "Copy" and
   "Move"` (`src/mir/mir.hpp:645`) -- the closest existing
   peer regrets the collapse. Charon preserves the
   distinction in its verification pipeline. rustc itself
   keeps both variants. Three peers, three independent
   reasons to preserve.

**Methodology rules invoked.** "C semantics with
information preservation" (preserve information rustc gives
us where downstream consumers may want it); "No defensive
fallbacks" does not apply -- the distinction is rustc-given
content, not a defensive fallback.

**Prior-art evidence.** `prior-art/rustc.md` §1
(`Operand::Copy` / `Operand::Move` docstrings -- semantic
distinction); §5 (codegen_operand collapse with
`move_annotation` debug-info diff); §6 (annotation
conditions).

`prior-art/mrustc.md` -- `src/mir/mir.hpp:645` regretted
collapse; type-driven validity tracking compensates but at
visible cost.

`prior-art/charon.md` -- preserves Copy/Move through
verification IR; pretty-printer emits `copy <p>` / `move <p>`
keyword-prefix style.

**Worked example.** A function `fn consume(s: String) -> usize`
takes its argument by-move. The MIR call site emits
`mir::Operand::Move(_2)` for the argument. Translation:
`Operand::Move(Place::from(_2))`. A future backend wanting
to elide an intermediate copy on the way to the call frame
can consult the `Move` tag; a backend that doesn't, gets
the same C value-load as `Copy` and is unaffected.

**Cross-refs.** `spec-snapshot.md` §20.1 (variant docstrings),
§20.4 (C-semantics lowering -- bare load semantics
identical, variant tag informational).

---

### D-6-2 -- Operand storage and interning [LOCKED]

**Question.** Where does an `Operand` live in the model --
inline at use sites, or via an `OperandId` indirection
into a top-level interner?

**Key options.**

1. **Option A -- inline at use sites.** `Operand` is
   embedded by value into stages 7-9 payloads (`Rvalue`,
   `Statement`, `Terminator`). No `OperandId`, no
   interner.
2. **Option B -- `OperandId` interner.** Mirror stage 4's
   `Const` interning; introduce `OperandInterner` and
   reference operands by `OperandId`.

**Decision.** Option A. Operands live inline at use sites;
no `OperandId`; no operand-level interner.

**Reasoning.** Two concrete arguments against interning:

1. **No downstream consumer of operand-content-uniqueness.**
   `Const` interning exists because `Const`-content-identity
   matters for downstream consumers: stage-3
   `GenericArg::Const(ConstId)` participates in
   `Ty`-identity; stage-9 instance-key construction reads
   `ConstId`s; F4-2 cross-build identity threads through
   them. Operands have no analogous consumer -- no IR
   structure consumes
   "two operand uses are content-equivalent."
2. **Heterogeneous, small payloads.** Operand variants are
   small (~24 bytes worst case) and structurally
   heterogeneous; interning's overhead (a hash lookup at
   construction; ID dereference at use) buys nothing.

**Methodology rules invoked.** "Compiler-theory-first" --
interning is a deduplication optimisation; without a
deduplication consumer, it's premature abstraction. "Eager
translation" (Q2) does not require interning; it requires
fully-owned IR, which inline-by-value satisfies trivially.

**Prior-art evidence.** rustc's MIR has no operand
interner (`prior-art/rustc.md` §7). `rustc_public` follows
the same pattern (§8). mrustc inlines at use sites with
spaceship `ord(...)` for structural equality
(`prior-art/mrustc.md`). Charon has a generic
`HashConsed<T>` infrastructure but explicitly does not apply
it to `Operand` (`prior-art/charon.md` D-6-2 mapping).
Four-out-of-four peers store inline.

**Worked example.** A function with two consecutive
statements `_2 = _1` and `_3 = _1` produces MIR with two
`Rvalue::Use(Operand::Copy(Place::from(_1)))` operand
slots. Under Option B, both would resolve to the same
`OperandId` via interning -- a single deduplication step
saving ~16 bytes per repeat. Under Option A (this
decision), each `Rvalue::Use` carries its own
`Operand::Copy(Place { local: LocalId(1), projection: Box<[]> })`
inline. The "saved" 16 bytes per duplicate is dwarfed by
the cost of a parallel `OperandInterner` table, the
`OperandId` type and serde derives, and the indirection at
every use site -- all to deduplicate something no
downstream consumer asks about. Inline storage is
strictly cheaper.

**Cross-refs.** `spec-snapshot.md` §20.2 (identity and
storage).

---

### D-6-3 -- Operand-site invariants [LOCKED]

**Question.** What invariants does the bridge enforce at
operand construction? Specifically: does `Operand::Copy(p)`
require that `p`'s type is `Copy` (rustc's
pre-drop-elaboration invariant)?

**Key options.**

1. **Option A -- enforce rustc's pre-drop-elaboration
   type-`Copy` invariant on `Operand::Copy(p)`.** Mirror
   rustc's documented invariant; reject if `p`'s type is
   non-`Copy`.
2. **Option B -- enforce a weaker invariant.** Require
   only that the place be loadable (e.g., its type is
   `Sized` and validation rules pass), without specifically
   checking the `Copy` bound.
3. **Option C -- enforce nothing at operand construction.**
   Inherit Place validity from §19.7 transitively; do not
   add operand-site checks.

**Decision.** Option C. No type-`Copy` enforcement on
`Operand::Copy(p)`. One invariant only:

- **I-Operand-Place-Validity** (§20.5) -- discharged by
  inheritance: enforcing Place validity (§19.7) transitively
  covers all `Operand::Copy` / `Operand::Move` slots.

No type-coherence enforcement on `Operand::Constant(id)` at
operand site; consumers (stages 7-9) own that check.

**Reasoning.** The rustc-side type-`Copy` invariant on
`Operand::Copy` is documented as required *pre-drop-
elaboration only* and *explicitly gone post-drop-elaboration*
(`rustc_middle/src/mir/syntax.rs:1312-1313`):

> "Before drop elaboration, the type of the place must be
> `Copy`. After drop elaboration there is no such
> requirement."

RegseqModel reads `optimized_mir`, which is post-drop-
elaboration. The invariant has been discharged before the
IR reaches us; the bridge would be re-checking already-
checked content. Per "no defensive fallbacks," we do not
add the check.

Place validity (§19.7) is non-vacuous only once stages 7-9
materialise operand use sites; the walk piggybacks on the
§3.3 Phase B body-validation traversal that stage 11 owns
(D-5-2 deferral).

**Methodology rules invoked.** "No defensive fallbacks" --
do not re-check what rustc has discharged; "Compositional
behavior beats special-case flags" -- I-Operand-Place-Validity
is fully derived from §19.7 + §3.3 Phase B; no operand-side
parallel walk.

**Prior-art evidence.** `prior-art/rustc.md` §1 (the
pre-drop-elaboration / post-drop-elaboration distinction),
§10 D-6-3 mapping. `prior-art/mrustc.md` -- mrustc enforces
no operand-construction invariants; the move/copy semantic
is checked at use sites via per-local validity tracking.
`prior-art/charon.md` -- Charon enforces no type-`Copy` on
`Operand::Copy(p)`; `simplify_constants` micro-pass
establishes a separate post-pass invariant (orthogonal to
this question).

**Worked example.** A function `fn drop_string(s: String)`
contains an `Operand::Move(_1)` at its drop point. In
pre-drop-elaboration MIR, this would also appear as
`Operand::Move(_1)`; in *theory* an `Operand::Copy(_1)` at
that site would have been rejected because `String` is not
`Copy`. In `optimized_mir` (post-drop-elaboration), the
type-`Copy` constraint is gone -- rustc itself drops the
check. RegseqModel reads `optimized_mir`; if a hand-crafted
fixture were to write `Operand::Copy(_1)` at this site, the
bridge would not flag it. Place validation (§19.7) still
runs and catches structural problems (e.g., `_1` not in
`local_decls`); the type-`Copy` semantic check is rustc's
upstream concern.

**Cross-refs.** `spec-snapshot.md` §20.5 (operand-site
invariants), §19.7 (Place validation, inherited).

---

### D-6-K -- `pretty.rs::render_operand` deferral [LOCKED]

**Question.** Where does `pretty.rs::render_operand` land
-- stage 6 (alongside the Operand vocabulary), stage 11
(alongside `render_place`, latest first-batch dependency),
or stage 14 (alongside `render_const`, latest dependency
across all variants)?

**Key options.**

1. **Option A -- stage 6 (immediate).** Land
   `render_operand` alongside the variant set. Requires
   the `Constant` arm to call `render_const` (stage 14,
   not yet filled) and the `Copy`/`Move` arms to call
   `render_place` (stage 11, not yet filled).
2. **Option B -- stage 11 (latest first-batch
   dependency).** Co-locate with `render_place` (D-5-K).
   The `Constant` arm late-binds to `render_const` once
   stage 14 fills it.
3. **Option C -- stage 14 (latest dependency overall).**
   Wait for both `render_place` (stage 11) and
   `render_const` (stage 14) before landing
   `render_operand`. Maximally deferred.

**Decision.** Option B (stage 11). Filed as outbound
expectation in `stage-11/expectations.md` "From stage 6"
alongside the existing D-5-K `render_place` deferral.

**Reasoning.** Per-variant rendering input dependency:

| Variant         | Rendering input                                 | Stage of input |
| --------------- | ----------------------------------------------- | -------------- |
| `Copy(p)`       | `render_place(p, body)` + `copy ` keyword prefix | 11            |
| `Move(p)`       | `render_place(p, body)` + `move ` keyword prefix | 11            |
| `Constant(id)`  | `render_const(id)`                              | 14            |

The `Copy` / `Move` arms depend on `render_place` (stage
11) which itself depends on `Body` materialisation +
`LocalId` `_N` rendering. The `Constant` arm depends on
`render_const` (stage 14, per D-4-K).

`render_operand` could land at stage 11 or stage 14. Stage
11 is chosen because:

1. `render_place` lands at stage 11 (D-5-K). Co-locating
   `render_operand` with `render_place` keeps the rendering
   surface coherent.
2. The `Constant` arm can call `render_const` once stage 14
   fills it; the dependency chain is acyclic.
3. Snapshot tests covering `Operand`-bearing body content
   become available at stage 11 (when bodies materialise),
   blocking on `render_operand` rather than waiting for
   stage 14.

This mirrors `render_place`'s dependency analysis (D-5-K):
land at the latest of the *first batch* of dependencies
that unblock testing, with the late-binding arm calling
its later-landing dependency.

**Methodology rules invoked.** "Renderer style is decided
deliberately" (per stage-5 D-5-K precedent; deferral
target chosen by dependency-chain analysis, not by default).

**Prior-art evidence.** D-5-K precedent: `render_place`
deferred to stage 11 with the same dependency-on-Body-
materialisation argument. `prior-art/charon.md` --
Charon's pretty-printer uses keyword-prefix style
(`copy <p>` / `move <p>` / `const <c>`); one valid prior-art
point for stage 11's eventual `render_operand`.

**Worked example.** Rendering `Operand::Move(_3)` requires
two pieces: the `move ` keyword prefix (self-contained
text) and `render_place(_3, body)`. The latter returns
`_3` (the `LocalId` `_N` sigil rendering, stage 11) when
the place has no projections, or `*_3.field` etc. when it
does (still stage 11, since field-name resolution reads
from `LocalDecl.ty`'s layout). The full output is `move _3`.
Now consider `Operand::Constant(id)` where `id` resolves
to `Const::Int(I32, -1)` -- rendering produces `const -1_i32`,
which requires `render_const(id)` (stage 14). At stage 11
where `render_operand` lands, the `Copy` and `Move` arms
work end-to-end; the `Constant` arm calls into a stage-14-
filled `render_const` (which lands later in the dependency
chain) -- the call is statically valid as soon as both
landed; tests for the `Constant` arm gate on stage 14.

**Cross-refs.** `spec-snapshot.md` §20.6 (rendering
deferral), §18.2 (per-method ownership table -- updated
with `render_operand` row). Outbound: `stage-11/expectations.md`
"From stage 6" (filed at §3 Work step 4).
