# ASPLOS Implementation Plan
Status: derived execution plan for compiler implementation work.
This document translates the frozen planning material in `docs/guide/` into an operational implementation plan that can be used to create harness task groups and worker-ready task specs.
This document is not the source of truth for construct boundaries, operations, properties, or lowering semantics. The authoritative sources remain:
- `docs/guide/implementation-guide.md`
- `docs/guide/construct-catalog.md`
- `docs/guide/lowering-contract.md`
- `docs/guide/device-coverage.md`
- `docs/guide/open-issues.md`
- `docs/guide/pl011-spec-outline.md`
- `docs/guide/e1000-spec-outline.md`
- `docs/guide/artifact-manifest.md`

## 1. Purpose
This plan bridges the gap between the frozen handoff in `docs/guide/`, the current compiler baseline, and the harness task system under `.harness/tasks/`.
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
2. DSL surface
3. declaration model and extraction
4. PL011 end-to-end
5. E1000 end-to-end
6. paper-facing evaluation
   
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
- `regseq-macros`: proc macros and declarative register-surface macros, including `register_bitfields!`
- `regseq-model`: new rustc-independent crate for declaration models, platform binding models, joined lowering inputs, serialization, and debug artifacts
- `regseq-driver`: rustc-linked extraction crate; owns HIR/MIR collection and joins extracted Layer 1 data with parsed Layer 2 binding data before lowering
- `regseq-analysis`: analysis and validation passes over rustc-independent models
- `regseq-backends`: single crate with per-OS modules for now; Linux and Zephyr remain separate task groups even if they share one crate
- `regseq-diagnostics`: shared diagnostics and reporting support
Backend crates should stay unified initially. Split them only if a later phase shows the single-crate backend layout is blocking task isolation or dependency hygiene.
Later lowering and backend tasks should also treat `docs/experiments/mir-structured-codegen/report.md` as required reference material for MIR extraction shape, CFG structuring, expression folding, and decorated C emission.

## 4. Phase Overview
| Phase | Name                                 | Primary Goal                                                  | Main Output                                   |
| ----- | ------------------------------------ | ------------------------------------------------------------- | --------------------------------------------- |
| A     | Workspace Activation                 | turn the bootstrap workspace into an implementation workspace | live crates, task groups, compiler layering   |
| B     | DSL Surface and Extraction Bootstrap | make a PL011-core Regseq surface compile and extract          | facade, macros, model, extractor              |
| C     | PL011 Specification and Extraction   | validate the first real device spec and extraction path       | PL011 spec crate and extracted model          |
| D     | PL011 Lowering and Validation        | prove one full end-to-end device path                         | generated PL011 Linux and Zephyr artifacts    |
| E     | E1000 Specification and Extraction   | expand to the richer device and its constructs                | E1000 spec crate and extracted model          |
| F     | E1000 Lowering and Validation        | prove the richer device can lower and build                   | generated E1000 Linux and Zephyr artifacts    |
| G     | Evaluation Packaging                 | assemble paper-facing evidence                                | comparison tables, metrics, benchmark outputs |

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

## 6. Phase B: DSL Surface and Extraction Bootstrap
Phase B should not be treated as facade-only work. It should be split into two workstreams.

### Phase B1 Objective
Create a compile-only PL011-core Regseq surface in Rust.

### Phase B2 Objective
Create the declaration model and extractor support needed to turn that surface into a reviewable internal artifact.

### Scope
In scope for the first Phase B slice:
- PL011-core constructs only
- compile-only authoring support
- declaration modeling
- extraction bootstrap
- extracted-model artifact emission for the supported subset
  
Out of scope for the first Phase B slice:
- E1000-specific constructs
- final lowering
- full dual-device syntax closure
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
Explicitly deferred until after PL011 extraction works:
- DMA optional channels
- Hardware Reset
- Pin/GPIO
- Timer Management
- Atomic Operations
- Network Interface Management
- Bus Resource Management
- Parameter Registration
- Deferred Work
  
### Requirements
1. The facade crate must provide real public types for the supported resources and traits.
2. The macro crate must provide only the attribute and declarative surface needed by the supported subset.
3. The API surface must align exactly with validated construct and property names from the guide.
4. The extractor must preserve operation-class distinctions: driver-facing trait operations, driver-invoked intrinsics, and compiler-managed operations.
5. The model must represent type-driven specification choices, especially synchronization types and their protected data types.
6. The extractor must treat `driver_name` and `compatible` as declared by Driver Lifecycle and consumed elsewhere, not redeclared.
7. The extracted-model artifact must be serializable and reviewable.
8. Phase B2 must define the Layer 2 platform binding artifact format, the rustc-independent Rust model for that artifact, and the join boundary where extracted Layer 1 declarations are combined with parsed Layer 2 data.
   
### Task Description
Phase B should decompose into task groups such as:
- facade resource marker types
- facade traits for lifecycle, serial, register access, clock, and interrupt
- intrinsic stubs for log and delay if required by fixtures
- macro support for `#[rseq::driver]`, `#[rseq::device_class]`, `#[rseq::mmio]`, `#[rseq::interrupt]`, `#[rseq::clock]`, `#[rseq::property]`, `#[rseq::pm]`, and `register_bitfields!`
- declaration-model types for supported constructs
- platform-binding model types and file schema definition
- binding parser and loader for Layer 2 input
- extractor collection for supported item kinds
- Layer 1 + Layer 2 join logic prior to lowering
- extracted-model serialization and snapshot tests
  
### Outputs
- live `regseq` facade crate
- live `regseq-macros` crate
- live `regseq-model` crate
- compile-only PL011-core fixture support
- internal declaration model for the supported subset
- platform binding format and parser for the supported subset
- extraction support in `regseq-driver`
- joined Layer 1 and Layer 2 model ready for later lowering
- extracted-model debug artifact for supported fixtures
  
### Test Strategy
- compile-only fixture tests for the supported DSL forms
- macro parsing tests for all supported attributes
- `register_bitfields!` parsing and expansion tests
- type-shape tests for resource wrappers and traits
- binding parser tests for representative Layer 2 inputs
- extraction tests asserting expected declarations, expected properties, operation classes, synchronization metadata, and successful Layer 1 + Layer 2 joins
- snapshot tests for the extracted-model artifact
  
### Exit Criteria
Phase B is complete when a PL011-core fixture compiles through the Regseq surface, the extractor produces a stable internal model for that fixture, and no unsupported primitives were invented to make the fixture compile.

## 7. Phase C: PL011 Specification and Extraction
### Objective
Author the first real device specification in scope and validate that the extraction path handles a real driver, not just compile-only fixtures.

### Scope
In scope:
- PL011 spec crate
- PL011 platform binding instances using the already-defined binding format
- extraction coverage for validated PL011 construct-bearing behavior
Out of scope:
- final generated driver correctness
- E1000 support
- full optional PL011 feature coverage on day one if not needed for the validated slice
  
### Requirements
1. The PL011 spec crate must express the validated PL011 slice without inventing new primitives.
2. The spec must remain at the semantic level and not encode Linux- or Zephyr-specific API choices.
3. The extraction path must cover the supported PL011 constructs and their declared properties.
4. The extracted-model artifact must be suitable for human review.
5. The PL011 spec must be joinable with a real Layer 2 binding instance before lowering work starts.
6. Any caution construct implemented in the chosen slice must carry explicit QC notes and tests.
   
### Task Description
Phase C should decompose into:
- PL011 crate scaffolding and fixture integration
- PL011 binding fixtures that exercise clock, interrupt, firmware-property, and other deployment-specific inputs
- register block declaration work
- lifecycle and device-class declaration work
- interrupt, clock, property, and synchronization extraction coverage
- optional expansion tasks for DMA, reset, pinctrl, timer, or atomic only after the minimal spec extracts cleanly
  
### Outputs
- `track-a-pl011` spec crate
- PL011 binding fixture set
- extracted PL011 model artifact
- joined PL011 lowering-input artifact
- extraction comparison notes against expected PL011 construct usage
  
### Test Strategy
- compile tests for the PL011 spec crate
- binding parse and join tests for PL011 fixture inputs
- extraction tests keyed to PL011 expected declarations
- snapshot tests for extracted and joined PL011 model artifacts
- focused tests for caution constructs present in the chosen PL011 slice
  
### Exit Criteria
Phase C is complete when the PL011 spec compiles against the real Regseq surface, extraction captures the expected PL011 declarations and properties, a real PL011 binding instance joins cleanly with the spec, and reviewers can inspect serialized extracted and joined artifacts and trace them back to source inputs.

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
1. Lowering must preserve the validated construct-bearing behavior of the extracted PL011 model.
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
- lowering unit tests over extracted PL011 models
- backend snapshot or golden-structure tests
- integration tests that assert presence of required structural features
- environment-backed smoke tests in QEMU or equivalent
  
### Exit Criteria
Phase D is complete when PL011 lowers to Linux output that is structurally correct and buildable, PL011 lowers to Zephyr output after Linux stabilization, and no validated PL011 construct-bearing behavior is missing from the supported slice.

## 9. Phase E: E1000 Specification and Extraction

### Objective
Extend the Regseq surface and extraction path to cover the richer, performance-sensitive E1000 device.

### Scope
In scope:
- E1000 spec crate
- E1000-only construct expansion
- extraction support for richer DMA, PCI, parameters, network, deferred work, hardware reset, and atomic-operation behavior
Out of scope:
- benchmark packaging
- broad abstraction redesign unless a validated mismatch is discovered
  
### Requirements
1. E1000 support must build on the PL011-proven model rather than bypass it.
2. The added surface must cover Bus Resource Management, Network Interface Management, Parameter Registration, Deferred Work, Hardware Reset, Atomic Operations, and richer DMA usage.
3. The spec must remain semantic and must not encode Linux mechanisms like NAPI as first-class DSL constructs.
4. Runtime parameters and compiler-managed preamble behavior must be represented explicitly in the model.
   
### Task Description
Phase E should decompose into:
- E1000-specific facade expansion
- E1000-specific extractor and model expansion
- E1000 spec crate authoring
- extraction tests for PCI, DMA ring declarations, params, interrupts, network operations, deferred work, hardware reset, and atomic state
  
### Outputs
- `track-a-e1000` spec crate
- extracted E1000 model artifact
- extraction comparison notes for E1000 constructs
  
### Test Strategy
- compile tests for the E1000 spec
- extraction tests for the new construct set
- snapshot tests for extracted E1000 artifacts
- caution tests for parameter registration, deferred work, logging, synchronization, and timed delay
  
### Exit Criteria
Phase E is complete when the E1000 spec compiles and extracts against the real Regseq surface and the extracted model captures the validated DMA, network, interrupt, parameter, reset, and atomic structure needed for lowering.

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
- lowering tests over extracted E1000 models
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
- extractor-to-model tests
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
6. Do not create E1000-facing worker tasks until PL011 extraction is established.
7. Do not combine Linux-first lowering work and Zephyr follow-up into one oversized task.
   
## 14. Immediate Next Planning Output
The next artifact after this plan should be a harness task breakdown for:
- Phase A
- Phase B
- Phase C
Those phases are now the planning priority because they define the first worker-visible implementation slice and determine whether the rest of the task stream remains coherent.
