# Linux integration — the end-to-end probe/teardown path

The composition is the thing being specified: the five constructs **do not lower
in isolation**, and the Linux shape preserves `generic_c`'s ordering exactly
except for the one structural re-partition forced by the kernel's two-phase driver
model. This document threads the five constructs into one driver shape — the
ordered `.probe` bring-up and the reverse `.remove` teardown — and states the
cross-construct invariants that a per-construct spec must never break.

**Target pin:** Linux 6.6+. Mirrors `generic_c`'s bring-up ordering
(`body_emit.rs:5197-5311`) re-rendered into Linux idiom. `generic_c` is a *host
model*; the Linux call targets are proposed materializations to confirm against
the pinned kernel docs — never asserted as done.

---

## 1. Structural seam: register-once vs probe-per-device

`generic_c` emits the whole bring-up sequence (`register → create → associate →
preamble → BRM acquire → on_init`) inside **one scenario function**
(`body_emit.rs:5191-5373`). Linux **re-partitions** the same ordered steps across
two kernel entry points:

- **`register_driver` runs once at module load** — installs
  `.probe`/`.remove`/`.shutdown` pointers + the match table, then returns.
  `regseq_lc_register` + bus selector (`body_emit.rs:5197-5205`) →
  `pci_register_driver(&drv)` (reference-drivers/e1000/linux/e1000_main.c:224) / `platform_driver_register` /
  `amba_driver_register` (reference-drivers/pl011/linux/amba-pl011.c:3205).
- **Everything from `create` onward is the body of `.probe(dev)`**, invoked **once
  per matched device**. The four lifecycle *scenarios* are not four functions on
  Linux — they are `.probe` success (orderly), `.probe` error path (init-fail),
  `.remove` (orderly/surprise), and `.shutdown` (system shutdown).

Ordering **within** each phase is preserved exactly; only `register_driver` is
hoisted out of the per-device body.

---

## 2. Ordered steps inside `.probe` (re-renders `body_emit.rs:5206-5311`)

Steps marked **(native)** can fail with a kernel status that propagates
**directly**, never wrapped in `RegseqError`.

| # | `generic_c` marker | Linux PCI | Linux platform / AMBA |
|---|---|---|---|
| 1 | `regseq_lc_create()` (`:5206`) + Policy-P morph (`:5184-5189`) | alloc private data + `alloc_etherdev` (reference-drivers/e1000/linux/e1000_main.c:952) | `devm_kzalloc` (reference-drivers/pl011/linux/amba-pl011.c:2982) |
| 2 | `regseq_lc_associate()` (`:5207`) | `pci_set_drvdata(pdev, netdev)` (reference-drivers/e1000/linux/e1000_main.c:958) | `amba_set_drvdata(dev, uap)` (reference-drivers/pl011/linux/amba-pl011.c:3024) |
| 3 | `regseq_lc_preamble(res)` forward (`:5214-5217`) | acquire in declaration order (real acquire **backend-[Open]**) | same |
| 4a | `regseq_bus_enable_device()` PCI-only (`:5257-5258`) | **(native)** `pci_enable_device(pdev)` (reference-drivers/e1000/linux/e1000_main.c:934); `if(err) return err` (`:939-940`) | n/a — default-on (**not-applicable**) |
| 4b | `regseq_bus_claim_regions(bar)` every-bus (`:5263-5265`) | **(native)** `pci_request_selected_regions(...)`; `goto err_pci_reg` (reference-drivers/e1000/linux/e1000_main.c:942-944) | **fused** into 4e via `devm_ioremap_resource`; no separate claim |
| 4c | `regseq_bus_set_master()` PCI&&dma (`:5268-5269`) | `pci_set_master(pdev)` (reference-drivers/e1000/linux/e1000_main.c:946) — **dormant** (`dma_present` defaults false) | n/a |
| 4d | `regseq_bus_get_handle()` unconditional, seam anchor (`:5272`) | **(native)** `pci_resource_start(pdev,bar)`/`pci_resource_len` (reference-drivers/e1000/linux/e1000_main.c:979,976) | `platform_get_resource(pdev,IORESOURCE_MEM,0)` (reference-drivers/pl011/linux/amba-pl011.c:3123); AMBA `&dev->res` |
| 4e | `regseq_bus_map_region(base,size,bar)` keystone, **after** 4d (`:5277-5283`) | **(native)** `pci_iomap`/`pci_ioremap_bar(pdev,BAR_0)` (reference-drivers/e1000/linux/e1000_main.c:970)/`pcim_iomap_regions` | **(native)** `devm_ioremap_resource(dev,res)` (reference-drivers/pl011/linux/amba-pl011.c:2914; `IS_ERR`→`PTR_ERR` 2915-2916)/`devm_platform_ioremap_resource` — fuses claim∘map |
| 5 | register-access intrinsics (`emit_mmio` `:2283-2323`) now valid | `readl/writel` (reference-drivers/e1000/linux/e1000_main.c:2010) or `ioread32/iowrite32` over `membase+off` | `readl/writel` over `port.membase` (reference-drivers/pl011/linux/amba-pl011.c:2690,2692) |
| 6 | `regseq_lc_call("on_init")` always (`:5311`) + author body iff overridden (`:5318-5319`) | device init after MMIO live (e1000 hw init `:986`+) | hw config after `membase` set |
| 7 | diag logs (`emit_diag_log` `:2722`) interleaved | `dev_dbg/info/warn/err` (reference-drivers/e1000/linux/e1000_main.c:2953; reference-drivers/pl011/linux/amba-pl011.c:449,468,927,2948) | same |

---

## 3. The keystone (I04)

Step 4e `map_region` **consumes** the `(base,size)` 4d resolved — never an
independent address. The BRM block is gated on a resolved binding
(`body_emit.rs:5250`) and `map_region` is emitted strictly **after**
`get_resource_handle`. Swapping the host `(base,size)` for real
`pci_resource_start`/`DT_REG_ADDR` changes **no** register-access op
(`prelude.rs:160-162`) — that decoupling makes steps 5–7 OS-portable. Reordering
4d/4e is wrong even if each call looks right alone.

---

## 4. Two error channels at probe (hard boundary)

1. **Authored `RegseqError`** from `on_init` → exit-adapter (`emit_exit_adapter`
   `body_emit.rs:4732-4828`): niche read (`:4654-4707`), `return 0` on `Ok`
   (`:4819-4820`), else `switch` → `return <SYMBOL>` per the nine-row table
   (`errno.rs:35-48`). **Errno symbols are identical** on Linux
   (`EINVAL`/`ENOMEM`/`EIO`/…); only the **return form is shell** — `generic_c`
   emits `return EINVAL;` (exit), a Linux `.probe` returns the **negated**
   `return -EINVAL;`. Absent variant fails loud (`:4807`), never silent `EIO`.
2. **Native kernel status** from compiler-generated preamble calls (4a/4b/4d/4e)
   propagates **directly** — `return err` (reference-drivers/e1000/linux/e1000_main.c:940), `goto err_pci_reg`
   (`:944`), `return PTR_ERR(base)` (reference-drivers/pl011/linux/amba-pl011.c:2916). **Never** wrapped in
   `RegseqError` (would break `-EPROBE_DEFER`/`-ENODEV`). The split is **authored
   vs compiler-generated code**, not syntactic.

---

## 5. Teardown / remove path — the strict reverse of bring-up

`generic_c` routes teardown through `emit_teardown_dispatch`
(`body_emit.rs:5391-5407`): `regseq_lc_teardown(present)` (`:5398`), the teardown
callback marker, the author body iff overridden (`:5401`), then resource
postambles in **REVERSE declaration order** (`:5404-5406`) with a `skipped` flag
when `present==0`. The `present` bit is the scenario selector.

| scenario (`body_emit.rs`) | `present` | Linux entry | Linux idiom |
|---|---|---|---|
| **orderly** (`:5317-5322`) | 1 | `.remove(dev)` (reference-drivers/e1000/linux/e1000_main.c:184, reference-drivers/pl011/linux/amba-pl011.c:3196) | reverse-release, HW reachable |
| **surprise** (`:5328-5339`) | 0 | `.remove(dev)` on hot-unplug | HW gone → MMIO-touching releases **skipped** |
| **shutdown** (`:5360-5368`) | n/a | `.shutdown(dev)` (`pci_driver.shutdown`) | quiesce only, no full teardown |
| **init-fail** (`:5345-5354` → `emit_init_fail_body` `:5421-5483`) | partial | `.probe` error path | reverse-unwind to high-water mark (§6) |

**Reverse-order postamble = exact mirror of the probe BRM acquire.** e1000's
`.remove` (reference-drivers/e1000/linux/e1000_main.c:1252) unwinds in the reverse of the probe order:
`iounmap(hw->hw_addr)` (`:1275`, reverse of 4e `map_region`) →
`pci_release_selected_regions(pdev,bars)` (`:1278`, reverse of 4b `claim_regions`)
→ `pci_disable_device(pdev)` (`:1284`, reverse of 4a `enable_device`). This **is**
`generic_c`'s reverse postamble loop (`:5404-5406`) realized in PCI calls. For
`devm_`-managed resources (AMBA/platform), release is **automatic** in reverse
acquisition order — `pl011_remove` (reference-drivers/pl011/linux/amba-pl011.c:3029) writes no manual unmap.

**Orderly vs surprise = the `present` bit.** Orderly (`present=1`): device present
→ HW-touching postambles **run**. Surprise (`present=0`): device gone → MMIO-touching
postambles are **`skipped(surprise)`** (`body_emit.rs:5403`); software-only release
(free memory, unregister) still runs.

---

## 6. Init-fail rollback (`.probe` error path)

`generic_c` captures `on_init`'s `Result`, and on the **Err** niche
(`if (__disc != Ok)`, `:5475`) emits `regseq_lc_on_init_err()`, the `on_init_failure`
marker (`:5477`), and the author failure body (`:5478-5480`). Linux renders this as
`.probe`'s **`goto`-unwind ladder** — reverse-release everything acquired up to the
high-water mark (e1000's `err_pci_reg`/`err_ioremap` labels reachable from `:944`+)
before `return -<errno>`. For `devm_`-managed resources the unwind is **automatic**:
returning non-zero from `.probe` triggers the devres stack to release in reverse.
This is the `generic_c` teardown loop scoped to what actually ran. Note the
ladder's **native-unwind entries** (a §2 *(native)* preamble step fails before
`on_init` ever runs) share these release blocks but **never** run
`on_init_failure` or the errno adapter — the hook fires only on the authored
`on_init`→Err niche above (scaffold §3; the §4 channel split realized in
control flow).

**`on_system_shutdown`** is the only omit-when-absent callback
(`body_emit.rs:5360-5368`: no marker/slot when absent) → the Linux `.shutdown`
hook: quiesce, **no** resource teardown (system is going down).

---

## 7. `poll_until` (composition note)

**recognized-not-lowered** in `generic_c` (`domain_attr.rs:95`; no emission). The
Linux target would be `readl_poll_timeout()` (`<linux/iopoll.h>`), but since the
reference emits nothing this is **needs-new-surface**, not a confirmed lowering;
its timeout→`RegseqError::Timeout` edge is also unlowered. See
[`linux-open-questions.md`](./linux-open-questions.md).

---

## 8. The six cross-construct invariants (the load-bearing composition rules)

1. **Bring-up ordering is inter-construct and load-bearing.** BRM's four managed
   ops are synthesized *inside* device-lifecycle's preamble
   (`body_emit.rs:5219-5285`), between `associate` and `on_init`, in the canonical
   order `enable_device → claim_regions → set_bus_master → get_resource_handle`,
   then register-access's `map_region`. Linux keeps this order within `.probe`; the
   only restructuring is hoisting `register_driver` to module-init. A spec that
   lowers any primitive standalone, or reorders the preamble, is wrong even if each
   call is individually correct.

2. **register-access consumes the BRM-resolved `(base,size)` — invariant I04.**
   `map_region` (step 4e) is fed `get_resource_handle`'s output (step 4d) and
   nothing else; the MMIO accessors (`readl`/`writel`) address `mapped_base +
   offset`. The `(base,size)` source is the *only* thing that differs between host
   model and real Linux (`prelude.rs:160-162`) — `pci_resource_start`+`pci_iomap`
   (PCI) vs `platform_get_resource`+`devm_ioremap_resource` (platform/AMBA).
   Everything downstream is unchanged.

3. **Two error channels, never merged.** Authored `RegseqError` from `on_init` →
   exit-adapter (`body_emit.rs:4732-4828`, `errno.rs:35-48`) → negated `-E<errno>`
   from `.probe`. Native preamble status (`pci_enable_device`,
   `pci_request_regions`, `devm_ioremap_resource`) propagates *directly*
   (`return err`/`goto`/`PTR_ERR`) and is never round-tripped — wrapping
   `-EPROBE_DEFER`/`-ENODEV` would break probe deferral and erase platform
   semantics. The boundary is authored-vs-compiler-generated code, not syntax.
   `generic_c` (host model, no failing native calls) materializes only the authored
   adapter today; that is not license to collapse native failures.

4. **Teardown is the strict reverse of bring-up.** The postamble loop
   (`body_emit.rs:5404-5406`) reverses acquire order; e1000's `.remove` proves it:
   `iounmap` (`:1275`) → `pci_release_selected_regions` (`:1278`) →
   `pci_disable_device` (`:1284`), exactly reversing probe steps 4e→4b→4a. The
   `present` bit selects whether HW-touching releases run (orderly) or are skipped
   (surprise). `devm_` resources reverse automatically.

5. **The `struct device *` is the physical join point.** The Policy-P morph's
   `void *__rseq_dev` (`body_emit.rs:5186`) carries the Linux device identity —
   intended as the `struct device *`, though which concrete bus struct
   (`pci_dev`/`platform_device`/`amba_device`) backs the `void *` and the exact
   `*_get_drvdata`/`container_of` recovery remain **open question C-4**
   (`linux-open-questions.md`; unexercised by `generic_c`). Whatever the
   recovery, it is the `dev` argument to `dev_*` logging
   (diagnostics-logging → level→macro, reassemble one `printk` fmt, literal `%`→`%%`,
   `&str` via `%.*s` with interior-NUL gap), the handle the BRM preamble ops take
   (`pci_enable_device(pdev)`), and the anchor `devm_*` lifetimes bind to.
   Diagnostics-logging, BRM, and device-lifecycle all thread through this one
   pointer.

6. **AMBA/platform `devm_` fuses claim∘map** (one `devm_ioremap_resource` covers
   steps 4b+4e) but preserves the *semantic* order — reserve-then-map still happens,
   release-on-unwind is still reverse. The model's every-bus `claim_regions`
   collapses to a no-op marker on the devm path; this must be called out, not
   silently dropped (silent truncation reads as coverage).
