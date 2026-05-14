# regseq-model-80: Terminator + TerminatorKind + CallCallee + SwitchTargets + AssertKind (P1a-DS-13b)

## Description

Implement the `Terminator` IR data shape per `spec.md` §23: the
`Terminator` wrapper struct (`{ kind, span }`), the closed
6-variant `TerminatorKind` enum (`Goto(BlockId)`, `SwitchInt {
discr, targets }`, `Return`, `Unreachable`, `Call { callee, args,
destination, target }`, `Assert { cond, expected, msg, target }`),
and the supporting sub-enums / sub-structs (`CallCallee` 2-arm,
`SwitchTargets { cases, otherwise }`, `AssertKind` 8-variant
panic-message taxonomy).

`Terminator` is the control-flow-edge surface of MIR -- the typed
expression of "how a basic block exits and where execution
continues." Body-local content; lives inline inside basic-block
storage; equality is structural.

**The `Drop` rustc variant is collapsed at the bridge per §23.4 --
no IR-side `Drop` variant exists.** This task does not introduce
`Drop` into RegseqModel; the Drop-as-Call collapse is bridge-side
(P1a-BR-06).

## Input Context

- `docs/discussions/regseq-model/spec.md` §23.1 (`Terminator` +
  `TerminatorKind` + variant-drop registry; `spec.md:8916-9097`)
- `docs/discussions/regseq-model/spec.md` §23.1.4
  (`Terminator.successors()` per-variant table;
  `spec.md:9067-9097`)
- `docs/discussions/regseq-model/spec.md` §23.2 (Per-variant
  payload shapes; `spec.md:9098-9208`)
- `docs/discussions/regseq-model/spec.md` §23.3 (Sub-enums and
  sub-structs -- `CallCallee`, `SwitchTargets`, `AssertKind`;
  `spec.md:9209-9339`)
- `docs/discussions/regseq-model/spec.md` §23.4 (Drop-as-Call
  collapse -- bridge-side; `spec.md:9340-9489`) -- referenced
  for Non-Goals
- `docs/discussions/regseq-model/spec.md` §23.5 (Identity and
  storage; `spec.md:9490-...`)
- `docs/discussions/regseq-model/spec.md` §23.6 (Terminator-site
  invariants -- referenced for Non-Goals)
- `crates/regseq-model/src/operand.rs` (`Operand` from
  regseq-model-77)
- `crates/regseq-model/src/place.rs` (`Place` from regseq-model-76)
- `crates/regseq-model/src/source.rs` (`Span`)
- `crates/regseq-model/src/ids.rs` (`InstanceId`, `BlockId`)
- `crates/regseq-model/src/statement.rs` (regseq-model-79;
  same module-organization conventions for body-local types)
- `.harness/spec-batches/regseqmodel-phase-1a.md` (parent brief;
  P1a-DS-13b row)

## Implementation Notes

Create a new module `crates/regseq-model/src/terminator.rs`
(re-exported from `lib.rs`). The sub-enums (`CallCallee`,
`SwitchTargets`, `AssertKind`) live in the same module.

`BlockId` is introduced by regseq-model-81 (P1a-DS-14a); this
task uses `BlockId` as an opaque type and depends on
regseq-model-81 landing first. Document the dependency at the
module preamble.

### Public API -- Terminator wrapper struct

Per `spec.md:8920-8929`:

```rust
/// A control-flow transfer ending a basic block, with the
/// source location it derives from. Body-local content; lives
/// inline inside basic-block storage, not in any top-level
/// interner table. Equality is structural.
pub struct Terminator {
    pub kind: TerminatorKind,
    pub span: Span,
}
```

The wrapper carries exactly one diagnostic field (`span`)
beyond the variant payload. `source_info.scope: SourceScope`
(DWARF-emission only) is dropped per `spec.md:8932-8946`.

### Public API -- TerminatorKind (closed 6-variant)

Per `spec.md:8949-9007`:

```rust
/// Closed 6-variant set covering every basic-block exit in
/// well-formed driver MIR post-pipeline. Any rustc
/// `TerminatorKind<'tcx>` variant outside this set is either
/// pipeline-discharged (FalseEdge, FalseUnwind, Yield,
/// CoroutineDrop, UnwindResume, UnwindTerminate) or
/// DSL-rejected (TailCall, InlineAsm); see §23.6.1. Drop is
/// collapsed at the bridge to a synthetic Call per §23.4.
pub enum TerminatorKind {
    /// Unconditional branch. Single successor.
    Goto(BlockId),

    /// Multi-target conditional branch on an integer-typed
    /// discriminant. The closed-form `cases` list plus an
    /// `otherwise` block; see §23.3.2.
    SwitchInt {
        discr:   Operand,
        targets: SwitchTargets,
    },

    /// Function return. The return value lives at
    /// `LocalId::RETURN_PLACE` (`_0`); its type is
    /// `FnSig.output()` (stage 11). No payload.
    Return,

    /// Terminating UB marker. Executing this terminator is
    /// undefined behaviour. Emitted naturally from `loop {}`
    /// patterns and as the output of cleanup-pass
    /// simplifications under panic=abort.
    Unreachable,

    /// Function call. `callee` distinguishes compile-time-
    /// resolved Direct calls from runtime-resolved Indirect
    /// calls (function-pointer calls and post-rewrite virtual
    /// dispatch); see §23.3.1. `target = None` indicates a
    /// divergent call (callee does not return; e.g.,
    /// `panic!()`, `-> !` callee).
    Call {
        callee:      CallCallee,
        args:        Box<[Operand]>,
        destination: Place,
        target:      Option<BlockId>,
    },

    /// Guarded conditional branch. If `cond != expected`, the
    /// runtime invokes the panic helper with `msg`; otherwise
    /// execution continues at `target`. Under panic=abort,
    /// "invokes the panic helper" means "calls a chosen abort
    /// routine and follows __builtin_unreachable()".
    Assert {
        cond:     Operand,
        expected: bool,
        msg:      AssertKind,
        target:   BlockId,
    },
}
```

Cardinality 6. The `Drop` rustc variant is **NOT** present;
it is collapsed at the bridge to a synthetic Call per §23.4.

### Public API -- CallCallee (closed 2-arm)

Per `spec.md:9217-9244`:

```rust
/// Closed 2-arm classification of `Call.callee`. `Direct`
/// carries a compile-time-resolved `InstanceId`; `Indirect`
/// carries a runtime function-pointer Operand. Universal IR
/// distinction across LLVM (`call` vs SSA-value), Cranelift
/// (`call` vs `call_indirect`), Charon (`FnOperand::{
/// Regular(FnPtr), Move(Place) }`), and mrustc
/// (`CallTarget::{ Path, Value, Intrinsic }`).
pub enum CallCallee {
    /// Direct call: callee resolves at compile time to a
    /// specific InstanceKind (Item, Intrinsic, shim, drop-
    /// glue, etc.). Subject to the closure-completeness
    /// invariant per §23.6.2 B-7.
    Direct(InstanceId),

    /// Indirect call: callee is a runtime function-pointer
    /// value. Used for (a) function-pointer calls --
    /// `let f: fn(i32) -> i32 = some_fn; f(x)`, and (b)
    /// post-stage-11-rewrite virtual dispatch -- vtable-loaded
    /// function pointer through the receiver's fat-pointer
    /// metadata field. Subject to the operand-type-is-FnPtr
    /// invariant per §23.6.2 B-5.
    Indirect(Operand),
}
```

### Public API -- SwitchTargets

Per `spec.md:9263-9273`:

```rust
/// Closed-form switch target table: explicit (case_value ->
/// block) pairs plus a structurally separate otherwise
/// destination. No positional "last entry is otherwise"
/// convention; the type system enforces the otherwise field
/// is present.
pub struct SwitchTargets {
    /// (case value, branch target) pairs. Empty when the
    /// switch has no cases; the otherwise field then carries
    /// the entire dispatch.
    pub cases: Box<[(u128, BlockId)]>,

    /// Catch-all destination when `discr` matches no case
    /// value.
    pub otherwise: BlockId,
}
```

Case-value width is `u128` per `spec.md:9276-9282` (RegseqModel
uses bare `u128`; rustc's `Pu128` HashStable wrapper is not
inherited).

### Public API -- AssertKind (closed 8-variant)

Per `spec.md:9295-9332`:

```rust
/// Closed 8-variant set of panic-message kinds for
/// `Assert.msg`. Subset of rustc's 11-variant `AssertKind<O>`
/// (drops 3 coroutine-only variants per D-9-5:
/// `ResumedAfterReturn`, `ResumedAfterPanic`,
/// `ResumedAfterDrop`). Coroutines are stage-3 §15.1
/// type-rejected, so the dropped variants are structurally
/// unreachable in well-formed RegseqModel bodies.
pub enum AssertKind {
    /// Index-out-of-bounds: `index >= len`.
    BoundsCheck { len: Operand, index: Operand },

    /// Arithmetic overflow on a checked binary op. `BinOp`
    /// reuses regseq-model-78's BinOp vocabulary.
    Overflow(BinOp, Operand, Operand),

    /// Negation overflow: `-i128::MIN` etc.
    OverflowNeg(Operand),

    /// `x / 0`. Operand is the divisor.
    DivisionByZero(Operand),

    /// `x % 0`. Operand is the divisor.
    RemainderByZero(Operand),

    /// `*p` where p has alignment requirement R but
    /// `(p as usize) % R != 0`. `required` is R as a
    /// usize-typed operand; `found` is `p` as a usize.
    MisalignedPointerDereference {
        required: Operand,
        found:    Operand,
    },

    /// `*p` where `p` is null. No operand carried (the failing
    /// pointer was a null literal).
    NullPointerDereference,

    /// Construction of an enum value with a discriminant
    /// outside the valid set. Operand is the out-of-range
    /// discriminant value.
    InvalidEnumConstruction(Operand),
}
```

### Conventions

- **No `Drop` variant.** Per `spec.md:9056-9062` and §23.4,
  rustc's `Drop` is collapsed at the bridge to a synthetic
  `Call` to drop-glue. There is **no** `TerminatorKind::Drop`
  in RegseqModel.
- **No `unwind: UnwindAction` field on Call or Assert.** Per
  `spec.md:9170-9175` and `spec.md:9205-9207`, dropped under
  panic=abort.
- **No `call_source: CallSource`, no `fn_span: Span` on Call.**
  Per `spec.md:9176-9181`, both diagnostic-only.
- **`args: Box<[Operand]>` slim-pointer storage** matches
  §21's Aggregate convention. **Per-arg `Spanned<Operand>`
  spans are dropped** at the bridge per `spec.md:9158-9162`.
- **`destination: Place` is always present** in optimized_mir
  (rustc pre-allocates the destination Place post-cleanup;
  source-MIR's `Option<Place>` is gone) per
  `spec.md:9162-9165`.
- **`SwitchTargets.cases: Box<[(u128, BlockId)]>`** uses bare
  `u128` (not rustc's `Pu128` wrapper).
- **`BlockId` is an opaque dependency** introduced by
  regseq-model-81 (P1a-DS-14a). Import via
  `use crate::basic_block::BlockId;` (or the project's chosen
  module path).
- **`AssertKind::Overflow` reuses regseq-model-78's `BinOp`
  vocabulary.** Import via `use crate::rvalue::BinOp;`.
- **`Terminator.successors()` helper.** Per `spec.md:9067-9097`,
  expose `Terminator::successors(&self) -> impl Iterator<Item =
  BlockId>` with the per-variant order pinned by the spec
  table:
  - `Goto(target)` -> `[target]`
  - `SwitchInt { targets: SwitchTargets { cases, otherwise }
    }` -> each `(_, target)` in `cases` (storage order)
    followed by `[otherwise]`
  - `Return` -> `[]`
  - `Unreachable` -> `[]`
  - `Call { target: Some(t), .. }` -> `[t]`
  - `Call { target: None, .. }` -> `[]`
  - `Assert { target, .. }` -> `[target]`
  This is consumed by the body-walk (P1a-VAL-02 / P1a-DS-14b)
  for B-Successors-Valid pre-check + DFS-RPO.
- **Worst-case enum width** documented in the type-level doc
  comment per `spec.md`'s sizing notes (informational only).
- **Serde**: all types carry `Operand` / `Place` / `Span` /
  typed-IDs / primitives -- all serde-friendly. Use
  `#[derive(Serialize, Deserialize)]`.
- **Derive `Clone, Debug, PartialEq, Eq, Hash`** on all five
  types (`Terminator`, `TerminatorKind`, `CallCallee`,
  `SwitchTargets`, `AssertKind`).
- **Doc comments mandatory** on every public type, variant,
  field. Cite §23.x subsections per the project's
  spec-traceability convention.

## Non-Goals

- **Drop-as-Call collapse** (`spec.md:9340-9489`) is bridge-
  side (P1a-BR-06). This task does NOT introduce a `Drop`
  variant; the absence is the spec.
- **Bridge translation from rustc `mir::TerminatorKind<'tcx>`**
  (the §23.7 per-variant table; `spec.md:9835-9903`) is
  P1a-BR-06.
- **Pipeline-discharged variant rejections** (`FalseEdge`,
  `FalseUnwind`, `Yield`, `CoroutineDrop`, `UnwindResume`,
  `UnwindTerminate`, `TailCall`, `InlineAsm` per
  `spec.md:9015-9054`) are bridge-side rejections (P1a-BR-06).
- **Terminator-site invariants** (B-1 `discr` type,
  B-2 `cond: Ty::Bool`, B-3 destination-type-matches-callee-
  return, B-4 args-length-matches-callee-arity, B-5 indirect-
  callee-FnPtr, B-6 per-arg-type-match, B-7 Direct closure
  completeness, B-8 vtable-indirect-rewrite-shape, B-9
  successors-valid per `spec.md` §23.6.2) are body-walk
  territory: P1a-VAL-01 + P1a-VAL-02 + P1a-VAL-03.
- **C lowering** for `TerminatorKind` (§23 C-lowering rows) is
  phase-1b.
- **Rendering** (`render_terminator`, `render_call_callee`,
  `render_switch_targets`, `render_assert_kind`) lives at
  `pretty-printer.md` §7.5-§7.9 and is implemented in
  P1a-PP-03.
- **Vtable-indirect rewrite** (`spec.md` §25.7.2) is body-pass
  territory (P1a-BR-07).

## Test Plan

**Test organization**: `unit-in-src` -- tests live in
`crates/regseq-model/src/terminator.rs` `#[cfg(test)] mod
tests`.

Coverage requirements (every public type / variant exercised at
least once per `SESSION.md` Testing Requirements):

1. **`Terminator` construction.** Build with `kind:
   TerminatorKind::Return` and a fixture `Span`; assert reads.
2. **`TerminatorKind::Goto` construction.** Build with
   `BlockId(3)`; assert single-field tuple-variant access.
3. **`TerminatorKind::SwitchInt` construction.** Build with a
   non-empty `SwitchTargets`; assert `discr` and `targets`
   field-by-field reads.
4. **`TerminatorKind::Return` construction.** Build the unit
   variant; assert distinctness from other unit-like variants.
5. **`TerminatorKind::Unreachable` construction.** Build the
   unit variant; assert distinctness from `Return`.
6. **`TerminatorKind::Call` construction (Direct callee).**
   Build with `callee: CallCallee::Direct(InstanceId(7))`,
   `args: Box::new([Operand::Constant(_)])`,
   `destination: Place::from(LocalId(0))`, `target: Some(BlockId(1))`.
7. **`TerminatorKind::Call` construction (Indirect callee).**
   Build with `callee: CallCallee::Indirect(Operand::Copy(_))`.
8. **`TerminatorKind::Call` -- divergent (`target: None`).**
   Build with `target: None`; assert.
9. **`TerminatorKind::Assert` construction.** Build with
   `cond: Operand::Constant(_)`, `expected: true`, `msg:
   AssertKind::NullPointerDereference`, `target: BlockId(2)`.
10. **All 6 TerminatorKind variants pairwise distinct.** Build
    one of each (using whatever payload makes sense); assert
    all `(6 choose 2) = 15` pairs are inequal.
11. **`CallCallee::Direct` / `CallCallee::Indirect` distinct
    over different payloads.** Build one of each; assert
    inequality.
12. **`CallCallee::Direct` equality.** Two `Direct(InstanceId(N))`
    with the same `N` are `==`; differ on `N` -> inequal.
13. **`SwitchTargets` construction -- empty cases.** Build
    with `cases: Box::new([])` and a non-trivial `otherwise`;
    assert.
14. **`SwitchTargets` construction -- non-empty cases.** Build
    with `cases: Box::new([(0u128, BlockId(1)), (1u128,
    BlockId(2))])` and `otherwise: BlockId(3)`; assert
    iteration / length.
15. **`SwitchTargets` equality / Hash.** Two structurally
    identical `SwitchTargets` are `==` and hash-equal.
16. **`AssertKind::BoundsCheck` construction.** Build with
    `len` and `index` operands; assert named-field reads.
17. **`AssertKind::Overflow` construction.** Build with
    `(BinOp::Add, op1, op2)`; assert tuple-variant reads.
18. **`AssertKind::OverflowNeg` construction.** Build with
    operand; assert.
19. **`AssertKind::DivisionByZero` / `RemainderByZero`.** Build
    one of each; assert distinctness.
20. **`AssertKind::MisalignedPointerDereference`
    construction.** Build with `required` and `found`
    operands; assert named-field reads.
21. **`AssertKind::NullPointerDereference` construction.**
    Unit variant; assert distinctness from operand-bearing
    variants.
22. **`AssertKind::InvalidEnumConstruction` construction.**
    Build with the out-of-range discriminant operand; assert.
23. **All 8 AssertKind variants pairwise distinct.** Build
    one of each; assert all `(8 choose 2) = 28` pairs
    inequal.
24. **`Terminator::successors()` -- `Goto(target)`** yields
    `[target]`.
25. **`Terminator::successors()` -- `SwitchInt`** yields the
    `cases` order followed by `otherwise` per
    `spec.md:9077`. Test with both empty and non-empty
    `cases`.
26. **`Terminator::successors()` -- `Return`** yields `[]`.
27. **`Terminator::successors()` -- `Unreachable`** yields
    `[]`.
28. **`Terminator::successors()` -- `Call { target: Some(t), .. }`**
    yields `[t]`.
29. **`Terminator::successors()` -- `Call { target: None, .. }`**
    yields `[]`.
30. **`Terminator::successors()` -- `Assert { target, .. }`**
    yields `[target]`.
31. **No `TerminatorKind::Drop` variant.** Compile-time check:
    a `match` on `TerminatorKind` covering 6 explicit variants
    with no `_` arm compiles. (Confirms cardinality 6 and
    absence of Drop.)
32. **Serde round-trip -- `Terminator` (each TerminatorKind
    variant).**
33. **Serde round-trip -- `CallCallee` (each variant).**
34. **Serde round-trip -- `SwitchTargets`.**
35. **Serde round-trip -- `AssertKind` (each variant).**

`cargo test -p regseq-model` passes.

## Review

After tests pass, request a Codex review using the four-block
template:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-model-80 against its task spec at
.harness/tasks/regseq-model/80-terminator-and-sub-enums.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-model/src/terminator.rs
- crates/regseq-model/src/lib.rs

Focus on:
- Spec deviations (every rule in the Implementation Notes section).
- Variant counts: TerminatorKind=6 (NO Drop), CallCallee=2,
  AssertKind=8.
- Absence of unwind/call_source/fn_span fields per
  spec.md:9170-9181.
- `Terminator::successors()` per-variant order matches
  spec.md:9074-9082.
- Test coverage vs. the spec's Test Plan, especially the
  successors() per-variant cases and the no-Drop confirmation.
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

Iterate until VERDICT: APPROVED.

## Quality Gate

```
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

The authoritative gate is `task.py done` per `SESSION.md`.

## Acceptance Criteria

- New module `crates/regseq-model/src/terminator.rs` created
  and re-exported from `crates/regseq-model/src/lib.rs`.
- `Terminator { kind, span }` defined per `spec.md:8926-8929`.
- `TerminatorKind` closed 6-variant enum (Goto / SwitchInt /
  Return / Unreachable / Call / Assert; **NO** Drop) defined
  per `spec.md:8958-9007`.
- `CallCallee` closed 2-variant enum (Direct / Indirect)
  defined per `spec.md:9226-9244`.
- `SwitchTargets { cases: Box<[(u128, BlockId)]>, otherwise:
  BlockId }` defined per `spec.md:9264-9273`.
- `AssertKind` closed 8-variant enum defined per
  `spec.md:9295-9331`.
- `Terminator::successors()` helper per `spec.md:9067-9097`
  with the per-variant order pinned.
- `args: Box<[Operand]>` storage; per-arg spans dropped per
  `spec.md:9158-9162`.
- All public types carry doc comments citing the §23.x
  subsections.
- Test Plan items 1-35 implemented; all pass under
  `cargo test -p regseq-model`.
- `cargo fmt --all -- --check` and
  `cargo clippy -p regseq-model -- -D warnings` pass.
- Codex Task Review returns VERDICT: APPROVED.
