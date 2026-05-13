# Stage 11 -- Discussion Record

Distilled record of the design dialogue producing the
stage-11 body-level vocabulary spec. Each entry captures
the question raised, key options considered, the decision,
the reasoning chain, methodology rules invoked, prior-art
evidence, and cross-references to `spec-snapshot.md` (the
spec) and `review-log.md` (the round-by-round audit
trail; created on first review round).

Status of decisions in this file mirrors `spec-snapshot.md`'s
"Body-level vocabulary" section (§25) plus adjacent
in-place updates to §14 (FnSig placement note) and §18.2
(per-method ownership renderer-row reframing per D-11-K).

D-tag convention: prefixed `D-11-K` form per `../README.md`
§5.2.

Status tags:
- `[LOCKED]` -- audit-shape recorded; per-stage Phase A
  closed; ready for Phase B Codex review.
- `[REOPENED]` -- a later sub-decision or sibling D-tag
  forced a revision; superseded entry kept for history.
- `[DEFERRED to stage N]` -- moved to a later stage's scope.

Phase A closed 2026-05-05 with all 8 top-level D-tags +
1 sub-decision (D-11-6.1) + 1 K-tag (D-11-K) LOCKED across
a single combined cluster.

---

## 1. Decision discussions

### D-11-1 -- `Body` field set [LOCKED]

**Question.** What is the closed `Body` function-level
container shape that the bridge fills from rustc's
`mir::Body<'tcx>` and that downstream consumers (stage 12
Items, stage 14 backends) read as the unit of MIR? Stage 11
must define the field set, layout convention for the
local_decls partition, and the relationship to the parent
Item.

**Options considered.**
- (a) Flat 4-field shape: `Body { basic_blocks, local_decls,
  arg_count, span }`. rustc + rustc_public precedent.
- (b) Charon-style with `Locals` substructure:
  `Body { basic_blocks, locals: Locals, span }` where
  `Locals { arg_count, locals: IndexVec<LocalId, LocalDecl> }`.
  Co-locates partition info with data.

**Decision.** (a) flat 4-field shape. Per spec-snapshot.md
§25.1: `Body { basic_blocks: IndexVec<BlockId, BasicBlock>,
local_decls: IndexVec<LocalId, LocalDecl>, arg_count: usize,
span: Span }`.

**Reasoning.**
- rustc_public is structurally closest peer (post-mono,
  owned IR, no `'tcx`). Its 6-field shape (`{ blocks,
  locals, arg_count, var_debug_info, spread_arg, span }`)
  collapses to 4 in RegseqModel after dropping
  `var_debug_info` (lifted to per-LocalDecl per D-8-3 +
  D-11-2.5) and `spread_arg` (rust-call ABI rejected per
  D15).
- Charon's `Locals` substructure pays off because Charon's
  locals carry richer state (binder kind, mut flag, phase
  markers) that benefit from grouping. RegseqModel's
  4-field LocalDecl (D-11-2) is simpler; substructure adds
  layer without proportional benefit.
- Semantic operations (`return_local()`, `arg_locals()`,
  `inner_locals()`, `fresh_var()`) work cleanly as methods
  on Body directly without requiring substructure.
- Body has no `fn_sig` field (D-11-3 reframe; FnSig on
  Item-side per 4-peer convergence).
- Body has no `item: ItemId` back-pointer (D-11-1.7 zero
  shape; matches D-11-5 alt (c)).
- 14 rustc Body fields are dropped; rejection rationale per
  field documented in §25.1's rejection table.

**Methodology rules invoked.**
- *C-semantics with information preservation* -- store
  rustc's resolved body form (post-mono, post-cleanup);
  the 17->4 collapse follows mechanically from RegseqModel
  binding constraints.
- *Compositional behavior beats special-case flags* -- no
  parallel `is_post_mono` / `is_no_debuginfo` / etc. flags;
  the dropped fields fall out compositionally from each
  binding constraint.
- *Re-derive from evidence; do not import baggage* -- field
  set re-derived from kickoff evidence + 4-peer prior-art,
  not lifted from rustc's full Body shape.

**Prior-art evidence.**
- rustc: 17-field `Body<'tcx>` at `mir/mod.rs:208-335`;
  rationale per dropped field at `prior-art/rustc.md:32`
  §1.
- rustc_public: 6 fields `{ blocks, locals, arg_count,
  var_debug_info, spread_arg, span }` at `body.rs:13-38`;
  see `prior-art/rustc_public.md`. Closest peer to
  RegseqModel.
- Charon: 3 fields `{ basic_blocks, locals: Locals, span }`
  with `Locals { arg_count, locals }` substructure at
  `gast.rs:31-41`; see `prior-art/charon.md`.
- mrustc: 3-field minimum `{ locals, drop_flags, blocks }`
  at `mir.hpp:1117-1130`; see `prior-art/mrustc.md`.

**Worked example.**
```rust
// Constructed Body for `fn add(x: u32, y: u32) -> u32 { x + y }`:
Body {
    basic_blocks: IndexVec::from_iter(vec![
        BasicBlock { statements: ..., terminator: Return },
    ]),
    local_decls: IndexVec::from_iter(vec![
        LocalDecl { ty: t_u32, mutability: Mut, span: ..., debug_name: None },     // _0 RETURN_PLACE
        LocalDecl { ty: t_u32, mutability: Not, span: ..., debug_name: Some("x") }, // _1 arg
        LocalDecl { ty: t_u32, mutability: Not, span: ..., debug_name: Some("y") }, // _2 arg
    ]),
    arg_count: 2,
    span: ...,
}
```

**Spec cross-reference.** §25.1.

**Sub-decisions.**
- **D-11-1.1** Cardinality: 4 fields. [LOCKED]
- **D-11-1.2** `basic_blocks: IndexVec<BlockId, BasicBlock>`
  inherited from stage-10 D-10-2; placement on Body
  confirmed. [LOCKED]
- **D-11-1.3** `local_decls: IndexVec<LocalId, LocalDecl>`
  flat (no Locals substructure). [LOCKED]
- **D-11-1.4** `arg_count: usize` flat alongside local_decls.
  Layout convention `[0]=RETURN_PLACE, [1..=arg_count]=args,
  [arg_count+1..]=temps` matches rustc + rustc_public + Charon.
  Method shorthands on Body without substructure layer.
  [LOCKED]
- **D-11-1.5** `span: Span` whole-body diagnostic location;
  per-construct diagnostics use their own spans. rustc +
  rustc_public + Charon all keep this. [LOCKED]
- **D-11-1.6** Rejection table for 14 dropped rustc Body
  fields with one-line rationale per drop (post-mono / no
  DWARF / borrowck-discharged / coroutine-rejected /
  collector-discharged / instrumentation-rejected /
  debuginfo-lifted-to-LocalDecl / Item-side-linkage). See
  §25.1 rejection table. [LOCKED]
- **D-11-1.7** No body-level `item: ItemId` field; Body↔Item
  linkage Item-side per D-11-5 (zero shape). Matches
  rustc_public + Charon (no body-side back-pointer);
  rustc's `MirSource` is closest analog and is dropped per
  the rejection table. [LOCKED]

---

### D-11-2 -- `LocalDecl` field set [LOCKED]

**Question.** What is the closed `LocalDecl` per-local
container shape that the bridge fills from rustc's
`mir::LocalDecl<'tcx>` (combined with body-level
`var_debug_info`) and that the joint validation walk plus
stage-13 renderers consume?

**Options considered.**
- (a) 3-field `{ ty, span, mutability }` (rustc_public).
- (b) 3-field `{ index, name, ty }` (Charon; no
  mutability, no span).
- (c) 4-field `{ ty, mutability, span, debug_name }` (lift
  named-local from body-level var_debug_info per D-8-3).
- (d) Bare `TypeRef` (mrustc).

**Decision.** (c) 4-field shape per stage-8 D-8-3's outbound:
`LocalDecl { ty: TyId, mutability: Mutability, span: Span,
debug_name: Option<StringId> }`.

**Reasoning.**
- The named-local lift to per-LocalDecl is justified by
  C-shape ergonomics: per-local lookup at render time +
  codegen identifier emission without secondary scan over
  body-level `var_debug_info`. rustc itself partitions
  `var_debug_info` per-`Local` for codegen consumption
  via `FunctionCx.per_local_var_debug_info` -- structurally
  identical to the design D-8-3 specified.
- `mutability` and `span` preserve information rustc gives
  us for free (information-preservation rule); cost is
  small (1 byte + 4 bytes per LocalDecl); bridge has the
  data. Charon's drop is purchasable because Charon is a
  verification IR; RegseqModel's C-codegen + author
  debugging benefits from preservation.
- `debug_name = None` covers all synthetic locals
  (compiler-synthesized rustc temps; bridge-introduced
  Drop-as-Call locals; rewrite-pass-introduced vtable-
  rewrite locals). No parallel `is_synthetic` flag needed
  (compositional rule).
- Charon's `Local.name: Option<Symbol>` is direct positive
  precedent for the lift. mrustc's commented-out
  `local_names` is negative precedent (their HIR-from-MIR
  flow doesn't preserve, so they rejected). RegseqModel
  inherits debug-friendliness as a design priority.

**Methodology rules invoked.**
- *C-semantics with information preservation* -- preserve
  mutability + span + debug_name; rustc has the info; storing
  it costs little.
- *Compositional behavior beats special-case flags* -- no
  `is_synthetic` flag; `debug_name=None` distinguishes synthetic.

**Prior-art evidence.**
- rustc: 5 fields + 9-variant `LocalInfo` at
  `mir/mod.rs:951-1054`; LocalInfo is mostly diagnostic /
  borrowck (almost entirely upstream-discharged for our
  purposes). See `prior-art/rustc.md:104` §2 (with §2.1-2.5
  sub-sections at lines 134, 142, 146, 150, 160).
- rustc_public: 3 fields `{ ty, span, mutability }` at
  `body.rs:126-131`; named-locals body-level via
  `var_debug_info` + `VarDebugInfo::local()` helper at
  `body.rs:732-739`. See `prior-art/rustc_public.md:176`
  §2.
- Charon: 3 fields `{ index, name: Option<Symbol>, ty }` at
  `gast.rs:47-52`; positive precedent for debug_name lift.
  See `prior-art/charon.md:139` §2.
- mrustc: bare `HIR::TypeRef` per local; commented-out
  `local_names` (rejected). See `prior-art/mrustc.md:141`
  §2.

**Spec cross-reference.** §25.2.

**Sub-decisions.**
- **D-11-2.1** Cardinality: 4 fields. [LOCKED]
- **D-11-2.2** `ty: TyId` (stage-3 type interner reference).
  [LOCKED]
- **D-11-2.3** `mutability: Mutability` (stage-3 D11 enum
  `{ Not, Mut }`); information-preservation rationale.
  [LOCKED]
- **D-11-2.4** `span: Span` (stage-1 file-local Span);
  whole-LocalDecl diagnostic location; `Span::dummy()` for
  synthetic locals. [LOCKED]
- **D-11-2.5** `debug_name: Option<StringId>` lifted from
  body-level `var_debug_info` at construction time per
  stage-8 D-8-3 outbound. Closes D-8-3's outbound. The
  bridge translates each `VarDebugInfo` entry whose `value =
  VarDebugInfoContents::Place(p)` with empty projection
  into a per-LocalDecl.debug_name for `p.local`. Composite
  / non-Place / non-empty-projection entries are
  bridge-rejected. [LOCKED]
- **D-11-2.6** Rejection table for 3 dropped rustc LocalDecl
  fields (`local_info` 9-variant enum / `user_ty` / `source_info.scope`)
  with one-line rationale per drop. See §25.2's rejection
  table. [LOCKED]

---

### D-11-3 -- `FnSig` location reframe [LOCKED]

**Question.** Where does the function signature live in
RegseqModel's IR — on the Body, on the Item, or as a
side-table? Stage-1 inbound framed `FnSig` as a structural
field of Body (with `abi: FnAbi` as a structural field of
FnSig per Q2). Stage-11 must answer the placement question
left open by stage 1.

**Options considered.**
- (a) Body field: `Body { ..., fn_sig: FnSig }`. Stage-1
  inbound reading.
- (b) Item field: `Item.kind = Function { fn_sig, ... }`
  per stage-12's Item.kind expansion.
- (c) Instance field: `InstanceKind::Function { flavor,
  virtual_idx, fn_sig, ... }` extending stage-9 §9.4.
- (d) Drop `fn_sig` field; reach via `local_decls[0..=arg_count]`
  + abi on Body.
- (e) FnSig on Item only; Body has no fn_sig at all.

**Decision.** (e) FnSig on Item-side; Body has no fn_sig
field. 4-peer convergence (rustc, rustc_public, Charon,
mrustc); supersedes stage-1 inbound's Body-side framing.

**Reasoning.**
- 4-peer evidence (rustc-internal `tcx.fn_sig(def_id)`,
  rustc_public `FnDef::fn_sig() -> PolyFnSig` at
  `ty.rs:713-717`, Charon `FunDecl.signature: FunSig`
  separate from `FunDecl.body`, mrustc HIR-layer `Function`
  signature) all keep signature Item-side. No peer puts it
  Body-side.
- Function-defining Items carry the signature; bodies
  consume content shaped by it but don't duplicate the
  signature object. Foreign declarations (extern fns
  with no body) still have signatures via the Item-side
  field — alt (a) Body-side cannot accommodate this case.
- §14's canonical FnSig schema (5 fields: `inputs`, `output`,
  `abi`, `c_variadic`, `safety`) is independent of placement;
  stage-3 D15 already locked the abi enum. Stage-1 inbound's
  schema framing stands; only the placement reframes.
- Stage-1's `sigs` side-table framing fails Q2 (eager
  translation; fully-owned IR; no side-channel lookups).
  Item-side structural field IS the eager owned representation.
- Argument types remain reachable from Body via positional
  layout (`local_decls[1..=arg_count]`); return type via
  `local_decls[RETURN_PLACE]`. Body-side schema is not lossy;
  signature is just not duplicated.
- Stage 12 (Items) absorbs the precise schema (`Item.kind =
  Function { fn_sig, ... }` per its expanded variant set);
  stage 11 only locks the placement decision.

**Methodology rules invoked.**
- *Spec is mutable* -- stage-1 inbound's framing is reframed
  based on evidence-driven 4-peer convergence; the schema
  carries through but the placement was an open question.
- *Re-derive from evidence; do not import baggage* -- the
  Item-side placement is the convergent answer evidence
  surfaces; stage-1's Body-side reading was a default to be
  pressure-tested, not a binding choice.
- *Compositional behavior beats special-case flags* -- no
  parallel `has_signature` flag on Body; foreign declarations
  use `Item.body = None` + Item.kind=Function.fn_sig=Some.

**Prior-art evidence.**
- rustc: signature reached via `tcx.fn_sig(def_id) ->
  EarlyBinder<PolyFnSig>` at `ty/mod.rs`; `Body::return_ty()`
  reads `local_decls[RETURN_PLACE].ty` directly. See
  `prior-art/rustc.md:173` §3 (with §3.1-3.4 sub-sections
  at lines 194, 202, 211, 217).
- rustc_public: `FnDef::fn_sig() -> PolyFnSig` at `ty.rs:713-717`;
  no body-level fn_sig field at `body.rs:13-38`. Argument
  types reached via `Body::arg_locals()` helper. See
  `prior-art/rustc_public.md:63` §1 (Body field set; covers
  the no-fn_sig observation directly).
- Charon: `FunDecl.signature: FunSig` separate structural
  field from `FunDecl.body: Body`; `FunSig { is_unsafe,
  generics, inputs, output }` carries no abi (verification
  IR specifics). See `prior-art/charon.md:209` §3.
- mrustc: HIR-layer `Function::m_sig` carrying `m_variadic`,
  `m_unsafe`; no MIR-layer FnSig. See
  `prior-art/mrustc.md:200` §3.

**Cross-stage friction documentation.** Stage-1 inbound
expectation framed FnSig as "a structural field of FnSig
... structural, not a side-channel lookup." Stage-1's
framing pinned: (1) FnSig is structural and owned (Q2),
(2) abi is inside FnSig, not in a separate side-channel.
The stage-1 framing did NOT pin location (Body vs Item);
the implied "Body" reading was a default. D-11-3 reframes
the location to Item-side without contradicting the
stage-1 schema or Q2/D15. The reframe is a "Spec is
mutable" application.

**Spec cross-reference.** §14 (placement note added);
§25.4 (Body has no fn_sig field; signature reachable
through `local_decls`).

**Sub-decisions.**
- **D-11-3.1** Body has no `fn_sig` field (negative pin;
  closes the stage-1 inbound's Body-side framing). [LOCKED]
- **D-11-3.2** FnSig location: Item-side per (e); 4-peer
  convergence. Stage-12 finalizes the precise placement
  (`Item.kind = Function { fn_sig, ... }` likely; could be
  refined). [LOCKED]
- **D-11-3.3** Argument types reached via
  `Body.local_decls[1..=arg_count].ty` (positional
  convention; matches rustc + rustc_public + Charon).
  [LOCKED]
- **D-11-3.4** Return type reached via
  `Body.local_decls[RETURN_PLACE].ty` (where `RETURN_PLACE
  = LocalId(0)` per D-11-4); matches rustc's `Body::return_ty()`
  + rustc_public's positional helpers. [LOCKED]
- **D-11-3.5** Outbound expectation to
  `stage-12/expectations.md` "From stage 11 (per D-11-3)":
  stage 12 absorbs FnSig as structural field of
  `Item.kind = Function`. The canonical FnSig schema is the
  pinned 5-field form per §14.3 (`inputs: Vec<TyId>`,
  `output: TyId`, `abi: FnAbi`, `c_variadic: bool`,
  `safety: Safety`); D-11-3 does NOT revise §14's schema.
  Stage 12 absorbs the §14 schema as a structural field of
  `Item.kind = Function`; the placement decision is
  stage-11's contribution, the schema is stage-1/stage-3's
  pinned form. [LOCKED]
- **D-11-3.6** Stage-1 inbound's "FnSig as structural field
  of Body" framing is **superseded** by (e); the reframe
  is documented in this audit-shape entry per the
  "Spec is mutable" methodology rule. [LOCKED]

---

### D-11-4 -- `LocalId` typed-index [LOCKED]

**Question.** How is `LocalId` declared in the IR? Stage-1
forward-declared it conceptually only; stage-11 lands the
typed-index introduction, the `RETURN_PLACE` const,
size/niche assertions, and the sigil convention.

**Options considered.**
- (a) `newtype_index!` macro with `const RETURN_PLACE = 0;`
  using stage-10 D-10-3.2's const-body arm. Mirrors stage-10's
  `BlockId::START_BLOCK` pattern.
- (b) Plain u32 newtype with manual associated const + manual
  niche.

**Decision.** (a) `newtype_index!` with const-body declaration.
Per spec-snapshot.md §25.3.

**Reasoning.**
- Symmetric with stage-10 D-10-3 (BlockId via newtype_index!
  + macro arm extension + niche/size assertions + sigil
  convention). Mechanical inheritance of established pattern.
- Stage-10 D-10-3.2 already extended the macro grammar with
  the const-body arm; stage 11 consumes it. Closes stage-10's
  deferred `LocalId::RETURN_PLACE` migration outbound.
- rustc + Charon use proper typed newtype indexes;
  rustc_public + mrustc use bare aliases (negative precedent
  for the alt-(b) bare-typedef approach).
- `RETURN_PLACE` naming matches rustc convention and stage-1
  inbound text (vs rustc_public's `RETURN_LOCAL`); the
  "PLACE" suffix signals its role in `Place { local:
  RETURN_PLACE, projection: [] }`.

**Methodology rules invoked.**
- *Re-derive from evidence; do not import baggage* -- typed
  index discipline derived from stage-1 ID conventions +
  rustc/Charon evidence.
- *Compiler-theory-first* -- typed indexes prevent confusion
  between `LocalId` and `BlockId` at compile time; bare
  typedefs (mrustc/rustc_public) lose this safety.

**Prior-art evidence.**
- rustc: `newtype_index!` Local with `const RETURN_PLACE = 0`
  at `mir/mod.rs:842-850`. Const-body grammar landed at
  `rustc_index_macros/src/newtype.rs:87`. See
  `prior-art/rustc.md:223` §4 (with §4.1-4.2 sub-sections
  at lines 256, 272).
- rustc_public: bare `pub type Local = usize` with
  `RETURN_LOCAL = 0` at `body.rs:850`; weaker discipline.
  See `prior-art/rustc_public.md`.
- Charon: `generate_index_type!` macro at `src/ids/mod.rs`;
  ZERO universal constant + `MAX_INDEX = u32::MAX` niche.
  See `prior-art/charon.md`.
- mrustc: typedef-asymmetry (`BasicBlockId` typed at
  `mir.hpp:20`, locals raw `unsigned int`); D-11-4 closes
  this asymmetry. See `prior-art/mrustc.md`.

**Worked example.**
```rust
// crates/regseq-model/src/idx.rs
newtype_index! {
    pub struct LocalId {
        const RETURN_PLACE = 0;
    }
}

// crates/regseq-model/src/lib.rs (assertions appended)
const _: () = assert!(size_of::<LocalId>() == 4);
const _: () = assert!(size_of::<Option<LocalId>>() == 4);
```

**Spec cross-reference.** §25.3.

**Sub-decisions.**
- **D-11-4.1** `LocalId` introduced via `newtype_index!`
  in `crates/regseq-model/src/idx.rs`. Mechanical inheritance
  from stage-1 ID conventions + stage-10 D-10-3.1 BlockId
  parallel. Closes stage-1 inbound "Define LocalId via
  newtype_index!". [LOCKED]
- **D-11-4.2** `const RETURN_PLACE = 0` declared inline
  using stage-10 D-10-3.2's const-body macro arm. Closes
  stage-10 D-10-3.2's deferred migration. Closes stage-1's
  RETURN_PLACE convention framing. [LOCKED]
- **D-11-4.3** `size_of::<LocalId>() == 4` and
  `size_of::<Option<LocalId>>() == 4` static assertions
  appended to `crates/regseq-model/src/lib.rs` alongside
  the stage-10 BlockId assertions. Closes stage-1 inbound
  niche optimization. [LOCKED]
- **D-11-4.4** Sigil textual convention `LocalId(N) -> _N`
  pinned at stage 11; rendering site **deferred to stage
  13 per D-11-K**. [LOCKED]

---

### D-11-5 -- `BodyId` vs `ItemId` resolution [LOCKED]

**Question.** Does RegseqModel allocate a separate `BodyId`
for bodies, or do bodies live as a structural field of Item
without a distinct ID? Stage-1 inbound called this an open
question explicitly.

**Options considered.**
- (a) `BodyId == ItemId` numerically (one ID space; same
  numeric value indexes both).
- (b) Distinct `BodyId`; bodies hold back-link to ItemId.
- (c) No `BodyId`; `Item.body: Option<Body>` structural
  field.

**Decision.** (c) no BodyId. Bodies live as a structural
field of Item per spec-snapshot.md §25.5.

**Reasoning.**
- 4-peer convergence: rustc has no BodyId (`tcx.instance_mir(InstanceKind)`
  is the only fetch path); rustc_public has no BodyId
  (`Instance::body() -> Option<Body>` at `mono.rs:53-55`);
  Charon has no BodyId (`FunDecl.body: Body` direct
  structural field); mrustc HIR-layer has no BodyId.
- Composes with Q3 (single-level IR; bodies are content of
  items, not peers).
- Composes with Q2 (eager translation, fully-owned IR;
  `Option<Body>` is owned content, not a query).
- Composes with D-11-3 alt (e) (FnSig on Item-side);
  signature + body co-located on the same Item creates a
  coherent function representation.
- Saves an ID space + lookup table.

**Methodology rules invoked.**
- *Re-derive from evidence; do not import baggage* -- no
  peer allocates BodyId; the stage-1 open question resolves
  per evidence.
- *Compositional behavior beats special-case flags* -- no
  parallel BodyId table; bodies live structurally.

**Prior-art evidence.**
- rustc: no BodyId; `tcx.instance_mir(InstanceKind)` at
  `ty/mod.rs:1671`. See `prior-art/rustc.md:290` §5 (with
  §5.1-5.2 sub-sections at lines 308, 316).
- rustc_public: `Instance::body() -> Option<Body>` at
  `mono.rs:53-55` returning owned Body. Same shape used by
  `ClosureDef::body()` and `CoroutineDef::body()`. See
  `prior-art/rustc_public.md`.
- Charon: `FunDecl.body: Body` direct structural field at
  `gast.rs` FunDecl. See `prior-art/charon.md`.
- mrustc: HIR-layer `Function::m_code: ExprPtr` (different
  architecture but same effect: no separate BodyId). See
  `prior-art/mrustc.md`.

**Spec cross-reference.** §25.5.

**Sub-decisions.**
- **D-11-5.1** No `BodyId` allocation; no separate ID space
  for bodies. 4-peer convergence. [LOCKED]
- **D-11-5.2** Bodies live as a structural field of `Item`
  per (c); stage-12 finalizes the precise placement
  (`Item.body` top-level vs `ItemKind::Function.body`
  variant-internal). [LOCKED]
- **D-11-5.3** `Option<Body>` accommodates: foreign
  declarations (`extern "C" fn` with no body) -> `body =
  None`; non-Function ItemKinds (Struct, Enum, Union, Static,
  Const, vtable type Items) -> body variant-specific.
  Stage-11 pins `Option<Body>` as the wrapper; stage-12 picks
  the structural placement. [LOCKED]
- **D-11-5.4** Body-fetch convention: walkers reach Body
  via Item; no reverse lookup. Composes with D-11-1.7 zero-
  shape Body↔Item linkage. [LOCKED]
- **D-11-5.5** Outbound expectation to `stage-12/expectations.md`
  "From stage 11 (per D-11-5)": stage 12 absorbs Body as
  structural component of Item per (c). Joins the D-11-3
  outbound (FnSig on Item) into the broader stage-12
  Item.kind=Function expansion. [LOCKED]

---

### D-11-6 -- Joint validation walk shape and ownership [LOCKED]

**Question.** Where does the body-level validation walk
live, and what is its visitor-trait shape? 5 deferred
walk-implementations (D-5-2, D-7-3, D-8-4 Part C, D-9-10
Part C, D-10-5 Part C) converge on stage 11 plus the
construction invariants 1+9 fold-in.

**Options considered.**
- (a) Direct-loop walk in a `validate(model)` function
  (mrustc's `MIR_Validate` pattern at `check.cpp:445`).
- (b) Visitor trait with override hooks (rustc_public's
  `MirVisitor` + Charon's `VisitAst`/`VisitBody` bi-modal
  shape).

**Decision.** (b) visitor with override hooks. Located in
`crates/regseq-model/src/visit.rs` per stage-10 model-vs-
analysis line. 10 `visit_X` / `super_X` hook pairs covering
body, basic_block, local_decl, statement, terminator, place,
rvalue, operand, ty, call_callee.

**Reasoning.**
- 3-peer positive (rustc, rustc_public, Charon) + 1 negative
  (mrustc); convergent on visitor-with-override-hooks.
- mrustc's direct-loop walk forfeits visitor composability;
  every new check requires re-implementing traversal logic.
  Negative precedent.
- rustc_public's `make_mir_visitor!` macro at `visit.rs:42-517`
  is the load-bearing template (17 visit/super pairs;
  exhaustive-destructuring discipline). RegseqModel uses the
  same template adapted to its smaller surface.
- Validation enforces invariants the bridge claims to
  uphold — that's a model concern (live with `RegseqModel::validate`),
  not an analysis. Per stage-10 model-vs-analysis line:
  the IR knows how to validate itself; CFG analyses
  (predecessors, dominators, RPO iteration) live in
  `regseq-analysis`.
- All 5 deferred walks fold cleanly into the visitor's
  hooks (D-5-2 → visit_place; D-7-3 → visit_rvalue;
  D-8-4 → visit_statement; D-9-10 → visit_terminator;
  D-10-5 §24.4.3 → visit_body pre-check + traversal).
- Read-only `Visitor` trait at stage 11; `MutVisitor` for
  transformation passes (per D-11-7) is a phase-1 impl
  extension — those passes do direct mutation rather than
  generic visitor (mrustc's `MIR_Cleanup_Virtualize` at
  `cleanup.cpp:548-684` precedents direct mutation).

**Methodology rules invoked.**
- *Compiler-theory-first* -- visitor pattern is the standard
  compiler-IR walk; rich literature on traversal patterns.
- *Compositional behavior beats special-case flags* -- 10
  hook pairs compose into one walk; alternative (multiple
  passes) duplicates traversal cost.
- *Re-derive from evidence; do not import baggage* -- hook
  surface is RegseqModel-specific (10 vs rustc_public's 17),
  not lifted wholesale.

**Prior-art evidence.**
- rustc: `make_mir_visitor!` macro at `mir/visit.rs:1-160`;
  rich surface with PlaceContext / MutatingUseContext.
  See `prior-art/rustc.md:332` §6 (with §6.1-6.6 sub-sections
  covering codegen-SSA walk / cranelift walk /
  BuilderMethods abstraction / make_mir_visitor! template /
  pre-walk pattern / RegseqModel implications).
- rustc_public: `MirVisitor` + `MutMirVisitor` via
  `make_mir_visitor!` at `visit.rs:42-517`. 17 visit/super
  hook pairs; exhaustive-destructuring discipline. See
  `prior-art/rustc_public.md:385` §5 (with §5.1-5.5
  sub-sections at lines 393, 427, 460, 502, 531).
- Charon: bi-modal `VisitAst` (full crate) + `VisitBody`
  (body-only) at `src/ast/visitor.rs:130-191`. See
  `prior-art/charon.md:408` §6.
- mrustc: direct-loop in `MIR_Validate` at
  `check.cpp:445` (negative precedent). See
  `prior-art/mrustc.md:341` §6.

**Worked example.**
```rust
// crates/regseq-model/src/visit.rs (sketch)
pub trait Visitor {
    fn visit_body(&mut self, body: &Body) { self.super_body(body) }
    fn super_body(&mut self, body: &Body) {
        // pre-checks: B-NonEmpty, B-Successors-Valid (§25.6.2)
        // DFS-RPO traversal: B-RPO + B-Reachable + B-Entry
        for decl in &body.local_decls { self.visit_local_decl(decl); }
        for bb in &body.basic_blocks { self.visit_basic_block(bb); }
    }
    // ... 9 more visit_X / super_X hook pairs ...
}
```

**Spec cross-reference.** §25.6 (visitor trait + 5 deferred
walk absorption + body-level §24.4.3 procedure).

**Sub-decisions.**
- **D-11-6.1** Construction invariants 1 (TyId closure) and
  9 (Direct(InstanceId) closure) fold into existing visitor
  hooks (visit_local_decl + visit_ty for inv 1;
  visit_terminator::Call for inv 9); no separate pre-pass.
  [LOCKED]
- **D-11-6.2** Visitor trait location:
  `crates/regseq-model/src/visit.rs`. Per Q1 + stage-10
  model-vs-analysis line. [LOCKED]
- **D-11-6.3** Hook surface: 10 visit/super pairs (body,
  basic_block, local_decl, statement, terminator, place,
  rvalue, operand, ty, call_callee). Subset of rustc_public's
  17 — RegseqModel doesn't carry SwitchTargets / VarDebugInfo
  / per-renderer hooks because rendering is stage-13's per
  D-11-K and var_debug_info is consumed at construction.
  [LOCKED]
- **D-11-6.4** Mutability discipline: read-only `Visitor`
  trait at stage 11. `MutVisitor` for transformation passes
  is phase-1 impl extension; not specified at stage 11
  (passes per D-11-7 do direct mutation, not generic
  visitor). [LOCKED]
- **D-11-6.5** Body-level walk procedure (§24.4.3
  absorption): `visit_body` implements 6-step DFS-RPO with
  B-NonEmpty + B-Successors-Valid pre-checks GUARDING
  subsequent indexing; B-Reachable + B-RPO assertions in
  steps 4-5; B-Entry implicit from start point + step-1
  pre-check. [LOCKED]
- **D-11-6.6** Walk-implementation absorption summary: 5
  deferred walks (D-5-2 / D-7-3 / D-8-4 Part C / D-9-10
  Part C / D-10-5 Part C) all close at stage-11 joint
  visitor; per-construct invariants from each owning stage's
  spec sections (§19.7, §20.5, §21.5, §22.5, §23.6, §24.4.3)
  enforced at the appropriate hooks. [LOCKED]

---

### D-11-7 -- Body-level transformation pass timing [LOCKED]

**Question.** When do body-level transformation passes
run — at bridge construction time, as post-construction
passes, or hybrid? Two passes are in scope: Drop-as-Call
collapse (per stage-9 D-9-2 + §23.4) and vtable-indirect
rewrite (per stage-3 D5 + stage-9 D-9-3 (b)).

**Options considered.**
- (a) Both at bridge construction time (atomic per-rustc-
  terminator translation).
- (b) Both post-construction passes (Charon's
  `desugar_drops` + `transform_dyn_trait_calls`).
- (c) Hybrid: Drop-as-Call at bridge time; vtable-rewrite
  post-construction.

**Decision.** (c) hybrid timing per spec-snapshot.md §25.7.

**Reasoning.**
- **Drop-as-Call at bridge time is FORCED** by D-9-1.1's
  rejection of the `Drop` variant from `TerminatorKind`.
  The bridge MUST translate `mir::TerminatorKind::Drop` —
  there is no Drop variant in RegseqModel's vocabulary to
  defer. Bridge-time is the only valid timing.
- **Vtable-rewrite at post-construction is structurally
  indicated.** Bridge-time rewrite would couple stage-12
  vtable Item synthesis with bridge body construction
  (vtable types must be Item-collected before the rewrite
  can reference them). Post-construction matches Q2 (eager
  translation produces fully-owned IR; passes mutate
  fully-owned IR) cleanly.
- rustc's codegen-time direct dispatch (via
  `Instance::Virtual` + `tcx.vtable_entries`) is structurally
  inadmissible for C-emission under Q2 (no `'tcx`
  post-bridge; codegen has no `tcx` access).
- Charon + mrustc both run vtable-rewrite post-construction.
  Convergent precedent.

**Cross-stage friction documentation (D-11-7.6).**
Charon's compact in-place projection-chain rewrite at line
119 of `transform_dyn_trait_calls.rs` does:
```
call.func = FnOperand::Move(method_field_place)
```
where `method_field_place = (*receiver.ptr_metadata).method`
uses a `PtrMetadata` PlaceElem variant in Charon's
projection vocabulary.

**Stage-5 D-5-1 locked a 6-variant `PlaceElem` set: Deref /
Field / Index / ConstantIndex / Subslice / Downcast. NO
`PtrMetadata` variant.**

Without `PtrMetadata` as a PlaceElem, RegseqModel cannot
inline the metadata extraction into a Place projection
chain. The metadata MUST be extracted as a separate
`Rvalue::UnaryOp(UnOp::PtrMetadata, ...)` Statement. The
chain expands to 4-5 fresh locals (`_t_recv`, `_t_meta`,
`_t_fn`, `_t_data`, optionally `_t_raw` for Ref source-
normalization) vs Charon's 0-fresh-locals form.

The verbosity is **forced by stage-5's locked vocabulary**,
not a stage-11 design choice. Reopening D-5-1 to add
`PtrMetadata` is out of stage-11 scope (would cascade
through stage-5's projection-typing rules + walk + invariants
+ §19.4 type-narrowing). `UnOp::PtrMetadata` IS in stage-7
D-7-1.6's locked `UnaryOp` enum — that's how the metadata
extraction happens via Rvalue rather than via Place.

**D-9-3 reopening question (mid-Phase-A).** During Phase A
discussion, the question was raised: should stage-9 D-9-3 be
reopened to alt (a) (keep Direct(InstanceId) for virtual;
codegen-side dispatch on `flavor: Virtual` via existing
`virtual_idx` payload)? Alt (a) would have removed the
stage-11 vtable-rewrite pass entirely.

Conclusion: **don't reopen, proceed with hybrid (c)**:
- The cross-stage friction with stage-5 D-5-1 (no
  `PtrMetadata` PlaceElem) forces multi-statement rewrite
  shape regardless of which stage owns the rewrite — not a
  stage-9-specific cost.
- Stage-9's IR-uniformity preference (one Call code path
  post-rewrite) is reasonable; D-9-3 trade-off was
  IR-uniformity-vs-codegen-simplicity, and stage-9 picked
  IR-uniformity with 12 rounds of Codex review pedigree.
- Reopening cost (Phase A re-do for stage-9, merger
  contract re-open, ~10-15 stage-9 findings re-confirm) is
  unfavorable for a stylistic preference shift.
- Stage-11 absorbs the rewrite-pass cost; D-11-7 hybrid (c)
  timing locks.

**Pass scheduling (D-11-7.5).** Vtable-rewrite runs
**before** the joint validation walk. Stage-9 §23.6.2
B-3/B-4/B-6 invariants assume post-rewrite shape;
running validation pre-rewrite would emit spurious
B-3/B-4/B-6 errors at every Virtual-flavor Call site
(pre-rewrite, args[0] type does not match
`callee_fn_sig(callee_id).inputs()[0]` for Virtual calls;
the helper resolves the signature Item-side per
§23.6.2's stage-11 reframe). Stage-9 expectations.md
explicitly says: "Stage-11 sequencing: rewrite first,
validate after."

**Methodology rules invoked.**
- *Compiler-theory-first* -- post-construction transformation
  passes are standard compiler practice; the timing decision
  derives from coupling-avoidance (Q2 + stage-12 Item
  synthesis dependency).
- *No defensive fallbacks* -- rustc-style codegen-time
  direct dispatch is structurally inadmissible for our
  targets, not a fallback to consider.
- *Compositional behavior beats special-case flags* --
  rewrite produces post-rewrite invariants compositionally;
  no parallel "is post-rewrite" flag.
- *C-semantics with information preservation* -- post-rewrite
  IR carries the projection-chain explicitly; the synthesis
  is C-shape (data + function-pointer pairs) not Rust-shape.
- *Spec is mutable (rejected as too costly here)* --
  reopening stage-9 D-9-3 is allowed by methodology but
  cost-benefit is unfavorable.

**Prior-art evidence.**
- rustc: deliberately does NOT rewrite — codegen-time
  direct dispatch via `Instance::Virtual` +
  `meth::VirtualIndex::from_index(idx).get_fn(...)` reading
  `tcx.vtable_entries`. Structurally inadmissible for our
  C-emission target. See `prior-art/rustc.md:399` §7
  (with §7.1 sub-section at line 403 covering the
  codegen-SSA Virtual-call path).
- rustc_public: silent (no transformation passes). See
  `prior-art/rustc_public.md`.
- Charon: `desugar_drops` post-construction pass
  (`desugar_drops.rs:23-72`); `transform_dyn_trait_calls`
  post-construction pass (`transform_dyn_trait_calls.rs:31-122`)
  registered in INITIAL_CLEANUP_PASSES at `mod.rs:104, 111`.
  In-place projection-chain rewrite at line 119; load-bearing
  precedent for D-11-7 with the cross-stage friction note.
  See `prior-art/charon.md`.
- mrustc: HIR-level Drop handling (different architecture);
  `MIR_Cleanup_Virtualize` at `cleanup.cpp:548-684`
  post-construction post-monomorphization timing; single-
  field `Call.fcn` rewrite at line 685 matching
  `CallCallee::Indirect(Operand)`. Load-bearing precedent.
  See `prior-art/mrustc.md`.

**Spec cross-reference.** §25.7 (Drop-as-Call collapse rules
+ vtable-rewrite procedure with cross-stage friction note +
pass scheduling).

**Sub-decisions.**
- **D-11-7.1 [master]** Hybrid (c) timing: Drop-as-Call at
  bridge construction time + vtable-indirect rewrite as
  post-construction pass. [LOCKED]
- **D-11-7.2** Drop-as-Call at bridge time. Forced by
  D-9-1.1. Allocates `_t_drop_arg` (`*mut <place_ty>`) and
  `_t_drop_ret` (`()`) per Drop site via D-11-8 API. Inserts
  `Statement::Assign(_t_drop_arg, Rvalue::RawPtr(Mut, place))`
  appended to current basic block's statements list. Per
  stage-9 §23.4 step 4. [LOCKED]
- **D-11-7.3** Vtable-indirect rewrite as post-construction
  pass. Charon `transform_dyn_trait_calls` + mrustc
  `MIR_Cleanup_Virtualize` peer precedents. Avoids coupling
  stage-12 Item synthesis with bridge body construction.
  [LOCKED]
- **D-11-7.4** Multi-statement rewrite procedure follows
  stage-9 expectations §23.3 (5 fresh locals incl. source-
  normalization for Ref receivers; explicit Statement-level
  extraction sites). Stage-11 spec references the procedure
  rather than duplicating it. [LOCKED]
- **D-11-7.5** Pass scheduling: vtable-rewrite precedes
  joint validation walk (D-11-6); §23.6.2 B-3/B-4/B-6
  invariants assume post-rewrite shape. [LOCKED]
- **D-11-7.6** Cross-stage friction with stage-5 D-5-1:
  spec records the forcing relationship (no `PtrMetadata`
  PlaceElem variant ⇒ multi-statement shape is the only
  legal expression). Reopening D-5-1 is out of stage-11
  scope. [LOCKED]
- **D-11-7.7** Stage-12 dependency notice: vtable-rewrite
  consumes Items synthesized by stage 12 (vtable type Items
  per `(Trait, projection_bindings)`; vtable instance Items
  per `(Trait, SelfTy)`; vtable shim Function Items). Stage
  12 owns synthesis; stage 11 confirms consumption. Phase-1
  implementation sequencing requires stage-12 synthesis to
  land before stage-11 rewrite executes. [LOCKED]

---

### D-11-8 -- LocalDecls allocation API [LOCKED]

**Question.** What is the API for allocating fresh
LocalDecls during bridge construction (Drop-as-Call) and
post-construction passes (vtable-rewrite)?

**Options considered.**
- (a) Single method on Body: `Body::push_local(decl) -> LocalId`.
  Both consumers use the same method.
- (b) Builder pattern: `BodyBuilder::push_local` for
  construction; direct `body.local_decls.push` for passes.
- (c) Split APIs (mrustc-style): `BodyBuilder::push_local`
  + `BodyMutator::push_local` with pending-statements
  buffer.

**Decision.** (a) single method on Body + convenience
helper for synthetic locals. Per spec-snapshot.md §25.8.

**Reasoning.**
- (c) split with buffering is not needed. mrustc's
  `MirMutator` buffer exists because their rewrite walks
  DURING basic-block iteration (mid-iteration mutation
  safety). RegseqModel's rewrite walks `basic_blocks_iter()`;
  for each block, builds the new statements + new Call
  atomically, then replaces `block.statements` (extending)
  + `block.terminator` as a unit. Single-block-rewrite-as-
  unit pattern. No buffer needed. Drop-as-Call at bridge
  time is similarly atomic per rustc terminator.
- (b) Builder is unnecessary intermediate. Post-construction
  passes need to mutate Body directly (insert statements,
  replace terminator), so a Builder→Body pipeline doesn't
  simplify.
- (a) is the minimum sufficient API. Same method, two
  consumers (bridge + passes). Charon's layered design is
  overkill for our locked LocalDecl shape (4 fields per
  D-11-2; no rich helpers needed).
- Convenience helper `push_synthetic_local(ty)` defaults
  `(Mutability::Mut, Span::dummy(), debug_name: None)`
  for bridge-introduced + rewrite-pass-introduced locals.

**Methodology rules invoked.**
- *Compositional behavior beats special-case flags* -- one
  API method composed with the convenience helper for
  synthetic locals; no parallel "is_synthetic" allocation
  channel.
- *No defensive fallbacks* -- mrustc's buffering pattern
  exists for a problem we don't have; we don't add it
  defensively.
- *C-semantics with information preservation* -- helper
  preserves the synthetic-local profile uniformly.

**Prior-art evidence.**
- rustc: pub-field-push discipline (no encapsulating
  method); `LocalDecl::new(ty, span)` defaults to Mut +
  `LocalInfo::Boring` + outermost-span; direct
  `body.local_decls.push(...)`. See `prior-art/rustc.md`
  D-11-8 section (the rustc.md prior-art file is organized
  by D-tag; the LocalDecls allocation discussion follows
  the §6 visitor section at line 332+).
- rustc_public: `Body::new` consumes complete `Vec`
  collections; **no incremental push API**; `pub(super)`
  privacy enforces entry via `new`. Construct-once shape;
  doesn't accommodate post-construction passes. See
  `prior-art/rustc_public.md`.
- Charon: layered — leaf `Locals::new_var(name, ty) ->
  Place` + higher-level `BodyTransformCtx::fresh_var` /
  `borrow_to_new_var` / `raw_borrow_to_new_var` trait
  helpers. Auto-`StorageLive`-on-fresh-var (does NOT
  transfer per D-8-1). See `prior-art/charon.md`.
- mrustc: split APIs — `MirBuilder::new_temporary`
  (`mir_builder.cpp:232`) + `MirMutator::new_temporary`
  (`cleanup.cpp:25-100`) with pending-statements buffering.
  See `prior-art/mrustc.md`.

**Worked example.**
```rust
// Bridge Drop-as-Call collapse
let _t_drop_arg = body.push_synthetic_local(ptr_to_place_ty);
let _t_drop_ret = body.push_synthetic_local(unit_ty);
// ... emit Statement + synthesized Call

// Vtable-rewrite pass
let _t_recv = body.push_synthetic_local(args[0].ty(...));
let _t_meta = body.push_synthetic_local(ty_ptr_to_vtable);
let _t_fn   = body.push_synthetic_local(fn_ptr_ty);
let _t_data = body.push_synthetic_local(ty_ptr_to_unit);
// ... emit projection-chain Statements + rewrite Call
```

**Spec cross-reference.** §25.8.

**Sub-decisions.**
- **D-11-8.1 [master]** Single allocation API on Body (alt
  a). Both bridge and post-construction passes consume the
  same method. [LOCKED]
- **D-11-8.2** Method shape: `Body::push_local(&mut self,
  decl: LocalDecl) -> LocalId`. Returns the freshly-
  allocated LocalId (positional). [LOCKED]
- **D-11-8.3** Convenience helper: `Body::push_synthetic_local(&mut self,
  ty: TyId) -> LocalId`. Defaults `debug_name = None`,
  `mutability = Mut`, `span = Span::dummy()`. For bridge-
  introduced + rewrite-pass-introduced locals. [LOCKED]
- **D-11-8.4** No BodyBuilder pattern; Body is mutable
  directly during both bridge construction and post-
  construction passes. Composes with D-11-7 hybrid timing.
  [LOCKED]
- **D-11-8.5** No buffering pattern; single-block-rewrite-
  as-unit traversal makes mid-iteration mutation a non-
  issue. Charon's auto-`StorageLive`-on-fresh-var pattern
  explicitly does NOT transfer per D-8-1. [LOCKED]
- **D-11-8.6** Consumer enumeration: bridge per-rustc-local
  population (`push_local` with full LocalDecl); bridge
  Drop-as-Call collapse (2× `push_synthetic_local` per Drop
  site); vtable-rewrite pass (4-5× `push_synthetic_local`
  per virtual-call site). [LOCKED]

---

### D-11-K -- Categorical rendering deferral to stage 13 [LOCKED]

**Question.** Where does rendering of body-level constructs
land — at stage 11 (per the per-stage K-tag pattern from
D-5-K through D-10-K), or at stage 13 (categorically, as a
coherent rendering-decision surface)?

**Options considered.**
- (a) Per-renderer retargeting at stage 11: each predecessor
  stage's K-tag (D-5-K through D-10-K + stage-1 _N sigil +
  stage-10 bbN sigil) lands at stage 11 individually.
- (b) Categorical retargeting to stage 13: single master
  decision moves all body-level rendering ownership to stage
  13 alongside `render_ty` (D14 deferred from stage 3).

**Decision.** (b) categorical retargeting to stage 13. Per
spec-snapshot.md §18.2 + the §19.8 / §20.6 / §21.8 / §22.8
/ §23.9 / §24.9 in-place updates.

**Reasoning.**
- Stage 13 is the project's pretty-printing-decisions stage.
  Renderer style, sigil conventions, and per-method ownership
  form a coherent surface that benefits from a single stage
  owning them.
- Stage-11's deferrals (inherited from D-5-K through D-10-K
  + stage-1 _N + stage-10 bbN) were originally targeted there
  because Body materialization is a renderer dependency —
  but stage 13 also depends on Body (via signature rendering,
  body-content rendering subtree) plus type-system rendering
  (`render_ty` from D14). All renderers landing together at
  stage 13 produces a coherent rendering decision; stage 11
  doesn't have to commit to a rendering style for body-content
  while stage 13 separately commits to a rendering style for
  types — the styles are decided as a unit.
- Stage 11 retains all structural spec ownership (Body /
  LocalDecl / FnSig location / LocalId definition / walks /
  passes). Only RENDERING moves; structural ownership is
  unchanged.
- Single master decision over per-renderer retargeting
  produces a cleaner audit-shape entry + cleaner merger-
  contract bookkeeping.

**Methodology rules invoked.**
- *Spec is mutable* -- categorical retargeting is the kind
  of cross-stage scope decision the methodology accommodates.
- *Compositional behavior beats special-case flags* -- single
  master decision composes; per-renderer retargeting would
  duplicate the rationale 8 times.
- *Compiler-theory-first* -- coherent rendering-decision
  surface is sound design; rendering style affects
  readability uniformly across IR constructs.

**Prior-art evidence.**
- rustc: scattered Debug/Display impls per type with
  `#[debug_format = "_{}"]` / `#[debug_format = "bb{}"]`
  attributes on newtype indexes; the macro-emitted Debug
  impl is the natural rendering spot. See
  `prior-art/rustc.md:537` §9.
- rustc_public: in-crate pretty-printer (`pretty.rs`); no
  deferral pattern. See `prior-art/rustc_public.md:681`
  §8.
- Charon: co-locates renderer with IR (`fmt_with_ctx.rs`).
  See `prior-art/charon.md:719` §9.
  See `prior-art/charon.md`.
- mrustc: interleaves rendering with construction (opposite
  philosophy). See `prior-art/mrustc.md`.

(Note: rendering-decision-stage as a distinct project stage
is RegseqModel-specific; no peer has the same staging
discipline.)

**Spec cross-reference.** §18.2 (per-method ownership table
reframed); §19.8 / §20.6 / §21.8 / §22.8 / §23.9 / §24.9
(per-stage renderer deferral notes flipped to stage 13 with
attribution to predecessor D-X-K).

**Sub-decisions.**
- **D-11-K.1 [master]** Categorical rendering deferral: all
  body-level renderers + `_N` and `bbN` sigil rendering
  wirings move to stage 13. Single decision, not per-renderer
  retargeting. [LOCKED]
- **D-11-K.2** Per-renderer enumeration (the items moving):
  9 public renderers (`render_place` originally D-5-K,
  `render_operand` D-6-K, `render_rvalue` D-7-K,
  `render_statement` D-8-K, `render_terminator` +
  `render_call_callee` + `render_switch_targets` +
  `render_assert_kind` D-9-K, `render_basic_block` D-10-K),
  plus 1 internal helper (`render_terminator_kind`), plus
  2 sigil rendering wirings (`_N` for `LocalId` from
  stage-1 inbound, `bbN` for `BlockId` from D-10-K.2) =
  **12 items total**. Stage-11 spec attribution to
  predecessor stages preserved. [LOCKED]
- **D-11-K.3** Operational consequence at stage-11 merger
  contract close (per README §5.5 step 4): D-5-K, D-6-K,
  D-7-K, D-8-K, D-9-K, D-10-K all flip to `[REOPENED]` in
  top-level `decisions.md` with annotation `"(retargeted to
  stage 13 per D-11-K)"`. Cross-stage bookkeeping: rendering
  items move out of `stage-11/expectations.md` into a new
  `stage-13/expectations.md` "From stage 11 (per D-11-K)"
  bundle preserving attribution. [LOCKED]
- **D-11-K.4** §18.2 per-method ownership table treatment:
  body-level renderer rows are RETARGETED in place from
  "stage 11 (deferred)" to "stage 13 (per D-11-K)" at the
  stage-11 merger contract. Rows remain in §18.2 with the
  new ownership; stage 13 will refine them further (e.g.,
  expanded per-renderer detail) when it lands. Stage-11
  spec-snapshot.md §18.2 has been pre-emptively updated
  to reflect the retargeted state with attribution to
  predecessor D-X-K decisions inheriting their dependency
  analysis. [LOCKED]
- **D-11-K.5** Stage-11 retains structural spec ownership:
  validation walks (D-11-6), transformation passes (D-11-7),
  construction-invariant fold-ins (D-11-6.1), data structures
  (D-11-1 through D-11-5 + D-11-8) all stay at stage 11. This
  is a `RENDERING-only` retargeting; structural ownership
  unchanged. [LOCKED]
- **D-11-K.6** Cross-references: stage-11 spec-snapshot.md
  notes the categorical deferral at every site that would
  have referenced a body-level renderer (joint visitor's
  `visit_X` hooks don't render — rendering is stage-13's
  `render_X`). Stage-13 expectations.md inbound bundle (filed
  at Phase B close) preserves attribution to predecessor
  stages so stage 13 understands the renderer dependency
  graph (Cast arm late-binds to render_ty; Aggregate
  field/variant names resolve via §11 layout vocabulary;
  etc.) inherited from each originating stage's audit shape.
  [LOCKED]

---

## 2. Phase A discussion threads (substantive)

### Stage-1 inbound reframe (D-11-3)

The user's hands-on engagement with D-11-3 ratified the
4-peer-evidence-driven reframe. Stage-1 inbound's framing
"FnSig as a structural field of Body" was a default reading
(stage-1 didn't pin location explicitly; only schema +
abi-as-structural). The 4-peer evidence (rustc, rustc_public,
Charon, mrustc) converged on Item-side placement. Stage-11
locks alt (e) per "Spec is mutable" methodology; stage-1's
schema framing carries through.

Recorded in spec-snapshot.md §14 (placement note) + §25.4
(reframe documentation).

### D-9-3 reopening question (D-11-7)

Mid-Phase-A, the question was raised whether stage-9 D-9-3
should be reopened to alt (a) (keep `Direct(InstanceId)` for
virtual; codegen-side dispatch on `flavor: Virtual` via
existing `virtual_idx` payload) to remove the stage-11
vtable-rewrite pass.

The cost-benefit analysis covered:
- Reopening cost: stage-9 Phase A re-do, merger contract
  re-open, ~10-15 stage-9 findings re-confirm
- Stage-11 rewrite cost: 4-5 fresh locals per virtual-call
  site, multi-statement projection chain, pass-scheduling
  complexity
- Cross-stage friction with stage-5 D-5-1 forces multi-
  statement shape regardless of which stage owns the rewrite
- Stage-9's IR-uniformity preference (one Call code path)
  is reasonable

User decision: **don't reopen, proceed with hybrid (c)**.
Recorded in this discussions.md D-11-7 entry's reasoning
section.

### Categorical D-11-K reframe scope (cross-cutting)

The user introduced the categorical D-11-K decision at
stage-11 phase 0 ("I want to defer those to stage 13 since
stage 13 is particularly responsible for deciding on how
to pretty print or render IR constructs"). This pre-decision
shaped stage-11's entire scope (rendering moves; structural
spec stays). D-11-K formalizes the user's pre-decision as
a master decision with full audit shape.

The cross-stage operational bookkeeping (move rendering
items out of stage-11/expectations.md into a new
stage-13/expectations.md "From stage 11 (per D-11-K)"
bundle) is performed at stage-11 merger contract close per
the §5.5 7-step protocol; tracked as task #5 in the project's
task list. The §18.2 per-method ownership table updates have
been pre-emptively applied to stage-11/spec-snapshot.md so
the categorical retargeting is visible during Phase B Codex
review.

---

## 3. Cross-references

- Top-level `../README.md` -- methodology rules referenced
  in each entry's "Methodology rules invoked" section.
- Top-level `../status.md` -- stage progression.
- `memory.md` -- stage-11 invariants, Codex prompt template
  in §4.
- `status.md` -- D-tag allocation table (mirrors this file's
  status flags); Phase A close inventory; Phase B work plan
  in §5.
- `expectations.md` -- inbound forward refs from stages 1, 3,
  5, 6, 7, 8, 9, 10. Each entry's "Spec cross-reference"
  section indicates which inbound items the D-tag closes.
- `spec-snapshot.md` §25 -- new Body-level vocabulary
  section (containing §25.1 through §25.11).
- `spec-snapshot.md` §14 -- in-place clarification on FnSig
  placement (Item-side per D-11-3).
- `spec-snapshot.md` §18.2 -- in-place reframing of
  per-method ownership table per D-11-K's categorical
  retargeting.
- `spec-snapshot.md` §19.8 / §20.6 / §21.8 / §22.8 / §23.9 /
  §24.9 -- in-place per-stage renderer-deferral note
  flips to stage 13 per D-11-K.
- `prior-art/{rustc,rustc_public,charon,mrustc}.md` --
  primary-source evidence cited per entry.
- `review-log.md` -- Phase B per-round Codex audit (created
  at first review round).

---

## 4. Outbound expectations to file at merger contract

To file in `stage-12/expectations.md` "From stage 11"
(per D-11-3 + D-11-5):
- FnSig migration to `Item.kind = Function { fn_sig, ... }`
  per D-11-3. The canonical FnSig schema is the pinned
  5-field form per §14.3 (`inputs`, `output`, `abi`,
  `c_variadic`, `safety`); D-11-3 reframes the placement
  (Item-side instead of Body-side) without revising
  §14's schema. Stage 12 absorbs the §14 schema as a
  structural field of `Item.kind = Function`.
- `Body` materialisation under Item per D-11-5 (schema
  placement: top-level `Item.body: Option<Body>` vs
  variant-internal `ItemKind::Function.body`; stage-12
  picks).
- Construction invariant 9 closure (D-9-10 B-7); stage-11
  visitor checks `Direct(InstanceId)` membership in
  `model.instances`; stage-12 owns `model.instances` integrity.

To file in `stage-13/expectations.md` "From stage 11 (per
D-11-K)":
- 12-item rendering bundle (9 public renderers + 1 internal
  helper + 2 sigil wirings per D-11-K.2) with attribution
  to predecessor
  stages (D-5-K through D-10-K originating; D-11-K
  retargets).
- §18.2 per-method ownership table consolidation: stage 13
  owns all rendering ownership rows including existing
  `render_ty` (D14) plus the 9 public body-content
  renderer rows + 1 internal helper (per D-11-K.2's
  12-item enumeration) + 2 sigil wirings.

Filing happens at stage-11 merger contract step 7 (per
README §5.5 7-step protocol), after per-stage Codex
review APPROVES.
