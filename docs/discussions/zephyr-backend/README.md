# Zephyr backend — the per-OS lowering spec

This directory is the **complete Zephyr backend lowering spec** for the five
in-scope Regseq constructs. It is the per-OS realization of the OS-neutral
foundation in [`../backend-lowering/`](../backend-lowering/README.md): that
directory defines the method (the four dispositions, the primitive census, the
agnostic-core/OS-shell seam); this directory *executes* that method against one
pinned target — **Zephyr 3.7 LTS, forward-compatible with 4.x**. It is the
sibling of the Linux spec ([`../linux-backend/`](../linux-backend/README.md))
and deliberately mirrors its file layout so the two can be diffed construct by
construct.

It is grounded in two things and only two things: (1) the **as-built** reference
backend `generic_c` (`crates/regseq-backends/src/generic_c/`), which is a *host
model* and the reference lowering; and (2) the **vendored stock Zephyr drivers**
under `reference-drivers/{pl011,e1000}/zephyr/`, which are authoritative as-built
Zephyr C — the PL011 UART (`uart_pl011.c`, the platform/devicetree path,
`DEVICE_MMIO_*`) and the Intel e1000 NIC (`eth_e1000.c`, the PCIe path,
`DEVICE_PCIE_INST_*` + `pcie_probe_mbar`). Every normative claim cites a repo
`path:line` or a pinned Zephyr doc/source URL. Where `generic_c` lowers nothing
(`poll_until`, `BarType::Io`, `ClassTag`, `UserDataField`), the Zephyr spec says
so explicitly and does **not** invent a lowering.

---

## Scope

**In scope — exactly five constructs:**

1. **register-access** — the five MMIO ops, `poll_until`, the `map_region`
   keystone seam, the `Mmio<T>`/`#[rseq::register_block]` offset recovery, the
   `Field`/`FieldValue` descriptors.
2. **bus-resource-management** — `register_driver`, the four managed preamble
   ops, config-space intrinsics, the `MmioBinding` / `BusType` / `BarType` model.
3. **device-lifecycle** — the five callbacks, the four scenarios, the managed
   steps, the typed match set, the container morph.
4. **diagnostics-logging** — the four log levels and their template lowering.
5. **RegseqError** — the framework marker, the nine-variant errno taxonomy, the
   exit-adapter boundary, and its hard separation from native OS errors.

**Out of scope:** interrupts, DMA, serial / data-path. (Out-of-scope facts —
e.g. the `dma_present` bit gating `set_bus_master` — appear only as a gate
input, never as a lowered primitive.)

---

## Read order

1. **[`zephyr-model-analysis.md`](./zephyr-model-analysis.md)** — *the sweep.*
   The whole-model census walked one primitive at a time, each assigned one of
   the four dispositions (native-materialization / shared-procedural-core /
   not-applicable / needs-new-surface) for Zephyr, with a citation and an
   open-question flag where the choice is version-sensitive. Start here.

2. **[`zephyr-lowering-per-construct.md`](./zephyr-lowering-per-construct.md)** —
   *the five constructs in depth.* The normative per-construct lowering prose.
   This is the body of the spec.

3. **[`zephyr-primitive-materialization.md`](./zephyr-primitive-materialization.md)** —
   *the per-primitive table.* One row per inventory primitive: model surface →
   `generic_c` reference → concrete Zephyr materialization → disposition.

4. **[`zephyr-scaffold-and-dispatch.md`](./zephyr-scaffold-and-dispatch.md)** —
   *the TU shell.* How the lowered primitives are wired into a real Zephyr
   driver translation unit: `DT_DRV_COMPAT` + `DEVICE_DT_INST_DEFINE` static
   instantiation, the per-instance init function, the optional PM-device slot,
   `LOG_MODULE_REGISTER`, and the RegseqError adapter fused into the init
   return path.

5. **[`zephyr-integration-probe-path.md`](./zephyr-integration-probe-path.md)** —
   *the composition.* The end-to-end driver shape: the ordered bring-up path
   re-partitioned across build time and boot time, the (largely absent)
   teardown path, and the seven cross-construct invariants.

6. **[`zephyr-traceability-matrix.md`](./zephyr-traceability-matrix.md)** — *the
   consolidated trace.* Every construct's trace rows in one matrix:
   model surface · `generic_c` ref · Zephyr lowering · scaffold point ·
   error/lifetime · open decision.

7. **[`zephyr-open-questions.md`](./zephyr-open-questions.md)** — *the decisions
   to ratify.* Every open decision gathered from every construct, each with a
   recommendation; plus the honest completeness ledger (which constructs are
   fully lowered, which carry `needs-new-surface` gaps), including the explicit
   `unresolved-constructs` list **and the legal-but-unlowered input-shape
   rejection ledger**.

8. **[`codex-review.md`](./codex-review.md)** — *the external review.* The
   verbatim Codex (GPT-5.5) verdict on this spec, with the per-issue
   disposition log of what was fixed and what was routed to the open
   questions.

---

## The three facts to internalize first (load-bearing on Zephyr)

**1. The cross-construct composition is real, and Zephyr re-partitions it
harder than Linux.** `generic_c` orders the bus preamble **inside** lifecycle
synthesis — `enable_device → claim_regions → set_bus_master →
get_resource_handle → map_region` (`body_emit.rs:5244-5285`) — and
register-access's `map_region` **consumes** the BRM-resolved `(base,size)`.
Linux hoists only `register_driver` to module init; Zephyr hoists registration,
matching, creation, AND association to **build time** (`DEVICE_DT_INST_DEFINE`
under `DT_INST_FOREACH_STATUS_OKAY`, uart_pl011.c:789-793), and on the
platform path even `get_resource_handle` is build-time
(`DEVICE_MMIO_ROM_INIT(DT_DRV_INST(n))`, uart_pl011.c:759). Everything from the
resource preamble through `on_init` becomes the body of the per-instance
**init function** the kernel calls at boot. Hoisting preserves relative order;
swapping the host `(base,size)` literal for the DT `reg` property or the PCIe
BAR probe's MBAR (`pcie_get_mbar` in generated code — the BAR-number accessor,
per-construct §B.3.4) changes **no** register-access op (invariant **I04**,
`prelude.rs:158-162`).

**2. Authored `RegseqError` is a different channel from native OS failures.**
On Zephyr both surface as a negative errno from the init function, but they
must not be conflated: an authored `RegseqError` from `on_init` crosses the
errno exit-adapter (`return -EINVAL;`, the negated form of `errno.rs:35-48`);
a compiler-generated preamble call's native status (`-ENODEV` on a missing
PCIe BDF, eth_e1000.c:215-217; reset/pinctrl failures, uart_pl011.c:584-588)
propagates **directly** and is never wrapped. There is no `-EPROBE_DEFER`
analogue to protect — but conflation would still erase native semantics.

**3. The teardown half of the construct largely has no Zephyr carrier.**
Classic Zephyr device objects are permanent statics: no `.remove`, no unbind,
no hot-unplug path exists in either vendored driver
(`DEVICE_DT_INST_DEFINE`'s argument list has no teardown slot,
uart_pl011.c:789-791; eth_e1000.c:337-344). The bring-up half lowers
idiomatically; orderly/surprise teardown is **explicitly not lowered** on the
3.7-LTS baseline (the 4.x `device_deinit()` fork is an open question), and only
`on_system_shutdown` finds a native optional slot (the PM-device argument).
This is a property of Zephyr's device model, stated with citations — not a
specification gap.

---

## Citation discipline

Every normative claim cites a repo `path:line` **or** a pinned Zephyr doc /
source URL (docs.zephyrproject.org pinned to `3.7.0` — mutable `latest` URLs
are not acceptable for a version-pinned spec (Codex review issue 8);
github.com/zephyrproject-rtos pinned to `v3.7-branch`/`v3.7.0`;
elixir.bootlin.com pinned to `v3.7.0`). Shorthand basenames used throughout
resolve per the legend in
[`zephyr-traceability-matrix.md`](./zephyr-traceability-matrix.md):
`body_emit.rs`/`prelude.rs`/`errno.rs`/`naming.rs`/`mod.rs` under
`crates/regseq-backends/src/generic_c/`; `domain_attr.rs`/
`device_lifecycle.rs`/`target.rs` under `crates/regseq-model/src/`;
`uart_pl011.c`/`uart_pl011_registers.h`/`arm_pl011.yaml` under
`reference-drivers/pl011/zephyr/`; `eth_e1000.c`/`eth_e1000_priv.h`/
`intel,e1000.yaml` under `reference-drivers/e1000/zephyr/`. Zephyr-API claims are additionally
corroborated by an in-repo `file:line` into the vendored stock drivers
(`reference-drivers/{pl011,e1000}/zephyr/`) wherever one exists — the in-repo
witness is preferred over the doc URL. Live fetching of Zephyr URLs was not
available in the authoring session, so those URLs are cited as canonical
locations rather than with fetched anchors; claims that rest on a URL *beyond*
what the vendored drivers corroborate are flagged as open questions
(notably the `pcie_conf_read/write` word-index signature, BRM-O1). A
`generic_c` `none` (the model recognizes a primitive but the reference lowers
nothing) is a deliberate fact, never an omission — it produces a
`recognized-not-lowered` / `needs-new-surface` disposition, not an invented
Zephyr call.

---

## Target pin

> **Pin supersession (2026-06-10, backend-zephyr-c unit, GRILL D05/D15).**
> The normative pin for the `zephyr_c` backend is now **Zephyr v4.4.0**
> (tag `v4.4.0`, commit `684c9e8f32e4`), aligning the spec with the eval
> harness (`versions.lock`: Zephyr v4.4.0 + SDK 1.0.1 + `qemu_cortex_a53`).
> The 3.7-pinned citations throughout these documents **remain as authored
> evidence**: the emitted API surface (`DEVICE_DT_INST_DEFINE`,
> `DEVICE_MMIO_*`, `sys_io`, `LOG_*`, `BUILD_ASSERT`/
> `DT_NUM_INST_STATUS_OKAY`) is stable across 3.7→4.4 and stays corroborated
> by the vendored drivers. The unit-load-bearing C-series pins (C-9 guard
> macros; C-5 `init_res`/`device_is_ready`) are re-verified against the
> local v4.4.0 checkout — see the verification notes at the end of
> [`zephyr-open-questions.md`](./zephyr-open-questions.md) §C. Teardown
> stays fail-loud despite the re-pin (D06): `device_deinit()` exists on
> 4.4 but is unverified/undesigned — delivered by `spec-convergence-pl011`.
> The full mechanical re-cite of 3.7 URLs is folded into the A-19 sweep.

**Zephyr 3.7 LTS, forward-compatible with 4.x.** The single most consequential
sub-pin is **platform/DT vs PCIe**, which changes the realization of half the
BRM rows — and unlike the foundation docs' original framing, **Zephyr-over-PCI
is resolved affirmatively**: the vendored e1000 port proves Zephyr has a
first-class PCIe host layer (`#include <zephyr/drivers/pcie/pcie.h>`,
eth_e1000.c:18) and a complete bring-up idiom (eth_e1000.c:208-262), so
`BusType::Pci` is lowerable on Zephyr, with version-pinning caveats carried as
open questions. Version-sensitive choices (the 4.x `device_deinit()` teardown
fork; PM-device trigger semantics at poweroff; the `pcie_conf_*` signature;
`sys_read64` arch-conditionality) are flagged in
[`zephyr-open-questions.md`](./zephyr-open-questions.md) rather than silently
resolved.
