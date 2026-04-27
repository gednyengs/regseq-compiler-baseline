# Regseq Compiler Pipeline Refactor

## Purpose

This document defines the refactor plan for the live `regseq-driver`
pipeline orchestration so it matches the progressively populated
Regseq Model architecture in
`docs/discussions/models/regseq-model/spec.md` (sections 12-13).

The target flow is:

```text
Rust source
-> rustc HIR/MIR
-> Layer-1 RegseqModel construction
-> Layer-1 validation
-> Platform file parsing
-> Layer-2 platform binding (in-place population)
-> Layer-2 validation (full-model post-conditions)
-> Full RegseqModel (serializable)
```

Each layer is validated at its own phase boundary; Layer-1
post-conditions (binding slots empty, etc.) cannot be checked after
Layer-2 runs.

Analysis and backend codegen remain disconnected in this iteration:

```text
Full RegseqModel
-> Analyses
-> Backend Codegen
```

This document is scoped to the orchestration layer: how the phases
connect end-to-end, how errors propagate, and how the live callback
produces the full model. Construction internals (HIR/MIR extraction,
body building, monomorphization) are covered separately.

## Summary Finding

The codebase has most of the necessary building blocks, but the live
`EmitSpec::Model` path does not orchestrate them.

The live callback currently does:

```text
after_analysis
-> build_declaration_skeleton
-> emit_regseq_model_json
```

The emitted JSON is a declaration skeleton:

- platform file is not read,
- platform bindings are not joined,
- Phase-2 validation is not reached,
- placeholder function bodies may remain,
- `platform_metadata`, resource `binding`, and property `bound_value`
  fields are absent.

The intended live path should become:

```text
after_analysis
-> Layer-1 RegseqModel construction   (build_layer1_model)
-> Layer-1 validation                 (verify_phase_1)
-> platform file parsing              (parse_platform_file_path)
-> Layer-2 platform binding           (populate_bindings)
-> Layer-2 validation                 (verify_phase_2)
-> Full RegseqModel JSON emission     (emit_regseq_model_json)
```

Layer-2 binding and Layer-2 validation are two separate phases.
`populate_bindings` performs the join only; `verify_phase_2` is an
explicit phase invoked by the orchestrator. This mirrors the Layer-1
`construction -> verify_phase_1` split, keeps every phase boundary
visible in the orchestrator, and makes each transformation and its
verifier independently callable. The current codebase couples them
inside `MutationGuard::commit`; this refactor decouples them
(see Step B3). The live callback reaches neither today.

## Phases

The Regseq driver pipeline has seven top-level phases. Each phase
has a defined input, a defined output, and a single responsibility.

| #   | Phase                     | Input                          | Output                         |
| --- | ------------------------- | ------------------------------ | ------------------------------ |
| 1   | `RustcFrontendPhase`      | Rust source                    | rustc HIR/MIR                  |
| 2   | `Layer1ConstructionPhase` | rustc HIR/MIR                  | Layer-1 `RegseqModel`          |
| 3   | `Layer1ValidationPhase`   | Layer-1 `RegseqModel`          | Layer-1 model + `VerifyReport` |
| 4   | `PlatformParsePhase`      | `platform.toml` path           | `PlatformFile`                 |
| 5   | `Layer2BindingPhase`      | Layer-1 model + `PlatformFile` | Layer-2 (bound) `RegseqModel`  |
| 6   | `Layer2ValidationPhase`   | Layer-2 `RegseqModel`          | Layer-2 model + `VerifyReport` |
| 7   | `FullModelEmitPhase`      | `RegseqModel` (any phase)      | `RegseqModel.json` on disk     |

`Layer1ConstructionPhase` is the only composite phase; its subphases
are described below. Each phase's current code mapping lives in
"Current State By Phase".

`FullModelEmitPhase` operates on any `RegseqModel` regardless of which
phase has run. The live `EmitSpec::Model` path always pairs it with a
Layer-2 model; test harnesses, `--dump-stage`, and analysis-only flows
may pair it with a Layer-1 model.

### Layer1ConstructionPhase subphases

`Layer1ConstructionPhase` decomposes into three subphases that share
rustc HIR/MIR as input and progressively populate the Layer-1
`RegseqModel`:

| Subphase                             | Responsibility                                                                                                     |
| ------------------------------------ | ------------------------------------------------------------------------------------------------------------------ |
| `Layer1DeclarationSubphase`          | Declarations (modules, items, type defs, register blocks, driver struct). Essentially all top-level Rust items.    |
| `Layer1BodySubphase`                 | Function bodies (MIR -> Regseq IR translation).                                                                    |
| `Layer1DependencyDerivationSubphase` | Per-module `abstractions_used` / `intrinsics_used` sets, derived from populated bodies and top-level declarations. |

This decomposition is a construction-internal implementation detail
and is not user-visible. Subphase specifications and execution order
are owned by the construction-internals refactor.

### Phase ownership

Each phase owns its own helpers, registries, extraction surfaces,
and setup logic. The orchestrator (`pipeline::run`)
sequences phases; it does not perform any phase's work inline.
Cross-phase data flows through phase outputs (the typed values in
the phases table), not through orchestrator-side scratch state.

Consequences:

- Phase-internal helpers (extraction surfaces, attribute registries,
  metadata builders) live inside the phase that consumes them, not
  in the orchestrator and not in `RegseqCallbacks::after_analysis`.
- `RegseqCallbacks::after_analysis` is reduced to rustc-integration
  concerns: configure rustc options, register `rseq` tool
  attributes, invoke the orchestrator, dispatch the result to emit /
  dump / fatal. It runs no extraction surfaces of its own.
- Adding a new phase requires no orchestrator surgery beyond
  inserting the call site.
- Each phase is invocable in isolation, which is required for the
  dump-phase plumbing in `Emit, Dump, and Check` (e.g.,
  `--dump-phase=platform-parse` runs phase 4 alone).

### Verification, Analysis, and Backend Checks

The pipeline classifies every check into exactly one of four
categories. The category determines where the check lives, when it
runs, and what kind of error it produces.

| Category | Where it lives | When it runs | Failure mode |
| -------- | -------------- | ------------ | ------------ |
| **Cat 1: Rustc-enforced** | `rustc` itself | At Rust compile time, before our pipeline | Rust compile error; never reaches the Regseq pipeline |
| **Cat 2: RegseqModel structural** | `Layer1ValidationPhase` and `Layer2ValidationPhase` verifiers | Mandatory, after every transformation that produces or modifies a `RegseqModel` | Hard `VerifierError`; aborts the pipeline |
| **Cat 3: Domain analyses** | `regseq-analysis` crate, run as analysis passes after Layer-2 validation | Configurable via `--add-analysis` / `--skip-analysis` / `--deny` / `--allow` | Per-analysis: lint or hard error |
| **Cat 4: Backend-specific** | The relevant backend crate's pre-lowering pass | Only when emitting that backend | Backend codegen error; gates only that backend |

Cat 2 is the verifier. The verifier checks that the IR is
**well-formed** -- it does not enforce semantic, configurable, or
target-specific rules. Concretely, the verifier never adds:

- **Cat 1 rules** (defensive duplicates of rustc's type checker,
  trait resolver, or borrow checker). These read as "we don't trust
  rustc," and rot when the toolchain evolves. The toolchain is
  pinned; rely on it.
- **Cat 3 rules** (domain analyses such as "no `sleep()` in ISR
  context", ISR `bind_target` consistency, `PinCtrlApply` pin-state
  resolution). These belong in `regseq-analysis` so they can be
  configured, skipped, or extended without touching the verifier.
- **Cat 4 rules** (target-specific requirements such as Linux
  needing `trigger_type` on interrupt bindings). These belong with
  the backend that needs them; gating them in the verifier would
  reject specs that compile fine for other targets.

The split mirrors LLVM (mandatory `verify` + optional analysis
passes + target-specific backend checks) and MLIR (verifier traits
+ pass pipelines + dialect-specific lowerings). It is the standing
discipline that keeps the verifier cheap, deterministic, and free
of policy.

Each `### Layer*ValidationPhase` section enumerates its verifier
rule set (Cat 2 only) and lists the rules commonly mistaken for
verifier work (Cat 1 / 3 / 4) with pointers to where they actually
live.

## Emit, Dump, and Check

The pipeline exposes three independent demand-input flags. Each
declares a kind of work to perform; the orchestrator runs the
smallest phase prefix that satisfies the union of all demands.

### `--emit <SPEC>`

Declares a downstream-consumable artifact. Format is stable; output
path is stable. May appear multiple times.

| Spec                | Output                                       | Phases required          |
| ------------------- | -------------------------------------------- | ------------------------ |
| `model` (default)   | `<output_dir>/drivers/<driver>.model.json`   | 1-7                      |
| `analysis` (future) | TBD                                          | 1-6 + analyses           |
| `driver` (future)   | TBD                                          | 1-6 + analyses + backend |

`--emit=model` always emits the Layer-2 model. Layer-1 is
intermediate and is never an emit target; use
`--dump-phase=layer1-validation` to inspect Layer-1.

### `--dump-phase <PHASE>`

Captures the named phase's output for inspection. Format may evolve;
location is debug-oriented and carries no stability promise. May
appear multiple times. Backed by `Vec<DumpPhase>` on
`CompilerOptions`.

| Dump phase            | Dumps                                                                                              | Phases required |
| --------------------- | -------------------------------------------------------------------------------------------------- | --------------- |
| `rustc-frontend`      | rustc HIR/MIR; delegates to `-Z dump-mir`. Output lands in rustc's dump directory, not ours.       | 1               |
| `layer1-construction` | Layer-1 `RegseqModel` JSON (binding slots `None`).                                                 | 1-2             |
| `layer1-validation`   | Layer-1 `RegseqModel` JSON + Phase-1 `VerifyReport`.                                               | 1-3             |
| `platform-parse`      | Parsed `PlatformFile` JSON; the empty `PlatformFile` if `--platform-file` is absent.               | 4               |
| `layer2-binding`      | Layer-2 `RegseqModel` JSON (bindings populated, validation not yet run).                           | 1-5             |
| `layer2-validation`   | Layer-2 `RegseqModel` JSON + Phase-2 `VerifyReport`.                                               | 1-6             |

`FullModelEmitPhase` has no dump variant -- emit is the dump for
that phase boundary.

Dump location (all dumps except `rustc-frontend`):
`<output_dir>/dumps/<driver>/<phase>.json`. The `<phase>` slug uses
the kebab-case names above.

Convenience aggregates: `--dump-phase=all` expands to every dumpable
phase; `--dump-phase=layer1` and `--dump-phase=layer2` expand to the
phases of that layer.

### `--check`

Runs all reachable validation phases but suppresses every `--emit`
side effect. Phase reach: 1-6 (everything through Layer-2
validation). Phase 7 (`FullModelEmitPhase`) is never run under
`--check`.

When combined with `--emit=...`, `--check` wins -- emits are
suppressed. `--dump-phase` is unaffected and still produces dumps.

Use cases: CI gates, IDE-on-save validation, pre-commit hooks.

### Phase-execution rule

```text
executed_phases =
  union(
    phases_required_by(emits),       // includes implicit default if no demand flag is given
    phases_required_by(dumps),
    phases_required_by(check),
  )
```

When none of `--emit`, `--dump-phase`, or `--check` is given, the
orchestrator applies the implicit default `--emit=model`. Specifying
any of those flags (including `--emit=` with an empty spec list)
suppresses the implicit default; the user's flags are the sole
demands.

If the pipeline fails at phase N, dumps for phases < N are still
written; phase N and later are not. This matches LLVM's
`--print-after-all` and rustc's `-Z dump-mir` semantics: the
last-good intermediate stays observable for debugging.

### Platform file is always optional

`--platform-file` is `Option<PathBuf>`. When absent,
`PlatformParsePhase` produces the empty `PlatformFile`; subsequent
phases receive a zero-section input; `Layer2ValidationPhase` rejects
any required binding that lacks a value. Drivers with no required
bindings compile end-to-end without a platform file.

`cargo-regseq` does not validate platform-file presence at the CLI
layer. The pipeline owns the diagnostic attribution.

### Examples

| Invocation                                              | Phases | Emits        | Dumps                    |
| ------------------------------------------------------- | ------ | ------------ | ------------------------ |
| `cargo regseq`                                          | 1-7    | `model.json` | none                     |
| `cargo regseq --check`                                  | 1-6    | none         | none                     |
| `cargo regseq --check --dump-phase=layer1-validation`   | 1-6    | none         | `layer1-validation.json` |
| `cargo regseq --emit=model --dump-phase=layer2-binding` | 1-7    | `model.json` | `layer2-binding.json`    |
| `cargo regseq --emit=model --check`                     | 1-6    | none         | none                     |
| `cargo regseq --dump-phase=layer1-validation`           | 1-3    | none         | `layer1-validation.json` |

## Why These Changes Are Needed

The model spec defines a progressively populated IR:

1. Phase 1 constructs a declaration-side `RegseqModel` from rustc
   HIR/MIR.
2. Phase 2 joins platform-file data into the same model.
3. Verification enforces phase boundaries.
4. The live `EmitSpec::Model` path emits the fully populated Layer-2
   model. The model is structurally serializable at every phase, so
   test harnesses, `--dump-stage`, and future analysis-only flows may
   emit Layer-1.

The live callback currently serializes too early. This creates several
practical problems:

- `--platform-file` is accepted and validated by `cargo-regseq` but
  never consumed by `regseq-driver` in the live model emit path.
  `CompilerOptions.platform_file` has zero readers in
  `crates/regseq-driver/src`.
- Output JSON is missing Layer-2 information even when a valid
  platform file is supplied.
- The JSON artifact looks like a compiler product but is not the full
  model downstream consumers should read.
- Phase-2 errors cannot surface from live model emit because Phase 2
  is never run.
- Existing `model_emit` coverage can pass with an empty platform
  file, which proves the live path is not testing the intended flow.
- Errors are flattened to `eprintln!` rather than propagated as
  structured fatal diagnostics.

## Current State By Phase

### RustcFrontendPhase

Current status: implemented and live, but the callback body has
significant phase-ownership violations and dead-code accumulation
from an earlier design.

Relevant code:

- `crates/regseq-driver/src/callbacks.rs`
- `RegseqCallbacks::config`
- `RegseqCallbacks::after_analysis`

What it does today (rustc-integration concerns -- correctly
located):

- Runs inside rustc's `after_analysis` hook.
- Receives a live `TyCtxt`.
- Registers `rseq` tool attributes.
- Disables normal codegen.
- Sets MIR-related options such as `mir_opt_level = Some(0)` and
  `always_encode_mir = true`.

What it also does today (phase work that does not belong here):

- Builds the four attribute registries (`IntrinsicRegistry`,
  `FrameworkTypeRegistry`, `AbstractionTypeRegistry`,
  `PinStateRegistry`). These are consumed by
  `Layer1ConstructionPhase` and should be built inside that phase's
  setup, not in the callback.
- Calls 12+ extraction surfaces
  (`extract_source_locations`, `extract_driver_identities`,
  `extract_register_access_surface`, `extract_interrupt_surface`,
  `extract_synchronization_surface`, `extract_trait_surface`,
  `extract_property_surface`, `extract_clock_surface`,
  `extract_reset_surface`, `extract_pinctrl_surface`,
  `extract_timed_delay_surface`, `extract_logging_surface`,
  `extract_pm_surface`) and `eprintln!`s the results. These calls
  are dead -- their results are never consumed. Where the model
  needs them, `construction.rs` re-runs them internally
  (`construction.rs:896-931`). They are remnants of a pre-
  `construction.rs` design that emitted directly from the
  callback.
- Runs the `REGSEQ_VALIDATE_ATTRS`-gated debug pass
  (`validate_cross_crate_attrs`), which is unrelated to either
  rustc integration or the live pipeline.

Current gap:

- The callback is doing orchestration AND extraction inline,
  violating phase ownership.
- All authentic Layer-1 extraction work happens twice: once for
  `eprintln!` logging in the callback, once for real consumption
  in `construction.rs`.
- The live model emit branch jumps directly from declaration
  skeleton construction to JSON emission (skipping
  `Layer1ValidationPhase`, `PlatformParsePhase`,
  `Layer2BindingPhase`, and `Layer2ValidationPhase`).
- Errors are logged with `eprintln!` rather than propagated as
  structured fatal pipeline errors.

Needed changes:

- Strip the callback to rustc-integration concerns only: configure
  rustc options, register `rseq` tool attributes, invoke
  `pipeline::run`, dispatch the result to emit / dump
  / fatal. No extraction work and no registry construction in the
  callback body.
- Move attribute-registry construction
  (`IntrinsicRegistry`/`FrameworkTypeRegistry`/`AbstractionTypeRegistry`/`PinStateRegistry`)
  into `Layer1ConstructionPhase`'s setup. The phase already takes
  `tcx`; nothing in rustc forces these registries to be built at
  the callback boundary.
- Delete the 12+ dead extraction calls from the callback. The
  surface modules themselves (`crate::extract::*`) stay where they
  are; only the callback-side invocations go.
- Keep or remove the `REGSEQ_VALIDATE_ATTRS` debug gate at the
  implementer's discretion; if kept, route it through a dedicated
  `--debug-pass` mechanism rather than ad-hoc env-var detection in
  `after_analysis`.
- Route pipeline failures through `tcx.dcx().fatal(...)` (see the
  Decisions section) instead of silently logging and continuing.

### Layer1ValidationPhase

Current status: implemented but under-wired and live-uninvoked.
The `verify_phase_1` entrypoint calls only two of the six rule
modules in `crates/regseq-model/src/verify/rules/`.

Relevant code:

- `crates/regseq-model/src/verify/phase_1.rs` (entrypoint -- calls
  `rules::module::run` and `rules::cross_item::run` only).
- `crates/regseq-model/src/verify/rules/module.rs` (wired).
- `crates/regseq-model/src/verify/rules/cross_item.rs` (wired).
- `crates/regseq-model/src/verify/rules/body.rs` (defines
  `pub fn run`; unwired).
- `crates/regseq-model/src/verify/rules/driver_struct.rs` (defines
  `pub fn run`; unwired).
- `crates/regseq-model/src/verify/rules/body.rs::run_with_isr_forbidden`
  (defines an ISR-context entry point; unwired -- but see Cat 3
  notes below).

#### Layer-1 verifier rule set (Cat 2 only)

The Layer-1 verifier enforces this closed set of structural
invariants. Each entry tracks current implementation status; the
goal is "covered" everywhere.

**Phase-1 boundary** (no Layer-2 leakage)

- [x] All resource `binding` slots are `None`.
- [x] All property `bound_value` slots are `None`.
- [x] Driver struct `platform_metadata` is `None`.

**Structural integrity** (model is well-formed)

- [x] Every assigned `DeclId` has a `source_locations` entry.
- [x] Every `TypeDefId` / `FunctionId` / `RegisterBlockId` /
      `RegisterDescriptorId` / `ConstantId` reference resolves.
- [ ] Every `BasicBlockId` and `LocalId` reference inside a body
      resolves to a declared block / local. (Module: `body.rs`,
      unwired.)
- [ ] Every `OrderedRegion { entry, exit }` is single-entry,
      single-exit: every block reachable from `entry` without
      passing through `exit` is the region; no other CFG predecessor
      enters mid-region. (Not present.)

**Module tree** (DSL well-formedness, structural)

- [x] Exactly one Driver module.
- [x] No duplicate item names within any module.

**Driver struct** (DSL well-formedness, structural)

- [x] Driver identity present (name, compatible).
- [~] Every resource field has its required `id`.
- [ ] No two resources of the same kind share an `id`. Required for
      deterministic Phase-2 binding match. (Module:
      `driver_struct.rs::check_unique`, unwired.)
- [ ] Every property field has its required `property_id`.
- [~] User-data field is `Option<_>` shape. (Note: the inner
      `T: Copy` bound is Cat 1 -- enforced by rustc.)

**Trait impls** (DSL well-formedness, structural)

- [ ] Each Regseq trait impl declares the full required method set
      (Lifecycle: init/deinit; Serial: 10 methods; etc.). (Not
      present.)

**Module dependency derivation** (IR consistency)

- [ ] Each module's `abstractions_used` set equals the abstractions
      actually referenced by items/bodies in that module.
- [ ] Each module's `intrinsics_used` set equals the intrinsics
      actually invoked by bodies in that module.

Legend: `[x]` covered today; `[~]` partial / inconsistent location;
`[ ]` not covered. Wiring the unwired modules (`body.rs`,
`driver_struct.rs`) closes most of the partials immediately; the
rest are net-new rules.

#### Intentionally NOT in the Layer-1 verifier

The following rules are commonly mistaken for verifier work but
belong elsewhere. Adding them to `verify_phase_1` is a category
error.

**Cat 1 -- enforced by rustc** (don't duplicate)

- Operand types match operation expectations. (Rust's type system.)
- Args supplied to a `Call` match the callee's `Param` list. (MIR
  is well-typed by construction.)
- No writes to read-only registers. (Rustc rejects via
  `R::Access: WriteAccess` trait bound on `Mmio::write`.)
- Cross-block field usage. (Rustc rejects via
  `R: RegisterOf<Block>` bound.)
- ISR handler return type is `IrqResult`. (Rust signature check.)
- User-data inner `T: Copy` bound. (Rust trait bound.)
- `Function` has a body. (MIR extraction would already have
  failed.)

The corresponding `ConstructionError` defensive variants
(`CrossBlockFieldUsage`, `WrongRegisterAccessKind`) are also Cat 1
and are candidates for removal -- see the Open Questions section.

**Cat 3 -- domain analyses** (live in `regseq-analysis`)

- No `sleep()` / `busy_wait()` in an ISR-context body.
- ISR `bind_target` consistency (`Auto` requires exactly one
  interrupt; `ExplicitField(name)` matches a declared interrupt).
- `has_index_param == true` implies the targeted interrupt
  resource has shape `Array(N)`.
- `PinCtrlApply` marker resolves to a declared `pin_state`.
- Logging template arity matches `LogArg` count.
- Resource-cleanup correctness on error paths
  (dominator-based rollback analysis from spec.md Section 10).
- Reachability / dead-code analysis on bodies.

**Cat 4 -- backend-specific** (live with each backend)

- Linux interrupt binding requires `trigger_type`.
- Linux device-tree `compatible` string format.
- Architecture-specific MMIO alignment / size limits.
- Zephyr `IRQ_CONNECT`-compatible binding shape.

Current gap:

- The live callback does not call `verify_phase_1`.
- Several Cat 2 rules above are unwired (`body.rs`,
  `driver_struct.rs`, `OrderedRegion` SESE) or net-new
  (trait-impl method sets, dependency-derivation consistency).
- The current live skeleton would fail strict Phase-1 validation
  because bodies remain placeholders -- a construction prerequisite
  owned by the construction-internals refactor.

Needed changes:

- Call `verify_phase_1(&model)` after `Layer1ConstructionPhase`.
- Preserve the returned `VerifyReport` end-to-end via
  `PipelineOutput`.
- Wire the disconnected rule modules (`body.rs`,
  `driver_struct.rs`) into `verify_phase_1`.
- Add the net-new Cat 2 rules listed above as their own
  `verify::rules::*` modules; each gets its own unit tests under
  `crates/regseq-model/tests/`.

### PlatformParsePhase

Current status: implemented, not live-orchestrated.

Relevant code:

- `crates/regseq-model/src/platform_parser.rs`
- `parse_platform_file_path`
- `parse_platform_file_str`
- `crates/regseq-config/src/options.rs`
- `CompilerOptions.platform_file`
- `crates/cargo-regseq/src/cli.rs`

What it captures today:

- Required `version`.
- Required `[platform]` fields: `os`, `board`, `arch`.
- Optional `[bus]`.
- `[properties]`.
- Dotted construct sections: `[mmio.<id>]`, `[interrupt.<id>]`,
  `[clock.<id>]`, `[reset.<id>]`, `[pinctrl.<id>]`.
- Unknown/future sections are preserved or classified depending on
  structure.

Current gap:

- `cargo-regseq` validates that the platform file path exists and
  serializes it into `CompilerOptions`.
- `regseq-driver` does not read `self.options.platform_file` in the
  live model emit path.

Needed changes:

- In the pipeline wrapper, call:

  ```rust
  let platform = regseq_model::platform_parser::parse_platform_file_path(
      &options.platform_file,
  )?;
  ```

- Map parser failures to structured pipeline errors.

### Layer2BindingPhase

Current status: implemented, not live-orchestrated.

Relevant code:

- `crates/regseq-model/src/join/mod.rs`
- `populate_bindings`
- `crates/regseq-model/src/join/sections.rs`

What it captures today:

- `DriverStruct.platform_metadata` from `[platform]` and `[bus]`.
- Resource bindings matched by declared IDs:
  clock, reset, pinctrl, interrupt, MMIO.
- Property `bound_value` entries from `[properties]`.

Validation behavior:

- Unknown platform sections are rejected during join.
- Property type mismatches are rejected during join.

Current gap:

- The live callback never calls `populate_bindings`.
- `populate_bindings` is currently coupled to `verify_phase_2` via
  `MutationGuard::commit`. Layer-2 validation is a separate phase
  (see `Layer2ValidationPhase` and the Decisions section); this
  coupling is removed in B3.

Needed changes:

- Decouple `populate_bindings` from `verify_phase_2`: pure binding
  returns `Result<(), JoinError>` with no auto-verification.
- `JoinError` no longer carries a `Phase2VerificationFailed`
  variant; missing required bindings surface from
  `verify_phase_2`, not from the join.
- Call `populate_bindings(&mut model, &platform)` after Phase-1
  validation; the orchestrator follows it with an explicit
  `verify_phase_2` call.
- Treat `JoinError` as a fatal model build error
  (`PipelineError::PlatformJoin`).

### Layer2ValidationPhase

Current status: rule set partially implemented; entrypoint coupled
to `populate_bindings` via `MutationGuard::commit`.

Relevant code:

- `crates/regseq-model/src/verify/phase_2.rs`
- `crates/regseq-model/src/verify/rules/phase_2_boundary.rs`
- `crates/regseq-model/src/verify/rules/phase_2_cross_item.rs`
- `MutationGuard::commit` (current invocation site -- removed in B3)

#### Layer-2 verifier rule set (Cat 2 only)

The Layer-2 verifier enforces this closed set of structural
post-binding invariants. It re-runs the Layer-1 rule set on the
mutated model (the structural invariants from Phase 1 must still
hold) and adds the Phase-2 boundary rules.

**Phase-1 invariants still hold** (re-run after Phase 2)

- [x] Every Layer-1 Cat 2 rule from `Layer1ValidationPhase` is
      re-checked. The Layer-2 verifier uses the same rule modules,
      not duplicated logic. Any rule unwired in Layer 1 is also
      unwired here.

**Phase-2 boundary completeness**

- [x] Every required resource declaration has a populated
      `binding`. (`phase_2_boundary.rs`.)
- [x] Every required property has a populated `bound_value`.
- [ ] `DriverStruct.platform_metadata` is populated when the
      pipeline received a non-empty `PlatformFile`. (Empty
      `PlatformFile` legitimately leaves it `None`; only flag the
      case where some bindings were joined but metadata was not.)

**Phase-2 cross-item integrity**

- [x] Every populated `binding` references a declared resource.
      (`phase_2_cross_item.rs`.)
- [x] Every populated `bound_value` references a declared
      property.
- [ ] Optional resource bindings that ARE populated reference an
      existing platform section (defensive; deterministic Phase-2
      join already enforces this, but the post-condition should be
      checkable without re-reading the `PlatformFile`).

Legend: `[x]` covered today; `[ ]` not covered.

#### Intentionally NOT in the Layer-2 verifier

**Cat 1 -- enforced by rustc**

- Type compatibility between a property's declared `ScalarType` and
  the platform-supplied value at the type system level is N/A
  (platform values come from TOML, not Rust); the join's
  `PropertyTypeMismatch` is correctly a join-time error
  (`JoinError`), not a verifier rule.

**Cat 3 -- domain analyses** (live in `regseq-analysis`)

- Resource-cleanup / rollback correctness now that bindings are
  known.
- Initialization-order analysis using the bound resources.
- Cross-resource consistency (e.g., MMIO region `size` is large
  enough for the referenced register block's footprint -- this is
  a useful analysis, but it's a semantic check on the joined
  values, not a structural well-formedness rule).
- Overlapping MMIO regions across resources.

**Cat 4 -- backend-specific** (live with each backend)

- Linux interrupt binding requires `trigger_type` (the verifier
  accepts a binding without `trigger_type` because Zephyr or
  other backends may not need it).
- Linux device-tree `compatible` string format.
- Architecture-specific MMIO alignment / size limits.

Current gap:

- No standalone callable entrypoint -- always invoked indirectly
  through `MutationGuard::commit` inside `populate_bindings`.
- Cannot be called separately from binding, which blocks
  `layer2-binding` dump capture (the bound-but-not-validated state
  is unobservable today).
- Diagnostics are not surfaced through a driver-level pipeline
  error.
- A handful of Cat 2 rules above are net-new (`platform_metadata`
  populated, optional-binding section reference).

Needed changes:

- Expose `verify_phase_2(&model) -> Result<VerifyReport,
  VerifierError>` as a standalone public function, symmetric with
  `verify_phase_1`.
- Remove the auto-invocation from `MutationGuard::commit`; the
  orchestrator calls `verify_phase_2` explicitly after
  `populate_bindings`.
- Surface failures through `PipelineError::Phase2Verification`.
- Preserve the returned `VerifyReport` end-to-end via
  `PipelineOutput`.
- Add the net-new Cat 2 rules listed above; each gets its own unit
  test under `crates/regseq-model/tests/`.

### FullModelEmitPhase

Current status: implemented and live, but ordered incorrectly.

Relevant code:

- `crates/regseq-driver/src/callbacks.rs`
- `emit_regseq_model_json`
- `crates/regseq-model/src/serialize.rs`
- `serialize_regseq_model`

What it does today:

- Serializes a `RegseqModel` with symbols resolved to strings.
- Writes to either an explicit model path or
  `<output_dir>/drivers/<driver_name>.model.json`.

Current gap:

- It emits the model before platform parsing, platform binding, and
  validation.
- JSON write failures are strings rather than structured errors.

Needed changes:

- Move emission after full model construction.
- Preserve current behavior that JSON write failures are JSON
  emission failures, not construction or platform binding failures.
- Keep path override behavior unchanged.

## Decisions

- **Verifier reports:** preserve both Phase-1 and Phase-2
  `VerifyReport`s end-to-end via symmetric standalone verifiers.
  - `verify_phase_1(&model) -> Result<VerifyReport, VerifierError>`
    is unchanged; `pipeline::run` keeps the report
    instead of discarding.
  - `verify_phase_2(&model) -> Result<VerifyReport, VerifierError>`
    is exposed as a standalone callable function, symmetric with
    `verify_phase_1`. The pipeline calls it explicitly after
    `populate_bindings`.
  - `PipelineOutput` carries `phase1_report: VerifyReport` and
    `phase2_report: VerifyReport`. Rationale: symmetry (both phases
    produce lints per spec Section 13); downstream lint-surface work
    gets both reports without another API churn.

- **Bind/verify decoupling:** `populate_bindings` performs the join
  only; `verify_phase_2` is a separate, explicitly-invoked phase. The
  current code couples them inside `MutationGuard::commit`; this
  refactor breaks that coupling. Reasons: (a) symmetry with the
  Layer-1 `construction -> verify_phase_1` split; (b) every phase
  boundary stays visible in the orchestrator; (c) each transformation
  and its verifier are independently callable, which is necessary for
  the dump-phase plumbing in `Emit, Dump, and Check` (`layer2-binding`
  dumps must capture the bound-but-not-yet-validated model).
  Implementation freedom: whether `MutationGuard` retains any role
  beyond verification is left to B3.

- **Fatal diagnostics:** `tcx.dcx().fatal(msg)` from the callback.
  Unwinds through `catch_with_exit_code`; `cargo-regseq` inherits
  rustc's non-zero exit. A driver-local
  `format_pipeline_error(&PipelineError) -> String` produces a single
  structured message per variant.

## Proposed Production Entrypoints

### `pipeline::run`

Add a driver-level pipeline module at
`crates/regseq-driver/src/pipeline.rs`.

Entrypoint:

```rust
pub fn run<'tcx>(
    tcx: TyCtxt<'tcx>,
    options: &CompilerOptions,
) -> Result<PipelineOutput, PipelineError>
```

Per the `Phase ownership` principle, `pipeline::run` does not
take pre-built attribute registries. `Layer1ConstructionPhase`
builds them internally from `tcx` as part of its setup.

Responsibilities:

1. `build_layer1_model` (defined in `model-refactor.md`); builds
   its own attribute registries internally.
2. `verify_phase_1`.
3. `parse_platform_file_path` (or `PlatformFile::default()` if no
   `--platform-file`).
4. `populate_bindings` (pure binding only; no auto-verification).
5. `verify_phase_2` (explicit Layer-2 validation call).
6. Return the fully populated model and both verify reports.

### `PipelineOutput` and `PipelineError`

```rust
pub struct PipelineOutput {
    pub model: RegseqModel,
    pub phase1_report: VerifyReport,
    pub phase2_report: VerifyReport,
}

pub enum PipelineError {
    Construction(ConstructionError),
    Phase1Verification(VerifierError),
    PlatformParse(PlatformParseError),
    PlatformJoin(JoinError),
    Phase2Verification(VerifierError),
}
```

JSON emission stays outside `pipeline::run` so write failures
remain clearly classified as emission failures.

The callback handles:

```text
PipelineError -> model construction / platform / validation failure
JSON emit error -> JSON emission failure
```

## Callback Refactor

Current live model branch:

```text
build_declaration_skeleton
-> emit_regseq_model_json
```

Proposed live model branch:

```text
pipeline::run
-> emit_regseq_model_json
```

Driver and analysis emit branches remain stubbed:

- `EmitSpec::Driver`: accepted but not connected.
- `EmitSpec::Analysis`: accepted but not connected.

No backend or analysis codegen is connected as part of this refactor.

## Error Handling Requirements

For the live compiler pipeline, failures should be fatal.

Recommended behavior:

- Construction failure: fatal.
- Phase-1 verification failure: fatal.
- Platform parse failure: fatal.
- Platform join / Phase-2 verification failure: fatal.
- JSON write failure: fatal, classified as emission failure.

Fatal diagnostic mechanism: `tcx.dcx().fatal(msg)` from the callback.
Unwinds through `catch_with_exit_code`; `cargo-regseq` inherits
rustc's non-zero exit.

## Execution Plan: Wave B -- Pipeline Orchestration

Builds on Wave A from `model-refactor.md`. Acceptance targets the
callback, the pipeline module, and the live `model_emit` test.

### Step B1 -- `construction::build_layer1_model` orchestrator

`build_layer1_model` is the construction-level orchestrator that
consumes Wave A outputs. Its specification and acceptance criteria
live in `model-refactor.md`. Pipeline-layer acceptance for this step
is that the narrow synthetic fixture from B2 and the PL011 fixture
each produce a Layer-1 model on which `verify_phase_1` returns
`Ok(_)`.

Codex review gate.

### Step B2 -- Narrow synthetic fixture

Scope: author the minimum fixture needed for the live pipeline test.
One driver plus one of each resource kind. No closures. At least one
cross-crate concrete monomorphization (for example,
`Option::<u32>::unwrap_or`) to exercise Wave A beyond trivial cases.

Files to create:

- `tests/fixtures/narrow-full-spec/Cargo.toml`
- `tests/fixtures/narrow-full-spec/src/lib.rs` -- `#[rseq::driver]`
  struct with `#[rseq::mmio]`, `#[rseq::clock]`, `#[rseq::reset]`,
  `#[rseq::interrupt]`, `#[rseq::pinctrl]`, `#[rseq::property]`
  fields, and a `Lifecycle::init` that calls `clock.enable()`,
  `reset.deassert()`, a register write, and `Option::unwrap_or` on
  a property.
- `tests/fixtures/narrow-full-spec/platform.toml` -- matching
  `[platform]`, `[bus]`, one section per resource, one property
  value.

Acceptance: fixture compiles via
`cargo check --manifest-path tests/fixtures/narrow-full-spec/Cargo.toml`;
B1's `build_layer1_model` test consumes it. The fixture lives
outside the workspace; `-p narrow-full-spec` will not resolve.

Codex review gate.

### Step B3 -- Decouple `populate_bindings` from `verify_phase_2`

Scope: split Layer-2 binding and Layer-2 validation into two
independently callable phases. Expose `verify_phase_2` as a
standalone function symmetric with `verify_phase_1`. Preserve both
verifier reports end-to-end through this split.

Files to modify:

- `crates/regseq-model/src/verify/phase_2.rs` -- expose
  `pub fn verify_phase_2(&RegseqModel) -> Result<VerifyReport,
  VerifierError>` if not already public; ensure it returns a
  `VerifyReport` on success and a `VerifierError` on failure (no
  wrapping in `JoinError`).
- `crates/regseq-model/src/join/mod.rs:177-198` -- change
  `populate_bindings(&mut RegseqModel, &PlatformFile)
  -> Result<(), JoinError>` to do binding only. Remove the
  `MutationGuard::commit` -> `verify_phase_2` invocation.
- `crates/regseq-model/src/join/mod.rs` (`JoinErrorKind`) -- drop
  the `Phase2VerificationFailed` variant; `JoinError` no longer
  wraps verifier errors.
- `crates/regseq-model/src/model/mutation.rs` (or wherever
  `MutationGuard` lives) -- remove the auto-`verify_phase_2` from
  `commit`. Whether `MutationGuard` retains any other purpose is an
  implementation choice for this step; if it provides no remaining
  value, delete it.
- No change needed to `verify_phase_1`.

Acceptance:

- `verify_phase_2` is callable on any post-Phase-1 model regardless
  of whether `populate_bindings` has run; tests cover both the
  bound-and-valid and bound-but-invalid cases.
- A new unit test in `crates/regseq-model/tests/` confirms
  `populate_bindings` succeeds against a platform file that joins
  cleanly but leaves required bindings unset; `verify_phase_2` then
  fails with `RequiredBindingMissing`. Proves the decoupling.
- Existing join tests are updated to call `verify_phase_2`
  explicitly where they relied on auto-validation; behavior on
  malformed input is unchanged.
- No production caller invokes `verify_phase_2` through a
  `populate_bindings` side effect.

Codex review gate.

### Step B4 -- `pipeline::run` + `PipelineError`

Scope: add the driver-level pipeline module.

Files to create:

- `crates/regseq-driver/src/pipeline.rs` (see `PipelineError` and
  `PipelineOutput` above).

Algorithm:

1. `let mut model = build_layer1_model(tcx)
   .map_err(PipelineError::Construction)?;`
2. `let phase1_report = verify_phase_1(&model)
   .map_err(PipelineError::Phase1Verification)?;`
3. Parse the platform file when provided; otherwise use
   `PlatformFile::default()`. Concretely:
   `let platform = match &options.platform_file {
       Some(path) => parse_platform_file_path(path)
           .map_err(PipelineError::PlatformParse)?,
       None => PlatformFile::default(),
   };`
4. `populate_bindings(&mut model, &platform)
   .map_err(PipelineError::PlatformJoin)?;`
5. `let phase2_report = verify_phase_2(&model)
   .map_err(PipelineError::Phase2Verification)?;`
6. `Ok(PipelineOutput { model, phase1_report, phase2_report })`.

Files to modify:

- `crates/regseq-driver/src/lib.rs` -- `pub mod pipeline;`.

Acceptance:

- New test `crates/regseq-driver/tests/pipeline_run.rs` --
  narrow synthetic fixture + `platform.toml`; assert emitted JSON
  contains `platform_metadata.os/board/arch`, at least one resource
  `binding`, at least one property `bound_value`, and no function
  with a one-block `Unreachable` body.
- Negative driver-level tests (drive `pipeline::run` directly):
  malformed TOML -> `PipelineError::PlatformParse`;
  platform section with unknown resource id ->
  `PipelineError::PlatformJoin(_)`.
- Optional-platform-file test (drive `pipeline::run` directly):
  `options.platform_file = None` against a narrow synthetic with
  required bindings ->
  `PipelineError::Phase2Verification(RequiredBindingMissing)`. Same
  fixture with an all-optional-binding driver -> `Ok(_)` with
  empty bindings.

Codex review gate.

### Step B5 -- Switch live callback to `pipeline::run`

Scope: replace the current `EmitSpec::Model` branch; strip the
callback to rustc-integration concerns only; wire fatal diagnostics.

Files to modify:

- `crates/regseq-driver/src/callbacks.rs` -- delete the 12+ dead
  `extract::*_surface` calls in `after_analysis` (the surface
  modules themselves stay; only the callback-side invocations go).
  Delete the four `*::Registry::build(tcx)` calls -- registries
  move into `Layer1ConstructionPhase`'s setup. Delete the
  `REGSEQ_VALIDATE_ATTRS` debug gate (or relocate it behind a
  proper `--debug-pass` flag; this is implementation choice).
- `crates/regseq-driver/src/callbacks.rs:200-215` -- replace the
  `wants_model()` branch with:

  ```rust
  match pipeline::run(tcx, &self.options) {
      Ok(out) => match emit_regseq_model_json(
          &out.model,
          &self.options.output_dir,
          self.model_path_override(),
      ) {
          Ok(path) => eprintln!(
              "regseq-driver: RegseqModel JSON emitted to {}",
              path.display(),
          ),
          Err(e) => tcx.dcx().fatal(
              format!("RegseqModel JSON emission failed: {e}"),
          ),
      },
      Err(e) => tcx.dcx().fatal(format_pipeline_error(&e)),
  }
  ```

  The post-strip `after_analysis` body should consist of: rustc
  options (already in `config()`), `rseq` tool registration, the
  block above, and dump/check dispatch (when those land).
- Add local helper
  `format_pipeline_error(&PipelineError) -> String` producing one
  structured line per variant.
- Driver and analysis `EmitSpec::*` branches stay stubbed via
  `eprintln!` -- not fatal (they are not connected yet).

Acceptance:

- `EmitSpec::Model` with narrow synthetic + valid platform file
  -> exit 0, JSON at `<output_dir>/drivers/<driver>.model.json`
  with Layer-2 fields.
- Malformed platform TOML -> non-zero exit with a
  `format_pipeline_error`-produced fatal diagnostic.
- Invocation with no `--platform-file` against a driver that has
  required bindings -> non-zero exit from a `format_pipeline_error`
  diagnostic
  (`PipelineError::Phase2Verification(RequiredBindingMissing)`).
- Invocation with no `--platform-file` against a driver with no
  required bindings -> exit 0; emitted JSON has all binding slots
  `None`.

Codex review gate.

### Step B6 -- Strengthen live tests

Scope: close the empty-platform-file loophole; add negative live
tests at the correct layer.

Files to modify:

- `crates/regseq-driver/tests/model_emit.rs` -- swap fixture to
  narrow synthetic; write a real `platform.toml`; assert
  `platform_metadata.os/board/arch`, at least one resource
  `binding`, at least one property `bound_value`, at least one
  function with a non-placeholder `Body`.

Files to create:

- `crates/regseq-driver/tests/model_emit_malformed_platform.rs` --
  drive the compiler with a valid `--platform-file` path pointing
  at a malformed TOML; assert non-zero exit AND no JSON written.
  Match on a stable marker string emitted by
  `format_pipeline_error`, not verbatim stderr.
- `crates/cargo-regseq/tests/cli_optional_platform.rs` (or extend
  existing CLI tests) -- invoke `cargo-regseq` without
  `--platform-file`; assert the CLI accepts the invocation
  (`CompilerOptions.platform_file == None`) and the driver runs.
  Pair with a fixture that has no required bindings so the run
  succeeds end-to-end, proving the absent-platform-file path is a
  first-class flow.
- `crates/cargo-regseq/src/cli.rs` -- change `platform_file:
  PathBuf` to `Option<PathBuf>`; remove the existence check at
  `cli.rs:88` (the pipeline now owns the diagnostic).

Acceptance: workspace CI green. No test uses an empty platform
file as a stand-in for a missing one. Absent-flag behavior is
exercised in `cargo-regseq`; pipeline-error behavior (malformed
TOML, unknown resource id, required binding missing) is exercised
in `regseq-driver`.

Codex review gate. End of Wave B.

## Testing Plan

### Update live `model_emit` test

Current `model_emit` uses an empty platform file and still passes.
That changes under B6.

Add a real platform file with at least:

- `[platform]`,
- `[bus]` if the fixture has bus metadata,
- one required resource binding,
- one required property value.

Assertions on emitted JSON:

- `platform_metadata` exists.
- `platform_metadata.os`, `board`, and `arch` match the platform file.
- Resource `binding` fields exist where applicable.
- MMIO binding has `base` and `size`.
- Interrupt binding has `irq_number`.
- Clock binding has `default_frequency` if declared.
- Property `bound_value` exists for required properties.

Also assert that the emitted JSON does not represent a plain skeleton.
For functions expected to translate, ensure their bodies are not the
one-block placeholder `Unreachable` body.

### Add platform-binding pipeline test

Test:

```text
Rust fixture
-> build_layer1_model
-> verify_phase_1
-> parse platform file
-> populate_bindings
-> emitted model has Layer-2 fields
```

This can be done without invoking the cargo wrapper if the rustc
test harness can access `TyCtxt`.

### Add negative platform-file test

Test one clean failure mode:

- missing platform file, or
- malformed TOML, or
- platform section references an unknown resource id.

Avoid overfitting exact stderr. Prefer asserting:

- pipeline returns the expected error variant, or
- live compiler run fails and no JSON output is written.

### Keep existing parser and verifier tests

The parser, joiner, and verifier already have focused tests. The new
tests prove integration, not duplicate every parser/verifier case.

## Verification

End-to-end validation sequence once both Waves are in:

1. Unit / construction tests: `cargo test --workspace` green.
2. Wave B direct check:
   `cargo test -p regseq-driver --test pipeline_run` --
   narrow synthetic produces a model with `platform_metadata`,
   resource `binding`, and property `bound_value` populated.
3. Live CLI:
   `cargo run --bin cargo-regseq -- --emit=model
   --platform-file=tests/fixtures/narrow-full-spec/platform.toml`
   against narrow synthetic. Inspect emitted JSON for Layer-2
   fields; confirm no `Unreachable`-only bodies in function
   entries.
4. Optional-platform-file path:
   `cargo run --bin cargo-regseq -- --emit=model`
   (no `--platform-file`) against narrow synthetic with required
   bindings -> non-zero exit from a `format_pipeline_error`
   diagnostic
   (`PipelineError::Phase2Verification(RequiredBindingMissing)`),
   no JSON written. Same invocation against a fixture with no
   required bindings -> exit 0, JSON written with all binding slots
   `None`.
5. Pipeline-level negative: truncate the platform TOML to a
   malformed fragment, re-run step 3 -- non-zero exit from a
   fatal diagnostic produced by `format_pipeline_error`, no JSON
   written.
6. Check mode: `cargo run --bin cargo-regseq -- --check
   --platform-file=tests/fixtures/narrow-full-spec/platform.toml`
   against narrow synthetic -- exit 0, no JSON written under
   `<output_dir>/drivers/`.
7. Dump mode: `cargo run --bin cargo-regseq --
   --dump-phase=layer1-validation` against narrow synthetic
   (no `--platform-file`) -- exit 0, dump file written under
   `<output_dir>/dumps/<driver>/layer1-validation.json`, no
   production JSON written.

## Out of Scope

- `EmitSpec::Driver` and `EmitSpec::Analysis` wiring; Linux/Zephyr
  backend codegen.
- Broad renaming (`build_declaration_skeleton` ->
  `build_layer1_declaration_skeleton`, etc.) -- follow-up cleanup
  once the refactor is merged.
- Surfacing Phase-2 `VerifyReport` lints to the user -- the report
  is preserved on `PipelineOutput` but not yet displayed.

## Critical Files

- `crates/regseq-driver/src/callbacks.rs` (B5)
- `crates/regseq-driver/src/pipeline.rs` -- new (B4)
- `crates/regseq-model/src/join/mod.rs` (B3)
- `tests/fixtures/narrow-full-spec/` -- new (B2)
- `crates/regseq-driver/tests/model_emit.rs` (B6)
- `crates/regseq-driver/tests/model_emit_malformed_platform.rs`
  -- new (B6)
- `crates/cargo-regseq/tests/cli_optional_platform.rs` -- new (B6)
- `crates/cargo-regseq/src/cli.rs` -- `platform_file` becomes
  `Option<PathBuf>`; existence check removed (B6)
- `crates/regseq-driver/tests/pipeline_run.rs` -- new (B4)

## Reusable Anchors (call signatures preserved or evolving)

- `parse_platform_file_path`
  (`crates/regseq-model/src/platform_parser.rs`) -- unchanged.
- `verify_phase_1`
  (`crates/regseq-model/src/verify/phase_1.rs`) -- unchanged.
- `emit_regseq_model_json`
  (`crates/regseq-driver/src/callbacks.rs`) -- unchanged.
- `populate_bindings`
  (`crates/regseq-model/src/join/mod.rs:177`) -- signature
  preserved as `Result<(), JoinError>`, but body is decoupled from
  `verify_phase_2` in B3. `JoinError` loses the
  `Phase2VerificationFailed` variant.
- `verify_phase_2`
  (`crates/regseq-model/src/verify/phase_2.rs`) -- promoted to a
  public standalone function `(&RegseqModel) ->
  Result<VerifyReport, VerifierError>`, symmetric with
  `verify_phase_1`.
- `MutationGuard::commit`
  (`crates/regseq-model/src/model/mutation.rs`) -- no longer
  auto-invokes `verify_phase_2`. May be retained for non-verification
  duties or removed entirely depending on B3's findings.

## Open Questions For Review

1. Should `populate_bindings` continue to drop the Phase-2
   `VerifyReport`, or should the live compiler pipeline preserve
   and log lints?
2. What is the preferred fatal diagnostic mechanism inside
   `regseq-driver` callbacks?
3. Should the first live full-model test use a narrow synthetic
   fixture or PL011? A narrow fixture reduces body-coverage risk;
   PL011 proves real fixture value sooner. (Same question appears
   in `model-refactor.md`; the decision drives Step B2 there and
   Step A7 here.)

## Recommended Initial Scope

The first implementation focuses on a narrow full-model slice:

- model emit only,
- no backend connection,
- no analysis connection,
- one platform-backed fixture,
- real Layer-2 fields in JSON,
- strict failures for platform parse/join errors,
- no broad renaming until behavior is stable.

This aligns the live compiler path with the model spec while keeping
the blast radius limited.

## Codex Review Loop

Each step in the execution plan has a Codex review gate. The gate
runs AFTER the step's tests are green locally and BEFORE starting
the next step. Codex-raised concerns are either (a) addressed within
the same step and re-reviewed, or (b) filed as follow-up tasks
tagged on the step that introduced them. No step crosses its gate
with open Codex blockers. Wave B as a whole also gets an
integration-pass Codex review before the live callback is switched
over.
