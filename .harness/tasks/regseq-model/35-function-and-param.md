# regseq-model-35: Function, Param, ParamKind

## Description

Implement the function model types. All callable entities -- free
functions, trait methods, helper methods, ISR handlers, abstraction
wrapper methods -- share the same `Function` representation.

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 2, step 2.5)
- `docs/discussions/models/regseq-model/spec.md` (Section 8: Function parameters, Attrs)
- `crates/regseq-model/src/identity/` (FunctionId, Symbol)
- `crates/regseq-model/src/ir/types.rs` (RegseqType)
- `crates/regseq-model/src/ir/body.rs` (Body)
- `crates/regseq-model/src/model/type_def.rs` (Attrs)

## Implementation Notes

Create `crates/regseq-model/src/model/function.rs`. Update `model/mod.rs`.

### Public API

```rust
// model/function.rs

/// A callable entity: free function, trait method, helper method,
/// ISR handler, or abstraction wrapper method.
///
/// Domain-specific classification comes from structural position in
/// the model (which section owns the function) and from the `attrs`
/// field.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Function {
    pub id: FunctionId,
    pub name: Symbol,
    pub params: Vec<Param>,
    pub return_type: RegseqType,
    pub body: Body,
    pub attrs: Attrs,
}

/// A function parameter.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Param {
    pub kind: ParamKind,
    pub name: Symbol,
    pub ty: RegseqType,
}

/// The semantic role of a parameter.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum ParamKind {
    /// self -- this parameter carries method semantics.
    Receiver,
    /// Normal parameter.
    Regular,
}
```

### Conventions

- `Function` contains Symbol and Body fields, so no standalone serde
  derives. `ParamKind` can derive serde (no Symbol fields).
- A function with `params[0].kind == Receiver` is a method.
- The receiver's type determines the target type (e.g.,
  `Ref(Defined(driver_type_id), Mutable)` for `&mut self`).
- `body` is always present -- the spec does not have bodyless function
  declarations. Functions are only extracted if they have MIR bodies.
- During Phase 5 Pass 1 (declaration skeleton), `body` is initialized
  as an empty `Body` (no locals, single entry block with `Return`
  terminator, empty source map). Pass 2 replaces it with the
  constructed body. This avoids making `body` an `Option`.

## Non-Goals

- No impl block or module ownership (regseq-model-38, 39).
- No body construction logic (Phase 5).

## Test Plan

- Construct a `Function` representing a free function (no Receiver param).
- Construct a `Function` representing a method (params[0].kind == Receiver).
- Construct a `Function` with multiple Regular params.
- Serde round-trip for `ParamKind` both variants.
- Verify a method function's first param kind is Receiver.
- `cargo test -p regseq-model` passes.

## Review

After tests pass, use `codex:rescue` to review against spec.md
Section 8 (Function parameters). Verify the shared Function
representation covers all callable entities. Iterate until approved.

## Quality Gate

```
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

## Acceptance Criteria

- `model/function.rs` created with all types.
- Re-exported from `model/mod.rs`.
- All tests pass. Codex review approved.
