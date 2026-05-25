# regseq-driver-79 — Feature-complete-gate reconciliation engine (G1)

## 1. Description

Build the **reconciliation engine** that collapses the bridge-coverage trackers
into ONE authoritative per-row truth, so we can confidently declare the bridge
**IR-structurally feature-complete** before authoring/lowering device-driver
specs. This is task **G1** of the architect-directed feature-complete gate; the
design is the **Codex-APPROVED (3 rounds)** plan
`.harness/audits/2026-05-25-feature-complete-gate-plan.md`.

Today coverage is split across three un-reconciled sources — the matrix `status`
column (hand-backfilled, systematically understates), the 146-row runner sidecar
(executable, all PASS via Mode A), and legacy `test-ir-fixtures` coverage (cited
per-row in the matrix) — plus the Section 7 validator axis (just reconciled by
driver-78). `coverage_drift.py` reports the drift read-only but does not derive a
truth, and its scan-all-cells parser mis-reads the matrix's heterogeneous tables.

G1 extends `coverage_drift.py` into a **schema-aware** engine that DERIVES each
row's coverage class from the sources while PRESERVING human adjudications, and
emits: an authoritative **coverage ledger**, a **needs-validation worklist**, a
**drift report**, and a **feature-complete verdict**. **G1 does NOT mutate the
matrix** — the schema-aware in-place de-drift write is the gated follow-up G1b.

**Scope of "feature-complete" (fixed):** IR-structural acceptance — a construct
bridges to a `RegseqModel` that passes `model.validate()`. NOT C codegen, NOT
runtime.

**Architect-approved decisions (plan Forks A/B/C):** (A) the ledger is the
authoritative artifact (no matrix write in G1; G1b does the gated write);
(B) accept legacy-fixture coverage as a **distinct `validated-legacy` tier**, and
**demote stale-legacy** (cited fixture unresolvable) to needs-validation;
(C) verdict = `needs-validation == ∅` over Sections 1-6 (runner ∪ legacy ∪
out-of-scope) ∧ Section 7 closed, **carrying the curated-enumeration caveat**.

## 2. Input Context

### Design source

- `.harness/audits/2026-05-25-feature-complete-gate-plan.md` — the Codex-APPROVED
  plan. §1 (schema-heterogeneity table + Section-6 three shapes), §2.1 (derive-
  coverage / preserve-adjudications rule), §2.2 (schema-aware legacy-evidence
  model), §2.3 (derived-truth vocabulary), §2.4 (the four outputs + verdict).

### Tool to extend (modification site)

- `scripts/python/coverage_drift.py` (175 lines) — read-only drift reporter.
  `STATUS_VALUES` closed set (`:45-55`), `ROW_ID_RE` (`:58`), `load_sidecar`
  (`:66-75`, reads `id` + `status` from `rows.toml`), `_cells` (`:80-88`),
  `parse_matrix` (`:89-113`, the fragile scan-all-cells logic to replace), the
  bucket/print `main` (`:121-172`). Pure stdlib + `tomllib`.

### Matrix schema (the parser's spec — from plan §1, verified at HEAD)

`docs/discussions/regseq-model/bridge-coverage-matrix.md`. Heterogeneous tables:
- **canonical** `id│construct│expectation│status│fixture_path│notes` (coverage in
  `status`, evidence in `fixture_path`) — e.g. `:101`.
- **sub-axis** `id│sub-construct│status│notes` (coverage in `status`, evidence in
  `notes`, no fixture_path) — e.g. `:125-130`.
- **interaction** `id│interaction│expectation│status│fixture_path / bug-task│notes`
  — e.g. `:492`.
- **Section 6 main (IRREGULAR — 3 row shapes)** declared
  `id│rejection variant│source trigger│status│notes` but: (6a) `status`=`rejected`
  + coverage in `notes` (`RJ-001..008`, `:586-595`); (6b) `status`=coverage, e.g.
  `RJ-009`=`covered` / `RJ-028,031,032`=`partial` (`:601,625,628,629`); (6c)
  six-cell `…│rejected│blocked-*│notes` (`RJ-014/017/020/021/024`,
  `:606,614,617,618,621`). **Rule: per Section-6 row, scan cells after the
  rejection-variant for the unique canonicalized coverage/adjudication token
  (covered/uncovered/partial/blocked-*, leading word of any parenthetical); treat
  a bare `rejected` as the expectation, never coverage.**
- **Section 6 sub-axis** `id│sub-construct│status│notes` (coverage in `status`) —
  `RJ-028/031/032` sub-axes (`:633-659`).
- **Section 7** `id│Variant│exercised?│notes` — a SEPARATE validator axis
  (`yes`/`partial`/`no`), reconciled by driver-78 (`:663-736`). Folded into the
  verdict as an already-closed axis; NOT a Sections-1-6 coverage row.
- Status taxonomy + `expectation_class` legend at `:8-58` (`rejected` is an
  expectation, ∉ status taxonomy). The **4** currently-unparseable Sections-1-6
  rows are `CT-002` (`:472`), `RJ-004` (`:591`), `RJ-007` (`:594`), `RJ-008`
  (`:595`) — all parenthetical status/notes qualifiers.

### Sidecar schema (the runner signal)

- `crates/bridge-coverage-runner/matrix_metadata/rows.toml` — 146 `[[row]]`
  entries; fields `id`, `section`, `construct`, `expectation`, `status`,
  `cluster_key`, `snippet_path`, and EITHER `inspect` (positive rows) OR
  `expected_rejection` (rejection rows, e.g. `:81-89`). **The per-row `status`
  string is NOT the runner signal and may itself be stale** — some committed
  sidecar rows carry `status = "uncovered"` yet are passing runner experiments
  (rejection rows in particular). **Runner-verified = the id is PRESENT in
  `rows.toml`**, full stop: all 146 committed rows PASS Mode A (146/146/0), so
  presence is the signal; Mode A is the separate gate that keeps them green.

### Legacy evidence (the legacy signal — plan §2.2)

- `crates/test-ir-fixtures/fixtures/*.rs` (no_op.rs, closure.rs, diag_logging.rs,
  trait_method_resolution.rs, trait_default_method.rs, closure_apply_indirection.rs)
  run by `crates/test-ir-fixtures/tests/*.rs` + `crates/regseq-model/tests/e2e_*`.
  Resolution helper anchors `crates/test-ir-fixtures/src/lib.rs:114,282`.
  `tests/fixtures/playground*` is experimentation, **not** evidence.

### Prior tasks

- driver-72 created `coverage_drift.py` (commit `dfa5fad`) + first surfaced the
  drift. driver-78 reconciled Section 7 (commit `5fd9e9c`).

### Python tooling convention

- Project uses uv (uv 0.9.11, `uv.lock` present, `[tool.uv] package = false`).
  Existing tools run via `uv run python scripts/python/<tool>.py` or plain
  `python3`. No Python test suite exists yet (no `test_*.py` in `scripts/python/`).

## 3. Implementation Notes

### Change A — schema-aware matrix parser (replaces `parse_matrix`)

Per-table-schema detection (route by the table's header line + section) yielding,
per row, a record: `(id, section, expectation|None, coverage_token, reason_text,
evidence_ref)`. `coverage_token` is **canonicalized** (leading taxonomy word, the
parenthetical → `reason_text`). Implements the plan §1 column map incl. the
Section-6 scan rule and the Section-6/Section-7 special cases. Section 7 rows are
parsed on the `exercised?` axis and tagged as the validator axis (not mixed into
the Sections-1-6 coverage set). **AC: 0 unparseable Sections-1-6 rows** (the 4
known ones now parse), or any residual is an explicitly-enumerated anomaly the
tool reports by id (e.g. a 6c row whose coverage token cannot be uniquely located)
— never silently dropped.

### Change B — per-row signals

- `runner`: **`id ∈ rows.toml`, INDEPENDENT of the row's `status` string** (which
  may read `"uncovered"` on rejection rows yet still be a passing experiment —
  §2). Presence is the signal because Mode A keeps all 146 green; the tool reads
  `rows.toml` and does NOT shell Mode A.
- `legacy`: resolve the row's evidence per the plan §2.2 model — **named-fixture**
  (file exists under `test-ir-fixtures/fixtures/` or the cited `tests/` surface),
  **universal** (`every fixture`/`many`/`implicit via helper` → distinct
  `validated-legacy-universal` sub-tier), **runner-snippet** (a `snippets/<id>.rs`
  ref folds into `runner`), **playground** (not evidence), **none/stale** (`--`,
  empty, or a named file that does not exist → demote, Fork B).
- `adjudicated`: `coverage_token ∈ blocked-*` OR a Section-6/`rejected` row whose
  parsed `(coverage_token=uncovered|covered, reason_text≠"")` documents why no
  fixture exists. **Detected via the parsed `(coverage_token, reason_text)` pair,
  NOT the taxonomy token alone.**

### Change C — derivation (plan §2.3 + §2.1 preserve rule)

`derived_truth(row)`:
1. if `adjudicated` → `out-of-scope` (carry `coverage_token` + `reason_text`
   verbatim). **Adjudication wins — never overwrite a `blocked-*`/reason-bearing
   row with a derived `uncovered`/`covered`.**
2. else if `runner` → `validated-runner`.
3. else if `legacy` (resolved named or universal) → `validated-legacy`
   (or `validated-legacy-universal`).
4. else → `needs-validation`.

### Change D — outputs

Default = read-only summary (counts per truth class + the drift delta, preserving
today's behavior). Plus:
- **`coverage_ledger.json` + `coverage_ledger.md`** written under
  `docs/discussions/regseq-model/` on `--write-ledger` (idempotent). Authoritative,
  keyed by id: `{id, section, truth, evidence, matrix_current_status, reason}`.
- **needs-validation worklist** — the `needs-validation` set grouped by
  section/family (each an explicit fixture-authoring candidate for F-b).
- **drift report** — rows where matrix's hand-set status ≠ derived truth
  (the existing report, now driven by the derivation).
- **feature-complete verdict** — `pass` iff `needs-validation == ∅` over
  Sections 1-6 ∧ Section 7 residual addressed-or-accepted; else `not-yet` + the
  residual list. Always prints the caveat: *coverage of enumerated rows is not a
  proof the enumeration is exhaustive over the Rust surface.*

**G1 writes ONLY the ledger artifacts — it never edits the matrix or sidecar.**

### Change E — pytest dev dependency + tests

`uv add --dev pytest` (creates `[dependency-groups] dev`, updates `uv.lock`).
Tests in `scripts/python/test_coverage_drift.py`, run via `uv run pytest`. The
tool's RUNTIME imports stay stdlib + `tomllib` only (no pytest import in the tool).

## 4. Non-Goals

- **No matrix in-place write** — that is the gated follow-up **G1b**. G1 must not
  modify `bridge-coverage-matrix.md` content at all.
- **No legacy→runner migration** (F-c), **no new fixtures / runner rows / snippets**
  (F-a/F-b), **no codegen / runtime** anything.
- **No change to the tool's runtime dependency surface** (stdlib + `tomllib`); the
  only dependency added is `pytest` in the uv **dev** group.
- No change to the sidecar schema or `rows.toml`/`snippets/` content.
- No CI wiring (Mode C is deferred Category 5).

## 5. Test Plan (`uv run pytest`, in `scripts/python/test_coverage_drift.py`)

**Parser unit tests (synthetic inline matrix tables, one per schema shape):**
- B1 canonical row → coverage from `status`, evidence from `fixture_path`.
- B2 sub-axis row → coverage from `status`, evidence from `notes`.
- B3 interaction row → coverage from `status`, evidence from `fixture_path / bug-task`.
- B4 Section-6 **6a** (`status`=`rejected`, notes=`uncovered`) → coverage
  `uncovered`, `rejected` NOT read as coverage.
- B5 Section-6 **6a parenthetical** (notes=`uncovered (no known surface)`) →
  coverage `uncovered`, `reason_text="no known surface"`.
- B6 Section-6 **6b** (`RJ-009`-like `status`=`covered`) → coverage `covered`.
- B7 Section-6 **6c** (`…│rejected│blocked-rustc-infeasible│notes`) → coverage
  `blocked-rustc-infeasible`, adjudicated.
- B8 Section-6 sub-axis → coverage from `status`.
- B9 Section-7 row (`exercised?`=`yes`) → tagged validator axis, NOT a
  Sections-1-6 coverage row.

**Derivation unit tests (synthetic rows):**
- B10 runner signal (id in sidecar) → `validated-runner`.
- B11 named-fixture legacy resolves → `validated-legacy`; universal →
  `validated-legacy-universal`; stale (missing file) → `needs-validation`.
- B11a **playground evidence is rejected**: a row whose only evidence cites
  `tests/fixtures/playground*` is NOT validated-legacy → `needs-validation` (real
  rows cite playground at matrix `:204,:207`).
- B11b **runner-snippet evidence folds into runner**: a row whose evidence is a
  `snippets/<id>.rs` ref is treated as the `runner` signal, not legacy (real rows
  cite a snippet path at matrix `:497`).
- B12 **adjudication preservation**: a `blocked-rustc-infeasible` row with no
  runner/legacy signal → `out-of-scope` (NOT `needs-validation`); reason preserved.
- B13 a row with no signal → `needs-validation`.

**Integration tests (real matrix + sidecar at HEAD):**
- B14 every Sections-1-6 row parses (0 unparseable, or only explicitly-listed
  anomalies); the 4 known unparseable rows (`CT-002`, `RJ-004`, `RJ-007`,
  `RJ-008`) now classify.
- B15 ledger row count == matrix Sections-1-6 id-row count + the Section-7 rows
  (accounted on their axis); no id appears twice; no invented id.
- B16 all 146 sidecar ids classify `validated-runner`.
- B17 known adjudications preserved: `RJ-024`=`blocked-rustc-infeasible`,
  `RJ-032-F`=`blocked-unimplemented-phase`, `OP-003-I`=`blocked-bridge-rejects-by-design`.
- B18 `--write-ledger` is idempotent (run twice → no diff).
- B19 the tool imports only stdlib + `tomllib` (no third-party runtime import).

## 6. Review

### Files to send to Codex

- `.harness/tasks/regseq-driver/79-feature-complete-gate-reconciliation.md` (this).
- `.harness/audits/2026-05-25-feature-complete-gate-plan.md` (the approved plan).
- The implemented diff (`coverage_drift.py`, `test_coverage_drift.py`,
  `pyproject.toml`/`uv.lock`, the generated `coverage_ledger.{md,json}`).

### Reviewer focus

1. Does the schema-aware parser correctly classify EACH shape (canonical /
   sub-axis / interaction / 6a / 6b / 6c / Section-6-sub-axis / Section-7), with
   `rejected` never read as coverage and parentheticals canonicalized?
2. Is the **adjudication-preservation** rule correctly implemented — no
   `blocked-*`/reason-bearing row ever derived to `needs-validation`/flipped?
3. Does the ledger account for every matrix row exactly once (Sections-1-6 on the
   coverage axis, Section-7 on the validator axis), and do the 146 sidecar ids all
   land `validated-runner`?
4. Is the matrix genuinely untouched (no content diff), and are runtime imports
   stdlib+tomllib only (pytest dev-group only)?
5. Is the verdict logic + caveat honest?

### Adversarial prompt template (four-block)

```
<task>
Review the implemented diff for regseq-driver-79 (cwd = repo root): the feature-complete-gate
reconciliation engine (G1), extending scripts/python/coverage_drift.py per the APPROVED plan
.harness/audits/2026-05-25-feature-complete-gate-plan.md. It must derive a per-row truth
(validated-runner / validated-legacy[/-universal] / out-of-scope / needs-validation) from the
matrix + 146-row sidecar + legacy fixtures, with a schema-aware parser (canonical / sub-axis /
interaction / Section-6 three shapes / Section-6-sub-axis / Section-7) that never reads `rejected`
as coverage and PRESERVES adjudications (blocked-* + reason-bearing rejected/uncovered). It writes
a coverage_ledger + worklist + verdict but must NOT modify bridge-coverage-matrix.md. Run
`uv run pytest` and `python3 scripts/python/coverage_drift.py`. Verify against the plan + the spec
Test Plan. Out of scope: the G1b matrix write; authoring fixtures.
</task>
<grounding_rules>
Cite file:line for every claim. Run the tool + tests; spot-check ledger rows against the matrix.
</grounding_rules>
<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
PARSER_ALL_SCHEMAS_CORRECT: PASS|FAIL + cite
ADJUDICATION_PRESERVED: PASS|FAIL + cite
LEDGER_ACCOUNTS_ALL_ROWS: PASS|FAIL + cite
MATRIX_UNTOUCHED_AND_RUNTIME_STDLIB: PASS|FAIL + cite
VERDICT_LOGIC_HONEST: PASS|FAIL + cite
NEW ISSUES: numbered or "none"
REQUIRED FIXES: numbered (only if VERDICT != APPROVED)
</structured_output_contract>
<default_follow_through_policy>
Do not ask clarifying questions. Read the files, decide, stop.
</default_follow_through_policy>
```

## 7. Quality Gate

- `uv run pytest` green (the new `test_coverage_drift.py` suite, B1–B19).
- `python3 scripts/python/coverage_drift.py` runs under plain `python3` (proves no
  runtime dep creep) and prints the reconciliation summary + verdict.
- `uv run python scripts/python/coverage_drift.py --write-ledger` writes
  `docs/discussions/regseq-model/coverage_ledger.{md,json}`; a second run is a no-op
  (idempotent — `git diff` empty).
- Mode A re-run `146/146/0` exit 0 (confirms the sidecar snapshot the ledger trusts
  is green at commit time).
- The tool's runtime imports are stdlib + `tomllib` only (no pytest/third-party
  import outside the test file).
- **`bridge-coverage-matrix.md` has ZERO content change** (G1 does not write it).
- Diff scope confined to: `scripts/python/coverage_drift.py`,
  `scripts/python/test_coverage_drift.py` (new), `pyproject.toml` + `uv.lock`
  (pytest dev dep), and the generated `coverage_ledger.{md,json}`. No `.rs`, no
  `rows.toml`, no `snippets/`, no matrix content, no other harness files beyond the
  tasks.json status flip.

## 8. Acceptance Criteria

1. `coverage_drift.py` parses every Sections-1-6 matrix row with a schema-aware
   parser (the 4 previously-unparseable rows now classify; any residual anomaly is
   reported by id, never silently dropped); Section 7 is parsed on its own axis.
2. Per-row derivation produces one of `validated-runner` / `validated-legacy` /
   `validated-legacy-universal` / `out-of-scope` / `needs-validation`, with
   adjudications (`blocked-*` + reason-bearing rejected/uncovered) PRESERVED — no
   adjudicated row is ever derived to `needs-validation` or flipped.
3. All 146 sidecar ids classify `validated-runner`; known adjudications preserved
   (`RJ-024`, `RJ-032-F`, `OP-003-I` as in the Test Plan).
4. `--write-ledger` emits authoritative `coverage_ledger.{md,json}` under
   `docs/discussions/regseq-model/`, keyed by id, idempotent.
5. The tool prints a **needs-validation worklist** (grouped) and a
   **feature-complete verdict** (`needs-validation == ∅` over Sections 1-6 ∧
   Section 7 closed, with the curated-enumeration caveat).
6. `uv run pytest` green (B1–B19); `python3 scripts/python/coverage_drift.py` runs
   stdlib-only; Mode A `146/146/0`.
7. `bridge-coverage-matrix.md` content is unchanged; diff confined per the Quality
   Gate (tool + test + pyproject/uv.lock + ledger artifacts).
8. `pytest` added only to the uv **dev** group; the tool's runtime deps remain
   stdlib + `tomllib`.
