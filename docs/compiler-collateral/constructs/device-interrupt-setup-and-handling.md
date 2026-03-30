# Device Interrupt Setup And Handling

## Purpose

`Device interrupt setup and handling` defines the interrupt resource
 declaration, attribute-bound handler-method surface, and compiler-managed
 interrupt registration/line-control lifecycle.

This is the canonical compiler-facing bound-handler construct for hardware
 interrupts.

## Accepted Primitive Inventory

### Layer 1

| Primitive | Kind | Stable surface |
|---|---|---|
| `interrupt.id` | property | `#[rseq::interrupt(id = "...")]` |
| `interrupt.irq_resource` | resource | typed field `Interrupt` / `Option<Interrupt>` / `[Interrupt; N]` |
| `interrupt.handler_binding` | handler_binding | `#[rseq::isr]` on a driver method |

### Layer 2

| Primitive | Kind | Stable surface |
|---|---|---|
| `interrupt.trigger_type` | property | hardware binding field |
| `interrupt.trigger_polarity` | property | hardware binding field |
| `interrupt.priority` | property | hardware binding field |
| `interrupt.shared` | property | hardware binding field |

### Compiler-managed

| Primitive | Role |
|---|---|
| `interrupt.enable` | enable line in init/resume-generated phases |
| `interrupt.disable` | disable line in suspend/deinit-generated phases |
| `interrupt.register_handler` | register/deregister OS interrupt handler glue |
| `interrupt.allocate_vectors` | allocate vectors for MSI/MSI-X style targets when exercised |

## Accepted Surface Baseline

### Bound-handler baseline

Stable binding forms:

```rust
impl Driver {
    #[rseq::isr]
    fn handle_irq(&mut self) -> IrqResult { /* ... */ }
}
```

```rust
impl Driver {
    #[rseq::isr(rx_irq)]
    fn handle_rx(&mut self) -> IrqResult { /* ... */ }
}
```

```rust
impl Driver {
    #[rseq::isr(channel_irqs)]
    fn handle_channel(&mut self, index: usize) -> IrqResult { /* ... */ }
}
```

Accepted bound-handler rules:

- callback entry context: `H` (explicit handler-managed entry)
- bare `#[rseq::isr]` auto-binds when exactly one interrupt resource exists
- explicit field binding is required for multi-resource cases
- array binding uses `index: usize`
- wrappers recover context and invoke the handler, but do not auto-acquire
  locks

### Return contract

Stable ISR return type:

```rust
enum IrqResult {
    Handled,
    NotHandled,
    DeferWork,
}
```

This is a dispatch result, not `RegseqError`.

### Excluded baseline

- `Interrupt` itself has no driver-callable wrapper methods
- line-level `enable` / `disable` stay compiler-managed

## Extraction Baseline

The frontend must recognize:

- `#[rseq::interrupt(id = "...")]` on `Interrupt`-typed fields
- optional `Option<Interrupt>` and array `[Interrupt; N]` forms
- `#[rseq::isr]` method attributes in bare, explicit-field, and array-bind
  forms
- Layer 2 interrupt binding properties (`trigger_type`, `trigger_polarity`,
  `priority`, `shared`)

Primary specification signals:

- field attribute for interrupt id
- typed interrupt field declarations
- method attributes for handler binding
- Layer 2 binding data for line characteristics

Required extraction validations:

- bare `#[rseq::isr]` only when the interrupt resource is unique
- one handler per non-optional interrupt resource
- array handlers require `index: usize`
- shared interrupts must support a `NotHandled` path

## Lowering Baseline

Lowering ownership:

- handler binding: backend realizes OS-specific wrapper and return mapping
- line control: compiler-generated
- registration/deregistration: compiler-generated
- vector allocation: compiler-generated and conditional on vectorized targets

### Generated lifecycle phase table

| Primitive | Init preamble | Deinit postamble | Suspend postamble | Resume preamble |
|---|---|---|---|---|
| `register_handler` | yes | yes | no | no |
| `enable` | yes | no | no | yes |
| `disable` | no | yes | yes | no |
| `allocate_vectors` | conditional | no | no | no |

Accepted lowering notes:

- interrupt registration is ordered after the resources and state it depends on
- PM-generated interrupt transitions are class-G PM behavior:
  - suspend postamble `S-1`: disable lines
  - resume preamble `R-3`: enable lines
- per-source interrupt masking inside device registers belongs to Register
  Access, not to this construct

## Cross-Construct Dependencies

Depends on:

- `Synchronization` for lock legality and phase-based ISR structuring
- `Register Access` for device-local mask/status/ack operations
- `Device power state management` for PM-phase line gating

Feeds:

- `Deferred work item scheduling` through `IrqResult::DeferWork`
- `Serial/UART Management` and other device-class callbacks via interrupt-
  driven event delivery

## Constraints and Caveats

- Keep Interrupt on the bound-handler surface, not on the trait callback
  surface.
- Keep `enable` / `disable` compiler-managed.
- Wrapper context recovery and lock acquisition remain separate concerns.
- Shared-interrupt ownership checks must remain present where `shared = true`.
- Vector allocation is part of the stable model but is not exercised by PL011;
  it should stay conditional in compiler implementation.

## Representative Examples

```rust
struct Pl011 {
    #[rseq::interrupt(id = "combined_irq")]
    irq: Interrupt,
}

impl Pl011 {
    #[rseq::isr]
    fn handle_irq(&mut self) -> IrqResult {
        IrqResult::Handled
    }
}
```

## Implementation Checklist

- Recognize interrupt resource fields and `#[rseq::interrupt(id = ...)]`.
- Recognize all accepted `#[rseq::isr]` binding forms.
- Generate OS-specific wrapper glue and `IrqResult` mapping.
- Enforce accepted bound-handler auto-bind and array-signature rules.
- Emit `register_handler`, `enable`, and `disable` at the accepted lifecycle
  phases.
- Keep `allocate_vectors` conditional and target-specific.
- Keep per-source device-register masking/acknowledgment in Register Access
  lowering rather than Interrupt lowering.
