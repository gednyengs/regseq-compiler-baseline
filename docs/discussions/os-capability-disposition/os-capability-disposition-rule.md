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

**Forbidden phrasing:** "silently different behavioral envelopes". The whole
point of the mechanism is that the envelopes differ *loudly*.

## The nine amendments (authoritative text, as ratified 2026-06-10)

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
   table row IDs (grammar `CAP-<OS>-<FORM>-<SLOT>-<N>`), not prose claims.
   Structurally enforced: the emitter can only push ratified `CapabilityRow`
   consts.

3. **Version-Bound Citations** *(as amended — citation bar)*. Each
   `AbsentDispatch` row cites the pinned OS version incl. commit AND ≥1
   source anchor in the pinned local checkout or vendored reference; doc URLs
   are optional supplements; the bus/registration form is named by the row
   key. Empty citations are errors.

4. **Body Still Checked** *(verbatim + mechanism)*. The callback body remains
   part of `RegseqModel`; under `AbsentDispatch` the body runs the complete
   body-lowering pipeline and the output is discarded — all body-lowering
   legality checks run by construction; unsupported constructs inside the
   body remain D.2 rejections. The discarded lowering has no observable
   effect on the TU beyond the audit block.

5. **Required Callback Override** *(policy ratified; delivery deferred)*. If
   the DSL marks a callback `required`, `AbsentDispatch` becomes an emit-time
   rejection; default is `best_effort`. DSL delivery (model+macro+bridge) is
   the named successor unit `callback-requirement-marker`. Until it ships,
   every authored callback is implicitly `best_effort`.

6. **Conformance Artifact** *(verbatim + schema)*. Every OS-backend build
   emits a per-target report (the `ConformanceReport` struct on
   `ZephyrEmit`/`LinuxEmit`, serialized to `conformance.<os>.json`):
   delivered/absent_dispatch/rejected slots with citation IDs. generic_c
   (the oracle) is exempt: conformance is measured against it; a self-report
   is vacuous.

7. **Generated C Audit Block** *(verbatim + grammar)*. Non-emission leaves a
   named generated comment block with slot, reason, and ratified row ID:

   ```c
   /* RSEQ-ABSENT-SLOT: <slot_name>
    * disposition: AbsentDispatch row: <ROW-ID>
    * [ratified: <row citation, verbatim from the CapabilityRow const>]
    * body checked, not delivered: dispatch cannot occur on this target. */
   ```

   Emitted at the position the callback's lowered code would have occupied;
   exactly one block per absent slot, zero when no slot is absent; the
   `RSEQ-ABSENT-SLOT:` prefix is stable and greppable.

8. **Trace Whitelist Coupling** *(verbatim; entries deferred)*. Any oracle
   marker omission uses the same row ID/citation as the artifact and test
   log. **Binding on `backend/zephyr-c-e1000`**, which lands the entries: a
   trace-whitelist entry licensing an absent slot's missing oracle markers
   MUST cite the CAP row ID + row citation — the same ID as the C audit
   block, conformance report, and test log (the A-20 per-entry ratified
   citations precedent). No eval-side edits were made by this unit.

9. **Evolution Audit** *(verbatim + mechanics)*. On OS-version re-pin, every
   `AbsentDispatch` row is re-reviewed; stale rows fail the capability-table
   audit. Mechanics: a single `ZEPHYR_PIN` const is the one place a re-pin
   edits; rows spell the pin their claim was ratified against as a
   **literal** (never a reference to the const — that would make the audit
   vacuous); the staleness test compares every Zephyr
   `AbsentDispatch`/`DesignedScheduled` row's pin and fails naming each
   stale row; re-review = re-verify on the new pin + update the row's
   pin/citation (+ the doc mirror via the sync test).

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
| `spec-convergence-pl011` | Delivers `on_deinit` on Zephyr (row CAP-ZEPHYR-DT-DEINIT-1, `DesignedScheduled` → delivered; the row is re-ratified to `Delivered` with `N+1` if substance changes). |

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
