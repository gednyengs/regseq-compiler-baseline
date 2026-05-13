# Expectations on stage 16 (incoming)

Stage 16 covers the **MMIO family**: `mmio_*` intrinsic
vocabulary entries, the `Mmio<T>` resource type, the
`#[rseq::mmio(...)]` declarative attribute family,
volatility codegen rule, Linux-backend MMIO lowering
(`readl` / `writel` / `__iomem` qualification),
bridge-side rejection of
`core::ptr::{read,write}_volatile`, and the `RegisterId`
typed-index.

The convention is described in `../README.md` under "Forward
expectations live in `stage-N/expectations.md`."

**Provenance note.** Most items below were originally filed
against the monolithic "Stage 14: Domain-specific
extensions" plan; the 2026-05-12 strategy revision
relocated them to this phase-2 stage. The `Mmio<T>` resource
+ `#[rseq::mmio(...)]` attribute items are new -- introduced
by the strategy revision to scope the MMIO family stage. See
`../stage-14/expectations.md` for the foundation items that
stayed at stage 14 and the strategy-revision context.

## From stage 1 (relocated 2026-05-12)

- **`RegisterId` typed-index** (and related domain-specific
  IDs). Per `../stage-1/spec-snapshot.md`'s "IDs added at
  later stages" note, stage 16 introduces additional typed
  IDs as the MMIO surface needs them. They follow the same
  conventions (`newtype_index!` macro, niche-encoded u32,
  hand-written serde, single namespace per kind). The
  `RegisterId` typed-index identifies a single register
  within a register bank.

## From stage 3 (relocated 2026-05-12)

- [ ] **MMIO intrinsic vocabulary entries** (from stage 3
  D9).

  *Stage-3 commitment.* D9 locks volatility on the
  intrinsic `InstanceId` (no `Ty` bit, no Statement flag);
  MMIO access in the regseq DSL flows through
  `#[rseq::intrinsic("mmio_*")]` exclusively. See
  `../stage-3/spec-snapshot.md` Section 1.13 (D9 audit
  shape) and Section 4.11 (worked example).

  *Stage-16 deliverable.* Add the MMIO intrinsic family to
  the closed vocabulary list maintained per stage-14's Q7
  mechanism. Concrete names from the pl011 fixture
  (`tests/fixtures/pl011-full-spec/src/regseq/mmio.rs:35-71`):
  - `mmio_read`
  - `mmio_write`
  - `mmio_read_field`
  - `mmio_write_field`
  Linux-backend codegen for these `InstanceId`s emits
  volatile-marked C accesses
  (`*(volatile uint32_t*)addr` and the write counterpart,
  or `readl` / `writel` family). The volatility is a
  property of the InstanceId's emission, not an IR-level
  flag.

- [ ] **Bridge-side rejection of
  `core::ptr::{read,write}_volatile`** (from stage 3 D9).

  Enforce D9's Call-translation-phase rejection: raw
  `core::ptr::read_volatile` and
  `core::ptr::write_volatile` calls are out-of-DSL-scope
  and must be rejected at the bridge with a diagnostic
  pointing to the `#[rseq::intrinsic("mmio_*")]` API. Same
  posture as D1.4 Category A rejections, applied to
  function calls instead of types.

## New (introduced by 2026-05-12 strategy revision)

- [ ] **`Mmio<T>` resource type.** Stage 16 specifies the
  `Mmio<T>` resource type that drivers use to represent
  memory-mapped register banks. Spec includes:
  - Rust shape: likely a `repr(C)` wrapper around a base
    address + the parameterized register-bank type T (or
    a phantom-typed wrapper that only carries the type
    parameter while the address lives in a sibling
    field). Exact shape TBD by stage 16.
  - How `#[rseq::mmio(...)]` declarative attributes
    attach to fields of T to declare access width,
    offsets, and read-write semantics.
  - Linux-backend lowering rule: `memremap` / `ioremap`
    mapping at probe time; `__iomem` qualification of
    generated C pointers; how `Mmio<T>` instances acquire
    their base address (from the probe context's
    resource map, populated by DT-binding stage-15
    machinery).
  - Composition with `mmio_read` / `mmio_write` /
    `mmio_read_field` / `mmio_write_field` intrinsics
    from the vocabulary above.
  - How `Mmio<T>` instances are passed around (by
    reference per the C-ABI rule, since `Mmio<T>` is
    likely non-Copy and contains an `UnsafeCell<T>` to
    represent the mutable hardware-backed register
    bank).

- [ ] **`#[rseq::mmio(...)]` declarative attribute
  family.** Stage 16 specifies the attribute syntax and
  semantics. Candidate parameters:
  - `access_width = 32` -- the access width in bits (8,
    16, 32, 64).
  - `offset = 0x18` -- byte offset within the register
    bank.
  - `read_only = true` / `write_only = true` -- access
    direction constraints; bridge enforces violations as
    rejected DSL.
  - Reset value, side-effect documentation, etc. (TBD by
    stage 16).
  The attribute attaches to fields of the register-bank
  type; the bridge parses the attribute into a
  structured `DomainAttr` per stage-14's sibling-table
  architecture (key kind: `FieldPath`).

- [ ] **Field-level MMIO bit-field attributes.** For
  composite registers with sub-fields (e.g., a status
  register with separate bits for busy / error / ready),
  stage 16 specifies whether sub-field attributes are
  inline within `#[rseq::mmio(...)]` or are a separate
  attribute family (e.g.,
  `#[rseq::mmio_field(name = "busy", bit = 5)]`).
  Composition with `mmio_read_field` /
  `mmio_write_field` intrinsics determines the
  bit-extraction / bit-insertion codegen.

## From stage 14 (filed 2026-05-13, per D-14-10)

- **MMIO worked example dropped from stage-14 Appendix
  A.** D-14-10 carved out the worked C-lowering examples
  (the former Appendix A.1-A.11 in stage-14
  spec-snapshot.md) into `generic-c-backend.md` §9. The
  Linux-target example A.8 (`mmio_read` intrinsic
  lowering to `readl((volatile void __iomem *)(base +
  offset))`) was dropped rather than moved, per "no
  forward-looking placeholders" (README §5.9 rule 4).
  Stage 16 (MMIO family) owns deriving:
    - The closed `IntrinsicKind::Mmio(_)` vocabulary
      (read / write / read_field / write_field / etc.) per
      the Q7 intrinsic recognition mechanism.
    - The Linux backend's MMIO lowering (`readl` /
      `writel` / `readl_relaxed` / barrier composition) in
      the future `linux-backend.md` merger artifact.
    - The generic-C backend's MMIO lowering -- if MMIO
      intrinsics are admissible on the hosted-userspace
      generic-C target at all (TBD; likely confined to a
      stub harness that traces ops to stdout for the
      DSL -> IR -> C -> captured-stdout test suite).
  - Cross-ref: `../stage-14/spec-snapshot.md` Appendix A
    (carve-out pointer to `generic-c-backend.md` §9);
    `../spec.md` §13.2 + Section 1.13 (D9) volatility on
    `InstanceId` codegen commitment.

## Cross-refs

- `../stage-14/expectations.md` -- foundation items (Q7
  intrinsic recognition mechanism + DomainAttr
  sibling-table architecture) that this stage builds on.
- `../stage-15/expectations.md` -- `Lifecycle` trait +
  driver-struct discovery + DT-binding + `RegseqError`
  -- drivers using MMIO compose with these (probe-time
  resource acquisition, `Lifecycle::probe()` returns
  `Result<(), RegseqError>`).
- `../stage-3/spec-snapshot.md` Section 1.13 (D9) --
  volatility-on-InstanceId commitment.
- `tests/fixtures/pl011-full-spec/src/regseq/mmio.rs` --
  reference MMIO intrinsic surface from the pl011
  fixture.
