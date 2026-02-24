# Task: cargo-regseq-03 - Toolchain and sysroot detection

## Description

Implement toolchain and sysroot detection for cargo-regseq. The tool needs to:
1. Find the pinned nightly rustc (from `rust-toolchain.toml`)
2. Determine the sysroot path for that toolchain
3. Locate the `regseq-driver` binary

## Input Context

Read before starting:
- crates/cargo-regseq/src/lib.rs
- crates/cargo-regseq/src/cli.rs
- crates/cargo-regseq/README.md
- rust-toolchain.toml

## Output Specification

Create:
- crates/cargo-regseq/src/toolchain.rs

Modify:
- crates/cargo-regseq/src/lib.rs (add `pub mod toolchain;`)

## Implementation Notes

Define:

```rust
pub struct ToolchainInfo {
    /// Path to the rustc binary for the pinned nightly
    pub rustc: PathBuf,

    /// Sysroot path for the pinned nightly
    pub sysroot: PathBuf,

    /// Path to the regseq-driver binary
    pub driver: PathBuf,
}
```

Implement `ToolchainInfo::detect() -> Result<ToolchainInfo>`:

1. **Find rustc**: run `rustup which rustc` and capture stdout. This respects
   the `rust-toolchain.toml` in the current directory. Verify the binary exists.

2. **Find sysroot**: run `rustc --print sysroot` (using the rustc found above)
   and capture stdout. Trim whitespace.

3. **Find regseq-driver**: look in these locations in order:
   a. `REGSEQ_DRIVER` environment variable (if set)
   b. Same directory as the `cargo-regseq` binary itself (`std::env::current_exe()`)
   c. On `$PATH` via `which regseq-driver`
   If not found in any location, return an error with a helpful message.

All subprocess calls should capture stderr and include it in error messages
on failure.

## Test Plan

Create:
- crates/cargo-regseq/tests/toolchain.rs

Tests to implement:
1. `ToolchainInfo::detect()` succeeds when run from the repo root (finds
   pinned nightly via rust-toolchain.toml). Verify `rustc` path exists and
   `sysroot` path is a directory. Note: regseq-driver won't exist yet, so
   this test should verify rustc and sysroot detection work, but expect the
   driver lookup to fail gracefully.
2. Test with `REGSEQ_DRIVER` env var set to a temp file; verify it picks up
   the env var path.
3. Test error message when rustup is not available (if feasible; otherwise
   skip this test and document why).

Test command: `cargo test -p cargo-regseq`

## Acceptance Criteria

- Tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace` clean
