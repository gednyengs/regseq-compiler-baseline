# Linux scaffold & dispatch — the module shell the primitives wire into

The per-construct spec ([`linux-lowering-per-construct.md`](./linux-lowering-per-construct.md))
says what each primitive *becomes*. This document says where those lowered
primitives *live*: the kernel-module scaffold, the registration structs, the
`.probe`/`.remove`/`.shutdown` slots, the scenario→entry-point dispatch, and the
emission of diagnostics-logging and the RegseqError adapter into that scaffold.

The governing fact, established in
[`linux-lowering-per-construct.md`](./linux-lowering-per-construct.md) §C.0: on
Linux **the kernel's bus core is the OS**. `generic_c` synthesizes a driving
`main` + per-scenario functions and an `EntryShim`; Linux emits **none** of that.
The driver registers once at module load, and the kernel drives the canonical
sequence by calling registered callback slots. The four `LifecycleScenario`
variants (`backend_options.rs:24-42`) are therefore **not four functions** — they
are the kernel's invocation paths into one registered callback set.

(For Linux the registration framework is `module_*_driver` + a `pci_driver` /
`platform_driver` / `amba_driver` struct. The Zephyr analogue `DEVICE_DT_DEFINE`
is *not* a Linux artifact; it appears only as the cross-OS contrast.)

---

## 1. Module scaffold — what is emitted once, at module scope

Three things are module-scoped (emitted once, not per device or per scenario):

1. **The device-ID match table** + `MODULE_DEVICE_TABLE` (§C.2). A zero-terminated
   array rendered from the **full** `match_set: Vec<MatchEntry>`
   (`device_lifecycle.rs:51`) — not `generic_c`'s first-only trace render
   (`render_match_trace`, `body_emit.rs:5499-5509`).
2. **The static driver struct**, selected by `BusType` (the BRM bus selector,
   `bus_selector_token` `body_emit.rs:5050-5061`):

   | `BusType` | struct | `.name` source | registration |
   |---|---|---|---|
   | `Pci` | `struct pci_driver` | `regseq_lc_register` name (`:5197`) → `.name` (reference-drivers/e1000/linux/e1000_main.c:181) | `pci_register_driver` / `module_pci_driver` (reference-drivers/e1000/linux/e1000_main.c:224) |
   | `Platform` | `struct platform_driver` | → `.driver.name` | `platform_driver_register` / `module_platform_driver` (reference-drivers/pl011/linux/amba-pl011.c:3203) |
   | `Amba` | `struct amba_driver` | → `.drv.name` (reference-drivers/pl011/linux/amba-pl011.c:3190) | `amba_driver_register` / `module_amba_driver` (reference-drivers/pl011/linux/amba-pl011.c:3205) |

   The struct's `.probe`/`.remove`/`.shutdown` fields point at the synthesized
   callback bodies (§2–§4). `.id_table` points at the match table; `.shutdown` is
   **omitted** when `on_system_shutdown` is not overridden (§4).
3. **The registration call**, idiomatically the collapsing `module_*_driver(drv)`
   macro (which expands to `module_init`/`module_exit` wrapping
   `*_register`/`*_unregister`, reference-drivers/e1000/linux/e1000_main.c:235). This is the **time-split** from
   `generic_c`, where the `register` marker re-prints in every per-scenario
   function. The call's `int` failure is **native** (propagate from `module_init`),
   never wrapped in `RegseqError` (§B.2).

`register_driver` is the *only* step hoisted out of the per-device body. The
`bound` argument `generic_c` traces (`body_emit.rs:5145-5153`) is host-only and
**not-applicable** on Linux: registration always installs `.probe` regardless of
region resolution, because regions resolve at probe time.

---

## 2. `.probe()` — the bring-up callback (the orderly + init-fail entry)

The kernel invokes `.probe()` after a successful table match. Signatures are
bus-shaped: `e1000_probe(struct pci_dev*, const struct pci_device_id*)`
(reference-drivers/e1000/linux/e1000_main.c:915); `pl011_probe(struct amba_device*, const struct amba_id*)`
(reference-drivers/pl011/linux/amba-pl011.c:2971). The bring-up steps lower into its body in canonical order
(`generic_c` `body_emit.rs:5206-5311`):

```
create     → devm_kzalloc / alloc_etherdev (zeroed private container, §C.7)
associate  → pci_set_drvdata / amba_set_drvdata / SET_NETDEV_DEV
preamble   → [ BRM block: enable_device → claim_regions → set_bus_master
               → get_resource_handle → map_region ]   (the cross-construct seam)
on_init    → author bring-up body; Ok→return 0 / Err→goto ladder→return -errno
```

The **BRM block is nested inside the preamble slot** (`body_emit.rs:5244-5285`),
between `associate` and `on_init`, in the exact canonical order — device-lifecycle
owns the slot + forward ordering, BRM/register-access own the per-op calls. See
[`linux-integration-probe-path.md`](./linux-integration-probe-path.md) for the
full ordered seam and the I04 keystone.

Two failure exits leave `.probe()`, and they are different channels (§5):
- **native** preamble status (`pci_enable_device` etc.) → `if(err) goto err_…;` /
  `return err;` directly (reference-drivers/e1000/linux/e1000_main.c:939-944);
- **authored** `on_init` `RegseqError` → the negated errno adapter (§6).

Both unwind through the goto-error ladder's shared reverse-release blocks (§3),
but **only the authored exit runs `on_init_failure` and the errno adapter** — a
native preamble failure means `on_init` never ran, so the authored hook must not
fire (§3). `.probe()` returns the first failing errno; `0` on success.

---

## 3. The `.probe()` goto-error ladder — init-fail dispatch

`generic_c` realizes init-fail as a separate `init-fail` scenario function that
captures `on_init`'s `Result`, reads the niche discriminant, and on `Err` emits
`regseq_lc_on_init_err()` + the `on_init_failure` marker + author body
(`emit_init_fail_body`, `body_emit.rs:5421-5483`), asserting via stdout and exiting
**cleanly**. On Linux this is **not a separate function** — it is `.probe()`'s
labelled `goto`-cleanup ladder, which has **two disjoint families of entries**
(the §5 channel split, realized in control flow):

- **Native-unwind entries** — a compiler-generated preamble call fails
  (reference-drivers/e1000/linux/e1000_main.c:939-944). The jumped-to label releases what was acquired so far
  and returns the native errno **directly**. `on_init` never ran on this path,
  so **`on_init_failure` is NOT called**: as-built the hook runs only after
  `on_init`'s `Result` discriminant reads `Err` (`emit_init_fail_body`,
  `body_emit.rs:5470-5481`).
- **The authored `on_init -> Err` entry** — the `if (err) goto err_…;` branch
  after the authored `on_init` call (reference-drivers/e1000/linux/e1000_main.c:971-972 shape) — no standalone
  artifact. **This is the only entry that runs `on_init_failure`**: the author
  cleanup hook, emitted **inside** its cleanup block **before `return -errno`**
  (the §6 adapter). Because the kernel does **not** call `.remove()` on probe
  failure, `on_init_failure` + the partial postamble **is** the failure cleanup;
  `on_deinit` does *not* run.
- The **partial high-water-mark reverse postamble** is shared by both families:
  release exactly the resources acquired so far, in reverse (`err_ioremap:
  iounmap → free_netdev → pci_disable_device`, reference-drivers/e1000/linux/e1000_main.c:1224-1239). This HWM
  rollback is **deferred as-built** in `generic_c` (its init-fail fixture is
  fieldless) — on Linux it is mandatory: the backend must thread the
  acquired-resource list into the ladder. `devm_*` resources unwind
  automatically (returning non-zero from `.probe` triggers the devres stack to
  release in reverse).

The behavioral oracle diverges: `generic_c` asserts via stdout trace + clean exit;
Linux's oracle is the **negative errno returned from `.probe()`** (the kernel logs
it, the device stays unbound).

---

## 4. `.remove()` and `.shutdown()` — teardown dispatch

`generic_c` routes teardown through `emit_teardown_dispatch`
(`body_emit.rs:5391-5407`) and emits **two byte-distinct functions** for orderly
vs surprise. Linux funnels both into the **one** `.remove()` slot (reference-drivers/e1000/linux/e1000_main.c:1252;
reference-drivers/pl011/linux/amba-pl011.c:3029), branching on a runtime `present` predicate:

| `generic_c` scenario | `present` | Linux entry | shape |
|---|---|---|---|
| orderly (`:5317-5322`) | 1 | `.remove(dev)` | HW reachable → full reverse release |
| surprise (`:5328-5339`) | 0 | `.remove(dev)` device-gone branch | HW-touching releases **skipped** |
| shutdown (`:5360-5368`) | n/a | `.shutdown(dev)` | quiesce only, no full teardown |

- **`teardown` present-dispatch** (`regseq_lc_teardown(present)`, `:5398`) → a
  runtime predicate at the top of `.remove()` (`pci_device_is_present()` /
  `pci_channel_offline(pdev)`), not two functions.
- **`on_deinit`** (orderly) / **`on_surprise_remove`** (device-gone) → the author
  body inlined on the matching branch. The `.remove()` return is bus-shaped:
  PCI and AMBA `.remove` are **`void`** (reference-drivers/e1000/linux/e1000_main.c:1252;
  reference-drivers/pl011/linux/amba-pl011.c:3029), while platform at v6.6 still carries `int (*remove)` + `void
  (*remove_new)` and by v6.13 only `void (*remove)` (`.remove_new` retired) —
  this spec pins the **version-gated member**: `.remove_new` when
  `LINUX_VERSION_CODE < KERNEL_VERSION(6, 11, 0)`, `.remove` from v6.11 on
  (C-3, resolved; v6.6 `platform_device.h:236-250`, v6.13
  `platform_device.h:236-239` — pinned URLs in C-3). Both arms are `void`, so
  on every pinned slot the `Result` is
  structurally **discarded** (CR-4).
- **`postamble`** (reverse) → the resource-release span, exact mirror of the
  `.probe()` acquire: `iounmap` (reference-drivers/e1000/linux/e1000_main.c:1275) → `pci_release_selected_regions`
  (:1278) → `pci_disable_device` (:1284). `skipped(surprise)` → skip HW pokes on the
  device-gone branch. For `devm_*` resources the release is **automatic** — the
  postamble emits nothing (`pl011_remove` writes no manual unmap).

**`.shutdown()`** is the lone **omit-when-absent** slot (§C.6, D10/I02): emitted
only when `on_system_shutdown` is overridden, pointing at the author body
(`pci_driver.shutdown` reference-drivers/e1000/linux/e1000_main.c:186; for AMBA the `.drv.shutdown` on
`device_driver`). Returns `void`; quiesce only, no resource teardown. When absent,
the `.shutdown` initializer is left out of the driver struct entirely.

---

## 5. The two error channels at the scaffold boundary

Every callback return is a potential errno boundary, and the scaffold must keep two
channels distinct (the hard rule, [`linux-model-analysis.md`](./linux-model-analysis.md)
§*native-vs-authored*; `model-analysis-strategy.md:197-221`):

1. **Authored `RegseqError`** — only values the author constructed in a callback
   body (`on_init`, …). Crosses the §6 negated-errno adapter → `return -EINVAL;`
   from `.probe()`.
2. **Native kernel status** — from compiler-generated preamble calls. Propagates
   **directly** (`return err`, `goto err_…`, `return PTR_ERR(base)`), **never**
   wrapped. `-EPROBE_DEFER` must survive untouched — the §6 adapter only emits the
   nine C02 codes (+ defensive `-EIO`), so it can never synthesize or overwrite it.

The boundary is **authored-vs-compiler-generated code**, not syntax. Both surface
as a negative errno from `.probe()`, which is exactly why the scaffold must not
collapse them.

---

## 6. RegseqError adapter — emitted at the callback return site

`generic_c` emits a standalone `int <entry>__regseq_exit(void)` adapter
(`emit_exit_adapter`, `body_emit.rs:4732-4828`) that the harness `main` calls. On
Linux there is no harness `main`; the adapter is **relocated into the `.probe()` /
callback return** with three substitutions and nothing else (§E.3):

1. **Negate every error arm**: `case <tag>: return -<SYMBOL>;`, `default: return
   -EIO;`. The `Ok` arm stays `return 0;`.
2. **Relocate** from the standalone symbol + `main` shim to the callback return
   site. At `on_init`, the discriminant read is **fused** with the §3 init-fail
   cleanup: the synthesized lifecycle path already reads `on_init`'s `Result`
   discriminant (`body_emit.rs:5439-5476`, sharing `niche_discriminant_read_block`
   at `:5466`); on Linux, `Err` runs the cleanup ladder **and then** `return
   -<errno>`. (D-2: `generic_c` keeps these two uses separate — the Linux probe must
   do both.)
3. **Header**: `#include <linux/errno.h>` via the same demand machinery
   (`HelperUsage::record_errno_include`, `prelude.rs:434-438`) — a no-error TU is
   byte-unchanged.

Everything else is reused verbatim: the niche read (`body_emit.rs:4654-4707`), the
tag-view switch (`:4763-4809`), symbolic-only emission (I03), fail-loud on an
unmapped marked variant (`:4807`), and the C02 table (`errno.rs:35-48`) — whose
nine symbols are **identical** on Linux (all valid UAPI errnos). The Linux delta is
**polarity + placement**, not symbols. On the pinned `void` teardown slots
(PCI/AMBA `.remove`; platform `.remove_new`/`.remove` per the C-3 version
gate), a `RegseqError` from
`on_deinit` has **no return channel** and can only be logged (D-3).

---

## 7. Diagnostics-logging — emitted into the scaffold bodies

Each `Log(level)` call (recognized at `body_emit.rs:2100-2104`) lowers to **one**
device-aware macro invocation, reassembling `generic_c`'s decomposed `fwrite`
segments into a single `printk` format string + arg list (§D.3). The scaffold
relevance:

- **The `dev` argument is the scaffold join point — and the unsolved seam.**
  `dev_*` needs the `struct device *` that §1–§2 bind (`&pdev->dev`). But lowered
  bodies are OS-neutral ordinary functions (bank 5, `mod.rs:2276`) and callbacks
  run over `&inst.inner` — **no lowered function has `dev` in scope today**; only
  statements the scaffold itself synthesizes (where `pdev` is a parameter) do.
  Interim rule: emit `dev_dbg/info/warn/err(dev, …)`
  (reference-drivers/pl011/linux/amba-pl011.c:449,460,468,927) **only in scaffold-synthesized statements**;
  every `Log` in a lowered ordinary body (including authored callbacks and their
  helpers) lowers to `pr_*(…)` until the device-context strategy — signature
  threading, container recovery, or intentional `pr_*` fallback — is ratified
  (A-3; the per-construct spec §D.3.2 defines the three options). This is the
  **one real composition seam** for diag-log.
- **Level → macro selection** (the `[LEVEL]` text tag is dropped; severity becomes
  structural via the macro / `KERN_*` prefix). Reassembly reuses
  `diag_specifier_and_cast_for` whole; literal `%` is doubled to `%%`; `&str` →
  `%.*s` + `(int)len,(const char*)data`; a trailing `\n` is mandatory (else
  `KERN_CONT`).
- **No error/exit-adapter interaction.** `Log` returns unit; the v6.6 `dev_*`
  level macros expand to **`void`** functions (`_dev_err`/`_dev_warn`/`_dev_info`,
  `dev_printk.h:34-56,143-160`, pinned URL in §D.4 of the per-construct spec),
  and `pr_*`'s `printk` `int` is conventionally ignored. This channel is entirely
  separate from §5–§6 — it never produces an errno and never touches the
  lifecycle/exit-adapter.

The emitted statement is `dev_<level>(dev, "<F>", <A…>); goto bb_<target>;` — the
`Log` call always has a `Some` target and unit return (`body_emit.rs:2728-2730`),
matching `generic_c`'s CFG-flattened `goto` discipline.

---

## 8. Where `generic_c`'s emission is OS-specific — the Linux decision-point checklist

**Scope caveat (read first).** This table enumerates only the **Linux-divergent
sites** — the places where the Linux emission differs from `generic_c`'s host
shell. It is *not* a complete implementation checklist: everything not named
here (the multi-bank assembly — name/symbol tables, ABI/type layout, helper-
demand tracking, statics/consts recovery, the body bank, entry classification;
`emit_generic_c_inner` `mod.rs:237`, bank 5 `mod.rs:2276`) is OS-neutral
reference material in `generic_c`, the **authoritative reference lowering**, per
[`../backend-lowering/agnostic-core-vs-os-shell.md`](../backend-lowering/agnostic-core-vs-os-shell.md)
and the input contract. Under the ratified backend contract
([`linux-open-questions.md`](./linux-open-questions.md) **§F.1**), this table
documents **where `generic_c`'s emission is OS-specific**: it is the checklist
of decision points a native Linux emission must cover when lowering RegseqModel
directly. The sites below are hard-coded into `generic_c`'s emit paths
as-built — which requires no prerequisite refactor, since the Linux backend is
an independent lowering, not a parameterization of `generic_c`; the table
remains the candidate parameterization list if shared-machinery extraction is
ever evaluated later (after at least two OS backends exist).

| `generic_c` seam | reshape for Linux |
|---|---|
| `emit_lifecycle_synthesis` (`body_emit.rs:5088-5285`) | split the linear preamble into module-init registration (§1) + a `.probe` body (§2); drop the synthesized `main`/runner/`EntryShim` (`mod.rs:439-440`) |
| `bus_selector_token` (`:5050-5061`) | `BusType` → registration struct + call + framework (§1) |
| `emit_teardown_dispatch` (`:5391-5407`) | two byte-distinct fns → one `.remove()` branching on a `present` predicate (§4) |
| `emit_config` (`:2344-2397`) | config accessor selection + status/out-param return-shape marshalling (§B.4) |
| based/faked accessor switch (`:2271-2281`) | collapses to the based form — `base` is always the mapped iomem cookie (§A.1) |
| `emit_exit_adapter` (`:4732-4828`) | negate arms, relocate to the callback return, `<linux/errno.h>` (§6) |
| `emit_diag_log` (`:2722-2849`) | reassemble `fwrite` segments into one `printk` macro call (§7) |
| `prelude::BUS_RESOURCE_HELPERS` (`prelude.rs:175-296`) | host hooks replaced wholesale by real kernel calls |
| the emit gate (`mod.rs:319-323`) | the construct-presence gate that drives whether the scaffold is emitted at all |
