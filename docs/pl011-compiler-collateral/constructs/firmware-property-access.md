# Firmware Property Access

## Purpose

`Firmware Property Access` defines the declarative property surface used to
 populate driver fields from DT/ACPI/registry-style firmware descriptions and
 platform-file fallbacks during init preamble generation.

This is a purely declarative/compiler-managed construct. It has no trait,
 handler, wrapper, or resource surface.

## Accepted Primitive Inventory

### Layer 1

| Primitive | Kind | Stable surface |
|---|---|---|
| `firmware_property.declaration` | property | `#[rseq::property(id = "...")]` on a driver field |
| `firmware_property.default_value` | property | `default = ...` in `#[rseq::property(...)]` |
| `firmware_property.vendor_prefix` | property | `vendor_prefix = "..."` in `#[rseq::property(...)]` |
| `firmware_property.required` | property | field type `T` vs `Option<T>` |

### Layer 2

| Input | Role |
|---|---|
| platform file `[properties]` values | compiled-in fallback source keyed by property id |

### Compiler-managed

| Primitive | Role |
|---|---|
| `firmware_property.read_property` | generated init-preamble property extraction |

## Accepted Surface Baseline

Stable declaration pattern:

```rust
struct Pl011 {
    #[rseq::property(id = "reg-io-width", default = 4)]
    reg_io_width: u32,

    #[rseq::property(id = "current-speed")]
    current_speed: Option<u32>,
}
```

Accepted baseline:

- `declaration`, `default_value`, and `vendor_prefix` are field-attribute
  properties
- `required` is type-driven (`T` vs `Option<T>`)
- `read_property` is compiler-managed only

## Extraction Baseline

The frontend must recognize:

- `#[rseq::property(...)]` field attributes
- field types for required vs optional semantics
- Layer 2 `[properties]` entries keyed by property id

Primary specification signals:

- field attributes for property id/default/vendor prefix
- field type for required/optional semantics
- Layer 2 binding data as part of the extraction contract for fallback joining

Required extraction validations:

- reject required properties with no compiled-in fallback path
- reject duplicate property ids
- reject platform/type mismatches
- reject invalid platform-file enum variants for declared property enums
- warn on unknown platform-file keys with no matching Layer 1 declaration
- reject integer values that overflow the declared Rust integer type
- warn when a required field (`T`) also declares a `default`
- keep the explicit-id Layer 1 / Layer 2 join model

Normative validation contract:

- `PROP-001`: required property with no compiled-in fallback is an error
- `PROP-002`: platform-file type mismatch is an error
- `PROP-003`: invalid platform-file enum variant is an error
- `PROP-004`: unknown platform-file key is a warning
- `PROP-005`: integer out of range for the declared Rust integer type is an
  error
- `PROP-006`: duplicate property ids are an error
- `PROP-007`: required field plus `default` is a warning/lint

## Lowering Baseline

Lowering ownership:

- `read_property` is compiler-generated in the init preamble
- Layer 1 / Layer 2 joins remain compiler-internal
- consumer constructs read populated fields as ordinary Rust struct fields
- when `vendor_prefix` is present, the compiler prepends it to the property id
  using the target OS naming rule when generating the firmware-read call
  (comma-prefixed DT naming, target-specific normalization, etc.)

### Generated lifecycle phase table

| Primitive | Init preamble | Deinit postamble | Suspend postamble | Resume preamble |
|---|---|---|---|---|
| `read_property` | yes | no | no | no |

### Stable resolution model

Firmware-derived values follow the accepted three-source resolution order:

1. OS firmware source at runtime
2. platform-file compiled-in fallback
3. `default = ...` attribute fallback

The generated init preamble populates the field on `self` before the driver's
 `Lifecycle::init` body runs.

Stable runtime invariant:

- `PROP-008`: every required property has a compiled-in fallback path in the
  generated init preamble
- required-property absence is therefore eliminated by design before the
  driver's `Lifecycle::init` body runs
- runtime firmware-read failure is a defense-in-depth fallback path, not a
  normal operating mode

## Cross-Construct Dependencies

Feeds:

- `Register Access` (e.g. `reg-io-width`)
- `Serial/UART Management` (e.g. `current-speed`, flow-control-related values)
- `Kernel timer lifecycle management` (polling-related properties)
- `Driver Lifecycle` init ordering, because populated fields appear on `self`

Related note:

- these are runtime data-flow edges, not property-ownership edges

## Constraints and Caveats

- Preserve the accepted explicit-id Layer 1 / Layer 2 join pattern.
- Required-property absence is eliminated by design through extraction-time
  rejection plus compiled-in fallback generation.
- Runtime failure paths here are defense-in-depth fallback paths, not normal
  operating conditions.
- This construct must remain declarative/compiler-managed; do not invent a
  driver-invoked property-read API.

## Representative Examples

```rust
struct Pl011 {
    #[rseq::property(id = "reg-io-width", default = 4)]
    reg_io_width: u32,

    #[rseq::property(id = "arm,primecell-periphid")]
    periph_id: Option<u32>,
}
```

## Implementation Checklist

- Recognize `#[rseq::property(...)]` declarations and their field types.
- Join property ids against Layer 2 `[properties]` data.
- Generate init-preamble property extraction with the accepted three-source
  resolution model.
- Implement the full accepted validation contract (`PROP-001` through
  `PROP-008`) directly in the compiler package.
- Enforce required/optional semantics from field type.
- Surface populated values to later driver code as ordinary struct fields.
- Keep consumer data-flow documentation separate from ownership modeling.
