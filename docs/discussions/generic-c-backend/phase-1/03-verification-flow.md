# Phase 1 · Doc 3 — Verification flow

**Status: FILLED (2026-05-26).** Pins the end-to-end automated
verification for the generic-C backend: the **codegen-coverage harness** —
the codegen analog of the bridge-coverage runner that closed the
front-end gate. The matrix rows from Doc 1
(`01-construct-surface-and-matrix.md`) are this harness's fixtures; the
emitted-C determinism rests on Doc 2's naming (D1) + declaration order
(D2), and the behavioral oracle on Doc 2's diag-log interception (D6) +
panic-to-stderr split (D4).

**Locked direction** (from `../decisions-and-state.md` → Decisions locked): the
pipeline is `bridge → RegseqModel → emit C → host cc → run → assert`, with
a **dual oracle** — diag-log **golden stdout** (behavior) + **golden
C-file snapshot** (determinism/diff) — and fixtures that log *computed*
values so the print is a genuine end-to-end test of the lowering.

---

## 1. The pipeline

Six stages; each asserts a boundary and contributes to a single
deterministic **PASS/FAIL per fixture**.

| # | Stage | Boundary asserted | "pass" means |
| - | ----- | ----------------- | ------------ |
| 1 | **Acquire `RegseqModel`** — Rust source through the bridge (primary), or a hand-built IR fixture (`crates/test-ir-fixtures/`, supplementary) | the model validates (`RegseqModel::validate`) | a valid, post-mono model exists |
| 2 | **Emit C** — the generic-C backend lowers the model to one `.c` (Doc 2 D3 single TU) | emission completes without backend error | a `.c` string is produced |
| 3 | **Snapshot oracle** — compare emitted `.c` to the committed golden `.c` | determinism + reviewability (Doc 2 D1/D2) | byte-identical to golden |
| 4 | **Compile** — host `cc` compiles emitted `.c` + the generated `main.c` (Doc 2 D3) with the pinned flags (§7) | the emitted C is well-formed + the `_Static_assert` layout guards (D9/D10) hold | `cc` exits 0, no warnings-as-errors |
| 5 | **Run** — execute the linked binary | runtime behavior + no panic/UB-trap | process exits 0; stderr empty (no D4 panic) unless the fixture is a panic-fixture |
| 6 | **Behavioral oracle** — compare captured **stdout** to the golden expected string | the lowering computed the right values | stdout == golden stdout |

A fixture **PASSes** iff stages 1–6 all hold. Any stage failing →
**FAIL**, attributed to that stage (so a snapshot drift is
distinguishable from a behavioral miscompute or a compile error).

---

## 2. The dual oracle

Two independent oracles, both required:

- **Behavioral (stage 6).** Fixtures log *computed* values via diag-log;
  diag-log lowers to `printf` → **stdout** (Doc 2 D6). The harness captures
  stdout and compares to a golden expected string. Because the logged
  value is *computed by the lowered code* (e.g. an arithmetic result, an
  enum-match outcome, a struct-field read), a correct print proves the
  **exercised** lowering path for that fixture — the operands, rvalues,
  terminators, and layout *actually on the path to the printed value* —
  end to end, not just that `printf` fired. Whole-surface confidence comes
  from the fixture **corpus** collectively (the §3 coverage attribution),
  not from any single fixture.
- **Determinism / review (stage 3).** The emitted `.c` is compared to a
  committed golden snapshot. Stable identifiers (D1) + deterministic
  declaration order (D2) make the snapshot **byte-stable**, so a diff is a
  real change, reviewable like the bridge-era golden IR snapshots. This
  oracle also catches non-determinism (a snapshot that differs run-to-run
  fails immediately).

**Composition into the verdict.** Both must pass. The split is
diagnostic, not auto-classifying: a stage-3 (snapshot) failure is
**review-required** — the emitted C changed. A *passing* stage-6
(behavior) alongside it only proves *this fixture's exercised path* still
computes correctly, so the diff may be benign (naming/ordering) **or** a
semantic change to emitted C that this fixture does not observe (an
untaken branch, an unexercised construct). The reviewer decides; the
harness **never auto-blesses** a snapshot diff. A stage-6 failure means a
*correctness* regression (the lowering computed the wrong value); a
stage-4 failure means *invalid emitted C* (a layout-guard `_Static_assert`
or a type error). Panics land on **stderr**
(D4), so a behavioral fixture's stdout stays clean; a **panic-fixture**
asserts on stderr + non-zero exit instead (stage 5 inverts).

---

## 3. Fixture corpus

- **Sources.** *Primary:* small Rust source snippets compiled through the
  real bridge (end-to-end realism; reuse the bridge-era snippet authoring
  conventions). *Supplementary:* hand-built `RegseqModel` IR fixtures via
  `crates/test-ir-fixtures/` for constructs hard to elicit from source
  (e.g. a specific `TagEncoding::Niche` shape, a specific
  `FunctionInstanceFlavor` shim). Both feed stage 1 identically.
- **The "log a computed value" pattern.** Each fixture *computes* a result
  through the construct under test, then logs it:
  ```rust
  let x = compute_via(/* construct under test */);
  rseq::log_info!("result={}", x);   // golden stdout: "[INFO] result=<expected>\n"
  ```
  The golden stdout asserts the exact number/string, so the construct is
  genuinely exercised (not merely emitted).
- **Coverage attribution.** Each fixture declares the **Doc-1 matrix row
  IDs** it exercises (a `covers = ["TY-3", "RV-6", "BO-1", "TM-3", …]` tag
  in the fixture manifest). The harness aggregates per-row pass/fail into the
  ledger (§5), so the matrix's `covered` rows map 1:1 to green fixtures.
- **Panic fixtures.** A subset deliberately trip an `Assert` / `Unreachable`
  (D4) and assert stderr + non-zero exit, covering the AK-* / TM-4 / TM-6
  rows.
- **On-disk layout + manifest (pinned).** Each fixture is a directory
  `crates/codegen-coverage-runner/fixtures/<name>/` containing:
  - `manifest.toml` — `name` (string), `kind` (`"rust"` | `"ir"`), `entry`
    (the entry instance's `emission_name`, default `regseq_main`; Doc 2
    D3), `covers` (array of Doc-1 matrix row IDs), `expect`
    (`"pass"` | `"panic"`);
  - the input — `input.rs` (kind `rust`, compiled through the bridge) **or**
    `model.rs` (kind `ir`, builds the `RegseqModel` via
    `crates/test-ir-fixtures/`);
  - `golden.c` — the golden emitted-C snapshot (stage 3);
  - `expected.stdout` — the golden behavioral output (stage 6);
  - `expected.stderr` — present only when `expect = "panic"` (stage 5).
  The derived ledger is `.harness/generic-c/coverage_ledger.{json,md}`
  (§5). This directory + manifest schema mirrors the archived
  bridge-coverage runner's fixture conventions; `covers` is the sole
  coverage-attribution source the ledger aggregates.

---

## 4. The harness

- **Where it lives.** A new workspace crate/tool,
  `crates/codegen-coverage-runner` (the codegen analog of the archived
  `bridge-coverage-runner`; registered in `Cargo.toml`, alphabetical).
  Driven by a CLI mode like the bridge runner's "Mode A".
- **Host C toolchain.** Stages 4–5 shell out to a **subprocess** `cc`
  (pinned in §7) in a per-fixture **hermetic temp dir** (emitted `.c` +
  generated `main.c` → object → binary → run), capturing stdout/stderr +
  exit code. Subprocess (not in-process) keeps the host compiler/runtime
  isolated and the harness crash-safe.
- **Determinism + hermeticity.** No network; fixed `cc` flags; temp dirs
  cleaned per fixture; emitted C is a pure function of the model (D1/D2),
  so re-runs are byte-stable. The harness sorts fixtures + ledger rows
  deterministically.

---

## 5. The codegen-coverage ledger (docs-vs-harness split)

The **matrix** (design enumeration) lives in Doc 1 (docs). The **live
derived ledger** lives in **`.harness/generic-c/`** (the analog of the
bridge era's `coverage_ledger.{json,md}`):

- `coverage_ledger.json` — machine-readable: per matrix-row-ID
  `{ status: covered|deferred|unspecced|out-of-scope, fixtures: [...],
  result: pass|fail|n/a }`, plus a top-level `verdict`, `drift` record, and
  a `toolchain` reproducibility stamp (`cc` + version). The committed JSON is
  a **pure function of (matrix × fixture results)** — **no wall-clock /
  `last_run` timestamp** — so re-runs are byte-stable (§4) and a diff shows
  only real coverage drift (the earlier `last_run` field is dropped: a
  per-row timestamp made every run rewrite the whole ledger, defeating §4's
  byte-stability and burying drift in noise).
- `coverage_ledger.md` — human-readable rollup regenerated from the JSON.

**Regeneration.** The runner (re)writes both on each Mode-A run from
(a) Doc 1's matrix (the row set + design status) and (b) the fixture run
results. Drift between the matrix's `covered` set and the green-fixture
set is a ledger error (a row claimed `covered` in Doc 1 with no green
fixture, or vice-versa). The ledger is committed so drift shows in diffs.

---

## 6. Verdict + the Phase-1 exit gate

- **Per-fixture** verdict: PASS/FAIL from §1.
- **Phase verdict** = `pass` **iff** every Doc-1 matrix row that is
  **in-scope for Phase 1** (status `covered` — i.e. not `deferred` to a
  later phase, not `out-of-scope`) is exercised by ≥ 1 green fixture, and
  no fixture FAILs. This mirrors `../roadmap.md`'s per-phase exit gate and
  the front-end feature-complete gate.
- **`deferred`/`unspecced` rows** must be resolved (moved to `covered` via
  the Doc 2 decisions + spec delta + implementation) before the gate can
  read `pass` — the ledger surfaces any still-open row.
- **CI integration.** A CI job runs the codegen-coverage runner (the
  codegen analog of running the bridge runner's Mode A) and fails on any
  fixture FAIL or any ledger drift. Golden snapshots + golden stdout are
  committed; a CI failure is either a real regression or an un-blessed
  golden change.

---

## 7. Toolchain + reproducibility

- **Pinned host compiler + flags.** A C11 host compiler that provides the
  Doc 2 **host-target assumptions** (gcc or clang): `-std=gnu11`
  (GNU mode for the `__builtin_inf`/`__builtin_nan` float consts [D11],
  `__int128` [D11], and the fat-pointer/extension usage), **`-fwrapv`**
  (signed +,-,* wrap, D7), `-Wall`. The harness defaults to `cc`; the
  pinned version is recorded in the ledger for reproducibility.
  `_Float16`/`_Float128` (D8/D11 host-dependent) gate the fixtures that
  use them — skipped with a recorded reason when the compiler lacks them.
- **Non-flag host assumptions.** Beyond compiler flags, Doc 2 pins three
  host-target behaviors the chosen compiler must honor (recorded so the
  per-target backends know what generic-C assumed): **(a)** arithmetic
  (sign-extending) right shift for signed integers (D7 shifts);
  **(b)** function↔object pointer round-trip, i.e. `FnPtrToPtr`/
  `void*`-of-fn (D8, POSIX-guaranteed); **(c)** the float
  constant-expression builtins (D11, already supplied by `-std=gnu11`).
  gcc and clang on POSIX hosts honor all three. The harness records the
  compiler + version in the ledger; a compiler lacking any of (a)–(c) is a
  **harness-config error**, not a per-fixture skip (unlike the
  `_Float16/128` width gate above).
- **Golden stability across machines.** The **golden `.c` snapshot is
  compiler-independent** — it is *our* emitted C, a pure function of the
  model (D1 stable names + D2 deterministic order), so it is byte-stable
  across machines regardless of `cc`. The **golden stdout** is likewise
  machine-independent for the integer/bool/`&str` log values P1 exercises
  (fixtures avoid host-width-dependent prints unless the value is itself
  the point of the test).
- **Build-equivalence-quad.** The emitted C must be identical across the
  build-equivalence quad (the snapshot oracle enforces this: same model →
  same `.c`). The host-target assumptions (§D7/D8/D11) are the only
  non-ISO dependencies and are explicitly recorded so the per-target
  backends (Linux/Zephyr) know what the generic-C target assumed.

---

## Cross-refs

- **Doc 1** — the matrix rows are these fixtures (coverage attribution §3,
  ledger §5); the gate (§6) reads the matrix's in-scope `covered` set.
- **Doc 2** — D1 naming + D2 decl-order make the C snapshot deterministic
  (§2/§7); D3 single-TU + generated `main` is the compile/run unit (§1);
  D4 panic→stderr keeps the behavioral oracle clean (§2); D6 diag-log→
  stdout *is* the behavioral oracle (§2); D9/D10 `_Static_assert` guards
  are checked at stage-4 compile (§1); D7/D8/D11 host assumptions pin the
  toolchain (§7).
- `../roadmap.md` — the per-phase exit gate this harness enforces (§6).
