#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// A driver that recognizes CLEAN (valid `Clock` + `#[rseq::clock(id="uartclk")]`
// + an MMIO region); the binding reject is driven entirely by the fixture's
// `platform.toml` `[clock.uartclk]` section.
include!("../../_lifecycle_surface.rs");
include!("../../_clock_surface.rs");

#[rseq::driver(name = "clkbind", bus = "amba",
    amba = [
    { id = 0x00041011, mask = 0x000fffff },
    ],
    compatible = ["regseq,clkbind"]
)]
pub struct ClkBind {
    #[rseq::mmio(id = "regs")]
    regs: Mmio<pl011::Regs>,
    #[rseq::clock(id = "uartclk")]
    clk: Clock,
}

impl Lifecycle for ClkBind {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        self.clk.enable()?;
        let _ = self.clk.get_rate();
        Ok(())
    }
}
