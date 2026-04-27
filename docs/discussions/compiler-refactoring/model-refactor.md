# Regseq Model Construction Refactor

## Purpose

This document defines the refactor plan for the `regseq-driver`
construction layer so every registered `Function` in a Layer-1
`RegseqModel` carries a real monomorphic body. It replaces the
current `DefId`-keyed extraction with `ty::Instance`-keyed
extraction, eliminating the placeholder-body problem that blocks
strict Phase-1 verification.

This document is scoped to the construction layer: how rustc HIR/MIR
is extracted into a Layer-1 `RegseqModel`. Pipeline orchestration
(callback, platform file, emit) is covered in `pipeline-refactor.md`.
The pipeline refactor depends on this one: without real bodies for
every registered function, strict Phase-1 validation cannot run in
the live path.

## Summary Finding

Extraction is `DefId`-keyed, not `Instance`-keyed.

`construction/pass2.rs:119` calls `tcx.optimized_mir(def_id)`, which
returns generic MIR for generic functions (containing
`ty::TyKind::Param`). `cross_crate.rs:75-208` registers callees by
`DefId` and discards the generic substitutions at call sites.

As a result, cross-crate generic bodies (`Option::<T>::as_ref`,
`Mmio::<T>::modify`) and local generic helpers (`field_val<R>`)
cannot be translated into Regseq's monomorphic IR (spec Section 8).
The test harness `pl011_construction.rs` documents this at
line 442-454:

> Generic cross-crate bodies still carry `ty::TyKind::Param` in
> their MIR; monomorphizing them would be a new construction rule
> outside scope.

The production Layer-1 path therefore cannot rely on
`Function.body: Body` being populated uniformly. The conservative
alternatives (silent skip and leave placeholders, represent as
external declarations, or relax the verifier) all break the spec
invariant that every `Function` in the model is a fully monomorphic,
body-carrying entity.

Moving to instance-keyed extraction dissolves the body-policy
question: every registered function resolves to a concrete body,
so `Function.body: Body` stays mandatory and `verify_phase_1` needs
no relaxation.

## Decisions

- **Monomorphization:** shift to instance-keyed extraction.
  `FunctionId` becomes 1:1 with `ty::Instance<'tcx>`. Pass 2 calls
  `instance_mir`; call-site substitutions are resolved via
  `Instance::try_resolve` in `TypingEnv::fully_monomorphized`.
  Supersedes any `Option<Body>` proposal -- with monomorphization,
  every registered function resolves to a concrete body, so
  `Function.body: Body` stays mandatory and `verify_phase_1` needs
  no relaxation.

- **Model semantics:** the `RegseqModel` stores monomorphic function
  instances, not source-level generic definitions. A single source
  function `Option::<T>::map` that is called with `T = u32` and
  `T = u64` produces two distinct `Function` entries in the model,
  each with its own `FunctionId`, its own `name` + `mono_args`
  symbols, and its own monomorphic `Body`. The model does not
  retain the generic source definition independently -- generics
  exist only as rustc-side `DefId`s consulted during extraction.
  Consumers of `RegseqModel` (analyses, backends, serializer) never
  see `TyKind::Param`.

- **Deterministic ordering:**
  - `InstanceInterner` iteration order is insertion order (via
    `IndexVec<FunctionId, ty::Instance<'tcx>>`). Insertion order
    is the order in which the Pass-1 worklist observes each unseen
    instance.
  - Worklist ordering is deterministic: the seed set is sorted by
    `tcx.def_path_str(def_id)` before enqueue; at each
    call-terminator walk, newly discovered instances are enqueued
    in the order they appear in the MIR body (MIR block index,
    then terminator position). This reproduces the same
    `FunctionId` sequence across runs given the same input crate.
  - `iter_instances` and any JSON emission traversal follow
    `FunctionId` order. No `HashMap` iteration leaks into output.

- **Body-bearing policy (explicit catalog).** Model `Function`
  entries are created for, and only for, the following resolutions
  of a call terminator through `Instance::try_resolve`:
  - `Ok(Some(Instance { def: InstanceKind::Item(def_id), args }))`
    where `def_id` is NOT marked `#[rseq::intrinsic]` -> a
    `Function` with a real monomorphic `Body`.
  - `Ok(Some(Instance { def: InstanceKind::Item(def_id), .. }))`
    where `def_id` IS marked `#[rseq::intrinsic]` -> NOT a
    `Function`; the call terminator lowers to
    `TerminatorKind::IntrinsicCall` via `IntrinsicLookup` (Rule 1
    in `pass2.rs`). The intrinsic's body is never walked.
  - `Ok(Some(Instance { def: InstanceKind::Intrinsic(_), .. }))`
    -> rustc compiler intrinsic; rejected with
    `ConstructionError::UnsupportedInstanceKind`. Spec forbids.
  - `Ok(Some(Instance { def: InstanceKind::Virtual(..), .. }))` ->
    `dyn Trait` dispatch; rejected. Spec forbids.
  - `Ok(Some(Instance { def: InstanceKind::DropGlue(..) |
    FnPtrShim(..) | ReifyShim(..) | ClosureOnceShim(..) |
    CloneShim(..) | FnPtrAddrShim(..) | ThreadLocalShim(..), .. }))`
    -> rejected with `UnsupportedInstanceKind`. Not representable.
  - `Ok(None)` -> unresolvable generic call (open generic or
    trait-object-shaped); rejected with `UnresolvableGenericCall`.
  - `Err(ErrorGuaranteed)` -> rejected with
    `InstanceResolutionFailed`.
  - Trait method stubs without bodies (for example,
    `regseq::serial::Serial::rx_char` as declared on the trait)
    are never reached as call targets: static dispatch resolves
    the call to the impl's method (local, has MIR). If
    `try_resolve` ever yields a trait-declaration `Instance::Item`
    without a body, that is an `InstanceResolutionFailed` case.
  - Closures: `AggregateKind::Closure` rejection in
    `mir_mapping.rs:861-867` stands; call sites that would resolve
    to closure bodies hit `Instance::resolve_closure` -> rejected
    as `UnsupportedInstanceKind` until the closure wave lands.

- **First fixture for live pipeline test:** narrow synthetic
  (`tests/fixtures/narrow-full-spec/`, specified in
  `pipeline-refactor.md`). PL011 cross-crate body coverage is
  proven as part of Wave A's acceptance (via the existing
  `pl011_construction.rs` test ported onto the instance-keyed
  path); Layer-2 assertions on PL011 can land separately if narrow
  synthetic already gates the orchestration change.

- **Closures:** out of scope for this refactor.
  `AggregateKind::Closure` stays a rejection in
  `mir_mapping.rs:861-867`. The narrow synthetic fixture must
  avoid closures; PL011 currently does not use closures
  (register-poll intrinsics are deferred per spec Section 10.2).
  Closure body translation is a separate follow-up.

## Current State By Subphase

### Layer1DeclarationSubphase

Current status: implemented and live for model emit.

Relevant code:

- `crates/regseq-driver/src/construction.rs`
- `build_declaration_skeleton`
- `build_pass1_output`
- `CrossRefMaps`
- construction submodules under
  `crates/regseq-driver/src/construction/`

What it captures today:

- Module tree.
- Driver module identification.
- Driver struct metadata and identity.
- Type definitions and classifications.
- Register descriptors.
- Register blocks.
- Constants and statics.
- Impl blocks and trait impls.
- Resource declarations:
  - clocks,
  - resets,
  - pinctrl,
  - interrupts,
  - MMIO,
  - sync resources,
  - properties,
  - user data.
- Source locations.
- Cross-reference maps:
  - rustc `DefId` -> model `TypeDefId`,
  - rustc `DefId` -> model `RegisterBlockId`,
  - rustc `DefId` -> model `FunctionId` (the one Wave A replaces),
  - rustc `DefId` -> model `RegisterDescriptorId`,
  - rustc `DefId` -> model `ConstantId`.

What it intentionally does not do:

- It does not populate platform bindings.
- It does not translate all MIR bodies.
- `build_declaration_skeleton` discards the cross-reference maps
  and returns only the model.
- Function bodies are placeholder bodies.

Current gap:

- The live callback uses `build_declaration_skeleton`, but the
  production Layer-1 pipeline needs `build_pass1_output` so later
  body construction can use the cross-reference maps.
- The declaration output alone is not a verifier-clean Layer-1
  model if function body completeness is required.

Needed changes:

- Keep `build_pass1_output` as the declaration subphase entry.
- Add a production wrapper (`build_layer1_model`, below) that
  consumes `Pass1Output` and completes Layer-1 construction.
- Treat `build_declaration_skeleton` as a test/helper or legacy
  skeleton entry, not the live model emit entry.
- Rekey the function map from `DefId` to `ty::Instance` via an
  `InstanceInterner` (Wave A, step A2).

### Layer1BodySubphase

Current status: functionality implemented, not live-orchestrated.

Relevant code:

- `crates/regseq-driver/src/construction/pass2.rs`
- `build_body`
- `TypeDefIndex`
- `ConstantIndex`
- `FunctionIndex`
- `IntrinsicLookup`
- `DerefCollapseLookup`
- `OrderedMarkers`
- `TryMarkers`
- `LogArgUnwrap`
- `RegisterIndex`
- `PinStateLookup`

What it captures today:

- MIR local declarations.
- Basic blocks.
- Statements.
- Terminators.
- Calls and intrinsic calls.
- Register intrinsic metadata.
- Pinctrl intrinsic metadata.
- Logging metadata.
- `?` pattern folding.
- Deref collapse for transport abstractions.
- Ordered regions.
- Source maps for body components.

Current evidence:

- Many tests call `build_body` directly.
- `pl011_construction.rs` contains a test-local `run_pipeline`
  that runs Pass 1, builds bodies for many functions, attaches
  successful bodies, and derives dependencies.
- `construction_pass2.rs` manually attaches a translated body
  back into a cloned model.

Current gap:

- There is no production `build_layer1_model` function that
  translates and attaches bodies across the whole model.
- Untranslatable bodies (cross-crate generics) cannot be resolved
  in production. Tests sometimes skip failures and leave
  placeholders.
- `verify_phase_1` rejects placeholder bodies, so body
  construction must be made sufficiently complete before strict
  Phase-1 validation can run in the live path.

Needed changes:

- Add a production body orchestration function
  (`build_layer1_model`).
- Iterate over `pass1.cross_refs.instances` (Wave A).
- For each instance, call `build_body` with the instance
  parameter; attach the resulting `Body` to the matching
  `Function`.
- With monomorphization in place, every call resolution yields
  either a concrete body-bearing function or an explicit
  rejection. No silent placeholders.

### Layer1DependencyDerivationSubphase

Current status: implemented, not live-orchestrated.

Relevant code:

- `crates/regseq-driver/src/construction/module_deps.rs`
- `derive_module_dependencies`

What it captures today:

- Per-module `abstractions_used`.
- Per-module `intrinsics_used`.

What these fields mean:

- `abstractions_used` summarizes Regseq abstraction families used
  by the module, such as MMIO, clock, reset, interrupt, pinctrl,
  and sync primitives.
- `intrinsics_used` summarizes Regseq intrinsic calls found in
  translated bodies.

Why it exists:

- It gives analyses and backends a deterministic per-module
  summary.
- It prevents every downstream consumer from rescanning all
  declarations and bodies to discover basic dependency facts.

Current gap:

- It is only accurate after bodies are populated.
- The live model emit path does not call it.

Needed changes:

- Run `derive_module_dependencies(&mut model)` after body
  attachment and before `verify_phase_1`.
- Treat it as the terminal step of Layer-1 construction.

## Proposed Production Entrypoint

### `construction::build_layer1_model`

Add the production Layer-1 construction entrypoint:

```rust
pub fn build_layer1_model<'tcx>(
    tcx: TyCtxt<'tcx>,
    registries: &ExtractionRegistries<'_>,
) -> Result<RegseqModel, ConstructionError>
```

Responsibilities:

1. Call `build_pass1_output`.
2. Build Pass-2 helper lookups:
   - `IntrinsicLookup`,
   - `DerefCollapseLookup`,
   - `OrderedMarkers`,
   - `TryMarkers`,
   - `LogArgUnwrap`,
   - `TypeDefIndex`,
   - `ConstantIndex`,
   - `FunctionIndex`.
3. Iterate `(function_id, instance)` from
   `pass1.cross_refs.instances.iter_instances()`.
4. For each, call `build_body(tcx, instance, function_id, ...)`
   and attach the returned `Body` to the matching `Function` via a
   local `attach_body(&mut model, function_id, body)` helper. On
   Err: fatal (Wave A removes the silent-skip path).
5. Run `derive_module_dependencies(&mut model)`.
6. Return a Layer-1 `RegseqModel`.

This function does not parse platform files and does not emit JSON.
It owns only HIR/MIR-to-Layer-1 construction.

## Execution Plan: Wave A -- Instance-Based Extraction

Self-contained within `regseq-driver` construction plus one
`regseq-model` field. Lands before any orchestration work.
Acceptance is preservation of existing construction behavior plus
new coverage of previously-skipped generics.

### Step A1 -- Model schema change

Scope: add `Function.mono_args: Option<Symbol>` to carry rendered
generic args for diagnostic disambiguation.

Files to modify:

- `crates/regseq-model/src/model/function.rs` -- add the field;
  update doc comment that claims "functions are only extracted if
  they have MIR bodies" to match the new invariant (per-instance
  bodies, mandatory).
- `crates/regseq-model/src/serialize.rs` -- serialize `mono_args`
  when `Some`.
- Every `Function {...}` constructor in
  `crates/regseq-model/tests/`,
  `crates/regseq-driver/tests/support/`, and
  `crates/regseq-driver/src/construction/` -- initialize
  `mono_args: None` for now.

Acceptance: `cargo test -p regseq-model -p regseq-driver` green.

Codex review gate.

### Step A2 -- `InstanceInterner` replaces `CrossRefMaps.function_map`

Scope: introduce instance-keyed identity. Pass 1 registrations now
go through an interner keyed by
`(DefId, ty::GenericArgsRef<'tcx>)`; `FunctionId` minting stays
unchanged.

Files to create:

- `crates/regseq-driver/src/construction/instance_interner.rs` --
  new module with:

  ```rust
  InstanceInterner<'tcx> {
      by_instance: HashMap<
          (DefId, ty::GenericArgsRef<'tcx>),
          FunctionId,
      >,
      by_function: IndexVec<FunctionId, ty::Instance<'tcx>>,
  }
  ```

  and helpers `intern(&mut self, Instance) -> FunctionId`,
  `lookup(&self, Instance) -> Option<FunctionId>`,
  `iter_instances(&self) -> impl Iterator<
  Item = (FunctionId, &ty::Instance<'tcx>)>`.
  `iter_instances` yields in `FunctionId` order (`IndexVec`
  iteration), i.e., insertion order -- the deterministic ordering
  contract.

Files to modify:

- `crates/regseq-driver/src/construction.rs` --
  `CrossRefMaps.functions` becomes
  `CrossRefMaps.instances: InstanceInterner<'tcx>`; threaded
  through.
- `crates/regseq-driver/src/construction/walk.rs:52` -- replace
  `function_map: HashMap<DefId, FunctionId>` with
  `instance_interner: InstanceInterner<'tcx>`.
- `crates/regseq-driver/src/construction/mir_mapping.rs:78-91` --
  `FunctionIndex` wraps the interner; `lookup(Instance)` replaces
  `lookup(DefId)`.

Acceptance: behavior preserved -- every call site that previously
used `function_map` now calls through the interner with
`Instance::mono(tcx, def_id)` for the non-generic hot path.
Construction tests still green with no semantic change.

Codex review gate.

### Step A3 -- Pass 1 seeding becomes instance-keyed

Scope: seed the worklist from local authored items as
`Instance::mono(...)`; generic local helpers are not seeded
directly, they enter only via call-site instantiation.

Files to modify:

- `crates/regseq-driver/src/construction/walk.rs:718-724` --
  `build_function_stub` interns `Instance::mono(tcx, def_id)` when
  `tcx.generics_of(def_id).count() == 0`, rejects (or skips at seed
  stage) generic items. ISR handlers and Regseq trait impl methods
  must remain non-generic per spec.
- `crates/regseq-driver/src/construction/cross_crate.rs:75-208` --
  Rule-3 cross-crate walker transitions from DefId enqueue to
  Instance enqueue. Caller-side substitution applied via
  `EarlyBinder::bind(callee_generic_args).instantiate(tcx,
  caller_instance.args)` before `Instance::try_resolve`. Worklist
  enqueue order inside a caller body is MIR block index, then
  terminator position within the block -- matches MIR's natural
  traversal and is deterministic. Seed list is sorted by
  `tcx.def_path_str(def_id)` before enqueue.
- `crates/regseq-driver/src/construction/cross_crate.rs:431` --
  swap `TypingEnv::non_body_analysis(...)` to
  `TypingEnv::fully_monomorphized()` (the seed is now fully
  concrete).
- `crates/regseq-driver/src/construction/cross_crate_types.rs:83` --
  same `optimized_mir` -> `instance_mir` swap; iterate over
  instances.

Acceptance: PL011 construction test passes; `run_pipeline` in
`tests/pl011_construction.rs:402-461` no longer skips
`Mmio::modify`, `Option::as_ref`, etc. -- they now appear as
distinct `Function` entries with real bodies.

Codex review gate.

### Step A4 -- Pass 2 queries `instance_mir` + substitutes types

Scope: `build_body` takes an `ty::Instance<'tcx>` parameter;
queries monomorphic MIR; substitutes `TyKind::Param` /
`TyKind::Alias` away at every type boundary.

Files to modify:

- `crates/regseq-driver/src/construction/pass2.rs:95-119` --
  signature gains `instance: ty::Instance<'tcx>`;
  `let mir_body = tcx.instance_mir(instance.def);`.
- `crates/regseq-driver/src/construction/pass2.rs:142`
  (`build_local_decls`) -- thread `instance.args` +
  `TypingEnv::fully_monomorphized()` into `LocalIdMap`. Local
  types are substituted via
  `tcx.instantiate_and_normalize_erasing_regions(instance.args,
  env, local_decls[i].ty)` before `map_ty`.
- `crates/regseq-driver/src/construction/mir_mapping.rs` --
  `map_ty` gains a debug-assert that rejects `TyKind::Param` (a
  leak = substitution bug); `LocalIdMap` gains a
  `local_ty_normalized(local)` helper.
- `crates/regseq-driver/src/construction/pass2.rs:1131` --
  `resolve_callee_def_id` -> `resolve_callee_instance`: pulls
  `(def_id, args)` from `TyKind::FnDef`, substitutes against
  caller's instance, calls `Instance::try_resolve(tcx,
  TypingEnv::fully_monomorphized(), def_id, resolved_args)`,
  returns the resolved `Instance<'tcx>`. Returns
  `ConstructionError::UnsupportedInstanceKind` for any
  non-`InstanceKind::Item` resolution (FnPtrShim, Virtual,
  ReifyShim, DropGlue, etc.);
  `ConstructionError::UnresolvableGenericCall` for `Ok(None)`;
  `ConstructionError::InstanceResolutionFailed` for `Err(_)`.
- `crates/regseq-driver/src/construction/pass2.rs:929-935` --
  lookup takes the resolved `Instance`, not a bare `DefId`.

Files to modify (new error variants):

- `crates/regseq-driver/src/construction/error.rs` (or wherever
  `ConstructionError` is defined) -- add
  `UnsupportedInstanceKind`, `UnresolvableGenericCall`,
  `InstanceResolutionFailed`, and (cycle detection) `Recursion`.

Acceptance:

- PL011 construction test passes including `Mmio::modify`,
  `Option::as_ref`, `field_val<R>` -- all produce `Function`
  entries with real bodies.
- New negative test: fixture that calls through a `dyn Trait`
  receiver -> `UnsupportedInstanceKind { kind: Virtual, .. }`.
- New negative test: fixture with a function-pointer indirect call
  -> `UnresolvedCallTarget` (unchanged behavior) or
  `UnsupportedInstanceKind`.

Codex review gate.

### Step A5 -- Name disambiguation

Scope: populate `Function.mono_args` for generic instantiations so
diagnostics distinguish `Option<u32>::map` from
`Option<u64>::map`.

Files to modify:

- `crates/regseq-driver/src/construction/walk.rs` and
  `crates/regseq-driver/src/construction/cross_crate.rs` -- at
  intern time, render the generic args via a new helper

  ```rust
  fn render_mono_args<'tcx>(
      tcx: TyCtxt<'tcx>,
      args: ty::GenericArgsRef<'tcx>,
  ) -> Option<Symbol>
  ```

  that keeps only `GenericArgKind::Type` / `GenericArgKind::Const`,
  formats `<T1, T2, ...>` via `ty::Ty::Display` / `ty::Const::Display`,
  and interns. Empty -> `None`.
- Symbol collision mitigation: for `TyKind::Adt` whose
  `DefId.krate != LOCAL_CRATE`, prepend the defining crate name,
  mirroring the canonicalization already in
  `cross_crate.rs:476-487`.

Acceptance: snapshot test over PL011 -- two distinct
monomorphizations of the same generic produce distinct `Function`
entries with distinct `mono_args` values.

Codex review gate.

### Step A6 -- Recursion guard

Scope: detect recursive call cycles (spec forbids but handle
cleanly).

Files to modify:

- `crates/regseq-driver/src/construction/cross_crate.rs` -- after
  worklist drains, walk the interned call graph; any strongly
  connected component of size > 1 or self-loop ->
  `ConstructionError::Recursion { cycle: Vec<FunctionId> }`. Keep
  the existing `MAX_CROSS_CRATE_DEPTH = 16` guard rail from
  `cross_crate.rs:49` as defense-in-depth.

Acceptance: negative test: recursive helper in a fixture ->
`ConstructionError::Recursion`.

Codex review gate.

### Step A7 -- Wave A regression sweep

Scope: run full workspace test matrix; fix any latent DefId-keyed
assumption.

Files to audit (non-exhaustive): every `HashMap<DefId, _>` in
`crates/regseq-driver/src/construction/`, every `function_map`
reader, every test helper in
`crates/regseq-driver/tests/support/`.

Acceptance: `cargo test --workspace` green. PL011 construction
test exercises generic bodies previously skipped.

Codex review gate. End of Wave A.

## Body Attachment Policy

Under `DefId`-keyed extraction the policy was an open question:
what to do when `tcx.optimized_mir(def_id)` returns generic MIR
with `TyKind::Param`? Options were (a) silently skip and leave
placeholders, (b) represent as external declarations, (c) relax
the verifier.

Under instance-keyed extraction the question dissolves:

- Local authored functions: a `FunctionId` exists only for a
  concrete `ty::Instance`. `instance_mir(instance.def)` returns
  the monomorphic body. Failure to build a body is a construction
  error.
- Cross-crate concrete instances: same. Every
  `Instance::try_resolve` success gives a concrete `DefId` + `args`
  with a queryable MIR body.
- Unresolvable and shim cases: rejected with explicit error
  variants per the body-bearing catalog above.

No silent placeholders. Placeholder behavior is documented in code
and tests; `verify_phase_1` outcome is predictable.

## Risks and Mitigations

1. **`Instance::try_resolve` failures on core trait methods.**
   Mitigation: log every `Ok(None)` / `Err` with caller + callee
   def-paths + args; unit-test against `Option::map`,
   `Result::map_err`, `Iterator::fold` with multiple scalar
   instantiations before PL011.
2. **Function-count explosion.** One generic can produce many
   instances. Mitigation: reuse `MAX_CROSS_CRATE_DEPTH` pattern
   with a parallel `MAX_INSTANCES` cap; surface a friendly
   diagnostic.
3. **Type-substitution leaks.** A missed substitution leaves
   `TyKind::Param` in `map_ty` inputs. Mitigation: debug-assert
   `TyKind::Param` rejection in `map_ty`; every
   `local_decls[i].ty` access goes through
   `LocalIdMap::local_ty_normalized`.
4. **Serialization bump from `mono_args`.** JSON consumers see a
   new optional field. Mitigation: serialize as omitted when
   `None`; document in `ir-reference.md` if that doc governs
   shape.
5. **PL011 regressions during Wave A.** The construction test is
   the canary; running it at the end of every Wave-A step keeps
   the blast radius visible.
6. **Rustc API drift on `TypingEnv::fully_monomorphized`.** The
   toolchain is pinned. If rustc moves the API, update at one
   site; do not add defensive shims.

## Verification

Extraction-level validation sequence once Wave A is in:

1. Unit / construction tests: `cargo test --workspace` green.
   PL011 extracts without placeholder bodies.
2. Wave A direct check:
   `cargo test -p regseq-driver --test pl011_construction` --
   assert all previously-skipped generics now carry real bodies
   and `verify_phase_1(&model)` returns `Ok(_)`.

## Out of Scope

- Closure body translation (extraction wave rejects
  `AggregateKind::Closure` via unchanged `mir_mapping.rs:861-867`).
  PL011 and the narrow synthetic fixture must not use closures;
  closures land in a follow-up alongside `poll::until` /
  `RegisterPoll`.
- Broad renaming (`build_declaration_skeleton` ->
  `build_layer1_declaration_skeleton`, etc.) -- follow-up cleanup
  once the refactor is merged.
- Specialization edge cases, drop-glue instantiations, reify
  shims, fn-pointer shims -- all rejected explicitly in the
  instance walker.
- Recursive generic bodies -- rejected.

## Critical Files

- `crates/regseq-driver/src/construction.rs` (A1-A7)
- `crates/regseq-driver/src/construction/walk.rs` (A2, A3, A5)
- `crates/regseq-driver/src/construction/cross_crate.rs` (A3, A5,
  A6)
- `crates/regseq-driver/src/construction/cross_crate_types.rs` (A3)
- `crates/regseq-driver/src/construction/pass2.rs` (A4)
- `crates/regseq-driver/src/construction/mir_mapping.rs` (A2, A4)
- `crates/regseq-driver/src/construction/instance_interner.rs` --
  new (A2)
- `crates/regseq-driver/src/construction/pipeline.rs` -- new
  (construction orchestrator; see `build_layer1_model` above)
- `crates/regseq-model/src/model/function.rs` (A1)
- `crates/regseq-model/src/serialize.rs` (A1)
- `crates/regseq-driver/tests/build_layer1_model.rs` -- new

## Reusable Anchors (unchanged call signatures or minimal changes)

- `build_pass1_output`
  (`crates/regseq-driver/src/construction.rs:1137`) -- internals
  rekey from DefId to Instance in Wave A; caller signature
  preserved.
- `build_body`
  (`crates/regseq-driver/src/construction/pass2.rs:95`) -- gains
  an `instance: ty::Instance<'tcx>` parameter; otherwise internals
  intact.
- `derive_module_dependencies`
  (`crates/regseq-driver/src/construction/module_deps.rs`) --
  unchanged.

## Open Questions For Review

1. Should cross-crate declarations without MIR be represented with
   a distinct model shape instead of rejection? Under the
   instance-keyed catalog above they are rejected; the alternative
   is an explicit "external declaration" function kind.
2. Should `verify_phase_1` require real bodies for all cross-crate
   instances (current plan) or only local/authored ones?
3. Should the first live full-model test use a narrow synthetic
   fixture or PL011? A narrow fixture reduces body-coverage risk;
   PL011 proves real fixture value sooner. (Same question appears
   in `pipeline-refactor.md`; the decision drives Step B2 there
   and Step A7 here.)

## Codex Review Loop

Each step has a Codex review gate. The gate runs AFTER the step's
tests are green locally and BEFORE starting the next step.
Codex-raised concerns are either (a) addressed within the same
step and re-reviewed, or (b) filed as follow-up tasks tagged on
the step that introduced them. No step crosses its gate with open
Codex blockers. Wave A as a whole also gets an integration-pass
Codex review before Wave B begins (in `pipeline-refactor.md`), to
catch cross-step regressions in the extraction rewrite.
