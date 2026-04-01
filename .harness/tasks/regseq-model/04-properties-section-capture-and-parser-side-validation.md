# Task: regseq-model-04 - `[properties]` section capture and parser-side validation

## Description

Implement first-class capture of the Layer 2 `[properties]` section and the
 parser-side validation rules that belong purely to the platform file itself.
 This task should make the property frontier real without drifting into
 cross-layer property checks that need Layer 1 declarations or final join work.

## Input Context

Read before starting:

- crates/regseq-model/src/platform.rs
- crates/regseq-model/src/platform_parser.rs
- docs/plans/asplos-implementation-plan.md
- docs/compiler-collateral/constructs/firmware-property-access.md
- docs/discussions/layer-2-format/layer-2-format-discussion.md

## Output Specification

Create:

- crates/regseq-model/src/properties.rs
- crates/regseq-model/tests/platform_properties.rs

Modify:

- crates/regseq-model/src/platform.rs
- crates/regseq-model/src/platform_parser.rs
- crates/regseq-model/src/lib.rs

## Implementation Notes

Make `[properties]` a first-class supported section in the parsed platform-file
 model.

Required behavior for this task:

- capture `[properties]` entries as `PropertyEntry` values
- preserve property keys exactly as written
- preserve supported property values using the accepted `PlatformValue` subset
- reject unsupported TOML value kinds for property values

The supported property value kinds for the initial parser are:

- integer
- boolean
- string
- list
- table/map

The parser should reject unsupported TOML value kinds for `[properties]`, such
 as:

- float
- datetime
- any TOML value kind outside the accepted initial subset

Important boundary for this task:

- this is **parser-side** and **section-local** validation only
- do not validate property keys against Layer 1 declarations
- do not infer required vs optional semantics from Rust fields
- do not perform type compatibility checks against Layer 1 field types
- do not implement `PROP-001` through `PROP-005` here except where the rule is
  purely a parser-side shape rule

Concretely:

- supported value-kind enforcement belongs here
- Layer 1 declaration matching does not
- property-id unknown/duplicate relative to Layer 1 does not
- integer-range validation relative to a declared Rust type does not
- invalid enum variants relative to a declared Layer 1 enum do not

## Test Plan

Create:

- crates/regseq-model/tests/platform_properties.rs

Tests to implement:

1. Verify scalar property values parse:
   - integer
   - boolean
   - string
2. Verify aggregate property values parse:
   - list
   - table/map
3. Verify unsupported TOML value kinds in `[properties]` are rejected
   deterministically.
4. Verify property keys are preserved exactly as written.
5. Verify the parsed `[properties]` representation remains rustc-independent
   and serializable/debug-friendly.

Test command: `cargo test -p regseq-model --test platform_properties`

## Acceptance Criteria

- `[properties]` is a first-class supported section in the Layer 2 parser path
- supported property value kinds are captured correctly
- unsupported property value kinds are rejected at parser-validation time
- the task stays strictly parser-local and does not drift into cross-layer
  property checks
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
