#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// The serial device-class M1 vertical-slice fixture (construct #7, PLAN T06/T07):
// a PL011-shaped AMBA driver implementing the framework-pure v1 in-scope serial
// surface — base `Serial` (C04), `SerialRuntimeLineConfig::line_format_apply`
// (C03), and `SerialInterruptDriven` (C01). The defining shift from the old flat
// `Serial`: interrupt arming is NOT baked into bring-up; `irq_source_set(source,
// state)` is the only place a source is armed/disarmed (the author owns WHICH
// BITS via the match arms), and the COMPILER emits those calls at each per-OS
// site (WHEN). There is no authored `#[rseq::isr]` — the compiler synthesizes the
// ISR from `irq_cause_decode` + the data path.

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");
include!("../../_serial_pl011_surface.rs");
include!("../../_serial_surface.rs");

/// UART input clock for divisor programming (QEMU virt PL011: 24 MHz; this
/// fixture pins 48 MHz for a clean 115200 divisor).
const UART_CLOCK_HZ: u32 = 48_000_000;

// LCR_H field encodings (PL011 TRM §3.3.7).
const LCR_H_FEN: u32 = 1 << 4;
const LCR_H_WLEN_SHIFT: u32 = 5;
const LCR_H_STP2: u32 = 1 << 3;
const LCR_H_PEN: u32 = 1 << 1;
const LCR_H_EPS: u32 = 1 << 2;
// IFLS: RXIFLSEL=1/2 (2<<3), TXIFLSEL=1/8 (0).
const IFLS_VALUE: u32 = 2 << 3;

/// AMBA-matched PL011 serial driver. `#[rseq::device_class(serial)]` selects the
/// serial-class lowering; the `Interrupt` line is compiler-managed (construct #6)
/// and its handler is the compiler-synthesized serial ISR (no authored
/// `#[rseq::isr]`).
#[rseq::driver(name = "serialpl011", bus = "amba",
    amba = [
    { id = 0x00041011, mask = 0x000fffff },
    // The Linux Tier-2 campaign reserves the test UART (ttyAMA1) with periphid
    // 0x00abc123 (mask 0x00ffffff) via a DT periphid override, so the stock
    // built-in amba-pl011 cannot bind it — this entry lets the generated driver
    // bind the reserved UART (same convention as pl011-composed; T08 R05 boot).
    { id = 0x00abc123, mask = 0x00ffffff },
    ],
    compatible = ["regseq,serialpl011"]
)]
#[rseq::device_class(serial)]
pub struct SerialPl011 {
    #[rseq::mmio(id = "regs")]
    regs: Mmio<pl011::Regs>,
    #[rseq::interrupt(id = "uart_irq")]
    irq: Interrupt,
    /// Software shadow of IMSC; driven exclusively by `irq_source_set` + the
    /// bring-up/teardown mask-all.
    imsc_shadow: u32,
}

impl SerialPl011 {
    /// Commit the IMSC shadow to hardware (whole-register write, no RMW).
    fn commit_imsc_shadow(&mut self) {
        self.regs.imsc.write(self.imsc_shadow);
    }
    /// Mask all sources (bring-up/teardown quiesce).
    fn mask_all_interrupts(&mut self) {
        self.imsc_shadow = 0;
        self.regs.imsc.write(0);
    }
    /// Clear all latched sources (W1C).
    fn clear_all_interrupts(&mut self) {
        self.regs.icr.write(ALL_UART_INTERRUPTS_MASK);
    }
}

impl Lifecycle for SerialPl011 {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        regseq::log::debug!("serialpl011 probe start");
        // Safe IRQ default: all sources masked, nothing latched. The line is
        // compiler-managed (registered disabled before this callback, enabled
        // after its Ok).
        self.mask_all_interrupts();
        self.clear_all_interrupts();
        regseq::log::info!("serialpl011 up");
        Ok(())
    }

    fn on_deinit(&mut self) -> Result<(), RegseqError> {
        regseq::log::warn!("serialpl011 deinit: masking interrupts");
        self.mask_all_interrupts();
        self.clear_all_interrupts();
        Ok(())
    }
}

impl Serial for SerialPl011 {
    fn port_service_set(&mut self, scope: PortScope, on: bool) -> PrevState {
        match scope {
            PortScope::Block => {
                let was_on = (self.regs.cr.read() & CR_UARTEN) != 0;
                if on {
                    self.regs.cr.write_field(
                        pl011::cr::UARTEN_ENABLED + pl011::cr::TXE_ENABLED + pl011::cr::RXE_ENABLED,
                    );
                } else {
                    self.regs.cr.write(0);
                }
                PrevState::new(was_on)
            }
            PortScope::Tx => {
                let cur = self.regs.cr.read();
                let was_on = (cur & (1 << 8)) != 0;
                let next = if on { cur | (1 << 8) } else { cur & !(1 << 8) };
                self.regs.cr.write(next);
                PrevState::new(was_on)
            }
            PortScope::Rx => {
                let cur = self.regs.cr.read();
                let was_on = (cur & (1 << 9)) != 0;
                let next = if on { cur | (1 << 9) } else { cur & !(1 << 9) };
                self.regs.cr.write(next);
                PrevState::new(was_on)
            }
        }
    }

    fn data_ready_query(&self, dir: ReadyDir) -> bool {
        let fr = self.regs.fr.read();
        match dir {
            ReadyDir::TxSpace => (fr & FR_TXFF) == 0,
            ReadyDir::RxData => (fr & FR_RXFE) == 0,
        }
    }

    fn data_unit_write(&mut self, unit: u32) {
        self.regs.dr.write(unit & DR_DATA_MASK);
    }

    fn data_unit_read(&mut self) -> RxDatum {
        let raw = self.regs.dr.read();
        let unit = raw & DR_DATA_MASK;
        let mut bits = 0u32;
        if (raw & DR_OE) != 0 {
            bits |= RX_ERR_HW_OVERRUN;
        }
        if (raw & DR_BE) != 0 {
            bits |= RX_ERR_BREAK;
        }
        if (raw & DR_PE) != 0 {
            bits |= RX_ERR_PARITY;
        }
        if (raw & DR_FE) != 0 {
            bits |= RX_ERR_FRAMING;
        }
        RxDatum::new(unit, RxError::from_bits(bits))
    }
}

impl SerialRuntimeLineConfig for SerialPl011 {
    /// Procedural line-format apply (D04). Commits baud + framing; does NOT read
    /// back (I03). The compiler's choreography guarantees the UART is disabled
    /// before this runs (PL011 §3.3.8: reprogram LCR_H/IBRD/FBRD only while
    /// disabled).
    fn line_format_apply(&mut self, f: LineFormat) -> ConfigResult {
        // Encode WLEN; 9-bit framing is unrepresentable (WLEN is 2 bits).
        let wlen = match f.data_bits {
            DataBits::Five => 0u32,
            DataBits::Six => 1,
            DataBits::Seven => 2,
            DataBits::Eight => 3,
            DataBits::Nine => return ConfigResult::Unsupported,
        };
        let stp2 = match f.stop_bits {
            StopBits::One => 0u32,
            StopBits::Two => LCR_H_STP2,
        };
        let par = match f.parity {
            Parity::None => 0u32,
            Parity::Odd => LCR_H_PEN,
            Parity::Even => LCR_H_PEN | LCR_H_EPS,
        };
        let lcr_h = (wlen << LCR_H_WLEN_SHIFT) | LCR_H_FEN | stp2 | par;

        // Baud divisor (datasheet §3.3.6, round-to-nearest):
        // D*64 = (clk*8 + baud) / (2*baud); IBRD = D>>6, FBRD = D & 0x3f.
        let clk = UART_CLOCK_HZ as u64;
        let baud = f.baud as u64;
        let div64 = (clk * 8 + baud) / (baud * 2);
        let ibrd = (div64 >> 6) as u32;
        let fbrd = (div64 & 0x3f) as u32;

        regseq::log::info!("serialpl011 line: baud={}", f.baud);
        self.regs.ibrd.write(ibrd);
        self.regs.fbrd.write(fbrd);
        self.regs.lcr_h.write(lcr_h);
        self.regs.ifls.write(IFLS_VALUE);
        ConfigResult::Ok
    }
}

impl SerialInterruptDriven for SerialPl011 {
    /// Backend-called per-source mask op (C01/D03). The author owns the bits; the
    /// compiler owns WHEN. All `(source, state)` pairs covered.
    fn irq_source_set(&mut self, source: SerialIrqSource, state: IrqSourceState) {
        let mask = match source {
            SerialIrqSource::TxReady => TX_INTERRUPT_MASK,
            SerialIrqSource::RxData => RX_INTERRUPT_MASK,
            SerialIrqSource::LineError => ERROR_INTERRUPT_MASK,
        };
        match state {
            IrqSourceState::Armed => self.imsc_shadow |= mask,
            IrqSourceState::Disarmed => self.imsc_shadow &= !mask,
        }
        self.commit_imsc_shadow();
    }

    /// Decode pending causes from MIS; W1C-ack the line-error causes as part of
    /// decoding (no separate ack op; F1/B5).
    fn irq_cause_decode(&mut self) -> CauseSet {
        let status = self.regs.mis.read();
        let mut bits = 0u32;
        if (status & MIS_TX) != 0 {
            bits |= CAUSE_TX_READY;
        }
        if (status & MIS_RX) != 0 {
            bits |= CAUSE_RX_DATA;
        }
        if (status & MIS_RT) != 0 {
            bits |= CAUSE_RX_TIMEOUT;
        }
        if (status & ERROR_INTERRUPT_MASK) != 0 {
            bits |= CAUSE_LINE_ERROR;
            // W1C-ack the error causes (the only causes not self-clearing via the
            // data path or the mask).
            self.regs.icr.write(status & ERROR_INTERRUPT_MASK);
        }
        CauseSet::from_bits(bits)
    }
}

impl SerialQueueControl for SerialPl011 {
    /// Shift-register-inclusive TX-drain (T09 INCLUDE). Reads FR.BUSY + FR.TXFF —
    /// the same device fact PL011's Linux `tx_empty` / Zephyr `irq_tx_complete`
    /// report; NOT derivable from `data_ready_query(TxSpace)` (FR.TXFF alone).
    fn tx_drain_query(&self) -> bool {
        (self.regs.fr.read() & (FR_BUSY | FR_TXFF)) == 0
    }
}

// No `regseq_main`: the generic-c backend synthesizes `main`; the manifest's
// serial scenario drives bring-up → arm → TX → fire → RX → teardown.
