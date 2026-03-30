# Peripheral Clock Management

## Purpose

`Peripheral clock management` defines the typed clock resource surface, the
 Layer 1 / Layer 2 join for clock identity and fallback frequency, and the
 compiler-managed acquisition lifecycle for peripheral clocks.

This is a typed-resource construct. It has no callback or handler-binding
 surface.

## Accepted Primitive Inventory

### Layer 1

| Primitive | Kind | Stable surface |
|---|---|---|
| `clock_management.clock_id` | property | `#[rseq::clock(id = "...")]` |
| `clock_management.required` | property | `Clock` vs `Option<Clock>` field type |
| `clock_management.enable` | operation | `Clock::enable(&self) -> Result<(), RegseqError>` |
| `clock_management.disable` | operation | `Clock::disable(&self)` |
| `clock_management.get_rate` | operation | `Clock::get_rate(&self) -> u32` |

### Layer 2

| Primitive | Kind | Stable surface |
|---|---|---|
| `clock_management.default_frequency` | property | hardware binding field |

### Compiler-managed

| Primitive | Role |
|---|---|
| `clock_management.acquire_clock` | acquire/release clock resource around device lifetime |

## Accepted Surface Baseline

### Wrapper baseline

Stable typed resource:

```rust
struct MyDriver {
    #[rseq::clock(id = "uart_clk")]
    clock: Clock,
}
```

Accepted wrapper signatures:

```rust
impl Clock {
    fn enable(&self) -> Result<(), RegseqError>;
    fn disable(&self);
    fn get_rate(&self) -> u32;
}
```

Accepted wrapper conventions:

- `&self` is the receiver convention
- no trait capture is justified
- `Clock` vs `Option<Clock>` encodes the `required` property

### Property baseline

- `clock_id`: Layer 1 field attribute join key
- `required`: Layer 1 type-driven property
- `default_frequency`: Layer 2 hardware binding fallback

## Extraction Baseline

The frontend must recognize:

- `#[rseq::clock(id = "...")]` on `Clock` or `Option<Clock>` fields
- field type `Clock` vs `Option<Clock>`
- wrapper-method calls `enable()`, `disable()`, `get_rate()`
- Layer 2 clock binding data keyed by `clock_id`

Primary specification signals:

- field attribute for `clock_id`
- field type for `required`
- Layer 2 binding data for `default_frequency`

Required extraction validations:

- every clock field must have a `clock_id`
- every clock resource must have a `default_frequency` in Layer 2
- `Clock` means acquisition failure is a hard init error
- `Option<Clock>` means acquisition failure yields `None`

## Lowering Baseline

Lowering ownership:

- wrapper methods: backend lowers to target clock-control APIs
- `acquire_clock`: compiler-generated from the field declaration and Layer 2
  clock binding
- fallback rate selection: compiler-managed using runtime-query or compiled-in
  fallback

### Generated lifecycle phase table

| Primitive | Init preamble | Deinit postamble | Suspend postamble | Resume preamble |
|---|---|---|---|---|
| `acquire_clock` | yes | yes | no | no |

Notes:

- PM-phase clock enable/disable belongs to `Device power state management`, not
  to this construct's own generated lifecycle primitive
- accepted resource-ordering clarification remains:
  - MMIO map happens before clock enable in init
  - clock enable happens before the first meaningful register access

## Cross-Construct Dependencies

Depends on:

- `Bus Resource Management` / Layer 2 resource binding

Feeds:

- `Register Access` and `Serial/UART Management` through `get_rate()` and
  clocked register access
- `Device power state management` for PM-generated clock gating / power-domain
  ordering

## Constraints and Caveats

- Keep this construct non-trait-based.
- Preserve the accepted wrapper signatures:
  - `enable` fallible
  - `disable` infallible in the stable surface
  - `get_rate` infallibly returns `u32`
- SoC power-before-clock ordering is a shared compiler constraint and must be
  respected where PM lowering emits clock-related transitions.

## Representative Examples

```rust
struct Pl011 {
    #[rseq::clock(id = "uart_clk")]
    clock: Clock,
}

fn configure_uart(&self) -> Result<(), RegseqError> {
    let rate = self.clock.get_rate();
    // derive baud divisors from rate
    Ok(())
}
```

## Implementation Checklist

- Recognize `#[rseq::clock(id = ...)]` and `Clock` / `Option<Clock>` fields.
- Join `clock_id` with Layer 2 clock binding data.
- Generate `acquire_clock` in init/deinit lifecycle code.
- Lower `enable`, `disable`, and `get_rate` through the target clock API.
- Implement the runtime-query or `default_frequency` fallback path for
  `get_rate`.
- Keep PM-phase clock transitions in the PM lowering layer, not in this
  construct's wrapper surface.
