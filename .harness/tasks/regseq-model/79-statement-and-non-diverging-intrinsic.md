# regseq-model-79: Statement, StatementKind, NonDivergingIntrinsic, CopyNonOverlapping (P1a-DS-13a)

## Description

Implement the `Statement` IR data shape per `spec.md` §22: the
`Statement` wrapper struct (`{ kind, span }`), the closed
3-variant `StatementKind` enum (`Assign(Place, Rvalue)`,
`SetDiscriminant(Place, VariantIdx)`,
`Intrinsic(NonDivergingIntrinsic)`), and the supporting
`NonDivergingIntrinsic` 2-variant sub-enum
(`Assume(Operand)`, `CopyNonOverlapping(CopyNonOverlapping)`)
plus the `CopyNonOverlapping { src, dst, count }` named-fields
struct.

`Statement` is the sub-terminator instruction surface of MIR --
the typed expression of "an in-block side-effect that is not a
control-flow transfer." Body-local content; lives inline inside
basic-block storage; equality is structural over `(kind, span)`.

## Input Context

- `docs/discussions/regseq-model/spec.md` §22.1 (`Statement` +
  `StatementKind`; `spec.md:8006-8217`)
- `docs/discussions/regseq-model/spec.md` §22.2 (Per-variant
  payload shapes; `spec.md:8218-8427`)
- `docs/discussions/regseq-model/spec.md` §22.3 (Sub-enums and
  sub-structs -- `NonDivergingIntrinsic` + `CopyNonOverlapping`;
  `spec.md:8428-8493`)
- `docs/discussions/regseq-model/spec.md` §22.4 (Identity and
  storage; `spec.md:8495-...`)
- `docs/discussions/regseq-model/spec.md` §22.5 (Statement-site
  invariants -- referenced for Non-Goals; enforcement deferred
  to body-walk)
- `crates/regseq-model/src/place.rs` (`Place` from regseq-model-76)
- `crates/regseq-model/src/operand.rs` (`Operand` from
  regseq-model-77)
- `crates/regseq-model/src/rvalue.rs` (`Rvalue` from
  regseq-model-78)
- `crates/regseq-model/src/source.rs` (`Span`)
- `crates/regseq-model/src/ids.rs` (`VariantIdx`)
- `.harness/spec-batches/regseqmodel-phase-1a.md` (parent brief;
  P1a-DS-13a row)

## Implementation Notes

Create a new module `crates/regseq-model/src/statement.rs`
(re-exported from `lib.rs`).

### Public API -- Statement wrapper struct

Per `spec.md:8010-8019`:

```rust
/// A sub-terminator instruction in a basic block, with the
/// source location it derives from. Body-local content; lives
/// inline inside basic-block storage, not in any top-level
/// interner table. Equality is structural.
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span,
}
```

Field order is `kind, span` per `spec.md:8052-8055` (matches
rustc_public's analog at `rustc_public/src/mir/body.rs:470-474`).

### Public API -- StatementKind (closed 3-variant)

Per `spec.md:8062-8094`:

```rust
/// A typed sub-terminator instruction. Body-local content;
/// appears inside `Statement.kind`.
pub enum StatementKind {
    /// In-place value assignment: `place := rvalue`. Does NOT
    /// call drop on the previous LHS value (drop is the
    /// explicit `Drop` terminator's job at stage 9). The LHS
    /// place type and the RHS rvalue's resulting type must be
    /// equal modulo lifetime erasure (§22.5.2 I-Statement-
    /// Assign-Type-Equality).
    Assign(Place, Rvalue),

    /// In-place enum tag-write: writes the discriminant for
    /// the named variant to the place, without touching
    /// variant payload fields. Matches rustc's
    /// `SetDiscriminant`: tag-only mutation; payload is
    /// independently set by separate `Assign` statements. The
    /// target place must have type `Ty::Enum { item, args,
    /// layout }` (§22.5.2 I-Statement-SetDiscriminant-Target);
    /// the variant index must be in-range.
    SetDiscriminant(Place, VariantIdx),

    /// Non-diverging rustc-internal hint primitive: an
    /// `Assume` (boolean hint to the optimizer that the
    /// operand is true) or a `CopyNonOverlapping` (inline
    /// non-overlapping byte copy without a terminator
    /// boundary). Distinct from regseq DSL semantic
    /// intrinsics, which flow through Calls (per Q7) -- this
    /// variant carries only rustc's own non-diverging
    /// primitives. See §22.3.1 for the closed sub-enum.
    Intrinsic(NonDivergingIntrinsic),
}
```

The 3 variants are exhaustive per `spec.md:8097-8102`. No
`Other` / `Opaque` arm.

### Public API -- NonDivergingIntrinsic (closed 2-variant)

Per `spec.md:8432-8455`:

```rust
/// The non-diverging rustc-internal hint primitives that
/// survive into RegseqModel as Statement-level content. Closed
/// 2-variant set; the bridge rejects any rustc
/// `mir::NonDivergingIntrinsic<'tcx>` arm outside this set
/// (current rustc has only the two arms; future additions land
/// as new sub-enum variants).
pub enum NonDivergingIntrinsic {
    /// Boolean hint: "the operand is true." Backends may use
    /// this to backtrack the boolean's derivation and elide
    /// subsequent bound checks. If the operand is false,
    /// semantics matches `TerminatorKind::Unreachable`.
    Assume(Operand),

    /// Inline non-overlapping byte copy:
    /// `*dst[i] := *src[i]` for `i in [0, count)`.
    /// `src` and `dst` are pointers (or references); `count`
    /// is `usize`. The src and dst regions must not overlap
    /// (caller obligation; not checked by the operation).
    CopyNonOverlapping(CopyNonOverlapping),
}
```

### Public API -- CopyNonOverlapping (named-fields struct)

Per `spec.md:8466-8482`:

```rust
/// Payload for `NonDivergingIntrinsic::CopyNonOverlapping`.
/// Named-fields struct (NOT a tuple) -- src/dst/count are all
/// `Operand` and order-confusable in a tuple shape; named
/// fields prevent silent wrong-direction copy bugs at the
/// bridge translation site and the stage-14 codegen site.
pub struct CopyNonOverlapping {
    pub src: Operand,
    pub dst: Operand,
    pub count: Operand,
}
```

The named-fields choice is **correctness-essential** per
`spec.md:8467-8476`; do not collapse to a tuple.

### Conventions

- **No DWARF / `SourceInfo.scope` carry.** Per
  `spec.md:8021-8044`, rustc's `SourceInfo { span, scope }` is
  collapsed to `span` only -- the `scope` field is
  debuginfo-only and dropped at the bridge translation site.
  Rustc's `Statement.debuginfos: StmtDebugInfos<'tcx>` is
  dropped entirely.
- **Flat `Assign(Place, Rvalue)` storage.** Per
  `spec.md:8222-8236`, no `Box<(Place, Rvalue)>` wrapper.
  RegseqModel does not inherit rustc's 16-byte
  `StatementKind<'_>` size budget.
- **Flat tuple for `SetDiscriminant(Place, VariantIdx)`.** Per
  `spec.md:8281-8293`. No struct-with-named-fields, no Box.
- **Flat `Intrinsic(NonDivergingIntrinsic)`.** Per
  `spec.md:8371-8378`, divergence from rustc's
  `Intrinsic(Box<NonDivergingIntrinsic>)`.
- **Nested `NonDivergingIntrinsic` sub-enum** per
  `spec.md:8380-8405`: do not fan out to top-level Statement
  variants. The two arms share the "rustc-internal hint
  primitive" semantic grouping; future rustc additions land
  as new sub-enum variants without expanding the closed
  Statement surface.
- **`Statement::Intrinsic` carries only rustc's
  `NonDivergingIntrinsic`** -- regseq DSL semantic intrinsics
  flow through `TerminatorKind::Call` per Q7
  (`spec.md:8408-8414`). No DSL-intrinsic arm here.
- **`SetDiscriminant.place` type narrowing.** Per
  `spec.md:8316-8324`, the target place must have type
  `Ty::Enum { item, args, layout }`. Other shapes (Struct,
  Union, Tuple, primitive, etc.) reject at the bridge per
  the I-Statement-SetDiscriminant-Target invariant
  (validator-side; P1a-VAL-01 / body walk).
- **Worst-case `Statement` width** ~96 bytes per
  `spec.md:8104-8112` (Assign dominates ~84 bytes plus span
  + discriminator). Documentation only.
- **Serde**: `Statement`, `StatementKind`,
  `NonDivergingIntrinsic`, `CopyNonOverlapping` carry
  `Place`, `Rvalue`, `Operand`, `VariantIdx`, and `Span` --
  all serde-friendly. Use
  `#[derive(Serialize, Deserialize)]`.
- **Derive `Clone, Debug, PartialEq, Eq, Hash`** on all four
  types.
- **Doc comments mandatory** on every public type, variant,
  field. Cite the spec subsection (§22.1.1, §22.1.2, §22.2.x,
  §22.3.x) in the doc comments per the project's
  spec-traceability convention.

## Non-Goals

- **Rejected rustc statement variants** (`FakeRead`,
  `AscribeUserType`, `BackwardIncompatibleDropHint`,
  `Coverage(...)`, `PlaceMention`, `Nop`, `StorageLive` /
  `StorageDead`, `Retag`, `ConstEvalCounter`, plus the
  `Assign(_, Rvalue::Ref(_, BorrowKind::Fake, _))` whole-
  statement NOP per `spec.md:8114-8208`) are bridge-side
  rejections (P1a-BR-05).
- **Bridge translation from rustc `mir::StatementKind<'tcx>`**
  (the §22.6 per-variant table) is P1a-BR-05.
- **Statement-site invariants** (I-Statement-Assign-Type-
  Equality, I-Statement-SetDiscriminant-Target,
  I-Statement-SetDiscriminant-VariantIdx, I-Statement-
  Intrinsic-Assume-OperandTy, I-Statement-Intrinsic-
  CopyNonOverlapping-OperandTys per `spec.md` §22.5.2) are
  body-walk territory: P1a-VAL-01 (Place validation
  propagation) + P1a-VAL-02 (ID referential integrity) +
  P1a-VAL-03 (FnSig consistency).
- **C lowering** for `Statement` (§22 C-lowering rows) is
  phase-1b.
- **Rendering** (`render_statement`) lives at
  `pretty-printer.md` §7.4 and is implemented in P1a-PP-03.
- **Per-statement `debuginfos` carry** (intentionally dropped
  per `spec.md:8030-8044`); no compatibility path.

## Test Plan

**Test organization**: `unit-in-src` -- tests live in
`crates/regseq-model/src/statement.rs` `#[cfg(test)] mod
tests`.

Coverage requirements (every public type / variant exercised at
least once per `SESSION.md` Testing Requirements):

1. **`Statement` construction.** Build with `kind:
   StatementKind::Assign(...)` and a fixture `Span`; assert
   field-by-field reads.
2. **`Statement` field order.** Confirm by struct-init that
   `kind` precedes `span` in the construction site (the spec
   pins the order; this is a documentation test).
3. **`StatementKind::Assign` construction.** Build with a
   `Place` + `Rvalue::Use(Operand::Constant(_))`; assert
   pattern match.
4. **`StatementKind::SetDiscriminant` construction.** Build
   with a `Place` + `VariantIdx(2)`; assert pattern match.
5. **`StatementKind::Intrinsic(Assume)` construction.** Build
   with `NonDivergingIntrinsic::Assume(Operand::Constant(c))`;
   assert pattern match descending into the nested sub-enum.
6. **`StatementKind::Intrinsic(CopyNonOverlapping)`
   construction.** Build with a `CopyNonOverlapping { src,
   dst, count }`; assert all three operands read back.
7. **All 3 StatementKind variants pairwise distinct.** Build
   one of each; assert all 3 pairs are inequal.
8. **`StatementKind::Assign` equality -- identical.** Two
   structurally identical `Assign(Place, Rvalue)` are `==`.
9. **`StatementKind::Assign` equality -- differing place.**
10. **`StatementKind::Assign` equality -- differing rvalue.**
11. **`StatementKind::SetDiscriminant` equality -- differing
    variant.** Two `SetDiscriminant(p, VariantIdx(0))` and
    `SetDiscriminant(p, VariantIdx(1))` are inequal.
12. **`Statement` equality -- spans participate.** Build two
    `Statement { kind: same, span: differing }`; assert
    inequality. (Verifies the wrapper carries `span` in
    structural equality.)
13. **`NonDivergingIntrinsic::Assume` construction.** Build
    with an `Operand`; assert pattern match.
14. **`NonDivergingIntrinsic::CopyNonOverlapping`
    construction.** Build wrapping a `CopyNonOverlapping`;
    assert single-field tuple-variant access.
15. **`NonDivergingIntrinsic` -- 2 variants distinct.** Build
    one of each; assert inequality.
16. **`CopyNonOverlapping` field-name discipline.** Build with
    `CopyNonOverlapping { src: a, dst: b, count: c }`; verify
    that swapping `src` and `dst` produces a structurally
    distinct value (equality returns false). This validates
    the named-fields-correctness rationale per
    `spec.md:8470-8476`.
17. **`CopyNonOverlapping` -- count typing.** Build with
    `count: Operand::Constant(_)` representing a usize-typed
    constant; assert the field reads back as the constructed
    operand. (The usize-typing is invariant-side per
    `spec.md:8492` -- this test only checks the storage shape
    accepts an operand.)
18. **`Statement` Hash discrimination.** Insert several
    structurally distinct `Statement` values into a `HashSet`;
    assert size matches the number of distinct values.
19. **Serde round-trip -- `Statement` (Assign).**
20. **Serde round-trip -- `Statement` (SetDiscriminant).**
21. **Serde round-trip -- `Statement` (Intrinsic / Assume).**
22. **Serde round-trip -- `Statement` (Intrinsic /
    CopyNonOverlapping).**
23. **Serde round-trip -- `NonDivergingIntrinsic` (each
    variant).**
24. **Serde round-trip -- `CopyNonOverlapping`.** A standalone
    round-trip of the named-fields struct.
25. **No `Box` indirection inspection.** Compile-time check
    via `static_assertions::assert_not_impl_any!` style
    (optional; if the project uses `static_assertions`) that
    `StatementKind::Assign` payload is flat. If
    `static_assertions` is not in the project, document the
    invariant in the type-level doc comment instead.

`cargo test -p regseq-model` passes.

## Review

After tests pass, request a Codex review using the four-block
template:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-model-79 against its task spec at
.harness/tasks/regseq-model/79-statement-and-non-diverging-intrinsic.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-model/src/statement.rs
- crates/regseq-model/src/lib.rs

Focus on:
- Spec deviations (every rule in the Implementation Notes section).
- Variant counts: StatementKind=3, NonDivergingIntrinsic=2.
- Field naming on `CopyNonOverlapping { src, dst, count }`
  (named fields, NOT a tuple).
- No DWARF / `SourceInfo.scope` carry per spec.md:8021-8044.
- Test coverage vs. the spec's Test Plan.
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

- New module `crates/regseq-model/src/statement.rs` created
  and re-exported from `crates/regseq-model/src/lib.rs`.
- `Statement { kind, span }` defined per `spec.md:8016-8019`.
- `StatementKind` closed 3-variant enum (Assign /
  SetDiscriminant / Intrinsic) defined per `spec.md:8062-8094`.
- `NonDivergingIntrinsic` closed 2-variant enum (Assume /
  CopyNonOverlapping) defined per `spec.md:8440-8455`.
- `CopyNonOverlapping { src, dst, count }` named-fields
  struct defined per `spec.md:8477-8482`.
- All public types carry doc comments citing the relevant
  §22.x subsection per the spec-traceability convention.
- Test Plan items 1-25 implemented; all pass under
  `cargo test -p regseq-model`.
- `cargo fmt --all -- --check` and
  `cargo clippy -p regseq-model -- -D warnings` pass.
- Codex Task Review returns VERDICT: APPROVED.
