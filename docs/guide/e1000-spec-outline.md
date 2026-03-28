# E1000 Driver Spec Outline

Early implementation skeleton. Not a final DSL spec.

## Constructs used

- Register Access (register_block, Mmio via PCI BAR 0)
- Network Interface Management (NetworkDevice trait)
- DMA Management (DmaRegion for TX/RX descriptor rings, streaming DMA for packet buffers)
- Bus Resource Management (PCI bus, config space access)
- Synchronization (SpinLock for TX ring, IrqSpinLock for ISR-shared state)
- Interrupt (MSI with legacy fallback)
- Parameter Registration (ring sizes, debug level, interrupt throttle)
- Driver Lifecycle (#[rseq::driver], Lifecycle trait)
- Power Management (#[rseq::pm])
- Diagnostic Logging (regseq::log)
- Timed Delay (PHY settling delays)
- Deferred Work (link state change handling)
- Hardware Reset (hardware reset sequence)
- Atomic Operations (device state flags)

## Likely driver struct

```rust
#[rseq::driver(name = "e1000", compatible = ["8086:100e", "8086:100f"], bus = "pci")]
#[rseq::device_class(network)]
#[rseq::network(kind = "ethernet", max_mtu = 9216)]
#[rseq::pm(idle = false, wakeup_capable = true)]
pub struct E1000 {
    #[rseq::mmio(id = "regs", bar = 0, access_width = 32)]
    regs: Mmio<E1000Registers>,

    #[rseq::interrupt(id = "irq", msi = true, fallback_legacy = true)]
    irq: Interrupt,

    #[rseq::dma_region(id = "tx_desc", entry_size = 16, count_from = "tx_ring_size")]
    tx_ring: DmaRegion,

    #[rseq::dma_region(id = "rx_desc", entry_size = 16, count_from = "rx_ring_size")]
    rx_ring: DmaRegion,

    tx_lock: SpinLock<TxState>,
    state_flags: Atomic<u32>,
    link_work: Work,

    #[rseq::param(id = "tx_ring_size", default = 256, min = 64, max = 4096)]
    tx_ring_size: u32,

    #[rseq::param(id = "rx_ring_size", default = 256, min = 64, max = 4096)]
    rx_ring_size: u32,

    #[rseq::param(id = "debug_level", default = 0, writable = true)]
    debug_level: u32,
}
```

## Key properties

- register_block: E1000Registers (CTRL, STATUS, EERD, ICR, IMS, IMC, RCTL, TCTL, RDBAL, TDBAL, RDH, RDT, TDH, TDT, etc.)
- access_width: 32
- bus: PCI (vendor 8086, device 100e/100f)
- bar: 0 (primary register region)
- interrupt: MSI with legacy fallback
- DMA regions: TX and RX descriptor rings, entry_size = 16
- DMA mask: 64-bit
- max_mtu: 9216 (jumbo frames)
- tx/rx_ring_size: tunable, default 256
- debug_level: writable runtime parameter

## Key operations

- NetworkDevice trait: open, stop, start_xmit, set_rx_mode, set_mac_address, get_stats, change_mtu
- Intrinsics: regseq::net::deliver_packet, regseq::net::set_link_state, regseq::net::flow_control
- Intrinsics: regseq::dma::map/unmap/sync_for_cpu/sync_for_device
- Intrinsics: regseq::pci::read_config/write_config
- Intrinsics: regseq::log::error/warn/info/debug
- Intrinsics: regseq::delay::busy_wait/sleep
- ISR handler: read ICR, process RX/TX completions, schedule link_work
- Lifecycle: init (reset HW, read EEPROM MAC, zero rings, program ring base addresses)

## Notable compiler-managed work

- Init preamble: enable PCI device, claim BARs, set DMA mask (64-bit), enable bus master, map MMIO (BAR 0), resolve params, allocate DMA regions, init sync primitives, allocate MSI vector, register interrupt
- Deinit postamble: reverse order
- PCI-specific: bus type drives probe code generation (pci_register_driver vs module_platform_driver)
- DMA ring allocation: compiler allocates tx_ring/rx_ring in preamble using entry_size * tx_ring_size from #[rseq::param]

## OS differences of note

- Linux: module_param for ring sizes; pci_alloc_irq_vectors for MSI; dma_alloc_coherent for rings; NAPI-style RX delivery/lowering
- FreeBSD: TUNABLE_INT / SYSCTL_ADD_INT for ring sizes; pci_alloc_msi for MSI; bus_dmamem_alloc for rings; iflib framework
- Windows: registry for ring sizes; bus-negotiated interrupts; NdisMAllocateSharedMemory for rings; NDIS miniport
- Zephyr: Kconfig for ring sizes; no MSI; platform-specific DMA; ethernet_init framework
