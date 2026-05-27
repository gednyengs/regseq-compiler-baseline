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

- **Last updated**: 2026-05-27
- **Workstream status**: Phase-1 **planning + spec delta complete**
  (all 3 Level-2 plan docs + the `generic-c-backend.md` spec delta filled
  + Codex-approved). The bridge front-end gate PASSES (`1a6a996`, archived
  handoff). Next is **`task.py` tasks** → implementation.
- **Last completed**: **Phase-1 task c1 CLOSED (`regseq-backends-06`,
  implementer `f410115`)** — assessed: fmt/clippy + **34 tests** green (19
  backend units + 3 behavioral layout tests that bridge real
  `repr(Rust)`/union/tuple/array aggregates → emit → **compile** [the §14
  `_Static_assert` guards holding *is* the layout-fidelity proof] + a
  negative wrong-offset test; + the existing suite); emit byte-stable; ledger
  unchanged; diff scope clean. Implementation is high quality (all 4 filed
  fixes landed; declarator handling sophisticated; determinism via Vec-order
  not HashSet). **A soundness Codex pass found 4 real EDGE-CASE bugs** beyond
  the tested common path → **follow-up `regseq-backends-07`**: (#2, HIGH/panic)
  `Node::Slice` has no discovery edge to its element aggregate → panic on
  `&[(tuple)]`/`&[[Struct;N]]` reachable only via the slice; (#4, MED/silent)
  `const` prefixed to the whole rendered inner declaration → wrong qualifier
  for nested pointer pointees; (#1, low) synthetic `_padN` can collide with a
  real field named `_pad0`; (#3, low) nested fn-ptr *return* declarator emits
  invalid C. **None block the d→e critical path** (the first green fixture
  won't hit them); c1's verified scope is correct. **Follow-up
  `regseq-backends-07` FILED** (Codex-reviewed; the 4 fixes — discovery/
  ordering edge split incl. the array-element ordering case, declarator
  `const` placement, collision-proof pad names incl. union `_pad`, recursive
  fn-ptr-return declarator). **The two canonical-doc bugs the reviews
  surfaced are FIXED** (§14/Doc-2-D10 alignment + §15 fat-pointer/array
  exception — see § Decisions locked). Was: **task c1 FILED
  (`regseq-backends-06`)** — "Generic-C type system + field-offset emission":
  fills banks 2–3 (the
  `Ty`→C spelling + D1 naming, the §14/D10 field-offset aggregate emitter,
  the §15 bank-2/3 forward-decls/definitions + by-value-containment topo-sort,
  the §3.2 `rust_slice_<T>`/`trait_obj_<Trait>` shapes). **The original "(c)"
  was SPLIT** (user-approved): c1 = the type-system foundation (sized,
  non-enum, non-`dyn`); **c2 (next) = enum representation §13/D9** (Doc-2
  top-risk, reuses c1's D10 primitive); `dyn`/vtable/dispatch + field-access
  are later/task-d. c1 produces **no green fixture** (needs a body+diag-log,
  d/e); verified backend-internally (emit aggregate defs → compile → the
  `_Static_assert` layout guards holding *is* the proof). Codex 2-round (r1
  NOT_READY → 4 fixes [alignment via `__attribute__((aligned(layout.align)))`
  not `_Alignas`/`aligned(n)`; fat-pointer topo edge for `Ref/Ptr`-to-unsized;
  `generic_args`+`args_key_for_generic_args(model, …)`; discovery-roots
  hand-off to d] → r2 APPROVED_WITH_MINOR_FIXES, both applied). **FOLLOW-UP
  TO TRACK**: the review surfaced a **latent bug in canonical §14 / Doc-2
  D10** — `repr(packed(n))` → `aligned(n)` over-aligns (true align =
  `min(max_field_align,n)` = `layout.align`); the task spec **overrides** it +
  flags a small doc fix to §14 + D10. Spec at
  `.harness/tasks/regseq-backends/06-generic-c-types-and-layout.md`.
  **Earlier: Phase-1 task b CLOSED (`regseq-backends-05`,
  implementer `5bfdcb8`)** — assessed **clean, no defects** (contrast task
  a's determinism hole). Gates all green: fmt / clippy (touched crates,
  all-targets) / tests — 6 backend structural units + **5 behavioral C tests**
  (real compile+run via `toolchain`: `regseq_panic`→stderr+abort; saturating
  f64→i32 = INT32_MAX/0/INT32_MIN; `*WithOverflow` = 5/0/1) + adapted
  `emit_seam` (2) + existing 24; prelude `cc -fsyntax-only -Wall`
  warning-free; emit byte-stable; ledger **unchanged** (169
  covered-without-green, `not-pass`); diff scope clean (no matrix/ledger/spec
  edits). Faithful: all 8 §16 `AssertKind` messages verbatim; §10.1/§11
  helpers correct; the 3 filed fixes landed; `emit_seam` was *strengthened*
  (now proves the pipeline advances past `Stage::Emit`→`Stage::Snapshot`),
  not gutted. Implementer hand-set `status: done` (no task.py `completed`
  event); end state correct. **Was: Phase-1 task b — "Generic-C prelude +
  5-bank emission envelope + panic/helpers": the
  model-INDEPENDENT scaffolding behind the `emit_generic_c` seam (§15 5-bank
  driver; static prelude = includes + `struct rust_str` + `regseq_panic`
  [`static inline`] + the §16 `AssertKind` catalog; the §10/§11
  `*WithOverflow` + saturating-`FloatToInt` helper block, `static inline`,
  f16/f128 behind source-level `#if`). Banks 2–5 / per-`T` fat structs /
  use-site lowering are c/d/e; no green fixture from b. Codex
  APPROVED_WITH_MINOR_FIXES → 3 mechanical fixes applied (pin `static inline`
  panic; pin source-level `#if` f16/f128 gating; resolve the hook
  error-contract contradiction — hooks return `Ok("")`, emit never returns
  `NotImplemented` on the happy path); filed without a 2nd round (fixes were
  verbatim-responsive, no judgment). Spec at
  `.harness/tasks/regseq-backends/05-generic-c-prelude-and-envelope.md`.
  **Earlier this session: task a CLOSED (`regseq-backends-04`)** — the
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
- **In-flight**: **task c1 CLOSED**; **task `regseq-backends-07` (c1 robustness
  fixes) CLOSED — implementer `34a4ae1`, assessed clean** (see below);
  **task c2 (`regseq-backends-08`) CLOSED — implementer `3b66248`, assessed
  CLEAN, Codex soundness pass SOUND (no issues).** Banks 2-3 now emit
  definitions for **every** sized aggregate (struct/union/tuple/closure/array/
  slice/**enum**); `dyn`/vtable + bodies (banks 4-5) remain. **Next to draft:
  task d** (bodies — §7.1 terminators / §10 Rvalue / §11 casts / §12 Const /
  §4-§5 places-operands; **must also widen discovery roots** to signature/
  local/static types per the c1 hand-off), then e (diag-log, first green
  fixture).
  - **`regseq-backends-08` assessment (`3b66248`):** the §13/D9 union-of-views
    enum representation in a new `enum_emit.rs`, reusing the §14 primitive
    (refactored to a shared `emit_offset_fields` + `GuardDesignator`). fmt /
    clippy / **all tests green** (37 backend-lib + 10 `generic_c_layout`,
    incl. the required shifted-alignment `enum Shift { A(u8), B(u32) }` + the
    wrong-offset injection + §9.8/§9.9 collapse + Empty + byte-identical);
    determinism stable; diff scope matches the spec; **ledger unchanged**; c1
    non-enum output byte-for-byte preserved (`member_prefix:""`). **Codex
    soundness pass: SOUND, no issues** (verified fidelity across all
    EnumShape×TagEncoding, shifted-align/non-zero-start views, sizeof/alignof,
    collapse logic, panic paths; re-ran the tests). No follow-ups from c2.
  - **`regseq-backends-07` assessment (`34a4ae1`):** verified in an isolated
    worktree (the implementer was concurrently editing the main tree for 08):
    fmt / clippy (`-D warnings`) / **all tests green** (32 backend-lib + 37
    runner), determinism byte-stable, diff scope matches the spec, **ledger
    untouched**. All 4 fixes correct (traced the edge split, `& &u32` →
    `const uint32_t *const *`, the `PadNamer`, the recursive `fn_ptr_decl`).
    A **Codex soundness pass returned UNSOUND with 3 findings — 2 are FALSE
    POSITIVES**: #1 (slice-of-refs-to-aggregate panic) + #2 (fnptr-signature-
    only aggregate panic) assume the aggregate isn't discovered, but the bridge
    `nominal_adt_instances_pass` (`bridge/mod.rs:2375-2432`) makes **every**
    interned nominal `Ty` an `InstanceKind` root, and `discover()` roots on all
    of them — so `names.agg_name` can't panic. **Verified empirically** (bridged
    Codex's exact `&[[&Inner;4]]` + fnptr-sig shapes → `emit` returns `Ok`, both
    aggregates emitted). **1 real but minor finding (#3):** `&fn() -> i32`
    emits `const` on the fn's **return type** instead of the fn-pointer **value**
    (`const int32_t (*(*p))(void)` vs `int32_t (*const *p)(void)`) — a *silent
    wrong qualifier* (compiles, no miscompile/panic), same class as the original
    #4 but for the fn-ptr-pointee sub-case Fix B didn't cover. The implementer
    **documented it in-code** as out-of-verified-scope (`types.rs` FnPtr branch).
    **Does NOT block c2 or the d→e critical path.** **CLOSED via
    `regseq-backends-09`** (implementer `c25f7d6`, assessed clean): threaded
    `self_const` into `fn_ptr_decl` → `(*const decl)(params)`; `&fn() -> i32`
    now emits `int32_t (*const (*p))(void)` (const on the fn-ptr value).
    fmt/clippy/tests green (40 backend-lib + 11 layout; +3 const-case units
    [shared-ref / `&mut` / `*const`] + 1 behavioral compile, all asserting the
    exact spelling + the negative "no const on return"); 07 Fix D recursive
    path preserved; diff scope matches; ledger unchanged. No Codex soundness
    pass (surgical, fully traced, no invariant relaxed; the spec review already
    C11-probed the spelling).
  **DOC-FIX FOLLOW-UP TO TRACK (from the c2 review):** the c2 spec **overrides**
  the §13 / Doc-2 D9 *literal* enum representation ("tag field + payload `union`
  at `union_start`, fields at `offset−union_start`") — Codex r1 showed it is
  **not C-placeable** for shifted-alignment `Direct` enums (`enum E { A(u8),
  B(u32) }`: tag@0, A@1, B@4 — a naturally-aligned `uint32_t` payload cannot
  live in a `union` placed at offset 1). c2 uses the strictly-general
  **union-of-views** instead (the tag + each non-fieldless variant as full-object
  §14 views at **absolute** offsets in one `union`, collapsed inline for a
  single view → matches §9.8/§9.9). **The §13 + Doc-2 D9 doc fix is APPLIED**
  (`5d422fe`, Codex-APPROVED; the union-of-views replaces the broken
  `union_start` form + corrects a stale `_Alignas(align)`→`aligned(align)` in
  D9 — mirrors the c1 §14/§15 + D10 fixes in `fe5b9ab`). No open doc-fix item.
- **Uncommitted state**: clean (everything committed; only the standing
  playground dirs untracked). **`origin/main` is at `a8b676a`** (the prior
  local chain was pushed this session). The unpushed local commits are,
  oldest→newest: `6eb1c20` (c2 / `regseq-backends-08` filing) → the HANDOFF
  refresh commit for this hand-off. **Offer to push at session start.** The two
  standing untracked dirs stay untouched by rule —
  `tests/fixtures/playground-pl011/`, `crates/regseq-driver/tests/fixtures/`
  (experimentation; never commit, never modify).

### Next actions (in priority order)

1. **`task.py` tasks** — file the Phase-1 implementation tasks per the
   8-section template; Codex-reviewed; commit + push; implementer picks
   up. **(a) DONE — `regseq-backends-04`** (`9e3f46b`); harness live. **(b)
   DONE — `regseq-backends-05`** (`5bfdcb8`); prelude + 5-bank envelope +
   panic/helpers; emit seam live. **(c1) DONE — `regseq-backends-06`**
   (`f410115`); type system + field-offset, banks 2–3 live; assessed clean on
   the common path; a soundness pass found 4 edge-case bugs → **`regseq-backends-07`
   DONE** (c1 robustness fixes; `34a4ae1`, assessed clean). **The original
   "(c) types + layout" was SPLIT into c1 (foundation, non-enum) + c2 (enum
   §13/D9).** **(c2) DONE + assessed CLEAN — `regseq-backends-08`** (`3b66248`,
   Codex soundness SOUND); enum representation via the union-of-views form.
   **Next: draft task d** (bodies — terminators / Rvalue / casts / Const /
   places-operands; widen discovery roots), then e (diag-log, first green
   fixture). The
   natural breakdown (input contract =
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
- **Task "(c) types + layout" SPLIT into c1 + c2 (2026-05-26, user-approved).**
  c1 (`regseq-backends-06`) = the type-system foundation: `Ty`→C spelling +
  D1 naming + the **§14/D10 field-offset emitter** (shared primitive) + the
  §15 bank-2/3 topo-sort + the §3.2 `rust_slice`/`trait_obj` shapes — for
  **sized, non-enum, non-`dyn`** types. c2 = **enum representation §13/D9**
  (Doc-2 top-risk, "was WRONG" on first contact) **reusing c1's D10
  primitive**. `dyn`/vtable/dispatch + field-*access* are later/task-d.
  Rationale: isolate the top-risk enum for focused review; c1 unblocks the
  critical path to the first green fixture (primitives → d → e) without
  waiting on enum. Plan: a→b→**c1→c2**→d→e.
- **Canonical-doc fixes (FIXED 2026-05-26, surfaced by the c1 reviews).**
  Two latent bugs in the frozen design docs, both corrected:
  (a) **§14 / Doc-2 D10 alignment** — said `repr(packed(n))` → `aligned(n)`
  (over-aligns; true align = `min(max_field_align, n) = layout.align`) and
  `_Alignas` on a tag (invalid). Now: `__attribute__((aligned(layout.align)))`
  (+ `packed`), always `layout.align`.
  (b) **§15 fat-pointer exception** — the blanket "no edge through Ref/Ptr"
  missed that a slice fat struct whose element is a `Ty::Array(base,N)` emits
  `base (*)[N]`, and a C array needs its element **complete** even behind a
  pointer → an ordering edge to `base` (the non-array element still needs only
  a forward-decl). Now pinned in §15. (Both surfaced by Codex; the
  `regseq-backends-07` fixes implement (b).)
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
| P1 impl | Rust surface + diag-log → C, + verification harness | in progress (a+b+c1+07+c2+09 DONE+assessed; d/e to draft — "(c)" split into c1+c2; banks 2-3 complete for all sized aggregates incl. enums; const-correctness complete) | `c25f7d6` (09) | generic-C backend MVP + codegen-coverage harness |

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

**"Let's continue / what's next"** → § Current state → Next actions.
**Currently (fresh-session entry point):** Phase-1 tasks **a, b, c1,
`regseq-backends-07`, and c2 (`regseq-backends-08`) all DONE + assessed clean**
(07: Codex soundness over-flagged 2 false positives + 1 real-minor #3; c2:
Codex soundness SOUND, no issues; see § Current state / § Update log). **The
next task to DRAFT is d** (bodies — §7.1 terminators / §10 Rvalue / §11 casts /
§12 Const / §4-§5 places-operands; d→e is the critical path to the first green
fixture; d must also widen discovery roots to signature/local/static types).
The §13/D9 doc fix is **APPLIED** (`5d422fe`). The 07 #3 fn-ptr-value const
follow-up is **CLOSED** (`regseq-backends-09`, `c25f7d6`, assessed clean) — so
**all filed generic-C backend tasks (a, b, c1, 07, c2, 09) are done + assessed**
and the const-correctness story is complete. **`origin/main` is at `71be0f0`**;
the unpushed tail is `c25f7d6` (09 impl) + this HANDOFF commit — offer to push.

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

- 2026-05-27: **`regseq-backends-09` CLOSED** (07 #3 fn-ptr-value const
  placement; implementer `c25f7d6`). Threaded a `self_const` flag into
  `fn_ptr_decl` (emits `(*const decl)(params)`) and passed `top_const` through
  the `Ty::FnPtr` arm of `declare_qualified`, dropping the old
  `format!("const {inner}")` prefix. `&fn() -> i32` now emits
  `int32_t (*const (*p))(void)` (const on the fn-pointer **value**), not
  `const int32_t (*(*p))(void)` (return type). Assessed in the main tree
  (tree clean): fmt / clippy / **all tests green** (40 backend-lib + 11
  `generic_c_layout`), with 3 new const-case units (shared-ref / `&mut` /
  `*const`, each asserting the exact spelling + the negative "no const on
  return") + 1 behavioral compile; the 07 Fix D recursive fn-ptr-return path
  is byte-identical (`star="*"`); diff scope = `types.rs` + tests + `tasks.json`;
  **ledger unchanged**. No Codex soundness pass (surgical declarator change,
  fully traced across all 4 const cases, comprehensive tests, no load-bearing
  invariant relaxed; the spec review had already C11-probed the spelling).
  **All filed generic-C backend tasks are now done + assessed. Next: draft
  task d (bodies).**
- 2026-05-27: **Phase-1 task c2 (`regseq-backends-08`) CLOSED** (enum
  representation; implementer `3b66248`). The §13/D9 **union-of-views** form in
  a new `generic_c/enum_emit.rs`, reusing the §14 primitive (refactored to a
  shared `emit_offset_fields` + `GuardDesignator` carrying a member-path
  prefix). Assessed in the main tree (implementer done, tree clean): fmt /
  clippy (`-D warnings`) / **all tests green** (37 backend-lib + 10
  `generic_c_layout` + the rest), incl. the required shifted-alignment
  `enum Shift { A(u8), B(u32) }` (tag@0/A@1/B@4), the wrong-offset injection
  (guards bite), §9.8/§9.9 inline collapse, `Empty`, and byte-identical
  determinism. Diff scope = `enum_emit.rs` + `layout_emit.rs` refactor +
  `mod.rs` bank-3 wiring + tests + `tasks.json`; **ledger unchanged**; c1
  non-enum output byte-for-byte preserved (`member_prefix:""`). **Codex
  soundness pass: SOUND, no issues** — verified fidelity across every
  EnumShape×TagEncoding, shifted-align/non-zero-start views, sizeof/alignof
  (views padded to size, union max, outer `aligned(layout.align)`), the
  one-view/zero-view collapse, panic paths; re-ran the suite. **Banks 2-3 now
  emit every sized aggregate incl. enums.** No c2 follow-ups. **Next: draft
  task d (bodies).**
- 2026-05-27: **`regseq-backends-09` FILED** (`25d0a3d`, Codex APPROVED) — the
  07 #3 fn-ptr-value const placement follow-up (user chose "file now"). A small
  surgical declarator fix: thread `self_const` into `fn_ptr_decl` so a `Ref`/
  `Ptr` to a fn-pointer pointee emits `const` on the fn-pointer **value**
  (`int32_t (*const (*p))(void)`) not the return type, and drop the
  `const`-prefix in `declare_qualified`'s `FnPtr` arm. Independent of task d;
  awaits an implementer. Codex compiled a C11 probe confirming the target
  spelling enforces read-only; the only review fix was stale doc anchors. The
  local chain through the c2 close was **pushed** (`origin/main` = `55795b0`).
- 2026-05-27: **Phase-1 task `regseq-backends-07` CLOSED** (c1 robustness
  fixes; implementer `34a4ae1`). Assessed in an **isolated git worktree** at
  `34a4ae1` (the implementer was concurrently editing the main tree for c2, so
  a main-tree build would have been polluted): fmt / clippy (`-D warnings`) /
  **all tests green** (32 backend-lib + 37 runner), determinism byte-stable,
  diff scope = `generic_c/{mod,types,layout_emit}.rs` + tests + `tasks.json`,
  **ledger unchanged** (`not-pass`). The 4 fixes are correct: the discovery/
  ordering edge **split** (no false `ContainmentCycle` for `struct Inner { xs:
  &[Inner] }`; array-element ordering edge; cyclic array-element slice = clean
  error), the `declare_qualified` const placement (`& &u32` →
  `const uint32_t *const *`), the collision-proof `PadNamer` (struct `_padN` +
  union `_pad`), the recursive `fn_ptr_decl`. **Codex soundness pass: UNSOUND
  (3 findings) — but 2 are FALSE POSITIVES** (#1 slice-of-refs-to-aggregate +
  #2 fnptr-sig-only aggregate "panic": both assume the aggregate isn't
  discovered, but the bridge `nominal_adt_instances_pass` `bridge/mod.rs:2375`
  makes every interned nominal `Ty` an `InstanceKind` root and `discover()`
  roots on all of them — **verified empirically**: bridged Codex's exact
  `&[[&Inner;4]]` + fnptr-sig shapes → `emit` returns `Ok`, no panic). **1 real
  minor finding (#3):** `&fn() -> i32` emits `const` on the fn return type, not
  the fn-pointer value — a silent wrong qualifier (no panic/miscompile), same
  class as the original #4 for the fn-ptr-pointee sub-case Fix B didn't cover;
  the implementer documented it in-code as out-of-verified-scope. **Does not
  block c2 / d→e.** Lesson reinforced: a soundness Codex pass can over-flag —
  verify against the discovery-roots invariant + empirically before acting
  ([[assessment-soundness-pass]]). **Open: file a tiny follow-up for #3 or fold
  into a later hardening pass.**
- 2026-05-27: **Phase-1 task c2 FILED** — `regseq-backends-08`
  (`.harness/tasks/regseq-backends/08-generic-c-enum-representation.md`, group
  `regseq-backends`, phase `1b`, dep `regseq-backends-06`; commit `6eb1c20`).
  The enum half of the split "(c)": fills the bank-3 enum-body hook with the
  offset-faithful C enum representation (gc §13 / Doc-2 D9), reusing c1's
  §14/D10 field-offset primitive. **Representation only** — the tag
  *operations* (`Discriminant`/`SetDiscriminant`/`Downcast`/`Aggregate::Enum`/
  `Const::EnumValue`) + tag-value constants are **task d** (symmetric with c1
  deferring struct field access). Codex **2-round** (r1 NOT_READY → r2
  APPROVED_WITH_MINOR_FIXES, applied). **r1 caught a real soundness bug**: the
  literal §13/D9 "tag field + payload `union` at `union_start`, fields at
  `offset−union_start`" form is **not C-placeable** for shifted-alignment
  `Direct` enums (`enum E { A(u8), B(u32) }`: tag@0, A@1, B@4 — a
  naturally-aligned `uint32_t` payload cannot live in a `union` at offset 1).
  **Resolved with the union-of-views**: the tag + each non-fieldless variant
  become full-object §14 views (fields at **absolute** offsets → naturally
  aligned, **no packing, no `union_start`**), all sharing one `union`,
  **collapsed inline** for a single view (→ §9.8 tag-only struct, §9.9
  single-field struct). The spec **overrides** §13/D9; **the §13 + Doc-2 D9
  doc fix is APPLIED** (`5d422fe`, Codex-APPROVED — also corrected a stale
  `_Alignas(align)`→`aligned(align)` in D9; mirrors the c1 `fe5b9ab` fixes).
  Pre-grounded the "tag is always a prefix for `Direct`" premise against pinned
  rustc (`rustc_abi/src/layout.rs:803-824`, `StructKind::Prefixed`; variant
  fields at `prefix_size.align_to(prefix_align)`, lines 1260-1264). Codex r2
  re-verified the union-of-views against the same source. (A stray `enum_layout`
  ELF Codex compiled while probing was removed pre-commit, not tracked.)
  **Next: draft task d** (bodies).
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
- 2026-05-26: **Phase-1 task b FILED** — `regseq-backends-05`
  (`.harness/tasks/regseq-backends/05-generic-c-prelude-and-envelope.md`,
  group `regseq-backends`, phase `1b`, dep `regseq-backends-04`). The
  model-independent generic-C scaffolding behind the `emit_generic_c` seam:
  the §15 5-bank TU driver (banks 2–5 = empty c/d hooks), the static prelude
  (includes + `struct rust_str` + `regseq_panic` + the §16 `AssertKind`
  message catalog), and the §10/§11 helper block (`*WithOverflow` +
  saturating `FloatToInt`, `static inline`, f16/f128 behind source-level
  `#if`). Scope calls (Codex-endorsed): only the monomorphic `rust_str` in
  the prelude (slices/trait-objs → c); fixed helper block (b = defs, d =
  use); `AssertKind` routine+catalog in b, per-site operand formatting in d.
  Verification is backend-internal (prelude/helpers/panic compile + behave
  via `toolchain::compile_and_run`; emit byte-stable) — **no green fixture**
  (that needs a body + diag-log, d/e). Codex APPROVED_WITH_MINOR_FIXES → 3
  mechanical fixes applied (static-inline panic; source-level `#if`
  f16/f128; hook-contract: hooks return `Ok("")`, emit never returns
  `NotImplemented`); filed without a 2nd round. **Next: draft task c**
  (types + layout — §2/§13/§14: the offset-faithful aggregate/enum emission
  + `_Static_assert` guards + the bank-3 by-value topo-sort).
- 2026-05-26: **Phase-1 task b CLOSED** — implementer landed
  `regseq-backends-05` (`5bfdcb8`): `regseq-backends/src/generic_c/{mod,prelude}.rs`
  (the §15 5-bank driver + bank-1 static prelude + helper block), `lib.rs`
  slimmed to a re-export, a 5-test behavioral suite in
  `codegen-coverage-runner/tests/generic_c_prelude.rs`, and the `emit_seam`
  test adapted (emit now returns `Ok`). **Assessment: clean, no defects.**
  fmt / clippy / all tests green; the behavioral tests *compile + run real C*
  asserting the §16 panic contract, §11 saturating cast, §10.1 overflow; the
  prelude is `-Wall`-warning-free; emit is byte-stable; the ledger is
  unchanged (no coverage added — correct for task b). The emit seam is now
  **live** (returns the bank-1 prelude; banks 2–5 are empty c/d hooks). No
  Codex soundness pass needed (no load-bearing invariant relaxed; the
  change is additive + well-tested). **Next: draft task c** (types + layout).
- 2026-05-26: **Phase-1 task "(c)" SPLIT + task c1 FILED** —
  `regseq-backends-06` ("Generic-C type system + field-offset emission":
  `Ty`→C spelling + D1 naming + the §14/D10 field-offset aggregate emitter +
  the §15 bank-2/3 forward-decls/definitions topo-sort + the §3.2
  `rust_slice`/`trait_obj` shapes; sized non-enum non-`dyn`). User approved
  splitting the original "(c)" into **c1 (foundation) + c2 (enum §13/D9,
  Doc-2 top-risk, reuses c1's D10 primitive)**; `dyn`/vtable/dispatch +
  field-access are later/task-d. No green fixture (needs body+diag-log, d/e);
  verified by emit→compile→`_Static_assert` layout guards. Codex 2-round (r1
  NOT_READY → 4 fixes: alignment via `__attribute__((aligned(layout.align)))`;
  fat-pointer topo edge for `Ref/Ptr`-to-unsized; `generic_args` +
  `args_key_for_generic_args(model,…)`; discovery-roots hand-off to d → r2
  APPROVED_WITH_MINOR_FIXES, both applied). Review **surfaced a latent §14/
  Doc-2-D10 alignment doc bug** (`packed(n)`→`aligned(n)` over-aligns); the
  task overrides it + a small doc fix is flagged (§ Decisions locked).
  **Next: draft task c2** (enum representation).
- 2026-05-26: **Phase-1 task c1 CLOSED + soundness follow-up `regseq-backends-07`
  FILED + canonical-doc fixes.** c1 (`f410115`) assessed: fmt/clippy + 34
  tests green (incl. 3 behavioral tests that bridge real
  `repr(Rust)`/union/tuple/array aggregates → emit → compile [§14
  `_Static_assert` guards = layout-fidelity proof] + a negative wrong-offset
  test); byte-stable; ledger unchanged; high-quality impl (all 4 c1-filed
  fixes landed; determinism via Vec-order). **A Codex soundness pass found 4
  real edge-case bugs** (UNSOUND verdict): #2 (HIGH/panic) `Node::Slice` has
  no discovery edge to its element aggregate → panic on `&[(tuple)]`/
  `&[[Struct;N]]`; #4 (MED/silent) `const` prefixed to the rendered inner
  declaration → wrong qualifier for nested pointers; #1 `_padN`/union-`_pad`
  collide with real fields named that; #3 nested fn-ptr-return declarator
  invalid. None block the d→e path. Filed `regseq-backends-07` (Codex 3-round:
  the array-slice-element completeness distinction [pointer-to-struct =
  forward-decl ok; pointer-to-array = element must be complete] + the union
  `_pad` were the deeper catches). **Fixed two canonical-doc bugs the reviews
  surfaced**: §14/Doc-2-D10 alignment (`aligned(layout.align)` not
  `aligned(n)`/`_Alignas`-on-tag) + §15 fat-pointer/array-element ordering
  exception. **Next: draft task c2** (enum §13/D9).
