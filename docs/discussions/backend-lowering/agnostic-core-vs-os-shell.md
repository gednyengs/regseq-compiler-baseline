# The agnostic core vs the OS shell — where the seam actually runs

**The thesis.** `generic_c` is the only existing backend and the reference
lowering, but its reusable value is **not** "statement emission." Its real
*agnostic core* is the **multi-bank single-translation-unit assembly** in
`generic_c/mod.rs` — the name tables, the ABI/type layout, the helper-demand
tracking, the statics/consts handling, the body-lowering spine, the entry-shim
classification, and the errno adapter.

**The verdict, stated precisely:** the core/shell split below is a **correct
conceptual decomposition of `generic_c` as built — not an as-built plugin
interface**. No materialization-layer seam exists in the code today: the
assembly pushes the generic-c prelude blocks directly into the output
(`mod.rs:296-324`), and `body_emit` hard-codes the host-hook names at the emit
sites — the MMIO accessor pair (`body_emit.rs:2271-2281`), the bus
registration line (`:5150-5153`), and the map-region preamble sequencing
(`:5271-5283`). A new OS backend (Linux, Zephyr) can **preserve the structure**
this document classifies, but it could not simply swap a materialization
layer — factoring one out of those sites is what a shared-core approach *would*
require, **if** that approach were taken. The ratified backend contract takes
the other path: each backend lowers RegseqModel directly, treating `generic_c`
as the authoritative reference lowering, with no such refactor prerequisite
([`../linux-backend/linux-open-questions.md`](../linux-backend/linux-open-questions.md)
§F.1).

The seam between "reusable core" and "OS-specific shell" is therefore **not a
per-primitive boolean** ("is `mmio_read` portable? yes/no"). It runs *through*
each bank: nearly every bank has a model-driven **mechanism** (what to emit,
in what order, named how, demanded by whom) and an OS-specific **materialization
target** (the concrete `regseq_*` host hook, the synthesized `main`, the errno
numeric). The single recurring rule:

> **Mechanism = core. Materialization target = shell.**
> Whatever computes *structure* from the model is reusable; whatever names the
> *concrete output construct* (a host hook, a `main`, an `EINVAL`) is the shell.

This document classifies each bank of the actual assembly against that rule and
ends with a reuse table a backend author can act on.

---

## The assembly, as built

`emit_generic_c_inner` (`generic_c/mod.rs:237-340`) is the whole pipeline. It
builds a name table + an instance-symbol table, discovers + topo-sorts the type
graph, recovers const-memory-static storage, emits five output banks, and
appends the errno adapter and the lifecycle synthesis:

```
  Bank 1   prelude + demand-driven helper block         mod.rs:297-324
  Bank 2   type forward-decls                            mod.rs:271, :1230
  Bank 3   type definitions (topo-sorted)                mod.rs:272, :1277
  Bank 4   prototypes + extern statics                   mod.rs:273, :2180
  Bank 5   function bodies + static initializers         mod.rs:274, :2256
  + errno adapter (entry == ErrResult)                   mod.rs:280-294, :329-331
  + lifecycle synthesis (TU has drivers)                 mod.rs:336-338
```

Supporting that output are six pieces of machinery built up front: the
`NameTable` (mod.rs:86), the `InstanceSymbols` table (mod.rs:118), the
discovery/topo type-graph walk (mod.rs:884, :949), the C-ABI effective-type
resolver (mod.rs:1330-1362), the `HelperUsage` demand tracker
(prelude.rs:338), and the const-memory-static storage recovery
(mod.rs:1628). The classification below covers all of it.

---

## Bank: name tables — **CORE** (with a shell-specific reserved-name input)

`NameTable` (mod.rs:86, built `:981`) and `InstanceSymbols` (mod.rs:118, built
`:1012`) map every model node / instance to a final, collision-free C
identifier. The whole thing is a **pure function of the model**: derive a base
name (`naming::aggregate_base_name` / `instance_symbol_base`), apply the
external-vs-internal linkage rule (mod.rs:1043-1054; naming.rs:279-287), run the
`_clash_<n>` net.

This is **reusable core**. Any C-targeting backend needs identifiers, the
external-linkage ABI contract (suffix-free, stable across monos), and the clash
net — none of it is OS-specific.

The **one shell-flavored input** is the *reserved-name set* — and the as-built
set is **narrower than the prelude**. `prelude_ordinary_idents`
(mod.rs:1099-1148) reserves only the panic/unreachable traps, the
arithmetic/cast helper family, and — when the TU has lifecycle drivers — the
`regseq_lc_*` trace hooks. It does **not** reserve the always-emitted
`regseq_mmio_read`/`regseq_mmio_write` (prelude.rs:87-102) or the conditional
`regseq_bus_*` helpers (prelude.rs:188-224); an author symbol colliding with
those is a latent gap in the reference backend, not a behavior to inherit. The
**reservation mechanism** is core; a new backend supplies its own *complete*
set (its full helper/macro namespace — e.g. Zephyr's `LOG_*` /
`DEVICE_DT_*`), not generic-c's partial one.

---

## Bank 1: prelude + helper block — **SPLIT** (core runtime support vs pure shell, core demand mechanism)

Bank 1 (`prelude.rs`, emitted `mod.rs:297-324`) is the clearest split in the
whole assembly, because the file *physically separates* the two halves.

**Core runtime support** (reusable, OS-neutral):
- The arithmetic / cast helper library — `regseq_{add,sub,mul}_<T>` over
  `__builtin_*_overflow` and the saturating `regseq_<f>_to_<T>` casts, emitted on
  demand by `helper_block` (prelude.rs:426-514, emit at :457-511) — together with
  the always-present host `regseq_panic` / `regseq_unreachable` traps
  (prelude.rs:50-59). These implement Rust's checked-arithmetic and panic
  semantics in C; any C backend needs them.
- The monomorphic `struct rust_str` fat-pointer (prelude.rs:41-44).

**Pure OS-shell** (the host model — every line here is a *fake* of hardware/OS
that a real backend replaces):
- The register-access host device model: `regseq_mmio_read` / `regseq_mmio_write`
  over a flat `regseq_regfile[]` with tracing (prelude.rs:85-102).
- The `LIFECYCLE_HELPERS` trace hooks `regseq_lc_*` (prelude.rs:115-148).
- The `BUS_RESOURCE_HELPERS` host bus model — `regseq_bus_register_driver`,
  `regseq_bus_enable_device`, `regseq_bus_claim_regions`, `regseq_bus_set_master`,
  `regseq_bus_get_handle`, `regseq_bus_map_region`, the based accessors
  `regseq_bus_mmio_read/write`, and the config-space model
  `regseq_bus_on_config_read/write` (prelude.rs:175-296).

Every one of these host hooks **documents its real-target realization as
backend-[Open]** in its own comment — `register_driver` →
`pci_register_driver`/`module_amba_driver`/`DEVICE_DT_DEFINE` (prelude.rs:186-189);
`claim_regions` → `pci_request_selected_regions` / `devm_ioremap_resource` /
Zephyr-no-op (prelude.rs:202-208); `map_region`'s `(base,size)` →
`pci_resource_start`/`DT_REG_ADDR` (prelude.rs:160-162). **This shell *is* the OS
backend's main job: replace the host hook library with target API calls.**

**Core demand mechanism** (reusable): *which* helpers get emitted is computed,
not fixed. `HelperUsage` (prelude.rs:338) records each helper a body actually
emits; `helper_block` walks the fixed tables and emits only the recorded ones
(prelude.rs:457-511); the `<errno.h>` include is likewise demand-driven
(prelude.rs:434-439, recorded at mod.rs:290). And the *conditional emission* of
the three host blocks — `LIFECYCLE_HELPERS` only when `drivers` is non-empty,
`BUS_RESOURCE_HELPERS` only when a binding/bus/config intrinsic is present
(mod.rs:309-324) — is a core gating mechanism that keeps a TU byte-minimal. A
backend reuses the demand-tracking machinery and re-points the *contents*.

---

## Banks 2–3: type forward-decls + definitions (ABI / type layout) — **CORE**

Banks 2 (mod.rs:1230) and 3 (mod.rs:1277), plus all the machinery feeding them
— `discover` (mod.rs:884), `topo_sort` (mod.rs:949), the by-value containment
edge set (`agg_edges` :702, `containment_target` :837), the fat-pointer node
shapes (`Node::Slice`/`TraitObj` :58), the `Ty`→C spelling (`types`), the
section-14 layout emitter (`layout_emit`), the enum union-of-views
(`enum_emit`), and the `_Static_assert` layout guards — are **reusable core in
full**.

This is the "type-system foundation": translate the model's `Ty`
interner into C struct/union/enum definitions, topologically ordered so each is
complete before it is contained by value, with layout asserts. None of it reads
an OS identity. The C-ABI effective-type resolver (mod.rs:1330-1362,
`effective_input_ty`/`effective_output_ty`) is part of this core: it decides the
*boundary* spelling of each signature position, which is an ABI fact, not an OS
fact.

A Linux/Zephyr C backend emits driver structs, register-block layouts, and the
`Result`/`RegseqError` enum **identically**. (A non-C backend would replace this
bank wholesale, but within the C-target family it is pure reuse.)

---

## Banks 4–5: declarations, definitions, statics/consts — **CORE**

Bank 4 (mod.rs:2180) emits prototypes + `extern` statics; Bank 5 (mod.rs:2256)
emits function definitions + static initializers. The supporting statics/consts
machinery — the const-memory-static storage recovery (`const_memory_static_storage`
mod.rs:1628, `StorageTy` :1586, the `scan_*` walkers :1655-2068) that re-types a
bridge `Array(u8,N)` to its sound consumer pointee — is **reusable core**: it is
a strict-aliasing-correctness concern, OS-independent.

The qualifier prefixes (`inline`/`weak`/`section`, mod.rs:1453-1472) and the
signature spelling (`fn_signature` :1480) are core too. The **only** OS-shaped
decision in these banks is *exclusion*: a diag-log-carrying instance is skipped
(its body is replaced by the §Bank-1 lowering, not emitted) — but that exclusion
is driven by model recognition (`is_diag_log_instance`, mod.rs:1563), so even it
is core mechanism keyed on a model attr.

---

## Body lowering — **SPLIT, and this is the cross-construct seam**

`body_emit::emit_body` lowers each `Body` to a C block. It has two clearly
separable halves.

**The procedural spine — CORE.** Lowering `Assign`/`SetDiscriminant`/`Intrinsic`
statements, `Goto`/`SwitchInt`/`Return`/`Unreachable`/`Call`/`Assert`
terminators, every `Rvalue`/`Operand`/`Place` projection, the C-ABI call-site
marshalling — this is the OS-neutral translation of the MIR-shaped IR
(§3 of [`regseqmodel-input-contract.md`](./regseqmodel-input-contract.md)). It
is identical for every C backend.

**The domain-intrinsic interception — the SEAM.** Inside the ordinary `Call`
path, **three** constructs are intercepted by instance identity and lowered to
host hooks instead of a C call (`body_emit.rs:2100-2125`). The fourth
recognized intrinsic family, `PollUntil` (`domain_attr.rs:88-100`), has **no**
emit arm — recognized-not-lowered
(`docs/discussions/register-access/register-access-surface-convergence.md:424-427`):

| recognition (CORE) | materialization (SHELL) |
|---|---|
| `IntrinsicKind::Log(level)` (body_emit.rs:2101) | `emit_diag_log` → `fwrite` (`:2722`) ⇒ a real backend: `dev_*` / `LOG_*` |
| `IntrinsicKind::Mmio(op)` (body_emit.rs:2113) | `emit_mmio` → `regseq_mmio_*` / `regseq_bus_mmio_*` (`:2242-2326`) ⇒ `readl`/`writel` / `ioread32` / `DEVICE_MMIO` |
| `IntrinsicKind::Config(op)` (body_emit.rs:2122) | `emit_config` → `regseq_bus_on_config_*` (`:2344-2397`) ⇒ `pci_read_config_*` |

The **recognition** (reading `instance_intrinsic`, dispatching by `IntrinsicKind`)
is reusable core — a new backend keeps it verbatim. The **register/field
arithmetic** inside `emit_mmio` (offset recovery from `#[repr(C)]` layout, the
`(read & mask) >> shift` field read, the RMW `modify`) is *also* largely core:
it is the construct's semantics, not the OS's. What is **shell** is the final
call target — the `regseq_mmio_read(off,width)` host hook vs `readl(addr)`.

A precise demonstration of where the seam sits: the bus-base switch at
`body_emit.rs:2271-2281`. When the TU has a resolved binding, `emit_mmio` routes
through the *based* accessor `regseq_bus_mmio_read` (absolute mapped address)
instead of the faked-base `regseq_mmio_read` — **with identical arguments**. The
code comment states the invariant outright: "identical args, so no register-access
op changes (I04); this is the single difference the keystone makes to
register-access lowering." That is the seam in one line: the *operation* (core)
is fixed; only the *accessor it lands on* (shell) moves.

### The cross-construct composition lives in the core

The lifecycle synthesis (`emit_lifecycle_synthesis`, `body_emit.rs:5088-5382`)
is where the constructs compose, and it is **predominantly core mechanism**: it
computes the canonical preamble ordering
(`register → create → associate → preamble → [enable_device → claim_regions →
set_bus_master → get_resource_handle → map_region] → on_init → teardown`,
`:5197-5285`), the PCI/DMA gating matrix (`:5251-5270`), the forward/reverse
resource order (`:5214-5217`, `:5403-5406`), and the init-fail
niche-discriminant **inspection** (`:5421-5483`). As built, the init-fail path
is inspection only: it emits the `on_init -> Err` marker and the optional
`on_init_failure` call (`:5470-5482`) — **no resource rollback / high-water-mark
release is performed yet**; the partial reverse postamble is deferred until a
resourceful init-fail fixture exists
(`docs/discussions/regseq-device-lifecycle/device-lifecycle-surface-convergence.md:222-226`).
All of that **ordering and gating logic is what a new backend must preserve** —
it is the construct composition, not the OS.

The **shell** parts of the synthesis are, again, the materialization targets: the
emitted `regseq_lc_*` / `regseq_bus_*` host-hook call lines (replaced by
`pci_register_driver` + `pcim_enable_device` + `pci_request_regions` + `pci_iomap`
on Linux-PCI, or `DEVICE_DT_DEFINE` + `DEVICE_MMIO_MAP` on Zephyr), and the
Policy-P container-morph struct shape (`:5184-5189`), whose `void *__rseq_dev` /
`uint32_t __rseq_state` stand in for the real OS device handle / state. The
*ordering* is core; the *lines emitted at each step* are shell.

---

## Entry-shim classification — **SPLIT** (classification core, shim shape shell)

Resolving the entry and classifying its return shape — `resolve_entry`
(mod.rs:432), `classify_entry_return` (mod.rs:565), the
`UnitVoid`/`ErrResult`/`LifecycleOnly` decision (naming.rs:73) — is **core**: it
reads the model's entry signature, the certified `is_result` flag, and the
`FrameworkType` marker. A new backend makes the *same* classification.

The **materialized shim** is **shell**: `generate_main_c` (naming.rs:131-197)
emits a hosted `int main(void)` that calls the entry / adapter / lifecycle
runner. A Linux backend has no `main` — it emits `module_init`/`module_exit` +
`.probe`/`.remove`; a Zephyr backend emits `DEVICE_DT_DEFINE` init hooks. The
**decision** (does this entry succeed-void, return an error, or is the program
just the lifecycle runner) is reused; the **C scaffold** that realizes it is
replaced.

---

## Errno adapter — **SPLIT** (read+switch mechanism core, errno table + exit form shell)

The `RegseqError → errno` exit-adapter (`emit_exit_adapter`,
`body_emit.rs:4732-4828`) is the authored-error boundary materialization.

**Core mechanism:** read the `Result` niche discriminant
(`niche_discriminant_read_block`, `:4654-4707` — *shared verbatim* with the DL
init-fail path, `:5466`), branch `Ok → success`, otherwise switch the inner
`RegseqError` tag and emit one arm per variant, **failing loud on an unmapped
marked variant** (`EmitError::ErrnoMapping`, mod.rs:2367). This discriminant-read
+ switch-synthesis skeleton is reusable.

**Shell:** the C02 mapping table itself (`errno.rs:35-48`,
`InvalidParameter→EINVAL`, …) and the *form* of the result — `return EINVAL;`
from an `int` adapter feeding a process exit code. The table file's own header
says it plainly: "the host-error mapping is a per-target concern, not a property
of the target-agnostic model" (errno.rs:6-9). A Linux backend maps the same
variants to **negative** errnos returned from `.probe` (`return -EINVAL;`); a
Zephyr backend maps to its own `-E*` set. The keys (the `RegseqError` variant
names) are model facts; the *values and the return convention* are shell.

This is also where the **authored/native error boundary** (the §8 rule in the
input contract) shows up structurally: the adapter only ever materializes the
**authored** channel. Native preamble failures (`devm_ioremap_resource`,
`clk_get`, …) are a *separate* shell concern a real backend wires straight to the
native return path — they never pass through this adapter. `generic_c`, a host
model with no failing native calls, materializes only the authored side today.

---

## Reuse table — what a new C backend keeps vs replaces

| bank / machinery | classification | reuse verbatim | replace (the shell) |
|---|---|---|---|
| Name tables (`NameTable`, `InstanceSymbols`) | core | derivation, linkage rule, clash net | the reserved-name set (as built only panic/arith/cast + conditional `regseq_lc_*`, mod.rs:1099-1148 — a new backend supplies its *complete* helper/macro namespace) |
| Bank 1 helpers (arith/cast/panic) | core | the whole helper library + demand tracking | — |
| Bank 1 host model (`regseq_mmio/bus/lc_*`) | **shell** | the conditional-emission gating | **every host hook** → target API |
| Banks 2–3 (type layout, topo, enum, ABI) | core | the whole type-system foundation | — (within the C-target family) |
| Banks 4–5 (decls, defs, statics/consts) | core | signatures, qualifiers, CMS re-typing | — |
| Body spine (stmt/term/rvalue/place/call) | core | the whole procedural translation | — |
| Domain-intrinsic interception | **seam** | recognition + construct arithmetic | the call target (host hook → `readl`/`dev_*`/config) |
| Lifecycle synthesis | **seam** | preamble ordering + gating + init-fail inspection (rollback deferred) | the emitted host-hook lines + container-morph shape |
| Entry-shim classification | core | the `UnitVoid`/`ErrResult`/`LifecycleOnly` decision | `generate_main_c` → `module_init`/probe/`DEVICE_DT_DEFINE` |
| Errno adapter | core | discriminant read + switch synthesis + fail-loud | the C02 table values + the exit-code/return convention |

**The takeaway for a backend author:** you are not designing a new emitter from
scratch — the structure to preserve is `mod.rs`'s assembly: its name tables,
type layout, body spine, ordering, and classification. What gets substituted is
the *materialization* — the host-hook library (Bank 1), the call targets of the
three intercepted intrinsics + the lifecycle-step lines, the entry scaffold,
and the errno table/return convention. If a shared-core approach were ever
taken, that substitution would be a **refactor, not a plug-in swap**: today the
assembly pushes the generic-c prelude blocks directly (mod.rs:296-324) and the
emit arms hard-code the host-hook names (body_emit.rs:2271-2281, `:5150-5153`,
`:5271-5283`), so a materialization layer would first have to be factored out
at those sites. Under the ratified backend contract, however, no such refactor
is prerequisite — each backend lowers RegseqModel directly, consulting
`generic_c` as the authoritative reference
([`../linux-backend/linux-open-questions.md`](../linux-backend/linux-open-questions.md)
§F.1) — so read this core/shell classification as **analysis**: it tells a
backend author which parts of `generic_c` are OS-neutral reference material and
which are OS-specific decision points to re-decide natively. The next document,
[`model-analysis-strategy.md`](./model-analysis-strategy.md), turns this into a
repeatable per-OS sweep.
