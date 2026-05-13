# Stage 13 -- Pretty-printing: Stage Memory

Stage-13-specific briefing for an LLM agent picking up
the pretty-printing work. Read in addition to (not
instead of) the top-level `../README.md` (architectural
foundation, global methodology rules) and `../status.md`
(top-level progress tracker).

This file is referenced **only** from `../status.md`'s
"Active focus" block. The top-level `../README.md` does
not reference it -- by convention, to keep top-level
context lean.

This file captures stage-13 *invariants* (mission,
workflow, principles, read order, Codex prompt
template). Stage-13 *current state* (which decisions are
locked, what's open, active cluster, next concrete unit
of work) lives in `status.md`.

## 0. Mission

Stage 13 specifies the pretty-printer for RegseqModel.
Its scope, originally just `render_ty` (D14 deferred
from stage 3), broadened by stage-11 D-11-K +
stage-12 D-12-K cascades into a single coherent
rendering-decision surface covering 20 inbound items:

- 1 from stage 3: `render_ty` per the closed 19-variant
  `Ty` enum (`../spec.md` §15.1).
- 12 from stage 11 (per D-11-K): 9 public body-level
  renderers (`render_place`, `render_operand`,
  `render_rvalue`, `render_statement`,
  `render_terminator`, `render_call_callee`,
  `render_switch_targets`, `render_assert_kind`,
  `render_basic_block`), 1 internal helper
  (`render_terminator_kind`), and 2 sigil rendering
  wirings (`_N` for `LocalId`, `bbN` for `BlockId`).
- 7 from stage 12 (per D-12-K): `render_item`, ItemKind
  variant rendering, `Linkage` enum rendering,
  `InlineHint` enum rendering, Function-flag +
  lang-item-flag rendering, `link_section` rendering
  decorator, `opaque_handle_eligible` rendering
  decorator.

Plus: **§18.1 sigil-table review** (confirm or refine
the canonical sigil set + reconcile with the in-code
doc-comment table at
`crates/regseq-model/src/pretty.rs:22-34`) and **§18.2
per-method ownership-table consolidation** (stage 13
owns ALL rendering ownership rows now; the table form
itself may want refinement as a single coherent
surface).

Stage 13 sits **ninth in the stage chain** (stages 5-11
form the strict bottom-up MIR-shape chain, stage 12
the project-level Item table, stage 13 is the
pretty-printing surface that those vocabularies
project to text). It is the penultimate phase-1 stage;
stage 14 (domain-specific extensions) follows.

Stage 13's content is referenced by:

- **Stage 14 domain-specific extensions.** `render_const`
  (deferred to stage 14 by D-4-K) and `render_instance`
  (deferred to stage 14 by stage-1 inbound) inherit the
  stage-13 master rendering style (D-13-1) for
  consistency. Stage 13 files an outbound to
  `../stage-14/expectations.md` "From stage 13" at
  merger close noting the inheritance.
- **End-of-phase-1 implementation sweep.** Stage 13's
  spec contributions (textual choices, envelope shape,
  per-arm specs, default-suppression rules, sigil
  conventions, worked-example specimens) drive the
  actual `pretty.rs` implementation that lands at the
  end-of-phase-1 implementation sweep.
- **Phase-2 device pressure.** Pretty-printed output
  drives snapshot tests for pl011 / e1000 driver
  fixtures; the spec's worked-example specimens
  (D-13-7) are the canonical reference.

Stage 13 receives content from earlier stages:

- **Stage 1** (foundation; `PrettyPrinter` scaffold;
  `render_span`, `render_def_path_value` filled;
  `render_def_path(ItemId)` / `render_ty` /
  `render_const` / `render`/ `render_instance` stubs;
  sigil-conventions table). Stage 13 fills the
  remaining stubs with stage-13-locked rendering
  specifications.
- **Stage 3** (type system; closed 19-variant `Ty`
  enum at §15.1). Stage 13 owns the per-variant
  textual rendering for each of the 19 variants
  under D-13-5, inheriting from stage 3's structural
  pinning.
- **Stages 5-10** (places, operands, rvalues,
  statements, terminators, basic blocks; per-construct
  vocabularies pinned). Each stage's K-tag (D-5-K
  through D-10-K) was retargeted to stage 13 by D-11-K;
  per-arm dependency analyses preserved as inheritance
  for stage 13's per-renderer specifications under
  D-13-6.
- **Stage 11** (MIR bodies; D-11-K master cascade).
  The load-bearing carry-forward: stage 11 retargets
  ALL body-level rendering to stage 13 as a coherent
  rendering-decision surface, with the rationale that
  rendering style (sigil conventions, indentation,
  span-rendering mode, ID-resolution depth) should be
  picked ONCE for the whole body-level surface rather
  than per-renderer piecemeal.
- **Stage 12** (Items; D-12-K Item-level rendering
  cascade). Adds 7 Item-level rendering items as a
  purely additive bundle on top of the D-11-K
  cascade -- D-11-K covered all body-level rendering;
  D-12-K covers Item-level rendering.

The output of stage 13 is `spec-snapshot.md` (the
canonical per-stage spec). This file briefs the agent
on *how* decisions about that spec are reached.

Stage 13's surface is **breadth-heavy** (20 inbound
items + 2 stage-internal table consolidations + 1
worked-example specimen suite) but **depth-light** per
item: most per-arm work is mechanical once D-13-1
master style is locked, because per-stage K-tag
inheritance has already pinned the structural variants
each renderer dispatches over. The load-bearing
decisions concentrate at D-13-1 (master style choice)
and D-13-4 (Item-level shape), with the other tags
mostly inheriting + specifying.

## 1. Workflow (the rhythm we use)

Stage 13 inherits the two-phase rhythm proven in
stages 3-12:

**Phase A -- Discussion pass.** In conversation: lay
out the open decisions in a cluster with options +
recommendation + question(s). The user weighs in.
Sub-questions are answered. Reasoning chains are
validated. Decisions emerge.

**Phase B -- Spec commit + adversarial review.** Locked
decisions land in `spec-snapshot.md` with the full
audit shape (Decision / Reasoning / Methodology rules /
Prior-art evidence / Worked example). A Codex review-
correction loop runs against the cluster:
1. Initial review prompt (template in §4).
2. Codex returns verdict (BLOCK / MINOR / APPROVE) +
   numbered issues.
3. Address each issue; record corrections in
   `review-log.md`.
4. Re-run Codex with a fresh prompt naming the
   corrections.
5. Repeat until APPROVE.

**Reviews are per-cluster, not per-decision and not
big-bang.** Stage 13's decision count (8 main D-tags +
sub-decisions per Phase A discussion = ~25-40 locks
total estimated) lands in the upper band of recent
stages (stage-11: 10+ tags; stage-12: 7+ tags;
stage-9: 11+ tags). The default plan is a **single
combined cluster** covering all 8 main D-tags + sub-
decisions (mirrors stages 6-12). D-13-1 (master style)
is load-bearing -- the master style locks the surface
for every other tag's per-arm work, so cluster
parallelisation is unattractive: any session split
would race over the master style. Single-cluster
default avoids the merge-tier overhead.

`discussions.md` holds the distilled decision record;
`review-log.md` holds the round-by-round audit trail.

**Decision tag convention.** Stage 13 uses the
prefixed format from day one (`D-13-1`, `D-13-2`, ...,
with sub-tags `D-13-1.1`, `D-13-6.1`, etc., per
`../README.md` §5.2). Stages 1 and 3 use legacy un-
prefixed forms; stage 13 does not inherit those.

**No D-13-K tag anticipated.** Stage 13 owns all 20
inbound items + the stage-internal table
consolidations; nothing categorically defers to
stage 14. (`render_const` and `render_instance` are
already deferred to stage 14 per D-4-K and stage-1
inbound respectively; stage 14 inherits D-13-1 master
style as a forward expectation, filed in
`../stage-14/expectations.md` at merger close, not as
a K-tag.)

## 2. Stage-13-specific principles

Corollaries of `../README.md` "Methodology rules"
applied to the pretty-printing surface:

- **Rust-source-style is the converged default; explicit
  divergence requires evidence.** Stage-13's prior-art
  pass shows convergence across rustc-internal,
  rustc_public, Charon, and the project's own
  `render_def_path_value` precedent on Rust-source-style
  rendering. Stage-3 D14 carried the lean as input; the
  evidence makes it the strongly-supported default for
  D-13-1. Deviation (constructor-style, C-source-style)
  requires explicit evidence-based argument; mrustc's
  `operator<<` constructor-form output is the only
  cross-project counter-precedent and is the
  anti-pattern (see `prior-art/mrustc.md`).

- **Pick rendering style ONCE for the whole surface.**
  D-11-K's load-bearing rationale (and the reason
  D-12-K compositionally extends it): the 20-item
  rendering bundle is a single coherent surface. Style
  decisions (sigil conventions, indentation, span-
  rendering mode, attribute-decorator syntax,
  ID-resolution depth, default-suppression rules)
  apply uniformly. Per-renderer style dissent is
  rejected by construction -- stage 13's D-13-1
  through D-13-4 lock the global style; D-13-5 +
  D-13-6 spell out per-arm specs INHERITING that
  style, not re-litigating it.

- **Methods on `PrettyPrinter` returning `String`.**
  Q5 pinned the printer as context-bearing (`&self.model`
  for ID resolution). The shape is already shipped at
  stage 1 (`pretty.rs::PrettyPrinter<'a>`). Stage 13
  inherits the structural pattern; the rustc_public
  free-fn-on-`&mut W` analog informs per-method shape
  (see `prior-art/rustc_public.md`) but RegseqModel's
  shipped methods-returning-String shape is the
  carry-forward. Mid-stage: `String`-allocation is
  acceptable for the snapshot-test consumer + the
  diagnostic consumer; if v2 phase-2 device pressure
  surfaces a hot-path consumer, an additional
  `&mut String` overload is a non-breaking extension.

- **Snapshot stability is the load-bearing consumer.**
  Per Q5 the pretty-printer is the primary
  serialization (textual MIR-inspired form is the
  human-readable contract; serde JSON is secondary).
  Insta snapshot tests are the day-one consumer.
  Default-suppression rules (D-13-4 sub-decisions),
  span-rendering mode (D-13-3), and worked-example
  specimens (D-13-7) all weight toward making
  snapshots stable across non-spec changes (e.g., the
  bridge populating a previously-empty field
  shouldn't tickle every snapshot). Always-on span
  rendering is rejected for the default; rustc's
  toggle pattern + rustc_public's complete omission
  are the two viable D-13-3 candidates.

- **Adopt converged textual choices; synthesize
  uncovered cases from analogous patterns.** Body-level
  textual choices (Rust-source-style; `_N` / `bbN`
  sigils; `fn $path(...) -> Ret { let _N: T; ...; bbN: { ... } }`
  envelope; 4-space INDENT; pre/post-wrap projection
  chain) are converged across rustc + rustc_public +
  Charon. Adopt directly under D-13-2. Item-level
  textual choices (Function-Item display, attribute
  rendering, default-suppression for unset flags) are
  NOT covered by primary sources -- rustc + rustc_public
  render bodies, not Items. Synthesize from analogous
  rustc patterns (HIR `#[link_section = "..."]`
  attribute prefix, `unsafe fn` modifier keywords)
  under D-13-4.

- **Compositional behavior beats special-case flags.**
  Same as stages 3-12. Examples for stage 13:
  - Default-suppression for `Linkage::Default` /
    `InlineHint::None` / `link_section: None` /
    `is_unsafe: false` / `is_naked: false` / lang-flag
    `false` / `opaque_handle_eligible: false` falls
    out of "render only if non-default"; no
    `should_render: bool` flag per Item.
  - Sigil rendering for `_N` / `bbN` falls out of
    `LocalId` / `BlockId`'s typed-index display; no
    parallel `render_local_sigil` / `render_block_sigil`
    helpers (the rendering wires alongside first
    consumer per D-11-K + D-10-K.2).
  - Per-renderer style coherence falls out of the
    D-13-1 master style being shared across the full
    method surface; no per-method style flag.
  - Span rendering on/off falls out of a
    `PrettyPrinter` option-field or thread-local
    toggle if D-13-3 picks the toggle candidate; no
    per-renderer span-on flag.

- **Code does not reference discussion-folder paths.**
  Same as stages 3-12. Inline rendering specs in
  comments at the implementation-site reference the
  D-13-X tag (e.g., `/* per D-13-2 envelope */`) or
  inline restate the rule, NOT
  `docs/discussions/regseq-model/stage-13/...` paths
  or section numbers.

- **Spec is mutable; revise on awkward shapes.** Same
  as stages 3-12. The §18.1 sigil-table drift in
  `pretty.rs:22-34` (stale `BodyId -> Body/N` row;
  stale stage-landing comments at lines 35-36) is an
  example -- D-13-8's table review revises the spec's
  §18.1 rather than working around the drift in
  per-renderer arms.

- **Worked-example specimens are spec content.** Per
  Q5 the pretty-printer's textual contract is
  primary. Worked-example specimens (≥1 canonical
  fragment per public renderer in `spec-snapshot.md`)
  pin the textual contract for snapshot stability;
  they are spec-side content, not test fixtures. Each
  specimen pairs a small input value with the rendered
  output. Renderers' per-arm specs (D-13-5, D-13-6,
  D-13-4) reference the specimen for cross-validation.
  Insta snapshots in tests then mirror the specimen.

- **No bridge-side work.** Stage 13 is consumer-side
  only: pretty-printing reads the already-built
  `RegseqModel`. Bridge-side type construction is
  fully owned by stages 3 / 11 / 12. Stage-13's spec
  contributions land in the consumer-facing API
  surface (`pretty.rs` methods + `../spec.md` §18 +
  the deferred-renderer rows in §19.8 / §20.6 / §21.8 /
  §22.8 / §23.9 / §24.9).

- **C-semantics for body-level + Item-level rendering.**
  Same alphabet, different consumers. Pretty-printed
  output is HUMAN-readable text for snapshot tests +
  diagnostics; the C backend doesn't consume
  pretty-print output (it consumes the IR directly).
  The rendering style choice is therefore not C-vs-Rust
  source choice -- both are viable. Rust-source-style
  is preferred per the prior-art convergence + the
  in-project `render_def_path_value` precedent +
  stage-3 D14 lean.

- **F4 / F4-2 cross-build concerns are out of stage-13
  scope.** Pretty-printed output is consumer-side and
  does not participate in F4 / F4-2 cross-build
  identity. Per `../README.md` §5.9 spec is
  topic-organized, not stage-organized; stage 13's
  spec contributions land in §18 (pretty-printer
  surface) + §15 (Ty rendering) + bullet-points in §19
  through §25 (per-construct deferred-renderer rows),
  NOT in §16 (F4 / determinism).

## 3. Read order for new agents picking up stage 13

In priority:

0. **If you are opening a specific D-tag**: read this
   file + `status.md` + `discussions.md` entries for
   the decisions in your cluster (once they exist) +
   the relevant spec sections in top-level
   `../spec.md`:
   - Section 15 (`Ty` enum -- stage-3 schema; stage 13
     owns per-variant rendering under D-13-5).
   - Section 18 (Pretty-printer -- the surface stage 13
     fills); §18.1 sigil table; §18.2 per-method
     ownership table.
   - Sections 19, 20, 21, 22, 23, 24, 25 (Place,
     Operand, Rvalue, Statement, Terminator, BasicBlock,
     Body vocabularies; per-construct deferred-
     renderer rows in §19.8 / §20.6 / §21.8 / §22.8 /
     §23.9 / §24.9 are stage-13-owned post-D-11-K).
   - Section 8 (Item / ItemKind -- stage-12 schema;
     stage 13 owns Item-level rendering under D-13-4).
   - Section 9.4 (`InstanceKind` -- post-Option-B;
     `Function { fn_sig, body, flavor, virtual_idx }`
     consumed by Function-Item rendering).
   - Section 14 (FnSig -- consumed by Function-Item
     rendering and `Ty::FnPtr` rendering).

1. This file (`stage-13/memory.md`) -- stage-13
   invariants.

2. `status.md` -- current state (locked decisions,
   active cluster, next concrete unit of work).

3. `spec-snapshot.md` -- the working spec (initially
   a copy of `../spec.md`, 11955 lines; stage-13
   contributions accumulate as in-place updates to
   §18.1 + §18.2, additions to `pretty.rs` method
   bodies, and per-construct deferred-renderer rows
   flipping from "deferred to stage 13 per D-11-K /
   D-12-K" to concrete render specifications).

4. `expectations.md` -- inbound forward refs (20-item
   bundle: stage 3 + stage 11 D-11-K + stage 12
   D-12-K, plus 3 stage-internal items).

5. `discussions.md` -- distilled decision records
   (created when the first decision lands; lazy).

6. `review-log.md` -- round-by-round Codex audit trail
   (created when the first review round runs; lazy).

7. `prior-art/<project>.md` -- per-stage primary-source
   research. Landed at stage open:
   - `prior-art/rustc.md` -- rustc-internal
     (`rustc_middle/src/mir/pretty.rs` + `ty/print/pretty.rs`).
     Hybrid Rust-source-style with sigil decorations;
     per-IR-type Debug/Display impls (one big match per
     type); 4-space INDENT; off-by-default span
     rendering with `-Z mir-include-spans` toggle;
     pre/post-wrap projection chain.
   - `prior-art/rustc_public.md` -- stable IR
     (`rustc_public/src/mir/pretty.rs`). Same
     Rust-source-style; structural pattern is free
     `pretty_X` fns on `&mut W: io::Write` (closest
     analog to RegseqModel's `PrettyPrinter` method
     shape); zero span rendering; concrete enumeration
     pattern in `pretty_assert_message` directly maps
     to `render_assert_kind`.
   - `prior-art/charon.md` -- LLBC IR. AstFormatter
     trait + FmtWithCtx trait + WithCtx wrapper
     threading `FmtCtx { translated, generics, locals,
     indent_level }` context; Rust-source-style
     output; span omission.
   - `prior-art/mrustc.md` -- Rust-to-C++ peer.
     Counter-precedent: `MirDumper` emits Rust-source-
     leaning sigil-style for MIR dumps but parallel
     `operator<<` overloads emit constructor-form for
     debug logs (anti-pattern: same value renders two
     different ways).

   Per-cluster top-up candidates (research only when
   the cluster's evidence demands it):
   - rustc-dev-guide MIR pretty-printing reference
     (https://rustc-dev-guide.rust-lang.org/) for
     cross-reference.
   - `rustc_codegen_cranelift` body-walk pattern at
     `rustc_codegen_cranelift/src/base.rs:284` for
     Body-level pretty-print structural analog.

8. `../README.md` -- top-level briefing (invariants only).

9. `../status.md` -- top-level progress tracker
   (cross-stage signals).

10. `../spec.md` -- merged top-level spec; consult
    Sections 8 (Item), 9.4 (InstanceKind post-Option-B),
    14 (FnSig), 15 (Ty), 18 (Pretty-printer), 19 (Place),
    20 (Operand), 21 (Rvalue), 22 (Statement),
    23 (Terminator), 24 (BasicBlock), 25 (Body).

## 4. Codex review cadence and prompt template

The cadence and current round status live in
`status.md` "Codex review cadence (status)." This
section captures the *template* (reusable across
rounds and clusters).

Prompt template skeleton (paste-and-edit per round; per
the project root `CLAUDE.md` "Prompt shape" guidance):

```
<task>
Adversarial review of stage-13 pretty-printer spec, decisions <X>-<Y>, ROUND <N>.

[If N > 1: round context + summary of prior corrections]

Spec under review: docs/discussions/regseq-model/stage-13/spec-snapshot.md (focus: <sections>).
Out of scope: <e.g. unrelated D-tags, markdown style, body-level vocabulary already pinned at stages 5-11, Item-level vocabulary already pinned at stage 12, render_const deferred to stage 14 per D-4-K, render_instance deferred to stage 14 per stage-1 inbound, bridge-side construction concerns>.
Evidence files: <full list relevant to this cluster, including 4 prior-art/*.md files>.
Evaluate: <numbered criteria; 1. round-N-fix verification if N > 1; 2. new issues from corrections; 3. residual issues; 4-N. soundness/citation/consistency, especially the master rendering-style coherence across the 20-item surface (D-11-K + D-12-K cascades) and the worked-example specimens for snapshot stability>.
</task>

<grounding_rules>
Cite file:line for every claim. Do not speculate. Mark hypotheses as hypotheses. Distinguish "what spec claims" from "what evidence file says."
</grounding_rules>

<structured_output_contract>
First line: VERDICT: <BLOCK | MINOR | APPROVE>
Then numbered issues: N. [BLOCK | MINOR | NIT] <tag> <line ref>
  <description> <suggested fix>
Hard cap 25; NITs only with budget.
Last line: TOTAL: <N> issues (<B> BLOCK / <M> MINOR / <T> NIT)
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, decide, stop.
</default_follow_through_policy>
```

Run via:
```bash
cat <<'EOF' | uv run python /<repo-root>/scripts/python/codex_runner.py --cwd /<repo-root> --effort high --timeout 1500 --message-only > /tmp/codex_d<cluster>_review_r<N>.txt 2>&1
<prompt>
EOF
```

with `run_in_background: true`. Capture output to
`/tmp/...`. Record each round in `review-log.md` with
verdict, issue list, and corrections applied.

## 5. What this file is NOT

- **Not the stage status tracker.** Use `status.md` for
  current state (locked decisions, active cluster, next
  concrete unit of work, blockers, deferred items).
- **Not a substitute for `spec-snapshot.md` (the spec)
  or `discussions.md` (the decision records).** Briefing
  only.
- **Not append-only.** Update sections in place when
  stage-13 invariants change (rare).
- **Not where decisions live.** Decisions live in
  `spec-snapshot.md` (and `discussions.md` for audit
  shape).
