# Task: regseq-facade-16 - Diagnostic logging anchors and re-exports

## Description

Implement the facade-side canonical logging intrinsic anchors and the user-facing
`regseq::log` re-export surface. This task owns the function-like anchor
declarations, the Regseq-owned typed argument carriers, the lightweight
`macro_rules!` logging surface, and the public export structure.

## Input Context

Read before starting:

- crates/regseq/src/lib.rs
- crates/regseq/src/intrinsics/mod.rs
- docs/compiler-collateral/compiler-baseline-overview.md
- docs/compiler-collateral/constructs/diagnostic-logging.md
- docs/discussions/regseq-intrinsics/regseq-intrinsics-discussion.md

## Output Specification

Create:

- crates/regseq/src/intrinsics/diagnostic_logging.rs
- crates/regseq/src/log.rs
- crates/regseq/tests/logging.rs

Modify:

- crates/regseq/src/intrinsics/mod.rs
- crates/regseq/src/lib.rs

## Implementation Notes

Establish canonical function-like anchors that the logging macros will expand
into.

Do not use `core::fmt::Arguments<'_>` in the stable contract. Instead, use a
Regseq-owned logging shape with:

- one anchor per log level
- string-literal template
- typed argument carriers

An acceptable Phase-B shape is:

```rust
pub mod intrinsics {
    pub mod diagnostic_logging {
        pub enum LogArg<'a> {
            U8(u8), U16(u16), U32(u32), U64(u64), Usize(usize),
            I8(i8), I16(i16), I32(i32), I64(i64), Isize(isize),
            Bool(bool),
            Str(&'a str),
            Ptr(*const ()),
        }

        #[inline(never)]
        #[rseq::intrinsic]
        pub fn error<'a, const N: usize>(
            template: &'static str,
            args: [LogArg<'a>; N],
        ) { unimplemented!() }

        #[inline(never)]
        #[rseq::intrinsic]
        pub fn warn<'a, const N: usize>(
            template: &'static str,
            args: [LogArg<'a>; N],
        ) { unimplemented!() }

        #[inline(never)]
        #[rseq::intrinsic]
        pub fn info<'a, const N: usize>(
            template: &'static str,
            args: [LogArg<'a>; N],
        ) { unimplemented!() }

        #[inline(never)]
        #[rseq::intrinsic]
        pub fn debug<'a, const N: usize>(
            template: &'static str,
            args: [LogArg<'a>; N],
        ) { unimplemented!() }
    }
}
```

Expose a user-facing `regseq::log` module that contains the accepted
`macro_rules!` logging macros directly in `regseq`.

Phase-B baseline:

- the stable authored surface is still macro-based:
  - `regseq::log::error!`
  - `regseq::log::warn!`
  - `regseq::log::info!`
  - `regseq::log::debug!`
- canonical compiler identity lives on the function-like logging anchors
- the anchors consume:
  - a template string literal
  - a fixed-size array of typed `LogArg` carriers
- the facade should define the closed accepted `LogArg` set for this batch:
  - unsigned integers
  - signed integers
  - `bool`
  - `&str`
  - pointers
- the `regseq::log` module should expose only macro re-exports, not ordinary
  wrapper functions
- for this batch, define the logging macros as lightweight `macro_rules!`
  helpers in `regseq`; do not introduce a proc-macro dependency for logging
- the canonical contract should be independent of Rust formatting carriers
- conversion from accepted primitive argument types into `LogArg` should be part
  of the facade-side support surface for this batch
- the accepted restricted template subset for this batch should include:
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
  where `width` is a decimal width encoded in the template literal

Do not include:

- device-context injection
- `debug_logging` deployment toggles
- backend/native format lowering
- `core::fmt::Arguments<'_>` or `Display` / `Debug` as the stable contract
- advanced format-language parsing beyond the restricted logging subset

## Test Plan

Create:

- crates/regseq/tests/logging.rs

Tests to implement:

1. Verify the canonical logging anchor paths are publicly available under
   `regseq::intrinsics::diagnostic_logging`.
2. Verify `regseq::log` exports the four accepted logging macros.
3. Verify representative logging calls compile through the `regseq::log::*`
   surface and lower into the canonical `template + LogArg[]` anchor shape.
4. Verify the facade does not expose ordinary logging wrapper functions under
   `regseq::log`.
5. Verify `core::fmt::Arguments<'_>` is not part of the canonical logging
   anchor signature.
6. Verify representative zero-padded hex forms such as `{:08x}` and
   `{:#010x}` compile through the facade surface.

Test command: `cargo test -p regseq --test logging`

## Acceptance Criteria

- the canonical logging anchor namespace exists under `regseq::intrinsics`
- `regseq::log` exists as the user-facing logging surface
- the task stays limited to facade anchors, typed carriers, and lightweight
  macro plumbing
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
