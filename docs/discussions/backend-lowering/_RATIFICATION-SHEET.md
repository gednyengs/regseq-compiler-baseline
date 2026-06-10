# A-series ratification sheet — 2026-06-09

One line per pending policy decision (A-series of both OS ledgers). **Bold IDs gate
implementation start** (Linux A-3/A-10/A-11/A-12); everything else can be ratified any
time. "If rejected" names the live alternative and its cost. Full rationale + citations
live in the source ledgers (`linux-backend/linux-open-questions.md` §A,
`zephyr-backend/zephyr-open-questions.md` §A) — this sheet decides, the ledgers explain.

**Verdict semantics:** an empty `Verdict` cell means **the recommendation stands as
ratified by default**. Only the four **bold gating rows** require an explicit verdict
before implementation (`ACCEPT`, or the chosen alternative in a word or two). Fill any
non-gating cell only to override. Dispositions get applied back into the ledgers as
owner ratifications.

## Linux (12)

| ID | Decision | Recommendation | If rejected → alternative & cost | Verdict |
|---|---|---|---|---|
| A-1 | MMIO accessor family | `readl`/`writel` for Mem BARs; `ioread*`/`iowrite*` reserved for the Io path | Uniform `ioread*` everywhere — subsumes the `BarType::Io` case (B-2) but less idiomatic for the dominant Mem case | |
| A-2 | Resource acquisition | `devm_*`/`pcim_*` managed by default → postamble emits nothing (devres unwinds) | Bare `pci_*` → explicit reverse-ladder `remove()`; manual HWM unwind on probe failure | |
| **A-3** | Device context for `dev_*` logging | **(iii)** `pr_*` in all lowered ordinary bodies; `dev_*` only in scaffold-synthesized statements | (i) thread `dev` through every reachable signature (invasive, changes lowered ABI) or (ii) `container_of` recovery (doesn't cover non-driver receivers) | **RATIFIED: (iii)** — `dev_*` where a `struct device*` is lexically in scope at the emission site as-built (scaffold-synthesized statements); `pr_*` everywhere else. No threading, no `container_of` |
| A-4 | `&str` interior-NUL | Idiomatic `%.*s` + documented divergence (truncates at NUL) | Byte-faithful per-byte emission / `%*ph` hexdump — faithful but unidiomatic log lines | |
| A-5 | `dev_dbg` gating | Honor dynamic-debug gating (debug lines compile out by default) | Force always-on for generic_c parity — unidiomatic, log spam in production builds | |
| A-6 | RegseqError shape (+ D-2 probe fusion) | As-built faithful carrier + boundary adapter; confirm `.probe` fuses Err-cleanup with negated-errno return | Per-site `return -errno` (perspective-6 sketch) — discards the certified discriminant machinery, reshapes helpers | |
| A-7 | `.remove()` orderly/surprise | One `.remove()` branching on a runtime `present` predicate (`pci_device_is_present`) | Always-orderly single path — loses surprise-teardown semantics generic_c distinguishes | |
| A-8 | AMBA `.shutdown` placement | Confirm: lives on `device_driver.shutdown` (`.drv`), not `amba_driver` | (Pure confirmation — no real alternative; rejecting means re-research the field placement) | |
| A-9 | `on_init` Result discard on void slots | Discard-and-optionally-`dev_err` (CR-4: void `.remove`/`.shutdown` can't carry it) | Emit-time diagnostic forbidding Result-returning deinit — stricter, rejects valid specs | |
| **A-10** | Preamble gate on Linux | **(a)** keep as-built `is_resolved()` literal gate — L2 must carry semantically-dead `base`/`size`; escalate (b) as the durable fix | (b) now: Linux `mappable` predicate (`effective_bar.is_some()`) — a model/backend gate extension before any implementation exists | **RATIFIED: (a)** + emit-time fail-loud diagnostic (MMIO ops with unresolved binding = hard error); (b) escalated as durable fix (B-11) |
| **A-11** | Infallible config intrinsics vs fallible `pci_*_config_*` | Explicit `(void)` discard (reads **and** writes), visible in generated C; file the fallible-surface model extension with the construct owner | Discard+`dev_warn` (noise policy unpinned), reject-at-emit (blocks config users), or extend the model now (cross-construct scope creep) | **RATIFIED:** explicit `(void)` discard, reads + writes; fallible-surface extension escalated (B-12) |
| **A-12** | Width-8 MMIO on non-`CONFIG_64BIT` | 64-bit-only: `readq`/`writeq` + fail-loud `#error` guard in any width-8 TU | Pick one `io-64-nonatomic-*` header globally (wrong split order for some devices, silently) or a per-block split-order model attribute (needs-new-surface) | **RATIFIED:** 64-bit-only + `#error` guard; split-order attribute only if a real driver demands it |

## Zephyr (19)

| ID | Decision | Recommendation | If rejected → alternative & cost | Verdict |
|---|---|---|---|---|
| A-1 | `(base,size)` provenance | DT `reg` is normative; platform.toml literals advisory cross-check (mismatch = diagnostic) | Literal-only `device_map` (ignores DT — unidiomatic) or DT-only (loses the cross-check). Decides whether a Zephyr TU needs platform.toml at all | |
| A-2 | Base threading into op sites | Faithful first: TU-static `mm_reg_t` + mandatory `BUILD_ASSERT(single-instance)`; graduate to `DEVICE_MMIO_GET(dev)` when device threading lands | Require per-device threading up front — blocks on the device-lifecycle container-morph seam before any Zephyr emission works | |
| A-3 | Zephyr-PCIe preamble order | Adopt reference order (probe→enable→map) as THE recorded per-OS deviation from canonical | Keep canonical enable→handle on Zephyr-PCI — contradicts the only vendored reference (BAR sizing with decode off) | |
| A-4 | Offset-0x00 `read_config` | Serve from cached `dev->pcie->id` (zero bus traffic; matches host caching semantics) | Always issue real `pcie_conf_read` — diverges from as-built caching rule | |
| A-5 | Non-static `&str` log args (deferred mode) | Call-site bounce buffer: copy `min(len,CAP-1)`, NUL-terminate, `%s`; divergences documented | `CONFIG_LOG_MODE_IMMEDIATE` (config-coupled, keeps NUL gap) or `LOG_HEXDUMP_*` (byte-faithful, not a formatted line) | |
| A-6 | Static `&str` with interior NUL | Fail-loud at emit time (bytes known at emission); `LOG_HEXDUMP_DBG` documented opt-in | Silent `%s` truncation — violates the construct's fail-loud discipline | |
| A-7 | Compile-time log level | Honor per-driver Kconfig module level (idiomatic; mirrors Linux A-5) | Force `LOG_LEVEL_DBG` for generic_c always-on parity — unidiomatic | |
| A-8 | Module-name fallback (no `DriverDescriptor`) | Sanitized TU emission name; descriptor seam stays for lifecycle TUs | Require a descriptor / hard error — rejects descriptor-less TUs that are otherwise valid | |
| A-9 | Logging Kconfig pins | Document in generated collateral now; asserting belongs to the future build harness | Assert now — no Zephyr build harness exists to host the assert | |
| A-10 | `Unsupported` errno symbol | Keep `-EOPNOTSUPP` (symbol-identical to the C02 table; valid in Zephyr) | Zephyr-idiomatic `-ENOTSUP` — breaks the table's per-target invariance | |
| A-11 | Standalone-entry rc observability | `LOG_ERR` the nonzero rc before return (kernel ignores `main()` rc) | Leave unobservable until a harness channel exists | |
| A-12 | Authored `on_surprise_remove` on Zephyr | Compile-time diagnostic (callback can't run on this target; silence = truncation) | Silent omit — violates no-silent-truncation. (Representability itself is B-7) | |
| A-13 | `MatchEntry::Amba`-only match set | Reject with a diagnostic naming the fix (add `Devicetree{compatible}`; PL011 precedent) | Silent skip or auto-synthesized compatible — invents device identity. (Model surface is B-5) | |
| A-14 | `descriptor.name` carrier | Spend it on the log module name + emitted symbol prefixes (vendored idiom) | Drop the name (loses authored intent); no single Zephyr runtime slot exists either way | |
| A-15 | Init level/priority knob | `BackendOptions` knob, default `POST_KERNEL` + kernel default priority; revisit at Device Class construct | Hardcode a level — wrong for some classes; the class is what couples the level | |
| A-16 | Class-neutral scaffold now | Yes — `DEVICE_DT_INST_DEFINE` + `NULL` api for `ClassTag::Generic` ONLY; any non-Generic tag = emit diagnostic | `NULL` api for declared classes (device invisible to class consumers = silent truncation) or defer all scaffolds (blocks everything on Device Class) | |
| A-17 | Config(ROM)/data(RAM) split | Build-now: whole spec driver into `data`; split is a later refinement | Implement the split now — premature; model's `DriverHandle` doesn't distinguish the halves yet | |
| A-18 | `on_system_shutdown` interim policy | Emit the `pm_device_action` dispatch wrapper best-effort, trigger pin documented (harmless if action never fires) | Diagnose/reject authored `on_system_shutdown` until C-4 trigger confirmed (Codex's stricter suggestion) | |
| A-19 | Citation-shorthand sweep | Dedicated mechanical sweep as its own reviewed change | Fold ~3000 lines of citation churn into a content fix — review noise, regression risk | |

## Notes

- Linux A-8 and A-6's D-2 half are *confirmations* (yes/no), not choices.
- Cross-OS rhymes worth deciding consistently: Linux A-5 ↔ Zephyr A-7 (debug gating),
  Linux A-4 ↔ Zephyr A-5/A-6 (string faithfulness), Linux A-9 ↔ Zephyr A-11/A-18
  (unobservable results: log-and-continue).
- B-series (model-surface gaps) and C-series (version pins) are intentionally absent:
  B escalates to the construct pipeline, C is delegable verification.
