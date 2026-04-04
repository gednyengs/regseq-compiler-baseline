# Bus Resource Management

## Purpose

`Bus Resource Management` defines the explicit portable attachment/fabric
identity declaration, bus-family-specific binding metadata, optional PCI BAR
declaration, PCI-conditional config-space intrinsics, and the compiler-managed
bus-resource acquisition steps that other constructs depend on.

This is a declarative and compiler-managed construct. It has no trait or Track
B surface.

## Accepted Primitive Inventory

### Layer 1

| Primitive | Kind | Stable surface |
|---|---|---|
| `bus_resource.bus_type` | property | `#[rseq::driver(bus = "...")]` |
| `bus_resource.bar_index` | property | `#[rseq::mmio(bar = N)]` |
| `bus_resource.read_config` | operation | bus-config intrinsic |
| `bus_resource.write_config` | operation | bus-config intrinsic |

### Layer 2

| Input | Role |
|---|---|
| `bus.type` | deployment-side confirmation/refinement of the portable bus family |
| bus-family-specific metadata | deployment-side identity/binding facts such as AMBA or PCI details |

### Compiler-managed

| Primitive | Role |
|---|---|
| `bus_resource.enable_device` | enable device on config-space-bearing buses |
| `bus_resource.claim_regions` | claim declared bus-backed resource regions |
| `bus_resource.set_bus_master` | enable bus mastering for DMA-capable devices |
| `bus_resource.get_resource_handle` | populate compiler-internal bus handle context |

## Accepted Surface Baseline

### Declarative baseline

Stable bus declaration:

```rust
#[rseq::driver(bus = "pci")]
struct MyDriver { /* ... */ }
```

Portable bus taxonomy:

- `integrated`: firmware-described integrated/on-chip device with no stronger
  bus identity
- `amba`: ARM AMBA / PrimeCell-family device
- `pci`: PCI / PCIe enumerated device
- future families may include `i2c`, `spi`, `usb`, and `virtio`

Accepted taxonomy rules:

- `platform` is not the preferred portable taxonomy term
- Linux `platform_driver` is an OS-specific realization of portable
  `integrated`
- `amba` and `pci` remain portable bus/fabric identities rather than Linux-only
  registration choices

Stable PCI BAR declaration:

```rust
struct MyDriver {
    #[rseq::mmio(id = "regs", bar = 0)]
    regs: Mmio<MyRegs>,
}
```

Accepted decisions:

- `bus_type` is an explicit Layer 1 property, not inferred ambient context
- `bar_index` is a field-level property consumed by `Register Access`
- the compiler-internal bus handle is not a separate driver-surface primitive
- `bus_type` is a portable attachment/fabric identity, not an OS-native driver
  registration label

### Layer 2 baseline

Layer 2 should provide a dedicated structured bus section rather than forcing
bus identity details into generic firmware properties.

Representative examples:

```toml
[bus]
type = "integrated"
firmware = "devicetree"
```

```toml
[bus]
type = "amba"
amba_id = 0x00041011
revision = 3
```

```toml
[bus]
type = "pci"
vendor_id = 0x8086
device_id = 0x100e
```

Accepted Layer 2 decisions:

- Layer 1 declares the authored portable bus family
- Layer 2 supplies deployment-side bus metadata for that family
- bus identity details should not be modeled as ordinary `[properties]` values
  unless they are genuinely driver-visible runtime properties
- bus metadata participates in driver-level lowering decisions rather than
  per-resource joins

### Intrinsic baseline

Stable compiler baseline preserves dedicated config-space intrinsics for
config-space-bearing buses:

```rust
let value = regseq::bus::read_config::<u32>(offset)?;
regseq::bus::write_config::<u32>(offset, value)?;
```

These are conditional on `bus_type` selecting a config-space-bearing bus such
as PCI.

## Extraction Baseline

The frontend must recognize:

- `#[rseq::driver(bus = "...")]` on the driver item
- `#[rseq::mmio(bar = N)]` on BAR-backed MMIO fields
- config-space intrinsic call sites
- declarations in other constructs that imply bus-handle consumers

Primary specification signals:

- item attribute for `bus_type`
- field attribute for `bar_index`
- intrinsic call sites for config-space access

Required extraction validations:

- `bar_index` is only meaningful when `bus_type` selects a PCI-like bus
- config-space intrinsics are only valid on config-space-bearing buses
- `set_bus_master` is only emitted when DMA-capable declarations are present
- `get_resource_handle` remains available even when the driver does not use
  PCI-only operations directly
- reject unknown `bus_type` values
- reject Layer 1 / Layer 2 bus-family conflicts when a dedicated Layer 2 bus
  section is present
- validate family-specific Layer 2 keys against the selected `bus_type`

## Lowering Baseline

Lowering ownership:

- config-space intrinsics: backend lowers to target bus APIs
- bus enable/claim/mastering/bus-handle setup: compiler-generated
- bus-type-to-OS-registration mapping: backend-owned realization driven by the
  portable `bus_type` plus Layer 2 deployment facts

### Generated lifecycle phase table

| Primitive | Init preamble | Deinit postamble | Suspend postamble | Resume preamble |
|---|---|---|---|---|
| `enable_device` | conditional | no | no | no |
| `claim_regions` | conditional | no | no | no |
| `set_bus_master` | conditional | no | no | no |
| `get_resource_handle` | yes | no | no | no |

Accepted lowering notes:

- PL011 exercises the universal declarative/compiler-managed subset even though
  it has zero direct concern captures for the bus-management concerns
- PCI-specific generated steps remain conditional on `bus_type`
- `get_resource_handle` is the accepted home for compiler-internal bus handle
  population
- Linux realization examples:
  - `integrated` -> `platform_driver`
  - `amba` -> `amba_driver`
  - `pci` -> `pci_driver`
- other OSes may realize the same portable `bus_type` through different native
  registration mechanisms

## Cross-Construct Dependencies

Feeds:

- `Register Access`
- `Peripheral clock management`
- `Device interrupt setup and handling`
- `DMA Management`
- `Hardware reset sequencing and post-reset polling`
- `Diagnostic Logging`

Depends on:

- driver item declaration for `bus_type`
- BAR-backed MMIO declarations where present

## Constraints and Caveats

- Keep `bus_type` explicit in Layer 1.
- Prefer portable taxonomy terms such as `integrated` over Linux-native labels
  such as `platform`.
- Do not reintroduce `bus_device_handle` as a separate driver-surface
  primitive.
- Keep PCI-only operations structurally conditioned by `bus_type`.
- Preserve `get_resource_handle` as the source of compiler-internal bus context.
- Keep bus-family deployment facts in a dedicated Layer 2 bus section rather
  than overloading generic firmware properties.

## Representative Examples

```rust
#[rseq::driver(bus = "pci")]
struct E1000 {
    #[rseq::mmio(id = "regs", bar = 0)]
    regs: Mmio<E1000Regs>,
}
```

```rust
#[rseq::driver(name = "pl011", compatible = ["arm,pl011"], bus = "amba")]
struct Pl011 {
    #[rseq::mmio(id = "uart_regs")]
    regs: Mmio<Pl011Registers>,
}
```

## Implementation Checklist

- Recognize `bus_type` from `#[rseq::driver(...)]`.
- Implement the accepted portable bus taxonomy, starting with:
  - `integrated`
  - `amba`
  - `pci`
- Recognize `bar_index` on BAR-backed MMIO fields.
- Add normalized Layer 2 bus-section support for family-specific deployment
  metadata.
- Lower config-space intrinsics only on valid bus families.
- Emit `enable_device`, `claim_regions`, `set_bus_master`, and
  `get_resource_handle` at the accepted points.
- Map portable `bus_type` into OS-native driver-registration mechanisms during
  backend realization rather than baking Linux terms into the portable model.
- Preserve compiler-internal bus handle context for downstream resource
  lowering.
