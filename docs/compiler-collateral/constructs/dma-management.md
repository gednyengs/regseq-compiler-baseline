# DMA Management

## Purpose

`DMA Management` defines the coherent-memory, streaming-mapping, DMA-channel,
 and compiler-managed translation/lifecycle surfaces used to lower DMA-capable
 drivers.

This is a large mixed-form resource construct. It has no accepted trait or
 bound-handler callback surface in the current stable baseline.

## Accepted Primitive Inventory

### Layer 1

| Primitive | Kind | Stable surface |
|---|---|---|
| `dma.region_id` | property | `#[rseq::dma_region(...)]` field attribute |
| `dma.entry_size` | property | `#[rseq::dma_region(...)]` field attribute |
| `dma.entry_count` | property | `#[rseq::dma_region(...)]` field attribute |
| `dma.channel_id` | property | `#[rseq::dma_channel(...)]` field attribute |
| `dma.channel_direction` | property | `#[rseq::dma_channel(...)]` field attribute |
| `dma.dma_region` | resource | typed field `DmaRegion<T>` |
| `dma.dma_channel` | resource | typed field `DmaChannel` |
| `dma.region_access` | operation | wrapper method |
| `dma.streaming_map` | operation | `regseq::dma::map(...) -> Result<DmaMapping, RegseqError>` |
| `dma.streaming_unmap` | operation | `DmaMapping` Drop / RAII unmap |
| `dma.streaming_sync` | operation | `DmaMapping::sync_for_cpu()` / `sync_for_device()` |
| `dma.channel_configure` | operation | `DmaChannel::configure(&self)` |
| `dma.channel_prepare` | operation | `prepare(&DmaMapping)` / `prepare_sg(&[&DmaMapping])` |
| `dma.channel_submit` | operation | merged stable surface `DmaChannel::submit(&self) -> Result<(), RegseqError>` |
| `dma.channel_issue_pending` | operation | merged into stable `submit()` surface |
| `dma.channel_stop` | operation | `DmaChannel::stop(&self)` |

### Layer 2

| Primitive | Kind | Stable surface |
|---|---|---|
| `dma.dma_mask` | property | hardware binding field |
| `dma.slave_addr_width` | property | hardware binding field |
| `dma.slave_maxburst` | property | hardware binding field |

### Compiler-managed

| Primitive | Role |
|---|---|
| `dma.alloc_coherent` | coherent region allocation/free around device lifetime |
| `dma.request_channel` | channel acquisition/release around device lifetime |
| `dma.set_dma_mask` | configure DMA mask from binding/platform data |
| `dma.sg_construction` | compiler-internal scatter/gather construction |
| `dma.address_translation` | compiler-internal device-address translation |

## Accepted Surface Baseline

### Coherent-region baseline

Stable typed resource:

```rust
struct MyDriver {
    #[rseq::dma_region(id = "rx_ring", entry_size = 32, entry_count = 128)]
    rx_ring: DmaRegion<RxDesc>,
}
```

### Streaming-mapping baseline

Stable streaming handle:

```rust
let mapping = regseq::dma::map(buf, direction)?;
let addr = mapping.device_addr();
mapping.sync_for_device();
```

Accepted decisions:

- `DmaMapping` is the stable streaming-DMA handle surface
- stable handle methods include `device_addr()`, `sync_for_cpu()`, and
  `sync_for_device()`
- unmap is RAII-driven through Drop
- no separate raw-address-only stable surface

### Channel-control baseline

Stable channel surface:

```rust
impl DmaChannel {
    fn configure(&self);
    fn prepare(&self, mapping: &DmaMapping) -> Result<(), RegseqError>;
    fn prepare_sg(&self, mappings: &[&DmaMapping]) -> Result<(), RegseqError>;
    fn submit(&self) -> Result<(), RegseqError>;
    fn stop(&self);
}
```

Accepted decisions:

- `configure()` is parameterless; configuration is declaration/L2-derived
- `submit()` is the stable merged surface over the validated
  `channel_submit` + `channel_issue_pending` primitives
- `stop()` is infallible in the stable surface

### Excluded baseline

- no DMA-specific completion callback surface enters the stable compiler
  baseline
- no bound-handler DMA callback model is accepted at this stage
- PL011 fallback-to-PIO remains driver-authored control flow

## Extraction Baseline

The frontend must recognize:

- `#[rseq::dma_region(...)]` and `#[rseq::dma_channel(...)]` field attributes
- `DmaRegion<T>` and `DmaChannel` typed fields
- streaming map/sync use and `DmaMapping` handle flow
- Layer 2 DMA binding data:
  - `dma_mask`
  - `slave_addr_width`
  - `slave_maxburst`

Primary specification signals:

- field attributes for coherent-region and channel declarations
- typed fields for DMA resources
- wrapper and intrinsic call sites for DMA operations
- Layer 2 binding data for mask and slave parameters

Required extraction validations:

- DMA lowering family is conditioned by `bus_type`
- channel configuration derives from declaration + Layer 2 data
- streaming handle use must remain within `DmaMapping` lifetime rules
- no DMA completion callback surface should be inferred from backend-specific
  callback APIs

## Lowering Baseline

Lowering ownership:

- coherent allocation/release: compiler-generated
- channel request/release: compiler-generated
- DMA mask setup: compiler-generated
- SG construction and address translation: compiler-generated internals
- streaming map/sync/unmap and channel operations: backend lowers stable
  surface to target DMA APIs

### Generated lifecycle phase table

| Primitive | Init preamble | Deinit postamble | Suspend postamble | Resume preamble |
|---|---|---|---|---|
| `alloc_coherent` | yes | yes | no | no |
| `request_channel` | yes | yes | no | no |
| `set_dma_mask` | yes | no | no | no |
| `sg_construction` | compiler-internal | compiler-internal | no | no |
| `address_translation` | compiler-internal | compiler-internal | no | no |

Accepted PM rule:

- DMA is class `V` under PM
- DMA quiesce/restart is driver-authored and compiler-validated
- DMA does not appear in the generated PM sequence

## Cross-Construct Dependencies

Depends on:

- `Bus Resource Management` for `bus_type`, `set_bus_master`, and bus-handle
  context
- `Register Access` for register programming that consumes DMA-visible
  addresses
- `Device power state management` for PM interaction rules

Feeds:

- `Device interrupt setup and handling` and `Kernel timer lifecycle
  management` as existing completion-delivery paths
- device-class constructs such as `Serial/UART Management` that use DMA as a
  transport path

## Constraints and Caveats

- Keep the accepted absence of a DMA callback surface.
- Preserve the `DmaMapping` handle model and RAII unmap behavior.
- Treat Linux dmaengine descriptor callbacks as lowering-internal only.
- Treat PL011 fallback-to-PIO as driver-authored control flow.
- Carry forward audit constraint `IC-3` on DMA pause/status lowering.

## Representative Examples

```rust
struct Pl011 {
    #[rseq::dma_channel(id = "rx", direction = "dev_to_mem")]
    rx_dma: DmaChannel,
}

fn start_rx_dma(&self, mapping: &DmaMapping) -> Result<(), RegseqError> {
    self.rx_dma.configure();
    self.rx_dma.prepare(mapping)?;
    self.rx_dma.submit()
}
```

## Implementation Checklist

- Recognize DMA region/channel declarations and typed fields.
- Join DMA declarations with Layer 2 mask and slave configuration data.
- Implement the stable `DmaMapping` handle surface.
- Lower `configure`, `prepare`, `prepare_sg`, `submit`, and `stop` with the
  accepted wrapper semantics.
- Keep SG construction and address translation compiler-managed.
- Keep DMA PM behavior in the compiler-validated / driver-authored bucket.
