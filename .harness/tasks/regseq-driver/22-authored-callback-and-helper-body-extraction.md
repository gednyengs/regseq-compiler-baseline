# Task: regseq-driver-22 - Post-drop-elaboration MIR extraction for callbacks and helpers

## Description

Extend the frontend extraction pipeline so the executable callback and
reachable-helper semantics needed by the supported PL011 slice become real
compiler input via post-drop-elaboration MIR, rather than behavior being
reconstructed later in lowering.

This task exists because the current lowering path still fabricates PL011
TX/RX/init/ISR logic inside compiler source instead of lowering from Rust's
real executable body semantics, including cleanup on failure paths and scope
exits.

## Input Context

Read before starting:

- `.harness/tasks/regseq-driver/19-raw-frontend-artifact-emission.md`
- `.harness/tasks/regseq-driver/20-serial-v1-trait-capture-alignment.md`
- `tests/fixtures/pl011-full-spec/src/driver.rs`
- `tests/fixtures/pl011-full-spec/src/registers.rs`
- `crates/regseq-driver/src/extract/`
- `crates/regseq-driver/src/raw_artifact.rs`
- `crates/regseq-model/src/declaration/raw_input.rs`
- `docs/discussions/pl011-full-spec/pl011-unified-spec-boundary.md`
- `docs/discussions/pl011-full-spec/pl011-full-spec-status.md`
- `/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_hir/src/hir.rs`
- `/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_hir/src/def.rs`
- `/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_hir/src/intravisit.rs`
- `/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_middle/src/mir/mod.rs`
- `/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_middle/src/mir/syntax.rs`
- `/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_middle/src/mir/statement.rs`
- `/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_middle/src/mir/terminator.rs`
- `/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_middle/src/query/mod.rs`
- `/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_mir_transform/src/lib.rs`

## Output Specification

Modify:

- `crates/regseq-driver/src/`
- `crates/regseq-driver/tests/`
- `crates/regseq-model/src/declaration/raw_input.rs`
- raw frontend fixture snapshots only where needed for deterministic proof

Add frontend extraction support for MIR bodies from rustc's
`mir_drops_elaborated_and_const_checked` query for callbacks and reachable
helpers in the supported subset.

## Implementation Notes

Implementation continuity note:

- a previous implementation attempt was rejected because
  `crates/regseq-driver/src/extract/body.rs` still extracted executable bodies
  from HIR into `BodyExpr`
- do not preserve that HIR `BodyExpr` path as the default executable-body
  extraction route
- the concrete migration goal for this task is:
  - keep HIR only for root discovery / declarative structure
  - replace executable-body extraction with MIR from
    `mir_drops_elaborated_and_const_checked`
  - thread that MIR-derived data into the raw artifact

This task must extend the real frontend extraction pipeline, not create an
analysis-local side channel or a hand-constructed body representation inside
analysis or lowering.

The required data path is:

- authored Rust fixture source
- rustc `mir_drops_elaborated_and_const_checked` MIR for reachable
  callbacks/helpers
- `regseq-driver` extraction
- serialized raw frontend artifact
- `regseq-model` raw-input representation

Extraction algorithm requirements:

- use HIR only to discover the authored roots:
  - driver type
  - callback method `LocalDefId`s
  - candidate inherent helper `LocalDefId`s reachable from that driver type
- use MIR to extract executable semantics:
  - call `tcx.mir_drops_elaborated_and_const_checked(local_def_id)`
  - immediately `.borrow()` the returned `Steal<mir::Body<'tcx>>` inside
    `after_analysis`
  - do not trigger `optimized_mir` for the same `DefId` during extraction
- discover reachable helpers transitively from MIR bodies:
  - start from authored callback / ISR roots
  - inspect `Call` terminators (and any other supported call-bearing MIR form)
  - resolve local callee `DefId`s
  - continue until the reachable local-helper closure is complete

The raw frontend artifact must carry an owned, serializable MIR-derived
representation. It cannot expose rustc arena-bound types directly.

The extracted MIR representation should preserve, for the supported subset:

- per-function identity and provenance
- whether the function is a callback or a reachable helper
- helper-call edges
- local declarations and their types as needed for later lowering
- basic-block CFG structure
- statements and terminators
- successor / cleanup-edge structure
- calls
- returns and failure exits
- drops / cleanup edges introduced by post-drop-elaboration MIR
- expressions and intermediate computations needed for correct codegen
- register/property/resource operations that appear in executable semantics
- enough source/debug provenance to relate MIR entities back to authored
  functions and meaningful names later

The extraction should cover:

- authored `Lifecycle` callbacks
- authored `Serial` callbacks
- authored ISR handlers
- all reachable helper functions needed by the supported slice

For the supported PL011 slice, the extraction should be prepared to capture at
least these MIR forms when they appear:

- statements:
  - `Assign`
  - `StorageLive`
  - `StorageDead`
  - `SetDiscriminant` where relevant
  - `Nop`
- terminators:
  - `Goto`
  - `SwitchInt`
  - `Call`
  - `Drop`
  - `UnwindResume`
  - `UnwindTerminate`
  - `Return`
  - `Assert`
  - unwind actions attached to `Call` / `Drop`, including:
    - `Continue`
    - `Unreachable`
    - `Terminate(reason)`
    - `Cleanup(BasicBlock)`

- rvalues/operands expected in the supported slice:
  - `Use`
  - `Ref`
  - `BinaryOp`
  - `UnaryOp`
  - `Discriminant`
  - `Aggregate`
  - `Cast`
  - `CopyForDeref` where it appears in the selected compiler version
  - `Operand::Constant`

If other MIR forms appear in the supported slice, capture them with enough
owned structure and provenance for later validation/diagnostics rather than
silently discarding them.

The extraction must also define a rustc-independent type serialization
strategy for the supported subset. At minimum, it must specify how to
serialize:

- primitives
- references and mutability
- ADTs by stable identity plus generic arguments as needed
- tuples / arrays if they appear
- unsupported type forms with deterministic diagnostics

This task should also make the migration/disposition of the existing HIR body
pipeline explicit. The default expectation is a clean replacement for
executable body capture rather than indefinite coexistence of HIR-body and
MIR-body pipelines.

Scalar constant/value serialization must also be explicit for the supported
subset. At minimum:

- integer and boolean constants should serialize as owned literal values
- simple string/byte literals may serialize as owned literal payloads if they
  appear
- complex constants outside the supported slice, including promoted/statics
  that the owned MIR model cannot represent faithfully yet, should be surfaced
  as unsupported with stable provenance rather than silently erased

Critical negative constraints:

- do not manually construct PL011 callback semantics in the extractor
- do not recreate drop elaboration manually in `regseq-driver`; use the rustc
  `mir_drops_elaborated_and_const_checked` query that already contains explicit
  drop/cleanup semantics
- do not switch to `optimized_mir` for this task; later optimization passes can
  erase or rewrite structure and cleanup in ways that are bad for faithful
  extraction
- do not define MIR-side node types that are specific to PL011 register names,
  interrupt classes, FIFO policies, or UART-only helper templates
- do not define MIR-side node types that are Linux-specific or encode Linux APIs
- do not silently discard MIR statements, terminators, or cleanup edges that
  are inconvenient for later lowering
- do not serialize `Ty<'tcx>` or other rustc arena-bound values directly into
  raw artifacts
- if a construct is outside the supported subset, capture enough provenance to
  diagnose/reject it later rather than silently dropping it

This task is about faithful MIR capture. It should not itself lower to
portable semantic behavior or Linux codegen decisions, and it does not need to
prettify MIR for human readability.

## Test Plan

Add tests to verify:

1. callback MIR bodies are present in the raw frontend artifact
2. reachable helper MIR bodies are present in the raw frontend artifact
3. per-function identity/provenance is preserved
4. drop/cleanup semantics from `mir_drops_elaborated_and_const_checked` are
   preserved in the raw artifact for representative failure/scope-exit cases
5. body capture flows through the real extraction pipeline rather than being
   manually constructed downstream
6. helper discovery follows the real HIR-root plus MIR-call-graph path rather
   than a manually curated helper list
7. extraction completes without `Steal` panics for the supported callback and
   helper set
8. unsupported constructs in the body subset are surfaced deterministically

Test commands:

- `cargo test -p regseq-driver`
- `cargo clippy -p regseq-driver -- -D warnings`

## Acceptance Criteria

- raw frontend artifacts contain MIR-derived body data for callbacks and required helpers
- body data is produced by the real extraction pipeline from `mir_drops_elaborated_and_const_checked`
- captured bodies are scoped per function and preserve helper-call structure
- cleanup/drop semantics needed for later lowering are present in captured data
- the raw body artifact uses an owned, rustc-independent MIR representation
- helper reachability is derived from real callback-root MIR traversal
- extraction remains OS-neutral and device-neutral in representation
- tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
