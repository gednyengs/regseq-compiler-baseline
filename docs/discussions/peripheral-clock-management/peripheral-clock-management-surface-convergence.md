# peripheral-clock-management — surface convergence (construct #7)

The compiler-side spec contract per the three-repo convention: the converged
DSL surface + the as-built record. Authored at the unit's M4 (2026-06-20); the
eval-side design analysis
(`~/regseq-eval/docs/constructs/peripheral-clock-management/`) cross-links here.
Decision IDs (D##/C##/R##/I##/S##/N#) refer to the unit's GRILL/PRD/PLAN chain
under `.claude/sekekama/grill-to-plan/specs/peripheral-clock-management/`
(`GRILL-2026-06-19-121132` + `PRD-2026-06-19-123922` + `PLAN-2026-06-19-134012`
v2). **Throughline:** clock mirrors the landed `device-interrupt` construct
end-to-end (recognition → `model.clock` table → L2 tenant → lifecycle preamble
slotting → per-backend lowering → coverage ledger).

## The converged surface (frozen; ZERO extensions over the built scope)

- **L1**: a typed `Clock` field on the `#[rseq::driver]` struct via
  `#[rseq::clock(id = "…")]` (`id` MANDATORY, no field-name fallback — D13).
  The `Clock` shell carries three author ops — `enable()` / `disable()` /
  `get_rate()` — tagged on its facade methods with
  `#[rseq::intrinsic("clock_{enable,disable,get_rate}")]` (S15, consumed
  read-only by recognition). `acquire` / `release` are **compiler-managed**:
  synthesized from the clock *field* (the `model.clock` resource), never an
  author call and never an `IntrinsicKind` (I01). The only allowed L1 attr key
  is `id`; `mode`/`default_frequency`/`provider`/`set_rate`/`clk_bulk_*`/
  `clock_auto_enable`/`required` are forbidden L1 keys (`CLK-ERR-FORBIDDEN-KEY`,
  N21 — they belong in the platform section). `ClockOp::SetRate` is reserved
  (not built — R02 / Out-of-Scope).
- **L2**: `[clock.<id>]` — `mode` (`"managed"` | `"static"`),
  `default_frequency` (Hz; REQUIRED and `> 0` on every resolved binding — D15),
  `provider` (the backend-interpreted con-id / DT clock spec; `Some` iff
  `Managed`, `None` iff `Static` — D14/I03). All fields are `Option` at parse
  time; required-ness and the mode↔provider↔frequency invariant are enforced by
  the `ClockBinder` tenant, in exact agreement with
  `ClockBinding::is_resolved()` (C02/I03). A duplicate `[clock.<id>]` section is
  a free TOML parse error; an orphan section (no matching L1 `Clock`) is
  REJECTED at bind (N4 — stricter than interrupt, which accepts orphans).
- **Placement (C07 — the lifecycle ordering contract)**: `acquire_clock` is
  synthesized in the init preamble immediately after the bus `map_region`
  keystone and BEFORE the interrupt register; `release` runs at teardown in
  reverse declaration order, after `on_deinit` (and after the interrupt
  unregister), before the resource (region) postamble (I08). The compiler never
  synthesizes `enable`/`disable` (R04/R05/I01). On the REAL backends, acquisition
  failure is a hard init error that aborts the preamble before any later step
  runs (I02/D05b): linux `devm_clk_get` → `IS_ERR`/`PTR_ERR` propagation, zephyr
  `device_is_ready` → `-ENODEV`. The generic-c oracle is a handle-less global
  singleton, so its `regseq_clock_acquire` is a non-failing `void` trace hook —
  there is no acquisition that can fail to witness there (the backend-[Open]
  device-handle risk lands at M3, not M1).

## As-built (M1–M4, 2026-06-20; compiler branch `construct/peripheral-clock-management`)

- **Model (M1).** `AttrData::ClockResource{id, required} = 6` (next-free after
  `SyncResource = 5`), `IntrinsicKind::Clock(ClockOp) = 7`,
  `ClockOp{Enable=0, Disable=1, GetRate=2}`, `ClockMode{Static=0, Managed=1}`;
  the `ClockTables` sibling table (`model.clock`, the `InterruptTables` mirror,
  keyed by driver `ItemId`). `ClockBinding::is_resolved()` is the single source
  of truth for the mode↔provider invariant AND `default_frequency.is_some_and(>0)`
  (I03/D15) that the tenant `bind`/`validate` and the backend fold-selection both
  agree with (C02). Append-only, `repr(u8)`, pre-construct JSON pinned
  (`#[serde(default)]`); discriminant-stability + no-collision sweep in
  `domain_attr.rs` + `clock.rs` (I06/R13/S13).
- **Config (M1).** `ClockBindingSpec{mode, default_frequency, provider}` (all
  `Option`) + `PlatformBindings.clock` + the `[clock.<id>]` parse branch +
  `BadClock{id,key,detail}` parse error (S01/C01/D14).
- **Frontend / recognition (M2, regseq-driver `bridge/clock/`).** The C06
  acquire-stage family `CLK-ERR-*`: `-MISSING-ID` (N1), `-UNSUPPORTED-FORM`
  (`Option<Clock>`/array/ref/nested, N2/N14), `-MALFORMED-ATTR` (unknown attr
  key, N12), `-ATTR-ON-NON-CLOCK` (N13), `-DUPLICATE-ID` (dup L1 id in one
  driver, N15), `-FORBIDDEN-KEY` (N21), `-SPOOFED-RECEIVER` (a `clock_*`
  intrinsic marker whose receiver is not a bound `Clock`, N22). `clock_*`
  recognition is keyed by the pre-existing facade markers (read-only). The
  infallible `disable()?` is a free Rust type error (N11 — no diagnostic owned).
- **Binding (M2, regseq-l2 `ClockBinder`).** Registered in `REGISTRY` after
  `InterruptBinder` (after bus-resource). The C06 bind-stage family
  `CLK-L2-001..006`: `MissingSection` (N3, 001), `BadMode` (N6, 002),
  `MissingDefaultFrequency`/zero-frequency (N5/N18, 003), `ManagedWithoutProvider`
  (N7, 004), `StaticWithProvider` (N8, 005), `OrphanSection` (N4, 006 — the
  reverse-join reject). The checks agree exactly with `is_resolved()` (C02/I03);
  the `default_frequency` validity split (parse-half in config, semantic-half
  here) covers the full range without a gap.
- **generic_c (M1).** `CLOCK_HELPERS` — the five global-singleton host hooks
  `regseq_clock_{acquire,release,enable,disable,get_rate}` keyed by
  `const char* clk_id`, each tracing a `CLK …` line, with an exact-sized
  `REGSEQ_CLOCK_SLOTS` slot table recording `default_frequency` per clock
  (sized to `clock_resource_count`, mirroring `REGSEQ_ATOMIC_CELLS`; freed on
  release). Gated on `model_has_clock_resource` (resource presence, not
  intrinsic presence) — a no-clock TU is byte-unchanged C (I07). `emit_clock`
  dispatch + `acquire_clock`/`release` slotted into the lifecycle
  preamble/postamble (C05/C07). The `CLK …` trace is the behavioral oracle,
  frozen at `clock-pl011-managed` (OI-3). Static: `enable`/`disable` → no-op,
  `get_rate` → const-fold of `default_frequency`; managed `get_rate` → raw live
  query with no fallback (R08/R10/I04/D07/D10).
- **linux_c (M3).** A managed clock FIELD morphs to `struct clk *` in the priv
  `inner` struct (the sync-lock-morph mirror, via `emit_type_defs`
  field_override); `acquire` = `devm_clk_get(dev, provider)` + `IS_ERR`/`PTR_ERR`
  propagation, slotted after the region map and before the interrupt in BOTH the
  AMBA (`&adev->dev`) and PCI (`&pdev->dev`) scaffolds. `enable` →
  `clk_prepare_enable` (sleepable path only — atomic split designed-in),
  `disable` → `clk_disable_unprepare`, `get_rate` → raw `clk_get_rate`; release
  is automatic (devm). Static → const-fold/no-op. A clock on a
  `#[rseq::serial]` driver is REJECTED loudly (the serial-uart path hardcodes
  `port.uartclk`; clock is not integrated there). (R09/R10/S08/C04/D08/D05b.)
- **zephyr_c (M3).** `device_is_ready(<ctlr>)` acquire gate (`-ENODEV`), slotted
  region→clock→`IRQ_CONNECT` in the init hub. Managed is INLINE (no field morph
  — the controller is a build-time-const DT macro):
  `DEVICE_DT_GET(DT_INST_CLOCKS_CTLR_BY_IDX(0, idx))` + `clock_control_on`/`_off`/
  `_get_rate`; the subsys is synthetic
  `(clock_control_subsys_t)DT_INST_CLOCKS_CELL_BY_IDX(0, idx, id)`. Static →
  const-fold/no-op. Real PL011-Zephyr-qemu is static, so the managed path is
  witnessed SYNTHETICALLY (D12). (R09/S09/C04.)
- **Coverage closure (M4).** Matrix rows `CLK-1..CLK-9` (7 behavioral + 2 reject
  families) added to Doc-1, all `deferred` (the LC/IRQ precedent — `_clock` is
  interner-isolated, outside Mode-A `discover`, so `covered` would mint
  `covered_without_green` drift). The real gate is `tests/clock_covers.rs`
  (covers aggregation) + the sole-binary clock e2es. Discriminant-stability:
  `domain_attr.rs` pins `AttrData::ClockResource=6`/`IntrinsicKind::Clock=7`/
  `ClockOp` + the no-collision check (T01); `clock.rs` pins `ClockMode` (T19).
- **Fixtures.** `fixtures/_clock/` (interner-isolated): the behavioral-oracle
  pair `clock-pl011-managed` / `clock-pl011-static` (D09), the composed
  interaction `clock-pl011-composed` (region + managed clock + interrupt, C07
  ordering), 14 reject fixtures (7 acquire + 7 bind), and the linux/zephyr ×
  managed/static emission witnesses (distinct from the generic-c behavioral
  oracle — D09).

## Amendments (binding, discovered during implementation)

1. **enable() failure PROPAGATES (M3, Codex BLOCKING #1).** Managed
   `clk_prepare_enable`/`clock_control_on` nonzero return constructs `Err` into
   the author's `Result<(), RegseqError>` destination (so the author's `?`
   returns it), NOT a silent `Ok(())`. `Result<(), RegseqError>` is
   niche-encoded (Ok = 0x2, Err = the RegseqError byte), so `memset(0)` =
   `Err(RegseqError::HardwareError)` → `-EIO` via the scaffold adapter. v1 maps
   ANY enable failure to RegseqError's first variant; a precise
   errno→RegseqError policy is out of scope (only `devm_clk_get` acquire carries
   the native `PTR_ERR` channel).
2. **zephyr get_rate FAILS LOUD (M3, Codex BLOCKING #2).** `get_rate` returns
   `u32` (no error channel), so a `clock_control_get_rate` failure `k_panic()`s
   rather than silently returning a fallback `0`. (Linux `clk_get_rate` already
   returns the rate directly — raw, no change.)
3. **Shared-codegen latent bug fixed (M3, Codex BLOCKING round-2; spans beyond
   clock).** `linux_c`/`zephyr_c` `Rvalue::Discriminant` read the RAW tag byte,
   but the MIR `SwitchInt` switches on the Rust SEMANTIC discriminant. Clock's
   `enable()?` is the FIRST `?`-on-fallible in a per-target body → the first to
   switch a niche-encoded `Result`; the Ok path (raw `0x2`) fell through to
   `BUG()`/`k_panic`. Fixed both `Rvalue::Discriminant` arms to decode
   tag→discriminant (each tagged variant's `tag_value` → its `discriminant`,
   else the untagged niche variant's), GATED on
   `is_identity = variants.all(|v| v.tag_value == Some(v.discriminant))` so
   Direct enums (incl. explicit discriminants) keep the legacy raw read (ZERO
   golden churn) and only niche enums decode. Verified by extracting the emitted
   linux `on_init`, stubbing the kernel hooks, and RUNNING it (enable=OK → Ok,
   no BUG; enable=FAIL → Err propagated).
4. **First author-accessed ZST resource (M1).** `Clock` is the first ZST
   resource the author *accesses* (`self.clk.enable()`). ZST fields are dropped
   from the emitted C struct, so `&(*_1).clk` was invalid; fixed `place_walk`
   (skip dropped ZST fields, leave the lvalue at the parent) and `ref_expr`
   (cast `&parent` to the receiver ptr type for a ZST place — never deref'd,
   matching Rust semantics). A shared-codegen fix affecting only ZST-field
   borrows (previously unhit).

## Taxonomy feedback notes (recorded, NEVER applied to the frozen pages)

1. **D13 — `id` always required**: as with device-interrupt, this construct
   REQUIRES `id` (no field-name fallback). The peripheral input clock is a
   board-level identity selected by the integrator, not a struct-layout
   accident — an intended tightening. Feed back when the taxonomy thaws.
2. **Orphan `[clock.<id>]` rejected (N4)**: clock chose a strict reverse-join
   (an orphan platform section with no matching L1 `Clock` is a bind error),
   stricter than device-interrupt (which accepts orphans). Recorded as a
   deliberate divergence, not an inconsistency.

## Phase limitations (documented; obligations registered)

- **Out of scope (designed-in, not built):** `set_rate` (+ `ClockOp::SetRate`),
  `Option<Clock>` (+ `devm_clk_get_optional`), the atomic-context `clk_enable`
  split (v1 emits only the sleepable `clk_prepare_enable`), the reset-relative
  ordering edge, `clk_bulk_*`, the managed `get_rate` `rate == 0` runtime
  diagnostic, the acquired-but-never-enabled warning, and the frozen PL011 union
  re-wire (path A — S14 property stand-in / S15 facade are GRILL negative-space).
- **Witness scope (honest):** managed gating is witnessed on Linux + a synthetic
  Zephyr fixture; the real PL011-Zephyr-qemu target is static. The construct is
  N=1 (PL011-only-witnessed, QEMU-static) — artifact-integrity polish per the
  PLAN's scheduling note, not a held-out generalization claim.

## Traceability — realized surface ↔ spec IDs

Requirements (R01–R13):

| ID | realized by |
|----|-------------|
| R01 declare `Clock` field; compiler-managed acquire/release | recognition (`bridge/clock`) + `emit_lifecycle_synthesis` |
| R02 `enable`/`disable`/`get_rate` author ops (`set_rate` reserved) | `IntrinsicKind::Clock(ClockOp)` + `emit_clock` (all backends) |
| R03 `[clock.<id>]` platform section | `ClockBindingSpec` + parse branch |
| R04 synth `acquire_clock` after map_region, before interrupt; no auto-enable | generic-c/linux/zephyr preamble slotting |
| R05 release reverse-decl-order, after quiesce before unmap; release-only | teardown postamble slotting |
| R06 `ClockBinder` join into `model.clock`; unjoined required = error | `ClockBinder` + `CLK-L2-001` |
| R07 generic-c `regseq_clock_*` gated on `model_has_clock_resource`; oracle | `CLOCK_HELPERS` + gate + `clock-pl011-managed` golden |
| R08 static fold to const/no-op in backend; op + binding stay in IR | static arms of `emit_clock` (all backends) |
| R09 Linux + Zephyr both modes (full 2×2) | `linux_c/clock.rs` + `zephyr_c/clock.rs` + witnesses |
| R10 managed `get_rate` raw live, no fallback; static folds | generic-c/linux/zephyr `get_rate` lowering |
| R11 `CLK-*`/`CLK-L2-*` negative set | `ClockDiagnostic` + `ClockL2Error` + reject corpus |
| R12 fixtures: oracle pair + emission witnesses, kinds distinguished | `fixtures/_clock/` + `clock_covers.rs` |
| R13 append-only recognition enums + discriminant-stability + JSON round-trip | `domain_attr.rs` + `clock.rs` sweeps |

Invariants (I01–I10):

| ID | realized by |
|----|-------------|
| I01 acquire/release compiler-managed; enable/disable/get_rate author; no auto-enable | preamble synthesis + `emit_clock` |
| I02 required form never treats absent clock as present; acquire failure = hard error | linux `PTR_ERR` abort + zephyr `device_is_ready`/`-ENODEV` (generic-c acquire is a handle-less trace-only hook — no failure path) |
| I03 managed ⇒ provider; static ⇏ provider; `default_frequency` always `> 0` | `ClockBinding::is_resolved()` + `ClockBinder` |
| I04 `default_frequency` never a managed runtime fallback | managed `get_rate` raw; static fold only |
| I05 `DriverHandle` holds only constitutive resources; clock = top-level table | `model.clock: ClockTables` |
| I06 recognition enums append-only, `repr(u8)`, next-free, no collision | `domain_attr.rs` no-collision sweep |
| I07 no-`ClockResource` TU emits byte-unchanged C | `model_has_clock_resource` gate |
| I08 lifecycle order bus→map_region→acquire_clock→interrupt, exact reverse | `clock-pl011-composed` (CLK-7) |
| I09 clock id namespace construct-local; key platform-global; dup L1 id rejected | `CLK-ERR-DUPLICATE-ID` + per-driver join |
| I10 narrow v1 enforcement (no enable-before-access, no atomic-ctx reject) | scope of the built recognition/lowering |

Contracts (C01–C07):

| ID | realized by |
|----|-------------|
| C01 `[clock.<id>]` + `ClockBindingSpec` | regseq-config |
| C02 `model.clock: ClockTables`, `is_resolved()` invariant | regseq-model + `ClockBinder` + `CLK-L2-*` |
| C03 recognition / `from_name` | `domain_attr.rs` + `bridge/clock` |
| C04 per-op lowering matrix (managed \| static) × (generic-c \| linux \| zephyr) | the three backend `emit_clock` paths |
| C05 generic-c host hook ABI (`regseq_clock_*`, singleton keyed by `clk_id`) | `CLOCK_HELPERS` |
| C06 negative / compile-fail set (`CLK-*` / `CLK-L2-*`) | recognition + binding diagnostics + reject corpus |
| C07 lifecycle ordering (init preamble + exact-reverse deinit) | preamble/postamble slotting + `clock-pl011-composed` |

Surfaces S14 (`#[rseq::property]` stand-in) and S15 (facade markers) are GRILL
negative-space (`considered — no decision`); S15's facade markers are consumed
read-only by recognition and need no change.
