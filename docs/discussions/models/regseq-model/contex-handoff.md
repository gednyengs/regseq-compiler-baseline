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

## 0. CURRENT STATE banner (2026-04-20 session 2, layered through 2026-04-23 session 4)

**If you read only one section, read this one.**

**Session-4 deltas (2026-04-22 + 2026-04-23)** layered on top
of the session-2 banner below: user-data initialization Path A
done (`regseq-model-64`); shadow-register design complete but
deferred; four PL011 fixture issues discovered that block
golden regeneration; a bundled `pl011-fixture-cleanup`
tracker item authored at
`.harness/task-spec-status.yaml`. Full session-4 context in
§10; active-next-step adjusted in §0.3. The session-2 framing
below (strategy artifacts, 3-stage plan, QEMU-relative
comparison) remains authoritative.

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
   performance including DMA). Internal staging is the 3-stage
   progressive plan (Stage 1 paper-min core, Stage 2 flow
   control + error injection, Stage 3 performance extensions);
   see §0.3. Each stage is paper-capable at its closure; the
   final paper reports the cumulative F+I+P result.
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

All COMMITTED (three thematic commits on main, 2026-04-20).
Accepted by Codex across two adversarial debates (strategy:
4 rounds converged; methodology: 4 rounds converged). Further
doc-level edits have landed since (3-stage tagging, cheap-fix
audit) directly in these artifacts.

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

**Stage 1 audit provenance** (NEW 2026-04-21):

- `audit-history/` -- Codex transcripts for the three-round
  Stage 1 fixture audit loop. Each round's output preserved in
  the repo rather than in ephemeral `/tmp/`. Future audit loops
  (Stage 2, Stage 3, re-audits) should follow the same
  preserve-in-repo pattern. See `audit-history/README.md` for
  round summaries.

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

**3-stage progressive implementation plan** (agreed 2026-04-21).

The single-stage and two-thread framings from earlier drafts of
this document are superseded. Work progresses in three stages;
each stage produces a paper-capable deliverable.

Stage membership for every REQ-* is authoritatively defined in
`docs/discussions/pl011-full-spec/pl011-spec-requirements.md`
Section 1.2 (stage-membership summary). Implementation state
(which REQs are currently captured in the live fixture) is
tracked in `pl011-full-spec-status.md`.

**Stage 1 -- Paper-min core** (ACTIVE; no QEMU patches required)

Deliverable: compiler pipe-cleaned end-to-end; hand-authored
Linux golden reference driver byte-equal to emitter output;
Zephyr `uart_driver_api` backend emission; paper-min workload
campaign results for F1/F2, F6-01/02, F7, F8, F10 (non-CRTSCTS
portion), F3/F4 via driver-shadow readback, P1/P2/P3/P4/P5/P6,
IL1-5, IZ1-4 (gated on control-channel proof).

Scheduled work (parallelizable where dependencies allow):

1. **Fixture extensions** (Rust-only; no QEMU) -- **[DONE
   2026-04-21]** per the three-round Codex audit loop
   (`pl011-full-spec/audit-history/stage-1-round-*`). Final
   verdict `stage-1-complete`. All items landed:
   - REQ-CFG-01 divisor now uses round-to-nearest per datasheet
     §3.3.6 Example 3-1 (earlier integer truncation was
     off-by-one in the fractional divisor).
   - REQ-CFG-03/04 real `SerialConfig`-driven line format with
     WLEN/parity/stop encoding; validation split into pure
     `line_control_value` + `write_line_control` so rejected
     configs cannot leave the port disabled mid-sequence.
   - REQ-IRQ-03 / REQ-CORR-MASK-SHADOW IMSC software shadow with
     full-mask write-through.
   - REQ-IRQ-04 named-source helpers including
     `enable_error_interrupts` / `disable_error_interrupts`.
   - REQ-LIFE-01/02 `wait_for_tx_complete` helper inserted
     before every LCR_H/IBRD/FBRD reprogram; disable-then-wait
     ordering in every lifecycle transition.
   - REQ-RES-CLK-01 const fallback replaced by
     `uart-clock-hz` `#[rseq::property]`.
   - REQ-RES-PIN-01 default pins in `port_open`, sleep pins in
     `port_close` (previously only init/deinit).
   - REQ-PERF-BATCH-01 `pending_tx` removed; `tx_byte` is now an
     unchecked primitive matching Linux `pl011_tx_char`.
   - REQ-BRK-01 `send_break` doc comment establishing the
     pure-toggle caller-owned-duration contract per Linux
     `pl011_break_ctl` / Windows `PL011HwSetBreak`.
   - REQ-BC-LIFECYCLE module-level doc block above
     `impl Lifecycle` stating compiler-acquired vs body-operated
     resources, lifecycle-ordering guarantees, panic contract.
   - REQ-CORR-RESERVED-01 captured by framework
     (`write_field`/`modify` distinction in
     `call_lowering.rs:47-65`).

   Lifecycle decomposition adopted (Linux-shaped): init does
   reset + IRQ mask/clear only; port_open does pinctrl + clock +
   UART bring-up; port_close does teardown + clock off + sleep
   pins; configure preserves the IMSC shadow. This shape lets
   the Linux backend emit 1:1 to `probe`/`startup`/`shutdown`/
   `remove`/`set_termios`, while the Zephyr backend concatenates
   init+port_open at `device_init` for its single-lifecycle
   model.

2. **Harness work** (parallel to fixture; no QEMU) -- [PENDING]:
   - Zephyr control-channel proof per
     `pl011-experiment-methodology.md` Section 5.1 (semihosting /
     aux serial / modified DT). Binary gate for Zephyr rows.
   - Host control-channel for RX completion bracket per Section
     4.2 (needed for W-P2-01, W-P3-01, W-P6-01).
   - Topology-fairness DTB pair (`pl011-regseq.dtb` +
     `pl011-native.dtb`) per Section 5.5.
3. **User-data initialization Path A** (compiler rule
   extension; `regseq-model-64`) -- **[DONE 2026-04-22]**.
   Extended the compiler's user-data acceptance rule to admit
   either `Option<T: Copy>` OR a compiler-controlled zero-init
   whitelist of primitive scalars + arrays + `Copy` structs of
   these. Unblocked the fixture's `imsc_shadow: u32` bare-u32
   user-data. Design doc at
   `docs/discussions/models/regseq-model/user-data-initialization.md`;
   task marked done with **Option A disposition** (see
   §10 Session 4 for the full disposition context).

4. **PL011 fixture cleanup** (drift + 4 issues + goldens) --
   **[ACTIVE NEXT STEP]**. Bundled cleanup covering:
   - Stage-1 Codex-audit rename drift in ~9 test files'
     hardcoded method-name lists (`program_polling_port` ->
     `bring_up_port`/`reprogram_line_format`;
     `configure_line_control` -> `line_control_value` +
     `write_line_control`; `pending_tx: Option<u8>` removal).
   - Four compiler-pass-skip fixture issues at
     `tests/fixtures/pl011-full-spec/src/driver.rs` lines
     148, 190, 334, 529; enumerated authoritatively in
     `docs/discussions/pl011-full-spec/pl011-full-spec-status.md`
     "Known fixture issues (discovered 2026-04-23)".
   - Golden regeneration (`raw-frontend.json`,
     `regseq-model.json`, `ir/*.json`) after the fixes land.

   Tracker item `pl011-fixture-cleanup` is at
   `.harness/task-spec-status.yaml`, status
   `ready_to_elaborate`; brief stub at
   `.harness/spec-batches/pl011-fixture-cleanup.md`. Task
   spec not yet authored. This task blocks items 5-8 below.

5. **Co-author the Linux golden reference** -- [PENDING]:
   `tests/fixtures/pl011-full-spec/expected/pl011-reference-linux-driver.c`
   in the emission contract's "normal form". Co-authored by
   user + Claude; not implementer-delegatable. Blocked on
   item 4 (needs a fixture that compiles through the pipeline
   cleanly to produce meaningful reference output).
6. **Co-author the emission contract** -- [PENDING]:
   `docs/discussions/models/regseq-model/linux-emission-contract.md`
   crystallized alongside the reference. Blocked on item 5.
7. **Emission data tables + exemplar fragments** -- [PENDING]:
   `crates/regseq-backends/src/linux/emission_tables.rs`,
   `tests/goldens/pl011/linux/constructs/*.c`. Blocked on
   item 6.
8. **Phase 7A-F emitter retarget** -- [PENDING]: per Section 4
   task grid. Deliverable: byte-equal emitter output to the
   reference. The harness task `regseq-backends-02` (at
   `.harness/tasks/regseq-backends/02-emission-contract-and-exemplars.md`)
   depends on `regseq-model-64` (done) plus other blockers;
   add `pl011-fixture-cleanup`'s task id to this list when
   that task is authored.
9. **Pilot run** -- [PENDING]: native drivers only, full
   Stage-1 suite. Finalizes PROVISIONAL tolerance constants
   for Stage-1 rows. Blocked on item 8 + item 2.
10. **Stage-1 real campaign** -- [PENDING]: Regseq-vs-native,
    interleaved batches, per-batch sentinel rerun. Closes
    Stage 1. Blocked on item 9.

**When resuming (session 5)**: item 4 (PL011 fixture cleanup)
is the active next step. Item 2 (harness work) can proceed in
parallel since it does not touch the fixture or the emitter.
Item 4 unblocks items 5-10. Item 3 (Path A) is done; do not
re-open it.

**Important discrepancy -- harness vs. session-documented
sequencing**: `uv run python scripts/python/task.py next-step`
currently returns `regseq-backends-02` (Linux emission
contract) as the next executable task, because the harness
only sees the explicit `depends_on` graph and every declared
dependency of `regseq-backends-02` is done. **Do not pick up
`regseq-backends-02` first.** The fixture drift + four fixture
issues block its verification; the worker will hit the same
cargo-test-workspace failures the Path A implementer did. The
correct session-5 sequence is:

1. Author the `pl011-fixture-cleanup` task spec per its brief
   at `.harness/spec-batches/pl011-fixture-cleanup.md`.
2. Add the newly-authored cleanup task's id to
   `regseq-backends-02`'s `depends_on` array in `tasks.json`
   (per the cleanup brief's Update Instructions).
3. Execute `pl011-fixture-cleanup` (fix the 4 fixture issues,
   update ~9 test files, regenerate goldens).
4. Then `regseq-backends-02` becomes executable with a clean
   workspace.

The task-spec-authoring workflow at
`.harness/task-spec-workflow.md` handles step 1; the normal
executable-task workflow handles step 3 and 4.

**Parallel design work delivered in session 4 but not
implemented**: the shadow-register DSL extension is fully
designed and Codex-reviewed at
`docs/discussions/models/regseq-model/shadow-registers.md`.
Status is **deferred**; revisit post-pipe-clean when a working
Rust-to-C driver baseline is generated. No tracker item yet
(intentional).

**Stage 2 -- Flow control + error injection + independent register
observation** (queued; requires QEMU patches)

Deliverable: paper rows F5, F6-03, F9, F10 (CRTSCTS portion);
F3/F4 rows strengthened from driver-readback to QMP-based
independent observation.

Scheduled work:

1. **QEMU patches** per `pl011-experiment-methodology.md`
   Sections 3.1, 3.2, 3.4 (~30-300 LoC combined; ship with qtest).
2. **Fixture extensions**: REQ-FLOW-01, REQ-FLOW-02, REQ-LIFE-03
   exercise path.
3. **Linux reference driver extensions**: `set_termios` CRTSCTS
   branch, `throttle` / `unthrottle`, auto-RTS-disable-before-drop
   ordering. Apply as diff on top of Stage-1 reference.
4. **Stage-2 pilot** (incremental, only new rows) and campaign.

**Stage 3 -- Performance extensions** (queued; requires QEMU
patch 3.3 + DMA fixture work)

Deliverable: paper rows P7, P1/P2/P5-DMA-01.

Scheduled work:

1. **QEMU patch 3.3** (IFLS threshold semantics; ~40-60 LoC).
2. **Fixture additions**: REQ-PERF-IFLS-01 (runtime IFLS
   selection), REQ-PERF-DMA-01/02 (DMACR + PIO-DMA threshold
   policy).
3. **Reference-driver DMA lowering** (Linux quantitative; Zephyr
   as feature-delta observation only).
4. **Stage-3 pilot** (incremental) and campaign.

**Stage membership authority**: stage assignments for every
REQ-* live in `pl011-spec-requirements.md` Sections 1 and 1.2.
This handoff does not duplicate those assignments; if any
conflict arises, defer to the requirements doc.

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
- The 3-stage progressive plan (Stage 1 / Stage 2 / Stage 3
  per §0.3) is implementation staging only. The paper reports
  the cumulative F+I+P result.
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

**ACTIVE (Stage 1 per §0.3 / 3-stage plan).** The Stage 1 scope
(fixture extensions, harness work, co-authored Linux golden
reference, Phase 7A-F emitter retarget, pilot run, Stage-1
campaign) is the current critical path. See §0.3 for the
ordered work list; this section preserves the historical
co-authoring notes that shaped the emission contract approach.

Note on the reference driver's role: it serves two purposes
from Stage 1 onwards -- (a) the compiler-determinism oracle
("the emitter's output byte-equals this reference"), and
(b) the paper's native-comparison baseline shape (native Linux
`amba-pl011` is the actual behavioral baseline; the hand-
authored reference is the *Regseq-generated* driver's authored
twin). Both uses need the same artifact.

Stages 2 and 3 extend the reference driver by diff (CRTSCTS
branches and DMA lowering respectively); Stage-1 authoring
should leave clean extension points.

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

### PL011 strategy artifact package (session 2; COMMITTED 2026-04-20)

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

### Session 4 (2026-04-22 + 2026-04-23) -- UNCOMMITTED

Two multi-day design workstreams closed plus one implementation
task marked done. Everything is in the working tree; nothing
pushed.

**User-data initialization Path A** (2026-04-22):

The PL011 fixture's Stage-1 `imsc_shadow: u32` user-data field
was blocking the compiler's construction pass (which required
`Option<T: Copy>` for user-data fields). A three-round Codex
adversarial debate explored six alternatives -- safe trait,
unsafe trait, attribute opt-in, compile-time `Default`
evaluation, definite-assignment analysis, author-written
`Default` impl -- and converged on a two-rule compiler-
controlled whitelist with no author-facing trait or attribute:

- **Rule 1** (new): compiler-controlled zero-init whitelist
  admits `u8, u16, u32, u64, bool` + arrays and `Copy` structs
  of these. Tuples excluded (the model's `RegseqType` has no
  Tuple variant; adding one cascades).
- **Rule 2** (unchanged): `Option<T: Copy>`.

Deliverables:
- `docs/discussions/models/regseq-model/user-data-initialization.md`
  (design doc, ~400 lines; includes "Stage-1 backend-tested
  scope (explicit narrowing)" and "Stage-1 Rule-1 whitelist
  narrowing (tuples excluded)" subsections).
- `docs/discussions/models/regseq-model/user-data-initialization-codex-review/`
  (5 transcripts: rounds 1-3 design debate; rounds 4-5
  brief-review + task-spec-review approvals).
- `.harness/spec-batches/user-data-init-path-a.md` (Codex-
  approved brief after 11 review rounds).
- `.harness/tasks/regseq-model/64-user-data-init-path-a.md`
  (Codex-approved task spec after 14 review rounds).
- `.harness/tasks.json` (new `regseq-model-64` entry under
  `regseq-model` group, status `done`; `regseq-backends-02`
  now depends on `regseq-model-64`).
- `.harness/task-spec-status.yaml` (new `user-data-init-path-a`
  tracker item, status `tasked`).

**Implementation status**: `regseq-model-64` was implemented by
a worker agent, Codex-approved (VERDICT: APPROVED), and marked
`done` per **Option A disposition**: the 13 downstream tests
in the acceptance criteria pass, but a broader
`cargo test --workspace` does not pass cleanly due to
pre-existing PL011 fixture drift the Path A work did not
itself introduce. Original fail-reason (since cleared from
`tasks.json` on the transition to `done`): the implementer
fixed `construction_log_metadata.rs` and
`construction_ordered_region.rs`; `pl011_construction.rs` and
9 other test files (`pl011_full_spec_lowering`,
`linux_emission_scaffold`, `semantic_body_lowering`,
`pl011_lowering_completeness_proof`, `semantic_model_projection`,
`source_to_extraction_07/09/10`) remained broken due to drift
in hardcoded method-name lists and specimen-fidelity
assertions (the Stage-1 Codex audit renamed
`program_polling_port` to `bring_up_port`/`reprogram_line_format`,
`configure_line_control` to `line_control_value` +
`write_line_control`, and removed `pending_tx: Option<u8>`).

**Shadow-register design + implementation plan** (2026-04-22):

Parallel to Path A, the shadow-register DSL extension was
designed end-to-end and reviewed by Codex, but NOT implemented
(per-user-direction: deferred until post-pipe-clean). DSL
surface: `#[rseq::register_entry(shadow = true, reset_value =
<lit>)]` on register-block entries. Five semantic-IR variants
(`ShadowRead`, `ShadowReadField`, `ShadowWrite`,
`ShadowWriteField`, `ShadowForceRead`; modify composes via
read+write). 13-20 engineering days estimated.

Deliverables:
- `docs/discussions/models/regseq-model/shadow-registers.md`
  (combined design + 7-phase implementation plan).
- `docs/discussions/models/regseq-model/shadow-registers-codex-review/`
  (2 transcripts: round-1-design-review,
  round-2-plan-review; both with folded amendments).
- Status: **deferred**; no tracker item or task spec authored
  (intentional -- revisit post-pipe-clean when a working
  Rust-to-C driver baseline is generated).

**Four fixture issues discovered** (2026-04-23):

After `regseq-model-64` landed, an attempt to audit whether
the fixture was ready for golden regeneration surfaced four
compiler-pass-skip issues on the fixture that neither the
Stage-1 Codex audit nor the Path A work caught. Details in
`docs/discussions/pl011-full-spec/pl011-full-spec-status.md`
"Known fixture issues (discovered 2026-04-23)":

1. `line_control_value` -- tuple destructuring at `driver.rs:190`
2. `uart_clock_hz` -- method-reference ZST at `driver.rs:334`
3. `rx_char` -- bitflags constant missing from `ConstantIndex`
   at `driver.rs:529`
4. `set_baud_rate` -- rustc `Assert` terminator for division
   at `driver.rs:148`

Issues 1-3 are fixture-only edits (~30-45 min total); issue 4
needs investigation to classify (release-profile MIR settings
vs compiler change).

**PL011 fixture cleanup tracker item** (2026-04-23):

The fixture drift + four fixture issues + golden regeneration
are bundled into one follow-up batch:

- `.harness/spec-batches/pl011-fixture-cleanup.md` (brief-stub
  authored; Codex review pending).
- `.harness/task-spec-status.yaml` -- new tracker item
  `pl011-fixture-cleanup`, status `ready_to_elaborate`,
  depends on `user-data-init-path-a`.
- Task spec NOT yet authored; the tracker is ready for a
  future session to elaborate it.

**When resuming session 5**:
- `regseq-model-64` is done; do not re-open it.
- The next major activity is **pl011-fixture-cleanup** (author
  its task spec + implement + regenerate goldens). This
  unblocks `regseq-backends-02` (Linux emission contract) and
  the hand-authored Linux reference C driver authoring.
- Run `uv run python scripts/python/task.py next-step` to
  confirm the current harness-visible next task. It may
  suggest another pending task first; the task-spec-status
  workflow triggers when no executable task is ready.

### Session 3 (2026-04-21) -- UNCOMMITTED

All session-3 work is in the working tree; nothing pushed. The
user will decide whether to commit as one cluster or split. The
uncommitted changes break down as follows:

**3-stage plan adoption** (frontend of the session):
- `docs/discussions/pl011-full-spec/pl011-spec-requirements.md`
  -- added `[stage-1]` / `[stage-2]` / `[stage-3]` tags to every
  claim-path REQ; added §1.2 stage-membership summary; authority
  for stage assignments.
- `docs/discussions/pl011-full-spec/pl011-full-spec-status.md`
  -- reframed around 3-stage plan; separated stage membership
  (deferred to spec-requirements §1.2) from implementation
  state; added Stage 1 captured/scheduled snapshot.
- `docs/discussions/pl011-full-spec/pl011-unified-spec-boundary.md`,
  `pl011-experiment-methodology.md`, `pl011-feature-matrix.md`,
  `pl011-feature-to-construct-mapping.md`,
  `pl011-full-spec-methodology.md`, `pl011-workloads.md` -- stage
  references updated; cheap-fix-audit items landed in-place
  (4-OS corpus note; W-P6-01 host-round-trip rewrite; repo path
  fixes; REQ-FLOW-* stage membership; tolerance-provenance
  note).
- This document (`contex-handoff.md`) -- §0.1/§0.3/§0.5/§7/§10/§15
  updated to reflect 3-stage plan; Stage 1 item 1 marked DONE.

**Stage 1 fixture fixes** (bulk of the session):
- `tests/fixtures/pl011-full-spec/src/driver.rs` -- grown from
  394 to ~630 lines. Fixes listed inline in §0.3 Stage 1 item 1
  above. Validated through three rounds of Codex adversarial
  review; final verdict `stage-1-complete`.
- `docs/discussions/pl011-full-spec/audit-history/` (NEW) --
  Codex transcripts for the three-round audit loop, plus a
  README.

**Related uncommitted files not produced this session** (present
in the working tree at session start or added by the user):
- `CLAUDE.md` (global user instructions file, not repo-scope)
- `docs/discussions/pl011-full-spec/pl011-minimized-datasheet.md`
  (user-provided datasheet excerpt used during this session's
  audit; load-bearing for the fixture review)
- `docs/discussions/pl011-full-spec/{freebsd,zephyr,windows}-driver-analysis*`
  (OS-specific analysis docs and task briefs; pre-session 3)
- `docs/discussions/pl011-full-spec/linux-driver-analysis.md`
  and related deletion of `linux-driver-analysis-primecell.md`
  (pre-session 3)

**When resuming**: run `git status --short` to see current
uncommitted set. The fixture changes plus the audit-history
transcripts are the primary session-3 outputs; the
spec-requirements + status + handoff edits are the supporting
doc trail.

### Session 2 (2026-04-20 continued)

**All session-2 strategy work was committed 2026-04-20** in
three thematic commits (four-OS corpus; Linux feature inventory
+ PrimeCell derivative; PL011 evaluation strategy package +
handoff update). Follow-up doc edits landed 2026-04-21
(cheap-fix audit and 3-stage tagging) as in-place updates. The
working tree at the start of session 2 also contained:

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

All session-2 transcripts saved under `/tmp/pl011-analysis/`
are ephemeral and do not persist across sessions. Regenerate
with the same prompt pattern if needed.

### Session 3: PL011 Stage 1 fixture audit (3 rounds to converged)

Stream 5: **Stage 1 fixture audit** (3 rounds to
`stage-1-complete`). Round 1 found 10 gaps (3 requiring user
decisions: TX batching / send_break semantics /
lifecycle-decomposition; 7 mechanical + 1 framework question).
Round 2 found 2 residuals (REQ-LIFE-02 regression + stale
status doc). Round 3 converged. All 39 Stage-1 claim-path REQs
are captured (37 fixture-level, 2 backend-emission-deferred).

Transcripts preserved in-repo at
`docs/discussions/pl011-full-spec/audit-history/` with
`README.md` summarizing round outcomes. This is the first time
Codex transcripts have been committed to the repo rather than
left in `/tmp`; future audit loops should follow the same
preserve-in-repo pattern.

### Codex-runner pattern

```bash
cat <<'EOF' | uv run --project /path/to/repo python \
    /path/to/repo/scripts/python/codex_runner.py \
    --timeout 540 --message-only
<task>
...prompt...
</task>
<grounding_rules>Cite file:line for every claim.</grounding_rules>
<structured_output_contract>...</structured_output_contract>
EOF
```

Use absolute paths to avoid cwd-drift issues if the Bash tool's
working directory has been changed by an earlier command.
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
- Strategy artifact package (8 docs) is COMMITTED; it is the
  authoritative source for paper-evaluation claims.
- **The current critical path is Stage 1 of the 3-stage
  progressive plan** (§0.3). Stage 1 is Rust-only + harness
  work + reference-driver co-authoring + Phase 7A-F emitter
  retarget + native-only pilot + Stage-1 campaign. No QEMU
  patches are required in Stage 1.
- **Stage 1 fixture extensions are DONE (2026-04-21)**. Three-
  round Codex audit loop converged at `stage-1-complete`.
  Transcripts in `docs/discussions/pl011-full-spec/audit-history/`.
  Fixture changes in `tests/fixtures/pl011-full-spec/src/driver.rs`
  are UNCOMMITTED; see §10 session-3 entry.
- **User-data initialization Path A is DONE (2026-04-22)**.
  Task `regseq-model-64` implemented, Codex-approved, marked
  `done` per Option A disposition. Unblocks the fixture's
  `imsc_shadow: u32` bare-u32 user-data. Design at
  `docs/discussions/models/regseq-model/user-data-initialization.md`;
  full session-4 context in §10.
- **Shadow-register DSL design is COMPLETE but implementation
  is DEFERRED**. Combined design + 7-phase implementation
  plan at
  `docs/discussions/models/regseq-model/shadow-registers.md`
  (Codex-reviewed across two rounds). No tracker item yet;
  revisit post-pipe-clean.
- **Four fixture issues discovered 2026-04-23** block golden
  regeneration. Enumerated authoritatively in
  `docs/discussions/pl011-full-spec/pl011-full-spec-status.md`
  "Known fixture issues (discovered 2026-04-23)":
  `line_control_value` tuple destructuring (`driver.rs:190`),
  `uart_clock_hz` method-ref ZST (`driver.rs:334`),
  `rx_char` bitflags const missing from ConstantIndex
  (`driver.rs:529`), `set_baud_rate` Assert terminator for
  division (`driver.rs:148`).
- **Active next step: Stage 1 item 4 -- PL011 fixture cleanup**
  (tracker item `pl011-fixture-cleanup`, status
  `ready_to_elaborate`; brief stub at
  `.harness/spec-batches/pl011-fixture-cleanup.md`). This
  task bundles the four fixture fixes, the ~9-test-file
  rename-drift cleanup, and golden regeneration. It unblocks
  item 5 (Linux reference C authoring) and item 8
  (`regseq-backends-02` emission contract).
- Harness work (§0.3 item 2) can still proceed in parallel
  since it does not touch the fixture.
- Stage 2 (flow control; QEMU patches 3.1/3.2/3.4) and Stage 3
  (IFLS + DMA; QEMU patch 3.3) are queued behind Stage 1.
- Stage membership authority: `pl011-spec-requirements.md` §1.2.
  Implementation state: `pl011-full-spec-status.md`. The
  handoff does not duplicate the REQ-to-stage mapping.
- Do NOT: resurrect `compat.rs`; use the reference as
  driver-quality oracle; claim byte-parity to anything as
  evidence of driver quality; use `codex:codex-rescue` agent.
- Four-OS corpus lives at `reference-drivers/pl011/`.
- Codex adversarial debate pattern works well; use
  `scripts/python/codex_runner.py` with
  `run_in_background: true` and structured prompts.
