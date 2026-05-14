# regseq-driver-41: Place + Operand + Rvalue + Statement Bridge

## Description

Translate the body-content vocabulary from rustc's MIR into the
`regseq-model` surface, bottom-up: Place per `spec.md` §19.5, Operand
per §20.3 (including the §20.3.1 RuntimeChecks operand-surface
lowering), Rvalue per §21.6, Statement per §22.6, plus Statement-site
rejections per §22.5.1.

This is the per-construct body-content layer. It runs during Phase 5
body materialisation (regseq-driver-43) but the per-construct
translators are factored into this task as standalone callable
functions consumed by regseq-driver-42 (Terminator) and
regseq-driver-43 (Body materialisation).

## Input Context

- `docs/discussions/regseq-model/spec.md` §19 (Place + projections;
  §19.5 translation rules)
- `docs/discussions/regseq-model/spec.md` §20 (Operand + LiteralValue;
  §20.3 translation rules; §20.3.1 RuntimeChecks lowering)
- `docs/discussions/regseq-model/spec.md` §21 (Rvalue + sub-enums;
  §21.6 translation rules)
- `docs/discussions/regseq-model/spec.md` §22 (Statement +
  StatementKind; §22.5.1 statement-site rejections; §22.6
  translation rules)
- `crates/regseq-model/src/place.rs` (Place + PlaceElem from
  regseq-model-76)
- `crates/regseq-model/src/operand.rs` (Operand + LiteralValue from
  regseq-model-77)
- `crates/regseq-model/src/rvalue.rs` (Rvalue + sub-enums from
  regseq-model-78)
- `crates/regseq-model/src/statement.rs` (Statement + StatementKind +
  NonDivergingIntrinsic from regseq-model-79)
- `crates/regseq-driver/src/bridge/mod.rs` (BridgeCtxt + BridgeError
  from regseq-driver-36)
- `crates/regseq-driver/src/bridge/ty.rs`,
  `crates/regseq-driver/src/bridge/const_.rs` (ty + const translation
  from regseq-driver-37, regseq-driver-38)
- `crates/regseq-driver/src/bridge/fnsig.rs` (FnSig translation from
  regseq-driver-39)

## Implementation Notes

Create the body-content translators under
`crates/regseq-driver/src/bridge/body/`. Wire `pub mod body;` from
`crates/regseq-driver/src/bridge/mod.rs`.

### Module structure

- `crates/regseq-driver/src/bridge/body/mod.rs` -- shared per-body
  translation context; re-exports for Terminator + body materialisation.
- `crates/regseq-driver/src/bridge/body/place.rs` -- Place translation
  per spec.md §19.5.
- `crates/regseq-driver/src/bridge/body/operand.rs` -- Operand
  translation per spec.md §20.3 + §20.3.1 RuntimeChecks lowering.
- `crates/regseq-driver/src/bridge/body/rvalue.rs` -- Rvalue
  translation per spec.md §21.6.
- `crates/regseq-driver/src/bridge/body/statement.rs` -- Statement
  translation per spec.md §22.6 + §22.5.1 rejections.

### Public API

```rust
// bridge/body/place.rs

/// Translate a rustc `mir::Place` into a `regseq_model::Place`. Per
/// spec.md §19.5.
pub fn translate_place<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    body_cx: &mut BodyCtxt,
    place: rustc_middle::mir::Place<'tcx>,
) -> Result<regseq_model::Place, BridgeError>;
```

```rust
// bridge/body/operand.rs

/// Translate a rustc `mir::Operand` into a `regseq_model::Operand`.
/// Per spec.md §20.3.
pub fn translate_operand<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    body_cx: &mut BodyCtxt,
    operand: rustc_middle::mir::Operand<'tcx>,
) -> Result<regseq_model::Operand, BridgeError>;

/// Lower the RuntimeChecks operand surface per spec.md §20.3.1.
pub fn lower_runtime_check_operand<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    body_cx: &mut BodyCtxt,
    check: rustc_middle::mir::Operand<'tcx>,
) -> Result<regseq_model::Operand, BridgeError>;
```

```rust
// bridge/body/rvalue.rs

/// Translate a rustc `mir::Rvalue` into a `regseq_model::Rvalue`. Per
/// spec.md §21.6 dispatch table over the 9-variant Rvalue + the
/// per-row sub-enum mapping (CastKind / BorrowKind / AggregateKind /
/// BinOp / UnOp).
pub fn translate_rvalue<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    body_cx: &mut BodyCtxt,
    rvalue: rustc_middle::mir::Rvalue<'tcx>,
) -> Result<regseq_model::Rvalue, BridgeError>;
```

```rust
// bridge/body/statement.rs

/// Translate a rustc `mir::Statement` into a `regseq_model::Statement`.
/// Per spec.md §22.6 + §22.5.1 statement-site rejections.
pub fn translate_statement<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    body_cx: &mut BodyCtxt,
    stmt: rustc_middle::mir::Statement<'tcx>,
) -> Result<Option<regseq_model::Statement>, BridgeError>;
```

`Option<Statement>` accommodates rustc statements that elide at the
DSL boundary per spec.md §22.6 (e.g. `StorageLive` / `StorageDead`
where the spec elects elision).

### Per-construct translation rules

- Place (spec.md §19.5): walk the local + projection chain; map each
  rustc `ProjectionElem` to a `PlaceElem` per the closed 6-variant
  enum.
- Operand (spec.md §20.3): map Copy / Move / Constant per the §20.3
  dispatch.
- RuntimeChecks operand surface (spec.md §20.3.1): the
  bridge-side operand-form lowering for runtime-check operands.
- Rvalue (spec.md §21.6): dispatch over the 9 Rvalue arms; sub-enum
  mapping for CastKind, BorrowKind, AggregateKind, BinOp (26
  variants), UnOp.
- Statement (spec.md §22.6 + §22.5.1): dispatch over the 3-variant
  StatementKind; §22.5.1 statement-site rejections raise
  `BridgeError::DslRejection`.

### Drop-glue trigger interaction

Per the dependency on regseq-driver-40, when Statement / Rvalue
translation visits a non-Copy Place ownership transfer it eagerly
triggers `bridge::synth::synthesise_drop_glue` for the moved Ty so
later body materialisation has the drop-glue Item available.

## Non-Goals

- Terminator translation + Drop-as-Call collapse (regseq-driver-42).
- Basic-block translation (regseq-driver-42).
- Body materialisation + LocalDecls allocation API (regseq-driver-43).
- Vtable-indirect rewrite (regseq-driver-43).
- Diagnostic-logging recognition (regseq-driver-44).
- DiagLogPeel + format-template parsing (regseq-driver-45).

## Test Plan

### Unit tests (`crates/regseq-driver/src/bridge/body/*` `#[cfg(test)] mod tests`)

- `place.rs`: for each of the 6 closed `PlaceElem` variants per
  regseq-model-76, hand-construct a rustc place that exercises the
  variant; assert the translated PlaceElem matches.
- `operand.rs`: for each of Copy / Move / Constant, assert the
  translated Operand matches.
- `rvalue.rs`: for each of the 9 Rvalue arms per regseq-model-78,
  assert the translated Rvalue matches; for each BinOp variant
  per the 26-variant enum, assert the translated BinOp matches.
- `statement.rs`: for each StatementKind variant (Assign /
  SetDiscriminant / Intrinsic), assert the translated Statement
  matches.

### Integration tests (`crates/regseq-driver/tests/bridge_body.rs`)

Bridge tests are integration-in-tests because they need rustc-compiled
fixtures.

Positive coverage (one fixture per major construct row):

- Place fixtures (one per PlaceElem variant): Deref via
  `*p`; Field via `s.x`; Index via `arr[i]`; ConstantIndex via slice
  pattern; Subslice via slice subrange pattern; Downcast via match
  on enum variant + field access.
- Operand: Copy via `let _ = x;` where x is Copy; Move via
  `let _ = s;` where s is non-Copy; Constant via `let _ = 1u32;`.
- RuntimeChecks: a fixture exercising a runtime-check operand surface
  per spec.md §20.3.1.
- Rvalue: one fixture per arm (Use / Repeat / Ref / RawPtr /
  ThreadLocalRef / AddressOf if applicable / Len / Cast / BinaryOp /
  CheckedBinaryOp / UnaryOp / Discriminant / Aggregate / ShallowInitBox
  per the spec.md §21.6 rows the data shape supports).
- Statement: one fixture per StatementKind (Assign / SetDiscriminant
  / Intrinsic).

Negative coverage (one fixture per spec.md §22.5.1 statement-site
rejection row):

- A fixture using a statement form that §22.5.1 lists as rejected --
  assert `BridgeError::DslRejection` with the rejection-specific
  variant.

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-driver-41 against its task spec at
.harness/tasks/regseq-driver/41-place-operand-rvalue-statement-bridge.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-driver/src/bridge/mod.rs
- crates/regseq-driver/src/bridge/body/mod.rs
- crates/regseq-driver/src/bridge/body/place.rs
- crates/regseq-driver/src/bridge/body/operand.rs
- crates/regseq-driver/src/bridge/body/rvalue.rs
- crates/regseq-driver/src/bridge/body/statement.rs
- crates/regseq-driver/tests/bridge_body.rs
- (any new fixture files under crates/regseq-driver/tests/fixtures/)

Focus on:
- Spec deviations vs spec.md §19.5 / §20.3 / §20.3.1 / §21.6 / §22.6 /
  §22.5.1.
- Per-PlaceElem / per-Operand / per-Rvalue arm / per-StatementKind row
  coverage.
- Drop-glue synthesis trigger on non-Copy Place ownership transfer
  (consumes regseq-driver-40's synthesise_drop_glue).
Do NOT flag anything explicitly marked out of scope (Non-Goals,
later task IDs). Terminator translation and Drop-as-Call are
regseq-driver-42; body materialisation is regseq-driver-43.
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

- `crates/regseq-driver/src/bridge/body/{mod,place,operand,rvalue,statement}.rs`
  created and wired from `bridge/mod.rs`.
- Per-construct translators implemented per spec.md §19.5 / §20.3 /
  §20.3.1 / §21.6 / §22.6.
- Each translation rule covered by a positive test.
- Each spec.md §22.5.1 rejection row covered by a negative integration
  fixture.
- All tests pass; Codex Task Review verdict APPROVED.
