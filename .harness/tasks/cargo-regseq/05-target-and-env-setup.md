# Task: cargo-regseq-05 - Force --target and clean inherited env

## Description

Ensure that `build_cargo_command()` always passes an explicit `--target`
flag to cargo and cleans `CARGO_PRIMARY_PACKAGE` from the inherited
environment. These two changes enable regseq-driver to reliably distinguish
target crates from build scripts and proc-macros.

Background: When `--target` is explicitly set on the cargo invocation, cargo
passes `--target <triple>` through to rustc only for target crates. Build
scripts and proc-macros are compiled for the host and do NOT receive
`--target`. This gives the driver a reliable signal for passthrough
decisions. Both Charon and Miri use this pattern.

## Input Context

Read before starting:
- crates/cargo-regseq/src/invoke.rs
- crates/cargo-regseq/src/toolchain.rs
- crates/cargo-regseq/tests/invoke.rs

## Output Specification

Modify:
- crates/cargo-regseq/src/toolchain.rs (add `host` field to ToolchainInfo)
- crates/cargo-regseq/src/invoke.rs (inject `--target`, remove env var)
- crates/cargo-regseq/tests/invoke.rs (update existing tests, add new tests)

## Implementation Notes

### Add host triple to ToolchainInfo

Add a `host` field to `ToolchainInfo`:

```rust
pub struct ToolchainInfo {
    pub rustc: PathBuf,
    pub sysroot: PathBuf,
    pub driver: PathBuf,
    /// Host triple (e.g., "x86_64-unknown-linux-gnu").
    pub host: String,
}
```

Detect it in `ToolchainInfo::detect()` by running:

```
rustc -vV
```

Parse stdout for the line starting with `host: ` and extract the triple.
Use the already-detected `rustc` path (not just `rustc` from PATH).

### Update build_cargo_command

Two changes to `build_cargo_command()`:

1. **Inject `--target` if not already present in `cargo_args`.** Check
   whether the user already passed `--target` in the trailing cargo args
   (same pattern as the existing `--color` check). If not, add
   `--target <toolchain.host>`. This must come after `check` and before
   user cargo args.

2. **Remove `CARGO_PRIMARY_PACKAGE` from the inherited environment.** Call
   `command.env_remove("CARGO_PRIMARY_PACKAGE")` so cargo sets it fresh
   for each crate. Without this, a stale inherited value could confuse
   the driver's primary-package detection.

The `build_cargo_command` signature gains access to the host triple via the
existing `toolchain: &ToolchainInfo` parameter (no signature change needed).

### Update existing tests

The `sample_options_and_toolchain()` helper must set the new `host` field.
Use a fixed test value like `"x86_64-unknown-linux-gnu"`.

Existing tests that assert on the full args list (e.g.,
`build_command_sets_cargo_check_and_wrapper_env`,
`build_command_adds_color_when_not_provided_by_user`,
`build_command_forwards_extra_cargo_args`) will need their expected args
updated to include `--target x86_64-unknown-linux-gnu`.

## Test Plan

Add to crates/cargo-regseq/tests/invoke.rs:

Tests to add:

1. Build command without `--target` in cargo_args; verify `--target` and
   the host triple appear in the command args.
2. Build command with `--target aarch64-unknown-linux-gnu` in cargo_args;
   verify the command does NOT add a second `--target` (user override).
3. Build command; verify `CARGO_PRIMARY_PACKAGE` is explicitly removed
   from the environment (check that `get_envs()` contains the key with
   a `None` value, which is how `env_remove` is represented).
4. Verify `ToolchainInfo::detect()` populates `host` with a non-empty
   string (integration test, same as existing toolchain tests).

Test command: `cargo test -p cargo-regseq`

## Acceptance Criteria

- All new and updated tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace` clean
