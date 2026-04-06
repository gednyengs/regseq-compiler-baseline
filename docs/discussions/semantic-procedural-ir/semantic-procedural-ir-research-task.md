# Semantic Procedural IR Research Task

## Purpose

This document defines a focused research task for designing the next internal
Regseq model boundary:

- a well-structured Regseq item hierarchy
- a backend-facing semantic procedural IR for executable bodies
- a clear split between HIR-derived declarative structure and MIR-derived
  executable semantics

This is a research/design exploration task, not an implementation task.

The intended consumer is a researcher or LLM-based research agent that can:

- inspect the existing codebase
- inspect the discussion documents
- inspect the current MIR capture shape
- propose a structured model design for later refinement

## Problem Statement

The current compiler has:

1. rustc MIR capture for executable bodies
2. a rustc-independent MIR model
3. a partial portable behavior model
4. Linux-specific backend planning and emission

However, the current backend-facing boundary is still too weak.

As a result:

- MIR-local structure still leaks too far toward backend emission
- wrapper-operation residue still leaks too far toward backend emission
- value-returning helpers are not fully represented in the portable model
- helper/ISR bodies can still become malformed in emitted target code

The latest `regseq-linux-backend-02` rejection is evidence that:

- the compiler now needs a stronger internal semantic model
- backend code generators should consume a Regseq-owned semantic procedural IR,
  not half-lowered MIR/Rust constructs

The design goal is therefore:

- preserve source/logical structure in the internal Regseq model
- derive executable semantics from MIR bodies
- normalize Rust/MIR-specific constructs into Regseq-owned semantics before
  backend planning and emission
- keep the design structurally friendly to current C-based backends (Linux and
  Zephyr) without making the internal model itself Linux-shaped or Zephyr-shaped

## Existing Discussion Material To Read

Read these first:

- [semantic-procedural-ir-discussion.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/semantic-procedural-ir/semantic-procedural-ir-discussion.md)
- [pl011-full-spec-mir-study.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/semantic-procedural-ir/pl011-full-spec-mir-study.md)
- [lowering-plan-interface-boundary.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/linux-codegen/lowering-plan-interface-boundary.md)
- [regseq-intrinsics-discussion.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/regseq-intrinsics/regseq-intrinsics-discussion.md)
- [register-access-discussion.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/register-access/register-access-discussion.md)

## Key Code To Inspect

### Current MIR capture and raw model

- [crates/regseq-driver/src/extract/body.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/src/extract/body.rs)
- [crates/regseq-driver/src/extract/mir_extract.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/src/extract/mir_extract.rs)
- [crates/regseq-model/src/declaration/mir_body.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/declaration/mir_body.rs)

### Current portable lowering / behavior model

- [crates/regseq-backends/src/lowering/behavior.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/lowering/behavior.rs)
- [crates/regseq-backends/src/lowering/plan.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/lowering/plan.rs)
- [crates/regseq-backends/src/lowering/mir_interpreter/context.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/lowering/mir_interpreter/context.rs)
- [crates/regseq-backends/src/lowering/mir_interpreter/place_resolver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/lowering/mir_interpreter/place_resolver.rs)
- [crates/regseq-backends/src/lowering/mir_interpreter/call_classifier.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/lowering/mir_interpreter/call_classifier.rs)
- [crates/regseq-backends/src/lowering/mir_interpreter/value_lowering.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/lowering/mir_interpreter/value_lowering.rs)
- [crates/regseq-backends/src/lowering/mir_interpreter/cfg_helpers.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/lowering/mir_interpreter/cfg_helpers.rs)
- [crates/regseq-backends/src/lowering/mir_interpreter/cfg_walker.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/lowering/mir_interpreter/cfg_walker.rs)

### Current Linux-facing emission layer

- [crates/regseq-backends/src/linux/emit_behavior.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/linux/emit_behavior.rs)
- [crates/regseq-backends/src/linux/emit_module.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/linux/emit_module.rs)
- [crates/regseq-backends/src/linux/c_ast.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/linux/c_ast.rs)

### Current canonical failure evidence

- [tests/fixtures/pl011-full-spec/regseq-out/regseq_pl011_full_spec_module.c](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/regseq-out/regseq_pl011_full_spec_module.c)
- [tests/e2e/tests/pl011_driver_emission.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/e2e/tests/pl011_driver_emission.rs)
- [regseq-linux-backend-02 task spec](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/.harness/tasks/regseq-linux-backend/02-pl011-linux-driver-emission-for-supported-slice.md)

### Source fixture that anchors the MIR study

- [tests/fixtures/pl011-full-spec/src/driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)
- [tests/fixtures/pl011-full-spec/expected/raw-frontend.json](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/expected/raw-frontend.json)

### Local rustc source references

- [rustc_hir/src/hir.rs](/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_hir/src/hir.rs)
- [rustc_hir/src/def.rs](/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_hir/src/def.rs)
- [rustc_middle/src/mir/syntax.rs](/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_middle/src/mir/syntax.rs)
- [rustc_middle/src/mir/statement.rs](/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_middle/src/mir/statement.rs)
- [rustc_middle/src/mir/terminator.rs](/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_middle/src/mir/terminator.rs)
- [rustc_mir_transform/src/lib.rs](/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_mir_transform/src/lib.rs)

## Scope Boundaries

### Rust item kinds in scope for this research

We are currently interested in function-like executable bodies only:

- free functions
- methods

For this research pass:

- closures are out of scope as semantic-body inputs
- const/statics/promoted consts/inline consts are out of scope as body-bearing
  semantic items, though their values may still matter as support data

### Regseq top-level item modeling questions in scope

The research should explicitly address which Rust-level top-level items become
first-class Regseq model items.

At minimum, discuss:

- Module
- Struct
- Enum
- Constant
- Function
- Impl
- Trait
- TypeAlias

It is acceptable to recommend a narrower first implementation scope, but the
hierarchy should be explicit.

### HIR scope

HIR should be treated as the declarative/source-structure extraction layer.

Research what information should be derived from HIR for the Regseq model, such
as:

- module and item hierarchy
- driver identity and metadata
- type definitions and names
- trait definitions / recognized trait references
- impl ownership and method grouping
- free-function ownership
- source-level naming/provenance
- associated constants and their ownership relationships

### MIR scope

MIR should be treated as the executable-body extraction layer.

Research what semantic information should be derived from function/method MIR
bodies, including at least:

- locals and local identity
- parameters and return place
- statements
- terminators
- operands
- rvalues
- places and projections
- constants / associated-const references
- unwind/cleanup information

The research should explicitly address which MIR constructs should survive into
the semantic IR and which should be normalized away before backend planning.

## Design Direction Constraints

The research should assume the following design constraints unless it presents a
strong argument otherwise:

1. The internal Regseq model should preserve **logical source structure**.
   - Modules are logical namespaces, not emitted OS files.
   - Backends remain free to choose completely different physical file layouts.

2. The backend-facing semantic IR should be:
   - Regseq-owned
   - Rust/MIR-independent in surface form
   - structurally friendly to current C-based backends
   - not Linux-shaped or Zephyr-shaped

3. Wrapper-based extraction is acceptable for now.
   - The research may assume wrapper surfaces such as
     `ReadWrite::write/modify/read` are valid recognized semantic surfaces.
   - The research should not assume a full intrinsic redesign is already in
     place.

4. The model may be structurally C-like in hierarchy and procedure shape.
   - This means explicit modules/items/functions/locals/statements/expressions
     are encouraged.
   - This does **not** mean the model should collapse into raw C syntax or raw
     OS-specific semantics.

5. The research should prefer extending the current semantic model where
   reasonable, but it may recommend replacement if the current `BehaviorPlan`
   family is fundamentally the wrong abstraction boundary.

## Questions The Research Must Answer

The research output should answer, explicitly:

### A. Top-Level Regseq Model Hierarchy

1. What should the top-level Regseq item hierarchy be?
2. Should `Module` contain a uniform `Vec<Item>`?
3. Should `Impl` remain a first-class item, or should it be flattened?
4. What item kinds are needed in the first semantic-model slice?

### B. Function / Body Ownership

5. How should function-like items be classified?
   - free functions
   - inherent methods
   - trait methods
   - ISRs
6. What metadata belongs on a function item versus an impl item?
7. How should associated constants relate to impls and types?

### C. HIR-Derived Declarative Model

8. Which declarative facts should come from HIR?
9. Which facts should *not* be recomputed from MIR?
10. How should source-level/module-level provenance be preserved?

### D. MIR-Derived Semantic Procedural IR

11. What is the recommended semantic procedural IR shape?
12. Should it be statement/expression-based, operation-node-based, or hybrid?
13. Should current `BehaviorPlan` / `BehaviorNode` be extended or replaced?
14. What function-body families must it support from the current `pl011-full-spec` MIR study?

### E. Locals, Values, and Types

15. How should locals be represented?
    - authored params
    - authored locals
    - return place
    - compiler temporaries
16. How should value expressions be represented?
17. How rich does the type model need to be?
    - scalar types
    - structs
    - enums
    - arrays
    - references
    - option-like/result-like carriers

### F. Call Classification and Normalization

18. How should calls be classified in the semantic IR?
19. Should `Call` remain a generic expression/statement kind with semantic
    sub-kinds such as `HelperCall`, `IntrinsicCall`, `RegisterOp`,
    `ResourceOp`, etc.?
20. Which wrapper calls should normalize directly into semantic operations?
21. Which framework or structural calls should remain explicit?
22. Which calls should become hard failures if they survive too far?

### G. Backend Boundary

23. What exactly should Linux/Zephyr backends consume?
24. What must never be visible to backends anymore?
25. Is there value in a second, shared C-oriented lower IR after the semantic
    IR, or should the semantic IR itself be sufficient for the current backends?

### H. Migration

26. What migration path should move the compiler from the current MIR +
    `BehaviorPlan` state to the proposed model?
27. Which current types/modules can be reused?
28. Which current backend responsibilities must move upward into the semantic IR
    layer?

## Desired Deliverable Format

The research output should be structured as a design note with the following
sections:

1. Executive summary
2. Proposed top-level Regseq item hierarchy
3. Proposed function/body ownership model
4. Proposed semantic procedural IR
5. HIR extraction scope
6. MIR normalization scope
7. Backend consumption boundary
8. Migration strategy from current code
9. Open questions / tradeoffs

Where possible, the deliverable should include:

- concrete proposed type sketches
- short pseudo-Rust enums/structs
- rationale for each major design choice
- explicit references to current code or MIR-study evidence

## Preferred Style Of Answer

The research should:

- be concrete
- be architecture-first
- avoid vague “could be many things” language
- call out tradeoffs explicitly
- ground recommendations in the current `pl011-full-spec` MIR evidence

The research should not:

- jump prematurely into Linux-specific code layout
- assume backends will continue handling MIR residue
- rely on rustc inlining to solve wrapper normalization
- ignore the existing partial portable model in `behavior.rs` and `plan.rs`

## Current Working Assumption

The likely outcome is that the compiler needs:

- a stronger Regseq item hierarchy
- a stronger backend-facing semantic procedural IR
- a clearer separation between:
  - logical source structure
  - semantic procedural structure
  - backend physical/emitted layout

The purpose of this research task is to turn that working assumption into a
coherent, reviewable design proposal.
