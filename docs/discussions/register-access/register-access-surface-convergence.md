# Register Access — converged DSL surface (eval ⇄ compiler)

> **Home:** this is the **final spec contract** for register-access, co-located with
> the implementation in this repo (`docs/discussions/register-access/`). The
> evidence-grounded **design analysis** (adequacy, model, DSL-surface, design) lives
> in the eval repo at `regseq-eval/docs/constructs/register-access/`; filename
> references below to those analysis docs resolve there. The compiler-side
> realization discussion is `register-access-discussion.md` (this folder).

**Date:** 2026-06-04 · **Status:** ✅ **LOWERING CONFIRMED** — PL011 **and** E1000 register access lower
through the live generic-c backend, compile with `cc`, and run with correct behavioral traces (Codex
independently re-verified: **CLAIM-SOUND-WITH-CAVEATS**, 2026-06-04). See §9. Plan was Codex-reviewed
(SOUND-WITH-CHANGES; 6 refinements folded — see §8).
**Purpose:** refine the register-access DSL surface by adopting the **proven, ergonomic** features of the
earlier PL011 spec attempt (`compiler:tests/fixtures/pl011-full-spec/src`) and **converging with the
compiler's own exp01-validated direction** (`compiler:docs/discussions/register-access/register-access-
discussion.md`), so the surface has a guaranteed **path through the compiler**.
**Supersedes the *syntax* of** `register-access-dsl-surface.md` (the `register_block!`-generates-everything +
`Mmio<T>`-method-with-token shape). **Preserves** all authoritative decisions in `register-access-model.md`
(the model is unchanged) and the lowering/IR/test payload in `register-access-design.md`.

---

## 0. Why converge (and what we borrow)

Two independent surfaces existed for the same construct:
- **my v1 eval surface** (`register-access-dsl-surface.md`): one `register_block!` macro generating everything;
  operations on `Mmio<T>` taking a `Field<T,R>` token (`regs.read(FR)`).
- **the earlier full-spec + the compiler's exp01/discussion**: a **tock-registers-style** split surface,
  type-checked in `compiler:experiments/register-access/exp01` and adopted as the compiler's working direction.

The compiler's shape is **already proven to type-check** and is what the compiler team is building toward.
Since the eval surface **must** lower through the compiler (user constraint #3), we converge onto it. The
positive features we borrow (all from the earlier spec / exp01):

1. **tock-registers field syntax** — `NAME OFFSET(n) NUMBITS(w) [NAMED = v, …]`. Familiar to every Rust-embedded
   author; battle-tested; reads like the hardware manual.
2. **access-mode as the register's wrapper type** — `dr: ReadWrite<…>`, `fr: ReadOnly<…>`, `icr:
   WriteOnly<…>`. Access legality becomes **method availability** (a `ReadOnly` field simply has no `write`),
   which is simpler and stronger than my v1 `Mode`-bound scheme and needs no special compiler check.
3. **`FieldValue` `+`-composition** — `WLEN_8 + FEN::SET` builds a multi-field write value, const-evaluable.
4. **module-per-register namespace** — `FR_REG::BUSY::FIELD`, `LCRH_REG::WLEN::WLEN_8`. Self-documenting,
   collision-free, reusable across blocks.
5. **`Mmio<T>: Deref<Target=T>`** — authored access reads naturally: `self.regs.cr.modify(…)`.
6. **explicit `#[repr(C)]` block struct** — the block is real, HIR-visible Rust; reserved gaps are explicit;
   field-local metadata has a natural home; the compiler extracts over ordinary struct fields, not only macro
   output.

The **split** (schemas in a macro, layout in an explicit struct) is itself a borrowed decision: it is the
compiler's preferred shape because *field schema and block layout are different concerns* (discussion §"Why
The Explicit Register Block Matters"). My model's clause "**Macro = expansion contract is firm; exact syntax
is a proposal**" gives us the latitude to adopt it without changing any model decision.

---

## 1. Model fidelity & L1/L2 separation (the non-negotiables)

This surface still realizes `register-access-model.md` exactly — only the *spelling* changed:
- **3 concerns** — transport (resource type + L2 binding), register-block definition (macro + struct),
  register operations (wrapper methods + poll). ✓
- **`Mmio<T>` (direct/infallible)** is the build resource; `Regs<T>` (mediated/fallible) stays designed-in,
  not built. ✓
- **composition-over-absorption scope boundary** — indirection/paged, FIFO, locking, execution-context,
  ordering remain OUT. The compiler independently agrees: `ordered!` deferred to a later construct;
  relational/staging-commit metadata kept visible but deferred (discussion §"Relational Register Metadata").
  ✓ (strong cross-witness of the model's §7 exclusions).
- **deferrals** — ~~register arrays~~ (now implemented, see §10), mediated transports, variant layouts. ✓

**L1/L2 separation (user constraint #1) — explicit:**

| Layer | Lives in | Carries |
|---|---|---|
| **L1** (device-invariant) | the driver `.rs` spec | `register_bitfields!` schemas; the `#[repr(C)] #[rseq::register_block]` struct (layout, offsets, widths, access-mode, fields, named values); `#[rseq::register]` semantic metadata (side_effects, hw_updates); the `#[rseq::mmio] regs: Mmio<T>` resource declaration; the operations |
| **L2** (per-integration) | the platform file (`platform.toml`) | the MMIO region **binding** — a `[mmio.<id>]` section with `base` + `size` (DT/static, PL011) **or** a `bar` selector (PCI, E1000), keyed by the resource id. Joined to L1 at `map_region`. |

The register block `T` is **transport-independent and carries no address** (model §4). Base/size/BAR attach to
the **`#[rseq::mmio]` resource field** (the consumer-side declaration site), resolved from `platform.toml`.
This is exactly the model's L1/L2 split, preserved.

**Reconciling "not a memory image" (design doc §4.1) with the explicit `#[repr(C)]` struct:** the struct is
**layout-only** — it exists so the compiler can derive each register's byte offset from `#[repr(C)]` field
order + `Reserved<u32>` spacers. The author **never** performs a cached load/store on it; `self.regs.cr` is a
`Deref` to a zero-storage `ReadWrite<u32,R>` placeholder, and `.read()/.write()/.modify()` **lower to volatile,
side-effect-aware MMIO** at `base + offset`. This is precisely the tock-registers model: a `#[repr(C)]` layout
type whose every access is volatile. So "not a memory image" = *the access semantics are volatile MMIO, never
cached struct loads* — which this surface upholds.

---

## 2. The authored surface (three pieces + resource + ops)

### 2.1 `regseq::register_bitfields!` — field/register schemas (the only generative piece)  [Recommendation: syntax; Contract: the expansion]
```rust
regseq::register_bitfields! [
    u32,                                          // storage width for the registers below

    DR_REG [ DATA OFFSET(0) NUMBITS(8) [] ],

    FR_REG [
        RXFE OFFSET(4) NUMBITS(1) [],
        TXFF OFFSET(5) NUMBITS(1) [],
        BUSY OFFSET(3) NUMBITS(1) [],
    ],

    LCRH_REG [
        WLEN OFFSET(5) NUMBITS(2) [ WLEN_5 = 0, WLEN_6 = 1, WLEN_7 = 2, WLEN_8 = 3 ],
        FEN  OFFSET(4) NUMBITS(1) [ CLEAR = 0, SET = 1 ],
        PEN  OFFSET(1) NUMBITS(1) [],
    ],

    CR_REG [
        UARTEN OFFSET(0) NUMBITS(1) [ DISABLED = 0, ENABLED = 1 ],
        TXE    OFFSET(8) NUMBITS(1) [ DISABLED = 0, ENABLED = 1 ],
        RXE    OFFSET(9) NUMBITS(1) [ DISABLED = 0, ENABLED = 1 ],
    ],

    ICR_REG [ RXIC OFFSET(4) NUMBITS(1) [], TXIC OFFSET(5) NUMBITS(1) [] ],
];
```
**Expansion contract** [Contract — the metadata; the module shape is the proven exp01 contract]: for each
register `XYZ_REG`, a module `XYZ_REG` containing a zero-size marker `struct Register;` and, per field `F`, a
sub-module `F` with `pub const FIELD: Field<W, Register> = Field::new(offset, width);` plus, per named value
`N = k`, `pub const N: FieldValue<W, Register> = FieldValue::new(offset, width, k);`. All const-evaluable
(C-emittable). This is the exp01-validated contract (`compiler:experiments/register-access/exp01/...`).
Multiple `register_bitfields!` invocations per file and descriptor reuse across blocks are supported
(discussion Q2).

### 2.2 `#[repr(C)] #[rseq::register_block(...)]` — the explicit block layout  [Contract = metadata; Recommendation = spelling]
```rust
#[repr(C)]
#[rseq::register_block(endian = "little", access_width = 32)]
pub struct Pl011Regs {
    pub dr:      ReadWrite<u32, DR_REG::Register>,        // 0x000
    pub rsr_ecr: ReadWrite<u32, RSR_ECR_REG::Register>,   // 0x004
    _reserved0: [Reserved<u32>; 4],                       // 0x008..0x018
    #[rseq::register(hw_updates)]
    pub fr:      ReadOnly<u32, FR_REG::Register>,          // 0x018  (hardware-updated flags)
    _reserved1: [Reserved<u32>; 1],                       // 0x01C
    pub ibrd:    ReadWrite<u32, IBRD_REG::Register>,       // 0x024
    pub fbrd:    ReadWrite<u32, FBRD_REG::Register>,       // 0x028
    pub lcr_h:   ReadWrite<u32, LCRH_REG::Register>,       // 0x02C
    pub cr:      ReadWrite<u32, CR_REG::Register>,         // 0x030
    // … imsc/ris/mis …
    #[rseq::register(side_effects = "write_one_to_clear")]
    pub icr:     WriteOnly<u32, ICR_REG::Register>,        // 0x044
}
```
- **`#[rseq::register_block(endian, access_width)]`** — block-level L1 metadata; **inert** (generates nothing;
  consumed by extraction). [Contract]
- **access mode = the wrapper type** (`ReadOnly`/`WriteOnly`/`ReadWrite`); **width duplicated** in the wrapper
  (`<u32, …>`) and the descriptor (`Field<u32, …>`) — intentional, for simple extraction + consistency check
  (discussion Q1). [Contract: the duplication is meaningful]
- **offsets are layout-derived** from `#[repr(C)]` field order + `Reserved<u32>` spacers [Contract: the compiler
  computes per-register `OFFSET` from layout], **but must be checked against an explicit per-register expected
  offset** (Codex #5). The layout `_Static_assert`s the backend already emits prove *C-matches-Rust-layout*, not
  *Rust-layout-matches-hardware* — a miscounted `Reserved` would pass silently. [Contract: each register
  carries an explicit expected offset (e.g. a `@ 0xNN` annotation on the field, or a per-register
  `assert_eq!(BLOCK::reg::OFFSET, 0xNN)` in the spec/fixture) that the compiler validates and emits as a
  `_Static_assert` against the layout-derived offset.] *(This also resolves §8 focus #4: layout-derived **with**
  explicit guards.)*
- **`#[rseq::register(side_effects = "…", hw_updates)]`** — per-register semantic L1 metadata on the field;
  **inert** (discussion §"Side Effects In Register Access"). `side_effects ∈ {write_one_to_clear, read_clear,
  …}`. [Contract = the metadata set; Recommendation = the attribute spelling]

### 2.3 `#[rseq::mmio]` resource on the driver struct  [Contract]
```rust
pub struct Pl011 {
    #[rseq::mmio(id = "regs")]            // L1: just a resource id; NO transport params here
    regs: Mmio<Pl011Regs>,
}

pub struct E1000 {
    #[rseq::mmio(id = "regs")]            // L1: identical shape — the device is MMIO-admissible ({MMIO})
    regs: Mmio<E1000Regs>,
}
```
```toml
# platform.toml — L2 (per-integration). The CONCRETE transport + params live HERE, never in the .rs spec.
# Format per layer-2-format-discussion.md + pl011-full-spec/platform.toml: version + [platform] +
# construct-owned sections keyed by the resource id. Section name = the concrete transport/resource-kind
# (`mmio`), an L2 fact — NOT the L1 Rust type (decided 2026-06-04).
version = "1"
[platform]
os = "linux"
board = "qemu-virt"
arch = "aarch64"

# PL011 (DT/AMBA, static region):
[mmio.regs]                       # section `mmio`, key `regs` ↔ #[rseq::mmio(id = "regs")]
base = 0x09000000
size = 0x1000
# E1000 (PCI): a BAR selector — base/size resolved at probe by bus-resource:
# [mmio.regs]
# bar = 0
```
- the **single register-access namespace attribute** is `#[rseq::mmio(id = "…")]` (taxonomy house style); in L1
  it marks the **field** as the typed resource and names the platform-file binding key — **no transport
  parameter** (`bar`/base/size) appears in the Rust spec. [Contract — **L1/L2 fix per Codex #2**: `bar` is a
  *concrete transport parameter* and the model puts those in L2 (`register-access-model.md:85,197`).]
  - **O4(b) reconciliation (bus-resource-management construct #3, 2026-06-07) — `bar` is now L1-or-L2, L1
    preferred.** The original "no `bar` in the Rust spec" rule above is **superseded** for `bar` specifically:
    a *default-decoded* BAR index is a device-invariant L1 fact (the E1000 register window is always BAR0,
    regardless of board), so `#[rseq::mmio(id = "regs", bar = 0)]` is the **L1-preferred** declaration. The L2
    `[mmio.<id>] bar` remains legal and **overrides** L1 when present: **effective BAR = `L2.bar ?? L1.bar`**
    (GRILL I01). `base`/`size` stay **L2-only** (per-integration). So: `bar` may appear at L1 or L2 (L1
    preferred, L2 overrides); `base`/`size` are L2-only. The E1000 fixture is migrated to the L1 form
    (`tests/fixtures/regaccess-e1000/src/lib.rs`), with the agreeing L2 `bar = 0` retained for back-compat; the
    L2-supplies form (no L1 `bar`, as PL011 uses for `base`/`size`) is unchanged. A01 is **assumed, not
    ratified** — if reverted, the L2-supplies form still works (low-risk). Hybrid support is proven by the
    bridge join tests (`regseq-l2/src/bus_resource.rs::tests::{join_by_id_resolves_base_size_and_effective_bar,
    l2_bar_overrides_l1}`).
- `Mmio<T>: Deref<Target = T>`; `T` must be a `#[rseq::register_block]` type (a compiler-side extraction check,
  not a trait bound — discussion Q4).
- L2 binding (a `[mmio.<id>]` section with `base`/`size` **or** a `bar` selector) lives in `platform.toml`
  keyed by the resource id; `map_region` (compiler-managed, init+deinit) joins L1+L2. The `Mmio<T>` type itself
  encodes the **admissible transport set = {MMIO/direct}** (the L1 device fact); the board picks the concrete
  binding (L2). Section name = the concrete transport/resource-kind (`mmio`), matching the reference
  platform.toml + the `#[rseq::mmio]` token (decided 2026-06-04).

### 2.4 Operations — wrapper methods (via Deref) + poll  [Contract = semantics; Recommendation = spelling]

> **Spelling finalized 2026-06-04** (see §"Form A vs Form B — Form B now CONFIRMED"): the op set below was
> locked as `read()`/`write(v)` (whole-register raw), `read_field(F)`/`write_field(FV)` (field), `modify(FV)`
> (field RMW). There is **no `get`/`set`** and no whole-register `modify`; the `set`-vs-`write` `[Open]` is
> resolved. The semantics below stand; read the spelling through that mapping.

```rust
impl Pl011 {
    fn enable(&self) {
        self.regs.cr.modify(CR_REG::UARTEN::ENABLED + CR_REG::TXE::ENABLED + CR_REG::RXE::ENABLED);
    }
    fn set_word_length_8(&self) {
        self.regs.lcr_h.modify(LCRH_REG::WLEN::WLEN_8 + LCRH_REG::FEN::SET);
    }
    fn put(&self, byte: u32) -> Result<(), RegseqError> {
        regseq::poll::until(|| self.regs.fr.read(FR_REG::TXFF::FIELD) == 0, DELAY, TIMEOUT)?;
        self.regs.dr.write(DR_REG::DATA.val(byte));      // Field::val(v) -> FieldValue
        Ok(())
    }
    fn ack_irqs(&self) { self.regs.icr.write(ICR_REG::RXIC::SET + ICR_REG::TXIC::SET); }  // W1C
}
```
Operation set (per wrapper type — legality by availability):
- `ReadOnly<T,R>`: `read(Field<T,R>) -> T`.
- `ReadWrite<T,R>`: `read(Field<T,R>) -> T`, `write(FieldValue<T,R>)`, `modify(FieldValue<T,R>)`, `set(FieldValue<T,R>)`.
- `WriteOnly<T,R>`: `write(FieldValue<T,R>)`, `set(FieldValue<T,R>)`.
- `Field::val(v) -> FieldValue` (arbitrary value write); `FieldValue + FieldValue` (multi-field compose).
- `regseq::poll::until(cond, delay, timeout) -> Result<(), RegseqError>` — the one intrinsic; `cond` is a
  side-effect-free read predicate; composes with `?`.

**Semantics** [Contract]: `read(F)` returns the masked+shifted field value; `write(fv)` writes the composed
value (bits outside the composed mask = 0); `modify(fv)` is RMW (read, clear `fv.mask`, OR `fv.value`, write —
preserves unmentioned fields); `set` ≈ `write` for the wrapper [exact `set` vs `write` distinction is **[Open]**
— harness call; tock uses `set` for a raw whole-register value]. `read`/`write`/`modify`/`set` are **infallible**
on `Mmio<T>` (direct class); `poll::until` returns `Result` (timeout). This matches `register-access-model.md`
§5 and `register-access-design.md` §4.4.

---

## 3. Operation form decision — Form B (Deref-to-wrapper)

The corpus has two operation forms for the same construct:
- **Form A** (earlier full-spec `driver.rs` + `regseq/mmio.rs`): ops on `Mmio<Block>` taking a token —
  `regs.read(MIS::REG)`, `regs.read_field(FR::BUSY)`, `regs.write_field(fv)`, `regs.modify(REG, fv)`. Methods
  tagged `#[rseq::intrinsic("mmio_read")]` etc.
- **Form B** (exp01 + the compiler's current `register-access-discussion.md`): ops on the per-field **wrapper**
  reached via `Mmio<T>: Deref` — `regs.fr.read(FR_REG::BUSY::FIELD)`, `regs.lcr_h.modify(WLEN_8 + FEN::SET)`,
  `regs.dr.write(DR_REG::DATA.val(b))`.

**Decision: Form B.** It is the compiler's **current accepted direction** (discussion §"Wrapper Call-Site
Recognition"), is **exp01-type-checked**, gives access-legality for free (a `ReadOnly` wrapper has no `write`
method), and the old extraction's recognizer already keys on wrapper-type receivers
(`regseq-driver-old/src/extract/register_access.rs` `is_known_wrapper_method`). Form A is recorded as the
predecessor.

## 4. Compiler path — extraction + NORMALIZATION (finalized; Codex #1/#3/#4)

**Key architecture decision (Codex #1): normalize register ops into model-level facts in the BRIDGE, at
extraction/HIR time** — where receiver type, the block struct, and the const args are fully resolved — so the
backend never re-discovers register semantics from ordinary calls. The old
`regseq-driver-old/src/extract/register_access.rs` is the **template** (HIR pass producing
`RegisterAccessSurface { register_blocks, mmio_fields, wrapper_call_sites, poll_call_sites,
register_descriptors }`); port into the live `crates/regseq-driver` bridge (alongside `bridge/diag_log/`),
**enriching the call-site fact** to carry everything lowering needs (Codex #4):
- **`#[rseq::register_block]` struct** → register-block model: per field `{name, offset (layout-derived from
  `#[repr(C)]` order + `Reserved` sizes), access_width, access_mode (from wrapper type), side_effects/hw_updates
  (from `#[rseq::register]`), descriptor R, **expected_offset** (Codex #5, see §2.2)}`.
- **`register_bitfields!` modules** (HIR-visible) → per register `{R marker, fields:[{name, offset, width,
  named_values}]}` — recover via **rustc const-eval** of the generated `Field`/`FieldValue` consts (Codex #3:
  exp01 emits `Field::new(offset,width)` and `FieldValue::new(offset,width,value)`; do **not** assume the old
  2-arg mask/shift syntactic shape — const-eval is robust to the constructor arity).
- **`#[rseq::mmio(id="…")]` field** → an `Mmio<T>` resource + the platform-file binding key (the `id`), recorded
  in the reserved `domain_attrs.field_attrs`.
- **each wrapper-method call site** (`read`/`write`/`modify`/`set` on a `ReadOnly`/`WriteOnly`/`ReadWrite`
  receiver, reached via `Mmio<T>` Deref) → a **normalized register-op fact** carrying `{op kind, resource id,
  block-field id, byte offset, access_width, field mask+shift, write value (const-eval of the FieldValue
  argument), call DefId/Instance, receiver place, span}` (Codex #4). This is what the backend lowers from.
- **`regseq::poll::until`** → the `MmioPollUntil` intrinsic (path match, like diag-log) + its predicate.

**Domain model** (`crates/regseq-model/src/domain_attr.rs`): append-only `AttrData`/`IntrinsicKind` additions
(discriminants serialization-normative) for the register-block, the Mmio resource, the **normalized register-op
facts**, and `MmioPollUntil` — per `register-access-dsl-surface.md` §7.2 (Codex-confirmed against the live
model).

## 5. Compiler path — generic-c lowering (finalized; Codex #1/#6)

Per the locked decision (`register-access-design.md` §6) + the diag-log interception precedent
(`generic_c/body_emit.rs` Call-terminator → `emit_diag_log`, ~:2098): register read/write/modify/poll do
**NOT** lower as plain struct-field projection (registers ≠ cached memory). The backend **intercepts the
normalized register-op fact** (from §4 — NOT by re-tracing places; receiver-place recovery is kept only as an
optional validation/assertion path, Codex #1) and emits **hook-fn calls** against a host device-model:
```c
uint64_t regseq_mmio_r{8,16,32,64}(RegseqDev*, uint32_t off);
void     regseq_mmio_w{8,16,32,64}(RegseqDev*, uint32_t off, uint64_t v);
```
- `regs.fr.read(BUSY::FIELD)` → `(regseq_mmio_r32(dev, FR_OFF) & MASK) >> SHIFT`  (offset+mask from the fact).
- `regs.cr.modify(fv)` → `r` + `(cur & ~mask) | value` + `w` (RMW, exactly the old `reg_modify` shape).
- `regs.dr.write(fv)`/`.set` → `regseq_mmio_w32(dev, DR_OFF, value)`.
- `poll::until(cond, …)` → loop over the read-hook + delay + timeout→`RegseqError`.

**Host device-model + the offset-checking behavioral oracle (Codex #6).** generic-c emits, in the prelude
(Bank 1), a default device-model: a static register-file array whose `regseq_mmio_r/w{8,16,32,64}` **TRACE each
access to stdout** as `W32 @0x024 = 0x0000001a` / `R32 @0x018 = 0x00000000` *and* read/write the backing file.
A write→read-back→`diag-log` spec then produces a deterministic trace; `expected.stdout` asserts the **exact
offsets + values**, so a *wrong offset is caught* (a pure round-trip would not — Codex #6). Seed distinct
sentinel values at distinct expected offsets to make any offset confusion visible. (Richer device models —
fake-UART `DR`→stdout, W1C/read-clear, FIFO — are a follow-on; the tracing passthrough proves the hook lowering
+ offset recovery end-to-end and soundly.)

## 6. Implementation plan (compiler exploration branch `explore/regaccess-lowering`)

Sequential; each step compiles/tests before the next. Build: `cargo build -p cargo-regseq -p regseq-driver`
(toolchain `nightly-2026-02-01`). Emit: `cargo-regseq regseq --platform-file platform.toml --emit
generic-c --output-dir regseq-out` (with `REGSEQ_DRIVER=target/debug/regseq-driver`). *(The
`--target-os` flag was hard-removed in the L2-platform-binding work — target identity now comes solely
from `[platform]` in `platform.toml`; `--emit` selects the backend.)*
Behavioral: `codegen-coverage-runner` 6-stage pipeline on a `<name>/{input.rs,manifest.toml,golden.c,
expected.stdout}` fixture (or a `--emit bin` + run).

1. **Facade crate** `crates/regseq/` (currently empty) — populate the register support types (Form B: `Field`,
   `FieldValue`+`Add`, `ReadOnly`/`WriteOnly`/`ReadWrite` with their methods, `Reserved`, `Mmio<T>: Deref`,
   `poll::until`) sourced from exp01 + full-spec; add the `register_bitfields!` macro (in `regseq-macros`).
   Facade-task 04/22 already specify these. Gate: a fixture using the surface **type-checks**.
2. **Extraction** in `crates/regseq-driver` — port the old extractor; feed `regseq-model`. Gate: `--emit model`
   on a register fixture shows the register block + ops in the model.
3. **Domain-model** additions in `regseq-model`. Gate: model round-trips (serialize) with the new variants.
4. **generic-c lowering** in `regseq-backends/src/generic_c` — the hook calls + offset recovery + the default
   device-model prelude. Gate: `--emit generic-c` on the fixture emits hook calls + compiles with `cc`.
5. **PL011 fixture** — author the register block (subset sufficient for a write→read-back→log proof + a
   `poll::until`), confirm behavioral pass.
6. **E1000 fixture** — second device (PCI BAR0; split-32 ring base; read-clear ICR), confirm lowering.
7. **Fold back** learnings into this doc + `register-access-dsl-surface.md`; Codex-confirm; save handoffs at
   each gate.

## 7. Open items / next

- **Codex-review** this converged surface + plan (foci in §8) BEFORE writing compiler code.
- Then execute §6 steps 1→6.
- Doc consolidation: this doc supersedes the *syntax* of `register-access-dsl-surface.md`; after lowering is
  proven, fold the converged surface back (or mark v1 superseded-by-this).

## 8. Codex-review foci
1. The **Form-B offset-recovery** lowering strategy (§5 crux) — is resolving the receiver place's block-field
   projection → `offsetof` the right tractable approach in this backend, or is there a cleaner one (e.g. carry
   the offset in the descriptor / a normalized intrinsic at extraction time)?
2. **L1/L2 separation** (§1) — base/size/BAR on the `#[rseq::mmio]` resource + platform.toml, never the block.
3. **Model fidelity** — does the converged surface still realize `register-access-model.md` (esp. the
   `Mmio<T>`-only build, composition scope boundary)?
4. **Offset strategy** — layout-derived vs explicit-offset guards (`_Static_assert`).
5. **Extractability** — does Form B give the compiler everything (offset, width, access-mode, side-effects,
   field mask/shift, named values) per §4?

---

## 9. Lowering CONFIRMED — implementation + results (2026-06-04)

Built on the compiler exploration branch `explore/regaccess-lowering`. **Both PL011 and E1000 register-access
surfaces lower through the live generic-c backend, compile with `cc -std=gnu11`, and run with the correct
behavioral trace.** Codex independently re-verified by reading the emitted C and re-running it
(**CLAIM-SOUND-WITH-CAVEATS**).

### What was implemented (live compiler crates)
- **`regseq-model/src/domain_attr.rs`** — `IntrinsicKind::Mmio(MmioOp{Read,ReadField,Write,WriteField,Modify})`
  + `PollUntil` (append-only discriminants); `from_name` vocab; `MmioOp` exported.
- **`regseq-driver/src/bridge/diag_log/recognition.rs`** — the mmio ops are recognized via the **existing
  diag-log `#[rseq::intrinsic("…")]` path** (extended `intrinsic_kind_for_name` + `fn_name_matches_kind`); no
  new bridge module needed for the MVP. *(Mirror of `from_name` — keep in sync.)*
- **`regseq-backends/src/generic_c/prelude.rs`** — a tracing host **device-model** in the Bank-1 prelude:
  `regseq_mmio_read(off,width)` / `regseq_mmio_write(off,width,val)` back a flat LE register file **and print
  `W/R<bits> @0x<off> = 0x<val>`** (the offset-checking behavioral oracle, Codex #6).
- **`regseq-backends/src/generic_c/body_emit.rs`** — `call()` intercepts `IntrinsicKind::Mmio(op)` →
  `emit_mmio`, which reads `(desc).offset/.width/.mask/.shift/.value` off the descriptor argument and emits the
  hook calls. `modify` lowers as RMW in C (`read` + `(cur & ~mask)|value` + `write`) — no generic Rust bit-ops.
- **`regseq-macros/src/register_block.rs`** — the `register_block!` macro threads each register's **byte offset
  + width** into `RegisterId::new` / `Field::new` / `FieldValue::new`, so the offset rides in the descriptor
  value (the backend needs no type-level offset recovery). *(This is the one surface/macro change; it makes the
  offset a property of the descriptor — arguably an improvement. The pre-existing `pl011-full-spec` /
  `log-intrinsic-smoke` fixtures use the old 2-arg constructors and would need their facades updated to match;
  branch-scoped.)*
- **Fixtures** (`compiler:tests/fixtures/`): `regaccess-pl011/` and `regaccess-e1000/` — each a `register_block!`
  block + a Form-A facade (offset-carrying descriptors, intrinsic-tagged ops, `Mmio<T>` carrying a `base`
  field) + a **driver struct with the resource declared** `#[rseq::mmio(id = "regs")] regs: Mmio<…>` joined to
  the platform file's **`[mmio.regs]`** section (PL011 = `base`+`size`; E1000 = `bar = 0`), with compile-time
  **offset guards** (`const _: () = assert!(pl011::fr::OFFSET == 0x18)`, Codex #5). The `platform.toml` follows
  the accepted L2 format (`version` + `[platform]` + the construct-owned `[mmio.<id>]` section). *(The earlier
  throwaway `regaccess-mvp` thin-slice fixture was removed once the two device fixtures subsumed it.)*

### Results (behavioral traces)
PL011 — `write DR='A'` / read-back (round-trips) / `write IBRD` / `modify CR` (RMW) / `read_field FR.BUSY`:
```
W32 @0x000 = 0x00000041   R32 @0x000 = 0x00000041   W32 @0x01c = 0x0000001a
R32 @0x020 = 0x00000000   W32 @0x020 = 0x00000101   R32 @0x018 = 0x00000000
```
E1000 — `modify CTRL` / `read STATUS` / split-64 base (TDBAH **high** then TDBAL **low**) / `TDLEN` /
`modify TCTL` / doorbell `TDT` / read-clear `ICR`:
```
R32 @0x000 ; W32 @0x000 = 0x40 ; R32 @0x004 ; W32 @0x014 = 0x1 ; W32 @0x010 = 0x2000
W32 @0x018 = 0x80 ; R32 @0x00c ; W32 @0x00c = 0x2 ; W32 @0x01c = 0x1 ; R32 @0x008
```
Offsets are correct end-to-end (the macro computes them from declaration order + `Reserved`); round-trip, RMW,
masked field-read, and split-64 high-then-low all verified. Run via `cargo-regseq … --emit generic-c` →
`cc -std=gnu11 generic.c main.c` → run.

### Honest scope & caveats (Codex-flagged)
- **Exercised ops:** `read`, `read_field`, `write`, `modify` (PL011 + E1000). **`write_field`** is implemented
  in the backend but **not exercised** by these fixtures. **`poll_until`** is *recognized* (an `IntrinsicKind`)
  but its **generic-c lowering is not yet implemented** (no `emit` case) — do not claim it.
- **Host-backend scoping (not a hardware-binding proof):** `Mmio<T>` is a ZST and the device model is a single
  **global** register file (no real base) — a real target binds a base via the lifecycle/`map_region` seam
  (out of scope for register-access). E1000 offsets are **compact** in the fixture (real addresses noted in
  comments) to keep it small; absolute offsets don't change what the lowering proves.
- **Recognition reuse** lives in the `diag_log` module (a future refactor could split a `register_access`
  bridge module). Dead intrinsic-stub bodies are still emitted (harmless; `-Wall` noise).
- This proves **generic-c** lowering (the design crucible / behavioral oracle). Linux/Zephyr lowering of these
  same ops is per the lowering tables in `register-access-design.md` §5 (not built here).

### Form A vs Form B — Form B now CONFIRMED (2026-06-04)

The original proof used **Form A** (`regs.read(REG)` / `write(REG,v)` / `modify(REG,fv)`) as the readiest path.
Form B — the **recommended** author surface (§2, wrapper-via-`Deref`) — has now itself been **lowered, compiled,
and run**, with traces **identical to the Form A baseline** for PL011 and E1000 (+ an all-ops coverage fixture),
and **Codex-independently re-verified** (`CONFIRMED-WITH-CAVEATS`, both caveats then hardened). Form A is fully
**superseded**: the compiler now lowers only the Form B (wrapper-receiver) surface.

**Locked operation naming** (the converged surface; `set`/`get` dropped):

| Op | wrapper kinds | meaning |
|---|---|---|
| `read() -> T` / `write(v: T)` | RO/RW · WO/RW | whole-register **raw** |
| `read_field(F) -> T` / `write_field(FV)` | RO/RW · WO/RW | masked field (write: bits outside the mask = 0) |
| `modify(FV)` | RW | field RMW (preserves unmentioned bits) |

This **resolves the §3 `[Open]` `set`-vs-`write` distinction**: there is no `get`/`set`. Whole-register raw
access is `read()`/`write()`; field access is the `_field` forms; RMW is `modify` (field-scoped only — a
*whole-register* `modify` is degenerate: supplying all bits = `write`, and a closure-transform = the dropped
`update`, so whole-register RMW is `read()`→compute→`write()`). Name-based op dispatch (vs tock's type-overloaded
`get`/`set`) also matches the recognizer, which keys on method name.

**The offset-recovery mechanism was deeper than "a recognition-site delta."** Recognition *is* callee-attribute-
based, so tagging the wrapper methods reused that path. But in Form B the register is the **receiver** (`regs.dr`,
via `Mmio<T>: Deref`), so whole-register `read()`/`write()` carry **no descriptor argument** — the register byte
offset can no longer ride in a token. It is recovered **from the `#[repr(C)]` block layout at emit time**
(**Strategy C**, layout-derived + backend-local): the receiver `args[0]` is a borrow `_t = &(*deref).field` whose
place projects `Field(idx)` into the block; the backend back-traces that borrow (whole-body single-assignment
reaching def — the receiver borrow can land in a predecessor block when a descriptor argument like
`FieldValue::new(..)` or the `+`-compose splits the block), reads `layout.fields[idx].offset` / `.size`, and emits
the hook with a literal offset/width. Descriptors therefore **drop the register offset** and carry only
register-relative `mask`/`shift`/`value`. Codex foundation-reviewed the approach (`SOUND-WITH-CHANGES`: bounded
reaching-def chase, width guards `∈{1,2,4,8}`, strict prefix-type walk, fail-loud on every malformed shape).

The descriptor-offset-in-the-token approach (Form A) and the bridge-normalization ideal (a `RegisterAccessSurface`
fact pass, Codex #1) both remain valid alternatives; Strategy C is the bounded first realization for the generic-c
oracle (graduate to a bridge pass if multiple backends ever need register facts). Compiler branch
`explore/regaccess-lowering`; fixtures `regaccess-{pl011,e1000,formb-ops}`.

## 10. Register arrays (the A3 deferral) — IMPLEMENTED (2026-06-04)

The model always admitted **register arrays** (`register-access-model.md` §"Register definitions": *both single
registers and register arrays at a stride*); they were the **A3 deferral** (E1000-witnessed: `MTA[128]`/`VFTA[128]`
@4 B, `RA[15]` @8 B-pairs — adequacy doc `register-access.md`). Now implemented + proven, completing E1000.

- **Surface = native indexing** (tock-style, Form-B-consistent — chosen over the old `RegArray<T>`/`.mta(i)`
  proposal, Codex-endorsed): a contiguous array is a block field `[Wrapper<W,R>; N]` accessed `regs.mta[i].write(v)`;
  a strided/paired array (RA) is an array of a `#[repr(C)]` element struct, `regs.ra[i].ral.write(v)`. Every op
  (`read`/`write`/`read_field`/`write_field`/`modify`) works on an indexed element.
- **Macro syntax** — `name[N]: AccessKind { fields }` (contiguous) and `name[N]: { sub: AccessKind { fields }, … }`
  (struct array → a generated `Entry` element + per-sub-register modules). `OFFSET` guards now align to each field's
  `#[repr(C)]` alignment so they stay in step with the real layout even for mixed-width blocks.
- **Mechanism** — the **same Strategy C** receiver back-trace, with `recover_register` generalized to a
  **left-to-right projection chain walk** carrying a running type and accumulating a byte-offset **expression**:
  `Field` += `fields[idx].offset` (const), `Index(local)` += `(uint64_t)(i) * stride` (stride from the resolved
  array layout), width = the final element size. So `regs.mta[i]` → `(base + i*4)`, `regs.ra[i].ral` →
  `(base + i*8 + 0)` — uniformly handling single register / array / array-of-struct.
- **Bounds-checking is free + memory-safe** — `regs.mta[i]` emits Rust's array-bounds `Assert` in the caller MIR
  (lowered by the backend's existing assert path), so an out-of-range index panics like any Rust array; the assert
  block precedes every array-element mmio in the emitted C.
- **Proven** — E1000 `MTA[8]`/`VFTA[8]` (contiguous) + `RA[4]` (RAL/RAH pairs, incl. a `write_field` on `RAH.AV`),
  all with runtime indices and correct offsets; a u8-width block and a mixed-width alignment guard (`Reserved<u8>`
  before a u32 register → offset 8) independently verified. **Codex-reviewed**: plan `SOUND-WITH-CHANGES` (folded
  in), implementation `SHIP-WITH-NITS` — the one nit (mixed-width `OFFSET`-guard alignment) then fixed.
- **Known limitation** — native `[Wrapper; N]` is inherently **contiguous** (stride = element size); a non-contiguous
  logical stride is modeled by the struct-element form (RA), not an explicit-stride escape. Mediated transports and
  variant layouts remain deferred.
