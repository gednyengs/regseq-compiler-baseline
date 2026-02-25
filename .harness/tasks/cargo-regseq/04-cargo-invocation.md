# Task: cargo-regseq-04 - Cargo invocation with RUSTC_WRAPPER

## Description

Wire everything together: parse CLI args, detect toolchain, convert to
CompilerOptions, serialize to JSON, and invoke `cargo check` with
`RUSTC_WRAPPER` set to the `regseq-driver` binary.

## Input Context

Read before starting:
- crates/cargo-regseq/src/lib.rs
- crates/cargo-regseq/src/cli.rs
- crates/cargo-regseq/src/toolchain.rs
- crates/regseq-config/src/options.rs
- crates/cargo-regseq/README.md

## Output Specification

Create:
- crates/cargo-regseq/src/invoke.rs

Modify:
- crates/cargo-regseq/src/lib.rs (add `pub mod invoke;`, wire into `run()`)

## Implementation Notes

Define:

```rust
pub fn invoke_cargo(
    options: &CompilerOptions,
    toolchain: &ToolchainInfo,
    cargo_args: &[String],
    color: ColorChoice,
) -> Result<(), Error>
```

This function:
1. Builds a `Command` for `cargo check` (not `cargo build` -- we intercept
   via RUSTC_WRAPPER and don't need actual binary output)
2. Sets environment variables:
   - `RUSTC_WRAPPER` = path to regseq-driver
   - `REGSEQ_OPTIONS` = JSON-serialized CompilerOptions (via serde_json)
   - `REGSEQ_SYSROOT` = sysroot path (driver needs this for passthrough mode)
3. Forwards `--color` to cargo unless the user already passed `--color` in
   the trailing cargo args. Check for `--color` presence in `cargo_args`.
4. Appends any pass-through cargo args
5. Creates the output directory if it does not exist
6. Runs the command, inheriting stdout/stderr (so the user sees cargo output)
7. Checks the exit status and returns an error on failure

Only three env vars. The entire compiler configuration is carried in
`REGSEQ_OPTIONS` as a single JSON string. The driver deserializes it with
`serde_json::from_str::<CompilerOptions>()`.

`REGSEQ_SYSROOT` is separate because it's a driver-internal concern (needed
for passthrough mode when compiling build scripts and proc-macros), not a
compiler option.

### Exposing command for testing

Implement a `build_cargo_command(...)` helper that constructs the Command
without running it. `invoke_cargo` calls this internally. Tests inspect
the returned Command's env vars and args.

```rust
pub fn build_cargo_command(
    options: &CompilerOptions,
    toolchain: &ToolchainInfo,
    cargo_args: &[String],
    color: ColorChoice,
) -> Result<Command, Error>
```

### Wiring

Update `run()` in `lib.rs` to:
1. Parse CLI args via `CargoCli::parse()`, destructure to get `Cli`
2. Validate via `cli.validate()?`
3. Extract `cargo_args` and `color` before conversion
4. Convert to CompilerOptions via `cli.into_compiler_options()`
5. Detect toolchain
6. Call `invoke_cargo(&options, &toolchain, &cargo_args, color)`

Note: `color` is passed separately to `invoke_cargo` because it controls
cargo's own `--color` flag. It also appears inside `CompilerOptions` for
the driver's use. Extract it from `cli` before `into_compiler_options()`
consumes the struct.

Handle the case where regseq-driver is not found with a clear error message
suggesting how to install/build it.

## Test Plan

Create:
- crates/cargo-regseq/tests/invoke.rs

Tests to implement:
1. Build command with base options; verify `cargo check` is the command,
   RUSTC_WRAPPER is set, REGSEQ_SYSROOT is set.
2. Build command; verify REGSEQ_OPTIONS is valid JSON that deserializes
   to a CompilerOptions matching the input.
3. Build command with `--check` in options; verify the deserialized
   REGSEQ_OPTIONS has check=true.
4. Build command with emit specs; verify deserialized REGSEQ_OPTIONS
   has correct emit list.
5. Build command with params; verify deserialized REGSEQ_OPTIONS has
   correct params.
6. Build command with color=Always and no user `--color` in cargo_args;
   verify `--color always` appears in command args.
7. Build command with cargo_args containing `--color never`; verify the
   command does NOT add a second `--color`.
8. Build command with extra cargo_args `["--release", "-v"]`; verify they
   appear in the command args.
9. Verify output directory is created when it does not exist (use temp dir).
10. Verify CompilerOptions.platform_file is an absolute path.

Test command: `cargo test -p cargo-regseq`

## Acceptance Criteria

- All tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace` clean
- Running `cargo run -p cargo-regseq -- regseq --target-os linux --platform-file <any.toml>`
  produces a clear error about regseq-driver not being found (since it is not
  built yet), not a crash or confusing message
