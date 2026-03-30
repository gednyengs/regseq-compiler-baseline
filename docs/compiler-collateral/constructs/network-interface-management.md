# Network Interface Management

## Purpose

`Network interface management` defines the device-class network callback
surface, network-specific declaration properties, packet-delivery intrinsics,
and compiler-managed netdev allocation/registration.

This is the network analogue of `Serial/UART Management`: a device-class
callback construct plus a small intrinsic companion surface.

## Accepted Primitive Inventory

### Layer 1

| Primitive | Kind | Stable surface |
|---|---|---|
| `network_interface.kind` | property | `#[rseq::network(kind = "...")]` |
| `network_interface.max_mtu` | property | `#[rseq::network(max_mtu = ...)]` |
| `network_interface.tx_queues` | property | `#[rseq::network(tx_queues = ...)]` |
| `network_interface.rx_queues` | property | `#[rseq::network(rx_queues = ...)]` |
| `network_interface.open` | operation | device-class trait method |
| `network_interface.stop` | operation | device-class trait method |
| `network_interface.start_xmit` | operation | device-class trait method |
| `network_interface.set_rx_mode` | operation | device-class trait method |
| `network_interface.set_mac_address` | operation | device-class trait method |
| `network_interface.get_stats` | operation | device-class trait method |
| `network_interface.change_mtu` | operation | device-class trait method |
| `network_interface.deliver_packet` | operation | network intrinsic |
| `network_interface.set_link_state` | operation | network intrinsic |
| `network_interface.flow_control` | operation | network intrinsic |

### Layer 2

| Primitive | Kind | Stable surface |
|---|---|---|
| `network_interface.default_mac` | property | hardware binding field |

### Compiler-managed

| Primitive | Role |
|---|---|
| `network_interface.alloc_netdev` | allocate/register and deregister/free OS network device |

## Accepted Surface Baseline

### Device-class callback baseline

Current accepted compiler baseline uses a `Network` device-class trait:

```rust
trait Network {
    fn open(&mut self) -> Result<(), RegseqError>;
    fn stop(&mut self) -> Result<(), RegseqError>;
    fn start_xmit(&mut self, packet: NetPacket) -> TxResult;
    fn set_rx_mode(&mut self);
    fn set_mac_address(&mut self, addr: MacAddress) -> Result<(), RegseqError>;
    fn get_stats(&mut self) -> NetStats;
    fn change_mtu(&mut self, new_mtu: u32) -> Result<(), RegseqError>;
}
```

Accepted callback conventions:

- callback entry context: `S` (subsystem-serialized / stack-serialized entry)
- `start_xmit` returns `TxResult`, not `RegseqError`
- `get_stats` returns `NetStats`, not `RegseqError`
- nested lifecycle is `open` / `stop`, analogous to serial `port_open` /
  `port_close`

### Intrinsic baseline

Stable companion intrinsics:

```rust
regseq::net::deliver_packet(packet);
regseq::net::set_link_state(LinkState::Up);
regseq::net::flow_control(FlowControl::TxRx);
```

These are Category C driver-invoked operations and do not belong in the
device-class trait.

### Property baseline

- `kind`, `max_mtu`, `tx_queues`, `rx_queues`: Layer 1 declaration properties
- `default_mac`: Layer 2 deployment/binding property

## Extraction Baseline

The frontend must recognize:

- `#[rseq::network(...)]` driver-item attributes
- `impl Network for Driver` trait methods
- network intrinsic call sites
- Layer 2 `default_mac`

Primary specification signals:

- item attributes for network declaration properties
- trait method surface for device-class callbacks
- intrinsic call sites for packet delivery, link state, and flow control
- Layer 2 binding data for default MAC address

Required extraction validations:

- `tx_queues` and `rx_queues` are separate properties
- `start_xmit` returns `TxResult`
- `get_stats` returns `NetStats`
- `deliver_packet` remains intrinsic, not callback-shaped

## Lowering Baseline

Lowering ownership:

- `Network` callbacks: backend realizes OS-specific callback contracts
- intrinsics: backend lowers to network-stack operations
- `alloc_netdev`: compiler-generated

### Generated lifecycle phase table

| Primitive | Init preamble | Deinit postamble | Suspend postamble | Resume preamble |
|---|---|---|---|---|
| `alloc_netdev` | yes | yes | no | no |

Accepted lowering notes:

- `alloc_netdev` covers allocation/registration in init and
  deregistration/free in deinit
- DMA, Interrupt, and Bus Resource constructs provide the lower-level
  mechanisms consumed inside network callbacks
- exact `TxResult` and `NetStats` shape details remain near-frozen rather than
  fully closed

## Cross-Construct Dependencies

Depends on:

- `DMA Management` for ring and packet-buffer data paths
- `Device interrupt setup and handling` for RX/TX completion delivery
- `Bus Resource Management` for PCI/network-device setup
- `Diagnostic Logging` for network diagnostics

Feeds:

- `Deferred work item scheduling` for link/reset/admin work
- higher-level E1000 driver structure and spec authoring

## Constraints and Caveats

- Keep the Category C intrinsics out of the `Network` trait.
- Preserve `tx_queues` and `rx_queues` as separate declaration properties.
- Keep `alloc_netdev` compiler-managed and lifecycle-spanning.
- Treat exact `TxResult`, `NetStats`, and intrinsic signature details as
  near-frozen implementation work, not open primitive-mapping questions.

## Representative Examples

```rust
#[rseq::network(kind = "ethernet", max_mtu = 9216, tx_queues = 1, rx_queues = 1)]
struct E1000 {
    // ...
}
```

## Implementation Checklist

- Recognize all `#[rseq::network(...)]` declaration properties.
- Extract `Network` trait methods with the accepted callback/result categories.
- Keep `deliver_packet`, `set_link_state`, and `flow_control` as intrinsics.
- Generate `alloc_netdev` across init/deinit.
- Preserve DMA/interrupt/bus dependencies instead of folding them into this
  construct.
