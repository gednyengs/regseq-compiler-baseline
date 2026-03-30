# Open Issues

Carry-forward items for compiler/DSL work. None are blockers; all are
items to watch during implementation.

## Accepted-with-caution constructs

### Synchronization

Two declarable properties (lock_type, protected_data_type) are
expressed through the Rust type system (generic parameters), not
through #[rseq::...] attributes. The compiler must treat the type
choice as a specification decision that drives code generation
(spin_lock_irqsave vs mutex_lock vs k_sem_take). Implementation
should verify that the type-based property model works correctly with
the compiler's property extraction pipeline.

### Diagnostic Logging

Only 1 declared_here property (debug_logging). The auto-injected
device context is compiler_derived from Driver Lifecycle's driver_name.
The compiler must generate LOG_MODULE_REGISTER on Zephyr automatically
from the driver name. Implementation should verify that the device
context injection works correctly across all 4 OSes.

### Parameter Registration

Declarative construct: the driver writes #[rseq::param] attributes and
the compiler generates OS-specific registration code. There is no
explicit runtime invocation by the driver. The compiler must handle the
full load-time declaration (module_param) and, for writable params, the
runtime registration node (debugfs, SYSCTL_ADD_*). Implementation
should verify that both paths are generated correctly.

### Firmware Property Access

Same declarative pattern as Parameter Registration. The compiler
generates OS-specific property extraction code from #[rseq::property]
attributes. No explicit driver invocation. Implementation should
verify that the extraction code is generated in the init preamble
and handles all property types (u32, bool, String, Option, enum).

### Timed Delay

Zero attribute-declared properties. The operation selection (busy_wait
vs sleep) and duration values are specification decisions expressed as
call-site choices. The compiler must:
- select the appropriate OS-specific delay function
- warn on sleep in ISR context
- warn on busy_wait for long durations in thread context

### Deferred Work

Only 1 declared_here property (work_handler_binding), expressed via
#[rseq::work_handler] method attribute. The compiler must generate
the OS-specific work item initialization, handler wrapper, and
cancel/flush in the deinit postamble.

## Rejected construct

### Capability Extraction (subsumed by Register Access)

Concern [45] was rejected in Step 9A (fails check 2 and check 5).
Capability flag extraction is handled through Register Access: the
driver reads capability registers using self.regs.hcc.read(...) and
extracts bits using the register_bitfields! definitions. No separate
compiler support is needed. The register_block definition is the
mechanism for expressing which capability bits exist.

## Cross-construct property overlap

The `compatible` and `driver_name` properties are declared by Driver
Lifecycle (#[rseq::driver]) and consumed by Bus Resource Management
for probe code generation. The compiler should resolve this through
a single declaration site; Bus Resource Management reads the values
from the driver declaration, not from a separate source.

## Track B deferred items

5 singletons are deferred from Track A:
- [38] Device Event Notification (deferred to Track B)
- [12] Kernel Memory Allocation (compiler-internal)
- [26] Device Identity (compiler-generated)
- [27a] Startup Registration (compiler-generated)
- [27d] Hardware Init Callbacks (lifecycle-internal)

These should not affect PL011 or E1000 compiler work. For E1000,
device event notification (link state changes) is handled through
the Network Interface Management construct's set_link_state intrinsic.
