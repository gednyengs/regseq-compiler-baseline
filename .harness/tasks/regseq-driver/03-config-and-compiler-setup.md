# Task: regseq-driver-03 - Configuration and compiler setup

## Description

Implement `RegseqCallbacks::config()` to configure the compiler for regseq
mode, and deserialize `REGSEQ_OPTIONS` into `CompilerOptions` so the driver
knows what to do. This task makes the regseq path fully configured but does
not yet implement the analysis pipeline (that comes later).

## Input Context

Read before starting:
- crates/regseq-driver/src/lib.rs
- crates/regseq-driver/src/args.rs
- crates/regseq-driver/src/callbacks.rs
- crates/regseq-config/src/options.rs
- crates/regseq-config/src/types.rs

## Output Specification

Modify:
- crates/regseq-driver/src/callbacks.rs (add config hook, hold CompilerOptions)
- crates/regseq-driver/src/lib.rs (deserialize options before creating callbacks)
- crates/regseq-driver/Cargo.toml (add `serde_json` dependency)

## Implementation Notes

### Deserialize REGSEQ_OPTIONS

In `run()`, when the dispatch result is `Regseq`, deserialize the
`REGSEQ_OPTIONS` environment variable into `CompilerOptions`:

```rust
let options_json = std::env::var("REGSEQ_OPTIONS")
    .expect("REGSEQ_OPTIONS must be set in regseq mode");
let options: regseq_config::CompilerOptions =
    serde_json::from_str(&options_json).unwrap_or_else(|e| {
        panic!("REGSEQ_OPTIONS contains invalid JSON: {e}\nraw: {options_json}");
    });
```

The first `.expect()` is safe because `dispatch()` already verified that
`REGSEQ_OPTIONS` is present before returning `Regseq` (the env var is
re-read here for its value; dispatch only checked presence). The second
unwrap handles JSON parse failures with a helpful error message that
includes both the serde error and the raw JSON.

Update the `Regseq` arm in `run()` to deserialize options and pass them
into `RegseqCallbacks::new()`:

```rust
args::DriverMode::Regseq => {
    let options_json = std::env::var("REGSEQ_OPTIONS")
        .expect("REGSEQ_OPTIONS must be set in regseq mode");
    let options: regseq_config::CompilerOptions =
        serde_json::from_str(&options_json).unwrap_or_else(|e| {
            panic!("REGSEQ_OPTIONS contains invalid JSON: {e}\nraw: {options_json}");
        });
    rustc_driver::run_compiler(
        &rustc_args,
        &mut callbacks::RegseqCallbacks::new(options),
    );
}
```

### Update RegseqCallbacks

Add `CompilerOptions` to the struct:

```rust
pub struct RegseqCallbacks {
    options: regseq_config::CompilerOptions,
}

impl RegseqCallbacks {
    pub fn new(options: regseq_config::CompilerOptions) -> Self {
        Self { options }
    }

    /// Access the compiler options. Used by the pipeline (later tasks)
    /// and by integration tests to verify construction.
    pub fn options(&self) -> &regseq_config::CompilerOptions {
        &self.options
    }
}
```

### config() hook

Implement `config()` on `RegseqCallbacks`. This runs before the compiler
starts and configures it for regseq mode:

```rust
fn config(&mut self, config: &mut rustc_interface::interface::Config) {
    // 1. Disable codegen -- we extract from TyCtxt, no LLVM needed.
    config.opts.unstable_opts.no_codegen = true;

    // 2. Register the rseq tool attribute namespace so #[rseq::driver(...)],
    //    #[rseq::mmio(...)], etc. survive to HIR as inert tool attributes
    //    instead of causing "unknown attribute" errors.
    config
        .opts
        .unstable_opts
        .crate_attr
        .push("feature(register_tool)".to_string());
    config
        .opts
        .unstable_opts
        .crate_attr
        .push("register_tool(rseq)".to_string());

    // 3. Disable MIR optimizations -- we need unoptimized MIR for analysis.
    config.opts.unstable_opts.mir_opt_level = Some(0);

    // 4. Always encode MIR -- ensures MIR is available for all items.
    config.opts.unstable_opts.always_encode_mir = true;
}
```

These settings match the patterns used by Charon and Clippy:
- `no_codegen`: Charon sets this for the target crate. We never need LLVM.
- `register_tool(rseq)`: The old regseq-driver used this exact approach.
  Injecting synthetic crate attributes via `crate_attr` is the standard way
  to register tool attribute namespaces from a driver.
- `mir_opt_level = 0`: Both Charon and Clippy disable MIR optimizations.
  We need unoptimized MIR to accurately map back to source constructs.
- `always_encode_mir = true`: Charon sets this on all crates (even deps).
  We only need it for the target crate.

### after_analysis() -- error propagation

Update `after_analysis` to log the compiler options for now (placeholder
for the pipeline). The key improvement over the old driver: do NOT ignore
errors.

```rust
fn after_analysis<'tcx>(
    &mut self,
    _compiler: &rustc_interface::interface::Compiler,
    _tcx: rustc_middle::ty::TyCtxt<'tcx>,
) -> rustc_driver::Compilation {
    // Placeholder: log that we reached the regseq pipeline entry point.
    // The actual pipeline (HIR extraction, analysis, codegen) will be
    // added in subsequent tasks.
    eprintln!(
        "regseq-driver: reached after_analysis for target_os={:?}",
        self.options.target_os,
    );
    rustc_driver::Compilation::Stop
}
```

### Serde dependency

Add `serde_json` to `[dependencies]` in Cargo.toml:

```toml
[dependencies]
regseq-config = { path = "../regseq-config" }
serde_json = "1"
```

## Test Plan

Add to crates/regseq-driver/tests/args.rs (or create a new test file):

Testing the `config()` hook directly is not practical since it requires a
full `rustc_interface::Config`. Instead, test the deserialization and the
callbacks struct construction:

1. Deserialize a valid REGSEQ_OPTIONS JSON string into CompilerOptions;
   verify fields match.
2. Deserialize an invalid JSON string; verify it returns an error.
3. Construct `RegseqCallbacks::new(options)`; verify `cb.options()`
   returns the expected `CompilerOptions`.
4. Verify `CompilerOptions` from regseq-config round-trips through
   serde_json (serialize then deserialize; this confirms the driver can
   read what cargo-regseq writes).

Test command: `cargo test -p regseq-driver`

## Acceptance Criteria

- All new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace` clean
- (Optional manual check) Running `cargo regseq --target-os linux
  --platform-file <any.toml>` with `REGSEQ_DRIVER` pointing to the built
  binary produces the placeholder "reached after_analysis" message for the
  target crate. This requires a valid platform TOML and a crate to compile;
  it is not part of the automated test suite.
