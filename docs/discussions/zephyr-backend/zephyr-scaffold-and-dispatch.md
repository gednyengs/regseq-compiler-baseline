# Zephyr scaffold & dispatch — the TU shell the primitives wire into

The per-construct spec
([`zephyr-lowering-per-construct.md`](./zephyr-lowering-per-construct.md)) says
what each primitive *becomes*. This document says where those lowered
primitives *live*: the driver translation-unit scaffold, the build-time
instantiation block, the per-instance **init function** (the one runtime
dispatch hub), the scenario→entry-point collapse, and the emission of
diagnostics-logging and the RegseqError adapter into that scaffold.

The governing fact, established in
[`zephyr-lowering-per-construct.md`](./zephyr-lowering-per-construct.md) §C.0:
on Zephyr **the kernel's init-level walk is the runner, and registration is a
build-time fact**. `generic_c` synthesizes a driving `main` + per-scenario
functions and an `EntryShim`; Linux hoists registration to module load and
keeps `.probe`/`.remove`/`.shutdown` runtime slots; Zephyr emits **none** of
that — a driver is a set of statically instantiated device objects, one per
status-okay DT node (`DT_INST_FOREACH_STATUS_OKAY` → `DEVICE_DT_INST_DEFINE`,
uart_pl011.c:789-793; eth_e1000.c:337-346), and the only runtime lifecycle
artifact is the per-instance init function. The four `LifecycleScenario`
variants (`backend_options.rs:24-42`) are therefore **not an emission input**:
the emitted scaffold is scenario-independent, and the scenarios collapse to
the kernel's natural paths (§5).

(For Zephyr the registration framework is `DT_DRV_COMPAT` +
`DEVICE_DT_INST_DEFINE`. The Linux analogue `module_*_driver` + driver structs
is *not* a Zephyr artifact; it appears only as the cross-OS contrast.)

---

## 1. The emitted TU, top to bottom

A lifecycle TU emitted for Zephyr contains *only* the scaffold below — no
`main`, no runner, no per-scenario functions (RD-7's no-lifecycle byte-purity
is trivially preserved: a no-lifecycle TU emits no scaffold,
[`zephyr-lowering-per-construct.md`](./zephyr-lowering-per-construct.md) §C.10).
Annotated skeleton, every line grounded in the vendored drivers:

```c
/* 1. compatible pin — from MatchEntry::Devicetree (uart_pl011.c:9) */
#define DT_DRV_COMPAT <compatible_as_token>

/* 2. diag-log module block — demand-driven, iff ≥1 log site (eth_e1000.c:9-12) */
#define LOG_MODULE_NAME <sanitize(DriverDescriptor.name)>
#include <zephyr/logging/log.h>
LOG_MODULE_REGISTER(LOG_MODULE_NAME);

/* 3. includes — demand-driven, same discipline as generic_c's <errno.h>
      (prelude.rs:345-350): <zephyr/device.h>, <zephyr/sys/sys_io.h>,
      <zephyr/sys/device_mmio.h>, <errno.h> (RegseqError TUs — header name
      unchanged on Zephyr), <zephyr/drivers/pcie/pcie.h> (Pci bus only,
      eth_e1000.c:18), <zephyr/pm/device.h> (iff on_system_shutdown) */

/* 4. per-instance state — the Policy-P morph split (§C.8) */
struct <drv>_config {
    DEVICE_MMIO_ROM;                  /* FIRST member (uart_pl011.c:42-43) */
};
struct <drv>_data {
    DEVICE_MMIO_RAM;                  /* FIRST member (uart_pl011.c:63-64) */
    struct <spec_driver> inner;       /* spec driver embedded verbatim (CR-7) */
    uint32_t __rseq_state;            /* retained; zero in the static image */
    /* Pci only: struct pcie_dev *pcie;  (eth_e1000_priv.h:92-93) */
};

/* 5. authored helper fns + Field/FieldValue const aggregates — the faithful
      body bank, unchanged from generic_c's agnostic core (bank map: §9) */

/* 6. optional PM hook — ONLY when on_system_shutdown is overridden (§4) */

/* 7. the init function — the one runtime dispatch hub (§3) */
static int <drv>_init(const struct device *dev) { ... }

/* 8. instantiation block — one per DT instance (§2) */
#define RSEQ_INIT(n)                                                  \
    /* Pci only: DEVICE_PCIE_INST_DECLARE(n);  (eth_e1000.c:313-314) */ \
    static struct <drv>_data data_##n = {                             \
        /* Pci only: DEVICE_PCIE_INST_INIT(n, pcie), */               \
    };  /* remainder zero-filled — C static image = generic_c's {0} */ \
    static const struct <drv>_config cfg_##n = {                      \
        DEVICE_MMIO_ROM_INIT(DT_DRV_INST(n)),  /* (uart_pl011.c:758-759) */ \
    };                                                                \
    DEVICE_DT_INST_DEFINE(n, <drv>_init, <pm_or_NULL>,                \
        &data_##n, &cfg_##n, <level>, <priority>,                     \
        NULL /* api — ClassTag::Generic only; non-Generic diagnoses (Q8a) */);
DT_INST_FOREACH_STATUS_OKAY(RSEQ_INIT)

/* 9. single-instance guard — REQUIRED while the faithful TU-static base
      carrier is in effect (per-construct §A.2.2, OQ-3/A-2): a TU-static
      mm_reg_t with >1 instance silently aliases one base across devices */
BUILD_ASSERT(DT_NUM_INST_STATUS_OKAY(DT_DRV_COMPAT) == 1,
             "TU-static MMIO base requires a single status-okay instance");
```

Multi-entry match sets repeat blocks 1+8 per compatible — the vendored pl011
idiom (`#undef DT_DRV_COMPAT` / second foreach, uart_pl011.c:9-10, :795-829).
Build sidecars (DT binding yaml + Kconfig) accompany the TU
(arm_pl011.yaml; intel,e1000.yaml, Kconfig.e1000); whether the compiler emits
them or treats them as integration inputs is part of open question Q5.

---

## 2. Build-time instantiation — register / match / create / associate

Four canonical steps are emitted as the **instantiation block**, not as code
that runs (the time-split table,
[`zephyr-lowering-per-construct.md`](./zephyr-lowering-per-construct.md) §C.0):

| canonical step | scaffold artifact | witness |
|---|---|---|
| `register` + match | `DT_DRV_COMPAT` + binding yaml `compatible:` + `DT_INST_FOREACH_STATUS_OKAY(...)` → `DEVICE_DT_INST_DEFINE(...)` | uart_pl011.c:9, :789-793; arm_pl011.yaml:3 |
| `create` | `static struct <drv>_data data_##n` + `static const struct <drv>_config cfg_##n` — static-storage zero image = `inst = {0}` | uart_pl011.c:758-787; eth_e1000.c:316-318, :333-335 |
| `associate` | the `&data`/`&config` arguments of the define macro; read-back is `dev->data`/`dev->config` in every callback | uart_pl011.c:789-791, :572-573; eth_e1000.c:211 |
| bus selector (BRM) | macro family + context fields: Platform/Amba → plain define; Pci → + `DEVICE_PCIE_INST_DECLARE/INIT` | eth_e1000.c:313-318, :337-346 |

Three Zephyr-only degrees of freedom surface here, all flagged, none silently
resolved: the **init level/priority** arguments (no model surface; the
vendored drivers differ and the level is class-coupled — Q7), the **api
vtable** argument (`ClassTag` is recognized-not-lowered, so the class-neutral
form passes `NULL` for `ClassTag::Generic` **only**; a non-Generic tag is an
emit-time diagnostic, per-construct §C.9 — Q8a), and the **`descriptor.name`
carrier** (no single Zephyr slot; spent on the log module name and symbol
prefixes — Q6).

Registration cannot fail at runtime (it is a build-time fact); PCI device
absence is checked natively at init entry
(`dev->pcie->bdf == PCIE_BDF_NONE → return -ENODEV;`, eth_e1000.c:215-217).
The generic_c `binding=bound|none` trace argument is a host artifact with no
Zephyr realization (`body_emit.rs:5145-5153`).

---

## 3. The init function — the one runtime dispatch hub

The kernel invokes `static int <drv>_init(const struct device *dev)` once per
instance at its init level (uart_pl011.c:570, :789-790; eth_e1000.c:208,
:337-342). Everything from the resource preamble through `on_init` lowers into
its body **in canonical order** (`body_emit.rs:5209-5311`), with the BRM block
nested between associate and `on_init` exactly as generic_c synthesizes it:

```
platform/DT path                          PCIe path
----------------                          ---------
(native) side-resource preambles, in      BDF presence guard → -ENODEV
  declaration order: reset/clock/           (eth_e1000.c:215-217, native)
  pinctrl (uart_pl011.c:582-609)
[claim_regions: zero calls — the DT       pcie_get_mbar(bdf, effective_bar,
 `reg` capture happened at build]           &mbar)   (BAR-number accessor,
                                            §B.3.4; reference ordinal idiom
                                            pcie_probe_mbar, eth_e1000.c:219;
                                            check validity → -EINVAL/-ENODEV,
                                            §B.6)
[enable_device: not emitted, default-on]  pcie_set_cmd(bdf, CMDSTAT_MEM, true)
                                            (+ MASTER iff dma_present — §B.3.1;
                                            probe→enable→map = the one recorded
                                            ordering deviation, BRM-O2/A-3,
                                            pending ratification)
DEVICE_MMIO_MAP(dev, K_MEM_CACHE_NONE)    device_map(&slot, mbar.phys_addr,
  (canonical slot: after the preambles,     mbar.size, K_MEM_CACHE_NONE)
   before any MMIO op, §A.2.2; pl011's      (eth_e1000.c:223-224)
   literal map-first layout satisfies
   only the weaker witnessed constraint,
   uart_pl011.c:577-580)
authored on_init body (spec-view over     same
  &((struct <drv>_data*)dev->data)->inner)
Ok  → return 0;                           same
Err → on_init_failure cleanup, then       same
      return -<errno>;  (fused adapter, §6)
```

Native failures inside this body (`-ENODEV`, reset/pinctrl status) return
**directly** — they never touch the RegseqError adapter (the two-channel rule,
[`zephyr-integration-probe-path.md`](./zephyr-integration-probe-path.md) §4).
After a nonzero return the device is permanently not-ready
(`device_is_ready() == false`; pin flagged Q9) — there is no re-probe and no
`-EPROBE_DEFER` analogue.

---

## 4. The optional PM slot — `on_system_shutdown`

The lone omit-when-absent callback (D10/I02, `body_emit.rs:5355-5366`) maps
structurally onto the **third argument of `DEVICE_DT_INST_DEFINE`**:

- **Overridden** → emit `PM_DEVICE_DT_INST_DEFINE(n, <drv>_pm_action)` and
  pass `PM_DEVICE_DT_INST_GET(n)` — the vendored conditional idiom
  (uart_pl011.c:106-117, consumed at :771, :789). The PM contract is an
  **action callback** over `enum pm_device_action`, not a dedicated
  shutdown hook (pinned:
  <https://docs.zephyrproject.org/3.7.0/services/pm/device.html>; not
  live-verified, Q3), so the emitted artifact is a generated dispatch wrapper
  `static int <drv>_pm_action(const struct device *dev, enum pm_device_action action)`:
  the Q3-chosen action(s) run the author body (its `Result` discarded, CR-4;
  `return 0;`); **every other action returns `-ENOTSUP`** per the PM contract.
- **Not overridden** → pass `NULL` (uart_pl011.c:115-116; eth_e1000.c:339).
  No PM artifact is emitted at all — the exact omit-when-absent contract.

Which PM action fires at system power-off and under which `CONFIG_PM_DEVICE*`
options is config/version-sensitive and **not asserted** (Q3); the slot
mapping stands on the in-repo citations regardless. The primitive is therefore
**slot-mapped, trigger-unresolved** — not a claimed materialization until
Q3/C-4 closes (post-review, Codex issue 4; interim emit-vs-diagnose policy =
open-questions A-18).

---

## 5. Scenario → dispatch collapse

`generic_c` emits one function per driver per selected scenario
(`body_emit.rs:5191-5193`) plus a runner + `EntryShim` `main`. Zephyr emits
the scenario-independent scaffold once, and the four scenarios collapse to
kernel paths:

| scenario (`backend_options.rs:24-42`) | generic_c artifact | Zephyr path |
|---|---|---|
| orderly (bring-up) | `__rseq_lc_*_orderly` | init fn returns 0 at boot; orderly *teardown* has **no baseline path** (Q1) |
| init-fail | `__rseq_lc_*_initfail` + `emit_init_fail_body :5409-5483` | init fn error return; kernel records `init_res`; device never ready |
| surprise | `__rseq_lc_*_surprise` | **no path** — static DT devices cannot vanish (Q2) |
| shutdown | `__rseq_lc_*_shutdown` (`:5355-5366`) | system power-off → the §4 PM action (Q3) |

The behavioral oracle shifts accordingly: generic_c asserts an `LC <event>`
stdout trace + clean exit; Zephyr's observables are the init return code /
`device_is_ready()` state and the structured log stream — flagged as an
oracle divergence, not silently substituted
([`zephyr-lowering-per-construct.md`](./zephyr-lowering-per-construct.md) §C.5).

**Teardown dispatch is not emitted.** `emit_teardown_dispatch`'s `present=1|0`
bit, the `on_deinit`/`on_surprise_remove` calls, and the reverse postambles
(`body_emit.rs:5391-5406`) have no slot on the 3.7 baseline — see
[`zephyr-integration-probe-path.md`](./zephyr-integration-probe-path.md) §5
for the synthesized body specified *for whenever a slot exists* (Q1).

---

## 6. RegseqError adapter emission into the scaffold

Per [`zephyr-lowering-per-construct.md`](./zephyr-lowering-per-construct.md)
§E.3/§E.4 — the generic_c adapter (`emit_exit_adapter`,
`body_emit.rs:4709-4828`) with two substitutions and nothing else:

1. **Lifecycle TU**: no standalone `<entry>__regseq_exit` is emitted. The
   niche-discriminant read of `on_init`'s `Result` (already synthesized by the
   init-fail path, `body_emit.rs:5340-5354`) is **fused** with the errno
   conversion at the init fn's return site: `Ok` → `return 0;`; `Err` →
   `on_init_failure` cleanup, then `switch` on the tag-view →
   `case <tag>: return -<SYMBOL>;` per variant in deterministic order,
   `default: return -EIO;` (defensive only, I05). Fail-loud at emit time on
   any marked-but-unmapped variant (`body_emit.rs:4807`, R08).
2. **Standalone-entry TU** (no lifecycle): the adapter function is kept and
   called from Zephyr's application `int main(void)`, with the B3
   early-errno-return shape (`naming.rs:181-182`) preserved. The kernel
   ignores `main`'s return value, so the rc executes but is unobservable —
   ZE-OQ3.

The `#include <errno.h>` stays demand-driven via the reused `HelperUsage`
machinery (`prelude.rs:345-350`, `:434-439`) — **header name unchanged** on
Zephyr (minimal libc provides all nine C02 symbols); a no-error TU stays
byte-unchanged (I02).

---

## 7. Diagnostics-logging emission into the scaffold

Per [`zephyr-lowering-per-construct.md`](./zephyr-lowering-per-construct.md)
§D.5: the file-scope module block (skeleton item 2) is emitted **iff the TU
contains ≥1 intercepted `Log` call** — the same demand-driven discipline as
the errno include. The module name is the sanitized `DriverDescriptor.name`
(the device-lifecycle seam, inverted in granularity vs Linux: per-TU module,
not per-call `dev`); fallback for a TU with no `DriverDescriptor` is open
(LOG-O4, recommend the TU emission name). Each log site then lowers to exactly
one `LOG_DBG/INF/WRN/ERR("<F>", <A…>)` statement + the successor `goto` in
place of its `Call` terminator — no other scaffold interaction. The
compile-time `LOG_LEVEL` symbol choice (per-driver Kconfig vs forced
`LOG_LEVEL_DBG`) is LOG-O3; the Kconfig baseline pins (`CONFIG_LOG=y`,
deferred mode, cbprintf-complete, `CONFIG_CBPRINTF_FULL_INTEGRAL` for 64-bit
conversions) are documented in §D.9 / LOG-O5.

---

## 8. What is deliberately NOT in the scaffold

- **`main` / `__rseq_lc_run` / `EntryShim`** — no Zephyr artifact; the
  kernel's init-level walk is the runner (§C.10).
- **Teardown slots** — no remove/unbind argument exists in
  `DEVICE_DT_INST_DEFINE` (uart_pl011.c:789-791; eth_e1000.c:337-344); Q1/Q2.
- **`claim_regions` calls** — zero calls on both paths (the called-out
  elision, §B.3.2).
- **Unmap / release pairing** — mappings are permanent; nothing was claimed
  (§B.6).
- **A match table artifact** — no `MODULE_DEVICE_TABLE`/modalias analogue;
  matching is the devicetree build (§C.2).
- **A class API vtable** — `ClassTag` is recognized-not-lowered; `NULL` api
  build-now for `ClassTag::Generic` only — non-Generic tags diagnose at emit
  time (per-construct §C.9, Q8a).

---

## 9. The agnostic core, bank by bank — reused / forked / target-specific

(Added post-review, Codex issue 7.) The input contract requires a backend to
be a fold over `model.instances` plus the two side tables, with the C-ABI
seam, symbol derivation, constants/statics/strings, and an out-of-band
`BackendContext` all honored
(docs/discussions/backend-lowering/regseqmodel-input-contract.md:28-48,
:50-69, :109-124, :126-139). generic_c realizes that contract as a five-bank
single-TU assembly (`crates/regseq-backends/src/generic_c/mod.rs:1-13`, driver
`mod.rs:232-328`). The Zephyr backend's disposition for **every** bank:

| generic_c core surface | citation | Zephyr disposition |
|---|---|---|
| five-bank TU ordering driver (prelude → helpers → banks 2-5) | `mod.rs:232-328` (`:271-274`, `:325-328`) | **reused** — the emitted driver TU is still one ordered C file; scaffold items 1-4/6-9 (§1) slot around the banks as Zephyr's bank-1-adjacent shell |
| bank 1: static prelude + arithmetic/cast helper library | `mod.rs:681`; `prelude.rs` | **forked** — the host stdio/hook prelude (trace hooks, `fwrite` plumbing) is replaced by the §1-item-3 Zephyr include set; the arithmetic/cast helper library and `rust_str` are **reused** |
| helper-demand tracking (`HelperUsage`, demand-driven includes) | `prelude.rs:345-350`, `:434-439` | **reused verbatim** — drives `<errno.h>` (§6) and the log-module block (§7) emission gates |
| name tables + symbol derivation (sanitize, generic suffix, `_clash_<n>` net) | `naming.rs:202`, `:279-287`; `mod.rs:1043-1054`; input-contract §2.4 | **reused verbatim** — `emission_name` → C symbol is OS-independent |
| discovery roots (aggregate/fat-pointer census banks 2-5 must name) | `mod.rs:868-877`, `:884-924` | **reused verbatim** |
| banks 2-3: aggregate forward-decls + definitions + layout guards | `mod.rs:1226-1310` | **reused verbatim** — `#[repr(C)]` layout is target-ABI, not OS, territory |
| banks 4-5: declarations + definitions (statics, consts, function bodies) | `mod.rs:273-274`; `body_emit.rs` | **reused** — body lowering is the shared procedural core; only the op-site materializations substitute per the per-construct doc (sys_io, pcie_*, LOG_*) |
| C-ABI side table (`sig_c_abi_lowerings`, boundary vs body-local types) | `mod.rs:1330-1362`; input-contract §2.3 | **reused verbatim** |
| target ABI facts (`pointer_width`, `endian`) | input-contract §1 table; `body_emit.rs:370-372`, `:3209` | **reused** — sourced from `model.target`, now an ARM/x86 Zephyr target rather than host |
| entry-shim classification | `mod.rs:432-540`; `naming.rs:73-103` | **classification reused; emission target-specific** — no synthesized `main`/runner (§C.10); standalone-entry TUs keep the adapter called from Zephyr's `main` (§6) |
| RegseqError errno adapter | `mod.rs:521-534`; `body_emit.rs:4709-4828`; `errno.rs:35-48` | **reused with the two §6 substitutions** (negated arms; init-fn return-site fusion) |
| `BackendContext` (options + `PlatformIdentity`) | input-contract §1.1; `backend_options.rs:129` | **target-specific** — `PlatformIdentity` selects the Zephyr framework (generic_c only traces it); `LifecycleScenario` options stop being an emission input (§5) |
| lifecycle synthesis (markers, preamble/BRM ordering, morph, init-fail) | `body_emit.rs:5174-5483` | **mechanism reused, artifacts re-homed** — the ordered steps re-partition across build time and the init fn (§§2-3); the scenario fns/runner are not emitted |
