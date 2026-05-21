# regseq-driver-64: implementation evidence

Implementation deliverable for `regseq-driver-64`, the F1 scaffold
of the bridge-coverage-runner per
`.harness/tasks/regseq-driver/64-bridge-coverage-runner-f1-scaffold.md`
and the bridge-coverage-runner strategy at
`docs/discussions/regseq-model/bridge-coverage-runner-strategy.md`
section 3 (architecture) + section 11 (phasing -- F1 deliverable).

Parent context: this task consumes the F0 typed-outcome surface
shipped by regseq-driver-63 (closed at `174c7ff`) read-only.

## Scope

- New workspace member `crates/bridge-coverage-runner/` with the
  seven binary-crate source modules per strategy section 3
  (`main.rs`, `matrix.rs`, `snippet.rs`, `oracle.rs`, `inspect.rs`,
  `cluster.rs`, `report.rs`). No `src/lib.rs` (binary-only in F1).
- 10-row matrix metadata sidecar at
  `crates/bridge-coverage-runner/matrix_metadata/rows.toml`.
- 10 hand-coded `#![no_std]` snippets at
  `crates/bridge-coverage-runner/snippets/`.
- End-to-end smoke tests at
  `crates/bridge-coverage-runner/tests/smoke.rs`.
- One-line workspace member addition to `Cargo.toml`.

No edits to other crates. No edits to the F0 helper at
`crates/test-ir-fixtures/src/lib.rs`. No spec / strategy / matrix
doc edits.

## Before-fix evidence

- Pre-fix HEAD `c597c06` carried only the F1 task-spec registration
  commit. `crates/bridge-coverage-runner/` did NOT exist. The
  workspace `Cargo.toml` did NOT list it.
- The matrix-coverage workflow before F1: every uncovered matrix
  row would require an individual `regseq-driver-NN` task with its
  own fixture authoring -- the bridge-coverage-runner strategy
  exists specifically to replace that pattern.
- The F0 surface (`try_bridge_fixture_path` +
  `BridgeOutcome`) existed but had zero downstream callers outside
  the F0 smoke test at
  `crates/test-ir-fixtures/tests/typed_outcome_smoke.rs`.

## After-fix evidence

### Change A: workspace registration + crate manifest + ancillary files

- `Cargo.toml:3-15` now lists `crates/bridge-coverage-runner` as
  the first member (alphabetical ordering preserved).
- `crates/bridge-coverage-runner/Cargo.toml` declares the binary
  crate with dependencies `test-ir-fixtures` (path),
  `regseq-model` (path), `regseq-driver` (path), `toml = "0.8"`,
  `serde = "1"` (with `derive`), `sha2 = "0.10"`. No `clap`,
  `tempfile`, `assert_cmd`, or `regex` per Non-Goals.
- `crates/bridge-coverage-runner/.gitignore` lists `reports/`.
- `crates/bridge-coverage-runner/snippets/README.md` documents the
  row-id naming convention.

### Change B: `src/matrix.rs` -- TOML loader + subset-OK drift check

- `crates/bridge-coverage-runner/src/matrix.rs` defines
  `RowsFile`, `Row`, `Expectation`, `ExpectedRejection`, the
  `MatrixError` enum, and the `DriftReport` shape.
- `matrix::load(metadata_path: &Path) -> Result<RowsFile,
  MatrixError>` parses TOML via `toml = "0.8"`.
- `matrix::check_drift(rows, matrix_md_path) ->
  Result<DriftReport, MatrixError>` implements the
  **subset-OK** drift check: `missing_in_matrix` rows hard-fail
  via `MatrixError::DriftHardFail`; `missing_in_sidecar` rows
  surface as informational data only. This deviates from
  strategy section 4.1's strict shape; the deviation is
  documented in the module-level doc-comment.
- Row-ID extraction (`extract_matrix_row_ids` +
  `is_row_id`) is hand-rolled per Non-Goals (no `regex` dep).
  Five unit tests under `#[cfg(test)] mod tests` cover the row-ID
  pattern (top-level / sub-axis / nested / negative cases) and the
  table-extraction pass.

### Change C: `src/snippet.rs` -- per-snippet load + name<->row mapping

- Defines `Snippet { path, hash_prefix, byte_len }` and
  `load_snippets(snippets_dir, rows) -> Result<Vec<Snippet>,
  SnippetError>`.
- Computes SHA-256 first 8 hex chars via `sha2::Sha256` per
  strategy section 8.
- Validator-failing rows surface as
  `SnippetError::ValidatorFailingNotSupported` per strategy
  section 2 (out of scope for F1).

### Change D: `src/oracle.rs` -- subprocess rustc pre-flight + bridge invocation

- Defines `OracleOutcome` with `Ok(Box<RegseqModel>)` /
  `ValidatorFailed` / `Rejected` / `RustcCleanReject` /
  `RustcICE` / `RustcNoAfterAnalysis` / `Panic` variants. The
  `Ok` variant boxes `RegseqModel` to satisfy clippy's
  `large_enum_variant` lint (documented in the doc-comment).
- `oracle::invoke(snippet_path)` runs the two-step algorithm:
  1. Subprocess rustc with `--crate-type lib --edition 2021
     --emit=metadata -o <tmp.rmeta>` per OPEN QUESTION (c)
     adjudication -- `--emit=metadata` is the stable equivalent
     to strategy section 6.3's `-Zno-codegen`.
  2. On rustc success, `try_bridge_fixture_path` inside
     `std::panic::catch_unwind(AssertUnwindSafe(...))` per
     strategy section 6.2.
- Subprocess temp path is deterministic per snippet stem;
  cleanup runs unconditionally via `fs::remove_file` after
  the subprocess returns. No `tempfile` dep per Non-Goals.

### Change E: `src/inspect.rs` -- InspectionPredicate enum + dispatch

- Defines the **complete** 23-variant `InspectionPredicate`
  enum per strategy section 7.
- Dispatch arms implemented for the 5 demo-set predicates
  (`TyVariant`, `ItemKind`, `FunctionInstanceFlavor`, `All`,
  `Any`); the remaining 18 leaf variants fall through to
  `InspectError::Unimplemented { variant, arg }` so the F3
  follow-up scope is visible per-row.
- Strategy section 7 deviation: `All`/`Any` ship as
  newtype-variant `Vec<InspectionPredicate>` (not struct
  variants) so the adjacently-tagged serde representation
  matches the natural TOML array shape. Documented in the
  module doc-comment.

### Change F: `src/cluster.rs` -- fingerprint + grouping

- Defines `RowResult`, `ActualOutcomeTag`, `Cluster`, `Fingerprint`,
  `ClusterMember` types.
- `cluster::group(results) -> Vec<Cluster>` hashes the
  fingerprint tuple via `DefaultHasher` and buckets by hash.
  Members sorted by snippet length (shortest first); clusters
  sorted by member count descending then by cluster-key
  ascending.
- `bridge_error_variant(err)` exhaustively maps every
  `BridgeError` variant to a static tag string for the
  `variant_or_phase` fingerprint component. No string parsing
  on error messages -- typed dispatch only.

### Change G: `src/report.rs` -- markdown report emitter

- `report::render(metadata_path, snippets_dir, results,
  clusters, drift)` emits the markdown shape from strategy
  section 9: header / summary table / drift report / per-row
  outcomes table / failure clusters with representative
  repros.
- ISO 8601 timestamp via in-module `unix_to_ymdhms` (Howard
  Hinnant civil-from-days). No `chrono` dep per Non-Goals.

### Change H: `src/main.rs` -- CLI + run loop

- Hand-rolled CLI per Non-Goals (no `clap`). Four required
  flags: `--metadata`, `--snippets`, `--matrix-md`, `--out`.
- Run loop: load sidecar -> drift check (hard-fail on invented
  IDs) -> load snippets -> per-row `oracle::invoke` +
  `classify` -> cluster::group -> report::render -> write
  `report.md` -> exit 0 on all-pass, exit 1 on any-fail, exit
  2 on configuration / IO error.
- Per-row classification matrix per Change G of the task spec:
  positive + Ok + inspection-pass -> PASS; positive +
  inspection-unimpl -> FAIL (F3 follow-up); rejected + matching
  variant -> PASS; all other shapes -> FAIL with detailed
  diagnostic in the per-row detail column.

### Change I: `matrix_metadata/rows.toml` -- 10 demo rows

- 6 positive rows: TY-018 (`TyVariant=Enum`), IK-002
  (`ItemKind=Static`), IK-005 (`ItemKind=Struct`), TY-014-A
  (`Any(TyVariant=Dynamic|Never)`), FF-011
  (`FunctionInstanceFlavor=DropGlue`), IX-002
  (`All(TyVariant=Struct, ItemKind=Function)`).
- 4 rejected rows: RJ-002 (`DslRejection::CVoid`), RJ-016
  (`Coroutine`), RJ-026 (`UnsupportedAbi`), RJ-028-A
  (`UnsupportedCAbiType::Input`).
- Every row carries explicit `cluster_key` per strategy
  section 8.

### Change J: 10 hand-coded `#![no_std]` snippets

- Files at `crates/bridge-coverage-runner/snippets/`:
  `TY-018.rs`, `IK-002.rs`, `IK-005.rs`, `TY-014-A.rs`,
  `FF-011.rs`, `IX-002.rs`, `RJ-002.rs`, `RJ-016.rs`,
  `RJ-026.rs`, `RJ-028-A.rs`.
- Empirical verification (Mode A run against the committed
  sidecar at `2026-05-21T15:26:55Z`): all 10 rows PASS with
  exit code 0. No deviation from spec'd `expected_rejection`
  shapes -- each rejected snippet trips exactly the
  `BridgeError` variant its sidecar entry predicts.

### Change K: `tests/smoke.rs` -- three end-to-end smoke tests

- `smoke_3_positive_rows`: 3-row mini-corpus (TY-018, IK-002,
  IK-005) -> runner exits 0 with report containing each row id
  and a `## Summary` section.
- `smoke_drift_hard_fail_on_invented_row_id`: mini-sidecar
  carries fake `ZZ-999` row -> runner exits non-zero with
  `drift-hard-fail` in stderr.
- `smoke_full_demo_set_runs_to_completion`: full 10-row demo
  set -> runner exits 0; report contains `| Total rows | 10 |`,
  `| Pass | 10 |`, and every demo row id.
- All three pass under
  `cargo test -p bridge-coverage-runner -- --test-threads=1`.

## B3 manual Mode A invocation

```
$ cargo run -p bridge-coverage-runner -- \
    --metadata crates/bridge-coverage-runner/matrix_metadata/rows.toml \
    --snippets crates/bridge-coverage-runner/snippets/ \
    --matrix-md docs/discussions/regseq-model/bridge-coverage-matrix.md \
    --out /tmp/brcr-manual-test
$ cat /tmp/brcr-manual-test/report.md
```

Excerpt of the generated report's summary:

```
| Class | Count |
| ----- | ----- |
| Total rows | 10 |
| Pass | 10 |
| Fail | 0 |
| Outcomes by class | Ok=6, Rejected=4 |

Drift report:
- Sidecar rows missing from matrix: none
- Matrix rows missing from sidecar: 349 (subset-OK in F1; see strategy section 4.1)
```

All 10 demo rows pass; 6 surface as `Ok` (positive + inspection
passed) and 4 surface as `Rejected` with the spec'd
`BridgeError` variant.

## Demo-set predictive accuracy

Per-row empirical outcomes from the Mode A run above:

| Row       | Expectation | Actual outcome             | Pass |
| --------- | ----------- | -------------------------- | ---- |
| TY-018    | positive    | Ok + TyVariant(Enum) pass  | PASS |
| IK-002    | positive    | Ok + ItemKind(Static) pass | PASS |
| IK-005    | positive    | Ok + ItemKind(Struct) pass | PASS |
| TY-014-A  | positive    | Ok + Any(...) pass         | PASS |
| FF-011    | positive    | Ok + DropGlue pass         | PASS |
| IX-002    | positive    | Ok + All(...) pass         | PASS |
| RJ-002    | rejected    | DslRejection (CVoid)       | PASS |
| RJ-016    | rejected    | Coroutine                  | PASS |
| RJ-026    | rejected    | UnsupportedAbi             | PASS |
| RJ-028-A  | rejected    | UnsupportedCAbiType(Input) | PASS |

No deviation from the spec'd `expected_rejection` shapes; no
implementer adjustment of snippets or sidecar required.

## Open-question adjudications

The three open questions the task spec raised for Codex review:

- **(a) Drift-check granularity**: AGREED_SUBSET_OK in F1. The
  sidecar-row -> matrix-row direction hard-fails (the sidecar
  MUST NOT invent IDs); the matrix-row -> sidecar-row direction
  is informational (the F1 sidecar covers 10 of ~445 rows).
  Strategy section 4.1's strict shape becomes feasible only when
  the sidecar reaches full coverage in F4.
- **(b) InspectionPredicate F1 completeness**: AGREED_DEMO_ONLY.
  The enum ships with all 23 variants so the sidecar can be
  authored against the full vocabulary; dispatch arms exist only
  for the 5 demo-set predicates. The remaining 18 surface as
  `InspectError::Unimplemented { variant, arg }` and are visible
  per-row in the report. F3 fills in dispatch.
- **(c) rustc pre-flight flags**: AGREED_EMIT_METADATA. F1 uses
  `--crate-type lib --edition 2021 --emit=metadata -o
  <tmp.rmeta>` instead of strategy section 6.3's
  `-Zno-codegen`. `--emit=metadata` is a stable rustc flag and
  produces metadata-only output without exercising
  `-Z`-unstable paths -- robust across nightly toolchain bumps.

## F1 readiness signal for F2

The runner's API + report shape is operationally sufficient for
the F2 LLM authoring loop per strategy section 11:

- The sidecar TOML schema is locked: F2 batches add rows by
  appending `[[row]]` entries with the same five-field shape.
- The inspection-predicate vocabulary covers the 5 demo
  dispatchers; F2 rows that use unimplemented predicates surface
  as `Unimplemented { variant, arg }` in the per-row report,
  giving F3 a precise scope to fill in.
- The report's per-row table + cluster shape carries every
  data point an F2 review loop needs: row ID, expectation,
  actual outcome, BridgeError variant (typed), inspection
  pass/fail, snippet path. No format string parsing required.
- The drift check fires the moment an F2 batch invents a
  sidecar row id that does NOT exist in the matrix markdown
  (the matrix authority gate).

F2 may begin authoring runs against this scaffold.

## Out of scope (per task Non-Goals)

- No `src/lib.rs` in F1 (binary-only).
- No LLM-driven snippet authoring (F2/F4).
- Dispatch arms for the 18 non-demo predicates (F3).
- JSON report output (deferred per strategy section 9).
- CI gating / Mode C nightly run (F5 / strategy section 10).
- Mode B agent integration (F5).
- F0 helper modifications: `git diff main --
  crates/test-ir-fixtures/` is empty.
- spec.md / pretty-printer.md / generic-c-backend.md /
  bridge-coverage-matrix.md / bridge-coverage-runner-strategy.md
  changes -- none.
- Beyond the spec-allowed dependencies (`toml`, `serde + derive`,
  `sha2`), no new crates: no `clap`, no `tempfile`, no
  `assert_cmd`, no `regex`.
- Expansion of the 10-row demo set into F2's 30-row batch or
  F4's 100-row batch.
