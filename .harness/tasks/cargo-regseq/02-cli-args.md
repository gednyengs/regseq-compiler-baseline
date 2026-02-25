# Task: cargo-regseq-02 - CLI argument parsing

## Description

Implement CLI argument parsing for `cargo regseq` using clap derive macros.
The type definitions for compiler options (TargetOs, EmitSpec, LogLevel,
DumpStage, RegseqParam, ColorChoice, CompilerOptions) live in regseq-config.
This task builds the clap layer on top and the conversion from parsed CLI
args to CompilerOptions.

Usage: `cargo regseq [OPTIONS] [-- CARGO_ARGS]`

No subcommand -- all options are top-level. When cargo invokes a subcommand,
it passes the subcommand name as the first argument:
`cargo-regseq regseq --target-os linux ...`. The parser must handle this by
accepting and discarding the leading `regseq` token.

## Input Context

Read before starting:
- crates/cargo-regseq/src/lib.rs
- crates/cargo-regseq/src/main.rs
- crates/cargo-regseq/Cargo.toml
- crates/regseq-config/src/lib.rs
- crates/regseq-config/src/types.rs
- crates/regseq-config/src/emit.rs
- crates/regseq-config/src/param.rs
- crates/regseq-config/src/options.rs

## Output Specification

Create:
- crates/cargo-regseq/src/cli.rs

Modify:
- crates/cargo-regseq/Cargo.toml:
  - add `regseq-config = { path = "../regseq-config" }` to dependencies
  - add `serde_json` as regular dep for CompilerOptions serialization
  - add `tempfile` as dev-dependency (for tests needing temporary files)
- crates/cargo-regseq/src/lib.rs (add `pub mod cli;`, wire into `run()`)
- crates/cargo-regseq/tests/smoke.rs (remove -- replaced by cli.rs tests)

## Implementation Notes

Use `clap` with derive macros. The regseq-config types implement `FromStr`
and `Display`, so clap can parse them directly.

### Error type

Replace the placeholder `Error` unit struct from task 01 with an error enum
using `thiserror`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Validation(String),

    #[error("{0}")]
    Toolchain(String),

    #[error("{0}")]
    Invoke(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
```

The `run()` function continues to return `Result<(), Error>`. Module-level
functions (cli, toolchain, invoke) return `Result<T, Error>` using this enum.
Do not use `anyhow` -- keep typed errors throughout.

### Cargo subcommand wrapper

Cargo invokes subcommands by passing the subcommand name as the first
argument: `cargo-regseq regseq --target-os linux ...`. Use the standard
clap pattern for cargo subcommands:

```rust
#[derive(clap::Parser)]
#[command(name = "cargo", bin_name = "cargo")]
pub enum CargoCli {
    /// Regseq compiler driver.
    Regseq(Cli),
}
```

Parse via `CargoCli::parse()`, then destructure to get the inner `Cli`:

```rust
let CargoCli::Regseq(cli) = CargoCli::parse();
```

Tests use `CargoCli::try_parse_from(["cargo-regseq", "regseq", ...])`.

### Struct definition

```rust
use regseq_config::{
    TargetOs, EmitSpec, LogLevel, DumpStage, RegseqParam,
    ColorChoice, CompilerOptions,
};

/// cargo regseq -- Regseq compiler driver.
pub struct Cli {
    // --- Core options ---

    /// Target OS for code generation
    pub target_os: TargetOs,

    /// Path to the platform file (.toml)
    pub platform_file: PathBuf,

    /// Output directory for generated artifacts (default: ./regseq-out/)
    pub output_dir: PathBuf,

    // --- Output control ---

    /// What to emit (repeatable; default: driver)
    pub emit: Vec<EmitSpec>,

    /// Validate without writing output files
    pub check: bool,

    // --- Analysis control ---

    /// Additional analyses to run (repeatable)
    pub add_analysis: Vec<String>,

    /// Analyses to skip from the default set (repeatable)
    pub skip_analysis: Vec<String>,

    // --- Diagnostic control ---

    /// Treat this diagnostic as an error (repeatable)
    pub deny: Vec<String>,

    /// Suppress this diagnostic (repeatable)
    pub allow: Vec<String>,

    // --- Parameters ---

    /// Key-value parameters for compiler and backends (repeatable)
    pub params: Vec<RegseqParam>,

    // --- Debug/logging ---

    /// Set log level (default: warn)
    pub log_level: LogLevel,

    /// Enable verbose output (shorthand for --log-level debug)
    pub verbose: bool,

    /// Dump compiler state at a pipeline stage
    pub dump_stage: Option<DumpStage>,

    // --- Terminal ---

    /// Color output (always, auto, never; default: auto)
    pub color: ColorChoice,

    // --- Cargo pass-through ---

    /// Extra arguments passed through to cargo check
    pub cargo_args: Vec<String>,
}
```

### Option details

- `--target-os` is required. Clap uses TargetOs::from_str automatically.
- `--platform-file` is required, must be an existing file path.
- `--output-dir` defaults to `./regseq-out/`.
- `--emit <SPEC>` is repeatable. Accepts the format `TYPE[.SUBTYPE][=VALUE]`.
  When not specified, defaults to `[EmitSpec::Driver { match_id: None }]`.
- `--check` is a boolean flag. Overrides `--emit` at runtime (pipeline runs
  but no artifacts are written).
- `--add-analysis <NAME>` / `-a <NAME>` is repeatable.
- `--skip-analysis <NAME>` / `-s <NAME>` is repeatable.
- `--deny <NAME>` is repeatable.
- `--allow <NAME>` is repeatable.
- `--param <KEY=VALUE>` / `-P <KEY=VALUE>` is repeatable.
  Clap uses RegseqParam::from_str.
- `--log-level <LEVEL>` defaults to Warn. Clap uses LogLevel::from_str.
- `--verbose` / `-v` is a boolean flag.
- `--dump-stage <STAGE>` is optional. Clap uses DumpStage::from_str.
- `--color` defaults to Auto. Clap uses ColorChoice::from_str. No short flag
  (cargo itself uses `--color` without a short, and `-c` could be confused
  with `--check`).
- `cargo_args` captures everything after `--`.

### Validation

Implement `Cli::validate(&self) -> Result<(), Error>`. This validates syntax
and structure. It takes `&self` (immutable) and returns errors on invalid
input. Semantic validation (do these analysis names exist? are these param
keys recognized?) is the driver's responsibility.

The principle: cargo-regseq validates structure, the driver validates semantics.

Checks in order:

1. **Platform file exists and is a file.** If the path does not exist or is
   not a regular file, return error with a clear message including the path.
2. **Output directory is not a file.** If `--output-dir` points to an
   existing path that is a regular file (not a directory), return error.
   If it doesn't exist yet, that's fine -- it will be created later.
3. **Add/skip analysis conflict.** If a name appears in both
   `--add-analysis` and `--skip-analysis`, return error listing the
   conflicting names.
4. **Deny/allow conflict.** If a name appears in both `--deny` and
   `--allow`, return error listing the conflicting names.
5. **Name validation.** Validate all `--add-analysis`, `--skip-analysis`,
   `--deny`, `--allow` values match the `id(.id)*` pattern using
   `regseq_config::validate_dotted_ident()`.
6. **EmitSpec analysis name validation.** For each EmitSpec::Analysis that
   has an `analysis_name`, validate it matches `id(.id)*`.

### Log level resolution

Implement `Cli::resolved_log_level() -> LogLevel`:
- If `--verbose` is set and `log_level == LogLevel::Warn` (the default),
  return `LogLevel::Debug`.
- Otherwise return `self.log_level` as-is.

This is a simple value check, not a source-tracking check. The consequence
is that `-v --log-level warn` returns Debug (same as `-v` alone), which is
acceptable since explicitly requesting the default + verbose is an unusual
edge case and Debug is the more useful outcome.

### Conversion to CompilerOptions

Implement `Cli::into_compiler_options(self) -> CompilerOptions`:

```rust
pub fn into_compiler_options(self) -> CompilerOptions {
    let mut emit = if self.emit.is_empty() {
        vec![EmitSpec::Driver { match_id: None }]
    } else {
        self.emit
    };
    emit.dedup(); // remove consecutive duplicates (Vec is small; dedup suffices)

    CompilerOptions {
        target_os: self.target_os,
        platform_file: std::fs::canonicalize(&self.platform_file)
            .unwrap_or(self.platform_file),
        output_dir: std::path::absolute(&self.output_dir)
            .unwrap_or(self.output_dir),
        emit,
        check: self.check,
        add_analysis: self.add_analysis,
        skip_analysis: self.skip_analysis,
        deny: self.deny,
        allow: self.allow,
        params: self.params,
        log_level: self.resolved_log_level(),
        dump_stage: self.dump_stage,
        color: self.color,
    }
}
```

This resolves defaults (emit, log level), deduplicates emit specs, and
absolutifies paths before creating the struct. The `verbose` flag and
`cargo_args` are consumed here and do not appear in CompilerOptions.

### Wiring

Update `run()` in `lib.rs`:
1. Replace the placeholder `Error` unit struct with the new error enum
2. Parse CLI args via `CargoCli::parse()`, destructure to get `Cli`
3. Call `cli.validate()?`
4. Convert to CompilerOptions via `cli.into_compiler_options()`
5. Print a debug message showing the options (temporary)

Delete `tests/smoke.rs` -- it calls `run()` with no args, which will fail
now that `run()` parses CLI arguments. The new `tests/cli.rs` provides
comprehensive coverage.

### Help output

Ensure all struct fields have doc comments that serve as clap help text.
`cargo regseq --help` should display usage information.

## Test Plan

Create:
- crates/cargo-regseq/tests/cli.rs

All tests parse argument vectors via `CargoCli::try_parse_from(...)` and
destructure to get the inner `Cli`. For tests that need a platform file to
exist, create a temporary file using `tempfile`.

The base args helper should produce:
`["cargo-regseq", "regseq", "--target-os", "linux", "--platform-file", "<tmp>"]`

Tests to implement:

Core argument parsing:
1. Parse with base args; verify target_os is Linux, platform_file correct
2. Parse with `--target-os zephyr`; verify Zephyr
3. Parse with `--target-os LINUX`; verify case-insensitive
4. Parse with `--output-dir /tmp/out`; verify override
5. Parse without `--output-dir`; verify default is `./regseq-out/`
6. Parse without `--target-os`; verify error
7. Parse without `--platform-file`; verify error
8. Parse with `-- --release -v`; verify cargo_args

Emit and check:
9. Parse without `--emit`; into_compiler_options().emit is
   [Driver { match_id: None }]
10. Parse with `--emit analysis`; verify Analysis { name: None, path: None }
11. Parse with `--emit driver --emit analysis.resources`; verify both
12. Parse with `--emit analysis.resources=report.json`; verify path
13. Parse with `--check`; verify check is true
14. Parse without `--check`; verify check is false

Analysis control:
15. Parse with `-a foo.bar`; verify add_analysis contains "foo.bar"
16. Parse with `-a foo -a bar.baz`; verify both
17. Parse with `-s staging`; verify skip_analysis contains "staging"
18. Parse with same name in `-a x` and `-s x`; verify validate() returns error

Diagnostic control:
19. Parse with `--deny shadow.infer`; verify vec
20. Parse with `--allow staging.warn`; verify vec
21. Parse with same name in `--deny x` and `--allow x`;
    verify validate() returns error

Param option:
22. Parse `-P analysis.staging.enabled=true`; verify scope/name/value
23. Parse `-P debug=true`; verify General scope
24. Parse `-P bad-name=value`; verify parse error

Log level and verbose:
25. Parse without `--log-level` or `-v`; verify Warn
26. Parse with `--log-level debug`; verify Debug
27. Parse with `--log-level TRACE`; verify case-insensitive
28. Parse with `-v`; verify verbose is true
29. Parse with `-v` only; verify resolved_log_level() returns Debug
30. Parse with `-v --log-level trace`; verify resolved_log_level()
    returns Trace (explicit takes precedence)

Dump stage:
31. Parse without `--dump-stage`; verify None
32. Parse with `--dump-stage hir`; verify Some(Hir)
33. Parse with `--dump-stage post-analysis`; verify PostAnalysis

Color:
34. Parse without `--color`; verify Auto
35. Parse with `--color always`; verify Always
36. Parse with `--color never`; verify Never

Validation of dotted ident values:
37. Parse with `-a 123bad`; verify validate() returns error
38. Parse with `--deny bad-name`; verify validate() returns error

Conversion to CompilerOptions:
39. Parse base args, call into_compiler_options(); verify emit defaults
    to [Driver { match_id: None }] and log_level is Warn
40. Parse with `-v`, call into_compiler_options(); verify log_level is Debug
41. Parse with `--emit driver --emit driver --emit analysis`; call
    into_compiler_options(); verify emit has 2 entries (deduplication)

Test command: `cargo test -p cargo-regseq`

## Acceptance Criteria

- All 41 tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace` clean
