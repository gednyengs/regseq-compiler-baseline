# Spec Batch: device-generality-cleanup -- Post-Phase-7 Device-Class Generality Cleanup

## Description

Post-`regseq-linux-backend-03` audit uncovered widespread PL011-shape
leakage in "generic" Linux backend and portable-lowering code. This
brief catalogs the findings, maps them to existing queued tasks
where they can be absorbed, and defines new executable harness
tasks for the findings that no queued task will fix.

This batch is **task-spec authoring work** that produces a mix of:

- **amendments** to already-authored specs `regseq-linux-backend-04`
  and `regseq-linux-backend-05` (fold new Implementation Notes into
  their existing scope so PL011 shape is not re-introduced during
  the CFG emitter rewrites)
- **new executable task specs** for generality gaps that no queued
  task touches (per-resource probe schema, property-realization
  honoring + `linux_read_api` latent-bug fix, AMBA-mask binding
  override, portable-lowering fallback-path test guard)

The concrete outcome is a set of task specs that, once executed,
make the Linux backend treat device shape (number of MMIO regions,
interrupts, clocks; custom field names; property types beyond
u32/u16/u8/bool) correctly rather than assuming PL011's specific
shape.

The batch is bounded: it does not redesign the portable plan, does
not touch the regseq-macros authoring surface, does not introduce
new DSL primitives, and does not expand the PL011 reference
fixture.

## Input Context

Read before starting:

- `.harness/config.md`
- `.harness/SESSION.md`
- `.harness/task-spec-workflow.md`
- `.harness/task-spec-status.yaml`
- `.harness/spec-batches/BRIEF_TEMPLATE.md`
- `.harness/tasks/regseq-linux-backend/03-linux-plan-self-sufficiency.md`
  (the boundary-cleanup task whose completion surfaced the
  findings; acceptance criteria constrain PL011 byte parity in
  isolation, which is why new tasks are needed)
- `.harness/tasks/regseq-linux-backend/04-emit-semantic-cfg-rewrite.md`
  (queued; absorbs findings 8, 9 below with amendments)
- `.harness/tasks/regseq-linux-backend/05-emit-module-cfg-rewrite-and-emit-behavior-delete.md`
  (queued; absorbs findings 2-7, 13 below with amendments; current
  spec does not list the PL011-shape invariants that must be
  preserved)
- `.harness/tasks/regseq-lowering/10-new-lowering-entry-from-regseq-model.md`
  (the model-era portable planner whose backend-neutrality is
  challenged by findings 11-12)
- `.harness/tasks/regseq-lowering/11-delete-dead-lowering-submodules.md`
  (queued; naturally removes findings 11-12 by deleting the
  legacy lowering path, but the new path must not regress)
- `crates/regseq-backends/src/linux/emit_module.rs` (lines 258-274,
  295-300, 622-627, 657-758, 700-732)
- `crates/regseq-backends/src/linux/emit_regs.rs` (lines 51-57)
- `crates/regseq-backends/src/linux/emit_semantic.rs` (lines 499-506)
- `crates/regseq-backends/src/linux/plan.rs` (fields `has_optional_clock`,
  `shared_struct_fields`, `property_realization` / `LinuxPropertyRealization`)
- `crates/regseq-backends/src/linux/planner.rs` (lines 109-116,
  134-149, 171-204, 434-454)
- `crates/regseq-backends/src/lowering/mir_interpreter/place_resolver.rs`
  (lines 219-277)
- `crates/regseq-backends/src/lowering/semantic_lowering/semantic_walker.rs`
  (lines 205-260)
- `crates/regseq-backends/src/lowering/semantic_lowering/ref_resolver.rs`
  (lines 423-441)
- `crates/regseq-model/src/binding/normalize.rs` (line 308 -- Amba
  bus key allowlist; informs task C below)
- `docs/discussions/models/regseq-model/migration.md` Q7 (the
  layered plan architecture the findings challenge)
- `docs/discussions/linux-codegen/lowering-plan-interface-boundary.md`
  (the backend-boundary design note; superseded by migration.md
  where they conflict, but still the written design for
  LinuxBackendPlan's role)

## Output Specification

Create:

- `.harness/tasks/regseq-linux-backend/06-per-resource-probe-schema.md`
  (Task A below: per-resource Linux probe/emission schema --
  multi-MMIO/IRQ/clock; removes `has_optional_clock` bool; probe
  steps carry field_name)
- `.harness/tasks/regseq-linux-backend/07-property-realization-honoring.md`
  (Task B below: consume `LinuxPropertyRealization.field_name` and
  `linux_read_api`; replace `field_type` allowlist with exhaustive
  typed enum; fixes latent bool-property correctness bug)
- `.harness/tasks/regseq-lowering/12-amba-mask-binding-override.md`
  (Task C below: add `amba_mask` to binding schema; plumb through
  portable plan; default to PrimeCell mask only when binding does
  not supply one)
- `.harness/tasks/track-a-validation/14-multi-resource-fixture-gate.md`
  (Task D below: adds a two-clock-two-MMIO test fixture that
  exercises tasks A/B/C; anchors the "no first-resource fallback"
  invariant as an executable test; runs after
  `regseq-lowering-11` deletes the legacy fallback paths)

Modify:

- `.harness/tasks/regseq-linux-backend/04-emit-semantic-cfg-rewrite.md`
  (add Implementation Note: "`is_option_pointer_context` must not
  string-match field names; the new CFG emitter consumes typed
  resource-handle metadata -- see Finding 9 in
  `device-generality-cleanup.md`")
- `.harness/tasks/regseq-linux-backend/05-emit-module-cfg-rewrite-and-emit-behavior-delete.md`
  (add Implementation Notes enumerating **Findings 2, 3, 4, 5, 6,
  7, 8, and 13** in `device-generality-cleanup.md`:
  register-accessor helpers must read the MMIO field name from
  the plan (Findings 2, 3); clock/IRQ probe steps must render
  authored field names rather than hardcoded `d->clock` /
  `d->irq` (Findings 4, 5); per-clock optionality comes from each
  `GetClock` step, not a plan-wide bool (Finding 6);
  `uart_driver.nr` must come from plan (Finding 7);
  register-prefix selection must be per-descriptor, not
  `.first()` (Finding 8); `ReadProperty` probe emission must
  consume `LinuxPropertyRealization.field_name` +
  `linux_read_api` (Finding 13, overlap with Task B -- whichever
  lands first defines the interface, task 05 preserves it).
  Add `regseq-linux-backend-06` (Task A) and
  `regseq-linux-backend-07` (Task B) as additional `depends_on`
  for task 05 in `.harness/tasks.json`.)

Update when done:

- `.harness/tasks.json` (register the four new task IDs)
- `.harness/task-spec-status.yaml`: add a new tracker item
  `device-generality-cleanup` with status `tasked`, depending on
  `regseqmodel-phase-7`, listing the four generated task IDs

Output type: **executable harness tasks**.

## Implementation Notes

### Background: the findings catalog

Each finding below is a citation with file:line, a one-line defect
description, and a mechanism explaining why non-PL011 input fails.
Severity per the agent taxonomy (HARD = literal PL011 value; SHAPE =
cardinality/shape assumption; LATENT = today-broken for PL011-like
edge cases).

**HARD**

1. `crates/regseq-backends/src/linux/planner.rs:139` -- AMBA mask
   hardcoded to `0x000f_ffff_u32`. The binding schema at
   `crates/regseq-model/src/binding/normalize.rs:308` accepts
   `amba_id` and `revision` under the Amba bus but no
   `amba_mask` key. Any AMBA device needing a different mask silently
   emits a wrong match table. **Addressed by: Task C.**

**SHAPE**

2. `crates/regseq-backends/src/linux/emit_module.rs:258-274` --
   `{snake}_reg_read/write/modify` accessors hardcode `d->regs` in
   their bodies. Not read from plan. Non-`regs` MMIO field names
   break. Multi-MMIO silently collapses. **Addressed by: Task A
   (plan carries MMIO field names) + Task 05 amendment.**

3. `crates/regseq-backends/src/linux/emit_module.rs:686-697`
   (`LinuxProbeStep::MapMmio`) -- `d->regs = devm_ioremap_resource(...)`
   on both AMBA and platform branches; platform branch hardcodes
   `IORESOURCE_MEM, 0`. Multi-MMIO drivers acquire only the first
   region. **Addressed by: Task A + Task 05 amendment.**

4. `crates/regseq-backends/src/linux/emit_module.rs:657-672`
   (`LinuxProbeStep::GetClock`) -- hardcodes `d->clock` on both
   `devm_clk_get` and `devm_clk_get_optional` paths; `clock_id` is
   passed but not rendered into C; `NULL` hint means the Linux
   driver cannot distinguish clocks by name. Multi-clock drivers
   lose all but one clock. **Addressed by: Task A + Task 05
   amendment.** Also subsumes removal of the `has_optional_clock:
   bool` field on `LinuxBackendPlan`: per-clock optionality encodes
   on each `LinuxProbeStep::GetClock` variant.

5. `crates/regseq-backends/src/linux/emit_module.rs:719-732`
   (`LinuxProbeStep::RequestIrq`) -- `d->irq = adev->irq[0]` (AMBA) /
   `platform_get_irq(pdev, 0)` (platform). Multi-interrupt drivers
   collapse to one slot. **Addressed by: Task A + Task 05 amendment.**

6. `crates/regseq-backends/src/linux/emit_module.rs:745-758` +
   `crates/regseq-backends/src/linux/plan.rs` (`has_optional_clock`
   field added by task 03) -- one-bit encoding of a fact that
   needs per-clock resolution. A driver with a required bus_clock
   and an optional baud_clock cannot be expressed. **Addressed by:
   Task A (delete field; move optionality onto each clock step).**

7. `crates/regseq-backends/src/linux/emit_module.rs:622-627` --
   `.nr = 1` hardcoded in `uart_driver` initializer. Multi-port
   UART specs register only one port. **Addressed by: Task 05
   amendment (plan carries port_count; defaults to 1).**

8. `crates/regseq-backends/src/linux/emit_module.rs:295-300` +
   `crates/regseq-backends/src/linux/emit_regs.rs:51-57` --
   `reg_prefix = plan.register_layout.blocks.first()...unwrap_or("REG")`.
   Multi-block descriptors emit bitfield defines under the first
   block's prefix. **Addressed by: Task 05 amendment (new CFG
   emitter must derive the descriptor's block prefix
   per-descriptor, not via `.first()`).**

9. `crates/regseq-backends/src/linux/emit_semantic.rs:499-506` --
   `is_option_pointer_context` matches `field.name == "clock" ||
   "reset" || "pinctrl" || "regs"`. A driver using `bus_clk` or
   `memory_region` gets integer-zero sentinel instead of NULL for
   `Option<pointer>`. **Addressed by: Task 04 amendment (new CFG
   emitter reads resource-handle type from model, not field-name
   string).**

10. Two distinct allowlists in `crates/regseq-backends/src/linux/planner.rs`:
    - Lines 109-116 (shared-struct C type mapping for
      `LinuxSharedStructField.c_type`): `"u32"/"u16"/"u8"/"bool"`
      only; **`u64` is not in this allowlist** -- an authored `u64`
      property gets a `u32` field in the generated driver-state
      struct (hard mis-type, not just mis-read).
    - Lines 439-445 (`linux_read_api` selection): handles `u64`
      and `bool` correctly; unknown scalar spellings still fall
      through to `device_property_read_u32`.
    **Addressed by: Task B (single exhaustive `LinuxPropertyScalar`
    enum feeds both call sites; unknown scalars fail the plan
    build).**

11. `crates/regseq-backends/src/lowering/mir_interpreter/place_resolver.rs:219-277`
    -- `identify_field_from_type` returns `resource_fields.first()`
    on any resource-prefix match; `find_mmio_field_name` returns
    first MMIO field, fallback to literal `"regs"`. Portable-plan
    contamination: runs before the Linux planner ever sees the
    data. **Addressed by: `regseq-lowering-11` (deletes the legacy
    MIR interpreter path) + Task D guard-test (asserts no
    first-resource fallback in the surviving
    `build_driver_lowering_plan_from_model` path).**

12. `crates/regseq-backends/src/lowering/semantic_lowering/semantic_walker.rs:205-260`
    + `ref_resolver.rs:423-441` -- `unique_resource_field_name(kind)`
    returns `None` when 0 or 2+ resources of that kind exist. Its
    doc admits: "useful for current-slice lowering bridges that can
    be made structural only when the ownership is unambiguous."
    Multi-clock drivers silently lose the clock->rate mapping during
    semantic lowering. Portable-plan contamination. **Addressed by:
    `regseq-lowering-11` + Task D guard-test.**

**LATENT CORRECTNESS BUG (not just generality)**

13. `crates/regseq-backends/src/linux/emit_module.rs:700-718`
    (`LinuxProbeStep::ReadProperty`) -- ignores
    `LinuxPropertyRealization.field_name` and
    `LinuxPropertyRealization.linux_read_api`. Derives field name
    from `property_id.replace('-', "_")`. Hardcodes
    `device_property_read_u32`. Planner populates these at
    `planner.rs:436-453` but grep confirms three hits only (all
    planner-side; no emitter consumer). A bool property today emits
    a u32 read against the Linux firmware property API; a property
    with a custom field_name writes to a nonexistent struct field.
    **Addressed by: Task B (urgent; independent of all other
    tasks; ships first).**

**SOFT (convention, not a breaker)**

14. `crates/regseq-backends/src/linux/planner.rs:171-204` --
    `serial_dev_name_policy()` generates `"tty<initials>"`. Backend
    naming policy, deterministic, legible for any serial driver.
    **No action proposed.**

### Task A: Per-resource Linux probe/emission schema

`.harness/tasks/regseq-linux-backend/06-per-resource-probe-schema.md`

Depends on: `regseq-linux-backend-03`. Independent of Tasks B/C/D.

Scope:

1. Extend `LinuxProbeStep` variants to carry the authored
   `field_name`:
   - `GetClock { clock_id, field_name, optional, name_hint }` (the
     `name_hint` is the string passed to `devm_clk_get_*` for Linux
     consumer-name matching; `None` when the driver does not
     declare one)
   - `RequestIrq { interrupt_id, field_name, index }` (where
     `index` is the resource index, defaulting to 0 but explicit
     on the plan)
   - `MapMmio { mmio_id, field_name, index }` (same pattern)
2. Delete `LinuxBackendPlan.has_optional_clock`. Per-clock
   optionality lives on each `GetClock` step.
3. Update `build_linux_backend_plan` in
   `crates/regseq-backends/src/linux/planner.rs` to populate the
   new fields from the corresponding
   `DriverLoweringPlan.resources.*[i].field_name` values already on
   the portable plan.
4. **Populate `MmioObligation.register_block_type` in the
   model-era lowering entry.** Today
   `crates/regseq-backends/src/lowering/from_model.rs:201-208`
   hardcodes `register_block_type: None`, which makes the
   register-accessor helpers unable to resolve which `d->{mmio_field}`
   pointer to dereference in multi-MMIO drivers. Extend the
   model-era planner to emit the authored register-block type name
   (already carried on `regseq_model::model::MmioResource` via the
   driver-mmio declaration). When one MMIO owns zero or multiple
   register blocks with ambiguous ownership, `build_linux_backend_plan`
   must fail with a structured `LinuxPlanError` rather than silently
   defaulting to the first block.
5. Carry the MMIO field name into each `LinuxRegisterBlock` on
   `LinuxBackendPlan.register_layout.blocks` (new field
   `owning_mmio_field: String`). Update `emit_module.rs` probe
   emission (and the shared-header struct) to render
   `d->{field_name}` at each site. Update `emit_register_accessors`
   to render `d->{owning_mmio_field}` rather than the hardcoded
   `d->regs`, selecting the correct pointer per accessor call.
6. Preserve PL011 byte parity: PL011's single-clock, single-MMIO,
   single-IRQ case must still emit the same strings. Add a byte
   parity smoke test analogous to the one in
   `regseq-linux-backend-03`.

Out of scope: changing portable `DriverLoweringPlan` field
**types** (field_name already exists on every resource variant;
`register_block_type` exists as `Option<String>` and is the field
this task populates rather than adds); Zephyr backend; Linux
register-helper emission in `emit_regs.rs` beyond the block-prefix
fix already in scope of task 05's amendment.

Testing: add a two-MMIO, two-clock, two-IRQ hand-built
`DriverLoweringPlan` fixture in a new test file under
`crates/regseq-backends/tests/`. Assert that each resource's field
name appears in the emitted C and no `d->regs` / `d->clock` /
`d->irq` collisions occur. Keep the existing PL011 scaffold tests
green.

### Task B: Property realization consumption + linux_read_api

`.harness/tasks/regseq-linux-backend/07-property-realization-honoring.md`

Depends on: `regseq-linux-backend-03`. Independent. **Ships first
(latent correctness bug, short task).**

Scope:

1. `emit_module.rs:700-718` must read
   `LinuxPropertyRealization.field_name` (not derive it from
   `property_id`) and dispatch the read API via
   `LinuxPropertyRealization.linux_read_api`.
2. Replace `planner.rs`'s `"u32"/"u16"/"u8"/"bool"` string-match
   allowlist with an exhaustive typed enum
   `LinuxPropertyScalar { U8, U16, U32, U64, Bool }`. Unknown
   authored scalars fail `build_linux_backend_plan` with a
   structured `LinuxPlanError`, not a silent fallback.
3. `planner.rs:109-116` (same allowlist, different call site --
   the shared-header struct field type mapping) uses the same
   typed enum.

Out of scope: non-scalar properties (arrays, strings, enums) --
those are a future authoring-surface extension; failing hard on
them here is the correct move.

Testing: add a bool-property fixture; assert the emitted probe
body contains `device_property_read_bool` and writes to the
authored field name. Add a u64-property fixture; same. Add a
rejected-scalar fixture; assert `LinuxPlanError` with the scalar
spelling in the message. Keep PL011 scaffold tests green (PL011
uses u32 `"fifo-size"` + u32 `"current-speed"`; typed-enum
migration is behavior-preserving for them).

### Task C: AMBA mask binding + planner override

`.harness/tasks/regseq-lowering/12-amba-mask-binding-override.md`

Depends on: `regseq-lowering-10` (model-era portable planner, the
entry point this task extends) and the last binding-schema task
landed by Phase 6 (identify via `tasks.json` at authoring time --
the brief names the phase because the exact task ID of the
binding-schema terminal task is phase-6 authoring-era state; the
spec authoring worker MUST resolve it to a concrete executable
task ID before writing the spec). Independent of Tasks A/B/D
otherwise.

Scope:

1. Add `amba_mask: Option<u32>` to the Amba bus-binding schema.
   Extend the key allowlist at
   `crates/regseq-model/src/binding/normalize.rs:308`.
2. Plumb `amba_mask` through
   `DriverLoweringPlan.resources.bus_identity.amba_mask: Option<u32>`.
3. `planner.rs:139`: if the binding supplies `amba_mask`, use it;
   otherwise default to `0x000f_ffff_u32` and emit a structured
   diagnostic noting the default (via
   `DriverLoweringDiagnostics`, landed by `regseq-lowering-10`).

Out of scope: non-AMBA bus match-table customization; PCI match
tables; of-match-table compatible-string policies.

Testing: PL011 binding without `amba_mask` still produces
`0x000f_ffff`. A synthesized binding with `amba_mask = 0x00ff_ffff`
produces that value in the generated match table. A binding with
`amba_mask` present but `amba_id` absent is a binding error
(matching the existing enum-schema invariant).

### Task D: Multi-resource fixture gate

`.harness/tasks/track-a-validation/14-multi-resource-fixture-gate.md`

Depends on: Tasks A, B, C, **and** `regseq-lowering-11` (legacy
lowering deletion).

Scope: a new test fixture (code + binding) exercising a hypothetical
"generic-uart" device with two clocks, two MMIO regions, two
interrupts, and a bool property. Drives the full pipeline
(`normalize -> populate_bindings ->
build_driver_lowering_plan_from_model -> build_linux_backend_plan
-> emit_linux_artifacts`). Asserts:

1. Every resource's authored field name appears in the generated
   driver-state struct (`{snake}_data`).
2. Each `devm_clk_get` / `devm_ioremap_resource` /
   `platform_get_irq` call references the authored field name.
3. No `d->regs`, `d->clock`, `d->irq` string appears unless the
   author named a field exactly that.
4. No production code reads `.first()` on
   `resource_fields`, `mmio_fields`, or
   `register_layout.blocks` (enforced via a targeted ripgrep
   assertion in a build-script-style test, analogous to the
   existing grep gates in spec 03).

Out of scope: host-build proof of the generic fixture (the
assertion is structural, not kernel-compilation); Zephyr.

Testing: the fixture itself is the test.

### Sequencing

1. Task B (property realization) -- ships first: smallest, no
   dependencies, fixes a latent correctness bug.
2. Task C (AMBA mask) -- independent, small, may ship in parallel
   with A/B.
3. Task A (per-resource schema) -- largest; co-design with Tasks
   04/05 since they rewrite the same files.
4. Task D (guard test) -- after A/B/C land and
   `regseq-lowering-11` deletes the legacy fallback paths.

### Amendments to queued tasks

`regseq-linux-backend-04` and `regseq-linux-backend-05` spec files
gain one Implementation Note section each that references this
brief by path and enumerates the findings they must not
re-introduce. Do NOT re-scope their existing work; the amendments
are constraints on the CFG-emitter rewrite, not new features.

## Non-Goals

- No redesign of the two-layer plan split (`DriverLoweringPlan` vs
  `LinuxBackendPlan`). Migration.md Q7 settled that; the findings
  do not challenge the split, they challenge what each layer
  encodes.
- No changes to the authoring surface: no new `#[rseq::*]`
  attributes, no new abstractions in `regseq-macros` or `regseq`.
- No expansion of the PL011 fixture. PL011 remains the single
  lead fixture; the new two-MMIO-two-clock fixture in Task D is
  synthetic and lives under `crates/regseq-backends/tests/` (not
  under `tests/fixtures/`).
- No Zephyr backend work.
- No correctness or generality work on non-serial device classes
  (serial-UART is the only realized device class). Tasks written
  here must not assume non-serial device classes exist; they must
  not preclude them either.
- No performance or optimization work.
- No task that directly modifies code. All four new task IDs +
  the two queued-spec amendments are spec-authoring output; code
  execution happens later via the normal `task.py start <id>`
  loop.

## Acceptance Criteria

This batch is complete when:

1. Four new task spec files exist at the paths listed under Output
   Specification, each following the standard task-spec structure
   (Description / Input Context / Implementation Notes / Non-Goals
   / Test Plan / Review / Quality Gate / Acceptance Criteria).
2. The two amendments to `regseq-linux-backend-04` and
   `regseq-linux-backend-05` are applied as additional
   Implementation Notes referencing this brief by path
   (`.harness/spec-batches/device-generality-cleanup.md`) and
   the specific finding numbers.
3. `.harness/tasks.json` lists the four new task IDs with their
   correct `depends_on` relationships, expressed as concrete
   **executable task IDs** (never tracker phase IDs). Specifically:
   - `regseq-linux-backend-06` (Task A): depends on
     `regseq-linux-backend-03`, `regseq-lowering-10`.
   - `regseq-linux-backend-07` (Task B): depends on
     `regseq-linux-backend-03`.
   - `regseq-lowering-12` (Task C): depends on
     `regseq-lowering-10` and the terminal Phase-6 binding-schema
     task (resolved to a concrete task ID at spec-authoring
     time by reading `.harness/tasks.json`).
   - `track-a-validation-14` (Task D): depends on
     `regseq-linux-backend-06`, `regseq-linux-backend-07`,
     `regseq-lowering-12`, and `regseq-lowering-11`.
4. `.harness/task-spec-status.yaml` contains a new item
   `device-generality-cleanup` with status `tasked`, depending on
   `regseqmodel-phase-7`, with `generated_task_ids` listing the
   four new task IDs.
5. Each new task spec's Test Plan includes construct-level tests
   that would fail against the current `main` branch if the
   finding were reintroduced (i.e. the test prevents regression,
   not just asserts positive output).
6. This brief has been Codex-reviewed (per the project review
   gate) with verdict APPROVED before any task-spec files are
   authored.

## Update Instructions

After authoring the four new task spec files and applying the two
amendments:

1. Add a new tracker item `device-generality-cleanup` to
   `.harness/task-spec-status.yaml` with:
   - `phase: "post-7"`
   - `title: Device-class generality cleanup (post-phase-7 residual)`
   - `status: tasked`
   - `depends_on: [regseqmodel-phase-7]`
   - `target_harness_groups: [regseq-linux-backend, regseq-lowering,
     track-a-validation]`
   - `source_refs: this brief path`
   - `brief: .harness/spec-batches/device-generality-cleanup.md`
   - `generated_task_ids`: the four new IDs
2. Do not move any other tracker item; `regseqmodel-phase-8` stays
   `ready_to_elaborate` and `regseqmodel-phase-9` stays
   `not_ready` -- Phase 8/9 are orthogonal to this batch.
3. Modify `tasks.json` entries for existing queued tasks to add
   cross-task dependencies:
   - `regseq-linux-backend-05.depends_on` gains
     `regseq-linux-backend-06` (Task A) and
     `regseq-linux-backend-07` (Task B), so task 05 does not run
     before the per-resource schema and the property-realization
     fix land.
   - No other queued task's `depends_on` is modified.
