# Hardware Reset Sequencing And Post-Reset Polling

## Purpose

`Hardware reset sequencing and post-reset polling` defines the typed reset-line
resource surface, the Layer 1 / Layer 2 join for reset identity and sharing,
and the compiler-managed reset acquisition lifecycle.

This is a typed-resource construct. It does not own register polling after
reset. Post-reset readiness polling remains part of `Register Access`.

## Accepted Primitive Inventory

### Layer 1

| Primitive | Kind | Stable surface |
|---|---|---|
| `hardware_reset.reset_required` | property | `ResetLine` vs `Option<ResetLine>` field type |
| `hardware_reset.reset_line` | resource | typed field `ResetLine` / `Option<ResetLine>` |
| `hardware_reset.assert` | operation | `ResetLine::assert(&self) -> Result<(), RegseqError>` |
| `hardware_reset.deassert` | operation | `ResetLine::deassert(&self) -> Result<(), RegseqError>` |
| `hardware_reset.toggle` | operation | `ResetLine::toggle(&self, delay: Duration) -> Result<(), RegseqError>` |

### Layer 2

| Primitive | Kind | Stable surface |
|---|---|---|
| `hardware_reset.reset_shared` | property | hardware binding field |
| `hardware_reset.reset_line_id` | property | hardware binding field |

### Compiler-managed

| Primitive | Role |
|---|---|
| `hardware_reset.acquire_reset` | acquire/deassert in init, assert/release in deinit |

## Accepted Surface Baseline

### Wrapper baseline

Stable typed resource:

```rust
struct MyDriver {
    reset: Option<ResetLine>,
}
```

Accepted wrapper signatures:

```rust
impl ResetLine {
    fn assert(&self) -> Result<(), RegseqError>;
    fn deassert(&self) -> Result<(), RegseqError>;
    fn toggle(&self, delay: Duration) -> Result<(), RegseqError>;
}
```

Accepted wrapper conventions:

- `&self` is the receiver convention
- `ResetLine` vs `Option<ResetLine>` encodes `reset_required`
- field-name join is the current default Layer 1 to Layer 2 mechanism
- reset operations remain wrapper methods, not traits or intrinsics

### Property baseline

- `reset_required`: Layer 1 type-driven optionality
- `reset_shared`: Layer 2 ownership/sharing fact
- `reset_line_id`: Layer 2 target reset-line identity

### Excluded baseline

- post-reset polling does not belong to this construct
- software reset via ordinary register writes remains `Register Access`

## Extraction Baseline

The frontend must recognize:

- `ResetLine` / `Option<ResetLine>` typed fields
- field-name join into the Layer 2 reset resource section
- wrapper-method calls `assert()`, `deassert()`, and `toggle(Duration)`
- Layer 2 reset binding data:
  - `reset_shared`
  - `reset_line_id`

Primary specification signals:

- field type for `reset_required`
- typed reset field declaration for the resource
- wrapper-method call sites for reset operations
- Layer 2 binding data for identity and sharing

Required extraction validations:

- `ResetLine` means acquisition failure is a hard init error
- `Option<ResetLine>` means missing reset is allowed
- every reset field must join to exactly one Layer 2 reset resource entry
- `toggle` delay must use accepted `regseq::Duration`

## Lowering Baseline

Lowering ownership:

- wrapper methods: backend lowers to target reset-controller APIs
- `acquire_reset`: compiler-generated from the field declaration and Layer 2
  binding data
- post-reset register polling: lowered by `Register Access`, not by this
  construct

### Generated lifecycle phase table

| Primitive | Init preamble | Deinit postamble | Suspend postamble | Resume preamble |
|---|---|---|---|---|
| `acquire_reset` | yes | yes | no | no |

Accepted lowering notes:

- generated lifecycle ownership belongs only to `acquire_reset`
- wrapper methods themselves do not carry generated lifecycle participation
- accepted init ordering remains:
  - MMIO map -> clock enable -> reset deassert -> pinctrl apply

## Cross-Construct Dependencies

Depends on:

- `Timed CPU delay and sleep` for `toggle(Duration)`
- `Bus Resource Management` / Layer 2 binding join

Feeds:

- `Register Access` for post-reset readiness polling
- `Device power state management` only indirectly through broader lifecycle
  ordering; reset is not PM-owned

## Constraints and Caveats

- Keep generated lifecycle ownership on `acquire_reset`, not on wrapper
  methods.
- Preserve field-name join as the default reset-resource join mechanism.
- Keep post-reset polling in `Register Access`.
- Preserve the fallible `RegseqError`-based wrapper surface.

## Representative Examples

```rust
struct Pl011 {
    reset: Option<ResetLine>,
}

fn hw_reset(&self) -> Result<(), RegseqError> {
    if let Some(reset) = &self.reset {
        reset.toggle(regseq::Duration::from_us(10))?;
    }
    Ok(())
}
```

## Implementation Checklist

- Recognize `ResetLine` / `Option<ResetLine>` fields.
- Join reset fields to Layer 2 `reset_line_id` / `reset_shared` data.
- Generate `acquire_reset` in init/deinit lifecycle code.
- Lower `assert`, `deassert`, and `toggle` through target reset APIs.
- Keep any post-reset register polling in Register Access lowering.
