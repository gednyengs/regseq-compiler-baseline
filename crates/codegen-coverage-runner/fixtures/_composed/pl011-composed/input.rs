#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// The COMPOSED PL011 fixture (backend-linux-c T02): the first time the five
// refined construct surfaces — register-access, bus-resource-management,
// device-lifecycle, diagnostics-logging, RegseqError — flow through the real
// compiler together. Its generic_c run blesses the canonical PL011 oracle
// trace (T03); the linux_c backend lowers the SAME bound model to the
// first-light kernel module (T04-T10).
//
// Device facts are transcribed from the Form-A donor
// `tests/fixtures/pl011-full-spec/src/{driver,registers}.rs` (datasheet §3.3.8
// bring-up sequence, hardware-faithful register map), restricted to this
// unit's scope: no IRQ/clock/pinctrl resources, and the donor's unbounded
// `while FR.BUSY` wait becomes a BOUNDED spin loop (`poll::until` is
// recognized_not_lowered, ledger B-1 — out of scope).

// The shared device-lifecycle author surface (the `Lifecycle` trait + the
// inline `RegseqError`). The backend synthesizes the driving `main`; the
// manifest's `lifecycle_scenario = "orderly"` selects the bring-up scenario.
include!("../../_lifecycle_surface.rs");

// ---------------------------------------------------------------------------
// Diagnostics-logging author surface: the canonical intrinsic anchors + the
// namespaced `regseq::log::{debug,info,warn,error}!` macros (the T01
// spelling). Mirrors the `diag_*` fixtures' inline surface.
// ---------------------------------------------------------------------------
pub mod regseq {
    pub mod log {
        pub mod intrinsics {
            pub enum LogArg<'a> {
                U8(u8), U16(u16), U32(u32), U64(u64),
                I8(i8), I16(i16), I32(i32), I64(i64),
                Bool(bool), Str(&'a str),
            }
            impl From<u8> for LogArg<'_> { fn from(v: u8) -> Self { LogArg::U8(v) } }
            impl From<u16> for LogArg<'_> { fn from(v: u16) -> Self { LogArg::U16(v) } }
            impl From<u32> for LogArg<'_> { fn from(v: u32) -> Self { LogArg::U32(v) } }
            impl From<u64> for LogArg<'_> { fn from(v: u64) -> Self { LogArg::U64(v) } }
            impl From<i8> for LogArg<'_> { fn from(v: i8) -> Self { LogArg::I8(v) } }
            impl From<i16> for LogArg<'_> { fn from(v: i16) -> Self { LogArg::I16(v) } }
            impl From<i32> for LogArg<'_> { fn from(v: i32) -> Self { LogArg::I32(v) } }
            impl From<i64> for LogArg<'_> { fn from(v: i64) -> Self { LogArg::I64(v) } }
            impl From<bool> for LogArg<'_> { fn from(v: bool) -> Self { LogArg::Bool(v) } }
            impl<'a> From<&'a str> for LogArg<'a> { fn from(v: &'a str) -> Self { LogArg::Str(v) } }

            #[inline(never)] #[rseq::intrinsic("log-debug")]
            pub fn log_debug(_fmt: &'static str, _args: &[LogArg<'_>]) {}
            #[inline(never)] #[rseq::intrinsic("log-info")]
            pub fn log_info(_fmt: &'static str, _args: &[LogArg<'_>]) {}
            #[inline(never)] #[rseq::intrinsic("log-warn")]
            pub fn log_warn(_fmt: &'static str, _args: &[LogArg<'_>]) {}
            #[inline(never)] #[rseq::intrinsic("log-error")]
            pub fn log_error(_fmt: &'static str, _args: &[LogArg<'_>]) {}
        }
        macro_rules! debug { ($fmt:literal $(, $arg:expr)* $(,)?) => {
            $crate::regseq::log::intrinsics::log_debug($fmt, &[$($crate::regseq::log::intrinsics::LogArg::from($arg)),*]) }; }
        pub(crate) use debug;
        macro_rules! info { ($fmt:literal $(, $arg:expr)* $(,)?) => {
            $crate::regseq::log::intrinsics::log_info($fmt, &[$($crate::regseq::log::intrinsics::LogArg::from($arg)),*]) }; }
        pub(crate) use info;
        macro_rules! log_warn { ($fmt:literal $(, $arg:expr)* $(,)?) => {
            $crate::regseq::log::intrinsics::log_warn($fmt, &[$($crate::regseq::log::intrinsics::LogArg::from($arg)),*]) }; }
        pub(crate) use log_warn as warn;
        macro_rules! error { ($fmt:literal $(, $arg:expr)* $(,)?) => {
            $crate::regseq::log::intrinsics::log_error($fmt, &[$($crate::regseq::log::intrinsics::LogArg::from($arg)),*]) }; }
        pub(crate) use error;
    }
}

// ---------------------------------------------------------------------------
// Register-access author surface (Form B). The codegen-coverage-runner bridges
// a single self-contained `input.rs` (no `--extern`), so the `register_block!`
// proc-macro is unavailable: this is the hand-written mirror of its expansion,
// per `tests/fixtures/regaccess-pl011/src/regseq.rs` (trimmed to u32, the
// widths this fixture uses). Register access is recognized by the
// `#[rseq::intrinsic("mmio_*")]` attrs + the `Mmio` wrapper name + the
// `#[repr(C)]` block layout (Strategy C).
// ---------------------------------------------------------------------------
use core::marker::PhantomData;
use core::ops::{Add, Deref};

/// Field (bitfield) descriptor: register-relative `mask` + `shift`.
#[derive(Clone, Copy)]
pub struct Field<W, R> {
    pub mask: W,
    pub shift: W,
    _r: PhantomData<R>,
}
impl<W, R> Field<W, R> {
    pub const fn new(mask: W, shift: W) -> Self {
        Self {
            mask,
            shift,
            _r: PhantomData,
        }
    }
}

/// Field write value: register-relative `mask` + composed `value`. Composes via `+`.
#[derive(Clone, Copy)]
pub struct FieldValue<W, R> {
    pub mask: W,
    pub value: W,
    _r: PhantomData<R>,
}
impl<W, R> FieldValue<W, R> {
    pub const fn new(mask: W, value: W) -> Self {
        Self {
            mask,
            value,
            _r: PhantomData,
        }
    }
}
impl<R> Add for FieldValue<u32, R> {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        // Same register; OR the masks and the values.
        Self {
            mask: self.mask | rhs.mask,
            value: self.value | rhs.value,
            _r: PhantomData,
        }
    }
}

/// Read-only register wrapper. `#[repr(transparent)]` over the storage width so
/// `#[repr(C)]` gives byte offsets; access legality = method availability.
#[repr(transparent)]
pub struct ReadOnly<W, R> {
    _v: W,
    _r: PhantomData<R>,
}
impl<W, R> ReadOnly<W, R> {
    /// Whole-register raw read.
    #[inline(never)]
    #[rseq::intrinsic("mmio_read")]
    pub fn read(&self) -> W {
        loop {}
    }
    /// Masked + shifted field read.
    #[inline(never)]
    #[rseq::intrinsic("mmio_read_field")]
    pub fn read_field(&self, _f: Field<W, R>) -> W {
        loop {}
    }
}

/// Write-only register wrapper.
#[repr(transparent)]
pub struct WriteOnly<W, R> {
    _v: W,
    _r: PhantomData<R>,
}
impl<W, R> WriteOnly<W, R> {
    /// Whole-register raw write.
    #[inline(never)]
    #[rseq::intrinsic("mmio_write")]
    pub fn write(&self, _val: W) {}
}

/// Read-write register wrapper.
#[repr(transparent)]
pub struct ReadWrite<W, R> {
    _v: W,
    _r: PhantomData<R>,
}
impl<W, R> ReadWrite<W, R> {
    /// Whole-register raw read.
    #[inline(never)]
    #[rseq::intrinsic("mmio_read")]
    pub fn read(&self) -> W {
        loop {}
    }
    /// Masked + shifted field read.
    #[inline(never)]
    #[rseq::intrinsic("mmio_read_field")]
    pub fn read_field(&self, _f: Field<W, R>) -> W {
        loop {}
    }
    /// Whole-register raw write.
    #[inline(never)]
    #[rseq::intrinsic("mmio_write")]
    pub fn write(&self, _val: W) {}
    /// Masked field write (bits outside the mask = 0).
    #[inline(never)]
    #[rseq::intrinsic("mmio_write_field")]
    pub fn write_field(&self, _fv: FieldValue<W, R>) {}
    /// Read-modify-write a field (preserves unmentioned bits).
    #[inline(never)]
    #[rseq::intrinsic("mmio_modify")]
    pub fn modify(&self, _fv: FieldValue<W, R>) {}
}

/// Reserved gap in a register block (`#[repr(C)]` spacer).
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct Reserved<W> {
    _v: W,
}

// Per-width `new()` constructors (literal `0` → native, body-available).
impl<R> ReadOnly<u32, R> {
    pub const fn new() -> Self {
        Self { _v: 0, _r: PhantomData }
    }
}
impl<R> WriteOnly<u32, R> {
    pub const fn new() -> Self {
        Self { _v: 0, _r: PhantomData }
    }
}
impl<R> ReadWrite<u32, R> {
    pub const fn new() -> Self {
        Self { _v: 0, _r: PhantomData }
    }
}
impl Reserved<u32> {
    pub const fn new() -> Self {
        Self { _v: 0 }
    }
}

/// Typed MMIO resource: the register-block layout carrier, deref'd so
/// `regs.<reg>` selects a register by field access (the carrier storage is
/// never read — every access lowers to a device-model hook).
pub struct Mmio<T> {
    inner: T,
}
impl<T> Mmio<T> {
    pub const fn new(inner: T) -> Self {
        Mmio { inner }
    }
}
impl<T> Deref for Mmio<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.inner
    }
}

/// Construct a `FieldValue` from a field descriptor and a raw (unshifted)
/// value — the Form-A donor's `field_val` helper.
fn field_val<R>(field: Field<u32, R>, raw: u32) -> FieldValue<u32, R> {
    FieldValue::new(field.mask, raw << field.shift)
}

// ---------------------------------------------------------------------------
// The PL011 register block — the hand-written mirror of the Form-A donor's
// `register_block! { pub pl011 { … } }` declaration, HARDWARE-FAITHFUL byte
// offsets (this fixture's trace is the L3 oracle against QEMU's real PL011
// device model, so the offsets must hit the real registers).
// ---------------------------------------------------------------------------
pub mod pl011 {
    use super::{Field, ReadOnly, ReadWrite, Reserved, WriteOnly};

    pub mod dr {
        pub enum Register {}
    }
    pub mod fr {
        pub enum Register {}
        /// UART busy transmitting (datasheet §3.3.8 wait condition).
        pub const BUSY: super::Field<u32, Register> = super::Field::new(1 << 3, 3);
    }
    pub mod ibrd {
        pub enum Register {}
        pub const BAUD_DIVINT: super::Field<u32, Register> = super::Field::new(0xffff, 0);
    }
    pub mod fbrd {
        pub enum Register {}
        pub const BAUD_DIVFRAC: super::Field<u32, Register> = super::Field::new(0x3f, 0);
    }
    pub mod lcr_h {
        use super::FieldValue32;
        pub enum Register {}
        pub const PEN_DISABLED: FieldValue32<Register> = FieldValue32::new(1 << 1, 0);
        pub const STP2_ONE_STOP_BIT: FieldValue32<Register> = FieldValue32::new(1 << 3, 0);
        pub const FEN_DISABLED: FieldValue32<Register> = FieldValue32::new(1 << 4, 0);
        pub const FEN_ENABLED: FieldValue32<Register> = FieldValue32::new(1 << 4, 1 << 4);
        pub const WLEN_EIGHT_BITS: FieldValue32<Register> = FieldValue32::new(0x3 << 5, 0x3 << 5);
    }
    pub mod cr {
        use super::FieldValue32;
        pub enum Register {}
        pub const UARTEN_DISABLED: FieldValue32<Register> = FieldValue32::new(1, 0);
        pub const UARTEN_ENABLED: FieldValue32<Register> = FieldValue32::new(1, 1);
        pub const TXE_DISABLED: FieldValue32<Register> = FieldValue32::new(1 << 8, 0);
        pub const TXE_ENABLED: FieldValue32<Register> = FieldValue32::new(1 << 8, 1 << 8);
        pub const RXE_DISABLED: FieldValue32<Register> = FieldValue32::new(1 << 9, 0);
        pub const RXE_ENABLED: FieldValue32<Register> = FieldValue32::new(1 << 9, 1 << 9);
    }
    pub mod imsc {
        pub enum Register {}
    }
    pub mod icr {
        pub enum Register {}
    }

    /// `FieldValue<u32, R>` shorthand for the per-register const tables.
    pub type FieldValue32<R> = super::FieldValue<u32, R>;

    /// The block: real PL011 byte offsets (PrimeCell UART TRM / donor
    /// `registers.rs`).
    #[repr(C)]
    pub struct Regs {
        /// 0x000 — Data Register.
        pub dr: ReadWrite<u32, dr::Register>,
        /// 0x004 — Receive Status / Error Clear (unused here).
        pub rsr_ecr: Reserved<u32>,
        /// 0x008..0x014 — reserved.
        pub _reserved0: [Reserved<u32>; 4],
        /// 0x018 — Flag Register (read-only, hardware-updated flags).
        pub fr: ReadOnly<u32, fr::Register>,
        /// 0x01C — reserved.
        pub _reserved1: Reserved<u32>,
        /// 0x020 — IrDA low-power counter (unused here).
        pub ilpr: Reserved<u32>,
        /// 0x024 — Integer Baud Rate Divisor.
        pub ibrd: ReadWrite<u32, ibrd::Register>,
        /// 0x028 — Fractional Baud Rate Divisor.
        pub fbrd: ReadWrite<u32, fbrd::Register>,
        /// 0x02C — Line Control Register.
        pub lcr_h: ReadWrite<u32, lcr_h::Register>,
        /// 0x030 — Control Register.
        pub cr: ReadWrite<u32, cr::Register>,
        /// 0x034 — Interrupt FIFO Level Select (unused here).
        pub ifls: Reserved<u32>,
        /// 0x038 — Interrupt Mask Set/Clear Register.
        pub imsc: ReadWrite<u32, imsc::Register>,
        /// 0x03C RIS, 0x040 MIS — reserved here (no IRQ construct).
        pub _reserved2: [Reserved<u32>; 2],
        /// 0x044 — Interrupt Clear Register (write-only, W1C).
        pub icr: WriteOnly<u32, icr::Register>,
    }
}

// Hardware-offset guards: compile-time proof the layout-derived offsets match
// the PL011 map. A miscounted `Reserved` gap fails the build here rather than
// silently mis-lowering (the regaccess-pl011 donor's Codex-#5 convention).
const _: () = assert!(core::mem::offset_of!(pl011::Regs, dr) == 0x000);
const _: () = assert!(core::mem::offset_of!(pl011::Regs, fr) == 0x018);
const _: () = assert!(core::mem::offset_of!(pl011::Regs, ibrd) == 0x024);
const _: () = assert!(core::mem::offset_of!(pl011::Regs, fbrd) == 0x028);
const _: () = assert!(core::mem::offset_of!(pl011::Regs, lcr_h) == 0x02C);
const _: () = assert!(core::mem::offset_of!(pl011::Regs, cr) == 0x030);
const _: () = assert!(core::mem::offset_of!(pl011::Regs, imsc) == 0x038);
const _: () = assert!(core::mem::offset_of!(pl011::Regs, icr) == 0x044);

// ---------------------------------------------------------------------------
// The composed driver: AMBA-matched (ARM PrimeCell UART, the amba-pl011 id
// table entry), bus-managed MMIO region joined to the platform file's
// `[mmio.regs]` (QEMU virt PL011: base 0x0900_0000, size 0x1000) → a RESOLVED
// binding (A-10: the L2 literals are required as the trigger).
// ---------------------------------------------------------------------------

/// UART input clock: QEMU virt's fixed 24 MHz apb_pclk (the donor's
/// `uart-clock-hz` property default specialized to the L2 board).
const UART_CLOCK_HZ: u32 = 24_000_000;
/// Stage-1 default line speed (donor `current-speed` property default).
const BAUD_RATE: u32 = 115_200;
/// Bound on the FR.BUSY spin (the donor's unbounded wait, bounded: no
/// `poll::until` in this unit).
const BUSY_SPIN_BOUND: u32 = 1_000;
/// All eleven PL011 interrupt sources (IMSC/ICR claim-path bits).
const ALL_UART_INTERRUPTS_MASK: u32 = 0x7ff;

// Two-row match table: the real PrimeCell PL011 periphid (the amba-pl011
// idiom) PLUS the eval campaign's reserved test periphid (the L2 DTB
// overrides ttyAMA1 to 0x00abc123 so the stock built-in driver cannot bind
// it; this driver can). The second row also exercises the FULL match_set →
// multi-row amba_id table lowering (the kernel tables are inherently
// multi-row + zero-sentinel).
#[rseq::driver(name = "pl011", bus = "amba", amba = [
    { id = 0x00041011, mask = 0x000fffff },
    { id = 0x00abc123, mask = 0x00ffffff },
])]
pub struct Pl011Composed {
    #[rseq::mmio(id = "regs")]
    regs: Mmio<pl011::Regs>,
}

impl Pl011Composed {
    /// Disable UART operation before reprogramming divisors or line control
    /// (datasheet §3.3.8; donor `disable_uart`).
    fn disable_uart(&mut self) {
        self.regs
            .cr
            .write_field(pl011::cr::UARTEN_DISABLED + pl011::cr::TXE_DISABLED + pl011::cr::RXE_DISABLED);
    }

    /// Bounded wait for the current TX character to leave the shift register
    /// (donor `wait_for_tx_complete`, bounded). `false` = still busy past the
    /// bound — the validation failure `on_init` surfaces as a `RegseqError`.
    fn wait_tx_idle(&self) -> bool {
        let mut tries: u32 = 0;
        while self.regs.fr.read_field(pl011::fr::BUSY) != 0 {
            tries += 1;
            if tries >= BUSY_SPIN_BOUND {
                return false;
            }
        }
        true
    }

    /// Program integer + fractional baud divisors with the donor's
    /// round-to-nearest math (datasheet §3.3.6 Example 3-1); returns
    /// `(ibrd, fbrd)` for the log line.
    fn set_baud_rate(&mut self) -> (u32, u32) {
        let div_x64 = (UART_CLOCK_HZ * 8 + BAUD_RATE) / (2 * BAUD_RATE);
        let ibrd_val = div_x64 >> 6;
        let fbrd_val = div_x64 & 0x3f;
        self.regs
            .ibrd
            .write_field(field_val(pl011::ibrd::BAUD_DIVINT, ibrd_val));
        self.regs
            .fbrd
            .write_field(field_val(pl011::fbrd::BAUD_DIVFRAC, fbrd_val));
        (ibrd_val, fbrd_val)
    }

    /// Mask all interrupt sources (donor `mask_all_interrupts`).
    fn mask_all_interrupts(&mut self) {
        self.regs.imsc.write(0);
    }

    /// Clear all pending interrupt sources; W1C semantics (donor
    /// `clear_all_interrupts`).
    fn clear_all_interrupts(&mut self) {
        self.regs.icr.write(ALL_UART_INTERRUPTS_MASK);
    }
}

impl Lifecycle for Pl011Composed {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        regseq::log::debug!("pl011 probe start");
        // Datasheet §3.3.8 programming sequence (donor `bring_up_port`,
        // minus FIFO-level/IRQ-unmask steps — no IRQ construct in scope):
        // disable, wait for TX idle, disable FIFO, program divisors + line
        // format, mask + clear interrupts, enable.
        self.disable_uart();
        if !self.wait_tx_idle() {
            regseq::log::error!("pl011 tx busy past spin bound {}", BUSY_SPIN_BOUND);
            return Err(RegseqError::DeviceBusy);
        }
        self.regs.lcr_h.modify(pl011::lcr_h::FEN_DISABLED);
        let (ibrd_val, fbrd_val) = self.set_baud_rate();
        regseq::log::info!("pl011 baud divisors: {} {}", ibrd_val, fbrd_val);
        // 8N1 + FIFO enable, composed as the donor's `line_control_value`.
        self.regs.lcr_h.write_field(
            pl011::lcr_h::WLEN_EIGHT_BITS
                + pl011::lcr_h::STP2_ONE_STOP_BIT
                + pl011::lcr_h::PEN_DISABLED
                + pl011::lcr_h::FEN_ENABLED,
        );
        self.mask_all_interrupts();
        self.clear_all_interrupts();
        self.regs
            .cr
            .write_field(pl011::cr::UARTEN_ENABLED + pl011::cr::TXE_ENABLED + pl011::cr::RXE_ENABLED);
        // Enable read-back sanity check: UARTEN must be observable.
        let cr_now = self.regs.cr.read();
        if cr_now & 0x1 == 0 {
            regseq::log::error!("pl011 enable readback failed: {:#x}", cr_now);
            return Err(RegseqError::HardwareError);
        }
        regseq::log::info!("pl011 up");
        Ok(())
    }

    fn on_deinit(&mut self) -> Result<(), RegseqError> {
        // Donor teardown facts (port_close/deinit compressed): defensively
        // mask so no late source re-latches, quiesce the UART, clear latched
        // causes.
        regseq::log::warn!("pl011 deinit: masking interrupts");
        self.mask_all_interrupts();
        self.disable_uart();
        self.clear_all_interrupts();
        Ok(())
    }
}

// No `regseq_main`: the generic-c backend synthesizes `main` (RD-4/RD-6). The
// manifest's `lifecycle_scenario = "orderly"` lowers to: register
// (match=amba:0x41011/0xfffff) → create → associate → resource preamble → bus
// acquisition (claim_regions, get_handle, map_region(0x09000000, 0x1000)) →
// `LC call on_init` (the §3.3.8 bring-up through the based accessor) →
// `LC call on_deinit` (quiesce) → teardown.
