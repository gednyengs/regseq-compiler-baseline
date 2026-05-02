# Expectations on stage 9 (incoming)

Stage 9 covers MIR terminators (Call, Return, Goto, Drop, etc.).

The convention is described in `../README.md` under "Forward
expectations live in `stage-N/expectations.md`."

## From stage 1

- **`Drop` lowered to `Call`.** Per the C-semantics methodology
  rule and the spec's "Where InstanceId references appear"
  section, stage 9 lowers `Drop` terminators to regular `Call`
  terminators targeting the drop-glue `Instance` for the type.
  Stage 9 terminators do NOT have a separate `Drop` variant;
  there is no exception path for drops. The bridge synthesizes
  drop-glue `Instance`s as part of the mono-item closure (per
  invariant 9 in the spec).

- **Vtable-indirect dispatch (Call terminator shape).** When MIR
  contains a `dyn Trait` value, dynamic dispatch becomes an
  indirect call through the vtable-loaded function pointer.
  Stage 9 owns the **Call terminator shape** that expresses
  this dispatch (see "From stage 3" below for the two design
  candidates). Stage 12 owns vtable type Item synthesis and
  vtable instance Item synthesis (per
  `stage-12/expectations.md` sections A-D); stage 11 owns the
  body-level rewrite that recognises `Cast::Unsize` coercions
  and rewrites `<dyn Trait as Trait>::method` calls into the
  indirect form. The vtable instance value is a mixed-aggregate
  matching rustc's `tcx.vtable_entries(trait_ref)` sequence
  (drop-glue fn pointer + size + alignment + per-method fn
  pointers + supertrait vtable pointers + `Vacant` slots for
  non-dispatchable methods); both the drop-glue slot and each
  method fn pointer reference vtable shim Function Items
  synthesised at stage 12 (drop-in-place shim per `(Trait,
  ConcreteImpl)`; method shim per `(Trait, ConcreteImpl,
  method)`).

- **Possibly extend `DefPathData` for synthetic vtable scopes.**
  If stage 9 needs a *new* `DefPathData` variant for synthetic
  vtable scopes (rather than overloading an existing variant),
  appending it is a stage-1 revision: append to the `DefPathData`
  enum + the `def_path_data_sort_rank` rank table at the next
  free rank (currently 19). Trigger Codex review of the addition
  per the toolchain-bump methodology event. The
  `anon_namespace_name` table at
  `crates/regseq-model/src/def_path.rs` would also need a new
  arm for the new variant.

- **Closes construction invariant 9.** Once stage 9 introduces
  call-site references to `InstanceId`, validate's invariant 9
  (instance closure-completeness) becomes enforceable
  non-vacuously. Stage 9 walks call sites in `validate` and
  checks each `InstanceId` resolves; positive + negative test in
  `tests/model.rs`.

## Bridge concerns

- **Drop-glue `Instance` interning.** Drop-glue is one of rustc's
  shim variants tracked by `RustcInstanceKindTag::DropGlue`.
  The bridge's instance-interning HashMap (transient) must
  separate drop-glue from `Item`-kind instances of the same
  `DefId` via the `fn_shim_disambig` discriminator (per the F4-2
  RegseqInstanceKey rule).

- **`tcx.vtable_entries(trait_ref)` consumption.** Bridge code
  reads this rustc API to materialize vtable contents.

## From stage 3

- [ ] **Define Call terminator shape supporting vtable-indirect
  dispatch** (from stage 3 D5).

  *Constraint surfaced by stage 3.* `Ty::Dynamic.vtable_ty: TyId`
  links a trait object to its vtable type. After the stage-11
  body-level rewriting pass (see `stage-11/expectations.md`)
  walks a `Call` whose callee is `<dyn Trait as Trait>::method`,
  the rewritten Call must dispatch through a vtable
  function-pointer field (loaded at runtime from the receiver
  fat pointer's `vtable` slot). This is *not* an
  `InstanceId`-pinned dispatch in the sense of
  `stage-1/spec-snapshot.md:873-876`: there is no single `Instance`
  whose `InstanceId` identifies the runtime callee; the callee
  is loaded from a field at call-site time.

  *Two design candidates, both requiring some revision:*

  (a) Public Instance discriminator addition. Rustc's
      `InstanceKind::Virtual` exists as a callable variant
      (rank 5 in `stage-1/spec-snapshot.md:786-805`) but stage 1
      keeps the public Regseq `InstanceKind` to six emission
      variants only (`stage-1/spec-snapshot.md:820-849`), and
      explicitly states that the rustc shim discriminator
      `fn_shim_disambig` is **not** exposed as a public
      Instance field (`stage-1/spec-snapshot.md:747-754`) -- it
      survives only in `symbol_name`, `body`, and the F4-2
      sort key. Therefore making vtable-indirect calls
      observable to stage-9/codegen via the existing public
      InstanceKind requires either (a-i) adding a public
      `Virtual` variant to InstanceKind (a stage-1 revision
      that reopens the closed six-variant set per
      `stage-1/spec-snapshot.md:869-871`), or (a-ii) promoting
      `fn_shim_disambig` to a public Instance field exposing
      the shim discriminator (also a stage-1 revision). Cost:
      stage-1 spec mutability per the spec-mutability rule
      cited at `stage-1/spec-snapshot.md:919-927`.

  (b) Widen `Call.callee` from `InstanceId` to an enum
      `CallCallee::{Direct(InstanceId), Indirect(Operand)}` (or
      similar). The `Indirect(Operand)` carries the loaded
      vtable function pointer. Cost: stage-9-only widening; no
      stage-1 revision needed; consumers (codegen, body
      analyses) must handle two callee kinds.

  *Recommended evaluation criteria.* (i) Cost to stage-1's
  locked InstanceKind closure -- option (a) requires a stage-1
  revision; option (b) does not. (ii) Cost to stage-11's pass
  implementation -- which option is simpler to produce?
  (iii) Cost to codegen -- which option is simpler to consume?
  (iv) Consistency with rustc's design -- rustc treats Virtual
  as a variant of `InstanceKind`, suggesting option (a-i) is
  the rustc-aligned path. (v) Future extensibility -- if other
  rustc shim kinds (FnPtrShim, ReifyShim, etc.) eventually need
  to surface, do they fit option (a)'s discriminator/variant
  pattern or option (b)'s indirect-Operand pattern more
  naturally?

  *Cross-refs.*
  - `stage-3/spec-snapshot.md` Section 1.10 (D5 audit shape);
    Section 1.10.2 (D5.2 distribution table).
  - `stage-3/discussions.md` Section 1 D5 entry.
  - `stage-1/spec-snapshot.md:786-805` (rustc InstanceKind
    disambiguator table including Virtual).
  - `stage-1/spec-snapshot.md:820-848` (current six-variant
    InstanceKind).
  - `stage-1/spec-snapshot.md:873-876` (Call sites carry InstanceId).
  - `stage-3/prior-art/charon.md` F.3 (Charon's
    `transform_dyn_trait_calls` rewrites to
    `(move (*@receiver.ptr_metadata).method)(...)`, an indirect
    call form).
