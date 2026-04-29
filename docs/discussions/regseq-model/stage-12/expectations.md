# Expectations on stage 12 (incoming)

Stage 12 covers Rust Items: expands the `Item` placeholder to
carry source-level item metadata (`def_path`, `kind`, attributes,
visibility, ABI / linkage / inline hints, unsafe flag).

The convention is described in `../memory.md` under "Forward
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
  `prior-art/README.md`. Stage 13 builds on this for intrinsic
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
  `stage-1/vocabulary.md` "Determinism (F4 in detail)" for the
  full algorithm.

- **`item_def_paths` parallel side table.** Bridge populates
  `item_def_paths[item_id]` with rustc's `DefPathHash` for the
  originating `DefId` (or the bridge-derived hash for synthetic
  items). Construction invariant 2 enforces length parity at
  stage 1; content correctness lands here.
