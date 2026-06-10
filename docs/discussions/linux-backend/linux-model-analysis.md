# Linux model analysis — the whole-model sweep

This is the Linux execution of the per-OS sweep method
([`../backend-lowering/model-analysis-strategy.md`](../backend-lowering/model-analysis-strategy.md)).
It walks the primitive census
([`../backend-lowering/primitive-inventory.md`](../backend-lowering/primitive-inventory.md))
one row at a time and assigns each primitive exactly one of the four
dispositions for **Linux mainline LTS 6.6+**, with a citation. Nothing is left as
a `?`; every `not-applicable` is logged (§7) so a dropped op cannot masquerade as
coverage, and every `needs-new-surface` is filed as a blocking gap (§8), not
papered over.

**The four dispositions** (from the strategy doc §*The four dispositions*):
**native-materialization** (a direct Linux primitive — keep `generic_c`'s
recognition + arithmetic, replace the call target); **shared-procedural-core**
(no single primitive — reuse the reference *mechanism* verbatim, e.g. ordering,
the niche read); **not-applicable** (does not exist / no-op / gated off — emit
nothing, log it); **needs-new-surface** (the model or reference carries nothing
to lower — escalate, do not invent).

**The decisive sub-pin: PCI vs platform/DT.** It changes half the BRM and
register-access rows. `is_pci_like` is the model gate (`device_lifecycle.rs:101-104`,
only `Pci` today); on the PCI path the preamble keeps its full
claim→handle→map shape, on platform/DT the claim∘map step **fuses** into one
`devm_ioremap_resource`. AMBA is a *third* registration framework
(`amba_driver_register`), distinct from both.

---

## 1. The sweep table (primitive × Linux → disposition + citation)

### register-access

| primitive | Linux disposition | citation |
|---|---|---|
| `mmio_read` (`MmioOp::Read`) | **native-materialization** → `readl`/`readb/w/q` (or `ioread*`) over `base+off`; width-8 (`readq`) is `CONFIG_64BIT`-gated, fail-loud otherwise (per-construct §A.2, A-12) | `body_emit.rs:2297-2302`; width from `recover_register` `:2514-2520`; <https://docs.kernel.org/driver-api/device-io.html> |
| `mmio_read_field` (`MmioOp::ReadField`) | **native-materialization** → `(readl(base+off) & M) >> S` | `body_emit.rs:2303-2311` (mask/shift arithmetic = core, kept) |
| `mmio_write` (`MmioOp::Write`) | **native-materialization** → `writel(val, base+off)` (**value-first** arg order) | `body_emit.rs:2283-2287`; <https://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git/plain/include/asm-generic/io.h?h=v6.6> |
| `mmio_write_field` (`MmioOp::WriteField`) | **native-materialization** → `writel((V & M), base+off)` (masked whole-reg, not RMW) | `body_emit.rs:2288-2296`; `register-access-surface-convergence.md §2.4` |
| `mmio_modify` (`MmioOp::Modify`) | **native-materialization** → `cur=readl; writel((cur&~M)|(V&M))` | `body_emit.rs:2312-2323` (RMW shape = core) |
| `poll_until` (`IntrinsicKind::PollUntil`) | **needs-new-surface** — `generic_c` lowers nothing; candidate `readl_poll_timeout` is non-normative | `domain_attr.rs:95`; not intercepted `body_emit.rs:2100-2125`; <https://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git/plain/include/linux/iopoll.h?h=v6.6> |
| `map_region` (keystone) | **native-materialization** (fed BRM `(base,size)`) → PCI `pci_iomap`; platform/DT `devm_ioremap_resource` | `body_emit.rs:5277-5284`; must stay after `get_resource_handle` (I04) |

### bus-resource-management

| primitive | Linux disposition | citation |
|---|---|---|
| `register_driver` (bus selector) | **native-materialization** → `module_{pci,platform,amba}_driver` by `BusType`; i2c/spi/usb deferred | selector `body_emit.rs:5050-5061`; reference-drivers/e1000/linux/e1000_main.c:224; reference-drivers/pl011/linux/amba-pl011.c:3205 |
| `enable_device` | **native-materialization** (PCI) → `pci_enable_device`/`pcim_enable_device`; **not-applicable** (platform/AMBA, default-on) | gate `body_emit.rs:5256-5259`; reference-drivers/e1000/linux/e1000_main.c:934 |
| `claim_regions` | **native-materialization** (PCI) → `pci_request_selected_regions`; **fused into `map_region`** (platform/DT/AMBA) | `body_emit.rs:5260-5266`; reference-drivers/e1000/linux/e1000_main.c:942; fusion `prelude.rs:206-208` |
| `set_bus_master` | **not-applicable today** (dormant; target `pci_set_master` identified) | gate `body_emit.rs:5267-5270`; `dma_present` placeholder `device_lifecycle.rs:62-70`; reference-drivers/e1000/linux/e1000_main.c:946 |
| `get_resource_handle` | **shared-procedural-core** (seam anchor) → PCI `pci_resource_start/len`; platform/DT `platform_get_resource` | `body_emit.rs:5271-5272`; reference-drivers/e1000/linux/e1000_main.c:979; reference-drivers/pl011/linux/amba-pl011.c:3123 |
| `read_config` (`ConfigOp::Read`) | **native-materialization** (PCI) → `pci_read_config_{byte,word,dword}` by width 1/2/4 | `body_emit.rs:2364-2375`; <https://docs.kernel.org/PCI/pci.html> |
| `write_config` (`ConfigOp::Write`) | **native-materialization** (PCI) → `pci_write_config_{byte,word,dword}`; status `int` explicitly discarded `(void)…` pending A-11 (v6.6 `pci.h:1213-1215`) | `body_emit.rs:2376-2394` |
| `MmioBinding` | **shared-procedural-core** — role kept, `base/size` values replaced by enumeration | `device_lifecycle.rs:209-238`; consumed `body_emit.rs:5277-5284` |
| `BusType` + `is_pci_like` | **native-materialization** — drives framework + PCI-only gate | `body_emit.rs:5050-5061`; gate `device_lifecycle.rs:101-104` |
| `BarType::Io` | **needs-new-surface** — `generic_c` lowers nothing; Linux port-I/O is a distinct surface | `device_lifecycle.rs:243-250`; based-MMIO accessors are readl-shaped `prelude.rs:233-250` |

### device-lifecycle

| primitive | Linux disposition | citation |
|---|---|---|
| `register` marker | **native-materialization** → `pci/amba/platform_driver.name` + the registration call | `body_emit.rs:5197-5199`; reference-drivers/e1000/linux/e1000_main.c:181; reference-drivers/pl011/linux/amba-pl011.c:3190 |
| `MatchEntry` (full set) | **native-materialization** → `pci/of/amba_device_id[]` + `MODULE_DEVICE_TABLE` | `device_lifecycle.rs:110-130`; reference-drivers/pl011/linux/amba-pl011.c:3167-3186; reference-drivers/e1000/linux/e1000_main.c:64,182 |
| `create` + container morph | **shared-procedural-core** → zeroed private alloc (`devm_kzalloc`/`alloc_etherdev`) | morph `body_emit.rs:5184-5189`; reference-drivers/pl011/linux/amba-pl011.c:2982; reference-drivers/e1000/linux/e1000_main.c:952 |
| `associate` | **native-materialization** → `pci_set_drvdata`/`amba_set_drvdata`/`SET_NETDEV_DEV` | `body_emit.rs:5207`; reference-drivers/e1000/linux/e1000_main.c:958; reference-drivers/pl011/linux/amba-pl011.c:3024 |
| `preamble` (forward) | **shared-procedural-core** — owns the slot + forward order; per-op call is BRM/reg-access | `body_emit.rs:5214-5217` + BRM block `:5244-5285` |
| `postamble` (reverse) | **shared-procedural-core** → reverse release; or **not-applicable** under `devm_*` (auto) | `body_emit.rs:5403-5406`; reference-drivers/e1000/linux/e1000_main.c:1275,1278,1284 |
| `on_init` | **shared-procedural-core** → author body at end of `.probe`; `Ok→0` / `Err→-errno` | `body_emit.rs:5311,5318-5319`; Result capture `:5345-5354` |
| `on_deinit` | **shared-procedural-core** → author body in `.remove()`; Result discarded | `body_emit.rs:5321`; reference-drivers/e1000/linux/e1000_main.c:1252 |
| `on_surprise_remove` | **shared-procedural-core** → `.remove()` device-gone branch; no HW access | `body_emit.rs:5332-5339` |
| `on_init_failure` | **shared-procedural-core** → cleanup on the authored `on_init`→Err ladder arm only, before `return -errno` (never on a native preamble failure — the hook fires only after `on_init`'s Result reads Err) | `body_emit.rs:5421-5483` (Err gate `:5470-5481`); reference-drivers/e1000/linux/e1000_main.c:1224-1239 |
| `on_system_shutdown` | **native-materialization** (omit-when-absent) → `.shutdown` slot | `body_emit.rs:5360-5368`; reference-drivers/e1000/linux/e1000_main.c:186,5200 |
| `teardown` present-dispatch | **shared-procedural-core** → runtime predicate at top of `.remove()` | `body_emit.rs:5398`; `pci_device_is_present`/error-handlers reference-drivers/e1000/linux/e1000_main.c:187 |
| `register/create/associate/on_init→Err` markers | **shared-procedural-core** (synthesized; preserve ordering; markers have no Linux runtime form) | `body_emit.rs:5206-5207,5475-5476` |
| four scenarios (Orderly/Surprise/InitFail/Shutdown) | **shared-procedural-core** — *not* four functions; the kernel's invocation paths into `.probe`/`.remove`/`.shutdown` | `backend_options.rs:26-32` |
| `ResourceField`/`ResourceKind::Mmio` | **shared-procedural-core** → `void __iomem *` container member | `device_lifecycle.rs:186-201,253-257` |
| `UserDataField`/`UserDataKind` | **needs-new-surface** — `recognized_not_lowered`; only incidental zero-init satisfies it | `device_lifecycle.rs:263-279`; incidental `{0}` `body_emit.rs:5305` |
| `ClassTag`/`device_class` | **needs-new-surface** — `recognized_not_lowered`; deferred to Device Class construct | `device_lifecycle.rs:135-144`; `device-lifecycle-surface-convergence.md §1.2` |

### diagnostics-logging

| primitive | Linux disposition | citation |
|---|---|---|
| `log-debug`/`log-info`/`log-warn`/`log-error` | **native-materialization** → `dev_dbg/info/warn/err` (or `pr_*`); level → macro | `body_emit.rs:2101-2104,2722`; reference-drivers/pl011/linux/amba-pl011.c:449,460,468,927; reference-drivers/e1000/linux/e1000.h:325-330 |
| template/format reassembly | **shared-procedural-core** — reuse `parse_template` + `diag_specifier_and_cast_for` whole; reassemble one `printk` fmt (inverse of fwrite decomposition) | `body_emit.rs:5903-5950,5798-5850` |
| `&str` argument | **native-materialization** → `%.*s` + `(int)len,(const char*)data` (interior-NUL gap) | `body_emit.rs:2812-2817`; C11 §7.21.6.1 |
| pointers/`{:p}`, `isize`/`usize`, `char`, source-context | **not-applicable** (not in the as-built surface — not lowered, do not invent) | rejected `parse_placeholder :5956-6015`, `diag_specifier_and_cast_for :5793` |

### RegseqError

| primitive | Linux disposition | citation |
|---|---|---|
| `FrameworkType::RegseqError` marker | **shared-procedural-core** — reuse `is_framework_error_enum`/`classify_entry_return` verbatim | `mod.rs:670-679,647-658` |
| `Result<(),RegseqError>` carrier / `Ok` channel | **shared-procedural-core** — same niche read; `Ok(())→return 0;` | `body_emit.rs:4654-4707,4819-4821` |
| `RegseqError → errno` exit-adapter | **shared-procedural-core** (read+switch) + **shell** (return form) → negate arms, relocate to `.probe` return, `<linux/errno.h>` | `body_emit.rs:4732-4828`; `prelude.rs:434-438` |
| nine `RegseqError::* → errno` rows | **native-materialization** (table values) — **symbol-identical** to `generic_c`; only polarity changes (`return -EINVAL;`) | `errno.rs:35-48` (all nine are valid Linux UAPI errnos) |
| native operational failure channel | **shared-procedural-core** — native `-errno` (incl. `-EPROBE_DEFER`) propagates **directly**, never wrapped | `model-analysis-strategy.md:197-221`; preamble `body_emit.rs:5256-5285` |

---

## 2. register-access sweep notes

The op algebra is **backend-invariant**: offset recovery (`recover_register`,
`body_emit.rs:2439-2529`), the mask/shift/value descriptor reads, the masked-write
confinement, and the read-clear-OR RMW are all computed before any accessor is
named (`body_emit.rs:2271-2281`). Linux therefore differs in exactly two places —
the **accessor primitive** (width-parametric host helper → statically-selected
`readb/w/l/q`) and the **provenance of `base`** (host global regfile → real
`void __iomem *` from `map_region`). On Linux there is **no faked path**: `base`
is always the mapped cookie, so the `generic_c` based/faked switch collapses to
the based form, and I04 holds trivially. Two contracts to honor: `writel` is
**value-first** (`writel(value, addr)`), and `readl`/`writel` are LE + fully
ordered, matching the block default and the volatile-MMIO contract (`readl_relaxed`
is **not** selected — the construct does not model ordering).

## 3. bus-resource-management sweep notes

The structural pivot: `generic_c`'s one flat scenario function splits across the
Linux driver model along one seam — `register_driver` lifts to **module init**,
everything from `enable_device` through `map_region` and `on_init` becomes the
body of **`.probe`**. The order *within* probe is preserved exactly. The one place
the model's op count and Linux's call count differ is the **claim∘map fusion** on
platform/DT: the model fires a per-binding `claim_regions` on every bus, but on
platform/DT `devm_ioremap_resource` subsumes the reservation, so the model's
`claim → handle → map` triple collapses to two Linux calls — a Linux backend must
**not** assume `claim_regions` always emits a call. Config-space has a **width-8
gap** (no `u64` PCI accessor) and a **return-shape inversion** (status-int +
out-param vs the model's value-returning `read_config::<T>`).

## 4. device-lifecycle sweep notes

The structural shift: `generic_c` *synthesizes the OS* (a driving `main` + per-
scenario functions); on Linux the **kernel's bus core is the OS** and drives the
*same* canonical sequence by calling registered `.probe`/`.remove`/`.shutdown`
slots. The four `LifecycleScenario` variants are therefore **not four emitted
functions** on Linux — they are the kernel's natural invocation paths. Most
`generic_c` managed-step *markers* are pure host-oracle observability with **no
Linux runtime artifact** (`register`/`create`/`associate`/`teardown`/`on_init_err`
emit nothing); the *step* they mark becomes a position in probe/remove, and the
only runtime-observable equivalents are the return codes and `dev_*` logging.
`create`/`associate`/`preamble` are the exceptions whose *step* lowers to real
code.

## 5. diagnostics-logging sweep notes

Unlike the other constructs, logging has **no finalized surface-convergence doc**
— the authoritative as-built surface is the model intrinsic taxonomy plus
`generic_c`'s `emit_diag_log`. The idiomatic Linux move is the **inverse** of
`generic_c`'s per-segment `fwrite` decomposition: reassemble the decomposed
segments into **one** `printk` format string + argument list (the same move the
Zephyr lowering makes for `cbprintf`). The `(spec,cast)` pairs from
`diag_specifier_and_cast_for` carry over unchanged because `printk` is the *same*
`printf` varargs ABI. The one real composition seam is the **`dev` argument** —
`dev_*` needs the `struct device *` that device-lifecycle/BRM bind; `generic_c`
has none because it writes to a flat `stdout`.

## 6. RegseqError sweep notes

The Linux backend inherits the **entire** recognition + carrier +
discriminant-read machinery unchanged and substitutes only what is genuinely
Linux-specific: **errno polarity** (`return EINVAL;` → `return -EINVAL;`),
**adapter placement** (standalone `__regseq_exit` + harness `main` →
the `.probe`/callback return site), and the **include name** (`<errno.h>` →
`<linux/errno.h>`). The C02 table is **symbol-identical** — all nine symbols are
valid Linux UAPI errnos, and `Unsupported → EOPNOTSUPP` is already the correct
userspace-safe choice (not kernel-internal `ENOTSUPP`). The per-target divergence
the design anticipated shows up on Linux as **polarity, not symbol choice**.

---

## 7. Dropped / not-applicable ledger (logged, never silent)

Per the strategy doc's anti-truncation rule, everything dropped on Linux is
recorded here:

- **`enable_device` on platform/AMBA** — not-applicable (devices are default-on);
  the model already omits it via the `is_pci` gate (`body_emit.rs:5257`).
- **`claim_regions` as a distinct call on platform/DT/AMBA** — not-applicable as
  a standalone call; **subsumed** into `devm_ioremap_resource` (`prelude.rs:206-208`).
  The op is *not* dropped semantically (reserve-then-map still happens), only its
  separate materialization fuses.
- **`set_bus_master`** — not-applicable today; dormant (`dma_present` defaults
  false, no fixture sets it; DMA out of scope, GRILL O2).
- **diag-log `[LEVEL]` text tag** — dropped; severity becomes structural (macro +
  `KERN_*` + `dev_*` device prefix), not message text.
- **pointers/`{:p}`, `isize`/`usize`, `char`, source-context** — not lowered;
  absent from the as-built surface (do not invent).
- **`on_init` always-marker when not overridden** — no Linux form (the trait-
  default no-op emits nothing).

## 8. `needs-new-surface` escalations (blocking gaps, not worked around)

- **`poll_until`** — `recognized_not_lowered` (`domain_attr.rs:95`). `generic_c`
  emits nothing; `readl_poll_timeout` is a candidate, not a confirmed lowering.
  Its timeout→`RegseqError::Timeout` edge is also undesigned. Blocks until
  `generic_c` (or a fixture) establishes the predicate→`cond` translation and the
  DELAY/TIMEOUT units.
- **`BarType::Io`** — `recognized_not_lowered` (`device_lifecycle.rs:243-250`).
  Linux port-I/O (`inb/outb` or `ioread*` over a `pci_iomap` Io cookie) is a
  distinct surface the model has no emit path for; pushes the accessor-family
  choice toward `ioread32`/`iowrite32` uniformly. Escalate as a model gap.
- **`UserDataField`/`UserDataKind`** — `recognized_not_lowered`
  (`device_lifecycle.rs:263-279`). No designed lowering; only incidentally
  satisfied by the zeroed allocation. A new backend inherits none and must not
  invent one.
- **`ClassTag`/`device_class`** — `recognized_not_lowered`
  (`device_lifecycle.rs:135-144`). No `class_register` synthesized — that would be
  invention; deferred to the Device Class construct.
- **Config-space width-8** — admitted by `config_access_width` but no PCI
  primitive exists (accessors top out at 32-bit dword). Reject or split into two
  dwords — undesigned.
- **>1 resolved region per driver** — rejected today (`mod.rs:248-254` /
  `max_resolved_bindings_per_driver`); a real multi-BAR Linux driver needs the
  model extended.
- **multi-`compatible` rendering** — `render_match_trace` renders only `.first()`
  (`body_emit.rs:5499-5508`); Linux match tables are inherently multi-row. The
  *table emission* is straightforward, but the model surface the host trace reads
  undercounts; the backend must read the full `match_set`.

## 9. Cross-construct ordering re-verification (end-to-end, not per-primitive)

Re-checked per strategy step 5: the Linux `map_region` target
(`pci_iomap` / `devm_ioremap_resource`) consumes the `(base,size)` produced by
`get_resource_handle` (`pci_resource_start/len` / `platform_get_resource`) and
nothing else; `map_region` stays strictly **after** `get_resource_handle` inside
the `.probe` preamble; the postamble reverses it exactly. The authored/native
error boundary holds: native preamble status propagates directly, the authored
`on_init` `RegseqError` crosses the negated adapter, and `-EPROBE_DEFER` can never
be synthesized or overwritten by the adapter (it only emits the nine C02 codes +
defensive `-EIO`). I04 holds because the `(base,size)` *source* is the only thing
that differs between host and Linux.

## 10. Completeness check (the strategy doc's six points, answered for Linux)

1. **Disposition assigned for every row** — yes (§1, all five constructs swept).
2. **Each disposition cited** — yes (repo `path:line` or pinned doc URL per row).
3. **Version-sensitive choices recorded as open questions** — yes
   ([`linux-open-questions.md`](./linux-open-questions.md): accessor family,
   managed-vs-bare, `.remove` void signature, claim∘map fusion realization).
4. **Cross-construct ordering re-verified end-to-end** — yes (§9).
5. **Everything dropped is logged** — yes (§7).
6. **Every `needs-new-surface` filed as a blocking gap** — yes (§8): `poll_until`,
   `BarType::Io`, `UserDataField`, `ClassTag`, config width-8, multi-region,
   multi-`compatible`.

**Verdict.** All four register-access MMIO ops, all four BRM managed ops (PCI),
config read/write, the full lifecycle scaffold, all four log levels, and the
entire RegseqError channel are **lowerable on Linux** with confident citations.
The residual gaps are the six `needs-new-surface` rows above — all pre-existing
`recognized_not_lowered` model facts, none introduced by the Linux target.
