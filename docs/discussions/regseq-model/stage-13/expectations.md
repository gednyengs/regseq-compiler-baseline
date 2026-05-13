# Expectations on stage 13 (incoming)

Stage 13 covers type-system pretty-printing: filling
`pretty.rs::render_ty` and any related rendering concerns
required to pretty-print the `Ty` enum once all its content
sources have been filled (FnSig from stage 11; Item / DefPath
from stage 12). Stage 13's rendering-style decision for `Ty`
also serves as the input for stage 14's `render_const`
(deferred from stage 4 per D-4-K because `render_const`'s
dependency chain is longer than `render_ty`'s — see
`../stage-14/expectations.md` "From stage 4").

The convention is described in `../README.md` under "Forward
expectations live in `stage-N/expectations.md`."

## From stage 3

- [ ] **Fill `pretty.rs::render_ty` stub.** The stub at
  `crates/regseq-model/src/pretty.rs` panics with
  `unimplemented!("stage 3 fills render_ty")` (the original
  scaffold message; if updated as part of this stage's work,
  rename to "stage 13 fills render_ty"). Stage 3 originally
  reserved this slot as an open decision (informally "D14 --
  renderer style") but deferred it: the renderer's input depends
  on `Ty` content references that only become fully populated
  by stage 11 (`FnSig`) and stage 12 (`Item` / DefPath). Locking
  the renderer style at stage 3 would have produced a spec that
  changes shape every time later stages fill in content.

  *Stage-3 lean (preserved as input, not binding).* Rust-source-
  style notation: `&'r T`, `*const T`, `[T; N]`, `dyn Trait`,
  `for<'a> fn(...)`, etc. Charon precedent. See
  `../stage-3/discussions.md` (the historical D14 entry,
  preserved as deferred-decision context). The lean was
  motivated by stage 3's wish to mirror what humans already read
  in rustc's `--emit=mir` output and by the precedent set by
  Charon. Stage 13 is free to validate this lean against the
  fully-populated input domain, or to choose differently
  (constructor-style, C-source-style, or other) -- the call
  belongs to stage 13.

  *Stage-13 deliverables.*

  1. Make the renderer-style call. Pin the chosen style as a
     stage-13 decision (mint a fresh `D-13-X` tag).
  2. Specify the rendering for each variant of the closed
     19-variant `Ty` enum (see `../stage-3/spec-snapshot.md`
     Section 1.1 for the variant list). Cover ADT-shaped
     variants (which call into `render_def_path`, owned by
     stage 12), `Ty::FnPtr` (which expands its `FnSig`, owned
     by stage 11), `Ty::Dynamic` (auto-traits + vtable
     reference), `Ty::Tuple` (synthetic field names), and the
     unsized markers (`Str`, `Slice`, `Never`).
  3. Implement the renderer. Per stage-11 D-11-K's
     categorical rendering deferral, the body-level
     renderers (`render_place`, `render_operand`,
     `render_rvalue`, `render_statement`, `render_terminator`
     + 3 sub-renderers, `render_basic_block`) and the
     `_N` (LocalId) + `bbN` (BlockId) sigil rendering
     wirings are all stage-13-owned alongside `render_ty`.
     Coordinate among the body-level renderer cluster,
     stage 12's `render_def_path`, and stage 14's
     `render_instance` -- the renderers should produce
     coherent output (consistent quoting, sigils,
     verbosity). See `../stage-11/discussions.md` D-11-K
     for the categorical-retargeting audit.
  4. Update insta snapshots accordingly.

  *Cross-refs.*
  - `../stage-3/spec-snapshot.md` Section 1.1 -- the variant
    table the renderer must handle.
  - `../stage-3/discussions.md` -- historical D14 entry
    (deferred), preserves the Rust-source-style lean and the
    options that were considered (constructor style, C-source
    style).
  - `../stage-3/prior-art/charon.md` -- Theme G (Charon's
    renderer style, the prior-art evidence behind the
    Rust-source-style lean).
  - `../stage-1/spec-snapshot.md` -- pretty-printer scaffold
    interface and the existing `render_ty` stub.
  - `../stage-11/expectations.md` -- coordination on
    `render_local` style and `FnSig` content.
  - `../stage-12/expectations.md` -- coordination on
    `render_def_path` style and `Item` content.
  - `../stage-14/expectations.md` -- coordination on
    `render_instance` style.

## Bridge concerns

Stage 13 has no bridge-side work: pretty-printing is consumer-
side (codegen, snapshot tests, diagnostics) and reads the
already-built `RegseqModel`. Bridge-side type construction is
fully owned by stage 3 (Ty variants), stage 11 (FnSig
content), and stage 12 (Item content + DefPath rendering).

## Open items

(none yet -- created when stage 13 opens)

---

## From stage 11 (per D-11-K)

Filed at stage-11 merger contract close (2026-05-05; per
README §5.5 step 7). Categorical rendering bundle per
D-11-K's master decision retargeting all body-level
rendering ownership from stage 11 to stage 13. Predecessor-
stage attribution preserved per D-11-K.6.

Stage 13 owns ALL of the following as a coherent
rendering-decision surface (alongside the pre-existing
`render_ty` from D14 deferred from stage 3):

### Public renderer rows (9 items)

- [ ] **`render_place(&Place, &Body)`** -- originally
  D-5-K (stage 5; deferred to stage 11; retargeted to
  stage 13 per D-11-K). Per-variant dependency analysis:
  see top-level `discussions.md` `## Stage 5` D-5-K
  audit shape. Field/variant name resolution dispatches
  per predecessor type's layout (`LayoutKind::Struct.fields[idx].name`,
  `LayoutKind::Union.fields[idx].name`, tuple
  `fields[idx].name`, `EnumShape`-active-variant
  `VariantLayout.fields[idx].name`).

- [ ] **`render_operand(&Operand, &Body)`** -- originally
  D-6-K. Per-variant dependency: `Copy(p)` / `Move(p)`
  delegate to `render_place` + `copy ` / `move ` keyword
  prefix; `Constant(id)` delegates to `render_const` (stage
  14 per D-4-K).

- [ ] **`render_rvalue(&Rvalue, &Body)`** -- originally
  D-7-K. Per-variant dependency: routes through
  `render_operand` / `render_place` / `render_ty` (Cast
  arm; co-located at stage 13). Aggregate field/variant
  names resolve from `AggregateKind`'s per-variant `TyId`
  via §12 type interner.

- [ ] **`render_statement(&Statement, &Body)`** --
  originally D-8-K. Per-variant dependency: `Assign(Place,
  Rvalue)` delegates; `SetDiscriminant(Place, VariantIdx)`
  reads variant name from `place_ty(p)`'s
  `LayoutKind::Enum.shape` per §11.1; `Intrinsic(...)`
  delegates to `render_operand`.

- [ ] **`render_terminator(&Terminator, &Body)`** --
  originally D-9-K. Public top-level entry; dispatches
  per-variant on `TerminatorKind`. The `Direct(InstanceId)`
  arm late-binds to `render_def_path` (stage 12).

- [ ] **`render_call_callee(&CallCallee, &Body)`** --
  originally D-9-K. Two-arm dispatch on `Direct(InstanceId)`
  / `Indirect(Operand)`.

- [ ] **`render_switch_targets(&SwitchTargets, &Body)`** --
  originally D-9-K. Explicit-split storage shape
  (`cases: Box<[(u128, BlockId)]>` + `otherwise: BlockId`).

- [ ] **`render_assert_kind(&AssertKind, &Body)`** --
  originally D-9-K. 8-variant dispatch.

- [ ] **`render_basic_block(&BasicBlock, &Body)`** --
  originally D-10-K (D-10-K.1). Envelope:
  `bbN: { stmt_0; ...; stmt_n; terminator; }`.

### Internal helper (1 item)

- [ ] **`render_terminator_kind(&TerminatorKind, &Body)`**
  -- originally D-9-K. Per-variant dispatch helper invoked
  by `render_terminator`. Not part of the §18.2 public
  surface.

### Sigil rendering wirings (2 items)

- [ ] **`_N` sigil for `LocalId` rendering** -- originally
  stage-1 inbound (`_0` for `RETURN_PLACE`, `_1`, `_2`, ...;
  textual convention pinned at stage-11 §25.3 +
  top-level `spec.md` §18.1 sigil table). Stage 13 wires
  the rendering site alongside its first consumer.

- [ ] **`bbN` sigil for `BlockId` rendering** -- originally
  D-10-K.2 (textual convention pinned at stage-10 §24.3.4 +
  top-level `spec.md` §18.1 sigil table). Stage 13 wires
  the rendering site alongside its first consumer.

### §18.2 per-method ownership table consolidation

Stage 13 owns all rendering ownership rows in top-level
`spec.md` §18.2 including:
- The pre-existing `render_ty` row (D14 deferred from
  stage 3).
- The 9 public body-content renderer rows (per the bundle
  above).
- The 1 internal helper row (`render_terminator_kind`).
- The 2 sigil rendering wirings (often expressed as
  helper-method commentary rather than dedicated table
  rows).

Stage-11 merger contract retargeted §18.2 rows in place
(rows remain with new `stage 13 (per D-11-K)` ownership;
not removed). Stage 13 may refine the table form when it
lands.

### Rendering-style coherence requirement (D-11-K rationale)

The 12-item bundle plus pre-existing `render_ty` form a
coherent rendering-decision surface. Stage 13 picks the
rendering style (sigil conventions, indentation, span-
rendering mode, ID-resolution depth) ONCE for the whole
body-level + type-level surface rather than per-renderer
piecemeal. This is D-11-K's load-bearing motivation.

### Cross-refs

- Top-level `spec.md` §18.2 (per-method ownership table;
  body-level renderer rows now stage-13-owned).
- Top-level `spec.md` §19.8, §20.6, §21.8, §22.8, §23.9,
  §24.9 (per-stage renderer deferral notes flipped to
  stage 13 per D-11-K).
- Top-level `spec.md` §25.3 (`_N` sigil pinned), §24.3.4
  (`bbN` sigil pinned).
- Top-level `discussions.md` `## Stage 11 (MIR bodies)`
  D-11-K audit shape; `## Stage 5` / `## Stage 6` /
  `## Stage 7` / `## Stage 8` / `## Stage 9` / `## Stage 10`
  D-X-K predecessor audit shapes (per-variant dependency
  analyses preserved as inheritance for stage 13).
- Top-level `decisions.md` `### Stage 11 (MIR bodies)`
  D-11-K entry; predecessor `### Stage N` D-X-K entries
  flipped to `[REOPENED]` with annotation
  `(retargeted to stage 13 per D-11-K)`.

---

## From stage 12 (per D-12-K)

Filed at stage-12 merger contract close (2026-05-06; per
README §5.5 step 7). Categorical Item-level rendering
deferral bundle per D-12-K's master decision. Purely
additive on top of the D-11-K cascade -- D-11-K covered
all body-level rendering; D-12-K covers Item-level
rendering, which D-11-K did not address. No predecessor
K-tag retargeting (per D-12-K.2).

Stage 13 owns the following Item-level rendering items
as part of the same coherent rendering-decision surface
established by D-11-K (alongside the pre-existing
`render_ty` from D14 and the 12-item D-11-K bundle).
`render_def_path(ItemId)` is the only non-deferred
Item-level renderer (locked at stage 12 per D-12-6 with
a one-line cache-lookup body against
`Item.def_path.rendered`).

### Item-level renderer rows (7 items)

- [ ] **`render_item(ItemId)`** -- full-Item display
  renderer. Composes DefPath (via stage-12-owned
  `render_def_path`) + ItemKind (via #2 below) +
  Item-level metadata (linkage, inline_hint, is_unsafe,
  is_naked, link_section, lang-item flags,
  opaque_handle_eligible). Per-variant dependency: see
  top-level `discussions.md` `## Stage 12` D-12-K audit
  shape and the D-12-1.3 typed-fields list.

- [ ] **ItemKind variant rendering** -- per-variant
  formatting hooks for the closed 6-variant flat
  `ItemKind` (`Function`, `Static { mutability }`,
  `Const`, `Struct`, `Enum`, `Union`). Includes
  `Static.mutability` decoration per D-12-1.2 (c).
  ItemKind variants are unit except `Static`; Function /
  Const / Struct / Enum / Union per-mono content
  (fn_sig, body, layouts, etc.) lives on `InstanceKind`
  per the Option B reframe (D-12-1.4) and is reached
  through Instance-side renderers, not ItemKind-side.

- [ ] **`Linkage` enum rendering** -- closed 4-variant
  `{ Default, NoMangle, Foreign, Weak }` per D-12-2.3.
  Style decision: omit `Default` from rendered output
  (default-suppression), inline the rest as
  `[linkage = "no_mangle"]` style annotations or similar.
  Stage 13 picks the exact textual form.

- [ ] **`InlineHint` enum rendering** -- closed 4-variant
  `{ None, Hint, Always, Never }` per D-12-1.3. Same
  default-suppression pattern as `Linkage`: omit `None`;
  render the rest as `#[inline]` / `#[inline(always)]` /
  `#[inline(never)]` style annotations or alternative.

- [ ] **Function-flag + lang-item-flag rendering**
  (combined entry per D-12-K.1 #5) -- the 5 boolean
  flags split into two source surfaces:
  - **Function flags**: `is_unsafe`, `is_naked` (per
    D-12-1.3). Style decision: render as `unsafe` /
    `#[naked]` keyword/annotation prefixes on Function
    Item display.
  - **Lang-item flags**: `is_phantom_data`,
    `is_unsafe_cell`, `is_manually_drop` (per §8.2 +
    D-12-3.1). Style decision: render as
    `[#[lang = "phantom_data"]]`-style annotations or
    a more compact form. Stage 13 picks; default-
    suppression for "all flags false" (the common case
    for non-lang-item ADTs).

- [ ] **`link_section: Option<StringId>` rendering
  decorator** -- per D-12-1.3. Style decision:
  `Some(s)` renders as `#[link_section = "..."]`
  attribute prefix; `None` suppressed. Stage 13 wires
  the rendering site at first consumer.

- [ ] **`opaque_handle_eligible` flag rendering
  decorator** -- bridge outcome flag (whether an Item is
  eligible for the opaque-handle codegen pattern).
  Default-suppression for `false`; `true` renders as a
  small annotation (`[opaque_handle]` or similar). Stage
  13 picks the exact textual form.

### §18.2 per-method ownership table consolidation

Per D-12-K.3, §18.2 gets new rows for the 7-entry
deferral bundle, each marked "deferred to stage 13 per
D-12-K". The existing `render_def_path` row was already
flipped at stage-12 merger close to "stage 12; D-12-6"
per D-12-6.3.

Stage 13 picks rendering style for the Item-level
surface (default-suppression rules, attribute-syntax
choice, multi-line vs one-line Item display) ONCE for
all 7 items, in coherence with the D-11-K body-level
surface and the D14 `render_ty` style. This is the
load-bearing rationale for the categorical deferral.

### Cross-refs

- Top-level `spec.md` §3 + §3.4 (Item population in
  bridge construction-pipeline order); §8 + §8.0 + §8.2
  (Item schema, Item-level metadata, lang-item flag
  set); §9.4 (`InstanceKind::Function` / `Static` post-
  Option-B); §13.4 (synthetic Item DefPathHash); §18.2
  (per-method ownership table).
- Top-level `discussions.md` `## Stage 12 (Rust Items)`
  D-12-K audit shape (`### D-12-K`,
  `#### D-12-K.1` deferral list,
  `#### D-12-K.2` outbound-bundle commit,
  `#### D-12-K.3` §18.2 ownership-table updates).
- Top-level `decisions.md` `### Stage 12 (Rust Items)`
  D-12-K entry (`[LOCKED]`).
- Stage-12 stage-internal: `../stage-12/discussions.md`
  §1 D-12-K audit shape and §4 outbound bundle commit;
  `../stage-12/status.md` §9 outbound expectations
  filing checklist.
