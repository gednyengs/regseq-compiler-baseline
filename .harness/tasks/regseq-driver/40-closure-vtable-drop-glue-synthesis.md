# regseq-driver-40: Closure + VTable + Drop-Glue Item Synthesis

## Description

Synthesise the six families of bridge-only Items mandated by `spec.md`
§13 plus the Phase 2 trigger table at §3.4: ClosureCaptures Item +
ClosureCall Function Item + ClosureDropGlue Function Item + VTableType
Item + VTableInstance Item + VTableShim Function Item, plus the
drop-glue Function Items required for owned non-Copy Place dropping.

Synthesis runs **during** Phase 2 translation per the §3.4 D-12-5.2
synthesis-interleaving rule. Each family writes a Phase-2 ItemHandle
into a per-family working cache; Phase 5 re-keys handles to the
final ItemId values produced by regseq-driver-36's Phase 4 assignment
and populates `RegseqModel.vtables: HashMap<PrincipalTraitKey, ItemId>`.

## Input Context

- `docs/discussions/regseq-model/spec.md` §13 (Synthesised Items) --
  closure family at spec.md:4503+, vtable family per §13.2-§13.4
- `docs/discussions/regseq-model/spec.md` §3.4 (Phase 2 trigger table
  + D-12-5.2 synthesis-interleaving rule + D-12-5.3 handle-to-ItemId
  remap)
- `crates/regseq-model/src/synthesised/` (SyntheticItemKind +
  PrincipalTraitKey + the synthetic Item shapes from regseq-model-72,
  regseq-model-73)
- `crates/regseq-driver/src/bridge/mod.rs` (BridgeCtxt + HandleTable
  from regseq-driver-36)
- `crates/regseq-driver/src/bridge/ty.rs` (translate_ty from
  regseq-driver-37)

## Implementation Notes

Create `crates/regseq-driver/src/bridge/synth/` subtree. Wire
`pub mod synth;` from `crates/regseq-driver/src/bridge/mod.rs`.

### Module structure

- `crates/regseq-driver/src/bridge/synth/mod.rs` -- entry points + the
  per-family working caches on `BridgeCtxt`.
- `crates/regseq-driver/src/bridge/synth/closure.rs` -- ClosureCaptures
  + ClosureCall + ClosureDropGlue synthesis per §13.1.
- `crates/regseq-driver/src/bridge/synth/vtable.rs` -- VTableType +
  VTableInstance + VTableShim synthesis per §13.2 plus the
  `RegseqModel.vtables` re-key in Phase 5.
- `crates/regseq-driver/src/bridge/synth/drop_glue.rs` -- drop-glue
  Function Item synthesis (consumed by regseq-driver-42's
  Drop-as-Call collapse).

### Public API

```rust
// bridge/synth/mod.rs

/// Synthesise the closure family for a closure mono-item. Records
/// three handles (ClosureCaptures + ClosureCall + ClosureDropGlue) in
/// the closure working cache. Per spec.md §13.1.
pub fn synthesise_closure<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    closure_def: rustc_middle::ty::ClosureArgs<'tcx>,
) -> Result<ClosureSynthHandles, BridgeError>;

/// Synthesise the vtable family for a (Trait, ImplTy) pair. Records
/// three handles (VTableType + VTableInstance + VTableShim) in the
/// vtable working cache and dedups on PrincipalTraitKey per
/// spec.md §13.3. Per spec.md §13.2.
pub fn synthesise_vtable<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    principal: regseq_model::PrincipalTraitKey,
    impl_ty: regseq_model::TyId,
) -> Result<VTableSynthHandles, BridgeError>;

/// Synthesise a drop-glue Function Item for a non-Copy Ty. Records a
/// handle in the drop-glue working cache. Consumed by
/// regseq-driver-42's Drop-as-Call collapse.
pub fn synthesise_drop_glue<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    drop_target_ty: regseq_model::TyId,
) -> Result<ItemHandle, BridgeError>;

/// Phase-5 re-key pass. Walks every per-family working cache, resolves
/// ItemHandle -> ItemId via the HandleTable, enters each synthesised
/// Item into RegseqModel.items, populates
/// RegseqModel.item_synthetic_kinds[item], and populates
/// RegseqModel.vtables[principal] = vtable_instance_item_id.
pub fn rekey_synthesis_in_phase5(
    cx: &mut BridgeCtxt<'_>,
) -> Result<(), BridgeError>;
```

### Synthesis ordering (spec.md §3.4 D-12-5.2)

Per spec.md §3.4 D-12-5.2 synthesis fires **during** Phase-2
translation (it is interleaved, not deferred to a later phase). Each
synthesis function runs eagerly when the Phase-2 translation visits a
trigger site:

- ClosureCaptures synthesis triggers when Phase 2 visits a closure
  expression.
- VTable synthesis triggers when Phase 2 visits a `dyn Trait` cast or
  a `dyn Trait` method call.
- Drop-glue synthesis triggers when Phase 2 visits a non-Copy Place
  ownership transfer (Move semantics) or a Drop terminator -- the
  drop-glue Item is synthesised here so that regseq-driver-42 can
  collapse the Drop terminator to a Call against it.

### Phase-2 working caches per family

`BridgeCtxt` extends with:

- `closure_synth_cache: Vec<ClosureSynthHandles>`
- `vtable_synth_cache: HashMap<PrincipalTraitKey, VTableSynthHandles>`
- `drop_glue_synth_cache: HashMap<TyId, ItemHandle>`

The `vtable_synth_cache` is keyed by PrincipalTraitKey per spec.md
§13.3 to deduplicate synthesis when multiple `dyn Trait` sites share
the same principal+impl pair.

### Phase-5 re-key + RegseqModel.vtables population

Per spec.md §13.3 the Phase-5 re-key pass:

1. Iterates the closure / vtable / drop-glue working caches.
2. For each handle, looks up the final ItemId via
   `BridgeCtxt.handles.handle_to_item_id`.
3. Inserts the synthesised Item into `RegseqModel.items[item_id]`.
4. Records `RegseqModel.item_synthetic_kinds[item_id]` per the
   SyntheticItemKind variant for that family.
5. For VTableInstance Items, inserts
   `RegseqModel.vtables.insert(principal_key, vtable_instance_item_id)`.

## Non-Goals

- Per-construct Place / Operand / Rvalue / Statement translation
  (regseq-driver-41).
- Terminator translation + Drop-as-Call collapse (regseq-driver-42 --
  consumes the drop-glue Item synthesised here).
- Body materialisation + vtable-indirect rewrite (regseq-driver-43 --
  consumes the vtable Items synthesised here).
- ConstMemoryStatic synthesis (regseq-driver-38).
- Synthetic Item data shapes (owned by regseq-model-72 and
  regseq-model-73).

## Test Plan

### Unit tests (`crates/regseq-driver/src/bridge/synth/*` `#[cfg(test)] mod tests`)

- `closure.rs`: synthesise a closure for a stub closure_def; assert
  3 handles are recorded in the closure cache and the per-handle
  Item shapes match spec.md §13.1.
- `vtable.rs`: synthesise the same PrincipalTraitKey twice; assert
  the cache dedup hits and only 3 handles total are recorded.
- `drop_glue.rs`: synthesise drop-glue for the same TyId twice; assert
  the cache dedup hits and only 1 handle is recorded.
- `mod.rs`: `rekey_synthesis_in_phase5` populates `model.items`,
  `model.item_synthetic_kinds`, and `model.vtables` deterministically
  across 5 repeated runs of the same input cache.

### Integration tests (`crates/regseq-driver/tests/bridge_synth.rs`)

Bridge tests are integration-in-tests because they need rustc-compiled
fixtures.

Positive coverage (one fixture per synthesis family):

- Closure fixture -- `pub fn f() -> impl Fn(u32) -> u32 { |x| x + 1 }`
  -- assert ClosureCaptures + ClosureCall + ClosureDropGlue Items
  exist in the post-bridge RegseqModel and the closure cache contains
  exactly one entry.
- VTable fixture -- `pub trait T { fn m(&self); }` plus
  `impl T for u32 { fn m(&self) {} }` plus `pub fn f(t: &dyn T) { t.m(); }`
  -- assert VTableType + VTableInstance + VTableShim Items exist for
  the (T, u32) PrincipalTraitKey and `RegseqModel.vtables` contains
  the principal -> vtable_instance_item_id mapping.
- Drop-glue fixture -- a fixture that creates and drops a non-Copy
  ADT (e.g. `struct S(u32); impl Drop for S { fn drop(&mut self) {} }`
  + `pub fn f() { let _ = S(0); }`) -- assert a drop-glue Function
  Item is synthesised for the S Ty.

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-driver-40 against its task spec at
.harness/tasks/regseq-driver/40-closure-vtable-drop-glue-synthesis.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-driver/src/bridge/mod.rs
- crates/regseq-driver/src/bridge/synth/mod.rs
- crates/regseq-driver/src/bridge/synth/closure.rs
- crates/regseq-driver/src/bridge/synth/vtable.rs
- crates/regseq-driver/src/bridge/synth/drop_glue.rs
- crates/regseq-driver/tests/bridge_synth.rs
- (any new fixture files under crates/regseq-driver/tests/fixtures/)

Focus on:
- Spec deviations vs spec.md §13.1 / §13.2 / §13.3 / §13.4 / §3.4.
- Synthesis-interleaving rule (D-12-5.2): synthesis fires during
  Phase 2, not deferred.
- Phase-5 re-key: handle -> ItemId; model.items / item_synthetic_kinds /
  vtables populated deterministically.
- VTable cache deduplication on PrincipalTraitKey.
- Drop-glue cache deduplication on TyId.
Do NOT flag anything explicitly marked out of scope (Non-Goals,
later task IDs). Drop-as-Call collapse and vtable-indirect rewrite
are owned by regseq-driver-42 / regseq-driver-43 and consume the
synthesised Items produced here.
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

- `crates/regseq-driver/src/bridge/synth/{mod,closure,vtable,drop_glue}.rs`
  created and wired from `bridge/mod.rs`.
- Per-family Phase-2 working caches added to BridgeCtxt.
- `rekey_synthesis_in_phase5` populates `model.items`,
  `model.item_synthetic_kinds`, and `model.vtables` per spec.md §13.3.
- VTable cache dedup hits via PrincipalTraitKey; drop-glue cache
  dedup hits via TyId.
- All unit and integration tests pass; Codex Task Review verdict
  APPROVED.
