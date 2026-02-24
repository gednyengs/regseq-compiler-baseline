# Task: cargo-regseq-04 - Cargo invocation with RUSTC_WRAPPER

## Description

Wire everything together: parse CLI args, detect toolchain, and invoke
`cargo build` with `RUSTC_WRAPPER` set to the `regseq-driver` binary.
Pass Regseq-specific configuration via environment variables so the driver
can read them.

## Input Context

Read before starting:
- crates/cargo-regseq/src/lib.rs
- crates/cargo-regseq/src/cli.rs
- crates/cargo-regseq/src/toolchain.rs
- crates/cargo-regseq/README.md

## Output Specification

Create:
- crates/cargo-regseq/src/invoke.rs

Modify:
- crates/cargo-regseq/src/lib.rs (add `pub mod invoke;`, wire into `run()`)

## Implementation Notes

Define:

```rust
pub fn invoke_cargo(args: &BuildArgs, toolchain: &ToolchainInfo) -> Result<()>
```

This function:
1. Builds a `Command` for `cargo build`
2. Sets environment variables:
   - `RUSTC_WRAPPER` = path to regseq-driver
   - `REGSEQ_TARGET_OS` = target OS string ("linux" or "zephyr")
   - `REGSEQ_PLATFORM_FILE` = absolute path to platform file
   - `REGSEQ_OUTPUT_DIR` = absolute path to output directory
   - `REGSEQ_SYSROOT` = sysroot path (driver needs this for passthrough mode)
3. Appends any pass-through cargo args from `BuildArgs::cargo_args`
4. Creates the output directory if it does not exist
5. Runs the command, inheriting stdout/stderr (so the user sees cargo output)
6. Checks the exit status and returns an error on failure

Update `run()` in `lib.rs` to:
1. Parse CLI args
2. Detect toolchain
3. Call `invoke_cargo`

Handle the case where regseq-driver is not found with a clear error message
suggesting how to install/build it.

## Test Plan

Create:
- crates/cargo-regseq/tests/invoke.rs

Tests to implement:
1. Unit test `invoke_cargo` with a mock driver (a simple shell script or binary
   that writes its environment variables to a file). Verify that RUSTC_WRAPPER,
   REGSEQ_TARGET_OS, REGSEQ_PLATFORM_FILE, REGSEQ_OUTPUT_DIR are set correctly.
   Note: this test should not invoke actual cargo -- test the Command construction
   by exposing a `build_cargo_command` helper that returns the Command without
   running it, and inspect its env/args.
2. Verify output directory is created when it does not exist (use temp dir).
3. Verify platform_file path is absolutified even if given as relative.

Test command: `cargo test -p cargo-regseq`

## Acceptance Criteria

- All tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace` clean
- Running `cargo run -p cargo-regseq -- regseq build --target-os linux --platform-file <any.toml>`
  produces a clear error about regseq-driver not being found (since it is not
  built yet), not a crash or confusing message
