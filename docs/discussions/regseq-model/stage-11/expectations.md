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

## From stage 5

- [ ] **Implement Place validation walk** (per D-5-2).

  Stage 5 owns Place's validation contract -- five rule
  categories pinned in `stage-5/spec-snapshot.md` §19.7:

  1. **Type-coherence walk.** Each `PlaceElem` step narrows
     the type per the per-variant rules in §19.4 (predecessor
     type requirement; successor type derivation; active
     variant interaction with `Field` after `Downcast`).
  2. **Head-local existence.** The head `LocalId` resolves
     to a valid entry in the enclosing body's `local_decls`
     table.
  3. **`FieldIdx` range.** For each `Field(idx, _)`, `idx`
     is in range for the predecessor type's field list,
     dispatched by predecessor (per the §19.1 doc and §11
     layout vocabulary): `LayoutKind::Struct.fields.len()`
     for `Ty::Struct{..}`; for `Ty::Closure(item, args)`,
     look up the parallel `Ty::Struct(item, args, _)` via
     `find_nominal(NominalKind::Struct, item, args)` and use
     that struct's `LayoutKind::Struct.fields.len()`
     (closure-as-struct identity-vs-layout split per stage-3
     D6.3); `LayoutKind::Union.fields.len()` for
     `Ty::Union{..}`; the tuple's `fields.len()` for
     `Ty::Tuple{..}`; the active variant's
     `VariantLayout.fields.len()` for `Ty::Enum{..}` after
     `Downcast`.
  4. **`VariantIdx` range.** For each `Downcast(idx)`, let
     `shape` be the `EnumShape` carried in the predecessor
     `Ty::Enum{..}`'s `layout.kind` (per §11.1, the layout's
     inner `LayoutKind::Enum.shape`). Then `idx` is in
     `0..variant_count(shape)`, where `variant_count`
     dispatches on `EnumShape` (§11.1): `Empty -> 0`,
     `Single { .. } -> 1`,
     `Multiple { variants, .. } -> variants.len()`.
  5. **Structural invariants.** I-Deref-first and
     I-Downcast-followed-by-Field (§19.3): `Deref` appears
     only at projection[0]; `Downcast(_)` is non-last and
     immediately followed by `Field(_, _)`.

  Walk implementation defers to stage 11 because Places live
  inside body content (operand / rvalue / statement /
  terminator payloads). Stage-1's bodies are empty; the walk
  is non-vacuous only once stage 11 materialises `Body` /
  `LocalDecls` / basic-blocks.

  *Stage-11 deliverable.* A §3.3 Phase B body-validation step
  that walks all Places appearing in body content and applies
  the five rules in a single traversal. The rule statements
  themselves do not need re-derivation -- they are pinned in
  §19.7 and are stage-5-owned. The walk is stage-11-owned
  (mechanical traversal + diagnostic shape).

  The deferral mirrors D-4-K's pattern (renderer rules at the
  spec-owning stage; implementation at the dependency-ready
  stage). `stage-5/spec-snapshot.md` §3.3 Phase B step 8
  carries a forward-declaration NOTE pointing to this
  expectation.

  *Cross-refs.*
  - `stage-5/spec-snapshot.md` §19.7 (validation rule
    statements), §19.4 (type-narrowing rules), §3.3 Phase B
    step 8 NOTE.
  - `stage-5/discussions.md` D-5-2 (audit-shape entry for
    the deferral decision); D-5-1.5 (I-Deref-first audit);
    D-5-1.6 (I-Downcast-followed-by-Field audit).

- [ ] **Fill `pretty.rs::render_place(&Place, &Body)` stub**
  (per D-5-K).

  Stage 5 deferred this renderer to stage 11 (originally
  proposed for stage 12, retargeted on D-5-2 + D-5-K
  combined Codex review R1 BLOCK finding `d5k-name-source-
  and-target` after evidence cross-check showed `Item`
  carries no field/variant name tables; names live in §11
  layout schema's `FieldLayout.name` / `VariantLayout.name`,
  reachable at render time via the head local's
  `LocalDecl.ty`).

  Per-variant dependency analysis from
  `stage-5/discussions.md` D-5-K:

  | Variant | Rendering input | Stage |
  | --- | --- | --- |
  | `Deref` | self-contained glyph (`*`) | -- |
  | `Field(FieldIdx, _)` | predecessor type's layout (stage 3, locked) -> `LayoutKind::Struct.fields[idx].name` (or `Closure`-via-parallel-Struct, `Union.fields[idx].name`, tuple `fields[idx].name`, or active variant's `VariantLayout.fields[idx].name` after `Downcast`); reachable at render time via head local's `LocalDecl.ty` | 11 (Body materialisation) |
  | `Index(LocalId)` | LocalId rendering (`_N`) | 11 |
  | `ConstantIndex { ... }` | self-contained numeric formatting | -- |
  | `Subslice { ... }` | self-contained numeric formatting | -- |
  | `Downcast(VariantIdx)` | enum's `LayoutKind::Enum.shape` -> `EnumShape::Multiple.variants[idx].name` or `Single.variant.name`; reachable at render time via head local's `LocalDecl.ty` | 11 (same as Field) |

  Plus head `LocalId` rendering: stage 11.

  The `Field`'s inline `TyId` slot is consumed by validation
  / codegen, NOT by rendering -- so `render_ty` (stage 13)
  is not a `render_place` dependency. `Item` (stage 12) is
  not a dependency either: it carries `def_path` / `kind` /
  `span` / flags / `opaque_handle_eligible`, not
  field/variant name tables. Stage 11 is the latest of the
  actual dependencies (Body materialisation + LocalId `_N`
  rendering); landing `render_place` there ensures all
  transitive dependencies are ready for a complete
  implementation.

  Stub message at `crates/regseq-model/src/pretty.rs`:
  `unimplemented!("stage 11 fills render_place (deferred per D-5-K)")`.

  `stage-5/spec-snapshot.md` §18.2 per-method ownership
  table lists `render_place(&Place, &Body)` -- stage 11.

  Audit shape: `stage-5/discussions.md` D-5-K parent. The
  retarget rationale (originally stage 12; corrected to
  stage 11) is recorded under the *Why not Option C* entry
  there.

## From stage 6

- [ ] **Fill `pretty.rs::render_operand(&Operand, &Body)`
  stub** (per D-6-K).

  Stage 6 deferred this renderer to stage 11, alongside
  D-5-K's `render_place`. Per-variant dependency analysis
  from `stage-6/discussions.md` D-6-K:

  | Variant         | Rendering input                                              | Stage |
  | --------------- | ------------------------------------------------------------ | ----- |
  | `Copy(p)`       | `render_place(p, body)` + `copy ` keyword prefix              | 11    |
  | `Move(p)`       | `render_place(p, body)` + `move ` keyword prefix              | 11    |
  | `Constant(id)`  | `render_const(id)`                                            | 14    |

  The `Copy` / `Move` arms depend on `render_place` (stage
  11), which itself depends on `Body` materialisation +
  `LocalId` `_N` rendering. The `Constant` arm depends on
  `render_const` (stage 14, per D-4-K). The renderer lands
  at stage 11 -- the latest of the *first batch* of
  dependencies -- with the `Constant` arm late-binding to
  its stage-14 dependency. Co-locating with `render_place`
  keeps the rendering surface coherent and unblocks
  body-content snapshot tests when bodies materialise.

  Stub message at `crates/regseq-model/src/pretty.rs`:
  `unimplemented!("stage 11 fills render_operand (deferred per D-6-K)")`.

  `stage-6/spec-snapshot.md` §18.2 per-method ownership
  table lists `render_operand(&Operand, &Body)` -- stage 11.

  Audit shape: `stage-6/discussions.md` D-6-K. Spec content:
  `stage-6/spec-snapshot.md` §20.6.

  *Cross-refs.*
  - `stage-6/spec-snapshot.md` §20 (Operand vocabulary),
    §20.6 (rendering deferral), §18.2 (per-method ownership
    table updated with `render_operand` row).
  - `stage-6/discussions.md` D-6-K parent (deferral
    rationale and per-variant dependency audit).
  - D-5-K precedent in `stage-5/discussions.md` (the same
    "land at latest first-batch dependency" pattern).

## From stage 7

- [ ] **Fill `pretty.rs::render_rvalue(&Rvalue, &Body)`
  stub** (per D-7-K).

  Stage 7 deferred this renderer to stage 11, alongside
  D-5-K's `render_place` and D-6-K's `render_operand`.
  Per-variant dependency analysis from
  `stage-7/discussions.md` D-7-K:

  | Variant | Rendering input | Stage |
  | --- | --- | --- |
  | `Use(op)` | `render_operand(op, body)` | 11 |
  | `Repeat(op, n)` | `render_operand(op, body)` + numeric `n` (self-contained) | 11 |
  | `Ref(prov, p)` | `&` / `&mut` keyword (per `Provenance` discriminator) + `render_place(p, body)` | 11 |
  | `RawPtr(mutbl, p)` | `&raw const` / `&raw mut` keyword (per `Mutability` discriminator) + `render_place(p, body)` | 11 |
  | `Cast(ck, op, ty)` | `render_operand(op, body)` + CastKind name + `render_ty(ty)` | 11 (operand), 13 (ty late-bind) |
  | `BinaryOp(op, l, r)` | BinOp name + `render_operand(l, body)` + `render_operand(r, body)` | 11 |
  | `UnaryOp(op, o)` | UnOp name + `render_operand(o, body)` | 11 |
  | `Discriminant(p)` | `discriminant(` + `render_place(p, body)` + `)` | 11 |
  | `Aggregate(kind, ops)` | AggregateKind dispatch + per-element `render_operand` + per-variant field/variant names resolved directly from `kind`'s `TyId` via the §12 type interner | 11 |

  **Aggregate field/variant name resolution is direct
  from `AggregateKind`'s per-variant `TyId`** (no
  `LocalDecl.ty` lookup needed -- every `AggregateKind`
  variant in the locked 7-variant set carries the
  destination TyId in its payload). Specifically:
  `Struct(TyId)` resolves via `Ty::Struct { layout }` ->
  `LayoutKind::Struct.fields`; `Enum(TyId, VariantIdx)`
  via `Ty::Enum { layout }` -> `LayoutKind::Enum.shape` ->
  active variant's `VariantLayout.fields`; `Union(TyId,
  FieldIdx)` via `Ty::Union { layout }` ->
  `LayoutKind::Union.fields`; `Closure(TyId)` via the
  parallel `Ty::Struct(captures-struct)` reached by
  `find_nominal(NominalKind::Struct, item, args)` per
  §13.1, then its `LayoutKind::Struct.fields`;
  `Array(TyId)` and `Tuple(TyId)` via `Ty::Array(elem, n)`
  and `Ty::Tuple { fields, layout }` respectively;
  `RawPtr(TyId, Mutability)` is positional
  `(data_ptr, metadata)` with no field names.

  **What forces deferral to stage 11 is the operand- and
  place-rendering subtree**, not aggregate name
  resolution. Nearly every `Rvalue` variant embeds an
  `Operand` or a `Place`; rendering those requires the
  per-local `LocalDecl` context owned by the body-level
  data structures landing at stage 11. The Cast arm late-
  binds to stage-13 `render_ty` for full destination-type
  rendering and becomes fully functional once stage 13
  lands; tests for the Cast arm gate on stage 13.

  Stub message at `crates/regseq-model/src/pretty.rs`:
  `unimplemented!("stage 11 fills render_rvalue")`.

  Top-level `spec.md` §18.2 per-method ownership table
  lists `render_rvalue(&Rvalue, &Body)` -- stage 11.

  *Cross-refs.*
  - Top-level `spec.md` §21 (Rvalue vocabulary), §21.8
    (rendering deferral note), §18.2 (per-method
    ownership table -- updated with `render_rvalue` row).
  - `discussions.md` `## Stage 7` D-7-K (deferral
    rationale and per-variant dependency audit).
  - D-5-K precedent in `discussions.md` `## Stage 5` and
    D-6-K precedent in `discussions.md` `## Stage 6` (the
    same "land at latest first-batch dependency" pattern).


---

## From stage 8

**Filed 2026-05-05 at stage-8 merger contract close.**
Three-item bundle from D-8-K + D-8-3 + D-8-4 Part C.

### `render_statement` deferral

Per D-8-K, stage-8 defers `pretty.rs::render_statement`
to stage 11 alongside `render_place` (D-5-K),
`render_operand` (D-6-K), `render_rvalue` (D-7-K).

Per-variant dependency analysis from §22.8 of top-level
`spec.md` plus §18.2 per-method ownership row:

- `Assign(Place, Rvalue)` needs `render_place` +
  `render_rvalue` (transitively `render_operand`); the
  `render_rvalue::Cast` arm late-binds to `render_ty` at
  stage 13.
- `SetDiscriminant(Place, VariantIdx)` needs
  `render_place` + variant-name lookup direct from
  `place_ty(p)`'s `LayoutKind::Enum.shape` per the
  §11.1 cases (`EnumShape::Single { variant, .. } ->
  variant.name`; `EnumShape::Multiple { variants, .. } ->
  variants[vidx].name`). No additional late-binding to
  later stages -- the §10 / §11 vocabulary needed is
  already pinned at construction time.
- `Intrinsic(NonDivergingIntrinsic)` needs
  `render_operand` for the embedded operands in both
  arms (`Assume(op)` and `CopyNonOverlapping{src, dst,
  count}`); the `render_operand::Constant` arm late-
  binds to `render_const` at stage 14.

Stage 11 owns `render_statement` along with the rest
of the body-rendering subtree; stage-13 / stage-14
late-bindings inherited via the `render_rvalue` /
`render_operand` per-variant arms (no new
late-bindings introduced by `render_statement` itself).

Stub message at `crates/regseq-model/src/pretty.rs`:
`unimplemented!("stage 11 fills render_statement")`.

Top-level `spec.md` §18.2 per-method ownership table
lists `render_statement(&Statement, &Body) -- stage 11`.

*Cross-refs.*
- Top-level `spec.md` §22 (Statement vocabulary), §22.8
  (rendering deferral note), §18.2 (per-method
  ownership table -- updated with `render_statement` row).
- `discussions.md` `## Stage 8` D-8-K (deferral
  rationale and per-variant dependency audit).
- D-5-K, D-6-K, D-7-K precedent in `discussions.md`
  `## Stage 5` / `## Stage 6` / `## Stage 7` (the same
  "land at latest first-batch dependency" pattern).

### Statement walk implementation

Per D-8-4 Part C, stage 8 specifies the rules of the
12-row rejection table (§22.5.1) and the 8 positive
invariants (§22.5.2 -- B-0a Place inherited from §19.7,
B-0b Rvalue inherited from §21.5, B-0c Operand
inherited from §20.5, B-1 through B-5 stage-8-
introduced) but defers the body-level visitor walk
that applies them to stage 11. Compositional with the
§19.7 Place walk and §21.5.3 Rvalue walk inheritance:
a stage-11 visitor that walks `BasicBlock.statements[i]`
visits Place / Rvalue / Operand / Statement invariants
in a single traversal pass.

The stage-11 visitor signature is something like
`fn validate_statement(&self, s: &Statement, body: &Body)
-> Result<(), DiagnosticList>` -- reading the per-
statement diagnostic location from `s.span` and the
per-local type information from `body.local_decls`.

Same split as §19.7's Place validation, §20.5's
operand-site invariants, and §21.5.3's rvalue-site
invariants -- all of which inherit the walk via §3.3
Phase B body validation.

*Cross-refs.*
- Top-level `spec.md` §22.5 (full invariants registry),
  §22.5.3 (walk-implementation deferral note).
- `discussions.md` `## Stage 8` D-8-4 (5-part registry
  audit + walk-impl phase placement rationale).
- D-5-2 walk-impl deferral and D-7-3 walk-impl deferral
  in `discussions.md` `## Stage 5` and `## Stage 7`
  (the inheritance pattern stage 11 will compose).

### Named-local preservation

Per D-8-3, stage-8's Statement wrapper
`Statement { kind: StatementKind, span: Span }` drops
the per-statement `StmtDebugInfos` (rustc's DWARF
instrumentation hints; no C-shape meaning per stage-1
debuginfo rejection). However, the body-level
`var_debug_info: Vec<VarDebugInfo>` mapping
(`rustc_middle/src/mir/mod.rs:264, 1235-1257`)
carries user-source variable names ("local `_3`
corresponds to user variable `temp`") and is a
distinct semantic concern from `StmtDebugInfos`.

**Stage 11's `Body` / `LocalDecls` design must
preserve named-local mapping** for:
1. **Pretty-printer readability per Q5.** Body-content
   snapshot tests displayed with named locals are
   dramatically more readable than `_3` / `_4` / `_5`
   placeholders.
2. **Generated-C readability.** Reviewers reading
   stage-14 codegen output benefit from
   `temp = ...;` over `_3 = ...;`. Named locals
   carry through stage-14 identifier emission.
3. **Downstream debugger / linter usability.**
   Named locals make stack traces and watch-variable
   displays in clang/gcc-emitted debug info match
   the original Rust source.

**Recommended provision:**
`LocalDecl.debug_name: Option<StringId>` populated
from rustc's `var_debug_info` at stage-11 body
construction time. The `Option` accommodates locals
that have no source-level name (compiler-synthesised
temporaries from `RemoveUnneededDrops` and similar
required passes); the `StringId` indirection routes
through stage-1's `StringInterner`.

Stage 11's `render_local` / `render_place` and the
C-backend identifier emission policy then dispatch
on `LocalDecl.debug_name`: if `Some(s)`, render the
interned name; if `None`, fall back to `_<N>` where
`N` is the local index.

Note distinct from stage-1's debuginfo rejection,
which covered DWARF-emission instrumentation
(`StmtDebugInfos`) but did NOT preempt body-level
name-mapping.

*Cross-refs.*
- Top-level `spec.md` §22.1.1 (`Statement` wrapper
  struct + the named-local note distinguishing
  body-level `var_debug_info` from per-statement
  `StmtDebugInfos`).
- `discussions.md` `## Stage 8` D-8-3 (wrapper struct
  audit + named-local-preservation outbound rationale).
- Stage-1 spec.md §5.4 / debuginfo-rejection text
  (the DWARF-emission rejection that does not extend
  to name-mapping).

---

## From stage 9

**Filed 2026-05-05 at stage-9 Phase A close.**
Five-item bundle from D-9-K + D-9-10 Part C + D-9-3 (b)
+ D-9-2 + D-9-10 B-7. (Filed at Phase A close so
expectations land before the merger contract; stage-9's
own status.md tracks Phase B drafting / Codex review /
merger as separate tasks.)

### `render_terminator` deferral

Per D-9-K, stage 9 defers `pretty.rs::render_terminator`
to stage 11 alongside `render_place` (D-5-K),
`render_operand` (D-6-K), `render_rvalue` (D-7-K), and
`render_statement` (D-8-K). Stage 11 owns all five
`render_*` methods together.

**Four public renderers to implement** (per stage-9
spec-snapshot §23 deferral note + §18.2 per-method
ownership table update). A fifth dispatch helper
`render_terminator_kind` is an internal implementation
detail, not in the public §18.2 ownership table.

- `render_terminator(&Terminator, &Body)` -- public
  top-level entry point. Internally dispatches per-
  variant on `TerminatorKind`.
- `render_call_callee(&CallCallee, &Body)` -- public.
  Two-arm dispatch on `Direct(InstanceId)` /
  `Indirect(Operand)` per D-9-3 (b).
- `render_switch_targets(&SwitchTargets, &Body)` --
  public. Explicit-split storage shape (`cases` +
  `otherwise`) per D-9-6.
- `render_assert_kind(&AssertKind, &Body)` -- public.
  8-variant subset per D-9-5.

**Per-variant dependency analysis from D-9-K spec
content:**

| Variant | Rendering input | Stage |
| --- | --- | --- |
| `Goto(BlockId)` | block-label rendering convention | 11 (Body owns BlockId space) |
| `SwitchInt { discr, targets }` | `render_operand(discr, body)` + `render_switch_targets(targets, body)` | 11 |
| `Return` | self-contained keyword | -- |
| `Unreachable` | self-contained keyword | -- |
| `Call { callee, args, destination, target }` | `render_call_callee(callee, body)` + per-arg `render_operand` + `render_place(destination, body)` + optional block-label for target | 11 + 12 (CallCallee::Direct: see below) |
| `Assert { cond, expected, msg, target }` | `render_operand(cond, body)` + bool literal + `render_assert_kind(msg, body)` + block-label | 11 |

**`CallCallee::Direct(InstanceId)` rendering**
late-binds to **stage 12**: the InstanceId resolves
to an `Instance`, whose `Item.def_path` is rendered
via `render_def_path(ItemId)` per stage-1's helper
(stub at `crates/regseq-model/src/pretty.rs`; stage-12
fills the body per
`stage-12/expectations.md` "From stage 1" --
`Item.def_path.rendered` is the cached symbol-name
string). Stage 11's `render_call_callee` for the
`Direct` arm calls `render_def_path` and the result is
the C function symbol name.

**`CallCallee::Indirect(Operand)`** delegates entirely
to `render_operand` -- no stage-12 dependency.

**`render_assert_kind` per-variant dispatch:**
- `BoundsCheck { len, index }` -> `"index out of bounds: the len is " + render_operand(len) + " but the index is " + render_operand(index)` (or similar; mirror rustc's diagnostic phrasing).
- `Overflow(op, l, r)` -> "attempt to {op} with overflow" + per-operand rendering.
- `OverflowNeg(o)` -> "attempt to negate with overflow" + render_operand(o).
- `DivisionByZero(o)` / `RemainderByZero(o)` -> "attempt to divide / remainder by zero" + render_operand(o).
- `MisalignedPointerDereference { required, found }` -> "misaligned pointer dereference: address must be a multiple of {required} but is {found}" + per-operand rendering.
- `NullPointerDereference` -> "null pointer dereference occurred" (no operands).
- `InvalidEnumConstruction(o)` -> "trying to construct an enum from an invalid value " + render_operand(o).

Phrasing closely mirrors rustc's `AssertMessage::description()` at
`rustc_middle/src/mir/syntax.rs:1066-1078`-adjacent strings; stage 11
picks whether to verbatim-mirror or use a Regseq-flavored phrasing.

Stub messages at `crates/regseq-model/src/pretty.rs`
(four public stubs; `render_terminator_kind` is an
internal helper without a separate stub):
```
unimplemented!("stage 11 fills render_terminator (deferred per D-9-K)")
unimplemented!("stage 11 fills render_call_callee (deferred per D-9-K)")
unimplemented!("stage 11 fills render_switch_targets (deferred per D-9-K)")
unimplemented!("stage 11 fills render_assert_kind (deferred per D-9-K)")
```

Top-level `spec.md` §18.2 per-method ownership table
will list (post-merger; 4 rows -- one per public
renderer):
- `render_terminator(&Terminator, &Body)` -- stage 11.
- `render_call_callee(&CallCallee, &Body)` -- stage 11.
- `render_switch_targets(&SwitchTargets, &Body)` -- stage 11.
- `render_assert_kind(&AssertKind, &Body)` -- stage 11.

*Cross-refs.*
- Stage-9 `spec-snapshot.md` §23 (Terminator
  vocabulary) and the stage-9 §18.2 per-method
  ownership row update (post-merger lands in top-level
  `spec.md` §18.2).
- `stage-9/discussions.md` D-9-K (deferral rationale
  and per-variant dependency audit).
- D-5-K, D-6-K, D-7-K, D-8-K precedent in
  `discussions.md` `## Stage 5` / `## Stage 6` /
  `## Stage 7` / `## Stage 8` (the "land at latest
  first-batch dependency" pattern; stage 9 closes the
  cascade).

### Terminator walk implementation

Per D-9-10 Part C, stage 9 specifies the rules of the
9-row rejection table (D-9-9; §23.5 in stage-9
spec-snapshot) and the 9 positive invariants
(§23.6 -- B-0a Place inherited from §19.7, B-0c
Operand inherited from §20.5, B-1 through B-7
stage-9-introduced) but defers the body-level visitor
walk that applies them to stage 11. Compositional with
the §19.7 Place walk, §21.5.3 Rvalue walk, §20.5
Operand walk, and §22.5 Statement walk inheritance: a
stage-11 visitor that walks `BasicBlock.terminator`
visits Place / Operand / Statement / Terminator
invariants in a single traversal pass.

The stage-11 visitor signature is something like
`fn validate_terminator(&self, t: &Terminator, body: &Body)
-> Result<(), DiagnosticList>` -- reading the
per-terminator diagnostic location from `t.span` and
the per-local type information from `body.local_decls`.

**Stage-9-introduced invariants the walk implements:**
- B-1 SwitchInt.discr type ∈ {Bool, Char, Int, Uint}.
- B-2 Assert.cond type = Ty::Bool.
- B-3 Call destination type = callee return type.
- B-4 Call args length = callee arity.
- B-5 Call callee callability (Direct: InstanceId
  resolves to a callable Instance; Indirect: Operand
  type ∈ {Ty::FnPtr}).
- B-6 Per-arg type match.
- B-7 Closes construction invariant 9 (every Direct
  InstanceId resolves in `model.instances`).

Same split as §19.7's Place validation, §20.5's
operand-site invariants, §21.5.3's rvalue-site
invariants, and §22.5's statement-site invariants --
all of which inherit the walk via §3.3 Phase B body
validation.

Visitor-trait extensions: stage 11 adds Terminator,
CallCallee, SwitchTargets, AssertKind to the existing
visitor trait surface (covering the per-Terminator,
per-CallCallee, per-SwitchTargets, per-AssertKind walk
hooks). Same pattern as stages 5-8 visitor extensions.

*Cross-refs.*
- Stage-9 `spec-snapshot.md` §23.5 (rejection
  registry) + §23.6 (positive invariants) +
  §23.7 (walk-implementation deferral note);
  post-merger lands in top-level `spec.md` §23.
- `stage-9/discussions.md` D-9-10 (5-part registry
  audit + walk-impl phase placement rationale).
- D-5-2, D-7-3, D-8-4 walk-impl deferrals in
  `discussions.md` `## Stage 5` / `## Stage 7` /
  `## Stage 8` (the inheritance pattern stage 11 will
  compose).

### Body-level vtable-indirect rewrite pass

Per D-9-3 (b), stage 9 locks the post-rewrite Call
terminator shape: `Call.callee: CallCallee::Indirect(
Operand)` carrying the loaded vtable function pointer.
The body-level pass that produces this shape from
rustc's `<dyn Trait as Trait>::method` Calls is owned
by stage 11.

This expectation **closes the stage-3 D5 inbound**
filed at lines 73-141 above (which left the Call
terminator shape candidate-comparison open pending
stage-9 resolution). Now that D-9-3 (b) is locked, the
post-rewrite Call shape is **`Indirect(Operand)`** --
not the InstanceKind::Virtual extension that
candidate (a) would have introduced. The "From stage
3" expectation continues to apply for the rewrite
pass implementation; this section adds the stage-9-
locked shape commitment.

**Stage-11 deliverable** (extending the stage-3 entry
above):

The pass walks every `BasicBlock.terminator` in every
body. For each `TerminatorKind::Call` whose callee
resolves to a Regseq Instance with `kind =
InstanceKind::Function { flavor:
FunctionInstanceFlavor::Virtual, .. }` (per top-level
`spec.md` §9.4 -- the canonical post-stage-1 InstanceKind
shape with the `flavor` field):

1. **Extract the receiver and recover virtual_idx.**
   The receiver is `args[0]`, whose type is
   `Ty::Ref(dyn_ty, _)` or `Ty::Ptr(dyn_ty, _)` where
   `dyn_ty: Ty::Dynamic { auto_traits, vtable_ty }`
   (the bare `Ty::Dynamic` may only appear as the
   pointee of `Ty::Ref` / `Ty::Ptr` per the spec's
   model invariant 12). For dyn-Trait drops (per
   §23.4 step 1), `args[0]` is the synthetic
   `&raw mut place` operand of type `Ty::Ptr(dyn_ty,
   Mutability::Mut)`. The receiver is a fat pointer
   (data pointer + vtable pointer).

   The vtable index `idx` is read from the resolved
   Regseq Instance's `virtual_idx` field (the
   `Function { ..., virtual_idx: Some(idx) }` field
   per stage-9/spec-snapshot.md §9.4 (which post-merger lands in top-level spec.md §9.4)). For method virtual calls,
   `idx` is the method's slot in
   `tcx.vtable_entries(trait_ref)`; for dyn-drops,
   `idx = 0` (the
   `COMMON_VTABLE_ENTRIES_DROPINPLACE` convention per
   `rustc_middle/src/ty/vtable.rs:45-49`).

2. **Materialize the receiver into a fresh local
   `_t_recv` and synthesize the vtable-pointer
   projection chain + data-pointer extraction.**
   Stage-9 `Operand` is `Copy(Place) | Move(Place) |
   Constant(ConstId)` per §20.1; the Call's `args[0]`
   may be any of those forms. The rewrite first
   normalizes args[0] to a fresh receiver local.

   - **Step 2a: receiver materialization.** Match
     `args[0]` and produce `_t_recv: LocalId` with the
     same type as args[0]:
     - If `args[0] = Operand::Copy(p)` or
       `Operand::Move(p)`: insert
       `_t_recv = Rvalue::Use(args[0].clone())` -- a
       trivial Use rvalue copies/moves the existing
       Place into the fresh local. (For Move, this
       moves args[0]'s place into _t_recv; subsequent
       uses of _t_recv consume the moved value.)
     - If `args[0] = Operand::Constant(_)`: this case
       does not arise for receivers of `&dyn Trait` /
       `*mut dyn Trait` types in well-formed input
       (constants of dyn-Trait pointer type would
       require a vtable Static, which the bridge
       resolves to a non-Constant operand form per
       stage-12 vtable instance synthesis). If
       encountered, materialize via
       `_t_recv = Rvalue::Use(Operand::Constant(c))`.

     After this step, `_t_recv` is a LocalId of type
     `Ty::Ref(dyn_ty, _)` or `Ty::Ptr(dyn_ty, _)`
     (matching args[0]'s type).

   Insert the following Statements at the end of the
   current basic block (after step 2a, before the
   Call terminator):

   - `_t_meta = Rvalue::UnaryOp(UnOp::PtrMetadata, Operand::Copy(Place::from(_t_recv)))`
     -- the metadata field of the fat pointer (the
     vtable pointer, as `*const VtableType` where
     `VtableType` is the synthesised vtable type Item
     pinned by stage-12 §A; specifically derived from
     `dyn_ty.vtable_ty`).
   - `_t_fn = Rvalue::Use(Operand::Copy(Place { local:
     _t_meta, projection: [Deref, Field(virtual_idx,
     fn_ptr_ty)] }))` -- the function-pointer-typed
     field at vtable slot `virtual_idx`. The
     predecessor type at the `Field` step is
     `Ty::Struct(VtableType, ...)`, which is
     admissible per §19.4 PlaceElem typing.

   - **Source normalization for reference receivers.**
     `CastKind::PtrToPtr` requires raw-pointer source
     and raw-pointer destination per stage-7 §21
     (mirroring rustc's
     `rustc_mir_transform/src/validate.rs:1382-1392`
     PtrToPtr typing rule). For receivers with type
     `Ty::Ref(dyn_ty, _)` (method-virtual calls), the
     rewrite first synthesizes a raw-pointer
     intermediate via
     `_t_raw = Rvalue::RawPtr(<mut>, Place { local:
     _t_recv, projection: [Deref] })` -- this `&raw
     const *_t_recv` (or `&raw mut *_t_recv` for
     `&mut dyn Trait` receivers) form is a valid
     Rvalue::RawPtr per stage-7 §21.2 with a Place
     whose post-Deref projected type is `dyn_ty`
     (admissible per §19.4 PlaceElem Deref-on-`Ty::Ref`
     typing rule). The result `_t_raw` has type
     `Ty::Ptr(dyn_ty, Mutability::Not)` (or
     `Mutability::Mut` for `&mut dyn Trait`
     receivers). For receivers already with type
     `Ty::Ptr(dyn_ty, _)` (dyn-Trait drops per §23.4
     where _t_recv was materialized from the bridge-
     synthesized `&raw mut place`), this normalization
     step is skipped; use `_t_recv` directly via
     `Operand::Copy(Place::from(_t_recv))` as the
     PtrToPtr source.

   - `_t_data = Rvalue::Cast(CastKind::PtrToPtr,
     <_t_raw_or_recv_operand>, Ty::Ptr(unit_ty, mut))`
     -- the data pointer extracted from the fat raw
     pointer via a `PtrToPtr` cast that drops the
     metadata. The source operand is
     `Operand::Move(Place::from(_t_raw))` for the
     reference-receiver case (after normalization) or
     `Operand::Copy(Place::from(_t_recv))` for the
     raw-pointer-receiver case (no normalization
     needed). The destination type is `Ty::Ptr(Ty::Tuple([]),
     Mutability::Not)` for `&dyn Trait` receivers and
     `Ty::Ptr(Ty::Tuple([]), Mutability::Mut)` for
     `&mut dyn Trait` / `*mut dyn Trait` (drop)
     receivers. The `PtrToPtr` cast is in the locked
     stage-7 `CastKind` vocabulary (per §21.3); both
     source and destination are `Ty::Ptr` thanks to
     the source-normalization step above.

3. **Rewrite the Call.** Replace
   `Call.callee = Direct(<virtual_instance_id>)` with
   `Call.callee = Indirect(Operand::Move(Place::from(_t_fn)))`.
   **Replace `args[0]`** with
   `Operand::Move(Place::from(_t_data))` to match the
   erased-receiver fn_sig at the loaded vtable slot.
   Other args (`args[1..]`), destination, target
   unchanged. (For dyn-drops, `args.len() == 1`
   per §23.4 step 3; `args[1..]` is empty.)
4. **Allocate fresh LocalIds.** `_t_recv`, `_t_meta`,
   `_t_fn`, `_t_data`, and (for reference receivers)
   `_t_raw` are fresh `LocalId`s (with corresponding
   LocalDecl entries: `_t_recv` typed same as args[0],
   `_t_meta` typed `Ty::Ptr(<VtableType TyId>,
   Mutability::Not)`, `_t_fn` typed fn-pointer,
   `_t_raw` typed `Ty::Ptr(<dyn_ty TyId>, mut)`,
   `_t_data` typed `Ty::Ptr(<unit TyId>, mut)`).
   Register all in `body.local_decls`.

**Post-rewrite invariant timing.** §23.6.2 B-3 / B-4 /
B-6 hold post-rewrite. Pre-rewrite, args[0] type does
not match the callee's fn_sig.inputs()[0]:
- For **method-virtual calls**: args[0] is
  `&dyn Trait` (`Ty::Ref(dyn_ty, _)`) or
  `&mut dyn Trait`; callee fn_sig.inputs()[0] is
  `*const ()` or `*mut ()`.
- For **dyn-Trait drops**: args[0] is the synthetic
  `&raw mut place` of type
  `Ty::Ptr(dyn_ty, Mutability::Mut)` per §23.4 step 4;
  callee fn_sig.inputs()[0] is `*mut ()`.

The rewrite resolves the mismatch by extracting the
data pointer. Stage-11 sequencing: rewrite first,
validate after.

**Method index lookup.** The `method_idx` comes from
rustc's `tcx.vtable_entries(trait_ref)` consumption;
the method's position in the vtable layout is the
field index in the synthesised vtable type Item (per
`stage-12/expectations.md` "From stage 3" Section A,
which pins the field-order alignment).

**Charon precedent.** Charon's
`transform/normalize/transform_dyn_trait_calls.rs:31-122`
implements the equivalent pass: detects the same
input shape (`FnOperand::Regular(FnPtr{kind:
Trait/Dyn})`), synthesises the same projection chain
(`PtrMetadata` -> `Deref` -> `Field(method_field_id)`),
and mutates `call.func = FnOperand::Move(method_field_place)`
at line 119. RegseqModel's pass mirrors the structure;
the only difference is the Operand/Place vocabulary
(stage-5 + stage-6 vs Charon's owned IR).

**mrustc precedent.** mrustc's `MIR_Cleanup_Virtualize`
at `src/mir/cleanup.cpp:548-684` is a peer
cleanup-discharged variant: synthesises a vtable load
(`Field(Deref(vtable_lv), vtable_idx)`) and flips
`Call.fcn = Path(<dyn Trait>::method)` to
`Value(<vtable_loaded_lv>)`.

*Cross-refs.*
- Stage-9 `spec-snapshot.md` §23.3 (D-9-3 Call
  callee shape audit), §23.4 (D-9-2 Drop-as-Call
  collapse).
- `stage-9/discussions.md` D-9-3 (callee shape
  audit + load-bearing decision rationale).
- "From stage 3" entry above (lines 73-141) -- the
  stage-3 D5 expectation that this section closes.
- `stage-12/expectations.md` "From stage 3"
  Section A.2 (Vacant slot representation;
  field-index alignment is the load-bearing reason
  Vacant slots cannot be omitted -- our
  rewrite pass uses the field index for method
  lookup).

### Drop-as-Call bridge concerns: fresh LocalIds

Per D-9-2, the bridge collapses MIR `Drop { place,
target, ... }` into a synthetic `Call` to drop-glue.
The collapse synthesises:

1. **Pre-Call `&raw mut place` Statement.** A
   `Statement::Assign(_t_drop_arg, Rvalue::RawPtr(
   Mutability::Mut, place))` **appended as the last
   statement** of the current basic block, immediately
   before the synthetic Call terminator (and after all
   earlier statements that may have mutated `place`).
   `_t_drop_arg` is a fresh LocalId of type `*mut
   <place_ty>`. See `stage-9/spec-snapshot.md` §23.4
   step 4 "Why append, not prepend" for rationale.

2. **Fresh unit-typed return slot.** Drop-glue
   returns `()`. The Call's destination is a fresh
   LocalId `_t_drop_ret` of type `Ty::Tuple([])`.

**Stage-11 obligations:**

- **LocalDecls allocation.** `body.local_decls`
  receives two new entries per Drop-collapse site:
  `_t_drop_arg: LocalDecl { ty: Ty::Ptr(place_ty,
  Mutability::Mut), debug_name: None, ... }` and
  `_t_drop_ret: LocalDecl { ty: Ty::Tuple([]),
  debug_name: None, ... }`. The bridge populates these
  during construction (alongside the per-MIR-local
  LocalDecl entries from rustc's `Body.local_decls`).
  The Ty enum variant for raw pointers is `Ty::Ptr(TyId,
  Mutability)` per stage-3 §10 (rustc raw pointers
  translate to `Ty::Ptr` per stage-3's bridge
  translation rules); there is no `Ty::RawPtr` variant.

- **Statement insertion.** The bridge inserts the
  `Rvalue::RawPtr` Assign Statement into the basic
  block's `statements: Vec<Statement>` (per stage-8
  storage shape) at the position immediately preceding
  the Drop-becomes-Call terminator. Position is the
  end of the existing Statements list, since
  rustc's Drop is the basic block's terminator and no
  later Statements exist.

This is **bridge-construction-time work**, not a
post-construction pass. The bridge translates each
rustc terminator one-shot; for `Drop`, the translation
emits both the pre-Call Statement and the
synthesized Call terminator atomically. Stage-11
owns the LocalDecls allocation API (via
`Body::push_local(...)` or similar) that the bridge
consumes.

*Cross-refs.*
- Stage-9 `spec-snapshot.md` §23.4 (D-9-2 audit shape
  with rustc-internal + Charon precedent citations).
- `stage-9/discussions.md` D-9-2 (5-step bridge
  translation rule).
- `rustc_codegen_ssa/src/mir/block.rs:599-693` (rustc
  precedent: codegen-SSA's internal Drop-to-Call
  collapse via `Instance::resolve_drop_in_place` +
  `helper.do_call`; reference for the bridge's
  collapse mechanism).
- `charon-lib/src/transform/normalize/desugar_drops.rs:23-72`
  (Charon precedent: opt-in `--desugar-drops` pass
  rewriting `Drop` to `Call` with `&raw mut place`
  arg + fresh unit-typed return slot + copied
  target/on_unwind block ids).

### Closes construction invariant 9

Per D-9-10 B-7, stage 9 introduces the closure-
correctness invariant: every `Direct(InstanceId)`
referenced from any Terminator::Call resolves in
`model.instances`. `Indirect(Operand)` callees do
not pin an InstanceId and do not contribute to the
closure check.

Stage 1's `RegseqModel::validate` deferred this
invariant pending bodies that hold Call sites. Now
that stage 9 has pinned the Call shape, the walk
becomes non-vacuous once stage 11 materializes
`Body` / `BasicBlocks` / `Terminator`.

**Stage-11 deliverable** (extending the existing
"Construction invariant 1 extends" entry from "From
stage 1"):

`validate` walks every body's basic blocks; for each
`TerminatorKind::Call`, if `callee = Direct(id)`,
check `id` exists in `model.instances`. Add positive +
negative test in `tests/model.rs`. The deferred-stub
comment in `crates/regseq-model/src/model.rs::validate`
calls out invariant 9 explicitly per stage-1's
inbound expectation.

*Cross-refs.*
- Stage-9 `spec-snapshot.md` §23.6 (B-7 invariant
  text + walk-deferral note).
- `stage-9/discussions.md` D-9-10 (full 5-part
  registry audit including B-7).
- Stage-1 spec.md construction invariant 9 (the
  deferred invariant this section closes).


## From stage 10

Five-item outbound bundle from stage 10 (MIR basic
blocks). Stage 10 closed 2026-05-05 with all 6 D-tags
+ 21 sub-decisions LOCKED. Top-level spec.md §24
holds the locked content; this bundle records what
stage 10 deferred to stage 11 implementation.

- **`render_basic_block` deferral** (per spec.md §24.9 +
  §18.2 ownership table). Stage 11 owns the
  basic-block renderer
  `pretty.rs::render_basic_block(&BasicBlock, &Body)`
  alongside `render_place`, `render_operand`,
  `render_rvalue`, `render_statement`, and
  `render_terminator` (closes the deferral cascade
  D-5-K through D-9-K + D-10-K). The basic-block
  envelope is:
  ```
  bbN: {
      <statement_0>;
      ...
      <statement_n>;
      <terminator>;
  }
  ```
  Stub at stage-11 entry: `unimplemented!("stage 11
  fills render_basic_block")`.

- **Body walk implementation per §24.6.3 / §24.4.3.**
  Stage 11 introduces the body-level walk validator
  that enforces stage-10's Part B positive
  invariants. Procedure (per spec.md §24.4.3 steps
  1-6):
  1. Pre-check B-NonEmpty
     (`!body.basic_blocks.is_empty()`).
  2. Pre-check B-Successors-Valid (every
     `Terminator.successors()` BlockId per §23.1.4 is
     `< body.basic_blocks.len()`); precedes DFS so
     out-of-range successors surface as a clean
     diagnostic rather than as an `IndexVec` indexing
     panic.
  3. DFS from `BlockId(0)` using
     `Terminator.successors()` (per §23.1.4); record
     post-order; reverse to obtain RPO.
  4. Assert B-Reachable
     (`rpo.len() == body.basic_blocks.len()`).
  5. Assert B-RPO (positional: `rpo[i] ==
     BlockId::from_u32(i as u32)` for all i).
  6. B-Entry follows from the step-3 starting point
     plus the step-1 pre-clear of B-NonEmpty.
  Part A (rejection rules) is bridge-enforced at
  construction (per spec.md §24.7); the stage-11 walk
  inherits the post-construction guarantee without
  re-checking.

- **`BlockId` sigil-rendering helper.** Stage 11
  introduces the helper that emits `BlockId(N)` as
  `bbN` in the textual format, alongside its first
  consumer (whichever of `render_basic_block`
  envelope label, `render_terminator`'s `Goto`
  target, or `render_terminator`'s `SwitchInt`
  targets lands first in stage-11 implementation
  order). The textual convention is pinned at
  spec.md §24.3.4; stage 11 wires the rendering
  site.

- **Body-level basic-block table integration.**
  Stage 11's `Body` struct gets a public field
  `basic_blocks: IndexVec<BlockId, BasicBlock>` per
  spec.md §24.2 (bare typed-IndexVec; no wrapper, no
  cache, no alias). CFG analyses (predecessors,
  dominators, RPO iteration) live in
  `regseq-analysis` per the principled
  model-vs-analysis line documented in §24.2.3, not
  on `Body` itself.

- **`LocalId::RETURN_PLACE` migration.** When stage
  11 introduces `LocalId` as a typed-index per its
  inbound expectation "From stage 1", migrate the
  `RETURN_PLACE` const declaration to use the new
  `newtype_index!` const-body arm landed by stage 10
  (per spec.md §24.3.2):
  ```rust
  newtype_index! {
      pub struct LocalId {
          const RETURN_PLACE = 0;
      }
  }
  ```
  Keeps the body-local-IDs convention uniform with
  `BlockId::START_BLOCK = 0`.
