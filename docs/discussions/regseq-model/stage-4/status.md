# Stage 4 Status

Status: **OPEN -- D-4-1 LOCKED.** First sub-decision cluster
closed at APPROVE on 2026-05-02 across 6 Codex review rounds
(20 distinct findings closed). Next: D-4-2 (`ConstIdentityKey`
per-variant canonical key shape).

Last updated: 2026-05-02.

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
  cross-build relocation identity). The full F4-2 closure
  also depends on D-4-2's per-variant `ConstIdentityKey` byte
  form; the *form* is canonical, the *byte serialisation*
  pins in D-4-2.
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

**SOLO.** D-4-1 sub-decisions 1.1-1.9 are tightly coupled (variant
set ↔ identity-key shape ↔ encoding ↔ uninit/aggregate
distinction). Stage 3's parallelisation was justified by 11
decisions splitting into independent clusters; stage 4's
sub-decisions fit one cohesive cluster. Solo kept the prior-art
evidence base in active context across the full sequence; the
Codex review loop ran cleanly against one converging
spec-snapshot. If a later sub-decision (e.g., the deferred
renderer-style decision) surfaces a different evidence base,
late split is tractable per stage-3 precedent.

## Locked decisions

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

## Open decisions (post-D-4-1)

- **D-4-2** -- `ConstIdentityKey` per-variant canonical key shape.
  Depends on D-4-1 variant set; pins the byte-level
  serialisation order for each variant's identity key (e.g.,
  for `Bytes(Vec<Byte>)` the per-`Byte` discriminant + payload
  order; for `Aggregate(Vec<ConstId>)` the recursive child-
  resolution rule's exact byte form).
- **D-4-3+** -- Reference-typed const handling
  (`SyntheticItemKind::ConstMemoryStatic` family extension).
  See "Stage-4-specific deferred items" below.
- **D-4-K (open)** -- `pretty.rs::render_const` renderer style.

## Codex review cadence (status)

- **Round 1** (D-4-1 initial review, 2026-05-02): closed at
  R6=APPROVE.
  Trajectory:
  - R1: 10 issues (7 BLOCK / 3 MINOR). All closed by R2.
  - R2: 5 issues (2 BLOCK / 3 MINOR). All closed by R3.
  - R3: 2 issues (1 BLOCK / 1 MINOR). All closed by R4.
  - R4: 1 MINOR (cross-reference correctness). Closed by R5.
  - R5: 1 MINOR (pre-existing float-padding wording
    inconsistency surfaced as deeper-residual). Closed by R6.
  - R6: APPROVE 0.
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

1. **Frame D-4-2.** Identity-key shape per variant (Bool /
   Char / Int / Uint / Float / ZST / Bytes / Aggregate /
   EnumValue / NullPtr / Reloc). Depends on D-4-1 variant
   set (locked); naturally extends the `ArgsKey` and
   `canonical_args(GenericArgs)` work in §12.3.
2. Lay out options + recommendation + sub-questions in
   conversation per Phase A.
3. Edit `discussions.md` with audit-shape decision text;
   edit `spec-snapshot.md` §9.7 with concrete byte-level
   identity-key forms per variant.
4. Run Codex review against the cluster; iterate to APPROVE.
5. Continue with D-4-3+ (reference-typed const handling
   `ConstMemoryStatic` family extension) and the renderer-
   style decision.
