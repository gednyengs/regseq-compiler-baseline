# External Modeled Items Proposal

## Purpose

This note proposes how the semantic procedural IR should handle nominal types
that are not authored in the driver spec crate but are referenced by the
supported code slice being analyzed.

The immediate driver for this proposal is the repeated failure to represent
`regseq`-owned types such as `RxChar` and `RegseqError` honestly in semantic
bodies without falling back to:

- `Opaque`
- fake `ItemId`
- premature ABI flattening

The proposal is intended to unblock a concrete implementation plan for
`regseq-lowering-07`.

This revision narrows the immediate implementation scope:

- reuse the existing `regseq-model` item types and identity machinery
- do not redesign `ItemId`
- do not require a broad canonical-path overhaul in this step
- introduce a dedicated external-item discovery/materialization pass before
  semantic lowering

## Core Problem

The current model assumes that meaningful nominal types appearing in semantic
bodies are item-hierarchy types.

That assumption is no longer true.

Some semantically relevant types are:

- external to the driver spec crate
- referenced in the supported analyzed slice
- not present in the current item hierarchy

Examples today include:

- `regseq::serial::RxChar`
- `regseq::error::RegseqError`

The issue is not that these are external in itself. The issue is that they are
real nominal types that appear in supported semantic flow but currently do not
receive honest item identity in the Regseq model.

## Design Goals

1. The compiler should model external items only when they are actually
   referenced by the analyzed supported slice.
2. The item hierarchy should remain truthful: no fake `ItemId`, no ad hoc
   scalar replacement merely to satisfy validation.
3. The semantic model should distinguish:
   - authored/local items
   - external normal items
   - external framework-semantic items
4. The solution should not require a hardcoded inventory of all external items.
5. A small amount of predefined semantic classification is acceptable for
   discovered framework types with Regseq meaning.

## Non-Goals

- Do not add a general “all crates in the dependency graph” model.
- Do not flatten semantic types to backend ABI representations in Phase 2B.
- Do not preserve the current limitation by weakening validation.
- Do not use fake `ItemId` placeholders.

## Proposed Rule

An external item may become a modeled item if and only if:

- it belongs to a supported external crate
- it is actually referenced by the supported analyzed slice
- it is needed to type or identify supported semantic IR faithfully

For the current implementation slice, the only supported external crate is:

- `regseq`

This is a discovery-driven rule, not a hardcoded inventory rule.

That means:

- if an external `regseq` item is never referenced by analyzed code, it should
  not appear in the modeled item set
- if an external `regseq` item is referenced by analyzed code and needed for
  type/identity fidelity, it should be modeled

## Proposed Model Direction

### 1. Broaden the item universe

The Layer 2 item hierarchy should no longer mean “authored driver-spec items
only”.

Instead, it should mean:

- all modeled items required by the supported analyzed slice

That includes:

- local/authored items
- discovered external items from supported crates

`ItemId` remains a single model-owned identifier:

```rust
pub struct ItemId(pub u32);
```

Do not split `ItemId` into local/external forms. Once an item is admitted to
`ItemStore`, it should behave like any other modeled item.

### 2. Reuse existing provenance for modeled origin

Modeled items should carry enough provenance to distinguish:

- local/authored item
- external discovered item
- external discovered framework-semantic item

For the immediate implementation slice, this does not require a new model
field. The existing `Provenance` field on modeled items is sufficient.

Example:

- local/authored item: existing authored provenance
- external discovered `regseq` item: `Provenance::new("external:regseq")`

The finer distinction between ordinary external items and framework-semantic
external items may remain classification logic for now rather than requiring a
new stored field.

### 3. Use crate-qualified canonical paths for external items

Canonical identity must be unambiguous across crates.

Today the paths effectively assume the local driver crate as the root. That is
no longer sufficient once external items become first-class modeled items.

For the immediate fix, this does not require a broad canonical-path redesign.
The existing `CanonicalPath(String)` can already carry crate-qualified external
paths directly.

The immediate rule should be:

- local/authored items keep the current normalized canonical paths
- discovered external items use the crate-qualified path already available from
  the external type identity source, for example:
  - `regseq::serial::RxChar`
  - `regseq::error::RegseqError`

This must still satisfy:

- deterministic
- crate-disambiguating
- stable within a compilation
- not backend-shaped
- consistent enough for resolver lookup and item reuse

If a broader canonical-path normalization scheme is needed later, it can be
added as a follow-on change. It is not required to unblock the current task.

## Discovery Strategy

External items should be discovered from the analyzed slice, not from a
prebuilt registry.

For the immediate implementation slice, discovery should be intentionally
narrow:

- scan `MirTy::Adt { path, .. }` in the MIR bodies that will be lowered by the
  supported slice
- when such an ADT path belongs to a supported external crate and is not
  already modeled, materialize it as an external modeled item

This discovery should happen in a dedicated prepass between authored hierarchy
construction and semantic lowering.

Do not hide hierarchy mutation inside `type_lowering::lower_mir_ty()`.

`lower_mir_ty()` should remain a pure mapping/lookup step against the already
prepared hierarchy.

Broader discovery sources may be added later if needed, but they are not
required for the current unblock.

If a discovered nominal type resolves to a supported external crate and is
needed for supported-slice lowering, it should be materialized as a modeled
item.

## Semantic Classification Rule

Discovery is dynamic.

Semantic classification may be partly predefined.

That means:

- the compiler should not hardcode “always include `RxChar` and
  `RegseqError`”
- but if those items are discovered, the compiler may classify them as
  framework-semantic items with special meaning

So:

- inclusion is discovery-driven
- classification may be rule-driven

## First-Slice External `regseq` Handling

The first implementation slice should support discovered external `regseq`
items needed by the current representative set, including cases such as:

- `RxChar`
- `RegseqError`
- `SerialErrors` if referenced
- `SerialConfig` if referenced in supported semantic lowering

These should be modeled as real hierarchy items with real `ItemId`, not as
`Opaque` and not as ABI scalars.

## Why This Direction

This proposal is preferred over the alternatives:

- Better than `Opaque`:
  it preserves semantic truth and keeps validation meaningful.
- Better than fake `ItemId`:
  it preserves identity coherence.
- Better than ABI scalarization:
  it keeps semantic lowering independent from backend representation.
- Better than a separate builtin-only type bucket:
  it also handles non-framework external nominal types from supported crates in
  a principled way while keeping the first implementation narrow.

## Expected Effects on the Current Problem

Under this proposal:

- `RxChar` becomes a discovered external modeled item
- `RegseqError` becomes a discovered external modeled item
- `SemanticType::Struct { id, name }` or `SemanticType::Enum { id, name }`
  becomes truthful again because the referenced item actually exists
- validation can continue to reject `Opaque` in supported bodies
- semantic lowering no longer needs fake ids or backend-shaped scalar
  replacements

## Implications for `regseq-lowering-07`

The current task should not continue patching around this gap locally.

Instead, the implementer’s next plan should assume:

1. external discovered `regseq` items may be materialized into the modeled
   item hierarchy
2. external items can use crate-qualified external paths directly as canonical
   identities for this step
3. semantic lowering should resolve nominal types through those modeled items
4. `RxChar` and `RegseqError` should no longer be represented as `U32`

This likely requires coordinated changes across:

- hierarchy construction / item materialization
- semantic lowering
- validation/tests

The preferred implementation shape is:

1. build the authored/local hierarchy as today
2. run an external-item discovery/materialization prepass over the MIR bodies
   in scope
3. add minimal discovered external `regseq` items to the `ItemStore`
4. run semantic lowering against the augmented hierarchy

This keeps the change minimal while fixing the semantic identity problem
honestly.

## Recommendation

Use this proposal as the basis for the next implementation plan.

That plan should specify:

- the dedicated external-item discovery/materialization prepass
- which MIR type forms it scans and how it recognizes supported external items
- how discovered external items are inserted into the hierarchy/store
- the crate-qualified canonical-path convention used for external items
- the provenance used for discovered external items
- how `type_lowering` and semantic lowering resolve those items through real
  `ItemId`
- what tests prove:
  - discovery is driven by analyzed code, not a hardcoded inventory
  - `RxChar` / `RegseqError` no longer become `Opaque` or `U32`
  - semantic lowering remains free of fake identity and ABI flattening
