# OS-absent-slot rule — Codex adversarial stress-test (2026-06-10)

Status: input to the `os-capability-disposition` unit's grill. The rule draft below
was proposed in-session (Gedeon + Claude), then stress-tested by Codex (GPT-5.5,
effort=high, grounded in this repo at `25f1553`). Codex verdict: **adopt with
amendments, not as drafted**. The nine amendments are the grill's ratification
agenda.

## Problem

The composed E1000 spec authors all four lifecycle callbacks, including
`on_surprise_remove` (`crates/codegen-coverage-runner/fixtures/_composed/e1000-composed/input.rs:510`).
linux_c lowers it (PCI present-predicate dispatch in `.remove`). zephyr_c REJECTS it
(`crates/regseq-backends/src/zephyr_c/mod.rs:232`) per the ratified D.2 discipline
(`docs/discussions/zephyr-backend/zephyr-open-questions.md` §D.2): every
legal-but-unlowered shape fails loud, never silent. Unlike Zephyr's deinit gap
(resolved by v4.4.0 `device_deinit()`), there is NO upcoming API: Zephyr's static DT
device model structurally cannot deliver a surprise-removal event. Consequence: a
single-file E1000 spec consumable by both backends is impossible while the rejection
stands.

## The rule draft that was stress-tested

A backend MAY auditably non-emit an authored lifecycle callback when the callback's
dispatch condition PROVABLY CANNOT OCCUR on the target OS. Draft guardrails:
citation-mandatory impossibility; auditable in artifact + test log; applies only to
dispatch-impossibility (never "backend doesn't support yet"); body still
type-checked/bridged; oracle scenario markers whitelisted under the same citation.

Thesis framing under test (Gedeon): "One spec lowers to multiple OS drivers; the
backend for each OS decides the proper lowering. For some OSes, certain
functionality is not present. That is not a limitation of Regseq — it is the
strength. Write once, generate everywhere."

## Codex verdict (verbatim)

**Verdict: adopt with amendments, not as drafted.**

As drafted, the rule is too easy to abuse because "provably cannot occur" is a
free-form claim. But the idea is defensible if it becomes a closed, audited
OS-capability disposition, not an ad hoc callback escape hatch.

Grounding facts: D.2 currently requires legal-but-unlowered Zephyr input shapes to
"fail-loud rejection at emit time" and never silent omission, including authored
`on_surprise_remove` (zephyr-open-questions.md:414, :426). The Zephyr emitter
enforces that today (zephyr_c/mod.rs:232). Linux does have a concrete
present-predicate dispatch to `on_surprise_remove`
(e1000-composed/golden.linux.c:370). The authored E1000 body is log-only today, but
that is fixture-specific, not a DSL guarantee (e1000-composed/input.rs:510).

### Soundness attack

"Dispatch condition cannot occur" is stable only when scoped to a pinned OS version,
bus family, and driver registration form. It is not globally stable. Zephyr static
DT devices lacking hot-remove is a plausible absent slot; "backend does not know
Zephyr hotplug yet" is not. A future Zephyr PCIe hotplug subsystem would invalidate
the old claim for `TargetOs::Zephyr + PCIe`, even if it remains true for
`TargetOs::Zephyr + static DT`.

Adversarial cases:

- `on_system_shutdown`: Zephyr already has a PM-device slot but unresolved trigger
  semantics; the repo treats it as designed/scheduled, not unsupported
  (zephyr_c/mod.rs:240; backend-lowering/_RATIFICATION-SHEET.md:53). That must not
  become "absent."
- suspend/resume: these belong to a separate PM construct with explicit ordering,
  not driver lifecycle (docs/compiler-collateral/constructs/driver-lifecycle.md:11;
  device-power-state-management.md:101). Absence must be decided there, not by
  lifecycle callback analogy.
- `on_init_failure`: generic only emits it when an authored `on_init` can return
  `Err` (generic_c/body_emit.rs:5411). If an OS probe "cannot fail," that is almost
  certainly false once native allocation, mapping, and registration failures are
  included.
- Body support cannot be skipped. The current Zephyr negative test puts a
  `poll_until` inside `on_surprise_remove` and only proves the slot rejection fires
  first (zephyr_c/mod_tests.rs:520). Under OS-absent omission, that same body must
  still be lowered/type-checked enough to reject recognized-not-lowered constructs
  like `poll_until`.

### Slippery slope

Yes, this opens a D.2 escape hatch unless the rule is table-driven. The existing
method already has the right vocabulary: `not-applicable` means emit nothing but log
it; `needs-new-surface` blocks and escalates
(backend-lowering/model-analysis-strategy.md:46, :53). Use that, but make it
mechanical.

Required guardrails:

1. Closed `AbsentSlotClass` enum, not free-form strings.
2. Per-OS capability table keyed by `(target_os, os_version_pin, bus_family,
   registration_form, slot)`.
3. Each row has disposition: `Delivered`, `AbsentDispatch`, `NeedsNewSurface`, or
   `RejectedUnsupported`.
4. Only `AbsentDispatch` may non-emit an authored callback.
5. A backend may not invent `AbsentDispatch`; it must reference a ratified row.
6. Tests fail if emitted artifacts omit a callback without a named absent-slot
   audit block.
7. Test log must list delivered vs absent slots.
8. Coverage/ledger must treat an absent slot as covered only when the matrix row is
   explicitly marked covered/deferred, preserving the existing drift gates
   (codegen-coverage-runner/src/ledger.rs:230).
9. Citation must be pinned to an OS version or local checkout (precedent:
   zephyr-open-questions.md:369 v4.4.0 verification notes).

### Semantics

Do not require the `on_surprise_remove` body to be pure. That is the wrong proof
obligation. If the event truly cannot be dispatched, the body is unreachable on that
target. But the author must be told the behavioral envelope differs.

The body may persist data, notify another subsystem, or update crash-consistency
state on Linux. On Zephyr static DT, that behavior is absent, not "safely optimized
away." So the compiler must emit a conformance report saying: authored slot present,
target dispatch absent, body checked but not delivered.

Longer-term, the DSL should probably expose callback criticality:

    #[rseq::callback_requirement(required)]
    fn on_surprise_remove(...)

Default can be `best_effort` only if thesis policy wants broad portability. If
marked `required`, Zephyr static DT must reject even with an `AbsentDispatch` row.

### Thesis framing

"Write once, generate everywhere" survives only if rewritten as:

> One Regseq spec can be lowered by multiple OS backends, with each backend
> producing a compile-time conformance report that enumerates which authored
> lifecycle slots are delivered, absent by OS dispatch model, or rejected.
> Differences are explicit, cited, and machine-checked.

Do not say "silently different behavioral envelopes." The mechanism must make them
non-silent. The PL011 fixture identity mechanism is the right precedent: licensed
regions require non-empty ratified citations and exact checked names
(codegen-coverage-runner/src/identity.rs:14; tests/composed_identity.rs:52).

### Ratifiable amendments

Adopt only with these amendments:

1. **Closed Capability Table.** D.2 gains a companion per-OS capability table.
   `OS-absent slot` is legal only for rows explicitly ratified as `AbsentDispatch`.
2. **No Free-Form Proofs.** Emitters may cite only table row IDs, not prose claims.
3. **Version-Bound Citations.** Each `AbsentDispatch` row cites a pinned OS
   version/source/doc and names the bus/registration form.
4. **Body Still Checked.** The callback body remains part of `RegseqModel`; all
   body-lowering legality checks still run. Unsupported constructs inside the body
   remain D.2 rejections.
5. **Required Callback Override.** If the DSL marks a callback `required`,
   `AbsentDispatch` becomes an emit-time rejection.
6. **Conformance Artifact.** Every build emits a per-target report:
   `delivered_slots`, `absent_dispatch_slots`, `rejected_slots`, citation IDs.
7. **Generated C Audit Block.** Non-emission must leave a named generated comment
   block with slot, reason, and ratified row ID.
8. **Trace Whitelist Coupling.** Any oracle marker omission uses the same row
   ID/citation as the artifact and test log.
9. **Evolution Audit.** On OS-version re-pin, every `AbsentDispatch` row is
   re-reviewed; stale rows fail the capability-table audit.

For timing: ratify this in its own unit, not buried inside `spec-convergence-pl011`.
The PL011 unit can depend on it, but this rule changes D.2's safety boundary and
deserves a focused review.

## Downstream consequences (recorded in-session)

- `backend/zephyr-c-e1000` becomes **born-converged**: one `e1000-composed` file for
  all three backends, consuming this unit's `AbsentDispatch` row +
  `spec-convergence-pl011`'s mixed-match/deinit work. `spec-convergence-e1000`
  dissolves into its acceptance criteria.
- Unit order ratified in-session (Gedeon): `os-capability-disposition` →
  `spec-convergence-pl011` → `backend/zephyr-c-e1000`, serial (this unit and the
  convergence unit both edit `zephyr_c/mod.rs`).
