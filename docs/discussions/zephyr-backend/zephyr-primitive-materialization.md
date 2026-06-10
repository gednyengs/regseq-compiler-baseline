# Zephyr primitive materialization — per-primitive table

The concrete Zephyr realization of every primitive in the census
([`../backend-lowering/primitive-inventory.md`](../backend-lowering/primitive-inventory.md)),
one row each: **model surface → `generic_c` reference → Zephyr materialization →
disposition**. This is the quick-reference index; the normative prose is in
[`zephyr-lowering-per-construct.md`](./zephyr-lowering-per-construct.md) and the
full trace (scaffold/error/open columns) is in
[`zephyr-traceability-matrix.md`](./zephyr-traceability-matrix.md).

Dispositions (per [`zephyr-model-analysis.md`](./zephyr-model-analysis.md)):
**NM** native-materialization · **SPC** shared-procedural-core · **NA**
not-applicable · **NNS** needs-new-surface.

Zephyr materializations are pinned to 3.7 LTS / 4.x and are **backend-[Open]**
proposals confirmed against the vendored stock drivers
(`reference-drivers/{pl011,e1000}/zephyr/`) + pinned Zephyr docs/source — never
asserted as shipped. Platform/DT vs PCIe is given as `plat` / `PCIe` where they
differ. `m` = the mapped base lvalue; `OFF` = the recovered offset expression;
`B` = recovered width × 8.

---

## register-access

| primitive | model_source | generic_c ref | Zephyr materialization | disp. |
|---|---|---|---|---|
| `mmio_read` | `domain_attr.rs:125-127,167` | `body_emit.rs:2297-2302` | `dst = (CTy)sys_read{B}(m + OFF);` (width → `sys_read8/16/32/64`) | NM |
| `mmio_read_field` | `domain_attr.rs:128-129,168` | `body_emit.rs:2303-2311` | `dst = (CTy)((sys_read{B}(m+OFF) & (fld).mask) >> (fld).shift);` | NM |
| `mmio_write` | `domain_attr.rs:130-131,169` | `body_emit.rs:2283-2287` | `sys_write{B}((uint{B}_t)(val), m + OFF);` (**value-first**, eth_e1000_priv.h:120) | NM |
| `mmio_write_field` | `domain_attr.rs:132-133,170` | `body_emit.rs:2288-2296` | `sys_write{B}((uint{B}_t)((fv).value & (fv).mask), m + OFF);` (masked, not RMW) | NM |
| `mmio_modify` | `domain_attr.rs:134-135,171` | `body_emit.rs:2312-2323` | `{uint{B}_t cur=sys_read{B}(m+OFF); sys_write{B}((cur&~(fv).mask)\|((fv).value&(fv).mask), m+OFF);}` | NM |
| `poll_until` | `domain_attr.rs:94-95,172` | **none** | *(candidate, non-normative)* `WAIT_FOR(cond, timeout_us, k_busy_wait(delay_us))` | NNS |
| `map_region` | `device_lifecycle.rs:209-229,236-239` | `body_emit.rs:5273-5284` | plat `DEVICE_MMIO_MAP(dev, K_MEM_CACHE_NONE)` (uart_pl011.c:577); PCIe `device_map(&h, mbar.phys_addr, mbar.size, K_MEM_CACHE_NONE)` (eth_e1000.c:223-224) | NM |
| `Mmio<T>` + `#[rseq::register_block]` offset recovery | `device_lifecycle.rs:253-257,186-201` | `recover_register` `body_emit.rs:2439-2530` | **reused unchanged** (emit-time); output = base-relative byte-offset table (the stock idiom, eth_e1000_priv.h:41-61) | SPC |
| `Field`/`FieldValue` descriptors | closed op set `domain_attr.rs:125-136` | `body_emit.rs:2282,2292-2295,2308-2310,2317-2322` | same const aggregates; `.mask/.shift/.value` feed `sys_*` expressions; `FIELD_PREP`/`GENMASK` not adopted (cosmetic only) | SPC |

## bus-resource-management

| primitive | model_source | generic_c ref | Zephyr materialization | disp. |
|---|---|---|---|---|
| `register_driver` | `device_lifecycle.rs:55-61`; selector `body_emit.rs:5050-5061` | `body_emit.rs:5203-5205` | no runtime call — static instantiation family: `DEVICE_DT_INST_DEFINE` + `DT_INST_FOREACH_STATUS_OKAY`; Pci adds `DEVICE_PCIE_INST_DECLARE/INIT`; Amba folds into Platform | NM |
| `enable_device` | `device_lifecycle.rs:96-104` | `body_emit.rs:5256-5259` | PCIe `pcie_set_cmd(bdf, PCIE_CONF_CMDSTAT_MEM, true)` (eth_e1000.c:220-221); plat — *(default-on)* | NM/NA |
| `claim_regions` | `device_lifecycle.rs:209-229` | `body_emit.rs:5260-5266` | **zero calls, both paths** — plat: claim ≡ DT `reg` via `DEVICE_MMIO_ROM_INIT`; PCIe: subsumed into the BAR probe (§B.3.4) ("Zephyr has no runtime claim", prelude.rs:202-208) | NA |
| `set_bus_master` | `device_lifecycle.rs:62-70` | `body_emit.rs:5267-5270` | `pcie_set_cmd(bdf, PCIE_CONF_CMDSTAT_MASTER, true)` — *dormant (`dma_present` false)* | NA |
| `get_resource_handle` | BRM convergence :87,131 (I06) | `body_emit.rs:5271-5272` | PCIe `pcie_get_mbar(bdf, effective_bar, &mbar)` — BAR-number accessor, §B.3.4 (reference ordinal idiom `pcie_probe_mbar`, eth_e1000.c:219); plat **structural** — handle = `dev`, `(base,size)` build-time via `DEVICE_MMIO_ROM_INIT` (uart_pl011.c:759) | SPC |
| `read_config` | `domain_attr.rs:110-113,175` | `body_emit.rs:2364-2375` | `pcie_conf_read(bdf, off >> 2)` + shift/mask for w 1/2 (word-granular); value-returning — no out-param marshal | NM |
| `write_config` | `domain_attr.rs:114-117,176` | `body_emit.rs:2376-2394` | `pcie_conf_write(bdf, off >> 2, val)` for w 4; w 1/2 word-RMW (needs decision); w 8 two words or reject | NM |
| `MmioBinding` | `device_lifecycle.rs:203-229,236-239` | `body_emit.rs:5244-5284` | role preserved; `base/size` from `pcie_get_mbar` (PCIe) / DT `reg` (plat); `effective_bar` (a BAR **number**) → `pcie_get_mbar`'s bar-index arg, NOT `pcie_probe_mbar`'s memory-BAR ordinal (§B.3.4, C-1) | SPC |
| `BarType::Mem` | `device_lifecycle.rs:244-247` | `body_emit.rs:5273-5284` | `PCIE_CONF_CMDSTAT_MEM` decode + mem-BAR probe/map; plat `DEVICE_MMIO_*` | NM |
| `BarType::Io` | `device_lifecycle.rs:248-249` | **none** | port-I/O surface (x86 `sys_in*/sys_out*` + io-BAR) — uncited, undesigned, do not invent | NNS |
| `BusType` + `is_pci_like` | `device_lifecycle.rs:79-103` | `body_emit.rs:5050-5061` | macro-family selection + unchanged PCI gate; **Amba ≡ Platform**; I2c/Spi/Usb deferred; Other rejected upstream | NM |

## device-lifecycle

| primitive | model_source | generic_c ref | Zephyr materialization | disp. |
|---|---|---|---|---|
| `register` marker | `device_lifecycle.rs:47-52` | `body_emit.rs:5196-5199` | build time: `DT_DRV_COMPAT` + binding yaml + `DT_INST_FOREACH_STATUS_OKAY` → `DEVICE_DT_INST_DEFINE` (uart_pl011.c:9,789-793) | NM |
| `MatchEntry::Devicetree` | `device_lifecycle.rs:112` | `body_emit.rs:5501-5503` (first-only) | binding `compatible:` + `DT_DRV_COMPAT`; full set → one block per compatible (uart_pl011.c:795-829) | NM |
| `MatchEntry::Pci` | `device_lifecycle.rs:114-119` | `body_emit.rs:5504` | DT-mediated: `include: pcie-device.yaml` + `DEVICE_PCIE_INST_DECLARE/INIT` + BDF guard (eth_e1000.c:313-318,215-217) | NM |
| `MatchEntry::Amba` | `device_lifecycle.rs:121` | `body_emit.rs:5505` | **no Zephyr carrier** (no periphid bus); compatible-only matching | NNS |
| `MatchEntry::Acpi/Usb/I2c/Spi` | `device_lifecycle.rs:122-129` | `body_emit.rs:5506` (`"?"`) | n/a — bridge-rejected at parse (CR-5) | NA |
| `create` + morph | `device_lifecycle.rs:176-183` | `body_emit.rs:5206,5174-5189,5287-5306` | static per-instance data+config objects, build-time init, zero-filled remainder (uart_pl011.c:758-787; eth_e1000.c:316-318) | SPC |
| `associate` | synthesized | `body_emit.rs:5207` | `&data`/`&config` args of `DEVICE_DT_INST_DEFINE`; read-back `dev->data`/`dev->config` | NM |
| `preamble` (acquire) | `device_lifecycle.rs:178-187` | `body_emit.rs:5209-5217` | acquisition span at top of the init fn (forward order; per-op call = BRM/reg-access) | SPC |
| `postamble` (release) | `device_lifecycle.rs:178-182` | `body_emit.rs:5391-5406` | **not lowered on baseline** — no teardown slot; mappings permanent | NA |
| `teardown` present-dispatch | synthesized | `body_emit.rs:5398` | **no carrier** — static DT devices cannot be removed | NA |
| `on_init` | `device_lifecycle.rs:162-163` | `body_emit.rs:5311,5318-5319` | author body inlined in the init fn after the map; `Ok→return 0` / `Err→-errno` | SPC |
| `on_deinit` | `device_lifecycle.rs:164-165` | `body_emit.rs:5315-5322` | **recognized-not-lowered** on 3.7 baseline; 4.x `device_deinit()` fork = Q1 | NNS |
| `on_surprise_remove` | `device_lifecycle.rs:166` | `body_emit.rs:5323-5339` | **no Zephyr analog** (no hot-unplug for static DT devices) | NNS |
| `on_init_failure` | `device_lifecycle.rs:167-168` | `body_emit.rs:5340-5354,5409-5483` | author cleanup inlined on the init-fn error path before `return -errno` | SPC |
| `on_system_shutdown` | `device_lifecycle.rs:169-170` | `body_emit.rs:5355-5366` | PM-device slot: `PM_DEVICE_DT_INST_DEFINE/GET` when overridden, `NULL` when absent (uart_pl011.c:106-117,771,789); generated `pm_device_action` dispatch wrapper, `-ENOTSUP` on unchosen actions; **trigger semantics unresolved (Q3)** | NM (slot) / open (trigger, Q3) |
| orderly scenario | `backend_options.rs:25-26` | `body_emit.rs:5315-5322` | boot-init success; orderly *teardown* has no baseline path | SPC |
| surprise scenario | `backend_options.rs:27-28` | `body_emit.rs:5323-5339` | no Zephyr path (no removal) | NA |
| init-fail scenario | `backend_options.rs:29-30` | `body_emit.rs:5340-5354,5409-5479` | init fn returns negative errno; device permanently not-ready | SPC |
| shutdown scenario | `backend_options.rs:31-32` | `body_emit.rs:5355-5366` | system power-off → PM-device action path | SPC |
| container morph (Policy-P) | `body_emit.rs:5174-5189` | `body_emit.rs:5184-5189,5287-5306` | `inner` → data struct; `__rseq_dev` **inverted** (callbacks get `const struct device*`, container via `dev->data`); `__rseq_state` → plain data member | SPC |
| `ResourceField`/`Mmio` | `device_lifecycle.rs:186-201,253-257` | `body_emit.rs:5159-5164` | `DEVICE_MMIO_RAM` / `mm_reg_t address` member; acquired at init, never released | SPC |
| `UserDataField`/`UserDataKind` | `device_lifecycle.rs:262-279` | **none** (`{0}` incidental) | C static-storage zero image (partial designated init zero-fills); inherited, not designed | NNS |
| `ClassTag`/`device_class` | `device_lifecycle.rs:53-54,134-150` | **none** | **not lowered**; build-now class-neutral `DEVICE_DT_INST_DEFINE` + NULL api for `Generic` **only** — non-Generic tags = emit-time diagnostic (§C.9; class wrappers = Device Class construct) | NNS |
| lifecycle runner + entry shim | `device_lifecycle.rs:27-31` | `body_emit.rs:4975-4979,5376-5380`; `naming.rs:73-103` | **no artifact** — kernel init-level walk is the runner; Zephyr owns `main` | NA |

## diagnostics-logging

| primitive | model_source | generic_c ref | Zephyr materialization | disp. |
|---|---|---|---|---|
| `log-debug` | `domain_attr.rs:141-143,161` | `body_emit.rs:2100-2104,2722-2849` | `LOG_DBG("<F>", <A…>)` (eth_e1000.c:121,284) | NM |
| `log-info` | `domain_attr.rs:144-145,162` | same | `LOG_INF("<F>", <A…>)` | NM |
| `log-warn` | `domain_attr.rs:146-147,163` | same | `LOG_WRN("<F>", <A…>)` | NM |
| `log-error` | `domain_attr.rs:148-149,164` | same | `LOG_ERR("<F>", <A…>)` (eth_e1000.c:67,154,203) | NM |
| template reassembly | Pipeline-A flat args `body_emit.rs:2712-2715` | `parse_template :5903-5950`; fwrite decomposition `:2752-2843` | one cbprintf fmt `F` + arg list `A` (inverse of fwrite decomposition); no trailing `\n` | SPC |
| literal segment | — | `c_escape :6020-6037` | append to `F`; **double literal `%`→`%%`** (gc never doubles, `:6017-6022`) | SPC |
| numeric placeholder | `diag_specifier_and_cast_for :5798-5850` | `snprintf→fwrite :2819-2836` | reuse `(spec,cast)` whole; cbprintf renders directly; `%lld/%llu` need `CONFIG_CBPRINTF_FULL_INTEGRAL` | NM |
| static `&str` value | const carrier `:2873-2918` | `:2794-2802` | NUL-free → C string literal via `%s` (rodata, packager keeps pointer); interior NUL → fail loud / hexdump (O2) | NM |
| non-static `&str` value | `rust_str` `prelude.rs:39-44` | `:2812-2817` | bounce buffer + NUL + `%s` (deferred-mode packaging safety; snprintk idiom eth_e1000.c:29-37); truncation divergences documented (O1) | NM |
| `[LEVEL]` text tag | `level_tag :6058-6065` | pushed `:2767-2770` | **dropped** — severity + module rendered structurally by the log core | NA |
| TU logging scaffold | — (stdio needs none) | demand-driven discipline `prelude.rs:345-350` | `#define LOG_MODULE_NAME <sanitize(name)>` + `#include <zephyr/logging/log.h>` + `LOG_MODULE_REGISTER` iff ≥1 log site (eth_e1000.c:9-12) | NM |
| pointers/`{:p}`, `isize`/`usize`, `char` | rejected `:5952-5955,6008-6012,5790-5793` | — | **not lowered** (absent from as-built) | NA |

## RegseqError

| primitive | model_source | generic_c ref | Zephyr materialization | disp. |
|---|---|---|---|---|
| `FrameworkType::RegseqError` marker | `domain_attr.rs:62-68,75-80` | `mod.rs:670-679,565-660` | reuse `is_framework_error_enum`/`classify_entry_return` verbatim | SPC |
| `Result<(),RegseqError>` carrier / `Ok` | rustc niche (Ok=0/Err=1) | `body_emit.rs:4819-4821` | same niche read; `Ok(())→return 0;` (= Zephyr init contract, eth_e1000.c:261) | SPC |
| exit-adapter | `mod.rs:521-534` | `body_emit.rs:4709-4828` | negate arms; fuse into the init-fn return path (lifecycle TU) / keep adapter called from `main` (standalone); `<errno.h>` **unchanged** | SPC + shell |
| `InvalidParameter → EINVAL` | `regseq-error-spec.md:221-229` | `errno.rs:37` | `return -EINVAL;` (idiom uart_pl011.c:173) | NM |
| `OutOfMemory → ENOMEM` | `:222` | `errno.rs:38` | `return -ENOMEM;` | NM |
| `HardwareError → EIO` | `:221-229` | `errno.rs:39` | `return -EIO;` (idiom eth_e1000.c:132) | NM |
| `Timeout → ETIMEDOUT` | `:221-229` | `errno.rs:40` | `return -ETIMEDOUT;` | NM |
| `Unsupported → EOPNOTSUPP` | `:221-229` | `errno.rs:41` | `return -EOPNOTSUPP;` (vs Zephyr idiom `-ENOTSUP` — ZE-OQ1) | NM |
| `DeviceBusy → EBUSY` | `:221-229` | `errno.rs:42` | `return -EBUSY;` | NM |
| `PermissionDenied → EPERM` | `:221-229` | `errno.rs:43` | `return -EPERM;` | NM |
| `DeviceNotFound → ENODEV` | `:221-229` | `errno.rs:44` | `return -ENODEV;` (native idiom eth_e1000.c:216 is the *other* channel) | NM |
| `GenericFailure → EIO` | `:229` | `errno.rs:45` | `return -EIO;` (also the negated defensive `default:`) | NM |
| demand-driven include | — | `prelude.rs:345-350,434-439` | `#include <errno.h>` via the same machinery — **header name unchanged** | SPC |
| native failure channel | — | preamble `body_emit.rs:5256-5285` | native negative errno propagates **directly**, never wrapped; no `-EPROBE_DEFER` analogue | SPC |

## cross-construct

| primitive | model_source | generic_c ref | Zephyr materialization | disp. |
|---|---|---|---|---|
| `TargetOs::Zephyr` | `target.rs:69-74,91-97` | carried, not acted on (`target.rs:108-109`) | the framework-selection token this backend acts on | NM |
| entry-shim classification | `mod.rs:432-540` | `naming.rs:73-103,146-156,181-182` | classification reused; Zephyr `main` shape; kernel ignores `main`'s rc (ZE-OQ3) | SPC + shell |

---

## Disposition tally

| disposition | members | notes |
|---|---|---|
| **native-materialization** | the five MMIO ops, `map_region`, `register_driver`, `enable_device` (PCIe), config read/write, `BarType::Mem`, `BusType` selection, `register`/`MatchEntry::{Devicetree,Pci}`/`associate`, `on_system_shutdown` (**slot only** — the trigger contract is open, Q3), all four log levels + numeric/`&str` args + the TU log scaffold, all nine errno rows, `TargetOs::Zephyr` | direct Zephyr primitive (or build-time macro family); reuse arithmetic, replace the target |
| **shared-procedural-core** | offset recovery, `Field`/`FieldValue`, `get_resource_handle`, `MmioBinding`, the lifecycle bring-up scaffold (create/morph/preamble/on_init/init-fail/scenario collapse), `ResourceField`, template reassembly, the RegseqError marker + carrier + adapter mechanism + native-channel rule | reuse the reference *mechanism* (ordering, niche read, format walk) |
| **not-applicable** | `claim_regions` (both paths), `enable_device` (plat), `set_bus_master` (dormant), teardown dispatch/postamble/surprise (baseline), runner + entry shim, `MatchEntry::Acpi/Usb/I2c/Spi`, `[LEVEL]` tag, `{:p}`/`isize`/`char` | emit nothing — logged in [`zephyr-model-analysis.md`](./zephyr-model-analysis.md) §7 |
| **needs-new-surface** | `poll_until`, `BarType::Io`, config width-8 / sub-word write RMW, `MatchEntry::Amba`, `on_deinit` (4.x fork), `on_surprise_remove`, `UserDataField`, `ClassTag`, >1 region | escalated gaps — [`zephyr-model-analysis.md`](./zephyr-model-analysis.md) §8 |
