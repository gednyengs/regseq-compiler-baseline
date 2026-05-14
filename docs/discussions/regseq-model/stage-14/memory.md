# Stage 14 memory

Stage briefing for **Stage 14: Phase-1 foundation
extensions**. Per `../README.md` §4 folder layout, this
file captures workflow, methodology principles for this
stage, the read order, the D-tag discussion agenda, and
cross-stage interactions. The companion `status.md`
tracks current state (locked decisions, active focus,
next action).

## Scope

Stage 14 covers the phase-1 closure spec-side per the
2026-05-12 strategy revision (see `../README.md` §1.3 +
§2's "Domain extensions iterate per stage in phase 2"
rule). The original monolithic "Stage 14: Domain-specific
extensions" plan was superseded; per-domain-family work
moved to phase-2 stages 15-18+. This stage owns the
**foundation** that those phase-2 stages will build on:

1. **Q7 intrinsic recognition mechanism + storage**
   (corrected per external-review Finding #1).
2. **`IntrinsicKind` closed vocabulary with diagnostic-
   logging entries only** (MMIO / atomic / interrupt /
   clock entries live in stages 16-19+).
3. **DomainAttr sibling-table architecture** on
   `RegseqModel` -- specified abstractly; the intrinsic
   table is the first concrete instance.
4. **Three composer renderers**: `render_const`,
   `render_instance`, top-level `render` -- the final
   foundation rendering surface per stage-13 D-13-4.6.
5. **`core::ffi::c_void` rejection enforcement** -- the
   one universal bridge rejection rule (not tied to any
   single domain family).
6. **External-review Findings #1-#4** (2026-05-07):
   intrinsic recognition correction (folds into #1
   above); `#[naked]` rejection + `is_naked` field
   removal; stale `emission_name` forward-pointer
   retarget; Specimen 70 split.

After stage-14 close: implementation sweep #1 (covers
stages 1-14 in one pass), generic-C backend (hosted
userspace, libc, `main()`, `printf`), and phase-1 test
suite (DSL -> IR -> C -> captured-stdout integration
tests). Then phase 2 opens with Stage 15.

## Methodology principles for this stage

Read `../README.md` §2 in full. The rules below are the
ones most relevant to stage-14's open work; they are
binding, not advisory.

- **Spec before implementation.** Stage 14 is spec-only.
  Implementation sweep follows stage-14 close per the
  README §2 amendment. Spec / impl do not interleave
  within this stage.

- **No defensive fallbacks.** The toolchain is pinned
  (nightly-2026-02-01-aarch64-apple-darwin per
  `rustc-references.md`). The Q7 recognition path uses
  one specific rustc API; do not add fallbacks for "what
  if the API moves." If the chosen path breaks under a
  future toolchain bump, we update the spec at that
  point.

- **Compositional behavior beats special-case flags.**
  The DomainAttr sibling-table architecture (D-14-3) is
  the architectural precedent for stages 15-19+. Resist
  the temptation to add per-Item operational-intrinsic
  flags or per-family marker bits when the sibling-table
  composition naturally covers the case. The architecture
  IS the mechanism.

- **C semantics with information preservation.** The
  intrinsic-table stores RESOLVED `IntrinsicKind`, not
  the raw `#[rseq::intrinsic("NAME")]` attribute string.
  rustc has done the attribute-walking work; store the
  result, not the input. (Same posture as stage-12 D-12-1
  fn_sig / body Item-side storage.)

- **Code does not reference discussion-folder paths.**
  Stage-14 introduces new constructs (`IntrinsicKind`,
  `IntrinsicInfo`, `intrinsic_items` field, DomainAttr
  sibling-table key-kind set) that need stable
  architectural references in code comments. Use D-tag
  references (`D-14-1`, `D-14-3`) or restate the rule
  inline; do NOT cite
  `stage-14/discussions.md`-style paths.

- **Compiler-theory-first.** The Q7 mechanism choice
  + DomainAttr architecture choice are both decisions
  with established compiler-design precedent
  (closed-vocabulary attribute-driven dispatch tables;
  sibling tables for sparse / family-specific metadata).
  Lean on the prior-art evidence in step 6.

- **Forward expectations live in
  `stage-N/expectations.md`.** Stage-14 surfaces
  concerns for phase-2 stages 15-18+. File those in the
  appropriate `stage-15/expectations.md` /
  `stage-16/expectations.md` / `stage-17/expectations.md`
  / `stage-18/expectations.md` -- not in
  `stage-14/expectations.md`.

## Read order

Read in this order before starting work. The expectations
file already enumerates the inbound forward refs; this
ordering is for the broader project context.

1. **`memory.md`** -- this file.
2. **`status.md`** -- current state + active focus +
   D-tag inventory + next action.
3. **`expectations.md`** -- inbound forward refs
   (foundation-only after the 2026-05-12 trim;
   per-family items live at `../stage-15/expectations.md`
   etc.).
4. **`../README.md`** -- project-wide methodology +
   workflow. Particularly §1.3 (three-phase
   methodology), §2 (binding methodology rules including
   the "Dual backend" + "Domain extensions iterate per
   stage in phase 2" amendments), §5.1 (stage
   lifecycle), §5.5 (merger contract), §5.9 (spec.md
   content rules).
5. **`../decisions.md`** -- registry of locked D-tags
   from prior stages. Critical entries for stage 14:
   - D-3-1.13 (D9 in legacy form) -- MMIO volatility
     on InstanceId.
   - D-3-1.14 (D10 in legacy form) -- atomic ordering
     on InstanceId.
   - D-3-1.15 (D16 in legacy form) -- c_void
     rejection.
   - D-12-1.3 -- typed Item-level fields including
     `is_naked` (Finding #2 will reopen this).
   - D-12-1.5 -- DomainAttr forward-pointer to
     stage-14 sibling-table architecture.
   - D-12-5 -- canonical 5-phase bridge construction-
     pipeline order (intrinsic-table population fits
     somewhere in this pipeline).
   - D-12-6 -- `render_def_path(ItemId)` stub fill
     (composer renderers compose this).
   - D-13-1 -- master rendering style (composers
     inherit).
   - D-13-2 -- body envelope + indentation + sigil
     placement (composers inherit).
   - D-13-3 -- span / debuginfo gating (composers
     inherit).
   - D-13-4.3 -- per-flag Item-level rendering table
     (Finding #2 reopens to remove `#[naked]` row).
   - D-13-4.6 -- building-block-vs-composer
     architectural division (the composer trio is
     this stage's payload).
   - D-13-5 / D-13-6 -- `render_ty` + body-level
     renderer per-arm specs (composers consume).
   - D-13-7 -- worked-example specimens (Specimen 70
     is the Finding #4 target).
   - D-13-9 -- spec split into co-equal
     `spec.md` + `pretty-printer.md`.
   - D-9-9 -- 9-row rejection table (Finding #2
     reopens to add `#[naked]` row).
6. **`../rustc-references.md`** -- pinned rustc-src
   paths. This file currently lacks paths for
   tool-namespace attribute machinery + custom-
   attribute walking + intrinsic-recognition codepaths;
   the evidence pass in stage-14 step 4 will surface
   them and they will be appended.
7. **`spec-snapshot.md`** -- current spec (working
   copy copied 2026-05-10 from top-level). Critical
   sections for stage 14:
   - §3 / §3.1 -- `RegseqModel` root schema (the
     intrinsic-table + DomainAttr sibling-tables attach
     here).
   - §3.3 -- deserialise validation phases (population
     timing + validator hook points for the new
     sibling tables).
   - §3.4 -- canonical 5-phase bridge construction-
     pipeline (intrinsic-table population fits in
     Phase 2 Translation+Synthesis).
   - §8 -- Item schema. **§8 :1754-1758 is the
     forward-pointer paragraph Finding #1 corrects in
     place.** §8 :1631-1636 (`is_naked` field +
     comment) is the Finding #2 deletion target. §8
     :1738 is the field-enumeration narrative
     Finding #2 trims. §8 :1888-1892 is the stale
     "stage 13 (codegen)" forward-pointer Finding #3
     retargets.
   - §13.2 -- trait-surface design rule (composers
     producing renderings for `dyn Trait` instances
     inherit this).
   - §15.1 -- rejection table (c_void rejection
     enforcement; `#[naked]` rejection row added per
     Finding #2 in §23.6.1-ish).
   - §9.7 -- `ConstInterner` and the `Const` vocabulary
     (composer `render_const` input; closed 11 variants:
     `Bool / Char / Int / Uint / Float / ZST / Bytes /
     Aggregate / EnumValue / NullPtr / Reloc`).
     Reference-typed const handling (`&str` / `&[T]` ->
     `Aggregate([Reloc, Uint])`+`ConstMemoryStatic` synth)
     follows the same section.
   - §22.4 -- `TerminatorKind::Call` (intrinsic
     dispatch consumer; classifier walks
     `intrinsic_items` table).
   - §23.6.1 (or wherever Item-translation rejections
     land) -- Finding #2 adds `#[naked]` rejection
     row.
8. **`pretty-printer-snapshot.md`** -- current
   pretty-printer spec. Critical sections for stage
   14:
   - §6 -- `render_ty` per-arm specs (composer
     `render_const` + `render_instance` consume).
   - §7 -- body-level renderer per-arm specs
     (composer `render_instance` consumes for
     Function-Instance bodies).
   - §9 -- worked-example specimens (stage-14
     composers add ~10-20 specimens here; **Specimen
     70 :1460-1475 is the Finding #4 fix target**).
   - per-flag rendering table at D-13-4.3 surface
     (Finding #2 removes the `#[naked]` row).
9. **`../stage-3/spec-snapshot.md` §1.13 / §1.14 /
   §1.15** -- D9 / D10 / D16 audit shapes; context for
   the per-family stages-15+ inheritance and for the
   c_void rejection enforcement.
10. **`../stage-4/spec-snapshot.md` §9.7 + §13.3** --
    `Const` enum variant list + `ConstMemoryStatic`
    family; composer `render_const` reference.
11. **`../stage-12/discussions.md` D-12-1.5 +
    `../stage-12/spec-snapshot.md` §13.1 / §13.3** --
    DomainAttr forward-pointer context.
12. **`../stage-13/discussions.md` D-13-4.6 +
    D-13-9** -- building-block-vs-composer
    architectural division (the composer trio is
    stage-14's owned territory); spec-split rationale.

## D-tag discussion agenda

Pre-staged D-tags from `../status.md` "Next action"
section; refined here with sub-question structure. The
D-tags are candidates surfaced by the
expectations.md inbound bundle + the external-review
findings; the rustc evidence pass in step 4 may surface
additional D-tags or merge candidates.

### Cross-cutting questions (resolve first)

These thread through D-14-1 / D-14-3 and may yield
their own D-tags after the evidence pass:

- **Operational-intrinsic vs declarative-property
  attribute architecture.** Is the operational-intrinsic
  table (Q7-recognized callees) an instance of the same
  DomainAttr sibling-table architecture as
  declarative-property attributes
  (`#[rseq::mmio(...)]` / `#[rseq::interrupt(...)]` /
  etc.), or two parallel architectures? Trade-off:
  - *Unified*: one sibling-table architecture with
    discrimination by family/key-kind; cleaner spec
    surface; potentially polymorphic payloads.
  - *Parallel*: two architectures (operational +
    declarative); type-safe per-family payloads; more
    tables on `RegseqModel`.
  The choice ripples through D-14-1 storage shape,
  D-14-3 unified-vs-per-family, and every phase-2
  stage's attribute spec. **Worth pinning before
  D-14-1 / D-14-3 lock.**

- **Stage-14 closure-spec preparation for impl sweep
  #1.** Does stage 14's spec carry any
  implementation-orientation prose (population timing,
  validator hook points, the bridge codepath sketch)?
  Or stay pure declarative spec, letting the impl
  sweep design the codepath from scratch? README §2's
  "spec / impl don't interleave" suggests the latter,
  but the impl sweep that follows stage 14 has no spec
  stage to land in, so any spec-shaped clarifications
  discovered during impl sweep would either
  retroactively amend stage 14 or land as informal
  notes. **Decide the boundary upfront.**

### D-14-1 -- Q7 intrinsic recognition mechanism + storage (load-bearing)

This is the largest cluster and gates D-14-2 + D-14-3
storage shape.

- **Recognition codepath.** Which rustc API does the
  bridge walk to find `#[rseq::intrinsic("NAME")]` on
  a callee? Candidates: `tcx.get_attrs(did, ...)`
  filtered by tool-namespace path; manual
  `hir::attrs::Attribute` walk; `rustc_resolve`
  tool-namespace machinery. Needs a concrete
  rustc-source evidence pass + Codex review on the
  choice.
- **`register_tool(rseq)` validation.** Does the
  bridge verify the crate-level declaration is
  present, or trust it? Diagnostic shape if missing.
- **NAME parsing.** `#[rseq::intrinsic("NAME")]`
  carries a string literal. How is it parsed (raw
  `Symbol` extraction vs full attribute-meta-list
  traversal)? What's the error shape for malformed
  payloads (missing argument, non-string literal,
  multi-argument)?
- **`IntrinsicKind` enum shape.** Closed enum with
  discriminants per intrinsic name. Append-only
  ordering (mirrors `regseq_instance_kind_rank` per
  stage-1 R5.1 / R5.2)? Variant per intrinsic
  (`Log_Debug`, `Log_Info`, ...) vs single variant
  with a category tag?
- **Storage shape.**
  `intrinsic_items: IndexVec<ItemId, Option<IntrinsicInfo>>`
  is the working candidate. Refinements: does
  `IntrinsicInfo` need `source_item: ItemId`
  (redundant with the IndexVec key)? Is
  `name: StringId` worth keeping (diagnostic-only) or
  is `kind` sufficient?
- **Population timing.** Where in stage-12's 5-phase
  bridge pipeline (D-12-5) does intrinsic-table
  population happen? Phase 2 (Translation+Synthesis)
  is the obvious slot -- attaching to the Item
  discovery walk.
- **Spec home.** New §X on `RegseqModel` root for
  "Operational intrinsic table"? Or fold into the
  existing `RegseqModel` struct in §3 with a
  forward-pointer to a new section for the schema
  details?

### D-14-2 -- Diagnostic-logging intrinsic vocabulary

Smaller surface; ergonomic implications for phase-1
testing.

- **Vocabulary entries.** Per-level intrinsics
  (`log_debug`, `log_info`, `log_warn`, `log_error`)
  vs single intrinsic with a level operand
  (`log(LogLevel, ...)`). Trade-off: per-level
  dispatches naturally on `InstanceId` (no operand
  inspection at codegen); single-intrinsic-with-level
  is fewer vocabulary entries.
- **Argument shape.** Just `&str`? Or string + small
  set of value-args (`i32`, `u32`, `usize`,
  pointer)? Format-string convention?
- **Macro vs direct call.** What does the DSL user
  write? `log!("msg {}", x)` (regseq-provided macro
  expanding to intrinsic call) is ergonomic;
  `regseq::log::info(...)` (direct method call) is
  bridge-tractable. The bridge sees only the
  post-expansion form; stage 14 must pin the
  post-expansion intrinsic shape. Macro shape is
  impl-sweep territory.
- **Backend lowering contract** (informational, since
  backend is impl-sweep work): generic-C lowers to
  `fprintf(stderr, ...)`; Linux backend lowers to
  `printk(KERN_INFO, ...)`. Stage 14 records the
  contract; backends implement during their
  respective impl passes.

### D-14-3 -- DomainAttr sibling-table architecture

Architectural; instantiated only once in stage 14
(the intrinsic table per D-14-1) but specifies the
pattern for stages 15-19+.

- **Key-kind set.** Stage-4 §1287-1299 mentions
  `ItemId` / `InstanceId` / `FieldPath` / `Span`.
  Which of these get concrete v1 schemas?
- **Storage shape per key kind.**
  `IndexVec<KeyId, Option<AttrData>>` works for
  `ItemId` / `InstanceId`;
  `HashMap<FieldPath, AttrData>` and
  `HashMap<Span, AttrData>` are the natural shapes
  for the variable-keyed ones. Stage 14 picks the
  trait/contract shape.
- **One unified table or per-family tables.**
  Reviewer's `DomainTables` sketch had
  `item_props`, `instance_props`, `field_props`,
  `span_props` -- one table per key kind, holding a
  polymorphic `AttrData`. Alternative: one table per
  (family x key kind) -- e.g., separate
  `intrinsic_items`, `mmio_field_attrs`,
  `interrupt_item_attrs`. Trade-off: unified-by-key
  is fewer tables but a polymorphic payload;
  per-family is more tables but type-safe payloads.
- **Relationship to `intrinsic_items` from D-14-1.**
  Resolved jointly with the cross-cutting
  operational-vs-declarative question above.
- **Population + validation contract.** When does
  the bridge populate? What does the validator
  check (referenced-key existence, no-duplicate
  entries, etc.)?

### D-14-4 -- `render_const(ConstId, TyId) -> String`

Per-arm specs for the closed 11-variant `Const` enum:
`Bool / Char / Int / Uint / Float / ZST / Bytes /
Aggregate / EnumValue / NullPtr / Reloc` (per
`spec-snapshot.md` §9.7 line 2828-2913, NOT the stale
`Str / Tuple / Memory / MemoryStatic / Zst` listing from
the pre-evidence-pass inventory -- `Str` / `[T]` are
handled via `Aggregate([Reloc, Uint])`+`ConstMemoryStatic`
synthesis per §9.7's "Reference-typed const handling"
rule, not as Const variants). Inherits D-13-1 master
style + composes `render_ty` (stage 13) + uses
`render_def_path(ItemId)` (stage 12) for the `Reloc`
arm.

The rustc evidence pass at `prior-art/rustc-const.md`
grounded 12 sub-decisions:

- **D-14-4.1 (load-bearing)** -- API signature.
  `render_const(ConstId, TyId) -> String` (pair form
  mirrors rustc's `pretty_print_const_value_tcx(tcx,
  ct, ty, fmt)`). The consumer's `TyId` is required for
  the `Bytes / Aggregate / ZST / NullPtr / Reloc` arms
  whose rendering shape depends on type, not const.
- **D-14-4.2** -- scalar variants (Bool / Char / Int /
  Uint / Float). Decimal value + rustc-style
  underscore-prefixed width suffix.
- **D-14-4.3** -- ZST per-arm; consumer-Ty-driven
  dispatch.
- **D-14-4.4** -- Aggregate per-arm dispatch on consumer
  `Ty.kind()` (Array / Tuple / Struct shapes).
- **D-14-4.5** -- EnumValue per-arm; variant-shape
  dispatch via `EnumShape`.
- **D-14-4.6** -- Bytes per-arm; consumer-Ty-driven
  dispatch (`&str` / `&[u8]` / `[u8; N]` / structural
  fallback).
- **D-14-4.7** -- NullPtr per-arm; rustc-mirrored
  typed-cast form `0x0 as <Ty>`.
- **D-14-4.8** -- Reloc per-arm; resolved-Item-kind
  dispatch (Function path-only; Static path + cast).
- **D-14-4.9** -- type-context threading for recursive
  children (caller resolves per-field types via parent
  Ty's layout and passes in).
- **D-14-4.10** -- sentinel collapsing for `T::MIN` /
  `T::MAX` (Int / Uint).
- **D-14-4.11** -- integer hex form admission
  (decimal-only recommended; existing `0xFFu8` specimen
  inconsistency).
- **D-14-4.12** -- float infinity / NaN representation
  (rustc-mirrored underscore-broken form).

**Specimen contract.** ~5-10 specimens per `D-13-7`
convention. Resolve the existing specimen inconsistency
at `pretty-printer-snapshot.md:1140 / 1152 / 1204 / 1212
/ 1226 / 1241 / 1287` (mixed underscore vs no-underscore
forms; mixed decimal vs hex) -- standardise on rustc's
underscore-decimal form during D-14-4.2 lock.

**Spec home.** `pretty-printer.md` §6 expansion or new
dedicated section (e.g., §6.5 or §10 -- placement chosen
during drafting). Per the §5.9 rule 6 spec split, the
producer-side `Const` enum stays in `spec.md` §9.7; the
consumer-side rendering surface lands in
`pretty-printer.md`.

**Phase A drafting plan** documented in
`status.md` "Next action" section. Phase B Codex review
expected ~3-5 rounds (comparable to D-14-2's 6 rounds
with similar multi-sub-decision shape; smaller per-arm
scope).

### D-14-5 -- `render_instance(InstanceId) -> String`

`InstanceKind` dispatch; composes `render_def_path`
(stage 12 D-12-6) + `render_ty` (stage 13 D-13-5) +
body-level renderers (stage 13 D-13-2 / D-13-6) +
`render_const` (stage 14 D-14-4) + the §5.3 per-flag
attribute helper. Spec lands at
`pretty-printer-snapshot.md` §10.2 (currently a
`(Pending D-14-5 lock.)` stub).

The rustc evidence pass at
`prior-art/rustc-instance.md` (Step 4, 2026-05-13)
grounded 8 sub-decisions across 10 Q-tags. Three
asymmetries with rustc's prior art load-bear on the
design:
- ADT variants (Struct/Enum/Union) have **no rustc
  precedent** -- rustc emits nominal types implicitly
  at use sites; RegseqModel emits them as C typedefs
  (Q7, D-14-5.5).
- Foreign declarations (`body: None`) must render as
  declaration form `[...]fn PATH(args) -> Ret;`;
  rustc's MIR pretty-printer skips them entirely
  (Q5, D-14-5.2).
- Static / Const variants surface pre-evaluated
  `ConstId`s via `render_const`; rustc renders them
  via value-constructing MIR bodies (Q6,
  D-14-5.3 / D-14-5.4).

Eight sub-decisions allocated:

- **D-14-5.1 (load-bearing)** -- API signature.
  Single-argument `render_instance(InstanceId) -> String`
  (no pair-form needed -- `InstanceKind` carries enough
  per-variant context). Gates the per-variant
  signatures.
- **D-14-5.2 (load-bearing)** -- Function variant
  envelope. Attribute block + signature decoration
  (`[unsafe ][extern "ABI" ]fn PATH(args) -> Ret`) +
  body-vs-declaration dispatch on `body: Option<Body>`.
  `flavor` / `virtual_idx` are identity metadata (not
  rendered).
- **D-14-5.3** -- Static variant. Form
  `[<attrs>]static [mut ]PATH: <render_ty(ty)>[ = <render_const(init.unwrap(), ty)>];`.
  Mutability from `ItemKind::Static { mutability }` per
  stage-12 D-12-1.2 (c). `init = None` -> drop the
  `= ...` segment (foreign extern static).
- **D-14-5.4** -- Const variant. Form
  `[<attrs>]const PATH: <render_ty(ty)> = <render_const(value, ty)>;`.
  `value` always populated per stage-12 D-12-2 (no
  None branch).
- **D-14-5.5 (load-bearing, no rustc precedent)** --
  ADT variants (Struct / Enum / Union). Two options:
  (A) header-only (§5.4 wrapper unmodified); (B) full
  definition body from §11 Layout. Option B is the
  evidence preponderance per Q7: matches §2.1 master
  Rust-source-style, surfaces layout consistently,
  supports field-level diagnostics. Sub-questions if B
  locked: `EnumShape` dispatch, per-variant ctor form
  (unit / tuple / named-field per §3.4), `#[repr(...)]`
  decoration (likely deferred), discriminant value
  rendering (likely deferred).
- **D-14-5.6** -- DomainAttr attribute weaving.
  Ordering rule for the attribute block (Item-flag
  stream vs `domain_attrs.item_attrs` stream);
  per-`AttrData` variant render rule (stage 14:
  `AttrData::Intrinsic(IntrinsicKind::Log(...))` ->
  `#[rseq::intrinsic("log-<sev>")]`).
- **D-14-5.7 (load-bearing)** -- `render_item` vs
  `render_instance` layering boundary. Choice:
  (a) composer-from-scratch (`render_instance` doesn't
  call `render_item`; factors §5.3 per-flag table into
  a shared helper); (b) compose via `render_item`
  wrapper (forces a §5.4 signature change for ABI
  context). Recommendation per Q3: (a).
- **D-14-5.8** -- Cross-stage friction watch. No
  anticipated REOPENs of stage-13 D-tags from current
  evidence. The §5.3 helper factor-out is localized
  reorganization. Monitor at Phase B; register here if
  drafting surfaces a REOPEN.

**Phase A drafting plan** (9 steps; per the audit-shape
in `discussions.md` D-14-5 entry): lock D-14-5.1 first
(gate), then D-14-5.2 (load-bearing Function variant),
then D-14-5.3 + D-14-5.4 (surgical Static / Const),
then D-14-5.5 (the architectural ADT choice), then
D-14-5.6 (DomainAttr weaving), then D-14-5.7 (layering
boundary), then D-14-5.8 final friction outcome, then
~10-15 specimens to `pretty-printer-snapshot.md` §9,
then Phase B Codex review (anticipated 2-4 rounds
comparable to D-14-4's 4-round trajectory).

**Specimen contract.** ~10-15 specimens per D-13-7
convention -- one per InstanceKind variant covering
the common case + edge cases (Function with intrinsic
attribute, Function foreign-declaration, Static
initialized vs foreign, ADTs with non-trivial layouts
including the `EnumShape::Empty` / `Single` /
`Multiple` cases). Placement: new §9.11 subsection in
`pretty-printer-snapshot.md`.

**Spec home.** `pretty-printer-snapshot.md` §10.2
(replacing the current stub). Cross-references to
`spec.md` §9.4 (InstanceKind structure) added per
§5.9 rule 5; if D-14-5.5 (B) locks, `spec.md` §9.4
gets a paragraph pointing to §10.2 for ADT body
rendering (mirrors §9.7 pointer added at D-14-4).

### D-14-6 -- Top-level `render(&self) -> String`

The third and final composer-renderer cluster per
stage-13 D-13-4.6. Phase A LOCKED 2026-05-13 with
all 6 sub-decisions; Phase B Codex review in
progress (R1 BLOCK -> R2 MINOR/Lockability:yes ->
R3 MINOR/Lockability:yes -> R4 MINOR/Lockability:yes
-> R5 close-out pending). See `discussions.md` D-14-6
entry for the full audit-shape per sub-decision +
`status.md` for the cluster's current state +
`review-log.md` for the Phase B round trail.

**Locked sub-decisions** (audit-shape detail in
`discussions.md`):

- **D-14-6.1** -- API signature
  `render(&self) -> String` (zero-argument; no
  `single` filter; no write-sink form).
- **D-14-6.2 (load-bearing)** -- iteration over
  `model.instances.iter_enumerated()` in canonical
  F4-2 InstanceId order per `spec.md` §16.3; no
  render-time re-sort (F4-2 already groups by
  `regseq_instance_kind_rank` per §9.3).
- **D-14-6.3** -- per-Instance separator: top-level
  composer emits `"\n\n"` between consecutive
  `render_instance(id)` outputs (which end at `}`
  or `;` with no trailing newline per the §9.11
  specimen contract); no leading or trailing
  separator. First-flag pattern adapted from
  rustc's `write_mir_pretty` at `pretty.rs:328-335`.
- **D-14-6.4** -- file-level envelope: no header,
  no trailer, no metadata wrapper. Output starts at
  first rendered byte of first `render_instance`
  output (Stream-1 + Stream-2 attribute block per
  §10.2.7 when non-empty, otherwise kind-keyword
  line); ends at last rendered byte (`}` or `;`)
  with no trailing newline injected.
- **D-14-6.5** -- empty-model case: empty `String`
  `""` identity (loop never runs, no separator
  emitted).
- **D-14-6.6** -- cross-stage friction outcome:
  zero new stage-13 D-tag REOPENs. The §9.11
  specimens already established the no-trailing-
  newline contract; D-14-5's signature unchanged.

**Spec contributions (landed).**
- `pretty-printer-snapshot.md` §10.3 (§10.3.1-
  §10.3.7 covering signature, iteration order,
  separator, envelope, empty case, composition
  outline, worked examples).
- `pretty-printer-snapshot.md` §1.3 composer
  listing + §8 ownership table row finalised.
- `pretty-printer-snapshot.md` §9.12 specimens
  129-133 (5 specimens covering empty model,
  single-Instance Function, multi-Instance mixed
  rank, multi-Instance same-rank-group with
  foreign + intrinsic-attributed, full per-rank
  spectrum).
- `prior-art/rustc-render.md` (Step 4 evidence
  pass; 6 Q-tags Q1-Q6).
- `rustc-references.md` Top-level render section
  (4 file:line landmarks).

**Step 4 rustc evidence landmarks** (consumed at
`prior-art/rustc-render.md`):
- `rustc_middle/src/mir/pretty.rs:317-369`
  `write_mir_pretty` -- canonical top-level entry;
  3-line WARNING header + first-flag-guarded
  blank-line separator + `dump_mir_def_ids` walk.
  RegseqModel diverges on the WARNING header
  (snapshot suite enforces stability, premise
  doesn't apply) and on the separator bytes
  (composer-owned `"\n\n"` because per-Instance
  outputs have no trailing newline).
- `rustc_mir_transform/src/lib.rs:328-359`
  `mir_keys` -- HIR-walk source-order; less
  deterministic than RegseqModel's F4-2.
- `rustc_middle/src/mir/mono.rs:508-574`
  `items_in_deterministic_order` -- codegen-tier
  per-CGU sort with two modes (symbol-name by
  default; span+symbol-name under
  `-Z codegen-source-order`); supporting evidence
  for D-14-6.2's "sort upstream, don't re-sort"
  posture.
- `rustc_public` has **no** top-level batch
  renderer; confirms RegseqModel's top-level
  composer is RegseqModel-new at the stable-IR
  tier (mirrors D-14-5's ADT-variant finding).

### D-14-7 -- `#[naked]` rejection (Finding #2)

**Cluster status**: D-14-7 Phase B LOCKED at R6
APPROVE / Lockability:yes / Close-out:YES
(2026-05-13). All 9 sub-decisions D-14-7.1 through
D-14-7.9 LOCKED at Phase A (2026-05-13). 6-round
Phase B trajectory closed 2026-05-13: R1 MINOR
(3 findings: §8.0 field-count + audit pre-lock
header + Q5 ALLOW_LIST) -> R2-R5 each MINOR (1
finding per round; recap-text-duplication cascade)
-> R6 APPROVE 0 findings. **Total: 7 findings
closed across R1-R5 (0B+7M+0N)**; Lockability:yes
sustained across R1-R6. The R5 cascade-breaking
fix (bumping the four lagging surfaces to the
full R5 trajectory at once) ended the cascade in
one pass; R6 confirmed APPROVE. **D-14-7 closes
the first surgical-fix cluster** of stage 14
(external-review Finding #2). See `discussions.md`
D-14-7 for the audit-shape detail per sub-decision
+ `review-log.md` D-14-7 cluster section for the
round-by-round trail + `status.md` for the
cluster's current state.

**Locked sub-decisions** (audit-shape detail in
`discussions.md`):

- **D-14-7.1 (load-bearing)** -- Reject `#[naked]`
  at Item translation in v1. Diagnostic names
  `#[naked]` explicitly and recommends the future
  phase-2 stage-17 `#[rseq::interrupt(...)]` domain
  extension as the deferral path for ISR-stub
  synthesis. Mirrors stage-9 D-9-9 row 8's
  diagnostic-shape pattern (name the construct +
  suggest the DSL alternative).
- **D-14-7.2** -- Recognition API:
  `tcx.codegen_fn_attrs(did).flags.contains(CodegenFnAttrFlags::NAKED)`
  during stage-12 D-12-5 Phase 2 (Translation+
  Synthesis) per-Item walk. Same source-of-truth
  bit rustc's own codegen consults at
  `rustc_codegen_ssa/src/mono_item.rs:46`.
- **D-14-7.3** -- Rejection-table placement at
  `spec-snapshot.md` §15.1 with new "Item
  translation" phase row + header note extended
  to four categories (Item-attribute DSL
  rejections joins non-rigid `TyKind` / rigid
  DSL-rejected / closed `PlaceElem` vocabulary
  rejections). Sibling row to the stage-9 D-9-9
  §23.6.1 terminator-translation row 8 for
  `TerminatorKind::InlineAsm`.
- **D-14-7.4 + D-14-7.5** -- `Item.is_naked: bool`
  field deletion at §8.0 + `instance_is_naked`
  helper deletion at §3.5.2. 14-helper inventory
  narrative downshifts to 13 helpers (12 stage-12
  D-12-1.8 helpers as revised + 1 stage-14
  D-14-1.6 domain-attribute helper).
- **D-14-7.6** -- Pretty-printer per-flag table
  edits: §5.3 per-flag table drops `is_naked =
  true` / `is_naked = false` rows; §5.6 row-order
  recap drops item 3 and renumbers items 4-6 to
  3-5; §5.1 atomic-shape example list drops
  `#[naked]`; §5.2 default-suppression narrative
  drops "not naked".
- **D-14-7.7** -- Specimen alignment at §9.9:
  Specimen 69 input drops `is_naked: false`;
  Specimen 70 input drops `is_naked: true` and
  output drops `#[naked]` rendered line +
  trailing parenthetical referencing D-14-9 /
  Finding #4 for the cross-finding
  `opaque_handle_eligible` cleanup (the
  parenthetical was hygiene only and has since
  been removed by D-14-9 closing the
  cross-finding); Specimen 75 input drops
  `is_naked: false`.
- **D-14-7.8** -- §3.6.6 paragraph 3 finalisation
  from pre-lock "D-14-7 territory" wording to
  Stage-14-D-14-7 locked-state wording
  acknowledging the §15.1 rejection row and that
  no `AttrData::Naked` variant exists.
- **D-14-7.9** -- Cross-stage friction outcome:
  three REOPENs at merger close:
  - **D-12-1.3 [REOPENED]** -- `is_naked` field
    removed.
  - **D-9-9 [REOPENED]** -- rejection vocabulary
    extended with §15.1 Item-translation row
    sibling to the §23.6.1 terminator-translation
    row 8 (InlineAsm). §23.6.1 9-row table stays
    at 9 rows; §15.1 table grows by one row.
  - **D-13-4.3 [REOPENED]** -- per-flag rendering
    table row removed; §5.6 row-order recap
    renumbered.
  D-12-1.8 stays LOCKED (helper-count update is
  downstream of D-12-1.3's field-list revision).

**Spec contributions (landed).**
- `spec-snapshot.md` §3.5.2 + §3.6.6 + §8.0 +
  §15.1 edits.
- `pretty-printer-snapshot.md` §5.1 + §5.2 + §5.3
  + §5.6 + §9.9 edits.
- `prior-art/rustc-naked.md` (Step 4 evidence
  pass; 6 Q-tags Q1-Q6).
- `rustc-references.md` "`#[naked]` handling"
  section (5 file:line landmarks).

**Step 4 rustc evidence landmarks** (consumed at
`prior-art/rustc-naked.md`):
- `rustc_attr_parsing/src/attributes/codegen_attrs.rs:221-333`
  `NakedParser` -- attribute parser; emits
  `AttributeKind::Naked(Span)`; allowed targets
  Fn + Method variants; ALLOW_LIST cross-attribute
  compatibility pass.
- `rustc_middle/src/middle/codegen_fn_attrs.rs:167`
  `CodegenFnAttrFlags::NAKED = 1 << 2` -- the
  source-of-truth flag bit folded from
  `AttributeKind::Naked` at
  `rustc_codegen_ssa/src/codegen_attrs.rs:82`.
- `rustc_middle/src/middle/codegen_fn_attrs.rs:14-28`
  `codegen_instance_attrs` -- strips NAKED for
  non-Item `InstanceKind`s; confirms strictly
  source-Item-level recognition.
- `rustc_codegen_ssa/src/mir/naked_asm.rs:30-41`
  `codegen_naked_asm` -- asserts the body's
  START_BLOCK terminator must be
  `TerminatorKind::InlineAsm`; structural overlap
  with stage-9 D-9-9 row 8.
- `rustc_codegen_ssa/src/mono_item.rs:44-51` +
  `:74-82` -- naked Fns dispatch to global asm
  emission; predefine pass skips them.

### D-14-8 -- `emission_name` forward-pointer retarget (Finding #3)

**Cluster status**: D-14-8 Phase B LOCKED 2026-05-13
at R2 APPROVE / Lockability:yes / Close-out:YES. All
Phase A content unchanged through Phase B; 2-round
trajectory R1 MINOR (2 findings: tracker line-number
wording + placeholder D-9-? in audit) -> R2 APPROVE
0 findings. Pure spec hygiene; no sub-decisions; no
D-tag flip. Single applied edit at `spec-snapshot.md`
§9.2 paragraph 3 (the `Struct / Enum / Union`
`emission_name` bridge-construction-rule bullet,
closing clause). Replaces the formerly stale
forward pointer to "stage 13 (codegen)" (stage 13
closed as the pretty-printer spec per D-13-9, not
codegen) with phase-1 determinism + uniqueness
commitment + forward pointer to the C-backend spec
(per README §5.9 rule 7 dual-backend principle).
See `discussions.md` D-14-8 entry for the audit-
shape detail + reasoning + worked example +
`review-log.md` D-14-8 cluster section for the R1-
R2 trail.

### D-14-9 -- Specimen 70 split (Finding #4)

**Cluster status**: D-14-9 Phase B LOCKED 2026-05-13
at R2 APPROVE / Lockability:yes / Close-out:YES. All
Phase A content unchanged through Phase B; 2-round
trajectory R1 MINOR (2 findings: stale "three
clusters" preamble wording + audit overstatement of
how Specimen 70b differs from Specimen 73) -> R2
APPROVE 0 findings. Pure pretty-printer-
spec hygiene; no sub-decisions; no D-tag flip on
D-13-7. Two applied edits at `pretty-printer-
snapshot.md` §9.9: (i) Specimen 70 input downshifts
`opaque_handle_eligible: true` to `false` and the
output drops the `#[opaque_handle]` line, leaving a
clean Function attribute-stress specimen (no_mangle
+ inline(always) + link_section + unsafe); (ii) new
Specimen 70b carrying
`kind: ItemKind::Struct, def_path: <pl011::Handle>,
opaque_handle_eligible: true` renders
`#[opaque_handle]` above `struct pl011::Handle`,
preserving coverage of the bridge-outcome
rendering on the only `ItemKind` that can carry
the flag per `spec-snapshot.md` §8.0 invariant 19.
The D-14-7 cross-finding parenthetical that
forward-pointed to D-14-9 is also removed. See
`discussions.md` D-14-9 entry for the audit-shape
detail + reasoning + worked example +
`review-log.md` D-14-9 cluster section for the
R1-R2 trail.

### `core::ffi::c_void` rejection enforcement

**Cluster status**: Phase B LOCKED 2026-05-13 at R3
APPROVE / Lockability:yes / Close-out:YES. All Phase
A content unchanged through Phase B; 3-round
trajectory R1 MINOR (3 findings: Coroutine family
double-classification + stale line numbers +
dangling "Phase B." fragment) -> R2 MINOR (1
finding: missed memory.md stale line number) -> R3
APPROVE 0 findings. No new D-tag; stage-3
D16 already specifies the rejection at the
source-of-truth layer; stage-3 D1.4 recommends the
opaque-struct pattern. Phase A drafting verified
that the §15.1 c_void row already exists in the
rejection table (`core::ffi::c_void` | type
translation | recognised via
`tcx.is_lang_item(LangItem::CVoid)`; opaque-struct
pattern recommended), the §15.0
dispatch-ordering rule already gates the rejection
algorithmically (the special-ADT check runs
unconditionally before generic struct/enum/union
translation), and Appendix A.1 already walks the
`*mut c_void` -> `*mut OpaqueCtx` rewrite end-to-
end. One applied edit: a new "**Enforcement model**"
paragraph + an "**Opaque-struct pattern**"
paragraph added to the §15.1 introduction (after
the four-category narrative paragraph, before the
rejection table) explicitly naming which §15.1
rows the §15.0 gate enforces and defining the
opaque-struct pattern inline (`#[repr(C)]` ZST
struct with a single private zero-length-array
field; admitted via the ordinary `Ty::Struct`
path; lowers to incomplete-struct pointer at the
C-ABI boundary per §14.5 + §8.0 invariant 19). The
new paragraphs forward-reference Appendix A.1's
end-to-end rewrite. No D-tag allocated; no D-tag
flip. See `discussions.md` cluster entry for the
audit-shape detail + `review-log.md` cluster
section for the R1-R3 trail.

## Workflow notes for this stage

- **Working artifacts.** Stage 14 produces edits to
  both `spec-snapshot.md` (structural spec --
  `IntrinsicKind` enum + storage schema, c_void
  rejection enforcement, Findings #2 / #3) and
  `pretty-printer-snapshot.md` (composer renderer
  specs, Specimen 70 fix, per-flag table edit). Per
  D-13-9, both files are co-equal merger artifacts;
  both integrate into top-level
  `spec.md` + `pretty-printer.md` at merger close.

- **No parallel sessions planned.** Stage 14's
  D-tag clusters are not independent enough to
  parallelize: D-14-1 / D-14-3 share the cross-cutting
  architecture question; D-14-4 / D-14-5 / D-14-6 are
  sequential (each composer depends on the previous);
  Findings #2-#4 are small enough to land alongside
  the main decisions. Solo work; no `sessions/`
  subfolder.

- **Codex review cadence.** Per-stage per-D-tag-cluster
  review as we close each decision's spec
  contribution. Post-merge tier-3 Codex review per
  README §5.5 step 6 against both `spec.md` and
  `pretty-printer.md` (per D-13-9 / §5.5 step 6
  amendment).

- **Implementation sweep #1 follows stage close.**
  Stage 14's spec MUST close before impl sweep
  starts. Per README §2 amendment, the sweep covers
  stages 1-14 inclusive against the locked spec.
  Spec-shaped clarifications surfaced during the
  impl sweep can amend stage 14 (per the
  "Stage-14 closure-spec preparation" cross-cutting
  question above; this is one of the things stage 14
  must decide upfront).

- **Outbound expectations.** Stage 14 may surface
  concerns for phase-2 stages 15-18+. File those in
  the appropriate `../stage-15/expectations.md`
  through `../stage-18/expectations.md` (already
  exist with their inbound bundles) -- not in
  `stage-14/expectations.md`.

- **Decision tag form.** Stage 14 uses the prefixed
  `D-{stage}-{decision}` form per README §5.2.
  Start at `D-14-1`. Sub-decisions use dotted form
  (`D-14-1.1`, `D-14-1.2`, ...).

## Cross-stage interactions

Stages this stage depends on heavily (read in step 5 +
step 7 above):

- **Stage 12 (Items + DefPath).** Item schema
  (`spec.md` §8), `render_def_path` stub fill (D-12-6),
  DomainAttr forward-pointer (D-12-1.5), canonical
  5-phase bridge pipeline (D-12-5). The intrinsic-
  table and DomainAttr sibling-tables attach to the
  `RegseqModel` schema commits stage 12 finalized;
  population fits into stage-12's Phase 2 slot.

- **Stage 13 (Pretty-printing).** Master rendering
  style (D-13-1), per-arm specs for `render_ty`
  (D-13-5) and body-level renderers (D-13-6),
  specimen conventions (D-13-7), spec-split
  convention (D-13-9), composer-vs-building-block
  division (D-13-4.6). Stage 14's composer trio is
  the natural completion of stage-13's rendering
  surface.

- **Stage 4 (Constants).** 11-variant `Const` enum
  (D-4-1) + `ConstId` table; the `render_const`
  input. `Reloc(InstanceRef)` arm composes
  `render_def_path` (stage 12) + `render_instance`
  (this stage).

- **Stage 1 (Foundation).** Pretty-printer scaffold,
  `IndexVec` patterns, `newtype_index!` macro
  (`intrinsic_items` storage uses the
  ItemId-keyed `IndexVec` pattern). Q6 facade
  promotion of diagnostic-logging primitives
  starts here (the first wave of facade content).

- **Stage 3 (Type system).** D9 / D10 / D16
  rejection commitments. Stage 14 enforces c_void
  rejection (D16); MMIO / atomic rejection
  enforcement relocates to stages 16 / 18 per the
  expectations split.

Stages this stage **does NOT** depend on heavily
(read on demand during work):

- Stages 5-11 (MIR vocabulary). Composer
  `render_instance` consumes body-level renderers
  from stage 13 (which cover these); stage-14
  spec doesn't touch the MIR vocabulary directly.

- Stage 2 (Serialization format). Stable-but-secondary
  per Q5; stage-14's new tables (intrinsic, DomainAttr)
  get serde derives per the existing convention;
  no schema-level work beyond derive placement.

## When stage 14 closes

Standard merger contract per README §5.5 (7-step
protocol). Stage-14 specifics:

- Step 1-2: append `discussions.md` + `review-log.md`
  to top-level under `## Stage 14` headers.
- Step 3: register each finalized decision in
  `decisions.md` under `## Stage 14`.
- Step 4: flip the five-decision REOPEN queue to
  `[REOPENED]`: `D-9-10` (per D-14-2.5), `D-13-6`
  (per D-14-4.9), `D-12-1.3` + `D-9-9` + `D-13-4.3`
  (all three per D-14-7.9 / Finding #2).
- Step 5: integrate `spec-snapshot.md` edits into
  top-level `spec.md` per §5.9 rules 1-5 (in-place
  for refinements; new sections for additions like
  IntrinsicKind / DomainAttr). Integrate
  `pretty-printer-snapshot.md` edits into top-level
  `pretty-printer.md` per §5.9 rule 6 (composer
  renderer per-arm specs land in a new section or
  in §6 / §7 expansion; Specimen 70 / 70b edits at
  §9; per-flag table edit at D-13-4.3 surface).
- Step 6: post-merge tier-3 Codex review on BOTH
  `spec.md` AND `pretty-printer.md` per §5.5 step 6
  amendment.
- Step 7: update `../status.md` to mark stage 14
  closed; announce phase-1 implementation sweep #1
  as the next major project step; freeze
  `stage-14/spec-snapshot.md` + `stage-14/pretty-
  printer-snapshot.md` with "Snapshot at close of
  stage 14" headers.
