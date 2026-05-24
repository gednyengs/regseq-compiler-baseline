# regseq-driver-76 — Raw-pointer aggregate with an unsized (slice/str) pointee (RV-009-G slice case)

## 1. Description

Building a raw pointer from parts whose result is a **slice** (`*const [T]`)
or **str** (`*const str`) raw pointer is rejected by the bridge:

```rust
#![feature(core_intrinsics)]
pub fn raw_slice(data: *const u32, len: usize) -> *const [u32] {
    core::intrinsics::aggregate_raw_ptr(data, len)
}
```

At HEAD this returns `BridgeOutcome::Rejected(UnsupportedTy { ty_repr: "[u32]" })`.
The **sized** counterpart (`*const u32` via `aggregate_raw_ptr(data, ())`)
already bridges `Ok` (committed row `RV-009-G`).

Root cause: `core::intrinsics::aggregate_raw_ptr` lowers to
`Rvalue::Aggregate(AggregateKind::RawPtr(pointee, mutability), [data, meta])`.
The bridge's `translate_aggregate_kind` `RawPtr` arm
(`bridge/body/rvalue.rs:196-199`) translates the **bare pointee** via the
sized-context `translate_ty`, which erases regions then calls
`translate_erased_ty(.., /*allow_unsized_marker=*/ false)` (`bridge/ty.rs:26-32`).
With the marker `false`, `TyKind::Slice` (`ty.rs:221-227`) and `TyKind::Str`
(`ty.rs:203-210`) reject with `UnsupportedTy`. The pointer **type** paths
(`TyKind::RawPtr` `ty.rs:233-242`, `TyKind::Ref` `ty.rs:229-232`) instead
translate their pointee with `allow_unsized_marker = true` — which is why
`*const [u32]` works as a plain parameter/return type but not when
reconstructed from parts.

Fix: translate the RawPtr aggregate's pointee in the same unsized-allowed
context the pointer type paths use. The model already admits a `Ty::Slice` /
`Ty::Str` `Ptr` pointee (`regseq-model/src/ty.rs:233,246`; result-type
recovery `body_ty.rs:314-316`; metadata recovery `body_ty.rs:321-339`); no
model or validator change is needed.

Plan + Codex review (APPROVED, no required fixes):
`.harness/audits/2026-05-24-rv009g-slice-rawptr-plan.md`. Empirically proven
via a reverted spike: slice / mut-slice / str all reach `Ok`, sized
unchanged; dyn-from-parts stays `Rejected(Foreign)` (out of scope, below).

## 2. Input Context

### Design source

- `.harness/audits/2026-05-24-rv009g-slice-rawptr-plan.md` — Option A
  (recommended, Codex-APPROVED). Codex independently probed dyn-from-parts
  (`Rejected(Foreign { std::ptr::metadata::VTable })`) and confirmed the
  sized case is marker-invariant (`u32` interns identically).

### Spec anchors

- `docs/discussions/regseq-model/spec.md` — `Ty::Slice`/`Ty::Str` are unsized
  markers valid only as a `Ref`/`Ptr` pointee (model `ty.rs:233,246`).
- `docs/discussions/regseq-model/bridge-coverage-matrix.md:304` — `RV-009-G`
  `AggregateKind::RawPtr`, described "raw-pointer aggregate (slice / dyn from parts)".

### Bridge code anchors (consumed/modified)

- `crates/regseq-driver/src/bridge/body/rvalue.rs:196-199` — the
  `mir::AggregateKind::RawPtr(pointee, mutability)` arm (**modification
  site**); current call `translate_ty(cx, pointee)`. `translate_ast_mutability`
  at `:213-218` is unchanged. Import line `:10`.
- `crates/regseq-driver/src/bridge/ty.rs:26-32` — `translate_ty` (erase +
  `translate_erased_ty(.., false)`).
- `:38-48` — `erase_regions_or_reject`.
- `:50-79` — `translate_erased_ty` (private; runs `normalize_authorized_projections`
  + forward cache; takes `allow_unsized_marker`). **New helper goes adjacent.**
- `:203-210` Str gate; `:221-227` Slice gate; `:229-242` Ref/RawPtr type
  paths (the `true`-marker references). `:201` Uint arm (marker-invariant →
  sized unaffected).

### Model surface (consumed read-only)

- `crates/regseq-model/src/ty.rs:233` `Ty::Str`, `:246` `Ty::Slice(TyId)`.
- `crates/regseq-model/src/body_ty.rs:314-316` RawPtr aggregate result-type
  recovery (`Ty::Ptr(*pointee, *mutability)`); `:321-339` `ptr_metadata_ty`
  (`Ty::Slice(_) | Ty::Str` → `usize`; `Ty::Dynamic` → vtable ptr).
- `crates/regseq-model/src/place_validator.rs:242,250` — `AggregateKind::RawPtr`
  imposes no extra aggregate-kind constraint beyond operand validation.

### Prior tasks

- regseq-driver-70 (A1/F70) — landed the sized `RV-009-G` + the projection
  folder; escalated the slice case (this task) and FF-005 (the other F70
  residual, a separate later task). Deferred the DynMetadata Foreign hazard.

### Test surface

- New: `crates/regseq-driver/tests/bridge_rawptr_slice_aggregate.rs`.
- Broadened snippet `crates/bridge-coverage-runner/snippets/RV-009-G.rs`
  (add the slice case alongside sized; keep the row id `RV-009-G`).
- Mode A runner over the full corpus.

## 3. Implementation Notes

### Change A — unsized-allowed pointee helper in `ty.rs`

Add adjacent to `translate_erased_ty`:

```rust
/// Translate the pointee of a raw-pointer / reference position, where an
/// unsized `Slice`/`Str`/`Dynamic` marker is admissible (mirrors the
/// `TyKind::RawPtr` / `TyKind::Ref` type-path pointee translation). The
/// aggregate's rustc pointee is not pre-erased, so erase first.
pub(crate) fn translate_ptr_pointee<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    pointee: rustc_middle::ty::Ty<'tcx>,
) -> Result<TyId, BridgeError> {
    let pointee = erase_regions_or_reject(cx.input.tcx, pointee)?;
    translate_erased_ty(cx, pointee, /* allow_unsized_marker = */ true)
}
```

Keep `translate_erased_ty` private; expose only this intent-named entry.

### Change B — RawPtr aggregate arm uses the helper

In `translate_aggregate_kind` (`rvalue.rs:196-199`):

```rust
mir::AggregateKind::RawPtr(pointee, mutability) => Ok(AggregateKind::RawPtr(
    crate::bridge::ty::translate_ptr_pointee(cx, pointee)?,
    translate_ast_mutability(mutability),
)),
```

No other arm changes. The existing `operand_count != 2` guard (`rvalue.rs`
`and_then` tail) is unchanged and still applies.

### Change C — coverage row + tests

- **Broaden `snippets/RV-009-G.rs`** to add the slice case alongside the
  existing sized fn (keep the row id `RV-009-G`; per Codex the matrix already
  describes it as "slice / dyn from parts", and a new sidecar id absent from
  the matrix is a hard drift failure at `matrix.rs:246`). The predicate stays
  `inspect = { kind = "AggregateKind", arg = "RawPtr" }`; the discriminating
  signal is that the snippet now reaches `Ok` AT ALL (HEAD rejects the slice
  fn). Example:

  ```rust
  #![feature(core_intrinsics)]
  pub fn raw_sized(data: *const u32) -> *const u32 {
      core::intrinsics::aggregate_raw_ptr(data, ())
  }
  pub fn raw_slice(data: *const u32, len: usize) -> *const [u32] {
      core::intrinsics::aggregate_raw_ptr(data, len)
  }
  ```

- **Update the matrix `RV-009-G` row** (`docs/discussions/regseq-model/bridge-coverage-matrix.md:304`):
  status `uncovered` → `covered`, with a note that slice + str pointees now
  bridge via the unsized-allowed aggregate pointee translation, dyn-from-parts
  stays `Rejected(Foreign)` (DynMetadata hazard, A1-deferred).

- **New integration test** `bridge_rawptr_slice_aggregate.rs` (helper pattern
  from `bridge_zero_arg_closure_call.rs`; `catch_unwind` guard). See Test Plan.

## 4. Non-Goals

- **dyn-from-parts** (`*const dyn Trait` via `aggregate_raw_ptr`) is OUT.
  Codex probed it: `Rejected(Foreign { std::ptr::metadata::VTable })` even
  with Change A applied — it hits the A1-deferred DynMetadata Foreign hazard
  (`ty.rs` dyn vtable synthesis + Foreign rejection), a separate task class.
  Do not attempt to unblock it; a test MAY assert it still rejects (optional).
- **Do NOT change `translate_ty` semantics, the sized path, the `operand_count`
  guard, or any model/validator.** Sized `u32` must stay byte-identical.
- **Do NOT widen `translate_erased_ty` visibility** (Option C) or destructure
  the full `rvalue_ty` (Option B); use the named helper.
- No `UnOp::PtrMetadata` / `core::ptr::metadata` work (that is RV-007-C,
  already covered).

## 5. Test Plan

Integration tests in `bridge_rawptr_slice_aggregate.rs` (each wraps the
in-process bridge in `catch_unwind`; assert `BridgeOutcome::Ok` + the model
validates, and that the rvalue is an `AggregateKind::RawPtr` whose pointee is
the expected unsized/sized marker):

- **B1 (slice)**: `raw_slice(data: *const u32, len: usize) -> *const [u32]`
  → `Ok` + validates; rvalue `AggregateKind::RawPtr` with a `Ty::Slice(u32)`
  pointee (i.e. result type `Ty::Ptr(Slice(u32), Not)`).
- **B2 (mut slice)**: `*mut u32, usize -> *mut [u32]` → `Ok` + validates;
  pointee `Ty::Slice`, mutability `Mut`.
- **B3 (str)**: `*const u8, usize -> *const str` → `Ok` + validates; pointee
  `Ty::Str`.
- **B4 (sized regression)**: `*const u32 via (data, ())` → `Ok` + validates;
  pointee `Ty::Uint(U32)` (unchanged from HEAD).
- **B5 (Mode A)**: full runner over the corpus + the broadened `RV-009-G`
  reports `Total == Pass`, `Fail == 0`, exit 0; every pre-existing row
  unchanged (re-run Mode A at HEAD and after; diff per-row).
- **B6 (optional, dyn stays rejected)**: a `*const dyn Trait`-from-parts shape
  still returns `Rejected` (documents the out-of-scope boundary).

## 6. Review

### Files to send to Codex

- `.harness/tasks/regseq-driver/76-rawptr-slice-aggregate.md` (this spec)
- `.harness/audits/2026-05-24-rv009g-slice-rawptr-plan.md`
- The implemented diff (Changes A/B/C).

### Reviewer focus

1. Does Change A translate the pointee in the unsized-allowed context exactly
   as the `TyKind::RawPtr`/`Ref` type paths do, incl. region erasure, and keep
   `translate_erased_ty` private?
2. Is the sized case byte-identical (marker-invariant for sized pointees)?
3. Does the slice/str result validate end-to-end (result type `Ty::Ptr` over a
   `Ty::Slice`/`Ty::Str` pointee; metadata operand recovery) with no
   model/validator change?
4. Diff scope confined to the helper + the one arm + the RV-009-G snippet +
   matrix row + the new test? No new sidecar row id, no matrix drift.

### Adversarial prompt template (four-block)

```
<task>
Review the implemented diff for regseq-driver-76 (cwd = repo root). The fix adds a
pub(crate) translate_ptr_pointee helper in bridge/ty.rs (erase regions + translate the
pointee with allow_unsized_marker=true) and calls it from the AggregateKind::RawPtr arm in
bridge/body/rvalue.rs, so a slice/str raw-pointer aggregate bridges Ok instead of
UnsupportedTy. Verify against the spec anchors + plan. You may build and run the Mode A
runner and bridge probe snippets. Out of scope: dyn-from-parts (deferred Foreign hazard);
the FF-005 FnPtrShim residual.
</task>
<grounding_rules>
Cite file:line for every claim. Read the diff + cited code; you may build/probe.
</grounding_rules>
<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
SLICE_STR_BRIDGES_OK: PASS|FAIL + cite
SIZED_UNREGRESSED: PASS|FAIL + cite
NO_MODEL_OR_VALIDATOR_CHANGE: PASS|FAIL + cite
DIFF_SCOPE_CONFINED: PASS|FAIL + cite
NEW ISSUES: numbered or "none"
REQUIRED FIXES: numbered (only if VERDICT != APPROVED)
</structured_output_contract>
<default_follow_through_policy>
Do not ask clarifying questions. Read the files, decide, stop.
</default_follow_through_policy>
```

## 7. Quality Gate

- `cargo build` clean.
- `cargo fmt --all -- --check` clean.
- `cargo clippy -p regseq-driver --lib -- -D warnings` clean AND the new test
  target clean: `cargo clippy -p regseq-driver --test bridge_rawptr_slice_aggregate
  -- -D warnings`. **NOTE:** do NOT gate on `--all-targets` — it has MANY
  PRE-EXISTING test-target lint failures (clippy-version drift in untouched
  test files; documented in the driver-74/-75 assessments). The bar is: the
  new lib helper + the new test add no new lints.
- `cargo test -p regseq-driver -- --test-threads=1` green (incl. the new
  `bridge_rawptr_slice_aggregate.rs`).
- Mode A: `Total == Pass`, `Fail == 0`, exit 0, with the broadened `RV-009-G`.
- Diff scope confined to: `bridge/ty.rs` (the new helper only),
  `bridge/body/rvalue.rs` (the RawPtr arm only), `snippets/RV-009-G.rs`, the
  `RV-009-G` matrix row, and the new test file. No new sidecar row id; no
  edits to `translate_ty`, the sized path, the operand-count guard, or any
  model/validator.

## 8. Acceptance Criteria

1. `aggregate_raw_ptr(data, len) -> *const [u32]` bridges `BridgeOutcome::Ok`
   and validates; rvalue `AggregateKind::RawPtr` with a `Ty::Slice(u32)`
   pointee.
2. The mut-slice (`*mut [u32]`) and str (`*const str`) cases bridge `Ok` +
   validate.
3. The sized case (`*const u32`) is unchanged and passing.
4. dyn-from-parts is out of scope: the fix does not touch the dyn path, which
   still returns `Rejected(Foreign)`. A test asserting this (B6) is optional,
   not required for acceptance.
5. Mode A: `Total == Pass`, `Fail == 0`, exit 0, with `RV-009-G` covered;
   every pre-existing row unchanged.
6. No model or validator change; `translate_erased_ty` stays private.
7. `cargo build` / `fmt` / `clippy -p regseq-driver --lib` / `clippy -p
   regseq-driver --test bridge_rawptr_slice_aggregate` / `cargo test -p
   regseq-driver` all green.
