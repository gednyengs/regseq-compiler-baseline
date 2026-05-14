# regseq-model-81: BasicBlock + BlockId typed-index + container + ordering invariants (P1a-DS-14a)

## Description

Implement the `BasicBlock` IR data shape per `spec.md` §24: the
`BasicBlock` struct (`{ statements: Box<[Statement]>, terminator:
Terminator }`), the `BlockId` typed-index newtype (with the
`START_BLOCK = 0` const-body macro arm), the body-level container
shape (`IndexVec<BlockId, BasicBlock>`), and the documented
block-ordering invariants (B-RPO, B-Entry; walk enforcement
deferred to body-walk).

`BasicBlock` is the per-block container surface of MIR -- the
structural unit binding `Statement` and `Terminator` content into
the unit of control flow. Body-local content; lives inline inside
the body's basic-block table; equality is structural.

This task also extends the existing `newtype_index!` macro at
`crates/regseq-model/src/idx.rs` with a const-body arm so the
`START_BLOCK = 0` named const can be declared inline per
`spec.md:24.3.2`.

## Input Context

- `docs/discussions/regseq-model/spec.md` §24.1 (`BasicBlock`
  struct + dropped fields; `spec.md:10238-10373`)
- `docs/discussions/regseq-model/spec.md` §24.2 (Body-level
  basic-block container; `spec.md:10374-10477`)
- `docs/discussions/regseq-model/spec.md` §24.3 (`BlockId`
  typed-index + macro extension + size assertions + sigil;
  `spec.md:10478-10585`)
- `docs/discussions/regseq-model/spec.md` §24.4 (Block-ordering
  invariants -- B-RPO, B-Entry; `spec.md:10586-10715`)
- `docs/discussions/regseq-model/spec.md` §24.5 (Identity and
  storage; `spec.md:10716-10748`)
- `crates/regseq-model/src/statement.rs` (`Statement` from
  regseq-model-79)
- `crates/regseq-model/src/terminator.rs` (`Terminator` from
  regseq-model-80)
- `crates/regseq-model/src/idx.rs` (existing `newtype_index!`
  macro -- empty-body arm only at present; this task adds the
  const-body arm)
- `crates/regseq-model/src/lib.rs` (existing crate-level size
  assertions pattern; this task appends `BlockId` assertions)
- `.harness/spec-batches/regseqmodel-phase-1a.md` (parent brief;
  P1a-DS-14a row)

## Implementation Notes

Create a new module `crates/regseq-model/src/basic_block.rs`
(re-exported from `lib.rs`). The macro extension lands in the
existing `crates/regseq-model/src/idx.rs`.

### Public API -- BasicBlock struct

Per `spec.md:10248-10260`:

```rust
/// A basic block: zero or more straight-line statements
/// followed by exactly one control-flow terminator. Body-local
/// content; lives inline inside the body's basic-block table
/// at `Body.basic_blocks` (per §24.2). Equality is structural.
pub struct BasicBlock {
    /// The straight-line instruction sequence preceding the
    /// terminator. Finalized single-fill payload per §24.1.2;
    /// consumers index by `usize` or iterate via
    /// `&[Statement]` deref.
    pub statements: Box<[Statement]>,

    /// The control-flow transfer ending this block. Exactly
    /// one terminator per block (no Option; see §24.1.3).
    pub terminator: Terminator,
}
```

**Dropped fields**:
- `is_cleanup: bool` per `spec.md:10336-10348` (panic=abort
  pipeline-discharged; no driver-relevant information).
- `after_last_stmt_debuginfos: StmtDebugInfos<'tcx>` per
  `spec.md:10350-10362` (no DWARF emission target).

**Collapsed**:
- `terminator: Option<Terminator>` -> `terminator: Terminator`
  per `spec.md:10311-10333` (rustc's Option is construction-
  only; RegseqModel consumes post-construction MIR).

### Public API -- BlockId typed-index

Per `spec.md:10485-10501` and `spec.md:10510-10532`:

```rust
newtype_index! {
    pub struct BlockId {
        const START_BLOCK = 0;
    }
}
```

Generates u32-backed niche-encoded storage (via stage-1's
private `NonMaxU32`), `Idx` trait impl, manual `Ord`/`PartialOrd`
over the logical index, and a `Debug` impl rendering as
`BlockId(N)`. The `START_BLOCK` const is emitted inside the impl
as `pub const START_BLOCK: BlockId = BlockId::from_u32(0);`.

### Macro extension -- const-body arm

Per `spec.md:10510-10532`, extend the existing `newtype_index!`
macro at `crates/regseq-model/src/idx.rs` with a const-body arm:

```text
newtype_index! {
    pub struct $Name {
        const NAME_1 = INT_LITERAL_1;
        const NAME_2 = INT_LITERAL_2;
        ...
    }
}
```

Each `const NAME_K = INT_LITERAL_K;` line generates
`pub const NAME_K: $Name = $Name::from_u32(INT_LITERAL_K);`
inside the impl. Multi-const support is included
(zero-extra-cost) for IDs that may declare multiple named
consts. Integer-literal-only -- the use cases (RETURN_PLACE = 0,
START_BLOCK = 0) only need integer literals; do not generalize
to const expressions.

The existing empty-body arm (`pub struct $Name {}`) **continues
to work unchanged**.

### Size + niche assertions

Per `spec.md:10546-10560`, append to
`crates/regseq-model/src/lib.rs`:

```rust
const _: () = assert!(core::mem::size_of::<BlockId>() == 4);
const _: () = assert!(core::mem::size_of::<Option<BlockId>>() == 4);
```

Both assertions are non-vacuous: `BlockId == 4` checks the u32
backing; `Option<BlockId> == 4` catches a regression where the
`NonMaxU32` niche encoding silently breaks.

### Body-level container shape

Per `spec.md:10381-10398`:

```rust
// (Belongs to the Body struct in regseq-model-82; this task
// only documents the field's type and invariants.)
pub basic_blocks: IndexVec<BlockId, BasicBlock>,
```

Bare `IndexVec` storage -- no wrapper struct (per
`spec.md:10401-10413`, no `BasicBlocks<'tcx>` analog). No CFG
cache (per `spec.md:10416-10465`, the model-vs-analysis line
puts CFG analyses in `regseq-analysis`). No type alias (per
`spec.md:10467-10476`).

### Block-ordering invariants (documentation only)

Per `spec.md:10586-10638`:

- **B-RPO.** `Body.basic_blocks: IndexVec<BlockId, BasicBlock>`
  is stored in reverse-postorder of the CFG rooted at
  `BlockId::START_BLOCK`. Iterating the storage yields blocks
  in RPO; consumers may exploit this without re-deriving.
  Inheritance: rustc's `ReorderBasicBlocks` cleanup pass
  reorders the `IndexVec` storage; the bridge force-enables
  the pass per the cleanup-pass whitelist.
- **B-Entry.** `BlockId::START_BLOCK == BlockId(0)` is the
  entry block of every `Body`.
- **Walk enforcement** of B-RPO + B-Entry + B-Reachable +
  B-NonEmpty + B-Successors-Valid is **deferred to the
  body-walk** (regseq-model-82 / P1a-DS-14b sites the joint
  validation visitor; `spec.md:10641-10702` lays out the
  6-step DFS-RPO procedure).

Document both invariants in module-level doc comments and cite
the body-walk task ID where enforcement lands.

### Sigil convention

Per `spec.md:10562-10584`, the textual rendering of
`BlockId(N)` is `bbN`. The macro's auto-generated `Debug` impl
renders `BlockId(N)` (uniform with all other typed IDs); the
`bbN` sigil belongs to RegseqModel's pretty-printer and is
implemented at P1a-PP-03 (`render_basic_block` envelope).
Document the sigil convention in the type-level doc comment as
a forward-pointer.

### Conventions

- **Equality and hashing** are structural via
  `derive(PartialEq, Hash)` on `BasicBlock`, walking through
  to `Statement` and `Terminator` derives (which themselves
  recurse into Operand / Place / InstanceId / TyId / ConstId).
- **`statements: Box<[Statement]>` storage** matches the
  finalized-payload convention per `spec.md:10286-10309`
  (saves 8 bytes vs `Vec<Statement>` no spare-capacity field).
- **`terminator: Terminator` non-Optional** per
  `spec.md:10311-10333`. RegseqModel mirrors the structural
  guarantee at the type level rather than carrying a runtime
  invariant.
- **No block-level metadata** (no span, no scope) per
  `spec.md:10364-10372`. Spans propagate via per-`Statement`
  and per-`Terminator` `span` fields.
- **`F4` / `F4-2` cross-build identity is OUT of scope.**
  `BasicBlock` is body-local content; per-build, structural
  equality only.
- **Serde**: `BasicBlock` carries `Box<[Statement]>` +
  `Terminator` (both serde-friendly via regseq-model-79 / 80).
  `BlockId` is a typed-index newtype; mirror the project's
  serde discipline for other typed-index newtypes (e.g.,
  `LocalId`, `InstanceId`). Use
  `#[derive(Serialize, Deserialize)]` on `BasicBlock`. The
  `newtype_index!` macro should produce serde derives if the
  empty-body arm already does so; if not, the const-body arm
  must preserve identical serde semantics.
- **Derive `Clone, Debug, PartialEq, Eq, Hash`** on
  `BasicBlock`.
- **Doc comments mandatory** on every public type / field.
  Reference §24.1.X subsections per the spec-traceability
  convention. Document the dropped fields (`is_cleanup`,
  `after_last_stmt_debuginfos`) and the Option-collapse on
  `terminator` in the type-level doc comment for `BasicBlock`.

## Non-Goals

- **Body-level walk procedure** (the §24.4.3 6-step DFS-RPO
  procedure; `spec.md:10641-10702`) is implemented in
  regseq-model-82 (P1a-DS-14b)'s joint validator scaffolding
  and exercised by P1a-VAL-02.
- **Bridge translation from rustc `mir::BasicBlockData<'tcx>`**
  (the §24.7 per-rustc-block translation) is P1a-BR-06.
- **Defensive bridge-side rejection** of `is_cleanup=true`
  blocks (§24.6.1 row 1) is bridge-side (P1a-BR-06).
- **C lowering** for `BasicBlock` is phase-1b
  (`generic-c-backend.md` §7).
- **Rendering** (`render_basic_block`) lives at
  `pretty-printer.md` §7.10 and is implemented in P1a-PP-03;
  the `bbN` sigil-rendering helper lives there.
- **`LocalId::RETURN_PLACE` migration to the const-body arm**
  is regseq-model-82 (P1a-DS-14b) territory per
  `spec.md:10537-10540`.
- **The `Body` struct itself** + the
  `Body.basic_blocks: IndexVec<BlockId, BasicBlock>` field is
  regseq-model-82 (P1a-DS-14b).

## Test Plan

**Test organization**: `unit-in-src` -- tests live in
`crates/regseq-model/src/basic_block.rs` and (for the macro
extension tests) in
`crates/regseq-model/src/idx.rs`'s `#[cfg(test)] mod tests`.

Coverage requirements (every public type / variant / accessor
exercised at least once per `SESSION.md` Testing Requirements):

1. **`BasicBlock` construction.** Build with
   `statements: Box::new([])` and a fixture `Terminator`
   (`Terminator { kind: TerminatorKind::Return, span: ... }`);
   assert field reads.
2. **`BasicBlock` with non-empty statements.** Build with two
   `Statement` entries in `statements: Box::new([s1, s2])`;
   assert `block.statements.len() == 2` and iteration yields
   the expected order.
3. **`BasicBlock` equality -- identical.** Two structurally
   identical `BasicBlock` values are `==`.
4. **`BasicBlock` equality -- differing statements.** Two
   `BasicBlock` values with identical terminator but differing
   statement vectors are inequal.
5. **`BasicBlock` equality -- differing terminator.** Two
   `BasicBlock` values with identical statements but differing
   terminator are inequal.
6. **`BasicBlock` Hash.** Insert two distinct `BasicBlock`
   values into a `HashSet`; assert size 2; insert a clone of
   each; assert size still 2.
7. **`BlockId::START_BLOCK` const.** Assert
   `BlockId::START_BLOCK == BlockId::from_u32(0)` and
   `BlockId::START_BLOCK.as_u32() == 0`.
8. **`BlockId` size.** Assert `size_of::<BlockId>() == 4`
   (covers the `const _: () = assert!(...)` at `lib.rs`).
9. **`Option<BlockId>` niche.** Assert
   `size_of::<Option<BlockId>>() == 4`.
10. **`BlockId` Debug rendering.** Assert
    `format!("{:?}", BlockId::from_u32(7))` yields a string
    containing `"BlockId"` and `"7"` (the precise format may
    be `BlockId(7)` per stage-1 convention).
11. **`BlockId` Ord behavior.** Assert
    `BlockId::from_u32(0) < BlockId::from_u32(1) <
    BlockId::from_u32(7)`.
12. **`newtype_index!` const-body arm -- single const.**
    Define a fixture typed-index in the `idx.rs` test module
    with the const-body arm
    (`newtype_index! { pub struct FixtureId { const FIRST = 5; } }`)
    and assert `FixtureId::FIRST == FixtureId::from_u32(5)`.
13. **`newtype_index!` const-body arm -- multi-const.**
    Define a fixture typed-index with two consts
    (`const A = 0; const B = 3;`) and assert both resolve
    correctly.
14. **`newtype_index!` empty-body arm -- still works.**
    Define a fixture typed-index with the empty body
    (`newtype_index! { pub struct EmptyFixtureId {} }`) and
    assert it can be constructed via `from_u32`. (Confirms the
    macro extension doesn't regress the empty-body arm.)
15. **`BasicBlock` serde round-trip -- empty statements.**
    Build a `BasicBlock` with empty statements + Return
    terminator; round-trip via `serde_json::to_string` /
    `from_str`; assert `==`.
16. **`BasicBlock` serde round-trip -- non-empty statements.**
    Round-trip a `BasicBlock` with two statements + a Call
    terminator; assert `==`.
17. **`BlockId` serde round-trip.** Round-trip
    `BlockId::from_u32(7)`; assert `==`.
18. **No `is_cleanup` field.** Compile-time check via struct
    init: `BasicBlock { statements, terminator }` -- adding
    a third field name (e.g., `is_cleanup: false`) yields a
    compile error. (Documented in test comment.)
19. **No `Option<Terminator>` -- terminator is required.**
    Compile-time check: `BasicBlock { statements,
    terminator: None }` yields a type error. (Documented in
    test comment.)

`cargo test -p regseq-model` passes.

## Review

After tests pass, request a Codex review using the four-block
template:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-model-81 against its task spec at
.harness/tasks/regseq-model/81-basic-block.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-model/src/basic_block.rs
- crates/regseq-model/src/idx.rs (the const-body macro arm
  extension only)
- crates/regseq-model/src/lib.rs (the appended size assertions
  for BlockId only)

Focus on:
- Spec deviations (every rule in the Implementation Notes section).
- BasicBlock has exactly two fields (statements + terminator);
  no is_cleanup; no after_last_stmt_debuginfos; terminator
  non-Optional.
- BlockId macro generates `pub const START_BLOCK: BlockId =
  BlockId::from_u32(0);` and the const-body arm preserves
  the empty-body arm semantics.
- Size + niche assertions present in lib.rs.
- Test coverage vs. the spec's Test Plan, especially the
  macro-arm tests.
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

- New module `crates/regseq-model/src/basic_block.rs` created
  and re-exported from `crates/regseq-model/src/lib.rs`.
- `BasicBlock { statements: Box<[Statement]>, terminator:
  Terminator }` defined per `spec.md:10248-10260`.
- `BlockId` typed-index introduced via `newtype_index! {
  pub struct BlockId { const START_BLOCK = 0; } }` per
  `spec.md:10485-10491`.
- `newtype_index!` macro at `crates/regseq-model/src/idx.rs`
  extended with the const-body arm per `spec.md:10510-10532`;
  the empty-body arm continues to work.
- Size + niche assertions appended to
  `crates/regseq-model/src/lib.rs` per `spec.md:10553-10560`.
- B-RPO and B-Entry invariants documented in module-level doc
  comments with forward-pointers to regseq-model-82 / P1a-VAL-02
  for walk enforcement.
- `bbN` sigil-rendering convention documented in the type-level
  doc comment as a forward-pointer to P1a-PP-03.
- All public types carry doc comments citing the §24.x
  subsections.
- Test Plan items 1-19 implemented; all pass under
  `cargo test -p regseq-model`.
- `cargo fmt --all -- --check` and
  `cargo clippy -p regseq-model -- -D warnings` pass.
- Codex Task Review returns VERDICT: APPROVED.
