# Paper Migration Validation

This note records Step 8 of the semantic procedural IR validation checklist.

The goal is to validate the migration plan as a pipeline transition on paper:

- where each new semantic-IR concept is produced
- what existing stage stops owning that concern
- whether any stage is assuming impossible recovery from older
  representations

This is a design-feasibility check, not an implementation.

## Inputs

- final research plan at `~/.claude/plans/happy-crafting-muffin.md`
- current pipeline code:
  - [emit_driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/src/emit_driver.rs)
  - [normalize.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/declaration/normalize.rs)
  - [input.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-analysis/src/input.rs)
  - [planner.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/lowering/planner.rs)
  - [plan.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/lowering/plan.rs)

## Current Canonical Pipeline

Today, the canonical production path is:

1. raw frontend artifact captured by `regseq-driver`
2. JSON normalization into `DeclarationModel`
3. platform parsing and binding normalization
4. join
5. `AnalysisInput::new(decl, joined, bus_binding)`
6. `build_driver_lowering_plan(&input)`
7. Linux backend planning
8. Linux artifact emission

This is visible in:
- [emit_driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/src/emit_driver.rs)

The current portable lowering stage constructs:
- `DriverLoweringPlan`
- with `behavior: BehaviorPlan`

from:
- `AnalysisInput`
- `DeclarationModel`
- joined artifact data
- MIR-derived bodies already normalized into `MirBodyDeclSet`

## Proposed Migration Stages

The design proposes:

1. add semantic IR types
2. build semantic IR directly from HIR + MIR
3. update Linux emitter to consume `SemanticModel`
4. remove `BehaviorPlan`

The question is whether each of these stages has a real information source in
the current pipeline.

## Stage-by-Stage Validation

### Stage 1: Introduce semantic IR types

Proposed result:

- `semantic_ir.rs` with:
  - item hierarchy
  - semantic body IR
  - semantic type universe
  - references and helper types
  - `SemanticModel`

Validation:
- pure type introduction
- no pipeline dependence

Verdict:
- feasible with no migration risk

### Stage 2: Build semantic IR directly from HIR + MIR

Proposed result:

- new `semantic_lowering` module producing `SemanticModel`

Expected inputs:

- HIR-derived declarative facts
- MIR body capture
- MIR interpreter context

#### 2A. MIR executable-body facts

Current availability:
- yes
- current normalized declaration model already carries:
  - `mir_bodies: MirBodyDeclSet`
  - each `MirFnBodyDecl` has:
    - `name`
    - `def_path`
    - `kind`
    - `receiver`
    - `params`
    - `return_type`
    - `body`
    - `helper_calls`

Visible in:
- [normalize.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/declaration/normalize.rs)
- [input.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-analysis/src/input.rs)

Verdict:
- feasible
- semantic body construction has a real current source of executable facts

#### 2B. HIR-derived function classification facts

Needed for:
- `FunctionKind`
- `FunctionRole`
- ISR classification
- trait provenance

Current availability:
- partial but real
- current declaration-side extraction already carries:
  - callback inventory
  - trait-family grouping
  - function kind in `MirFnBodyDecl.kind`
  - ISR markers

Verdict:
- feasible for the current slice
- enough exists to classify functions structurally and semantically for PL011

#### 2C. Item hierarchy facts (`Module`, `ImplDef`, item containment)

Needed for:
- full item hierarchy
- impl ownership
- logical source structure
- associated item grouping

Current availability:
- **not fully present yet in today’s normalized declaration schema**

Important current fact:
- current normalized declaration artifacts preserve many declaration facts
- but they do **not yet** preserve a first-class item hierarchy
- they also do **not yet** carry canonical paths for every non-body item and
  field in the full way the design expects

Concrete evidence:
- `MirFnBodyDecl` already has `def_path`
- but `DeclarationModel` today is still primarily construct-family-indexed
- the plan’s `Module/Item/ImplDef/StructFieldDef.canonical_path` layer does not
  already exist in normalized artifacts

Validation result:
- this is **not** an impossible recovery problem
- but it **is** a real migration precondition

Required precondition:
- extend frontend extraction and/or normalization so the data needed for the
  item hierarchy and canonical cross-links is actually captured before semantic
  lowering tries to build `Module/Item/...`

Verdict:
- feasible
- but only if the migration plan explicitly includes extraction/schema
  enrichment before or alongside semantic lowering

#### 2D. Can `BehaviorPlan` be used as the main bridge?

Current plan says no.

Validation:
- correct
- current `BehaviorPlan` does not carry:
  - typed locals
  - local naming
  - structural function ownership
  - full constant identity
  - option-carrier semantics

So any main migration path that depends on recovering these from
`BehaviorPlan` would be unsound.

Verdict:
- the plan is correct to reject `BehaviorPlan -> SemanticModel` as the primary
  migration path

### Stage 3: Update Linux emitter to consume `SemanticModel`

Proposed result:

- `emit_behavior.rs` walks `Stmt` / `Expr`
- `emit_module.rs` uses `SemanticModel.functions`

Current availability:
- the Linux emitter already sits behind:
  - `DriverLoweringPlan`
  - Linux backend plan
- so swapping the behavior field from `BehaviorPlan` to `SemanticModel` is a
  real structural migration point

Validation:
- feasible
- the current emitter already consumes a compiler-owned portable behavior field;
  it does not have to be redesigned from scratch

Verdict:
- feasible

### Stage 4: Remove `BehaviorPlan`

Proposed result:

- old behavior IR deleted
- `DriverLoweringPlan.behavior` becomes `SemanticModel`

Validation:
- feasible in principle
- but only after Stage 2 and Stage 3 are complete

Important sequencing note:
- tests that currently assert on `BehaviorPlan` must migrate with the new IR
- the current `pl011_full_spec_lowering` and related backend tests will need
  semantic-IR replacements, not just type renames

Verdict:
- feasible

## Current Pipeline vs Proposed Pipeline

### Current

```text
RawFrontendArtifact
  -> DeclarationModel
  -> AnalysisInput
  -> DriverLoweringPlan { behavior: BehaviorPlan }
  -> LinuxBackendPlan
  -> emitted artifacts
```

### Proposed

```text
RawFrontendArtifact
  -> DeclarationModel
  -> Item Hierarchy + SemanticModel   (new semantic lowering path)
  -> AnalysisInput / lowering context
  -> DriverLoweringPlan { behavior: SemanticModel }
  -> LinuxBackendPlan
  -> emitted artifacts
```

Important clarification:
- the proposed item hierarchy is not recoverable from current `BehaviorPlan`
- it must be produced upstream from enriched HIR/declaration extraction plus
  MIR body elaboration

## Migration Findings

### What is feasible without concern

- introducing semantic IR types
- building semantic bodies from current MIR capture
- replacing backend consumption of `BehaviorPlan`
- removing `BehaviorPlan` once the new path exists

### What requires an explicit migration precondition

The migration plan needs to say more explicitly that:

- current extraction/normalization must be enriched to carry the full item
  hierarchy and canonical paths required by the new model

Why:
- today’s pipeline already has enough MIR body information
- but it does not yet have the complete item-structured declaration face that
  the new hierarchy assumes

This is not a design flaw in the semantic IR.
It is a migration-shaping requirement.

### What does *not* appear to rely on impossible recovery

The plan does **not** currently depend on:

- recovering typed locals from `BehaviorPlan`
- recovering canonical item identity from backend plans
- asking Linux to infer semantic ownership or MIR meaning

That is good and is exactly what this validation step was meant to test.

## Recommendation

Before implementation tasks are created, the migration plan should explicitly
state one additional precondition:

- extend extraction/normalization so the item hierarchy inputs
  (`Module`, `ImplDef`, item containment, canonical paths for relevant items and
  fields) are available before semantic lowering begins

That keeps the migration story honest and prevents Stage 2 from appearing to
manufacture structure from data the current normalized model does not yet carry.

## Step 8 Verdict

Step 8 is **completed**.

Outcome:

- the migration plan is broadly feasible
- it does not rely on impossible reconstruction from `BehaviorPlan`
- one real precondition was identified:
  - upstream extraction/normalization must be enriched to provide the full item
    hierarchy inputs the new model expects
