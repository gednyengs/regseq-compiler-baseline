# Zephyr integration — the end-to-end probe/init path

The composition is the thing being specified: the five constructs do not lower
in isolation. Zephyr preserves `generic_c`'s bring-up ordering
(`crates/regseq-backends/src/generic_c/body_emit.rs:5197-5311`) but
re-partitions it **harder than Linux does**: where Linux hoists only
`register_driver` to module load (linux-integration-probe-path.md:17-35),
Zephyr hoists registration, matching, creation, AND association to **build
time**. `generic_c` is the host model; every Zephyr call target below is a
**proposed materialization grounded in the vendored reference drivers** —
cited per line — never asserted as implemented. `TargetOs::Zephyr` exists in
the model as a closed-vocabulary token
(`crates/regseq-model/src/target.rs:69-74`, parse :91-97) that `generic_c`
carries but does not act on (target.rs:108-109); this spec is what a Zephyr
backend would act on.

## 1. Structural seam: build-time instantiation, boot-time init

`generic_c` emits the whole bring-up (`register → create → associate →
preamble → BRM acquire → map_region → on_init`) inside one scenario function
per driver (`body_emit.rs:5191-5373`). Zephyr splits the same ordered steps
across **three binding times**:

1. **Build time (macro expansion + devicetree compile)** — `register_driver`,
   the match set, `create`, and `associate`:
   - `regseq_lc_register` + the bus selector (`body_emit.rs:5197-5205`;
     selector tokens :5050-5061, whose comment already names
     `DEVICE_DT_DEFINE` as the Zephyr target :5045-5049) →
     `DEVICE_DT_INST_DEFINE(n, init_fn, pm, data, config, level, prio, api)`
     (uart_pl011.c:789-791) or a framework-class wrapper such as
     `ETH_NET_DEVICE_DT_INST_DEFINE` (eth_e1000.c:337-344).
   - **Matching is devicetree-resolved at build time, for every bus family.**
     `MatchEntry::Devicetree { compatible }`
     (`crates/regseq-model/src/device_lifecycle.rs:112`) →
     `#define DT_DRV_COMPAT arm_pl011` (uart_pl011.c:9; comma→underscore token
     form) + `DT_INST_FOREACH_STATUS_OKAY(PL011_INIT)` (uart_pl011.c:793) —
     one device object per status-okay DT node. Even
     `MatchEntry::Pci { vendor, device }` (device_lifecycle.rs:114-119) is
     DT-mediated on Zephyr: the e1000 matches `DT_DRV_COMPAT intel_e1000`
     (eth_e1000.c:7) with the PCI identity carried by the DT node and bound
     via `DEVICE_PCIE_INST_DECLARE(inst)` / `DEVICE_PCIE_INST_INIT(inst, pcie)`
     (eth_e1000.c:314, :317; binding
     reference-drivers/e1000/zephyr/intel,e1000.yaml). There is **no runtime
     match loop** — `LC register <name> match=…`
     (device-lifecycle-surface-convergence.md:178-180) becomes a compile-time
     fact.
   - `regseq_lc_create` (`body_emit.rs:5206`) → **static per-instance
     allocation**, not runtime alloc:
     `static struct pl011_data pl011_data_port_##n = {…}`
     (uart_pl011.c:773-787) + `static struct pl011_config pl011_cfg_port_##n`
     (uart_pl011.c:758-766); e1000: `static struct e1000_dev dev_##inst`
     (eth_e1000.c:316-318). The zero-init container semantics (`inst = {0}`,
     `body_emit.rs:5301-5306`; user-data Rule-1/Rule-2 zero-init covered by
     it, device_lifecycle.rs:262-279) are satisfied by C static initialization
     of the data struct — partial designated initializers zero the rest.
   - `regseq_lc_associate` (`body_emit.rs:5207`) → wired by the DEFINE macro
     itself: the `struct device` object statically carries the `data`/`config`
     pointers, recovered in callbacks as `dev->config` / `dev->data`
     (uart_pl011.c:572-573). No runtime call exists or is needed.

2. **Boot time, per device object** — everything from the resource preamble
   onward is the body of the **init function** (`pl011_init`,
   uart_pl011.c:570; `e1000_probe`, eth_e1000.c:208), invoked by the kernel at
   the declared init level/priority (`PRE_KERNEL_1,
   CONFIG_SERIAL_INIT_PRIORITY` uart_pl011.c:790; `CONFIG_ETH_INIT_PRIORITY`
   eth_e1000.c:342). Zephyr has no probe deferral (`-EPROBE_DEFER` does not
   exist); cross-device ordering is by init level/priority instead — a backend
   knob (see [`zephyr-open-questions.md`](./zephyr-open-questions.md)).

3. **Scenario re-partition.** generic_c's four scenario helpers
   (`backend_options.rs:25-32`, one fn per scenario `body_emit.rs:5191-5193`)
   do not map to four Zephyr functions: *orderly-success* = init fn returns 0;
   *init-fail* = init fn error path; *orderly/surprise teardown* and
   *shutdown* have **no stock slot** (§5).

## 2. Ordered steps inside the init fn (re-renders `body_emit.rs:5209-5311`)

Steps marked **(native)** can fail with a Zephyr status that propagates
directly, never wrapped in `RegseqError`. Order within the phase is preserved
exactly.

| # | `generic_c` marker | Zephyr platform/DT (pl011) | Zephyr PCIe (e1000) |
|---|---|---|---|
| 3 | `regseq_lc_preamble(res)` forward order (`body_emit.rs:5214-5217`; device_lifecycle.rs:178-187 declaration order) | **(native)** reset toggle (uart_pl011.c:582-589), clock_control_on (:591-596), pinctrl_apply_state (:604-609) — acquired in order before HW touch | — (no side resources in reference) |
| 4a | `regseq_bus_enable_device()` PCI-gated (`:5256-5259`; gate device_lifecycle.rs:96-104) | **not-applicable** — platform devices are default-on (gating matrix bus-resource-management-surface-convergence.md:128) | **(native)** `pcie_set_cmd(bdf, PCIE_CONF_CMDSTAT_MEM \| PCIE_CONF_CMDSTAT_MASTER, true)` (eth_e1000.c:220-221) — **fused with 4c, one call** |
| 4b | `regseq_bus_claim_regions(bar)` every-bus (`:5260-5266`) | **VANISHES — Zephyr has no region-reservation runtime.** This resolves the "no-runtime Zephyr" backend-[Open] (bus-resource-management-surface-convergence.md:133-135): neither vendored driver claims anything before mapping. The host trace-only no-op (prelude.rs:209-211) collapses to zero emitted code. Must be a called-out elision, never a silent drop | same — no claim call exists in eth_e1000.c's probe (:208-262) |
| 4c | `regseq_bus_set_master()` PCI && `dma_present` (`:5267-5270`; placeholder gate device_lifecycle.rs:62-70, dormant — no fixture sets it) | n/a | the `PCIE_CONF_CMDSTAT_MASTER` bit of the same `pcie_set_cmd` (eth_e1000.c:220-221). Note: the reference sets it unconditionally because the real e1000 does DMA; the lowering keeps the model's gate — when `dma_present` is false, emit `PCIE_CONF_CMDSTAT_MEM` alone |
| 4d | `regseq_bus_get_handle()` unconditional, seam anchor (`:5271-5272`) | **moves to build time**: `DEVICE_MMIO_ROM_INIT(DT_DRV_INST(n))` captures the DT `reg` (base,size) into ROM config (uart_pl011.c:43 `DEVICE_MMIO_ROM`, :759 init) — the L2 platform-file join (register-access-surface-convergence.md:214-218) is the devicetree itself | **(native)** runtime: BDF guard `dev->pcie->bdf == PCIE_BDF_NONE → return -ENODEV` (eth_e1000.c:215-217), then the BAR probe resolving `mbar.phys_addr/size` — generated: `pcie_get_mbar(bdf, effective_bar, &mbar)` (BAR-number accessor, per-construct §B.3.4); reference: `pcie_probe_mbar(bdf, 0, &mbar)` (an ordinal-keyed enumerator, eth_e1000.c:213, :219) |
| 4e | `regseq_bus_map_region(base, size, bar)` keystone, strictly **after** 4d (`:5273-5284`; consumes MmioBinding resolved base/size, device_lifecycle.rs:209-229, :236-239) | `DEVICE_MMIO_MAP(dev, K_MEM_CACHE_NONE)` (uart_pl011.c:577) — consumes exactly the ROM_INIT-captured (base,size); mapped base stored per-device in `DEVICE_MMIO_RAM` data (uart_pl011.c:64) | `device_map(&dev->address, mbar.phys_addr, mbar.size, K_MEM_CACHE_NONE)` (eth_e1000.c:223-224) — consumes exactly the BAR probe's output; stored per-device (`mm_reg_t address`, eth_e1000_priv.h:87) |
| 5 | register-access intrinsics now valid (`emit_mmio` `:2283-2323`; the five MmioOps domain_attr.rs:125-136) | `DEVICE_MMIO_GET(dev)` (uart_pl011_registers.h:40-43) yields the block base; the reference's own comment enforces the keystone: "Must be placed after DEVICE_MMIO_MAP" (uart_pl011.c:579-580). Accesses = `sys_read32/sys_write32(base + offset)`; offsets from `#[repr(C)]` recovery unchanged (`recover_register` body_emit.rs:2439-2520) | `ior32/iow32` = `sys_read32/sys_write32((_dev)->address + (_reg))` (eth_e1000_priv.h:118-125) |
| 6 | `regseq_lc_call("on_init")` always (CR-3, `:5308-5311`) + author body iff overridden (`:5318-5319`) | the init-fn body after the map: disable→configure→enable sequence (uart_pl011.c:603-659) | descriptor-ring + MAC setup (eth_e1000.c:226-261) |
| 7 | diag logs interleaved (`emit_diag_log` `:2722-2849`) | `LOG_MODULE_REGISTER` once per TU (eth_e1000.c:9-12; module name ← `DriverDescriptor.name`, device_lifecycle.rs:47-52) + `LOG_DBG/INF/WRN/ERR` (eth_e1000.c:121, :144, :203, :284, :67). Level moves from generic_c's data tag (`level_tag` :6058-6064) to macro selection; the whole fwrite decomposition reassembles into ONE cbprintf format (inverse of generic_c, reusing `diag_specifier_and_cast_for` :5798-5850 whole); literal `%` → `%%`; tag and trailing `\n` dropped (LOG_* adds its own) | same |

**Per-instance mapped base retires the single-global-base guard.** generic_c
rejects >1 resolved region per driver because its host model has one global
mapped base (`max_resolved_bindings_per_driver`, body_emit.rs:5063-5086).
Zephyr stores the mapped base per device object (`DEVICE_MMIO_RAM`
uart_pl011.c:64; `mm_reg_t address` eth_e1000_priv.h:87), so the limitation is
host-model-only — but the spec keeps the single-region restriction until the
model threads per-access region bases.

Two post-review clarifications (Codex issues 2 and 3): **(a)** while the
faithful TU-static base carrier is in effect (OQ-3/A-2), the spec also keeps a
single-**instance** restriction, enforced by the emitted
`BUILD_ASSERT(DT_NUM_INST_STATUS_OKAY(DT_DRV_COMPAT) == 1, …)` guard
(per-construct §A.2.2) — a multi-instance devicetree fails the C build until
the per-device `DEVICE_MMIO_GET(dev)` threading lands. **(b)** the table above
is the normative order: on the platform path generated code keeps generic_c's
canonical preambles-then-map order even though pl011 itself places
`DEVICE_MMIO_MAP` textually first (uart_pl011.c:577 vs :582-609 — its only
documented constraint is map-before-regs-access, :579-580); on the PCIe path
the probe→enable→map permutation is the **single** recorded deviation from the
canonical preamble (BRM-O2/A-3, pending ratification, never silently emitted
both ways).

## 3. The keystone (I04) — preserved verbatim

`map_region` consumes the (base,size) that `get_resource_handle` resolved —
never an independent address (`body_emit.rs:5273-5284`, gated on a resolved
binding `:5250`). Both Zephyr paths prove the same decoupling the convergence
doc claims (bus-resource-management-surface-convergence.md:141-149): swapping
the host literal (base,size) for `DEVICE_MMIO_ROM_INIT`'s DT `reg` (platform)
or the PCIe BAR probe's MBAR (`pcie_get_mbar`) changes **no register-access op** — steps
5-7 are OS-portable unchanged. On Zephyr the platform-path "join" is the
devicetree compile itself: the model's L2 `[mmio.<id>]` (base,size)
(register-access-surface-convergence.md:214-218) is realized by the DT node's
`reg` property, making the L1↔L2 join a build-time fact.

## 4. Two error channels at init (hard boundary)

1. **Authored `RegseqError`** from `on_init` → the exit-adapter machinery
   (`emit_exit_adapter` body_emit.rs:4709-4828; niche read, `Ok → return 0`
   :4819-4821) with the nine-row errno table (`errno.rs:35-48`; variant
   catalog docs/discussions/regseq-error/regseq-error-spec.md:221-229). Zephyr
   init fns return 0 or a **negative errno** — the reference returns `-EINVAL`
   (via pl011_set_baudrate, uart_pl011.c:171-174) and `-ENODEV`
   (eth_e1000.c:216) — so, exactly as on Linux, only the **return form is
   shell**: `generic_c`'s `return EINVAL;` becomes `return -EINVAL;` from the
   init fn. Errno symbols themselves carry over (Zephyr libc defines them),
   with one convention flag: `Unsupported → EOPNOTSUPP` (errno.rs:41) vs
   Zephyr's in-tree `-ENOTSUP` habit (uart_pl011.c:264) — open question.
2. **Native Zephyr status** from compiler-generated preamble steps propagates
   **directly**: `return ret` from reset toggle (uart_pl011.c:584-588) and
   pinctrl (:605-609), `return -ENODEV` on the missing-BDF guard
   (eth_e1000.c:215-217). Never wrapped in `RegseqError`. A nonzero init
   return leaves the device object failed — `device_is_ready()` returns false
   thereafter (Zephyr device model docs,
   https://docs.zephyrproject.org/3.7.0/kernel/drivers/index.html) — it does
   NOT unload anything. The split is authored-vs-compiler-generated code, not
   syntax (same boundary as linux-integration-probe-path.md:72-86).

`poll_until` stays **recognized-not-lowered** (domain_attr.rs:94-95, :172; no
generic_c emission) — no Zephyr lowering is invented here; the candidate idiom
is an open question.

## 5. The teardown / remove path

### 5.1 The headline divergence: stock Zephyr has no remove path

Both vendored references define **no teardown function at all**: uart_pl011.c
registers only `pl011_init` (uart_pl011.c:789-791) and eth_e1000.c only
`e1000_probe` (eth_e1000.c:337-344); neither file contains an unmap, disable,
or release call after init. Devices created by `DEVICE_DT_INST_DEFINE` are
static ROM objects whose lifetime is the system's lifetime — there is no
module unload, no hot-unplug dispatch for DT devices, and no `devm`-style
automatic unwind stack. Consequently the four generic_c teardown semantics
(`emit_teardown_dispatch` body_emit.rs:5391-5406; scenarios :5314-5369) map to
Zephyr as follows — two with proposed slots, two flagged open:

| scenario (`body_emit.rs`) | `present` | Zephyr slot | status |
|---|---|---|---|
| **orderly** (`:5317-5322`) | 1 | no stock slot in the vendored idiom; candidates: Zephyr 4.x `device_deinit()` or a PM device action (`PM_DEVICE_ACTION_SUSPEND/TURN_OFF`, https://docs.zephyrproject.org/3.7.0/services/pm/device.html) — both version/Kconfig-sensitive | **open question** (slot choice); the synthesized *body* is specified below |
| **surprise** (`:5323-5339`) | 0 | no DT-device hotplug model exists; PCIe surprise removal is outside the vendored idiom | **open question** — possibly not representable on Zephyr |
| **init-fail** (`:5340-5354` → `emit_init_fail_body` `:5409-5483`) | partial | the init fn's error-return path | **lowered** (§5.3) |
| **shutdown** (`:5355-5366`) | n/a | no per-device shutdown hook in the references; `PM_DEVICE_ACTION_TURN_OFF` at system off requires `CONFIG_PM_DEVICE` | **open question** |

### 5.2 The synthesized teardown body (whenever a slot exists)

Whatever slot carries it, the body mirrors `emit_teardown_dispatch`
(body_emit.rs:5391-5406): the `present` dispatch, the callback marker ALWAYS
(CR-3, device-lifecycle-surface-convergence.md:200-204), the author body iff
overridden (its Result discarded, CR-4 :205-206), then resource postambles in
**REVERSE declaration order** (`:5403-5406`, device_lifecycle.rs:178-187).

The reverse order is reference-grounded even without a remove fn: pl011
bring-up *ends* with `pl011_enable(dev)` (uart_pl011.c:657-659), so quiesce
*begins* with its inverse `pl011_disable(dev)` (uart_pl011.c:124-127) — and
the init fn itself demonstrates the discipline internally (disable → configure
→ re-enable, uart_pl011.c:616-617, :634-636, :658). The Zephyr-specific unwind
inventory, reverse of the probe table:

1. quiesce HW / mask interrupts-as-state (`uart->imsc = 0` analog of init's
   uart_pl011.c:639-640) — reverses on_init's enable;
2. **no unmap**: neither `DEVICE_MMIO_MAP` nor `device_map` has an unmap
   counterpart in the vendored idiom — `map_region`'s postamble mirror is an
   explicit no-op callout, not an invented `k_mem_unmap`;
3. **no region release**: `claim_regions` vanished at bring-up (no Zephyr
   runtime, bus-resource-management-surface-convergence.md:133-135), so its
   reverse vanishes symmetrically;
4. **no disable_device** on platform (default-on); on PCIe the inverse of
   `pcie_set_cmd(…, true)` (eth_e1000.c:220-221) would be
   `pcie_set_cmd(…, false)` — unwitnessed in any vendored driver, flagged not
   asserted.

So on Zephyr the orderly teardown reduces almost entirely to **HW quiesce +
the authored `on_deinit` body**: the OS-resource unwind that dominates the
Linux `.remove` (iounmap → release_regions → disable,
linux-integration-probe-path.md:104-111) has no Zephyr counterpart because
nothing was claimed and mappings are permanent.

**Orderly vs surprise = the `present` bit** (body_emit.rs:5398, :5403):
`present=1` runs HW-touching postambles; `present=0` emits them
`skipped(surprise)`. Since Zephyr's only HW-touching postamble content is the
quiesce writes, a surprise path — if representable at all — would skip exactly
those MMIO writes and run only the authored software-state cleanup in
`on_surprise_remove`.

### 5.3 Init-fail rollback (the init fn's error path)

generic_c captures `on_init`'s `Result`, reads the niche discriminant, and on
Err emits `LC on_init -> Err` → the always-on `on_init_failure` marker → the
author failure body iff overridden (`emit_init_fail_body`,
body_emit.rs:5409-5483; clean-exit semantics, trace-asserted not
exit-code-asserted, GRILL D12 per
device-lifecycle-surface-convergence.md:218-227). On Zephyr this renders as
the init fn's early-return ladder, and the rollback obligation is
**structurally minimal**:

- the reference early-returns with no unwind at every native failure
  (uart_pl011.c:586-588, :607-609; eth_e1000.c:215-217) — correct, because
  nothing acquired needs release: allocation is static, there is no claim, and
  the mapping is permanent;
- the only true high-water-mark rollback is **side-resource state** (clocks
  enabled at uart_pl011.c:593 would ideally be gated off) — the same
  reverse-partial-postamble semantics generic_c defers as-built
  (`emit_init_fail_body` does not yet thread `resource_names`,
  device-lifecycle-surface-convergence.md:222-227), so Zephyr defers it
  symmetrically;
- after the error return the device is permanently not-ready
  (`device_is_ready() == false`); there is no retry/deferral mechanism.

The **authored-vs-native channel split survives teardown**: an `on_init`
authored `Err(RegseqError::…)` exits the init fn as the mapped negative errno
(errno.rs:35-48, negated per the Zephyr return form), while a native preamble
failure returns its own status untouched — the two never merge in the
error-return ladder.

**`on_system_shutdown`** remains the lone omit-when-absent callback
(body_emit.rs:5355-5366: no marker, no slot when not overridden, D10/I02) — on
Zephyr this means a driver that does not override it contributes **no PM hook
at all** (no `PM_DEVICE_DT_INST_DEFINE` line emitted), mirroring how the
reference makes PM conditional per-SoC (`PM_INST_DEFINE(n)` empty by default,
uart_pl011.c:106-117).

## 6. The seven load-bearing composition rules, Zephyr-flavored

(siblings of linux-integration-probe-path.md §8)

1. **Bring-up ordering is inter-construct and load-bearing, but Zephyr
   re-partitions it across binding times.** BRM's managed ops stay synthesized
   INSIDE device-lifecycle's preamble between associate and on_init, in
   canonical order enable_device → claim_regions → set_bus_master →
   get_resource_handle, then register-access's map_region
   (body_emit.rs:5219-5285;
   bus-resource-management-surface-convergence.md:121-124). Zephyr hoists
   register+match+create+associate to build time (DEVICE_DT_INST_DEFINE,
   uart_pl011.c:789-793) and, on the platform path, hoists
   get_resource_handle to build time too (DEVICE_MMIO_ROM_INIT,
   uart_pl011.c:759). Hoisting preserves relative order — a step never
   executes after one that follows it in the canonical sequence — but a spec
   reader must check ordering across binding times, not within one function.

2. **register-access consumes the BRM-resolved (base,size) — I04 holds on
   both Zephyr paths.** DEVICE_MMIO_MAP consumes exactly what
   DEVICE_MMIO_ROM_INIT captured (uart_pl011.c:577, :759); device_map consumes
   exactly what the BAR probe resolved (reference pcie_probe_mbar,
   eth_e1000.c:219-224; generated pcie_get_mbar, per-construct §B.3.4). The reference
   itself documents the order constraint ("Must be placed after
   DEVICE_MMIO_MAP", uart_pl011.c:579). Downstream MMIO (DEVICE_MMIO_GET,
   uart_pl011_registers.h:40-43; sys_read32/sys_write32,
   eth_e1000_priv.h:118-125) is unchanged from the model's view — offsets
   still come from #[repr(C)] recovery (body_emit.rs:2439-2520).

3. **Two error channels, never merged.** Authored RegseqError from on_init →
   errno table (errno.rs:35-48) → negated return from the init fn (return
   form matches uart_pl011.c:173, eth_e1000.c:216). Native status from
   preamble steps (pcie BDF guard, reset, pinctrl) propagates directly. Zephyr
   sharpens the stakes: there is no EPROBE_DEFER to break, but a wrapped
   native error would still erase -ENODEV semantics and the device-not-ready
   contract.

4. **claim_regions vanishes on Zephyr — an explicit elision, not silent
   truncation.** The model keeps the every-bus claim op
   (body_emit.rs:5260-5266); the Zephyr shell emits nothing for it because no
   reservation runtime exists (resolves the backend-[Open] at
   bus-resource-management-surface-convergence.md:133-135). Symmetrically its
   teardown reverse vanishes. Any Zephyr traceability matrix must carry this
   as 'lowered-to-nothing (called out)'.

5. **enable_device + set_bus_master fuse into one pcie_set_cmd**
   (eth_e1000.c:220-221) — the Zephyr analog of Linux devm's claim∘map fusion:
   semantic order preserved inside one call. The dma_present gate
   (device_lifecycle.rs:62-70, dormant) governs whether the MASTER bit is
   included, not whether a separate call exists. (The BRM section §B.3.1
   ratifies the emitted form as separable calls, with single-call fusion a
   permitted peephole.)

6. **The `const struct device *` is the physical join point** — Policy-P's
   void* __rseq_dev (body_emit.rs:5186) realized. It is statically created by
   registration (construct: device-lifecycle), carries data/config (the
   morph: spec driver embeds in dev->data, recovered uart_pl011.c:572-573 —
   note Zephyr splits the container into ROM config + RAM data, a refinement
   of the single-container morph at body_emit.rs:5174-5189), anchors the MMIO
   map (DEVICE_MMIO_MAP(dev), construct: BRM/register-access), and is the
   argument every callback receives. Unlike Linux's dev_*, Zephyr logging
   does NOT thread through it — LOG_* binds to the module
   (LOG_MODULE_REGISTER, eth_e1000.c:9-12), with the module name supplied by
   DriverDescriptor.name (device_lifecycle.rs:47-52): a cross-construct
   dependency from diagnostics-logging onto device-lifecycle that has no
   generic_c analog (generic_c's stdio prelude is fixed, prelude.rs:34).

7. **Scenario selection is harness-side, and Zephyr collapses it.**
   generic_c's four scenario helpers (backend_options.rs:25-32) become:
   orderly-success and init-fail = the two exits of the one init fn;
   orderly-teardown/surprise/shutdown = version-sensitive PM/deinit slots
   (open). The trace contract's per-scenario subsets
   (device-lifecycle-surface-convergence.md:193-198) remain the behavioral
   spec for whichever slots exist.

## 7. Cross-OS coherence with the Linux spec

Identical canonical order, identical two-channel rule, identical
reverse-teardown semantics; the deltas are:

- (a) registration/match/create/associate at **build time** vs module-load;
- (b) claim **vanishes** vs explicit-or-devm-fused;
- (c) **no remove path** vs `.remove`;
- (d) **module-bound logging** vs dev-bound logging;
- (e) **no probe deferral** vs `-EPROBE_DEFER`.
