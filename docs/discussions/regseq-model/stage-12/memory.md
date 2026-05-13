# Stage 12 -- Rust Items: Stage Memory

Stage-12-specific briefing for an LLM agent picking up the
Item-level work. Read in addition to (not instead of) the
top-level `../README.md` (architectural foundation, global
methodology rules) and `../status.md` (top-level progress
tracker).

This file is referenced **only** from `../status.md`'s
"Active focus" block. The top-level `../README.md` does not
reference it -- by convention, to keep top-level context lean.

This file captures stage-12 *invariants* (mission, workflow,
principles, read order, Codex prompt template). Stage-12
*current state* (which decisions are locked, what's open,
active cluster, next concrete unit of work) lives in
`status.md`.

## 0. Mission

Stage 12 specifies the `Item` and `ItemKind` types: the
source-level Item alphabet plus the per-`ItemKind` field
expansion that absorbs the stage-11 D-11-3 + D-11-5
outbound (`Function { fn_sig: FnSigId, body: Option<Body>,
... }`). Stage 12 closes the stage-1 inbound bundle
(Item-placeholder expansion, `def_path` field wiring,
`render_def_path` stub fill, construction invariants 3 / 4 /
6 closure), confirms the bridge-synthesised Item machinery
already pinned at §13 (closure family, vtable family,
ConstMemoryStatic) is internally consistent with the
expanded ItemKind, and pins the lang-item recognition
mechanism (AdtFlags predicate-based for the 3 §8.2 flags;
non-flagged lang-items queried on-demand at call sites).

Stage 12 sits **eighth in the stage chain** (stages 5-11
form the strict bottom-up MIR-shape chain: Place ->
Operand -> Rvalue -> Statement -> Terminator -> BasicBlock
-> Body, and stage 12 is the project-level Item table that
bodies live inside). It is the last stage before stage 13
(pretty-printing) and stage 14 (domain-specific extensions).

Stage 12's content is referenced by:

- **Stage 13 pretty-printing.** Stage 12 fills the
  `render_def_path(ItemId)` stub (the only non-deferred
  Item-level renderer per §18.2). Broader Item-level
  rendering (Item display, ItemKind variant rendering,
  attribute rendering if any) defers to stage 13 per
  D-12-K's compositional retargeting onto the D-11-K
  cascade.
- **Stage 14 domain-specific extensions.** Q7 intrinsic
  recognition lifts onto the lang-item-recognition
  mechanism stage 12 pins (AdtFlags predicates for ADT
  lang-items + closed-vocabulary attribute-based
  recognition for intrinsic dispatch via
  `tcx.intrinsic_raw(did)`); stage 14 builds on this for
  intrinsic dispatch tables, ordered regions, MMIO, ISR
  classification.

Stage 12 receives content from earlier stages:

- **Stage 1** (foundation crate; ID conventions; placeholder
  Item, DefPath shipping). The empty `Item` struct at
  `crates/regseq-model/src/placeholders.rs` becomes the
  full Item shape per §8.0 (already specified in the merged
  spec); stage 12 wires the `def_path: DefPath` field to
  the bridge population path; closes construction
  invariants 3 (F4 monotonicity), 4 (BridgeInput
  spec_crate_stable_id), 6 (`def_path.crate_id < crates.len()`).
- **Stage 3** (type system; D5 dyn-Trait dispatch; D6
  closure family). The Ty-level shapes are pinned
  (`Ty::Dynamic { auto_traits, vtable_ty: TyId }`,
  `Ty::Closure(ItemId, GenericArgs)`); stage 12 confirms
  the bridge-synthesised Items at §13 (vtable type Items,
  vtable instance Items, vtable shim Function Items,
  closure captures-struct Items, closure call-method
  Function Items, closure drop-glue Function Items) are
  synthesised in correct construction-pipeline order.
  All synthesis machinery is already pinned; stage 12
  inherits without re-litigating.
- **Stage 4** (constants; D-4-3 ConstMemoryStatic).
  CLOSED at stage 4 (whole-family scope including the
  stage-12-territory contribution at §13.3 +
  §13.4); stage 12 inherits by reference, no further
  spec work.
- **Stage 9** (terminators; D-9-3 (b) callee shape).
  Stage 9 resolved that the DefPathData encoding for
  synthetic vtable scopes is owned by stage 12 (per
  stage-9's outbound). §13.4 already pins the encoding
  as `TypeNs("<regseq_synth>")` prefix; stage 12 confirms.
- **Stage 11** (MIR bodies; D-11-3 + D-11-5 + D-11-6.1
  outbound bundle). The load-bearing inbound. Stage-11
  reframed FnSig location to Item-side (D-11-3) and
  pinned Body as a structural field of Item (D-11-5);
  stage 12 absorbs both into the `ItemKind::Function {
  fn_sig: FnSigId, body: Option<Body>, ... }` schema
  expansion. Construction invariant 9 closure (B-7) was
  pinned as visitor-side at stage 11; stage 12 owns
  `model.instances` integrity (no schema work; the
  closure check happens in the stage-11-owned
  joint-validation visitor).

The output of stage 12 is `spec-snapshot.md` (the
canonical per-stage spec). This file briefs the agent on
*how* decisions about that spec are reached.

Stage 12's surface is **structurally smaller** than
stages 9-11 because §8 / §9 / §13 / §14 are heavily
pre-pinned by stages 1, 3, 4, 9, and 11. Stage 12 owns
the few remaining design choices: ItemKind cardinality
+ per-variant data shape (D-12-1), foreign-item dispatch
rule (D-12-2), lang-item recognition mechanism (D-12-3),
construction-invariant closure (D-12-4), bridge
construction-pipeline order (D-12-5), `render_def_path`
stub fill (D-12-6), and the categorical rendering
deferral (D-12-K). Compared to stage 11's 10 D-tags,
stage 12 has 7 (6 main + K) -- consistent with the
lower decision density of an integration / closure
stage.

## 1. Workflow (the rhythm we use)

Stage 12 inherits the two-phase rhythm proven in stages
3, 4, 5, 6, 7, 8, 9, 10, and 11:

**Phase A -- Discussion pass.** In conversation: lay
out the open decisions in a cluster with options +
recommendation + question(s). The user weighs in.
Sub-questions are answered. Reasoning chains are
validated. Decisions emerge.

**Phase B -- Spec commit + adversarial review.** Locked
decisions land in `spec-snapshot.md` with the full audit
shape (Decision / Reasoning / Methodology rules /
Prior-art evidence / Worked example). A Codex
review-correction loop runs against the cluster:
1. Initial review prompt (template in Section 4).
2. Codex returns verdict (BLOCK / MINOR / APPROVE) +
   numbered issues.
3. Address each issue; record corrections in
   `review-log.md`.
4. Re-run Codex with a fresh prompt naming the corrections.
5. Repeat until APPROVE.

**Reviews are per-cluster, not per-decision and not
big-bang.** Stage 12's decision count (6 main + 1 K-tag
+ sub-decisions on D-12-1 / D-12-4 = ~10 locks total) is
comparable to stage 10 (6 + sub) and smaller than stage
11 (10 total). The default plan is a **single combined
cluster** covering all 6 main D-tags + sub-decisions +
D-12-K (mirrors stages 6-11). If Phase A surfaces a
clean cluster split (e.g., schema decisions D-12-1/2/3
separable from invariant-closure decisions
D-12-4/5/6/K) the parallel-session machinery in
`../README.md` §5.3 is available; the single-cluster
default avoids the merge-tier overhead unless evidence
demands it.

`discussions.md` holds the distilled decision record;
`review-log.md` holds the round-by-round audit trail.

**Decision tag convention.** Stage 12 uses the prefixed
format `D-12-K` from day one (`D-12-1`, `D-12-2`, ...,
with sub-tags `D-12-1.1`, `D-12-4.1`, etc., per
`../README.md` §5.2). Stages 1 and 3 use legacy
un-prefixed forms; stage 12 does not inherit those.

## 2. Stage-12-specific principles

Corollaries of `../README.md` "Methodology rules"
applied to the Item-level surface:

- **Store rustc's resolved Item-level metadata, not the
  surface attribute set.** rustc offers a rich per-`DefId`
  metadata surface: `tcx.fn_sig(did)`,
  `tcx.codegen_fn_attrs(did)` (17+ flags + symbol_name /
  link_section / link_name / target_features / inline /
  optimize / linkage / alignment), `tcx.intrinsic_raw(did)`,
  `tcx.adt_def(did)`, `tcx.is_foreign_item(did)`,
  `tcx.is_mir_available(did)`, `tcx.visibility(did)`. Most
  of this is NOT load-bearing for C codegen of Regseq
  drivers under Q2 (eager monomorphisation, baked at
  bridge). The stage-12 schema lifts only the subset that
  is load-bearing per the methodology rule "preserve
  information rustc gives us for free, interrogated per
  construct -- ask what it carries, who consumes it,
  what shape best serves them." Per-DefId metadata that
  is not consumed by stage-13 pretty-printing or stage-14
  domain-specific lowerings stays in rustc-side queries
  at construction time only; the model carries the
  resolved outcome where the outcome is observable, not
  the raw query.

- **Closed `ItemKind` cardinality, six emission kinds.**
  Per current §8.0, `ItemKind = { Function, Static,
  Const, Struct, Enum, Union }`. The cardinality is
  closed against the pinned toolchain. The 6-variant
  set is consciously richer than rustc_public's
  4-variant `{ Fn, Static, Const, Ctor }` because
  RegseqModel consumes ADT layout for C codegen --
  Struct / Enum / Union are first-class for downstream
  consumers per the rustc-internal precedent (14-variant
  ItemKind with Struct/Enum/Union as variants at
  `rustc_hir/src/hir.rs:4446-4448`). Variants rustc has
  but RegseqModel rejects/lifts/erases (Trait, TraitAlias,
  Impl, AssocTy/AssocFn/AssocConst, Macro,
  ExternCrate, Use, ForeignMod, Mod, Field, TyParam,
  LifetimeParam, etc.) per §8.1 lifting policy and §15.1
  rejection cascade.

- **`ItemKind::Function` expands; non-Function variants
  stay unit.** Per stage-11 D-11-3 + D-11-5, `Function`
  expands to a struct-variant with at minimum
  `{ fn_sig: FnSigId, body: Option<Body>, ... }`.
  Static / Const / Struct / Enum / Union remain unit
  variants. Per-monomorphisation data for non-Function
  kinds lives on `InstanceKind` (`Static.init`,
  `Const.value`, `{Struct, Enum, Union}.layout`) per
  the existing §9.4 schema, NOT on the corresponding
  ItemKind variant. Rationale: Items carry source-level
  alphabet (one Item per source-level def, regardless
  of how many monomorphisations); per-monomorphisation
  data lives on Instance per Q2's eager-monomorphisation
  baking.

- **`ItemKind::Function` minimum field set, additional
  fields gated by load-bearing C-codegen need.** v1
  pins the minimum schema `{ fn_sig: FnSigId, body:
  Option<Body> }` per stage-11. Additional fields
  (inline hint per `CodegenFnAttrs.inline: InlineAttr`,
  CodegenFnAttrFlags subset, etc.) are admitted only if
  v1 C codegen of pl011 / e1000 driver fixtures
  demonstrates load-bearing need. Fields that v1
  defers: full CodegenFnAttrs lift (most flags are
  symbol-mangling / linkage concerns folded into
  `Instance.emission_name` per §9.2; not load-bearing
  at the Item level). Fields that v1 candidates
  for inclusion: `inline: InlineAttr` (4-variant
  closed mirror) for `static inline` / `__attribute__
  ((always_inline))` / `__attribute__((noinline))` C
  emission. The exact minimum is a Phase-A decision.

- **`Item.kind` accessor pattern: convenience methods
  on `ItemKind`, match-arm-readable.** Idiomatic Rust:
  `ItemKind::as_function(&self) -> Option<&FunctionData>`
  + analogous `as_static` / `as_const` if non-Function
  variants ever gain fields. Match-arm-only access is
  also supported at the call site; the convenience
  method is for the common case where the consumer
  already knows the variant. Matches rustc-internal's
  pattern (per-variant accessors on `ItemKind`).

- **Foreign / extern function dispatch via
  `tcx.is_foreign_item(did)`.** The bridge dispatches
  on rustc's `is_foreign_item` predicate (parent
  `DefKind == ForeignMod` per
  `rustc_middle/src/hir/mod.rs:165-168`). When the
  predicate returns `true`: `body: None` per stage-11
  D-11-5; `fn_sig` still populated (rustc's
  `tcx.fn_sig(did)` is callable for foreign fns even
  though `is_mir_available(did)` is false). Symbol name
  comes from `#[link_name]` (only meaningful on foreign
  items) folded into `Instance.emission_name`; ABI from
  the `ForeignMod` parent's `extern "ABI"` clause.
  Charon's `Result<Body, Opaque>` is the closest peer
  (one-bit signal in the body field itself, same
  semantics as `Option<Body>`); rustc_public's
  `is_foreign_item()` boolean is the same shape with
  predicate access. rustc-internal's dedicated
  `ForeignItem` struct (separate from `ItemKind`) is a
  divergence stage 12 considered and rejected: stage
  11 already pinned the `body: Option<Body>` shape;
  splitting into a separate Item type would contradict
  the pinned design. The has_body flag rustc carries
  on domestic `ItemKind::Fn` (distinguishing
  declared-but-unimplemented from bodyful) is NOT
  applicable to RegseqModel: post-monomorphisation,
  every callable has either a body (source-defined or
  shim-synthesised) or is foreign; the
  declared-but-unimplemented case (rustc's "function
  signature without body in trait declarations") is
  erased per §8.1's trait-lifting policy.

- **Closed lang-item flag set; AdtFlags-predicate
  recognition.** Per current §8.2, three lang-item flags
  on `Item`: `is_phantom_data`, `is_unsafe_cell`,
  `is_manually_drop`. Recognition mechanism: AdtFlags
  bitflag predicates (`adt_def.is_phantom_data()` etc.)
  per `rustc_middle/src/ty/adt.rs:414-440`. The 3-flag
  set is closed; non-flagged lang-items (CVoid,
  MaybeUninit, Pin, Box, Drop trait, drop_in_place fn,
  Fn/FnMut/FnOnce traits, panic_fn, etc.) are queried
  on-demand at the call/use site via
  `tcx.is_lang_item(did, LangItem::X)` -- NOT stored on
  Item. Rationale: methodology rule "store the result
  rustc gives us for free" applies only when the result
  is load-bearing for codegen lowering. The 3 flagged
  lang-items are load-bearing for type-system /
  drop-elaboration / interior-mutability semantics
  (PhantomData triggers ZST handling; UnsafeCell
  triggers the §17.2.1 trait-object dispatch carve-out;
  ManuallyDrop triggers no-auto-drop). Other lang-items
  are queried where they are consumed (CVoid at type
  rejection, MaybeUninit at composition-driven
  no-auto-drop, etc.). No opaque `LangItem` tag stored
  on Item; the recognition is closed and predicate-based.
  Charon's string-based `lang_item: Option<String>` is
  rejected as a shape: string-based recognition admits
  open-vocabulary drift (per §17.4 closed `Ty` enum
  rationale).

- **Bridge construction-pipeline order.** Discovery
  (rustc mono-item collector at
  `rustc_monomorphize/src/collector.rs:1803`) ->
  synthesis (vtables per §13.2 triggered at
  `Ty::Dynamic` translation; closures per §13.1
  triggered at `Ty::Closure` translation;
  ConstMemoryStatic per §9.7 + §13.3 triggered at
  reference-typed const translation) -> unified F4
  sort across source + synthetic Items per §16.1 ->
  ItemId assignment + parallel `item_def_paths` /
  `item_synthetic_kinds` population. The synthesis
  step is interleaved with type translation (the
  bridge mints synthetic Items the moment a triggering
  type is encountered; cached via the relevant
  side tables per §13). The unified F4 sort happens
  AFTER all discovery + synthesis is complete --
  source-derived and bridge-synthesised Items go
  through the same comparator (all DefPathHash-keyed,
  with synthetic items using `stable_crate_id == 0`
  per §13.4 rejection-enforced disjoint domain). No
  "synthesis pass before discovery" or "discovery in
  ItemId order" alternatives; the order is pinned by
  Q2 (eager monomorphisation; bridge does the work
  once, fully).

- **Construction-invariant closure for stage-1
  deferrals.** Stage 1 deferred 3 invariants
  (3 F4 monotonicity; 4 BridgeInput
  spec_crate_stable_id; 6
  `def_path.crate_id.index() < crates.len()`)
  pending stage-12 absorption. Stage 12 closes all
  three at validate:
  - **Invariant 3** -- walk `items` and check the F4
    sort comparator key is monotonically non-decreasing
    (per §16.1 `def_path_data_sort_rank` + segment-by-
    segment comparator). Out-of-order items reject as
    model corruption.
  - **Invariant 4** -- add `BridgeInput` parameter to
    `RegseqModel::validate(&self, &BridgeInput)`;
    check `crates[primary_crate].stable_crate_id ==
    bridge_input.spec_crate_stable_id`. The other half
    of invariant 4 (`primary_crate.index() <
    crates.len()`) is already model-checkable from
    stage-1.
  - **Invariant 6** -- walk `items` and check
    `item.def_path.crate_id.index() < crates.len()`
    for every item. Bound check; trivial.

  Test coverage: positive + negative tests in
  `tests/model.rs` per the existing pattern.

- **Categorical rendering deferral compositional with
  D-11-K.** D-11-K already retargeted body-level
  rendering (D-5-K through D-10-K + sigils) to stage
  13. Stage 12's D-12-K extends the cascade to
  Item-level rendering: Item display rendering,
  ItemKind variant rendering, FunctionData
  pretty-printing -- all defer to stage 13. The only
  non-deferred Item-level renderer is
  `pretty.rs::render_def_path(ItemId)` per D-12-6
  (stage-1 stub fill; the body is trivial since the
  eager `def_path.rendered: StringId` is the cached
  result). Stage 12 hands stage 13 a frozen Item-level
  vocabulary; stage 13 picks the rendering shape with
  full input-readiness on top of `render_ty` and the
  body-level renderers retargeted from D-11-K.

- **Compositional behavior beats special-case flags.**
  Same as stages 3-11. Examples for stage 12:
  - Foreign-item dispatch falls out of `body: None`
    per stage-11 D-11-5; no separate `is_foreign:
    bool` flag on Item.
  - Lang-item recognition for ADT lang-items falls
    out of AdtFlags predicates (rustc-side); no
    parallel `lang_item: Option<LangItem>` field on
    Item.
  - Per-monomorphisation data for non-Function
    ItemKind variants falls out of `InstanceKind`
    payloads; no parallel per-variant ItemKind fields.
  - Bridge-synthesised Item identity falls out of
    structured `SyntheticItemKind` per §13.3 +
    synthetic `DefPathHash` per §13.4; no parallel
    `is_synthetic: bool` flag on Item.
  - Construction-pipeline order falls out of Q2
    (eager mono baking) + the F4 comparator; no
    explicit pipeline-stage tags.

- **C-semantics for Item-level surface.** Each
  source-level Item produces source-level traceback
  metadata (DefPath, kind, span, attributes) that
  drives:
  ```c
  /* ItemKind::Function -> emitted as C function */
  ret_ty fn_emission_name(arg_ty1 arg1, ..., arg_tyN argN) {
      /* body from Item.kind=Function.body, if Some */
  }

  /* ItemKind::Static -> emitted as C static (per Instance) */
  static_ty static_emission_name = init_value;

  /* ItemKind::Struct/Enum/Union -> emitted as C struct/etc.
     (per Instance, monomorphisation-aware) */
  struct emission_name { fields... };
  ```
  The Item itself carries the source-level alphabet;
  per-mono Instance is the emission unit. Stage 12
  fixes the data the Instance + body consumes.

- **F4 / F4-2 cross-build concerns are stage-12 scope.**
  Items participate in F4 cross-build identity (per
  §16.1 / §16.2). Stage 12 owns invariant 3 closure
  (F4 monotonicity walk at validate). InstanceKey
  (F4-2) was closed at stage 1 + stage 9; stage 12
  inherits.

## 3. Read order for new agents picking up stage 12

In priority:

0. **If you are opening a specific D-tag**: read this
   file + `status.md` + `discussions.md` entries for
   the decisions in your cluster (once they exist) +
   the relevant spec sections in top-level
   `../spec.md`:
   - Section 7 (DefPath / DefPathData / DefPathHash --
     stage-1 schema, inherited).
   - Section 8 (Item -- the surface stage 12 expands).
   - Section 9 (Instance / InstanceKind -- per-mono
     schema; stage 12's ItemKind expansion shapes the
     parent-Item linkage).
   - Section 13 (SyntheticItemKind -- the bridge-
     synthesised Item machinery already pinned by
     stages 1, 3, 4; stage 12 confirms internal
     consistency).
   - Section 14 (FnAbi / FnSig -- stage-11 D-11-3
     reframe pinned FnSig as Item-side; stage 12
     absorbs into ItemKind::Function expansion).
   - Section 16 (Determinism / F4 -- stage-1 schema;
     stage 12 closes invariant 3 walk).
   - Section 18.2 (per-method ownership table for the
     pretty-printer -- stage 12 owns
     `render_def_path`; broader Item rendering
     deferred to stage 13 per D-12-K).
   - Section 25 (Body-level vocabulary -- stage-11
     content; ItemKind::Function.body materialisation
     references this).

1. This file (`stage-12/memory.md`) -- stage-12 invariants.

2. `status.md` -- current state (locked decisions,
   active cluster, next concrete unit of work).

3. `spec-snapshot.md` -- the working spec (initially
   a copy of `../spec.md`, 11163 lines; stage-12
   contributions accumulate as in-place updates to §8,
   §9.4 ItemKind::Function expansion, and additions
   for the construction-invariant closure rules).

4. `expectations.md` -- inbound forward refs (sizable
   bundle covering stages 1, 3, 9, 11; stage 4 already
   CLOSED by D-4-3).

5. `discussions.md` -- distilled decision records
   (created when the first decision lands; lazy).

6. `review-log.md` -- round-by-round Codex audit trail
   (created when the first review round runs; lazy).

7. `prior-art/<project>.md` -- per-stage primary-source
   research. Landed at stage open:
   - `prior-art/rustc.md` -- rustc-internal
     (rustc_hir/rustc_middle). 14-variant ItemKind with
     first-class Struct/Enum/Union; dedicated
     ForeignItem struct + ForeignItemKind (Fn/Static/
     Type); has_body bool flag; AdtFlags + LangItem
     dual recognition.
   - `prior-art/rustc_public.md` -- stable IR. Minimal
     4-variant ItemKind { Fn, Static, Const,
     Ctor(CtorKind) }; ADT recovered via type system;
     `is_foreign_item()` boolean predicate; no
     lang-item API.
   - `prior-art/charon.md` -- LLBC IR. Orthogonal
     split ItemKind (scope: TopLevel/Closure/TraitDecl/
     TraitImpl) + TypeDeclKind (Struct/Enum/Union/
     Opaque/Alias/Error); `Result<Body, Opaque>` body
     encoding; string-based `lang_item: Option<String>`.
   - `prior-art/mrustc.md` -- Rust-to-C++ peer.
     Category-based tagged unions (TypeItem / ValueItem
     / MacroItem); Linkage-based foreign encoding;
     TraitMarkings (cached trait flags) instead of
     enumerated lang-items.

   Per-cluster top-up candidates (research only when
   the cluster's evidence demands it):
   - `rustc_codegen_cranelift::base.rs` for foreign-fn
     declaration emission patterns (relevant if D-12-2
     surfaces ABI / link_name questions).
   - rustc-dev-guide MIR overview
     (https://rustc-dev-guide.rust-lang.org/mir/) for
     cross-reference of foreign-item handling.

8. `../README.md` -- top-level briefing (invariants only).

9. `../status.md` -- top-level progress tracker (cross-
   stage signals).

10. `../spec.md` -- merged top-level spec; consult
    Sections 7 (DefPath), 8 (Item), 9 (Instance + InstanceKind),
    13 (Synthesised Items), 14 (FnAbi + FnSig), 16
    (Determinism), 18.2 (per-method ownership table),
    25 (Body-level vocabulary).

## 4. Codex review cadence and prompt template

The cadence and current round status live in `status.md`
"Codex review cadence (status)." This section captures
the *template* (reusable across rounds and clusters).

Prompt template skeleton (paste-and-edit per round; per
the project root `CLAUDE.md` "Prompt shape" guidance):

```
<task>
Adversarial review of stage-12 Item-level spec, decisions <X>-<Y>, ROUND <N>.

[If N > 1: round context + summary of prior corrections]

Spec under review: docs/discussions/regseq-model/stage-12/spec-snapshot.md (focus: <sections>).
Out of scope: <e.g. unrelated D-tags, markdown style, body-level rendering decisions deferred to stage 13 per D-11-K + D-12-K, vtable / closure synthesis machinery already pinned at stage 3, ConstMemoryStatic family already CLOSED by D-4-3>.
Evidence files: <full list relevant to this cluster, including 4 prior-art/*.md files>.
Evaluate: <numbered criteria; 1. round-N-fix verification if N > 1; 2. new issues from corrections; 3. residual issues; 4-N. soundness/citation/consistency, especially the absorption of stage-11 D-11-3 + D-11-5 outbound (FnSig + Body on Item) and the closure of stage-1 inbound deferrals (invariants 3, 4, 6)>.
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

with `run_in_background: true`. Capture output to `/tmp/...`.
Record each round in `review-log.md` with verdict, issue
list, and corrections applied.

## 5. What this file is NOT

- **Not the stage status tracker.** Use `status.md` for
  current state (locked decisions, active cluster, next
  concrete unit of work, blockers, deferred items).
- **Not a substitute for `spec-snapshot.md` (the spec)
  or `discussions.md` (the decision records).** Briefing
  only.
- **Not append-only.** Update sections in place when
  stage-12 invariants change (rare).
- **Not where decisions live.** Decisions live in
  `spec-snapshot.md` (and `discussions.md` for audit
  shape).
