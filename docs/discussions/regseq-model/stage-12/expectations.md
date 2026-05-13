# Expectations on stage 12 (incoming)

Stage 12 covers Rust Items: expands the `Item` placeholder to
carry source-level item metadata (`def_path`, `kind`, attributes,
visibility, ABI / linkage / inline hints, unsafe flag).

The convention is described in `../README.md` under "Forward
expectations live in `stage-N/expectations.md`."

## From stage 1

- **Expand `Item` placeholder.** The empty struct at
  `crates/regseq-model/src/placeholders.rs` becomes the full
  Item shape. Per the spec ("Lifting `impl` items"), `ItemKind`
  has six variants -- `Function`, `Static`, `Const`, `Struct`,
  `Enum`, `Union` -- mirroring `InstanceKind`. No `Impl`,
  `Trait`, or `AssocType` variants (those are erased / lifted
  per the methodology). Move the type out of `placeholders.rs`
  into its own module (e.g., `item.rs`).

- **`Item.def_path: DefPath` field.** Per the spec ("Where
  DefPath lives"), `DefPath` is part of `Item` at stage 12.
  Stage 1 ships the `DefPath` type and the
  `render_def_path` helper; stage 12 wires the field onto Item.

- **Bridge populates `Item.def_path.rendered`.** When the bridge
  constructs an `Item` for a source-level def, it calls
  `render_def_path(&path, &model.crates, &model.strings)`
  (already shipped in `crates/regseq-model/src/def_path.rs`),
  interns the resulting String, and stores the StringId on
  `Item.def_path.rendered`. This closes the construction
  invariant pinned in stage 1: two RegseqModels for the same
  source produce byte-identical `rendered` strings for
  corresponding items. The bridge must call the helper -- it
  must not re-implement the rendering rule.

- **Fill `pretty.rs::render_def_path(ItemId)` stub.** Stage-1
  stub at `crates/regseq-model/src/pretty.rs` panics with a
  message naming stage 12. Stage 12's body is:

  ```rust
  self.model.strings.resolve(self.model.items[id].def_path.rendered).to_owned()
  ```

  No re-rendering: the eager `rendered` field is the cached
  result of `render_def_path` at construction time. The
  `render_def_path_value(&DefPath)` method shipped in stage 1
  remains useful for paths not attached to an `Item`.

- **Close construction invariant 6.** Stage 1 deferred this
  invariant. Once `Item.def_path` exists, `validate` walks
  every item and checks
  `item.def_path.crate_id.index() < crates.len()`. Add positive
  + negative test in `tests/model.rs`. The deferred-stub comment
  in `crates/regseq-model/src/model.rs::validate` calls this out
  explicitly.

- **Close construction invariant 3.** Stage 1 deferred the F4
  item-sort-order check. Once `Item.def_path` exists, `validate`
  can verify items are placed in the F4-sorted order by
  computing the comparator key from
  `(crates[item.def_path.crate_id].name, item.def_path.segments,
  ...)` and checking monotonicity. Add positive + negative test
  in `tests/model.rs`. The deferred-stub comment in `validate`
  calls this out.

- **Lang-item / instance-kind classification.** Stage 12 covers
  the convergent-pattern-5 lang-item recognition described in
  `stage-0/prior-art/README.md`. Stage 14 builds on this for intrinsic
  recognition (Q7).

- **Bridge-synthesized Items.** Per the spec section on
  "Bridge-synthesized Items," the principal case is trait-object
  vtables: when MIR has a `dyn Trait` value, the bridge
  synthesizes a Static `Instance` whose value is the vtable
  struct. This Static needs an `Item` -- rustc has no
  source-level `DefId` for the vtable itself. Synthesized Items
  are conceptually owned by the bridge but appear in `items`
  alongside source-level items so consumers can walk
  `instances` uniformly. Construction invariant 2
  (`item_def_paths` mirrors `items`) extends to synthetic items:
  `item_def_paths[item_id]` is the bridge-derived `DefPathHash`
  defined in the spec section.

- **Niche optimization for `Option<ItemId>`.** Stage 1's
  `lib.rs` asserts `Option<ItemId>` is 4 bytes. Don't break it.

## Bridge concerns

- **Closes construction invariant 4 (spec-crate-stable-id half).**
  Stage 1's `RegseqModel::validate` checks
  `primary_crate.index() < crates.len()` but defers the
  "corresponds to spec crate" half pending bridge input. Stage
  12 (or whenever the bridge starts producing real models) adds
  a `BridgeInput` parameter to validation that supplies the
  expected `spec_crate_stable_id`; validate then checks
  `crates[primary_crate].stable_crate_id == spec_crate_stable_id`.

- **Item ordering.** Bridge sorts items per the F4 comparator
  before assigning `ItemId`s, with rustc's `Symbol` interner in
  scope. After sort + ItemId assignment, names are re-interned
  through the model's `StringInterner`. See
  `stage-1/spec-snapshot.md` "Determinism (F4 in detail)" for the
  full algorithm.

- **`item_def_paths` parallel side table.** Bridge populates
  `item_def_paths[item_id]` with rustc's `DefPathHash` for the
  originating `DefId` (or the bridge-derived hash for synthetic
  items). Construction invariant 2 enforces length parity at
  stage 1; content correctness lands here.

## From stage 3

- [ ] **Define and synthesise vtable type Items, vtable instance
  Items, vtable shim Function Items, and closure synthesis
  details** (from stage 3 D5 and D6).

  *Stage-3 work already done.* The Ty-level shapes are pinned:
  `Ty::Dynamic { auto_traits, vtable_ty: TyId }` (D5.1);
  `Ty::Closure(ItemId, GenericArgs)` (D1.1 + D6.1 contract);
  the `Ty::Closure` / `Ty::Struct` shared-Item connection (D6.3).
  See `stage-3/spec-snapshot.md` Sections 1.10 and 1.11.

  *Stage-12 deliverable.* Synthesise the following
  bridge-synthesised Items (extending stage-1's
  "Bridge-synthesized Items" framework at
  `stage-1/spec-snapshot.md:901-942`):

  ### A. Vtable type Items (D5)

  Per `(Trait, projection_bindings)` reachable in the IR (via
  `Ty::Dynamic` construction), synthesise:

  - **One Item.**
    - `kind`: `ItemKind::Struct`.
    - `def_path` rendered form: `<vtable type for `Trait`<projection_args>>`
      (or alternative; pick a form distinct from the per-(Trait,
      SelfTy) instance Item below).
    - `def_path_hash`: `hash((trait_def_path_hash,
      projection_args_canonical_hash))`. Stable across builds.
    - `crate_id`: per `stage-1/spec-snapshot.md:931-933`.
    - DefPathData segment representation: pick concrete encoding
      (overload an existing variant, or add a new
      `DefPathData::VTableType` variant per stage-1's
      anticipated revision procedure at
      `stage-1/spec-snapshot.md:919-927`).

  - **One `Ty::Struct(item, args, layout)`.** Interned via
    `translate_adt`'s reservation flow with the synthesised
    ItemId. Layout: synthesised directly from rustc's
    `tcx.vtable_entries(trait_ref)` sequence (drop_glue ptr
    field + size + align + per-method fn-ptr fields + supertrait
    vtable ptrs + Vacant slots for non-dispatchable methods, per
    `stage-1/spec-snapshot.md:884-895`). Field types use **erased**
    receivers (`*const ()` / `*mut ()`) per the rustc vtable
    convention.

    *Vacant slot representation.* Each `Vacant` entry from
    rustc's `tcx.vtable_entries` corresponds to a
    non-dispatchable method (e.g., a method with a `Self: Sized`
    bound). Stage-12 must represent these as **explicit
    placeholder fn-pointer fields with a fixed null
    initializer** in the vtable instance, *not* as omitted
    fields. The rationale: stage-11's rewriting pass identifies
    methods by their position in `tcx.vtable_entries`, and that
    position must align with the field index in the vtable
    `Ty::Struct`. Omitting Vacant entries would shift indices
    and break method-index lookup; using an explicit placeholder
    preserves index alignment with rustc's vtable layout. The
    placeholder field type is the same erased fn-pointer shape
    as a normal method entry; the instance initialiser sets it
    to `0` (NULL).

  ### B. Vtable instance Items (D5)

  Per `(Trait, ConcreteImpl)` reachable (via `Cast::Unsize`
  rvalues at coercion sites; stage-11's pass triggers
  registration), synthesise:

  - **One Item per stage-1's reservation at
    `stage-1/spec-snapshot.md:910-927`**:
    - `kind`: `ItemKind::Static`.
    - `def_path` rendered form: `<vtable for `Trait` for
      `SelfTy`>` (stage-1's existing reservation).
    - `def_path_hash`: `hash((trait_def_path_hash,
      self_ty_canonical_hash))` (stage-1's existing reservation).
    - DefPathData segment representation: same as the type Item
      above (or distinct -- stage-1 anticipates either choice).

  - **One Instance** with:
    - `InstanceKind::Static { ty: <vtable type Item's
      Ty::Struct's TyId>, init: <ConstId for the const aggregate> }`.
    - `init` references the drop-in-place shim Function Item's
      InstanceId (per C.2 below) plus per-method shim Function
      Item InstanceIds (one per method, per C.1 below) plus
      const size/align values, in the field order of the vtable
      type. **Both** the drop slot and the method slots use
      shims rather than raw concrete-receiver Instances, because
      the vtable fn-pointer field types are erased-receiver
      throughout.

  ### C. Vtable shim Function Items (D5)

  Two families of shims, both motivated by the same
  erased-vs-concrete receiver mismatch -- vtable fn-pointer slots
  use erased receivers (`*const ()` / `*mut ()`); impl methods
  and rustc's `DropGlue` Instance use concrete receivers.

  **C.1 Method shims.** Per `(Trait, ConcreteImpl, method)`
  (where method ranges over the trait's dispatchable methods),
  synthesise:

  - **One Function Item** (rustc precedent:
    `InstanceKind::VTableShim` per
    `stage-1/spec-snapshot.md:786-805`):
    - `kind`: `ItemKind::Function`.
    - `def_path` rendered form: pick a synthetic form (e.g.,
      `<shim for `Trait`::`method` for `SelfTy`>`).
    - Body: takes erased receiver (`*const ()` or `*mut ()`),
      casts to concrete (`&SelfTy` or `&mut SelfTy`), tail-calls
      the lifted impl method (which has concrete receiver per
      `stage-1/spec-snapshot.md:597-603`).
    - Signature: matches the vtable type Item's corresponding
      function-pointer field (erased receiver).

  **C.2 Drop-in-place shim.** Per `(Trait, ConcreteImpl)` (i.e.,
  per concrete `SelfTy` reachable as a trait object), synthesise:

  - **One Function Item**:
    - `kind`: `ItemKind::Function`.
    - `def_path` rendered form: pick a synthetic form (e.g.,
      `<drop shim for `SelfTy` for `Trait`>`).
    - Body: takes erased receiver `*mut ()`, casts to `*mut
      SelfTy`, tail-calls `core::ptr::drop_in_place::<SelfTy>`
      (rustc's `DropGlue` Instance for `SelfTy` per
      `rustc_middle/src/ty/instance.rs:152-157`).
    - Signature: `fn(*mut ())` -- erased receiver, matches the
      vtable type Item's `drop_in_place` slot.

  Without C.1 method shims, vtable method-slot initializers
  cannot type-check; without the C.2 drop-in-place shim, the
  vtable's drop slot has the same problem -- rustc's
  `tcx.vtable_entries` resolves the drop entry to concrete
  `drop_in_place::<SelfTy>` per
  `rustc_middle/src/ty/vtable.rs:121-130`, whose `fn(*mut
  SelfTy)` signature does not type-equal the vtable slot's
  `fn(*mut ())`. Both shim families bridge the erased-receiver
  gap with identical synthesis machinery; the only difference is
  what the shim tail-calls (a lifted impl method vs.
  `drop_in_place::<SelfTy>`).

  ### D. `RegseqModel.vtables` dedup table (D5)

  - **Schema:** `vtables: HashMap<(DefPathHash, ArgsKey),
    ItemId>`. Key components are IR types (`DefPathHash` is a
    stage-1 concept; `ArgsKey` is the F4-2 deep-canonicalizer
    output for `GenericArgs` per `stage-3/spec-snapshot.md` Section
    3.1); zero rustc deps (Q1 preserved). Using `ArgsKey` rather
    than raw `GenericArgs` ensures the key is stable cross-build
    (raw `GenericArgs` carry `TyId`/`ConstId` whose integer
    values may differ across builds per Section 3.2's cross-build
    determinism rule; only canonical forms are stable).
  - **Population:** by the bridge during type translation, when
    `rustc::TyKind::Dynamic` is encountered. The bridge mints
    the vtable type Item (one-shot) and caches its ItemId for
    repeat encounters of the same `(Trait,
    projection_bindings)`. The `ArgsKey` is computed once per
    insertion via the deep canonicalizer over the projection
    args.
  - **Serde:** mark as `#[serde(skip)]` (bridge-only,
    non-serialized); rebuild on deserialize from
    `RegseqModel.items` by walking synthesised vtable type Items
    and re-keying by `(trait_def_path_hash, ArgsKey-of-
    projection_args)`. Mirrors how `TyInterner.lookup` is
    rebuilt at deserialize per `stage-3/spec-snapshot.md` Section
    3.2's serialization rules.

  ### E. Closure captures-struct Items (D6)

  Per closure encountered in MIR, synthesise:

  - **One Item.**
    - `kind`: `ItemKind::Struct`.
    - `def_path`: rustc's closure DefPath verbatim (uses
      `DefPathData::Closure` segment per
      `stage-1/spec-snapshot.md:472`).
    - Fields: from rustc's `tupled_upvars_ty()` (per
      `stage-3/findings-rustc-draft.md` A.3:125), flattened to
      named fields (`_0`, `_1`, ...).
    - Layout: from `tcx.layout_of(closure_ty)`.

  - **Two Ty entries** for the same Item:
    - `Ty::Closure(item, args)` -- identity view, no inline
      layout (D1.1).
    - `Ty::Struct(item, args, layout)` -- layout-bearing view.
    - Connection: shared `(ItemId, GenericArgs)` per D6.3;
      consumers use `model.types.find` to navigate.

  ### F. Closure call-method Function Items (D6)

  Per closure, synthesise 1-3 Function Items based on
  `ClosureKind` (per `stage-3/findings-rustc-draft.md`
  A.3:127):

  - **`FnOnce`-typed closure** -> `call_once`.
  - **`FnMut`-typed closure** -> `call_mut` + inherited
    `call_once`.
  - **`Fn`-typed closure** -> `call` + inherited `call_mut` +
    inherited `call_once`.

  Each Function Item:
  - `kind`: `ItemKind::Function`.
  - `def_path`: closure's DefPath as parent +
    `ValueNs(intern("call" | "call_mut" | "call_once"))` segment
    with disambiguator 0 (D6.2).
  - Signature: substituted from `closure_sig_as_fn_ptr_ty`
    (`stage-3/findings-rustc-draft.md` A.3:108-114), with the
    captures struct as receiver:
    - `call_once`: receiver by value.
    - `call_mut`: receiver by `&mut`.
    - `call`: receiver by `&`.

  ### G. Closure drop-glue Function Item (D6)

  Iff the captures need dropping (any field has non-trivial
  drop), synthesise a drop-glue Function Item:

  - `kind`: `ItemKind::Function`.
  - Body: `drop_in_place` semantics for the captures struct.
  - Handled by the same drop-glue machinery used for any other
    struct with non-trivial fields per
    `stage-1/spec-snapshot.md:686-693`; not closure-specific.

  *Cross-refs.*
  - `stage-3/spec-snapshot.md` Sections 1.10, 1.11, 4.9, 4.10
    (full SESSION-A spec).
  - `stage-3/discussions.md` Section 1 D5 / D6 entries; Section
    2.6 region-erasure primer.
  - `stage-1/spec-snapshot.md:786-805` (rustc InstanceKind including
    Virtual / VTableShim); :820-868 (current six-variant
    InstanceKind); :884-895 (vtable Ty deferred to stage 9/12);
    :901-942 (Bridge-synthesized Items framework, vtable Static
    Item reservation).
  - `stage-1/spec-snapshot.md:597-603` (impl-method lifting with
    concrete receivers; motivates the vtable shim need).
  - `stage-1/spec-snapshot.md:472`, `:545-554`
    (`DefPathData::Closure` and verbose anon-namespace
    rendering); `:600-611` (impl-method lifting for closure call
    methods); `:655-659` (`GenericArg` has no lifetime variant).
  - `stage-3/findings-rustc-draft.md` A.3 (`ClosureArgs`,
    `ClosureKind`), A.4 (`Dynamic`).
  - `stage-3/prior-art/charon.md` F.2 (closure synthesis), F.3
    (vtable synthesis).
  - `stage-3/prior-art/mrustc.md` F (closures-as-structs, fat
    pointers).

## From stage 5

(No stage-12 deliverables incoming from stage 5.)

The originally-filed `render_place` deferral was retargeted
from stage 12 to stage 11 on the D-5-2 + D-5-K combined Codex
review R1 (BLOCK finding `d5k-name-source-and-target`). It
was subsequently retargeted again from stage 11 to **stage
13** under stage-11 D-11-K's categorical rendering deferral
(see `../stage-11/discussions.md` D-11-K). Field and variant
names live in §11 layout schema's `FieldLayout.name` /
`VariantLayout.name` (already pinned at stage 3), reachable
at render time via the head local's `LocalDecl.ty`; `Item`
carries `def_path` / `kind` / `span` / flags /
`opaque_handle_eligible`, not field/variant name tables, so
stage 12 is not actually a `render_place`
dependency. The retargeted expectation now lives in
`stage-11/expectations.md` "From stage 5" alongside D-5-2's
Place-validation-walk expectation.

## From stage 9

**Filed 2026-05-05 at stage-9 Phase A close.**

- **[Stage-9 callee-shape disposition]
  Possibly extend `DefPathData` for synthetic vtable
  scopes (originally filed as stage-1 inbound to stage 9
  in `../stage-9/expectations.md`).** Stage-9 D-9-3
  resolved to candidate **(b)** -- widen `Call.callee`
  to `CallCallee::{Direct(InstanceId), Indirect(Operand)}`
  -- and uses the current spec's
  `InstanceKind::Function.flavor: FunctionInstanceFlavor`
  field (with `Virtual` variant per top-level `spec.md`
  §9.4; the `flavor` field is canonical in the current
  spec, post-stage-1 revision) for the bridge's
  emit-then-rewrite mechanism.
  Stage 9 introduces no new InstanceKind variant and
  therefore no Virtual-shim-scope DefPathData
  variant.

  **Stage-12's separate DefPathData encoding choice
  for vtable type Items and vtable instance Items**
  (per "From stage 3" Sections A and B above --
  "overload an existing variant, or add a new
  `DefPathData::VTableType` variant per stage-1's
  anticipated revision procedure at
  `stage-1/spec-snapshot.md:919-927`") **remains a
  stage-12 decision**; stage 9 does not close it. The
  stage-12 deliverable still owns the per-Item
  encoding choice.

  *Cross-refs.*
  - `../stage-9/expectations.md` "From stage 1"
    (original "Possibly extend `DefPathData`" inbound
    expectation -- now closed).
  - `../stage-9/discussions.md` D-9-3 (callee-shape
    audit; (b) decision rationale).
  - "From stage 3" Sections A.1 and B (this file --
    stage-12 still owns the DefPathData encoding choice
    for synthetic vtable Items).

- **Vtable shim Function Item synthesis (cross-ref to
  stage-3 inbound).** Stage-9 D-9-3 (b) commits to the
  body-level rewriting model: stage-11's pass produces
  the `Indirect(Operand)` Call shape by synthesizing
  vtable function-pointer loads in IR. The Operand the
  Indirect arm carries points at a vtable field whose
  type matches the **erased-receiver** vtable shim
  Function Items synthesised at stage 12 (per "From
  stage 3" Sections A.2 and C above). No additional
  stage-12 deliverable beyond what's already filed; this
  entry is informational.

## From stage 4

- **[CLOSED by D-4-3]** **`SyntheticItemKind::ConstMemoryStatic`
  family.** Closed in stage 4 under decision D-4-3
  (whole-family scope). Stage 4 closed both halves: the
  stage-4 contribution (bridge dispatch arms, const-shape
  composition, §3.3 Phase B step 9 dedup check) and the
  stage-12-territory contribution (§13.3 `SyntheticItemKind`
  enum extension with `ConstMemoryStatic { content_hash:
  ContentHash }` variant; §13.4 const-memory-family segments
  and `DefPathHash` canonical-payload-bytes row). All edits
  landed in `stage-4/spec-snapshot.md`. Stage 12 inherits the
  changes by reference; no further stage-12 spec work is
  required for this family. Cross-refs:
  `stage-4/spec-snapshot.md` §9.7 ("Reference-typed const
  handling (D-4-3)" subsection + bridge dispatch table
  `Slice` / `Indirect`-over-Memory rows + `Memory(memory)`
  GlobalAlloc projection row), §13.3 (`ContentHash` type +
  `ConstMemoryStatic` variant), §13.4 (const-memory family
  segments + DefPathHash construction), §3.3 Phase B step 9
  (`ConstMemoryStatic` dedup sub-bullet);
  `stage-4/discussions.md` D-4-3 parent + D-4-3.1 through
  D-4-3.10; `stage-4/review-log.md` D-4-3 rounds.

---

## From stage 11

Filed at stage-11 merger contract close (2026-05-05; per
README §5.5 step 7). Two structural items per D-11-3 +
D-11-5; one invariant-closure item per D-11-6.1 (B-7).

### `Item.kind = Function { fn_sig, body, ... }` schema expansion (D-11-3 + D-11-5)

Stage-11 D-11-3 reframed FnSig placement to Item-side
(supersedes stage-1 inbound's Body-side framing).
Stage-11 D-11-5 placed Body as a structural field of
Item (no separate `BodyId` allocation). Both place
function-defining content on a single Item per
`Item.kind = Function { ... }` per stage-12 schema.

**Stage-12 deliverable.** Expand the `ItemKind` enum's
`Function` variant to carry both:

```rust
ItemKind::Function {
    fn_sig: FnSigId,    // §14.3 5-field schema (inputs, output,
                        //   abi, c_variadic, safety) preserved unchanged;
                        //   stage-11 D-11-3 reframes placement only.
    body: Option<Body>, // None for foreign declarations
                        //   (extern "C" fn with no body); Some for
                        //   source-defined fns + bridge-synthesised
                        //   shims (drop glue, vtable shims).
    // ... other Function-relevant Item fields per stage-12 design ...
}
```

Stage-11 spec assumes `Item.kind.as_function() ->
&FunctionData` accessor or analogous extraction (the
§23.6.2 `callee_fn_sig` helper uses
`item.kind.as_function().fn_sig`). Stage 12 picks the
exact accessor shape; the structural content (fn_sig +
body + flavor-relevant metadata) is what stage-11
relies on.

**Cross-refs.**
- `../stage-11/spec-snapshot.md` §25.4 (FnSig location);
  §25.5 (Body-vs-Item linkage); §23.6.2 (`callee_fn_sig`
  helper).
- `../stage-11/discussions.md` D-11-3 + D-11-5 audit
  shape.
- `../decisions.md` `### Stage 11 (MIR bodies)` D-11-3 +
  D-11-5 entries.

### Construction invariant 9 closure (D-11-6.1 B-7)

Stage-11 D-11-6.1 fold-in pins the joint-validation
visitor's `visit_terminator::Call` arm to verify every
`CallCallee::Direct(InstanceId)` resolves in
`model.instances`. The verification is stage-11-owned
(visitor at `crates/regseq-model/src/visit.rs`); stage
12 owns `model.instances` integrity (one-to-one with
Items; populated by collector; exhausted by validator).

**Stage-12 deliverable.** Confirm `model.instances`
integrity invariant per stage-1 D-9 closure-completeness
when `ItemKind` finalises. No additional schema work
required at stage 12 beyond the existing
`model.instances` table; the closure check happens at
the joint-validation pass (stage-11-owned).

**Cross-refs.**
- `../stage-11/spec-snapshot.md` §25.6.3 (construction
  invariant fold-in).
- `../stage-11/discussions.md` D-11-6.1 audit shape.
- `../decisions.md` `### Stage 11 (MIR bodies)` D-11-6.1
  entry.
