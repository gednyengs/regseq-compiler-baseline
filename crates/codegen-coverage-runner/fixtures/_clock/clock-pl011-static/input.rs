#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// The peripheral-clock-management M1 behavioral-oracle fixture (STATIC half,
// PLAN T13): the SAME driver spec as `clock-pl011-managed`, bound instead to a
// STATIC `[clock.uartclk]` (mode = "static", no provider). The mode lives in the
// platform binding, not the source — the identical author body lowers
// differently per the backend's resolved binding (D07, the backend-owned
// const-fold): `enable()`/`disable()` become no-ops (no `CLK enable`/`disable`
// trace) and `get_rate()` const-folds to `default_frequency` (no `CLK rate`
// hook). The compiler-managed `acquire_clock`/`release` are still emitted (they
// record the frequency; no provider lookup). `expected.stdout` shows the
// acquire/release framing, NO enable/disable/rate hooks, and the FOLDED rate in
// the `W…` write — the static behavioral oracle (D09), distinct from the managed
// pair member.

include!("../../_lifecycle_surface.rs");
include!("../../_clock_surface.rs");

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
        // Identical author body to the managed fixture; the static binding folds
        // enable() to a no-op and get_rate() to the const default_frequency.
        self.clk.enable()?;
        let rate = self.clk.get_rate();
        self.regs.dr.write(rate);
        Ok(())
    }

    fn on_deinit(&mut self) -> Result<(), RegseqError> {
        self.clk.disable();
        Ok(())
    }
}

// No `regseq_main`: the generic-c backend synthesizes `main`; the manifest's
// `lifecycle_scenario = "orderly"` drives the run.
