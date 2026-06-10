## Review loop PASS — review_loop.py, 2026-06-09

Round history:
- round 1: ok=False (2 issues)
- round 2: ok=True

Final: green=True

Still-open issues at loop end:
- (none)

---

# Codex external review — Zephyr backend spec

External review of `docs/discussions/zephyr-backend/` by Codex (GPT-5.5), the
project's external review model. Verdict, issues, and suggestions are
reproduced **verbatim** below; the disposition log at the end records what was
fixed in the spec and what was routed to
[`zephyr-open-questions.md`](./zephyr-open-questions.md) as a human decision.

---

## Verdict (verbatim)

```
verdict ok=False
```

## Issues (verbatim)

1. High: PCI BAR lowering uses the wrong Zephyr API contract. The spec maps
   `MmioBinding.effective_bar` directly to `pcie_probe_mbar(..., effective_bar, ...)`
   in `zephyr-lowering-per-construct.md:451`, `zephyr-primitive-materialization.md:49`,
   and `zephyr-traceability-matrix.md:48`. Zephyr 3.7 says `pcie_probe_mbar()`
   takes a memory-BAR ordinal and explicitly says the BAR number is not that
   index; actual BAR number lookup is `pcie_get_mbar()`
   (https://github.com/zephyrproject-rtos/zephyr/blob/v3.7.0/include/zephyr/drivers/pcie/pcie.h#L1819-L1838).
   This will silently map the wrong BAR for legal inputs where
   `effective_bar != 0`.

2. High: the proposed TU-static MMIO base is not implementable for Zephyr
   multi-instance drivers. The spec says Zephyr emits one static device per
   status-okay DT instance in `zephyr-scaffold-and-dispatch.md:95`, but also
   recommends a translation-unit static `mm_reg_t` in
   `zephyr-open-questions.md:41` and `zephyr-lowering-per-construct.md:154`.
   That conflicts with Zephyr’s per-instance `DEVICE_DT_INST_DEFINE()` model
   (https://docs.zephyrproject.org/3.7.0/kernel/drivers/index.html) and with
   the vendored PL011 pattern using per-device `DEVICE_MMIO_RAM`/`DEVICE_MMIO_GET`
   at `reference-drivers/pl011/zephyr/uart_pl011.c:63` and
   `reference-drivers/pl011/zephyr/uart_pl011.c:577`.

3. High: probe composition ordering is internally inconsistent. The input
   contract and generic_c reference order are
   enable -> claim -> set_bus_master -> get_resource_handle -> map_region
   (`regseqmodel-input-contract.md:340`,
   `crates/regseq-backends/src/generic_c/body_emit.rs:5256`). The Zephyr PCI
   spec instead recommends probing BAR before enabling PCI MEM in
   `zephyr-open-questions.md:51` and `zephyr-scaffold-and-dispatch.md:137`,
   while also claiming order preservation in
   `zephyr-lowering-per-construct.md:338`. Platform ordering is also split:
   `zephyr-integration-probe-path.md:84` puts reset/clock/pinctrl before
   map_region, but `zephyr-lowering-per-construct.md:125` says
   `DEVICE_MMIO_MAP` is first statement, matching PL011 at
   `reference-drivers/pl011/zephyr/uart_pl011.c:577`.

4. High: `on_system_shutdown` is marked as materialized, but the PM mapping is
   under-specified. The spec maps it to a PM-device slot in
   `zephyr-primitive-materialization.md:72` and
   `zephyr-scaffold-and-dispatch.md:161`, while admitting the actual PM action
   and Kconfig trigger are not asserted in
   `zephyr-lowering-per-construct.md:902`. Zephyr PM callbacks are action
   callbacks over `enum pm_device_action`, with unsupported actions returning
   `-ENOTSUP` (https://docs.zephyrproject.org/3.7.0/services/pm/device.html);
   that is not a dedicated system-shutdown callback.

5. High: several legal input-contract shapes remain unresolved while the
   completeness ledger says none are unresolved. The input contract exposes
   config access widths and lifecycle callbacks
   (`regseqmodel-input-contract.md:210`, `regseqmodel-input-contract.md:274`),
   and generic_c accepts config widths 1/2/4/8 at
   `crates/regseq-backends/src/generic_c/body_emit.rs:2399`. The Zephyr spec
   leaves sub-word/64-bit config access, `on_deinit`, `on_surprise_remove`,
   `ClassTag`, `UserDataField`, and multi-region policy as NNS in
   `zephyr-open-questions.md:159`, but then claims `unresolved-constructs: []`
   in `zephyr-open-questions.md:287`.

6. Medium: non-Generic device classes are not actually specified. The model
   carries `device_class` (`crates/regseq-model/src/device_lifecycle.rs:52`),
   Zephyr’s driver model uses an API pointer for class APIs
   (https://docs.zephyrproject.org/3.7.0/kernel/drivers/index.html), and the
   vendored e1000/PL011 drivers wire real API structures at
   `reference-drivers/e1000/zephyr/eth_e1000.c:296` and
   `reference-drivers/pl011/zephyr/uart_pl011.c:743`. The spec admits ClassTag
   is load-bearing in `zephyr-lowering-per-construct.md:958`, but the scaffold
   emits `NULL /* api */` at `zephyr-scaffold-and-dispatch.md:82`.

7. Medium: the spec does not fully define the backend “agnostic core” required
   by the input contract. The contract requires lowering across instances plus
   side tables, ABI/type layout, C ABI, symbol derivation,
   constants/statics/strings, and backend context
   (`regseqmodel-input-contract.md:28`, `regseqmodel-input-contract.md:115`,
   `regseqmodel-input-contract.md:126`). The Zephyr scaffold mostly gestures
   at “faithful body bank” in `zephyr-scaffold-and-dispatch.md:65` instead of
   specifying which generic_c banks are reused, forked, or target-specific.

8. Medium: citation discipline does not meet the spec’s own rule.
   `README.md:128` says every normative claim must cite a repo path:line or
   pinned Zephyr URL, but the docs repeatedly use shorthand filenames or
   section references, and several normative Zephyr links use mutable `latest`
   URLs, e.g. `zephyr-lowering-per-construct.md:847`,
   `zephyr-lowering-per-construct.md:1364`, and
   `zephyr-integration-probe-path.md:134`. That is not acceptable for a
   3.7 LTS / 4.x-pinned backend spec.

## Suggestions (verbatim)

1. Change PCI BAR materialization to use `pcie_get_mbar()` for actual BAR
   numbers, or define and test an explicit BAR-number-to-memory-BAR-ordinal
   conversion before using `pcie_probe_mbar()`.
2. Make MMIO base storage per-device, likely by threading
   `const struct device *dev` or generated driver data into lowered
   register-access helpers; reject multi-instance models until that is
   implemented.
3. Pick one Zephyr probe order per platform path and state the allowed
   deviation from generic_c explicitly, with cited rationale and generated
   rollback/error behavior.
4. Downgrade `on_system_shutdown`, non-Generic `ClassTag`, unsupported config
   widths, teardown callbacks, and multi-region inputs to explicit rejected
   inputs until their lowering contracts are closed.
5. Replace mutable `latest` docs links and section-only citations with pinned
   Zephyr 3.7/4.x URLs and repo `path:line` citations throughout.

## Unresolved or failed constructs (verbatim)

```
unresolved_or_failed_constructs=[]
```

---

## Disposition log (post-review fixes)

Applied in this directory after the review; WebFetch was unavailable in the
fixing session too, so header-level API claims remain flagged fetch-unverified
in [`zephyr-open-questions.md`](./zephyr-open-questions.md) §C.

| # | sev | disposition |
|---|---|---|
| 1 | High | **Fixed.** PCI BAR materialization changed to `pcie_get_mbar(bdf, effective_bar, &mbar)` everywhere generated code is specified (`pcie_probe_mbar` retained only where the *vendored reference's* behavior is described — its literal `0` is a memory-BAR ordinal that coincides with BAR0 on the e1000). Signature pin added to open-questions C-1. |
| 2 | High | **Fixed + routed.** The TU-static base is now explicitly constrained to single-instance TUs, enforced by an emitted `BUILD_ASSERT(DT_NUM_INST_STATUS_OKAY(DT_DRV_COMPAT) == 1, …)` guard; multi-instance models are rejected at C compile time until per-device base threading (the container-morph seam) lands. The TU-static-vs-`DEVICE_MMIO_GET(dev)` ratification stays open-questions A-2, now with the guard as part of the faithful option. |
| 3 | High | **Fixed + routed.** One order pinned per path: platform = canonical (side-resource preambles → `DEVICE_MMIO_MAP` → `on_init`), with PL011's literal map-first layout noted as satisfying only the weaker witnessed constraint (map before first MMIO op); PCIe = probe → enable → map as the single recorded deviation, still pending ratification (A-3). The self-contradictory "first statement" wording removed. |
| 4 | High | **Fixed + routed.** `on_system_shutdown` downgraded from a claimed materialization to "slot mapped, trigger contract unresolved": the spec now emits a generated `pm_device_action` dispatch wrapper (author body on the Q3-chosen action(s), `-ENOTSUP` otherwise per the PM contract) and the disposition tables say NM(slot)/open(trigger). Interim emit-vs-diagnose policy = new open-questions A-18. |
| 5 | High | **Fixed.** Open-questions §D rewritten as a two-level ledger: construct-level `unresolved-constructs: []` retained, plus an explicit **legal-but-unlowered input shapes** table whose interim normative contract is fail-loud rejection at emit time (never silent emission/omission). |
| 6 | Medium | **Fixed + routed.** Non-Generic `ClassTag` is now an explicit emit-time diagnostic (no silent `NULL` api for a declared class); `NULL` api applies to `ClassTag::Generic` only. Ratification stays A-16. |
| 7 | Medium | **Fixed.** New §9 in `zephyr-scaffold-and-dispatch.md`: the generic_c agnostic-core bank map (five-bank assembly, name tables, symbol derivation, C-ABI side table, statics/consts/strings, helper-demand tracking, entry classification, adapter, BackendContext), each marked reused / forked / target-specific with citations. |
| 8 | Medium | **Partially fixed + routed.** All `docs.zephyrproject.org/latest` URLs pinned to `3.7.0`; README's citation rule no longer allows `latest`; a global shorthand-path legend added to README (the matrix already had one). Full expansion of every shorthand basename to a repo-rooted path is routed as a mechanical cleanup item (open-questions A-19) rather than churning ~3000 lines unreviewed. |
