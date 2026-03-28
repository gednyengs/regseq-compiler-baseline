# Device Coverage

How the validated Track A construct set covers PL011 and E1000.

## PL011

**Total elements**: 403

**Capture summary:**

| Category | Count | Pct |
|----------|-------|-----|
| driver_facing_trait_operation | 246 | 61.0% |
| driver_invoked_intrinsic | 41 | 10.2% |
| compiler_managed_operation | 15 | 3.7% |
| language_passthrough | 27 | 6.7% |
| utility_or_compiler_internal | 64 | 15.9% |
| deferred_out_of_scope | 10 | 2.5% |

**Constructs used:**

| Construct | Elements | Caution |
|-----------|----------|---------|
| Synchronization | 65 | yes |
| Serial/UART Management | 48 |  |
| DMA Management | 39 |  |
| Diagnostic Logging | 25 | yes |
| Register Access | 24 |  |
| Driver Lifecycle | 22 |  |
| Peripheral clock management | 15 |  |
| Kernel timer lifecycle management | 13 |  |
| Firmware property access | 11 | yes |
| Device interrupt setup and handling | 11 |  |
| Timed CPU delay and sleep | 10 | yes |
| Device power state management | 9 |  |
| Atomic memory access and ordering | 5 |  |
| Pin controller and GPIO configuration | 3 |  |
| Bus Resource Management | 1 |  |
| Hardware reset sequencing and post-reset polling | 1 |  |

**Notable driver-facing operations:**

- Atomic memory access and ordering
- DMA Management
- Device interrupt setup and handling
- Device power state management
- Driver Lifecycle
- Hardware reset sequencing and post-reset polling
- Kernel timer lifecycle management
- Peripheral clock management
- Pin controller and GPIO configuration
- Register Access
- Serial/UART Management
- Synchronization

**Notable intrinsics:**

- DMA Management
- Diagnostic Logging
- Timed CPU delay and sleep

**Notable compiler-managed operations:**

- Bus Resource Management
- Firmware property access
- Register Access

**Deferred / out-of-scope**: 10 elements across:

- Kernel Memory Allocation `[12]`
- Device Identity `[26]`
- Startup Registration `[27a]`
- Hardware Init Callbacks `[27d]`
- Device Event Notification `[38]`

**Accepted-with-caution constructs affecting this device:**

- **Diagnostic Logging**: Property-light: 1 declared_here property (debug_logging). Device context is compiler_derived from Driver Lifecycle.
- **Firmware property access**: Declarative construct: the driver writes #[rseq::property] attributes; the compiler generates OS-specific property extraction code (device_property_read_u32, DT_INST_PROP_OR, OF_getencprop). No explicit runtime invocation.
- **Synchronization**: Type-system-expressed properties (lock_type, protected_data_type). These pass the 4-criterion test but are expressed through Rust generic parameters, not #[rseq::...] attributes.
- **Timed CPU delay and sleep**: Zero attribute-declared properties. Check 2 passes on the argument that operation selection and hardware-mandated duration values constitute specification decisions, even though they are expressed as call-site choices rather than static attributes.

---

## E1000

**Total elements**: 2384

**Capture summary:**

| Category | Count | Pct |
|----------|-------|-----|
| driver_facing_trait_operation | 1243 | 52.1% |
| driver_invoked_intrinsic | 289 | 12.1% |
| compiler_managed_operation | 233 | 9.8% |
| language_passthrough | 117 | 4.9% |
| utility_or_compiler_internal | 409 | 17.2% |
| deferred_out_of_scope | 93 | 3.9% |

**Constructs used:**

| Construct | Elements | Caution |
|-----------|----------|---------|
| Register Access | 623 |  |
| Network Interface Management | 207 |  |
| Synchronization | 157 | yes |
| Parameter Registration | 148 | yes |
| Driver Lifecycle | 131 |  |
| Diagnostic Logging | 117 | yes |
| Bus Resource Management | 104 |  |
| Timed CPU delay and sleep | 96 | yes |
| DMA Management | 64 |  |
| Atomic memory access and ordering | 35 |  |
| Hardware reset sequencing and post-reset polling | 24 |  |
| Device power state management | 21 |  |
| Device interrupt setup and handling | 19 |  |
| Deferred work item scheduling | 19 | yes |

**Notable driver-facing operations:**

- Atomic memory access and ordering
- DMA Management
- Deferred work item scheduling
- Device interrupt setup and handling
- Device power state management
- Driver Lifecycle
- Hardware reset sequencing and post-reset polling
- Network Interface Management
- Register Access
- Synchronization

**Notable intrinsics:**

- Bus Resource Management
- DMA Management
- Diagnostic Logging
- Register Access
- Timed CPU delay and sleep

**Notable compiler-managed operations:**

- Bus Resource Management
- Parameter Registration
- Register Access

**Deferred / out-of-scope**: 93 elements across:

- Kernel Memory Allocation `[12]`
- Device Identity `[26]`
- Startup Registration `[27a]`
- Hardware Init Callbacks `[27d]`
- Device Event Notification `[38]`

**Accepted-with-caution constructs affecting this device:**

- **Deferred work item scheduling**: Property-light construct.
- **Diagnostic Logging**: Property-light: 1 declared_here property (debug_logging). Device context is compiler_derived from Driver Lifecycle.
- **Parameter Registration**: Declarative construct: the driver writes #[rseq::param] attributes; the compiler generates module_param/SYSCTL registration code. No explicit runtime invocation by the driver.
- **Synchronization**: Type-system-expressed properties (lock_type, protected_data_type). These pass the 4-criterion test but are expressed through Rust generic parameters, not #[rseq::...] attributes.
- **Timed CPU delay and sleep**: Zero attribute-declared properties. Check 2 passes on the argument that operation selection and hardware-mandated duration values constitute specification decisions, even though they are expressed as call-site choices rather than static attributes.

---
