# Phase-7 Migration Design: Consumers of `RegseqModel`

> **Status**: Design discussion. Supersedes
> `.harness/spec-batches/regseqmodel-phase-7.md` as the source of truth
> for what Phase 7 actually should do. Once approved, the brief and the
> six already-authored task specs (`regseq-model-61`,
> `regseq-analysis-06`, `regseq-lowering-10`,
> `regseq-linux-backend-03`, `regseq-driver-35`,
> `track-a-validation-11`) will be rewritten or retired against this
> document.

## Why this document exists

The six Phase-7 specs framed consumer migration as *"keep the consumer
APIs intact; route their data through a `compat.rs` adapter that
projects `RegseqModel` back to the old shape."* An implementer attempting
`regseq-driver-35` hit a meta-blocker: `RegseqModel` does not carry
every family `AnalysisInput` exposes today (callback-impl sets,
wrapper/poll/log/timed-delay/duration-ctor/PM-gate call sites, guard
scopes, lock/completion operations, property lints, the full
`SemanticModel` + `ItemStore`), and extending it is a Phase-6 non-goal.

The right conclusion is the one the user arrived at: **the old models
were declared insufficient for generating correct drivers. We cannot
project `RegseqModel` back to the shape that was the problem.**
Consumers of `RegseqModel` must be rethought, not mapped.

This document answers the six questions that drive the redesign, using
the evidence of the current codebase plus the target architecture
defined by `spec.md`, `ir-reference.md`, `compiler-knowledge.md`, and
`execution-plan.md`. Two independent analyses were produced in parallel
(one via Codex, one via an Explore agent) and reconciled here;
divergences are called out.

---

## Q0 -- Context: what was insufficient about the old models?

The old architecture split one compiler contract into lossy
projections. Evidence from the authoritative specs:

- **Three separate models require projections between them**:
  `DeclarationModel` (declaration-side facts), `BindingModel`
  (platform bindings), `JoinedArtifact` (reviewer-facing recombination)
  -- each a fresh slice of the same data, each with its own invariants
  and a re-projection cost. `spec.md:68-91` labels this the "earlier
  evolutionary stage." The `JoinedArtifact` even materializes
  per-family `joined/decl_only/binding_only` lists
  (`crates/regseq-model/src/joined/artifact.rs:21-78`), which only exist
  because the split forced the information to be re-reconciled.
- **Expression-tree semantic IR (`Stmt`/`Expr` trees in
  `SemanticModel`) cannot represent complex control flow**
  (`crates/regseq-model/src/semantic/model.rs:1-29`). Loops,
  `?`-propagation, error paths, and ordered-region boundaries cannot
  be encoded cleanly as tree nodes. This is exactly why
  `execution-plan.md:3-6` says the old stack *"make\[s\] it difficult
  to produce correct driver code."*
- **String-based cross-references**: `CanonicalPath(pub String)` is
  the cross-layer key (`crates/regseq-model/src/item/identity.rs:1-58`).
  Every cross-reference is a string lookup; renames silently break;
  typed verification is impossible.
- **Domain-specific ad-hoc types** (`RegisterBlock`, `ResourceHandle`,
  etc.) rather than a unified `RegseqType` system.
- **Backend layers exist solely to bridge fragments**. `mir_interpreter/`
  walks MIR to produce a `BehaviorPlan`; `semantic_lowering/` walks
  MIR again to populate `SemanticBody` trees; `behavior_planner` and
  `body_interpreter` translate captured bodies into yet another
  shape. Each layer carries its own definitions of "what a call
  looks like" and re-derives facts that were already true upstream.

`RegseqModel` replaces all of this with a single progressively-filled
IR: CFG-based `Body` with `BasicBlock`s / `Terminator`s / typed
`IntrinsicCall` metadata (`ir-reference.md:1032-1065`), typed flat
`DeclId`s, a unified `RegseqType` system, and structural fidelity to
the spec's module tree. The point of the refactor is that downstream
passes read *directly* from this one structure rather than from a
cascade of lossy projections.

**The critical implication for Phase 7**: any migration that tries to
preserve the old model shapes -- via `compat.rs` adapters, via
`AnalysisInput` signatures, via `SemanticModel` projections -- is
working against the very reason `RegseqModel` exists. Consumer code
that was structured around the old shape must be *replaced*, not
bridged.

---

## Q1 -- Current consumers

The consumers that cross crate boundaries (true architectural
consumers):

| # | Consumer | Path | Primary entry points |
|---|----------|------|----------------------|
| 1 | `regseq-driver::emit_driver` | `crates/regseq-driver/src/emit_driver.rs` | `emit_driver` (`:69`), `EmitDriverResult` (`:27`), `EmitDriverError` (`:34`) |
| 2 | `regseq-analysis::input` | `crates/regseq-analysis/src/input.rs` | `AnalysisInput::new` (`:42`); accessors `declaration/joined/mir_bodies/property_lints/mir_reachable_helper_names` (`:55,60,434,291,473`) |
| 3 | `regseq-analysis::readiness` | `readiness.rs` | `check_readiness` (`:121`), `ReadinessReport` (`:69`) |
| 4 | `regseq-analysis::body_readiness` | `body_readiness.rs` | `check_body_readiness` (`:118`), `BodyReadinessReport` (`:64`) |
| 5 | `lowering::planner` | `crates/regseq-backends/src/lowering/planner.rs` | `build_driver_lowering_plan*` (`:79,126,152`) |
| 6 | `lowering::plan` (data) | `lowering/plan.rs` | `DriverLoweringPlan` (`:31-73`) |
| 7 | `linux::planner` | `linux/planner.rs` | `build_linux_backend_plan` (`:37`) |
| 8 | `linux::plan` (data) | `linux/plan.rs` | `LinuxBackendPlan` (`:23`) |
| 9 | `linux::emitter` | `linux/emitter.rs` | `emit_linux_artifacts` (`:72`) |
| 10 | E2E tests | `tests/e2e/tests/*.rs` | (9 files) |

**Internal submodules of lowering/linux that are not independent
consumer boundaries** (they are implementation of #5-#9):

- `lowering/{behavior_planner, body_interpreter, mir_interpreter/,
  hierarchy, projection, semantic_lowering/}`
- `linux/{emit_module, emit_behavior, emit_semantic, emit_regs, c_ast}`

These submodules are treated as implementation detail in the
classification below. They live or die with the crate-boundary
consumer they implement.

**Pre-existing drift from the Phase-7 brief**: `linux/emit_semantic.rs`
already contains a rewrite of body emission that "Replaces the legacy
`emit_behavior.rs`" (`crates/regseq-backends/src/linux/emit_semantic.rs:1-8`),
and `linux/emit_module.rs:330-520` routes helper / ISR / UART emission
through semantic bodies. The Phase-7 brief's claim that "Linux emission
stays untouched" (`.harness/spec-batches/regseqmodel-phase-7.md:24-29`)
is already false in the tree. This matters for the migration plan
because some "Phase-8" rewrite work is already partially done and must
be accounted for.

---

## Q2 -- Old-pipeline responsibilities

Condensed (one sentence per consumer; full evidence in Codex analysis):

- **`emit_driver`**: orchestrate the old hot path (raw JSON →
  `normalize_from_json` → `binding::normalize` → `perform_join` →
  `AnalysisInput` → lowering → Linux plan → Linux emission).
- **`AnalysisInput`**: pair `DeclarationModel` + `JoinedArtifact` +
  `Option<BusBinding>` and expose cross-model queries for downstream
  passes.
- **`readiness`**: reviewer-facing validation of declaration + join
  completeness, callback integrity, property lints, optionality, bus
  consistency.
- **`body_readiness`**: validate the MIR capture subset -- query
  flavor, callback/helper coverage, unsupported MIR nodes,
  cleanup/drop visibility, `?`-pattern visibility.
- **`lowering/planner`**: translate `AnalysisInput` into
  `DriverLoweringPlan`; project `SemanticModel` from `ItemHierarchy`
  along the canonical path.
- **`behavior_planner` / `body_interpreter` / `mir_interpreter`**:
  translate captured bodies into `BehaviorPlan` (three overlapping
  implementations; MIR is canonical).
- **`hierarchy` / `semantic_lowering` / `projection`**: build
  `ItemHierarchy` from enriched extraction, lower MIR CFG into
  `SemanticBody` trees, then project a backend-facing `SemanticModel`.
- **`linux/planner`**: map portable obligations to Linux-specific
  realization decisions (registration shape, probe/remove, serial-core).
- **`linux/emitter`**: write artifact files from the Linux plan.
- **`linux/emit_{module,behavior,semantic,regs}` + `c_ast`**: the
  C-generation implementation stack. `emit_behavior` is legacy
  `BehaviorPlan`-based; `emit_semantic` is the CFG-closer rewrite
  already landed in the tree.

---

## Q3 -- Survival classification

This is the key output of this document. The matrix below is the
reconciled result of the two parallel analyses; where they diverged,
the choice is justified.

| Consumer | Status | Rationale |
|----------|--------|-----------|
| `emit_driver` | **SURVIVES** | Small orchestration with a narrow signature change: call `normalize()` + `populate_bindings()` + the new lowering entry, stop constructing the three legacy artifacts. The logic is ~50 lines; calling this a "rewrite" overstates the work. (Codex: SURVIVES; Explore: REWRITE. Codex wins -- the scope is too small to warrant "rewrite".) |
| `regseq-analysis::input` (`AnalysisInput`) | **DELETED** | The type exists *only* to pair `DeclarationModel` + `JoinedArtifact` + `Option<BusBinding>`. With a unified `RegseqModel` the pairing concept is unnecessary. Consumers pass `&RegseqModel` directly. (Codex: DELETED; Explore: SHRINKS. Codex wins -- shrinking would preserve a dead pairing.) |
| `regseq-analysis::readiness` | **SHRINKS** | Phase-legality checks (required binding present, property value present, callback coverage) already belong to `verify_phase_1/2`. What remains is *reviewer-facing diagnostic reporting* (callback family inventory, property-lint visibility) -- a smaller module focused on diagnostics, not validation. |
| `regseq-analysis::body_readiness` | **DELETED** | Every check it performs ("is this MIR body supported?", "does the capture cover all callbacks?", "are `?`-patterns visible?") either (a) is enforceable by `normalize()` success / IR construction in Phase 5, or (b) is a Phase-5 invariant enforced by the IR type system itself. Its existence reflects the old layering; after Phase 6 the checks are structural. |
| `lowering::planner` | **REWRITE** | The portable plan is still the right contract (backends consume `DriverLoweringPlan`), but the planner's internal logic -- `project_semantic_model`, `ItemHierarchy` traversal, `BehaviorPlan` composition -- is entirely replaced by direct traversal of `RegseqModel`'s module tree + CFG bodies. The signature becomes `fn build_driver_lowering_plan_from_model(&RegseqModel) -> DriverLoweringPlan`. |
| `lowering::behavior_planner` | **DELETED** | `execution-plan.md:393` explicitly removes it. |
| `lowering::body_interpreter` | **DELETED** | `execution-plan.md:393` explicitly removes it. |
| `lowering::mir_interpreter/` | **DELETED** | `execution-plan.md:393` explicitly removes it. |
| `lowering::hierarchy` | **DELETED** | `ItemHierarchy` is replaced by `RegseqModel::root_module` (see `spec.md:129-151`). Keeping it would preserve a redundant projection. (Codex: DELETED; Explore: SURVIVES. Codex wins -- the module tree IS the hierarchy.) |
| `lowering::projection` | **DELETED** | `SemanticModel` no longer exists as a backend-facing projection; bodies are read from `RegseqModel::Function.body` directly. (Codex: DELETED; Explore: SURVIVES. Codex wins.) |
| `lowering::semantic_lowering/` | **DELETED** | `execution-plan.md:393` explicitly removes it. Its job (MIR → `SemanticBody`) is replaced by Phase-5 IR construction producing CFG `Body` directly. |
| `lowering::plan` (data) | **SURVIVES (shrunk)** | `DriverLoweringPlan` is the portable backend contract. The `legacy_behavior: BehaviorPlan` field is removed with `behavior_planner`; the `SemanticModel` field is removed or replaced with the direct CFG handle. |
| `linux::planner` | **SURVIVES** | Consumes `DriverLoweringPlan`, produces `LinuxBackendPlan`. Contract preserved. |
| `linux::plan` (data) | **SURVIVES** | Pure data container. |
| `linux::emitter` | **SURVIVES** | File-writing + manifest tracking, no old-model coupling. |
| `linux::emit_module` | **REWRITE** | Walks `SemanticModel`/`SemanticBody` today; must walk CFG `Body` instead. Execution-plan Phase 8.1 ("goto-based C emitter from CFG IR") owns this. |
| `linux::emit_behavior` | **DELETED** | Legacy `BehaviorPlan` emitter; already superseded by `emit_semantic` in the tree. |
| `linux::emit_semantic` | **REWRITE** | Already partially written as a semantic-body walker, but walks `Stmt`/`Expr` trees. Must be retargeted to CFG `Body` with `BasicBlock`/`Terminator` traversal. |
| `linux::emit_regs` | **SURVIVES** | Register-helper emission is driven by `DriverLoweringPlan` register data, not by legacy models. |
| `linux::c_ast` | **SURVIVES** | Pure C-syntax builder, backend-neutral. |
| E2E tests | **SURVIVE (assertions rewritten)** | The nine tests stay, but specific assertions that lock in `DeclarationModel`/`JoinedArtifact`/`SemanticModel` surfaces are rewritten or retired. |

### Divergences between the two parallel analyses (and resolution)

| Consumer | Codex | Explore | Resolution |
|----------|-------|---------|------------|
| `emit_driver` | SURVIVES | REWRITE | **SURVIVES** (Codex) -- scope is a narrow orchestration swap. |
| `AnalysisInput` | DELETED | SHRINKS | **DELETED** (Codex) -- the pairing is the problem. |
| `body_readiness` | DELETED | SHRINKS | **DELETED** (Codex) -- checks are structural invariants. |
| `lowering/planner` | REWRITE | SURVIVES | **REWRITE** (Codex) -- internals are rebuilt top-down. |
| `hierarchy` | DELETED | SURVIVES | **DELETED** (Codex) -- `RegseqModel::root_module` IS the hierarchy. |
| `projection` | DELETED | SURVIVES | **DELETED** (Codex) -- no `SemanticModel` target to project to. |

Codex's call is right on every divergence. The Explore agent's classifications leaned toward preserving concepts that the execution plan already marks for deletion.

---

## Q4 -- For each surviving consumer: needs, gaps, output, delta

### `regseq-driver::emit_driver` (SURVIVES)

- **Needs from `RegseqModel`**: the fully populated model produced by
  `normalize()` + `populate_bindings()`. Nothing else.
- **Gaps**: none. The Phase-6 API is sufficient.
- **Output**: `EmitDriverResult` / `EmittedArtifactSet` (unchanged).
- **Delta**: replace the legacy three-step pipeline with
  `normalize(raw) + populate_bindings(&mut model, platform) +
  build_driver_lowering_plan_from_model(&model) +
  build_linux_backend_plan + emit_linux_artifacts`. No compat adapter.

### `regseq-analysis::readiness` (SHRINKS)

- **Needs**: read-only access to `RegseqModel` for diagnostic
  reporting -- callback family inventory, property-lint visibility,
  resource field names.
- **Gaps**: property lints are NOT currently modeled on `RegseqModel`
  (`crates/regseq-model/src/model/{driver,module}.rs`). Old
  `DeclarationModel::property_lints` is not carried forward. **Design
  decision needed**: compute lints on-demand from property field
  shape/defaults, OR fold them into `verify_phase_1` as structured
  findings. Recommendation: fold into `verify_phase_1` with a
  `LintFinding` vocabulary -- keeps the diagnostic paired with the
  rule that produced it and surfaces in one place.
- **Output**: `ReadinessReport` (shrunk -- only reviewer-oriented
  facts, not phase-legality failures).
- **Delta**: delete every check that's now a Phase-6 verifier rule;
  keep the inventory and rendering.

### `lowering::planner` (REWRITE)

- **Needs**: structural inputs for identity, resources, registers,
  sync, platform metadata, functions, CFG bodies. `RegseqModel`
  provides all of these:
  - `model.root_module` tree with submodules, driver module, type
    defs (`model/module.rs:43-88`).
  - Driver struct with resource slots and bindings
    (`model/driver.rs:21-70`, `model/resources.rs:30-279`).
  - Register descriptors + blocks (`model/register.rs:15-117`).
  - Functions with `Body` (CFG) (`model/function.rs:18-40`,
    `ir/body.rs:17-40`).
- **Gaps**: the old `AnalysisInput` exposed pre-computed call-site
  families (wrapper call sites, poll call sites, log call sites,
  timed-delay sites, PM gates, MIR-helper transitive closure) that
  are NOT carried on `RegseqModel` or its module tree. Recommendation:
  **compute on-demand** by traversing function bodies and matching on
  `IntrinsicCall` terminator kinds + their `IntrinsicMetadata`
  payloads. These are pure derived facts; materializing them on the
  model adds storage without information. Add a small `query/` helper
  module under `regseq-model/src/` so the traversals are reusable and
  deterministic.
- **Output**: `DriverLoweringPlan` -- same shape minus
  `legacy_behavior` and the `SemanticModel` reference.
- **Delta**: entry point becomes
  `build_driver_lowering_plan_from_model(&RegseqModel,
  &LoweringConfig) -> Result<DriverLoweringPlan, LoweringError>`. The
  internal machinery (`project_semantic_model`, item hierarchy walk,
  behavior plan composition, MIR interpretation) is deleted entirely.

### `lowering::plan` (SURVIVES, shrunk)

- **Needs**: nothing structural; it's a data container.
- **Gaps**: none.
- **Output**: `DriverLoweringPlan` consumed by `linux/planner`.
- **Delta**: delete `legacy_behavior` field and the `SemanticModel`
  field. Any fields currently typed `regseq_model::semantic::*`
  become `regseq_model::ir::*` / `regseq_model::model::*` types.

### `linux::planner` / `linux::plan` / `linux::emitter` / `linux::emit_regs` / `linux::c_ast` (SURVIVES)

- **Needs**: `DriverLoweringPlan` (portable contract). No change.
- **Gaps**: none.
- **Output**: `LinuxBackendPlan` (planner), generated C files
  (emitter + regs + c_ast).
- **Delta**: minor import changes if plan field types move; no logic
  changes.

### `linux::emit_module` and `linux::emit_semantic` (REWRITE)

- **Needs**: CFG `Body` traversal access (`ir::body::Body`,
  `BasicBlock`, `Statement`, `Terminator`, `IntrinsicCall` metadata).
- **Gaps**: the execution plan's Phase-8 goto-based C emitter and
  dominator-based resource cleanup
  (`execution-plan.md:375-389`) are specifications, not code. The
  current `emit_semantic.rs` walks `Stmt`/`Expr` trees and must be
  retargeted. This is real Phase-8 work that the Phase-7 brief
  wrongly classified as out-of-scope; with `emit_semantic.rs` already
  in the tree as the current path, it is already partly on the Phase-7
  side of the boundary.
- **Output**: C source for helper functions, ISR handlers, UART ops,
  driver integration.
- **Delta**: swap `Stmt`/`Expr` recursion for CFG block-by-block
  emission with goto-based control-flow lowering.

### E2E tests (SURVIVE; assertions rewritten)

- **Needs**: RegseqModel-level or generated-C-level assertions.
- **Gaps**: some current assertions (e.g.,
  `pl011_lowering_input_stability.rs` asserts against `AnalysisInput`
  accessor byte-stability) lock in the old shape; they cannot be
  preserved.
- **Delta**: see Q5 for the two assertions that MUST exist; most
  others either already pass (binary-level tests like
  `pl011_linux_module_host_build_proof.rs`) or get their assertions
  shifted to RegseqModel-level invariants.

---

## Q5 -- Validation strategy

> **Rewritten.** The original formulation named "PL011 C byte-parity
> old vs new" as the strongest validation. That is wrong: the old
> compiler pipeline's C output is neither complete nor correct, so
> byte-parity against it would lock in bugs. Mature compilers
> (LLVM/MLIR/Cranelift/rustc) do not validate new backends by
> blessing legacy text; they specify lowering, assert structural
> legality, and run programs. The revised strategy (**D'**) follows
> that pattern.

The migration is correct if a **layered stack of signals** all hold
on the PL011 fixture. No single oracle; each layer catches a
different class of defect.

### Layer 1 -- Emission contract (the specification)

A normative markdown document `linux-emission-contract.md` plus a
small Rust data-table module (`emission_tables.rs`) that together
form an executable specification of "for each RegseqModel construct,
what C pattern should emit."

Markdown surface:
- The C normal form (one C function per `RegseqModel::Function`, one
  C label per `BasicBlock`, canonical goto graph, canonical cleanup
  labels, no inlining, canonical locals). Phase-8.1 goto-based
  emission is the design target
  (`execution-plan.md:375-389`); the normal form codifies it.
- Naming policy (symbol prefixes, label shapes, variable conventions).
- Normalization algorithm used to produce the blessed reference file
  (whitespace, ordering, comment policy). Downstream byte-equal
  regression is only well-defined after the normalization algorithm
  is specified here.

Rust data tables (naturally tabular material):
- Intrinsic -> Linux API mapping (`clock_enable` ->
  `clk_prepare_enable`, etc.; a small amount of this already exists
  in prose in `compiler-knowledge.md:470-811`).
- Reversible-intrinsic cleanup pairs.
- Callback-family return-value adaptation.
- Ordered-region barrier placement.

### Layer 2 -- Construct exemplars

~15 hand-authored C fragments under
`tests/goldens/pl011/linux/constructs/*.c`, each tied to exactly one
CFG construct or emission rule: straight-line sequence, branch
(`SwitchInt`), loop (back-edge Goto), `IntrinsicCall` with
`error_target`, `OrderedRegion`, cleanup ladder, etc. These are not
human-stylistic Linux C; they are the mechanical output the contract
requires.

(Callback wrapper / parameter binding / return adaptation fragments
are Linux module-integration policy, not CFG-level, and belong to
the `emit_module` task's exemplar set, not the contract-level
exemplars.)

### Layer 3 -- Construct-level tests

For each exemplar, a unit test that constructs a minimal CFG body
(directly, via the IR builder) and asserts the emitter produces the
matching exemplar bytes. This forces the emitter's lowering rules to
match the contract at a construct granularity rather than only at
whole-driver granularity. These tests are authored by the task that
writes the emitter (`emit_semantic` / `emit_module`), not by the
contract-authoring task.

### Layer 4 -- Hand-authored PL011 normalized reference (authoring-first)

Commit
`tests/fixtures/pl011-full-spec/expected/pl011-reference-linux-driver.c`
as a **hand-authored** PL011 Linux driver written in the emission
contract's normal form (one C function per `RegseqModel::Function`,
one label per `BasicBlock`, canonical gotos, canonical cleanup
ladders, no inlining). Authored jointly (user + Claude) as part of
task `regseq-backends-02`, and validated via host-kernel build
proof + QEMU functional smoke BEFORE any implementer begins work
on the emitter.

This reference is the ground-truth oracle the emitter must
reproduce. Byte-equal regression against this file on every
subsequent pipeline run catches composition-level emission
regressions that the fragment-level construct tests can't see.

The authoring-first ordering (reference before emitter, not emitter
before reference) is the compiler-design practice followed by
LLVM's handwritten CodeGen tests, MLIR lit tests, and rustc's
codegen-test corpus: specify + validate the target output first,
then build the producer.

### Layer 5 -- Host-kernel build proof

The existing `pl011_linux_module_host_build_proof.rs` infrastructure
builds the generated `.ko` against a real pinned kernel tree and
verifies `modinfo`. This validates C well-formedness + ABI
compatibility, not semantic correctness. Retained as a secondary
signal.

### Layer 6 -- Determinism golden

Two consecutive runs of the new pipeline on the PL011 fixture
produce byte-identical output. Guards against nondeterminism
introduced during the emitter rewrite. Task 60's existing Phase-6
determinism golden covers the model layer; this layer extends it
through emission.

### Layer 7 -- QEMU functional smoke (the critical falsification)

An ARM64 QEMU `virt` boot harness built on the already-frozen
PL011 eval-env baseline (pinned `v6.18.21` kernel, Buildroot
`2026.02` + BusyBox + `kmod` rootfs, second AMBA PL011 reserved via
DT / periphid override; see `experiments/pl011-eval-env/notes/`).

The smoke does **not** validate against `/dev/ttyAMA0` (the stock
console, owned by Linux's built-in PL011 driver); it targets the
backend-generated `dev_name` from `LinuxSerialCoreRealization`
(`crates/regseq-backends/src/linux/planner.rs:101-104,126-137`).

Pass criterion:
1. Boot the pinned QEMU image with the generated `.ko` available.
2. `insmod` the module.
3. Probe succeeds; no kernel oops/warnings in dmesg.
4. Open the backend-defined tty device.
5. Write at least one byte through the generated driver's
   `tx_byte` / `startup` / `port_open` path.
6. Close; `rmmod`; no leaks / warnings.

This is the first point in the pipeline where register offsets,
callback signatures, ordered-region barriers, and Linux API usage
get *executed*, not merely *emitted*. It is the only layer that can
falsify a self-consistent-but-wrong contract.

### Layer 8 -- CFG structural invariants (Test B)

Positive assertions that facts the old models *could not express*
are present in the new model:

- `OrderedRegion { entry, exit }` metadata exists on every function
  body that contains an `ordered!` region
  (`ir-reference.md:1737-1764`, `spec.md:915-929`).
- Every fallible `IntrinsicCall` carries an `error_target` pointing
  to the `?`-propagation block (not a reconstructed SwitchInt
  pattern) (`ir-reference.md:1032-1065`).
- `verify_phase_2(&model)` rejects platform files missing any
  required binding slot.
- The derived obligations a redesigned `lowering::planner` computes
  from the model (acquisition order, resource policies, callback
  inventory) are shape-stable across two runs on the same input.

### How the layers compose

| Defect class | Caught by |
|--------------|-----------|
| Contract is wrong | Layer 7 (QEMU smoke executes the wrong C) |
| Emitter deviates from contract | Layer 3 (construct tests) |
| PL011-specific emitter bug | Layer 4 (manual review before blessing) + Layer 7 |
| Subsequent emission regression | Layer 4 (byte-equal) + Layer 5 (build) + Layer 7 (ongoing smoke) |
| Nondeterminism | Layer 6 |
| Structural IR invariant violation | Layer 8 |
| Semantic Linux-API misuse shared by contract + reference | Layer 7 (falsification) |

A defect class that no layer catches is a known gap -- notably,
concurrency/interrupt-ordering bugs and hardware-functional
mistakes the QEMU smoke does not exercise. These are deferred to a
follow-on eval track (QEMU behavioral suite + hardware-in-the-loop
where applicable).

### Existing-test disposition

- `pl011_driver_emission.rs` (on-disk artifacts): unchanged.
- `pl011_linux_module_host_build_proof.rs` (host build): continues
  at Layer 5. Its target shifts from old-emitter output to the
  blessed reference after Phase-7C-validation.
- `pl011_frontend_qc.rs`, `pl011_layer_boundary.rs`,
  `platform_file_boundary.rs`: unchanged (pre-normalize).
- `pl011_bus_identity_proof.rs`: rewritten to assert bus identity
  on `RegseqModel.root_module.driver_module().driver.identity.bus`.
- `pl011_lowering_input_stability.rs`: retired -- its stability
  assertions over `AnalysisInput` accessors target a shape that is
  being deleted.
- `driver_e2e.rs`, `pl011_spec_slice.rs`: unchanged (RUSTC_WRAPPER
  smoke).
- **New**: blessing harness + ongoing QEMU regression + construct
  tests + byte-equal regression.

---

## Q6 -- Recommended migration order

Codex identified the crucial reversal: **`emit_driver` is the LAST
cut, not the first.** The blocked task had it backwards. With that
correction:

### Phase 7A -- Analysis rewrite (foundational)

1. Build the `regseq-model::query` helper module: deterministic CFG
   traversals that answer "all wrapper call sites", "all poll call
   sites", "all log call sites", etc., from a `&RegseqModel`. This
   is the replacement for `AnalysisInput`'s pre-computed families.
2. Rewrite `regseq-analysis::readiness` and delete
   `regseq-analysis::body_readiness` + `regseq-analysis::input`.
   The surviving `readiness` module takes `&RegseqModel` and emits
   reviewer-facing diagnostics only.
3. Fold property lints into `verify_phase_1` as a `LintFinding`
   vocabulary (or document the on-demand computation choice).

**Cut point**: `cargo test -p regseq-analysis` passes against
`RegseqModel`; production pipeline unchanged; old `AnalysisInput`
still exists but is unreferenced by analysis.

### Phase 7B -- New lowering entry point

4. Add `build_driver_lowering_plan_from_model(&RegseqModel,
   &LoweringConfig) -> Result<DriverLoweringPlan, LoweringError>`
   alongside the existing entry. Internally uses the `query/`
   helpers plus direct module-tree walks. Produces the same plan
   shape today (minus `legacy_behavior`) so downstream backend code
   continues compiling.
5. Remove `DriverLoweringPlan::legacy_behavior` and
   `DriverLoweringPlan::semantic_model` fields. Update the one
   Linux emission site that reads them.

**Cut point**: Linux planner + emitter compile unchanged against
the new entry; legacy lowering entry still exists but unused.

### Phase 7C -- Linux body emission over CFG (actual architectural cut)

6. Retarget `linux::emit_semantic` from `Stmt`/`Expr` tree walks
   to `BasicBlock`/`Terminator` traversal with goto-based control
   flow (Phase-8.1 per `execution-plan.md`). `emit_module` follows
   suit.
7. Delete `linux::emit_behavior` (already superseded).
8. Delete `lowering::{behavior_planner, body_interpreter,
   mir_interpreter, hierarchy, projection, semantic_lowering}` per
   `execution-plan.md:393`. These are no longer reachable.

**Cut point**: PL011 backend unit tests green; PL011 byte-parity
test (Test A) passes; PL011 CFG-invariant test (Test B) passes.

This is where the execution-plan's Phase 7 and Phase 8 boundaries
blur. Given `emit_semantic` already landed as a post-`emit_behavior`
stopgap, the Phase-7 redesign should own the full CFG emitter
retarget rather than deferring it.

### Phase 7D -- Pipeline cutover

9. Cut `emit_driver` over: call `normalize()` +
   `populate_bindings()` + `build_driver_lowering_plan_from_model()`
   + the rest. Delete the legacy three-step pipeline construction
   code.
10. Delete the legacy lowering entry point (step 4's "alongside").

**Cut point**: production path is `normalize() → populate_bindings()
→ consumers`. Old pipeline calls are gone from `emit_driver`.

### Phase 7E -- E2E retarget

11. Rewrite the two legacy-bootstrap tests
    (`pl011_bus_identity_proof.rs`,
    `pl011_lowering_input_stability.rs`) against
    `RegseqModel`-level facts.
12. Add Test A and Test B.
13. Verify the other seven E2E tests pass unchanged.
14. Flip the tracker: `regseqmodel-phase-7` → `tasked` (it was
    prematurely flipped when the original specs landed; a redesign
    warrants the same flip at the new decomposition), and promote
    `regseqmodel-phase-8` → `ready_to_elaborate` only after
    acknowledging that Phase-7C already absorbed much of what
    Phase 8 was scoped for.

**Cut point**: `cargo test --workspace` green; Test A and Test B
green; `compat.rs` was never created because it was the wrong
pattern.

---

## Open decisions

Before executing this plan, the following need explicit answers:

1. **Property lints**: on-demand computation vs
   `verify_phase_1::LintFinding`? Recommendation above is the
   latter; confirm.
2. **Redundant work with Phase 8**: `execution-plan.md` scopes the
   goto-based C emitter and dominator-based cleanup to Phase 8, but
   the tree already has a partial semantic emitter that this plan
   retargets in Phase 7C. Do we redraw the Phase-7 / Phase-8
   boundary to put the CFG emitter retarget into Phase 7, leaving
   Phase 8 for dominator-based cleanup + structured-recovery
   follow-up? Recommended: yes.
3. **Old-module deletion timing**: `declaration/`, `binding/`,
   `joined/`, `semantic/`, `item/` are scoped for Phase-9 deletion.
   Can we delete them earlier -- at the end of Phase 7D -- since
   nothing reads them after `emit_driver` cuts over? Or do their
   parallel pipelines still need to exist for the byte-parity test
   (Test A)? Recommendation: keep them through Test A, delete in
   Phase-7E final step.
4. **`compat.rs` scope**: does it exist at all in this redesign? A
   small version might still be useful if any E2E test needs a view
   over `RegseqModel` that matches one legacy accessor shape for
   diff'ing purposes. Recommendation: no -- expose the query helpers
   directly; delete the `regseq-model-61` spec.
5. **Already-authored Phase-7 specs**: six specs are committed
   (`regseq-model-61`, `regseq-analysis-06`, `regseq-lowering-10`,
   `regseq-linux-backend-03`, `regseq-driver-35`,
   `track-a-validation-11`). None match this redesign. Retire them
   and write a new six-task decomposition against Phase 7A-E?
   Recommendation: yes. The old specs and the brief should be moved
   to `.harness/archive/` with a note pointing at this document.

---

## Changes this triggers in the authoritative specs

Minimal; the target architecture in `spec.md` / `ir-reference.md` /
`compiler-knowledge.md` is unchanged. The redesign only affects:

- `.harness/spec-batches/regseqmodel-phase-7.md` -- rewrite against
  this document's Q6 plan.
- `docs/discussions/models/regseq-model/execution-plan.md` Phase 7
  narrative -- refresh to reflect that Phase 7C absorbs part of
  Phase 8.1.
- `.harness/task-spec-status.yaml` -- `regseqmodel-phase-7` moves
  back to `in_elaboration`; `regseqmodel-phase-8` moves back to
  `not_ready` until the boundary with 7C is resolved.
- `.harness/tasks.json` -- the six Phase-7 task entries currently
  marked pending should be replaced by a new set once the
  redesign's decomposition is approved.

---

## Q7 -- Justification of the `DriverLoweringPlan` and `LinuxBackendPlan` intermediate IRs

The proposed pipeline is four stages:

    Source spec
        -> RegseqModel               (Phase-1/2 compiler-owned IR)
        -> DriverLoweringPlan        (backend-neutral obligations)
        -> LinuxBackendPlan          (Linux-specific realization)
        -> C artifacts

Two independent analyses (one via Codex, one via direct code read)
agree on the following.

### What each IR carries today

**`DriverLoweringPlan`** (`crates/regseq-backends/src/lowering/plan.rs:32-73`)
is a mixed bag:

- *Portable obligations* (justified as an IR boundary): driver
  identity + provenance, callback inventory per family, resource
  obligations by kind with policies, ordered
  acquisition/teardown steps, device-class role obligations,
  register obligations including computed per-field byte offsets,
  synchronization obligations (lock fields / scopes / completion
  uses). Lines 34-48 and 79-481 enumerate these.
- *Explicitly transitional / redundant fields*: `behavior:
  SemanticModel` (the projected old-IR behavioral source of truth,
  lines 56-66), `legacy_behavior: BehaviorPlan` (marked "transitional"
  and "Do not add new consumers", lines 67-72),
  `callback_actions: Vec<CallbackBodyPlan>` (explicitly documented
  as *not* the backend-consumed source of truth, lines 49-55).

**`LinuxBackendPlan`** (`crates/regseq-backends/src/linux/plan.rs:23-45`)
is genuinely Linux-shaped: `platform_driver` / `amba_driver`
registration naming + AMBA IDs; `uart_driver` / `uart_ops` serial-core
realization; explicit `PORT_*` value allocation; ordered
probe/remove step lists with Linux-specific step kinds (`MapMmio` via
`devm_ioremap_resource`, `RequestIrq` via `devm_request_irq`,
`GetClock` via `devm_clk_get + clk_prepare_enable`, etc., lines
122-163); emitted-file layout decisions; Linux property read APIs
(`device_property_read_*`); Linux behavior mapping (portable
functions into probe/remove/uart_ops/ISR/helper buckets).

### Established-compiler-design framing

The closest analogy is **MLIR progressive lowering / dialect
conversion**, not LLVM IR -> Machine IR. The comparison table:

| Stage | Regseq | LLVM | GCC | MLIR |
|-------|--------|------|-----|------|
| High-level IR | `RegseqModel` (CFG + typed `DeclId` + `RegseqType`) | LLVM IR | GIMPLE | high-level dialects |
| Portable lowered form | `DriverLoweringPlan` (obligations / decisions, not executable code) | Target-independent Machine IR passes | RTL legalization pre-target | intermediate dialects (linalg, memref) |
| Target-specific form | `LinuxBackendPlan` (Linux policy) | Target-specific Machine IR | target RTL | target dialects (llvm, spirv) |
| Output | C source | assembly | assembly | bytecode/LLVM IR |

The Regseq pipeline is structurally correct: two progressive
lowering boundaries between source model and target emission. But
the *representation level* is closer to dialect-conversion
*decision records* than to executable lower-level IR. LLVM/GCC lower
into progressively-lower executable programs; Regseq lowers into
progressively-more-target-committed **plans** that reference
behavioral IR rather than replacing it. That is a legitimate design
choice and matches how higher-level DSL compilers (e.g., MLIR's
linalg dialect, TorchScript's graph lowering) often work.

### What is justified vs what is vestigial

**Keep the two-layer split.** The existing design doc
`docs/discussions/linux-codegen/lowering-plan-interface-boundary.md`
(lines 21-47, 133-180) already argues that the "first OS-shaped plan
must not be the global lowering contract" -- collapsing
`RegseqModel -> LinuxBackendPlan` directly would either (a) force
Zephyr and future backends to re-derive every portable obligation
from the raw model, or (b) promote Linux decisions into the model.
Both are wrong.

**Trim `DriverLoweringPlan` to a pure portable-obligations contract.**
Delete three fields:

1. `behavior: SemanticModel` -- the Phase-7 redesign replaces the
   `SemanticModel` projection entirely. Backends walk `RegseqModel`
   function bodies via `regseq-model::query` helpers or a
   function-by-function accessor on the plan (e.g., `plan.function_bodies:
   &BTreeMap<FunctionId, &Body>` borrowing from the model).
2. `legacy_behavior: BehaviorPlan` -- already documented as
   transitional. Deleted when `behavior_planner`/`body_interpreter`
   are deleted.
3. `callback_actions: Vec<CallbackBodyPlan>` + the `LoweringAction`
   enum -- already non-canonical per its own rustdoc; duplicates
   CFG information already in RegseqModel's IR. If a backend wants
   "what does this callback do?", it walks the callback's CFG
   body; it doesn't need a pre-flattened action list.

What should remain:
- Driver identity, callback inventory, resource obligations +
  policies, ordered acquisition/teardown, device-class roles,
  register obligations (including computed layout), sync
  obligations, provenance. These *are* information the backend
  should not re-derive.

**Fix `LinuxBackendPlan`'s leaky boundary.** Today,
`emit_linux_artifacts` takes BOTH plans (`linux/emitter.rs:72`:
`emit_linux_artifacts(plan: &LinuxBackendPlan, portable:
&DriverLoweringPlan, ...)`), and `emit_module_integration`
likewise consumes both (`linux/emit_module.rs:34-115`). The
emitter reads interrupt classes and helper bodies from
`portable.behavior`, resource facts from `portable.resources`,
then overlays Linux naming from `LinuxBackendPlan`. That means
`LinuxBackendPlan` is not a complete emission contract -- it is a
*Linux-policy overlay on top of the portable plan*. Two ways to
fix:

- **Option A (clean boundary)**: `LinuxBackendPlan` copies /
  owns every fact the emitter reads (function-body handles,
  register facts, resource facts). `emit_linux_artifacts` takes
  only `&LinuxBackendPlan`. Pro: testable and independent. Con:
  duplication of data during compilation.
- **Option B (scoped overlay, documented)**: explicitly accept
  that `LinuxBackendPlan` is an overlay and pass both references
  together as a `LinuxEmitInput { portable: &DriverLoweringPlan,
  linux: &LinuxBackendPlan }` bundle. Document the overlay
  relationship. Pro: no duplication. Con: `LinuxBackendPlan` is
  not a standalone IR -- it's a *companion* representation.

Recommendation: **Option A**, but only materialize the refs /
handles the emitter actually reads (function IDs, not full
body copies). This keeps the Linux plan a real target IR while
avoiding full-body duplication. The portable body IR is still
borrowed from `RegseqModel` via stable `FunctionId` lookups.

**VERIFIED** (follow-up grep, not a hypothesis): `LinuxResourceSequencing`
and `LinuxDeploymentMetadata` are populated by the Linux planner
(`linux/planner.rs:44-48, 232-273, 437-472`) and asserted by unit
tests (`crates/regseq-backends/tests/lowering_boundary.rs:662-667`
and `crates/regseq-backends/tests/pl011_full_spec_lowering.rs:2218-2326`,
`crates/regseq-backends/tests/pl011_lowering_completeness_proof.rs:2172-2186`),
but **no emitter reads `.resource_acquisition` or `.deployment_metadata`**
-- `rg '\.resource_acquisition|\.deployment_metadata'` returns only
planner-populations and test-assertions. They are reporting/test
surfaces, not emission-critical IR. The redesign should either
relocate them into a diagnostics module or delete them; the
retained tests convert to asserting the equivalent fact on
`DriverLoweringPlan` (which already carries acquisition order and
deployment metadata) rather than on the Linux plan.

### Concrete invariants each IR should own

After the redesign:

- `RegseqModel`: single source of truth for source-spec semantics
  plus platform bindings. Immutable post-Phase-2.
- `DriverLoweringPlan`: derived from `RegseqModel` alone. Owns
  obligations + derived layouts + per-resource policies. Backend-
  neutral: no Linux API name, no Zephyr API name, no file-layout
  choice, no emission-shape policy. References into RegseqModel
  for bodies.
- `LinuxBackendPlan`: derived from `DriverLoweringPlan` alone.
  Owns Linux names, Linux API sequences, registration shape,
  serial-core policy, file partitioning, and Linux-specific
  callback signature adaptation. Self-sufficient for emission.
- C source: derived from `LinuxBackendPlan` plus borrowed
  function bodies via `FunctionId`.

---

## Q8 -- C AST: decoupling + completeness

### Current state: not actually a C AST

`crates/regseq-backends/src/linux/c_ast.rs` is a **typed
pretty-printing DSL for top-level translation-unit items**, not a
C AST. Node kinds present:

- `BlockComment`, `LineComment`, `BlankLine`
- `Include { path, system }`, `Define { name, value }`,
  `IncludeGuardOpen/Close`
- `StructDef` (structured fields with `type_name: String`)
- `StaticDef` (initializer as `Vec<String>`, not a designator
  AST)
- `FuncDef` (`signature: String`, `body: Vec<String>` -- the
  function body is *already rendered C source text*, not AST
  nodes)
- `MacroCall`, `Raw(String)` (the escape hatch)

The actual C language (statements, expressions, declarations,
declarators) is emitted by string formatters in `emit_module.rs`
and `emit_semantic.rs` -- not as AST nodes. The "AST" is the
file-scaffolding layer only.

### Decoupling: the move is easy

The current module is **not actually Linux-specific**. Only
`generated_file_banner` contains Linux kernel idioms (SPDX
`GPL-2.0` header, "Linux driver artifact" phrasing;
`c_ast.rs:232-241`). Everything else -- includes, defines,
include guards, struct definitions, function shells -- is plain
C and already reusable.

**Recommendation**: move `c_ast.rs` from `crates/regseq-backends/src/linux/`
to `crates/regseq-backends/src/c/` (new sibling module). Keep
`generated_file_banner` in `linux/` as Linux-specific banner
policy; parameterize or re-home it as Linux conventions. Do NOT
split into a separate `crates/regseq-c-ast/` workspace crate yet
-- the representation will need rapid iteration as Phase-7/8 CFG
emission lands; freezing a crate boundary too early traps the
design.

Boundary rule:
- `backends::c`: backend-neutral C syntax. Translation units,
  declarations, declarators, types, initializers/designators,
  expressions, statements, labels, `goto`.
- `linux/`: Linux policy -- `__iomem`, `__init`, `__exit`,
  `container_of`, `MODULE_*`, `platform_driver`/`amba_driver`,
  `uart_ops`, Linux API names, goto-chain cleanup idioms.
- `linux/` retains non-C artifacts (`Makefile`, `Kbuild`) per
  `linux/emitter.rs:205-245`.

Zephyr and future C-targeting backends consume `backends::c`.
Non-C backends (if ever) bypass it.

### Completeness: large gaps, prioritized by Phase-8 needs

Compared to C11 Annex A (statement/expression productions),
`lang-c` (`External Declaration`, `Declaration`, `Declarator`,
`Initializer`, `Expression`, `Statement`, `ForStatement`,
`Labeled Statement`, `Switch Statement`), and ANTLR's
`CParser.g4`, the current module covers only top-level scaffolding.

Gaps, ranked by when they must land:

| Gap | Priority | Needed for |
|-----|----------|------------|
| Statement AST (decl-stmt, assignment, if/else, while, return, expression-stmt, compound-stmt) | **Phase-7C urgent** | CFG emitter walks `BasicBlock`s into C; `emit_semantic.rs:524-950` currently string-formats these |
| Expression AST (literals, locals, field, call, unary/binary ops, cast, ternary) | **Phase-7C urgent** | `emit_semantic.rs:363-495` currently string-formats these |
| Initializer + designator AST | **Phase-7C urgent** | `uart_ops`, `uart_driver`, match tables, driver struct initializers currently use `Vec<String>` initializer lines |
| Declarator + type AST (pointers, arrays, qualifiers, storage classes) | **Phase-7C urgent** | Forward declarations and function signatures are raw strings today |
| Labels + `goto` | **Phase-7C urgent** | Phase-8.1 is explicitly goto-based emission (`execution-plan.md:381-389`) |
| `for` / `do while` / `break` / `continue` | Deferrable | PL011 uses only `while`; defer until a real driver needs them |
| `switch` / `case` | Deferrable | Current code uses `if` chains; defer |
| `typedef`, `enum`, `union` | Deferrable | Not needed for current generated code |
| Preprocessor conditionals (`#if`/`#else`/`#endif`), parameterized macros | Deferrable | Current emission does not generate conditional code |
| C23 attribute grammar | Deferrable | Not a completeness driver; generic extension hooks matter more for `__iomem` / `__init` / `__exit` |

### Reference-spec question

Do we need a formal C reference to drive C-AST completeness?

**Answer**: the C standard is a **guardrail, not a target**. Drive
completeness from *"what Regseq drivers actually emit today"* --
reading `emit_semantic.rs`, `emit_module.rs`, and the PL011
fixture tells you what must be representable. Use the ISO C
grammar (C11 Annex A, C23 working draft), `lang-c`'s node types,
and ANTLR's `CParser.g4` as references for **naming and
layering** -- so the Regseq C AST production names match
standard terminology and don't paint themselves into a corner
when a new driver lands that needs a construct we haven't built.

References for guardrail use:
- ISO C11 Annex A (N1570, freely available) -- `statement`,
  `labeled-statement`, `compound-statement`,
  `selection-statement`, `iteration-statement`, `jump-statement`,
  `translation-unit`, `external-declaration`,
  `function-definition`, `postfix-expression`, `initializer-list`,
  `designator`.
- C23 (N3220 working draft) -- `attribute-specifier-sequence`
  (6.7.13.2) if we ever want standardized attributes.
- `lang-c` (`docs.rs/lang-c`) -- a production Rust C99-ish AST;
  useful cross-check for node shape.
- ANTLR `grammars-v4/c/CParser.g4` -- an executable grammar for
  validation.

Do **not** take the approach of implementing full C because the
standard says so. Implement what real Regseq drivers emit; keep
the AST structure compatible with the standard productions so
extension is cheap.

### Concrete recommendations

1. Create `crates/regseq-backends/src/c/` containing the current
   `c_ast.rs` contents (top-level scaffolding). Land as part of
   Phase-7C.
2. In Phase-7C, extend it with statement + expression + declarator
   + initializer + label/goto ASTs -- scoped to what the CFG
   emitter needs for PL011 plus a small buffer.
3. Keep Linux-specific banners, attribute spellings, and kernel
   macro conventions in `linux/`.
4. Document the supported C subset in a top-of-module comment,
   citing C11 Annex A production names. A future
   `crates/regseq-c-ast/` workspace crate may become justified
   when (a) a second live backend consumes it and (b) the
   representation has stabilized.

