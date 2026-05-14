# regseq-model-72: Closure-family synthesised Items (P1a-DS-08a)

## Description

Implement the closure-family synthesised-Item data shapes from
`spec.md` §13.1: the `ClosureCaptures` Item discriminator, the
`ClosureCall` Function-Item shape, the `ClosureDropGlue`
Function-Item shape, and the `ClosureCallKind` enum that
parameterises `ClosureCall`. These three synthetic Items plus the
`ClosureCallKind` enum are the producer-side IR data the bridge
populates when it encounters a closure expression in MIR.

This task lands the closure-family **discriminator variants** of
`SyntheticItemKind` (the full enum lands in regseq-model-73 / P1a-DS-08b)
along with the `ClosureCallKind` enum. The closure-as-struct
identity-vs-layout split per stage-3 D6.3 is honored: the parallel
`Ty::Struct(ClosureCaptures, args, layout)` is a bridge-time
computation (P1a-BR-04 territory), but the data-shape side defines
the `SyntheticItemKind::ClosureCaptures` variant alongside its
companion `ClosureCall` / `ClosureDropGlue` variants here so the
synthesised-Item discriminator surface for the closure family is
type-complete.

## Input Context

- `docs/discussions/regseq-model/spec.md` §13.1 (Closure family;
  `spec.md:4509-4587`)
- `docs/discussions/regseq-model/spec.md` §13.3 (`SyntheticItemKind`
  closure-family variants; `spec.md:4861-4905`)
- `docs/discussions/regseq-model/spec.md` §9.4 (`InstanceKind::Function`
  per Option B; defines the per-call-method Function Item carrier
  shape)
- `docs/discussions/regseq-model/spec.md` §8 (Item / ItemKind --
  context for which `ItemKind` variants the synthesised closure
  Items map to)
- `crates/regseq-model/src/lib.rs` (existing Stage-1-grandfathered
  foundation -- the surfaces this task extends)
- `crates/regseq-model/src/placeholders.rs` (existing `Item`
  placeholder; this task does not replace it but adds the closure-
  family discriminator shapes the synthesised-Items machinery
  consumes)
- `crates/regseq-model/src/ids.rs` (`ItemId`, `DefPathHash` types
  the synthesised-Item discriminators reference)
- `.harness/spec-batches/regseqmodel-phase-1a.md` (parent brief;
  P1a-DS-08a row)

## Implementation Notes

Add closure-family synthetic-Item discriminator variants under
`crates/regseq-model/src/synthetic.rs` (new module; re-export from
`lib.rs`). The full `SyntheticItemKind` enum housing all six
synthetic kinds lands in regseq-model-73; this task contributes
the three closure-family variants and the `ClosureCallKind`
sub-enum.

### Public API

The closure-family discriminator variants land as part of the
`SyntheticItemKind` enum (`spec.md:4861-4905`):

```rust
// Closure family (§13.1, §13.4). Each variant carries the
// closure expression's rustc DefPathHash inline so the
// synthetic identity payload is self-contained -- consumers
// and validators do not have to parse `DefPath.segments`
// hex to recover identity (matching the vtable family's
// self-contained PrincipalTraitKey pattern).
ClosureCaptures  { closure: DefPathHash },                              // source-derived; see §13.1 carve-out
ClosureCall      { closure: DefPathHash, call_kind: ClosureCallKind },  // synthetic
ClosureDropGlue  { closure: DefPathHash },                              // synthetic
```

The accompanying closed sub-enum (`spec.md:4905`):

```rust
pub enum ClosureCallKind { Call, CallMut, CallOnce }
```

### Per-Item carriage rules (from spec.md §13.1)

1. **`ClosureCaptures` Item** lifts as `ItemKind::Struct` (per
   `spec.md:4513-4529`). The Item is **source-derived**: its
   `def_path` is rustc's closure DefPath verbatim and
   `item_def_paths[id]` is `tcx.def_path_hash(closure_did)`.
   `SyntheticItemKind::ClosureCaptures` is a **discriminator
   only** -- the underlying identity is rustc's, not synthetic
   (this is the only `SyntheticItemKind` variant whose Item
   carries a rustc-derived `DefPathHash`; invariant 2 in §3.1
   carves out this exception).

2. **`ClosureCall` Function Items**, 1-3 per closure, lift as
   `ItemKind::Function` (per `spec.md:4531-4566`). One per
   `ClosureCallKind` arm reachable for that closure's
   `ClosureKind`:
   - `FnOnce`: `call_once` only.
   - `FnMut`: `call_mut` plus inherited `call_once`.
   - `Fn`: `call` plus inherited `call_mut` and `call_once`.
   The corresponding Function `Instance`'s
   `InstanceKind::Function { fn_sig, body, flavor, virtual_idx }`
   (per Option B / §9.4) carries the substituted signature
   derived from the closure's `closure_sig_as_fn_ptr_ty`.

3. **`ClosureDropGlue` Function Item** is synthesised iff the
   captures need dropping (per `spec.md:4554`). Lifts as
   `ItemKind::Function`.

### Conventions

- `SyntheticItemKind::ClosureCaptures` is the **single
  source-derived** synthetic-kind discriminator; the underlying
  Item carries rustc's `DefPathHash` (non-synthetic
  `stable_crate_id`). Validators must respect the §3.1 invariant
  2 carve-out (`spec.md:5200-5203`).
- `SyntheticItemKind::ClosureCall.closure` and
  `SyntheticItemKind::ClosureDropGlue.closure` payload fields
  carry the closure expression's rustc `DefPathHash` inline so
  the synthetic identity payload is self-contained per
  `spec.md:4866-4871`.
- `ClosureCallKind` is closed and stable: variant order
  `Call` / `CallMut` / `CallOnce` matches the synthetic
  `DefPathHash` discriminant table at `spec.md:5157`
  (`Call=0, CallMut=1, CallOnce=2`).
- **Serde**: per the brief's cross-cutting note, types
  containing `Symbol`/`StringId` are deferred from serde until
  the resolution-time pass. `DefPathHash` is `(StableCrateId,
  u64)`-shaped per §7.3 and contains no `StringId`; serde derives
  on `SyntheticItemKind` closure-family variants and on
  `ClosureCallKind` are admissible. Use `#[derive(Serialize,
  Deserialize)]` on both.
- Derive `Clone, Copy, Debug, PartialEq, Eq, Hash` on
  `ClosureCallKind`. Derive `Clone, Debug, PartialEq, Eq, Hash`
  on the closure-family discriminator carriers.
- The companion **call-method ABI rewrite** (`FnAbi::Rust` per
  `spec.md:4556-4566`) is enforced bridge-side (P1a-BR-04); this
  task does not encode the rewrite at the data-shape layer.
- The companion **`Ty::Closure` identity view** vs
  **`Ty::Struct(ClosureCaptures, args, layout)` layout view**
  split (per `spec.md:4571-4587` / D6.3) is reachable via
  `find_nominal(NominalKind::Struct, item, args)` on the type
  interner; this task only commits the synthetic-Item
  discriminator shape.

## Non-Goals

- The full `SyntheticItemKind` enum (including the vtable family
  and the const-memory family) lands in regseq-model-73
  (P1a-DS-08b). This task only contributes the closure-family
  variants and `ClosureCallKind`.
- The `PrincipalTraitKey` / `ProjectionBindingsKey` /
  `ContentHash` types and the synthetic `DefPath` segment
  encoding + synthetic `DefPathHash` construction algorithm land
  in regseq-model-73 (P1a-DS-08b).
- Bridge-side closure synthesis (closure encounter +
  `ClosureCaptures` Item lifting + per-`ClosureKind` call-method
  Function Item synthesis + `FnAbi::RustCall -> FnAbi::Rust`
  rewrite) is P1a-BR-04 territory.
- Computation of the parallel `Ty::Struct(ClosureCaptures, args,
  layout)` for the captures-struct layout view is bridge-side
  (P1a-BR-02 / P1a-BR-04).
- Validation that `ClosureCaptures` is the carve-out exception
  in invariant 2 lands in P1a-VAL-02.
- `Coroutine` / `CoroutineClosure` / `CoroutineWitness` rejection
  (`spec.md:4568-4569`) is bridge-side (P1a-BR-02 §15.1
  rejection rows).

## Test Plan

**Test organization**: `unit-in-src` -- tests live in
`crates/regseq-model/src/synthetic.rs` `#[cfg(test)] mod tests`.

Coverage requirements (every public type / function exercised at
least once per `SESSION.md` Testing Requirements):

1. **`ClosureCallKind` construction.** Construct each of the 3
   variants: `Call`, `CallMut`, `CallOnce`. Assert pairwise
   inequality. Assert clone produces equal value.
2. **`ClosureCallKind` derives.** Verify `Hash` produces
   identical output for cloned values and distinct outputs for
   distinct variants (via a `HashSet` round-trip).
3. **`ClosureCallKind` Debug.** Assert `Debug` formats yield
   distinguishable strings for the 3 variants.
4. **`SyntheticItemKind::ClosureCaptures` construction.**
   Construct with a fixture `DefPathHash`. Assert the variant
   round-trips through `Clone` and `==`.
5. **`SyntheticItemKind::ClosureCall` construction.** Construct
   for each `ClosureCallKind` variant; assert per-`call_kind`
   distinctness with a fixed closure DefPathHash.
6. **`SyntheticItemKind::ClosureDropGlue` construction.**
   Construct with a fixture `DefPathHash`. Assert distinctness
   from `ClosureCall { closure: same, call_kind: _ }` and from
   `ClosureCaptures { closure: same }`.
7. **Cross-variant inequality.** For one fixed
   `closure: DefPathHash`, assert all four (Captures + 3 Call
   kinds + DropGlue) are pairwise distinct.
8. **Serde round-trip.** For each closure-family variant
   constructed above, run a `serde_json::to_string` /
   `from_str` round-trip and assert `==` of the round-tripped
   value with the original. Verify `ClosureCallKind` round-trips
   in isolation as well.
9. **`ClosureCallKind` rank table.** Confirm by inspection /
   exhaustive `match` test that the variant declaration order
   matches the discriminant table at `spec.md:5157`
   (`Call=0, CallMut=1, CallOnce=2`).

`cargo test -p regseq-model` passes.

## Review

Once tests pass, request a Codex review of the implementation by
invoking `scripts/python/codex_runner.py` via the Bash tool (per
`SESSION.md` Review section). Use the four-block template
verbatim:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-model-72 against its task spec at
.harness/tasks/regseq-model/72-closure-family-synthesised-items.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-model/src/synthetic.rs
- crates/regseq-model/src/lib.rs

Focus on:
- Spec deviations (every rule in the Implementation Notes section).
- Correctness issues and generality loss.
- Design misalignment with the spec's public API / architecture.
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

Iterate (fix -> re-invoke `codex_runner.py`) until VERDICT: APPROVED.

## Quality Gate

```
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

The authoritative gate is `task.py done`, which runs cleanup plus
workspace-wide test/fmt/clippy per `SESSION.md` Testing
Requirements. The above commands are dev-time guidance.

## Acceptance Criteria

- New module `crates/regseq-model/src/synthetic.rs` created and
  re-exported from `crates/regseq-model/src/lib.rs`.
- `ClosureCallKind` enum defined verbatim per `spec.md:4905`
  (3 variants in declaration order Call / CallMut / CallOnce).
- The three closure-family `SyntheticItemKind` variants
  (`ClosureCaptures`, `ClosureCall`, `ClosureDropGlue`) defined
  per `spec.md:4866-4874` payload shapes.
- All public types carry doc comments (per `SESSION.md` Coding
  Conventions).
- Test Plan items 1-9 implemented as `#[cfg(test)] mod tests`
  cases; all pass under `cargo test -p regseq-model`.
- `cargo fmt --all -- --check` and
  `cargo clippy -p regseq-model -- -D warnings` pass.
- Codex Task Review returns VERDICT: APPROVED.
