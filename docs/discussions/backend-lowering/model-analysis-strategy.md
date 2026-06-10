# Model analysis strategy — a repeatable per-OS sweep

**Purpose.** When you bring up a new OS backend (Linux, Zephyr, …), you do not
start from a blank page and you do not free-associate. You **sweep the primitive
inventory**, and for each primitive you assign exactly one of four dispositions
against your target OS, with a citation. This document defines that method, the
four dispositions, the per-OS target pins, and the worked sweep skeleton.

It depends on the three other documents:
[`primitive-inventory.md`](./primitive-inventory.md) is the list you sweep;
[`regseqmodel-input-contract.md`](./regseqmodel-input-contract.md) is what each
primitive hands you; [`agnostic-core-vs-os-shell.md`](./agnostic-core-vs-os-shell.md)
tells you which half of each bank you keep vs replace.

> **Citation rule (inherited).** Every disposition cites either a repo
> `path:line` (the as-built model surface or the reference lowering) **or** a
> pinned OS-doc URL. Where an OS API behaves differently across releases, the pin
> is flagged version-sensitive and recorded as an open question — it is **not**
> silently resolved. The reference backend `generic_c` is a *host model*: its
> host hooks name their intended OS realizations in comments
> (e.g. prelude.rs:186-208), but those names are **backend-[Open]**, to be
> confirmed against the pinned OS docs during the sweep, never assumed.

---

## The four dispositions

For each primitive row, assign one:

1. **native-materialization** — the target OS has a *direct* primitive this
   lowers onto. You keep the reference backend's recognition + construct
   arithmetic ([`agnostic-core-vs-os-shell.md`](./agnostic-core-vs-os-shell.md)
   "core") and replace only the **call target**.
   *Example:* `mmio_read` → Linux `readl()` / `ioread32()`; `register_driver`
   (PCI) → `pci_register_driver()`.

2. **shared-procedural-core** — there is no single OS primitive; the lowering is
   reused **mechanism**, not a target call. You keep the reference machinery
   essentially verbatim. This is the disposition for the assembly's core banks
   and, crucially, for the **cross-construct ordering** (the BRM→`map_region`
   preamble sequence) and the **error-channel boundary**.
   *Example:* the name tables, type layout, body spine; the canonical preamble
   ordering (`body_emit.rs:5197-5285`); the `Result` niche-discriminant read
   (`body_emit.rs:4654-4707`).

3. **not-applicable** — the primitive does not exist / is a no-op / is gated off
   on this target. You emit nothing (and `log()` that you dropped it — silent
   truncation reads as "covered").
   *Example:* `claim_regions` on Zephyr (no runtime region reservation;
   prelude.rs:202-208); `enable_device` on a platform/DT bus (default-on;
   `body_emit.rs:5256-5259`); `set_bus_master` with no DMA (dormant; §dormant).

4. **needs-new-surface** — the target OS needs something the *model does not yet
   carry*, or the reference backend never lowered. This is a gap to escalate, not
   to paper over: it blocks until the model (or a new fixture) is extended.
   *Example:* `BarType::Io` port-I/O (recognized-not-lowered;
   device_lifecycle.rs:243-250); `poll_until` timeout
   (recognized-not-lowered; domain_attr.rs:95); `ClassTag`, `UserDataField`
   (recognized-not-lowered); more than one resolved region per driver (rejected
   today, `mod.rs:248-254`).

A clean sweep is a table: **primitive × OS → disposition + citation**. Anything
left as a `?` is an explicit open question, never an implied success.

---

## The method, step by step

For each primitive in [`primitive-inventory.md`](./primitive-inventory.md):

1. **Read the as-built surface.** Open the construct's surface-convergence doc
   (register-access / bus-resource-management / device-lifecycle) and confirm the
   normative semantics the lowering must preserve — *not* the host realization,
   the semantics. The reference `generic_c_source` column shows you the reference
   materialization to diff against.

2. **Read what the model hands you.** From the `model_source` column +
   [`regseqmodel-input-contract.md`](./regseqmodel-input-contract.md), confirm the
   exact carrier (e.g. `MmioBinding.{effective_bar,base,size}`, `MmioOp`,
   `LifecycleOps`).

3. **Locate the seam.** From
   [`agnostic-core-vs-os-shell.md`](./agnostic-core-vs-os-shell.md), identify
   which half is core (kept) and which is shell (the materialization target you
   are choosing now).

4. **Consult the pinned OS docs** (below) for the target primitive, and assign a
   disposition. If the OS API is version-sensitive, pin the version and record an
   open question.

5. **Preserve composition.** Before finalizing a register-access or BRM
   disposition, re-check the cross-construct ordering: does your `map_region`
   target still consume the BRM-resolved `(base,size)` *after* your
   `get_resource_handle`? (Ordering invariant **I06**, `body_emit.rs:5271`; the
   `(base,size)` source-swap is the *separate* invariant **I04**,
   prelude.rs:160-162.) A per-primitive
   choice that breaks the preamble ordering is wrong even if each primitive looks
   right in isolation.

6. **Classify the error channel.** Decide, per fallible operation, whether it is
   **authored** (lowers through the RegseqError exit-adapter) or **native** (the
   OS call's status propagates directly). See §*Native vs authored* — this is not
   a per-primitive convenience, it is a hard rule.

---

## OS-target pins

Pin these before sweeping; flag every version-sensitive choice as an open
question rather than resolving it silently.

### Linux — modern mainline LTS (6.6+)

The single most consequential pin is **PCI vs platform/DT**, because it changes
the realization of half the BRM and register-access rows:

| primitive | PCI path | platform / DT path |
|---|---|---|
| `register_driver` | `pci_register_driver()` | `platform_driver_register()` / `module_platform_driver()`; **AMBA is distinct** → `amba_driver_register()` + periphid (bus-resource-management-surface-convergence.md, GRILL D07) |
| `enable_device` | `pci_enable_device()` / `pcim_enable_device()` | n/a — default-on (not-applicable) |
| `claim_regions` | `pci_request_regions()` / `pci_request_selected_regions()` | fused into the map call (`devm_ioremap_resource`) |
| `map_region` `(base,size)` source | `pci_resource_start()` / `pci_resource_len()` → `pci_iomap()` / `pcim_iomap_regions()` | `platform_get_resource()` → `devm_ioremap_resource()` / `devm_platform_ioremap_resource()` |
| `mmio_read/write` accessor | `readl`/`writel` *or* `ioread32`/`iowrite32` (selection is an open question) | same |
| `set_bus_master` | `pci_set_master()` (DMA-gated) | n/a |
| log-{debug,info,warn,error} | `dev_dbg`/`dev_info`/`dev_warn`/`dev_err` | same |
| RegseqError / native | authored → negative errno from `.probe`; native (`-EPROBE_DEFER`, `-ENODEV`, …) propagated **directly** | same |

Pinned reference docs (version-sensitive — confirm against the target kernel):
- PCI driver API: <https://www.kernel.org/doc/html/latest/PCI/pci.html>
- Device I/O (`readl`/`writel`, `ioread32`/`iowrite32`): <https://www.kernel.org/doc/html/latest/driver-api/device-io.html>
- Managed resources / `devm_*`: <https://www.kernel.org/doc/html/latest/driver-api/driver-model/devres.html>
- Platform devices: <https://www.kernel.org/doc/html/latest/driver-api/driver-model/platform.html>

**Open questions (Linux):** `readl` vs `ioread32` selection per region/arch;
PCI probe/remove return semantics vs the synthesized scenario set; whether
`pcim_*` (managed) or bare `pci_*` is the default; how `-EPROBE_DEFER` surfaces
through the authored/native boundary at the preamble.

### Zephyr — recent release (3.7 LTS / 4.x)

Scope = devicetree / platform-style devices.

| primitive | Zephyr realization |
|---|---|
| `register_driver` | `DEVICE_DT_DEFINE()` + DT instance macros (`DT_INST_*`) |
| `enable_device` / `claim_regions` / `set_bus_master` | largely **not-applicable** (no runtime PCI enable; region is resolved/sized, not reserved — prelude.rs:202-208) |
| `map_region` | `DEVICE_MMIO_MAP()` / `DEVICE_MMIO_ROM`/`RAM` + `DEVICE_MMIO_GET()` |
| `mmio_read/write` accessor | `sys_read32`/`sys_write32` (via the `DEVICE_MMIO` mapped address) |
| log-{debug,info,warn,error} | `LOG_MODULE_REGISTER` + `LOG_DBG`/`LOG_INF`/`LOG_WRN`/`LOG_ERR` |
| RegseqError / native | authored → `-E*` from the device init hook; native propagated directly |

Pinned reference docs (version-sensitive):
- Device model / `DEVICE_DT_DEFINE`: <https://docs.zephyrproject.org/latest/kernel/drivers/index.html>
- Devicetree API (`DT_*` / `DT_INST_*`): <https://docs.zephyrproject.org/latest/build/dts/api/api.html>
- `DEVICE_MMIO_*`: <https://docs.zephyrproject.org/latest/hardware/peripherals/device_mmio.html>
- Logging (`LOG_*`): <https://docs.zephyrproject.org/latest/services/logging/index.html>

**Open questions (Zephyr):** **Zephyr-over-PCI is deferred** — the entire
PCI-gated column (`enable_device` / `set_bus_master`, `bar`-selected regions) has
no obvious Zephyr-DT mapping; treat as needs-new-surface / out-of-scope unless a
trivial mapping appears. Whether `claim_regions` is genuinely a no-op or maps to
a `DEVICE_MMIO` reservation step is version-sensitive.

---

## Worked sweep skeleton (fill in per OS)

| primitive | Linux (PCI / platform) | Zephyr (DT) | notes |
|---|---|---|---|
| mmio_read / write / *_field / modify | native-materialization → `readl`/`writel` (or `ioread32`) | native-materialization → `sys_read32`/`write32` | keep `emit_mmio` field arithmetic (core); replace the accessor only |
| poll_until | needs-new-surface (recognized-not-lowered; domain_attr.rs:95) | needs-new-surface | reference emits nothing; design `readl_poll_timeout` (Linux) / Zephyr poll, or fail loud |
| map_region (keystone) | native-materialization → `pci_iomap` / `devm_ioremap_resource`, fed BRM `(base,size)` | native-materialization → `DEVICE_MMIO_MAP` | **must stay after get_resource_handle** (I06) |
| read_config / write_config | native-materialization (PCI) → `pci_read_config_*` / `pci_write_config_*` | not-applicable (no PCI) | PCI-gated; BRM-001 non-PCI rejection |
| register_driver | native-materialization → `pci_register_driver` / `platform_/amba_driver_register` | native-materialization → `DEVICE_DT_DEFINE` | DL owns the marker; BRM supplies the selector token |
| enable_device | native (PCI) / not-applicable (platform) | not-applicable | PCI-gated (`is_pci_like`) |
| claim_regions | native (PCI `pci_request_regions`) / fused (AMBA) | not-applicable (no-op) | every-bus in the model; realization differs per target |
| set_bus_master | not-applicable today (dormant; `dma_present` defaults false) | not-applicable | out-of-scope DMA fact gates it |
| get_resource_handle | shared-procedural-core (captures handle context) | shared-procedural-core | unconditional; the seam anchor for map_region |
| BarType::Io | needs-new-surface (recognized-not-lowered) | not-applicable (no port I/O) | Linux port-I/O is a new lowering surface |
| on_init / deinit / surprise / init_failure / system_shutdown | shared-procedural-core (callbacks) + native shim shape | shared-procedural-core | bodies emit as functions; scenario *shape* is OS shell |
| orderly/surprise/init-fail/shutdown scenarios | shell (scenario shape: probe/remove/shutdown) | shell (init-hook shape) | the *selection* + *ordering* is core (BackendOptions); the realized hooks are shell |
| register / create / associate / teardown / on_init→Err | shared-procedural-core (synthesized steps) | shared-procedural-core | backend-synthesized; preserve ordering |
| preamble / postamble | shared-procedural-core (forward/reverse) | shared-procedural-core | trace-only today; real acquire/release is backend-[Open] |
| MatchEntry | native-materialization → `pci_device_id` / OF `compatible` table / AMBA periphid | native-materialization → DT `compatible` | render into the OS match table |
| ClassTag / device_class | needs-new-surface (recognized-not-lowered) | needs-new-surface | class-trait cross-check is the Device Class construct's job |
| ResourceField / ResourceKind::Mmio | shared-procedural-core | shared-procedural-core | drives preamble/postamble + the BRM block |
| UserDataField / UserDataKind | needs-new-surface (recognized-not-lowered) | needs-new-surface | zero-init implicit today (incidental, not designed) |
| log-{debug,info,warn,error} | native-materialization → `dev_*` | native-materialization → `LOG_*` | template/format lowering deferred (logging surface in flux) |
| FrameworkType::RegseqError marker | shared-procedural-core (recognition) | shared-procedural-core | the `FrameworkType` recognition is reused verbatim |
| RegseqError::* → errno | native-materialization (table values) | native-materialization (table values) | keys are model facts; **values + return convention are per-target** |
| RegseqError → errno exit-adapter | shared-procedural-core (read+switch) + shell (return form) | shared-procedural-core + shell | replace `return EINVAL;` (exit) with `return -EINVAL;` (probe) |
| Result<(),RegseqError> carrier | shared-procedural-core (niche read) | shared-procedural-core | the discriminant-read block is OS-neutral |

This skeleton is a starting partition, not a verdict: every cell still needs the
pinned-doc citation and the open-question flags from §*OS-target pins*.

---

## Native vs authored — the rule that overrides the table

The error-channel disposition is **not** a per-primitive convenience; it is a
hard provenance rule that the
[`regseqmodel-input-contract.md`](./regseqmodel-input-contract.md) §8 and the
regseq-error perspective-6 doc state outright:

- **Authored** failures — a `RegseqError` the *spec author wrote* in a callback
  body (`on_init`, …) — lower through the RegseqError exit-adapter
  (errno table → the target's negative-errno / `-E*` return). This is the
  `errno_symbol` path (`errno.rs:35-48`, `emit_exit_adapter`
  `body_emit.rs:4732-4828`).
- **Native** failures — a status from a *compiler-generated* preamble call
  (`devm_ioremap_resource`, `clk_get`, `dma_request_chan`, `pinctrl_select_state`,
  …) — propagate the native errno **directly**. They are **never** round-tripped
  through `RegseqError`: wrapping `-EPROBE_DEFER` as a generic code breaks Linux
  probe deferral and erases platform semantics the DSL cannot express.

So when you sweep BRM / lifecycle preamble ops for a real OS, each op that *can
fail natively* gets a **native** error disposition (direct propagation), even
though the op itself is `native-materialization` or `shared-procedural-core` for
its *success* path. The boundary is the authored/generated code split, not a
syntactic one. `generic_c` (a host model with no failing native calls)
materializes only the authored adapter today — do not mistake that for permission
to collapse native failures into `RegseqError`.

---

## Completeness check (run before declaring a sweep done)

A sweep is complete only when, for every row in
[`primitive-inventory.md`](./primitive-inventory.md):

1. a disposition is assigned (no `?` left);
2. each disposition cites a repo `path:line` or a pinned OS-doc URL;
3. every version-sensitive OS choice is recorded as an open question, not
   resolved silently;
4. the cross-construct ordering (BRM → `map_region`; the authored/native error
   boundary) is re-verified end-to-end, not just per primitive;
5. anything dropped (a `not-applicable`, a gated-off op) is **logged**, so silent
   truncation cannot masquerade as coverage;
6. every `needs-new-surface` is filed as a blocking gap against the model, not
   worked around in the backend.
