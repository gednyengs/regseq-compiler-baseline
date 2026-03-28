# Lowering Contract

Implementation-facing specification for each validated Track A
construct. For each construct: what the driver author expresses,
what the compiler must lower, which OS-specific mechanisms are
involved.

## Parameter Registration

**Driver author expresses:**

- `id`: #[rseq::param(id = "...")]
- `param_type`: Rust struct field type
- `default`: #[rseq::param(default = ...)]
- `min`: #[rseq::param(min = ...)]
- `max`: #[rseq::param(max = ...)]
- `writable`: #[rseq::param(writable = true)]
- `description`: #[rseq::param(description = "...")]

**Compiler must lower:**

- `declare_parameter`: Declare a named, typed, optionally constrained parameter that the driver author supplies as static configuration and the compiler exposes through the OS's parameter interface. The compiler generates both the load-time declaration and, if writable, the runtime registration node.

**OS lowering:**

- **Linux**:
  - `declare_parameter`: load_time: module_param(name, type, perms) or E1000_PARAM macro; runtime_diagnostic: debugfs_create_file / debugfs_create_dir; structural_notes: Linux separates load-time params (module_param, global) from diagnostic params (debugfs, per-device). The DSL compiler must generate both paths for writable params.
- **FreeBSD**:
  - `declare_parameter`: load_time: TUNABLE_INT("hw.driver.name", &val); runtime_diagnostic: SYSCTL_ADD_INT / SYSCTL_ADD_PROC / SYSCTL_ADD_COUNTER_U64; structural_notes: FreeBSD unifies tunables and diagnostics into the SYSCTL tree. Per-device params use SYSCTL_ADD_* with a per-device context node. Handler-backed params (SYSCTL_ADD_PROC) allow computed values.
- **Windows**:
  - `declare_parameter`: load_time: Registry HKLM\...\Parameters\name; runtime_diagnostic: Registry (persistent key-value); structural_notes: Windows uses registry for both load-time and runtime. No separate diagnostic interface; all parameters are registry entries.
- **Zephyr**:
  - `declare_parameter`: load_time: Kconfig CONFIG_* symbol; runtime_diagnostic: N/A (no runtime parameter mechanism); structural_notes: Zephyr params are compile-time only via Kconfig. Writable params generate a compile warning. No runtime modification path.

---

## Register Access

**Driver author expresses:**

- `register_block`: register_bitfields! [...] and #[rseq::register_block(...)]
- `access_width`: #[rseq::mmio(access_width = 32)]
- `bar`: #[rseq::mmio(bar = 0)]
- `endian`: #[rseq::register_block(endian = "little")]
- `side_effects`: #[rseq::register(side_effect = "write_1_to_clear")]
- `shadow_registers`: #[rseq::register(shadow = true)]
- 2 trait operation(s): read, write
- 1 intrinsic(s): poll

**Compiler must lower:**

- `map_region`: Map a physical device register range into the kernel virtual address space. Compiler-managed: the compiler generates the mapping call in the init preamble and the unmapping call in the deinit postamble. The driver author declares the region; the compiler handles the mapping lifecycle.
- Generate OS-specific implementations for 2 trait operation(s)
- Replace 1 intrinsic call(s) with OS-specific code

**OS lowering:**

- **Linux**:
  - `map_region`: devm_ioremap_resource / devm_platform_ioremap_resource / pci_ioremap_bar
  - `read`: readl/readb/readl_relaxed (raw MMIO) or regmap_read/serial_in (framework)
  - `write`: writel/writeb (raw MMIO) or regmap_write/regmap_update_bits/regmap_set_bits/serial_out (framework)
  - `poll`: read_poll_timeout / readl_relaxed_poll_timeout / readl_relaxed_poll_timeout_atomic / regmap_read_poll_timeout / ata_wait_register
- **FreeBSD**:
  - `map_region`: bus_alloc_resource(SYS_RES_MEMORY) + bus_space_subregion
  - `read`: bus_space_read_4 (raw) or uart_getreg/read_reg (framework)
  - `write`: bus_space_write_4 (raw) or uart_setreg/write_reg (framework)
  - `poll`: (hand-rolled polling loops with DELAY/pause_sbt)
- **Windows**:
  - `map_region`: NdisMMapIoSpace / StorPortGetDeviceBase
  - `read`: NdisReadRegisterUlong / READ_REGISTER_ULONG / StorPortReadRegisterUlong
  - `write`: NdisWriteRegisterUlong / WRITE_REGISTER_ULONG / StorPortWriteRegisterUlong
  - `poll`: (hand-rolled polling loops with KeStallExecutionProcessor)
- **Zephyr**:
  - `map_region`: device_map / DEVICE_MMIO_RAM_PTR
  - `read`: sys_read32 / ns16550_inbyte
  - `write`: sys_write32 / ns16550_outbyte
  - `poll`: (hand-rolled loops with k_busy_wait)

---

## DMA Management

**Driver author expresses:**

- `region_id`: #[rseq::dma_region(id = "tx_desc")]
- `entry_size`: #[rseq::dma_region(entry_size = 16)]
- `entry_count`: #[rseq::dma_region(count = 256)] or #[rseq::dma_region(count_from = "rx_ring_size")]
- `dma_mask`: platform file: dma_mask = 64
- `channel_id`: #[rseq::dma_channel(id = "tx_dma")]
- `channel_direction`: #[rseq::dma_channel(direction = "memory_to_device")]
- `slave_addr_width`: platform file: addr_width = 1
- `slave_maxburst`: platform file: max_burst = 1
- 6 trait operation(s): region_access, channel_configure, channel_prepare, channel_submit, channel_issue_pending, channel_stop
- 3 intrinsic(s): streaming_map, streaming_unmap, streaming_sync

**Compiler must lower:**

- `alloc_coherent`: Allocate coherent DMA region in init preamble. Size = entry_size * count. Free in deinit postamble.
- `request_channel`: Request DMA engine channel in init preamble. Release in deinit postamble.
- `set_dma_mask`: Configure the device's DMA address mask in init preamble. Determines whether 32-bit or 64-bit DMA addresses are used.
- `sg_construction`: Build scatter-gather lists internally during channel_prepare or streaming_map. The driver author does not construct SG lists directly.
- `address_translation`: Translate between virtual, physical, and bus-physical addresses during map operations. Compiler-internal.
- Generate OS-specific implementations for 6 trait operation(s)
- Replace 3 intrinsic call(s) with OS-specific code

**OS lowering:**

- **Linux**:
  - `region_access`: vaddr returned by dma_alloc_coherent; dma_addr_t returned as out-param. E1000 writes txdr->dma to TDBAL register.
  - `streaming_map`: dma_map_single(dev, buf, len, direction). Returns dma_addr_t. Direction selects cache operation (clean for ToDevice, invalidate for FromDevice).
  - `streaming_unmap`: dma_unmap_single(dev, dma_addr, len, direction)
  - `streaming_sync`: dma_sync_single_for_cpu / dma_sync_single_for_device
  - `channel_configure`: dmaengine_slave_config(chan, config). Config struct has src_addr, dst_addr, src_addr_width, dst_addr_width, src_maxburst, dst_maxburst.
  - `channel_prepare`: dmaengine_prep_slave_sg(chan, sgl, nents, direction, flags). Internally builds SG list from buffer using sg_init_table, sg_set_buf. Returns a dma_async_tx_descriptor.
  - `channel_submit`: dmaengine_submit(desc). Returns a cookie for status tracking.
  - `channel_issue_pending`: dma_async_issue_pending(chan)
  - `channel_stop`: dmaengine_terminate_sync(chan). Synchronous: guarantees no callback is running on return.
  - `alloc_coherent`: dma_alloc_coherent / dma_free_coherent
  - `request_channel`: dma_request_chan / dma_release_channel
  - `set_dma_mask`: dma_set_mask_and_coherent(dev, DMA_BIT_MASK(bits))
  - `sg_construction`: sg_init_table, sg_set_buf, for_each_sg, sg_dma_len, sg_dma_address
  - `address_translation`: page_address, virt_to_page, page_to_phys, bvec_virt
- **FreeBSD**:
  - `region_access`: bus_dmamem_alloc returns vaddr; bus_dmamap_load provides bus_addr_t. Two-step process requiring a tag+map pair.
  - `streaming_map`: bus_dmamap_load(tag, map, buf, len, callback, ...) + bus_dmamap_sync(PREWRITE|POSTREAD). Requires pre-created tag and map objects.
  - `streaming_unmap`: bus_dmamap_unload(tag, map)
  - `streaming_sync`: bus_dmamap_sync(BUS_DMASYNC_POSTREAD) / bus_dmamap_sync(BUS_DMASYNC_PREWRITE)
  - `channel_configure`: Part of bus_dma_tag_create constraints (alignment, boundary, address range, max size, max segments).
  - `channel_prepare`: bus_dmamap_load(tag, map, buf, len, callback) for single-buffer; bus_dmamap_load_mbuf_sg for packet buffers.
  - `channel_submit`: bus_dmamap_sync(PREWRITE) + hardware register kick.
  - `channel_issue_pending`: N/A (hardware register kick after sync)
  - `channel_stop`: bus_dmamap_unload + bus_dma_tag_destroy
  - `alloc_coherent`: bus_dma_tag_create + bus_dmamem_alloc / bus_dmamem_free + bus_dma_tag_destroy
  - `request_channel`: N/A (no slave DMA engine abstraction)
  - `set_dma_mask`: bus_dma_tag_create(lowaddr = BUS_SPACE_MAXADDR / BUS_SPACE_MAXADDR_32BIT)
  - `sg_construction`: bus_dmamap_load_mbuf_sg
  - `address_translation`: memdesc_vaddr, memdesc_vmpages
- **Windows**:
  - `region_access`: NdisMAllocateSharedMemory returns both PVOID (virtual) and NDIS_PHYSICAL_ADDRESS (physical). E1000 writes TransmitDescriptorsPa to device register.
  - `streaming_map`: StorPortGetPhysicalAddress / NdisGetPhysicalAddressLow. Address translation is simpler (no explicit cache ops on x86).
  - `streaming_unmap`: No-op on most Windows targets (x86 identity mapping)
  - `streaming_sync`: No-op on x86 (cache-coherent architecture)
  - `channel_configure`: N/A (no slave DMA engine abstraction)
  - `channel_prepare`: N/A
  - `channel_submit`: N/A
  - `channel_issue_pending`: N/A
  - `channel_stop`: N/A
  - `alloc_coherent`: NdisMAllocateSharedMemory / NdisMFreeSharedMemory or StorPortAllocateDmaMemory / StorPortFreeDmaMemory
  - `request_channel`: N/A
  - `set_dma_mask`: Implicit in adapter registration
  - `sg_construction`: NdisMInitializeScatterGatherDma, SCATTER_GATHER_LIST
  - `address_translation`: StorPortGetPhysicalAddress, MmGetSystemAddressForMdlSafe
- **Zephyr**:
  - `region_access`: Platform-specific coherent allocation. Address is physical on most embedded targets.
  - `streaming_map`: Direct physical address on most embedded targets. Cache management via explicit cache flush/invalidate instructions.
  - `streaming_unmap`: No-op on most embedded targets (direct physical)
  - `streaming_sync`: Explicit cache invalidate/clean instructions (platform-specific)
  - `channel_configure`: dma_config(dev, channel, config). Config struct has channel_direction, source_burst_length, dest_burst_length, source_data_size, dest_data_size.
  - `channel_prepare`: dma_reload(dev, channel, src, dst, size). Simpler model without explicit descriptors.
  - `channel_submit`: dma_start(dev, channel). Simpler: start is implicit after config+reload.
  - `channel_issue_pending`: dma_start(dev, channel) (submit and issue are combined)
  - `channel_stop`: dma_stop(dev, channel)
  - `alloc_coherent`: Platform-specific
  - `request_channel`: dma_config (static channel assignment)
  - `set_dma_mask`: N/A (physical addressing)
  - `sg_construction`: N/A (simple buffer-based DMA)

---

## Synchronization

**Driver author expresses:**

- `lock_type`: Rust struct field type: Mutex<T>, SpinLock<T>, IrqSpinLock<T>, Completion
- `protected_data_type`: Generic parameter: Mutex<DriverState>, IrqSpinLock<CallbackState>
- 5 trait operation(s): mutex_lock, spinlock_lock, completion_wait, completion_signal, completion_reset

**Compiler must lower:**

- `init_primitives`: Initialize all synchronization primitives in init preamble. Mutex: allocate and init. SpinLock/IrqSpinLock: init. Completion: init to unsignaled state.
- `destroy_primitives`: Destroy synchronization primitives in deinit postamble.
- `subsystem_locking`: Compiler-generated acquire/release of subsystem-required locks at framework boundaries. When the driver calls a Serial or Network device class operation, the compiler generates the appropriate subsystem lock acquisition (uart_port_lock, rtnl_lock). The driver does not call these directly.
- Generate OS-specific implementations for 5 trait operation(s)

**OS lowering:**

- **Linux**:
  - `mutex_lock`: mutex_lock / mutex_unlock (RAII generated by compiler)
  - `spinlock_lock`: SpinLock: spin_lock / spin_unlock. IrqSpinLock: spin_lock_irqsave / spin_unlock_irqrestore (saves/restores flags). Device locks: uart_port_lock_irqsave / uart_port_unlock_irqrestore.
  - `completion_wait`: wait_for_completion_timeout(completion, timeout). Also: init_waitqueue_head + wait_event for wait-queue pattern.
  - `completion_signal`: complete(completion). Also: wake_up_all / wake_up_interruptible for wait-queue pattern.
  - `completion_reset`: reinit_completion(completion).
  - `init_primitives`: mutex_init, spin_lock_init, init_completion
  - `destroy_primitives`: mutex_destroy (debug builds only), cleanup_srcu_struct
  - `subsystem_locking`: uart_port_lock_irqsave / uart_port_unlock_irqrestore, rtnl_lock / rtnl_unlock
- **FreeBSD**:
  - `mutex_lock`: mtx_lock(MTX_DEF) / mtx_unlock or sx_xlock / sx_xunlock
  - `spinlock_lock`: SpinLock: mtx_lock(MTX_DEF) / mtx_unlock. IrqSpinLock: mtx_lock_spin / mtx_unlock_spin.
  - `completion_wait`: cv_timedwait(cv, mtx, timeout) or wakeup/tsleep pattern.
  - `completion_signal`: cv_signal(cv) or wakeup(ident).
  - `completion_reset`: cv_init (re-init).
  - `init_primitives`: mtx_init, cv_init
  - `destroy_primitives`: mtx_destroy, cv_destroy
  - `subsystem_locking`: (subsystem locks not applicable)
- **Windows**:
  - `mutex_lock`: ExAcquireFastMutex / ExReleaseFastMutex
  - `spinlock_lock`: SpinLock: KeAcquireSpinLockAtDpcLevel / KeReleaseSpinLockFromDpcLevel. IrqSpinLock: KeAcquireSpinLock / KeReleaseSpinLock (raises/restores IRQL). Device locks: NdisAcquireSpinLock / NdisReleaseSpinLock.
  - `completion_wait`: KeWaitForSingleObject(event, ...) with timeout. KeWaitForMultipleObjects for multi-event wait.
  - `completion_signal`: KeSetEvent(event, IO_NO_INCREMENT, FALSE).
  - `completion_reset`: KeClearEvent(event).
  - `init_primitives`: KeInitializeSpinLock, ExInitializeFastMutex, KeInitializeEvent
  - `destroy_primitives`: N/A (stack/struct-embedded, no explicit destroy)
  - `subsystem_locking`: NdisAcquireSpinLock (NDIS subsystem lock)
- **Zephyr**:
  - `mutex_lock`: k_mutex_lock / k_mutex_unlock
  - `spinlock_lock`: k_spin_lock / k_spin_unlock (saves/restores key). No SpinLock vs IrqSpinLock distinction on Zephyr.
  - `completion_wait`: k_sem_take(sem, timeout). Binary semaphore used as completion.
  - `completion_signal`: k_sem_give(sem).
  - `completion_reset`: k_sem_reset(sem).
  - `init_primitives`: k_mutex_init, k_spin_lock (static init), k_sem_init
  - `destroy_primitives`: N/A (no explicit destroy)
  - `subsystem_locking`: (subsystem locks not applicable)

---

## Bus Resource Management

**Driver author expresses:**

- `bus_type`: #[rseq::driver(bus = "pci")]
- `bar_index`: #[rseq::mmio(bar = 0)]
- `compatible` (consumed from Driver Lifecycle)
- `driver_name` (consumed from Driver Lifecycle)
- 2 intrinsic(s): read_config, write_config

**Compiler must lower:**

- `enable_device`: Enable PCI device I/O and memory resources in init preamble. Disable in deinit postamble.
- `claim_regions`: Claim exclusive BAR regions in init preamble. Release in deinit postamble.
- `set_bus_master`: Enable bus mastering for DMA-capable PCI devices.
- `get_resource_handle`: Retrieve bus-level device handle and convert between device structures.
- Replace 2 intrinsic call(s) with OS-specific code

**OS lowering:**

- **Linux**:
  - `read_config`: pci_read_config_byte / pci_read_config_word / pci_read_config_dword
  - `write_config`: pci_write_config_word / pci_write_config_dword
  - `enable_device`: pci_enable_device / pcim_enable_device
  - `claim_regions`: pci_request_regions / pci_request_selected_regions
  - `set_bus_master`: pci_set_master
  - `get_resource_handle`: to_pci_dev / to_platform_device / pci_name
- **FreeBSD**:
  - `read_config`: pci_read_config(dev, offset, width)
  - `write_config`: pci_write_config(dev, offset, value, width)
  - `enable_device`: (bus attachment)
  - `claim_regions`: bus_alloc_resource(SYS_RES_MEMORY)
  - `set_bus_master`: pci_enable_busmaster
  - `get_resource_handle`: bus_alloc_resource_any / rman_get_bushandle / rman_get_bustag
- **Windows**:
  - `read_config`: NdisReadPciSlotInformation
  - `write_config`: (config space write)
  - `enable_device`: (PnP start device)
  - `claim_regions`: (resource list from PnP)
  - `set_bus_master`: (implicit)
  - `get_resource_handle`: NdisMQueryAdapterResources
- **Zephyr**:
  - `read_config`: pcie_conf_read
  - `write_config`: pcie_conf_write
  - `enable_device`: (static config)
  - `claim_regions`: (static DT)
  - `set_bus_master`: (implicit)
  - `get_resource_handle`: recover_bus_cb

---

## Network Interface Management

**Driver author expresses:**

- `kind`: #[rseq::network(kind = "ethernet")]
- `max_mtu`: #[rseq::network(max_mtu = 9216)]
- `tx_queues`: #[rseq::network(tx_queues = 4)]
- `rx_queues`: #[rseq::network(rx_queues = 4)]
- `default_mac`: platform file: default_mac = "00:11:22:33:44:55"
- 7 trait operation(s): open, stop, start_xmit, set_rx_mode, set_mac_address, get_stats, change_mtu
- 3 intrinsic(s): deliver_packet, set_link_state, flow_control

**Compiler must lower:**

- `alloc_netdev`: Allocate and register the network device with the OS stack in init preamble.
- Generate OS-specific implementations for 7 trait operation(s)
- Replace 3 intrinsic call(s) with OS-specific code

**OS lowering:**

- **Linux**:
  - `open`: ndo_open callback
  - `stop`: ndo_stop callback
  - `start_xmit`: ndo_start_xmit
  - `set_rx_mode`: ndo_set_rx_mode
  - `set_mac_address`: ndo_set_mac_address + eth_hw_addr_set
  - `get_stats`: ndo_get_stats64
  - `change_mtu`: ndo_change_mtu
  - `alloc_netdev`: alloc_etherdev + register_netdev
  - `deliver_packet`: napi_gro_receive / netif_receive_skb
  - `set_link_state`: netif_carrier_on / netif_carrier_off
  - `flow_control`: netif_stop_queue / netif_wake_queue
- **FreeBSD**:
  - `open`: iflib_if_init
  - `stop`: iflib_if_stop
  - `start_xmit`: iflib_if_transmit
  - `set_rx_mode`: iflib_if_multi_set
  - `set_mac_address`: iflib_if_set_mac_addr
  - `get_stats`: iflib_if_get_counter
  - `change_mtu`: iflib_if_mtu_set
  - `alloc_netdev`: iflib_get_ifp
  - `deliver_packet`: if_input / iflib_rx
  - `set_link_state`: iflib_link_state_change
  - `flow_control`: (iflib handles)
- **Windows**:
  - `open`: MiniportInitialize
  - `stop`: MiniportHalt
  - `start_xmit`: MiniportSendPackets
  - `set_rx_mode`: MiniportSetMulticastList
  - `set_mac_address`: (registry-based)
  - `get_stats`: MiniportQueryInformation(OID_GEN_STATISTICS)
  - `change_mtu`: MiniportSetInformation(OID_GEN_CURRENT_PACKET_FILTER)
  - `alloc_netdev`: NdisMRegisterMiniport
  - `deliver_packet`: NdisMIndicateReceivePacket
  - `set_link_state`: NdisMIndicateStatus
  - `flow_control`: NdisMSendResourcesAvailable
- **Zephyr**:
  - `open`: ethernet_init
  - `stop`: N/A
  - `start_xmit`: ethernet_send
  - `set_rx_mode`: (not applicable)
  - `set_mac_address`: net_if_set_link_addr
  - `get_stats`: (not standard)
  - `change_mtu`: (fixed at compile time)
  - `alloc_netdev`: ethernet_init
  - `deliver_packet`: net_recv_data
  - `set_link_state`: (callback)
  - `flow_control`: (callback)

---

## Serial/UART Management

**Driver author expresses:**

- `default_baud_rate`: platform file: default_baud_rate
- `fifo_depth`: platform file: fifo_depth = 16
- `supports_flow_control`: platform file: supports_flow_control = true
- `supports_modem_signals`: platform file: supports_modem_signals = false
- 7 trait operation(s): port_open, port_close, configure, tx_byte, rx_char, tx_ready, console_write
- 1 intrinsic(s): push_rx_data

**Compiler must lower:**

- `register_driver`: Register the serial driver with the tty/console subsystem.
- Generate OS-specific implementations for 7 trait operation(s)
- Replace 1 intrinsic call(s) with OS-specific code

**OS lowering:**

- **Linux**:
  - `port_open`: uart_ops.startup
  - `port_close`: uart_ops.shutdown
  - `configure`: uart_ops.set_termios
  - `tx_byte`: uart_ops.putchar / serial_out(UART_TX)
  - `rx_char`: serial8250_read_char -> uart_insert_char -> tty_flip_buffer_push
  - `tx_ready`: serial_in(UART_LSR) & UART_LSR_THRE
  - `console_write`: uart_console_write
  - `register_driver`: uart_register_driver / uart_add_one_port
  - `push_rx_data`: tty_flip_buffer_push
- **FreeBSD**:
  - `port_open`: uart_bus_attach
  - `port_close`: uart_bus_detach
  - `configure`: uart_bus_param
  - `tx_byte`: uart_putc
  - `rx_char`: uart_rx_put
  - `tx_ready`: uart_getreg(UART_LSR)
  - `console_write`: uart_cnputc
  - `register_driver`: DRIVER_MODULE + uart_bus_probe
  - `push_rx_data`: (integrated in receive path)
- **Zephyr**:
  - `port_open`: (static init)
  - `port_close`: (N/A)
  - `configure`: uart_configure
  - `tx_byte`: uart_poll_out
  - `rx_char`: uart_fifo_read
  - `tx_ready`: uart_irq_tx_ready
  - `console_write`: uart_poll_out (loop)
  - `register_driver`: DEVICE_DT_INST_DEFINE
  - `push_rx_data`: uart_irq_callback_user_data_set

---

## Driver Lifecycle

**Driver author expresses:**

- `driver_name`: #[rseq::driver(name = "e1000")]
- `compatible`: #[rseq::driver(compatible = ["arm,pl011"])]
- `device_class`: #[rseq::device_class(serial)]
- 4 trait operation(s): init, deinit, suspend, resume

**Compiler must lower:**

- `register_driver`: Register driver with bus subsystem for device discovery and matching.
- `create_device`: Create device object in the OS device model during probe.
- `associate_private_data`: Store driver-private context in the OS device object.
- Generate OS-specific implementations for 4 trait operation(s)

**OS lowering:**

- **Linux**:
  - `init`: probe callback body
  - `deinit`: remove callback body
  - `suspend`: pm.suspend callback
  - `resume`: pm.resume callback
  - `register_driver`: module_platform_driver / module_pci_driver / pci_register_driver
  - `create_device`: device_add / cdev_device_add (for char devices)
  - `associate_private_data`: dev_set_drvdata / platform_set_drvdata / pci_set_drvdata / netdev_priv (via alloc_etherdev)
- **FreeBSD**:
  - `init`: attach callback body
  - `deinit`: detach callback body
  - `suspend`: DEVMETHOD(device_suspend)
  - `resume`: DEVMETHOD(device_resume)
  - `register_driver`: DRIVER_MODULE / DECLARE_MODULE
  - `create_device`: device_add_child / make_dev_s
  - `associate_private_data`: device_get_softc (embedded in device allocation)
- **Windows**:
  - `init`: DriverEntry / AddDevice / StartDevice body
  - `deinit`: RemoveDevice body
  - `suspend`: PowerDown IRP
  - `resume`: PowerUp IRP
  - `register_driver`: DriverEntry + NdisMRegisterMiniport
  - `create_device`: IoCreateDevice / IoAttachDeviceToDeviceStackSafe
  - `associate_private_data`: NdisMSetAttributesEx / DeviceExtension
- **Zephyr**:
  - `init`: DEVICE_DT_INST_DEFINE init function body
  - `deinit`: (static, no remove)
  - `suspend`: pm_device_action_run(PM_DEVICE_ACTION_SUSPEND)
  - `resume`: pm_device_action_run(PM_DEVICE_ACTION_RESUME)
  - `register_driver`: DEVICE_DT_INST_DEFINE
  - `create_device`: DEVICE_API
  - `associate_private_data`: CONTAINER_OF from device->data

---

## Diagnostic Logging

**Driver author expresses:**

- `debug_logging`: platform file: debug_logging = true
- 4 intrinsic(s): log_error, log_warn, log_info, log_debug

**Compiler must lower:**

- `register_log_module`: Register the logging module with the OS tracing subsystem. Zephyr-specific: LOG_MODULE_REGISTER.
- Replace 4 intrinsic call(s) with OS-specific code

**OS lowering:**

- **Linux**:
  - `log_error`: dev_err(dev, fmt, ...)
  - `log_warn`: dev_warn
  - `log_info`: dev_info
  - `log_debug`: dev_dbg (compiled out unless DEBUG or dynamic debug)
  - `register_log_module`: (not needed; dev_* auto-contextualizes)
- **FreeBSD**:
  - `log_error`: device_printf(dev, fmt, ...)
  - `log_warn`: device_printf
  - `log_info`: device_printf
  - `log_debug`: DPRINTF (conditional on debug flag)
  - `register_log_module`: (not needed)
- **Windows**:
  - `log_error`: StorPortLogError / StorPortDebugPrint(0, ...)
  - `log_warn`: StorPortDebugPrint(0, ...)
  - `log_info`: StorPortDebugPrint(1, ...)
  - `log_debug`: StorPortDebugPrint(3, ...)
  - `register_log_module`: (not needed)
- **Zephyr**:
  - `log_error`: LOG_ERR(fmt, ...)
  - `log_warn`: LOG_WRN
  - `log_info`: LOG_INF
  - `log_debug`: LOG_DBG (compiled out unless CONFIG_LOG_DEFAULT_LEVEL >= 4)
  - `register_log_module`: LOG_MODULE_REGISTER(driver_name, LOG_LEVEL_INF)

---

## Device interrupt setup and handling

**Driver author expresses:**

- `id`: #[rseq::interrupt(id = "irq")]
- `mode`: #[rseq::interrupt(msi = true)] or #[rseq::interrupt(msix = true)]
- `trigger_type`: platform file trigger_type field
- `shared`: platform file shared field
- `vector_count`: platform file msix_count field or [Interrupt; N] array size
- `fallback_legacy`: #[rseq::interrupt(fallback_legacy = true)]
- `priority`: platform file priority field
- 1 trait operation(s): handle
- 2 intrinsic(s): enable, disable

**Compiler must lower:**

- `register`: Compiler-generated in init preamble: request the IRQ line and bind the driver's ISR. Reverse (deregister) in deinit postamble.
- `allocate_vectors`: Compiler-generated in init preamble for PCI devices with msi/msix properties: allocate interrupt vectors from the PCI subsystem. Compiler implements the fallback strategy (MSI-X -> MSI -> legacy) based on the fallback_legacy property.
- Generate OS-specific implementations for 1 trait operation(s)
- Replace 2 intrinsic call(s) with OS-specific code

**OS lowering:**

- **Linux**:
  - `handle`: request_irq callback returning IRQ_HANDLED/IRQ_NONE; shared handlers form a chain dispatched in sequence
  - `enable`: enable_irq(irq)
  - `disable`: disable_irq(irq) -- synchronous, waits for in-flight ISR to complete. Also disable_irq_nosync for non-waiting variant.
  - `register`: request_irq / devm_request_irq / devm_request_threaded_irq
  - `allocate_vectors`: pci_alloc_irq_vectors / pci_irq_vector
- **FreeBSD**:
  - `handle`: bus_setup_intr callback returning FILTER_HANDLED/FILTER_STRAY; interrupt filter model
  - `enable`: (automatic after bus_setup_intr; explicit via register writes)
  - `disable`: (bus_teardown_intr for full removal; register writes for masking)
  - `register`: bus_setup_intr / bus_teardown_intr
  - `allocate_vectors`: pci_alloc_msi / pci_alloc_msix / pci_msix_count
- **Windows**:
  - `handle`: IoConnectInterrupt ISR via KeSynchronizeExecution; flag-based registration state; deferred processing via DPC
  - `enable`: NICApplyInterruptMask / StorPortSynchronizeAccess
  - `disable`: NICDisableInterrupts -- flag-based masking
  - `register`: IoConnectInterrupt / NdisMRegisterInterrupt
  - `allocate_vectors`: StorPortGetMSIInfo (bus-negotiated)
- **Zephyr**:
  - `handle`: irq_config_func direct callback; single handler per IRQ; no shared-interrupt chain
  - `enable`: irq_enable(irq)
  - `disable`: irq_disable(irq)
  - `register`: irq_config_func (static, compile-time)
  - `allocate_vectors`: N/A (no PCI MSI support)

---

## Device power state management

**Driver author expresses:**

- `idle`: #[rseq::pm(idle = false)]
- `wakeup_capable`: #[rseq::pm(wakeup_capable = false)]
- 2 trait operation(s): suspend, resume

**Compiler must lower:**

- `pm_registration`: Register PM callbacks with the OS framework.
- `clock_gating`: Auto-disable clocks in suspend postamble, auto-enable in resume preamble.
- Generate OS-specific implementations for 2 trait operation(s)

**OS lowering:**

- **Linux**:
  - `suspend`: dev_pm_ops.suspend / pm_runtime callbacks
  - `resume`: dev_pm_ops.resume / pm_runtime callbacks
  - `pm_registration`: DEFINE_SIMPLE_DEV_PM_OPS
- **FreeBSD**:
  - `suspend`: DEVMETHOD(device_suspend) / bus_generic_suspend
  - `resume`: DEVMETHOD(device_resume) / bus_generic_resume
  - `pm_registration`: DEVMETHOD entries
- **Windows**:
  - `suspend`: PoCallDriver for power IRP / StorPortPoFxIdleComponent
  - `resume`: PoStartNextPowerIrp / StorPortPoFxActivateComponent
  - `pm_registration`: PnP power dispatch
- **Zephyr**:
  - `suspend`: pm_device_action_run / pm_device_busy_set/clear
  - `resume`: pm_device_runtime_enable
  - `pm_registration`: PM_DEVICE_DT_INST_DEFINE

---

## Peripheral clock management

**Driver author expresses:**

- `clock_id`: #[rseq::clock(id = "uart_clk")]
- `default_frequency`: platform file: default_frequency = 48000000
- `required`: clock: Clock vs clock: Option<Clock>
- 3 trait operation(s): enable, disable, get_rate

**Compiler must lower:**

- `acquire_clock`: Acquire clock reference in init preamble. Release in deinit postamble.
- Generate OS-specific implementations for 3 trait operation(s)

**OS lowering:**

- **Linux**:
  - `enable`: clk_prepare_enable
  - `disable`: clk_disable_unprepare
  - `get_rate`: clk_get_rate
  - `acquire_clock`: devm_clk_get / devm_clk_get_optional
- **FreeBSD**:
  - `enable`: clk_enable
  - `disable`: clk_release
  - `get_rate`: clk_get_freq
  - `acquire_clock`: clk_get_by_ofw_index
- **Windows**:
  - `enable`: NO-OP (firmware-managed)
  - `disable`: NO-OP
  - `get_rate`: platform file constant
  - `acquire_clock`: N/A
- **Zephyr**:
  - `enable`: clock_control_on
  - `disable`: clock_control_off
  - `get_rate`: clock_control_get_rate
  - `acquire_clock`: (DT static)

---

## Firmware property access

**Driver author expresses:**

- `prop_id`: #[rseq::property(id = "reg-io-width")]
- `prop_default`: #[rseq::property(default = 4)]
- `vendor_prefix`: #[rseq::property(vendor_prefix = "arm")]
- `prop_required`: field type: u32 (required) vs Option<u32> (optional)

**Compiler must lower:**

- `read_property`: Read a typed property from the device's firmware description. Compiler-managed: generated in init preamble for each #[rseq::property] field.

**OS lowering:**

- **Linux**:
  - `read_property`: device_property_read_u32 / of_property_read_u32 / of_property_read_bool
- **FreeBSD**:
  - `read_property`: OF_getencprop / resource_int_value
- **Windows**:
  - `read_property`: StorPortInvokeAcpiMethod / ACPI_METHOD_*
- **Zephyr**:
  - `read_property`: DT_INST_PROP_OR (compile-time)

---

## Hardware reset sequencing and post-reset polling

**Driver author expresses:**

- `reset_required`: reset: ResetLine vs reset: Option<ResetLine>
- `reset_shared`: platform file: shared = true
- `reset_line_id`: platform file: line = "uart-reset"
- 3 trait operation(s): assert_reset, deassert_reset, toggle_reset

**Compiler must lower:**

- Generate OS-specific implementations for 3 trait operation(s)

**OS lowering:**

- **Linux**:
  - `assert_reset`: reset_control_assert
  - `deassert_reset`: reset_control_deassert
  - `toggle_reset`: reset_control_reset
- **FreeBSD**:
  - `assert_reset`: (vendor-specific reset_hw)
  - `deassert_reset`: (vendor-specific)
  - `toggle_reset`: (vendor-specific)
- **Windows**:
  - `assert_reset`: NO-OP (firmware-managed)
  - `deassert_reset`: NO-OP
  - `toggle_reset`: NO-OP
- **Zephyr**:
  - `assert_reset`: reset_line_toggle_dt (assert phase)
  - `deassert_reset`: reset_line_toggle_dt (deassert phase)
  - `toggle_reset`: reset_line_toggle_dt

---

## Pin controller and GPIO configuration

**Driver author expresses:**

- `pin_states`: platform file: states = ["default", "sleep"]
- `pinctrl_required`: pinctrl: PinCtrl vs pinctrl: Option<PinCtrl>
- 1 trait operation(s): apply_state

**Compiler must lower:**

- Generate OS-specific implementations for 1 trait operation(s)

**OS lowering:**

- **Linux**:
  - `apply_state`: pinctrl_pm_select_default_state / pinctrl_apply_state
- **FreeBSD**:
  - `apply_state`: fdt_pinctrl_configure
- **Windows**:
  - `apply_state`: NO-OP
- **Zephyr**:
  - `apply_state`: pinctrl_apply_state / gpio_pin_configure_dt

---

## Atomic memory access and ordering

**Driver author expresses:**

- `atomic_type`: Atomic<u32>
- `initial_value`: Atomic::new(0)
- 5 trait operation(s): load, store, fetch_modify, compare_exchange, bit_test_set

**Compiler must lower:**

- Generate OS-specific implementations for 5 trait operation(s)

**OS lowering:**

- **Linux**:
  - `load`: smp_load_acquire / READ_ONCE + barrier
  - `store`: smp_store_release / WRITE_ONCE + barrier
  - `fetch_modify`: atomic_fetch_or / atomic_add_return
  - `compare_exchange`: atomic_cmpxchg / cmpxchg
  - `bit_test_set`: test_and_set_bit
- **FreeBSD**:
  - `load`: atomic_load_acq_int
  - `store`: atomic_store_rel_int
  - `fetch_modify`: atomic_fetchadd_int / atomic_set_int
  - `compare_exchange`: atomic_cmpset_32
  - `bit_test_set`: atomic_testandset_int
- **Windows**:
  - `load`: ReadAcquire / InterlockedCompareExchange(read)
  - `store`: WriteRelease / InterlockedExchange
  - `fetch_modify`: InterlockedOr / InterlockedAdd
  - `compare_exchange`: InterlockedCompareExchange
  - `bit_test_set`: InterlockedBitTestAndSet
- **Zephyr**:
  - `load`: atomic_get + barrier
  - `store`: barrier + atomic_set
  - `fetch_modify`: atomic_or / atomic_add
  - `compare_exchange`: atomic_cas
  - `bit_test_set`: atomic_test_and_set_bit

---

## Timed CPU delay and sleep

**Driver author expresses:**

- 2 intrinsic(s): busy_wait, sleep

**Compiler must lower:**

- Replace 2 intrinsic call(s) with OS-specific code

**OS lowering:**

- **Linux**:
  - `busy_wait`: udelay / mdelay / cpu_relax
  - `sleep`: msleep / usleep_range / fsleep
- **FreeBSD**:
  - `busy_wait`: DELAY
  - `sleep`: pause / pause_sbt
- **Windows**:
  - `busy_wait`: KeStallExecutionProcessor / StorPortStallExecution
  - `sleep`: NdisMSleep / KeDelayExecutionThread
- **Zephyr**:
  - `busy_wait`: k_busy_wait
  - `sleep`: k_sleep

---

## Deferred work item scheduling

**Driver author expresses:**

- `work_handler_binding`: #[rseq::work_handler(field_name)]
- 3 trait operation(s): submit, submit_unique, handle_work

**Compiler must lower:**

- `init_work`: Initialize work item in preamble.
- `cancel_work`: Cancel/flush pending work in deinit postamble.
- Generate OS-specific implementations for 3 trait operation(s)

**OS lowering:**

- **Linux**:
  - `submit`: schedule_work / queue_work
  - `submit_unique`: schedule_work (inherent coalescing)
  - `init_work`: INIT_WORK
  - `cancel_work`: cancel_work_sync / flush_work
- **FreeBSD**:
  - `submit`: taskqueue_enqueue
  - `submit_unique`: taskqueue_enqueue (inherent coalescing)
  - `init_work`: TASK_INIT
  - `cancel_work`: taskqueue_drain
- **Windows**:
  - `submit`: IoQueueWorkItem / KeInsertQueueDpc
  - `submit_unique`: IoQueueWorkItem
  - `init_work`: IoAllocateWorkItem / KeInitializeDpc
  - `cancel_work`: IoFreeWorkItem
- **Zephyr**:
  - `submit`: k_work_submit
  - `submit_unique`: k_work_submit (inherent coalescing)
  - `init_work`: k_work_init
  - `cancel_work`: k_work_cancel_sync

---

## Kernel timer lifecycle management

**Driver author expresses:**

- `timer_id`: #[rseq::timer(id = "poll")]
- `timer_mode`: #[rseq::timer(mode = "periodic")]
- `interval_ms`: #[rseq::timer(interval_ms = 100)] or interval_from = "poll_rate"
- `tolerance_ms`: #[rseq::timer(tolerance_ms = 10)]
- `resolution`: #[rseq::timer(resolution = "high")]
- `timer_handler_binding`: #[rseq::timer_handler(field_name)]
- 4 trait operation(s): start, start_with, stop, handle_timer

**Compiler must lower:**

- `init_timer`: Create timer in preamble. Destroy in deinit postamble.
- `auto_routing`: If handler body needs thread context, compiler routes callback through work queue.
- Generate OS-specific implementations for 4 trait operation(s)

**OS lowering:**

- **Linux**:
  - `start`: mod_timer / hrtimer_start
  - `stop`: timer_delete_sync / hrtimer_cancel
  - `init_timer`: timer_setup / hrtimer_setup
- **FreeBSD**:
  - `start`: callout_reset / callout_schedule
  - `stop`: callout_drain
  - `init_timer`: callout_init_mtx
- **Windows**:
  - `start`: KeSetTimer / StorPortRequestTimer
  - `stop`: KeCancelTimer
  - `init_timer`: KeInitializeTimer / StorPortInitializeTimer
- **Zephyr**:
  - `start`: k_timer_start
  - `stop`: k_timer_stop
  - `init_timer`: k_timer_init

---
