# regseq-lowering-12: AMBA Mask Binding + Planner Override

**Depends on**: `regseq-lowering-10`, `regseq-model-60`.

## Description

Removes a PL011-tuned hardcode. `crates/regseq-backends/src/linux/planner.rs:139`
currently emits `amba_mask = amba_id.map(|_| 0x000f_ffff_u32)`
whenever the driver declares an AMBA bus -- the PrimeCell
peripheral-ID low-20-bit mask. This mask is correct for PL011 and
other PrimeCell-family peripherals but not universal across AMBA
devices. The binding schema at
`crates/regseq-model/src/binding/normalize.rs:308` accepts
`amba_id` and `revision` under the Amba bus identity but provides
no way for a platform author to specify a non-PrimeCell mask.

This task:

1. Adds `amba_mask: Option<u32>` to the Amba bus binding schema
   (Phase-1 binding normalization).
2. Plumbs `amba_mask` through
   `DriverLoweringPlan.resources.bus_identity` (both the legacy
   `build_driver_lowering_plan` path and the model-era
   `build_driver_lowering_plan_from_model` path landed by
   `regseq-lowering-10`).
3. Updates `linux/planner.rs:139` to consume the binding value
   when present; otherwise default to `0x000f_ffff_u32` and emit
   a structured `DriverLoweringDiagnostics` note recording the
   default was applied.

Source: `.harness/spec-batches/device-generality-cleanup.md`
Finding 1, Task C.

## Input Context

- `.harness/spec-batches/device-generality-cleanup.md` Finding 1
  and the Task C scope section.
- `.harness/tasks/regseq-lowering/10-new-lowering-entry-from-regseq-model.md`
  -- the model-era portable planner this task extends; defines
  `DriverLoweringDiagnostics` (landed by that task) which this
  task uses for the "default applied" signal.
- `crates/regseq-model/src/binding/normalize.rs` lines 300-330
  (bus-type key allowlist: currently `["amba_id", "revision"]`
  under `BusType::Amba`; this task extends that allowlist).
- `crates/regseq-model/src/binding/mod.rs` -- `BusBinding` and
  `BindingModel` public types; the binding value layer that
  transports the mask downstream.
- `crates/regseq-model/src/model/driver.rs` lines 122-133 --
  `PlatformBusBinding { bus_type: String, values:
  BTreeMap<String, PlatformValue> }`. This task does NOT add a
  typed field here; it extends the set of recognized keys in
  the `values` map to include `"amba_mask"`.
- `crates/regseq-model/src/join/sections.rs` lines 40-120 -- the
  join path where AMBA bus identity is lifted from the
  binding model into the joined artifact (relevant for the
  legacy lowering path).
- `crates/regseq-backends/src/lowering/plan.rs` lines 228-252 --
  `BusIdentity`: this task adds `amba_mask: Option<u32>`.
- `crates/regseq-backends/src/lowering/planner.rs` -- legacy
  `build_driver_lowering_plan`; populates `BusIdentity`
  from `BindingModel`. This task plumbs `amba_mask` through.
- `crates/regseq-backends/src/lowering/from_model.rs` -- model-era
  planner; populates `BusIdentity` from `RegseqModel`. This task
  plumbs `amba_mask` through.
- `crates/regseq-backends/src/lowering/diagnostics.rs` lines
  30-37 -- `DriverLoweringDiagnostics` struct landed by
  `regseq-lowering-10`. This task adds one boolean field.
- `crates/regseq-backends/src/linux/planner.rs` lines 134-150 --
  `build_registration` (where line 139 lives).
- `tests/fixtures/pl011-full-spec/platform.toml` -- PL011
  binding fixture; confirms today's path does not supply
  `amba_mask` and still produces `0x000f_ffff`.

## Implementation Notes

### Binding schema changes

In `crates/regseq-model/src/binding/normalize.rs`:

1. Extend the Amba key allowlist (currently
   `["amba_id", "revision"]` at line 308) to
   `["amba_id", "revision", "amba_mask"]`.

2. Parse `amba_mask` via the existing `extract_integer` helper
   at `binding/normalize.rs:183-192`, matching the pattern used
   for `amba_id` and `revision`. Today `extract_integer` returns
   `Option<i64>` -- non-integer or missing values return `None`,
   which silently falls through. This task preserves that
   behavior for consistency with the sibling keys: a missing or
   non-integer `amba_mask` yields `amba_mask: None` on the
   parsed binding, and the Linux planner then applies the
   PrimeCell default with the diagnostic described below. The
   binding layer does NOT fail on a malformed `amba_mask`
   spelling; platform-author errors on this key degrade to
   "default applied," not "build error." This is a deliberate
   choice that matches the existing treatment of `amba_id` +
   `revision`; if a stricter error surface is desired, it should
   be added uniformly across Amba keys in a separate task. Task C
   does NOT extend `BindingNormalizeError` with a new variant.

3. Store the parsed value in the existing
   `BusBinding::Amba`-side value bag (field-bag identical to how
   `amba_id` and `revision` are stored today); no new binding
   struct required.

### Model-era bus identity

Do NOT add a typed `amba_mask` field to `PlatformBusBinding`.
`PlatformBusBinding` at
`crates/regseq-model/src/model/driver.rs:122-133` carries a
`values: BTreeMap<String, PlatformValue>` bag keyed by
family-specific names (`amba_id`, `revision`). The existing
`from_model.rs` path reads `amba_id` and `revision` out of that
map (see `crates/regseq-backends/src/lowering/from_model.rs`
around lines 365-389). This task extends the set of recognized
keys in the map to include `amba_mask`; the `PlatformBusBinding`
struct definition does not change.

The Phase-1 `BusBinding` (the pre-model binding-model type,
populated by `binding::normalize`) DOES gain a typed
`amba_mask: Option<u32>` field alongside its existing
`amba_id: Option<u32>` and `revision: Option<u32>` fields.
That typed shape is already established in `BusBinding`; this
task adds one more entry to it.

### Join path

`crates/regseq-model/src/join/sections.rs` lifts AMBA values from
the `BindingModel` into the joined artifact. Extend the lift to
include `amba_mask`. Behavior-preserving: when the binding carries
no mask, `amba_mask: None` is lifted.

### Portable plan

In `crates/regseq-backends/src/lowering/plan.rs::BusIdentity`:
add

```rust
pub struct BusIdentity {
    pub bus_type: BusType,
    pub compatible: Vec<String>,
    pub amba_id: Option<u32>,
    pub amba_revision: Option<u32>,
    /// Mask value for the AMBA peripheral-ID match table entry.
    /// `None` when the binding did not supply one; the Linux
    /// planner then applies the PrimeCell default
    /// (`0x000f_ffff`).
    pub amba_mask: Option<u32>,
}
```

### Legacy portable planner

`crates/regseq-backends/src/lowering/planner.rs` -- wherever
`BusIdentity` is constructed from `BindingModel`, read the new
`amba_mask` field and thread it through.

### Model-era portable planner

`crates/regseq-backends/src/lowering/from_model.rs` -- wherever
`BusIdentity` is constructed from `RegseqModel`, extract
`amba_mask` from
`driver.platform_metadata.bus.values.get("amba_mask")`
(following the existing `amba_id` / `revision` extraction
pattern). The map is typed `BTreeMap<String, PlatformValue>`;
match on `PlatformValue::Integer` and narrow to `u32`, the same
way the existing code narrows `amba_id`. Missing key or non-
integer value yields `amba_mask: None`.

### Diagnostics

`DriverLoweringDiagnostics` is a field-bag `struct`, not an enum
(see `crates/regseq-backends/src/lowering/diagnostics.rs:30-37`).
Extend it with one boolean field:

```rust
#[derive(Debug, Clone, Default)]
pub struct DriverLoweringDiagnostics {
    pub linux_resource_sequencing: LinuxResourceSequencing,
    pub linux_deployment_metadata: LinuxDeploymentMetadata,
    /// True when `DriverLoweringPlan.resources.bus_identity.bus_type`
    /// is `BusType::Amba` and `amba_id.is_some()` but the binding
    /// did not supply `amba_mask`. Signals that the Linux planner
    /// will apply the PrimeCell default (`0x000f_ffff`) downstream.
    /// Reporting-only; not consumed by any emitter.
    pub amba_mask_default_applied: bool,
}
```

The field defaults to `false` via `#[derive(Default)]`, so existing
callers that construct a `DriverLoweringDiagnostics` without this
field continue to compile. Task C flips the field to `true` in
both the legacy and model-era portable planners when the
condition above holds.

### Linux planner

`crates/regseq-backends/src/linux/planner.rs:134-150`
(`build_registration`):

```rust
fn build_registration(plan: &DriverLoweringPlan) -> LinuxRegistrationShape {
    let snake_name = to_snake_case(&plan.identity.driver_name);
    let is_amba = plan.resources.bus_identity.bus_type == BusType::Amba;

    let amba_id = plan.resources.bus_identity.amba_id;
    let amba_mask = if is_amba {
        plan.resources.bus_identity.amba_mask
            .or(amba_id.map(|_| 0x000f_ffff_u32))
    } else {
        None
    };

    // (diagnostic recorded by the portable planner on the
    // DriverLoweringPlan.diagnostics field if the fallback
    // was triggered; see below)

    LinuxRegistrationShape { /* ... */ }
}
```

Note: the diagnostic is emitted by the **portable planner**
(not the Linux planner) because `DriverLoweringDiagnostics` is
owned by the portable plan. Concretely: in both
`lowering/planner.rs` (legacy) and `lowering/from_model.rs`
(model-era), after `BusIdentity` is populated, if
`bus_type == Amba && amba_mask.is_none() && amba_id.is_some()`,
set `plan.diagnostics.amba_mask_default_applied = true`. The
Linux planner consumes
`amba_mask.unwrap_or(0x000f_ffff_u32)` downstream.

### Grep gates

```
rg -n '0x000f_ffff' crates/regseq-backends/src/
# must return at most one match: the fallback literal in
# lowering/planner.rs OR lowering/from_model.rs (one of the two)
# or in linux/planner.rs; acceptable. Any OTHER hit is suspect.

rg -n 'amba_mask' crates/regseq-model/src/binding/normalize.rs
# must return at least one match (new allowlist entry).
```

### What this task does NOT do

- Does not change PCI, platform, or non-AMBA match-table
  emission.
- Does not expose the mask through the authoring surface (no
  `#[rseq::...]` attribute change). Authors who need a
  non-default mask supply it in `platform.toml` under the
  Amba bus section.
- Does not change of-match-table compatible-string policy.
- Does not touch `regseq-macros` or any authoring crate.

## Non-Goals

- No Zephyr backend work.
- No redesign of `DriverLoweringDiagnostics`.
- No fixture changes to PL011 beyond asserting the existing
  behavior (no `amba_mask` in `platform.toml` -> default still
  `0x000f_ffff` + diagnostic emitted).
- No new compatible-string or match-table fields beyond
  `amba_mask`.

## Test Plan

### Unit tests

1. `binding/normalize.rs`: parsing
   `[bus.amba] amba_id = 0x00041011, amba_mask = 0x00ff_ffff`
   produces a binding carrying `amba_mask = Some(0x00ff_ffff)`.
   Parsing the PL011 fixture (no `amba_mask` key) produces
   `amba_mask = None`.

2. `amba_mask` present but not an integer (e.g., a string) yields
   `amba_mask: None` on the parsed binding, matching the
   silent-fallthrough behavior of `extract_integer`. Downstream,
   the Linux planner applies the PrimeCell default and the
   `amba_mask_default_applied` diagnostic is set. This test
   documents the intentional consistency with the existing
   `amba_id` / `revision` parse behavior; Task C does not add
   strict validation at the binding layer.

3. `lowering/planner.rs` legacy path: a `BindingModel` with
   `amba_mask = Some(0x00ff_ffff)` produces a
   `DriverLoweringPlan.resources.bus_identity.amba_mask ==
   Some(0x00ff_ffff)`. Without the key, `None`. With the key
   missing and `amba_id` present, the plan's
   `diagnostics.amba_mask_default_applied` equals `true`.

4. `lowering/from_model.rs` model-era path: same three cases.

### Integration tests

5. Linux emission with a binding that supplies
   `amba_mask = 0x00ff_ffff`: the generated
   match table contains
   `{ .id = 0x00041011, .mask = 0x00ff_ffff }`.

6. PL011 emission (no binding override): byte-identical to
   baseline. `plan.diagnostics.amba_mask_default_applied ==
   true` on the portable plan but does not change emitted C.

### Clippy / format / regression

7. `cargo test --workspace` green.
8. `cargo fmt --all -- --check` clean.
9. `cargo clippy -p regseq-backends -p regseq-model -- -D warnings` clean.

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540 --message-only
<task>
Review regseq-lowering-12 against
.harness/tasks/regseq-lowering/12-amba-mask-binding-override.md.

Files to audit:
- crates/regseq-model/src/binding/normalize.rs (allowlist +
  parse)
- crates/regseq-model/src/join/sections.rs (join lift)
- crates/regseq-model/src/model/driver.rs (no struct change;
  `values` map gains recognition for `"amba_mask"`)
- crates/regseq-backends/src/lowering/plan.rs (BusIdentity
  field)
- crates/regseq-backends/src/lowering/planner.rs (legacy
  population)
- crates/regseq-backends/src/lowering/from_model.rs (model-era
  population)
- crates/regseq-backends/src/lowering/diagnostics.rs
  (amba_mask_default_applied field on
  DriverLoweringDiagnostics)
- crates/regseq-backends/src/linux/planner.rs:134-150
  (consumption + default)

Focus on:
- amba_mask key added to the Amba bus binding schema.
- BusIdentity.amba_mask: Option<u32> threaded through both
  portable-plan paths.
- plan.diagnostics.amba_mask_default_applied set to true by
  the portable planner (not the Linux planner) when an AMBA
  binding lacks the key.
- Linux planner falls back to 0x000f_ffff only when
  bus_identity.amba_mask is None.
- PL011 match-table value unchanged (byte parity).
- Binding with amba_mask = 0x00ff_ffff produces that value in
  the emitted match table.

Do NOT flag non-AMBA bus work or authoring-surface changes --
out of scope.
</task>

<grounding_rules>
Cite file:line for every claim. Mark hypotheses as hypotheses.
</grounding_rules>

<structured_output_contract>
First line: VERDICT: APPROVED | REQUIRES FIXES
If REQUIRES FIXES: numbered list, each entry exactly
  <file>:<line-range> -- <what is wrong> -- <what the spec requires>
If APPROVED: one paragraph (<= 120 words).
Total response <= 400 words.
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Form a verdict, stop.
</default_follow_through_policy>
EOF
```

Iterate until APPROVED.

## Quality Gate

```
cargo test -p regseq-model
cargo test -p regseq-backends
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
```

## Acceptance Criteria

- Amba bus binding schema accepts `amba_mask` as a `u32` key.
- `BusBinding` (Phase-1 binding-model type) carries
  `amba_mask: Option<u32>` populated by `binding::normalize`.
  `PlatformBusBinding.values` (model-era) carries the
  `"amba_mask"` key when supplied. The model-era planner reads
  the bag entry; the legacy planner reads the typed field.
  Neither layer adds a typed field to `PlatformBusBinding`.
- `BusIdentity.amba_mask: Option<u32>` present on the portable
  plan and populated by both legacy and model-era planners.
- `DriverLoweringDiagnostics` gains the
  `amba_mask_default_applied: bool` field; set to `true`
  exactly when an AMBA driver's binding lacks `amba_mask` and
  `amba_id` is present, else `false` (default).
- `linux/planner.rs` consumes `amba_mask` with PrimeCell
  fallback; literal `0x000f_ffff` appears in at most one
  production code location.
- PL011 emission byte-identical to baseline; PL011
  `DriverLoweringPlan.diagnostics.amba_mask_default_applied ==
  true`.
- Codex review APPROVED.
