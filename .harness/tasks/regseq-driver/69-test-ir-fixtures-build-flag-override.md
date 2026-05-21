# regseq-driver-69: test-ir-fixtures per-fixture build-flag override (D1: TM-006-C + RJ-006)

## Description

This task implements proposal **D1** from the F68 post-F4 triage at
`.harness/audits/2026-05-21-post-f4-triage.md:349-415`. It extends
the `test-ir-fixtures` helper with a per-fixture build-flag
override surface, threads the override through the
bridge-coverage-runner's `rows.toml` schema, and authors two new
sidecar rows that exercise the override to bring two F4
escalations to PASS:

- **TM-006-C** (`AssertKind::OverflowNeg`): the helper at
  `crates/test-ir-fixtures/src/lib.rs:342` hard-wires
  `config.opts.cg.overflow_checks = Some(false)`, so MIR build
  does not insert the Assert::OverflowNeg runtime check on `-x`.
  Per F4 evidence at `.harness/expectations/regseq-driver-67.md:264-278`,
  re-enabling `overflow_checks` for this row is the recommended
  follow-up.
- **RJ-006** (`DslRejectionKind::Statement`): the bridge rejects
  every MIR statement variant at
  `crates/regseq-driver/src/bridge/body/statement.rs:43-82`, but
  rustc's required cleanup passes strip every rejection-eligible
  statement before `instance_mir`-time, so the bridge never sees
  one. Per F4 evidence at
  `.harness/expectations/regseq-driver-67.md:353-373`, the
  `Retag` arm at `body/statement.rs:73-77` comments "only emitted
  with `-Z mir-emit-retag`"; opting the helper into that `-Z`
  flag for this row exposes the rejection.

The deliverable's leverage comes from the reusable override
surface, not just the two-row unblock. Future bridge-coverage
rows that depend on non-default rustc flags can reuse the same
mechanism without further helper changes (subject to the
allow-list policy below).

### Why now

D1 is row 1 of the F68 closing-recommendations table at
`.harness/audits/2026-05-21-post-f4-triage.md:519-549` (D1 row at `:539`): HIGH
priority, smallest infrastructure unblock per row count, no
dependencies on other proposals. The runner's sidecar coverage
moves from 123 to 125 rows.

### Out of scope

- Bridge / model / spec / matrix / strategy / pretty-printer /
  generic-c-backend changes.
- Other F4 escalation re-attempts (A1, B1, B2, C1, C2, F1 each
  own their own task per the F68 triage document).
- New `InspectionPredicate` dispatch arms. The implementer
  verifies that an existing arm covers TM-006-C's expected
  outcome; if no arm exists at HEAD, the implementer escalates
  to the user (with the inspect.rs evidence) rather than
  silently extending the dispatch surface.
- Widening the `BuildOverrides` allow-list beyond the two
  values this task authorizes (`overflow_checks` + the single
  `-Z` flag `mir-emit-retag`). Other rustc flags belong to
  dedicated follow-up tasks with their own approval gate, per
  the No-Defensive-Fallbacks memory and the F68 Non-Goal.
- Helper API renames or restructures. `try_bridge_fixture` and
  `try_bridge_fixture_path` remain backward-compatible.
- Process-global mutex / `RUSTC_LOGGER_INIT` changes.
- New dependencies in either crate.
- Modifying `tests/fixtures/playground/` (per the standing
  playground rule in `.harness/HANDOFF.md`).

## Input Context

### Design source

- F68 audit doc at
  `.harness/audits/2026-05-21-post-f4-triage.md:349-415` --
  D1 proposal scope.
- F68 task spec at
  `.harness/tasks/regseq-driver/68-post-f4-batch-triage.md:215-244`
  -- Non-Goals enumeration this task respects.
- Strategy doc at
  `docs/discussions/regseq-model/bridge-coverage-runner-strategy.md`
  sections 4 + 4.1 + 8 -- sidecar schema rules.

### F4 expectations evidence (read-only)

- `.harness/expectations/regseq-driver-67.md:264-278` --
  TM-006-C per-candidate outcomes + recommended override.
- `.harness/expectations/regseq-driver-67.md:353-373` --
  RJ-006 per-candidate outcomes + recommended override.

### Helper code (one new struct + one new entry point; existing
entry refactored to thread overrides through)

- `crates/test-ir-fixtures/src/lib.rs:156` --
  `try_bridge_fixture(name)` (existing; stays
  backward-compatible).
- `crates/test-ir-fixtures/src/lib.rs:183` --
  `try_bridge_fixture_path(path)` (existing; stays
  backward-compatible).
- `crates/test-ir-fixtures/src/lib.rs:282-323` --
  `try_compile_and_bridge_with_crate_name` workhorse + args
  vec; refactored to accept overrides.
- `crates/test-ir-fixtures/src/lib.rs:331-345` --
  `Callbacks::config` block; the override taps the hard-wired
  flags here.

### Runner code (one struct field + one wiring path)

- `crates/bridge-coverage-runner/src/matrix.rs:34-56` -- `Row`
  struct; gains optional `build_overrides` field.
- `crates/bridge-coverage-runner/src/oracle.rs:26` --
  `try_bridge_fixture_path` import; replaced (or supplemented)
  with the override-aware variant on rows that opt in.
- `crates/bridge-coverage-runner/src/oracle.rs:67-80` --
  `invoke()` signature; gains override parameter.
- `crates/bridge-coverage-runner/src/main.rs` -- the run-loop
  call site that drives `oracle::invoke`.

### Bridge code (read-only)

- `crates/regseq-driver/src/bridge/body/statement.rs:43-82` --
  the statement variants the bridge rejects. Lines 73-77 carry
  the `Retag` arm with the comment "only emitted with
  `-Z mir-emit-retag`, outside bridge compilation contract"
  -- the RJ-006 target.
- `crates/regseq-driver/src/bridge/body/terminator.rs` -- the
  Assert handling; read-only verification that the bridge's
  Assert translation has a path that surfaces
  `AssertKind::OverflowNeg` to the inspect surface (the
  implementer verifies; the bridge has the AssertKind enum at
  some path the spec author has not pinned, which is itself a
  spec-clarity hazard the implementer should resolve in round 1).

### Matrix doc (read-only; pull canonical row metadata only)

- `docs/discussions/regseq-model/bridge-coverage-matrix.md:342`
  -- TM-006-C row (section / construct / status fields the new
  sidecar entry mirrors).
- `docs/discussions/regseq-model/bridge-coverage-matrix.md:546`
  -- RJ-006 row.

### Prior tasks

- regseq-driver-67 (`c8c16f3`): F4 corpus close; escalated
  TM-006-C + RJ-006.
- regseq-driver-68 (`c3456a5` + `4e14bb1`): F68 post-F4
  triage; proposed D1 + Codex-revised audit doc.

## Implementation Notes

### Change A: extend the test-ir-fixtures helper with a build-flag override

**File**: `crates/test-ir-fixtures/src/lib.rs`.

Add a new public struct adjacent to `BridgeOutcome` (the precise
position is the implementer's call; convention is "next to the
public types it parameterizes"):

```rust
/// Per-fixture build-flag overrides for the bridge helper.
///
/// All fields default to None / empty; the default value
/// preserves the helper's existing hard-wired behavior. The
/// override surface is intentionally narrow: only the two
/// flags this proposal authorizes (`overflow_checks` and the
/// `-Z mir-emit-retag` flag) are exposed here. Adding any
/// further flag is a separate task with its own approval gate
/// per the F68 audit document's D1 proposal.
#[derive(Debug, Clone, Default)]
pub struct BuildOverrides {
    /// Overrides `config.opts.cg.overflow_checks` in the helper's
    /// `Callbacks::config` block. None preserves the existing
    /// hard-wired `Some(false)`. `Some(true)` re-enables runtime
    /// overflow checks, which is the documented requirement for
    /// TM-006-C `AssertKind::OverflowNeg` per
    /// `.harness/expectations/regseq-driver-67.md:264-278`.
    pub overflow_checks: Option<bool>,
    /// Allow-listed `-Z` flag names to append to the rustc args
    /// vec as `-Z <flag>` after the helper's existing
    /// `-C panic=abort`. Allow-list (initial drop):
    ///   - `"mir-emit-retag"`: exposes
    ///     `mir::StatementKind::Retag` to the bridge for RJ-006.
    /// Any value outside the allow-list MUST cause the helper
    /// to panic at the workhorse entry with the diagnostic:
    ///   `"unrecognized -Z override {value:?}; allow-list = [\"mir-emit-retag\"]"`
    /// Adding values to the allow-list is a separate task per
    /// the No-Defensive-Fallbacks memory.
    pub z_flags: Vec<String>,
}
```

Add a public entry point alongside `try_bridge_fixture_path`:

```rust
pub fn try_bridge_fixture_path_with_overrides(
    path: &Path,
    overrides: &BuildOverrides,
) -> BridgeOutcome
```

This entry derives `diagnostic_name` + `crate_name` the same way
as `try_bridge_fixture_path` and calls a workhorse equivalent
that threads the overrides through.

Refactor `try_compile_and_bridge_with_crate_name` (currently at
`:282-345`) to take an extra `overrides: &BuildOverrides`
parameter. The existing `try_compile_and_bridge` (`:274-276`)
and `try_bridge_fixture_path` (`:183-190`) pass
`&BuildOverrides::default()` -- they remain behaviorally
identical at HEAD.

Within the workhorse:

1. Before constructing the args vec, validate `overrides.z_flags`
   against the allow-list. Panic with the documented diagnostic
   if any value is unrecognized. (The allow-list lives in code
   as a small `const Z_FLAG_ALLOWLIST: &[&str] = &["mir-emit-retag"];`
   slice that the panic message references explicitly.)

2. Append `"-Z".to_string(), flag.clone()` pairs for each
   `flag in overrides.z_flags` to the args vec, after the
   existing `-C panic=abort`. Ordering matters for diagnostic
   reproducibility; the implementer iterates `z_flags` in its
   stored order (`Vec` preserves insertion order; no sorting).

3. In `Callbacks::config`, replace the hard-wired
   `config.opts.cg.overflow_checks = Some(false);` with:
   ```rust
   config.opts.cg.overflow_checks = match overrides.overflow_checks {
       Some(v) => Some(v),
       None => Some(false),
   };
   ```
   All other hard-wired flags (`no_codegen`, `mir_opt_level`,
   `always_encode_mir`, `mir_enable_passes`,
   `cg.debug_assertions`, `debug_assertions`) remain
   unchanged.

The `Callbacks` struct gains an `overrides: BuildOverrides`
field so `config` can read them. The struct is local to the
workhorse and not part of the public API.

Re-export `BuildOverrides` from the crate root so the runner
can `use test_ir_fixtures::BuildOverrides;`.

### Change B: add `build_overrides` to the runner's row schema

**File**: `crates/bridge-coverage-runner/src/matrix.rs`.

Extend the `Row` struct (currently `:34-56`) with an optional
field:

```rust
/// Optional per-row build-flag overrides forwarded to the
/// helper via `try_bridge_fixture_path_with_overrides`. None
/// preserves the helper's default config.
pub build_overrides: Option<RowBuildOverrides>,
```

Add a sibling struct (next to `ExpectedRejection` at `:80-95`):

```rust
#[derive(Debug, Deserialize, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct RowBuildOverrides {
    pub overflow_checks: Option<bool>,
    #[serde(default)]
    pub z_flags: Vec<String>,
}

impl From<&RowBuildOverrides> for test_ir_fixtures::BuildOverrides {
    fn from(value: &RowBuildOverrides) -> Self {
        Self {
            overflow_checks: value.overflow_checks,
            z_flags: value.z_flags.clone(),
        }
    }
}
```

The `#[serde(deny_unknown_fields)]` attribute is load-bearing:
without it, unknown override keys in `rows.toml` would be
silently ignored, defeating the load-time enforcement story.
Test Plan B3 case (3) below covers the deny-unknown-fields
behavior explicitly.

Rationale for the mirror: the runner's TOML schema is
`Deserialize`-flavored and may evolve independently of the
helper's API. Keeping the runner-side struct lets the schema
add fields without dragging the helper along. The `From` impl
keeps the two surfaces in lockstep at field-set granularity.

Alternative the implementer may take if they prefer fewer
types: re-export the helper's `BuildOverrides` and use it
directly as the runner's row field. The criterion is "no two
parallel field sets"; either shape is acceptable provided
runner-side allow-list enforcement still happens at load time
(see B2 below).

**Allow-list enforcement at load time** (B2): when `matrix.rs`
parses a `build_overrides.z_flags` entry, it MUST reject (with
a typed error, not a panic) any value outside the allow-list.
The helper's panic at workhorse entry is a defense-in-depth
backstop; the runner's load-time validation is the first line.
This avoids a runtime panic in the middle of a long sidecar run.

### Change C: thread the override through the oracle + main loop

**File**: `crates/bridge-coverage-runner/src/oracle.rs`.

Change `invoke()` signature to:

```rust
pub fn invoke(
    snippet_path: &Path,
    overrides: Option<&test_ir_fixtures::BuildOverrides>,
) -> OracleOutcome
```

The `catch_unwind` wrapper at `:79-80` calls
`try_bridge_fixture_path_with_overrides(path, overrides)` when
`overrides.is_some()`, otherwise calls the existing
`try_bridge_fixture_path(path)`. (The two branches collapse if
the implementer chooses to always call the override-aware
variant with `BuildOverrides::default()`; both shapes pass.)

The pre-flight `rustc_preflight` step (`:69-77`) is NOT
override-aware in this drop: the pre-flight runs plain rustc
metadata emission and does not exercise any of the override
flags. If TM-006-C or RJ-006 requires the pre-flight to also
receive `-Z mir-emit-retag` (it does not, empirically -- the
flag affects MIR-build downstream of metadata), no pre-flight
change is needed. The implementer verifies this assumption
empirically by running TM-006-C / RJ-006 through both
pre-flight + in-process bridge; if pre-flight rejects the
snippet, the implementer escalates to the user (with the
pre-flight stderr capture) rather than widening pre-flight
to override-aware.

**File**: `crates/bridge-coverage-runner/src/main.rs`.

The run-loop call site that invokes `oracle::invoke` passes
`row.build_overrides.as_ref().map(Into::into).as_ref()` (or
equivalent) -- the implementer picks the precise lifetime
shape. The conversion uses the `From<&RowBuildOverrides> for
BuildOverrides` impl from Change B.

### Change D: author two new sidecar rows

**File**: `crates/bridge-coverage-runner/snippets/TM-006-C.rs`.

```rust
pub fn f(x: i32) -> i32 {
    -x
}
```

**File**: `crates/bridge-coverage-runner/snippets/RJ-006.rs`.

The implementer chooses the simplest source shape that, under
`-Z mir-emit-retag`, surfaces `mir::StatementKind::Retag` to
the bridge's body translator. The canonical trigger is a
function with a reference-typed parameter -- pinned rustc's
`AddRetag` pass emits a `StatementKind::Retag(RetagKind::FnEntry, ...)`
for every reference-typed function argument on entry to the
body, per `compiler/rustc_mir_transform/src/add_retag.rs:80-86`
(under the active sysroot). Plain Rvalue::Ref assignments
(e.g., `let _r = &mut x;`) do NOT emit Retag in current rustc:
per the same pass at `add_retag.rs:130-156`, the assignment
arm returns `None` for `Rvalue::Ref(..)`, so those statements
are deliberately skipped.

Recommended candidate:

```rust
pub fn f(_p: &mut u32) {}
```

The function signature carries the reference type; the
`FnEntry` retag fires on entry. The empty body keeps the rest
of the MIR minimal so the bridge sees the `Retag` statement
without other rejection-eligible noise. The `&mut` flavor is
preferred over `&` only because `RetagKind::FnEntry` over a
mutable reference is the path AddRetag covers most explicitly;
either reference flavor should work and the implementer may
swap if `&u32` empirically produces a Retag and yields a
simpler MIR.

If neither flavor produces Retag under the pinned nightly's
`AddRetag` pass (e.g., the pass has been re-gated since the
2026-02-01 toolchain was pinned), the implementer escalates
to the user with the empirical evidence (rustc-emitted MIR
dump showing the absence of Retag) rather than guessing
further shapes.

**File**: `crates/bridge-coverage-runner/matrix_metadata/rows.toml`.

Append two `[[row]]` entries (the implementer reads the matrix
doc at `bridge-coverage-matrix.md:342` and `:546` to fill the
canonical `section` and `construct` values; the placeholders
below are illustrative):

```toml
[[row]]
id = "TM-006-C"
section = "<from matrix doc>"
construct = "<canonical TM-006-C construct name>"
expectation = "positive"
status = "covered"
cluster_key = "TM-006"
snippet_path = "TM-006-C.rs"
inspect = { kind = "<AssertKind predicate>", arg = "OverflowNeg" }
build_overrides = { overflow_checks = true }

[[row]]
id = "RJ-006"
section = "<from matrix doc>"
construct = "DslRejectionKind::Statement"
expectation = "rejected"
status = "covered"
cluster_key = "RJ-006"
snippet_path = "RJ-006.rs"
expected_rejection = { variant = "DslRejection", kind = "Statement" }
build_overrides = { z_flags = ["mir-emit-retag"] }
```

The implementer verifies (i) the `inspect` predicate kind for
TM-006-C against the InspectionPredicate enum at
`crates/bridge-coverage-runner/src/inspect.rs` (the F3 dispatch
surface is closed at 22/22, but the spec author has not pinned
which arm covers Assert::OverflowNeg -- the implementer reads
inspect.rs to find it; if no arm exists at HEAD, the
implementer escalates to the user with the inspect.rs
evidence), and (ii) the `expected_rejection.kind` value for
RJ-006 against the `DslRejectionKind` enum at
`crates/regseq-driver/src/bridge/mod.rs:580-597`.

### Change E: NO bridge or model changes

The bridge's `Retag` rejection at
`crates/regseq-driver/src/bridge/body/statement.rs:73-77`
already produces `DslRejectionKind::Statement` with the
documented message; this proposal does NOT modify it. The
Assert::OverflowNeg surfacing is similarly handled by the
existing bridge terminator + assert paths; this proposal does
NOT add or modify any bridge arm.

## Non-Goals

- Do NOT change the bridge, the regseq pass whitelist, or the
  model.
- Do NOT widen the `BuildOverrides` allow-list beyond
  `overflow_checks` + `z_flags = ["mir-emit-retag"]`. Other
  flags belong to separate tasks.
- Do NOT change the helper's process-global mutex or
  `RUSTC_LOGGER_INIT` serialization.
- Do NOT rename or restructure existing helper API.
  `try_bridge_fixture` and `try_bridge_fixture_path` remain
  backward-compatible.
- Do NOT re-attempt other F4 escalations.
- Do NOT modify `tests/fixtures/playground/`.
- Do NOT add new dependencies to either crate.
- Do NOT modify the matrix doc, the spec doc, the strategy
  doc, the pretty-printer doc, or the generic-c-backend doc.
- Do NOT add new `InspectionPredicate` dispatch arms.
- Do NOT change the runner's pre-flight `rustc_preflight` path
  unless empirically forced by the override (and even then,
  escalate to the user rather than expanding scope unilaterally).

## Test Plan

### B1: workspace stays green

`RUST_TEST_THREADS=1 cargo test --workspace -- --test-threads=1`
green. Existing tests pass without modification.

### B2: helper override surface unit tests

Add unit tests under either
`crates/test-ir-fixtures/src/lib.rs` (inline `#[cfg(test)]`
module) or `crates/test-ir-fixtures/tests/overrides.rs` -- the
implementer picks the conventional layout. Tests cover:

1. `BuildOverrides::default()` produces a struct where every
   field is `None` / empty. Sanity check.
2. Calling
   `try_bridge_fixture_path_with_overrides(p, &BuildOverrides::default())`
   on an existing fixture produces the same `BridgeOutcome`
   discriminant as `try_bridge_fixture_path(p)`. No behavioral
   drift on the default path.
3. Calling
   `try_bridge_fixture_path_with_overrides(p, &BuildOverrides { z_flags: vec!["unrecognized-flag".to_string()], ..Default::default() })`
   panics. The panic message contains the allow-list (verified
   with `std::panic::catch_unwind` + payload string match).

### B3: runner matrix-load allow-list validation

Add a unit test in `crates/bridge-coverage-runner/src/matrix.rs`
(or a sibling test file). Test cases:

1. A `rows.toml` snippet with `build_overrides = { z_flags = ["mir-emit-retag"] }` loads cleanly.
2. A `rows.toml` snippet with `build_overrides = { z_flags = ["bogus-flag"] }` produces a typed `MatrixError` (the implementer chooses the error-variant name) with a message that names the bogus value and the allow-list.
3. A `rows.toml` snippet with an unknown override key, e.g.
   `build_overrides = { unknown_flag = true }`, fails to
   deserialize because of `#[serde(deny_unknown_fields)]`. The
   resulting error message names the unknown field. This case
   guards against future override surfaces being silently
   accepted by the loader.

### B4: runner end-to-end Mode A with overrides

Manual Mode A invocation:

```
target/debug/bridge-coverage-runner \
  --metadata crates/bridge-coverage-runner/matrix_metadata/rows.toml \
  --snippets crates/bridge-coverage-runner/snippets/ \
  --matrix-md docs/discussions/regseq-model/bridge-coverage-matrix.md \
  --out /tmp/brcr-d1/
```

Expects: exit 0, `Total = 125`, `Pass = 125`, `Fail = 0`.

### B5: TM-006-C surfaces Assert::OverflowNeg

The runner's per-row report for TM-006-C MUST show `PASS`
with the inspection predicate matching against an
`AssertKind::OverflowNeg`-bearing body. The implementer
verifies the predicate match is positive (not a false-positive
via a different AssertKind) by reading the report's per-row
diagnostic OR by adding a debug print in inspect.rs during
development (the print must NOT be committed).

### B6: RJ-006 surfaces a DslRejectionKind::Statement

The runner's per-row report for RJ-006 MUST show `PASS` --
i.e., the bridge rejected with `DslRejection { kind: Statement, .. }`
matching the sidecar's `expected_rejection`. The implementer
verifies via the report.

### B7: no regression on the existing 123 rows

`target/debug/bridge-coverage-runner` Mode A output's per-row
PASS / FAIL classification for the 123 pre-existing rows is
identical to the F4 close state. The implementer compares
against the F4 report at the F4 closing commit (`c8c16f3`)
OR against a fresh pre-Change-D run.

### B8: drift check still passes

The matrix sidecar drift check (subset-OK) reports clean: both
new rows are documented in the matrix doc (`:342` and `:546`).
No new orphan rows; no missing matrix entries.

## Review

### Files to send to Codex (`scripts/python/codex_runner.py`)

- This file
  (`.harness/tasks/regseq-driver/69-test-ir-fixtures-build-flag-override.md`).
- The F68 audit doc
  (`.harness/audits/2026-05-21-post-f4-triage.md`) -- D1
  proposal source.
- The F4 expectations file
  (`.harness/expectations/regseq-driver-67.md`) -- TM-006-C
  + RJ-006 empirical evidence.
- The helper source (`crates/test-ir-fixtures/src/lib.rs`).
- The runner matrix source
  (`crates/bridge-coverage-runner/src/matrix.rs`).
- The runner oracle source
  (`crates/bridge-coverage-runner/src/oracle.rs`).
- The bridge statement source
  (`crates/regseq-driver/src/bridge/body/statement.rs`).
- The strategy doc
  (`docs/discussions/regseq-model/bridge-coverage-runner-strategy.md`)
  for sidecar schema rules in sections 4 + 4.1 + 8.

### Reviewer focus

- **Override-surface narrowness**: the allow-list is restricted
  to `overflow_checks` + `mir-emit-retag`. Is the spec's
  enforcement story (panic in helper + typed error in matrix
  loader) coherent and sufficiently defended against drift?
- **Empirical evidence chain**: is the spec's claim that
  `overflow_checks=true` for `fn f(x:i32){-x}` actually
  produces `AssertKind::OverflowNeg` supported by F4 evidence,
  or is it a hypothesis that should be marked as such for the
  implementer?
- **`-Z mir-emit-retag` empirical reliability**: does the
  candidate snippet shape `pub fn f(_p: &mut u32) {}` actually
  emit a `StatementKind::Retag(RetagKind::FnEntry, ...)` under
  the pinned nightly when the flag is set? The spec cites
  AddRetag's `:80-86` FnEntry path and `:130-156` Rvalue::Ref
  skip; verify against the rustc source under the active
  sysroot.
- **InspectionPredicate coverage for TM-006-C**: does the F3
  dispatch surface (22/22 arms per HANDOFF line 30) actually
  include an arm that matches Assert::OverflowNeg, or is the
  spec implicitly demanding a new dispatch arm in violation of
  Non-Goal "Do NOT add new InspectionPredicate dispatch arms"?
  The spec says the implementer escalates to the user if no
  arm exists, but Codex should verify the arm exists at HEAD.
- **Backward compatibility**: existing callers of
  `try_bridge_fixture`, `try_bridge_fixture_path`,
  `try_compile_and_bridge_with_crate_name` continue to work
  without modification. The workhorse signature change is
  internal; the external API is additive only.
- **Pre-flight override decision**: the spec asserts pre-flight
  does NOT need overrides because `-Z mir-emit-retag` affects
  only MIR-build downstream of metadata emission. Verify this
  assumption empirically against the rustc source or the
  pinned nightly's documented `-Z` semantics.
- **Citation drift at HEAD**: every `file:line` against HEAD
  (`4e14bb1`).
- **Off-by-N row count math**: 123 (F4 close at `c8c16f3`) + 2
  (D1 sidecar rows) = 125 total. Verify the math.
- **No new dependencies**: neither
  `crates/test-ir-fixtures/Cargo.toml` nor
  `crates/bridge-coverage-runner/Cargo.toml` gains a new
  dependency.
- **Allow-list enforcement layering**: matrix loader fails fast
  at load time (typed error); helper panic is defense-in-depth.
  Is this layering called out clearly enough?

### Adversarial prompt template

```
<task>
Review .harness/tasks/regseq-driver/69-test-ir-fixtures-build-flag-override.md.

Identify defects under the following adversarial checks:

1. OVERRIDE-SURFACE NARROWNESS: Is the allow-list (overflow_checks + mir-emit-retag) enforced at both layers (matrix loader load-time + helper workhorse defense-in-depth)?

2. EMPIRICAL EVIDENCE CHAIN: For each unblock claim (overflow_checks=true produces Assert::OverflowNeg for fn f(x:i32){-x}; -Z mir-emit-retag produces Retag for the cited snippet shape), is the claim supported by the F4 expectations file or by a citation to rustc source / documented -Z semantics? Flag claims that are hypothetical or unsupported.

3. INSPECTIONPREDICATE COVERAGE: Does the F3 dispatch surface (`crates/bridge-coverage-runner/src/inspect.rs`) already contain an arm that matches Assert::OverflowNeg? If not, the spec's Non-Goal "Do NOT add new InspectionPredicate dispatch arms" contradicts Change D's TM-006-C row. Verify by reading inspect.rs.

4. BACKWARD COMPATIBILITY: Do the existing helper entries (try_bridge_fixture, try_bridge_fixture_path) continue to work with the refactored try_compile_and_bridge_with_crate_name? Flag any signature change that breaks them.

5. PRE-FLIGHT OVERRIDE: Does -Z mir-emit-retag affect the pre-flight metadata-only rustc invocation, or only the in-process MIR-build downstream? If pre-flight is affected, Change C's "no pre-flight changes" assumption breaks. Verify against the pinned nightly's documented -Z semantics if you can find them.

6. CITATION DRIFT AT HEAD: Every file:line anchor against HEAD (4e14bb1).

7. OFF-BY-N ROW COUNT: 123 + 2 = 125. Verify the F4 close at c8c16f3 actually had 123 rows in rows.toml.

8. SCOPE CREEP: Does the spec implicitly invite scope creep (e.g., "implementer may extend the allow-list", "implementer may modify pre-flight") in any phrasing that should be tightened?

9. ALLOW-LIST LAYERING CLARITY: Is the two-layer enforcement (matrix loader = first line; helper panic = backstop) explicit enough that a Codex reader can verify both are wired?

10. ESCALATION PATHS: Does every "implementer escalates" clause have a clear escalation target (the user) and a clear trigger condition? Flag any vague escalation language.
</task>

<grounding_rules>
- Cite file:line for every claim. The format `path/to/file.md:NNN-MMM` is the expected style.
- For each cited file:line in the task spec, open the file and verify the citation. Quote the source, do not paraphrase the spec's claim.
- Mark hypotheses as hypotheses. If you suspect a defect but cannot prove it without empirical runner execution, say so explicitly.
- Pinned toolchain is `nightly-2026-02-01`; HEAD is `4e14bb1`.
- For -Z flag semantics, refer to rustc source under the active sysroot if accessible; otherwise mark as a runtime-verifiable assumption the implementer must check.
</grounding_rules>

<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
OVERRIDE-SURFACE NARROWNESS: PASS | FAIL with cite
EMPIRICAL EVIDENCE CHAIN: PASS | FAIL with per-defect cite
INSPECTIONPREDICATE COVERAGE: PASS | FAIL with cite
BACKWARD COMPATIBILITY: PASS | FAIL with cite
PRE-FLIGHT OVERRIDE: PASS | FAIL with cite
CITATION DRIFT AT HEAD: list each drifted anchor or "none"
OFF-BY-N ROW COUNT: PASS | FAIL with cite
SCOPE CREEP: PASS | FAIL with cite
ALLOW-LIST LAYERING CLARITY: PASS | FAIL with cite
ESCALATION PATHS: PASS | FAIL with cite
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
  `Total = 125`, `Pass = 125`, `Fail = 0`, exit 0.
- `git diff --stat main..` shows changes confined to:
  - `crates/test-ir-fixtures/src/lib.rs` (Change A).
  - Helper tests file (Test Plan B2).
  - `crates/bridge-coverage-runner/src/matrix.rs` (Change B).
  - `crates/bridge-coverage-runner/src/oracle.rs` (Change C).
  - `crates/bridge-coverage-runner/src/main.rs` (Change C).
  - `crates/bridge-coverage-runner/matrix_metadata/rows.toml`
    (Change D; +2 rows).
  - `crates/bridge-coverage-runner/snippets/TM-006-C.rs`
    (Change D).
  - `crates/bridge-coverage-runner/snippets/RJ-006.rs`
    (Change D).
- No bridge / model / spec / matrix / strategy /
  pretty-printer / generic-c-backend changes.
- No new dependencies in either crate's Cargo.toml.
- Codex Task Review verdict: APPROVED or
  APPROVED_WITH_MINOR_FIXES.

## Acceptance Criteria

1. `BuildOverrides` public struct landed in
   `crates/test-ir-fixtures/src/lib.rs` with `overflow_checks`
   + `z_flags` fields and the documented default.
2. `try_bridge_fixture_path_with_overrides` public entry
   landed.
3. `z_flags` allow-list enforcement panics in the helper
   workhorse with the documented diagnostic when an
   unrecognized value is passed.
4. `Row.build_overrides` optional field landed in
   `crates/bridge-coverage-runner/src/matrix.rs` with the
   sibling `RowBuildOverrides` struct and `From` impl for
   the helper's `BuildOverrides` (OR direct re-export per the
   implementer's call; see Change B).
5. Matrix loader validates `z_flags` values against the
   allow-list at load time and emits a typed error for
   unrecognized values; verified by B3.
6. Oracle's `invoke()` threads overrides through to the helper.
7. Main loop's run loop passes the row's overrides into
   `oracle::invoke`.
8. TM-006-C snippet + sidecar row land; the runner reports
   PASS for the row against an Assert::OverflowNeg-bearing
   inspection.
9. RJ-006 snippet + sidecar row land; the runner reports PASS
   for the row against a `DslRejection { kind: Statement, .. }`
   rejection.
10. `Total = 125`, `Pass = 125`, `Fail = 0` after the runner's
    Mode A invocation.
11. Existing 123 rows continue to PASS (no regression vs F4
    close).
12. `cargo fmt --all -- --check`,
    `cargo clippy ... -D warnings`,
    `RUST_TEST_THREADS=1 cargo test --workspace -- --test-threads=1`
    all green.
13. Codex Task Review verdict: APPROVED or
    APPROVED_WITH_MINOR_FIXES.
14. No bridge / model / spec / matrix / strategy /
    pretty-printer / generic-c-backend file modifications.
15. No new dependencies in either crate.
16. No new `InspectionPredicate` dispatch arms; if TM-006-C's
    expected inspection requires one, the implementer
    escalates to the user (with the inspect.rs evidence)
    rather than silently extending dispatch.
