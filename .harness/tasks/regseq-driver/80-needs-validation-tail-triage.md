# regseq-driver-80 — Triage the 47-row needs-validation tail (feature-complete gate; Type-B analytical)

## 1. Description

The feature-complete gate (G1/driver-79) produced a verdict of **`not-yet`** with
**47 Sections-1-6 `needs-validation` rows** (+ 3 Section-7 residuals handled
elsewhere). This task **triages those 47 rows** into an actionable partition so we
know exactly what stands between HEAD and an IR-structurally feature-complete
bridge — and decompose the work correctly instead of blindly authoring 47 snippets
(which would re-make the F4 escalation mistakes).

**Type-B analytical, no committed source changes.** The method is the proven
probe-first discipline (per-row HEAD-grounded probe via `try_bridge_fixture_path`,
throwaway like the R1–R5 feasibility probes) + the F5/driver-68 triage pattern.
The deliverable is a triage doc partitioning all 47 rows + a per-bucket follow-up
task plan.

**Why this matters / what the probe already hints (must be verified per row, not
trusted):** a large share of the 47 are matrix-`covered`/`partial` rows whose only
evidence is **playground** — the gate correctly demoted them (playground is
experimentation, not committed coverage), but the construct very likely already
bridges, so the fix is just to *commit a real snippet* (easy). A second cluster is
the ~12 `RJ-*` rejection rows, several of which likely have **no `#![no_std]`
source witness** and should *reclassify to adjudicated* (rustc-infeasible /
by-design), exactly as RJ-009/RJ-014 already were — shrinking the real gap. So
"47" is an upper bound; the triage's job is to find the true partition.

## 2. Input Context

### Design source

- The gate ledger: `docs/discussions/regseq-model/coverage_ledger.{json,md}`
  (authoritative; the 47 are the `truth == "needs-validation"` rows). Regenerate /
  re-read with `uv run python scripts/python/coverage_drift.py` (verdict +
  section-grouped worklist) — driver-79.
- `.harness/audits/2026-05-21-post-f4-triage.md` (driver-68) — the prior Type-B
  triage deliverable; this task mirrors its structure (per-item root cause +
  empirical evidence + disposition + follow-up).
- `.harness/audits/2026-05-25-feature-complete-gate-plan.md` — the gate plan; the
  needs-validation worklist is its F-b feeder.

### Probe surface

- `crates/test-ir-fixtures/src/lib.rs:223-224` `try_bridge_fixture_path(path)` —
  run a minimal `#![no_std]` witness through the real bridge in-process; observe
  `BridgeOutcome` (`:85-112`: Ok / ValidatorFailed / Rejected / RustcRejected) or a
  panic/ICE. `Ok` means the bridge produced a model AND `model.validate()` passed
  (`:456`); `ValidatorFailed` means it bridged but the IR was invalid (`:469`).
  Probes are THROWAWAY (never committed), per the R-series feasibility-probe
  convention + the playground rule.
- The pinned-nightly rustc source (memory [[rustup-rustc-source]]) for confirming
  whether a shape is rustc-producible from `#![no_std]` source.

### First-pass characterization (NON-EXHAUSTIVE starting hints — verify each)

**The authoritative, complete set of 47 is the live ledger's `truth ==
"needs-validation"` rows** — the implementer MUST derive it by running
`uv run python scripts/python/coverage_drift.py` and reading
`coverage_ledger.json`, NOT from this block. The groupings below are illustrative
hints for a few clusters and deliberately do NOT name all 47 (rows such as
`RV-003`, `FT-009`, `TM-006-F/G/H`, and others are live needs-validation rows
omitted here); every row in the live set must be triaged. Line refs into
`bridge-coverage-matrix.md`:

- **Likely "playground → commit" (construct already bridges; just needs a
  committed snippet):** `OP-003-C` (`:204` Const::Int), `OP-003-F` (`:207`
  Const::ZST), `FT-017` (`:459` const fn in const ctx), `FT-019` (`:461` assoc
  const), `IX-016` (`:509` const eval × const item), `IX-020` (`:513` Drop +
  assoc consts + methods), `RV-006-A` (`:259` BinOp::Add), `RV-006-I` (`:267`
  AddWithOverflow), `TM-006-B` (`:351` AssertKind::Overflow). (`TM-005-A`/`TM-005-C`
  cite "every direct fn call" — a universal phrase the evidence-classifier didn't
  match; decide commit-a-row vs a tool universal-detection refinement.)
- **Likely authorable-now (fresh `#![no_std]` snippet, should bridge Ok):**
  `TY-005` (`:107` Float f16/f32/f64/f128), `TM-005-D` (`:344` `-> !` diverging
  call), `FT-003` (`:445` nested Drop), `FT-004` (`:446` `ManuallyDrop<T>`),
  `CT-004` (`:474` edition 2024), `SK-006` (`:392` ClosureDropGlue),
  `SK-011` (`:397` VTableDropShim over `dyn`+Drop), `OP-003-G-1` (`:218`
  Byte::Uninit / MaybeUninit), `RV-007` (`:232` UnaryOp).
- **Rejection rows needing adjudicate-vs-author-vs-fix triage (outcome genuinely
  unknown per row — do NOT presume infeasible):** `RJ-001` (`:588` Box),
  `RJ-010/011/012` (`:602-604` NonRigidTy Bound/Placeholder/Infer — these MAY be
  rustc-infeasible siblings of the adjudicated RJ-009/014, but confirm via probe +
  rustc source, don't presume), `RJ-013` (`:605` NonRigidTy **Alias** — explicitly
  NOT presumed infeasible: HEAD preserves non-authorized `TyKind::Alias`
  projections as `NonRigidTy{Alias}` (`bridge/ty.rs:55,61,438`) and prior triage
  saw source-realizable Alias failures (`2026-05-21-post-f4-triage.md:63`); this is
  the **A1-residue** territory — likely Bucket B real-fix or a witness-bearing
  author case), `RJ-018` (`:615` UnsafeBinder), `RJ-023` (`:620` Layout), `RJ-025`
  (`:622` UnsupportedTy), `RJ-029` (`:626` UnsupportedConst), `RJ-030` (`:627`
  GenericOpaqueHandle).
- **Aggregate rows whose coverage is defined by sub-axes (modeling disposition,
  not a single fixture):** `RV-005` (`:230` Cast), `RV-006` (`:231` BinaryOp),
  `RV-009` (`:234` Aggregate), `ST-003` (`:312` Intrinsic), `TM-006` (`:330`
  Assert), `RJ-028` (`:625`), `RJ-031` (`:628`), `RJ-032` (`:629`).
- **Possible real-fix / const-surface candidates (R-series-style):** `OP-003-K`
  (`:212` Const::Reloc — fn-ptr literal const, R1/const-fn-ptr territory),
  `OP-003-G-3` (`:220` Byte::Reloc), `FT-021` (`:463` diag-log format reject →
  RJ-031), `SK-001` (`:387` SyntheticItemKind::None — verify why demoted).

(Counts are illustrative; the partition is the deliverable.)

### Prior art / discipline

- R1–R5 (driver-73..77) — the real-fix pattern a Bucket-B row would feed.
- driver-68 — the analytical-triage assessment template ([[assessment-soundness-pass]]
  does NOT apply: no validator change).

## 3. Method (per-row procedure)

For **each of the 47 rows** (read from the ledger's `needs-validation` set; do not
hard-code — re-derive via `coverage_drift.py` so the set is HEAD-current):

1. **Identify the construct + its intended witness** from the matrix row
   (construct, expectation_class, notes) and the spec.
2. **Probe HEAD**: write a minimal `#![no_std]` witness and run it through
   `try_bridge_fixture_path`; record the `BridgeOutcome` (or panic/ICE), verbatim.
   For a row citing playground, reconstruct the playground shape as a throwaway
   probe (do NOT depend on `tests/fixtures/playground*`).
3. **Classify into exactly one bucket:**
   - **A — author-now**: a minimal witness bridges `Ok` + `validate()` passes
     today (includes "playground → commit" rows). → feeds an F-b snippet
     sub-batch. Record the witness shape + the inspection predicate it would use.
   - **B — needs-bridge-fix**: a *realizable* witness ICEs, is *wrongly* rejected,
     OR bridges to an invalid model (`BridgeOutcome::ValidatorFailed`) — any
     genuine bridge gap on a producible shape. → feeds an R-series real-fix task.
     Record the failure (outcome/anchor) + a one-line root-cause hypothesis.
   - **C — reclassify-adjudicated**: no realizable `#![no_std]` witness
     (rustc-infeasible), or the bridge *correctly* rejects by design, or
     unimplemented-phase. → a matrix status reclassification (row leaves
     needs-validation as `blocked-*`). Record the evidence + target status class.
   - **D — aggregate/modeling**: the row is an aggregate whose coverage is defined
     by its sub-axes; decide the disposition (e.g., covered-iff-≥1-sub-axis, or a
     matrix/tool modeling note). → a modeling note, not a fixture.
4. For Bucket A/B, note whether the witness needs `BuildOverrides`
   (`overflow_checks`/`z_flags`, driver-69) or special features.

A row that resists classification after a 3-probe budget is recorded as
**needs-deeper-investigation** (its own mini-escalation), not force-fit.

## 4. Non-Goals

- **No committed source / snippets / fixtures / runner rows.** Probes are
  throwaway. Authoring the Bucket-A snippets is the follow-up F-b sub-batches;
  fixing Bucket-B is the follow-up real-fix tasks; the Bucket-C reclassifications
  are applied either by G1b (the gated matrix write) or a dedicated reclass task —
  NOT here.
- **No matrix write, no `coverage_drift.py` change, no ledger regen-commit.** (The
  tool may be RUN read-only to re-derive the set.)
- No bridge/validator/model source changes; no `task.py add-task` calls beyond
  this task's own status flip (Mode B — the partition PROPOSES the follow-up
  decomposition; the architect gates filing).
- No codegen/runtime; the verification boundary stays IR-structural.

## 5. Deliverable structure + verification

### Deliverable — `.harness/audits/2026-05-25-needs-validation-tail-triage.md`

A per-row table covering all 47 rows:

| id | section | construct | expectation | probe outcome (verbatim) | bucket (A/B/C/D/needs-deeper-investigation) | witness / root-cause / target-status | follow-up |

Plus a roll-up: counts per bucket; the **proposed follow-up decomposition** —
(A) the snippet sub-batches grouped by family/section, (B) the real-fix task
candidates with root-cause one-liners, (C) the reclassification list with target
status classes, (D) the aggregate modeling dispositions — and the **revised
feature-complete picture** (how many true author/fix items remain after C/D
shrink the 47).

### Verification (assessor)

- Every one of the 47 ledger `needs-validation` rows appears exactly once,
  classified A/B/C/D (or needs-deeper-investigation), with a verbatim probe
  outcome.
- Spot-check ≥5 probe outcomes by re-running the cited witness through
  `try_bridge_fixture_path` (trust-but-verify the empirical claims).
- Each Bucket-C reclassification cites concrete evidence (rustc source / probe)
  for "no witness" / "by-design" / "unimplemented-phase" + a valid target status
  from the closed taxonomy.
- Coherence: the roll-up's follow-up decomposition matches the per-row buckets;
  no auto-filing (`tasks.json` diff = only this task's status flip).

## 6. Review

### Files to send to Codex

- This spec + the deliverable triage doc (at assessment time) + the gate ledger +
  spot-cited matrix rows + a sample of the probe witnesses.

### Reviewer focus

1. Are all 47 ledger rows covered exactly once, each with a verbatim HEAD probe
   outcome (not a guess)?
2. Are the Bucket-B (needs-fix) and Bucket-C (reclassify) calls sound — i.e. is a
   "wrongly rejected" really realizable, and is a "rustc-infeasible" really
   un-producible from `#![no_std]` source (cite rustc)?
3. Does the follow-up decomposition correctly partition the work (snippet batches
   vs real-fix tasks vs reclassifications vs modeling), with no row mis-bucketed
   into "easy author" when it actually needs a fix?
4. No committed source / matrix write / auto-filing?

### Adversarial prompt template (four-block)

```
<task>
Assess the regseq-driver-80 deliverable (cwd = repo root): a triage of the gate's 47
needs-validation rows into author-now / needs-bridge-fix / reclassify-adjudicated /
aggregate-modeling buckets, each backed by a HEAD probe via try_bridge_fixture_path. Verify
all 47 (the truth=="needs-validation" rows from docs/discussions/regseq-model/coverage_ledger.json)
are covered once, probe outcomes are real (re-run a sample), and the bucket calls + follow-up
decomposition are sound. Out of scope: authoring snippets, fixing the bridge, writing the matrix.
You may build + run the runner / probes.
</task>
<grounding_rules>
Cite file:line for every claim. Re-run a sample of the cited probes; do not accept a probe
outcome on faith. Cite rustc source for any "rustc-infeasible" claim.
</grounding_rules>
<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
ALL_47_TRIAGED_ONCE: PASS|FAIL + cite
PROBE_OUTCOMES_REAL: PASS|FAIL + cite (name any that don't reproduce)
BUCKET_CALLS_SOUND: PASS|FAIL + cite (esp. B needs-fix + C reclassify)
DECOMPOSITION_COHERENT_NO_AUTOFILE: PASS|FAIL + cite
NEW ISSUES: numbered or "none"
REQUIRED FIXES: numbered (only if VERDICT != APPROVED)
</structured_output_contract>
<default_follow_through_policy>
Do not ask clarifying questions. Read the files, decide, stop.
</default_follow_through_policy>
```

## 7. Quality Gate

- `uv run python scripts/python/coverage_drift.py` re-run to confirm the
  `needs-validation` set is exactly the 47 triaged (set parity).
- `cargo test -p test-ir-fixtures` green (sanity; probes use the helper) — no code
  changes expected.
- Diff scope: ONLY `.harness/audits/2026-05-25-needs-validation-tail-triage.md`
  (new) + `.harness/tasks.json` (this task's status flip). No `.rs`, no matrix, no
  `coverage_drift.py`, no ledger, no snippets, no other tasks.json entries.

## 8. Acceptance Criteria

1. `.harness/audits/2026-05-25-needs-validation-tail-triage.md` exists with a
   per-row table covering ALL 47 gate `needs-validation` rows exactly once, each
   with a verbatim HEAD probe outcome and a bucket (A author-now / B needs-fix /
   C reclassify-adjudicated / D aggregate-modeling, or needs-deeper-investigation).
2. Every Bucket-A row records a concrete witness shape + intended inspection
   predicate; every Bucket-B records the failure + root-cause hypothesis; every
   Bucket-C cites evidence + a valid target status class; every Bucket-D states the
   modeling disposition.
3. A roll-up with per-bucket counts + the proposed follow-up decomposition
   (snippet sub-batches / real-fix candidates / reclassifications / modeling) + the
   revised post-triage feature-complete picture (true remaining author/fix count).
4. No committed source, no matrix write, no `coverage_drift.py`/ledger change, no
   auto-filing (tasks.json diff = only this task's status flip); probes throwaway.
5. Probe outcomes are reproducible (assessor re-runs a sample); the
   `needs-validation` set parity is confirmed against the live ledger.
