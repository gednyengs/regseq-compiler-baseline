# regseq-model-41: Construction Scaffold + Pass 1 Declaration Skeleton

## Description

Create `crates/regseq-driver/src/construction.rs` and implement
Pass 1 of the two-pass rustc-MIR-to-Regseq-IR construction
algorithm: walk the crate's module tree via rustc (HIR + `TyCtxt`
queries) and assign every declaration-level identifier before any
function body is processed.

Construction runs in-process with extraction in the same rustc
invocation. It consumes live `TyCtxt` plus the in-memory
attribute registries built by Phase 4 extraction (intrinsic
registry, framework-type registry, abstraction-type registry,
pin-state attribute table). It does NOT consume
`RawFrontendArtifact` or the legacy `Mir*` owned types in
`crates/regseq-model/src/declaration/mir_body.rs` -- both are
scaffolding Phase 9 deletes.

This task produces a `RegseqModel` with the full declaration
skeleton in place: every `ModuleId`, `ConstantId`, `FunctionId`,
`TypeDefId`, `RegisterDescriptorId`, `RegisterBlockId`,
`ResourceId`, `PropertyId`, and `UserDataId` is assigned and
resolvable. Function bodies are attached as empty `Body`
placeholders -- Pass 2 (task 43) fills them.

## Input Context

- `docs/discussions/models/regseq-model/spec.md` (Section 2 module
  tree, Section 3 module items, Section 4 driver struct, Section 7
  identity scheme, Section 10 abstraction/intrinsic catalogs)
- `docs/discussions/models/regseq-model/ir-reference.md`
  (Section 8 Pass 1 algorithm)
- `.harness/spec-batches/regseqmodel-phase-5.md`
- `crates/regseq-model/src/identity/` (DeclId, typed wrappers,
  Symbol, SymbolInterner)
- `crates/regseq-model/src/model/` (RegseqModel, Module, TypeDef,
  Function, RegisterDescriptor, RegisterBlock, Constant,
  DriverStruct, Resource/Property/UserData fields, SyncResource)
- `crates/regseq-model/src/ir/body.rs` (empty `Body` shape Pass 1
  attaches to each Function)
- `crates/regseq-driver/src/callbacks.rs` (the rustc callback
  where construction is invoked after extraction surfaces are
  built)
- `crates/regseq-driver/src/extract/intrinsic_registry.rs`
  (`IntrinsicRegistry` in-memory shape)
- `crates/regseq-driver/src/extract/framework_types.rs`
- `crates/regseq-driver/src/extract/abstraction_types.rs`
- `crates/regseq-driver/src/extract/pinctrl.rs` (pin-state attr
  table)
- `crates/regseq-driver/src/extract/source_locations.rs` (span
  resolution helpers if any are already in place; otherwise use
  `tcx.sess.source_map()` directly)
- `tests/fixtures/pl011-full-spec/src/driver.rs` (the fixture
  Pass 1 is validated against at compile time through a rustc
  harness)

## Implementation Notes

Create `crates/regseq-driver/src/construction.rs`. Expose it
from `crates/regseq-driver/src/lib.rs`.

### Public entry point

Two entry points land together. `build_declaration_skeleton` is the
compatibility signature callers use when they only need the
model; `build_pass1_output` additionally returns the DefId
cross-reference maps downstream passes (task 42+) consume to
resolve rustc `DefId`s to model IDs without re-walking the HIR.

```rust
pub fn build_declaration_skeleton<'tcx>(
    tcx: TyCtxt<'tcx>,
    registries: &ExtractionRegistries<'_>,
) -> Result<RegseqModel, ConstructionError>;

pub fn build_pass1_output<'tcx>(
    tcx: TyCtxt<'tcx>,
    registries: &ExtractionRegistries<'_>,
) -> Result<Pass1Output, ConstructionError>;

pub struct Pass1Output {
    pub model: RegseqModel,
    pub cross_refs: CrossRefMaps,
}

pub struct CrossRefMaps {
    pub type_defs: HashMap<DefId, TypeDefId>,
    pub register_blocks: HashMap<DefId, RegisterBlockId>,
    pub functions: HashMap<DefId, FunctionId>,
    pub register_descriptors: HashMap<DefId, RegisterDescriptorId>,
}
```

`AttrValue` is extended with two variants to carry payloads Pass 2
consumers (task 52 pin_state id, task 43 ISR metadata) rely on:

```rust
pub enum AttrValue {
    String(String),
    Bool(bool),
    Int(i64),             // NEW
    Unit,
    KeyValues(Vec<AttrEntry>),  // NEW -- keyword-arg maps
}
```

Pass 1 rejects unknown `#[rseq::<name>]` attribute names with
`ConstructionError::UnknownRseqAttribute`. The allowlist of
recognized short names lives in `construction/attrs.rs`
(`KNOWN_RSEQ_ATTRS`). Add to it when a new `#[rseq::...]` attribute
is introduced by the DSL.

`ExtractionRegistries` is a small borrow-only bundle (define in
this task) of the in-memory attribute/registry data already
produced by Phase 4 extraction:

```rust
pub struct ExtractionRegistries<'a> {
    pub intrinsic: &'a IntrinsicRegistry,
    pub framework_types: &'a FrameworkTypeRegistry,
    pub abstraction_types: &'a AbstractionTypeRegistry,
    pub pin_states: &'a PinStateRegistry,
}
```

Replace the exact type names if the existing extraction modules
already expose equivalent shapes; reuse them rather than
introduce new aliases.

`ConstructionError` is a new enum carrying provenance (rustc
`Span` + item path) for every failure case below.

### Pass 1 obligations

1. **Module tree.** Walk modules starting at the crate root via
   `tcx.hir()`. For each module assign a `ModuleId`, intern the
   module name through the model's `SymbolInterner`, and
   determine kind: `Driver` if the module contains the driver
   struct (located by scanning HIR for a `#[rseq::driver]`-marked
   ADT), `Normal` otherwise. Exactly one Driver module across the
   tree -- structural violation is a hard error
   (`ConstructionError::MultipleDriverStructs` /
   `NoDriverStruct`).
2. **Type definitions** (Section 3.4): for every struct/enum the
   crate defines (or references from `core`/facade -- see task
   47), assign `TypeDefId`, record fields/variants, resolve
   classification via `DefId` lookup in the registries:
   `Abstraction(RegseqAbstraction)` if the type's `DefId` is in
   `abstraction_types`, `Framework(FrameworkTypeKind)` if in
   `framework_types`, else `UserDefined`. Fill the `Attrs` bag
   from `tcx.get_attrs(def_id, sym::rseq)` (or the precise rustc
   attribute query equivalent).
3. **Constants** (Section 3.2 + 6): for every `const` / `static`,
   assign `ConstantId`; record name, `RegseqType` from
   `tcx.type_of(def_id)`, and `LiteralValue` from
   `tcx.const_eval_poly(def_id)` (or peer query).
4. **Register descriptors** (Section 3.5): recognize types
   produced by the `register_bitfields!`/`register_block!` macros
   (identified by marker traits or attribute annotations);
   assign `RegisterDescriptorId`; record name, access width,
   fields with bit offset/width and named values.
5. **Register blocks** (Section 3.6): for each `#[repr(C)]` layout
   struct produced by `register_block!`, assign
   `RegisterBlockId`; record block attributes (endian, access
   width), ordered fields with access kind, descriptor reference
   (`RegisterDescriptorId` resolved from the field's type), byte
   offset (from layout computation), array count for reserved
   gaps.
6. **Free functions** (Section 3.3), **impl-block methods**
   (Section 3.7, 6), **Regseq trait impl methods** (Section 5),
   and **ISR handlers** (Section 6 marked with `#[rseq::isr]`):
   assign `FunctionId`; record params with `ParamKind` (Receiver
   vs Regular), return type, and trait/ISR metadata in the
   `Attrs` bag (e.g., ISR `bind_target`, `has_index_param`,
   `context`). Each `Function.body: Body` field is initialized
   to a default-empty `Body` (single block with `Unreachable`
   terminator, or whatever placeholder convention
   `crates/regseq-model/src/model/function.rs` uses). Pass 2
   (task 43) replaces the placeholder body in place.
7. **Driver struct fields** (Section 4) — emit into the typed
   `DriverStruct` field categories (`clocks`, `resets`,
   `pinctrl`, `interrupts`, `mmio`, `sync`, `properties`,
   `user_data`) per `crates/regseq-model/src/model/driver.rs`:
   - Clock/Reset/PinCtrl/Interrupt/Mmio resources: assign
     `ResourceId`; record `field_name`, `Shape`
     (Single/Optional/Array(N) -- derived from the field's
     `ty::Ty`), kind-specific taxonomic properties from
     `#[rseq::clock/reset/pinctrl/interrupt/mmio]` attributes;
     leave `binding` slot `None`.
   - Synchronization resources (`SyncResource`): assign
     `ResourceId`; record `field_name`, `sync_kind`
     (Mutex/SpinLock/IrqSpinLock/Completion derived from the
     field's `Defined(TypeDefId)` type), `payload_type` (omitted
     for `Completion`). No binding slot.
   - Properties: assign `PropertyId`; record `field_name`,
     `field_type` (constrained to `ScalarType`), `property_id`,
     `default_value`, `vendor_prefix`, `required`; leave
     `bound_value` `None`.
   - User data: assign `UserDataId`; record `field_name` and
     `inner_type` (`Option<T>` where `T: Copy` -- enforce the
     constraint via `tcx.layout_of` / trait-query).
8. **Source locations.** Populate the model-level
   `source_locations: BTreeMap<DeclId, SourceOrigin>` by
   resolving each item's `Span` via `tcx.sess.source_map()` to a
   `SourceLocation { file: FileId, start_line, start_col,
   end_line, end_col }`. Intern file paths through the model's
   `SymbolInterner`. Use `Source(SourceLocation)` with the
   resolved range; never `CompilerGenerated` for Pass 1 (no
   synthesized declarations exist at this stage).

### Symbol interning

All names go through the model's `SymbolInterner`. After Pass 1,
no raw `String`s remain in any declaration-level entity.

### Cross-references

Pass 1 finishes with all IDs assigned, so cross-references (MMIO
resource → `RegisterBlockId`, function call target →
`FunctionId`, type field → `TypeDefId`) are resolvable by live
`DefId` → typed `DeclId` lookup. Pass 2 may process bodies in any
order.

### Hard-error conditions

- `ConstructionError::NoDriverStruct` -- no `#[rseq::driver]`
  ADT found.
- `ConstructionError::MultipleDriverStructs` -- more than one.
- `ConstructionError::DuplicateItemName { module, name }`.
- `ConstructionError::InvalidUserDataShape { field }` -- user
  data whose type is not `Option<T>`.
- `ConstructionError::UserDataNotCopy { field, ty }`.
- `ConstructionError::MissingResourceId { field, kind }`.
- `ConstructionError::UnknownRseqAttribute { span, attr }`.
- `ConstructionError::RegistryLookupMismatch` -- registry maps a
  `DefId` to an unrecognized canonical name (e.g., an unknown
  `FrameworkTypeKind` string).

Each variant carries rustc `Span` provenance via
`tcx.sess.source_map()` resolution for diagnostic messages.

## Non-Goals

- No body construction, no statement/terminator translation, no
  recognition rules -- those are tasks 42-53.
- No platform binding / join (Phase 6).
- No verifier wiring (Phase 6).
- No consumption of `RawFrontendArtifact` or the legacy `Mir*`
  owned types.

## Test Plan

Follow the existing `source_to_extraction_*` testing convention:
compile **inline reduced specimens** through the existing
rustc test harness. Do NOT drive the real
`tests/fixtures/pl011-full-spec` fixture here -- that is task
54's responsibility (end-to-end PL011 validation).

- Positive test specimen: inline spec crate mirroring PL011's
  structure at reduced scope (one driver module with one
  driver struct carrying at least one field per category:
  one clock resource, one mmio resource, one interrupt, one
  pin_state marker + pinctrl, one property, one user_data
  field, one sync primitive; one top-level const; one
  register descriptor + register block; one trait impl
  method; one helper; one ISR). Compile through the
  harness, run `build_declaration_skeleton()`, and assert:
  - Exactly one Driver module exists in the constructed
    `RegseqModel`.
  - Every specimen item appears with the correct typed
    `DeclId` and correct classification
    (`UserDefined`/`Abstraction`/`Framework`).
  - Driver struct fields are categorized correctly into
    `clocks`, `resets`, `pinctrl`, `interrupts`, `mmio`,
    `sync`, `properties`, `user_data`, with
    `binding`/`bound_value` slots all `None`.
  - Every Function's `body` is the empty-placeholder shape
    (task 43 will fill it later).
  - `source_locations` has one entry per assigned `DeclId`,
    all `SourceOrigin::Source(...)` with resolvable file
    paths interning through the model's `SymbolInterner`.
  - MMIO resource's register_block cross-reference resolves
    to the correct `RegisterBlockId`.
- Negative test specimens (5 inline specimens under
  `crates/regseq-driver/tests/specimens/` or similar):
  - Two driver structs →
    `ConstructionError::MultipleDriverStructs`.
  - No driver struct → `ConstructionError::NoDriverStruct`.
  - User data field of type `T` (not `Option<T>`) →
    `ConstructionError::InvalidUserDataShape`.
  - User data field `Option<T>` where `T` is not `Copy` →
    `ConstructionError::UserDataNotCopy`.
  - Missing required `id` on `#[rseq::clock]` →
    `ConstructionError::MissingResourceId`.
- Assertions are **structural, in-test** -- compare field
  values on the constructed `RegseqModel` directly. Do NOT
  serialize to JSON. `Serialize` wiring for model types is
  task 54's scope.
- `cargo test -p regseq-driver` passes.

## Review

After tests pass, use `codex:rescue` to review against
`docs/discussions/models/regseq-model/spec.md` Section 7
(identity scheme), Section 9 (body ownership), and
`ir-reference.md` Section 8 (Pass 1 algorithm). Verify no
string-keyed cross-reference logic, no `RawFrontendArtifact`
import, no `Mir*` owned-type import, and every entity kind from
spec.md Section 3 is handled. Iterate until approved.

## Quality Gate

```
cargo test -p regseq-driver
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-driver -- -D warnings
```

## Acceptance Criteria

- `crates/regseq-driver/src/construction.rs` exists with the
  public `build_declaration_skeleton()` entry point consuming
  `TyCtxt` + `ExtractionRegistries`.
- `ConstructionError` enum carries rustc-Span provenance for
  every documented failure case.
- The inline positive-test specimen produces a declaration
  skeleton matching the structural assertions listed in Test
  Plan (no JSON golden in this task; serialization wiring lives
  in task 54).
- Negative tests cover every named hard-error condition (5
  specimens, one per error variant).
- No reference to `RawFrontendArtifact` or the legacy
  `regseq_model::declaration::mir_body::*` types anywhere in
  construction code.
- Codex review approved.
