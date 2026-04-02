# Phase C-01 Declaration Model Plan

## Purpose

This note proposes the planning direction for `phase-c-01`.

The immediate goal of `phase-c-01` is not to design the final Regseq IR for
all later compiler work. The immediate goal is narrower:

- take the raw frontend artifact produced by `regseq-driver`
- normalize the Layer 1 declaration side into stable rustc-independent types
- preserve enough structure and provenance that later binding, join, analysis,
  and lowering work are not boxed in

This plan exists because the right question at this point is not:

- "what is the final perfect IR?"

It is:

- "what is the right first internal declaration model, given what the frontend
  already proves and given the later stages we know must exist?"

## Architectural stance

`regseq-model` should be treated as the beginning of the Regseq IR space, but
not as one monolithic IR layer.

The staged architecture should be:

1. Declaration IR
- normalized Layer 1 declarations and authored operations

2. Binding IR
- normalized Layer 2 binding/platform facts

3. Joined IR
- declaration + binding join result

4. Lowering-oriented IR
- backend-facing structures derived from the joined model

`phase-c-01` is only about step 1.

These should be treated as coexisting IR families, not as one model that
mutates in place. The declaration IR remains a stable input to later binding,
join, analysis, and lowering-oriented representations.

## Core planning principle

Do not try to design the final lowering-oriented model now.

Instead, design a declaration IR that is:

- stable
- explicit
- loss-minimizing
- reviewable
- traversable
- clearly separated from binding and join work

This means the declaration IR should preserve information rather than compress
it aggressively. If there is a tradeoff, prefer slightly more explicit data now
over premature collapsing that would force later phases to reconstruct meaning.

`phase-c-01` should normalize from the current raw frontend artifact as it
exists, rather than silently expanding raw extraction work inside the same
batch. If the current raw artifact is missing a needed family, that should be
recorded explicitly as a prerequisite or deferral rather than hidden inside
declaration-model tasking.

## What the declaration IR must represent

The declaration IR should normalize the Layer 1 side of the supported PL011
frontend we now have.

That means it must be able to represent:

- driver identity
- device-class identity
- callback-family declarations
- callback methods and their callback contexts
- resource declarations
- firmware-property declarations
- register block declarations
- register descriptor facts
- authored operation evidence
- ordering regions and sequencing anchors
- source provenance / traceability

Concretely, it should be able to absorb the raw artifact families already
present in `RawFrontendArtifact`, including:

- `identity`
- `register_access`
- `trait_callbacks`
- `firmware_properties`
- `clock`
- `reset`
- `pinctrl`
- `interrupts`
- `synchronization`
- `timed_delay`
- `logging`
- `pm_gate`

One explicit limitation at the current boundary:

- ordered regions are not yet present in `RawFrontendArtifact`

So `phase-c-01` must either:

- treat ordered-region normalization as deferred until raw-artifact support is
  added, or
- add that raw-artifact support as a narrowly declared prerequisite before
  declaration normalization begins

It should not pretend ordered-region data is already available in the current
raw artifact pipeline.

## What the declaration IR must not do yet

It must not:

- parse or normalize Layer 2 binding data
- perform Layer 1 / Layer 2 joins
- decide backend lowering strategies
- erase construct ownership in favor of backend-shaped nodes
- infer missing semantics not present in the raw artifact

Examples of things that should stay out of `phase-c-01`:

- whether a property is satisfied by Layer 2
- whether a resource is optional in a given deployment
- generated lifecycle ordering across constructs
- OS-specific callback glue
- DMA lowering conventions
- raw-artifact restructuring beyond clearly declared prerequisites

## Design rules for the declaration IR

### 1. Preserve construct ownership

Do not flatten the model into a generic bag of nodes too early.

The declaration IR should keep facts under construct-owned or family-owned
boundaries such as:

- serial callback family
- register-access declarations and operations
- firmware-property declarations
- interrupt declarations and ISR bindings

This makes later validation and lowering easier to reason about.

### 2. Preserve operation classes

Do not reduce authored activity to undifferentiated call-site lists.

At minimum, the declaration IR should preserve operation classes such as:

- register wrapper operations
- trait callback bodies/method identities
- wrapper call sites by family
- explicit ordered regions
- resource-local operation evidence

This does not require full body IR, but it does require more than a flat
"method name appeared" summary.

### 3. Preserve source traceability

Every normalized declaration entity should retain enough provenance to answer:

- where did this come from?
- which declaration or method did it originate from?
- what raw artifact fact did it normalize from?

Likely provenance fields include:

- source file path
- containing item or method name
- raw artifact family of origin

Provenance must be preserved per normalized entity, not only in a top-level
model metadata record. In particular, operation evidence should retain
`containing_fn`-style provenance so later analysis can still answer which
callback or helper a normalized operation came from.

Exact rustc spans are not required if they would keep the model rustc-shaped,
but provenance should remain reviewable.

### 4. Keep normalization local and explicit

Each normalization step should have a clear justification.

Good examples:

- canonicalizing public trait-family names
- canonicalizing resource optionality and field names
- converting raw stringly method tags into typed enums
- representing register wrapper methods as typed operation enums

Bad examples:

- collapsing all resources into one backend-neutral blob
- inferring joined deployment state
- rewriting serial operations around hypothetical buffered semantics
- flattening construct-local operation evidence into one unowned global bag

### 5. Prefer typed enums over string tags in the normalized layer

The raw artifact can remain string-leaning where necessary.
The declaration IR should move toward explicit enums/structs for:

- callback family
- callback context
- resource kind
- wrapper method kind
- property shape
- interrupt field shape

This is one of the main values of `phase-c-01`.

### 6. Keep operation evidence construct-owned

The normalized model may expose a top-level cross-reference or traversal index
for convenience, but construct-local declarations remain the primary owner of
their own operation evidence.

For example:

- clock call sites stay attached to clock declarations
- reset call sites stay attached to reset declarations
- pinctrl apply sites stay attached to pinctrl declarations
- register wrapper operations stay attached to register-access declarations

If a top-level `OperationEvidenceSet` exists, it should be supplementary rather
than the sole owner of operation evidence.

### 7. Treat pre-existing Layer 2 leakage explicitly

Some extracted families already carry Layer 2-like facts in the raw artifact,
most notably interrupt declarations.

`phase-c-01` should not normalize those as ordinary declaration facts.
Instead, it should preserve them only as:

- opaque sidecar data, or
- explicit `layer2_hints` / `binding_hints` fields

This keeps the declaration IR honest while avoiding data loss before the
binding IR exists.

## Proposed declaration-IR shape

The exact names can change, but the shape should follow this structure.

### Top level

- `DeclarationModel`
  - `driver: DriverDecl`
  - `resources: ResourceDeclSet`
  - `callbacks: CallbackDeclSet`
  - `registers: RegisterDeclSet`
  - `properties: PropertyDeclSet`
  - `operations_index: Option<OperationEvidenceIndex>`
  - `build_context: DeclarationBuildContext`

### Driver declaration

- `DriverDecl`
  - driver name
  - compatible strings
  - device class

### Callback declarations

- `CallbackDeclSet`
  - lifecycle impl
  - serial impl
  - optional serial-console impl
  - optional power-management impl
  - callback contexts

Important current boundary:

- the raw artifact currently preserves callback-family impl facts, not
  method-by-method callback declarations
- `phase-c-01` should therefore normalize callback families at impl level
  unless raw extraction is explicitly extended first
- later method-level callback identity can be added as a follow-on refinement
  if analysis pressure proves it necessary

### Resource declarations

- `ResourceDeclSet`
  - `mmio`
  - `interrupts`
  - `clock`
  - `reset`
  - `pinctrl`
  - `synchronization`

Each sub-family should retain its own declarations, optionality, call-site
evidence, and provenance. This preserves the audit path back to the raw
artifact families.

### Register declarations and evidence

- `RegisterDeclSet`
  - register blocks
  - register fields
  - register descriptor facts
  - named values

### Property declarations

- `PropertyDeclSet`
  - property id
  - field name
  - field type
  - required vs optional
  - default literal
  - vendor prefix
  - property-side lints

### Operation evidence

- `OperationEvidenceIndex`
  - optional, supplementary cross-reference for traversal convenience
  - never the sole owner of operation evidence

Ordered regions should only appear here once they are present in the raw
artifact pipeline. Until then, they remain a declared limitation rather than an
imagined normalized family.

The important point is not the exact grouping. The important point is that the
declaration IR must remain typed, provenance-preserving, and construct-aware.

## How this helps later phases without overcommitting now

This plan is intended to answer the concern:

- "how can we model this now if we do not yet know every later use?"

The answer is:

- do not optimize for every later use
- optimize for a strong immediate contract and information preservation

If `phase-c-01` preserves:

- construct ownership
- operation classes
- stable typed forms
- provenance

then later phases can reshape the model safely into:

- binding-aware forms
- joined forms
- lowering-oriented forms

What would be hard to recover later is:

- which construct owned a fact
- which method/callback originated an operation
- whether an operation was ordered explicitly
- how a resource/property was declared

So that is exactly what the declaration IR should preserve.

One additional rule:

- `target_os` and similar build-time compilation settings should remain in a
  build-context envelope, not inside the declaration identity itself

## Proposed `phase-c-01` task split

This plan suggests a three-task batch.

1. Declaration-model schema
- define the normalized declaration types in `regseq-model`
- typed enums/structs
- provenance fields
- clear family boundaries

2. Raw-artifact normalization
- map `RawFrontendArtifact` into the declaration model
- keep normalization local and deterministic
- no binding/join logic
- normalize from the current raw artifact as-is
- if a missing family such as ordered regions is considered necessary, treat
  that as a declared prerequisite rather than an implicit part of this task

3. Reviewability and traversal baseline
- serialization/debug snapshot for the declaration model
- fixture-backed tests over the supported PL011 frontend
- traversal-friendly access patterns for later analysis

If a fourth task is needed, it should likely be:

4. Declaration-model validation smoke
- reviewer-facing checks that the model preserves the expected frontend facts
- still no Layer 2 join

## Main risks to avoid

1. Over-normalization
- erasing distinctions that later phases need

2. Premature backend shaping
- turning declaration IR into Linux/Zephyr-shaped nodes too early

3. Hidden join logic
- baking Layer 2 assumptions into declaration normalization

4. Loss of provenance
- making the normalized model hard to audit against raw extraction

5. Construct flattening
- moving construct-local operation evidence into a single unowned global bag

6. Silent raw-artifact expansion
- smuggling new extraction work into declaration-model normalization without
  declaring it as a prerequisite

## Current recommendation

Proceed with `phase-c-01` task authoring now, using this plan as the starting
point and treating it as a declaration-IR plan only.

Do not block `phase-c-01` on DMA refinement, but keep the declaration model
open enough that later DMA-oriented declaration facts can be added without
restructuring the whole layer.
