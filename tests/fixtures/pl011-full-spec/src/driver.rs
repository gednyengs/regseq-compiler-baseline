use regseq::{
    ordered, Clock, Duration, Field, FieldValue, Interrupt, IrqResult, Lifecycle, Mmio, PinCtrl,
    PinState, RegseqError, ResetLine, RxChar, Serial, SerialConfig, SerialErrors,
};

use crate::registers::{
    Pl011Registers, CR_REG, DR_REG, FBRD_REG, FR_REG, IBRD_REG, ICR_REG, IFLS_REG, IMSC_REG,
    LCRH_REG,
};

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

pub struct DefaultState;
impl PinState for DefaultState {}

pub struct SleepState;
impl PinState for SleepState {}

#[rseq::driver(name = "pl011-full-spec", compatible = ["arm,pl011"], bus = "amba")]
#[rseq::device_class(serial)]
pub struct Pl011FullSpec {
    #[rseq::mmio(id = "uart_regs")]
    pub regs: Mmio<Pl011Registers>,

    #[rseq::property(id = "current-speed", default = 115200)]
    pub current_speed: u32,

    #[rseq::property(id = "fifo-size", default = 32)]
    pub fifo_size: u32,

    #[rseq::interrupt(id = "uart_irq")]
    pub irq: Interrupt,

    #[rseq::clock(id = "uart_clk")]
    pub clock: Option<Clock>,

    pub reset: Option<ResetLine>,

    #[rseq::pinctrl(id = "uart_pins")]
    pub pinctrl: Option<PinCtrl>,

    pending_tx: Option<u8>,
}

impl Pl011FullSpec {
    const ALL_UART_INTERRUPTS_MASK: u32 = 0x07ff;
    const RX_INTERRUPT_MASK: u32 = (1 << 4) | (1 << 6);
    const TX_INTERRUPT_MASK: u32 = 1 << 5;
    const ERROR_INTERRUPT_MASK: u32 = (1 << 7) | (1 << 8) | (1 << 9) | (1 << 10);
    const EXPLICIT_ISR_CLEAR_MASK: u32 = Self::ERROR_INTERRUPT_MASK;
    const DEFAULT_UART_CLOCK_HZ: u32 = 48_000_000;

    /// Disable UART operation before reprogramming divisors or line control.
    fn disable_uart(&mut self) {
        self.regs
            .cr
            .write(CR_REG::UARTEN::DISABLED + CR_REG::TXE::DISABLED + CR_REG::RXE::DISABLED);
    }

    /// Disable FIFO mode to begin from a clean bring-up state.
    fn disable_fifo(&mut self) {
        self.regs.lcr_h.modify(LCRH_REG::FEN::DISABLED);
    }

    /// Mask all interrupt sources during polling-oriented bring-up.
    fn mask_all_interrupts(&mut self) {
        self.regs.imsc.write(raw_reg_val::<IMSC_REG::Register>(
            Self::ALL_UART_INTERRUPTS_MASK,
            0,
        ));
    }

    /// Clear all pending PL011 interrupt sources.
    fn clear_all_interrupts(&mut self) {
        self.regs.icr.write(raw_reg_val::<ICR_REG::Register>(
            Self::ALL_UART_INTERRUPTS_MASK,
            Self::ALL_UART_INTERRUPTS_MASK,
        ));
    }

    /// Program FIFO trigger levels for interrupt-driven service.
    fn configure_interrupt_levels(&mut self) {
        self.regs
            .ifls
            .write(IFLS_REG::RXIFLSEL::ONE_HALF + IFLS_REG::TXIFLSEL::ONE_EIGHTH);
    }

    /// Program integer and fractional baud divisors.
    ///
    /// This uses the common PL011 formula:
    /// `(uart_clk_hz * 4) / baud_rate`, with integer/fractional split at 6 bits.
    fn set_baud_rate(&mut self, uart_clk_hz: u32, baud_rate: u32) {
        let div_times_64 = (uart_clk_hz as u64 * 4) / baud_rate as u64;
        let ibrd = (div_times_64 >> 6) as u32;
        let fbrd = (div_times_64 & 0x3f) as u32;

        self.regs
            .ibrd
            .write(field_val(IBRD_REG::BAUD_DIVINT::FIELD, ibrd));
        self.regs
            .fbrd
            .write(field_val(FBRD_REG::BAUD_DIVFRAC::FIELD, fbrd));
    }

    /// Program 8N1 with FIFOs enabled.
    ///
    /// The cross-OS polling path evidence shows that `LCR_H` must be written
    /// after `IBRD`/`FBRD` so the divisor update commits.
    fn configure_line_control(&mut self) {
        self.regs
            .lcr_h
            .write(LCRH_REG::WLEN::EIGHT_BITS + LCRH_REG::FEN::ENABLED);
    }

    /// Enable UART operation with both RX and TX active.
    fn enable_uart(&mut self) {
        self.regs
            .cr
            .write(CR_REG::UARTEN::ENABLED + CR_REG::TXE::ENABLED + CR_REG::RXE::ENABLED);
    }

    /// Enable RX and receive-timeout interrupts for the interrupt-driven path.
    fn enable_rx_interrupts(&mut self) {
        self.regs.imsc.modify(raw_reg_val::<IMSC_REG::Register>(
            Self::RX_INTERRUPT_MASK,
            Self::RX_INTERRUPT_MASK,
        ));
    }

    /// Enable TX interrupts when transmit work is pending.
    fn enable_tx_interrupts(&mut self) {
        self.regs.imsc.modify(raw_reg_val::<IMSC_REG::Register>(
            Self::TX_INTERRUPT_MASK,
            Self::TX_INTERRUPT_MASK,
        ));
    }

    /// Disable TX interrupts when no more transmit work remains.
    fn disable_tx_interrupts(&mut self) {
        self.regs.imsc.modify(raw_reg_val::<IMSC_REG::Register>(
            Self::TX_INTERRUPT_MASK,
            0,
        ));
    }

    /// Shared polling-oriented bring-up sequence.
    fn program_polling_port(&mut self, uart_clk_hz: u32, baud_rate: u32) {
        self.disable_uart();
        self.disable_fifo();
        ordered! {
            self.set_baud_rate(uart_clk_hz, baud_rate);
            self.configure_line_control();
            self.configure_interrupt_levels();
            self.mask_all_interrupts();
            self.clear_all_interrupts();
            self.enable_uart();
            self.enable_rx_interrupts();
        }
    }

    /// Resolve the UART input clock frequency used for divisor programming.
    fn uart_clock_hz(&self) -> u32 {
        self.clock
            .as_ref()
            .map(Clock::get_rate)
            .unwrap_or(Self::DEFAULT_UART_CLOCK_HZ)
    }

    /// Enable the UART clock when this deployment models one explicitly.
    fn enable_clock_if_present(&self) -> Result<(), RegseqError> {
        if let Some(clock) = &self.clock {
            clock.enable()?;
        }
        Ok(())
    }

    /// Disable the UART clock when this deployment models one explicitly.
    fn disable_clock_if_present(&self) {
        if let Some(clock) = &self.clock {
            clock.disable();
        }
    }

    /// Toggle the external reset line once before the first hardware bring-up.
    fn toggle_reset_if_present(&self) -> Result<(), RegseqError> {
        if let Some(reset) = &self.reset {
            reset.toggle(Duration::from_us(10))?;
        }
        Ok(())
    }

    /// Apply the active/default pin muxing state when pinctrl is modeled.
    fn apply_default_pins_if_present(&self) -> Result<(), RegseqError> {
        if let Some(pinctrl) = &self.pinctrl {
            pinctrl.apply::<DefaultState>()?;
        }
        Ok(())
    }

    /// Apply the sleep pin muxing state during teardown when modeled.
    fn apply_sleep_pins_if_present(&self) -> Result<(), RegseqError> {
        if let Some(pinctrl) = &self.pinctrl {
            pinctrl.apply::<SleepState>()?;
        }
        Ok(())
    }

    /// Shared shutdown sequence for the polling-oriented slice.
    fn shutdown_port(&mut self) {
        self.mask_all_interrupts();
        self.clear_all_interrupts();
        self.disable_fifo();
        self.disable_uart();
        self.pending_tx = None;
    }

    /// Drain all currently received characters from the FIFO.
    fn service_rx_fifo(&mut self) -> bool {
        let mut handled = false;
        while self.rx_char().is_some() {
            handled = true;
        }
        handled
    }

    /// Service one staged transmit byte if the TX interrupt fired.
    fn service_tx_fifo(&mut self) -> bool {
        if let Some(byte) = self.pending_tx {
            if self.tx_ready() {
                self.regs
                    .dr
                    .write(field_val(DR_REG::DATA::FIELD, byte as u32));
                self.pending_tx = None;
                self.disable_tx_interrupts();
                return true;
            }
            return false;
        }

        self.disable_tx_interrupts();
        true
    }

    /// Clear explicitly acknowledged interrupt sources (error conditions).
    fn clear_explicit_interrupts(&mut self, status: u32) {
        let clear_mask = status & Self::EXPLICIT_ISR_CLEAR_MASK;
        if clear_mask != 0 {
            self.regs
                .icr
                .write(raw_reg_val::<ICR_REG::Register>(clear_mask, clear_mask));
        }
    }
}

impl Lifecycle for Pl011FullSpec {
    fn init(&mut self) -> Result<(), RegseqError> {
        // Keep the first full-spec behavior step self-contained by using the
        // declared property as the baud source and the declared UART clock
        // where one is available.
        self.toggle_reset_if_present()?;
        self.apply_default_pins_if_present()?;
        self.enable_clock_if_present()?;
        self.program_polling_port(self.uart_clock_hz(), self.current_speed);
        Ok(())
    }

    fn deinit(&mut self) -> Result<(), RegseqError> {
        self.shutdown_port();
        self.disable_clock_if_present();
        self.apply_sleep_pins_if_present()?;
        Ok(())
    }
}

impl Serial for Pl011FullSpec {
    fn port_open(&mut self) -> Result<(), RegseqError> {
        self.enable_clock_if_present()?;
        self.program_polling_port(self.uart_clock_hz(), self.current_speed);
        Ok(())
    }

    fn port_close(&mut self) {
        self.shutdown_port();
        self.disable_clock_if_present();
    }

    fn configure(&mut self, _config: &SerialConfig) -> Result<(), RegseqError> {
        // `SerialConfig` remains intentionally opaque in Phase B/C boundary work,
        // so this step preserves only the validated current-speed intent.
        self.enable_clock_if_present()?;
        self.program_polling_port(self.uart_clock_hz(), self.current_speed);
        Ok(())
    }

    fn tx_byte(&mut self, byte: u8) {
        if self.pending_tx.is_none() && self.tx_ready() {
            self.regs
                .dr
                .write(field_val(DR_REG::DATA::FIELD, byte as u32));
            return;
        }

        self.pending_tx = Some(byte);
        self.enable_tx_interrupts();
    }

    fn rx_char(&mut self) -> Option<RxChar> {
        if self.regs.fr.read(FR_REG::RXFE::FIELD) != 0 {
            return None;
        }

        let dr = self.regs.dr.read_raw();
        let byte = field_from_raw(DR_REG::DATA::FIELD, dr) as u8;

        let mut errors = SerialErrors::empty();
        if field_from_raw(DR_REG::OE::FIELD, dr) != 0 {
            errors = errors | SerialErrors::OVERRUN;
        }
        if field_from_raw(DR_REG::BE::FIELD, dr) != 0 {
            errors = errors | SerialErrors::BREAK;
        }
        if field_from_raw(DR_REG::PE::FIELD, dr) != 0 {
            errors = errors | SerialErrors::PARITY;
        }
        if field_from_raw(DR_REG::FE::FIELD, dr) != 0 {
            errors = errors | SerialErrors::FRAMING;
        }

        Some(RxChar::new(byte, errors))
    }

    fn tx_ready(&mut self) -> bool {
        self.regs.fr.read(FR_REG::TXFF::FIELD) == 0
    }

    fn tx_empty(&mut self) -> bool {
        self.regs.fr.read(FR_REG::BUSY::FIELD) == 0
    }

    fn flush_rx_fifo(&mut self) {
        while self.regs.fr.read(FR_REG::RXFE::FIELD) == 0 {
            let _ = self.regs.dr.read_raw();
        }
    }

    fn flush_tx_fifo(&mut self) {
        self.regs.lcr_h.modify(LCRH_REG::FEN::DISABLED);
        self.regs.lcr_h.modify(LCRH_REG::FEN::ENABLED);
    }

    fn send_break(&mut self, on: bool) {
        if on {
            self.regs.lcr_h.modify(LCRH_REG::BRK::ENABLED);
        } else {
            self.regs.lcr_h.modify(LCRH_REG::BRK::DISABLED);
        }
    }
}

impl Pl011FullSpec {
    #[rseq::isr]
    pub fn handle_irq(&mut self) -> IrqResult {
        let status = self.regs.mis.read_raw();

        if status == 0 {
            return IrqResult::NotHandled;
        }

        let mut handled = false;

        if status & Self::RX_INTERRUPT_MASK != 0 {
            handled |= self.service_rx_fifo();
        }

        if status & Self::TX_INTERRUPT_MASK != 0 {
            handled |= self.service_tx_fifo();
        }

        self.clear_explicit_interrupts(status);

        if handled || (status & Self::EXPLICIT_ISR_CLEAR_MASK != 0) {
            IrqResult::Handled
        } else {
            IrqResult::NotHandled
        }
    }
}
