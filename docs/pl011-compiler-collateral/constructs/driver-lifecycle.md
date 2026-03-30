# Driver Lifecycle

## Purpose

`Driver Lifecycle` defines the per-driver callback contract and compiler-managed
 registration steps needed to bring a device instance into existence and tear it
 down cleanly. In compiler terms, this construct owns the driver identity
 surface, the base callback trait, and the generated registration/device-
 creation glue.

This construct does not own `suspend` or `resume`. That boundary was closed in
 favor of the separate `Device power state management` construct.

## Accepted Primitive Inventory

### Layer 1

| Primitive | Kind | Stable surface |
|---|---|---|
| `driver_lifecycle.driver_name` | property | `#[rseq::driver(name = "...")]` |
| `driver_lifecycle.compatible` | property | `#[rseq::driver(compatible = [...])]` |
| `driver_lifecycle.device_class` | property | `#[rseq::device_class(...)]` |
| `driver_lifecycle.init` | operation | `Lifecycle::init(&mut self) -> Result<(), RegseqError>` |
| `driver_lifecycle.deinit` | operation | `Lifecycle::deinit(&mut self) -> Result<(), RegseqError>` |

### Layer 2

This construct does not declare Layer 2 properties directly. `compatible`
 participates in Layer 1 / Layer 2 joining during bus-level registration.

### Compiler-managed

| Primitive | Role |
|---|---|
| `driver_lifecycle.register_driver` | emit bus-level driver registration glue |
| `driver_lifecycle.create_device` | construct/populate driver instance for a matched device |
| `driver_lifecycle.associate_private_data` | connect runtime device handle and driver state |

## Accepted Surface Baseline

### Trait baseline

Recommended trait:

```rust
trait Lifecycle {
    fn init(&mut self) -> Result<(), RegseqError>;
    fn deinit(&mut self) -> Result<(), RegseqError>;
}
```

- Callback entry context: `F` (framework-managed entry)
- Trait grouping: `per_construct`
- Compiler work should key on the accepted trait family name `Lifecycle`.
- Any future naming polish is a compatibility follow-up, not a blocker for
  current compiler tasking.

### Property baseline

- `driver_name`: item attribute on the driver type
- `compatible`: item attribute on the driver type
- `device_class`: item attribute on the driver type

Naming normalization already accepted:

- earlier mapping artifacts sometimes used `compatible_ids`
- the canonical compiler-facing primitive name is `compatible`

## Extraction Baseline

The frontend must recognize:

- `#[rseq::driver(name = "...", compatible = [...])]` on the driver type
- `#[rseq::device_class(...)]` on the driver type
- `impl Lifecycle for DriverType { ... }`

Primary specification signals:

- item attributes for `driver_name`, `compatible`, `device_class`
- trait impl surface for `init` and `deinit`

Required extraction validations:

- every driver must declare `driver_name`
- every driver must declare `compatible`
- every driver in the current baseline must declare `device_class`
- every driver must implement `Lifecycle`
- `device_class` must match any device-class trait implementation required by
  other constructs, such as `Serial` or `Network`

## Lowering Baseline

Lowering ownership:

- trait callbacks: backend realizes callback contracts
- properties: compiler emits registration/device-match glue
- compiler-managed primitives: compiler emits lifecycle and registration glue

### Generated lifecycle phase table

| Primitive | Init preamble | Deinit postamble | Suspend postamble | Resume preamble |
|---|---|---|---|---|
| `register_driver` | no | no | no | no |
| `create_device` | yes | no | no | no |
| `associate_private_data` | yes | no | no | no |

Notes:

- `register_driver` is a registration-time compiler action, not a callback-
  phase preamble/postamble step
- `create_device` and `associate_private_data` happen before `Lifecycle::init`
- `deinit` runs before any generated deinit cleanup owned by dependent
  constructs

## Cross-Construct Dependencies

Upstream inputs:

- `Bus Resource Management` consumes `driver_name` and `compatible`
- `Serial/UART Management` depends on `device_class = serial`

Downstream relationships:

- `Device power state management` is separate and optional; it must not be
  folded back into `Lifecycle`
- `Diagnostic Logging` derives device-context identity from `driver_name`

## Constraints and Caveats

- Treat `Lifecycle` and `PowerManagement` as separate contracts. This is an
  accepted divergence from earlier v2.2-style combined lifecycle thinking.
- All trait callbacks use `&mut self`.
- Use `Result<(), RegseqError>` as the stable callback return shape.
- Missing `device_class` is a hard extraction error because it controls
  dependent device-class lowering.

## Representative Examples

Layer 1:

```rust
#[rseq::driver(name = "pl011-uart", compatible = ["arm,pl011", "arm,sbsa-uart"])]
#[rseq::device_class(serial)]
struct Pl011 {
    // fields populated by compiler-managed preamble
}

impl Lifecycle for Pl011 {
    fn init(&mut self) -> Result<(), RegseqError> { /* ... */ }
    fn deinit(&mut self) -> Result<(), RegseqError> { /* ... */ }
}
```

## Implementation Checklist

- Recognize `driver` and `device_class` item attributes on the driver type.
- Normalize `compatible_ids`-style upstream artifacts to `compatible`.
- Enforce required `Lifecycle` implementation.
- Emit registration glue using `driver_name` and `compatible`.
- Create/populate the driver instance before calling `init`.
- Preserve the accepted separation between `Lifecycle` and
  `PowerManagement`.
