# Task: regseq-driver-19 - Raw frontend artifact emission for frontend closure

## Description

Add a reviewable, serializable raw frontend artifact emission path to
`regseq-driver` so the accepted PL011 slice can be captured as a stable
post-extraction artifact before Phase C normalized model work begins.

This task closes the biggest remaining frontend-evidence gap in Phase B:
today the compiler proves extraction mostly through stderr/debug text. B-09
needs a machine-readable artifact that preserves what Phase B actually
captures without inventing normalized declaration or binding models.

## Input Context

Read before starting:

- docs/plans/asplos-implementation-plan.md
- docs/compiler-collateral/compiler-baseline-overview.md
- docs/compiler-collateral/compiler-constraints.md
- archive/guide/open-issues.md
- archive/guide/pl011-spec-outline.md
- .harness/tasks/regseq-driver/04-driver-identity-attribute-capture.md
- .harness/tasks/regseq-driver/05-callback-context-baseline-scaffolding.md
- .harness/tasks/regseq-driver/06-register-mmio-frontend-capture-scaffolding.md
- .harness/tasks/regseq-driver/10-lifecycle-and-serial-trait-capture.md
- .harness/tasks/regseq-driver/12-clock-resource-and-wrapper-capture.md
- .harness/tasks/regseq-driver/13-resetline-resource-and-wrapper-capture.md
- .harness/tasks/regseq-driver/14-pinctrl-resource-and-wrapper-capture.md
- .harness/tasks/regseq-driver/15-timed-delay-capture-and-legality-baseline.md
- .harness/tasks/regseq-driver/16-diagnostic-logging-recognition.md
- .harness/tasks/regseq-driver/17-firmware-property-declaration-capture.md
- .harness/tasks/regseq-driver/18-pm-gate-capture-and-structural-validation.md
- tests/e2e/tests/pl011_spec_slice.rs

## Output Specification

Create or modify:

- crates/regseq-driver/src/callbacks.rs
- crates/regseq-driver/src/main.rs
- crates/regseq-driver/src/lib.rs
- crates/regseq-driver/src/raw_artifact.rs
- crates/regseq-driver/tests/raw_artifact.rs

Modify if needed:

- crates/regseq-config/src/emit.rs
- crates/regseq-config/src/lib.rs
- crates/regseq-config/tests/emit.rs
- crates/regseq-driver/tests/config.rs
- crates/cargo-regseq/tests/cli.rs
- crates/cargo-regseq/tests/invoke.rs

## Implementation Notes

This task should define a **raw frontend artifact**, not a normalized model.

The artifact should be:

- serializable
- stable enough for checked-in review snapshots
- derived directly from the extraction families that already exist
- explicit about what was captured vs what is still absent

The artifact should not:

- interpret Layer 2 values beyond the already-parsed B-07 boundary
- join Layer 1 and Layer 2 into a final semantic model
- collapse constructs into backend-oriented forms
- hide missing extraction behind invented defaults

### Required artifact content

The raw artifact should preserve, at minimum, the frontend families already
captured in the accepted PL011 slice:

- driver identity
- callback-context evidence
- register/MMIO extraction facts
- trait callback capture facts
- firmware-property declarations
- optional resource declarations present in the slice:
  - clock
  - reset line
  - pinctrl

If a family is not present for the target crate, record that cleanly as absent
or empty rather than synthesizing placeholder semantics.

### Artifact shape

Use a rustc-independent, serde-serializable struct hierarchy in
`crates/regseq-driver/src/raw_artifact.rs`.

The shape should stay deliberately close to the existing extractor outputs. A
good default is a top-level `RawFrontendArtifact` containing:

- target crate identity
- target OS from compiler options
- one optional or repeated field per extracted frontend family
- enough source-local names/ids to support later human review

The artifact should preserve:

- names
- ids
- optional-vs-required distinctions
- callback-family and callback-context classification
- register block / field / descriptor names that were actually extracted

For any extracted register-access wrapper call used by the accepted PL011 slice,
the raw artifact must preserve enough raw evidence for later review:

- containing function identity
- referenced register/member names when already present in the extractor output

This remains raw evidence, not normalized call-flow semantics.

Do not introduce:

- normalized construct ownership tables
- joined binding facts
- backend lowering hints

### Emission route

Add an explicit raw-artifact emission contract to the existing emit/output path.

The B-09 closure target is:

- running the real compiler path on a target crate
- emitting a checked-in-reviewable raw artifact file under `output_dir`

Keep the emission contract explicit and deterministic. The spec should not rely
on stderr scraping once this task lands.

Required emission contract:

- introduce a dedicated emit kind for the raw frontend artifact through the
  existing emit/options path
- emit the raw frontend artifact only when that emit kind is requested
- downstream snapshot/QC tasks should request this emit kind through the real
  compiler path rather than assuming implicit always-on emission

Use a stable output location under `output_dir` for the emitted raw artifact:

- `drivers/<driver-name>.raw.json`

If the target crate contains one accepted driver, emitting one primary raw
artifact file is sufficient for Phase B.

The emitted JSON must be deterministic enough for checked-in snapshot
comparison:

- use one canonical path per emitted driver
- preserve field names consistently
- use deterministic array/list ordering wherever the raw artifact stores
  repeated extracted facts

### Quality and caution notes

This task is where the accepted-with-caution PL011-facing frontend evidence
should stop relying on ad hoc debug text.

Be careful to preserve:

- firmware-property declaration details such as `default = ...`
- optional resource ids for clock and pinctrl
- callback-method identity, even if the real register accesses are delegated to
  helpers

The raw artifact should record both when available:

- callback declarations
- lower-level containing functions where register wrapper calls were found

That is still raw evidence and should not be forced into a normalized callback
execution model yet.

## Non-Goals

Do not implement:

- normalized declaration models
- normalized binding models
- Layer 1 + Layer 2 join logic
- backend lowering
- Phase C analysis passes

## Test Plan

Tests/commands to support:

1. Unit/integration tests prove the raw artifact struct is serializable and can
   be populated from representative extracted frontend facts
2. Existing extraction-focused tests remain green
3. The compiler path can emit the raw artifact without relying on stderr-only
   proof

Suggested commands:

- `cargo test -p regseq-driver --test raw_artifact`
- `cargo test -p regseq-config --test emit --test options`
- `cargo test -p cargo-regseq --test cli --test invoke`
- `cargo test -p regseq-driver`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

## Acceptance Criteria

- `regseq-driver` defines a serializable raw frontend artifact type
- the raw artifact preserves the accepted frontend families needed by the PL011
  slice
- the compiler can emit a raw artifact file under `output_dir`
- the design remains explicitly pre-normalized
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
