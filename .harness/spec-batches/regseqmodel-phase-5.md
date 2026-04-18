# Spec Batch: regseqmodel-phase-5 - MIR-to-Regseq-IR construction pass

## Description

This batch authors executable harness task specs for the two-pass IR
construction that transforms extraction state (in-memory) into a
populated `RegseqModel` with CFG-based Regseq IR function bodies.

This is the hardest phase. It includes Pass 1 (declaration skeleton),
Pass 2 (body construction), 7 recognition rules, cross-crate function
extraction, and `BodySourceMap` population, implemented incrementally
against PL011 fixture functions of increasing complexity.

**Architecture.** Construction runs inside the `regseq-driver`
compiler wrapper in the same rustc invocation as extraction.
`RegseqModel` types live in `regseq-model` (unchanged from Phases
0-2). Construction logic lives in
`crates/regseq-driver/src/construction.rs` (new file), consumes
live `TyCtxt` + the in-memory extraction attribute registries
(intrinsic / framework_types / abstraction_types / pin_state),
and produces a `RegseqModel` directly.

`RawFrontendArtifact` / `raw-frontend.json` and the owned `Mir*`
types in `crates/regseq-model/src/declaration/mir_body.rs` are
**legacy scaffolding** on their way out; Phase 9 deletes them.
Phase 5 does NOT consume the raw artifact as an input and does
NOT build its IR from the legacy `Mir*` owned types. Construction
walks rustc state directly via `tcx.optimized_mir(def_id)`,
`tcx.type_of(def_id)`, `tcx.get_attrs(def_id, _)`, and peer
queries.

The output of this phase is a fully built `RegseqModel` with all
declaration-side fields populated, all function bodies attached
as Regseq IR, all `source_locations` resolved, and every binding
slot left as `None`. Pipeline wiring (`normalize()`, platform
join, verifier invocation) is Phase 6 and must not bleed into
Phase 5 tasks.

## Input Context

Read before starting:

- `.harness/config.md`
- `.harness/SESSION.md`
- `.harness/task-spec-status.yaml`
- `.harness/spec-batches/BRIEF_TEMPLATE.md`
- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 5,
  including 5.1-5.6 and the Gaps+Risks rows for `?`-pattern correctness
  and cross-crate attribute visibility)
- `docs/discussions/models/regseq-model/ir-reference.md` (Sections 1-8;
  Section 5 contains the `?`-pattern recognition algorithm; Section 6
  defines `BodySourceMap`/`OrderedRegion`; Section 8 is the
  construction pass spec)
- `docs/discussions/models/regseq-model/spec.md` (Section 7 identity
  scheme, Section 8 type construction from MIR, Section 9 body
  ownership, Section 10 intrinsic recognition mechanism)
- `crates/regseq-model/src/ir/` (Phase 0 + 1 leaf and CFG types)
- `crates/regseq-model/src/model/` (Phase 2 model types)
- `crates/regseq-model/src/identity/` (Phase 0 identity types)
- `crates/regseq-driver/src/callbacks.rs` (the rustc callback that
  already runs extraction -- Phase 5 construction plugs in here)
- `crates/regseq-driver/src/extract/` (Phase 4 extraction surfaces:
  `intrinsic_registry`, `framework_types`, `abstraction_types`,
  `identity_enrichment`, `source_locations`, `body`, `pinctrl`,
  `mir_extract`, etc. -- all already in-memory during construction)
- `tests/fixtures/pl011-full-spec/src/driver.rs` (the 9 PL011 test
  targets construction validates against)

`RawFrontendArtifact` / `raw-frontend.json` / the owned `Mir*`
types are legacy scaffolding Phase 9 deletes; Phase 5 ignores
them and works directly off rustc state.

## Output Specification

Create:

- `.harness/tasks/regseq-model/` task spec files (one per recognition
  rule or complexity tier; see Implementation Notes)

Update when done:

- `.harness/task-spec-status.yaml` (mark `regseqmodel-phase-5` tasked,
  populate `generated_task_ids`, advance `regseqmodel-phase-6` to
  `ready_to_elaborate`)
- `.harness/tasks.json` (one entry per new task spec)

## Implementation Notes

- New file: `crates/regseq-driver/src/construction.rs`. Construction
  runs in-process with `TyCtxt` access. Only the in-memory
  attribute registries (`IntrinsicRegistry`, `FrameworkTypes`,
  `AbstractionTypes`, pin-state attribute table) built during
  extraction are passed in alongside `tcx`. No deserialization,
  no serialized-def-path string matching, no dependence on the
  legacy `Mir*` owned types or `RawFrontendArtifact` -- live
  `DefId`s, live `ty::Ty<'tcx>`, and rustc MIR queries
  throughout.
- **Pass 1 (declaration skeleton)** must complete BEFORE any body is
  processed: assign every `DeclId` (Module/Constant/Function/TypeDef/
  RegisterDescriptor/RegisterBlock/Resource/Property/UserData), record
  type classifications (`UserDefined` / `Abstraction` / `Framework`),
  intern every name through the model's `SymbolInterner`, and populate
  the model-level `source_locations: BTreeMap<DeclId, SourceOrigin>`
  by resolving rustc spans directly via `tcx.sess.source_map()`. Walk
  the module tree from the crate root using HIR/`tcx` queries;
  identity_enrichment data remains available in-memory for identity
  disambiguation where HIR alone is insufficient.
- **Pass 2 (body construction)** for each function discovered in
  Pass 1:
  - Obtain the monomorphized body via
    `tcx.optimized_mir(def_id)`. Map rustc `mir::LocalDecl<'tcx>`
    -> regseq-model `LocalDecl` with mapped `RegseqType` and
    interned `Symbol` name (derived from MIR `var_debug_info`).
  - Translate statements/terminators using the per-construct mappings
    in ir-reference.md Sections 1-5.
  - Drop MIR cleanup blocks (`is_cleanup` blocks ending in `resume`)
    -- they have no Regseq IR representation.
  - Build a structurally-parallel `BodySourceMap` per ir-reference.md
    Section 6 and Section 7. Use `Source(SourceLocation)` for nodes
    attributable to user spans (including Drop terminators inserted
    by Rust's drop elaboration) and `CompilerGenerated` only for
    truly synthesized nodes.
  - Attach the `Body` to the owning `Function` -- bodies do not live
    in a global collection (spec.md Section 9).
- **Recognition rules** (apply in the order below per terminator;
  ir-reference.md Sections 5 and 8 are authoritative):
  1. **Intrinsic recognition.** Callee `DefId` lookup in the Phase 4
     in-memory intrinsic registry (`HashMap<DefId, String>`) produces
     `IntrinsicCall`. Method intrinsics preserve `&self` as `args[0]`
     -- do not lift the receiver into a dedicated field.
  2. **`?`-pattern folding.** Match the MIR pattern in
     ir-reference.md Section 5 by `DefId` against
     `<T as Try>::branch` and `<T as FromResidual>::from_residual`
     (any `Try` implementor -- resolved via `tcx` trait method
     `DefId`s, not by the implementing type's name or string
     prefix).
     Validate the SwitchInt has exactly Continue (0), Break (1), and
     `otherwise: unreachable`; retarget `destination` to the unwrapped
     Continue value, set `target` to the post-Continue block, set
     `error_target` to the post-`from_residual` block. **Eliminate**
     the Try-wrapper temporary, the ControlFlow temporary, the
     `Try::branch` Call, the `SwitchInt`, the Continue extraction
     statement, the `FromResidual` Call, and the residual temporaries.
     **Fallback rule:** if the pattern does not match at all, treat
     as a regular Call with `error_target: None`. **Hard error:** if
     `Try::branch` is present but the SwitchInt or `FromResidual`
     shape is structurally invalid, emit a hard error and stop.
     Preserve the Drop cascade Rust inserts on the Break path -- it
     is the cleanup mechanism, not extra noise.
  3. **Core library function extraction.** `Option::map`,
     `Option::unwrap_or`, `<FieldValue as Add>::add`, etc. are
     extracted as regular `Function`s with bodies. Their
     monomorphized MIR is available via `tcx.optimized_mir(def_id)`
     for cross-crate instances and must be visited during Pass 1
     ID assignment so call references resolve.
  4. **Deref collapse.** `<Mmio<T> as Deref>::deref` Call resolves
     to transparent access to the inner block. With register
     intrinsics on `Mmio<T>` directly, this is rare except inside
     `modify()`'s body -- preserve Mmio receiver identity for
     downstream register-metadata resolution.
  5. **OrderedRegion detection.** Recognize paired
     `regseq::internal::ordered_region_begin()` /
     `ordered_region_end()` markers by `DefId` (resolved once
     at construction setup from the facade crate), drop both
     calls, and record `OrderedRegion { entry, exit }` as
     **body-level metadata** on `Body.ordered_regions` (NOT a
     nested statement). Region membership is reachability from
     `entry` without passing through `exit` (SESE invariant per
     ir-reference.md Section 6).
  6. **Logging metadata extraction.** For logging intrinsic calls,
     extract the template string from the MIR constant arg, parse
     the restricted format subset into `Vec<FormatPlaceholder>`,
     strip `LogArg` carriers, and store
     `IntrinsicMetadata::Log { template, placeholders }`. Argument
     values become plain `Operand`s in `args`.
  7. **Register metadata resolution.** For register intrinsics on
     `Mmio<T>`, resolve `Block` from receiver type, `R` (register
     marker) from the type parameter, `R::Access` for access kind,
     and the field argument for mask/shift; store the resolved
     `IntrinsicMetadata::Register { register_block, register_offset,
     access_width, field }`. Type parameters do not survive into the
     IR -- this metadata is the resolved output.
- All names use the model's `SymbolInterner` -- no raw `String`s
  remain in the constructed model.
- Forward references resolve by ID lookup, not by construction order
  (Pass 1 assigns IDs first; Pass 2 may run in any order).
- **Incremental implementation order** (9 PL011 test targets):
  1. Straight-line code -- `disable_uart()`
  2. SwitchInt for if/else -- `tx_ready()`, `send_break()`
  3. `?`-pattern folding -- `enable_clock_if_present()`
  4. Loop detection (back-edge Goto) -- `flush_rx_fifo()`
  5. OrderedRegion detection -- `program_polling_port()`
  6. Logging metadata extraction -- (add a logging call to the
     fixture if none exercises it)
  7. Register metadata resolution -- all register access functions
  8. `modify()` body extraction -- `disable_fifo()`,
     `configure_line_control()` (verify two `IntrinsicCall`
     terminators in the extracted `Mmio::modify` body)
  9. Volatile read-to-dead-temp -- `flush_rx_fifo()`
     (`let _ = self.regs.read(dr::REG)` produces a `RegisterRead`
     that downstream backends MUST not elide)
- Decompose into one task spec per recognition rule (or complexity
  tier where rules collapse together). Each task carries a tightly
  scoped test plan against the listed PL011 function and must
  reference the relevant ir-reference.md section.
- **Phase boundary.** Phase 5 ends with a `RegseqModel` fully built
  by `construction.rs` (declarations + bodies + source map +
  ordered regions + module abstraction/intrinsic sets), all binding
  slots left `None`, and a `RegseqModel` JSON golden under the PL011
  fixture. **Do not** wire `normalize()`, the platform parser, or
  the legality verifier in Phase 5 tasks; that work is Phase 6.

## Non-Goals

- No `RawFrontendArtifact` consumer logic -- construction is
  in-process with rustc, not JSON-driven.
- No normalization pipeline wiring (`normalize()`, `populate_bindings()`),
  legality verifier, or pipeline glue -- Phase 6.
- No backend changes.
- No consumer migration.
- No closure support in the IR (deferred until needed).
- No synchronization-intrinsic recognition (deferred until a
  sync-using driver is ported).

## Acceptance Criteria

- One task spec per recognition rule or PL011 complexity tier as
  listed above, each with a fixture function or hand-built MIR
  specimen as its test target.
- For each PL011 function in the incremental list, the constructed
  IR is verified by a structural assertion (block count, terminator
  shape, `IntrinsicMetadata` contents) AND by a serialized JSON
  golden artifact under
  `tests/fixtures/pl011-full-spec/expected/ir/`. The golden
  boundary is `RegseqModel`, not `RawFrontendArtifact`.
- `?`-pattern folding produces no `Try::branch`, no `FromResidual`,
  no `ControlFlow` temporaries in the resulting IR.
- `Mmio::modify` body extracted from monomorphized core/facade MIR
  contains exactly two `IntrinsicCall` terminators (`mmio_read` then
  `mmio_write`).
- `OrderedRegion` is recorded on `Body.ordered_regions` and the
  marker calls are absent from the body's blocks.
- A volatile read-to-dead-temp produces a `RegisterRead`
  `IntrinsicCall` whose destination is a `LocalDecl` with no
  subsequent use; the IR retains it.
- Cross-crate function extraction validated: the
  monomorphized MIR for `Option::map`, `Option::unwrap_or`, and
  `<FieldValue as Add>::add` becomes regular `Function`s with
  bodies attached to their owning module.
- Negative cases: a malformed `?` desugaring (Try::branch present,
  SwitchInt shape wrong) produces a hard error; an unknown intrinsic
  callee falls through to a regular `Call`; a `std` type encountered
  during construction is a hard error.
- Tracker updated; `regseqmodel-phase-6` advances to
  `ready_to_elaborate`.

## Update Instructions

1. Change `regseqmodel-phase-5` status to `tasked`.
2. Fill `generated_task_ids`.
3. Move `regseqmodel-phase-6` to `ready_to_elaborate`.
