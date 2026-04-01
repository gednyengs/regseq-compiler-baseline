# Task: regseq-model-02 - Layer 2 platform-file schema and parsed artifact

## Description

Implement the rustc-independent schema and parsed-artifact baseline for the
 initial Layer 2 platform file. This task should define the small Phase-B model
 used by the parser and validation path without drifting into the final
 normalized binding model that belongs to Phase C.

This task owns the **shape** of the parsed platform file, not TOML parsing
 itself.

## Input Context

Read before starting:

- crates/regseq-model/src/lib.rs
- docs/plans/asplos-implementation-plan.md
- docs/compiler-collateral/compiler-baseline-overview.md
- docs/compiler-collateral/constructs/firmware-property-access.md
- docs/discussions/layer-2-format/layer-2-format-discussion.md

## Output Specification

Create:

- crates/regseq-model/src/platform.rs
- crates/regseq-model/tests/platform_schema.rs

Modify:

- crates/regseq-model/src/lib.rs
- crates/regseq-model/Cargo.toml only if a small additional dependency is
  strictly required for this schema task

## Implementation Notes

Define the initial parsed Layer 2 model described in the discussion note. The
 leading baseline is:

```rust
pub struct PlatformFile {
    pub version: String,
    pub platform: PlatformHeader,
    pub properties: Vec<PropertyEntry>,
    pub construct_sections: Vec<ConstructSection>,
    pub unknown_sections: Vec<UnknownSection>,
}

pub struct PlatformHeader {
    pub os: String,
    pub board: String,
    pub arch: String,
}

pub struct PropertyEntry {
    pub key: String,
    pub value: PlatformValue,
}

pub struct ConstructSection {
    pub section_kind: String,
    pub section_key: Option<String>,
    pub values: std::collections::BTreeMap<String, PlatformValue>,
}

pub struct UnknownSection {
    pub section_path: Vec<String>,
    pub values: std::collections::BTreeMap<String, PlatformValue>,
}

pub enum PlatformValue {
    Integer(i64),
    Boolean(bool),
    String(String),
    List(Vec<PlatformValue>),
    Table(std::collections::BTreeMap<String, PlatformValue>),
}
```

The exact names may vary, but the task must preserve the same structural
 boundary:

- a single parsed platform-file artifact
- required top-level version/platform header
- first-class property entries
- preservation of future construct-owned sections
- preservation of unknown but syntactically valid sections
- scalar and aggregate value support limited to:
  - integer
  - boolean
  - string
  - list
  - table/map

Important boundaries for this task:

- this is a **Phase-B parsed artifact**, not the final normalized binding model
- do not encode final join semantics
- do not encode backend lowering information
- do not hardcode PL011-only property names into the model types
- do not design a generic “resource bag” model

Preferred implementation qualities:

- rustc-independent
- serializable / debug-friendly
- deterministic value and section ordering where practical
- easy for later parser and validation tasks to consume

If helpful, derive traits such as:

- `Debug`
- `Clone`
- `PartialEq`
- `Eq` where applicable
- `Serialize`
- `Deserialize`

## Test Plan

Create:

- crates/regseq-model/tests/platform_schema.rs

Tests to implement:

1. Verify the core schema types can be constructed and imported.
2. Verify `PlatformValue` supports the accepted scalar and aggregate kinds.
3. Verify a representative `PlatformFile` can preserve:
   - `version`
   - `platform.os`
   - `platform.board`
   - `platform.arch`
   - one property entry
   - one construct-owned future section
   - one unknown section
4. Verify serialization round-trips or debug-friendly inspection work for the
   representative parsed artifact shape.

Test command: `cargo test -p regseq-model --test platform_schema`

## Acceptance Criteria

- the initial Layer 2 parsed-artifact schema exists in `regseq-model`
- the schema matches the current Layer 2 discussion direction
- the task stays clearly short of the Phase C normalized binding model
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
