# Zephyr model analysis — the whole-model sweep

This is the Zephyr execution of the per-OS sweep method
([`../backend-lowering/model-analysis-strategy.md`](../backend-lowering/model-analysis-strategy.md)).
It walks the primitive census
([`../backend-lowering/primitive-inventory.md`](../backend-lowering/primitive-inventory.md))
one row at a time and assigns each primitive exactly one of the four
dispositions for **Zephyr 3.7 LTS / 4.x**, with a citation. Nothing is left as a
`?`; every `not-applicable` is logged (§7) so a dropped op cannot masquerade as
coverage, and every `needs-new-surface` is filed as a blocking gap (§8), not
papered over.

**The four dispositions** (from the strategy doc §*The four dispositions*):
**native-materialization** (a direct Zephyr primitive — keep `generic_c`'s
recognition + arithmetic, replace the call target); **shared-procedural-core**
(no single primitive — reuse the reference *mechanism* verbatim, e.g. ordering,
the niche read); **not-applicable** (does not exist / no-op / gated off — emit
nothing, log it); **needs-new-surface** (the model or reference carries nothing
to lower — escalate, do not invent).

**The decisive sub-pin: platform/DT vs PCIe — and the PCI question is now
answered.** The strategy doc deferred Zephyr-over-PCI as an open question
(model-analysis-strategy.md:158-162). The vendored e1000 port **resolves it
affirmatively**: Zephyr has a first-class PCIe host layer
(`#include <zephyr/drivers/pcie/pcie.h>`, reference-drivers/e1000/zephyr/eth_e1000.c:18)
and a complete bring-up idiom (eth_e1000.c:208-262), so `BusType::Pci` is
lowerable on Zephyr. Both paths share the same build-time instantiation
(`DEVICE_DT_INST_DEFINE`); they diverge in the resource preamble — platform/DT
captures `(base,size)` at build time (`DEVICE_MMIO_ROM_INIT`, uart_pl011.c:759)
while PCIe resolves it at init time (reference idiom `pcie_probe_mbar`,
eth_e1000.c:219; generated code uses `pcie_get_mbar`, per-construct §B.3.4). A
second Zephyr-only collapse: **`Amba` folds into `Platform`** — Zephyr has no
AMBA/periphid bus framework; the PL011 (an ARM PrimeCell) is matched purely by
DT compatible (`DT_DRV_COMPAT arm_pl011`, uart_pl011.c:9).

**The structural pivot that colors every lifecycle row:** Zephyr has no runtime
registration at all. Registration, matching, creation, and association are
**build-time** facts (static instantiation per status-okay DT node,
uart_pl011.c:789-793); the kernel's init-level walk at boot is the runner; and
the teardown half of the lifecycle has **no carrier** on the 3.7-LTS baseline
(no remove/unbind slot exists, uart_pl011.c:789-791, eth_e1000.c:337-344).

---

## 1. The sweep table (primitive × Zephyr → disposition + citation)

### register-access

| primitive | Zephyr disposition | citation |
|---|---|---|
| `mmio_read` (`MmioOp::Read`) | **native-materialization** → `sys_read{8,16,32,64}(m + OFF)` by recovered width | `body_emit.rs:2297-2302`; width `:2514-2520`; in-repo witness eth_e1000_priv.h:123-128; pinned `v3.7-branch include/zephyr/sys/sys_io.h` |
| `mmio_read_field` (`MmioOp::ReadField`) | **native-materialization** → `(sys_read{B}(m+OFF) & M) >> S` | `body_emit.rs:2303-2311` (mask/shift arithmetic = core, kept) |
| `mmio_write` (`MmioOp::Write`) | **native-materialization** → `sys_write{B}(val, m+OFF)` (**value-first** arg order) | `body_emit.rs:2283-2287`; arg order eth_e1000_priv.h:120 |
| `mmio_write_field` (`MmioOp::WriteField`) | **native-materialization** → `sys_write{B}((V & M), m+OFF)` (masked whole-reg, not RMW) | `body_emit.rs:2288-2296`; register-access-surface-convergence.md §2.4 |
| `mmio_modify` (`MmioOp::Modify`) | **native-materialization** → `cur=sys_read{B}; sys_write{B}((cur&~M)\|(V&M))` (compiler-emitted RMW) | `body_emit.rs:2312-2323`; open-coded RMW idiom uart_pl011.c:121 |
| `poll_until` (`IntrinsicKind::PollUntil`) | **needs-new-surface** — `generic_c` lowers nothing; candidate `WAIT_FOR(...)` is non-normative | `domain_attr.rs:94-95, :172`; convergence §9 "do not claim it"; candidate `v3.7-branch include/zephyr/sys/util.h` |
| `map_region` (keystone) | **native-materialization** (fed BRM `(base,size)`) → platform `DEVICE_MMIO_MAP(dev, K_MEM_CACHE_NONE)`; PCIe `device_map(&h, mbar.phys_addr, mbar.size, K_MEM_CACHE_NONE)` | `body_emit.rs:5273-5284`; uart_pl011.c:577; eth_e1000.c:223-224; must stay after `get_resource_handle` (I06) |
| `Mmio<T>` + `#[rseq::register_block]` offset recovery | **shared-procedural-core** — emit-time mechanism reused unchanged; result = the stock Zephyr byte-offset register table | `recover_register` `body_emit.rs:2439-2530`; offset-table model eth_e1000_priv.h:41-61 |
| `Field`/`FieldValue` descriptors | **shared-procedural-core** — same const aggregates; `.mask/.shift/.value` feed the `sys_*` expressions unchanged | `body_emit.rs:2282, :2292-2295, :2308-2310, :2317-2322`; `FIELD_PREP`/`GENMASK` noted cosmetic-only (uart_pl011.c:630-631) |

### bus-resource-management

| primitive | Zephyr disposition | citation |
|---|---|---|
| `register_driver` (bus selector) | **native-materialization** → no runtime call; selects the static instantiation family: `DEVICE_DT_INST_DEFINE` + `DT_INST_FOREACH_STATUS_OKAY` (Platform/Amba/Pci, Pci adding `DEVICE_PCIE_INST_DECLARE/INIT`) | selector `body_emit.rs:5050-5061`; uart_pl011.c:789-793; eth_e1000.c:313-318, :337-346 |
| `enable_device` | **native-materialization** (PCIe) → `pcie_set_cmd(bdf, PCIE_CONF_CMDSTAT_MEM, true)`; **not-applicable** (platform/Amba, default-on) | gate `body_emit.rs:5256-5259`; eth_e1000.c:220-221; prelude.rs:196-197 |
| `claim_regions` | **not-applicable** — **zero calls on both paths** ("Zephyr has no runtime claim", the as-built contract's own words); platform claim ≡ DT `reg` via `DEVICE_MMIO_ROM_INIT`; PCIe subsumed into the §B.3.4 `pcie_get_mbar` resolution (reference idiom: `pcie_probe_mbar`) | prelude.rs:202-208; `body_emit.rs:5260-5266`; uart_pl011.c:759; eth_e1000.c:219; resolves the backend-[Open] at bus-resource-management-surface-convergence.md:133-135 |
| `set_bus_master` | **not-applicable today** (dormant; target `pcie_set_cmd(bdf, PCIE_CONF_CMDSTAT_MASTER, true)` identified) | gate `body_emit.rs:5267-5270`; `dma_present` placeholder `device_lifecycle.rs:62-70`; eth_e1000.c:220-221 |
| `get_resource_handle` | **shared-procedural-core** (seam anchor) → PCIe runtime `pcie_get_mbar(bdf, effective_bar, &mbar)` (BAR-number accessor, per-construct §B.3.4; reference ordinal idiom `pcie_probe_mbar`); platform **structural** (handle = `dev`; `(base,size)` captured at build by `DEVICE_MMIO_ROM_INIT`) | `body_emit.rs:5271-5272`; eth_e1000.c:213, :219; uart_pl011.c:759 |
| `read_config` (`ConfigOp::Read`) | **native-materialization** (PCIe) → `pcie_conf_read(bdf, off >> 2)` + shift/mask for widths 1/2; value-returning (no Linux out-param inversion) | `body_emit.rs:2364-2375`; pinned `v3.7.0 include/zephyr/drivers/pcie/pcie.h` (fetch-unverified, BRM-O1); header included eth_e1000.c:18 |
| `write_config` (`ConfigOp::Write`) | **native-materialization** (PCIe, width 4); widths 1/2 need word-RMW (**needs explicit decision** — bus-observable); width 8 = two words or reject | `body_emit.rs:2376-2394`; same pinned header |
| `MmioBinding` | **shared-procedural-core** — role kept, `base/size` values replaced by enumeration (I04 swap) | `device_lifecycle.rs:203-229, :236-239`; consumed `body_emit.rs:5244-5284` |
| `BusType` + `is_pci_like` | **native-materialization** — drives macro-family selection + unchanged PCI gate; **Amba ≡ Platform** at the framework layer | `body_emit.rs:5050-5061`; gate `device_lifecycle.rs:101-103`; uart_pl011.c:9 + arm_pl011.yaml |
| `BarType::Mem` | **native-materialization** — `PCIE_CONF_CMDSTAT_MEM` decode + mem-BAR probe/map; platform `DEVICE_MMIO_*` path | `device_lifecycle.rs:244-247`; eth_e1000.c:219-224; uart_pl011.c:43, :577 |
| `BarType::Io` | **needs-new-surface** — stays `recognized_not_lowered`; no in-repo Zephyr port-I/O witness; do not invent | `device_lifecycle.rs:248-249`; same model-side Mem/Io unification block as Linux (linux-lowering-per-construct.md:385-390) |

### device-lifecycle

| primitive | Zephyr disposition | citation |
|---|---|---|
| `register` marker | **native-materialization** → build time: `DT_DRV_COMPAT` + binding yaml + `DT_INST_FOREACH_STATUS_OKAY` → `DEVICE_DT_INST_DEFINE` | `body_emit.rs:5196-5199`; uart_pl011.c:9, :789-793; eth_e1000.c:7, :337-346 |
| `MatchEntry::Devicetree` | **native-materialization** → binding `compatible:` + `DT_DRV_COMPAT`; multi-entry → one instantiation block per compatible (stronger than `generic_c`'s first-only render) | `device_lifecycle.rs:112`; arm_pl011.yaml:3; uart_pl011.c:9-10, :795-829 |
| `MatchEntry::Pci` | **native-materialization** → DT-mediated PCIe: `include: pcie-device.yaml` + `DEVICE_PCIE_INST_DECLARE/INIT` + runtime BDF presence guard | `device_lifecycle.rs:114-119`; intel,e1000.yaml:8; eth_e1000.c:313-318, :215-217 |
| `MatchEntry::Amba` | **needs-new-surface** — no Zephyr AMBA/periphid bus; an Amba-only match set cannot lower (Q4) | `device_lifecycle.rs:121`; PL011 matches by compatible only, uart_pl011.c:9 |
| `MatchEntry::Acpi/Usb/I2c/Spi` | **not-applicable** — bridge-rejected at parse (CR-5); never reach a backend | `device_lifecycle.rs:122-129`; device-lifecycle-surface-convergence.md §1.1 |
| `create` + container morph | **shared-procedural-core** → static per-instance data+config objects, build-time initialized, zero-filled remainder | `body_emit.rs:5206, :5174-5189, :5287-5306`; uart_pl011.c:758-787; eth_e1000.c:316-318, :333-335 |
| `associate` | **native-materialization** → the `&data`/`&config` arguments of `DEVICE_DT_INST_DEFINE`; read-back `dev->data`/`dev->config` | `body_emit.rs:5207`; uart_pl011.c:789-791, :572-573; eth_e1000.c:211, :340-341 |
| `preamble` (forward) | **shared-procedural-core** — owns the slot + forward order at the top of the init fn; per-op call is BRM/reg-access | `body_emit.rs:5209-5217` + BRM block `:5244-5285`; eth_e1000.c:208-224; uart_pl011.c:577-609 |
| `postamble` (reverse) | **not-applicable on baseline** — no teardown slot exists; mappings never unmapped (follows Q1 if a carrier is adopted) | `body_emit.rs:5391-5406`; no teardown in uart_pl011.c:789-791 / eth_e1000.c:337-344 |
| `teardown` present-dispatch | **not-applicable on baseline** — static DT devices cannot be removed | `body_emit.rs:5398`; Q1/Q2 |
| `on_init` | **shared-procedural-core** → author body inlined in the init fn after the map; `Ok→return 0` / `Err→-errno` | `body_emit.rs:5311, :5318-5319`; uart_pl011.c:582-661; eth_e1000.c:226-261 |
| `on_deinit` | **needs-new-surface on 3.7 baseline** — recognized-not-lowered (no removal in the classic device model); 4.x `device_deinit()` fork = Q1 | `device_lifecycle.rs:164-165`; both vendored drivers teardown-free |
| `on_surprise_remove` | **needs-new-surface** — no Zephyr analog at all (no hot-unplug for static DT devices); Q2 | `device_lifecycle.rs:166` |
| `on_init_failure` | **shared-procedural-core** → author cleanup inlined on the init-fn error path before `return <negative errno>` | `body_emit.rs:5340-5354, :5409-5483`; pattern uart_pl011.c:584-588 |
| `on_system_shutdown` | **native-materialization (slot only — trigger contract open, Q3)** (omit-when-absent) → the PM-device argument of `DEVICE_DT_INST_DEFINE`: `PM_DEVICE_DT_INST_DEFINE/GET` + generated `pm_device_action` dispatch wrapper (`-ENOTSUP` on unchosen actions) when overridden, `NULL` when absent; **not a claimed materialization until Q3 closes** | `body_emit.rs:5355-5366`; uart_pl011.c:106-117, :771, :789; eth_e1000.c:339; trigger semantics = Q3, interim policy A-18 |
| four scenarios (Orderly/Surprise/InitFail/Shutdown) | **shared-procedural-core** — *not* four functions; collapse to the kernel's natural paths (boot-init success/failure, power-off, nothing) | `backend_options.rs:24-42`; emitted scaffold is scenario-independent |
| `ResourceField`/`ResourceKind::Mmio` | **shared-procedural-core** → `DEVICE_MMIO_RAM`/`mm_reg_t` member acquired in the init fn, never released | `device_lifecycle.rs:186-201, :253-257`; uart_pl011.c:64; eth_e1000_priv.h:87 |
| `UserDataField`/`UserDataKind` | **needs-new-surface** — no designed lowering; C static-storage zero image inherits the same incidental guarantee as `generic_c`'s `{0}` | `device_lifecycle.rs:262-279`; `body_emit.rs:5301-5306`; partial designated init eth_e1000.c:316-318 |
| `ClassTag`/`device_class` | **needs-new-surface** — recognized-not-lowered; on Zephyr the class is *visibly* load-bearing (macro family + API vtable selection), which is exactly the deferred Device Class territory | `device_lifecycle.rs:53-54, :134-150`; uart_pl011.c:534-542 vs eth_e1000.c:296-306, :337-344 |
| lifecycle runner + entry shim | **not-applicable** — Zephyr owns `main`; the kernel's init-level walk is the runner; no artifact emitted | `body_emit.rs:4975-4979, :5376-5380`; `naming.rs:73-103`; `mod.rs:436-460` |

### diagnostics-logging

| primitive | Zephyr disposition | citation |
|---|---|---|
| `log-debug`/`log-info`/`log-warn`/`log-error` | **native-materialization** → `LOG_DBG/LOG_INF/LOG_WRN/LOG_ERR`; level → macro (structural, not text) | `body_emit.rs:2100-2104, :2722`; eth_e1000.c:67, :121, :154, :203, :284 |
| template/format reassembly | **shared-procedural-core** — reuse `parse_template` + `diag_specifier_and_cast_for` whole; reassemble one cbprintf fmt (inverse of the fwrite decomposition); double literal `%`→`%%`; no trailing `\n` | `body_emit.rs:5903-5950, :5798-5850, :6017-6022`; newline-free idiom eth_e1000.c:67-284 |
| numeric arguments | **native-materialization** — `(spec,cast)` pairs carry over unchanged (cbprintf is C99-printf-compatible) | `body_emit.rs:5798-5850`; hex idiom eth_e1000_priv.h:119, :126; pinned formatted_output docs 3.7.0 |
| static-literal `&str` argument | **native-materialization** → NUL-free bytes as a C string literal via `%s`; interior NUL = fail loud or hexdump fallback (LOG-O2) | `body_emit.rs:2794-2802, :2873-2918`; hexdump witness eth_e1000.c:29-37 |
| non-static `&str` argument | **native-materialization with a documented gap** → call-site bounce buffer + NUL + `%s` (deferred-mode packager copies transient strings NUL-terminated; raw `%.*s` on a fat pointer risks over-read) — LOG-O1 | `prelude.rs:39-44`; `body_emit.rs:2812-2817`; snprintk idiom eth_e1000.c:29-37; pinned logging docs 3.7.0 |
| `[LEVEL]` text tag | **not-applicable** — dropped; severity + module identity rendered structurally by the log core | `level_tag` `body_emit.rs:6058-6065`; pinned logging docs 3.7.0 |
| TU logging scaffold | **native-materialization** (no `generic_c` analogue) → `LOG_MODULE_REGISTER` block, demand-driven, module name ← sanitized `DriverDescriptor.name` | eth_e1000.c:9-12; `device_lifecycle.rs:47-52`; demand-driven discipline `prelude.rs:345-350` |
| pointers/`{:p}`, `isize`/`usize`, `char`, source-context | **not-applicable** (absent from the as-built surface — not lowered, do not invent) | rejected `body_emit.rs:5952-5955, :6008-6012, :5790-5793, :5809-5813` |

### RegseqError

| primitive | Zephyr disposition | citation |
|---|---|---|
| `FrameworkType::RegseqError` marker | **shared-procedural-core** — reuse `is_framework_error_enum`/`classify_entry_return` verbatim | `mod.rs:670-679, :565-660` |
| `Result<(),RegseqError>` carrier / `Ok` channel | **shared-procedural-core** — same niche read; `Ok(())→return 0;` byte-identical (matches the Zephyr init contract) | `body_emit.rs:4819-4821`; eth_e1000.c:261 |
| `RegseqError → errno` exit-adapter | **shared-procedural-core** (read+switch) + **shell** (return form) → negate arms, fuse into the init-fn return path; `<errno.h>` include name **unchanged** (a non-delta vs Linux) | `body_emit.rs:4709-4828`; `prelude.rs:345-350, :434-439`; minimal-libc errno.h pinned v3.7.0 |
| nine `RegseqError::* → errno` rows | **native-materialization** (table values) — **symbol-identical** to `generic_c`; only polarity changes (`return -EINVAL;`) | `errno.rs:35-48`; negative-errno convention pinned (design_guidelines); idiom uart_pl011.c:173, eth_e1000.c:216 |
| native operational failure channel | **shared-procedural-core** — native negative errno propagates **directly**, never wrapped; no `-EPROBE_DEFER` analogue exists on Zephyr | strategy doc §Native vs authored; eth_e1000.c:215-217; uart_pl011.c:584-586 |

### cross-construct

| primitive | Zephyr disposition | citation |
|---|---|---|
| `TargetOs::Zephyr` (closed vocabulary) | **native-materialization** — the token this backend acts on (framework selection); `generic_c` carries but ignores it | `target.rs:69-74, :91-97, :108-109` |
| entry-shim classification (UnitVoid/ErrResult/LifecycleOnly) | **shared-procedural-core** (classification) + **shell** (Zephyr `main` shape); the kernel ignores `main`'s return value → rc observability is ZE-OQ3 | `mod.rs:432-540`; `naming.rs:73-103, :146-156, :181-182`; system_threads docs (pinned) |

---

## 2. register-access sweep notes

The op algebra is **backend-invariant**: offset recovery (`recover_register`,
`body_emit.rs:2439-2530`), the mask/shift/value descriptor reads, the
masked-write confinement, and the read-clear-OR RMW are all computed before any
accessor is named (`body_emit.rs:2271-2281`). Zephyr differs in exactly three
places — the **accessor** (host hooks → `sys_read*/sys_write*`, with the
value-first write argument order, eth_e1000_priv.h:120), the **base carrier**
(TU-global `regseq_bus_mapped_base` → `mm_reg_t` from the `DEVICE_MMIO`
scaffold or a `device_map`-filled handle field), and **`map_region`**
(`regseq_bus_map_region` → `DEVICE_MMIO_MAP` / `device_map`, both with the
witnessed `K_MEM_CACHE_NONE`). That three-point swap is sufficient *because of*
the construct's own keystone invariant I04 (`prelude.rs:158-162`): changing the
`(base,size)` source changes no op. The result of offset recovery — a register
table as base-relative byte offsets — is byte-for-byte the dominant Zephyr MMIO
idiom (`eth_e1000_priv.h:41-61, :118-128`). One honest gap inherited from the
model, not the target: no memory-barrier construct exists (`ordered!`
deferred), so the stock PL011's explicit `barrier_dmem_fence_full()`
(uart_pl011.c:179, :644-647) is outside what the construct can express.

## 3. bus-resource-management sweep notes

The structural pivot: `generic_c`'s one flat scenario function splits along a
**different seam than Linux**. Linux lifts `register_driver` to runtime module
init; Zephyr has **no runtime registration at all** — drivers are statically
instantiated device objects, one per enabled DT node, and everything from
`enable_device` through `map_region` and `on_init` becomes the body of the
per-instance **init function** the kernel calls at boot. The gates carry over
unchanged (`is_pci_like`, `dma_present`, the ≥1-resolved-binding block gate,
the single-region guard — which **matches** the non-named `DEVICE_MMIO` API
exactly). Two ops become structural: `claim_regions` emits **zero calls on
both paths** (the as-built contract names this realization verbatim,
prelude.rs:202-208), and platform-path `get_resource_handle` is satisfied by
the build-time `DEVICE_MMIO_ROM_INIT` capture. One ordering wrinkle is flagged,
not silently resolved: the Zephyr PCIe reference probes the BAR *before*
enabling memory decode (eth_e1000.c:219-221), inverting the canonical
`enable → handle` order — recommended as a ratified per-OS deviation (BRM-O2).
Config space inverts Linux's marshalling problem: Zephyr exposes only a
**word-granular** 32-bit accessor pair, so widths 1/2 marshal by
containing-word shift/mask (and write-side word-RMW needs an explicit
decision — a config-space RMW is bus-observable), while the return shape
matches the model directly (no status-out-param inversion).

## 4. device-lifecycle sweep notes

The structural shift, twice over: `generic_c` *synthesizes the OS* (a driving
`main` + per-scenario functions); Linux redistributes the canonical sequence
across runtime callbacks; Zephyr hoists **registration, matching, creation,
and association to build time** (macro expansion + devicetree compile) and
leaves only the init function as a runtime lifecycle artifact. The four
`LifecycleScenario`s are not an emission input on Zephyr: the emitted scaffold
is scenario-independent, and the scenarios collapse to the kernel's natural
paths (boot-init success, boot-init failure, power-off, nothing). The
teardown half of the construct **largely has no carrier** — orderly/surprise
teardown does not lower on the 3.7-LTS baseline (no remove/unbind slot exists;
this is a cited property of the device model, not a spec gap), `on_deinit`
hangs on the version-sensitive 4.x `device_deinit()` fork (Q1), and only
`on_system_shutdown` finds a native optional slot (the PM-device argument,
whose omit-when-absent shape maps the D10/I02 contract structurally). Zephyr is
*stronger* than `generic_c` on one row: the full `match_set` is consumable
(one instantiation block per compatible, uart_pl011.c:795-829), where
`generic_c` renders only the first entry.

## 5. diagnostics-logging sweep notes

Zephyr **has** a native structured logging engine, so the idiomatic lowering
is the **inverse** of `generic_c`'s per-segment `fwrite` decomposition:
reassemble the parsed segments into one cbprintf format string + argument list
(the same move the Linux lowering makes for `printk`). The `(spec,cast)` pairs
from `diag_specifier_and_cast_for` carry over whole because cbprintf is
C99-printf-compatible. Three Zephyr-specific deltas: the `[LEVEL]` text tag is
dropped (severity + module identity are structural), the trailing newline is
dropped (the log core appends it; every vendored `LOG_*` call is
newline-free), and the composition seam **inverts in granularity** vs Linux —
device/driver identity is carried per-TU by `LOG_MODULE_REGISTER` (module name
← `DriverDescriptor.name`), not per-call by a `dev` pointer. The one real gap
is the non-static `&str` argument under the default deferred logging mode
(the packager copies transient strings NUL-terminated; a Rust fat pointer is
not NUL-terminated) — the recommended bounce-buffer lowering and its
documented truncation divergences are LOG-O1.

## 6. RegseqError sweep notes

The Zephyr backend inherits the **entire** recognition + carrier +
discriminant-read machinery unchanged and substitutes exactly two things:
**errno polarity** (`return EINVAL;` → `return -EINVAL;` — Zephyr's API-wide
negative-errno convention) and **adapter placement** (standalone
`__regseq_exit` + harness `main` → fused into the `DEVICE_DT_INST_DEFINE`-
registered init function's return path). Unlike Linux, the **include name does
not change**: Zephyr's libcs (including minimal libc) provide standard
`<errno.h>` defining all nine C02 symbols, so the demand-driven include
machinery is reused verbatim, header name and all. The C02 table is
**symbol-identical**; the only convention flag is `Unsupported → -EOPNOTSUPP`
vs Zephyr's in-tree `-ENOTSUP` habit (ZE-OQ1, recommendation: stay
symbol-identical). Two Zephyr-specific semantic facts: the kernel records a
failed init's errno into `dev->state->init_res` as a clamped positive
`uint8_t` (the variant identity partially survives at runtime), and `main`'s
return value is ignored by the kernel, so the standalone-entry adapter
executes correctly but its rc is unobservable (ZE-OQ3). There is **no
`-EPROBE_DEFER` analogue** to protect — the adapter's closed nine-code set
cannot collide with any kernel-recognized control errno on Zephyr.

---

## 7. Dropped / not-applicable ledger (logged, never silent)

Per the strategy doc's anti-truncation rule, everything dropped on Zephyr is
recorded here:

- **`claim_regions` on BOTH paths** — not-applicable: zero calls emitted.
  Platform: the claim *is* the DT `reg` property captured at build time
  (uart_pl011.c:759, :43). PCIe: subsumed into the §B.3.4 `pcie_get_mbar`
  resolution (reference idiom: `pcie_probe_mbar`, eth_e1000.c:219). The
  as-built contract names this realization verbatim
  (prelude.rs:202-208); the ordering slot is retained. This is a **called-out
  elision** — stronger than Linux, where only the platform path fuses.
- **`enable_device` on platform/Amba** — not-applicable (default-on); the model
  already omits it via the `is_pci` gate (`body_emit.rs:5256-5259`).
- **`set_bus_master`** — not-applicable today; dormant (`dma_present` defaults
  false, no fixture sets it; DMA out of scope, GRILL O2). Target identified:
  the `PCIE_CONF_CMDSTAT_MASTER` bit (eth_e1000.c:220-221).
- **teardown dispatch / postamble / orderly / surprise on the 3.7 baseline** —
  not-applicable: no remove/unbind slot exists in `DEVICE_DT_INST_DEFINE`
  (uart_pl011.c:789-791; eth_e1000.c:337-344); static DT devices are
  permanent. Carried forward as Q1/Q2, not silently dropped.
- **lifecycle runner + entry shim** — not-applicable: Zephyr owns `main`; the
  kernel's init-level walk is the runner. A lifecycle TU emits only the
  scaffold; RD-7's no-lifecycle byte-purity is trivially preserved.
- **`MatchEntry::Acpi/Usb/I2c/Spi`** — not-applicable: bridge-rejected at
  parse (CR-5); never reach a backend. No `MODULE_DEVICE_TABLE`/modalias
  analogue exists on Zephyr (no module autoload).
- **diag-log `[LEVEL]` text tag** — dropped; severity becomes structural
  (macro selection + log-core prefix), not message text.
- **diag-log trailing `\n`** — dropped; the log output stage appends the line
  ending (every vendored `LOG_*` call is newline-free).
- **pointers/`{:p}`, `isize`/`usize`, `char`, source-context, indirect log
  calls, hex-on-`&str`** — not lowered; absent from / rejected by the as-built
  surface (do not invent).
- **`binding=bound|none` registration trace arg** — host-trace artifact,
  not-applicable on Zephyr (static instantiation; regions resolve at init).
- **`on_init` always-marker when not overridden** — no Zephyr runtime form
  (the trait-default no-op emits nothing beyond the scaffold).
- **`MODULE_DEVICE_TABLE` analog** — does not exist; the modalias artifact is
  Linux-only.

## 8. `needs-new-surface` escalations (blocking gaps, not worked around)

- **`poll_until`** — `recognized_not_lowered` (`domain_attr.rs:94-95`).
  `generic_c` emits nothing; the convergence doc §9 says "do not claim it".
  Candidate (non-normative): `WAIT_FOR(cond, timeout_us, k_busy_wait(delay_us))`
  (`v3.7-branch include/zephyr/sys/util.h`; untimed degenerate form
  uart_pl011.c:224-226). Its timeout→`RegseqError::Timeout` edge is undesigned.
  Blocks until `generic_c` establishes a reference. (RA-OQ5)
- **`BarType::Io`** — `recognized_not_lowered` (`device_lifecycle.rs:248-249`).
  Zephyr does have an x86 port-I/O surface (`sys_in*/sys_out*`) but no in-repo
  reference exercises it and the model-side Mem/Io unification decision is
  unmade — the same blocking gap Linux records. Escalate. (BRM-O5)
- **Config-space width-8 + sub-word write RMW** — `config_access_width` admits
  widths {1,2,4,8} but Zephyr's accessor is one 32-bit word op; width 8 = two
  words or reject, and widths 1/2 on the write side require a word RMW that is
  **bus-observable** — needs an explicit decision before silent emission
  (parity with the Linux width-8 gap).
- **Non-PCI config access (BRM-001)** — the rejection diagnostic is deferred
  upstream (bus-resource-management-surface-convergence.md:209-211); the
  Zephyr spec inherits the same unbuilt-diagnostic dependency Linux records.
- **`MatchEntry::Amba`** — no Zephyr carrier (no periphid bus). An Amba-only
  match set cannot lower; whether to diagnose or require a companion
  `Devicetree` entry is model-side. (Q4)
- **`on_deinit` / orderly teardown carrier** — none on 3.7 LTS; the 4.x
  `device_deinit()` fork is version-sensitive and deliberately not asserted.
  (Q1/ZE-OQ2)
- **`on_surprise_remove`** — no Zephyr analog at all; possibly not
  representable on this target. (Q2)
- **`UserDataField`/`UserDataKind`** — `recognized_not_lowered`. The C
  static-storage zero image inherits the same incidental guarantee as
  `generic_c`'s `{0}`; inherited, not designed. A new lowering must not be
  invented here.
- **`ClassTag`/`device_class`** — `recognized_not_lowered`; on Zephyr the
  class *visibly* selects the define-macro family and API vtable
  (uart_pl011.c:534-542 vs eth_e1000.c:337-344), which is exactly the deferred
  Device Class construct's territory. Build-now: class-neutral
  `DEVICE_DT_INST_DEFINE` with NULL api (Q8a).
- **>1 resolved region per driver** — rejected today
  (`max_resolved_bindings_per_driver`, `body_emit.rs:5240-5243`); the
  constraint *aligns* with the non-named `DEVICE_MMIO` single-region API;
  multi-region would graduate to `DEVICE_MMIO_NAMED_*` — model extension
  required first.

## 9. Cross-construct ordering re-verification (end-to-end, not per-primitive)

Re-checked per strategy step 5: the Zephyr `map_region` target
(`DEVICE_MMIO_MAP` / `device_map`) consumes the `(base,size)` produced by
`get_resource_handle` (`DEVICE_MMIO_ROM_INIT`'s build-time capture /
the PCIe BAR probe's MBAR, `pcie_get_mbar`) and nothing else; `map_region` stays strictly after
`get_resource_handle` on both paths (the reference itself documents the
constraint: "Must be placed after DEVICE_MMIO_MAP", uart_pl011.c:579-580).
Hoisting steps to build time preserves relative order — a step never executes
after one that follows it in the canonical sequence — but ordering must be
checked **across binding times**, not within one function. The one genuine
permutation is flagged, not absorbed: the PCIe reference probes the BAR before
setting memory decode (eth_e1000.c:219-221), inverting canonical
`enable → handle`; BRM-O2 recommends ratifying the reference order as a
per-OS deviation. The authored/native error boundary holds: native preamble
status propagates directly, the authored `on_init` `RegseqError` crosses the
negated adapter, and no kernel-recognized control errno exists for the adapter
to collide with (no `-EPROBE_DEFER` analogue). I04 holds because the
`(base,size)` *source* is the only thing that differs between host and Zephyr.

## 10. Completeness check (the strategy doc's six points, answered for Zephyr)

1. **Disposition assigned for every row** — yes (§1, all five constructs swept
   plus the cross-construct rows).
2. **Each disposition cited** — yes (repo `path:line`, in-repo vendored-driver
   witness, or pinned Zephyr doc/source URL per row).
3. **Version-sensitive choices recorded as open questions** — yes
   ([`zephyr-open-questions.md`](./zephyr-open-questions.md)): the
   `pcie_conf_*` signature, `sys_read64` arch-conditionality, the 4.x
   `device_deinit()` fork, PM trigger semantics, errno-symbol presence across
   libcs, the PCIe DT binding spelling.
4. **Cross-construct ordering re-verified end-to-end** — yes (§9).
5. **Everything dropped is logged** — yes (§7).
6. **Every `needs-new-surface` filed as a blocking gap** — yes (§8):
   `poll_until`, `BarType::Io`, config width-8/sub-word-RMW, non-PCI config,
   `MatchEntry::Amba`, the teardown carrier, `on_surprise_remove`,
   `UserDataField`, `ClassTag`, multi-region.

**Verdict.** All five register-access MMIO ops, the BRM managed-op block on
both bus paths (with the claim elision called out), config read/write (PCIe),
the full bring-up half of the lifecycle scaffold (register/match/create/
associate/preamble/on_init/init-fail) plus the shutdown PM slot, all four log
levels, and the entire RegseqError channel are **lowerable on Zephyr** with
confident citations grounded in the two vendored stock drivers. The residual
gaps are the §8 `needs-new-surface` rows — all pre-existing
`recognized_not_lowered` model facts or Zephyr-device-model properties
(teardown), none introduced by the Zephyr target itself.
