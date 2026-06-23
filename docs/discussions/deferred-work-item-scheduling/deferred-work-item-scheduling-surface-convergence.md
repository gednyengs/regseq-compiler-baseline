# deferred-work-item-scheduling — surface convergence

The compiler-side spec contract per the three-repo convention: the converged DSL
surface + the as-built record. Authored at the unit's M3 (2026-06-22; compiler branch
`construct/deferred-work-item-scheduling`, off compiler `main` @ `7cf6bba`); the
eval-side design analysis
(`~/regseq-eval/docs/constructs/deferred-work-item-scheduling/`) cross-links here.
Decision IDs (D##/C##/R##/I##/S##/G##/SF#) refer to the unit's GRILL/PRD/PLAN chain
under
`~/regseq-eval/.claude/sekekama/grill-to-plan/specs/deferred-work-item-scheduling/`
(`GRILL-2026-06-22-124836` + `PRD-2026-06-22-130922` + `PLAN-2026-06-22-145441`).

**Scope:** a driver declares a typed opaque `Work` field, binds a handler that runs
**later in sleepable thread context**, and schedules it with `self.work.submit()`,
with compiler-managed `init_work`/`cancel_work`. This is the OS-agnostic
"deferred/bottom-half work item" capability (Linux `work_struct`/`INIT_WORK`/
`schedule_work`, Zephyr `k_work`); the E1000 `reset_task` is the witness shape.
**Throughline:** the construct mirrors the landed `peripheral-clock-management` +
`device-interrupt-setup-and-handling` constructs (recognition → a `model.deferred_work`
sibling table → lifecycle preamble/postamble slotting → generic-c lowering → coverage
ledger), with **one major divergence and one genuinely-new path**: (1) **0 L1 / 0 L2
properties** (D02) ⇒ **no L2 phase at all** (no `[deferred_work.<id>]` section, no
binder, no `platform.toml`); (2) **type-driven recognition** — the `Work` field is
recognized by reading `#[rseq::abstraction("deferred_work")]` on the field's resolved
**ADT item DefId**, NOT by a field attribute and NOT by the type name (D04), which no
prior typed-resource construct (`Mmio`/`Clock`/`Interrupt`/`PinCtrl`, all name-test +
field-attr) does.

## The converged surface (frozen; ZERO extensions over the built scope)

- **L1**: a typed opaque `Work` field on the `#[rseq::driver]` struct — **no field
  attribute** (recognized BY TYPE, D04). The facade `Work` is an opaque ZST tagged
  `#[rseq::abstraction("deferred_work")]`; it carries ONE author op — `submit(&self)`
  — tagged `#[rseq::intrinsic("deferred_work_submit")]` (infallible at surface,
  returns `()`, I07). The handler is bound by a **bare** `#[rseq::work_handler]
  fn(&mut self) -> ()` inherent method (the bare word form ONLY — `field=`/name-value/
  empty-`()` forms are rejected, SF-1); the bare handler binds the **sole** `Work`
  (G6/D10/D12). `submit` is a **direct field-path** call `self.work.submit()` only —
  aliases/projections/UFCS spellings are rejected (D11/B-3). `init_work`/`cancel_work`
  are **compiler-managed**: synthesized from the `Work` *field* (the
  `model.deferred_work` resource), never an author call and never an `IntrinsicKind`
  (R04/I01). The only built form is the **required single** `Work`.
- **L2**: **NONE.** The construct has 0 L1 / 0 L2 properties (D02): no
  `[deferred_work.<id>]` platform section, no L2 binder, no `platform.toml` join. The
  `Work` resource is a capability layered on the device (a sleepable-context callback),
  not part of the `DriverHandle`'s address footprint, so it gets its own top-level
  table (D03) but contributes nothing to the platform binding.
- **Placement (C07/I04/I05 — the lifecycle ordering contract)**: `init_work` is
  synthesized in the init preamble **after** `struct {target} inst = {0};` and after
  the bus/clock/pinctrl/interrupt preambles, but **BEFORE** the author `on_init`
  callback (so `WRK init` is traced before any `submit` the author issues inside
  `on_init` — I05 holds structurally for the compiler-owned lifecycle entry, NOT as a
  whole-program theorem). `cancel_work` runs at teardown **after** the author
  `on_deinit` and **before** the resource postambles (drain-before-free, I04). Both are
  **resource-gated** (emitted for every required bound `Work`, regardless of any submit
  call-site — D07/D13). The compiler never synthesizes `submit` (R03).
- **Host work model (C05 — generic-c oracle)**: `Work` is an opaque ZST with no
  in-struct storage, so the `&f` in every `regseq_work_*(&f)` call is the
  **compiler-owned slot** `&__regseq_work_slots[idx]`, where `idx` is the deterministic
  per-resource **declaration-order** index, **identical across init/submit/cancel** for
  a given resource (T07). The synthesized per-handler wrapper is **self-recovering and
  lock-free**: `static void rseq_work_<label>_<i>(void *__ctx)` recovers the driver
  from the registered ctx (`&__t->inner`, the interrupt-wrapper shape) and runs the
  authored handler — **no lock** (C05 item 3). The harness-only `regseq_work_drain()`
  models the later sleepable-context run; a cancelled `Work` cannot re-arm (cancel
  zeroes the registration; a post-cancel submit+drain dispatches nothing — C05 item 4).

## As-built (M1–M3, 2026-06-22; compiler branch `construct/deferred-work-item-scheduling`)

- **Model (M1, `regseq-model`).** `AttrData::WorkHandler { field: Option<StringId> } = 9`
  (next-free after `PinStateMarker = 8`; only the bare `field: None` form is built —
  `Some` is design-in), `IntrinsicKind::DeferredWork(DeferredWorkOp) = 9` (next-free
  after `Pinctrl = 8`), `DeferredWorkOp { Submit = 0 }` (`SubmitUnique` reserved, NOT
  added — G4). `IntrinsicKind::from_name` maps exactly `"deferred_work_submit" =>
  DeferredWork(Submit)`. The `DeferredWorkTables` sibling table (`model.deferred_work`,
  the `ClockTables`/`InterruptTables` mirror, keyed by driver `ItemId`, D03):
  `DriverDeferredWork { resources: Vec<WorkResource> }`,
  `WorkResource { name, field, required, offset, handler: Option<InstanceRef>,
  submit_sites: Vec<SubmitSite> }`, `SubmitSite { variant: SubmitVariant }`,
  `SubmitVariant { Submit = 0 }`. Append-only, `repr(u8)`, pre-construct JSON pinned
  (`#[serde(default)]` on the model field + `submit_sites`); discriminant-stability +
  no-collision sweep in `domain_attr.rs`, table round-trips + insert-order determinism
  in `deferred_work.rs` (R09/S02/S03/S11).
- **Config (M1).** **None** — 0 L2 (D02). No `PlatformBindings` field, no parse branch,
  no binder. (This is the construct's defining divergence from clock/pinctrl.)
- **Facade (M1).** `regseq/deferred_work.rs` fixture-side: `#[rseq::abstraction("deferred_work")]
  pub struct Work { _private: () }` + `impl Work { #[rseq::intrinsic("deferred_work_submit")]
  pub fn submit(&self) {…} }`. `#[rseq::work_handler]` is a registered tool attribute
  (`#![register_tool(rseq)]`), no macro-crate code. The intrinsic token is byte-identical
  to the model `from_name` token (C01/D16).
- **Frontend / recognition (M1–M2, `regseq-driver` `bridge/deferred_work/`).**
  - **Type-driven field recognition (`recognition.rs`, THE NEW PATH).** Resolves a
    driver-struct field's type to its ADT item DefId and reads
    `#[rseq::abstraction("deferred_work")]` on that item (`recognise_abstraction_attr`,
    mirroring `framework_type::recognise_framework_type_attr`'s `get_attrs_by_path` +
    `meta_item_list` — NOT the name-based `is_clock_ty`/`is_interrupt_ty` test). A
    `type WorkAlias = Work;` alias (same DefId) recognizes; an unmarked local `struct
    Work` (same name, different DefId) does NOT — it falls through to device-lifecycle
    user-data classification (D04). Unsupported forms `Option<Work>` / `[Work;N]` /
    `Vec<Work>` / `&Work` are recognized-and-rejected (the `Vec` case is an extra
    `Adt` arm with no clock precedent). Routed away from the user-data classifier in
    `device_lifecycle/resolve.rs::classify_fields`; wired in `bridge/assign.rs` after
    `materialise_drivers`.
  - **Handler binding (`resolve.rs`, mirror `interrupt::resolve_isrs`).** Collects ALL
    `#[rseq::work_handler]` sites; rejects if **any** is not the bare word form via
    `Attribute::is_word()` (uniformly rejects `(field="…")` list, `= "…"`/`= 123`
    name-value, and empty `()` — SF-1, reusing the `HANDLER-SIGNATURE` diagnostic, no
    new id). Validates `fn(&mut self) -> ()`, resolves the bare handler to the sole
    `Work`, **force-materializes** the handler `InstanceRef` (Phase-5, the
    `InterruptHandler.body` mirror), sets `WorkResource.handler`. ≥2 `Work` is
    hard-rejected by the bare-handler discipline (binds resource[0], the second
    required `Work` → `REQUIRED-WORK-NO-HANDLER`; two handlers → `DUPLICATE-HANDLER`) —
    the design-in `BARE-WH-AMBIGUOUS` is NOT built (SF-2).
  - **Submit recognition (`submit.rs`).** The driver-side `intrinsic_kind_for_name` +
    `fn_name_matches_kind` tag `Work::submit` as `DeferredWork(Submit)` (M1 BLOCKING-1
    — model-only `from_name` left it untagged). `validate_deferred_work_signatures`
    (mirror `validate_clock_signatures`) catches callee/receiver-type spoofing. A
    **post-body-materialization** pass (mirror the sync/atomic op-legality body pass —
    NOT the clock signature-check phase, M1 BLOCKING-2) recovers the receiver to a
    `FieldPath`, **rejects** any receiver that is not a **strict single auto-ref hop**
    to a bound `Work` field (`chase_to_place`: the receiver temp's sole reaching def
    must be ONE `Ref`/`RawPtr` of the borrowed place — no `Use`/copy chasing; every
    alias/projection rejects, B-3), and records a `SubmitSite` on the matched resource.
- **Diagnostics (M1–M2, `bridge/mod.rs`).** `BridgeError::DeferredWorkValidation(DeferredWorkDiagnostic)`
  (the `ClockDiagnostic`/`InterruptDiagnostic` mirror). The built `DWS-ERR-*`
  acquire-stage family: `HANDLER-SIGNATURE` (bad return / bad receiver / SF-1 non-bare
  `work_handler`), `REQUIRED-WORK-NO-HANDLER`, `DUPLICATE-HANDLER` (fires single-`Work`),
  `BARE-WH-ZERO-WORK`, submit-spoof, submit-alias (B-3), and the four unsupported-form
  rejects. Design-in ids (`BARE-WH-AMBIGUOUS`, `EXCLUDED-PRIMITIVE`,
  `COLLAPSE-WITH-TIMER`) are NOT added. Every deferred-work bridge reject localizes to
  `Stage::Acquire` (`pipeline.rs` maps every `BridgeError` to `Acquire`).
- **generic_c (M1, M3).** `emit_deferred_work` at the Call dispatch (next to `Clock`);
  `WORK_HELPERS` prelude — `regseq_work_t` slot table, `regseq_work_{init,submit,cancel}`
  (compiler-emitted) + `regseq_work_drain` (**harness-only**, never lowered from author
  DSL) — each printing a `WRK <event>` line, gated on `model_has_deferred_work_resource`
  (RESOURCE presence — a `Work`-less TU's `.c` is byte-unchanged, I06/D07);
  `#define REGSEQ_WORK_SLOTS {deferred_work_resource_count}`. Wrapper synthesis
  (`prepare_deferred_work_emission`, mirror `prepare_irq_emission` minus the IrqResult
  tag / arm reachability): per bound handler a lock-free self-recovering wrapper +
  `regseq_work_init(&__regseq_work_slots[idx], W_f, (void *)&inst)`. Lifecycle slotting
  (`work_init_block`/`work_cancel_block`, mirror `clock_acquire_block`/`clock_release_block`):
  `init_work` in the preamble before `on_init`; `cancel_work` at teardown after
  `on_deinit`, before the resource postambles. The synthesized orderly scenario drives
  `init → (on_init: submit) → drain⇒dispatch → cancel`, then the **post-cancel re-arm
  probe** (a harness-side submit+drain after teardown that dispatches nothing, C05 item
  4); both the post-`on_init` drain and the probe are **submit-gated** (a recorded
  submit site), so a never-submitted `Work` traces `init … cancel`. The `WRK` spelling
  is frozen at the `deferred-work-reset` golden (OI-4).
- **linux_c / zephyr_c.** The real backends **reject** the construct (`Unsupported`) —
  design-in (PRD A1/D20). The sole behavioral oracle is generic-c codegen-shape + the
  synthesized harness (the clock precedent for a host-integration construct whose only
  end-to-end witness, E1000, is not built E2E).
- **Coverage closure (M3).** Matrix rows `DWS-1..DWS-6` added to Doc-1, all `deferred`
  (the LC/IRQ/CLK/PIN precedent — `_deferred_work` is interner-isolated, outside Mode-A
  `discover` — `discover` is non-recursive and the container dir has no manifest — so a
  `covered` status would mint `covered_without_green` drift). Following the IRQ
  precedent: behavioral rows (one fixture covers several — `deferred-work-reset` covers
  DWS-1/2/3) + a single grouped C06 reject-family row (DWS-6, all 16 reject fixtures).
  The real gate is `tests/dws_covers.rs` (covers aggregation) + the sole-binary e2es.
- **Fixtures (`fixtures/_deferred_work/`, interner-isolated, 19).** `deferred-work-reset`
  (the behavioral keystone — byte-exact `golden.c` + `expected.stdout` dual oracle),
  `deferred-work-nosubmit` (T-nosubmit distinguisher), `deferred-work-alias` (T14
  recognition probe — bridge + model inspection, no golden by design), and 16 reject
  fixtures (the built C06 set + the unmarked-`Work` recognition negative). Tests:
  `dws_reset_e2e` / `dws_reset_smoke_e2e` / `dws_nosubmit_e2e` /
  `dws_alias_recognition_e2e` / `dws_reject_e2e` / `dws_covers`, plus the
  `work_hooks_emitted_only_for_deferred_work_tu` byte-stability unit guard in
  `regseq-backends`.

## Amendments (binding, discovered during implementation)

1. **`Work::submit` must be tagged on BOTH the model and the driver (M2, Codex
   pre-finalize BLOCKING-1).** The driver tags intrinsics via
   `bridge/intrinsic/recognition.rs::intrinsic_kind_for_name`, not the model
   `from_name`; model-only wiring left `Work::submit` untagged and M1 never reached
   `emit_deferred_work`. Both mappings are wired and kept in lockstep (a driver-side
   recognition test is the guard).
2. **Submit field-path recovery is a post-body pass, not the signature-check phase
   (M2, Codex pre-finalize BLOCKING-2).** The clock signature-check sees only callee
   item / receiver type; recovering the direct field path + recording per-`Work`
   `submit_sites` needs the materialized call MIR, so it mirrors the sync/atomic body
   pass.
3. **`&f` is the compiler-owned slot, not in-struct storage (M2, Codex pre-finalize
   BLOCKING-3).** `Work` is an opaque ZST and generic-c omits ZST fields, so
   `&inst.inner.work` has no storage; `&f = &__regseq_work_slots[idx]` with a
   deterministic per-resource declaration-order index, consistent across
   init/submit/cancel (a golden same-slot assertion is the guard).
4. **Alias-reject is structural (strict single-auto-ref-hop), not name-based (M2,
   B-3).** The bridge IR strips local names (`LocalDecl.debug_name` is `None` for ALL
   locals, including `self` and any alias), so Codex's suggested "user-name vs
   compiler-temp" discriminator is impossible. `chase_to_place` was rewritten from a
   `Use`-recursion chase to a strict single-`Ref`/`RawPtr` hop: the direct
   `self.work.submit()` accepts (one auto-ref hop); every alias/projection/UFCS
   spelling reborrows and rejects.
5. **`#[rseq::work_handler]` bare-only via `is_word()` (M2, SF-1).** Reject if **any**
   `work_handler` attr is not the bare word form — `Attribute::is_word()` is the
   canonical `AttrArgs::Empty` predicate, uniformly rejecting the `(field="…")` list,
   `= "…"`/`= 123` name-value, and empty `()` forms (a `value_str()`-only check missed
   `= 123`). Reuses `HANDLER-SIGNATURE` — no new locked-set id (C06 stays locked, D17).
6. **The harness drain + post-cancel probe are submit-gated (M3, T15/T16).** Re-gated
   from resource-presence to submit-site presence so the T-nosubmit distinguisher
   traces `init … cancel` with no submit/drain/dispatch while `init_work`/`cancel_work`
   stay resource-gated (D07/D13). The post-cancel re-arm probe (C05 item 4) the M1
   submit-in-`on_init` shape cannot exercise naturally is a harness-side
   submit+drain-after-teardown asserting no dispatch.
7. **The unmarked-`Work` negative rejects at user-data validation, not
   `BARE-WH-ZERO-WORK` (M3, T14, empirical).** A struct-typed unmarked field named
   `Work` is rejected by device-lifecycle user-data classification ("field … of type
   `Work` is not a valid user-data field") BEFORE the bare-handler check fires — a
   stronger, more direct proof that recognition is marker-based (a name-based
   recognizer would have claimed the field and never routed it to user-data).

## Taxonomy feedback notes (recorded, NEVER applied to the frozen pages)

- **`submit_unique` belongs on the same `Work` op family.** The frozen taxonomy
  surface lists 6 ops; v1 builds 5 (init/submit/cancel/drain[harness]/wrapper) and
  reserves `SubmitVariant::SubmitUnique` (the "queue only if not already pending"
  variant) without adding it — design-in.
- **The `offset` field on `WorkResource` is preserved for a future
  `container_of`-style self-recovery** but is unused in v1 (the generic-c backend keys
  on the per-resource slot index; the `Work` ADT is a ZST, so the field has no real
  storage). S14.

## Phase limitations (documented; obligations registered)

- **generic-c only.** `linux_c`/`zephyr_c` reject the construct (`Unsupported`). The
  real-OS lowerings (Linux `INIT_WORK`/`schedule_work`/`cancel_work_sync`, Zephyr
  `k_work_init`/`k_work_submit`/`k_work_cancel_sync`) are design-in; the obligation is
  registered, no P1 emission. (A1 `blocking:false` — if rejected, Tier-2 QEMU
  `-device e1000` becomes required; currently D20-deferred.)
- **No whole-program submit-before-init theorem.** `init_work` precedes the author
  `on_init` **structurally** (the compiler-owned lifecycle entry); the construct does
  NOT prove a submit can never reach a slot before its `init_work` across arbitrary
  call graphs (whole-program analysis is out of scope, I05/D14).
- **`IrqResult::DeferWork` stays guarded.** This construct does NOT un-gate the
  `body_emit.rs` `DeferWork` guard; the existing `irq-deferwork-reject` golden remains
  green (D19/S13 — the IRQ→deferred-work bridge is a separate, later obligation).
- **Out of scope (rejection built, support not):** `Option<Work>` / multi-resource
  `field=` *support*, `submit_unique`, excluded-primitive rejection, the design-in
  `BARE-WH-AMBIGUOUS` (≥2 `Work`), any L2/`platform.toml` work.

## Traceability — realized surface ↔ spec IDs

| Spec ID | Obligation | Realized |
|---|---|---|
| R01 | typed `Work` field, type-recognized, no field attr | `bridge/deferred_work/recognition.rs`; DWS-5; `dws_alias_recognition_e2e` |
| R02 | bind bare `#[rseq::work_handler] fn(&mut self)` | `bridge/deferred_work/resolve.rs`; SF-1 |
| R03 | `self.work.submit()` direct field-path, infallible | `bridge/deferred_work/submit.rs` (B-3); DWS-2 |
| R04 | compiler-emitted `init_work`/`cancel_work`, resource-gated | `work_init_block`/`work_cancel_block`; DWS-1/DWS-4 |
| R05 | per-handler self-recovering lock-free wrapper, mirror interrupt | `prepare_deferred_work_emission`; DWS-2 (no-lock assertion) |
| R06 | traceable `regseq_work_*` hooks, resource-gated | `WORK_HELPERS` + `model_has_deferred_work_resource`; DWS-1 |
| R07 | reject author errors with `DWS-ERR-*` | `DeferredWorkDiagnostic`; DWS-6; `dws_reject_e2e` |
| R08 | reset golden + oracle + T-nosubmit + T-bytestable | `dws_reset_e2e` / `dws_nosubmit_e2e` / `work_hooks_emitted_only_for_deferred_work_tu` |
| R09 | append-only, stable recognition (discriminant + JSON round-trip) | `domain_attr.rs` + `deferred_work.rs` tests |
| I01 | driver-instance recovery (`void *` ctx thread) | wrapper `&__t->inner` |
| I02 | no auto-lock on handler entry | wrapper no-lock (C05 item 3) — DWS-2 |
| I04 | cancel-before-free | `work_cancel_block` teardown placement; DWS-3 |
| I05 | init-before-submit structural for the lifecycle entry | preamble `init_work` before `on_init` (NOT whole-program) |
| I06 | emission resource-gated + byte-stable | `model_has_deferred_work_resource`; DWS-4 + the byte-stability unit guard |
| I07 | submit infallible at surface | facade `submit(&self) -> ()` |
| C01–C07 | facade / model+markers / recognition / lowering / hook ABI+oracle / negatives / lifecycle | as-built sections above |
| D02 | 0 L1 / 0 L2 ⇒ no L2 phase | no config / binder / platform.toml |
| D04 | type-driven recognition (item-DefId abstraction marker) | `recognise_abstraction_attr`; DWS-5 + the unmarked-`Work` negative |
| D07/D13 | resource-gated lifecycle, not submit-gated | DWS-4 (nosubmit) |
| D11/B-3 | direct field-path submit only; alias rejects | `chase_to_place` strict single-auto-ref-hop |
| D19 | `IrqResult::DeferWork` emit guard unrelaxed | `irq-deferwork-reject` golden green |
