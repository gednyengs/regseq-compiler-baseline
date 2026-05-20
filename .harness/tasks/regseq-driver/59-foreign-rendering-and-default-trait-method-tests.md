# regseq-driver-59: Foreign-decl rendering snapshot + default trait method body coverage tests

## Description

Close the two Test Plan coverage gaps left by the regseq-driver-58
implementation (commit `e7796d7`): B8 (foreign-decl rendering with the
Specimens 113 + 120 `#[foreign]` attribute form) and B9 (default trait method
bodies surface as `ItemKind::Function` at the trait's DefPath -- the
Change H spec amendment from regseq-driver-58).

The regseq-driver-58 implementation correctly realized the four-bucket
Item/Instance contract:

- The eager HIR walk at
  `crates/regseq-driver/src/bridge/mod.rs` covers
  `DefKind::Fn | AssocFn` including default trait method bodies gated
  on `tcx.is_mir_available(def_id)`.
- The validator at `crates/regseq-model/src/model.rs:3528-3633`
  enforces the four-bucket policy via named `InvariantViolation`
  variants (`Bucket1MultipleInstances`, `Bucket4HasInstance`,
  `ForeignInstanceBodyOrInitNotNone`).
- The spec.md §8.1 amendment at
  `docs/discussions/regseq-model/spec.md:2349-2358` records the
  Change H contract: "Default trait method bodies are owned by the
  trait, not by any impl. The body lives at the trait's DefPath, and
  surfaces as an `ItemKind::Function` Item at the trait's DefPath."

Two test classes from the regseq-driver-58 Test Plan were NOT covered
by the landed implementation:

1. **B8 -- foreign-decl rendering (`#[foreign]` attribute)**. The
   regseq-driver-58 B3 test at
   `crates/regseq-driver/tests/bridge_eager_hir_enumeration.rs:305`
   verifies the structural shape of foreign Instances
   (`body == None` / `init == None`, `linkage == Foreign`) but does
   NOT assert the rendered `#[foreign]\n...;` form per
   `pretty-printer.md` Specimens 113 + 120 (`pretty-printer.md:2098-2117`
   foreign function; `:2236-2254` foreign static).
   A pretty-printer regression that dropped the `#[foreign]` attribute
   or changed the declaration shape would slip through.

2. **B9 -- default trait method body coverage**. The Change H spec
   amendment is normative but has NO automated guard. The eager HIR
   walk's `tcx.is_mir_available(def_id)` predicate is the load-bearing
   check that distinguishes trait methods with default bodies from
   declarations only. A future regression that:
   - Filtered out trait associated fns entirely (e.g., skipping
     `DefKind::AssocFn` with `tcx.trait_of_assoc(def_id).is_some()`)
   - Changed the predicate to e.g. `impl_of_assoc.is_some()` (which
     would still pass B1-B5 because nothing in those fixtures has a
     trait with a default body)

   ... would silently break the Change H contract. We need a fixture
   that exercises the exact path.

After this task lands:

- A new e2e test bridges a foreign-decl fixture end-to-end and
  asserts the rendered output contains `#[foreign]\nfn ...;` and
  `#[foreign]\nstatic ...;` shapes per Specimens 113 (foreign function)
  and 120 (foreign static).
- A new e2e test bridges a trait-with-default-body fixture
  end-to-end and asserts the default method body surfaces as a
  Function Item at the trait's DefPath (not at any impl).
- Both tests serve as regression guards for the regseq-driver-58
  contract.

Both test additions are pure test surface. No production code
change, no spec amendment.

## Input Context

### Spec / pretty-printer anchors

- `docs/discussions/regseq-model/spec.md:2349-2358` -- the Change H
  amendment establishing default trait method body Item shape.
- `docs/discussions/regseq-model/spec.md:2618-2654` -- §9.4 Function
  variant `body: None` carve-out for foreign decls.
- `docs/discussions/regseq-model/spec.md:2668-2677` -- §9.4 Static
  variant `init: None` for foreign decls.
- `docs/discussions/regseq-model/pretty-printer.md:2098-2117`
  Specimen 113 -- the canonical foreign-function rendering form:
  ```
  #[foreign]
  extern "C" fn libc::printf(_1: *const u8) -> i32;
  ```
- `docs/discussions/regseq-model/pretty-printer.md:2236-2254`
  Specimen 120 -- the canonical foreign-static rendering form:
  ```
  #[foreign]
  static libc::stderr: *mut libc::FILE;
  ```
- `docs/discussions/regseq-model/pretty-printer.md:3640-3698`
  Static rendering rule -- `static [mut ]<path>: <ty>[ = <init>];`
  with the `init: None` trailer.

### Existing test patterns to mirror

- `crates/regseq-model/tests/e2e_trait_method_resolution.rs` (from
  regseq-driver-56) -- the canonical e2e snapshot pattern: rustc
  callback compiles a fixture, bridges it, calls
  `PrettyPrinter::render`, snaps via insta.
- `crates/regseq-model/tests/snapshots/e2e_trait_method_resolution__render.snap`
  -- the snapshot format. New snapshots in this task follow the same
  format.
- `crates/regseq-driver/tests/bridge_eager_hir_enumeration.rs:305-364`
  (B3) -- structural foreign-Instance assertions; B8 extends this
  with a rendered-form snapshot.

### Bridge code anchors (read-only -- no changes in this task)

- `crates/regseq-driver/src/bridge/mod.rs` HIR walk for
  `DefKind::AssocFn` with default body via `tcx.is_mir_available`.
- `crates/regseq-driver/src/bridge/mod.rs` foreign-decl HIR walk
  for `DefKind::ForeignFn` / `ForeignStatic`.

## Implementation Notes

### Change A: B8 -- foreign-decl rendering snapshot

**New fixture**: `crates/test-ir-fixtures/fixtures/foreign_rendering.rs`.

```rust
#![no_std]

extern "C" {
    pub fn extern_helper(x: u32) -> u32;
    pub static EXTERN_VALUE: u32;
}

pub fn entry() {}
```

The `pub fn entry() {}` keeps the crate non-empty and ensures the
bridge MonoItem walk has at least one source-defined fn to start
from. The foreign decls are reached via the eager HIR walk added in
regseq-driver-58 Change B.

**Fixture registration**: also add a `FOREIGN_RENDERING: &str =
"foreign_rendering"` constant to the `fixture_names` module at
`crates/test-ir-fixtures/src/lib.rs:34-64` (mirroring the existing
`TRAIT_METHOD_RESOLUTION` entry from regseq-driver-56). If the crate
maintains a manual fixture list at
`crates/test-ir-fixtures/src/lib.rs:283-291` (per Codex's hint),
add `"foreign_rendering"` there too.

**New test**: `crates/regseq-model/tests/e2e_foreign_rendering.rs`.

Follow the e2e_trait_method_resolution.rs pattern: rustc callback
+ `compile_with_bridge` helper + `PrettyPrinter::render` +
`insta::assert_snapshot!`. The snapshot asserts the rendered output
contains:

- `#[foreign]\nfn ...::extern_helper(_1: u32) -> u32;` (foreign-fn
  declaration, terminated with semicolon, no body).
- `#[foreign]\nstatic ...::EXTERN_VALUE: u32;` (foreign-static
  declaration, terminated with semicolon, no `= init`).
- `fn ...::entry() -> () { ... }` (source-defined fn with full body
  -- sanity-check that the rendering does NOT incorrectly mark
  source-defined fns with `#[foreign]`).

Snapshot file:
`crates/regseq-model/tests/snapshots/e2e_foreign_rendering__render.snap`.

### Change B: B9 -- default trait method body coverage

**New fixture**: `crates/test-ir-fixtures/fixtures/trait_default_method.rs`.

```rust
#![no_std]

pub trait Greeter {
    fn greet(&self) -> u32 { 42 }
}

pub struct S;
impl Greeter for S {}

pub fn entry(s: &S) -> u32 {
    s.greet()
}
```

The trait `Greeter` declares `greet` with a default body. The impl
`Greeter for S` does NOT override `greet`. Per Change H from
regseq-driver-58 (`spec.md:2349-2358`), the default body must surface
as a `ItemKind::Function` Item at the trait's DefPath
(`...::Greeter::greet`), and the trait container itself stays erased.

**Fixture registration**: also add a `TRAIT_DEFAULT_METHOD: &str =
"trait_default_method"` constant to the `fixture_names` module at
`crates/test-ir-fixtures/src/lib.rs:34-64` (same pattern as Change
A's fixture). Update the manual fixture list at
`crates/test-ir-fixtures/src/lib.rs:283-291` correspondingly.

**New test**: `crates/regseq-model/tests/e2e_trait_default_method.rs`.

Two-part test:

**Part 1 -- structural assertion** (pure-model, no `TyCtxt` escape).

Codex Pass 1 flagged that the existing
`crates/test-ir-fixtures/src/lib.rs:78-92` helper returns only
`RegseqModel` and does NOT expose `TyCtxt` from the rustc callback,
so any predicate computed via `tcx.trait_of_assoc` / `tcx.impl_of_assoc`
must be moved into a rustc-callback-local helper. The cleaner
alternative is to do the structural assertion purely on the model:
the bridge has already classified the Item via its def_path segments
and `BridgeItemFlags`, so the test does not need `TyCtxt` at all.

Assertions on `model: &RegseqModel` only:

- Walk `model.items` and find every Function Item whose def_path's
  terminal segment is `DefPathData::ValueNs(s)` where the resolved
  string `s == "greet"`. Call this set `greet_items`.
- Assert `greet_items.len() == 1`: exactly one Function Item named
  `greet` exists (regression guard against accidental duplicate
  enumeration via both trait-default-body AND impl-method paths).
- Assert the single `greet_item`'s def_path segments contain NO
  `DefPathData::Impl` segment (i.e., the Item is anchored at the
  trait's container, not at any `impl#N` block). This is the
  load-bearing Change H assertion: a regression that mis-enumerated
  the default body as an impl method would fail this check.
- Assert `model.item_bridge_flags[&greet_item].is_generic == false`
  and `model.item_bridge_flags[&greet_item].signature_non_rigid == false`
  (bucket 1 classification).
- Assert there is exactly one Function Instance keyed on
  `greet_item` and that its `body == Some(_)` (the default body
  materializes).
- Assert no Item exists whose def_path's terminal segment is
  `DefPathData::TypeNs(s)` with `s == "Greeter"`. If such an Item
  existed, the trait container would be leaking into `model.items`,
  which violates `spec.md:2354-2355` ("`ItemKind` has six variants
  -- `Function`, `Static`, `Const`, `Struct`, `Enum`, `Union`. No
  `Trait` variant.").

These assertions are all on the model's Item/Instance/BridgeItemFlags
state -- no `TyCtxt` access required.

**Part 2 -- snapshot assertion**:

Snap the rendered model and assert it contains:
- `fn ...::Greeter::greet(_1: &...::S) -> u32 { ... 42_u32 ... }`
  -- the default body rendered at the trait's DefPath. (Note: the
  rendered Self type substitution may vary by rustc behavior; the
  snapshot pins the actual observed shape.)
- `fn ...::entry(_1: &...::S) -> u32 { ... }` -- the caller body.
- No body-less `;`-terminated declaration for `Greeter::greet`
  (since the default body is present, the rendering is a full
  function definition, not a declaration).

Snapshot file:
`crates/regseq-model/tests/snapshots/e2e_trait_default_method__render.snap`.

### Change C: harness expectations addendum

**Anchor**: `.harness/expectations/regseq-driver-59.md` (new file).

Record:
- Reference to regseq-driver-58's expectations file as the parent
  context.
- The two specific coverage gaps and their resolution via Changes
  A + B.
- A note that this is pure test-surface work -- no production code
  change, no spec amendment, no schema change.

## Non-Goals

- Do not modify any production code under
  `crates/regseq-driver/src/` or `crates/regseq-model/src/`. This
  task is pure test-surface coverage; if the existing
  regseq-driver-58 implementation correctly enumerates foreign
  decls and default trait method bodies, both new tests pass
  as-is. If they fail, that's a regseq-driver-58 implementation
  bug to file as a separate follow-up -- not in scope here.
- Do not modify the spec.md §8.1 Change H amendment. The amendment
  is already normative; this task adds the automated guard.
- Do not modify `pretty-printer.md`. Specimen 113 (foreign function)
  and Specimen 120 (foreign static) are already specified.
- Do not modify the regseq-driver-58 test files
  (`bridge_eager_hir_enumeration.rs`, `validate_bridge_completeness_buckets.rs`).
  Both remain authoritative for their bucket-coverage classes; the
  new tests are e2e snapshot additions, not replacements.
- Do not extend the four-bucket validator. The four-bucket policy
  from regseq-driver-58 stays intact.
- Do not modify `tests/fixtures/playground/`. Playground is the
  user's experimentation space.

## Test Plan

### B8: foreign-decl rendering snapshot

**New file**: `crates/regseq-model/tests/e2e_foreign_rendering.rs`.

After bridging the new
`crates/test-ir-fixtures/fixtures/foreign_rendering.rs` fixture:

- Render via `PrettyPrinter::render`.
- Snap via `insta::assert_snapshot!`.
- The snapshot's literal content must contain:
  - `#[foreign]\nfn` substring (proves the foreign-fn rendering form).
  - `#[foreign]\nstatic` substring (proves the foreign-static rendering form).
  - `extern_helper` def_path segment in the foreign-fn body-less
    declaration.
  - `EXTERN_VALUE` def_path segment in the foreign-static body-less
    declaration.
  - `entry` def_path segment in a full-body fn definition WITHOUT
    `#[foreign]`.

### B9: default trait method body coverage

**New file**: `crates/regseq-model/tests/e2e_trait_default_method.rs`.

After bridging the new
`crates/test-ir-fixtures/fixtures/trait_default_method.rs` fixture:

- Structural assertions per Change B Part 1 (Function Item with
  terminal `ValueNs("greet")` segment, NO `DefPathData::Impl`
  ancestor, `is_generic == false`, `signature_non_rigid == false`,
  exactly one Function Instance with `body == Some(_)`).
- Snapshot assertion per Change B Part 2 (rendered output contains
  the default body's `42_u32` and the caller body).
- Negative assertions: no Item exists whose def_path terminal
  segment is `TypeNs("Greeter")` (trait container would leak if
  present); no Function Item exists with both `DefPathData::Impl`
  ancestor AND terminal `ValueNs("greet")` (catches mis-enumeration
  as an impl method).

### B10: workspace test suite stays green

`cargo test --workspace -- --test-threads=1` green. Specifically
watch:

- All regseq-driver-55 / 56 / 57 / 58 coverage tests stay green
  (Changes A + B are additive).
- The new `e2e_foreign_rendering` + `e2e_trait_default_method`
  tests pass on first run (initial `insta` snapshot acceptance via
  `cargo insta review` or `INSTA_UPDATE=always` during development;
  the committed snapshot is the pinned regression target).

## Review

### Files to send to Codex (`scripts/python/codex_runner.py`)

- This file (`.harness/tasks/regseq-driver/59-foreign-rendering-and-default-trait-method-tests.md`).

(No spec amendment in this task.)

### Reviewer focus

- **Test gap claim accuracy**: does the Description correctly
  identify that B8 + B9 are missing from the regseq-driver-58
  implementation? Confirm by grepping
  `crates/regseq-driver/tests/` and `crates/regseq-model/tests/`
  for `#[foreign]` substring assertions (B8) and default-trait-method
  fixtures (B9).
- **Fixture viability**: do the proposed `foreign_rendering.rs` and
  `trait_default_method.rs` fixtures actually exercise the intended
  bridge paths? Specifically: does the eager HIR walk in
  regseq-driver-58 reach `extern "C"` blocks at the crate root?
  Does `tcx.is_mir_available` return `true` for the
  `Greeter::greet` default body?
- **Snapshot assertion completeness**: do the snapshot content checks
  in Change A and Change B catch the contract violations they aim
  for? Specifically: would removing the `#[foreign]` attribute from
  the pretty-printer cause B8 to fail? Would the eager HIR walk
  failing to enumerate `Greeter::greet` cause B9 to fail?
- **Negative-test discipline**: does Part 1 of Change B explicitly
  assert the negative cases (no Item whose def_path terminal is
  `TypeNs("Greeter")`; no Function Item with `DefPathData::Impl`
  ancestor AND terminal `ValueNs("greet")`)? Per
  `feedback_no_defensive_fallbacks`, the test should not silently
  accept ANY Function Item named `greet`; it should specifically
  check the trait-container shape.
- **Scope creep**: the task is test-surface only. Does any Change
  edit production code, spec.md, or pretty-printer.md? Flag any.

### Adversarial prompt template

```
<task>
Review .harness/tasks/regseq-driver/59-foreign-rendering-and-default-trait-method-tests.md.
Identify:
- Citation drift (any file:line that does not anchor the claim).
- Fixture viability for the proposed e2e tests.
- Snapshot assertion completeness.
- Scope creep beyond test-surface additions.
</task>

<grounding_rules>
- Cite file:line for every claim.
- Mark hypotheses as hypotheses.
- Don't speculate about code or spec sections you didn't read.
</grounding_rules>

<structured_output_contract>
VERDICT: APPROVED | APPROVED WITH MINOR FIXES | NOT READY
PER-CHANGE VERIFICATION: A, B, C bullets with file:line
CITATION DRIFT: list or "none"
FIXTURE-VIABILITY CHECK: B8 fixture / B9 fixture -- PASS / FAIL per fixture
SNAPSHOT-COMPLETENESS CHECK: B8 / B9 -- PASS / FAIL with citation
SCOPE-CREEP CHECK: pass/fail with citation
NEW ISSUES: numbered list or "none"
REQUIRED FIXES: numbered list (only if VERDICT != APPROVED)
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, decide, stop.
</default_follow_through_policy>
```

## Quality Gate

- `cargo fmt --all` clean.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo test --workspace -- --test-threads=1` green; specifically
  the two new e2e tests pass against committed snapshots.
- New snapshot files committed:
  - `crates/regseq-model/tests/snapshots/e2e_foreign_rendering__render.snap`
  - `crates/regseq-model/tests/snapshots/e2e_trait_default_method__render.snap`
- No production code under `crates/regseq-driver/src/` or
  `crates/regseq-model/src/` modified. Confirm via `git diff main
  --stat -- crates/regseq-driver/src/ crates/regseq-model/src/`
  returning zero lines.
- Codex Task Review verdict: APPROVED or APPROVED WITH MINOR FIXES.

## Acceptance Criteria

1. **B8 e2e test lands**:
   `crates/regseq-model/tests/e2e_foreign_rendering.rs` exists and
   bridges `crates/test-ir-fixtures/fixtures/foreign_rendering.rs`
   end-to-end, asserting the rendered output contains the
   `#[foreign]\nfn` shape per Specimen 113 (foreign function) and the
   `#[foreign]\nstatic` shape per Specimen 120 (foreign static).

2. **B9 e2e test lands**:
   `crates/regseq-model/tests/e2e_trait_default_method.rs` exists
   and bridges `crates/test-ir-fixtures/fixtures/trait_default_method.rs`
   end-to-end, asserting structurally that the default trait method
   body surfaces as a Function Item at the trait's DefPath, and
   the snapshot pins the rendered default-body form.

3. **Negative assertions present**:
   - B8 asserts that source-defined `entry()` does NOT carry the
     `#[foreign]` attribute (regression guard against accidental
     attribute over-application).
   - B9 asserts that no Item whose def_path terminal segment is
     `TypeNs("Greeter")` exists (trait-container leak guard); no
     Function Item with both a `DefPathData::Impl` ancestor AND
     terminal `ValueNs("greet")` exists (regression guard against
     the default-body Item being mis-anchored to an impl block);
     exactly one Function Item named `greet` exists overall
     (duplicate-enumeration guard).

4. **No production code changes**: `git diff main --stat --
   crates/regseq-driver/src/ crates/regseq-model/src/` returns
   zero. The task is pure test surface.

5. **No spec / pretty-printer changes**: `git diff main --stat --
   docs/` returns zero (no normative documents are touched by
   this task).

6. **Snapshot files committed**: two new `.snap` files under
   `crates/regseq-model/tests/snapshots/` are checked in.

7. **Full workspace test suite passes** including the two new
   e2e tests.

8. **Expectations file lands**:
   `.harness/expectations/regseq-driver-59.md` records the
   coverage-gap-resolution evidence with file:line citations
   against the new test files.
