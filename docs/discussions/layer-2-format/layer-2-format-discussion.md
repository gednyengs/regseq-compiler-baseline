# Layer 2 Format Discussion

## Purpose

This note captures the current discussion around the Layer 2 platform-binding
 file format that sits between:

- Layer 1: authored Regseq driver declarations
- Layer 3: compiler lowering and backend emission

The immediate goal is to clarify what Phase B-07 is supposed to define before
 we turn it into task specs.

This note is intentionally scoped to the accepted PL011 implementation stream.
 It should help answer:

- what a Layer 2 file is for
- what belongs in it
- what the initial B-07 subset should cover
- what should be deferred to later binding-model and join work

## Current Role of Layer 2

The accepted compiler model is:

- Layer 1 answers:
  - what the driver is
  - what resources, callbacks, declarations, and intent it exposes
- Layer 2 answers:
  - how a particular device instance is wired into a system
  - what deployment-specific values and fallback data are available
- Layer 3 answers:
  - how the compiler realizes the joined result for a target OS

For the current implementation stream, Layer 2 is the **platform binding
 artifact**. It is not Rust source. It is compiler input.

## Guiding Questions and Working Answers

### 1. What is the Layer 2 file for?

**Working answer**

The Layer 2 file is the declarative deployment artifact that captures facts
 that vary by board, SoC integration, firmware environment, or target
 deployment, but that are not appropriate to encode in Layer 1 Rust driver
 source.

Typical Layer 2 responsibilities include:

- MMIO/interrupt/clock/reset/pinctrl binding identity and deployment facts
- firmware-property fallback values
- deployment toggles such as logging behavior
- device-instance data that changes across platforms without changing the
  driver logic

It should not encode:

- driver-authored control flow
- wrapper-call behavior
- callback bodies
- lowering decisions

### 2. Is B-07 defining the final Layer 2 model?

**Working answer**

No.

Phase B-07 should define:

- the accepted Layer 2 file shape for the supported subset
- the parser
- input validation
- parsed property capture
- limited boundary checks that prove Layer 1 declarations and Layer 2 data can
  be related

Phase B-07 should **not** define:

- the final normalized binding model
- the full Layer 1 + Layer 2 join
- lowering-oriented internal representations

Those belong in Phase C.

### 3. Is the Layer 2 file meant to be one file or many files?

**Working answer**

The leading direction should be:

- **one platform file per concrete device instance / deployment unit**

Reasons:

- the accepted planning material repeatedly talks about a singular
  `platform file`
- it keeps the Layer 2 input reviewable
- it makes it easier to ship a compiled-in fallback artifact as one unit
- it avoids scattering per-device facts across unrelated inputs during the
  early compiler stages

This does not rule out future includes or composition, but the initial
 supported format should assume one explicit file.

### 4. Should the Layer 2 file be sectioned by construct?

**Working answer**

Yes.

The strongest direction is a sectioned format where each section groups
 deployment facts by construct or resource family.

For example:

- `[properties]`
- sections for MMIO, interrupt, clock, reset, pinctrl, serial deployment
  capabilities, and so on

Reasons:

- it aligns with construct ownership
- it keeps validation local and traceable
- it reduces ambiguity about which subsystem owns which field
- it fits the accepted rule that Layer 1 and Layer 2 are joined by explicit
  construct-local identifiers rather than by one giant flat namespace

### 5. What is the first required section for B-07?

**Working answer**

`[properties]` is the first required section.

That is the clear center of gravity for B-07 because:

- [firmware-property-access.md](/Users/nyengele/Documents/sekekama/regseq-compiler-v2/docs/compiler-collateral/constructs/firmware-property-access.md)
  explicitly names platform-file `[properties]` values as the accepted Layer 2
  input
- Phase B-06 only captured the Layer 1 declaration side
- the accepted validation rules `PROP-001` through `PROP-005` need parsed Layer
  2 data

So B-07 is primarily about making `[properties]` real.

### 6. Does B-07 need to define all Layer 2 sections for PL011?

**Working answer**

Not necessarily.

The initial B-07 target should be:

- the file/container shape
- the parser/validator framework
- the `[properties]` section
- the minimal compatibility checks required to prove the Layer 1 / Layer 2
  boundary

Other Layer 2 sections may need to be acknowledged in the file design, but B-07
 does not need to fully close every future section if doing so would outrun the
 accepted frontend-first plan.

So the likely rule is:

- define the file so it can host the broader accepted subset
- fully implement only the PL011-relevant property frontier first

### 7. Should Layer 2 field names mirror Layer 1 names exactly?

**Working answer**

No, not as a general rule.

The accepted pattern is:

- Layer 1 owns the authored declaration identity
- Layer 2 owns deployment-specific data keyed by a binding identifier
- the compiler performs the join

This means:

- some joins are explicit by `id = "..."`
- some joins may use field-name conventions when that is already the accepted
  pattern for a construct
- Layer 2 should not be forced into Layer 1 naming when the construct already
  has a more specific binding key

For firmware properties specifically, the key is the **property id**, not the
 Rust field name.

### 8. What does B-07 need to validate for `[properties]`?

**Working answer**

At minimum, B-07 should make it possible to validate the accepted property-side
 rules that depend on parsed Layer 2 data:

- unknown property keys with no matching Layer 1 declaration
- platform/type mismatch
- invalid enum variant
- integer out-of-range for the declared Rust integer type
- required property with no compiled-in fallback path

This does **not** mean B-07 must implement the final joined model. It means the
 parser and boundary-validation path must preserve enough structure to support
 those checks.

### 9. Should Layer 2 be human-readable and reviewable?

**Working answer**

Yes.

The file should be:

- text-based
- deterministic to parse
- stable enough for fixture review
- suitable for checked-in examples and validation tests

This is important because Phase B uses the parsed binding artifact as evidence
 for frontend closure before the final normalized model exists.

### 10. Should B-07 choose the final serialization technology now?

**Working answer**

Only if needed for the supported subset.

What matters more than the exact technology is that the initial format be:

- sectioned
- deterministic
- easy to validate
- easy to diff
- expressive enough for the accepted `[properties]` subset

If one concrete syntax is chosen in B-07, it should be treated as the accepted
 frontend contract for the supported subset, not as an implementation detail.

But the discussion at this stage should focus first on the **information model**
 and section boundaries, then on the concrete syntax.

## Current Leading Direction

The strongest current direction is:

- one Layer 2 platform file per device instance/deployment unit
- human-readable, text-based format
- sectioned by construct/resource family
- B-07 fully implements:
  - file/container shape
  - parser and validation pipeline
  - `[properties]` capture
  - the limited Layer 1 / Layer 2 boundary checks needed for the accepted
    firmware-property frontier
- broader normalized binding-model design remains deferred to Phase C

## B-07 Boundary

### In scope for B-07

- file/container shape for the supported Layer 2 subset
- parser entry point
- validation path
- parsed representation sufficient for frontend closure
- `[properties]` section capture
- limited Layer 1 / Layer 2 compatibility checks

### Out of scope for B-07

- final normalized binding model
- final joined Layer 1 + Layer 2 compiler input
- lowering-oriented internal representation
- backend emission
- real PL011 frontend slice assembly

## Main Risks to Avoid

### 1. Overfitting the full future binding model too early

B-07 should not try to solve Phase C inside the parser.

### 2. Making the file format property-only forever

The first implemented section may be `[properties]`, but the container should
 be shaped so later accepted sections fit naturally.

### 3. Smuggling join logic into the parser

The parser should parse and validate Layer 2 input. It should not silently
 perform the final semantic join.

## Questions to Revisit Before Writing B-07 Task Specs

1. What concrete text format do we want for the initial platform file?
2. What exact parsed representation should Phase B keep before Phase C
   normalization?
3. Which non-`[properties]` sections, if any, must be acknowledged in the
   initial file shape even if they are not fully implemented in B-07?
4. What is the smallest realistic PL011 binding fixture that proves the B-07
   boundary cleanly?

## Status

This discussion note is intended to be the staging ground for the B-07 task
 decomposition discussion. It captures the working shape of the Layer 2 file
 problem without prematurely freezing the full binding model.
