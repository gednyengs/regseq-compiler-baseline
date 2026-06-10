# Backend lowering — the OS-neutral foundation

This directory is the **OS-neutral foundation** for authoring new OS backends
(Linux, Zephyr, …). It does not specify any one target. It specifies what every
target backend shares: the API a backend reads out of `regseq-model`, the seam
between the reusable agnostic core and the OS-specific shell in the reference
backend, a repeatable per-OS analysis method, and the authoritative census of
in-scope lowering primitives.

It is grounded in the **as-built** reference backend. `generic_c`
(`crates/regseq-backends/src/generic_c/`) is the only existing backend and the
reference lowering; it is a **host model** (it emits a single C translation unit
against a faked register file / bus model with tracing hooks), so every claim
about a real OS realization (`pci_register_driver`, `devm_ioremap_resource`,
`DEVICE_DT_DEFINE`, `readl`/`ioread32`, …) is marked **backend-[Open]** and
deferred to the per-OS sweep, never asserted as done.

---

## Scope

**In scope — exactly five constructs:**

1. **register-access** — MMIO read/write/field/modify, `poll_until`, the
   `map_region` keystone seam.
2. **bus-resource-management** — the four compiler-managed preamble ops
   (`enable_device` / `claim_regions` / `set_bus_master` / `get_resource_handle`),
   `register_driver`, config-space intrinsics, the `MmioBinding` / `BusType`
   model.
3. **device-lifecycle** — the five callbacks, the four scenarios, the managed
   steps, the typed match set.
4. **diagnostics-logging** — the four log levels.
5. **RegseqError** — the framework marker, the nine-variant errno taxonomy, the
   exit-adapter boundary, and its hard separation from native OS errors.

**Out of scope:** interrupts, DMA, serial / data-path. (Where an out-of-scope
fact leaks in — e.g. the `dma_present` bit gating `set_bus_master` — it appears
only as a gate input, never as a lowered primitive.)

---

## Read order

Read these in order; each builds on the last.

1. **[`regseqmodel-input-contract.md`](./regseqmodel-input-contract.md)** — *the
   API a backend consumes.* The `RegseqModel` fields you traverse; items /
   instances / signatures / bodies; the domain-attribute recognition layer; the
   lifecycle scenario model; the BRM binding model and the `map_region` seam; the
   diag-log calls; and the RegseqError flow with its authored-vs-native boundary.
   Start here — it defines every term the other docs use.

2. **[`agnostic-core-vs-os-shell.md`](./agnostic-core-vs-os-shell.md)** — *the
   seam.* A bank-by-bank classification of `generic_c`'s actual multi-bank
   assembly (name tables, ABI/type layout, helper-demand, statics/consts, body
   lowering, entry-shim, errno adapter) into reusable-core vs OS-shell. The seam
   is not a per-primitive boolean — it runs *through* each bank along one rule:
   **mechanism is core, the materialization target is shell.** Ends with a reuse
   table (keep verbatim vs replace).

3. **[`model-analysis-strategy.md`](./model-analysis-strategy.md)** — *the
   method.* A repeatable per-OS sweep: for each primitive, assign one of
   **native-materialization / shared-procedural-core / not-applicable /
   needs-new-surface**, with a citation. Includes the Linux (6.6+, PCI vs
   platform/DT) and Zephyr (3.7/4.x, DT) target pins, a worked sweep skeleton,
   the native-vs-authored error rule, and a completeness check.

4. **[`primitive-inventory.md`](./primitive-inventory.md)** — *the census.* The
   authoritative table of every in-scope primitive with `status`, `model_source`,
   and `generic_c_source`. This is the list the sweep method walks. Keep it open
   alongside the strategy doc.

---

## The two facts to internalize first

**1. The cross-construct composition is real and load-bearing.** The constructs
do not lower in isolation. `generic_c` orders the bus preamble **inside**
lifecycle synthesis — `enable_device → claim_regions → set_bus_master →
get_resource_handle → map_region` (`body_emit.rs:5197-5285`) — and
register-access's `map_region` **consumes** the BRM-resolved `(base, size)`. A
spec that lowers a primitive in isolation, or reorders this preamble, is wrong
even if each primitive looks right alone. Swapping the host `(base,size)` source
for a real `pci_resource_start` / `DT_REG_ADDR` changes **no** register-access
operation (invariant I04, `prelude.rs:160-162`) — that is precisely what makes
the seam OS-portable.

**2. Authored RegseqError is a different error channel from native OS failures.**
The author's `RegseqError` (returned from a callback body) crosses the lossy
exit-adapter boundary; a compiler-generated preamble call's native status
(`-EPROBE_DEFER`, `-ENODEV`, …) propagates **directly** and is never wrapped.
The boundary is the authored/generated code split. Conflating them breaks probe
deferral and erases platform semantics.

---

## Citation discipline

Every normative claim in this directory cites a repo `path:line` **or** a pinned
OS-doc URL. There are no uncited assertions. A `generic_c_source = none` in the
inventory is a deliberate fact (the model recognizes the primitive but the
reference backend lowers nothing), not an omission. Where the reference backend
does **not** lower something — `poll_until` → timeout, `BarType::Io`, `ClassTag`,
the `TargetOs` framework selection, and the gate-lowered (dormant)
`set_bus_master` — it is called out explicitly rather than papered over with an
invented lowering.
