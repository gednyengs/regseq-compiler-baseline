# Implementation Guide

This document is an implementation-planning guide for turning the frozen
construct handoff into real compiler work, real device specs, and real
evaluation artifacts.

Its purpose is to guide a cold LLM session to:

- understand what the current implementation scope must achieve
- understand the current compiler baseline
- decompose the work into excellent harness tasks
- attach meaningful tests and quality gates to each task
- preserve the research/evaluation goals that matter for the ASPLOS paper

This is not a methodology-discovery document. The discovery pipeline is
already frozen for the current scope.

## 1. Scope and claim

The current implementation scope is limited to the two artifact devices:

- PL011
- E1000

This effort does **not** attempt full-corpus closure. It does **not**
attempt
to finalize the general Regseq DSL for all devices and OSes.

The frozen claim is:

- the validated construct set is sufficient to specify the analyzed PL011
  and E1000 driver implementations
- every analyzed Step 4 external-operation element for those drivers is
  accounted for by the validated Step 9 construct set
- the handoff package is sufficient to begin DSL/compiler work

Implementation work should therefore aim to demonstrate:

1. real DSL surface encoded in code
2. real PL011 and E1000 specs written against that surface
3. real compiler extraction/lowering for those specs
4. real generated artifacts that are structurally faithful to the
   validated constructs
5. practical evaluation evidence suitable for the paper

### 1.1 Regseq philosophy

Regseq is not trying to transliterate existing OS drivers into a new
syntax. It is trying to let a driver author specify a device **once** in
terms of portable semantic constructs, then let the compiler generate the
OS-specific realization.

The core philosophical commitments are:

- Regseq captures the **what**, not the **how**
- constructs are **discovered** from cross-OS evidence, not invented ad hoc
- the specification should describe device semantics and driver intent,
  while the compiler chooses OS mechanisms
- the system is based on **semantic union**, not lowest-common-denominator
  abstraction

This means:

- the driver spec should not say “use Linux API X”
- the driver spec should say what the device requires semantically
- the compiler should decide whether Linux, Zephyr, or another target uses
  different mechanisms to realize that requirement

Example:

- the spec should not encode “use NAPI”
- the spec should encode network receive/transmit intent, interrupt intent,
  DMA structure, and delivery behavior
- the Linux backend may then realize that intent using NAPI, while another
  backend uses a different mechanism

Whenever an implementation choice is ambiguous, the first question should
be:

- “Is this a portable semantic requirement of the driver/device, or is it
  an OS realization detail?”

If it is the latter, it belongs in the compiler, not in the driver spec.

### 1.2 The three-layer model

Regseq should be understood as three cooperating layers:

1. **Layer 1: driver specification**
   - portable device semantics
   - validated construct usage
   - driver-authored configuration
   - runtime behavior expressed in Regseq primitives
2. **Layer 2: hardware binding**
   - system-specific integration data
   - addresses, IRQ routing, clocks, reset lines, power-domain hookups,
     board-specific resource facts
3. **Layer 3: OS target / compiler backend**
   - API mappings
   - derivation rules
   - generated lifecycle and framework integration
   - optimization strategies

Layer 1 and Layer 2 together form the complete specification input to the
compiler. Layer 3 is built into the compiler.

The clean boundary is:

- Layer 1 answers: “what does the driver need semantically?”
- Layer 2 answers: “how is this device instance wired into this board or
  system?”
- Layer 3 answers: “how should the target OS realize these requirements?”

The implementation work should be understood as building:

- a Rust-hosted representation of **Layer 1**
- a path for consuming **Layer 2** bindings
- Linux and Zephyr **Layer 3** backends

### 1.3 What the Rust DSL is

The Rust DSL is not a completely separate language. It is a disciplined
combination of:

- Rust attributes for declarative metadata
- resource and wrapper types for construct-bearing state
- traits for driver-facing operations
- compiler-recognized intrinsics for special operations
- macros for declarative register/bitfield/spec structure
- ordinary Rust for non-taxonomic control flow and local computation

This gives a practical split:

- **taxonomic / construct-bearing content**
  - compiler-recognized and lowered per target
- **non-taxonomic content**
  - ordinary Rust logic that largely passes through

Examples of construct-bearing content:

- register block declarations
- interrupt intent
- DMA regions/channels
- synchronization primitives
- parameter declarations
- driver lifecycle and device-class integration declarations

Examples of ordinary Rust content:

- local helper functions
- control flow
- arithmetic
- state updates that do not require OS-specific realization

Cold sessions should therefore avoid two mistakes:

- treating the DSL as “just a macro syntax problem”
- treating every piece of driver logic as a taxonomic construct

The right mental model is:

- Rust provides the host language
- Regseq provides the semantic annotations, primitives, and compiler hooks

### 1.4 How to decide: Rust metadata vs platform binding vs compiler

One of the most important design decisions during implementation is where a
property belongs.

Use this rubric.

Put something in the **Rust spec / Layer 1** if it is:

- a portable semantic requirement
- invariant across deployments of the same device logic
- chosen by the driver author
- needed to define the construct itself

Typical Layer 1 examples:

- register access width
- register side effects
- latency/ordering/atomicity intent
- queue/ring structure
- parameter defaults, min/max, and mutability
- lock type / protected data type
- DMA region structure
- device-class behavior exposed by the driver

Put something in the **platform binding / Layer 2** if it is:

- board-, SoC-, firmware-, or integration-specific
- expected to vary across deployments of the same device IP
- analogous to devicetree, ACPI, or board configuration facts

Typical Layer 2 examples:

- MMIO base address
- IRQ line or routing
- clock tree hookup
- reset controller hookup
- power-domain membership
- instance-specific BAR/resource assignment
- platform-determined DMA mask or bus width constraints

Put something in the **compiler / Layer 3** if it is:

- an OS API choice
- generated lifecycle boilerplate
- a derived mechanism decision
- an optimization strategy

Typical Layer 3 examples:

- Linux NAPI choice
- Linux `devm_ioremap_resource` vs Zephyr `device_map`
- hard IRQ vs deferred work realization
- generated subsystem registration and teardown boilerplate

### 1.5 Practical rule for ambiguous choices

When a cold session must choose where something belongs, ask these
questions in order:

1. Is this fact true because of the device/driver semantics themselves?
   - if yes, prefer Layer 1
2. Is this fact true only because of how this board/system wires the
   device?
   - if yes, prefer Layer 2
3. Is this fact really a target-OS mechanism or compiler derivation?
   - if yes, prefer Layer 3

If a proposed new primitive is really just an OS idiom, do **not** add it
to the DSL surface without review.

### 1.6 Current-scope implication

The implementation should treat the frozen handoff package as
the authoritative definition of:

- which constructs exist
- which operations exist
- which declarable properties exist

But those constructs/properties must still be realized in the three-layer
model above:

- Layer 1: Rust-hosted Regseq spec
- Layer 2: platform binding data
- Layer 3: Linux and Zephyr lowering logic

This is the philosophical grounding that should guide task creation,
surface design, extraction, and backend implementation.

## 2. Source documents for cold start

Before creating harness tasks or writing code, a cold session should read
the companion handoff documents:

1. this file
2. `artifact-manifest.md`
3. `construct-catalog.md`
4. `lowering-contract.md`
5. `device-coverage.md`
6. `open-issues.md`
7. `pl011-spec-outline.md`
8. `e1000-spec-outline.md`

For compiler baseline context, a cold session should also read in the
current compiler workspace:

1. `.harness/SESSION.md`
2. `.harness/config.md`
3. `.harness/tasks.json`
4. workspace `Cargo.toml`
5. `crates/cargo-regseq/README.md`
6. `crates/regseq-config/README.md`
7. `crates/regseq-driver/README.md`
8. `docs/experiments/mir-structured-codegen.md`

Supplementary reference:

- `research/construct-discovery-v2/outputs/phase8/dsl-spec.md`

Use the v2.2 DSL spec as a **syntactical reference only** for already
designed surface patterns (attribute shapes, resource-style patterns,
trait/intrinsic idioms). Do **not** use it as the authority for construct
boundaries, operation sets, or property sets. The frozen handoff package is
authoritative for those.

## 3. Current compiler baseline

The current compiler workspace is an early compiler foundation, not yet a
complete implementation workspace for this scope.

What exists:

- harness-based workflow under `.harness`
- completed task stream for:
  - `cargo-regseq`
  - `regseq-config`
  - `regseq-driver`
- working wrapper/config pipeline:
  - CLI parsing
  - toolchain detection
  - `RUSTC_WRAPPER` orchestration
  - `REGSEQ_OPTIONS` serialization/deserialization
  - rustc callback setup
- a MIR/HIR-to-C experiment in `regseq-driver/src/experiment`
- passing workspace tests for the implemented crates

What does **not** exist yet in real code:

- a real `regseq` facade crate in the workspace
- a real `regseq-macros` crate in the workspace
- a validated internal model for constructs, operations, and properties
- a real analysis/lowering/backend pipeline for the current construct set
- a PL011 spec crate
- an E1000 spec crate
- a generated-driver validation and benchmarking harness

Implication:

Implementation must build on the harness, but it cannot skip
directly to “compiler emits final drivers.” The missing layers must be
planned explicitly.

### 3.1 Compiler output format

The compiler should be planned as emitting **target-specific C
source artifacts** plus supporting debug/manifest artifacts.

The intended model is:

- the Regseq compiler emits C source and any associated generated headers
- the generated C is compiled by the target OS's native build toolchain
- the compiler may also emit extracted-model and lowering-manifest artifacts
  for validation and debugging

This means harness tasks should assume:

- backend work targets valid target-specific C emission
- build/integration tasks must verify compatibility with target build
  conventions
- debug artifacts are first-class validation outputs, but C emission is the
  primary generated driver artifact for this scope

## 4. Non-goals

Implementation should **not** aim to prove:

- full-corpus Regseq closure
- universal support for all devices
- performance parity with mature handwritten drivers
- full Windows/FreeBSD build validation on day one
- exact source equivalence with the reference drivers

Implementation **should** aim to prove:

- the validated constructs can be encoded as a real DSL surface
- PL011 and E1000 can be specified against that surface
- the compiler can extract and lower those specs
- the generated outputs are structurally faithful and buildable in the
  chosen evaluation targets
- E1000 performance is operationally reasonable, with bounded and
  explainable degradation relative to the handwritten reference driver
- paper execution scope is Linux and Zephyr only

## 4.1 Target-OS priority

Compiler work must not try to implement all OS backends at once.
The target-OS priority should be:

1. **Linux first**
   - strongest evidence base in the current handoff
   - most practical environment for early build and benchmark work
   - primary target for E1000 performance evaluation
2. **Zephyr second**
   - required result target, not an optional stretch goal
   - useful structurally different target
   - especially relevant for PL011 and embedded-oriented validation
3. **FreeBSD and Windows later**
   - useful comparison targets
   - outside the paper execution scope

Implication:

- the first real generated-driver milestones should target Linux
- Zephyr should follow once the basic Linux extraction/lowering path is
  stable, and both PL011 and E1000 should ultimately have Linux and
  Zephyr outputs in this scope
- FreeBSD and Windows should be treated as later expansion work, not as
  day-one backend scope

## 5. What the device specs must capture

The Regseq specs must capture **device intent**, not OS-specific APIs.

### 5.1 General rule

Each device spec should capture:

- hardware structure
- resource topology
- driver-authored configuration
- validated construct usage
- runtime behavior expressed in the validated primitives

Each device spec should not directly encode:

- Linux-specific API names
- FreeBSD-specific API names
- Zephyr-specific API names
- Windows-specific API names
- compiler-managed lifecycle boilerplate
- backend-specific helper decisions

### 5.2 PL011 spec contents

The PL011 spec should capture at minimum:

- register block layout
- MMIO access width
- one interrupt resource and its intent
- optional DMA channels (TX/RX)
- clock resource
- optional reset line
- optional pinctrl state
- relevant firmware properties
- power-management properties
- synchronization resources needed for ISR/shared state
- timer use if actually needed by the validated slice
- atomic state if used by the validated slice
- serial-facing operations:
  - open/close
  - configure
  - tx/rx primitives
  - console write

The PL011 spec should be judged complete when all validated PL011
construct-bearing behavior in the frozen handoff can be expressed without new
primitive invention.

### 5.3 E1000 spec contents

The E1000 spec should capture at minimum:

- register block layout
- PCI bus identity and resource usage
- BAR selection
- interrupt intent:
  - MSI / legacy fallback
- TX/RX DMA regions
- DMA mask and ring sizing
- network device properties:
  - queue counts
  - MTU
  - MAC behavior where validated
- runtime parameters:
  - ring sizes
  - debug level
  - other validated tunables
- synchronization resources
- deferred work
- reset sequencing
- timing/delay behavior
- power-management intent
- runtime behavior:
  - open
  - stop
  - transmit
  - receive-mode change
  - stats
  - MTU change
  - deliver-packet intrinsic
  - set-link-state intrinsic
  - flow-control intrinsic

The E1000 spec must remain at the cross-OS abstraction level. It should
not encode “use NAPI” as a first-class DSL construct. NAPI is a Linux
lowering strategy selected by the compiler.

### 5.4 Platform binding artifact

This implementation scope requires a distinct **platform binding artifact**
for Layer 2.

Its purpose is to capture system-specific integration facts that should not
be hard-coded into the Rust driver spec. This artifact is the compiler
input that complements the Layer 1 Rust spec.

The platform binding artifact should contain facts such as:

- MMIO base addresses or resource identifiers
- IRQ routing / interrupt line information
- clock/reset/power-domain hookups
- instance-specific BAR/resource assignment
- board- or SoC-specific DMA constraints
- other deployment-specific integration facts

For planning purposes, the exact syntax may still evolve, but the
artifact should be treated as:

- declarative
- machine-readable
- separate from the Rust device specification
- conceptually analogous to devicetree, ACPI, or board configuration data

Cold sessions should therefore create tasks that:

- define the binding artifact format
- parse/load the binding artifact
- join Layer 1 Rust declarations with Layer 2 binding data before lowering

### 5.5 Operation classes and implementation meaning

The validated construct operations fall into three classes, and each class
implies a different compiler implementation pattern.

1. **driver_facing_trait_operation**
   - the driver author implements this as a trait method or equivalent
   - the compiler generates the OS-facing callback registration and wrapper
     logic around it
2. **driver_invoked_intrinsic**
   - the driver author calls this in the spec body
   - the compiler recognizes and replaces the call with target-specific
     lowering
3. **compiler_managed_operation**
   - the driver author does not invoke this directly
   - the compiler generates it as lifecycle/framework glue based on the
     declared constructs and properties

These classes must stay explicit in the internal model because they imply
different extraction, validation, and lowering behavior.

Cold sessions should not collapse these classes into a single generic
“operation” bucket when creating tasks.

### 5.6 Init preamble, teardown, and failure model

Lowering should assume a compiler-generated **ordered init
preamble** and a corresponding **reverse-order cleanup model**.

The intended behavior is:

- resource acquisition and setup steps are emitted in dependency order
- if an init step fails, the compiler emits cleanup for already-acquired
  resources in reverse order
- normal deinitialization mirrors the successful init sequence in reverse
  order

This is a guide-level architectural rule, not an implementation detail.
Cold sessions should preserve it when creating lowering tasks, especially
for:

- MMIO mapping
- IRQ registration
- DMA allocation/setup
- device-class registration
- clocks, reset, power, and other compiler-managed resources

## 6. Implementation phases

The work should be decomposed into explicit phases. Harness tasks should
be organized around these phases, not around broad open-ended goals.

### Phase A: compiler baseline extension

Goal:
- turn the current compiler workspace from a wrapper/experiment baseline
  into an implementation workspace for this scope

Outputs:
- new harness task stream for this implementation effort
- workspace members for the DSL surface
- clear compiler internal layering

### Phase B: DSL surface in code

Goal:
- encode the frozen validated surface in the compiler workspace as real
  code

Expected outputs:
- `regseq` facade crate
- if needed, `regseq-macros` crate
- validated resource types, traits, and intrinsics
- compile-only DSL authoring support sufficient to write PL011/E1000 specs

### Phase C: PL011-first specification and extraction

Goal:
- author first real device spec in the current scope
- validate declaration extraction and internal model generation

Expected outputs:
- PL011 spec crate
- internal extracted representation for the PL011 spec
- serialized extracted-model artifact for review and debugging
- tests showing extraction fidelity

### Phase D: PL011 lowering and validation

Goal:
- prove that one target device can be lowered into real target-specific
  output

Expected outputs:
- generated PL011 driver artifact(s)
- serialized lowered-model or lowering-manifest artifact(s)
- structural comparison against reference driver responsibilities
- buildability/functional validation in the chosen environment

### Phase E: E1000 specification and extraction

Goal:
- author the richer, performance-sensitive device spec

Expected outputs:
- E1000 spec crate
- extracted E1000 internal representation
- serialized extracted-model artifact for review and debugging
- tests covering DMA/network/interrupt/parameter constructs

### Phase F: E1000 lowering and validation

Goal:
- prove the richer target device can be lowered and evaluated

Expected outputs:
- generated E1000 driver artifact(s)
- serialized lowered-model or lowering-manifest artifact(s)
- structural comparison against reference responsibilities
- buildability
- targeted performance evaluation

### Phase G: paper-facing evaluation packaging

Goal:
- collect the metrics and comparisons that support the paper story

Expected outputs:
- artifact comparison tables
- spec-vs-generated size metrics
- compile/build results
- E1000 performance benchmark results

## 7. Harness task design rules

Every cold LLM session creating harness tasks from this guide should follow
these rules.

### 7.1 Task granularity

Tasks should be:

- narrow in responsibility
- testable in isolation
- limited to one crate or one tightly-related pair of crates
- small enough that a high-quality worker can complete them in one
  focused session

Avoid tasks like:
- “implement the DSL”
- “build the backend”
- “add PL011 support”

Prefer tasks like:
- “create `regseq` crate with validated resource marker types”
- “add validated interrupt attribute surface to facade crate”
- “extract Driver Lifecycle declarations into internal config model”
- “lower Register Access MMIO mapping for Linux backend”

### 7.2 API-first rule

The existing harness expects:

1. public API stubs first
2. tests against the API
3. implementation

Tasks should preserve that discipline.

### 7.3 Public/testable boundaries

Whenever possible, task boundaries should align with:

- parse/collect layer
- internal-model layer
- lowering layer
- backend emission layer
- validation harness layer

This makes unit and integration tests meaningful.

### 7.4 Traceability rule

Every implementation task should state which frozen handoff
artifact(s) it draws from:

- construct-catalog
- lowering-contract
- device-coverage
- spec outlines

Tasks should never invent new DSL primitives without explicitly calling
that out as a problem requiring human review.

## 8. Quality gates for harness tasks

Every task should include both a **test plan** and a **quality gate**.

### 8.1 Minimum quality gate

At minimum, tasks should pass:

- crate-local tests
- relevant integration tests
- `cargo fmt`
- `cargo clippy`

### 8.2 Design fidelity gate

For DSL/extraction tasks, the task should verify:

- the implemented surface matches the frozen handoff
- no unsupported primitives are invented
- names align with accepted constructs/properties

### 8.3 Extraction fidelity gate

For extractor/model tasks, the task should verify:

- all expected declarations are captured
- key properties preserve type and ownership semantics
- operation classes are preserved where relevant:
  - driver-facing
  - driver-invoked intrinsic
  - compiler-managed
- a serialized extracted-model artifact is emitted and reviewable

### 8.4 Lowering fidelity gate

For backend/lowering tasks, the task should verify:

- generated output contains the required major structural responsibilities
- no required compiler-managed behavior is missing
- target-specific mechanism choice is plausible and documented
- a serialized lowered-model or lowering-manifest artifact is emitted and
  reviewable

### 8.5 Caution gate

For constructs marked `accepted_with_caution`, tasks must include an
explicit QC note if they are implemented:

- Synchronization
- Diagnostic Logging
- Parameter Registration
- Firmware Property Access
- Timed Delay
- Deferred Work

These are not blockers, but they require extra scrutiny in tests and
task descriptions.

## 9. How to break the work into harness tasks

### 9.1 New task groups that should exist

The current harness task groups stop at compiler bootstrap. The
implementation likely needs new groups like:

- `regseq-facade`
- `regseq-macros`
- `regseq-model`
- `regseq-lowering`
- `regseq-linux-backend`
- `regseq-zephyr-backend`
- `track-a-pl011`
- `track-a-e1000`
- `track-a-validation`
- `track-a-eval`

These exact names are not mandatory, but the separation of concerns is.

### 9.2 Early high-priority task sequence

The first few tasks should not touch E1000 yet. They should establish the
minimum viable implementation stack:

1. add `regseq` facade crate to workspace
2. add validated marker/resource types
3. add validated trait surface
4. add validated intrinsic stubs
5. add attribute/macro support needed to parse device specs
6. define internal declaration model for validated constructs
7. extend the compiler driver to extract declarations into that model
8. create PL011 spec crate
9. validate PL011 extraction

Only after that should the harness move into lowering tasks.

### 9.3 PL011-first rule

PL011 should be the first end-to-end implementation target because it is:

- narrower than E1000
- enough to validate the end-to-end compiler path
- less likely to get blocked on performance-sensitive backend complexity

The first generated-driver milestone should therefore be:

- PL011 extraction
- PL011 lowering
- PL011 build/functional validation

### 9.4 E1000-second rule

E1000 should be the second device because it stress-tests:

- DMA
- network interface management
- interrupt complexity
- parameters
- deferred work
- performance-sensitive lowering

This is the right place to surface any abstraction or lowering weakness
that matters for the paper.

## 10. Testing strategy by phase

### 10.1 DSL surface tests

Purpose:
- verify that the validated syntax and types compile

Examples:
- attribute parsing tests
- intrinsic signature tests
- trait presence tests
- compile-only spec fixtures

### 10.2 Extraction/model tests

Purpose:
- verify that specs are transformed into the expected internal model

Examples:
- PL011 fixture extracts expected resources and operations
- E1000 fixture extracts expected DMA/network/interrupt shapes
- property ownership is preserved correctly
- accepted-with-caution constructs carry the right metadata
- extracted-model debug artifact is stable enough for review

### 10.3 Lowering tests

Purpose:
- verify that target-specific outputs contain the expected structure

Examples:
- generated Linux output contains driver registration
- generated output contains MMIO mapping boilerplate
- generated output contains interrupt registration logic
- generated output contains DMA ring allocation shape
- lowered-model or lowering-manifest debug artifact matches emitted
  responsibilities

Prefer structural assertions over brittle full-file golden diffs in the
early phases.

### 10.4 End-to-end compiler tests

Purpose:
- verify that `cargo-regseq` + `regseq-driver` can compile real specs

Examples:
- PL011 spec -> extracted artifact
- PL011 spec -> generated C artifact
- E1000 spec -> extracted artifact
- E1000 spec -> generated C artifact

### 10.5 Environment-backed validation tests

Purpose:
- validate the generated artifacts in realistic environments

These should be a separate layer, not normal crate unit tests.

Examples:
- PL011 boot/smoke tests in QEMU
- E1000 build and smoke tests in Linux guest
- benchmark runner for E1000

## 11. Device-level validation requirements

### 11.1 PL011 validation requirements

The PL011 implementation should be validated at three levels:

1. **spec completeness**
   - all validated PL011 construct-bearing behavior represented

2. **structural lowering**
   - generated driver performs expected major responsibilities:
     - MMIO map
     - interrupt setup
     - serial registration
     - optional clock/reset/property extraction
     - synchronization setup

3. **functional environment**
   - QEMU-based boot/load/smoke validation
   - basic tx/rx or console sanity if feasible

PL011 performance results are optional/lightweight compared to E1000.

### 11.2 E1000 validation requirements

The E1000 implementation should be validated at four levels:

1. **spec completeness**
   - all validated E1000 construct-bearing behavior represented

2. **structural lowering**
   - generated driver performs expected major responsibilities:
     - PCI enable / BAR claim
     - MMIO map
     - interrupt setup / fallback
     - DMA ring allocation
     - network device registration
     - runtime parameter setup
     - deferred work / reset / timing where applicable

3. **buildability / functional sanity**
   - generated driver builds in the chosen target environment
   - driver can at least initialize and expose the expected interface

4. **performance reasonableness**
   - benchmark against handwritten reference driver
   - no catastrophic overhead
   - degradation is measurable and explainable

## 12. Paper-facing comparison methodology

The results must support the paper. The comparisons should be chosen
accordingly.

### 12.1 What to compare

Compare the generated drivers to the analyzed reference drivers on:

1. **coverage / expressiveness**
   - are all validated construct-bearing behaviors represented?

2. **structural responsibility parity**
   - probe/init responsibilities
   - resource acquisition
   - interrupt handling structure
   - DMA setup
   - device-class integration
   - suspend/resume or equivalent lifecycle paths

3. **artifact size**
   - spec LOC
   - generated output LOC
   - handwritten reference LOC
   - amount of generated boilerplate

4. **buildability**
   - does the generated driver build in the chosen environment?

5. **runtime metrics**
   - especially for E1000

### 12.2 What not to compare

Avoid using as primary results:

- exact source equivalence
- exact CFG equivalence
- full performance parity claims
- superficial API-name matching

### 12.3 E1000 performance framing

The intended paper claim for this scope is:

- not parity with the handwritten driver
- but reasonable operational performance with bounded degradation

Good primary metrics for E1000:

- throughput
- latency
- CPU utilization
- drops/loss
- optionally interrupt/poll behavior if observable

The writeup should present these as evidence that the abstraction is
practically viable, not that it fully matches production-tuned drivers.

### 12.4 Minimum E1000 benchmark contract

Every benchmark task for E1000 should specify:

- baseline:
  - generated Linux driver
  - handwritten Linux reference driver
- environment controls:
  - same kernel version/configuration
  - same QEMU/KVM setup
  - same guest image
  - same MTU and queue configuration where applicable
  - same workload generator and peer setup
- workload classes:
  - at least one throughput-oriented workload
  - at least one latency-sensitive or small-packet workload
- measurement rules:
  - repeated runs
  - collected throughput/latency/CPU/drop metrics
  - note any interrupt/poll observations if available
- reporting rule:
  - explain any degradation in terms of current lowering/compiler maturity
  - do not claim parity unless the data actually supports it

## 13. Evaluation environments

### 13.1 QEMU for PL011

QEMU is sufficient and recommended for:

- PL011 functional bring-up
- serial-path validation
- early Linux/Zephyr smoke tests

### 13.2 QEMU/KVM baseline for E1000

For E1000, use:

- QEMU/KVM for initial functional validation and baseline benchmarking

This is sufficient if the benchmark methodology is careful and
applied equally to the generated and handwritten reference drivers.

### 13.2.1 Zephyr integration note

Zephyr backend tasks should assume a different integration model from
Linux:

- Linux output can be validated as module- or kernel-style generated C that
  plugs into the Linux build path
- Zephyr output must be compatible with Zephyr's static build model and
  surrounding CMake/Kconfig/devicetree-oriented integration flow

This does not require the implementation guide to prescribe the full
Zephyr build layout up front, but cold sessions should account for the fact
that Zephyr integration is not the same as Linux module integration.

### 13.3 Stronger environments

If later needed, stronger environments may be added, but they are not
required to begin:

- fuller Linux guest benchmarking setups
- more realistic network peers
- more demanding workload orchestration

### 13.4 What the environment must support

At minimum, the evaluation setup should support:

- deterministic builds
- generated driver artifact injection
- reference driver comparison under the same setup
- workload execution
- metric collection
- repeatability across runs
- baseline/generated driver swaps under the same setup

## 14. Suggested milestone sequence

This is the recommended milestone order.

### Milestone 1: DSL surface exists in code

Deliverables:

- real `regseq` facade crate
- validated syntax/resources/intrinsics compile

Gate:

- compile-only DSL fixtures pass

### Milestone 2: PL011 spec compiles and extracts

Deliverables:

- PL011 spec crate
- extracted internal model artifact

Gate:

- extraction tests prove expected construct/property capture

### Milestone 3: PL011 lowers and validates

Deliverables:

- generated PL011 driver artifact
- generated Linux artifact
- generated Zephyr artifact
- structural comparison notes
- QEMU functional sanity evidence

Gate:

- no missing validated PL011 construct-bearing behavior

### Milestone 4: E1000 spec compiles and extracts

Deliverables:

- E1000 spec crate
- extracted internal model artifact

Gate:

- extraction tests for DMA/network/interrupt/params pass

### Milestone 5: E1000 lowers and validates

Deliverables:

- generated E1000 driver artifact
- generated Linux artifact
- generated Zephyr artifact
- structural comparison notes
- buildability evidence

Gate:

- no missing validated E1000 construct-bearing behavior

### Milestone 6: E1000 benchmark results

Deliverables:

- benchmark environment
- generated vs handwritten driver comparisons
- bounded-degradation result narrative

Gate:

- generated driver is operationally reasonable

## 15. Cold-start instructions for creating harness tasks

If a cold LLM session is asked to create new harness tasks in the current
compiler workspace, it should:

1. read this document and the companion handoff package
2. inspect current workspace members and confirm the baseline
3. create new task groups only for missing implementation layers
4. avoid creating giant umbrella tasks
5. for each task, specify:
   - exact input context files
   - exact output files
   - public API expectations
   - tests
   - acceptance criteria
   - relationship to the validated constructs/devices
   - whether the task must emit a reviewable debug artifact
6. sequence tasks so PL011 reaches end-to-end first
7. sequence Linux before Zephyr within each device path unless a task is
   purely shared infrastructure
8. explicitly reserve E1000 benchmarking as a later validation phase

## 16. Final planning rule

Compiler work should always be driven by this priority order:

1. preserve fidelity to the frozen handoff
2. create a real usable DSL surface
3. get PL011 end-to-end first
4. get E1000 end-to-end second
5. produce evaluation artifacts that support the paper

Do not optimize prematurely for a broader general-purpose compiler
architecture.
This effort should deliver a credible, validated artifact path first.
