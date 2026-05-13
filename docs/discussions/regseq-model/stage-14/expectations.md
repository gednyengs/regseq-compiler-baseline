# Expectations on stage 14 (incoming)

Stage 14 covers **phase-1 foundation extensions**: the Q7
intrinsic-recognition mechanism (with diagnostic-logging as
the sole initial vocabulary), the DomainAttr sibling-table
architecture on `RegseqModel`, three composer renderers
(`render_const`, `render_instance`, top-level `render`),
plus the foundation-level bridge rejection rules and the
four external-review findings (2026-05-07).

The convention is described in `../README.md` under "Forward
expectations live in `stage-N/expectations.md`."

**Strategy revision note (2026-05-12).** The original "Stage
14: Domain-specific extensions" plan was monolithic and
covered the entire per-domain-family surface (MMIO, atomics,
interrupts, clock / reset, ...). Per README §1.3 + §2's
"Domain extensions iterate per stage in phase 2" rule, the
per-domain-family work is relocated to phase-2 stages
15, 16, 17, 18, .... This `expectations.md` retains only the
**foundation** items; per-family items live at:

- `../stage-15/expectations.md` -- regseq-trait scaffolding
  + `Lifecycle` + driver-struct discovery + DT-binding
  metadata + `RegseqError`.
- `../stage-16/expectations.md` -- MMIO family (vocabulary,
  `Mmio<T>` resource, `#[rseq::mmio(...)]` attribute family,
  `core::ptr::{read,write}_volatile` rejection enforcement,
  `RegisterId` typed-index).
- `../stage-17/expectations.md` -- interrupt / ISR family
  (`IsrId` typed-index, `#[rseq::interrupt(...)]` attribute
  family, ISR subkind of `InstanceKind::Function`).
- `../stage-18/expectations.md` -- atomic family (vocabulary
  + ordering encoding choice, `core::sync::atomic` rejection
  enforcement, memory-barriers / fences policy).

## From stage 1

- **Pin the intrinsic vocabulary (Q7) -- mechanism + initial
  vocabulary.** Per Q7, the bridge identifies domain
  intrinsics by the `NAME` argument of
  `#[rseq::intrinsic("NAME")]` on the callee, not by the
  callee's path. Names form a closed vocabulary maintained
  in the bridge; unknown names are rejected. Stage 14 ships
  the **mechanism** (recognition path + closed-vocabulary
  type + resolved-intrinsic storage on `RegseqModel`) plus
  the **diagnostic-logging vocabulary entries** (initial
  closed set; e.g., `log_debug`, `log_info`, `log_warn`,
  `log_error` -- exact names + arity TBD via Codex review).
  Per-domain-family vocabulary entries (`mmio_*`,
  `atomic_*`, ...) are filed in their respective phase-2
  stage's `expectations.md`.

  Recognition mechanism is corrected per external-review
  Finding #1: the bridge walks callee attributes via
  rustc's tool-namespace machinery rooted by the
  crate-level `#![register_tool(rseq)]` declaration; NOT
  `tcx.intrinsic_raw(did)` (which targets rustc-internal
  `#[rustc_intrinsic]` items, not custom tool-namespace
  attributes).

- **Single-level IR (Q3).** Domain extensions live as
  additional variants or tags on existing IR variants --
  not as a separate IR layer. For example, an ISR subkind
  of `Function` can be added to `InstanceKind::Function`
  (e.g., as an `is_isr: bool` field, or as a new
  `InstanceKind::Isr` variant). Stage 1's closed
  six-variant `InstanceKind` is the closed baseline; any
  domain-family extension appends to it (with a
  corresponding append-only entry in
  `regseq_instance_kind_rank`).

  **`Ty` is closed at stage 3.** Per
  `../stage-3/spec-snapshot.md` Section 1.1's "Forward
  reference: `Ty` enum closure (Q3)", the 19-variant `Ty`
  enum is closed -- stage 14 (and any later domain-family
  stage) may **not** add new `Ty` variants. Domain
  extensions land on `InstanceKind` (per the bullet above),
  on `Item` flags (mirroring D1.5's lang-item flags
  `is_phantom_data` / `is_unsafe_cell` /
  `is_manually_drop`), or on tags attached to existing IR
  nodes -- never as new type shapes. The set of type shapes
  RegseqModel can represent is fixed at stage 3.

- **Fill `pretty.rs::render_instance` stub.** Stage-1 stub
  at `crates/regseq-model/src/pretty.rs` panics with
  `unimplemented!("stage 14 fills render_instance")`. Stage
  14 replaces the body with a renderer producing
  `Item::<args>` per the spec sigil for `InstanceId`.

## From stage 4

- [ ] **Fill `pretty.rs::render_const` stub.** The stub at
  `crates/regseq-model/src/pretty.rs` panics with
  `unimplemented!("stage 14 fills render_const (deferred per
  D-4-K)")`. Stage 4 specified the closed 11-variant `Const`
  enum (D-4-1) + per-variant byte-encoding (D-4-2) +
  reference-typed handling via
  `SyntheticItemKind::ConstMemoryStatic` (D-4-3), but
  deferred the renderer to stage 14 per D-4-K (extending
  the D14 -> stage-13 precedent for `render_ty` one stage
  further because `render_const`'s dependency chain is
  longer). Stage 14 is the latest of `render_const`'s
  dependencies; landing the renderer there ensures all
  transitive renderers are filled.

  *Variant-by-variant dependency.*

  | Variant                              | Renderer needs                                                                                                  |
  | ------------------------------------ | --------------------------------------------------------------------------------------------------------------- |
  | `Bool` / `Char`                      | self-contained (variant + payload renders directly)                                                             |
  | `Int` / `Uint` / `Float`             | self-contained (kind discriminant + value-bits)                                                                 |
  | `ZST`                                | consumer's `Ty` to render which ZST it is (`()` vs `PhantomData<u32>` etc.)                                     |
  | `Bytes(_)`                           | consumer's `Ty` (bytes-as-struct vs bytes-as-array; field labels)                                               |
  | `Aggregate(_)`                       | consumer's `Ty` (tuple field labels vs struct field names vs array)                                             |
  | `EnumValue { variant_idx, payload }` | consumer's `Ty::Enum` to resolve `variant_idx` to a variant name                                                |
  | `NullPtr(TyId)`                      | `render_ty` (stage 13) for the pointee type                                                                     |
  | `Reloc(InstanceRef)`                 | `render_def_path(ItemId)` (stage 12) + `render_instance` (stage 14, this stage) for the target Instance's identity |

  Counts: 5 of 11 variants are self-contained scalars
  (`Bool` / `Char` / `Int` / `Uint` / `Float`); 5 of 11
  depend on `render_ty` (stage 13); 1 of 11 (`Reloc`)
  additionally depends on `render_def_path(ItemId)` (stage
  12) and `render_instance` (stage 14, filled in this
  stage).

  *Stage-4 lean (preserved as input, not binding).* No
  specific rendering style was endorsed at stage 4 -- D-4-K
  explicitly declined to make the call. Sample structural
  form for reference (debug-grade only, not authoritative):

  ```text
  Const::Bool(true)
  Const::Int(I32, 0xfffffffffffffffffffffffffffffff9)   // -7i32
  Const::Aggregate([Const/3, Const/7])
  Const::EnumValue { variant_idx: 1, payload: [Const/3] }
  Const::Reloc(Static @ defhash=<hex>, args=<hex>, shim=0)
  ```

  Stage 14 inherits the rendering-style decision from stage
  13's `render_ty` (Rust-source-style is the stage-3 lean
  preserved into stage-13 expectations -- e.g., `5i32`,
  `"hello"`, `(1, 2, 3)`, `Foo { a: 5 }` -- which would
  compose naturally with `render_const`).

  *Stage-14 deliverables.*

  1. Coordinate with stage 13's `render_ty` style decision;
     specify the rendering for each `Const` variant
     compatible with that style. Pin as a stage-14 decision.
  2. Specify the rendering for each variant of the closed
     11-variant `Const` enum (see
     `../stage-4/spec-snapshot.md` §9.7 for the variant
     list). Cover scalars (`Bool` / `Char` / `Int` / `Uint`
     / `Float`), `ZST`, `Bytes` (three-state byte sequence),
     `Aggregate` / `EnumValue` (recursion via
     `canonical_const`), `NullPtr` (recurses into
     `render_ty`), and `Reloc` (composes `render_def_path` +
     `render_instance`).
  3. Implement the renderer. Coordinate with stage 12's
     `render_def_path`, this stage's own `render_instance`,
     and stage 13's `render_ty` -- the four renderers should
     produce coherent output.
  4. Update insta snapshots accordingly.

  *Cross-refs.*
  - `../stage-4/spec-snapshot.md` §9.7 -- the 11-variant
    `Const` enum the renderer must handle, plus the `Byte`
    three-state and `InstanceRef` 4-tuple shapes.
  - `../stage-4/spec-snapshot.md` §13.3 --
    `ConstMemoryStatic` family (D-4-3); rendering of
    `Reloc` targets pointing at synthesised statics.
  - `../stage-4/discussions.md` D-4-K parent -- deferral
    rationale (dependency analysis on `render_ty` /
    `render_def_path` / `render_instance`).
  - `../stage-1/spec-snapshot.md` -- pretty-printer
    scaffold interface and the existing `render_const`
    stub.
  - `../stage-12/expectations.md` -- coordination on
    `render_def_path` style.
  - `../stage-13/expectations.md` -- coordination on
    `render_ty` style (the input that `render_const`
    inherits from).

## Bridge concerns

- **Domain attribute parsing -- architecture only.**
  `#[rseq::*]` attributes become structured `DomainAttr`
  on `Item` / `Instance` per the C-semantics methodology
  rule (don't pass annotations through as opaque
  source-side metadata; structure them per the consumer's
  needs). Stage 14 specifies the **architecture**
  (sibling-table key-kind rule on `RegseqModel` root +
  `IndexVec` storage pattern + bridge population +
  validation contract). Per-family attribute parsing
  (`#[rseq::mmio(...)]`, `#[rseq::interrupt(...)]`,
  `#[rseq::clock(...)]`, etc.) is per-phase-2-stage
  territory.

- **Recognition surface.** Bridge code identifies
  intrinsics by attribute-name lookup against the closed
  vocabulary maintained per stage-14's Q7 mechanism.
  Mirror the "no defensive fallbacks" rule: unknown names
  are rejected.

## From stage 3

- [ ] **Bridge-side rejection of `core::ffi::c_void`** (from
  stage 3 D16).

  *Stage-3 commitment.* D16 hardens D1.5's "recognition
  deferred" into "actively rejected pending recognition."
  The bridge rejects `core::ffi::c_void` at type
  translation with a diagnostic recommending the
  opaque-struct pattern. See
  `../stage-3/spec-snapshot.md` Section 1.15 (D16 audit
  shape) and Section 4.15 (worked example).

  *Stage-14 deliverable.* Enforce the type-translation-
  phase rejection at the bridge:
  - Recognition: `tcx.is_lang_item(did, LangItem::CVoid)`
    (per D1.5's locked recognition-path note in
    `../stage-3/spec-snapshot.md` Section 1.6 lines
    402-406; not in `AdtFlags`).
  - Scope: the c_void type itself; pointers to it
    (`*mut c_void`, `*const c_void`) reject transitively
    when the inner type is encountered during pointer
    translation -- not via a separate raw-pointer rule.
  - Diagnostic: recommends the opaque-struct pattern
    (`#[repr(C)] struct Opaque { _private: [u8; 0] }`)
    per D1.4's existing precedent for opaque C-side
    handles.

  *Trigger conditions for re-opening D16 (deferred to
  post-v1).* A v1 driver fixture or phase-2 domain stage
  surfaces a stable need for FFI-canonical
  pointer-to-c_void lowering -- specifically
  `*mut c_void` -> C `void *` and/or
  `*const c_void` -> C `const void *` (the two pointer
  forms have distinct C spellings per locked D7/D8's
  `Ty::Ptr(TyId, Mutability)` and Section 1.12's
  raw-pointer `const`-derivation rule). When that
  materialises, D16 reopens to add an `is_c_void` flag on
  `Item` and lift the rejection.

  *Rationale for keeping in stage 14 (foundation).* This
  rejection is a universal bridge rule applying to every
  driver; it is not tied to any specific domain family
  (unlike `core::ptr::{read,write}_volatile` rejection
  which redirects to MMIO, or `core::sync::atomic`
  rejection which redirects to atomics). c_void rejection
  redirects to the universal opaque-struct pattern.

---

## From stage 13

Filed at stage-13 merger contract close (2026-05-07; per
README §5.5 step 7). Stage 14 owns rendering COMPOSERS per
the building-block-vs-composer architectural division
locked at D-13-4.6; stage-13 master rendering style
(D-13-1) governs all stage-14 rendering work. Spec
contributions land in top-level `pretty-printer.md`
(co-equal merger artifact per D-13-9), not `spec.md`.

### Composer renderers (3 items)

- [ ] **`render_const(ConstId) -> String`** -- originally
  D-4-K (deferred from stage 4 because `render_const`'s
  dependency chain is longer than `render_ty`'s). Composes
  `render_ty` (stage 13) for type-context arms across the
  closed 11-variant `Const` enum (`spec.md` §17 / D-4-1):
  Bool / Char / Int / Uint / Float / ZST / Bytes /
  Aggregate / EnumValue / NullPtr / Reloc. Inherits D-13-1
  master Rust-source-style. Per-arm specs land in
  `pretty-printer.md` §6 (or new dedicated section);
  worked-example specimens land in §9 alongside stage-13's
  specimens.

- [ ] **`render_instance(InstanceId) -> String`** --
  originally stage-1 inbound; the headline consumer-facing
  renderer per D-13-4.6 architectural division. Composes
  `render_item` (stage-13 wrapper output: attribute block +
  `[unsafe ]fn` / `static [mut ]` / `const` / `struct` /
  `enum` / `union` kind-keyword + DefPath) + per-Instance
  content via `InstanceKind` dispatch:
  - `Function { fn_sig, body, flavor, virtual_idx }` -->
    signature `(arg_tys) -> ret_ty` rendered via `render_ty`
    over `FnSig`; body rendered via D-13-2 envelope +
    body-level renderers.
  - `Static { ty, init }` --> `: <render_ty(ty)> =
    <render_const(init)>`.
  - `Const { ty, value }` --> `: <render_ty(ty)> =
    <render_const(value)>`.
  - `Struct/Enum/Union { layout }` --> ADT field/variant
    layout rendering composed from `render_ty` over the
    parallel `Ty::Struct/Enum/Union`.

- [ ] **Top-level `render(&self) -> String`** -- originally
  stage-1 inbound + reframed at D-13-4.6 (stage 14 owns
  composers + top-level dump). Iterates `model.instances`
  (Q2 codegen-keyed) and composes per-Instance via
  `render_instance`. Inherits D-13-2 envelope at the
  per-function level; per-Static / per-Const / per-ADT
  emissions follow the rendered shape from
  `render_instance`.

### Master-style inheritance

All three composers inherit:
- D-13-1 master rendering style (Rust-source-style with
  sigil decorations; `PrettyPrinter<'a>` methods returning
  `String`).
- D-13-2 body envelope + 4-/8-space indentation + sigil
  wirings (`_<id>`, `bb<id>`).
- D-13-3 span / debuginfo gating via `PrettyPrintOptions`
  (`include_spans`, `include_debug_names`); `with_options`
  constructor; default-off.
- D-13-4 Item-level rendering shape (composed by
  `render_instance` for the Item-side wrapper portion).
- D-13-5 `render_ty` per-arm specs (consumed by
  `render_const` for type-context arms; by
  `render_instance` for FnSig / layout rendering).
- D-13-6 body-level renderer per-arm specs (consumed by
  `render_instance` for Function-Instance bodies).
- D-13-7 worked-example specimen format + 1:1 insta
  mirroring; stage-14 specimens land in
  `pretty-printer.md` §9 alongside stage-13's ~75
  specimens.

### Spec split inheritance (per D-13-9)

Stage-14's pretty-printer spec contributions land in the
top-level `pretty-printer.md` companion file, NOT
`spec.md`, per README §5.9 rule 6. Working surface:
`stage-14/pretty-printer-snapshot.md` for the bulk
authoring + minimal in-place edits to existing
`spec-snapshot.md` for cross-references where needed. At
stage-14 merger close, `pretty-printer.md` integrates the
new composer + foundation-extension content; `spec.md`
sees structural-spec edits where the foundation extensions
demand them (Q7 storage on `RegseqModel`, DomainAttr
sibling-table architecture, `IntrinsicKind` enum).

### F-12-Reify (informational; carried forward from stage 12)

Stage-12 surfaced rustc's `InstanceKind::ReifyShim` payload
drift to `(DefId, Option<ReifyReason>)` (see
`../stage-12/discussions.md` §5). Flagged for stage-9
reopening if codegen / phase-2 device pressure surfaces
collisions. Stage-14 may re-evaluate during
intrinsic-recognition or domain-extension work if the
reify-shim path becomes load-bearing.

---

## From external review (2026-05-07)

External review at
`/Users/nyengele/Downloads/regseq-dsl-spec-review.md`
surfaced four prior-stage findings that stage 14 closes
alongside its primary scope. All four claims verified
against the actual spec text on 2026-05-07. One additional
reviewer claim (JSON trust-model warning label) verified as
already addressed at `spec.md:615-642` and is NOT carried
forward.

Each finding lands at stage-14 merger close: in-place edits
to top-level `spec.md` / `pretty-printer.md` per README
§5.9 rule 1; status flips in `decisions.md` per §5.5 step 4
where prior locked decisions are revised; new D-14-X
entries in `decisions.md` under `## Stage 14`; rationale
audit-trailed in `stage-14/discussions.md`.

### Finding #1 -- Operational-intrinsic recognition + storage gap

- [ ] **Correct the `#[rseq::intrinsic("NAME")]`
  recognition paragraph and add resolved-intrinsic
  storage.**

  *Issue.* The §8 forward-pointer paragraph at
  `spec.md:1754-1758` carries two errors:
  - (a) Names `tcx.intrinsic_raw(did)` as the recognition
    mechanism. `intrinsic_raw` returns
    `Option<rustc_middle::ty::IntrinsicDef>` for items
    marked `#[rustc_intrinsic]` -- a rustc-internal
    compiler-intrinsic API. It is the wrong hook for
    custom tool-namespace attributes;
    `#[rseq::intrinsic("NAME")]` is visible only via
    attribute-walking machinery rooted by the crate-level
    `#![register_tool(rseq)]` declaration.
  - (b) Says the attribute is "not stored on Item." Per
    Q2's eager-translation rule, `'tcx` does not survive
    past the bridge call; downstream codegen cannot
    re-query rustc, so the resolved intrinsic identity
    must live on `RegseqModel`.

  *Resolution.* Stage 14 lands new decision D-14-X:
  - **Recognition.** Bridge walks callee attributes via
    rustc tool-namespace machinery, matches the
    `rseq::intrinsic` path, parses the NAME literal
    against a closed `IntrinsicKind` vocabulary
    (closed-set scope defined for stage 14 per Q7 + the
    diagnostic-logging vocabulary; MMIO / atomic /
    interrupt / clock entries appended at their
    respective phase-2 stages); unknown names rejected
    per Q7's closed-vocabulary rule.
  - **Storage.** Per-Item sibling table on `RegseqModel`:
    ```rust
    pub enum IntrinsicKind { /* closed, append-only */ }
    pub struct IntrinsicInfo {
        pub kind: IntrinsicKind,
        pub source_item: ItemId,
        pub name: StringId,        // diagnostic only
    }
    pub intrinsic_items: IndexVec<ItemId, Option<IntrinsicInfo>>;
    ```
  - **Codegen consumption.**
    `TerminatorKind::Call { callee: Direct(InstanceId), .. }`
    stays structurally ordinary; classifier walks
    `model.intrinsic_items[model.instances[id].item]`.

  *Edits at stage-14 close.*
  - **No D-tag flip.** D-12-1.5 covers DomainAttr
    declarative property attributes (`#[rseq::mmio(...)]`
    / `#[rseq::interrupt(...)]` family) -- not the
    operational `#[rseq::intrinsic]` attribute. The §8
    paragraph is forward-looking commentary; stage 14
    fulfils it (in-place edit per README §5.9 rule 1)
    rather than reopens it.
  - In `spec.md:1754-1758`, replace the paragraph in
    place with the corrected mechanism + storage
    description.
  - Add `IntrinsicKind` enum + `IntrinsicInfo` struct +
    `intrinsic_items` field schema in stage-14's new
    sibling-tables section (location to be decided
    alongside the broader DomainAttr architecture).

### Finding #2 -- `is_naked` on Item with no representable body

- [ ] **Reject `#[naked]` in v1; remove `is_naked` field.**

  *Issue.* Stage-12 D-12-1.3 carries `is_naked: bool` on
  Item (`spec.md:1631-1636`) with comment "C-codegen emits
  naked functions without prologue/epilogue (raw asm body
  only); load-bearing for ISR entry stubs in kernel
  drivers." Stage-9 D-9-9 rejects
  `TerminatorKind::InlineAsm` (`spec.md:8312-8315`). A
  function marked `is_naked: true` therefore has no
  representable body -- the IR has no carrier for the raw
  asm. The two locked decisions are irreconcilable as
  currently written.

  *Resolution.* Stage 14 lands new decision D-14-Y:
  **reject `#[naked]` in v1**. Mirrors D-9-9's InlineAsm
  posture. Future driver pressure for ISR entry stubs
  handled via a separate domain extension (deferred;
  `#[rseq::interrupt(...)]` synthesises a C/asm wrapper
  around an ordinary Rust handler body). Smaller-scope
  option chosen over the reviewer's preferred
  domain-extension framing because v1 has no concrete
  ISR-stub fixture demanding the extension; rejection
  now, lift later under fixture pressure is consistent
  with D-12-3.3's "is_pin / is_c_void / is_box deferred
  per documented criteria" pattern.

  *Edits at stage-14 close.*
  - **D-12-1.3 [REOPENED]** in `decisions.md` with
    annotation `(revised in stage 14 per D-14-Y:
    is_naked field removed; v1 rejects
    #[naked]-attributed Items at bridge translation.)`.
  - **D-9-9 [REOPENED]** in `decisions.md` with
    annotation `(revised in stage 14 per D-14-Y:
    rejection table extended with #[naked]
    Item-translation row, sibling to InlineAsm
    Call-translation row.)`.
  - **D-13-4.3 [REOPENED]** in `decisions.md` with
    annotation `(revised in stage 14 per D-14-Y:
    #[naked] row removed from per-flag rendering table
    -- flag no longer exists on Item.)`.
  - In `spec.md:1631-1636`, delete the `is_naked` field
    block (the `// is_naked corresponds to #[naked] ...`
    comment + `pub is_naked: bool,` line).
  - In `spec.md:1298`, delete the `instance_is_naked`
    helper line.
  - In `spec.md:1738`, drop "`is_naked` +" from the
    field enumeration narrative.
  - In `spec.md` §23.6.1 (or the Item-translation
    rejection-table area), add a new row: `#[naked]`
    Items rejected at Item translation; diagnostic
    recommends deferring to a future ISR-stub domain
    extension.
  - In `pretty-printer.md` per-flag rendering table
    (D-13-4.3 surface), remove the `#[naked]` row.
  - Specimen 70 in `pretty-printer.md` updated under
    Finding #4.

### Finding #3 -- Stale "stage 13 (codegen)" forward reference

- [ ] **Retarget the dead `emission_name` forward
  pointer.**

  *Issue.* `spec.md:1892` reads "the construction rule is
  pinned in stage 13 (codegen)." Stage 13 closed as the
  pretty-printer spec (D-13-9), not codegen. The
  C-identifier-safe construction algorithm for
  `Struct / Enum / Union` `emission_name` from
  `(DefPath, ArgsKey)` is genuinely unspecified.

  *Resolution.* No D-tag flip -- pure spec hygiene. Stage
  14 retargets to "future C-backend spec" since the
  algorithm itself is C-backend territory (escaping for C
  keywords, non-ASCII, leading digits, double
  underscores; hash-suffix collision-resistance scheme;
  length cap and maximum-name fallback). Phase-1 commits
  only to determinism + uniqueness, both already
  preserved by `(DefPath, ArgsKey)` content.

  *Edits at stage-14 close.*
  - In `spec.md:1888-1892`, replace "the construction
    rule is pinned in stage 13 (codegen)" with: "phase-1
    commits to determinism + uniqueness only; the full
    normative algorithm (escaping for C keywords /
    non-ASCII / leading digits / double underscores,
    hash-suffix collision-resistance scheme, length cap
    and maximum-name fallback) is the C-backend spec's
    responsibility and is out of phase-1 scope."
  - Logged in `stage-14/discussions.md` as a stage-14
    hygiene fix (no new D-tag).

### Finding #4 -- Specimen 70: impossible `opaque_handle_eligible` on Function

- [ ] **Split Specimen 70 -- Functions are structurally
  ineligible for `opaque_handle_eligible: true`.**

  *Issue.* `pretty-printer.md:1460-1475` Specimen 70
  ("Function with full attribute stress") declares
  `kind: ItemKind::Function, ...,
  opaque_handle_eligible: true` and renders
  `#[opaque_handle]` above `unsafe fn bsp::reset`. But
  `spec.md:1647-1648` restricts
  `opaque_handle_eligible: true` to `ItemKind::Struct`
  (non-generic, `repr(C)`, ZST). The specimen exercises a
  state the bridge will never produce.

  *Resolution.* No D-tag flip on D-13-7 -- the decision
  text (~75 specimens, per-arm spec coverage) is not
  revised; only the specimen content is. Stage-13
  internal bug found during stage-14 review; surgical
  fix.

  *Edits at stage-14 close.*
  - In `pretty-printer.md:1460-1475`, replace Specimen
    70 with a Function-only stress (drop
    `opaque_handle_eligible: true` and `is_naked: true`
    per Finding #2; output drops `#[naked]` and
    `#[opaque_handle]` lines).
  - Add a new Specimen 70b covering Struct with
    `opaque_handle_eligible: true` to preserve coverage
    of the bridge-outcome rendering. Specimen count
    drifts to ~76 (acceptable per D-13-7.2's
    policy-bucket-exception spirit; alternative is to
    renumber the rest, but the gap is harmless).
  - Logged in `stage-14/discussions.md` as a stage-13
    specimen-bug fix (no new D-tag).

### Cross-finding integration

Findings #2 and #4 interact: Specimen 70 currently
combines Function + naked + opaque_handle. After Finding
#2 drops `is_naked` from Item-side and Finding #4 drops
`opaque_handle_eligible: true` from Function specimens,
Specimen 70 becomes a clean "Function with linkage +
inline + link_section + unsafe" stress; the new Specimen
70b carries `opaque_handle_eligible: true` on a Struct.
Both edits land together at stage-14 merger close to keep
the specimen surface coherent.

Findings #1 and the broader DomainAttr sibling-tables
work overlap: D-14-X's `intrinsic_items` table is the
first instance of the sibling-tables architecture. Stage
14 should land both in the same merger pass so the schema
decisions stay coherent (`intrinsic_items` vs
`item_props` vs `instance_props` vs `field_props` vs
`span_props` -- see the reviewer's suggested
`DomainTables` sketch for one possible shape; not
binding).

### Out-of-scope reviewer points (informational)

The external review also raised three concerns explicitly
framed as "C backend spec, not RegseqModel core" by the
reviewer himself. Recorded here so stage 14 does not
accidentally absorb them:

- *C ABI boundary marshalling adapter.* The reviewer's
  proposed `lower_abi_arg` / `raise_abi_return` /
  `emit_local_c_abi_definition` / `emit_c_abi_call`
  algorithm is C-backend spec territory. RegseqModel
  already stores the Rust-side `FnSig.inputs / output`
  plus the boundary-shape evidence in
  `sig_c_abi_lowerings`; the executable algorithm
  consuming both is downstream of phase-1.
- *Shared C aggregate emission algorithm.* Reviewer's
  proposed `c_layout_plan(ty: TyId) -> CLayoutPlan`
  shared helper. Phase-1's `Layout` already stores the
  bridge-resolved facts (offsets, padding,
  packed/aligned, ZST presence); phase-1 makes no
  commitment to the C-emission algorithm.
- *Normative C identifier / name-mangling algorithm.*
  See Finding #3 -- the algorithm itself is C-backend
  spec; only the stale forward-pointer is in phase-1
  scope.
