# Regseq Primitive Mapping Framework

## Purpose

This document defines how Regseq primitives should be represented in the
Rust-hosted Regseq surface. Its purpose is to remove interpretation work from
compiler implementation tasks by making the mapping from semantic primitive to
surface construct explicit.

This document is meant to answer four questions:

1. Which kinds of surface constructs exist in Regseq?
2. Which kinds of primitives should map to which constructs?
3. What must be specified for each primitive before implementation starts?
4. What background is necessary to make those decisions correctly?

This document is intentionally self-contained. It is written so that a reader
can understand the problem and evaluate mapping decisions without consulting any
other project file.

## Scope

This document is about the representation of Regseq primitives in the source
language and in the compiler boundary between source, extraction, and lowering.

It is not primarily a lowering document. It does not define target-specific API
choices. It does not try to describe every backend mechanism. It also does not
replace the construct inventory. Instead, it defines the decision framework for
how already-accepted Regseq primitives should appear in code and in extracted
artifacts.

The intended first use of this framework is the PL011-core implementation
slice. The first mapping pass should freeze the PL011-core primitives before
broader DSL and extraction tasking proceeds. E1000-only primitives should be
handled in a later pass once the PL011-core surface is stable.

## Why This Exists

The Regseq construct inventory tells us which constructs, operations, and
properties exist semantically. That is necessary, but it is not enough to begin
compiler implementation safely.

Implementation still requires a second layer of decisions:

- Should a primitive be represented as an inert `#[rseq::...]` attribute?
- Should it be represented as a Rust trait method?
- Should it be represented as an explicit compiler-recognized intrinsic call?
- Should it be represented as a method on a typed resource wrapper?
- Should it be represented only in hardware binding?
- Should it not appear directly in user code at all because it is
  compiler-generated?

If these questions are left open, implementation workers will make them
ad hoc. That creates avoidable drift in syntax, extraction logic, internal
models, and lowering behavior. This document exists to close that gap before
implementation work expands.

## Construct and Primitive

This document uses two related terms precisely.

### Construct

A construct is a semantic unit in the Regseq model, such as Driver Lifecycle,
Register Access, DMA Management, Synchronization, Interrupt, or Firmware
Property Access.

A construct may contain:

- operations
- declarable properties
- compiler-managed responsibilities
- shared resources

### Primitive

A primitive is one concrete unit within a construct that must be represented in
the source language, hardware binding, or compiler model.

Examples of primitives include:

- one specific operation such as `poll`, `suspend`, or `log_info`
- one specific declarable property such as `driver_name` or `access_width`
- one resource declaration
- one handler binding
- one compiler-managed responsibility such as registration or property
  extraction

One construct therefore contains multiple primitives. Primitive mapping is the
work of deciding how each of those concrete units should be represented.

## Regseq Background

### The Three Layers

Regseq has three conceptual layers:

1. Layer 1: driver specification
2. Layer 2: hardware binding
3. Layer 3: compiler backend

Layer 1 captures portable device semantics and driver intent.

Layer 2 captures deployment-specific integration facts, such as addresses, IRQ
routing, clock hookups, reset topology, power-domain membership, BAR
assignments, and similar board- or platform-specific information.

Layer 3 captures target-specific realization, including OS API mappings,
generated registration glue, lifecycle boilerplate, and other backend choices.

Layer 1 and Layer 2 together are the input to the compiler. Layer 3 is the
compiler's implementation.

The key boundary is:

- Layer 1 answers: what does the driver need semantically?
- Layer 2 answers: how is this device instance wired into a system?
- Layer 3 answers: how should a target OS realize those requirements?

### Regseq Is a Rust-Hosted DSL

Regseq is not a separate language. It is a disciplined combination of:

- inert custom tool attributes in the `rseq` namespace
- helper macros
- typed resource wrappers
- Rust traits
- compiler-recognized intrinsics
- ordinary Rust code for non-taxonomic logic

Not every semantic primitive should be represented the same way. Different
surface forms exist because different primitive shapes carry different kinds of
meaning.

In practice, these surface forms compose. A driver-facing trait method may
contain ordinary Rust logic, calls to Regseq intrinsics, and calls to wrapper
methods that either carry specification meaning or provide ergonomic sugar over
underlying core primitives.

### `rseq` vs `regseq`

Regseq deliberately separates the `rseq` attribute namespace from the `regseq`
crate namespace.

- `#[rseq::...]` is the compiler-recognized inert custom tool attribute
  namespace.
- `regseq` is the user-facing crate namespace for helper macros, types, traits,
  and intrinsic stubs.

This separation matters. The `rseq` namespace must remain a tool-attribute
namespace owned by the compiler path. It must not be modeled as a proc-macro
attribute namespace provided by the `regseq` or `regseq-macros` crates.

### Operation Classes Are Semantic, Not Surface Syntax

Each primitive may be classified semantically as one of:

- `driver_facing_trait_operation`
- `driver_invoked_intrinsic`
- `compiler_managed_operation`

These are semantic classes. They tell us how the compiler should treat an
operation during extraction, validation, and lowering. They do not by
themselves fully determine the Rust syntax.

For example, a primitive labeled as driver-facing does not necessarily have to
be a Rust trait method. Some driver-facing primitives may be more naturally
represented as methods on typed resource wrappers.

This document therefore separates:

- semantic operation class
- Rust surface form

Both must be specified.

### Frozen vs Open Decisions

Not every mapping question has the same level of uncertainty.

Some parts of the surface are already close to frozen:

- the use of inert `#[rseq::...]` attributes for declarative metadata
- helper macros such as `register_bitfields!`
- typed resource wrappers as struct fields
- optional resources represented with `Option<T>`
- generic synchronization primitives carrying protected-data type information
- compiler-managed-only primitives not appearing directly in user code

Other areas remain genuinely open and need explicit mapping decisions:

- trait grouping
- trait signatures and `self` model
- intrinsic syntax
- ergonomic wrapper API design
- handler binding mechanics
- error and result patterns

The final mapping artifact should record confidence level explicitly for each
primitive so workers can tell which decisions are frozen and which remain open.

For this framework, `near-frozen` means strongly motivated by existing evidence
and examples and unlikely to change, but not yet confirmed by final
per-primitive review.

### Higher-Risk Constructs

Some constructs carry more ambiguity than others and should be treated as
higher-risk mapping areas during the first pass.

Examples include:

- Synchronization
- Diagnostic Logging
- Parameter Registration
- Firmware Property Access
- Timed Delay
- Deferred Work

These require extra care because the primitive shapes span multiple surface
forms, type-driven properties, compiler-derived behavior, or context-sensitive
lowering choices.

## Regseq Surface Constructs

Regseq primitives may be captured using the following surface constructs. The
lists of typical uses below are non-exhaustive.

### 1. Inert `#[rseq::...]` Attributes

These are declarative metadata attached to items, fields, or methods. They are
recognized by the compiler as inert tool attributes and extracted from Rust
syntax. They are not runtime calls and they are not implemented by user code.

Typical use:

- declaring driver identity
- declaring device class
- declaring resources such as MMIO, interrupts, DMA channels, clocks, resets
- declaring parameters
- declaring firmware properties
- declaring handler bindings

### 2. Helper Macros

These are Rust macros used to express structural or repetitive declarations
more ergonomically than plain attributes alone.

Typical use:

- declaring register layouts and bitfields
- generating repeated structural definitions that are still part of Layer 1

Helper macros do not own the `rseq` attribute namespace.

### 3. Typed Resource Wrappers

These are Rust types that represent hardware resources or compiler-recognized
state in the specification surface.

Typical use:

- mapped MMIO register regions
- interrupts
- clocks
- reset lines
- DMA channels
- synchronization resources
- timers
- completions
- work items

These wrappers serve two roles.

First, they can be semantic carriers. In that role, the wrapper type itself,
its generic arguments, its field position, and any attached attributes carry
extractable specification intent.

Second, they can host ergonomic helper APIs. In that role, wrapper methods may
make authoring easier by combining one or more underlying Regseq intrinsics with
ordinary Rust code. The wrapper method itself is not automatically a separate
special primitive. The true lowering hook remains the intrinsic call or calls
inside it.

### 4. Rust Traits

Traits capture callback-shaped behavior or framework-facing contracts that the
driver must implement directly. They are the strongest fit when the semantic
shape is "the driver provides behavior that the compiler or generated glue
adapts to the target OS."

Typical use:

- lifecycle entrypoints
- device-class callback contracts
- framework callback contracts
- other driver-authored callback-shaped behavior

### 5. Intrinsics

Intrinsics are explicit operations that the driver calls in code, but whose
meaning is compiler-recognized and target-lowered. They are not normal library
calls. They express driver intent that must be preserved during extraction and
replaced during lowering.

Typical use:

- logging
- timed delay and sleep
- explicit DMA streaming operations
- explicit config-space access
- explicit runtime notifications

Intrinsics are the true explicit lowering hooks. A wrapper method may call an
intrinsic internally, but that does not make the wrapper method itself a new
primitive category. In general, the compiler should treat wrapper methods as
ergonomic authoring APIs unless the mapping artifact explicitly says the method
surface itself carries semantic meaning.

Here, "intrinsic" names a semantic role, not necessarily one fixed call shape.
The final mapping may realize that role either as a dedicated intrinsic surface
or as a compiler-recognized operation reached through a known typed method
surface, depending on what best preserves the intended semantics.

### 6. Hardware Binding

Hardware binding captures deployment-specific information that varies across
boards, systems, firmware descriptions, or integration environments. It is not
authored directly in the Rust driver spec except by reference or join.

Typical use:

- MMIO base addresses
- IRQ routing
- clock tree hookup
- reset hookup
- power-domain membership
- BAR assignments
- DMA masks and bus width facts that vary by deployment

### 7. Compiler-Managed Only Primitives

Some primitives should not appear directly in user code at all. They are
generated by the compiler from declarations and properties.

Typical use:

- registration glue
- private data association
- preamble setup
- postamble cleanup
- property extraction
- parameter registration
- framework resource acquisition that follows from declarations

## Primitive Categories and Assignment Rules

This section defines the decision rules for choosing a surface construct. The
categories below are non-exhaustive defaults, not a claim that every primitive
belongs cleanly to exactly one category.

### Category A: Declarative Metadata

Definition:
Facts that describe what exists, how it is identified, or what properties it
has, without themselves representing executable behavior.

Default representation:
- inert `#[rseq::...]` attribute

Typical examples:

- driver identity
- device class
- MMIO declarations
- interrupt declarations
- clock declarations
- parameter declarations
- firmware property declarations
- handler binding declarations

Use an attribute when:

- the primitive is static metadata
- it attaches naturally to an item, field, or method
- the compiler should extract it from syntax rather than from runtime behavior

### Category B: Callback-Shaped Driver Behavior

Definition:
Behavior implemented by the driver and later wrapped or registered by the
compiler as framework, lifecycle, or class behavior.

Default representation:
- Rust trait

Typical examples:

- probe or init
- deinit or remove
- suspend and resume
- serial class behavior
- network device class behavior
- other OS-facing or framework-facing callback contracts

Use a trait when:

- the primitive is implemented by the driver
- generated glue needs to call back into that implementation
- the primitive is part of a coherent callback contract rather than a
  one-off operation on a resource handle

### Category C: Explicit Special Operations in Driver Code

Definition:
Operations that the driver calls directly in code, where the call itself is
part of the specification and must be recognized by the compiler and replaced
or lowered specially.

Default representation:
- intrinsic

Typical examples:

- logging
- delay
- sleep
- DMA streaming map or sync
- configuration-space access
- link-state notification
- explicit packet delivery

Use an intrinsic when:

- the driver must call it explicitly
- the call expresses semantic intent rather than ordinary library computation
- the compiler must intercept and lower it specially

### Category D: Operations on Typed Resources

Definition:
Operations that semantically belong to a specific typed resource rather than to
the driver as a callback provider.

Default representation:
- inherent method on a typed wrapper or resource type

Typical examples:

- register read or write on an MMIO wrapper
- lock acquisition on a synchronization wrapper
- possibly clock operations on a clock wrapper
- possibly completion operations on a completion wrapper

Use a wrapper method when:

- the operation acts on a resource object
- the operation is not best understood as a callback contract
- the syntax should preserve resource ownership and type association directly

This category is important because some primitives may be semantically
driver-authored without being best represented as traits.

If the wrapper method is only ergonomic sugar, the true semantic primitive
should still be recorded separately, usually as one or more underlying
intrinsics plus ordinary Rust around them. If the wrapper type itself carries
specification intent through attributes, field placement, or generics, that
meaning belongs to the wrapper as semantic carrier, not to the helper method as
a distinct lowering hook.

### Category E: Deployment-Specific Facts

Definition:
Facts that vary by board, SoC, firmware description, or integration context.

Default representation:
- hardware binding

Typical examples:

- addresses
- routing
- hookup facts
- BAR assignments
- deployment-specific DMA constraints

Use hardware binding when:

- the fact is not invariant across deployments of the same device logic
- the fact belongs to integration rather than portable semantics

### Category F: Compiler-Derived or Compiler-Generated Behavior

Definition:
Behavior that should occur in the final driver realization but should not be
spelled directly by the driver author.

Default representation:
- compiler-managed only primitive

Typical examples:

- parameter registration
- firmware-property extraction
- framework registration
- private-data association
- resource acquisition implied by declarations

Use compiler-managed only representation when:

- the driver does not invoke or implement the primitive directly
- the behavior should be synthesized from declarations and properties

## Assignment Heuristic

For each primitive, answer these questions in order:

1. Is it a deployment-specific fact?
   - If yes, place it in hardware binding.

2. Is it declarative metadata attached to syntax?
   - If yes, use an inert `#[rseq::...]` attribute or helper macro.

3. Is it behavior the driver implements as part of a callback contract?
   - If yes, use a trait unless a stronger reason exists for another surface.

4. Is it an explicit special operation the driver must call?
   - If yes, represent it as an intrinsic.
   - If user-facing sugar is helpful, allow wrapper methods or helper APIs to
     call that intrinsic internally.

5. Is it an operation on a specific typed resource?
   - If yes, prefer a wrapper method over a trait.
   - Then decide whether the wrapper method is:
     - an ergonomic helper over underlying intrinsics, or
     - an ordinary typed method with no special compiler role.

6. Is it behavior that should be generated by the compiler from declarations?
   - If yes, classify it as compiler-managed only.

This heuristic is the default rule. Construct-level overrides are allowed, but
they must be stated explicitly and justified.

## What Must Be Specified for Each Primitive

Before implementation begins, each primitive should be captured with a complete
mapping record. The record should include the following fields.

### 1. Primitive Identity

What exact primitive is being discussed?

Why it matters:
Implementation tasks must point to a stable named unit rather than rely on a
construct-level summary.

### 2. Owning Construct

Which Regseq construct owns the primitive?

Why it matters:
Some properties are shared across constructs and some constructs overlap.
Ownership must be frozen to avoid duplicate extraction or contradictory
declaration sites.

### 3. Semantic Operation Class

If the primitive is an operation, which semantic class does it belong to?

- driver-facing
- driver-invoked intrinsic
- compiler-managed
- not an operation

Why it matters:
Extraction and lowering behave differently for each class.

### 4. Surface Form

What Rust or binding surface form carries the primitive?

Allowed values:

- item attribute
- field attribute
- method attribute
- helper macro
- trait method
- wrapper method
- intrinsic
- typed field or wrapper type
- generic parameter
- hardware binding field
- compiler-managed only

Why it matters:
This is the decision that implementation workers otherwise make ad hoc.

### 5. Attachment Point

Where does the primitive live concretely?

Examples:

- on the driver type
- on a driver field
- on a method
- on a register block definition
- on a typed resource wrapper
- in the hardware binding schema

Why it matters:
Attachment point determines parsing, extraction, and model structure.

### 6. Property Carrier

How are the primitive's properties represented?

Allowed carriers:

- attribute arguments
- Rust field types
- generic arguments
- wrapper type parameters
- method signature
- binding schema fields
- compiler-derived only

Why it matters:
Many important properties are not ordinary attributes. Synchronization is the
main example: some properties are type-driven.

### 7. Invocation Model

How does the driver interact with the primitive?

Allowed values:

- implemented by driver
- called by driver
- declared by driver
- supplied in hardware binding
- generated by compiler only

Why it matters:
This distinguishes callback contracts from intrinsics from declarations from
compiler-managed glue.

### 8. Extraction Source

Where does the compiler extract the primitive from?

Examples:

- item attributes
- field attributes
- method attributes
- type information
- generic arguments
- trait implementation surface
- intrinsic call sites
- parsed Layer 2 binding data

Why it matters:
The extraction path should not have to be invented during implementation.

### 9. Lowering Responsibility

Who realizes the primitive during lowering?

Examples:

- backend lowers explicit intrinsic call
- backend realizes callback contract
- compiler emits preamble setup
- compiler emits registration glue
- compiler performs Layer 1 plus Layer 2 join only

Why it matters:
Lowering ownership needs to line up with the surface form and semantic class.

### 10. Trait Grouping and Self Model

If the primitive is a trait method, specify:

- which trait it belongs to
- whether the driver type implements that trait directly
- what the `self` model is
- whether the trait is per construct, per device class, or part of a larger
  callback contract

Why it matters:
Saying "this is a trait" is not enough. Trait grouping strongly affects the
user-facing DSL, extraction shape, and generated glue.

### 11. Error and Result Model

If the primitive can fail, specify:

- whether it returns `Result`
- what error type category applies
- whether failure is represented by return type, status code, boolean, or
  another mechanism

Why it matters:
Error modeling affects trait signatures, intrinsic signatures, and callback
glue.

### 12. Notes and Evidence Basis

Record:

- any ambiguity that had to be resolved
- the reason for the chosen mapping
- any important construct-specific override to the default heuristic

Why it matters:
Some mappings will be obvious and some will not. Ambiguous cases should not be
silently decided.

### 13. Confidence Level

Record whether the mapping is:

- frozen
- near-frozen
- open

Why it matters:
The final artifact should distinguish decisions that are already effectively
settled from decisions that still require evidence-backed resolution.

## Output Schema

The final mapping artifact should use one record per primitive. Mixed-form
constructs should therefore decompose into multiple records rather than trying
to encode multiple primitive decisions in one row.

The canonical output shape should be a structured record set with these
top-level fields:

- `version`
- `scope`
- `records`

Machine-readable enum values in the schema use `snake_case`, even where the
surrounding prose uses hyphenated English labels such as `near-frozen`.

Each entry in `records` should use the following schema:

```yaml
version: 1
scope: pl011-core
records:
  - primitive_id: string
    construct: string
    primitive_kind: operation | property | resource | handler_binding | compiler_managed
    semantic_operation_class: driver_facing | driver_invoked_intrinsic | compiler_managed | not_an_operation
    surface_form: item_attribute | field_attribute | method_attribute | helper_macro | trait_method | wrapper_method | intrinsic | typed_field | generic_parameter | hardware_binding_field | compiler_managed_only
    attachment_point: string
    property_carrier:
      - attribute_arguments | rust_field_type | generic_arguments | wrapper_type_parameters | method_signature | binding_schema_fields | compiler_derived_only
    invocation_model: implemented_by_driver | called_by_driver | declared_by_driver | supplied_in_hardware_binding | generated_by_compiler_only
    extraction_source:
      - item_attributes | field_attributes | method_attributes | type_information | generic_arguments | trait_impl_surface | intrinsic_call_sites | layer2_binding_data
    lowering_responsibility:
      - backend_lowers_intrinsic
      - backend_realizes_callback_contract
      - compiler_emits_preamble_setup
      - compiler_emits_registration_glue
      - compiler_performs_layer1_layer2_join
    trait_grouping:
      trait_name: string | null
      implemented_by: driver_type | wrapper_type | none
      self_model: by_ref | by_mut_ref | by_value | static | none
      grouping_basis: per_construct | per_device_class | shared_callback_contract | none
    error_model:
      can_fail: true | false
      return_shape: result | option | bool | status_code | infallible | other
      error_category: string | null
      notes: string | null
    notes: string
    evidence_basis: string
    confidence: frozen | near_frozen | open
```

Schema rules:

- `primitive_id` must be unique across the artifact.
- `construct` names the owning Regseq construct, not a broader subsystem.
- `primitive_kind` distinguishes operations, properties, resources, handler
  bindings, and compiler-managed responsibilities.
- `surface_form` must name the primary representation of the primitive.
- `property_carrier`, `extraction_source`, and `lowering_responsibility` are
  lists because some primitives legitimately draw from more than one source.
- `trait_grouping` must be populated only when `surface_form` is
  `trait_method`; otherwise it should use the `none` values shown above.
- `error_model` should still be present for every record so omissions are not
  ambiguous.
- if a construct needs both a declaration and an operation, create two
  records.
- if a wrapper method is only ergonomic sugar over an intrinsic, record the
  intrinsic as the primitive and explain the sugar in `notes` or
  `evidence_basis`.

Illustrative record:

```yaml
- primitive_id: register_access.poll
  construct: Register Access
  primitive_kind: operation
  semantic_operation_class: driver_invoked_intrinsic
  surface_form: intrinsic
  attachment_point: driver method body or wrapper helper body
  property_carrier:
    - compiler_derived_only
  invocation_model: called_by_driver
  extraction_source:
    - intrinsic_call_sites
  lowering_responsibility:
    - backend_lowers_intrinsic
  trait_grouping:
    trait_name: null
    implemented_by: none
    self_model: none
    grouping_basis: none
  error_model:
    can_fail: false
    return_shape: bool
    error_category: null
    notes: null
  notes: Wrapper sugar may sit on top of the poll primitive.
  evidence_basis: Register polling is an explicit special operation rather than a callback contract.
  confidence: open
```

## Worked Examples

These examples are illustrative decision patterns, not substitutes for final
per-primitive mapping records.

### Example 1: Driver Lifecycle

Primitive shape:
- identity metadata
- callback-shaped driver behavior
- compiler-generated registration and device creation glue

Recommended mapping:
- `#[rseq::driver(...)]` and `#[rseq::device_class(...)]` as inert attributes
  for declarative metadata
- lifecycle entrypoints such as init, deinit, suspend, and resume as trait
  methods because they are callback-shaped behavior implemented by the driver
- registration and device creation as compiler-managed only behavior

Confidence:
- attribute forms: frozen
- callback trait grouping and exact signatures: open
- compiler-managed registration behavior: frozen at the category level

### Example 2: Register Access

Primitive shape:
- declarative register block structure
- typed MMIO resource
- explicit polling behavior
- compiler-managed region mapping

Recommended mapping:
- register layout and bitfield shape via helper macro plus inert attributes
- MMIO region as a typed wrapper field
- read and write as wrapper methods on that typed MMIO wrapper, with the
  wrapper implementation calling underlying Regseq intrinsics
- poll as an intrinsic, optionally exposed through wrapper-level sugar if that
  improves authoring ergonomics
- region mapping as compiler-managed only behavior

This example shows why semantic operation class alone does not fully prescribe
Rust syntax.

Confidence:
- register declaration shape: frozen
- typed MMIO wrapper presence: near-frozen
- read/write as wrapper sugar over underlying intrinsics: open but strongly
  motivated
- poll exact intrinsic surface: open

### Example 3: Interrupt

Primitive shape:
- declarative resource
- optional explicit control operations
- callback-shaped handler behavior

Recommended mapping:
- interrupt declaration via inert field attribute
- handler behavior via one clearly-specified callback binding form, such as:
  - trait method on a driver-facing interrupt contract
  - annotated method on the driver type
  - field-associated handler reference if that model proves more faithful
- explicit enable or disable only if the specification truly needs to call them
  directly; otherwise they should remain compiler-managed, exposed through
  wrapper sugar over intrinsics, or omitted from the user surface as
  appropriate

Confidence:
- resource declaration by attribute: frozen
- handler binding model: open
- explicit control operations: open

### Example 4: Synchronization

Primitive shape:
- typed synchronization resource
- type-driven properties
- operations on the resource itself

Recommended mapping:
- synchronization resources as typed fields
- lock type and protected-data type carried through Rust types and generics
- runtime use expressed through wrapper-local APIs rather than through global
  free functions
- if lock acquisition is explicit in user code, prefer a guard-producing API
  over explicit lock and unlock pairs unless evidence strongly requires the
  latter

Confidence:
- typed field plus generic property carrier: frozen or near-frozen
- guard-based usage model: open but likely
- exact relationship between guard APIs and underlying core primitives: open

### Example 5: Firmware Property Access

Primitive shape:
- declarative property metadata
- compiler-generated extraction behavior

Recommended mapping:
- property declaration via inert `#[rseq::property(...)]` field attribute
- compiler-generated property extraction in lifecycle preamble
- no explicit driver-facing runtime invocation for the read itself unless a
  separate construct requires one

Confidence:
- declaration by attribute: frozen
- compiler-managed extraction category: frozen

### Example 6: Deferred Work

Primitive shape:
- declarative handler binding
- callback-shaped handler behavior
- explicit scheduling operation
- compiler-generated setup and teardown glue

Recommended mapping:
- handler binding as inert method or field attribute
- handler behavior as a callback contract
- submit or schedule as either:
  - an intrinsic, or
  - wrapper-level ergonomic sugar over that intrinsic on a work-item handle
- initialization and cleanup as compiler-managed only behavior

Confidence:
- mixed-form nature of the construct: frozen
- exact handler binding model: open
- scheduling intrinsic and wrapper-sugar split: open

### Example 7: DMA Management

Primitive shape:
- declarative region and channel metadata
- typed resource wrappers
- explicit streaming operations
- compiler-managed allocation and setup

Recommended mapping:
- DMA regions and channels declared via inert attributes on typed fields
- DMA resources represented as typed wrappers
- streaming operations represented as intrinsics, optionally exposed through
  wrapper-level sugar on DMA resource handles
- allocation and request/setup behavior represented as compiler-managed only

Confidence:
- mixed-form nature: frozen
- declaration pattern: near-frozen
- streaming intrinsic and wrapper-sugar split: open

### Example 8: Timer Management

Primitive shape:
- declarative timer metadata
- callback-shaped timer handler behavior
- explicit start and stop control operations
- compiler-managed setup and teardown glue

Recommended mapping:
- timer declaration via inert attributes
- handler behavior via one clearly-specified callback binding model
- control operations expressed as wrapper-local methods, potentially serving as
  sugar over timer intrinsics where needed
- setup and teardown as compiler-managed only behavior

Confidence:
- mixed-form nature: frozen
- exact control surface and handler model: open

### Example 9: Power Management

Primitive shape:
- declarative policy metadata
- callback-shaped suspend and resume behavior
- compiler-managed registration and integration glue

Recommended mapping:
- power-management policy declared via inert attributes
- suspend and resume represented through traits or another clearly-specified
  callback grouping aligned with lifecycle behavior
- registration and framework integration handled as compiler-managed only

Confidence:
- policy declaration by attribute: frozen
- cross-construct trait grouping with lifecycle: open

## Conflict Resolution Rules

When two surface forms seem plausible, use the following precedence.

1. Respect Layer boundaries first.
   - If something belongs in hardware binding, it should not be encoded in the
     Rust specification surface.

2. Prefer declarative forms for declarative facts.
   - If a primitive is metadata rather than behavior, start with attributes or
     macros, not traits or intrinsics.

3. Prefer callback contracts for framework-shaped driver behavior.
   - If the compiler or generated glue conceptually calls into driver behavior,
     start with traits.

4. Prefer wrapper methods for resource-local operations.
   - If the operation semantically belongs to a typed handle, do not force it
     into a trait without a strong reason.

5. Keep the true lowering hook distinct from wrapper ergonomics.
   - If the driver must call it and the compiler must lower it specially,
     model the semantic primitive as an intrinsic.
   - Then decide whether wrapper methods should expose sugar over that
     intrinsic for better authoring ergonomics.

6. Keep compiler-generated glue out of the user surface when possible.
   - Do not force compiler-managed behavior into explicit user-authored syntax
     unless the construct requires it.

7. Allow construct-level overrides, but require them to be stated explicitly.
   - The default heuristic is a guide, not a replacement for evidence-backed
     construct-level decisions.

## Recommended Process for Finalizing Primitive Mappings

The correct process is:

1. Enumerate the validated primitives construct by construct.
2. For each primitive, create a complete mapping record using the fields in
   this document.
3. Mark each decision with a confidence level such as frozen, near-frozen, or
   open.
4. Resolve ambiguous primitives using source evidence and construct-level
   reasoning.
5. Freeze the mapping artifact before writing broad implementation task specs
   for the DSL surface, extraction, and lowering.

Phase A workspace activation can proceed independently. The primitive-mapping
artifact is most critical before broad Phase B surface and extraction work.

## Questions the Final Mapping Artifact Must Answer

At minimum, the final mapping work must answer:

- Which primitives are traits, and how are those traits grouped?
- Which primitives are true intrinsics, and which wrapper methods are merely
  ergonomic sugar over those intrinsics?
- Which driver-facing primitives are better represented as wrapper methods than
  traits?
- Which properties are carried by attributes, and which are carried by types or
  generics?
- Which facts belong in Layer 2 hardware binding rather than Layer 1?
- Which primitives are compiler-managed only and should never be explicit in
  user code?
- Which mixed-form constructs require more than one surface construct?
- Which mappings are frozen, near-frozen, or open?

## Final Principle

The goal is not to minimize the number of surface constructs. The goal is to
assign each primitive to the construct that best preserves its meaning,
ownership, extraction path, and lowering responsibility.

Good primitive mapping makes implementation mechanical.
Bad primitive mapping forces implementers to invent design.

## Example Surface Encodings

This section gives short illustrative encodings using names from the frozen
construct vocabulary. The snippets show shape and composition, not final API
commitments.

### 1. Declarative Attributes on a Driver Specification

This example shows declarative metadata for Driver Lifecycle, Power Management,
Register Access, Interrupt, Clock Management, and Firmware Property Access.

```rust
#[rseq::driver(name = "pl011-uart", compatible = ["arm,pl011"])]
#[rseq::device_class(serial)]
#[rseq::pm(idle = false, wakeup_capable = false)]
struct Pl011Uart {
    #[rseq::mmio(id = "uart_regs", access_width = 32)]
    regs: Mmio<UartRegisters>,

    #[rseq::interrupt(id = "combined_irq")]
    irq: Interrupt,

    #[rseq::clock(id = "uart_clk")]
    clock: Clock,

    #[rseq::property(id = "reg-io-width", default = 4)]
    reg_io_width: u32,
}
```

What this shows:
- `driver_name`, `compatible`, and `device_class` carried by item attributes
- `idle` and `wakeup_capable` carried by a power-management attribute
- `access_width` carried by an MMIO field attribute
- interrupt and clock identity carried by field attributes
- firmware property declaration carried by a field attribute

### 2. Helper Macro for Register Layout

This example shows the helper-macro role for the Register Access construct.

```rust
register_bitfields! {
    UartRegisters [
        DR [
            DATA OFFSET(0) NUMBITS(8) [],
        ],
        FR [
            TXFF OFFSET(5) NUMBITS(1) [],
            RXFE OFFSET(4) NUMBITS(1) [],
        ],
    ]
}
```

What this shows:
- register layout and bitfield structure expressed structurally
- helper macro use rather than callback or intrinsic use
- a home for declarative register structure that can be referenced by `Mmio<T>`

### 3. Trait Surface for Callback-Shaped Behavior

This example shows callback-shaped behavior for a serial device class. The
operation names match the frozen serial management vocabulary.

```rust
trait Serial {
    fn port_open(&mut self) -> Result<(), RegseqError>;
    fn port_close(&mut self) -> Result<(), RegseqError>;
    fn configure(&mut self) -> Result<(), RegseqError>;
    fn tx_byte(&mut self, byte: u8) -> Result<(), RegseqError>;
    fn rx_char(&mut self) -> Option<u8>;
    fn tx_ready(&self) -> bool;
    fn console_write(&mut self, bytes: &[u8]) -> Result<(), RegseqError>;
}

impl Serial for Pl011Uart {
    fn port_open(&mut self) -> Result<(), RegseqError> {
        Ok(())
    }

    fn port_close(&mut self) -> Result<(), RegseqError> {
        Ok(())
    }

    fn configure(&mut self) -> Result<(), RegseqError> {
        Ok(())
    }

    fn tx_byte(&mut self, byte: u8) -> Result<(), RegseqError> {
        self.regs.write_dr(byte)?;
        Ok(())
    }

    fn rx_char(&mut self) -> Option<u8> {
        if self.regs.rxfe() {
            None
        } else {
            Some(self.regs.read_dr())
        }
    }

    fn tx_ready(&self) -> bool {
        !self.regs.txff()
    }

    fn console_write(&mut self, bytes: &[u8]) -> Result<(), RegseqError> {
        for &byte in bytes {
            self.tx_byte(byte)?;
        }
        Ok(())
    }
}
```

What this shows:
- class behavior represented as a trait
- ordinary Rust plus wrapper use inside callback implementations
- exact trait signatures still open

### 4. Intrinsics in Driver Code

This example shows explicit special operations that the driver calls directly.

```rust
fn handle_rx(&mut self) -> Result<(), RegseqError> {
    regseq::log::log_info("rx interrupt");

    while self.regs.poll_rx_ready() {
        let ch = self.regs.read_dr();
        self.push_rx_byte(ch)?;
    }

    regseq::delay::busy_wait(10);
    Ok(())
}
```

What this shows:
- explicit intrinsic calls for logging and timed delay
- ordinary Rust loop structure around those calls
- wrapper-level sugar may still sit on top of the underlying polling primitive

### 5. Typed Resource Wrappers as Semantic Carriers

This example shows wrappers carrying extractable meaning through field type,
generic type, and attached attributes.

```rust
struct CallbackState {
    rx_count: u32,
}

struct Pl011Uart {
    #[rseq::interrupt(id = "combined_irq")]
    irq: Interrupt,

    #[rseq::clock(id = "uart_clk")]
    clock: Clock,

    cb_lock: IrqSpinLock<CallbackState>,
}
```

What this shows:
- typed resources carried by wrapper fields
- synchronization intent carried by `IrqSpinLock<CallbackState>`
- protected-data type carried by a generic, not an attribute

### 6. Wrapper Sugar Over Underlying Intrinsics

This example shows ergonomic wrapper methods that make spec authoring easier
while the underlying intrinsic call remains the true lowering hook.

```rust
impl Mmio<UartRegisters> {
    fn read_dr(&self) -> u8 {
        regseq::intrinsics::mmio_read(self, UartRegisters::DR)
    }

    fn write_dr(&self, byte: u8) -> Result<(), RegseqError> {
        regseq::intrinsics::mmio_write(self, UartRegisters::DR, byte)
    }

    fn poll_rx_ready(&self) -> bool {
        regseq::intrinsics::poll(self, UartRegisters::FR)
    }
}
```

What this shows:
- wrapper methods as ergonomic authoring APIs
- wrapper bodies may delegate to one or more underlying intrinsics
- the helper method is not a separate primitive category

### 7. Mixed-Form Deferred Work Shape

This example shows one construct spanning declarative metadata, callback
behavior, explicit operations, and compiler-managed glue.

```rust
struct ExampleDriver {
    work: Work,
}

impl ExampleDriver {
    #[rseq::work_handler(work)]
    fn handle_work(&mut self) {
        regseq::log::log_debug("work item running");
    }

    fn queue_work(&mut self) {
        regseq::work::submit(&self.work);
    }
}
```

What this shows:
- `work_handler_binding` carried by a method attribute
- callback-shaped handler behavior in a driver method
- explicit scheduling via an intrinsic
- setup and teardown left to compiler-managed behavior

### 8. Hardware Binding Example

This example shows Layer 2 deployment-specific facts separated from the Rust
driver specification.

```toml
[device]
driver_name = "pl011-uart"

[mmio.uart_regs]
base_address = 0x09000000

[interrupt.combined_irq]
irq = 33

[clock.uart_clk]
provider = "apb-pclk"
```

What this shows:
- Layer 1 resource identity joined with Layer 2 deployment facts
- MMIO base address, IRQ routing, and clock hookup outside the Rust spec
- not every primitive belongs in Layer 1
