# Stage 4 Status

Status: **OPEN -- D-4-1 LOCKED, D-4-2 LOCKED.** D-4-1 closed
at APPROVE on 2026-05-02 across 6 Codex review rounds (20
findings closed). D-4-2 closed at APPROVE on 2026-05-03 across
7 Codex review rounds (16 findings closed). Next: D-4-3+
(reference-typed const handling -- `SyntheticItemKind::ConstMemoryStatic`
family extension; intersects stage-12 territory).

Last updated: 2026-05-03.

For top-level project status, see `../status.md`. For project-wide
invariants, see `../README.md`. For stage-4 invariants (workflow,
principles, read order, Codex prompt template), see `memory.md`.
For the working spec, see `spec-snapshot.md`.

## Mission

Stage 4 specifies the `Const` enum and `ConstIdentityKey` schema
that the bridge fills from rustc-evaluated const values, and the
two-phase validation discipline (`type-level` before `TyInterner`;
`value-level` after `TyInterner` + `instances`) that resolves the
dependency cycle stage 3 deferred. See `memory.md` Section 0 for
the full mission framing.

## Inbound expectations (from `expectations.md`)

Four items, all from stage 1:

- [x] **Pin canonical const-eval result form.** Closed by
  D-4-1 (parent + .5 + .7 — variant set with inline scalar tags
  pinning the type-level subset; `InstanceRef` 4-tuple for
  cross-build relocation identity) and D-4-2 (per-`Const`-
  variant byte-encoding tags closing the F4-2 fingerprint via
  the concrete `ConstIdentityKey` byte form).
- [x] **Cover scalars, byte arrays, aggregate constants
  (struct / array / enum variants), and function pointers as
  constants.** Closed by D-4-1 — 11-variant `Const` enum
  covers all listed surfaces. Reference-typed `&str` / `&[T]`
  remains stage-4-deferred (see "Stage-4-specific deferred
  items" below).
- [ ] **Fill `pretty.rs::render_const` stub.** Open. Renderer-
  style decision will be tagged D-4-K (post-D-4-2) or
  deferred to a later stage following the D14 → stage-13
  precedent if the renderer's input depends on content not
  yet pinned.
- [x] **Preserve `Option<ConstId>` 4-byte niche optimization.**
  Closed by D-4-1 (`ConstId = u32` with `NonMaxU32` niche; no
  variant payload widens `ConstId`).

## Cluster split

**SOLO** for both D-4-1 and D-4-2.

D-4-1 sub-decisions 1.1-1.9 are tightly coupled (variant set ↔
identity-key shape ↔ encoding ↔ uninit/aggregate distinction).

D-4-2 sub-decisions 2.1-2.9 are tightly coupled per the same
pattern (variant tag values ↔ payload byte forms ↔ length
prefixes ↔ recursion / acyclicity rules). One converging
spec-snapshot kept the Codex review loop cleanly traversable;
the alternating BLOCK / MINOR trajectory (R1=BLOCK → R2=MINOR
→ R3=BLOCK → R4=BLOCK → R5=MINOR → R6=MINOR → R7=APPROVE)
matches D-4-1's pattern of deeper findings surfacing as
cumulative consistency tightens.

Stage 3's parallelisation was justified by 11 decisions
splitting into independent clusters; stage 4's sub-decisions
fit one cohesive cluster per cluster. If a later sub-decision
(e.g., the deferred renderer-style decision or D-4-3+
reference-typed handling) surfaces a different evidence base,
late split is tractable per stage-3 precedent.

## Locked decisions

- **D-4-2** (2.1-2.9): `ConstIdentityKey` per-variant byte-form
  schema. Locked across 7 Codex review rounds (R1-R7), 16
  findings closed.
  - **D-4-2.1** -- Variant tag: `u8` width, contiguous
    `0..=10` numeric assignment in source-declaration order
    (`Bool=0`, `Char=1`, `Int=2`, `Uint=3`, `Float=4`, `ZST=5`,
    `Bytes=6`, `Aggregate=7`, `EnumValue=8`, `NullPtr=9`,
    `Reloc=10`); append-only stability.
  - **D-4-2.2** -- Scalar payload byte forms: per-variant
    `u8` kind discriminant (from §10's `#[repr(u8)]` enums)
    plus value-bits in `u128` LE for `Int` / `Uint` / `Float`
    (small widths sign- or zero-extended; floats have IEEE
    bit-pattern in low-order bits, high-order zero); `Bool`
    is 1 byte; `Char` is `u32` LE.
  - **D-4-2.3** -- Length-prefix width: `u32` LE for
    `Vec<Byte>`, `Vec<ConstId>`, and `EnumValue.payload`,
    matching §12.3's `GenericArgs` container precedent.
  - **D-4-2.4** -- `EnumValue.variant_idx` encoding: `u32`
    LE, placed before the `payload` length prefix.
  - **D-4-2.5** -- `Byte` sub-discriminator: `u8` sub-tag
    (`Uninit=0`, `Value=1`, `Reloc=2`) with tag-specific
    payload.
  - **D-4-2.6** -- Aggregate / EnumValue recursion +
    acyclicity: depth-first inline recursion via
    `canonical_const(ConstId)`; three-state DFS marks
    (`Unseen` / `Visiting` / `Done`) for cycle detection
    in §3.3 step 9; value-level lookup injectivity ordered
    after acyclicity.
  - **D-4-2.7** -- `NullPtr(TyId)` encoding: recursive
    `canonical_ty(TyId)` per §12.3 walk rule.
  - **D-4-2.8** -- `Reloc(InstanceRef)` byte layout: 34
    bytes total in struct-declaration order
    (`u8 regseq_kind_rank` + 16-byte `def_path_hash` +
    16-byte `args_key.to_be_bytes()` + `u8 fn_shim_disambig`);
    `args_key` is the documented BE exception to §12.3's
    general LE rule.
  - **D-4-2.9** -- `ZST` identity: tag-only (single byte,
    no payload).

D-4-2's contribution also includes:
- Per-`Const`-variant byte-encoding tags subsection in §9.7
  (parallel to §12.3's per-`Ty` table).
- §12.3 `ConstId` walk-rule row updated from stage-4-schema
  stub to concrete recursion rule pointing at §9.7.
- §3.3 Phase B step 9 extended with const-graph acyclicity
  sub-bullet (three-state DFS) and value-level lookup
  injectivity sub-bullet (after acyclicity).
- §3.3 Phase B step 4 scoped to type-level subset only;
  full `ConstInterner.lookup` injectivity completed by
  step 9's value-level pass.
- §9.7 `Const validation phase split` subsection refined
  to reflect the four-sub-step ordering of step 9.

- **D-4-1** (1.1-1.9): `Const` enum variant set. Locked across
  6 Codex review rounds (R1-R6), 20 findings closed.
  - **D-4-1.1** -- Scalar grouping: per-type variants `Bool(bool)`,
    `Char(u32)`, `Int(IntKind, u128)`, `Uint(UintKind, u128)`,
    `Float(FloatKind, u128)`. Bit-pattern in low-order bits;
    high-order bits zero for narrow widths.
  - **D-4-1.2** -- ZST canonicalisation: single `ZST` variant
    (no payload); type from consumer context.
  - **D-4-1.3** -- Aggregate field elements:
    `Aggregate(Vec<ConstId>)` with interned children. Storage
    shape; canonical identity recursively uses children's
    canonical content per §12.3.
  - **D-4-1.4** -- Enum payload:
    `EnumValue { variant_idx: VariantIdx, payload: Vec<ConstId> }`.
    Discriminant implicit via consumer's `Ty::Enum`'s
    `VariantLayout.discriminant`.
  - **D-4-1.5** -- Relocation reference: `InstanceRef` 4-tuple
    `(regseq_kind_rank: u8, def_path_hash: DefPathHash,
    args_key: ArgsKey, fn_shim_disambig: u8)`. Same shape as
    §9.3's `RegseqInstanceKey` minus the local `ItemId`
    (replaced with `def_path_hash` for cross-build stability).
  - **D-4-1.6** -- Null pointer typing: `NullPtr(TyId)` per
    pointee type.
  - **D-4-1.7** -- Type tag placement: inline-only; no outer
    `Const { ty, kind }` wrapper.
  - **D-4-1.8** -- Uninit / byte-level handling:
    `Bytes(Vec<Byte>)` with `Byte = Uninit | Value(u8) |
    Reloc(InstanceRef)` (charon-inspired three-state).
    Maximal-relocation-run well-formedness invariant.
  - **D-4-1.9** -- Aggregate first-class (not demote-to-static).
    Bridge picks `Bytes` vs `Aggregate` based on rustc's
    const-eval shape (provenance/relocations vs structured
    valtree).

D-4-1's contribution also includes:
- `ArgsKey::EMPTY` constant in §12.3 (for non-generic
  Static / VTable instance projections).
- `GenericArgs container encoding` rule in §12.3 (length
  prefix + N entries with `GenericArg::{Type, Const}`
  per-entry tags). Closes the `canonical_args(GenericArgs)`
  references in the per-`Ty` table.
- New step 9 in §3.3 Phase B (value-level const validation),
  with the previous step 9 (vtable rebuild) renumbered to
  step 10. Updated dependency-arrow summary
  `consts (type-level) ← sigs ← types ← (layout, instances) ←
  consts (value-level)`.

## Open decisions (post-D-4-2)

- **D-4-3+** -- Reference-typed const handling
  (`SyntheticItemKind::ConstMemoryStatic` family extension).
  See "Stage-4-specific deferred items" below.
- **D-4-K (open)** -- `pretty.rs::render_const` renderer style.

## Codex review cadence (status)

- **D-4-1 cluster** (initial review, 2026-05-02): closed at
  R6=APPROVE.
  Trajectory:
  - R1: 10 issues (7 BLOCK / 3 MINOR). All closed by R2.
  - R2: 5 issues (2 BLOCK / 3 MINOR). All closed by R3.
  - R3: 2 issues (1 BLOCK / 1 MINOR). All closed by R4.
  - R4: 1 MINOR (cross-reference correctness). Closed by R5.
  - R5: 1 MINOR (pre-existing float-padding wording
    inconsistency surfaced as deeper-residual). Closed by R6.
  - R6: APPROVE 0.

- **D-4-2 cluster** (initial review, 2026-05-03): closed at
  R7=APPROVE.
  Trajectory:
  - R1: 6 issues (2 BLOCK / 4 MINOR). All closed by R2.
  - R2: 4 MINOR (residual cleanup of R1 fixes that did not
    propagate to all parallel mentions). All closed by R3.
  - R3: 1 BLOCK (validation-order-before-acyclicity:
    recursive value-level key computation was scheduled
    before the acyclicity check that makes it safe). Closed
    by R4.
  - R4: 1 BLOCK + 1 MINOR (R3 phase-split refactor did not
    propagate to two parallel mentions). All closed by R5.
  - R5: 1 MINOR (`canonical_const_repr` → `canonical_const`
    naming inconsistency surfaced). Closed by R6.
  - R6: 2 MINOR (cross-reference inaccuracies in §12.3's
    `GenericArg::Const` payload note and D-4-2 parent's
    `Array` cross-ref). All closed by R7.
  - R7: APPROVE 0.

Round-by-round audit trail with verdicts, issues, and
corrections: `review-log.md`.

Prompt template: see `memory.md` Section 4.

## Stage-4-specific deferred items

- **Reference-typed const handling for `&str` / `&[T]`.**
  D-4-1's bridge construction explicitly defers
  `mir::ConstValue::Slice { data, meta }` and
  `mir::ConstValue::Indirect` arms targeting
  `GlobalAlloc::Memory(_)`. Closing the case requires:
  - extending §13.3's `SyntheticItemKind` universe with a new
    family `ConstMemoryStatic { content_hash }` (or similar
    name) to bridge-synthesise an immutable `Static` Item per
    unique memory-content blob,
  - extending §13.4's synthesised-DefPath construction with a
    memory-content-keyed segment encoding so two builds with
    identical byte content produce the same synthesised
    `DefPathHash`,
  - extending §3.3 Phase B's value-level const validation step
    (the new step 9 stage 4 introduces) with a duplicate-
    content-hash check against the materialised
    `ConstMemoryStatic` Items,
  - extending the §9.7 bridge construction table with the
    `Slice` / `Indirect`-via-`Memory` dispatch arms producing
    `Aggregate([Reloc(memory_static_inst_ref), Uint(Usize,
    meta)])`.
  Filed for a follow-on D-4-K decision (likely D-4-3 or
  later, depending on whether D-4-2 covers it as part of the
  full identity-key schema). Outbound expectation filed in
  `stage-12/expectations.md` (since §13.3 / §13.4 changes
  intersect stage-12 territory).

## Next concrete unit of work

1. **Frame D-4-3+** -- Reference-typed const handling
   (`SyntheticItemKind::ConstMemoryStatic` family extension).
   This is the largest remaining stage-4 piece; it intersects
   stage-12 territory (§13.3 `SyntheticItemKind` universe;
   §13.4 synthesised-DefPath construction). Outbound
   expectation already filed in `stage-12/expectations.md`.
2. **Frame D-4-K** -- `pretty.rs::render_const` renderer style.
   May be deferable to a later stage following the D14 →
   stage-13 precedent if the renderer's input depends on
   content not yet pinned.
3. Lay out options + recommendation + sub-questions per
   Phase A; edit `spec-snapshot.md` and `discussions.md`;
   run Codex review loop to APPROVE; lock and commit.
