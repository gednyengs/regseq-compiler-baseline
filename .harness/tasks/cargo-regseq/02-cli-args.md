# Task: cargo-regseq-02 - CLI argument parsing

## Description

Implement CLI argument parsing for `cargo regseq`. The subcommand supports:
- `cargo regseq build` -- build a Regseq driver crate
- Regseq-specific arguments: `--target-os`, `--platform-file`, `--output-dir`
- Pass-through arguments forwarded to `cargo build`

When cargo invokes a subcommand, it passes the subcommand name as the first
argument: `cargo-regseq regseq build --target-os linux`. The parser must
handle this by accepting and discarding the leading `regseq` token.

## Input Context

Read before starting:
- crates/cargo-regseq/src/lib.rs
- crates/cargo-regseq/src/main.rs
- crates/cargo-regseq/Cargo.toml
- crates/cargo-regseq/README.md

## Output Specification

Create:
- crates/cargo-regseq/src/cli.rs

Modify:
- crates/cargo-regseq/src/lib.rs (add `pub mod cli;`, wire into `run()`)

## Implementation Notes

Use `clap` with derive macros. Define:

```rust
pub struct Cli {
    pub command: Command,
}

pub enum Command {
    Build(BuildArgs),
}

pub struct BuildArgs {
    /// Target OS for code generation
    pub target_os: TargetOs,

    /// Path to the platform file (.toml)
    pub platform_file: PathBuf,

    /// Output directory for generated C files (default: ./regseq-out/)
    pub output_dir: PathBuf,

    /// Extra arguments passed through to cargo build
    pub cargo_args: Vec<String>,
}

pub enum TargetOs {
    Linux,
    Zephyr,
}
```

- `target_os` is required, accepts "linux" or "zephyr" (case-insensitive)
- `platform_file` is required, must be an existing file path
- `output_dir` defaults to `./regseq-out/`
- `cargo_args` captures everything after `--` as pass-through args

Update `run()` to parse CLI args via `Cli::parse()` and print a debug message
showing the parsed values (temporary -- will be replaced by actual invocation later).

## Test Plan

Create:
- crates/cargo-regseq/tests/cli.rs

Tests to implement:
1. Parse `["cargo-regseq", "regseq", "build", "--target-os", "linux", "--platform-file", "/tmp/test.toml"]`
   with a temp file created for platform_file; verify target_os is Linux and platform_file is correct
2. Parse with `--target-os zephyr`; verify target_os is Zephyr
3. Parse with `--target-os LINUX` (uppercase); verify case-insensitive parsing works
4. Parse with `--output-dir /tmp/out`; verify output_dir override
5. Parse without `--target-os`; verify error (required argument)
6. Parse without `--platform-file`; verify error (required argument)
7. Parse with extra args after `--`: `["...", "build", "--target-os", "linux",
   "--platform-file", "/tmp/test.toml", "--", "--release", "-v"]`;
   verify cargo_args contains ["--release", "-v"]

Note: for tests that need a platform file to exist, create a temporary file
using `tempfile` or `std::fs::File::create` in a temp directory.

Test command: `cargo test -p cargo-regseq`

## Acceptance Criteria

- All 7 tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace` clean
