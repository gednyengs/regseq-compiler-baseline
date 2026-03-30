# Diagnostic Logging

## Purpose

`Diagnostic Logging` defines the stable logging surface, debug-level deployment
 toggle, compiler-derived device-context injection, and compiler-managed module
 registration needed to lower portable driver logging.

This is an intrinsic-heavy construct. It has no trait, handler-binding, or
 typed-resource surface.

## Accepted Primitive Inventory

### Layer 1

| Primitive | Kind | Stable surface |
|---|---|---|
| `diagnostic_logging.log_error` | operation | `regseq::log::error!(...)` |
| `diagnostic_logging.log_warn` | operation | `regseq::log::warn!(...)` |
| `diagnostic_logging.log_info` | operation | `regseq::log::info!(...)` |
| `diagnostic_logging.log_debug` | operation | `regseq::log::debug!(...)` |

### Layer 2

| Primitive | Kind | Stable surface |
|---|---|---|
| `diagnostic_logging.debug_logging` | property | platform/hardware binding field |

### Compiler-managed

| Primitive | Role |
|---|---|
| `diagnostic_logging.register_log_module` | emit module/log-registration glue |

## Accepted Surface Baseline

Stable logging surface uses macros:

```rust
regseq::log::error!("DMA channel {} acquisition failed", channel);
regseq::log::warn!("buffer overflow on RX path");
regseq::log::info!("initialized with {} DMA channels", count);
regseq::log::debug!("status = {:#010x}", status);
```

Accepted decisions:

- macro surface, not function surface
- Rust-format-string input, compile-time translation to backend/native format
- logging is infallible and fire-and-forget
- `debug!` is conditionally compiled/elided based on `debug_logging`

## Extraction Baseline

The frontend must recognize:

- `regseq::log::error!`, `warn!`, `info!`, and `debug!` macro invocations
- format string literal plus arguments
- `debug_logging` in Layer 2 binding data

Primary specification signals:

- logging macro call sites
- Layer 2 `debug_logging` binding value

Required extraction validations:

- logging calls must occur in driver contexts where device identity is
  available
- `debug!` remains valid even when elided by `debug_logging = false`
- format-string shape must be validated at compile time for lowering
- only the accepted primitive Rust argument set is allowed:
  - integers: `u8`, `u16`, `u32`, `u64`, `usize`, `i8`, `i16`, `i32`, `i64`,
    `isize`
  - `bool`
  - `&str`
  - raw pointers and pointer-like values lowered as `%p`

## Lowering Baseline

Lowering ownership:

- log macros: backend lowers to target OS logging facilities
- device-context injection: compiler-derived from accepted Driver Lifecycle
  state and bus/runtime identity
- `register_log_module`: compiler-generated

### Generated lifecycle phase table

| Primitive | Init preamble | Deinit postamble | Suspend postamble | Resume preamble |
|---|---|---|---|---|
| `register_log_module` | no | no | no | no |

Accepted lowering notes:

- device context is injected implicitly; the driver does not pass a device
  handle
- format lowering is compile-time Rust-format to backend/native format
  translation with validation
- only primitive Rust argument types participate in the stable baseline; named
  arguments and trait-driven formatting remain deferred
- supported primitive lowering is:
  - integers via type-driven integer printf forms
  - `bool` as integer-style `0` / `1`
  - `&str` as `%s`
  - pointer-like values as `%p`
- debug logging is a deployment-specific Layer 2 toggle, not a Layer 1
  declaration

## Cross-Construct Dependencies

Depends on:

- `Driver Lifecycle` for compiler-derived `driver_name` context
- bus/resource identity for device-context enrichment

Related constraints:

- `nbcon` console protocol remains a carried-forward implementation constraint
- logging is not part of the shared `RegseqError` callback convention

## Constraints and Caveats

- Keep the macro surface. Do not revert to a function-style stable API.
- Preserve fully implicit device-context injection.
- Keep `debug_logging` as a deployment-specific Layer 2 property.
- Preserve compile-time format validation and translation.

## Representative Examples

```rust
regseq::log::error!("DMA pause failed: {}", reason);
regseq::log::debug!("mapped RX buffer at {:#x}", addr);
```

## Implementation Checklist

- Recognize the four stable logging macros.
- Lower macro calls to backend logging APIs with compile-time format
  translation.
- enforce compile-time argument-count and primitive-type validation during
  format lowering.
- reject named arguments and trait-driven formatting (`Display` / `Debug`)
  outside the accepted primitive set.
- Inject device context implicitly from accepted compiler state.
- Honor `debug_logging` when deciding whether `debug!` expands to code.
- Keep logging infallible and separate from callback error handling.
