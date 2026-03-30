# Pin Controller And GPIO Configuration

## Purpose

`Pin controller and GPIO configuration` defines the typed pinctrl resource
surface, the Layer 1 / Layer 2 join for available pin states, and the
compiler-managed init-phase pinctrl acquisition flow.

This is a typed-resource construct. It has no trait or bound-handler surface.

## Accepted Primitive Inventory

### Layer 1

| Primitive | Kind | Stable surface |
|---|---|---|
| `pin_gpio.pinctrl_required` | property | `PinCtrl` vs `Option<PinCtrl>` field type |
| `pin_gpio.pinctrl` | resource | typed field `PinCtrl` / `Option<PinCtrl>` |
| `pin_gpio.apply_state` | operation | `PinCtrl::apply<S: PinState>(&self) -> Result<(), RegseqError>` |

### Layer 2

| Primitive | Kind | Stable surface |
|---|---|---|
| `pin_gpio.pin_states` | property | hardware binding field |

### Compiler-managed

| Primitive | Role |
|---|---|
| `pin_gpio.acquire_pinctrl` | acquire pinctrl handle and apply default state in init |

## Accepted Surface Baseline

### Wrapper baseline

Stable typed resource:

```rust
struct MyDriver {
    pinctrl: Option<PinCtrl>,
}
```

Accepted wrapper signature:

```rust
impl PinCtrl {
    fn apply<S: PinState>(&self) -> Result<(), RegseqError>;
}
```

Accepted wrapper conventions:

- `&self` is the receiver convention
- `PinCtrl` vs `Option<PinCtrl>` encodes `pinctrl_required`
- field-name join is the default Layer 1 to Layer 2 mechanism
- `#[rseq::pinctrl(id = "...")]` remains an accepted optional override
- compile-time state markers are the preferred stable state-selection surface

### Property baseline

- `pinctrl_required`: Layer 1 type-driven optionality
- `pin_states`: Layer 2 list of valid named states (for example `default`,
  `sleep`)

### Excluded baseline

- PM-phase pin-state transitions are not owned by this construct's wrapper
  surface
- GPIO controller APIs outside the pinctrl state-selection path are not part of
  this construct

## Extraction Baseline

The frontend must recognize:

- `PinCtrl` / `Option<PinCtrl>` typed fields
- field-name join into the Layer 2 pinctrl resource section
- optional `#[rseq::pinctrl(id = "...")]` explicit join override
- `apply::<State>()` wrapper-method calls
- Layer 2 `pin_states` lists

Primary specification signals:

- field type for `pinctrl_required`
- typed pinctrl field declaration for the resource
- optional field attribute for explicit join override
- wrapper-method call sites for state transitions
- Layer 2 state inventory for valid state markers

Required extraction validations:

- every pinctrl field must join to exactly one Layer 2 state inventory
- `apply::<S>()` must use a compile-time-valid state marker from that inventory
- `PinCtrl` means acquisition failure is a hard init error
- `Option<PinCtrl>` means missing pinctrl is allowed

## Lowering Baseline

Lowering ownership:

- wrapper method: backend lowers to target pinctrl state-selection APIs
- `acquire_pinctrl`: compiler-generated init-preamble step
- PM-phase sleep/default state transitions: owned by `Device power state
  management`

### Generated lifecycle phase table

| Primitive | Init preamble | Deinit postamble | Suspend postamble | Resume preamble |
|---|---|---|---|---|
| `acquire_pinctrl` | yes | no | no | no |

Accepted lowering notes:

- `acquire_pinctrl` owns only init-phase generated work
- there is no accepted compiler-emitted deinit pinctrl step
- PM owns generated suspend/resume pin-state transitions

## Cross-Construct Dependencies

Depends on:

- `Bus Resource Management` / Layer 2 resource binding join
- `Device power state management` for PM-phase pin-state ownership

Feeds:

- callback/resource users that require pin muxing before register or interrupt
  activity begins

## Constraints and Caveats

- Keep this construct non-trait-based.
- Preserve the preamble-only lifecycle model for `acquire_pinctrl`.
- Keep the exact join-key mechanism in the accepted state:
  - field-name join as default
  - explicit `#[rseq::pinctrl(id = ...)]` as optional override
- Preserve state-selection as a fallible wrapper-method surface.

## Representative Examples

```rust
struct Pl011 {
    pinctrl: Option<PinCtrl>,
}

fn enter_default_pins(&self) -> Result<(), RegseqError> {
    if let Some(pinctrl) = &self.pinctrl {
        pinctrl.apply::<Default>()?;
    }
    Ok(())
}
```

## Implementation Checklist

- Recognize `PinCtrl` / `Option<PinCtrl>` fields.
- Join pinctrl fields to Layer 2 `pin_states`.
- Support both field-name join and explicit `#[rseq::pinctrl(id = ...)]`.
- Generate `acquire_pinctrl` in the init preamble only.
- Lower `apply::<S>()` through target pinctrl APIs.
- Keep PM-generated suspend/resume state transitions in PM lowering.
