# ASPLOS Implementation Plan
Status: derived execution plan for compiler implementation work.
This document translates the frozen planning material in `archive/guide/` into an operational implementation plan that can be used to create harness task groups and worker-ready task specs.
This document is not the source of truth for construct boundaries, operations, properties, or lowering semantics. The authoritative sources remain:
- `archive/guide/implementation-guide.md`
- `archive/guide/construct-catalog.md`
- `archive/guide/lowering-contract.md`
- `archive/guide/device-coverage.md`
- `archive/guide/open-issues.md`
- `archive/guide/pl011-spec-outline.md`
- `archive/guide/e1000-spec-outline.md`
- `archive/guide/artifact-manifest.md`

## 1. Purpose
This plan bridges the gap between the frozen handoff in `archive/guide/`, the current compiler baseline, and the harness task system under `.harness/tasks/`.
It should be used to:
- define implementation phases in operational terms
- identify concrete requirements and outputs for each phase
- establish test and quality gates before task writing begins
- prevent premature expansion into E1000 or multi-OS complexity
- keep task decomposition consistent across cold worker sessions

## 2. Planning Rules

### 2.1 Authority Rule
Use the construct catalog and lowering contract as the authority for:
- construct identity
- operation classes
- declarable properties
- lowering responsibilities
Use the PL011 compiler collateral package as the primary operational planning
input for PL011 compiler work:
- `docs/compiler-collateral/compiler-baseline-overview.md`
- `docs/compiler-collateral/compiler-constraints.md`
- `docs/compiler-collateral/constructs/`
Use the spec outlines only to decide:
- which validated constructs appear in PL011 and E1000
- what the first implementation slices should contain
  
### 2.2 Scope Rule
This track is limited to:
- PL011
- E1000
- Linux
- Zephyr
  
This track does not target:
- full-corpus DSL closure
- FreeBSD or Windows implementation
- exact source equivalence with handwritten drivers
- performance parity with mature reference drivers
  
### 2.3 Sequencing Rule
Overall sequencing is:
1. workspace activation
2. frontend closure
3. internal model and join
4. PL011 lowering and validation
5. E1000 specification, extraction, and model closure
6. E1000 lowering and validation
7. paper-facing evaluation
   
Within lowering phases, sequencing is:
1. Linux first
2. Zephyr second
   
### 2.4 Device Rule
PL011 is the first end-to-end target. E1000 must not drive the initial DSL or extraction surface.

### 2.5 Layering Rule
Every task and phase should preserve the three-layer model:
- Layer 1: Rust Regseq driver specification
- Layer 2: platform binding artifact
- Layer 3: compiler lowering and backend emission
No task should encode target-OS mechanisms directly into Layer 1.

### 2.6 Caution Rule
The following constructs require explicit QC notes in this plan and in any resulting harness tasks:
- Synchronization
- Diagnostic Logging
- Parameter Registration
- Firmware Property Access
- Timed Delay
- Deferred Work
These are not blockers, but they require stronger extraction and lowering tests than ordinary constructs.
For PL011-core frontend closure, these constructs also require extraction-level
QC, not just lowering-level QC.

## 3. Current Baseline
The current workspace already has a functioning bootstrap path for:
- `cargo-regseq`
- `regseq-config`
- `regseq-driver`
The current workspace does not yet have real crate implementations for:
- `regseq`
- `regseq-macros`
- `regseq-analysis`
- `regseq-backends`
- `regseq-diagnostics`
Those crate directories already exist, but they are not yet live workspace members with implementation code. Workspace activation is therefore real implementation work, not clerical setup.
The current implementation plan resolves crate topology as follows:
- `regseq`: public Layer 1 facade for driver authors
- `regseq-macros`: helper proc macros and declarative register-surface macros,
  including `register_bitfields!`; inert `#[rseq::...]` tool attributes are
  registered by `regseq-driver`, not provided by proc macros
- `regseq-model`: new rustc-independent crate for declaration models, platform binding models, joined lowering inputs, serialization, and debug artifacts
- `regseq-driver`: rustc-linked extraction crate; owns HIR/MIR collection and joins extracted Layer 1 data with parsed Layer 2 binding data before lowering
- `regseq-analysis`: analysis and validation passes over rustc-independent models
- `regseq-backends`: single crate with per-OS modules for now; Linux and Zephyr remain separate task groups even if they share one crate
- `regseq-diagnostics`: shared diagnostics and reporting support
`regseq-model` work therefore spans two different planning stages:
- Phase B for Layer 2 format/parser support and raw-artifact serialization helpers
- Phase C for normalized declaration/binding models and joined lowering inputs
Backend crates should stay unified initially. Split them only if a later phase shows the single-crate backend layout is blocking task isolation or dependency hygiene.
Later lowering and backend tasks should also treat `docs/experiments/mir-structured-codegen/report.md` as required reference material for MIR extraction shape, CFG structuring, expression folding, and decorated C emission.

## 4. Phase Overview
| Phase | Name                                 | Primary Goal                                                  | Main Output                                                |
| ----- | ------------------------------------ | ------------------------------------------------------------- | ---------------------------------------------------------- |
| A     | Workspace Activation                 | turn the bootstrap workspace into an implementation workspace | live crates, task groups, compiler layering                |
| B     | Frontend Closure                     | make the accepted PL011 frontend compile, parse, and extract | compiling PL011 surface, parsed bindings, raw extraction artifact |
| C     | Internal Model and Join              | turn raw frontend capture into stable compiler inputs         | normalized model, binding model, joined artifact           |
| D     | PL011 Lowering and Validation        | prove one full end-to-end device path                         | generated PL011 Linux and Zephyr artifacts                 |
| E     | E1000 Specification, Extraction, and Model Closure | expand the compiler pipeline to the richer device and its constructs | E1000 spec crate and joined model artifact                 |
| F     | E1000 Lowering and Validation        | prove the richer device can lower and build                   | generated E1000 Linux and Zephyr artifacts                 |
| G     | Evaluation Packaging                 | assemble paper-facing evidence                                | comparison tables, metrics, benchmark outputs              |

## 5. Phase A: Workspace Activation

### Objective
Convert the current bootstrap workspace into an implementation workspace that can host real DSL, model, lowering, backend, and validation work.

### Scope
In scope:
- add missing crates to the workspace
- establish crate responsibilities and dependencies
- create task groups for the implementation effort
- define a stable internal layering boundary for later tasks
Out of scope:
- real DSL surface behavior
- extraction logic
- device specifications
- backend lowering
  
### Requirements
1. `Cargo.toml` must include the implementation crates that the guide expects to exist for this track.
2. Each implementation crate must have a minimal `Cargo.toml`, `src/lib.rs`, public crate documentation, and compile in the workspace.
3. Internal crate boundaries must be explicit and resolved as part of this phase:
   - the rustc-independent internal declaration model lives in `regseq-model`
   - extraction lives in `regseq-driver`
   - analysis passes live in `regseq-analysis`
   - Linux and Zephyr backends live as modules inside `regseq-backends` unless a later split becomes necessary
4. The harness must gain new task groups for implementation work.
5. The workspace must continue to pass baseline formatting, lint, and test checks.
   
### Task Description
Phase A should decompose into narrow setup tasks such as:
- activate `regseq` as a real workspace crate
- activate `regseq-macros` as a real workspace crate
- activate `regseq-analysis`, `regseq-backends`, and `regseq-diagnostics`
- add and activate `regseq-model` as a real workspace crate
- define crate-level README and module boundary expectations
- create harness task groups for facade, model, lowering, PL011, E1000, validation, and evaluation
  
### Outputs
- updated workspace membership
- minimal live implementation crates
- documented crate roles and dependency boundaries
- explicit crate-topology decision recorded in code and docs
- new harness task-group layout for the ASPLOS implementation stream
  
### Test Strategy
- `cargo check --workspace`
- `cargo test --workspace`
- crate smoke tests for any new public APIs introduced in setup work
- verification that no inactive stub crate remains outside the workspace by accident
  
### Exit Criteria
Phase A is complete when all required crates are live workspace members, the workspace builds cleanly, harness groups exist for the implementation stream, and later phases can create API-first tasks without restructuring the workspace again.

## 6. Phase B: Frontend Closure

### Objective
Close the accepted PL011 frontend end-to-end:
- the accepted Layer 1 DSL surface compiles
- Layer 2 platform files parse and validate
- the compiler can find and extract accepted PL011 primitives from HIR/MIR
- extraction emits a reviewable raw artifact

This phase should close frontend capture without requiring the final normalized
internal model, analysis stack, or lowering design to be complete first.

### Scope
In scope:
- PL011-core constructs only
- authorable Layer 1 facade and helper macro support
- inert `#[rseq::...]` tool-attribute recognition in the compiler path
- Layer 2 platform-binding file format plus parser and validator
- raw HIR/MIR extraction of accepted primitives
- raw extraction artifact emission
- PL011 spec and fixture coverage sufficient to prove frontend closure
  
Out of scope:
- normalized rustc-independent declaration-model design
- normalized binding-model design
- full Layer 1 + Layer 2 join logic
- analysis passes beyond extraction-level validation
- lowering and backend realization
- E1000-specific constructs
- performance-sensitive backend decisions
  
### PL011-Core Construct Subset
The initial supported subset should include:
- Driver Lifecycle
- Register Access
- Serial/UART Management
- Interrupt
- Clock Management
- Synchronization
- Firmware Property Access
- Power Management
Allowed as stubs if needed by the PL011 fixture:
- Diagnostic Logging intrinsics
- Timed Delay intrinsics
Deferred from the initial PL011-core pass, but still within Phase B before full
frontend closure:
- DMA optional channels
- Hardware Reset
- Pin/GPIO
- Timer Management
- Atomic Operations
- Network Interface Management
- Bus Resource Management
- Parameter Registration
- Deferred Work
These constructs should be pulled into a second Phase B expansion pass once the
core PL011 frontend path is proven. They do not move to Phase C, because
Phase C is reserved for normalized model, join, and analysis work.
  
### Requirements
1. The facade crate must provide real public types for the supported resources, wrappers, and traits.
2. The macro crate must provide only the helper-macro and declarative surface needed by the supported subset.
3. The frontend surface must align with the accepted PL011 compiler collateral and validated construct/property names from the guide.
4. The Layer 2 platform-binding format must be defined, parsed, and validated without requiring the final normalized binding model.
5. The extractor must preserve operation-class distinctions: driver-facing trait operations, driver-invoked intrinsics, and compiler-managed operations.
6. The extractor must preserve type-driven specification choices, especially synchronization types, optional resources, and protected-data type parameters.
7. The extractor must treat `driver_name` and `compatible` as declared by Driver Lifecycle and consumed elsewhere, not redeclared.
8. The raw extraction artifact must be serializable, reviewable, and keyed by construct identity.
9. The raw extraction artifact schema must preserve enough structure for later internal work:
   - operation classes
   - typed properties and generic choices
   - declaration order where later lifecycle work depends on it
   - intrinsic call sites and source context
10. Phase B must include a minimal Layer 1 / Layer 2 boundary compatibility check proving that parsed bindings and raw extraction outputs can be related without yet defining the final joined model.
This constrains what Phase B must capture, not the final normalized model shape.
   
### Task Description
Phase B should decompose into task groups such as:
- facade resource marker types and typed wrappers
- facade traits for lifecycle, serial, power management, and other supported callback families
- intrinsic stubs for log and delay if required by fixtures
- helper macro support for `register_bitfields!` and any additional
  non-attribute helper macros needed by the PL011-core slice
- Layer 2 file-schema definition plus binding parser and validator
- extraction and semantic handling of inert `#[rseq::driver]`,
  `#[rseq::device_class]`, `#[rseq::mmio]`, `#[rseq::interrupt]`,
  `#[rseq::clock]`, `#[rseq::property]`, and `#[rseq::pm]` tool attributes
- construct-by-construct collection for supported item kinds from HIR/MIR
- raw extraction artifact serialization and snapshot tests
- `track-a-pl011` spec-crate work as the primary evidence that the accepted
  frontend surface and extraction paths are real, not only synthetic fixtures
- fixture work where needed to isolate narrow frontend cases before the full
  PL011 slice is assembled
  
### Outputs
- live `regseq` facade crate
- live `regseq-macros` crate
- compile-capable PL011-core frontend support
- `track-a-pl011` spec crate for the first real frontend slice
- platform-binding format and parser/validator for the supported subset
- extraction support in `regseq-driver`
- raw extraction debug artifact for supported fixtures/specs
- PL011 binding fixture inputs and frontend closure notes
  
### Test Strategy
- compile tests for the supported DSL forms and the first real PL011-facing frontend slice
- macro parsing tests for all supported helper macros
- `register_bitfields!` parsing and expansion tests
- type-shape tests for resource wrappers and traits
- binding parser and validation tests for representative Layer 2 inputs
- extraction tests asserting expected declarations, expected properties,
  operation classes, synchronization metadata, declaration ordering where
  relevant, and intrinsic call-site capture
- extraction-level QC tests for caution constructs present in the PL011-core slice
- a minimal Layer 1 / Layer 2 boundary compatibility smoke test
- snapshot tests for the raw extraction artifact
  
### Exit Criteria
Phase B is complete when the accepted PL011 frontend slice, including the
deferred Phase B expansion constructs needed by the PL011 baseline, compiles,
representative Layer 2 bindings parse and validate, extraction captures the
expected PL011 primitives into a stable raw artifact, the raw artifact is
reviewable and traceable back to source, and no unsupported primitives were
invented to make the frontend compile.

## 7. Phase C: Internal Model and Join
### Objective
Turn raw frontend capture into stable compiler inputs by defining the normalized
internal model, the normalized binding model, the Layer 1 + Layer 2 join path,
and the first analysis/validation passes needed before lowering begins.

### Scope
In scope:
- normalized declaration-model types
- normalized platform-binding model types
- Layer 1 + Layer 2 join logic
- joined-artifact serialization
- analysis and validation bootstrap over normalized models
Out of scope:
- backend lowering and emission
- E1000 support
- broad surface redesign unless raw extraction reveals a proven mismatch
  
### Requirements
1. The normalized declaration model must be informed by what raw extraction actually produces, rather than assumed in advance.
2. The normalized model must preserve construct ownership, operation classes, type-driven properties, and source traceability from the raw artifact.
3. The normalized binding model must represent the accepted Layer 2 file format without widening the frontend contract.
4. The join boundary must combine extracted Layer 1 data with parsed Layer 2 data into a reviewable joined artifact.
5. Phase C must establish the minimum analysis and validation passes needed to confirm model completeness and consistency for the PL011 slice.
6. The joined artifact must be stable enough to serve as the input contract for lowering work in Phase D.
   
### Task Description
Phase C should decompose into:
- normalized declaration-model types for the supported constructs
- normalized binding-model types for the supported Layer 2 subset
- join logic that combines raw extraction output with parsed binding data
- joined-artifact serialization and snapshot coverage
- analysis/bootstrap checks for operation-class integrity, property completeness,
  synchronization metadata, and cross-construct ownership consistency
- optional refinement tasks if raw extraction reveals a proven mismatch that
  must be resolved before lowering
  
### Outputs
- normalized declaration model for the supported PL011 slice
- normalized binding model for the supported Layer 2 subset
- joined PL011 lowering-input artifact
- joined-artifact snapshots
- model and join comparison notes against expected PL011 construct usage
  
### Test Strategy
- normalization tests from raw extraction into the declaration model
- binding-model tests over accepted Layer 2 parsed inputs
- join tests keyed to PL011 expected declarations and binding data
- snapshot tests for joined PL011 artifacts
- focused tests for caution constructs present in the chosen PL011 slice
  
### Exit Criteria
Phase C is complete when the raw extraction artifact normalizes cleanly into the
internal model, a real PL011 binding instance joins cleanly with that model,
reviewers can inspect the joined artifact and trace it back to frontend inputs,
and the joined artifact is stable enough to serve as the lowering input for
Phase D.

## 8. Phase D: PL011 Lowering and Validation

### Objective
Prove that one artifact device can be lowered into real target-specific output and validated in realistic environments.

### Sequencing
This phase should run in two steps:
1. Linux lowering and validation
2. Zephyr lowering and validation
The Linux path is the first hard requirement. Zephyr follows once the basic lowering path is stable.

### Scope
In scope:
- PL011 lowering model
- Linux backend emission
- Zephyr backend emission
- generated debug artifacts
- buildability and smoke validation
Out of scope:
- E1000
- paper benchmark packaging
  
### Requirements
1. Lowering must preserve the validated construct-bearing behavior of the joined PL011 model.
2. Compiler-managed work from the lowering contract must appear in the generated output where applicable.
3. Linux output must be build-oriented first, with QEMU or equivalent smoke validation.
4. Zephyr output must be structurally compatible with Zephyr's static build model.
5. Lowering must emit a reviewable lowering-manifest or equivalent debug artifact.
6. Lowering and backend tasks should use the MIR-structured codegen experiment report as reference material for control-flow structuring, expression folding, and readable C emission strategy.
   
### Task Description
Phase D should decompose into:
- lowering IR or manifest definition
- Linux lowering for lifecycle/probe registration, MMIO mapping, IRQ registration, clock/property/synchronization realization, and serial device-class integration
- Linux backend emission
- PL011 Linux build and smoke validation harness
- Zephyr lowering and emission for the same supported subset
- PL011 Zephyr smoke validation
  
### Outputs
- generated PL011 Linux artifact
- generated PL011 Zephyr artifact
- lowering-manifest artifacts
- structural comparison notes
- smoke validation evidence
  
### Test Strategy
- lowering unit tests over joined PL011 models
- backend snapshot or golden-structure tests
- integration tests that assert presence of required structural features
- environment-backed smoke tests in QEMU or equivalent
  
### Exit Criteria
Phase D is complete when PL011 lowers to Linux output that is structurally correct and buildable, PL011 lowers to Zephyr output after Linux stabilization, and no validated PL011 construct-bearing behavior is missing from the supported slice.

## 9. Phase E: E1000 Specification, Extraction, and Model Closure

### Objective
Extend the Regseq frontend, model, and extraction path to cover the richer,
performance-sensitive E1000 device and produce lowering-ready joined artifacts.

### Scope
In scope:
- E1000 spec crate
- E1000-only construct expansion
- extraction support for richer DMA, PCI, parameters, network, deferred work, hardware reset, and atomic-operation behavior
- normalized model and join support for the E1000 slice
Out of scope:
- benchmark packaging
- broad abstraction redesign unless a validated mismatch is discovered
  
### Requirements
1. E1000 support must build on the PL011-proven model rather than bypass it.
2. The added surface must cover Bus Resource Management, Network Interface Management, Parameter Registration, Deferred Work, Hardware Reset, Atomic Operations, and richer DMA usage.
3. The spec must remain semantic and must not encode Linux mechanisms like NAPI as first-class DSL constructs.
4. Runtime parameters and compiler-managed preamble behavior must be represented explicitly in the model.
5. Phase E should follow the same frontend-to-model sequencing used for PL011:
   frontend capture first, then normalized model and join closure.
   
### Task Description
Phase E should decompose into:
- E1000-specific facade expansion
- E1000-specific extractor expansion
- E1000-specific model and join expansion
- E1000 spec crate authoring
- extraction tests for PCI, DMA ring declarations, params, interrupts, network operations, deferred work, hardware reset, and atomic state
  
### Outputs
- `track-a-e1000` spec crate
- joined E1000 model artifact
- extraction and model comparison notes for E1000 constructs
  
### Test Strategy
- compile tests for the E1000 spec
- extraction tests for the new construct set
- snapshot tests for extracted and joined E1000 artifacts
- caution tests for parameter registration, deferred work, logging, synchronization, and timed delay
  
### Exit Criteria
Phase E is complete when the E1000 spec compiles and extracts against the real
Regseq surface, the normalized and joined E1000 model captures the validated
DMA, network, interrupt, parameter, reset, and atomic structure needed for
lowering, and the richer device still fits the PL011-proven pipeline shape.

## 10. Phase F: E1000 Lowering and Validation

### Objective
Prove that the richer artifact device can be lowered into real target-specific output and validated well enough to support the ASPLOS claim.

### Sequencing
This phase should run in two steps:
1. Linux lowering, buildability, and benchmark readiness
2. Zephyr lowering and structural validation
   
### Scope
In scope:
- E1000 lowering and backend emission
- Linux-first build and smoke validation
- targeted performance harness work
- Zephyr structural output for the supported subset
Out of scope:
- parity claims against the handwritten driver
- multi-OS expansion beyond Linux and Zephyr
  
### Requirements
1. Linux lowering must cover the compiler-managed preamble requirements for PCI, DMA, interrupt allocation, and parameter handling.
2. Backend emission must preserve the structure needed for buildability and evaluation.
3. Linux output must be suitable for build validation and benchmark use.
4. Zephyr output must be structurally faithful for the supported subset, even if it is not the primary benchmark target.
5. Lowering artifacts must explain any major mechanism choices.
6. Lowering and backend tasks should use the MIR-structured codegen experiment report as reference material for MIR-driven extraction shape and readable C emission decisions.
   
### Task Description
Phase F should decompose into:
- Linux lowering for PCI, MMIO, DMA rings, interrupt handling, params, deferred work, and network integration
- Linux backend emission
- E1000 Linux build validation
- benchmark harness integration
- Zephyr lowering and emission tasks for the supported subset
- comparison and validation note generation
  
### Outputs
- generated E1000 Linux artifact
- generated E1000 Zephyr artifact
- lowering-manifest artifacts
- buildability evidence
- benchmark-ready environment outputs
  
### Test Strategy
- lowering tests over joined E1000 models
- backend structure tests for emitted C
- Linux build integration tests
- smoke tests in virtualized environments
- benchmark harness sanity tests
  
### Exit Criteria
Phase F is complete when E1000 Linux output builds and is usable for evaluation, E1000 Zephyr output exists for the supported scope, and the generated driver is operationally reasonable for benchmark use.

## 11. Phase G: Evaluation Packaging

### Objective
Assemble the outputs required to support the ASPLOS evaluation narrative.

### Scope
In scope:
- artifact comparison tables
- spec-vs-generated metrics
- build and validation summaries
- E1000 benchmark outputs and bounded-degradation framing
Out of scope:
- new compiler surface work
- new lowering mechanisms unless required to fix evaluation blockers
  
### Requirements
1. Evaluation packaging must be generated from actual artifacts produced by prior phases.
2. Performance reporting must use the bounded-degradation framing from the guide, not parity claims.
3. Comparison notes must focus on structural responsibilities and validation evidence.
   
### Task Description
Phase G should decompose into:
- artifact collection and normalization
- comparison-table generation
- build-result summarization
- benchmark-result summarization
- paper-facing note assembly
  
### Outputs
- comparison tables
- size, build, and result summaries
- benchmark result package
- paper-facing evidence bundle
  
### Test Strategy
- reproducibility checks for generated reports
- consistency checks between reported numbers and source artifacts
- review pass over benchmark framing and comparison methodology
  
### Exit Criteria
Phase G is complete when the paper-facing evidence can be regenerated from tracked artifacts and performance claims remain within the scope defined by the guide.

## 12. Cross-Phase Test Strategy
Each phase should contribute to a layered test stack.

### Unit Level
- facade type behavior
- macro parsing
- model construction
- lowering transformations
- backend rendering helpers
  
### Integration Level
- compile-only spec fixtures
- extractor-to-raw-artifact tests
- raw-artifact-to-model tests
- model-to-lowering tests
- backend structure tests
  
### End-to-End Level
- `cargo-regseq` orchestration over real specs
- artifact emission checks
- generated output presence and shape checks
  
### Environment-Backed Level
- PL011 smoke validation
- E1000 build and smoke validation
- E1000 benchmark runs
  
## 13. Harness Translation Rules
When converting this plan into harness tasks:
1. Do not write worker tasks directly from the guide. Write them from this plan plus the guide.
2. Keep each task narrow enough to fit one crate or one tightly related crate pair.
3. Every task must cite the specific guide artifacts it draws from.
4. Every task must include implementation scope, explicit non-goals, a test plan, and a quality gate.
5. Caution constructs must include an explicit QC note in the task text.
6. Do not create E1000-facing worker tasks until PL011 frontend closure is established.
7. Do not let normalized model, join, or analysis tasks block frontend-closure task authoring.
8. Use the accepted PL011 compiler collateral as the primary task-authoring input for PL011 compiler tasks.
9. Do not combine Linux-first lowering work and Zephyr follow-up into one oversized task.
   
## 14. Immediate Next Planning Output
The next artifact after this plan should be a harness task breakdown for:
- Phase A
- Phase B
- Phase C
Those phases are now the planning priority because they define the first worker-visible implementation slice and determine whether the rest of the task stream remains coherent.
