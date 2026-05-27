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
- **Workstream status**: Phase-1 **planning + spec delta complete**
  (all 3 Level-2 plan docs + the `generic-c-backend.md` spec delta filled
  + Codex-approved). The bridge front-end gate PASSES (`1a6a996`, archived
  handoff). Next is **`task.py` tasks** → implementation.
- **Last completed**: **Phase-1 task a CLOSED (`regseq-backends-04`)** — the
  `codegen-coverage-runner` scaffold landed + assessed. Implementer commit
  `a422d89` (crate + 6-stage pipeline, dual oracle, manifest schema, Doc-1
  matrix parser + coverage ledger, host-`cc` toolchain, the `regseq-backends`
  `emit_generic_c` seam stub, 24 tests, empty corpus). **Assessment**: fmt /
  clippy / 24 tests / Mode-A baseline (176 rows, verdict `not-pass`, exit 1)
  all green; diff scope clean; seam faithful. **One load-bearing defect
  caught + fixed** (`9e3f46b`): the committed `coverage_ledger.json` carried
  a per-row wall-clock `last_run` → every run rewrote all 176 rows
  (non-deterministic, violating Doc 3 §4 byte-stability + burying drift).
  Root cause = a Doc-3 §5↔§4 tension (§5 listed `last_run`); fix drops it +
  reconciles §5; ledger now byte-stable (verified by running twice). See
  [[codegen-ledger-determinism]]. NOTE: the implementer hand-set
  `status: done` (no task.py `completed` history event); end state correct.
  The spec was Codex-2-round-APPROVED before filing. The **Doc-1 status-flip
  workflow is now RESOLVED** (`111da9b`, see § Decisions locked): matrix
  `status` = scope/spec axis (reconciled to post-delta — all 169 in-scope
  rows `covered`); coverage tracked live by the ledger; b–e author fixtures,
  no per-task matrix edits. **Prior: spec delta committed (`cb0b793`)** — now
  the complete §§1–17 input contract. Applied to the canonical
  `docs/discussions/regseq-model/generic-c-backend.md` — every Doc-2
  `Spec-delta: yes` decision (D1,D2,D4,D5,D7,D8,D9,D10,D11,D12)
  transcribed: §7 identifier policy gains type-name derivation (D1); **new
  §7.1 per-terminator emission** replaces the old "deferred" paragraph
  (D5); new §10 Rvalue+BinOp/UnOp (D7), §11 casts (D8), §12 `Const`
  initializers (D11), §13 enum offset-faithful representation (D9,
  generalizing §9.8/§9.9), §14 field-offset reproduction (D10), §15
  declaration-order + C-ABI prototypes + `c_variadic` + the by-value-containment edge set (D2), §16 panic +
  `AssertKind` table (D4), §17 symbol/linkage (D12); §2/§5 rows point into
  the new sections. Numbering kept §1–§9 intact (only external ref is to
  §8); new sections appended §10–§17. Codex APPROVED after 3 rounds (r1:
  bank-4 statics / bank-2 struct-vs-union / non-enum `Discriminant`→0; r2:
  Doc-2↔spec drift on the struct/union forward-decl — backported into Doc
  2 D2; r3 APPROVED). The §15 by-value-containment edge-set refinement (so
  the type-defs task is a pure transcription) was a follow-on Codex pass
  (APPROVED 4/4). D3 + D6 are `Spec-delta: no` (D6's printf shape was
  already §8; only its call-site cross-ref was added to §7.1).
- **Earlier this session**: **Phase-1 Level-2 plan docs ALL FILLED** at
  `docs/discussions/generic-c-backend/phase-1/` (committed `63b43cd`; each
  Codex-APPROVED):
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
- **In-flight**: **none.** Task a is closed; **next is to draft task b**
  (prelude + envelope — D2/§15 banks + D4/§16 panic + helpers).
- **Uncommitted state**: clean. Task-a chain committed: spec/registration
  (`0f6ea9c`), implementer scaffold (`a422d89`), determinism fix (`9e3f46b`),
  + this HANDOFF refresh. **None of the post-`74078b2` commits are pushed**
  (offer to push). The two standing untracked dirs stay untouched by rule —
  `tests/fixtures/playground-pl011/`, `crates/regseq-driver/tests/fixtures/`
  (experimentation; never commit, never modify).

### Next actions (in priority order)

1. **`task.py` tasks** — file the Phase-1 implementation tasks per the
   8-section template; Codex-reviewed; commit + push; implementer picks
   up. **(a) DONE — `regseq-backends-04` landed + assessed + closed**
   (`9e3f46b`); the `codegen-coverage-runner` harness is live (honest
   `not-pass` baseline; the emit seam awaits a body). **Next: draft task b**,
   then c→d→e, one per pass. The natural breakdown (input contract =
   `generic-c-backend.md` §§2–17, complete): (a) the
   **codegen-coverage-runner** crate +
   harness plumbing (Doc 3: pipeline, dual oracle, `manifest.toml`,
   ledger) — buildable before the backend so fixtures can be authored;
   (b) the **prelude + envelope** (D2/§15 banks, D4/§16 panic, helpers);
   (c) **types + layout** (§2/§13/§14 — the offset-faithful aggregate/enum
   emission + `_Static_assert` guards); (d) **bodies** (§7.1 terminators,
   §10 Rvalue/BinOp, §11 casts, §12 Const, §4/§5 places/operands);
   (e) **diag-log** (§8 + §7.1 interception) — the first end-to-end
   behavioral fixture. Sequence by the dependency (a→b→c→d→e); each task
   cites its spec sections + the Doc-1 matrix rows it closes.
2. **(Later) Phase-2 RegseqError** — describe at Phase-2 start; not before.

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
- **Coverage tracking = two axes, two homes (RESOLVED 2026-05-26, `111da9b`).**
  Doc-1's `status` column is the **scope/spec axis** (is the lowering pinned
  + in P1 scope); it was reconciled to the post-spec-delta truth (all
  `deferred`+`unspecced` gap rows → `covered`; now 169 covered / 7
  out-of-scope / 0 open). The **coverage axis** (does a committed green
  fixture exercise the row) lives **only in the ledger**
  (`.harness/generic-c/coverage_ledger.{json,md}`, regenerated by the runner
  from matrix × fixtures). So — UNLIKE the bridge matrix (one doc that
  doubled as the live tracker, edited per fixture batch) — **the Doc-1
  matrix stays stable**: tasks b–e author fixtures and the ledger
  auto-reflects green-ness; the `status` column is re-touched **only if the
  spec changes** (new construct specced → new `covered` row; punt to a later
  phase → `out-of-scope`/`deferred`). Needs **no harness change** (the
  runner already: `covered`+no-green → not-pass; `out-of-scope` → n/a; any
  residual `unspecced` → not-pass). Gate `pass` ⇔ every `covered` row has a
  green fixture + no fixture FAILs. (Per-row Doc-1 `notes`/`spec ref` cells
  were not exhaustively repointed at the flip — historical; authoritative
  lowering = the spec-delta §§7.1/10–17 per Doc-2 D1–D12.)
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
| P1 plan docs | entry + 3 docs (surface+matrix / decisions / verification) | ALL FILLED (each Codex APPROVED) | `63b43cd` | `phase-1/01` 170-row matrix+gaps, `02` D1–D12 decisions, `03` codegen-coverage harness |
| P1 spec delta | Doc-2 decisions → canonical `generic-c-backend.md` | done (Codex APPROVED) | (uncommitted) | gc §§10–17 (Rvalue/casts/Const/enum/field-offset/decl-order/panic/symbols) + §7.1 terminators + §2/§5/§7 edits |
| P1 impl | Rust surface + diag-log → C, + verification harness | in progress (task a `regseq-backends-04` DONE `9e3f46b` — harness live; b–e to draft) | `a422d89`+`9e3f46b` | generic-C backend MVP + codegen-coverage harness |

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
3. `docs/discussions/generic-c-backend/roadmap.md` — phase sequence + exit
   gates + the front-end↔backend dependency model.
4. **`docs/discussions/regseq-model/generic-c-backend.md` §§1–17 — the
   canonical C-emission spec; the COMPLETE input contract for
   implementation** (read this before writing any task or code).
5. The Phase-1 plan docs `docs/discussions/generic-c-backend/phase-1/`:
   **Doc 1** (`01-…`) the construct/coverage matrix + gap list (the Doc-1
   row IDs each task closes), **Doc 2** (`02-…`) the D1–D12 decisions (the
   *why* behind each spec rule), **Doc 3** (`03-…`) the verification flow
   (the `codegen-coverage-runner` harness the impl is gated by).
6. Only if needed: the archived bridge HANDOFF
   (`.harness/archive/2026-05-26-bridge-coverage-runner/HANDOFF.md`) for
   methodology depth or front-end questions.

---

## § First-action recommendations by user input (STABLE)

**"Let's continue / what's next"** → § Current state → Next actions
(currently: **file the Phase-1 `task.py` implementation tasks** — the
plan + spec are complete and committed; see the Next-actions a→e sequence).

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
- 2026-05-26: **Phase-1 plan docs committed** (`63b43cd`,
  `gc: Phase-1 Level-2 plan docs — matrix, decisions, verification`; the
  three docs + HANDOFF refresh; playground dirs left untracked). Not
  pushed (commit-only requested).
- 2026-05-26: **Phase-1 spec delta** applied to canonical
  `docs/discussions/regseq-model/generic-c-backend.md` (every Doc-2
  `Spec-delta: yes`). In-place: §7 identifier policy gains type-name
  derivation (D1); **§7.1 NEW** per-terminator emission replaces the old
  "deferred" paragraph (D5, incl. diag-log call-site interception cross-ref
  for D6); §2 enum/struct/union + §5 Constant rows point into the new
  sections. Appended **§10** Rvalue+BinOp/UnOp (D7), **§11** casts (D8),
  **§12** `Const` initializers (D11), **§13** enum offset-faithful
  representation (D9, generalizing §9.8/§9.9), **§14** field-offset
  reproduction (D10), **§15** declaration-order + C-ABI prototypes +
  `c_variadic` (D2), **§16** panic + `AssertKind` table (D4), **§17**
  symbol/linkage (D12). §1–§9 numbering kept intact (only external ref is
  to §8); §9 gained a forward-note. Codex review APPROVED after 3 rounds
  (r1: bank-4 must extern-declare ALL forward-ref statics not just foreign
  + bank-2 needs `union <T>;` vs `struct <T>;` + non-enum `Discriminant`→0
  per rvalue.rs:62-66; r2: backport the struct/union forward-decl
  distinction into Doc-2 D2 to kill doc↔spec drift; r3 APPROVED). D3/D6 are
  `Spec-delta: no`. **Uncommitted** (spec delta + Doc-2 backport +
  this HANDOFF refresh; no commit requested). **Next: `task.py` tasks**
  (harness crate → prelude/envelope → types/layout → bodies → diag-log).
- 2026-05-26: **§15/D2 refinement** — enumerated the **by-value-containment
  edge set** for the bank-3 type-definition topo-sort (the only
  load-bearing emission order; instance/fn/static order is forward-declared
  → F4-deterministic only). Pinned in both `generic-c-backend.md` §15 and
  Doc-2 D2 (kept in sync): edge `T→U` iff `U` is a `FieldLayout.ty`
  (struct/union/tuple/enum-payload/closure-capture) **or** a `Ty::Array`
  element reached that way; **no** edge through `Ref`/`Ptr`/`FnPtr`
  (forward-declarable) or `FnDef` (ZST). Closes the precision gap so the
  types/layout task is a pure transcription. Codex APPROVED (4/4:
  C-correct / IR-faithful / spec↔Doc-2 agree / no new contradiction).
- 2026-05-26: **Phase-1 task a filed** — `regseq-backends-04`
  (`.harness/tasks/regseq-backends/04-codegen-coverage-runner-scaffold.md`,
  group `regseq-backends`, phase `1b`, deps `regseq-backends-03` +
  `test-ir-fixtures-01`). The `codegen-coverage-runner` scaffold per Doc 3:
  6-stage pipeline (bridge → emit → snapshot → cc → run → behavioral) with
  per-stage failure attribution; dual oracle; `manifest.toml` schema +
  discovery; Doc-1 matrix parser (grouped-range IDs `BO-1..5` taken
  verbatim) + coverage ledger (`.harness/generic-c/coverage_ledger.{json,md}`,
  3 drift directions + Doc-3 §6 verdict); host-`cc` toolchain
  (`-std=gnu11 -fwrapv -Wall`, hermetic temp dir). Adds the **emit seam**
  `regseq_backends::emit_generic_c(&RegseqModel) -> Result<String, EmitError>`
  as a `NotImplemented` stub (tasks b–e implement the body; the harness is
  its first consumer). Scope calls (Codex-endorsed): seam in `regseq-backends`
  now; `kind="ir"` deferred; no corpus ships (golden.c needs emit → honest
  `not-pass` baseline); per-stage tests substitute for the impossible
  whole-loop smoke. Codex 2-round (r1 APPROVED_WITH_MINOR_FIXES → 4 fixes →
  r2 APPROVED). **Open process question for the architect**: the Doc-1
  status-flip workflow (resolved `unspecced` rows must read `covered` for
  `pass` to be reachable) — not decided; harness machinery correct either
  way. Spec + `tasks.json` + this HANDOFF refresh committed together (not
  pushed). **Next: draft task b** (prelude + envelope, D2/§15 banks +
  D4/§16 panic + helpers).
- 2026-05-26: **Phase-1 task a CLOSED** — implementer landed
  `regseq-backends-04` (`a422d89`): the `codegen-coverage-runner` crate
  (6-stage pipeline / dual oracle / manifest schema / Doc-1 matrix parser /
  coverage ledger / host-`cc` toolchain) + the `regseq_backends::emit_generic_c`
  seam stub + 24 tests + the empty-corpus README. **Assessment** (trust-but-
  verify): fmt / clippy (2 crates, all-targets) / 24 runner tests / Mode-A
  baseline run (176 rows, verdict `not-pass`, exit 1, `cc` recorded) all
  green; diff scope clean (matches the spec file set); the emit seam is
  faithful + well-documented; the impl even handled an unspecced edge
  (`|` inside a matrix inline-code cell). **One load-bearing defect caught +
  fixed** (`9e3f46b`): the committed `coverage_ledger.json` carried a per-row
  wall-clock `last_run`, so every run rewrote all 176 rows — non-deterministic,
  violating Doc 3 §4 byte-stability + defeating §5's drift-diff purpose. Root
  cause = a Doc-3 §5↔§4 tension (§5's row schema listed `last_run`); fix drops
  the field (ledger = pure function of matrix × results) + reconciles Doc 3 §5
  + a determinism note in `ledger.rs`; verified byte-stable by running twice.
  Lesson saved: [[codegen-ledger-determinism]] (run the producer twice when
  assessing a determinism-bearing committed artifact — same applies to the
  forthcoming golden `.c` snapshots). Implementer hand-set `status: done`
  (no task.py `completed` history event); end state correct. **Next: draft
  task b** (prelude + envelope).
- 2026-05-26: **Doc-1 status-flip question RESOLVED** (`111da9b`). The spec
  delta resolved Doc-1's entire gap list, so its design-time statuses were
  stale; reconciled in one pass — all 19 `deferred` + 83 `unspecced` rows →
  `covered` (169 covered / 7 out-of-scope / 0 open). Decision: matrix
  `status` = scope/spec axis (stable post-delta); coverage axis tracked live
  by the ledger; b–e author fixtures, no per-task matrix edits (cleaner than
  the bridge's per-batch matrix flips — the ledger artifact makes the
  separation possible). No harness change. Recorded in § Decisions locked +
  a Doc-1 header note. **Next: draft task b.**
