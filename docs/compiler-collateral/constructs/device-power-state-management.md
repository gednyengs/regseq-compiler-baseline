# Device Power State Management

## Purpose

`Device power state management` defines the optional per-driver suspend/resume
 contract and the generated PM ordering around clocks, pin state, interrupts,
 and shadow-state restoration. In compiler terms, this construct is the PM gate
 and the owner of PM-phase generated lifecycle behavior.

## Accepted Primitive Inventory

### Layer 1

| Primitive | Kind | Stable surface |
|---|---|---|
| `power_management.idle` | property | `#[rseq::pm(idle = ...)]` |
| `power_management.wakeup_capable` | property | `#[rseq::pm(wakeup_capable = ...)]` |
| `power_management.suspend` | operation | `PowerManagement::suspend(&mut self) -> Result<(), RegseqError>` |
| `power_management.resume` | operation | `PowerManagement::resume(&mut self) -> Result<(), RegseqError>` |

### Layer 2

This construct has no direct Layer 2 declarations. It consumes state supplied
 by dependent constructs during PM lowering.

### Compiler-managed

| Primitive | Role |
|---|---|
| `power_management.pm_registration` | emit OS PM registration/wiring |
| `power_management.clock_gating` | emit generated PM resource ordering for class-G resources |

## Accepted Surface Baseline

### PM gate

`#[rseq::pm(...)]` is the single stable gate for PM. If the PM attribute is
 absent, the compiler must not require `PowerManagement`.

### Trait baseline

Recommended trait:

```rust
trait PowerManagement {
    fn suspend(&mut self) -> Result<(), RegseqError>;
    fn resume(&mut self) -> Result<(), RegseqError>;
}
```

- Callback entry context: `F` (framework-managed entry)
- Trait grouping: `per_construct`
- Compiler work should key on the accepted trait family name
  `PowerManagement`.
- Any future naming polish is a compatibility follow-up, not a blocker for
  current compiler tasking.

### Property baseline

- `idle`: item attribute argument on `#[rseq::pm(...)]`
- `wakeup_capable`: item attribute argument on `#[rseq::pm(...)]`

## Extraction Baseline

The frontend must recognize:

- `#[rseq::pm(idle = ..., wakeup_capable = ...)]` on the driver type
- `impl PowerManagement for DriverType { ... }`

Primary specification signals:

- item attributes for `idle` and `wakeup_capable`
- trait impl surface for `suspend` and `resume`

Required extraction validations:

- if `#[rseq::pm(...)]` is present, the driver must implement
  `PowerManagement`
- if PM is absent, `PowerManagement` must not be required

## Lowering Baseline

Lowering ownership:

- trait callbacks: backend realizes callback contracts
- PM registration: compiler-generated
- PM resource ordering: compiler-generated for class-G resources, compiler-
  validated for driver-authored interaction points

### Generated lifecycle phase table

| Primitive | Init preamble | Deinit postamble | Suspend postamble | Resume preamble |
|---|---|---|---|---|
| `pm_registration` | no | no | no | no |
| `clock_gating` | no | no | yes | yes |

This table shows only the PM construct's own primitives. The full generated PM
sequence also includes dependent-construct primitives from `Interrupt`,
`PinCtrl`, and `Register Access`.

Accepted PM ordering baseline:

- suspend: driver body first, then generated PM postamble
- resume: generated PM preamble first, then driver body

Accepted generated PM sequence:

- suspend postamble: `Interrupt -> PinCtrl -> Clock`
- resume preamble: `Clock -> PinCtrl -> Interrupt -> shadow flush`

Accepted PM ownership classes:

- compiler-generated: `Clock`, `PinCtrl`, `Interrupt`
- compiler-validated / driver-authored: `Timer`, `DMA`
- not PM-relevant: all other constructs unless explicitly noted

## Cross-Construct Dependencies

Depends on:

- `Peripheral clock management`
- `Pin controller and GPIO configuration`
- `Device interrupt setup and handling`
- `Register Access` shadow restoration behavior

Constrains:

- `Kernel timer lifecycle management`
- `DMA Management`

Notes:

- timer stop/restart remains driver-authored and compiler-validated
- DMA quiesce/restart remains driver-authored and compiler-validated

## Constraints and Caveats

- Preserve the accepted `Lifecycle` / `PowerManagement` split.
- `clock_gating` names a PM-generated resource-ordering role, not just literal
  clock enable/disable calls; SoC power-before-clock ordering is a carry-
  forward compiler constraint.
- PM ordering must remain aligned with the accepted class-G / class-V model.
- `shadow flush` in resume preamble depends on Register Access metadata.

## Representative Examples

```rust
#[rseq::pm(idle = false, wakeup_capable = false)]
struct Pl011 {
    // resource fields
}

impl PowerManagement for Pl011 {
    fn suspend(&mut self) -> Result<(), RegseqError> { /* save/quiet device */ }
    fn resume(&mut self) -> Result<(), RegseqError> { /* restore driver state */ }
}
```

## Implementation Checklist

- Recognize `#[rseq::pm(...)]` as the PM gate.
- Enforce optionality: PM trait required only when PM is declared.
- Emit PM registration glue per backend.
- Implement generated PM ordering exactly as accepted.
- Keep timer and DMA in the compiler-validated / driver-authored bucket.
- Integrate SoC power-domain ordering where clock gating is lowered.
