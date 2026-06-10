STATUS: INCOMPLETE

# Backend-lowering — morning hand-off

_Generated 2026-06-09 (evening pass; supersedes the earlier same-day `_STATUS.md`,
which predated the Zephyr track and reported sandbox-aborted reviews — both
claims are now false; see `_CROSS-OS-REVIEW.md` F3). Cold-start summary for the
backend-lowering spec effort: one OS-neutral foundation + per-OS sweeps over
exactly five constructs (register-access, bus-resource-management,
device-lifecycle, diagnostics-logging, RegseqError). Read §1 first — it is the
exact gap list._

---

## 1. Exactly what is missing

**No track failed. No construct failed or is unresolved on either OS**
(`failed_tracks=[]`; Linux 5/5 constructs specified, `failed=[]`,
`unresolved=[]`; Zephyr 5/5 constructs specified, `failed=[]`, `unresolved=[]`).
What blocks DRAFT-COMPLETE is **reviews**: the foundation's inventory review and
seam review both returned `ok=False` with concrete findings, and neither per-OS
track holds a green external review (`review_ok=False` for both Linux and
Zephyr — Codex returned `ok=False` on each; findings were dispositioned in-spec
but no re-review has produced a pass verdict).

### 1a. Inventory review (`primitive-inventory.md`) — `ok=False`, 5 missing items

Verbatim from the review:

1. Absent model enum variants: `TargetOs::Linux` and `TargetOs::Zephyr` are not
   inventoried; they are the closed OS target vocabulary at
   `crates/regseq-model/src/target.rs:69`, parsed by `TargetOs::from_token` at
   `crates/regseq-model/src/target.rs:91`, and carried by `PlatformIdentity.os`
   at `crates/regseq-model/src/target.rs:114`.
2. Absent model enum variant: `BarType::Mem` is not inventoried; it is the
   default MMIO carrier at `crates/regseq-model/src/device_lifecycle.rs:244`,
   paired with `BarType::Io` at `crates/regseq-model/src/device_lifecycle.rs:248`.
3. Mis-cited backend detail: the `Mmio<T> resource + #[rseq::register_block]
   offset recovery` row cites only the function start at
   `crates/regseq-backends/src/generic_c/body_emit.rs:2439`; the claimed
   `Field(idx)` offset recovery is at
   `crates/regseq-backends/src/generic_c/body_emit.rs:2461` and the width
   recovery is at `crates/regseq-backends/src/generic_c/body_emit.rs:2514`.
4. Mis-cited backend detail: the `format template + placeholder modifiers` row
   cites `Modifier` shape at
   `crates/regseq-backends/src/generic_c/body_emit.rs:5871`, but the accepted
   eight hex forms are enumerated at
   `crates/regseq-backends/src/generic_c/body_emit.rs:5966`.
5. Mis-cited backend detail: the `entry-shim classification` row cites
   `crates/regseq-backends/src/generic_c/naming.rs:84` for B3 early errno
   return; the actual `rc != 0` early return is at
   `crates/regseq-backends/src/generic_c/naming.rs:181`.

The review also raised four accuracy issues (not absences): (i) it CONFIRMS the
inventory covers the current `IntrinsicKind`, `MmioOp`, `ConfigOp`, `LogLevel`,
`FrameworkType::RegseqError`, and all five lifecycle callbacks
(`crates/regseq-model/src/domain_attr.rs:77,:88,:110,:125,:141`,
`crates/regseq-model/src/device_lifecycle.rs:152`); (ii) `set_bus_master`
should be labelled **gate-lowered**, not plain lowered — emitted only for
`is_pci && dma_present` (`body_emit.rs:5267`), `dma_present` defaults false
(`device_lifecycle.rs:62`), no fixture emits it (`prelude.rs:212`); (iii) the
`create` row conflates the always-emitted `regseq_lc_create()` trace marker
(`body_emit.rs:5206`) with the `needs_inst`-gated zero-init `inst = {0}`
(`body_emit.rs:5287,:5301`); (iv) the Field/FieldValue row cites stale
`set(...)` text in `register-access-surface-convergence.md:243` — the closed op
set is `domain_attr.rs:125`, matched at `body_emit.rs:2282`.

### 1b. Seam review (`agnostic-core-vs-os-shell.md` + `regseqmodel-input-contract.md`) — `ok=False`, 7 issues

Verbatim findings (condensed; citations preserved):

1. **The input contract is not exact.** It claims `RegseqModel` is
   target-agnostic and the listed surface is everything consumed
   (`regseqmodel-input-contract.md:9-11`), but `RegseqModel` exposes
   `target: TargetInfo` (`crates/regseq-model/src/model.rs:60-62`) and
   `body_emit` reads `model.target.pointer_width`/`endian`
   (`body_emit.rs:370-371,:3161-3164,:3203-3209`); the backend also consumes
   out-of-band `BackendContext` (`backend_options.rs:117-127`, `mod.rs:237-242`).
   An implementer following the table misses ABI/target data.
2. **Both docs overcount call-intercepted intrinsics** — they say four
   constructs ride on `Call` interception
   (`agnostic-core-vs-os-shell.md:175-184`,
   `regseqmodel-input-contract.md:159-163`), but `call()` intercepts only
   `Log`, `Mmio`, `Config` (`body_emit.rs:2100-2125`); `PollUntil` has no emit
   arm (`domain_attr.rs:88-100,:167-172`; convergence doc says don't claim it,
   `register-access-surface-convergence.md:424-427`).
3. **RegseqError/lifecycle boundary overstated** — the contract says the
   backend touches RegseqError at the outermost fallible boundary including
   `on_init` (`regseqmodel-input-contract.md:363-368`), but `generic_c` only
   materializes the errno adapter for the entry return
   (`mod.rs:276-294`, `body_emit.rs:4732-4828`); lifecycle init-fail just reads
   the niche discriminant for trace markers + optional `on_init_failure`
   (`body_emit.rs:5411-5420,:5464-5482`;
   `device-lifecycle-surface-convergence.md:218-227`).
4. **"Rollback" overstated** — the init-fail path is called a rollback core
   mechanism (`agnostic-core-vs-os-shell.md:203-210`), but no high-water-mark
   release exists yet (`body_emit.rs:5470-5482`); the surface doc records the
   partial-postamble rollback as deferred
   (`device-lifecycle-surface-convergence.md:222-226`).
5. **Name-reservation described too broadly** — `prelude_ordinary_idents`
   reserves only panic/arith/cast helpers + lifecycle trace hooks
   (`mod.rs:1099-1148`), not the always-emitted `regseq_mmio_read/write`
   (`prelude.rs:87-102`) or conditional `regseq_bus_*` (`prelude.rs:188-224`),
   contra `agnostic-core-vs-os-shell.md:67-72`.
6. **The docs imply a materialization seam that does not exist as-built** —
   "re-point materialization targets" (`agnostic-core-vs-os-shell.md:8-10,
   :286-290`) vs the assembly directly pushing generic-c prelude blocks
   (`mod.rs:296-324`) and hard-coded host hook names at the emit sites
   (`body_emit.rs:2271-2281,:5150-5153,:5271-5283`). A new backend cannot swap
   a layer without refactoring. (This is the same fact Linux F-1 escalates.)
7. **Stale typed-`LogArg` hint** (`regseqmodel-input-contract.md:354-359`) —
   generic-c reads `args[0]` as template bytes + `args[1..]` raw operands and
   never reconstructs `LogArg` (`body_emit.rs:2732-2740`;
   `logging-and-printing-discussion.md:424-429`).

### 1c. Per-OS external reviews — neither track is review-green

| track | constructs | failed/unresolved | external review state |
|---|---|---|---|
| **Linux** | 5/5 specified | none | Codex `ok=False`, 9 issues — all dispositioned (fixed in-spec or routed to A-3/A-10/A-11/C-3/F-1), **no re-review pass verdict** |
| **Zephyr** | 5/5 specified | none | Codex `ok=False`, 8 issues — all dispositioned (post-review fix pass applied, routed items in A/B/C), **no re-review pass verdict** |

### 1d. What closing the gap requires

(1) Fix the five inventory absences/mis-cites + four accuracy issues in
`primitive-inventory.md`; (2) correct the seven seam-review overstatements in
`agnostic-core-vs-os-shell.md` / `regseqmodel-input-contract.md`; (3) ratify
the decisions in §3 (at minimum the pre-implementation set); (4) re-run all
three reviews to a pass verdict.

---

## 2. What was produced

### 2a. Foundation — `docs/discussions/backend-lowering/`
- `primitive-inventory.md` — the whole-model primitive census (review-flagged, §1a).
- `regseqmodel-input-contract.md` — what a backend consumes from `RegseqModel` (review-flagged, §1b).
- `agnostic-core-vs-os-shell.md` — the core/shell seam analysis (review-flagged, §1b).
- `model-analysis-strategy.md` — the per-OS sweep method (stale Zephyr-PCI deferral; cross-OS F2).
- `foundation-review.md`, `README.md`, `_CROSS-OS-REVIEW.md` (this directory's consistency review).

### 2b. Linux track — `docs/discussions/linux-backend/` (8 files)
Full five-construct lowering spec pinned to mainline LTS 6.6+: per-construct
lowering, model analysis/sweep, primitive materialization, scaffold-and-dispatch
(module init + `.probe`/`.remove`/`.shutdown`), integration probe path,
traceability matrix, open-questions ledger, Codex review. Status vocabulary:
every construct is **core-subset lowerable** (the subset `generic_c` actually
lowers has a confident cited Linux lowering); **no construct is complete** —
residuals are dispositioned in `linux-open-questions.md` A/B/C.

### 2c. Zephyr track — `docs/discussions/zephyr-backend/` (8 files)
Full five-construct lowering spec pinned to Zephyr 3.7 LTS (4.x forks flagged):
same 8-file layout. Zephyr-over-PCI resolved **affirmatively** (vendored e1000
witness) — this supersedes the foundation's deferral (cross-OS F2).
`claim_regions` = called-out elision; teardown **not lowered on the 3.7
baseline** (no device-deinit carrier — a cited property of the OS, not a spec
failure); `on_system_shutdown` = PM-slot-mapped, trigger-unresolved. D.2 carries
an explicit fail-loud rejection ledger for every legal-but-unlowered input shape.

### 2d. Cross-OS review — `_CROSS-OS-REVIEW.md`
Verdict: the two specs **mostly agree** on the shared agnostic core, input
contract, composition rule, and the authored-vs-native error split. Five
findings F1–F5: one real pending semantic divergence (F1, Zephyr PCIe order —
decision §3 #5), stale foundation text (F2), the stale `_STATUS.md` (F3 —
resolved by this file), non-comparable completeness vocabulary (F4), and the
Zephyr citation-shorthand sweep (F5).

---

## 3. Ranked human decisions to ratify

Pulled from `linux-open-questions.md` (A/C/F), `zephyr-open-questions.md` (A),
and `_CROSS-OS-REVIEW.md`. Ranked: implementation-blockers first, then
cross-OS-coherence, then policy defaults, then hygiene. Recommendations in
**bold** are the spec authors'; every item is argued with citations at the
linked source.

**Tier 1 — blocks any backend implementation**

1. **[Linux F-1, P0; = seam issue 6] How a new backend obtains the agnostic
   core.** No factored materialization seam exists as-built. Options:
   (a) **factor the seam out of `generic_c` first** (the
   `linux-scaffold-and-dispatch.md` §8 table is already the parameterization
   list; regression-guarded by the byte-stable corpus) vs (b) fork the assembly
   into a sibling backend. **Recommend (a).** Prerequisite work, out of spec scope.
2. **[Linux A-10, P0] The preamble gate** — keep host `is_resolved()` (L2
   literal `base&&size` required as trigger even though Linux discards the
   values) vs a Linux `mappable` predicate (model/backend gate extension).
   **Recommend keeping (a) for the first implementation, escalating (b) as the
   durable fix.**
3. **[Linux A-3] Diagnostics device-context strategy** — how a lowered body
   reaches `struct device *`: signature threading vs `container_of` recovery
   vs **intentional `pr_*` fallback (recommended; pinned interim rule)**.
4. **[Linux A-11] PCI config-accessor status disposition** — the model's
   `read_config` is infallible; v6.6 accessors return `int`. **Recommend
   explicit `(void)` discard** + file the fallible-surface model extension
   with the construct owner.

**Tier 2 — cross-OS coherence (the one real divergence + pins)**

5. **[Zephyr A-3 ≡ cross-OS F1] Zephyr PCIe preamble order** — ratify
   `probe → enable → map` (the vendored e1000 order) as a formal per-OS shell
   permutation of the canonical `enable → handle → map`, or force canonical
   order. **Recommend adopting the reference order as the single recorded
   deviation.** Until ratified this is the only standing cross-OS semantic
   inconsistency.
6. **[Linux C-3] Platform teardown slot** — ratify the `.remove_new` pin for
   the 6.6+ target (`.remove` becomes `void` only at v6.11; `.remove_new`
   later retired), or adopt a version-gated signature policy. Determines D-3
   (no return channel for `on_deinit`'s `Result`).
7. **[Zephyr A-1] `(base,size)` provenance** — **DT `reg` as the normative
   source** with platform.toml literals as advisory cross-check; decides
   whether a Zephyr TU needs a platform.toml at all.
8. **[Zephyr A-2] Base threading** — **TU-static base + mandatory
   `BUILD_ASSERT(DT_NUM_INST_STATUS_OKAY == 1)` guard** now, graduating to
   per-device `DEVICE_MMIO_GET(dev)` when the container-morph threading lands;
   or require per-device threading up front.
9. **[Zephyr A-18] `on_system_shutdown` interim policy** — **emit the
   `pm_device_action` dispatch wrapper best-effort with the trigger pin
   documented** (slot witnessed, trigger Kconfig/version-sensitive, C-4) vs
   Codex's downgrade-to-reject.

**Tier 3 — policy defaults to ratify (recommendations exist; none blocks the spec)**

10. **[Linux A-2]** Managed `devm_*`/`pcim_*` vs bare acquisition —
    **recommend managed** (postamble delegated to devres).
11. **[Linux A-1]** `readl`/`writel` for Mem BARs, `ioread*`/`iowrite*`
    reserved for the Io path — **recommended split**.
12. **[Linux A-6/D-2]** RegseqError shape — **faithful carrier + boundary
    adapter** (as-built), NOT per-site `-errno`; confirm the probe-site fusion
    of Err-cleanup + negated-errno return.
13. **[Linux A-7]** One `.remove()` + runtime `present` predicate vs two
    teardown paths — **recommend one + predicate**.
14. **[Zephyr A-5/A-6]** `&str` logging under deferred mode — **call-site
    bounce buffer** for non-static; **fail-loud at emit** for interior-NUL
    statics. Pairs with **[Linux A-4]** (`%.*s` + documented divergence).
15. **[Linux A-5 / Zephyr A-7]** Debug-level gating — **honor idiomatic
    dynamic-debug / Kconfig module level** (divergence from generic_c's
    always-on documented on both OSes).
16. **[Zephyr A-10]** `Unsupported` errno — **stay symbol-identical
    `-EOPNOTSUPP`** (don't switch to Zephyr's `-ENOTSUP` idiom).
17. **[Zephyr A-15/A-16/A-17]** Init level/priority knob (**`BackendOptions`
    knob, `POST_KERNEL` default**); class-neutral scaffold (**build now for
    `ClassTag::Generic` only, non-Generic = emit diagnostic**); config/data
    split (**whole driver into `data` now**).
18. Remaining low-stakes defaults: Linux A-8/A-9 (AMBA `.shutdown` placement;
    Result-discard on void teardown), Zephyr A-4 (offset-0x00 config from
    `dev->pcie->id`), A-8 (module-name fallback), A-9 (Kconfig pins document-
    not-assert), A-11 (rc observability via `LOG_ERR`), A-12/A-13 (diagnostics
    for `on_surprise_remove` / Amba-only match sets).

**Tier 4 — documentation coherence**

19. **[Cross-OS F4]** Adopt a shared status vocabulary ("sweep complete" /
    "core subset lowerable" / "specified for exercised shapes" / "complete
    construct lowerable") and apply it to both ledgers.
20. **[Cross-OS F2]** Update `model-analysis-strategy.md`'s stale Zephyr-PCI
    deferral to point at the affirmative Zephyr resolution.
21. **[Zephyr A-19 ≡ cross-OS F5]** The mechanical citation-shorthand
    expansion sweep, as its own reviewed change.

**Escalations (NOT ratifiable here — `needs-new-surface` model gaps; escalate
to the construct owners, do not invent in a backend).** Shared by both OSes:
`poll_until` (no generic_c reference), `BarType::Io` port-I/O, config width-8
(+ Zephyr sub-word write RMW), non-PCI config diagnostic (BRM-001), >1 resolved
region, `MatchEntry` multi-row tables, `UserDataField`, `ClassTag`,
i2c/spi/usb. Zephyr-only: teardown carrier on 3.7 vs 4.x `device_deinit()`
(B-6/C-3), surprise-removal representability (B-7), `MatchEntry::Amba` has no
Zephyr target (B-5). Full lists: `linux-open-questions.md` §B,
`zephyr-open-questions.md` §B.

---

## 4. Upstream review verdicts (persisted)

| review | verdict | findings | disposition |
|---|---|---|---|
| **inventory_review** (foundation census) | `ok=False` | 5 missing (2 absent enum families, 3 mis-cites) + 4 accuracy issues | **unfixed** — itemized verbatim in §1a; fix in `primitive-inventory.md` |
| **seam_review** (core/shell + input contract) | `ok=False` | 7 issues (overcounted intercepts, overstated RegseqError/rollback claims, inexact input contract, no as-built seam, stale `LogArg`) | **unfixed** — itemized in §1b; fix in the two foundation docs; issue 6 already escalated as Linux F-1 |
| **Linux Codex (GPT-5.5) external review** | `ok=False`, 9 issues (3×P0, 3×P1, 2×P2, 1×P3), `unresolved_or_failed_constructs=[]` | EXECUTED 2026-06-09 (after an earlier sandbox-aborted zero-finding attempt) | every issue fixed in-spec or routed to A-3/A-10/A-11/C-3/F-1; verbatim output + per-issue table in `../linux-backend/codex-review.md`; **no pass verdict until ratified + re-reviewed** |
| **Zephyr Codex (GPT-5.5) external review** | `ok=False`, 8 issues (5×High, 3×Medium) | EXECUTED 2026-06-09 | post-review fix pass applied (e.g. `pcie_get_mbar` vs `pcie_probe_mbar` BAR number/ordinal fix; `on_system_shutdown` downgraded to slot-mapped/trigger-unresolved; ledger restructured two-level); disposition log in `../zephyr-backend/codex-review.md`; **no pass verdict** |
| **Cross-OS consistency review** | mostly-consistent, 5 findings (F1–F5) | this directory's `_CROSS-OS-REVIEW.md` | F1 = decision §3 #5; F2/F4/F5 = §3 #19–21; F3 resolved by this file |

---

## 5. Cold-start pointers

- Foundation method + citation rule: `README.md`, `model-analysis-strategy.md`.
- The two per-OS ledgers (read these before anything else in their
  directories): `../linux-backend/linux-open-questions.md`,
  `../zephyr-backend/zephyr-open-questions.md`.
- Scope guard: exactly five constructs; interrupts/DMA/data-path are OUT.
  Cross-construct composition (BRM preamble inside lifecycle synthesis;
  `map_region` consuming the BRM-resolved `(base,size)`; authored RegseqError
  ≠ native OS errors) is load-bearing in both specs — preserve it in any edit.
