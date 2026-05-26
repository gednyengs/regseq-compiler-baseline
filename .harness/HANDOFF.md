# Session Handoff — Generic-C Backend Workstream

Persistent snapshot of in-flight work state for fresh-session
resumption. Incrementally updated after each task/decision close so the
next session can pick up at full quality.

**This handoff is scoped to the generic-C backend workstream** (codegen:
`RegseqModel` → host-compilable C). The PRIOR workstream — bridge
coverage, which closed the *front-end* IR-structural feature-complete
gate — is archived verbatim at
`.harness/archive/2026-05-26-bridge-coverage-runner/HANDOFF.md` (read it
for bridge-era detail, the F0–F9 / R1–R7 history, and the full
conventions catalogue).

> **Arrived here from a bridge-era task spec?** Completed bridge tasks
> under `.harness/tasks/regseq-driver/` (e.g. 69, 70, 71, 72, 78) cite
> `.harness/HANDOFF.md` for snippet/authoring protocol or bridge state.
> That content moved to the archived file above — it is NOT below.
> (Those historical specs are intentionally not rewritten.)

## Update protocol

- Update **§ Current state** after every task/decision close (new commit
  SHA, in-flight item, decision deltas).
- Update **§ Phase arc** only when a backend phase completes.
- Update **§ Decisions locked** when a design fork is resolved.
- **§ Workflow / Anchors / Tooling / Conventions / Reading order /
  First-action** are stable; touch only on a methodology change.

The volatility split means ~90% of updates touch only **§ Current state**.

---

## § Current state (UPDATE EVERY TASK/DECISION CLOSE)

- **Last updated**: 2026-05-26
- **Workstream status**: Phase-1 **planning complete** (all 3 Level-2
  plan docs filled + Codex-approved). The bridge front-end gate PASSES
  (`1a6a996`, archived handoff). Next is the spec delta → tasks →
  implementation.
- **Last completed**: **Phase-1 Level-2 plan docs ALL FILLED** at
  `docs/discussions/generic-c-backend/phase-1/` (each Codex-APPROVED):
  - **Doc 1** (`01-…`, construct surface + matrix) — 170 rows / 13
    families, classified covered/deferred/unspecced/out-of-scope, + a
    grouped **gap list**. Surfaced two non-pre-flagged structural gaps:
    struct/union field-offset reproduction (`FieldLayout` is source/MIR
    order, not memory order, layout.rs:37) and payload-carrying enum C
    shape. Codex APPROVED (3 rounds).
  - **Doc 2** (`02-…`, architecture decisions) — **D1–D12** pin every
    gap concretely. Highlights: D5 terminator emission (all 6, the §7
    core delta); D7 Rvalue/BinOp (`-fwrapv` for signed +,-,*; **shifts in
    the unsigned domain** since `-fwrapv` does NOT cover shift UB; Div/Rem
    rely on upstream guarding Asserts); D8 casts (FloatToInt saturating
    helper; Transmute via memcpy; FnPtrToPtr host assumption); **D9 enum =
    offset-faithful** (NOT naive `{tag;union}`; `TagScalar.offset`
    arbitrary, variant field offsets absolute; niche decode compares the
    cell vs precomputed `VariantLayout.tag_value` → handles mod-width +
    wrapping); D10 field-offset reproduction (sort-by-offset + explicit
    pad + `_Static_assert` guards); D11 Const→C (float consts =
    constant-expression hex-float/`__builtin_inf`/`__builtin_nan`, NOT a
    union member access which isn't a constant expr); D12 symbol/linkage/
    section/inline/`c_variadic`. Codex first-contact caught the D9 + D7
    errors pre-draft; APPROVED after 3 review rounds.
  - **Doc 3** (`03-…`, verification flow) — the codegen-coverage harness
    (`crates/codegen-coverage-runner`): 6-stage pipeline, dual oracle
    (diag-log golden **stdout** [behavior] + golden **`.c` snapshot**
    [determinism]), pinned fixture layout + `manifest.toml` schema,
    `.harness/generic-c/coverage_ledger.{json,md}` ledger, exit gate
    (`pass` ⇔ every in-scope `covered` row green), toolchain (`-fwrapv
    -std=gnu11`, gcc/clang host assumptions). Codex APPROVED.
  - Prior: Level-1 roadmap (`e74fbb3`), scaffolding (`2a8aa60`).
- **In-flight**: **none.** Next is the **Phase-1 spec delta** into
  `generic-c-backend.md` (every Doc-2 `Spec-delta: yes`), then `task.py`
  tasks.
- **Uncommitted state**: **all three Phase-1 plan docs** (`01`/`02`/`03`)
  are filled but **not yet committed** (no commit requested). The two
  standing untracked dirs stay untouched by rule —
  `tests/fixtures/playground-pl011/`, `crates/regseq-driver/tests/
  fixtures/` (experimentation; never commit, never modify).

### Next actions (in priority order)

1. **Phase-1 spec delta** into the canonical
   `docs/discussions/regseq-model/generic-c-backend.md` — apply every
   Doc-2 decision flagged `Spec-delta: yes`: D1 type-name + identifier
   policy (§7); D2 a new declaration-order section + C-ABI prototypes +
   `c_variadic`; D4 panic section + the `AssertKind` message table; **D5
   the §7 terminator-emission section (the core, was DEFERRED)**; D7 a new
   Rvalue section + BinOp/UnOp tables; D8 a cast section + saturating-cast
   contract + host assumptions; D9 a generalized enum-representation
   section (supersedes §9.8/§9.9 as examples); D10 a field-offset section;
   D11 the per-`Const` initializer table (§5); D12 a symbol/linkage/
   section/inline section. Codex-reviewed (2–4 rounds). (Doc 3 is
   backend/harness-internal — **no** spec delta.)
2. **`task.py` tasks** — file the implementation tasks (the generic-C
   backend in `crates/regseq-backends`, the `codegen-coverage-runner`
   crate + fixtures) per the 8-section template; Codex-reviewed; commit +
   push; implementer picks up.
3. **(Later) Phase-2 RegseqError** — describe at Phase-2 start; not before.

---

## § Decisions locked (UPDATE ON FORK RESOLUTION)

Converged with the architect (2026-05-26). Record so they're not relitigated.

- **Two-level planning.** L1 = phase roadmap (sequence + exit gates +
  cross-track deps). L2 = per-phase plan-first design doc → spec → Codex
  → tasks.
- **Phase roadmap (L1) — authoritative doc:
  `docs/discussions/generic-c-backend/roadmap.md`.** P1 = full supported
  **Rust surface + diagnostic logging** (fully scoped). P2 =
  **RegseqError** — the abstraction for authoring OS-agnostic errors that
  are mapped to OS-specific error codes during lowering; **definition
  only — detailed description + spec + tasks deferred to Phase-2 start**
  (per architect). **P3+ = domain constructs one at a time, deliberately
  UNSCHEDULED** — picked by immediate need after P1–P2 (Register Access
  anticipated, NOT committed; pool also incl. Interrupts/Atomics/Lifecycle/
  Ordered-regions). Post-P1 ordering is partly forced by **front-end
  availability** (most domains aren't bridged — stages 16/17/18).
- **Domain phases are gated by what's in `RegseqModel`.** Diag-logging is
  the ONLY domain construct currently bridged (survives as
  `AttrData::Intrinsic(IntrinsicKind::Log(level))` on the callee item).
  MMIO/interrupt/atomic are spec stages 16/17/18, NOT implemented (atomics
  rejected). So P3+ each need a front-end (bridge) phase to land the
  construct in the IR before the backend can lower it. RegseqError
  bridge-status: **TBD — deferred to Phase-2 start** (per the roadmap;
  not investigated in the L1 doc by architect decision).
- **Per-phase exit gate** mirrors the bridge gate: a domain is
  "codegen-complete" when every construct lowers, compiles, runs, and
  auto-validates on the host.
- **Host-validatability varies by domain.** diag-log (printf) and
  register access (backing byte array) simulate *faithfully*; lifecycle
  and interrupts are partly *target* semantics → host validates structure
  only; real behavior needs Linux/Zephyr.
- **Phase-1 verification = a codegen-coverage harness** (bridge →
  `RegseqModel` → emit C → host `cc` → run → assert), with a DUAL oracle:
  diag-log **golden stdout** (behavior) + **golden C-file snapshot**
  (determinism/diff). Write fixtures that log *computed* values so the
  print is a genuine end-to-end test of the lowering.
- **Diag-log recognition (for the backend):** key on the durable model
  fact — a `Terminator::Call { callee: CallCallee::Direct(inst), .. }`
  where `model.instances[inst].item` carries
  `AttrData::Intrinsic(IntrinsicKind::Log(level))` in
  `domain_attrs.item_attrs`. Recognition is **location-independent**:
  bridge keys on the `#[rseq::intrinsic("log-…")]` attribute + the leaf
  fn name (`log_…`), not any module path; keyed by `DefPathHash`. Args
  arrive **already peeled flat** (`[fmt_const, a1, … aN]`; no `LogArg`/
  `&[LogArg]` types). **Backend rule: the log-marked item is a
  recognition tag — intercept at call sites; do NOT emit a C definition
  for it.**
- **Open Phase-1 architecture decisions (resolve in the Phase-1 doc):**
  (a) **naming** — lean on `Instance.emission_name` (bridge-minted,
  "uniqueness guaranteed") + interned `FieldLayout.name`; confirm its
  determinism scope; derive only *type* names (from `DefPath`). (b)
  **declaration order** — forward-declare everything (prototypes + type
  fwd-decls + `extern` statics) up front; topologically sort only *type
  definitions* by by-value containment (pointers break cycles). (c)
  **file layout** — single translation unit first for the validation
  loop; encode module hierarchy in `DefPath`-qualified names so a later
  partition is a no-rename split (multi-file gated/deferred). (d) **host
  panic/abort** — Assert failure / `panic=abort` / `Unreachable` →
  `printf` + `abort()`/`__builtin_trap()`. (e) **terminator emission** —
  `generic-c-backend.md` §7 explicitly DEFERS all 6 terminators
  (`Return`/`Goto`/`SwitchInt`/`Call`/`Assert`/`Unreachable`, + Drop-as-
  Call); spec + implement them in P1.
- **Scope OUT of generic-C:** real-OS lifecycle/devicetree/probe-remove
  (per-target Linux/Zephyr); MMIO/interrupt/atomic until bridged.
- **Doc governance (option A):** canonical specs (`generic-c-backend.md`,
  future `linux-backend.md`/`zephyr-backend.md`) stay as co-equal merger
  artifacts in `docs/discussions/regseq-model/`; the new
  `docs/discussions/generic-c-backend/` holds roadmap/plan/matrix/strategy.

---

## § Phase arc (UPDATE ON PHASE CLOSE)

| Phase | Scope | Status | Commit | Deliverable |
| ----- | ----- | ------ | ------ | ----------- |
| (scaffold) | workstream homes + reseed HANDOFF | done | `2a8aa60` | docs HQ + `.harness/generic-c/` + archived bridge handoff |
| L1 roadmap | phase sequence + deps + gates | done | `e74fbb3` | `docs/discussions/generic-c-backend/roadmap.md` (Codex APPROVED) |
| P1 plan docs | entry + 3 docs (surface+matrix / decisions / verification) | ALL FILLED (each Codex APPROVED) | (uncommitted) | `phase-1/01` 170-row matrix+gaps, `02` D1–D12 decisions, `03` codegen-coverage harness |
| P1 impl | Rust surface + diag-log → C, + verification harness | pending | — | generic-C backend MVP + codegen-coverage harness |

---

## § Project orientation (STABLE)

- **Regseq compiler**: Rust → C device-driver compiler. User is the
  designer/architect.
- **Where we are**: the rustc→`RegseqModel` front-end (the "bridge") is
  IR-structurally feature-complete (gate `pass`). The next stage is
  **codegen**: lowering the validated IR to C, starting with a generic-C
  (hosted-userspace) backend, built in domain phases.
- **Verification boundary (this workstream)**: a construct lowers to C
  that compiles + runs on a host toolchain and auto-validates. NOT
  real-OS integration (separate per-target backends).
- **Auto-memory**: at
  `~/.claude/projects/-home-nyengele-Documents-sekekama-regseq-compiler-asplos/memory/`.
  Carries role + binding rules (compiler-theory-first, no-defensive-
  fallbacks, spec-before-implementation, Codex review loop, playground-is-
  experimentation-space) + the bridge-feature-complete-gate milestone.

---

## § Workflow conventions (STABLE — carried from the bridge workstream)

### Plan → spec → implement loop

1. **Plan-first design doc** (pre-spec) → `docs/discussions/generic-c-backend/`
   or `.harness/audits/`: resolve forks, HEAD-grounded; Codex-review.
2. **Spec delta** into the canonical `generic-c-backend.md` (the 8-section
   template below), Codex-reviewed (most specs take 2–4 rounds).
3. **File** via `task.py add-task` (see Tooling); commit + push; implementer
   picks up.
4. **Assess** the implementer's close (build/fmt/clippy/test + the codegen
   harness + diff-scope), trust-but-verify; **soundness pass via Codex on
   any load-bearing change** ([[assessment-soundness-pass]]).

### The 8-section task-spec template

1. Description · 2. Input Context (design source / spec anchors / code
anchors / prior tasks / test surface) · 3. Implementation Notes (Change
A/B/C…) · 4. Non-Goals · 5. Test Plan · 6. Review (files / reviewer
focus / four-block adversarial prompt) · 7. Quality Gate · 8. Acceptance
Criteria (numbered, concrete).

### Codex invocation (canonical)

Use the **`codex-runner` Skill** (single source of truth; bundled runner
is pure stdlib — plain `python3`, no `uv run`). The repo's local
`scripts/python/codex_runner.py` is retired.

```bash
cat <<'EOF' | python3 "/home/nyengele/.claude/skills/codex-runner/scripts/codex_runner.py" --timeout 900 --message-only
<task>
... what to review/identify; scope boundaries; files to read ...
</task>

<grounding_rules>
- Cite file:line for every claim. Mark hypotheses as hypotheses.
- Don't speculate about code/spec you didn't read.
</grounding_rules>

<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
<per-check fields with PASS/FAIL + citation>
NEW ISSUES: numbered or "none"
REQUIRED FIXES: numbered (only if VERDICT != APPROVED)
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, decide, stop.
</default_follow_through_policy>
EOF
```

Bash `timeout` ≥ `950000` (Codex `--timeout 900` + margin). `--effort high`
default. Codex empirically probes code (runs `cargo`/tools) — trust
findings. Flags: `--message-only`, `--no-yolo`, `--model`, `--effort`, `--cwd`.

### Commit conventions

- Conventional prefixes (`feat:`, `fix:`, `gc-NN:` / group prefix, `chore:`).
- **No author attributions** (per global CLAUDE.md §1.3). HEREDOC for
  multi-line bodies.
- Commit/push only when the user asks; the project commits to `main`
  directly (single-architect workflow). HANDOFF refresh is committed in
  the same session that did the work.

### Cargo test gotcha

`cargo test --workspace` in parallel mode can hit a rustc-driver stderr
interleaving quirk (E0463). Use
`RUST_TEST_THREADS=1 cargo test --workspace -- --test-threads=1` OR test
individual crates.

---

## § Key file anchors (STABLE; update on architectural change)

### Design sources

- `docs/discussions/regseq-model/generic-c-backend.md` — **canonical
  C-emission spec** (co-equal merger artifact; the IR→C mapping: §2 Ty
  table, §3 pointers/fat-pointers, §4 places, §5 operands, §6 intrinsics,
  §7 body envelope [terminators DEFERRED], §8 diag-log→printf, §9 worked
  examples).
- `docs/discussions/regseq-model/spec.md` — the IR producer-surface
  (§8 Item, §9 Instance/Const, §10 Ty, §11 Layout, §14 FnSig/FnAbi + §14.5
  C-ABI admissibility, §19–§23 place/operand/rvalue/statement/terminator,
  §25 Body, §26 diag-log, §3.6 domain attribute tables).
- `docs/discussions/regseq-model/pretty-printer.md` — sibling consumer
  surface (identifier/render conventions the C backend coincides with).
- `docs/discussions/generic-c-backend/` — this workstream's roadmap /
  per-phase plans / codegen-coverage matrix / cross-backend strategy.

### Code surfaces

- `crates/regseq-model/src/` — the owned IR the backend consumes (rustc-
  free; walk **instances**, not items — post-mono).
- `crates/regseq-backends/` — the NEW backend crate (currently a 2-line
  stub; the generic-C impl lands here).
- `crates/regseq-backends-old/` — prior C + Linux backend (~3k LOC, built
  against the OLD model; reference for shape only, NOT ported — had holes).
- `crates/regseq-driver/src/bridge/diag_log/` — diag-log recognition
  (`recognition.rs` attr+name; `mod.rs` orchestration; `peel.rs` arg
  flattening; `format.rs` template grammar).
- `crates/test-ir-fixtures/` — the fixture harness (reusable for codegen
  fixtures).

### Harness

- `.harness/HANDOFF.md` — **this file** (workstream resume).
- `.harness/generic-c/` — live ledgers / trackers / scratchpads.
- `.harness/tasks/<group>/NN-*.md` + `tasks.json` — registered task specs.
- `.harness/archive/2026-05-26-bridge-coverage-runner/HANDOFF.md` — the
  closed bridge workstream (deep history + full conventions catalogue).

---

## § Tooling specifics (STABLE)

### task.py add-task

```bash
uv run python scripts/python/task.py add-task \
  --id <group>-NN --name "<title>" \
  --spec .harness/tasks/<group>/NN-<slug>.md \
  --group <group> --depends-on <prev> --phase 1b
```

### Toolchain

- Pinned `nightly-2026-02-01` per `rust-toolchain.toml`.
- Repo uses **uv** for Python; test/dev deps via `uv add --dev`; tool
  runtimes stay stdlib-only ([[uv-python-tooling]]).
- Workspace members registered in `Cargo.toml` (alphabetical).

---

## § Constraints + gotchas (STABLE)

- **No defensive fallbacks** (memory `feedback_no_defensive_fallbacks`).
- **No author attributions** in commits (global CLAUDE.md).
- **Playground is experimentation space** — never modify/commit
  `tests/fixtures/playground*` or `crates/regseq-driver/tests/fixtures/`.
- **The model is post-mono + rustc-free** — the backend walks `instances`
  and trusts layout/C-ABI as bridge certificates (don't recompute).
- **`FieldLayout` is indexed by source/MIR field order**, not memory
  order; offsets are from the containing aggregate.
- Run any new Python tooling's tests (`uv run pytest …`) after changes.

---

## § Reading order for cold start (STABLE)

1. **This handoff** (you're reading it) — esp. § Current state + §
   Decisions locked.
2. Auto-memory (loads automatically) — incl. the
   bridge-feature-complete-gate milestone.
3. `docs/discussions/generic-c-backend/` README + roadmap (once it exists).
4. `docs/discussions/regseq-model/generic-c-backend.md` — the canonical
   C-emission spec (the input contract for implementation).
5. The Phase-1 plan-first doc + codegen-coverage matrix (once they exist).
6. Only if needed: the archived bridge HANDOFF
   (`.harness/archive/2026-05-26-bridge-coverage-runner/HANDOFF.md`) for
   methodology depth or front-end questions.

---

## § First-action recommendations by user input (STABLE)

**"Let's continue / what's next"** → § Current state → Next actions. If no
task in flight, the next artifact is the L1 roadmap doc or the Phase-1
plan-first doc (whichever § Current state names).

**"Implementer is done"** → `git fetch && git log --oneline HEAD..origin/main`
→ `git pull --ff-only` → read the task spec + any expectations → verify
(build / fmt / clippy / test / **codegen harness run**) → diff-scope check
→ soundness Codex pass on load-bearing changes → update § Current state +
§ Phase arc + § Update log → commit.

**"File a new task"** → 8-section template → Codex (2–4 rounds) → apply
fixes → `task.py add-task` → commit → update § Current state.

**"Compact / hand off"** → update § Current state + § Decisions locked;
the rest is stable.

---

## § Update log (append-only)

- 2026-05-26: Generic-C backend workstream opened. Bridge front-end gate
  closed (`1a6a996`); bridge HANDOFF archived to
  `.harness/archive/2026-05-26-bridge-coverage-runner/HANDOFF.md`. Created
  `docs/discussions/generic-c-backend/` (design HQ) + `.harness/generic-c/`
  (working state) + reseeded this HANDOFF. Strategy converged with the
  architect (see § Decisions locked): two-level planning; phased roadmap
  gated by front-end availability (only diag-log bridged); Phase-1 =
  Rust surface + diag-log with a codegen-coverage harness; canonical spec
  stays in `regseq-model/` (option A). Scaffolding Codex-reviewed.
- 2026-05-26: **Level-1 roadmap** drafted at
  `docs/discussions/generic-c-backend/roadmap.md`. Per architect framing:
  P1 (Rust surface + diag-log) fully scoped with deliverables + open
  architecture decisions + exit gate; P2 (RegseqError = OS-agnostic errors
  mapped to OS-specific codes during lowering) recorded as DEFINITION
  ONLY, detail/spec/tasks deferred to Phase-2 start; P3+ domain constructs
  deliberately UNSCHEDULED (one-at-a-time, picked by immediate need after
  P1–P2; Register Access anticipated, not committed). Roadmap states the
  front-end↔backend dependency model (only diag-log bridged; MMIO/intr/
  atomic = stages 16/17/18) and the per-phase exit gate. Codex review
  APPROVED (4/4: faithful-to-architect / factual / no-overscheduling /
  consistent; verified the bridged-vs-not + §7-defers-terminators +
  emission_name claims against source). Reconciled § Decisions locked +
  § Current state to match (dropped the older fixed P3/P4 draft order).
  Next: the Phase-1 plan-first (Level-2) design doc.
- 2026-05-26: **Phase-1 plan-doc structure** stood up at
  `docs/discussions/generic-c-backend/phase-1/` (entry `README.md` + 3
  skeletons: `01-construct-surface-and-matrix.md`,
  `02-architecture-decisions.md`, `03-verification-flow.md`), per the
  architect's 3-document strategy + "settle in docs, not implementation."
  Locked decisions embedded (the 6 architecture directions, diag-log
  interception rule, dual-oracle verification); open work marked TO FILL.
  **The construct-surface re-derivation (Doc 1) is deliberately deferred
  to a fresh session** (it's the heaviest, most quality-critical artifact;
  the architect chose to set up cleanly now and re-derive with full
  budget). Doc 1 carries its own precise "Method" (re-derive from the 15
  `regseq-model/src` files + `generic-c-backend.md`, classify each
  construct, output the gap list). Codex review APPROVED (4/4:
  faithful-to-decisions / gap-claims-accurate / source-files-exist /
  self-consistent — Codex independently confirmed §7 defers all 6
  terminators + per-Rvalue/BinOp/CastKind is unspecced + the source files
  exist). **Next session: fill Doc 1 (re-derive), then Docs 2+3, then the
  `generic-c-backend.md` spec delta + `task.py` tasks.**
- 2026-05-26: **Doc 1 — construct surface + codegen-coverage matrix**
  FILLED at `phase-1/01-construct-surface-and-matrix.md`. Re-derived
  against the 15 `regseq-model/src` files + reconciled with
  `generic-c-backend.md` + `spec.md` (§14.5/§21/§23/§26). 170 rows in 13
  families, classified covered/deferred/unspecced/out-of-scope, with a
  grouped gap list. Confirmed the skeleton's pre-flags (gc §7 defers all 6
  terminators; per-Rvalue/BinOp/CastKind/AggregateKind genuinely unspecced
  per spec §21.3.4 "commits only to the variant set"; Ty §2 covers all 19;
  Place/Operand/intrinsics/diag-log covered) AND surfaced two
  non-pre-flagged structural gaps: struct/union field-offset reproduction
  (`FieldLayout` is source/MIR order, not memory order) and
  payload-carrying enum C shape (only fieldless-Direct + single-niche
  examples exist). Added a `Body`/`BasicBlock`/`LocalDecl` family the
  skeleton had folded into notes. Codex review APPROVED after 3 rounds
  (r1 caught the missing Body/LocalDecl rows + hybrid status strings +
  gap-list "all-and-only" violations; r2 caught a residual parenthetical
  status [LY-1] + one uncited unspecced row [LY-14]; r3 APPROVED 3/3, no
  residual issues). A programmatic cross-check confirms all 96
  unspecced/deferred rows are cited in the gap list and every status is a
  legend value. **Uncommitted** (no commit requested). **Next: Docs 2+3.**
- 2026-05-26: **Phase-1 Docs 2 + 3 FILLED** (both Codex-APPROVED). **Doc 2
  — architecture decisions** (`02-…`): D1–D12 resolve the entire Doc-1 gap
  list to pinned C-lowering rules. Codex **first-contact** (pre-draft) on
  the open forks caught two real errors before they were written: (1) the
  naive `{tag; union}` enum shape is WRONG — `TagScalar.offset` is
  arbitrary and variant field offsets are absolute to the enum object, so
  D9 is **offset-faithful** (reuses D10's pad+`_Static_assert` technique;
  niche decode compares the cell against precomputed
  `VariantLayout.tag_value`, handling mod-2^width + wrapping); (2)
  `-fwrapv` does NOT cover signed **shift** UB, so D7 lowers wrapping
  shifts in the unsigned domain. Three review rounds then fixed: missing
  `c_variadic` (gap C12 → D2), D9 niche wrapping, and the D11 float
  constant (union member access is not a C constant expression → switched
  to hex-float / `__builtin_inf` / `__builtin_nan` constant-expression
  forms, valid in static initializers + bit-faithful incl. NaN payloads).
  **Doc 3 — verification flow** (`03-…`): the codegen-coverage harness
  (`crates/codegen-coverage-runner`) — 6-stage pipeline, dual oracle
  (diag-log golden stdout + golden `.c` snapshot), pinned fixture-dir +
  `manifest.toml` schema, `.harness/generic-c/` ledger, exit gate,
  toolchain (`-fwrapv -std=gnu11` + the gcc/clang host assumptions). Codex
  review narrowed the oracle claims (one fixture proves its *exercised
  path*, not the whole chain; snapshot drift is review-required, never
  auto-blessed) and completed the host-assumption list. **All three plan
  docs uncommitted** (no commit requested). **Next: the
  `generic-c-backend.md` spec delta (every Doc-2 `Spec-delta: yes`) →
  `task.py` tasks.**
