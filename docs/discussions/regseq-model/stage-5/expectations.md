# Expectations on stage 5 (incoming)

Stage 5 covers MIR places and projections. It is the head of the
MIR pipeline (5 -> 6 -> 7 -> 8 -> 9 -> 10 -> 11): each subsequent
stage's content references the previous stage's vocabulary.

The convention is described in `../README.md` under "Forward
expectations live in `stage-N/expectations.md`."

## Status at stage opening

**No inbound forward refs filed.** At the time stage 5 opens:

- No earlier stage filed a forward ref tagged "stage 5" /
  "stage-5" in its `spec-snapshot.md` or `discussions.md`.
- The `stage-5/` folder did not exist before this opening; lazy
  creation happened with the stage's opening, not with an
  earlier stage's prefiling.
- Top-level `../status.md` "Deferred items (cross-stage)"
  carries no entries naming stage 5 as the closing stage.

The closest things to forward-references are *usage* mentions in
prose -- e.g., the top-level `../spec.md` parenthetically
references `PlaceElem::Field(FieldIdx, _)` when describing why
ZST fields remain in the IR for projection purposes. These are
not filed expectations; they are anticipations that stage 5 will
formalise the matching surface. They do not constitute a
checklist item.

## Stage-5 deliverables (derived, not filed)

These are not "incoming expectations" -- they are the natural
scope stage 5 must address as the MIR pipeline's head:

- The `Place` value type: a head-local plus a projection chain.
- The `PlaceElem` enum: the projection vocabulary. Rustc's
  `ProjectionElem` enum has 8 variants (Deref, Field, Index,
  ConstantIndex, Subslice, Downcast, OpaqueCast,
  UnwrapUnsafeBinder per `prior-art/rustc.md` §2). Of these,
  6 are mandatory + 1 optional reachable in our `optimized_mir`
  input (`OpaqueCast` is stripped; `UnwrapUnsafeBinder` is
  gated). D-5-1 closes the closed-vocabulary set. Note:
  `Subtype` is **not** a `PlaceElem` -- it lives on `CastKind`
  (stage 7).
- Identity / equality / hashing rules for `Place` (used downstream
  in dataflow analyses and in operand-/rvalue-/statement-level
  identity once stages 6-8 land).
- C-semantics framing: how each `PlaceElem` lowers to a C lvalue
  expression (Deref -> unary `*`, Field -> `.fname`, Index ->
  `[i]`, etc.).
- Bridge mapping: rustc `mir::Place<'tcx>` /
  `mir::PlaceElem<'tcx>` to Regseq `Place` / `PlaceElem`, with
  the closed-vocabulary rejection rule for any variant outside
  the locked set.
- Validation rules in §3.3 Phase B: place type-coherence (each
  projection narrows the type from the previous step), local
  existence (the head local must be a valid `LocalId` in the
  enclosing body once stage 11 introduces `LocalDecls`), field
  / variant index range checks against the projected type's
  layout.
- Renderer intent: whether `pretty.rs::render_place` lands in
  stage 5 itself, or defers to a later stage (mirroring D14 ->
  stage-13 and D-4-K -> stage-14 precedents). Decide
  deliberately based on dependency-chain analysis.

If any of these deliverables uncovers a constraint that later
stages must satisfy (e.g., "stage 11's `LocalDecl` must carry
type X to make Place validation cheap"), file it in the relevant
later stage's `expectations.md` per the project convention.

## Cross-stage dependencies stage 5 inherits

These are not requirements *on* stage 5; they are upstream
content stage 5 reads:

- **`LocalId` (stage-11 introduction).** `Place.local: LocalId`.
  At stage-5 opening, `LocalId` is a forward-declared concept
  per `../stage-11/expectations.md` "From stage 1"; the typed-ID
  becomes a Rust type when stage 11 (or stage 10's `BlockId` arm
  if it lands first) introduces the const-body `newtype_index!`
  arm. Stage 5's spec can reference `LocalId` by name as the
  head of a `Place`; the implementation sweep at end-of-phase-1
  materialises it.

- **`TyId` and the `Ty` enum (stage-3, locked).** Place
  validation walks projections through the type system: each
  `PlaceElem` derives a successor type from the previous step's
  type. `Ty` content (`Ty::Adt`, `Ty::Tuple`, `Ty::Array`,
  `Ty::Slice`, `Ty::Ref`, `Ty::RawPtr`, `Ty::Dynamic`, ...) is
  pinned in stage 3 / `../spec.md` Section 1; stage 5 reads it.

- **`FieldIdx` and `VariantIdx`.** `PlaceElem::Field` carries a
  `FieldIdx`; `PlaceElem::Downcast` carries a `VariantIdx`.
  These index types are stage-3 territory (the layout fields
  vector indexed by `FieldIdx` per stage-3 D11; the variant
  vector indexed by `VariantIdx` per stage-3 D6). Stage 5 reads
  them.

- **`Const` (stage-4, locked).** `PlaceElem::ConstantIndex`
  carries a constant offset; stage 4's `Const` provides the
  scalar value form. The stage-4 `Const::Uint(Usize, _)` variant
  is the natural payload for offsets.

## Outbound forward refs filed by stage 5

(Populated as stage 5 proceeds. Outbound refs go in the
*receiving* stage's `expectations.md`, not here. This section
serves as a register of where stage 5 has filed downstream
work, for cross-checking at stage close.)

(none yet)
