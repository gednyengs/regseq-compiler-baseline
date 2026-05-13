# Stage 12 Status

Status: **STAGE 12 CLOSED AND MERGED 2026-05-06.** Phase
A locked all 7 D-tags + 29 sub-decisions across a single
combined cluster; Phase B per-stage Codex review closed at
R7 APPROVE / Lockability:yes / 0 issues; merger contract
per README §5.5 7-step protocol completed (top-level
`discussions.md`, `review-log.md`, `decisions.md` extended
with stage-12 content; stage-11 D-11-3 + D-11-5 flipped
to `[REOPENED]` with annotation `(revised in stage 12 per
D-12-1.4)`; `spec-snapshot.md` integrated into top-level
`spec.md`); post-merge tier-3 Codex review pass R1-R3
closed at R3 APPROVE / Lockability:yes / 0 issues on
2026-05-06. Total stage-12 Codex cost: 10 rounds (7
per-stage + 3 post-merge); 31 distinct findings closed
(20 BLOCK + 11 MINOR + 0 NIT). Stage-12 lands within the
stage-9 / stage-10 / stage-11 trajectory band.

The current authoritative spec lives at top-level
`../spec.md`; this `stage-12/spec-snapshot.md` is now
a frozen historical artifact (with the "Snapshot at close
of stage 12" header).

Phase A locked all 7 D-tags + 29 sub-decisions across a
single combined cluster (mirrors stages 6-11). Decisions
emerged through ordered discussion: D-12-1 -> D-12-2 ->
D-12-3 -> D-12-4 -> D-12-5 -> D-12-6 -> D-12-K.

**Most load-bearing finding: D-12-1.4 reframe under
Option B.** Stage-11 D-11-3 (FnSig location: Item-side)
and D-11-5 (no BodyId; bodies as `Item.body`) were placed
on Item, but this contradicted the rest of the spec's
pre-mono Item model: §16.1 F4 sort comparator carries no
GenericArgs (Items are 1:1 with rustc DefIds; pre-mono);
§13.4 source-derived Items use rustc's DefPathHash
verbatim (per-DefId, not per-(DefId, GenericArgs));
§9.3 RegseqInstanceKey carries `generic_args_key`
alongside `item_id` (one Item, multiple Instances per
generic). Yet stage-3 §15.1 rejects Param/Bound/Placeholder
TyKind variants -- so a generic FnSig with Param TyIds
can't be stored. If `Item.kind.fn_sig` exists for a
generic fn, it must be already-substituted (post-mono).
The two views were incompatible. **Option B locked**:
restore symmetry by moving fn_sig + body to
`InstanceKind::Function { fn_sig, body, flavor,
virtual_idx }`; ItemKind variants become unit (with the
single non-unit `Static { mutability }` carrying source-
level mutability per stage-1 intent + D-12-1.2 (c)).
This **reopens stage-11 D-11-3 + D-11-5** -- tracked in
task #15; flips to `[REOPENED]` in `decisions.md` at
stage-12 merger contract close with annotation
`(revised in stage 12 per D-12-1.4)`. Stage-11 spec
content requires in-place edits in §25.4 (FnSig
reframe), §25.5 (Body reframe), §23.6.2 (callee_fn_sig
helper drops Item indirection), §9.4
(InstanceKind::Function gains fn_sig + body), §3.1
invariant 10 (Body presence reword). Stage-11's other
decisions (D-11-1 Body shape, D-11-4 LocalId, D-11-6
visitor, D-11-7 hybrid timing, D-11-8 push API, D-11-K
rendering deferral) are NOT affected.

**Other Phase A findings worth recording for Phase B
audit-shape capture:**

- **D-12-1.2 (c) Static.mutability moves to
  ItemKind::Static.** Stage-9's placement of
  `mutability: Mutability` on `InstanceKind::Static`
  was a denormalization (statics can't be generic in
  Rust; 1:1 Item:Instance always; mutability is a
  source-level keyword choice). Option B's symmetry
  argument extends here: source-level metadata on
  Item, per-mono data on Instance. **Reopens a
  stage-9 decision** (the one that placed mutability
  on InstanceKind::Static); flag for merger contract.

- **D-12-1.3 Item-level Function-flag fields with
  defaults.** rustc's `CodegenFnAttrs` carries 17+
  flags + multiple sub-fields. Lift only what's
  load-bearing for v1 C codegen of pl011/e1000
  driver fixtures: `inline_hint: InlineHint`
  (4-variant closed), `is_unsafe: bool`, `is_naked:
  bool`. Place at Item-level (not per-variant on
  ItemKind::Function) with sensible defaults for
  non-fn variants; flat schema; simpler match arms.
  Other CodegenFnAttrs flags either fold into
  `Instance.emission_name` per §9.2 (mangling) or
  are out of v1 scope (target_features, optimize,
  alignment, ...).

- **D-12-1.4 Linkage closed 4-variant.** rustc's
  `Linkage` has 9 variants (LinkOnceAny, LinkOnceODR,
  Appending, Common, Internal, Private, External,
  WeakAny, WeakODR). Most are LLVM-specific. Closed
  4-variant `{ Default, NoMangle, Foreign, Weak }`
  covers driver use cases; other rustc variants
  rejected at bridge with `BridgeError::UnsupportedLinkage`
  per §17.4 closed-vocabulary rule. Phase-2 device
  pressure can reopen.

- **D-12-1.5 DomainAttr forward-pointer only.**
  Stage-1 R5.2 anticipated `DomainAttr` on Item /
  Instance for `#[rseq::*]` property attributes;
  stage-4 §1287-1299 explicitly scoped the "no
  general attribute table" wording to stage-3 only
  and confirmed stage-14 owns the property
  vocabulary as sibling tables on `RegseqModel`
  keyed by `ItemId` / `InstanceId` / `FieldPath` /
  `Span`. Stage 12 adds NO schema; only commentary
  updates to §8.0 (drop misleading "no attribute
  table" wording) + §3 forward-pointer note.

- **D-12-1.8 Instance-side pull-through helpers as
  methods on RegseqModel.** Architectural
  enhancement raised mid-Phase-A: every Instance
  has `Instance.item: ItemId`, so Item-level
  metadata is reachable via simple pull-through
  helpers. v1 inventory: `instance_def_path`,
  `instance_span`, `instance_linkage`,
  `instance_link_section`, `instance_inline_hint`,
  `instance_is_unsafe`, `instance_is_naked`,
  `instance_is_phantom_data`, `instance_is_unsafe_cell`,
  `instance_is_manually_drop`,
  `instance_opaque_handle_eligible`, plus
  `instance_fn_sig` and `instance_body` (Option B
  pull-throughs from InstanceKind::Function).
  Composes with §23.6.2 `callee_fn_sig` (existing
  pattern). Methods-on-RegseqModel chosen over
  wrapper type for v1 simplicity; wrapper type can
  be added later if call-site ergonomics demand.

- **D-12-5 5-phase pipeline.** Pinned: Discovery
  (rustc collector) -> Translation+Synthesis
  interleaved (vtables, closures, ConstMemoryStatic
  triggered during translation, NOT pre/post-pass)
  -> F4 sort across source + synthetic Items ->
  ItemId assignment + side-table population (post-
  sort, deterministic) -> Instance construction
  with cross-reference resolution. Cross-reference
  handling during construction (placeholder ItemIds
  vs DefPathHash key) is bridge-implementation
  choice; spec only commits to post-sort
  F4-deterministic ItemIds as the externally-
  observable property. Lands in new §3.4 subsection
  at merger.

Last updated: 2026-05-06 (Phase A complete; Phase B
active).

For top-level project status, see `../status.md`. For
project-wide invariants, see `../README.md`. For stage-12
invariants (workflow, principles, read order, Codex prompt
template), see `memory.md`. For the working spec, see
`spec-snapshot.md` (initial copy; stage-12 contributions
accumulate during Phase B). For inbound forward refs, see
`expectations.md`. For the audit trail, see
`discussions.md` (lazy; created when the first decision
lands). For the round-by-round Codex audit trail, see
`review-log.md` (lazy; created at first review round).

## 0. Inbound expectations checklist

From `expectations.md`. Status flags:
- `[ ]` -- in scope; will close as a stage-12 D-tag locks
- `[OUT-13]` -- out of stage-12 scope; deferred to stage 13
  per the categorical D-12-K decision
- `[CLOSED]` -- already closed by an earlier stage's D-tag
- `[x]` -- closed by a locked stage-12 D-tag

### From stage 1

- [ ] **Expand `Item` placeholder.** The empty struct at
  `crates/regseq-model/src/placeholders.rs` becomes the
  full Item shape per §8.0 (already specified in merged
  spec). Will close jointly with D-12-1 (ItemKind
  expansion); Item struct schema is already pinned at
  §8.0, no reschema needed -- the close-out is the
  ItemKind expansion that completes the schema.
- [ ] **`Item.def_path: DefPath` field.** Already pinned
  at §8.0; stage 12 confirms wired through the bridge
  population path. No D-tag needed (already-spec'd).
- [ ] **Bridge populates `Item.def_path.rendered`.**
  Bridge calls
  `render_def_path(&path, &model.crates, &model.strings)`
  per stage-1 shipped helper, interns the result, stores
  `StringId` on `Item.def_path.rendered`. Will close
  under D-12-5 (bridge construction-pipeline order; the
  rendered-form population is part of the per-Item
  population step).
- [ ] **Fill `pretty.rs::render_def_path(ItemId)` stub.**
  Will close under D-12-6 (stub fill; trivial body
  using the eager `def_path.rendered` cache).
- [ ] **Close construction invariant 6.** Will close
  under D-12-4.2 (`item.def_path.crate_id.index() <
  crates.len()` walk at validate).
- [ ] **Close construction invariant 3.** Will close
  under D-12-4.1 (F4 sort monotonicity walk at
  validate).
- [ ] **Lang-item / instance-kind classification.** Will
  close under D-12-3 (closed 3-flag set per §8.2 +
  AdtFlags-predicate-based recognition + closed-vocab
  rule for non-flagged lang-items).
- [ ] **Bridge-synthesized Items.** Already pinned at §13
  (closure family + vtable family + ConstMemoryStatic).
  Will close under D-12-5 (bridge construction-pipeline
  order confirms synthesis is interleaved with type
  translation; no schema reshape).
- [ ] **Niche optimization for `Option<ItemId>`.** Stage
  1's `lib.rs` asserts. Stage 12 doesn't break it -- the
  ItemKind expansion is internal to ItemKind, not the
  `Option<ItemId>` size assertion. Implicitly addressed;
  no D-tag.

### Bridge concerns (from stage 1)

- [ ] **Closes construction invariant 4
  (spec-crate-stable-id half).** Will close under
  D-12-4.3 (`BridgeInput` parameter on validate;
  `crates[primary_crate].stable_crate_id ==
  bridge_input.spec_crate_stable_id` check).
- [ ] **Item ordering.** Bridge sorts items per F4
  comparator before assigning `ItemId`s. Will close
  under D-12-5 (bridge construction-pipeline order
  pins discovery -> synthesis -> unified F4 sort ->
  ItemId assignment).
- [ ] **`item_def_paths` parallel side table.** Bridge
  populates per-Item with rustc's `DefPathHash` (or
  bridge-derived synthetic hash per §13.4). Will close
  under D-12-5 (population happens at the ItemId
  assignment step alongside `item_synthetic_kinds`).

### From stage 3

- [ ] **Define and synthesise vtable type Items, vtable
  instance Items, vtable shim Function Items, and
  closure synthesis details** (from stage 3 D5 and D6).
  Synthesis machinery already pinned at §13.1 + §13.2 +
  §13.3 + §13.4. Will close under D-12-5 (bridge
  construction-pipeline order confirms synthesis is
  internally consistent with the expanded ItemKind:
  vtable type Items use `ItemKind::Struct`; vtable
  instance Items use `ItemKind::Static`; vtable method
  shim + drop shim Function Items use
  `ItemKind::Function` with the expanded
  `{ fn_sig, body }` schema; closure captures-struct
  Items use `ItemKind::Struct`; closure call-method
  Function Items + drop-glue Function Items use
  `ItemKind::Function`). The DefPathData encoding
  question stage-9 left open for stage 12 is already
  pinned at §13.4 (`TypeNs("<regseq_synth>")` prefix).

### From stage 4

- [CLOSED] **`SyntheticItemKind::ConstMemoryStatic`
  family.** Closed at stage 4 under D-4-3 (whole-family
  scope including stage-12-territory contribution).
  Stage 12 inherits by reference; no spec work.

### From stage 9

- [CLOSED] **DefPathData encoding for synthetic vtable
  scopes.** Closed at stage 9 (reframed as stage-12
  decision; §13.4 already pins the encoding). No
  separate stage-12 work; D-12-5 confirms.
- [ ] **Vtable shim Function Item synthesis cross-ref.**
  Informational; the shim Items use the expanded
  `ItemKind::Function` schema. Will close under D-12-1
  (Function variant expansion confirms shim Items fit).

### From stage 11

- [ ] **`Item.kind = Function { fn_sig, body, ... }`
  schema expansion** (D-11-3 + D-11-5). Will close
  under D-12-1 + sub-decisions (cardinality, Function
  field schema, non-Function variants stay unit,
  accessor pattern).
- [ ] **Construction invariant 9 closure** (D-11-6.1
  B-7). Stage 11 owns the joint-validation visitor's
  closure check (`Direct(InstanceId)` resolves in
  `model.instances`); stage 12 owns `model.instances`
  integrity (one-to-one with Items; populated by
  collector; exhausted by validator). No schema work
  required at stage 12; closes implicitly under D-12-5
  (bridge construction-pipeline order pins
  `model.instances` population).

## 1. D-tag allocation

D-tags allocated upfront from grounded rustc + prior-art
evidence (per `../README.md` §5.1 step 5). Sub-decisions
allocated during Phase A as evidence demands.

| Tag       | Status    | Locked content (one-line summary)                                                                                                                                                                         |
|-----------|-----------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| D-12-1    | [LOCKED]  | Item schema + ItemKind shape: 6-variant flat ItemKind; per-mono data on InstanceKind (Option B); Item-level typed metadata per stage-1 R5.2.                                                              |
| D-12-1.1  | [LOCKED]  | Cardinality: closed 6-variant flat `{ Function, Static, Const, Struct, Enum, Union }`. Aligns with rustc-internal first-class ADT representation.                                                         |
| D-12-1.2  | [LOCKED]  | `ItemKind::Static { mutability: Mutability }`; Const/Struct/Enum/Union stay unit. Mutability moves from InstanceKind::Static to ItemKind::Static (option c) per stage-1 intent. **Reopens stage-9.**     |
| D-12-1.3  | [LOCKED]  | Item-level typed fields: `linkage: Linkage` (4-variant), `link_section: Option<StringId>`, `inline_hint: InlineHint` (4-variant), `is_unsafe: bool`, `is_naked: bool`. Visibility omitted.                |
| D-12-1.4  | [LOCKED]  | **Option B reframe**: `InstanceKind::Function { fn_sig: FnSigId, body: Option<Body>, flavor, virtual_idx }`. Restores symmetry. **Reopens stage-11 D-11-3 + D-11-5** (task #15).                          |
| D-12-1.5  | [LOCKED]  | DomainAttr forward-pointer commentary only at §8.0 + §3; no stage-12 schema. Stage-14 owns property vocabulary on RegseqModel root sibling tables per stage-4 §1287-1299.                                  |
| D-12-1.6  | [LOCKED]  | Hybrid accessor: `InstanceKind::as_function() -> Option<&FunctionInstanceData>` + match-arm-readable. Named `FunctionInstanceData` struct extracted from struct-variant.                                  |
| D-12-1.7  | [LOCKED]  | Stage-11 D-11-3 + D-11-5 reopening process: flip to `[REOPENED]` in `decisions.md` at stage-12 merger close per README §5.5 step 4 with annotation `(revised in stage 12 per D-12-1.4)`.                  |
| D-12-1.8  | [LOCKED]  | Instance-side pull-through helpers: methods on `RegseqModel` (13-helper v1 inventory). Composes with §23.6.2 callee_fn_sig pattern.                                                                       |
| D-12-2    | [LOCKED]  | Foreign / extern declaration dispatch.                                                                                                                                                                     |
| D-12-2.1  | [LOCKED]  | `InstanceKind::Function.body: None <=> tcx.is_foreign_item(did)`. fn_sig populated for foreign fns (rustc tcx.fn_sig is callable). flavor: Item; virtual_idx: None.                                       |
| D-12-2.2  | [LOCKED]  | `InstanceKind::Static.init: None <=> tcx.is_foreign_item(did)`. Mutability lives on `ItemKind::Static` per D-12-1.2 (c); InstanceKind::Static is now `{ ty, init }` (2-field).                            |
| D-12-2.3  | [LOCKED]  | Linkage translation: is_foreign->Foreign; NO_MANGLE/export_name->NoMangle; WeakAny/WeakODR->Weak; otherwise Default. Other rustc Linkage variants rejected with BridgeError::UnsupportedLinkage.          |
| D-12-2.4  | [LOCKED]  | `link_name` stays Instance-side (folded into `Instance.emission_name` per §9.2); informational restatement of D-12-1.3 omit.                                                                              |
| D-12-3    | [LOCKED]  | Lang-item recognition mechanism.                                                                                                                                                                            |
| D-12-3.1  | [LOCKED]  | Closed 3-flag set carries (`is_phantom_data`, `is_unsafe_cell`, `is_manually_drop`); no expansion in v1. is_pin/is_c_void/is_box deferred per documented criteria.                                        |
| D-12-3.2  | [LOCKED]  | Recognition via AdtFlags predicates: `adt_def.is_phantom_data()` etc. at bridge type-translation time. O(1) bitflag tests.                                                                                |
| D-12-3.3  | [LOCKED]  | Non-flagged lang-items queried on-demand at consumer sites: `tcx.is_lang_item(did, LangItem::*)` / `tcx.lang_items().X()`. Never stored on Item.                                                          |
| D-12-3.4  | [LOCKED]  | Reject opaque `LangItem` tag and Charon-style string-based `lang_item: Option<String>`. Closed-vocabulary discipline per §17.4.                                                                            |
| D-12-4    | [LOCKED]  | Construction-invariant closure for stage-1 deferrals (3 invariants).                                                                                                                                       |
| D-12-4.1  | [LOCKED]  | Invariant 3 closure: F4 sort monotonicity walk at validate; non-strict `key[i] <= key[i+1]`; landing site Phase B step 3 (d).                                                                              |
| D-12-4.2  | [LOCKED]  | Invariant 6 closure: `item.def_path.crate_id.index() < crates.len()` walk; landing site Phase B step 3 (c).                                                                                               |
| D-12-4.3  | [LOCKED]  | Invariant 4 closure: `BridgeInput { spec_crate_stable_id }` parameter on validate; `validate(&self, &BridgeInput)` signature change; landing site Phase B step 3 (e).                                     |
| D-12-5    | [LOCKED]  | Bridge construction-pipeline order: 5-phase. New §3.4 subsection.                                                                                                                                          |
| D-12-5.1  | [LOCKED]  | Canonical 5-phase pipeline: Discovery -> Translation+Synthesis -> F4 Sort -> ItemId Assignment + side-tables -> Instance Construction.                                                                    |
| D-12-5.2  | [LOCKED]  | Synthesis interleaved during Phase 2 translation; NOT a separate pre/post-pass. Idempotent caching via §13 side tables.                                                                                   |
| D-12-5.3  | [LOCKED]  | Post-sort ItemId assignment. Cross-reference handling during construction (placeholder ItemIds vs DefPathHash key) is bridge-implementation choice.                                                       |
| D-12-5.4  | [LOCKED]  | New §3.4 subsection "Bridge construction-pipeline order" lands at stage-12 merger.                                                                                                                         |
| D-12-6    | [LOCKED]  | `pretty.rs::render_def_path(ItemId)` stub fill.                                                                                                                                                            |
| D-12-6.1  | [LOCKED]  | One-line body: `self.model.strings.resolve(self.model.items[id].def_path.rendered).to_owned()`. Cache lookup via eager `Item.def_path.rendered`.                                                          |
| D-12-6.2  | [LOCKED]  | `render_def_path(ItemId)` (ItemId convenience) and `render_def_path_value(&DefPath)` (value-level renderer) coexist with documented division of labor.                                                    |
| D-12-6.3  | [LOCKED]  | §18.2 per-method ownership table updates: `render_def_path` row flips from "deferred to stage 12" to "stage 12; D-12-6".                                                                                  |
| D-12-K    | [LOCKED]  | Categorical Item-level rendering deferral to stage 13, compositional with D-11-K cascade. Only `render_def_path` (D-12-6) stays at stage 12.                                                              |
| D-12-K.1  | [LOCKED]  | Deferral list: render_item, ItemKind variant rendering, Linkage rendering, InlineHint rendering, Function-flag rendering, link_section rendering, lang-item flag rendering, opaque_handle_eligible.       |
| D-12-K.2  | [LOCKED]  | Outbound bundle of 7 entries to file in `stage-13/expectations.md` "From stage 12 (per D-12-K)" at merger close. No predecessor K-tag retargeting (purely additive).                                       |
| D-12-K.3  | [LOCKED]  | §18.2 per-method ownership table gets new "deferred to stage 13 per D-12-K" rows for the 7-entry bundle.                                                                                                   |

Locked-tag count: **7 main + 29 sub-decisions = 36
locks-in-conversation**. `[LOCKED]` flag denotes "locked
during Phase A; awaiting Phase B audit-shape commit +
per-stage Codex review APPROVE before flipping to final
`[LOCKED]`." Status flips to `[REOPENED]` only if a
Phase B Codex review round surfaces a substantive issue
requiring decision revision (rare).

**Cross-stage reopenings flagged for stage-12 merger
close** (per D-12-1.7 process):
- **Stage-11 D-11-3** (FnSig location: Item-side) ->
  `[REOPENED]` per D-12-1.4 Option B; FnSig now lives on
  `InstanceKind::Function`.
- **Stage-11 D-11-5** (no BodyId; bodies as `Item.body`)
  -> `[REOPENED]` per D-12-1.4 Option B; bodies now
  live as `body: Option<Body>` on `InstanceKind::Function`.
- **Stage-9 InstanceKind::Static.mutability placement**
  -> `[REOPENED]` per D-12-1.2 (c); mutability moves to
  `ItemKind::Static`. (Specific stage-9 D-tag to
  identify at merger contract close.)

## 2. Active cluster

**Phase A complete 2026-05-06.** Single combined cluster
covered all 7 D-tags + 29 sub-decisions in conversation.
Final Phase A discussion order: D-12-1 -> D-12-2 ->
D-12-3 -> D-12-4 -> D-12-5 -> D-12-6 -> D-12-K (matched
the planned order; no cluster split needed).

Substantive Phase A discussion threads worth recording
for Phase B audit-shape capture:

- **D-12-1.4 Option B reframe (load-bearing).**
  Mid-Phase-A user question surfaced the asymmetry: for
  Static / Const / Struct / Enum / Union, per-mono data
  lives on InstanceKind; but stage-11 D-11-3 + D-11-5
  put fn_sig + body on Item. Investigation revealed a
  contradiction: §16.1 F4 sort + §13.4 source DefPathHash
  + §9.3 RegseqInstanceKey all imply Items are pre-mono
  (1:1 with rustc DefIds), but stage-3 §15.1 rejects
  Param/Bound TyKind variants — so a stored FnSig must
  be already-substituted (post-mono). The two views
  were incompatible. Option B locks: restore symmetry
  by moving fn_sig + body to `InstanceKind::Function`.
  Stage-11's 4-peer convergence citation was
  re-examined and found to actually support per-Instance
  body (rustc_public's `Instance::body() -> Option<Body>`
  is the cited pattern, not `Item.body`). Reopens
  stage-11 D-11-3 + D-11-5; minimal stage-11 spec edits
  required (~50-100 lines of in-place updates to §25.4,
  §25.5, §23.6.2, §9.4, §3.1 inv 10).

- **D-12-1.2 (c) Static.mutability placement.** Stage-1
  spec-snapshot.md:840-841 explicitly placed mutability
  on `ItemKind::Static { mutability, .. }`; current
  spec.md §9.4 has it on `InstanceKind::Static`. Option
  B's symmetry argument extends here: source-level
  metadata on Item, per-mono on Instance. (c) honors
  stage-1 intent + matches Option B + statics-can't-be-
  generic guarantees the move is sound. Reopens a
  stage-9 decision (which stage-9 D-tag specifically
  identified at merger contract close).

- **D-12-1.5 stage-3-scoped "no attribute table" wording
  needs §8.0 update.** Stage-4 spec-snapshot.md:1287-1299
  explicitly scoped the "no general attribute table"
  claim to stage 3 only, AND committed to stage-14
  `DomainAttr` sibling tables on `RegseqModel` keyed
  by `ItemId` / `InstanceId` / `FieldPath` / `Span`.
  The merged spec.md dropped the "scoped to stage 3"
  qualifier when absorbing stage-4 content. Stage 12
  restores the framing: typed Item fields per
  D-12-1.3 land at stage 12; structured DSL property
  attributes (`#[rseq::*]`) land in stage-14 sibling
  tables. No stage-12 schema change for DomainAttr.

- **D-12-1.8 architectural enhancement:** the
  `Instance.item: ItemId` back-reference enables
  pull-through helpers on `RegseqModel` for Item
  metadata (e.g., `model.instance_linkage(id)`). v1
  inventory: 12 helpers covering identity, source-level
  metadata, lang-item flags, opaque_handle outcome,
  plus the post-Option-B fn_sig and body pull-throughs.
  Methods-on-RegseqModel chosen over wrapper-type
  for v1 simplicity.

- **D-12-2.3 closed Linkage 4-variant.** rustc's
  Linkage has 9 variants (LinkOnceAny, LinkOnceODR,
  Appending, Common, Internal, Private, External,
  WeakAny, WeakODR). Most are LLVM-specific. Closed
  4-variant `{ Default, NoMangle, Foreign, Weak }`
  covers driver use cases; other rustc variants
  rejected at bridge per §17.4 closed-vocabulary
  rule. Phase-2 device pressure can reopen.

- **D-12-5 5-phase bridge pipeline.** Discovery
  (rustc collector) -> Translation+Synthesis
  interleaved (vtables, closures, ConstMemoryStatic
  triggered during translation per §13) -> F4 sort
  across source + synthetic Items -> ItemId
  assignment + side-table population (post-sort,
  deterministic) -> Instance construction. Cross-
  reference handling during construction is
  bridge-implementation choice; spec only commits to
  post-sort F4-deterministic ItemIds as the
  externally-observable property.

**Phase B active (next).** Spec commit + Codex review.

## 3. Codex review cadence (status)

**Phase B active 2026-05-06.** Per-stage review
trajectory TBD; rounds R1, R2, ... will be appended
here as they run, recording verdict + issue counts +
close-out status. Codex prompt template is in
`memory.md` §4.

## 4. Open phase set-up recap

For audit / methodology trace:

- [x] Stage-12 folder bootstrapped (already existed with
  `expectations.md` from earlier-stage forward refs;
  no creation needed).
- [x] `spec-snapshot.md` copied from top-level
  `../spec.md` (11163 lines) as Phase A working
  starting point.
- [x] `expectations.md` read as entry checklist
  (sizable bundle covering stages 1, 3, 9, 11; stage 4
  already CLOSED by D-4-3).
- [x] Rustc kickoff evidence pass completed:
  - `rustc_hir/src/definitions.rs:274-321`
    (DefPathData 19-variant confirmation; matches
    spec.md §7.1).
  - `rustc_middle/src/ty/instance.rs:64-185`
    (InstanceKind 15-variant confirmation; ReifyShim
    payload drift surfaced -- now `(DefId,
    Option<ReifyReason>)` -- non-stage-12 concern,
    flagged as F-12-Reify cross-stage finding).
  - `rustc_span/src/def_id.rs:93-116` (DefPathHash =
    Fingerprint(StableCrateId, Hash64); matches
    spec.md §7.3).
  - `rustc_hir/src/def.rs:100-196` (DefKind 31-variant
    survey; 6 map cleanly to ItemKind, 11 reject/lift/
    erase, 14 surface as DefPath segments only).
  - `rustc_hir/src/lang_items.rs:79-451` (LangItem
    enum 203-208 variants; recognition via
    `tcx.is_lang_item(did, LangItem::X)` +
    AdtFlags predicates for ADT lang-items).
  - `rustc_middle/src/ty/adt.rs:30-63, 414-440`
    (AdtFlags bits + predicate methods
    `is_phantom_data` / `is_unsafe_cell` /
    `is_manually_drop` / `is_pin` / `is_box`).
  - `rustc_middle/src/middle/codegen_fn_attrs.rs:61-214`
    (`CodegenFnAttrs` field inventory: 17+ flags +
    inline / optimize / symbol_name / link_section /
    link_name / target_features / linkage /
    alignment).
  - `rustc_middle/src/hir/mod.rs:165-168`
    (`is_foreign_item(did)` predicate: parent
    `DefKind == ForeignMod`).
  - `rustc_monomorphize/src/collector.rs:1059-1062,
    1803` (collector skips foreign items; entry point
    `collect_crate_mono_items`).
  - `rustc_middle/src/query/mod.rs` (per-DefId query
    inventory: `def_kind`, `fn_sig`,
    `codegen_fn_attrs`, `intrinsic_raw`, `adt_def`,
    `is_mir_available`, `visibility`, etc.).
  - `rustc_public/src/lib.rs:120-183`
    (rustc_public ItemKind = 4-variant {Fn, Static,
    Const, Ctor(CtorKind)}; CrateItem methods
    including `is_foreign_item()` boolean).
  - `rustc_public/src/mir/mono.rs:14-40`
    (rustc_public InstanceKind = 4-variant lossy
    collapse {Item, Intrinsic, Virtual{idx}, Shim}).
  - `rustc_middle/src/mir/mod.rs:119-142`
    (mir::Body owner linkage via
    `body.source.instance.def_id()`).
- [x] D-tags allocated at Open phase (7 main + 14
  initial sub-decisions = 21 locks). During Phase A
  discussion, sub-decisions expanded to 29 (Phase-A-
  surfaced sub-decisions on D-12-1 added .5 / .6 / .7 /
  .8; D-12-2 expanded to .1-.4; D-12-3 expanded to
  .1-.4; D-12-5 expanded to .1-.4; D-12-6 expanded to
  .1-.3; D-12-K expanded to .1-.3). **Final
  locked-tag count: 7 main + 29 sub-decisions = 36
  locks-in-conversation.**
- [x] Per-stage prior-art research landed in 4
  primary-source files (per-D-tag synthesis +
  per-project takeaways):
  - `prior-art/rustc.md` (~530 words; rustc_hir/
    rustc_middle 14-variant ItemKind + dedicated
    ForeignItem struct + AdtFlags + LangItem dual
    recognition).
  - `prior-art/rustc_public.md` (~490 words; stable
    IR 4-variant minimal ItemKind + `is_foreign_item()`
    boolean + no lang-item API).
  - `prior-art/charon.md` (~550 words; LLBC
    orthogonal split ItemKind + TypeDeclKind +
    `Result<Body, Opaque>` + string-based
    `lang_item: Option<String>`).
  - `prior-art/mrustc.md` (~650 words; Rust-to-C++
    category-based tagged unions + Linkage-based
    foreign + TraitMarkings cached trait flags).
- [x] `memory.md` drafted (workflow, principles, read
  order, Codex template) from grounded knowledge.
- [x] `status.md` drafted (this file) from grounded
  knowledge.

## 5. Next concrete unit of work

**Phase B: spec commit + Codex review.**

Three sub-units of Phase B work, sequenced:

1. **Audit-shape entries in `stage-12/discussions.md`.**
   For each of the 7 locked D-tags (with sub-decisions),
   write a per-decision entry with the audit shape
   (Decision / Reasoning / Methodology rules invoked /
   Prior-art evidence / Worked example). Estimated
   600-1200 lines total (D-12-1 is the largest entry --
   the Option B reframe substantively engages with
   stage-11 D-11-3 + D-11-5; D-12-5 is also substantial
   given the 5-phase pipeline; D-12-3 / D-12-6 are
   smallest). Stage-11's discussions.md (10 D-tags) is
   the size benchmark.

2. **Spec content in `stage-12/spec-snapshot.md`.**
   In-place edits per `../README.md` §5.9 (live, not
   historical; topic-based; audit shape stripped). Key
   sections to edit:
   - **§3 root struct**: forward-pointer note for
     stage-14 `DomainAttr` sibling tables (D-12-1.5);
     no schema change.
   - **§3.1 invariant 10**: reword for Option B Body
     placement on `InstanceKind::Function`.
   - **§3.3 Phase B step 3**: reordered to
     (a)..(g) per D-12-4 invariant-closure landing.
   - **New §3.4**: "Bridge construction-pipeline
     order" with the 5-phase contract per D-12-5.
   - **§8 Item schema**: add Item-level fields per
     D-12-1.3 (linkage, link_section, inline_hint,
     is_unsafe, is_naked); ItemKind 6-variant per
     D-12-1.1 with `Static { mutability }` per
     D-12-1.2 (c).
   - **§8.0 commentary**: drop misleading "no general
     attribute table" wording; replace with
     stage-12-vs-stage-14 split per D-12-1.5.
   - **§8.2 lang-item flags**: reaffirm closed 3-flag
     set per D-12-3.
   - **§9.4 InstanceKind**: `Function { fn_sig: FnSigId,
     body: Option<Body>, flavor, virtual_idx }` per
     D-12-1.4 (Option B); `Static { ty, init }`
     (mutability removed per D-12-1.2 (c)).
   - **§14.3 / §14.4 FnSig**: location reframe from
     Item-side to Instance-side per Option B.
   - **§18.2 per-method ownership table**:
     `render_def_path` flips to "stage 12" per D-12-6;
     7 new "deferred to stage 13 per D-12-K" rows for
     the Item-level rendering bundle.
   - **§23.6.2 callee_fn_sig helper**: drops Item
     indirection; resolves FnSig directly via Instance.
   - **Stage-11 territory in §25.4 / §25.5**: reframe
     under Option B (FnSig per Instance; Body per
     Instance).
   - **New typed enums**: `Linkage` (4-variant) +
     `InlineHint` (4-variant) + `BridgeInput` struct +
     13-helper Instance-side method inventory
     (D-12-1.8).

   Estimated 400-800 lines of net spec content
   (mostly in-place updates; new content is the §3.4
   subsection + Linkage/InlineHint enum definitions +
   BridgeInput struct + helper-method declarations).

3. **Codex per-stage review loop** with template from
   `memory.md` §4. Rounds R1, R2, ... iterate until
   APPROVE. Each round records in
   `stage-12/review-log.md` with verdict + issue
   counts + corrections. Stage-9 / stage-10 / stage-11
   trajectory benchmark: 5-12 rounds typical for this
   size of cluster; stage-12's smaller surface (7
   D-tags, mostly reframe + close-out work) suggests
   lower-end of the band.

After Phase B APPROVES: stage-12 merger contract per
README §5.5 7-step protocol (file outbound bundles +
flip stage-11 D-11-3 / D-11-5 to `[REOPENED]` per task
#15 + flip the relevant stage-9 InstanceKind::Static
mutability decision to `[REOPENED]` per D-12-1.2 (c)).

## 6. Blockers

None. All upstream dependencies are pinned:

- Stage 1 (foundation crate; ID conventions; DefPath
  shipping; placeholder Item) -- closed.
- Stage 3 (type system; D5 dyn-Trait dispatch; D6
  closure family; bridge-synthesised Item machinery
  at §13) -- closed.
- Stage 4 (constants; D-4-3 ConstMemoryStatic,
  whole-family scope) -- closed.
- Stage 9 (terminators; DefPathData encoding for
  synthetic vtable scopes resolved as stage-12
  decision; §13.4 already pins the encoding) -- closed.
- Stage 11 (MIR bodies; D-11-3 + D-11-5 outbound
  bundle; FnSig + Body on Item) -- closed.

Stages 13 (pretty-printing) and 14 (domain-specific
extensions) are downstream and depend on stage-12
output.

## 7. Deferred items (stage-12-specific)

To be populated as Phase A surfaces them. Stage-specific
deferrals live here; cross-stage deferrals live in
`../status.md` "Deferred items (cross-stage)".

(none yet)

## 8. Cross-stage findings (informational)

Findings surfaced during stage-12 evidence pass that are
NOT stage-12 D-tags but worth recording for cross-stage
coherence:

- **F-12-Reify: ReifyShim payload drift (stage-9
  territory).** Rustc's `ty::InstanceKind::ReifyShim`
  payload has expanded to `(DefId, Option<ReifyReason>)`
  since stage-9 was written; the additional
  `ReifyReason` distinguishes vtable-vs-fn-pointer
  reification contexts (per
  `rustc_middle/src/ty/instance.rs:40-60` documentation
  block). This may break stage-9's `RegseqInstanceKey`
  injectivity invariant (two `ReifyShim` instances with
  different `ReifyReason` would share `(rank, did,
  args)` and collapse). Stage 12 is Item-level; this is
  a stage-9 Instance-level concern. **Action:** flag in
  stage-12 outbound; if codegen / phase-2 device
  pressure surfaces collisions, file a stage-9
  reopening question. Otherwise, document and leave to
  next stage-9 review. Reference:
  `rustc_middle/src/ty/instance.rs:40-185`.

## 9. Outbound expectations to file at Phase B close

To file at Phase B close (after per-stage Codex
APPROVES, before merger contract). Targets:

### To `stage-13/expectations.md` "From stage 12 (per D-12-K)"

Categorical retargeting bundle (operational consequence
of D-12-K). Item-level rendering items that defer to
stage 13:

1. **Item display rendering.** A `pretty.rs` helper
   that renders an `Item` (DefPath + kind + flags +
   spans) as a one-line summary or multi-line dump.
   Stage-12 owns `render_def_path` (D-12-6); broader
   Item display defers.
2. **ItemKind variant rendering.** Per-variant
   formatting for `ItemKind::Function { fn_sig, body }`
   etc. Composes with body-level rendering already
   retargeted from D-11-K.
3. **FunctionData pretty-printing.** The fn_sig +
   body presentation in pretty-printed form for
   snapshot / diagnostic dumps.
4. **Lang-item flag rendering.** The 3 §8.2 flags
   surface in pretty-printed form (e.g., `[#[lang =
   "phantom_data"]]` annotation).
5. **`opaque_handle_eligible` flag rendering.** Bridge
   outcome flag; deserves a small render hook.

Predecessor-stage attribution: stage 12 (Items).

### To `stage-14/expectations.md` "From stage 12"

1. **Q7 intrinsic recognition mechanism.** Stage 14
   builds on stage-12's lang-item-recognition pattern
   (closed-vocabulary + `tcx.intrinsic_raw(did)` + per-
   item attribute storage on `#[rseq::intrinsic(NAME)]`
   callees) for intrinsic dispatch tables, ordered
   regions, MMIO, ISR classification. Stage-12's
   `is_phantom_data`/`is_unsafe_cell`/`is_manually_drop`
   pattern is the template; stage-14 generalises.
2. **Per-Item attribute table for declarative
   properties** (`#[rseq::mmio(access_width = 32)]`,
   `#[rseq::interrupt(trigger_type = "...")]`, etc.).
   Stage-12 §8.0 explicitly says "No general attribute
   table on Item" -- stage 14 may revisit this if
   declarative property attributes need per-Item
   storage. Stage-12 default: confirm no attribute
   table; stage 14 adds if needed.

## 10. How to update this file

Per `../README.md` §8 conventions:

- Update `Last updated:` at the top when material state
  changes (decisions lock, Codex rounds run, blockers
  appear or clear, outbound bundles get filed).
- Keep this file lean -- cross-stage signals belong in
  `../status.md`; per-decision audit shape belongs in
  `discussions.md`; per-round Codex audit belongs in
  `review-log.md`.
- This file is per-stage-internal and not touched by
  the merger (per `../README.md` §5.8).
