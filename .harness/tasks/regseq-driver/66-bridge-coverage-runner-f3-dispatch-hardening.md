# regseq-driver-66: Dispatch hardening (14 arms) + sub-axis `expected_rejection` matcher (F3 of bridge-coverage-runner)

## Description

F3 of the bridge-coverage-runner strategy per
`docs/discussions/regseq-model/bridge-coverage-runner-strategy.md`
§11. Implement the remaining 14 `InspectionPredicate` dispatch
arms left `Unimplemented` after F1 + F2, and extend the runner's
`expected_rejection_matches` to enforce sub-axis precision
(`DslRejection.kind`, `UnsupportedCAbiType.position`,
`DiagLogFormat.reason`, `DiagLogValidation.diagnostic`) for
rejected rows.

Building on F2 (regseq-driver-65, closed at `3a01888`): the
runner ships with 8 implemented dispatch arms (F1 implemented 5
leaves + 2 composites; F2 added 3 leaves) and 14 `Unimplemented`
fallthrough arms. F3 closes them out so F4's full-corpus batch
(~100 rows) has zero predicate-vocabulary gaps. The F2 expectations
file at `.harness/expectations/regseq-driver-65.md:304-333`
enumerates the 14 remaining arms and locks F3's scope.

F3 also addresses the **sub-axis matcher gap** F2 surfaced: the
F1 `expected_rejection_matches` at
`crates/bridge-coverage-runner/src/main.rs:264-299` only enforces
the top-level `BridgeError` variant tag. F2's 12 rejected rows all
PASSED because their variant matched, but the per-row
`DiagLogFormatReason` / `CAbiTypePosition` / `DslRejectionKind` /
`DiagLogValidationDiagnostic` sub-axis was NOT enforced. F3 extends
the matcher to check every applicable sub-axis field, with each
field treated as optional (absent => only variant is checked,
preserving F2 row passes).

Strategy §11 lists F3's scope as "Inspection-predicate dispatch
hardening + failure clustering polish + report shape refinement
based on F2 findings". F2 surfaced no obvious cluster-shape or
report-shape issues beyond the sub-axis matcher gap, so F3
focuses on dispatch hardening + sub-axis matcher. No cluster
or report changes are in F3 scope.

## Input Context

### Design source

- `docs/discussions/regseq-model/bridge-coverage-runner-strategy.md`
  (current change-log version v3 per the strategy doc's change
  log; the H1 title at `:1` still reads v2 pending an editorial
  sweep -- consistent with driver-65's framing).
  - §7 (Inspection predicates) -- the 22-variant enum definition.
    F3 implements the 14 remaining `Unimplemented` arms.
  - §8 (Failure clustering / fingerprint) -- unchanged; F3
    introduces no cluster-shape changes.
  - §9 (Reporting) -- unchanged; F3 introduces no report-shape
    changes beyond the sub-axis matcher's effect on per-row
    diagnostic detail strings.
  - §11 (Phasing) -- F3 = "Inspection-predicate dispatch
    hardening + failure clustering polish + report shape
    refinement based on F2 findings", estimated 2 agent-hours.

### F2 deliverable consumed

- `.harness/expectations/regseq-driver-65.md:304-333` -- F3
  readiness signal enumerates the 14 unimplemented arms (the
  source-of-truth list F3 closes out) plus the IX-017 follow-up
  note (the `BodyAbsent` arm partially unblocks the IX-017
  re-attempt; full unblock may require a future `DomainAttr`
  predicate which is OUT OF SCOPE for F3).

### F1 + F2 runner state

- `crates/bridge-coverage-runner/src/inspect.rs:34-82` defines
  the 22-variant `InspectionPredicate` enum (20 leaf variants +
  2 composites).
- `crates/bridge-coverage-runner/src/inspect.rs:140-194` --
  `variant_name(pred) -> &'static str` and the `arg_str`
  shape, complete for all 22 variants (F2 already updated).
- `crates/bridge-coverage-runner/src/inspect.rs:196-285` --
  helper functions for the 5 + 3 implemented arms:
  - `ty_variant_tag(ty)` at `:196`
  - `item_kind_tag(kind)` at `:221`
  - `synthetic_kind_variant_tag(sk)` at `:233`
  - `const_variant_tag(c)` at `:249`
  - `flavor_tag(flavor)` at `:267`
- `crates/bridge-coverage-runner/src/inspect.rs:289+` -- the
  main `check(model, pred) -> Result<(), InspectError>`
  dispatch. The 14 unimplemented arms currently fall through to
  `Err(InspectError::Unimplemented { variant, arg })`.

### Model type anchors (the 14 arms walk these slices)

- `crates/regseq-model/src/statement.rs:34` -- `StatementKind`
  (3 variants: `Assign` / `SetDiscriminant` / `Intrinsic`).
- `crates/regseq-model/src/statement.rs:64` --
  `NonDivergingIntrinsic` (2 variants: `Assume` /
  `CopyNonOverlapping`).
- `crates/regseq-model/src/terminator.rs:64` -- `TerminatorKind`
  (6 variants: `Goto` / `SwitchInt` / `Return` / `Unreachable` /
  `Call` / `Assert`).
- `crates/regseq-model/src/terminator.rs:113` -- `CallCallee`
  (2 variants: `Direct` / `Indirect`).
- `crates/regseq-model/src/terminator.rs:141` -- `AssertKind`
  (8 variants: `BoundsCheck` / `Overflow` / `OverflowNeg` /
  `DivisionByZero` / `RemainderByZero` /
  `MisalignedPointerDereference` / `NullPointerDereference` /
  `InvalidEnumConstruction`).
- `crates/regseq-model/src/rvalue.rs:19` -- `Rvalue` (9 variants:
  `Use` / `Repeat` / `Ref` / `RawPtr` / `Cast` / `BinaryOp` /
  `UnaryOp` / `Discriminant` / `Aggregate`).
- `crates/regseq-model/src/rvalue.rs:77` -- `CastKind` (11
  variants including `PointerCoercion(...)` nested at `:104`).
- `crates/regseq-model/src/rvalue.rs:104` -- `PointerCoercion`
  sub-enum (4 variants: `ReifyFnPointer` / `UnsafeFnPointer` /
  `ClosureFnPointer` / `Unsize`).
- `crates/regseq-model/src/rvalue.rs:117` -- `BinOp` (26
  variants per matrix RV-006-A..Z).
- `crates/regseq-model/src/rvalue.rs:174` -- `UnOp` (3 variants:
  `Not` / `Neg` / `PtrMetadata`).
- `crates/regseq-model/src/rvalue.rs:185` -- `AggregateKind` (7
  variants per matrix RV-009-A..G).
- `crates/regseq-model/src/place.rs:70` -- `PlaceElem` (6
  variants: `Deref` / `Field` / `Index` / `ConstantIndex` /
  `Subslice` / `Downcast`).
- `crates/regseq-model/src/item.rs:138` -- `Linkage` (4 variants:
  `Default` / `NoMangle` / `Foreign` / `Weak`).
- Scalar-kind sources (cross-variant predicate; the implementer
  confirms exact line numbers against HEAD):
  - `crates/regseq-model/src/ty.rs:81-156` carries `IntKind`,
    `UintKind`, `FloatKind` enums (declarations may be inside
    macro calls -- treat the line range as the authoritative
    region).
  - `crates/regseq-model/src/ty.rs:222-232` shows the
    `Ty::Int(IntKind)` / `Ty::Uint(UintKind)` / `Ty::Float(FloatKind)`
    constructors that the ScalarKind arm walks.

### Sub-axis matcher anchors (Change B)

- `crates/bridge-coverage-runner/src/matrix.rs` -- the
  `ExpectedRejection` struct shape (currently carries `variant`,
  `kind`, `position` per F1+F2). F3 adds `reason: Option<String>`
  and `diagnostic: Option<String>` for DiagLog sub-axes.
- `crates/bridge-coverage-runner/src/main.rs:264-299` --
  `expected_rejection_matches(err, expected) -> Result<(), String>`
  (returns `Ok(())` on match, `Err(detail)` on mismatch with a
  human-readable detail string used by the report's per-row
  detail column). F3 extends this function to enforce every
  sub-axis field that applies to the current `BridgeError`
  variant.
- `crates/regseq-driver/src/bridge/mod.rs:376` -- `BridgeError`
  enum, the value the matcher receives. The four sub-axis-bearing
  variant locations + their sub-enum declaration locations:
  - `BridgeError::DslRejection { kind: DslRejectionKind, .. }`
    variant at `crates/regseq-driver/src/bridge/mod.rs:379-384`;
    `DslRejectionKind` enum at
    `crates/regseq-driver/src/bridge/mod.rs:580-597` (8
    variants: `Box` / `CVoid` / `Atomic` / `PlaceProjection` /
    `Rvalue` / `Statement` / `Terminator` / `BasicBlock`).
  - `BridgeError::UnsupportedCAbiType { position:
    CAbiTypePosition, .. }` variant at
    `crates/regseq-driver/src/bridge/mod.rs:480-487`;
    `CAbiTypePosition` enum at
    `crates/regseq-driver/src/bridge/mod.rs:363-371` (3
    variants: `Input(usize)` / `Output` / `Field`).
  - `BridgeError::DiagLogFormat { reason: DiagLogFormatReason,
    .. }` variant at
    `crates/regseq-driver/src/bridge/mod.rs:513-518`;
    `DiagLogFormatReason` enum at
    `crates/regseq-driver/src/bridge/mod.rs:535-551` (6
    variants).
  - `BridgeError::DiagLogValidation { diagnostic:
    DiagLogValidationDiagnostic, .. }` variant at
    `crates/regseq-driver/src/bridge/mod.rs:524-529`;
    `DiagLogValidationDiagnostic` enum at
    `crates/regseq-driver/src/bridge/mod.rs:556-575` (6
    variants: `D1` / `D2` / `D4` / `D6` / `D7` / `D8`).

### Prior tasks

- regseq-driver-65 (closed at `3a01888`): F2 first snippet batch
  (22 committed + 2 escalated) + mini-F3 dispatch slice (3
  arms). F3 closes the remaining 14.
- regseq-driver-64 (closed at `9f93c2b`, rpath fix `e54ddf8`):
  F1 scaffold + 10-row demo. The architectural foundation.

## Implementation Notes

### Change A: 14 dispatch arms in `inspect.rs`

**Anchor**: extend the existing `check` function's match
expression and add 13 new helper variant-tag functions (one per arm except `BodyAbsent` which has no helper) adjacent to
the existing F1+F2 helpers at `inspect.rs:196-285`.

Per-arm specification (target shape):

**Body traversal pattern** (used by every body-walking arm
below): `RegseqModel` does NOT have a `bodies` field; bodies
live under `InstanceKind::Function(FunctionInstanceData { body:
Option<Body> })`. The F2 `CallDivergent` arm at
`crates/bridge-coverage-runner/src/inspect.rs:363-376` is the
canonical traversal:

```rust
model.instances.iter()
    .filter_map(|inst| match &inst.kind {
        InstanceKind::Function(data) => data.body.as_ref(),
        _ => None,
    })
    .flat_map(|body| body.basic_blocks.iter())
```

Every body-walking arm below uses this pattern; deeper traversal
(`bb.statements.iter()` / `bb.terminator.kind`) chains on top.

| Arm | Walks | Helper function (returns &'static str) | Argument string conventions |
| --- | ----- | -------------------------------------- | --------------------------- |
| `StatementKind(arg)` | body traversal -> `bb.statements.iter()` matching `stmt.kind` variant tag | `statement_kind_tag(&StatementKind) -> &'static str` returning `"Assign"` / `"SetDiscriminant"` / `"Intrinsic"` | flat variant tag |
| `TerminatorKind(arg)` | body traversal -> `bb.terminator.kind` (`Terminator` is non-optional per `crates/regseq-model/src/basic_block.rs:45` and F2's CallDivergent precedent) | `terminator_kind_tag(&TerminatorKind) -> &'static str` (6 variants) | flat variant tag |
| `RvalueVariant(arg)` | body traversal -> `bb.statements.iter()` -> `StatementKind::Assign(_, rvalue)` matching the `Rvalue` variant tag | `rvalue_variant_tag(&Rvalue) -> &'static str` (9 variants) | flat variant tag (`"Use"` / `"Repeat"` / `"Ref"` / `"RawPtr"` / `"Cast"` / `"BinaryOp"` / `"UnaryOp"` / `"Discriminant"` / `"Aggregate"`) |
| `PlaceElem(arg)` | body traversal -> every `Place::projection[*]` in every statement + terminator; match the `PlaceElem` variant tag at ANY projection position | `place_elem_tag(&PlaceElem) -> &'static str` (6 variants) | flat variant tag (`"Deref"` / `"Field"` / `"Index"` / `"ConstantIndex"` / `"Subslice"` / `"Downcast"`) |
| `AggregateKind(arg)` | body traversal -> every `Rvalue::Aggregate(kind, _)` in body statements; match the `AggregateKind` variant tag | `aggregate_kind_tag(&AggregateKind) -> &'static str` (7 variants) | flat variant tag |
| `CastKind(arg)` | body traversal -> every `Rvalue::Cast(kind, _, _)`; match the `CastKind` variant tag, with `PointerCoercion(sub)` flattened as `"PointerCoercion::ReifyFnPointer"` etc. | `cast_kind_tag(&CastKind) -> &'static str` (11+4 sub-variants) | dotted compound for the nested case: `"PointerCoercion::Unsize"`, `"PointerCoercion::ReifyFnPointer"`, etc.; flat for the others (`"IntToInt"`, `"Transmute"`, ...) |
| `BinOp(arg)` | body traversal -> every `Rvalue::BinaryOp(op, _, _)`; match the `BinOp` variant tag | `bin_op_tag(&BinOp) -> &'static str` (26 variants) | flat variant tag (`"Add"` / `"Sub"` / ... / `"Offset"`) |
| `UnOp(arg)` | body traversal -> every `Rvalue::UnaryOp(op, _)`; match the `UnOp` variant tag | `un_op_tag(&UnOp) -> &'static str` (3 variants) | flat variant tag (`"Not"` / `"Neg"` / `"PtrMetadata"`) |
| `AssertKind(arg)` | body traversal -> every `TerminatorKind::Assert { msg, .. }` in body terminators; match the `AssertKind` variant tag | `assert_kind_tag(&AssertKind) -> &'static str` (8 variants) | flat variant tag (`"BoundsCheck"` / `"Overflow"` / `"OverflowNeg"` / `"DivisionByZero"` / `"RemainderByZero"` / `"MisalignedPointerDereference"` / `"NullPointerDereference"` / `"InvalidEnumConstruction"`) |
| `NonDivergingIntrinsic(arg)` | body traversal -> every `StatementKind::Intrinsic(intrinsic)` in body statements; match the sub-variant tag | `non_diverging_intrinsic_tag(&NonDivergingIntrinsic) -> &'static str` (2 variants) | flat variant tag (`"Assume"` / `"CopyNonOverlapping"`) |
| `CallCallee(arg)` | body traversal -> every `TerminatorKind::Call { callee, .. }`; match the `CallCallee` variant tag | `call_callee_tag(&CallCallee) -> &'static str` (2 variants) | flat variant tag (`"Direct"` / `"Indirect"`) |
| `BodyAbsent` | walk `model.instances`; succeed if at least one `InstanceKind::Function` instance has `data.body.is_none()` | n/a (no helper) | n/a (no arg) |
| `Linkage(arg)` | walk `model.items`; match the `Linkage` variant tag from each item's per-item Linkage field | `linkage_tag(&Linkage) -> &'static str` (4 variants) | flat variant tag (`"Default"` / `"NoMangle"` / `"Foreign"` / `"Weak"`) |
| `ScalarKind(arg)` | walk `model.types`; for `Ty::Int(ik)` / `Ty::Uint(uk)` / `Ty::Float(fk)`, match the inner scalar-kind variant tag | `scalar_kind_tag_from_ty(&Ty) -> Option<&'static str>` (returns `None` for non-Int/Uint/Float Ty variants, else `"I8"` / `"U16"` / ... / `"F128"`) | flat scalar-kind variant tag |

**Argument-string conventions are normative**: the F2 sidecar
rows (e.g., TY-014-A's `"Dynamic"` arg, IK-002's `"Static"` arg)
use the flat variant tag string of the matched variant. F3
follows the same convention for the 14 new arms, with one
exception: `CastKind::PointerCoercion(sub)` uses a dotted
compound `"PointerCoercion::<sub>"` because flat `"PointerCoercion"`
would ambiguously match all four nested sub-variants. The dotted
form (e.g., `"PointerCoercion::Unsize"`) is the smallest extension
that preserves precision. Document this convention in the
`cast_kind_tag` helper's doc-comment AND in the
`crates/bridge-coverage-runner/snippets/README.md` naming
convention note.

**Match-arm shape**: each arm follows the F2 pattern -- a
single `match pred` arm that walks the relevant model slice via
`iter().any(...)`, returning `Ok(())` on first match or
`Err(InspectError::NotFound { variant, arg })` on exhaustion.
For `BodyAbsent`, the `arg` field in `NotFound` is the empty
string (matching the `arg_str` convention at
`inspect.rs:190`).

**Per-arm unit tests**: 14 new `#[cfg(test)] mod tests` unit
tests inside `inspect.rs`, mirroring the F2 pattern at
`inspect.rs` end-of-file. Each test:

1. Constructs a minimal `RegseqModel` that contains the relevant
   construct (e.g., one `StatementKind::SetDiscriminant` for the
   `StatementKind` arm).
2. Asserts the predicate `Ok(())` against a matching argument.
3. Asserts `Err(InspectError::NotFound { variant, arg })`
   against a distinct argument.

The `CastKind` arm's test MUST include an explicit
`PointerCoercion::Unsize` (or another sub-variant) probe in
addition to a flat-variant probe (e.g., `IntToInt`), because
the dotted-compound encoding is the only non-flat convention
in F3's dispatch surface. Test name suggestion:
`cast_kind_arm_matches_pointer_coercion_dotted_compound_and_flat_variants`.

Construction shapes can re-use the patterns from F2's tests
(`call_divergent_arm_matches_divergent_call_and_reports_not_found_otherwise`
at `inspect.rs` end-of-file is the most complete construction
example).

**Why not refactor the existing 8 arms first**: F1+F2 helpers
already match this shape; F3 simply extends. No refactor of the
existing dispatch (each existing arm stays as-written). Per
CLAUDE.md surgical-changes rule.

### Change B: sub-axis matcher in `main.rs` + `ExpectedRejection` schema in `matrix.rs`

**Anchor for schema**:
`crates/bridge-coverage-runner/src/matrix.rs` -- the
`ExpectedRejection` struct definition (current fields: `variant`,
`kind`, `position`). Add two optional fields:

```rust
#[derive(Debug, Deserialize)]
pub struct ExpectedRejection {
    pub variant: String,
    pub kind: Option<String>,
    pub position: Option<String>,
    pub reason: Option<String>,        // NEW: DiagLogFormatReason sub-axis
    pub diagnostic: Option<String>,    // NEW: DiagLogValidationDiagnostic sub-axis
}
```

**Anchor for matcher**:
`crates/bridge-coverage-runner/src/main.rs:264-299` --
`expected_rejection_matches(err, expected) -> Result<(), String>`.
The current shape returns `Ok(())` on variant match and
`Err(detail)` on mismatch, with the detail string consumed by
the report's per-row detail column. F3 extends the function to
enforce every applicable sub-axis field with `Option::None => no
constraint` semantics (so existing F2 sidecar rows without the
new fields remain accepting):

| `expected.variant` value | sub-axis field enforced (if `Some`) | mapping |
| ------------------------ | ----------------------------------- | ------- |
| `"DslRejection"` | `kind` | matched against `BridgeError::DslRejection { kind: DslRejectionKind }` variant tag |
| `"UnsupportedCAbiType"` | `position` | matched against `BridgeError::UnsupportedCAbiType { position: CAbiTypePosition }` variant tag (`"Input"` / `"Output"` / `"Field"`; the `Input(usize)` index is NOT matched -- just the variant) |
| `"DiagLogFormat"` | `reason` | matched against `BridgeError::DiagLogFormat { reason: DiagLogFormatReason }` variant tag |
| `"DiagLogValidation"` | `diagnostic` | matched against `BridgeError::DiagLogValidation { diagnostic: DiagLogValidationDiagnostic }` variant tag (`"D1"` / `"D2"` / `"D4"` / `"D6"` / `"D7"` / `"D8"`) |
| any other variant value | none (variant-only match) | -- |

The matcher returns `Err(detail)` if a sub-axis is `Some` but
does NOT match the actual `BridgeError` variant's sub-axis tag,
preserving the F1 `Result<(), String>` signature.

**Backwards compatibility**: F2 sidecar rows (RJ-003, RJ-005,
RJ-019, RJ-022, RJ-027, RJ-028-B, RJ-028-C, RJ-031-A through
RJ-031-F) all have the existing `variant` + (in some cases) `kind`
or `position` fields. Without the new `reason` / `diagnostic`
fields populated, those rows match exactly as before -- the
optional-field semantics preserve their PASS status.

**Sidecar precision uplift (F3 scope)**: as part of Change B,
F3 ALSO uplifts the 5 F2 DiagLogFormat rows
(RJ-031-A / RJ-031-B / RJ-031-C / RJ-031-E / RJ-031-F; note
RJ-031-D was deferred to F4 per the F2 task spec) to carry
their precise `reason` sub-axis (`"DebugFormatterRejected"`,
`"PositionalArgRejected"`, `"NamedArgRejected"`,
`"UnclosedBrace"`, `"UnmatchedClosingBrace"` respectively).
This converts the runner's report from "all DiagLogFormat
reasons collapse to one variant tag" to "each DiagLogFormat
reason has independent coverage". Cite the DiagLog reasons enum
at `crates/regseq-driver/src/bridge/mod.rs:535` for the variant
spellings.

**No additional kind/position uplifts**: RJ-002 (kind = "CVoid")
and RJ-003 (kind = "Atomic") already carry their `kind` field
in the F2 sidecar; the new matcher will now ENFORCE those fields
where it previously ignored them (backward-compat for these
rows requires the existing `kind` value to match the actual
`DslRejectionKind`, which it does empirically per F2 evidence).
Similarly RJ-028-B (position = "Output") and RJ-028-C (position =
"Field") already carry their `position` field; the matcher's new
enforcement of `position` for `UnsupportedCAbiType` rows
preserves their PASS status.

**Unit tests for the matcher**: 5 new unit tests inside
`main.rs`'s existing `#[cfg(test)] mod tests` (or
`main.rs` end-of-file `#[cfg(test)]` block if one exists today):

1. `expected_rejection_matches_variant_only_when_sub_axis_absent`:
   `expected.variant = "DslRejection"`, `expected.kind = None`,
   actual = `BridgeError::DslRejection { kind: Atomic, .. }` ->
   matcher returns `Ok(())` (no sub-axis constraint).
2. `expected_rejection_matches_kind_when_kind_present`:
   `expected.variant = "DslRejection"`, `expected.kind =
   Some("Atomic")`, actual = `DslRejection { kind: Atomic }`
   -> `Ok(())`; same expected with actual `kind: Box` -> `Err(_)`
   with the detail naming the mismatch.
3. `expected_rejection_matches_reason_when_reason_present`:
   `expected.variant = "DiagLogFormat"`, `expected.reason =
   Some("DebugFormatterRejected")`, actual = matching reason ->
   `Ok(())`; same expected with actual = `PositionalArgRejected`
   -> `Err(_)`.
4. `expected_rejection_matches_position_when_position_present`:
   `expected.variant = "UnsupportedCAbiType"`, `expected.position
   = Some("Field")`, actual variant `Field` -> `Ok(())`; same
   expected with actual `Output` -> `Err(_)`.
5. `expected_rejection_matches_diagnostic_when_diagnostic_present`:
   `expected.variant = "DiagLogValidation"`, `expected.diagnostic
   = Some("D1")`, actual = `DiagLogValidation { diagnostic: D1 }`
   -> `Ok(())`; same expected with actual `diagnostic: D6` ->
   `Err(_)`.

If the `main.rs` doesn't have a `#[cfg(test)]` mod block today,
the implementer creates one. Per CLAUDE.md surgical-changes rule,
the test block is bounded to the matcher's surface.

### Change C: harness expectations file

**New file**: `.harness/expectations/regseq-driver-66.md`.

Record:

- Before-fix evidence: F2 closed with 14 unimplemented dispatch
  arms + the F2 expectations file's documented sub-axis matcher
  gap.
- After-fix evidence: all 14 arms ship with helpers + unit
  tests; the sub-axis matcher uplifts 5 DiagLogFormat rows from
  variant-only to reason-precise matching; runner reports all
  32 rows still PASS post-uplift (BACKWARD-COMPAT property:
  uplifting sub-axis precision MUST NOT regress any F2 row).
- F4 readiness signal: the runner's dispatch coverage is now
  complete (20 leaf arms implemented; 2 composites already in
  F1; total 22/22 implemented). F4 can author rows against any
  predicate vocabulary the matrix requires. No further mini-F*
  interleaves required.
- IX-017 follow-up status: F3 implements `BodyAbsent` dispatch.
  Whether IX-017 can be re-attempted as a F4 row using
  `BodyAbsent` as the predicate is recorded as a
  YES/NO/INVESTIGATE annotation -- the implementer empirically
  probes the IX-017 round-2 snippet against the new BodyAbsent
  arm and records the outcome.
- Out-of-scope deferrals: no new enum variants (no
  `DomainAttr` / `IntrinsicAttr`); no cluster-shape changes; no
  report-shape changes beyond the sub-axis matcher's downstream
  effect on per-row detail strings.

## Non-Goals

- Do NOT add new `InspectionPredicate` enum variants. F3
  implements dispatch for the EXISTING 14 unimplemented arms.
  Any new predicate (e.g., `DomainAttr` for IX-017's full
  unblock) is a separate task.
- Do NOT modify the F1 runner architecture beyond `inspect.rs`
  (Change A), `matrix.rs` (Change B schema additions), and
  `main.rs` (Change B matcher extension + tests). The other
  five F1 modules (`snippet.rs`, `oracle.rs`, `cluster.rs`,
  `report.rs`, `build.rs`) stay untouched. `Cargo.toml`
  unchanged.
- Do NOT modify the F0 helper at
  `crates/test-ir-fixtures/src/lib.rs`.
- Do NOT modify spec.md, pretty-printer.md, generic-c-backend.md,
  bridge-coverage-matrix.md, or
  bridge-coverage-runner-strategy.md.
- Do NOT add new sidecar rows. F3 is dispatch hardening; F4 is
  the row-set expansion phase (~100 rows). The only sidecar
  edit in F3 is the OPTIONAL sub-axis precision uplift for the
  5 DiagLogFormat rows (Change B).
- Do NOT modify the cluster shape at `cluster.rs`. F2 surfaced no
  cluster-shape findings; the simple-hash bucketing per strategy
  §8 stays as-is.
- Do NOT modify the report shape at `report.rs` (other than
  the per-row detail string text that surfaces the sub-axis
  matcher's enforcement -- and that is a DOWNSTREAM consequence
  of Change B, not a separate report-shape edit).
- Do NOT re-attempt IX-017 or IX-014 as committed sidecar rows.
  Those are F4 / future-task decisions; F3 only records whether
  the new `BodyAbsent` arm unblocks IX-017 (in the expectations
  file).
- Do NOT pursue ScalarKind dispatch with sub-variants of
  `Int(IntKind)` / `Uint(UintKind)` / `Float(FloatKind)`
  separately. The single `ScalarKind` arm matches the inner
  variant tag uniformly (per strategy §7); no per-IntKind /
  per-UintKind / per-FloatKind separation.
- Do NOT add new dependencies. F3 uses only the F1+F2 deps
  (`test-ir-fixtures`, `regseq-model`, `regseq-driver`, `toml`,
  `serde + derive`, `sha2`).
- Do NOT modify `tests/fixtures/playground/`.
- Do NOT add CI gating, JSON report output, or Mode B agent
  integration (deferred to F5+ per strategy §10).

## Test Plan

### B1: workspace stays green

`cargo test --workspace -- --test-threads=1` green:

- All existing F0/F1/F2 tests pass unchanged.
- 14 new unit tests inside `inspect.rs` for the F3 dispatch
  arms pass.
- 5 new unit tests inside `main.rs` for the sub-axis matcher
  pass.
- The 3 F1 smoke tests at
  `crates/bridge-coverage-runner/tests/smoke.rs` pass
  unchanged.

### B2: runner reports all-pass on the existing sidecar (BACKWARD-COMPAT)

After Change A + Change B land, the manual Mode A invocation
produces a `report.md` with `Total rows = 32`, `Pass = 32`,
`Fail = 0` (unchanged from F2's post-close state). Specifically:

- Every F1 row (10 rows, 5 leaf + 2 composite predicates)
  remains PASS.
- Every F2 row (22 rows, 8 leaf + 2 composite predicates +
  per-row sub-axis enforcement post-uplift) remains PASS.
- Zero `Unimplemented` outcomes in any row's per-row detail
  string in the report.
- For the 5 uplifted DiagLogFormat rows (`reason` added to
  RJ-031-A / RJ-031-B / RJ-031-C / RJ-031-E / RJ-031-F), the
  per-row diagnostic detail reflects the precise sub-axis match.
  No additional uplifts to RJ-002/RJ-003 are needed (they
  already carry `kind`; the matcher's new enforcement of `kind`
  preserves their PASS status by construction).

### B3: precision regression test (uplift backward compat)

The 5 newly uplifted DiagLogFormat rows MUST NOT regress under
the new matcher (and the existing F2 rows that already carry
`kind` / `position` fields -- RJ-002 / RJ-003 / RJ-028-B /
RJ-028-C -- MUST continue to PASS as the matcher now ENFORCES
those previously-ignored fields). For
each uplift candidate, the implementer empirically verifies:

- Before uplift: row PASSES (variant-only match).
- After uplift (sub-axis field added to sidecar): row STILL
  PASSES (variant + sub-axis both match).
- If the uplift is reverted (sub-axis field set to a WRONG
  value), row FAILS.

This trio confirms the matcher enforces what it claims. The
"wrong value" check is a one-off implementer probe; no
committed test artifact required.

### B4: `cargo fmt` + `cargo clippy` clean

`cargo fmt --all -- --check` and `cargo clippy --workspace
--all-targets -- -D warnings` clean.

### B5: F0 + F1 + F2 surface unchanged

`git diff main -- crates/test-ir-fixtures/` empty.
`git diff main -- crates/bridge-coverage-runner/src/snippet.rs
crates/bridge-coverage-runner/src/oracle.rs
crates/bridge-coverage-runner/src/cluster.rs
crates/bridge-coverage-runner/src/report.rs
crates/bridge-coverage-runner/build.rs
crates/bridge-coverage-runner/Cargo.toml
crates/bridge-coverage-runner/snippets/` empty (no snippet
additions, no other F1 module touches).

### B6: IX-017 BodyAbsent probe

The implementer probes the IX-017 round-2 snippet shape (per F2
expectations file's IX-017 §) against the newly-implemented
`BodyAbsent` predicate. Outcome recorded in the harness
expectations file as one of:

- **YES**: BodyAbsent matches the IX-017 round-2 snippet's
  model (some Function instance has `body == None`); IX-017 is
  unblocked for F4 re-attempt.
- **NO**: BodyAbsent does NOT match; IX-017 needs a different
  predicate (likely the deferred `DomainAttr` / `IntrinsicAttr`).
- **INVESTIGATE**: probe outcome ambiguous; recorded for F4
  decision.

This is documentation-only in F3; IX-017's re-attempt as a
committed sidecar row is F4 work.

## Review

### Files to send to Codex (`scripts/python/codex_runner.py`)

- This file
  (`.harness/tasks/regseq-driver/66-bridge-coverage-runner-f3-dispatch-hardening.md`).
- The runner strategy doc
  (`docs/discussions/regseq-model/bridge-coverage-runner-strategy.md`)
  as design source-of-truth (§7, §8, §9, §11).
- The F2 expectations file
  (`.harness/expectations/regseq-driver-65.md`) as the
  F3-readiness-signal source enumerating the 14 unimplemented
  arms.

### Reviewer focus

- **Dispatch-arm completeness**: do the 14 specified arms cover
  the F2 expectations file's 14 unimplemented enumeration
  EXACTLY? Any over-coverage or under-coverage?
- **Argument-string conventions**: are the per-arm argument
  conventions (flat tag, dotted compound for CastKind's
  PointerCoercion) correct against the model enum spellings?
- **CastKind PointerCoercion encoding**: is the dotted
  `"PointerCoercion::Unsize"` encoding the cleanest extension,
  or should the predicate be split into two arms (`CastKind`
  for the top-level variants, `PointerCoercionKind` for the
  nested four)? Note: splitting would expand the enum by 1
  variant, which is a Non-Goal in this task.
- **ScalarKind cross-variant semantics**: the spec says the
  `ScalarKind` arm matches the inner scalar-kind variant tag
  across Ty::Int / Ty::Uint / Ty::Float. Is this the intended
  shape, or does the strategy expect three separate predicates
  (IntKind / UintKind / FloatKind)? Cross-reference strategy §7
  description.
- **Sub-axis matcher backward-compat**: does the
  `Option::None => no constraint` semantics preserve every F2
  sidecar row's PASS status? Audit RJ-003 (kind), RJ-019 / RJ-022
  / RJ-027 / RJ-028-B / RJ-028-C (variant-only or position
  only), and RJ-031-A/B/C/E/F (none had reason today).
- **BodyAbsent IX-017 unblock**: is the BodyAbsent probe in B6
  the right granularity? Should F3 commit IX-017 as a row if
  BodyAbsent matches? Note: spec explicitly defers that to F4.
- **Sidecar precision uplift scope (Change B)**: the spec
  newly uplifts 5 F2 sidecar rows (RJ-031-A / RJ-031-B /
  RJ-031-C / RJ-031-E / RJ-031-F) with `reason` precision, AND
  the existing `kind` / `position` fields on RJ-002 / RJ-003 /
  RJ-028-B / RJ-028-C are now ENFORCED by the extended matcher
  (no new sidecar field for those four, but their previously-
  unenforced values are now load-bearing). Is this combined
  scope in F3 per strategy §11's "report shape refinement based
  on F2 findings", or should the uplift split into a separate
  F3.5 task?
- **Out-of-scope discipline**: F3 enumerates 12 Non-Goals.
  Confirm Implementation Notes does not creep into any.
- **Test coverage**: 14 dispatch arm tests + 5 matcher tests =
  19 new tests. Are any per-arm tests likely to be flaky or
  brittle?
- **Citation drift**: every file:line citation against HEAD.

### Adversarial prompt template

```
<task>
Review .harness/tasks/regseq-driver/66-bridge-coverage-runner-f3-dispatch-hardening.md.

Identify:
- Dispatch-arm completeness (14 arms vs. F2 expectations file enumeration).
- Argument-string convention correctness against model enum spellings.
- CastKind PointerCoercion encoding (dotted compound vs. enum split).
- ScalarKind cross-variant semantics (one arm vs. three).
- Sub-axis matcher backward-compat against F2 sidecar.
- Sidecar precision uplift scope decision.
- BodyAbsent + IX-017 probe granularity.
- Out-of-scope creep against the 12 Non-Goals.
- Test coverage adequacy and flake risk.
- Citation drift against HEAD.
</task>

<grounding_rules>
- Cite file:line for every claim.
- Mark hypotheses as hypotheses.
- For argument-string conventions, inspect crates/regseq-model/src/{statement,terminator,rvalue,place,item,ty}.rs for enum variant spellings.
- For sub-axis matcher backward-compat, inspect crates/bridge-coverage-runner/matrix_metadata/rows.toml for the current F2 sidecar entries' shapes.
- For BodyAbsent IX-017 probe, the IX-017 round-2 snippet's outcome is documented in .harness/expectations/regseq-driver-65.md §"IX-017 escalated-after-attempt".
</grounding_rules>

<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
DISPATCH-ARM COMPLETENESS CHECK: PASS / FAIL with citation
ARGUMENT-STRING CONVENTION CHECK: PASS / FAIL with per-arm citation
CASTKIND POINTERCOERCION ENCODING: AGREE_DOTTED | DISAGREE_<alternative>
SCALARKIND CROSS-VARIANT SEMANTICS: AGREE_UNIFIED | DISAGREE_<split>
SUB-AXIS MATCHER BACKWARD-COMPAT CHECK: PASS / FAIL with citation
SIDECAR PRECISION UPLIFT SCOPE: AGREE_IN_F3 | DISAGREE_<reframe>
BODYABSENT IX-017 PROBE GRANULARITY: AGREE_PROBE_ONLY | DISAGREE_<alternative>
OUT-OF-SCOPE DISCIPLINE: PASS / FAIL
TEST COVERAGE ADEQUACY: PASS / FAIL
CITATION DRIFT: list or "none"
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
- `cargo test --workspace -- --test-threads=1` green. Specifically:
  - All existing F0/F1/F2 tests pass unchanged.
  - 14 new unit tests in `inspect.rs` for the F3 dispatch arms
    pass.
  - 5 new unit tests in `main.rs` for the sub-axis matcher
    pass.
- Manual Mode A invocation per B2 produces `report.md` with
  `Total rows = 32`, `Pass = 32`, `Fail = 0` (no regression
  from F2 close-state).
- B3 precision regression check confirms each uplift's
  pre/post/wrong-value behavior matches the matcher contract.
- B6 IX-017 BodyAbsent probe outcome (YES / NO / INVESTIGATE)
  recorded in the expectations file.
- `git diff --stat main..` shows changes confined to:
  - `crates/bridge-coverage-runner/src/inspect.rs` (Change A: 14
    arms + 13 helpers + 14 unit tests)
  - `crates/bridge-coverage-runner/src/matrix.rs` (Change B:
    `ExpectedRejection` schema +2 optional fields)
  - `crates/bridge-coverage-runner/src/main.rs` (Change B:
    matcher extension + 5 unit tests)
  - `crates/bridge-coverage-runner/matrix_metadata/rows.toml`
    (Change B: sub-axis precision uplift for 5 DiagLogFormat
    rows by adding `reason` fields)
  - `crates/bridge-coverage-runner/snippets/README.md` (Change A:
    CastKind dotted-compound encoding note)
  - `.harness/expectations/regseq-driver-66.md` (Change C)
  - `.harness/tasks.json` (registration entry)
- No F0 modifications.
- No `snippet.rs` / `oracle.rs` / `cluster.rs` / `report.rs` /
  `build.rs` / `Cargo.toml` modifications.
- No new snippet files; no new sidecar rows (only the precision
  uplift on existing rows).
- No bridge / model / spec / pretty-printer / generic-c-backend
  changes.
- Codex Task Review verdict: APPROVED or APPROVED_WITH_MINOR_FIXES.

## Acceptance Criteria

1. **All 14 dispatch arms land**: `crates/bridge-coverage-runner/src/inspect.rs`
   extends `check` with implemented arms for `StatementKind`,
   `TerminatorKind`, `RvalueVariant`, `PlaceElem`,
   `AggregateKind`, `CastKind`, `BinOp`, `UnOp`, `AssertKind`,
   `NonDivergingIntrinsic`, `CallCallee`, `BodyAbsent`,
   `Linkage`, `ScalarKind`. The fallthrough arm is now
   exclusively `All`/`Any` composites (which already dispatch).
   Zero `Unimplemented` returns from the demo/F2 sidecar.

2. **13 new helper variant-tag functions (one per arm except `BodyAbsent` which has no helper) land**: one per arm (excluding
   `BodyAbsent` which has no helper) plus `scalar_kind_tag_from_ty`
   for the cross-variant ScalarKind dispatch. Helper naming matches
   the F1+F2 pattern (`<thing>_tag` returning `&'static str`).

3. **14 unit tests for dispatch arms land**: in `inspect.rs`'s
   `#[cfg(test)] mod tests` block. Each test constructs a
   minimal `RegseqModel` with the relevant construct +
   asserts the predicate's Ok-on-match / NotFound-on-mismatch
   behavior.

4. **Sub-axis matcher extension lands**:
   `crates/bridge-coverage-runner/src/main.rs:264+` extends
   `expected_rejection_matches` to enforce `kind` / `position` /
   `reason` / `diagnostic` sub-axes per the Change B table when
   the corresponding field is `Some`. `None` semantics preserve
   variant-only matching.

5. **`ExpectedRejection` schema extends**: adds `reason:
   Option<String>` and `diagnostic: Option<String>` fields in
   `crates/bridge-coverage-runner/src/matrix.rs`.

6. **5 DiagLogFormat rows uplifted to reason precision**:
   `crates/bridge-coverage-runner/matrix_metadata/rows.toml`
   adds `reason = "<variant tag>"` to RJ-031-A / RJ-031-B /
   RJ-031-C / RJ-031-E / RJ-031-F (5 rows; RJ-031-D was
   deferred from F2 to F4 per the F2 task spec). RJ-002 /
   RJ-003 retain their existing `kind` field unchanged; the new
   matcher enforces those fields by construction.

7. **Matcher tests land**: 5 new unit tests in `main.rs`
   exercising the sub-axis enforcement matrix (variant-only,
   kind-precise, reason-precise, position-precise,
   diagnostic-precise).

8. **CastKind dotted-compound encoding documented**:
   `crates/bridge-coverage-runner/snippets/README.md` notes the
   `"PointerCoercion::<sub>"` convention for CastKind sidecar
   args.

9. **Runner exits 0 with no regressions**: manual Mode A run
   produces `Total rows = 32`, `Pass = 32`, `Fail = 0`. The
   uplifted rows show their precise sub-axis match in their
   per-row detail strings.

10. **B6 IX-017 BodyAbsent probe recorded**: harness
    expectations file at `.harness/expectations/regseq-driver-66.md`
    documents the probe outcome (YES / NO / INVESTIGATE) for
    F4's row-selection decision.

11. **F1+F2 architecture untouched**: `git diff main --
    crates/bridge-coverage-runner/src/snippet.rs
    crates/bridge-coverage-runner/src/oracle.rs
    crates/bridge-coverage-runner/src/cluster.rs
    crates/bridge-coverage-runner/src/report.rs
    crates/bridge-coverage-runner/build.rs
    crates/bridge-coverage-runner/Cargo.toml` returns zero.

12. **F0 untouched**: `git diff main --
    crates/test-ir-fixtures/` returns zero.

13. **No scope creep**: `git diff main` touches ONLY the files
    listed in Quality Gate's `git diff --stat` enumeration. No
    bridge / model / spec / pretty-printer / generic-c-backend
    / matrix / strategy doc changes.

14. **F4 readiness signal**: the harness expectations file's
    closing note states that the runner's dispatch coverage is
    operationally complete (20/20 leaf arms + 2 composites). F4
    can author rows against the full predicate vocabulary
    without further mini-F3 interleaves. This is the bridge
    between F3 and F4.
