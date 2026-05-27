# regseq-backends-04: Scaffold `codegen-coverage-runner` crate + 6-stage harness plumbing (Phase-1 task a)

## Description

Scaffold `crates/codegen-coverage-runner/` per
`docs/discussions/generic-c-backend/phase-1/03-verification-flow.md`
(Doc 3, Codex-APPROVED at HEAD) — the codegen analog of the
`bridge-coverage-runner` that closed the front-end gate. This task lands
the runner binary crate, the **6-stage pipeline** (Doc 3 §1), the **dual
oracle** (Doc 3 §2), the **fixture manifest schema** (Doc 3 §3), the
**Doc-1 matrix parser + coverage ledger** (Doc 3 §5), and the **host-`cc`
subprocess toolchain** (Doc 3 §4/§7), plus the **emit seam** into
`regseq-backends` that the pipeline's stage 2 consumes.

This is **task a** of the Phase-1 implementation sequence (`.harness/HANDOFF.md`
§ Current state → Next actions: a→e). It is filed first **deliberately**:
the generic-C backend (`crates/regseq-backends/`) is a 2-line stub, so the
harness is built *before* the backend can emit real C. Stage 2 therefore
calls a backend seam that returns a structured "not yet implemented" error;
the harness reports honest red (verdict `not-pass`) against an **empty
fixture corpus** until tasks b–e implement the lowering and author the first
fixtures. Every stage that *can* be exercised independently of emit — stage
1 (bridge), stages 4–6 (compile/run/oracle, driven with hand-written C in
tests), the manifest parser, the matrix parser, and the ledger generator —
**is** exercised by this task's tests. The seam to the (stubbed) backend is
verified to surface its error as a stage-2 failure.

Three scope decisions this task makes (flagged for review — see § Review):

1. **The emit seam lives in `regseq-backends`** (Change B): this task adds
   the public `emit_generic_c(&RegseqModel) -> Result<String, EmitError>`
   contract as a stub returning `Err(EmitError::NotImplemented)`. The
   harness is the seam's first consumer; tasks b–e implement its body and
   extend `EmitError`. Signature falls straight out of Doc 2 D3 (one `.c`
   string per model) — no design freedom — so the consumer defines it.
2. **`kind = "ir"` fixtures are deferred** (Non-Goals): the manifest schema
   *parses* the `kind` field faithfully (Doc 3 §3), but the executor
   supports only `kind = "rust"` (bridge a source snippet, the path with a
   clean runtime story). `kind = "ir"` (hand-built models compiled into the
   binary) needs a builder-registration design and is not on the path to
   the first behavioral fixtures; it returns a clear "unsupported in this
   phase" error.
3. **No coverage corpus ships** (Non-Goals): `golden.c` requires a working
   emit, so this task ships **zero** `fixtures/<name>/` directories. The
   first real fixture is task e. This task ships the fixture *home* + README
   documenting the schema, and the ledger run over the empty corpus is the
   honest "nothing implemented yet" Phase-1 baseline.

## Input Context

### Design source

- `docs/discussions/generic-c-backend/phase-1/03-verification-flow.md`
  (Doc 3 — the LOAD-BEARING design source-of-truth, Codex-APPROVED).
  Section anchors this task consumes:
  - §1 (The pipeline): the 6 stages + per-stage boundary + failure
    attribution. Each stage is a function with a stage-tagged result.
  - §2 (The dual oracle): stage-3 snapshot (determinism/review) + stage-6
    behavioral (stdout), both required; panic-fixtures invert stage 5
    (assert stderr + non-zero exit). The harness **never auto-blesses** a
    snapshot diff.
  - §3 (Fixture corpus): the on-disk layout + `manifest.toml` schema
    (`name`, `kind` rust|ir, `entry` default `regseq_main`, `covers` array
    of Doc-1 row IDs, `expect` pass|panic); the per-fixture file set
    (`input.rs` | `model.rs`, `golden.c`, `expected.stdout`,
    `expected.stderr` [panic only]).
  - §4 (The harness): a workspace crate driven by a Mode-A CLI; host `cc`
    in a per-fixture hermetic temp dir; determinism + hermeticity.
  - §5 (The ledger): `.harness/generic-c/coverage_ledger.{json,md}`
    regenerated each run from (a) Doc-1 matrix row-set + design status and
    (b) fixture results; drift = covered-without-green or green-without-
    covered; top-level `verdict`.
  - §6 (Verdict + exit gate): per-fixture PASS/FAIL; phase verdict = `pass`
    iff every in-scope `covered` Doc-1 row has ≥1 green fixture and no
    fixture FAILs.
  - §7 (Toolchain): pinned `cc` flags `-std=gnu11 -fwrapv -Wall`; the
    `_Float16/_Float128` width gate (per-fixture skip with recorded reason);
    the three non-flag host assumptions (recorded, harness-config error if
    unmet); compiler + version recorded in the ledger.
- `docs/discussions/generic-c-backend/phase-1/02-architecture-decisions.md`
  § D3 (lines 144–174) — **single TU + generated `main.c`**: one `.c` per
  model (no `.h`); the harness *writes* a `main.c` that calls a designated
  entry instance (`regseq_main` by default, or the manifest's `entry`);
  `int main(void) { regseq_main(); return 0; }`; entry returns unit/`void`
  or an `{i,u}32` exit code; a non-conforming entry return is a
  harness-setup error.
- `docs/discussions/regseq-model/generic-c-backend.md` §15 (lines 988–1044,
  the 5-bank single-TU emission order — context for what stage 2 *will*
  produce) + §16 (lines 1048–1078, the `regseq_panic` → **stderr** +
  `abort()` routine — why panic-fixtures assert on stderr, keeping the
  stdout oracle clean).

### Matrix source (consumed read-only by the ledger generator)

- `docs/discussions/generic-c-backend/phase-1/01-construct-surface-and-matrix.md`
  (Doc 1, Codex-APPROVED). The runner parses its tables for the **row
  set + design status**. Table format (uniform across all family tables):
  ```
  | id | construct (variant/field) | C lowering | status | spec ref | notes |
  ```
  Column 1 = row ID (e.g. `IT-1`, `IN-5b`, `BD-2`, `TY-18`, `RV-6`,
  `AK-3`, `LY-7`); column 4 = `status` ∈ the **legend** (Doc 1 lines
  31–44): `covered` | `deferred` | `unspecced` | `out-of-scope`. The parser
  extracts (id, status) from every table row, skipping the header + the
  `| -- | ... |` separator rows.
  - **Note on the status column (assumption, confirm with architect):** Doc
    1's per-row statuses are its design-time snapshot; many rows the spec
    delta later resolved are still marked `unspecced`/`deferred` in the
    frozen Doc 1. This task treats the `status` column as the **current
    design status** the ledger reflects verbatim. The workflow for flipping
    `unspecced`/`deferred` → `covered` as tasks b–e close gaps (edit Doc 1?
    a side scope-list?) is **out of scope here** — it does not change the
    harness's parse/aggregate machinery, and with the empty corpus the
    verdict is `not-pass` regardless of which rows read `covered`.

### Code anchors

- `crates/regseq-backends/src/lib.rs` — the current 2-line stub (`//!`
  doc-comment only). Change B replaces it with the emit seam.
- `crates/test-ir-fixtures/src/lib.rs:223` — `try_bridge_fixture_path(path:
  &Path) -> BridgeOutcome`: the stage-1 bridge invocation. Returns
  `BridgeOutcome` (lines 85–112): `Ok(RegseqModel)` (bridge **and**
  `validate()` both succeeded — Doc 3 stage-1 "pass"), `ValidatorFailed
  { model, violation }`, `Rejected(BridgeError)`, `RustcRejected { stderr }`.
  Only `Ok(model)` is a stage-1 pass; the other three are stage-1 failures.
- `crates/bridge-coverage-runner/` — the direct structural analog (the
  archived bridge runner). Reuse its shape, NOT its content:
  - `build.rs` — the **sysroot-rpath** build script
    (`rustc --print=sysroot` → `cargo:rustc-link-arg=-Wl,-rpath,<sysroot>/lib`).
    Required because stage 1 invokes the rustc-linked bridge. Copy verbatim.
  - `src/main.rs:13-15` — `#![feature(rustc_private)]` + `extern crate
    rustc_driver;`, inherited for the same reason; the hand-rolled
    Mode-A arg parser (no `clap`).
  - `Cargo.toml` — `publish = false`, `[package.metadata.rust-analyzer]
    rustc_private = true`, `[[bin]]`, path deps. The codegen runner's deps
    differ (see Change A).
- `Cargo.toml:3-14` — workspace `members` (alphabetical). Change A inserts
  `crates/codegen-coverage-runner` (sorts before `crates/regseq`).

### Prior tasks

- `test-ir-fixtures-01` (done): the fixture harness whose
  `try_bridge_fixture_path` this runner's stage 1 consumes.
- `regseq-backends-03` (done): the prior task in this group (chronological
  lineage; the C-AST work predates the new-vocabulary backend and is not
  consumed here).
- Bridge front-end feature-complete gate (`1a6a996`, archived HANDOFF):
  stage 1's bridge is structurally complete, so source snippets bridge to a
  valid `RegseqModel`.

### Test surface

- New: `crates/codegen-coverage-runner/tests/` — integration tests per
  Change J. No existing test file is modified.
- `crates/regseq-backends/` currently has no tests; Change B's seam stub is
  covered by a trivial unit test there (the seam returns its error) **or**
  by the runner's stage-2 seam test — see Change J.

## Implementation Notes

### Change A: workspace registration + crate manifest + build.rs

**Anchor**: `Cargo.toml:3-14`.

Insert `crates/codegen-coverage-runner` into `members`, keeping
alphabetical order (it sorts immediately before `crates/regseq`):

```toml
members = [
    "crates/bridge-coverage-runner",
    "crates/cargo-regseq",
    "crates/codegen-coverage-runner",
    "crates/regseq",
    ...
]
```

**New file**: `crates/codegen-coverage-runner/Cargo.toml`.

```toml
[package]
name = "codegen-coverage-runner"
version = "0.1.0"
edition = "2021"
publish = false

[package.metadata.rust-analyzer]
rustc_private = true

[[bin]]
name = "codegen-coverage-runner"
path = "src/main.rs"

[dependencies]
test-ir-fixtures = { path = "../test-ir-fixtures" }
regseq-model = { path = "../regseq-model" }
regseq-backends = { path = "../regseq-backends" }
toml = "0.8"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

Rationale per dep: `test-ir-fixtures` = stage-1 bridge; `regseq-model` =
`RegseqModel`; `regseq-backends` = the stage-2 emit seam (Change B);
`toml` + `serde/derive` = manifest parsing (Doc 3 §3); `serde_json` =
ledger JSON (Doc 3 §5). No `clap` (hand-rolled CLI per the bridge-runner
precedent), no `tempfile` (use `std::env::temp_dir()` + a process-unique
stem, cleaned per fixture), no `cc`/`regex` crate.

**New file**: `crates/codegen-coverage-runner/build.rs` — copy
`crates/bridge-coverage-runner/build.rs` verbatim (sysroot-rpath, required
because stage 1 links the rustc-backed bridge).

**New file**: `crates/codegen-coverage-runner/.gitignore` — ignore any
scratch run output dir if the CLI writes one (mirror the bridge runner's
`reports/` ignore only if Change I emits there; otherwise omit).

### Change B: `regseq-backends` emit seam (the stage-2 contract)

**Anchor**: `crates/regseq-backends/src/lib.rs` (the 2-line stub).

Replace the stub body with the **public emit entry point** the harness's
stage 2 calls — the contract tasks b–e implement:

```rust
//! Regseq codegen backends (Linux, Zephyr). Generic-C backend entry point.

use regseq_model::RegseqModel;

/// Lower a post-mono `RegseqModel` to a single generic-C translation unit
/// (`generic-c-backend.md` §15 five-bank order). Phase-1 tasks b–e fill
/// this in; until then it returns [`EmitError::NotImplemented`].
pub fn emit_generic_c(model: &RegseqModel) -> Result<String, EmitError> {
    let _ = model;
    Err(EmitError::NotImplemented)
}

/// Errors from generic-C emission. Tasks b–e extend this; the only variant
/// this task ships is the not-yet-implemented marker.
#[derive(Debug)]
#[non_exhaustive]
pub enum EmitError {
    /// The generic-C backend lowering is not yet implemented.
    NotImplemented,
}

impl std::fmt::Display for EmitError { /* "generic-C emission not implemented" */ }
impl std::error::Error for EmitError {}
```

Confirm the exact `RegseqModel` import path against
`crates/regseq-model/src/` at HEAD. `#[non_exhaustive]` lets b–e add
variants without a breaking change. The harness consumes only `Ok(String)`
vs `Err(impl Display)` — it does **not** match individual `EmitError`
variants, so it stays decoupled from b–e's error taxonomy.

This is the **sole** edit to `regseq-backends` in this task — no lowering
logic. (See § Review: confirm the seam belongs here vs. task b.)

### Change C: `src/manifest.rs` — fixture manifest schema + discovery

**New file**: `crates/codegen-coverage-runner/src/manifest.rs`.

The `manifest.toml` schema per Doc 3 §3:

```rust
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub name: String,
    pub kind: FixtureKind,
    #[serde(default = "default_entry")]
    pub entry: String,           // emission_name of the entry instance; default "regseq_main"
    pub covers: Vec<String>,     // Doc-1 matrix row IDs
    pub expect: Expect,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum FixtureKind { Rust, Ir }

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Expect { Pass, Panic }

fn default_entry() -> String { "regseq_main".to_string() }
```

Plus a `discover(fixtures_dir) -> Result<Vec<Fixture>, ManifestError>` that
walks `fixtures_dir`, loads each `<name>/manifest.toml`, resolves the
sibling input/golden/expected paths, and returns fixtures **sorted by
`name`** (Doc 3 §4 determinism). A `Fixture` carries the parsed manifest +
the resolved `PathBuf`s (input, golden_c, expected_stdout, expected_stderr
[Some iff `expect == Panic`]). An empty `fixtures_dir` returns an empty
`Vec` (the Phase-1 baseline), **not** an error.

### Change D: `src/matrix.rs` — Doc-1 matrix row-set + status parser

**New file**: `crates/codegen-coverage-runner/src/matrix.rs`.

Parse the Doc-1 markdown tables (Input Context → Matrix source) into the
row universe:

```rust
pub struct MatrixRow { pub id: String, pub status: Status }

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Status { Covered, Deferred, Unspecced, OutOfScope }

pub fn parse(matrix_md_path: &Path) -> Result<Vec<MatrixRow>, MatrixError>;
```

Algorithm: read UTF-8; a line is a **candidate matrix data row** iff it
starts with `|`, splits into ≥ 6 `|`-delimited cells, and its **first cell**
trims to a Doc-1 row-ID shape. Doc-1 IDs come in three observed forms
(verify against HEAD): plain `IT-1` / `TY-18`, suffixed `IN-5b`, and
**grouped ranges** `BO-1..5` / `BO-12..14`
(`01-construct-surface-and-matrix.md:275-280`). The accepting shape is
therefore `[A-Z]{2,3}-\d+(\.\.\d+)?[a-z]?`. For a candidate row, cell 4
(status) **must** be a legend value (`covered` / `deferred` / `unspecced` /
`out-of-scope`); a candidate whose cell 4 is not a legend value is a **hard
error** (the matrix drifted from the legend). Header rows (`| id |`) and
separator rows (`| -- |`) fail the ID-shape test and are skipped.

**Row-ID granularity (no range expansion).** The grouped range
`BO-1..5` is taken **verbatim as a single row ID** — the harness mirrors
Doc 1's row granularity 1:1 and does **not** expand it into `BO-1 … BO-5`.
Fixtures reference the verbatim Doc-1 ID in their `covers` list
(`covers = ["BO-1..5", ...]`). (Rationale: expansion would invent coverage
semantics not present in Doc 1; if finer attribution is wanted, Doc 1 splits
the row. Confirm this verbatim policy in review.)

Return rows **sorted by id**, duplicates rejected (each Doc-1 row ID is
unique). Hand-roll the cell split (no `regex` dep).

### Change E: `src/toolchain.rs` — `main.c` generation + `cc` compile/run (stages 4–5)

**New file**: `crates/codegen-coverage-runner/src/toolchain.rs`.

The **emit-independent** half of the pipeline — takes a C source **string**
(not a model), so it is unit-testable with hand-written C:

```rust
pub struct RunOutput { pub stdout: String, pub stderr: String, pub exit_code: Option<i32> }

pub enum ToolchainError { CompileFailed { stderr: String }, /* spawn/io */ ... }

/// Generate the Doc-2 D3 `main.c` for an entry instance name.
pub fn generate_main_c(entry: &str) -> String;   // int main(void){ entry(); return 0; }

/// Compile `tu_c` + a generated `main.c` for `entry` with the pinned flags,
/// run the binary in a hermetic temp dir, capture stdout/stderr/exit.
pub fn compile_and_run(tu_c: &str, entry: &str, cc: &Path) -> Result<RunOutput, ToolchainError>;
```

- Hermetic temp dir under `std::env::temp_dir()` with a process-unique stem;
  write `tu.c` + `main.c`; cleaned (RAII / explicit `remove_dir_all`) even
  on error.
- `cc` flags per Doc 3 §7: `-std=gnu11 -fwrapv -Wall` (+ `-o <bin>`). A
  non-zero `cc` exit → `ToolchainError::CompileFailed { stderr }` (stage-4
  failure). `-Werror` is **not** set (Doc 3 §1 stage-4 "no warnings-as-
  errors" is the spec's wording for the gate, not a `cc` flag here —
  warnings are surfaced but do not fail compile; confirm interpretation in
  review).
- Run the binary, capturing stdout/stderr/exit code. `exit_code` is `None`
  if the process was killed by a signal (an `abort()` from `regseq_panic`
  shows as a signal death / non-zero — the panic-fixture path keys on "did
  not exit 0", per stage-5 inversion).
- **The entry contract is the harness's** (Doc 2 D3, `02-architecture-
  decisions.md:155-166`): the harness writes `main.c`, the entry takes no
  args and returns unit/`void` **or** an `{i,u}32` exit code, and a
  non-conforming entry return is a harness-setup error. This task implements
  the **unit-`void` form** — `int main(void) { regseq_main(); return 0; }`.
  `{i,u}32` exit-code propagation (and the nonconforming-return setup error)
  is a **deferred harness feature** — it lands with the first fixture whose
  entry returns a code (tasks b–e), **not** owned by emit. `generate_main_c`
  is the unit-`void` form only for now.

### Change F: `src/oracle.rs` — snapshot + behavioral comparison (stages 3, 6)

**New file**: `crates/codegen-coverage-runner/src/oracle.rs`.

Pure comparison logic (string in, verdict out — unit-testable):

```rust
/// Stage 3: emitted `.c` vs golden `.c`. Byte-exact.
pub fn snapshot(emitted: &str, golden: &str) -> Result<(), OracleMismatch>;

/// Stage 6 (expect = pass): captured stdout vs golden. Byte-exact.
pub fn behavioral_stdout(captured: &str, golden: &str) -> Result<(), OracleMismatch>;

/// Stage 5/6 (expect = panic): assert non-zero exit + stderr matches golden.
pub fn behavioral_panic(run: &RunOutput, golden_stderr: &str) -> Result<(), OracleMismatch>;
```

`OracleMismatch` carries a unified-ish diff summary (first differing line +
context) for the report. The harness **never** rewrites a golden here
(no auto-bless — Doc 3 §2); golden (re)generation is the explicit Change-I
`--bless` path, a human-invoked action.

### Change G: `src/pipeline.rs` — the 6-stage per-fixture pipeline

**New file**: `crates/codegen-coverage-runner/src/pipeline.rs`.

Compose the stages with **failure attribution** (Doc 3 §1: a failure is
tagged to its stage so a snapshot drift is distinguishable from a
behavioral miscompute or a compile error):

```rust
pub enum Stage { Acquire, Emit, Snapshot, Compile, Run, Behavioral }

pub struct FixtureResult {
    pub name: String,
    pub covers: Vec<String>,
    pub verdict: Verdict,            // Pass | Fail { stage: Stage, detail: String } | Skipped { reason }
}

pub fn run_fixture(fixture: &Fixture, cc: &Path) -> FixtureResult;
```

Stage wiring:
1. **Acquire** — `kind == Rust`: `try_bridge_fixture_path(input)`; `Ok(model)`
   → continue, else `Fail { Acquire, .. }`. `kind == Ir` → `Skipped { reason:
   "kind=ir unsupported in Phase-1 task a" }` (Non-Goals).
2. **Emit** — `regseq_backends::emit_generic_c(&model)`; `Ok(c)` → continue,
   `Err(e)` → `Fail { Emit, detail: e.to_string() }`. (Until tasks b–e land,
   every rust fixture stops here — the honest Phase-1-baseline behavior.)
3. **Snapshot** — `oracle::snapshot(&c, &golden_c)`; mismatch →
   `Fail { Snapshot, .. }`.
4. **Compile** — `toolchain::compile_and_run` (compile half); failure →
   `Fail { Compile, .. }`.
5. **Run** — captured by stage 4's `compile_and_run`; the run result is
   inspected here.
6. **Behavioral** — `expect == Pass`: `oracle::behavioral_stdout` (and
   assert exit 0 / clean stderr); `expect == Panic`: `oracle::behavioral_panic`
   (non-zero exit + `expected.stderr`). Mismatch → `Fail { Behavioral, .. }`.

`_Float16/_Float128` width-gated fixtures (Doc 3 §7) → `Skipped { reason }`
with the recorded reason (the gating mechanism: a manifest field or a probe
— defer the *trigger* to the first such fixture in tasks b–e; ship the
`Skipped` plumbing now).

### Change H: `src/ledger.rs` — coverage ledger generation (Doc 3 §5/§6)

**New file**: `crates/codegen-coverage-runner/src/ledger.rs`.

Aggregate (matrix rows × fixture results) → the ledger + verdict:

```rust
pub struct Ledger { pub verdict: Verdict, pub rows: Vec<LedgerRow>, pub toolchain: ToolchainInfo }
pub struct LedgerRow { pub id, pub status: Status, pub fixtures: Vec<String>, pub result: RowResult, pub last_run }
// RowResult: Pass | Fail | NotApplicable (for deferred/unspecced/out-of-scope rows)
```

- Map each matrix row to the fixtures whose `covers` lists it; a row's
  `result` = `Pass` iff `status == Covered` and ≥1 covering fixture is green;
  `Fail` if a covering fixture FAILs or a `Covered` row has no green fixture;
  `NotApplicable` for `deferred` / `out-of-scope` rows (not gated this phase).
- **Drift** (Doc 3 §5, `03-verification-flow.md:148-152` — **both
  directions**):
  1. a `covers` row ID **absent from the matrix** → hard error (the fixture
     invented a row);
  2. a `Covered` matrix row with **no green fixture** (covered-without-green);
  3. a **green fixture whose `covers` names a non-`Covered` row**
     (green-without-covered — exercising a row the matrix marks
     `unspecced`/`deferred`/`out-of-scope`).
  Record all three in the ledger; (2) and (3) make the verdict `not-pass`.
- **Verdict** (Doc 3 §6, `03-verification-flow.md:158-166`):
  - `out-of-scope` rows never gate.
  - `deferred` rows are not gated this phase (deferred to a later phase by
    design).
  - `unspecced` rows are **unresolved in-scope gaps**: while any remain, the
    verdict is `not-pass` and the ledger lists them as open gaps (Doc 3 §6:
    "deferred/unspecced rows must be resolved … before the gate can read
    pass"; `deferred` is the by-design exception, `unspecced` is not).
  - `pass` iff **no `unspecced` rows remain** AND every `Covered` row has ≥1
    green fixture AND no fixture FAILed; else `not-pass`. With the empty
    corpus + the stubbed emit, the verdict is `not-pass` (the honest
    baseline).
  - **Status-flip dependency (flag for the architect).** Reaching `pass`
    requires the Doc-1 rows the Doc-2 decisions + spec delta *resolved* to
    read `covered` in the matrix (an `unspecced` row that the spec delta
    already pinned must be flipped to `covered`). The **workflow** for that
    flip — edit Doc 1 as each task b–e closes a gap, vs. a side scope-list —
    is **not decided by this task** and is an open process question (Input
    Context → Matrix source note). The harness machinery above is correct
    under either resolution; only the *reachability* of `pass` depends on
    it. This task ships the machinery + the honest `not-pass` baseline.
- `ToolchainInfo` = `cc` path + `cc --version` first line (Doc 3 §7
  reproducibility record).
- Write **both** `coverage_ledger.json` (serde_json, pretty, sorted rows)
  and `coverage_ledger.md` (human rollup regenerated from the JSON) to the
  ledger-out dir (default `.harness/generic-c/`, created if absent).
  Confirm the dir's existing contents at HEAD before writing (don't clobber
  unrelated trackers).

### Change I: `src/main.rs` — Mode-A CLI + run loop

**New file**: `crates/codegen-coverage-runner/src/main.rs`.

`#![feature(rustc_private)]` + `extern crate rustc_driver;` (stage-1 bridge
linkage, per the bridge-runner precedent). Modules: `manifest`, `matrix`,
`toolchain`, `oracle`, `pipeline`, `ledger`. Hand-rolled arg parser.

Flags:
- `--fixtures <dir>` (default `crates/codegen-coverage-runner/fixtures/`).
- `--matrix <path>` (default the Doc-1 matrix md path).
- `--ledger-out <dir>` (default `.harness/generic-c/`).
- `--cc <path>` (default `cc`).
- `--bless` (off by default): regenerate `golden.c` / `expected.stdout` /
  `expected.stderr` for each fixture from the actual emit/run (the
  human-invoked authoring action; **not** auto-bless — see Doc 3 §2). With
  the empty corpus + stubbed emit this is a no-op this task; ship the flag +
  plumbing, document that it is inert until emit lands.

Run loop: parse matrix → discover fixtures → `run_fixture` each (sorted) →
aggregate into the ledger → write ledger → print a summary → exit `0` iff
verdict `pass` and no fixture FAILed, else `1`. (Empty corpus + any
`Covered` row → exit `1`, honestly.)

### Change J: `tests/` — per-stage verification (no real fixtures)

**New files** under `crates/codegen-coverage-runner/tests/`.

Because emit is stubbed and no fixtures ship, the tests verify each stage
*that can be exercised independently*:

1. **`toolchain.rs`** (stages 4–6, the load-bearing proof): drive
   `compile_and_run` + the oracle with **hand-written C strings** (inline in
   the test, not committed fixtures):
   - a "pass" C TU (`regseq_main` prints `"[INFO] result=42\n"`) compiles,
     runs, exits 0, and `behavioral_stdout` matches.
   - a "panic" C TU (calls `abort()` after printing to stderr) → non-zero
     exit; `behavioral_panic` matches the stderr.
   - a malformed C TU → `ToolchainError::CompileFailed`.
   These prove the `cc` subprocess + hermetic temp dir + capture + dual
   oracle end-to-end against real C — the machinery tasks b–e depend on.
2. **`bridge_smoke.rs`** (stage 1): bridge a trivial `#![no_std]` snippet
   via `try_bridge_fixture_path`, assert `BridgeOutcome::Ok(model)` (stage-1
   pass path is wired).
3. **`emit_seam.rs`** (stage 2): assert `regseq_backends::emit_generic_c`
   on any model returns `Err`, and that `pipeline::run_fixture` surfaces it
   as `Fail { stage: Emit, .. }` (the seam is connected and honest).
4. **`manifest.rs`** unit: parse a valid `manifest.toml` (rust + ir + panic
   variants), the `entry` default, and reject a malformed one.
5. **`matrix.rs`** unit: parse a small fixed markdown table → expected
   (id, status) rows; reject an unknown status string.
6. **`ledger.rs`** unit: aggregate synthetic (rows × results) → expected
   verdict, covering all three drift directions + the verdict rule:
   - empty-corpus + a `Covered` row → `not-pass` (covered-without-green);
   - a `covers` ID absent from the matrix → hard drift error;
   - a green fixture covering a non-`Covered` row → green-without-covered
     drift → `not-pass`;
   - a remaining `unspecced` row → `not-pass` (not gated away like
     `deferred`/`out-of-scope`);
   - all-`Covered`-green + no `unspecced` + no fixture FAIL → `pass`;
   - deterministic (sorted) row order in the JSON.

Prefer integration tests (`tests/`) invoking the public module surface; if
a module is binary-private (`main.rs` modules), expose the needed functions
via a thin `src/lib.rs` **only if** required for testability (the bridge
runner was binary-only — match that if integration tests can reach the
logic via the binary; otherwise a minimal `lib.rs` is acceptable and should
be called out in review).

### Change K: `fixtures/README.md` — schema documentation (empty corpus)

**New file**: `crates/codegen-coverage-runner/fixtures/README.md`.

Document the per-fixture directory layout + `manifest.toml` schema (Doc 3
§3) so task e (and beyond) author fixtures against a written convention.
State explicitly that the corpus is **empty in this task** (golden.c needs
a working emit) and that the first fixture lands with task e. No `<name>/`
directories ship.

## Non-Goals

- Do NOT implement any generic-C lowering. Change B ships the seam **stub
  only**; the prelude/envelope/types/bodies/diag-log are tasks b–e.
- Do NOT author any coverage fixture (no `fixtures/<name>/` dirs, no
  `golden.c`, no `input.rs`). `golden.c` requires a working emit; the first
  fixture is task e. Change K ships the README + schema only.
- Do NOT execute `kind = "ir"` fixtures. The schema parses `kind`; the
  executor `Skip`s `ir` with a clear reason. Builder-registration design
  for hand-built models is a later task.
- Do NOT auto-bless snapshot or stdout goldens. `--bless` is human-invoked
  and inert this task (Doc 3 §2: the harness never auto-blesses).
- Do NOT add CI wiring (Doc 3 §6 CI integration is a separate, later step,
  mirroring the bridge runner deferring CI).
- Do NOT modify Doc 1 / Doc 2 / Doc 3, `generic-c-backend.md`, `spec.md`,
  the bridge, the model, or `test-ir-fixtures`. This task is the runner
  crate + the `regseq-backends` seam stub only.
- Do NOT add deps beyond `test-ir-fixtures`, `regseq-model`,
  `regseq-backends`, `toml`, `serde/derive`, `serde_json`. No `clap`,
  `tempfile`, `regex`, `assert_cmd`, or a `cc` crate.
- Do NOT resolve the Doc-1 status-flip workflow (Input Context note). This
  task reflects the matrix's current statuses verbatim.

## Test Plan

### B1: crate builds + workspace stays green
- `cargo build -p codegen-coverage-runner` clean.
- `cargo build -p regseq-backends` clean (the seam stub compiles).
- `cargo test --workspace -- --test-threads=1` green (existing tests
  unchanged; the new tests pass). Per the HANDOFF cargo-test gotcha, use
  `RUST_TEST_THREADS=1` if the parallel E0463 quirk appears.

### B2: per-stage tests (Change J) — the load-bearing verification
- `tests/toolchain.rs`: pass-C compiles+runs+stdout matches; panic-C →
  non-zero exit + stderr matches; malformed-C → `CompileFailed`.
- `tests/bridge_smoke.rs`: trivial snippet → `BridgeOutcome::Ok`.
- `tests/emit_seam.rs`: emit returns `Err`; pipeline tags `Fail { Emit }`.
- `tests/manifest.rs`, `tests/matrix.rs`, `tests/ledger.rs`: parse +
  aggregate + verdict + drift + determinism units pass.

### B3: end-to-end Mode-A run (honest baseline)
Run the binary against the empty corpus:
```bash
cargo run -p codegen-coverage-runner -- \
  --fixtures crates/codegen-coverage-runner/fixtures/ \
  --matrix docs/discussions/generic-c-backend/phase-1/01-construct-surface-and-matrix.md \
  --ledger-out .harness/generic-c/
```
Asserts: writes `coverage_ledger.{json,md}`; verdict `not-pass` (empty
corpus, `Covered` rows unfilled); exit code `1`; the JSON records the `cc`
version. This is the Phase-1 baseline ledger.

### B4: fmt + clippy
- `cargo fmt --all -- --check` clean.
- `cargo clippy -p codegen-coverage-runner -p regseq-backends -- -D warnings`
  clean. (Per [[clippy-all-targets-test-drift]], do **not** gate on
  `--all-targets --workspace`; scope clippy to the two crates this task
  touches + their new tests.)

### B5: diff scope
`git status` / `git diff --stat` confined to: `Cargo.toml` (member add),
`crates/codegen-coverage-runner/**` (new), `crates/regseq-backends/src/lib.rs`
(seam), the generated `.harness/generic-c/coverage_ledger.{json,md}`
baseline, the new `.harness/tasks/regseq-backends/04-*.md` + `tasks.json`
entry. No doc/spec/bridge/model/test-ir-fixtures changes.

## Review

### Files to send to Codex
- This spec
  (`.harness/tasks/regseq-backends/04-codegen-coverage-runner-scaffold.md`).
- Doc 3 (`docs/discussions/generic-c-backend/phase-1/03-verification-flow.md`)
  — the design source-of-truth.
- Doc 2 §D3 + Doc 1 (the matrix the parser consumes + the manifest/entry
  contract).
- `generic-c-backend.md` §15/§16 (what stage 2 will emit; the panic→stderr
  split).
- `crates/test-ir-fixtures/src/lib.rs:85-237` (the stage-1 API + outcomes),
  `crates/bridge-coverage-runner/{build.rs,Cargo.toml,src/main.rs}` (the
  structural analog), `crates/regseq-backends/src/lib.rs` (the seam target).

### Reviewer focus
- **Doc-3 fidelity**: do the 6 stages + dual oracle + manifest schema +
  ledger + verdict match Doc 3 §1–§6 exactly?
- **The three scope decisions** (Description): (1) emit seam in
  `regseq-backends` vs. deferring to task b; (2) `kind=ir` deferral; (3)
  empty-corpus / no-golden. AGREE or propose alternative.
- **Seam signature**: is `emit_generic_c(&RegseqModel) -> Result<String,
  EmitError>` the right contract given Doc 2 D3 (one `.c` per model; harness
  generates `main.c`)? Is `#[non_exhaustive] EmitError` the right
  decoupling so b–e extend it freely?
- **`main.c` / entry-return** (Change E): is the unit-`void` `int main(void)
  { regseq_main(); return 0; }` sufficient for this task, with `{i,u}32`
  exit-code propagation deferred to whoever lands the first entry that
  returns a code? Or must it be handled now?
- **Stage-4 "no warnings-as-errors"** (Change E): is surfacing `-Wall`
  warnings without `-Werror` the correct reading of Doc 3 §1 stage-4?
- **Testability without emit**: are the Change-J per-stage tests a sound
  substitute for the bridge runner's whole-loop smoke test, given emit is
  stubbed? Is the `lib.rs`-for-testability escape hatch acceptable, or
  should the runner stay binary-only and drive logic through the binary?
- **Doc-1 status-flip note**: is treating the matrix status verbatim (and
  deferring the flip workflow) correct, or does it need pinning now?
- **Citation drift**: verify the file:line anchors against HEAD.

### Adversarial prompt template
```
<task>
Review .harness/tasks/regseq-backends/04-codegen-coverage-runner-scaffold.md
— the scaffold for the codegen-coverage harness (Phase-1 task a), built
BEFORE the generic-C backend exists (it is a 2-line stub). Identify:
- Doc-3 fidelity (6 stages / dual oracle / manifest schema / ledger / verdict).
- Soundness of the three scope decisions: emit seam in regseq-backends;
  kind=ir deferral; empty-corpus / no-golden-this-task.
- Whether the emit-seam signature + #[non_exhaustive] EmitError is the
  right harness↔backend contract per Doc 2 D3.
- Whether the Change-J per-stage tests genuinely verify the machinery
  given emit is stubbed (no whole-loop smoke test is possible).
- Scope creep against the Non-Goals.
- Citation drift against HEAD.
Read: Doc 3, Doc 2 §D3, Doc 1, generic-c-backend.md §15/§16,
test-ir-fixtures/src/lib.rs, bridge-coverage-runner/{build.rs,main.rs},
regseq-backends/src/lib.rs.
</task>

<grounding_rules>
- Cite file:line for every claim. Mark hypotheses as hypotheses.
- Don't speculate about code/spec you didn't read.
</grounding_rules>

<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
DOC-3 FIDELITY: PASS / FAIL with citation
SCOPE-DECISIONS: per-decision AGREE / DISAGREE_<rationale>
SEAM-CONTRACT: PASS / FAIL with citation
TESTABILITY-WITHOUT-EMIT: PASS / FAIL with citation
SCOPE-CREEP: PASS / FAIL
CITATION DRIFT: list or "none"
NEW ISSUES: numbered or "none"
REQUIRED FIXES: numbered (only if VERDICT != APPROVED)
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, decide, stop.
</default_follow_through_policy>
```

## Quality Gate
- `cargo build -p codegen-coverage-runner` + `-p regseq-backends` clean.
- `cargo fmt --all -- --check` clean.
- `cargo clippy -p codegen-coverage-runner -p regseq-backends -- -D warnings`
  clean.
- `cargo test --workspace -- --test-threads=1` green (existing unchanged +
  the new per-stage tests).
- B3 Mode-A run writes the baseline ledger with verdict `not-pass` + exit 1.
- `git diff --stat` confined to the Change-A/B/C–K file set + the tasks
  registration. No doc/spec/bridge/model/test-ir-fixtures changes.
- Codex Task Review: APPROVED or APPROVED_WITH_MINOR_FIXES.

## Acceptance Criteria

1. **Workspace registration**: `Cargo.toml` `members` contains
   `crates/codegen-coverage-runner` (alphabetical).
2. **Crate scaffold**: the crate builds with `build.rs` (sysroot-rpath),
   `#![feature(rustc_private)]`, and the module set `manifest` / `matrix` /
   `toolchain` / `oracle` / `pipeline` / `ledger` + `main` (Doc 3 §4).
3. **Emit seam**: `regseq_backends::emit_generic_c(&RegseqModel) ->
   Result<String, EmitError>` exists, returns `Err(EmitError::NotImplemented)`,
   and is the **only** change to `regseq-backends`.
4. **6-stage pipeline**: `pipeline::run_fixture` implements stages 1–6 with
   per-stage failure attribution (Doc 3 §1); `kind=ir` and width-gated
   fixtures `Skip` with a recorded reason.
5. **Dual oracle**: stage-3 snapshot (byte-exact) + stage-6 behavioral
   (stdout, or panic stderr+non-zero exit) per Doc 3 §2; no auto-bless.
6. **Manifest schema**: `manifest.toml` parses `name` / `kind` / `entry`
   (default `regseq_main`) / `covers` / `expect` per Doc 3 §3; discovery is
   name-sorted; empty corpus → empty result, not an error.
7. **Matrix parser**: parses Doc-1 tables into (id, status) rows, accepting
   the plain / suffixed / **grouped-range** ID forms (`IT-1`, `IN-5b`,
   `BO-1..5`) **verbatim** (no range expansion); rejects a candidate row
   with a non-legend status; sorted + duplicate-free.
8. **Ledger + verdict**: writes `.harness/generic-c/coverage_ledger.{json,md}`
   from (matrix × fixture results) with **all three** Doc-3 §5 drift
   directions (absent-ID hard error, covered-without-green, green-without-
   covered), the `cc`-version record, and the Doc-3 §6 verdict (`out-of-scope`
   never gates; `deferred` not gated; remaining `unspecced` → `not-pass`;
   `pass` iff no `unspecced` + all `Covered` green + no FAIL). Empty corpus
   → `not-pass`.
9. **Mode-A CLI**: `--fixtures` / `--matrix` / `--ledger-out` / `--cc` /
   `--bless` with the Change-I defaults; exits 1 on `not-pass`.
10. **Per-stage tests pass** (Change J / B2): toolchain (pass/panic/
    malformed C), bridge smoke, emit-seam, manifest, matrix, ledger.
11. **Empty corpus + schema README**: no `fixtures/<name>/` dirs ship;
    `fixtures/README.md` documents the schema and states the corpus is
    empty until task e.
12. **No scope creep**: `git diff` confined to the Quality-Gate file set; no
    backend lowering, no fixtures, no doc/spec/bridge/model changes.
