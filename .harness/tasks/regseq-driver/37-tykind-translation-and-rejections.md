# regseq-driver-37: TyKind Translation and Rejections

## Description

Implement the bridge translation from rustc's `TyKind` into the
`regseq-model` `Ty` enum per `spec.md` §15.0, including the special-ADT
dispatch table (PhantomData / UnsafeCell / ManuallyDrop are
lang-item-flagged; Box / `c_void` / Atomic* are DSL-rejected), the
§15.1 type-translation row rejections, region erasure with
substantive-HRTB rejection per §17.1, and the Appendix A.1
`*mut c_void` rejection fixture.

This task lands the per-`TyKind` translation that subsequent bridge
tasks depend on for typing every Const, Operand, Place, and Statement.
It runs inside Phase 2 of the pipeline scaffolded by regseq-driver-36.

## Input Context

- `docs/discussions/regseq-model/spec.md` §15 (Bridge translation rules
  and rejections) -- especially §15.0 per-TyKind translation table at
  spec.md:5849-5864 and §15.1 rejection rows
- `docs/discussions/regseq-model/spec.md` §17.1 (Region erasure +
  substantive-HRTB rejection)
- `docs/discussions/regseq-model/spec.md` Appendix A.1 (`*mut c_void`
  rejection example at spec.md:13028+)
- `crates/regseq-model/src/ty/` (the 19-variant `Ty` enum landed by
  regseq-model-66)
- `crates/regseq-model/src/layout/` (Layout helpers landed by
  regseq-model-67)
- `crates/regseq-model/src/ty_intern.rs` (TyInterner from
  regseq-model-68)
- `crates/regseq-driver/src/bridge/mod.rs` (BridgeCtxt + BridgeError
  from regseq-driver-36)

## Implementation Notes

Create `crates/regseq-driver/src/bridge/ty.rs`. Wire `pub mod ty;` from
`crates/regseq-driver/src/bridge/mod.rs`. Extend `BridgeError` with the
§15.1 rejection variants and the §17.1 substantive-HRTB rejection
variant.

### Public API

```rust
// bridge/ty.rs

/// Translate a rustc `TyKind` into a `regseq_model::TyId`. Runs during
/// Phase 2 of `run_bridge`. Per spec.md §15.0 lines 5849-5864.
pub fn translate_ty<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    ty: rustc_middle::ty::Ty<'tcx>,
) -> Result<regseq_model::TyId, BridgeError>;

/// Erase regions on a rustc `Ty`, rejecting substantive HRTB binders
/// per spec.md §17.1.
pub fn erase_regions_or_reject<'tcx>(
    tcx: rustc_middle::ty::TyCtxt<'tcx>,
    ty: rustc_middle::ty::Ty<'tcx>,
) -> Result<rustc_middle::ty::Ty<'tcx>, BridgeError>;
```

### Per-TyKind translation table (spec.md §15.0 lines 5849-5864)

The translation runs as a `match` over `ty.kind()`. Per spec.md §15.0
each rustc `TyKind` row maps to either (a) one closed `Ty` variant,
(b) an Adt-special dispatch path, or (c) a §15.1 rejection.

### Special-ADT dispatch (ordered)

Per spec.md §15.0 the Adt arm dispatches as:

1. PhantomData -- emit per the `LangItem::PhantomData` lang-item flag
   (ManuallyDrop and UnsafeCell follow the same lang-item-flagged
   shape per spec.md §15.0).
2. UnsafeCell -- emit per `LangItem::UnsafeCell`; consumers query via
   the §17.2 `contains_unsafe_cell` helper (regseq-model-66).
3. ManuallyDrop -- emit per `LangItem::ManuallyDrop`.
4. Box -- reject with `BridgeError::DslRejection` per spec.md §15.1.
5. `core::ffi::c_void` (any pointee position other than `*mut c_void`
   appendix A.1 carve-out) -- reject per spec.md §15.1; the explicit
   Appendix A.1 fixture is integration-tested below.
6. `core::sync::atomic::Atomic*` -- reject per spec.md §15.1.
7. Any other ADT -- generic `Ty::Adt` translation per spec.md §15.0.

### Region erasure + substantive-HRTB rejection (spec.md §17.1)

Per spec.md §17.1 the bridge erases regions before interning into the
`TyInterner`. Substantive HRTB binders (projection / nested /
non-ref-slot trait predicates AND relational binders per the brief's
caution carry-forward) are rejected with
`BridgeError::SubstantiveHrtb`.

### Module structure

- `crates/regseq-driver/src/bridge/ty.rs` -- the `translate_ty`
  dispatch + the special-ADT helpers + `erase_regions_or_reject`
- Extend `crates/regseq-driver/src/bridge/mod.rs::BridgeError` with
  `DslRejection { kind: DslRejectionKind, ty_repr: String }` and
  `SubstantiveHrtb { binder_repr: String }` variants

## Non-Goals

- Const translation (regseq-driver-38).
- FnSig translation (regseq-driver-39).
- Synthesised closure / vtable / drop-glue Item production
  (regseq-driver-40).
- Place / Operand / Rvalue / Statement translation (regseq-driver-41).
- Terminator translation (regseq-driver-42).
- Body materialisation (regseq-driver-43).
- §17.2 fat-pointer mutability rules (phase-1b territory per the brief
  Non-Goals).

## Test Plan

### Unit tests (`crates/regseq-driver/src/bridge/ty.rs` `#[cfg(test)] mod tests`)

- For each closed `Ty` variant produced by spec.md §15.0, assert the
  translation result identity for a hand-constructed `TyKind` input
  where one is feasible without a full rustc compilation (Bool, Char,
  Int, Uint, Float, Str, Never, primitive cases per the §15.0 table).
- Region-erasure helper: construct a non-substantive binder; assert
  it erases cleanly. Construct a substantive (relational) binder;
  assert `BridgeError::SubstantiveHrtb`.

### Integration tests (`crates/regseq-driver/tests/bridge_ty.rs`)

Bridge tests are integration-in-tests because they need rustc-compiled
fixtures.

Positive coverage (one fixture per non-rejection §15.0 row that needs
a real ADT / Adt-special dispatch path):

- PhantomData fixture -- `pub fn f(_: core::marker::PhantomData<u32>) {}`
  -- assert the result is an ordinary `Ty::Struct` carrying the
  `LangItem::PhantomData` Item flag (per §15.0 PhantomData translation
  rule at spec.md:5870+; the locked Ty enum has no `Phantom` variant
  per spec.md:3643). The lang-item flag is what distinguishes
  PhantomData from a generic Struct downstream.
- UnsafeCell fixture -- `pub fn f(_: &core::cell::UnsafeCell<u32>) {}`
  -- assert the result is `Ty::Struct` with the
  `LangItem::UnsafeCell` Item flag; assert
  `contains_unsafe_cell` returns true on the resulting Ty.
- ManuallyDrop fixture -- assert `Ty::Struct` with the
  `LangItem::ManuallyDrop` Item flag.
- Generic ADT fixture -- `pub struct S { x: u32 } pub fn f(_: S) {}`
  -- assert `Ty::Struct` with no lang-item flag.
- Reference / raw-pointer fixture -- assert `Ty::Ref` / `Ty::Ptr`.
- Slice fixture -- `pub fn f(_: &[u32]) {}` -- assert `Ty::Slice`.
- Array fixture -- `pub fn f(_: [u32; 4]) {}` -- assert `Ty::Array`.
- Tuple fixture -- `pub fn f(_: (u32, u8)) {}` -- assert `Ty::Tuple`.

Negative coverage (one fixture per §15.1 rejection row at the
type-translation phase). Per `spec.md:5904-5994` the §15.1
type-translation rejection rows include:

- **DSL-domain rejections** (the bridge rejects the construct as a
  policy decision):
  - Box fixture -- `pub fn f(_: Box<u32>) {}` -- assert
    `BridgeError::DslRejection { kind: Box, .. }`.
  - Atomic family fixture -- `pub fn f(_: &core::sync::atomic::AtomicU32) {}`
    -- assert `BridgeError::DslRejection { kind: Atomic, .. }`. Cover
    every Atomic* variant (AtomicU8/U16/U32/U64/I8/I16/I32/I64/Bool/Ptr/Usize/Isize).
  - `c_void` pointee fixture -- `pub fn f(_: *const core::ffi::c_void) {}`
    -- assert rejection.
  - **Appendix A.1 `*mut c_void` rejection fixture** (per spec.md:13028+)
    -- the explicit appendix example -- assert rejection.
- **Non-rigid TyKinds** (per locked §15.1 row at `spec.md:5957`:
  Param, Bound, Placeholder, Infer, Alias, Error). Post-
  monomorphization these should never appear; if they do, the
  bridge rejects:
  - Param / Bound / Placeholder / Infer / Alias / Error fixtures
    (synthetic; if such a TyKind reaches the bridge it's a rustc bug
    or a fixture wired to drive the rejection path -- assert the
    bridge rejects with `BridgeError::NonRigidTy { kind, .. }` for
    each of the six variants).
- **Foreign types** -- `extern { type Opaque; } pub fn f(_: &Opaque) {}`
  -- assert `BridgeError::Foreign`.
- **Coroutine family** -- async fn / coroutine / coroutine-closure
  fixtures -- assert `BridgeError::Coroutine`.
- **Pat (refinement types)** -- if/when rustc emits `TyKind::Pat`
  -- assert `BridgeError::Pat`.
- **UnsafeBinder** -- `unsafe<'a>` binder fixture -- assert
  `BridgeError::UnsafeBinder`.
- **Dynamic auto-only** -- `pub fn f(_: &dyn Send) {}` (no principal
  trait) -- assert `BridgeError::DynAutoOnly`.
- **DynStar** -- `dyn*` fixture -- assert `BridgeError::DynStar`.
- **Custom auto-traits** -- a user-defined `auto trait` fixture --
  assert `BridgeError::CustomAutoTrait`.
- **Substantive HRTB** -- a function with a `for<'a>` projection
  predicate -- assert `BridgeError::SubstantiveHrtb` (per §17.1).

**Out of scope for BR-02** (delegated to other bridge tasks): the
§15.1 rejection rows that fire at non-type-translation phases --
specifically `PlaceElem` rejections (Place translation, P1a-BR-05),
Statement-level CleanupPostBorrowck NOPed rows (Statement translation,
P1a-BR-05), and any C-ABI admissibility rejections (FnSig translation,
P1a-BR-03b). BR-02 owns only the type-translation rejection rows
that fire when translating a `TyKind` to a `Ty`.

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-driver-37 against its task spec at
.harness/tasks/regseq-driver/37-tykind-translation-and-rejections.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-driver/src/bridge/mod.rs
- crates/regseq-driver/src/bridge/ty.rs
- crates/regseq-driver/tests/bridge_ty.rs
- (any new fixture files under crates/regseq-driver/tests/fixtures/)

Focus on:
- Spec deviations vs spec.md §15.0 / §15.1 / §17.1 / Appendix A.1.
- Per-TyKind row coverage in tests.
- Special-ADT dispatch ordering.
- Rejection enforcement at the bridge site (closed-enum exhaustiveness
  is handled model-side).
- Test coverage vs. the spec's Test Plan including the Appendix A.1
  fixture.
Do NOT flag anything explicitly marked out of scope (Non-Goals,
later task IDs).
</task>

<grounding_rules>
Cite file:line for every claim and every required fix. Do not
speculate about code you did not read. Mark hypotheses as
hypotheses.
</grounding_rules>

<structured_output_contract>
First line: VERDICT: APPROVED | REQUIRES FIXES
If REQUIRES FIXES: numbered list, each entry exactly
  <file>:<line-range> -- <what is wrong> -- <what the spec requires>
If APPROVED: one paragraph (<=120 words) summarizing what was
verified and why any deliberate deviations are acceptable.
Total response <= 400 words.
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, form a verdict,
stop.
</default_follow_through_policy>
EOF
```

## Quality Gate

```
cargo test -p regseq-driver
cargo fmt --all -- --check
cargo clippy -p regseq-driver -- -D warnings
```

## Acceptance Criteria

- `crates/regseq-driver/src/bridge/ty.rs` created with `translate_ty`
  and `erase_regions_or_reject`.
- `BridgeError::DslRejection` and `BridgeError::SubstantiveHrtb`
  variants extended into `bridge/mod.rs`.
- Each spec.md §15.0 row covered by a positive test.
- Each spec.md §15.1 rejection row covered by a negative integration
  fixture, including the Appendix A.1 `*mut c_void` example.
- All tests pass; Codex Task Review verdict APPROVED.

## Audit Findings -- Re-execution Requirements

This task was previously executed and the implementation was reverted
because a Codex post-implementation audit (both a scoped-fix review and
the 4-partition re-review) identified material spec deviations. On
re-execution, the implementation MUST address every finding below in
addition to the original Implementation Notes.

### Tier A fix already landed

- `is_c_field_bad_pointee` is split from `is_c_boundary_bad_pointee`
  (in `bridge/fnsig.rs`) -- this is task 39's concern but the variant
  payload `CAbiTypePosition` enum lives in `bridge/mod.rs` and is
  consumed by FnPtr ABI translation. Preserve.

### Required additional work

1. **RustCall ABI rejection (spec section 14.2).** `translate_abi` in
   `bridge/ty.rs` MUST reject `ExternAbi::RustCall` (spec section 14.2
   admits `RustCall` only on rewritten synthesized closure-call shim
   items; raw FnPtr ABI must be Rust or C). To make the existing
   closure-call fixture pass, this task MUST ALSO implement the
   spec section 13.1 closure-call shim rewrite: every `Fn::call` /
   `FnMut::call_mut` / `FnOnce::call_once` direct call against a closure
   captures struct must be lowered into a Rust-ABI `ClosureCall`
   synthetic Function Item produced by `synth/closure.rs`, with the
   call arguments tuple-normalised. The bridge MUST substitute the
   call's callee from the rustc closure-trait method (RustCall) to
   the synthesized shim (Rust ABI) BEFORE `translate_abi` runs. The
   pre-existing test
   `bridge_body::translates_representative_place_and_rvalue_rows_from_mir`
   exercises this path and must pass post-implementation.

2. **Closure captures parallel Ty::Struct view (spec section 13.1).**
   Each `TyKind::Closure(def_id, args)` MUST intern BOTH:
   - `Ty::Closure(item, args)` (the existing intern), AND
   - A parallel `Ty::Struct { item, layout }` view of the closure's
     captures field-by-field, sharing the SAME ItemId as the
     `Ty::Closure` intern.
   Layout MUST be computed from `tcx.layout_of(closure_ty)` and field
   types from `ClosureArgs::upvar_tys()`. Add a
   `RegseqModel.closure_captures_struct_ty: HashMap<TyId, TyId>`
   (`#[serde(skip)]` mirroring `vtables`) mapping `Ty::Closure` TyId
   to its `Ty::Struct` view TyId.

3. **Dynamic vtable_ty resolved to VTableType-backed Ty::Struct (spec
   section 13.0 / 13.2).** `Ty::Dynamic.vtable_ty` MUST reference a
   real `Ty::Struct { item, layout, .. }` whose Item carries
   `SyntheticItemKind::VTableType { principal }`. The vtable Struct
   layout (pointer-width per slot, slot count from
   `tcx.vtable_entries(principal_poly_trait_ref)`) MUST go through
   `pending_items` and Phase-4 ItemId assignment -- DO NOT use any
   "eager push" helper that bypasses the F4 sort. Coordinate the
   ordering with the synth/vtable pipeline (regseq-driver-40) so the
   vtable Type Item exists in `pending_items` at translate-time and
   gets its final ItemId during Phase 4.

4. **No direct push to `model.items` from Phase 2 (spec section 3.4).**
   `intern_item` and any synthetic item helper called from
   `translate_erased_ty` MUST register items via `pending_items` /
   `HandleTable`, never via `model.items.push(...)` directly. Phase 3
   (F4 sort) and Phase 4 (ItemId assignment) own all final ItemId
   ordering.

### Test coverage

- Positive test: a closure expression interns both `Ty::Closure(item,
  args)` and the parallel `Ty::Struct { item, layout }` view, sharing
  the same ItemId; the captures field count and types match the
  closure body's upvars.
- Positive test: a `dyn Trait` type's `Ty::Dynamic.vtable_ty` resolves
  to `Ty::Struct(item, ...)` where `model.item_synthetic_kinds[item]
  == SyntheticItemKind::VTableType { principal }`.
- Negative test: an FnPtr with ExternAbi::RustCall NOT inside a
  closure-call shim context is rejected with the appropriate error
  variant (likely `BridgeError::UnsupportedTy` or a new variant).
- Negative test: every dropped rustc TyKind that section 15.1 lists
  (Bound / Placeholder / Infer / Alias / Error, plus Foreign /
  Coroutine / Pat / UnsafeBinder / DynStar / unbinder-substantive-HRTB)
  is rejected.
- `TyKind::Error` translation path MUST have a test (the prior
  implementation rejected via `NonRigidTyKind::Error` but no test
  exercised the path).
