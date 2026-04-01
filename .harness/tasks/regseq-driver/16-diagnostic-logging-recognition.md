# Task: regseq-driver-16 - Diagnostic logging recognition

## Description

Implement the narrow compiler-facing extraction scaffolding for the accepted
diagnostic logging surface. This task should prove `regseq-driver` can recover
the logging frontend shape without introducing backend format lowering or
device-context injection behavior.

## Input Context

Read before starting:

- crates/regseq-driver/src/extract/interrupts.rs
- crates/regseq-driver/src/extract/traits.rs
- crates/regseq-driver/src/extract/mod.rs
- docs/compiler-collateral/compiler-baseline-overview.md
- docs/compiler-collateral/constructs/diagnostic-logging.md
- docs/discussions/regseq-intrinsics/regseq-intrinsics-discussion.md

## Output Specification

Create:

- crates/regseq-driver/src/extract/logging.rs
- crates/regseq-driver/tests/logging.rs

Modify:

- crates/regseq-driver/src/extract/mod.rs

## Implementation Notes

Recognize the accepted logging frontend surface in the bounded Phase-B way that
best matches the revised canonical contract:

- canonical logging anchors under
  `regseq::intrinsics::diagnostic_logging::{error,warn,info,debug}`
- preserve authored log level as one of:
  - `error`
  - `warn`
  - `info`
  - `debug`
- recover the surrounding function/method name as source context for later
  device-identity validation

This task may recognize the expanded anchor-call pattern rather than the raw
macro token spelling if that is the more stable HIR-visible shape.

For this batch, extraction should treat the canonical logging call as carrying:

- a template string literal
- a fixed-size array of typed `LogArg` carriers

Do not build this task around `core::fmt::Arguments<'_>`.

The extraction output should remain internal to `regseq-driver`. A narrow
record such as `LoggingSurface` / `LogCallSite` is appropriate.

Required capture details:

- log level
- containing function/method name
- whether the logging call came from the canonical anchor path
- enough argument metadata to distinguish:
  - the template string literal
  - number of dynamic arguments
  - typed `LogArg` kinds used at the call site

Required structural validations for this task:

- reject templates outside the accepted restricted logging subset
- reject unsupported stable argument classes outside this accepted primitive
  set and preserve the recognized `LogArg` kind for accepted ones:
  - integers: `u8`, `u16`, `u32`, `u64`, `usize`, `i8`, `i16`, `i32`, `i64`,
    `isize`
  - `bool`
  - `&str`
  - raw pointers / pointer-like values intended for `%p`-style lowering
- reject mismatch between placeholder count and argument count
- preserve `debug` calls as valid even though later deployment toggles may
  elide them

The accepted placeholder/modifier subset for this batch is:

- `{}`
- `{:x}`
- `{:X}`
- `{:#x}`
- `{:#X}`
- `{:p}`
- `{:0widthx}`
- `{:0widthX}`
- `{:#0widthx}`
- `{:#0widthX}`

where `width` is a decimal width encoded in the template literal. Representative
accepted forms include `{:08x}` and `{:#010x}`.

Keep this task bounded:

- do not implement device-context injection
- do not implement `debug_logging` deployment gating
- do not lower the captured logging surface into backend-native formats yet
- do not attempt full join validation that a driver identity is available;
  preserve source context for later checks instead

## Test Plan

Create:

- crates/regseq-driver/tests/logging.rs

Tests to implement:

1. Verify the four accepted logging levels are recognized.
2. Verify a template literal plus primitive dynamic arguments is captured.
3. Verify unsupported placeholder forms are rejected deterministically.
4. Verify unsupported non-primitive argument types are rejected
   deterministically.
5. Verify placeholder-count mismatches are rejected deterministically.
6. Verify a direct canonical anchor call is recognized in addition to the
   macro-authored surface.
7. Verify logging calls remain recognizable in both trait-callback and ISR
   bodies.
8. Verify accepted zero-padded hex width forms such as `{:08x}` and
   `{:#010x}` are recognized.

Test command: `cargo test -p regseq-driver --test logging`

## Acceptance Criteria

- `regseq-driver` can reach the accepted logging frontend forms
- the task stays limited to frontend recognition and bounded structural
  validation
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
