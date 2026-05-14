# regseq-model-77: Operand and LiteralValue (P1a-DS-12a)

## Description

Implement the `Operand` IR data shape per `spec.md` §20: the
closed 3-variant `Operand` enum (`Copy(Place)`, `Move(Place)`,
`Constant(ConstId)`). Operand is the RHS-atom surface of MIR --
the typed expression of "which value." It is body-local content;
no top-level interner table; equality is structural.

`LiteralValue` is the data-shape carrier on top of which
`Const::*` literal variants are projected for Operand-context
rendering. The phase-1 spec keeps the literal-value vocabulary
inside the `Const` enum (§9.7); this task re-exports the relevant
literal-value types from the `Const` surface (already shipped by
P1a-DS-05 / regseq-model-69) so consumers in body-local contexts
have a stable, focused import path. **No new `LiteralValue` enum
is invented**: the locked spec embeds literal values inside
`Const` per `spec.md:2883-3286`.

## Input Context

- `docs/discussions/regseq-model/spec.md` §20.1 (`Operand` enum
  definition; `spec.md:6903-6940`)
- `docs/discussions/regseq-model/spec.md` §20.2 (Identity and
  storage; `spec.md:6959-6973`)
- `docs/discussions/regseq-model/spec.md` §20.5 (Operand-site
  invariants -- referenced; enforcement deferred to P1a-VAL-01;
  `spec.md:7094-7132`)
- `docs/discussions/regseq-model/spec.md` §9.7 (`Const` vocabulary
  and the literal-value carriers; the `Constant(ConstId)`
  variant references this through `model.consts`)
- `docs/discussions/regseq-model/spec.md` §20.3.1 (`RuntimeChecks`
  lowering rule -- referenced for Non-Goals; bridge-side
  P1a-BR-05)
- `crates/regseq-model/src/place.rs` (`Place` from regseq-model-76)
- `crates/regseq-model/src/ids.rs` (`ConstId`)
- `.harness/spec-batches/regseqmodel-phase-1a.md` (parent brief;
  P1a-DS-12a row)

## Implementation Notes

Create a new module `crates/regseq-model/src/operand.rs`
(re-exported from `lib.rs`).

### Public API -- Operand

Per `spec.md:6905-6940`:

```rust
/// A typed RHS atom: either a load from a place, or a constant
/// value. Body-local content; appears inside a body's rvalues /
/// statements / terminators, not in any top-level interner
/// table. Equality is structural.
pub enum Operand {
    /// Load a value from the place. Marks the place as
    /// participating in a copy-style read; semantically
    /// equivalent to `Move` after drop-elaboration in
    /// optimized_mir but distinguishable for backend lowering
    /// (e.g., debug-info annotation, copy-vs-memcpy dispatch).
    Copy(Place),

    /// Load a value from the place. Marks the place as
    /// participating in a move-style consumption. At rustc's
    /// `Call` argument site this enables in-place passing
    /// semantics; elsewhere in optimized_mir this is
    /// operationally equivalent to `Copy`.
    Move(Place),

    /// A constant value, referenced by its `ConstId` in the
    /// model's `ConstInterner` (§9.7). Drops rustc's
    /// `Box<ConstOperand>` wrapper -- `user_ty` is
    /// NLL/borrowck-only and dead in optimized_mir;
    /// operand-level `span` is dropped (the use-site span is
    /// naturally carried by the containing Statement's
    /// `SourceInfo` at stage 8 / Terminator-level for Call
    /// args at stage 9).
    Constant(ConstId),
}
```

### Public API -- LiteralValue (re-export pass-through)

Per `spec.md` §9.7 the literal-value vocabulary already lives
inside the `Const` enum (Int / Uint / Float / Bool / Char / ZST /
Bytes / Aggregate / EnumValue / NullPtr / Reloc plus the `Byte`
three-state enum). To give Operand consumers a focused import
path, re-export the existing `Const`-side types and helpers from
this module:

```rust
// In crates/regseq-model/src/operand.rs:

pub use crate::const_value::Const as LiteralValue;
// (Or whichever module name P1a-DS-05 / regseq-model-69 chose
//  for the Const enum's home.)
```

The `LiteralValue` alias is **purely a re-export** that aliases
the `Const` enum into the operand module's namespace; no new
type is introduced. If the `Const` enum's home module name
differs from the assumption above, adjust the re-export path
accordingly. Document the alias as a backward-compatibility /
discoverability convenience: consumers wiring up Operand-context
rendering reach the literal-value vocabulary through this alias
without knowing the Const enum's filesystem path.

If the project decides a re-export alias is undesirable,
substitute by referring to `Const` directly in the public
documentation and remove the `pub use` -- document the choice
in the worker's Codex review submission. The important contract
is: **no new literal-value enum is invented at this task**.

### Conventions

- **`Operand` is body-local content** per `spec.md:6960-6963`;
  no `OperandId`, no top-level interner. Derive `PartialEq, Eq,
  Hash` for structural equality.
- **No `Box<ConstOperand>` wrapper** per `spec.md:6953-6957`:
  `Constant(ConstId)` is flat; `ConstId` is 4 bytes
  (`NonMaxU32`) so the inline storage is the natural choice.
  Contrast with rustc's `Box<ConstOperand>` indirection (which
  exists because `ConstOperand` carries `Span` + `UserTy` +
  `Const` inline; RegseqModel's stripped wrapper plus
  interned-`Const` storage removes the size pressure).
- **Worst-case enum width** per `spec.md:6948-6951`: ~24 bytes
  including discriminant. `Place` ~20 bytes + 4-byte
  discriminator; `Constant(ConstId)` is well under that. Do
  not introduce explicit `repr(...)` annotations unless project
  convention requires them; let the compiler choose the niche
  layout.
- **`RuntimeChecks` rewrite is bridge-side** per
  `spec.md:7010-7078`: the bridge translates
  `mir::Operand::RuntimeChecks(checks)` into
  `Operand::Constant(intern_const(Const::Bool(b)))` before the
  value reaches RegseqModel. The IR has no `RuntimeChecks`
  variant.
- **Operand-site invariants** (I-Operand-Place-Validity,
  no-type-`Copy`-enforcement, no-type-coherence-on-Constant)
  per `spec.md:7099-7128` are documented as **forward-pointers
  to P1a-VAL-01** and the body-content stages (P1a-DS-12b /
  P1a-DS-13a). This task does not enforce them at the data
  shape.
- **Serde**: `Operand` carries `Place` (already serde-friendly
  via regseq-model-76) and `ConstId` (typed-ID); serde derives
  are admissible. Use `#[derive(Serialize, Deserialize)]` with
  the project's enum-tagging convention. The `LiteralValue`
  re-export alias inherits `Const`'s serde shape (typically
  serde-friendly for primitive variants and `#[serde(skip)]`
  for `Symbol`-bearing variants per the brief's cross-cutting
  note).
- **Derive `Clone, Debug, PartialEq, Eq, Hash`** on `Operand`.
- **Doc comments mandatory** on every public variant.

## Non-Goals

- **`RuntimeChecks` rewrite** at the bridge (`spec.md:7010-7078`)
  is P1a-BR-05.
- **Operand-site invariants enforcement** (I-Operand-Place-
  Validity inheritance, the no-type-`Copy` / no-type-coherence
  rules per `spec.md:7099-7128`) is body-walk territory:
  P1a-VAL-01 (Place validation propagation) + P1a-VAL-02 (ID
  referential integrity for embedded `ConstId`s).
- **Bridge translation from rustc `mir::Operand<'tcx>`** (the
  §20.3 per-variant table) is P1a-BR-05.
- **C lowering** for `Operand` (§20.4) is phase-1b.
- **Rendering** (`render_operand`) lives at
  `pretty-printer.md` §7.2 and is implemented in P1a-PP-03.
- **Inventing a new `LiteralValue` enum.** The locked spec
  keeps literal values inside `Const`; this task only
  re-exports for namespace convenience.
- **Drop of rustc's per-argument `Spanned<Operand>` at
  Call sites** (`spec.md:6993-7008`) is bridge-side stripping
  in P1a-BR-06's terminator translation.

## Test Plan

**Test organization**: `unit-in-src` -- tests live in
`crates/regseq-model/src/operand.rs` `#[cfg(test)] mod tests`.

Coverage requirements (every public variant exercised at least
once per `SESSION.md` Testing Requirements):

1. **`Operand::Copy(Place)` construction.** Build with a bare-
   local `Place::from(LocalId(0))`; assert pattern match
   succeeds and the embedded place reads back.
2. **`Operand::Move(Place)` construction.** Build with a
   non-trivial projection chain; assert pattern match succeeds.
3. **`Operand::Constant(ConstId)` construction.** Build with
   `ConstId(7)`; assert single-field tuple-variant access.
4. **All 3 variants pairwise distinct.** Build one of each;
   assert all 3 pairs are inequal -- the variant tag
   participates in equality.
5. **`Operand::Copy` vs `Operand::Move` distinct over same
   `Place`.** Build `Copy(p)` and `Move(p)` with identical
   `Place`; assert `==` returns false (the read-flavor
   discriminator matters per `spec.md:6911-6924`).
6. **`Operand::Copy` equality -- identical Place.** Two
   `Copy(p)` values with identical `Place` are `==`.
7. **`Operand::Copy` inequality -- differing Place.** Two
   `Copy(p)` values with differing `Place` are inequal.
8. **`Operand::Constant` equality -- identical ConstId.** Two
   `Constant(ConstId(N))` with the same `N` are `==`.
9. **`Operand::Constant` inequality -- differing ConstId.**
   Differ on `N`; assert inequality.
10. **`Operand` Hash discrimination.** Insert one of each of
    the 3 variants into a `HashSet`; assert size 3; insert a
    clone of each; assert size still 3.
11. **`Operand` clone equality.** For each variant, clone +
    assert `==`.
12. **Serde round-trip -- `Operand::Copy`.** Round-trip via
    `serde_json::to_string` / `from_str`; assert `==`.
13. **Serde round-trip -- `Operand::Move`.**
14. **Serde round-trip -- `Operand::Constant`.**
15. **Serde round-trip -- `Operand::Copy` with non-trivial
    projection.** Verify `Place` interior round-trips through
    the wrapper.
16. **`LiteralValue` re-export reachable.** Confirm `use
    regseq_model::operand::LiteralValue;` (or the project's
    chosen path) compiles and resolves to the underlying
    `Const` enum from regseq-model-69. (If the worker chooses
    not to provide the re-export per the Implementation Notes
    convention discussion, omit this test and document the
    choice in the Codex review submission.)
17. **`Operand` width sanity.** Use a `const _: () =
    assert!(size_of::<Operand>() <= ...);` per project
    convention if the project pins explicit size assertions
    on body-local enums; otherwise document the worst-case
    width per `spec.md:6948-6951` in the type-level doc
    comment as guidance.

`cargo test -p regseq-model` passes.

## Review

After tests pass, request a Codex review using the four-block
template:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-model-77 against its task spec at
.harness/tasks/regseq-model/77-operand-and-literal-value.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-model/src/operand.rs
- crates/regseq-model/src/lib.rs

Focus on:
- Spec deviations (every rule in the Implementation Notes section).
- Correctness issues and generality loss.
- Design misalignment with the spec's public API / architecture.
- Test coverage vs. the spec's Test Plan.
- Whether `Operand` is structural and free of any Box wrapper
  per spec.md:6953-6957.
- Whether `LiteralValue` is correctly framed as a re-export
  (not an invented type).
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

- New module `crates/regseq-model/src/operand.rs` created and
  re-exported from `crates/regseq-model/src/lib.rs`.
- `Operand` closed 3-variant enum (`Copy(Place)`,
  `Move(Place)`, `Constant(ConstId)`) defined per
  `spec.md:6910-6940`.
- `LiteralValue` re-export alias provided (or the alternative
  documented per the Implementation Notes convention
  discussion).
- All public variants carry doc comments matching the spec's
  prose.
- Test Plan items 1-17 implemented; all pass under
  `cargo test -p regseq-model`.
- `cargo fmt --all -- --check` and
  `cargo clippy -p regseq-model -- -D warnings` pass.
- Codex Task Review returns VERDICT: APPROVED.
