# regseq-model-94: E2E -- diagnostic-logging fixture (P1a-E2E-02)

## Description

End-to-end integration test that bridges the canonical
diagnostic-logging fixture (one call to each of
`regseq::log::{debug,info,warn,error}!` with a single `u32`
argument) through the full phase-1a `regseq-driver` pipeline and
asserts the post-bridge `RegseqModel` has the spec-mandated
shape: four flat-arg `Call` Terminators (one per severity), each
satisfying the `I-DiagLog-CallArity` invariant captured at
`spec.md:9650+` / `spec.md:11792+` (post-DiagLogPeel arity =
1 fmt + N flat args, NOT the pre-peel 2 inputs of
`(fmt, &[LogArg<'_>])`); each Call's callee Item carries an
`IntrinsicKind::Log(LogLevel::*)` `AttrData` entry in
`domain_attrs.item_attrs`; the `LogArg` carrier ADT does not
appear anywhere in the post-peel `RegseqModel` (per `spec.md`
§26.4 carrier-survival rule at `spec.md:11962+`); and the
pretty-printed model matches a frozen `insta` snapshot.

This is the second end-to-end fixture in phase-1a (the first is
the no-op smoke in `test-ir-fixtures-01`). It exercises the
diagnostic-logging recognition codepath landed by
`regseq-driver-44` (P1a-BR-08a), the DiagLogPeel body-level pass
landed by `regseq-driver-45` (P1a-BR-08b), and the cumulative
phase-1a IR + pretty-printer surface.

## Input Context

- `.harness/spec-batches/regseqmodel-phase-1a.md` (parent brief;
  P1a-E2E-02 row at axis 5)
- `.harness/SESSION.md` (project overview, Review section
  four-block template)
- `docs/discussions/regseq-model/spec.md` §26 (Diagnostic logging
  intrinsic family; `spec.md:12333+`)
  - §26.2 (severity vocabulary + intrinsic NAME mapping;
    `spec.md:12396+`) -- the four `IntrinsicKind::Log(_)` discriminants
    `LogLevel::Debug` / `Info` / `Warn` / `Error`
    mapping to `log-debug` / `log-info` / `log-warn` / `log-error`
    / `log-error` per `spec.md:1542-1551`.
  - §26.3 (intrinsic function shape; `spec.md:12427+`)
  - §26.4 (`LogArg` argument carrier + carrier-survival rule;
    `spec.md:12481+` / `spec.md:11962+`)
  - §26.5 (author macros + post-expansion shape;
    `spec.md:12639+`)
  - §26.6 (bridge translation contract; `spec.md:12692+`)
- `docs/discussions/regseq-model/spec.md` §25.7.3 (DiagLogPeel
  Phase-5 body construction pass; `spec.md:11772+`)
- `docs/discussions/regseq-model/spec.md` §23.6.2 B-4 / B-6
  (post-peel I-DiagLog-CallArity exception; `spec.md:9650+`,
  `spec.md:9718+`)
- `docs/discussions/regseq-model/spec.md` §3.6 (Domain attribute
  tables; `spec.md:1419+`)
  - §3.6.1 (`DomainAttrTables` + `item_attrs: IndexVec<ItemId,
    Vec<AttrData>>`; `spec.md:1432+`)
  - §3.6.4 (Phase-2 recognition codepath populates
    `item_attrs[item]` with `IntrinsicKind::Log(LogLevel)`)
- `docs/discussions/regseq-model/pretty-printer.md` §10.3
  (top-level render the snapshot is taken against;
  `pretty-printer.md:4201+`)
- `crates/test-ir-fixtures/src/lib.rs` and
  `crates/test-ir-fixtures/fixtures/diag_logging.rs` (the
  fixture file + helper API shipped by `test-ir-fixtures-01`)
- `crates/regseq-model/src/lib.rs` (the `RegseqModel`,
  `DomainAttrTables`, `AttrData`, `IntrinsicKind`, `LogLevel`,
  `Body`, `Terminator`, `TerminatorKind::Call`, `CallCallee`
  types this task asserts against)
- `archive/.harness/tasks/regseq-model/24-place-and-projection.md`
  (the SHAPE / SIZE reference)

## Implementation Notes

This task lands a single integration test file at
`crates/regseq-model/tests/e2e_diag_logging.rs` (the assertion
surface is the `RegseqModel` + pretty-printer, so the file
lives under the `regseq-model` crate per the brief's E2E
placement rule). The test consumes the
`crates/test-ir-fixtures/fixtures/diag_logging.rs` source +
the `bridge_fixture(fixture_names::DIAG_LOGGING)` helper.

### `crates/regseq-model/Cargo.toml` dev-dependency addition

Add to the `[dev-dependencies]` section:

```toml
test-ir-fixtures = { path = "../test-ir-fixtures" }
insta = { version = "1", features = ["yaml"] }
```

If `insta` is already present at workspace level, the bare
`insta = { workspace = true }` form is acceptable; mirror the
existing dev-dep style under `crates/regseq-model/Cargo.toml`.

### `crates/regseq-model/tests/e2e_diag_logging.rs`

The test file's contents (all assertions named explicitly in the
Test Plan below):

```rust
use regseq_model::{
    AttrData, IntrinsicKind, LogLevel, RegseqModel, TerminatorKind,
};
use test_ir_fixtures::{bridge_fixture, fixture_names, render_default};

#[test]
fn diag_logging_post_peel_shape_and_attrs() {
    let model = bridge_fixture(fixture_names::DIAG_LOGGING);

    // Section A: validation.
    model.validate().expect("validate must pass");

    // Section B: locate the four post-peel Call terminators.
    let calls = collect_log_calls(&model);
    assert_eq!(
        calls.len(), 4,
        "expected one Call per severity (debug/info/warn/error)"
    );

    // Section C: assert post-peel arity + flat-arg shape per call.
    for (call, _expected_level) in &calls {
        assert!(
            call.args.len() >= 2,
            "post-peel call must have fmt + >=1 flat arg \
             (I-DiagLog-CallArity, spec.md:9650+ / 11792+)"
        );
        // Per the diag_logging.rs fixture: each macro call passes
        // exactly one `u32` argument, so post-peel arity = 1 fmt
        // + 1 flat = 2.
        assert_eq!(
            call.args.len(), 2,
            "diag_logging fixture passes one u32 -> post-peel arity = 2"
        );
    }

    // Section D: assert no LogArg ADT survives anywhere.
    assert_no_logarg_in_model(&model);

    // Section E: assert each callee carries IntrinsicKind::Log(_).
    let observed_levels = collect_log_levels(&model, &calls);
    assert_levels_match_fixture(&observed_levels);

    // Section F: snapshot.
    insta::assert_snapshot!("diag_logging_render", render_default(&model));
}
```

The helper functions (`collect_log_calls`,
`assert_no_logarg_in_model`, `collect_log_levels`,
`assert_levels_match_fixture`) live as private module-local
helpers in the same test file; their expected behaviour is
specified by the Test Plan section below.

### Conventions

- Doc comments are not required on integration-test private
  helpers (per Rust convention) but ARE required on any
  `pub`-marked helper if the worker chooses to expose one for
  reuse.
- The snapshot file lands at
  `crates/regseq-model/tests/snapshots/e2e_diag_logging__diag_logging_render.snap`.
- Snapshot review flows through `cargo insta review` (developer
  side); committed snapshot is the frozen baseline.
- The fixture's macro calls (per
  `crates/test-ir-fixtures/fixtures/diag_logging.rs`) use
  exactly four severities -- `debug`, `info`, `warn`, `error`.
  The spec ships four `LogLevel` variants total (Debug /
  Info / Warn / Error per §26.2 + the dash-form NAME mapping at
  `spec.md:1542-1551`); the fixture exercises ALL FOUR severities
  so the assertion's `observed_levels` set is exactly
  `{Debug, Info, Warn, Error}`. Test Plan E captures this exactly.
- The `LogArg` carrier-survival assertion (Section D) walks
  every Ty in the model and asserts no `Ty::Adt` / `Ty::Struct`
  / `Ty::Enum` resolves to a DefPath whose final segment renders
  as `LogArg`. The carrier ADT must be entirely consumed by the
  DiagLogPeel pass per `spec.md:11962-11963`.

## Non-Goals

- The bridge implementation itself (recognition codepath,
  DiagLogPeel pass, format-template grammar parser) is
  `regseq-driver-44` (P1a-BR-08a) + `regseq-driver-45`
  (P1a-BR-08b) territory; this task only asserts the bridge's
  output shape.
- The `regseq::log::*` author macros' implementation is
  `regseq-macros` territory (out of phase-1a scope; the macros
  ship pre-existing per `spec.md` §26.5).
- Backend C-emission of diagnostic logging (per `spec.md` §26.9)
  is phase-1b territory.
- The `IntrinsicKind::from_name` closed-vocabulary lookup
  itself (covering all four severities at the data-shape level) is
  unit-tested by `regseq-model-74` (P1a-DS-09); this E2E task only
  asserts the bridge produces the expected per-severity Call
  shapes.
- Format-template grammar edge cases (`{:x}`, `{:#010x}`,
  rejection of `{:?}` per D3 at `spec.md:12840`) are unit-tested
  by `regseq-driver-45` (P1a-BR-08b); this task only verifies
  the post-peel arity + flat-arg shape.
- Snapshot acceptance flow / CI integration of `cargo insta
  review` is out of scope (developer-side, per
  `test-ir-fixtures-01` precedent).

## Test Plan

**Test organization**: `integration-in-tests` -- the test file
lives at `crates/regseq-model/tests/e2e_diag_logging.rs`. Per
the brief's axis 5 placement rule, E2E tests register under the
`regseq-model` group (assertion surface is the `RegseqModel` +
pretty-printer, even though the bridge they exercise lives in
`regseq-driver`).

The test asserts a complete six-section profile against the
post-bridge `RegseqModel`. Every assertion is enumerated below
so the worker can implement each precisely.

### A. Validation pass

1. `bridge_fixture(fixture_names::DIAG_LOGGING)` returns a
   `RegseqModel` without panic. (Helper panics on bridge errors
   per `test-ir-fixtures-01`'s contract.)
2. `model.validate()` returns `Ok(())` (per `spec.md` §3.3
   validation order; covers ID referential integrity, layout
   consistency, and Place type-coherence over every body
   including the four logging-call bodies).

### B. Locate the four post-peel Call terminators

3. Walk `model.instances` -> Function `Instance` for the
   fixture's `emit_logs` function -> its `Body` ->
   `body.basic_blocks` -> per-block `Terminator`. Collect every
   `TerminatorKind::Call { callee: CallCallee::Direct(item), .. }`
   whose `item`'s `domain_attrs.item_attrs[item]` Vec contains
   at least one `AttrData::Intrinsic(IntrinsicKind::Log(_))`
   entry.
4. Assert exactly **four** such Calls were collected (one per
   `debug` / `info` / `warn` / `error` invocation in the
   fixture).

### C. Post-peel arity + flat-arg shape per Call

5. For each of the four Calls:
   - `call.args.len() == 2`. The fixture passes one `u32` to
     each macro call; the post-DiagLogPeel arity per
     `spec.md:11792+` is `1 + N` where `N` is the flat-arg
     count, so `1 + 1 = 2`.
   - `call.args[0]` is a fmt operand whose source `Ty` is a
     `&'static str`. Specifically, `call.args[0]` is an
     `Operand::Constant(...)` whose const Ty is `&str` (per
     `spec.md:9658+`).
   - `call.args[1]` is a flat `u32` operand (NOT a slice
     aggregate, NOT a `LogArg::U32(_)` const wrapper). The
     operand's resolved `Ty` is `Ty::Uint(UintKind::U32)`.
6. **No 2-input pre-peel shape survives.** No Call collected in
   step 4 has `args.len() == 2` with `args[1]` typed as a
   slice / array of `LogArg<'_>`. (This is the corollary of
   §26.4's carrier-survival rule at the Call-site granularity:
   if any post-bridge Call has the pre-peel signature,
   DiagLogPeel did not fire for it.)

### D. `LogArg` carrier-survival negative assertion

7. Walk every `Ty` in the model's type interner and assert no
   `Ty` resolves to an ADT whose terminal `DefPath` segment
   renders as `LogArg`. The walk must cover:
   - All `Ty::Adt(item, args, ...)` / `Ty::Struct(item, ...)` /
     `Ty::Enum(item, ...)` variants per the `Ty` enum from
     `regseq-model-67` (P1a-DS-02).
   - All transitive `Ty` references (function-signature inputs/
     outputs, local-decl Tys, projection Tys).
8. **Stronger form.** Walk every `Item` in `model.items` and
   assert no Item's `def_path` rendered form contains the
   string `"LogArg"`. The `LogArg` ADT lives in the
   `regseq` crate's source; if it appears in `model.items`
   the bridge has incorrectly preserved it (per
   `spec.md:11962-11963`).

### E. `IntrinsicKind::Log(_)` attribute population per callee

9. For each of the four collected Calls (step 3), inspect the
   callee `Item`'s `domain_attrs.item_attrs[item]` Vec.
   Filter to `AttrData::Intrinsic(IntrinsicKind::Log(level))`
   entries; assert exactly one such entry per callee Item.
10. Build the multiset `observed_levels: BTreeSet<LogLevel>`
    from the four `level` values extracted in step 9.
11. Assert `observed_levels == {LogLevel::Debug,
    LogLevel::Info, LogLevel::Warn, LogLevel::Error}` exactly
    (the full §26.2 four-severity vocabulary; the fixture
    exercises every locked LogLevel variant).
12. **No spurious attributes.** Assert no other `AttrData`
    variant (e.g., `AttrData::Mmio(_)` if the enum exists at
    phase-1a) appears on these four callee Items'
    `item_attrs` vectors -- the diagnostic-logging callees
    must carry exclusively their `IntrinsicKind::Log(_)`
    decoration at phase-1a.

### F. Pretty-printed snapshot

13. `render_default(&model)` returns a non-empty `String`.
14. `insta::assert_snapshot!("diag_logging_render",
    render_default(&model))` matches the committed snapshot at
    `crates/regseq-model/tests/snapshots/e2e_diag_logging__diag_logging_render.snap`.
15. **Snapshot baseline shape** (verifiable by inspection of
    the committed `.snap` file):
    - The `emit_logs` Function Instance is rendered with its
      DefPath + signature envelope per `pretty-printer.md` §10.2.1.
    - The body envelope contains four `call ...` lines (one per
      severity), each with the post-peel `(fmt, u32)` argument
      pair rendered per `pretty-printer.md` §7.7
      `render_call_callee` + §7.2 `render_operand` pair-form.
    - Each callee Item's pretty-printed form carries an
      attribute decorator per `pretty-printer.md` §10.2.7
      Stream 2 (DomainAttr walk over `item_attrs`) reflecting
      `IntrinsicKind::Log(LogLevel::*)` -- the exact rendered
      form follows the §10.2.7 attribute-stream rendering rule.
    - No `LogArg` identifier appears anywhere in the rendered
      output (a textual mirror of Section D's structural
      assertion).

`cargo test -p regseq-model --test e2e_diag_logging` passes.
`cargo test --workspace` regression-clean.

## Review

Once tests pass, request a Codex review of the implementation by
invoking `scripts/python/codex_runner.py` via the Bash tool (per
`SESSION.md` Review section). Use the four-block template
verbatim:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-model-94 against its task spec
at .harness/tasks/regseq-model/94-e2e-diagnostic-logging.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-model/Cargo.toml (dev-dependencies addition)
- crates/regseq-model/tests/e2e_diag_logging.rs
- crates/regseq-model/tests/snapshots/e2e_diag_logging__diag_logging_render.snap

Focus on:
- Spec deviations (every rule in the Implementation Notes section).
- Correctness issues and generality loss; in particular, that
  the test asserts the *post-peel* (1 + N flat) Call shape, not
  the pre-peel (fmt, &[LogArg]) shape.
- Design misalignment with the spec's public API / architecture.
- Test coverage vs. the spec's Test Plan -- every Test Plan
  enumeration item (A1-F15) must be present.
- The LogArg carrier-survival negative assertion (Section D)
  must scan both Tys and Item DefPaths.
- The `IntrinsicKind::Log` level-set assertion must be exactly
  `{Debug, Info, Warn, Error}` (all four locked severities).
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

Workspace-wide because this task spans `regseq-model` (assertion
surface + new test file) + `test-ir-fixtures` (helper crate
consumed) + `regseq-driver` (bridge it exercises end-to-end):

```
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
```

The authoritative gate is `task.py done`, which runs cleanup plus
workspace-wide test/fmt/clippy per `SESSION.md` Testing
Requirements. The above commands are dev-time guidance.

## Acceptance Criteria

- `crates/regseq-model/Cargo.toml` `[dev-dependencies]`
  augmented with `test-ir-fixtures` (path) + `insta`.
- `crates/regseq-model/tests/e2e_diag_logging.rs` created with
  the test `diag_logging_post_peel_shape_and_attrs` covering
  every Test Plan enumeration item A1 through F15.
- `crates/regseq-model/tests/snapshots/e2e_diag_logging__diag_logging_render.snap`
  committed; baseline shape per Test Plan F15 (four `call` lines,
  post-peel `(fmt, u32)` arg shape, per-callee `IntrinsicKind::Log`
  attribute decorator, no `LogArg` identifier anywhere).
- `cargo test -p regseq-model --test e2e_diag_logging` passes.
- `cargo test --workspace` regression-clean.
- `cargo fmt --all -- --check` and `cargo clippy --workspace --
  -D warnings` pass.
- Codex Task Review returns VERDICT: APPROVED.
