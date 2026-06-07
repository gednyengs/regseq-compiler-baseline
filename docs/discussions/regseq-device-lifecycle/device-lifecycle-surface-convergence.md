# Device Lifecycle — converged DSL surface + as-built (eval ⇄ compiler)

Construct #4 (taxonomy: *driver self-registration, device↔driver matching,
probe/remove entry points*; 100% cross-OS prevalence). This is the **spec
contract** for the Device Lifecycle construct: the converged author-facing
surface (what the DSL exposes) plus the **as-built** Stage-2 realization in the
`generic_c` backend (the behavioral oracle). It is the device-lifecycle analogue
of `docs/discussions/register-access/register-access-surface-convergence.md` and
is authored directly here (compiler repo) to avoid cross-repo history
fragmentation; the eval design analysis (`eval:
docs/constructs/device-lifecycle/`) cross-links it.

**Sources.** Eval design analysis `docs/constructs/device-lifecycle/{device-lifecycle.md,
-model.md, -dsl-surface.md, -design.md, -harness-redesign.md}`; the
GRILL/PRD/PLAN (`eval: .claude/specs/device-lifecycle/`, decisions D01–D18,
C01–C10, CR-1..CR-7, RD-1..RD-8); the prior taxonomy review
(`driver_lifecycle_taxonomy_review.md`). Every contract below cites an ID.

**Status: as-built, Stage-2 complete.** Branch `explore/device-lifecycle-lowering`,
commits `404612d` (M0/M1) → `afccc6f` (M8). Whole workspace `cargo test` green;
all 13 LC-* matrix rows covered.

---

## 0. Why converge (and the scope of the claim)

The claim is **feasibility + analytic generalization**, NOT proven completeness:
the construct's authored surface lowers, through the `generic_c` behavioral
oracle, to a *traced lifecycle* (registration → match → bring-up → callbacks →
teardown) that **composes with register-access** under a target-OS layout morph.
The oracle is the simulated OS: each managed step prints an `LC <event>` line;
that trace IS the behavioral contract (the `expected.stdout` half of the dual
oracle), and the emitted target-view C struct is the `golden.c` half.

**What is in scope (Stage-2, generic_c):** the typed match descriptor; the
5-callback all-optional `Lifecycle` trait; compiler-synthesized bring-up/teardown
glue for four canonical scenarios with init-failure rollback and
orderly-vs-surprise dispatch; user-data field-init conformance; the spec-view →
target-view driver-handle morph (Policy-P) that composes with register-access;
the D1/D2/D3 validation diagnostics. **Out of scope:** real `map_region`
base-binding (D15 — the register file is a global device model), arbitrary
harness scripting / per-call instance identity (RD-1, accepted loss), the
class-trait cross-check (deferred to the Device Class construct, D06).

---

## 1. The authored surface (the converged DSL)

Declared **inline per fixture** via attribute markers (CR-2), exactly as
register-access inlines `Mmio`/`Field` — the near-empty `crates/regseq` facade is
not the home this stage. The shared snippet is
`crates/codegen-coverage-runner/fixtures/_lifecycle_surface.rs`.

### 1.1 The driver struct + match descriptor — `#[rseq::driver(...)]`  [Contract]

```rust
#[rseq::driver(name = "e1000", pci = [{ vendor = 0x8086, device = 0x100e }])]
pub struct E1000 { /* author fields: user-data + Mmio<_> resources */ }
```

- `name` — the stable logical driver name (registration trace, symbols,
  diagnostics).
- Match sugar, one typed family (C03/D08): `compatible = ["vendor,device", …]`
  (devicetree), `pci = [{ vendor, device, [subsystem], [class] }]`,
  `amba = [{ id, mask }]`. Numeric literals accept hex/dec/bin/oct + `_`
  separators, range-checked to u16/u32.
- Deferred bus families `acpi`/`usb`/`i2c`/`spi` are a **loud parse rejection**
  (`BridgeError::DeviceLifecycleUnsupportedBus`, CR-5) — not a silent drop, not a
  validation diagnostic.

### 1.2 The device class — `#[rseq::device_class(<tag>)]`  [Contract: recognized; unvalidated]

A single bare identifier above the driver attr: `serial`/`network` map to typed
`ClassTag`s, any other ident → `Other(verbatim)`, absent → `Generic`. Recognized
and carried in the model; the class-trait cross-check is the Device Class
construct's job (D06/D09), so it is **unvalidated** here.

### 1.3 The `Lifecycle` trait — five optional callbacks (C09)  [Contract]

```rust
pub trait Lifecycle {
    fn on_init(&mut self) -> Result<(), RegseqError> { Ok(()) }
    fn on_deinit(&mut self) -> Result<(), RegseqError> { Ok(()) }
    fn on_surprise_remove(&mut self) -> Result<(), RegseqError> { Ok(()) }
    fn on_init_failure(&mut self) -> Result<(), RegseqError> { Ok(()) }
    fn on_system_shutdown(&mut self) -> Result<(), RegseqError> { Ok(()) }
}
```

All five have default no-op bodies, so a driver compiles with an empty
`impl Lifecycle for D {}` — or, at the bridge seam, **no `impl` at all**
(construct-level optionality, RD-3). A 6th method is a rustc `E0407` (no
construct diagnostic needed). `RegseqError` is an inline
`#[rseq::framework_type("regseq-error")]` enum with ≥2 variants (so
`Result<(), RegseqError>` is niche-encoded), errno-mappable
(`HardwareError`→EIO, `DeviceBusy`→EBUSY).

### 1.4 No harness verbs (RD-1).

There are **no** author `lifecycle::{bind,teardown,shutdown}` verbs and **no**
opaque `DriverHandle<D>`. The author writes only the driver + match descriptor +
the optional `impl Lifecycle`; the backend synthesizes the driving `main`. This
is a Stage-2 *realization* change only — the author-facing surface (the trait +
the descriptor) is unchanged, so adequacy/representability is unaffected.

---

## 2. The model IR (`crates/regseq-model/src/device_lifecycle.rs`)

A sibling table `DeviceLifecycleTables.drivers: BTreeMap<ItemId, DriverLifecycle>`
keyed by the driver struct's Item. Each `DriverLifecycle` = `{ descriptor, handle,
ops }`:

- `DriverDescriptor { name: StringId, match_set: Vec<MatchEntry>, device_class:
  ClassTag }`. `MatchEntry` is the closed 7-variant typed match
  (Devicetree/Pci/Amba modeled + parsed; Acpi/Usb/I2c/Spi modeled but rejected).
- `DriverHandle { resource_fields: Vec<ResourceField>, user_data_fields:
  Vec<UserDataField> }` — the spec-view field split.
- `LifecycleOps` — five `Option<InstanceRef>` (content-addressed, so they survive
  the Phase-4 instance sort). `Some` = overridden (the backend synthesizes a call
  to the materialized body); `None` = the trait default.

---

## 3. Recognition — item-driven (`regseq-driver/src/bridge/device_lifecycle/`)

Fully **item-driven** (RD-2): `#[rseq::driver]`/`#[rseq::device_class]` interning
mirrors `framework_type`; the `Lifecycle` trait `DefId` is recovered from each
driver's `impl` trait ref (not from a bind bound). `recognition.rs` parses the
raw tool-attribute token stream (the bracketed array/record sugar that
`meta_item_list` cannot handle); `resolve.rs` runs the override-check + field
classification.

- **No-impl split (RD-3, Codex B1):** field classification runs for EVERY
  `#[rseq::driver]`; callback resolution runs only for drivers with an
  `impl Lifecycle` (else `LifecycleOps::default()`). A no-impl driver is valid
  and flows through with its descriptor + classified fields intact.
- **User-data classification (T11/C10):** each non-`Mmio` field →
  Rule-2 `Option<T: Copy>` → `OptionCopy`, Rule-1 scalar whitelist
  (`bool`/`u8`/`u16`/`u32`/`u64`) → `ZeroInit`, everything else (signed/`u128`/
  `usize` ints, arrays, structs, `Option<non-Copy>`) → the **D3** diagnostic.
  Build-now is scalar-only (C10); the rejection is decided from the rustc type
  BEFORE `bridge::ty::translate_ty`, so a bad field is a clean D3, not a generic
  translator error.

---

## 4. The synthesized harness (RD-4/RD-6) + scenario selection (RD-5)

The `generic_c` backend (the simulated OS) **synthesizes the driving `main`** from
the model — `body_emit.rs::emit_lifecycle_synthesis`. For each driver it emits:

1. a per-driver **target-view container** struct (§7);
2. one **helper fn per selected scenario** (`__rseq_lc_<idx>_<label>_<scenario>`),
   each sequencing register → create → associate → preambles → `on_init` → the
   scenario's teardown/shutdown → postambles, against the model's tables;
3. a no-arg **runner** `__rseq_lc_run()` that calls the selected helpers.

`main` stays an `EntryShim` product (Codex B2/B3): it calls the author entry (if
any) then the runner — for an error-returning entry it returns on nonzero before
the scenarios; a lifecycle TU with no author `regseq_main` is valid (`main` = run
scenarios then `return 0`). **Byte-exactness (RD-7):** a TU with no lifecycle
driver emits today's `main` unchanged — generic-c emission is pure over
`(model, backend params)`.

**Scenario selection (RD-5):** the `backend.lifecycle.scenario=<csv>` compiler
param (default = all four), carried by a fixture's manifest `lifecycle_scenario`
and threaded through `cargo-regseq` + the codegen-coverage-runner.

---

## 5. The trace contract (C01) — the behavioral oracle

The canonical ordering (the contract; exact spellings blessed progressively per
OI-3, as-built shown):

```
LC register <name> match=<dt:…|pci:0x….:0x….|amba:0x…/0x…>
LC create
LC associate
LC preamble <res>                 # forward, per resource (bring-up acquire)
LC call on_init                   # ALWAYS (CR-3); author body invoked iff overridden
LC on_init -> Err                 # init-fail path only
LC call on_init_failure           # init-fail path, ALWAYS once it triggers (CR-3)
LC teardown present=<0|1>         # teardown dispatch marker
LC call on_deinit                 # present=1 (orderly), ALWAYS
LC call on_surprise_remove        # present=0 (surprise), ALWAYS
LC postamble <res>                # present=1, reverse (released)
LC postamble <res> skipped(surprise)   # present=0, reverse (HW gone)
LC call on_system_shutdown        # shutdown path, ONLY when overridden
```

The block above is the **union** of all lines; each scenario emits a **subset**:
*orderly/surprise* run the full bring-up + teardown-dispatch (preamble… on_init…
`teardown present=<1|0>`… `on_deinit`|`on_surprise_remove`… postamble[released|
skipped]); *init-fail* runs bring-up + on_init→`Err`→on_init_failure then clean
exit (NO teardown marker, NO postamble as-built); *shutdown* runs bring-up +
(iff overridden) `LC call on_system_shutdown` (NO teardown marker, NO postamble).

- **CR-3 (marker rule):** the four "glued" callbacks (`on_init`, `on_deinit`,
  `on_surprise_remove`, `on_init_failure`) ALWAYS emit their `LC call <cb>`
  marker — an absent override means the trait-default no-op ran, which still
  counts as "invoked". `on_system_shutdown` is the lone **omit-when-absent**
  callback (D10/I02): no marker, no slot, ABSENT in `golden.c` unless overridden.
- **CR-4:** a non-init callback's `Result` is discarded (best-effort teardown);
  the emitted C compiles and the process exits cleanly.
- Resource preamble/postamble are **trace-only** in the oracle (D15) — the
  register file is global; there is no per-instance base.

---

## 6. The four scenarios (as-built)

- **Orderly teardown (M2):** `present=1` + always-on `on_deinit` + reverse
  postambles (released). LC-7/LC-10.
- **Surprise removal (M4):** `present=0` + always-on `on_surprise_remove` +
  reverse postambles `skipped(surprise)`; byte-distinct from orderly. LC-11.
- **Init-failure rollback (M3):** the synthesized body captures `on_init`'s
  `Result`, reads its niche discriminant (reusing the errno exit-adapter
  machinery), and on `Err` emits `LC on_init -> Err` → `LC call on_init_failure`
  (+ invoke iff overridden) → clean exit. Asserted via the stdout trace, NOT a
  nonzero exit (GRILL D12). The reverse **partial postamble** (high-water-mark
  release of the succeeded preambles, D12) is **deferred as-built**: the init-fail
  fixture is fieldless, so there is nothing to roll back; the HWM release lands
  when a resourceful init-fail fixture is added (`emit_init_fail_body` does not
  yet thread `resource_names`). The init-fail path emits NO teardown-dispatch
  marker and NO postamble. LC-6.
- **System shutdown (M5):** `on_system_shutdown` register-or-omit (above). LC-8
  (pl011, omit) / LC-9 (e1000, present).

---

## 7. Policy-P morph (LC-12) — the CONTAINER realization + the two-view rule (CR-7)

Every lifecycle driver realizes in a **target-view container** that embeds the
spec driver and wraps it with the OS-managed metadata:

```c
struct __rseq_lc_<idx>_<label>_target {
    void     *__rseq_dev;     /* P1: prepended device handle (container_of in a real OS; NULL here) */
    struct input_<Driver> inner;   /* P4: the spec driver, embedded verbatim (Mmio kept as-is, P2) */
    uint32_t  __rseq_state;   /* P3: appended */
};
```

The synthesized `create()` builds the container (`= {0}`); **the callbacks run
SPEC-VIEW over `&inst.inner`** — a valid pointer to the embedded spec driver.
Per **CR-7 (the two-view rule)**: MIR/model TyIds + projections + register-offset
recovery (`recover_register`) stay **spec-view, untouched**; the target-view is
consulted only for the C aggregate emission. So the morph is **non-behavioral**:
the stdout trace is byte-IDENTICAL with/without it (verified — an empty `git diff
*/expected.stdout` after re-blessing), and the register write hits the **block
offset** (e.g. `regseq_mmio_write(0, 4, …)` for `dr` at offset 0 within the
register block), NOT the container/driver offset — proving register-access
composes through the handle morph (OI-1).

**Why container_of, not a flattened field relocation:** the embedding is the
standard way real Linux drivers wrap a driver in a framework-managed container,
and it realizes the two-view rule with **zero** changes to the callback ABI,
field projection, or `recover_register` (no shared-code-path edits → no
register-access regression). A flattened relocation (re-indexed target-view
callback bodies) was the original P1–P4 wording but is unnecessary unless the
thesis claims flattened target callbacks — the spec-view composition (M7-A)
already banks that. (Codex foundation review PROCEED-WITH-CARE → focused review
ACCEPTABLE-WITH-CHANGES endorsing the container morph.)

---

## 8. Diagnostics (`bridge/mod.rs::DeviceLifecycleDiagnostic`, I09)

A per-construct enum carried by `BridgeError::DeviceLifecycleValidation`:

- **D1 `MissingMatch`** — a well-formed `#[rseq::driver]` with no device-match
  attribute.
- **D2 `EmptyMatch`** — a match attribute present but empty (`compatible=[]`).
  D1/D2 split on a `match_attr_present` flag (set when a match bracket is
  consumed); a **malformed** match is still a silent drop, not D1/D2.
- **D3 `UserDataNotInitializable`** — a user-data field that is neither Rule-1 nor
  Rule-2 (§3).
- **CR-5** unsupported-bus sugar is a *separate*
  `BridgeError::DeviceLifecycleUnsupportedBus`, deliberately distinct from D1/D2/D3.

D4 is deferred. Runtime init-failure is a clean-exit trace path (§6), NOT a
diagnostic.

---

## 9. As-built — milestones, fixtures, coverage, results

**Milestones (branch `explore/device-lifecycle-lowering`):** M0 walking skeleton
+ M1 typed match (`404612d`); M2 orderly (`ad011a6`); M3 init-fail (`c0fbe3f`);
M4 surprise (`61827f0`); M5 shutdown (`67fc9e6`); M6 user-data + D3 (`d755b4c`);
M7-A register-access composition (`29fb454`); M7-B Policy-P container morph
(`8d6da9e`); M8 diagnostics + corpus green (`afccc6f`).

**Fixtures** (`crates/codegen-coverage-runner/fixtures/_lifecycle/`), each a
six-stage dual-oracle gate in a dedicated sole-test-per-binary process (D17 — the
emit's MIR is interner-history-sensitive, so the byte-snapshot is reproducible
only in a clean process):

| fixture | scenario(s) | covers | proves |
|---|---|---|---|
| lifecycle-fake | orderly | LC-1/2/5/7 | the walking skeleton + orderly teardown |
| lifecycle-initfail | init-fail | LC-1/2/6 | on_init Err → on_init_failure → clean exit |
| lifecycle-surprise | surprise | LC-1/2/7 | present=0 + on_surprise_remove, byte-distinct |
| lifecycle-pl011 | shutdown | LC-1/4/8 | AMBA match; shutdown OMIT (5-negative) |
| lifecycle-e1000 | shutdown | LC-1/3/9 | PCI match; shutdown PRESENT |
| lifecycle-handle | orderly+surprise | LC-1/10/11/12/13 | register-access composition + user-data zero-init + the morph |

Negatives: `lifecycle_diag` (D1/D2/D3 + CR-5 via `try_bridge_fixture_path →
BridgeOutcome::Rejected`, precise diagnostics). Coverage: `lifecycle_covers`
(every LC-1..13 row covered; every `covers` id present in the matrix — OI-4).
Bridge-unit: `regseq-driver/tests/bridge_device_lifecycle_recognition.rs` (match
parsing, override-check, user-data classification, no-impl optionality, D1/D2/D3).

**Representative trace** (lifecycle-handle, orderly — the composition + user-data
zero-init in one trace):

```
LC register handle match=dt:acme,handle
LC create / LC associate / LC preamble regs
LC call on_init
W32 @0x000 = 0x00000000        # the zero-initialized count, written to dr (C10 proof)
W32 @0x000 = 0x00000005        # count set, re-written (field read/write through self)
LC teardown present=1 / LC call on_deinit / LC postamble regs
```

The register offset `0x000` is `dr`'s offset **within the block**, recovered
unchanged despite `regs` sitting after `count` in the driver and despite the
container morph — the load-bearing OI-1 composition result.

---

## 10. Scope, caveats, regression guard

- **Accepted losses (RD-1, Q5):** arbitrary scenario scripting, repeated binds,
  per-call instance identity, interleavings. Acceptable for a behavioral oracle
  whose claim is the fixed canonical scenarios (feasibility + analytic
  generalization); no representability claim weakens.
- **Non-behavioral morph (CR-7):** the target-view is a `golden.c` representability
  artifact; the device pointer / state are unobservable in the oracle.
- **Register-access regression guard:** the morph touches NO register-access code
  (`recover_register`/`emit_mmio`/`layout_emit` unchanged); the
  `regaccess-{pl011,e1000,formb-ops}` proof fixtures were re-run green after M7-B.
- **Coverage ledger:** LC-* rows are kept `deferred` in the shared Doc-1 matrix; a
  dedicated lifecycle completeness test (not the cross-construct `ledger::build`,
  whose whole-corpus drift model would mis-flag the interner-isolated lifecycle
  fixtures) asserts the LC-* rows are fully covered.

---

## 11. The build

```
cd ~/Documents/sekekama/regseq-compiler-asplos   # toolchain nightly-2026-02-01
cargo build                                       # clean
cargo test                                        # all green
# lifecycle gates (each sole-test-per-binary):
cargo test -p codegen-coverage-runner --test lifecycle_e2e --test lifecycle_initfail_e2e \
  --test lifecycle_surprise_e2e --test lifecycle_pl011_e2e --test lifecycle_e1000_e2e \
  --test lifecycle_handle_e2e --test lifecycle_diag --test lifecycle_covers
# bridge-unit:
cargo test -p regseq-driver --test bridge_device_lifecycle_recognition
# re-bless a lifecycle golden: BLESS=1 cargo test -p codegen-coverage-runner --test <binary>
```
