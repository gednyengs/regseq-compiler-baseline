//! PL011 driver authored against the **framework-pure serial surface**
//! (construct #7, v1 in-scope subset). Re-cut from `pl011-full-spec`: the
//! register block, baud→divisor math, and LCR encode are reused; the surface is
//! restructured into base `Serial` (C04) + `SerialRuntimeLineConfig`
//! (`line_format_apply` only, C03) + `SerialInterruptDriven` (C01).
//!
//! The defining shift vs the old flat `Serial`: the per-source interrupt mask is
//! NOT baked into bring-up. `irq_source_set(source, state)` is the only place a
//! source is armed/disarmed (the author owns WHICH BITS via the match arms); the
//! compiler emits the calls at each per-OS site (WHEN). There is no authored
//! `#[rseq::isr]` — the compiler synthesizes the ISR from `irq_cause_decode` +
//! the data path.

use crate::regseq::{
    ordered, CauseSet, ConfigResult, DataBits, Field, FieldValue, Interrupt, IrqSourceState,
    LineFormat, Mmio, Parity, PortScope, PrevState, ReadyDir, RegseqError, RxDatum, RxError, Serial,
    SerialInterruptDriven, SerialIrqSource, SerialRuntimeLineConfig, StopBits,
};

use crate::registers::pl011::{self, cr, dr, fbrd, fr, ibrd, icr, ifls, imsc, lcr_h, mis};

/// Construct a `FieldValue` from a field descriptor and a raw shifted value.
fn field_val<R>(field: Field<u32, R>, raw: u32) -> FieldValue<u32, R> {
    FieldValue::new(field.mask, raw << field.shift)
}

/// Construct a raw `FieldValue` covering one or more register bits directly.
fn raw_reg_val<R>(mask: u32, value: u32) -> FieldValue<u32, R> {
    FieldValue::new(mask, value)
}

/// Extract a field's shifted value from a full raw register read.
fn field_from_raw<R>(field: Field<u32, R>, raw: u32) -> u32 {
    (raw & field.mask) >> field.shift
}

#[rseq::driver(name = "serial-pl011", compatible = ["arm,pl011"], bus = "amba")]
#[rseq::device_class(serial)]
pub struct SerialPl011 {
    #[rseq::mmio(id = "uart_regs")]
    pub regs: Mmio<pl011::Regs>,

    #[rseq::property(id = "current-speed", default = 115200)]
    pub current_speed: u32,

    /// UART input clock frequency for divisor programming.
    #[rseq::property(id = "uart-clock-hz", default = 48_000_000)]
    pub uart_clock_hz_property: u32,

    #[rseq::interrupt(id = "uart_irq")]
    pub irq: Interrupt,

    /// Software shadow of `IMSC`. Every mask mutation goes through the shadow and
    /// is recomputed + written whole (no RMW on the live register). In the
    /// framework-pure surface the shadow is driven exclusively by
    /// `irq_source_set` (the per-source mask op) and the bring-up/teardown
    /// `mask_all`.
    imsc_shadow: u32,
}

impl SerialPl011 {
    const ALL_UART_INTERRUPTS_MASK: u32 = 0x07ff;
    /// RXIM | RTIM — RX-data couples the receive-timeout source (Zephyr Z11).
    const RX_INTERRUPT_MASK: u32 = (1 << 4) | (1 << 6);
    /// TXIM.
    const TX_INTERRUPT_MASK: u32 = 1 << 5;
    /// FEIM | PEIM | BEIM | OEIM.
    const ERROR_INTERRUPT_MASK: u32 = (1 << 7) | (1 << 8) | (1 << 9) | (1 << 10);
    /// The error sources are W1C-acked in `irq_cause_decode`.
    const EXPLICIT_ISR_CLEAR_MASK: u32 = Self::ERROR_INTERRUPT_MASK;

    /// Commit the current `imsc_shadow` value to the hardware `IMSC` register.
    fn commit_imsc_shadow(&mut self) {
        self.regs.write_field(raw_reg_val::<imsc::Register>(
            Self::ALL_UART_INTERRUPTS_MASK,
            self.imsc_shadow,
        ));
    }

    /// Mask all interrupt sources (bring-up + teardown).
    fn mask_all_interrupts(&mut self) {
        self.imsc_shadow = 0;
        self.commit_imsc_shadow();
    }

    /// Clear all pending PL011 interrupt sources in `ICR` (W1C).
    fn clear_all_interrupts(&mut self) {
        self.regs.write_field(raw_reg_val::<icr::Register>(
            Self::ALL_UART_INTERRUPTS_MASK,
            Self::ALL_UART_INTERRUPTS_MASK,
        ));
    }

    /// Program FIFO trigger levels for interrupt-driven service.
    fn configure_interrupt_levels(&mut self) {
        self.regs
            .write_field(ifls::RXIFLSEL::ONE_HALF + ifls::TXIFLSEL::ONE_EIGHTH);
    }

    /// Program integer + fractional baud divisors (datasheet §3.3.6,
    /// round-to-nearest). `D = (UARTCLK*8 + baud) / (2*baud)` → IBRD = D>>6,
    /// FBRD = D & 0x3f.
    fn set_baud_rate(&mut self, uart_clk_hz: u32, baud_rate: u32) {
        let clk = uart_clk_hz as u64;
        let baud = baud_rate as u64;
        let div_times_64 = (clk * 8 + baud) / (baud * 2);
        let ibrd_val = (div_times_64 >> 6) as u32;
        let fbrd_val = (div_times_64 & 0x3f) as u32;

        self.regs.write_field(field_val(ibrd::BAUD_DIVINT, ibrd_val));
        self.regs.write_field(field_val(fbrd::BAUD_DIVFRAC, fbrd_val));
    }

    /// Validate a `LineFormat` and compute the `LCR_H` field value without
    /// writing. Returns `Err(())` for an unrepresentable format (9-bit framing —
    /// `LCR_H.WLEN` is 2 bits wide). The procedural escape hatch (D04): the
    /// author owns the encode + the legality decision.
    fn line_control_value(f: &LineFormat) -> Result<FieldValue<u32, lcr_h::Register>, ()> {
        let wlen = match f.data_bits {
            DataBits::Five => lcr_h::WLEN::FIVE_BITS,
            DataBits::Six => lcr_h::WLEN::SIX_BITS,
            DataBits::Seven => lcr_h::WLEN::SEVEN_BITS,
            DataBits::Eight => lcr_h::WLEN::EIGHT_BITS,
            DataBits::Nine => return Err(()),
        };
        let stp2 = match f.stop_bits {
            StopBits::One => lcr_h::STP2::ONE_STOP_BIT,
            StopBits::Two => lcr_h::STP2::TWO_STOP_BITS,
        };
        let (pen, eps) = match f.parity {
            Parity::None => (lcr_h::PEN::DISABLED, lcr_h::EPS::ODD),
            Parity::Odd => (lcr_h::PEN::ENABLED, lcr_h::EPS::ODD),
            Parity::Even => (lcr_h::PEN::ENABLED, lcr_h::EPS::EVEN),
        };
        Ok(wlen + stp2 + pen + eps + lcr_h::FEN::ENABLED)
    }
}

/// Lifecycle: probe-time install of a safe IRQ default; teardown re-mask.
impl crate::regseq::Lifecycle for SerialPl011 {
    fn init(&mut self) -> Result<(), RegseqError> {
        // The compiler has acquired MMIO + IRQ handles and wired the synthesized
        // ISR before `init`. Install a safe default: all sources masked, no
        // pending causes latched.
        self.mask_all_interrupts();
        self.clear_all_interrupts();
        Ok(())
    }

    fn deinit(&mut self) -> Result<(), RegseqError> {
        // Defensive re-mask so no spurious source re-latches between teardown and
        // handle release.
        self.mask_all_interrupts();
        Ok(())
    }
}

impl Serial for SerialPl011 {
    fn port_service_set(&mut self, scope: PortScope, on: bool) -> PrevState {
        match scope {
            PortScope::Block => {
                let was_on = self.regs.read_field(cr::UARTEN::FIELD) != 0;
                if on {
                    self.regs.write_field(
                        cr::UARTEN::ENABLED + cr::TXE::ENABLED + cr::RXE::ENABLED,
                    );
                } else {
                    self.regs.write_field(
                        cr::UARTEN::DISABLED + cr::TXE::DISABLED + cr::RXE::DISABLED,
                    );
                }
                PrevState::new(was_on)
            }
            PortScope::Tx => {
                let was_on = self.regs.read_field(cr::TXE::FIELD) != 0;
                if on {
                    self.regs.modify(cr::REG, cr::TXE::ENABLED);
                } else {
                    self.regs.modify(cr::REG, cr::TXE::DISABLED);
                }
                PrevState::new(was_on)
            }
            PortScope::Rx => {
                let was_on = self.regs.read_field(cr::RXE::FIELD) != 0;
                if on {
                    self.regs.modify(cr::REG, cr::RXE::ENABLED);
                } else {
                    self.regs.modify(cr::REG, cr::RXE::DISABLED);
                }
                PrevState::new(was_on)
            }
        }
    }

    fn data_ready_query(&self, dir: ReadyDir) -> bool {
        match dir {
            // TX has space iff the TX FIFO is not full.
            ReadyDir::TxSpace => self.regs.read_field(fr::TXFF) == 0,
            // RX has data iff the RX FIFO is not empty.
            ReadyDir::RxData => self.regs.read_field(fr::RXFE) == 0,
        }
    }

    fn data_unit_write(&mut self, unit: u32) {
        self.regs.write_field(field_val(dr::DATA, unit & 0xff));
    }

    fn data_unit_read(&mut self) -> RxDatum {
        let dr_raw = self.regs.read(dr::REG);
        let unit = field_from_raw(dr::DATA, dr_raw);

        let mut errors = RxError::empty();
        if field_from_raw(dr::OE, dr_raw) != 0 {
            errors = errors | RxError::HW_OVERRUN;
        }
        if field_from_raw(dr::BE, dr_raw) != 0 {
            errors = errors | RxError::BREAK;
        }
        if field_from_raw(dr::PE, dr_raw) != 0 {
            errors = errors | RxError::PARITY;
        }
        if field_from_raw(dr::FE, dr_raw) != 0 {
            errors = errors | RxError::FRAMING;
        }

        RxDatum::new(unit, errors)
    }
}

impl SerialRuntimeLineConfig for SerialPl011 {
    /// Procedural line-format apply (D04). Commits baud + framing; does not read
    /// back (I03). The compiler's choreography guarantees the UART is disabled
    /// (`port_service_set(Block, false)`) before this is called — the datasheet
    /// (§3.3.8) requires LCR_H/IBRD/FBRD be reprogrammed only while disabled.
    fn line_format_apply(&mut self, f: LineFormat) -> ConfigResult {
        let lcr_h_value = match Self::line_control_value(&f) {
            Ok(v) => v,
            Err(()) => return ConfigResult::Unsupported,
        };
        let uart_clk_hz = self.uart_clock_hz_property;
        crate::regseq::log_info!("serial-pl011 baud: {}", f.baud);
        ordered! {
            self.set_baud_rate(uart_clk_hz, f.baud);
            self.regs.write_field(lcr_h_value);
            self.configure_interrupt_levels();
        }
        ConfigResult::Ok
    }
}

impl SerialInterruptDriven for SerialPl011 {
    /// Backend-called per-source mask op (C01/D03). The author owns the
    /// mask-register bits; the compiler owns WHEN this runs. All `(source,
    /// state)` pairs are covered.
    fn irq_source_set(&mut self, source: SerialIrqSource, state: IrqSourceState) {
        let mask = match source {
            SerialIrqSource::TxReady => Self::TX_INTERRUPT_MASK,
            SerialIrqSource::RxData => Self::RX_INTERRUPT_MASK,
            SerialIrqSource::LineError => Self::ERROR_INTERRUPT_MASK,
        };
        match state {
            IrqSourceState::Armed => self.imsc_shadow |= mask,
            IrqSourceState::Disarmed => self.imsc_shadow &= !mask,
        }
        self.commit_imsc_shadow();
    }

    /// Decode pending serviceable causes from `MIS`, and W1C-ack the line-error
    /// causes as part of decoding (there is no separate ack op; F1/B5) so error
    /// causes do not re-latch.
    fn irq_cause_decode(&mut self) -> CauseSet {
        let status = self.regs.read(mis::REG);

        let mut causes = CauseSet::empty();
        if field_from_raw(mis::TXMIS, status) != 0 {
            causes = causes | CauseSet::TX_READY;
        }
        if field_from_raw(mis::RXMIS, status) != 0 {
            causes = causes | CauseSet::RX_DATA;
        }
        if field_from_raw(mis::RTMIS, status) != 0 {
            causes = causes | CauseSet::RX_TIMEOUT;
        }
        if status & Self::ERROR_INTERRUPT_MASK != 0 {
            causes = causes | CauseSet::LINE_ERROR;
        }

        // W1C-ack the error causes (the only causes not self-clearing via the
        // data path / mask). Writing 1 clears; writing 0 is a no-op.
        let clear_mask = status & Self::EXPLICIT_ISR_CLEAR_MASK;
        if clear_mask != 0 {
            self.regs
                .write_field(raw_reg_val::<icr::Register>(clear_mask, clear_mask));
        }

        causes
    }
}
