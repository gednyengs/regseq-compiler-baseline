# Stage 13 Status

Last updated: 2026-05-07 (**STAGE 13 CLOSED AND
MERGED**). Phase A locked all 8 main D-tags + 41
sub-decisions across a single combined cluster with
1 D-tag closed-by-absorption (D-13-8 → D-13-9.6).
Phase B per-stage Codex review closed at R4
MINOR-with-NIT-only-and-Lockability:yes. Merger
contract per README §5.5 (with D-13-9.3 spec-split
amendment) completed steps 1-7; tier-3 post-merge
Codex review pass R1-R4 closed at R4 APPROVE / 0
issues / Lockability:yes on 2026-05-07.

The current authoritative spec lives at top-level
`../spec.md` + `../pretty-printer.md` (the new
co-equal merger artifact introduced by D-13-9). This
`stage-13/spec-snapshot.md` and
`stage-13/pretty-printer-snapshot.md` are now frozen
historical artifacts (each with a "Snapshot at close
of stage 13" header).

Stage 13 = pretty-printing. Broadened by stage-11 D-11-K
+ stage-12 D-12-K cascades to a single coherent
rendering-decision surface covering 20 inbound items: 1
from stage 3 (`render_ty`), 12 from stage 11 / D-11-K
(9 public body-level renderers + 1 internal helper +
2 sigil wirings `_N` / `bbN`), 7 from stage 12 / D-12-K
(`render_item`, ItemKind variant rendering, Linkage
enum rendering, InlineHint enum rendering, Function-flag
+ lang-item-flag rendering, link_section decorator,
opaque_handle_eligible decorator). Plus §18.2 ownership-
table consolidation and §18.1 sigil-table review.

The current authoritative spec lives at top-level
`../spec.md`; this stage's working copy is
`stage-13/spec-snapshot.md` (copied from `../spec.md`
at Open phase per README §5.1 step 2).

## 0. Inbound expectations checklist

From `expectations.md`. Status flags:
- `[ ]` -- in scope; will close as a stage-13 D-tag locks.
- `[CLOSED]` -- already closed by an earlier stage's
  D-tag (none currently for stage 13).

### From stage 3 (1 item)

- [ ] **Fill `pretty.rs::render_ty` stub** with the
  stage-13 master rendering-style call + per-variant
  rendering for the closed 19-variant `Ty` enum (stage-3
  §15.1). Stage-3 D14 lean preserved as input, not
  binding. Will close under D-13-1 (master style) +
  D-13-5 (per-variant `render_ty` arms).

### From stage 11 / D-11-K (12 items)

Public renderer rows (9):

- [ ] **`render_place(&Place, &Body)`** -- D-5-K
  predecessor. Will close under D-13-1 (style) +
  D-13-6.1 (per-arm spec).
- [ ] **`render_operand(&Operand, &Body)`** -- D-6-K
  predecessor. Will close under D-13-1 + D-13-6.2.
- [ ] **`render_rvalue(&Rvalue, &Body)`** -- D-7-K
  predecessor. Will close under D-13-1 + D-13-6.3.
- [ ] **`render_statement(&Statement, &Body)`** --
  D-8-K predecessor. Will close under D-13-1 + D-13-6.4.
- [ ] **`render_terminator(&Terminator, &Body)`** --
  D-9-K predecessor. Will close under D-13-1 + D-13-6.5.
- [ ] **`render_call_callee(&CallCallee, &Body)`** --
  D-9-K predecessor. Will close under D-13-1 + D-13-6.5.
- [ ] **`render_switch_targets(&SwitchTargets, &Body)`**
  -- D-9-K predecessor. Will close under D-13-1 +
  D-13-6.5.
- [ ] **`render_assert_kind(&AssertKind, &Body)`** --
  D-9-K predecessor. Will close under D-13-1 + D-13-6.5.
- [ ] **`render_basic_block(&BasicBlock, &Body)`** --
  D-10-K predecessor. Will close under D-13-1 +
  D-13-2 (envelope) + D-13-6.6.

Internal helper (1):

- [ ] **`render_terminator_kind(&TerminatorKind,
  &Body)`** -- D-9-K predecessor; not part of §18.2
  public surface. Will close under D-13-6.5.

Sigil rendering wirings (2):

- [ ] **`_N` sigil for `LocalId`** -- stage-1
  inbound + stage-11 §25.3. Will close under D-13-2
  (envelope coverage) + D-13-8 (sigil-table review).
- [ ] **`bbN` sigil for `BlockId`** -- D-10-K.2 +
  stage-10 §24.3.4. Will close under D-13-2 + D-13-8.

### From stage 12 / D-12-K (7 items)

- [ ] **`render_item(ItemId)`** -- full-Item display
  renderer. Will close under D-13-1 + D-13-4 (Item-level
  shape).
- [ ] **ItemKind variant rendering** -- per-variant
  (Function / Static{mutability} / Const / Struct /
  Enum / Union). Will close under D-13-4.
- [ ] **`Linkage` enum rendering** -- 4-variant. Will
  close under D-13-4.
- [ ] **`InlineHint` enum rendering** -- 4-variant.
  Will close under D-13-4.
- [ ] **Function-flag + lang-item-flag rendering**
  (`is_unsafe`, `is_naked`, `is_phantom_data`,
  `is_unsafe_cell`, `is_manually_drop`). Will close
  under D-13-4.
- [ ] **`link_section` rendering decorator**. Will
  close under D-13-4.
- [ ] **`opaque_handle_eligible` rendering decorator**.
  Will close under D-13-4.

### Stage-internal items (3)

- [ ] **§18.1 sigil-table review.** Confirm or refine
  the canonical sigil set (in `../spec.md` §18.1) +
  reconcile with the in-code doc-comment table at
  `crates/regseq-model/src/pretty.rs:22-34`. Will close
  under D-13-8.
- [ ] **§18.2 per-method ownership-table consolidation.**
  Stage 13 owns ALL rendering ownership rows (the
  D14-deferred `render_ty` row, the 9 public body-level
  rows, the 1 internal helper row, the 2 sigil wirings,
  the 7 Item-level rows from D-12-K). Will close under
  D-13-8.
- [ ] **Worked-example specimens for snapshot
  stability.** Per-renderer canonical fragments, ≥1 per
  public renderer, to anchor insta snapshot tests. Will
  close under D-13-7.

## 1. D-tag allocation

D-tags allocated upfront from grounded rustc + project-
scaffold evidence (per `../README.md` §5.1 step 5).
Sub-decisions allocated during Phase A as evidence
demands.

| Tag      | Status     | Locked content (one-line summary)                                                                                                                                    |
|----------|------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| D-13-1   | [open]     | Master rendering style + method shape: Rust-source-style + `PrettyPrinter` methods returning `String` (vs constructor-style; vs free fns over `&mut W`).             |
| D-13-2   | [open]     | Body envelope + indentation + sigil placement: `fn $path(...) -> Ret { let _N: T; ...; bbN: { ... } }` + 4-space INDENT (rustc-converged candidate).                 |
| D-13-3   | [open]     | Span-rendering mode: off-by-default with toggle (rustc) vs always-on inline vs always-on out-of-line vs `PrettyPrinter` option-field (snapshot-tunable).             |
| D-13-4   | [open]     | Item-level rendering shape: Function-Item display + default-suppression rules + attribute-decorator syntax. 7-entry D-12-K bundle resolves under sub-decisions.      |
| D-13-5   | [open]     | `render_ty` per-arm specifications across the closed 19-variant `Ty` enum (stage-3 §15.1). Inherits D-13-1 master style.                                             |
| D-13-6   | [open]     | Body-level renderer per-arm specifications (12 D-11-K items). Per-variant arms; place-projection chain shape; Aggregate/Adt rendering. Inherits D-13-1 master style. |
| D-13-7   | [open]     | Worked-example specimens for snapshot stability. ≥1 canonical fragment per public renderer; insta snapshot anchor.                                                   |
| D-13-8   | [open]     | §18.1 sigil-table review + §18.2 per-method ownership-table consolidation. Operational structural cleanup.                                                            |

Locked-tag count target: 8 main D-tags + sub-decisions
that emerge during Phase A. Stage-9 / stage-10 /
stage-11 / stage-12 trajectory benchmark: 6-11 main
D-tags typical for this scope. Stage 13's wider surface
(20 inbound items + table consolidations) puts it in
the upper band; sub-decision count likely 25-40 once
Phase A discussion lands.

No D-13-K tag anticipated: stage 13 owns all 20 inbound
items + the table consolidations; nothing categorically
defers to stage 14. (`render_const` and `render_instance`
already deferred to stage 14 per D-4-K and stage-1
inbound respectively; stage 14 inherits D-13-1 master
style as a forward expectation, filed in
`../stage-14/expectations.md` at merger close, not as a
K-tag.)

## 2. Active cluster

**Open phase active.** D-tag allocation drafted (this
file's §1); prior-art research framed by the D-tags is
the next step (per README §5.1 step 6). Phase A discussion
order plan: D-13-1 (master style) -> D-13-2 (envelope) ->
D-13-3 (span mode) -> D-13-4 (Item-level shape) -> D-13-5
(`render_ty` arms) -> D-13-6 (body-level arms) -> D-13-7
(specimens) -> D-13-8 (table cleanup). D-13-1 is
load-bearing -- the master style locks the surface for
every other tag's per-arm work.

Cluster strategy: single combined cluster (matches
stages 6-12). No parallel sessions anticipated -- the
20 inbound items have a strong shared dependency on
D-13-1; splitting would race over the master style.

## 3. Codex review cadence (status)

Per-stage Codex review trajectory: TBD; rounds R1, R2,
... will be appended here as they run, recording
verdict + issue counts + close-out status. Codex prompt
template lands in `memory.md` §4 once drafted (Open
phase step 7).

## 4. Open phase set-up recap

For audit / methodology trace:

- [x] Stage-13 folder bootstrapped (already existed
  with `expectations.md` from stage-3 + stage-11 D-11-K
  + stage-12 D-12-K; no creation needed).
- [x] `stage-13/expectations.md` "From stage 12 (per
  D-12-K)" section filed (was missing -- top-level
  status.md claimed it had been filed at stage-12
  merger close, but the file content showed only the
  stage-3 + stage-11 sections; reconstructed from
  authoritative source `../stage-12/discussions.md`
  §1 D-12-K + §4 outbound bundle commit).
- [x] `spec-snapshot.md` copied from top-level
  `../spec.md` (11955 lines) as Phase A working
  starting point.
- [x] `expectations.md` read as entry checklist
  (20-item inbound bundle + 3 stage-internal items).
- [x] Rustc + project-scaffold kickoff evidence pass
  completed:
  - `rustc_middle/src/mir/pretty.rs` (~2024 lines):
    canonical rustc MIR pretty-printer; hybrid
    Rust-source-style with sigil decorations; Local
    debug as `_N`, BasicBlock debug as `bbN`; envelope
    `fn $path(_1: T, ...) -> Ret { $scope_tree
    $blocks }` with 4-space INDENT; spans out-of-line
    opt-in via `-Z mir-include-spans`; per-construct
    rendering through Debug/Display impls (one big
    match per type).
  - `rustc_public/src/mir/pretty.rs` (~466 lines):
    rustc_public stable IR pretty-printer; same
    Rust-source-style; same envelope; structurally
    free `pretty_X` fns over `&mut W: io::Write`
    (closer match to RegseqModel's `PrettyPrinter`
    method shape than the rustc-internal Debug-impl
    pattern); no span rendering at all.
  - `rustc_middle/src/ty/print/pretty.rs` (~3533
    lines): TY pretty-printer; confirmed Rust-source-
    style; representative arms: `Ref` -> `&{region }
    {mut }T`; `Array` -> `[T; N]`; `Slice` -> `[T]`;
    `Tuple` -> `(T,)` / `(T1, T2, ...)`; `Adt` ->
    defers to `print_def_path`; `FnPtr` -> defers to
    `sig_tys`; `Dynamic` -> `dyn Trait` with optional
    `(dyn Trait + 'r)` parens; default-suppression
    for `'_` and `'static` lifetimes.
  - `crates/regseq-model/src/pretty.rs`: project's
    existing PrettyPrinter scaffold; sigil-conventions
    table at lines 22-34; `render_span` and
    `render_def_path_value` filled (Rust-source-style
    `crate::module::Item`); `render_ty`, `render`,
    `render_const`, `render_def_path(ItemId)`,
    `render_instance` stubbed `unimplemented!`.
  - `crates/regseq-model/src/def_path.rs`: existing
    Rust-source-style precedent (`crate::module::Item`,
    `{closure#0}`, `{impl#3}`).
  - `../spec.md` §18.1 (sigil table) + §18.2
    (per-method ownership table): confirmed; stage 13
    owns both for review/consolidation.
- [x] D-tags allocated (8 main + 0 initial sub-
  decisions; sub-decisions expand during Phase A
  discussion as evidence demands). Locked in §1 above.
- [x] Per-stage prior-art research landed across 4
  primary-source files (per-D-tag synthesis +
  per-project takeaways):
  - `prior-art/rustc.md` (~218 lines; rustc-internal
    `mir/pretty.rs` + `ty/print/pretty.rs`; hybrid
    Rust-source-style with sigil decorations;
    Debug/Display impls per IR type; off-by-default
    span rendering with `-Z mir-include-spans` toggle;
    pre/post-wrap projection chain).
  - `prior-art/rustc_public.md` (~200 lines; stable
    IR `mir/pretty.rs`; same Rust-source-style; free
    `pretty_X` fns on `&mut W: io::Write` -- closest
    structural analog to RegseqModel's `PrettyPrinter`
    method shape; zero span rendering;
    `pretty_assert_message` enumeration directly
    models `render_assert_kind`).
  - `prior-art/charon.md` (~178 lines; LLBC IR
    `pretty/`; AstFormatter trait + FmtWithCtx trait
    + WithCtx wrapper threading `FmtCtx { translated,
    generics, locals, indent_level }` context;
    Rust-source-style output; span omission; local
    sigil `@N` divergence + `const (...)` wrapper for
    constant operands).
  - `prior-art/mrustc.md` (~184 lines; Rust-to-C++
    peer; counter-precedent: dual-rendering split
    where `MirDumper` emits Rust-source-leaning
    sigil-style for MIR dumps but parallel
    `operator<<` overloads emit constructor-form for
    debug logs; the same value renders two different
    ways -- the anti-pattern stage 13 avoids).
- [x] `memory.md` drafted (workflow, principles, read
  order, Codex template) from grounded knowledge.
- [x] `status.md` finalized with prior-art findings
  + Phase A plan.

## 5. Cross-stage friction surfaced during Open phase

Drift / cleanup items found during the kickoff evidence
pass. Most are implementation-time concerns
(spec-before-implementation rule defers all stage-13
implementation to end-of-phase-1 sweep), but stage 13's
spec contributions can resolve the spec-side drift now.

- **Stale `BodyId -> Body/N` sigil row in
  `crates/regseq-model/src/pretty.rs:22-34` doc-comment
  table.** No `BodyId` exists post stage-11 D-11-5 +
  stage-12 D-12-1.4 Option B reframe (bodies live as
  `InstanceKind::Function.body: Option<Body>`; no
  separate ID). Stage 13's §18.1 review (D-13-8) cleans
  this up.

- **Stale stage-landing comments in `pretty.rs:35-36`**
  (says `LocalId`/`BlockId` sigils land in stages
  10/11). D-11-K retargeted both to stage 13. Cleanup
  at end-of-phase-1 implementation sweep, not spec
  side.

- **Stale `render_def_path(ItemId)` stub message at
  `pretty.rs:138-142`** (says "stage 12 fills"). D-12-6
  locked the body but spec-before-implementation rule
  defers the code edit. Cleanup at end-of-phase-1
  implementation sweep, not spec side.

- **§18.2 doc reference to §24.3.4** for `bbN` sigil
  pinning -- worth verifying the section exists in the
  current `../spec.md` (D-13-8).

These items get tracked in §7 and discharged through
the relevant D-tag's audit in Phase A.

## 6. Blockers

None. All upstream dependencies are pinned:

- Stage 1 (foundation; pretty.rs scaffold; sigil-
  conventions table; `render_def_path_value` precedent;
  span rendering primitives; `render_span`
  implementation) -- closed.
- Stage 3 (type system; closed 19-variant `Ty` enum at
  §15.1; D14 historical lean preserved as input,
  non-binding) -- closed.
- Stage 4 (constants; D-4-K defers `render_const` to
  stage 14, NOT stage 13) -- closed.
- Stages 5-10 (places, operands, rvalues, statements,
  terminators, basic blocks; per-construct vocabularies
  pinned for body-level renderers) -- closed.
- Stage 11 (MIR bodies; D-11-K master cascade;
  per-renderer dependency analysis preserved as
  inheritance) -- closed.
- Stage 12 (Items; D-12-1 ItemKind shape + Option B
  reframe; D-12-1.3 Item-level typed metadata;
  D-12-3.1 closed 3-flag lang-item set; D-12-K
  Item-level rendering deferral) -- closed.

Stage 14 (domain-specific extensions; `render_const`
stage-4 deferral; `render_instance`) is downstream and
inherits D-13-1 master style.

## 7. Deferred items (stage-13-specific)

To be populated as Phase A surfaces them. Stage-specific
deferrals live here; cross-stage deferrals live in
`../status.md` "Deferred items (cross-stage)".

(none yet)

## 8. Cross-stage findings (informational)

Findings surfaced during stage-13 Open phase that are
NOT stage-13 D-tags but worth recording for cross-stage
coherence.

- **F-13-Code-Drift: scaffold sigil/stub drift.** The
  in-code `crates/regseq-model/src/pretty.rs` doc
  comments + `render_def_path(ItemId)` stub message
  reference state that pre-dates stage-9 / stage-11 /
  stage-12 reframes. Implementation-time concern
  (spec-before-implementation rule); flagged here for
  the end-of-phase-1 implementation sweep checklist.

## 9. Outbound expectations to file at Phase B close

To file at Phase B close (after per-stage Codex
APPROVES, before merger contract). Targets:

### To `../stage-14/expectations.md` "From stage 13"

- **`render_const` inherits D-13-1 master rendering
  style.** Stage 4 D-4-K deferred `render_const` to
  stage 14 because `render_const`'s dependency chain
  is longer than `render_ty`'s; stage 13 locks the
  master style for both. Stage 14 implements
  `render_const` against the D-13-1 style spec.
- **`render_instance` inherits D-13-1 master style.**
  Stage-1 inbound deferred `render_instance` to stage
  14; stage 13's master style governs.

(Other outbounds may surface during Phase A discussion.)

## 10. How to update this file

Per `../README.md` §8 conventions:

- Update `Last updated:` at the top when material state
  changes.
- Keep this file lean -- cross-stage signals belong in
  `../status.md`; per-decision audit shape belongs in
  `discussions.md`; per-round Codex audit belongs in
  `review-log.md`.
- This file is per-stage-internal and not touched by
  the merger (per `../README.md` §5.8).
