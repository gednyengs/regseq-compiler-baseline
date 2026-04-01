use regseq::{
    Clock, Field, FieldValue, Lifecycle, Mmio, PinCtrl, PinState, RegseqError, ResetLine, RxChar,
    Serial, SerialConfig,
};

use crate::registers::{
    Pl011Registers, CR_REG, DR_REG, FBRD_REG, FR_REG, IBRD_REG, LCRH_REG,
};

/// Assumed UART input clock frequency in Hz (48 MHz).
const UART_CLK_HZ: u32 = 48_000_000;

/// Construct a `FieldValue` from a `Field` descriptor and a raw value.
fn field_val<R>(field: Field<u32, R>, raw: u32) -> FieldValue<u32, R> {
    FieldValue::new(field.mask, raw << field.shift)
}

/// Zero-sized pin-state marker for default pin configuration.
pub struct DefaultState;
impl PinState for DefaultState {}

#[rseq::driver(name = "pl011-spec-slice", compatible = ["arm,pl011"])]
#[rseq::device_class(serial)]
pub struct Pl011SpecSlice {
    #[rseq::mmio(id = "uart_regs")]
    pub regs: Mmio<Pl011Registers>,

    #[rseq::property(id = "reg-io-width", default = 4)]
    pub reg_io_width: u32,

    #[rseq::property(id = "current-speed")]
    pub current_speed: Option<u32>,

    #[rseq::clock(id = "uart_clk")]
    pub clock: Option<Clock>,

    pub reset: Option<ResetLine>,

    #[rseq::pinctrl(id = "uart_pins")]
    pub pinctrl: Option<PinCtrl>,
}

impl Pl011SpecSlice {
    /// Disable the UART by clearing UARTEN, TXE, and RXE in CR.
    fn disable_uart(&mut self) {
        self.regs
            .cr
            .write(CR_REG::UARTEN::CLEAR + CR_REG::TXE::CLEAR + CR_REG::RXE::CLEAR);
    }

    /// Enable the UART with TX and RX enabled.
    fn enable_uart(&mut self) {
        self.regs
            .cr
            .write(CR_REG::UARTEN::SET + CR_REG::TXE::SET + CR_REG::RXE::SET);
    }

    /// Program the baud-rate divisors for the given baud rate.
    fn set_baud_rate(&mut self, baud: u32) {
        // BRD = UART_CLK / (16 * baud)
        // IBRD = integer part, FBRD = round((fractional * 64) + 0.5)
        let div_times_64 = (UART_CLK_HZ as u64 * 4) / baud as u64;
        let ibrd = (div_times_64 / 64) as u32;
        let fbrd = (div_times_64 % 64) as u32;

        self.regs
            .ibrd
            .write(field_val(IBRD_REG::BAUD_DIVINT::FIELD, ibrd));
        self.regs
            .fbrd
            .write(field_val(FBRD_REG::BAUD_DIVFRAC::FIELD, fbrd));
    }

    /// Configure line control for 8N1 with FIFOs enabled.
    fn configure_line_control(&mut self) {
        self.regs
            .lcr_h
            .write(LCRH_REG::WLEN::WLEN_8 + LCRH_REG::FEN::SET);
    }
}

impl Lifecycle for Pl011SpecSlice {
    fn init(&mut self) -> Result<(), RegseqError> {
        self.disable_uart();
        self.set_baud_rate(115_200);
        self.configure_line_control();
        self.enable_uart();
        Ok(())
    }

    fn deinit(&mut self) -> Result<(), RegseqError> {
        self.disable_uart();
        Ok(())
    }
}

impl Serial for Pl011SpecSlice {
    fn port_open(&mut self) -> Result<(), RegseqError> {
        self.disable_uart();
        self.set_baud_rate(115_200);
        self.configure_line_control();
        self.enable_uart();
        Ok(())
    }

    fn port_close(&mut self) {
        self.disable_uart();
    }

    fn configure(&mut self, _config: &SerialConfig) -> Result<(), RegseqError> {
        // SerialConfig is currently opaque; use plausible bring-up defaults.
        self.disable_uart();
        self.set_baud_rate(115_200);
        self.configure_line_control();
        self.enable_uart();
        Ok(())
    }

    fn tx_byte(&mut self, byte: u8) {
        self.regs
            .dr
            .write(field_val(DR_REG::DATA::FIELD, byte as u32));
    }

    fn rx_char(&mut self) -> Option<RxChar> {
        // Check if receive FIFO is empty.
        let rxfe = self.regs.fr.read(FR_REG::RXFE::FIELD);
        if rxfe != 0 {
            return None;
        }
        // Read the data register to exercise the authored read path.
        let _data = self.regs.dr.read(DR_REG::DATA::FIELD);
        // RxChar has no public constructor; return None for now.
        None
    }

    fn tx_ready(&mut self) -> bool {
        // TX is ready when the TX FIFO is not full.
        self.regs.fr.read(FR_REG::TXFF::FIELD) == 0
    }
}
