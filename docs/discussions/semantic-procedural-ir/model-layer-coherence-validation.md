# Model-Layer Coherence Validation

This note records Step 5 of the semantic procedural IR validation checklist.

The goal is to validate that each major concern in the proposed architecture has
exactly one authoritative layer, and that any cross-layer relationship is an
explicit bridge rather than overlapping ownership.

## Inputs

- final research plan at `~/.claude/plans/happy-crafting-muffin.md`
- [backend-boundary-validation.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/semantic-procedural-ir/backend-boundary-validation.md)
- [ownership-and-identity-stress-test.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/semantic-procedural-ir/ownership-and-identity-stress-test.md)

## Layer Set

The plan defines four layers:

1. `DeclarationModel`
2. Item Hierarchy
3. `DriverLoweringPlan`
4. OS Backend Plan

The semantic procedural IR is attached to `FunctionDef.body` inside the Item
Hierarchy but is treated as its own semantic authority for executable body
meaning.

For validation purposes, we therefore use these five authority buckets:

1. `DeclarationModel`
2. Item Hierarchy
3. Semantic Procedural IR
4. `DriverLoweringPlan`
5. OS Backend Plan

## Authority Matrix

| Concern | Authoritative Layer | Validation Result |
|---------|---------------------|-------------------|
| Resource obligations | `DeclarationModel` | clean |
| Callback inventory | `DeclarationModel` | clean |
| Register block layouts and bitfields | `DeclarationModel` | clean |
| Property declarations and defaults | `DeclarationModel` | clean |
| Item identity | Item Hierarchy | clean |
| Item containment | Item Hierarchy | clean |
| Impl ownership | Item Hierarchy | clean |
| Type definitions | Item Hierarchy | clean |
| Constant identity | Item Hierarchy | clean |
| Executable body semantics | Semantic Procedural IR | clean |
| Semantic function role | Semantic Procedural IR | clean |
| Backend-facing portable obligations | `DriverLoweringPlan` | clean |
| Acquisition / teardown ordering | `DriverLoweringPlan` | clean |
| Device-class role obligations | `DriverLoweringPlan` | clean |
| Portable behavioral plan exposed to backends | `DriverLoweringPlan` carrying `SemanticModel` | clean |
| OS API mapping | OS Backend Plan | clean |
| Emitted file layout / registration shape | OS Backend Plan | clean |

## Validation Observations

### 1. `DeclarationModel` is not overreaching

`DeclarationModel` owns:

- construct-family obligations
- callback inventory
- register declarations
- property declarations

It does **not** claim to own:

- logical item containment
- executable body semantics
- backend-ready portable sequencing
- emitted layout

Verdict:
- coherent

### 2. Item Hierarchy is not being asked to do obligation planning

The Item Hierarchy owns:

- items
- modules
- impl grouping
- type definitions
- constants
- structural function ownership

It does **not** claim to own:

- total resource obligation sets
- acquisition / teardown ordering
- device-class obligation planning

Verdict:
- coherent

### 3. Semantic Procedural IR has a clean authority scope

Semantic Procedural IR owns:

- what function bodies mean
- local/value/control-flow semantics
- semantic function role

It does **not** claim to own:

- emitted C syntax
- OS-specific API calls
- file partitioning

Verdict:
- coherent

### 4. `DriverLoweringPlan` is acting as a bridge, not a duplicate model

`DriverLoweringPlan` is the point where:

- declarative obligations
- semantic bodies

meet for backend consumption.

It does not try to replace:

- the declaration layer
- the item hierarchy
- the semantic IR

Instead, it packages the backend-facing subset of those layers.

Verdict:
- coherent

### 5. OS Backend Plan stays on realization, not semantic recovery

The OS Backend Plan owns:

- OS-specific API mapping
- registration/probe/remove realization
- file layout
- emitted artifact organization

It does **not** claim to own:

- wrapper normalization
- body meaning
- item identity
- obligation discovery

Verdict:
- coherent

## Bridge Analysis

There are two intentional bridges:

### Bridge 1: `DeclarationModel` <-> Item Hierarchy

Bridge mechanism:

- `CanonicalPath`
- `CrossLinkTable`

Purpose:

- align construct-family declaration facts with logical items and fields

This is an explicit bridge, not overlapping authority.

### Bridge 2: Item Hierarchy / Semantic IR -> `DriverLoweringPlan`

Bridge mechanism:

- `SemanticModel` projection into `DriverLoweringPlan.behavior`

Purpose:

- expose backend-consumable semantic bodies without giving backends direct
  traversal access to the internal item hierarchy

This is also an explicit bridge, not overlapping authority.

## Potential Ambiguity Check

Question:
- is `FunctionRole` owned by the Item Hierarchy or by the Semantic IR?

Answer:
- the plan says `FunctionRole` is semantic, not structural
- it is stored on `FunctionDef`, but semantically belongs to the executable-body
  / backend-dispatch interpretation of the function

This is acceptable as long as we preserve the conceptual distinction:
- `FunctionDef` is the container
- `FunctionRole` remains part of semantic meaning, not item-ownership facts

Verdict:
- acceptable, not a blocker

## No Overlap Findings

This validation did not find a concern that is jointly authoritative across two
layers without an explicit bridge.

The design is strongest in these areas:

- declaration facts vs item structure
- semantic meaning vs backend realization
- portable plan vs OS-specific plan

## Step 5 Verdict

Step 5 is **completed**.

Outcome:

- the layer architecture is coherent
- each major concern has one authoritative layer
- the cross-layer relationships are explicit bridges rather than accidental
  overlap
- no new architectural blocker was discovered
