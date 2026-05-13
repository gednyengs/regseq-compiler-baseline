# Formal Representability Matrix: Linux PL011 and Linux E1000

## Scope

This matrix records whether the accepted Regseq taxonomy can represent the
semantics exercised by two Linux reference drivers:

- PL011 UART:
  `/Users/nyengele/driverbase/os/linux/drivers/tty/serial/amba-pl011.c`
- legacy E1000 NIC:
  `/Users/nyengele/driverbase/os/linux/drivers/net/ethernet/intel/e1000`

The claim being tested here is semantic representability, not source-to-source
translation. A Linux feature is represented only when the matrix can cite both:

1. concrete source evidence in the reference driver, and
2. an accepted taxonomy construct, compiler-managed primitive, Layer 2 binding
   rule, or compiler constraint in the authoritative taxonomy collateral.

Authoritative taxonomy root for this matrix:

`/Users/nyengele/Documents/sekekama/regseq-taxonomy-work/research/construct-discovery-v3/track-a/10-authoritative/compiler-collateral`

Taxonomy references below use paths relative to that root. The repo-local
`docs/compiler-collateral` copy is useful context, but it is not treated as
the source of truth for this matrix because it currently differs from the
authoritative handoff package.

Rows marked as backend-specific lowering or compiler-managed are not weaker
than direct DSL rows. They state a different owner: the driver author expresses
the semantic fact through the accepted Regseq surface, while the backend or
compiler emits the Linux-specific mechanism.

## Evidence Standard

A row is considered formally represented when it satisfies one of the following
evidence routes.

| Route | Meaning | Required evidence |
|---|---|---|
| `R-DIRECT` | Accepted Layer 1 DSL surface directly captures the feature. | Source feature plus construct inventory/surface evidence. |
| `R-CM` | Accepted compiler-managed primitive owns the feature. | Source feature plus construct compiler-managed primitive/lowering evidence. |
| `R-L2` | Feature is deployment or hardware binding data joined with Layer 1. | Source feature plus Layer 2/join evidence. |
| `R-BE` | Feature is target-specific realization of an accepted semantic construct or accepted compiler constraint. | Source feature plus lowering/constraint evidence. |
| `R-RUST` | Feature is ordinary driver algorithm over represented resources and has no independent OS-facing semantic contract. | Source feature plus evidence that the underlying resources/operations are represented. |
| `R-TCAP` | Feature depends on a target capability; unsupported targets use a represented fallback or reject the configuration. | Source feature plus target-capability/lowering evidence. |
| `N/A` | Row records that an accepted taxonomy feature is not exercised by the audited source. | Taxonomy evidence plus source absence evidence. |
| `EXT-CANDIDATE` | Existing taxonomy can express the operational behavior, but a first-class portable primitive may be justified later. | Source feature plus explicit caveat that current accepted taxonomy does not expose a dedicated surface. |
| `GAP` | No accepted route found. | Must cite source evidence and absence/contradiction in taxonomy evidence. |

This document deliberately separates `R-DIRECT`, `R-CM`, `R-L2`, and `R-BE`.
Collapsing them into a single "covered" label would hide the core Regseq design
boundary between authored specification, deployment binding, compiler synthesis,
and OS-specific backend lowering.

## Taxonomy Evidence Ledger

| ID | Accepted taxonomy evidence |
|---|---|
| `T-BASE` | The exported handoff package is authoritative for compiler consumption and covers validated PL011, E1000, NS16550, and RTL8139 constructs with no surviving blocking gap: `compiler-baseline-overview.md:17-44`. Accepted surface families and compiler-managed responsibilities are summarized at `compiler-baseline-overview.md:65-145`. Validation status marks the package ready for PL011 and E1000 compiler work at `validation-status.md:13-18` and concentrates remaining risk in `IC-1` through `IC-15` at `validation-status.md:121-128`. |
| `T-LIFE` | `Driver Lifecycle` owns `driver_name`, `compatible`, `device_class`, `Lifecycle::init`, `Lifecycle::deinit`, and compiler-managed `register_driver`, `create_device`, `associate_private_data`: `constructs/driver-lifecycle.md:18-37`. Lowering ownership is explicit at `driver-lifecycle.md:91-112`. |
| `T-BUS` | `Bus Resource Management` owns explicit `bus_type`, PCI BAR declaration, config-space intrinsics, and compiler-managed `enable_device`, `claim_regions`, `set_bus_master`, `get_resource_handle`: `constructs/bus-resource-management.md:16-35`. PCI operations are structurally conditioned by `bus_type`: `bus-resource-management.md:91-105`. |
| `T-REG` | `Register Access` owns register block metadata, `Mmio<T>`, `read`, `write`, `poll`, Layer 2 physical mapping facts, and compiler-managed `map_region`: `constructs/register-access.md:15-40`. Backend MMIO lowering and map-region ownership are at `register-access.md:115-128`. |
| `T-SER` | `Serial/UART Management` owns `Serial`, optional `SerialConsole`, `push_rx_data`, Layer 2 serial properties, and compiler-managed serial registration: `constructs/serial-uart-management.md:16-38`. Callback and lowering ownership are at `serial-uart-management.md:47-77` and `serial-uart-management.md:102-124`. |
| `T-NET` | `Network Interface Management` owns `Network` callbacks, network declaration properties, `deliver_packet`, `set_link_state`, `flow_control`, Layer 2 `default_mac`, and compiler-managed `alloc_netdev`: `constructs/network-interface-management.md:16-43`. Lowering ownership is at `network-interface-management.md:112-130`. |
| `T-IRQ` | `Device Interrupt Setup and Handling` owns `Interrupt`, `#[rseq::isr]`, Layer 2 IRQ properties, and compiler-managed `enable`, `disable`, `register_handler`, `allocate_vectors`: `constructs/device-interrupt-setup-and-handling.md:16-38`. Bound-handler and lowering rules are at `device-interrupt-setup-and-handling.md:67-94` and `device-interrupt-setup-and-handling.md:120-145`. |
| `T-DMA` | `DMA Management` owns `DmaRegion<T>`, `DmaChannel`, streaming `DmaMapping`, sync/map/unmap, channel configure/prepare/submit/stop, Layer 2 DMA parameters, and compiler-managed coherent allocation/channel acquisition/DMA mask/address translation: `constructs/dma-management.md:16-51`. The accepted absence of a DMA callback surface is explicit at `dma-management.md:105-110` and `dma-management.md:183-189`. |
| `T-PM` | `Device Power State Management` owns `#[rseq::pm]`, `PowerManagement::suspend`, `PowerManagement::resume`, PM registration, and generated Clock/PinCtrl/Interrupt ordering: `constructs/device-power-state-management.md:14-31`. Accepted PM ordering and ownership classes are at `device-power-state-management.md:101-115`. |
| `T-TIMER` | `Kernel Timer Lifecycle Management` owns `Timer`, timer metadata, `#[rseq::timer_handler]`, `start`, `start_with`, `stop`, compiler-managed `init_timer`, and `auto_routing`: `constructs/kernel-timer-lifecycle-management.md:13-36`. Handler and lowering rules are at `kernel-timer-lifecycle-management.md:59-77` and `kernel-timer-lifecycle-management.md:102-153`. |
| `T-WORK` | `Deferred Work Item Scheduling` owns `Work`, `#[rseq::work_handler]`, `submit`, `submit_unique`, and compiler-managed `init_work`/`cancel_work`: `constructs/deferred-work-item-scheduling.md:17-33`. Lowering and coalescing caveats are at `deferred-work-item-scheduling.md:109-129`. |
| `T-SYNC` | `Synchronization` owns `Mutex`, `SpinLock`, `IrqSpinLock`, `Completion`, lock/wait/signal/reset operations, compiler-managed init/destroy, and callback-context discipline: `constructs/synchronization.md:16-36` and `synchronization.md:76-90`. |
| `T-ATOMIC` | `Atomic Memory Access and Ordering` owns typed `Atomic<T>`, explicit ordering operations, fetch/compare/bit-test wrappers, and intentionally excludes MMIO ordering: `constructs/atomic-memory-access-and-ordering.md:16-35` and `atomic-memory-access-and-ordering.md:113-118`. |
| `T-FW` | `Firmware Property Access` owns `#[rseq::property]`, defaults, required/optional semantics, Layer 2 property fallbacks, and compiler-managed `read_property`: `constructs/firmware-property-access.md:16-33`. The three-source resolution model is at `firmware-property-access.md:109-125`. |
| `T-PARAM` | `Parameter Registration` owns `#[rseq::param]`, default/range/writable/description/type metadata, and compiler-managed `declare_parameter`: `constructs/parameter-registration.md:16-34`. Type/range validation is compiler-generated per `parameter-registration.md:80-99`. |
| `T-LOG` | `Diagnostic Logging` owns `regseq::log::{error,warn,info,debug}!`, Layer 2 debug toggle, device-context injection, and compiler-managed log registration: `constructs/diagnostic-logging.md:16-33` and `diagnostic-logging.md:79-108`. |
| `T-CLOCK` | `Peripheral Clock Management` owns `Clock`, clock identity/requiredness, `enable`, `disable`, `get_rate`, Layer 2 default frequency, and compiler-managed `acquire_clock`: `constructs/peripheral-clock-management.md:16-34`. |
| `T-PIN` | `Pin Controller and GPIO Configuration` owns `PinCtrl`, requiredness, `apply`, Layer 2 `pin_states`, and compiler-managed `acquire_pinctrl`: `constructs/pin-controller-and-gpio-configuration.md:15-31`. PM owns generated suspend/resume pin transitions: `pin-controller-and-gpio-configuration.md:99-118`. |
| `T-RESET` | `Hardware Reset Sequencing` owns `ResetLine`, assert/deassert/toggle, Layer 2 reset identity/sharing, and compiler-managed `acquire_reset`: `constructs/hardware-reset-sequencing-and-post-reset-polling.md:16-35`. |
| `T-DELAY` | `Timed CPU Delay and Sleep` owns `busy_wait`, `sleep`, and `Duration`; it explicitly keeps Linux `cpu_relax()` poll hints in Register Access lowering, not authored delay: `constructs/timed-cpu-delay-and-sleep.md:16-27` and `timed-cpu-delay-and-sleep.md:59-65`. |
| `T-CONSTRAINTS` | Shared constraints license key backend obligations: AMBA FIFO fallback `IC-1`, earlycon `IC-2`, DMA pause/status/residue `IC-3`, nbcon `IC-4`, MMIO barriers `IC-5`, `push_rx_data` overflow `IC-6`, power-domain ordering `IC-7`, poll-loop yield hints `IC-8`, PCI preamble/rollback `IC-9`, DMA descriptor barriers `IC-10`, NetPacket offload metadata `IC-11`, port I/O lowering `IC-12`, device capability tables `IC-13`, TX packet completion `IC-14`, and PHY/MII infrastructure `IC-15`: `compiler-constraints.md:25-229`. |

## Linux PL011 Matrix

| ID | Linux feature | Source evidence | Taxonomy evidence | Route | Status / caveat |
|---|---|---|---|---|---|
| `PL011-01` | AMBA and SBSA driver registration. | `amba-pl011.c:3061`, `amba-pl011.c:3089`, `amba-pl011.c:3100`. | `T-LIFE`, `T-BUS`. | `R-CM`, `R-BE` | Represented as compiler-generated registration glue. Linux AMBA/platform split is backend realization of portable bus identity. |
| `PL011-02` | OF/ACPI/AMBA hardware matching and model-data selection. | `amba-pl011.c:3048`, `amba-pl011.c:3054`, `amba-pl011.c:3073`. | `T-LIFE`, `T-BUS`, `T-FW`. | `R-L2`, `R-BE` | Match IDs and AMBA metadata belong in Layer 2/bus binding; backend emits Linux match tables. |
| `PL011-03` | Device state allocation and drvdata association. | `amba-pl011.c:2888`, `amba-pl011.c:2930`, `amba-pl011.c:2935`. | `T-LIFE`. | `R-CM` | `create_device` and `associate_private_data` cover this; no authored DSL API required. |
| `PL011-04` | MMIO resource mapping. | `amba-pl011.c:2814`, `amba-pl011.c:2820`. | `T-BUS`, `T-REG`. | `R-CM`, `R-L2` | `Mmio<T>` plus Layer 2 region facts drives compiler-generated `map_region`. |
| `PL011-05` | Variant register offsets and 16/32-bit access width. | `amba-pl011.c:86`, `amba-pl011.c:172`, `amba-pl011.c:295`, `amba-pl011.c:2907`. | `T-REG`, `T-FW`. | `R-DIRECT`, `R-L2` | Register block metadata and firmware property population cover offset/access-width variation. |
| `PL011-06` | FIFO depth and AMBA revision fallback. | `amba-pl011.c:103`, `amba-pl011.c:121`, `amba-pl011.c:204`, `amba-pl011.c:2897`. | `T-SER`, `T-BUS`, `T-CONSTRAINTS`. | `R-L2`, `R-BE` | FIFO depth is Layer 2 serial data; AMBA runtime fallback is explicitly `IC-1`, not a new authored primitive. |
| `PL011-07` | Accepted serial semantic callback subset within Linux `uart_ops`. | `amba-pl011.c:2304`, `amba-pl011.c:2338`, `/Users/nyengele/driverbase/os/linux/include/linux/serial_core.h:375`. | `T-SER`. | `R-DIRECT`, `R-BE` | Core serial callbacks map to `Serial`/`SerialConsole`; the Linux table also contains target-specific admin, modem, break, throttle, and poll hooks accounted for in more specific rows below. |
| `PL011-08` | Port open/close session lifecycle. | `amba-pl011.c:1894`, `amba-pl011.c:2009`. | `T-SER`, `T-CLOCK`, `T-IRQ`, `T-DMA`, `T-PIN`. | `R-DIRECT`, `R-CM` | Serial `port_open`/`port_close` are nested device-class lifecycle, not `Lifecycle::init/deinit`. |
| `PL011-09` | Baud, termios, parity, stop bits, modem/flow configuration. | `amba-pl011.c:2057`, `amba-pl011.c:2090`, `amba-pl011.c:2229`. | `T-SER`, `T-REG`, `T-CLOCK`. | `R-DIRECT`, `R-RUST` | `Serial::configure` plus clock rate and register writes captures this; detailed divisor math is ordinary Rust. |
| `PL011-10` | PIO receive path and pushing bytes to TTY. | `amba-pl011.c:320`, `amba-pl011.c:1457`. | `T-SER`, `T-REG`, `T-CONSTRAINTS`. | `R-DIRECT`, `R-BE` | FIFO drain is ordinary Rust over registers; `push_rx_data` and overflow handling are accepted (`IC-6`). |
| `PL011-11` | PIO transmit path. | `amba-pl011.c:1490`, `amba-pl011.c:1504`. | `T-SER`, `T-REG`. | `R-DIRECT`, `R-RUST` | TX-ready predicates and byte writes map to Serial callbacks plus register access. |
| `PL011-12` | IRQ registration and interrupt demux/ack handler. | `amba-pl011.c:1829`, `amba-pl011.c:1591`. | `T-IRQ`, `T-REG`, `T-SYNC`, `T-SER`. | `R-CM`, `R-DIRECT` | Line registration is compiler-managed; device-local status/mask/ack remains Register Access in the ISR body. |
| `PL011-13` | DMA channel acquisition and slave configuration. | `amba-pl011.c:399`, `amba-pl011.c:416`, `amba-pl011.c:441`, `amba-pl011.c:529`. | `T-DMA`, `T-FW`. | `R-CM`, `R-L2` | Channel request/config derives from field declarations and Layer 2 DMA binding. |
| `PL011-14` | DMA residue capability test and PIO fallback. | `amba-pl011.c:475`. | `T-DMA`, `T-CONSTRAINTS`. | `R-TCAP`, `R-BE` | If target DMA cannot report residue at required granularity, the represented behavior is to reject/disable RX DMA and use represented PIO. |
| `PL011-15` | TX DMA mapping, prepare, submit, callback, fallback to PIO. | `amba-pl011.c:596`, `amba-pl011.c:637`, `amba-pl011.c:655`, `amba-pl011.c:718`. | `T-DMA`, `T-SER`, `T-REG`. | `R-DIRECT`, `R-BE`, `R-RUST` | Mapping/prepare/submit are accepted DMA operations. Linux descriptor callbacks are lowering-internal; fallback decision is ordinary Rust control flow. |
| `PL011-16` | RX DMA pause/status/residue/drain/retrigger. | `amba-pl011.c:814`, `amba-pl011.c:931`, `amba-pl011.c:979`. | `T-DMA`, `T-SER`, `T-CONSTRAINTS`. | `R-DIRECT`, `R-BE` | `IC-3` treats residue query and capability-gated pause/resume as resolved DMA surface plus backend sequencing obligation. Deeper DMA callback-model work remains separate from this row. |
| `PL011-17` | DMA RX poll timer. | `amba-pl011.c:1047`, `amba-pl011.c:1159`, `amba-pl011.c:1201`. | `T-TIMER`, `T-DMA`, `T-SER`, `T-PM`. | `R-DIRECT`, `R-CM` | Timer resource/handler/control are accepted. Timer/DMA PM interaction remains driver-authored and compiler-validated. |
| `PL011-18` | RS-485 half-duplex RTS timing and RX/TX gating. | `amba-pl011.c:1271`, `amba-pl011.c:1399`, `amba-pl011.c:2283`, `amba-pl011.c:2870`. | `T-SER`, `T-TIMER`, `T-REG`. | `R-RUST`, `R-BE`, `EXT-CANDIDATE` | Operational behavior is expressible with serial configuration, timers, register access, and state. A first-class portable RS-485/serial-flow construct remains a valid extension candidate. |
| `PL011-19` | Clock acquisition, enable, rate query, disable. | `amba-pl011.c:2893`, `amba-pl011.c:1780`, `amba-pl011.c:1784`, `amba-pl011.c:2028`. | `T-CLOCK`, `T-PM`. | `R-DIRECT`, `R-CM` | `Clock` wrapper methods and compiler-managed acquisition cover direct behavior; PM gating order is PM-owned. |
| `PL011-20` | Pinctrl default/sleep states. | `amba-pl011.c:1775`, `amba-pl011.c:2030`, `amba-pl011.c:2427`. | `T-PIN`, `T-PM`. | `R-DIRECT`, `R-CM` | Init acquisition/default state is PinCtrl; suspend/resume state transitions are PM lowering. |
| `PL011-21` | Reset line absence in this Linux driver. | No reset acquisition in probe path around `amba-pl011.c:2877`. | `T-RESET`. | `N/A` | Taxonomy has `ResetLine`; this source simply does not exercise it. Absence is not a gap. |
| `PL011-22` | Firmware properties for DMA polling, access width, baud. | `amba-pl011.c:510`, `amba-pl011.c:514`, `amba-pl011.c:2907`, `amba-pl011.c:2997`. | `T-FW`. | `R-CM`, `R-L2` | Property reads are generated init-preamble population, not driver-callable APIs. |
| `PL011-23` | Suspend/resume integration. | `amba-pl011.c:2944`, `amba-pl011.c:2954`, `amba-pl011.c:2965`. | `T-PM`, `T-CLOCK`, `T-PIN`, `T-IRQ`, `T-DMA`, `T-TIMER`. | `R-DIRECT`, `R-CM` | `PowerManagement` covers driver body; generated PM order covers Clock/PinCtrl/Interrupt; DMA/Timer remain driver-authored and compiler-validated. |
| `PL011-24` | Console, nbcon, and earlycon. | `amba-pl011.c:2406`, `amba-pl011.c:2519`, `amba-pl011.c:2551`, `amba-pl011.c:2607`, `amba-pl011.c:2623`, `amba-pl011.c:2701`, `amba-pl011.c:2713`, `amba-pl011.c:2738`. | `T-SER`, `T-REG`, `T-SYNC`, `T-CONSTRAINTS`. | `R-DIRECT`, `R-CM`, `R-BE` | `SerialConsole` captures `console_write`; earlycon and nbcon are accepted backend/compiler constraints (`IC-2`, `IC-4`). Debug poll console is separated below because it is not a first-class serial surface. |
| `PL011-25` | Locking across ISR, DMA callback, timer, console. | `amba-pl011.c:553`, `amba-pl011.c:1597`, `/Users/nyengele/driverbase/os/linux/include/linux/serial_core.h:605`, `/Users/nyengele/driverbase/os/linux/include/linux/serial_core.h:694`. | `T-SYNC`, `T-IRQ`, `T-TIMER`, `T-SER`. | `R-DIRECT`, `R-CM` | Handler bodies manage lock scopes explicitly; subsystem-serialized serial callbacks use context rules. |
| `PL011-26` | Logging, errors, polling, delays, ordering. | `amba-pl011.c:425`, `amba-pl011.c:436`, `amba-pl011.c:894`, `amba-pl011.c:1174`, `amba-pl011.c:1579`, `amba-pl011.c:1819`, `amba-pl011.c:2209`, `amba-pl011.c:3102`. | `T-LOG`, `T-DELAY`, `T-REG`, `T-CONSTRAINTS`. | `R-DIRECT`, `R-BE` | Logging macros and delay intrinsics are accepted; `cpu_relax` and MMIO barriers are Register Access lowering constraints (`IC-5`, `IC-8`). |
| `PL011-27` | Linux serial-core registration, global port table, DT alias line numbering, and last-port unregister. | `amba-pl011.c:2744`, `amba-pl011.c:2754`, `amba-pl011.c:2787`, `amba-pl011.c:2843`. | `T-SER`, `T-LIFE`, `T-FW`. | `R-CM`, `R-BE` | `uart_driver`, `uart_register_driver`, `uart_add_one_port`, `uart_remove_one_port`, `amba_ports[]`, alias-based line selection, and last-port unregister are Linux serial-subsystem glue generated from Serial/Lifecycle plus binding data. |
| `PL011-28` | Legacy platform-data init/exit hooks and board integration. | `amba-pl011.c:1798`, `amba-pl011.c:2032`, `amba-pl011.c:2435`. | `T-LIFE`, `T-BUS`, `T-FW`, `T-DMA`. | `R-BE`, `R-RUST` | Legacy `plat->init()`/`plat->exit()` and platform DMA filter data are target/board integration hooks. They are not a portable Regseq primitive; represented specs should prefer Layer 2 binding and backend glue, with opaque legacy callbacks treated as backend adaptation. |
| `PL011-29` | Runtime modem control/status and hardware flow-control events. | `amba-pl011.c:1448`, `amba-pl011.c:1549`, `amba-pl011.c:1649`, `amba-pl011.c:1672`, `amba-pl011.c:2175`. | `T-SER`, `T-REG`, `T-SYNC`. | `R-BE`, `R-RUST` | The accepted serial surface has flow-control configuration and Layer 2 capability facts, but Linux modem status callbacks and `delta_msr_wait` wiring are serial-backend glue plus register logic, not a separate portable trait. |
| `PL011-30` | Interrupt-enable sequencing, RX threshold drain workaround, ISR pass limit, and CTS-event workaround. | `amba-pl011.c:1574`, `amba-pl011.c:1594`, `amba-pl011.c:1841`, `amba-pl011.c:1851`. | `T-IRQ`, `T-REG`, `T-CONSTRAINTS`. | `R-BE`, `R-RUST` | IRQ line ownership remains Interrupt; startup mask/clear/drain quirks and ISR pass limits are device-specific register algorithms over accepted Register Access, with backend ordering/barrier constraints. |
| `PL011-31` | KGDB/debug poll-console get/put path and IRQ quiescing. | `amba-pl011.c:1713`, `amba-pl011.c:1736`, `amba-pl011.c:1755`, `amba-pl011.c:2322`, `amba-pl011.c:2658`, `amba-pl011.c:2670`. | `T-SER`, `T-REG`, `T-IRQ`, `T-CONSTRAINTS`. | `R-BE`, `R-RUST` | `CONFIG_CONSOLE_POLL` hooks are Linux debugger/console backend glue over register polling and interrupt quiescing. They are intentionally not claimed as direct `SerialConsole` surface. |
| `PL011-32` | Serial-core administrative callbacks and TIOCSSERIAL validation. | `amba-pl011.c:2249`, `amba-pl011.c:2259`, `amba-pl011.c:2268`, `amba-pl011.c:2304`. | `T-SER`, `T-BUS`, `T-REG`. | `R-BE`, `R-RUST` | `type`, `config_port`, and `verify_port` are Linux serial-core administrative glue over device identity and MMIO facts; they do not require a portable authored primitive. |
| `PL011-33` | RX break/SysRq handling and unlock-time SysRq check. | `amba-pl011.c:337`, `amba-pl011.c:361`, `amba-pl011.c:1626`, `amba-pl011.c:1694`. | `T-SER`, `T-REG`, `T-SYNC`. | `R-BE`, `R-RUST` | Break status and SysRq side effects are Linux serial-core delivery policy around represented RX/register/locking behavior; `break_ctl` writes remain ordinary register logic. |

## Linux E1000 Matrix

| ID | Linux feature | Source evidence | Taxonomy evidence | Route | Status / caveat |
|---|---|---|---|---|---|
| `E1000-01` | PCI driver/module registration. | `e1000_main.c:180`, `e1000_main.c:217`. | `T-LIFE`, `T-BUS`. | `R-CM`, `R-BE` | `pci_driver`, module init/exit, and device glue are generated from driver/bus declarations. |
| `E1000-02` | PCI IDs and device matching. | `e1000_main.c:22`, `e1000_hw.h:382`. | `T-LIFE`, `T-BUS`. | `R-L2`, `R-BE` | Vendor/device IDs are Layer 2 PCI bus metadata used by backend registration/matching. |
| `E1000-03` | PCI enable, region claim, bus mastering, BAR mapping. | `e1000_main.c:930`, `e1000_main.c:970`. | `T-BUS`, `T-REG`, `T-DMA`, `T-CONSTRAINTS`. | `R-CM`, `R-L2`, `R-BE` | PCI preamble belongs to Bus Resource; BAR-backed MMIO joins to Register Access. `IC-9` fixes ordering and rollback. |
| `E1000-04` | DMA mask setup. | `e1000_main.c:990`. | `T-DMA`, `T-BUS`. | `R-CM`, `R-L2` | `dma_mask` is Layer 2 capability data; compiler emits backend DMA-mask setup. |
| `E1000-05` | Netdev allocation, private data, register/unregister/free. | `e1000_main.c:952`, `e1000_main.c:1194`, `e1000_main.c:1262`. | `T-NET`, `T-LIFE`. | `R-CM` | Accepted `alloc_netdev` spans allocation/registration and deregistration/free. |
| `E1000-06` | Linux `net_device_ops`. | `e1000_main.c:821`. | `T-NET`. | `R-DIRECT`, `R-BE` | Core open/stop/xmit/rx-mode/MAC/stats/MTU map to `Network`; Linux table wiring is backend glue. |
| `E1000-07` | Open/close sequencing. | `e1000_main.c:1352`, `e1000_main.c:1432`. | `T-NET`, `T-DMA`, `T-IRQ`, `T-REG`. | `R-DIRECT`, `R-CM` | Network nested lifecycle coordinates represented DMA/IRQ/register operations. |
| `E1000-08` | Legacy shared IRQ request and ISR. | `e1000_main.c:250`, `e1000_main.c:3748`. | `T-IRQ`, `T-REG`, `T-WORK`, `T-NET`. | `R-CM`, `R-DIRECT` | Handler binding and shared-interrupt `NotHandled` semantics are accepted; line registration is compiler-managed. |
| `E1000-09` | NAPI poll/service loop. | `e1000_main.c:1009`, `e1000_main.c:3798`. | `T-NET`, `T-IRQ`, `T-DMA`. | `R-BE` | NAPI is Linux's backend realization of interrupt mitigation, packet delivery, and TX/RX completion. It should not become portable DSL surface. |
| `E1000-10` | Coherent TX/RX descriptor rings. | `e1000.h:143`, `e1000_main.c:1515`, `e1000_main.c:1707`. | `T-DMA`, `T-REG`. | `R-DIRECT`, `R-CM`, `R-RUST` | Ring memory is `DmaRegion<T>`; descriptor layout and ring arithmetic are ordinary Rust over represented DMA/register resources. |
| `E1000-11` | Streaming DMA for SKBs and fragments. | `e1000_main.c:2877`, `e1000_main.c:2929`, `e1000_main.c:1960`. | `T-DMA`, `T-NET`. | `R-DIRECT`, `R-BE` | `DmaMapping` captures map/sync/unmap; packet buffer plumbing is backend-specific network lowering. |
| `E1000-12` | Descriptor DMA ordering barriers. | `e1000_main.c:3019`, `e1000_main.c:3845`, `e1000_main.c:4547`. | `T-DMA`, `T-REG`, `T-CONSTRAINTS`. | `R-BE` | Descriptor and MMIO barriers are lowering obligations, not authored DSL calls; `IC-5` and `IC-10` cover MMIO and DMA descriptor barriers. |
| `E1000-13` | TX offload path: TSO, checksum, VLAN context descriptors. | `e1000_main.c:2694`, `e1000_main.c:2764`, `e1000_main.c:3099`. | `T-NET`, `T-DMA`, `T-REG`, `T-CONSTRAINTS`. | `R-DIRECT`, `R-RUST`, `R-BE` | `Network::start_xmit` plus packet metadata and register/DMA operations represent the path; `IC-11` requires checksum, TSO, and VLAN metadata in `NetPacket`. |
| `E1000-14` | TX queue stop/wake and packet completion accounting. | `e1000_main.c:3066`, `e1000_main.c:3880`. | `T-NET`, `T-DMA`, `T-ATOMIC`, `T-SYNC`, `T-CONSTRAINTS`. | `R-DIRECT`, `R-BE` | `flow_control` captures semantic queue control; `IC-14` covers per-packet completion as compiler/backend generated buffer cleanup. |
| `E1000-15` | RX delivery, checksum, VLAN, GRO/frags. | `e1000_main.c:3946`, `e1000_main.c:4001`, `e1000_main.c:4350`. | `T-NET`, `T-DMA`. | `R-DIRECT`, `R-BE` | `deliver_packet` lowers to Linux NAPI/GRO paths; checksum/VLAN metadata is packet metadata carried through Network/DMA. |
| `E1000-16` | Register macros and port-I/O fallback. | `e1000_osdep.h:28`, `e1000_hw.h:373`, `e1000_hw.c:436`, `e1000_main.c:930`, `e1000_main.c:4874`. | `T-REG`, `T-BUS`, `T-CONSTRAINTS`. | `R-DIRECT`, `R-BE` | MMIO registers map to `Mmio<T>`; `IC-12` requires the same Register Access surface to lower to port I/O when a PCI BAR is I/O-space. |
| `E1000-17` | PHY/MDIO/MII and link handling. | `e1000_hw.c:2785`, `e1000_hw.c:2922`, `e1000_ethtool.c:86`. | `T-REG`, `T-NET`, `T-DELAY`, `T-CONSTRAINTS`. | `R-RUST`, `R-BE` | PHY protocols are ordinary device algorithms over represented register/delay/network operations; `IC-13` and `IC-15` cover capability tables and PHY/MII infrastructure. |
| `E1000-18` | EEPROM/NVM and default MAC address. | `e1000_hw.c:3845`, `e1000_hw.c:3954`, `e1000_hw.c:4223`. | `T-REG`, `T-NET`. | `R-RUST`, `R-L2` | NVM access protocol is device logic over registers; `default_mac` is Layer 2/network binding when known. |
| `E1000-19` | Module parameters and option validation. | `e1000_param.c:21`, `e1000_param.c:35`, `e1000_param.c:243`. | `T-PARAM`. | `R-DIRECT`, `R-CM` | `#[rseq::param]` metadata and compiler-generated type/range validation cover this construct. |
| `E1000-20` | Workqueues, delayed work, watchdog, reset task. | `e1000.h:298`, `e1000_main.c:1106`, `e1000_main.c:2427`, `e1000_main.c:3505`. | `T-WORK`, `T-TIMER`, `T-NET`. | `R-DIRECT`, `R-CM`, `R-BE` | Explicit work maps to `Work`; Linux delayed work may lower as timer plus work depending target. |
| `E1000-21` | PM, suspend/resume/shutdown, Wake-on-LAN. | `e1000_main.c:178`, `e1000_main.c:5061`, `e1000_main.c:5155`. | `T-PM`, `T-NET`, `T-DMA`, `T-IRQ`, `T-REG`. | `R-DIRECT`, `R-CM`, `R-BE` | PM trait covers driver body; PCI power/wake calls are backend realization. WOL policy is PM/network metadata plus register logic. |
| `E1000-22` | PCI error recovery callbacks. | `e1000_main.c:172`, `e1000_main.c:5235`. | `T-LIFE`, `T-BUS`, `T-PM`, `T-BASE`. | `R-BE` | Linux AER callback shape is target-specific lifecycle/bus/PM recovery glue. The authoritative taxonomy does not record AER as an accepted extension candidate; current validation concentrates remaining risk in `IC-1` through `IC-15`. |
| `E1000-23` | Synchronization, atomics, bit flags. | `e1000_hw.c:85`, `e1000.h:188`, `e1000_main.c:3611`. | `T-SYNC`, `T-ATOMIC`. | `R-DIRECT`, `R-RUST` | Locks and atomics are accepted; higher-level bitflag state machines are ordinary Rust. |
| `E1000-24` | Logging and diagnostics. | `e1000.h:315`, `e1000_main.c:1202`. | `T-LOG`. | `R-DIRECT`, `R-BE` | Device-scoped Linux logging lowers from accepted logging macros with compiler-derived context. |
| `E1000-25` | Ettool management plane: link settings, rings, EEPROM, WOL, coalesce, tests, stats, and physical-ID LED. | `e1000_ethtool.c:1719`, `e1000_ethtool.c:1745`, `e1000_ethtool.c:1807`, `e1000_ethtool.c:1860`, `e1000_ethtool.c:1880`. | `T-NET`, `T-PM`, `T-DMA`, `T-REG`, `T-PARAM`, `T-CONSTRAINTS`. | `R-BE`, `R-RUST` | `IC-15` supports PHY/MII ethtool/media-control delegation. Ring, EEPROM, WOL, coalesce, stats, self-test, and LED identify are Linux management-plane glue plus ordinary driver logic over represented resources, not a portable `Ethtool` trait. |
| `E1000-26` | CE4100 secondary MDIO BAR and config-flash NVM path. | `e1000_main.c:1022`, `e1000_hw.h:785`, `e1000_hw.c:3861`. | `T-BUS`, `T-REG`, `T-NET`, `T-CONSTRAINTS`. | `R-L2`, `R-BE`, `R-RUST` | Secondary BAR/MDIO mapping and config-flash NVM access are device-model facts plus register-backed algorithms. `IC-15` covers PHY/MII infrastructure selection from device facts. |
| `E1000-27` | Manageability/ASF/BMC pass-through, ARP interception, SMBus capability, and management VLAN preservation. | `e1000_main.c:298`, `e1000_main.c:325`, `e1000_hw.c:482`, `e1000_hw.c:5496`. | `T-REG`, `T-NET`, `T-PM`, `T-CONSTRAINTS`. | `R-BE`, `R-RUST` | Manageability is device firmware/management policy expressed through registers and network VLAN state. It is accounted for as device-specific logic over represented resources and backend capability facts, not a separate portable management primitive. |
| `E1000-28` | PCI/PCI-X config-space tuning and bus errata: MWI, MMRBC, relaxed ordering, and DMA fairness. | `e1000_hw.c:384`, `e1000_hw.c:588`, `e1000_hw.c:593`, `e1000_main.c:4846`, `e1000_main.c:4862`. | `T-BUS`, `T-DMA`, `T-REG`, `T-CONSTRAINTS`. | `R-DIRECT`, `R-BE`, `R-RUST` | PCI config-space intrinsics cover authored config access where needed; MWI/MMRBC/ordering errata are backend/device-specific bus policy and ordinary register/config algorithms under PCI/DMA lowering. |
| `E1000-29` | Legacy MII ioctl management plane. | `e1000_main.c:829`, `e1000_main.c:4737`, `e1000_main.c:4755`. | `T-NET`, `T-REG`, `T-SYNC`, `T-CONSTRAINTS`. | `R-BE`, `R-RUST` | `SIOCGMIIPHY`, `SIOCGMIIREG`, and `SIOCSMIIREG` are Linux management-plane glue over represented PHY register access and locking; no portable ioctl surface is claimed. |
| `E1000-30` | Runtime netdev feature negotiation and offload toggling. | `e1000_main.c:784`, `e1000_main.c:798`, `e1000_main.c:1031`, `e1000_main.c:1827`. | `T-NET`, `T-REG`, `T-CONSTRAINTS`. | `R-BE`, `R-RUST` | `ndo_fix_features`/`ndo_set_features`, RXALL/RXFCS/HIGHDMA/IFF flags, and runtime offload toggles are Linux network-stack glue plus represented network metadata/register behavior. |
| `E1000-31` | Netpoll emergency polling path. | `e1000_main.c:833`, `e1000_main.c:5212`. | `T-NET`, `T-IRQ`, `T-REG`. | `R-BE`, `R-RUST` | Netpoll is Linux-specific emergency polling of the IRQ/service path over represented interrupt and register behavior. |

## Gap Ledger

No `GAP` row is currently justified for the core Linux PL011 or legacy Linux
E1000 driver semantics.

The following rows should remain visible because they constrain how strong the
representability claim can be:

| Item | Current conclusion | Why it is not a blocking taxonomy gap |
|---|---|---|
| PL011 RS-485 | `EXT-CANDIDATE` for first-class serial-flow modeling. | Existing behavior is expressible with Serial, Timer, Register Access, and ordinary Rust, but the taxonomy should not claim a dedicated RS-485 primitive yet. |
| PL011 DMA residue/progress query | `R-DIRECT` plus `R-BE` for Linux sequencing. | `IC-3` treats residue query and capability-gated pause/resume as resolved DMA surface plus backend-lowering obligation. Future work is deeper DMA callback/progress notification, not this stop/drain query path. |
| E1000 ethtool plane | `R-BE` / `R-RUST`, not a current extension candidate. | Core network data path is represented, `IC-15` covers ethtool/media-control delegation hooks where required, and network validation records no required network surface expansion. A future portable diagnostics trait would be new scope, not a repair to this matrix. |
| E1000 PCI error recovery | `R-BE`, not a current extension candidate. | The Linux AER callback table is not a portable driver semantic in the current taxonomy, but it can be backend-owned recovery glue over Lifecycle/Bus/PM. The authoritative collateral does not list it as a deferred extension. |

## Verification Metrics

Second-pass verification used four targeted read-only verifier agents:

| Verification axis | Device | Blocking gaps | Important findings integrated | Minor findings integrated |
|---|---|---:|---:|---:|
| Source-feature completeness | PL011 | 0 | 4 | 3 |
| Taxonomy-evidence validity | PL011 | 0 | 3 | 2 |
| Source-feature completeness | E1000 | 0 | 4 | 3 |
| Taxonomy-evidence validity | E1000 | 0 | 2 | 1 |

Integrated matrix changes from this pass:

- added explicit PL011 rows for serial-core registration, platform-data hooks,
  modem status/control, IRQ-enable quirks, debug poll console, administrative
  callbacks, and SysRq/break behavior
- narrowed PL011 rows whose earlier wording overclaimed direct DSL coverage
  for the full Linux `uart_ops` table, vendor quirks, poll console, and reset
  absence
- added explicit E1000 rows for CE4100 secondary BAR/config flash,
  manageability/ASF, PCI-X/MWI bus errata, MII ioctl, runtime feature toggles,
  and netpoll
- tightened E1000 taxonomy evidence for port I/O, AER, and ethtool so the
  matrix does not claim unsupported extension candidates

## Validation Boundaries

This matrix supports a taxonomy-level representability claim. It does not prove:

- generated Linux C is behaviorally equivalent to the reference drivers,
- Zephyr lowering has the same feature availability as Linux,
- every optional Linux management-plane callback should be included in the
  portable DSL surface,
- the current compiler implementation already extracts all listed surfaces.

Those are separate compiler/backend validation tasks. The result here is that
the accepted taxonomy supplies a defensible semantic home for the audited Linux
features, with the caveats above kept explicit rather than hidden.
