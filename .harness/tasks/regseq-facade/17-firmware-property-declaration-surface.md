# Task: regseq-facade-17 - Firmware property declaration surface

## Description

Implement the facade-side declaration baseline for accepted firmware-property
 metadata. This task exists to make the declaration forms authorable from a
 `regseq`-using crate and to prove the compiler-managed property surface can be
 written without inventing any new wrapper or runtime API.

This is a declaration-surface task, not a typed-resource or intrinsic task.

## Input Context

Read before starting:

- crates/regseq/src/lib.rs
- docs/compiler-collateral/compiler-baseline-overview.md
- docs/compiler-collateral/constructs/firmware-property-access.md
- docs/compiler-collateral/compiler-constraints.md

## Output Specification

Create:

- crates/regseq/tests/property.rs

Modify only if needed:

- crates/regseq/src/lib.rs

## Implementation Notes

The accepted firmware-property Layer 1 surface is:

```rust
struct Pl011 {
    #[rseq::property(id = "reg-io-width", default = 4)]
    reg_io_width: u32,

    #[rseq::property(id = "current-speed")]
    current_speed: Option<u32>,

    #[rseq::property(id = "periphid", vendor_prefix = "arm")]
    periph_id: Option<u32>,
}
```

Important boundary for this task:

- `#[rseq::property(...)]` is a compiler-recognized inert tool attribute
- it is not a proc macro
- it is not a wrapper API
- `regseq` must not expose a driver-invoked `read_property` function or method
- required vs optional semantics come from field type:
  - `T` = required
  - `Option<T>` = optional

Phase-B declaration-side baseline to support:

- `#[rseq::property(id = "...")]`
- optional `default = ...`
- optional `vendor_prefix = "..."`
- field type drives required vs optional semantics

Use the existing crate-level `register_tool(rseq)` setup already present in
 `regseq`. For the integration test target itself, add the required crate
 attributes so the authored declaration forms compile:

```rust
#![feature(register_tool)]
#![register_tool(rseq)]
```

Keep this task strictly bounded:

- do not introduce any new `regseq` source module for properties
- do not invent a `Property<T>` wrapper type
- do not introduce a `read_property` API
- do not parse Layer 2 `[properties]`
- do not validate firmware/platform values
- do not generate init-preamble property population

If a tiny crate-doc clarification in `crates/regseq/src/lib.rs` helps explain
 that `#[rseq::property(...)]` is declarative and compiler-managed, that is
 acceptable, but this task should remain primarily test-driven.

## Test Plan

Create:

- crates/regseq/tests/property.rs

Tests to implement:

1. Verify representative `#[rseq::property(...)]` declarations compile on
   driver fields for:
   - required `u32` field with `id` and `default`
   - optional `Option<u32>` field with `id`
   - field using `vendor_prefix`
2. Verify required vs optional semantics remain purely type-driven by
   authoring both `T` and `Option<T>` fields under `#[rseq::property(...)]`.
3. Verify declared fields remain ordinary Rust fields after declaration; no
   special wrapper type is required to hold the property value.
4. Verify representative declarations compile without importing or using any
   helper property-read API beyond ordinary field declarations.

Test command: `cargo test -p regseq --test property`

## Acceptance Criteria

- the accepted firmware-property declaration shapes are authorable from a
  `regseq`-using crate
- the task does not invent any property wrapper or runtime property-read API
- the type-driven `T` vs `Option<T>` distinction is preserved as the authored
  declaration baseline
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
