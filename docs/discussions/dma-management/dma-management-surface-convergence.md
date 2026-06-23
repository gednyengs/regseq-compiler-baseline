# DMA Management — converged DSL surface (eval ⇄ compiler)

> **Home:** this is the **final spec contract** for dma-management, co-located with
> the implementation in this repo (`docs/discussions/dma-management/`). The
> evidence-grounded **design analysis** (adequacy, model, DSL-surface, design, the
> OS-agnostic naming amendment) lives in the eval repo at
> `regseq-eval/docs/constructs/dma-management/`; filename references below to those
> analysis docs resolve there. The Stage-B decision record (GRILL), PRD, and PLAN
> live in eval at `…/.claude/sekekama/grill-to-plan/specs/dma-management/`.

**Date:** 2026-06-23 · **Status:** ✅ **LOWERING CONFIRMED (generic-c, candidate-prototype)** — the
**E1000 coherent half** (`DmaRegion<T>` + A2 + `element_access` + the store→doorbell fence) **and** the
**PL011 channel half** (slave `DmaChannel` 5-op transport + `span` endpoint + completion registration) lower
through the live generic-c backend, compile with `cc`, and run with correct behavioral/codegen-shape traces;
**streaming** (`dma::stream`/acquire/release/RAII-unmap) lowers as a gc-shape oracle. M1–M3 committed at
`cedf072`; **M4** (the C03 layout-verify rule with the real `DmaPod` bit-validity predicate, streaming emit,
the capability-gating contract, the `DMA-ERR-*` diagnostic family) **+ M5** (the Doc-1 `DMA-1..16` matrix rows,
the `dma_covers.rs` honest-coverage gate, this collateral reconciliation) are built + green but **uncommitted**.
Full suite **2757 passed / 0 failed / 12 ignored**. Plan was Codex-reviewed (READY, 2-round); the M4 milestone
review returned **UNSOUND → all 3 blockers + 2 hardening folded** (the `DmaPod` bit-validity fix, the non-const
index reject, fail-closed). **Maturity: `candidate-prototype`.**

**Purpose:** pin the **converged, OS-agnostic L1 DSL surface** + the IR-survival, L2-schema, generic-c-lowering,
verify-rule, and ordering **contracts** the compiler honors for dma-management, and record the **as-built** scope
**honestly** — every designed-not-built surface is labeled as such, matching the `dma_covers.rs` partition
(§9.3). This doc supersedes nothing in the eval **model** (`dma-management-model.md` is unchanged); it is the
contract the eval design analysis cross-links.

---

## 0. Why this construct (and what it owns)

The frozen taxonomy's `dma-management` surface was **operation-less** on the coherent `DmaRegion<T>` resource
and carried **OS-leaking (Linux-API-derived) names**; neither E1000's coherent-ring data path nor PL011's RX
coherent-buffer transfer was faithfully expressible. This construct adds:

1. **A2 addressing/access** on the coherent region — `device_addr`/`device_addr_of`/`element_count` +
   `element_access` (the closure-scoped typed borrow), the **true surface additions** (FIX1 A2), two-witnessed
   in-corpus (E1000 ring + PL011 RX).
2. **OS-agnostic names** (the C08 map, GRILL D04) — the construct's core thesis claim; no Linux spellings on L1
   (`acquire_from_device`/`release_to_device`, not `sync_cpu`/`sync_device`; `stage_transfer`/`start_transfer`,
   not `prepare`/`submit`; `device_address_bits`, not `dma_mask`; `endpoint_*`, not `slave_*`).
3. **The DMA↔MMIO ordering obligation** — the `dma_mmio_store_order` fence (store→doorbell), **owned by
   dma-management** (register-access emits only the plain doorbell; dma-management inserts the fence between).

**Scope boundary held** (GRILL Out of Scope): descriptor **publish/harvest** → `device-command-transport`;
descriptor **wire format**/phase-bit/SGL → device-class `#[repr(C)]` type `T`; device **doorbell** →
register-access; ring producer/consumer indices → host/device-class; **raw device IRQ** → future interrupt
construct; `max_segment_size`/count → Track-B governance.

---

## 1. Model fidelity & L1/L2 separation (the non-negotiables)

This surface realizes the eval `dma-management-model.md` + GRILL exactly; only the *spelling* is the C08
OS-agnostic map. **L1 stays OS-neutral** (the lowering absorbs divergence); the build is **generic-c only /
candidate-prototype** this cycle.

| Layer | Lives in | Carries |
|---|---|---|
| **L1** (device-invariant) | the driver `.rs` spec | `#[rseq::dma_region(id, element_size, element_count, alignment?)]` + `DmaRegion<T>`; `#[rseq::dma_channel(id, direction)]` + `DmaChannel`; `dma::stream` + `DmaStreamBuffer`; the A2 ops; `element_access` verbs (+ per-call reinterpret `V`); the channel 5-op transport; `region.span(range)`; `#[rseq::dma_completion(channel)]` registration |
| **L2** (per-integration) | the platform file (`platform.toml`) | `[dma.<id>]` — region `{device_address_bits, alignment?, region_cache_maintenance}` + channel `{endpoint_addr_width, endpoint_max_burst_beats, binding_scope, cap_*}`, keyed by the resource id. Joined to L1 by the `regseq-l2` `dma` tenant. |

**Two distinct gates (Codex R1#4):** `dma_present` = **bus-master only** (E1000 coherent/streaming on PCI),
gates `regseq_bus_set_master`. The **host-helper emission gate** = "driver has ANY DMA resource
(region | channel | stream)", gates the `DMA_HELPERS` prelude. PL011 (slave) ⇒ `dma_present = false` BUT the
host helpers ARE emitted. **Never the same flag** (`device_lifecycle.rs` `dma_present` vs
`model_has_dma_resource` in `generic_c/body_emit.rs`).

---

## 2. The converged L1 DSL surface (C10) — OS-agnostic names

### 2.1 Coherent `DmaRegion<T>` + A2 addressing + `element_access`  [Contract]

```rust
struct E1000Dma {
    #[rseq::dma_region(id = "tx_ring", element_size = 16, element_count = 256)]   // alignment? omitted ⇒ align_of::<T>()
    tx_ring: DmaRegion<LegacyTxDesc>,                                              // T = a #[repr(C)] descriptor
}

// A2 addressing/introspection (FIX1 A2):
let base = self.tx_ring.device_addr();          // -> device base
let a    = self.tx_ring.device_addr_of(0);      // -> base + i*element_size
let n    = self.tx_ring.element_count();         // -> the declaration const

// element_access: a CLOSURE-SCOPED typed borrow (D01) — read/write/update; the
// per-call reinterpret view V rides the turbofish (V = T by default):
self.tx_ring.write::<LegacyTxDesc>(0, |d: &mut LegacyTxDesc| {
    d.buffer_addr = PKT_BUF_DMA_ADDR;
    d.lower = TXDESC_LOWER;
});
```

- **`element_access` is a closure-scoped borrow** (D01): `read`/`write`/`update(i, |&[mut] V| …)`, giving the
  compiler a **lexical** bracket for the ordering/cache obligation rather than a returned guard whose Drop point
  must be inferred. The closure is a higher-ranked borrow that must not escape.
- **The access mode is the explicit verb** (D02), never inferred from `&V` vs `&mut V`; the IR carries the
  **two-bit effect carrier** `{consumes_device_data, publishes_cpu_data}` derived from it
  (`read={1,0}` · `write={0,1}` · `update={1,1}`).
- **A2 ops are NOT `dma_present` triggers** — `device_addr`/`device_addr_of`/`element_count` are addressing, not
  bus-mastering facts.
- **mandatory attribute keys** = `id`/`element_size`/`element_count`; `alignment` is optional (omitted ⇒
  `align_of::<T>()`, **NOT 1**, D08).

### 2.2 Slave `DmaChannel` + the 5-op transport + lifecycle  [Contract]

```rust
struct Pl011Dma {
    #[rseq::dma_channel(id = "rx", direction = "device_to_host")]   // mandatory id + direction
    rx: DmaChannel,
    #[rseq::dma_region(id = "rx_buf", element_size = 1, element_count = 64)]
    rx_buf: DmaRegion<u8>,
}

fn on_init(&mut self) -> Result<(), RegseqError> {
    self.rx.configure();                                  // parameterless (config = declaration/L2-derived)
    self.rx.stage_transfer(self.rx_buf.span(0..32))?;     // endpoint = DmaRegionSpan | &DmaStreamBuffer
    self.rx.start_transfer()?;                            // merges submit + issue-pending
    Ok(())
}
fn on_deinit(&mut self) -> Result<(), RegseqError> { self.rx.stop(); Ok(()) }   // stop() infallible
```

Op set (the C08 names; the old Linux spellings in parentheses):
- `configure()` — parameterless.
- `stage_transfer(endpoint) -> Result` (was `prepare`) — `endpoint: impl TransferEndpoint`
  (`DmaRegionSpan` | `&DmaStreamBuffer`); `NULL` ⇒ `Err`.
- `start_transfer() -> Result` (was `submit` + `issue_pending`) — merges submit + issue-pending;
  `dma_submit_error` ⇒ `Err`.
- `stop()` — **infallible** (void).
- `remaining_bytes() -> Result<usize>` (was `query_residue`) — `-ENOSYS`/no-meaningful-residue ⇒ `Err`.

**Direction (D05/C06):** the endpoint's source/destination role is fixed by the **channel's declared
direction**, NOT the span (the span is direction-neutral): a `device_to_host` channel's coherent span is the
**destination**. **Lifecycle:** `request_channel`/`release_channel` are **compiler-synthesized** (bound to
device lifetime, LIFO teardown), never authored.

### 2.3 Streaming `DmaStreamBuffer` + RAII unmap  [Contract]

```rust
let sb = dma::stream(buf, flow)?;       // -> Result<DmaStreamBuffer, RegseqError> (was dma::map; encodes mapping-error)
let addr = sb.device_addr();
sb.release_to_device();                 // ownership → device (was sync_for_device)
sb.acquire_from_device();               // ownership → CPU    (was sync_for_cpu)
// RAII unmap at scope exit (was DmaMapping Drop)
```

- `dma::stream(buf, flow) -> Result<DmaStreamBuffer>` (free function); the `Result` is `#[must_use]` +
  lint-enforced (§5.2).
- `device_addr()`, `acquire_from_device()` / `release_to_device()` = the ownership transitions.
- **RAII unmap on Drop** — see §5.1 for the as-built realization (scope-exit cleanup; the IR carries no Drop
  terminator).

### 2.4 `region.span(range)` → `DmaRegionSpan` (the stage endpoint, provisional)  [Contract]

```rust
self.rx.stage_transfer(self.rx_buf.span(0..32))?;   // element-indexed, non-borrowing, direction-neutral
```
`span(range)` returns a **non-borrowing**, **direction-neutral** `DmaRegionSpan { device_addr = base +
start*element_size, len = range.len()*element_size, provenance = region }`. The range is **element-indexed**;
for `DmaRegion<u8>` elements == bytes (PL011). The **bare `stage_transfer(&DmaRegion<T>)` is a forbidden
anti-form** (§5.3). Marked **provisional** (single PL011/Linux witness, D05).

### 2.5 `#[rseq::dma_completion(channel)]` — completion registration (re-admitted)  [Contract]

```rust
#[rseq::dma_completion(channel = "rx")]
fn rx_complete(&mut self, result: DmaCompletion) -> Result<(), RegseqError>;
```
dma-management owns the **DMA-framework completion-callback registration** (the generated
`regseq_dma_register_completion`); the handler body is device-class logic; raw device IRQs stay with the future
interrupt construct. **Build scope this cycle (D06): recognition + IR binding + registration codegen-shape only
— the host model NEVER fires the callback** (delivery is designed-not-built, §9.3). The bound `channel` MUST
name a declared `#[rseq::dma_channel(id)]` on the owning driver (a dangling completion is rejected,
`DMA-ERR-UNKNOWN-COMPLETION-CHANNEL`).

---

## 3. IR survival contract (C09)

Must survive into the model (`crates/regseq-model/src/dma.rs` — the `DmaTables`/`DriverDma` sibling table,
keyed by the `#[rseq::driver]` struct's `ItemId`, the clock/pinctrl mirror; `#[serde(default)]` keeps pre-DMA
models deserializable):

- **`DmaRegionFacts`** — `{id, element_size, element_count, alignment (= align_of::<T>() if omitted),
  element_type T, element_layout: DmaLayoutFacts{size, align, is_pod}, element_accesses, binding}`.
- **`ElementAccessFact`** (the C01 carrier, per `element_access` site) — `{region, index?, view_type V,
  view_layout, verb, effects{consumes_device_data, publishes_cpu_data}, lexical_extent}`.
- **`DmaChannelFacts`** — `{id, direction, binding}`.
- **`DmaStreamFacts`** — `{flow, lifetime: DmaStreamBuffer{extent}}` (the RAII lifetime carrier).
- **`DmaRegionSpan`** at `stage_transfer` sites — `{device_addr: SpanAddr{element_offset, element_size}, len,
  provenance}`.
- **`CompletionBinding`** — `{channel, handler, payload}` (registration metadata; never delivered this cycle).
- **`StoreOrderEdge`** — `{region, region_store_op, mmio_write}` AS CONCRETE ENDPOINTS (the publishing region
  store + the MMIO write it publishes).
- **the field markers** in `domain_attrs.field_attrs` / `item_attrs` — `AttrData::{DmaRegionResource,
  DmaChannelResource, DmaCompletionBinding}` (append-only discriminants 9/10/11 after `PinStateMarker = 8`);
  `IntrinsicKind::Dma(DmaOp)` (append-only discriminant 9) with the full op vocabulary in `from_name`.

**Note on `DmaLayoutFacts.is_pod`:** the M1 value is the conservative `!needs_drop` PROXY; the **authoritative**
`DmaPod` predicate is recomputed from the rustc `Ty` in the M4 verify pass (§5.1) — the stored proxy is not the
contract. **Note on `view_type`:** the M1 `view_type`/`view_layout` carrier is recorded from the FIRST callee
type-arg (best-effort = the declared `T`); the C03 verify reads the reinterpret `V` independently off the
closure rust-call signature (§5.1), so the recorded `view_type` is NOT load-bearing for the verify.

**Stays backend (open):** exact per-target API selection; the Zephyr stateful-config accumulation; the precise
closure-vs-guard codegen; whether `device_addr_of(i)` is inline arithmetic or a hook;
the exact `StoreOrderEdge` last-publishing-store trigger (over-fenced conservatively this cycle, §5.4).

---

## 4. L2 schema (C02) — `[dma.<id>]`

`crates/regseq-config/src/platform.rs` (`DmaBindingSpec`, structural parse) + `crates/regseq-l2/src/dma.rs`
(the `dma` tenant: `bind` joins, `validate` enforces presence + D07):

```toml
[dma.tx_ring]                          # a DmaRegion
device_address_bits      = 32          # (was dma_mask)
alignment                = 16          # bytes; OMITTED ⇒ align_of::<T>() (NOT 1) — D08
region_cache_maintenance = "none"      # none | per_access ; per_access REQUIRES cacheline isolation (FM4), recorded not enforced

[dma.rx]                               # a DmaChannel
endpoint_addr_width      = 1           # bytes (was slave_addr_width)
endpoint_max_burst_beats = 16          #       (was slave_maxburst)
binding_scope            = "concrete"  # concrete | template | family | conditional ; OMITTED ⇒ concrete (D07)
cap_pause                = "present"    # present | absent | unknown(omitted) ; absent legal ONLY in a concrete binding
cap_cyclic               = "absent"
```

- region: `device_address_bits` / `alignment` / `region_cache_maintenance` (closed `none|per_access` vocab,
  `DMA-L2-002`).
- channel: `endpoint_addr_width` / `endpoint_max_burst_beats` / `binding_scope` / `cap_*`.
- **a recognized region with NO `[dma.<id>]` section is `DMA-L2-001`**; an orphan section is `DMA-L2-003`.
- the **capability-gating contract** (`binding_scope` vocab `DMA-L2-004`, `cap_*` value vocab `DMA-L2-005`, the
  `absent`-only-in-concrete rule `DMA-L2-006`) is §5.4.

---

## 5. Verify rules + the diagnostic family

### 5.1 C03 layout verify — the real `DmaPod` bit-validity predicate (M4/T19; the centerpiece)  [Contract]

`crates/regseq-driver/src/bridge/dma/layout.rs` — a **TyCtxt-aware** pass `verify_dma_layout(cx)` (wired into
`materialise_bodies` after `recognise_dma_region_sites`, so the per-access view sites exist AND the rustc `Ty`
is in hand). For the **declaration `T`** AND **every `element_access` view `V`** (read off the closure's
rust-call signature `fn((&[mut] V,))` — NOT a positional generic arg, since `DmaRegion::<T>::write::<V,F>`
carries `[T, V, F=closure]`):

```
require: V: Sized + DmaPod
require: size_of::<V>()  <= element_size
require: align_of::<V>() <= region_alignment        # region_alignment = alignment? else align_of::<T>()
require: element_size % align_of::<V>() == 0
require: i in [0, element_count)                     # where the index is a static const
else: COMPILE-FAIL (DMA-ERR-LAYOUT-VERIFY, naming the violated clause)
```

**`DmaPod` = repr(C) BIT-VALIDITY POD** (`dma_pod_reason(tcx, ty)`, recursive over the rustc `Ty`/`AdtDef`).
A DMA device writes **arbitrary bytes** into the coherent buffer, so a sound view must have **no validity
invariant any byte pattern can violate** (the `bytemuck::Pod` distinction):
- **ACCEPTS:** `Int`/`Uint`/`Float` scalar leaves ONLY (every bit pattern valid; floats incl. NaN); arrays
  (recurse on the element); `#[repr(C)]`/`#[repr(transparent)]` structs+unions whose every field is `DmaPod`;
  `PhantomData`/ZST markers; `()`.
- **REJECTS** (each with a named reason): `bool` (only 0x00/0x01 valid) · `char` (must be a valid Unicode
  scalar) · **ALL enums** (only declared discriminants valid) · `Drop` glue · `Ty::Ref`/`RawPtr`/`FnPtr`
  (a descriptor carries integer bus addresses, not host pointers) · DSTs `Slice`/`Str`/`Dynamic` · unresolved
  generics `Param`/`Alias`/infer/bound/placeholder/error · non-`repr(C)` aggregates · tuples.

Two soundness rails folded from the M4 Codex review:
- **non-const index → REJECT** (DMA-ERR-LAYOUT-VERIFY): generic-c emits `base + i*element_size` runtime
  arithmetic with **NO runtime bounds guard**, so a present-but-non-const index cannot be statically bounded
  (mirrors the M3 non-const-span-range reject).
- **FAIL-CLOSED:** an unrecoverable declared `T` or an unresolvable view `V` → REJECT (not a silent skip) — a
  soundness check must not pass when it cannot verify. (The E1000/PL011 concrete `repr(C)`/scalar types recover
  cleanly, so failing closed causes no false-reject — confirmed by the goldens.)
- **hardening:** unresolved-generic kinds are rejected **before** the `needs_drop` query (which would misbehave
  on a non-monomorphic type).

### 5.2 C10 fallibility lint (M2/T17)  [Contract]

A discarded `Result` from `dma::stream`/`stage_transfer`/`start_transfer`/`remaining_bytes` is a compile-fail
(`DMA-ERR-DISCARDED-RESULT`), **including** the explicit `let _ = …` and `core::mem::{drop,forget}(op())` forms
that Rust's `unused_must_use` does NOT catch. `stop()` is infallible and exempt. (Documented residual gaps —
aggregate-store-never-read, reassignment-before-read, projected destination, indirect sink — are FALSE-ACCEPTs,
never false-rejects.)

### 5.3 Anti-form / direction / span-bounds (M3/T20)  [Contract]

- **anti-form** (`DMA-ERR-ANTI-FORM`): a bare `stage_transfer(&DmaRegion<T>)` (a region passed directly instead
  of via `region.span(range)`).
- **span-bounds** (`DMA-ERR-SPAN-BOUNDS`): a zero-length (`start >= end`) or out-of-bounds (`end > element_count`)
  span, or a **non-const span range** (rejected this cycle — `region.span(const..const)` only).
- **direction** (`DMA-ERR-DIRECTION-MISMATCH`): the diagnostic exists; the span is direction-neutral by design,
  so a span endpoint is accepted for either channel direction. The region-incompatible-with-direction sub-check
  (a non-device-writable region as an RX destination) has **no IR carrier this cycle** and is
  **recorded-not-enforced** (a documented deferral).

### 5.4 C04/D07 capability-gating contract — NO cap ops built (M4/T21)  [Contract]

`crates/regseq-l2/src/dma.rs::validate_capability_scope`. The capability OPS themselves are
**designed-not-built** this cycle (§9.3); the **only buildable C04 oracle** is the L2 rule:
```
binding_scope in { concrete (omitted default), template, family, conditional }   # else DMA-L2-004
cap_* in { present, absent, unknown(omitted) }                                    # else DMA-L2-005
cap_* = "absent" is legal ONLY in a CONCRETE binding:                             # else DMA-L2-006
    a template / family / conditional binding's `absent` is REJECTED (the resolved controller is
    unknown there, so its capability cannot be statically denied — it must use `unknown`).
```
A **concrete** binding's `cap_* = "absent"` is **accepted by the rule** — there is no cap op to reject yet
(designed-not-built, Codex R1#9).

### 5.5 The full diagnostic family (T22)  [Contract]

**Recognition half — `DmaDiagnostic` on `BridgeError` (`crates/regseq-driver/src/bridge/mod.rs`), with a
programmatic `code()` accessor (14 `DMA-ERR-*` codes):** `MISSING-KEY`, `MALFORMED-ATTR`, `ATTR-ON-NON-REGION`,
`UNSUPPORTED-FORM`, `DUPLICATE-ID`, `LAYOUT-MISMATCH` (the M1 declaration-time numeric guard),
`MISSING-DIRECTION`, `DISCARDED-RESULT` (C10), `COMPLETION-ATTR`, `UNKNOWN-COMPLETION-CHANNEL`, `ANTI-FORM`,
`DIRECTION-MISMATCH`, `SPAN-BOUNDS`, `LAYOUT-VERIFY` (the M4 C03 per-access verify). Every variant's `code()`
is the prefix of its rendered message (asserted by the corpus consistency test).

**Binding half — `DmaL2Error` (`crates/regseq-l2/src/dma.rs`, 6 `DMA-L2-*` codes):** `001` missing section ·
`002` bad cache-maintenance · `003` orphan section · `004` bad binding-scope · `005` bad capability value ·
`006` absent-capability-in-template.

---

## 6. Ordering (C07)

```
dma_mmio_store_order(region_store, mmio_write)  [BUILT this cycle, D09]:
   emit regseq_dma_wmb() between the last publishing region store and the MMIO doorbell that exposes it.
   GOLDEN (E1000 TX): …region stores… -> regseq_dma_wmb() -> regseq_mmio_w32(TDT), in order; fails if the
   fence is absent/misordered. register-access emits the plain doorbell; dma-management inserts ONLY the fence.

completion_marker_order(marker_read, payload_read)  [DESIGNED, NOT BUILT this cycle, D09]:
   the DD/phase consume fence (regseq_dma_rmb), FM1 marker-field identification, FM3 MMIO-sourced ordering —
   explicitly fenced out of the Stage-C build + goldens (§9.3).
```

---

## 7. generic-c lowering + the host DMA model (C11)  [Contract]

`crates/regseq-backends/src/generic_c/` — `body_emit.rs` (the `emit_dma`/`emit_dma_channel`/`emit_dma_stream`
arms + the `emit_lifecycle_synthesis` preamble/postamble) + `prelude.rs` (`DMA_HELPERS`). **Emission gate =
driver has ANY DMA resource** (region | channel | stream), distinct from the bus-master `dma_present` flag.

### 7.1 The host DMA device-model hook interface (the test contract)

```c
/* the host model is a GLOBAL singleton — the hooks take an `id` string, no device handle */
/* coherent region: a REAL aligned backing slab + a synthetic device base */
void*    regseq_dma_alloc_coherent(const char* id, uint64_t size, uint64_t align, uint64_t* out_dev_addr);
void     regseq_dma_free_coherent(const char* id);
void*    regseq_dma_region_cpu(const char* id);          /* the backing buffer (element_access reads/writes it) */
uint64_t regseq_dma_region_dev_base(const char* id);     /* the synthetic device base (device_addr) */
/* streaming */
uint64_t regseq_dma_stream(void* cpu, uint64_t len, int dir);     /* was dma_map; returns a synthetic dev addr */
void     regseq_dma_unmap(uint64_t dev_addr, uint64_t len, int dir);
void     regseq_dma_acquire_from_device(uint64_t dev_addr, uint64_t len);
void     regseq_dma_release_to_device(uint64_t dev_addr, uint64_t len);
/* the store_order fence — golden-asserted (D09/C07) */
void     regseq_dma_wmb(void);     void regseq_dma_rmb(void);   /* rmb DEFINED but NOT emitted this cycle */
/* channel (a fake DMA engine that records the call sequence) */
int      regseq_dma_request_channel(const char* id, int dir);   void regseq_dma_release_channel(const char* id);
void     regseq_dma_configure(const char* id);
int      regseq_dma_stage_transfer(const char* id, uint64_t addr, uint64_t len, int dir);
int      regseq_dma_start_transfer(const char* id);             void regseq_dma_stop(const char* id);
int      regseq_dma_remaining_bytes(const char* id, uint64_t* out);
/* completion: registration recorded; NOT invoked this cycle (D06/C05) */
void     regseq_dma_register_completion(const char* id);
```
The coherent-region path keeps a **REAL `aligned_alloc` backing slab** (honoring the declared `align`/`size`) +
a synthetic device base, so an `element_access` closure's typed stores land in **observable memory** (the
behavioral byte oracle reads them). The channel/streaming hooks **print a `DMA <event>` line** (the recorded
call sequence is the codegen-shape oracle — a fixture-local golden spelling, NOT a source contract, Codex R1#10).

### 7.2 Per-op lowering (as-built)

| Op | generic-c lowering |
|---|---|
| region `alloc_coherent`/`free` | compiler-synthesized preamble/postamble (declaration order; LIFO teardown) |
| `device_addr()` / `device_addr_of(i)` | `regseq_dma_region_dev_base(id)` (+ `i*element_size` for `_of`) |
| `element_count()` | the declaration const, inlined |
| `element_access(i, \|V\| …)` | a properly-aligned typed temp `V __tmp;` + `memcpy` to/from `regseq_dma_region_cpu(id) + i*element_size` (consuming reads in first; the inlined closure stores run on `&__tmp`; publishing copies back); **NO cache op** (coherent, C01); a publishing write also emits the `regseq_dma_wmb()` fence (§5.4 over-fence note) |
| `region.span(range)` | a value-symbolic no-op; the consuming `stage_transfer` re-derives `dev_base(prov) + offset*stride` + `len` |
| `dma::stream(buf, flow)` | `__dma_stream_<n>_dev = regseq_dma_stream(cpu, len, dir)` (captures dev/len/dir into a body-level triple) + an `Ok(buffer)` Result |
| `acquire_from_device` / `release_to_device` / buffer `device_addr` | the matching hook reading the tracked triple |
| streaming RAII unmap | `regseq_dma_unmap(dev, len, dir)` at the function's normal `Return`, LIFO (§5.1 below) |
| `configure`/`stage_transfer`/`start_transfer`/`stop`/`remaining_bytes` | the `regseq_dma_*` channel hooks; fallible ops build a REAL `Result` (an `int __dma_rc` capture + an `rc==0` Ok / else Err branch — NOT an int→struct cast) |
| `request_channel`/`release_channel` | compiler-synthesized channel preamble/postamble |
| completion registration | `regseq_dma_register_completion(channel)` (never fires the callback) |
| `dma_mmio_store_order` | `regseq_dma_wmb()` (golden-asserted) |

**The streaming RAII unmap seam (T15) — as-built honest claim:** the regseq body IR carries **NO `Drop`
terminator** (rustc's MIR Drop is elaborated into a `Call` to drop-glue). So for the candidate-prototype the
RAII unmap is realized as a **deterministic SCOPE-EXIT cleanup**: a matching `regseq_dma_unmap` is emitted at
the function's normal `Return`, in reverse map order (LIFO), reading the body-level `(dev,len,dir)` triple. The
honest claim is *"the surface EXPRESSES RAII unmap; generic-c lowers it as a deterministic function-exit
cleanup"* — **NOT** behavioral Drop-timing fidelity (precise drop-glue placement + early/conditional drop is
future work). Codex confirmed this acceptable for the prototype.

---

## 8. Compiler path — recognition + lowering (as-built file map)

| Concern | File | What |
|---|---|---|
| IR | `regseq-model/src/dma.rs` | `DmaTables`/`DriverDma` + all the §3 facts |
| IR markers | `regseq-model/src/domain_attr.rs` | `AttrData::{DmaRegionResource(9), DmaChannelResource(10), DmaCompletionBinding(11)}`; `IntrinsicKind::Dma(DmaOp)(9)` + `DmaOp` + `from_name` |
| recognition | `regseq-driver/src/bridge/dma/recognition.rs` | Phase-4 region/channel/completion materialisation; Phase-5 `element_access`/span/streaming + the fallibility lint + the anti-form check |
| verify | `regseq-driver/src/bridge/dma/layout.rs` | the C03 layout verify + the `DmaPod` predicate (§5.1) |
| diagnostics | `regseq-driver/src/bridge/mod.rs` | `DmaDiagnostic` + `code()` |
| L2 | `regseq-config/src/platform.rs`, `regseq-l2/src/dma.rs` | `DmaBindingSpec` parse + the `dma` tenant (join + validate + the capability contract) |
| backend | `regseq-backends/src/generic_c/body_emit.rs`, `prelude.rs` | the `emit_dma*` arms + lifecycle synthesis + `DMA_HELPERS` |

The construct **rides the `Mmio<T>` typed-access precedent** (an `IntrinsicKind::Dma(...)` variant + an
`emit_dma` arm beside `emit_mmio`); the `element_access` closure-borrow uses a **NARROW HRTB carve-out** (scoped
to a recognized `dma_element_*` call + the closure-reified-sig SHAPE) so the real borrow surface bridges while
the global substantive-HRTB guard is unchanged for all non-DMA code.

---

## 9. AS-BUILT — implementation + results

### 9.1 Built + exercised (generic-c, candidate-prototype)

- **M1–M3 (committed `cedf072`):** the IR + bridge recognition + L2 schema + the generic-c lowering for the
  **E1000 coherent half** (`DmaRegion<T>` + A2 + `element_access` + the store→doorbell fence + the host model)
  and the **PL011 channel half** (slave `DmaChannel` 5-op transport + lifecycle + `span` endpoint + completion
  registration), with `dma_present` narrow wiring + the C10 fallibility lint.
- **M4 (uncommitted):** T19 the C03 layout verify rule (the real `DmaPod` bit-validity predicate); T15 the
  streaming `DmaStreamBuffer` emit (map/acquire/release + the scope-exit RAII unmap); T21 the capability-gating
  contract (L2 `absent`-in-non-concrete reject; no cap ops); T22 the `DMA-ERR-*` diagnostic family + `code()` +
  the compile-fail oracle corpus. **+ the Codex milestone-review fold:** the `DmaPod` bit-validity fix
  (reject `bool`/`char`/enum), the non-const-index reject, fail-closed on unrecoverable `T`/`V`.
- **M5 (uncommitted):** T24 the Doc-1 `DMA-1..16` matrix rows + `dma_covers.rs` (the honest-coverage gate);
  T25 this collateral reconciliation + the `candidate-prototype` label + the designed-not-built deferrals.

### 9.2 Tests + goldens

- **Full `cargo test`: 2757 passed / 0 failed / 12 ignored.**
- **Behavioral / codegen-shape goldens (six-stage, interner-isolated under `fixtures/_dma/`):**
  - `dma_e1000_region` — the coherent-ring **byte oracle** (the descriptor bytes in the backing slab) + the
    **store_order fence sequence** (`…stores → regseq_dma_wmb() → regseq_mmio_w32(TDT)`).
  - `dma_pl011_channel` — the channel call-order (`alloc_coherent → request_channel → register_completion →
    configure → stage_transfer (REAL span addr/len) → start_transfer → release_channel → free_coherent`).
- **gc-shape / unit witnesses:** `dma_streaming_gc_shape` (the streaming map→release→acquire→unmap order);
  `bridge_dma_recognition` (47 bridge tests — recognition + the C03/C10/anti-form reject families, all
  neuter-witnessed); the `regseq-l2` `dma` tests (the L2 join + the capability contract, neuter-witnessed).
- **Coverage gate:** `dma_covers.rs` (§9.3).

### 9.3 Designed-NOT-built this cycle (honest deferrals — matching the `dma_covers.rs` partition)

The `candidate-prototype` maturity is **generic-c only**; these are designed (eval design analysis + GRILL) but
**not built / not exercised**, and are labeled as such in the Doc-1 matrix (`DMA-13..16`, all `deferred` with a
**DESIGNED-NOT-BUILT** note) and excluded from coverage by `dma_covers.rs`:

- **Completion delivery/firing** (`DMA-13`, D06) — only the registration shape (`DMA-7`) is built; the host
  model never invokes the callback (no `DmaCompletion` is constructed/delivered).
- **`completion_marker_order` / `regseq_dma_rmb`** (`DMA-14`, D09) — only the `store_order` fence (`DMA-4`) is
  built; the consume fence (FM1 marker-field identification, FM3 MMIO-sourced ordering) is not.
- **Capability ops + the concrete-`absent` cap-op rejection** (`DMA-15`, D07/S08) — `pause`/`resume`/
  `stage_circular_transfer`/`stage_segments` are not built; only the L2 capability **contract** (`DMA-11`,
  DMA-L2-006) is. The concrete-`absent` op rejection has no op to reject yet.
- **Linux/Zephyr lowering + Tier-2** (`DMA-16`) — the real-backend DMA lowering (Linux
  `dma_alloc_coherent`/`dmaengine_*`/`dma_wmb`; Zephyr stateful `dma_config`/`dma_start`/cache-op ordering) +
  a Tier-2 behavioral leg (model §7, design §5).
- **SG construction / `host_to_host` / partial-sync / slave asymmetry** — designed-in, not built.

**The `dma_covers.rs` honest-coverage partition** classifies all 16 DMA-* rows into:
**FIXTURE-COVERED** {DMA-1..7, DMA-12} (a `_dma` six-stage fixture exercises it) · **TEST-VERIFIED** {DMA-8
(streaming gc-shape), DMA-9 (C03 layout reject), DMA-10 (C10 fallibility), DMA-11 (capability contract)} ·
**DESIGNED-NOT-BUILT** {DMA-13, DMA-14, DMA-15, DMA-16}. The test pins the union to the exact matrix universe
(a new row forces a conscious classification) and rejects a fixture overclaiming a non-FIXTURE-COVERED row. **No
row claimed built here is marked designed-not-built by the gate, and vice versa** — this doc and the gate agree.

### 9.4 Honest scope & caveats

- **Generic-c only.** This proves the generic-c lowering (the design crucible / behavioral oracle); Linux/Zephyr
  lowering is per the design tables (`dma-management-design.md` §5), not built (`DMA-16`).
- **The store_order fence is conservatively OVER-fenced** (it fires after every publishing element write, not
  only on the genuine last-publishing-store → doorbell edge — extra `wmb()`s never break ordering; the precise
  `StoreOrderEdge` trigger is `[Open]` in design §6, `R-storeorder-trigger`).
- **The streaming RAII unmap is scope-exit, not true Drop placement** (§7.2) — misses early/conditional drop and
  non-Return exits (no panic-unwind path in generic-c, so moot here).
- **`per_access` cache maintenance is RECORDED-not-enforced** (the cacheline-isolation precondition FM4 is a
  contract note, not a check, this cycle).
- **The direction region-compatibility sub-check is recorded-not-enforced** (no IR carrier this cycle, §5.3).
- **The host model is a global singleton** (no real base/IOMMU) — a real target binds via the lifecycle/L2 seam
  (out of scope for the generic-c oracle).

---

## 10. Cross-links

- **Eval design analysis** (adequacy / model / DSL-surface / design / amendments):
  `regseq-eval/docs/constructs/dma-management/` — `dma-management.md`, `dma-management-model.md`,
  `dma-management-dsl-surface.md`, `dma-management-design.md`, `dma-management-taxonomy-amendments.md` (the C08
  name map §1), `README.md`, `codex-foundation-review-summary.md`.
- **Stage-B decision record + PRD + PLAN:**
  `regseq-eval/.claude/sekekama/grill-to-plan/specs/dma-management/` — `GRILL-2026-06-22-154000.md`,
  `PRD-2026-06-22-155302.md`, `PLAN-2026-06-22-160634.md`.
- **Doc-1 construct-surface matrix** (the `DMA-1..16` rows + the legend):
  `docs/discussions/generic-c-backend/phase-1/01-construct-surface-and-matrix.md`.
- **The compiler-collateral stub** (reconciled to C08 + maturity + deferrals):
  `docs/compiler-collateral/constructs/dma-management.md`.
- **The coverage gate:** `crates/codegen-coverage-runner/tests/dma_covers.rs`.

---

## 11. Open items / next

- **Commit M4 + M5** (currently uncommitted in this worktree) when the construct's work lands; merge `--no-ff`
  to `main`, then prune the worktree.
- **Stage D-2:** reconcile the eval design docs to the GRILL + this as-built (the design docs predate the
  M4 bit-validity fold + the M5 row taxonomy).
- **Governance (O4):** the frozen-taxonomy `[F]` renames (the C08 worst leaks) + the completion re-admission
  route through the gap-remediation procedure on the taxonomy branch (not applied here).
- **Future build legs** (when prioritized): completion delivery (`DMA-13`), the consume fence (`DMA-14`), the
  capability ops (`DMA-15`), Linux/Zephyr lowering + Tier-2 (`DMA-16`).
