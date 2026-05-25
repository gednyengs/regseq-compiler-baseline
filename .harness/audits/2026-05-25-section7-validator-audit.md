# regseq-driver-78 -- Section 7 validator-invariant coverage audit

Type-B analytical task. No source changes. Audits whether each
`InvariantViolation` variant enforced by `RegseqModel::validate()` /
`validate_bridge_completeness()` has a genuine exercising test (construct a
defective model -> call `validate*` -> assert that specific variant), and
updates Section 7 of `bridge-coverage-matrix.md` accordingly.

Date: 2026-05-25. Grounded at HEAD (commit `8ab0e90` + working tree).

## 0. Method

For each of the **61** `InvariantViolation` variants
(`crates/regseq-model/src/model.rs:163-831`, confirmed 61 by
`awk`/grep over the enum block):

1. **Enforcing method** -- located the `Err(InvariantViolation::X { .. })`
   construction site(s) (not the enum definition) by grep over
   `crates/regseq-model/src/model.rs` and
   `crates/regseq-model/src/layout_validator.rs`. Every variant is
   constructed in a live `validate*` path; **no dead variant exists**.
2. **Exercising test** -- searched for a `#[test]` that injects the defect,
   calls `validate()` / `validate_bridge_completeness()` (directly or via the
   per-file `validate(model)` helper), and asserts the result is *that*
   variant. A bare grep hit was not accepted; each cited test was opened and
   confirmed to assert the variant.
3. **Coverage class** -- `yes` / `partial` / `no` (see classification rule
   below).
4. For each `no`/`partial`: fixture-form (`constructed-model` unless
   justified otherwise) + a one-line defect sketch (seed for the follow-up
   fixture task; NOT authored here).

### Test corpus actually searched

The spec named five validator test files. A sixth surface was found and is
material to the result (see Finding 2):

- `crates/regseq-model/tests/id_referential_integrity_validator.rs` (57 tests)
- `crates/regseq-model/tests/layout_validator.rs` (63)
- `crates/regseq-model/tests/place_validator.rs` (19)
- `crates/regseq-model/tests/validate_bridge_completeness_buckets.rs` (13)
- `crates/regseq-model/tests/validate_item_instance_coverage.rs` (5)
- **`crates/regseq-model/src/model_tests.rs`** -- an in-crate
  `#[cfg(test)] #[path = "model_tests.rs"] mod tests;` module wired at
  `crates/regseq-model/src/model.rs:121-123` (~106 tests). Compiled and run by
  `cargo test -p regseq-model`. The spec's first-pass grep (`<<< NONE >>>`
  markings in `78-section7-validator-audit.md:146-197`) only covered the five
  external files and therefore missed this surface.

The `e2e_*` siblings were grep-scanned for the otherwise-uncovered variants;
none add coverage for the three `no` variants.

### Classification rule (stated for reproducibility)

- **`yes`** -- at least one genuine asserting test directly injects the
  defect and asserts this variant. For variants whose enum carries a free-form
  `reason: &'static str` (a diagnostic string, not a structural
  discriminator), `yes` holds if at least one reason is asserted; any
  materially untested reasons are flagged in the row note but do not by
  themselves downgrade to `partial`, because they share one variant + one
  validate path.
- **`partial`** -- the variant is asserted, but it carries a *typed,
  enum-like discriminator field* (`which`, `side`, `end`, ...) whose
  enumerated values gate genuinely distinct code paths, and only a strict
  subset of those values is tested. The untested arm gets a defect sketch.
- **`no`** -- no asserting test anywhere in the corpus above.

## 1. Per-variant audit table (enum order; VL-ids per the renumbered Section 7)

| VL-id | Variant | Enforcing method (`file:line`) | Cov | Exercising test(s) `file:fn` (or `no` note) |
| ----- | ------- | ------------------------------ | --- | -------------------------------------------- |
| VL-001 | `DanglingId` | `check_*_id` + `validate_body_ty_closure` (`model.rs:2501,3442,3737-3822`) | yes | `model_tests.rs:invariant_1_dangling_item_id_in_instance`; `id_ref:invariant_1_reject_dangling_ty_in_const_null_ptr`, `invariant_9_reject_const_reloc`, `span_bounds_reject_file_out_of_bounds`, `body_ty_closure_rejects_dangling_const_id_in_operand` |
| VL-002 | `ItemDefPathsLengthMismatch` | `validate` (`model.rs:1692`) | yes | `model_tests.rs:invariant_2_item_def_paths_length_mismatch` |
| VL-003 | `ItemSyntheticKindsLengthMismatch` | `validate` (`model.rs:1699`) | yes | `id_ref:invariant_2_reject_synthetic_kinds_short`; `model_tests.rs:invariant_2_item_synthetic_kinds_length_mismatch` |
| VL-004 | `PrimaryCrateOutOfRange` | `validate` (`model.rs:1674`) | yes | `model_tests.rs:invariant_4_primary_crate_out_of_range` |
| VL-005 | `PrimaryCrateStableIdMismatch` | `validate` (`model.rs:1710`) | yes | `model_tests.rs:invariant_4_primary_crate_stable_id_mismatch` |
| VL-006 | `DuplicateStableCrateId` | `validate_unique_stable_crate_ids` (`model.rs:2083`) | yes | `model_tests.rs:invariant_5_duplicate_stable_crate_id` |
| VL-007 | `DefPathCrateIdOutOfBounds` | `validate_items` (`model.rs:2098`) | yes | `id_ref:invariant_6_reject_def_path_crate_id_out_of_bounds` |
| VL-008 | `DuplicateInternerEntry` | `validate_{const,fn_sig,ty}_interner` (`model.rs:2183,2197,2216`) | yes | `id_ref:invariant_11_ty_interner_pass_and_reject_duplicate`, `invariant_11_fn_sig_interner_pass_and_reject_duplicate`, `invariant_11_const_interner_pass_and_reject_duplicate`, `phase_b_then_c_ordering` |
| VL-009 | `DuplicateItemDefPathHash` | `validate_item_def_path_hash_uniqueness` (`model.rs:3318`) | yes | `id_ref:invariant_18_reject_duplicate_item_def_path_hash` |
| VL-010 | `F4OutOfOrder` | `validate_f4_item_order` (`model.rs:2139`) | yes | `id_ref:f4_sort_reject_permuted_input` |
| VL-011 | `F4OutOfOrderInstance` | `validate_f4_instance_order` (`model.rs:3343`) | yes | `id_ref:f4_2_sort_reject` |
| VL-012 | `DuplicateInstanceKey` **(new row)** | `validate_unique_instance_keys` (`model.rs:3368`) | yes | `validate_bridge_completeness_buckets.rs:bucket1_function_item_with_duplicate_shim_instances_rejected` |
| VL-013 | `DefPathRenderMismatch` | `validate_items` (`model.rs:2115`) | yes | `id_ref:def_path_render_reject_malformed` |
| VL-014 | `SpanOutOfBounds` | `validate_span` (`model.rs:3401`) | yes | `id_ref:span_bounds_reject_lo_gt_hi`, `span_bounds_reject_hi_gt_content_len` |
| VL-015 | `SpanNotOnUtf8Boundary` | `validate_span` (`model.rs:3409,3417`) | yes | `id_ref:span_bounds_reject_not_on_utf8_boundary_lo` (end="lo"), `span_bounds_reject_not_on_utf8_boundary_hi` (end="hi") |
| VL-016 | `DuplicateVTablePrincipal` | `rebuild_vtables` (`model.rs:3380`) | yes | `id_ref:phase_c_vtables_rebuild_reject_duplicate_principal` |
| VL-017 | `SynthesizedSentinelInvalid` | `validate_synthesized_sentinel` (`model.rs:2049-2070`) | yes | `model_tests.rs:invariant_8_synthesized_sentinel_invalid_path`, `invariant_8_synthesized_sentinel_with_content_rejected`, `invariant_8_synthesized_sentinel_with_wrong_line_starts_rejected` |
| VL-018 | `StringInternerSentinelInvalid` | `validate_string_interner` (`model.rs:2323,2328`) | **no** | no asserting test in any corpus file; unreachable backstop (see S.2/S.3 finding 5) |
| VL-019 | `DuplicateString` | `validate_string_interner` (`model.rs:2335`) | **no** | no asserting test in any corpus file; unreachable backstop (see S.2/S.3 finding 5) |
| VL-020 | `PlaceWalk` | `validate` via `validate_body_places` (`model.rs:1740`) | yes | `id_ref:place_validator_structural_error_at_index_5_not_masked_by_type_error_at_index_0`; `place_validator.rs:joint_validator_integration_pass_and_reject` |
| VL-021 | `SyntheticKindItemKindMismatch` | `validate_synthetic_kinds` (`model.rs:2379`) | **no** | no asserting test in any corpus file |
| VL-022 | `DirectCalleeOutOfRange` | `validate_body_direct_callees` (`model.rs:2406`) | yes | `id_ref:terminator_b7_direct_callee_out_of_range_rejects`; `place_validator.rs:joint_validator_rejects_dangling_direct_call_instance` |
| VL-023 | `TargetMalformed` | `validate_target` (`layout_validator.rs:38-`) | yes | `layout_validator.rs:target_reject_pointer_width_invalid`, `target_reject_endian_disagrees`, `target_reject_scalar_layouts_missing`, `target_reject_scalar_layouts_non_power_of_two_alignment`, `target_reject_scalar_layouts_wrong_int_width`, `target_reject_scalar_layouts_oversized_alignment` |
| VL-024 | `BuildMalformed` | `validate_build` (`layout_validator.rs:257,262`) | yes | `layout_validator.rs:build_reject_toolchain_empty` (note: only the toolchain-empty reason of the two construction sites is asserted; `reason`-string, not a structural discriminator) |
| VL-025 | `FnSigMalformed` | `validate_fn_sigs` (`layout_validator.rs:274,280,288`) | yes | `layout_validator.rs:fn_sig_c_variadic_reject_non_c_abi`, `fn_sig_c_variadic_reject_zero_inputs`, `fn_sig_reject_inputs_never` |
| VL-026 | `CAbiLoweringMalformed` | `validate_c_abi_lowerings` / `validate_c_abi_shape` (`layout_validator.rs:301-360`) | yes | `layout_validator.rs:cabi_lowering_reject_arity_mismatch`, `cabi_lowering_reject_substituted_emit_as_dangling`, `cabi_lowering_reject_tag_scalar_non_int`, `cabi_lowering_reject_some_for_rust_abi`, `cabi_lowering_reject_none_for_c_abi` |
| VL-027 | `LayoutMalformed` | `validate_layouts` + tuple/array/struct/union/enum helpers (`layout_validator.rs:407-`) | yes | `layout_validator.rs:layout_reject_field_overlap`, `layout_reject_field_offset_misaligned`, `layout_reject_field_overflows_layout_size` (note: struct-kind reasons asserted; tuple/array/union/enum `LayoutMalformed` reasons untested -- same variant, `reason`-string) |
| VL-028 | `TagScalarOutOfRange` | `validate_tag_scalar_width_fit` (`layout_validator.rs:668`) | yes | `layout_validator.rs:tag_scalar_width_fit_reject_overflow` |
| VL-029 | `InstanceLayoutMismatch` | `validate_instance_layouts` (`layout_validator.rs:739`) | yes | `layout_validator.rs:invariant_14_reject_kind_mismatch_struct_to_enum` |
| VL-030 | `InstanceLayoutItemArgsMismatch` | `validate_instance_layouts` (`layout_validator.rs:746`) | yes | `layout_validator.rs:invariant_14_reject_item_args_mismatch` |
| VL-031 | `ConstGraphCycle` | `validate_const_graph_acyclic` / `dfs_visit` (`layout_validator.rs:824`) | yes | `layout_validator.rs:const_graph_cycle_reject_self_loop`, `const_graph_cycle_reject_two_node`, `const_graph_cycle_reject_enum_value_payload` |
| VL-032 | `DomainAttrMalformed` | `validate_domain_attrs` / `validate_field_path` / `validate_attr_data_payload` (`layout_validator.rs:853-932`) | yes | `layout_validator.rs:domain_attr_item_attrs_len_reject`, `domain_attr_field_path_reject_non_adt_item`, `domain_attr_field_path_reject_variant_idx_out_of_range`, `domain_attr_field_path_reject_field_idx_out_of_range`, `domain_attr_nested_id_reject_dangling`, `intrinsic_singleton_reject_two_intrinsics` |
| VL-033 | `OpaqueHandleInconsistent` | `validate_opaque_handle_eligibility` (`layout_validator.rs:1031-`) | yes | `layout_validator.rs:invariant_19_reject_non_struct_kind`, `invariant_19_reject_generic`, `invariant_19_reject_non_zst_layout`, `invariant_19_reject_by_value_use` |
| VL-034 | `StableCrateIdZeroCollision` | `validate_stable_crate_id_zero` (`layout_validator.rs:1194`) | yes | `layout_validator.rs:invariant_20_reject_real_crate_zero` |
| VL-035 | `SyntheticDomainBoundary` | `validate_stable_crate_id_zero` (`layout_validator.rs:1209,1215`) | yes | `layout_validator.rs:invariant_20_reject_source_derived_synthetic_domain`, `invariant_20_reject_synthetic_non_zero_domain` |
| VL-036 | `UnitLayoutMismatch` | `validate_unit_layout` (`layout_validator.rs:1012`) | yes | `layout_validator.rs:unit_layout_reject_non_canonical` |
| VL-037 | `SourceFileMalformed` | `validate_source_files` (`layout_validator.rs:1235,1241,1253`) | yes | `layout_validator.rs:source_file_reject_line_starts_non_monotonic`, `source_file_reject_line_starts_does_not_start_at_zero`, `source_file_reject_line_start_exceeds_content_len` |
| VL-038 | `DuplicateSourceFilePath` | `validate_source_files` (`layout_validator.rs:1265`) | yes | `layout_validator.rs:source_file_reject_duplicate_path` |
| VL-039 | `BodyTyIdOutOfRange` | `validate_body_ty_closure` (`model.rs:2430,2480,2527,2554`) | yes | `id_ref:body_ty_closure_rejects_dangling_local_decl_ty` |
| VL-040 | `StatementAssignTypeMismatch` | `validate_body_statement_invariants` (`model.rs:2658`) | yes | `id_ref:statement_b1_assign_type_mismatch_rejects` |
| VL-041 | `StatementAssignMalformed` | `validate_body_statement_invariants` + `validate_copy_non_overlapping` (`model.rs:2641,2649,2697,2720,2810,2819`) | **partial** | `id_ref:statement_b1_assign_malformed_rejects_ptr_metadata_on_non_fat_pointer` asserts only `side="rvalue"`; the `side="place"` arm is untested |
| VL-042 | `SetDiscriminantVariantIdxOutOfRange` | `validate_body_statement_invariants` (`model.rs:2678`) | yes | `id_ref:statement_b2_set_discriminant_variant_idx_out_of_range_rejects` |
| VL-043 | `SetDiscriminantNonEnumTarget` | `validate_body_statement_invariants` (`model.rs:2689`) | yes | `id_ref:statement_b3_set_discriminant_non_enum_target_rejects` |
| VL-044 | `AssumeOperandNotBool` | `validate_body_statement_invariants` (`model.rs:2711`) | yes | `id_ref:statement_b4_assume_operand_not_bool_rejects` |
| VL-045 | `CopyNonOverlappingOperandType` | `validate_copy_non_overlapping` (`model.rs:2757,2769,2779,2788`) | **partial** | `id_ref:statement_b5_copy_non_overlapping_count_type_rejects` asserts only `which="count"`; the `which` arms `"src"`, `"dst"`, `"pointee"` are untested |
| VL-046 | `SwitchIntDiscrTypeMismatch` | `validate_switch_int_discr` (`model.rs:2883,2891`) | yes | `id_ref:terminator_b1_switch_int_discr_type_mismatch_rejects` |
| VL-047 | `AssertCondNotBool` | `validate_assert_cond` (`model.rs:2910,2918`) | yes | `id_ref:terminator_b2_assert_cond_not_bool_rejects` |
| VL-048 | `CallDestinationTypeMismatch` | `validate_call` (`model.rs:3120,3129`) | yes | `id_ref:terminator_b3_call_destination_type_mismatch_rejects`, `terminator_b3_call_destination_type_mismatch_rejects_indirect` |
| VL-049 | `CallArgsArityMismatch` | `validate_call` (`model.rs:3055`) | yes | `id_ref:terminator_b4_call_args_arity_mismatch_rejects_direct`, `terminator_b4_call_args_arity_mismatch_rejects_indirect`; `model_tests.rs:non_log_direct_call_uses_generic_b4`, `indirect_call_does_not_receive_log_exception` |
| VL-050 | `CallDirectCalleeNotFunction` | `validate_call` (`model.rs:2978`) | yes | `id_ref:terminator_b5_call_direct_callee_not_function_rejects` |
| VL-051 | `CallIndirectCalleeNotFnPtr` | `validate_call` (`model.rs:2996,3008`) | yes | `id_ref:terminator_b5_call_indirect_callee_not_fn_ptr_rejects` |
| VL-052 | `CallArgTypeMismatch` | `validate_call` (`model.rs:3070,3081`) | yes | `id_ref:terminator_b6_call_arg_type_mismatch_rejects_direct`, `terminator_b6_call_arg_type_mismatch_rejects_indirect` |
| VL-053 | `CallVirtualIdxInconsistent` | `validate_call` (`model.rs:2954,2962`) | yes | `id_ref:terminator_b8_virtual_idx_inconsistent_rejects` |
| VL-054 | `CallTargetDivergenceMismatch` | `validate_call` (`model.rs:3099,3106`) | yes | `id_ref:terminator_b9_target_divergence_mismatch_rejects_non_diverging_no_target`, `terminator_b9_target_divergence_mismatch_rejects_indirect_divergent_target_some` |
| VL-055 | `DiagLogCallArg0NotStrRef` | `validate_log_intrinsic_call` (`model.rs:3186,3194`) | yes | `model_tests.rs:rejects_args_zero_not_str_ref` |
| VL-056 | `DiagLogCallArityBelowMinimum` | `validate_log_intrinsic_call` (`model.rs:3178`) | yes | `model_tests.rs:rejects_empty_args` |
| VL-057 | `DiagLogCallArgNotAdmissible` | `validate_log_intrinsic_call` (`model.rs:3213,3222`) | yes | `model_tests.rs:rejects_args_one_not_admissible_log_arg_source` |
| VL-058 | `ItemMissingInstance` | `validate_bridge_completeness` (`model.rs:3648,3702`) | yes | `validate_bridge_completeness_buckets.rs:bucket1_struct_zero_instances_rejected`, `bucket2_foreign_function_zero_instances_rejected`; `validate_item_instance_coverage.rs:b1_struct_item_without_instance_is_rejected`, `b2_foreign_struct_item_without_instance_is_rejected` |
| VL-059 | `Bucket1MultipleInstances` | `validate_bridge_completeness` (`model.rs:3654,3719`) | yes | `validate_bridge_completeness_buckets.rs:bucket1_struct_multiple_instances_rejected`, `bucket1_function_item_with_two_primary_instances_rejected` |
| VL-060 | `ForeignInstanceBodyOrInitNotNone` | `validate_bridge_completeness` (`model.rs:3677`) | yes | `validate_bridge_completeness_buckets.rs:bucket2_foreign_function_body_must_be_none`, `bucket2_foreign_static_init_must_be_none` |
| VL-061 | `Bucket4HasInstance` | `validate_bridge_completeness` (`model.rs:3692`) | yes | `validate_bridge_completeness_buckets.rs:bucket4_rpit_function_with_instance_rejected` |

## 2. Fixture-form + defect sketch for every `no` / `partial`

Three of the five gaps (VL-021 + the two `partial`s) are `constructed-model`
(these invariants police internal model well-formedness and are
representable in a hand-built `RegseqModel`). **Two (VL-018, VL-019) are
`unreachable-by-construction`** -- the `StringInterner` encapsulation makes the
defect unrepresentable through any existing API, so they are validator
backstops, not ordinary fixture candidates (see finding 5). None is
`source-snippet`.

| VL-id | Variant | Gap | Fixture-form | Defect to inject (one-line seed) / reachability |
| ----- | ------- | --- | ------------ | ----------------------------------------------- |
| VL-018 | `StringInternerSentinelInvalid` | no test | **unreachable-by-construction** | `StringInterner` has private fields; `new()` always seats the empty sentinel at `StringId(0)` (`string_interner.rs:34-40`), `intern()` only appends fresh non-empty strings (`:48-56`), and `Deserialize` rejects an empty table and a non-empty index 0 (`:133-142`). No safe public/crate-test path can build a sentinel-violating interner. Exercising VL-018 would require a *new test-only unchecked constructor / direct-`storage` hook* added to `string_interner.rs` -- a source change, not a model-level fixture. NOT a fixture-batch task. |
| VL-019 | `DuplicateString` | no test | **unreachable-by-construction** | Same encapsulation: `intern()` is content-deduplicating (`string_interner.rs:49-50`) and `Deserialize` rejects duplicate entries (`:157-161`, tested by `string_interner_tests.rs:deserialize_rejects_duplicate_entries`). The deserialize check rejects *before* an interner is built, so `validate_string_interner`'s `DuplicateString` arm is never reached. Reaching it needs the same test-only hook as VL-018. NOT a fixture-batch task. |
| VL-021 | `SyntheticKindItemKindMismatch` | no test | constructed-model | Push an `Item` with `kind = ItemKind::Function` but `item_synthetic_kinds[item] = SyntheticItemKind::ClosureCaptures { .. }` (which requires `Struct`). `item_synthetic_kinds` is a `pub` field (`model.rs:83`), so this is buildable from an external or in-crate test; expect `validate()` -> `SyntheticKindItemKindMismatch`. Inject on a minimal item so no earlier validator shadows the check. |
| VL-041 | `StatementAssignMalformed` (`side="place"` arm) | only `side="rvalue"` asserted | constructed-model | An `Assign(place, rvalue)` whose *place*-side type recovery fails (e.g. a projection `body_ty` cannot resolve) while the rvalue recovers; expect `StatementAssignMalformed { side: "place" }`. |
| VL-045 | `CopyNonOverlappingOperandType` (`which` arms `src`/`dst`/`pointee`) | only `which="count"` asserted | constructed-model | (a) `CopyNonOverlapping` with a non-pointer `src` operand -> `which="src"`; (b) non-pointer `dst` -> `which="dst"`; (c) `src`/`dst` with disagreeing pointee types -> `which="pointee"`. |

## 3. Step-5 summary roll-up

### Counts (over all 61)

Coverage axis (Step 3) is `yes`/`partial`/`no`; the triage axis (Step 4)
splits the uncovered set into the spec's count buckets. VL-018/VL-019 are
uncovered AND `unreachable-by-construction`, so they sit in the `unreachable`
bucket rather than the fixture-bound `no` bucket (avoids double-counting).

| class | count | variants |
| ----- | ----- | -------- |
| `yes` | 56 | all except those below |
| `partial` | 2 | VL-041 `StatementAssignMalformed`, VL-045 `CopyNonOverlappingOperandType` |
| `no` (uncovered, fixture-bound) | 1 | VL-021 `SyntheticKindItemKindMismatch` |
| `unreachable-by-construction` (uncovered backstop) | 2 | VL-018 `StringInternerSentinelInvalid`, VL-019 `DuplicateString` |
| dead (never constructed in source) | 0 | -- |
| **total** | **61** | |

(Coverage-axis tally for cross-check: 56 `yes`, 2 `partial`, 3 `no` =
VL-018 + VL-019 + VL-021.)

### Fixture-batch follow-up candidates, grouped by enforcing method / file

One sub-batch of **3 gaps** (1 `no` + 2 `partial` completions); all land in
`model.rs` validators and would most naturally extend
`crates/regseq-model/src/model_tests.rs` or the relevant `tests/` file:

- **`validate_synthetic_kinds` (`model.rs:2341`)** -- VL-021
  `SyntheticKindItemKindMismatch` (constructed-model; `item_synthetic_kinds`
  is `pub`).
- **`validate_copy_non_overlapping` (`model.rs:2741`)** -- VL-045, add the
  `which` = `"src"` / `"dst"` / `"pointee"` arms.
- **`validate_body_statement_invariants` (`model.rs:2627`)** -- VL-041, add the
  `side="place"` arm.

**Excluded from the fixture batch** (per the Step-4 `unreachable-by-construction`
rule): VL-018 `StringInternerSentinelInvalid`, VL-019 `DuplicateString`. These
require a test-only unchecked `StringInterner` constructor / direct-`storage`
hook to exercise, which is a source change to `string_interner.rs`, not a
model-level fixture. Flagged for a possible dedicated follow-up that either
adds such a test hook or formally documents them as defensive backstops.

### Second-order findings

1. **Section 7 was stale by exactly one variant.** The enum has 61 variants;
   Section 7 listed 60. `DuplicateInstanceKey` (enum #12, `model.rs:269`,
   added by R1/driver-73 commit `e89e02d`) was missing, making VL-012..VL-060
   off-by-one from the enum from `DefPathRenderMismatch` onward. Fixed by the
   Section 7 renumber (Change B): `DuplicateInstanceKey` inserted as VL-012,
   old VL-012..VL-060 -> VL-013..VL-061. Re-confirmed: `DuplicateInstanceKey`
   *is* covered
   (`validate_bridge_completeness_buckets.rs:bucket1_function_item_with_duplicate_shim_instances_rejected`).

2. **The spec's "7 zero-reference variants" premise was an artifact of an
   incomplete grep.** The spec's first-pass map searched only the five
   external test files and marked seven variants `<<< NONE >>>`. Four of those
   seven are in fact covered by genuine asserting tests in the *sixth* surface,
   `crates/regseq-model/src/model_tests.rs` (the in-crate `#[cfg(test)]`
   module at `model.rs:121-123`):
   - VL-017 `SynthesizedSentinelInvalid` -> `invariant_8_synthesized_sentinel_*` (3 tests).
   - VL-055 `DiagLogCallArg0NotStrRef` -> `rejects_args_zero_not_str_ref`.
   - VL-056 `DiagLogCallArityBelowMinimum` -> `rejects_empty_args`.
   - VL-057 `DiagLogCallArgNotAdmissible` -> `rejects_args_one_not_admissible_log_arg_source`.
   Only the remaining **3** (VL-018, VL-019, VL-021) are genuinely uncovered.
   Any future fixture-batch decomposition must include `model_tests.rs` in its
   coverage search or it will re-derive the same false `no` set.

3. **Several already-`yes` VL-001..VL-011 rows are sourced from
   `model_tests.rs`, not the file Section 7's header named.** Section 7's
   header credited `id_referential_integrity_validator.rs` as the sole source,
   but VL-002 `ItemDefPathsLengthMismatch`, VL-004 `PrimaryCrateOutOfRange`,
   VL-005 `PrimaryCrateStableIdMismatch`, and VL-006 `DuplicateStableCrateId`
   are exercised only in `model_tests.rs`. All eleven VL-001..VL-011 rows
   re-confirm as `yes` -- **no downgrades** -- but the header's single-file
   framing was inaccurate even for them. The Section 7 header prose is updated
   to name the six-file corpus.

4. **No dead variants and no `source-snippet` cases.** All 61 variants are
   constructed in a live `validate*` path; no `no`/`partial` variant is
   source-producible (no evidence the bridge can emit an invalid model).

5. **VL-018 and VL-019 are unreachable validator backstops.** The
   `validate_string_interner` arms for `StringInternerSentinelInvalid` and
   `DuplicateString` (`model.rs:2323,2328,2335`) cannot be tripped by any
   existing construction path: `StringInterner`'s fields are private, `new()`
   seats the empty sentinel (`string_interner.rs:34-40`), `intern()` only
   appends fresh non-empty content-deduplicated strings (`:48-56`), and
   `Deserialize` rejects empty / non-empty-index-0 / duplicate entries
   (`:133-166`). The interner's only test surface (`string_interner_tests.rs`)
   exercises the *deserialize-rejection* path, which fails before an interner
   is built, so `validate_string_interner` never sees a violating interner.
   Consequently these two arms are defensive backstops, reachable only via a
   test-only unchecked constructor / direct-`storage` hook that does not exist
   today. They are correctly `no` on coverage but `unreachable-by-construction`
   on triage, and are excluded from the fixture-batch follow-up. (Initial draft
   of this audit wrongly proposed a `Deserialize`-rebuild constructed-model
   sketch for them; corrected here -- `Deserialize` is exactly the path that
   rejects the defect.)

## 4. Scope / quality-gate confirmation

- Read-only over all source: no `.rs`, no `rows.toml`, no `snippets/`, no
  runner-sidecar edit. The deliverable consists of exactly three paths -- this
  audit doc (new), the Section 7 block of `bridge-coverage-matrix.md`, and the
  `tasks.json` status flip via `task.py` -- which are the only paths the
  finishing-step commit stages (`git add`), exactly the three the Quality Gate
  permits.
- **Pre-existing untracked worktree state, NOT part of this deliverable:**
  `crates/regseq-driver/tests/fixtures/` and
  `tests/fixtures/playground-pl011/` were already present (untracked) at the
  start of this task and are unrelated to the Section 7 audit. They are left
  untouched and deliberately NOT staged; the commit excludes them.
- `cargo test -p regseq-model` is green at HEAD (sanity only; no code change).
- Audit table row count == 61 == enum variant count.
