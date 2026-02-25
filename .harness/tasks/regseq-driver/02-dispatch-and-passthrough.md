# Task: regseq-driver-02 - Dispatch logic and passthrough

## Description

Implement the dispatch logic that determines whether regseq-driver should
run the regseq pipeline or act as a transparent rustc passthrough. Also
implement sysroot injection from the `REGSEQ_SYSROOT` environment variable
and handle cargo probe queries (`--print`, `-vV`).

The dispatch uses four signals (checked in order):
1. Is this a `--print` or `-vV` query? -> passthrough
2. Is `--target` absent? -> passthrough (build script or proc-macro)
3. Is `CARGO_PRIMARY_PACKAGE` unset? -> passthrough (dependency)
4. Is `REGSEQ_OPTIONS` absent? -> passthrough (not invoked via cargo-regseq)
5. All checks pass -> regseq mode

Background: cargo-regseq always passes `--target <host>` to cargo (task
cargo-regseq-05). Cargo forwards `--target` to rustc only for target crates.
Build scripts and proc-macros are compiled for the host and never receive
`--target`. This gives the driver a reliable structural signal.

## Input Context

Read before starting:
- crates/regseq-driver/src/lib.rs
- crates/regseq-driver/src/main.rs
- crates/regseq-driver/README.md

## Output Specification

Create:
- crates/regseq-driver/src/args.rs
- crates/regseq-driver/src/callbacks.rs

Modify:
- crates/regseq-driver/src/lib.rs (add modules, update `run()`, remove `DefaultCallbacks`)

## Implementation Notes

### args.rs -- Argument parsing and dispatch signals

When cargo uses `RUSTC_WRAPPER`, it invokes the wrapper with rustc as the
first argument:

```
regseq-driver /path/to/rustc --crate-name foo --edition 2021 ...
```

Parse the incoming arguments to extract:
- The path to the actual rustc binary (first arg after the binary name)
- The remaining rustc arguments

```rust
/// Parsed command-line arguments for the driver.
pub struct DriverArgs {
    /// Path to the rustc binary (from RUSTC_WRAPPER invocation).
    /// None if invoked directly (not as a wrapper).
    pub rustc_path: Option<String>,
    /// Arguments to pass to rustc.
    pub rustc_args: Vec<String>,
}
```

Implement `DriverArgs::parse_from(args: &[String])` which accepts a
pre-collected argument slice (including args[0], the binary name):
1. Skip the binary name (args[0]).
2. If args[1] does not start with `-`, treat it as the rustc path
   (wrapper mode). The remaining args are rustc_args. This heuristic
   is safe because rustc paths are always absolute filesystem paths
   (e.g., `/usr/bin/rustc`, `~/.rustup/.../rustc`).
3. If args[1] starts with `-`, treat this as a direct invocation.
   rustc_path is None, all args are rustc_args.

In production, `run()` calls `parse_from(&std::env::args().collect::<Vec<_>>())`.
Tests call `parse_from` directly with controlled argument vectors.

Implement `DriverArgs::dispatch(&self) -> DriverMode`:

```rust
pub enum DriverMode {
    /// Forward to rustc unchanged (cargo probe, dependency, etc.)
    Passthrough,
    /// Run the regseq compilation pipeline.
    Regseq,
}
```

The dispatch checks, in order:
1. **Cargo probe query**: Any arg is `-vV`, or any `--print` arg is present
   (check for `--print` or `--print=...` in rustc_args). Cargo sends these
   to discover compiler capabilities. The driver must not interfere.
2. **No `--target` flag**: Check rustc_args for `--target` or `--target=...`.
   If absent, this is a host crate (build script or proc-macro).
3. **`CARGO_PRIMARY_PACKAGE` not set**: Check `std::env::var`. If absent,
   this is a dependency crate.
4. **`REGSEQ_OPTIONS` not set**: Check `std::env::var`. If absent, the
   driver was not invoked through cargo-regseq.
5. All checks pass -> `DriverMode::Regseq`.

Provide helper functions. These must be `pub` because they are tested
from integration tests in `tests/args.rs`:

```rust
/// Check if an argument flag is present (e.g., "--target", "-vV").
pub fn has_arg(args: &[String], flag: &str) -> bool

/// Get the value of an argument (supports --arg=value and --arg value).
pub fn arg_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str>
```

### callbacks.rs -- Callbacks structs

Define two `rustc_driver::Callbacks` implementations:

```rust
/// Passthrough callbacks -- no-op, lets rustc run normally.
pub struct PassthroughCallbacks;

impl rustc_driver::Callbacks for PassthroughCallbacks {}
```

```rust
/// Regseq callbacks -- hooks into the compilation pipeline.
pub struct RegseqCallbacks;

impl rustc_driver::Callbacks for RegseqCallbacks {
    fn after_analysis<'tcx>(
        &mut self,
        _compiler: &rustc_interface::interface::Compiler,
        _tcx: rustc_middle::ty::TyCtxt<'tcx>,
    ) -> rustc_driver::Compilation {
        // Placeholder: will be filled in by subsequent tasks.
        // For now, just stop compilation (no codegen needed).
        rustc_driver::Compilation::Stop
    }
}
```

`PassthroughCallbacks` is the empty impl -- rustc runs normally.

`RegseqCallbacks` is the skeleton that will be expanded in task 03. For
now it only implements `after_analysis` returning `Compilation::Stop`
(prevents codegen, which we never need).

### Updated run()

Replace `run()` in `lib.rs` with the full dispatch logic. Remove
`DefaultCallbacks` (replaced by `PassthroughCallbacks`):

```rust
pub fn run() -> i32 {
    let early_dcx = rustc_session::EarlyDiagCtxt::new(
        rustc_session::config::ErrorOutputType::default(),
    );
    rustc_driver::init_rustc_env_logger(&early_dcx);
    rustc_driver::install_ice_hook(BUG_REPORT_URL, |_| {});

    rustc_driver::catch_with_exit_code(|| {
        let args = args::DriverArgs::parse_from(
            &std::env::args().collect::<Vec<_>>(),
        );

        // Build the rustc argument list.
        let mut rustc_args = Vec::new();
        if let Some(ref path) = args.rustc_path {
            rustc_args.push(path.clone());
        }
        rustc_args.extend(args.rustc_args.iter().cloned());

        // Inject --sysroot from REGSEQ_SYSROOT if not already present.
        if let Ok(sysroot) = std::env::var("REGSEQ_SYSROOT") {
            if !args::has_arg(&rustc_args, "--sysroot") {
                rustc_args.push("--sysroot".to_string());
                rustc_args.push(sysroot);
            }
        }

        // Dispatch based on signals.
        match args.dispatch() {
            args::DriverMode::Passthrough => {
                rustc_driver::run_compiler(&rustc_args, &mut callbacks::PassthroughCallbacks);
            }
            args::DriverMode::Regseq => {
                rustc_driver::run_compiler(&rustc_args, &mut callbacks::RegseqCallbacks);
            }
        }
    })
}
```

This ensures the driver uses the correct sysroot even when invoked through
RUSTC_WRAPPER, where the sysroot might not be passed automatically.

## Test Plan

Create:
- crates/regseq-driver/tests/args.rs

Tests for `DriverArgs::parse_from()`:
1. Parse wrapper-mode args: `parse_from(&["regseq-driver", "/usr/bin/rustc",
   "--crate-name", "foo"])` -> rustc_path is Some("/usr/bin/rustc"),
   rustc_args is ["--crate-name", "foo"]
2. Parse direct-invocation args: `parse_from(&["regseq-driver", "--crate-name",
   "foo"])` -> rustc_path is None, rustc_args is ["--crate-name", "foo"]

Tests for dispatch logic:

Since `dispatch()` reads environment variables, extract the signal-reading
into a separate `DispatchSignals` struct so the pure logic can be tested
with controlled inputs (no env var manipulation needed). `dispatch()`
constructs a `DispatchSignals` from args + env, then delegates to `mode()`:

```rust
pub struct DispatchSignals {
    pub has_print_query: bool,
    pub has_target: bool,
    pub has_cargo_primary_package: bool,
    pub has_regseq_options: bool,
}

impl DispatchSignals {
    pub fn mode(&self) -> DriverMode { ... }
}
```

Tests for DispatchSignals::mode():
3. All false -> Passthrough (no signals at all)
4. has_print_query=true, rest true -> Passthrough (probe query)
5. has_target=false, rest true -> Passthrough (host crate)
6. has_cargo_primary_package=false, rest true -> Passthrough (dependency)
7. has_regseq_options=false, rest true -> Passthrough (not via cargo-regseq)
8. All true -> Regseq
9. has_print_query via `--print=sysroot` in args -> detected correctly
10. has_print_query via `-vV` in args -> detected correctly
11. has_target via `--target=x86_64-unknown-linux-gnu` (= form) -> detected
12. has_target via `--target x86_64-unknown-linux-gnu` (space form) -> detected

Tests for helper functions:
13. has_arg finds `--target` in args
14. has_arg returns false when flag is absent
15. arg_value extracts value from `--target=foo` form
16. arg_value extracts value from `--target foo` form
17. arg_value returns None when flag is absent

Test command: `cargo test -p regseq-driver`

## Acceptance Criteria

- All 17 tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace` clean
