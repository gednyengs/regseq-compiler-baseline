#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// Per-OS synchronization lowering fixture (T07/C02): an AMBA-matched driver
// declaring all four lock/signal kinds, whose `on_init` operates each, so the
// linux_c + zephyr_c backends exercise the full C02 primitive matrix + the
// per-OS guard hidden state (Linux `flags` / Zephyr `key`) end-to-end (bridge
// recognition -> emit_{linux,zephyr}_c). Run ONLY by the dedicated
// `sync_os_e2e` binary (underscore corpus: never the generic 6-stage sweep).

include!("../../_lifecycle_surface.rs");
include!("../../_sync_surface.rs");

#[rseq::driver(name = "sync-os", bus = "amba",
    amba = [{ id = 0x00099002, mask = 0x000fffff }],
    compatible = ["acme,sync-os"])]
pub struct SyncOs {
    pub mtx: Mutex<u32>,
    pub spin: SpinLock<u32>,
    pub irqspin: IrqSpinLock<u32>,
    #[rseq::sync(cardinality = SingleWaiter)]
    pub done: Completion,
}

impl Lifecycle for SyncOs {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        // Sleeping mutex: scope-end release.
        {
            let _g = self.mtx.lock();
        }
        // Plain spinlock: scope-end release.
        {
            let _g = self.spin.lock();
        }
        // IRQ-safe spinlock with EXPLICIT early release (the guard-state
        // round-trip: irqsave writes `flags`, irqrestore consumes it).
        let g = self.irqspin.lock();
        g.guard_drop_early();
        // Completion ops (point events).
        self.done.signal();
        self.done.reset();
        let _ = self.done.wait(Duration::from_ms(10));
        Ok(())
    }
}
