# regseq-linux-backend-08: RegisterUartPort Serial-Resource Generality

**Depends on**: `regseq-linux-backend-03`, `regseq-linux-backend-06`,
`regseq-linux-backend-07`.

## Description

Post-device-generality-cleanup audit fix. Closes four remaining
PL011-shaped leaks inside `linux::emit_module.rs`'s
`RegisterUartPort` handling and its sibling
`emit_uartclk_initializers`. These leaks were not part of the
original 14-finding catalog in
`.harness/spec-batches/device-generality-cleanup.md`; they were
surfaced by a follow-up two-source device-independence audit
(Claude + Codex) after
`regseq-linux-backend-06`, `regseq-linux-backend-07`, and
`regseq-lowering-12` landed.

The four leaks, each in the same serial-port
framework-integration region of `emit_module.rs`:

1. **Primary MMIO and IRQ positional picker**:
   `primary_mmio_field` / `primary_irq_field` helpers at
   `crates/regseq-backends/src/linux/emit_module.rs:648-665`
   return the FIRST `MapMmio` / `RequestIrq` step in probe order
   (consumed at
   `crates/regseq-backends/src/linux/emit_module.rs:841-846`).
   For a UART with multiple MMIO regions or multiple IRQs, the
   serial port (`d->port.membase`, `d->port.irq`) is silently
   wired to the first resource in probe order, not to a
   plan-selected "primary" region.

2. **Uartclk clock selection is order-dependent**:
   `emit_uartclk_initializers` at
   `crates/regseq-backends/src/linux/emit_module.rs:675-698`
   iterates **every** `LinuxProbeStep::GetClock` and emits one
   `d->port.uartclk = ...` line per clock. For a multi-clock UART,
   the last clock in probe order wins at runtime (later
   assignments overwrite earlier ones); there is no plan-level
   decision about which clock backs `uart_port.uartclk`.

3. **Serial-speed default string-match on `property_id`**:
   Same function, at
   `crates/regseq-backends/src/linux/emit_module.rs:684-690`,
   selects the current-speed default literal by matching
   `p.property_id == "current-speed"`. A driver whose
   speed-default property carries any other authored id silently
   falls through to the hardcoded `"115200"` literal -- losing
   the authored default.

4. **FIFO-size wiring string-match on `field_name`**:
   `crates/regseq-backends/src/linux/emit_module.rs:853-860`
   matches `realization.field_name == "fifo_size"` to decide
   whether to emit `d->port.fifosize = d->fifo_size;`. A driver
   whose FIFO-size property has any other authored field name
   (e.g. `"fifo_depth"`) silently drops the wiring. This is the
   anti-pattern Finding 9 in the cleanup brief called out for
   `is_option_pointer_context` field-name matching --
   reincarnated on a property.

All four are implicit device-level conventions baked into the
Linux backend core. This task replaces them with typed plan
fields, populated by the Linux planner, consumed by the emitter.
Authoring-surface overrides (where authors mark a specific
resource, clock, or property as the serial-port primary / uartclk
reference / FIFO-size / clock-rate-default via `#[rseq::...]`
attributes) are **deferred to a future authoring-surface task**;
the planner's initial population uses today's conventions
(first-in-probe-order primary MMIO / IRQ / clock;
`property_id == "current-speed"` for clock-rate default;
`property_id == "fifo-size"` for FIFO size), but the conventions
live in ONE place (the planner), typed on the plan, and flagged
with TODO doc comments pointing to the future override task. This
mirrors the `name_hint: None` pattern task 06 used for multi-clock
consumer-name resolution.

This task does **not** fix the `uart_driver.nr = 1` literal
(Finding 7 in the cleanup brief, owned by
`regseq-linux-backend-05`'s amendment), the first-block register
prefix (Finding 8, owned by task 05's amendment), or the
`is_option_pointer_context` field-name match (Finding 9, owned
by `regseq-linux-backend-04`'s amendment). Those three remain the
responsibility of the pending CFG-rewrite tasks.

## Input Context

- `.harness/spec-batches/device-generality-cleanup.md` -- the
  originating brief (14 findings); the four leaks this task
  addresses are adjacent to but not in that catalog.
- `.harness/tasks/regseq-linux-backend/03-linux-plan-self-sufficiency.md`
  -- introduced `LinuxSerialCoreRealization` and
  `LinuxPropertyRealization`; this task extends both.
- `.harness/tasks/regseq-linux-backend/06-per-resource-probe-schema.md`
  -- introduced per-step `field_name` on `LinuxProbeStep`; the
  planner's new primary-resource / uartclk population walks those
  steps.
- `.harness/tasks/regseq-linux-backend/07-property-realization-honoring.md`
  -- introduced `LinuxPropertyScalar` and reshaped
  `LinuxPropertyRealization`; this task adds a sibling typed role
  field.
- `.harness/tasks/regseq-linux-backend/05-emit-module-cfg-rewrite-and-emit-behavior-delete.md`
  -- the pending CFG rewrite that owns Findings 7 and 8 via its
  device-generality amendment; task 08's coordination section
  below extends that amendment and adds a dependency.
- `crates/regseq-backends/src/linux/plan.rs`
  (`LinuxSerialCoreRealization`, `LinuxPropertyRealization`,
  `LinuxPlanError`) -- the plan-level types this task extends.
- `crates/regseq-backends/src/linux/emit_module.rs` lines
  648-665 (`primary_mmio_field`, `primary_irq_field`), 675-699
  (`emit_uartclk_initializers`), 834-864 (`RegisterUartPort`
  case) -- the emission sites this task rewrites.
- `crates/regseq-backends/src/linux/planner.rs` -- the planner
  that populates `LinuxSerialCoreRealization` and
  `LinuxPropertyRealization` today; this task extends the
  population.
- `crates/regseq-backends/tests/` -- the multi-resource test
  fixture from task 06 (extended here) and the PL011 scaffold
  (byte-parity guarantee).

## Implementation Notes

### Plan type changes

In `crates/regseq-backends/src/linux/plan.rs`:

1. Extend `LinuxSerialCoreRealization` with four new fields.
   `primary_mmio_field`, `primary_irq_field`, and `uartclk_field`
   are required `String`s because a Linux serial port cannot be
   emitted without a primary MMIO (`uart_port.membase`), a
   primary IRQ (`uart_port.irq`), or a clock backing
   `uart_port.uartclk` (serial-core baud-rate division reads it).
   If any is missing the plan build fails with a structured
   error (see below). `uartclk_optional` carries the
   optional-flag from the selected clock's `GetClock` step so
   the emitter can decide between the direct-rate form and the
   ternary-fallback form.

```rust
pub struct LinuxSerialCoreRealization {
    // ... existing fields (uart_driver_name, uart_ops_name,
    //     realized_ops, port_type, dev_name) ...

    /// Driver-struct field name for the MMIO region the serial
    /// port uses for `uart_port.membase`. The emitter renders
    /// `d->port.membase = d->{primary_mmio_field};`.
    ///
    /// Today populated by the Linux planner as the `field_name`
    /// of the first `LinuxProbeStep::MapMmio` in probe order.
    /// Authoring-surface override (e.g., `#[rseq::serial_primary]`
    /// on a specific `Mmio<T>` field) is a future task; until
    /// that lands, a multi-MMIO UART silently wires the first
    /// region. The planner documents this via a doc comment at
    /// the population site.
    pub primary_mmio_field: String,

    /// Driver-struct field name for the IRQ the serial port uses
    /// for `uart_port.irq`. The emitter renders
    /// `d->port.irq = d->{primary_irq_field};`.
    ///
    /// Same future-override caveat as `primary_mmio_field`.
    pub primary_irq_field: String,

    /// Driver-struct field name for the clock that backs
    /// `uart_port.uartclk`. The emitter renders EXACTLY ONE
    /// `d->port.uartclk = ...` line using this field; multi-clock
    /// drivers no longer collide on this slot.
    ///
    /// Today populated by the Linux planner as the `field_name`
    /// of the first `LinuxProbeStep::GetClock` in probe order.
    /// Same future-override caveat as `primary_mmio_field`.
    pub uartclk_field: String,

    /// Whether the clock at `uartclk_field` was declared
    /// optional. Carried on the serial-core realization so the
    /// emitter does not re-walk `plan.probe.steps` to answer the
    /// same question.
    pub uartclk_optional: bool,
}
```

2. Add a typed serial-port role enum and a field on
   `LinuxPropertyRealization`:

```rust
/// Serial-port wiring role for a Linux firmware property.
///
/// Identifies properties that the serial-core integration
/// consumes beyond their generic `ReadProperty` probe-step
/// emission. Allows the `RegisterUartPort` emission step to
/// decide what to wire based on typed role metadata rather than
/// string-matching authored ids or field names.
///
/// Additional roles MAY be added as other serial-core fields
/// surface (e.g., `RxFifoTriggerLevel`, `AutoFlowEnabled`).
/// Non-serial drivers leave this field `None` (the planner
/// gates role assignment on whether
/// `LinuxSerialCoreRealization` is also being populated).
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SerialPortRole {
    /// Property provides the FIFO depth. The emitter renders
    /// `d->port.fifosize = d->{field_name};` inside the
    /// `RegisterUartPort` step.
    FifoSize,
    /// Property provides the clock-rate default used when the
    /// serial-core clock is optional and missing at runtime.
    /// The emitter substitutes `{DRIVER_UPPER}_DEFAULT_{PROP_UPPER}`
    /// (the define already emitted for properties with a
    /// `default_literal`) into the ternary fallback in
    /// `emit_uartclk_initializers`. When the property has no
    /// `default_literal`, the emitter keeps the current
    /// framework-constant fallback of `"115200"` (documented as
    /// the Linux UART subsystem default baud rate, not a
    /// device-specific hardcode).
    ClockRateDefault,
}

pub struct LinuxPropertyRealization {
    // ... existing fields (property_id, field_name, scalar,
    //     required, default_literal) ...

    /// Serial-port wiring role, if any. `None` for properties
    /// that only participate in generic `ReadProperty` emission.
    /// Set to `Some(_)` ONLY when the driver is a serial-core
    /// driver (i.e. `plan.serial_core.is_some()` is also true
    /// on the constructed plan).
    pub serial_role: Option<SerialPortRole>,
}
```

3. Extend `LinuxPlanError` with three new variants:

```rust
#[derive(Debug, thiserror::Error)]
pub enum LinuxPlanError {
    // ... existing variants (PlanError, AmbiguousRegisterBlockOwnership,
    //     UnsupportedPropertyScalar) ...

    #[error("serial driver '{driver_name}' has no MapMmio probe step; \
             cannot populate LinuxSerialCoreRealization.primary_mmio_field")]
    MissingSerialPortPrimaryMmio { driver_name: String },

    #[error("serial driver '{driver_name}' has no RequestIrq probe \
             step; cannot populate LinuxSerialCoreRealization.primary_irq_field")]
    MissingSerialPortPrimaryIrq { driver_name: String },

    #[error("serial driver '{driver_name}' has no GetClock probe \
             step; cannot populate LinuxSerialCoreRealization.uartclk_field")]
    MissingSerialPortUartClk { driver_name: String },
}
```

All three fire only when `serial_core` is being populated on the
plan (i.e., the driver is a serial device). Non-serial drivers
never hit these paths.

### Planner changes

In `crates/regseq-backends/src/linux/planner.rs`:

1. Where `LinuxSerialCoreRealization` is constructed, after the
   probe steps are built, populate the four new fields by walking
   `probe.steps`:

```rust
// Inside the serial-core population branch:
let primary_mmio_field = probe
    .steps
    .iter()
    .find_map(|s| match s {
        LinuxProbeStep::MapMmio { field_name, .. } => {
            Some(field_name.clone())
        }
        _ => None,
    })
    .ok_or_else(|| LinuxPlanError::MissingSerialPortPrimaryMmio {
        driver_name: plan.identity.driver_name.clone(),
    })?;
let primary_irq_field = probe
    .steps
    .iter()
    .find_map(|s| match s {
        LinuxProbeStep::RequestIrq { field_name, .. } => {
            Some(field_name.clone())
        }
        _ => None,
    })
    .ok_or_else(|| LinuxPlanError::MissingSerialPortPrimaryIrq {
        driver_name: plan.identity.driver_name.clone(),
    })?;
let (uartclk_field, uartclk_optional) = probe
    .steps
    .iter()
    .find_map(|s| match s {
        LinuxProbeStep::GetClock { field_name, optional, .. } => {
            Some((field_name.clone(), *optional))
        }
        _ => None,
    })
    .ok_or_else(|| LinuxPlanError::MissingSerialPortUartClk {
        driver_name: plan.identity.driver_name.clone(),
    })?;
// TODO(regseq-authoring-surface-follow-up): replace
// first-in-probe-order selection with an authored-override lookup
// once `#[rseq::serial_primary]` / `#[rseq::uartclk_source]` (or
// equivalent) lands.
```

2. The serial_role assignment in `build_property_realization`
   (existing function from task 07) is gated so that non-serial
   drivers never get tagged. Concretely: the planner determines
   "will this driver have serial-core realization" BEFORE
   building property realizations (the existing planner logic
   already computes this; see the condition guarding
   `LinuxSerialCoreRealization` construction). The implementer
   threads that boolean into `build_property_realization` (e.g.,
   an added `is_serial_driver: bool` parameter) and uses it:

```rust
let serial_role = if is_serial_driver {
    match property_id.as_str() {
        "fifo-size" => Some(SerialPortRole::FifoSize),
        "current-speed" => Some(SerialPortRole::ClockRateDefault),
        _ => None,
    }
} else {
    None
};
// TODO(regseq-authoring-surface-follow-up): replace the id-match
// with a typed lookup keyed on an authored `#[rseq::serial_role(...)]`
// attribute on the property declaration.
```

   The convention strings (`"fifo-size"`, `"current-speed"`)
   live in exactly ONE place (this function) and are the only
   remaining device-level string constants in the Linux backend
   planner. The doc comment must name them as "transitional
   authoring conventions" and cite the follow-up task.

### Emitter changes

In `crates/regseq-backends/src/linux/emit_module.rs`:

1. **Delete** the `primary_mmio_field` and `primary_irq_field`
   helper functions (currently lines 648-665). Their call sites
   in the `RegisterUartPort` case read
   `serial.primary_mmio_field` and `serial.primary_irq_field`
   directly:

```rust
LinuxProbeStep::RegisterUartPort => {
    let serial = plan.serial_core.as_ref().unwrap();
    body.push("/* Serial port setup */".into());
    body.push(format!("d->port.dev = {dev_expr};"));
    if plan.registration.is_amba {
        body.push("d->port.mapbase = adev->res.start;".into());
    }
    body.push(format!(
        "d->port.membase = d->{};", serial.primary_mmio_field
    ));
    body.push(format!(
        "d->port.irq = d->{};", serial.primary_irq_field
    ));
    emit_uartclk_initializer(&mut body, plan, &upper);
    // ... (FIFO wiring, see below) ...
}
```

   The `if let Some(...)` wrappers around the previous helper
   returns are deleted; the new fields are always populated
   (planner errors if not).

2. Rewrite the FIFO wiring to iterate `property_realization`
   and match on `serial_role`:

```rust
for realization in &plan.property_realization {
    if realization.serial_role == Some(SerialPortRole::FifoSize) {
        body.push(format!(
            "d->port.fifosize = d->{};", realization.field_name
        ));
    }
}
```

   No `field_name == "fifo_size"` string match. Zero or multiple
   `FifoSize`-tagged properties are left to the planner (today:
   zero or one, by id-match convention; future: enforced via
   authoring-surface validation).

3. Rename `emit_uartclk_initializers` to
   `emit_uartclk_initializer` (singular) and rewrite as a scalar
   emitter that reads `serial.uartclk_field` / `uartclk_optional`
   and finds the `ClockRateDefault` via `serial_role`:

```rust
fn emit_uartclk_initializer(
    body: &mut Vec<String>,
    plan: &LinuxBackendPlan,
    upper: &str,
) {
    let serial = plan
        .serial_core
        .as_ref()
        .expect("emit_uartclk_initializer called without serial_core");

    // Linux UART subsystem default baud rate (115200 bps).
    // Not a device-specific hardcode.
    let default_speed_fallback = "115200";
    let default_speed_define = plan
        .property_realization
        .iter()
        .find(|p| p.serial_role == Some(SerialPortRole::ClockRateDefault))
        .and_then(|p| {
            p.default_literal.as_ref().map(|_| {
                let prop_upper = p.property_id.replace('-', "_").to_uppercase();
                format!("{upper}_DEFAULT_{prop_upper}")
            })
        })
        .unwrap_or_else(|| default_speed_fallback.to_string());

    let field = &serial.uartclk_field;
    if serial.uartclk_optional {
        body.push(format!(
            "d->port.uartclk = d->{field} ? \
             clk_get_rate(d->{field}) : {default_speed_define};"
        ));
    } else {
        body.push(format!(
            "d->port.uartclk = clk_get_rate(d->{field});"
        ));
    }
}
```

   Exactly ONE `d->port.uartclk` assignment line is emitted. No
   iteration over probe steps. The `"current-speed"` property-id
   literal no longer appears in this function. The `"115200"`
   framework-constant fallback stays (it is the Linux UART
   subsystem default baud rate, not a device-specific hardcode;
   the short comment above the constant names it as such).

4. No other changes to `emit_module.rs` in this task.

### Coordination with regseq-linux-backend-05

Task 05 rewrites `emit_module.rs` wholesale (CFG retargeting).
If task 05 runs after task 08 without being updated, the
RegisterUartPort generality invariants landed here CAN be
silently undone when the CFG rewrite lands. Task 08 therefore
performs two coordination edits on the task 05 spec as part of
its implementation (not at runtime by the task 05 implementer):

1. **Amend task 05's dependency list**: update both the spec
   file and `.harness/tasks.json` to add
   `regseq-linux-backend-08` as a dependency of task 05.

   - **Spec-file edit**: at the top of
     `.harness/tasks/regseq-linux-backend/05-emit-module-cfg-rewrite-and-emit-behavior-delete.md`,
     the line currently reads:

     ```
     **Depends on**: `regseq-backends-02`, `regseq-linux-backend-03`,
     `regseq-linux-backend-04`.
     ```

     Replace with:

     ```
     **Depends on**: `regseq-backends-02`, `regseq-linux-backend-03`,
     `regseq-linux-backend-04`, `regseq-linux-backend-08`.
     ```

   - **`.harness/tasks.json` edit**: `scripts/python/task.py` has
     no dependency-editing subcommand (the registered commands
     are `next`, `next-step`, `show`, `start`, `done`, `fail`,
     `reset`, `status`, `list`, `blocked`, `doctor`). The
     dependency edit is therefore a direct JSON edit -- the same
     mechanism the original
     `.harness/spec-batches/device-generality-cleanup.md`
     Update Instructions used when it added tasks 06 and 07 to
     task 05's depends list. The "do not modify tasks.json
     directly" rule in `.harness/SESSION.md` applies to task
     STATUS transitions (which the CLI manages); spec-authoring
     dependency edits go direct.

     Locate the task 05 entry in `.harness/tasks.json` (today at
     `"id": "regseq-linux-backend-05"`, approximately line
     1857) and append `"regseq-linux-backend-08"` to its
     `depends_on` array. The resulting array is:

     ```json
     "depends_on": [
         "regseq-backends-02",
         "regseq-linux-backend-03",
         "regseq-linux-backend-04",
         "regseq-linux-backend-06",
         "regseq-linux-backend-07",
         "regseq-linux-backend-08"
     ],
     ```

     Run `uv run python scripts/python/task.py show regseq-linux-backend-05`
     after the edit to verify the dependency is picked up.

2. **Extend task 05's device-generality amendment section**:
   append a bullet-point sub-section titled "Findings from
   regseq-linux-backend-08 (RegisterUartPort generality)"
   enumerating the four invariants task 05 MUST preserve:
   - `d->port.membase` is rendered from
     `LinuxSerialCoreRealization.primary_mmio_field`, not a
     first-in-probe-order helper.
   - `d->port.irq` is rendered from
     `LinuxSerialCoreRealization.primary_irq_field`.
   - Exactly ONE `d->port.uartclk` assignment is emitted, using
     `LinuxSerialCoreRealization.uartclk_field` and
     `uartclk_optional` (not an iteration over all `GetClock`
     steps).
   - `d->port.fifosize` is wired iff a
     `LinuxPropertyRealization` carries
     `serial_role == Some(FifoSize)`; the clock-rate default
     lookup uses `serial_role == Some(ClockRateDefault)`. No
     `property_id` / `field_name` string literals.

The spec-file edit + `tasks.json` edit must land in the same
commit as the task 08 code changes. If task 05 has been rewritten
between task 08 being spec'd and implemented, the task 08
implementer ports the amendment forward using the same four
bullets (adjusting file:line citations as needed).

### Grep gates

After this task:

```
rg -n 'fn primary_mmio_field|fn primary_irq_field' crates/regseq-backends/src/linux/emit_module.rs
# zero matches: the two helper fn definitions are deleted.

rg -n '"current-speed"|"fifo-size"|"fifo_size"' crates/regseq-backends/src/linux/emit_module.rs
# zero matches. The two authored-id conventions live in
# planner.rs build_property_realization only.

rg -n 'serial_role' crates/regseq-backends/src/linux/
# multiple matches expected: the enum definition, the
# LinuxPropertyRealization field, the planner assignment, and
# the emitter consumers (FIFO + clock-rate default).

rg -c 'd->port\.uartclk' crates/regseq-backends/src/linux/emit_module.rs
# exactly 1 string-formatting site (inside emit_uartclk_initializer);
# grep returns one file with count 1. A count >= 2 flags
# re-introduction of per-clock iteration.

rg -c 'd->port\.fifosize' crates/regseq-backends/src/linux/emit_module.rs
# exactly 1 string-formatting site.

rg -c 'd->port\.membase' crates/regseq-backends/src/linux/emit_module.rs
# exactly 1: the single assignment rendered from
# serial.primary_mmio_field.

rg -c 'd->port\.irq' crates/regseq-backends/src/linux/emit_module.rs
# exactly 1: the single assignment rendered from
# serial.primary_irq_field.

rg -n '"115200"' crates/regseq-backends/src/linux/
# at most one match, in emit_module.rs::emit_uartclk_initializer,
# documented as the Linux UART subsystem default baud rate.
```

### What this task does NOT do

- Does not touch the `.nr = 1` literal at
  `emit_module.rs:643` (Finding 7; owned by task 05's amendment).
- Does not touch the `plan.register_layout.blocks.first()` idiom
  at `emit_module.rs:273, 315` or `emit_regs.rs:54`
  (Finding 8; owned by task 05's amendment).
- Does not modify `emit_semantic.rs` or the
  `is_option_pointer_context` field-name match (Finding 9; owned
  by task 04's amendment).
- Does not add an authoring-surface attribute. No
  `#[rseq::serial_primary]`, no `#[rseq::serial_role(...)]`, no
  `#[rseq::uartclk_source]`. That work is a separate follow-up
  task, cited only in TODO doc comments.
- Does not modify the portable `DriverLoweringPlan` or
  `PropertyObligation`. The role and primary-resource / uartclk
  metadata are Linux-specific and live on the Linux plan only;
  a portable plan extension is a later design question if another
  backend (e.g., Zephyr) surfaces the same need.
- Does not add new `SerialPortRole` variants beyond `FifoSize`
  and `ClockRateDefault`. Other serial-core fields (`iotype`,
  `flags`, `rs485`, `rxfifo_trig_level`) are out of scope; if
  they need plan-level generality in the future, the enum grows
  in a follow-up task.

## Non-Goals

- No Zephyr / other-OS backend work.
- No authoring surface changes.
- No changes to the PL011 fixture or golden files. PL011 has
  one MMIO, one IRQ, one optional clock, `current-speed` (u32,
  default) and `fifo-size` (u32) -- after migration it flows
  through `primary_mmio_field = "regs"`, `primary_irq_field =
  "irq"`, `uartclk_field = <PL011 clock field>`,
  `uartclk_optional = true`, `serial_role = FifoSize` on
  fifo-size, `serial_role = ClockRateDefault` on current-speed;
  PL011 emission is byte-identical.
- No performance or optimization work.
- No new top-level plan types. The extension is four fields on
  `LinuxSerialCoreRealization`, one field on
  `LinuxPropertyRealization`, one enum, three error variants.

## Test Plan

### Unit tests

1. `LinuxPlanError::MissingSerialPortPrimaryMmio` fires when
   `build_linux_backend_plan` is invoked on a serial driver whose
   probe sequence contains no `MapMmio` step. New test in
   `crates/regseq-backends/tests/multi_resource_plan.rs` (or
   whichever file houses the task-06 multi-resource fixtures).

2. `LinuxPlanError::MissingSerialPortPrimaryIrq` fires
   symmetrically for the missing-IRQ case.

3. `LinuxPlanError::MissingSerialPortUartClk` fires for the
   missing-clock case.

4. `build_property_realization` sets `serial_role =
   Some(SerialPortRole::FifoSize)` for a property with
   `property_id = "fifo-size"` and
   `Some(SerialPortRole::ClockRateDefault)` for a property with
   `property_id = "current-speed"`, **only when** the driver is
   a serial-core driver. For a non-serial driver (serial-core
   not populated), `serial_role = None` for both.

5. The planner populates `LinuxSerialCoreRealization.primary_mmio_field`,
   `primary_irq_field`, `uartclk_field`, and `uartclk_optional`
   with the first-in-probe-order values, for a hand-built
   portable plan with multiple MMIO / IRQ / clock resources
   (multi of each).

### Emission tests

6. **Multi-resource serial emission**: hand-build a portable
   plan with two MMIO regions (`memory_a`, `memory_b`), two IRQs
   (`primary_irq`, `secondary_irq`), two clocks
   (`bus_clk` required, `ref_clk` optional), and two
   properties: `fifo-size` -> `fifo_depth` field (u32), and
   `baud-default` -> `baud_rate` field (u32, non-role). Invoke
   `build_linux_backend_plan` + `emit_linux_artifacts`; assert:
   - Generated C contains exactly one `d->port.membase = d->memory_a;`
     line (first MMIO selected).
   - Generated C contains exactly one `d->port.irq = d->primary_irq;`
     line.
   - Generated C contains exactly one `d->port.uartclk = ...`
     line using `d->bus_clk` (first clock; required, so no
     ternary).
   - Generated C contains `d->port.fifosize = d->fifo_depth;`
     (not `d->fifo_size`).
   - Generated C does not wire `d->port.fifosize = d->baud_rate`
     (non-role property is not wired).

7. **Typed ClockRateDefault role is honored for any
   property id**: hand-build (or bypass-planner patch) a
   `LinuxBackendPlan` with one optional clock and a property
   `{property_id: "default-speed", field_name: "default_speed_val",
    scalar: U32, required: false, default_literal: Some("9600"),
    serial_role: Some(SerialPortRole::ClockRateDefault)}`.
   Invoke `emit_linux_artifacts`; assert the optional-clock
   ternary emits
   `{DRIVER_UPPER}_DEFAULT_DEFAULT_SPEED` (prop_upper derived
   from `"default-speed"`.replace('-',"_").to_uppercase()) in
   the `else` arm, not `"115200"`. This test verifies the
   EMITTER honors a typed role regardless of the property id;
   it is the regression guard for leak 3.

8. **Planner convention limitation**: drive the planner
   end-to-end on the same property as test 7, but with
   serial_role assignment coming from the planner's convention
   rather than being forced. Assert
   `realization.serial_role == None` for
   `property_id = "default-speed"`, and the emitted optional-clock
   ternary therefore uses `"115200"`. This test documents the
   current planner convention (id-match only), separate from the
   emitter-level role honoring tested in test 7. Both tests
   update together when the authoring-surface follow-up lands.

### Output-parity smoke (PL011)

9. Re-run the PL011 emission scaffold with the capture/compare
   pattern. PL011 resolves:
   - `primary_mmio_field` = PL011's authored MMIO field name
   - `primary_irq_field` = PL011's authored IRQ field name
   - `uartclk_field` = PL011's authored clock field name
   - `uartclk_optional = true` (PL011's clock is optional)
   - `serial_role = FifoSize` on the `fifo-size` property
   - `serial_role = ClockRateDefault` on `current-speed`
   - Emitted `d->port.fifosize = d->fifo_size;` byte-identical.
   - Optional-clock ternary uses
     `PL011_DEFAULT_CURRENT_SPEED` define, identical to
     pre-task output.

### Clippy / format / regression

10. `cargo test --workspace` green.
11. `cargo fmt --all -- --check` clean.
12. `cargo clippy -p regseq-backends -- -D warnings` clean.

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540 --message-only
<task>
Review regseq-linux-backend-08 against
.harness/tasks/regseq-linux-backend/08-register-uart-port-resource-generality.md.

Files to audit:
- crates/regseq-backends/src/linux/plan.rs (SerialPortRole enum;
  primary_mmio_field + primary_irq_field + uartclk_field +
  uartclk_optional on LinuxSerialCoreRealization;
  serial_role on LinuxPropertyRealization; three new LinuxPlanError
  variants)
- crates/regseq-backends/src/linux/planner.rs (primary-resource +
  uartclk population from probe.steps with structured errors on
  missing; serial_role population in build_property_realization
  gated on serial-core driver)
- crates/regseq-backends/src/linux/emit_module.rs (primary_mmio_field
  / primary_irq_field helper fns DELETED; RegisterUartPort reads
  serial.primary_mmio_field / primary_irq_field; FIFO wiring
  iterates property_realization on serial_role;
  emit_uartclk_initializer emits exactly ONE uartclk line using
  serial.uartclk_field / uartclk_optional; ClockRateDefault
  looked up via serial_role, not property_id)
- .harness/tasks/regseq-linux-backend/05-emit-module-cfg-rewrite-and-emit-behavior-delete.md
  (amended: depends_on gains regseq-linux-backend-08; amendment
  section gains "Findings from regseq-linux-backend-08" bullets)
- .harness/tasks.json (task 05 depends_on updated via task.py)
- crates/regseq-backends/tests/ (multi-resource + multi-clock +
  custom-field-name FIFO tests; missing-MMIO/IRQ/Clk error tests;
  typed-role ClockRateDefault emitter test; planner-convention
  limitation test; PL011 byte-parity smoke)

Focus on:
- No `fn primary_mmio_field` / `fn primary_irq_field` definitions
  in emit_module.rs (grep gate).
- No `"current-speed"` / `"fifo-size"` / `"fifo_size"` literals
  in emit_module.rs; both convention strings live only in
  planner.rs build_property_realization, gated on is_serial_driver.
- `d->port.fifosize` wiring key is
  `serial_role == Some(FifoSize)`, not `field_name == "fifo_size"`.
- `d->port.membase` / `d->port.irq` read from
  `serial.primary_mmio_field` / `serial.primary_irq_field`,
  populated by the planner.
- Exactly ONE `d->port.uartclk` assignment line emitted; uses
  `serial.uartclk_field` / `uartclk_optional`.
- `ClockRateDefault` is looked up via `serial_role`, not
  `property_id`.
- `LinuxPlanError::MissingSerialPortPrimaryMmio` /
  `MissingSerialPortPrimaryIrq` / `MissingSerialPortUartClk`
  fire with driver name when a serial driver's probe sequence
  lacks the respective step.
- `serial_role` is `None` for non-serial drivers even when their
  properties match the id convention.
- PL011 byte parity preserved.
- Task 05 spec amended in the same commit: depends_on includes
  task 08; amendment section extended with the four bullets.
- No changes to `uart_driver.nr = 1`, no changes to the
  first-block reg_prefix idiom, no changes to
  `is_option_pointer_context` -- those belong to tasks 05 and 04.

Do NOT flag:
- Absence of an authoring-surface override for primary
  resources, uartclk source, or serial roles (deferred to a
  follow-up task; this task MUST leave TODO doc comments citing
  the follow-up).
- The `"115200"` fallback literal; documented as the Linux UART
  subsystem default baud rate.
- `SerialPortRole` variants beyond `FifoSize` and
  `ClockRateDefault` -- out of scope.
</task>

<grounding_rules>
Cite file:line for every claim and every required fix. Do not
speculate about code you did not read. Mark hypotheses as
hypotheses.
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

- `LinuxSerialCoreRealization` carries `primary_mmio_field: String`,
  `primary_irq_field: String`, `uartclk_field: String`, and
  `uartclk_optional: bool`; all required.
- `LinuxPropertyRealization` carries
  `serial_role: Option<SerialPortRole>`; populated as `Some(_)`
  only when the driver has serial-core realization.
- `SerialPortRole` enum defined with exactly two variants:
  `FifoSize` and `ClockRateDefault`.
- `LinuxPlanError::MissingSerialPortPrimaryMmio`,
  `MissingSerialPortPrimaryIrq`, and `MissingSerialPortUartClk`
  defined and raised when a serial driver's probe sequence lacks
  the respective step.
- `primary_mmio_field` / `primary_irq_field` helper functions in
  `emit_module.rs` deleted; no residual call sites.
- `emit_uartclk_initializers` renamed to
  `emit_uartclk_initializer` (singular) and rewritten as scalar
  (emits exactly ONE `d->port.uartclk = ...` line).
- `emit_module.rs` contains no `"current-speed"` or
  `"fifo-size"` / `"fifo_size"` string literal; both conventions
  live only in `planner.rs::build_property_realization` with a
  TODO doc comment pointing to the authoring-surface follow-up.
- `RegisterUartPort` emission renders `d->port.membase`,
  `d->port.irq`, `d->port.uartclk`, and `d->port.fifosize` via
  typed plan fields; `emit_uartclk_initializer` finds the
  `ClockRateDefault` property via `serial_role`.
- `serial_role` assignment is gated on the driver being a
  serial-core driver; non-serial drivers retain
  `serial_role = None` even for id-matching properties.
- Multi-resource (two-MMIO, two-IRQ, two-clock, custom-field-name
  FIFO) emission test passes and asserts explicit first-resource
  selection and exactly-one uartclk line.
- Missing-MMIO / missing-IRQ / missing-Clk plan-build tests raise
  the new structured errors.
- Typed-role ClockRateDefault emitter test (custom property id
  with forced role) passes and produces the correct
  `{DRIVER_UPPER}_DEFAULT_{PROP_UPPER}` define in the ternary
  fallback.
- Planner-convention limitation test documents current id-match
  behavior.
- Task 05 spec amended: `depends_on` includes
  `regseq-linux-backend-08`; amendment section extended with the
  four RegisterUartPort invariants listed in "Coordination with
  regseq-linux-backend-05" above.
- PL011 emission byte-identical to baseline.
- Codex review APPROVED.
