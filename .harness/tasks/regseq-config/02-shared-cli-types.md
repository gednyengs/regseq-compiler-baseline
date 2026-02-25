# Task: regseq-config-02 - Shared compiler option types

## Description

Define the shared types that represent Regseq compiler options. These types
are used by both cargo-regseq (CLI parsing) and regseq-driver (receiving
configuration via the REGSEQ_OPTIONS env var). All types implement `FromStr`
and `Display` for string-based serialization, and derive
`Serialize`/`Deserialize` for structured serialization (JSON).

The central type is `CompilerOptions` -- the serializable struct that crosses
the process boundary from cargo-regseq to regseq-driver via a single JSON
env var.

## Input Context

Read before starting:
- crates/regseq-config/src/lib.rs
- crates/regseq-config/Cargo.toml
- crates/regseq-config/README.md

## Output Specification

Create:
- crates/regseq-config/src/types.rs
- crates/regseq-config/src/emit.rs
- crates/regseq-config/src/param.rs
- crates/regseq-config/src/options.rs

Modify:
- crates/regseq-config/Cargo.toml (add `serde_json` as dev-dependency)
- crates/regseq-config/src/lib.rs (add pub mod declarations for all new
  modules, re-export all public types)

## Implementation Notes

### types.rs -- Simple enum types

Define the following enums, each with `FromStr` and `Display` impls:

```rust
/// Target operating system for code generation.
pub enum TargetOs {
    Linux,
    Zephyr,
}
```
- FromStr: accepts "linux", "zephyr" (case-insensitive)
- Display: produces lowercase "linux", "zephyr"

```rust
/// Log verbosity level.
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}
```
- FromStr: accepts "error", "warn", "info", "debug", "trace" (case-insensitive)
- Display: produces lowercase
- Implement `Default` for LogLevel, returning `Warn`

```rust
/// Pipeline stage for debug dumps.
pub enum DumpStage {
    Hir,
    Mir,
    Model,
    PostAnalysis,
    PreCodegen,
}
```
- FromStr: accepts "hir", "mir", "model", "post-analysis", "pre-codegen"
  (case-insensitive)
- Display: produces "hir", "mir", "model", "post-analysis", "pre-codegen"

```rust
/// Terminal color setting.
pub enum ColorChoice {
    Always,
    Auto,
    Never,
}
```
- FromStr: accepts "always", "auto", "never" (case-insensitive)
- Display: produces lowercase
- Implement `Default` for ColorChoice, returning `Auto`

All enums should derive `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize`.

Use `#[serde(rename_all = "kebab-case")]` on enums so serde serialization
matches the Display output (e.g., `PostAnalysis` serializes as
`"post-analysis"`).

Use a shared error type for parse failures:

```rust
#[derive(Debug, thiserror::Error)]
#[error("invalid value '{value}' for {type_name}; expected one of: {expected}")]
pub struct ParseEnumError {
    pub value: String,
    pub type_name: &'static str,
    pub expected: &'static str,
}
```

### emit.rs -- Emit specification

```rust
/// What the compiler should produce as output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum EmitSpec {
    /// Generate C driver source files.
    Driver {
        /// Optional match ID for multi-driver selection (future).
        match_id: Option<String>,
    },
    /// Emit analysis results.
    Analysis {
        /// Specific analysis name, or None for aggregate report.
        analysis_name: Option<String>,
        /// Output path, or None for default location.
        path: Option<PathBuf>,
    },
}
```

Implement `FromStr` for `EmitSpec`. The CLI format is:
`TYPE[.SUBTYPE][=PATH_OR_VALUE]`

Parsing:
1. Split on first `=` -> left is type spec, right is value (if present).
2. Split type spec on first `.` -> base type and optional subtype.
3. Dispatch on base type:
   - "driver": value (if any) is `match_id`.
   - "analysis": subtype (if any) is `analysis_name`, value (if any) is `path`.
4. Unknown base type -> error.

Implement `Display` for `EmitSpec` that produces the canonical CLI form:
- `Driver { match_id: None }` -> "driver"
- `Driver { match_id: Some("pl011") }` -> "driver=pl011"
- `Analysis { analysis_name: None, path: None }` -> "analysis"
- `Analysis { analysis_name: None, path: Some("report.json") }` -> "analysis=report.json"
- `Analysis { analysis_name: Some("resources"), path: None }` -> "analysis.resources"
- `Analysis { analysis_name: Some("resources"), path: Some("out.json") }` ->
  "analysis.resources=out.json"

Define an error type:

```rust
#[derive(Debug, thiserror::Error)]
pub enum EmitParseError {
    #[error("unknown emit type '{0}'; expected 'driver' or 'analysis'")]
    UnknownType(String),
}
```

### param.rs -- Parameter parsing

```rust
/// Scope prefix for --param values.
pub enum ParamScope {
    Analysis,
    Backend,
    General,
}
```
- Derive `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize`
- Display: produces "analysis", "backend", "general"
- Use `#[serde(rename_all = "lowercase")]`

```rust
/// A parsed --param KEY=VALUE argument.
pub struct RegseqParam {
    pub scope: ParamScope,
    pub name: String,
    pub value: String,
}
```
- Derive `Debug, Clone, PartialEq, Eq, Serialize, Deserialize`

Implement `FromStr` for `RegseqParam`:

1. Split on the first `=`. If no `=`, return error.
2. The left side is the key, the right side is the value.
3. If the key starts with `analysis.`, strip prefix, set scope=Analysis.
4. If the key starts with `backend.`, strip prefix, set scope=Backend.
5. If the key starts with `regseq.`, strip prefix, set scope=General.
6. Otherwise, scope=General, name is the full key.
7. Validate that the resulting name matches `id(.id)*` where each `id` is
   `[a-zA-Z_][a-zA-Z0-9_]*`. Return error if not.
8. Return error if name is empty.

Define a dedicated error type:

```rust
#[derive(Debug, thiserror::Error)]
pub enum ParamParseError {
    #[error("missing '=' in parameter: '{0}'")]
    MissingEquals(String),

    #[error("empty parameter name")]
    EmptyName,

    #[error("invalid parameter name '{name}': {reason}")]
    InvalidName { name: String, reason: String },

    #[error("unknown scope prefix '{0}'; expected 'analysis', 'backend', or 'regseq'")]
    UnknownScope(String),
}
```

Also provide a standalone validation function for identifiers used in
diagnostic names (--deny/--allow use the same `id(.id)*` pattern):

```rust
/// Validate that `name` matches the `id(.id)*` pattern.
pub fn validate_dotted_ident(name: &str) -> Result<(), String>
```

This function is used by both RegseqParam::from_str and by cargo-regseq's
CLI validation for --deny/--allow/--add-analysis/--skip-analysis values.

### options.rs -- CompilerOptions

The serializable struct that crosses the process boundary:

```rust
/// Complete compiler configuration, serialized to JSON and passed
/// from cargo-regseq to regseq-driver via the REGSEQ_OPTIONS env var.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompilerOptions {
    pub target_os: TargetOs,
    pub platform_file: PathBuf,
    pub output_dir: PathBuf,
    pub emit: Vec<EmitSpec>,
    pub check: bool,
    pub add_analysis: Vec<String>,
    pub skip_analysis: Vec<String>,
    pub deny: Vec<String>,
    pub allow: Vec<String>,
    pub params: Vec<RegseqParam>,
    pub log_level: LogLevel,
    pub dump_stage: Option<DumpStage>,
    pub color: ColorChoice,
}
```

This struct is constructed by cargo-regseq from parsed CLI args and
deserialized by regseq-driver from the `REGSEQ_OPTIONS` env var.

No validation methods on CompilerOptions itself -- validation happens in
cargo-regseq before construction. CompilerOptions is a data-only struct.

## Test Plan

Create:
- crates/regseq-config/tests/types.rs
- crates/regseq-config/tests/emit.rs
- crates/regseq-config/tests/param.rs
- crates/regseq-config/tests/options.rs

### types.rs tests

TargetOs:
1. Parse "linux" -> Linux
2. Parse "ZEPHYR" -> Zephyr (case-insensitive)
3. Parse "invalid" -> error
4. Display Linux -> "linux"

LogLevel:
5. Parse "debug" -> Debug
6. Parse "TRACE" -> Trace (case-insensitive)
7. Parse "invalid" -> error
8. Default is Warn

DumpStage:
9. Parse "hir" -> Hir
10. Parse "post-analysis" -> PostAnalysis
11. Parse "pre-codegen" -> PreCodegen
12. Parse "invalid" -> error

ColorChoice:
13. Parse "always" -> Always
14. Parse "AUTO" -> Auto (case-insensitive)
15. Parse "invalid" -> error
16. Default is Auto

### emit.rs tests

EmitSpec parsing:
17. Parse "driver" -> Driver { match_id: None }
18. Parse "driver=pl011" -> Driver { match_id: Some("pl011") }
19. Parse "analysis" -> Analysis { analysis_name: None, path: None }
20. Parse "analysis.resources" -> Analysis { analysis_name: Some("resources"),
    path: None }
21. Parse "analysis.resources=report.json" -> Analysis { analysis_name:
    Some("resources"), path: Some("report.json") }
22. Parse "analysis=report.json" -> Analysis { analysis_name: None,
    path: Some("report.json") }
23. Parse "unknown" -> error
24. Parse "" -> error
25. Display round-trip: parse then display for each variant above, verify
    string matches original input

### param.rs tests

RegseqParam parsing:
26. Parse "analysis.staging.enabled=true" -> scope=Analysis,
    name="staging.enabled", value="true"
27. Parse "backend.linux.header_path=/usr/include" -> scope=Backend,
    name="linux.header_path", value="/usr/include"
28. Parse "regseq.debug=true" -> scope=General, name="debug", value="true"
29. Parse "debug=true" (no prefix) -> scope=General, name="debug", value="true"
30. Parse "key=val=ue" -> value is "val=ue" (split on first = only)
31. Parse "noequals" -> MissingEquals error
32. Parse "=value" -> EmptyName error
33. Parse "bad-name=value" -> InvalidName error (contains -)
34. Parse "123start=value" -> InvalidName error (starts with digit)
35. Parse "a.b.c=value" (no scope prefix) -> scope=General, name="a.b.c"

validate_dotted_ident:
36. "foo" -> Ok
37. "foo.bar.baz" -> Ok
38. "_private.field" -> Ok
39. "" -> Err
40. "bad-name" -> Err
41. ".leading_dot" -> Err
42. "trailing_dot." -> Err
43. "double..dot" -> Err

### options.rs tests

Serde round-trip (use `serde_json`):
44. Serialize TargetOs::Linux to JSON -> "linux"; deserialize back -> Linux
45. Serialize DumpStage::PostAnalysis to JSON -> "post-analysis"; deserialize back
46. Serialize ColorChoice::Auto to JSON -> "auto"; deserialize back
47. Serialize EmitSpec::Driver { match_id: None } to JSON; deserialize back
48. Serialize EmitSpec::Analysis { analysis_name: Some("resources"),
    path: Some("out.json") } to JSON; deserialize back
49. Serialize a RegseqParam to JSON; deserialize back, verify all fields
50. Serialize a full CompilerOptions struct to JSON; deserialize back,
    verify all fields match. This is the critical test -- it proves the
    REGSEQ_OPTIONS env var round-trip works.

Test command: `cargo test -p regseq-config`

## Acceptance Criteria

- All 50 tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace` clean
