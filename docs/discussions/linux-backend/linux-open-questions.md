# Linux backend — open questions & completeness ledger

Every open decision gathered from the five constructs and the composition, plus
the honest completeness ledger. The list is partitioned into:
**(A) policy decisions to ratify** (a clear recommendation exists; none blocks
specification, though A-3/A-10/A-11/A-12 must be ratified before implementation);
**(B) `needs-new-surface` gaps** (blocking model/reference extensions —
escalate, do not paper over); **(C) version-sensitive pins to confirm against
the target kernel**. Section **(D)** is the completeness ledger, including the
explicit unresolved/failed-construct list; **(E)** records the external-review
status; **(F)** records the implementation contract escalated from that
review — now **resolved by owner ratification** (independent-lowering
contract; see §F.1), not an open decision.

Each item cites the construct it came from and a repo `path:line` or pinned doc.

---

## A. Policy decisions to ratify (recommendation in **bold**; none blocks the *spec*. A-3, A-10, A-11, A-12 — the four implementation-gating items — were **ratified by the owner 2026-06-09**; see the inline RATIFIED notes)

1. **MMIO accessor family — `readl`/`writel` vs `ioread32`/`iowrite32`.**
   `emit_mmio`'s field arithmetic (`body_emit.rs:2283-2323`) is the reusable core;
   which Linux accessor materializes it is version/arch-sensitive. `ioread32`/
   `iowrite32` also work over `pci_iomap` Io cookies, so a uniform `ioread*` choice
   would subsume the `BarType::Io` case (B-2). **Recommend `readl`/`writel` for Mem
   BARs (idiomatic, fully ordered), reserving `ioread*`/`iowrite*` for the Io path.**
   *(register-access; <https://docs.kernel.org/driver-api/device-io.html>.)*

2. **Managed (`pcim_*`/`devm_*`) vs bare (`pci_*`) resource acquisition.** Managed
   mappers auto-unwind on detach, so the reverse postamble loop emits **nothing**;
   bare mappers require the explicit `remove()` reverse ladder. This changes whether
   the postamble is emitted explicitly or delegated to devres. **Recommend
   `devm_*`/`pcim_*` by default** (simpler teardown, automatic HWM unwind on probe
   failure). *(register-access §A.6, BRM §B.3, device-lifecycle §C.4;
   <https://docs.kernel.org/driver-api/driver-model/devres.html>.)*

3. **Diagnostics device-context strategy — how a lowered body reaches
   `struct device *` (decide before implementation; Codex review issue 5).**
   `dev_*` needs the `struct device *` that the lifecycle/BRM synthesis binds —
   but as-built **no lowered function has it**: bank 5 emits OS-neutral ordinary
   functions (`mod.rs:2276`), lifecycle callbacks run spec-view over
   `&inst.inner` (`body_emit.rs:5472`), and `Log` lowering is a per-call-site
   interception. "Use `dev_*` where the device is reachable" is therefore not yet
   an implementable rule. The three candidate strategies (per-construct §D.3.2):
   (i) **signature threading** — inject a `dev` parameter into every lowered fn
   reachable from a lifecycle callback (a call-graph rule; changes lowered
   signatures); (ii) **container recovery** — `container_of` from the callback's
   `&inst.inner` back to the morph container, then `__rseq_dev` (does not cover
   helpers with non-driver receivers); (iii) **intentional `pr_*` fallback** with
   precise conditions. **Interim rule pinned in the spec: (iii)** — `dev_*` only
   in scaffold-synthesized statements, `pr_*` in all lowered ordinary bodies.
   **Recommend ratifying (iii) for the first implementation** and revisiting
   (i)/(ii) when device-identity in logs is actually demanded.
   *(diagnostics-logging §D.3.2; scaffold §7.)*
   **RATIFIED (Gedeon, 2026-06-09): (iii)** — with "available" pinned to a
   decidable criterion: the emitter uses `dev_*` exactly where a `struct
   device *` is **lexically in scope at the emission site as-built**
   (scaffold-synthesized probe/remove/shutdown bodies and generated
   preamble/postamble statements within them); all lowered author bodies and
   helpers use `pr_*` (with the `pr_fmt` module prefix). No call-graph
   reasoning, no signature threading, no `container_of` recovery. Revisit
   (i)/(ii) only when device-identity in lowered-body logs is demanded.

4. **`&str` interior-NUL faithfulness.** `%.*s` truncates at an interior NUL, so it
   is not byte-faithful like `generic_c`'s `fwrite`. **Recommend idiomatic `%.*s` for
   the common case + document the divergence**; a byte-faithful fallback (per-byte
   emission or `%*ph` hex-dump) is available if faithfulness is later mandated.
   *(diagnostics-logging §D.6; C11 §7.21.6.1.)*

5. **`dev_dbg` dynamic-debug gating vs forced-on parity.** `dev_dbg`/`pr_debug`
   compile out unless `DEBUG`/`CONFIG_DYNAMIC_DEBUG`; `generic_c` always emits the
   debug line to stdout. **Recommend honoring idiomatic dynamic-debug gating** (not
   forcing always-on). *(diagnostics-logging §D.3.5.)*

6. **RegseqError lowering shape — faithful carrier + boundary adapter vs per-site
   `-errno`.** The as-built `generic_c` mechanism lowers the body faithfully and
   converts only at the boundary; the perspective-6 doc sketches per-site `return
   -errno;` (`perspective-6-...md:604-625`) — **not** what `generic_c` implements.
   **Recommend the as-built faithful-carrier shape** (reuses the certified
   discriminant-read machinery; keeps helpers carrier-shaped). *(RegseqError D-1.)*
   **(D-2)** On Linux the `.probe` must **fuse** the `on_init` Err-cleanup branch
   (`emit_init_fail_body :5475-5481`) with the negated-errno return — `generic_c`
   keeps the cleanup and the errno conversion in separate code paths; the Linux
   probe does both at one site. Confirm this fusion is the intended structure.

7. **`.remove()` orderly/surprise — one slot + `present` predicate vs two paths.**
   `generic_c` emits two byte-distinct teardown functions; Linux has one `.remove()`
   slot. **Recommend one `.remove()` branching on a runtime `present` predicate**
   (`pci_device_is_present`/`pci_channel_offline`). *(device-lifecycle §C.4.)*

8. **AMBA `.shutdown` placement.** For AMBA the shutdown hook lives on
   `struct device_driver.shutdown` (`.drv`), not on `amba_driver` directly (unlike
   PCI's `pci_driver.shutdown`). Confirm the per-framework field placement.
   *(device-lifecycle §C.6.)*

9. **`on_init` Result discard on void `.remove`/`.shutdown`.** The pinned
   teardown slots return `void` (PCI/AMBA `.remove`; platform `.remove_new` or
   void `.remove` per the C-3 version gate; `.shutdown`), so a `Result` from
   `on_deinit`/`on_system_shutdown` is
   structurally discarded (CR-4). **Recommend discard-and-optionally-log** (a
   `RegseqError` escaping `deinit` can only reach `dev_err`). See C-3 for the
   version-sensitivity. *(RegseqError D-3, device-lifecycle §C.4/§C.6.)*

10. **The Linux preamble gate — host `is_resolved()` vs a Linux `mappable`
    predicate (decide before implementation; Codex review issue 2, P0).**
    As-built, the whole BRM/map preamble is gated on `is_resolved()` = literal
    L2 `base && size` (`device_lifecycle.rs:236-238`; filter
    `body_emit.rs:5244-5250`), yet on Linux those literal *values* are discarded
    in favor of probe-time enumeration. The composition is consistent only under
    one of two policies: **(a)** keep the as-built gate — L2 literal `base`/`size`
    remain *required as the trigger* on Linux even though their values go unused
    (the spec's pinned interim rule, per-construct §A.5); or **(b)** define a
    Linux-specific `mappable` predicate — e.g. `effective_bar.is_some()` for
    PCI, region-index presence for platform/DT — which is a model/backend gate
    extension (a PCI binding with `effective_bar` but no L2 literals would then
    lower, where today it silently skips the preamble). **Recommend (a) for the
    first implementation** (zero model change, faithful to the as-built
    keystone), **escalating (b) as the durable fix** alongside the Tier-2 real
    enumeration work — (a) means Linux platform files must still carry
    semantically-dead `base`/`size` literals, which is tolerable scaffolding but
    wrong long-term. *(register-access §A.5, BRM §B.5;
    `regseqmodel-input-contract.md` §6.)*
    **RATIFIED (Gedeon, 2026-06-09): (a)**, strengthened with an emit-time
    **fail-loud diagnostic**: a TU containing MMIO ops whose binding is not
    `is_resolved()` is a hard error naming the missing L2 `base`/`size`
    literals — never a silently skipped preamble. (b) is escalated to the
    construct owner as the durable fix (see B-11), to be designed once both
    OS backends exist.

11. **PCI config-space accessor status — ignore, log, reject, or propagate
    (decide before implementation; Codex review issue 6; extended to writes in
    round 2).** The model's `read_config::<T>(off) -> T` and
    `write_config::<T>(off,v)` are infallible, while v6.6
    `pci_read_config_{byte,word,dword}` **and**
    `pci_write_config_{byte,word,dword}` all return a status `int`
    (reads `pci.h:1210-1212`, writes `pci.h:1213-1215`,
    <https://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git/plain/include/linux/pci.h?h=v6.6>).
    The §B.6 direct-propagation rule covers *preamble* generated calls; a config
    intrinsic sits mid-body in an authored function, so propagation has **no
    surface** without a model extension. Options: explicit discard
    (`(void)pci_read_config_dword(...)` / `(void)pci_write_config_dword(...)`);
    discard + `dev_warn` on non-zero;
    reject config intrinsics at emit until a fallible surface exists; or extend
    the model (`read_config -> Result<T, …>`) — a needs-new-surface escalation.
    **Interim rule pinned in the spec: explicit discard, for reads and writes
    both** (the drop is visible
    in the generated C). **Recommend ratifying explicit discard** and filing the
    fallible-surface extension with the construct owner rather than inventing a
    channel in the backend. *(BRM §B.4.)*
    **RATIFIED (Gedeon, 2026-06-09):** explicit `(void)` discard for reads
    **and** writes (visible in the generated C). Idiomatic: real Linux
    drivers overwhelmingly ignore these status returns — the in-band failure
    signal is all-ones read data (surprise-removal detection, out of scope
    here). The fallible-surface model extension is escalated to the BRM
    construct owner (see B-12).

12. **Width-8 MMIO on non-`CONFIG_64BIT` targets — split-order policy (decide
    before implementation; round-2 review).** v6.6 `asm-generic/io.h` defines
    `readq`/`writeq` (and `ioread64`/`iowrite64`) only under `CONFIG_64BIT`
    (<https://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git/plain/include/asm-generic/io.h?h=v6.6>);
    on architectures without native 64-bit MMIO they exist only via
    `<linux/io-64-nonatomic-lo-hi.h>` / `<linux/io-64-nonatomic-hi-lo.h>`
    (<https://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git/plain/include/linux/io-64-nonatomic-lo-hi.h?h=v6.6>,
    <https://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git/plain/include/linux/io-64-nonatomic-hi-lo.h?h=v6.6>),
    which split the access into two 32-bit ops whose order (low-then-high vs
    high-then-low) is device-register semantics the model has no surface to
    express. **Interim rule pinned in the spec (per-construct §A.2): width-8
    lowers to `readq`/`writeq` for `CONFIG_64BIT` targets only, and a TU
    containing a width-8 MMIO op carries a fail-loud `#ifndef CONFIG_64BIT` /
    `#error` guard** — never a silent non-atomic split. Options to ratify: keep
    the 64-bit-only restriction; pick one `io-64-nonatomic-*` header globally
    (loses per-device correctness); or extend the model with a per-block
    split-order attribute (needs-new-surface). **Recommend keeping the
    64-bit-only restriction** until a driver actually demands 64-bit registers
    on a 32-bit arch. *(register-access §A.2.)*
    **RATIFIED (Gedeon, 2026-06-09):** keep the 64-bit-only restriction with
    the fail-loud `#error` guard. Nothing in the corpus demands width-8 on a
    32-bit arch (E1000's 64-bit quantities are architecturally two 32-bit
    registers, TDBAL/TDBAH; PL011 is narrow). The per-block split-order
    attribute remains conditional needs-new-surface — justified only by the
    first real driver that requires it.

---

## B. `needs-new-surface` gaps (blocking — escalate against the model, do not invent)

1. **`poll_until` → `readl_poll_timeout`.** `recognized_not_lowered`
   (`domain_attr.rs:95`); `generic_c` emits nothing. Adopting `readl_poll_timeout`
   (`<linux/iopoll.h>`) requires resolving (a) how the side-effect-free read
   predicate translates into the macro's single-`readl`-into-`val` `cond`, and (b)
   what units the DSL `DELAY`/`TIMEOUT` carry (the macro is microseconds). Its
   timeout→`RegseqError::Timeout`→`ETIMEDOUT` edge is also undesigned. **Do not lower
   before `generic_c` establishes a reference.** *(register-access §A.4.)*

2. **`BarType::Io` port-I/O.** `recognized_not_lowered`
   (`device_lifecycle.rs:243-250`); `generic_c` has no Io emit path. Linux port-I/O
   (`inb/outb` or `ioread*`/`iowrite*` over a `pci_iomap` Io cookie) is a distinct
   surface — the based-MMIO accessors are `readl`-shaped and undefined on an Io
   cookie. e1000's `need_ioport`/`io_base` path (reference-drivers/e1000/linux/e1000_main.c:974-983) shows real
   drivers use it. **Escalate as a blocking model gap.** *(register-access §A; BRM §B.5.)*

3. **Config-space width-8.** `config_access_width` admits a `u64` `T`, but PCI
   config accessors top out at 32-bit dword — no 8-byte primitive. Reject or split
   into two dwords. **Undesigned.** *(BRM §B.4.)*

4. **Non-PCI config access (BRM-001).** A config intrinsic on a non-PCI driver has
   no config space and should be rejected, but BRM-001 is **deferred** (needs
   call-site→driver linkage); today such a call slips past recognition. The Linux
   spec depends on this as-yet-unbuilt diagnostic. *(BRM §B.4.)*

5. **More than one resolved region per driver.** Rejected today (`mod.rs:248-254` /
   `max_resolved_bindings_per_driver`), so `map_region`'s single global mapped base
   is unambiguous. A real multi-BAR Linux driver (e1000 maps BAR_0 + flash + io)
   needs the model extended to per-region `ioremap`. **needs-new-surface.**
   *(device-lifecycle §C.7.)*

6. **`MatchEntry` multi-`compatible` / multi-row tables.** `render_match_trace`
   renders only `.first()` (`body_emit.rs:5499-5508`); Linux `pci_device_id[]` / OF
   `of_device_id[]` / `amba_id[]` tables are inherently multi-row + zero-sentinel.
   The table emission is straightforward, but the backend must read the **full**
   `match_set`, not the host trace's first-only view. *(device-lifecycle §C.2.)*

7. **`UserDataField` / `UserDataKind`.** `recognized_not_lowered`
   (`device_lifecycle.rs:263-279`); satisfied today only incidentally by the
   container zero-init `{0}`. No designed lowering — a new backend inherits none and
   must rely on zero-alloc or design one (do not invent). *(device-lifecycle §C.7.)*

8. **`ClassTag` / `device_class`.** `recognized_not_lowered`
   (`device_lifecycle.rs:135-144`); deferred to the Device Class construct. No
   `class_create`/`class_register` synthesized — that would be invention against the
   citation rule. *(device-lifecycle §C.8.)*

9. **`set_bus_master` (dormant).** `pci_set_master(pdev)` (reference-drivers/e1000/linux/e1000_main.c:946) is the
   identified target, but the gate `is_pci && dma_present` is never satisfied —
   `dma_present` defaults false and no fixture sets it (DMA out of scope, owned by
   the unbuilt DMA construct). Lowered-but-gated, never exercised. *(BRM §B.3.3.)*

10. **`Other(sid)` and `I2c`/`Spi`/`Usb` buses.** `Other(sid)` is rejected at
    surface validation (needs-new-surface); `I2c`/`Spi`/`Usb` carry a model variant
    but are bridge-rejected at parse, so their `i2c_add_driver`/`spi_register_driver`/
    `usb_register` registration + id-table lowering is **deferred**. *(BRM §B.2,
    device-lifecycle §C.1.)*

---

11. **Preamble-gate generalization (`mappable` predicate) — escalated from the
    A-10 ratification (2026-06-09).** Replace the literal-L2 `is_resolved()`
    trigger with a per-OS resolvability predicate (PCI: `effective_bar.is_some()`;
    platform/DT: region-index presence; Zephyr: DT `reg` presence — pairs with
    zephyr A-1). Design AFTER both OS backends exist, against their real needs;
    until then L2 literals remain the trigger (semantically dead on Linux).

12. **Fallible config-access surface — escalated from the A-11 ratification
    (2026-06-09).** `read_config`/`write_config` are infallible while PCI
    accessors return status; mid-body intrinsics have no propagation channel.
    Candidate: `read_config -> Result<T, …>` (BRM construct-owner decision —
    cross-construct error-channel implications). Interim everywhere: explicit
    `(void)` discard.

## C. Version-sensitive pins to confirm against the target kernel

1. **`claim∘map` fusion realization on the devm path.** The model emits a separate
   `claim_regions` (every-bus, `body_emit.rs:5263-5265`) + `map_region`, but
   `devm_ioremap_resource` fuses them. Is `claim_regions` a genuine no-op on the
   devm path, or does it map to a distinct `devm_request_mem_region` step? The
   realization is backend-[Open] (`prelude.rs:202-208`). Confirm against the pinned
   kernel. *(BRM §B.3.2.)*

2. **`-EPROBE_DEFER` surfacing.** A native preamble deferral must propagate through
   `.probe`'s return without ever touching the authored RegseqError channel. This
   needs an explicit native-status return path in the synthesized preamble, which
   `generic_c` (no failing native calls) does not exercise. Confirm the preamble's
   native-error plumbing. *(RegseqError §E.6, composition note 3.)*

3. **Platform teardown slot — RESOLVED: version-gated member selection
   (round-2 review; supersedes the bare `.remove_new` pin from Codex issue 4).**
   The verified facts: PCI `.remove` is `void`
   (vendored witness reference-drivers/e1000/linux/e1000_main.c:1252), AMBA `.remove` is `void`
   (reference-drivers/pl011/linux/amba-pl011.c:3029), but `struct platform_driver` at v6.6 **still has
   `int (*remove)(struct platform_device *)` plus
   `void (*remove_new)(struct platform_device *)`**, with the header comment
   telling new drivers to implement `.remove_new()` until the conversion
   completes (v6.6 `platform_device.h:236-250`,
   <https://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git/plain/include/linux/platform_device.h?h=v6.6>);
   `.remove` itself became `void` in v6.11 and `.remove_new` was later
   retired — at v6.13 the struct carries **only**
   `void (*remove)(struct platform_device *)` (v6.13 `platform_device.h:236-239`,
   <https://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git/plain/include/linux/platform_device.h?h=v6.13>).
   A bare `.remove_new` pin therefore does not compile across the stated
   `6.6+` range. **The pinned policy (scaffold §4, per-construct §E.4): one
   `void`-signature teardown function; the driver-struct initializer member is
   version-gated — `.remove_new` when
   `LINUX_VERSION_CODE < KERNEL_VERSION(6, 11, 0)`, `.remove` from v6.11 on.**
   Both arms share the `void (*)(struct platform_device *)` signature, so the
   emitted function body is identical and
   `on_deinit`'s `Result` has no return channel on any teardown
   slot on either arm (D-3, unchanged). Separately, confirm the four synthesized scenarios
   (Orderly/Surprise/InitFail/Shutdown, `backend_options.rs:26-32`) are a faithful
   partition of the Linux entry points, or whether they miss states (`.suspend`/
   `.resume`, runtime PM — out of current scope but unconfirmed).
   *(device-lifecycle, RegseqError D-3.)*

4. **Morph `__rseq_dev` recovery.** Which concrete Linux device struct (`pci_dev`
   vs `platform_device` vs `amba_device`) backs the `void *`, and how
   `container_of`/`*_get_drvdata` recovers the embedded `inner`. The morph is
   non-behavioral in the host oracle (NULL/unobservable, `body_emit.rs:5176-5182`),
   so this is unexercised by `generic_c`. *(device-lifecycle §C.7.)*

5. **Big-endian accessor selection.** `endian="little"` → `readl` (LE) is pinned,
   but a big-endian block would need `ioread32be`/`iowrite32be`. BE is
   recognized-not-lowered in `generic_c` (LE-only host), so BE accessor selection is
   unspecified. *(register-access §A.2.)*

6. **Non-converged logging design doc.** `logging-and-printing` is the one construct
   with **no finalized surface-convergence doc**; its discussion is marked
   superseded (`logging-and-printing-discussion.md:469-472`). Confirm the as-built
   four-level / nine-modifier surface is the frozen contract before freezing this
   Linux mapping. *(diagnostics-logging §D.0/§D.6.)*

7. **PCI managed acquisition cascade + map call — RESOLVED: PINNED against the
   campaign tree (backend-linux-c-e1000 T03, the GRILL O3 verification;
   2026-06-10).** All citations are the campaign kernel tree
   `~/regseq-eval/e1000/linux/kernel-src/e1000-x86_64-linux`, **kernel 6.18.21**
   (`include/config/kernel.release`).

   **Verified facts.**
   - `pcim_enable_device` (drivers/pci/devres.c:370-393) registers exactly ONE
     devres action — `pcim_disable_device` (devres.c:351-359:
     `pci_disable_device` unless pinned) — and sets `pdev->is_managed = true`.
     Its cascade covers the *enable only*.
   - The v6.6-era auto-management of subsequent plain requests **does not exist
     in this tree**: `pci_request_selected_regions` (drivers/pci/pci.c:4031-4036)
     → `__pci_request_selected_regions` (pci.c:4003-4020) →
     `__pci_request_region` (pci.c:3936-3961) is plain
     `request_region`/`__request_mem_region` with **no devres path and no
     `is_managed` check**; the only behavioral reader of `is_managed` left is
     MSI auto-teardown (drivers/pci/msi/msi.c:85) — irrelevant here (no IRQ
     constructs). The per-construct spec's §B.3 combination
     (`pcim_enable_device` → plain `pci_request_selected_regions` →
     `pci_iomap`, linux-lowering-per-construct.md:314-321,366-367,395-396) was
     fact-pinned at v6.6, where `__pci_request_region` recorded managed regions
     via `find_pci_dr` [hypothesis for the v6.6 mechanism name — verified only
     insofar as the v6.6-pinned spec asserts the combination was managed; the
     rework that removed it landed around v6.11]. On 6.18.21 that combination
     LEAKS the region and the mapping on detach — it would silently violate
     ratified A-2's no-explicit-release `.remove`. **This supersedes §B.3's
     map/claim step realization for the PCI arm.**
   - Managed realizations available in-tree: `pcim_request_region`
     (devres.c:724-747; region only), `pcim_iomap` (devres.c:538-568; map only,
     legacy iomap table, "This SHOULD only be used once per BAR",
     NULL-returning for backwards compat), `pcim_iomap_regions` (devres.c:679,
     "This function is DEPRECATED. Do not use it in new code. Use
     pcim_iomap_region() instead"), and **`pcim_iomap_region(pdev, bar, name)`**
     (devres.c:607-643, `EXPORT_SYMBOL`): `pci_request_region` + `pci_iomap`
     fused behind ONE devres entry whose release unmaps AND releases the
     region; returns `IOMEM_ERR_PTR` on failure.
   - In-tree managed-NIC witness (campaign tree): the r8169 probe runs
     `pcim_enable_device(pdev)`
     (drivers/net/ethernet/realtek/r8169_main.c:5416) →
     `pcim_iomap_region(pdev, region, KBUILD_MODNAME)` (r8169_main.c:5428).
     This fills the GRILL D06(2) gap ("no in-corpus managed-PCI witness") with
     a campaign-tree witness.

   **THE PIN (consumed verbatim by T04; gates every linux PCI snapshot bless):**
   ```c
   ret = pcim_enable_device(pdev);      /* native status propagates unwrapped */
   if (ret)
           return ret;
   <base> = pcim_iomap_region(pdev, <effective_bar>, "<drv name>");
   if (IS_ERR(<base>))
           return PTR_ERR(<base>);
   ```
   - **Map call = `pcim_iomap_region`** — a deliberate fourth option over the
     GRILL's three candidates: `pci_iomap` and the witnessed `pci_ioremap_bar`
     are unmanaged in this tree (leak under A-2); `pcim_iomap` does not request
     the region (would need a separate `pcim_request_region` = two devres
     entries) and is the legacy-table API; the kernel's own docs deprecate the
     multi-BAR wrapper in favor of `pcim_iomap_region`.
   - **Devres cascade on detach (LIFO):** unmap + release region (the
     `pcim_iomap_region` entry) → `pci_disable_device` (the
     `pcim_enable_device` action). `.remove` emits NO release ladder (A-2
     holds). Probe-failure unwind rides the same cascade.
   - **Claim∘map fusion recorded:** the ratified D06(2) *shape* named
     `pci_request_selected_regions(pdev, 1<<effective_bar, name)` as a separate
     step; on 6.18.21 the managed request and map FUSE into the single
     `pcim_iomap_region` call — the exact PCI analogue of the AMBA
     `devm_ioremap_resource` claim∘map fusion already whitelisted at L3
     (`devres-claim-fusion` row; C-1 above). The oracle's separate
     `BUS claim_regions` / `BUS map` events stay whitelist-licensed under the
     same A-2 citation plus this pin.
   - **Deviation from the vendored witness recorded:** the in-repo e1000 probe
     is deliberately UNMANAGED (`pci_enable_device`/`pci_enable_device_mem` +
     manual `goto` unwind + `pci_ioremap_bar`,
     reference-drivers/e1000/linux/e1000_main.c:931-972); it witnesses the
     acquisition *shape* only. The managed variant rests on ratified A-2 + this
     campaign-tree verification + the r8169 witness.
   - `pci_set_master` is NOT emitted: no DMA constructs exist (unit non-goals);
     the witness's `set_master` serves its DMA engine.
   - **Version note:** `pcim_iomap_region` postdates the v6.6 fact-pin (it
     arrived with the devres rework, ~v6.11 era) [hypothesis as to the exact
     version; verified present + exported in 6.18.21]. The unit's compile gate
     is the campaign tree only. If a pre-rework target ever materializes, the
     recorded fallback is the era's `pcim_iomap_regions(pdev, 1<<bar, name)` +
     `pcim_iomap_table(pdev)[bar]` idiom — recorded here, NOT emitted.
   *(bus-resource-management §B.3; device-lifecycle §C.4; GRILL
   backend-linux-c-e1000 D06(2)/O3.)*

7. **RMW atomicity / ordering.** `mmio_modify` lowers to a non-atomic
   read-modify-write (matching `generic_c`); the construct excludes ordering/locking
   (`convergence §1`, OUT). No `ordered!`/fence is modeled. Confirm this is
   acceptable for the target devices, or escalate when the ordering construct lands.
   *(register-access §A.3.)*

---

## D. Completeness ledger

### D.1 Per-construct lowering status

The status column deliberately distinguishes **"core subset lowerable"** (the
subset `generic_c` actually lowers has a confident, cited Linux lowering) from
"complete construct lowerable" — **no construct earns the latter**; each carries
the enumerated residual gaps. (Downgraded per the Codex review, issue 8: earlier
drafts said "fully lowerable", which overstated against the section-B list.)

| construct | status | residual gaps |
|---|---|---|
| **register-access** | **core subset lowerable** — the five MMIO ops + `map_region` are native-materialization with confident citations; the construct as a whole is **not complete** | `poll_until` (B-1), `BarType::Io` accessor (B-2), BE accessor (C-5) — all pre-existing `recognized_not_lowered`; preamble gate policy (A-10); width-8 `CONFIG_64BIT` gate (A-12) |
| **bus-resource-management** | **core subset lowerable** (PCI + platform/DT MMIO path) — `register_driver`, the managed preamble ops, config read/write widths 1/2/4; **not complete** | width-8 config (B-3), non-PCI config (B-4), `set_bus_master` dormant (B-9), i2c/spi/usb (B-10), claim∘map fusion realization (C-1), config status disposition (A-11), gate (A-10) |
| **device-lifecycle** | **core scaffold lowerable** — module/probe/remove/shutdown scaffold, scenarios, match tables, container morph; **not complete** | multi-region (B-5), multi-`compatible` read (B-6), `UserDataField` (B-7), `ClassTag` (B-8) — all pre-existing `recognized_not_lowered`; platform teardown version gate (C-3, resolved) |
| **diagnostics-logging** | **core subset lowerable** — all four levels + template reassembly + `&str` (with documented interior-NUL divergence); **not complete** | device-context strategy undecided (A-3); non-converged design doc to confirm (C-6); interior-NUL faithfulness is a policy choice (A-4) |
| **RegseqError** | **core mechanism lowerable** — marker + carrier + adapter reused; only polarity/placement/include change; nine errno rows symbol-identical | `-EPROBE_DEFER` plumbing (C-2), void-teardown channel (C-3/D-3) |

### D.2 Unresolved / failed constructs

**`[]` — none failed as a whole** (the precise claim; nothing stronger).

No construct failed to lower or was left unresolved *as a whole* — and no
construct is *completely* lowered either. The precise statement is: the **core
in-scope subset** of every construct (the subset `generic_c` actually lowers)
has a confident, cited Linux lowering; everything beyond that subset is
explicitly dispositioned in sections A–C, none silently. Every residual item is
either a **policy decision** (section A — four of which, A-3/A-10/A-11/A-12, must
be ratified before implementation) or a **pre-existing `recognized_not_lowered`
model fact** (section B) that the Linux target *surfaces* but did not
*introduce* — `poll_until`, `BarType::Io`, `UserDataField`, `ClassTag`, config
width-8, multi-region, and multi-`compatible` are all gaps inherited from the
model/reference, filed here as blocking escalations rather than worked around in
the backend.

The one structural divergence the Linux target *adds* — splitting `generic_c`'s
single synthesized scenario function into module-init registration + the
kernel-driven `.probe`/`.remove`/`.shutdown` callbacks — is specified in
[`linux-scaffold-and-dispatch.md`](./linux-scaffold-and-dispatch.md) and
[`linux-integration-probe-path.md`](./linux-integration-probe-path.md). Note the
scope boundary, though (README; F-1): this directory specifies the Linux
*lowering deltas*, not the whole-model fold a backend implementation must also
perform — for that fold, `generic_c` is the authoritative reference lowering;
**F-1 (resolved by owner ratification, §F.1)** sets the contract: the Linux
backend lowers RegseqModel directly, consulting `generic_c` as reference, with
no prerequisite seam refactor.

---

## E. Process / review status (not a lowering decision)

This section is **not** part of the technical ledger above (A/B/C/D). It records
the external-review status, kept separate so it never mixes with the
construct-lowering open questions.

1. **The external Codex review EXECUTED on 2026-06-09** (after an earlier
   sandbox-aborted attempt that produced zero findings). Verdict: `ok=False`
   with 9 cited issues (3×P0, 3×P1, 2×P2, 1×P3) and
   `unresolved_or_failed_constructs=[]`. Full verbatim output and the per-issue
   disposition table are in [`codex-review.md`](./codex-review.md). Every issue
   was either **fixed in the spec** (the native/authored ladder split, the
   platform `.remove_new` facts, the `dev_*` void-return wording, the README
   scope and citation-discipline corrections, this ledger's downgrade) or
   **routed to a decision in this file**: **A-3** (device-context strategy),
   **A-10** (preamble gate), **A-11** (config status), **C-3** (platform
   teardown pin), **F-1** (agnostic-core implementation contract). Of those
   five, **C-3 was resolved in the round-2 review application** (the
   version-gated platform teardown policy) and **F-1 by owner ratification**
   (the independent-lowering contract, §F.1 — superseding the round-2 interim
   seam-refactor pin);
   **A-3, A-10 and A-11 remain the outstanding ratifications**, joined by
   **A-12** (width-8 MMIO arch gate, added in round 2). The spec carries
   external review findings but **not** an external pass verdict until those
   are ratified and re-reviewed.

---

## F. Implementation-contract decisions (escalated from the Codex review)

1. **How the Linux backend obtains the agnostic core — RATIFIED BY OWNER
   (Gedeon, 2026-06-09; supersedes the round-2 loop-applied pin of option (a)).**
   The thesis contract is: the driver spec and its RegseqModel are provided
   once; a backend's sole obligation is to lower RegseqModel into an idiomatic
   OS-specific driver. **No shared-core contract exists between backends.**
   Ratified implementation path:
   - Each backend lowers RegseqModel directly, using **generic_c as the
     authoritative reference lowering** — for semantics, emission ordering,
     construct coverage, and the whole-model fold structure (per this
     directory's traceability matrices and `regseqmodel-input-contract.md`
     §1–§5) — but **owning its own emission shape**: OS include sets, natural
     type/declaration ordering, and scaffolding conventions. Backends must
     read as native drivers for their OS, not as restructured generic_c
     output; the OS's conventions win every conflict. Reference-guided
     implementation, not copy-then-edit.
   - `generic_c` remains the **frozen behavioral validation oracle**; backend
     work never mutates it.
   - **No seam refactor is prerequisite.** Extraction of shared machinery is
     an optional later optimization, to be evaluated only after at least two
     OS backends exist (rule of three; the §8 site table remains the candidate
     parameterization list if that day comes).
   With this ratified, the document set is implementable as specified:
   implementation = lower RegseqModel per this directory, consulting generic_c
   as reference. (History: the review loop's round-2 fix had interim-pinned
   option (a) — seam-refactor-first — to make the deferral decidable; the
   owner ratification above replaces that pin and is the normative contract.)
