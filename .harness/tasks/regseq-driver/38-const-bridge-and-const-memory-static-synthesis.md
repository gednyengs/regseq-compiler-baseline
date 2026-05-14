# regseq-driver-38: Const Bridge and ConstMemoryStatic Synthesis

## Description

Translate rustc constants into the `regseq-model` `Const` enum per
`spec.md` §9.7, including reference-typed const handling that
synthesises `ConstMemoryStatic` Items during bridge Phase 2 / Phase 4 /
Phase 5 per the §3.4 trigger table. Closure / vtable / drop-glue Item
synthesis lives in regseq-driver-40; this task owns only the
ConstMemoryStatic synthesis trigger and the per-variant const dispatch.

## Input Context

- `docs/discussions/regseq-model/spec.md` §9.7 (Const vocabulary
  bridge dispatch table)
- `docs/discussions/regseq-model/spec.md` §3.4 (Phase 2 trigger table
  for synthesised Items)
- `docs/discussions/regseq-model/spec.md` §3 (Top-level RegseqModel
  pipeline ordering)
- `crates/regseq-model/src/const_/` (the 11-variant `Const` enum +
  Byte three-state + InstanceRef + ConstInterner from regseq-model-69)
- `crates/regseq-model/src/synthesised/` (SyntheticItemKind +
  ConstMemoryStatic Item shape from regseq-model-73)
- `crates/regseq-driver/src/bridge/mod.rs` (BridgeCtxt + BridgeError
  from regseq-driver-36)
- `crates/regseq-driver/src/bridge/ty.rs` (translate_ty from
  regseq-driver-37)

## Implementation Notes

Create `crates/regseq-driver/src/bridge/const_.rs`. Wire `pub mod const_;`
from `crates/regseq-driver/src/bridge/mod.rs`. Extend `BridgeCtxt` with
a Phase-2 working cache for ConstMemoryStatic synthesis.

### Public API

```rust
// bridge/const_.rs

/// Translate a rustc `mir::Const` into a `regseq_model::ConstId`. Per
/// spec.md §9.7 dispatch table. Reference-typed constants trigger
/// ConstMemoryStatic Item synthesis via `synthesise_const_memory_static`.
pub fn translate_const<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    konst: rustc_middle::mir::Const<'tcx>,
) -> Result<regseq_model::ConstId, BridgeError>;

/// Synthesise a ConstMemoryStatic Item for a reference-typed const.
/// Records a Phase-2 ItemHandle in the working cache; the handle is
/// remapped to the final ItemId during Phase 4 (regseq-driver-36's
/// HandleTable) and the synthetic Item enters
/// `RegseqModel.items` during Phase 5.
pub fn synthesise_const_memory_static<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    pointee_const: regseq_model::ConstId,
) -> Result<ItemHandle, BridgeError>;
```

### Per-variant const dispatch (spec.md §9.7)

The per-variant translation runs as a `match` over the rustc const
representation. Per spec.md §9.7 each rustc const variant maps to one
of the 11 closed `Const` variants:

- Scalar -- Int / Uint / Float / Bool / Char / NullPtr per the §9.7
  table at spec.md:3328+.
- ZST -- the unit / `()` carrier.
- Aggregate -- recursive translation over fields; reference-typed
  constants take the §3.4 trigger path (see below).
- EnumValue -- per the §9.7 dispatch row.
- Bytes -- byte-aggregate constants per §9.7.
- Reloc -- references-to-statics carrier per spec.md:3328
  (`Aggregate([Reloc, Uint])`).

### Reference-typed const handling (spec.md §3.4 Phase 2 trigger)

Per spec.md §3.4 a reference-typed const triggers Phase-2
ConstMemoryStatic synthesis: the bridge first translates the pointee
const, then calls `synthesise_const_memory_static`, then emits the
parent `Const::Aggregate([Reloc(handle), Uint(offset)])` carrier. The
Phase-2 ItemHandle is remapped to the final ItemId during Phase 4 per
regseq-driver-36's `HandleTable`.

### Module structure

- `crates/regseq-driver/src/bridge/const_.rs` -- `translate_const` +
  `synthesise_const_memory_static` + per-variant helpers.
- `crates/regseq-driver/src/bridge/mod.rs::BridgeCtxt` -- extended
  with `const_memory_static_cache: Vec<ConstMemoryStaticHandle>`
  populated during Phase 2 and drained during Phase 5 per §3.4.

## Non-Goals

- FnSig translation (regseq-driver-39).
- Closure / vtable / drop-glue synthesis (regseq-driver-40).
- Place / Operand / Rvalue / Statement translation (regseq-driver-41).
- Terminator translation (regseq-driver-42).
- Body materialisation (regseq-driver-43).
- ConstMemoryStatic data shape (owned by regseq-model-73).

## Test Plan

### Unit tests (`crates/regseq-driver/src/bridge/const_.rs` `#[cfg(test)] mod tests`)

- For each scalar variant (Int / Uint / Float / Bool / Char / NullPtr)
  per spec.md §9.7, hand-construct a rustc const value and assert the
  translation result.
- ZST translation: assert the unit const maps to the spec-pinned ZST
  variant.
- `synthesise_const_memory_static` records a handle into the
  Phase-2 cache; assert the cache size grows by 1 per call.

### Integration tests (`crates/regseq-driver/tests/bridge_const.rs`)

Bridge tests are integration-in-tests because they need rustc-compiled
fixtures.

Positive coverage (one fixture per non-rejection §9.7 row):

- Scalar Int -- `pub const N: i32 = 42;` + use site -- assert
  `Const::Int`.
- Scalar Bool -- `pub const B: bool = true;` -- assert `Const::Bool`.
- Char -- `pub const C: char = 'a';` -- assert `Const::Char`.
- ZST -- `pub const Z: () = ();` -- assert ZST variant.
- Aggregate -- `pub const T: (u32, u8) = (1, 2);` -- assert
  `Const::Aggregate` with the recursive field translation.
- EnumValue -- `pub enum E { A, B(u32) } pub const X: E = E::A;` --
  assert `Const::EnumValue`.
- Bytes -- `pub const BS: [u8; 3] = [1, 2, 3];` -- assert
  `Const::Bytes` (or the §9.7-pinned variant for byte aggregates).
- Reference-typed -- `pub const R: &u32 = &42;` -- assert (a) a
  ConstMemoryStatic Item is synthesised in
  `RegseqModel.items` after Phase 5, (b) the parent const renders as
  `Const::Aggregate([Reloc(ItemId(_)), Uint(0)])` per spec.md:3328,
  (c) `RegseqModel.item_synthetic_kinds` records
  `SyntheticItemKind::ConstMemoryStatic` for the synthesised item id.

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-driver-38 against its task spec at
.harness/tasks/regseq-driver/38-const-bridge-and-const-memory-static-synthesis.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-driver/src/bridge/mod.rs
- crates/regseq-driver/src/bridge/const_.rs
- crates/regseq-driver/tests/bridge_const.rs
- (any new fixture files under crates/regseq-driver/tests/fixtures/)

Focus on:
- Spec deviations vs spec.md §9.7 / §3.4 Phase-2 trigger table.
- Per-Const-variant row coverage in tests.
- Reference-typed const synthesis ordering (Phase 2 cache + Phase 4
  ItemId remap + Phase 5 enter into model.items).
- ConstMemoryStatic side-table population
  (item_synthetic_kinds[item] = ConstMemoryStatic).
Do NOT flag anything explicitly marked out of scope (Non-Goals,
later task IDs). Closure / vtable / drop-glue synthesis is owned by
regseq-driver-40.
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

- `crates/regseq-driver/src/bridge/const_.rs` created with
  `translate_const` and `synthesise_const_memory_static`.
- BridgeCtxt extended with the Phase-2 ConstMemoryStatic working cache.
- Each spec.md §9.7 dispatch row covered by a positive test.
- Reference-typed const integration test asserts the full Phase 2 ->
  Phase 4 -> Phase 5 synthesis cycle and the
  `item_synthetic_kinds[item] = ConstMemoryStatic` side-table entry.
- All tests pass; Codex Task Review verdict APPROVED.
