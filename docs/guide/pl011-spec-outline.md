# PL011 Driver Spec Outline

Early implementation skeleton. Not a final DSL spec.

## Constructs used

- Register Access (register_block, Mmio, read/write/poll)
- Serial/UART Management (Serial device class trait)
- DMA Management (DmaChannel for TX/RX DMA)
- Synchronization (IrqSpinLock for ISR-shared state)
- Interrupt (single combined IRQ)
- Clock Management (uart_clk)
- Hardware Reset (optional reset line)
- Pin/GPIO Configuration (optional pinctrl)
- Firmware Property Access (reg-io-width, etc.)
- Driver Lifecycle (#[rseq::driver], Lifecycle trait)
- Power Management (#[rseq::pm])
- Diagnostic Logging (regseq::log)
- Timed Delay (regseq::delay)
- Timer Management (RX DMA poll timer)
- Atomic Operations (state flags)

## Likely driver struct

```rust
#[rseq::driver(name = "pl011-uart", compatible = ["arm,pl011"])]
#[rseq::device_class(serial)]
#[rseq::pm(idle = false, wakeup_capable = false)]
pub struct Pl011Uart {
    #[rseq::mmio(id = "uart_regs", access_width = 32)]
    regs: Mmio<Pl011Registers>,

    #[rseq::interrupt(id = "combined_irq")]
    irq: Interrupt,

    #[rseq::clock(id = "uart_clk")]
    clock: Clock,

    reset: Option<ResetLine>,
    pinctrl: Option<PinCtrl>,

    #[rseq::dma_channel(id = "tx_dma", direction = "memory_to_device")]
    tx_dma: Option<DmaChannel>,

    #[rseq::dma_channel(id = "rx_dma", direction = "device_to_memory")]
    rx_dma: Option<DmaChannel>,

    cb_lock: IrqSpinLock<CallbackState>,

    #[rseq::property(id = "reg-io-width", default = 4)]
    reg_io_width: u32,
}
```

## Key properties

- register_block: Pl011Registers (DR, RSR_ECR, FR, IBRD, FBRD, LCR_H, CR, IMSC, RIS, MIS, ICR, DMACR)
- access_width: 32
- clock: "uart_clk" with platform-specified frequency
- interrupt: single combined IRQ, level-triggered
- DMA channels: optional TX and RX
- pin states: "default" and "sleep"
- fifo_depth: 16 (platform file)
- default_baud_rate: platform-specified
- reg-io-width: 4 (default, overridable per variant)

## Key operations

- Serial trait: port_open, port_close, configure, tx_byte, rx_char, tx_ready, console_write
- ISR handler: read RIS, drain RX FIFO, handle TX, update modem status
- DMA: configure channel, prepare/submit/issue_pending for DMA-capable variants
- Lifecycle: init (disable UART, configure, enable), suspend/resume with clock gating

## Notable compiler-managed work

- Init preamble: map MMIO, enable clock, deassert reset, apply pinctrl, init shadows, extract properties, init sync primitives, request DMA channels, register interrupt
- Deinit postamble: reverse order
- Suspend postamble: disable interrupt, apply sleep pin state, disable clock
- Resume preamble: enable clock, apply default pin state, enable interrupt, flush shadows
