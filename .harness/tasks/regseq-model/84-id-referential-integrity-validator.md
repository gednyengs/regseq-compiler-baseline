# regseq-model-84: ID referential integrity validator + interner uniqueness + F4 / F4-2 sort + DefPath / span / vtables (P1a-VAL-02)

## Description

Extend the existing `RegseqModel::validate` walk at
`crates/regseq-model/src/model.rs:200-449` so it covers the full set of
Phase B referential-integrity, content-keyed-uniqueness, ordering,
rendering, span-bounds, and Phase C `vtables` rebuild checks pinned by
`spec.md` `§3.1` and `§3.3`. The existing walk only enforces a stage-1
subset (per the per-invariant comments in the file); this task closes
the remainder pinned to the data-shape surface that ships in
P1a-DS-01..P1a-DS-10.

The validator must implement, in `§3.3` Phase B order:

- `§3.1` invariants 1, 2, 6, 9, 11, 18 (full coverage; the partial
  stage-1 coverage is replaced).
- `§16.1` F4 sort comparator output check on `items` (uses the
  `def_path_data_sort_rank` table at `spec.md:6031-6053`).
- `§16.3` F4-2 `RegseqInstanceKey` sort output check on `instances`.
- `§3.3` `DefPath.rendered` matches canonical rendering
  (`spec.md:1057-1062`).
- `§3.3` source-span bounds, sentinel-aware (`spec.md:1093-1102`).
- `§3.3` Phase C `vtables` rebuild from
  `item_synthetic_kinds[id] == VTableType { principal }` with
  duplicate-`PrincipalTraitKey` rejection (`spec.md:1104-1113`).

## Input Context

- `docs/discussions/regseq-model/spec.md` `§3.1` invariants 1, 2, 6, 9,
  11, 18 (lines 426-447, 466-467, 474-479, 491-497, 549-555).
- `docs/discussions/regseq-model/spec.md` `§3.3` Phase B (lines
  676-1102) and Phase C (lines 1104-1113).
- `docs/discussions/regseq-model/spec.md` `§16.1` (lines 6004-6058) and
  `§16.3` (lines 6071-6085).
- `docs/discussions/regseq-model/spec.md` `§7.2` (`DefPath` rendering)
  for the canonical-render reference.
- `docs/discussions/regseq-model/spec.md` `§5` (Source files and
  spans) for span-bounds semantics.
- `crates/regseq-model/src/model.rs:140-449` -- existing walk to
  extend.
- `crates/regseq-model/src/def_path.rs` -- `DefPath` / `DefPathHash` /
  `render_def_path` (already shipped at stage 1 / stage 12).
- `Item`, `ItemKind`, `Item.def_path` from regseq-model-72 (P1a-DS-07)
  and the `SyntheticItemKind` / `PrincipalTraitKey` from
  regseq-model-74 (P1a-DS-08b).
- `Ty` / `TyInterner` / `TyIdentityKey` from regseq-model-67 +
  regseq-model-69 (P1a-DS-02 + P1a-DS-04).
- `FnSig` / `FnSigInterner` from regseq-model-71 (P1a-DS-06).
- `Const` / `ConstInterner` from regseq-model-70 (P1a-DS-05).
- `Instance` / `InstanceKind` / `RegseqInstanceKey` /
  `FunctionInstanceFlavor` from regseq-model-75 (P1a-DS-10).
- `archive/.harness/tasks/regseq-model/24-place-and-projection.md` as
  the SHAPE / SIZE reference.

## Implementation Notes

The existing `validate(&self) -> Result<(), InvariantViolation>` is
extended in place; the canonical Phase B / Phase C ordering pinned at
`spec.md:676-1119` is the authoritative sequence (do not reorder).

### Invariant 1 -- IDs resolve (full closure)

Walk every ID-bearing field across `crates`, `source_files`, `items`
(including `Item.def_path.crate_id` and any nested IDs in the now-real
`ItemKind`), `item_def_paths` (each `DefPathHash` is value-only -- no
ID resolution), `item_synthetic_kinds` (resolves nested IDs in
`PrincipalTraitKey` per `§13.3`), `types`, `consts`, `sigs`,
`sig_c_abi_lowerings`, and `instances`. The existing per-Instance
helpers at `crates/regseq-model/src/model.rs:326-449` are reused; new
helpers are added for the now-non-placeholder Item / Ty / Const / FnSig
content. Dangling IDs surface as the existing
`InvariantViolation::DanglingId { field, value, table_len }`.

### Invariant 2 -- mirror lengths

Per `spec.md:433-447`: extend the existing length check to also assert
`item_synthetic_kinds.len() == items.len()`. Add
`InvariantViolation::ItemSyntheticKindsLengthMismatch { items_len,
synthetic_kinds_len }`.

### Invariant 6 -- every `DefPath.crate_id` resolves

Per `spec.md:466-467` and `§3.3` step 3 (c). For every
`item.def_path.crate_id`, assert `crate_id.index() < crates.len()`.
Surface as
`InvariantViolation::DefPathCrateIdOutOfBounds { item: ItemId,
crate_id: CrateId, crates_len: usize }`.

### Invariant 9 -- Instance closure-completeness post Phase A

Per `spec.md:474-479`: every `InstanceId` referenced **anywhere** in
the model must resolve. Walk every `InstanceId` slot across `Const`
(`Reloc(InstanceRef)`, `Bytes::Reloc(InstanceRef)`),
`item_synthetic_kinds` (`VTableInstance { .. }`,
`VTableMethodShim { .. }`, etc.), and any `Body` content if the
instance carries one (delegated downstream; for VAL-02, the
data-shape surfaces are sufficient -- the bridge populates body
`InstanceId`s, so the integration test exercises constructed Body
content). Reject with
`InvariantViolation::DanglingId { field: "<...>", .. }`.

### Invariant 11 -- content-keyed interner uniqueness

Per `spec.md:491-497`. For each of `types` / `sigs` / `consts`:

- `TyInterner`: walk `model.types`, recompute the `TyIdentityKey` per
  `§12.1`, and assert no two TyIds map to canonically-equal keys.
- `FnSigInterner`: same, over `model.sigs` per `§14.4`.
- `ConstInterner`: same, over `model.consts` per `§9.7`. Note that
  value-level `Const` recomputation depends on const-graph acyclicity
  (verified by P1a-VAL-03); VAL-02's `ConstInterner` content-key check
  short-circuits on `Aggregate` / `EnumValue` recursion if the cycle
  detector trips and yields a deferred-to-VAL-03 error.

Surface as `InvariantViolation::DuplicateInternerEntry { interner:
&'static str, first: u32, second: u32 }`.

### Invariant 18 -- `item_def_paths` global uniqueness

Per `spec.md:549-555`. HashSet sweep across `item_def_paths`; reject
the first duplicate with
`InvariantViolation::DuplicateItemDefPathHash { hash: DefPathHash,
first: ItemId, second: ItemId }`.

### F4 sort output check (`§16.1`)

Implement the `def_path_data_sort_rank` table verbatim per
`spec.md:6031-6053` (do **not** invent shortcuts; the table is
append-only and must match the spec line-for-line). For each adjacent
pair `(items[i], items[i + 1])`, compute the `(crate_name,
crate_stable_id, segments)` sort key per `spec.md:6010-6022` and
assert non-strict monotonicity (`key[i] <= key[i + 1]`); ties are
admitted per `spec.md:706-708`. Reject with
`InvariantViolation::F4OutOfOrder { first: ItemId, second: ItemId }`.

### F4-2 sort output check (`§16.3`)

For each adjacent pair `(instances[i], instances[i + 1])`, compute the
`RegseqInstanceKey = (regseq_kind_rank, item_id, generic_args_key,
fn_shim_disambig)` per `spec.md:6075-6080` and assert non-strict
monotonicity. Reject with
`InvariantViolation::F4OutOfOrderInstance { first: InstanceId, second:
InstanceId }`.

### `DefPath.rendered` canonical-rendering check (`§3.3`)

Per `spec.md:1057-1062`: for each `Item.def_path`, re-derive the
canonical rendered form via `render_def_path(crate_id, segments,
crates, strings)` (§7.2) and assert equality with the stored
`rendered: StringId`. Reject with
`InvariantViolation::DefPathRenderMismatch { item: ItemId, expected:
String, actual: String }`.

### Source-span bounds (`§3.3`, sentinel-aware)

Per `spec.md:1093-1102`: walk every `Span { file, lo, hi }` carried
inside `instances[..].span`, `Item.span` (when shipped), and any
`AttrData` payload that carries a `Span`. For each span:

- Assert `span.file.index() < model.source_files.len()`.
- For `SourceFile { content: Some(text), .. }`: assert
  `lo <= hi <= text.len()` and that both `lo` and `hi` lie at valid
  UTF-8 byte boundaries (use `text.is_char_boundary(lo)` and `..(hi)`).
- For `SourceFile { content: None, .. }` (sentinel /
  upstream-crate): accept without `lo`/`hi` content checks; still
  assert `line_starts` is monotonic and starts at 0 (one-shot
  precondition; not per-span).

Reject with
`InvariantViolation::SpanOutOfBounds { file: SourceFileId, lo: u32,
hi: u32, content_len: usize }` and
`InvariantViolation::SpanNotOnUtf8Boundary { file, lo, hi, end:
&'static str /* "lo" | "hi" */ }`.

### Phase C -- rebuild `vtables` with duplicate rejection

Per `spec.md:1104-1113`: scan `item_synthetic_kinds` for entries of
`SyntheticItemKind::VTableType { principal }`; rebuild
`RegseqModel.vtables: HashMap<PrincipalTraitKey, ItemId>` by inserting
`(principal, item_id)`. Reject duplicates with
`InvariantViolation::DuplicateVTablePrincipal { principal:
PrincipalTraitKey, first: ItemId, second: ItemId }`. The rebuilt map
is stored on `&mut RegseqModel` via a new
`fn rebuild_vtables(&mut self) -> Result<(), InvariantViolation>`
that the joint validator calls after Phase B; alternatively, the
rebuild is performed in-pass and stored back on `self` (the existing
`#[serde(skip)]` field on `RegseqModel.vtables` from P1a-DS-01 is the
target). The exact `&self` vs `&mut self` split is an implementation
choice; the spec only mandates that Phase C runs after Phase B and
rejects duplicates.

### Conventions

- All public types and functions carry doc comments; no unicode
  symbols (per `SESSION.md` "Coding Conventions").
- Per the user's "no defensive fallbacks" rule, the validator does
  not silently tolerate corrupt serialised models -- every check
  produces a typed `InvariantViolation` variant.
- The existing per-invariant comments at
  `crates/regseq-model/src/model.rs:200-231` are updated to reflect
  full coverage (no longer "deferred to stage 12 / 11").
- Phase B ordering is binding per `spec.md:676-680` ("Identity
  uniqueness in `TyInterner` (§12) depends on `FnSigId` and `ConstId`
  being content-unique (invariant 11), so those inner interners are
  validated first.").

## Non-Goals

- Place / projection structural and type-coherence checks per `§19.3`
  / `§19.4` / `§19.7` are owned by P1a-VAL-01 (regseq-model-83).
- Layout consistency, invariant 14 (InstanceKind layout matches Ty
  nominal variant), invariant 19 (`opaque_handle_eligible`),
  invariant 20 (`StableCrateId(0)` synthetic-domain), `UNIT_LAYOUT`
  check, target / build validation, FnSig / CAbiLowering checks,
  const-graph acyclicity, domain-attr validation per `§3.6.5`, and
  source-file validation are owned by P1a-VAL-03 (regseq-model-85).
- Bridge translation (rustc state -> RegseqModel) is owned by the
  P1a-BR-* axis.
- Pretty-printing is owned by the P1a-PP-* axis.

## Test Plan

Tests live in two tiers per the brief discipline (`unit-in-src` +
`integration-in-tests`). Each invariant the validator enforces gets
**both** a positive test (a model that passes) and a negative test (a
model that violates the invariant and is rejected with the right error
variant). Integration tests under
`crates/regseq-model/tests/id_referential_integrity_validator.rs`
build constructed-model fixtures using `RegseqModel::with_primary` +
push helpers.

### Invariant 1 -- IDs resolve

- `invariant_1_pass_full_closure`: a model whose every ID-bearing
  field resolves (Item / Ty / Const / FnSig / Instance / source-files
  / strings / synthetic kinds / sig_c_abi_lowerings) -- accepted.
- `invariant_1_reject_dangling_item_def_path_crate_id`: dangling
  CrateId on an Item -- rejected.
- `invariant_1_reject_dangling_ty_in_const_aggregate`: dangling TyId
  inside a `Const::NullPtr(t)` -- rejected.
- `invariant_1_reject_dangling_fn_sig_in_synthetic_kind`: dangling
  FnSigId inside a `SyntheticItemKind::VTableMethodShim { .. }` -- 
  rejected.

### Invariant 2 -- mirror lengths

- `invariant_2_pass`: `items.len() == item_def_paths.len() ==
  item_synthetic_kinds.len()` -- accepted.
- `invariant_2_reject_def_paths_short`: drop one `item_def_paths`
  entry -- rejected with `ItemDefPathsLengthMismatch`.
- `invariant_2_reject_synthetic_kinds_short`: drop one
  `item_synthetic_kinds` entry -- rejected with
  `ItemSyntheticKindsLengthMismatch`.

### Invariant 6 -- DefPath.crate_id resolves

- `invariant_6_pass`: every Item's `def_path.crate_id` is in range --
  accepted.
- `invariant_6_reject`: one Item's `def_path.crate_id` is out of
  bounds -- rejected with `DefPathCrateIdOutOfBounds`.

### Invariant 9 -- Instance closure-completeness

- `invariant_9_pass`: every `InstanceId` reached from `Const::Reloc`
  / `Bytes::Reloc` / `SyntheticItemKind::VTableInstance { .. }`
  resolves -- accepted.
- `invariant_9_reject_const_reloc`: `Const::Reloc(InstanceRef { .. })`
  whose projection misses the instances table -- rejected.
- `invariant_9_reject_synthetic_kind_vtable_instance`:
  `SyntheticItemKind::VTableInstance` referencing a missing
  InstanceId -- rejected.

### Invariant 11 -- content-keyed interner uniqueness

- `invariant_11_ty_interner_pass`: distinct entries in `model.types`
  whose `TyIdentityKey`s differ -- accepted.
- `invariant_11_ty_interner_reject_duplicate`: two distinct TyIds
  mapping to canonically-equal keys -- rejected with
  `DuplicateInternerEntry { interner: "TyInterner", .. }`.
- `invariant_11_fn_sig_interner_pass` / `_reject_duplicate`: same for
  `FnSigInterner`.
- `invariant_11_const_interner_pass` / `_reject_duplicate`: same for
  `ConstInterner` (scalar variants only -- value-level recursion
  defers to VAL-03's acyclicity pass).

### Invariant 18 -- `item_def_paths` global uniqueness

- `invariant_18_pass`: distinct `DefPathHash`es across all items --
  accepted.
- `invariant_18_reject`: two items mapping to the same
  `DefPathHash` -- rejected with `DuplicateItemDefPathHash`.

### F4 sort output check (`§16.1`)

- `f4_sort_pass_canonical_fixture`: items pre-sorted per the F4
  comparator (sample fixture: 5 items spanning at least 3 crates and
  at least 4 distinct `DefPathData` rank values) -- accepted.
- `f4_sort_reject_permuted_input`: a permutation of the canonical
  fixture (one adjacent swap that violates F4) -- rejected with
  `F4OutOfOrder { first, second }` pinning the offending pair.
- `f4_sort_pass_ties_admitted`: two adjacent items whose sort keys
  are exactly equal -- accepted (per `spec.md:706-708`).
- `f4_sort_pass_per_def_path_data_rank`: a fixture exercising every
  variant of `def_path_data_sort_rank` (`CrateRoot=0`, `Impl=1`, ...,
  `NestedStatic=18`) at least once -- accepted.

### F4-2 sort output check (`§16.3`)

- `f4_2_sort_pass`: instances pre-sorted by
  `RegseqInstanceKey` -- accepted.
- `f4_2_sort_reject`: an adjacent pair violating the
  `(regseq_kind_rank, item_id, generic_args_key, fn_shim_disambig)`
  ordering -- rejected with `F4OutOfOrderInstance`.

### `DefPath.rendered` canonical-rendering check

- `def_path_render_pass`: every `Item.def_path.rendered` matches the
  canonical re-derivation -- accepted.
- `def_path_render_reject_malformed`: one Item whose
  `def_path.rendered` does NOT match the canonical render -- rejected
  with `DefPathRenderMismatch { expected, actual }`.

### Source-span bounds

- `span_bounds_pass_content_some`: a Span over a `SourceFile { content:
  Some(text), .. }` with `lo <= hi <= text.len()` and both at UTF-8
  boundaries -- accepted.
- `span_bounds_reject_lo_gt_hi`: `lo > hi` -- rejected with
  `SpanOutOfBounds`.
- `span_bounds_reject_hi_gt_content_len`: `hi > text.len()` --
  rejected with `SpanOutOfBounds`.
- `span_bounds_reject_not_on_utf8_boundary_lo`: `lo` not at a UTF-8
  boundary in a multi-byte file -- rejected with
  `SpanNotOnUtf8Boundary { end: "lo" }`.
- `span_bounds_reject_not_on_utf8_boundary_hi`: same for `hi`.
- `span_bounds_pass_content_none_sentinel`: a Span over the
  synthesised sentinel (`SourceFile { content: None, .. }`) --
  accepted.
- `span_bounds_reject_file_out_of_bounds`: `span.file.index() >=
  source_files.len()` -- rejected with `DanglingId`.

### Phase C -- vtables rebuild

- `phase_c_vtables_rebuild_pass`: `item_synthetic_kinds` carries two
  `VTableType` entries with distinct `PrincipalTraitKey`s; after
  validate, `model.vtables` contains both `(principal, item_id)`
  pairs -- accepted.
- `phase_c_vtables_rebuild_reject_duplicate_principal`: two
  `VTableType` entries with the same `PrincipalTraitKey` -- rejected
  with `DuplicateVTablePrincipal { principal, first, second }`.
- `phase_c_vtables_empty_pass`: zero `VTableType` entries; rebuilt
  map is empty -- accepted.

### Phase ordering

- `phase_b_then_c_ordering`: a model that fails *both* a Phase B
  invariant 11 check and a Phase C duplicate-principal check; the
  validator returns the Phase B error (Phase C is gated behind Phase
  B success per `spec.md:676-680` + `spec.md:1104-1113`).

### Quality gates

- `cargo test -p regseq-model` passes (unit + integration tiers
  above).
- All public types and functions carry doc comments.

## Review

After tests pass, run a Codex review using the `codex_runner.py`
four-block template per `.harness/SESSION.md` Review section:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-model-84 (P1a-VAL-02: ID
referential integrity + interner uniqueness + F4 / F4-2 sort + DefPath
/ span / vtables) against its task spec at
.harness/tasks/regseq-model/84-id-referential-integrity-validator.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-model/src/model.rs (extended walk + Phase C
  vtables rebuild + new InvariantViolation variants)
- crates/regseq-model/src/lib.rs (re-exports if any new module
  surfaces here)
- crates/regseq-model/tests/id_referential_integrity_validator.rs

Focus on:
- Spec deviations against spec.md §3.1 invariants 1, 2, 6, 9, 11, 18;
  §3.3 Phase B ordering (lines 676-1102) and Phase C (lines
  1104-1113); §16.1 (lines 6004-6058) F4 comparator and
  def_path_data_sort_rank table verbatim; §16.3 (lines 6071-6085)
  F4-2 RegseqInstanceKey.
- Phase ordering: TyInterner uniqueness depends on FnSigInterner /
  ConstInterner being checked first per spec.md:676-680.
- DefPath rendered re-derivation matches §7.2 canonical rules.
- Source-span bounds are sentinel-aware per spec.md:1093-1102.
- Phase C vtables rebuild rejects duplicate PrincipalTraitKeys per
  spec.md:1104-1113 and does NOT silently overwrite.
- Test Plan coverage: each invariant has both a positive and a
  negative test; F4 / F4-2 sort tests include canonical fixtures and
  permuted inputs.
Do NOT flag anything explicitly marked out of scope (Non-Goals, later
task IDs).
</task>

<grounding_rules>
Cite file:line for every claim and every required fix. Do not
speculate about code you did not read. Mark hypotheses as hypotheses.
</grounding_rules>

<structured_output_contract>
First line: VERDICT: APPROVED | REQUIRES FIXES
If REQUIRES FIXES: numbered list, each entry exactly
  <file>:<line-range> -- <what is wrong> -- <what the spec requires>
If APPROVED: one paragraph (<=120 words) summarizing what was verified
and why any deliberate deviations are acceptable.
Total response <= 400 words.
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, form a verdict, stop.
</default_follow_through_policy>
EOF
```

Iterate `fix -> codex_runner.py review` until VERDICT: APPROVED.

## Quality Gate

```
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

## Acceptance Criteria

- `RegseqModel::validate` (and any private helpers) extended to cover
  invariants 1 / 2 / 6 / 9 / 11 / 18; F4 sort comparator output check
  per `spec.md:6004-6058`; F4-2 sort comparator output check per
  `spec.md:6071-6085`; `DefPath.rendered` canonical-rendering check
  per `spec.md:1057-1062`; source-span bounds (sentinel-aware) per
  `spec.md:1093-1102`; Phase C `vtables` rebuild + duplicate
  rejection per `spec.md:1104-1113`.
- `InvariantViolation` enum extended with the new variants named in
  Implementation Notes (or equivalent typed variants that pin the
  same diagnostic information).
- Per-invariant comments at `crates/regseq-model/src/model.rs:200-231`
  updated to reflect full coverage.
- Test Plan covers each invariant with both positive and negative
  cases against constructed-model fixtures, including the F4 sort's
  canonical and permuted-input fixtures.
- `cargo test -p regseq-model` passes; Codex Task Review verdict is
  APPROVED.

## Audit Findings -- Re-execution Requirements

This task was previously executed and the implementation was reverted
because a Codex post-implementation audit (both a scoped-fix review and
the 4-partition re-review) identified material spec deviations. On
re-execution, the implementation MUST address every finding below in
addition to the original Implementation Notes.

### Tier A fixes already landed in `crates/regseq-model/src/model.rs`

The following pre-fixes are already in the working tree and MUST be
preserved (do not undo them):

- `InvariantViolation::DirectCalleeOutOfRange` -- moved out of
  `place_validator`'s public error type into a top-level invariant
  variant (spec section 23.6 B-9 is owned by this task, not by
  place_validator).
- `validate_synthetic_kinds` -- now enforces
  `SyntheticItemKind` <-> `ItemKind` consistency per spec section 13
  (e.g., `VTableType` -> `Struct`, `VTableInstance` -> `Static { Not }`,
  `VTableMethodShim` / `VTableDropShim` / `DropGlue` / `ClosureCall` /
  `ClosureDropGlue` -> `Function`, `ClosureCaptures` -> `Struct`,
  `ConstMemoryStatic` -> `Static { Not }`).
- `validate_string_interner` -- enforces spec section 12 Phase B step 1
  (non-empty table, `StringId(0) == ""` sentinel, content uniqueness via
  rebuild).

### Required additional work

1. **Place validator structural pre-pass (spec section 19.3).** Split
   `validate_place` in `place_validator.rs` into two passes: a
   structural pre-pass over the FULL projection chain that checks
   `I-Deref-first` and `I-Downcast-followed-by-Field` for every
   element, then the section 19.4 type-narrowing walk only if the
   structural pass returned `Ok(())`. A type-narrowing error at element
   0 MUST NOT be allowed to mask a structural error at element 5.

2. **Statement-site invariants B-1..B-5 (spec section 22.5).** Add new
   InvariantViolation variants and a `validate_body_statement_invariants`
   method on `RegseqModel`. Wire it into `validate()`'s per-instance
   walk. The required invariants:
   - **B-1** `Assign(p, rv)`: `place_ty(p) == rvalue_ty(rv)` modulo
     lifetime erasure. Use rustc's well-formedness rule.
   - **B-2** `SetDiscriminant(p, vi)`: `vi <
     variant_count(place_ty(p))` per `EnumShape` cardinality.
   - **B-3** `SetDiscriminant(p, _)`: `place_ty(p) = Ty::Enum { .. }`
     (other shapes rejected).
   - **B-4** `Intrinsic(NonDivergingIntrinsic::Assume(op))`:
     `operand_ty(op) == Ty::Bool`.
   - **B-5** `Intrinsic(CopyNonOverlapping { src, dst, count })`:
     `src` / `dst` are sized pointer or raw-pointer types; `count` is
     `Ty::Uint(Usize)`.

3. **Terminator-site invariants B-1..B-9 (spec section 23.6).** Add
   new InvariantViolation variants and a
   `validate_body_terminator_invariants` method that enforces ALL of:
   - **B-1** `SwitchInt.discr` operand type
     `∈ {Bool, Char, Int(_), Uint(_)}`.
   - **B-2** `Assert.cond` operand type `== Ty::Bool`.
   - **B-3** `Call.destination` type matches `callee_fn_sig.output()`
     when `target: Some(_)`. Applies to BOTH Direct and Indirect calls
     -- for Indirect, recover the FnSig from
     `operand_ty(callee_operand) = Ty::FnPtr(sig_id)`.
   - **B-4** `Call.args.len() == callee_sig.inputs.len()`. Applies to
     BOTH Direct and Indirect.
   - **B-5** Direct callee MUST resolve to
     `InstanceKind::Function(_)` specifically (bounds-check alone is
     insufficient; rejection variants for non-Function targets are
     required). Indirect callee operand type MUST be `Ty::FnPtr(_)`.
   - **B-6** Per-argument type equality: for each i,
     `operand_ty(args[i]) == callee_sig.inputs[i]` modulo lifetime
     erasure. Applies to BOTH Direct and Indirect.
   - **B-8** When the callee Instance is
     `FunctionInstanceFlavor::Virtual`, `virtual_idx.is_some()` MUST
     hold. Conversely, non-Virtual flavors MUST have
     `virtual_idx.is_none()`.
   - **B-9** `target.is_none() iff callee_output == Ty::Never`. Both
     directions: a divergent callee (return type Never) MUST have
     `target: None`; a non-divergent callee MUST have `target:
     Some(_)`.

4. **TyId closure walk (spec section 25.6.3 invariant 1).** Add a
   `validate_body_ty_closure` method that walks every TyId referenced
   by the body (LocalDecl.ty, projection field types, Rvalue::Cast
   destination type, AggregateKind carrier types, Operand-context
   types) and asserts each resolves within `model.types.len()`. Wire
   into the per-instance validate() walk.

5. **`body_ty` helper semantics.** When implementing
   `place_ty(model, body, place) -> ...`, `rvalue_ty(...)`, and
   `operand_ty(...)` helpers, distinguish:
   - "The spec rule does not apply at this site" (return a sentinel,
     skip the check).
   - "Type recovery failed because the body is malformed" (must
     produce a typed error, NOT silently skip).
   The previous implementation used `Option<TyId>` and silently
   skipped on `None`, which let several spec-mandated invariants
   silently pass on malformed bodies. Re-execution MUST NOT allow
   silent skips when the cause is malformedness rather than spec
   inapplicability.

6. **`rvalue_ty(UnOp::PtrMetadata)`** MUST recover the metadata type
   from the operand's pointee (spec section 21):
   - For `Ptr(dyn Trait)` pointee, return `Ty::Ptr(vtable_struct,
     Mutability::Not)` (the vtable pointer type).
   - For `Ptr(slice T)` pointee, return `Ty::Uint(Usize)`.
   Returning a hardcoded `Ty::Uint(Usize)` for all cases breaks B-1
   validation on the vtable-indirect rewrite output.

### Negative test coverage

Add `#[test]` cases that exercise rejection paths for EVERY new
InvariantViolation variant added in items 2-4 above. At minimum:

- One negative test per variant in the statement B-1..B-5 set.
- One negative test per variant in the terminator B-1..B-9 set
  (separate cases for Direct and Indirect callees where the
  invariant applies to both).
- A negative test for the structural-pre-pass ordering (item 1):
  construct a Place projection chain that has BOTH a type error at
  element 0 AND a structural error at element 5; assert the
  structural error is reported, not the type error.
- A negative test for the TyId closure walk (item 4): construct a
  body whose LocalDecl.ty references a TyId beyond
  `model.types.len()`.
- Positive smoke tests for invariants 2, 6, 18 that prior coverage
  flagged as missing.
