# Invariant Validation

This note records Step 7 of the semantic procedural IR validation checklist.

The goal is to state the key invariants the design is supposed to guarantee and
check whether any of them are contradicted by the current proposal.

## Inputs

- final research plan at `~/.claude/plans/happy-crafting-muffin.md`
- [backend-boundary-validation.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/semantic-procedural-ir/backend-boundary-validation.md)
- [model-layer-coherence-validation.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/semantic-procedural-ir/model-layer-coherence-validation.md)
- [cross-link-validation.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/semantic-procedural-ir/cross-link-validation.md)

## Invariant Set

### 1. Every executable body belongs to exactly one `FunctionDef`

Expected meaning:
- semantic bodies are attached only to `FunctionDef`
- no executable body floats unowned in the model

Validation:
- the plan defines `FunctionDef { body: SemanticBody, ... }`
- no other top-level item type is given an executable-body slot

Verdict:
- satisfied

### 2. Every `FunctionDef` has exactly one structural origin

Expected meaning:
- every function is exactly one of:
  - free function
  - inherent method
  - trait method

Validation:
- `FunctionKind` is a closed enum
- no multi-kind or nullable kind state is proposed

Verdict:
- satisfied

### 3. Every `FunctionDef` has exactly one semantic role

Expected meaning:
- role is explicit and backend-dispatchable

Validation:
- `FunctionDef.role: FunctionRole`
- role is not optional
- `FunctionRole` is a closed enum in the current slice

Verdict:
- satisfied

### 4. `FunctionKind` and `FunctionRole` are orthogonal

Expected meaning:
- structural origin is not overloaded with backend semantics
- semantic role is not overloaded with ownership/origin

Validation:
- the plan explicitly separates them
- worked examples use both dimensions independently
- ISR is modeled as:
  - `FunctionKind::InherentMethod`
  - `FunctionRole::Isr { binding }`

Verdict:
- satisfied

### 5. Every semantic local has identity, name, type, and origin

Expected meaning:
- backends never receive anonymous or typeless locals

Validation:
- `SemanticLocal` includes:
  - `id`
  - `name`
  - `ty`
  - `origin`

Verdict:
- satisfied

### 6. Every cross-layer identity uses `CanonicalPath`

Expected meaning:
- no short-name matching is required for declaration/item cross-linking

Validation:
- the plan defines `CanonicalPath` as the authoritative cross-layer identity
- `CrossLinkTable` uses `CanonicalPath`
- item-level and field-level linkages both use canonical path

Verdict:
- satisfied

### 7. `ItemId` is hierarchy-local only

Expected meaning:
- `ItemId` is not reused as a cross-layer identity key

Validation:
- the plan explicitly says `ItemId` is intra-hierarchy only
- declaration/item linkage uses `CanonicalPath`, not `ItemId`

Verdict:
- satisfied

### 8. `LocalId` is function-local only

Expected meaning:
- local identity does not leak across functions

Validation:
- the plan explicitly defines `LocalId` as local to a single `SemanticBody`

Verdict:
- satisfied

### 9. Every backend-facing constant has deterministic emitted identity

Expected meaning:
- backends do not invent constant names

Validation:
- `ConstantDef.stable_name`
- `ConstantRef.stable_name`
- both are derived deterministically from `canonical_path`

Verdict:
- satisfied

### 10. Unsupported MIR constructs fail explicitly

Expected meaning:
- no silent semantic degradation

Validation:
- the plan explicitly states hard errors for unsupported MIR residue
- it explicitly forbids:
  - silent drops
  - zero defaults
  - TODO comments as lowering substitutes
  - debug-format interchange

Verdict:
- satisfied

### 11. Backends consume only `DriverLoweringPlan` plus OS-specific planning

Expected meaning:
- backends do not traverse raw model layers directly

Validation:
- the plan explicitly states:
  - backends consume only `DriverLoweringPlan` + OS-specific plan
- previous validation confirmed the boundary is coherent

Verdict:
- satisfied

### 12. Backends never receive MIR/Rust residue

Expected meaning:
- no raw MIR locals
- no basic blocks
- no wrapper paths
- no `core::*` control-flow machinery

Validation:
- the plan explicitly lists what backends never see
- semantic lowering is responsible for normalizing or rejecting residue

Verdict:
- satisfied, with one condition:
  - semantic lowering must strictly enforce the no-residue rule

### 13. `Opaque` is diagnostic-only, not backend-emittable

Expected meaning:
- unsupported types can be recorded diagnostically
- but they must not cross the backend boundary for supported emission

Validation:
- the plan explicitly states that `Opaque` is not for emission
- verification section requires no `Opaque` fallbacks for `pl011-full-spec`

Verdict:
- satisfied

### 14. ISR roots are structurally and semantically constrained

Expected meaning:
- not every inherent method can be an ISR root

Validation:
- ISR validation rules require:
  - driver-owned inherent method
  - `#[rseq::isr]`
  - matching declared interrupt binding

Verdict:
- satisfied

## Minor Documentation Inconsistency

One small inconsistency remains in the document:

- the shortened `FunctionDef` snippet in Section 3.2 omits `canonical_path`,
  while the full `FunctionDef` definition in Section 2.5 includes it

This does not break the design invariants because the authoritative type
definition includes `canonical_path`, but it is worth cleaning up for clarity.

## Findings

The invariants are in good shape. The design is strong because the important
compiler contracts are:

- explicit
- closed over named types/enums
- tied to the actual layering model

The most important operational caveat is still:

- semantic lowering must enforce these invariants before backend planning

That is not a new design gap, but it is an important implementation discipline
requirement.

## Step 7 Verdict

Step 7 is **completed**.

Outcome:

- the design supports a stable and defensible invariant set
- no invariant is contradicted by the current proposal
- no new architectural blocker was discovered
