// The shared PL011 surface of the `_irq/*` fixtures (device-interrupt
// construct #6): the diagnostics-logging anchors, the register-access Form-B
// author surface, and the PL011 register block WITH the interrupt-path
// registers exposed (MIS at 0x040 read in the ISR, ICR at 0x044 for the W1C
// ack — the composed fixture keeps MIS reserved; here it IS the construct
// witness). Each `_irq/*` fixture pulls this in with
//   include!("../../_irq_pl011_surface.rs");
// alongside `_lifecycle_surface.rs` + `_interrupt_surface.rs`; the including
// `input.rs` owns the crate attrs and declares its own driver + impls.

// ---------------------------------------------------------------------------
// Diagnostics-logging author surface (the canonical intrinsic anchors + the
// namespaced macros), mirroring the composed fixture's inline surface.
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
// Register-access author surface (Form B), the hand-written `register_block!`
// expansion mirror (trimmed to u32), per the composed fixture.
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
        Self { mask, shift, _r: PhantomData }
    }
}

/// Field write value: register-relative `mask` + composed `value`.
#[derive(Clone, Copy)]
pub struct FieldValue<W, R> {
    pub mask: W,
    pub value: W,
    _r: PhantomData<R>,
}
impl<W, R> FieldValue<W, R> {
    pub const fn new(mask: W, value: W) -> Self {
        Self { mask, value, _r: PhantomData }
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

/// Read-only register wrapper.
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
    /// Whole-register raw write.
    #[inline(never)]
    #[rseq::intrinsic("mmio_write")]
    pub fn write(&self, _val: W) {}
    /// Masked field write (bits outside the mask = 0).
    #[inline(never)]
    #[rseq::intrinsic("mmio_write_field")]
    pub fn write_field(&self, _fv: FieldValue<W, R>) {}
}

/// Reserved gap in a register block (`#[repr(C)]` spacer).
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct Reserved<W> {
    _v: W,
}

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

/// Typed MMIO resource: the register-block layout carrier.
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

// ---------------------------------------------------------------------------
// The PL011 register block, HARDWARE-FAITHFUL byte offsets, with the
// interrupt-path registers exposed: IMSC (0x038), MIS (0x040, the ISR's
// status read), ICR (0x044, the W1C ack the handled-fire golden witnesses).
// ---------------------------------------------------------------------------
pub mod pl011 {
    use super::{FieldValue, ReadOnly, ReadWrite, Reserved, WriteOnly};

    pub mod dr {
        pub enum Register {}
    }
    pub mod cr {
        use super::FieldValue32;
        pub enum Register {}
        pub const UARTEN_DISABLED: FieldValue32<Register> = FieldValue32::new(1, 0);
        pub const UARTEN_ENABLED: FieldValue32<Register> = FieldValue32::new(1, 1);
        pub const RXE_ENABLED: FieldValue32<Register> = FieldValue32::new(1 << 9, 1 << 9);
    }
    pub mod imsc {
        pub enum Register {}
    }
    pub mod mis {
        pub enum Register {}
    }
    pub mod icr {
        pub enum Register {}
    }

    /// `FieldValue<u32, R>` shorthand for the per-register const tables.
    pub type FieldValue32<R> = FieldValue<u32, R>;

    /// The block: real PL011 byte offsets (PrimeCell UART TRM).
    #[repr(C)]
    pub struct Regs {
        /// 0x000 — Data Register.
        pub dr: ReadWrite<u32, dr::Register>,
        /// 0x004 — Receive Status / Error Clear (unused here).
        pub rsr_ecr: Reserved<u32>,
        /// 0x008..0x02C — reserved here (no divisor/line programming in the
        /// irq fixtures; the composed fixture owns the §3.3.8 bring-up).
        pub _reserved0: [Reserved<u32>; 10],
        /// 0x030 — Control Register.
        pub cr: ReadWrite<u32, cr::Register>,
        /// 0x034 — Interrupt FIFO Level Select (unused here).
        pub ifls: Reserved<u32>,
        /// 0x038 — Interrupt Mask Set/Clear Register.
        pub imsc: ReadWrite<u32, imsc::Register>,
        /// 0x03C — Raw Interrupt Status (unused; the ISR reads MIS).
        pub ris: Reserved<u32>,
        /// 0x040 — Masked Interrupt Status (the ISR's status read).
        pub mis: ReadOnly<u32, mis::Register>,
        /// 0x044 — Interrupt Clear Register (write-only, W1C).
        pub icr: WriteOnly<u32, icr::Register>,
    }
}

// Hardware-offset guards: a miscounted `Reserved` gap fails the build here
// rather than silently mis-lowering.
const _: () = assert!(core::mem::offset_of!(pl011::Regs, dr) == 0x000);
const _: () = assert!(core::mem::offset_of!(pl011::Regs, cr) == 0x030);
const _: () = assert!(core::mem::offset_of!(pl011::Regs, imsc) == 0x038);
const _: () = assert!(core::mem::offset_of!(pl011::Regs, mis) == 0x040);
const _: () = assert!(core::mem::offset_of!(pl011::Regs, icr) == 0x044);

/// All eleven PL011 interrupt sources (IMSC/ICR claim-path bits).
pub const ALL_UART_INTERRUPTS_MASK: u32 = 0x7ff;
