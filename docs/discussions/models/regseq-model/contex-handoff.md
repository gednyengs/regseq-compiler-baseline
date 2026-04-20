# Phase-7 Context Handoff

> **Purpose**: everything a new session needs to resume the Phase-7
> work without re-reading the full conversation history. Written at
> the end of a working session that re-elaborated Phase-7 from a
> retired 6-task framing into a 15-task D' (layered) validation
> batch, and began transitioning into the hand-authored PL011
> reference work.

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

We are about to **start co-authoring the PL011 reference driver**.
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

The user's answer to that question is the first thing to elicit
in the next session.

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

### Target artifacts (will be created)

- `tests/fixtures/pl011-full-spec/expected/pl011-reference-linux-driver.c`
  -- the hand-authored PL011 Linux reference. **Does not exist
  yet.** Co-authored target.
- `docs/discussions/models/regseq-model/linux-emission-contract.md`
  -- the emission contract markdown. Does not exist yet.
- `crates/regseq-backends/src/linux/emission_tables.rs` -- Rust
  data tables. Does not exist yet.
- `tests/goldens/pl011/linux/constructs/*.c` -- ~15 exemplar
  fragments. Directory does not exist yet.

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

## 10. Recent commit history (today's session)

```
5b23c6f regseqmodel-phase-7: flip validation from blessing-first to authoring-first
a26eefb regseqmodel-phase-7: author 15-task D' validation spec batch
4c67e6f regseqmodel-phase-7: retire original compat-adapter task specs
8197b09 docs: align migration.md + execution-plan.md with Phase-7 D' validation
f3664c4 docs: regseq-model Phase-7 redesign migration design doc
```

Previous work (pre-session, already landed):

```
d366c00 regseqmodel-phase-7: author consumer-migration task specs
  (the retired original 6-task batch)
c00ef9c regseq-model-60: PL011 Phase-6 end-to-end pipeline + JSON golden
  (the Phase-6 gate; tests/fixtures/.../regseq-model.json lives here)
```

---

## 11. Codex review history (useful context for future reviews)

Four rounds of Codex adversarial review occurred during the
Phase-7 re-elaboration today. Each returned `REQUIRES FIXES`
except the last:

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

Every round's transcript is in `/tmp/codex-*.txt` for the
session but those files don't persist across sessions. If
needed, regenerate with the same prompt skeletons.

The spec batch today is **considered implementer-ready** by
Codex at its final review. The only prerequisite before handing
off is the co-authored task 1 work (the hand-authored reference).

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

Before substantive authoring begins, get the user's answer on:

1. **Top-down vs bottom-up authoring order**. Claude recommended
   bottom-up (start with `tx_byte`). User's preference not yet
   confirmed.
2. **QEMU smoke gating on task 1 vs task 11**. If task 11 (QEMU
   harness) hasn't landed when the reference is ready, do we:
   (a) land the reference with only host-build validation + commit
       with a follow-up note promising to run smoke once the
       harness exists, OR
   (b) pair on task 11 first, then task 1's reference?
3. **Contract document authoring cadence**. Do we write the
   contract as we go (update after each function) or batch
   contract updates at the end of the reference?
4. **Data tables in parallel or last**. Are we authoring
   `emission_tables.rs` alongside the reference, or after?
5. **First function to translate**. If bottom-up: which PL011
   function? (`tx_byte` recommended; `tx_ready` / `rx_char`
   also small.)

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

- Phase 7 is elaborated into 15 tasks. All committed. Review
  `migration.md` Q5/Q6 + the `.harness/tasks/regseq-backends/02-*.md`
  spec.
- The immediate next step is NOT implementer work. It is
  co-authored: user + Claude, producing the hand-authored PL011
  Linux reference driver at
  `tests/fixtures/pl011-full-spec/expected/pl011-reference-linux-driver.c`.
- Ask user: top-down (contract first) or bottom-up
  (`tx_byte` first)?
- Use `migration.md`, `compiler-knowledge.md`, and the
  existing PL011 RegseqModel golden as the primary references.
- Do NOT resurrect `compat.rs`, do NOT bless emitter output,
  do NOT delegate task 1 to an implementer.
