# Logging And Printing Discussion

## Purpose

This note captures the current Regseq discussion around diagnostic logging and
printing. The immediate goal is to avoid locking `phase-b-05` into a logging
surface that depends too heavily on Rust runtime formatting machinery when the
compiler ultimately needs to lower authored intent into direct target-OS logging
or printing mechanisms.

This discussion is intentionally separate from the current accepted construct
taxonomy so that we can reason about design alternatives and run experiments
before settling the stable frontend surface.

## Current Pressure Point

The current Phase-B logging task specs assume a macro surface like:

```rust
regseq::log::info!("DMA channel {} acquisition failed", channel);
```

expanding into canonical function-like anchors such as:

```rust
regseq::intrinsics::diagnostic_logging::info(::core::format_args!(...));
```

That approach is convenient from a Rust authoring perspective, but it introduces
several problems for Regseq:

- it ties the frontend contract to Rust's formatting infrastructure
- it pushes compiler extraction toward `format_args!` expansion details
- it suggests `Display` / `Debug`-style semantics that Regseq does not want to
  promise
- it makes direct lowering to OS-native logging/printing APIs less explicit

In other words: `core::fmt::Arguments<'_>` is a Rust formatting carrier, not a
Regseq-owned logging IR.

## Why This Matters

Regseq is not trying to specify a Rust runtime logging facility. The compiler
needs to capture logging intent in a form that is easier to lower into target
mechanisms such as:

- Linux `printk` / `dev_*`
- Zephyr logging or print APIs
- Windows kernel logging or printing mechanisms
- other backend-native diagnostic sinks

Many of these backends already have their own formatting model or their own
printf-like surface. That suggests Regseq should preserve:

- log level
- message template
- typed arguments
- source context

instead of preserving a Rust-owned formatted argument object.

## Guiding Question

The central design question for this discussion is:

> How should an authored logging surface such as:
>
> ```rust
> regseq::log::info!("DMA channel {} acquisition failed", channel);
> ```
>
> or a more explicit typed variant such as:
>
> ```rust
> regseq::log::info!("DMA channel {u32} acquisition failed", channel);
> ```
>
> be captured so that the compiler can lower it cleanly into direct
> backend-native mechanisms such as:
>
> - Linux: `printk(KERN_INFO "DMA channel %d acquisition failed", channel)`
> - Zephyr: the corresponding native logging/printing call
> - Windows: the corresponding kernel logging/printing call
> - FreeBSD/macOS: the corresponding native logging/printing call

This is the question we want researchers to debate. The goal is not merely to
make the macro compile. The goal is to choose a frontend contract that makes
cross-OS lowering clean, analyzable, and defensible.

## Researcher Synthesis

The first researcher round converged more strongly than expected.

What they broadly agree on:

- `core::fmt::Arguments<'_>` should be dropped from the stable Regseq contract
- logging should be captured as Regseq-owned structured intent
- the canonical compiler-facing form should preserve:
  - level
  - template
  - typed arguments
- the authored surface should remain macro-based and familiar

Most importantly, the researchers did **not** prefer explicit typed placeholders
as the primary authored syntax. They consistently preferred:

```rust
regseq::log::info!("DMA channel {} acquisition failed", channel);
```

over:

```rust
regseq::log::info!("DMA channel {u32} acquisition failed", channel);
```

Their reasoning is consistent:

- the argument type is already known to the compiler
- repeating type information in the template is redundant
- placeholder syntax should describe formatting intent, not duplicate type
- backend lowering needs:
  - the template
  - the argument type
  - the log level
  rather than an explicitly typed placeholder string

## Current Leading Candidate

The strongest current direction is now:

1. Keep macros as the authored ergonomic surface.
2. Make those macros lower into Regseq-owned logging intent, not
   `core::fmt::Arguments<'_>`.
3. Treat the compiler as capturing a structured logging event rather than a
   Rust formatting operation.
4. Keep the authored placeholder surface implicit, but define a **restricted
   Regseq formatting subset** rather than inheriting all of Rust formatting.

Conceptually, the authored surface stays:

```rust
regseq::log::info!("DMA channel {} acquisition failed", channel);
```

But the canonical compiler-facing form becomes more like:

```rust
regseq::intrinsics::diagnostic_logging::info(
    "DMA channel {} acquisition failed",
    &[LogArg::U32(channel)],
);
```

This candidate has the right shape for lowering because it makes the stable
contract be:

- template string literal
- typed arguments
- log level

instead of a Rust-owned formatting object.

## Why This Candidate Is Attractive

- it gives the compiler the exact information it needs for backend-native
  lowering
- it keeps the authored syntax simple and familiar
- it avoids dependence on Rust formatting internals
- it keeps formatting and typing as separate concerns:
  - the template carries formatting intent
  - the `LogArg` variant carries type identity

That separation is important. The placeholder or modifier answers:

- decimal vs hex
- prefix vs no prefix
- pointer formatting

while the typed argument carrier answers:

- `u32` vs `u64`
- signed vs unsigned
- pointer vs string

Those two facts combine during lowering to produce the target-native format
surface.

## Candidate Directions Under Consideration

These are not final decisions. They are candidate directions we want reviewed
and challenged.

### 1. Restricted Regseq-Owned Template Language

Keep the authored surface macro-based, but define a Regseq-owned formatting
sub-language rather than inheriting Rust formatting.

This could mean:

- implicit placeholders such as `{}` with type inferred from the argument
- explicit typed placeholders such as `{u32}`, `{i32}`, `{str}`, `{ptr}`,
  `{bool}`

After the first researcher round, the strongest candidate is:

- implicit placeholders at the authored surface
- with a closed, restricted set of accepted modifiers

Examples currently considered plausible:

- `{}`
- `{:x}`
- `{:X}`
- `{:#x}`
- `{:#X}`
- `{:p}`
- `{:08x}`
- `{:08X}`
- `{:#010x}`
- `{:#010X}`

A bounded form of zero-padded hex width is now attractive because it appears in
the accepted collateral examples and still lowers cleanly to backend-native
printf-like formats.

### 2. Canonical Logging Intrinsic With Structured Arguments

The macro would expand into a canonical intrinsic that carries:

- level
- template or template pieces
- placeholder kinds
- typed arguments

Possible shapes include:

- one anchor per level:
  - `error(template, args)`
  - `warn(template, args)`
  - `info(template, args)`
  - `debug(template, args)`
- one generic anchor:
  - `emit(level, template, args)`

After the first researcher round, the strongest candidate is:

- one anchor per level
- string-literal template
- typed argument carrier array/slice

This keeps path-based intrinsic recognition simple and preserves the authored
log level directly in the anchor identity.

### 3. Logging As Compiler-Captured Intent, Not Rust Formatting

The logging surface may be best understood as:

- authored syntax that marks a diagnostic event
- compiler capture of structured intent
- backend-specific weaving into the target logging or printing mechanism

This is the "aspect-like" interpretation discussed so far.

### 4. Narrow Accepted Argument Kinds

The accepted argument set should likely stay closed and explicit, for example:

- signed integers
- unsigned integers
- booleans
- string slices
- pointers

This is attractive because it maps naturally to backend-native format surfaces
and avoids trait-driven formatting.

The current leading closed set is:

- unsigned integers
- signed integers
- `bool`
- `&str`
- pointers

with the important caveat that dynamic `&str` handling may require extra care
at backend lowering time because C-style logging sinks expect null-terminated
strings.

## Aspect-Like Interpretation

Logging in Regseq is beginning to look less like an ordinary library call and
more like a compiler-captured event:

- the authored code marks a logging point
- the compiler captures structured intent
- backend codegen weaves that intent into the target's native logging or
  printing mechanism

This is "aspect-like" only in the sense that authored syntax marks a concern
whose concrete realization is backend-specific and compiler-managed.

## Open Design Questions

### 1. What is the canonical logging intrinsic shape?

Options include:

- one anchor per level:
  - `error(template, args)`
  - `warn(template, args)`
  - `info(template, args)`
  - `debug(template, args)`
- one generic `emit(level, template, args)` anchor

Current leaning:

- one anchor per level

Open question:

- whether the argument carrier should be a `LogArg` enum slice/array exactly,
  or something mechanically similar but easier for extraction/lowering

### 2. What argument carrier should Regseq own?

We likely need a small closed set such as:

- unsigned integers
- signed integers
- `bool`
- string slices
- pointers

Open question:
- should this be represented as an enum like `LogArg`
- or as multiple intrinsic overload families
- or via macro-generated typed wrappers

### 3. What formatting/template language should the authored surface allow?

We need something narrower than general Rust formatting.

Current leading direction:

- authored surface remains Rust-like with implicit `{}` placeholders
- Regseq defines a restricted accepted subset rather than inheriting all Rust
  formatting

Still open:

- exactly which modifiers belong in the initial subset beyond the currently
  leading set
- whether width/padding should remain limited to bounded zero-padded hex forms
  in the first accepted version

The constraint is:
- it must be easy for the compiler to validate
- it must be easy to lower across OS backends

### 4. Should the logging macros parse the format surface themselves?

If the macro is responsible for turning:

```rust
regseq::log::info!("x={} y={}", x, y)
```

into a Regseq-owned canonical call, then we need to decide:

- how much parsing the macro performs
- whether `regseq-macros` should own that parsing
- how the generated call preserves enough structure for `regseq-driver`

This question remains important even with implicit placeholders, because the
macro may be the place where:

- placeholder count is checked
- format modifiers are constrained to the accepted subset
- arguments are converted into typed Regseq-owned carriers

Current leaning:

- if the logging surface only needs ordinary `macro_rules!` behavior, prefer
  defining it directly in `regseq`
- reserve `regseq-macros` for cases that truly require proc-macro machinery

### 5. What is the relationship between logging and printing?

We should decide whether:

- "logging" and "printing" are one unified construct family with levels as an
  optional dimension
- or two related but distinct surfaces

This matters because some targets distinguish:

- structured logging
- plain character/printf emission

### 6. What belongs in Phase B vs later work?

For Phase B, we probably only need:

- a stable authored logging surface
- a canonical compiler-facing logging intent shape
- bounded extraction and validation

We do not need:

- full backend-native format lowering
- deployment toggles
- source-context injection
- advanced formatting features

## Planned Experiments

We should create small experiments under:

- `experiments/logging-and-printing/`

Candidate experiment themes:

1. macro expands into `level + template literal + LogArg[]`
2. one-anchor-per-level vs generic `emit(...)`
3. restricted placeholder grammar and validation strategy
4. implicit `{}` placeholders vs explicit typed placeholders
5. extraction shape from HIR after macro expansion
6. how to handle `&str` safely and portably across backend logging sinks
7. whether placeholder-count validation should happen in the macro, in
   `regseq-driver`, or both

## What We Want From Researcher Review

The next researcher round should not merely critique the current `fmt::Arguments`
story. It should propose candidate solutions to the guiding question above.

Specifically, each researcher should analyze:

1. what authored logging/printing surface would best support their OS family
2. whether placeholders should be implicit or explicit/typed
3. what canonical compiler-facing intrinsic shape would be easiest to lower
4. what closed set of argument kinds should be accepted
5. one concrete example of lowering from authored syntax to a native OS call

The goal is to gather candidate solutions and then synthesize the best common
design, not to defend the current provisional direction.

The purpose of these experiments is to prove mechanical viability before
updating the task specs or the accepted logging surface.

## Current Recommendation

Do not implement the current `fmt::Arguments`-based logging task specs yet.

The current leading candidate is:

- authored macros with implicit `{}`-style placeholders
- a restricted Regseq-defined formatting subset
- one canonical anchor per log level
- string-literal template plus typed Regseq-owned argument carriers

Next steps:

1. build one or more focused experiments around this leading candidate
2. settle the exact argument carrier and restricted placeholder subset
3. then revise the Phase-B logging tasks to match the chosen design

## Status

Open discussion with a leading candidate. The current `fmt::Arguments`-based
Phase-B direction should be treated as superseded pending this design work.
