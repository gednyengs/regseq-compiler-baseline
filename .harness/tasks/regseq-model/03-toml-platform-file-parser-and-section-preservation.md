# Task: regseq-model-03 - TOML platform-file parser and section preservation

## Description

Implement the initial TOML parser for the Layer 2 platform file. This task
 should parse the required top-level contract, preserve future sections, and
 reject schema-level violations without drifting into cross-layer checks or the
 final normalized binding model.

## Input Context

Read before starting:

- crates/regseq-model/src/lib.rs
- crates/regseq-model/src/platform.rs
- docs/plans/asplos-implementation-plan.md
- docs/compiler-collateral/compiler-baseline-overview.md
- docs/discussions/layer-2-format/layer-2-format-discussion.md

## Output Specification

Create:

- crates/regseq-model/src/platform_parser.rs
- crates/regseq-model/tests/platform_parser.rs

Modify:

- crates/regseq-model/src/lib.rs
- crates/regseq-model/Cargo.toml

## Implementation Notes

Add a parser entry point for TOML platform files, for example:

```rust
pub fn parse_platform_file_str(input: &str) -> Result<PlatformFile, PlatformParseError>;
pub fn parse_platform_file_path(path: &std::path::Path) -> Result<PlatformFile, PlatformParseError>;
```

The exact API may vary, but it must support:

- parsing a TOML string
- parsing from a file path
- returning the Phase-B parsed artifact from `regseq-model-02`
- deterministic parser/schema errors

Required schema behavior for this task:

- require top-level `version`
- require `[platform]`
- require:
  - `platform.os`
  - `platform.board`
  - `platform.arch`
- preserve `[properties]` as the first supported section
- preserve future construct-owned sections without semantically validating them
- preserve unknown but syntactically valid sections as unknown sections

Section classification rule for this task:

- dotted table `[kind.key]` where `kind` is not `platform` or `properties`
  should be preserved as a `ConstructSection`
- simple table `[name]` where `name` is not `platform` or `properties`
  should be preserved as an `UnknownSection`

Important parser boundaries:

- accept syntactically valid future sections
- do not semantically validate future construct sections yet
- do not perform Layer 1 visibility or join checks
- do not enforce property/type compatibility against Rust fields
- do not normalize into the Phase C binding model

Use TOML as the concrete syntax for this task. Add the minimal TOML parsing
 dependency needed to support this.

Expected error classes for this task:

- syntactically invalid TOML
- missing required top-level keys
- wrong primitive kind for required keys
- malformed section shape for the supported initial contract

## Test Plan

Create:

- crates/regseq-model/tests/platform_parser.rs

Tests to implement:

1. Verify a minimal valid platform file parses with:
   - `version`
   - `[platform]`
   - `platform.os`
   - `platform.board`
   - `platform.arch`
2. Verify a valid file with `[properties]` parses.
3. Verify a valid file with future construct-owned sections parses and
   preserves those sections.
4. Verify missing `version` is rejected.
5. Verify missing `[platform]` is rejected.
6. Verify missing `platform.os`, `platform.board`, or `platform.arch` is
   rejected.
7. Verify syntactically malformed TOML is rejected deterministically.

Test command: `cargo test -p regseq-model --test platform_parser`

## Acceptance Criteria

- TOML platform files parse into the Phase-B parsed artifact
- the required initial contract is enforced
- future sections are preserved without premature semantic validation
- the parser remains separate from cross-layer and normalized-model concerns
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
