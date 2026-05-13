# Stage 6 -- Prior Art: mrustc primary source

Primary-source mrustc evidence for the operand-equivalent
abstraction in its MIR, scoped to what stage 6's decision
cluster (D-6-1 variant set, D-6-2 storage / interning,
D-6-3 invariants, D-6-K render deferral) needs. mrustc is
the closest existing peer: a Rust-to-C compiler with its own
HIR/MIR. Fresh primary-source pass per `../../README.md` §2
methodology rule (do not reuse stage-0 summaries; web-fetched
verbatim source is the evidence).

Repository: https://github.com/thepowersgang/mrustc
Primary files inspected:

- `src/mir/mir.hpp` -- IR type definitions (LValue, Param,
  RValue, Statement, Terminator, Constant)
- `src/mir/check.cpp` -- validation passes including the move/
  copy state tracker
- `src/trans/codegen_c.cpp` -- C backend (consumer pipeline;
  partial inspection)

Every factual claim cites a `file:line` in the mrustc tree.
Hypotheses are flagged. Inferences across two sites are
labelled INFERENCE.

---

## 1. Two operand-like types: `Param` and `RValue::Use`

mrustc factors operand-ness across two tagged unions, not
one. This is a structural difference from rustc's single
`Operand<'tcx>`.

**`RValue::Use(LValue)`** appears at the rvalue level for
direct value-of-a-place reads on the right-hand side of an
`Assign` statement [`src/mir/mir.hpp:644-650`]:

```cpp
TAGGED_UNION_EX(RValue, (), Tuple, (
    // TODO: Split "Use" into "Copy" and "Move" (Where 'move' indicates that the source is unused)
    (Use, LValue),
    (Borrow, struct {
        ::HIR::BorrowType   type;
        bool is_raw;
        LValue  val;
        }),
    (Constant, Constant),
    ...
```

**`Param`** is a separate operand-equivalent used inside
RValue sub-payloads (BinOp operands, Tuple/Array/Struct/Enum
constructors, Call args, SizedArray val) and Terminator
payloads (Call args)
[`src/mir/mir.hpp:528-545`, schematic line range based on
verbatim fetch]:

```cpp
/// Parameter - A value used when a rvalue just reads (doesn't require a lvalue)
/// Can be either a lvalue (memory address), or a constant
TAGGED_UNION_EX(Param, (), Constant, (
    (LValue, LValue),
    // TODO: Add `Borrow` here (makes some MIR manipulation more complex, but simplifies emitted code)
    (Borrow, struct {
        ::HIR::BorrowType   type;
        LValue  val;
        }),
    (Constant, Constant)
    ), (), (), (
        Param clone() const;
        ...
    )
);
```

Observations:

- **`Param` has 3 variants**: `LValue`, `Borrow`, `Constant`.
  The `Borrow` variant pre-bakes a borrow operation into the
  operand (rustc by contrast keeps `Operand` and `Rvalue::Ref`
  separate; `Operand` itself never carries borrow semantics).
- **The `RValue::Use(LValue)` arm is what bridges the
  assignment-RHS-of-a-place-load case** that rustc would
  encode as `Rvalue::Use(Operand::Copy(p))` or
  `Rvalue::Use(Operand::Move(p))`. Because `Param` is reused
  for everything else (BinOp/Call args/etc.), mrustc avoids
  having two parallel ways to talk about "load this place"
  -- but in doing so it loses the Copy/Move bit at every
  such use.

INFERENCE: mrustc's two-tier `RValue::Use(LValue)` plus
`Param::LValue` design treats "load this place" identically
in both contexts. The operand-vs-rvalue split is structural
(can a borrow appear here?) rather than semantic
(copy-or-move?). RegseqModel's stage 5/6 split puts `Place`
inline on `Operand::Copy` / `Operand::Move`, so the parallel
exists -- but RegseqModel preserves Copy/Move at the operand
level, where mrustc folds it away.

---

## 2. The TODO that confirms Copy/Move information loss

At [`src/mir/mir.hpp:645`], the `RValue::Use` variant carries
an explicit project-level TODO:

```cpp
// TODO: Split "Use" into "Copy" and "Move" (Where 'move' indicates that the source is unused)
(Use, LValue),
```

This is load-bearing evidence for stage 6's D-6-1 decision:

- **mrustc does not preserve Copy/Move in its IR.** The
  variant set has no `Copy` / `Move` discriminator; both
  collapse into `Use(LValue)`.
- **The mrustc maintainers acknowledge this is a deficiency**
  -- the comment is not "Use is sufficient" but "TODO: split
  into Copy and Move."
- **The reason cited is "Where 'move' indicates that the
  source is unused"** -- i.e., the desired distinction is
  precisely the move-out semantic that rustc's Move encodes.

For RegseqModel: mrustc is a peer that lost this information
and now wishes it hadn't. RegseqModel preserving Copy/Move
from day one (D-6-1 keeps both variants) avoids the same
debt.

---

## 3. Move-vs-Copy semantics tracked at validation time, not in the IR

mrustc compensates for the IR-level loss by deriving
move-vs-copy from type information during a validation pass
[`src/mir/check.cpp` (lines schematic per fetch summary,
~650-655)]:

```cpp
void move_val(const ::MIR::TypeResolve& state, const ::MIR::LValue& lv)
{
    ensure_valid(state, lv);
    if( ! state.lvalue_is_copy(lv) )
    {
        mark_validity(state, lv, false);
    }
}

void move_val(const ::MIR::TypeResolve& state, const ::MIR::Param& p)
{
    if( const auto* e = p.opt_LValue() )
    {
        move_val(state, *e);
    }
}
```

Key facts:

- **Move-vs-Copy is computed from the `LValue`'s resolved
  type** via `state.lvalue_is_copy(lv)`. The IR variant
  itself does not carry the bit.
- **Non-Copy types invalidate the place after `move_val`**
  via `mark_validity(state, lv, false)`. Subsequent
  `ensure_valid` calls on the same place will fail.
- **Param-typed move-out** unwraps `opt_LValue()` and
  recurses; constants and borrows are not subject to move-
  invalidation.
- **The validation pass is `MIR_Validate_ValState`**
  [`src/mir/check.cpp` line ~500]; it tracks per-local
  validity state across the CFG.

INFERENCE: this is a working design but pays a recurring
type-resolution cost at every operand consumption site, and
makes IR-level analyses harder to write (any pass wanting to
know "is this operand a move?" must consult type tables in
addition to the IR). RegseqModel's choice to preserve Copy/
Move on the variant means downstream passes can pattern-
match the variant directly with no type-resolution lookup.

---

## 4. Constant variant set

mrustc's `Constant` tagged union has 10 variants
[`src/mir/mir.hpp:480-525`]:

```cpp
TAGGED_UNION_EX(Constant, (), Int, (
    (Int, struct { S128 v; ::HIR::CoreType t; }),
    (Uint, struct { U128 v; ::HIR::CoreType t; }),
    (Float, struct { double v; ::HIR::CoreType t; }),
    (Bool, struct { bool v; }),
    (Bytes, ::std::vector< ::std::uint8_t>),
    (StaticString, ::std::string),
    // NOTE: These are behind pointers to save inline space (HIR::Path is ~11
    // words, compared to 4 for MIR::Constant without it)
    (Const, struct { ::std::unique_ptr<::HIR::Path> p; }),
    (Generic, ::HIR::GenericRef),
    // ZST function type, NOT its address
    (Function, struct { ::std::unique_ptr<::HIR::Path> p; }),
    (ItemAddr, ::std::unique_ptr<::HIR::Path>)
    ), ...
);
```

Notes:

- **Inline value storage for scalars** (`Int`, `Uint`,
  `Float`, `Bool`); per-variant primitive-type tag (`HIR::CoreType`).
- **Inline byte-vec storage** for `Bytes` and `StaticString`
  (no interner indirection -- each constant carries its own
  payload).
- **Pointer-indirected paths** for `Const` / `Function` /
  `ItemAddr` variants whose payload is a `HIR::Path` (~11
  words). The comment makes the size trade-off explicit:
  `unique_ptr<HIR::Path>` keeps `Constant` to ~4 words.
- **`Function` is "ZST function type, NOT its address"** --
  notably distinct from `ItemAddr`, which is the address.
- **`Generic` carries a `HIR::GenericRef`** -- a generic
  parameter reference resolved against the surrounding
  generic context. This is mrustc's analog for unmonomorphized
  const generics.

INFERENCE on Constant payload metadata: mrustc's `Constant`
does NOT carry a use-site `Span` or a `user_ty` ascription
field. Compare rustc's `ConstOperand` which wraps `Const`
with both. mrustc's plainer shape is closer to RegseqModel's
target (`Operand::Constant(ConstId)` with no wrapper).

INFERENCE on indirection vs interning: mrustc indirects
HIR::Path payloads via `unique_ptr` to keep the variant
compact, but does NOT intern the underlying `Constant`
values; equality is defined structurally via the spaceship
`ord(...)` method [`src/mir/mir.hpp:510-525`]. RegseqModel's
ConstId-based interning (stage 4) is a stronger discipline
than mrustc's, but the underlying motivation -- keeping the
operand variant small -- is shared and resolved differently.

---

## 5. No RuntimeChecks equivalent

mrustc has no analog of rustc's `Operand::RuntimeChecks`
variant. The investigation surfaced no `cfg!`-style or
session-flag-derived value variants in the IR.

Evidence:

- **The `Param` and `Constant` tagged unions cover the entire
  operand-shaped surface**; neither has a session-query or
  cfg-derived variant.
- **Overflow handling is via dedicated BinOp variants**:
  `eBinOp::ADD_OV`, `SUB_OV`, `MUL_OV`, `DIV_OV`
  [`src/mir/mir.hpp:424-440`, schematic]. The variant choice
  encodes "do checked arithmetic" rather than gating it on a
  session flag. `Terminator::Panic` handles overflow panic
  emission [`src/mir/mir.hpp:780`].
- **No counterpart to `cfg!(ub_checks)` / `cfg!(contract_checks)` /
  `cfg!(overflow_checks)`** is preserved as a runtime-evaluated
  operand. INFERENCE: mrustc resolves these at HIR-to-MIR
  lowering or earlier, baking the result into the IR rather
  than carrying a deferred query.

For RegseqModel: this aligns with the proposed D-6-1 choice
to drop `RuntimeChecks` from the operand variant set. mrustc
demonstrates the precedent: a peer Rust-to-C compiler does
not need the variant in its operand IR; checks are
either resolved upstream or expressed via dedicated BinOp
variants when they need to survive.

---

## 6. Storage: no operand interner

mrustc operands are stored inline at use sites. There is no
top-level `ParamId` / `OperandId` interner table.

Evidence:

- **`RValue` payload struct fields hold operands by value**
  [`src/mir/mir.hpp:644-704`]. Examples:
  - `(Use, LValue)` -- `LValue` inline.
  - `(BinOp, struct { Param val_l; eBinOp op; Param val_r; })`
    -- two `Param` values inline.
  - `(MakeDst, struct { Param ptr_val; Param meta_val; })` --
    two `Param` values inline.
- **`Statement::Assign` payload** [`src/mir/mir.hpp:732-735`]:
  ```cpp
  (Assign, struct {
      LValue dst;
      RValue src;
  }),
  ```
  `LValue` and `RValue` both stored by value.
- **`Terminator::Call` arguments** [`src/mir/mir.hpp:810-816`]:
  ```cpp
  (Call, struct {
      BasicBlockId ret_block;
      BasicBlockId panic_block;
      LValue ret_val;
      CallTarget fcn;
      ::std::vector<Param> args;
  })
  ```
  Arguments are a `std::vector<Param>` holding values inline.

For RegseqModel: this aligns with D-6-2 (no operand
interning). mrustc, despite being the closest peer, does not
intern operands; operand-content-uniqueness has no downstream
consumer in their pipeline either.

---

## 7. LValue / Place anchor (briefly)

For comparison anchor, mrustc's `LValue` is a Storage +
chained Wrapper structure [`src/mir/mir.hpp:47-300`]:

- **Storage roots**: `TAG_Return` (return slot),
  `TAG_Argument(idx)` (param), `TAG_Local(idx)` (local),
  `TAG_Static(HIR::Path)` (static item).
- **Wrapper chain (left-to-right)**: `TAG_Deref`,
  `TAG_Field(idx)`, `TAG_Downcast(idx)`, `TAG_Index(local_idx)`.

Note `TAG_Index(local_idx)` -- like rustc's `ProjectionElem::Index`
and like RegseqModel's stage-5 `PlaceElem::Index(LocalId)` --
the index value is held in a separate local, not embedded as
an operand. INFERENCE: this is the same acyclicity discipline
RegseqModel pinned in stage 5 (Place has no operand-level
back-edge); a peer compiler arrived at the same shape.

`Static` as a Storage root is a difference from rustc and
from RegseqModel stage 5: rustc/RegseqModel separate static
references into an Operand variant family or into a separate
constant-shaped path (RegseqModel uses
`SyntheticItemKind::ConstMemoryStatic` per stage 4 D-4-3).
This is not stage-6 territory and is not pursued further
here.

---

## 8. Validation pass placement

mrustc's operand-related validation lives in
[`src/mir/check.cpp`]:

- **`MIR_Validate_ValState`** -- the per-local validity-state
  tracker (~line 500). Walks the CFG, threads a per-local
  validity bit, and flags any consumption of an invalidated
  place via `MIR_BUG`.
- **`ensure_valid(state, lv)`** -- guards LValue use; the
  emitted error is "Use of non-valid lvalue - <lv>"
  [`src/mir/check.cpp:619-645`, schematic]. Takes the form of
  an `MIR_ASSERT` followed by a state-table lookup.
- **`move_val(state, lv)`** -- the move-side hook (Section 3
  above): invalidates non-Copy places after consumption.

There is no dedicated operand-construction-time invariant
check. INFERENCE: invariants on operand structure are
implicit in the type system (you cannot construct a `Param`
that doesn't fit one of its three arms because it's a
tagged union); operand semantics (move-out, double-use) are
checked at the consumption site via the validity-state
machinery, not at the operand-construction site.

For RegseqModel: stage 5 validation is split (rules at
stage 5; walk implementation at stage 11). mrustc places the
operand-side checks alongside the place-side checks in a
single CFG-walking pass. INFERENCE: both designs are viable;
RegseqModel has the stronger design because it allows the
spec to lock at stage-6 close while implementation lands
end-of-phase-1 (per the spec-before-implementation rule).

---

## 9. Operand textual rendering (briefly)

mrustc's tagged-union machinery generates `operator<<` for
all `Param` / `Constant` / `LValue` / `RValue` variants
(visible in the `friend ::std::ostream& operator<<(...)`
declarations within each `TAGGED_UNION_EX` definition,
`src/mir/mir.hpp` throughout). The actual implementation
lives in the corresponding `.cpp` file; not pursued in this
pass.

For RegseqModel D-6-K (defer `pretty.rs::render_operand` to
stage 11): mrustc's approach is "every IR node has its own
`operator<<` baked in"; RegseqModel's approach centralizes
rendering in `pretty.rs`. Both are viable; RegseqModel's
choice reflects the §1 D14 deferral pattern (renderer waits
on full content references that only become available later).
mrustc's per-type approach would require the field/variant
name tables stage 5 D-5-K cited as the dependency to be
inline-accessible at every operand-emit site, not just at
render time. INFERENCE: RegseqModel's centralized renderer
is a tighter fit for its stage-staged spec discipline; the
deferral target (stage 11) carries no extra cost.

---

## 10. Cross-D-tag findings

Mapping of evidence to D-6-K decisions:

### D-6-1 (variant set + payload shapes)

- **mrustc collapses Copy/Move into a single `RValue::Use(LValue)` /
  `Param::LValue(LValue)` variant** (Sections 1, 2). The IR
  carries no Copy/Move discriminator; the move-vs-copy
  semantic is recovered at validation time via type-driven
  per-local validity tracking (Section 3).
- **mrustc maintainers explicitly TODO the split**
  [`src/mir/mir.hpp:645`]: "Split 'Use' into 'Copy' and
  'Move'." This is direct evidence that the variant collapse
  was a regret, not a design preference. RegseqModel's
  proposed D-6-1 (preserve both variants) avoids paying the
  same debt.
- **mrustc has no `RuntimeChecks` analog** (Section 5);
  session-flag-derived values are resolved upstream or
  expressed via dedicated BinOp arithmetic variants. This
  aligns with D-6-1's choice to drop `RuntimeChecks` from
  the RegseqModel operand surface.
- **mrustc's `Constant` is inlined inside the operand-equivalent
  `Param` variant by value, not via an interner ID** (Section 4).
  The size-trade-off is handled per-variant via
  `unique_ptr<HIR::Path>` rather than an interner. RegseqModel's
  choice (`Constant(ConstId)`) is a stronger discipline than
  mrustc's, but the underlying motivation -- keep the
  operand variant compact -- is shared.

Net: mrustc evidence supports D-6-1's shape (preserve Copy/Move,
drop RuntimeChecks, ConstId reference). The Copy/Move TODO is
an especially load-bearing data point: a peer compiler that
made the opposite call now wishes it hadn't.

### D-6-2 (storage / interning)

- **mrustc operands are stored inline at use sites in
  RValue / Statement / Terminator payloads** (Section 6).
- **No operand interner table exists**; equality is
  structural / value-based [`src/mir/mir.hpp:520-525`].

Net: confirms D-6-2's no-interning choice has peer precedent
in a compiler with the same Rust-to-C target shape.

### D-6-3 (operand-site invariants)

- **mrustc enforces no IR-level operand-construction
  invariants** beyond the tagged-union variant well-formedness
  the type system gives for free (Section 8).
- **Move-vs-Copy semantic invariants are enforced at use
  sites by `MIR_Validate_ValState`** (Section 3, 8) via
  per-local validity-state tracking, not at operand
  construction.
- **No type-`Copy` enforcement at operand construction**;
  the validity tracker handles the post-hoc invalidation
  derived from the place's resolved type.

Net: mrustc evidence is consistent with D-6-3's choice to
not enforce a type-`Copy` invariant on `Operand::Copy(p)` at
operand construction. The invariant rustc once carried
(pre-drop-elaboration) is already discharged by the MIR
phase RegseqModel reads.

### D-6-K (rendering deferral)

- **mrustc's per-type `operator<<` rendering** is a different
  discipline than RegseqModel's centralized `pretty.rs`
  (Section 9). No specific evidence either supports or
  contradicts the stage-11 deferral choice; it's a stylistic
  difference.

Net: D-6-K decision (defer `render_operand` to stage 11
alongside `render_place`) is unaffected by mrustc evidence.

---

## 11. What's NOT in this pass

- **The full `emit_param` / `emit_lvalue` implementations in
  `codegen_c.cpp`** -- the file is too large for a single
  WebFetch pass to cover both type definitions and codegen
  bodies. The high-level dispatch and case-split structure
  was inferred from check.cpp's complementary view (which
  shows how validation discriminates the variants); the
  emitted-C distinction between Param variants would
  reasonably mirror that. Evidence is partial here; flagged
  for any later pass that needs operand-level codegen
  details.
- **mrustc's `RValue::Use` -> codegen_c emit chain.**
  Specifically whether the C output for an LValue read
  differs by anything once the move/copy distinction is gone.
  Probably not, by construction (no IR variant to switch on),
  but unverified.
- **Drop semantics interaction.** Whether `Drop` glue is
  emitted differently for Move-tagged vs Copy-tagged sites
  in mrustc -- not pursued because mrustc has no such tag.
  Drop emission lives in a separate place tied to the
  per-local validity state, but the mechanism wasn't traced
  in this pass.
- **Coroutine / async operand variants.** mrustc predates
  significant async/coroutine MIR work in modern rustc; the
  variant-set comparison here is on the synchronous-Rust
  core. No coroutine-shaped operand variants surfaced in
  the source files inspected.
- **Span / source-location tracking.** mrustc's MIR uses
  per-statement source attribution rather than per-operand
  span; not pursued because RegseqModel's D-6-1 has already
  declared use-site span recoverable from the containing
  Statement's SourceInfo at stage 8.
