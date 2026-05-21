# regseq-driver-64: Scaffold `bridge-coverage-runner` crate + 10-row demo (F1 of bridge-coverage-runner)

## Description

Scaffold `crates/bridge-coverage-runner/` per
`docs/discussions/regseq-model/bridge-coverage-runner-strategy.md`
§3 (Codex-approved v3 at HEAD) -- the F1 deliverable of the
bridge-coverage-runner strategy. This task lands the runner binary
crate with all seven source modules (`matrix.rs` / `snippet.rs` /
`oracle.rs` / `inspect.rs` / `cluster.rs` / `report.rs` / `main.rs`),
a 10-row TOML metadata sidecar covering a representative slice of
the matrix's positive + rejected expectation classes, ten
hand-coded `#![no_std]` snippets, and an end-to-end smoke test that
runs the binary against a 3-row mini-corpus and asserts on the
emitted markdown report.

F1 builds on regseq-driver-63 (closed at `174c7ff`) which shipped
the F0 typed-outcome surface (`BridgeOutcome` enum +
`try_bridge_fixture_path`) at
`crates/test-ir-fixtures/src/lib.rs:45-189`. F1 is binary-only per
strategy §3 ("No `src/lib.rs` in F1"); a public library API for
in-process agent consumption is deferred until F4+ (Mode B).

F1 deliberately does NOT do any LLM-driven snippet authoring (that
is F2/F4), does NOT exercise the full ~133-uncovered-row matrix
batch (deferred to F2 + F4), does NOT add CI gating (deferred to
F5), and does NOT add JSON report output (deferred per strategy
§9). The deliverable is the validate-the-loop milestone described
in strategy §11: prove the typed-oracle + inspection + cluster +
report pipeline works end-to-end against committed code so F2+
work can proceed without re-litigating runner architecture.

The 10-row demo set is hand-picked to exercise every component of
the loop at least once -- multiple `InspectionPredicate` dispatch
arms, both expectation classes (`positive` + `rejected`), composite
`All`/`Any` predicates, the subprocess rustc pre-flight code path,
and the cluster-fingerprint shape. F0's panic=abort enforcement at
`crates/test-ir-fixtures/src/lib.rs:322` is inherited transparently
by every snippet via `try_bridge_fixture_path`; F1 does not add a
parallel panic-strategy mechanism.

## Input Context

### Design source

- `docs/discussions/regseq-model/bridge-coverage-runner-strategy.md`
  (Codex-approved v3 at HEAD, committed at `4012635`). The strategy
  doc is the LOAD-BEARING design source-of-truth. Specific section
  anchors this task consumes:
  - §3 (Architecture): the 7-module decomposition + crate layout +
    the explicit "no `src/lib.rs` in F1" decision.
  - §4 (Matrix metadata sidecar): TOML format with worked examples;
    every per-row field this task's sidecar must carry.
  - §4.1 (Sidecar-vs-markdown drift check): pre-flight drift
    detection. SEE OPEN QUESTION (a) BELOW -- this task adopts a
    `subset-OK` granularity for F1 that deviates from the strict
    rule §4.1 sketches.
  - §5 (Snippet authoring strategy): F1 hand-codes 10 snippets;
    LLM authoring is F2 work.
  - §6 (Typed-outcome oracle) + §6.1 (`BridgeOutcome` shape) +
    §6.2 (panic capture via `catch_unwind`) + §6.3 (rustc-ICE
    taxonomy + subprocess pre-flight): the oracle module's contract.
  - §7 (Inspection predicates): the 23-variant `InspectionPredicate`
    enum. SEE OPEN QUESTION (b) BELOW -- this task ships the
    complete enum but implements dispatch arms only for the demo
    set's predicates; remaining variants return
    `InspectError::Unimplemented`.
  - §8 (Failure clustering / fingerprint): the explicit
    `cluster_key` per-row field; the fingerprint tuple.
  - §9 (Reporting): markdown-only in F1.
  - §10 (How we run it): Mode A invocation pattern.
  - §11 (Phasing): F1 = 3-4 agent-hours, scope = scaffold + 10-row
    demo + smoke test.
  - §12 (Open questions): questions #1, #2, #6, #8 are already
    settled by F0 (regseq-driver-63). Question #3 (composability)
    is partially settled by shipping `All`/`Any` composites in F1.
    Questions #4, #5, #7 are addressed in OPEN QUESTIONS section
    below.

### Matrix source

- `docs/discussions/regseq-model/bridge-coverage-matrix.md` (v2.1,
  Codex-approved, committed at `4012635`). This is the matrix the
  runner consumes; F1's drift check parses row-id cells from this
  file's tables.
  - Section 1.1 (`Ty` enum) at `bridge-coverage-matrix.md:89-167`
    -- contains TY-018 and TY-014-A demo rows.
  - Section 2.1 (`ItemKind`) at `bridge-coverage-matrix.md:361-371`
    -- contains IK-002 and IK-005 demo rows.
  - Section 2.3 (`FunctionInstanceFlavor`) at
    `bridge-coverage-matrix.md:390-408` -- contains FF-011 demo row.
  - Section 5 (Interaction matrix) at
    `bridge-coverage-matrix.md:474-525` -- contains IX-002 demo row.
  - Section 6.1 (`DslRejection`) at
    `bridge-coverage-matrix.md:537-548` -- contains RJ-002 demo row.
  - Section 6.3 (Top-level rejection variants) at
    `bridge-coverage-matrix.md:561-582` -- contains RJ-016 and
    RJ-026 demo rows.
  - RJ-028 sub-axes table at `bridge-coverage-matrix.md:586-590`
    -- contains the RJ-028-A demo row.

### F0 surface (consumed read-only)

- `crates/test-ir-fixtures/src/lib.rs:45-72` -- `BridgeOutcome` enum
  with four variants (`Ok` / `ValidatorFailed` / `Rejected` /
  `RustcRejected`).
- `crates/test-ir-fixtures/src/lib.rs:156` -- `try_bridge_fixture`
  (named fixture entry, unused by F1 runner).
- `crates/test-ir-fixtures/src/lib.rs:183` -- `try_bridge_fixture_path`
  (path-based entry, the runner's bridge invocation hook). Returns
  `BridgeOutcome`; never panics on bridge failures; sanitizes
  hyphenated stems to rustc-valid crate names per F0 §B2.
- `crates/test-ir-fixtures/src/lib.rs:282` --
  `try_compile_and_bridge_with_crate_name` (internal workhorse).
  Injects `-C panic=abort` at `lib.rs:322`; sets `no_codegen = true`,
  `mir_opt_level = Some(0)`, `always_encode_mir = true`,
  `mir_enable_passes = regseq_mir_passes()`, `overflow_checks =
  Some(false)`, `debug_assertions = false` in rustc config at
  `lib.rs:338-344` (matches the production `RegseqCallbacks` shape
  at `crates/regseq-driver/src/callbacks.rs:103-168`).

### Bridge type imports (for `inspect.rs` dispatch)

The runner's `inspect.rs` module walks a `RegseqModel` value's
internal arrays. Type anchors (all `pub`, re-exported from
`regseq-model`):

- `crates/regseq-model/src/ty.rs:222` -- `Ty` enum (19 variants;
  `TyVariant` predicate dispatch).
- `crates/regseq-model/src/item.rs:113` -- `ItemKind` enum (6+
  variants; `ItemKind` predicate dispatch).
- `crates/regseq-model/src/synthetic.rs:46` -- `SyntheticItemKind`
  enum (10+ variants; deferred).
- `crates/regseq-model/src/instance.rs:192` -- `FunctionInstanceFlavor`
  enum (15 variants; `FunctionInstanceFlavor` predicate dispatch).
- `crates/regseq-model/src/rvalue.rs:117` -- `BinOp` enum (deferred).
- `crates/regseq-driver/src/bridge/mod.rs:376` -- `BridgeError`
  (for `Rejected` outcome classification in `cluster.rs`).

The implementer must confirm each anchor's HEAD line number during
work; line numbers in this spec are recorded against HEAD at task
file time.

### Workspace state

- `Cargo.toml:3-14` -- workspace members. F1 adds
  `crates/bridge-coverage-runner` to the list.
- `crates/test-ir-fixtures/Cargo.toml:10-12` -- the F0 helper's
  dependencies (`regseq-model` + `regseq-driver`). The new runner
  crate adopts the same dependency shape plus `test-ir-fixtures`
  itself.
- `rust-toolchain.toml` -- pinned `nightly-2026-02-01`. The new
  crate inherits the workspace pin; no per-crate toolchain
  override.

### Prior tasks

- regseq-driver-63 (closed at `174c7ff`): the F0 typed-outcome
  helper. F1 consumes the F0 surface and is the canonical first
  downstream caller of `try_bridge_fixture_path`.
- regseq-driver-62 (closed at `3748f47`): chronological dependency
  only.
- Bridge-coverage-runner strategy v3 (committed at `4012635`):
  the F0-F5 plan.

### Test surface

- `crates/test-ir-fixtures/tests/typed_outcome_smoke.rs` -- F0
  smoke. F1 does not modify it; cited as the existence-proof for
  the consumed F0 API.
- New test under `crates/bridge-coverage-runner/tests/smoke.rs`
  per Change K below.

## Implementation Notes

### Change A: workspace registration + crate manifest + ancillary files

**Anchor**: `Cargo.toml:3-14`.

Add `crates/bridge-coverage-runner` to the workspace `members`
array. Maintain alphabetical ordering relative to existing entries
(the existing list at HEAD is already alphabetical):

```toml
members = [
    "crates/bridge-coverage-runner",
    "crates/cargo-regseq",
    ...
]
```

**New file**: `crates/bridge-coverage-runner/Cargo.toml`.

```toml
[package]
name = "bridge-coverage-runner"
version = "0.1.0"
edition = "2021"
publish = false

[package.metadata.rust-analyzer]
rustc_private = true

[[bin]]
name = "bridge-coverage-runner"
path = "src/main.rs"

[dependencies]
test-ir-fixtures = { path = "../test-ir-fixtures" }
regseq-model = { path = "../regseq-model" }
regseq-driver = { path = "../regseq-driver" }
toml = "0.8"
serde = { version = "1", features = ["derive"] }
sha2 = "0.10"
```

Rationale per dep:
- `test-ir-fixtures`: F0 surface (`try_bridge_fixture_path`,
  `BridgeOutcome`).
- `regseq-model`: `RegseqModel`, `Ty`, `ItemKind`, `Instance`,
  `InvariantViolation` for inspection + clustering.
- `regseq-driver`: `BridgeError` for `Rejected` outcome
  classification (re-exported through the F0 `BridgeOutcome::Rejected`
  variant; the runner pattern-matches on it).
- `toml = "0.8"`: sidecar parsing per strategy §4.
- `serde + derive`: `#[derive(Deserialize)]` on sidecar types.
- `sha2 = "0.10"`: snippet-hash prefix per strategy §8
  ("SHA-256 first-8-bytes").

No `clap` or other arg-parsing crate -- the CLI surface (3 flags)
is small enough that hand-rolled parsing in `main.rs` is cleaner
than a dep.

**New file**: `crates/bridge-coverage-runner/.gitignore`.

```
reports/
```

Per strategy §3 ("`reports/` -- generated, gitignored").

**New file**: `crates/bridge-coverage-runner/snippets/README.md`.

One-paragraph row-id naming convention. Anchors against strategy §3
and the matrix's row-id format (`TY-001.rs`, `RV-006-A.rs`,
`OP-003-G-1.rs`).

### Change B: `src/matrix.rs` -- TOML loader + subset-OK drift check

**New file**: `crates/bridge-coverage-runner/src/matrix.rs`.

Public surface:

```rust
//! Matrix metadata sidecar loader per
//! `docs/discussions/regseq-model/bridge-coverage-runner-strategy.md`
//! §4 (sidecar) + §4.1 (drift check).

use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct RowsFile {
    #[serde(rename = "row")]
    pub rows: Vec<Row>,
}

#[derive(Debug, Deserialize)]
pub struct Row {
    pub id: String,
    pub section: String,
    pub construct: String,
    pub expectation: Expectation,
    pub status: String,
    pub cluster_key: String,            // strategy §8: explicit, not derived.
    pub fixture_path: Option<String>,
    pub snippet_path: Option<String>,
    pub inspect: Option<crate::inspect::InspectionPredicate>,
    pub expected_rejection: Option<ExpectedRejection>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
pub enum Expectation {
    Positive,
    Rejected,
    ValidatorFailing,
}

#[derive(Debug, Deserialize)]
pub struct ExpectedRejection {
    pub variant: String,                // e.g., "DslRejection", "Coroutine".
    pub kind: Option<String>,           // sub-kind for DslRejection (e.g., "CVoid").
    pub position: Option<String>,       // sub-position for UnsupportedCAbiType.
}

pub fn load(metadata_path: &Path) -> Result<RowsFile, MatrixError>;

#[derive(Debug)]
pub struct DriftReport {
    pub missing_in_sidecar: Vec<String>, // matrix rows not in sidecar (F1: warn-only).
    pub missing_in_matrix: Vec<String>,  // sidecar rows not in matrix (F1: hard-fail).
}

/// Subset-OK drift check per OPEN QUESTION (a) adjudication.
/// `missing_in_matrix` (sidecar row not in matrix) is a hard fail
/// because the sidecar must never invent row IDs. `missing_in_sidecar`
/// (matrix row not in sidecar) is acceptable in F1 because the
/// sidecar legitimately covers a 10-row subset of the 445-row matrix;
/// returned as informational data, NOT a fail condition.
pub fn check_drift(rows: &RowsFile, matrix_md_path: &Path) -> Result<DriftReport, MatrixError>;
```

Drift-check algorithm:

1. Read `matrix_md_path` as UTF-8.
2. Iterate lines; for each line matching the table-row pattern
   `^\|\s*([A-Z]{2,3}-\d{3}(?:-[A-Z](?:-\d+)?)?)\s*\|`, extract the
   capture as a row id. (The character classes match all observed
   matrix row-ID shapes per `bridge-coverage-matrix.md`: top-level
   `TY-001`, sub-axis `RV-006-A`, nested `OP-003-G-1`.)
3. Build a `BTreeSet<String>` of matrix row ids.
4. Build a `BTreeSet<String>` of sidecar row ids from `rows.rows`.
5. `missing_in_matrix` = sidecar - matrix. NON-EMPTY -> return
   `MatrixError::DriftHardFail { rows: missing_in_matrix }`.
6. `missing_in_sidecar` = matrix - sidecar. Returned as part of
   the report (informational); never a fail.
7. F1 deviation from strategy §4.1: documented as OPEN QUESTION (a)
   adjudication below.

Cite the strategy §4.1 deviation explicitly in a module-level
doc-comment so future readers can find it via grep when the matrix
expands to full coverage and strict-mode becomes feasible.

### Change C: `src/snippet.rs` -- per-snippet load + name<->row mapping

**New file**: `crates/bridge-coverage-runner/src/snippet.rs`.

Public surface:

```rust
use std::path::{Path, PathBuf};

pub struct Snippet {
    pub row_id: String,
    pub path: PathBuf,
    pub hash_prefix: String,    // SHA-256 first 8 hex chars per strategy §8.
}

pub fn load_snippets(snippets_dir: &Path, rows: &[crate::matrix::Row]) -> Result<Vec<Snippet>, SnippetError>;
```

Per-snippet loading:

1. For each row with `expectation == Positive` OR
   `expectation == Rejected`, the row's `snippet_path` MUST be `Some`
   in F1. (Validator-failing rows are out of scope per strategy §2
   "Out of scope (initial landing): Section 7 validator-invariant
   rows".)
2. Resolve `snippet_path` relative to `snippets_dir`. Files must
   exist and be readable.
3. Compute SHA-256 of the file's bytes via `sha2::Sha256`, hex-encode,
   keep first 8 chars as `hash_prefix`.
4. Return a `Vec<Snippet>` parallel to the input row slice.

### Change D: `src/oracle.rs` -- subprocess rustc pre-flight + bridge invocation

**New file**: `crates/bridge-coverage-runner/src/oracle.rs`.

Implements strategy §6.2 (panic capture) + §6.3 (two-step rustc
pre-flight + in-process bridge run).

```rust
use std::path::Path;
use std::process::Command;
use test_ir_fixtures::{try_bridge_fixture_path, BridgeOutcome};
use regseq_model::{InvariantViolation, RegseqModel};
use regseq_driver::bridge::BridgeError;

pub enum OracleOutcome {
    Ok(RegseqModel),
    ValidatorFailed { violation: InvariantViolation },
    Rejected(BridgeError),
    RustcCleanReject { stderr: String },
    RustcICE { stderr: String },
    RustcNoAfterAnalysis,
    Panic { message: String },
}

pub fn invoke(snippet_path: &Path) -> OracleOutcome { ... }
```

Algorithm:

1. **Step 1 (rustc subprocess pre-flight per strategy §6.3)**:
   ```rust
   let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
   let tmp = tempfile::Builder::new()...;  // OR a process-local PathBuf in std::env::temp_dir() with a UUID-ish stem.
   let output = Command::new(rustc)
       .args([
           "--crate-type", "lib",
           "--edition", "2021",
           "--emit=metadata",                // stable; see OPEN QUESTION (c) below.
           "-o", tmp_path_str,
           snippet_path.to_str().expect("snippet path is valid UTF-8"),
       ])
       .output()?;
   ```
   
   If `!output.status.success()`:
   - `stderr = String::from_utf8_lossy(&output.stderr).into_owned();`
   - If `stderr.contains("internal compiler error")` or
     `stderr.contains("thread 'rustc' panicked")`:
     -> `OracleOutcome::RustcICE { stderr }`
   - Else:
     -> `OracleOutcome::RustcCleanReject { stderr }`
   - Return.

2. **Step 2 (in-process bridge run via F0 helper)**:
   Pre-flight succeeded; the snippet is syntactically + semantically
   valid Rust. Invoke `try_bridge_fixture_path` inside
   `std::panic::catch_unwind` per strategy §6.2.
   
   ```rust
   let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
       try_bridge_fixture_path(snippet_path)
   }));
   match result {
       Err(payload) => OracleOutcome::Panic {
           message: panic_message(&payload),
       },
       Ok(BridgeOutcome::Ok(model)) => OracleOutcome::Ok(model),
       Ok(BridgeOutcome::ValidatorFailed { violation, .. }) =>
           OracleOutcome::ValidatorFailed { violation },
       Ok(BridgeOutcome::Rejected(err)) => OracleOutcome::Rejected(err),
       Ok(BridgeOutcome::RustcRejected { .. }) =>
           OracleOutcome::RustcNoAfterAnalysis,
   }
   ```
   
   `panic_message` extracts the `&str` / `String` from
   `Box<dyn Any + Send>` via `downcast_ref::<&'static str>` /
   `downcast_ref::<String>`; falls back to `"<unknown panic payload>"`
   if neither matches. Standard panic-payload extraction shape; no
   library dependency.

3. **No `tempfile` crate dep**: use `std::env::temp_dir()` + a
   deterministic-stem based on the row id (e.g.,
   `bridge_coverage_runner_preflight_<row_id>.rmeta`). The file
   path is short-lived; the runner cleans it up in a `defer`-style
   shape after the subprocess completes. Cleanup must run even on
   error -- wrap in a `scopeguard`-style RAII or an explicit
   `let _ = fs::remove_file(&tmp_path);` after both branches.
   The compiler-lock at
   `crates/test-ir-fixtures/src/lib.rs:289-291` serialises bridge
   invocations -- the runner is single-process per strategy §10
   Mode A, so a single shared tmp dir is fine.

`tempfile` is NOT added as a dep; the simple PathBuf shape avoids
the extra crate.

`AssertUnwindSafe` is required because `try_bridge_fixture_path`
holds an `Arc<Mutex<...>>` internally per
`crates/test-ir-fixtures/src/lib.rs:325` (storage) +
`crates/test-ir-fixtures/src/lib.rs:328` (Callbacks struct field);
the `Mutex` is not `UnwindSafe` by default but the runner's use
is single-threaded and the panic-capture boundary is the runner's
job per strategy §6.2.

### Change E: `src/inspect.rs` -- InspectionPredicate enum + demo-set dispatch

**New file**: `crates/bridge-coverage-runner/src/inspect.rs`.

The enum ships **complete** (all 23 variants per strategy §7) so
the sidecar can be authored against the full predicate vocabulary.
Dispatch arms are implemented ONLY for the demo set's predicates
in F1 (see OPEN QUESTION (b) adjudication); remaining variants
return `InspectError::Unimplemented { variant }`.

**Strategy deviation note (Implementation-level only)**: strategy
§7 presents `All`/`Any` as struct variants with a `predicates`
field (`All { predicates: Vec<InspectionPredicate> }`). F1 uses
newtype variants (`All(Vec<InspectionPredicate>)`) so the
adjacently-tagged serde representation produces a flat
`{ kind = "All", arg = [...] }` TOML shape instead of the
nested `{ kind = "All", arg = { predicates = [...] } }` shape
struct variants force. The semantic content (all-of vs any-of
composition) is identical; the strategy's normative claim is the
composite SEMANTICS, not the Rust field name. Documented here so
future readers can grep for the deviation.

```rust
use serde::Deserialize;
use regseq_model::RegseqModel;

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", content = "arg", rename_all = "PascalCase")]
pub enum InspectionPredicate {
    TyVariant(String),
    ItemKind(String),
    SyntheticItemKind(String),
    FunctionInstanceFlavor(String),
    StatementKind(String),
    TerminatorKind(String),
    RvalueVariant(String),
    PlaceElem(String),
    ConstVariant(String),
    AggregateKind(String),
    CastKind(String),
    BinOp(String),
    UnOp(String),
    AssertKind(String),
    NonDivergingIntrinsic(String),
    CallCallee(String),
    CallDivergent(bool),
    BodyAbsent,
    Linkage(String),
    ScalarKind(String),
    All(Vec<InspectionPredicate>),
    Any(Vec<InspectionPredicate>),
}

#[derive(Debug)]
pub enum InspectError {
    Unimplemented { variant: &'static str },
    NotFound { variant: &'static str, arg: String },
}

pub fn check(model: &RegseqModel, pred: &InspectionPredicate) -> Result<(), InspectError>;
```

**F1 dispatch arms** (the demo set's predicates):

| variant                  | algorithm                                                                                    |
| ------------------------ | -------------------------------------------------------------------------------------------- |
| `TyVariant(arg)`         | Walk `model.types`; succeed if any entry's `Ty` variant tag matches `arg`.                   |
| `ItemKind(arg)`          | Walk `model.items`; succeed if any entry's `ItemKind` variant tag matches `arg`.             |
| `FunctionInstanceFlavor(arg)` | Walk `model.instances`; for each Function Instance check `flavor` variant tag matches.  |
| `All(predicates)`        | Every nested predicate succeeds (recursive `check` call).                                    |
| `Any(predicates)`        | At least one nested predicate succeeds (recursive `check` call).                             |

All other 18 leaf variants:
```rust
other => Err(InspectError::Unimplemented {
    variant: variant_name(other),
}),
```

where `variant_name` is a `match` returning a `&'static str` for
each variant (`"StatementKind"`, `"TerminatorKind"`, etc.).

The runner SURFACES `InspectError::Unimplemented` per-row in the
report but does NOT treat it as a failure of the runner itself;
it's a per-row "F3 follow-up needed" annotation.

Cross-reference in module doc-comments: every `Unimplemented`
return cites strategy §11 phase F3 ("Inspection-predicate dispatch
hardening + failure clustering polish"). When F3 lands, the
unimplemented arms get filled in.

### Change F: `src/cluster.rs` -- fingerprint + grouping

**New file**: `crates/bridge-coverage-runner/src/cluster.rs`.

Implements strategy §8 fingerprint tuple + simple-shape clustering.

```rust
pub struct Fingerprint {
    pub cluster_key: String,
    pub expectation_class: crate::matrix::Expectation,
    pub actual_outcome: ActualOutcomeTag,
    pub variant_or_phase: String,
    pub snippet_hash: String,
}

pub enum ActualOutcomeTag {
    Ok,
    Rejected(String),           // BridgeError variant name
    ValidatorFailed(String),    // InvariantViolation variant name
    RustcCleanReject,
    RustcICE,
    RustcNoAfterAnalysis,
    Panic(String),              // top non-std/rustc frame "file:line" or "<unknown>"
}

pub struct Cluster {
    pub fingerprint_hash: u64,
    pub members: Vec<ClusterMember>,
}

pub struct ClusterMember {
    pub row_id: String,
    pub snippet_path: PathBuf,
    pub one_line_detail: String,
}

pub fn group(results: &[RowResult]) -> Vec<Cluster>;
```

Hash via `std::collections::hash_map::DefaultHasher` on the
`Fingerprint` tuple (no `sha2` here -- the cluster grouping doesn't
need a cryptographic hash). Members sorted by snippet length
(shortest first) so the cluster's first member is the representative
repro per strategy §8 ("shortest snippet in the cluster").

**F1 simplicity**: per strategy §8 final paragraph, F1 ships
"INTENTIONALLY simple" clustering; no similarity scoring, no
minimal-repro selection beyond shortest-snippet. Hash the tuple,
group, sort descending by cluster size.

**Panic top-frame extraction**: best-effort. For F1, simply use
the panic message string from `OracleOutcome::Panic` and truncate
to the first 80 chars; F3 can refine with backtrace parsing.

### Change G: `src/report.rs` -- markdown report emitter

**New file**: `crates/bridge-coverage-runner/src/report.rs`.

Per strategy §9: markdown only in F1. Shape:

```markdown
# Bridge Coverage Runner Report

Generated: <ISO-Z timestamp>
Sidecar: <metadata path>
Snippets dir: <snippets path>

## Summary

| Class | Count |
| ----- | ----- |
| Total rows | N |
| Pass | X |
| Fail | Y |
| Outcomes by class | Ok=A, Rejected=B, RustcCleanReject=C, ... |

Drift report:
- Sidecar rows missing from matrix: <list or "none">
- Matrix rows missing from sidecar: <count> (subset-OK in F1; see strategy §4.1)

## Failure clusters

(Sorted by cluster size descending.)

### Cluster <fingerprint_hash_hex_8>

Fingerprint:
- cluster_key: <key>
- expectation_class: <class>
- actual_outcome: <tag>
- variant_or_phase: <variant>

Members (<n>):

- `<row_id>` -- `<snippet_path>` -- <one_line_detail>
- ...

Representative repro (<shortest_member>.rs):

```rust
<snippet content, fenced>
```
```

Per-row pass/fail decision algorithm:

| expectation | actual                                | pass/fail                                 |
| ----------- | ------------------------------------- | ----------------------------------------- |
| Positive    | `Ok(_)` + inspection passes           | PASS                                      |
| Positive    | `Ok(_)` + inspection fails / unimpl   | FAIL (inspection result recorded)         |
| Positive    | anything else                         | FAIL                                      |
| Rejected    | `Rejected(err)` matches expected      | PASS                                      |
| Rejected    | `Rejected(err)` mismatched variant    | FAIL                                      |
| Rejected    | anything else                         | FAIL                                      |

**`Inspection::Unimplemented` -> FAIL (no pass-with-caveat
shortcut).** Per strategy §7 the inspection step is the
ANTI-SILENT-PASS check: a positive row that bridges OK without
predicate verification cannot count as covered, because the bridge
might be returning a model that doesn't actually carry the claimed
construct. F1's 10-row demo set deliberately uses only the 5
implemented predicates (TyVariant / ItemKind /
FunctionInstanceFlavor / All / Any); an F2+ row that uses an
unimplemented predicate will FAIL until F3 implements that
variant's dispatch arm, signalling the F3 gap visibly in the
report rather than silently passing.

### Change H: `src/main.rs` -- CLI + run loop

**New file**: `crates/bridge-coverage-runner/src/main.rs`.

Mode A CLI per strategy §10:

```bash
bridge-coverage-runner \
  --metadata crates/bridge-coverage-runner/matrix_metadata/rows.toml \
  --snippets crates/bridge-coverage-runner/snippets/ \
  --matrix-md docs/discussions/regseq-model/bridge-coverage-matrix.md \
  --out reports/<timestamp>/
```

Arg parsing is hand-rolled (no `clap` dep). Required flags:

- `--metadata <path>`: rows.toml location.
- `--snippets <dir>`: snippets directory.
- `--matrix-md <path>`: matrix markdown for drift check (added to
  the strategy §10 invocation; strategy implicitly assumes a
  conventional location, F1 makes it explicit).
- `--out <dir>`: output directory for `report.md`. Created if absent.

Run loop:

```rust
fn main() -> ExitCode {
    let args = match parse_args(std::env::args()) { ... };
    
    let rows_file = matrix::load(&args.metadata)?;
    let drift = matrix::check_drift(&rows_file, &args.matrix_md)?;
    // drift.missing_in_matrix is a hard fail; matrix::check_drift returns Err in that case.
    // drift.missing_in_sidecar is informational.

    let snippets = snippet::load_snippets(&args.snippets, &rows_file.rows)?;

    let mut results: Vec<RowResult> = Vec::with_capacity(rows_file.rows.len());
    for (row, snippet) in rows_file.rows.iter().zip(snippets.iter()) {
        let outcome = oracle::invoke(&snippet.path);
        let inspection = run_inspection(row, &outcome);
        results.push(RowResult { row, snippet, outcome, inspection });
    }

    let clusters = cluster::group(&results);
    let report = report::render(&results, &clusters, &drift);
    fs::create_dir_all(&args.out)?;
    fs::write(args.out.join("report.md"), report)?;
    
    let exit_code = if results.iter().any(|r| r.is_fail()) { 1 } else { 0 };
    ExitCode::from(exit_code)
}
```

Where `run_inspection`:
- `expectation == Positive` + `outcome == Ok(model)` +
  `row.inspect.is_some()`: invoke `inspect::check(model, pred)`.
- `expectation == Positive` + `outcome != Ok(_)`: `None`
  (inspection isn't applicable -- the outcome itself is the fail
  signal).
- `expectation == Rejected`: `None` (inspection is positive-only).

The runner exits 0 if all rows pass per the Change G table; exits
1 otherwise. The smoke test in Change K verifies both exit codes.

### Change I: `matrix_metadata/rows.toml` -- 10 demo rows

**New file**:
`crates/bridge-coverage-runner/matrix_metadata/rows.toml`.

```toml
# 10-row demo set for F1 validate-the-loop milestone per
# bridge-coverage-runner-strategy.md §11. Composed to exercise:
#   - Both expectation classes (positive: 6, rejected: 4)
#   - Five InspectionPredicate dispatch arms (TyVariant / ItemKind /
#     FunctionInstanceFlavor / All / Any composite)
#   - Four BridgeError variants (DslRejection / Coroutine /
#     UnsupportedAbi / UnsupportedCAbiType)
#   - The subprocess rustc pre-flight path (no row deliberately
#     fails rustc in F1; pre-flight code path is verified via the
#     positive flow not surfacing RustcCleanReject)
#
# F2+ extends this sidecar to the full ~133-uncovered-row matrix
# corpus.

[[row]]
id = "TY-018"
section = "1.1"
construct = "Ty::Enum"
expectation = "positive"
status = "covered"
cluster_key = "TY-018"
snippet_path = "TY-018.rs"
inspect = { kind = "TyVariant", arg = "Enum" }

[[row]]
id = "IK-002"
section = "2.1"
construct = "ItemKind::Static"
expectation = "positive"
status = "covered"
cluster_key = "IK-002"
snippet_path = "IK-002.rs"
inspect = { kind = "ItemKind", arg = "Static" }

[[row]]
id = "IK-005"
section = "2.1"
construct = "ItemKind::Struct"
expectation = "positive"
status = "covered"
cluster_key = "IK-005"
snippet_path = "IK-005.rs"
inspect = { kind = "ItemKind", arg = "Struct" }

[[row]]
id = "TY-014-A"
section = "1.1"
construct = "Ty::Dynamic (no auto-traits)"
expectation = "positive"
status = "covered"
cluster_key = "TY-014"
snippet_path = "TY-014-A.rs"
# Composite Any: exercises the runner's `Any` dispatch arm. The
# snippet's model carries Ty::Dynamic (matches first leaf); the
# Ty::Never leaf is intentionally unmatched so Any's "at least one"
# semantics are exercised non-trivially. The arg-array shape
# follows from the newtype-variant + adjacent-tag encoding per
# Change E (Strategy deviation note).
inspect = { kind = "Any", arg = [
  { kind = "TyVariant", arg = "Dynamic" },
  { kind = "TyVariant", arg = "Never" },
] }

[[row]]
id = "FF-011"
section = "2.3"
construct = "FunctionInstanceFlavor::DropGlue"
expectation = "positive"
status = "covered"
cluster_key = "FF-011"
snippet_path = "FF-011.rs"
inspect = { kind = "FunctionInstanceFlavor", arg = "DropGlue" }

[[row]]
id = "IX-002"
section = "5"
construct = "Trait default body x body materialisation"
expectation = "positive"
status = "covered"
cluster_key = "IX-002"
snippet_path = "IX-002.rs"
# Composite predicate: the model must carry BOTH a Struct (the impl's
# concrete receiver) and a Function ItemKind (the trait's default body).
# Arg-array shape per Change E (newtype-variant encoding).
inspect = { kind = "All", arg = [
  { kind = "TyVariant", arg = "Struct" },
  { kind = "ItemKind", arg = "Function" },
] }

[[row]]
id = "RJ-002"
section = "6.1"
construct = "DslRejectionKind::CVoid"
expectation = "rejected"
status = "uncovered"
cluster_key = "RJ-002"
snippet_path = "RJ-002.rs"
expected_rejection = { variant = "DslRejection", kind = "CVoid" }

[[row]]
id = "RJ-016"
section = "6.3"
construct = "Coroutine"
expectation = "rejected"
status = "uncovered"
cluster_key = "RJ-016"
snippet_path = "RJ-016.rs"
expected_rejection = { variant = "Coroutine" }

[[row]]
id = "RJ-026"
section = "6.3"
construct = "UnsupportedAbi"
expectation = "rejected"
status = "uncovered"
cluster_key = "RJ-026"
snippet_path = "RJ-026.rs"
expected_rejection = { variant = "UnsupportedAbi" }

[[row]]
id = "RJ-028-A"
section = "6.3"
construct = "UnsupportedCAbiType::Input"
expectation = "rejected"
status = "uncovered"
cluster_key = "RJ-028"
snippet_path = "RJ-028-A.rs"
expected_rejection = { variant = "UnsupportedCAbiType", position = "Input" }
```

### Change J: 10 hand-coded `#![no_std]` snippets

**New files** under `crates/bridge-coverage-runner/snippets/`:

`TY-018.rs`:
```rust
#![no_std]
pub enum E { A, B }
pub fn f() -> E { E::A }
```

`IK-002.rs`:
```rust
#![no_std]
pub static COUNTER: u32 = 0;
```

`IK-005.rs`:
```rust
#![no_std]
pub struct S;
pub fn f() -> S { S }
```

`TY-014-A.rs`:
```rust
#![no_std]
pub trait R {
    fn r(&self) -> u32;
}
pub struct P;
impl R for P {
    fn r(&self) -> u32 { 0 }
}
pub fn f(p: &P) -> u32 {
    let d: &dyn R = p;
    d.r()
}
```

`FF-011.rs`:
```rust
#![no_std]
pub struct R;
impl Drop for R {
    fn drop(&mut self) {}
}
pub fn f() {
    let _r = R;
}
```

`IX-002.rs`:
```rust
#![no_std]
pub trait G {
    fn greet(&self) -> u32 { 0 }
}
pub struct S;
impl G for S {}
pub fn f(s: &S) -> u32 {
    s.greet()
}
```

`RJ-002.rs`:
```rust
#![no_std]
pub extern "C" fn f(_p: *const core::ffi::c_void) {}
```

`RJ-016.rs`:
```rust
#![no_std]
#![feature(coroutines, coroutine_trait, stmt_expr_attributes)]
pub fn coroutine_holder() {
    let _c = #[coroutine] || {
        yield 1u32;
    };
}
```

Rationale: a bare `pub async fn f() {}` does NOT empirically trigger
`BridgeError::Coroutine` through `run_bridge`; the bridge only emits
that variant when a coroutine-family `TyKind` reaches type translation
per `crates/regseq-driver/src/bridge/ty.rs:277-280`. The
`#[coroutine] ||` closure-literal shape (mirroring the working
`coroutine_holder` test source at
`crates/regseq-driver/tests/bridge_ty.rs:472-476`) creates a local
whose type IS a coroutine; the body materialiser walks the local
declaration during translation and surfaces the rejection.

`RJ-026.rs`:
```rust
#![no_std]
pub extern "system" fn f() {}
```

`RJ-028-A.rs`:
```rust
#![no_std]
pub extern "C" fn f(_t: (u32, u32)) {}
```

**Implementer verification step** (NOT a spec ambiguity but a
test-time check): each rejected snippet MUST surface the
`BridgeError` variant claimed by its `expected_rejection`. If a
snippet trips a different variant (e.g., RJ-016 fails through
`NonRigidTy::Alias` instead of `Coroutine` because of how rustc
handles empty `async fn` desugaring), update the snippet's source
shape OR the `expected_rejection` field to match the empirical
outcome -- and record the deviation in the harness expectations
file. F1 prioritises a working loop over matching the strategy's
predicted rejection shapes.

### Change K: `tests/smoke.rs` -- 3-row mini-corpus end-to-end

**New file**: `crates/bridge-coverage-runner/tests/smoke.rs`.

Per strategy §3 (`tests/smoke.rs -- runner runs end-to-end on a
3-row mini-corpus`).

Approach: invoke the runner binary as a subprocess via
`env!("CARGO_BIN_EXE_bridge-coverage-runner")` (cargo populates
this env var for integration tests; no `assert_cmd` dep needed).
Use a `tempdir`-style approach for the per-test sidecar +
snippets.

```rust
use std::path::PathBuf;
use std::process::Command;

#[test]
fn smoke_3_positive_rows() {
    // Build a 3-row mini-sidecar in a tempdir (TY-018, IK-002, RV-006-A
    // -- wait, we replaced RV-006-A; pick three positive rows that
    // don't require composite predicates).
    let tmp = make_tempdir("brcr_smoke_pos");
    write_mini_sidecar(&tmp, &["TY-018", "IK-002", "IK-005"]);
    copy_snippets(&tmp, &["TY-018", "IK-002", "IK-005"]);

    let runner = env!("CARGO_BIN_EXE_bridge-coverage-runner");
    let output = Command::new(runner)
        .args([
            "--metadata", &tmp.join("rows.toml").to_string_lossy(),
            "--snippets", &tmp.join("snippets").to_string_lossy(),
            "--matrix-md", project_root_join("docs/discussions/regseq-model/bridge-coverage-matrix.md").to_str().unwrap(),
            "--out", &tmp.join("report").to_string_lossy(),
        ])
        .output()
        .expect("runner subprocess spawn failed");

    assert!(output.status.success(),
            "runner exited non-zero: stdout={:?} stderr={:?}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr));

    let report = std::fs::read_to_string(tmp.join("report").join("report.md"))
        .expect("report.md exists");
    assert!(report.contains("# Bridge Coverage Runner Report"));
    assert!(report.contains("TY-018"));
    assert!(report.contains("IK-002"));
    assert!(report.contains("IK-005"));
}

#[test]
fn smoke_drift_hard_fail_on_invented_row_id() {
    // Mini-sidecar contains a fake row id ("ZZ-999") not present in matrix.md.
    // Runner exits non-zero with a drift-hard-fail message.
    ...
}

#[test]
fn smoke_full_demo_set_runs_to_completion() {
    // Mini-sidecar is the full 10-row F1 demo set.
    // Runner exits 0 (all rows pass per their expectations).
    // Report contains every row id + a "Summary" section.
    ...
}
```

`project_root_join` walks up from `CARGO_MANIFEST_DIR` to the
workspace root; standard pattern in existing integration tests.

The third test (`smoke_full_demo_set_runs_to_completion`) is the
load-bearing end-to-end validation: it asserts the runner produces
a coherent report for all 10 demo rows with exit code 0. If any
demo row's expected outcome doesn't match empirical bridge behavior,
this test fails and the implementer updates the snippet or
`expected_rejection` per the Change J verification step.

The mini-sidecar tests inject their own row sets; they do NOT use
the committed `matrix_metadata/rows.toml`. This isolates the smoke
tests from any future demo-set expansion.

### Change L: harness expectations entry

**New file**: `.harness/expectations/regseq-driver-64.md`.

Record:

- Before-fix evidence: bridge-coverage-runner does not exist;
  matrix-coverage authoring requires per-row task-spec drafting.
- After-fix evidence: `crates/bridge-coverage-runner/` lands with
  the seven modules per strategy §3; `cargo run -p
  bridge-coverage-runner -- --metadata ... --snippets ... --out
  ...` produces a `report.md` against the 10-row demo set with exit
  code 0.
- Demo-set verification: for each of the 10 rows, the empirical
  outcome matches the row's expectation (any deviation from the
  spec'd `expected_rejection` shape is recorded with the actual
  variant + a hypothesis about why).
- F2 readiness signal: the runner's API + report shape is
  operationally sufficient for the F2 LLM authoring loop per
  strategy §11. F2's input is the runner's report; F1 has shipped
  a report shape F2 can consume.
- Open-question adjudication outcomes (the three this task's Codex
  review surfaced -- record the agreed-on positions for future
  reference).

## Non-Goals

- Do NOT add a `src/lib.rs` to the runner crate. Strategy §3
  explicitly defers public library API for in-process agent
  consumption to F4+ (Mode B).
- Do NOT implement LLM-driven snippet authoring. F1 hand-codes the
  10 demo snippets; LLM authoring is F2/F4 work per strategy §5.
- Do NOT implement the remaining 18 `InspectionPredicate` dispatch
  arms beyond the demo set's 5 arms (TyVariant / ItemKind /
  FunctionInstanceFlavor / All / Any). F3 hardens dispatch per
  strategy §11.
- Do NOT implement JSON report output. Strategy §9 defers JSON
  output until automation consumers exist.
- Do NOT implement CI gating or Mode C nightly run. Strategy §10
  defers Mode C until corpus stabilises.
- Do NOT implement Mode B agent integration (auto-decomposing runner
  failures into `regseq-driver-NN` task proposals). Strategy §10
  defers Mode B to F5+.
- Do NOT modify the F0 surface at
  `crates/test-ir-fixtures/src/lib.rs`. F0 is read-only consumed
  by F1.
- Do NOT modify `bridge-coverage-runner-strategy.md` or
  `bridge-coverage-matrix.md`. F1 implements against the
  Codex-approved v3 + v2.1 frozen shapes.
- Do NOT modify spec.md, pretty-printer.md, generic-c-backend.md,
  or any model-validator. F1 is runner-only.
- Do NOT add any new dependency beyond `toml`, `serde + derive`,
  and `sha2`. No `clap`, no `tempfile`, no `assert_cmd`, no
  `regex` crate. Rationale: F1 is intentionally minimal; the small
  surface area keeps the F1 review tractable.
- Do NOT use `regex` for the drift-check row-ID parse. Hand-roll
  the character-class check or use a `str::strip_prefix` / split
  approach. Adding `regex` for a 10-character pattern is overweight.
- Do NOT scaffold non-runner crates or modify other workspace
  crates beyond adding the new member to `Cargo.toml`.
- Do NOT expand the 10-row demo set as part of F1. Any expansion
  to F2's 30 / F4's 100 rows is its own task; expanding the set
  here would creep F1's scope into F2's.

## Test Plan

### B1: workspace stays green

`cargo test --workspace -- --test-threads=1` green:

- All existing tests under `crates/regseq-model/tests/`,
  `crates/regseq-driver/tests/`, `crates/test-ir-fixtures/tests/`,
  and other workspace crates' tests pass unchanged.
- F0's `crates/test-ir-fixtures/tests/typed_outcome_smoke.rs`
  passes unchanged.
- The new `crates/bridge-coverage-runner/tests/smoke.rs` (Change K)
  passes -- all three test functions return green.

The runner crate's build participates in workspace builds
(`cargo build --workspace` covers it); F1 verifies via `cargo check
-p bridge-coverage-runner` clean.

### B2: smoke tests detail (Change K)

Three tests in `crates/bridge-coverage-runner/tests/smoke.rs`:

1. **`smoke_3_positive_rows`**: 3-row mini-corpus (TY-018, IK-002,
   IK-005) -> runner exits 0, report.md contains a header + each
   row id + a "Summary" section.
2. **`smoke_drift_hard_fail_on_invented_row_id`**: sidecar carries
   `ZZ-999`, a row id absent from the matrix -> runner exits
   non-zero with a drift-hard-fail in stderr.
3. **`smoke_full_demo_set_runs_to_completion`**: the full 10-row
   F1 demo set -> runner exits 0; report carries entries for all
   10 rows.

### B3: manual end-to-end Mode A invocation

Implementer runs the runner against the committed demo set:

```bash
cd <workspace-root>
cargo build -p bridge-coverage-runner
mkdir -p reports/$(date +%Y%m%d-%H%M%S)
target/debug/bridge-coverage-runner \
  --metadata crates/bridge-coverage-runner/matrix_metadata/rows.toml \
  --snippets crates/bridge-coverage-runner/snippets/ \
  --matrix-md docs/discussions/regseq-model/bridge-coverage-matrix.md \
  --out reports/<timestamp>/
```

Asserts:
- Exit code 0.
- `reports/<timestamp>/report.md` exists.
- Report shows 10 total rows, 10 pass, 0 fail (modulo possible
  Change-J verification adjustments).
- Each demo row appears in the per-row summary.

The `reports/<timestamp>/report.md` is captured (not committed)
and pasted into the harness expectations file as evidence.

### B4: `cargo clippy` + `cargo fmt` clean

`cargo fmt --all -- --check` and
`cargo clippy --workspace --all-targets -- -D warnings` both pass.
The new code's clippy posture: standard `-D warnings` discipline
per workspace convention. Any `#[allow(...)]` annotations must
carry a one-line justifying comment.

### B5: F0 surface unchanged

`git diff main -- crates/test-ir-fixtures/` returns zero. F0's
typed-outcome helper is a read-only dependency; F1 must not modify
it. Any F0 modification surfacing as part of F1 indicates scope
creep.

## Review

### Files to send to Codex (`scripts/python/codex_runner.py`)

- This file
  (`.harness/tasks/regseq-driver/64-bridge-coverage-runner-f1-scaffold.md`).
- The runner strategy doc
  (`docs/discussions/regseq-model/bridge-coverage-runner-strategy.md`)
  as design source-of-truth.
- The matrix doc
  (`docs/discussions/regseq-model/bridge-coverage-matrix.md`) as
  the row-id authority for the drift check.

### Reviewer focus

- **Architecture fidelity**: do the 7 modules in Change B-H match
  strategy §3 exactly? Module names, responsibilities, public
  surfaces.
- **F0 consumption correctness**: does Change D correctly invoke
  `try_bridge_fixture_path` and pattern-match on `BridgeOutcome`?
  Cross-reference against
  `crates/test-ir-fixtures/src/lib.rs:45-189`.
- **rustc pre-flight API**: is `--emit=metadata --crate-type lib
  --edition 2021` the right invocation shape for pinned nightly
  (2026-02-01) per strategy §6.3? SEE OPEN QUESTION (c) BELOW.
- **F1-scope discipline**: does Implementation Notes stay within
  the F1 boundary per strategy §11, or does it creep into F2-F5
  scope?
- **Demo-set composition**: do the 10 rows exercise every
  component of the loop (oracle dispatch, predicate dispatch,
  cluster fingerprint, report shape)?
- **OPEN QUESTION ADJUDICATIONS**: this task asks Codex to weigh
  in on three open design decisions that the v3 strategy doc
  doesn't fully pin down. Each has the spec's current position and
  a rationale; Codex either AGREES or proposes an alternative with
  rationale.
  
  **(a) Drift-check granularity**: strategy §4.1 says "refuse to
  run if any row ID is in one but not the other" (strict). This
  task adopts a **subset-OK** variant for F1: sidecar rows MUST
  exist in matrix (hard fail); matrix rows MAY exceed sidecar (the
  10-row sidecar legitimately covers a subset of the 445-row
  matrix). The strategy's strict shape becomes feasible only when
  the sidecar reaches full coverage in F4. Codex: AGREE or propose
  alternative.
  
  **(b) `InspectionPredicate` dispatch completeness in F1**:
  strategy §7 defines a 23-variant enum and says new variants
  require enum + dispatch-arm additions. F1 ships the complete
  enum but implements dispatch arms for only the 5 demo-set
  predicates (TyVariant / ItemKind / FunctionInstanceFlavor / All
  / Any); the other 18 variants return
  `InspectError::Unimplemented`. F3 (strategy §11) is the
  scheduled hardening phase. Codex: AGREE or push for full
  dispatch in F1.
  
  **(c) rustc pre-flight invocation flags**: strategy §6.3 names
  `--crate-type lib --edition 2021 -Zno-codegen` for the
  subprocess pre-flight. `-Zno-codegen` is unstable and the
  pinned nightly's exact form may have rotted. This task spec's
  Change D uses `--emit=metadata --crate-type lib --edition 2021
  -o <tmpfile.rmeta>` instead -- `--emit=metadata` is stable, gives
  metadata-only without codegen, and is a documented rustc flag
  on every supported channel. Codex: AGREE or propose alternative
  (and confirm against the pinned 2026-02-01 nightly's rustc
  flag set if possible).

- **Out-of-scope discipline**: Non-Goals enumerates 13 deferred
  items. Confirm Implementation Notes does not creep into any.
- **Demo-set predictive accuracy**: each rejected snippet's
  expected `BridgeError` variant is a hypothesis. Codex flags any
  obvious mismatch (e.g., a snippet that triggers `NonRigidTy`
  rather than the claimed variant) with rationale.
- **Citation drift**: every file:line citation in this spec
  anchors against HEAD. Codex verifies anchor lines against the
  committed state at task file time.

### Adversarial prompt template

```
<task>
Review .harness/tasks/regseq-driver/64-bridge-coverage-runner-f1-scaffold.md.
Identify:
- Architecture fidelity to strategy §3 (7-module decomposition).
- F0 consumption correctness against test-ir-fixtures HEAD.
- F1-scope discipline against strategy §11 phasing.
- Demo-set composition (10 rows exercise every loop component).
- Demo-set predictive accuracy (each rejected snippet maps to its
  claimed BridgeError variant).
- Out-of-scope creep against the 13 Non-Goals.
- Citation drift against HEAD.

Adjudicate three open design decisions this task surfaces:
- (a) Drift-check granularity: subset-OK for F1 vs strict per strategy §4.1.
- (b) InspectionPredicate dispatch completeness in F1: demo-only
  with Unimplemented stubs vs all 23 arms.
- (c) rustc pre-flight flags: --emit=metadata vs strategy §6.3's
  -Zno-codegen.
</task>

<grounding_rules>
- Cite file:line for every claim.
- Mark hypotheses as hypotheses.
- Don't speculate about code or spec sections you didn't read.
- For (c), if you don't have direct knowledge of the pinned
  2026-02-01 nightly's rustc flag set, mark the answer as a
  reasoned recommendation rather than a verified claim.
</grounding_rules>

<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
ARCHITECTURE-FIDELITY CHECK: PASS / FAIL with citation
F0-CONSUMPTION CHECK: PASS / FAIL with citation
F1-SCOPE-DISCIPLINE CHECK: PASS / FAIL with citation
DEMO-SET COMPOSITION CHECK: PASS / FAIL
DEMO-SET PREDICTIVE ACCURACY: per-row pass / hypothesis-flagged list
OUT-OF-SCOPE DISCIPLINE: PASS / FAIL
CITATION DRIFT: list or "none"

OPEN QUESTION ADJUDICATIONS:
  (a) Drift-check granularity: AGREE_SUBSET_OK | DISAGREE_<rationale>
  (b) InspectionPredicate F1 completeness: AGREE_DEMO_ONLY | DISAGREE_<rationale>
  (c) rustc pre-flight flags: AGREE_EMIT_METADATA | DISAGREE_<alternative>

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
  - All existing workspace tests pass unchanged.
  - The three new smoke tests in
    `crates/bridge-coverage-runner/tests/smoke.rs` pass.
- `cargo build -p bridge-coverage-runner` clean.
- Manual Mode A invocation per B3 produces a `report.md` with
  exit code 0 and all 10 demo rows covered.
- `git diff --stat HEAD..` shows changes confined to:
  - `Cargo.toml` (workspace registration only)
  - new `crates/bridge-coverage-runner/` tree:
    - `Cargo.toml`, `.gitignore`
    - `src/main.rs`, `src/matrix.rs`, `src/snippet.rs`,
      `src/oracle.rs`, `src/inspect.rs`, `src/cluster.rs`,
      `src/report.rs`
    - `matrix_metadata/rows.toml`
    - `snippets/README.md`, `snippets/<row_id>.rs` (x10)
    - `tests/smoke.rs`
  - new `.harness/expectations/regseq-driver-64.md`
  - harness `.harness/tasks.json` registration entry
- No F0 modifications (`git diff main -- crates/test-ir-fixtures/`
  returns zero).
- No bridge / model / spec / pretty-printer / generic-c-backend /
  matrix / strategy doc changes.
- Codex Task Review verdict: APPROVED or APPROVED_WITH_MINOR_FIXES.

## Acceptance Criteria

1. **Workspace registration lands**: `Cargo.toml`'s `members` array
   contains `crates/bridge-coverage-runner`.

2. **All seven source modules ship**: `crates/bridge-coverage-runner/src/`
   contains `main.rs`, `matrix.rs`, `snippet.rs`, `oracle.rs`,
   `inspect.rs`, `cluster.rs`, `report.rs` -- the seven names
   strategy §3 mandates. No `lib.rs`.

3. **10-row demo sidecar lands**:
   `crates/bridge-coverage-runner/matrix_metadata/rows.toml` carries
   the 10 demo rows from Change I with explicit `cluster_key` per
   row.

4. **10 hand-coded snippets land**: `crates/bridge-coverage-runner/snippets/`
   contains TY-018.rs, IK-002.rs, IK-005.rs, TY-014-A.rs, FF-011.rs,
   IX-002.rs, RJ-002.rs, RJ-016.rs, RJ-026.rs, RJ-028-A.rs.

5. **F0 consumption works end-to-end**: the runner invokes
   `try_bridge_fixture_path` per Change D; smoke tests demonstrate
   the full path TY-018-style snippet -> rustc pre-flight pass ->
   bridge `Ok` -> inspection pass -> report PASS.

6. **Drift check fires on invented row IDs**: smoke test
   `smoke_drift_hard_fail_on_invented_row_id` per Change K
   demonstrates the hard-fail path.

7. **Full demo set passes**: manual Mode A invocation per B3
   produces exit code 0 with all 10 rows passing their expectations.
   Any per-row deviation (e.g., a rejected snippet triggering a
   different `BridgeError` variant than spec'd) is recorded in the
   expectations file with hypothesis + corrective action.

8. **InspectionPredicate enum is complete; dispatch is partial**:
   `src/inspect.rs` defines all 23 variants per strategy §7;
   dispatch arms exist for `TyVariant`, `ItemKind`,
   `FunctionInstanceFlavor`, `All`, `Any`; the other 18 variants
   return `InspectError::Unimplemented` with a `&'static str`
   variant tag.

9. **Smoke test passes**: `cargo test -p bridge-coverage-runner`
   green; specifically the three test functions in Change K's
   `tests/smoke.rs`.

10. **No F0 modifications**:
    `git diff main -- crates/test-ir-fixtures/` returns zero.

11. **No scope creep**: `git diff main` touches ONLY the files
    listed in Quality Gate's `git diff --stat` enumeration. No
    bridge / model / spec / pretty-printer / generic-c-backend /
    matrix / strategy doc changes.

12. **Open question adjudications recorded**: the harness
    expectations file
    (`.harness/expectations/regseq-driver-64.md`) records the Codex
    adjudication outcomes for the three open questions (a), (b),
    (c) so future F2-F5 work has the precedent.

13. **F2 readiness signal**: the expectations file's closing note
    states whether the runner's API + report shape are
    operationally sufficient for the F2 LLM authoring loop per
    strategy §11. This is the bridge between F1 and F2.
