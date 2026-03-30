# Construct Catalog

Validated Track A constructs from Step 9. 19 active constructs
(13 accepted, 6 accepted_with_caution). 1 rejected construct
listed in the appendix.

## Multi-concern constructs

### Parameter Registration

- **ID**: `parameter-registration`
- **Status**: `accepted_with_caution`
- **Primary resource**: runtime-accessible parameter interface (sysfs/sysctl/debugfs)
- **Member concerns**: [37] Runtime-tunable parameter declaration (abstraction); [14d] Runtime diagnostic parameter registration (abstraction)

**Operations:**

| Operation | Class | Concern lineage |
|-----------|-------|-----------------|
| declare_parameter | `compiler_managed_operation` | [37, 14d] |

**Declarable properties (declared_here):**

- `id` (string): #[rseq::param(id = "...")]
- `param_type` (enum (bool, u8, u16, u32, u64, i32, String)): Rust struct field type
- `default` (literal matching param_type): #[rseq::param(default = ...)]
- `min` (literal matching param_type (optional)): #[rseq::param(min = ...)]
- `max` (literal matching param_type (optional)): #[rseq::param(max = ...)]
- `writable` (bool (default: false)): #[rseq::param(writable = true)]
- `description` (string (optional)): #[rseq::param(description = "...")]

**Step 9 notes:**

- Declarative construct: the driver writes #[rseq::param] attributes; the compiler generates module_param/SYSCTL registration code. No explicit runtime invocation by the driver.

---

### Register Access

- **ID**: `register-access`
- **Status**: `accepted`
- **Primary resource**: device register space / MMIO region
- **Member concerns**: [4] Device register read/write access (abstraction); [8] Device register read and status query (abstraction); [18] Hardware register write and bit manipulation (library); [43a] MMIO physical-to-virtual register mapping (abstraction); [28b] Hardware register polling with timeout (library)

**Operations:**

| Operation | Class | Concern lineage |
|-----------|-------|-----------------|
| map_region | `compiler_managed_operation` | [43a] |
| read | `driver_facing_trait_operation` | [4, 8] |
| write | `driver_facing_trait_operation` | [4, 18] |
| poll | `driver_invoked_intrinsic` | [28b] |

**Declarable properties (declared_here):**

- `register_block` (struct definition via register_bitfields! macro): register_bitfields! [...] and #[rseq::register_block(...)]
- `access_width` (enum (8, 16, 32)): #[rseq::mmio(access_width = 32)]
- `bar` (u32 (PCI devices only)): #[rseq::mmio(bar = 0)]
- `endian` (enum (little, big, native)): #[rseq::register_block(endian = "little")]
- `side_effects` (per-field annotation): #[rseq::register(side_effect = "write_1_to_clear")]
- `shadow_registers` (per-field annotation): #[rseq::register(shadow = true)]

**Cross-operation shared properties:**

- `mmio_region`: The mapped MMIO region (Mmio<T>) is shared across all 4 operations. Read, write, modify, and poll all operate on registers within the same mapped region. The map_region operation populates this shared resource.

---

### DMA Management

- **ID**: `dma-management`
- **Status**: `accepted`
- **Primary resource**: DMA buffer / DMA channel / device-visible address space
- **Member concerns**: [1a] DMA-coherent memory allocation and pools (abstraction); [1b] DMA address mapping and cache synchronization (abstraction); [1c] Scatter-gather list construction and traversal (abstraction); [1d] DMA channel management and transfer execution (abstraction); [43b] Address translation and memory descriptor access (abstraction)

**Operations:**

| Operation | Class | Concern lineage |
|-----------|-------|-----------------|
| region_access | `driver_facing_trait_operation` | [1a] |
| streaming_map | `driver_invoked_intrinsic` | [1b, 43b] |
| streaming_unmap | `driver_invoked_intrinsic` | [1b] |
| streaming_sync | `driver_invoked_intrinsic` | [1b] |
| channel_configure | `driver_facing_trait_operation` | [1d] |
| channel_prepare | `driver_facing_trait_operation` | [1d, 1c] |
| channel_submit | `driver_facing_trait_operation` | [1d] |
| channel_issue_pending | `driver_facing_trait_operation` | [1d] |
| channel_stop | `driver_facing_trait_operation` | [1d] |
| alloc_coherent | `compiler_managed_operation` | [1a] |
| request_channel | `compiler_managed_operation` | [1d] |
| set_dma_mask | `compiler_managed_operation` | [1b] |
| sg_construction | `compiler_managed_operation` | [1c] |
| address_translation | `compiler_managed_operation` | [43b] |

**Declarable properties (declared_here):**

- `region_id` (string): #[rseq::dma_region(id = "tx_desc")]
- `entry_size` (u32 (bytes)): #[rseq::dma_region(entry_size = 16)]
- `entry_count` (u32 or reference to #[rseq::param] field): #[rseq::dma_region(count = 256)] or #[rseq::dma_region(count_from = "rx_ring_size")]
- `dma_mask` (u32 (bits: 32 or 64)): platform file: dma_mask = 64
- `channel_id` (string): #[rseq::dma_channel(id = "tx_dma")]
- `channel_direction` (enum (memory_to_device, device_to_memory, bidirectional)): #[rseq::dma_channel(direction = "memory_to_device")]
- `slave_addr_width` (u32 (bytes: 1, 2, 4, 8)): platform file: addr_width = 1
- `slave_maxburst` (u32): platform file: max_burst = 1

**Cross-operation shared properties:**

- `dma_mask`: The DMA address width is shared across coherent allocation and streaming mapping. Both must respect the device's address capability. Set once in the init preamble.

---

### Synchronization

- **ID**: `synchronization`
- **Status**: `accepted_with_caution`
- **Primary resource**: lock / synchronization primitive
- **Member concerns**: [21] Mutual exclusion lock management (abstraction); [41] Synchronization primitive lifecycle and acquisition (abstraction); [42] Spinlock acquisition and release (library); [49] Device lock acquire and release (library); [28c] Event and completion synchronization (abstraction)

**Operations:**

| Operation | Class | Concern lineage |
|-----------|-------|-----------------|
| mutex_lock | `driver_facing_trait_operation` | [21] |
| spinlock_lock | `driver_facing_trait_operation` | [42, 49] |
| completion_wait | `driver_facing_trait_operation` | [28c, 41] |
| completion_signal | `driver_facing_trait_operation` | [28c, 41] |
| completion_reset | `driver_facing_trait_operation` | [28c, 41] |
| init_primitives | `compiler_managed_operation` | [21, 41, 42] |
| destroy_primitives | `compiler_managed_operation` | [21, 41] |
| subsystem_locking | `compiler_managed_operation` | [49] |

**Declarable properties (declared_here):**

- `lock_type` (enum (Mutex, SpinLock, IrqSpinLock, Completion)): Rust struct field type: Mutex<T>, SpinLock<T>, IrqSpinLock<T>, Completion
- `protected_data_type` (generic type parameter T): Generic parameter: Mutex<DriverState>, IrqSpinLock<CallbackState>

**Cross-operation shared properties:**

- `raii_guard`: The RAII guard pattern is shared across mutex_lock, spinlock_lock. All lock types return a Guard<T> that releases the lock on drop. This shared pattern enables the compiler's phase-based ISR structuring checks (SYN-005 through SYN-010).
- `init_destroy_lifecycle`: All synchronization types share the init/destroy lifecycle managed by the compiler in preamble/postamble.

**Step 9 notes:**

- Type-system-expressed properties (lock_type, protected_data_type). These pass the 4-criterion test but are expressed through Rust generic parameters, not #[rseq::...] attributes.

---

### Bus Resource Management

- **ID**: `bus-resource-management`
- **Status**: `accepted`
- **Primary resource**: bus-level device resources (handles, config space, address regions)
- **Member concerns**: [15] PCI configuration and identifier parsing (abstraction); [51] PCI device enable and region claim (abstraction); [52] Bus resource and handle access (abstraction)

**Operations:**

| Operation | Class | Concern lineage |
|-----------|-------|-----------------|
| read_config | `driver_invoked_intrinsic` | [15] |
| write_config | `driver_invoked_intrinsic` | [15] |
| enable_device | `compiler_managed_operation` | [51] |
| claim_regions | `compiler_managed_operation` | [51] |
| set_bus_master | `compiler_managed_operation` | [51] |
| get_resource_handle | `compiler_managed_operation` | [52] |

**Declarable properties (declared_here):**

- `bus_type` (enum (pci, platform, i2c, spi)): #[rseq::driver(bus = "pci")]
- `bar_index` (u32 (PCI only)): #[rseq::mmio(bar = 0)]

**Consumed properties (consumed_here):**

- `compatible`: consumed from Driver Lifecycle
- `driver_name`: consumed from Driver Lifecycle

**Cross-operation shared properties:**

- `bus_device_handle`: The bus-level device handle (pci_dev, platform_device, bus_space_tag_t) is shared across all operations. Populated by the compiler during probe.

---

### Network Interface Management

- **ID**: `network-interface`
- **Status**: `accepted`
- **Primary resource**: network interface (netdev)
- **Member concerns**: [2a] Network packet buffer and data-plane management (abstraction); [10b] Network interface address and carrier state management (abstraction); [19] Network interface setup and attachment (abstraction)

**Operations:**

| Operation | Class | Concern lineage |
|-----------|-------|-----------------|
| open | `driver_facing_trait_operation` | [19] |
| stop | `driver_facing_trait_operation` | [19] |
| start_xmit | `driver_facing_trait_operation` | [2a] |
| set_rx_mode | `driver_facing_trait_operation` | [10b] |
| set_mac_address | `driver_facing_trait_operation` | [10b, 19] |
| get_stats | `driver_facing_trait_operation` | [2a] |
| change_mtu | `driver_facing_trait_operation` | [19] |
| alloc_netdev | `compiler_managed_operation` | [19] |
| deliver_packet | `driver_invoked_intrinsic` | [2a] |
| set_link_state | `driver_invoked_intrinsic` | [10b] |
| flow_control | `driver_invoked_intrinsic` | [2a] |

**Declarable properties (declared_here):**

- `kind` (enum (ethernet, can, wifi)): #[rseq::network(kind = "ethernet")]
- `max_mtu` (u32 (default: 1500)): #[rseq::network(max_mtu = 9216)]
- `tx_queues` (u32 (default: 1)): #[rseq::network(tx_queues = 4)]
- `rx_queues` (u32 (default: 1)): #[rseq::network(rx_queues = 4)]
- `default_mac` (MacAddress (optional)): platform file: default_mac = "00:11:22:33:44:55"

**Cross-operation shared properties:**

- `netdev_handle`: The OS network device handle (net_device, if_t, NDIS_HANDLE) is shared across all operations. Populated in init preamble.

---

### Serial/UART Management

- **ID**: `serial-uart`
- **Status**: `accepted`
- **Primary resource**: serial port / UART device
- **Member concerns**: [2b] Serial/UART character buffer operations (abstraction); [44] Serial port driver setup (abstraction)

**Operations:**

| Operation | Class | Concern lineage |
|-----------|-------|-----------------|
| port_open | `driver_facing_trait_operation` | [44] |
| port_close | `driver_facing_trait_operation` | [44] |
| configure | `driver_facing_trait_operation` | [44, 2b] |
| tx_byte | `driver_facing_trait_operation` | [2b] |
| rx_char | `driver_facing_trait_operation` | [2b] |
| tx_ready | `driver_facing_trait_operation` | [2b] |
| console_write | `driver_facing_trait_operation` | [44] |
| register_driver | `compiler_managed_operation` | [44] |
| push_rx_data | `driver_invoked_intrinsic` | [2b] |

**Declarable properties (declared_here):**

- `default_baud_rate` (u32): platform file: default_baud_rate
- `fifo_depth` (u32): platform file: fifo_depth = 16
- `supports_flow_control` (bool): platform file: supports_flow_control = true
- `supports_modem_signals` (bool): platform file: supports_modem_signals = false

**Cross-operation shared properties:**

- `uart_port`: The OS serial port handle (uart_port, uart_softc) is shared across all operations.

---

### Driver Lifecycle

- **ID**: `driver-lifecycle`
- **Status**: `accepted`
- **Primary resource**: driver / device object within the OS device model
- **Member concerns**: [35] Driver registration and lifecycle binding (abstraction); [39] Device object creation and registration (abstraction); [9] Driver private data association (abstraction)

**Operations:**

| Operation | Class | Concern lineage |
|-----------|-------|-----------------|
| init | `driver_facing_trait_operation` | [35, 39, 9] |
| deinit | `driver_facing_trait_operation` | [35, 39] |
| suspend | `driver_facing_trait_operation` | [35] |
| resume | `driver_facing_trait_operation` | [35] |
| register_driver | `compiler_managed_operation` | [35] |
| create_device | `compiler_managed_operation` | [39] |
| associate_private_data | `compiler_managed_operation` | [9] |

**Declarable properties (declared_here):**

- `driver_name` (string): #[rseq::driver(name = "e1000")]
- `compatible` (string array): #[rseq::driver(compatible = ["arm,pl011"])]
- `device_class` (enum (serial, network, storage, ...)): #[rseq::device_class(serial)]

**Cross-operation shared properties:**

- `device_handle`: The OS device handle (struct device, device_t, DEVICE_OBJECT, struct device) shared across all lifecycle operations.
- `private_data`: The driver's private state structure. Compiler allocates (via alloc_etherdev, device extension, softc embedding) and provides self reference.

---

### Diagnostic Logging

- **ID**: `diagnostic-logging`
- **Status**: `accepted_with_caution`
- **Primary resource**: kernel logging / tracing facility
- **Member concerns**: [20] Diagnostic message emission (library); [48] Diagnostic logging and tracing (abstraction)

**Operations:**

| Operation | Class | Concern lineage |
|-----------|-------|-----------------|
| log_error | `driver_invoked_intrinsic` | [20, 48] |
| log_warn | `driver_invoked_intrinsic` | [20] |
| log_info | `driver_invoked_intrinsic` | [20] |
| log_debug | `driver_invoked_intrinsic` | [20, 48] |
| register_log_module | `compiler_managed_operation` | [48] |

**Declarable properties (declared_here):**

- `debug_logging` (bool (default: false)): platform file: debug_logging = true

**Cross-operation shared properties:**

- `device_context`: All log operations auto-prepend device identity (from #[rseq::driver(name)] + bus address). The compiler injects the device context argument.

**Step 9 notes:**

- Property-light: 1 declared_here property (debug_logging). Device context is compiler_derived from Driver Lifecycle.

---

## Singleton constructs

### Device interrupt setup and handling

- **ID**: `interrupt` (concern [3], abstraction)
- **Status**: `accepted`
- **Primary resource**: interrupt line / IRQ vector

**Operations:**

| Operation | Class |
|-----------|-------|
| handle | `driver_facing_trait_operation` |
| enable | `driver_invoked_intrinsic` |
| disable | `driver_invoked_intrinsic` |
| register | `compiler_managed_operation` |
| allocate_vectors | `compiler_managed_operation` |

**Declarable properties:**

- `id` (string): #[rseq::interrupt(id = "irq")]
- `mode` (enum (legacy, msi, msix)): #[rseq::interrupt(msi = true)] or #[rseq::interrupt(msix = true)]
- `trigger_type` (enum (level_high, level_low, edge_rising, edge_falling, edge_both)): platform file trigger_type field
- `shared` (bool (default: false for MSI/MSI-X, true for legacy)): platform file shared field
- `vector_count` (u32 (MSI-X only)): platform file msix_count field or [Interrupt; N] array size
- `fallback_legacy` (bool (default: true)): #[rseq::interrupt(fallback_legacy = true)]
- `priority` (u32 (optional, primarily embedded)): platform file priority field

---

### Device power state management

- **ID**: `power-management` (concern [5], abstraction)
- **Status**: `accepted`
- **Primary resource**: device power state

**Operations:**

| Operation | Class |
|-----------|-------|
| suspend | `driver_facing_trait_operation` |
| resume | `driver_facing_trait_operation` |
| pm_registration | `compiler_managed_operation` |
| clock_gating | `compiler_managed_operation` |

**Declarable properties:**

- `idle` (bool (default: false)): #[rseq::pm(idle = false)]
- `wakeup_capable` (bool (default: false)): #[rseq::pm(wakeup_capable = false)]

---

### Peripheral clock management

- **ID**: `clock-management` (concern [31], abstraction)
- **Status**: `accepted`
- **Primary resource**: peripheral clock reference

**Operations:**

| Operation | Class |
|-----------|-------|
| enable | `driver_facing_trait_operation` |
| disable | `driver_facing_trait_operation` |
| get_rate | `driver_facing_trait_operation` |
| acquire_clock | `compiler_managed_operation` |

**Declarable properties:**

- `clock_id` (string): #[rseq::clock(id = "uart_clk")]
- `default_frequency` (u32 (Hz)): platform file: default_frequency = 48000000
- `required` (bool (via Option<Clock>)): clock: Clock vs clock: Option<Clock>

---

### Firmware property access

- **ID**: `firmware-property-access` (concern [7], abstraction)
- **Status**: `accepted_with_caution`
- **Primary resource**: firmware description (DT/ACPI/DMI)

**Operations:**

| Operation | Class |
|-----------|-------|
| read_property | `compiler_managed_operation` |

**Declarable properties:**

- `prop_id` (string): #[rseq::property(id = "reg-io-width")]
- `prop_default` (literal (optional)): #[rseq::property(default = 4)]
- `vendor_prefix` (string (optional)): #[rseq::property(vendor_prefix = "arm")]
- `prop_required` (bool (via Option<T> or default)): field type: u32 (required) vs Option<u32> (optional)

**Step 9 notes:**

- Declarative construct: the driver writes #[rseq::property] attributes; the compiler generates OS-specific property extraction code (device_property_read_u32, DT_INST_PROP_OR, OF_getencprop). No explicit runtime invocation.

---

### Hardware reset sequencing and post-reset polling

- **ID**: `hardware-reset` (concern [27b], abstraction)
- **Status**: `accepted`
- **Primary resource**: hardware reset line

**Operations:**

| Operation | Class |
|-----------|-------|
| assert_reset | `driver_facing_trait_operation` |
| deassert_reset | `driver_facing_trait_operation` |
| toggle_reset | `driver_facing_trait_operation` |

**Declarable properties:**

- `reset_required` (bool (via Option<ResetLine>)): reset: ResetLine vs reset: Option<ResetLine>
- `reset_shared` (bool): platform file: shared = true
- `reset_line_id` (string): platform file: line = "uart-reset"

---

### Pin controller and GPIO configuration

- **ID**: `pin-gpio` (concern [30a], abstraction)
- **Status**: `accepted`
- **Primary resource**: pin multiplexing / GPIO

**Operations:**

| Operation | Class |
|-----------|-------|
| apply_state | `driver_facing_trait_operation` |

**Declarable properties:**

- `pin_states` (string array): platform file: states = ["default", "sleep"]
- `pinctrl_required` (bool (via Option<PinCtrl>)): pinctrl: PinCtrl vs pinctrl: Option<PinCtrl>

---

### Atomic memory access and ordering

- **ID**: `atomic-operations` (concern [11], abstraction)
- **Status**: `accepted`
- **Primary resource**: atomic variable

**Operations:**

| Operation | Class |
|-----------|-------|
| load | `driver_facing_trait_operation` |
| store | `driver_facing_trait_operation` |
| fetch_modify | `driver_facing_trait_operation` |
| compare_exchange | `driver_facing_trait_operation` |
| bit_test_set | `driver_facing_trait_operation` |

**Declarable properties:**

- `atomic_type` (generic type (u32, bool, etc.)): Atomic<u32>
- `initial_value` (T (optional, default: 0)): Atomic::new(0)

---

### Timed CPU delay and sleep

- **ID**: `timed-delay` (concern [28a], abstraction)
- **Status**: `accepted_with_caution`
- **Primary resource**: CPU execution time

**Operations:**

| Operation | Class |
|-----------|-------|
| busy_wait | `driver_invoked_intrinsic` |
| sleep | `driver_invoked_intrinsic` |

**Declarable properties:** none (see Step 9 notes)

**Step 9 notes:**

- Zero attribute-declared properties. Check 2 passes on the argument that operation selection and hardware-mandated duration values constitute specification decisions, even though they are expressed as call-site choices rather than static attributes.

---

### Deferred work item scheduling

- **ID**: `deferred-work` (concern [13], abstraction)
- **Status**: `accepted_with_caution`
- **Primary resource**: kernel work queue / deferred procedure call

**Operations:**

| Operation | Class |
|-----------|-------|
| submit | `driver_facing_trait_operation` |
| submit_unique | `driver_facing_trait_operation` |
| handle_work | `driver_facing_trait_operation` |
| init_work | `compiler_managed_operation` |
| cancel_work | `compiler_managed_operation` |

**Declarable properties:**

- `work_handler_binding` (method reference): #[rseq::work_handler(field_name)]

**Step 9 notes:**

- Property-light construct.

---

### Kernel timer lifecycle management

- **ID**: `timer-management` (concern [34], abstraction)
- **Status**: `accepted`
- **Primary resource**: software kernel timer

**Operations:**

| Operation | Class |
|-----------|-------|
| start | `driver_facing_trait_operation` |
| start_with | `driver_facing_trait_operation` |
| stop | `driver_facing_trait_operation` |
| handle_timer | `driver_facing_trait_operation` |
| init_timer | `compiler_managed_operation` |
| auto_routing | `compiler_managed_operation` |

**Declarable properties:**

- `timer_id` (string): #[rseq::timer(id = "poll")]
- `timer_mode` (enum (oneshot, periodic)): #[rseq::timer(mode = "periodic")]
- `interval_ms` (u32 (ms) or param reference): #[rseq::timer(interval_ms = 100)] or interval_from = "poll_rate"
- `tolerance_ms` (u32 (ms, optional)): #[rseq::timer(tolerance_ms = 10)]
- `resolution` (enum (standard, high)): #[rseq::timer(resolution = "high")]
- `timer_handler_binding` (method reference): #[rseq::timer_handler(field_name)]

---

## Appendix: Rejected / Subsumed

### Hardware capability flag extraction

- **ID**: `capability-extraction` (concern [45])
- **Status**: `rejected`
- **Failed check_2**: 0 declared_here properties. Capability register layouts belong to Register Access register_block definitions.
- **Failed check_5**: Capability extraction is a specialized register read. Its operations and specification decisions can be expressed through Register Access (read operation on capability registers defined in register_block).
- **Action**: subsume into `register-access`. Capability flag extraction is a register read operation on capability registers defined in the register_block. No separate construct needed.
