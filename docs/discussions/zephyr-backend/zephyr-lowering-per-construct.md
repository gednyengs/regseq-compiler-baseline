# Zephyr lowering — the five constructs in depth

The normative per-construct lowering prose for the Zephyr backend, pinned to
**Zephyr 3.7 LTS / 4.x**. Each construct section is self-contained: shape of
the lowering, the per-primitive normative emissions, the scaffold seams, the
error/lifetime channels, and the composition invariants. The consolidated
trace rows live in
[`zephyr-traceability-matrix.md`](./zephyr-traceability-matrix.md); the open
decisions referenced here (`OQ-*`, `O*`, `Q*`, `ZE-OQ*` — per-construct local
labels) are gathered with recommendations in
[`zephyr-open-questions.md`](./zephyr-open-questions.md).

**In-repo evidence base (shared by all five sections).** Two vendored stock
Zephyr drivers anchor every idiom claim: the PL011 UART
(`reference-drivers/pl011/zephyr/{uart_pl011.c,uart_pl011_registers.h,arm_pl011.yaml}`,
the platform/devicetree path) and the Intel e1000 NIC
(`reference-drivers/e1000/zephyr/{eth_e1000.c,eth_e1000_priv.h,intel,e1000.yaml,Kconfig.e1000}`,
the PCIe path). In-repo witnesses are preferred over doc URLs throughout.

---

## A. Register-access (whole-construct)

**OS pin:** Zephyr 3.7 LTS / 4.x (all out-of-repo citations pinned to the
`v3.7-branch` source tree). **Scope:** exactly the nine register-access
inventory rows — the five `MmioOp` intrinsics, `poll_until`, `map_region`, the
`Mmio<T>`/`#[rseq::register_block]` offset-recovery mechanism, and the
`Field`/`FieldValue` descriptors. **Reference lowering:** generic_c
(`crates/regseq-backends/src/generic_c/`). **Authoritative surface:**
`docs/discussions/register-access/register-access-surface-convergence.md`.

**In-repo Zephyr witnesses** (preferred over doc URLs): the stock PL011 driver
`reference-drivers/pl011/zephyr/{uart_pl011.c,uart_pl011_registers.h}`
(devicetree/platform path, `DEVICE_MMIO_*`) and the stock e1000 driver
`reference-drivers/e1000/zephyr/{eth_e1000.c,eth_e1000_priv.h}` (PCIe path,
`sys_read32`/`sys_write32` over a mapped base + byte-offset table).

### A.1 Shape of the lowering

generic_c's agnostic core reduces every register-access operation, at emit
time, to a pair `(byte offset, access width)` recovered from the `#[repr(C)]`
block layout (`body_emit.rs:2439-2530`) plus, for field ops, `mask`/`shift`/`value`
read off the `Field`/`FieldValue` descriptor argument (`body_emit.rs:2292-2295`,
`:2308-2310`, `:2317-2322`). The op then becomes a call against a base-relative
accessor pair (`regseq_bus_mmio_read/write(off, width)` over the installed
`regseq_bus_mapped_base`, `prelude.rs:182`, `:233-239`; selection at
`body_emit.rs:2271-2281`).

This is, byte for byte, the dominant Zephyr MMIO idiom: the stock e1000 driver
keeps a register table as a **byte-offset enum** (`eth_e1000_priv.h:41-61`) and
accesses it as `sys_read32((_dev)->address + (_reg))` /
`sys_write32(_val, (_dev)->address + (_reg))` (`eth_e1000_priv.h:118-128`),
where `address` is an `mm_reg_t` mapped base (`eth_e1000_priv.h:87`). The
Zephyr lowering therefore **reuses the entire agnostic core unchanged** — Call
interception (`body_emit.rs:2113-2116`), `recover_register` (`:2439-2530`,
incl. register arrays via `Index`-term stride expressions and the
width-∈-{1,2,4,8} guard `:2514-2520`), descriptor reads, the mask-confinement
semantics, and the C-side RMW shape — and swaps exactly three things:

1. **Accessor:** the host trace hooks → Zephyr's `sys_read{8,16,32,64}` /
   `sys_write{8,16,32,64}` (`include/zephyr/sys/sys_io.h`, pinned:
   <https://github.com/zephyrproject-rtos/zephyr/blob/v3.7-branch/include/zephyr/sys/sys_io.h>;
   32-bit witness in-repo at `eth_e1000_priv.h:118-128`).
2. **Base carrier:** the TU-global `regseq_bus_mapped_base` (`prelude.rs:182`)
   → the device's MMIO mapping, `mm_reg_t` obtained from `DEVICE_MMIO_GET(dev)`
   (`uart_pl011_registers.h:40-43`) or a `device_map`-filled handle field
   (`eth_e1000.c:223-224`, `eth_e1000_priv.h:87`).
3. **`map_region`:** the `regseq_bus_map_region` trace helper
   (`prelude.rs:224-232`) → `DEVICE_MMIO_MAP(dev, K_MEM_CACHE_NONE)`
   (devicetree/platform, `uart_pl011.c:577`) or
   `device_map(&handle, phys, size, K_MEM_CACHE_NONE)` (PCIe,
   `eth_e1000.c:223-224` — deferred, see OQ-4).

No register-access **op** changes when the base/mapping source changes — that
is the construct's own keystone invariant (I04, `prelude.rs:158-162`;
`device_lifecycle.rs:203-208`), and it is what makes this three-point swap
sufficient.

### A.2 The base seam — `map_region` and the `DEVICE_MMIO` scaffold

#### A.2.1 Scaffold (devicetree/platform path — the in-scope path)

Zephyr's canonical single-region MMIO scaffold, witnessed end-to-end in the
stock PL011 driver (`include/zephyr/sys/device_mmio.h`, pinned:
<https://github.com/zephyrproject-rtos/zephyr/blob/v3.7-branch/include/zephyr/sys/device_mmio.h>):

| Scaffold point | Zephyr emission | Witness |
|---|---|---|
| config struct, **first member** | `DEVICE_MMIO_ROM;` | `uart_pl011.c:42-43` |
| data struct, **first member** | `DEVICE_MMIO_RAM;` | `uart_pl011.c:63-64` |
| per-instance config initializer | `DEVICE_MMIO_ROM_INIT(DT_DRV_INST(n))` — pulls `(base, size)` from the DT node's `reg` property | `uart_pl011.c:758-759` |
| init function, before any op | `DEVICE_MMIO_MAP(dev, K_MEM_CACHE_NONE);` | `uart_pl011.c:577` |
| every op site | base = `DEVICE_MMIO_GET(dev)` (an `mm_reg_t`) | `uart_pl011_registers.h:40-43` |

`K_MEM_CACHE_NONE` is the witnessed cache attribute on **both** in-repo Zephyr
reference drivers (`uart_pl011.c:577`, `eth_e1000.c:223-224`) and is the
normative choice.

#### A.2.2 `map_region` placement and the bus-preamble composition

generic_c synthesizes `map_region` **inside the lifecycle preamble**, after the
BRM ops, in canonical order
`enable_device → claim_regions → set_bus_master → get_resource_handle → map_region`
(`body_emit.rs:5250-5284`), itself after the per-resource-field preamble
(`body_emit.rs:5209-5217`) and before the instance + `on_init`
(`body_emit.rs:5301-5311`). The Zephyr lowering **preserves this order on the
platform path**; the PCIe path carries exactly one recorded order deviation
(BAR probe before enable, §B.3.5 — pending ratification, A-3). On a
devicetree/platform device the slots collapse as follows:

- `enable_device` — not emitted: PCI-gated already in the model/backend
  (`device_lifecycle.rs:96-104`; `body_emit.rs:5256-5259`). Nothing
  Zephyr-specific to add.
- `claim_regions` — **no runtime realization on Zephyr**: the region is
  resolved/sized, never reserved — stated as-built in `prelude.rs:202-208`
  ("Zephyr has no runtime claim"). The ordering slot is retained (it is BRM's
  row); register-access only requires that `map_region` still follows it.
- `set_bus_master` — not emitted (`dma_present` defaults false;
  `body_emit.rs:5267-5270`). On Zephyr-PCIe the enable/master fusion is
  `pcie_set_cmd(bdf, PCIE_CONF_CMDSTAT_MEM | PCIE_CONF_CMDSTAT_MASTER, true)`
  (`eth_e1000.c:220-221`) — owned by the BRM Zephyr section (§B), deferred
  with OQ-4.
- `get_resource_handle` — unconditional (`body_emit.rs:5271-5272`); the Zephyr
  realization is the `const struct device *dev` parameter + its
  `DEVICE_MMIO_ROM` config slot — the handle `DEVICE_MMIO_MAP` consumes
  (`uart_pl011.c:570-577`).
- **`map_region`** — `DEVICE_MMIO_MAP(dev, K_MEM_CACHE_NONE)` emitted in the
  **canonical slot**: after the side-resource preambles, before any lowered
  MMIO op and the authored `on_init` body. The only ordering constraint the
  stock driver documents is the weaker one — the regs pointer "Must be placed
  after DEVICE_MMIO_MAP" (`uart_pl011.c:577-580`). PL011 itself places the map
  textually *first*, before its reset/clock/pinctrl preamble
  (`uart_pl011.c:577` vs `:582-609`); that layout satisfies the witnessed
  constraint, but generated code keeps generic_c's canonical
  preambles-then-map order (`body_emit.rs:5209-5217`, `:5273-5284`) — the
  witness deviation is noted, not adopted. This mirrors generic_c's guarantee
  that `map_region` precedes the author body in every scenario.

So the synthesized Zephyr init body is: *(resource preambles)* →
`DEVICE_MMIO_MAP` → *(instance)* → authored `on_init` — which is structurally
`pl011_init` (`uart_pl011.c:570-662`: map at `:577`, authored configuration
after).

**`(base, size)` provenance (OQ-1).** generic_c hands `map_region` the
platform.toml-resolved `MmioBinding.base/.size` literals
(`device_lifecycle.rs:209-229`, `:236-239`; `body_emit.rs:5277-5284`). On
Zephyr the idiomatic source is the devicetree `reg` property via
`DEVICE_MMIO_ROM_INIT(DT_DRV_INST(n))` (`uart_pl011.c:758-759`), which requires
a `DT_DRV_COMPAT` (`uart_pl011.c:9`) — i.e. it is keyed by the device-lifecycle
`MatchEntry::Devicetree` compatible, not by `[mmio.<id>]` literals. The
keystone invariant says this swap changes no op (`prelude.rs:158-162`);
**which source is normative** (DT-only, literal-only via `device_map`, or
DT-with-literal-cross-check) is OQ-1.

**Single region.** generic_c enforces ≤1 resolved region per driver
(`body_emit.rs:5240-5243`, `max_resolved_bindings_per_driver`); the non-named
`DEVICE_MMIO_*` API is likewise exactly single-region (first-member constraint,
`uart_pl011.c:42-43`, `:63-64`). The constraints align; multi-region would
graduate to `DEVICE_MMIO_NAMED_*` (out of scope this stage).

**Base threading into op sites (OQ-3).** generic_c's accessors read a TU-global
base (`prelude.rs:182`), legal because of the single-region rule and the single
lifecycle instance (`body_emit.rs:5301-5306`). The faithful first Zephyr
realization is the same: a TU-static `mm_reg_t` installed immediately after
`DEVICE_MMIO_MAP`. The idiomatic end-state is per-device — every op site
computes the base as `DEVICE_MMIO_GET(dev)` — but that requires the device
pointer to reach lowered author bodies, which is the device-lifecycle
container-morph seam (callbacks run spec-view over `&inst.inner`,
`body_emit.rs:5301-5306`), not register-access's to decide. Recorded as OQ-3.

**Hard constraint (post-review, Codex issue 2).** A TU-static base is sound
only when the TU instantiates **exactly one device**: generic_c's legality
argument is precisely the single-region + single-lifecycle-instance pair
(`body_emit.rs:5301-5306`), while `DT_INST_FOREACH_STATUS_OKAY` stamps one
device per status-okay DT node (§C.1) — with two or more instances a TU-static
base silently aliases the last-mapped device across all of them. The faithful
realization therefore MUST emit
`BUILD_ASSERT(DT_NUM_INST_STATUS_OKAY(DT_DRV_COMPAT) == 1, "TU-static MMIO base requires a single instance");`
so a multi-instance devicetree **fails the C build** instead of miscompiling
(macro pinned: <https://docs.zephyrproject.org/3.7.0/build/dts/api/api.html>;
fetch-unverified, open-questions C-9). Multi-instance support arrives only
with the per-device `DEVICE_MMIO_GET(dev)` threading (the OQ-3/A-2 graduation
path).

### A.3 The five MMIO ops

#### A.3.1 Accessor selection by recovered width

`recover_register` validates width ∈ {1, 2, 4, 8} bytes
(`body_emit.rs:2514-2520`); the width selects the accessor:

| width | read | write |
|---|---|---|
| 1 | `sys_read8(addr)` | `sys_write8(val, addr)` |
| 2 | `sys_read16(addr)` | `sys_write16(val, addr)` |
| 4 | `sys_read32(addr)` | `sys_write32(val, addr)` (in-repo witness `eth_e1000_priv.h:118-128`) |
| 8 | `sys_read64(addr)` | `sys_write64(val, addr)` — **arch-conditional**, OQ-2 |

Note the Zephyr write argument order is **value first, address second**
(`eth_e1000_priv.h:120`) — inverted from the generic_c hook signature
(`prelude.rs:96`).

#### A.3.2 Per-op normative shapes

With `m` = the mapped base lvalue (§A.2.2/OQ-3), `OFF` = the `recover_register`
offset expression (a literal, or `(const + (uint64_t)(i) * stride)` for arrays,
`body_emit.rs:2521-2528`), `B` = width×8, and `CTy` the destination's C type —
each shape is the exact generic_c emission (`body_emit.rs:2282-2323`) with the
hook swapped for the `sys_io` accessor:

- **`mmio_read`** (`MmioOp::Read`, gc `:2297-2302`):
  `dst = (CTy)sys_read{B}(m + OFF);`
- **`mmio_read_field`** (`MmioOp::ReadField`, gc `:2303-2311`):
  `dst = (CTy)((sys_read{B}(m + OFF) & (fld).mask) >> (fld).shift);`
- **`mmio_write`** (`MmioOp::Write`, gc `:2283-2287`):
  `sys_write{B}((uint{B}_t)(val), m + OFF);`
- **`mmio_write_field`** (`MmioOp::WriteField`, gc `:2288-2296`):
  `sys_write{B}((uint{B}_t)((fv).value & (fv).mask), m + OFF);` — the
  mask-confinement (`value & mask`) is the **locked** `write_field` semantics
  (bits outside the mask = 0, convergence doc §2.4 + the gc comment
  `:2289-2292`) and is preserved verbatim.
- **`mmio_modify`** (`MmioOp::Modify`, gc `:2312-2323`):
  `{ uint{B}_t cur = sys_read{B}(m + OFF); sys_write{B}((uint{B}_t)((cur & ~(fv).mask) | ((fv).value & (fv).mask)), m + OFF); }`
  — the RMW stays **compiler-emitted C** (no generic Rust bit-op lowering),
  exactly the gc shape, and matches the stock Zephyr idiom (open-coded RMW,
  `uart_pl011.c:121`, `:142-157`).

Intermediate arithmetic narrows from generic_c's `uint64_t` hooks to the
access-width-native `uint{B}_t`; bit-identical because `cur`, `mask`, and the
confined `value` all fit in B bits (width = the wrapper element size,
`body_emit.rs:2514-2515`).

#### A.3.3 Semantic notes (contract-preserving)

- **Volatility / "not a memory image."** The convergence contract requires
  every access to be volatile MMIO, never a cached struct load
  (`register-access-surface-convergence.md:82-86`, Deref contract `:212-214`).
  `sys_read*/sys_write*` are volatile accessor functions (`sys_io.h`, pinned
  URL above); the lowering never emits a struct-member load on the block type —
  the block layout exists only at emit time inside `recover_register`. (The
  stock PL011 driver uses the *other* accepted Zephyr idiom, a
  `volatile struct` pointer, `uart_pl011_registers.h:39-43` — equally volatile,
  but it does not match the `(offset, width)` form the agnostic core produces,
  so it is not the lowering target.)
- **RMW atomicity** — not provided, matching generic_c (plain read-then-write,
  `body_emit.rs:2312-2323`) and the stock drivers (unlocked `|=`,
  `uart_pl011.c:121`). Locking/execution-context is an explicit construct
  scope exclusion (composition-over-absorption, convergence doc `:63-66`).
- **Memory barriers** — none emitted. The model carries no ordering construct
  (`ordered!` deferred, convergence doc `:63-66`); the stock PL011 driver's
  explicit `barrier_dmem_fence_full()` around the baud-rate sequence
  (`uart_pl011.c:179`, `:644-647`) is therefore **outside** what this construct
  can express today. Honest gap, inherited from the model, not a
  Zephyr-lowering defect.
- **`side_effects`/`hw_updates` metadata** (`#[rseq::register]`) is
  inert/extraction-consumed (convergence doc `:160-163`); generic_c attaches no
  backend behavior, so neither does the Zephyr lowering.

### A.4 Offset recovery and descriptors (the OS-agnostic core, reused)

- **`Mmio<T>` + `#[rseq::register_block]` offset recovery** is emit-time
  machinery, not OS surface: the receiver back-trace + projection walk
  (`body_emit.rs:2439`, field accumulation + type cross-check `:2461-2485`,
  array `Index`/`ConstantIndex` stride terms `:2487-2506`, width guard
  `:2514-2520`) runs **unchanged** in the Zephyr backend; only the emitted call
  around `(OFF, width)` differs. All fail-loud paths are preserved. The result
  — a register table as base-relative byte offsets — is precisely the stock
  Zephyr e1000 register model (`eth_e1000_priv.h:41-61`).
- **`Field`/`FieldValue` descriptors** are ordinary model ADTs (closed op set =
  exactly the five `MmioOp`s, `domain_attr.rs:125-136`; no `MmioOp::Set`); they
  lower as the same const aggregates and feed `.mask/.shift/.value` into the
  §A.3.2 expressions, exactly as gc does (`body_emit.rs:2282`, `:2292-2295`,
  `:2308-2310`, `:2317-2322`). Zephyr's `FIELD_PREP`/`FIELD_GET`/`GENMASK`
  (used at `uart_pl011.c:630-631`) are a cosmetic alternative spelling only —
  not adopted, to keep the emission identical to the reference lowering.

### A.5 `poll_until` — recognized, NOT lowered

`regseq::poll::until` is an `IntrinsicKind` the bridge recognizes
(`domain_attr.rs:94-95`, `:172`;
`crates/regseq-driver/src/bridge/intrinsic/recognition.rs:90`, `:117`) but
**generic_c has no emission for it** (convergence doc §9 caveats: "its
generic-c lowering is not yet implemented — do not claim it"). Per the
citation rule, this spec does **not** invent a Zephyr lowering. For the
eventual lowering task only: Zephyr's native shape is
`WAIT_FOR(cond, timeout_us, k_busy_wait(delay_us))`
(`include/zephyr/sys/util.h`, pinned:
<https://github.com/zephyrproject-rtos/zephyr/blob/v3.7-branch/include/zephyr/sys/util.h>),
with the stock PL011 witnessing the untimed busy-poll degenerate form
(`uart_pl011.c:224-226`). Its `Result<(), RegseqError>`/`Timeout` channel would
compose with the RegseqError construct's Zephyr mapping — all deferred (OQ-5).

### A.6 Error and lifetime channels

- **The five ops are infallible** on `Mmio<T>` (direct class; convergence doc
  §2.4) — no error channel on Zephyr either; `sys_read*/sys_write*` return the
  value / `void`.
- **`map_region` is infallible at the call site on Zephyr-DT**:
  `DEVICE_MMIO_MAP` returns no status (`device_mmio.h`, pinned URL above;
  witnessed used statement-style with no check, `uart_pl011.c:577`). Contrast
  Linux (`pci_iomap` → `NULL` / `devm_ioremap_resource` → `ERR_PTR`): there is
  **no native map-failure path to wire** into the init-fail scenario on the DT
  path. The Zephyr init function's `int` return (negative errno, e.g.
  `-EINVAL` `uart_pl011.c:173`, `-ENODEV` `eth_e1000.c:215-217`) is the
  **native OS error channel**, distinct from authored `RegseqError` — same
  two-channel separation as generic_c (the errno exit-adapter is the
  RegseqError construct's seam, not register-access's).
- **Lifetime:** the mapping persists for the device's lifetime. As-built
  generic_c emits map only — no unmap helper exists (`prelude.rs:175-232` has
  no unmap; the preamble emits only `regseq_bus_map_region`,
  `body_emit.rs:5273-5284`) — and stock Zephyr drivers likewise never unmap
  (no remove path in `DEVICE_DT_INST_DEFINE`, `uart_pl011.c:789-791`). The
  behaviors align; the resource **postamble** slot (reverse-order,
  `body_emit.rs:5391-5406`) carries no register-access unmap action on Zephyr,
  matching generic_c.

### A.7 Composition invariants the Zephyr backend MUST preserve

1. **Preamble order** `[BRM ops] → map_region → on_init`, with `map_region`
   strictly before the first lowered MMIO op (`body_emit.rs:5250-5311`; Zephyr
   witness `uart_pl011.c:577-580`).
2. **I04 invariance**: changing the `(base,size)` source (platform.toml
   literal ↔ DT `reg` ↔ PCIe BAR) changes no register-access op
   (`prelude.rs:158-162`, `device_lifecycle.rs:203-208`).
3. **Closed op set**: exactly the five `MmioOp`s (`domain_attr.rs:125-136`);
   no width other than 1/2/4/8 (`body_emit.rs:2514-2520`).
4. **Locked semantics**: `write_field`/`modify` mask-confinement and RMW
   preserve-unmentioned-bits (`body_emit.rs:2288-2296`, `:2312-2323`;
   convergence doc §2.4/§9).
5. **Single resolved region per driver** (`body_emit.rs:5240-5243`) ↔ non-named
   `DEVICE_MMIO` single-region scaffold (§A.2.1).
6. **Fail-loud offset recovery** — every `recover_register` rejection path
   retained verbatim (`body_emit.rs:2439-2530`).

---

## B. Bus-resource-management

**Scope.** The Zephyr lowering of exactly the BRM primitives:
`register_driver` (the bus-path selector), the four compiler-managed preamble
ops (`enable_device`, `claim_regions`, `set_bus_master`,
`get_resource_handle`), the config-space intrinsics
(`read_config`/`write_config`), and the carrier types (`MmioBinding`,
`BusType`, `BarType::Mem`/`BarType::Io`). `map_region` is register-access's
primitive and out of scope to lower, but it is the downstream consumer of
BRM's resolved `(base,size)`, so the composition seam is specified where it
touches BRM
(docs/discussions/bus-resource-management/bus-resource-management-surface-convergence.md:144-150, I04).

**In-repo Zephyr evidence base.** Two vendored stock drivers anchor every
idiom claim: the PL011 UART (platform/DT path:
reference-drivers/pl011/zephyr/uart_pl011.c) and the Intel e1000 NIC (PCIe
path: reference-drivers/e1000/zephyr/eth_e1000.c). The e1000 port **resolves
the previously-open "Zephyr-over-PCI" question affirmatively**: Zephyr has a
first-class PCIe host layer (`#include <zephyr/drivers/pcie/pcie.h>`,
eth_e1000.c:18) and a complete bring-up idiom (eth_e1000.c:208-262), so
`BusType::Pci` is lowerable on Zephyr — version-pinning caveats in §B.7.

### B.1 The structural pivot — a linear preamble becomes static instantiation + an init fn

In `generic_c` the whole bring-up is one flat scenario function
(crates/regseq-backends/src/generic_c/body_emit.rs:5191-5285):
`regseq_lc_register` → `regseq_bus_register_driver(sel, bound)` (:5196-5205) →
create/associate → resource preambles (:5209-5217) → the gated bus block
`enable_device → claim_regions → set_bus_master → get_resource_handle → map_region`
(:5244-5285) → `on_init`.

On Zephyr this splits along a different seam than Linux. Linux lifts
`register_driver` to runtime module-init
(docs/discussions/linux-backend/linux-lowering-per-construct.md:242-253);
Zephyr has **no runtime registration at all** — a driver is a set of
**statically instantiated** device objects, one per enabled DT node, expanded
at C compile time by `DEVICE_DT_INST_DEFINE(n, init_fn, …)` under
`DT_INST_FOREACH_STATUS_OKAY` (uart_pl011.c:789-793; the e1000 equivalent
ETH_NET_DEVICE_DT_INST_DEFINE + foreach, eth_e1000.c:337-346). Everything from
`enable_device` through `map_region` and `on_init` becomes the body of the
instance **init function** (`pl011_init`, uart_pl011.c:570-580; `e1000_probe`,
eth_e1000.c:208-262), which the kernel calls at its init level during boot.
The order *within the init fn* preserves `body_emit.rs:5244-5285`'s canonical
order, with two Zephyr-specific deltas specified below (§B.3.2 claim elision,
§B.3.5 PCI probe/enable ordering — flagged open).

### B.2 `register_driver` — the bus-path selector

Device-lifecycle owns the registration marker; BRM supplies only the selector
token (`bus_selector_token`, body_emit.rs:5050-5061; emission :5140-5154,
:5203-5205). On Zephyr the selector picks the **instantiation macro family +
the per-instance context fields**, not a runtime call:

| `BusType` (crates/regseq-model/src/device_lifecycle.rs:79-94) | Zephyr realization | citation |
|---|---|---|
| `Platform` | `#define DT_DRV_COMPAT <compat>` + `DEVICE_DT_INST_DEFINE(n, <init>, pm, &data_##n, &cfg_##n, level, prio, api)` + `DT_INST_FOREACH_STATUS_OKAY(<INIT>)`; the region context is `DEVICE_MMIO_ROM` in the config struct, initialized `DEVICE_MMIO_ROM_INIT(DT_DRV_INST(n))` | uart_pl011.c:9, :43, :759, :789-793 |
| `Amba` | **folds into the Platform path.** Zephyr has no AMBA/periphid bus framework — the PL011, an ARM PrimeCell, is handled as a plain DT-compatible device (`DT_DRV_COMPAT arm_pl011`, uart_pl011.c:9; binding sidecar reference-drivers/pl011/zephyr/arm_pl011.yaml). Unlike Linux (`amba_driver` + periphid, linux-lowering-per-construct.md:284, GRILL D07), the `amba`/`platform` distinction is **erased at the Zephyr framework layer**; `MatchEntry::Amba{id,mask}` (device_lifecycle.rs:121) has no Zephyr match target — matching is by DT `compatible` only (DL's concern; recorded here because the selector consequence is BRM's) | uart_pl011.c:9; arm_pl011.yaml |
| `Pci` | the **same DT instantiation** plus a PCIe context: `DEVICE_PCIE_INST_DECLARE(inst)` + `DEVICE_PCIE_INST_INIT(inst, pcie)` materialize a `struct pcie_dev *pcie` member ("BDF & DID/VID", eth_e1000_priv.h:92-93) in the instance data, populated from the DT pcie node; the define macro is the class-appropriate wrapper (e1000 uses `ETH_NET_DEVICE_DT_INST_DEFINE`; the BRM-neutral base is `DEVICE_DT_INST_DEFINE` since `ClassTag` is recognized_not_lowered) | eth_e1000.c:7, :313-318, :337-346 |
| `I2c`/`Spi`/`Usb` | **deferred lowering** (matches the model annotation) | device_lifecycle.rs:86-91 |
| `Other(sid)` | rejected upstream at surface validation (unknown-bus) — never reaches the backend | bus-resource-management-surface-convergence.md:207 |

The generic_c `binding=bound|none` argument (body_emit.rs:5145-5153) is a
**host-trace artifact, not-applicable on Zephyr**: instantiation is static and
regions resolve at init time (same disposition as Linux,
linux-lowering-per-construct.md:288-291). Device presence on PCI is instead
checked natively at init entry:
`if (dev->pcie->bdf == PCIE_BDF_NONE) return -ENODEV;` (eth_e1000.c:215-217).

### B.3 The four managed ops — gating preserved, two ops become structural

The gates carry over **unchanged**: the whole block requires ≥1 resolved
binding (body_emit.rs:5244-5250); `enable_device` requires `is_pci_like`
(:5256-5259; device_lifecycle.rs:101-103); `set_bus_master` requires
`is_pci && dma_present` (:5267-5270; dma_present is the DMA-construct
placeholder defaulting false, device_lifecycle.rs:62-70); `claim_regions`
fires per resolved binding on every bus (:5260-5266); `get_resource_handle` is
unconditional (:5271-5272, I06). The single-resolved-region-per-driver guard
(`max_resolved_bindings_per_driver`, body_emit.rs:5063-5086) also carries over
and **matches Zephyr's idiom exactly**: the `DEVICE_MMIO_*` API is
single-region per device (uart_pl011.c:43, :64); a multi-region device would
need `DEVICE_MMIO_NAMED_*`, deferred with the model guard.

**B.3.1 `enable_device` (PCI-gated).** Lowers to
`pcie_set_cmd(dev->pcie->bdf, PCIE_CONF_CMDSTAT_MEM, true)` — setting the
command-register memory-decode bit (plus `PCIE_CONF_CMDSTAT_IO` would be the
Io-carrier analogue, §B.5). The in-repo reference sets it via exactly this
call (fused with the master bit):
`pcie_set_cmd(dev->pcie->bdf, PCIE_CONF_CMDSTAT_MEM | PCIE_CONF_CMDSTAT_MASTER, true)`
(eth_e1000.c:220-221). The convergence doc left the "Zephyr
`enable_device`+`set_bus_master` fusion" backend-[Open]
(bus-resource-management-surface-convergence.md:135); **this spec resolves it
as: emit separable calls** (`…CMDSTAT_MEM…` for enable, `…CMDSTAT_MASTER…` for
master) so the gating matrix stays op-per-op — the command-register bits are
independent, and since `set_bus_master` is dormant today (no fixture sets
`dma_present`), the emitted code is a single
`pcie_set_cmd(…, PCIE_CONF_CMDSTAT_MEM, true)` anyway. Fusing into one call
when both gates fire is a permitted peephole equivalent to the e1000 idiom.
**Platform/Amba:** not emitted (default-on; gate body_emit.rs:5256-5259,
prelude.rs:196-197). `pcie_set_cmd` returns void — no error leg.

**B.3.2 `claim_regions` — the no-runtime realization.** The as-built contract
already names Zephyr's realization verbatim: "Zephyr has no runtime claim (the
region is resolved/sized, not reserved)"
(crates/regseq-backends/src/generic_c/prelude.rs:202-208;
bus-resource-management-surface-convergence.md:129, :133-135). The model fires
`claim_regions(bar)` per resolved binding on every bus
(body_emit.rs:5260-5266); on Zephyr that op **emits zero calls** on both paths:

- **Platform/DT/Amba:** the claim *is* the DT node's `reg` property — captured
  at C compile time by `DEVICE_MMIO_ROM_INIT(DT_DRV_INST(n))` into the config
  struct (uart_pl011.c:759, :43). No reservation registry exists to claim
  against.
- **PCI:** subsumed into the §B.3.4 BAR resolution — `pcie_get_mbar` in
  generated code (reference idiom: `pcie_probe_mbar`, eth_e1000.c:219) — which
  validates and sizes the BAR; there is no `pci_request_regions` analogue in
  the reference idiom (eth_e1000.c:219 is the only region-touching call
  before mapping).

The Zephyr backend must therefore **not assume `claim_regions` emits a call**
— the same caveat the Linux spec records for its platform-path fusion
(linux-lowering-per-construct.md:311-316), strengthened on Zephyr to *both*
paths.

**B.3.3 `set_bus_master` (PCI ∧ `dma_present` — dormant).** Target identified:
`pcie_set_cmd(dev->pcie->bdf, PCIE_CONF_CMDSTAT_MASTER, true)` — the exact bit
the e1000 reference sets (eth_e1000.c:220-221). Gate unchanged
(body_emit.rs:5267-5270); `dma_present` defaults false and no driver sets it
(device_lifecycle.rs:62-70, GRILL O2), so this is emitted by no fixture today.
Disposition: **native-materialization target identified, not-applicable
today** (parity with Linux, linux-lowering-per-construct.md:319-323).

**B.3.4 `get_resource_handle` — the seam anchor, split by path.** Unconditional
in the model (body_emit.rs:5271-5272, I06). The "bus-handle context" it
captures diverges:

- **PCI:** the handle context is `dev->pcie` (the `struct pcie_dev` carrying
  BDF + cached DID/VID, eth_e1000_priv.h:92-93). The runtime resolution call
  is `struct pcie_bar mbar; pcie_get_mbar(dev->pcie->bdf, effective_bar, &mbar);`
  yielding the real `(mbar.phys_addr, mbar.size)`.
  `effective_bar = L2.bar ?? L1.bar` (device_lifecycle.rs:221-223) is a **BAR
  number**, and `pcie_get_mbar` is the v3.7.0 accessor keyed by BAR number.
  It is deliberately NOT `pcie_probe_mbar` — the call the reference makes with
  literal `0` (eth_e1000.c:213, :219) — whose argument is a **memory-BAR
  ordinal**; the pinned header states explicitly that the BAR number is not
  that index
  (<https://github.com/zephyrproject-rtos/zephyr/blob/v3.7.0/include/zephyr/drivers/pcie/pcie.h>;
  fetch-unverified, open-questions C-1). The two coincide on the e1000 only
  because BAR0 is its first memory BAR; feeding `effective_bar` to
  `pcie_probe_mbar` would silently map the wrong BAR whenever
  `effective_bar != 0` (Codex review issue 1). Both return a validity bool —
  the §B.6 check applies.
- **Platform/Amba:** the handle is the init fn's own
  `const struct device *dev`; the `(base,size)` were already captured **at
  build time** by `DEVICE_MMIO_ROM_INIT(DT_DRV_INST(n))` (uart_pl011.c:759) —
  i.e. `get_resource_handle` is **structural** (no runtime call), satisfied by
  the config-struct scaffold. `effective_bar` is `None` ⇒ generic_c's `bar=-1`
  degrade (body_emit.rs:5264, convergence doc :169).

**B.3.5 Composition seam: hand-off to `map_region` (register-access, out of
scope to lower).** Fed the resolved extent from B.3.4 with no register-access
op change (I04, device_lifecycle.rs:204-208; convergence doc :144-150):
**PCI** → `device_map(&data-><mmio_slot>, mbar.phys_addr, mbar.size, K_MEM_CACHE_NONE)`
(eth_e1000.c:223-224); **Platform/Amba** →
`DEVICE_MMIO_MAP(dev, K_MEM_CACHE_NONE)` in the init fn (uart_pl011.c:577),
after which accessors go through `DEVICE_MMIO_GET(dev)`
(reference-drivers/pl011/zephyr/uart_pl011_registers.h:39-43, "Must be placed
after DEVICE_MMIO_MAP" uart_pl011.c:579-580). **Ordering flag (open decision
O2):** the model's canonical order puts `enable_device` before
`get_resource_handle` (body_emit.rs:5219-5243); the Zephyr PCI reference does
the inverse — `pcie_probe_mbar` (:219) *then* `pcie_set_cmd` (:220-221) *then*
`device_map` (:223-224) — i.e. BAR sizing with memory decode still off. This
spec recommends adopting the reference order on Zephyr-PCI
(probe → enable → map) and recording the permutation as a ratified per-OS
deviation from the canonical preamble, rather than silently emitting either;
see [`zephyr-open-questions.md`](./zephyr-open-questions.md).

### B.4 Config-space intrinsics — `read_config` / `write_config`

`call()` intercepts `IntrinsicKind::Config(op)`
(crates/regseq-model/src/domain_attr.rs:110-117, :175-176) and routes to
`emit_config` (body_emit.rs:2122-2125, :2364-2394); width = `size_of::<T>()` ∈
{1,2,4,8} (`config_access_width`, :2404-2414), offset = `args[0]`. On Zephyr
the primitive is the PCIe host-layer pair `pcie_conf_read(bdf, reg)` /
`pcie_conf_write(bdf, reg, val)` (zephyr v3.7.0
`include/zephyr/drivers/pcie/pcie.h` — the header the reference includes at
eth_e1000.c:18;
<https://github.com/zephyrproject-rtos/zephyr/blob/v3.7.0/include/zephyr/drivers/pcie/pcie.h>).
Three Zephyr-specific facts (contrast linux-lowering-per-construct.md:349-367):

1. **Word-granular, not width-family.** Zephyr exposes only a 32-bit accessor
   whose `reg` parameter is a **config-word index** (byte offset ≫ 2), not a
   byte offset (pinned header above; **flagged fetch-unverified this session**,
   open question O1). Marshalling therefore inverts Linux's: width 4 at
   aligned offset → direct `pcie_conf_read(bdf, off >> 2)`; widths 1/2 → read
   the containing word, shift/mask (read) or read-modify-write the word
   (write); width 8 → two word accesses or reject (same width-8 gap
   disposition as Linux: **needs-new-surface** decision before emitting RMW
   silently — a config-space RMW is observable on the bus).
2. **Return shape matches the model.** `pcie_conf_read` returns the value
   directly — the model's `read_config::<T>(off) -> T` shape (`dst = (cty)…`,
   body_emit.rs:2372-2374) maps **without** Linux's status-out-param inversion
   (linux-lowering-per-construct.md:358-362). No native status to discard.
3. **PCI-ID caching is structurally satisfied.** The host PCI-ID-caching rule
   (prelude.rs:260-288: vendor/device id at 0x00 read once, then served from
   cache) maps to Zephyr's framework cache: `struct pcie_dev` carries the
   DID/VID alongside the BDF (eth_e1000_priv.h:92-93, comment "BDF & DID/VID").
   An offset-0x00 `read_config` may be served from `dev->pcie->id` with zero
   bus traffic — whether the backend serves from the framework cache or always
   issues `pcie_conf_read` is open decision O4. The PCI_COMMAND witness maps
   to a real `pcie_conf_read(bdf, 0x04 >> 2)`.

**Non-PCI** config access is not-applicable (no config space) and is the
**BRM-001** rejection — which is **deferred**
(bus-resource-management-surface-convergence.md:209-211), so the Zephyr spec
inherits the same unbuilt-diagnostic dependency the Linux spec records
(linux-lowering-per-construct.md:369-372).

### B.5 Carrier model types

- **`MmioBinding`** (device_lifecycle.rs:209-239): role preserved, **values
  replaced by enumeration** — the I04 swap the model documents itself ("or —
  on a real backend — `pci_resource_start`/`DT_REG_ADDR`",
  device_lifecycle.rs:206-208, :224-226). On Zephyr-PCI the resolved
  `(base,size)` come from `pcie_get_mbar(bdf, effective_bar, &mbar)` at init
  (the BAR-number accessor, §B.3.4; the reference's ordinal idiom is
  `pcie_probe_mbar`, eth_e1000.c:219); on Zephyr-platform from the DT `reg` property via
  `DEVICE_MMIO_ROM_INIT` at build (uart_pl011.c:759). What the Zephyr backend
  does with the platform.toml `[mmio.<id>] base/size` literals (advisory
  cross-check vs. DT-overlay generation vs. ignored) is open decision O3 —
  note `is_resolved()` (device_lifecycle.rs:236-239) gates the whole preamble,
  so the answer determines whether a Zephyr TU needs a platform.toml at all.
- **`BusType`** (device_lifecycle.rs:79-103): drives §B.2 macro-family
  selection and the unchanged `is_pci_like` gate. The Zephyr-specific
  collapse: `Amba` ≡ `Platform` at the framework layer (§B.2). `I2c`/`Spi`/
  `Usb` deferred (model :86-91); `Other` rejected upstream.
- **`BarType::Mem`** (device_lifecycle.rs:244-247, the default carrier): the
  memory-decode bit (`PCIE_CONF_CMDSTAT_MEM`, eth_e1000.c:220) + the mem-BAR
  probe/map path of §B.3.4/B.3.5 — every lowered region today.
- **`BarType::Io`** (device_lifecycle.rs:248-249): stays
  **recognized_not_lowered**, matching generic_c (status `none` in the
  inventory). Zephyr does have a port-I/O surface on x86 (an io-BAR probe +
  `sys_in*/sys_out*`), but no in-repo reference exercises it and the
  model-side unification decision is unmade (the same blocking gap Linux
  records, linux-lowering-per-construct.md:385-390). Do not invent a lowering;
  escalate (open question O5).

### B.6 Error channel + lifetime — native vs authored, and the no-teardown reality

- **Native channel.** Every §B.3 op is compiler-generated; failures surface as
  the **init fn's negative-errno return**, which Zephyr's device model records
  (the device then fails `device_is_ready()`; it is not re-probed). In-repo:
  `-ENODEV` on `PCIE_BDF_NONE` (eth_e1000.c:215-217). `pcie_set_cmd` is void;
  `device_map`/`DEVICE_MMIO_MAP` return void in the reference idiom
  (eth_e1000.c:223-224; uart_pl011.c:577) — map failure is not surfaced at the
  call site. The BAR probe returns a validity bool that the reference
  **ignores** (`pcie_probe_mbar`, eth_e1000.c:219); the generated
  `pcie_get_mbar` call (§B.3.4) SHOULD be checked, returning
  `-EINVAL`/`-ENODEV` (a deliberate hardening over the reference, parallel to
  Linux's `platform_get_resource` NULL check,
  linux-lowering-per-construct.md:334-335). There is **no `-EPROBE_DEFER`
  analogue** — Zephyr orders init by level/priority statically.
- **Authored channel.** Unchanged composition: native bring-up failures are
  **never** wrapped in `RegseqError`; only the authored `on_init` Result
  crosses the RegseqError adapter (the init-fail scenario capture,
  body_emit.rs:5340-5354, :5409-5479). The init fn returning `on_init`'s
  mapped `-errno` vs. the adapter's exit-code form is the RegseqError
  construct's per-OS section (§E), not BRM's.
- **Lifetime.** Zephyr devices are **never unloaded** — there is no module
  remove, no unregistration, and the claim-less model (§B.3.2) means there is
  nothing to release: no `pci_release_regions`/`pci_iounmap` pairing exists or
  is needed (contrast linux-lowering-per-construct.md:203-207).
  `device_map`/`DEVICE_MMIO_MAP` mappings are permanent. BRM contributes
  **zero teardown ops** to DL's postamble dispatch (body_emit.rs:5391-5406) on
  Zephyr; the reverse-order resource postambles remain DL trace semantics with
  no BRM body.

### B.7 Version pinning

Pinned to Zephyr **3.7 LTS** (citations to tag v3.7.0), forward-compatible
with 4.x for every API used here: `DEVICE_DT_INST_DEFINE`/
`DT_INST_FOREACH_STATUS_OKAY`, `DEVICE_MMIO_{ROM,RAM,ROM_INIT,MAP,GET}`,
`DEVICE_PCIE_INST_{DECLARE,INIT}`, `pcie_probe_mbar`, `pcie_get_mbar`,
`pcie_set_cmd`, `pcie_conf_{read,write}`, `device_map` — all are exercised by
the vendored reference drivers (which build against a recent Zephyr) except
`pcie_conf_{read,write}` and `pcie_get_mbar` (header-level claims, open
question O1 / C-1 — the get-vs-probe BAR-number/ordinal distinction is itself
a pinned-header claim). Scaffold
sidecars (DT binding yaml + Kconfig) are evidenced in-repo:
reference-drivers/e1000/zephyr/{intel,e1000.yaml,Kconfig.e1000},
reference-drivers/pl011/zephyr/arm_pl011.yaml.

---

## C. Device-lifecycle

> Citation note: every `file:line` below is an in-repo path verified during
> authoring (vendored Zephyr reference drivers under
> `reference-drivers/{pl011,e1000}/zephyr/`, model under
> `crates/regseq-model/src/`, generic_c under
> `crates/regseq-backends/src/generic_c/`). The two pinned Zephyr-doc URLs
> cited are stable doc roots but were **not live-verified** (WebFetch
> unavailable); claims that depend on them beyond what the vendored drivers
> corroborate are listed as open questions, not asserted.

### C.0 The structural shift: generic_c synthesizes the OS; Zephyr hoists half the lifecycle to *build time*

`generic_c` realizes the whole construct by synthesizing a driving `main`: one
`static void __rseq_lc_<idx>_<label>_<scenario>(void)` per driver per selected
scenario plus a runner `__rseq_lc_run()` that an `EntryShim` `main` calls
(`body_emit.rs:5088-5096`, `:5191-5193`, `:4975-4979`, `:5376-5380`; shim
composition `mod.rs:436-460`). Each managed step prints an `LC <event>` line —
that trace is the behavioral oracle
(`device-lifecycle-surface-convergence.md:25-33`; hooks `prelude.rs:115-148`).

On Linux the same canonical sequence redistributes across
`module_init`/`.probe`/`.remove`/`.shutdown`
(`linux-lowering-per-construct.md:405-441`). Zephyr shifts it **twice more**:

1. **Registration, matching, creation, and association move to BUILD time.** A
   Zephyr driver never registers at runtime. The driver file pins one
   compatible (`#define DT_DRV_COMPAT arm_pl011`, uart_pl011.c:9;
   `intel_e1000`, eth_e1000.c:7) and stamps one *statically allocated* device
   instance per status-okay devicetree node:
   `DT_INST_FOREACH_STATUS_OKAY(PL011_INIT)` (uart_pl011.c:793) /
   `DT_INST_FOREACH_STATUS_OKAY(E1000_PCI_INIT)` (eth_e1000.c:346), each
   expansion ending in `DEVICE_DT_INST_DEFINE(...)` (uart_pl011.c:789-791) or
   its class wrapper `ETH_NET_DEVICE_DT_INST_DEFINE(...)` (eth_e1000.c:337-344).
   Matching is the devicetree build, not a runtime table walk.
2. **The kernel's init-level walk at boot IS the runner.** The only *runtime*
   lifecycle artifact is the per-instance init function (`pl011_init`,
   uart_pl011.c:570-662; `e1000_probe`, eth_e1000.c:208-262), invoked once per
   instance by the kernel at the level/priority given in the define macro
   (`PRE_KERNEL_1, CONFIG_SERIAL_INIT_PRIORITY`, uart_pl011.c:790;
   `CONFIG_ETH_INIT_PRIORITY`, eth_e1000.c:342). No synthesized `main`, no
   `__rseq_lc_run` analog (Zephyr device docs, pinned:
   <https://docs.zephyrproject.org/3.7.0/kernel/drivers/index.html>).
3. **The teardown half of the construct largely has no carrier.** Classic
   Zephyr device objects are permanent: no `.remove`, no unbind, no hot-unplug
   path exists in either vendored driver. The orderly/surprise teardown
   scenarios therefore do not lower on the 3.7-LTS baseline (§C.6); only
   `on_system_shutdown` finds a native optional slot (the PM-device hook,
   §C.7).

| canonical step (convergence §5) | generic_c | Zephyr scaffold |
|---|---|---|
| `register` + match | marker re-run per scenario fn (`body_emit.rs:5196-5199`) | build time: `DT_DRV_COMPAT` + binding yaml + `DT_INST_FOREACH_STATUS_OKAY` → `DEVICE_DT_INST_DEFINE` |
| `create`, `associate` | per-scenario marker + gated `inst = {0}` (`:5206-5207`, `:5301-5306`) | build time: static per-instance `data`/`config` objects, wired into `struct device` by the define macro |
| `preamble`, BRM ops, `map_region`, `on_init` | top of each scenario fn (`:5209-5285`, `:5311`) | the body of the init function, run at boot |
| `teardown present=1\|0`, `on_deinit`/`on_surprise_remove`, `postamble` | orderly/surprise fns (`:5391-5406`) | **no classic carrier** (§C.6; 4.x `device_deinit()` fork = open question) |
| init-fail unwind | init-fail fn (`:5409-5483`) | negative errno return from the init fn; device never becomes ready |
| `on_system_shutdown` | shutdown fn, omit-when-absent (`:5355-5366`) | the optional PM-device slot of `DEVICE_DT_INST_DEFINE`; `NULL` when absent |

The four `LifecycleScenario`s (`backend_options.rs:24-42`) are a
*harness-side* concept (convergence: RD-5; inventory: "no model src"). On
Zephyr they are **not an emission input**: the emitted scaffold is
scenario-independent (all authored slots emitted once), and the scenarios
collapse to the kernel's natural paths — boot-init success (orderly bring-up),
boot-init failure (init-fail), power-off (shutdown), and *nothing* (surprise).

### C.1 `register` → `DT_DRV_COMPAT` + the instance-define macro (build-time, per DT node)

The `regseq_lc_register(name, match)` marker (`body_emit.rs:5197-5199`; hook
`prelude.rs:120-122`) packs two facts that split three ways on Zephyr:

- **The match set** → the driver file's `DT_DRV_COMPAT` token plus the binding
  yaml's `compatible:` key (uart_pl011.c:9 + arm_pl011.yaml:3; eth_e1000.c:7 +
  intel,e1000.yaml:6). See §C.2.
- **The registration act** → `DT_INST_FOREACH_STATUS_OKAY(<INIT macro>)`
  (uart_pl011.c:793; eth_e1000.c:346), whose per-instance expansion ends in
  `DEVICE_DT_INST_DEFINE(n, <init_fn>, <pm>, <data>, <config>, <level>, <priority>, <api>)`
  (uart_pl011.c:789-791) — or a class wrapper (eth_e1000.c:337-344). This is
  the Zephyr value of BRM's bus-path selector (`bus_selector_token`,
  `body_emit.rs:5050-5061`, whose own doc comment names `DEVICE_DT_DEFINE` as
  the Zephyr leg): `Platform` and (Zephyr-)`Pci` both go through the
  DT-instance define; there is no per-bus registration verb.
- **`descriptor.name`** (`device_lifecycle.rs:48-49`) has **no single Zephyr
  carrier**: the runtime device name comes from the DT node, not the driver.
  The vendored idiom spends the logical name on the log module
  (`LOG_MODULE_NAME eth_e1000`, eth_e1000.c:9-12 — the diagnostics-logging
  seam) and on symbol prefixes. Open decision Q6.

Time-split vs generic_c: registration runs **once, at build time, per DT
instance** — not once per scenario function, and not at module load as on
Linux (`linux-lowering-per-construct.md:459-462`).

Init level/priority is a Zephyr-only degree of freedom with no model surface
(uart_pl011.c:790 vs eth_e1000.c:342 differ, and the level is class-coupled) —
open decision Q7.

### C.2 `MatchEntry` → devicetree compatibles (full set, one block per entry); the AMBA gap

`generic_c` renders only the first entry into a trace string
(`render_match_trace`, `body_emit.rs:5499-5509`; SCOPE-1). Zephyr can consume
the **full** `match_set: Vec<MatchEntry>` (`device_lifecycle.rs:51`, variants
`:110-130`):

- **`Devicetree { compatible }`** → directly the binding `compatible:` +
  `DT_DRV_COMPAT` (arm_pl011.yaml:3; uart_pl011.c:9). The canonical Zephyr
  match.
- **Multi-entry match sets** → one instantiation block per compatible, exactly
  the vendored pl011 idiom: `#undef DT_DRV_COMPAT` /
  `#define DT_DRV_COMPAT SBSA_COMPAT` + a second
  `DT_INST_FOREACH_STATUS_OKAY(PL011_SBSA_INIT)` (uart_pl011.c:9-10,
  :795-829). Zephyr is thus *stronger* than generic_c's first-entry rendering
  here.
- **`Pci { vendor, device, … }`** → DT-mediated PCIe: the binding includes
  `pcie-device.yaml` (intel,e1000.yaml:8) and the instance macro
  declares/initializes the PCIe context from DT instance properties —
  `DEVICE_PCIE_INST_DECLARE(inst)` + `DEVICE_PCIE_INST_INIT(inst, pcie)` into
  the `pcie` member (eth_e1000.c:313-318; `struct pcie_dev *pcie`,
  eth_e1000_priv.h:92-93). The vendor/device pair lives in the board
  devicetree, not a runtime ID table; runtime presence is checked in the init
  fn (`dev->pcie->bdf == PCIE_BDF_NONE → -ENODEV`, eth_e1000.c:215-217).
  Authoring the board DT node is outside the compiler's emission; residual =
  open decision Q5.
- **`Amba { id, mask }`** → **no Zephyr carrier.** Zephyr has no AMBA periphid
  bus; the AMBA PL011 part is matched purely by compatible string
  (arm_pl011.yaml:3; uart_pl011.c:9). A `MatchEntry::Amba`-only match set
  cannot lower — open decision Q4 (recognized-not-lowered; do not invent a
  periphid table).
- **`Acpi`/`Usb`/`I2c`/`Spi`** (`device_lifecycle.rs:122-129`) are
  bridge-rejected at parse (CR-5, convergence §1.1) and never reach a backend
  — deferred, same as Linux.

No `MODULE_DEVICE_TABLE` analog exists (no module autoload on Zephyr); the
modalias artifact is Linux-only.

### C.3 `create` + `associate` → static instance objects + build-time wiring into `struct device`

- **`create`** (`regseq_lc_create`, `body_emit.rs:5206`; hook
  `prelude.rs:123-125`; gated zero-init container `:5287-5306`) → a
  **statically allocated, build-time-initialized** per-instance object pair:
  mutable `static struct pl011_data pl011_data_port_##n = { … }`
  (uart_pl011.c:773-787) /
  `static struct e1000_dev dev_##inst = { DEVICE_PCIE_INST_INIT(inst, pcie), }`
  (eth_e1000.c:316-318), plus a `static const` config (uart_pl011.c:758-766;
  eth_e1000.c:333-335). Allocation moves from runtime (`{0}` local in
  generic_c, `devm_kzalloc` on Linux) to the C static-storage image: fields
  not named in the designated initializer are zero — the same all-zero image
  generic_c's `inst = {0}` produces. generic_c's `needs_inst` gate
  (`:5287-5300`, a `-Wunused` dodge) is moot: the macro always defines the
  instance.
- **`associate`** (`regseq_lc_associate`, `body_emit.rs:5207`; hook
  `prelude.rs:126-128`) → the `&data`/`&config` arguments of
  `DEVICE_DT_INST_DEFINE` (uart_pl011.c:789-791; eth_e1000.c:340-341), which
  wire both objects into the kernel-owned `struct device` **at build time**.
  The round-trip read-back that Linux does via `pci_get_drvdata` is
  `dev->data` / `dev->config` in every callback
  (`struct e1000_dev *dev = ddev->data`, eth_e1000.c:211;
  `const struct pl011_config *config = dev->config; struct pl011_data *data = dev->data;`,
  uart_pl011.c:572-573).

Ordering is preserved, merely hoisted: register → create → associate all
complete before boot; the runtime residue of the canonical sequence begins at
the preamble (§C.4).

### C.4 Resource preamble + `on_init` → the body of the per-instance init function

The kernel invokes the init fn once per instance at its init level
(uart_pl011.c:789-790; eth_e1000.c:337-342). The bring-up steps lower into it
in canonical order — and the **BRM-inside-DL composition is preserved**: the
four managed ops + `map_region` slot between associate and `on_init`, exactly
where generic_c synthesizes them (`body_emit.rs:5209-5217` preamble loop,
`:5244-5285` BRM block in the order enable → claim → master → get_handle →
map).

- **`preamble` per `ResourceField`** (`regseq_lc_preamble`, `:5214-5217`;
  forward declaration order, `device_lifecycle.rs:178-187`) → the
  resource-acquisition span at the top of the init fn. Per bus path, with the
  Zephyr calls being the BRM/register-access deliverable while
  device-lifecycle owns the slot and ordering:
  - **PCI(e)** (eth_e1000.c:208-224): presence check
    (`PCIE_BDF_NONE → -ENODEV`, :215-217) → the BAR probe
    (:219 — the BAR-keyed handle/claim analog; reference:
    `pcie_probe_mbar(bdf, 0, &mbar)`, an **ordinal**-keyed enumerator;
    generated code uses the BAR-number accessor
    `pcie_get_mbar(bdf, effective_bar, &mbar)`, §B.3.4, with `effective_bar`
    the key generic_c traces in `regseq_bus_claim_regions(bar)`,
    `body_emit.rs:5263-5265`) →
    `pcie_set_cmd(bdf, PCIE_CONF_CMDSTAT_MEM | PCIE_CONF_CMDSTAT_MASTER, true)`
    (:220-221). **Composition note:** Zephyr *fuses* `enable_device` and
    `set_bus_master` into one config-command write — the model's `dma_present`
    gate (`device_lifecycle.rs:62-70`; generic_c gate
    `body_emit.rs:5267-5270`) decides whether the `MASTER` bit is included, it
    does not select a separate call. (The BRM section §B.3.1 ratifies the
    emitted form as separable calls with fusion as a permitted peephole.)
  - **Platform/DT** (uart_pl011.c): no enable/claim/master (matching the
    gating matrix — `enable_device` is PCI-only,
    `bus-resource-management-surface-convergence.md:129`); the resolved
    `(base,size)` is captured at build time by
    `DEVICE_MMIO_ROM_INIT(DT_DRV_INST(n))` in the config (uart_pl011.c:759,
    storage `DEVICE_MMIO_ROM`/`DEVICE_MMIO_RAM` at :43/:64).
- **`map_region` (the keystone seam, I04)** →
  `DEVICE_MMIO_MAP(dev, K_MEM_CACHE_NONE)` in its canonical slot in the init
  fn — after the preambles, before any MMIO op (§A.2.2; the witnessed hard
  constraint is only map-before-regs-access, uart_pl011.c:577-580) — on
  platform/DT, and
  `device_map(&dev->address, mbar.phys_addr, mbar.size, K_MEM_CACHE_NONE)`
  (eth_e1000.c:223-224) on PCIe. In both, the BRM-resolved `(base,size)` (DT
  `reg` via `DEVICE_MMIO_ROM_INIT`, or `mbar.phys_addr/size` from the BAR
  probe, §B.3.4) replaces generic_c's host literal
  (`regseq_bus_map_region(base,size,bar)`, `body_emit.rs:5277-5284`) **with no
  register-access op change** — accessors then run off the mapped base
  (`sys_write32(_val, (_dev)->address + (_reg))`, eth_e1000_priv.h:118-128),
  exactly the `MmioBinding` contract (`device_lifecycle.rs:203-229`,
  `:236-239`).
- **`on_init`** (marker always, author body iff overridden — CR-3,
  `body_emit.rs:5311`, `:5318-5319`; `LifecycleOps.on_init`,
  `device_lifecycle.rs:161-163`) → the author bring-up body inlined after the
  mapping, i.e. the rest of the init fn (pl011: :582-661; e1000: :226-261).
  Its `Result<(), RegseqError>` maps onto the init fn's `int` return:
  `Ok(())` → `return 0` (uart_pl011.c:661; eth_e1000.c:261); `Err(e)` → the
  negative errno via the **shared RegseqError→errno table** (`errno.rs:35-48`),
  whose tokens are native Zephyr errnos already used by the vendored drivers
  (`-ENODEV` eth_e1000.c:216, `-EINVAL` uart_pl011.c:173, `-EIO`
  eth_e1000.c:123). Not overridden → only the trait-default no-op "ran";
  nothing is emitted beyond the scaffold.

**Error-channel composition (preserved):** native acquire failures inside the
init fn are *already* errnos (eth_e1000.c:215-217; uart_pl011.c:584-588) — a
different channel from authored `RegseqError`, which is mapped through the
errno table at the `on_init` boundary. Both merge into the single `int` return
of the init fn, the same single-channel merge as Linux `.probe()`
(`linux-lowering-per-construct.md:520-523`).

### C.5 Init-failure → negative init return; the device never becomes ready

generic_c captures `on_init`'s `Result`, reads the niche discriminant, and on
`Err` prints `LC on_init -> Err` + the always-on `on_init_failure` marker +
the author body iff overridden, then exits cleanly (`emit_init_fail_body`,
`body_emit.rs:5421-5483`; hook `prelude.rs:135-137`; GRILL D12). On Zephyr:

- **The `Err` transition** → the `if (ret) { …; return ret; }` test inside the
  init fn (uart_pl011.c:584-588, :605-609). No standalone artifact.
- **`on_init_failure`** (`device_lifecycle.rs:167-168`) → the author cleanup
  hook inlined on the error path **before returning the negative errno**. As
  on Linux (no `.remove()` after failed probe,
  `linux-lowering-per-construct.md:549-550`), no later teardown hook will run
  for a failed init, so this inline unwind is the only cleanup; `on_deinit`
  must not run.
- **Partial (HWM) reverse postamble** → with build-time static allocation and
  `DEVICE_MMIO` there is typically nothing dynamically held to release in the
  vendored drivers (both error-return without unwinding: eth_e1000.c:216;
  uart_pl011.c:587). generic_c also defers HWM rollback as-built
  (`device-lifecycle-surface-convergence.md:218-227`). Specified as: thread
  the acquired-so-far list into the error path *when a Zephyr resource
  actually needs release*; nothing to emit for the build-now resource kinds.
- **Oracle divergence to flag:** generic_c asserts via stdout + clean exit;
  Zephyr's observable is the **nonzero init return**, after which the kernel
  records the failure and `device_is_ready()` is false for that instance
  (pinned doc: <https://docs.zephyrproject.org/3.7.0/kernel/drivers/index.html>
  — not live-verified, Q9).

### C.6 Orderly/surprise teardown → no classic Zephyr carrier (the honest gap)

generic_c's teardown suffix — `regseq_lc_teardown(present)` dispatch,
always-on callback marker, author body iff overridden, reverse postambles with
`skipped(surprise)` (`emit_teardown_dispatch`, `body_emit.rs:5391-5406`; hooks
`prelude.rs:132-134`, `:141-147`) — has **no slot to lower into** on baseline
Zephyr:

- Both vendored drivers define **no teardown of any kind**: no remove/unbind
  callback exists in `DEVICE_DT_INST_DEFINE`'s argument list
  (uart_pl011.c:789-791; eth_e1000.c:337-344), and device instances are
  permanent statics. The `present=1|0` dispatch bit and the reverse-order
  postambles therefore have no runtime artifact.
- **`on_deinit`** (`device_lifecycle.rs:164-165`): *recognized-not-lowered* on
  the Zephyr 3.7-LTS baseline. The version fork — Zephyr 4.x added a
  `device_deinit()` API to the device model — is deliberately left as **open
  question Q1** (version-sensitive; not citable from the vendored 3.x-era
  drivers; do not invent the macro spelling).
- **`on_surprise_remove`** (`device_lifecycle.rs:166`): **no Zephyr analog at
  all.** A static DT device cannot vanish; neither vendored driver models
  hot-unplug, and Zephyr-over-PCI hotplug is out of the pinned scope.
  *Recognized-not-lowered*; whether authoring it under `TargetOs::Zephyr`
  should diagnose or silently omit is open question Q2.
- **`postamble`** (`device_lifecycle.rs:178-182`; reverse order
  `body_emit.rs:5404-5406`): lowers only into whatever teardown slot is chosen
  by Q1/Q3; on the baseline, nothing is emitted. (`DEVICE_MMIO` mappings in
  the vendored drivers are never unmapped.)

This mirrors the inventory's own honesty rule: the construct's *bring-up half*
lowers idiomatically; the *teardown half* is explicitly not lowered on this
target baseline rather than invented.

### C.7 `on_system_shutdown` → the optional PM-device slot (omit-when-absent maps structurally)

`on_system_shutdown` is the lone omit-when-absent callback (D10/I02):
generic_c emits **no marker and no slot** when `None`, and marker + author
call when `Some` (`body_emit.rs:5355-5366`; `device_lifecycle.rs:169-170`).
Zephyr has a native optional-slot idiom that realizes exactly this shape — the
**third argument of `DEVICE_DT_INST_DEFINE`**, the PM device:

- Overridden → define the PM action hook and pass it:
  `PM_DEVICE_DT_INST_DEFINE(n, <drv>_pm_action)` +
  `PM_DEVICE_DT_INST_GET(n)` as the define's PM argument — precisely the
  vendored conditional idiom (`PM_INST_DEFINE`/`PM_INST_GET`,
  uart_pl011.c:106-117, consumed at :771 and :789). The PM contract is an
  **action callback**, not a dedicated shutdown hook:
  `static int <drv>_pm_action(const struct device *dev, enum pm_device_action action)`
  dispatches on the action enum, and unsupported actions return `-ENOTSUP`
  (pinned: <https://docs.zephyrproject.org/3.7.0/services/pm/device.html>;
  not live-verified, Q3). The generated wrapper therefore runs the author
  body only on the Q3-chosen action(s) — its `Result` discarded (CR-4),
  `return 0;` — and returns `-ENOTSUP` for every other action.
- Not overridden → **pass `NULL`** (uart_pl011.c:115-116 defines
  `PM_INST_GET(n)` to `NULL` in the no-PM case; eth_e1000.c:339 passes literal
  `NULL`). The slot is absent from the emitted scaffold — the exact
  omit-when-absent contract.

What is **not** asserted: which PM action (`PM_DEVICE_ACTION_TURN_OFF` vs
`SUSPEND`) fires at system power-off, and under which `CONFIG_PM_DEVICE*`
options the poweroff path walks devices at all — that trigger semantics is
config/version-sensitive (pinned doc root:
<https://docs.zephyrproject.org/3.7.0/services/pm/device.html>; not
live-verified) and is **open question Q3**. The *slot mapping* stands on the
in-repo citations regardless. **Consequently this primitive is slot-mapped,
trigger-unresolved — it must not be claimed as a materialized lowering until
Q3/C-4 closes** (disposition downgraded post-review, Codex issue 4; the
interim emit-vs-diagnose policy is open-questions A-18).

### C.8 The Policy-P container morph → Zephyr's config/data split; the back-pointer inverts

The morph
(`struct __rseq_lc_<idx>_<label>_target { void *__rseq_dev; <driver> inner; uint32_t __rseq_state; }`,
`body_emit.rs:5174-5189`; convergence §7, CR-7) lands on Zephyr's standard
per-instance state split:

- **`inner` (the spec driver, embedded verbatim)** → the mutable per-instance
  **`data` struct** (`struct pl011_data`, uart_pl011.c:63-74;
  `struct e1000_dev`, eth_e1000_priv.h:84-109), statically allocated per DT
  node (§C.3). Callbacks stay **spec-view**: the author body receives the
  embedded spec driver, recovered from `dev->data` (eth_e1000.c:211;
  uart_pl011.c:573) — CR-7's "callbacks run spec-view over `&inst.inner`" with
  `dev->data` playing `&inst.inner`. Register-offset recovery and field
  projection are untouched, so register-access composes through the morph
  unchanged (OI-1).
- **`void *__rseq_dev` (prepended device back-pointer)** → **inverted on
  Zephyr.** generic_c/Linux put a device pointer *inside* the container;
  Zephyr instead hands every callback `const struct device *dev` and reaches
  the container *through* it (`dev->data`, `dev->config` — eth_e1000.c:128,
  :211; uart_pl011.c:572-573). The slot's information content
  (container ↔ kernel-device association) is carried by the build-time wiring
  of §C.3; no stored member is needed. Where an author field must hold OS
  state, the vendored pattern is an explicit data-struct member (e.g.
  `struct net_if *iface`, eth_e1000_priv.h:99).
- **`uint32_t __rseq_state` (appended)** → a plain retained data-struct member
  (driver-private flags), zero at boot like the rest of the static image. No
  kernel-visible artifact.
- **Zephyr-only refinement with no model surface:** the **const config (ROM)
  vs data (RAM) split** (uart_pl011.c:42-60 vs :63-74; eth_e1000_priv.h:111-113).
  The model's `DriverHandle` does not distinguish immutable-per-instance facts
  from mutable state; build-now maps the whole spec driver into `data` and
  lets `DEVICE_MMIO_ROM`/`DEVICE_MMIO_RAM` (uart_pl011.c:43, :64) carry the
  MMIO binding per Zephyr convention. Splitting author fields into config is a
  quality refinement — open question Q8b.
- **`ResourceField`/`ResourceKind::Mmio`** (`device_lifecycle.rs:186-201`,
  `:253-257`) → the `DEVICE_MMIO_RAM`/`mm_reg_t address` member acquired in
  the init fn (§C.4) — never released (§C.6).
- **`UserDataField`/`UserDataKind`** (`device_lifecycle.rs:262-279`): as on
  Linux, there is **no designed lowering** — generic_c satisfies
  Rule-1/Rule-2 incidentally via the blanket `{0}` (`body_emit.rs:5301-5306`).
  Zephyr inherits the same guarantee from C static-storage zero-initialization
  of the per-instance data object (partial designated initializers zero-fill
  the rest — the vendored instances initialize only one member,
  eth_e1000.c:316-318): `ZeroInit` directly; `OptionCopy`'s `None` rests on
  the same all-zero-image assumption generic_c already makes. Inherited, not
  invented.

### C.9 `device_class` / `ClassTag` — recognized, not lowered (but Zephyr makes the deferral visible)

`ClassTag` (`device_lifecycle.rs:53-54`, `:134-150`) is carried, unvalidated,
with no generic_c lowering (inventory: `recognized_not_lowered`). The Zephyr
backend likewise **does not lower it**. Worth recording: on Zephyr the class
is *load-bearing in the scaffold idiom* — it selects the define-macro family
and API vtable (`Serial` ↔ `DEVICE_API(uart, …)` + plain
`DEVICE_DT_INST_DEFINE`, uart_pl011.c:534-542, :789-791; `Network` ↔
`ETH_NET_DEVICE_DT_INST_DEFINE` + `struct ethernet_api`, eth_e1000.c:296-306,
:337-344). That selection is exactly the Device Class construct's deferred
territory (D06/D09); build-now the Zephyr scaffold uses the class-neutral
`DEVICE_DT_INST_DEFINE` with a NULL/empty API **for `ClassTag::Generic`
only**, and a **non-Generic tag is an emit-time diagnostic** (post-review,
Codex issue 6): silently passing `NULL` api for a class the author declared
would produce a device invisible to its class-API consumers — exactly the
silent truncation the discipline forbids. The class-coupled wrappers stay
with the future Device Class construct — open question Q8a.

### C.10 Runner / entry shim — no Zephyr artifact; composition summary

`__rseq_lc_run` + the `EntryShim` `main` (`body_emit.rs:4975-4979`,
`:5376-5380`; `naming.rs:73-103`, B3 early-errno-return `:181-182`;
`mod.rs:436-460`) have **no Zephyr analog**: Zephyr owns `main`, and the
kernel's init-level walk performs the runner's job. Neither vendored driver
contains a `main` or any self-driving harness. A lifecycle TU emitted for
Zephyr contains *only* the scaffold (structs, init fn, optional PM hook,
define macros); RD-7's no-lifecycle byte-purity is trivially preserved (a
no-lifecycle TU emits no scaffold).

**Cross-construct composition, preserved end-to-end:**

1. **BRM preamble ⊂ DL preamble** — the managed ops nest inside the init fn
   between associate and `on_init`, in canonical order; on Zephyr-PCIe two of
   them *fuse* into one `pcie_set_cmd` (§C.4, eth_e1000.c:220-221) — the gate
   structure (`is_pci_like`, `dma_present`; `device_lifecycle.rs:96-104`,
   `:62-70`) survives as bit selection.
2. **`map_region` consumes the BRM-resolved `(base,size)`** —
   `DEVICE_MMIO_ROM_INIT`/`pcie_get_mbar` replace the host literal, no
   register-access op change (I04; §C.4).
3. **Authored `RegseqError` vs native errnos** — distinct channels merging
   into the init fn's single `int` return through the shared `errno.rs:35-48`
   table, whose tokens Zephyr natively uses (§C.4/C.5).
4. **DL register marker + BRM bus selector** jointly become the build-time
   choice of define macro + DT binding (§C.1/C.2).
5. **diagnostics-logging seam** — the scaffold's
   `LOG_MODULE_REGISTER(LOG_MODULE_NAME)` (eth_e1000.c:9-12) is where
   `descriptor.name` plausibly lands (Q6) and is the scaffold point the Zephyr
   diag-log lowering hangs off.

**Confidence statement:** the bring-up half
(register/match/create/associate/preamble/on_init/init-fail) and the shutdown
slot are specified confidently from the vendored drivers; the orderly/surprise
teardown half is *explicitly not lowered* on the pinned baseline (a property
of Zephyr's device model, stated with citations, not a specification gap),
with the 4.x `device_deinit()` fork and PM-trigger semantics carried as open
questions.

---

## D. Diagnostics-logging

### D.0 What the construct is in the IR

Diagnostics-logging materializes as exactly one intrinsic family:
`IntrinsicKind::Log(LogLevel)` at discriminant 0
(`crates/regseq-model/src/domain_attr.rs:88-90`), with the closed four-level
severity enum `Debug/Info/Warn/Error` (`domain_attr.rs:141-150`) recognized
from the dash-form names `log-debug`/`log-info`/`log-warn`/`log-error`
(`domain_attr.rs:161-164`). A log site reaches the backend as an ordinary
`Call` terminator whose `Direct` callee carries the intrinsic, with the
Pipeline-A flat operand slice `[fmt_literal, v1, ..., vN]` — the format
template literal followed by the value operands, never a reconstructed
`LogArg` (`crates/regseq-backends/src/generic_c/body_emit.rs:2712-2715, 2732-2739`).
The template rides as the `&str` const-aggregate carrier
`Aggregate([Reloc -> CMS, Uint(len)])` (`body_emit.rs:2873-2918`).

The construct has no internal gating and no failure channel: a `Log` callee
returns unit and never diverges, so the call always has a `Some` successor
(`body_emit.rs:2728-2730`); its only composition with the other constructs is
positional (it sits wherever the authored CFG put it, between
MMIO/bus/lifecycle operations).

### D.1 Agnostic core retained unchanged

The Zephyr backend reuses every stage of the generic_c diag-log pipeline
**except the final emission**:

- **Interception**: a `CallCallee::Direct` whose instance carries
  `IntrinsicKind::Log(level)` is intercepted before ordinary call lowering;
  recognition is keyed by instance identity, so an `Indirect` callee can never
  satisfy it (`body_emit.rs:2095-2104`).
- **Template read**: `read_template_bytes` walks `model.consts` +
  `resolve_reloc` + the CMS `Static { init }` to the literal bytes; D6
  guarantees a literal, anything else fails loud (`body_emit.rs:2851-2862,
  2876-2918`).
- **Template parse**: `parse_template` produces the ordered
  `Literal`/`Placeholder` segment sequence, accepting `{{`/`}}` escapes,
  embedded newlines, and the empty template; malformed braces fail loud
  (`body_emit.rs:5896-5950`).
- **Modifier set**: the default `{}` plus exactly the eight hex forms `{x,X}` ×
  `{plain, #, 08, #010}`, matched by exact byte (`body_emit.rs:5956-6014`);
  anything else (positional, named, `:?`, `:b`, `:o`, `:p`, precision, other
  widths) is rejected (`:6008-6012`).
- **Arity**: placeholder count must equal `args.len() - 1`
  (I-DiagLog-CallArity, `body_emit.rs:2741-2750`).
- **Specifier/cast table**: `diag_specifier_and_cast_for(modifier, ty)` is
  reused **whole** (`body_emit.rs:5798-5850`). It gates on the admissible
  numeric set (`bool/i8/i16/i32 → %d/(int)`, `u8/u16/u32 → %u/(unsigned)`,
  `i64 → %lld`, `u64 → %llu`; hex → `%[#][0w][ll]{x|X}` with the two-step
  source-width-unsigned reinterpret cast) and rejects `char`/`isize`/`usize`
  and any non-integer (`:5790-5793, 5809-5813, 5831-5835`). Its output is a
  printf-family conversion + varargs-promotion cast, and Zephyr's logging
  formats via cbprintf, a C99-printf-compatible engine
  (<https://docs.zephyrproject.org/3.7.0/services/formatted_output/index.html>),
  so the pair carries over unchanged.
- **`&str` modifier guard**: a hex modifier on a `&str` argument fails loud
  (`require_default_modifier`, `body_emit.rs:5852-5864`).
- **Successor contract**: the lowering replaces the call site one-for-one and
  closes with `goto bb_<target>` (`body_emit.rs:2845-2848`), so the
  construct's ordering relative to register-access / bus / lifecycle
  operations in the CFG is preserved exactly.

What changes is only the emission stage: generic_c decomposes the template
into per-segment byte-faithful `fwrite`s because hosted stdio has no
structured log sink (`body_emit.rs:2710-2721, 2752-2843`); Zephyr **has** a
native structured logging engine, so the idiomatic lowering **reassembles**
the segments into a single cbprintf format string plus an argument list — the
inverse of the fwrite decomposition, and the same move the Linux lowering
makes for `printk` (`docs/discussions/linux-backend/linux-lowering-per-construct.md:695-700`).

### D.2 Level → macro (the text tag becomes structural)

| `LogLevel` (`domain_attr.rs:141-150`) | Zephyr macro | generic_c tag (`level_tag`, `body_emit.rs:6058-6065`) |
|---|---|---|
| `Debug` | `LOG_DBG(...)` | `[DEBUG] ` |
| `Info`  | `LOG_INF(...)` | `[INFO] ` |
| `Warn`  | `LOG_WRN(...)` | `[WARN] ` |
| `Error` | `LOG_ERR(...)` | `[ERROR] ` |

In-repo evidence: `LOG_ERR("Unsupported register: 0x%x", r)`
(`reference-drivers/e1000/zephyr/eth_e1000.c:67`),
`LOG_DBG("tx.sta: 0x%02hx", ...)` (`:121`),
`LOG_ERR("Invalid RX descriptor length: %hu", ...)` (`:154`),
`LOG_ERR("Unhandled interrupt, ICR: 0x%x", icr)` (`:203`), zero-argument
`LOG_DBG("done")` (`:284`), and `LOG_DBG("iow32 %s 0x%08x", ...)` /
`LOG_DBG("ior32 %s 0x%08x", ...)`
(`reference-drivers/e1000/zephyr/eth_e1000_priv.h:119, 126`). Macro reference:
<https://docs.zephyrproject.org/3.7.0/services/logging/index.html>.

generic_c's inline `[LEVEL] ` text prefix (assembled into the first literal
run at `body_emit.rs:2767-2770`) is **dropped**: on Zephyr the severity and
the module identity are carried structurally by the log core, which renders
them in the output prefix (e.g. `<dbg> eth_e1000: ...`) and uses the level for
compile-time and runtime filtering
(<https://docs.zephyrproject.org/3.7.0/services/logging/index.html>).

### D.3 Format-string reassembly (inverse of the fwrite decomposition)

Walk `parse_template`'s segments in order, building one format literal `F` and
one argument list `A`:

- **`Segment::Literal(bytes)`**: append `c_escape(bytes)` to `F`, **and double
  every literal `%` to `%%`**. generic_c explicitly never doubles `%` because
  literal runs are `fwrite`n raw, never used as a format
  (`body_emit.rs:6017-6022`); on Zephyr the reassembled literal IS the
  cbprintf format, so the doubling is mandatory (same rule as the Linux
  lowering, `linux-lowering-per-construct.md:732-734`).
- **Numeric `Segment::Placeholder(modifier)`**: reuse
  `diag_specifier_and_cast_for(modifier, ty)` whole
  (`body_emit.rs:5798-5850`); append its `spec` to `F` and push `cast(expr)`
  onto `A`. All nine resulting conversions (`%d %u %lld %llu` and the hex
  family `%[#][0w][ll]{x|X}`) are supported by Zephyr's default cbprintf
  implementation (`CONFIG_CBPRINTF_COMPLETE`, C99-compatible:
  <https://docs.zephyrproject.org/3.7.0/services/formatted_output/index.html>);
  the 64-bit conversions additionally require `CONFIG_CBPRINTF_FULL_INTEGRAL`
  (same page). The zero-padded-hex family is idiomatically attested in-repo:
  `0x%08x` (`eth_e1000_priv.h:119, 126`), `0x%x` (`eth_e1000.c:67, 203`).
- **Static `&str` `Segment::Placeholder`** (default modifier only,
  `body_emit.rs:5852-5864`): the decoded literal bytes are known at emission
  time (generic_c reads them at `body_emit.rs:2787-2802`). If the bytes
  contain **no interior NUL**, pass them as a NUL-terminated C string-literal
  argument via `%s` appended to `F`. A string literal lands in rodata, which
  Zephyr's deferred-mode packager classifies as a read-only string and keeps
  as a pointer (valid forever) — safe
  (<https://docs.zephyrproject.org/3.7.0/services/logging/index.html>,
  "Logging strings"). If the bytes **do** contain an interior NUL, `%s`
  truncates and faithfulness is lost — fail loud or fall back to
  `LOG_HEXDUMP_DBG` (open decision O2; the byte-faithful hexdump fallback is
  attested in-repo at `eth_e1000.c:29-37`). The bytes are never folded into
  `F` itself, preserving generic_c's "value bytes are never a format"
  discipline (`body_emit.rs:6020-6022`).
- **Non-static `&str` `Segment::Placeholder`** (a runtime fat pointer
  `struct rust_str { const uint8_t *data; size_t len; }`,
  `crates/regseq-backends/src/generic_c/prelude.rs:39-44`; generic_c `fwrite`s
  `(op).data/(op).len` at `body_emit.rs:2804-2817`): this is **the gap**. The
  printf-natural `%.*s` with `(int)(expr).len, (const char *)(expr).data` is
  format-correct under `CONFIG_CBPRINTF_COMPLETE`, but Zephyr's default
  logging mode is deferred (`CONFIG_LOG_MODE_DEFERRED`,
  <https://docs.zephyrproject.org/3.7.0/services/logging/index.html>), where
  the call site packages the message and the packager copies transient
  (non-rodata) strings by NUL-terminated length — a Rust fat pointer is not
  NUL-terminated, so packaging can over-read past `len`. Recommended lowering
  (O1): a call-site bounce — copy `min(len, CAP-1)` bytes into a fixed stack
  buffer, NUL-terminate, pass the buffer via `%s` (the packager then copies it
  safely as a transient string). This is exactly the in-repo
  `snprintk`-into-stack-buffer idiom (`eth_e1000.c:29-37`). Divergences vs
  generic_c, both documented: truncation at the buffer cap, and truncation at
  any interior NUL. Alternatives: require `CONFIG_LOG_MODE_IMMEDIATE` (removes
  the packaging copy but keeps the interior-NUL gap), or `LOG_HEXDUMP_*`
  (byte-faithful but not a formatted line).
- **Trailing newline**: do **not** append one. generic_c appends a trailing
  `\n` byte (`body_emit.rs:2840-2843`) and the Linux lowering appends `\n` to
  the printk format; Zephyr's log output stage appends the line ending itself,
  and the idiom is newline-free messages — every `LOG_*` call in the vendored
  driver omits `\n` (`eth_e1000.c:67, 121, 154, 203, 284`;
  `eth_e1000_priv.h:119, 126`).
- **Zero-placeholder template**: lowers to the no-argument macro form, in-repo
  `LOG_DBG("done")` (`eth_e1000.c:284`).

### D.4 The emitted statement

One macro invocation per log site, followed by the successor `goto` (the
contract from `body_emit.rs:2845-2848`). Worked example —
`info!("DMA channel {} acquisition failed", channel)` with `channel: u32` (the
discussion doc's canonical example,
`docs/discussions/logging-and-printing/logging-and-printing-discussion.md:107-109`):

```c
LOG_INF("DMA channel %u acquisition failed", (unsigned)channel);
goto bb_<target>;
```

structurally the in-repo line `LOG_ERR("Unhandled interrupt, ICR: 0x%x", icr)`
(`eth_e1000.c:203`).

### D.5 Scaffold — module registration (the composition seam)

Any TU that uses `LOG_*` must register a log module once at file scope, before
any use. The exact in-repo pattern (`eth_e1000.c:9-12`):

```c
#define LOG_MODULE_NAME <module>
#define LOG_LEVEL <level-symbol>          /* optional compile-time gate */
#include <zephyr/logging/log.h>
LOG_MODULE_REGISTER(LOG_MODULE_NAME);
```

- **Module name = the device-lifecycle seam.** The module name comes from the
  sanitized `DriverDescriptor.name`
  (`crates/regseq-model/src/device_lifecycle.rs:47-52`). This is the Zephyr
  analogue of Linux's per-call `dev` pointer seam
  (`linux-lowering-per-construct.md:720-727`) — but inverted in granularity:
  Zephyr carries device/driver identity **per-TU** via the registered module,
  not per-call, so individual `LOG_*` calls take no device argument. A TU with
  log sites but no `DriverDescriptor` needs a fallback module name (open
  decision O4; recommend the TU's emission name).
- **Demand-driven.** Emit the scaffold iff the TU contains at least one
  intercepted `Log` call — the same demand-driven-include discipline generic_c
  applies to `<errno.h>` (`prelude.rs:345-350`).
- **Compile-time level.** The vendored driver gates on a subsystem Kconfig
  symbol via `#define LOG_LEVEL CONFIG_ETHERNET_LOG_LEVEL` (`eth_e1000.c:10`)
  rather than a driver-local Kconfig template (`Kconfig.e1000` carries none).
  For generated drivers the choice between an idiomatic per-driver Kconfig
  level and a forced `LOG_LEVEL_DBG` (so all four authored levels stay live,
  matching generic_c's always-on emission) is open decision O3.

### D.6 Ordering / gating / composition preserved

A log call is intercepted **in place**: the emission replaces exactly one
`Call` terminator and keeps its successor edge
(`body_emit.rs:2095-2104, 2845-2848`), so every authored ordering relative to
`mmio_*`, config-space, and lifecycle operations is preserved verbatim. Log
sites inside lifecycle callbacks (`on_init` etc.,
`device_lifecycle.rs:162-170`) land inside whatever Zephyr init/probe function
the device-lifecycle lowering synthesizes around those callbacks; the diag-log
construct contributes only the file-scope registration scaffold (§D.5) plus
the per-site statement — it never reorders or gates anything else.

One semantic divergence to flag: Zephyr's compile-time level filtering means a
`LOG_DBG` can compile out entirely when the module level excludes debug
(<https://docs.zephyrproject.org/3.7.0/services/logging/index.html>), whereas
generic_c emits every level unconditionally. This mirrors the Linux `dev_dbg`
dynamic-debug decision (`linux-lowering-per-construct.md:768-772`) and is
folded into open decision O3.

### D.7 Error / lifetime

- **No failure channel.** `Log` returns unit and never diverges
  (`body_emit.rs:2728-2730`); the `LOG_*` statement macros return no value.
  This channel is entirely separate from authored `RegseqError` and from
  native Zephyr init/resource error returns — the three-channel rule the
  cross-construct contract requires.
- **Lifetimes.** The format literal is static (rodata) and is kept by
  reference in deferred mode; numeric arguments are copied by value into the
  package at the call site; `&str` arguments per §D.3 (read-only kept by
  pointer, transient copied — the bounce buffer makes the copy source
  NUL-terminated and call-site-scoped, which is sufficient because packaging
  happens inside the macro invocation;
  <https://docs.zephyrproject.org/3.7.0/services/logging/index.html>). No heap
  allocation; deferred mode renders in the log-processing context.

### D.8 What is NOT lowered (explicit)

- **Pointers / `{:p}`**: not in the accepted modifier set
  (`body_emit.rs:5952-5955, 6008-6012`); the discussion doc floats `{:p}`
  (`logging-and-printing-discussion.md:217`) but it is absent from the
  as-built — do not invent (Zephyr would be `%p`, a new surface).
- **`isize`/`usize`/`char`**: explicitly rejected by the diag whitelist
  (`body_emit.rs:5790-5793, 5809-5813`).
- **Source context (file:line)**: carried by neither the intrinsic nor
  generic_c; not lowered.
- **Indirect (fn-pointer) log calls**: never recognized — Direct-only
  interception (`body_emit.rs:2098-2100`).
- **Hex modifier on `&str`**: rejected (`body_emit.rs:5852-5864`).

### D.9 Version pin

Pinned to Zephyr **3.7 LTS / 4.x** (all doc URLs pinned to 3.7.0).
`LOG_MODULE_REGISTER` + `LOG_DBG/INF/WRN/ERR` and the v2 (message-packaging)
log core are stable across this range — log v1 was removed before 3.7, so v2
deferred-mode semantics may be assumed. Required config baseline:
`CONFIG_LOG=y`; default `CONFIG_LOG_MODE_DEFERRED`; default
`CONFIG_CBPRINTF_COMPLETE`; `CONFIG_CBPRINTF_FULL_INTEGRAL` for the
`%lld/%llu` conversions
(<https://docs.zephyrproject.org/3.7.0/services/logging/index.html>,
<https://docs.zephyrproject.org/3.7.0/services/formatted_output/index.html>).
Whether the backend should *assert* these Kconfig pins in generated collateral
or merely document them is open decision O5.

---

## E. RegseqError

### E.0 What `regseq-error` is in the IR

`regseq-error` is the **authored semantic error channel**
(`docs/discussions/regseq-error/regseq-error-spec.md:30-44`), deliberately
**not** a wrapper over OS-native error families (`regseq-error-spec.md:39-44`).
It materializes as three things, lowered together and in order:

1. a **recognition marker** —
   `AttrData::FrameworkType(FrameworkType::RegseqError)` on the error enum's
   item (`crates/regseq-model/src/domain_attr.rs:62-68`, `:75-80`);
2. a **carrier** — `core::result::Result<(), RegseqError>` as the authored
   fallible output (`regseq-error-spec.md:178-196`);
3. a **boundary** — the outermost authored fallible callback exit, the *only*
   place semantic `RegseqError` becomes target-specific lowering input
   (`regseq-error-spec.md:69-94`).

The reference backend (generic_c) lowers this as faithful body emission of the
carrier plus a **single hand-emitted boundary adapter** (`emit_exit_adapter`,
`crates/regseq-backends/src/generic_c/body_emit.rs:4709-4828`) that reads the
`Result` niche discriminant and converts the inner `RegseqError` tag to a libc
`<errno.h>` symbol via a per-target, name-keyed table
(`crates/regseq-backends/src/generic_c/errno.rs:35-48`). The table lives in
the backend precisely because host-error mapping is a per-target concern
(`errno.rs:5-9`). The Zephyr backend inherits the **entire** recognition +
carrier + discriminant-read machinery unchanged and substitutes exactly two
things: **errno polarity** and **adapter placement**. Unlike Linux, the
**include name does not change** (§E.3).

### E.1 Recognition marker — `FrameworkType::RegseqError`

Read identically on every target: `is_framework_error_enum` walks
`model.domain_attrs.item_attrs`
(`crates/regseq-backends/src/generic_c/mod.rs:670-679`). It is the
certification gate in `classify_entry_return`: a `Result<(), E>`-shaped return
is eligible for errno lowering only when `E`'s item carries the marker; an
unmarked `Err` enum is a fail-loud rejection (`mod.rs:647-658`). **Zephyr:
reuse verbatim.** Its Zephyr role is the compile-time discriminator between
the **authored** channel (eligible for the C02 → `-errno` mapping) and the
**native** Zephyr channel — `DEVICE_MMIO_MAP` / clock / reset / PCIe-probe
failures that already speak negative errno and must bypass this construct
(§E.6).

### E.2 Carrier — `Result<(), RegseqError>` and the `Ok` channel

`rustc` lays the carrier out as a niche-encoded two-variant enum,
`Ok = variant 0`, `Err = variant 1`; generic_c certifies identity via the
compiler-certified `is_result` flag plus an `Ok`/`Err` name guard, and
requires the `Ok` payload be exactly `()` (`mod.rs:565-660`, unit-payload
check `:627-635`). The discriminant is read through the shared section-13
niche-read machinery, and on the `Ok` discriminant the adapter emits
`return 0;` (`body_emit.rs:4819-4821`).

**Zephyr:** the carrier's internal representation is unchanged. The success
channel maps onto Zephyr's universal device-init contract — **an init function
returns `0` on success**
(<https://docs.zephyrproject.org/3.7.0/kernel/drivers/index.html>; in-repo:
`reference-drivers/e1000/zephyr/eth_e1000.c:261` `return 0;` at the end of
`e1000_probe`). So `Ok(()) → return 0;` is **byte-identical** to generic_c on
Zephyr.

### E.3 The boundary adapter — polarity and placement (the Zephyr deltas)

generic_c's adapter is `int <entry>__regseq_exit(void)` (`AdapterSpec` minted
at `mod.rs:521-534`): it (1) calls the faithfully-emitted entry, (2) reads the
niche discriminant and `return 0;` on `Ok`, (3) otherwise `switch`es the inner
`RegseqError` Direct tag-view, one `case <tag_value>: return <SYMBOL>;` per
variant in deterministic variant order (`body_emit.rs:4798-4809`), with
`default: return EIO;` as corrupt-discriminant defense only (`:4824`, I05). A
marked variant absent from the table is `EmitError::ErrnoMapping` — fail-loud,
never a silent `EIO` (`:4807`, R08/I05). Symbols are emitted **symbolically**,
never as host numerics (I03, `errno.rs:11-13`), and `#include <errno.h>` is
demand-driven (`crates/regseq-backends/src/generic_c/prelude.rs:345-350`,
emission `:434-439`).

generic_c emits **positive** errno symbols, correct for a userspace process
exit code. Zephyr's API-wide convention is **negative errno on failure, `0` on
success**
(<https://docs.zephyrproject.org/3.7.0/develop/api/design_guidelines.html#return-codes>;
every error return in the vendored Zephyr drivers is negated —
`reference-drivers/e1000/zephyr/eth_e1000.c:216` `return -ENODEV;`, `:132`
`return -EIO;`, `reference-drivers/pl011/zephyr/uart_pl011.c:173`
`return -EINVAL;`).

The Zephyr adapter is therefore the generic_c adapter with **two substitutions
and nothing else**:

1. **Negate every error arm**: `case <tag_value>: return -<SYMBOL>;` and
   `default: return -EIO;`. The `Ok` arm stays `return 0;`.
2. **Relocate the boundary** from a standalone `<entry>__regseq_exit` +
   harness `main` to the **device init-function return site** (§E.4) when the
   TU has lifecycle drivers.

The third Linux delta — the include name — is **a non-delta on Zephyr**:
Zephyr's libc provides standard `<errno.h>` defining all nine C02 symbols
(minimal libc:
<https://elixir.bootlin.com/zephyr/v3.7.0/source/lib/libc/minimal/include/errno.h>),
so the demand-driven `HelperUsage` errno-include machinery
(`prelude.rs:345-350`, `:434-439`) is reused **verbatim, including the header
name**, and a no-error TU stays byte-unchanged (I02).

Everything else is reused verbatim: the niche read, the tag-view switch,
symbolic-only emission, deterministic variant order, and the
fail-loud-on-unmapped rule.

### E.4 Where the boundary lives on Zephyr — callback exits

- **`Lifecycle::init` ⇒ the device init function** registered with
  `DEVICE_DT_INST_DEFINE` — `static int <drv>_init(const struct device *dev)`
  (`reference-drivers/pl011/zephyr/uart_pl011.c:570` signature, `:789` and
  `:821` registration; `reference-drivers/e1000/zephyr/eth_e1000.c:208`
  `e1000_probe`, registered at `:337-338` via `ETH_NET_DEVICE_DT_INST_DEFINE`).
  The synthesized lifecycle path already captures `on_init`'s `Result` and
  branches on its discriminant (`body_emit.rs:5340-5354` +
  `emit_init_fail_body` `:5409-5479`). On Zephyr this read is **fused** with
  the §E.3 conversion: `Ok` → fall through to `return 0;`; `Err` → run the
  init-failure cleanup (`on_init_failure` + reverse postamble, the
  device-lifecycle construct's teardown machinery) **and then**
  `return -<errno>;`. This is the principal cross-construct composition point,
  identical in shape to the Linux probe fusion.
- **What the returned errno does on Zephyr** (a real semantic difference from
  Linux): the kernel records a failed init's result into
  `dev->state->init_res` as a **positive `uint8_t`, clamped to `UINT8_MAX`**
  (`do_device_init`,
  <https://elixir.bootlin.com/zephyr/v3.7.0/source/kernel/init.c>; field doc
  "Device initialization return code (positive errno value)",
  <https://elixir.bootlin.com/zephyr/v3.7.0/source/include/zephyr/device.h>),
  and `device_is_ready()` subsequently reports the device unusable. The
  authored `RegseqError` variant identity thus **partially survives at
  runtime** as the clamped positive errno — Zephyr offers no module-unload
  path that would discard it.
- **`Lifecycle::deinit`** — Zephyr **3.7 LTS has no device-deinit channel at
  all**: statically defined devices are never torn down, so a `RegseqError`
  escaping `deinit` has **no return channel** and can only be logged
  (composing with diagnostics-logging, out of this section's scope). Newer
  Zephyr added `device_deinit()` with an `int`-returning deinit op
  (`DEVICE_DT_DEINIT_DEFINE`) — present in v4.2.0 `include/zephyr/device.h`,
  **absent in v4.1.0** (review-checked; pin confirmation = open-questions C-3).
  Version-sensitive — **open decision ZE-OQ2** (mirrors Linux D-3).
- **`on_surprise_remove` / `on_system_shutdown`** — no Zephyr device-core
  errno channel exists for either (no hot-unplug protocol for static DT
  devices; no per-device shutdown callback in the base device model). Same
  disposition as deinit: log-only.
- **Standalone authored entry (no lifecycle)** — generic_c's `ErrResult` shim
  has `main` return the adapter's errno
  (`crates/regseq-backends/src/generic_c/naming.rs:146-156`), and with a
  lifecycle runner does the B3 early-errno-return (`naming.rs:181-182`).
  Zephyr supports an application `int main(void)`, but **the kernel ignores
  its return value**
  (<https://docs.zephyrproject.org/3.7.0/kernel/services/threads/system_threads.html>)
  — there is no process exit code. The adapter itself (an ordinary `int`
  function) is retained unchanged with §E.3 polarity, and the B3 gating
  composition (`if (rc != 0) return rc;` before any runner) still **executes**
  correctly inside `main`; only the *observability* of the final rc is lost.
  How to surface it (LOG_ERR composition vs. a harness-specific channel) is
  **open decision ZE-OQ3**.

### E.5 Recommended shape: faithful carrier + boundary-only conversion

Follow the as-built generic_c shape exactly: authored bodies produce/consume
the `Result` C struct faithfully, and **only** the boundary adapter (and the
fused init-fail read) ever inspects it. The adapter stays hand-emitted for the
same reason generic_c's is (`body_emit.rs:4709-4716`): model constants are
numeric/relocs with no symbolic-macro form, so a model-level body cannot emit
`return -EINVAL;`. This reuses the entire certified discriminant-read
machinery, keeps errno conversion out of authored helper bodies, and same-type
`?` on `Result<_, RegseqError>` already normalizes pre-translation (resolved
in regseq-driver-92).

### E.6 Cross-construct composition — authored vs native channel

Native operational failures are a **different error channel** from authored
`RegseqError` (`regseq-error-spec.md:134-153`;
`docs/discussions/regseq-error/perspective-6-authored-versus-native-error-boundary.md`).
On Zephyr both channels share the `int` return type and the negative-errno
vocabulary, but must not be conflated:

- **Authored channel** — only values constructed as `RegseqError::*` flow
  through the §E.3 adapter; the marker certifies membership.
- **Native channel** — the compiler-generated bring-up preamble
  (`enable_device → claim_regions → set_bus_master → get_resource_handle → map_region`,
  ordered inside lifecycle synthesis at `body_emit.rs:5256-5284`) lowers to
  native Zephyr APIs whose failures already speak negative errno and propagate
  **directly**, never round-tripped through `RegseqError`. The vendored
  drivers show both halves: `e1000_probe` returns a native `-ENODEV` when no
  PCIe BDF matched, *before* any authored logic could run
  (`eth_e1000.c:213-216`), and `pl011_init` propagates native reset/pinctrl
  failures as raw `ret` (`uart_pl011.c:584-586`, `:605-607`).
- **No deferral protocol to protect.** Linux's sharpest hazard —
  `-EPROBE_DEFER` must survive untouched — has **no Zephyr analogue**: Zephyr
  initializes statically-defined devices exactly once, in init-level/priority
  order, and a missing dependency surfaces as `device_is_ready() == false` on
  the consumer side, not as a retry errno
  (<https://docs.zephyrproject.org/3.7.0/kernel/drivers/index.html>). The
  adapter's closed nine-code set (+ defensive `-EIO`) therefore cannot collide
  with any kernel-recognized control errno on Zephyr.

### E.7 The nine variant→errno mappings on Zephyr

The C02 table (`errno.rs:35-48`) is name-keyed and order-independent;
`HardwareError` and `GenericFailure` intentionally share `EIO`
(`errno.rs:22-24`). All nine symbols are defined by Zephyr's libcs, including
minimal libc
(<https://elixir.bootlin.com/zephyr/v3.7.0/source/lib/libc/minimal/include/errno.h>).
The Zephyr table is **symbol-identical** to generic_c's; as on Linux, the
per-target divergence is **polarity, not symbol choice**:

| Variant | C02 symbol (`errno.rs`) | Zephyr emits |
|---|---|---|
| `InvalidParameter` | `EINVAL` (`errno.rs:37`) | `return -EINVAL;` |
| `OutOfMemory` | `ENOMEM` (`errno.rs:38`) | `return -ENOMEM;` |
| `HardwareError` | `EIO` (`errno.rs:39`) | `return -EIO;` |
| `Timeout` | `ETIMEDOUT` (`errno.rs:40`) | `return -ETIMEDOUT;` |
| `Unsupported` | `EOPNOTSUPP` (`errno.rs:41`) | `return -EOPNOTSUPP;` (see ZE-OQ1) |
| `DeviceBusy` | `EBUSY` (`errno.rs:42`) | `return -EBUSY;` |
| `PermissionDenied` | `EPERM` (`errno.rs:43`) | `return -EPERM;` |
| `DeviceNotFound` | `ENODEV` (`errno.rs:44`) | `return -ENODEV;` |
| `GenericFailure` | `EIO` (`errno.rs:45`) | `return -EIO;` |

One idiom note: Zephyr driver code conventionally signals "not supported" with
`-ENOTSUP` (vendored example `uart_pl011.c:264`; note this is `ENOTSUP`, a
perfectly public Zephyr symbol — *not* Linux's kernel-internal `ENOTSUPP`).
Whether `Unsupported` should align with that idiom or stay symbol-identical to
the C02 table is **open decision ZE-OQ1**; the recommendation is to stay
symbol-identical (keeps the table per-target-invariant; `-EOPNOTSUPP` is valid
and used in Zephyr).

### E.8 Error / lifetime semantics summary

- **Success**: `Ok(())` → `return 0;` — identical to generic_c
  (`body_emit.rs:4819-4821`) and to the Zephyr init contract.
- **Authored failure**: `Err(RegseqError::V)` → `return -errno_symbol(V);` at
  the init-function boundary, **after** the device-lifecycle init-failure
  cleanup runs. Fail-loud at emit time if `V` is marked but unmapped
  (`body_emit.rs:4807`); never silent.
- **Corrupt discriminant**: `default: return -EIO;` — defensive only (I05,
  mirrors `body_emit.rs:4824`).
- **Native failure**: bypasses this construct entirely; the native negative
  errno propagates directly (§E.6).
- **Post-failure lifetime**: the negative errno is absorbed by the kernel into
  `dev->state->init_res` as a clamped positive `uint8_t`; the device is
  permanently not-ready for this boot
  (<https://elixir.bootlin.com/zephyr/v3.7.0/source/kernel/init.c>). All nine
  symbols' values fit in `uint8_t` in Zephyr's libcs, so clamping does not
  alias them in practice (verification folded into ZE-OQ4).
- **Determinism (I03)**: emission stays symbolic (`-EINVAL`); the Zephyr
  toolchain's `<errno.h>` resolves numerics. Variants emit in deterministic
  variant order (`body_emit.rs:4798-4801`).

### E.9 Scaffold points

- **Marker recognition + entry classification**: reuse `mod.rs:565-660` and
  `:670-679` unchanged.
- **Carrier discriminant read**: reuse the section-13 niche-read machinery
  (`body_emit.rs:4709-4828` consumes it) unchanged.
- **Adapter body**: a Zephyr analogue of `emit_exit_adapter`
  (`body_emit.rs:4732-4828`) that negates the error arms; for lifecycle TUs it
  is fused into the `DEVICE_DT_INST_DEFINE`-registered init function's return
  path rather than a standalone `__regseq_exit`.
- **errno table**: reuse `errno.rs:35-48` verbatim (symbols identical); negate
  at the emit site only.
- **Demand-driven include**: reuse `prelude.rs:345-350` / `:434-439` verbatim
  — header name `<errno.h>` unchanged on Zephyr.
- **Composition with device-lifecycle init-fail**: fuse the §E.4 discriminant
  read with `emit_init_fail_body`'s cleanup branch (`body_emit.rs:5340-5354`,
  `:5409-5479`), exactly as the Linux spec does
  (`docs/discussions/linux-backend/linux-lowering-per-construct.md` §E.4/§E.9).
- **Entry-shim analogue**: the `EntryShim::ErrResult` / B3 shapes
  (`naming.rs:73-103`, `:146-156`, `:181-182`) carry over structurally into
  Zephyr `main()`, with rc observability deferred to ZE-OQ3.
