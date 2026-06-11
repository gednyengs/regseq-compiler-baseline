# The OS-capability-disposition rule — as ratified 2026-06-10

This is the normative record of the `os-capability-disposition` unit: the
amendment of the D.2 fail-loud discipline
(`docs/discussions/zephyr-backend/zephyr-open-questions.md` §D.2) with a
table-driven OS-absent-slot disposition. It carries the nine amendments
verbatim as ratified, the thesis wording, the generic_c exemption rationale,
and the successor registry.

Companion documents:
- Founding stress-test (problem statement + Codex adversarial findings):
  `codex-stress-test-2026-06-10.md` (committed `df7b000`).
- The ratified capability table itself: `capability-table.md` (the human
  mirror of `crates/regseq-backends/src/capability.rs`, sync-tested).
- Decision record: GRILL `os-capability-disposition`
  (`.claude/sekekama/grill-to-plan/specs/os-capability-disposition/`).

## The rule, in one paragraph

An authored lifecycle callback whose dispatch condition **structurally
cannot occur** on a target OS may be *auditably non-emitted* instead of
rejected — but only under a ratified `AbsentDispatch` row of the per-OS
capability table. Non-emission is never silent: the generated C carries a
named `RSEQ-ABSENT-SLOT` audit block in the callback's place, the per-target
conformance report carries the row (with its citation), and the test log
carries the `RSEQ-CONFORMANCE` line — all rendered from the same ratified
row const, so cross-artifact row-ID consistency holds by construction. The
callback body remains fully checked: it runs the complete body-lowering
pipeline in a throwaway context whose output is discarded. Everything not in
a ratified `AbsentDispatch` row keeps the D.2 fail-loud rejection; this rule
narrows the rejection space by exactly one mechanism.

## Thesis (ratified verbatim, C07/D15)

> One Regseq spec can be lowered by multiple OS backends, with each backend
> producing a compile-time conformance report that enumerates which authored
> lifecycle slots are delivered, absent by OS dispatch model, or rejected.
> Differences are explicit, cited, and machine-checked — never silent.

"Silently different behavioral envelopes" is forbidden phrasing.

> **Recorded note (2026-06-10, spec-convergence-pl011 GRILL D01/D08 + A6).**
> The conformance REPORT artifact gained a non-slot section:
> `ignored_match_entries` enumerates recognized foreign-transport match
> entries a backend did not consume under the D03(a) mixed-match rule, each
> carrying the fixed registration-form reason, the GRILL-decision license
> const, and the consumed-natively backend list. This is an ADDITIONAL
> enumeration consistent with the thesis's "differences are explicit, cited,
> and machine-checked — never silent"; the slot-conformance claim above and
> the capability table's lifecycle-slot domain are unchanged (the audit cites
> the GRILL decision, never a capability row).

## The nine amendments (authoritative text, as ratified 2026-06-10)

The texts below are the GRILL C06 record verbatim (`C02`/`C04`/`C05` are
that record's contract labels; their concrete shapes are reproduced in the
as-built mechanics notes that follow). Implementation glosses live ONLY in
the notes section — never inside the ratified texts.

1. **Closed Capability Table** *(as amended — 5-variant enum)*. D.2 gains a
   companion per-OS capability table. "OS-absent slot" is legal only for rows
   explicitly ratified as `AbsentDispatch`. Dispositions form the closed enum
   `{Delivered, AbsentDispatch, DesignedScheduled, NeedsNewSurface,
   RejectedUnsupported}`; rows are keyed by `(target_os, os_version_pin,
   bus_family, registration_form, slot)`. Only `AbsentDispatch` may non-emit
   an authored callback; `DesignedScheduled` records witnessed/designed slots
   whose emission is scheduled behind a named pin or successor and remains a
   reject-at-emit scheduling diagnostic.

2. **No Free-Form Proofs** *(verbatim + grammar)*. Emitters may cite only
   table row IDs (grammar C02), not prose claims. Structurally enforced: the
   emitter can only push ratified `CapabilityRow` consts.

3. **Version-Bound Citations** *(as amended — citation bar)*. Each
   `AbsentDispatch` row cites the pinned OS version incl. commit AND ≥1 source
   anchor in the pinned local checkout or vendored reference; doc URLs are
   optional supplements; the bus/registration form is named by the row key.
   Empty citations are errors.

4. **Body Still Checked** *(verbatim + mechanism)*. The callback body remains
   part of `RegseqModel`; under `AbsentDispatch` the body runs the complete
   body-lowering pipeline and the output is discarded — all body-lowering
   legality checks run by construction; unsupported constructs inside the
   body remain D.2 rejections. The discarded lowering has no observable
   effect on the TU beyond the audit block.

5. **Required Callback Override** *(policy ratified; delivery deferred)*. If
   the DSL marks a callback `required`, `AbsentDispatch` becomes an emit-time
   rejection; default is `best_effort`. DSL delivery (model+macro+bridge) is
   the named successor unit `callback-requirement-marker`.

6. **Conformance Artifact** *(verbatim + schema)*. Every OS-backend build
   emits a per-target report (C05): delivered/absent_dispatch/rejected slots
   with citation IDs. generic_c (the oracle) is exempt: conformance is
   measured against it; a self-report is vacuous.

7. **Generated C Audit Block** *(verbatim + grammar)*. Non-emission leaves a
   named generated comment block (C04) with slot, reason, and ratified row ID.

8. **Trace Whitelist Coupling** *(verbatim; entries deferred)*. Any oracle
   marker omission uses the same row ID/citation as the artifact and test
   log. Binding on `backend/zephyr-c-e1000`, which lands the entries.

9. **Evolution Audit** *(verbatim + mechanics)*. On OS-version re-pin, every
   `AbsentDispatch` row is re-reviewed; stale rows fail the capability-table
   audit. Mechanics: single `ZEPHYR_PIN` const; staleness test compares every
   Zephyr `AbsentDispatch`/`DesignedScheduled` row's pin and fails naming
   each stale row; re-review = re-verify on the new pin + update row
   pin/citation (+ doc mirror via the sync test).

## As-built mechanics notes (non-normative; the ratified texts above govern)

- **C02 row-ID grammar** (amendment 2's "grammar C02"):
  `CAP-<OS>-<FORM>-<SLOT>-<N>` — `<OS>` the upper-case target OS, `<FORM>`
  the registration-form shorthand (`DT`, `PCI`, …), `<SLOT>` ∈ `{SURPRISE,
  SHUTDOWN, DEINIT, INIT, INITFAIL}`, `<N>` a positive integer; IDs are
  append-only and never renamed; a re-ratification that changes substance
  mints `N+1`.
- **C04 audit-block grammar** (amendment 7's "(C04)"):

  ```c
  /* RSEQ-ABSENT-SLOT: <slot_name>
   * disposition: AbsentDispatch row: <ROW-ID>
   * [ratified: <row citation, verbatim from the CapabilityRow const>]
   * body checked, not delivered: dispatch cannot occur on this target. */
  ```

  Emitted at the position the callback's lowered code would have occupied;
  exactly one block per absent slot, zero when no slot is absent; the
  `RSEQ-ABSENT-SLOT:` prefix is stable and greppable; generated from the
  same `CapabilityRow` const as the report row.
- **C05 conformance shape** (amendment 6's "(C05)"): the
  `ConformanceReport` struct on `ZephyrEmit`/`LinuxEmit`, serialized by the
  coverage runner to a deterministic `conformance.<os>.json` beside the
  emitted C; the `RSEQ-CONFORMANCE` test-log line derives from the same
  struct.
- **Amendment 5, until the successor ships:** every authored callback is
  implicitly `best_effort` (accepted at ratification).
- **Amendment 8, the binding rule spelled out:** a trace-whitelist entry
  licensing an absent slot's missing oracle markers MUST cite the CAP row
  ID + row citation — the same ID as the C audit block, conformance report,
  and test log (the A-20 per-entry ratified citations precedent). No
  eval-side edits were made by this unit.
- **Amendment 9, as built:** rows spell the pin their claim was ratified
  against as a **literal** (never a reference to the `ZEPHYR_PIN` const —
  that would make every row track a re-pin automatically and the staleness
  audit could never fire; Codex M2 review finding). `ZEPHYR_PIN` remains
  the single authoritative *current* pin the audit compares against.

## The generic_c exemption (D09, rationale recorded)

The conformance report is an OS-backend artifact. `generic_c` is the
reference semantics conformance is measured AGAINST — it delivers every
authored slot via scenario markers, so a self-report is definitionally
vacuous: it could never say anything but "all delivered", and producing it
would either touch frozen paths (F-1) or dress a hardcoded claim up as a
measurement. generic_c therefore emits no report, and the runner synthesizes
none for it.

## What this unit deliberately did NOT change

- **DSL surface** (regseq-model / regseq-macros / bridge): untouched. The
  boundary rule is hard: backend concerns never enter the DSL surface. The
  backend defines its own slot-name enum (`capability::LifecycleSlot`).
- **Eval side**: no trace-whitelist entries, no harness edits (amendment 8
  is recorded as a rule only).
- **Non-slot D.2 rejections** (`BarType::Io`, `ClassTag`, config widths,
  user-data shapes, multi-region, multi-compatible): remain prose-ledger
  entries. The capability table's domain is lifecycle slots only.
- **suspend/resume**: a separate PM construct with explicit ordering;
  absence is decided there, never by lifecycle-callback analogy.
- **generic_c**: frozen (F-1), exempt (above).
- **`zephyr_build.rs` west gate**: the optional live-workspace version
  assert was offered at ratification and not taken.

## Successor registry (GRILL Open item 4)

| successor | scope inherited from this unit |
|---|---|
| `callback-requirement-marker` | Amendment-5 delivery: the DSL `required`/`best_effort` callback marker (model field + macro + bridge). Until it ships, every authored callback is implicitly `best_effort`. |
| `backend/zephyr-c-e1000` | Amendment-8 delivery: the eval-side L3 trace-whitelist entries licensing the absent slot's missing oracle markers, each citing CAP-ZEPHYR-DT-SURPRISE-1 + its citation. The unit consumes that row for the one-spec-three-backends E1000 emit (the zephyr PCIe scaffold lifts the `UnsupportedInUnit` gate there). |
| `spec-convergence-pl011` | **DELIVERED (2026-06-10).** `on_deinit` on Zephyr shipped (row CAP-ZEPHYR-DT-DEINIT-1 flipped `DesignedScheduled` → `Delivered` IN PLACE, no `N+1` — the unit delivered exactly what the row scheduled, as pre-registered; citation updated to the live v4.4.0 anchors). |

## Delivered mechanism — quick index

- Table + report types: `crates/regseq-backends/src/capability.rs`
  (sync-tested mirror: `capability-table.md`).
- zephyr_c transition: `crates/regseq-backends/src/zephyr_c/mod.rs`
  (AbsentDispatch lookup → throwaway-context full lowering → audit block +
  report row; scheduling rejections table-cited).
- linux_c population: `crates/regseq-backends/src/linux_c/mod.rs` (all
  authored slots delivered; PCI surprise cites CAP-LINUX-PCI-SURPRISE-1).
- Runner serialization: `crates/codegen-coverage-runner/src/conformance.rs`
  + the unconditional `conformance.<os>.json` snapshot in both OS stage
  functions; the `RSEQ-CONFORMANCE` summary line prints from the same
  struct.
- End-to-end witness: `fixtures/_composed/pl011-absentslot-zephyr` +
  `tests/zephyr_absentslot_e2e.rs` (sole binary); matrix row `OS-1`
  (`deferred`; real gate = `tests/os_capability_covers.rs`).
