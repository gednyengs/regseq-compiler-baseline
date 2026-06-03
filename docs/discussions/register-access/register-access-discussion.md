# Register Access Discussion

## Purpose

This note consolidates the current design discussion around Register Access for
the Regseq compiler implementation stream. It is intentionally self-contained.

## Current Accepted Baseline

The accepted Register Access construct currently gives us four important
frontend elements:

1. An explicit register-block declaration surface
- `register_access.register_block`
- represented as a helper-macro / register-block declaration form

2. A typed MMIO resource
- `Mmio<T>`

3. Resource-local wrapper operations
- `read`
- `write`
- `modify`
- `set`

4. A free-function polling intrinsic
- `regseq::poll::until(...) -> Result<(), RegseqError>`

The accepted metadata surface also already includes register-oriented facts:

- `access_width`
- `endian`
- `side_effects`
- `hw_updates`

And the current baseline overview explicitly treats Register Access as part of
the wrapper-family baseline, with `poll::until` as an accepted intrinsic
exception.

## What We Are Deciding

The main design questions are not about whether Register Access exists. That is
already accepted. The open questions are about frontend shape:

1. What should `register_bitfields!` define?
2. What should remain explicit in the `#[rseq::register_block]` struct?
3. Where should register side effects live?
4. What compiler reasoning should the frontend shape preserve for later
   validation and lowering?

## Current Direction

The strongest current direction is:

1. `register_bitfields!` should define register and field schemas
2. the actual register block should remain an explicit Rust struct
3. the explicit struct should carry field-local register metadata
4. the compiler should preserve enough information to reason about:
   - access legality
   - side effects
   - polling
   - ordering-sensitive patterns

This means the helper macro should not be asked to define the whole MMIO block.

## Preferred Frontend Shape

The preferred shape is:

```rust
regseq::register_bitfields! [
    u32,

    DR_REG [
        DATA OFFSET(0) NUMBITS(8) []
    ],

    FR_REG [
        TXFF OFFSET(5) NUMBITS(1) [],
        BUSY OFFSET(3) NUMBITS(1) []
    ],

    LCRH_REG [
        WLEN OFFSET(5) NUMBITS(2) [
            WLEN_5 = 0,
            WLEN_6 = 1,
            WLEN_7 = 2,
            WLEN_8 = 3
        ],
        FEN OFFSET(4) NUMBITS(1) []
    ]
];

#[repr(C)]
#[rseq::register_block(endian = "little", access_width = 32)]
pub struct Pl011Registers {
    pub dr: ReadWrite<u32, DR_REG::Register>,
    pub fr: ReadOnly<u32, FR_REG::Register>,
    pub lcr_h: ReadWrite<u32, LCRH_REG::Register>,
}
```

This split is preferred because:

- field schema and block layout are different concerns
- the MMIO block remains explicit and HIR-visible
- reserved gaps remain explicit in authored code
- field-local metadata has a natural place on explicit struct fields
- extraction can reason over normal struct fields rather than only macro
  expansion products

## Why The Explicit Register Block Matters

The explicit `#[rseq::register_block]` struct is the right place for:

- physical layout facts
- per-register access wrappers
- reserved ranges
- field-local metadata
- later compiler validations tied to specific hardware locations

This is especially important for constructs like PL011, where:

- some registers are read-only
- some are write-only
- some share layouts
- some have overloaded write semantics
- reserved gaps matter to real layout fidelity

The explicit block also aligns naturally with `Mmio<T>`, because `T` should be
an authored register-block type rather than an entirely macro-generated opaque
artifact.

## Register Helper Macro Role

`register_bitfields!` should define:

- register names
- field names
- bit offsets
- bit widths
- optional named values / field variants
- a register descriptor shape usable by `ReadOnly`, `WriteOnly`, and
  `ReadWrite`

It should not own:

- inert `#[rseq::...]` attributes
- the whole MMIO block
- backend lowering
- ordering behavior

Its expansion should be usable by the facade register support types and then
referenced from the explicit register-block struct.

### Current preferred expansion contract

The current preferred direction is that `register_bitfields!` is the only
generative part of the register frontend surface.

For a register like `FR_REG`, the minimum useful expansion shape is:

```rust
pub mod FR_REG {
    pub struct Register;

    pub mod BUSY {
        pub const FIELD: Field<u32, super::Register> = Field::new(3, 1);
    }

    pub mod TXFF {
        pub const FIELD: Field<u32, super::Register> = Field::new(5, 1);
    }
}
```

And for fields with named values:

```rust
pub mod LCRH_REG {
    pub struct Register;

    pub mod WLEN {
        pub const FIELD: Field<u32, super::Register> = Field::new(5, 2);
        pub const WLEN_5: FieldValue<u32, super::Register> =
            FieldValue::new(5, 2, 0);
        pub const WLEN_6: FieldValue<u32, super::Register> =
            FieldValue::new(5, 2, 1);
        pub const WLEN_7: FieldValue<u32, super::Register> =
            FieldValue::new(5, 2, 2);
        pub const WLEN_8: FieldValue<u32, super::Register> =
            FieldValue::new(5, 2, 3);
    }
}
```

This gives the frontend a stable generated contract:

- `Register` marker type per register descriptor
- `Field<T, R>` constants for read-side field selection
- `FieldValue<T, R>` constants for named write-side values

That contract is currently the most important missing mechanical decision.

## Side Effects In Register Access

The current taxonomy already gives us:

- `side_effects`
- `hw_updates`

Those are important and should remain first-class.

The discussion so far suggests that side effects should be carried on the
explicit register-block fields, not only buried inside bitfield macro output.

The preferred frontend shape is:

```rust
#[repr(C)]
#[rseq::register_block(endian = "little", access_width = 32)]
pub struct Pl011Registers {
    #[rseq::register(side_effects = "...", hw_updates = "...")]
    pub fr: ReadOnly<u32, FR_REG::Register>,
}
```

So the current preference is:

- `register_bitfields!` owns bit layout and named values
- `#[rseq::register_block(...)]` owns block-level layout facts
- `#[rseq::register(...)]` on explicit struct fields owns per-register semantic
  metadata such as `side_effects` and `hw_updates`

Important side-effect categories likely include:

- write-one-to-clear
- latch/commit-on-write
- hardware-updated / asynchronously changing fields
- other read/write semantics that affect legality or lowering

The point is not only documentation. These facts are useful for:

- extraction
- legality checks
- warning generation
- lowering decisions
- preventing misleading helper APIs

## A Likely Missing Piece: Relational Register Metadata

The current accepted metadata is strong for field-local facts, but discussion
has surfaced one likely missing class of facts:

- relationships between registers

Examples:

- one register stages values for another
- one register commits or latches values previously written elsewhere
- one write invalidates or clears state reflected by another register

This is different from ordinary `side_effects`.

It suggests we may eventually need one or more explicit relational metadata
constructs for patterns like:

- staging/commit
- latch trigger
- commit dependency

This is not yet a settled Phase-B implementation requirement, but it is an
important candidate construct to keep visible during design review.

## Compiler Reasoning We Should Preserve

The Register Access frontend should preserve enough structure for later
compiler reasoning in at least these areas.

### 1. Access Legality

The compiler must be able to distinguish:

- `ReadOnly`
- `WriteOnly`
- `ReadWrite`

That means the wrapper-type split is not just ergonomic. It is part of the
static legality story.

### 2. Register-Metadata Reachability

Extraction should preserve:

- `access_width`
- `endian`
- `side_effects`
- `hw_updates`

These are not optional commentary. They are input facts for later analysis and
lowering.

### 3. Wrapper Call-Site Recognition

The compiler should be able to recognize call sites on known register wrapper
types:

- `read`
- `write`
- `modify`
- `set`

These are the normal authored register operations and must be visible in
frontend extraction.

### 4. Polling Recognition

The compiler should recognize:

- `regseq::poll::until(...)`

This matters because polling is a stable intrinsic exception inside the
register-access baseline, not just an arbitrary helper call.

### 5. Ordering-Sensitive Patterns

The compiler should preserve enough information for later reasoning about:

- stage then commit patterns
- read-modify-write behavior
- hardware-updated fields
- ordering-sensitive sequences

This does not mean all ordering policy must land immediately in `phase-b-02`,
but it does mean the frontend shape should not destroy the information needed
for later checks.

## `regseq::ordered!`

The current decision is to keep `regseq::ordered!` out of `phase-b-02`.

That is still the right immediate choice because:

- `phase-b-02` should stay focused on the shared register/MMIO foundation
- `ordered!` is closer to user-facing sequencing/ordering control than to the
  core register declaration model
- its semantics overlap with later synchronization and ordering discussions

But the discussion also surfaced an important planning requirement:

- `regseq::ordered!` must have an explicit later home

It should not be silently deferred without ownership. The current plan assigns
it to the later synchronization/interrupt shared-extension work.

## Implications For `phase-b-02`

The current discussion supports the following `phase-b-02` direction:

1. `Mmio<T>` as the typed MMIO shell
2. explicit register support types:
   - `ReadOnly`
   - `WriteOnly`
   - `ReadWrite`
   - `Reserved`
   - `Field`
   - `FieldValue`
3. `regseq::poll::until(...)`
4. `register_bitfields!`
5. register/MMIO frontend extraction scaffolding

And it suggests these implementation refinements:

- `register_bitfields!` should define register schemas, not whole blocks
- the explicit `#[rseq::register_block]` struct should remain the main layout
  surface
- field-local side-effect metadata should remain visible on the explicit block
- driver extraction should recognize wrapper-method call sites and register
  metadata explicitly

## Open Questions

The following questions remain open for further design discussion:

1. What exact syntax should carry field-local register metadata on the explicit
   block fields?
2. Which side-effect classes need stable names now, versus later?
3. Do we need a dedicated relational metadata mechanism for staging/commit
   patterns?
4. What exact expansion contract should `register_bitfields!` provide to
   facade-side register support types?
5. How much ordering reasoning belongs in the Register Access construct versus
   the later `ordered!` surface?

The following concrete frontend-mechanics questions should also be answered
before the Register Access surface is treated as fully settled.

### Q1. Where should access width live in the type surface?

`register_bitfields!` currently starts with a storage width such as `u32`. That
raises an important design question:

- should authored block fields repeat the width explicitly?

For example:

```rust
pub fr: ReadOnly<u32, FR_REG::Register>
```

versus:

```rust
pub fr: ReadOnly<FR_REG::Register>
```

This is really a question about where the width lives:

- on the wrapper type (`ReadOnly<u32, R>`)
- on the generated register descriptor (`FR_REG::Register`)
- or both

Initial observation:

- the current preferred direction is to keep width in both places
- that means:
  - wrapper field type: `ReadOnly<u32, FR_REG::Register>`
  - generated descriptor contract: `Field<u32, FR_REG::Register>`
- the duplication is intentional:
  - easier HIR extraction
  - simpler method signatures
  - direct consistency checking between field declaration and descriptor

So the working position is to keep the width explicit in wrapper field types.

### Q2. Can one file define bitfields for multiple register blocks?

We should decide whether `register_bitfields!` is intended to support:

- one invocation per block
- multiple invocations in one file
- one invocation that defines descriptors used by multiple blocks

Initial observation:

- the current preferred direction is that this should work naturally
- because the macro defines descriptor modules, not full blocks
- multiple invocations per file and descriptor reuse across multiple explicit
  register blocks should both be supported

### Q3. Can the compiler reach all relevant register information?

We need to ensure the combined frontend shape preserves enough information for:

- extraction
- analysis
- Regseq model synthesis

That includes:

- register and field names
- offsets and bit widths
- named values
- wrapper access kind
- block-level metadata
- field-level semantic metadata
- any later relational metadata

This is not just a syntax question. It is a compiler-reachability question.

### Q4. What makes `Mmio<Pl011Registers>` meaningful?

Right now, `Pl011Registers` as an authored struct is just an ordinary Rust
type. So we need to define what makes:

```rust
regs: Mmio<Pl011Registers>
```

more than just a generic container.

This raises two subquestions:

1. What constraint exists on `T` in `Mmio<T>`?
2. What machinery makes field access and wrapper calls work?

For example, if authored code writes:

```rust
self.regs.lcr_h.modify(...)
```

then something in the type machinery must make that meaningful.

Initial observation:

- the current preferred direction is that `Mmio<T>` should implement
  `Deref<Target = T>`
- that gives authored code the expected field-access shape:
  - `self.regs.lcr_h.modify(...)`
- and keeps the contract lightweight at Phase B time
- `T` does not need a Rust trait bound right now
- instead, the compiler should validate at extraction time that `T` is an
  accepted `#[rseq::register_block]` type

So the current working position is:

- `Mmio<T>` has a `Deref<Target = T>` field-access contract
- `#[rseq::register_block]` validity is a compiler-side check, not a facade
  trait bound

### Q5. Can we trace the full register-access lifecycle through the compiler?

We should be able to pretend we are the compiler and trace this sequence:

1. define register schemas with `register_bitfields!`
2. define the explicit `#[rseq::register_block]` struct
3. declare `Mmio<Pl011Registers>` on the driver struct
4. call `read`, `write`, and `modify` on the resulting register fields
5. recover all the facts needed for analysis and Regseq model synthesis

If that full chain is not explainable in compiler terms, then the frontend
surface is underspecified.

### Q6. What do the macro and attributes actually generate?

We need to define the underlying generated types and contracts for:

- `register_bitfields!`
- `#[rseq::register_block(...)]`
- `#[rseq::register(...)]`

This includes:

- what Rust items are generated
- what traits or marker types exist
- how register descriptors connect to wrapper types
- how `Mmio<T>` relates to the authored block type
- how extraction can recover the information later

Without that, the surface remains plausible but mechanically incomplete.

Current preferred answer:

- `register_bitfields!` generates Rust items
- `#[rseq::register_block(...)]` generates nothing and is consumed by compiler
  extraction as inert metadata
- `#[rseq::register(...)]` generates nothing and is consumed by compiler
  extraction as inert metadata

So only the helper macro is generative. The attributes remain declarative.

## Current Preferred Underlying Machinery

The current preferred minimal machinery model is:

- `Mmio<T>`
  - opaque MMIO resource carrier
  - `Deref<Target = T>` for field access

- `ReadOnly<T, R>`
  - exposes `read`

- `WriteOnly<T, R>`
  - exposes `write`
  - exposes `set`

- `ReadWrite<T, R>`
  - exposes `read`
  - exposes `write`
  - exposes `modify`
  - exposes `set`

- `Reserved<T>`
  - layout spacer / reserved-slot carrier

- `Field<T, R>`
  - const-constructible field descriptor carrying offset and width

- `FieldValue<T, R>`
  - const-constructible value carrier for field writes
  - should support composition, likely via `Add<Output = Self>`

And the corresponding frontend split is:

- `register_bitfields!`
  - generates register descriptor modules and field/value constants
- `#[rseq::register_block(...)]`
  - marks the explicit block as a register block
- `#[rseq::register(...)]`
  - carries per-field semantic metadata

## What The Compiler Must Be Able To Recover

Given the preferred split, the compiler should be able to recover:

- register block names and field names from the explicit struct
- access kind and width from wrapper field types
- block-level metadata from `#[rseq::register_block(...)]`
- field-level semantic metadata from `#[rseq::register(...)]`
- field offsets, widths, and named values from macro-generated items
- wrapper call sites from HIR/MIR method calls on known wrapper types
- polling intrinsic call sites from `regseq::poll::until(...)`

This means the macro expansion must remain HIR-visible and structurally
predictable, not opaque.

## What The First Experiment Validated

The first isolated experiment under:

- `experiments/register-access/exp01`

validated that the current preferred machinery model is mechanically coherent
in a minimal Rust prototype.

The experiment successfully type-checked all of the following:

1. A generated register descriptor contract from `register_bitfields!`
   including:
   - `FR_REG::Register`
   - `FR_REG::BUSY::FIELD`
   - named values like `LCRH_REG::WLEN::WLEN_8`

2. An explicit `#[repr(C)]` register-block struct using:
   - `ReadOnly<u32, FR_REG::Register>`
   - `ReadWrite<u32, LCRH_REG::Register>`
   - `WriteOnly<u32, INT_REG::Register>`
   - `Reserved<u32>`

3. `Mmio<T>` field access via:
   - `Deref<Target = T>`

4. Wrapper-call syntax such as:

```rust
regs.lcr_h.modify(LCRH_REG::WLEN::WLEN_8 + LCRH_REG::FEN::SET);
```

5. Polling syntax such as:

```rust
poll::until(
    || regs.fr.read(FR_REG::BUSY::FIELD) == 0,
    Duration::from_us(10),
    Duration::from_ms(100),
)
```

### Lessons confirmed by the experiment

- `Mmio<T>: Deref<Target = T>` is sufficient to make the authored field-access
  syntax work
- the generated register module + marker type + `Field` / `FieldValue`
  constants are enough to connect macro output to wrapper method arguments
- keeping width explicit in wrapper field types is mechanically simple and
  coherent
- `FieldValue` composition is a required part of the design, not optional
- the inert-attribute split remains sound:
  - helper macro is generative
  - `#[rseq::register_block(...)]` and `#[rseq::register(...)]` remain metadata
    only

### What the experiment did not settle

The experiment validated type-level coherence, but it did not yet settle:

- the final extraction strategy for recovering macro-generated bitfield metadata
  in `regseq-driver`
- the final authored syntax for field-level metadata on real register blocks
- the final internal representation of layout, side-effect, and relational
  metadata in the compiler
- the final production macro syntax and testing surface

So the experiment should be treated as confirmation of the minimal machinery
contract, not as the final production design in every detail.

## Current Working Position

The best current working position is:

- keep explicit register blocks
- keep bitfield schemas in `register_bitfields!`
- keep field-local metadata visible and extractable
- treat `register_bitfields!` as the only generative frontend piece
- use generated register marker types plus `Field` / `FieldValue` constants as
  the bridge between macro output and wrapper methods
- use `Mmio<T>: Deref<Target = T>` as the current field-access contract
- preserve enough structure for later legality, side-effect, and
  ordering-sensitive analysis
- defer `ordered!` itself to the later batch, but do not lose sight of it
- keep relational register metadata visible as a likely missing design area

That is the position this note records for further review.
