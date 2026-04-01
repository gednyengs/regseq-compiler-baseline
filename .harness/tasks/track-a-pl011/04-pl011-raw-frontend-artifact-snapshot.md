# Task: track-a-pl011-04 - PL011 raw frontend artifact snapshot and review proof

## Description

Use the real `tests/fixtures/pl011-spec-slice` fixture as the main frontend
closure evidence source by emitting, checking in, and reviewing a raw frontend
artifact snapshot for the accepted polling-first PL011 slice.

This task should turn the real PL011 fixture from “compile/extract smoke” into
“reviewable raw frontend artifact proof.”

## Input Context

Read before starting:

- docs/plans/asplos-implementation-plan.md
- docs/discussions/pl011-spec-slice/pl011-spec-slice-discussion.md
- docs/discussions/layer-2-format/layer-2-format-discussion.md
- .harness/tasks/regseq-driver/19-raw-frontend-artifact-emission.md
- .harness/tasks/track-a-pl011/01-spec-crate-scaffold-and-compiler-path.md
- .harness/tasks/track-a-pl011/02-register-schema-and-mmio-surface.md
- .harness/tasks/track-a-pl011/03-polling-driver-slice-and-layer2-fixture.md
- tests/fixtures/pl011-spec-slice/Cargo.toml
- tests/fixtures/pl011-spec-slice/platform.toml
- tests/fixtures/pl011-spec-slice/src/lib.rs
- tests/fixtures/pl011-spec-slice/src/registers.rs
- tests/fixtures/pl011-spec-slice/src/driver.rs
- tests/e2e/tests/pl011_spec_slice.rs

## Output Specification

Create or modify:

- tests/fixtures/pl011-spec-slice/expected/raw-frontend.json
- tests/e2e/tests/pl011_spec_slice.rs

Modify if needed:

- tests/fixtures/pl011-spec-slice/.gitignore

## Implementation Notes

This task should run the real PL011 fixture through the real compiler path and
check in the resulting **raw frontend artifact snapshot**.

The checked-in snapshot should be:

- generated from the real fixture
- human-reviewable
- stable enough for regression comparison

Use the emitted raw artifact from `regseq-driver-19` as the source of truth.
Do not recreate the snapshot by scraping stderr debug text.

The checked-in snapshot should come from the canonical emitted path:

- `regseq-out/drivers/pl011-spec-slice.raw.json`

The e2e path for this task should request the dedicated raw-artifact emit kind
explicitly through the real compiler options path.

### Snapshot content expectations

The checked-in raw artifact for the accepted slice should visibly include the
PL011-facing frontend facts already proven across B-08:

- driver identity for `pl011-spec-slice`
- register block / MMIO surface for `Pl011Registers`
- representative register descriptor facts for the chosen subset
- trait callback-family facts for `Lifecycle` and `Serial`
- firmware-property declarations:
  - `reg-io-width`
  - `current-speed`
- optional resource declarations when present:
  - `clock`
  - `reset`
  - `pinctrl`

The snapshot should not be hand-edited into a normalized summary. It should
still look recognizably like raw extracted compiler evidence.

### E2E review proof

Extend `tests/e2e/tests/pl011_spec_slice.rs` so it proves, through the real
compiler path, that:

- the raw artifact file is emitted
- the raw artifact matches the checked-in snapshot after any required
  deterministic normalization of volatile data
- the checked-in `platform.toml` still parses under the B-07 parser

If absolute paths or similarly unstable fields appear, normalize only those
small known-volatile parts before comparison. Do not weaken the review proof
into a vague substring check.

Do not normalize away the substantive repeated frontend facts that Phase B is
trying to review, such as:

- callback-family entries
- property declarations
- register descriptor facts
- register-access containing-function evidence

### Keep the task narrow

Do not widen this task into:

- final joined artifacts
- backend-ready summaries
- new frontend constructs
- full Phase C review tooling

This task exists to make the real PL011 slice reviewable at the raw-artifact
stage.

## Non-Goals

Do not implement:

- normalized declaration or binding models
- Layer 1 + Layer 2 join logic
- backend lowering
- new PL011 surface beyond the accepted B-08 slice

## Test Plan

Tests/commands to support:

1. The real PL011 fixture emits a raw frontend artifact
2. The emitted artifact matches the checked-in reviewed snapshot
3. Existing PL011 e2e compile/extraction assertions remain green
4. Workspace tests and clippy remain green

Suggested commands:

- `cargo test -p regseq-e2e --test pl011_spec_slice -- --ignored --nocapture`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

## Acceptance Criteria

- a checked-in raw frontend artifact snapshot exists for the real PL011 slice
- the snapshot is derived from emitted compiler output, not hand-synthesized
- the e2e test proves artifact emission plus snapshot agreement
- the task stays at raw frontend evidence level
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
