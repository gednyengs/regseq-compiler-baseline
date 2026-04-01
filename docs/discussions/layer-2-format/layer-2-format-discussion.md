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

- `[platform]`
- `[properties]`
- sections for MMIO, interrupt, clock, reset, pinctrl, serial deployment
  capabilities, and so on

Reasons:

- it aligns with construct ownership
- it keeps validation local and traceable
- it reduces ambiguity about which subsystem owns which field
- it fits the accepted rule that Layer 1 and Layer 2 are joined by explicit
  construct-local identifiers rather than by one giant flat namespace

The leading implication is:

- do **not** use one generic heterogeneous resource bag as the main file model
- prefer construct-owned sections whose fields match accepted construct-local
  Layer 2 facts

### 4a. Do we want a small `[platform]` header?

**Working answer**

Probably yes, but keep it lightweight.

The strongest current direction is to allow a small top-level platform metadata
 section for facts such as:

- board / deployment label
- architecture
- optional descriptive metadata needed for fixture review

This section should remain descriptive rather than authoritative for decisions
 already made elsewhere in the toolchain.

For example:

- target OS is already selected by the compiler invocation path
- driver identity already lives in Layer 1 declarations

So a `[platform]` section may exist for readability and fixture organization,
 but it should not duplicate or override the compiler’s primary configuration
 authorities.

### 4b. What is the initial required `[platform]` contract?

**Working answer**

For the initial B-07 subset, the following should be required:

- top-level `version`
- `[platform]`
- `platform.os`
- `platform.board`
- `platform.arch`

This gives the initial file shape:

- a stable schema/version marker
- enough deployment metadata to make fixtures concrete and reviewable

The current working assumption is:

- omission of these keys is a schema error in B-07 parser/validation work

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

### 11. What is the leading concrete syntax?

**Working answer**

The current leading candidate is **TOML**.

Reasons:

- human-readable
- deterministic
- easy to diff in reviewed fixtures
- naturally supports sectioned documents such as:
  - `[platform]`
  - `[properties]`
  - future construct-owned sections
- already fits the current operational intuition around a checked-in platform
  file

This is still a design direction, not yet the final B-07 contract, but TOML is
 now the most plausible default unless a stronger reason appears to choose
 otherwise.

### 12. How should Layer 2 joins be keyed?

**Working answer**

Layer 2 should preserve the accepted mixed join model:

- explicit-id join where the Layer 1 construct declares an explicit binding id
- field-name join where that is the accepted construct-local pattern

This means the Layer 2 file should not force one universal join style.

Examples:

- firmware properties join by explicit property id
- some resource families may join by explicit `id = "..."`
- reset or pinctrl-style constructs may still rely on field-name join by
  default when that is the accepted construct-local rule

So the file format should be designed around **construct-owned join semantics**,
 not around one global joining convention.

### 13. What should B-07 do with future sections it does not yet validate?

**Working answer**

B-07 should accept placeholder future sections without semantically validating
 them yet.

That means:

- syntactically valid future sections may be parsed and preserved
- B-07 does not need to implement construct-local semantic validation for those
  sections yet
- those sections should not block B-07 unless they are syntactically malformed

This lets the file/container shape acknowledge future growth without forcing
 B-07 to fully close every future Layer 2 construct section.

### 14. What small parsed model should B-07 keep?

**Working answer**

B-07 should keep a small parsed platform-file model, but not the final
 normalized binding model from Phase C.

The leading working shape is:

- `PlatformFile`
  - `version`
  - `platform: PlatformHeader`
  - `properties`
  - `construct_sections`
  - `unknown_sections`
- `PlatformHeader`
  - `os`
  - `board`
  - `arch`
- `PropertyEntry`
  - property key
  - parsed value
- `ConstructSection`
  - section kind/name
  - section key/id where applicable
  - parsed raw values
- `UnknownSection`
  - section path/name
  - parsed raw values

Important boundary:

- this is a Phase-B parsed artifact model
- it exists to support parser behavior, validation, and boundary checks
- it is **not** the final normalized binding model

### 15. What value kinds are in scope for the initial parser?

**Working answer**

The initial parser should support:

- scalar values:
  - integer
  - boolean
  - string
- aggregate values:
  - list
  - table/map

This is enough to support:

- `[properties]`
- lightweight `[platform]`
- future construct-owned sections that may need lists or nested tables

### 16. What boundary checks belong to B-07?

**Working answer**

B-07 parser/validation work should enforce only:

- platform-file schema rules
- parser rules
- section-local rules for supported sections such as `[properties]`

B-07 should **not** enforce:

- cross-layer visibility checks against Layer 1 declarations
- Rust-type compatibility checks against driver fields
- final required-property completeness
- final Layer 1 + Layer 2 join semantics

Those are later compiler checks after the platform file has been parsed.

## Synthesized Template

The following template reflects the current leading direction for the initial
 Layer 2 platform file shape:

```toml
version = "1"

[platform]
os = "linux"
board = "raspberry-pi-4"
arch = "aarch64"

[properties]
reg-io-width = 4
current-speed = 115200

# Future construct-owned sections:
#
# [mmio.uart_regs]
# base = "..."
# size = 0x1000
#
# [interrupt.combined_irq]
# trigger_type = "level_high"
# shared = false
#
# [clock.uart_clk]
# default_frequency = 48000000
#
# [reset.reset]
# reset_line_id = "uart-reset"
# reset_shared = false
#
# [pinctrl.pinctrl]
# pin_states = ["default", "sleep"]
```

What this template is trying to show:

- `version` gives the file a stable schema/version marker
- `[platform]` is part of the required initial contract and carries deployment
  metadata, including:
  - `os`
  - `board`
  - `arch`
- `[properties]` is the first fully implemented section for B-07
- future sections are construct-owned rather than folded into one generic
  resource namespace
- future sections may be accepted and preserved before they are fully
  semantically validated
- section keys should follow the accepted construct-local join rule:
  - explicit ids where the construct has them
  - field-name join where that is the accepted pattern

This template is a discussion artifact, not yet the final B-07 contract.

## Current Leading Direction

The strongest current direction is:

- one Layer 2 platform file per device instance/deployment unit
- TOML as the leading concrete syntax
- human-readable, text-based format
- required initial `version` and `[platform]` contract
- sectioned by construct/resource family
- B-07 fully implements:
  - file/container shape
  - parser and validation pipeline
  - `[properties]` capture
  - the limited Layer 1 / Layer 2 boundary checks needed for the accepted
    firmware-property frontier
- broader normalized binding-model design remains deferred to Phase C

The strongest structure direction is:

- `[properties]` is the first fully implemented section
- future resource/deployment sections should be construct-owned rather than
  grouped inside one generic resource bag
- future sections may be accepted and preserved before they are semantically
  validated
- Layer 2 keys should follow the accepted construct-local join rule, not a
  forced universal naming convention

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

### 4. Reintroducing a monolithic resource bag

Even if future sections cover many resource families, the file should stay
 organized by construct ownership and section-local validation, not collapse
 back into one heterogeneous resource namespace.

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
