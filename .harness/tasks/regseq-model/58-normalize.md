# regseq-model-58: `normalize()` Function + NormalizeError + Provenance Contract

## Description

Implement `normalize(input: &RawFrontendArtifact) -> Result<RegseqModel,
NormalizeError>`: the Phase-6 entry point that turns the Phase-5
serialized output (`RawFrontendArtifact` JSON) into a Phase-1
`RegseqModel` with all declaration-side fields populated and every
binding slot `None`. Calls `verify_phase_1` internally on success
before returning `Ok`.

**Input-type resolution (Codex review fix).** The Phase-6 brief names
the abstract type `RawArtifactInput`; the concrete Rust type exposed
by `regseq-driver` is `RawFrontendArtifact` in
`crates/regseq-driver/src/raw_artifact.rs`. This task uses the
concrete type `&RawFrontendArtifact`. If a future phase introduces a
trait-level abstraction, it can be added additively; Phase 6 does not
speculate.

This task owns the **provenance contract** (spec.md Section 12 line
1356-1357): every normalized entity and every `NormalizeError` must
cite its originating `RawFrontendArtifact` surface + containing item.

## Input Context

- `docs/discussions/models/regseq-model/spec.md` Section 12
  (lines 1337-1365: Phase-1 extraction normalization properties
  and error conditions; locally-scoped normalization; determinism
  contract).
- `.harness/spec-batches/regseqmodel-phase-6.md` Implementation
  Notes: "Determinism contract" (lines 71-74),
  "Locally-scoped normalization" (75-78), "Provenance" (79-81),
  "normalize() error conditions" (82-87).
- `crates/regseq-driver/src/raw_artifact.rs` -- the full 13
  Extraction Surfaces schema, starting at line 46 with
  `pub struct RawFrontendArtifact`.
- `crates/regseq-model/src/model/regseq_model.rs` -- output
  struct.
- `crates/regseq-model/src/verify/` -- verifier from tasks 55.
- `crates/regseq-model/src/declaration/normalize.rs` -- the
  **legacy** normalization (reference only; being replaced).
  Study the shape of its error handling but do not re-export its
  types; this task authors a clean replacement, not a port.
- `.harness/tasks/regseq-model/55-verifier-framework-and-phase1.md`
  (`verify_phase_1` called internally after construction).
- `tests/fixtures/pl011-full-spec/expected/raw-frontend.json` --
  the PL011 fixture input used by task 60's end-to-end test;
  task 58 uses it in unit tests too.

## Implementation Notes

### Module layout

Create `crates/regseq-model/src/normalize/` with:
- `mod.rs` (public surface: `normalize`, `NormalizeError`,
  `NormalizeErrorKind`, `NormalizeProvenance`).
- `surface.rs` (per-surface normalization helpers -- one per
  `RawFrontendArtifact` top-level surface, e.g.,
  `normalize_driver_identity`, `normalize_register_block_decl`,
  `normalize_clock_surface`, etc.).
- `provenance.rs` (the `NormalizeProvenance` type + helpers for
  citing raw-input surface paths).

### Error model

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct NormalizeError {
    pub kind: NormalizeErrorKind,
    pub provenance: NormalizeProvenance,
}

pub enum NormalizeErrorKind {
    // ----- From spec.md line 1361-1364 -----
    UnrecognizedEnumTag { surface: &'static str, field: &'static str, tag: String },
    MissingRequiredAttribute { surface: &'static str, attribute: &'static str, item: String },
    StructuralViolation { description: String },
    TypeConstraintViolation { description: String },
    // ----- Leaking verification errors from verify_phase_1 -----
    Phase1VerificationFailed { inner: VerifierError },
}

pub struct NormalizeProvenance {
    /// Dotted path into the RawFrontendArtifact, e.g.,
    /// "clock_surfaces[3].fields[1].attributes.id".
    pub surface_path: String,
    /// The logical containing item, e.g., "DriverStruct(PL011Driver)"
    /// or "Module(crate::registers)".
    pub containing_item: String,
}
```

Every error MUST populate both `surface_path` and `containing_item`
non-empty. Normalizers pass a `NormalizeContext` down the call stack
holding the currently-open item name and the current surface path
(built via push/pop as fields are visited).

### Locally-scoped normalization

Per spec.md line 1353: each module and item normalizes
independently. Implementation follows this strictly:
- `normalize` is the outer orchestrator.
- It delegates to per-surface functions that take a
  `NormalizeContext` + the surface slice.
- Cross-references resolve via typed `DeclId`s assigned during
  Phase 5 Pass 1 and carried through
  `RawFrontendArtifact`. This task does NOT reassign IDs or
  walk the raw JSON by string path; all cross-references are
  already pre-resolved upstream.

### Determinism contract

Per spec.md line 1352 and brief lines 71-74: same
`RawFrontendArtifact` → byte-identical `RegseqModel`
serialization, always. Achieve this by:
- Sorting every `Vec` before insertion into `BTreeMap` /
  `BTreeSet` containers in the output model (keys are
  `DeclId`s, already deterministic).
- Sorting any intermediate working `Vec` that ends up in the
  output in a canonical order (by `DeclId`, or by name if the
  collection is keyed by string).
- Asserting determinism in unit tests (two runs on the same
  input produce byte-identical serialized `RegseqModel`).

### `normalize()` entry

```rust
pub fn normalize(
    input: &RawFrontendArtifact,
) -> Result<RegseqModel, NormalizeError> {
    let mut ctx = NormalizeContext::root();
    let mut model = RegseqModel::from_verified_components_unchecked(/* empty */);
    // Per-surface normalization, one call per top-level surface.
    surface::normalize_driver_identity(&mut ctx, &input.driver_identity, &mut model)?;
    surface::normalize_register_blocks(&mut ctx, &input.register_blocks, &mut model)?;
    // ... (one per surface)

    // Phase-1 post-condition: verify before returning.
    verify_phase_1(&model)
        .map_err(|inner| NormalizeError {
            kind: NormalizeErrorKind::Phase1VerificationFailed { inner },
            provenance: NormalizeProvenance {
                surface_path: String::from("<post-normalize verify>"),
                containing_item: String::from("<RegseqModel root>"),
            },
        })?;
    Ok(model)
}
```

`from_verified_components_unchecked` (introduced in task 57) is the
only way to construct a `RegseqModel` without going through
`begin_mutation`. Normalization doesn't mutate a pre-existing model;
it builds from scratch, so the mutation API does not apply here --
this is why `normalize()` depends on task 55 (verifier) but NOT on
task 57 (mutation API).

## Non-Goals

- **No modification of `crates/regseq-driver/src/construction.rs`
  or the Phase-5 Pass-1 / Pass-2 logic.** This task is a pure
  wrapper over `RawFrontendArtifact`.
- No platform-file binding (task 59).
- No Phase-2 verification (task 56).
- No consumer migration (Phase 7).
- No backend changes (Phase 8).
- No trait-level abstraction over the input type; commit to
  `&RawFrontendArtifact` concretely.
- Do not re-export or depend on types from
  `crates/regseq-model/src/declaration/normalize.rs`; that
  module is legacy and slated for removal.

## Test Plan

Unit tests under
`crates/regseq-model/tests/normalize.rs`:

- **Happy path**: load
  `tests/fixtures/pl011-full-spec/expected/raw-frontend.json` via
  serde, call `normalize`, assert `Ok(model)`. Assert
  `model.modules` contains exactly one Driver module with
  expected name; assert every binding slot is `None`; assert
  every TypeDef has a classification. This test is the positive
  case for the Phase-1 post-condition.
- **Determinism**: call `normalize` twice on the same input;
  serialize both outputs to JSON; assert byte-identical.
- **Negative -- unrecognized enum tag**: mutate the fixture to
  set a `RawPropertyLint` tag to `"BogusLint"`, call
  `normalize`, assert
  `NormalizeErrorKind::UnrecognizedEnumTag { surface:
  "property_surfaces", ..., tag: "BogusLint" }` with
  populated `NormalizeProvenance`.
- **Negative -- missing required attribute**: mutate the
  fixture to remove a clock's `id`, call, assert
  `MissingRequiredAttribute { attribute: "id", .. }`.
- **Negative -- structural violation (multi-driver)**: mutate
  the fixture to have two `RawDriverIdentity` entries in
  different modules, call, assert
  `StructuralViolation { description: <mentions driver count> }`.
- **Negative -- type-constraint violation (non-Option user
  data)**: mutate fixture so a user-data field's `RegseqType`
  is `Scalar(u32)` instead of `Option(Scalar(u32))`, call,
  assert `TypeConstraintViolation`.
- **Provenance check**: for each negative test above, assert
  `NormalizeProvenance.surface_path` is non-empty and
  references the offending surface; assert `containing_item`
  is non-empty.
- **Phase-1 verifier integration**: this test proves `normalize()`
  does not silently bypass `verify_phase_1` on its own output.
  Expose a `#[cfg(test)] pub(crate) fn normalize_with_post_hook(
  input: &RawFrontendArtifact, hook: impl FnOnce(&mut RegseqModel))
  -> Result<RegseqModel, NormalizeError>` helper that runs the
  full per-surface normalization, then applies `hook(&mut model)`
  immediately before the internal `verify_phase_1` call. The test
  calls this helper with a hook that mutates one clock resource's
  `binding` to `Some(...)`, then asserts the returned error is
  `NormalizeErrorKind::Phase1VerificationFailed { inner: VerifierError {
  kind: UnexpectedBindingPopulated { slot: "clock" }, .. } }`.
  The hook is test-only (`#[cfg(test)]`) and does not exist in
  release builds.

## Review

After tests pass, use `codex:rescue` to verify: (1) every
`NormalizeError` variant in spec.md line 1361-1364 is represented;
(2) every error cites non-empty `surface_path` + `containing_item`;
(3) the determinism test is a true byte-equality assertion (not a
structural-equality shortcut); (4) the non-goal "do not modify
construction.rs" is satisfied -- `git diff --name-only main..HEAD`
must not list `crates/regseq-driver/src/construction*`; (5) the
type-constraint-violation negative test exercises the specific
example in the brief (`Option<T: Copy>` user-data rule). Iterate
until approved.

## Quality Gate

```
cargo test -p regseq-model --test normalize
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

## Acceptance Criteria

- `normalize(&RawFrontendArtifact) -> Result<RegseqModel,
  NormalizeError>` lives in `crates/regseq-model/src/normalize/`
  and is re-exported from the crate root.
- Every `NormalizeErrorKind` variant from spec.md line 1361-1364
  is implemented and tested.
- `NormalizeProvenance` is always populated; no error carries an
  empty `surface_path` or `containing_item`.
- Determinism test asserts byte-identical JSON across two runs.
- PL011 happy-path positive test passes and the resulting model
  passes `verify_phase_1`.
- `git diff --name-only` shows **zero** changes to
  `crates/regseq-driver/src/construction.rs` or any file under
  `crates/regseq-driver/src/construction/`.
- All unit tests pass.
- Codex review approved.
