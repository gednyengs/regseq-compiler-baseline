# RV-009-G slice plan: raw-pointer aggregate with an unsized (slice/str) pointee

R4 of the real-fix backlog (first of the two F70 residuals; FF-005 follows).
Plan-first, probe-grounded at HEAD on pinned nightly-2026-02-01.

## 0. The HANDOFF framing was mis-anchored

The HANDOFF describes the RV-009-G slice gap as "a separate pre-existing
bridge gap at `bridge/body/rvalue.rs:197`". A HEAD-grounded probe disproved
the anchor: the `AggregateKind::RawPtr` arm at `rvalue.rs:197` is not itself
the rejection site — it merely forwards the **bare pointee** to the
**sized-context** type translator, which is what rejects. The real gate is
`translate_erased_ty_inner`'s `allow_unsized_marker` flag (`bridge/ty.rs:221-227`).

## 1. Problem statement (verified at HEAD)

`core::intrinsics::aggregate_raw_ptr(data, meta)` builds a raw pointer from a
data pointer + metadata and lowers to `Rvalue::Aggregate(AggregateKind::RawPtr(..), [data, meta])`.

Empirical probe outcomes (throwaway test via `try_bridge_fixture_path`,
since reverted):

| Snippet (result type) | HEAD | After spike |
| --- | --- | --- |
| `aggregate_raw_ptr(data, ())` → `*const u32` (sized) | `Ok` | `Ok` |
| `aggregate_raw_ptr(data, len)` → `*const [u32]` (slice) | `Rejected: UnsupportedTy { "[u32]" }` | `Ok` |
| `aggregate_raw_ptr(data, len)` → `*mut [u32]` (mut slice) | (same class) | `Ok` |
| `aggregate_raw_ptr(data, len)` → `*const str` | (same class) | `Ok` |

Control facts (also probed, all `Ok` at HEAD): `*const [u32]` as a plain
function parameter/return type bridges fine; `&[u32]` bridges fine. So the
slice-pointee *type* is fully representable — only the **aggregate
construction path** translates the pointee in the wrong context.

Root cause: `rvalue.rs:172` `translate_aggregate_kind`'s
`mir::AggregateKind::RawPtr(pointee, mutability)` arm calls
`translate_ty(cx, pointee)`. `translate_ty` (`ty.rs:26-32`) erases regions
then calls `translate_erased_ty(cx, ty, /*allow_unsized_marker=*/ false)`.
With the marker `false`, `TyKind::Slice` (`ty.rs:221-227`) and `TyKind::Str`
(`ty.rs:203-210`) reject with `UnsupportedTy`. The pointer **type** paths
(`TyKind::RawPtr` `ty.rs:233-242`, `TyKind::Ref` `ty.rs:229-232`) instead
call `translate_erased_ty(cx, *pointee, /*allow_unsized_marker=*/ true)`,
which is why `*const [u32]` works as a type but not when rebuilt from parts.

## 2. Code-surface grounding (all at HEAD)

- `crates/regseq-driver/src/bridge/body/rvalue.rs:197-200` — the RawPtr arm
  (modification site). Imports `ty::translate_ty` at `:10`.
- `crates/regseq-driver/src/bridge/ty.rs:50-79` — `translate_erased_ty`
  (private; runs projection-normalization + caches; takes the marker).
- `:221-227` Slice gate; `:203-210` Str gate; `:233-242` RawPtr type path
  (the `true`-marker reference); `:38-48` `erase_regions_or_reject`.
- `crates/regseq-model/src/ty.rs:246` `Ty::Slice(TyId)` + `:233` `Ty::Str` —
  "Appears only as a `Ref` or `Ptr` pointee" (exactly this case).
- `crates/regseq-model/src/body_ty.rs:314-316` — recovers the aggregate
  result type as `Ty::Ptr(*pointee, *mutability)`; `:321-339` `ptr_metadata_ty`
  already handles `Ty::Slice(_) | Ty::Str` pointees (→ `usize` metadata) and
  `Ty::Dynamic` (→ vtable ptr). So the model + place/body validators already
  admit a `Ty::Slice`/`Ty::Str` `Ptr` pointee; only the bridge forward path
  rejected it.
- `crates/regseq-model/src/place_validator.rs:250` — `AggregateKind::RawPtr(_,_) => {}`
  (no extra constraint on the aggregate kind itself).

## 3. Fix directions evaluated

### Option A — unsized-allowed pointee translation in the RawPtr arm (RECOMMENDED)

Add a small `pub(crate)` helper in `ty.rs` that translates a raw/ref pointee
in the unsized-allowed context (mirroring exactly what the `TyKind::RawPtr`
type path does internally), and call it from the RawPtr aggregate arm:

```rust
// bridge/ty.rs (next to translate_erased_ty)
pub(crate) fn translate_ptr_pointee<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    pointee: rustc_middle::ty::Ty<'tcx>,
) -> Result<TyId, BridgeError> {
    let pointee = erase_regions_or_reject(cx.input.tcx, pointee)?;
    translate_erased_ty(cx, pointee, /* allow_unsized_marker = */ true)
}

// bridge/body/rvalue.rs RawPtr arm
mir::AggregateKind::RawPtr(pointee, mutability) => Ok(AggregateKind::RawPtr(
    crate::bridge::ty::translate_ptr_pointee(cx, pointee)?,
    translate_ast_mutability(mutability),
)),
```

**Empirically proven** (spike above): slice / mut-slice / str all reach `Ok`;
the sized case is unchanged (`u32` is sized → identical `TyId` either way).
The helper keeps `translate_erased_ty` private and exposes a single,
intent-named entry; it does region-erasure first because the aggregate's
rustc `pointee` (unlike the type-path's already-erased pointee) is raw.

### Option B — translate the full `rvalue_ty` and destructure `Ty::Ptr` — NOT chosen

`translate_ty(cx, rvalue_ty)` yields `Ty::Ptr(pointee, mut)`; resolve and
read both back. Reuses the type path with zero `ty.rs` additions, but forces
a structurally-required `else`/catch-all for the "result type is not a
`Ty::Ptr`" case that cannot occur for a `RawPtr` aggregate — i.e. a dead
defensive arm (CLAUDE.md §1.2). Option A matches the existing arm shape
(keep `pointee` + `mutability` from the MIR match) with no dead branch.

### Option C — bump `translate_erased_ty` to `pub(crate)` and call it directly — NOT chosen

Spreads a 3-arg internal entry (with the projection-normalization +
caching contract) across modules. The named helper documents intent and
keeps the marker discipline inside `ty.rs`.

## 4. Why Option A is principled

A raw-pointer aggregate's pointee is, by construction, a pointee — the
exact position where the model admits the unsized `Slice`/`Str`/`Dynamic`
markers. Translating it in the sized context was the bug; translating it in
the same unsized-allowed context the pointer **type** paths already use makes
the aggregate path consistent with the type path. No new model variant, no
validator change, no new rejection.

## 5. Proposed scope

- **Change A** — `ty.rs`: add `pub(crate) fn translate_ptr_pointee`.
- **Change B** — `rvalue.rs`: RawPtr arm uses `translate_ptr_pointee`.
- **Change C** — coverage: promote the committed `RV-009-G` (sized) and add a
  new sibling sidecar row for the slice case (next free id, e.g. `RV-009-H`)
  + snippet; matrix RV-009-G note updated to reflect slice coverage. Decide
  with Codex whether the slice case is a new row id or an additional snippet
  under RV-009-G's cluster.
- **Change D** — integration test (`bridge_rawptr_slice_aggregate.rs` or fold
  into an existing rvalue test): sized (regression) + slice + mut-slice + str,
  each `BridgeOutcome::Ok` + validates, asserting the rvalue is
  `AggregateKind::RawPtr` with a `Ty::Slice`/`Ty::Str` pointee.

Non-goals: dyn-from-parts (`*const dyn Trait` via `aggregate_raw_ptr`) unless
it falls out for free — `ptr_metadata_ty` handles `Ty::Dynamic`, but the dyn
metadata operand is a vtable pointer and may surface the DynMetadata Foreign
hazard A1 explicitly deferred; probe it, and if it rejects for a *separate*
reason, scope it out with a note. No change to `translate_ty` semantics, the
sized path, or any validator.

## 6. Acceptance criteria (preview)

1. `aggregate_raw_ptr(data, len) -> *const [u32]` bridges `Ok` + validates,
   rvalue = `AggregateKind::RawPtr` with a `Ty::Slice(u32)` pointee.
2. `*mut [u32]` and `*const str` slice/str cases bridge `Ok` + validate.
3. The sized case (`*const u32`) is unchanged and passing.
4. Mode A `Total == Pass`, `Fail == 0`, exit 0, with the new slice row.
5. build / fmt / clippy `-p regseq-driver --lib` + new test target / tests green.

## 7. Risks

- **Low.** The fix is a context-flag correction on one translation call;
  empirically proven for slice/mut-slice/str with no sized regression.
- The only open scoping question is dyn-from-parts (§5 non-goals) — handle by
  probe-and-scope, not by speculative code.
- `translate_ptr_pointee` must erase regions (the aggregate pointee is raw);
  the spike included the erase and passed.

## 8. Open questions for Codex

1. Option A (named `pub(crate)` helper) vs Option B (destructure full
   `rvalue_ty`) vs Option C (expose `translate_erased_ty`) — is A the right
   minimal, dead-code-free choice?
2. Row taxonomy: new `RV-009-H` slice row vs an extra snippet under the
   `RV-009` cluster for the slice case — which matches the runner's
   cluster_key conventions?
3. Is the dyn-from-parts case in or out of scope, given A1's deferred
   DynMetadata Foreign hazard? (Probe result to be included in the spec.)
4. Any Phase-5 / `CallArgTypeMismatch` / metadata-operand-type validation
   that a slice raw-ptr aggregate must satisfy beyond what the sized case
   already exercises?
