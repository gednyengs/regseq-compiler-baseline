# Expectations on stage 11 (incoming)

Stage 11 covers MIR bodies and signatures (introduces `LocalId`,
expands `Body` and `FnSig` placeholders).

The convention is described in `../README.md` under "Forward
expectations live in `stage-N/expectations.md`."

## From stage 1

- **Define `LocalId` via `newtype_index!`.** Same situation as
  `BlockId` (stage 10): the body-local convention
  `LocalId::RETURN_PLACE = LocalId::from_u32(0)` requires a
  const-body arm in the macro at
  `crates/regseq-model/src/idx.rs`. If stage 10 lands first, the
  macro arm already exists; otherwise stage 11 lands it. The
  current empty-body arm must continue to work unchanged for
  the existing 9 ID newtypes.

- **Expand `Body` placeholder.** The empty struct at
  `crates/regseq-model/src/placeholders.rs` becomes the full
  MIR body shape: `local_decls: IndexVec<LocalId, LocalDecl>`,
  `basic_blocks: IndexVec<BlockId, BasicBlock>`, etc. Move the
  type out of `placeholders.rs` into its own module (e.g.,
  `body.rs`).

- **Expand `FnSig` placeholder.** The empty struct at
  `placeholders.rs` becomes the full function-signature shape:
  parameter list with types, return type, and `abi: FnAbi`
  (the enum vocabulary `{ Rust, C }` is locked stage-3 D15;
  see `stage-3/spec-snapshot.md` Section 1.9). The `FnAbi` field
  is a **structural field of `FnSig`** -- not a side-channel
  lookup -- per Q2 (eager translation; fully-owned IR;
  `architecture.md`) and the type-identity-preservation
  rationale in stage-3 D15. No `HashMap<FnSigId, FnAbi>`
  side-channel; no post-translation rustc query. Move out of
  `placeholders.rs`.

- **Resolve the `BodyId == ItemId` open question.** Spec's
  "Open items" notes one body per item is the rustc convention;
  whether `BodyId == ItemId` numerically (so the IR has one
  less ID to track) or whether bodies get their own sequential
  IDs is a stage-11 question. Pick one; document the choice in
  `stage-11/spec-snapshot.md`.

- **`_N` sigil rendering.** Add `LocalId`-rendering to
  `pretty.rs` (`_0` for `RETURN_PLACE`, `_1`, `_2`, ...).
  Currently `pretty.rs` has a comment noting this is deferred
  to stage 10/11.

- **Niche optimization for `Option<LocalId>`.** Append the
  4-byte assertions to `crates/regseq-model/src/lib.rs`.

- **Construction invariant 1 extends.** When `Body` and `FnSig`
  carry IDs (e.g., `LocalDecl.ty: TyId`, `FnSig.return_ty: TyId`,
  `FnSig.abi: ...`), stage 11 extends `RegseqModel::validate`'s
  invariant-1 walk to those fields.

## Bridge concerns

- **`tcx.optimized_mir(def_id)` consumption.** Bridge code reads
  rustc's optimized MIR (with the cleanup-pass whitelist applied
  by `regseq-driver/src/callbacks.rs::regseq_mir_passes`) and
  populates `Body`. Stage 11 also needs to consume
  `tcx.fn_sig(def_id)` for signatures, including the `Abi` field.

- **Function-signature instances vs items.** Per the spec, every
  `InstanceKind::Function` has a sig (always present, even for
  foreign declarations). The bridge populates `sigs` with one
  entry per Function instance.

## From stage 3

- [ ] **Implement body-level dyn-Trait dispatch rewriting pass**
  (from stage 3 D5).

  *Stage-3 work already done.* The bridge populates
  `Ty::Dynamic.vtable_ty: TyId` and registers vtable type Items
  (via stage-12's synthesis machinery; see
  `stage-12/expectations.md`). The model-side dedup table
  `RegseqModel.vtables: HashMap<(DefPathHash, ArgsKey),
  ItemId>` is populated as part of bridge work (stage-12 spec;
  uses `ArgsKey` -- the F4-2 deep-canonicalizer output -- not
  raw `GenericArgs`, for cross-build stability).
  See `stage-3/spec-snapshot.md` Section 1.10.

  *Stage-11 deliverable.* A body-level pass that:

  (a) Recognises `Cast::Unsize` rvalues coercing
      `Concrete -> dyn Trait` and triggers stage-12's
      registration of a per-(Trait, ConcreteImpl) vtable
      instance Item (kind=Static, rendered form
      `<vtable for Trait for SelfTy>` per stage-1's reservation
      at `stage-1/spec-snapshot.md:910-927`). The pass calls into
      stage-12's vtable-instance synthesis logic; this stage
      walks the body to find the coercion sites.

  (b) Rewrites `TerminatorKind::Call` whose callee is
      `<dyn Trait as Trait>::method` into a vtable-indirect
      dispatch per the shape stage-9 chose (see
      `stage-9/expectations.md`). The receiver's
      `Ty::Dynamic.vtable_ty` supplies the vtable struct's TyId;
      the method index supplies the field lookup.

  *Cross-stage shape commitments.* Stage 9 owns the post-rewrite
  Call terminator shape -- a public Instance discriminator
  addition (parallel to rustc's Virtual InstanceKind, requiring
  a stage-1 revision since `fn_shim_disambig` is currently
  private per `stage-1/spec-snapshot.md:747-754`) or a widened
  `Call.callee` shape; see `stage-9/expectations.md` for the
  candidate comparison. Stage 12 owns:
  - vtable type Item synthesis (kind=Struct, per (Trait,
    projection_bindings)),
  - vtable instance Item synthesis (kind=Static, per (Trait,
    SelfTy), per stage-1's reservation),
  - vtable shim Function Item synthesis in two families --
    method shims per (Trait, ConcreteImpl, method) and a
    drop-in-place shim per (Trait, ConcreteImpl) -- both for
    receiver erasure (vtable slots are erased-receiver, while
    impl methods and `drop_in_place::<SelfTy>` are
    concrete-receiver),
  - the `RegseqModel.vtables` dedup table.

  *Charon precedent.* `transform_dyn_trait_calls` in Charon's
  `transform/normalize/` rewrites direct
  `<dyn Trait as Trait>::method(self, args)` calls into vtable
  function-pointer calls of the form
  `(move (*@receiver.ptr_metadata).method)(move (@receiver),
  move (@args))` (`stage-3/prior-art/charon.md` F.3). Charon's
  pass also handles vtable instance registration alongside the
  rewriting.

  *Out of stage-3 scope per D5.2.*

  *Cross-refs.*
  - `stage-3/spec-snapshot.md` Section 1.10 (D5 audit shape);
    Section 1.10.2 (D5.2 distribution table).
  - `stage-9/expectations.md` (Call terminator shape decision).
  - `stage-12/expectations.md` (vtable Item synthesis machinery).
  - `stage-3/prior-art/charon.md` F.3.
