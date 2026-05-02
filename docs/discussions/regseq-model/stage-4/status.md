# Stage 4 Status

Status: **OPEN**. Stage just opened; cluster planning in progress.

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

- [ ] **Pin canonical const-eval result form.** F4-2 instance
  ordering's `ArgsKey` fingerprint reaches const arguments via
  `GenericArg::Const(ConstId)`; two builds must produce identical
  fingerprints for equivalent const-evaluated values.
- [ ] **Cover scalars, byte arrays, aggregate constants
  (struct / array / enum variants), and function pointers as
  constants.**
- [ ] **Fill `pretty.rs::render_const` stub** at
  `crates/regseq-model/src/pretty.rs` (currently panics with
  `unimplemented!("stage 4 fills render_const")`). May defer
  renderer style if the renderer's input depends on content not
  yet pinned (mirroring D14's deferral).
- [ ] **Preserve `Option<ConstId>` 4-byte niche optimization**
  (asserted in `lib.rs`).

## Open decisions (provisional cluster)

The decision space stage 4 covers (D-4-K tags allocated as
decisions are framed):

- **`Const` enum variant set.** The closed list of value
  flavours. From spec.md §9.7 the surface stage 4 must cover:
  scalar values with type tag, ZST values, byte arrays / raw
  initialised bytes, aggregate field sequences (preserving
  order), enum discriminant + payload constants, references to
  `InstanceId` (function/static/vtable pointer relocations),
  typed null pointers, padding/uninitialised bytes.

- **`ConstIdentityKey` schema.** The canonical key shape per
  variant. Per-variant decisions may sub-cluster (`D-4-K.A`
  scalars, `D-4-K.B` aggregates, `D-4-K.C` relocations, etc.).

- **Two-phase validation discipline.** The split between type-
  level consts (validated before `TyInterner`) and value-level
  consts (validated after `TyInterner` + `instances`). One
  `ConstInterner` table with a two-pass discipline, or two
  distinct interner tables (`TypeConstInterner` +
  `ConstInterner`)? Spec deferral is the validation phase split,
  not the interner shape -- stage 4 picks the shape.

- **Cross-build relocation encoding.** The canonical form
  `(regseq_instance_kind_rank, item_def_path_hash,
  canonical_generic_args, fn_shim_disambig_or_None)` is named in
  spec.md §9.7; stage 4 pins the precise field-by-field encoding
  (byte order, hash algorithm choice, sub-canonicaliser for
  generic args).

- **Const-eval result form.** The exact bytes/struct shape
  carried per scalar/aggregate variant -- how rustc's const-eval
  output is unpacked into `Const`.

- **Renderer style for `render_const`.** The text shape
  `pretty.rs::render_const` produces. May defer to a later stage
  if dependencies on stage-9/11/12 content emerge -- decide
  deliberately.

**Cluster split: TBD.** The decisions above are tightly coupled
(variant set <-> identity key <-> encoding), so solo or two-
session is more likely than three. Final split decided when the
first decision is framed; if parallel, allocate D-4-K tags up
front to avoid namespace races (per `../README.md` §5.3).

## Locked decisions

(none yet)

## Codex review cadence (status)

(no rounds run yet)

Prompt template: see `memory.md` Section 4.

## Stage-4-specific deferred items

(none yet)

## Next concrete unit of work

1. Read top-level `../spec.md` §9.7 (ConstInterner), §3.3
   (validation order phases), §13.2 (vtable family --
   downstream consumer of value-level consts with relocations),
   and §15.1 (rejections, including the stage-4-pinned-shape row
   for "Const value referencing the type").
2. Frame `D-4-1`. Natural starting point is the `Const` enum
   variant set (it gates everything else: identity key shape
   depends on the variants; validation phase split partitions
   the variants; renderer maps over them). Lay out options +
   recommendation + sub-questions in conversation per Phase A.
3. Decide cluster split (solo or parallel) once the first
   decision frame surfaces enough of the dependency graph.
4. Run a fresh stage-4 prior-art pass (per `../README.md` §2
   methodology rule -- do not reuse stage-0 summaries) into:
   `rustc_middle::ty::Const` / `ConstKind`, miri allocation +
   relocation model, charon const handling, kani const
   resolution, MLIR `arith` constants, LLVM constant expressions.
   Findings land in `prior-art/<project>.md`.
