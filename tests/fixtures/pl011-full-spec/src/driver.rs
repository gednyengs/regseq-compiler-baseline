use regseq::{
    ordered, Clock, DataBits, Duration, Field, FieldValue, FlowControl, Interrupt, IrqResult,
    Lifecycle, Mmio, Parity, PinCtrl, PinState, RegseqError, ResetLine, RxChar, Serial,
    SerialConfig, SerialErrors, StopBits,
};

use crate::registers::pl011::{
    self, cr, dr, fbrd, fr, ibrd, icr, ifls, imsc, lcr_h, mis,
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

#[rseq::pin_state(id = "default")]
pub struct DefaultState;
impl PinState for DefaultState {}

#[rseq::pin_state(id = "sleep")]
pub struct SleepState;
impl PinState for SleepState {}

#[rseq::driver(name = "pl011-full-spec", compatible = ["arm,pl011"], bus = "amba")]
#[rseq::device_class(serial)]
pub struct Pl011FullSpec {
    #[rseq::mmio(id = "uart_regs")]
    pub regs: Mmio<pl011::Regs>,

    #[rseq::property(id = "current-speed", default = 115200)]
    pub current_speed: u32,

    #[rseq::property(id = "fifo-size", default = 32)]
    pub fifo_size: u32,

    /// Default UART input clock frequency, used when no `Clock` resource
    /// is bound (see `uart_clock_hz`). REQ-RES-CLK-01 requires this
    /// fallback to be a declared property so device-tree or ACPI firmware
    /// can override it per deployment, rather than a baked-in constant.
    #[rseq::property(id = "uart-clock-hz", default = 48_000_000)]
    pub uart_clock_hz_property: u32,

    #[rseq::interrupt(id = "uart_irq")]
    pub irq: Interrupt,

    #[rseq::clock(id = "uart_clk")]
    pub clock: Option<Clock>,

    #[rseq::reset(id = "uart_rst")]
    pub reset: Option<ResetLine>,

    #[rseq::pinctrl(id = "uart_pins")]
    pub pinctrl: Option<PinCtrl>,

    /// Software shadow of the `IMSC` register. Every IMSC mutation must
    /// pass through the shadow helpers (`enable_*_interrupts`,
    /// `disable_*_interrupts`, `mask_all_interrupts`) rather than touching
    /// the hardware register directly; the helpers recompute the full mask
    /// from the shadow and write it through, avoiding RMW on the live
    /// register in the hot path (REQ-IRQ-03, REQ-CORR-MASK-SHADOW,
    /// REQ-PERF-LATENCY-01).
    imsc_shadow: u32,
}

impl Pl011FullSpec {
    const ALL_UART_INTERRUPTS_MASK: u32 = 0x07ff;
    const RX_INTERRUPT_MASK: u32 = (1 << 4) | (1 << 6);
    const TX_INTERRUPT_MASK: u32 = 1 << 5;
    const ERROR_INTERRUPT_MASK: u32 = (1 << 7) | (1 << 8) | (1 << 9) | (1 << 10);
    const EXPLICIT_ISR_CLEAR_MASK: u32 = Self::ERROR_INTERRUPT_MASK;

    /// Disable UART operation before reprogramming divisors or line control.
    fn disable_uart(&mut self) {
        self.regs
            .write_field(cr::UARTEN::DISABLED + cr::TXE::DISABLED + cr::RXE::DISABLED);
    }

    /// Wait for the current TX character (if any) to finish leaving the shift
    /// register. Required by the datasheet programming sequence
    /// (§3.3.8 lines 1072-1078) before reprogramming LCR_H, IBRD, or FBRD:
    /// the UART must first be disabled, then the current transmission
    /// allowed to complete, before any of the line-control registers are
    /// rewritten.
    fn wait_for_tx_complete(&self) {
        while self.regs.read_field(fr::BUSY) != 0 {}
    }

    /// Disable FIFO mode to begin from a clean bring-up state.
    fn disable_fifo(&mut self) {
        self.regs.modify(lcr_h::REG, lcr_h::FEN::DISABLED);
    }

    /// Commit the current `imsc_shadow` value to the hardware IMSC
    /// register. The write covers all eleven claim-path IRQ bits so
    /// that reserved bits (15:11) are written as zero per the datasheet
    /// "read as zero, do not modify" convention, while the active bits
    /// come from the shadow without an intervening RMW.
    fn commit_imsc_shadow(&mut self) {
        self.regs.write_field(raw_reg_val::<imsc::Register>(
            Self::ALL_UART_INTERRUPTS_MASK,
            self.imsc_shadow,
        ));
    }

    /// Mask all interrupt sources (used during bring-up and teardown).
    fn mask_all_interrupts(&mut self) {
        self.imsc_shadow = 0;
        self.commit_imsc_shadow();
    }

    /// Clear all pending PL011 interrupt sources in `ICR`. W1C semantics
    /// mean writing 1 to a bit clears that interrupt; writing 0 has no
    /// effect, so the mask-and-value pair is identical.
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

    /// Program integer and fractional baud divisors.
    ///
    /// Datasheet §3.3.6 Example 3-1 specifies round-to-nearest for the
    /// 6-bit fractional divisor: `m = integer(BRD_F × 64 + 0.5)`.
    /// Equivalently, the combined 22-bit divisor D = (UARTCLK × 4) / baud
    /// should be computed with round-to-nearest. In integer math:
    /// `D = (UARTCLK × 8 + baud) / (2 × baud)`, which is equivalent to
    /// adding half the denominator before division.
    fn set_baud_rate(&mut self, uart_clk_hz: u32, baud_rate: u32) {
        let clk = uart_clk_hz as u64;
        let baud = baud_rate as u64;
        let div_times_64 = (clk * 8 + baud) / (baud * 2);
        let ibrd_val = (div_times_64 >> 6) as u32;
        let fbrd_val = (div_times_64 & 0x3f) as u32;

        self.regs
            .write_field(field_val(ibrd::BAUD_DIVINT, ibrd_val));
        self.regs
            .write_field(field_val(fbrd::BAUD_DIVFRAC, fbrd_val));
    }

    /// Validate a `SerialConfig` and compute the corresponding `LCR_H`
    /// field value without writing any register.
    ///
    /// Pure validator: this method has no side effects and must be called
    /// before any destructive reprogram step (`disable_uart` +
    /// `wait_for_tx_complete`) so that an `Unsupported` config cannot
    /// leave the port disabled mid-sequence. Round-2 Codex audit flagged
    /// the side-effect ordering as a REQ-LIFE-02 regression; this split
    /// is the fix.
    ///
    /// Stage-1 scope: data bits 5/6/7/8, parity None/Even/Odd, stop bits
    /// 1/2, FIFO always enabled. 9-bit framing is unsupported by the PL011
    /// hardware (LCR_H.WLEN is 2 bits wide) and is rejected. Flow control
    /// other than `FlowControl::None` is Stage-2 material (REQ-FLOW-01)
    /// and is rejected here; Stage 1 CRTSCTS workloads fall back to the
    /// non-flow-control form.
    fn line_control_value(
        config: &SerialConfig,
    ) -> Result<FieldValue<u32, lcr_h::Register>, RegseqError> {
        let wlen = match config.data_bits {
            DataBits::Five => lcr_h::WLEN::FIVE_BITS,
            DataBits::Six => lcr_h::WLEN::SIX_BITS,
            DataBits::Seven => lcr_h::WLEN::SEVEN_BITS,
            DataBits::Eight => lcr_h::WLEN::EIGHT_BITS,
            DataBits::Nine => return Err(RegseqError::Unsupported),
        };

        let stp2 = match config.stop_bits {
            StopBits::One => lcr_h::STP2::ONE_STOP_BIT,
            StopBits::Two => lcr_h::STP2::TWO_STOP_BITS,
        };

        let (pen, eps) = match config.parity {
            Parity::None => (lcr_h::PEN::DISABLED, lcr_h::EPS::ODD),
            Parity::Odd => (lcr_h::PEN::ENABLED, lcr_h::EPS::ODD),
            Parity::Even => (lcr_h::PEN::ENABLED, lcr_h::EPS::EVEN),
        };

        if !matches!(config.flow_control, FlowControl::None) {
            return Err(RegseqError::Unsupported);
        }

        Ok(wlen + stp2 + pen + eps + lcr_h::FEN::ENABLED)
    }

    /// Program `LCR_H` from a pre-validated `FieldValue`.
    ///
    /// `LCR_H` acts as the latch for the 30-bit UARTLCR composite (LCR_H +
    /// IBRD + FBRD); callers must therefore ensure `IBRD`/`FBRD` have been
    /// programmed (or are about to be programmed in the same ordered block)
    /// before this method writes `LCR_H` per datasheet §3.3.7 lines
    /// 1018-1025. Validation is the caller's responsibility -- use
    /// `line_control_value` to build the argument; that function is the
    /// sole entry point that can return `Unsupported`, ensuring this
    /// write is always safe to perform after a `disable_uart`.
    fn write_line_control(&mut self, lcr_h_value: FieldValue<u32, lcr_h::Register>) {
        self.regs.write_field(lcr_h_value);
    }

    /// Produce the stage-1 default `SerialConfig`: current-speed baud from
    /// the declared property, 8N1 framing, no flow control. Used by
    /// `port_open` when no explicit configuration is supplied.
    fn default_serial_config(&self) -> SerialConfig {
        SerialConfig {
            baud_rate: self.current_speed,
            data_bits: DataBits::Eight,
            parity: Parity::None,
            stop_bits: StopBits::One,
            flow_control: FlowControl::None,
        }
    }

    /// Enable UART operation with both RX and TX active.
    fn enable_uart(&mut self) {
        self.regs
            .write_field(cr::UARTEN::ENABLED + cr::TXE::ENABLED + cr::RXE::ENABLED);
    }

    /// Enable RX and receive-timeout interrupts via the shadow.
    fn enable_rx_interrupts(&mut self) {
        self.imsc_shadow |= Self::RX_INTERRUPT_MASK;
        self.commit_imsc_shadow();
    }

    /// Enable TX interrupts via the shadow.
    fn enable_tx_interrupts(&mut self) {
        self.imsc_shadow |= Self::TX_INTERRUPT_MASK;
        self.commit_imsc_shadow();
    }

    /// Disable TX interrupts via the shadow.
    fn disable_tx_interrupts(&mut self) {
        self.imsc_shadow &= !Self::TX_INTERRUPT_MASK;
        self.commit_imsc_shadow();
    }

    /// Enable the four RX error-IRQ sources (FEIM, PEIM, BEIM, OEIM) via
    /// the shadow. Required by REQ-IRQ-04 so the error-handling paths
    /// exercised in Stage 2 workloads (W-F5-*, W-F6-03) can observe the
    /// error interrupts; stage-1 execution also runs the ISR's error-clear
    /// path when these sources are unmasked.
    fn enable_error_interrupts(&mut self) {
        self.imsc_shadow |= Self::ERROR_INTERRUPT_MASK;
        self.commit_imsc_shadow();
    }

    /// Disable the four RX error-IRQ sources via the shadow.
    fn disable_error_interrupts(&mut self) {
        self.imsc_shadow &= !Self::ERROR_INTERRUPT_MASK;
        self.commit_imsc_shadow();
    }

    /// Initial port bring-up used by `Serial::port_open`. Resets the FIFO,
    /// programs baud + line control, sets IFLS, clears pending ICR, and
    /// enables the UART with RX + RT interrupts unmasked. Resets the IMSC
    /// shadow to the RX-only baseline; callers that wish to preserve
    /// richer IRQ state must use `reprogram_line_format` instead.
    ///
    /// Datasheet §3.3.8 lines 1072-1078 programming sequence: disable the
    /// UART, wait for the current character to finish (`FR.BUSY == 0`),
    /// flush the TX FIFO (set FEN=0), reprogram the control registers,
    /// then re-enable.
    ///
    /// Config validation happens BEFORE any destructive register writes so
    /// that a rejected `SerialConfig` cannot leave the hardware in a
    /// half-programmed state.
    fn bring_up_port(&mut self, config: &SerialConfig) -> Result<(), RegseqError> {
        let lcr_h_value = Self::line_control_value(config)?;
        let uart_clk_hz = self.uart_clock_hz();
        self.disable_uart();
        self.wait_for_tx_complete();
        self.disable_fifo();
        regseq::log_info!("pl011 baud rate: {}", config.baud_rate);
        ordered! {
            self.set_baud_rate(uart_clk_hz, config.baud_rate);
            self.write_line_control(lcr_h_value);
            self.configure_interrupt_levels();
            self.mask_all_interrupts();
            self.clear_all_interrupts();
            self.enable_uart();
            self.enable_rx_interrupts();
        }
        Ok(())
    }

    /// Reprogram baud + line control on an already-open port without
    /// touching the FIFO, IFLS, or IMSC shadow. Used by
    /// `Serial::configure` so that line-format changes do not clobber
    /// the caller's IRQ-mask state.
    ///
    /// Config validation happens BEFORE `disable_uart` so a rejected
    /// `SerialConfig` cannot leave the open port in a disabled state
    /// (REQ-LIFE-02).
    fn reprogram_line_format(&mut self, config: &SerialConfig) -> Result<(), RegseqError> {
        let lcr_h_value = Self::line_control_value(config)?;
        let uart_clk_hz = self.uart_clock_hz();
        self.disable_uart();
        self.wait_for_tx_complete();
        regseq::log_info!("pl011 baud rate: {}", config.baud_rate);
        ordered! {
            self.set_baud_rate(uart_clk_hz, config.baud_rate);
            self.write_line_control(lcr_h_value);
            self.enable_uart();
        }
        Ok(())
    }

    /// Resolve the UART input clock frequency used for divisor programming.
    ///
    /// Preference order: bound `Clock` resource's rate when present, else
    /// the `uart-clock-hz` property. Using a property for the fallback
    /// satisfies REQ-RES-CLK-01 ("when `Clock` resource is absent, a
    /// default input clock frequency (property) is used").
    fn uart_clock_hz(&self) -> u32 {
        self.clock
            .as_ref()
            .map(Clock::get_rate)
            .unwrap_or(self.uart_clock_hz_property)
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

    /// Drain all currently received characters from the FIFO.
    fn service_rx_fifo(&mut self) -> bool {
        let mut handled = false;
        while self.rx_char().is_some() {
            handled = true;
        }
        handled
    }

    /// Clear explicitly acknowledged interrupt sources (error conditions).
    fn clear_explicit_interrupts(&mut self, status: u32) {
        let clear_mask = status & Self::EXPLICIT_ISR_CLEAR_MASK;
        if clear_mask != 0 {
            self.regs
                .write_field(raw_reg_val::<icr::Register>(clear_mask, clear_mask));
        }
    }
}

/// Resource ownership and lifecycle preconditions for `Pl011FullSpec`
/// (REQ-BC-LIFECYCLE).
///
/// **Compiler-acquired** (valid for the lifetime of every body method;
/// released only after `Lifecycle::deinit` returns):
/// - `Mmio<pl011::Regs>` register block via `#[rseq::mmio]`
/// - `Interrupt` IRQ handle via `#[rseq::interrupt]`; the compiler has
///   already registered `handle_irq` as the ISR before `init` runs
/// - `Option<Clock>` via `#[rseq::clock]`
/// - `Option<ResetLine>` via `#[rseq::reset]`
/// - `Option<PinCtrl>` via `#[rseq::pinctrl]`
/// - Properties: `current_speed`, `fifo_size`, `uart_clock_hz_property`
///
/// **Body-operated** (the compiler provides handles; the authored body
/// invokes the operational methods):
/// - Clock enable/disable (in `port_open`/`port_close`)
/// - Reset pulse (one-shot in `init`)
/// - Pinctrl state transitions (default in `port_open`, sleep in
///   `port_close`)
/// - All MMIO register writes (throughout)
/// - IMSC shadow maintenance (via the `*_interrupts` helpers)
///
/// **Lifecycle ordering guarantees from the compiler**:
/// 1. `init` runs exactly once per probe, after all handles are
///    acquired and the ISR is live.
/// 2. `port_open` and `port_close` bracket each open/close session;
///    multiple cycles may occur per probe.
/// 3. `configure` may be called any time the port is open; it is
///    neither first nor last in the lifecycle.
/// 4. `deinit` runs exactly once per probe, after the last `port_close`
///    (if any `port_open` ever ran); handles are released after `deinit`
///    returns.
///
/// Panic contract: `port_close` has no return type and is therefore
/// infallible by signature; all other lifecycle methods return
/// `Result<(), RegseqError>` so failure modes are expressed in the
/// signature rather than through panics.
impl Lifecycle for Pl011FullSpec {
    fn init(&mut self) -> Result<(), RegseqError> {
        // Probe-time work only. The compiler has already acquired MMIO,
        // IRQ, Clock, Reset, and PinCtrl handles and wired the ISR; we
        // toggle the external reset line (if present) and install a safe
        // IRQ default. Clock, pinctrl, and UART register programming are
        // deferred to `port_open`, matching the Linux `pl011_probe` /
        // `pl011_startup` split (amba-pl011.c:2971-3025 and 1801-1972).
        self.toggle_reset_if_present()?;
        self.mask_all_interrupts();
        self.clear_all_interrupts();
        Ok(())
    }

    fn deinit(&mut self) -> Result<(), RegseqError> {
        // Remove-time work only. `port_close` is expected to have run
        // and already torn down UART state, clock, and pins; we defensively
        // re-mask IRQs so any spurious source raised between `port_close`
        // and compiler handle release cannot re-latch.
        self.mask_all_interrupts();
        Ok(())
    }
}

impl Serial for Pl011FullSpec {
    fn port_open(&mut self) -> Result<(), RegseqError> {
        // Per Linux `pl011_hwinit` (amba-pl011.c:1801-1817): pinctrl
        // default → clock enable → hardware bring-up. The bring-up itself
        // follows the datasheet programming sequence.
        self.apply_default_pins_if_present()?;
        self.enable_clock_if_present()?;
        let cfg = self.default_serial_config();
        self.bring_up_port(&cfg)?;
        Ok(())
    }

    fn port_close(&mut self) {
        // Per Linux `pl011_shutdown` (amba-pl011.c:2042-2064): mask IRQs,
        // quiesce the UART, release clock and pins. Order matters: mask
        // IRQs first so no late source arrives mid-teardown; wait for
        // the current TX character so disabling the UART does not leave
        // a half-transmitted byte on the wire; clear latched ICR bits so
        // the next `port_open` starts from a clean IRQ state.
        self.mask_all_interrupts();
        self.wait_for_tx_complete();
        self.disable_uart();
        self.clear_all_interrupts();
        self.disable_clock_if_present();
        // `port_close` is infallible by trait signature, so pinctrl apply
        // is best-effort; any pinctrl error leaves pins in their current
        // state rather than panicking the caller.
        let _ = self.apply_sleep_pins_if_present();
    }

    fn configure(&mut self, config: &SerialConfig) -> Result<(), RegseqError> {
        // Reconfigure an already-open port. Uses `reprogram_line_format`
        // which preserves the IMSC shadow so the caller's IRQ-mask state
        // survives the line-format change.
        self.reprogram_line_format(config)?;
        Ok(())
    }

    /// Unchecked single-byte TX primitive.
    ///
    /// Writes one byte to `DR`. The caller MUST have observed
    /// `tx_ready() == true` immediately before this call; this method
    /// performs no internal checking and no backpressure staging.
    ///
    /// Multi-byte batched backends emit loops of the form:
    ///
    /// ```text
    /// while caller_has_more && self.tx_ready() { self.tx_byte(b); }
    /// if caller_has_more { self.enable_tx_interrupts(); }
    /// // Resume on TXIM via the ISR, which the OS layer ties to a
    /// // refill callback.
    /// ```
    ///
    /// This unchecked shape satisfies REQ-PERF-BATCH-01 (no byte-at-a-time
    /// ISR contract forced on the backend) and eliminates the silent
    /// overwrite that existed while a single-slot `pending_tx` staged the
    /// byte-at-a-time fallback. Matches the pattern used by
    /// `pl011_tx_char` in the Linux `amba-pl011` corpus driver.
    fn tx_byte(&mut self, byte: u8) {
        self.regs.write_field(field_val(dr::DATA, byte as u32));
    }

    fn rx_char(&mut self) -> Option<RxChar> {
        if self.regs.read_field(fr::RXFE) != 0 {
            return None;
        }

        let dr_raw = self.regs.read(dr::REG);
        let byte = field_from_raw(dr::DATA, dr_raw) as u8;

        let mut errors = SerialErrors::empty();
        if field_from_raw(dr::OE, dr_raw) != 0 {
            errors = errors | SerialErrors::OVERRUN;
        }
        if field_from_raw(dr::BE, dr_raw) != 0 {
            errors = errors | SerialErrors::BREAK;
        }
        if field_from_raw(dr::PE, dr_raw) != 0 {
            errors = errors | SerialErrors::PARITY;
        }
        if field_from_raw(dr::FE, dr_raw) != 0 {
            errors = errors | SerialErrors::FRAMING;
        }

        Some(RxChar::new(byte, errors))
    }

    fn tx_ready(&mut self) -> bool {
        self.regs.read_field(fr::TXFF) == 0
    }

    fn tx_empty(&mut self) -> bool {
        self.regs.read_field(fr::BUSY) == 0
    }

    fn flush_rx_fifo(&mut self) {
        while self.regs.read_field(fr::RXFE) == 0 {
            let _ = self.regs.read(dr::REG);
        }
    }

    fn flush_tx_fifo(&mut self) {
        self.regs.modify(lcr_h::REG, lcr_h::FEN::DISABLED);
        self.regs.modify(lcr_h::REG, lcr_h::FEN::ENABLED);
    }

    /// Toggle `LCR_H.BRK` to assert or release a line-break condition.
    ///
    /// Matches the corpus convention for PL011 break generation: Linux
    /// `pl011_break_ctl` (amba-pl011.c:1727-1742) and Windows
    /// `PL011HwSetBreak` (PL011hw.cpp:1180-1218) both implement a pure
    /// RMW toggle under a lock, with no duration enforcement and no TX
    /// FIFO drain. FreeBSD exposes a break ioctl with an empty case body
    /// (no-op), and Zephyr does not provide a break-generation API.
    ///
    /// The datasheet (§3.3.7 line 1016) requires software to hold
    /// `BRK == 1` for at least two complete frame times at the current
    /// line format. That timing is the caller's responsibility (the OS
    /// tty / ioctl layer): `tcsendbreak` on Linux and
    /// `IOCTL_SERIAL_SET_BREAK_OFF` on Windows are the natural points
    /// that drive the hold duration. The driver does not enforce it.
    ///
    /// Datasheet §3.3.7 lines 1043-1045 note that FIFO integrity is not
    /// guaranteed while `BRK` is asserted. The corpus accepts this as
    /// caller discipline (no Linux/Windows driver drains the TX FIFO
    /// before raising BRK); this fixture matches that convention.
    fn send_break(&mut self, on: bool) {
        if on {
            self.regs.modify(lcr_h::REG, lcr_h::BRK::ENABLED);
        } else {
            self.regs.modify(lcr_h::REG, lcr_h::BRK::DISABLED);
        }
    }
}

impl Pl011FullSpec {
    #[rseq::isr]
    pub fn handle_irq(&mut self) -> IrqResult {
        let status = self.regs.read(mis::REG);

        if status == 0 {
            return IrqResult::NotHandled;
        }

        let mut handled = false;

        if status & Self::RX_INTERRUPT_MASK != 0 {
            handled |= self.service_rx_fifo();
        }

        if status & Self::TX_INTERRUPT_MASK != 0 {
            // TX FIFO has drained below the IFLS threshold. In the
            // OS-integrated backend emission this hook re-enters the
            // caller's TX refill callback (Linux `pl011_tx_chars`,
            // Zephyr `uart_irq_tx_ready`); at the fixture level we mask
            // TXIM so the interrupt does not re-latch until the next
            // caller re-enables it via `enable_tx_interrupts`.
            self.disable_tx_interrupts();
            handled = true;
        }

        self.clear_explicit_interrupts(status);

        if handled || (status & Self::EXPLICIT_ISR_CLEAR_MASK != 0) {
            IrqResult::Handled
        } else {
            IrqResult::NotHandled
        }
    }
}
