# Parameter Registration

## Purpose

`Parameter Registration` defines the declarative parameter surface used to
expose driver-tunable values at load time and, when permitted, through
runtime-diagnostic parameter interfaces.

This is a purely declarative/compiler-managed construct. It has no trait,
handler-binding, intrinsic, or typed-resource surface.

## Accepted Primitive Inventory

### Layer 1

| Primitive | Kind | Stable surface |
|---|---|---|
| `param_reg.declaration` | property | `#[rseq::param(id = "...", ...)]` |
| `param_reg.default_value` | property | `default = ...` attribute argument |
| `param_reg.range_min` | property | `min = ...` attribute argument |
| `param_reg.range_max` | property | `max = ...` attribute argument |
| `param_reg.writable` | property | `writable = true | false` attribute argument |
| `param_reg.description` | property | `description = "..."` attribute argument |
| `param_reg.param_type` | property | Rust field type |

### Layer 2

This construct has no Layer 2 declarations.

### Compiler-managed

| Primitive | Role |
|---|---|
| `param_reg.declare_parameter` | generate load-time and optional runtime parameter registration |

## Accepted Surface Baseline

Stable declarative form:

```rust
#[rseq::param(
    id = "tx_ring_size",
    default = 256,
    min = 64,
    max = 4096,
    writable = false,
    description = "TX descriptor count"
)]
tx_ring_size: u32,
```

Accepted decisions:

- the driver declares parameters only through `#[rseq::param(...)]`
- `writable` is the key discriminator:
  - `false`: load-time-only parameter
  - `true`: runtime-diagnostic registration in addition to load-time support
- `param_type` is carried by the Rust field type

## Extraction Baseline

The frontend must recognize:

- `#[rseq::param(...)]` field attributes
- parameter field types
- default/range/writable/description arguments

Primary specification signals:

- field attribute arguments for all declaration properties except type
- field type for `param_type`

Required extraction validations:

- every parameter declaration has an `id`
- every parameter declaration has a default value
- range bounds, when present, must be type-correct
- `writable` defaults to `false`

## Lowering Baseline

Lowering ownership:

- the entire parameter interface is compiler-generated
- backend lowering selects module/sysctl/registry/Kconfig realization based on
  target OS and `writable`

### Generated lifecycle phase table

| Primitive | Init preamble | Deinit postamble | Suspend postamble | Resume preamble |
|---|---|---|---|---|
| `declare_parameter` | yes | no | no | no |

Accepted lowering notes:

- one semantic primitive covers both load-time declaration and optional
  runtime-diagnostic registration
- parameter validation code is compiler-generated from declared type/range
- PL011 does not exercise this construct; E1000 does

## Cross-Construct Dependencies

Depends on:

- none beyond the driver's own field declarations

Feeds:

- device-class or subsystem code that reads parameter-backed fields at init or
  runtime

## Constraints and Caveats

- Keep this construct declarative/compiler-managed only.
- Preserve `writable` as the key load-time vs runtime-diagnostic discriminator.
- Do not introduce a driver-invoked parameter API surface.

## Representative Examples

```rust
struct E1000 {
    #[rseq::param(id = "debug_level", default = 0, writable = true)]
    debug_level: u32,
}
```

## Implementation Checklist

- Recognize `#[rseq::param(...)]` on driver fields.
- Extract type/default/range/writable/description consistently.
- Generate load-time declaration for every parameter.
- Generate runtime-diagnostic registration only when `writable = true`.
- Emit type/range validation from the declared parameter metadata.
