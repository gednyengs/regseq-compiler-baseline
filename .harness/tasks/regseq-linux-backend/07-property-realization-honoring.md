# regseq-linux-backend-07: Property Realization Consumption + `linux_read_api` Honoring

**Depends on**: `regseq-linux-backend-03`.

## Description

Fixes a latent correctness bug in probe emission and closes a
silent mis-typing gap in the Linux planner's property handling.

Today, `crates/regseq-backends/src/linux/emit_module.rs:700-718`
(`LinuxProbeStep::ReadProperty` emission) **ignores** both
`LinuxPropertyRealization.field_name` and
`LinuxPropertyRealization.linux_read_api` -- two fields the
planner already populates. Instead the emitter:

- derives the destination struct field from
  `property_id.replace('-', "_")`
- hardcodes `device_property_read_u32` regardless of the
  property's actual type

Today the first defect is invisible because the planner's
property-field-name derivation happens to match the emitter's
derivation for every PL011 property. The second defect is
invisible because PL011 only declares u32 properties. Any other
driver triggers broken C:

- A bool property emits a u32 read against the Linux firmware
  property API (`device_property_read_bool` has a different
  signature; the emitted call is a type error).
- A u64 property emits a u32 read (silent 32-bit truncation).
- A property whose authored `field_name` differs from the
  id-to-snake-case transform writes to a nonexistent struct
  field.

Additionally, the planner at
`crates/regseq-backends/src/linux/planner.rs:109-116` maps
authored property types to shared-struct C types via a four-entry
allowlist (`u32`/`u16`/`u8`/`bool` -> same, else `u32`). The
fallback is not diagnosed. `u64` is absent from the allowlist,
so a u64 property silently gets a `u32` field declaration in the
generated driver-state struct (a hard mis-type, not just a
mis-read). The read-API selection at lines 439-445 does handle
u64 and bool but still falls unknown spellings to
`device_property_read_u32`.

This task unifies property handling behind a single exhaustive
typed enum `LinuxPropertyScalar`, makes the emitter consume
`LinuxPropertyRealization` instead of re-deriving from
`property_id`, and fails `build_linux_backend_plan` with a
structured error on unsupported scalar spellings.

Source: `.harness/spec-batches/device-generality-cleanup.md`
Findings 10, 13.

## Input Context

- `.harness/spec-batches/device-generality-cleanup.md` Findings
  10, 13, and the Task B scope section.
- `.harness/tasks/regseq-linux-backend/03-linux-plan-self-sufficiency.md`
  -- the prior boundary-cleanup task that promoted
  `LinuxPropertyRealization` fields onto the plan.
- `crates/regseq-backends/src/linux/plan.rs` -- the
  `LinuxPropertyRealization` struct (currently carries
  `c_type: String`, `linux_read_api: String`).
- `crates/regseq-backends/src/linux/planner.rs` lines 104-131
  (shared-struct field C-type mapping) and 434-454
  (`build_property_realization`) -- the two call sites that
  string-match on `field_type`.
- `crates/regseq-backends/src/linux/emit_module.rs` lines
  700-718 -- the `ReadProperty` probe step emission this task
  rewrites.
- `crates/regseq-backends/src/lowering/plan.rs` --
  `PropertyObligation.field_type: String`; this field is the
  authored scalar name from the driver declaration. This task
  does not change the portable type; it narrows the Linux
  planner's interpretation of it.

## Implementation Notes

### New typed enum

In `crates/regseq-backends/src/linux/plan.rs` (or a new
submodule if the file is over the 500-line convention):

```rust
/// Authored scalar type for a Linux firmware property.
///
/// Exhaustively enumerates every scalar type the Linux property
/// realization supports. Unknown spellings fail plan
/// construction; they are NOT silently promoted to `U32`.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum LinuxPropertyScalar {
    U8,
    U16,
    U32,
    U64,
    Bool,
}

impl LinuxPropertyScalar {
    /// Map an authored scalar spelling (`"u8"`, `"u16"`, `"u32"`,
    /// `"u64"`, `"bool"`) to the typed variant. Returns `None`
    /// on unknown spellings; callers surface that as a
    /// `LinuxPlanError::UnsupportedPropertyScalar`.
    pub fn parse(spelling: &str) -> Option<Self> { /* ... */ }

    /// Linux kernel C type name (e.g. `"u32"`, `"bool"`).
    pub fn c_type(self) -> &'static str { /* ... */ }

    /// Linux firmware property-read function name
    /// (e.g. `"device_property_read_u32"`).
    pub fn read_api(self) -> &'static str { /* ... */ }

    /// Call shape for this scalar's Linux property-read API.
    ///
    /// Linux's `device_property_read_bool` returns `bool`
    /// directly (presence test), while the integer variants
    /// return `int` and write the value through an out-pointer.
    /// The emitter branches on this to render the correct call
    /// shape.
    pub fn call_shape(self) -> LinuxPropertyReadShape {
        match self {
            Self::Bool => LinuxPropertyReadShape::AssignReturn,
            _ => LinuxPropertyReadShape::OutPointer,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum LinuxPropertyReadShape {
    /// Integer API shape: `int device_property_read_u32(dev,
    /// name, u32 *dest)`. The emitter renders
    /// `ret = api(dev, "{id}", &d->{field});` and branches on
    /// `ret` for default / required handling.
    OutPointer,
    /// Bool API shape: `bool device_property_read_bool(dev,
    /// name)`. The emitter renders
    /// `d->{field} = device_property_read_bool(dev, "{id}");`
    /// with no `ret` variable and no error branch (the return
    /// value IS the presence answer).
    AssignReturn,
}
```

### `LinuxPropertyRealization` changes

```rust
pub struct LinuxPropertyRealization {
    pub property_id: String,
    pub field_name: String,
    /// Typed scalar. Replaces the prior `c_type: String` +
    /// `linux_read_api: String` duo; both are derived from this
    /// via `LinuxPropertyScalar::c_type()` and
    /// `LinuxPropertyScalar::read_api()`.
    pub scalar: LinuxPropertyScalar,
    pub required: bool,
    pub default_literal: Option<String>,
}
```

Delete `c_type: String` and `linux_read_api: String` as
independent string fields; the typed enum is the single source
of truth.

### Planner changes

In `crates/regseq-backends/src/linux/planner.rs`:

1. `build_shared_struct_fields` (currently line 109-116) replaces
   its four-arm string match with
   `LinuxPropertyScalar::parse(&prop.field_type)`. Unknown
   spellings propagate as
   `LinuxPlanError::UnsupportedPropertyScalar { property_id,
   spelling }`.

2. `build_property_realization` (currently lines 434-454) uses
   the same `LinuxPropertyScalar::parse` call; on failure,
   bubbles the same error.

3. `build_linux_backend_plan` returns `Result<...,
   LinuxPlanError>` as today; the new error variant is
   additive.

### `LinuxPlanError` extension

```rust
#[derive(Debug, thiserror::Error)]
pub enum LinuxPlanError {
    #[error("linux plan error: {reason}")]
    PlanError { reason: String },

    #[error("unsupported property scalar type '{spelling}' on property '{property_id}'")]
    UnsupportedPropertyScalar {
        property_id: String,
        spelling: String,
    },
    // (If task 06 lands first, AmbiguousRegisterBlockOwnership is
    // already present. The two error variants are independent.)
}
```

### Emitter changes

In `crates/regseq-backends/src/linux/emit_module.rs` lines
700-718 (`LinuxProbeStep::ReadProperty`):

1. Remove the `field_name = property_id.replace('-', "_")`
   derivation. Look up the matching
   `LinuxPropertyRealization` on the plan via `property_id` and
   read its `field_name`.

2. Branch on `realization.scalar.call_shape()`:

   - `LinuxPropertyReadShape::OutPointer` (u8/u16/u32/u64): emit
     the existing integer-property pattern with
     `realization.scalar.read_api()` substituted for the
     hardcoded `device_property_read_u32`:

     ```c
     ret = {read_api}(dev, "{property_id}", &d->{field_name});
     if (ret)
         d->{field_name} = {UPPER}_DEFAULT_{PROP_UPPER};  /* has_default */
     // OR
     if (ret)
         return ret;                                       /* !has_default */
     ```

   - `LinuxPropertyReadShape::AssignReturn` (bool): emit the
     presence-test pattern with no `ret` variable and no error
     branch:

     ```c
     d->{field_name} = device_property_read_bool(dev, "{property_id}");
     ```

     Bool properties never take a `has_default` fallback: the
     return value IS the presence answer, so a default is
     semantically meaningless. If the authored property carries
     `default_literal: Some(_)`, the **emitter silently
     ignores** it for bool properties (no `_DEFAULT_` define is
     emitted, no fallback branch is rendered). The planner does
     NOT reject the combination with an error; authoring-surface
     validation of "bool properties should not carry defaults"
     is an upstream concern and out of scope for this task. The
     existing planner-level `LinuxPlanError` surface gains only
     `UnsupportedPropertyScalar` from this task; no
     `BoolPropertyHasDefault` variant.

3. The define-name construction for the default fallback
   (`{upper}_DEFAULT_{prop_upper}`) stays unchanged; it matches
   the spelling produced by `emit_constants` at
   `crates/regseq-backends/src/linux/emit_module.rs:238-246`.

### PL011 parity

PL011's `current-speed` (u32) and `fifo-size` (u32) property
declarations match the id-to-snake-case derivation and use
`device_property_read_u32`. After migration, the same values flow
through the typed enum and produce byte-identical C.

### Grep gates

```
rg -n 'property_id.replace' crates/regseq-backends/src/linux/
# must return zero matches in emit_module.rs (derivation gone).
# One match may remain in emit_module.rs emit_constants (line
# 249-253) for the define-name upper-casing, which is a
# separate derivation that stays.

rg -n 'device_property_read_u32' crates/regseq-backends/src/linux/emit_module.rs
# must return zero matches (dispatched via scalar.read_api()).

rg -n 'linux_read_api' crates/regseq-backends/src/
# must return zero matches after migration (field deleted from
# LinuxPropertyRealization).
```

### What this task does NOT do

- Does not add non-scalar property support (arrays, strings,
  enums). Those are out of scope; the plan fails hard on them.
- Does not modify the portable `DriverLoweringPlan`.
  `PropertyObligation.field_type` stays a `String` because
  portable lowering does not enforce a Linux-specific scalar
  vocabulary.
- Does not fix per-resource probe emission (owned by task 06).
- Does not modify `emit_semantic.rs`.

## Non-Goals

- No Zephyr backend work.
- No changes to authoring surface.
- No new fixture beyond the bool / u64 / rejected-scalar unit
  tests below.
- No performance or optimization work.

## Test Plan

### Unit tests

1. `LinuxPropertyScalar::parse` round-trips all five spellings.
   Unknown spelling returns `None`.

2. `build_linux_backend_plan` fails with
   `LinuxPlanError::UnsupportedPropertyScalar` when the portable
   plan has a property with `field_type = "i32"` (or any other
   spelling outside the allowlist). The error message includes
   both the property id and the spelling.

3. `build_linux_backend_plan` succeeds for a bool-property
   portable plan; `LinuxPropertyRealization.scalar` is
   `LinuxPropertyScalar::Bool`. Likewise u64.

### Emission tests

4. Bool-property fixture: hand-build a portable plan with one
   `bool` property (`name = "wakeup-source"`, `field_name = "wakeup"`).
   Emit artifacts. Assert the generated probe body contains
   the presence-test assignment shape
   `d->wakeup = device_property_read_bool(dev, "wakeup-source");`
   (not the integer-style `ret = ...(&d->wakeup)` shape). Assert
   no `ret` variable is declared for the bool read and no `if
   (ret)` branch follows it.

5. u64-property fixture: likewise with
   `device_property_read_u64` writing to the authored
   `field_name`.

6. Custom-field-name fixture: property with
   `property_id = "tx-threshold-level"` and `field_name =
   "tx_high_water"`. Assert the emitted probe writes to
   `d->tx_high_water`, not `d->tx_threshold_level`.

### Output-parity smoke (PL011)

7. PL011 byte-identical emission before and after (capture/compare
   per task 03 pattern). PL011's u32 properties round-trip
   through `LinuxPropertyScalar::U32` with no visible C change.

### Clippy / format / regression

8. `cargo test --workspace` green.
9. `cargo fmt --all -- --check` clean.
10. `cargo clippy -p regseq-backends -- -D warnings` clean.

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540 --message-only
<task>
Review regseq-linux-backend-07 against
.harness/tasks/regseq-linux-backend/07-property-realization-honoring.md.

Files to audit:
- crates/regseq-backends/src/linux/plan.rs (LinuxPropertyScalar +
  LinuxPropertyRealization changes; c_type + linux_read_api
  fields deleted)
- crates/regseq-backends/src/linux/planner.rs (single parse call
  site; LinuxPlanError::UnsupportedPropertyScalar)
- crates/regseq-backends/src/linux/emit_module.rs
  (ReadProperty consumes realization.field_name +
  scalar.read_api())
- crates/regseq-backends/tests/ (bool/u64/custom-field/rejected
  tests + PL011 byte-parity smoke)

Focus on:
- ReadProperty no longer derives field name from property_id;
  reads LinuxPropertyRealization.field_name.
- `device_property_read_u32` no longer appears as a literal in
  emit_module.rs; dispatched via scalar.read_api().
- `linux_read_api: String` and `c_type: String` fields on
  LinuxPropertyRealization are gone; LinuxPropertyScalar is the
  single source of truth.
- Unknown scalar spellings fail plan construction with
  UnsupportedPropertyScalar; no silent fallback to u32.
- PL011 byte parity preserved (PL011 only has u32 properties).

Do NOT flag non-scalar property support -- explicitly out of
scope.
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
cargo test -p regseq-backends
cargo test --workspace
cargo fmt --all -- --check
cargo clippy -p regseq-backends -- -D warnings
```

## Acceptance Criteria

- `LinuxPropertyScalar` enum defined with five variants
  (`U8`, `U16`, `U32`, `U64`, `Bool`) and
  `parse`/`c_type`/`read_api` methods.
- `LinuxPropertyRealization` carries `scalar:
  LinuxPropertyScalar` instead of independent `c_type: String`
  and `linux_read_api: String` fields.
- `LinuxPlanError::UnsupportedPropertyScalar { property_id,
  spelling }` variant added.
- `emit_module.rs` `ReadProperty` emission reads
  `realization.field_name` and `realization.scalar.read_api()`;
  no `property_id.replace('-', "_")` derivation for the target
  field, no hardcoded `device_property_read_u32` literal.
- Bool, u64, and custom-field-name property fixtures produce
  correct emitted C.
- Unknown-spelling property fixture fails
  `build_linux_backend_plan` with the structured error.
- PL011 emission byte-identical to baseline.
- Codex review APPROVED.
