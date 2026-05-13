
# Stage 2 Progress Report : E1000 (82540EM)

Stage 2 closed at **16 of 20 features**. The four outstanding items collapse onto **two underlying DSL gaps**: an array-of-registers IR variant (which gates three features: MTA, RAR, VFTA) and a deferred-execution primitive (which gates the TX-hang watchdog). 
Neither is an isolated workaround. Both are pre-existing requirements I had pinned as "stage-2 DSL extensions" in `e1000-spec-requirements.md`, and Stage 2 is the slot where they have to land. 

The spec-side body changes for the four blocked features take less than a day each *once the IR variants exist*. I expect both to close in the first half of next week, freeing the rest of next week for the start of Stage 3.

Stage 1 features remain green. Nothing in Stage 2 forced a regression in Stage 1.

## Part 1 : Completed features (16 of 20)

For each item I give (a) the smallest authoring change at the DSL surface, and (b) the compiler-side closure that made the lowering correct, since the latter is where most of the week actually went.

### 4. MSI with INTx fallback

**Author surface.** Already declarative on the driver struct:

```rust
#[rseq::interrupt(id = "e1000_irq", msi = true, fallback_legacy = true)]
pub irq: Interrupt,
```

The ISR body is unchanged from Stage 1 -- the `ICR.INT_ASSERTED` gate is correct in both lowering modes because 82540EM MSI ICR reads still surface the bit.

**Compiler closure.** Added a vector-negotiation preamble emitted before `Lifecycle::init`. Linux lowering: `pci_alloc_irq_vectors(pdev, 1, 1, PCI_IRQ_MSI | PCI_IRQ_LEGACY)` followed by a branch on the returned vector kind; Zephyr lowering: `pcie_msi_vector_connect` guarded by `pcie_msi_vector_alloc` success. The compiler now rejects `msix = true` on the 82540EM variant pin -- a declarative cross-check against the bus capability surface.

### 5. Suspend / resume (`PowerManagement`)

**Author surface.** New device-class-agnostic trait:

```rust
impl PowerManagement for E1000FullSpec {
    fn suspend(&mut self) -> Result<(), RegseqError> {
        self.mask_all_interrupts();
        self.regs.modify(rctl::REG, rctl::EN::DISABLED);
        self.regs.modify(tctl::REG, tctl::EN::DISABLED);
        self.drain_icr();
        for slot in self.tx_maps.iter_mut() { *slot = None; }
        for slot in self.rx_maps.iter_mut() { *slot = None; }
        Ok(())
    }

    fn resume(&mut self) -> Result<(), RegseqError> {
        self.reset_device()?;
        let mac = self.read_hardware_mac();
        self.bring_up_tx();
        self.bring_up_rx(mac)?;
        self.regs.write_field(ctrl::SLU::FORCE_LINK + ctrl::ASDE::ENABLED);
        self.regs.write_field(field_val(e1000::itr::INTERVAL, self.itr));
        self.unmask_default_interrupts();
        Ok(())
    }
}
```

**Compiler closure.** The interesting part is the **explicit non-preservation property**: I added

```rust
#[rseq::property(packet_state_volatility = "discard_on_suspend")]
```

so backends that target an OS expecting in-flight continuity (e.g. a hypothetical Hyper-V SR-IOV migration target) fail loudly at compile time rather than silently dropping frames. Linux/Zephyr both lower the trait pair to their native PM hooks (`struct pci_driver::{suspend,resume}` and `PM_DEVICE_DT_INST_DEFINE`).

### 8 (full). Link-state change via LSC IRQ

**Author surface.** Already in the ISR body:

```rust
if field_from_raw(icr::LSC, raw) != 0 {
    crate::regseq::net::set_link_state(self.observe_link());
}
```

**Compiler closure.** Added a liveness check: any ISR body referencing `set_link_state` must have LSC included in the unmasked-IMS set, otherwise the call is dead code at the device level. This caught a transient regression where I had reordered `unmask_default_interrupts()` ahead of LSC being added to `E1000_IMS_ENABLE_MASK` during a refactor.

### 15. TX completion via TXDW IRQ + cleanup loop (replaces Stage 1 #14)

**Author surface.** The `clean_tx` helper and the TXDW branch in `handle_irq` are the new completion site; the Stage 1 busy-poll on `STA.DD` inside `start_xmit` is deleted.

**Compiler closure.** A new MIR lint walks the body for `DescriptorAccess { field = STA }` reads inside loops without an interrupt-driven termination edge, and flags them. This caught one residual `expect("DD set within ring")` inside the cleanup loop that was masking an LBR-side underrun condition.

### 16 (full). TX queue wake from cleanup

**Author surface.** One line in the ISR:

```rust
crate::regseq::net::flow_control(FlowControl::TxOn);
```

**Compiler closure.** Pairing property. `flow_control(TxOff)` in any body method and `flow_control(TxOn)` from any cleanup/ISR path are now treated as a paired declarative obligation; the compiler emits a warning if only one half exists. Lowers to `netif_stop_queue`/`netif_wake_queue` (Linux) and `net_if_carrier_off`/`net_if_carrier_on` (Zephyr).

### 21 (full) and 39. RX moderation + IRQ throttle as tunables

**Author surface.** The three `#[rseq::param]` declarations were already on the struct (`rx_int_delay`, `rx_abs_int_delay`, `itr`); the closure was making them visible to the OS-side knob surface.

**Compiler closure.** Three small invariants:

1. A param's `default + min + max` propagate into the OS-side declaration (`module_param_named` + `MODULE_PARM_DESC` on Linux; Kconfig `int ... range A B` on Zephyr), so the bounds are enforced at module-load time, not at first MMIO write.
2. Params are sequenced before their first MMIO use in `Network::open`.
3. **Live re-tune is deliberately deferred to Stage 3.** A sysfs notifier hook into the body would be a Layer 1 surface expansion, and the workload-driven tuning evaluation (#40) is where it pays back. Documented as a deferred-property in the spec.

### 24. `set_mac_address`

**Author surface.** Already in the body, inside an `ordered!` block.

**Compiler closure.** Made the `ordered!` block's lowering contract explicit: MMIO writes inside the block do not reorder across it, and the compiler does not insert speculative RX-side refill operations between the disable and re-enable edges. Verified by emitting the lowered C and reading it against the SDM 14.5 ordering rule.

### 26. Promiscuous mode

**Author surface.** Already in `set_rx_mode`:

```rust
if mode.is_promiscuous() || !mode.multicast_groups().is_empty() {
    field = field + rctl::UPE::ENABLED + rctl::MPE::ENABLED;
}
```

**Compiler closure.** Added the OS-side rx-mode reflection intrinsic -- a uniform query over Linux's `IFF_PROMISC`/`IFF_ALLMULTI` net_device flags and Zephyr's `net_if_flag_is_set` equivalents -- so `regseq::net::current_rx_mode()` returns a normalized `RxModeSnapshot` regardless of backend.

### 30. `change_mtu`

**Author surface.** Body already validates against the variant-pin upper bound.

**Compiler closure.** Cross-check property: the compiler refuses to lower a `change_mtu` whose internal bound disagrees with `#[rseq::network(max_mtu = ...)]`. Caught a mid-week regression where the body had 1518 (frame-with-FCS) and the attribute had 1500 (MTU-proper) -- the kind of off-by-FCS bug that would have shipped silently.

### 31. RX flow control + 32. TX flow control

**Author surface.** Added inside `open` after `bring_up_rx`/`bring_up_tx`:

```rust
self.regs.write_field(ctrl::RFCE::ENABLED + ctrl::TFCE::ENABLED);
self.regs.write_field(field_val(fcal::VALUE, E1000_FC_PAUSE_DA_LO));
self.regs.write_field(field_val(fcah::VALUE, E1000_FC_PAUSE_DA_HI));
self.regs.write_field(field_val(fct::VALUE, E1000_FC_PAUSE_TYPE));
self.regs.write_field(field_val(fcttv::TTV, E1000_FC_DEFAULT_TTV));
self.regs.write_field(
    fcrtl::XONE::ENABLED + field_val(fcrtl::RTL, E1000_FC_LO_WATER),
);
self.regs.write_field(
    fcrth::XFCE::ENABLED + field_val(fcrth::RTH, E1000_FC_HI_WATER),
);
```

**Compiler closure.** New declarative property:

```rust
#[rseq::property(flow_control = "ieee802_3x")]
```

The compiler uses this both to (a) reject builds where the body programs flow-control registers but the device-class is configured without a `flow_control` declaration, and (b) advertise the capability to the OS via the appropriate backend hook (`ethtool_ops::get_pauseparam` on Linux; the corresponding capability bit on Zephyr).

### 34. Read-clear stats one-shot at init

**Author surface.** A single body call in `init`:

```rust
crate::regseq::stats::prime_counters();
```

**Compiler closure.** This was the most satisfying generalization of the week. Extended `register_block!` with a per-register `clear_on_read = true` attribute on the stats block, so the compiler can walk the register block and emit one `read_volatile` per clear-on-read register at the end of `init`. The PL011 fixture's `MIS` mask-clear semantics fell into the same lattice retrospectively -- I retro-fitted that fixture in 20 minutes after Stage 2 was done, with no body change.

### 35. Always-on counters

**Author surface.**

```rust
fn get_stats(&mut self) -> NetStats {
    NetStats {
        rx_packets: self.regs.read(gprc::REG) as u64,
        tx_packets: self.regs.read(gptc::REG) as u64,
        rx_bytes:   self.regs.read_widening_pair(gor::REG),
        tx_bytes:   self.regs.read_widening_pair(got::REG),
        ..NetStats::default()
    }
}
```

**Compiler closure.** Two pieces. First, a `#[rseq::widening_pair(low = gorcl, high = gorch)]` annotation on the register block, so the documented "low read latches high" semantic is encoded once in the register layout and consumed wherever the pair is referenced. Second, I relaxed the existing sequencing rule: MMIO reads inside a single struct-literal expression are now commit-ordered left-to-right rather than each read being a side-effecting statement -- this lets `get_stats` read like a value computation, not a sequence of imperative reads with `let _ =` intermediates.

### 36. Error counters

**Author surface.** Three additional fields in `NetStats`, read from `CRCERRS`, `RXERRC`, `MPC`.

**Compiler closure.** A `#[rseq::stat_semantics(...)]` annotation per stats register that names which `NetStats` field the register maps to, separating the body from the OS-specific stat-name lattice. `MPC` (missed packet count, an RX FIFO underflow indicator) lowers to `rx_dropped`, not `rx_errors`, to match `ethtool -S` convention -- the kind of decision the compiler should own once and the body should never repeat.

### 42. RXO recovery + dropped-packet accounting

**Author surface.** In the ISR:

```rust
if field_from_raw(icr::RXO, raw) != 0 {
    self.rx_dropped_shadow = self.rx_dropped_shadow.saturating_add(1);
    self.regs.write_field(field_val(
        rdt::VALUE,
        (self.next_rx_desc + self.rx_ring_size - 1) % self.rx_ring_size,
    ));
}
```

The shadow folds into `get_stats` (`rx_dropped += rx_dropped_shadow`) so the OS sees both the device-reported and driver-observed drops.

**Compiler closure.** A coverage lint on the ISR: for every cause bit unmasked in the IMS shadow at `open`, the ISR body must have at least one read-or-effect on the corresponding ICR field. I had missed RXO; the lint flagged it. This is exactly the kind of pre-existing-property check I want the compiler to be doing across all device classes.

---

## Part 2 -- Outstanding features (4 of 20)

The unifying observation: every outstanding feature is gated on a **DSL primitive** that does not yet exist. None of them is gated on a body authoring difficulty. I want to flag this because it changes how I think about what "Stage 2 incomplete" means: the spec-side authoring for these four items is between half a page and a page each. The work that did not happen this week is **the IR variant that makes those pages compile**.

### 27. Multicast hash filter (MTA[128])

**The gap.** `register_block!` currently expresses named registers and `Reserved<u32> [N]` fillers, but not a first-class typed array of registers. MTA is 128 sequential u32 registers backing a 4096-bit hash table.

**What I drafted.** The macro extension compiles; the body-side accessor reads naturally:

```rust
mta: ReadWrite[128] {
    VALUE OFFSET(0) NUMBITS(32) [],
},

// body
self.regs.array_set_bit(mta::TABLE, bucket / 32, bucket % 32);
```

**Where the week ran out.** The MIR did not carry a way to encode "indexed register access at a typed array site". I had been folding it into a runtime `Mmio::write_at_offset(base + index * 4)`, which is a layering violation -- it punctures the abstraction that register identity is a static property and turns it into a runtime address computation that the backends cannot reason about.

**The compiler-theory move.** Per the standing guidance to prefer proper IR variants over narrow workarounds, the right addition is a new MIR node:

```
RegisterAccess::Indexed { base: RegArrayId, stride: usize, index: Operand }
```

alongside the existing `RegisterAccess::Direct { field }`. The lowering pass projects the indexing into target idioms (`iowrite32(base + (i << 2))` on Linux, `sys_write32(...)` on Zephyr), and a verifier obligation lifts the bounds check to a const-eval over the array length when the index is statically bounded -- which it is at every authored call site, since hash buckets fall out of an Ethernet-CRC32 over the MAC and are therefore in `0..4096`.

**Plan.** Land the IR variant Monday. MTA closes Tuesday in one body change (CRC32 hash + bit-set).

### 28. Multi-MAC unicast (RAR[1..15])

**The gap.** Same family as #27, but with a subtle aggravation: RAR is an array of **pairs** (RAL at +0, RAH with field structure at +4), not a single-stride array.

**Why it is separately tricky.** The macro extension I drafted for MTA assumes "a register has fields" and a uniform stride. RAR forces a "register *entry* has multiple word offsets, each with their own field cluster". The right shape is:

```rust
rar: ReadWrite[16] of RarEntry {
    ral { VALUE OFFSET(0) NUMBITS(32) [] }       at +0,
    rah { ADDR_HIGH OFFSET(0) NUMBITS(16) [],
          AS OFFSET(16) NUMBITS(2)  [DEST_ADDR = 0, SRC_ADDR = 3],
          AV OFFSET(31) NUMBITS(1)  [INVALID = 0, VALID = 1] } at +4,
},
```

This is more invasive than the MTA case because the `FieldValue` algebra (the `+` operator) is currently single-register; once entries are structured, it has to become aware of register identity within a strided entry so `rah::AV::VALID + rah::AS::DEST_ADDR` does not accidentally form an additive value at the RAL offset.

**Plan.** This is genuinely the trickier of the two register-array cases. I will land MTA first to prove the single-stride lattice, then generalize to entry-shaped strides for RAR.

### 29. VLAN filter (VFTA + RCTL.VFE)

**The register half.** Trivial -- VFTA[128] is the same shape as MTA, indexed by VLAN ID directly (no hash needed; 12-bit VID gives exactly 4096 buckets). Comes free with #27.

**The harder half: the OS-event lattice.** The driver has to observe network-stack VLAN subscription changes:

- Linux: `ndo_vlan_rx_add_vid` / `ndo_vlan_rx_kill_vid` -- per-VID delta callbacks
- Zephyr: VLAN events on the net-stack event queue
- FreeBSD: `IFCAP_VLAN_HWFILTER` plus the per-VID add/del ioctls

The Layer 1 `Network` trait does **not** currently expose a VLAN-subscription callback; it exposes only `set_rx_mode`, which today conflates promiscuity, multicast, and (would-conflate) VLAN into one "tell me what to accept" idempotent function.

**The compiler-theory tension.** The device-class spec rule is that `set_*` methods are **idempotent full reprograms**, not deltas. The per-OS callbacks for VLAN are **deltas**. Reconciling them on the driver side via a shadow `VlanSet` and diffing against the new set is straightforward (it is the same shape as the IMS shadow), but I want to write that reconciliation down as a **property of Layer 1** before shipping it -- otherwise the next driver author who hits a similar mismatch (per-MAC-filter add/del in 10G NICs, for instance) will not know whether to author idempotently or differentially.

**Plan.** Add `Network::set_vlan_filter(&mut self, vlans: &VlanSet)` as an idempotent reprogram. Backend lowering owns the delta computation against a per-driver `VlanSet` shadow that the compiler synthesizes -- the driver author never sees the add/del split. Body change after that is half a page.

### 41. TX-hang watchdog

**The gap.** There is no `#[rseq::timer]` attribute, because there is no deferred-execution primitive in the DSL yet.

**Why it is more than a syntactic addition.** A timer body fires in an OS-chosen context that does not necessarily hold the same locks as `start_xmit`. The body must declare which device-state it observes vs. mutates, and the compiler must emit a per-OS synchronization preamble around timer entry. This is a **new ownership story** -- not just a new attribute.

**The construct family.** This is structurally the same problem as deferred RX servicing (#22, Stage 3 / NAPI). Both want:

```
DeferredWork {
    trigger:  Periodic { period: Duration }
            | IrqContinuation { source: IrqCause, budget: usize },
    body:     BodyMethod,
    context:  { observes: [State...], mutates: [State...] },
}
```

Lowered to `timer_list` / `delayed_work` (Linux), `k_work_delayable` (Zephyr), `callout` (FreeBSD); and for `IrqContinuation` to NAPI / DPC / iflib taskqueue respectively.

**Plan -- and why this is the right ordering.** Building this construct for the periodic-timer case **before** NAPI is the correct sequencing: the periodic trigger is the simpler instance, and proving the construct against it lets me write down the `observes/mutates` typing rules before the more complex interrupt-continuation case forces them under throughput pressure. This was an unplanned realization mid-week, and it is why #41 slipping into next week is not a setback -- it lets the same IR construct also unlock the headline Stage 3 win (#22) without a redesign.

---

## What I am taking away

- The two outstanding gaps were both known going in (`e1000-spec-requirements.md` Section 7). Stage 2 is the slot where they had to surface; they did.
- The closures that were genuinely satisfying this week were the ones where a device-specific need produced a **device-class-agnostic property** that other devices can reuse: `clear_on_read` on stats registers (PL011 also benefits), `widening_pair` for split 64-bit counters, the IRQ-cause-vs-IMS coverage lint, the `flow_control` pairing property. These are the artifacts I want the paper to feature.
- The register-array IR variant is the single highest-leverage unblock for next week. Three Stage 2 features (#27, #28, #29) depend on it, and it removes one of the open issues I was deferring out of Stage 2 entirely.
- The deferred-execution construct is the second-highest-leverage. One Stage 2 feature (#41) depends on it; one Stage 3 feature (#22, NAPI) is the largest single throughput win in the paper claim. Proving it on the timer case first is cheap insurance.
