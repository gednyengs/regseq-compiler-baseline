# Zephyr backend — open questions & completeness ledger

Every open decision gathered from the five constructs and the composition,
plus the honest completeness ledger. The list is partitioned into:
**(A) policy decisions to ratify** (a clear recommendation exists; none blocks
specification); **(B) `needs-new-surface` gaps** (blocking model/reference
extensions — escalate, do not paper over); **(C) version-sensitive pins to
confirm against the target Zephyr release**. Section **(D)** is the
completeness ledger, including the explicit unresolved/failed-construct list.

Each item cites the construct it came from (with its per-construct local
label: register-access `OQ-*`, BRM `O*`, device-lifecycle `Q*`, diag-log
`LOG-O*`, RegseqError `ZE-OQ*`) and a repo `path:line` or pinned doc.

One decision was **resolved during authoring** rather than left open: the
Zephyr `enable_device`+`set_bus_master` fusion (backend-[Open] at
bus-resource-management-surface-convergence.md:135) is ratified as **separable
`pcie_set_cmd` calls** — one per gated op, single-call fusion permitted as a
peephole when both gates fire — so the gating matrix stays op-per-op
([per-construct §B.3.1](./zephyr-lowering-per-construct.md); witness
eth_e1000.c:220-221).

---

## A. Policy decisions to ratify (recommendation in **bold**; none blocks the spec)

1. **`(base,size)` provenance — the L2 join on Zephyr.** *(register-access
   OQ-1 ≡ BRM O3.)* generic_c feeds `map_region` the platform.toml-resolved
   `MmioBinding.base/.size` literals (`device_lifecycle.rs:209-229`;
   `body_emit.rs:5277-5284`); the idiomatic Zephyr source is the DT `reg`
   property via `DEVICE_MMIO_ROM_INIT(DT_DRV_INST(n))` (uart_pl011.c:758-759),
   keyed by the `MatchEntry::Devicetree` compatible. Options: DT-only,
   literal-only via `device_map`, or DT-with-literal-cross-check. **Recommend
   DT `reg` as the normative source on Zephyr, with the platform.toml
   `[mmio.<id>]` literals retained as an advisory cross-check (diagnostic on
   mismatch), not as the map input.** Note `is_resolved()`
   (`device_lifecycle.rs:236-239`) gates the whole preamble — the ratified
   answer determines whether a Zephyr TU needs a platform.toml at all. I04
   guarantees the choice changes no register-access op (`prelude.rs:158-162`).

2. **Base threading into op sites — TU-static vs `DEVICE_MMIO_GET(dev)`.**
   *(register-access OQ-3; Codex review issue 2.)* generic_c reads a TU-global
   base (`prelude.rs:182`), legal under the single-region + single-instance
   rules — but `DT_INST_FOREACH_STATUS_OKAY` stamps one device per status-okay
   DT node, so a TU-static base **silently aliases one base across instances**
   on any multi-instance devicetree. **Recommend the faithful first
   realization: a TU-static `mm_reg_t` installed immediately after
   `DEVICE_MMIO_MAP`, PAIRED with the mandatory emitted guard
   `BUILD_ASSERT(DT_NUM_INST_STATUS_OKAY(DT_DRV_COMPAT) == 1, …)`**
   (per-construct §A.2.2; scaffold §1 item 9) so multi-instance models are
   rejected at C compile time — graduating to the idiomatic per-device
   `DEVICE_MMIO_GET(dev)` (uart_pl011_registers.h:40-43) once the device
   pointer threads into lowered author bodies; that threading is the
   device-lifecycle container-morph seam (`body_emit.rs:5301-5306`), not
   register-access's to decide. The alternative ratification is to skip the
   faithful step and require the per-device threading up front.

3. **Zephyr-PCIe preamble order — probe→enable→map vs canonical
   enable→handle.** *(BRM O2.)* The model's canonical order puts
   `enable_device` before `get_resource_handle` (`body_emit.rs:5219-5243`);
   the vendored PCIe reference inverts it — `pcie_probe_mbar` (eth_e1000.c:219)
   *then* `pcie_set_cmd` (:220-221) *then* `device_map` (:223-224), i.e. BAR
   sizing with memory decode still off. **Recommend adopting the reference
   order (probe → enable → map) on Zephyr-PCI and recording the permutation
   as a ratified per-OS deviation from the canonical preamble** — never
   silently emitting either order. Post-review (Codex issue 3) the sibling
   docs uniformly present this as **the spec'd PCIe order pending this
   ratification, and the single recorded deviation**; the platform path stays
   strictly canonical (preambles → map — per-construct §A.2.2 notes pl011's
   literal map-first layout as witness-only, not adopted).
   **RATIFIED (2026-06-11, backend-zephyr-c-e1000 D02/C01):** the reference
   order (probe → enable → map) is adopted as THE single recorded per-OS
   preamble deviation, rendered explicitly — an `RSEQ-DEVIATION` audit
   comment at the hoisted-probe emit site carrying the
   `backend-zephyr-c-e1000 GRILL D02` license (witness eth_e1000.c:219-224:
   BAR sizing with memory decode off) — never silent. `enable_device` is
   op-per-op gated: this fixture authors no `set_bus_master`, so only the
   MEM gate fires (`pcie_set_cmd(bdf, PCIE_CONF_CMDSTAT_MEM, true)`); the
   separable-calls + single-call-fusion-peephole policy stands ratified but
   unexercised. The platform/Amba path stays strictly canonical
   (byte-frozen, proven by the pl011 zephyr snapshots needing zero
   re-bless).

4. **Offset-0x00 `read_config` — framework cache vs always-read.** *(BRM
   O4.)* The host PCI-ID-caching rule (`prelude.rs:260-288`) is structurally
   satisfied by `struct pcie_dev` carrying DID/VID (eth_e1000_priv.h:92-93).
   **Recommend serving offset-0x00 reads from `dev->pcie->id` (zero bus
   traffic — matches the as-built host caching semantics); all other offsets
   issue a real `pcie_conf_read`.**
   **RATIFIED with containing-word scope (2026-06-11, backend-zephyr-c-e1000
   D06/C03):** any config READ whose containing word is word 0 — byte
   offsets 0x00–0x03, covering both `vendor@0x00` AND `device@0x02` — is
   served from `__rseq_pcie->id` with §B.4 shift/mask extraction and zero
   bus traffic (a literal offset-0x00-only cache would diverge from the
   host rule for the device-id half); all other offsets issue one live
   `pcie_conf_read`. The D05 RMW's internal read is always live, never the
   cache. The L3 containment expectation carries the consequence: NO config
   bus traffic for word-0 reads.

5. **Non-static `&str` log arguments under deferred mode.** *(diag-log
   LOG-O1; composition open list.)* A Rust fat pointer is not NUL-terminated;
   the deferred-mode packager copies transient strings by NUL-terminated
   length, so raw `%.*s` risks over-read. **Recommend the call-site bounce
   buffer: copy `min(len, CAP-1)` bytes to a fixed stack buffer, NUL-
   terminate, pass via `%s`** (the in-repo snprintk idiom, eth_e1000.c:29-37),
   with both truncation divergences (cap, interior NUL) documented.
   Alternatives kept on record: `CONFIG_LOG_MODE_IMMEDIATE` (removes the
   packaging copy, keeps the NUL gap) or `LOG_HEXDUMP_*` (byte-faithful, not
   a formatted line).

6. **Static `&str` with interior NUL.** *(diag-log LOG-O2.)* `%s` truncates;
   byte-faithfulness is lost on any `%s` path. **Recommend fail-loud at emit
   time** (matches the construct's fail-loud discipline; the literal bytes are
   known at emission), with `LOG_HEXDUMP_DBG` as the documented opt-in
   fallback (witness eth_e1000.c:29-37).

7. **Compile-time log level — per-driver Kconfig vs forced `LOG_LEVEL_DBG`.**
   *(diag-log LOG-O3.)* Zephyr's compile-time filtering can compile a
   `LOG_DBG` out entirely; generic_c emits every level unconditionally.
   **Recommend honoring the idiomatic Kconfig-driven module level**
   (the vendored pattern, `#define LOG_LEVEL CONFIG_…_LOG_LEVEL`,
   eth_e1000.c:10) and documenting the divergence from generic_c's always-on
   emission — mirroring the Linux `dev_dbg` dynamic-debug decision
   (linux-open-questions.md A-5).

8. **Module-name fallback for a TU with no `DriverDescriptor`.** *(diag-log
   LOG-O4.)* **Recommend the TU's emission name** (sanitized), keeping the
   `DriverDescriptor.name` seam (Q6) for lifecycle TUs.

9. **Kconfig pins — assert vs document.** *(diag-log LOG-O5.)* The logging
   baseline (`CONFIG_LOG=y`, deferred mode, `CONFIG_CBPRINTF_COMPLETE`,
   `CONFIG_CBPRINTF_FULL_INTEGRAL` for `%lld/%llu`). **Recommend documenting
   the pins in generated collateral now; asserting them belongs to the future
   build-harness stage** (no Zephyr build harness exists this stage).

10. **`Unsupported` → `-EOPNOTSUPP` vs Zephyr's `-ENOTSUP` idiom.**
    *(RegseqError ZE-OQ1; composition open list.)* Both symbols are public in
    Zephyr libc (`uart_pl011.c:264` uses `-ENOTSUP`). **Recommend staying
    symbol-identical to the C02 table (`-EOPNOTSUPP`, `errno.rs:41`)** — keeps
    the table per-target-invariant; `-EOPNOTSUPP` is valid and used in Zephyr.

11. **Standalone-entry rc observability.** *(RegseqError ZE-OQ3.)* Zephyr's
    kernel ignores `main()`'s return value, so the B3 adapter composition
    executes but the final rc is unobservable. **Recommend composing with
    diagnostics-logging — `LOG_ERR` the nonzero rc before returning** — until
    a harness-specific channel exists.

12. **`on_surprise_remove` authored under `TargetOs::Zephyr` — diagnose vs
    silent omit.** *(device-lifecycle Q2's policy half.)* **Recommend a
    compile-time diagnostic** (the author wrote a callback that cannot run on
    this target — silence would violate the no-silent-truncation rule); the
    representability question itself is B-7.
    **SUPERSEDED by the os-capability-disposition rule (2026-06-10):** the
    diagnose-vs-omit dichotomy was resolved by a third disposition —
    *auditable non-emission* under the ratified capability row
    CAP-ZEPHYR-DT-SURPRISE-1 (`AbsentDispatch`): named `RSEQ-ABSENT-SLOT`
    audit block in the C, conformance-report row, test-log line, body still
    fully checked. See
    `docs/discussions/os-capability-disposition/os-capability-disposition-rule.md`.

13. **`MatchEntry::Amba`-only match set — diagnose vs require a companion
    `Devicetree` entry.** *(device-lifecycle Q4's policy half.)* **Recommend
    rejecting with a diagnostic that names the fix** (add a
    `Devicetree { compatible }` entry — the PL011 precedent: AMBA part,
    compatible-string match, uart_pl011.c:9 + arm_pl011.yaml:3); the model
    surface question is B-5.

14. **`descriptor.name` carrier.** *(device-lifecycle Q6.)* No single Zephyr
    slot exists (the runtime device name comes from the DT node).
    **Recommend spending the logical name on the log module name
    (= the diag-log seam, eth_e1000.c:9-12) and on emitted symbol prefixes** —
    the vendored idiom.

15. **Init level/priority knob.** *(device-lifecycle Q7; composition open
    list.)* No model surface; the vendored values differ and the level is
    class-coupled (uart_pl011.c:790 vs eth_e1000.c:342); with no
    `-EPROBE_DEFER` analogue this knob carries the cross-device-ordering
    burden. **Recommend a `BackendOptions` knob with a documented default
    (`POST_KERNEL` + the kernel default device priority), revisited when the
    Device Class construct lands** (the class is what couples the level).
    **Evidence note (2026-06-11, backend-zephyr-c-e1000 D07): the hardcode
    holds unchanged on the PCIe arm.** The PCIe bus scan is
    `SYS_INIT(pcie_init, PRE_KERNEL_1|_2, …)` (v4.4.0
    `drivers/pcie/host/pcie.c:547-552`), so BDF enumeration strictly
    precedes any POST_KERNEL device init — no ordering pressure exists. The
    vendored e1000 itself is POST_KERNEL (hardcoded in
    `ETH_NET_DEVICE_DT_INST_DEFINE`, ethernet.h:1131-1148); a priority-60
    mirror was rejected as class coupling (the deferred Device Class
    construct's territory).

16. **Class-neutral scaffold build-now.** *(device-lifecycle Q8a; composition
    open list — framework-class wrappers; Codex review issue 6.)* **Recommend
    yes: class-neutral `DEVICE_DT_INST_DEFINE` + `NULL` api for
    `ClassTag::Generic` ONLY, with any non-Generic tag an emit-time
    diagnostic** (per-construct §C.9 — silently passing `NULL` api for a
    declared class would yield a device invisible to its class-API consumers,
    i.e. silent truncation), leaving class wrappers
    (`ETH_NET_DEVICE_DT_INST_DEFINE` + its second-phase iface init,
    eth_e1000.c:267-285, :337-344) to the Device Class construct — with the
    scope pin stated explicitly: **`on_init`'s contract spans only the
    device-init phase**.
    **DISPOSITION EVOLVED (2026-06-11, backend-zephyr-c-e1000 D14/C06): the
    non-Generic-tag REJECT is replaced by audited class-neutral emission.**
    A declared non-Generic class now emits the SAME NULL-api scaffold as
    `ClassTag::Generic` PLUS an `RSEQ-DEFERRED-CLASS` audit comment
    immediately above the instantiation define (naming the declared class,
    B-9, and the `backend-zephyr-c-e1000 GRILL D14` license) and an
    OPTIONAL skip-serialized conformance-report note — answering this
    item's anti-silent-truncation rationale the same way D03(a) lifted the
    all-foreign match rejection: the declared class is AUDITED, never
    silent, never a wall against the born-converged fixture. Class API
    delivery itself (macro family + vtable + second-phase init) stays the
    Device Class construct's (B-9). The `ClassTag::Generic` emission path
    is byte-frozen (C06 item 3).

17. **Config(ROM)/data(RAM) field split refinement.** *(device-lifecycle
    Q8b.)* The model's `DriverHandle` doesn't distinguish immutable-per-
    instance facts from mutable state. **Recommend build-now: whole spec
    driver into `data`** (with `DEVICE_MMIO_ROM/RAM` carrying the binding per
    convention); the config-split is a later quality refinement.

18. **`on_system_shutdown` interim emission policy.** *(device-lifecycle Q3's
    policy half; Codex review issue 4.)* The PM slot mapping is witnessed
    (uart_pl011.c:106-117), but Zephyr PM is an action-callback contract over
    `enum pm_device_action` — not a dedicated shutdown hook — and which
    action (if any) fires at system power-off is Kconfig/version-sensitive
    (C-4). The spec now emits a generated `pm_device_action` dispatch wrapper
    (author body on the chosen action(s), `-ENOTSUP` otherwise, per-construct
    §C.7 / scaffold §4) and the primitive is downgraded to *slot-mapped,
    trigger-unresolved*. The decision: **emit the wrapper best-effort with
    the trigger pin documented in generated collateral (recommended — the
    slot is witnessed and emission is harmless if the action never fires,
    paralleling the A-9 Kconfig-pin treatment), vs diagnose/reject authored
    `on_system_shutdown` until C-4 is confirmed** (Codex's suggested
    downgrade-to-reject).

19. **Citation-shorthand cleanup sweep.** *(Codex review issue 8, residual.)*
    Mutable `latest` URLs are eliminated and README now carries the global
    shorthand-path legend, but many normative claims still cite bare
    basenames (`uart_pl011.c:577`) or sibling sections rather than
    repo-rooted `path:line`. **Recommend a dedicated mechanical sweep**
    (expand every shorthand to the legend's full path, verify line anchors
    still hold) **as its own reviewed change** rather than folding ~3000
    lines of churn into a content fix.

20. **Aligned hardware-status read-back value divergence (L3 whitelist
    policy).** *(Addendum, ratified 2026-06-10 at the backend-zephyr-c C4
    chunk close — the external-review channel per the unit's
    autonomous-run delegation; closes the C4 review's blocking finding
    #1.)* The L3 capture is the LIVE QEMU device model; the committed
    oracle's device model is a passive register file (a read returns the
    last-written or reset-zero state — it cannot carry hardware-status
    semantics like the PL011 FR's reset `TXFE|RXFE = 0x90`). A read-back
    VALUE divergence at an ALIGNED event — op, width, offset, and stream
    position all equal — is therefore whitelistable per-entry on the
    capture side, mirroring the linux first-light precedent (the linux
    GRILL D07 `status-register-read-value` entry; same physics, same
    comparator). The license is VALUE-ONLY: the `on_init` MMIO stream's
    ops, widths, offsets, and ORDER still must match exactly, and an extra
    or reordered MMIO event is never whitelistable (C04) — the comparator
    enforces this structurally (a `matches="linux"` entry fires only at an
    aligned position and consumes both events; it cannot absorb extras).

## B. `needs-new-surface` gaps (blocking — escalate against the model, do not invent)

1. **`poll_until` → `WAIT_FOR`.** *(register-access OQ-5; composition open
   list.)* `recognized_not_lowered` (`domain_attr.rs:94-95`); generic_c emits
   nothing and the convergence doc §9 forbids claiming it. Candidate
   (non-normative): `WAIT_FOR(cond, timeout_us, k_busy_wait(delay_us))`
   (`v3.7-branch include/zephyr/sys/util.h`; untimed degenerate form
   uart_pl011.c:224-226), with the timeout→`RegseqError::Timeout` edge also
   undesigned. **Do not lower before generic_c establishes a reference.**

2. **`BarType::Io` port-I/O.** *(BRM O5.)* `recognized_not_lowered`
   (`device_lifecycle.rs:248-249`). Zephyr's x86 port-I/O surface
   (`sys_in*/sys_out*` + an io-BAR probe) is uncited in-repo; blocked on the
   same model-side Mem/Io unification decision Linux records
   (linux-lowering-per-construct.md:385-390). **Escalate.**

3. **Config-space width-8 and sub-word write RMW.** *(BRM §B.4.)*
   `config_access_width` admits {1,2,4,8}, but Zephyr's accessor is one
   32-bit word op: width 8 = two words or reject; widths 1/2 on the write
   side require a containing-word RMW that is **observable on the bus** —
   needs an explicit decision before silent emission (parity with the Linux
   width-8 gap). **Undesigned.**
   **PARTIALLY LIFTED (2026-06-11, backend-zephyr-c-e1000 D05/C02 — this is
   the explicit decision the entry asked for):** width-1/2 config WRITES
   are DELIVERED via the ratified containing-word RMW
   (`pcie_conf_read(word)` → lane merge → `pcie_conf_write(word)`),
   audit-commented at the emit site (`RSEQ-RMW`, citing the
   `backend-zephyr-c-e1000 GRILL D05` license) and recorded as
   bus-observable: one extra word read; adjacent bytes rewritten with
   read-back values — benign per PCI config semantics (QEMU's
   `e1000_write_config` reacts only to PCI_COMMAND, hw/net/e1000.c:1624).
   The RMW's internal read is always the LIVE word, never the A-4 word-0
   cache. **Width-8 (both ops, the two-word split), 64-bit, misaligned
   sub-word, and non-constant offsets KEEP the D.2 fail-loud rejection.**

4. **Non-PCI config access (BRM-001).** The rejection diagnostic is deferred
   upstream (bus-resource-management-surface-convergence.md:209-211); Zephyr
   inherits the same unbuilt-diagnostic dependency Linux records.

5. **`MatchEntry::Amba` has no Zephyr match target.** *(device-lifecycle Q4.)*
   No AMBA/periphid bus exists; matching is DT-compatible-only
   (uart_pl011.c:9). Whether the model should require a companion
   `Devicetree` entry for Zephyr targets (A-13 recommends the diagnostic) is
   a model-surface decision. **Do not invent a periphid table.**
   **RESOLVED BY RULE (2026-06-10, spec-convergence-pl011 D08/C06):** the
   D03(a) mixed-match rule is ratified — nativeness is per the backend's
   RATIFIED registration form (zephyr_c static-DT ⇒ `Devicetree`); foreign
   entries (e.g. `Amba`) are ignorable WITH a triple-rendered
   `RSEQ-IGNORED-MATCH` audit when ≥1 native entry exists; an all-foreign
   set keeps the A-13 reject naming the fix; no periphid table was invented
   (never-synthesize).

6. **Teardown carrier — `on_deinit` / postamble / orderly teardown.**
   *(device-lifecycle Q1 ≡ RegseqError ZE-OQ2; composition open list.)* The
   3.7-LTS baseline has no device-deinit channel; Zephyr added
   `device_deinit()` in v4.2.0 (absent in v4.1.0 — see C-3 for the pin).
   The synthesized teardown *body* is specified
   ([integration §5.2](./zephyr-integration-probe-path.md)); the *slot* is the
   blocking question, and the answer differs per release (also C-3). Neither
   vendored reference defines any teardown — no in-repo witness exists. **Do
   not invent the macro spelling.**
   **DELIVERED (2026-06-10, spec-convergence-pl011 C04; row
   CAP-ZEPHYR-DT-DEINIT-1 flipped `Delivered` in place per its GRILL D05):**
   on the v4.4.0 pin the slot is `DEVICE_DT_INST_DEINIT_DEFINE` (the macro
   spelling read live, not invented — see the §C verification notes below);
   authored `on_deinit` lowers to `static int <drv>_deinit(const struct
   device *dev)` through the same pipeline + nine-row negated-errno adapter
   as init; unauthored leaves the scaffold byte-identical; the `map_region`
   unmap mirror is an explicit no-op callout (no unmap exists, §5.2).

7. **Surprise-removal representability.** *(device-lifecycle Q2; composition
   open list.)* No hotplug dispatch exists for DT devices; the surprise
   scenario (`body_emit.rs:5323-5339`) may simply not be representable on
   Zephyr — i.e. a documented not-applicable like enable_device-on-platform,
   rather than a lowerable slot. Escalate alongside B-6.
   **RESOLVED BY ROW (2026-06-10): not representable, ratified.** The answer
   is capability row CAP-ZEPHYR-DT-SURPRISE-1 (`AbsentDispatch`, pin v4.4.0
   (684c9e8f32e4), anchors read live from the pinned checkout): the static-DT
   device model is build-time-fixed and no hot-remove dispatch API exists.
   Authored `on_surprise_remove` is now auditably non-emitted per the
   os-capability-disposition rule, never rejected and never silent. See
   `docs/discussions/os-capability-disposition/capability-table.md`.

8. **`UserDataField`/`UserDataKind`.** `recognized_not_lowered`
   (`device_lifecycle.rs:262-279`). The C static-storage zero image inherits
   generic_c's incidental `{0}` guarantee (`body_emit.rs:5301-5306`);
   `OptionCopy(None)` rests on the same all-zero-image assumption. **Inherited,
   not designed — a new backend must not invent the per-kind lowering.**

9. **`ClassTag`/`device_class`.** `recognized_not_lowered`
   (`device_lifecycle.rs:53-54, :134-150`). On Zephyr the class *visibly*
   selects the macro family + API vtable (uart_pl011.c:534-542 vs
   eth_e1000.c:337-344) — exactly the deferred Device Class construct's
   territory (D06/D09). Build-now disposition is A-16 — **as evolved by
   backend-zephyr-c-e1000 D14 (2026-06-11): the reject is replaced by
   audited class-neutral emission (see the A-16 evolution record + the D.2
   row); the construct gap itself stands.**

10. **>1 resolved region per driver.** Rejected today
    (`max_resolved_bindings_per_driver`, `body_emit.rs:5240-5243`); aligns
    with the non-named `DEVICE_MMIO` single-region API. Multi-region needs
    `DEVICE_MMIO_NAMED_*` **and** a model extension threading per-access
    region bases ([integration §2](./zephyr-integration-probe-path.md), the
    per-instance-base note). Model gap first.

---

## C. Version-sensitive pins to confirm (against Zephyr 3.7 LTS / 4.x)

1. **`pcie.h` signature pins: `pcie_conf_read`/`pcie_conf_write` AND
   `pcie_get_mbar` vs `pcie_probe_mbar`.** *(BRM O1; Codex review issue 1.)*
   The word-index (`reg` = byte offset ≫ 2), 32-bit-only claim is pinned to
   `v3.7.0 include/zephyr/drivers/pcie/pcie.h` but was **fetch-unverified in
   the authoring session** (the header is included at eth_e1000.c:18, but the
   vendored driver does not call the pair). Confirm signature + semantics
   before emitting the §B.4 marshalling. Same pin, same header, post-review:
   the spec now materializes `effective_bar` (a BAR **number**) via
   `pcie_get_mbar(bdf, effective_bar, &mbar)`, because `pcie_probe_mbar`'s
   argument is a **memory-BAR ordinal** (the header states the BAR number is
   not that index) — confirm both prototypes and the get/probe
   number-vs-ordinal distinction on the pinned release (also
   fetch-unverified; `pcie_get_mbar` is exercised by no vendored driver —
   the e1000 calls `pcie_probe_mbar(bdf, 0, …)`, eth_e1000.c:219, where the
   two coincide because BAR0 is its first memory BAR).
   **CLOSED — VERIFIED on v4.4.0 (2026-06-11, backend-zephyr-c-e1000 D04;
   see the verification notes below).**

2. **`sys_read64`/`sys_write64` arch-conditionality.** *(register-access
   OQ-2.)* Width-8 accessors exist in `sys_io.h` but availability is
   arch-conditional; confirm per supported target arch or reject width-8 at
   emit time on unsupported arches.

3. **The 4.x `device_deinit()` fork.** *(Q1/ZE-OQ2; pairs with B-6.)* Confirm
   the API's existence, signature, and Kconfig gating on the chosen 4.x
   release before specifying the orderly-teardown slot; on 3.7 LTS the answer
   is definitively "no channel". Release pin (round-2 review-checked, not
   live-verified in-session): `device_deinit()` / `DEVICE_DT_DEINIT_DEFINE`
   are present in **v4.2.0** `include/zephyr/device.h` and **absent in
   v4.1.0** — earlier spec text saying "4.1+" was wrong. Interim behavior
   until the pin is confirmed on the chosen release: all 4.x deinit lowering
   stays **fail-loud** (no `on_deinit` emission), identical to the 3.7
   baseline disposition.
   **RESOLVED on v4.4.0 (2026-06-10, spec-convergence-pl011 — see the
   verification notes below).** The lowering shipped; the interim fail-loud
   sentence above is historical.

4. **PM-device trigger semantics at power-off.** *(device-lifecycle Q3;
   composition open list.)* Which action (`PM_DEVICE_ACTION_TURN_OFF` vs
   `SUSPEND`) fires at system power-off, and under which `CONFIG_PM_DEVICE*`
   options the poweroff path walks devices at all
   (<https://docs.zephyrproject.org/3.7.0/services/pm/device.html>, not
   live-verified). The §C.7 slot mapping stands regardless — but until this
   pin is confirmed, `on_system_shutdown` is **slot-mapped,
   trigger-unresolved**, not a claimed materialization (post-review, Codex
   issue 4; the interim emission policy is A-18).

5. **`device_is_ready()` / `init_res` semantics.** *(device-lifecycle Q9;
   RegseqError ZE-OQ4.)* The failed-init bookkeeping (clamped positive
   `uint8_t` `init_res`, `device_is_ready()` false) is pinned to
   elixir.bootlin.com v3.7.0 `kernel/init.c` + `include/zephyr/device.h` but
   not live-verified; also verify all nine C02 errno values fit `uint8_t`
   across Zephyr's libc choices (minimal/newlib/picolibc) so clamping cannot
   alias variants.

6. **PCIe DT binding spelling + board-DT authoring.** *(device-lifecycle Q5;
   composition open list.)* The `pcie-device.yaml` include + DT-instance PCIe
   properties (intel,e1000.yaml:8; eth_e1000.c:313-318) are witnessed, but
   whether `MatchEntry::Pci { vendor, device }` lowers to a *generated* DT
   binding/overlay or the board DT node is an L2-style integration input
   outside the compiler's output is unratified. (The `effective_bar` mapping
   question formerly here is resolved post-review: `effective_bar` is a BAR
   number and feeds `pcie_get_mbar`, NOT `pcie_probe_mbar`'s memory-BAR
   ordinal — the reference's literal `0` coincides on the e1000 only; the
   remaining signature confirmation lives in C-1.)

7. **Unmap availability.** *(composition open list.)* `DEVICE_MMIO_MAP`/
   `device_map` have no unmap counterpart in the vendored idiom; confirm
   whether the pinned release offers one (`k_mem_unmap` exists but no
   vendored driver uses it) or whether `map_region`'s postamble mirror is
   permanently a no-op on Zephyr ([integration §5.2](./zephyr-integration-probe-path.md)
   already specifies the explicit no-op callout, not an invented unmap).

8. **Zephyr-over-PCI residual scope.** *(register-access OQ-4.)* BRM resolves
   `BusType::Pci` as lowerable (vendored e1000 witness); register-access's
   PCIe `map_region` leg (`device_map` after the BAR probe) is specified
   via BRM §B.3.5 but deferred from the register-access construct's own
   normative scope this stage. Close OQ-4 when the PCIe path is exercised by
   a fixture.

9. **`BUILD_ASSERT` / `DT_NUM_INST_STATUS_OKAY` availability.** *(register-
   access OQ-3's guard, per-construct §A.2.2; post-review.)* The mandatory
   single-instance guard relies on `DT_NUM_INST_STATUS_OKAY` (devicetree API,
   pinned: <https://docs.zephyrproject.org/3.7.0/build/dts/api/api.html>) and
   `BUILD_ASSERT` — both core, long-stable macros, but neither is exercised
   by the vendored drivers and the URLs were not live-fetched. Confirm on the
   pinned release.

### Verification notes — v4.4.0 re-pin (2026-06-10, backend-zephyr-c T04)

Read live from the **local v4.4.0 checkout** (tag `v4.4.0`, commit
`684c9e8f32e4`) in the eval workspace, for the two pins this unit's emitter
and smoke app rely on (GRILL D05/D15; the remaining §C items keep their
3.7-era status and stay folded into the A-19 sweep):

- **C-9 — VERIFIED on v4.4.0.** `BUILD_ASSERT(EXPR, MSG...)` is defined at
  `include/zephyr/toolchain/gcc.h:76` (C++ `static_assert` arm) /
  `gcc.h:87` (`_Static_assert` arm), with the generic fallback at
  `include/zephyr/toolchain/common.h:204`. `DT_NUM_INST_STATUS_OKAY(compat)`
  is defined at `include/zephyr/devicetree.h:3905`. The emitted
  single-instance guard (`BUILD_ASSERT(DT_NUM_INST_STATUS_OKAY(DT_DRV_COMPAT)
  == 1, …)`, A-2/D18) is well-founded on the pinned release.

- **C-5 — VERIFIED on v4.4.0.** `struct device_state` carries
  `uint8_t init_res` (`include/zephyr/device.h:466`) and
  `bool initialized : 1` (`device.h:471`). `do_device_init()`
  (`kernel/device.c:18-50`) records the **positive** value of a failed
  init's negative-errno return, with an explicit `rc > UINT8_MAX` clamp to
  `UINT8_MAX`, and sets `initialized = true` unconditionally;
  `z_impl_device_is_ready()` (`kernel/device.c:186-196`) returns
  `initialized && (init_res == 0U)` — so the generated init fn returning a
  negated errno yields `device_is_ready() == false`, the T14/T16 oracle.
  **Errno aliasing under the `uint8_t` clamp is impossible for the nine C02
  rows:** the seven distinct symbols map to identical values ≤ 116 across
  all three libc choices on the pinned stack — minimal
  (`lib/libc/minimal/include/errno.h`), newlib (SDK 1.0.1
  `aarch64-zephyr-elf` `sys/errno.h`), and the picolibc module
  (`modules/lib/picolibc/newlib/libc/include/sys/errno.h`): `EPERM=1`,
  `EIO=5`, `ENOMEM=12`, `EBUSY=16`, `ENODEV=19`, `EINVAL=22`,
  `EOPNOTSUPP=95`, `ETIMEDOUT=116`.

### Verification notes — deinit delivery (2026-06-10, spec-convergence-pl011)

Read live from the same **local v4.4.0 checkout** (tag `v4.4.0`, commit
`684c9e8f32e4`), for the pins the delivered deinit lowering (C04) and the
eval teardown legs rely on:

- **C-3/B-6/ZE-OQ2 — VERIFIED on v4.4.0.** `DEVICE_DT_DEINIT_DEFINE` is
  defined at `include/zephyr/device.h:243` and the instance form
  `DEVICE_DT_INST_DEINIT_DEFINE` at `device.h:272` (the macro is
  `DEVICE_DT_DEFINE` plus the one `deinit_fn` argument); the runtime API
  `device_deinit()` is declared at `device.h:928`. Error-channel semantics
  (`z_impl_device_deinit`, `kernel/device.c:199-218`): `-EPERM` if not
  initialized; `-ENOTSUP` if no deinit fn or the Kconfig is off; the
  driver's negative rc is returned **verbatim** with `initialized` left
  true on failure; on success `initialized` clears, so
  `device_is_ready()` flips false — the L2 teardown oracle.
- **The Kconfig silent-drop hazard (load-bearing).** `device_ops.deinit`
  exists only under `CONFIG_DEVICE_DEINIT_SUPPORT` (`device.h:504-507`,
  default **off**, `kernel/Kconfig.device:36`), and `Z_DEVICE_OPS` SILENTLY
  drops `deinit_fn` under `IF_ENABLED` when off (`device.h:1175-1180`): the
  build passes and `device_deinit()` returns `-ENOTSUP` at runtime. The
  compiler-staged L1 gate app prj.conf therefore pins
  `CONFIG_DEVICE_DEINIT_SUPPORT=y` (`zephyr_build.rs`), and the generated
  TU header documents the row (A-9 pattern); the eval campaign app prj.conf
  gains the same pin with the L2 teardown leg (spec-convergence-pl011 R07's
  eval half — until that lands, a campaign image answers `device_deinit()`
  with `-ENOTSUP`, the designed loud catch).

### Verification notes — the C-1 `pcie.h` pins (2026-06-11, backend-zephyr-c-e1000 D04)

Read live from the same **local v4.4.0 checkout** (tag `v4.4.0`, commit
`684c9e8f32e4`), closing the C-1 fetch-unverified flag for the five pins the
PCIe lowering consumes (the T04/C-9 pattern):

- **`pcie_conf_read` — VERIFIED.** `uint32_t pcie_conf_read(pcie_bdf_t bdf,
  unsigned int reg)` at `include/zephyr/drivers/pcie/pcie.h:176`; `reg` is
  documented as "the configuration word index (not address)" — the 32-bit
  word-index claim the §B.4 marshalling rests on holds.
- **`pcie_conf_write` — VERIFIED.** `void pcie_conf_write(pcie_bdf_t bdf,
  unsigned int reg, uint32_t data)` at `pcie.h:187`, same word-index
  semantics, no status return (nothing to discard — unlike the Linux
  accessors' `(void)` convention).
- **`pcie_get_mbar` — VERIFIED, the BAR-NUMBER form.** `bool
  pcie_get_mbar(pcie_bdf_t bdf, unsigned int bar_index, struct pcie_bar
  *mbar)` at `pcie.h:237`: `bar_index` is the BAR register number. This is
  the form `effective_bar` (the platform-toml `bar` value) feeds.
- **`pcie_probe_mbar` — VERIFIED DISTINCT, the memory-BAR ORDINAL form.**
  `pcie.h:254` ("index 0 = lowest-numbered memory BAR"); the implementation
  paths are distinct (`drivers/pcie/host/pcie.c:254-266`, separate
  get_bar/probe_bar walks). The vendored e1000's `probe_mbar(bdf, 0, …)`
  coincides with BAR number 0 only because BAR0 is its first memory BAR —
  the emitter uses `pcie_get_mbar`, never the ordinal form (D04).
- **`pcie_set_cmd` — VERIFIED.** `void pcie_set_cmd(pcie_bdf_t bdf,
  uint32_t bits, bool on)` at `pcie.h:293` — the op-per-op enable gate
  (`PCIE_CONF_CMDSTAT_MEM`; `_MASTER` only when `set_bus_master` is
  authored, A-3 record).

---

## D. Completeness ledger

The ledger is **two-level** (restructured post-review, Codex issue 5 — the
old single line read as "nothing is unresolved", which overstated the case):

**D.1 Construct level — unresolved-constructs: []**. No construct failed
specification; all five are specified to the limit of the as-built reference
+ vendored witnesses, with the gaps carried as explicit dispositions rather
than failures.

**D.2 Input-shape level — the disposition ledger.** The input contract
admits shapes whose Zephyr disposition this ledger records explicitly. The
DEFAULT for a shape with **no Zephyr lowering contract** is **fail-loud
rejection at emit time — never silent emission, never silent omission**
(the no-silent-truncation discipline; a deliberate hardening over generic_c,
which carries some shapes unvalidated, e.g. `ClassTag`). Rows marked
DELIVERED / auditable-non-emission / audited record the ratified departures
from that default (each citing its licensing unit); fail-loud applies to
every row that still says reject:

| legal input shape (contract cite) | gap | interim behavior |
|---|---|---|
| `write_config` widths 1/2 (word-RMW) and width 8, both ops (`config_access_width` admits {1,2,4,8}, generic_c `body_emit.rs:2399-2414`) | B-3 | **PARTIALLY LIFTED by backend-zephyr-c-e1000 (2026-06-11, D05/C02):** width-1/2 WRITES are DELIVERED via the audited containing-word RMW (`RSEQ-RMW` block at the emit site, bus-observable: one extra word read, adjacent bytes rewritten with read-back values); width-8 both ops (two-word split), 64-bit, misaligned sub-word, and non-constant offsets KEEP the fail-loud rejection; width-1/2 **reads** were already specified (§B.4) and are delivered with the D06 containing-word-0 cache |
| authored `on_deinit` under `TargetOs::Zephyr` | B-6 / C-3 | **DELIVERED by spec-convergence-pl011 (2026-06-10, C04; row CAP-ZEPHYR-DT-DEINIT-1 `Delivered`)**: lowers to the deinit fn + `DEVICE_DT_INST_DEINIT_DEFINE` swap on the v4.4.0 pin; unauthored stays byte-identical |
| authored `on_surprise_remove` under `TargetOs::Zephyr` | B-7 | **auditable non-emission** per capability row CAP-ZEPHYR-DT-SURPRISE-1 (`AbsentDispatch`, os-capability-disposition rule 2026-06-10): one `RSEQ-ABSENT-SLOT` audit block + conformance-report row + log line, body still fully checked; anything outside the ratified row keeps fail-loud rejection |
| non-Generic `ClassTag` | B-9 | **AUDITED CLASS-NEUTRAL EMISSION (backend-zephyr-c-e1000 2026-06-11, D14/C06 — the A-16 reject replaced):** the same NULL-api scaffold as `ClassTag::Generic` PLUS the `RSEQ-DEFERRED-CLASS` audit comment above the instantiation define + the optional skip-serialized conformance note; never silent, never rejected; class API DELIVERY stays the Device Class construct's (B-9); the Generic path is byte-frozen |
| `MatchEntry::Amba`-only match set | B-5 | reject at emit naming the fix (A-13: add a companion `Devicetree` entry) — RETAINED as the all-foreign arm of the D03(a) rule (spec-convergence-pl011 D08/C06); a MIXED set (foreign entries alongside ≥1 native `Devicetree` entry) now EMITS with each foreign entry triple-render audited (`RSEQ-IGNORED-MATCH` block at the `DT_DRV_COMPAT` pin + report `ignored_match_entries` + line section), never silently dropped |
| `UserDataField` kinds beyond the zero-image guarantee | B-8 | reject at emit (only the inherited all-zero-image shapes pass through) |
| >1 resolved region per driver | B-10 | already rejected (`max_resolved_bindings_per_driver`, `body_emit.rs:5240-5243`) — kept |
| >1 status-okay DT instance while the TU-static base carrier is in effect | A-2 | rejected at **C compile time** via the emitted `BUILD_ASSERT` guard (per-construct §A.2.2) |
| `BarType::Io` | B-2 | reject at emit (recognized-not-lowered upstream; no invented port-I/O surface) |
| `poll_until` | B-1 | parity with generic_c's as-built recognized-not-lowered disposition (convergence §9: do not claim it) — not a Zephyr-specific rejection |
| `on_system_shutdown` trigger contract | C-4 / A-18 | slot emitted (wrapper + `-ENOTSUP` dispatch) but **not claimed as materialized**; interim emit-vs-diagnose = A-18 |

Per-construct status:

| construct | status | residual gaps (all flagged above) |
|---|---|---|
| register-access | **fully specified** — five ops, map_region (both paths), offset recovery, descriptors | `poll_until` (B-1, pre-existing recognized-not-lowered); width-8 arch pin (C-2); OQ-4 scope close-out (C-8) |
| bus-resource-management | **specified for the exercised shapes AND DELIVERED on the PCIe arm** (backend-zephyr-c-e1000 2026-06-11) — selector, four managed ops (claim = called-out elision; the D02 probe→enable→map deviation audit-rendered), config width-4 both directions + width-1/2 reads (D06 word-0 cache) + width-1/2 writes (D05 RMW), carriers (`__rseq_pcie` TU-static, D12); Zephyr-over-PCI resolved affirmatively and EMITTING; width-8/64-bit/misaligned config ops are D.2 rejections | `BarType::Io` (B-2); config width-8 remainder (B-3); BRM-001 diagnostic (B-4); ~~`pcie.h` pins (C-1)~~ — C-1 CLOSED, verified on v4.4.0 |
| device-lifecycle | **bring-up half fully specified** (register/match/create/associate/preamble/on_init/init-fail); **teardown half DELIVERED on the v4.4.0 pin** (spec-convergence-pl011 C04: deinit fn + macro swap; B-6 closed); shutdown = **PM slot mapped, trigger contract unresolved** (C-4/A-18) | surprise representability (B-7, resolved-by-row), `MatchEntry::Amba` (B-5, resolved-by-rule D08), `UserDataField` (B-8), `ClassTag` (B-9), multi-region (B-10) |
| diagnostics-logging | **fully specified** — four levels, template reassembly, numeric + `&str` arguments, TU scaffold | non-static `&str` divergences are ratify-able policy (A-5/A-6), not gaps; Kconfig pins (A-9/C) |
| RegseqError | **fully specified** — marker, carrier, nine-row table (symbol-identical, negated), adapter fusion, two-channel rule | deinit return channel CLOSED (B-6/C-3 delivered: rc returned verbatim, kernel/device.c:199); rc observability (A-11); `uint8_t` clamp verified (C-5) |

**Cross-construct invariants re-verified end-to-end** (the seven rules,
[integration §6](./zephyr-integration-probe-path.md)): canonical preamble
order across binding times; I04 on both paths; the two-error-channel split;
the claim elision called out; the enable/master bit-selection fusion; the
`const struct device *` join point; the scenario collapse.

**Citation discipline note.** Every normative claim above and in the sibling
documents cites a repo `path:line` or a pinned Zephyr doc/source URL (all
docs.zephyrproject.org links pinned to `3.7.0` post-review — no `latest`);
shorthand basenames resolve per the README legend; in-repo vendored-driver
witnesses corroborate every macro-level usage claim. URLs were not
live-fetched during authoring **or during the post-review fix pass** — claims
resting on a URL beyond what the vendored drivers corroborate are in section
C, not asserted. The residual shorthand-expansion sweep is A-19.
