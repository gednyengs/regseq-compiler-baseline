#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// The peripheral-clock-management M1 behavioral-oracle fixture (MANAGED half,
// PLAN T08): the smallest valid clock spec on a PL011-shaped AMBA driver —
// construct-#1 register (DR) + ONE `#[rseq::clock(id = "uartclk")]` bound to a
// MANAGED `[clock.uartclk]` (mode = "managed" + provider). The compiler-managed
// `acquire_clock` rides the preamble (after the bus/map_region keystone); the
// author body `enable()`s the clock, reads its `get_rate()` (the live recorded
// rate — managed lowers to the host hook, D10/R10), and programs DR from it;
// `on_deinit` `disable()`s it and the compiler-managed `release` runs LIFO at
// teardown. `expected.stdout` is the CLK trace oracle (C05): `CLK acquire`
// before the author body, `CLK enable` before the first `W…`, the live rate,
// then `CLK disable` before `CLK release`.

include!("../../_lifecycle_surface.rs");
include!("../../_clock_surface.rs");

// AMBA-matched PL011-shaped driver with the bus-managed MMIO region joined to
// `[mmio.regs]` and the clock joined to `[clock.uartclk]`.
#[rseq::driver(name = "pl011clk", bus = "amba",
    amba = [
    { id = 0x00041011, mask = 0x000fffff },
    ],
    compatible = ["regseq,pl011clk"]
)]
pub struct Pl011Clk {
    #[rseq::mmio(id = "regs")]
    regs: Mmio<pl011::Regs>,
    #[rseq::clock(id = "uartclk")]
    clk: Clock,
}

impl Lifecycle for Pl011Clk {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        // The compiler-managed `acquire_clock` already ran (preamble). The
        // author enables the clock before touching the device, reads its rate,
        // and programs the data register from it.
        self.clk.enable()?;
        let rate = self.clk.get_rate();
        self.regs.dr.write(rate);
        Ok(())
    }

    fn on_deinit(&mut self) -> Result<(), RegseqError> {
        // Author quiesce: disable the clock. The compiler-managed `release`
        // runs AFTER this callback (LIFO, D16).
        self.clk.disable();
        Ok(())
    }
}

// No `regseq_main`: the generic-c backend synthesizes `main`; the manifest's
// `lifecycle_scenario = "orderly"` drives the run.
