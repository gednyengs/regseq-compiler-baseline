# pin-controller-and-GPIO-configuration — surface convergence

The compiler-side spec contract per the three-repo convention: the converged DSL
surface + the as-built record. Authored at the unit's M5 (2026-06-21); the eval-side
design analysis (`~/regseq-eval/docs/constructs/pin-controller-and-gpio-configuration/`)
cross-links here. Decision IDs (D##/C##/R##/I##/S##) refer to the unit's GRILL/PRD/PLAN
chain under
`~/regseq-eval/.claude/sekekama/grill-to-plan/specs/pin-controller-and-gpio-configuration/`
(`GRILL-2026-06-20-164240` + `PRD-2026-06-20-170618` + `PLAN-2026-06-20-173440`).
**Scope:** this construct is **pinctrl state-selection** only — a driver declares a
typed `PinCtrl` resource and `apply::<S>()`s to select a *named pin state* (e.g.
`default`/`sleep`); raw per-pin GPIO line manipulation is out of scope (PRD).
**Throughline:** pinctrl mirrors the landed `peripheral-clock-management` construct
end-to-end (recognition → `model.pinctrl` table → L2 tenant → lifecycle preamble
slotting **before** the clock → per-backend lowering → coverage ledger), with the
pinctrl-specific twist of **author-declared pin-state markers** and a **generic-`S`
→ marker** resolution that has no clock precedent.

## The converged surface (frozen; ZERO extensions over the built scope)

- **L1**: a typed `PinCtrl` field on the `#[rseq::driver]` struct via
  `#[rseq::pinctrl(id = "…")]` (`id` MANDATORY, no field-name fallback — D13). Two
  forms: **REQUIRED** `PinCtrl` and **OPTIONAL** `Option<PinCtrl>` (D02/D03 — both
  built). The `PinCtrl` shell carries ONE author op — `apply::<S>()` — tagged on its
  facade method with `#[rseq::intrinsic("pinctrl_apply")]`; on the optional form the
  forwarding `apply` lives on the `OptionalPinCtrl` extension trait whose concrete
  `impl … for Option<PinCtrl>` method carries the same intrinsic tag (post-mono the
  bridge canonicalizes the static trait call to that resolved-impl `DefId`, D02/C08).
  `acquire` / `release` are **compiler-managed**: synthesized from the pinctrl *field*
  (the `model.pinctrl` resource), never an author call and never an `IntrinsicKind`
  (I01/D05). The pin states are **author-declared markers** — unit ZST structs tagged
  `#[rseq::pin_state(id = "…")]` implementing the empty `PinState` trait (D04); the
  state name reaching L1 is read off the `S` marker's `PinStateMarker.id`, and ONLY a
  state name enters L1 (I01). The only allowed L1 attr keys are `id` (on the field)
  and `id` (on a marker).
- **L2**: `[pinctrl.<id>]` — a single key `pin_states` (an array of state-name
  strings) supplying the inventory the apply selects from (S01/C01). It MUST contain
  `"default"` (D07 — `acquire_pinctrl`'s default-apply has defined behavior only when
  the inventory declares it). SHAPE errors (missing key / non-array / non-string
  entry) are the parser's `BadPinctrl` (C01/T03/SF6); CONTENT errors are the
  `PinctrlBinder` tenant's `PIN-L2-*` family (below). A required `PinCtrl` with no
  section is `PIN-L2-001 NO-INVENTORY`; an **optional** `Option<PinCtrl>` with no
  section binds `None` (`resolved = false`) — a legal absent resource (D02/D06). A
  duplicate `[pinctrl.<id>]` section is a free TOML parse error; an orphan section (no
  matching L1 `PinCtrl`) is REJECTED at bind (`PIN-L2-006`, the clock-strict
  reverse-join).
- **Placement (C07/D08 — the lifecycle ordering contract)**: `acquire_pinctrl` is
  synthesized in the init preamble immediately after the bus `map_region` keystone and
  **BEFORE** the clock acquire (**pins-before-clock** — a board powers/muxes its pins
  before clocking the block), with the `default` state applied at acquire; `release`
  runs at teardown in reverse order, after `on_deinit` and after the clock release,
  before the region postamble. The compiler never synthesizes `apply` (R04/I01). On
  the REAL backends the **required** form's acquisition failure is a hard init error
  that aborts the preamble before any later step runs (I02): linux `devm_pinctrl_get`
  → `IS_ERR`/`PTR_ERR` + a `pinctrl_lookup_state`/`pinctrl_select_state` "default"
  with native-rc propagation; zephyr `pinctrl_apply_state(…, PINCTRL_STATE_DEFAULT)`
  rc-checked with native-rc propagation. The generic-c oracle's `regseq_pinctrl_acquire`
  carries a host injectable-failure knob (`PIN acquire …` then `LC acquire-fail
  pinctrl …`, S12). The **optional** form's apply is best-effort (rc ignored, C04).

## As-built (M1–M5, 2026-06-21; compiler branch `construct/pin-controller-and-gpio-configuration`)

- **Model (M1).** `AttrData::PinctrlResource{pinctrl_id, required} = 7` (next-free
  after `ClockResource = 6`), `AttrData::PinStateMarker{id} = 8`,
  `IntrinsicKind::Pinctrl(PinctrlOp) = 8` (next-free after `Clock = 7`),
  `PinctrlOp{ApplyState = 0}`; the `PinctrlTables` sibling table (`model.pinctrl`, the
  `ClockTables` mirror, keyed by driver `ItemId`). `PinctrlBinding::is_resolved()` =
  `resolved && !pin_states.is_empty()` is the single source of truth the tenant
  `bind`/`validate` and the backend fold-selection both agree with (C02/I03);
  `resolved = false` ⇒ empty `pin_states`. Append-only, `repr(u8)`, pre-construct JSON
  pinned (`#[serde(default)]`); discriminant-stability + no-collision sweep in
  `domain_attr.rs` + `pinctrl.rs` (I06/R13/S14).
- **Config (M1).** `PinctrlBindingSpec{pin_states: Vec<String>}` + `PlatformBindings.pinctrl`
  + the `[pinctrl.<id>]` parse branch + `BadPinctrl{id,key,detail}` parse error — the
  array-of-strings shape (per-entry `toml::Value::String` check) is new code, no scalar
  precedent (S01/C01/D14).
- **Frontend / recognition (M2, regseq-driver `bridge/pinctrl/`).** The C06
  acquire-stage family `PIN-ERR-*`: `-MISSING-ID`, `-MALFORMED-ATTR`,
  `-ATTR-ON-NON-PINCTRL`, `-UNSUPPORTED-FORM` (`[PinCtrl;N]`/`Vec<PinCtrl>`/bare
  `&PinCtrl`), `-DUPLICATE-ID`, `-MALFORMED-STATE-ATTR`, `-NON-ZST-MARKER`,
  `-DUPLICATE-STATE-MARKER`, `-INVALID-STATE-MARKER` (resolved name ∉ inventory —
  scoped to required/optional-present), `-UNRESOLVED-STATE-PARAM` (a generic `S` at the
  body site), `-AUTHOR-MANAGED-FIELD`, `-SPOOFED-RECEIVER` (legal receivers are
  `&PinCtrl` and `&Option<PinCtrl>`, D14). The post-bridge body-walk checks
  (`PIN-ERR-AUTHOR-MANAGED-FIELD`/`-UNRESOLVED-STATE-PARAM`/`-INVALID-STATE-MARKER`)
  live in `regseq-model` `pinctrl_semantics.rs` (the locked-code contract); the rest
  are bridge recognition. Recognition resolves `apply::<S>()`'s concrete marker
  `DefId` → `PinStateMarker.id` → the resolved state name stamped on the op (the new
  shape, no clock precedent).
- **Binding (M2, regseq-l2 `PinctrlBinder`).** Registered in `REGISTRY` after
  `ClockBinder` (reads no clock state). The C06 bind-stage family `PIN-L2-001..006`:
  `NoInventory` (001 — required, no section), `NoDefault` (002 — D07), `EmptyStates`
  (003), `EmptyStateName` (004), `DuplicateState` (005), `OrphanSection` (006). The
  `"default"` membership invariant is enforced HERE at bind (not in the model — the
  model intentionally delegates it to L2). `PIN-L2-MANY-MATCH` is **structurally
  unreachable** (explicit-`id` join + TOML duplicate-section rejection +
  `PIN-ERR-DUPLICATE-ID` together leave no input resolving one field to >1 section) —
  enforced by construction, no fixture.
- **generic_c (M1–M3).** `PINCTRL_HELPERS` — host hooks `regseq_pinctrl_{acquire,
  apply,release}` keyed by `const char* pinctrl_id`, each tracing a `PIN …` line, with
  an ordered `(event,state)` slot table sized to the resource count; gated on
  `model_has_resolved_pinctrl_resource` (RESOLVED presence — an absent-only/pinctrl-less
  TU is byte-unchanged C, I07). `emit_pinctrl` dispatch + `acquire_pinctrl`/`release`
  slotted into the preamble/postamble after `map_region` and BEFORE the clock acquire
  (C05/C07/D08). The `PIN …` trace is the behavioral oracle, `PIN` spelling frozen at
  `pinctrl-pl011-optional-bound-min` (the first golden). **Optional-absent compile-time
  branch fold (D06):** an absent `Option<PinCtrl>` statically folds `apply::<S>()` to a
  direct `Err(PinctrlAbsent)` — no `regseq_pinctrl_apply`, no `PIN apply` trace, no
  acquire/release, no runtime `Option` tag; the generic-c `errno` table gains
  `"PinctrlAbsent" => "ENODEV"` (it lands with the fold, since generic-c fails loud on
  an unmapped variant).
- **linux_c (M4, `linux_c/pinctrl.rs`).** The **required** ZST `PinCtrl` morphs to a
  `struct pinctrl *` member in the priv `inner` struct (the clock-field-morph mirror);
  acquire = `devm_pinctrl_get` + `IS_ERR`/`PTR_ERR` + a `pinctrl_lookup_state`/
  `pinctrl_select_state` "default" (native-rc propagated — preserves `-EPROBE_DEFER`),
  slotted after the region map and before the clock acquire in BOTH the AMBA
  (`&adev->dev`) and PCI (`&pdev->dev`) scaffolds; author `apply::<S>()` →
  `pinctrl_lookup_state` + `pinctrl_select_state` over the morphed field (required
  hard-fails on either). The **optional** form needs NO field morph: it uses the
  handle-less PM helpers `pinctrl_pm_select_<state>_state(dev)` with `dev` recovered
  per-site via `container_of(_1, struct <drv>_priv, inner)` → `&__rseq_priv->adev->dev`
  (the config-space construct's recovery idiom), rc `(void)`-ignored; the acquire is an
  auto-bind no-op (the kernel selects "default" pre-probe). Absent → the direct
  `Err(PinctrlAbsent)` fold (`return -ENODEV`, no API call, no `<linux/pinctrl/consumer.h>`).
  The errno table gains `"PinctrlAbsent" => "ENODEV"` (T19). (R09/S08/C04/D12.)
- **zephyr_c (M4, `zephyr_c/pinctrl.rs`).** DT-config, **no handle / no field morph**:
  `PINCTRL_DT_INST_DEFINE(0)` (top-level, resolved-only) + the init-hub
  `pinctrl_apply_state(PINCTRL_DT_INST_DEV_CONFIG_GET(0), PINCTRL_STATE_DEFAULT)`
  acquire (rc-checked + native-rc propagated for required; `(void)` for optional),
  slotted before the clock gate; author `apply::<S>()` →
  `pinctrl_apply_state(…, PINCTRL_STATE_<S>)` (required rc-checked / optional `(void)`).
  The binding gains `pinctrl-device.yaml` when resolved. Absent → the direct fold
  (`return -ENODEV`, no `pinctrl_apply_state`, no `PINCTRL_DT_INST_DEFINE`, no header,
  no yaml). The errno table gains `"PinctrlAbsent" => "ENODEV"` (T19). Witnessed
  SYNTHETICALLY (the real PL011-Zephyr-qemu target is static). (R10/S09/C04/D12.)
- **Coverage closure (M5).** Matrix rows `PIN-1..PIN-6` (framing, apply, resolution +
  both paths, absent-fold, required-acquire-failure, composed C07 ordering) +
  `PIN-8/PIN-9` (recognition + binding reject families) added to Doc-1, all `deferred`
  (the CLK/IRQ precedent — `_pinctrl` is interner-isolated, outside Mode-A `discover`,
  so `covered` would mint `covered_without_green` drift). **`PIN-7` is intentionally
  unminted (reserved):** pinctrl's behavioral surface is narrower than clock's (no
  managed/static duality), so the real-backend Linux/Zephyr lowering rides PIN-1..PIN-4
  + PIN-6 (the backend-agnostic rows) rather than a separate parity row, and the
  rejects stay at their as-built PIN-8/PIN-9 ids. The real gate is
  `tests/pinctrl_covers.rs` (covers aggregation) + the sole-binary pinctrl e2es.
  Discriminant-stability: `domain_attr.rs` pins the two `AttrData` variants +
  `IntrinsicKind::Pinctrl`/`PinctrlOp` + the no-collision check; `pinctrl.rs` pins the
  table round-trips + `is_resolved()` truth table (T25).
- **Fixtures.** `fixtures/_pinctrl/` (interner-isolated, 29): the present/pass set
  (`pinctrl-pl011-{optional-bound,optional-bound-trait,required-bound,required-bound-min}`),
  the optional-absent pair (`-optional-absent{,-iflet}`), `-required-acquire-failure`,
  the composed interaction `pinctrl-pl011-composed` (region + required pinctrl + managed
  clock + interrupt, C07 ordering), 21 reject fixtures (recognition + binding), and the
  linux/zephyr emission witnesses (distinct from the generic-c behavioral oracle).

## Amendments (binding, discovered during implementation)

1. **Absent fold uses the COMPOSITE offset (M4, Codex foundation #1).** The
   optional-absent fold writes `Err(PinctrlAbsent)` using the composite offset (the
   `Result` `Err`-payload-field offset + the `RegseqError` tag offset) read from the
   layout, with `PinctrlAbsent` found BY NAME — not an offset-0 assumption.
2. **Required Linux preamble PROPAGATES the native rc (M4, Codex foundation #2/#3).**
   The required acquire propagates the `pinctrl_select_state` rc (`if (__ret) return
   __ret;`, preserving `-EPROBE_DEFER`), not a flattened `-EINVAL`; the apply uses a
   temp `pinctrl_lookup_state` + `IS_ERR` check then `pinctrl_select_state` (no inlined
   lookup-in-select — `pinctrl_lookup_state` returns `ERR_PTR`).
3. **Required Zephyr acquire propagates the native rc (M4, Codex foundation #4).**
   `pinctrl_apply_state` nonzero `return __ret`, not a flattened `-ENODEV`.
4. **Resolved-only scaffolding (M4, Codex foundation #5).** ALL real pinctrl
   scaffolding (field morph / DT define / includes / yaml) is gated on
   `has_resolved_pinctrl`; an absent-only TU emits ONLY the fold, byte-stable (I07).
5. **Non-standard states fail loud (M4, Codex foundation #7).** D10 fixes the standard
   `default`/`sleep` states; any other named state is an `EmitError` (loud) in every M4
   real-backend path.
6. **Serial fail-loud guard (M4, Codex diff round-1 BLOCKING).** A `#[rseq::serial]`
   driver with a RESOLVED pinctrl is REJECTED loudly early in both serial emit fns
   (`linux_c/serial.rs`, `zephyr_c/serial.rs`) — the shared `body::emit_pinctrl` would
   otherwise lower an authored `apply` over a non-morphed field / no DT define = broken
   C. An optional-ABSENT pinctrl in serial needs no scaffold (its apply folds), so it
   is allowed.

## Taxonomy feedback notes (recorded, NEVER applied to the frozen pages)

1. **D13 — `id` always required**: as with device-interrupt and clock, this construct
   REQUIRES `id` (no field-name fallback). The pin group is a board-level identity
   selected by the integrator, not a struct-layout accident. Feed back when the
   taxonomy thaws.
2. **Author-declared pin-state markers (D04)**: the pin-state inventory is named by
   *author-declared* `#[rseq::pin_state]` markers, not framework-canonical constants —
   the state name is the author's contract surface, validated against the integrator's
   `pin_states`. A pinctrl-specific shape with no clock/interrupt precedent.
3. **Orphan `[pinctrl.<id>]` rejected (PIN-L2-006)**: a strict reverse-join, mirroring
   clock (stricter than device-interrupt, which accepts orphans). Deliberate.

## Phase limitations (documented; obligations registered)

- **Plain `if let Some` does NOT lower on the real backends.** The
  `if let Some(p) = &self.pinctrl { p.apply::<S>() }` path produces a `switchInt` +
  `Downcast` place the Linux/Zephyr body-emit first-light subset rejects. It is proven
  behaviorally by the **generic-c** oracle (the T15 recognition shape); on the real
  backends it is **design-in**. The optional real-backend witnesses use the
  `OptionalPinCtrl` **trait** path (intercepted at the call site, no switchInt). D02
  coverage is therefore "generic-c behavioral + real-backend trait-path" — NOT plain
  if-let on Linux/Zephyr. Unblocking needs switchInt/Downcast lowering + Option-tag
  materialization on the real backends (a substantial lift, out of N=1 scope).
- **Real backends lower `default`/`sleep` only.** Arbitrary named states are a loud
  `EmitError` on Linux and Zephyr today (amendment #5); the generic-c oracle accepts
  any inventory-resolved name. Widening the real-backend state map is design-in.
- **Resolved pinctrl is not wired through the dedicated real-backend serial scaffolds.**
  The witnesses use the generic AMBA/PCI path; a `#[rseq::serial]` driver with a
  resolved pinctrl fails loud (amendment #6). Integrating pinctrl into the serial-uart
  fast-path is out of scope.
- **Optional vs required failure semantics differ.** The optional real-backend apply is
  best-effort (rc ignored); the required form hard-fails. `Result<(), RegseqError>` on
  the optional `apply` therefore does NOT mean uniform failure propagation on the real
  backends — required propagates, optional swallows (C04/D10). Explicit so a spec reader
  does not assume otherwise.
- **Degenerate UNUSED `PinCtrl` field panics the bridge.** A driver with a `PinCtrl`
  field but an empty `impl Lifecycle` and no `apply` panics the bridge
  (`construct_nominal_adt_instances`/`rekey_synthesis_in_phase5`). Real drivers always
  apply, so it is degenerate; a pre-existing gap, not introduced here.
- **generic-c A1 no-rollback oracle.** The generic-c required-acquire-failure trace
  stops at `LC acquire-fail pinctrl <id>`; there is no rollback/unwind oracle for a
  *partial* multi-resource preamble failure (the real backends abort probe natively).
- **Witness scope (honest):** N=1 (PL011-only-witnessed); Zephyr is synthetic
  (QEMU-static target). State-selection only — artifact-integrity per the PLAN's
  scheduling note, not a held-out generalization claim.
- **Tier-2 (eval QEMU) behavioral smoke — Linux: PASSED (2026-06-21).** A
  compiler-generated **plain-AMBA** PL011 driver carrying an **optional-present**
  pinctrl (the serial fast-path fail-loud-rejects a resolved pinctrl, so the serial
  driver could not be the vehicle) was emitted to `golden.linux.c`, compiled into a
  real `pl011pin.ko` (vermagic `6.18.21 SMP aarch64`), and run on the eval
  Linux+QEMU substrate: it bound the reserved `9040000.pl011`, logged `pl011pin up`
  (probe ran the real `pinctrl_pm_select_default_state`), kept the console intact,
  and tore down cleanly (`pl011pin deinit…`, no oops) + `rmmod`. Vehicle:
  `_pinctrl_tier2/pl011pin-optional` (+ `tests/linux_pinctrl_tier2_e2e.rs`); campaign
  archive `pl011/linux/results/campaigns/2026-06-21-pinctrl-tier2/`. **Honest scope:**
  QEMU `virt` has NO pin controller, so `pinctrl_pm_select_default_state` is inert
  (returns 0, `dev->pins` NULL) — this is a **non-regression smoke** (the pinctrl
  scaffold does not break a real PL011 probe/teardown), NOT a witness of real pin
  muxing. The **required** form cannot be Tier-2'd on QEMU virt (its
  `pinctrl_lookup_state("default")` fails probe with no DT pin states). The Zephyr
  Tier-2 leg is not run (the PL011-Zephyr-qemu target is static; same inert reality).
- **Eval design-analysis errata applied (2026-06-21):** `PinCtrlAbsent` →
  `PinctrlAbsent`; "framework canonical markers" → author-declared; "build-narrow" →
  build-wide (per D02/D03/D10). Applied across the eval design analysis folder.

## Traceability — realized surface ↔ spec IDs

Requirements (R01–R15):

| ID | realized by |
|----|-------------|
| R01 declare `PinCtrl` field (required + optional); compiler-managed acquire/release | recognition (`bridge/pinctrl`) + lifecycle synthesis |
| R02 `apply::<S>()` author op; generic-`S` → marker resolution | `IntrinsicKind::Pinctrl(ApplyState)` + recognition marker resolve |
| R03 deferred-receiver shapes (if-let generic-c; trait real-backend) | `if let` (generic-c, T15) + `OptionalPinCtrl` trait |
| R04 synth `acquire_pinctrl` (default applied) after map_region, before clock; no auto-apply | generic-c/linux/zephyr preamble slotting |
| R05 `[pinctrl.<id>]` `pin_states` platform section | `PinctrlBindingSpec` + parse branch |
| R06 `PinctrlBinder` join into `model.pinctrl`; required-unjoined = error; `PIN-L2-*` | `PinctrlBinder` + `PIN-L2-001..006` |
| R07 marker resolution + membership validation (scoped) | recognition + `PIN-ERR-INVALID/UNRESOLVED-STATE-*` |
| R08 generic-c `regseq_pinctrl_*` gated on `model_has_resolved_pinctrl_resource`; oracle | `PINCTRL_HELPERS` + gate + first golden |
| R09 Linux lowering (required morph + optional PM helpers + absent fold) | `linux_c/pinctrl.rs` + witnesses |
| R10 Zephyr lowering (DT define + apply_state + absent fold) | `zephyr_c/pinctrl.rs` + witnesses |
| R11 optional-absent compile-time fold across all three backends | generic-c (T14) + linux/zephyr (T20/T21) + witnesses |
| R12 `PIN-ERR-*`/`PIN-L2-*`/`BadPinctrl` negative set | recognition + binding + parser + reject corpus |
| R13 append-only recognition enums + discriminant-stability + JSON round-trip | `domain_attr.rs` + `pinctrl.rs` sweeps |
| R14 `PinctrlAbsent` errno (`ENODEV`) in three backend tables | generic-c (T14) + linux/zephyr (T19) |
| R15 true end-to-end vertical slice (M1) + deferred receiver (M3) | `pinctrl-pl011-optional-bound-min` + if-let fixture |

Invariants (I01–I07):

| ID | realized by |
|----|-------------|
| I01 acquire/release/marker-resolution compiler-managed; only a state name enters L1 | preamble synthesis + recognition |
| I02 required form never treats absent pinctrl as present; acquire failure = hard error; apply never reads MMIO | linux `PTR_ERR`/rc abort + zephyr rc abort + generic-c failure knob |
| I03 `is_resolved()` (resolved ∧ non-empty states) the single truth | `PinctrlBinding::is_resolved()` + tenant + backend gate |
| I04 `"default"` membership enforced at bind | `PinctrlBinder` `PIN-L2-002` |
| I05 the `PIN …` trace is the generic-c oracle (first golden blesses `PIN`) | `PINCTRL_HELPERS` + `pinctrl-pl011-optional-bound-min` |
| I06 recognition enums append-only, `repr(u8)`, next-free, no collision | `domain_attr.rs` no-collision sweep |
| I07 no-resolved-pinctrl TU emits byte-unchanged C (absent-only = fold only) | `model_has_resolved_pinctrl_resource` gate |

Contracts (C01–C08):

| ID | realized by |
|----|-------------|
| C01 `[pinctrl.<id>]` `pin_states` (SHAPE parser / CONTENT binder) | regseq-config `BadPinctrl` (T03) + `PinctrlBinder` `PIN-L2-*` (T11) |
| C02 `model.pinctrl: PinctrlTables`, `is_resolved()` invariant | regseq-model + `PinctrlBinder` |
| C03 recognition / `from_name` / generic-`S` → marker | `domain_attr.rs` + `bridge/pinctrl` + `pinctrl_semantics.rs` |
| C04 per-op lowering matrix (required \| optional \| absent) × (generic-c \| linux \| zephyr) | the three backend `emit_pinctrl` paths |
| C05 generic-c host hook ABI (`regseq_pinctrl_*`, keyed by `pinctrl_id`) | `PINCTRL_HELPERS` |
| C06 negative / compile-fail set (`PIN-ERR-*` / `PIN-L2-*` / `BadPinctrl`) | recognition + binding + parser + reject corpus |
| C07 lifecycle ordering (pins-before-clock init preamble + exact-reverse deinit) | preamble/postamble slotting + `pinctrl-pl011-composed` (PIN-6) |
| C08 optional-form forwarding (`OptionalPinCtrl` trait → resolved-impl `DefId`) + `PinctrlAbsent` | recognition canonicalization + the three errno tables |

Surface S16 (a property-style pin-state stand-in) is GRILL negative-space
(`considered — no decision`; unblocking also needs optional-`Clock`, out of scope).
