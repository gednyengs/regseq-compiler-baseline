// The shared PL011 surface of the `_serial/*` fixtures (serial device-class
// construct #7): the diagnostics-logging anchors, the register-access Form-B
// author surface, and the PL011 register block with the FULL serial data +
// line-config + interrupt path exposed — DR (0x000), FR (0x018), IBRD (0x024),
// FBRD (0x028), LCR_H (0x02C), CR (0x030), IFLS (0x034), IMSC (0x038),
// MIS (0x040), ICR (0x044). Whole-register read/write/write_field wrappers; the
// author body does the explicit bit math (the realistic register-level work).
//
// Pulled in by each `_serial/*` fixture with
//   include!("../../_serial_pl011_surface.rs");
// alongside `_lifecycle_surface.rs`, `_interrupt_surface.rs`, and
// `_serial_surface.rs`. The including `input.rs` owns the crate attrs.

// ---------------------------------------------------------------------------
// Diagnostics-logging author surface (intrinsic anchors + namespaced macros).
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
// Register-access author surface (Form B), hand-written `register_block!`
// mirror (trimmed to u32 whole-register access).
// ---------------------------------------------------------------------------
use core::marker::PhantomData;
use core::ops::{Add, Deref};

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
        Self { mask: self.mask | rhs.mask, value: self.value | rhs.value, _r: PhantomData }
    }
}

/// Read-only register wrapper.
#[repr(transparent)]
pub struct ReadOnly<W, R> { _v: W, _r: PhantomData<R> }
impl<W, R> ReadOnly<W, R> {
    #[inline(never)] #[rseq::intrinsic("mmio_read")]
    pub fn read(&self) -> W { loop {} }
}

/// Write-only register wrapper.
#[repr(transparent)]
pub struct WriteOnly<W, R> { _v: W, _r: PhantomData<R> }
impl<W, R> WriteOnly<W, R> {
    #[inline(never)] #[rseq::intrinsic("mmio_write")]
    pub fn write(&self, _val: W) {}
}

/// Read-write register wrapper.
#[repr(transparent)]
pub struct ReadWrite<W, R> { _v: W, _r: PhantomData<R> }
impl<W, R> ReadWrite<W, R> {
    #[inline(never)] #[rseq::intrinsic("mmio_read")]
    pub fn read(&self) -> W { loop {} }
    #[inline(never)] #[rseq::intrinsic("mmio_write")]
    pub fn write(&self, _val: W) {}
    #[inline(never)] #[rseq::intrinsic("mmio_write_field")]
    pub fn write_field(&self, _fv: FieldValue<W, R>) {}
}

/// Reserved gap in a register block (`#[repr(C)]` spacer).
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct Reserved<W> { _v: W }

impl<R> ReadOnly<u32, R> { pub const fn new() -> Self { Self { _v: 0, _r: PhantomData } } }
impl<R> WriteOnly<u32, R> { pub const fn new() -> Self { Self { _v: 0, _r: PhantomData } } }
impl<R> ReadWrite<u32, R> { pub const fn new() -> Self { Self { _v: 0, _r: PhantomData } } }
impl Reserved<u32> { pub const fn new() -> Self { Self { _v: 0 } } }

/// Typed MMIO resource: the register-block layout carrier.
pub struct Mmio<T> { inner: T }
impl<T> Mmio<T> { pub const fn new(inner: T) -> Self { Mmio { inner } } }
impl<T> Deref for Mmio<T> { type Target = T; fn deref(&self) -> &T { &self.inner } }

// ---------------------------------------------------------------------------
// The PL011 register block — hardware-faithful byte offsets, serial path
// exposed. The field bit positions live as plain `const`s the author body
// masks against (the realistic register-level work).
// ---------------------------------------------------------------------------
pub mod pl011 {
    use super::{FieldValue, ReadOnly, ReadWrite, Reserved, WriteOnly};

    pub mod dr { pub enum Register {} }
    pub mod fr { pub enum Register {} }
    pub mod ibrd { pub enum Register {} }
    pub mod fbrd { pub enum Register {} }
    pub mod lcr_h { pub enum Register {} }
    pub mod cr {
        use super::FieldValue32;
        pub enum Register {}
        pub const UARTEN_ENABLED: FieldValue32<Register> = FieldValue32::new(1 << 0, 1 << 0);
        pub const TXE_ENABLED: FieldValue32<Register> = FieldValue32::new(1 << 8, 1 << 8);
        pub const RXE_ENABLED: FieldValue32<Register> = FieldValue32::new(1 << 9, 1 << 9);
    }
    pub mod ifls { pub enum Register {} }
    pub mod imsc { pub enum Register {} }
    pub mod mis { pub enum Register {} }
    pub mod icr { pub enum Register {} }

    /// `FieldValue<u32, R>` shorthand for the per-register const tables.
    pub type FieldValue32<R> = FieldValue<u32, R>;

    /// The block: real PL011 byte offsets (PrimeCell UART TRM).
    #[repr(C)]
    pub struct Regs {
        /// 0x000 — Data Register.
        pub dr: ReadWrite<u32, dr::Register>,
        /// 0x004 — Receive Status / Error Clear (unused here).
        pub rsr_ecr: Reserved<u32>,
        /// 0x008..0x014 — reserved.
        pub _reserved0: [Reserved<u32>; 4],
        /// 0x018 — Flag Register (TXFF/RXFE/BUSY readiness).
        pub fr: ReadOnly<u32, fr::Register>,
        /// 0x01C..0x020 — reserved (ILPR unused).
        pub _reserved1: [Reserved<u32>; 2],
        /// 0x024 — Integer Baud Rate Register.
        pub ibrd: ReadWrite<u32, ibrd::Register>,
        /// 0x028 — Fractional Baud Rate Register.
        pub fbrd: ReadWrite<u32, fbrd::Register>,
        /// 0x02C — Line Control Register.
        pub lcr_h: ReadWrite<u32, lcr_h::Register>,
        /// 0x030 — Control Register.
        pub cr: ReadWrite<u32, cr::Register>,
        /// 0x034 — Interrupt FIFO Level Select Register.
        pub ifls: ReadWrite<u32, ifls::Register>,
        /// 0x038 — Interrupt Mask Set/Clear Register.
        pub imsc: ReadWrite<u32, imsc::Register>,
        /// 0x03C — Raw Interrupt Status (unused; the decode reads MIS).
        pub ris: Reserved<u32>,
        /// 0x040 — Masked Interrupt Status (the decode's status read).
        pub mis: ReadOnly<u32, mis::Register>,
        /// 0x044 — Interrupt Clear Register (write-only, W1C).
        pub icr: WriteOnly<u32, icr::Register>,
    }
}

// Hardware-offset guards: a miscounted `Reserved` gap fails the build here.
const _: () = assert!(core::mem::offset_of!(pl011::Regs, dr) == 0x000);
const _: () = assert!(core::mem::offset_of!(pl011::Regs, fr) == 0x018);
const _: () = assert!(core::mem::offset_of!(pl011::Regs, ibrd) == 0x024);
const _: () = assert!(core::mem::offset_of!(pl011::Regs, fbrd) == 0x028);
const _: () = assert!(core::mem::offset_of!(pl011::Regs, lcr_h) == 0x02C);
const _: () = assert!(core::mem::offset_of!(pl011::Regs, cr) == 0x030);
const _: () = assert!(core::mem::offset_of!(pl011::Regs, ifls) == 0x034);
const _: () = assert!(core::mem::offset_of!(pl011::Regs, imsc) == 0x038);
const _: () = assert!(core::mem::offset_of!(pl011::Regs, mis) == 0x040);
const _: () = assert!(core::mem::offset_of!(pl011::Regs, icr) == 0x044);

// PL011 register bit positions / masks the author body uses.
/// All eleven PL011 interrupt sources (IMSC/ICR claim-path bits).
pub const ALL_UART_INTERRUPTS_MASK: u32 = 0x7ff;
/// FR.BUSY (transmit shift register busy).
pub const FR_BUSY: u32 = 1 << 3;
/// FR.RXFE (RX FIFO empty).
pub const FR_RXFE: u32 = 1 << 4;
/// FR.TXFF (TX FIFO full).
pub const FR_TXFF: u32 = 1 << 5;
/// DR data byte mask.
pub const DR_DATA_MASK: u32 = 0xff;
/// DR.FE (framing error, bit 8).
pub const DR_FE: u32 = 1 << 8;
/// DR.PE (parity error, bit 9).
pub const DR_PE: u32 = 1 << 9;
/// DR.BE (break error, bit 10).
pub const DR_BE: u32 = 1 << 10;
/// DR.OE (overrun error, bit 11).
pub const DR_OE: u32 = 1 << 11;
/// CR.UARTEN bit (for prev-state read).
pub const CR_UARTEN: u32 = 1 << 0;
/// IMSC/MIS RXIM|RTIM (RX + receive-timeout).
pub const RX_INTERRUPT_MASK: u32 = (1 << 4) | (1 << 6);
/// IMSC/MIS TXIM.
pub const TX_INTERRUPT_MASK: u32 = 1 << 5;
/// IMSC/MIS FEIM|PEIM|BEIM|OEIM.
pub const ERROR_INTERRUPT_MASK: u32 = (1 << 7) | (1 << 8) | (1 << 9) | (1 << 10);
/// MIS.RXMIS.
pub const MIS_RX: u32 = 1 << 4;
/// MIS.TXMIS.
pub const MIS_TX: u32 = 1 << 5;
/// MIS.RTMIS (receive timeout).
pub const MIS_RT: u32 = 1 << 6;
