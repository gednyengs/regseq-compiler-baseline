# Phase 1 · Doc 3 — Verification flow

**Status: SKELETON.** Pins the end-to-end automated verification for the
generic-C backend: the **codegen-coverage harness** — the codegen analog
of the bridge-coverage runner that closed the front-end gate. The matrix
rows from Doc 1 are this harness's fixtures. Fill after Doc 1.

**Locked direction** (from `.harness/HANDOFF.md` § Decisions locked):
the pipeline is `bridge → RegseqModel → emit C → host cc → run → assert`,
with a **dual oracle** — diag-log **golden stdout** (behavior) + **golden
C-file snapshot** (determinism/diff) — and fixtures that log *computed*
values so the print is a genuine end-to-end test of the lowering.

## Sections to fill

### 1. The pipeline
The exact stages, the boundary each asserts, and what "pass" means at
each. End state: a deterministic PASS/FAIL per fixture. *(TO FILL.)*

### 2. The dual oracle
- **Behavioral** — diag-log → `printf` → captured stdout compared to a
  golden expected string (fixtures log computed results).
- **Determinism / review** — the emitted C compared to a golden snapshot
  (stable names + decl order from Doc 2/D1 make this diffable).
Decide how the two compose into the verdict. *(TO FILL.)*

### 3. Fixture corpus
Source of fixtures (reuse `crates/test-ir-fixtures/` + bridge snippets vs.
new); the "log a computed value, assert the number" pattern; how a fixture
maps to Doc-1 matrix rows (coverage attribution). *(TO FILL.)*

### 4. The harness
Where it lives (a crate/tool, à la `bridge-coverage-runner`); how it
invokes the host C toolchain (`gcc`/`clang` — pin which + flags);
in-process vs subprocess; determinism + hermeticity. *(TO FILL.)*

### 5. The codegen-coverage ledger (docs-vs-harness split)
The **matrix** (design enumeration) lives in Doc 1 (docs). The **live
derived ledger** (per-construct pass/fail, drift, verdict — the analog of
`coverage_ledger.{json,md}`) lives in **`.harness/generic-c/`**. Pin the
ledger schema + how it's regenerated. *(TO FILL.)*

### 6. Verdict + the Phase-1 exit gate
How the harness produces the phase verdict (`pass` ⇔ every in-scope
construct covered + green), tying back to `../roadmap.md`'s per-phase exit
gate. CI integration (the codegen analog of running Mode A). *(TO FILL.)*

### 7. Toolchain + reproducibility
Pinned host compiler + flags; how golden snapshots stay stable across
machines; the build-equivalence-quad implications for emitted C. *(TO FILL.)*

## Cross-refs
- Doc 1 — the matrix rows are these fixtures.
- Doc 2 — D1 naming + D2 decl order make the C snapshot deterministic; D6
  diag-log interception produces the behavioral oracle output.
- `../roadmap.md` — the per-phase exit gate this harness enforces.
