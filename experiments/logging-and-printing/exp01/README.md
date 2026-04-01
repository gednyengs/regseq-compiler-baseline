# Logging And Printing Experiment 01

## Purpose

This experiment tests the current leading candidate for Regseq logging:

- authored surface remains macro-based and Rust-like
- the canonical compiler-facing form is:
  - log level
  - template string literal
  - typed argument carriers
- no `core::fmt::Arguments<'_>`

The experiment is intentionally small. It does not try to settle the final
macro crate placement or the final backend codegen strategy. It only proves the
mechanical viability of the canonical shape and a restricted lowering story.

## What It Tests

1. Authored syntax:

```rust
regseq::log::info!("DMA channel {} acquisition failed", channel);
```

2. Canonical expansion shape:

```rust
regseq::intrinsics::diagnostic_logging::info(
    "DMA channel {} acquisition failed",
    [LogArg::U32(channel)],
)
```

3. Restricted formatting subset:

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

4. Translation of the template + `LogArg` kinds into backend-native printf-like
   format strings for:

- Linux
- Zephyr
- Windows
- FreeBSD
- macOS

## How To Run

From the repo root:

```sh
cargo test --manifest-path experiments/logging-and-printing/exp01/Cargo.toml
```

## What To Look For

- whether the authored macro surface is easy to use
- whether the canonical form is simpler than `fmt::Arguments`
- whether the restricted formatting subset is enough for representative PL011
  diagnostics
- whether backend-native format strings fall out cleanly from:
  - template literal
  - log level
  - `LogArg` variants

## Expected Lessons

- whether one-anchor-per-level plus `LogArg[]` is a good canonical contract
- whether implicit placeholders are sufficient
- whether the initial formatting subset should stay very small
- what to record in the next revision of the Phase-B logging task specs
