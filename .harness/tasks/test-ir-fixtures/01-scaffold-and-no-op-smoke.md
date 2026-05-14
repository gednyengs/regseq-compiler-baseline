# test-ir-fixtures-01: `test-ir-fixtures` scaffold + no-op smoke fixture (P1a-TS-01-E2E-01)

## Description

Bootstrap the `test-ir-fixtures` workspace member crate at
`crates/test-ir-fixtures/` and ship the first end-to-end smoke
fixture: bridge the canonical `fn no_op() {}` Rust program through
the full phase-1a `regseq-driver` pipeline, validate the resulting
`RegseqModel`, pretty-print it, and lock the pretty-printed output
against an `insta` snapshot. The crate also ships the canonical
shared Rust fixture programs (no-op, the four diagnostic-logging
severity calls, a `dyn Reader` trait-object call site, a
`Fn(u32) -> u32` closure expression) plus the helper API
(`bridge_fixture(name) -> RegseqModel`) that downstream
`P1a-E2E-02` and `P1a-E2E-03` integration tests consume.

This task merges what would otherwise be two tasks (TS-01 scaffold
+ E2E-01 no-op smoke) per the post-draft Codex right-sizing
review: the no-op fixture is the natural smoke proving the
scaffold works end-to-end. The scaffold is sized to host all
phase-1a E2E fixtures, not just the no-op; the diagnostic-logging
and synthesis fixtures land their own `tests/e2e/*.rs` integration
files in `P1a-E2E-02` and `P1a-E2E-03` and reuse the helper API
defined here.

## Input Context

- `.harness/spec-batches/regseqmodel-phase-1a.md` (parent brief;
  P1a-TS-01-E2E-01 row at axis 5)
- `.harness/SESSION.md` (project overview, coding conventions,
  testing requirements, Review section four-block template)
- `.harness/README.md` (`test-support-crate` test-organization
  rule)
- `docs/discussions/regseq-model/spec.md` §3 (top-level
  `RegseqModel` root struct; `spec.md:204+`)
- `docs/discussions/regseq-model/spec.md` §3.3 (model validation
  order; the validate entry point the smoke calls; `spec.md:621+`)
- `docs/discussions/regseq-model/spec.md` §3.4 (bridge
  construction-pipeline order; `spec.md:1121+`)
- `docs/discussions/regseq-model/spec.md` §25 (Body; the no-op
  function lifts to a single-block Body with one `Return`
  Terminator and one `_0` LocalDecl; `spec.md:11076+`)
- `docs/discussions/regseq-model/pretty-printer.md` §10.3
  (top-level `render(&self) -> String`; the smoke asserts the
  rendered string is non-empty and snapshot-stable;
  `pretty-printer.md:4201+`)
- `Cargo.toml` workspace root (the `test-ir-fixtures` member is
  registered here; the new entry sits alongside the existing
  `crates/regseq-*` members)
- `crates/regseq-model/src/lib.rs` (the `RegseqModel` root + the
  pretty-printer the smoke fixture consumes via the bridge's
  output)
- `crates/regseq-driver/src/lib.rs` and
  `crates/regseq-driver/src/callbacks.rs` (the bridge the helper
  drives)
- `archive/.harness/tasks/regseq-model/24-place-and-projection.md`
  (the SHAPE / SIZE reference the brief points the worker at)

## Implementation Notes

This task lands the new workspace crate `test-ir-fixtures`, the
canonical fixture programs it ships, the bridge-driver helper
API, and a single integration test exercising the no-op fixture
end-to-end.

### Pre-task harness step (one-off direct edit)

Before invoking `task.py add-task` for any test-ir-fixtures task,
the harness's `.harness/tasks.json` must include a
`test-ir-fixtures` group entry (the CLI requires the group to
pre-exist; `task.py` does not currently expose an `add-group`
subcommand). Add the group by hand to `.harness/tasks.json` -- a
one-off direct edit justified by the absence of the
`add-group` CLI subcommand. This is the only direct-edit
exception in phase-1a; subsequent task creations under the group
must route through `task.py add-task --group test-ir-fixtures
...` per the harness rule.

### Workspace-root `Cargo.toml` registration

Append `crates/test-ir-fixtures` to the `[workspace] members`
list in the workspace-root `Cargo.toml`. Order: insert
alphabetically among `crates/...` entries (between
`crates/regseq-model-old` and `tests/e2e-old` per the existing
ordering pattern).

### `crates/test-ir-fixtures/Cargo.toml`

```toml
[package]
name = "test-ir-fixtures"
version = "0.1.0"
edition = "2021"
publish = false

[dependencies]
regseq-model  = { path = "../regseq-model" }
regseq-driver = { path = "../regseq-driver" }

[dev-dependencies]
insta = { version = "1", features = ["yaml"] }
```

The crate is `publish = false` since it is a test-support crate
internal to the workspace. The `dev-dependencies` block carries
`insta` for snapshot testing in the crate's own integration
tests; downstream consumer crates (E2E test crates depending on
`test-ir-fixtures`) re-declare `insta` in their own
`dev-dependencies`.

### `crates/test-ir-fixtures/src/lib.rs`

Public API surface (define signatures with `todo!()` first per
`SESSION.md` "Doing the Work" §6-§10, write tests against the
API, then implement):

```rust
//! Shared Rust IR fixtures + bridge helpers for phase-1a end-to-end
//! integration tests.

use regseq_model::RegseqModel;

/// Names of the canonical phase-1a fixtures shipped by this crate.
/// Each name maps to a `.rs` source file under `fixtures/`.
pub mod fixture_names {
    /// `fn no_op() {}` -- the minimal smoke fixture.
    pub const NO_OP: &str = "no_op";
    /// Calls `regseq::log::{debug,info,warn,error}!(...)` once each
    /// with one `u32` argument. Used by P1a-E2E-02.
    pub const DIAG_LOGGING: &str = "diag_logging";
    /// `dyn Reader` trait-object call site. Used by P1a-E2E-03.
    pub const TRAIT_OBJECT: &str = "trait_object";
    /// `|x| x + 1` closure expression. Used by P1a-E2E-03.
    pub const CLOSURE: &str = "closure";
}

/// Drive the phase-1a `regseq-driver` bridge against the named
/// fixture under `crates/test-ir-fixtures/fixtures/<name>.rs` and
/// return the constructed `RegseqModel`. Panics with a descriptive
/// message if the fixture name is unknown, the fixture file is
/// missing, the bridge errors, or the bridge produces an
/// `InvariantViolation` from `RegseqModel::validate`.
///
/// The helper compiles the fixture under the workspace's pinned
/// rustc toolchain (per `rust-toolchain.toml`) using the `regseq-driver`
/// binary entry point and parses the bridge's serialised output back
/// into an in-memory `RegseqModel`. See `bridge_fixture_with_options`
/// for the full configuration surface.
pub fn bridge_fixture(name: &str) -> RegseqModel;

/// Pretty-print the model with default `PrettyPrintOptions`
/// (`include_spans: false`, `include_debug_names: false`) per
/// `pretty-printer.md` §4.1 default mode. Convenience wrapper for
/// integration-test snapshot assertions.
pub fn render_default(model: &RegseqModel) -> String;
```

### `crates/test-ir-fixtures/fixtures/` directory

Ships **four** canonical Rust source files, one per fixture name
in the `fixture_names` module above. Each file is a standalone
`#![no_std]` Rust crate root authored to compile under the
workspace's pinned rustc toolchain when driven by `regseq-driver`.

1. **`fixtures/no_op.rs`** -- the minimal smoke:
   ```rust
   #![no_std]
   pub fn no_op() {}
   ```

2. **`fixtures/diag_logging.rs`** -- one call to each of the four
   severity macros with a single `u32` argument:
   ```rust
   #![no_std]
   use regseq::log;
   pub fn emit_logs(retries: u32) {
       log::debug!("debug event x={}", retries);
       log::info!("info event x={}", retries);
       log::warn!("warn event x={}", retries);
       log::error!("error event x={}", retries);
   }
   ```
   This fixture is used by P1a-E2E-02; it ships here so all
   phase-1a fixtures live in one place.

3. **`fixtures/trait_object.rs`** -- a `dyn Reader` call site:
   ```rust
   #![no_std]
   pub trait Reader { fn read_byte(&self) -> u8; }
   pub fn drive(r: &dyn Reader) -> u8 { r.read_byte() }
   ```
   Used by P1a-E2E-03.

4. **`fixtures/closure.rs`** -- a `Fn(u32) -> u32` closure:
   ```rust
   #![no_std]
   pub fn make_inc() -> impl Fn(u32) -> u32 { |x| x + 1 }
   ```
   Used by P1a-E2E-03.

### Smoke integration test

Lives at `crates/test-ir-fixtures/tests/no_op_smoke.rs` and
exercises the no-op fixture end-to-end. Test body:

```rust
use test_ir_fixtures::{bridge_fixture, fixture_names, render_default};

#[test]
fn no_op_bridge_validates_and_pretty_prints() {
    let model = bridge_fixture(fixture_names::NO_OP);

    // 1. Validation passes (helper would have panicked otherwise,
    //    but assert structurally for clarity).
    model.validate().expect("RegseqModel::validate must pass on no_op");

    // 2. Exactly one Function Instance is present (the `no_op` fn).
    //    No synthesised Items expected (no closures, no vtables, no
    //    drop-glue, no const-memory statics).
    assert_eq!(model.instances.len(), 1, "no_op should produce one Instance");

    // 3. Pretty-printed output is non-empty and snapshot-stable.
    let rendered = render_default(&model);
    assert!(!rendered.is_empty(), "render must be non-empty");
    insta::assert_snapshot!("no_op_render", rendered);
}
```

The `insta` snapshot file lands at
`crates/test-ir-fixtures/tests/snapshots/no_op_smoke__no_op_render.snap`.
The first `cargo test` run after implementation creates the
snapshot; subsequent runs compare against it. Snapshot review and
acceptance flows through `cargo insta review` (developer-side; not
automated in CI).

### Conventions

- Doc comments on every public type / function (per `SESSION.md`
  "Coding Conventions").
- No unicode symbols in source / docs (per global rules + project
  `SESSION.md`).
- The helper API uses panics for failure paths because it is a
  test-support crate; downstream tests want the Cargo test
  harness to surface the panic with a clear message rather than
  a `Result` the test must `.unwrap()`.
- `#![no_std]` on every fixture file: phase-1a's bridge does not
  pull in `std`, and the fixtures must compile under the same
  constraints production drivers do.
- The `regseq::log::*` macros referenced by `diag_logging.rs` are
  the author surface defined in `spec.md` §26.5; their bridge
  recognition lands in `regseq-driver-44` (P1a-BR-08a).
- Snapshot file naming follows `insta`'s default
  `<test-file-stem>__<snapshot-name>.snap` convention; do not
  override.

## Non-Goals

- The diagnostic-logging fixture's *assertions* (post-DiagLogPeel
  flat-arg Call shapes, `IntrinsicKind::Log(_)` attribute
  population, snapshot of the rendered model) are
  `regseq-model-94` (P1a-E2E-02) territory; this task only ships
  the fixture file and helper.
- The trait-object + closure synthesis assertions (synthesised
  VTable/ClosureCaptures Item presence, post-vtable-indirect
  Call shape, snapshot) are `regseq-model-95` (P1a-E2E-03)
  territory; this task only ships the fixture files and helper.
- The `regseq-driver` bridge itself (Phases 1-5) is implemented
  by `regseq-driver-36` through `regseq-driver-45` (P1a-BR-01
  through P1a-BR-08b); this task only orchestrates the existing
  driver via a helper.
- The pretty-printer composers themselves (top-level `render`,
  per-Item / per-Instance / per-Body renderers) are
  `regseq-model-86` through `regseq-model-93` (P1a-PP-01 through
  P1a-PP-06); this task only invokes the existing
  `RegseqModel::render` (or equivalent stage-14 composer entry
  point) via the `render_default` helper.
- No additional fixtures beyond the four canonical ones in this
  task. New fixtures land in their own E2E task or in a phase-1b
  task.
- No CI integration of `cargo insta review` -- snapshot
  acceptance remains a developer-side step.

## Test Plan

**Test organization**: this task uses two of the three test-org
kinds simultaneously per the parent brief's per-task naming:

1. `test-support-crate` -- the crate's own purpose (shipping
   shared fixtures + helpers to other crates).
2. `integration-in-tests` -- the no-op smoke at
   `crates/test-ir-fixtures/tests/no_op_smoke.rs`.

Coverage requirements (every public item exercised; every
fixture file present and parseable; the smoke test asserts the
end-to-end pipeline):

### Crate scaffold assertions (test-support-crate kind)

1. **Workspace registration.** After `Cargo.toml` edit, `cargo
   metadata --format-version 1 --no-deps` lists `test-ir-fixtures`
   in the `packages` array.
2. **Crate compiles.** `cargo build -p test-ir-fixtures` succeeds
   from a clean target.
3. **Fixture files present and well-formed.** Add a
   `#[cfg(test)] mod tests` in `lib.rs` that asserts each of the
   four `fixture_names::*` constants resolves to a readable file
   under `crates/test-ir-fixtures/fixtures/<name>.rs`. Use
   `std::fs::metadata` + `is_file()` and assert non-zero file
   size for each.
4. **Helper API contract -- unknown fixture panics.** A
   `#[cfg(test)] mod tests` test invokes
   `bridge_fixture("does_not_exist")` inside
   `std::panic::catch_unwind` and asserts the panic message
   names the missing fixture. Verifies the helper's failure
   path is descriptive.
5. **Helper API contract -- doc-comment presence.** A
   `cargo doc -p test-ir-fixtures --no-deps` invocation must
   succeed without `missing_docs` warnings (enforced via
   `#![warn(missing_docs)]` in `lib.rs`).

### Smoke integration test (integration-in-tests kind)

6. **`no_op_bridge_validates_and_pretty_prints`** at
   `crates/test-ir-fixtures/tests/no_op_smoke.rs` exercises the
   end-to-end pipeline:
   a. `bridge_fixture(fixture_names::NO_OP)` returns without
      panic.
   b. `model.validate()` returns `Ok(())` (per `spec.md` §3.3
      validation order).
   c. `model.instances.len() == 1`.
   d. `render_default(&model)` returns a non-empty `String`.
   e. The rendered output matches the
      `insta::assert_snapshot!("no_op_render", rendered)`
      snapshot at
      `crates/test-ir-fixtures/tests/snapshots/no_op_smoke__no_op_render.snap`.
7. **Snapshot baseline shape.** The committed snapshot must
   contain (verifiable by inspection):
   - The `no_op` Function Instance's rendered DefPath segment.
   - An empty function body envelope per `pretty-printer.md`
     §3.1 (single block with one `Return` terminator).
   - No synthesised-Item attribute decoration in the Item
     header (no closures, no vtables -- so no `#[regseq::synthetic
     = ...]` decorator).

`cargo test -p test-ir-fixtures` passes (covers tests 3-6);
workspace-wide gate covers test 2.

## Review

Once tests pass, request a Codex review of the implementation by
invoking `scripts/python/codex_runner.py` via the Bash tool (per
`SESSION.md` Review section). Use the four-block template
verbatim:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of test-ir-fixtures-01 against its task
spec at .harness/tasks/test-ir-fixtures/01-scaffold-and-no-op-smoke.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- Cargo.toml (workspace-root [workspace] members entry)
- crates/test-ir-fixtures/Cargo.toml
- crates/test-ir-fixtures/src/lib.rs
- crates/test-ir-fixtures/fixtures/no_op.rs
- crates/test-ir-fixtures/fixtures/diag_logging.rs
- crates/test-ir-fixtures/fixtures/trait_object.rs
- crates/test-ir-fixtures/fixtures/closure.rs
- crates/test-ir-fixtures/tests/no_op_smoke.rs
- crates/test-ir-fixtures/tests/snapshots/no_op_smoke__no_op_render.snap

Focus on:
- Spec deviations (every rule in the Implementation Notes section).
- Correctness issues and generality loss.
- Design misalignment with the spec's public API / architecture.
- Test coverage vs. the spec's Test Plan.
Do NOT flag anything explicitly marked out of scope (Non-Goals,
later task IDs).
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

Iterate (fix -> re-invoke `codex_runner.py`) until VERDICT: APPROVED.

## Quality Gate

Workspace-wide because this task touches the workspace `Cargo.toml`,
adds a new crate, and depends on all bridge + pretty-printer tasks:

```
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
```

The authoritative gate is `task.py done`, which runs cleanup plus
workspace-wide test/fmt/clippy per `SESSION.md` Testing
Requirements. The above commands are dev-time guidance.

## Acceptance Criteria

- `.harness/tasks.json` group `test-ir-fixtures` added by hand
  (one-off direct edit; documented in this task's
  Implementation Notes as the sole exception).
- Workspace-root `Cargo.toml` `[workspace] members` includes
  `crates/test-ir-fixtures`.
- `crates/test-ir-fixtures/Cargo.toml` created per the shape in
  Implementation Notes (package metadata + `regseq-model` /
  `regseq-driver` deps + `insta` dev-dep).
- `crates/test-ir-fixtures/src/lib.rs` created with the
  `fixture_names` module + `bridge_fixture` + `render_default`
  helper API; `#![warn(missing_docs)]` enabled; doc comments on
  every public item.
- `crates/test-ir-fixtures/fixtures/{no_op, diag_logging,
  trait_object, closure}.rs` created with the source bodies per
  Implementation Notes.
- `crates/test-ir-fixtures/tests/no_op_smoke.rs` created and
  passing under `cargo test -p test-ir-fixtures`; snapshot at
  `tests/snapshots/no_op_smoke__no_op_render.snap` committed.
- All Test Plan items 1-7 implemented and passing.
- `cargo fmt --all -- --check` and `cargo clippy --workspace --
  -D warnings` pass.
- Codex Task Review returns VERDICT: APPROVED.
