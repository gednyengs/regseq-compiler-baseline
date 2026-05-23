# regseq-driver-70: Bridge associated-type projection normalization (A1: RV-007-C + RV-009-G + FF-005)

## Description

This task implements proposal **A1** from the F68 post-F4 triage at
`.harness/audits/2026-05-21-post-f4-triage.md:87-131`. It extends
the bridge's type-translation entry to normalize two
associated-type projection families to their concrete monomorphic
types before the `TyKind::Alias(..)` short-circuit at
`crates/regseq-driver/src/bridge/ty.rs:296` fires, then authors
three new sidecar rows that exercise the normalization to bring
three F4 escalations to PASS:

- **RV-007-C** (`UnOp::PtrMetadata`): per F4 evidence at
  `.harness/expectations/regseq-driver-67.md:238-254`,
  `core::ptr::metadata::<T>(p)` produces a `Place` typed as
  `<T as core::ptr::Pointee>::Metadata`. Empirically the F4 probes
  observed TWO distinct rejection modes:
    - **Slice** (`<[u32] as Pointee>::Metadata`) and **sized**
      (`<u32 as Pointee>::Metadata`) candidates short-circuit at
      `crates/regseq-driver/src/bridge/ty.rs:296` with
      `BridgeError::NonRigidTy { kind: NonRigidTyKind::Alias, .. }`
      because the bridge does not normalize the projection.
      A1 addresses these two.
    - **Dyn-trait** (`<dyn Tr as Pointee>::Metadata`) candidate
      hits a downstream `BridgeError::Foreign` on
      `core::ptr::metadata::VTable` (an `extern type`) because
      rustc's own machinery partially normalizes the dyn case
      to a `DynMetadata`-shaped type whose interior contains a
      `NonNull<VTable>` pointer; the bridge's
      `TyKind::RawPtr` Foreign check at
      `crates/regseq-driver/src/bridge/ty.rs:103-108` then fires.
      The dyn case has a DIFFERENT root cause from the
      slice / sized cases and is **out of scope for A1**; see
      Out of scope below.
- **RV-009-G** (`AggregateKind::RawPtr`): per F4 evidence at
  `.harness/expectations/regseq-driver-67.md:256-262`,
  `core::ptr::from_raw_parts` and `core::intrinsics::aggregate_raw_ptr`
  carry the same `Pointee::Metadata` projection in their
  signatures. Slice + sized specializations both fail with
  `NonRigidTy` on the same projection.
- **FF-005** (`FunctionInstanceFlavor::FnPtrShim`): per F4 evidence
  at `.harness/expectations/regseq-driver-67.md:303-318`, the
  `Fn::call` resolution path that produces `FnPtrShim` builds a
  body whose monomorphized type set contains
  `<fn() as core::ops::FnOnce<()>>::Output`. All three tried call
  shapes (`caller<F: Fn()>(f: F)`, `caller(f: impl Fn())`,
  `fp.call(())` with `fn_traits` feature gate) hit `NonRigidTy`
  before any FnPtrShim instance materializes.

The leverage of A1 comes from a single normalization extension at
the bridge type-translation entry: one code-path change clears
three matrix rows across two distinct sub-axes (`UnOp::PtrMetadata`
+ `AggregateKind::RawPtr` for the `Pointee::Metadata` family, and
`FunctionInstanceFlavor::FnPtrShim` for the `Fn`-family `::Output`
family).

### Why now

A1 is row 2 of the F68 closing-recommendations table at
`.harness/audits/2026-05-21-post-f4-triage.md:519-549` (D1 row at
`:539` already closed by regseq-driver-69; A1 is the next HIGH
priority). The audit explicitly flags A1 as "highest leverage per
unit of bridge change" because the fix is bounded to one code path
in `ty.rs`, the test surface is well-defined by the F4 candidate
shapes, and the three escalations share the same root-cause
family. The runner's sidecar coverage moves from 125 to 128 rows.

### Out of scope

- Bridge changes outside `crates/regseq-driver/src/bridge/ty.rs`.
  The fix is intentionally single-file; the existing
  `instantiate_mir_and_normalize_erasing_regions` usage in
  `crates/regseq-driver/src/bridge/body/materialise.rs:277-289`
  and `crates/regseq-driver/src/bridge/mod.rs:784` is read-only
  reference for the rustc-API pattern, NOT a modification target.
- **Dyn-trait `Pointee::Metadata` normalization.** Per F4 evidence
  at `.harness/expectations/regseq-driver-67.md:243-244`, the
  `<dyn Tr as Pointee>::Metadata` case rejects with
  `BridgeError::Foreign` on `core::ptr::metadata::VTable` (an
  `extern type` declared in the pinned std), NOT with
  `NonRigidTy`. Adding a normalization step for the dyn case
  would not eliminate the Foreign rejection because rustc's
  machinery already partially normalizes the dyn projection to a
  `DynMetadata`-shaped type whose interior contains a
  `NonNull<VTable>` pointer; the bridge's `TyKind::RawPtr`
  Foreign check at
  `crates/regseq-driver/src/bridge/ty.rs:103-108` would still
  fire. Resolving the dyn case therefore requires a SEPARATE
  decision: either widen the bridge's Foreign handling for
  `DynMetadata`'s `NonNull<VTable>` interior, OR encode
  `DynMetadata<dyn Trait>` as an opaque model type. That decision
  belongs to a future task with its own approval gate; A1 leaves
  dyn-Pointee::Metadata behavior unchanged.
- Model / spec / matrix / strategy / pretty-printer /
  generic-c-backend changes.
- Helper changes (test-ir-fixtures lib + tests untouched; F69's
  override surface is unaffected).
- Runner schema changes (no new `Row` fields, no new
  `RowBuildOverrides` allow-list values).
- New `InspectionPredicate` dispatch arms (verified at HEAD: the
  required arms `UnOp("PtrMetadata")`,
  `AggregateKind("RawPtr")`, and `FunctionInstanceFlavor("FnPtrShim")`
  all already exist at
  `crates/bridge-coverage-runner/src/inspect.rs:352`, `:281`, and
  `:210` respectively).
- Other F4 escalation re-attempts (B1, B2, C1, C2, F1 each own
  their own task per the F68 triage document).
- Normalizing other `TyKind::Alias(..)` projection families
  beyond `Pointee::Metadata` and `Fn` / `FnMut` / `FnOnce`
  `::Output`. The bridge's `TyKind::Alias` short-circuit at
  `bridge/ty.rs:296` still rejects every other projection family
  (e.g., GAT projections, `<T as Iterator>::Item`); broadening the
  normalization surface beyond the two families this task
  authorizes is a separate task with its own approval gate per
  the No-Defensive-Fallbacks policy.
- Hand-rolled projection resolution (e.g., walking `tcx.predicates_of`
  and manually substituting). The fix MUST use a rustc-provided
  normalization API (`tcx.normalize_erasing_regions` or an
  equivalent) so the bridge stays aligned with rustc's projection
  semantics.
- Modifying `tests/fixtures/playground/` (per the standing
  playground rule in `.harness/HANDOFF.md`).
- New dependencies in any crate.

## Input Context

### Design source

- F68 audit doc at
  `.harness/audits/2026-05-21-post-f4-triage.md:56-131` --
  Cluster (A) + A1 proposal scope.
- F68 task spec at
  `.harness/tasks/regseq-driver/68-post-f4-batch-triage.md:46-60` --
  Non-Goals enumeration this task respects.
- Strategy doc at
  `docs/discussions/regseq-model/bridge-coverage-runner-strategy.md`
  sections 4 + 4.1 + 8 -- sidecar schema rules (no new sidecar
  fields are added in this task; only new rows).

### F4 expectations evidence (read-only)

- `.harness/expectations/regseq-driver-67.md:238-254` -- RV-007-C
  per-candidate outcomes (slice / dyn / sized specializations).
- `.harness/expectations/regseq-driver-67.md:256-262` -- RV-009-G
  per-candidate outcomes (`from_raw_parts` + intrinsic).
- `.harness/expectations/regseq-driver-67.md:303-318` -- FF-005
  per-candidate outcomes (three `Fn::call` shapes).

### Bridge code (the fix lands here)

- `crates/regseq-driver/src/bridge/ty.rs:23-29` -- `translate_ty`
  entry. Calls `erase_regions_or_reject` then
  `translate_erased_ty`.
- `crates/regseq-driver/src/bridge/ty.rs:35-45` --
  `erase_regions_or_reject`: rejects substantive HRTBs and
  invokes `tcx.erase_and_anonymize_regions(ty)`. Projection
  normalization MUST happen AFTER region erasure (the HRTB check
  must continue to fire on the un-normalized type to preserve
  current semantics) and BEFORE `translate_erased_ty_inner`'s
  match dispatches to the `TyKind::Alias` arm at `:296`.
- `crates/regseq-driver/src/bridge/ty.rs:47-60` --
  `translate_erased_ty`: thin wrapper that calls
  `translate_erased_ty_inner` and caches the (TyId, rustc-Ty)
  pair.
- `crates/regseq-driver/src/bridge/ty.rs:62-301` --
  `translate_erased_ty_inner`: the big `match ty.kind()` that
  dispatches every `TyKind`. The `TyKind::Alias(..)` arm at
  `:296` is the short-circuit site the fix removes for the two
  authorized families.

### Bridge code (existing rustc-API usage; reuse this pattern)

- `crates/regseq-driver/src/bridge/body/materialise.rs:277-289` --
  the bridge's established use of
  `rustc_instance.instantiate_mir_and_normalize_erasing_regions(...)`
  for MIR body translation. The type-translation extension in this
  task uses the analogous type-level normalization API
  (`tcx.normalize_erasing_regions` or
  `tcx.try_normalize_erasing_regions`) from the same `tcx`
  surface.
- `crates/regseq-driver/src/bridge/mod.rs:784` -- second call site
  of `instantiate_mir_and_normalize_erasing_regions` (mirror of
  the materialise.rs path).

### F3 dispatch surface (read-only; verified arms exist)

- `crates/bridge-coverage-runner/src/inspect.rs:352` --
  `un_op_tag(UnOp::PtrMetadata) => "PtrMetadata"`.
- `crates/bridge-coverage-runner/src/inspect.rs:281` --
  `aggregate_kind_tag(AggregateKind::RawPtr(_, _)) => "RawPtr"`.
- `crates/bridge-coverage-runner/src/inspect.rs:210` --
  `flavor_tag(F::FnPtrShim) => "FnPtrShim"`.
- `crates/bridge-coverage-runner/src/inspect.rs:820-839` --
  `InspectionPredicate::UnOp` dispatch.
- (The implementer verifies analogous dispatch arms for
  `InspectionPredicate::AggregateKind` and
  `InspectionPredicate::FunctionInstanceFlavor` by reading
  inspect.rs end-to-end. If any of the three predicates' dispatch
  arms are missing, the implementer escalates to the user with
  the inspect.rs evidence rather than silently adding a new arm;
  see Acceptance Criterion 16.)

### Matrix doc (read-only)

- `docs/discussions/regseq-model/bridge-coverage-matrix.md:282` --
  RV-007-C row.
- `docs/discussions/regseq-model/bridge-coverage-matrix.md:294` --
  RV-009-G row.
- `docs/discussions/regseq-model/bridge-coverage-matrix.md:398` --
  FF-005 row.

### Prior tasks

- regseq-driver-67 (`c8c16f3`): F4 corpus close; escalated
  RV-007-C / RV-009-G / FF-005.
- regseq-driver-68 (`c3456a5` + `4e14bb1`): F68 post-F4 triage;
  proposed A1.
- regseq-driver-69 (`660bc7f`): D1 close (per-fixture build-flag
  override); sets the spec → Codex → file pattern + F69 sidecar
  baseline (125 rows).

## Implementation Notes

### Change A: extend the bridge's type-translation entry to normalize the two authorized projection families

**File**: `crates/regseq-driver/src/bridge/ty.rs`.

The fix attaches a node-local projection-normalization step at
the entry of `translate_erased_ty` (`:47-60`), BEFORE the call to
`translate_erased_ty_inner` and BEFORE the `ty_forward_cache`
insertion. This insertion point is load-bearing:

- The substantive-HRTB check in `erase_regions_or_reject`
  (`:35-45`) still fires on the un-normalized type at
  `translate_ty` (`:23-29`) entry, preserving current HRTB
  semantics.
- Placing the normalization at `translate_erased_ty` entry (NOT
  at `translate_erased_ty_inner` entry) keeps the `ty_forward_cache`
  consistent: the cache at `:58` stores `(TyId, erased rustc Ty)`
  pairs that Phase-5 body materialization reads at
  `crates/regseq-driver/src/bridge/body/materialise.rs:621-670`
  to reconstitute rustc generic args. If normalization happened
  inside `_inner`, the inner would produce a TyId derived from
  the normalized ty while the outer cached the un-normalized ty,
  corrupting Phase-5 reconstruction. Normalizing at the outer's
  entry ensures both the produced TyId AND the cached rustc Ty
  reflect the same (post-normalization) input.

Sketch:

```rust
fn translate_erased_ty<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    ty: rustc_middle::ty::Ty<'tcx>,
    allow_unsized_marker: bool,
) -> Result<TyId, BridgeError> {
    // Node-local normalization of the two authorized projection
    // families. Other TyKind::Alias projections are preserved as-is
    // so they continue to short-circuit at the existing
    // `TyKind::Alias` arm in `translate_erased_ty_inner` (`:296`).
    let ty = normalize_authorized_projections(cx.input.tcx, ty);

    let ty_id = translate_erased_ty_inner(cx, ty, allow_unsized_marker)?;
    cx.ty_forward_cache.entry(ty_id).or_insert(ty);
    Ok(ty_id)
}
```

#### Helper signature and semantics

```rust
fn normalize_authorized_projections<'tcx>(
    tcx: rustc_middle::ty::TyCtxt<'tcx>,
    ty: rustc_middle::ty::Ty<'tcx>,
) -> rustc_middle::ty::Ty<'tcx>
```

The helper is a **node-local custom `TypeFolder`** (or equivalent
structural rewriter), NOT a call to
`tcx.normalize_erasing_regions(typing_env, ty)` on the whole
type. The reason this matters:
`tcx.normalize_erasing_regions(value)` recursively folds EVERY
alias in `value` via the query normalizer once
`value.has_aliases()` is true (per
`compiler/rustc_middle/src/ty/normalize_erasing_regions.rs:57-61`,
`:186-188` under the active sysroot). That would normalize
unauthorized sibling projections too, silently widening the
bridge's effective alias-acceptance set in a way the spec
forbids.

The helper instead walks the type structurally. At every
`TyKind::Alias(AliasTyKind::Projection, alias_ty)` node, it
checks the projection's trait via a lang-item / diagnostic-item
match (the `alias_ty.def_id` resolves via `tcx.parent(...)` to
the trait `DefId`, and the trait is then identified by
`tcx.is_lang_item(trait_def_id, LangItem::FnOnce | Fn | FnMut)`
for the `Fn`-family `::Output` projections OR by a
diagnostic-item / lang-item match for `core::ptr::Pointee` on
the `Metadata` associated item):

- If the projection trait is authorized AND the alias
  corresponds to one of the supported pointee shapes for
  `Pointee::Metadata` (slice or sized -- see below for the
  scope decision on dyn) OR is a Fn-family `::Output` projection
  over a function pointer type, the helper produces the
  concrete normalized type for THAT node only. It calls
  `tcx.normalize_erasing_regions(TypingEnv::fully_monomorphized(),
  ty)` on the single alias node (NOT the containing type) so
  the query normalizer's "fold all" behavior is constrained to
  one node.
- Otherwise, the helper leaves the alias node alone and the
  walk continues into the alias's interior (so e.g. a
  `Foo<<T as OtherTr>::Item>` keeps its inner `OtherTr::Item`
  alias intact while still letting an outer authorized projection
  in `Foo<...>`'s instantiation get rewritten).

If the implementer prefers a simpler shape, an equivalent
implementation is allowed: walk the type, identify each
authorized alias node, replace each with the concrete normalized
type returned by `tcx.normalize_erasing_regions` applied to just
that node. The criterion is "no unauthorized alias is
normalized as a side effect"; any structural rewriter that
satisfies it is acceptable.

The trait-family identification uses rustc-source-grounded
identifiers (the implementer verifies these against the pinned
nightly's rustc source before committing):

- `LangItem::FnOnce` / `LangItem::FnMut` / `LangItem::Fn` for
  the Fn-family `::Output` projections. The fallback std
  source confirms the lang items at
  `library/core/src/ops/function.rs:242-250` (per Codex's
  round-1 review of this spec). The associated-item identity
  for `::Output` uses `#[lang = "fn_once_output"]`.
- For `core::ptr::Pointee`, the pinned std declares
  `#[lang = "pointee_trait"]` on the trait and
  `#[lang = "metadata_type"]` on the `Metadata` associated
  type (per `library/core/src/ptr/metadata.rs:17-28`, `:56-67`
  in the fallback source; the implementer verifies against
  the active sysroot's installed source if available, otherwise
  against the rustc-source git mirror).

#### Expected concrete normalization targets (authorized shapes only)

For the `Pointee::Metadata` family (per `core::ptr::Pointee`
impls in the pinned std at
`library/core/src/ptr/metadata.rs:17-28`):

- `<[T] as Pointee>::Metadata` → `usize`
- `<T as Pointee>::Metadata` for sized `T` → `()` (the empty
  tuple)
- `<dyn Trait as Pointee>::Metadata` → **NOT NORMALIZED BY A1**
  (preserved as-is by the helper; out of scope per the Out of
  scope bullet on dyn-Pointee::Metadata above). The helper's
  pointee-shape match is restricted to slice + sized.

For the `Fn` / `FnMut` / `FnOnce` family (per
`library/core/src/ops/function.rs:242-250` confirming
`type Output` and `Self::Output` return resolution; compiler
projection confirmation at
`compiler/rustc_trait_selection/src/traits/project.rs:1564-1583`,
`:1681-1697`):

- `<fn(A1, .., An) -> R as FnOnce<(A1, .., An)>>::Output` → `R`
- Analogously for `Fn::Output` / `FnMut::Output` over function
  pointer types.

These targets are the **standard library definitions** of the
two projections.
`tcx.normalize_erasing_regions(typing_env, alias_node)` is
expected to resolve them to exactly these concrete shapes when
called with `typing_env = TypingEnv::fully_monomorphized()`,
matching the bridge's existing pattern at
`bridge/body/materialise.rs:287-293` and
`bridge/mod.rs:783-788`. The implementer empirically verifies
each of the two `Pointee::Metadata` shapes and the Fn-family
shape actually produce the expected concrete type by inspecting
the runner's per-row report after Change C; any divergence
escalates to the user with the empirical evidence rather than
guessing fixes.

#### Hypothesis flags (the implementer must empirically verify)

1. **Lang-item / diagnostic-item identifier stability**: the
   precise rustc identifier used to match `Pointee::Metadata`
   (`#[lang = "pointee_trait"]` + `#[lang = "metadata_type"]`)
   and the Fn-family `::Output` (`#[lang = "fn_once_output"]`)
   may differ between rustc versions. The implementer verifies
   the chosen identifiers against the pinned nightly's rustc
   source (active sysroot first; fallback to the rustc-source
   git mirror if the active sysroot's `library/core` is not
   shipped) before committing.

2. **Pointee-shape match precision**: the helper must match
   `<T as Pointee>::Metadata` on `T` being `Slice(_)` or a
   sized non-dyn-non-slice type. The exact predicate for
   "sized non-dyn-non-slice" uses
   `T.is_sized(tcx, ...)` AND `!matches!(T.kind(), Slice(_) |
   Dynamic(_, _))`. The implementer empirically verifies the
   sized predicate works for the authorized snippet shapes
   (slice → usize on `[u32]`, sized → () on `u32`) and does
   NOT accidentally match `dyn Trait` (which IS unsized but
   the helper's match must short-circuit on the `Dynamic`
   kind first).

3. **`normalize_erasing_regions` semantics on a single alias
   node**: calling
   `tcx.normalize_erasing_regions(typing_env, alias_ty)` where
   `alias_ty: Ty<'tcx>` is a `TyKind::Alias` node is the
   expected entry point; the implementer verifies via rustc
   source that this path produces the concrete monomorphic
   type for the two authorized families. `try_normalize_erasing_regions`
   is a safer alternative for cases where the projection might
   not normalize; the implementer picks the simpler API that
   the call sites' typing-env supports per
   `compiler/rustc_middle/src/ty/normalize_erasing_regions.rs:34-94`.

If any hypothesis empirically fails, the implementer escalates
to the user with the empirical evidence rather than guessing
adjacent fixes.

### Change B: NO new helper / runner / model code

The runner's sidecar schema is unchanged (no new fields, no new
allow-list values, no new `InspectionPredicate` arms). Three new
rows + three new snippets are the only artifacts on the runner
side; see Change C.

### Change C: author three new sidecar rows that exercise the normalization

**File**: `crates/bridge-coverage-runner/snippets/RV-007-C.rs`.

The snippet exercises the two A1-authorized `Pointee::Metadata`
specializations (slice + sized). The dyn case is deliberately
excluded per Out of scope above. Both `core::ptr::metadata` and
`core::ptr::from_raw_parts` are unstable per the pinned std
(`library/core/src/ptr/metadata.rs`); the snippet opts into the
`ptr_metadata` unstable feature gate:

```rust
#![feature(ptr_metadata)]

// Slice specialization: <[T] as Pointee>::Metadata normalizes
// to `usize`. UnOp::PtrMetadata fires here.
pub fn meta_slice(p: *const [u32]) -> usize {
    core::ptr::metadata(p)
}

// Sized specialization: <T as Pointee>::Metadata for sized T
// normalizes to `()`. UnOp::PtrMetadata fires here.
pub fn meta_sized(p: *const u32) {
    let _m: () = core::ptr::metadata(p);
}
```

The runner's `inspect = { kind = "UnOp", arg = "PtrMetadata" }`
predicate matches ANY occurrence of `UnOp::PtrMetadata` in the
translated body. Either of the two function bodies above
satisfies the predicate after Change A lands, so both
functions present together provide additional coverage at no
extra inspection cost. The implementer MUST NOT add a dyn
function to this snippet; doing so would reintroduce the
Foreign hazard and cause the row to FAIL.

**File**: `crates/bridge-coverage-runner/snippets/RV-009-G.rs`.

Exercises `AggregateKind::RawPtr` via `core::ptr::from_raw_parts`
on the slice and sized specializations. Same unstable-feature
gate as RV-007-C:

```rust
#![feature(ptr_metadata)]

pub fn raw_slice(data: *const u32, len: usize) -> *const [u32] {
    core::ptr::from_raw_parts(data, len)
}

pub fn raw_sized(data: *const u32) -> *const u32 {
    core::ptr::from_raw_parts(data, ())
}
```

**File**: `crates/bridge-coverage-runner/snippets/FF-005.rs`.

Exercises the `Fn::call` resolution path that produces a
FnPtrShim instance. The F4 evidence at
`.harness/expectations/regseq-driver-67.md:303-318` tried three
shapes; the simplest is the bare `caller<F: Fn()>(f: F)` shape:

```rust
pub fn target() {}

pub fn caller<F: Fn()>(f: F) {
    f();
}

pub fn entry() {
    let fp: fn() = target;
    caller(fp);
}
```

(The implementer verifies that this shape produces a
`FunctionInstanceFlavor::FnPtrShim` in the bridge's translated
output AFTER the normalization lands. If the FnPtrShim instance
is elided by rustc -- e.g., the `caller` is inlined under
`mir_opt_level=0` -- the implementer escalates to the user with
the per-shape outcome and probes the other two F4-documented
shapes before guessing further.)

**File**: `crates/bridge-coverage-runner/matrix_metadata/rows.toml`.

Append three `[[row]]` entries. The implementer reads
`docs/discussions/regseq-model/bridge-coverage-matrix.md:282`,
`:294`, `:398` for the canonical `section` and `construct`
values; representative entries:

```toml
[[row]]
id = "RV-007-C"
section = "<from matrix doc>"
construct = "UnOp::PtrMetadata"
expectation = "positive"
status = "covered"
cluster_key = "RV-007"
snippet_path = "RV-007-C.rs"
inspect = { kind = "UnOp", arg = "PtrMetadata" }

[[row]]
id = "RV-009-G"
section = "<from matrix doc>"
construct = "AggregateKind::RawPtr"
expectation = "positive"
status = "covered"
cluster_key = "RV-009"
snippet_path = "RV-009-G.rs"
inspect = { kind = "AggregateKind", arg = "RawPtr" }

[[row]]
id = "FF-005"
section = "<from matrix doc>"
construct = "FunctionInstanceFlavor::FnPtrShim"
expectation = "positive"
status = "covered"
cluster_key = "FF-005"
snippet_path = "FF-005.rs"
inspect = { kind = "FunctionInstanceFlavor", arg = "FnPtrShim" }
```

No `build_overrides` field on any of the three rows; the F69
override surface is not used here (the rejections are bridge-side
projection failures, not rustc-side codegen flag gates).

### Change D: NO bridge changes outside `ty.rs` (reaffirmed)

The fix is intentionally scoped to one file (`bridge/ty.rs`).
This restates the corresponding Out of scope bullet: no other
bridge module is touched. If empirical verification surfaces a
need to touch a second bridge module (e.g., a `bridge/body/rvalue.rs`
arm that needs to handle the post-normalization type
differently), the implementer escalates to the user rather than
expanding scope unilaterally.

## Non-Goals

- Do NOT touch model / spec / matrix / strategy / pretty-printer /
  generic-c-backend.
- Do NOT widen the projection-normalization surface beyond
  `Pointee::Metadata` + `Fn` / `FnMut` / `FnOnce` `::Output`.
  Other `TyKind::Alias` projection families remain rejected at
  the existing `bridge/ty.rs:296` short-circuit. Adding a third
  family belongs to a separate task with its own approval gate.
- Do NOT add new `InspectionPredicate` dispatch arms. All three
  required arms (`UnOp("PtrMetadata")`,
  `AggregateKind("RawPtr")`,
  `FunctionInstanceFlavor("FnPtrShim")`) already exist at
  HEAD per Input Context.
- Do NOT modify the helper (`crates/test-ir-fixtures/`). The
  F69 override surface is not used by A1's three rows.
- Do NOT modify the runner schema (`Row`, `RowBuildOverrides`,
  `MatrixError`, `InspectionPredicate`). Three new
  `[[row]]` entries + three new snippet files are the only
  runner-side artifacts.
- Do NOT re-attempt other F4 escalations.
- Do NOT modify `tests/fixtures/playground/`.
- Do NOT add new dependencies to any crate's Cargo.toml.
- Do NOT add hand-rolled projection resolution. Use a
  rustc-provided normalization API.
- Do NOT widen the bridge's `Foreign` handling at
  `bridge/ty.rs:103-108` if `DynMetadata<dyn Trait>`
  empirically surfaces a `Foreign` error. Escalate to the user
  instead.

## Test Plan

### B1: workspace stays green

`RUST_TEST_THREADS=1 cargo test --workspace -- --test-threads=1`
green. Existing tests pass without modification.

### B2: bridge-side normalization unit tests

Add unit tests under `crates/regseq-driver/tests/bridge_ty.rs`
(or a new sibling file `bridge_projection_normalization.rs` if
the implementer prefers; both are conventional locations). Tests
cover:

1. **Slice Pointee::Metadata**: a simple `fn` that uses
   `core::ptr::metadata::<[u32]>(p)` -- bridge translates to a
   body whose UnOp::PtrMetadata rvalue produces a
   `Ty::Uint(UintKind::Usize)` result type. The test source uses
   `#![feature(ptr_metadata)]`.
2. **Sized Pointee::Metadata**: a simple `fn` that uses
   `core::ptr::metadata::<u32>(p)` -- bridge produces a
   `Ty::Tuple { fields: [], .. }` result type (the unit type).
3. **Fn-family Output**: a simple `fn` that exercises
   `<fn() as FnOnce<()>>::Output` via `caller<F: Fn()>(f: F)` --
   bridge produces a body whose return type resolves to `()`
   (the unit fn-pointer's return type).
4. **Negative-case sanity (unauthorized projection)**: a `fn`
   that uses a non-authorized projection. The implementer picks
   a shape whose `TyKind::Alias` does NOT match
   `Pointee::Metadata` or any Fn-family `::Output` (e.g., a
   custom-trait associated-type projection
   `trait Tr { type Out; } fn f<T: Tr>() -> <T as Tr>::Out`).
   The bridge MUST still reject with
   `NonRigidTy { kind: NonRigidTyKind::Alias, .. }`. This guards
   against the normalization being accidentally too broad and
   against `tcx.normalize_erasing_regions` folding non-authorized
   alias siblings as a side effect of a containing-type
   normalization call.
5. **Dyn-trait Pointee::Metadata (regression sentinel,
   non-authorized)**: a `fn` that uses
   `core::ptr::metadata::<dyn Tr>(p)`. The bridge MUST still
   reject the dyn case (with `Foreign` on
   `core::ptr::metadata::VTable` per the F4 evidence at
   `.harness/expectations/regseq-driver-67.md:243-244`, or with
   `NonRigidTy` if rustc's machinery differs in this context).
   The test asserts that the outcome is a rejection (variant is
   `Foreign` OR `NonRigidTy`); the test does NOT assert
   `PASS`. This guards against A1 silently expanding the
   bridge's behavior on the dyn case beyond its current
   rejection.

### B3: runner end-to-end Mode A with the three new rows

Manual Mode A invocation:

```
target/debug/bridge-coverage-runner \
  --metadata crates/bridge-coverage-runner/matrix_metadata/rows.toml \
  --snippets crates/bridge-coverage-runner/snippets/ \
  --matrix-md docs/discussions/regseq-model/bridge-coverage-matrix.md \
  --out /tmp/brcr-a1/
```

Expects: exit 0, `Total = 128`, `Pass = 128`, `Fail = 0`.

### B4: per-row PASS verification

The runner's per-row report for RV-007-C, RV-009-G, and FF-005
MUST all show `PASS`:

- RV-007-C: positive, Ok outcome, inspection `UnOp("PtrMetadata")`
  matches.
- RV-009-G: positive, Ok outcome, inspection
  `AggregateKind("RawPtr")` matches.
- FF-005: positive, Ok outcome, inspection
  `FunctionInstanceFlavor("FnPtrShim")` matches.

### B5: no regression on the existing 125 rows

The per-row PASS/FAIL classification for the 125 pre-F70 rows
is identical to the F69 close (commit `660bc7f`) state. Spot
sentinels: TY-018, IK-002, TM-006-C, RJ-006, RJ-024, RJ-022.
The implementer compares against the F69 report at
`/tmp/d1-mode-a/report.md` (the assessor's snapshot from F69
close) OR a fresh pre-Change-A Mode A run. The normalization
extension MUST NOT change the outcome of any pre-existing row;
in particular, the negative-case projection rejections (e.g.,
RJ-022 substantive-HRTB rejections) must still fire.

### B6: drift check still passes

The matrix sidecar drift check (subset-OK) reports clean: all
three new rows are documented in the matrix doc (`:282`,
`:294`, `:398`). No new orphan rows; no missing matrix entries.

## Review

### Files to send to Codex (`scripts/python/codex_runner.py`)

- This file
  (`.harness/tasks/regseq-driver/70-bridge-associated-type-projection-normalization.md`).
- The F68 audit doc
  (`.harness/audits/2026-05-21-post-f4-triage.md`) -- A1
  proposal source.
- The F4 expectations file
  (`.harness/expectations/regseq-driver-67.md`) -- RV-007-C +
  RV-009-G + FF-005 empirical evidence.
- The bridge type-translation source
  (`crates/regseq-driver/src/bridge/ty.rs`).
- The bridge body materialisation source for the existing
  `normalize_erasing_regions` usage pattern
  (`crates/regseq-driver/src/bridge/body/materialise.rs`).
- The bridge top-level source for the other existing usage site
  (`crates/regseq-driver/src/bridge/mod.rs`).
- The F3 dispatch surface
  (`crates/bridge-coverage-runner/src/inspect.rs`).

### Reviewer focus

- **Normalization-surface narrowness**: the fix is restricted to
  `Pointee::Metadata` + `Fn` / `FnMut` / `FnOnce` `::Output`. Is
  the spec's enforcement story (lang-item match in the helper
  function, no fallthrough to other Alias families) coherent and
  sufficiently defended against accidental widening?
- **Empirical evidence chain**: are the three normalization
  targets (slice → usize and sized → () for Pointee::Metadata;
  fn-ptr return for Fn-family Output) empirically supported by
  the pinned-nightly std source? Verify by reading
  `library/core/src/ptr/metadata.rs` (slice / sized impls and
  the `#[lang = "pointee_trait"]` / `#[lang = "metadata_type"]`
  declarations) and `library/core/src/ops/function.rs` (the
  `#[lang = "fn_once_output"]` Output associated type)
  under the active sysroot if available, otherwise via the
  rustc-source git mirror. The dyn → DynMetadata target is
  EXPLICITLY NOT in A1; Codex verifies the spec's exclusion
  story holds.
- **Dyn exclusion is enforced**: the spec excludes
  dyn-Pointee::Metadata via the Out of scope bullet on dyn, the
  helper's pointee-shape match restriction (Hypothesis 2 of
  Change A), and the regression sentinel in Test Plan B2 case 5.
  Codex verifies all three are consistent and that the helper
  cannot accidentally normalize the dyn case.
- **Node-local normalization**: does the helper's structural
  walk normalize ONLY the authorized alias node, not the
  containing type? `tcx.normalize_erasing_regions(value)`
  recursively folds every alias in `value` once
  `value.has_aliases()` is true (per
  `rustc_middle/src/ty/normalize_erasing_regions.rs:57-61`,
  `:186-188`). The helper must therefore either (a) apply
  normalization to a single alias node `Ty<'tcx>`, or (b) use a
  custom folder that explicitly skips unauthorized alias nodes.
  Test Plan B2 case 4 guards against accidental over-normalization.
- **`normalize_erasing_regions` semantics over a single alias
  node**: does
  `tcx.normalize_erasing_regions(typing_env, alias_node)`
  resolve the projection to the expected concrete type when the
  typing-env is `TypingEnv::fully_monomorphized()` and
  `alias_node: TyKind::Alias`? Verify via rustc source. The
  spec hedges between `normalize_erasing_regions` and
  `try_normalize_erasing_regions` per Hypothesis 3 of Change A.
- **Insertion-point correctness**: the normalization is at
  `translate_erased_ty` entry (NOT `_inner` entry). Codex
  verifies (1) the HRTB check at `erase_regions_or_reject`
  (`:39-43`) still fires on the un-normalized type at
  `translate_ty` entry; (2) the `ty_forward_cache` at
  `translate_erased_ty:58` stores the post-normalization ty
  matching the produced TyId, preserving the Phase-5
  reconstruction invariant at
  `crates/regseq-driver/src/bridge/body/materialise.rs:621-670`;
  and (3) no other Phase-1 / Phase-2 invariant requires the
  pre-normalized type to reach `translate_erased_ty_inner`.
- **InspectionPredicate coverage**: the spec asserts the three
  required arms already exist at HEAD. Codex re-verifies the
  three citations
  (`inspect.rs:352`, `:281`, `:210`) against HEAD (`660bc7f`).
- **Backward compatibility**: existing callers of `translate_ty`,
  `translate_erased_ty`, and the inner workhorse continue to
  work without modification. The signature of
  `translate_erased_ty_inner` does not change.
- **Citation drift at HEAD**: every `file:line` against HEAD
  (`660bc7f`).
- **Off-by-N row count math**: 125 (F69 close at `660bc7f`) + 3
  (A1 new rows) = 128 total. Verify the math.
- **No new dependencies**: neither
  `crates/regseq-driver/Cargo.toml` nor any other crate's
  `Cargo.toml` gains a new dependency.
- **No new helper / runner / model surface**: F69's `BuildOverrides`
  + `RowBuildOverrides` + `MatrixError::UnknownZFlag` etc. are
  not touched. No new InspectionPredicate arms.
- **Escalation paths**: every "escalates to the user" clause
  names the user explicitly and carries an empirical trigger
  condition.

### Adversarial prompt template

```
<task>
Review .harness/tasks/regseq-driver/70-bridge-associated-type-projection-normalization.md.

This is the proposed task spec for A1 from the F68 post-F4 triage.
It extends the bridge's type-translation entry to normalize two
associated-type projection families (`<T as Pointee>::Metadata`
and `<F as Fn/FnMut/FnOnce<Args>>::Output`) before the
`TyKind::Alias` short-circuit at `crates/regseq-driver/src/bridge/ty.rs:296`
fires, then authors three new sidecar rows (RV-007-C, RV-009-G,
FF-005) to unblock three F4 escalations.

Identify defects under the following adversarial checks:

1. NORMALIZATION-SURFACE NARROWNESS: Is the fix restricted to the two authorized projection families (`Pointee::Metadata` for slice + sized only; `Fn` / `FnMut` / `FnOnce` `::Output`) with no fallthrough to other Alias families? Does the spec require lang-item / diagnostic-item / associated-item identity matches in the helper rather than name-based string matches? Does the helper normalize node-locally (per-alias-node) rather than recursively folding the whole containing type?

2. EMPIRICAL EVIDENCE CHAIN: For each authorized normalization target (slice→usize, sized→() for Pointee::Metadata; fn-ptr return for Fn-family Output), is the claim supported by the pinned-nightly std source? Verify by reading `library/core/src/ptr/metadata.rs` and `library/core/src/ops/function.rs` under the active sysroot if available, otherwise via the rustc-source git mirror. Verify the lang-item / diagnostic-item names (`pointee_trait`, `metadata_type`, `fn_once_output`).

3. DYN EXCLUSION COHERENCE: Is dyn-Pointee::Metadata consistently excluded? Cross-check the Out of scope bullet on dyn, the Description's two-rejection-modes wording, the helper's pointee-shape match (Hypothesis 2), Acceptance Criterion 1 and 15, and Test Plan B2 case 5 (the regression sentinel). Does any wording risk the helper accidentally normalizing the dyn case?

4. NORMALIZATION API SEMANTICS: Does `tcx.normalize_erasing_regions(TypingEnv::fully_monomorphized(), alias_node)` resolve a `TyKind::Alias` to the expected concrete type when called on a single alias node? Verify against rustc source. Flag if `try_normalize_erasing_regions` is the safer choice.

5. INSERTION-POINT CORRECTNESS + CACHE INVARIANT: The spec inserts normalization at `translate_erased_ty` entry (NOT `_inner` entry) so the `ty_forward_cache` at `:58` stores the post-normalization ty matching the produced TyId. Codex verifies (a) the HRTB check at `erase_regions_or_reject` still fires on the pre-normalized type at `translate_ty` entry; (b) the cache stays consistent with Phase-5 reconstruction at `bridge/body/materialise.rs:621-670`; (c) no other invariant requires the pre-normalized type to reach `_inner`.

6. INSPECTIONPREDICATE COVERAGE: Verify the three cited arms exist at HEAD: `UnOp("PtrMetadata")` at `inspect.rs:352`, `AggregateKind("RawPtr")` at `:281`, `FunctionInstanceFlavor("FnPtrShim")` at `:210`. Confirm the analogous dispatch arms (not just the tag functions) exist by reading inspect.rs end-to-end.

7. BACKWARD COMPATIBILITY: Do existing callers of `translate_ty`, `translate_erased_ty`, `translate_erased_ty_inner` continue to work? Flag any subtle behavioral change. In particular, does the cache change alter Phase-5 reconstruction outcomes for any pre-existing row?

8. CITATION DRIFT AT HEAD: Every file:line anchor against HEAD (660bc7f).

9. OFF-BY-N ROW COUNT: 125 + 3 = 128. Verify the F69 close at 660bc7f actually had 125 rows in rows.toml.

10. SCOPE CREEP: Does the spec implicitly invite scope creep (e.g., "implementer may widen the normalization surface", "implementer may modify the Foreign check", discretionary row splitting) in any phrasing that should be tightened? Verify the "NO bridge changes outside ty.rs" wording is consistent across Out of scope, Non-Goals, Quality Gate.

11. ESCALATION PATHS: Does every "implementer escalates" clause name the user explicitly and carry an empirical trigger condition?

12. SNIPPET SHAPE FEASIBILITY: Do the candidate snippet shapes (RV-007-C two-function slice+sized with `#![feature(ptr_metadata)]`, RV-009-G two-function slice+sized with the same feature gate, FF-005 `caller<F:Fn()>` shape) actually exercise the predicates they claim to exercise under rustc nightly-2026-02-01? Flag any shape that the bridge / rustc may collapse or elide before the predicate fires. Confirm the feature gate is required (`core::ptr::metadata` / `from_raw_parts` are unstable in the pinned std).
</task>

<grounding_rules>
- Cite file:line for every claim. The format `path/to/file.md:NNN-MMM` is the expected style.
- For each cited file:line in the task spec, open the file and verify the citation. Quote the source, do not paraphrase the spec's claim.
- For rustc-source claims, read under the active sysroot at
  `~/.rustup/toolchains/nightly-2026-02-01-aarch64-apple-darwin/lib/rustlib/rustc-src/`
  (`rust/compiler/` for compiler crates;
  `rust/library/core/` for std).
- Mark hypotheses as hypotheses. If you suspect a defect but cannot prove it without empirical runner execution, say so explicitly.
- Pinned toolchain is `nightly-2026-02-01`; HEAD is `660bc7f`.
</grounding_rules>

<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
NORMALIZATION-SURFACE NARROWNESS: PASS | FAIL with cite
EMPIRICAL EVIDENCE CHAIN: PASS | FAIL with per-defect cite
DYN EXCLUSION COHERENCE: PASS | FAIL with cite
NORMALIZATION API SEMANTICS: PASS | FAIL with cite
INSERTION-POINT CORRECTNESS + CACHE INVARIANT: PASS | FAIL with cite
INSPECTIONPREDICATE COVERAGE: PASS | FAIL with cite
BACKWARD COMPATIBILITY: PASS | FAIL with cite
CITATION DRIFT AT HEAD: list each drifted anchor or "none"
OFF-BY-N ROW COUNT: PASS | FAIL with cite
SCOPE CREEP: PASS | FAIL with cite
ESCALATION PATHS: PASS | FAIL with cite
SNIPPET SHAPE FEASIBILITY: PASS | FAIL with per-snippet cite
NEW ISSUES: numbered list or "none"
REQUIRED FIXES: numbered list (only if VERDICT != APPROVED)
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, decide, stop.
</default_follow_through_policy>
```

## Quality Gate

- `cargo fmt --all -- --check` clean.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `RUST_TEST_THREADS=1 cargo test --workspace -- --test-threads=1`
  green.
- Manual Mode A invocation of the runner produces
  `Total = 128`, `Pass = 128`, `Fail = 0`, exit 0.
- `git diff --stat main..` shows changes confined to:
  - `crates/regseq-driver/src/bridge/ty.rs` (Change A).
  - `crates/regseq-driver/tests/bridge_ty.rs` (Test Plan B2;
    or a new sibling test file).
  - `crates/bridge-coverage-runner/matrix_metadata/rows.toml`
    (Change C; +3 rows).
  - `crates/bridge-coverage-runner/snippets/RV-007-C.rs`
    (Change C).
  - `crates/bridge-coverage-runner/snippets/RV-009-G.rs`
    (Change C).
  - `crates/bridge-coverage-runner/snippets/FF-005.rs`
    (Change C).
- No model / spec / matrix / strategy / pretty-printer /
  generic-c-backend changes.
- No helper / runner schema changes.
- No new `InspectionPredicate` arms.
- No new dependencies in any crate's Cargo.toml.
- Codex Task Review verdict: APPROVED or
  APPROVED_WITH_MINOR_FIXES.

## Acceptance Criteria

1. The bridge's type-translation entry normalizes
   `<T as core::ptr::Pointee>::Metadata` to the concrete
   monomorphic type for the two A1-authorized pointee shapes
   (`usize` for slice; `()` for sized) before the
   `TyKind::Alias(..)` short-circuit at
   `crates/regseq-driver/src/bridge/ty.rs:296` fires. The dyn
   pointee shape is NOT in A1's scope; the bridge's existing
   behavior on `<dyn Trait as Pointee>::Metadata` is preserved
   unchanged.
2. The bridge's type-translation entry normalizes
   `<F as core::ops::FnOnce<Args>>::Output` (and the `Fn` /
   `FnMut` analogues) to the function-pointer's return type
   before the same short-circuit.
3. Normalization is restricted to the two authorized projection
   families (lang-item / diagnostic-item match, not string
   match); other `TyKind::Alias` projections continue to
   short-circuit as `NonRigidTy { kind: NonRigidTyKind::Alias, .. }`
   at the existing `:296` arm (verified by B2 case 4).
4. The fix uses a rustc-provided normalization API
   (`tcx.normalize_erasing_regions` or
   `tcx.try_normalize_erasing_regions`); no hand-rolled
   projection resolution.
5. RV-007-C snippet + sidecar row land; the runner reports
   PASS against `inspect = { kind = "UnOp", arg = "PtrMetadata" }`.
6. RV-009-G snippet + sidecar row land; the runner reports
   PASS against `inspect = { kind = "AggregateKind", arg = "RawPtr" }`.
7. FF-005 snippet + sidecar row land; the runner reports PASS
   against `inspect = { kind = "FunctionInstanceFlavor", arg = "FnPtrShim" }`.
8. `Total = 128, Pass = 128, Fail = 0` after the runner's
   Mode A invocation.
9. Existing 125 rows continue to PASS with the same per-row
   classification as the F69 close (no regression). In
   particular, every previously-rejected row that hits a
   non-authorized `TyKind::Alias` projection continues to
   reject with `NonRigidTy { kind: Alias, .. }` (verified by
   the per-row PASS/FAIL diff against the F69 report).
10. `cargo fmt --all -- --check`,
    `cargo clippy --workspace -- -D warnings`,
    `RUST_TEST_THREADS=1 cargo test --workspace -- --test-threads=1`
    all green.
11. Codex Task Review verdict: APPROVED or
    APPROVED_WITH_MINOR_FIXES.
12. No model / spec / matrix / strategy / pretty-printer /
    generic-c-backend file modifications.
13. No helper / runner schema changes (no new `Row` field, no
    new `RowBuildOverrides` field, no new `MatrixError` variant,
    no new `InspectionPredicate` arm).
14. No new dependencies in any crate.
15. The `<dyn Trait as Pointee>::Metadata` case is NOT
    normalized by A1 (the helper's pointee-shape match excludes
    `Dynamic` per Hypothesis 2 of Change A). The bridge's
    behavior on the dyn case remains unchanged from F69 baseline;
    test plan B2 case 5 verifies the dyn case still rejects.
16. If any of the three F3 dispatch arms required by Acceptance
    Criteria 5--7 does not exist at HEAD (the spec asserts all
    three do; the implementer verifies), the implementer
    escalates to the user with the inspect.rs evidence rather
    than silently adding a new arm.
17. Every other empirical hypothesis flagged in Change A
    (lang-item identifier choice, `normalize_erasing_regions`
    semantics over Alias) is verified during implementation;
    divergences from the spec's expected outcome escalate to
    the user with empirical evidence.
