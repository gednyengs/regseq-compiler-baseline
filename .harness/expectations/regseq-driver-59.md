# regseq-driver-59: implementation evidence

Implementation deliverable for `regseq-driver-59` (Changes A-C of
the task spec at
`.harness/tasks/regseq-driver/59-foreign-rendering-and-default-trait-method-tests.md`).
Closes the two Test Plan coverage gaps left by regseq-driver-58
(commit `e7796d7`): B8 (foreign-decl rendering with the
`#[foreign]` attribute form per pretty-printer.md Specimens 113 and
120) and B9 (default trait method bodies surface as
`ItemKind::Function` Items at the trait's DefPath, per the
regseq-driver-58 Change H amendment).

Parent context: `.harness/expectations/regseq-driver-58.md`.

## Scope

Pure test-surface additions. Two new fixtures, two new e2e
snapshot + structural tests, plus this expectations file. No
production code under `crates/regseq-driver/src/` or
`crates/regseq-model/src/` is modified. No spec or pretty-printer
amendment is touched.

## Coverage-gap-resolution evidence

### B8 -- foreign-decl rendering (`#[foreign]` attribute)

- **Gap (before this task)**: the regseq-driver-58 B3 test at
  `crates/regseq-driver/tests/bridge_eager_hir_enumeration.rs:305-360`
  asserts the structural shape of foreign Instances (`body == None`,
  `init == None`, `linkage == Foreign`) but does NOT assert the
  rendered `#[foreign]\n...;` form per pretty-printer Specimens 113
  (`pretty-printer.md:2098-2117`, foreign function) and 120
  (`pretty-printer.md:2236-2254`, foreign static). A pretty-printer
  regression dropping `#[foreign]` or changing the body-less
  declaration shape would slip through.

- **Resolution evidence**:
  - New fixture:
    `crates/test-ir-fixtures/fixtures/foreign_rendering.rs` --
    `extern "C"` block declaring `extern_helper(x: u32) -> u32`
    and `EXTERN_VALUE: u32`, plus a source-defined `entry` fn.
  - New `fixture_names::FOREIGN_RENDERING` constant at
    `crates/test-ir-fixtures/src/lib.rs` plus the corresponding
    entry in the manual fixture list within the same file.
  - New e2e test:
    `crates/regseq-model/tests/e2e_foreign_rendering.rs` bridges
    the fixture, calls `PrettyPrinter::render`, and snaps via
    `insta::assert_snapshot!`. Content assertions cover the
    `#[foreign]\nfn`, `#[foreign]\nstatic`, `extern_helper`,
    `EXTERN_VALUE`, and `entry` substrings. The negative
    assertion (per-chunk scan) verifies the source-defined
    `entry` fn does NOT carry `#[foreign]`.
  - Snapshot file:
    `crates/regseq-model/tests/snapshots/e2e_foreign_rendering__render.snap`.

### B9 -- default trait method body coverage

- **Gap (before this task)**: the regseq-driver-58 Change H
  amendment at `docs/discussions/regseq-model/spec.md:2351-2358`
  ("Default trait method bodies are owned by the trait, not by
  any impl. The body lives at the trait's DefPath, and surfaces
  as an `ItemKind::Function` Item at the trait's DefPath") is
  normative but has no automated guard. The eager HIR walk's
  `tcx.is_mir_available(def_id)` predicate is the load-bearing
  check that distinguishes trait methods with default bodies
  from declarations only. A regression that filtered out trait
  associated fns, or swapped the predicate for one keyed on
  `impl_of_assoc.is_some()`, would silently break the contract.

- **Resolution evidence**:
  - New fixture:
    `crates/test-ir-fixtures/fixtures/trait_default_method.rs` --
    `pub trait Greeter { fn greet(&self) -> u32 { 42 } }`,
    `pub struct S`, `impl Greeter for S {}` (no override), and a
    caller `pub fn entry(s: &S) -> u32 { s.greet() }`.
  - New `fixture_names::TRAIT_DEFAULT_METHOD` constant at
    `crates/test-ir-fixtures/src/lib.rs` plus the corresponding
    entry in the manual fixture list within the same file.
  - New e2e test:
    `crates/regseq-model/tests/e2e_trait_default_method.rs`.
    Part 1 makes pure-model structural assertions (no `TyCtxt`
    escape): exactly one Function Item named `greet`; no
    `DefPathData::Impl` ancestor; bucket-1 BridgeItemFlags
    (`is_generic == false`, `signature_non_rigid == false`);
    exactly one Function Instance with `body == Some(_)`; no
    Function Item with both an `Impl` ancestor AND terminal
    `ValueNs("greet")`; no Item with terminal `TypeNs("Greeter")`
    (trait container leak guard). Part 2 snaps the rendered
    model and asserts the `42_u32` default-body literal plus the
    block-body rendering shape.
  - Snapshot file:
    `crates/regseq-model/tests/snapshots/e2e_trait_default_method__render.snap`.

## Out of scope (per task Non-Goals)

- No changes to production code under `crates/regseq-driver/src/`
  or `crates/regseq-model/src/`. The regseq-driver-58
  implementation already correctly enumerates foreign decls and
  default trait method bodies; this task only adds the automated
  guards. Confirmed by `git diff main --stat --
  crates/regseq-driver/src/ crates/regseq-model/src/` returning
  zero lines.
- No spec or pretty-printer amendment. The Change H amendment
  at `spec.md:2351-2358` and the foreign-decl rendering specimens
  at `pretty-printer.md:2098-2117` (113) and `:2236-2254` (120)
  remain authoritative.
- No modifications to the regseq-driver-58 test files
  (`bridge_eager_hir_enumeration.rs`,
  `validate_bridge_completeness_buckets.rs`). Both remain
  authoritative for their bucket-coverage classes; the new
  e2e tests are additive.
- No four-bucket validator extension. The four-bucket policy from
  regseq-driver-58 stays intact.
- No `tests/fixtures/playground/` modification. Playground is
  the user's experimentation space.
