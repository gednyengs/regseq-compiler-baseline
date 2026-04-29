# Expectations on stage 11 (incoming)

Stage 11 covers MIR bodies and signatures (introduces `LocalId`,
expands `Body` and `FnSig` placeholders).

The convention is described in `../memory.md` under "Forward
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
  parameter list with types, return type, ABI annotation
  (`Abi` enum per the C-semantics methodology rule). Move out
  of `placeholders.rs`.

- **Resolve the `BodyId == ItemId` open question.** Spec's
  "Open items" notes one body per item is the rustc convention;
  whether `BodyId == ItemId` numerically (so the IR has one
  less ID to track) or whether bodies get their own sequential
  IDs is a stage-11 question. Pick one; document the choice in
  `stage-11/vocabulary.md`.

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
