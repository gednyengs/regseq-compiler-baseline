# Bridge Coverage Runner -- Strategy (v2)

A self-running coverage harness that turns
`docs/discussions/regseq-model/bridge-coverage-matrix.md` into an
executable spec: each matrix row becomes a one-shot bridge experiment,
each generated snippet is the input, the bridge run is the
measurement, and the failure log is the triage queue.

Revision note: v2 addresses Codex round-1 strategy review findings
(see "Change log" at end). The most significant deltas are
(1) typed-oracle API contract written out explicitly, (2) `must_intern`
collapsed to a closed `InspectionPredicate` enum, (3) phasing
re-estimated 15-25 agent-hours (was ~10), (4) F1 scope narrowed to
the typed-oracle prerequisite + 10-row demo before any full-corpus
work, (5) added rustc-ICE taxonomy, sidecar drift checks, and snippet
review/merge policy.

## 1. Goal

Replace the ~133-fixture-per-row task-spec batch with a single,
re-runnable runner that exercises every uncovered matrix row in
seconds-to-minutes and produces a deterministic pass/fail report.
The triage queue (failures grouped by `BridgeError` variant + panic
frame + row context) is then proposed (not auto-filed) into the
harness as ordinary `regseq-driver-NN` tasks.

## 2. Scope

**In scope:**
- Per-row source-level snippets at
  `crates/bridge-coverage-runner/snippets/<row_id>.rs`.
- A runner binary that loads each snippet, drives it through the
  bridge via a typed-outcome helper, classifies the outcome against
  the row's `expectation_class`, and emits a markdown triage report.
- Typed-outcome oracle (no regex on error messages).
- Closed-enum inspection predicates for the "did this snippet
  actually exercise the construct?" check.
- Failure clustering by row-id-aware fingerprint -> per-cluster
  triage entry.
- Manual snippet review gate before merging snippets to the
  committed corpus.

**Out of scope (initial landing):**
- Random / generative input (this is NOT a fuzzer).
- Reduction pipeline (snippets are minimal by construction).
- Section 7 validator-invariant rows (constructed-model tests;
  separate track).
- CI gating, CI nightly job (deferred until corpus stabilises).
- Auto-filing of `regseq-driver-NN` tasks. The runner proposes;
  humans / agents file via `task.py add-task` after approval per
  `.harness/task-spec-workflow.md:118-123`.
- Public library API for in-process agent consumption (deferred;
  binary-only initially).
- Rich JSON output for downstream automation (deferred; markdown
  only initially).

## 3. Architecture

```
crates/bridge-coverage-runner/
  Cargo.toml
  src/
    matrix.rs         -- matrix-row metadata loader (reads rows.toml)
    snippet.rs        -- per-snippet load + name<->row mapping
    oracle.rs         -- typed bridge invocation (calls the new
                         helper added to test-ir-fixtures)
    inspect.rs        -- post-bridge model inspection
                         (closed InspectionPredicate enum dispatch)
    cluster.rs        -- failure fingerprint + dedupe (simple shape;
                         no advanced grouping in F1)
    report.rs         -- markdown report emitter
    main.rs           -- CLI entry point + run loop
  snippets/
    README.md         -- one-paragraph row-id naming convention
    <row_id>.rs       -- one per row (e.g., `TY-001.rs`, `RV-006-A.rs`)
  matrix_metadata/
    rows.toml         -- machine-readable metadata sidecar
                         (per-row id, expected outcome, inspection
                         predicate)
  tests/
    smoke.rs          -- runner runs end-to-end on a 3-row mini-corpus
  reports/            -- generated, gitignored
```

Reuses a new typed-outcome helper added to
`crates/test-ir-fixtures/src/lib.rs` (see Section 6). The runner
crate has zero rustc deps; it depends on `regseq-model` for
inspection types and on `test-ir-fixtures` for the helper.

**No `src/lib.rs`** in F1: binary-only. Public library surface for
in-process agent consumption is deferred until Mode B work in F4+.

## 4. Matrix metadata sidecar

Sidecar at `crates/bridge-coverage-runner/matrix_metadata/rows.toml`.
The matrix markdown remains the human-readable source of truth; the
sidecar is the machine-readable derived shape the runner consumes.

```toml
# Per-row entry shape:
[[row]]
id = "TY-018"
section = "1.1"
construct = "Ty::Enum"
expectation = "positive"   # positive | rejected | validator-failing
status = "covered"
fixture_path = "crates/test-ir-fixtures/fixtures/enum_round_trip.rs"
snippet_path = "snippets/TY-018.rs"  # absent if status != "uncovered"

# Inspection (Section 7); see InspectionPredicate enum below.
[row.inspect]
kind = "TyVariant"
arg = "Enum"

# For rejected rows:
[[row]]
id = "RJ-001"
expectation = "rejected"
expected_rejection = { variant = "DslRejection", kind = "Box" }
```

Format choice: **TOML**. Rationale per Codex round-1:
- `.harness/task-spec-status.yaml` is harness planning state (YAML).
- `.harness/tasks.json` is harness execution state (machine-managed JSON).
- This sidecar is hand-authored Rust-owned metadata; TOML matches
  `Cargo.toml` conventions and has the cleanest Rust ecosystem
  story (`toml` crate).
- JSON used ONLY for generated reports if/when needed.

### 4.1 Sidecar-vs-markdown drift check

The runner refuses to run if the sidecar is out of sync with the
matrix's row IDs. The drift check, implemented as a runner
pre-flight step:

1. Parse the matrix markdown table headers (Section 1-6) and
   extract the leftmost `id` cell of every row.
2. Cross-reference against `rows.toml` row IDs.
3. Refuse to run if any row ID is in one but not the other.
4. Output a diff so the operator can decide whether to update the
   sidecar or the matrix.

This pre-flight is the only sanity barrier between the
human-authored matrix and the machine-consumed sidecar; the matrix
remains the source of truth.

## 5. Snippet authoring strategy

**Decision: LLM-driven authoring with a verification gate AND a
manual review gate before merging snippets to the committed corpus.**

For each row whose status is `uncovered`:

1. Run an agent batch (size: ~20 rows per agent) with the row's
   `construct`, `expectation`, `inspect` predicate, and matrix
   notes as the prompt payload.
2. Agent emits a candidate `#![no_std]` snippet to
   `snippets/<row_id>.rs` in a sandboxed branch.
3. Runner attempts compilation via the typed-oracle helper.
4. If compilation succeeds AND the inspection predicate confirms
   the construct's presence in the resulting `RegseqModel`, the
   snippet is "candidate accepted."
5. If compilation fails with an expected `BridgeError` variant
   matching the row's `expected_rejection`, the snippet is also
   "candidate accepted."
6. 3 retries before escalating to a human matrix-author task.
7. **Manual review gate (NEW per Codex round-1)**: candidate-
   accepted snippets land in a per-batch PR for human review
   before merging to the committed corpus. The review confirms
   (a) the snippet is minimal (doesn't drag in unrelated
   constructs that mask coverage of other rows), (b) the snippet
   exercises the row's INTENDED semantic shape (not a degenerate
   shape that happens to intern the right thing). Inspection
   proves construct presence; it does NOT prove minimality or
   semantic intent.

   **Ownership (per Codex round-2 finding 5)**: human review is
   the project lead's call (the matrix architect role). Agents
   MAY pre-review and annotate the PR with their own analysis,
   but the merge-to-corpus decision is human. PRs are batched
   per ~20-snippet authoring run; a batched PR is small enough
   for one reviewer to walk in 15-30 minutes. The PR description
   carries the agent's per-snippet inspection-pass output as
   evidence; the reviewer's job is the minimality + semantic-
   intent check, not the syntactic compile check.

**No hand-authoring shortcut for "trivial" rows.** Even
`TY-001 (Ty::Bool)` goes through the LLM batch so the verification
+ inspection + review loop is uniform.

## 6. Typed-outcome oracle

### 6.1 Prerequisite work in `test-ir-fixtures`

`crates/test-ir-fixtures/src/lib.rs` currently:
- exposes only `bridge_fixture(name) -> RegseqModel` at `:90`
- routes through `compile_and_bridge(name, src) -> RegseqModel` at `:161`
- converts `BridgeError` to `String` inside the rustc-driver callback
  at `:228-231` (typed information LOST before surfacing)
- panics on bridge error at `:257` and on validator error at `:263`

To expose the typed outcome, two layered changes:

**Change A (internal)**: lift the callback's result type from
`Result<RegseqModel, String>` to `Result<RegseqModel, BridgeError>`.
Replace `Err(format!(...))` at `lib.rs:230` with `Err(err)`. The
mutex now carries typed `Result<RegseqModel, BridgeError>` (not a
stringly-typed error).

**Change B (public surface)**: introduce a sibling helper that
returns the typed outcome AND distinguishes validator failure
from bridge failure:

```rust
/// Typed result of a fixture compile + bridge + validate sweep.
///
/// `Ok` means BOTH bridge AND model.validate() succeeded.
/// Distinct rejection / validator-failure / rustc-rejected /
/// no-result variants are exposed so the bridge-coverage runner
/// can classify outcomes without parsing error strings.
pub enum BridgeOutcome {
    /// Bridge succeeded AND model.validate() succeeded.
    Ok(RegseqModel),
    /// Bridge succeeded but validate() failed. The model
    /// is returned alongside the violation so the caller can
    /// pretty-print or inspect.
    ValidatorFailed {
        model: RegseqModel,
        violation: InvariantViolation,
    },
    /// Bridge returned a typed BridgeError.
    Rejected(BridgeError),
    /// Rustc exited before after_analysis was reached
    /// (rustc rejected the input, or an internal rustc ICE).
    /// `stderr` is captured for diagnostics.
    RustcRejected { stderr: String },
}

/// Typed entry point for fixtures already shipped under
/// `crates/test-ir-fixtures/fixtures/<name>.rs`.
pub fn try_bridge_fixture(name: &str) -> BridgeOutcome { ... }

/// Typed entry point for arbitrary source paths (used by the
/// bridge-coverage runner against snippets at runtime).
pub fn try_bridge_fixture_path(path: &Path) -> BridgeOutcome { ... }
```

The existing `bridge_fixture(name) -> RegseqModel` is rewired to
call `try_bridge_fixture(name)` and `panic!` on every non-`Ok`
variant -- backwards-compatible per Codex round-1's audit of
existing callers (all expect either `RegseqModel` or panic; no
caller treats `bridge_fixture` as fallible).

### 6.2 Panic capture

The typed-outcome helpers do NOT catch panics thrown by the bridge
itself (bugs panic; the runner wraps each invocation in
`std::panic::catch_unwind` and synthesises a fifth outcome class
`Panic { message: String }` in its own classifier).

### 6.3 Rustc-ICE / no-after-analysis taxonomy

`BridgeOutcome::RustcRejected` covers three sub-cases the runner
must distinguish:

- **rustc-clean-reject**: rustc emitted normal compile errors and
  exited; the snippet is bad Rust. Triage as a snippet-authoring
  bug.
- **rustc-internal-ICE**: rustc panicked or emitted an ICE
  ("internal compiler error"). Triage as an upstream rustc bug or
  a bridge prereq that exercises a rustc weakness; NOT a Regseq
  bridge bug.
- **rustc-no-after-analysis**: rustc completed but `after_analysis`
  was never reached (e.g., empty crate with no items). Triage as
  a fixture-design issue.

**Capture mechanism** (per Codex round-2 finding 7): the existing
in-process `rustc_driver::run_compiler` path at
`crates/test-ir-fixtures/src/lib.rs:240` does NOT structurally
capture stderr. The runner takes a two-step approach:

1. **rustc pre-flight as subprocess**: before each snippet, the
   runner shells out to the active rustc with
   `--crate-type lib --edition 2021 -Zno-codegen` (metadata-only)
   in subprocess mode, capturing stderr. If rustc exits non-zero,
   classify as `rustc-clean-reject` (if stderr lacks the ICE
   marker) or `rustc-internal-ICE` (if stderr contains
   `"internal compiler error"` / `"thread 'rustc' panicked"`).
   On rustc-success, proceed to step 2.
2. **In-process bridge run**: invoke `try_bridge_fixture_path`
   which uses the existing `rustc_driver::Callbacks` path. The
   structural failure modes here are `Rejected(BridgeError)` /
   `ValidatorFailed` / `Ok`; `RustcRejected` should not surface
   here because step 1 already filtered rustc-incompatible inputs.

The `rustc-no-after-analysis` sub-class can only surface from
step 2 (rustc accepted the input but the after_analysis callback
was never reached -- e.g., empty source). It's a low-likelihood
fixture-design issue, surfaced via the `RustcRejected { stderr }`
variant with an empty `stderr` and the runner's classifier
treating it as the third sub-class.

This split (subprocess pre-flight + in-process bridge) keeps
typed `BridgeError` information intact while still capturing
rustc stderr cleanly. Subprocess overhead is ~50-100ms per
snippet; acceptable for the runner's design target of seconds-to-
minutes per full pass.

## 7. Inspection predicates (anti-silent-pass)

For `expectation = positive` rows: after a successful compile, the
runner walks the resulting `RegseqModel` and verifies the row's
claimed construct actually appears. The sidecar's `[row.inspect]`
field carries a closed `InspectionPredicate` discriminator + arg.

Closed enum (per Codex round-1, replacing the spread-`must_intern_*`
shape):

```rust
pub enum InspectionPredicate {
    /// At least one `model.types` entry matches the named `Ty` variant.
    /// `arg` is the variant tag ("Bool", "Char", "Enum", "Closure", ...).
    TyVariant { arg: String },
    /// At least one `model.items[_].kind` matches the named `ItemKind`.
    ItemKind { arg: String },
    /// At least one `model.item_synthetic_kinds[_]` matches the named
    /// `SyntheticItemKind` variant.
    SyntheticItemKind { arg: String },
    /// At least one Function Instance's flavor matches.
    FunctionInstanceFlavor { arg: String },
    /// At least one body's statement matches the named `StatementKind` variant.
    StatementKind { arg: String },
    /// At least one body's terminator matches the named `TerminatorKind` variant.
    TerminatorKind { arg: String },
    /// At least one body's rvalue matches the named `Rvalue` variant.
    RvalueVariant { arg: String },
    /// At least one body's PlaceElem in any projection matches.
    PlaceElem { arg: String },
    /// At least one Const matches the named `Const` variant.
    ConstVariant { arg: String },
    /// At least one Rvalue::Aggregate uses the named AggregateKind variant.
    AggregateKind { arg: String },
    /// At least one Rvalue::Cast uses the named CastKind variant.
    CastKind { arg: String },
    /// At least one Rvalue::BinaryOp uses the named BinOp variant.
    BinOp { arg: String },
    /// At least one Rvalue::UnaryOp uses the named UnOp variant.
    UnOp { arg: String },
    /// At least one Terminator::Assert uses the named AssertKind variant.
    AssertKind { arg: String },
    /// At least one Statement::Intrinsic uses the named NonDivergingIntrinsic
    /// sub-variant ("Assume" or "CopyNonOverlapping").
    NonDivergingIntrinsic { arg: String },
    /// At least one Call terminator's callee matches the named
    /// CallCallee variant ("Direct" or "Indirect").
    CallCallee { arg: String },
    /// At least one Call terminator has `target == None`
    /// (diverging call) when `arg == "true"`, or `target.is_some()`
    /// when `arg == "false"`.
    CallDivergent { arg: bool },
    /// At least one Function Instance has `body == None`.
    BodyAbsent,
    /// At least one Item carries the named Linkage variant
    /// ("Default" / "NoMangle" / "Foreign" / "Weak").
    Linkage { arg: String },
    /// At least one Ty::Int / Ty::Uint / Ty::Float carries the
    /// named scalar-kind variant ("I8" / "U32" / "F64" / ...).
    /// The runner walks model.types and checks the kind tag on
    /// Int/Uint/Float entries.
    ScalarKind { arg: String },
    /// Composite: ALL of the listed predicates hold.
    All { predicates: Vec<InspectionPredicate> },
    /// Composite: AT LEAST ONE of the listed predicates holds.
    Any { predicates: Vec<InspectionPredicate> },
}
```

Each `<row>.inspect` field in `rows.toml` MUST resolve to a
variant of this enum. The `inspect.rs` module dispatches on the
discriminator to walk the relevant slice of the model. New
inspection predicates require an enum addition + a dispatch arm
-- a small, reviewable change rather than a free-form schema
extension.

## 8. Failure clustering / fingerprint

Per Codex round-1's "fingerprint over/under-dedupe" warning, the
fingerprint carries more context than top-frame + message stem.
Per Codex round-2's finding 3 (string-prefix heuristic for
multi-level IDs like `OP-003-G-1` is ambiguous), parent identity
is now an EXPLICIT sidecar field, not a derivation:

```toml
# Per-row entry shape (excerpt; full shape in §4):
[[row]]
id = "OP-003-G-1"
cluster_key = "OP-003-G"   # explicit cluster parent; the runner
                            # never tries to derive it from string-
                            # splitting the id.
# ... (rest of row metadata)
```

```
Fingerprint =
  (cluster_key,          // explicit sidecar field; "TY-018" for
                         // top-level rows, "RV-006" for any
                         // RV-006-A..Z sub-axis row, "OP-003-G"
                         // for OP-003-G-1..3 nested sub-rows, etc.
   expectation_class,    // positive | rejected | validator-failing
   actual_outcome,       // Ok | Rejected(variant) | ValidatorFailed(variant)
                         //   | RustcRejected(sub-kind) | Panic(top_frame)
   variant_or_phase,     // For Rejected: BridgeError variant name.
                         // For Panic: top non-std/rustc frame file:line.
                         // For ValidatorFailed: InvariantViolation variant.
                         // For RustcRejected: clean-reject | ICE |
                         //   no-after-analysis.
   snippet_hash)         // Optional SHA-256 first-8-bytes of snippet
                         // content, for cases where two rows produce
                         // the same fingerprint but distinct snippets.
```

`cluster_key` is a top-level field per row, not a derivation. For
top-level rows it equals the row id; for sub-axis rows it names
the parent. The matrix authors set it explicitly when authoring
`rows.toml`; the runner reads it without any inference. This
balances dedupe (sibling sub-axes that fail the same way cluster
together) with traceability (different parent rows don't
accidentally merge) and removes string-heuristic ambiguity for
multi-level IDs.

In F1, the cluster shape is INTENTIONALLY simple: hash the tuple,
group by hash. No sophisticated similarity scoring, no minimal-
repro selection beyond "shortest snippet in the cluster." Richer
clustering is deferred until the F1 + 10-row demo proves the basic
loop.

## 9. Reporting

In F1: markdown only. `reports/<timestamp>/report.md` per run.

Shape:
- Header: total rows / pass / fail / by-class / matrix coverage delta
  vs the previous run.
- Section per failure cluster (sorted by cluster size desc).
- Per-cluster: fingerprint, list of `(row_id, snippet_path,
  one-line detail)`, the shortest snippet in the cluster as a
  representative repro.

JSON report and automation consumers are deferred.

## 10. How we run it

**Mode A (local developer, F1):**

```bash
cargo run -p bridge-coverage-runner -- \
  --metadata crates/bridge-coverage-runner/matrix_metadata/rows.toml \
  --snippets crates/bridge-coverage-runner/snippets/ \
  --out reports/$(date +%Y%m%d-%H%M%S)/
```

Single-process, sequential per snippet. ~5-10 min for the full
~300-row matrix once authored; ~30 seconds for the 10-row F1 demo.

**Mode B (agent-driven, deferred to F4+):**

An agent dispatched with "run the bridge-coverage-runner, inspect
the resulting `report.md`, **propose a decomposition** of the
failures into `regseq-driver-NN` tasks for human approval before
filing." Wording is "propose decomposition", NOT "file from
report" -- the harness rule at `.harness/task-spec-workflow.md:118-123`
requires explicit approval before `task.py add-task`.

**Mode C (CI nightly):** deferred until corpus stabilises.

## 11. Phasing (revised per Codex round-1)

| Phase | Scope | Estimated agent-hours |
| ----- | ----- | --------------------- |
| F0    | Typed-oracle prerequisite: `BridgeOutcome` enum + `try_bridge_fixture[_path]` + rewire `bridge_fixture` to call the typed path. Smoke test against existing fixtures. | 2-3 |
| F1    | Scaffold runner crate (matrix.rs / snippet.rs / oracle.rs / inspect.rs / cluster.rs / report.rs / main.rs); author `rows.toml` covering ~10 demo rows; author 10 demo snippets by hand; smoke test the loop end-to-end. | 3-4 |
| F2    | First snippet batch: ~30 high-priority uncovered rows (Section 5 interactions + Section 6 rejection variants). LLM-author per Section 5 + verification gate + manual review. | 3-4 |
| F3    | Inspection-predicate dispatch hardening + failure clustering polish + report shape refinement based on F2 findings. | 2 |
| F4    | Full snippet batch: ~100 remaining uncovered rows. LLM-author per Section 5 + manual review. Parallelizable across agents. | 5-8 |
| F5    | Mode B agent integration + harness ingestion patterns + CI scaffolding (Mode C). | 2-3 |

**F0 + F1 = ~5-7 agent-hours = "validate the loop" milestone.**
This is the user-requested gate before committing to the full
corpus automation. Do not proceed past F1 until the 10-row demo
demonstrates the typed-oracle + inspection + report shape work
end-to-end against committed code.

Full F0-F4 total: **15-19 agent-hours**. F5 + CI: **+2-3
agent-hours**. Aligned with Codex round-1's re-estimated 15-25h
range, with the lower bound contingent on LLM authoring throughput.

## 12. Open questions for Codex (round 2)

1. **`BridgeOutcome` enum shape (Section 6)**: is the 4-variant
   enum (`Ok` / `ValidatorFailed` / `Rejected` / `RustcRejected`)
   the right granularity? Should `Panic` be a 5th variant at the
   helper level, or kept as runner-level synthesis via
   `catch_unwind`?
2. **Validator-failed variant fields**: `ValidatorFailed { model,
   violation }` returns the model so the caller can pretty-print.
   Is returning the model worth the extra allocation/memory cost,
   or should the helper return just the `InvariantViolation`?
3. **InspectionPredicate composability (Section 7)**: the
   `All`/`Any` composite variants enable arbitrary nesting. Is
   the resulting expressiveness needed for the matrix's row set,
   or should the enum stay flat (no composites) and rely on the
   sidecar to list multiple predicates per row?
4. **Sidecar drift check granularity (Section 4.1)**: refuse-to-
   run on ANY drift is strict; should the drift check warn + skip
   stale rows instead?
5. **Snippet review gate ownership (Section 5)**: is the
   manual-review PR step a human review, an agent review, or
   either? The harness has no explicit "snippet review" task
   shape today.
6. **Cluster fingerprint stability (Section 8)**: the
   `row_id_prefix` heuristic (full ID for non-sub-axis rows,
   parent ID for sub-axis rows) is hand-curated. Is there a
   structural way to derive the parent automatically from the
   sidecar?
7. **Phasing realism (Section 11)**: re-estimated to 15-25
   agent-hours. F0 (typed oracle) is the single load-bearing
   prerequisite; F4 (full corpus) is parallelizable. Is the
   sequencing right, or should F2 + F4 collapse (one big snippet
   batch instead of two)?
8. **F0 backwards-compat audit**: Codex round-1 verified
   `bridge_fixture` callers expect `RegseqModel`/panic semantics
   (`tests/no_op_smoke.rs:15`). Are there any callers OUTSIDE
   `crates/test-ir-fixtures/` and `crates/regseq-model/tests/`
   that would break under the F0 rewire?

## 13. Change log

- **v3 (current)**: addressed Codex round-2 minor fixes:
  - **§7 inspection predicates**: added 6 missing variants:
    `NonDivergingIntrinsic`, `CallCallee`, `CallDivergent`,
    `BodyAbsent`, `Linkage`, `ScalarKind` (per Codex round-2
    finding 2 -- prior closed enum missed predicates required
    for ST-003 / TM-005-A..D / BD-009/010 / CT-010 /
    TY-003.x / TY-004.x / TY-005.x sub-axis rows).
  - **§8 fingerprint**: replaced the implicit `row_id_prefix`
    derivation heuristic with an EXPLICIT `cluster_key` field
    on each `rows.toml` entry (per Codex round-2 finding 3 --
    multi-level IDs like `OP-003-G-1` make string heuristics
    ambiguous; explicit metadata removes inference).
  - **§6.3 rustc capture mechanism**: specified subprocess-based
    rustc pre-flight (captures stderr cleanly via
    `Command::output()`) followed by in-process bridge run
    (preserves typed `BridgeError`). Resolves the in-process-
    rustc-stderr-capture gap Codex flagged.
  - **§5 review-gate ownership**: specified that human review is
    the project-lead role; agents may pre-review and annotate.
    PRs batched per ~20-snippet authoring run for reviewer
    cadence.

- **v2 (initial revision)**: addressed Codex round-1 strategy findings:
  - **§6 typed oracle**: wrote out the `BridgeOutcome` enum
    contract explicitly with the four named variants
    (Ok/ValidatorFailed/Rejected/RustcRejected). Specified the
    two-layered Change A (internal callback result type lift)
    + Change B (public sibling helper) approach. Added §6.3
    rustc-ICE / no-after-analysis taxonomy.
  - **§7 inspection**: collapsed the spread-`must_intern_*`
    sidecar shape into a closed `InspectionPredicate` enum
    with named variants per inspection kind, plus
    `All`/`Any` composites.
  - **§8 fingerprint**: expanded from `(top_frame, message_stem)`
    to `(row_id_prefix, expectation_class, actual_outcome,
    variant_or_phase, snippet_hash)`. Cluster shape kept simple
    for F1; richer grouping deferred.
  - **§3 architecture**: dropped `src/lib.rs` from F1 (binary-
    only initially). Public library API for in-process agent
    consumption deferred until F5+.
  - **§4 sidecar**: added §4.1 drift check; locked TOML format
    with rationale.
  - **§5 snippet authoring**: kept 3-retry LLM as escalation
    default; added MANUAL REVIEW GATE before merging snippets
    to the committed corpus (Codex flagged that inspection
    proves construct presence but NOT minimality or semantic
    intent).
  - **§9 reporting**: deferred JSON output and automation
    consumers; markdown only in F1.
  - **§10 modes**: reworded Mode B as "propose decomposition"
    not "file from report" per the harness approval rule.
    Deferred Mode B + Mode C to F5+.
  - **§11 phasing**: re-estimated 15-25 agent-hours (was ~10).
    Split F0 (typed-oracle prerequisite) from F1 (scaffold +
    10-row demo). F0 + F1 = the validate-the-loop milestone.
  - **§12 questions**: refreshed open question list to cover
    the new design decisions (enum shape, predicate
    composability, drift granularity, review gate ownership,
    fingerprint stability, F0 backwards-compat).

- **v1 (initial draft)**: superseded; see /tmp/bridge_coverage_runner_strategy_v1.md
  for the archived shape.
