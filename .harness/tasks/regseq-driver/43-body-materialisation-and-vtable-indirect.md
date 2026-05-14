# regseq-driver-43: Body Materialisation + Vtable-Indirect Rewrite + Joint Validation

## Description

Materialise function bodies into `RegseqModel.instances[InstanceKind::Function].body`
during Phase 5 per `spec.md` §25.7. Wire the LocalDecls allocation API
per spec.md §25.8. Run the vtable-indirect rewrite per spec.md §25.7.2
**after** the Drop-as-Call collapse landed by regseq-driver-42 and
**before** the joint validation walk per spec.md §25.7 ordering rule.
Wire the joint validation walk per spec.md §25.6 to invoke the Place
walker landed by regseq-model-83 (P1a-VAL-01) over each materialised
body.

This task closes the body-content side of the bridge: Phase 5 takes
the per-Instance handle, materialises its Body, runs the vtable-indirect
rewrite, then hands the body to the joint validation walk.

## Input Context

- `docs/discussions/regseq-model/spec.md` §25 (Body-level vocabulary)
  -- especially §25.6 joint validation walk hooks, §25.7 body-level
  transformation passes (ordering rule), §25.7.2 vtable-indirect
  rewrite, §25.8 LocalDecls allocation API
- `docs/discussions/regseq-model/spec.md` §3.4 (Phase 5 ordering)
- `crates/regseq-model/src/body.rs` (Body + LocalDecl + LocalId +
  push_synthetic_local + LocalDecls allocation API + joint validation
  walk hooks from regseq-model-82)
- `crates/regseq-model/src/validators/place.rs` (Place walker from
  regseq-model-83 / P1a-VAL-01)
- `crates/regseq-driver/src/bridge/body/` (per-construct translators
  from regseq-driver-41 + regseq-driver-42)
- `crates/regseq-driver/src/bridge/synth/vtable.rs` (VTableType +
  VTableInstance + VTableShim Items + RegseqModel.vtables population
  from regseq-driver-40)

## Implementation Notes

Create `crates/regseq-driver/src/bridge/body/materialise.rs` and
`crates/regseq-driver/src/bridge/body/vtable_indirect.rs`. Wire both
from `crates/regseq-driver/src/bridge/body/mod.rs`. Update the Phase 5
hook in `bridge/mod.rs` to drive the new pass set.

### Public API

```rust
// bridge/body/materialise.rs

/// Materialise the body of one Function Instance during Phase 5 per
/// spec.md §25.7. Walks rustc basic blocks, populating LocalDecls via
/// the §25.8 allocation API and translating each block via
/// regseq-driver-42's `translate_basic_block`.
pub fn materialise_function_body<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    instance_handle: InstanceHandle,
) -> Result<regseq_model::Body, BridgeError>;

/// Phase-5 driver: materialise every Function body, run
/// vtable-indirect rewrite, run joint validation. Per spec.md §25.7
/// pass ordering.
pub fn run_phase5_body_passes(
    cx: &mut BridgeCtxt<'_>,
) -> Result<(), BridgeError>;
```

```rust
// bridge/body/vtable_indirect.rs

/// Rewrite Direct Calls against `dyn Trait` shim placeholders into
/// Indirect Calls that read the function pointer from the VTableInstance
/// per spec.md §25.7.2. Runs after Drop-as-Call collapse and before
/// joint validation.
pub fn rewrite_vtable_indirect(
    cx: &mut BridgeCtxt<'_>,
    body: &mut regseq_model::Body,
) -> Result<(), BridgeError>;
```

### Phase-5 pass ordering (spec.md §25.7)

Per spec.md §25.7 the Phase-5 body passes run in exactly this order
**per body**:

1. `materialise_function_body` -- walk rustc blocks, allocate LocalDecls
   via `Body::push_synthetic_local` (regseq-model-82) when synthesis
   needs fresh locals, translate each block via regseq-driver-42's
   `translate_basic_block` (which itself runs the per-statement and
   per-terminator translators including the Drop-as-Call collapse).
2. `rewrite_vtable_indirect` -- per spec.md §25.7.2, walk the body's
   Calls; for each Call whose callee resolves to a VTableShim Item
   (regseq-driver-40), rewrite the Call to `CallCallee::Indirect`
   that reads the function pointer from the VTableInstance Item. Per
   spec.md §25.7.2 this introduces 4-5 fresh locals via
   `Body::push_synthetic_local`.
3. `joint_validation_walk` -- invoke regseq-model-83's Place walker
   over the body's content per spec.md §25.6 hooks.

### Vtable-indirect rewrite (spec.md §25.7.2)

Per spec.md §25.7.2 the rewrite consumes the VTableType +
VTableInstance + VTableShim Items synthesised by regseq-driver-40 and
the `RegseqModel.vtables: HashMap<PrincipalTraitKey, ItemId>` map
populated in regseq-driver-40's Phase-5 re-key. For each
`CallCallee::Direct(item_id)` where `item_id` resolves through
`item_synthetic_kinds` to `SyntheticItemKind::VTableShim`, the rewrite:

1. Allocates 4-5 fresh locals via `Body::push_synthetic_local` per
   spec.md §25.7.2.
2. Synthesises a Place chain that reads the function pointer from the
   VTableInstance.
3. Replaces the Call's callee with `CallCallee::Indirect(place)`.

The §25.7 ordering rule requires this pass to run AFTER the
Drop-as-Call collapse (regseq-driver-42) and BEFORE the joint
validation walk.

### Joint validation walk wiring (spec.md §25.6)

Per spec.md §25.6 the joint validation walk runs after every body
pass; it invokes regseq-model-83's Place walker (the
`P1a-VAL-01` validator) over each Statement and Terminator in the
materialised Body. The walker enforces the §19.4 type-narrowing rules
and the I-Deref-first / I-Downcast-followed-by-Field structural
invariants.

### Module structure

- `crates/regseq-driver/src/bridge/body/materialise.rs` --
  `materialise_function_body` + `run_phase5_body_passes`.
- `crates/regseq-driver/src/bridge/body/vtable_indirect.rs` --
  `rewrite_vtable_indirect`.
- `crates/regseq-driver/src/bridge/mod.rs` -- Phase 5 hook updated to
  call `run_phase5_body_passes`.

## Non-Goals

- Closure / vtable / drop-glue Item synthesis (regseq-driver-40).
- Per-construct Statement / Terminator translation (regseq-driver-41,
  regseq-driver-42).
- Drop-as-Call collapse (regseq-driver-42; runs inside terminator
  translation).
- Diagnostic-logging recognition (regseq-driver-44).
- DiagLogPeel + format-template parsing (regseq-driver-45).
- Place walker implementation (regseq-model-83).

## Test Plan

### Unit tests (`crates/regseq-driver/src/bridge/body/{materialise,vtable_indirect}.rs` `#[cfg(test)] mod tests`)

- `materialise.rs`: hand-construct a stub rustc Body with N basic
  blocks; assert `materialise_function_body` produces a regseq_model
  Body with N translated basic blocks.
- `materialise.rs`: assert LocalDecls are populated in declaration
  order matching the rustc body.
- `vtable_indirect.rs`: hand-construct a Body with one Direct Call
  against a VTableShim Item id; run `rewrite_vtable_indirect`; assert
  the Call's callee is now Indirect, and the body's local count grew
  by 4 or 5 per spec.md §25.7.2.
- `vtable_indirect.rs`: hand-construct a Body with no VTableShim Calls;
  run the rewrite; assert the Body is unchanged.
- Pass ordering: assert that `run_phase5_body_passes` invokes the
  three passes in spec.md §25.7 order (materialise -> vtable-indirect
  -> joint validation).

### Integration tests (`crates/regseq-driver/tests/bridge_body_passes.rs`)

Bridge tests are integration-in-tests because they need rustc-compiled
fixtures.

Positive coverage:

- Trait-object dispatch fixture --
  `pub trait Reader { fn read(&self) -> u8; } impl Reader for u32 { fn read(&self) -> u8 { 0 } } pub fn f(r: &dyn Reader) -> u8 { r.read() }`
  -- assert the post-bridge Body contains a Call whose callee is
  `CallCallee::Indirect(_)` (not Direct against the trait shim).
- Drop-glue + vtable-indirect interaction -- a fixture exercising both
  a non-Copy drop and a `dyn Trait` call site -- assert (a) Drop-as-Call
  collapse fired before vtable-indirect rewrite (the drop-Call's callee
  is unaffected by the rewrite; the trait-object Call's callee is
  Indirect); (b) the final Body passes joint validation.
- No-op fixture (`pub fn no_op() {}`) -- assert the body materialises
  cleanly with one BasicBlock containing a Return Terminator and zero
  Statements.
- Joint validation rejection -- a hand-constructed bad Place
  (e.g. Field on a non-Struct local) injected into a fixture's Body
  before validation -- assert the joint validation walk surfaces the
  error path.

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-driver-43 against its task spec at
.harness/tasks/regseq-driver/43-body-materialisation-and-vtable-indirect.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-driver/src/bridge/mod.rs
- crates/regseq-driver/src/bridge/body/mod.rs
- crates/regseq-driver/src/bridge/body/materialise.rs
- crates/regseq-driver/src/bridge/body/vtable_indirect.rs
- crates/regseq-driver/tests/bridge_body_passes.rs
- (any new fixture files under crates/regseq-driver/tests/fixtures/)

Focus on:
- Spec deviations vs spec.md §25.6 / §25.7 / §25.7.2 / §25.8.
- Phase-5 pass ordering (materialise -> vtable-indirect -> joint
  validation), per spec.md §25.7.
- Vtable-indirect rewrite consumes regseq-driver-40's VTableShim /
  VTableInstance Items + RegseqModel.vtables map.
- LocalDecls allocation via Body::push_synthetic_local.
- Joint validation walk invokes regseq-model-83's Place walker.
Do NOT flag anything explicitly marked out of scope (Non-Goals,
later task IDs). Diagnostic-logging recognition is regseq-driver-44.
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

- `crates/regseq-driver/src/bridge/body/{materialise,vtable_indirect}.rs`
  created and wired from `bridge/body/mod.rs`.
- Phase 5 hook in `bridge/mod.rs` calls `run_phase5_body_passes`.
- Pass ordering matches spec.md §25.7 (materialise -> vtable-indirect
  -> joint validation).
- Vtable-indirect rewrite consumes regseq-driver-40's VTableShim
  Items and `RegseqModel.vtables`.
- Joint validation walk invokes regseq-model-83's Place walker per
  spec.md §25.6.
- All tests pass; Codex Task Review verdict APPROVED.
