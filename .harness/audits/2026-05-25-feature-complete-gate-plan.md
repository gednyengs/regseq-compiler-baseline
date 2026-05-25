# Plan — Tracker + matrix reconciliation: the bridge-feature-complete gate

Plan-first design doc (pre-spec). Grounded at HEAD (`5204a53`). Proposes the
decomposition + resolves the design forks for the architect-directed gate
([[bridge-feature-complete-gate]]): collapse the coverage trackers into ONE
authoritative per-row truth (*validated* / *needs-validation* /
*out-of-scope-with-reason*), **derived not hand-set**, so we can confidently
declare the bridge **IR-structurally feature-complete** before authoring/
lowering device-driver specs.

**Scope of "feature-complete" (fixed up front):** the verification boundary is
**IR-structural acceptance** — a construct bridges to a `RegseqModel` that
passes `model.validate()`. NOT C codegen, NOT runtime. The gate certifies "the
bridge represents the enumerated Rust surface as well-formed IR," which is what
kills *bridge* surprises during spec authoring; codegen/lowering surprises are a
separate, later gate.

## 1. HEAD-grounded state (what the reconciliation must consume)

**Three coverage sources, not auto-reconciled** (per `coverage_drift.py`
docstring + a fresh run at HEAD):

- **Runner sidecar** — `crates/bridge-coverage-runner/matrix_metadata/rows.toml`
  (146 rows) + `snippets/`. Every committed row is an executable experiment that
  PASSes Mode A. Highest-confidence signal. Keyed by matrix row id.
- **Matrix `status` column** — `bridge-coverage-matrix.md`, hand-backfilled per
  task. Known to systematically *understate* runner coverage (the F4 bulk corpus
  added sidecar rows without backfilling). NOT reliable as-is.
- **Legacy `test-ir-fixtures` coverage** — the matrix `fixture_path` column cites
  the covering fixture/test for legacy-covered rows. 94 rows are matrix-`covered`
  but absent from the sidecar ("matrix-only").

Plus the **Section 7 validator axis** (61 `InvariantViolation` rows) — a *separate*
coverage axis (constructed-model tests, `yes`/`partial`/`no`), just reconciled by
driver-78. Not part of the Sections-1-6 bridge-row reconciliation; folded into the
gate's final verdict as an already-closed axis.

**Drift-tool numbers at HEAD** (`coverage_drift.py --all`, re-run + Codex-verified):
`sidecar=146`, `matrix parsed=299 (63 unparseable)`, `matrix covered=120`,
`consistent (sidecar∩covered)=26`, `DRIFT (sidecar, matrix≠covered)=120`
(0 absent from matrix), `matrix-only covered=94`. Union covered = 146+120−26 ≈
**240** / ~301 Sections-1-6 rows.

**The "63 unparseable" decomposes** (verified): **59 are Section 7 VL rows**
(they use a `yes`/`no` *exercised?* column, not the covered/uncovered taxonomy →
0 status cells → reported unparseable; expected, different axis). Two VL rows
(`VL-041`, `VL-045`) DO parse because their exercised value happens to be
`partial`, which is also in the status taxonomy (matrix `:732,736`; parser exact
match `coverage_drift.py:89`). That leaves **exactly 4 real Sections-1-6 rows**:
`CT-002`, `RJ-004`, `RJ-007`, `RJ-008`. Their problem is **parenthetical/notes-
embedded coverage qualifiers** the tool's exact-match rejects:
`covered (rejection-path covered)`, `uncovered (no known surface…)`,
`covered (implicitly via helper…)` (matrix `:472,591,594,595`). There is no large
mystery set — but resolving them is NOT just "strip the parenthetical": see the
schema heterogeneity below.

**Matrix schema is HETEROGENEOUS — the load-bearing finding for parser + writer
design.** The legend (`:8-58`) describes a *canonical* row schema
`id | construct | expectation | status | fixture_path | notes`
(`expectation ∈ {positive, rejected, validator-failing}`; `status` = the closed
taxonomy `{covered, partial, uncovered, blocked-*}`). But the tables do NOT all
use it (verified at HEAD):

| schema | columns | where coverage lives | where evidence lives | example |
| ------ | ------- | -------------------- | -------------------- | ------- |
| **canonical** | `id│construct│expectation│status│fixture_path│notes` | `status` col | `fixture_path` col | §1.1 `TY-001` (`:101`) |
| **sub-axis** | `id│sub-construct│status│notes` | `status` col | `notes` col (no fixture_path) | `TY-003-C` → notes `instance_completeness.rs (Point.x: i32)` (`:125-130`) |
| **interaction** | `id│interaction│expectation│status│fixture_path / bug-task│notes` | `status` col | `fixture_path / bug-task` (dual: fixture OR bug-task) | `IX-001` (`:492`) |
| **Section 6 main (IRREGULAR — 3 shapes under one header)** | declared `id│rejection variant│source trigger│status│notes` | **varies per row** (see below) | `notes` col | §6.1–6.3 (`:586-629`) |
| **Section 6 sub-axis** | `id│sub-construct│status│notes` | `status` col | `notes` col | RJ-028/031/032 sub-axes (`:633-659`) |

**Section 6 main tables are the genuine trap — one header, three row shapes**
(verified `:586-629`):
- *(6a) status=expectation, coverage in notes* — the bulk: `status` cell literally
  holds `rejected` (the expectation, ∉ coverage taxonomy), and covered/uncovered is
  in `notes` (`RJ-001` notes=`uncovered`; `RJ-008` notes=`covered (implicitly…)`;
  `RJ-004/007` notes=`uncovered (reason…)`).
- *(6b) status=coverage* — `RJ-009` `status`=`covered` (reframed to a positive row;
  `:599-601`), coverage in the declared `status` cell, NOT `rejected`.
- *(6c) 6-cell, adjudication in an extra cell* — `RJ-014/017/020/021/024` are
  `…│rejected│blocked-rustc-infeasible│notes` (six cells against the 5-col header;
  `:606,614,617,618,621`): `status`=`rejected`, the adjudication token sits in an
  EXTRA cell before notes.

So Section 6 main has **no fixed coverage column**. The robust rule (and what
mostly works today): per Section-6 row, scan cells *after* the `rejection variant`
for the unique **canonicalized coverage/adjudication token** (covered/uncovered/
partial/blocked-*, taking the leading word of any parenthetical); treat a bare
`rejected` as the expectation, never as coverage. That single rule covers 6a/6b/6c
and resolves the unparseable `RJ-004/007/008` (parenthetical notes). `rejected` ∉
coverage taxonomy (verified `:12` vs `:19`). **35 Sections-1-6 table-header lines**
exist (`grep "^| *id "` minus the Section-7 header `:690`); the parser AND any
writer must be
**per-table-schema aware** — know, per table, which column carries coverage and
which carries evidence.

**Legacy coverage IS partially machine-derivable** (verified by sampling the
`fixture_path` column): rows split three ways —
- **named-fixture** — cites a concrete file, e.g. `BD-001 → no_op.rs`
  (`:426`). Cross-checkable: does `crates/test-ir-fixtures/fixtures/<f>.rs` exist
  AND get run by a passing test? (`fixtures/` holds no_op.rs, closure.rs,
  diag_logging.rs, trait_method_resolution.rs, trait_default_method.rs,
  closure_apply_indirection.rs; run by `tests/no_op_smoke.rs` etc. + the
  `regseq-model/tests/e2e_*` suite.)
- **universal** — `every fixture` / `every fixture via helper
  (test-ir-fixtures/src/lib.rs:188-198)` (`CT-001 :471`, `OP-001 :194`). Covered
  by construction as long as ANY fixture runs (e.g. `panic=abort`,
  `Operand::Copy`). True but not row-specific.
- **none** — `--` (`TY-001 :103`, status `uncovered`).

## 2. The reconciliation design

### 2.1 Derive coverage; PRESERVE adjudications (the load-bearing split)

The per-row truth has two kinds of input that must NOT be conflated:

- **Coverage (machine-derivable)** — `covered`/`partial`/`uncovered` is a
  function of the three sources: is there a passing sidecar row? a resolvable
  legacy fixture? neither? The tool DERIVES this.
- **Out-of-scope adjudication (human/Codex judgment, NOT derivable)** — the
  `rejected`-with-no-surface and every `blocked-*` class encode a *reason* a row
  is legitimately not covered (rustc-infeasible, rejects-by-design,
  unimplemented-phase, …). These were established by prior audits (C1/C2/B1/B2/F1
  reclassifications, Section 6). The tool must **carry these through unchanged** —
  it must never overwrite a `blocked-*`/adjudicated row with a derived
  `uncovered`, or it would erase hard-won human determinations.

So: **derived_status = adjudication if the row carries one, else derived coverage
from the sources.** This is the rule that makes "derive, don't hand-backfill"
safe — we derive the part that's mechanical and preserve the part that's judgment.

**Adjudication detection is NOT recoverable from the status taxonomy alone**
(Codex caught this). `blocked-*` classes are taxonomy-detectable, but a
`rejected`-with-no-known-surface adjudication is encoded as free text in the
*notes* column — `RJ-004` notes `uncovered (no known surface from #![no_std]
source)`, `RJ-007` notes `uncovered (requires coroutine source — blocked by
RJ-016)` (`:591,594`). So the schema-aware parser must extract, per row, an
explicit **`(coverage_token, reason_text)` pair** from the schema's coverage cell
(stripping any parenthetical into `reason_text`), and a row counts as
*adjudicated* iff `status ∈ blocked-*` **OR** it is a Section-6 / `rejected` row
whose parsed coverage is `uncovered`/`covered` carrying a non-empty reason that
documents why no fixture exists. The writer (§2.4 / Fork A) must preserve both the
coverage token AND the reason text for these rows verbatim — never collapse
`uncovered (no known surface)` to a bare derived `uncovered`, and never touch a
Section-6 row's `rejected` expectation cell.

### 2.2 Per-row signals (computed by the extended tool)

For each Sections-1-6 matrix row id:
1. `runner` — id ∈ sidecar AND that sidecar row passes Mode A. (Mode A already
   gates all 146; the tool reads the sidecar + trusts the green Mode A run, or
   optionally shells Mode A.)
2. `legacy` — a **schema-aware evidence model** resolves the row's evidence cell
   (`fixture_path` for canonical/interaction; `notes` for sub-axis/Section 6).
   Evidence forms, each with a resolution rule:
   - **named fixture** — cites a file (e.g. `no_op.rs`, or a notes citation like
     `instance_completeness.rs (Point.x: i32)`); resolve = the file exists under
     `crates/test-ir-fixtures/fixtures/` (or the named `regseq-model/tests/` /
     `regseq-driver/tests/` surface) AND is compiled+run by a passing test.
   - **universal** — `every fixture` / `every fixture via helper
     (test-ir-fixtures/src/lib.rs:188-198)` / `many` → covered by construction as
     long as any fixture runs (record as a distinct `validated-legacy-universal`
     sub-tier per Fork B, since it is true but not row-specific).
   - **implicit** — `covered (implicitly via helper…)` (e.g. `RJ-008`, panic=abort
     enforced for all) → universal-equivalent.
   - **runner snippet** — a `snippets/<id>.rs` path → this is really the `runner`
     signal, not legacy; fold into (1).
   - **playground** — any citation pointing at `tests/fixtures/playground*` is
     NOT load-bearing evidence (experimentation space, never committed coverage);
     treat as unresolved.
   - **none / stale** — `--`, empty, or a named file that no longer exists →
     **stale-legacy**, drops to `needs-validation` (overstated `covered`).
3. `adjudicated` — status ∈ {`blocked-*`} or a `rejected` row with an explicit
   "no surface" reason + a reason string.

### 2.3 Derived per-row truth (the gate's vocabulary)

| truth | rule | confidence |
| ----- | ---- | ---------- |
| **validated-runner** | `runner` | executable, highest |
| **validated-legacy** | `legacy` (named-resolved or universal) ∧ ¬`runner` | passing test, medium |
| **out-of-scope** | `adjudicated` (carry the reason verbatim) | human judgment |
| **needs-validation** | none of the above | the worklist |

### 2.4 Outputs

1. **Authoritative derived ledger** — per row: id, expectation_class, derived
   truth, evidence (sidecar id / fixture path+test / adjudication reason),
   and the matrix's current hand-set status (for the drift delta).
2. **needs-validation worklist** — the genuinely-uncovered tail, grouped by
   section/family, each an explicit fixture-authoring candidate. THIS is the
   residual that must reach ∅ (or be explicitly accepted) for the gate to pass.
3. **Drift report** — rows where hand-set matrix status ≠ derived truth (today
   ≥120). Surfaces both understatement (sidecar-covered, matrix says uncovered)
   and any overstatement (matrix says covered, no resolvable evidence —
   stale-legacy).
4. **Feature-completeness verdict** — bridge is IR-structurally feature-complete
   iff `needs-validation == ∅` over Sections 1-6 (every row validated-runner ∪
   validated-legacy ∪ out-of-scope), AND the Section 7 axis is closed. Section 7
   per driver-78 is **56 `yes` / 2 `partial` (VL-041, VL-045) / 3 `no`**, where the
   coverage-axis 3 `no` triage-splits into **1 fixture-bound** (VL-021) **+ 2
   unreachable backstops** (VL-018/019); the known Section-7 residual is therefore
   the **3-test batch** = VL-021 + the two `partial` completions (F-a). The verdict
   report states pass/not-yet + the residual list, and carries the §6.5 caveat
   (enumerated coverage ≠ exhaustiveness proof).

## 3. Design forks (resolved with a recommendation; Codex to pressure-test)

**Fork A — where the derived truth lives.** (Reworked after Codex: the matrix's
schema heterogeneity (§1) makes a blind in-place `status`-column rewrite unsafe —
it would corrupt Section 6, whose `status` IS the `rejected` expectation, and miss
sub-axis tables where coverage is in `status` but evidence is in `notes` with no
`fixture_path`.)
- A1 (blind in-place `status` rewrite) — **rejected**: not schema-safe.
- A2: the tool emits a **schema-agnostic authoritative derived ledger** keyed by
  row id (`coverage_ledger.{md,json}`) — the single machine truth; the matrix
  stays human-owned prose.
- A1′: a **schema-aware** in-place regen that, per table schema (§1 column map),
  writes ONLY the identified coverage token: for canonical/sub-axis/interaction,
  the `status` cell; **for Section 6 main (no fixed coverage column), in-situ
  replacement of the row's existing canonicalized coverage/adjudication token
  wherever it sits (notes / status=`covered` RJ-009 / the 6c extra cell)** — never
  the `rejected` expectation token, never an adjudication's reason text. Rows where
  no coverage token can be located (or the 6c six-cell anomaly) are flagged for
  manual review rather than rewritten. Gated on the ledger as source. (If this
  proves too fragile, the fallback is a one-time manual Section-6 normalization to
  a regular schema before any Section-6 write; the ledger covers Section 6
  regardless.)
- A3: invert (sidecar+fixtures become source, matrix fully generated) — too large,
  loses curated prose.
- **Recommended: A2 as the authoritative truth (primary, low-risk), with A1′ as a
  gated, schema-aware second step to stop the matrix lying.** The ledger gives
  "know exactly what's validated" immediately and unambiguously without touching
  the heterogeneous tables; A1′ then cleans up the matrix's visible drift but only
  via the per-schema column map + §2.1's preserve rule, so Section 6 / adjudication
  cells are untouchable. **A1′ should be split into its own sub-task (G1b)** so the
  risky write is reviewed separately from the (safe) ledger/worklist/verdict (§4).
  The tool keeps today's read-only report mode + adds the ledger emitter; the A1′
  `--write` is opt-in and lands only after G1's ledger is accepted.

**Fork B — how much to trust legacy coverage for the gate.**
- B1: accept `validated-legacy` as "validated" for the gate (don't require runner
  migration).
- B2: down-rank legacy to needs-validation (require a runner row for every
  "validated" claim → ~94-row migration before the gate can pass).
- **Recommended: B1, with a distinct `validated-legacy` tier kept visible.** The
  legacy fixtures are real passing tests; they DO validate the row. Forcing 94
  runner rows is large effort that doesn't change the feature-completeness truth.
  Keep the tier explicit so confidence is honest; optionally schedule migration as
  a low-priority follow-up. (Stale-legacy rows — `fixture_path` names a missing
  file — are the exception: those drop to needs-validation.)

**Fork C — gate verdict criterion.**
- **Recommended:** feature-complete ⇔ `needs-validation == ∅` over Sections 1-6
  (runner ∪ legacy ∪ out-of-scope) ∧ Section 7 residual addressed-or-accepted.
  If non-empty, the worklist items become fixture tasks; the gate "passes" only
  once they're covered or the architect explicitly accepts a documented residual.

## 4. Proposed decomposition (Mode B — architect approval gates filing)

- **Task G1 (the gate; Type-B tooling+analytical).** Extend `coverage_drift.py`
  into the reconciliation engine: a **per-table-schema-aware parser** (the §1
  column map — canonical / sub-axis / interaction / Section-6; extract
  `(coverage_token, reason_text)` per row; fix the 4 unparseable Sections-1-6
  rows; treat Section 7 as its own axis), the §2.2 schema-aware signals, the §2.3
  derivation with the §2.1 preserve rule (adjudication = `blocked-*` OR
  reason-bearing `rejected`/`uncovered`), and the four §2.4 outputs (ledger +
  worklist + drift report + verdict). Deliverable: the tool (read-only + ledger
  emitter) + the authoritative `coverage_ledger.{md,json}` + the needs-validation
  worklist + the verdict report. **Tool runtime deps stay stdlib + `tomllib`**
  (plain-`python3` invocable, like the existing tool). **Tests use `pytest`,
  added as a uv DEV dependency** (`uv add --dev pytest`; `[dependency-groups] dev`
  + `uv.lock`), run via `uv run pytest` — this is the repo's first Python test
  suite, and a dev/test dep is distinct from the tool's runtime deps (the project
  uses uv; uv 0.9.11 + `uv.lock` present). **No matrix in-place write, no
  legacy→runner migration, no new fixtures, no codegen.**
- **Task G1b (matrix de-drift; Type-B tooling, gated on G1).** Add the
  schema-aware A1′ `--write` mode: regenerate ONLY each row's coverage cell per its
  table schema (never the `expectation` cell, never adjudication reason text),
  idempotent, with a re-derive-and-diff check. Split out so the only
  matrix-mutating step is reviewed on its own.
- **Follow-up F-a (small, already specced by driver-78 §3):** the 3 constructed-
  model validator tests (VL-021 + VL-041/VL-045 partials) to close the Section 7
  residual.
- **Follow-up F-b (sized by G1's worklist):** author runner rows / fixtures for
  the genuinely-uncovered Sections-1-6 tail. Decomposed by family from G1's output.
- **Follow-up F-c (optional, Fork B):** migrate `validated-legacy` rows to runner
  rows for uniform highest-confidence coverage. Low priority; does not block the
  gate.

## 5. Non-goals (for G1)

- No C codegen / runtime validation (out of the verification boundary).
- No device-driver spec authoring/lowering (that's what the gate UNBLOCKS).
- No authoring of fixtures/runner rows or migrating legacy rows in G1 (those are
  F-a/F-b/F-c).
- No hand-editing of matrix status cells; no matrix mutation at all in G1 (the
  schema-aware coverage-cell write is G1b, gated on G1's ledger). Humans own the
  adjudication classes + prose permanently.
- No new runner-crate dependencies; no change to the sidecar schema.

## 6. Risks / open questions

**Designed-for after Codex round 1** (no longer open; recorded for the spec):
- *Adjudication preservation* — resolved by §2.1: adjudication = `blocked-*` OR a
  reason-bearing `rejected`/`uncovered` parsed from the schema's coverage cell;
  the writer preserves `(coverage_token, reason_text)` verbatim and never touches
  Section-6 `expectation` cells. (Taxonomy alone is NOT sufficient — that was the
  hole.)
- *Section-schema variance* — resolved by the §1 per-table column map; the parser
  is schema-aware, NOT a position-scan and NOT a scan-all-cells heuristic.
- *In-place write safety* — resolved by splitting the schema-aware write into G1b
  (Fork A1′), gated on G1's ledger; G1 itself never mutates the matrix.

**Still genuinely open for review / the architect:**
1. **Legacy `universal` coverage granularity.** "every fixture" / "many" /
   "implicit via helper" is true but coarse. The plan records it as a distinct
   `validated-legacy-universal` sub-tier (§2.2) — is that honest enough for the
   gate, or should universal-only rows be treated as `needs-validation` (pushing
   toward a row-specific runner row)?
2. **Stale-legacy detection reliability.** Can the tool resolve every legacy
   evidence form (named fixture / notes citation / `tests/` path / snippet) to a
   real passing test, or are some ticks pointing at surfaces it can't mechanically
   resolve (→ false stale-legacy demotions)? May need a small allow-list of
   recognized evidence forms.
3. **Verdict honesty.** Does `needs-validation == ∅` over runner∪legacy∪
   out-of-scope justify "IR-structurally feature-complete," given the matrix is a
   *curated enumeration* (coverage of enumerated rows ≠ proof the enumeration is
   exhaustive over the Rust surface)? The verdict carries this caveat explicitly
   (§2.4 pt 4) — is that the right disposition, or does the gate also need a
   separate enumeration-completeness check?
