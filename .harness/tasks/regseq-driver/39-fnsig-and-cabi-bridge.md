# regseq-driver-39: FnSig and C-ABI Bridge

## Description

Translate rustc function signatures into the `regseq-model` `FnSig` per
`spec.md` §14, and produce the per-signature `CAbiLowering` output
shape (Direct / Substituted { emit_as, reason }) per spec.md §14.5. The
bridge stores the **outcome** of the C-ABI admissibility predicates
`CAbiPositionAdmissible` and `CFieldAdmissible`; it does not re-derive
them at consumer time.

Runs during Phase 2 of the bridge pipeline scaffolded by
regseq-driver-36. Required by every Function Item the bridge produces.

## Input Context

- `docs/discussions/regseq-model/spec.md` §14 (FnAbi + FnSig + CAbi
  admissibility) -- especially §14.3 FnSig schema and §14.5
  CAbiLowering shape
- `crates/regseq-model/src/fnsig/` (FnAbi, FnSig, FnSigInterner,
  CAbiLowering shape from regseq-model-70)
- `crates/regseq-driver/src/bridge/mod.rs` (BridgeCtxt + BridgeError
  from regseq-driver-36)
- `crates/regseq-driver/src/bridge/ty.rs` (translate_ty from
  regseq-driver-37)

## Implementation Notes

Create `crates/regseq-driver/src/bridge/fnsig.rs`. Wire `pub mod fnsig;`
from `crates/regseq-driver/src/bridge/mod.rs`. The CAbi predicate
helpers live in a sibling submodule.

### Public API

```rust
// bridge/fnsig.rs

/// Translate a rustc `PolyFnSig` into a `regseq_model::FnSigId`. Per
/// spec.md §14.3 schema. Region erasure runs first via
/// `bridge::ty::erase_regions_or_reject`.
pub fn translate_fn_sig<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    sig: rustc_middle::ty::PolyFnSig<'tcx>,
) -> Result<regseq_model::FnSigId, BridgeError>;

/// Compute the CAbiLowering for a translated FnSig per spec.md §14.5.
/// Stores the per-position outcomes of the
/// CAbiPositionAdmissible / CFieldAdmissible predicates.
pub fn compute_cabi_lowering(
    cx: &mut BridgeCtxt<'_>,
    sig_id: regseq_model::FnSigId,
) -> regseq_model::CAbiLowering;
```

### CAbi predicate helpers (private)

Per spec.md §14.5 the bridge derives:

- `CAbiPositionAdmissible(ty, position)` -- per-argument and
  return-position admissibility.
- `CFieldAdmissible(ty)` -- per-field admissibility for record
  positions.

Each predicate returns either `Direct` (the position passes through
unchanged) or `Substituted { emit_as, reason }` (the position lowers
through the substitute typedef and the reason is recorded for
diagnostics).

The bridge runs each predicate once per FnSig translation and stores
the results into `RegseqModel.sig_c_abi_lowerings[sig_id]` so consumers
read the outcome instead of re-deriving.

### Module structure

- `crates/regseq-driver/src/bridge/fnsig.rs` -- `translate_fn_sig` +
  `compute_cabi_lowering` + private CAbi predicate helpers.

### FnSig translation flow (spec.md §14.3)

Per spec.md §14.3:

1. Erase regions on the rustc PolyFnSig (uses regseq-driver-37's
   `erase_regions_or_reject`).
2. Translate each input Ty via `bridge::ty::translate_ty`.
3. Translate the output Ty via `bridge::ty::translate_ty`.
4. Map the rustc ABI to the `FnAbi` variant per spec.md §14.1.
5. Intern the resulting `FnSig` via the model's FnSigInterner.
6. Compute and store the `CAbiLowering` per §14.5.

## Non-Goals

- Closure / vtable / drop-glue synthesis (regseq-driver-40).
- Place / Operand / Rvalue / Statement translation (regseq-driver-41).
- Terminator translation (regseq-driver-42).
- Body materialisation (regseq-driver-43).
- C-emission of the substituted typedef (phase-1b territory per the
  brief Non-Goals).

## Test Plan

### Unit tests (`crates/regseq-driver/src/bridge/fnsig.rs` `#[cfg(test)] mod tests`)

- For each FnAbi variant per spec.md §14.1 (Rust / RustCall / C /
  Unadjusted), construct a rustc PolyFnSig and assert the translated
  `FnSig.abi` matches.
- For each CAbiLowering output shape (Direct vs Substituted), assert
  the predicate output is recorded into
  `RegseqModel.sig_c_abi_lowerings`.

### Integration tests (`crates/regseq-driver/tests/bridge_fnsig.rs`)

Bridge tests are integration-in-tests because they need rustc-compiled
fixtures.

Positive coverage:

- Rust ABI fixture -- `pub fn add(a: u32, b: u32) -> u32 { a + b }`
  -- assert FnSig with 2 inputs / 1 output / Rust ABI / Direct
  CAbiLowering.
- C ABI fixture -- `pub extern "C" fn cfn(x: i32) -> i32 { x }` --
  assert C ABI / Direct CAbiLowering.
- Unit return -- `pub fn u() {}` -- assert empty inputs + Unit output.
- Reference input -- `pub fn r(p: &u32) -> u32 { *p }` -- assert Ref
  input / Direct CAbiLowering.
- Substituted-position fixture -- a function whose argument requires
  the §14.5 substitute typedef path -- assert Substituted CAbiLowering
  with the `emit_as` and `reason` fields populated.
- Region-substantive HRTB fixture -- assert
  `BridgeError::SubstantiveHrtb` (delegates to regseq-driver-37's
  helper; this asserts the wiring).

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-driver-39 against its task spec at
.harness/tasks/regseq-driver/39-fnsig-and-cabi-bridge.md. ALL CHECKS
MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-driver/src/bridge/mod.rs
- crates/regseq-driver/src/bridge/fnsig.rs
- crates/regseq-driver/tests/bridge_fnsig.rs
- (any new fixture files under crates/regseq-driver/tests/fixtures/)

Focus on:
- Spec deviations vs spec.md §14.1 / §14.3 / §14.5.
- Per-FnAbi-variant test coverage.
- CAbiLowering output is stored as the outcome of
  CAbiPositionAdmissible / CFieldAdmissible (the bridge does not
  re-derive at consumer time).
- Region erasure delegates to regseq-driver-37's
  `erase_regions_or_reject`.
Do NOT flag anything explicitly marked out of scope (Non-Goals,
later task IDs). Substituted-typedef C-emission is phase-1b.
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

- `crates/regseq-driver/src/bridge/fnsig.rs` created with
  `translate_fn_sig` and `compute_cabi_lowering`.
- Region erasure delegates to regseq-driver-37's helper.
- Each FnAbi variant per spec.md §14.1 covered by a positive test.
- Both CAbiLowering shapes (Direct / Substituted) covered by a
  positive test.
- All tests pass; Codex Task Review verdict APPROVED.
