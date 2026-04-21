# Phase-7 Context Handoff

> **Purpose**: everything a new session needs to resume work
> without re-reading the full conversation history. Originally
> written after a working session that re-elaborated Phase-7 into a
> 15-task D' (layered) validation batch. Extended after a
> subsequent session that refined the PL011 evaluation strategy
> into a benchmark-based comparison against native drivers and
> produced a full set of strategy artifacts plus a QEMU experiment
> methodology.

---

## 0. CURRENT STATE banner (2026-04-20, session 2)

**If you read only one section, read this one.**

### 0.1 What's current

The PL011 evaluation strategy has evolved beyond the "authoring-
first" pivot captured in Section 5 below. The paper thesis is now:

> Regseq's semantic driver model expresses a single ARM PrimeCell
> PL011 specification, and its compiler generates Linux and Zephyr
> drivers that match native-driver behavior within predefined
> tolerance rules on QEMU-relative functional and performance
> workloads derived from concrete PL011 system use cases.

Key shifts from the Section 5 framing:

1. **The hand-authored PL011 reference driver is now ONLY a
   compiler-determinism oracle** ("we match ourselves"), never a
   driver-quality oracle. Driver quality comes from benchmarks vs
   native drivers in QEMU.
2. **Paper claim is full F+I+P** (functional + integration +
   performance including DMA). Internal F+I-then-P staging is a
   compiler-dev tactic, invisible to the paper.
3. **Comparisons are QEMU-relative only**. Absolute-physical
   claims are deferred and recorded as future-work requirements.
4. **Comparison axes are derived from concrete PL011 system-use
   cases** (shell tty, system console output, inter-device link,
   firmware update channel, sensor data ingestion, etc.), not
   from corpus convergence or TRM inventory. Anti-gaming
   discipline.
5. **Four-OS corpus exists at `reference-drivers/pl011/`**: Linux,
   Zephyr, FreeBSD, Windows 10 IoT Core. Windows was NOT present
   when the Section 5 framing was written.

### 0.2 New strategy artifact package (under
`docs/discussions/pl011-full-spec/`)

All UNCOMMITTED in the working tree. Accepted by Codex across
two adversarial debates (strategy: 4 rounds converged;
methodology: 4 rounds converged).

**What we compare** (comparison axes + workloads + requirements):

- `pl011-comparison-axes.md` (NEW, LOAD-BEARING) -- system-use
  cases + F/P/I axes + dropped-axes ledger. The anti-gaming
  artifact.
- `pl011-workloads.md` (NEW) -- per-axis workload shapes
  (1 stimulus, 1 observable, 1 environment, 1 verdict rule) +
  tolerance rules + statistical contract.
- `pl011-spec-requirements.md` (NEW) -- traced requirements
  (each tagged axis / correctness / backend-contract).
  Deferred items flagged.
- `linux-driver-analysis-primecell.md` (NEW) -- PrimeCell-
  filtered derivative of the full Codex-approved Linux
  inventory (strips `vendor_st`, `vendor_sbsa`,
  `vendor_qdt_qdf2400_e44` material).

**How we run comparisons in QEMU** (methodology):

- `pl011-experiment-methodology.md` (NEW) -- operational bridge
  from abstract workloads to concrete QEMU execution. Defines:
  - Per-workload execution plan with 8-category status taxonomy
  - Four QEMU patches required (error+OE injection, modem+auto-
    flow, IFLS semantics, QMP register introspection)
  - Single-clock host-only timing protocol (no cross-clock
    guest-host sync)
  - Zephyr control-channel proof gate
  - Topology-fairness DTB pair (identical topology, binding
    differs only)
  - Pilot + real-campaign checkpoint discipline
  - Full reproducibility contract
  - Non-DMA core + DMA addendum parallelizable campaigns

**Amended** (reframing headers added; historical content
preserved):

- `pl011-unified-spec-boundary.md` -- new authoritative artifacts
  cited; legacy "at least 1 corpus driver" inclusion rule
  deprecated in favor of axis-derived requirements.
- `pl011-full-spec-methodology.md` -- corpus role inverted
  (baseline + policy source, not inclusion gate).
- `pl011-full-spec-status.md` -- status mapped against
  requirements rather than legacy roadmap items.

### 0.3 Immediate next activity (when resuming)

**DO NOT** go straight to co-authoring the PL011 Linux reference
driver. The next steps are different from Section 7 below.

1. **Review + commit the strategy artifact package** (8 files,
   all uncommitted). The current handoff doc + the strategy
   artifacts should land together.
2. **Parallelizable prep tracks**:
   (a) Implement the four QEMU patches per
       `pl011-experiment-methodology.md` Section 3.1-3.4. Each
       ~30-150 LoC. Ship with qtest coverage.
   (b) Build the topology-fairness DTB pair (`pl011-regseq.dtb`
       + `pl011-native.dtb`) per Section 5.5 of the same doc.
   (c) Prove the Zephyr auxiliary control channel per
       Section 5.1 of the same doc (semihosting, aux serial,
       or modified DT). If none works, Zephyr falls to F+I
       only for the paper.
   (d) Complete DMA in `driver.rs` (the pending item in
       `pl011-full-spec-status.md`).
3. **Pilot run** (native drivers only, full suite) per
   `pl011-experiment-methodology.md` Section 6. Finalizes the
   PROVISIONAL tolerance constants.
4. **Pre-campaign gate**: tolerance constants promoted from
   PROVISIONAL to pilot-derived.
5. **Real campaign** (Regseq-vs-native, interleaved batches,
   per-batch sentinel rerun, variance / host-drift stop-gates).
6. **Co-author `pl011-reference-linux-driver.c`** per the
   original Phase-7 plan; this is still needed as the compiler-
   determinism oracle, but not as driver-quality evidence.
7. **Report** and write the paper.

Tracks 2(a), 2(b), 2(c), and 2(d) can proceed in parallel. Pilot
gates the real campaign.

### 0.4 Codex adversarial debate pattern (used throughout)

This session used Codex multi-round adversarial debates
heavily:

- **Round 1**: submit artifact with structured questions asking
  for substantive challenges.
- **Responses**: accept / veto each challenge with reasoning.
  Claude retains veto power; Codex has no authority over scope.
- **Round N+1**: audit the fixes from round N and raise any
  remaining substantive issues.
- **Convergence**: Codex signals `converged` when it has no
  more substantive challenges.

Pattern worked very well. Typical convergence in 4 rounds.
Codex-runner pattern: `cat prompt.txt | uv run python
scripts/python/codex_runner.py --timeout 540 --message-only`.
Background execution via `run_in_background: true`. Response
files under `/tmp/pl011-analysis/` during this session (not
persistent across sessions).

### 0.5 What NOT to re-decide (settled across 8 Codex rounds
in session 2)

- Thesis wording (section 0.1 above).
- F+I-then-P is implementation-only, NOT paper staging.
- Comparisons are QEMU-relative only.
- Hand-authored reference is compiler-oracle only.
- FreeBSD and Windows are corpus-only, not backends.
- PrimeCell scope pin (no ST / SBSA / QDF2400 / vendor variants).
- Four-layer framing (L1 hardware / L2 policy / L3 platform /
  L4 OS realization) as the secondary organization; F/P/I is
  the paper's primary partition.
- Single-clock host-only timing; RX uses a completion-bracket
  protocol via an auxiliary control channel.
- Primary register assertion path = QMP `query-pl011-state`
  (Patch 3.4); debugfs/shell readback = secondary cross-check.
- Tolerance constants are PROVISIONAL until pilot data.
- No `pl011-get-rx-count` QMP command (dropped on Codex advice).
- Dropping axes is fine; keep them in the ledger with rationale.

---

## 1. Project at a glance

Regseq is a Rust-to-C device-driver compiler. Driver authors write
OS-agnostic driver specifications using the Regseq DSL (embedded in
Rust); the compiler generates target-OS-specific C code (Linux
first; Zephyr + others later).

Currently mid-refactor: replacing three separate internal models
(`DeclarationModel` + `BindingModel` + `JoinedArtifact` + expression-
tree `SemanticModel` + `Item` hierarchy) with a **unified
`RegseqModel`** that has CFG-based IR, typed flat `DeclId`s, and a
clean `RegseqType` system. Authoritative target design lives in
`docs/discussions/models/regseq-model/{spec.md, ir-reference.md,
compiler-knowledge.md, execution-plan.md}`.

**Phase 0-6 done**: RegseqModel types, normalize(), populate_bindings(),
verify_phase_1/2 all landed. PL011 Phase-6 pipeline golden at
`tests/fixtures/pl011-full-spec/expected/regseq-model.json`.

**Phase 7 now elaborated** into a 15-task batch. That's where we are.

---

## 2. Where we are right now (the critical state)

- Branch: `main`, 11 commits ahead of `origin/main`. Nothing pushed yet.
- `regseqmodel-phase-7` tracker status: **tasked** with 15
  `generated_task_ids`.
- `task.py next` reports `regseq-model-61`, but that is NOT what
  comes next in practice.
- **Actual next activity**: co-authored work on task
  `regseq-backends-02` -- specifically, the hand-authored PL011
  Linux reference driver at
  `tests/fixtures/pl011-full-spec/expected/pl011-reference-linux-driver.c`
  (not yet created). See Section 7 below.

Why the disconnect? Task `regseq-backends-02` is marked
"co-authored by user + Claude, not implementer-delegatable"
(`.harness/tasks/regseq-backends/02-emission-contract-and-exemplars.md:4-8`).
`task.py next` doesn't know this nuance -- it points at the
numerically-first ready task. When spec-authoring is gated on
human pairing, `task.py next` is advisory only.

---

## 3. Read-order for a cold session

Don't try to read everything. Read in this order and stop when
you have enough context to pair:

1. **This document** (you're here).
2. `docs/discussions/models/regseq-model/migration.md`
   -- Phase-7 design (Q0-Q8). Authoritative.
   -- Q5 is the validation strategy (D' layered stack).
   -- Q6 is the migration order.
   -- Q7 + Q8 justify the IR layering and C-AST decoupling.
3. `docs/discussions/models/regseq-model/execution-plan.md`
   -- Phase 7/8/9 boundaries were redrawn:
      - Phase 7C absorbed former 8.1 (goto-based CFG emission).
      - Phase 7F absorbed former 9.1 (old-model deletion).
      - Phase 8 now starts at 8.2 (dominator cleanup).
      - Phase 9 now starts at 9.2 (golden regen).
4. `.harness/tasks/regseq-backends/02-emission-contract-and-exemplars.md`
   -- the task the next session will actually pair on. Read cover
      to cover.
5. `docs/discussions/models/regseq-model/compiler-knowledge.md`
   (Sections 2 "Abstraction Types" and "Resource Cleanup on Error
   Paths" and "OrderedRegion: SESE Invariant and CFG Modes") --
   the raw material for the emission contract. The hand-authored
   reference will apply these lowering tables in concrete C.
6. `tests/fixtures/pl011-full-spec/src/driver.rs` -- the PL011
   source spec. The hand-authored C reference is its Linux-C
   translation.
7. `tests/fixtures/pl011-full-spec/expected/regseq-model.json` --
   the Phase-6 IR golden for PL011. Source of truth for the
   RegseqModel the emitter will walk.

Optional (for specific questions):
- `.harness/SESSION.md` -- Codex review invocation contract.
- `.harness/task-spec-workflow.md` -- required spec sections.
- `.harness/archive/regseqmodel-phase-7-original/` -- the retired
  Phase-7 specs (compat-adapter framing). Kept for historical
  reference; do not reintroduce.

---

## 4. The 15-task Phase-7 batch (at a glance)

| # | Task ID | Phase | Scope (one-liner) | Dep |
|---|---------|-------|---------------|-----|
| 1 | `regseq-backends-02` | 7.0 | Emission contract + exemplars + hand-authored PL011 reference | `regseq-model-60` |
| 2 | `regseq-backends-03` | 7.0 | C-AST move to `backends::c` + Phase-7C subset expansion | `regseq-model-60` |
| 3 | `track-a-validation-11` | 7.0 | QEMU boot+probe harness (productize frozen eval baseline) | `regseq-model-60` |
| 4 | `regseq-model-61` | 7A | `regseq-model::query` CFG traversal helpers | `regseq-model-60` |
| 5 | `regseq-model-62` | 7A | `LintFinding` API + fold property lints into `verify_phase_1` | `regseq-model-60` |
| 6 | `regseq-analysis-06` | 7A | Rewrite `readiness` over RegseqModel; delete `input`/`body_readiness` | 4, 5 |
| 7 | `regseq-lowering-10` | 7B | New `build_driver_lowering_plan_from_model` + trim DriverLoweringPlan | 4 |
| 8 | `regseq-linux-backend-03` | 7B | LinuxBackendPlan self-sufficiency | 7 |
| 9 | `regseq-linux-backend-04` | 7C | Retarget `emit_semantic` to CFG + new C AST | 1, 2, 8 |
| 10 | `regseq-linux-backend-05` | 7C | Retarget `emit_module` to CFG; delete `emit_behavior`; byte-equal to reference | 1, 8, 9 |
| 11 | `track-a-validation-12` | 7C-validation | Verify emitter output byte-equals pre-authored reference; run QEMU smoke | 3, 10 |
| 12 | `regseq-driver-35` | 7D | `emit_driver.rs` swaps to new pipeline | 11 |
| 13 | `track-a-validation-13` | 7E | Rewrite PL011 bus-identity E2E; delete lowering-input-stability; add Test B (CFG invariants) | 12 |
| 14 | `regseq-lowering-11` | 7F | Delete dead lowering submodules + legacy entry | 13 |
| 15 | `regseq-model-63` | 7F | Delete old model modules; final Phase-7 tracker gate | 14 |

Parallel within Phase 7.0: tasks 1, 2, 3, 4, 5 can all progress concurrently.

---

## 5. The critical design pivot: authoring-first validation

The batch went through MANY iterations. The final shape is a
significant departure from what most people first assume. If you
only absorb one thing from this doc, make it this.

### What was originally planned (retired)

"Run the new emitter on PL011, QEMU-test the output, and if the
smoke passes, commit the emitter's output as the 'blessed
reference' for byte-equal regression."

### Why that was wrong

1. The old compiler pipeline's C output is **neither complete nor
   correct** (user's explicit statement; see migration.md Q5).
   Byte-parity against it would lock in bugs.
2. Blessing unknown emitter output after a 1-smoke pass is
   build-blind: the implementer produces *something* that satisfies
   construct tests + QEMU boot, we lock it in, and structural
   composition errors surface only at review -- too late.
3. Mature compilers (LLVM, MLIR, Cranelift, rustc) do NOT validate
   new backends by blessing legacy output. They "specify lowering,
   assert structural legality, then run programs."

### What we do instead (authoring-first)

1. **User + Claude jointly hand-author** the full PL011 Linux
   reference at
   `tests/fixtures/pl011-full-spec/expected/pl011-reference-linux-driver.c`
   BEFORE any implementer touches the emitter. Authored in the
   emission contract's "normal form" -- one C function per
   `RegseqModel::Function`, one label per `BasicBlock`, canonical
   gotos, canonical cleanup ladders, no inlining.
2. Validate the reference via host-build (`.ko` against pinned
   kernel) + QEMU smoke (probe + tty open + byte write + close +
   unload without warnings). This is the first real-hardware-ish
   signal that the reference is correct.
3. THEN hand Phase-7A/B/C/D/E/F to an implementer, whose job is
   now simple: "produce byte-equal output to the reference."
4. Task `track-a-validation-12` is no longer "bless emitter
   output"; it's "verify emitter converges on the pre-authored
   reference."

### Why this generalizes

The emission contract + reference authoring approach is per-OS.
When Zephyr lands, we co-author a second
`zephyr-emission-contract.md` and second
`pl011-reference-zephyr-driver.c` using the SAME process. The
implementer work generalizes across drivers (any authored driver
RegseqModel + matching Linux reference). Per-OS contracts keep
Linux idioms out of the core.

---

## 6. What's NOT being done (do-not list)

Things earlier plans considered and rejected; do not re-propose:

- **No `compat.rs` adapter**. The original Phase-7 framing tried
  to project RegseqModel back to the legacy `DeclarationModel` /
  `JoinedArtifact` shape. Failed because RegseqModel doesn't
  carry every family the old shape exposed, and because the old
  shape was declared insufficient. The specs at
  `.harness/archive/regseqmodel-phase-7-original/` document this
  retirement; do not recreate.
- **No emitter-authors-the-reference flow**. See Section 5.
- **No modifications to `crates/regseq-driver/src/construction*`**.
  Phase-5 construction is frozen for Phase-7. Every spec has this
  as an explicit non-goal.
- **No hand-authoring the PL011 reference in idiomatic Linux
  style**. The reference uses the contract's normal form
  (mechanical compiler-output shape). Stylistic drift between
  reference and emitter is the #1 defect source; joint authoring
  in normal form prevents it.
- **No "blessing" against legacy output**. The oracle is the
  hand-authored reference, not any legacy C.
- **No implementer work on task 1**. Task 1 is co-authored
  prerequisite work.
- **No `task.py start regseq-model-61` before task 1 is done**.
  Even though `task.py next` points there, the real critical
  path starts with the co-authored task 1 reference.

---

## 7. The immediate next activity (where to pick up)

**SUPERSEDED by Section 0.3.** The co-authoring of the PL011
reference driver is NO LONGER the next step; prep tracks for the
QEMU experiment campaign come first. The reference driver is
still needed, but its role has narrowed to compiler-determinism
oracle only.

Historical content (describing the previously-planned
co-authoring activity) preserved below for context:

We were about to **start co-authoring the PL011 reference driver**.
The file does NOT exist yet; the user deleted their earlier draft
at `docs/discussions/pl011-full-spec/pl011-linux.c` (committed
git-wise never happened -- it was in the working tree and then
removed).

The last message from Claude proposed two starting orders:

- **Top-down**: draft the emission contract's "normal form"
  section as tight markdown (~1-2 pages), then walk the PL011
  RegseqModel.
- **Bottom-up**: pick one PL011 function (suggested `tx_byte` --
  small, register-touching, shows goto + error_target + MMIO
  intrinsic), hand-translate it, and let the normal form
  crystallize bottom-up.

Claude recommended **bottom-up** (contracts written in a vacuum
tend to miss things you only see when you put pen to paper).

The user's answer to that question was never resolved because the
strategy evolved in session 2; bottom-up remains a reasonable
default when the reference-driver activity is picked up later.

Regardless of the order, the immediate collaborative activity is:

```
1. Open tests/fixtures/pl011-full-spec/src/driver.rs
   (the PL011 source spec).
2. Open tests/fixtures/pl011-full-spec/expected/regseq-model.json
   (the Phase-6 IR).
3. Start writing
   tests/fixtures/pl011-full-spec/expected/pl011-reference-linux-driver.c
   function-by-function, applying the emission contract rules.
4. As contract gaps surface, either
   (a) pause and add the rule to linux-emission-contract.md,
       or
   (b) note it inline in the .c file with `// CONTRACT-GAP: ...`
       and keep going; revisit in batch.
5. Periodically check syntax with `cc -fsyntax-only`.
6. When a meaningful chunk (probe + one callback + teardown)
   compiles, try the host-build-proof path.
7. Once everything compiles, get QEMU smoke working (requires
   task `track-a-validation-11` done or a temporary bypass).
8. Commit the reference + contract + tables + exemplars as
   one coordinated commit when all three validations pass.
```

---

## 8. Key file paths (cheat sheet)

### PL011 strategy artifact package (session 2; UNCOMMITTED)

Primary docs (new):

- `docs/discussions/pl011-full-spec/pl011-comparison-axes.md` --
  LOAD-BEARING. What we compare.
- `docs/discussions/pl011-full-spec/pl011-workloads.md` -- how
  each axis is exercised.
- `docs/discussions/pl011-full-spec/pl011-spec-requirements.md`
  -- the definitive spec requirements list.
- `docs/discussions/pl011-full-spec/linux-driver-analysis-primecell.md`
  -- PrimeCell-filtered derivative.
- `docs/discussions/pl011-full-spec/pl011-experiment-methodology.md`
  -- QEMU experiment methodology bridge.

Amended (reframing headers added):

- `docs/discussions/pl011-full-spec/pl011-unified-spec-boundary.md`
- `docs/discussions/pl011-full-spec/pl011-full-spec-methodology.md`
- `docs/discussions/pl011-full-spec/pl011-full-spec-status.md`

Corpus (four-OS, fresh this session):

- `reference-drivers/pl011/README.md` -- index.
- `reference-drivers/pl011/linux/amba-pl011.c`
- `reference-drivers/pl011/linux/serial_pl011.h`
- `reference-drivers/pl011/zephyr/uart_pl011.c`
- `reference-drivers/pl011/zephyr/uart_pl011_registers.h`
- `reference-drivers/pl011/zephyr/arm_pl011.yaml`
- `reference-drivers/pl011/freebsd/uart_dev_pl011.c`
- `reference-drivers/pl011/windows/` -- 27 files (SerCx2 WDF
  driver for RPi 2/3, MIT).

### Target artifacts (future; when the reference driver activity resumes)

- `tests/fixtures/pl011-full-spec/expected/pl011-reference-linux-driver.c`
  -- hand-authored PL011 Linux reference. **Does not exist yet.**
  Compiler-determinism oracle only (not driver-quality oracle).
- `docs/discussions/models/regseq-model/linux-emission-contract.md`
  -- emission contract markdown. Does not exist yet.
- `crates/regseq-backends/src/linux/emission_tables.rs` -- Rust
  data tables. Does not exist yet.
- `tests/goldens/pl011/linux/constructs/*.c` -- ~15 exemplar
  fragments. Directory does not exist yet.

### QEMU experiment artifacts (to produce per experiment-methodology.md)

- `experiments/pl011-eval-env/qemu-patches/*.patch` -- the four
  QEMU PL011 patches (error+OE injection, modem+auto-flow,
  IFLS semantics, QMP register introspection).
- `experiments/pl011-eval-env/dtbs/pl011-regseq.dtb` and
  `pl011-native.dtb` -- topology-fairness DTB pair.
- `experiments/zephyr-pl011-eval/` -- Zephyr sample app +
  harness (pending control-channel proof).

### Source material (exists)

- `tests/fixtures/pl011-full-spec/src/driver.rs` -- PL011 DSL
  spec.
- `tests/fixtures/pl011-full-spec/expected/regseq-model.json` --
  Phase-6 RegseqModel golden.
- `tests/fixtures/pl011-full-spec/expected/raw-frontend.json` --
  Phase-4 frontend artifact.
- `docs/discussions/models/regseq-model/compiler-knowledge.md`
  Section "Resource Cleanup on Error Paths" (lines ~1033-1333),
  Section "OrderedRegion: SESE Invariant and CFG Modes" (lines
  ~836-1032), Section 2 "Abstraction Types" (lines ~450-820) --
  the raw lowering intent.

### Infrastructure (exists; to be productized)

- `experiments/pl011-eval-env/notes/` -- QEMU eval environment
  research notes. Frozen baseline: pinned ARM64 `v6.18.21` on
  `virt`, Buildroot 2026.02 + BusyBox + kmod rootfs, stock PL011
  on `ttyAMA0` console, second AMBA PL011 for OOT module
  binding.
- `experiments/pl011-eval-env/artifacts/regseq-pl011-amba-probe/`
  -- existing known-good OOT probe module; task 3 uses it as
  harness self-test.
- `tests/e2e/tests/pl011_linux_module_host_build_proof.rs` --
  existing host-build-proof test. Pattern for `#[ignore]`'d
  environmental tests; task 11 and task 12 extend it.

### Retired / archived

- `.harness/archive/regseqmodel-phase-7-original/` -- the six
  original Phase-7 specs, retired when the compat.rs-adapter
  framing failed. Read for historical context; do not
  reintroduce.
- `.harness/archive/regseqmodel-phase-7-original/README.md` --
  explains why they were retired.

---

## 9. Harness / workflow conventions

- **Never modify `.harness/tasks.json` directly for execution**
  (status changes). Use `task.py start`, `task.py done`,
  `task.py fail`. Spec-authoring updates (adding new tasks) DO
  edit tasks.json directly -- that's a workflow-step-12 concern,
  not a status concern.
- **Codex review**: always via `scripts/python/codex_runner.py`
  invoked through the Bash tool. Never use `codex:codex-rescue`
  agent / `/codex:rescue` skill (documented to hang). Pattern:
  ```bash
  cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540 --message-only
  <task>
  ...prompt...
  </task>
  <grounding_rules>Cite file:line for every claim.</grounding_rules>
  <structured_output_contract>...</structured_output_contract>
  EOF
  ```
- **Background long-running tasks** via `run_in_background: true`
  on Bash. Monitor completion via the `Monitor` tool's
  until-loop pattern. Monitor timeouts happen and are fine --
  the completion notification fires independently.
- **Adversarial review loop** before committing any design-level
  change. Codex has returned `REQUIRES FIXES` many times in this
  project; the fixes are always small + sharp. Don't skip the
  loop.
- **CLAUDE.md rules** (global + project):
  - No unicode symbols in Markdown (use `[x]` / `[ ]` / `NOTE:`
    / `TODO:` / `WARN:`).
  - No author attributions in commit messages.
  - Never proactively create docs unless explicitly requested.
  - Prefer clarity over cleverness.

---

## 10. Recent commit history

### Session 2 (2026-04-20 continued)

**All session-2 strategy work is UNCOMMITTED.** The working tree
contains:

- 5 new docs under `docs/discussions/pl011-full-spec/`
- 3 amendments to existing docs in the same directory
- 1 handoff-doc update (this file)
- Pre-existing uncommitted work from sessions prior to session 1
  (Phase-7 work in `crates/regseq-model/`, etc.)
- `reference-drivers/pl011/` directory exists (added by the user
  before session 2 began)

The user will decide whether to commit as one cluster or split
into multiple commits.

### Session 1 (original handoff)

```
1089768 docs: add Phase-7 context handoff for cross-session continuity
5b23c6f regseqmodel-phase-7: flip validation from blessing-first to authoring-first
a26eefb regseqmodel-phase-7: author 15-task D' validation spec batch
4c67e6f regseqmodel-phase-7: retire original compat-adapter task specs
8197b09 docs: align migration.md + execution-plan.md with Phase-7 D' validation
f3664c4 docs: regseq-model Phase-7 redesign migration design doc
```

### Pre-session (already landed)

```
d366c00 regseqmodel-phase-7: author consumer-migration task specs
  (the retired original 6-task batch)
c00ef9c regseq-model-60: PL011 Phase-6 end-to-end pipeline + JSON golden
  (the Phase-6 gate; tests/fixtures/.../regseq-model.json lives here)
```

---

## 11. Codex review history (useful context for future reviews)

### Session 1: Phase-7 re-elaboration (4 rounds)

1. **Decomposition round 1**: 5 MANDATORY fixes on the
   proposed structure (`emit_driver` last not first;
   `DriverLoweringPlan` field trim; etc.).
2. **Spec-batch round 1** (15 files): 7 MANDATORY fixes
   including verifier return-shape ambiguity, caller-inventory
   contradictions, body-access contract mismatch, QEMU pass
   criterion under-specified.
3. **Spec-batch round 2** (post-fix verification): 2 residual
   prompt-wording inconsistencies in review blocks.
4. **Spec-batch round 3** (tiebreaker): `APPROVED`.

The spec batch is **considered implementer-ready** by Codex at
its final review.

### Session 2: PL011 work (3 adversarial debate streams, 14 rounds total)

Stream 1: **Linux driver feature inventory** (6 rounds to
APPROVED). Produced
`docs/discussions/pl011-full-spec/linux-driver-analysis.md` (20
sections covering the full Linux PL011 driver: device binding,
MMIO, interrupts, FIFO, termios, modem/flow, DMA, clock/pinctrl,
console, PM, RS-485, vendor variants, tty/serial_core, locking,
resources, module metadata, lifecycle, policy, mask-policy
cross-cutting, vendor-dispatch cross-cutting).

Stream 2: **PL011 spec strategy** (4 rounds to SOUND with narrow
fix, then locked after researcher clarifications). Produced the
thesis in Section 0.1, the F/P/I taxonomy, the outcome-oriented
comparison-axes approach, the QEMU-relative-only constraint,
and the eight-artifact package.

Stream 3: **Strategy artifact adversarial debate** (4 rounds to
converged). Landed 13 mandatory fixes across axes.md,
workloads.md, requirements.md, and linux-driver-analysis-
primecell.md. Convergence at round 4.

Stream 4: **QEMU experiment methodology debate** (4 rounds to
converged). Landed major fixes around QEMU PL011 fidelity gaps
(overrun, IFLS, baud, modem lines), QMP device selector shape,
single-clock host-only timing protocol with RX completion
bracket, topology-fairness DTB pair, pilot-plus-sentinel
discipline, and full reproducibility contract. Convergence at
round 4.

All response transcripts saved under `/tmp/pl011-analysis/`
during session 2 but those files don't persist across sessions.
Regenerate with the same prompt pattern if needed.

### Codex-runner pattern

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540 --message-only
<task>
...prompt...
</task>
<grounding_rules>Cite file:line for every claim.</grounding_rules>
<structured_output_contract>...</structured_output_contract>
EOF
```

Background via Bash tool's `run_in_background: true`; monitor
completion via task notification. Do NOT use `codex:codex-rescue`
agent or `/codex:rescue` skill (documented to hang).

---

## 12. Verifying state (commands to run)

```bash
# Is the harness happy?
uv run python scripts/python/task.py doctor

# What does the harness think comes next?
uv run python scripts/python/task.py next

# Confirm tracker state.
grep 'status:' .harness/task-spec-status.yaml | head -20

# Confirm all 15 Phase-7 tasks are registered.
jq '.groups[].tasks[] | select(.id | test("-(06|10|11|12|13|35|61|62|63)$") or (. == "regseq-backends-02") or (. == "regseq-backends-03") or (. == "regseq-linux-backend-03") or (. == "regseq-linux-backend-04") or (. == "regseq-linux-backend-05"))' .harness/tasks.json | jq -s 'length'
# Expected: >= 15 (may return more if IDs match other groups).

# What's in the working tree?
git status --short
git log --oneline -10
```

---

## 13. Open micro-decisions to elicit in the next session

### Session-2 work produced these open decisions (priority order)

1. **Commit cluster plan**: the session-2 strategy package is
   uncommitted (see Section 10). Does the user want one
   cluster commit, or split by theme (axes/workloads/requirements
   together; amendments separately; methodology separately)?
2. **Parallel track allocation**: of the four prep tracks
   (QEMU patches, DTB pair, Zephyr control channel, DMA in
   driver.rs), which does the user want started first?
3. **QEMU patch implementer**: will the user implement the four
   QEMU patches personally, delegate to an implementer, or have
   Claude produce a first draft for review?
4. **Pilot run timing**: should the pilot run be scheduled after
   all four QEMU patches land, or iteratively per-patch (run
   whatever pilot workloads each patch unlocks as it lands)?
5. **Physical PL011 platform**: the experiment methodology
   references physical hardware for absolute claims. What exact
   PL011 platform is available? This informs deferred-requirement
   scheduling.

### Session-1 open decisions (DEFERRED until the reference driver
activity resumes; see Section 7 note)

1. Top-down vs bottom-up authoring order. Claude recommended
   bottom-up (start with `tx_byte`). User's preference not yet
   confirmed.
2. Contract document authoring cadence (as-we-go vs batch).
3. Data tables in parallel or last.
4. First function to translate (if bottom-up): `tx_byte`
   recommended; `tx_ready` / `rx_char` also small.

---

## 14. User preferences established this session

- Prefers adversarial Codex review loops for substantial design
  work.
- Uses "ultrathink" keyword when they want Claude to reason
  especially carefully on hard design questions.
- Decisions captured in migration.md "Open decisions" section
  get concrete answers; trust the doc's resolutions.
- No author attributions in commit messages (global CLAUDE.md
  rule).
- Reviewed + accepted the authoring-first flip (previous user
  message: "I want option 1").
- Prefers explicit file paths for artifacts
  (`tests/fixtures/pl011-full-spec/expected/pl011-reference-linux-driver.c`
  is user-specified).
- Co-authored activities on critical infrastructure (emission
  contract + reference) are user + Claude pair work, not
  delegated to an LLM implementer.

---

## 15. Quick-orient summary for the next session

If the next session has 5 minutes to orient:

- **Read Section 0** first. That's the current state.
- Phase 7 is elaborated into 15 tasks (committed in session 1).
- PL011 evaluation strategy has converged in session 2 on a
  **benchmark-based comparison against native drivers** thesis.
  The hand-authored reference driver is NOT the paper's main
  oracle; benchmarks in QEMU are.
- Eight strategy docs + the handoff exist in the working tree
  UNCOMMITTED. Review + commit is probably step 1.
- The immediate next step is NOT reference-driver authoring.
  It's the four parallel prep tracks in Section 0.3:
  (a) four QEMU patches, (b) topology-fairness DTBs,
  (c) Zephyr control-channel proof, (d) DMA in `driver.rs`.
- Then pilot run (native-only) -> tolerance finalization ->
  real campaign (Regseq vs native).
- Hand-authored reference driver activity resumes AFTER the
  campaign is in motion; it serves as compiler-determinism
  oracle, not driver-quality oracle.
- Do NOT: resurrect `compat.rs`; use the reference as
  driver-quality oracle; claim byte-parity to anything as
  evidence of driver quality; use `codex:codex-rescue` agent.
- Four-OS corpus lives at `reference-drivers/pl011/`.
- Codex adversarial debate pattern works well; use
  `scripts/python/codex_runner.py` with
  `run_in_background: true` and structured prompts.
