# Task: mir-schema-refactor-01 - MIR type and constant schema refactor

## Description

Perform a full, non-incremental refactor of the MIR schema used across the
compiler pipeline so that:

- `MirTy` becomes a resolved, backend-friendly type/schema model
- `MirConstValue` becomes a structured, backend-friendly constant model
- `MirConstant` remains typed but stops relying on string-encoded constant data
- generic instantiation identity is tracked explicitly via
  `MonomorphizationKey`

This is a repo-wide coordinated refactor. It is not an incremental migration
task. Do not preserve the old schema alongside the new one.

This task exists because the current MIR model is asymmetrical:

- `MirTy` is moderately structured but still rustc-shaped (`Adt`, `FnDef`)
- `MirConstValue` is too coarse (`Scalar`, `ZeroSized`, `Unsupported`)
- constant structure is currently recovered through ad hoc string encoding in
  `Unsupported { description }`
- `regseq-driver` and `regseq-model` currently carry mirrored MIR schema
  definitions that must be kept in sync manually

The goal is to replace that with one coherent model definition and refactor all
extractors, consumers, tests, and reports to use it directly.

## Branch Workflow

This task must be implemented on a dedicated development branch before any
merge back to `main`.

Required workflow:

1. create a dedicated branch from current `main`
2. perform the full refactor on that branch
3. keep iterating there until the full workspace is green
4. only then merge the finished branch back into `main`

Recommended branch name:

- `refactor/mir-schema-and-monomorphization`

Do not leave `main` in a partially migrated state.

## Input Context

Read before starting:

- `docs/validation/01-pl011/README.md`
- `docs/validation/01-pl011/01-authored-source.md`
- `docs/validation/01-pl011/checks/results/01-register-block-extraction.md`
- `docs/validation/01-pl011/checks/results/03-driver-struct-extraction.md`
- `docs/validation/01-pl011/checks/results/04-utility-functions-extraction.md`
- `docs/validation/01-pl011/checks/results/06-driver-constants-extraction.md`
- `docs/validation/01-pl011/checks/results/07-helper-methods-extraction-a.md`

Primary MIR schema definitions and extractors:

- `crates/regseq-model/src/declaration/mir_body.rs`
- `crates/regseq-model/src/declaration/raw_input.rs`
- `crates/regseq-model/src/declaration/normalize.rs`
- `crates/regseq-model/src/declaration/bodies.rs`
- `crates/regseq-driver/src/mir_repr.rs`
- `crates/regseq-driver/src/extract/mir_extract.rs`
- `crates/regseq-driver/src/extract/body.rs`
- `crates/regseq-driver/src/raw_artifact.rs`

High-churn backend consumers:

- `crates/regseq-backends/src/lowering/mir_interpreter/value_lowering.rs`
- `crates/regseq-backends/src/lowering/mir_interpreter/place_resolver.rs`
- `crates/regseq-backends/src/lowering/mir_interpreter/cfg_helpers.rs`
- `crates/regseq-backends/src/lowering/semantic_lowering/expr_lowering.rs`
- `crates/regseq-backends/src/lowering/semantic_lowering/external_items.rs`
- `crates/regseq-backends/src/lowering/semantic_lowering/option_lowering.rs`
- `crates/regseq-backends/src/lowering/semantic_lowering/semantic_walker.rs`

High-signal tests that exercise the current schema heavily:

- `crates/regseq-driver/tests/raw_artifact.rs`
- `crates/regseq-driver/tests/source_to_extraction_04_utility_functions.rs`
- `crates/regseq-driver/tests/source_to_extraction_06_driver_constants.rs`
- `crates/regseq-driver/tests/source_to_extraction_07_helper_methods_a.rs`
- `crates/regseq-driver/tests/source_to_extraction_08_helper_methods_b.rs`
- `crates/regseq-driver/tests/source_to_extraction_09_lifecycle_callbacks.rs`
- `crates/regseq-driver/tests/source_to_extraction_10_serial_callbacks_a.rs`
- `crates/regseq-driver/tests/source_to_extraction_11_serial_callbacks_b.rs`
- `crates/regseq-driver/tests/source_to_extraction_12_isr.rs`
- `crates/regseq-model/tests/body_normalization.rs`
- `crates/regseq-analysis/tests/body_readiness.rs`

## Target Schema

Implement the MIR schema around the following design.

`MirConstant` stays typed:

```rust
pub struct MirConstant {
    pub ty: MirTy,
    pub value: MirConstValue,
}
```

`MirTy` becomes a resolved schema model:

```rust
pub enum MirTy {
    Bool,
    Char,

    Int {
        signed: bool,
        width: IntWidth,
    },
    Float {
        width: FloatWidth,
    },

    Ref {
        mutbl: bool,
        ty: Box<MirTy>,
    },
    RawPtr {
        mutbl: bool,
        ty: Box<MirTy>,
    },

    Struct {
        path: String,
        fields: Vec<MirFieldTy>,
    },
    Enum {
        path: String,
        variants: Vec<MirVariantTy>,
    },
    Tuple {
        elements: Vec<MirTy>,
    },
    Array {
        element: Box<MirTy>,
        len: u64,
    },
    Slice {
        element: Box<MirTy>,
    },

    Fn {
        path: String,
        params: Vec<MirTy>,
        ret: Box<MirTy>,
    },

    Str,
    Bytes,
    Never,
    ZeroSizedLike {
        description: String,
    },

    Unsupported {
        description: String,
    },
}
```

```rust
pub enum IntWidth {
    I8,
    I16,
    I32,
    I64,
    I128,
    Isize,
    U8,
    U16,
    U32,
    U64,
    U128,
    Usize,
}
```

```rust
pub enum FloatWidth {
    F32,
    F64,
}
```

```rust
pub struct MirFieldTy {
    pub index: u32,
    pub name: String,
    pub ty: MirTy,
}

pub struct MirVariantTy {
    pub name: String,
    pub fields: Vec<MirFieldTy>,
}
```

`MirConstValue` becomes a structured constant model:

```rust
pub enum MirConstValue {
    Bits(u128),
    Bool(bool),
    Char(u32),
    Str(String),
    Bytes(Vec<u8>),

    Struct {
        fields: Vec<ConstField>,
    },
    Enum {
        variant: String,
        fields: Vec<ConstField>,
    },
    Tuple(Vec<MirConstValue>),
    Array(Vec<MirConstValue>),

    Unevaluated {
        path: String,
    },
    ZeroSized,
    Unsupported {
        path: Option<String>,
        description: String,
    },
}
```

```rust
pub struct ConstField {
    pub index: u32,
    pub name: Option<String>,
    pub value: Box<MirConstValue>,
}
```

Generic-origin identity is tracked separately:

```rust
pub struct MonomorphizationKey {
    pub origin_path: String,
    pub args: Vec<MirTy>,
}
```

At minimum, add `monomorphization: Option<MonomorphizationKey>` to extracted
function bodies.

Also refactor function metadata so interface types are structured rather than
stringly typed:

- `FnParamDecl.ty` should become `MirTy`
- `MirFnBodyDecl.return_type` should become `Option<MirTy>`

### Recursion / Indirection Rule

The target schema above describes the semantic shape the refactor must expose,
but the implementation must also handle recursive or mutually recursive nominal
types without infinitely inlining schemas.

Therefore, this task must introduce a deterministic nominal-indirection
mechanism for recursive or reused type schemas. Acceptable approaches include:

- a dedicated type-definition table plus nominal refs, or
- an explicit nominal-reference `MirTy` variant used when full inlining would
  recurse or duplicate unboundedly

Non-negotiable requirements:

- recursive types must be representable without infinite expansion
- the representation must stay deterministic across runs
- downstream consumers must not have to reverse-engineer recursion from ad hoc
  strings
- the implementer must document the chosen indirection mechanism in code and in
  any touched validation report/test comments where needed

## Output Specification

Primary writable surfaces:

- `crates/regseq-model/src/declaration/mir_body.rs`
- `crates/regseq-model/src/declaration/raw_input.rs`
- `crates/regseq-model/src/declaration/normalize.rs`
- `crates/regseq-model/src/declaration/bodies.rs`
- `crates/regseq-driver/src/mir_repr.rs`
- `crates/regseq-driver/src/extract/mir_extract.rs`
- `crates/regseq-driver/src/extract/body.rs`
- `crates/regseq-driver/src/raw_artifact.rs`
- `crates/regseq-driver/src/lib.rs`
- `crates/regseq-backends/src/lowering/mir_interpreter/`
- `crates/regseq-backends/src/lowering/semantic_lowering/`
- `crates/regseq-analysis/`
- MIR-related tests in `crates/regseq-driver/tests/`
- MIR-related tests in `crates/regseq-model/tests/`
- MIR-related tests in `crates/regseq-backends/tests/`
- MIR-related tests in `crates/regseq-analysis/tests/`
- golden MIR/raw-artifact fixtures and snapshot-style expectations under:
  - `tests/fixtures/**/expected/`
  - any checked-in JSON/raw frontend artifact expectations affected by the new
    MIR schema

Mandatory new or updated proof artifacts:

- at least one targeted extraction test proving same-path/different-args
  monomorphization identity, for example a generic function like
  `type_size::<u8>()` and `type_size::<u64>()` or an equivalent same-path,
  same-interface, different-instantiation case
- at least one downstream consumer test proving the new
  `MonomorphizationKey` is observable and usable outside raw extraction

The task may update any MIR-related validation report that currently relies on:

- `MirConstValue::Unsupported { description }` string parsing
- `MirConstValue::Scalar { bits, size }`
- `MirTy::Adt { path, args }`
- `MirTy::FnDef { path, args }`
- stringly typed function parameter / return types

If validation reports are updated:

- keep them honest
- update evidence and reasoning to match the new structured model
- do not weaken any existing check contract

## Constraints

- Do not implement this directly on `main`; use a dedicated branch first.
- Do not do an incremental compatibility rollout.
- Do not keep old and new MIR schemas side by side.
- Do not keep string-encoded structured constant payloads in
  `Unsupported.description`.
- Do not preserve duplicated authoritative MIR schema definitions across
  `regseq-model` and `regseq-driver`.
- Do not weaken any PL011 extraction check contract to make the refactor pass.
- Do not silently erase monomorphization identity.
- Do not replace structured type information with new string fields.
- Do not leave recursive type handling implicit or unspecified.
- Do not leave backend consumers depending on legacy `Adt`, `FnDef`, `Scalar`,
  or description-string parsing after the refactor is complete.
- Do not delete `crates/regseq-driver/src/mir_repr.rs` unless every
  path-inlined test harness that currently imports it is updated in the same
  refactor. The safer default for this task is to keep `mir_repr.rs` as a
  thin re-export shim over the canonical model definitions.

## Implementation Notes

This refactor should be executed in one coordinated pass.

### 1. Make one crate the schema source of truth

`crates/regseq-model/src/declaration/mir_body.rs` should become the canonical
MIR schema definition site.

`crates/regseq-driver/src/mir_repr.rs` must stop being an independently
maintained duplicate. Either:

- replace it with re-exports of the model definitions, or
- remove it and update imports accordingly

The end state must not require humans to manually synchronize two MIR schema
definitions.

Because current `source_to_extraction_*` tests inline `../src/mir_repr.rs`
directly, the default expected outcome for this task is:

- keep `crates/regseq-driver/src/mir_repr.rs` as a thin compatibility
  re-export shim over `regseq-model`'s canonical definitions

Only remove it if the refactor also updates every such test harness
consistently.

### 2. Refactor extraction to emit the new schema directly

Update MIR extraction so it:

- emits resolved `Struct` / `Enum` / `Tuple` / `Array` / `Fn` type shapes
- emits structured constant values instead of string hacks
- emits explicit `Unevaluated { path }` where evaluation is not possible
- emits `MonomorphizationKey` on functions when rustc provides an instantiated
  body

For constants:

- stop encoding resolved constant field data into
  `Unsupported { description }`
- represent resolved `FieldValue` constants as actual `Struct` payloads
- preserve enough information to keep the current remediation wins for:
  - register-block extraction
  - utility free functions
  - associated constants
  - helper-methods A

### 3. Refactor function interface metadata

Current function metadata is too stringly typed.

Update function-body declarations so that:

- parameters use `MirTy`
- return types use `MirTy`
- callable types use `MirTy::Fn { path, params, ret }`

This should eliminate string-only parameter and return type handling in MIR
body consumers.

This necessarily includes updating:

- raw extracted MIR input structs in
  `crates/regseq-model/src/declaration/raw_input.rs`
- MIR normalization in `crates/regseq-model/src/declaration/normalize.rs`
- any legacy declaration mirrors in `crates/regseq-model/src/declaration/bodies.rs`
  that must remain internally coherent

### 4. Preserve generic-instantiation identity without backend substitution

Backends should not be forced to perform generic substitution.

Instead:

- keep `MirTy` resolved and codegen-friendly
- add `MonomorphizationKey` where function bodies are instantiated
- use `origin_path + args` to distinguish monomorphizations

This is required because identical function interfaces can still produce
different instantiated bodies.

This task must prove that behavior with an exercised test case where:

- two extracted functions share the same authored path
- their callable interface types are identical
- their monomorphization args differ
- their extracted bodies or observed constant effects differ accordingly

### 5. Refactor backend and analysis consumers completely

Follow compile failures through the consumers and update them to the new
schema. Key patterns that must disappear:

- matches on `MirTy::Adt`
- matches on `MirTy::FnDef`
- matches on `MirConstValue::Scalar`
- tests or lowerers that parse semantic facts out of
  `Unsupported.description`

Update place/value lowering, external item discovery, option/result reasoning,
and body-walking utilities to use the new schema directly.

### 6. Refactor tests and reports to structured matching

All MIR-heavy tests should be updated to:

- match structured `Struct` / `Enum` / `Tuple` / `Array` constants
- match structured `Fn` / `Struct` / `Enum` types
- stop relying on the old `Unsupported` string conventions

In particular, the PL011 proof-oriented tests under
`crates/regseq-driver/tests/source_to_extraction_*` should use structured
matching instead of string parsing wherever the new schema makes that possible.

### 7. No partial completion

This task is not complete when just the schema compiles.

It is complete only when:

- extraction
- serialization
- backends
- analysis
- validation tests
- workspace

all agree on the new model and the old model is gone.

## Test Plan

Work iteratively on the dedicated branch, but before merge run at minimum:

- `cargo test -p regseq-driver --test source_to_extraction_01_register_block -- --nocapture`
- `cargo test -p regseq-driver --test source_to_extraction_03_driver_struct -- --nocapture`
- `cargo test -p regseq-driver --test source_to_extraction_04_utility_functions -- --nocapture`
- `cargo test -p regseq-driver --test source_to_extraction_06_driver_constants -- --nocapture`
- `cargo test -p regseq-driver --test source_to_extraction_07_helper_methods_a -- --nocapture`
- `cargo test -p regseq-driver --test source_to_extraction_08_helper_methods_b -- --nocapture`
- `cargo test -p regseq-driver --test source_to_extraction_09_lifecycle_callbacks -- --nocapture`
- `cargo test -p regseq-driver --test source_to_extraction_10_serial_callbacks_a -- --nocapture`
- `cargo test -p regseq-driver --test source_to_extraction_11_serial_callbacks_b -- --nocapture`
- `cargo test -p regseq-driver --test source_to_extraction_12_isr -- --nocapture`
- `cargo test -p regseq-driver --test raw_artifact -- --nocapture`
- `cargo test -p regseq-model body_normalization -- --nocapture`
- `cargo test -p regseq-analysis body_readiness -- --nocapture`
- the new monomorphization proof test(s) added by this task
- `cargo test --workspace`

If any PL011 report must be updated because the new schema changes how evidence
is expressed, update it before merge.

## Acceptance Criteria

- the repo contains one authoritative MIR schema definition, not two divergent
  mirrored copies
- `MirTy` uses the new resolved-schema representation
- `MirConstValue` uses the new structured constant representation
- function interface types are structured (`MirTy`), not string-only
- `MonomorphizationKey` is present on extracted function bodies
- recursive or mutually recursive nominal types are represented deterministically
  through an explicit indirection mechanism rather than infinite inlining
- backend and analysis consumers no longer depend on legacy `Adt`, `FnDef`,
  `Scalar`, or `Unsupported.description` parsing for structured facts
- same-path/different-args monomorphization is proven by an exercised test and
  at least one downstream consumer test
- all high-signal MIR validation tests listed above pass
- `cargo test --workspace` passes
- the refactor branch is merged back into `main` only after the full workspace
  is green
