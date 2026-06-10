# Linux lowering — the five constructs in depth

This is the body of the Linux backend spec: the normative per-construct lowering
for **Linux mainline LTS 6.6+**. The five sections are independent but compose —
read [`linux-integration-probe-path.md`](./linux-integration-probe-path.md) for
how they thread together. Each section's **trace rows** (model surface ·
`generic_c` ref · Linux lowering · scaffold · error · open decision) are
consolidated in [`linux-traceability-matrix.md`](./linux-traceability-matrix.md)
to avoid duplication.

Citation legend: repo cites are `path:line` (crate sources by file name,
vendored stock drivers by repo-relative
`reference-drivers/{pl011,e1000}/linux/<file>:line` path); Linux cites pin
mainline LTS 6.6+ as full pinned URLs —
`https://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git/plain/<path>?h=v6.6`
for kernel source, `https://docs.kernel.org/<page>.html` for documentation.
These are pinned canonical locations, not
fetched anchors (see the README's citation-discipline rules). Where an exact
accessor URL anchor could not be live-verified in this session, it is flagged in
open decisions rather than asserted.

---

## A. Register-access — Linux backend lowering

**Scope.** This lowers the *register-access* construct group exactly: the five
MMIO ops `mmio_read` / `mmio_read_field` / `mmio_write` / `mmio_write_field` /
`mmio_modify` (`MmioOp::{Read,ReadField,Write,WriteField,Modify}`,
`domain_attr.rs:125-136`), the `poll_until` intrinsic (`IntrinsicKind::PollUntil`,
`domain_attr.rs:95`), and the `map_region` keystone seam that binds the register
block to a BRM-resolved `(base,size)` (`device_lifecycle.rs:209-238`). The bus
preamble ops it composes with (`enable_device`/`claim_regions`/`set_bus_master`/
`get_resource_handle`) are the *bus-resource-management* construct and are
described here only as the seam context that produces `base`.

### A.1 The invariant that makes the whole construct cheap on Linux

In the `generic_c` reference, every MMIO op lowers to **one accessor call against
`base + offset`**, and the only thing the bus keystone changes is *which* `base`
(`body_emit.rs:2271-2281`, the `model_has_bus_binding` switch between the
faked-base `regseq_mmio_read/write` `prelude.rs:87,96` and the BRM-based
`regseq_bus_mmio_read/write` `prelude.rs:233,243`). The **op algebra is
backend-invariant** — offset recovery (Strategy C, `recover_register`
`body_emit.rs:2439-2529`), the descriptor `mask`/`shift`/`value` reads, the
masked-write confinement, and the read-clear-OR-writeback RMW are all computed
before the accessor is named.

Therefore the Linux lowering differs from `generic_c` in **exactly two places**:

1. **the accessor primitive** — `regseq_mmio_read/write(off,width,…)` (one
   width-parametric host helper) becomes a statically-selected Linux accessor
   (`readl`/`writel`/`ioread32`/`iowrite32`/…), picked at emit time from the
   literal `width` that `recover_register` already returns
   (`body_emit.rs:2515-2520`, width ∈ {1,2,4,8}); and
2. **the provenance of `base`** — the host's global `regseq_regfile` /
   `regseq_bus_mapped_base` (`prelude.rs:86,182`) becomes a real
   `void __iomem *` produced by `map_region` (§A.5).

Everything between those two — the mask/shift arithmetic, the RMW shape, the
offset expression — is emitted identically. On Linux there is **no faked path**:
`base` is *always* the mapped iomem cookie, so the `generic_c` faked/based switch
collapses to the based form. The I04 guarantee ("the based vs faked choice
changes no register-access op", `device_lifecycle.rs:206-208`) holds trivially.

### A.2 Width → accessor selection

`recover_register` returns the access width in bytes from the wrapper field's
layout size (`body_emit.rs:2514-2520`); the block's `access_width`/`endian` are L1
metadata (`convergence §2.2`, inert). Linux has no width-parametric accessor, so
the backend selects statically:

| width | `generic_c` (`prelude.rs:87/96`, `233/243`) | Linux MMIO (readl-family) | Linux port-safe (ioread-family) |
|---|---|---|---|
| 1 | `regseq_mmio_{read,write}(off,1,…)` | `readb` / `writeb` | `ioread8` / `iowrite8` |
| 2 | `…(off,2,…)` | `readw` / `writew` | `ioread16` / `iowrite16` |
| 4 | `…(off,4,…)` | `readl` / `writel` | `ioread32` / `iowrite32` |
| 8 | `…(off,8,…)` | `readq` / `writeq` († `CONFIG_64BIT` only) | `ioread64` / `iowrite64` († `CONFIG_64BIT` only) |

Source: <https://docs.kernel.org/driver-api/device-io.html>,
<https://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git/plain/include/asm-generic/io.h?h=v6.6>; in-repo
witnesses: the E1000 accessor macros `readl(hw->hw_addr + offset)` /
`writel(value, hw->hw_addr + offset)` (`reference-drivers/e1000/linux/e1000_osdep.h:29-42`) and PL011's
width-switched `pl011_read`/`pl011_write`, which select the l/w accessor by the
port's `iotype` (reference-drivers/pl011/linux/amba-pl011.c:319-334). Two contracts to honor:

- **Argument order flips for writes.** `generic_c` is `write_fn(off, width, val)`;
  Linux is **`writel(value, addr)`** — value first, address second
  (<https://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git/plain/include/asm-generic/io.h?h=v6.6>);
  in-repo `reference-drivers/e1000/linux/e1000_osdep.h:33,37`. The backend must
  reorder.
- **`readl`/`writel` are little-endian + fully ordered** (compiler + CPU barriers)
  (<https://docs.kernel.org/driver-api/device-io.html>). This matches the block default `endian =
  "little"` (`convergence §2.2`) and the construct's volatile-MMIO contract. The
  construct does **not** model ordering/fences (out of scope — `convergence §1`:
  "ordering remain OUT", `ordered!` deferred), so the conservative fully-ordered
  accessor is the correct default; `readl_relaxed` is not selected. Note the
  in-tree PL011 deliberately uses `readl_relaxed`/`writel_relaxed`
  (reference-drivers/pl011/linux/amba-pl011.c:325,334) — a performance choice the construct has no surface to
  express; selecting `_relaxed` would be an unrequested weakening.
- **† Width-8 is `CONFIG_64BIT`-gated — not portable.** v6.6
  `asm-generic/io.h` defines `readq`/`writeq` (and `ioread64`/`iowrite64`)
  only under `CONFIG_64BIT`
  (<https://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git/plain/include/asm-generic/io.h?h=v6.6>);
  on architectures without native 64-bit MMIO the kernel offers them only via
  `<linux/io-64-nonatomic-lo-hi.h>` / `<linux/io-64-nonatomic-hi-lo.h>`
  (<https://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git/plain/include/linux/io-64-nonatomic-lo-hi.h?h=v6.6>,
  <https://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git/plain/include/linux/io-64-nonatomic-hi-lo.h?h=v6.6>),
  which split the access into two 32-bit ops whose order is device-register
  semantics the model has no surface to express. **Interim normative rule:
  width-8 lowers to `readq`/`writeq` only for `CONFIG_64BIT` targets, and the
  backend emits a fail-loud guard (`#ifndef CONFIG_64BIT` / `#error`) into any
  TU containing a width-8 MMIO op** — never a silent non-atomic split. The
  non-64-bit policy (a global `io-64-nonatomic-*` include choice, or a model
  split-order surface) is open question **A-12**.

`readb/w/l/q` operate on a `void __iomem *`; a register is reached as `base + off`
where `off` is the literal byte offset (or, for register arrays,
`base + (i*stride)`) that `recover_register` already produced
(`body_emit.rs:2521-2528`).

### A.3 The five MMIO ops — per-op lowering

Below, `base` is the mapped `void __iomem *` (§A.5), `off` is the recovered
literal offset, and `M`/`S`/`V` are the field `mask`/`shift`/`value` read from the
`Field`/`FieldValue` descriptor const argument — the *same* `(desc).mask`/`.shift`
member reads `generic_c` uses (`body_emit.rs:2305-2321`), const-folded to
literals. The width-4 (`readl`/`writel`) forms are shown; widths 1/2/8 substitute
the §A.2 accessor.

- **`mmio_read`** (`MmioOp::Read`, gc `body_emit.rs:2297-2302`): whole-register
  raw read. `dst = (T)readl(base + off);`
- **`mmio_read_field`** (`MmioOp::ReadField`, gc `:2303-2311`): masked + shifted
  field read. `dst = (T)((readl(base + off) & M) >> S);`
- **`mmio_write`** (`MmioOp::Write`, gc `:2283-2287`): whole-register raw write.
  `writel((u32)val, base + off);`
- **`mmio_write_field`** (`MmioOp::WriteField`, gc `:2288-2296`): masked
  whole-register write — **not** RMW; bits outside the field are written 0
  (`convergence §2.4`: "bits outside the mask = 0"). `writel((u32)(V & M), base +
  off);`
- **`mmio_modify`** (`MmioOp::Modify`, gc `:2312-2323`): field RMW, preserving
  unmentioned bits. `{ u32 cur = readl(base + off); writel((cur & ~(u32)M) | (u32)(V
  & M), base + off); }`

These are byte-for-byte the `generic_c` arms with `read_fn`/`write_fn` →
`readl`/`writel` and the offset rebased onto the iomem cookie. The RMW idiom is
the kernel's own — PL011's `uap->im &= ~…; pl011_write(uap->im, uap, REG_IMSC)`
(reference-drivers/pl011/linux/amba-pl011.c:730-731) is a hand-written instance of the same
read/clear/write shape.

**`side_effects` / `hw_updates` metadata is inert** (`convergence §2.2`).
`generic_c` does not specialize the accessor on `write_one_to_clear` /
`read_clear` / `hw_updates`; Linux is identical: W1C is just `writel(field_mask,…)`
and read-clear is just `readl(…)` — the clearing is the *device's* behavior, not
the accessor's. A `hw_updates` register (e.g. PL011 `fr`) is re-read every access
because `readl` never caches (cf. the in-tree busy-wait re-reading `REG_FR`,
reference-drivers/pl011/linux/amba-pl011.c:352).

### A.4 `poll_until` — recognized, **not lowered** (no reference lowering)

`poll_until` is recognized in the model (`IntrinsicKind::PollUntil`,
`domain_attr.rs:95,172`; pretty-printed `pretty.rs:1913`; bridge recognition
`crates/regseq-driver/src/bridge/intrinsic/recognition.rs:90,117`) but
**`generic_c` emits no lowering for it**: `call()` intercepts only `Log`/`Mmio`/`Config`
(`body_emit.rs:2100-2125`), and a backend-wide search for `PollUntil` returns zero
matches. The convergence doc states this explicitly (`§9`).

Per the no-invent rule, **no normative Linux lowering is asserted.** The idiomatic
Linux target is `readl_poll_timeout(addr, val, cond, delay_us, timeout_us)`
(<https://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git/plain/include/linux/iopoll.h?h=v6.6>),
which returns `0` / `-ETIMEDOUT` and would feed the
`RegseqError::Timeout → ETIMEDOUT` mapping owned by the *RegseqError* construct
(`errno.rs:40`). But adopting it requires resolving how the side-effect-free read
predicate is translated into the macro's single-`readl`-into-`val` `cond`, and
what units the DSL `DELAY`/`TIMEOUT` carry (the macro is microseconds). These are
unresolved; the construct ships register-access on Linux with `poll_until`
*recognized-not-lowered*, matching `generic_c`.

### A.5 `map_region` — the keystone seam (where `base` is born)

`map_region` consumes the BRM-resolved binding (`MmioBinding.base`/`size`/
`effective_bar`, `device_lifecycle.rs:223-228`; gated by `is_resolved()` = both
base and size present, `:236-238`) and produces the iomem cookie every MMIO op
rebases on. In `generic_c` it is emitted **inside lifecycle synthesis, after
`get_resource_handle`**, in the canonical preamble order
`enable_device → claim_regions → set_bus_master → get_resource_handle →
map_region` (`body_emit.rs:5250-5285`). The Linux lowering must preserve that
ordering and bus-gating.

**The preamble gate is host-keyed — an explicit tension to resolve, not paper
over.** As-built, the entire preamble block (and therefore `map_region`) is
gated on `is_resolved()` = literal L2 `base && size`
(`device_lifecycle.rs:236-238`; the filter `body_emit.rs:5244-5250`), while on
Linux those literal *values* are discarded in favor of probe-time enumeration
(I04, below). The two facts compose only if Linux **keeps requiring the L2
literals as the trigger** even though their values go unused — otherwise a PCI
binding carrying `effective_bar` but no L2 `base`/`size` would silently skip
the whole BRM/map preamble. **Interim normative rule: the as-built gate stands**
(L2 literal `base`/`size` are still required for the preamble to be emitted on
Linux; their values are then replaced by enumeration). Replacing it with a
Linux-specific `mappable` predicate (keyed by `effective_bar` for PCI / region
index for platform/DT) is open question **A-10** — do not resolve it silently
in either direction.

**PCI path** (`BusType::Pci`, `is_pci_like()` `device_lifecycle.rs:101-104`):
1. `enable_device` → `pci_enable_device(pdev)` — PCI-gated (`body_emit.rs:5257`)
   (<https://docs.kernel.org/PCI/pci.html>).
2. `claim_regions` → `pci_request_regions(pdev, KBUILD_MODNAME)` (or
   `pci_request_selected_regions`) — `prelude.rs:202-208`.
3. `set_bus_master` → `pci_set_master(pdev)` — emitted only when PCI **and**
   `dma_present` (`body_emit.rs:5268`); `dma_present` defaults false
   (`device_lifecycle.rs:62-70`), so absent today.
4. `get_resource_handle` → `pdev` *is* the handle; `pci_resource_start(pdev,
   effective_bar)` / `pci_resource_len(pdev, effective_bar)` supply the real
   `(base,size)` (<https://docs.kernel.org/PCI/pci.html>; in-repo use:
   reference-drivers/e1000/linux/e1000_main.c:1024).
5. `map_region` → `base = pci_iomap(pdev, effective_bar, 0)`
   (<https://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git/plain/lib/pci_iomap.c?h=v6.6>)
   (maxlen 0 = whole BAR), or the BAR-shorthand `pci_ioremap_bar(pdev, BAR_0)`
   the vendored E1000 actually uses (reference-drivers/e1000/linux/e1000_main.c:970).
   `effective_bar = L2.bar ?? L1.bar`
   (`device_lifecycle.rs:221-223`); gc traces it (`body_emit.rs:5280-5283`) so the
   L2-override is observable.

**Platform/DT path** (`BusType::{Platform,Amba}`):
- `enable_device` is **not** emitted (not PCI-like; default-on,
  `prelude.rs:196-197`).
- `claim_regions` and `map_region` **fuse** on Linux: `res =
  platform_get_resource(pdev, IORESOURCE_MEM, 0); base = devm_ioremap_resource(
  &pdev->dev, res);` — or the one-shot `base = devm_platform_ioremap_resource(pdev,
  0)` (<https://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git/plain/drivers/base/platform.c?h=v6.6>).
  The prelude states this fusion verbatim
  (`prelude.rs:202-208`). The model still fires a per-binding `claim_regions`
  marker on every bus (`body_emit.rs:5260-5266`), but on Linux platform that
  separate claim is *subsumed* by `devm_ioremap_resource` — the two `generic_c`
  trace lines collapse into one Linux call. `effective_bar` is `None` here.

**`base` provenance — the I04 swap.** On the `generic_c` host, `base`/`size` are
the `[mmio.<id>]` platform-file literals (`device_lifecycle.rs:224-226`). On real
Linux they come from `pci_resource_start/len` (PCI) or `struct resource` via
`platform_get_resource` (DT) at probe — *swapping the literal source changes no
register-access op* (`device_lifecycle.rs:206-208`).

### A.6 Scaffold placement, error channel, lifetime

- **Scaffold.** The five MMIO ops lower into the body of whatever authored method
  contains them, which become ordinary C functions; the `Mmio<T>` resource field
  (`#[rseq::mmio(id="regs")]`) is realized as a `void __iomem *base` stored in the
  driver's private struct and reached via `pci_get_drvdata`/`platform_get_drvdata`.
  `map_region` (and its preamble) land in **`probe()`** (`regseq_lc_preamble`,
  `body_emit.rs:5214-5217`); the matching release lands in **`remove()`**
  (`regseq_lc_postamble`, reverse order, `:5403-5406`).
- **Error channel — two distinct, do not conflate.** The MMIO accessors are
  **infallible** — they return a value or `void` and cannot fail. A *resource*
  failure at `map_region` (`pci_iomap` → `NULL`; `devm_ioremap_resource` →
  `ERR_PTR`, test `IS_ERR`) is the **native OS channel**: `probe()` returns a
  negative errno. That is *separate* from an authored `RegseqError` (the init-fail
  scenario captures `on_init`'s `Result`, `body_emit.rs:5345-5354`).
  `poll_until`'s `-ETIMEDOUT` is the third, authored channel.
- **Lifetime.** Prefer the **managed** mappers (`devm_ioremap_resource` /
  `devm_platform_ioremap_resource`; PCI `pcim_*`): the unmap is tied to device
  detach, so the lifecycle postamble is satisfied implicitly. With the non-managed
  `pci_iomap`, `remove()` must pair `pci_iounmap` + `pci_release_regions`
  explicitly.
- **Array bounds.** Indexed register access (`regs.mta[i]`) carries Rust's
  array-bounds `Assert` in the caller MIR, lowered by the backend's generic assert
  path (not register-access-specific); the Linux realization (`BUG()`/`WARN`/
  return-error) is a shared backend concern, deferred.

---

## B. Bus-resource-management — Linux backend lowering

**Scope.** The Linux lowering of exactly the BRM primitives — `register_driver`
(the bus-path selector), the four compiler-managed preamble ops, the config-space
intrinsics (`read_config`/`write_config`), and the carrier types (`MmioBinding`,
`BusType`, `BarType::Io`). `map_region` is **out of scope as a primitive** (it is
register-access's), but it is the downstream consumer of this construct's resolved
`(base,size)`, so the composition is specified where it touches BRM.

### B.1 The structural pivot — a linear preamble becomes module-init + `.probe`

In `generic_c` the entire bring-up is one flat scenario function that runs, inline
and in order (`body_emit.rs:5197-5285`):

```
regseq_lc_register(name, match)        // device-lifecycle marker
regseq_bus_register_driver(sel, bound) // BRM selector — immediately after register (:5203-5205)
regseq_lc_create(); regseq_lc_associate();
regseq_lc_preamble(<resource>)...      // forward, per resource field
  regseq_bus_enable_device()           // if is_pci_like          (:5256-5259)
  regseq_bus_claim_regions(bar)...     // per resolved region, EVERY bus (:5260-5266)
  regseq_bus_set_master()              // if is_pci && dma_present (:5267-5270)
  regseq_bus_get_handle()              // unconditional           (:5271-5272)
  regseq_bus_map_region(base,size,bar) // register-access keystone (:5277-5284)
on_init(...)
```

On Linux this single sequence **splits across the driver model** along one seam:
`register_driver` lifts to **module init**, and everything from `enable_device`
through `map_region` and `on_init` becomes the **body of `.probe`**. The *order
within probe* is preserved exactly from `body_emit.rs:5197-5285` (disposition
**shared-procedural-core**). The seam that makes this OS-portable is invariant
**I04**: swapping the `(base,size)` source (host literal ↔ `pci_resource_start`/
`platform_get_resource`) changes **no** register-access op (`prelude.rs:160-162`).

```c
// module init — register_driver lives here, NOT in probe
static struct pci_driver e1000_driver = { .name="e1000", .id_table=e1000_ids, .probe=e1000_probe, .remove=e1000_remove };
module_pci_driver(e1000_driver);                 // selector → pci_register_driver

// probe — the preamble, in generic_c's order
static int e1000_probe(struct pci_dev *pdev, const struct pci_device_id *id) {
    int ret;
    ret = pcim_enable_device(pdev);              // enable_device   (PCI only)
    if (ret) return ret;                         // native errno, direct
    ret = pci_request_selected_regions(pdev, 1<<bar, "e1000"); // claim_regions (PCI)
    if (ret) return ret;
    // pci_set_master(pdev);                      // set_bus_master (dormant — see B.4)
    phys = pci_resource_start(pdev, bar);        // get_resource_handle
    len  = pci_resource_len(pdev, bar);
    regs = pci_iomap(pdev, bar, len);            // map_region (register-access keystone, I04)
    if (!regs) return -ENOMEM;
    // ... on_init(regs)  — authored callback; RegseqError → -errno (§E)
}
```

### B.2 `register_driver` — the bus-path selector

Device-lifecycle **owns the registration marker** (`regseq_lc_register`, carrying
the descriptor name + the rendered typed match set); bus-resource supplies
**only** the bus-path selector token (`body_emit.rs:5129-5154`). The token comes
from `bus_selector_token` over `BusType` (`body_emit.rs:5050-5061`). On Linux the
selector picks the registration struct + call, and the DL `match_set` renders into
that struct's **id table** (disposition **native-materialization**):

| `BusType` | registration | id table |
|---|---|---|
| `Pci` | `pci_register_driver()` / `module_pci_driver()` | `struct pci_device_id[]` ← `MatchEntry::Pci{vendor,device,…}` |
| `Platform` | `platform_driver_register()` / `module_platform_driver()` | `struct of_device_id[]` (`.compatible`) ← `MatchEntry::Devicetree` |
| `Amba` | `amba_driver_register()` / `module_amba_driver()` — **distinct** | `struct amba_id[]` (periphid + mask) ← `MatchEntry::Amba{id,mask}` |
| `I2c`/`Spi`/`Usb` | `i2c_add_driver`/`spi_register_driver`/`usb_register` | per-bus id table — **deferred lowering** |
| `Other(sid)` | — | **needs-new-surface** (rejected at surface validation) |

Two Linux-specific notes. (a) The `bound` argument (`body_emit.rs:5145-5153`) is a
**host-trace artifact with no Linux equivalent** — registration always installs
`.probe` regardless of region resolution, because regions resolve *at probe time*.
**not-applicable** on Linux. (b) `pci_register_driver` (and peers) return an `int`
at **module init**; that failure is **native** (propagate the negative errno from
`module_init`), distinct from a probe failure, and never wrapped in `RegseqError`.

### B.3 The probe preamble — the four managed ops, in order

The gate is unchanged: the whole block is gated on ≥1 resolved binding
(`body_emit.rs:5250`; "resolved" = host-keyed `is_resolved()` — see the §A.5
interim rule and open question A-10), and within it the PCI-only ops are further
gated by `is_pci_like` (`device_lifecycle.rs:101-104`, only `Pci` today).

**B.3.1 `enable_device`.** **PCI**: `pci_enable_device(pdev)` or managed
`pcim_enable_device(pdev)`, the first call in `.probe`. Returns `int` → **native**
errno, `return ret;` directly (gate `body_emit.rs:5256-5259`). **Platform/AMBA**:
**not-applicable** (default-on).

**B.3.2 `claim_regions` — the fusion seam.** The model emits `claim_regions(bar)`
**per resolved region on every bus** (NOT PCI-gated; `body_emit.rs:5260-5266`).
On Linux the realization **diverges by path**, changing the call count:
- **PCI** (stays a distinct step): `pci_request_selected_regions(pdev,
  1<<effective_bar, name)` reserves the BAR; mapping is the *separate* `pci_iomap`.
- **Platform/DT/AMBA** (fused — `claim_regions` has **no independent call**):
  `devm_ioremap_resource(dev, res)` / `devm_platform_ioremap_resource(pdev, idx)`
  **fuses** `request_mem_region` + `ioremap` into one call (`prelude.rs:206-208`).
  The model's `claim → handle → map` triple **collapses to two Linux calls**, and
  the standalone claim is **subsumed into the map**. A Linux backend must
  therefore **not** assume `claim_regions` always emits a call.
- Error: both fail **natively** → propagate directly.

**B.3.3 `set_bus_master`.** Target known, **dormant today**: `pci_set_master(pdev)`
(void). The gate is `is_pci && dma_present`, and `dma_present` is a placeholder
that defaults `false` and is set by no fixture (`device_lifecycle.rs:62-70`) — DMA
is **out of scope**. Disposition: **native-materialization target identified,
not-applicable today**.

**B.3.4 `get_resource_handle` — the seam anchor.** Unconditional when a resolved
binding is present (I06; `body_emit.rs:5271-5272`). This **resolves the region's
physical extent before mapping** (disposition **shared-procedural-core**):
- **PCI**: `pci_resource_start(pdev, effective_bar)` +
  `pci_resource_len(pdev, effective_bar)`.
- **Platform/DT**: `platform_get_resource(pdev, IORESOURCE_MEM, index)` →
  `struct resource *` with `.start`/`.end`. `effective_bar = None` ⇒ index by
  region order.

It **must stay before `map_region`** (I04). Error: `platform_get_resource` may
return `NULL` → **native** (`return -EINVAL`/`-ENODEV`).

**B.3.5 `map_region`** (composition target, register-access — out of scope to
lower). Fed the resolved extent from B.3.4: **PCI** `pci_iomap(pdev,
effective_bar, len)` / `pcim_iomap_regions(...)`; **platform/DT**
`devm_ioremap_resource(...)` (the fused call). No register-access op changes (I04).

### B.4 Config-space intrinsics — `read_config` / `write_config`

`call()` intercepts a Direct callee carrying `IntrinsicKind::Config(op)` and
routes to `emit_config` (`body_emit.rs:2122-2125, 2344-2397`); the access width is
`size_of::<T>()` ∈ {1,2,4,8} (`config_access_width`, `:2404-2414`), the offset is
`args[0]`. PCI-only. On Linux (native-materialization, PCI):

| op | width 1 | width 2 | width 4 | width 8 |
|---|---|---|---|---|
| `read_config::<T>(off)` | `pci_read_config_byte` | `pci_read_config_word` | `pci_read_config_dword` | **no primitive** |
| `write_config::<T>(off,v)` | `pci_write_config_byte` | `pci_write_config_word` | `pci_write_config_dword` | **no primitive** |

Three Linux-specific facts:
1. **Width-8 gap.** PCI config accessors top out at 32-bit dword — there is **no
   8-byte accessor**. Must be split into two dwords, or rejected.
   **needs-new-surface.**
2. **Return-shape marshalling — and the status `int`, on both directions.**
   The model's `read_config::<T>(off) -> T` returns
   the value directly; the Linux read accessor returns a status `int` and writes
   the value through an out-param (v6.6 `pci.h:1210-1212`,
   <https://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git/plain/include/linux/pci.h?h=v6.6>).
   The lowering must call into a temp, then assign the temp to the destination —
   inverting the C calling convention. The write accessors
   `pci_write_config_{byte,word,dword}` **also return a status `int`** (v6.6
   `pci.h:1213-1215`), against the model's infallible
   `write_config::<T>(off,v)`. Each status is a **native** PCIBIOS/errno
   result, never `RegseqError` — but its disposition needs care: the §B.6
   direct-propagation rule covers the *preamble* generated calls, whereas a
   config intrinsic sits mid-body in an **authored** function whose model
   surface is infallible, so there is **no channel to propagate the status
   through without a new surface**. **Interim normative rule: discard
   explicitly, for reads and writes both** — emit
   `(void)pci_read_config_dword(...)` / `(void)pci_write_config_dword(...)` so
   the drop is visible
   in the generated C, not implicit. Whether to instead log, reject, or extend
   the model to propagate is open question **A-11** (which covers both
   directions).
3. **PCI-ID caching is host-only.** The PCI-ID-caching witness
   (`prelude.rs:272-288`) and the `regseq_bus_config_space[256]` model are
   **host-model artifacts** — **not-applicable** on Linux, where vendor/device IDs
   are already in `pdev->vendor`/`pdev->device`. The `PCI_COMMAND` witness does map
   to a real `pci_read_config_word(pdev, PCI_COMMAND, &cmd)`.

**Non-PCI** config access is **not-applicable** (no config space) and is the
**BRM-001** rejection — but BRM-001 is **deferred**. So today a config intrinsic on
a non-PCI driver slips past recognition; the Linux spec depends on that as-yet-
unbuilt diagnostic. **Open question.**

### B.5 Carrier model types

- **`MmioBinding`** (`device_lifecycle.rs:209-238`) — the per-region carrier.
  `effective_bar` → the PCI BAR index (or `None` ⇒ platform/DT region index);
  `bar_type` → Mem (ioremap path) vs Io (§B.6). Its `base`/`size` are the **host's**
  resolved literal; on Linux their *values* are **replaced by enumeration**, but
  their *presence* still gates the preamble (`is_resolved()`, the interim rule of
  §A.5 / open question A-10) — the binding's *role* is preserved while its
  *values* are host-only.
- **`BusType`** (`device_lifecycle.rs:78-104`) — drives both the §B.2 framework
  selection and the `is_pci_like` PCI-only gate (only `Pci` today). The gate maps
  to Linux reality exactly: only PCI has config space, `enable_device`, and
  `set_master`.
- **`BarType::Io`** (`device_lifecycle.rs:243-250`, recognized_not_lowered) —
  **needs-new-surface**. Linux port-I/O is a distinct lowering surface: an
  `IORESOURCE_IO` BAR uses `inb/outb`/… or `ioread*/iowrite*`, not `readl`/`writel`.
  The clean unification (`ioread32`/`iowrite32` over `pci_iomap`) is a **design
  choice not yet made in the model** → escalate as a blocking gap, do not paper
  over.

### B.6 The error channel — native vs authored

Every preamble op in §B.3 is a **compiler-generated** call; its native status
(`-ENODEV`, `-EBUSY`, `-ENOMEM`, **`-EPROBE_DEFER`**, `IS_ERR(regs)`) propagates
**directly** as the `.probe` return and is **never** wrapped in `RegseqError`.
Wrapping `-EPROBE_DEFER` would break the kernel's re-probe deferral. The **only**
thing that crosses the `RegseqError` exit-adapter is the **authored** callback
return (`on_init`). The single Linux change to the adapter is the **return form**:
`return EINVAL;` (exit) becomes `return -EINVAL;` (negative errno from `.probe`).
The boundary is the authored/generated split, not a syntactic one.

---

## C. Device lifecycle — Linux backend lowering

### C.0 The structural shift: `generic_c` synthesizes the OS; Linux *is* driven by it

`generic_c` is a host model. It realizes the whole lifecycle by **synthesizing a
driving `main`** — `emit_lifecycle_synthesis` emits one
`static void __rseq_lc_<idx>_<label>_<scenario>(void)` helper per driver that runs
the entire canonical sequence inline, and a runner `__rseq_lc_run` that an
`EntryShim` `main` calls (`body_emit.rs:5088`, `:5191-5193`, `:5376-5380`; shim
`mod.rs:439-440`). Each managed step prints an `LC <event>` line — that trace
**is** the behavioral oracle (`device-lifecycle-surface-convergence.md:25-33`,
host hooks `prelude.rs:115-148`).

On Linux **none of that scaffolding exists**: the kernel's bus core is the OS. The
driver registers once at module load; the kernel then drives the *same* canonical
sequence by calling the registered `.probe`/`.remove`/`.shutdown` slots. So the
Linux lowering **preserves the canonical ordering** (`register → create →
associate → preamble → on_init → teardown → on_deinit|on_surprise_remove →
postamble`, convergence §5) but **distributes it across scaffold scopes**:

| canonical step group | `generic_c` | Linux scaffold |
|---|---|---|
| `register` | one marker line, re-run in every scenario fn | once, at `module_init` / `module_*_driver()` + the static driver struct |
| `create`, `associate`, `preamble`, `on_init` | top of each scenario fn | the body of `.probe()` |
| `teardown`, `on_deinit`/`on_surprise_remove`, `postamble` | orderly/surprise scenario fn | the body of `.remove()` |
| init-fail unwind | `init-fail` scenario fn | the `.probe()` goto-error ladder |
| `on_system_shutdown` | shutdown scenario fn | the body of `.shutdown()` |

The four `LifecycleScenario` variants (`backend_options.rs:24-42`) are therefore
**not four emitted functions on Linux** — they are the kernel's natural invocation
paths into the one registered callback set. A second consequence: most
`generic_c` managed-step **markers are pure host-oracle observability with no
Linux runtime artifact** (`prelude.rs:120-147`); the step they mark becomes a
*position* in probe/remove/shutdown, and the only runtime-observable equivalents
are (a) the return codes and (b) `dev_*` logging. `create`, `associate`, and
`preamble` are the exceptions: their *step* lowers to real code (allocation,
drvdata binding, resource acquire) even though their *marker* is trace-only.

### C.1 `register` + scenarios → the kernel module scaffold

The `regseq_lc_register(name, match)` marker (`body_emit.rs:5197-5199`) packs two
facts. On Linux they split: **`name`** → the bus-driver struct's name field
(`pci_driver.name` reference-drivers/e1000/linux/e1000_main.c:181; `amba_driver.drv.name` reference-drivers/pl011/linux/amba-pl011.c:3190;
`platform_driver.driver.name`); **the match set** → a separate device-ID table
(§C.2). The **registration call itself is selected by the bus path** that BRM
supplies (`bus_selector_token` `:5050-5061`):
- `Pci` → `pci_register_driver(&drv)` (reference-drivers/e1000/linux/e1000_main.c:224), idiomatically
  `module_pci_driver(drv)`; unregister `pci_unregister_driver`.
- `Amba` → `amba_driver_register(&drv)` (reference-drivers/pl011/linux/amba-pl011.c:3205) / `module_amba_driver`.
- `Platform` (DT) → `platform_driver_register(&drv)` (reference-drivers/pl011/linux/amba-pl011.c:3203) /
  `module_platform_driver`.
- `I2c`/`Spi`/`Usb` are bridge-rejected at parse, so they never reach the backend;
  their forms are deferred.

The scaffold point is `module_init`/`module_exit` (reference-drivers/e1000/linux/e1000_main.c:235), or the
collapsing `module_*_driver()` macro. The registration is **module-scoped and runs
once** — the time-split from `generic_c`, where `register` re-prints in every
per-scenario function.

### C.2 `MatchEntry` → the device-ID table + `MODULE_DEVICE_TABLE`

`generic_c` renders only the **first** entry into a trace string
(`render_match_trace`, `body_emit.rs:5499-5509`). Linux uses the **full**
`match_set: Vec<MatchEntry>` (`device_lifecycle.rs:51`) — the kernel match tables
are zero-terminated arrays:
- `Devicetree { compatible }` → `struct of_device_id[]` with `.compatible`, wired
  via `platform_driver.driver.of_match_table` + `MODULE_DEVICE_TABLE(of, …)`.
- `Pci { vendor, device, subsystem, class }` → `struct pci_device_id[]`
  (`PCI_DEVICE(vendor, device)`, or the long form when `subsystem`/`class` are
  `Some`), wired via `pci_driver.id_table` (reference-drivers/e1000/linux/e1000_main.c:182) +
  `MODULE_DEVICE_TABLE(pci, …)` (reference-drivers/e1000/linux/e1000_main.c:64).
- `Amba { id, mask }` → `struct amba_id[]` `{ .id, .mask }` (reference-drivers/pl011/linux/amba-pl011.c:3167-3184),
  zero-sentinel terminated (`{ 0, 0 }`), wired via `amba_driver.id_table` (:3194) +
  `MODULE_DEVICE_TABLE(amba, …)` (:3186).
- `Acpi`/`Usb`/`I2c`/`Spi` are modeled-but-bridge-rejected; their tables are
  deferred (do not emit — fail loud, mirroring the bridge).

`MODULE_DEVICE_TABLE` additionally feeds userspace autoload (modalias) — a
Linux-only artifact with no `generic_c` analog.

### C.3 Bring-up → the body of `.probe()`

The kernel invokes `.probe()` after a successful table match (signatures:
`e1000_probe(struct pci_dev*, const struct pci_device_id*)` reference-drivers/e1000/linux/e1000_main.c:915;
`pl011_probe(struct amba_device*, const struct amba_id*)` reference-drivers/pl011/linux/amba-pl011.c:2971). The
bring-up steps lower into it in canonical order:

- **`create`** (`regseq_lc_create`, `:5206`; container morph struct `:5184-5189`;
  zero-init `{0}` `:5305`) → allocate the private driver container, zeroed:
  `devm_kzalloc(&dev->dev, sizeof(...), GFP_KERNEL)` (reference-drivers/pl011/linux/amba-pl011.c:2982) or
  `alloc_etherdev(sizeof(struct e1000_adapter))` (reference-drivers/e1000/linux/e1000_main.c:952). The morph's
  prepended `void *__rseq_dev` is the kernel device back-pointer (`adapter->pdev =
  pdev`, reference-drivers/e1000/linux/e1000_main.c:961). See §C.7.
- **`associate`** (`regseq_lc_associate`, `:5207`; backend-synthesized) → bind the
  container to the kernel `struct device`: `pci_set_drvdata(pdev, netdev)`
  (reference-drivers/e1000/linux/e1000_main.c:958) / `amba_set_drvdata` / `SET_NETDEV_DEV` (reference-drivers/e1000/linux/e1000_main.c:956). The
  round-trip is read back by `.remove`/`.shutdown` via `pci_get_drvdata`
  (reference-drivers/e1000/linux/e1000_main.c:1254) / `amba_get_drvdata` (reference-drivers/pl011/linux/amba-pl011.c:3031).
- **`preamble`** (`regseq_lc_preamble` per `ResourceField` in **forward**
  declaration order, `:5214-5217`) → the resource-acquisition span. This is the
  **cross-construct seam**: the BRM ops nest here, in the canonical order
  `generic_c` slots them (`:5244-5285`) — on Linux PCI: `pci_enable_device`
  (reference-drivers/e1000/linux/e1000_main.c:934) → `pci_request_selected_regions` (:942) → `pci_set_master`
  (:946, DMA-gated) → BAR enumeration → `map_region` = `pci_ioremap_bar`
  (:970). The **per-op Linux call is the BRM / register-access task's
  deliverable**; device-lifecycle owns only the *slot and the forward ordering*.
  The handle lands in a `ResourceField` member (`void __iomem *`): `hw->hw_addr`
  (reference-drivers/e1000/linux/e1000_main.c:970), `uap->port.membase` (reference-drivers/pl011/linux/amba-pl011.c:2922).
- **`on_init`** (`regseq_lc_call("on_init")` always + author body iff overridden,
  `:5311`,`:5318-5319`) → the author bring-up code inlined at the end of `.probe()`.
  Its `Result<(), RegseqError>` maps to probe's `int`: `Ok(())` → fall through to
  `return 0`; `Err(e)` → the `goto`-ladder then `return -<errno>` via the **shared
  RegseqError→errno adapter** (`:5345-5354`/`:5421-5467`). When `on_init` is *not*
  overridden, nothing is emitted.

Authored `RegseqError` is a **distinct channel** from native bring-up failures:
`pci_enable_device`/`pci_request_selected_regions`/`ioremap` failures already yield
kernel errnos directly (reference-drivers/e1000/linux/e1000_main.c:939-944); `on_init`'s `RegseqError` is mapped
through the errno table.

### C.4 Teardown → the body of `.remove()`

For orderly unbind and surprise removal the kernel calls the **one** `.remove()`
slot (reference-drivers/e1000/linux/e1000_main.c:1252; reference-drivers/pl011/linux/amba-pl011.c:3029). The teardown suffix
(`emit_teardown_dispatch`, `body_emit.rs:5391-5407`) lowers as:
- **`teardown` present-dispatch** (`regseq_lc_teardown(present)`, `:5398`) → no
  standalone Linux artifact; the `present` bit becomes a **runtime predicate** at
  the top of `.remove()` gating HW access — `present=1` (orderly, HW reachable) vs
  `present=0` (surprise, HW gone, e.g. `pci_device_is_present()` /
  `pci_channel_offline(pdev)`). `generic_c` emits two byte-distinct functions;
  Linux funnels both into one `.remove()` branching on the predicate.
- **`on_deinit`** (orderly, `:5321` present=1) → the author teardown body inlined,
  HW reachable. Its `Result` is **discarded** (CR-4) — and modern `.remove()`
  returns `void` (reference-drivers/e1000/linux/e1000_main.c:1252), so there is structurally nowhere to return it.
- **`on_surprise_remove`** (surprise, `:5332-5339` present=0) → the same `.remove()`
  slot on the device-gone branch; the author body must not touch HW. No separate
  bus-ABI slot exists for it on Linux.
- **`postamble`** (`regseq_lc_postamble(res, skipped)` in **reverse** order,
  `:5404-5406`) → the resource-release span, exact-reverse of the §C.3 acquire:
  `iounmap` (reference-drivers/e1000/linux/e1000_main.c:1275) → `pci_release_selected_regions` (:1278) →
  `pci_disable_device` (:1284). `skipped(surprise)` → skip HW-touching releases
  while still doing local unmap/free. For `devm_*`-managed resources the release is
  **automatic** on `.remove()`, so the postamble may emit nothing.

Note: on Linux `.remove()` runs **only** for a device that probed successfully. A
probe failure does *not* call `.remove()` (§C.5).

### C.5 Init-failure → the `.probe()` goto-error ladder

`generic_c` captures `on_init`'s `Result`, reads the niche discriminant, and on
the `Err` branch emits `regseq_lc_on_init_err()` then the `on_init_failure` marker
+ author body (`emit_init_fail_body`, `body_emit.rs:5421-5483`). It asserts via the
stdout trace and **exits cleanly**, not a nonzero exit (GRILL D12). Linux's natural
realization is the **probe error ladder** — which has **two disjoint families of
exits** that must not be merged (the authored/native boundary, §E.6):

- **Native-unwind labels** — a compiler-generated preamble call fails
  (`pci_enable_device`, `pci_request_selected_regions`, `ioremap`/`IS_ERR`;
  reference-drivers/e1000/linux/e1000_main.c:939-944). The label chain releases the resources acquired so far,
  in reverse, and returns the **native** errno directly. `on_init` **never ran**
  on this path, so **`on_init_failure` is NOT called** — as-built it runs only
  after `on_init`'s `Result` discriminant reads `Err`
  (`emit_init_fail_body`, `body_emit.rs:5470-5481`), and the authored hook has
  no business observing an OS failure the author's code never saw.
- **The authored `on_init -> Err` arm** — `on_init` ran and returned
  `Err(RegseqError)` (the `if (err) goto err_…;` shape after the authored call,
  reference-drivers/e1000/linux/e1000_main.c:971-972). This is the **only** path that runs the
  `on_init_failure` author cleanup hook, *inside* its goto-cleanup block
  **before `.probe()` returns the negative errno** via the §E.3 adapter.
  Because the kernel does not call `.remove()` on probe failure,
  `on_init_failure` (plus the partial reverse postamble) **is** the failure
  cleanup; `on_deinit` does *not* run.
- **partial (high-water-mark) reverse postamble** — shared by both families: the
  labelled cleanup blocks release exactly the resources acquired so far, in
  reverse: `err_ioremap: … iounmap → free_netdev → pci_disable_device`
  (reference-drivers/e1000/linux/e1000_main.c:1224-1239). This is the HWM rollback the convergence doc records
  as **deferred as-built** in `generic_c` (the init-fail fixture is fieldless).
  On Linux it is mandatory and idiomatic — a Linux backend must thread the
  acquired-resource list into the ladder. `devm_*` resources unwind
  automatically. The two families differ only in their *entry* (which label is
  jumped to) and their *extras* (the authored arm runs the hook + adapter); the
  release blocks themselves are shared.

Divergence to flag: `generic_c`'s behavioral oracle for init-fail is the stdout
trace + clean exit; Linux's is the **negative errno returned from `.probe()`**.

### C.6 System shutdown → `.shutdown()` (omit-when-absent)

`on_system_shutdown` is the lone **omit-when-absent** callback (`:5360-5368`;
D10/I02). Overridden → emit the `.shutdown` slot pointing at the author body
(`pci_driver.shutdown` reference-drivers/e1000/linux/e1000_main.c:186, `e1000_shutdown` :5200; for AMBA the
`struct device_driver.shutdown` on `.drv`). Returns `void` → `Result` discarded.
Not overridden → **leave the `.shutdown` initializer out**; the kernel skips it.

### C.7 The container morph + `ResourceField`/`UserDataField` storage

The Policy-P morph (`struct __rseq_lc_<idx>_<label>_target { void *__rseq_dev;
<driver> inner; uint32_t __rseq_state; }`, `:5184-5189`) is the **standard Linux
private-data container**:
- `void *__rseq_dev` → the kernel device back-pointer recovered by
  `container_of`/drvdata (`adapter->pdev` reference-drivers/e1000/linux/e1000_main.c:961; `netdev_priv` :959).
- `inner` (the spec driver, embedded verbatim) → the device-specific private struct
  (`struct e1000_adapter`, `struct uart_amba_port`). Callbacks run **spec-view over
  `&inst.inner`** (CR-7) — on Linux that is `netdev_priv(netdev)` /
  `amba_get_drvdata(dev)` handed to the author callback.
- `uint32_t __rseq_state` → appended OS-managed metadata (driver-private flags).
- **`ResourceField`/`ResourceKind::Mmio`** (`device_lifecycle.rs:186-201,253-257`)
  → a `void __iomem *` member, acquired in `.probe()` and released in `.remove()`.
  `Mmio` is the only kind (closed taxonomy).
- **`UserDataField`/`UserDataKind`** (`device_lifecycle.rs:263-279`) is
  `recognized_not_lowered` even in `generic_c` — its init is *incidentally*
  satisfied by the container zero-init `{0}` (`:5305`), which on Linux is the
  zeroed allocation. `ZeroInit` maps directly; `OptionCopy` (`None` = unseeded) is
  satisfied for the build-now scalar whitelist by the same zeroing. There is **no
  designed lowering** — a new backend inherits none and must either rely on
  zero-alloc or design one (do not invent).

### C.8 `device_class` / `ClassTag` — recognized, not lowered

`ClassTag` (`device_lifecycle.rs:135-144`) is carried but **unvalidated and
unlowered** (`generic_c_source = none`); the class-trait cross-check is deferred to
the Device Class construct (convergence §1.2, D06). Linux **does not lower it** —
no `class_create`/`class_register` is synthesized (that would be invention against
the citation rule). It is recorded and passed through.

### C.9 Cross-construct composition (preserved, never isolated)

1. **BRM preamble ⊂ DL preamble.** The four BRM managed ops + register-access
   `map_region` are emitted *inside* the DL preamble slot, before `on_init`, in
   canonical order (`:5244-5285`). DL owns the slot+ordering; BRM/register-access
   own the calls.
2. **`map_region` consumes BRM-resolved `(base,size)`.** On Linux the host literal
   is replaced by `pci_resource_start/len` (PCI) or `DT_REG_ADDR`/
   `devm_ioremap_resource` (platform), **changing no register-access op** (I04).
3. **`on_init` Result → probe errno via the shared adapter.** The niche-discriminant
   read is shared between the RegseqError exit-adapter and the DL init-fail path.
4. **`register` marker (DL) + bus selector (BRM)** jointly pick the registration
   call (`pci_register_driver` vs `amba_driver_register` vs
   `platform_driver_register`).

The mapping is well-grounded in the vendored reference drivers (PCI:
`e1000_main.c`; AMBA/platform: `amba-pl011.c`) and the as-built oracle; it is
specified confidently. The residual uncertainties are genuine *design forks* and
*version-sensitive idiom choices*, not gaps in lowerability.

---

## D. Diagnostics-logging — Linux backend lowering

### D.0 Scope & status caveat

The group contains exactly four primitives: `IntrinsicKind::Log(LogLevel::{Debug,
Info,Warn,Error})` (`domain_attr.rs:90,141-149`; dash-form `from_name` `:161-164`).
The template parser, segment emitter, `&str` handling, numeric-specifier mapping
and level-tag helpers are not separate primitives but the *mechanism* by which
these four lower, so they are covered here as such.

Unlike the other constructs, `logging-and-printing` has **no finalized
`*-surface-convergence.md`**. The only doc is the design discussion, whose status
line reads "the current `fmt::Arguments`-based Phase-B direction should be treated
as superseded pending this design work" (`logging-and-printing-discussion.md:469-472`).
The authoritative *as-built* surface is therefore the model intrinsic taxonomy plus
`generic_c`'s `emit_diag_log`. This spec lowers the as-built reality and flags,
explicitly, where the discussion's aspirations (pointers, `{:p}`, source context)
are **not** in the as-built and so are **not** lowered.

### D.1 The model surface that reaches the backend

- **Recognition.** A `Call` whose `Direct` callee carries `IntrinsicKind::Log(level)`
  is intercepted in `BodyEmit::call` (`body_emit.rs:2100-2104`). Recognition is
  keyed by instance identity, so an `Indirect` (fn-pointer) callee can never match —
  the same constraint holds on Linux.
- **Flattened arguments.** By the time the backend sees the call, the bridge's
  DiagLogPeel pass has flattened the `LogArg` slice into a flat operand list:
  `args[0]` is the format-template `&str` literal, `args[1..]` are the raw value
  operands (`body_emit.rs:2714-2716,2732-2739`).
- **Template grammar.** A restricted subset: the default `{}`, the eight hex forms
  `{:x} {:X} {:#x} {:#X} {:08x} {:08X} {:#010x} {:#010X}`, `{{`/`}}` escapes,
  embedded newlines, and the empty template (`parse_template` `:5903-5950`;
  `parse_placeholder` `:5956-6015`).
- **Closed admissible argument kinds (as-built).** Signed `i8/i16/i32/i64`;
  unsigned `u8/u16/u32/u64`; `bool` (numeric 0/1); and `&str` (static + non-static
  runtime borrow). **Not admitted:** `isize`/`usize`, `char`, pointers/`{:p}`
  (`diag_specifier_and_cast_for` `:5798-5850`, comment `:5791-5793`).
- **Return / control.** A `Log` callee returns unit and never diverges, so `target`
  is always `Some` (`:2728-2730`).
- **Gates.** (a) arity: placeholder count == `args.len()-1` (`:2741-2750`);
  (b) modifier-subset; (c) admissible-type; (d) `&str` admits only the default
  modifier — a hex modifier on `&str` fails loud (`require_default_modifier`
  `:5856-5864`).

### D.2 Reference lowering (`generic_c`) — the contrast

`generic_c` targets a **hosted C program that writes to `stdout`**, and it is the
*only* construct lowered directly to a target stdlib sink with **no `regseq_*` host
hook**. Each `Log` call lowers to a byte-faithful `fwrite` sequence
(`emit_diag_log` `:2722-2849`): a bracketed text tag `[DEBUG|INFO|WARN|ERROR] `;
coalesced literal runs as `fwrite("<escaped>", 1, <len>, stdout)`; each `&str`
value as `fwrite` of its decoded bytes; numerics via `snprintf` into a `char[24]`
then `fwrite`; a trailing `\n`; then `goto bb_<target>`. The defining property is
**byte-faithfulness**: every byte is written with an explicit count, so an interior
NUL is reproduced and a literal `%` is **not** doubled (`c_escape` note `:6020-6022`).

### D.3 Idiomatic Linux lowering

The Linux kernel **has** a printf-style logging engine (`printk`/`vsnprintf`). The
idiomatic lowering therefore **reassembles** the decomposed segments back into a
single format string plus an argument list — the *inverse* of `generic_c`'s
per-segment `fwrite` decomposition, and the same move the Zephyr lowering makes for
`cbprintf`. Each `Log(level)` call lowers to **one** device-aware kernel logging
macro invocation.

**D.3.1 Level → macro (the text tag becomes structural).**

| `LogLevel` | device-aware (preferred) | non-device fallback | `KERN_*` level |
|---|---|---|---|
| `Debug` | `dev_dbg(dev, …)` | `pr_debug(…)` | `KERN_DEBUG` |
| `Info`  | `dev_info(dev, …)` | `pr_info(…)`  | `KERN_INFO` |
| `Warn`  | `dev_warn(dev, …)` | `pr_warn(…)`  | `KERN_WARNING` |
| `Error` | `dev_err(dev, …)`  | `pr_err(…)`   | `KERN_ERR` |

In-repo evidence: `dev_dbg(uap->port.dev, "no DMA platform data\n")`
(reference-drivers/pl011/linux/amba-pl011.c:449), `dev_err(…)` (:460), `dev_info(uap->port.dev, "DMA channel TX
%s\n", …)` (:468), `dev_warn(…)` (:927); and the `dev_*` wrapper idiom
(reference-drivers/e1000/linux/e1000.h:325-330). Macro→`KERN_*` mapping:
<https://docs.kernel.org/core-api/printk-basics.html>. `generic_c`'s inline
`[LEVEL] ` text tag is **dropped**: on Linux the severity is carried *structurally*
by the macro / `KERN_*` prefix, and `dev_*` additionally prepends the device
identity.

**D.3.2 The `dev` argument — cross-construct composition (the unsolved seam).**
`dev_*` requires a `struct device *` — exactly the device the
device-lifecycle/BRM synthesis binds (the probe's `platform_device`/`pci_dev`,
reached as `&pdev->dev`). In-repo: `struct device *dev = uap->port.dev;`
(reference-drivers/pl011/linux/amba-pl011.c:427) and `dev_info(&adapter->pdev->dev, …)` (reference-drivers/e1000/linux/e1000.h:326). This is
the **one real composition seam** for Linux diag-log — `generic_c` has none.

Be precise about where `dev` is actually reachable, because as-built it is
*nowhere in lowered code*: `Log` lowering is a per-call-site interception inside
ordinary lowered bodies, bank 5 emits OS-neutral ordinary functions
(`mod.rs:2276`), and the lifecycle callbacks run spec-view over `&inst.inner`
(`body_emit.rs:5472`, CR-7) — **no lowered function has a `struct device *`
parameter or a path to one today**. `dev` is structurally in scope only inside
the *backend-synthesized* probe/remove/shutdown scaffold statements themselves.
For a `Log` inside an authored callback body (or any helper it calls) to lower
to `dev_*`, the backend must adopt one of three strategies, none of which is
designed yet:
1. **signature threading** — inject a `dev` parameter into every lowered
   function reachable from a lifecycle callback (changes lowered signatures; a
   call-graph rule);
2. **container recovery** — recover the morph container from the callback's
   `&inst.inner` self pointer (`container_of` back to the §C.7 target struct,
   then `__rseq_dev`); only valid where the self pointer *is* the morph's
   embedded `inner`, so it does not cover helpers with non-driver receivers;
3. **intentional `pr_*` fallback** — every `Log` in a lowered ordinary body
   emits `pr_*`, with `dev_*` reserved for scaffold-synthesized statements.

**Interim normative rule: (3)** — `dev_*(dev, …)` only in statements the
scaffold itself synthesizes (where `pdev` is a parameter); **every `Log` in a
lowered ordinary body lowers to `pr_*`** until a strategy is ratified. The
choice is open question **A-3** and must be decided **before implementation** —
silently falling back per-call-site (some `dev_*`, some `pr_*`, by accident of
inlining) is the one outcome this rule exists to prevent.

**D.3.3 Format-string reassembly (inverse of the fwrite decomposition).** Walk
`parse_template`'s `Segment`s in order, building one format literal `F` and one
argument list `A`:
- **`Segment::Literal(bytes)`**: append `c_escape(bytes)` to `F` — **but** `F` is
  now a `printf` format, so every literal `%` **must** be doubled to `%%` (the exact
  opposite of `generic_c`). `\n`/`\t`/`\r`/octal escapes as in `c_escape`.
- **Numeric `Segment::Placeholder`**: reuse `diag_specifier_and_cast_for(modifier,
  ty)` **whole** (`:5798-5850`). Its `(spec, cast)` is already a `printf` conversion
  + varargs-promotion cast, and `printk` is the *same* varargs ABI, so it carries
  over unchanged — append `spec` to `F`, push `cast(expr)` to `A`. Default forms:
  `bool/i8/i16/i32 → %d/(int)`, `u8/u16/u32 → %u/(unsigned)`, `i64 → %lld`, `u64 →
  %llu`. Hex forms: `%[#][0w][ll]{x|X}`. All nine are standard `printk` conversions
  (<https://docs.kernel.org/core-api/printk-formats.html>; in-repo
  `reference-drivers/e1000/linux/e1000_param.c:203`,
  `reference-drivers/pl011/linux/amba-pl011.c:468`).
- **`&str` `Segment::Placeholder`** (default modifier only): append `%.*s` to `F`
  and push **two** args — `(int)len, (const char *)data`. For a non-static `&str`,
  read the fat pointer as `generic_c` does (`:2812-2817`); for a static `&str`, the
  decoded bytes as a C string-literal + its length (`:2794-2802`). Passing the
  string as a `%.*s` **argument**, never folding it into `F`, preserves
  `generic_c`'s "value bytes are never a format" safety and bounds the read to
  exactly `len` bytes. **Faithfulness gap:** C `%.*s` stops at the first interior
  NUL even within the precision (C11 §7.21.6.1), so an interior NUL is **not**
  reproduced — unlike `generic_c`'s `fwrite`. Same gap recorded for Zephyr.
- **Trailing newline**: append `\n` to `F`. Required — a message without `\n` is
  held as a `KERN_CONT` continuation and may interleave; corroborated in-repo
  (reference-drivers/pl011/linux/amba-pl011.c:449,460,468).

**D.3.4 The emitted statement.** `dev_<level>(dev, "<F>", <A…>);` followed by the
successor `goto bb_<target>`. Worked example (`info!("DMA channel {} acquisition
failed", channel)` with `channel: u32`):

```c
dev_info(dev, "DMA channel %u acquisition failed\n", (unsigned)channel);
goto bb_<target>;
```

structurally the in-repo line `dev_info(uap->port.dev, "DMA channel TX %s\n", …)`
(reference-drivers/pl011/linux/amba-pl011.c:468).

**D.3.5 `dev_dbg` conditional compilation.** `dev_dbg`/`pr_debug` compile to no-ops
unless `DEBUG` is defined, or are runtime-controllable under `CONFIG_DYNAMIC_DEBUG`.
This faithfully matches the lowest severity (debug commonly disabled in
production). Whether to instead force always-on debug for parity with `generic_c`
is an open decision.

### D.4 Error / lifetime

- **No failure channel.** `Log` returns unit and never diverges. On the Linux
  side there is no usable status either: the v6.6 `dev_*` level macros expand to
  **`void`** functions (`_dev_err`/`_dev_warn`/`_dev_info`, `dev_printk.h:34-56,
  143-160`,
  <https://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git/plain/include/linux/dev_printk.h?h=v6.6>),
  and `pr_*` expands to `printk`, whose `int` result is conventionally ignored.
  This channel is entirely **separate** from authored `RegseqError` and from
  native probe/resource failures (honors the three-channel rule).
- **Lifetimes / safety.** The format literal is `'static`; a non-static `&str`'s
  `(data,len)` borrow is live at the call by Rust's borrow rules; `%.*s` reads
  exactly `len` bytes → no over-read. No heap allocation — `printk` renders into the
  kernel ring buffer.

### D.5 What is NOT lowered (explicit)

- **Pointers / `{:p}`**: not in the accepted modifier set and no pointer numeric
  type. The discussion floats `{:p}` but it is absent from the as-built — do **not**
  invent. (If ever added, the Linux idiom is `%p`/`%px`/`%pK` — a new surface.)
- **`isize`/`usize`, `char`**: explicitly rejected (`:5793,5809-5813`).
- **Source context (file:line)**: neither the model intrinsic nor `generic_c`
  carries it. `dev_*` injects device identity but no source location — not lowered.
- **Indirect (fn-pointer) log calls**: never recognized (Direct-only, `:2100`).

### D.6 Version pin & open decisions

Pinned to mainline LTS **6.6+**. `dev_*`/`pr_*`/`KERN_*` are stable across this
range. Open decisions (none block specification — all are policy choices with a
clear recommendation, except (2) which must be decided before implementation):
(1) `&str` interior-NUL faithfulness — *recommend* idiomatic `%.*s` + documented
divergence; (2) the device-context strategy — signature threading vs container
recovery vs intentional `pr_*` fallback (§D.3.2); interim rule is `pr_*` in all
lowered ordinary bodies (A-3); (3) `dev_dbg` dynamic-debug gating vs forced-on —
*recommend* idiomatic gating; (4) the logging discussion is marked superseded —
confirm the as-built four-level / nine-modifier surface is the frozen contract
before freezing this mapping.

---

## E. RegseqError — Linux backend lowering

### E.0 What `regseq-error` is in the IR

`regseq-error` is the **authored semantic error channel** (`regseq-error-spec.md:30-44`,
`:69-94`), deliberately **not** a wrapper over OS-native error families. It
materializes as three things, all lowered together and in order:
1. a **recognition marker** — `AttrData::FrameworkType(FrameworkType::RegseqError)`
   on the error enum's item (`domain_attr.rs:77-80`, `:68`);
2. a **carrier** — `core::result::Result<(), RegseqError>` as the callback output
   (`regseq-error-spec.md:182`, `:196`);
3. a **boundary** — the outermost authored fallible callback exit, the *only* place
   semantic `RegseqError` is lowered to a target mechanism.

The reference backend lowers this as faithful body emission of the carrier (a
niche-encoded C struct) plus a **single hand-emitted boundary adapter**
(`emit_exit_adapter`, `body_emit.rs:4732-4828`) that reads the discriminant and
converts to a libc `<errno.h>` symbol via a per-target name→symbol table
(`errno.rs:35-48`). **The errno table lives in the backend precisely because the
host-error mapping is a per-target concern** (`errno.rs:5-9`). The Linux backend
inherits the *entire* recognition + carrier + discriminant-read machinery
unchanged and substitutes only what is Linux-specific: **errno polarity, adapter
placement, and the include name**.

### E.1 Recognition marker — `FrameworkType::RegseqError`

Read identically on every target: `is_framework_error_enum` walks
`model.domain_attrs.item_attrs` (`mod.rs:670-679`). It is the certification gate in
`classify_entry_return`: a `Result<(), E>`-shaped return is only eligible for errno
lowering when `E`'s item carries the marker; an unmarked `Err` enum is a
**fail-loud rejection** (`mod.rs:647-658`). **Linux: reuse verbatim.** Its role on
Linux is the compile-time discriminator between **the authored error channel**
(eligible for C02→`-errno`) and **every native operational channel** (which must
*bypass* this mapping — §E.6).

### E.2 Carrier — `Result<(), RegseqError>` and the `Ok` channel

`rustc` lays it out as a **niche-encoded two-variant enum**, `Ok = variant 0`,
`Err = variant 1` (`mod.rs:574-575`, `:613-625`). `generic_c` certifies via the
`is_result` flag + an `Ok`/`Err` name guard, and requires the `Ok` payload be
exactly `()` (`mod.rs:600-635`). The carrier is read through
`niche_discriminant_read_block` (`body_emit.rs:4654-4707`); on `Ok` the adapter
emits `return 0;` (`:4819-4821`). **Linux:** the carrier's internal representation
is unchanged; the success channel maps to the **universal kernel convention:
success is `0`** (`probe` returns `0` on success —
<https://docs.kernel.org/driver-api/driver-model/driver.html>). So `Ok(())` →
`return 0;` is **identical** on Linux.

### E.3 The boundary adapter — placement and polarity (the one real Linux delta)

`generic_c`'s adapter is a standalone `int <entry>__regseq_exit(void)` that (1)
calls the entry, (2) reads the discriminant and `return 0;` on `Ok`, (3) otherwise
`switch`es the inner `RegseqError` Direct tag-view emitting one
`case <tag_value>: return <SYMBOL>;` per variant (`:4763-4809`), with `default:
return EIO;` as **corrupt-discriminant defense only** (`:4824`; I05). A marked
variant absent from the table is `EmitError::ErrnoMapping` — fail-loud, never a
silent `EIO` (`:4807`). The TU `#include <errno.h>` demand-driven (`prelude.rs:434-438`),
and the harness `main` does `return <adapter>();` (`naming.rs:146-156`).

Two facts are decisive for Linux: `generic_c` emits **un-negated (positive)** errno
symbols (correct for a userspace process whose libc resolves the numeric,
`errno.rs:11-13`), and the symbols are emitted **symbolically** (host-deterministic,
I03).

**Linux.** The kernel's universal callback convention is **negative errno on
failure, `0` on success**
(<https://docs.kernel.org/driver-api/driver-model/driver.html>).
The Linux adapter is the `generic_c` adapter with **three substitutions and nothing
else**:
1. **Negate every error arm**: `case <tag_value>: return -<SYMBOL>;` and
   `default: return -EIO;`. The `Ok` arm stays `return 0;`.
2. **Relocate the adapter** from a standalone `<entry>__regseq_exit` + harness
   `main` to the **driver-core callback return site** (§E.4).
3. **Header**: the demand-driven include becomes `#include <linux/errno.h>`, routed
   through the same `HelperUsage::record_errno_include` machinery so a no-error TU
   is byte-unchanged.

Everything else is reused **verbatim**: the niche read, the tag-view switch, the
symbolic-only emission, the fail-loud-on-unmapped rule, and the C02 table itself,
whose nine symbols are all valid Linux UAPI errnos (§E.7).

### E.4 Where the boundary lives on Linux — callback exits

- **`Lifecycle::init` ⇒ the `probe()` return** (`int probe(struct platform_device
  *)` for platform/DT; `int probe(struct pci_dev *, const struct pci_device_id *)`
  for PCI). The synthesized lifecycle path already **captures `on_init`'s `Result`
  and reads its discriminant** (`body_emit.rs:5439-5476`, sharing
  `niche_discriminant_read_block` at `:5466`). On Linux this read is **fused** with
  the errno conversion of §E.3: `Ok` → fall through to `return 0;`; `Err` → run the
  init-failure cleanup (`on_init_failure` + reverse `postamble`, `:5475-5481`) **and
  then** `return -<errno>;`. `generic_c` keeps these two uses separate; the Linux
  probe must do both — the principal cross-construct composition point (D-2).
- **`Lifecycle::deinit` ⇒ the `remove()` callback** — whose return channel is
  bus-shaped: PCI `.remove` is `void` (vendored witness
  reference-drivers/e1000/linux/e1000_main.c:1252) and AMBA `.remove` is `void` (reference-drivers/pl011/linux/amba-pl011.c:3029), but
  `struct platform_driver` at v6.6 **still carries `int (*remove)` plus the
  `void (*remove_new)` transition member** that new drivers are told to
  implement (v6.6 `platform_device.h:236-250`,
  <https://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git/plain/include/linux/platform_device.h?h=v6.6>;
  the `int` return is ignored by the platform core apart from a warning),
  while `.remove` itself became `void` in v6.11 and `.remove_new` was later
  retired — at v6.13 only `void (*remove)` remains (v6.13
  `platform_device.h:236-239`,
  <https://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git/plain/include/linux/platform_device.h?h=v6.13>).
  Because the stated `6.6+` target spans that transition, this spec pins the
  **version-gated member selection** for platform (C-3, resolved): one
  `void`-signature teardown function, assigned to **`.remove_new` when
  `LINUX_VERSION_CODE < KERNEL_VERSION(6, 11, 0)` and to `.remove` from v6.11
  on** — both members share the `void (*)(struct platform_device *)`
  signature, so the function body is identical on both arms. On every teardown
  path, then, a
  `RegseqError` escaping `deinit` has **no return channel**; it can only be
  logged (composing with diagnostics-logging → `dev_err`, out of scope).
  (D-3/C-3.)

### E.5 Recommended shape: faithful carrier + boundary adapter

`generic_c` lowers the authored body **faithfully** — the body produces/consumes
the `Result` C struct, and **only** the outermost adapter (and the init-fail read)
ever inspects it (`body_emit.rs:4709-4716` explains *why* the adapter is
hand-emitted: model constants are numeric/relocs with no symbolic-macro form). The
perspective-6 doc's "`configure()` returns `-EINVAL` directly" examples
(`perspective-6-...md:604-625`) are an **illustrative sketch**, not the implemented
mechanism. The Linux backend should follow the as-built shape — faithful carrier
struct internally, negated-errno conversion **only at the callback boundary** —
because it reuses the entire certified discriminant-read machinery, keeps authored
helper bodies emitting/propagating the carrier rather than scattering errno
conversions, and same-type `?` on `Result<_, RegseqError>` already normalizes to a
discriminant match pre-translation. (D-1 records the alternative.)

### E.6 Cross-construct composition — authored vs native channel

Native operational failures are a **different error channel** from authored
`RegseqError` (`perspective-6-...md:5-12`, `:443-493`; `regseq-error-spec.md:134-153`).
On Linux the two share the same C `int` return type and *mostly* the same errno
vocabulary, but must not be conflated:
- **Authored channel** — only values constructed as `RegseqError::*` (or propagated
  from author helpers) flow through the §E.3 adapter. The marker certifies
  membership.
- **Native channel** — the compiler-generated preamble (`enable_device →
  claim_regions → set_bus_master → get_resource_handle → map_region`,
  `body_emit.rs:5256-5285`) calls native APIs (`pci_enable_device`,
  `pci_request_regions`, `devm_ioremap_resource`) that **already return negative
  errno**. These propagate **directly**, never round-tripped.

The sharpest consequence: **`-EPROBE_DEFER` has no `RegseqError` variant and must
survive untouched.** It is a kernel-internal errno (`517`, the `>512` range, not
exported to userspace) the device-model core recognizes to retry probe. The §E.3
adapter only ever emits the nine C02 codes (+ defensive `-EIO`), so it **can never
synthesize `-EPROBE_DEFER` and can never overwrite a native one**. The cleanup
ordering on the error path is the device-lifecycle reverse-`postamble` goto chain
(`body_emit.rs:5403-5406`).

### E.7 The nine variant→errno mappings on Linux

The C02 table (`errno.rs:36-47`) is name-keyed and order-independent, and two
variants may legitimately share a symbol (`HardwareError` and `GenericFailure` both
→ `EIO`). All nine are valid Linux **UAPI** errnos (numeric range ≤ 133):

| Variant | Symbol | Linux emits | UAPI header |
|---|---|---|---|
| `InvalidParameter` | `EINVAL` (22) | `return -EINVAL;` | `errno-base.h` |
| `OutOfMemory` | `ENOMEM` (12) | `return -ENOMEM;` | `errno-base.h` |
| `HardwareError` | `EIO` (5) | `return -EIO;` | `errno-base.h` |
| `Timeout` | `ETIMEDOUT` (110) | `return -ETIMEDOUT;` | `errno.h` |
| `Unsupported` | `EOPNOTSUPP` (95) | `return -EOPNOTSUPP;` | `errno.h` |
| `DeviceBusy` | `EBUSY` (16) | `return -EBUSY;` | `errno-base.h` |
| `PermissionDenied` | `EPERM` (1) | `return -EPERM;` | `errno-base.h` |
| `DeviceNotFound` | `ENODEV` (19) | `return -ENODEV;` | `errno-base.h` |
| `GenericFailure` | `EIO` (5) | `return -EIO;` | `errno-base.h` |

Note on `Unsupported → EOPNOTSUPP`: this is the **correct** userspace-visible
choice. Linux also has `ENOTSUPP` (524) but it is kernel-internal and must not leak
to userspace. `generic_c`'s table already picks `EOPNOTSUPP` (`errno.rs:41`), so the
Linux backend needs no change here — it only negates. The Linux table is thus
**symbol-identical** to `generic_c`'s; the per-target divergence the design
anticipated shows up on Linux as **polarity, not symbol choice**.

### E.8 Error / lifetime semantics summary

- **Success**: `Ok(())` → `return 0;` (identical; universal kernel convention).
- **Authored failure**: `Err(RegseqError::V)` → `return -errno_symbol(V);` at the
  callback boundary, after running any device-lifecycle init-failure cleanup.
  Fail-loud at *emit time* if `V` is marked but unmapped (`:4807`); never silent.
- **Corrupt discriminant**: `default: return -EIO;` (defensive only, I05).
- **Native failure**: bypasses this construct entirely; propagates the native
  negative errno (incl. `-EPROBE_DEFER`) directly.
- **Determinism (I03)**: emission stays symbolic (`-EINVAL`), so the `.c` is
  host-stable; the kernel build resolves the numeric. Variants emitted in
  deterministic order (`:4800-4801`).

### E.9 Scaffold points

- Marker recognition + classification: reuse `mod.rs:565-660` unchanged.
- Carrier discriminant read: reuse `body_emit.rs:4654-4707` unchanged.
- Adapter body: a Linux analogue of `emit_exit_adapter` (`:4732-4828`) that negates
  the arms and is emitted at the callback site rather than a standalone
  `__regseq_exit`.
- errno table: reuse `errno.rs:35-48` (symbols identical); negate at the emit site.
- Demand-driven include: reuse `HelperUsage::record_errno_include` /
  `helper_block` (`prelude.rs:363-367`, `:434-438`), substituting `<linux/errno.h>`.
- Composition with device-lifecycle init-fail: fuse the §E.4 discriminant read with
  `emit_init_fail_body`'s cleanup branch (`body_emit.rs:5439-5481`).
