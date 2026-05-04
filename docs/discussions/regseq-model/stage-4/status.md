# Stage 4 Status

Status: **CLOSED (spec-side) -- D-4-1 / D-4-2 / D-4-3 / D-4-K LOCKED.**
All four stage-1 inbound expectations are closed.
D-4-1 closed at APPROVE on 2026-05-02 across 6 Codex review
rounds (20 findings closed). D-4-2 closed at APPROVE on
2026-05-03 across 7 Codex review rounds (16 findings closed).
D-4-3 closed at APPROVE on 2026-05-03 across 4 Codex review
rounds (10 findings closed). D-4-K closed at APPROVE on
2026-05-03 across 4 Codex review rounds (7 findings closed) --
deferred `pretty.rs::render_const` to stage 14 (extending the
D14 → stage-13 precedent for `render_ty` one stage further
because `render_const`'s dependency chain is longer: 5 of 11
variants depend on `render_ty`; `Reloc` additionally depends
on `render_def_path` + `render_instance`, and stage 14 is the
latest of those). Stage 4's implementation-side work
(filling `crates/regseq-model/src/` with the locked design)
follows separately.

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
  closed by D-4-3 via the
  `SyntheticItemKind::ConstMemoryStatic` family extension and
  compositional `Aggregate([Reloc, Uint])` / `Reloc(_)`
  dispatch arms.
- [x] **Fill `pretty.rs::render_const` stub.** Closed by D-4-K
  via deferral to stage 14 (extending the D14 → stage-13
  precedent for `render_ty` one stage further because
  `render_const`'s dependency chain is longer). Of the 11
  `Const` variants: 5 are self-contained scalars; 5 depend on
  `render_ty` for rendering (stage 13); `Reloc` additionally
  depends on `render_def_path(ItemId)` (stage 12) and
  `render_instance` (stage 14). Stage 14 is the latest of
  those dependencies, so `render_const` lands there as a
  complete implementation. Stub message updated to "stage 14
  fills render_const (deferred per D-4-K)"; outbound
  expectation filed in `stage-14/expectations.md`
  "From stage 4."
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
(e.g., the deferred renderer-style decision D-4-K) surfaces a
different evidence base, late split is tractable per stage-3
precedent.

## Locked decisions

- **D-4-K**: `pretty.rs::render_const` renderer-style decision.
  Deferred to stage 14 (extending D14 → stage-13 precedent
  one stage further per the longer dependency chain). Locked
  across 4 Codex review rounds (R1-R4), 7 findings closed.
  Single-decision cluster (no sub-decisions). Of the 11 `Const`
  variants: 5 self-contained scalars; 5 depend on `render_ty`
  (stage 13); `Reloc` additionally depends on
  `render_def_path(ItemId)` (stage 12) and `render_instance`
  (stage 14). Outbound expectation filed in
  `stage-14/expectations.md` "From stage 4"; cross-reference
  in `stage-13/expectations.md` top-of-file note;
  `stage-4/expectations.md` render_const inbound flipped to
  `[CLOSED by D-4-K via deferral to stage 14]`;
  `spec-snapshot.md` §18.2 per-method ownership table
  refined.

- **D-4-3** (3.1-3.10): Reference-typed const handling --
  `SyntheticItemKind::ConstMemoryStatic` family. Whole-family
  scope (stage-4 + stage-12 §13). Locked across 4 Codex review
  rounds (R1-R4), 10 findings closed.
  - **D-4-3.1** -- Family name + payload:
    `SyntheticItemKind::ConstMemoryStatic { content_hash:
    ContentHash }`.
  - **D-4-3.2** -- `ContentHash` construction: SHA-256
    truncated to leading 16 bytes BE-loaded into `u128`,
    parallel to `ArgsKey` / `ProjectionBindingsKey`. Canonical
    input = the canonical D-4-2 `Const::Bytes(Vec<Byte>)`
    byte form of the memory allocation.
  - **D-4-3.3** -- §13.3 variant placement: append after
    `VTableDropShim` (append-only stability).
  - **D-4-3.4** -- §13.4 segments: three-segment form
    `["<regseq_synth>", "const_memory_static", <content_hash
    hex>]` (mirrors `VTableType` depth).
  - **D-4-3.5** -- §13.4 DefPathHash payload: just the
    16-byte content hash (`content_hash.0.to_be_bytes()`).
  - **D-4-3.6** -- Bridge synthesis rule: lazy on dispatch;
    synthesised Item carries `ItemKind::Static` + corresponding
    Instance with `kind: InstanceKind::Static { ty: array_ty,
    init: Some(c), mutability: Mutability::Not }`. Per-kind
    rich data lives on the Instance per stage-1 schema.
  - **D-4-3.7** -- Synthesised Static's Ty + Mutability:
    `Ty::Array(Ty::Uint(U8), N)` (type-erased to bytes for
    content-keyed dedup); `Mutability::Not` (matches
    `VTableInstance` precedent).
  - **D-4-3.8** -- Bridge dispatch arms: `Slice { data, meta }`
    → `Aggregate([Reloc, Uint(Usize, meta)])` (wide pointer);
    `Indirect { alloc_id, offset = 0 }` over Memory →
    `Reloc(synth_static_inst_ref(alloc_id))` (thin pointer);
    `Indirect { alloc_id, offset != 0 }` over Memory →
    bridge-rejected (no slot reserved).
  - **D-4-3.9** -- §3.3 Phase B step 9 dedup check:
    recompute `ContentHash` from each synth Static's
    `init: Some(c)` content; verify match with stored
    `content_hash` field; verify no two `ConstMemoryStatic`
    Items share a `content_hash`. Ordered after value-level
    lookup injectivity pass.
  - **D-4-3.10** -- Stage-12 outbound closure: flip
    `stage-12/expectations.md` "From stage 4" entry to
    `[CLOSED by D-4-3]` (bookkeeping).

D-4-3's contribution also includes:
- §13.3 `ContentHash` type + canonical-encoding subsection +
  collision honesty note (parallel to `ArgsKey` /
  `ProjectionBindingsKey`).
- §13.4 const-memory family entries: segments row, canonical-
  payload-bytes row, ownership rule (`primary_crate`).
- §9.7 "Reference-typed const handling (D-4-3)" subsection
  replacing the prior deferred subsection (lazy synthesis
  rule + worked examples + non-zero-offset rejection +
  pointee-type-at-use-site paragraph).
- §9.7 bridge dispatch table extended with `Slice` and
  `Indirect`-over-Memory rows; `Memory(memory)` GlobalAlloc
  projection row updated from "bridge-rejected" to the full
  4-tuple synthesis projection.
- §3.3 Phase B step 9 extended with `ConstMemoryStatic dedup`
  sub-bullet; §3.3's `SyntheticItemKind` ↔ `ItemKind`
  consistency list extended with `ConstMemoryStatic →
  ItemKind::Static`; new `ConstMemoryStatic Items are
  immutable byte-array statics with Bytes initialiser`
  validation rule (parallel to `VTableInstance` rule).
- `stage-12/expectations.md` "From stage 4" outbound flipped
  to `[CLOSED by D-4-3]`.

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

## Open decisions

- None remaining. Stage 4's spec-side work is complete:
  D-4-1 (const-eval result form), D-4-2 (identity-key byte
  form), D-4-3 (reference-typed handling), D-4-K (render_const
  stub via deferral to stage 14) -- all four stage-1 inbound
  expectations closed.

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

- **D-4-K cluster** (initial review, 2026-05-03): closed at
  R4=APPROVE.
  Trajectory:
  - R1: 4 issues (2 BLOCK / 2 MINOR / 0 NIT). All closed by R2.
    Largest: deferral-target retarget from stage 13 to stage 14
    (Reloc needs render_instance which is stage 14, so
    stage-13 deferral would still leave a transitive-stub gap).
  - R2: 2 issues (1 BLOCK / 1 MINOR / 0 NIT). All closed by R3.
    Missed-files retarget findings (stage-4/expectations.md
    + status.md review-target list).
  - R3: 1 MINOR (§18.2 snapshot-test prose holdover). Closed
    by R4.
  - R4: APPROVE 0.

- **D-4-3 cluster** (initial review, 2026-05-03): closed at
  R4=APPROVE.
  Trajectory:
  - R1: 6 issues (2 BLOCK / 3 MINOR / 1 NIT). All closed by R2.
  - R2: 3 issues (1 BLOCK / 2 MINOR / 0 NIT). All closed by R3.
    Deepest finding: `item.ty` schema mismatch (per-kind
    rich data lives on Instance, not Item).
  - R3: 1 MINOR (cross-ref propagation after status.md
    restructuring). Closed by R4.
  - R4: APPROVE 0.

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

- **None currently active.** The `&str` / `&[T]`
  reference-typed const handling deferred from D-4-1 is closed
  by D-4-3. Non-zero offsets
  into anonymous Memory allocations (`Indirect { alloc_id,
  offset != 0 }`) are bridge-rejected by D-4-3.8; these would
  arise from references into the middle of a larger
  anonymous-memory blob (e.g., `&FOO.field` where `FOO` is
  itself anonymous-memory-backed) and are not load-bearing
  for driver code. No slot is reserved in the dispatch table
  or `InstanceRef` shape; the case is rejected unless a
  future spec revision admits it.

## Next concrete unit of work

Stage 4's **spec-side work is complete**. All four stage-1
inbound expectations are closed (D-4-1, D-4-2, D-4-3, D-4-K).

The remaining stage-4 work is **implementation**: filling
`crates/regseq-model/src/` with the locked design. This is
parallel to stage-1's 7-step shipping pattern and is a
separate workstream from the spec-side decisions tracked in
this `status.md`.
