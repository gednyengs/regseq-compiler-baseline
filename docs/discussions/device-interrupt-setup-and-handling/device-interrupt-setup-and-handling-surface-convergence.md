# device-interrupt-setup-and-handling — surface convergence (construct #6)

The compiler-side spec contract per the three-repo convention: the converged
DSL surface + the as-built record. Seeded by the unit's T14 (2026-06-11);
the eval-side design analysis
(`~/regseq-eval/docs/constructs/device-interrupt-setup-and-handling/`)
cross-links here. Decision IDs (D##/C##/R##/I##/O#) refer to the unit's
GRILL/PRD/PLAN chain under
`.claude/sekekama/grill-to-plan/specs/device-interrupt-setup-and-handling/`.

## The converged surface (frozen; ZERO extensions)

- **L1**: a typed `Interrupt` field on the `#[rseq::driver]` struct via
  `#[rseq::interrupt(id = "…")]` (`id` MANDATORY, no field-name fallback —
  D08); a bound handler `#[rseq::isr]` (bare / `field = "…"` /
  array-bind) with the C04 signature `&mut self -> IrqResult`
  (`index: usize` for array-bind; never `Result<_, RegseqError>`).
  `IrqResult` is the `#[rseq::framework_type("irq-result")]`-tagged
  dispatch-result enum (`Handled` / `NotHandled` / `DeferWork`). The
  `Interrupt` shell is METHODLESS (I02): line control is compiler-managed,
  never an author call (`IRQ-ERR-DRIVER-CALLABLE-LINE-CONTROL`).
- **L2**: `[interrupt.<id>]` — `irq_number` (generic-c required; zephyr
  advisory; linux unused — R10), `trigger_type`/`trigger_polarity`
  (canonical spellings only, validation-first, NEVER flag emission —
  D10/R12), `priority` (zephyr `IRQ_CONNECT` arg only), `shared` (the
  physical board fact, default false — D05/I08).
- **Placement (C01, the one contract all backends consume from
  `regseq-backends::interrupt_order` — R17/D14)**: register DISABLED in the
  lifecycle init preamble, LAST among resource preambles (after bus ops +
  `map_region`); enable ONLY after `on_init` Ok (D02 — closes the
  early-firing hazard); `on_init`-Err unwind includes deregister (no
  disable: never enabled); deinit = `on_deinit` → disable → deregister-slot
  before the resource postambles (D09).

## As-built (all five milestones, 2026-06-11; compiler branch `construct/device-interrupt-setup-and-handling`)

- Model: `AttrData::{InterruptResource=2, IsrBinding=3}`,
  `FrameworkType::IrqResult=1`, the `InterruptTables` sibling (I06
  append-only; pre-construct JSON pinned). Arm reachability is tri-state
  with a PROOF-grade `ProvenReachable` (bounded may-set exactly the arm +
  CFG termination proof; Codex M1 hardening) — the R11 warning over-warns,
  never under-warns; R09's `DeferWork` guard fail-louds on `Unknown`.
- Frontend: the full C06 family (`IRQ-ERR-*`; O3 minted:
  `-MISSING-ID`, `-MISSING-HANDLER`, `-UNKNOWN-FIELD`, `-ISR-SIGNATURE`)
  + the R11 warning `IRQ-WARN-SHARED-NOTHANDLED`
  (`regseq_l2::interrupt::shared_discipline_warnings` →
  `tcx.dcx().warn`). Option/array forms recognized whole, emit-rejected
  (D06/R02/I05).
- generic_c: the C05 five-hook host line model; synchronous
  `regseq_irq_fire` (D07 — see Amendments); O1 minted as
  `IRQ fire line=<n> inert`; the `irq-pl011` golden IS the C01 grammar
  observable (matrix rows IRQ-1..13; `fixtures/_irq/`).
- linux_c: `request_irq(adev->irq[0], rseq_isr_<drv>, IRQF_NO_AUTOEN,
  "<name>", priv)` / gated `enable_irq` / `disable_irq`→`free_irq` manual
  pair (no devm); irqreturn_t wrapper, `NotHandled -> IRQ_NONE` never
  suppressed (R08); Err-path `free_irq` unwind (R16). PCI leg deferred to
  an e1000 irq fixture (UnsupportedInUnit).
- zephyr_c: `IRQ_CONNECT(DT_INST_IRQN(0), <L2 priority | DT cell>,
  rseq_isr_<drv>, DEVICE_DT_INST_GET(0), 0)` WITHOUT the stock immediate
  enable; enable split post-`on_init`-Ok; void wrapper discards the result
  (R08 zephyr leg); deinit `irq_disable` + the `RSEQ-ABSENT-OP` audit block
  at the deregister slot.
- Capability schema: `Disposition::StructurallyAbsent` (O5 minted),
  `CapabilitySlot::Operation` widening,
  `CAP-ZEPHYR-STATICIRQ-DEREGISTER-1` (citation re-verified live on
  v4.4.0 684c9e8f32e4), `audit_block()` per-disposition; conformance gains
  `structurally_absent_slots`; the stale-pin audit covers the new kind.
- Tier-2 handoff (eval repo): whitelist sections in
  `pl011/{linux,zephyr}/tools/trace-diff/whitelist.toml`; the binding
  single-vCPU pin + D11/R18 limitation in
  `pl011/TIER2-PIN-device-interrupt.md`.

## Amendments (binding, discovered during implementation)

1. **D07 supersedes the buildout-PLAN "host thread" note**: the generic-c
   fire is SYNCHRONOUS (`regseq_irq_fire` invokes the wrapper inline) for
   deterministic golden traces. (Pre-registered in the design analysis;
   restated here as the PLAN-§1 amendment record.)
2. **Shared-line registration cannot compose with registered-disabled on
   Linux** (M3, Codex review; compiler commit `bdf11a8`): the kernel
   rejects `IRQF_NO_AUTOEN | IRQF_SHARED` with `-EINVAL`
   (`kernel/irq/manage.c` `__setup_irq`), so the PLAN's flag formula
   (`IRQF_NO_AUTOEN + {IRQF_SHARED iff shared}`) is unlowerable as
   written. As-built: `shared = true` is EMIT-REJECTED on linux_c (and on
   zephyr_c, where the static connect cannot express sharing either).
   **The shared-line registration form needs a design amendment** —
   registered-enabled with device-level masking is the stock idiom — owned
   by the unit that lands a shared-irq fixture (e1000).
3. **T13 residual**: the full-spec model-JSON suite the PLAN expected does
   not exist (the spec-convergence unit converged `pl011-composed`; the
   full-spec `expected/*.json` are orphaned legacy). The fixture's
   interrupt surface is converged (canonical spellings; no-Debug rule);
   full-spec end-to-end convergence is blocked on the clock/reset/pinctrl
   constructs (pinned by
   `codegen-coverage-runner/tests/pl011_full_spec_irq_convergence.rs`).

## Taxonomy feedback notes (recorded, NEVER applied to the frozen pages)

1. **D08 — `id` always required**: the taxonomy's join-key surface allows a
   field-name fallback (as `#[rseq::mmio]` does); this construct REQUIRES
   `id` (an intended tightening — the interrupt line is a board-level
   identity, not a struct-layout accident). Feed back to the
   reconciliation when the taxonomy thaws.
2. **`IRQF_NO_AUTOEN` default (D02 generalizing G-INT-04)**: the taxonomy's
   register-disabled guidance is Linux-idiom-specific; the ratified
   generalization (register disabled on EVERY target; enable only after
   `on_init` Ok) is stronger than the page states — with amendment 2's
   shared-line caveat attached.

## Phase limitations (documented, obligations registered)

- ISR↔mainline race: accepted under the single-vCPU Tier-2 pin until the
  Synchronization construct lands (D11/R18) — wrappers acquire no locks.
- `DeferWork`: emit-rejected naming `deferred-work-item-scheduling` (R09).
- Option/array forms: recognized, lowering deferred (D06).
- Linux PCI irq leg (`pdev->irq`): deferred to an e1000 irq fixture.
- Shared lines: emit-rejected pending the amendment-2 design decision.
