# Compiler Knowledge Reference

This document catalogs the specific knowledge the Regseq compiler carries
about known types, intrinsics, and lowering patterns. It complements
[spec.md](spec.md) (model structure) and [ir-reference.md](ir-reference.md)
(IR constructs) with the concrete details the compiler needs to recognize,
extract, and lower domain-specific constructs.

Each entry documents what the compiler knows at build time -- not what the
IR looks like structurally, but what the compiler does with it.

---

## 1. Framework Types

Framework types are known types with known C lowering, identified by
`#[rseq::framework_type("...")]` and classified as
`Framework(FrameworkTypeKind)` in the model (spec.md Section 3.4). The
compiler knows their layout, constructors, and per-backend lowering.

### Duration

**Classification:** `Framework(FrameworkTypeKind::Duration)`

**Rust definition:**
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[rseq::framework_type("duration")]
pub struct Duration {
    us: u64,
}
```

**Layout:** Single `u64` field, value in microseconds.

**Constructors (all `const fn`, evaluated by rustc before extraction):**

| Constructor | Semantics |
|---|---|
| `Duration::from_us(n)` | `n` microseconds |
| `Duration::from_ms(n)` | `n * 1_000` microseconds |
| `Duration::from_s(n)` | `n * 1_000_000` microseconds |

By the time the Regseq compiler sees the MIR, these are fully evaluated
constant values. The compiler does not need to recognize the constructors
as intrinsics.

**Backend lowering:**

| Target | C representation | Conversion |
|---|---|---|
| Linux | `unsigned long` (jiffies) or `u64` (raw) | `usecs_to_jiffies(us)` for sleep, raw microseconds for `udelay()` |
| Zephyr | `k_timeout_t` | `K_USEC(us)` |
| Bare-metal | `uint64_t` | Raw microseconds, backend determines delay mechanism |

**Used by intrinsics:** `ResetToggle` (pulse duration), `BusyWait` (delay),
`Sleep` (delay), `PollUntil` (timeout and poll interval).

### RegseqError

**Classification:** `Framework(FrameworkTypeKind::RegseqError)`

**Semantics:** The shared compiler-facing semantic error space for authored
fallible Regseq code. It is NOT a Rust runtime error framework, NOT a
wrapper around OS-native errors, and NOT a logging/diagnostic type.

**Rust definition (provisional Phase-B variant set):**

```rust
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[rseq::framework_type("regseq-error")]
pub enum RegseqError {
    InvalidParameter,
    OutOfMemory,
    HardwareError,
    Timeout,
    Unsupported,
    DeviceBusy,
    PermissionDenied,
    DeviceNotFound,
    GenericFailure,
}
```

Does NOT implement `std::error::Error` or `Display`. The carrier is
`core::result::Result<T, RegseqError>` -- no custom `RegseqResult<T>`.

**IR representation:**

RegseqError is captured in the IR through standard mechanisms -- it is
a `Defined(TypeDefId)` with `Framework(RegseqError)` classification. No
special IR-level type machinery is needed.

In the `RegseqType` system, a `Result<(), RegseqError>` return type is:
```
Result(Unit, Defined(regseq_error_type_def_id))
```

Value operations use standard enum IR constructs:
- **Creation** (`return Err(RegseqError::Timeout)`): `SetDiscriminant`
  to set the variant tag, `Aggregate` to construct the `Result::Err`.
- **Matching** (`match result { Err(RegseqError::Timeout) => ... }`):
  `Discriminant(place)` + `SwitchInt` on the variant index.
- **Propagation** (`clock.enable()?`): the `IntrinsicCall` for the
  fallible intrinsic has `error_target: Some(err_bb)`. The `?` pattern
  is folded into the `error_target` edge. The error path contains the
  `Drop` cascade for live locals (from Rust's drop elaboration).

The `Framework(RegseqError)` classification is the signal that tells
the backend to apply special lowering -- it is not a structural
distinction in the IR.

**The boundary rule:**

Authored code produces `RegseqError`. Compiler-generated code (pre-init,
teardown) produces native OS errors directly. They meet at trait callback
exits (e.g., `Lifecycle::init`, `Serial::port_open`), where the backend
converts `RegseqError` to native form.

- Inside authored helpers and callbacks, `RegseqError` flows as an
  abstract semantic value. Helpers can create, propagate (`?`), match,
  and reclassify errors freely. No backend materialization happens at
  these sites.
- At trait callback exits (the outermost authored fallible boundary),
  the backend emits the conversion to native error codes.
- Compiler-generated preamble/postamble failures (resource acquisition,
  MMIO mapping, clock get, etc.) use target-native error codes directly.
  These never flow through `RegseqError`. This preserves backend-specific
  semantics (e.g., Linux `-EPROBE_DEFER` for deferred probing must not
  be collapsed into a generic `RegseqError` variant).

**Fallible intrinsics and error materialization:**

When a fallible intrinsic (e.g., `ClockEnable`) fails, its IR return
type is `Result<(), RegseqError>`. The `?` branches on Ok/Err.

In the generated C, the backend can optimize the common case: if the
error is only propagated via `?` to the callback exit (no matching on
specific variants), the backend short-circuits to native error
propagation without materializing a `RegseqError` value:

```c
ret = clk_prepare_enable(clk);
if (ret)
    goto err_cleanup;   // native errno flows directly
```

If the author matches on specific variants:
```rust
match self.clock.enable() {
    Err(RegseqError::Timeout) => { /* retry */ },
    Err(e) => return Err(e),
    Ok(()) => {},
}
```

Then the backend must convert the native error to a `RegseqError`
discriminant so the `SwitchInt` in the IR can branch correctly.
This conversion is backend-generated code, not represented in the IR.
The backend knows to do this because the type is classified as
`Framework(RegseqError)`.

**Per-variant native error mapping (provisional):**

| RegseqError variant | Linux | Zephyr | Windows |
|---|---|---|---|
| `InvalidParameter` | `-EINVAL` | `-EINVAL` | `STATUS_INVALID_PARAMETER` |
| `OutOfMemory` | `-ENOMEM` | `-ENOMEM` | `STATUS_INSUFFICIENT_RESOURCES` |
| `HardwareError` | `-EIO` | `-EIO` | `STATUS_IO_DEVICE_ERROR` |
| `Timeout` | `-ETIMEDOUT` | `-ETIMEDOUT` | `STATUS_TIMEOUT` |
| `Unsupported` | `-EOPNOTSUPP` | `-ENOTSUP` | `STATUS_NOT_SUPPORTED` |
| `DeviceBusy` | `-EBUSY` | `-EBUSY` | `STATUS_DEVICE_BUSY` |
| `PermissionDenied` | `-EACCES` | `-EACCES` | `STATUS_ACCESS_DENIED` |
| `DeviceNotFound` | `-ENODEV` | `-ENODEV` | `STATUS_NO_SUCH_DEVICE` |
| `GenericFailure` | `-EFAULT` | `-EFAULT` | `STATUS_UNSUCCESSFUL` |

**Deferred work:**

- **Intrinsic-local error types** (`ClockError`, `PollError`,
  `ResetError`, etc.) with `From<X> for RegseqError` conversions.
  These would refine the error space per intrinsic family without
  changing RegseqError itself.
- **Final variant taxonomy** -- the 9 variants are provisional. Some
  names may be normalized. `GenericFailure` is acknowledged as
  conceptually weak.
- **Error category validation** -- whether the compiler should enforce
  that specific intrinsics can only produce certain error variants.

### SerialErrors

**Classification:** `Framework(FrameworkTypeKind::SerialErrors)`

**Semantics:** Bitflags for serial receive error conditions. Used in
`RxChar` to report per-character errors detected during reception.

**Rust definition:**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[rseq::framework_type("serial-errors")]
pub struct SerialErrors {
    bits: u8,
}

impl SerialErrors {
    pub const NONE: Self = Self { bits: 0 };
    pub const OVERRUN: Self = Self { bits: 1 << 0 };
    pub const PARITY: Self = Self { bits: 1 << 1 };
    pub const FRAMING: Self = Self { bits: 1 << 2 };
    pub const BREAK: Self = Self { bits: 1 << 3 };

    pub const fn empty() -> Self { Self::NONE }
    pub const fn bits(self) -> u8 { self.bits }
    pub const fn contains(self, other: Self) -> bool { ... }
    pub const fn is_empty(self) -> bool { ... }
}

impl BitOr for SerialErrors { ... }
```

Bitflag type with `BitOr` support for combining flags. The spec author
constructs error flags with `SerialErrors::OVERRUN | SerialErrors::PARITY`
and tests with `contains()` or bitwise AND.

**IR representation:** Standard `Defined(TypeDefId)` with
`Framework(SerialErrors)` classification. Bitwise operations on the
inner `u8` appear as standard `BinaryOp(BitOr, ...)` / `BinaryOp(BitAnd, ...)`
in the IR. The `const fn` methods (`empty`, `bits`, `contains`,
`is_empty`) are trivial -- const-evaluated by rustc or inlined as field
access / bitwise ops. No special compiler treatment needed.

**Backend lowering:** Integer bitfield (`uint8_t`). Bit assignments match
the Rust definition (OVERRUN=1, PARITY=2, FRAMING=4, BREAK=8). Backends
may map to OS-specific serial error constants if the target framework
defines them.

### RxChar

**Classification:** `Framework(FrameworkTypeKind::RxChar)`

**Semantics:** A received character with associated error flags. Returned by
`Serial::rx_char()`.

**Rust definition:**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[rseq::framework_type("rx-char")]
pub struct RxChar {
    byte: u8,
    errors: SerialErrors,
}

impl RxChar {
    pub const fn new(byte: u8, errors: SerialErrors) -> Self { ... }
    pub const fn byte(self) -> u8 { self.byte }
    pub const fn errors(self) -> SerialErrors { self.errors }
}
```

**IR representation:** Standard `Defined(TypeDefId)` with
`Framework(RxChar)` classification. The constructor `new()` and accessors
`byte()` / `errors()` are `const fn` -- usable at compile time for
static values, but also callable at runtime with dynamic values (e.g.,
bytes read from a device register). In MIR, runtime calls to `new()`
appear as struct aggregate construction; `byte()` and `errors()` appear
as field access. No special compiler treatment needed. No intrinsics.

**Backend lowering:** Two-field struct. Layout:

```c
struct rx_char {
    uint8_t byte;
    uint8_t errors;  // SerialErrors bitfield
};
```

### SerialConfig

**Classification:** `Framework(FrameworkTypeKind::SerialConfig)`

**Semantics:** Serial port configuration covering baud rate, framing, and
flow control. Passed to `Serial::configure()`.

**Rust definition:**

```rust
#[rseq::framework_type("serial-config")]
pub struct SerialConfig {
    pub baud_rate: u32,
    pub data_bits: DataBits,
    pub parity: Parity,
    pub stop_bits: StopBits,
    pub flow_control: FlowControl,
}
```

Plain data struct with public fields. No methods, no intrinsics. The
spec author reads fields via standard field projection (`config.baud_rate`,
`config.data_bits`, etc.).

**IR representation:** Standard `Defined(TypeDefId)` with
`Framework(SerialConfig)` classification. Field access is standard `Field`
projection in the IR. No special compiler treatment.

**Backend lowering:** Backend decomposes the struct fields into the
target's configuration mechanism. May map to an OS-specific config struct
(e.g., Linux `struct ktermios`) or decompose into individual register
writes.

### DataBits

**Classification:** `Framework(FrameworkTypeKind::DataBits)`

**Semantics:** Number of data bits per serial frame.

**Rust definition:**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[rseq::framework_type("data-bits")]
pub enum DataBits {
    Five,
    Six,
    Seven,
    Eight,
    Nine,
}
```

**IR representation:** Standard `Defined(TypeDefId)` with
`Framework(DataBits)` classification. Matching on variants is standard
`Discriminant` + `SwitchInt`.

**Backend lowering:** The compiler knows the variant semantics. Backends
map to target-specific values:

| Variant | Linux (termios) | Register (PL011 LCR_H WLEN) |
|---|---|---|
| `Five` | `CS5` | `0b00` |
| `Six` | `CS6` | `0b01` |
| `Seven` | `CS7` | `0b10` |
| `Eight` | `CS8` | `0b11` |
| `Nine` | Not standard | Device-specific |

### Parity

**Classification:** `Framework(FrameworkTypeKind::Parity)`

**Semantics:** Parity mode for serial framing.

**Rust definition:**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[rseq::framework_type("parity")]
pub enum Parity {
    None,
    Even,
    Odd,
}
```

**IR representation:** Standard `Defined(TypeDefId)` with
`Framework(Parity)` classification.

**Backend lowering:**

| Variant | Linux (termios) | Register (PL011 LCR_H) |
|---|---|---|
| `None` | clear `PARENB` | PEN=0 |
| `Even` | `PARENB`, `PARODD` clear | PEN=1, EPS=1 |
| `Odd` | `PARENB \| PARODD` | PEN=1, EPS=0 |

### StopBits

**Classification:** `Framework(FrameworkTypeKind::StopBits)`

**Semantics:** Number of stop bits per serial frame.

**Rust definition:**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[rseq::framework_type("stop-bits")]
pub enum StopBits {
    One,
    Two,
}
```

**IR representation:** Standard `Defined(TypeDefId)` with
`Framework(StopBits)` classification.

**Backend lowering:**

| Variant | Linux (termios) | Register (PL011 LCR_H STP2) |
|---|---|---|
| `One` | clear `CSTOPB` | `0` |
| `Two` | `CSTOPB` | `1` |

### FlowControl

**Classification:** `Framework(FrameworkTypeKind::FlowControl)`

**Semantics:** Hardware flow-control mode.

**Rust definition:**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[rseq::framework_type("flow-control")]
pub enum FlowControl {
    None,
    RtsCts,
    DtrDsr,
}
```

**IR representation:** Standard `Defined(TypeDefId)` with
`Framework(FlowControl)` classification.

**Backend lowering:**

| Variant | Linux (termios) | Register (PL011 CR) |
|---|---|---|
| `None` | clear `CRTSCTS` | CTSEN=0, RTSEN=0 |
| `RtsCts` | `CRTSCTS` | CTSEN=1, RTSEN=1 |
| `DtrDsr` | `CDTRDSR` (if supported) | DTR/DSR modem lines |

### IrqResult

**Classification:** `Framework(FrameworkTypeKind::IrqResult)`

**Semantics:** Return value from ISR handlers. Indicates whether the
interrupt was handled.

**Variants:**
- `Handled` -- the interrupt was serviced
- `NotHandled` -- the interrupt was not for this device

**Backend lowering:**

| Target | C representation |
|---|---|
| Linux | `irqreturn_t` (`IRQ_HANDLED` / `IRQ_NONE`) |
| Zephyr | Backend-determined |

---

## 2. Abstraction Types

Abstraction types are OS-managed resource wrappers, identified by
`#[rseq::abstraction("...")]` and classified as
`Abstraction(RegseqAbstraction)` in the model. They are represented as
`Defined(TypeDefId)` in the IR type system. What makes them special:
(1) the `Abstraction` classification tells the compiler and backend
they have domain-specific semantics, (2) their instances on the driver
struct carry resource properties, (3) the backend lowers them to
platform-specific handle types, and (4) they carry
`#[rseq::intrinsic("...")]` methods that the compiler recognizes as
domain operations directly in MIR -- no wrapper indirection.

Each abstraction type's intrinsic methods are marked with
`#[rseq::intrinsic("canonical_name")]`. The compiler recognizes them
at the call site in MIR as a single `Call` terminator and replaces
them with `IntrinsicCall` during IR construction.

### Clock

**Abstraction:** `RegseqAbstraction::Clock`

**Wrapper methods and their intrinsics:**

| Method | Intrinsic | Returns | Fallible? |
|---|---|---|---|
| `enable()` | `clock_enable` | `Result<(), RegseqError>` | Yes |
| `disable()` | `clock_disable` | `()` | No |
| `get_rate()` | `clock_get_rate` | `u32` | No |

**Identity resolution:** `&Clock` argument traced to driver struct field
with `#[rseq::clock(id = "...")]`.

**Backend lowering examples:**

| Intrinsic | Linux | Zephyr |
|---|---|---|
| `clock_enable` | `clk_prepare_enable(clk)` | `clock_control_on(dev, subsys)` |
| `clock_disable` | `clk_disable_unprepare(clk)` | `clock_control_off(dev, subsys)` |
| `clock_get_rate` | `clk_get_rate(clk)` | `clock_control_get_rate(dev, subsys, &rate)` |

### ResetLine

**Abstraction:** `RegseqAbstraction::Reset`

**Wrapper methods and their intrinsics:**

| Method | Intrinsic | Returns | Fallible? |
|---|---|---|---|
| `assert()` | `reset_assert` | `Result<(), RegseqError>` | Yes |
| `deassert()` | `reset_deassert` | `Result<(), RegseqError>` | Yes |
| `toggle(duration)` | `reset_toggle` | `Result<(), RegseqError>` | Yes |

**Identity resolution:** `&ResetLine` argument traced to driver struct field.

**Backend lowering examples:**

| Intrinsic | Linux |
|---|---|
| `reset_assert` | `reset_control_assert(rst)` |
| `reset_deassert` | `reset_control_deassert(rst)` |
| `reset_toggle` | `reset_control_reset(rst)` |

### PinCtrl

**Abstraction:** `RegseqAbstraction::PinCtrl`

**Rust definition:**

```rust
#[rseq::abstraction("pinctrl")]
pub struct PinCtrl {
    _opaque: (),
}

pub trait PinState {}

impl PinCtrl {
    pub fn apply<S: PinState>(&self) -> Result<(), RegseqError> {
        pinctrl_apply::<S>(self)
    }
}
```

Opaque type, no public constructor. Single operation: `apply::<S>()`
selects a pin configuration described by the marker type `S`.

**Wrapper methods and their intrinsics:**

| Method | Intrinsic | Returns | Fallible? |
|---|---|---|---|
| `apply::<S>()` | `pinctrl_apply` | `Result<(), RegseqError>` | Yes |

**Pin state marker types and the Layer 1 / Layer 2 bridge:**

Pin states are zero-sized marker types defined by the spec author,
implementing `PinState`. Each carries a `#[rseq::pin_state(id = "...")]`
attribute that provides the logical state name -- the bridge between
the Rust type and the platform file.

```rust
#[rseq::pin_state(id = "default")]
pub struct DefaultState;
impl PinState for DefaultState {}

#[rseq::pin_state(id = "sleep")]
pub struct SleepState;
impl PinState for SleepState {}
```

The `id` is a spec-level logical name. The platform file declares which
logical pin states are valid for a given deployment:

```toml
[pinctrl.uart_pins]
pin_states = ["default", "sleep"]
```

This gives two levels of indirection:

```
Rust type (DefaultState)
    -> logical id ("default")          via #[rseq::pin_state(id = "...")]
    -> validated against platform file (must appear in pin_states list)
```

The backend is responsible for lowering the logical id to the
target-native API call. That translation lives in the code generator,
not the platform file.

**Identity resolution:**

1. `&PinCtrl` receiver preserved as the first arg in
   `IntrinsicCall.args`. Its Place expression identifies the resource
   instance (direct struct field, function parameter, etc.).
2. `S` type parameter after monomorphization -> concrete type (e.g.,
   `DefaultState`)
3. Compiler looks up the `TypeDefId` for `DefaultState`, reads
   `attrs.pin_state.id` from the type definition's `Attrs` bag ->
   logical id `"default"` (carried as `Symbol` in the IR via
   `IntrinsicMetadata::PinCtrl { state_id }`)
4. When the receiver Place resolves to a specific driver struct field
   (direct access), the compiler validates the logical id exists in
   the platform file's `pin_states` for that PinCtrl resource. When
   the receiver is a function parameter, validation is deferred to the
   call site.

The `PinState` trait stays a bare marker (no associated constants).
All identity flows through the `#[rseq::pin_state(id = "...")]`
attribute, consistent with how other Regseq constructs use `#[rseq::]`
attributes for identity (`#[rseq::clock(id = "...")]`,
`#[rseq::mmio(id = "...")]`, etc.).

**Backend lowering examples:**

| Logical id | Linux | Zephyr |
|---|---|---|
| `"default"` | `pinctrl_pm_select_default_state(dev)` | `pinctrl_apply_state(dev, PINCTRL_STATE_DEFAULT)` |
| `"sleep"` | `pinctrl_pm_select_sleep_state(dev)` | `pinctrl_apply_state(dev, PINCTRL_STATE_SLEEP)` |
| `"idle"` | `pinctrl_pm_select_idle_state(dev)` | Backend-determined |

### Interrupt

**Abstraction:** `RegseqAbstraction::Interrupt`

**Lifecycle:** Managed by the compiler in pre-init/teardown contexts. The
spec author declares the interrupt resource and the ISR handler; the
compiler generates the request/free calls.

**Backend lowering examples:**

| Operation | Linux |
|---|---|
| Request (pre-init) | `request_irq(irq, handler, flags, name, dev)` |
| Free (teardown) | `free_irq(irq, dev)` |

### Mmio (transport)

**Abstraction:** `RegseqAbstraction::Mmio`

**Role:** Transport wrapper for memory-mapped register access. `Mmio<T>`
is the first concrete transport kind in the Regseq framework. It
implements the `RegisterTransport` trait, enabling register access
intrinsics to operate through it.

**Rust definition:**

```rust
#[rseq::abstraction("mmio")]
#[repr(transparent)]
pub struct Mmio<T> {
    inner: T,
}

impl<T> RegisterTransport for Mmio<T> {
    type Block = T;
}

impl<T> Deref for Mmio<T> {
    type Target = T;
    fn deref(&self) -> &T { &self.inner }
}
```

`Mmio<T>` is generic over the register block type `T` (e.g.,
`Mmio<pl011::Regs>`). It implements `Deref<Target=T>` for layout
inspection, but register access uses methods directly on `Mmio<T>`,
not through the inner block's field types.

**Register access intrinsics on `Mmio<T>`:**

Register access intrinsics are methods on `Mmio<T>`, each marked
with `#[rseq::intrinsic("...")]`. See spec.md Section 10.2 for the
full architecture.

```rust
impl<Block> Mmio<Block> {
    #[rseq::intrinsic("mmio_read")]
    pub fn read<W: RegWidth, R: RegisterOf<Block>>(
        &self, reg: RegisterId<W, R>,
    ) -> W
    where R::Access: ReadAccess { ... }

    #[rseq::intrinsic("mmio_read_field")]
    pub fn read_field<W: RegWidth, R: RegisterOf<Block>>(
        &self, field: Field<W, R>,
    ) -> W
    where R::Access: ReadAccess { ... }

    #[rseq::intrinsic("mmio_write")]
    pub fn write<W: RegWidth, R: RegisterOf<Block>>(
        &self, reg: RegisterId<W, R>, value: W,
    )
    where R::Access: WriteAccess { ... }

    #[rseq::intrinsic("mmio_write_field")]
    pub fn write_field<W: RegWidth, R: RegisterOf<Block>>(
        &self, values: FieldValue<W, R>,
    )
    where R::Access: WriteAccess { ... }
}
```

The authored surface:
```rust
use pl011::{dr, fr, cr, lcr_h, imsc, mis};

self.regs.write_field(cr::UARTEN::ENABLED + cr::TXE::ENABLED);
let status = self.regs.read(mis::REG);
if self.regs.read_field(fr::RXFE) != 0 { ... }
```

The compiler resolves identity from the monomorphized type parameters
on the intrinsic call:
- `Block` (from `&self` receiver type `Mmio<pl011::Regs>`) -> register
  block = `pl011::Regs`
- `R` (e.g., `pl011::fr::Register`) -> register identity, scoped to
  block, unique + offset from `pl011::fr::OFFSET`
- `field` argument -> `Field<u32, R>` -> mask + shift
- `R::Access` -> access kind (ReadWrite, ReadOnly, WriteOnly)

See spec.md Section 10.2 for the full identity resolution chain.

**Register block binding:**

The `Mmio<T>` type parameter `T` binds the transport to a register
block. Register blocks are transport-independent (see spec.md
Section 3.6). The same register block definition could be used with
a future `I2c<T>` or `PcieBar<T>` transport without modification.

```rust
#[rseq::driver(...)]
pub struct Pl011FullSpec {
    #[rseq::mmio(id = "uart_regs")]
    pub regs: Mmio<Pl011Registers>,   // MMIO transport, Pl011Registers block
}
```

The `#[rseq::mmio(id = "uart_regs")]` attribute provides the spec-level
logical id. The platform file provides the physical binding:

```toml
[mmio.uart_regs]
base = "0xFE201000"
size = "0x1000"
```

**MIR behavior (expected):**

At `mir_drops_elaborated_and_const_checked` with `mir_opt_level = 0`,
register access through `Mmio<T>` produces a direct Call to the
intrinsic method:

```
_mmio_ref = &((*_self).regs: Mmio<pl011::Regs>)          // field projection
_result = Mmio::<pl011::Regs>::read_field::<u32, pl011::fr::Register>(
    _mmio_ref,           // &Mmio<pl011::Regs>
    fr::RXFE             // Field<u32, pl011::fr::Register>
) -> bb_next             // single Call terminator
```

Key observations:
- The intrinsic method call appears as a single `Call` terminator
  directly to `Mmio::read_field`. No wrapper indirection, no
  `Deref::deref` call in the register access path.
- The `&self` receiver (`_mmio_ref`) is a reference to the `Mmio<T>`
  resource field on the driver struct. Its Place expression carries
  the resource identity.
- All types are fully monomorphized -- `Mmio<pl011::Regs>`,
  `pl011::fr::Register`, `Field<u32, pl011::fr::Register>` are
  concrete.
- The `#[rseq::intrinsic("mmio_read_field")]` attribute on the method
  definition tells the IR constructor to emit an `IntrinsicCall`
  terminator instead of a regular `Call`.

**`read()` and `modify()` methods:**

`Mmio::read(reg)` reads the full raw register value (no field masking).
Returns the register's width type. Used in PL011 for reading the
data register (DR) and masked interrupt status (MIS) where multiple
fields are extracted from a single read.

`Mmio::modify(values)` is NOT an intrinsic. It is a regular method
on `Mmio<T>` that calls `self.read(reg)` then `self.write(reg, new_val)`:

```rust
impl<Block> Mmio<Block> {
    pub fn modify<W: RegWidth, R: RegisterOf<Block>>(
        &self, values: FieldValue<W, R>,
    )
    where R::Access: ReadAccess + WriteAccess {
        let current: W = self.read(R::REG);
        let new_val = (current & !values.mask) | values.value;
        self.write(R::REG, new_val);
    }
}
```

At `mir_opt_level = 0`, `modify()` is NOT inlined. The calling
function's MIR shows a single `Call` to `Mmio::modify()`. Inside
`modify()`'s own body, there are two `Call` terminators to
`Mmio::read()` and `Mmio::write()`, which are intrinsics. The IR
constructor extracts `modify()`'s body as a regular `Function`
containing two `IntrinsicCall` terminators separated by standard
bitwise computation (`BinaryOp(BitAnd)`, `UnaryOp(Not)`,
`BinaryOp(BitOr)`). No special compiler treatment is needed --
the backend decides whether to inline `modify()` or emit it as
a helper.

**Lifecycle:** Managed by the compiler in pre-init/teardown contexts.
The spec author does not call map/unmap -- the compiler generates these
in the pre-init and teardown function bodies.

**Backend lowering:**

| Operation | Linux | Zephyr |
|---|---|---|
| Map (pre-init) | `devm_ioremap_resource(dev, res)` | `DEVICE_MMIO_MAP(dev, ...)` |
| Unmap (teardown) | Managed by `devm_*` | Backend-determined |
| 32-bit read | `readl(base + offset)` | `sys_read32(base + offset)` |
| 32-bit write | `writel(value, base + offset)` | `sys_write32(value, base + offset)` |
| 8-bit read | `readb(base + offset)` | `sys_read8(base + offset)` |
| 8-bit write | `writeb(value, base + offset)` | `sys_write8(value, base + offset)` |

The access width (8/16/32-bit) is determined by the register block's
`access_width` property and the register's `T: RegWidth` type parameter.

**Volatile semantics:** MMIO reads and writes are inherently volatile --
hardware may have side effects on read, and writes must not be elided or
reordered by the compiler. This is implicit in the register access
intrinsics (the backend emits volatile accessors like `readl`/`writel`).
The IR does not need an explicit volatile annotation -- the
`RegisterRead`/`RegisterWrite` intrinsic identity is sufficient for the
backend to know volatile semantics are required.

**Read-and-discard pattern:** The PL011 driver uses
`let _ = self.regs.dr.read_raw()` to drain the RX FIFO by reading and
discarding characters. In the IR, this produces a `RegisterRead`
`IntrinsicCall` whose destination is a dead temporary. The backend MUST
still emit the read -- it is a volatile MMIO access with hardware side
effects (clearing the FIFO entry). The `RegisterRead` intrinsic identity
guarantees this; the backend should not optimize away reads to dead
destinations for register intrinsics.

### OrderedRegion: SESE Invariant and CFG Modes

The `ordered!` macro produces an `OrderedRegion { entry, exit }` on the
function body. The region uses a single-entry single-exit (SESE)
invariant: a block belongs to the region if it is reachable from `entry`
without passing through `exit`. See ir-reference.md Section 6 for the
formal definition.

This section documents the different CFG shapes that `ordered!` regions
can produce, with concrete examples for compiler implementation.

**Mode 1: Straight-line calls.**

The simplest and most common case. All operations are unconditional calls
in sequence.

```rust
ordered! {
    self.set_baud_rate(clk, baud);
    self.configure_line_control();
    self.enable_uart();
}
```

CFG:
```
          [barrier]
bb0: Call set_baud_rate ──────────► bb1
bb1: Call configure_line_control ─► bb2
bb2: Call enable_uart ────────────► bb3
          [barrier]
bb3: (continues after region)
```

Entry = bb0, exit = bb3. All blocks on a single path. Barrier at entry
and exit.

**Mode 2: Conditional inside region.**

An `if` or `match` creates a diamond in the CFG. Both branches are
contained within the region.

```rust
ordered! {
    self.set_baud_rate(clk, baud);
    if self.needs_extra_config {
        self.extra_config();
    }
    self.enable_uart();
}
```

CFG:
```
            [barrier]
bb0: Call set_baud_rate ──────────► bb1
bb1: SwitchInt(needs_extra) ──┬──► bb2 (true)
                              └──► bb4 (false)
bb2: Call extra_config ───────► bb3
bb3: Goto ────────────────────► bb4
bb4: Call enable_uart ────────► bb5
            [barrier]
bb5: (continues after region)
```

Entry = bb0, exit = bb5. The diamond (bb1-bb2-bb3-bb4) is fully
contained. Reachability from bb0 without passing through bb5 includes
all blocks regardless of their ID numbering.

Ordering guarantee: `set_baud_rate` before the conditional, which is
before `enable_uart`. The two branches are mutually exclusive so their
relative order is irrelevant.

**Mode 3: Fallible calls with error exits.**

When `?` is used inside `ordered!`, `error_target` edges create abnormal
exits from the region.

```rust
ordered! {
    self.reset.toggle(Duration::from_us(10))?;
    self.pinctrl.apply::<DefaultState>()?;
    self.clock.enable()?;
}
```

CFG:
```
              [barrier]
bb0: IntrinsicCall reset_toggle
         ├── target ──────► bb1
         └── error_target ► bb_err0  [barrier, then cleanup]

bb1: IntrinsicCall pinctrl_apply
         ├── target ──────► bb2
         └── error_target ► bb_err1  [barrier, then cleanup]

bb2: IntrinsicCall clock_enable
         ├── target ──────► bb3
         └── error_target ► bb_err2  [barrier, then cleanup]
              [barrier]
bb3: (continues after region, normal exit)

bb_err0: Drop cascade... return Err  -- OUTSIDE region
bb_err1: Drop cascade... return Err  -- OUTSIDE region
bb_err2: Drop cascade... return Err  -- OUTSIDE region
```

Entry = bb0, exit = bb3. Error blocks are outside the region (they are
targets of edges that leave the region). The `error_target` edges are
abnormal exits.

Barrier placement: at entry (bb0), at normal exit (bb3), and at each
abnormal exit (before jumping to bb_err0/1/2). The barrier at abnormal
exits ensures that operations already executed in the sequence are
visible before cleanup runs.

**Mode 4: Nested regions.**

An inner `ordered!` block within an outer one. Each gets its own
barriers.

```rust
ordered! {
    self.power_on();
    ordered! {
        self.set_clock_rate(clk_hz);
        self.configure_pll();
    }
    self.enable_output();
}
```

CFG:
```
            [outer barrier]
bb0: Call power_on ────────► bb1
            [inner barrier]
bb1: Call set_clock_rate ──► bb2
bb2: Call configure_pll ───► bb3
            [inner barrier]
bb3: Call enable_output ───► bb4
            [outer barrier]
bb4: (continues)
```

Outer: entry = bb0, exit = bb4. Inner: entry = bb1, exit = bb3. The
inner region is strictly contained within the outer. Inner barriers are
additive (may be redundant with outer, but correct). Overlapping
regions (where neither strictly contains the other) are prohibited.

**Mode 5: Loop inside region.**

A `while` loop creates back-edges in the CFG. SESE still holds.

```rust
ordered! {
    while self.regs.read_field(fr::RXFE) == 0 {
        let _ = self.regs.dr.read_raw();
    }
    self.enable_uart();
}
```

CFG:
```
            [barrier]
bb0: IntrinsicCall read_field(RXFE) ──► bb1
bb1: SwitchInt(== 0) ──┬──► bb2 (true, loop body)
                       └──► bb3 (false, loop exit)
bb2: IntrinsicCall read_raw(DR) ──► bb0  (back-edge)
bb3: Call enable_uart ──► bb4
            [barrier]
bb4: (continues)
```

Entry = bb0, exit = bb4. The back-edge (bb2 -> bb0) stays within the
region. One entry, one normal exit. The loop body is contained.

**Barrier kind.**

The `ordered!` barrier is a compiler barrier only:

```c
// C emission for barrier
__asm__ __volatile__("" ::: "memory");   // Linux
// or: barrier();                         // Linux kernel macro
// or: __compiler_barrier();              // Zephyr equivalent
```

This prevents the C compiler from reordering operations across region
boundaries. It does NOT imply a hardware memory barrier (`wmb()`,
`dmb`, etc.). MMIO volatile semantics (inherent in `readl`/`writel`)
handle hardware ordering for register accesses. Future DMA barrier
intrinsics will handle hardware ordering for DMA buffers as separate
`RegseqIntrinsic` variants.

### Resource Cleanup on Error Paths

When a lifecycle callback (e.g., `init()`) returns `Err`, the resources
it successfully acquired must be cleaned up. Rust's drop elaboration
does NOT handle this -- it only drops local variables going out of
scope, not driver struct fields accessed through `&mut self`. The
compiler must generate cleanup code on error paths.

**Strategy: dominator-based rollback.** The compiler analyzes each
lifecycle callback's CFG to determine which reversible operations
dominate each error exit point, and generates reverse operations in
reverse order on the error path. This produces the standard C kernel
goto-chain cleanup pattern.

**Reversible intrinsic pairs.** Only lifecycle operations with known
inverses need rollback. Configuration operations (register writes,
baud rate setup) are not reversible.

| Forward intrinsic | Reverse intrinsic |
|---|---|
| ClockEnable | ClockDisable |
| PinCtrlApply(state) | PinCtrlApply(teardown_state) |
| ResetAssert | ResetDeassert |
| ResetDeassert | ResetAssert |

The teardown pin state (e.g., "sleep") is identified from the platform
file's pin_states list and a `#[rseq::pin_state(teardown)]` marker
attribute (or similar convention to be defined).

**Framework behavior.** When a lifecycle callback returns `Err`, the
framework does NOT call the teardown callback (e.g., `deinit`). This
matches Linux kernel behavior where `remove()` is not called if
`probe()` fails. The error path rollback generated by the compiler is
the sole cleanup mechanism for failed callbacks.

#### Handled cases (dominator analysis)

The following cases are handled by the compiler using dominator
analysis. An IntrinsicCall to a reversible intrinsic that dominates an
error exit point (every path from entry to that exit passes through the
intrinsic) gets a corresponding reverse operation generated on the
error path.

**Case 1: Sequential `?` chain.**

```rust
fn init(&mut self) -> Result<(), RegseqError> {
    self.clock.enable()?;          // succeeds
    self.pinctrl.apply()?;         // FAILS
}
```

Clock enable dominates pinctrl's error exit. Generated cleanup:

```c
ret = clk_prepare_enable(dev->clk);
if (ret)
    goto err_out;

ret = pinctrl_pm_select_default_state(dev->dev);
if (ret)
    goto err_clock;

return 0;

err_clock:
    clk_disable_unprepare(dev->clk);
err_out:
    return ret;
```

**Case 4: Match on result.**

```rust
self.clock.enable()?;
match self.detect_mode()? {
    Mode::A => self.init_a()?,    // could fail
    Mode::B => self.init_b()?,    // could fail
}
```

Clock enable dominates error exits from both match arms. Each arm's
error path includes clock cleanup.

**Case 8: Early return in one branch.**

```rust
self.clock.enable()?;
if self.is_legacy {
    self.legacy_init()?;          // FAILS — clock cleanup needed
    return Ok(());
}
self.modern_init()?;              // FAILS — clock cleanup needed
```

Clock enable dominates both error exits.

**Case 10: Infallible operations in chain.**

```rust
self.clock.enable()?;
let rate = self.clock.get_rate();    // infallible — not tracked
self.set_baud_rate(rate)?;           // FAILS
```

Only reversible intrinsics are tracked. `get_rate()` is infallible and
not a reversible lifecycle operation — it is ignored by the analysis.
Clock enable dominates the error exit.

**Case 11: Inside ordered region.**

```rust
ordered! {
    self.clock.enable()?;
    self.set_baud_rate(clk_hz, baud)?;   // FAILS
}
```

Same as Case 1 but inside an ordered region. The error exit gets both
the ordered-region barrier and the clock cleanup.

**Case 12: Non-reversible failure.**

```rust
self.clock.enable()?;
self.clock.set_rate(48_000_000)?;    // FAILS
```

`set_rate` is not in the reversible pair table — there is no
"un-set-rate." Only `clock.enable()` needs reversal. The analysis
correctly generates `clk_disable_unprepare` on the error path without
attempting to undo the rate change.

#### Unhandled cases (require extensions beyond dominator analysis)

The following cases cannot be handled by dominator analysis alone. The
compiler does NOT generate rollback for these patterns. When the
compiler detects one of these patterns in a lifecycle callback, it
emits a diagnostic warning identifying the unhandled resource
acquisition, allowing the driver author to add explicit cleanup or
restructure the code.

**Case 2: Optional resource with `?`.**

```rust
if let Some(clock) = &self.clock {
    clock.enable()?;
}
self.pinctrl.apply()?;               // FAILS
// clock.disable() needed — but only if clock was present and enabled
```

**Why not handled:** The clock enable is inside a SwitchInt branch
(the if-let). It does not dominate the pinctrl error exit because the
else-branch (clock is None) skips it. The compiler cannot determine at
the error exit whether the clock branch was taken without additional
tracking.

**What is needed:** Conditional cleanup flags. The compiler generates a
boolean local (e.g., `_clock_enabled: bool = false`) set to `true`
after the enable succeeds. The error path checks the flag:
```c
if (_clock_enabled)
    clk_disable_unprepare(dev->clk);
```
This is analogous to Rust's internal drop flags for conditionally-
initialized locals.

**Case 3: Two optional resources.**

```rust
if let Some(clock) = &self.clock {
    clock.enable()?;
}
if let Some(pinctrl) = &self.pinctrl {
    pinctrl.apply()?;                // FAILS
}
// clock cleanup only if it was enabled
```

Same underlying problem as Case 2 — neither enable dominates the
other's error exit.

**What is needed:** Same conditional cleanup flag mechanism as Case 2,
applied per optional resource.

**Case 5: Runtime-conditional enable.**

```rust
self.clock.enable()?;
if self.current_speed > 115200 {
    self.pll.enable()?;              // conditional
}
self.configure()?;                   // FAILS
// pll cleanup only if it was enabled
```

Clock enable dominates the error exit (handled). PLL enable does not
(it is inside a runtime conditional).

**What is needed:** Conditional cleanup flag for the PLL, same
mechanism as Case 2.

**Case 6: Loop over array resources.**

```rust
for i in 0..self.num_channels {
    self.channels[i].clock.enable()?;   // FAILS at i=2
}
// channels[0] and [1] need cleanup, but not [2]
```

**Why not handled:** The enable is inside a loop with a back-edge. The
number of successful iterations is dynamic. Dominator analysis cannot
determine how many loop iterations completed.

**What is needed:** Loop induction variable tracking and reverse-loop
generation. The compiler must recognize the loop structure, capture the
induction variable's value at the error point, and generate a reverse
loop:
```c
for (i = 0; i < num_channels; i++) {
    ret = clk_prepare_enable(channels[i].clk);
    if (ret)
        goto err_loop;
}
// ...
err_loop:
    for (int j = i - 1; j >= 0; j--)
        clk_disable_unprepare(channels[j].clk);
```
This is a recognized pattern in Linux kernel drivers but requires
substantially more analysis than dominator-based rollback.

**Case 7: Acquisition inside a helper function.**

```rust
fn init(&mut self) -> Result<(), RegseqError> {
    self.setup_clocks()?;           // enables clock internally
    self.setup_pins()?;             // FAILS
    // clock needs cleanup — but enable is inside setup_clocks()
}
```

**Why not handled:** The clock enable is in `setup_clocks()`'s body,
not visible in `init()`'s CFG. Dominator analysis is intraprocedural
-- it only sees the current function's basic blocks.

**What is needed:** Interprocedural analysis with function summaries.
Each function would carry a summary of "resources acquired on success"
and "resources released on success." The caller's analysis uses the
callee's summary to determine what needs rollback. Alternatively, the
compiler could mandate that helper functions returning `Result` handle
their own rollback on failure (the callee cleans up its own partial
work before returning `Err`), making interprocedural tracking
unnecessary.

**Case 9: Re-acquisition after release.**

```rust
self.clock.enable()?;
// ... use clock ...
self.clock.disable();
self.clock.enable()?;              // re-enable
self.something_else()?;            // FAILS
// cleanup: clock.disable() (for the second enable)
```

**Why not handled:** Two clock enables dominate the error exit.
Dominator analysis would generate two `clock.disable()` calls, which
is wrong (double-disable). The compiler needs to track that the first
enable was cancelled by the intermediate disable.

**What is needed:** Resource state tracking through enable/disable
pairs. The analysis maintains a per-resource state (enabled/disabled)
and generates rollback only for the most recent state transition. This
is a dataflow analysis tracking the last-write to each resource's
lifecycle state along the path to each error exit.

#### Summary

| Case | Pattern | Handled | Extension needed |
|---|---|---|---|
| 1 | Sequential `?` chain | Yes | -- |
| 2 | Optional resource + `?` | No | Conditional cleanup flags |
| 3 | Two optional resources | No | Conditional cleanup flags |
| 4 | Match on result | Yes | -- |
| 5 | Runtime-conditional enable | Partial | Conditional cleanup flags |
| 6 | Loop over array resources | No | Reverse-loop generation |
| 7 | Acquisition in helper | No | Interprocedural summaries |
| 8 | Early return in branch | Yes | -- |
| 9 | Re-acquisition after release | No | Resource state tracking |
| 10 | Infallible ops in chain | Yes | -- |
| 11 | Inside ordered region | Yes | -- |
| 12 | Non-reversible failure | Yes | -- |

The handled cases (1, 4, 8, 10, 11, 12) cover the dominant patterns in
lifecycle callbacks. Cases 2, 3, and 5 (conditional cleanup) are the
most important extension — they cover Optional resources, which are
common in real drivers. Cases 6, 7, and 9 are less common and can be
deferred.

---

## 3. Intrinsic Lowering

This section will grow as backends are implemented. For each intrinsic,
it documents the per-target lowering -- what C code the backend emits.

The intrinsic signatures and architecture are defined in spec.md
Section 10.2. The lowering details here complement those definitions
with backend-specific implementation knowledge.

Detailed per-intrinsic lowering tables will be added as backends are
developed, starting with the Linux backend for PL011.

---

## 4. Const-Evaluated Patterns

Certain DSL surface constructs are `const fn` or `const` expressions that
rustc evaluates before the Regseq compiler's extraction stage. The compiler
does not need to recognize these as intrinsics -- by the time it sees the
MIR, they are already constant values.

### Duration constructors

`Duration::from_us(10)`, `Duration::from_ms(100)`, `Duration::from_s(1)`
are all `const fn`. In MIR, they appear as constant `Duration { us: N }`
values, not as function calls.

### FieldValue composition

`CR_REG::UARTEN::ENABLED + CR_REG::TXE::ENABLED` uses the `Add` impl
on `FieldValue<T, R>`, which combines masks and values via bitwise OR.

**Verified (MIR experiment):** At `mir_opt_level = 0`, FieldValue
addition is NOT const-evaluated. It appears as `Call` terminators to
`<FieldValue<u32, R> as Add>::add`. The individual named constants
(`CR_REG::UARTEN::ENABLED`, etc.) ARE const-evaluated (they appear as
`const` operands), but the `+` operator between them is a function call.

For `A + B + C`, the MIR produces two sequential calls:
```
_tmp = <FieldValue as Add>::add(const A, const B) -> bb1
bb1: _result = <FieldValue as Add>::add(move _tmp, const C) -> bb2
```

**IR construction handling:** `<FieldValue as Add>::add` is extracted as
a regular `Function` with a body (ir-reference.md Section 8, recognition
rule 4). The body contains standard bitwise operations:
`BinaryOp(BitOr, mask1, mask2)` and `BinaryOp(BitOr, value1, value2)`.
No special intrinsic needed. The backend decides whether to inline the
call or emit it as a separate function.

### Named register field values

`STATUS_REG::MODE::ACTIVE` is a `const` `FieldValue` with a precomputed
mask and value. In MIR, it appears as a constant operand.

### User-defined helper functions operating on register types

The PL011 driver defines free helper functions that construct and
decompose `Field<T,R>` and `FieldValue<T,R>` values:

```rust
fn field_val<R>(field: Field<u32, R>, raw: u32) -> FieldValue<u32, R> { ... }
fn raw_reg_val<R>(mask: u32, value: u32) -> FieldValue<u32, R> { ... }
fn field_from_raw<R>(field: Field<u32, R>, raw: u32) -> u32 { ... }
```

These are normal user-defined functions with no special Regseq semantics.
The compiler extracts them as regular `Function`s with standard IR
bodies (field access, bitwise ops, shifts). No special compiler
treatment, no intrinsics, no recognition rules. They happen to operate
on register-related types but are just arithmetic helpers.

---

## 5. Implementation Tasks: `#[rseq::]` Attribute Annotations

The `#[rseq::]` attributes are inert custom tool attributes registered
by the compiler wrapper (`regseq-driver`). They are NOT proc macros --
they are preserved in the AST/HIR for extraction. The `regseq-macros`
crate confirms this (lib.rs lines 6-9).

The following types in `crates/regseq/src/` must be annotated during
implementation. The compiler identifies types by these attributes during
extraction.

### Framework types (`#[rseq::framework_type("...")]`)

| Type | File | Attribute to add |
|---|---|---|
| `RegseqError` | `error.rs` (or wherever defined) | `#[rseq::framework_type("regseq-error")]` |
| `SerialErrors` | `serial.rs` | `#[rseq::framework_type("serial-errors")]` |
| `RxChar` | `serial.rs` | `#[rseq::framework_type("rx-char")]` |
| `SerialConfig` | `serial.rs` | `#[rseq::framework_type("serial-config")]` |
| `DataBits` | `serial.rs` | `#[rseq::framework_type("data-bits")]` |
| `Parity` | `serial.rs` | `#[rseq::framework_type("parity")]` |
| `StopBits` | `serial.rs` | `#[rseq::framework_type("stop-bits")]` |
| `FlowControl` | `serial.rs` | `#[rseq::framework_type("flow-control")]` |
| `Duration` | `time.rs` | `#[rseq::framework_type("duration")]` |
| `IrqResult` | `interrupts.rs` (or wherever defined) | `#[rseq::framework_type("irq-result")]` |

### Abstraction types (`#[rseq::abstraction("...")]`)

| Type | File | Attribute to add |
|---|---|---|
| `Clock` | `clock.rs` | `#[rseq::abstraction("clock")]` |
| `ResetLine` | `reset.rs` | `#[rseq::abstraction("reset")]` |
| `PinCtrl` | `pinctrl.rs` | `#[rseq::abstraction("pinctrl")]` |
| `Interrupt` | `interrupts.rs` | `#[rseq::abstraction("interrupt")]` |
| `Mmio<T>` | `mmio.rs` (or `registers.rs`) | `#[rseq::abstraction("mmio")]` |
| `Mutex<T>` | `sync.rs` | `#[rseq::abstraction("mutex")]` |
| `SpinLock<T>` | `sync.rs` | `#[rseq::abstraction("spinlock")]` |
| `IrqSpinLock<T>` | `sync.rs` | `#[rseq::abstraction("irq-spinlock")]` |
| `Completion` | `sync.rs` | `#[rseq::abstraction("completion")]` |

### Register wrapper types

`ReadOnly<T,R>`, `WriteOnly<T,R>`, `ReadWrite<T,R>` are register access
wrappers, not abstraction types. They do not need `#[rseq::abstraction]`.
The compiler recognizes them by their role in the `RegisterTransport`
trait system and their method signatures.

### `PinState` marker types

User-defined pin state marker types need `#[rseq::pin_state(id = "...")]`:

```rust
#[rseq::pin_state(id = "default")]
pub struct DefaultState;
impl PinState for DefaultState {}
```

These are defined by the spec author in their driver crate, not in the
`regseq` facade crate. The `PinState` trait itself (in `pinctrl.rs`)
does not need an attribute.

### Intrinsic functions (`#[rseq::intrinsic]`)

All intrinsic free functions in `crates/regseq/src/intrinsics/` need the
`#[rseq::intrinsic]` attribute. Some already have `#[inline(never)]` but
not the `#[rseq::intrinsic]` marker. The compiler recognizes intrinsics
by this attribute (or by known path as a fallback).

### Note on the `ErrorKind` alias

The `pub type ErrorKind = SerialErrors` alias in `serial.rs` should be
removed. The canonical name is `SerialErrors`. The PL011 fixture has
been updated to use `SerialErrors` directly.
