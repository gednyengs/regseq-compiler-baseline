# Cross-Link Validation

This note records Step 6 of the semantic procedural IR validation checklist.

The goal is to validate the `DeclarationModel <-> Item Hierarchy` linkage
directly and verify that:

- all cross-layer identity uses `CanonicalPath`
- item-level and field-level cross-links are both covered
- the design does not fall back to short-name matching

## Inputs

- final research plan at `~/.claude/plans/happy-crafting-muffin.md`
- [ownership-and-identity-stress-test.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/semantic-procedural-ir/ownership-and-identity-stress-test.md)
- [model-layer-coherence-validation.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/semantic-procedural-ir/model-layer-coherence-validation.md)

## Cross-Link Contract Being Validated

The plan says:

- `CanonicalPath` is the authoritative shared cross-layer identity
- `ItemId` is local to the Item Hierarchy
- `LocalId` is local to a function body
- `Provenance` is supplementary metadata, not the primary identity key
- a `CrossLinkTable` resolves both item-level and field-level canonical paths

This is the correct target contract. Step 6 checks whether the plan actually
states it consistently enough.

## Validation Checks

### 1. Item-level identity

Validated item-level entities:

- `FunctionDef`
- `StructDef`
- `ImplDef`
- `ConstantDef`

Each of these carries:

- `canonical_path: CanonicalPath`

And the plan explicitly uses canonical path matching for:

- callback methods
- register blocks
- associated constants
- impl ownership context

Verdict:
- clean

### 2. Field-level identity

Validated field-level entities:

- `StructFieldDef`

`StructFieldDef` now carries:

- `canonical_path: CanonicalPath`

And the plan explicitly shows field-level matching for resource fields, e.g.:

- `MmioFieldDecl (regs) -> "...::Pl011FullSpec::regs"`
- `StructFieldDef (regs) -> "...::Pl011FullSpec::regs"`

Verdict:
- clean

### 3. Cross-link resolution table

The plan now defines:

```rust
pub struct CrossLinkTable {
    path_to_item: BTreeMap<CanonicalPath, ItemId>,
    path_to_field: BTreeMap<CanonicalPath, (ItemId, usize)>,
}
```

This is sufficient for:

- item-level lookup
- field-level lookup

and removes the earlier inconsistency where field identities existed but the
lookup table indexed only items.

Verdict:
- clean

### 4. No fallback to short names

The current plan explicitly states that cross-reference uses:

- `CanonicalPath`

and that short names remain only for:

- display
- emission text

This is the correct boundary.

Verdict:
- clean

### 5. Provenance is not overloaded as identity

The plan states:

- `Provenance` is shared metadata
- `CanonicalPath` is the identity key

That is important because provenance alone is often too weak or too coarse to
serve as canonical linkage.

Verdict:
- clean

### 6. Scope of identity levels

The plan keeps identity scopes separated correctly:

- `CanonicalPath`:
  - cross-layer shared identity
- `ItemId`:
  - efficient intra-item-hierarchy reference
- `LocalId`:
  - function-local semantic-body identity

This separation is coherent and avoids accidental leakage of local identity into
cross-layer linkage.

Verdict:
- clean

## Minor Documentation Note

One small prose inconsistency remains:

- in the “Item Hierarchy carries canonical paths” bullet list, the plan lists:
  - `FunctionDef`
  - `StructDef`
  - `ImplDef`
  - `ConstantDef`
- but it does not mention `StructFieldDef` there, even though `StructFieldDef`
  does carry `canonical_path` and is used in the cross-link examples and the
  `CrossLinkTable`

This is not a design flaw. It is only a documentation completeness issue.

## Findings

The cross-link design is now strong because:

- identity is explicit
- item-level and field-level lookup are both covered
- short-name matching is no longer the architectural contract
- local identity and item identity remain properly separated

No new architectural blocker was found.

## Step 6 Verdict

Step 6 is **completed**.

Outcome:

- cross-layer linkage is coherent
- item-level and field-level resolution are both covered
- no fallback to short-name heuristics remains in the architecture
- only a minor documentation-completeness note remains
