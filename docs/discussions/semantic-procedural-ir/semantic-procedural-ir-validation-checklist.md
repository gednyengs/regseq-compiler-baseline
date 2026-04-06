# Semantic Procedural IR Validation Checklist

This document defines how to validate the semantic procedural IR design in
`~/.claude/plans/happy-crafting-muffin.md` before implementation begins.

The goal is to validate the design as a compiler architecture, not just as a
set of plausible type definitions.

## Purpose

Before implementation, we need to prove that the proposed model:

- can represent the supported Regseq driver slice faithfully
- fully decouples backends from MIR/Rust residue
- has a coherent ownership and cross-link story
- provides enough information for Linux and Zephyr backends without backend-side
  semantic reconstruction

This validation should be done on paper and against real source artifacts before
we write code.

## Status Tracker

Use this tracker to carry out the validation one step at a time.

Status values:

- `not started`
- `in progress`
- `completed`
- `blocked`

| Step | Validation Area | Status | Notes / Evidence |
|------|-----------------|--------|------------------|
| 1 | Representative Function Trace Validation | completed | `representative-function-trace-validation.md`; surfaced two design gaps: explicit optional-value presence tests and explicit option-like carrier values |
| 2 | HIR/MIR Coverage Matrix | completed | `hir-mir-coverage-matrix.md`; confirms the option-carrier issue is a real semantic-IR gap: missing general optional-value presence tests and explicit option-like carrier values |
| 3 | Ownership and Identity Stress Test | completed | `ownership-and-identity-stress-test.md`; ownership and identity model holds under overlapping names, associated items, field-level collisions, and non-driver helper ownership; no new blocker found |
| 4 | Backend Boundary Validation | completed | `backend-boundary-validation.md`; boundary is clean for the supported slice, provided semantic lowering strictly blocks `Opaque` types and unresolved semantic residue from reaching backend consumption |
| 5 | Model-Layer Coherence Validation | completed | `model-layer-coherence-validation.md`; each major concern has one authoritative layer and cross-layer relationships are explicit bridges rather than overlapping ownership |
| 6 | Cross-Link Validation | completed | `cross-link-validation.md`; item-level and field-level canonical-path linkage are both covered, and the architecture no longer depends on short-name matching |
| 7 | Invariant Validation | completed | `invariant-validation.md`; the design supports a stable invariant set, and no proposed invariant is contradicted by the current hierarchy, semantic IR, or layering model |
| 8 | Paper Migration Validation | completed | `paper-migration-validation.md`; migration is broadly feasible and does not rely on impossible recovery from `BehaviorPlan`, but it does require upstream extraction/normalization enrichment for full item-hierarchy inputs and canonical paths |
| 9 | Negative Validation | completed | `negative-validation.md`; key historical failure modes now either have explicit semantic representations or explicit hard-error behavior; no new blocker found |
| 10 | Acceptance Example Set | completed | `acceptance-example-set.md`; defines the concrete “must work on paper” semantic contracts for `uart_clock_hz`, `set_baud_rate`, `handle_irq`, `init`, and `service_tx_fifo` |

Recommended execution order:

1. Representative Function Trace Validation
2. HIR/MIR Coverage Matrix
3. Ownership and Identity Stress Test
4. Cross-Link Validation
5. Model-Layer Coherence Validation
6. Backend Boundary Validation
7. Invariant Validation
8. Paper Migration Validation
9. Negative Validation
10. Acceptance Example Set

At the end of each step:

- update the `Status` column
- record the evidence produced
- list any newly discovered design gaps before moving to the next step

## Validation Axes

### 1. Representative Function Trace Validation

Take real `pl011-full-spec` functions and manually lower them into the proposed
model.

Required sample set:

- `init`
- `handle_irq`
- `set_baud_rate`
- `uart_clock_hz`
- `service_rx_fifo`
- `service_tx_fifo`
- resource helpers:
  - clock helper
  - reset helper
  - pinctrl helper

For each sample, validate:

- the containing item hierarchy is unambiguous
- `FunctionKind` is correct
- `FunctionRole` is correct
- all parameters, locals, derived temps, and return values have a valid
  representation
- all control flow has a valid representation
- all register/resource/intrinsic behavior has a valid semantic representation
- no raw MIR locals or Rust paths are needed in the final semantic body

Pass condition:

- every representative function can be lowered into the proposed item hierarchy
  and `SemanticBody` without requiring ad hoc escape hatches

### 2. HIR/MIR Coverage Matrix

Build a matrix of in-scope source constructs and their semantic-model targets.

The matrix must cover:

- HIR/module-level constructs:
  - module
  - struct
  - enum
  - impl
  - free function
  - method
  - associated const
  - type alias
- MIR/function-body constructs:
  - parameters
  - authored locals
  - derived temps
  - return place
  - assignment
  - call
  - branch
  - loop
  - fallible flow / `?`
  - register read/write/modify
  - resource operations
  - resource property reads
  - constant references
  - interrupt class test
  - interrupt clear behavior
  - ordered regions

For each row in the matrix, record:

- source construct
- extraction source:
  - HIR
  - MIR
  - both
- semantic-model construct
- unsupported behavior, if any
- required hard-error rule if unsupported

Pass condition:

- every in-scope construct maps to an explicit semantic-model construct or an
  explicit hard error
- no row ends with "backend infers this"

### 3. Ownership and Identity Stress Test

Validate the ownership model against ambiguity cases.

Test cases:

- two different types with same short method name
- multiple impls containing methods with overlapping names
- associated constants with same short name on different owners
- identical short field names on different structs
- helper methods on non-driver helper types
- multi-module-qualified item names, even if multi-module support is deferred

Validate that the model can resolve these cases using:

- `CanonicalPath`
- `ItemId`
- `ImplDef`
- field-level canonical paths
- `CrossLinkTable`

Pass condition:

- all resolution works without short-name heuristics

### 4. Backend Boundary Validation

Validate that the backend boundary is actually clean.

The design must prove that backends receive:

- typed locals
- typed return values
- explicit statement/expression trees
- semantic register operations
- semantic resource operations
- semantic resource reads
- stable constant identities and stable emitted names

The design must also prove that backends never see:

- raw MIR locals like `_3`
- basic block indices
- Rust wrapper paths like `regseq::ReadWrite::<...>::modify`
- `core::option::Option` call spellings
- `core::ops::Try` spellings
- raw rustc type spellings
- ownership/grouping facts that must be reconstructed from names

Pass condition:

- a backend can emit code from `DriverLoweringPlan` without re-walking raw model
  data or reconstructing Rust semantics

### 5. Model-Layer Coherence Validation

Validate that each major concern has exactly one authoritative layer.

Required layers:

- `DeclarationModel`
- Item Hierarchy
- Semantic Procedural IR
- `DriverLoweringPlan`
- OS Backend Plan

For each concern, record the authoritative layer:

- resource obligations
- callback inventory
- register layouts
- property defaults
- item identity
- item containment
- impl ownership
- constant identity
- executable body semantics
- backend-facing portable obligations
- OS-specific API realization
- emitted file layout

Pass condition:

- every concern has one authoritative layer
- no concern is jointly authoritative in two layers without an explicit bridge

### 6. Cross-Link Validation

Validate the `DeclarationModel <-> Item Hierarchy` linkage design directly.

Required checks:

- all item-level links use `CanonicalPath`
- all field-level links use `CanonicalPath`
- `ItemId` is hierarchy-local only
- `LocalId` is function-local only
- provenance is supplementary metadata, not the primary identity key
- `CrossLinkTable` is sufficient for both item-level and field-level lookups

Specific examples to validate:

- callback method lookup
- register block lookup
- associated constant lookup
- resource field lookup

Pass condition:

- all cross-links can be described without reverting to short-name matching

### 7. Invariant Validation

Write down the invariants and verify that the design satisfies them.

Required invariants:

- every executable body belongs to exactly one `FunctionDef`
- every `FunctionDef` has exactly one `FunctionKind`
- every `FunctionDef` has exactly one `FunctionRole`
- `FunctionKind` and `FunctionRole` are orthogonal
- every semantic local has:
  - identity
  - name
  - type
  - origin
- every cross-layer identity uses `CanonicalPath`
- every backend-facing constant has a deterministic `stable_name`
- every unsupported MIR construct fails explicitly
- backends consume only `DriverLoweringPlan` plus OS-specific planning
- backends do not traverse `DeclarationModel` or the Item Hierarchy directly

Pass condition:

- no invariant is contradicted by the type definitions or layering model

### 8. Paper Migration Validation

Walk the existing compiler pipeline and annotate where each proposed concept is
produced and consumed.

Pipeline to validate:

1. HIR extraction
2. MIR extraction
3. item hierarchy construction
4. semantic lowering
5. `DriverLoweringPlan` construction
6. OS backend planning
7. emission

For each stage, record:

- what new data it produces
- what prior-layer data it consumes
- what old representation it replaces
- whether any information is assumed to be recoverable later

Pass condition:

- no stage depends on recovering missing semantic information from
  `BehaviorPlan`
- no stage says "backend can infer this later"

### 9. Negative Validation

Try to break the design with previously observed failure modes.

Required negative cases:

- duplicate short method names on different owners
- overlapping associated constant names
- overlapping resource field names
- value-producing helper with default path
- ISR with multiple interrupt-class tests and explicit clear behavior
- helper with multiple derived arithmetic temporaries
- fallible helper with cleanup path

Pass condition:

- the design still has a precise representation for each case
- no fallback depends on TODO comments, debug formatting, or heuristic name
  matching

### 10. Acceptance Example Set

Before implementation, declare a small set of semantic examples that the design
must support cleanly.

Required examples:

- `uart_clock_hz`
  - `ResourcePropertyRead + WithDefault`
- `set_baud_rate`
  - typed locals + arithmetic + register writes
- `handle_irq`
  - interrupt tests + accumulated result + explicit clear
- `init`
  - fallible helper sequencing
- `service_tx_fifo`
  - state update + helper predicates + register write

Pass condition:

- each example has a written semantic lowering that uses only the proposed model

## Required Deliverables

Before implementation starts, the design validation should produce:

1. A representative-function trace set
2. A HIR/MIR coverage matrix
3. An ownership and identity stress-test note
4. A model-layer authority table
5. A cross-link validation note
6. A written invariant list
7. A paper migration walkthrough
8. A negative-case analysis
9. A semantic acceptance-example set

## Exit Criteria

The design is ready for implementation only when all of the following are true:

- every in-scope HIR and MIR construct has an explicit semantic target or a hard
  error
- cross-layer identity is unambiguous and does not rely on short names
- representative `pl011-full-spec` functions lower without semantic gaps
- the backend boundary is clean and Rust/MIR-free
- no major concern lacks an authoritative layer
- the migration story does not rely on recovering missing semantics from the
  current `BehaviorPlan`

If any of those conditions fail, the design still needs refinement before code
work begins.
