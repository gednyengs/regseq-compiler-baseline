# Task: regseq-model-13 - MIR body schema and normalization

## Description

Add the raw and normalized model support needed to carry MIR from rustc's
`mir_drops_elaborated_and_const_checked` query for callbacks and helpers from
frontend extraction into analysis without collapsing it into PL011-specific or
Linux-specific semantics.

This task creates the model boundary between:

- captured MIR body structure from extraction
- later portable semantic lowering in Phase D

## Input Context

Read before starting:

- `.harness/tasks/regseq-driver/22-authored-callback-and-helper-body-extraction.md`
- `.harness/tasks/regseq-model/06-raw-frontend-artifact-to-declaration-model-normalization.md`
- `.harness/tasks/regseq-model/07-declaration-model-reviewability-and-traversal-baseline.md`
- `crates/regseq-model/src/declaration/raw_input.rs`
- `crates/regseq-model/src/declaration/`
- `tests/fixtures/pl011-full-spec/src/driver.rs`
- `/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_middle/src/mir/mod.rs`
- `/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_middle/src/mir/syntax.rs`
- `/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_middle/src/mir/statement.rs`
- `/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_middle/src/mir/terminator.rs`
- `/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_middle/src/query/mod.rs`

## Output Specification

Modify:

- `crates/regseq-model/src/declaration/`
- `crates/regseq-model/tests/`
- raw/declaration fixture expectations only where needed

Add typed raw and normalized body representations that preserve MIR body
structure for the supported subset.

## Implementation Notes

Implementation continuity note:

- a previous implementation attempt was rejected because
  `crates/regseq-model/src/declaration/bodies.rs` still defined the executable
  body path as the old HIR-style `BodyExpr` tree
- this task must not keep that expression-tree model as the default executable
  body representation
- the concrete migration goal is:
  - introduce owned MIR-oriented body types
  - normalize raw MIR capture into those owned MIR types
  - make the MIR CFG/basic-block model the canonical executable-body path
  - keep any temporary HIR-body coexistence explicitly transitional and clearly
    named

The model layer should introduce two distinct concepts:

- captured MIR body representation
- normalized declaration-side MIR representation

Do not collapse this into the later backend-oriented `BehaviorPlan`. The model
must preserve MIR structure, not already-performed lowering decisions.

This model is not a recursive expression tree. It must be a control-flow-graph
representation of MIR bodies:

- a per-function body
- local declarations
- basic blocks
- ordered statements per block
- exactly one terminator per block
- explicit successor / cleanup-edge structure

Do not treat “equivalent normalized concepts” as license to reconstruct MIR
back into HIR-like `BodyExpr` trees.

The normalized MIR model should preserve:

- per-function identity
- callback-vs-helper role
- helper-call graph edges
- locals and relevant type information
- basic blocks and their stable identities
- statements
- terminators
- control flow
- calls
- returns
- drops / cleanup semantics
- places
- operands
- rvalues
- references to registers, properties, and resources as they appear in the
  executable semantics

The normalized model must define rustc-independent owned types for at least:

- `MirType`
- `MirLocalDecl`
- `MirBasicBlock`
- `MirStatement`
- `MirTerminator`
- `MirPlace`
- `MirProjectionElem`
- `MirOperand`
- `MirRvalue`
- constants/provenance/query-flavor tags as needed

Critical negative constraints:

- no Linux-specific APIs or concepts in model body types
- no PL011-specific register names, mask classes, or helper templates embedded
  as model variants
- no device-class-specific semantic nodes whose meaning already assumes UART
  behavior
- do not normalize away cleanup/drop structure just to make the model look more
  source-like
- do not require pretty, source-like rendering at this stage; correctness of
  executable semantics is the priority
- do not silently assume this model is "optimized MIR"; preserve the query
  flavor explicitly so later work does not accidentally swap in `optimized_mir`
- do not keep the old HIR `BodyExpr` model as the default executable-body path;
  if temporary coexistence is needed during migration, the distinction must be
  explicit in type names and raw artifact schema

The normalized model should remain sufficiently generic that later OS-specific
backends can consume lowering products for Linux, Zephyr, Windows, FreeBSD,
and macOS without the body model having already chosen an OS mechanism.

## Test Plan

Add tests to verify:

1. raw MIR body structures deserialize/normalize correctly
2. normalized bodies preserve function boundaries and helper reachability
3. normalized bodies preserve control flow and cleanup semantics needed by the supported subset
4. normalization is deterministic and reviewer-readable enough for inspection, even if not pretty
5. CFG structure is preserved rather than reconstituted into HIR-like expression trees
6. normalization rejects malformed or unsupported body structures cleanly

Test commands:

- `cargo test -p regseq-model`
- `cargo clippy -p regseq-model -- -D warnings`

## Acceptance Criteria

- model layer carries MIR-derived body data from raw artifact to normalized declaration model
- captured-body types remain device-neutral and OS-neutral
- normalized body representation is clearly distinct from later portable semantic lowering
- cleanup/drop semantics remain available for later lowering
- normalized body representation is CFG-based, not expression-tree-based
- tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
