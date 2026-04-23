# regseq-model-64: User-data initialization Path A (zero-init whitelist + Option tier)

## Description

Extend the Regseq compiler's user-data acceptance predicate from
its current `Option<T: Copy>`-only rule to a two-rule contract:
either `Option<T: Copy>` (Rule 2, unchanged) or a compiler-
controlled zero-init whitelist of primitive scalars and aggregates
(Rule 1). This implements **Path A** of the design at
`docs/discussions/models/regseq-model/user-data-initialization.md`.

This task spans four crates (`regseq-driver`, `regseq-model`,
`regseq-backends` shared lowering, `regseq-backends/linux`) as an
**intentional override** of `task-spec-workflow.md:120-121`'s
"one crate or a tight crate pair" norm. The authority for the
override is the design doc's cross-cutting Implementation touch-
points section. The change is one coherent feature whose touch-
points cannot land in isolation without leaving the workspace
uncompilable mid-sequence.

The immediate blocker this unblocks: the PL011 Stage-1 fixture at
`tests/fixtures/pl011-full-spec/src/driver.rs:72` introduced
`imsc_shadow: u32` as a software shadow of the IMSC register per
REQ-CORR-MASK-SHADOW. The current construction rule rejects bare
`u32` with `InvalidUserDataShape`, blocking every downstream
workflow (raw-frontend regeneration, Phase-6 model golden, IR
goldens, Linux reference C co-authoring).

## Input Context

### Authoritative design source

- `docs/discussions/models/regseq-model/user-data-initialization.md`
  -- the authoritative design doc; all decisions are frozen here.
  Re-reading this is mandatory before starting any implementation
  step. All open questions have been resolved; no re-debate is
  permitted.
- `docs/discussions/models/regseq-model/user-data-initialization-codex-review/README.md`
  -- summary of the three-round Codex debate that produced the
  design; read this for context on why specific choices were made.
- `docs/discussions/models/regseq-model/user-data-initialization-codex-review/round-3-final-design.md`
  -- the converged review round; lists all touch-points and
  amendments.

### Batch brief

- `.harness/spec-batches/user-data-init-path-a.md` -- the brief
  that spawned this task spec. The brief's Implementation Notes
  and Non-Goals sections are binding. **Precedence order when
  sources conflict**: design doc
  (`docs/discussions/models/regseq-model/user-data-initialization.md`)
  is authoritative; this task spec supersedes the brief on any
  factual detail that drifted during multi-round review; the
  brief supersedes only on batch-completion semantics (tracker
  updates, acceptance criteria). If a worker encounters a
  conflict, follow this order and note the conflict in the
  final Codex review submission.

### Source files to read and modify

Construction (regseq-driver):

- `crates/regseq-driver/src/construction/driver.rs:216-253` --
  the construction rule being extended (the central change site).
- `crates/regseq-driver/src/construction/driver.rs:227` -- the
  current `inner_is_copy` binding site where the `Copy` trait
  predicate is invoked for `Option<T>`'s inner type.
- `crates/regseq-driver/src/construction/driver.rs:562-567` --
  the `type_is_copy_modulo_regions` helper definition and its
  use of `TypingEnv::fully_monomorphized()`. The Rule-1
  predicate must reuse this helper via a new call site (parallel
  to the existing `inner_is_copy` binding at `driver.rs:227`)
  that checks the outer user-data field type under the same
  monomorphized typing environment.
- `crates/regseq-driver/src/construction/walk.rs:827-884` --
  existing `ty_to_regseq` middle-type-to-`RegseqType` mapping
  helper. Construction's current user-data path calls this at
  `crates/regseq-driver/src/construction/driver.rs:239-241`. The
  helper currently has lossy `u128`/`i128` arms at
  `walk.rs:837-851` that map to `U64` (per legacy normalization's
  broad acceptance). Under the Stage-1 whitelist these arms must
  either be removed, tightened, or guarded by the new whitelist
  predicate so `u128`/`i128` are rejected at construction rather
  than silently remapped to `u64`. Verify this interaction
  during touch-point 1 implementation.
- `crates/regseq-driver/src/construction.rs:127,146,451,467` --
  `ConstructionError` variants + docs + display text.

Model (regseq-model):

- `crates/regseq-model/src/model/resources.rs:306-318` -- current
  `UserDataField` shape with `id`, `field_name`, `inner_type`.
- `crates/regseq-model/src/model/resources.rs:594-599` -- existing
  `UserDataField { ... }` literal in this file's own tests;
  must be updated to the new enum-kind shape.
- `crates/regseq-model/src/model/driver.rs:294-298` -- another
  existing `UserDataField { ... }` literal; must be updated.
- Run `rg 'UserDataField \{' crates/` to find all other literal
  construction sites mechanically. Every such site must be
  updated to construct via the new shape
  (`UserDataField { id, field_name, kind: UserDataKind::... }`).
- `crates/regseq-model/src/ir/types.rs:18-29` -- `RegseqType`
  enum (`Scalar`, `Bool`, `Array`, `Defined`, etc.; no `Tuple`
  variant exists today).
- `crates/regseq-model/src/ir/types.rs:52-63` -- `ScalarType`
  enum with integer variants only; `bool` is `RegseqType::Bool`,
  NOT part of `ScalarType`.
- `crates/regseq-model/src/model/type_def.rs:34-57` -- `TypeDef`
  for user-defined struct/enum representation.
- `crates/regseq-model/src/verify/mod.rs:205` --
  `UserDataFieldNotOptionCopy` error kind.
- `crates/regseq-model/src/verify/rules/driver_struct.rs:11,212,239`
  -- verifier rules + error text.
- `crates/regseq-model/src/verify/rules/cross_item.rs:348` --
  cross-item user-data type resolution.
- `crates/regseq-model/src/normalize/surface.rs:1044,1081,1111-1129`
  -- raw/legacy normalization enforcing `Option<T: Copy>`. The
  existing Rule-2 Copy table at lines 1111-1129 must REMAIN
  UNCHANGED (it governs Rule-2 acceptance, which is unchanged
  by Path A). A SEPARATE new Rule-1 zero-init whitelist table
  (admitting only `u8, u16, u32, u64, bool`) is added adjacent
  to it and is consulted only by the new zero-init
  classification path. See touch-point 8 for full details.
- `crates/regseq-model/src/normalize/surface.rs:173-186` --
  the current driver `TypeDef` seeding path; confirms that the
  existing normalization only handles the driver struct's own
  TypeDef.
- `crates/regseq-model/src/normalize/surface.rs:1024-1028` --
  the current user-data-attachment-from-driver-fields site;
  user-data extension in normalization plugs in here.
- `crates/regseq-model/src/normalize/raw_artifact.rs:414-435`
  -- `RawStructFact`/`RawFieldFact` raw-surface types. Relevant
  only if future Rule-1 Copy-struct normalization support is
  added; Stage 1 does NOT add raw non-driver struct mapping
  (see touch-point 8 for the Stage-1 normalization scope
  narrowing).

Lowering (regseq-backends):

- `crates/regseq-backends/src/lowering/plan.rs:109` --
  `ResourceObligations` (currently MMIO/IRQ/property/clock/reset/
  pinctrl only; user-data must be added).
- `crates/regseq-backends/tests/multi_resource_plan.rs:102-109` --
  existing `ResourceObligations { ... }` literal in tests; must
  be updated to include the new user-data field (empty default
  for non-user-data scenarios, populated for user-data ones).
- `crates/regseq-backends/tests/register_uart_port_generality.rs:133-140,430-437`
  -- additional `ResourceObligations { ... }` literals; same
  update required.
- Run `rg 'ResourceObligations \{' crates/` to find all other
  literal construction sites mechanically. Every such site must
  be updated.
- `crates/regseq-backends/src/lowering/from_model.rs:190` -- one
  of two lowering pipelines.
- `crates/regseq-backends/src/lowering/planner.rs:311` -- the
  other lowering pipeline.

Linux backend (regseq-backends/linux):

- `crates/regseq-backends/src/linux/planner.rs:23-56` -- existing
  `LinuxPlanError` enum location (where the new
  `UnsupportedUserDataShape` variant must be added, mirroring
  the adjacent `UnsupportedPropertyScalar` variant shape).
- `crates/regseq-backends/src/linux/planner.rs:146` --
  `shared_struct_fields` builder.
- `crates/regseq-backends/src/linux/plan.rs:400-433` -- existing
  `LinuxPropertyScalar` enum + `c_type` helper (pattern to mirror
  for `LinuxUserDataScalar`).
- `crates/regseq-backends/src/linux/emitter.rs:152-160` --
  the shared-header `CStructField` builder that consumes each
  `LinuxSharedStructField.c_type`; READ-ONLY anchor used to
  understand the rendering contract.
- `crates/regseq-backends/src/c/mod.rs:232-238` -- the C-AST
  `StructDef` renderer that outputs
  `<type_name> <field_name>;` for each field; READ-ONLY anchor
  that explains why non-scalar user-data must be rejected at
  Linux plan construction rather than passed through.
- `crates/regseq-backends/src/linux/emit_semantic.rs:216` --
  UNRELATED to shared-header rendering; handles function-body
  semantic IR lowering. **NOT MODIFIED IN THIS TASK**; listed
  only to explicitly scope it out.
- `crates/regseq-backends/src/linux/emit_module.rs:703` --
  existing `devm_kzalloc`. **NOT MODIFIED IN THIS TASK**; see
  Non-Goals.

### Existing test anchors

Construction/model tests (user-data patterns to mirror):

- `crates/regseq-driver/tests/construction_pass1.rs:309-321` --
  existing positive `Option<Stats>` user-data assertion reading
  `.inner_type`. This must be updated to match on the new
  `UserDataKind::Option` variant, preserving Rule-2
  `Option<Copy struct>` regression coverage.
- `crates/regseq-driver/tests/construction_pass1.rs:783-815` --
  existing user-data construction tests (negative cases); new
  Rule-1 tests extend this file.
- `crates/regseq-model/tests/normalize.rs:241-339` -- existing
  user-data normalization tests; new `UserDataField` enum tests
  belong here.
- `crates/regseq-model/tests/verify_phase_2.rs:344-367` --
  existing verifier tests for user-data; updated verifier tests
  belong here.
- `crates/regseq-model/tests/pl011_pipeline_phase6.rs:126-134` --
  Phase-6 model golden regeneration; owns the `REGEN_GOLDEN=1`
  path.

Backend/lowering tests:

- `crates/regseq-backends/tests/lowering_from_model.rs:201-239`
  -- existing lowering-plan construction test patterns (pipeline 1);
  user-data obligation tests extend this pattern.
- `crates/regseq-backends/tests/lowering_from_model.rs:365-469`
  -- existing PL011 cross-pipeline equivalence checks; user-data
  cross-pipeline tests extend these.
- `crates/regseq-backends/tests/lowering_boundary.rs` -- existing
  lowering boundary tests; user-data carriage tests belong here
  (grep inside the file for the appropriate test function
  matching PL011 patterns).
- `crates/regseq-backends/tests/property_realization_honoring.rs:47-108`
  -- existing Linux-plan helper that sets up shared-struct-field
  scenarios; user-data scenarios mirror this helper.
- `crates/regseq-backends/tests/property_realization_honoring.rs:250-266`
  -- existing shared-struct-field assertion pattern; user-data
  inclusion tests (scalar positive; non-scalar
  `UnsupportedUserDataShape` negative) extend this pattern.
- `crates/regseq-backends/tests/linux_emission_scaffold.rs:1064-1078`
  -- existing Linux emission tests covering shared-header driver
  state struct (`shared_header_has_driver_state_struct` or an
  adjacent function); the integration assertion that the emitted
  Linux C contains `u32 imsc_shadow;` must extend these tests.

## Implementation Notes

### Method: TODO-style checklist driven by TodoWrite

Before writing any code, build a TodoWrite list with **thirteen
entries**, one per touch-point below. Mark each entry
`in_progress` when starting that touch-point and `completed` only
after the corresponding test(s) pass. Do not batch completions.
If a touch-point reveals an unexpected dependency, pause and
surface the issue rather than expanding scope silently.

### Stage-1 scope narrowings (per design doc)

Before the checklist, note the scope narrowings that bound this
task:

- **Rule-1 primitive whitelist**: `u8, u16, u32, u64, bool` only.
  Signed integers, `usize`, `isize`, floats, `char`, `u128`,
  `i128` are all rejected (deferred per design doc "Full target
  whitelist (deferred)" section at lines 218-230).
- **Rule-1 aggregate shapes**: arrays and `Copy` structs
  recursively composed of accepted fields. **Tuples are excluded**
  per design doc "Stage-1 Rule-1 whitelist narrowing (tuples
  excluded)" section; `RegseqType` has no `Tuple` variant in the
  Stage-1 scope.
- **Backend scope**: scalar user-data only is guaranteed to
  produce a valid Linux plan. Arrays and `Copy` structs are
  accepted at model + lowering-plan levels but REJECTED at
  Linux plan construction with a new
  `LinuxPlanError::UnsupportedUserDataShape` variant BEFORE
  reaching `shared_struct_fields`. The C-AST renderer at
  `crates/regseq-backends/src/c/mod.rs:232-238` produces
  `<type_name> <field_name>;` output from
  `LinuxSharedStructField.c_type`; it never sees an unrealizable
  type because non-scalar user-data fails earlier. The existing
  `emit_semantic.rs:216` panic path is UNRELATED to this flow
  (it handles function-body semantic IR lowering, not shared-
  struct-field rendering) and is NOT modified by this task. See
  design doc "Stage-1 backend-tested scope (explicit narrowing)"
  section.

### The thirteen-entry TODO checklist

Construction (3 entries):

1. **`crates/regseq-driver/src/construction/driver.rs:216-253`**
   -- replace strict `Option<T>` check with a branch accepting
   either Rule 1 (zero-init whitelist) or Rule 2
   (`Option<T: Copy>`). Preserve the outer `Copy` check via the
   existing `type_is_copy_modulo_regions` helper at
   `driver.rs:227` (the Rule-1 path continues to use this for
   the outer field type; nested whitelist fields are implicitly
   `Copy` because `Copy` is closed under struct composition when
   every field is `Copy`). The `TypingEnv::fully_monomorphized()`
   helper at `driver.rs:562` is the correct typing environment
   for the monomorphized-type cycle guard; reuse it. **Reject
   types with `TyKind::Param` or unresolved projections BEFORE
   applying the whitelist.**

2. **`crates/regseq-driver/src/construction.rs:127,451`** --
   add a new `ConstructionError` variant for Rule-1 rejections
   (e.g., `UserDataNotZeroInitWhitelisted { field_name,
   rejected_type }`). Add the matching display text at the
   display arms starting at `construction.rs:451`. Update docs
   at `construction.rs:146,467` to reflect the two-rule contract
   (no longer "user-data must be `Option<T>`").

3. **Cycle guard with monomorphized-type keys**: implement the
   recursive whitelist predicate with a visited-set keyed by
   monomorphized type `(DefId, GenericArgsRef)` or `Ty<'tcx>`,
   NOT by `DefId` alone. Counterexample to the `DefId`-only
   approach: `Boxed<u32>` and `Boxed<NonZeroU32>` share a
   `DefId`; a `DefId`-keyed visited set would falsely accept
   `Boxed<NonZeroU32>` after visiting `Boxed<u32>`. Land this
   alongside touch-point 1.

Model (5 entries):

4. **`crates/regseq-model/src/model/resources.rs:306-318`** --
   extend `UserDataField` with a Rule-1/Rule-2 discriminator
   while preserving the existing `id: UserDataId` and
   `field_name: Symbol` fields. Target shape:

   ```rust
   #[derive(Clone, Eq, PartialEq, Debug, Serialize)]
   pub struct UserDataField {
       pub id: UserDataId,
       pub field_name: Symbol,
       pub kind: UserDataKind,
   }

   #[derive(Clone, Eq, PartialEq, Debug, Serialize)]
   #[serde(tag = "kind", content = "data")]
   pub enum UserDataKind {
       /// Rule-2: `Option<T: Copy>`. `inner_type` is the
       /// `RegseqType` of `T` inside the `Option`.
       Option { inner_type: RegseqType },
       /// Rule-1: zero-init-accepted type. `inner_type` is the
       /// `RegseqType` of the accepted bare value (no Option
       /// wrapping).
       ZeroInit { inner_type: RegseqType },
   }
   ```

   The `#[serde(tag = "kind", content = "data")]` attribute
   matches the adjacently-tagged enum pattern already used by
   `RegseqType` at
   `crates/regseq-model/src/ir/types.rs:16-18` and the
   serialization shape the normalization tests assert (see Test
   Plan). Use this attribute verbatim; do not default to
   externally-tagged serialization.

   The `inner_type: RegseqType` that currently lives on
   `UserDataField` moves into each `UserDataKind` variant; every
   match on `UserDataKind` must handle both variants. (The
   design doc's "Open follow-ups (for implementation phase)"
   section at `user-data-initialization.md:376-377` leaves this
   choice open; per user direction 2026-04-22, this task fixes
   the choice to the enum-variant shape for exhaustive-match
   semantic clarity.)

5. **`crates/regseq-model/src/verify/mod.rs:205`** -- rename or
   redocument the `UserDataFieldNotOptionCopy` error kind to
   reflect the two-rule contract. Rename to e.g.,
   `UserDataFieldNotZeroInitOrOptionCopy` or keep the name and
   update docs to reflect that Rule 1 (zero-init) is now also an
   accept path.

6. **`crates/regseq-model/src/verify/rules/driver_struct.rs:212,239`**
   -- update verifier logic and the diagnostic text that currently
   requires `Option<T>`. The verifier must accept either
   `UserDataKind::Option` or `UserDataKind::ZeroInit` and apply
   the appropriate per-variant validity checks.

7. **`crates/regseq-model/src/verify/rules/cross_item.rs:348`** --
   cross-item type resolution must follow the new classified
   stored type. Every reference to `u.inner_type` on the current
   `UserDataField` must move to `u.kind` and match on the enum.

8. **`crates/regseq-model/src/normalize/surface.rs:1044,1081,1102`**
   -- raw/legacy normalization currently enforces
   `Option<T: Copy>` and stores every user-data field as
   `Scalar(U32)`. Update the normalization pass to introduce
   **TWO separate predicates** (do not conflate them):

   - **Rule-1 zero-init whitelist predicate**: admits only
     `u8, u16, u32, u64, bool` + arrays + `Copy` structs of
     accepted fields (matching the construction-pass predicate
     from touch-points 1+3). This is the predicate the
     zero-init classification path uses.
   - **Rule-2 `Copy` validity predicate**: unchanged from
     current behavior. Admits any `T: Copy` (including `i8`,
     `i16`, `i32`, `i64`, `usize`, `isize`, `f32`, `f64`,
     `char`, etc. -- any primitive or user type rustc treats
     as `Copy`). This is the predicate the `Option<T>`
     classification path uses.

   The existing Copy table at `surface.rs:1102` currently
   includes `u128`, `i128`, `char`. Do NOT tighten this table
   to the Rule-1 whitelist; Rule 2 accepts `i32`, `i64`,
   `f32`, `char`, etc., and regressing those would break
   existing `Option<T>` user-data patterns. If the existing
   table serves a purpose beyond Rule-2 Copy checking (e.g.,
   an in-model type-name whitelist), keep that scope but
   introduce a NEW Rule-1-scoped primitive table adjacent to
   it, used only by the Rule-1 classification path.

   Updates required:
   - **Add** a new Rule-1 zero-init classification path in
     normalization that accepts a narrowed Stage-1 subset:
     **scalars (`u8, u16, u32, u64, bool`) and arrays of
     scalars**. Map accepted fields to
     `UserDataKind::ZeroInit { inner_type: ... }` with the
     faithful `RegseqType`.
   - **Stage-1 normalization scope narrowing for Rule-1 Copy
     structs**: raw-artifact normalization does NOT add
     non-driver `RawStructFact` -> `TypeDefId` seeding support
     in this task. The current normalization only seeds the
     driver's own `TypeDef` at
     `crates/regseq-model/src/normalize/surface.rs:173-186`;
     adding support for arbitrary non-driver Copy-struct
     user-data types via `RawStructFact` at
     `crates/regseq-model/src/normalize/raw_artifact.rs:414-435`
     is out of Path A scope. Rule-1 Copy struct user-data is
     therefore accepted at **construction** (via
     rustc-direct `TyCtxt` introspection) but is NOT accepted
     via the **normalization** path (which reads
     `raw-frontend.json`). If a future fixture adds a Rule-1
     Copy struct user-data field and needs the Phase-6 golden
     regeneration to work, that fixture's work includes
     extending `RawStructFact` normalization. For Stage-1
     (PL011's `imsc_shadow: u32` scalar), the normalization
     path encounters only scalar + array user-data and works
     correctly.
   - Rule-1 normalization REJECTS Copy-struct Rule-1 shapes
     (returns a clear error with a "normalization scope
     narrowing" message) rather than silently mishandling them.
   - **Preserve** the existing Rule-2 `Option<T: Copy>` path
     unchanged at the acceptance level. The Rule-2 primitive
     Copy table at `surface.rs:1111-1129` stays as-is (keeps
     `u128`, `i128`, `char`, signed ints, etc. -- all primitive
     string spellings currently in the table). Rule-2 fields
     map to `UserDataKind::Option { inner_type: ... }`.
   - **Rule-2 normalization scope is primitive-string-only**.
     The current normalization at
     `crates/regseq-model/src/normalize/surface.rs:1111-1129`
     is a string table that recognizes primitive type-name
     spellings and `()`. There is no TyCtxt access in
     normalization (it operates on a JSON raw artifact, not on
     rustc state). User-defined Copy structs in raw-artifact
     Rule-2 inputs (e.g., `Option<Stats>`) are **not supported
     by Stage-1 normalization** because adding non-driver
     `RawStructFact`-to-`TypeDefId` seeding (see
     `raw_artifact.rs:414-435` and `surface.rs:173-186`) is
     out of Path A scope. The construction path (rustc-backed)
     fully supports `Option<Stats>` via `TyCtxt` introspection;
     the normalization path does not.
     - If the raw artifact contains an `Option<T>` user-data
       field where `T` is a user-defined struct name, Rule-2
       normalization **rejects** with a clear
       "user-defined Rule-2 inner type not supported in
       Stage-1 normalization; use the construction path or
       extend normalization in a follow-up task" error.
     - If `T` is a primitive-string the existing table accepts,
       map to the corresponding `RegseqType` variant faithfully
       where representable (e.g., `"i32"` -> `Scalar(I32)`).
   - **Preserve existing Rule-2 inner-type mapping behavior
     for unrepresentable primitives**: for Rule-2 inner types
     NOT representable in `RegseqType` (`f32`, `f64`, `char`,
     `u128`, `i128`), keep whatever the existing normalization
     at `surface.rs:1081` does today (currently a lossy
     `Scalar(U32)` fallback). **Do NOT change Rule-2 behavior
     for unrepresentable types in this task**; the design doc
     locks Rule-2 semantics as unchanged. If a future task
     extends `RegseqType` to cover floats/`char`/128-bit ints,
     Rule-2 mapping can be made accurate at that point.
   - The instruction "preserve the author-declared inner type
     accurately" applies to the **Rule-1 zero-init path** (for
     which all accepted types are representable) and to
     Rule-2 inner types that happen to be representable. For
     unrepresentable Rule-2 inner types, current behavior is
     preserved as-is.

Lowering (3 entries):

9. **`crates/regseq-backends/src/lowering/plan.rs:109`** -- add
   user-data to `ResourceObligations` (currently only
   MMIO/IRQ/property/clock/reset/pinctrl). User-data entries
   carry the `UserDataKind` + `RegseqType` + field-name data
   downstream consumers need.

10. **`crates/regseq-backends/src/lowering/from_model.rs:190`** --
    one of two lowering pipelines; populate user-data obligations
    from the model's `DriverStruct.user_data` list. Each entry
    becomes a `ResourceObligations` user-data entry with the
    correct `UserDataKind` + `RegseqType` + field-name.

11. **`crates/regseq-backends/src/lowering/planner.rs:311`** --
    the legacy lowering pipeline.
    **Stage-1 scope narrowing**: the legacy pipeline reads
    `DeclarationModel` at `planner.rs:314-315` and builds
    resource obligations from it at `planner.rs:317-475`. The
    current `DeclarationModel` at
    `crates/regseq-model/src/declaration/mod.rs:69-94` has no
    user-data field, and raw declaration field facts lack
    `field_type` at
    `crates/regseq-model/src/declaration/raw_input.rs:451-459`.
    Extending the declaration model to carry user-data would
    cascade through raw-input parsing, normalization, and every
    consumer of `DeclarationModel`. That cascade is out of
    scope for Path A because the legacy pipeline is being
    retired per the RegseqModel Phase-7 migration (see
    `docs/discussions/models/regseq-model/migration.md`).

    In Stage-1, the legacy pipeline does NOT carry user-data.
    The `ResourceObligations.user_data` field (touch-point 9)
    is populated by the RegseqModel-based pipeline
    (`from_model.rs`, touch-point 10) only. The legacy pipeline
    leaves `ResourceObligations.user_data` empty. Cross-pipeline
    equivalence tests for user-data (at
    `crates/regseq-backends/tests/lowering_from_model.rs:365-469`)
    assert this: the RegseqModel pipeline populates user-data;
    the legacy pipeline leaves it empty. This is an acceptable
    Stage-1 asymmetry because the legacy pipeline is
    deprecated; no PL011 workflow depends on it carrying
    user-data.

    The worker does NOT need to modify `planner.rs:311` beyond
    adding the empty user-data initialization (i.e., ensuring
    the new `ResourceObligations.user_data` field is constructed
    with an empty `Vec` in the legacy path's
    `ResourceObligations` construction site, so the type
    compiles). If the legacy pipeline is already retired and
    the touch-point site no longer exists, skip this entry and
    note it in the worker's TODO checklist.

Linux backend (2 entries; item 12 split into 12a/12b):

12a. **`crates/regseq-backends/src/linux/planner.rs:146-186`** --
     include user-data in `shared_struct_fields` via a
     scalar-only path. Current insertion order (verify by reading
     `planner.rs:146-186`) is MMIO, IRQ, clocks, properties, then
     serial port. Insert scalar user-data fields **AFTER serial
     port** (i.e., at the end of the struct-field list). Each
     scalar `UserDataField` on the driver becomes a
     `LinuxSharedStructField` in this position, preserving
     declaration order among user-data entries.

     **Stage-1 Linux-plan acceptance policy**: admit only
     `UserDataKind::ZeroInit { inner_type: <scalar> }` fields.
     Reject all of the following with a new
     `LinuxPlanError::UnsupportedUserDataShape` variant (see
     touch-point 12b for the variant shape):
     - `UserDataKind::ZeroInit { inner_type: Array(..) }` (Rule-1
       array; declarator AST needed for correct C emission, not
       landed in Stage 1).
     - `UserDataKind::ZeroInit { inner_type: Defined(..) }`
       (Rule-1 Copy struct).
     - `UserDataKind::Option { inner_type: ANY }` (Rule-2
       Option; not yet C-emittable as user-data).

     The acceptance test for ZeroInit is "kind is ZeroInit AND
     `LinuxUserDataScalar::parse_from_regseq_type(inner_type)`
     returns `Some(_)`". All other combinations return the
     error variant. Rejection happens BEFORE the field would be
     inserted into `shared_struct_fields`.

     This ordering choice is fixed in the task spec to avoid
     ambiguity: tests and acceptance criteria verify the exact
     position mechanically. If a future task needs to change
     user-data positioning (e.g., ergonomic grouping), it must
     update this ordering contract explicitly.

     Mirror the existing field-building patterns for properties
     and serial-port fields already present in `planner.rs`. No
     shadow-register filtering or interaction is part of this
     task; shadow-register support is a separate deferred
     workstream at
     `docs/discussions/models/regseq-model/shadow-registers.md`.

12b. **`crates/regseq-backends/src/linux/plan.rs` + `planner.rs`**
     -- add two narrow pieces of infrastructure:

     (i) `LinuxUserDataScalar` enum in `plan.rs`, adjacent to the
         existing `LinuxPropertyScalar` at `plan.rs:400-433`. This
         is the ONLY helper that lands in Stage 1. Target shape
         (pseudocode; worker produces compilable Rust respecting
         repo style):

         ```rust
         // Pseudocode. Follow LinuxPropertyScalar at
         // crates/regseq-backends/src/linux/plan.rs:400-433 for
         // doc-comment style and method conventions.
         pub enum LinuxUserDataScalar { U8, U16, U32, U64, Bool }

         impl LinuxUserDataScalar {
             pub fn parse_from_regseq_type(ty: &RegseqType) -> Option<Self> {
                 match ty {
                     RegseqType::Scalar(ScalarType::U8)  => Some(Self::U8),
                     RegseqType::Scalar(ScalarType::U16) => Some(Self::U16),
                     RegseqType::Scalar(ScalarType::U32) => Some(Self::U32),
                     RegseqType::Scalar(ScalarType::U64) => Some(Self::U64),
                     RegseqType::Bool                    => Some(Self::Bool),
                     _ => None,
                 }
             }

             pub fn c_type(self) -> &'static str {
                 match self {
                     Self::U8    => "u8",
                     Self::U16   => "u16",
                     Self::U32   => "u32",
                     Self::U64   => "u64",
                     Self::Bool  => "bool",
                 }
             }
         }
         ```

     (ii) `LinuxPlanError::UnsupportedUserDataShape` variant in
          `planner.rs`, added to the existing `LinuxPlanError`
          enum at `planner.rs:23-56`. Mirror the shape of the
          adjacent `UnsupportedPropertyScalar` variant at
          `planner.rs:46-56`: struct-like payload with named
          fields carrying the offending field name and the
          rejected type's spelling, plus a `#[error(...)]`
          message in the same style.

     **No `user_data_aggregate_to_c_type` helper is landed in
     Stage 1.** Rendering aggregate user-data types correctly
     requires a richer C declarator AST than the current
     renderer supports: arrays must render as
     `u32 field_name[4];` via the declarator, not
     `u32[4] field_name;`, because the C-AST renderer at
     `c/mod.rs:232-238` produces a fixed
     `<type_name> <field_name>;` output from
     `LinuxSharedStructField.c_type`. Non-scalar user-data is
     instead rejected at Linux plan construction via
     `LinuxPlanError::UnsupportedUserDataShape` (per touch-point
     12a). A future task adds both the aggregate helper AND the
     declarator AST extension together when a fixture motivates
     the scope.

### Golden regeneration (after implementation passes compile)

After all 13 touch-points land and non-regression tests pass:

1. Regenerate
   `tests/fixtures/pl011-full-spec/expected/raw-frontend.json`
   via the `cargo-regseq` binary. The CLI is defined at
   `crates/cargo-regseq/src/cli.rs:24-37`; `--target-os` and
   `--platform-file` are mandatory, `--output-dir` has a default
   of `./regseq-out/`, and `--emit` is repeatable (defaulting
   to `driver` per `cli.rs:180-181`). Raw artifacts are written
   to `output_dir/drivers/<driver_name>.raw.json` per
   `crates/regseq-driver/src/callbacks.rs:369-377`. The
   subcommand requires `cargo-regseq` on `$PATH` and a
   discoverable `regseq-driver` binary per
   `crates/cargo-regseq/src/toolchain.rs:126-129`.

   The workspace provides both binaries; run them directly via
   `cargo run` to avoid installation. Concrete regeneration
   command (run from the workspace root, assumes the workspace
   builds cleanly):

   ```bash
   # Build both binaries in release profile so regseq-driver is
   # discoverable by cargo-regseq (which looks for it alongside
   # itself on PATH or under the workspace target directory).
   cargo build -p cargo-regseq -p regseq-driver --release

   # Add the release target dir to PATH so `cargo regseq` finds
   # both binaries. The workspace root's target directory is
   # `target/release` by default; adjust if CARGO_TARGET_DIR is
   # set.
   export PATH="$PWD/target/release:$PATH"

   # Regenerate the raw-frontend artifact for the PL011 fixture.
   (cd tests/fixtures/pl011-full-spec && \
     cargo regseq \
       --target-os linux \
       --platform-file platform.toml \
       --output-dir /tmp/pl011-regen \
       --emit raw-artifact)

   # Copy into the expected-artifact location. The filename
   # is derived from `artifact.driver_name` per
   # crates/regseq-driver/src/callbacks.rs:369-377. The current
   # PL011 fixture's driver_name is `pl011-full-spec` per the
   # existing expected/raw-frontend.json first lines. Verify the
   # file exists in /tmp/pl011-regen/drivers/ before copying; if
   # the driver-name has changed, discover deterministically:
   #   ls /tmp/pl011-regen/drivers/*.raw.json
   # which should print exactly one file.
   cp /tmp/pl011-regen/drivers/pl011-full-spec.raw.json \
      tests/fixtures/pl011-full-spec/expected/raw-frontend.json
   ```

   Commit the refreshed `expected/raw-frontend.json`.
2. Regenerate
   `tests/fixtures/pl011-full-spec/expected/regseq-model.json`
   via
   `REGEN_GOLDEN=1 cargo test -p regseq-model --test pl011_pipeline_phase6`.
3. Regenerate IR goldens under
   `tests/fixtures/pl011-full-spec/expected/ir/` via
   `UPDATE_GOLDENS=1 cargo test -p regseq-driver --test pl011_construction`.
4. Commit the three regenerated artifact sets in the same commit
   as the implementation changes.

### Cross-crate scope override

This task explicitly spans four crates (`regseq-driver`,
`regseq-model`, `regseq-backends` shared, `regseq-backends/linux`)
as a deliberate override of `task-spec-workflow.md:120-121`'s
single-crate guidance. Authority: design doc cross-cutting
Implementation touch-points section. Rationale: the change is
one coherent feature whose touch-points cannot split cleanly
without leaving the workspace uncompilable mid-sequence.

## Non-Goals

- **No Tier-2 author opt-in mechanism**. No `ZeroInit` trait, no
  `#[rseq::zero_init]` attribute, no `#[derive(ZeroInit)]` macro.
  Deferred per design doc; revisit when a fixture motivates it.
- **No widening of the Rule-1 whitelist beyond Stage-1 scope**.
  Do NOT add `i8`, `i16`, `i32`, `i64`, `usize`, `isize`, `f32`,
  `f64`, `char`, `u128`, `i128`, or tuples.
- **No Rule-2 niche-init hardening**. The latent `Option<bool>`
  soundness bug (where `kzalloc` gives `Some(false)` instead of
  `None`) is documented in the design doc and deferred.
- **No shadow-register feature work**. Shadow-register is a
  separate deferred workstream at
  `docs/discussions/models/regseq-model/shadow-registers.md`.
- **No `user_data_aggregate_to_c_type` helper** is landed in
  Stage 1. Rendering aggregate user-data types requires a
  richer C declarator AST than the current renderer supports
  (per the `c/mod.rs:232-238` renderer's fixed
  `<type_name> <field_name>;` output). A future task adds both
  the aggregate helper AND the declarator AST extension
  together. Non-scalar user-data is rejected at Linux plan
  construction instead (see touch-point 12a).
- **No changes to `crates/regseq-backends/src/linux/emit_semantic.rs`**.
  This file handles function-body semantic IR lowering, which
  is unrelated to the shared-header user-data emission path
  governed by this task. Listed here only to explicitly scope
  it out.
- **No changes to `crates/regseq-backends/src/linux/emit_module.rs:703`**
  (the `devm_kzalloc` call). `devm_kzalloc` already zero-
  initializes the whole struct, which is sufficient for every
  Stage-1 Rule-1 user-data field. No post-allocation
  initialization stores are emitted for user-data fields.
- **No changes to `Mmio::modify`'s `todo!()` gap** at
  `crates/regseq/src/mmio.rs:72-86` (the `modify` method body
  spanning lines 72-86, with `todo!()` at line 86). Orthogonal
  to user-data work.
- **No changes to tracker items or tasks for other phases**.
  Phase-7 or Phase-8 tasks unrelated to user-data-init are out
  of scope.

## Test Plan

### Positive tests (Rule 1 arms)

Add one test per arm to
`crates/regseq-driver/tests/construction_pass1.rs` (extending the
patterns at lines 783-815):

- **Scalar arm**: `u8`, `u16`, `u32`, `u64`, `bool` each
  accepted as user-data. One test per primitive (5 tests).
- **Array arm**: `[u32; 4]`, `[u8; 16]`, `[[u16; 2]; 8]` each
  accepted.
- **`Copy` struct arm**:

  ```rust
  #[derive(Copy, Clone)]
  struct Counters { rx: u32, tx: u32 }
  ```
  with a driver field `counters: Counters` accepted.

- **Nested struct arm**:

  ```rust
  #[derive(Copy, Clone)]
  struct Inner { x: u8 }
  #[derive(Copy, Clone)]
  struct Outer { inner: Inner }
  ```
  with a driver field `state: Outer` accepted.

### Positive tests (Rule 2 tier, unchanged behavior)

These tests enforce that Rule-2 `Option<T: Copy>` validity is
NOT regressed by the Rule-1 extension. Mandatory:

- `Option<u32>` accepted.
- `Option<u8>` accepted.
- **`Option<i32>` accepted** (critical regression test: `i32`
  is REJECTED by Rule-1 whitelist but MUST remain accepted by
  Rule-2 because `i32` is `Copy`. This test confirms the two
  predicates are not conflated.).
- **`Option<Copy struct>` accepted via construction path only**
  (preserves existing `Option<Stats>` coverage at
  `crates/regseq-driver/tests/construction_pass1.rs:309-321`):

  ```rust
  #[derive(Copy, Clone)]
  struct Stats { count: u32, sum: u64 }
  ```

  with a driver field `stats: Option<Stats>` accepted. Assert
  the resulting `UserDataField.kind` is
  `UserDataKind::Option { inner_type: RegseqType::Defined(...) }`
  referencing the `Stats` type def.

  **Note**: this test goes through the construction path (rustc
  extract -> Pass 1 -> Pass 2) where `TyCtxt` is available. It
  does NOT go through the normalization path (which reads
  `raw-frontend.json`). Rule-2 normalization does not support
  user-defined Copy structs in Stage-1 per touch-point 8.

### Negative tests (three categories)

**Category A -- explicitly rejected types** (one test per, from
the design doc's "Explicitly rejected" section, **EXCEPT the
never type `!` which has no Stage-1 test** -- see the per-type
guidance below and the matching Review/Acceptance exclusion).

**Fixture-authoring constraints** (mirror the existing
`construction_pass1.rs:785-807` style): tests are single-file
`#![no_std]` Rust fixtures compiled through the real rustc
driver path. Each fixture must compile cleanly through rustc's
type-check stage so that construction can actually SEE the
user-data field; otherwise rustc's own errors mask the
intended construction-pass rejection.

Per-type fixture guidance:

- **Enum rejected**: use a `#[derive(Copy, Clone)]` C-style enum
  with a zero discriminant (e.g.,
  `#[derive(Copy, Clone)] enum E { A, B }`); otherwise non-Copy
  enums fail rustc first.
- **Union rejected**: use a `Copy` union
  (e.g., `#[derive(Copy, Clone)] union U { a: u32, b: u32 }`).
  Note: constructing a union requires `unsafe`; the negative
  test only needs the field type declared on the driver struct,
  not an active read, so no `unsafe` code is required.
- **`NonZeroU32` rejected**: import via
  `use core::num::NonZeroU32;`. `NonZeroU32` is `Copy`.
- **`NonNull<u8>` rejected**: import via
  `use core::ptr::NonNull;`. `NonNull<T>` is `Copy`.
- **Reference (`&'static u32`) rejected**: fixture uses
  `&'static u32` (references need a lifetime). Shared references
  are `Copy`.
- **Function pointer (`fn() -> ()`) rejected**: fn pointers are
  `Copy`.
- **`Box<u32>` rejected**: `Box` is NOT `Copy`. The test asserts
  that the outer `Copy` check blocks it (error variant differs
  from whitelist rejection). Requires `extern crate alloc;` and
  `use alloc::boxed::Box;`.
- **`Rc<u32>` rejected**: not `Copy`. Requires
  `extern crate alloc; use alloc::rc::Rc;`.
- **`Arc<u32>` rejected**: not `Copy`. Requires
  `extern crate alloc; use alloc::sync::Arc;`.
- **`Cell<u32>` rejected**: not `Copy`. Import via
  `use core::cell::Cell;`.
- **`RefCell<u32>` rejected**: not `Copy`. Import via
  `use core::cell::RefCell;`.
- **`UnsafeCell<u32>` rejected**: not `Copy`. Import via
  `use core::cell::UnsafeCell;`.
- **`AtomicU32` rejected**: not `Copy`. Import via
  `use core::sync::atomic::AtomicU32;`.
- **Raw pointer (`*mut u8`) rejected**: raw pointers ARE `Copy`.
  Fixture uses `*mut u8` directly.
- **Never type (`!`)**: **not exercised as a construction-pass
  negative test in Stage 1**. Declaring a struct field of type
  `!` requires the unstable `never_type` feature, which is not
  enabled in Regseq fixtures. `!` remains named in the design
  doc's "Explicitly rejected" section as future-proofing (if a
  future Rust stabilization makes `!` nameable, the Rule-1
  predicate must reject it), but no Stage-1 fixture declares a
  `!` field. A `fn() -> !` fixture would test the function-
  pointer arm, not `!` itself; do not conflate these.

Non-`Copy` types (Cell, RefCell, UnsafeCell, Box, Rc, Arc,
AtomicU32) are rejected by the outer `Copy` check (a different
error variant than whitelist rejection). The test asserts on
that specific variant (`UserDataNotCopy` or equivalent existing
variant per `construction.rs:146-154`); see the Category-A
error-variant specification in Acceptance Criteria.

Types that are `Copy` but not on the whitelist (enum, union,
NonZero*, NonNull, references, fn pointers, raw pointers; `!`
is not fixture-testable in Stage 1 per the note above) are
rejected by the Rule-1 whitelist predicate with the new
`UserDataNotZeroInitWhitelisted` variant.

**Category B -- generic / projection gate** (one test per,
Rule-1 predicate requires fully monomorphized types):

- Generic parameter `T` rejected (fails `TyKind::Param` gate).
- Unresolved associated-type projection (`<T as Trait>::Assoc`)
  rejected.

**Category C -- Stage-1 out-of-whitelist / deferred primitives
and aggregates** (one test per, valid design-level candidates
but not admitted by Stage-1):

- Signed integers: `i8`, `i16`, `i32`, `i64` each rejected.
- Platform-width integers: `usize`, `isize` each rejected.
- Floats: `f32`, `f64` each rejected.
- Also out-of-whitelist: `char`, `u128`, `i128` each rejected.
- **Tuple** (`(u32, u32)`): rejected because `RegseqType` has
  no tuple variant in Stage-1 scope.

### Integration test (end-to-end PL011 fixture)

Add or adapt a test that:

- Compiles the PL011 fixture at
  `tests/fixtures/pl011-full-spec/src/driver.rs` through the
  real Regseq compiler path.
- Confirms `imsc_shadow: u32` (bare, no `Option`) is accepted at
  construction.
- Confirms `imsc_shadow` flows through the model as
  `UserDataField { kind: UserDataKind::ZeroInit { inner_type:
  RegseqType::Scalar(ScalarType::U32) }, ... }`.
- Confirms the resulting raw-frontend artifact regenerates
  cleanly.
- Confirms the Phase-6 model pipeline succeeds on the fixture.
- Confirms the Linux emitter produces a driver struct containing
  `u32 imsc_shadow;` (no `Option`, no wrapper).

### Lowering-plan tests

Add tests in:

- `crates/regseq-backends/tests/lowering_from_model.rs`
  (pipeline 1): user-data obligations are populated with
  correct `UserDataKind` + `RegseqType` + field-name data.
- `crates/regseq-backends/tests/lowering_boundary.rs`
  (cross-pipeline asymmetry test): the RegseqModel-based
  pipeline populates `ResourceObligations.user_data` with the
  expected entries; the legacy pipeline leaves it empty. This
  asserts the Stage-1 Path A asymmetry. A future task that
  retires the legacy pipeline (or extends it to carry
  user-data) can update this test to assert equivalence.
- `crates/regseq-backends/tests/property_realization_honoring.rs`
  (Linux-plan): scalar user-data fields appear in
  `shared_struct_fields` at the correct position (after serial
  port, at the end of the struct-field list) with correct
  `c_type` strings (`u32`, `u8`, `bool`, etc.).

### Linux-plan rejection tests (non-scalar user-data)

**Stage-1 Linux-plan acceptance policy**: only
`UserDataKind::ZeroInit { inner_type }` where `inner_type` is a
scalar (u8/u16/u32/u64/bool) is admitted to
`shared_struct_fields`. All other user-data shapes are rejected
with `LinuxPlanError::UnsupportedUserDataShape`. Specifically:

- `UserDataKind::ZeroInit { inner_type: scalar }`: **accepted**,
  emits `<c_type> <field_name>;` via
  `LinuxUserDataScalar::c_type`.
- `UserDataKind::ZeroInit { inner_type: Array(...) }`: **rejected**.
- `UserDataKind::ZeroInit { inner_type: Defined(...) }` (Copy
  struct): **rejected**.
- `UserDataKind::Option { inner_type: ANY }`: **rejected**. Rule-2
  `Option<T>` user-data is not yet C-emittable; the Stage-1
  narrowing admits only Rule-1 ZeroInit scalars. (Before this
  task, no user-data was emitted to Linux C at all, so this is
  a new-capability narrowing, not a regression.)

Add tests confirming these outcomes:

- **Accepted**: scalar `UserDataKind::ZeroInit` for each of
  u8/u16/u32/u64/bool produces a `LinuxSharedStructField` with
  the correct `c_type` string.
- **Rejected with `UnsupportedUserDataShape`** (one test per
  category):
  - `UserDataKind::ZeroInit { inner_type: Array(Scalar(U32), 4) }`
    (Rule-1 array; e.g., driver field `[u32; 4]`).
  - `UserDataKind::ZeroInit { inner_type: Defined(...) }`
    (Rule-1 Copy struct via a synthetic model fixture; note
    that the construction path admits this shape but the
    Linux planner does not).
  - `UserDataKind::Option { inner_type: Scalar(U32) }` (Rule-2
    scalar Option; e.g., driver field `Option<u32>`).
  - `UserDataKind::Option { inner_type: Defined(...) }` (Rule-2
    Copy-struct Option; e.g., driver field `Option<Stats>`).

The last test confirms that Rule-2 `Option<CopyStruct>` is
rejected at Linux-plan construction with a clear error, matching
the Stage-1 narrowing. This is distinct from construction-pass
acceptance of `Option<Stats>` as a valid user-data shape
(Rule 2 at the model level accepts it; Linux-plan does not
admit it to C emission in Stage 1).

Tests belong in
`crates/regseq-backends/tests/property_realization_honoring.rs`
(or an adjacent Linux-plan test file in `crates/regseq-backends/tests/`).

### Verifier tests

Extend `crates/regseq-model/tests/verify_phase_2.rs` (at the
existing user-data test patterns at lines 344-367):

- `UserDataKind::ZeroInit` with each accepted inner type passes
  verification.
- `UserDataKind::Option` with an accepted Copy inner type passes
  verification.
- Mismatched kind vs inner_type (e.g., `ZeroInit` with an
  obviously rejected type) fails verification with a clear
  diagnostic.

### Normalization tests

Extend `crates/regseq-model/tests/normalize.rs` (at the existing
user-data test patterns at lines 241-339). The current
`UserDataField` derives `Serialize` only (no `Deserialize`, per
`crates/regseq-model/src/model/resources.rs:310`); tests assert
on the serialized output shape rather than a full round-trip
unless the worker opts to add `Deserialize` as part of this
task. Specific tests:

- **Rule-2 construction from raw artifact** (Rule-2 regression):
  input raw field-type spelling `"Option<u32>"` produces a
  `UserDataField` with
  `kind = UserDataKind::Option { inner_type: RegseqType::Scalar(ScalarType::U32) }`.
  Assert the exact struct shape via pattern-match or serialize
  to JSON and compare with a fixed expected string.
- **Rule-1 scalar construction** (new positive):
  - Input `"u8"` produces `kind = UserDataKind::ZeroInit { inner_type: RegseqType::Scalar(ScalarType::U8) }`.
  - Input `"u16"` produces `kind = UserDataKind::ZeroInit { inner_type: RegseqType::Scalar(ScalarType::U16) }`.
  - Input `"u32"` produces `kind = UserDataKind::ZeroInit { inner_type: RegseqType::Scalar(ScalarType::U32) }`.
  - Input `"u64"` produces `kind = UserDataKind::ZeroInit { inner_type: RegseqType::Scalar(ScalarType::U64) }`.
  - Input `"bool"` produces `kind = UserDataKind::ZeroInit { inner_type: RegseqType::Bool }`.
- **Rule-1 array construction** (new positive): input
  `"[u32; 4]"` produces
  `kind = UserDataKind::ZeroInit { inner_type: RegseqType::Array(Box::new(RegseqType::Scalar(ScalarType::U32)), 4) }`.
- **Stage-1 whitelist tightening** (new negative):
  - Input `"u128"` produces a normalization error (previously
    accepted per `surface.rs:1102`; now tightened).
  - Input `"i128"` produces a normalization error.
  - Input `"char"` produces a normalization error.
  - Input `"(u32, u32)"` produces a normalization error (tuple
    rejection).
- **Serialization shape** (asserts the new enum variants
  serialize correctly via `serde_json`). The serde attribute
  mandated at touch-point 4 is
  `#[serde(tag = "kind", content = "data")]`, matching
  `RegseqType` at `crates/regseq-model/src/ir/types.rs:16-18`.
  Assert on these exact expected JSON payloads (pretty-print
  normalized; use `serde_json::to_value` + structural comparison
  or `serde_json::to_string` + exact string match with the
  worker's preferred whitespace convention):

  Field with `kind = UserDataKind::ZeroInit { inner_type: RegseqType::Scalar(ScalarType::U32) }`
  produces JSON for the `kind` slot equal to:

  ```json
  {
    "kind": "ZeroInit",
    "data": {
      "inner_type": {
        "kind": "Scalar",
        "data": {
          "kind": "U32"
        }
      }
    }
  }
  ```

  Field with `kind = UserDataKind::Option { inner_type: RegseqType::Scalar(ScalarType::U32) }`
  produces JSON for the `kind` slot equal to:

  ```json
  {
    "kind": "Option",
    "data": {
      "inner_type": {
        "kind": "Scalar",
        "data": {
          "kind": "U32"
        }
      }
    }
  }
  ```

  Field with `kind = UserDataKind::ZeroInit { inner_type: RegseqType::Bool }`
  produces JSON for the `kind` slot equal to:

  ```json
  {
    "kind": "ZeroInit",
    "data": {
      "inner_type": {
        "kind": "Bool"
      }
    }
  }
  ```

  (The `Bool` variant has no data payload since it is a unit
  variant in `RegseqType`; serde's adjacently-tagged
  representation omits the `"data"` key for unit variants.
  Adjust the expected JSON if the worker observes different
  behavior under the pinned serde version, but do not change
  the serde tagging attribute.)

### Golden-regeneration sub-step

After all tests pass, regenerate goldens per the Implementation
Notes section and commit them in the same commit as the
implementation changes. The regenerated
`expected/raw-frontend.json` must be consumed successfully by
every downstream test that depends on it (there are 13 such
tests across `regseq-backends`, `regseq-analysis`,
`regseq-model`, `regseq-driver`); all must stay green.

## Review

The verifier agent must confirm every one of the following
points. Each is anchored to a mechanically verifiable artifact
(file path + test name + diff property). If any point cannot be
verified, the task is not complete.

### Touch-point coverage

1. Each of the 13 TODO checklist entries has a concrete diff
   entry in the commit. No site remains at the old
   `Option<T>`-only contract.
2. `UserDataField` is an enum-with-struct-and-nested-kind shape
   per touch-point 4; `id` and `field_name` are preserved on the
   outer struct; `UserDataKind` has exactly two variants
   (`Option`, `ZeroInit`), both carrying `inner_type: RegseqType`.
3. Every existing consumer of `UserDataField` (grep for
   `u.inner_type` and similar) has been updated to match on
   `UserDataKind` with both variants handled.
4. The cycle guard in the Rule-1 predicate uses monomorphized-
   type keys (`(DefId, GenericArgsRef)` or `Ty<'tcx>`), NOT
   `DefId`-only. Verify by reading the predicate's visited-set
   implementation.
5. The outer `Copy` check at `driver.rs:227` is preserved for
   Rule-1 user-data fields (not removed or bypassed).
6. The Rule-1 predicate rejects `TyKind::Param` and unresolved
   projections BEFORE applying the whitelist.

### Stage-1 whitelist tightness

7. The primitive whitelist admits exactly `u8, u16, u32, u64,
   bool` -- no more, no less. Every other primitive has a
   corresponding negative test that passes (fails construction
   with the expected error).
8. The existing Rule-2 primitive Copy table at
   `crates/regseq-model/src/normalize/surface.rs:1111-1129` is
   **UNCHANGED** (keeps `u128`, `i128`, `char`, all signed ints,
   floats -- Rule-2 acceptance is NOT tightened to the Rule-1
   whitelist). A **separate new Rule-1 primitive whitelist**
   (admitting only `u8, u16, u32, u64, bool`) lives adjacent to
   the existing table and is consulted only by the Rule-1
   zero-init classification path. Verify by reading the diff
   that the existing table is untouched and the new table
   contains only five variants.
9. Tuples are rejected with a Category-C negative test.
10. The `LinuxUserDataScalar` enum in `linux/plan.rs` exactly
    mirrors the whitelist: `U8, U16, U32, U64, Bool`. The
    `parse_from_regseq_type` method returns `Some(_)` for the
    five accepted variants and `None` for every other
    `RegseqType` (including `Array`, `Defined`, etc.).
11. `LinuxPlanError::UnsupportedUserDataShape` variant exists on
    `planner.rs` with a struct-like payload mirroring the
    `UnsupportedPropertyScalar` shape at `planner.rs:46-56`.
    Linux plan construction calls this variant's constructor
    whenever a user-data field's `RegseqType` is non-scalar
    (i.e., `LinuxUserDataScalar::parse_from_regseq_type` returns
    `None`). No `user_data_aggregate_to_c_type` helper exists.

### Scope boundary preservation

12. `crates/regseq-backends/src/linux/emit_semantic.rs:216` is
    UNCHANGED. Verify via `git diff` that line 216 and its
    surrounding function body have no modifications.
13. `crates/regseq-backends/src/linux/emit_module.rs:703` is
    UNCHANGED. Verify same way.
14. `crates/regseq/src/mmio.rs:72-86` (the `Mmio::modify` body with `todo!()`
    gap) is UNCHANGED.
15. No changes land outside the authorized implementation
    scope:
    (a) Source code in `crates/regseq-driver`, `crates/regseq-model`,
        and `crates/regseq-backends` only (the four crate
        surfaces named in this task).
    (b) Fixture-golden updates under
        `tests/fixtures/pl011-full-spec/expected/` (regenerated
        `raw-frontend.json`, `regseq-model.json`, and
        `ir/*.json`).
    Any file changed outside these two categories is an
    out-of-scope modification. Verify via
    `git diff --name-only <base>..HEAD`. The task spec file
    itself and the Codex review transcripts for the
    brief-authoring batch are pre-existing harness artifacts
    produced by the spec-authoring phase (see
    `.harness/spec-batches/user-data-init-path-a.md`); they are
    NOT worker-owned implementation artifacts and should not
    appear in the implementation diff.
16. No changes land in `shadow-registers.md` or
    `shadow-registers-codex-review/`. Shadow-register is a
    separate workstream.

### End-to-end PL011 validation

17. PL011 fixture compiles. `imsc_shadow: u32` is accepted at
    construction with no `Option` wrapping.
18. PL011's `UserDataField` for `imsc_shadow` has
    `kind: UserDataKind::ZeroInit { inner_type:
    RegseqType::Scalar(ScalarType::U32) }`.
19. Regenerated `expected/raw-frontend.json`,
    `expected/regseq-model.json`, and `expected/ir/*.json` are
    committed in the same commit as the implementation.
20. All downstream tests that read the PL011 full-spec
    `expected/raw-frontend.json` pass. The precise test list
    (enumerable via
    `grep -rln 'expected/raw-frontend.json' crates/ | xargs grep -l 'pl011-full-spec'`):
    - `crates/regseq-driver/tests/semantic_model_projection.rs`
    - `crates/regseq-model/tests/populate_bindings.rs`
    - `crates/regseq-model/tests/normalize.rs`
    - `crates/regseq-model/tests/pl011_pipeline_phase6.rs`
    - `crates/regseq-model/tests/lint_findings.rs`
    - `crates/regseq-model/tests/query_api.rs`
    - `crates/regseq-analysis/tests/readiness.rs`
    - `crates/regseq-backends/tests/semantic_body_lowering.rs`
    - `crates/regseq-backends/tests/lowering_from_model.rs`
    - `crates/regseq-backends/tests/pl011_lowering_completeness_proof.rs`
    - `crates/regseq-backends/tests/pl011_full_spec_lowering.rs`
    - `crates/regseq-backends/tests/linux_emission_scaffold.rs`
    - `crates/regseq-backends/tests/lowering_boundary.rs`
    Every test in this list must pass against the regenerated
    `expected/raw-frontend.json`. Unrelated tests (e.g., those
    reading the PL011 spec-slice fixture at
    `crates/regseq-model/tests/declaration_reviewability.rs`) are
    not in scope for this acceptance gate.
21. Linux emission of the PL011 driver struct contains
    `u32 imsc_shadow;` as a field -- no wrapper, no `Option`,
    no post-kzalloc initialization store.

### Test coverage completeness

22. Every Rule-1 positive arm (scalar, array, `Copy` struct,
    nested struct) has at least one test.
23. Rule-2 regression tests are present and mandatory:
    - `Option<u32>` passes at construction.
    - `Option<u8>` passes at construction.
    - `Option<i32>` passes at construction (critical: confirms
      Rule-2 is NOT tightened to the Rule-1 whitelist; `i32`
      is `Copy` and must remain accepted under Rule 2).
    - `Option<Copy struct>` (e.g., `Option<Stats>`) passes at
      construction (preserves existing
      `construction_pass1.rs:309-321` coverage; asserts the
      resulting `UserDataField.kind` is
      `UserDataKind::Option { inner_type: RegseqType::Defined(...) }`).
24. Every Category-A, -B, -C negative test is present and
    fails at construction (not at a later phase).
    **Exception**: the never type `!` has no Stage-1 Category-A
    test (declaring a `!` field requires the unstable
    `never_type` feature). This exception is documented in the
    Test Plan and does not count as a missing test.
25. Integration test exercises the full Rule-1 path end-to-end
    on the PL011 fixture.

### Quality gates

26. `cargo test -p regseq-driver` passes.
27. `cargo test -p regseq-model` passes.
28. `cargo test -p regseq-backends` passes.
29. `cargo test --workspace` passes.
30. `cargo fmt --all -- --check` passes.
31. `cargo clippy --workspace -- -D warnings` passes.

If all 31 points verify, approve. If any fail, return
`REQUIRES FIXES` with a numbered list anchored to the failing
point.

## Quality Gate

```
cargo test -p regseq-driver
cargo test -p regseq-model
cargo test -p regseq-backends
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
```

All six commands must pass cleanly before `scripts/python/task.py
done regseq-model-64` is invoked.

## Acceptance Criteria

- All 13 TODO checklist entries have concrete diff entries.
- `UserDataField` has been extended to the enum-with-nested-kind
  shape specified in touch-point 4; all existing consumers have
  been updated.
- All positive tests (Rule-1 arms + Rule-2 tier + integration
  test) pass.
- All negative tests (Categories A, B, C) pass (types are
  rejected at construction with the specific error variants
  named by this task. Each negative test asserts on a specific
  variant, not on a string match or a generic error type:

  - Category A non-`Copy` types (`Box`, `Rc`, `Arc`, `Cell`,
    `RefCell`, `UnsafeCell`, `AtomicU32`): fail with the
    pre-existing `UserDataNotCopy` variant at
    `construction.rs:146-154`. The outer `Copy` check blocks
    them before the whitelist predicate runs.
  - Category A `Copy` types (enum, union, `NonZeroU32`,
    `NonNull`, references, fn pointers, raw pointers; `!`
    is not fixture-testable in Stage 1):
    fail with the new `UserDataNotZeroInitWhitelisted` variant
    added at touch-point 2.
  - Category B (`TyKind::Param`, unresolved projections): fail
    with the new `UserDataNotZeroInitWhitelisted` variant. The
    Rule-1 predicate rejects these BEFORE applying the whitelist
    per touch-point 1, so they never reach whitelist walking;
    the error still surfaces through the same variant with a
    payload indicating the rejected type.
  - Category C (out-of-whitelist primitives like `i32`,
    `usize`, `f32`; and tuples): fail with
    `UserDataNotZeroInitWhitelisted`.
  - Non-scalar user-data reaching Linux plan construction
    (Rule-1 accepted at model level; fails at Linux plan):
    fails with `LinuxPlanError::UnsupportedUserDataShape` added
    at touch-point 12b.
- Stage-1 scope boundaries are preserved: no changes to
  `emit_semantic.rs:216`, `emit_module.rs:703`, or
  `Mmio::modify`'s `todo!()`.
- PL011 fixture compiles with `imsc_shadow: u32` (bare u32). The
  emitted Linux driver struct contains `u32 imsc_shadow;` as a
  field.
- Regenerated goldens (`expected/raw-frontend.json`,
  `expected/regseq-model.json`, `expected/ir/*.json`) are
  committed in the same commit as the implementation.
- All 13 downstream tests that read
  `expected/raw-frontend.json` pass.
- `cargo test --workspace`, `cargo fmt --all -- --check`, and
  `cargo clippy --workspace -- -D warnings` all pass cleanly.
- The worker's implementation Codex review (per the SESSION.md
  "Review" step) returns `VERDICT: APPROVED`. This is the
  implementation-level Codex review, NOT the spec-authoring
  review. The spec-authoring Codex transcripts (brief and task
  spec reviews) are owned by the spec-authoring batch at
  `.harness/spec-batches/user-data-init-path-a.md` and are
  outside this task's acceptance criteria.
