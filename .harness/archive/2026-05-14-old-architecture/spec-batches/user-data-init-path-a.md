# Spec Batch: user-data-init-path-a -- User-data initialization contract (Path A)

## Description

This batch authors a single executable harness task that implements
**Path A** of the user-data initialization contract settled in
`docs/discussions/models/regseq-model/user-data-initialization.md`.
Path A extends the Regseq compiler's user-data acceptance predicate
from its current `Option<T: Copy>`-only rule to a two-rule contract:
either `Option<T: Copy>` (Rule 2, unchanged) or a compiler-controlled
zero-init whitelist of primitive scalars and aggregates (Rule 1).

The immediate blocker this unblocks: the PL011 Stage-1 fixture at
`tests/fixtures/pl011-full-spec/src/driver.rs:72` introduced
`imsc_shadow: u32` as a software shadow of the IMSC register per
REQ-CORR-MASK-SHADOW. The current construction rule at
`crates/regseq-driver/src/construction/driver.rs:249` rejects bare
`u32` with `InvalidUserDataShape`, which prevents the compiler
from building the fixture and blocks every downstream workflow
(raw-frontend regeneration, Phase-6 model golden, IR goldens,
Linux reference C co-authoring).

This batch authors executable task specs, not implementation.
Scope is a single task covering all twelve cross-crate touch-points
identified in the design doc. The task spans four crates
(`regseq-driver`, `regseq-model`, `regseq-backends` shared lowering,
`regseq-backends/linux`). This is wider than the
`task-spec-workflow.md:120-121` guidance of "one crate or a tight
crate pair," but is an intentional override: the change is one
coherent feature whose touch-points cannot land in isolation
without leaving the workspace broken, and splitting introduces
inter-commit coordination friction for limited benefit at this
scale.

## Input Context

Read before starting:

- `.harness/config.md`
- `.harness/SESSION.md`
- `.harness/task-spec-workflow.md`
- `.harness/task-spec-status.yaml`
- `.harness/tasks.json` (executable-task layer; update required
  per workflow lines 126-128)
- `.harness/spec-batches/BRIEF_TEMPLATE.md`
- `docs/plans/asplos-implementation-plan.md` (workflow line 107
  requires elaboration sessions to read the plan. No specific
  section of the plan governs this post-Phase-7 retrofit: the
  authoritative specification is the design doc at
  `docs/discussions/models/regseq-model/user-data-initialization.md`.
  The plan is listed here to satisfy the workflow contract; the
  worker may treat it as background context unless the plan has
  been amended to cover user-data initialization.)
- `docs/discussions/models/regseq-model/user-data-initialization.md`
  (the authoritative design doc; all decisions are frozen here)
- `docs/discussions/models/regseq-model/user-data-initialization-codex-review/README.md`
  (summary of the three-round Codex debate; no re-debate
  permitted)
- `docs/discussions/models/regseq-model/user-data-initialization-codex-review/round-3-final-design.md`
  (the converged review round; lists all 12 touch-points and
  all amendments)
- `crates/regseq-driver/src/construction/driver.rs:216-253` (the
  construction rule being extended)
- `crates/regseq-driver/src/construction/driver.rs:227,562`
  (existing `Copy` trait-check helper and the
  `TypingEnv::fully_monomorphized()` gate the whitelist predicate
  must reuse; required by the design doc's Rule 1 outer-`Copy`
  preservation clause)
- `crates/regseq-driver/src/construction.rs:127,146,451,467`
  (`ConstructionError` variants + docs + display text)
- `crates/regseq-model/src/model/resources.rs:306-318` (current
  `UserDataField` shape with `id`, `field_name`, `inner_type`
  fields)
- `crates/regseq-model/src/ir/types.rs:18-29` (`RegseqType`
  enum with `Scalar`, `Bool`, `Array`, `Defined`, etc.; no
  `Tuple` variant exists today)
- `crates/regseq-model/src/ir/types.rs:52-63` (`ScalarType`
  enum with integer variants only; `bool` is handled via
  `RegseqType::Bool`, not `ScalarType`)
- `crates/regseq-model/src/model/type_def.rs:34-57`
  (`TypeDef` and related types for user-defined struct/enum
  representation)
- `crates/regseq-model/src/verify/mod.rs:205`
  (`UserDataFieldNotOptionCopy` error kind)
- `crates/regseq-model/src/verify/rules/driver_struct.rs:11,212,239`
  (verifier rules + error text)
- `crates/regseq-model/src/verify/rules/cross_item.rs:348`
  (cross-item user-data type resolution)
- `crates/regseq-model/src/normalize/surface.rs:1044,1081,1102`
  (raw/legacy normalization enforcing `Option<T: Copy>`; primitive
  Copy table diverges from the Stage-1 whitelist and must be
  tightened)
- `crates/regseq-backends/src/lowering/plan.rs:109` (where
  `ResourceObligations` lives)
- `crates/regseq-backends/src/lowering/from_model.rs:190`
  (one of two lowering pipelines; populates user-data)
- `crates/regseq-backends/src/lowering/planner.rs:311`
  (the other lowering pipeline; also populates user-data)
- `crates/regseq-backends/src/linux/planner.rs:146` (where
  `shared_struct_fields` is built)
- `crates/regseq-backends/src/linux/plan.rs:400` (Linux
  scalar-to-C mapper; already supports Stage-1 whitelist)
- `crates/regseq-backends/src/linux/emit_semantic.rs:216`
  (current type emitter that panics on unknown structs; see
  Implementation Notes for its Stage-1 deferral)
- `crates/regseq-backends/src/linux/emit_module.rs:703`
  (existing `devm_kzalloc` call; NO CHANGE required for this
  task per the design doc's "Backend kzalloc path" section at
  `user-data-initialization.md:307-312`. `devm_kzalloc` already
  zero-initializes the whole struct; Stage-1 Rule-1 user-data
  fields require no post-allocation stores. Listed here as a
  no-change anchor the task spec must explicitly preserve.)

Existing test files to mirror (implementation anchors -- match
these styles for new tests in this task):

Construction/model tests (existing user-data patterns):
- `crates/regseq-driver/tests/construction_pass1.rs:783-815`
  (existing user-data construction tests; the positive/negative
  tests for Rule-1 should extend this file's patterns)
- `crates/regseq-model/tests/normalize.rs:241-339` (existing
  user-data normalization tests; the `UserDataField` enum
  extension tests belong here)
- `crates/regseq-model/tests/verify_phase_2.rs:344-367` (existing
  verifier tests for user-data; the updated verifier rule's
  tests belong here)
- `crates/regseq-model/tests/pl011_pipeline_phase6.rs:126-134`
  (Phase-6 model golden regeneration; this test owns the
  `REGEN_GOLDEN=1` path that the task must use to regenerate
  `regseq-model.json`)

Backend/lowering tests (existing user-data-adjacent patterns the
lowering-plan and Linux-planner tests should mirror):
- `crates/regseq-backends/tests/lowering_from_model.rs:1-10`
  (existing lowering-plan construction tests; the user-data
  obligation tests belong here)
- `crates/regseq-backends/tests/lowering_boundary.rs:1-15`
  (existing lowering boundary tests covering both pipelines;
  the user-data carriage tests belong here)
- `crates/regseq-backends/tests/property_realization_honoring.rs:1-9`
  (existing Linux-plan shared-struct-fields tests; the
  user-data-inclusion tests belong here)

## Output Specification

Create:

- `.harness/tasks/regseq-model/64-user-data-init-path-a.md` --
  the executable task spec.
- `docs/discussions/models/regseq-model/user-data-initialization-codex-review/round-4-brief-review.md`
  -- the final-round Codex transcript of the brief review,
  required by the Acceptance Criteria below. Must carry an
  `APPROVED` first-line verdict.
- `docs/discussions/models/regseq-model/user-data-initialization-codex-review/round-5-task-spec-review.md`
  -- the final-round Codex transcript of the task-spec review,
  required by the Acceptance Criteria below. Must carry an
  `APPROVED` first-line verdict. Intermediate rounds from either
  review may optionally be preserved as additional `round-N-*.md`
  files in the same directory.

Update when done:

- `.harness/tasks.json` -- add the new task entry under the
  `regseq-model` group; add a `depends_on` edge from
  `regseq-backends-02` to the new task.
- `.harness/task-spec-status.yaml` -- add a new tracker item
  with id `user-data-init-path-a`.

The output is one executable harness task spec plus two Codex
review transcripts plus associated tracker and graph updates.

## Implementation Notes

This batch exists to author ONE task spec. Do not split into
multiple task specs.

### Task-spec authoring constraints

The task spec must:

- Title the task "User-data initialization Path A: zero-init
  whitelist + Option tier".
- State explicitly that it spans four crates and cite
  `user-data-initialization.md` as authority for the cross-crate
  scope.
- Include an **Implementation Notes** section that lays out a
  TODO-style checklist with **thirteen** separate checklist
  entries (twelve numbered touch-points, with item 12 split
  into mandatory sub-items 12a and 12b for mechanical
  reviewability). The worker agent maintains this checklist via
  `TodoWrite` (or equivalent task-tracking) and marks each entry
  complete one-by-one. The entries:

  Construction (3):
  1. `crates/regseq-driver/src/construction/driver.rs:216-253` --
     replace strict `Option<T>` check with branch accepting either
     Rule 1 (zero-init whitelist) or Rule 2 (`Option<T: Copy>`);
     preserve the outer `Copy` check via the existing
     `type_is_copy_modulo_regions` helper at
     `driver.rs:227` (the Rule-1 path continues to use this for
     the outer field type; nested whitelist fields are implicitly
     `Copy` because `Copy` is closed under struct composition
     when every field is `Copy`). The
     `TypingEnv::fully_monomorphized()` helper at `driver.rs:562`
     is the correct typing environment for the monomorphized-
     type cycle guard; reuse it. Reject types with `TyKind::Param`
     or unresolved projections BEFORE applying the whitelist.
  2. `crates/regseq-driver/src/construction.rs:146,467` -- update
     error docs and display text that currently say "user-data
     must be `Option<T>`".
  3. Cycle guard: implement the recursive whitelist predicate
     with a visited-set keyed by monomorphized type
     (`(DefId, GenericArgsRef)` or `Ty<'tcx>`), NOT by `DefId`
     alone. Land alongside construction change.

  Model (5):
  4. `crates/regseq-model/src/model/resources.rs:306-318` --
     extend `UserDataField` with a Rule-1/Rule-2 discriminator
     while preserving the existing `id: UserDataId` and
     `field_name: Symbol` fields. Target shape:

     ```rust
     pub struct UserDataField {
         pub id: UserDataId,
         pub field_name: Symbol,
         pub kind: UserDataKind,
     }

     #[derive(Clone, Eq, PartialEq, Debug, Serialize)]
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

     The `UserDataField` struct keeps `id` and `field_name`
     outside the enum so existing consumers that read those
     fields continue to work. The `inner_type: RegseqType` that
     currently lives on `UserDataField` moves into each
     `UserDataKind` variant; every match on `UserDataKind` must
     handle both variants. (The design doc's "Open follow-ups
     (for implementation phase)" section at
     `user-data-initialization.md:376-377` explicitly names
     "decide the exact shape of the `UserDataField` model
     extension (enum variant vs classification flag) during
     implementation" as the open question. Per user direction
     2026-04-22, this task fixes the choice to the enum-variant
     shape for exhaustive-match semantic clarity. The task spec
     must cite this narrowing as an intentional implementation
     decision, not as a design-doc deviation.)
  5. `crates/regseq-model/src/verify/mod.rs:205` -- rename or
     redocument the `UserDataFieldNotOptionCopy` error kind to
     reflect the two-rule contract.
  6. `crates/regseq-model/src/verify/rules/driver_struct.rs:212,239` --
     update verifier logic and diagnostic text that currently
     require `Option<T>`.
  7. `crates/regseq-model/src/verify/rules/cross_item.rs:348` --
     cross-item type resolution must follow the new classified
     stored type.
  8. `crates/regseq-model/src/normalize/surface.rs:1044,1081,1111-1129`
     -- raw/legacy normalization currently enforces
     `Option<T: Copy>` and stores every user-data field as
     `Scalar(U32)`. The existing Rule-2 Copy table at lines
     1111-1129 (which includes `u128`, `i128`, `char`, signed
     ints, floats) must REMAIN UNCHANGED per the design doc's
     "Rule 2 unchanged" contract. A SEPARATE new Rule-1 zero-
     init primitive whitelist (admitting only
     `u8, u16, u32, u64, bool`) is added adjacent to it and is
     consulted only by the new zero-init classification path.

  Lowering (3):
  9. `crates/regseq-backends/src/lowering/plan.rs:109` -- add
     user-data to `ResourceObligations` (currently MMIO/IRQ/
     property/clock/reset/pinctrl only).
  10. `crates/regseq-backends/src/lowering/from_model.rs:190` --
      the RegseqModel-based lowering pipeline; populate user-data
      from `DriverStruct.user_data`.
  11. `crates/regseq-backends/src/lowering/planner.rs:311` -- the
      legacy lowering pipeline (deprecated per Phase-7
      migration). **Stage-1 Path A asymmetry**: legacy pipeline
      does NOT populate user-data; initialize to empty `Vec`
      only. Extending it would cascade through `DeclarationModel`
      + raw-input + legacy normalization and is out of Path A
      scope. See design doc's Lowering touch-points section for
      the asymmetry rationale.

  Linux backend (2 items -- split for mechanical reviewability):

  12a. `crates/regseq-backends/src/linux/planner.rs:146` --
      include user-data in `shared_struct_fields` (currently
      built from resources/properties/serial-port only). Each
      `UserDataField` on the driver becomes a C struct field
      emitted at the correct position. Mirror the existing
      field-building patterns for properties and serial-port
      fields already present in `planner.rs`. (No shadow-register
      filtering or interaction is part of this task;
      shadow-register support is a separate deferred workstream
      at `docs/discussions/models/regseq-model/shadow-registers.md`.)

  12b. `crates/regseq-backends/src/linux/plan.rs` -- add
      user-data C-type mapping infrastructure. Today only
      property scalars have a C-type mapping, via the
      `LinuxPropertyScalar` enum + `LinuxPropertyScalar::c_type`
      at `crates/regseq-backends/src/linux/plan.rs:400-433`
      (covers `u8, u16, u32, u64, bool`). Follow the same pattern
      for user-data:

      - Introduce `LinuxUserDataScalar` at
        `crates/regseq-backends/src/linux/plan.rs`, a narrow enum
        exactly matching the Stage-1 primitive whitelist:
        ```rust
        pub enum LinuxUserDataScalar { U8, U16, U32, U64, Bool }
        ```
        Note: `bool` cannot be represented by the model's existing
        `ScalarType` enum (which has only integer variants per
        `crates/regseq-model/src/ir/types.rs:52-63`). The mapping
        from `RegseqType` to `LinuxUserDataScalar` must handle
        `RegseqType::Scalar(ScalarType::U{8,16,32,64})` for the
        integer arm and `RegseqType::Bool` for the bool arm
        (per `crates/regseq-model/src/ir/types.rs:18-20`).
      - `LinuxUserDataScalar::parse_from_regseq_type(&RegseqType) -> Option<Self>`
        -- parses a `RegseqType` into the narrow enum. Returns
        `None` on unknown/out-of-whitelist types. Mirrors
        `LinuxPropertyScalar::parse`.
      - `LinuxUserDataScalar::c_type(self) -> &'static str`
        -- returns the Linux kernel spelling (e.g., `"u32"`).
        Exhaustive match; no default arm.
      - `user_data_aggregate_to_c_type(&RegseqType) -> Result<String, LinuxPlanError>`
        -- builds a C type expression for aggregate Rule-1
        shapes. Walks the `RegseqType` directly:
        - `RegseqType::Scalar(_)` or `RegseqType::Bool`:
          delegate to `LinuxUserDataScalar::parse_from_regseq_type(_).map(|s| s.c_type())`.
        - `RegseqType::Array(elem, n)`: recurse on `elem`,
          return `"<elem>[<n>]"`.
        - `RegseqType::Defined(type_def_id)`: look up the
          `TypeDef` by id; if it is a plain `Copy` struct of
          accepted fields (the Rule-1d shape the construction
          predicate admits), return the struct's C name.
        - Any other variant returns `Err(LinuxPlanError::
          UnsupportedUserDataShape)`.

      Together these helpers cover the Stage-1 Rule-1 shapes
      that the model admits: scalar (via
      `LinuxUserDataScalar::c_type`), array (via
      `user_data_aggregate_to_c_type` Array arm), and `Copy`
      struct (via `user_data_aggregate_to_c_type` Defined arm).
      Tuples are explicitly NOT supported by this batch: the
      model's `RegseqType` has no tuple variant, and adding one
      is out of scope for Path A. See the "Stage-1 Rule-1
      whitelist narrowing" subsection in the design doc for the
      updated scope. This satisfies the round-3 backend-
      realizability requirement at
      `docs/discussions/models/regseq-model/user-data-initialization-codex-review/round-3-final-design.md:90-95`.

      **Stage-1 backend-tested scope is narrowed to scalars only**
      per the design doc's "Stage-1 backend-tested scope (explicit
      narrowing)" section at
      `docs/discussions/models/regseq-model/user-data-initialization.md`.
      The model + lowering plan accept the Stage-1 Rule-1 whitelist
      (scalars, arrays, and `Copy` structs recursively; tuples
      are excluded). Linux **plan construction** is the enforcement
      point for Stage-1 backend scope: non-scalar user-data fields
      are rejected with a new `LinuxPlanError::UnsupportedUserDataShape`
      variant BEFORE being inserted into `shared_struct_fields`.
      The C-AST renderer at `c/mod.rs:232-238` (which produces
      `<type_name> <field_name>;` from a
      `LinuxSharedStructField.c_type` string via
      `linux/emitter.rs:152-160`) therefore never sees an
      unrealizable type.

      **`emit_semantic.rs:216` is NOT on the shared-header
      emission path and is NOT touched in this task.** Previous
      framing that described `emit_semantic.rs:216` as a
      "panic signal for non-scalar user-data" was factually
      incorrect: `emit_semantic.rs` handles a different code
      emission path (function-body semantic IR lowering), not
      shared-struct-field C-type rendering.

      **No `user_data_aggregate_to_c_type` helper lands in
      Stage 1.** Rendering arrays/structs correctly requires
      a richer C declarator AST than the current renderer
      supports (arrays must render as `u32 field[4];`, not
      `u32[4] field;`). A future task will add both the
      aggregate helper AND the declarator AST together; Stage 1
      lands only the scalar path (`LinuxUserDataScalar` enum)
      plus the Linux-plan rejection of non-scalar user-data.
      This satisfies the round-3 Codex backend-realizability
      requirement at
      `docs/discussions/models/regseq-model/user-data-initialization-codex-review/round-3-final-design.md:90-95`
      via the "constrain accepted backend-tested shapes" option.

      Task-spec acceptance criteria MUST confirm that this Stage-1
      scope narrowing is documented as intentional with a citation
      to the design doc's narrowing section, not mistaken for an
      oversight.

- Include a **Test Plan** section with:

  **Positive tests** (Rule 1 arms, one test each):
  - Scalar: `u8`, `u16`, `u32`, `u64`, `bool` accepted.
  - Array: `[u32; 4]`, `[u8; 16]` accepted.
  - `Copy` struct of Tier-1 primitives:
    ```rust
    #[derive(Copy, Clone)]
    struct Counters { rx: u32, tx: u32 }
    ```
    with a driver field `counters: Counters` accepted.
  - Nested struct:
    ```rust
    #[derive(Copy, Clone)]
    struct Inner { x: u8 }
    #[derive(Copy, Clone)]
    struct Outer { inner: Inner }
    ```
    with a driver field `state: Outer` accepted.

  **Positive tests** (Rule 2 tier, one test each):
  - `Option<u32>` accepted (unchanged behavior).
  - `Option<u8>` accepted.

  **Negative tests** are organized into three categories,
  matching the distinct design-doc sections they come from:

  **Category A -- explicitly rejected types** (one per category
  from the design doc's "Explicitly rejected" section at lines
  79-108; each listed type is rejected because zero-init is
  either invalid or semantically unsupported):
  - Enum rejected (even zero-discriminant).
  - Union rejected.
  - `NonZeroU32` rejected.
  - `NonNull<u8>` rejected.
  - Reference (`&u32`) rejected.
  - Function pointer (`fn()`) rejected.
  - `Box<u32>` rejected.
  - `Rc<u32>` rejected.
  - `Arc<u32>` rejected.
  - `Cell<u32>` rejected (not `Copy`; outer `Copy` check blocks it).
  - `RefCell<u32>` rejected.
  - `UnsafeCell<u32>` rejected (not `Copy`).
  - `AtomicU32` rejected (not `Copy`).
  - Raw pointer (`*mut u8`) rejected.
  - Never type (`!`) rejected.

  **Category B -- generic / projection gate** (from the Rule-1
  contract at design doc lines 58-62; rejected because the
  predicate requires fully monomorphized types):
  - Generic parameter `T` rejected (fails `TyKind::Param` gate).
  - Unresolved associated-type projection (`<T as Trait>::Assoc`)
    rejected.

  **Category C -- Stage-1 out-of-whitelist / deferred primitives
  and aggregates** (from design doc "Full target whitelist
  (deferred)" section at lines 191-203 and the "Stage-1 Rule-1
  whitelist narrowing (tuples excluded)" section; these types
  are valid Rule-1 candidates at the design level but are not
  admitted by the Stage-1 scope because backend C-type mapping
  or IR type representation has not been extended to support
  them):
  - Signed integers: `i8`, `i16`, `i32`, `i64` each rejected.
  - Platform-width integers: `usize`, `isize` each rejected.
  - Floats: `f32`, `f64` each rejected.
  - Also out of the Stage-1 whitelist for independent reasons
    (from the design doc's "Explicitly rejected" section): `char`,
    `u128`, `i128` each rejected.
  - **Tuple** (`(u32, u32)`): rejected because `RegseqType` has
    no tuple variant in Stage-1 scope. This test exists to
    confirm the tuple exclusion; flips from negative to positive
    when a future task adds `RegseqType::Tuple`.

  The distinction between B and C matters for the verifier agent:
  category-B tests confirm the predicate's param-gate works;
  category-C tests confirm the primitive whitelist is
  Stage-1-tight (rejecting types that a future widening would
  accept). If a future task widens the whitelist to include,
  e.g., `i32`, the corresponding category-C test flips from
  negative to positive; category-A tests never flip.

  **Integration test**:
  - PL011 fixture compiles end-to-end with `imsc_shadow: u32`
    (no `Option` wrapping, no `.unwrap_or(0)` workaround). Raw
    frontend artifact regenerates successfully. Phase-6 model
    pipeline produces a valid model.

  **Golden-regeneration sub-step**:
  - After the rule change compiles, regenerate:
    - `tests/fixtures/pl011-full-spec/expected/raw-frontend.json`
      via the production `--emit=raw-artifact` path.
    - `tests/fixtures/pl011-full-spec/expected/regseq-model.json`
      via `REGEN_GOLDEN=1 cargo test -p regseq-model --test pl011_pipeline_phase6`.
    - `tests/fixtures/pl011-full-spec/expected/ir/*.json` via
      `UPDATE_GOLDENS=1 cargo test -p regseq-driver --test pl011_construction`.
  - Commit regenerated artifacts in the same commit as the
    implementation.

- Include an **extensive Review section** that enumerates what
  a verifier agent must confirm:
  - Every one of the twelve touch-points has a concrete diff
    entry; no site left at the old `Option<T>`-only contract.
  - The `UserDataField` enum variants are exhaustive and every
    existing consumer matches both variants.
  - The cycle guard uses monomorphized-type keys, not `DefId`-only
    (counterexample: `Boxed<u32>` and `Boxed<NonZeroU32>` must be
    verified independently even when they share a `DefId`).
  - All negative test cases compile and fail at construction time
    with a clear error message (not a rustc type-check error or
    a panic).
  - The primitive whitelist exactly matches the Stage-1 scope:
    `u8, u16, u32, u64, bool` only. `i8`/`i16`/`i32`/`i64`,
    `usize`/`isize`, `f32`/`f64`, `char`, `u128`, `i128` are
    REJECTED at construction time. Every rejected primitive has
    a negative test.
  - The new `aggregate_to_c_type` function accepts only plain
    struct types (not enum, not union); the test suite exercises
    a two-level nested struct case.
  - PL011's `imsc_shadow: u32` is accepted at construction, flows
    through the model as `UserDataField::ZeroInit { inner_type:
    Scalar(U32) }`, is carried through both lowering pipelines,
    and is emitted in the Linux driver struct as `u32
    imsc_shadow;` (no `Option`, no wrapper).
  - All regenerated goldens commit cleanly and every downstream
    test depending on
    `tests/fixtures/pl011-full-spec/expected/raw-frontend.json`
    passes unchanged (the test suite in `regseq-backends`,
    `regseq-analysis`, `regseq-model`, `regseq-driver` that reads
    this fixture must remain green).

- Include a **Non-Goals** section with:
  - No Tier-2 author opt-in mechanism (no `ZeroInit` trait, no
    `#[rseq::zero_init]` attribute). This is explicitly deferred.
  - No widening of the Rule-1 whitelist beyond Stage-1 scope. Do
    NOT add `i32`, `usize`, `f32`, `char`, etc.
  - No Rule-2 niche-init hardening (the `Option<bool>` latent
    soundness bug). Documented in design doc; deferred.
  - No changes to the shadow-register feature. That is a separate
    deferred workstream at
    `docs/discussions/models/regseq-model/shadow-registers.md`.
  - No changes to Linux emitter's struct-type panic at
    `emit_semantic.rs:216`. The `aggregate_to_c_type` addition
    prepares the ground but is not consumed by the emitter in
    this task. Consumption is a future task when a fixture
    actually uses a user-defined struct as user-data.
  - **No changes to `crates/regseq-backends/src/linux/emit_module.rs:703`**
    (the `devm_kzalloc` call). `devm_kzalloc` already zero-
    initializes the whole struct, which is sufficient for every
    Stage-1 Rule-1 user-data field per the design doc at
    `user-data-initialization.md:307-312`. **No post-allocation
    initialization stores are emitted for user-data fields**;
    authoring any such store is an out-of-scope change. If a
    future task for Rule-2 niche-init hardening lands (see
    design doc deferred-work section), that task will add
    post-allocation `None`-init stores; this task does not.
  - No changes to `Mmio::modify`'s `todo!()` gap at
    `regseq/src/mmio.rs:72`. Orthogonal to user-data work.

- Include a **Quality Gate** section running:
  ```
  cargo test -p regseq-driver
  cargo test -p regseq-model
  cargo test -p regseq-backends
  cargo test --workspace
  cargo fmt --all -- --check
  cargo clippy --workspace -- -D warnings
  ```

- Include an **Acceptance Criteria** section with each
  acceptance bullet anchored to an artifact (file, test name, or
  diff property) that can be mechanically verified.

### Cross-crate scope override

The task spec must explicitly note that this task spans four
crates as an intentional override of `task-spec-workflow.md:120-121`.
The authority for the override is
`docs/discussions/models/regseq-model/user-data-initialization.md`;
the design lists 12 touch-points that cannot split cleanly without
leaving the workspace uncompilable mid-sequence.

### Task ID and group

- Task ID: `regseq-model-64`.
- Group: `regseq-model` (primary crate for the core model shape
  change; cross-crate scope documented in the task spec).

### Dependency graph

- The new task `regseq-model-64` depends on no other pending tasks.
  It is ready to start immediately.
- `regseq-backends-02` (at
  `.harness/tasks/regseq-backends/02-emission-contract-and-exemplars.md`)
  gains a new `depends_on` edge on `regseq-model-64`. This
  sequences emission-contract work to start only after the
  fixture is compilable through the compiler.

### Codex review gates (during this batch's execution)

Two Codex review rounds required, both scoped to IMPLEMENTATION
GUIDANCE ONLY (no re-debate of strategy or design):

1. After the brief is drafted: Codex reviews the brief for
   completeness (all needed input files listed, output scope
   clear, update instructions correct).
2. After the task spec is drafted: Codex reviews the task spec
   for implementation-guidance completeness (all 12 touch-points
   listed, test plan exhaustive, verification section sufficient).

Codex prompts must explicitly say: "Scope the review to the
implementation guidance. Do not re-debate design choices captured
in `user-data-initialization.md`."

## Non-Goals

- This batch does not implement Path A. It only authors the task
  spec. The implementation happens when a worker agent picks up
  `regseq-model-64` via the normal task execution flow.
- No changes to Phase-6 or Phase-7 tracker items. This is a new
  tracker entry, not a revision of existing phases.
- No shadow-register feature work (separate deferred workstream).
- No widening of Rule-1 whitelist (deferred to later tasks when
  backend mapping is extended).
- No revisiting of the design. The three-round Codex debate is
  closed; its decisions are binding.

## Acceptance Criteria

- `.harness/tasks/regseq-model/64-user-data-init-path-a.md`
  created with all required sections and the thirteen-entry TODO
  checklist embedded in Implementation Notes.
- `.harness/tasks.json` updated: new task entry for
  `regseq-model-64` under the `regseq-model` group with
  `depends_on: []`. `regseq-backends-02` entry gains
  `regseq-model-64` in its `depends_on` list.
- `.harness/task-spec-status.yaml` updated: new tracker item
  `user-data-init-path-a` with status `tasked` and
  `generated_task_ids: [regseq-model-64]`.
- Both Codex review rounds (brief + task spec) reached a
  final-round verdict with the first line exactly `VERDICT: APPROVED`.
  Intermediate rounds with `REQUIRES FIXES` verdicts had the
  fixes folded before the final round. The final-round
  transcripts are the ones preserved per the Output Specification.
  This is a single mechanically verifiable verdict contract:
  open the preserved transcript files and check the first line.
- The final-round transcripts of both Codex review rounds are
  preserved in-repo at:
  - `docs/discussions/models/regseq-model/user-data-initialization-codex-review/round-4-brief-review.md`
    (brief-review final transcript; must exist and carry an
    `APPROVED` verdict)
  - `docs/discussions/models/regseq-model/user-data-initialization-codex-review/round-5-task-spec-review.md`
    (task-spec-review final transcript; must exist and carry an
    `APPROVED` verdict)
  These files make the review-gate completion mechanically
  verifiable by checking for the files and reading their first
  lines for the verdict. Intermediate rounds may be additionally
  preserved as `round-N-*.md` files if helpful for audit history.

## Update Instructions

After the task spec is drafted and committed:

1. In `.harness/task-spec-status.yaml`:
   - Add a new tracker item:
     ```yaml
     - id: user-data-init-path-a
       phase: "post-7"
       title: User-data initialization Path A (zero-init whitelist + Option tier)
       status: tasked
       depends_on: []
       target_harness_groups:
         - regseq-model
       source_refs:
         - docs/discussions/models/regseq-model/user-data-initialization.md
       brief: .harness/spec-batches/user-data-init-path-a.md
       summary: >
         Implement Path A of the user-data initialization contract:
         extend construction to accept either Option<T: Copy> or a
         compiler-controlled zero-init whitelist covering primitive
         scalars (u8/u16/u32/u64/bool) plus arrays and Copy
         structs of these. Unblocks PL011 Stage-1 fixture
         (imsc_shadow: u32) and all downstream artifact
         regeneration.
       spec_authoring_notes:
         - Single cross-crate task covering 12 touch-points per the
           design doc.
         - Cross-crate scope (4 crates) is an intentional override
           of task-spec-workflow.md:120-121; authority is the
           design doc.
       generated_task_ids:
         - regseq-model-64
     ```

2. In `.harness/tasks.json`:
   - Add the new task under the `regseq-model` group:
     ```json
     {
       "id": "regseq-model-64",
       "name": "User-data initialization Path A: zero-init whitelist + Option tier",
       "spec": ".harness/tasks/regseq-model/64-user-data-init-path-a.md",
       "status": "pending",
       "depends_on": [],
       "artifacts": [],
       "fail_reason": null
     }
     ```
   - Modify the existing `regseq-backends-02` entry to add
     `regseq-model-64` to its `depends_on` array (preserving
     existing entries).

3. Do not alter any other tracker items or tasks. Other phases
   are unaffected by this batch.
