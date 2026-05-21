# regseq-driver-66: implementation evidence

Implementation deliverable for `regseq-driver-66`, the F3
dispatch-hardening (14 arms) + sub-axis matcher (4 axes)
phase of the bridge-coverage-runner per
`.harness/tasks/regseq-driver/66-bridge-coverage-runner-f3-dispatch-hardening.md`
and the bridge-coverage-runner strategy at
`docs/discussions/regseq-model/bridge-coverage-runner-strategy.md`
section 11 (phasing -- F3 "Inspection-predicate dispatch hardening
+ failure clustering polish + report shape refinement based on F2
findings").

Parent context: this task consumes the F2 runner state shipped by
regseq-driver-65 (closed at `3a01888`) read-write on
`crates/bridge-coverage-runner/src/inspect.rs` (Change A: 14 new
dispatch arms + 13 helper variant-tag functions + 14 unit tests),
`crates/bridge-coverage-runner/src/matrix.rs` (Change B: schema
extension), `crates/bridge-coverage-runner/src/main.rs` (Change B:
matcher extension + 5 unit tests), and
`crates/bridge-coverage-runner/matrix_metadata/rows.toml` (Change
B: sub-axis precision uplift for 5 DiagLogFormat rows). The F0
helper at `crates/test-ir-fixtures/src/lib.rs` and the other five
F1 modules (`snippet.rs`, `oracle.rs`, `cluster.rs`, `report.rs`,
`build.rs`) are read-only.

## Scope

- 14 new `InspectionPredicate` dispatch arms in
  `crates/bridge-coverage-runner/src/inspect.rs` (`StatementKind`,
  `TerminatorKind`, `RvalueVariant`, `PlaceElem`,
  `AggregateKind`, `CastKind`, `BinOp`, `UnOp`, `AssertKind`,
  `NonDivergingIntrinsic`, `CallCallee`, `BodyAbsent`,
  `Linkage`, `ScalarKind`) plus 13 helper variant-tag functions
  (one per arm except `BodyAbsent` which has no helper) and 14
  module-level unit tests.
- `ExpectedRejection` schema extension at
  `crates/bridge-coverage-runner/src/matrix.rs` adding `reason:
  Option<String>` and `diagnostic: Option<String>` for the
  DiagLog sub-axes.
- `expected_rejection_matches` extension at
  `crates/bridge-coverage-runner/src/main.rs:264+` enforcing
  every applicable sub-axis (`kind` for `DslRejection`,
  `position` for `UnsupportedCAbiType`, `reason` for
  `DiagLogFormat`, `diagnostic` for `DiagLogValidation`) when the
  corresponding field is `Some`. `None` semantics preserve the
  F2 variant-only matching behavior. 5 new matcher unit tests
  cover variant-only, kind-precise, reason-precise,
  position-precise, and diagnostic-precise cases.
- Sub-axis precision uplift for 5 F2 DiagLogFormat rows
  (RJ-031-A / RJ-031-B / RJ-031-C / RJ-031-E / RJ-031-F) in
  `crates/bridge-coverage-runner/matrix_metadata/rows.toml`,
  adding `reason = "<variant tag>"` to each.
- CastKind dotted-compound `"PointerCoercion::<sub>"` encoding
  convention documented in
  `crates/bridge-coverage-runner/snippets/README.md`.
- This expectations file.

No edits to F1+F2 runner architecture beyond `inspect.rs`,
`matrix.rs`, and `main.rs` (`snippet.rs`, `oracle.rs`,
`cluster.rs`, `report.rs`, `build.rs`, `Cargo.toml` untouched).
No new snippet files; the sidecar gains no new rows. No edits to
F0. No edits to spec / strategy / matrix / pretty-printer /
generic-c-backend docs. No bridge / model / facade changes.

## Before-fix evidence

- Pre-fix HEAD `a10172c` (the F3 task-spec registration commit)
  carried the F2 runner state: 32 sidecar rows + 32 snippets, 8
  implemented dispatch arms (5 F1 + 3 mini-F3 inside F2), and 14
  `Unimplemented` fallthrough arms blocked behind the
  fallthrough `other =>` catch-all in `check`. The F2
  expectations file at
  `.harness/expectations/regseq-driver-65.md:304-333` enumerates
  the 14 unimplemented arms; F3 closes them out.
- The F1 `expected_rejection_matches` at
  `crates/bridge-coverage-runner/src/main.rs:264-299` (pre-F3)
  enforced only the `variant` field of `BridgeError` (with
  partial enforcement for `DslRejection.kind` and
  `UnsupportedCAbiType.position`, but NO `DiagLogFormat.reason`
  or `DiagLogValidation.diagnostic` enforcement). All 5
  RJ-031-* DiagLogFormat rows in the F2 sidecar carried only the
  top-level `variant = "DiagLogFormat"` -- the per-row
  `DiagLogFormatReason` precision was recorded narratively in the
  F2 expectations file but not enforced by the runner.

## After-fix evidence

### Change A: 14 new `InspectionPredicate` dispatch arms

- `crates/bridge-coverage-runner/src/inspect.rs:289+` adds 13
  helper variant-tag functions adjacent to the existing F1+F2
  helpers (`ty_variant_tag`, `item_kind_tag`,
  `synthetic_kind_variant_tag`, `const_variant_tag`,
  `flavor_tag`):
  - `statement_kind_tag` (3 variants),
  - `terminator_kind_tag` (6 variants),
  - `rvalue_variant_tag` (9 variants),
  - `place_elem_tag` (6 variants),
  - `aggregate_kind_tag` (7 variants),
  - `cast_kind_tag` (11 top-level variants; nested
    `PointerCoercion(sub)` flattened as
    `"PointerCoercion::<sub>"` dotted compound to preserve
    precision per the spec's CastKind convention rule),
  - `bin_op_tag` (26 variants per matrix RV-006-A..Z),
  - `un_op_tag` (3 variants),
  - `assert_kind_tag` (8 variants),
  - `non_diverging_intrinsic_tag` (2 variants),
  - `call_callee_tag` (2 variants),
  - `linkage_tag` (4 variants),
  - `scalar_kind_tag_from_ty` (returns
    `Option<&'static str>` -- `None` for non-Int/Uint/Float
    Ty variants, else the inner scalar-kind variant tag
    `"I8"`/`"U16"`/.../`"F128"`).
- `crates/bridge-coverage-runner/src/inspect.rs` `check`
  function extends with 14 new dispatch arms following the F2
  pattern (single `match pred` arm walking the relevant model
  slice via `iter().any(...)`, returning `Ok(())` on first
  match or `Err(InspectError::NotFound { variant, arg })` on
  exhaustion). The body-walking arms reuse the canonical
  traversal pattern from the F2 `CallDivergent` arm
  (`model.instances.iter().filter_map(... Function ...
  data.body.as_ref()).flat_map(...)`). The `PlaceElem` arm
  walks every projection in every statement-bearing and
  terminator-bearing place (including operands' Copy/Move
  places and call/assert operands) via the
  `any_place_elem_in_block` helper.
- The dispatch now covers all 20 leaf variants + 2 composites
  = 22/22 of the predicate vocabulary. The pre-F3
  `other => Err(InspectError::Unimplemented { ... })` catch-all
  was removed because all leaf variants are now explicitly
  matched and the `All`/`Any` composites already dispatch.
  Compiler-side exhaustiveness checking would have flagged any
  remaining unimplemented variant. As a corollary, the
  `InspectError::Unimplemented` variant was removed from the
  error enum (no producer, and its main.rs consumer in
  `classify` had become dead code). The surviving error
  variants are `InspectError::NotFound` and
  `InspectError::AnyFailed`.
- 14 module-level unit tests in
  `crates/bridge-coverage-runner/src/inspect.rs` exercise each
  new arm:
  - `statement_kind_arm_matches_set_discriminant_and_reports_not_found_otherwise`
  - `terminator_kind_arm_matches_return_and_reports_not_found_otherwise`
  - `rvalue_variant_arm_matches_discriminant_and_reports_not_found_otherwise`
  - `place_elem_arm_matches_deref_in_assign_lhs_and_reports_not_found_otherwise`
  - `aggregate_kind_arm_matches_tuple_aggregate_and_reports_not_found_otherwise`
  - `cast_kind_arm_matches_pointer_coercion_dotted_compound_and_flat_variants`
    (explicitly covers BOTH the flat `IntToInt` variant and the
    dotted-compound `PointerCoercion::Unsize` variant, per
    spec's dotted-compound test-name requirement)
  - `bin_op_arm_matches_add_and_reports_not_found_otherwise`
  - `un_op_arm_matches_neg_and_reports_not_found_otherwise`
  - `assert_kind_arm_matches_null_pointer_dereference_and_reports_not_found_otherwise`
  - `non_diverging_intrinsic_arm_matches_assume_and_reports_not_found_otherwise`
  - `call_callee_arm_matches_direct_call_and_reports_not_found_otherwise`
  - `body_absent_arm_matches_function_without_body_and_reports_not_found_otherwise`
    (constructs a Function with `body: None`; verifies the
    inverse case with `body: Some(_)` reports
    `NotFound { variant: "BodyAbsent", arg: "" }`)
  - `linkage_arm_matches_no_mangle_and_reports_not_found_otherwise`
  - `scalar_kind_arm_matches_int_kind_inner_and_reports_not_found_otherwise`

### Change B: sub-axis matcher extension + schema additions

- `crates/bridge-coverage-runner/src/matrix.rs` adds two
  optional fields to the `ExpectedRejection` struct: `reason:
  Option<String>` (consumed by `DiagLogFormat` variant) and
  `diagnostic: Option<String>` (consumed by `DiagLogValidation`
  variant). Both are `Option<_>` so existing F2 sidecar rows
  that do not set them remain compatible.
- `crates/bridge-coverage-runner/src/main.rs`
  `expected_rejection_matches` extends with two new arms in the
  inner `match err` block:
  - `BridgeError::DiagLogFormat { reason, .. }` checks
    `expected.reason` against `diag_log_format_reason_tag`
    (added: 6 reasons -- `DebugFormatterRejected`,
    `PositionalArgRejected`, `NamedArgRejected`,
    `NonSubsetModifier`, `UnclosedBrace`,
    `UnmatchedClosingBrace`).
  - `BridgeError::DiagLogValidation { diagnostic, .. }`
    checks `expected.diagnostic` against
    `diag_log_validation_diagnostic_tag` (added: 6 diagnostics
    -- `D1`, `D2`, `D4`, `D6`, `D7`, `D8`).
  Both follow the existing `kind` / `position` shape:
  `Option::None => no constraint`, `Option::Some(want) =>
  enforce that actual_tag == want, else
  Err(detail-string-naming-mismatch)`.
- 5 new module-level unit tests inside `main.rs`'s
  `#[cfg(test)] mod tests` block cover the matcher matrix:
  - `expected_rejection_matches_variant_only_when_sub_axis_absent`
    (DslRejection, `kind = None`, atomic actual -- pass)
  - `expected_rejection_matches_kind_when_kind_present`
    (DslRejection, `kind = Some("Atomic")`, atomic actual ->
    Ok; Box actual -> Err with detail naming "Atomic" and
    "Box")
  - `expected_rejection_matches_reason_when_reason_present`
    (DiagLogFormat, `reason =
    Some("DebugFormatterRejected")` -> Ok against
    `DebugFormatterRejected` actual; Err naming mismatch
    against `PositionalArgRejected` expected)
  - `expected_rejection_matches_position_when_position_present`
    (UnsupportedCAbiType, `position = Some("Field")` -> Ok
    against `Field`; Err naming mismatch against `Output`)
  - `expected_rejection_matches_diagnostic_when_diagnostic_present`
    (DiagLogValidation, `diagnostic = Some("D1")` -> Ok
    against `D1`; Err naming mismatch against `D6`)

### Change B (continued): sidecar precision uplift

`crates/bridge-coverage-runner/matrix_metadata/rows.toml` adds
`reason = "<DiagLogFormatReason variant tag>"` to 5 RJ-031-*
rows:

| Row ID    | reason (added)            |
| --------- | ------------------------- |
| RJ-031-A  | `DebugFormatterRejected`  |
| RJ-031-B  | `PositionalArgRejected`   |
| RJ-031-C  | `NamedArgRejected`        |
| RJ-031-E  | `UnclosedBrace`           |
| RJ-031-F  | `UnmatchedClosingBrace`   |

RJ-031-D was deferred from F2 to F4 per the F2 task spec and
is not in this uplift. RJ-002 (`kind = "CVoid"`), RJ-003 (`kind
= "Atomic"`), RJ-028-B (`position = "Output"`), and RJ-028-C
(`position = "Field"`) already carried their sub-axis fields in
the F2 sidecar; the new matcher now ENFORCES those fields
(previously their values were ignored). Each row's existing
sub-axis value matches the actual `BridgeError` payload by
construction, so PASS status is preserved.

### Change C: CastKind dotted-compound encoding note

`crates/bridge-coverage-runner/snippets/README.md` gains a new
"Inspection-predicate `arg` string conventions" section noting
that:
- Most predicates take a flat variant tag (e.g., `"Add"`,
  `"Deref"`).
- `CastKind` uses the flat tag for top-level variants
  (`"IntToInt"`, `"Transmute"`, ...) but a dotted-compound
  `"PointerCoercion::<sub>"` for the four nested
  `PointerCoercion` sub-variants (e.g.,
  `"PointerCoercion::Unsize"`), preventing the
  ambiguous-collapse failure mode the flat `"PointerCoercion"`
  would cause. The dotted-compound encoding is also documented
  in the `cast_kind_tag` helper's doc-comment.

### B2: runner reports all-pass on the existing sidecar
(BACKWARD-COMPAT)

Manual Mode A invocation against the committed sidecar (post
Change A + Change B):

```
| Total rows | 32 |
| Pass | 32 |
| Fail | 0 |
| Outcomes by class | Ok=16, Rejected=16 |
```

All 32 rows PASS (10 F1 + 22 F2 committed). Zero
`Unimplemented` outcomes in any row's per-row detail string
(the variant has been removed; the dispatch always returns
`NotFound` on miss). The 5 uplifted DiagLogFormat rows now show
the precise sub-axis match enforced by the new matcher.

### B3: precision regression test (uplift backward compat)

Empirical probe via a tempdir sidecar copy:

- Before uplift (matcher pre-F3 ignoring `reason`): rows
  PASS by variant match.
- After uplift (matcher post-F3 enforcing `reason =
  "DebugFormatterRejected"` against actual
  `DebugFormatterRejected`): RJ-031-A still PASS, full sidecar
  still 32/32.
- Reverted to wrong value (`reason = "UnclosedBrace"` while
  actual remains `DebugFormatterRejected`): RJ-031-A FAILS
  with per-row detail `DiagLogFormat: expected reason
  UnclosedBrace but got DebugFormatterRejected`. The rest of
  the sidecar still PASS (31/32).

This trio confirms the matcher enforces what it claims for
the DiagLogFormat axis. The same enforcement-pattern check
applies to the (pre-existing) `kind` and `position` fields:
the new matcher elevates them from advisory to load-bearing.

### B6: IX-017 BodyAbsent probe

Empirically probed via a tempdir snippet of the F2 IX-017
round-2 shape (inlined log shim + `regseq::log::info!("event")`
with the bridge-accepting no-placeholder template). The probe
sidecar carried `inspect = { kind = "BodyAbsent" }`. Runner
output:

```
| Total rows | 1 |
| Pass | 1 |
| Fail | 0 |
| IX-017 | positive | Ok | Ok | PASS | ... OK (inspection passed) |
```

Outcome: **YES** -- the `BodyAbsent` predicate matches the
IX-017 round-2 model. The post-peel `log_info` function
instance has `body: None`, and the new dispatch arm walks
`model.instances`, succeeding on the first `Function` with
`body.is_none()`. IX-017 is unblocked for F4 re-attempt as a
committed sidecar row using `BodyAbsent` as the inspection
predicate. The follow-up `DomainAttr` / `IntrinsicAttr`
predicate from the F2 expectations file's IX-017 §
"Follow-up direction" remains a future enhancement for richer
distinguishing precision, but is no longer required to unblock
IX-017's basic coverage.

## F4 readiness signal

Dispatch coverage is now operationally complete:

- 20/20 leaf variants implemented:
  1. `TyVariant` (F1)
  2. `ItemKind` (F1)
  3. `SyntheticItemKind` (F2 mini-F3)
  4. `FunctionInstanceFlavor` (F1)
  5. `StatementKind` (F3)
  6. `TerminatorKind` (F3)
  7. `RvalueVariant` (F3)
  8. `PlaceElem` (F3)
  9. `ConstVariant` (F2 mini-F3)
  10. `AggregateKind` (F3)
  11. `CastKind` (F3, with dotted-compound `PointerCoercion`)
  12. `BinOp` (F3, 26 variants)
  13. `UnOp` (F3)
  14. `AssertKind` (F3)
  15. `NonDivergingIntrinsic` (F3)
  16. `CallCallee` (F3)
  17. `CallDivergent` (F2 mini-F3)
  18. `BodyAbsent` (F3)
  19. `Linkage` (F3)
  20. `ScalarKind` (F3, cross-variant Int/Uint/Float)
- 2/2 composites implemented:
  - `All` (F1)
  - `Any` (F1)

F4 can author rows against the full predicate vocabulary
without further mini-F3 interleaves. The sub-axis matcher now
enforces every applicable rejection sub-axis with
`Option::None => no constraint` semantics for backward
compatibility.

IX-017's escalation status is partially resolved: the
`BodyAbsent` arm shipped here is the minimum unblock per the
F2 expectations file's IX-017 § follow-up direction. IX-014
remains escalated against the IX-004/IX-010 OPEN BUG family
upstream resolution; F3 does not address it. The optional
richer `DomainAttr` / `IntrinsicAttr` predicate is deferred to
a future task (out of F3 scope per the task spec's Non-Goals:
no new enum variants).

## Out of scope (per task Non-Goals)

- New `InspectionPredicate` enum variants (e.g., `DomainAttr`,
  `IntrinsicAttr`) -- F3 only implements dispatch for the
  EXISTING 14 unimplemented arms.
- F1 runner architecture changes beyond `inspect.rs` (Change A),
  `matrix.rs` (Change B schema), and `main.rs` (Change B matcher
  + tests). `snippet.rs`, `oracle.rs`, `cluster.rs`, `report.rs`,
  `build.rs`, `Cargo.toml` unchanged.
- F0 helper modifications.
- spec.md / pretty-printer.md / generic-c-backend.md /
  bridge-coverage-matrix.md / bridge-coverage-runner-strategy.md
  changes.
- New sidecar rows. F3 is dispatch hardening; F4 is the row-set
  expansion phase. The only sidecar edit in F3 is the sub-axis
  precision uplift for the 5 DiagLogFormat rows.
- Cluster shape at `cluster.rs` (F2 surfaced no findings; simple
  hash bucketing per strategy section 8 stays as-is).
- Report shape at `report.rs` (no edits; the per-row detail
  string text is influenced by Change B's matcher detail
  strings, but that is a downstream consequence, not a
  report-shape change).
- IX-017 / IX-014 re-attempt as committed sidecar rows
  (deferred to F4 / future tasks).
- Per-IntKind / per-UintKind / per-FloatKind separate
  predicates -- ScalarKind matches the inner scalar-kind variant
  tag uniformly per strategy section 7.
- New dependencies beyond F1+F2 (`test-ir-fixtures`,
  `regseq-model`, `regseq-driver`, `toml`, `serde + derive`,
  `sha2`).
- Modifications to `tests/fixtures/playground/`.
- CI gating, JSON report output, Mode B agent integration
  (deferred to F5+ per strategy section 10).
