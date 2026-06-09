# Bus Resource Management — converged DSL surface + as-built (eval ⇄ compiler)

The bus-resource-management construct (#3) is the **real-backend keystone**: it brings a device
up on its bus, claims/resolves its register region(s), and hands a real `(base, size)` to
register-access's `map_region` — retiring the *faked* global-regfile base the `generic_c` oracle
previously handed. This doc is the converged DSL surface (what the construct exposes) plus the
**as-built** realization across milestones M1–M6.

The eval-side design analysis is the authority on representability/adequacy
(`~/regseq-eval/docs/constructs/bus-resource-management/`); this doc is the compiler-side spec
contract (recognition, model IR, lowering, diagnostics, the trace oracle, the build).

**Status: as-built, M1–M6 complete.** Branch `construct/bus-resource-management` (off `main`).
Full workspace suite **green at 2206/0** (post-Stream-B harness work, §9.1; was 2185/0 at construct
landing); **zero golden churn** (the host bus model is gated, so a no-`bus` TU's `.c` is byte-unchanged).
The compiler-repo work is **uncommitted** as of writing.

---

## 0. Why converge (and the scope of the claim)

The claim is **feasibility + analytic generalization**, not proven completeness (per the eval
methodology). The construct is proven at the **generic-c host altitude** via a three-level method —
each milestone proves at the model/recognition/join level (unit tests), the backend level
(emission string-checks or `cc` compile+run witnesses), **and** — for the L2-binding spine — end-to-end
through the codegen-coverage-runner harness: the keystone fixture (§5, §9.1) drives `platform.toml` →
`parse_platform_*` → `regseq_l2::bind_all` → bound model → emitted C → `cc` compile+run with **no
hand-set binding**, so the declared `[mmio.<id>] base/size` is proven to *flow* unfaked file→C. The
**L2 parse→join→emit leg is therefore no longer deferred** (Stream B, §9.1). What remains
**Tier-2-deferred** is the real `cargo-regseq` → `cc` first-light milestone — the real facade + MIR +
**real PCI enumeration** (which the generic-c host fakes) — out of scope here (the same deferral
register-access/device-lifecycle took for their composed fixtures).

---

## 1. The authored surface (the converged DSL)

The author surface is deliberately tiny (R01–R03): one item-attr key, two args on register-access's
existing field attr, and two free-function intrinsics. Everything else is compiler-managed glue.

### 1.1 The bus — `#[rseq::driver(bus = "…")]`  [Contract]
```rust
#[rseq::driver(name = "e1000", bus = "pci", pci = [{ vendor = 0x8086, device = 0x100e }])]
struct E1000 { /* … */ }
```
- `bus` selects the per-target framework path and **gates the PCI-only surface** (config intrinsics,
  `bar`, `type = "io"`, `enable_device`/`set_bus_master`). The token maps to a closed, append-only
  `BusType` (`pci`/`platform`/`amba`/`i2c`/`spi`/`usb`, else `Other`). `amba` is **distinct** from
  `platform` (GRILL D07: Linux lowers it to `amba_driver` + periphid, not `platform_driver`).
- `bus` is **optional** at recognition (an absent `bus` ⇒ `bus_type = None`): requiring it globally
  would break existing register-access/device-lifecycle fixtures. An **unknown** token (`Other`) is
  rejected by the surface validation (§8). The match sugar (`pci`/`amba`/`compatible`) is
  device-lifecycle's, unchanged.

### 1.2 The BAR / carrier — `#[rseq::mmio(id, bar?, type?)]`  [Contract]
```rust
struct E1000 {
    #[rseq::mmio(id = "regs", bar = 0)]      // L1-preferred BAR (O4(b)); type defaults "mem"
    regs: Mmio<e1000::Regs>,
}
```
- bus-resource **contributes** the `bar` (default-decoded BAR index, 0..5, PCI-like only) and
  `type = "io"` (port-I/O carrier, PCI-like only) args; register-access **owns** the `#[rseq::mmio]`
  attribute + the `Mmio<T>` resource. `bar` is **L1-or-L2, L1-preferred** (O4(b)/A01, see the
  register-access surface doc §2.3): **effective BAR = `L2.bar ?? L1.bar`**. `base`/`size` are L2-only.

### 1.3 The L2 binding — `[mmio.<id>]` in `platform.toml`  [Contract]
```toml
[mmio.regs]                # key `regs` ↔ #[rseq::mmio(id = "regs")]
bar  = 0                   # optional, PCI-like only (L2 supply/override of L1)
base = 0xf0000000          # optional; on generic-c the host literal (GRILL D06)
size = 0x20000             # optional; binds WITH base (I02)
type = "mem"               # optional ("mem"|"io"), PCI-like only; overrides L1 carrier
```
No new `[bus]`/`[bus_resource.*]` section — bus-resource owns the `bar`/`base`/`size`/`type` keys of
register-access's `[mmio.<id>]` section. `base`/`size` are present-together-or-absent-together (I02).

### 1.4 Config-space intrinsics — `regseq::bus::{read_config, write_config}`  [Contract]
```rust
let cmd: u16 = regseq::bus::read_config::<u16>(PCI_COMMAND);   // thin: PCI_COMMAND + PCI-ID caching
regseq::bus::write_config::<u16>(PCI_COMMAND, cmd | 0x4);      // recognized + lowerable (no witness, O3)
```
- PCI-conditional free functions (`BRM-001`). `read_config` is **thin** (the PCI_COMMAND witness +
  the **PCI-ID-caching** rule: the vendor/device id is read once and cached). `write_config` is
  **recognized + lowerable** with **no behavioral witness** (GRILL O3).

The four compiler-managed ops (§4) and `get_resource_handle` are **synthesized**, not authored (R04).

---

## 2. The model IR (`crates/regseq-model`)

- **`device_lifecycle.rs`:** `DriverDescriptor.bus_type: Option<BusType>` (+ `BusType::is_pci_like`,
  only `Pci` today) and `DriverDescriptor.dma_present: bool` (the `set_bus_master` second gate — a
  **placeholder** defaulting `false` until the DMA construct defines it, GRILL O2). Per region,
  `ResourceField.binding: MmioBinding { id, bar_index, bar_type, effective_bar, base, size }`
  (+ `is_resolved()` = base ∧ size) and `BarType { Mem, Io }`. All new serialized fields
  `#[serde(default)]` (pre-construct-#3 models still deserialize).
- **`domain_attr.rs`:** `IntrinsicKind::Config(ConfigOp { Read, Write })` **appended at discriminant 3**
  (Log=0/Mmio=1/PollUntil=2 unmoved); `from_name` maps `read_config`/`write_config`.

---

## 3. Recognition (`regseq-driver/src/bridge/`)

- `device_lifecycle/recognition.rs::parse_driver_args` parses `bus = "…"` → `bus_type`.
- `device_lifecycle/recognition.rs::recognise_mmio_field_attr` parses `bar`/`type` onto the
  `MmioBinding` (the `bar`/`type` **bus-gating** — BRM-002/003 — is the surface validation in §8, not
  recognition).
- `intrinsic/recognition.rs` maps `read_config`/`write_config` → `IntrinsicKind::Config` (the shared
  per-callee `#[rseq::intrinsic("…")]` path).
- **Join** (`bus_resource.rs::join_platform_bindings`, post-bridge, NOT rustc-driven): joins
  `[mmio.<id>]` (L2) to each MMIO resource field by `id` (fallback: field name); computes
  `effective_bar = L2.bar ?? L1.bar` and surfaces `(base, size)` + `bar_type`. A malformed platform
  file is fatal; an absent/unreadable one is non-fatal (no bindings → the faked path).

---

## 4. The four compiler-managed ops + the gating matrix (M3, R04 — as-built)

Synthesized into device-lifecycle's init preamble (after `register → create → associate`, before
`on_init`), in **canonical order** `enable_device → claim_regions → set_bus_master →
get_resource_handle`, then register-access's `map_region`. Emitted by
`generic_c/body_emit.rs::emit_lifecycle_synthesis`, gated on the driver having ≥1 resolved binding.

| Op | Gate (as-built) |
|---|---|
| `enable_device` | PCI-capable bus (`bus_type.is_pci_like()`) — absent on platform/amba (default-on) |
| `claim_regions` | per region-bearing binding, on **every** bus (NOT PCI-gated); host = trace-only no-op |
| `set_bus_master` | PCI-capable **AND** `dma_present` (placeholder ⇒ emitted by no fixture today) |
| `get_resource_handle` | **unconditional** (captures the bus-handle context, I06) |

`claim_regions`' three realizations (explicit `pci_request_selected_regions` / fused
`devm_ioremap_resource` / no-runtime Zephyr) are **backend-[Open]** (design §3/§8.2); generic-c emits
the host no-op trace. The `enable_device`+`set_bus_master` Zephyr fusion is likewise backend-[Open].
The single-global-mapped-base guard (`emit_generic_c_inner` rejects >1 resolved region per driver)
keeps `map_region`'s base unambiguous.

---

## 5. The keystone mapping slice + the bus-handle context (M1/M2)

```
get_resource_handle ─→ resolve (base,size) per effective key ─→ map_region ─→ Mmio<T> read/write
```
The resolved `(base, size)` (the `[mmio.<id>]` literal on generic-c, GRILL D06) feeds register-access's
`map_region` with **no register-access op change** when the source swaps (host literal ↔
`pci_resource_start`/`DT_REG_ADDR`) — I04. The based accessors (`regseq_bus_mmio_read/write`) trace
the **absolute mapped address** (`base + off`), retiring the faked base. The L2 `bar` override is
provable end-to-end (M2): `map_region` records the effective BAR (`bar = -1` ⇒ no BAR / platform-DT).

---

## 6. The trace contract (the behavioral oracle, generic-c host)

A bus-managed driver's bring-up trace (PCI, DMA-absent, single resolved region):
```
LC register "<name>" "<match>"
BUS register_driver path=pci binding=bound        # M6/T18 — the bus-path selector (DL emits register; BRM supplies the selector)
LC create
LC associate
LC preamble "regs"
BUS enable_device                                  # M3 — PCI-only
BUS claim_regions bar=0                            # M3 — every bus
BUS get_handle                                     # M3 — unconditional
BUS map base=0xf0000000 size=0x20000 bar=0         # keystone — the REAL declared base
R32 @0xf0000008 = …                                # based accessor — absolute mapped address
```
A non-PCI (`amba`) DT region degrades cleanly: `claim_regions bar=-1` + `get_handle` + `map … bar=-1`,
with **no** `enable_device`/`set_master` (asserted against the emitted C, not stdout silence). Config:
`BUS config_read off=0x4 width=2 = …` (PCI_COMMAND), `BUS config_read off=0x0 …` then
`BUS config_read id_cache off=0x0 …` (the caching rule), `BUS config_write …`.

---

## 7. Config-space intrinsics (M5 — as-built)

`call()` (`body_emit.rs`) intercepts a Direct callee carrying `IntrinsicKind::Config(op)` and routes
to `emit_config`: `read_config` → `dst = (cty)regseq_bus_on_config_read((uint32_t)(off), width)`,
`write_config` → `regseq_bus_on_config_write((uint32_t)(off), width, (uint64_t)(val))`; the width is
`size_of::<T>()` recovered from the destination (read) / value parameter (write) via
`callee_param_tys` (so a constant value types correctly), the offset is `args[0]`. `T` must be a
**scalar integer/bool of width 1/2/4/8** (`config_access_width` — the host model is `uint64_t`-backed;
a `u128`/aggregate `T` fails loud rather than emit a ≥64-bit shift / invalid cast); a missing
target/offset/value also fails loud. The host config block is emitted whenever a config intrinsic is
present (the gate includes `model_has_config_intrinsic`), so the lowered hooks are always defined. The host model (`prelude.rs::BUS_RESOURCE_HELPERS`) answers config reads, implements the
**PCI-ID-caching** rule (the vendor/device id at offset 0x00 is read fresh once then served from
cache) and the **PCI_COMMAND** witness; `write_config` records into config space (no witness, O3).
The PCI-gate (BRM-001, reject on non-PCI) is **deferred** (§8). The full Rust-source → `read_config`
→ `emit_config` exercise is **Tier-2-deferred** (intrinsic call emission is fixture/e2e-tested, like
mmio/diag-log).

---

## 8. Diagnostics — `bus_resource.rs::validate_bus_surface` (M6, T19/T20)

Runs over the joined L1+L2 view (post-join, in `callbacks.rs`), for **bus-declared drivers only**
(no-`bus` drivers are never affected — regression-proof). Returns the first `BrmError` (the bridge
surfaces it as fatal). **Enforced** (each unit-tested in `bus_resource.rs::tests`):

| Code | Trigger |
|---|---|
| **BRM-002** | `bar` (L1 or L2) on a non-PCI bus |
| **BRM-003** | `type = "io"` on a non-PCI bus |
| **BRM-005** | a region with exactly one of `base`/`size` (they bind together, I02) |
| **BRM-006** | a bus-managed region with no resolvable `(base, size)` on the host (D06 / I01 rejection leg) |
| *unknown bus* | a `BusType::Other` token (the non-enum `bus` value of R01/T02 — a generic recognition error, **NOT** an invented BRM code) |

**Deferred / frozen (recorded, not fabricated):**
- **BRM-001** (config intrinsic on non-PCI) — needs call-site → driver linkage (the config intrinsic is
  recognized on the facade fn, not the call site); deferred with the composed-fixture pipeline.
- **BRM-004** — the **frozen** surface-boundary diagnostic (`resource_name`/`bus_device_handle`
  resurfacing); no trigger surface exists in the build-now DSL, so it is recorded as
  **frozen-but-untestable** rather than given a fabricated contract (per the plan's T20 risk).
- **`BRM-007`** (missing/invalid `bus`) — a taxonomy-amendment candidate for when v3 unfreezes; the
  unknown-bus leg is surfaced as a generic recognition error meanwhile.

---

## 9. As-built — milestones, tests, coverage

| Milestone | Scope | Proof (generic-c altitude) | Codex |
|---|---|---|---|
| **M1** keystone | `(base,size)` → `map_region`, faked base retired | backend emission + `cc` compile+run + **harness e2e** (§9.1) | SOUND |
| **M2** BAR override | `effective_bar = L2.bar ?? L1.bar` observable | join + backend tests | SOUND |
| **M3** preamble + gating | four ops, canonical order + gating matrix | backend order/gating tests | SOUND |
| **M4** non-PCI degrade | amba no-BAR resolves; enable/master absent in emitted C | join + backend tests | SOUND (no findings) |
| **M5** config intrinsics | `IntrinsicKind::Config`, `emit_config`, host model | model + recognition + `cc` caching witness | SOUND (r2; r1 fixed a `write_config` constant-width bug) |
| **M6** registration + diagnostics + O4(b) | selector, BRM-002/003/005/006 + unknown-bus, O4(b) | backend selector test + 6 validation tests | — |

Tests: **11** `bus_resource_*` backend tests (`generic_c/mod_tests.rs`, the fast hand-set-binding
matrix — see the bank header there for the hand-set ⇄ fixture split) + the join/validation suite in
`regseq-l2/src/bus_resource.rs::tests` (join + BRM; extracted from `regseq-driver` into the light
`regseq-l2` crate by Stream B, §9.1) + model `config_intrinsic_*` + recognition
`recognises_config_space_intrinsics` + the harness end-to-end tier (§9.1).

### 9.1 Harness end-to-end coverage (Stream B — L2 binding through the runner)

The L2-binding spine is no longer verified only in disconnected halves (parse unit-tested in
`regseq-config`, join unit-tested in `regseq-l2`). Production already binds backend-neutrally
(`callbacks.rs`: `parse_platform_file_path` → `regseq_l2::bind_all` → `regseq_l2::validate_l2_surface`
→ re-`validate`, **before any backend is selected**); Stream B makes the **codegen-coverage-runner**
harness perform the same bind, so the **bound `RegseqModel`** is the verified artifact and the backend
goldens capture L2 correctness for free (backend-neutrally extensible to Zephyr/Linux).

- **Crate move (T0):** `join_platform_bindings` + `validate_bus_surface` + `BrmError` moved out of the
  rustc-private `regseq-driver` into a new light crate **`regseq-l2`** (`regseq-model` + `regseq-config`
  only), behind an extensible per-construct `L2Binder` dispatch (`bus-resource` is the first tenant;
  interrupts/clocks/DMA grow the registry). Both production and the runner depend on it; no Cargo cycle.
- **Keystone fixture (T3) — the parse→bind→emit proof:**
  `codegen-coverage-runner/fixtures/_bus/brm-e1000-keystone/` (`input.rs` + `platform.toml` +
  `golden.c` + `expected.stdout`), run by `tests/bus_keystone_e2e.rs`. A PCI driver
  `#[rseq::driver(bus="pci")]` + `#[rseq::mmio(id="regs", bar=0)]` whose `platform.toml` supplies
  `[mmio.regs] bar=0, base=0xf0000000, size=0x20000`; the emitted C carries
  `regseq_bus_map_region(0xf0000000ull, 0x20000ull, 0)` and the behavioral stdout shows the based
  access `W32 @0xf0000000` / `R32 @0xf0000008` — **with no hand-set `MmioBinding`**, proving the literal
  flows `platform.toml` → bound model → emitted C. (NB the same-named in-file
  `bus_resource_keystone_maps_the_real_declared_base` is the *hand-set* variant — it proves the emitter
  *consumes* the declared base; this fixture proves the base *arrives* there.)
- **BRM compile-fail oracles (T4/T5):** the runner manifest gained an `expect="reject"` shape
  (`[expected] stage="bind" diagnostic="BRM-0NN"`); two committed `_bus/` reject fixtures pair the
  unit-tested diagnostics with the harness end-to-end — `brm-halfspec-reject` (`base` without `size` →
  **BRM-005** at bind) and `brm-amba-bar-reject` (amba + `bar` → **BRM-002** at bind) — asserted GREEN
  by `tests/bus_reject_corpus_e2e.rs` (+ `bus_bind_reject_e2e.rs` for the verdict-level legs).
- **Platform identity (T7):** the keystone's `platform.toml` also carries a `[platform]` block; identity
  (`os`/`board`/`arch` → `PlatformIdentity` → `BackendContext`) flows file → backend, traced as a gated
  one-line C comment (generic-c ignores it; for the future Zephyr/Linux framework selection). The
  `[platform]` table is now the **single** source of target identity — the redundant `--target-os` CLI
  flag was hard-removed (see the L2-platform-binding plan, decisions D4–D8).

---

## 10. Scope, caveats, regression guard

- **Zero churn (regression guard):** `BUS_RESOURCE_HELPERS` is emitted only when a driver has a
  resolved binding, declares a `bus`, **or** uses a config-space intrinsic; **no generic-C golden /
  corpus fixture meets any of those**,
  so no register-access/diag-log/lifecycle generic-C golden changes. (The `pl011-full-spec` fixture
  *does* declare `bus = "amba"`, but it is a **model/IR** fixture — `expected/regseq-model.json` + `ir`,
  no generic-C golden — so the gate broadening cannot churn it; and `validate_bus_surface` passes it:
  amba, a resolved `(base,size)` region, no `bar` ⇒ no BRM.) Full suite **2206/0** (post-Stream-B; was
  2185/0 at construct-#3 landing — the delta is the new `regseq-l2`/harness tests, not golden churn).
- **Now covered (was Tier-2 at construct-#3 landing):** the L2 parse→join→emit *binding* pipeline is
  harness-end-to-end via the keystone fixture (§9.1) — `platform.toml` → bind → emitted C → `cc`, no
  hand-set binding — and the BRM compile-fail leg via the `expect="reject"` corpus.
- **Tier-2-deferred (still out of scope):** the **real `cargo-regseq` → `cc`** first-light run — the
  real facade + MIR + **real PCI enumeration** (the generic-c host fakes the enumeration); the full
  Rust-source → `read_config`/`emit_config` config-intrinsic exercise (the keystone drives MMIO, not
  config — §7); the `set_bus_master` DMA-presence inference (until the DMA construct, GRILL O2); the
  per-target framework calls + Zephyr fusion + claim-realization dispatch (backend-[Open]); a live QEMU bind.
- **A01 (O4(b)) is assumed, not ratified** — the E1000 L1 `bar` migration is reversible (the
  L2-supplies form still works).

---

## 11. The build

```bash
# always pin the toolchain (the compiler repo is nightly-2026-02-01):
cargo +nightly-2026-02-01 test --manifest-path <repo>/Cargo.toml            # full suite (2206/0)
cargo +nightly-2026-02-01 test --manifest-path <repo>/Cargo.toml -p regseq-backends bus_resource   # 11 backend tests
cargo +nightly-2026-02-01 test --manifest-path <repo>/Cargo.toml -p regseq-l2 bus_resource         # join + BRM validation (moved out of regseq-driver, §9.1)
cargo +nightly-2026-02-01 test --manifest-path <repo>/Cargo.toml -p regseq-model config_intrinsic
cargo +nightly-2026-02-01 test --manifest-path <repo>/Cargo.toml -p codegen-coverage-runner --test bus_keystone_e2e   # harness parse→bind→emit e2e (§9.1)
```
Key locations: model `regseq-model/src/{device_lifecycle,domain_attr,target}.rs` (`target.rs`:
`TargetOs`+`PlatformIdentity`); recognition `regseq-driver/src/bridge/{device_lifecycle,intrinsic}/recognition.rs`;
join + validation + L2 dispatch `regseq-l2/src/{bus_resource,lib,error}.rs` (moved out of `regseq-driver`,
§9.1); platform parse `regseq-config/src/platform.rs` (`parse_platform_file`); host model +
trace hooks `regseq-backends/src/generic_c/prelude.rs::BUS_RESOURCE_HELPERS`; synthesis + `emit_config`
`regseq-backends/src/generic_c/body_emit.rs`; gate + `BackendContext`/`platform_trace_comment`
`regseq-backends/src/generic_c/{mod,backend_options}.rs`; harness bind seam + fixtures
`codegen-coverage-runner/src/pipeline.rs` + `fixtures/_bus/`.
