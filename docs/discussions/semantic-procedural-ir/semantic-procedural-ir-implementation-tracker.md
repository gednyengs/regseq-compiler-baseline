# Semantic Procedural IR Implementation Tracker

This document tracks the progressive implementation flow for the semantic
procedural IR described in [plan.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/semantic-procedural-ir/plan.md).

It is intentionally separate from the design-validation checklist:

- [semantic-procedural-ir-validation-checklist.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/semantic-procedural-ir/semantic-procedural-ir-validation-checklist.md)
  proves the design is coherent on paper
- this tracker governs how we implement it step by step without letting later
  phases patch around earlier model gaps

## Purpose

The implementation must proceed progressively, with explicit review stops after
each phase.

The core rule is:

- introduce one new layer of truth at a time
- inspect it directly
- validate it in isolation
- only then allow downstream phases to depend on it

This is how we avoid repeating the earlier failure mode where the Linux backend
was forced to compensate for missing semantic lowering.

## Status Values

- `not started`
- `in progress`
- `completed`
- `blocked`

## Global Rules

These rules apply to every phase:

- do not let backend code reconstruct missing semantics from raw MIR/HIR residue
- do not accept short-name fallbacks where canonical identity is required
- do not advance phases when a newly discovered model gap is still unresolved
- when a phase surfaces a design problem, stop and revise the model/plan first
- attach concrete evidence artifacts before marking a phase complete

## Phase Tracker

| Phase | Name | Status | Primary Output | Gate / Evidence |
|------|------|--------|----------------|-----------------|
| 0 | Extraction Enrichment | not started | enriched extraction data with canonical identities and ownership facts | extraction artifact review |
| 1 | Semantic IR Type Introduction | not started | semantic IR/item hierarchy types compile in tree | type review against `plan.md` |
| 2A | Item Hierarchy Construction | not started | inspectable `Module` / `ItemStore` / `FunctionDef` / `ImplDef` / `ConstantDef` graph | hierarchy dump + ownership validation |
| 2B | Semantic Body Lowering | not started | readable `SemanticBody` artifacts for representative functions | semantic-body review + validation pass |
| 2C | SemanticModel Projection | not started | backend-facing `SemanticModel` projection | projection review against hierarchy |
| 3 | Linux Backend Migration | not started | Linux emitter consumes only `SemanticModel` | emitted C review + backend-local tests |
| 4 | Canonical Validation / Host Build Proof | not started | canonical wrapper-driven proof rerun | host-build proof artifacts |

## Phase 0: Extraction Enrichment

### Goal

Enrich upstream extraction and normalization so the compiler actually carries
the identity and ownership facts the semantic model expects.

### Scope

- `CanonicalPath` for:
  - functions
  - impls
  - constants
  - structs
  - relevant fields
- enough HIR ownership data for:
  - modules
  - impls
  - associated constants
  - ISR markers
- resource field identity
- register-block field identity

### Non-Goals

- no semantic body lowering yet
- no Linux emitter changes yet
- no `BehaviorPlan` replacement yet

### Required Artifacts

- extraction dump or equivalent debug artifact showing:
  - canonical paths
  - module/impl ownership
  - associated-const ownership
  - field identities
- short write-up explaining where each new fact comes from

### Gate

Do not proceed until:

- the extraction output visibly contains the needed identity and ownership facts
- no required semantic-IR cross-link still depends on short-name matching

### Notes / Findings

- pending

## Phase 1: Semantic IR Type Introduction

### Goal

Introduce the new model and IR types without changing lowering behavior yet.

### Scope

- item hierarchy types
- semantic body types
- semantic refs
- semantic model container
- supporting enums and ids

### Non-Goals

- no production lowering into these types yet
- no backend migration yet

### Required Artifacts

- compiling type definitions in-tree
- review diff against [plan.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/semantic-procedural-ir/plan.md)
- explicit record of any deliberate deviations from the plan

### Gate

Do not proceed until:

- the types compile cleanly
- the definitions match the approved plan
- no ad hoc deviations are introduced without explicit review

### Notes / Findings

- pending

## Phase 2A: Item Hierarchy Construction

### Goal

Construct the logical Layer 2 item hierarchy before full semantic-body lowering.

### Scope

- `Module`
- canonical `ItemStore`
- `FunctionDef`
- `ImplDef`
- `ConstantDef`
- minimal `StructDef`
- ownership / grouping links

### Non-Goals

- bodies may still be stubbed or minimally populated
- no backend-facing emission use yet

### Required Artifacts

- item hierarchy dump for `pl011-full-spec`
- ownership/cross-link report showing:
  - impl to methods
  - impl to associated consts
  - field-level canonical paths
  - item-level canonical paths

### Gate

Do not proceed until:

- the hierarchy is inspectable and internally consistent
- `Module.items`, `ItemStore`, and `ItemId` usage are coherent
- ownership and identity work without short-name recovery

### Notes / Findings

- pending

## Phase 2B: Semantic Body Lowering

### Goal

Lower representative MIR bodies into readable, validated `SemanticBody` IR.

### Recommended Rollout Order

Start with:

1. `uart_clock_hz`
2. `set_baud_rate`
3. `init`

Then expand to:

4. `service_tx_fifo`
5. `service_rx_fifo`
6. `handle_irq`
7. resource helpers

This order moves from value/resource patterns to option-carrier/stateful bodies
and finally ISR dispatch.

### Scope

- locals
- control flow
- option carriers
- fallible flow
- register/resource ops
- interrupt tests
- ordered regions

### Required Artifacts

- per-function semantic body dumps for the representative set
- semantic validation output covering:
  - no unresolved refs
  - no unsupported `Opaque` in supported bodies
  - `OptionUnwrap` domination holds
  - no stringly lowering-critical refs remain

### Gate

Do not proceed until:

- representative bodies are readable and semantically plausible
- validation passes succeed on the supported slice
- any newly discovered model gap has been either:
  - fixed in the design and implementation, or
  - explicitly declared unsupported with a hard error

### Notes / Findings

- pending

## Phase 2C: SemanticModel Projection

### Goal

Project the backend-facing `SemanticModel` from the populated item hierarchy.

### Scope

- function projection
- constant projection
- interrupt class projection
- backend-consumable behavioral projection only

### Required Artifacts

- `SemanticModel` dump for `pl011-full-spec`
- short review note proving projection does not lose required information

### Gate

Do not proceed until:

- `SemanticModel` contains all behavioral data Linux needs
- projection does not force backend-side semantic reconstruction

### Notes / Findings

- pending

## Phase 3: Linux Backend Migration

### Goal

Make the Linux backend consume only the new semantic model and stop relying on
`BehaviorPlan`.

### Scope

- rewrite behavior emission around `Stmt` / `Expr`
- consume typed locals
- consume structured refs
- consume stable constant identities

### Non-Goals

- do not patch around missing semantic lowering in Linux
- do not accept backend-local re-derivation of wrapper or MIR meaning

### Required Artifacts

- emitted `pl011-full-spec` Linux C from the new path
- backend-local test results
- code review note confirming Linux is not reconstructing semantics

### Gate

Do not proceed until:

- emitted C is structurally valid and reviewable
- backend-local tests pass
- no Linux-specific ad hoc fallback has been introduced to compensate for
  missing semantic IR fidelity

### Notes / Findings

- pending

## Phase 4: Canonical Validation / Host Build Proof

### Goal

Rerun the canonical wrapper-driven validation once Linux is fully on the new IR.

### Scope

- canonical `RUSTC_WRAPPER` / `EmitSpec::Driver` path
- fresh output
- deterministic hashes
- out-of-tree host build
- retained proof artifacts

### Required Artifacts

- proof log
- emitted artifact hashes
- build log
- resulting `.ko`
- `modinfo`

### Gate

Complete only when:

- emission succeeds
- host build succeeds
- proof artifacts are retained
- the resulting output is reviewable and free of the earlier semantic-lowering
  failure class

### Notes / Findings

- pending

## Review Hold Points

These are mandatory review stops. No skipping.

1. after Phase 0
2. after Phase 1
3. after Phase 2A
4. after the first narrow subset of Phase 2B (`uart_clock_hz`, `set_baud_rate`, `init`)
5. after full Phase 2B representative coverage
6. after Phase 2C
7. after Linux emits first end-to-end readable C from the new IR
8. after the canonical host-build proof

## Evidence Expectations

Each completed phase should point to concrete evidence, not just status prose.

Preferred evidence types:

- dumps/artifacts checked into the repo or attached in task output
- validation reports
- targeted test output
- reviewer notes referencing exact files/artifacts

## Blocker Log

Use this section to record design or implementation blockers discovered during
execution.

### Blocker Template

- `phase`:
- `date`:
- `symptom`:
- `root cause`:
- `is this a design gap or implementation bug?`:
- `required change`:
- `evidence`:
- `resolved?`:

### Active Blockers

- none recorded

## Current Assessment

At the time this tracker was created:

- the semantic-IR design had already passed the paper validation checklist
- the plan had been tightened through multiple review/subagent passes
- the implementation recommendation was to begin with Phase 0, not with backend
  rewriting

That is the baseline. Update this section if the implementation strategy
changes materially.
