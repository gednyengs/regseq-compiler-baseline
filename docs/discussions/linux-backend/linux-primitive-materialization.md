# Linux primitive materialization — per-primitive table

The concrete Linux realization of every primitive in the census
([`../backend-lowering/primitive-inventory.md`](../backend-lowering/primitive-inventory.md)),
one row each: **model surface → `generic_c` reference → Linux materialization →
disposition**. This is the quick-reference index; the normative prose is in
[`linux-lowering-per-construct.md`](./linux-lowering-per-construct.md) and the full
trace (scaffold/error/open columns) is in
[`linux-traceability-matrix.md`](./linux-traceability-matrix.md).

Dispositions (per [`linux-model-analysis.md`](./linux-model-analysis.md)):
**NM** native-materialization · **SPC** shared-procedural-core · **NA**
not-applicable · **NNS** needs-new-surface.

Linux materializations are pinned to mainline LTS 6.6+ and are **backend-[Open]**
proposals confirmed against the vendored stock drivers
(`reference-drivers/{pl011,e1000}/linux/`) + kernel.org docs — never asserted as
shipped. PCI vs platform/DT is given as `PCI` / `plat` where they differ.

---

## register-access

| primitive | model_source | generic_c ref | Linux materialization | disp. |
|---|---|---|---|---|
| `mmio_read` | `domain_attr.rs:127,167` | `body_emit.rs:2297-2302` | `dst = (T)readl(base+off);` (width → `readb/w/l/q`; width-8 `CONFIG_64BIT`-gated, A-12) | NM |
| `mmio_read_field` | `domain_attr.rs:129,168` | `body_emit.rs:2303-2311` | `dst = (T)((readl(base+off) & M) >> S);` | NM |
| `mmio_write` | `domain_attr.rs:131,169` | `body_emit.rs:2283-2287` | `writel((u32)val, base+off);` (**value-first**) | NM |
| `mmio_write_field` | `domain_attr.rs:133,170` | `body_emit.rs:2288-2296` | `writel((u32)(V & M), base+off);` (masked, not RMW) | NM |
| `mmio_modify` | `domain_attr.rs:135,171` | `body_emit.rs:2312-2323` | `{u32 cur=readl(base+off); writel((cur&~(u32)M)|(u32)(V&M), base+off);}` | NM |
| `poll_until` | `domain_attr.rs:95,172` | **none** | *(candidate, non-normative)* `readl_poll_timeout(addr,val,cond,delay_us,timeout_us)` | NNS |
| `map_region` | `device_lifecycle.rs:223-228,236-238` | `body_emit.rs:5277-5284` | PCI `base=pci_iomap(pdev,bar,0)`; plat `base=devm_ioremap_resource(dev,res)` | NM |

## bus-resource-management

| primitive | model_source | generic_c ref | Linux materialization | disp. |
|---|---|---|---|---|
| `register_driver` | `device_lifecycle.rs:61`; selector `body_emit.rs:5050-5061` | `body_emit.rs:5203-5205` | `module_pci_driver` / `module_platform_driver` / `module_amba_driver` | NM |
| `enable_device` | `device_lifecycle.rs:96-104` | `body_emit.rs:5256-5259` | PCI `pci_enable_device`/`pcim_enable_device`; plat — *(default-on)* | NM/NA |
| `claim_regions` | `device_lifecycle.rs:223` | `body_emit.rs:5260-5266` | PCI `pci_request_selected_regions`; plat **fused** into `devm_ioremap_resource` | NM/NA |
| `set_bus_master` | `device_lifecycle.rs:62-70` | `body_emit.rs:5267-5270` | `pci_set_master(pdev)` — *dormant (`dma_present` false)* | NA |
| `get_resource_handle` | `device_lifecycle.rs:236-238` | `body_emit.rs:5271-5272` | PCI `pci_resource_start/len`; plat `platform_get_resource(...,IORESOURCE_MEM,...)` | SPC |
| `read_config` | `domain_attr.rs:113,175` | `body_emit.rs:2364-2375` | `pci_read_config_{byte,word,dword}` (w 1/2/4); status+out-param marshal; status explicitly discarded `(void)…` pending A-11 | NM |
| `write_config` | `domain_attr.rs:116,176` | `body_emit.rs:2376-2394` | `pci_write_config_{byte,word,dword}` (w 1/2/4); status explicitly discarded `(void)…` pending A-11 | NM |
| `MmioBinding` | `device_lifecycle.rs:209-229` | `body_emit.rs:5244-5285` | `effective_bar`→BAR/region index; `base/size` *values*→enumerated resource, *presence* still gates the preamble (`is_resolved()`, A-10) | SPC |
| `BusType` + `is_pci_like` | `device_lifecycle.rs:78-104` | `body_emit.rs:5050-5061` | framework selection + PCI-only gate | NM |
| `BarType::Io` | `device_lifecycle.rs:243-250` | **none** | port-I/O surface (`inb/outb` or `ioread*` over `pci_iomap`) — undesigned | NNS |

## device-lifecycle

| primitive | model_source | generic_c ref | Linux materialization | disp. |
|---|---|---|---|---|
| `register` marker | `device_lifecycle.rs:48-51` | `body_emit.rs:5197-5199` | `name` → driver-struct `.name`; call from BRM selector | NM |
| `MatchEntry` | `device_lifecycle.rs:110-130` | `body_emit.rs:5499-5509` (first-only) | full set → `pci/of/amba_device_id[]` + `MODULE_DEVICE_TABLE` | NM |
| `create` + morph | synthesized; `body_emit.rs:5488-5492` | `body_emit.rs:5206,5184-5189,5305` | `devm_kzalloc`/`alloc_etherdev` (zeroed); `__rseq_dev`=device back-ptr | SPC |
| `associate` | synthesized | `body_emit.rs:5207` | `pci_set_drvdata`/`amba_set_drvdata`/`SET_NETDEV_DEV` | NM |
| `preamble` (acquire) | `device_lifecycle.rs:180,189` | `body_emit.rs:5214-5217` | acquire span of `.probe()` (forward order; per-op call = BRM/reg-access) | SPC |
| `postamble` (release) | `device_lifecycle.rs:180,189` | `body_emit.rs:5403-5406` | reverse release in `.remove()`; **NA** under `devm_*` (auto) | SPC/NA |
| `teardown` present-dispatch | synthesized | `body_emit.rs:5398` | runtime predicate at top of `.remove()` (`pci_device_is_present`) | SPC |
| `on_init` | `device_lifecycle.rs:162` | `body_emit.rs:5311,5318-5319` | author body at end of `.probe()`; `Ok→return 0`/`Err→-errno` | SPC |
| `on_init -> Err` marker | synthesized | `body_emit.rs:5475-5476` | the `if(err) goto err_…;` branch (no standalone artifact) | SPC |
| `on_init_failure` | `device_lifecycle.rs:168` | `body_emit.rs:5421-5483` (Err gate `:5470-5481`) | author cleanup on the `on_init`→Err ladder arm only, before `return -errno` (never on native preamble failure) | SPC |
| `on_deinit` | `device_lifecycle.rs:164` | `body_emit.rs:5321` | author body in `.remove()` (HW reachable); Result discarded | SPC |
| `on_surprise_remove` | `device_lifecycle.rs:166` | `body_emit.rs:5332-5339` | `.remove()` device-gone branch; no HW access | SPC |
| `on_system_shutdown` | `device_lifecycle.rs:170` | `body_emit.rs:5360-5368` | `.shutdown` slot → author body; **omit field when absent** | NM |
| orderly scenario | `backend_options.rs:26` | `body_emit.rs:5317-5322` | `.remove()` success path — *not a separate fn* | SPC |
| surprise scenario | `backend_options.rs:28` | `body_emit.rs:5328-5339` | `.remove()` device-gone path (merges via predicate) | SPC |
| init-fail scenario | `backend_options.rs:30` | `body_emit.rs:5345-5354` | `.probe()` `-errno`; no `.remove()`; self-unwind | SPC |
| shutdown scenario | `backend_options.rs:32` | `body_emit.rs:5360-5368` | `.shutdown` invocation at reboot/poweroff | SPC |
| `ResourceField`/`Mmio` | `device_lifecycle.rs:186-201,253-257` | `body_emit.rs:5159-5164` | `void __iomem *` container member; acquire/release | SPC |
| `UserDataField`/`UserDataKind` | `device_lifecycle.rs:263-279` | **none** (`{0}` incidental) | zeroed alloc satisfies `ZeroInit`/`OptionCopy(None)`; no designed lowering | NNS |
| `ClassTag`/`device_class` | `device_lifecycle.rs:135-144` | **none** | **not lowered** (no `class_register` — deferred to Device Class) | NNS |

## diagnostics-logging

| primitive | model_source | generic_c ref | Linux materialization | disp. |
|---|---|---|---|---|
| `log-debug` | `domain_attr.rs:143,161` | `body_emit.rs:2101-2104,2722` | `dev_dbg(dev, F, A…)` / `pr_debug` (`KERN_DEBUG`) | NM |
| `log-info` | `domain_attr.rs:145,162` | `body_emit.rs:2722` | `dev_info(dev, …)` / `pr_info` (`KERN_INFO`) | NM |
| `log-warn` | `domain_attr.rs:147,163` | `body_emit.rs:2722` | `dev_warn(dev, …)` / `pr_warn` (`KERN_WARNING`) | NM |
| `log-error` | `domain_attr.rs:149,164` | `body_emit.rs:2722` | `dev_err(dev, …)` / `pr_err` (`KERN_ERR`) | NM |
| template reassembly | bridge DiagLogPeel; `parse_template :5903-5950` | `emit_diag_log :2722-2849` | one `printk` fmt `F` + arg list `A` (inverse of fwrite decomposition) | SPC |
| literal segment | `:5891` | `flush_literal`, `c_escape :6020-6037` | append to `F`; **double literal `%`→`%%`** | SPC |
| numeric placeholder | `diag_specifier_and_cast_for :5798-5850` | `snprintf→fwrite :2829-2836` | reuse `(spec,cast)` whole: `%d/%u/%lld/%llu/%x…` + cast | NM |
| `&str` value | fat-ptr / static carrier | `:2812-2817` / `:2794-2802` | `%.*s` + `(int)len,(const char*)data` (interior-NUL gap) | NM |
| `[LEVEL]` text tag | `level_tag :6058-6065` | pushed `:2768-2770` | **dropped** — severity = macro/`KERN_*` + `dev_*` prefix | NA |
| pointers/`{:p}`, `isize`/`usize`, `char` | rejected `:5793,5956-6015` | — | **not lowered** (absent from as-built) | NA |

## RegseqError

| primitive | model_source | generic_c ref | Linux materialization | disp. |
|---|---|---|---|---|
| `FrameworkType::RegseqError` marker | `domain_attr.rs:77-80,68` | `mod.rs:670-679,647-658` | reuse `is_framework_error_enum`/`classify_entry_return` verbatim | SPC |
| `Result<(),RegseqError>` carrier / `Ok` | rustc niche (Ok=0/Err=1) | `body_emit.rs:4654-4707,4819-4821` | same niche read; `Ok(())→return 0;` | SPC |
| exit-adapter | hand-emitted (`:4709-4716`) | `body_emit.rs:4732-4828` | negate arms, relocate to `.probe`/callback return, `<linux/errno.h>` | SPC + shell |
| `InvalidParameter → EINVAL` | `regseq-error-spec.md:221` | `errno.rs:37` | `return -EINVAL;` (22) | NM |
| `OutOfMemory → ENOMEM` | `:222` | `errno.rs:38` | `return -ENOMEM;` (12) | NM |
| `HardwareError → EIO` | `:223` | `errno.rs:39` | `return -EIO;` (5) | NM |
| `Timeout → ETIMEDOUT` | `:224` | `errno.rs:40` | `return -ETIMEDOUT;` (110) | NM |
| `Unsupported → EOPNOTSUPP` | `:225` | `errno.rs:41` | `return -EOPNOTSUPP;` (95, **not** `ENOTSUPP`) | NM |
| `DeviceBusy → EBUSY` | `:226` | `errno.rs:42` | `return -EBUSY;` (16) | NM |
| `PermissionDenied → EPERM` | `:227` | `errno.rs:43` | `return -EPERM;` (1) | NM |
| `DeviceNotFound → ENODEV` | `:228` | `errno.rs:44` | `return -ENODEV;` (19) | NM |
| `GenericFailure → EIO` | `:229` | `errno.rs:45` | `return -EIO;` (5) | NM |
| demand-driven include | — | `prelude.rs:434-438` | `#include <linux/errno.h>` via same machinery | SPC |
| native failure channel | — | preamble `body_emit.rs:5256-5285` | native `-errno` (incl. `-EPROBE_DEFER`) propagates **directly**, never wrapped | SPC |

---

## Disposition tally

| disposition | count (approx.) | notes |
|---|---|---|
| **native-materialization** | the bulk — all five MMIO ops, `map_region`, `register_driver`, config read/write, `associate`, `on_system_shutdown`, all four log levels + numeric/`&str` args, all nine errno rows | direct Linux primitive; reuse arithmetic, replace call target |
| **shared-procedural-core** | the lifecycle scaffold (preamble/postamble/scenarios/markers), `get_resource_handle`, `MmioBinding`, the RegseqError adapter + carrier + marker, template reassembly | reuse the reference *mechanism* (ordering, niche read, format walk) |
| **not-applicable** | `enable_device`/`claim_regions` on platform/DT, `set_bus_master` (dormant), the `[LEVEL]` tag, `{:p}`/`isize`/`char`, `devm_*` postamble | emit nothing — logged in [`linux-model-analysis.md`](./linux-model-analysis.md) §7 |
| **needs-new-surface** | `poll_until`, `BarType::Io`, `UserDataField`, `ClassTag`, config width-8, >1 region, multi-`compatible` | escalated gaps — [`linux-model-analysis.md`](./linux-model-analysis.md) §8 |
