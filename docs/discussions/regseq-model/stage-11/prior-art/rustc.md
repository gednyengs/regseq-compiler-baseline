# Stage 11 -- Prior Art: rustc primary source

Primary-source rustc evidence for the **MIR body container vocabulary** -- `Body<'tcx>`, `LocalDecl<'tcx>`, the `Local` newtype-index, the body-level walk pattern in two backends, the LocalDecls allocation discipline, and rustc's chosen response to dyn-Trait dispatch (codegen-time dispatch via `InstanceKind::Virtual`, *not* a MIR-rewrite pass). Scoped to stage 11's candidate decision namespace:

- `D-11-1` Body field set (which of rustc's ~17 fields belongs in RegseqModel).
- `D-11-2` LocalDecl field set (4-field candidate vs. rustc's 5-field `LocalDecl` + 9-variant `LocalInfo`).
- `D-11-3` FnSig shape (full set, not just abi; pressure-test for alt (d) "drop body-level fn_sig, reach signature data through `local_decls[0..=arg_count]`" against rustc's `Body::return_ty()` precedent).
- `D-11-4` LocalId typed-index (rustc's `newtype_index!` Local + `const RETURN_PLACE = 0`; const-body grammar in the proc-macro).
- `D-11-5` BodyId vs ItemId resolution (rustc has no BodyId; `tcx.instance_mir(InstanceKind)` is the only fetch path, with `InstanceKind` itself acting as the key).
- `D-11-6` Joint validation walk shape and ownership (the codegen-SSA vs. cranelift two-walk split; the `make_mir_visitor!` template with `visit_X` / `super_X` discipline; the `non_ssa_locals` pre-walk pattern).
  - `D-11-6.1` Construction invariant 1 + 9 integration into the visitor.
- `D-11-7` Body-level transformation pass timing (rustc's deliberate **non-rewrite** for dyn-Trait dispatch; `MirPass::run_pass(tcx, body: &mut Body)` template for transformation passes that do exist).
- `D-11-8` LocalDecls allocation API (rustc's pub-field mutability + `LocalDecl::new` constructor + no public push API).
- `D-11-K` Categorical rendering deferral to stage 13.

Out of scope: rustc_public's collapsed Body schema (separate lens, in `./rustc_public.md`); peer rewrite-style dyn-Trait dispatch (Charon/mrustc; in `./charon.md` and `./mrustc.md`); the stage-10-closed `BasicBlock` per-block container (covered in `../../stage-10/prior-art/rustc.md`); the stage-9-closed `Terminator`, stage-8-closed `Statement`, stage-7-closed `Rvalue`, stage-6-closed `Operand`, stage-5-closed `Place` (covered in earlier-stage `prior-art/rustc.md` files).

All paths below are relative to the pinned-toolchain root:

```
~/.rustup/toolchains/nightly-2026-02-01-aarch64-apple-darwin/lib/rustlib/rustc-src/rust/compiler/
```

(See `../../rustc-references.md` for the canonical statement of this root path.)

Every factual claim cites `[file:line]`. Hypotheses are flagged HYPOTHESIS:; cross-site syntheses are flagged INFERENCE:. The Regseq compiler consumes `optimized_mir` outputs, so "what reaches the bridge" means: the body returned by `optimized_mir` after `run_analysis_to_runtime_passes` and `run_optimization_passes` have completed -- modulo the cleanup-pass whitelist enabled at `crates/regseq-driver/src/callbacks.rs:25-66`.

The prior-art lens here is **peer-compiler design rationale**, not field enumeration. The kickoff evidence pass already enumerated rustc's Body / LocalDecl / FnSig / VarDebugInfo fields. This file asks the rationale question: *why does rustc make these choices, what tradeoffs, and what would change under RegseqModel's different requirements (post-mono, no debuginfo, no borrowck, no coroutines, C-shape codegen)?*

---

## 1. The `Body<'tcx>` body container (D-11-1)

`Body<'tcx>` is the canonical body container, defined at `[rustc_middle/src/mir/mod.rs:208-335]`. The full set is 17 fields, of which the doc-comments and surrounding code reveal the rationale for inclusion:

```rust
pub struct Body<'tcx> {
    pub basic_blocks: BasicBlocks<'tcx>,                              // [213]
    pub phase: MirPhase,                                              // [220]
    pub pass_count: usize,                                            // [223]
    pub source: MirSource<'tcx>,                                      // [225]
    pub source_scopes: IndexVec<SourceScope, SourceScopeData<'tcx>>,  // [229]
    pub coroutine: Option<Box<CoroutineInfo<'tcx>>>,                  // [237]
    pub local_decls: IndexVec<Local, LocalDecl<'tcx>>,                // [244]
    pub user_type_annotations: ty::CanonicalUserTypeAnnotations<'tcx>,// [247]
    pub arg_count: usize,                                             // [255]
    pub spread_arg: Option<Local>,                                    // [261]
    pub var_debug_info: Vec<VarDebugInfo<'tcx>>,                      // [264]
    pub span: Span,                                                   // [267]
    pub required_consts: Option<Vec<ConstOperand<'tcx>>>,             // [275]
    pub mentioned_items: Option<Vec<Spanned<MentionedItem<'tcx>>>>,   // [289]
    pub is_polymorphic: bool,                                         // [306]
    pub injection_phase: Option<MirPhase>,                            // [313]
    pub tainted_by_errors: Option<ErrorGuaranteed>,                   // [315]
    pub coverage_info_hi: Option<Box<coverage::CoverageInfoHi>>,      // [324]
    pub function_coverage_info: Option<Box<coverage::FunctionCoverageInfo>>, // [334]
}
```

### 1.1 Why these fields exist in rustc

The comments are explicit about what each field is *for*:

- `phase` + `pass_count` exist for inlining purposes [`mod.rs:215-223`]: when an already-optimized MIR body is inlined into a not-yet-optimized one, the inliner must know whether to re-optimize the inlined fragment or skip those passes.
- `source: MirSource` carries the `def_id` plus an `InstanceKind`-derived identity (used in promoted bodies, shims). The body knows which item produced it.
- `source_scopes` is THE debuginfo carrier. Combined with `LocalDecl.source_info.scope` (the *syntactic/lint* scope) and `var_debug_info[i].source_info.scope` (the *debugger/visibility* scope), rustc builds a per-local two-scope tree (rationale spelled out at length in [`mod.rs:967-1052`]).
- `coroutine` is non-`None` only for actual coroutine bodies [`mod.rs:231-237`]. Drop shims for coroutines explicitly set this to `None` to avoid carrying redundant coroutine state.
- `user_type_annotations` is a borrowck artifact -- the user's `let x: T` annotation, used by the MIR borrow checker for region inference [`mod.rs:967-971`].
- `required_consts` is **soundness-critical** [`mod.rs:269-275`]: holds constants that have not yet been evaluated; the collector must verify they evaluate. The `Option` wrapper says "not yet computed."
- `mentioned_items` is **NOT soundness-critical** [`mod.rs:277-289`]: it's the "may become monomorphized" list that the collector traverses, used to avoid optimization-level-dependent compile errors.
- `is_polymorphic` is a permanently-frozen flag derived from `has_non_region_param()` at construction [`mod.rs:303-306`]: the doc-comment explicitly says **"WARNING**: Do not change this flag after the MIR was originally created, even if an optimization removed the last mention of all generic params."
- `injection_phase` is `Some` only if the function had `#[rustc_custom_mir]` [`mod.rs:308-313`].
- `tainted_by_errors` propagates upstream errors; `coverage_info_hi` and `function_coverage_info` are gated on `-Cinstrument-coverage`.
- `spread_arg` is the rust-call-ABI tuple-spread marker [`mod.rs:257-261`] (only for closures called as `Fn`-trait methods).

### 1.2 What shrinks under RegseqModel's requirements

Mapping each field against RegseqModel's binding constraints (post-mono per Q2 eager translation; no DWARF emission per stage-1; C-semantics; coroutine type-rejection per stage-3 §15.1):

- `phase` / `pass_count`: rustc-pipeline bookkeeping. Bridge consumes `optimized_mir`; phase is a fixed point. **OUT.**
- `source: MirSource`: needed only as a Body↔Item linkage; subsumed by D-11-5.
- `source_scopes`: the *lint* scope tree. RegseqModel emits no DWARF; lint emission happens upstream of the bridge. **OUT.**
- `coroutine`: stage-3 §15.1 type-rejects coroutines; this field can never be `Some` at the bridge boundary. **OUT.**
- `local_decls`: load-bearing. **IN.** (See §2.)
- `user_type_annotations`: borrowck-only; runs before MIR construction. **OUT.**
- `arg_count`: load-bearing partition marker (`[0]=ret, [1..arg_count+1]=args, [..]=temps`). **IN.**
- `spread_arg`: only relevant under rust-call ABI. Stage-3 D15 locks `FnAbi = {Rust, C}`. INFERENCE: under D15, `spread_arg` is unreachable; D-11-1 alt (a) drops it. HYPOTHESIS: if a future stage admits `Abi::RustCall`, this field re-enters; not now.
- `var_debug_info`: provides the named-local mapping required by stage-8 D-8-3. **PARTIALLY IN** -- consumed at construction time to populate `LocalDecl.debug_name`, body-level vector itself NOT carried; rustc itself partitions this per-Local at codegen time at [`rustc_codegen_ssa/src/mir/mod.rs:46-47, 119`] (`PerLocalVarDebugInfoIndexVec<'tcx, V>` = `IndexVec<Local, Vec<PerLocalVarDebugInfo<...>>>`), confirming the lift is the natural shape.
- `span`: whole-body diagnostic location. **IN.**
- `required_consts` / `mentioned_items`: collector-discharged before the bridge. **OUT.**
- `is_polymorphic`: post-mono is by definition fully concrete; this field is `false` invariably. **OUT.**
- `injection_phase`: `rustc_custom_mir` is rustc-internal MIR-builder testing infrastructure. **OUT.**
- `tainted_by_errors`: error gating; the bridge rejects upstream. **OUT.**
- `coverage_info_hi` / `function_coverage_info`: `-Cinstrument-coverage` only. **OUT.**

### 1.3 RegseqModel implications (D-11-1)

The peer-compiler design rationale supports the kickoff's D-11-1 alt (a) (six fields: `basic_blocks, local_decls, arg_count, fn_sig, span, source` plus the Body↔Item linkage subsumed by D-11-5). The 17→6 collapse follows from removing each field whose existence is gated on a rustc-only requirement (debuginfo, borrowck, coverage, coroutines, polymorphism, MIR-pipeline staging).

The *order* of these fields in rustc is informative: `basic_blocks` first, then pipeline metadata, then `local_decls`, then `var_debug_info` -- rustc's groupings reflect a "state machine + body content + debug overlays" mental model. RegseqModel's 6-field minimum is structural: container + locals + signature + diagnostic locator. HYPOTHESIS: ordering `basic_blocks` last (after `local_decls` and `fn_sig`) reads more naturally because basic-block content references `LocalId`s and signature info that should be "in scope" at read time. Decision deferred; this is style, not semantics.

---

## 2. `LocalDecl<'tcx>` and the `LocalInfo` enum (D-11-2)

`LocalDecl<'tcx>` at `[rustc_middle/src/mir/mod.rs:951-1054]`:

```rust
pub struct LocalDecl<'tcx> {
    pub mutability: Mutability,                                // [960]
    pub local_info: ClearCrossCrate<Box<LocalInfo<'tcx>>>,     // [962]
    pub ty: Ty<'tcx>,                                          // [965]
    pub user_ty: Option<Box<UserTypeProjections>>,             // [971]
    pub source_info: SourceInfo,                               // [1053]  // SourceInfo = { span, scope }
}
```

Five fields, but the *real* surface is 4 plus a 9-variant `LocalInfo` enum at [`mod.rs:1063-1093`]:

```rust
pub enum LocalInfo<'tcx> {
    User(BindingForm<'tcx>),                                   // [1069]
    StaticRef { def_id: DefId, is_thread_local: bool },        // [1071]
    ConstRef { def_id: DefId },                                // [1073]
    AggregateTemp,                                             // [1076]
    BlockTailTemp(BlockTailInfo),                              // [1079]
    IfThenRescopeTemp { if_then: HirId },                      // [1082]
    DerefTemp,                                                 // [1088]
    FakeBorrow,                                                // [1090]
    Boring,                                                    // [1092]
}
```

### 2.1 Why `LocalInfo` exists

Two motivations, both NON-applicable to RegseqModel:

(a) **Borrowck + diagnostics classification.** The doc-comment at [`mod.rs:1056-1062`] says "for diagnostics and for classifying variables into local variables, statics, etc, which is needed e.g. for borrow checking." Methods on `LocalDecl` like `is_user_variable` [`mod.rs:1132`], `is_ref_to_static` [`mod.rs:1145`], `is_deref_temp` [`mod.rs:1159`] all dispatch on `LocalInfo` for diagnostic-error message tailoring or borrowck rule selection. Mutable-binding diagnostics: `can_be_made_mutable` [`mod.rs:1106-1114`] inspects the `BindingForm::Var(VarBindingForm { binding_mode, .. })` payload to decide if a "make this mut" suggestion is appropriate.

(b) **Per-pass lifecycle.** `DerefTemp` [`mod.rs:1083-1088`] is created during the `Derefer` pass and replaced with `Boring` in runtime MIR. `FakeBorrow` [`mod.rs:1090`] exists for borrowck instrumentation only. Both phases are well before `optimized_mir`'s post-runtime phase, so by the time the bridge reads, only `User`, `StaticRef`, `ConstRef`, `AggregateTemp`, `BlockTailTemp`, `IfThenRescopeTemp`, `Boring` plausibly appear -- and even those are diagnostic-only.

### 2.2 Why `ClearCrossCrate<Box<LocalInfo>>`

`ClearCrossCrate` is a wrapper that becomes `Set(_)` in the originating crate but is cleared to `Clear` when the body is encoded for cross-crate use. The rationale from `LocalDecl::local_info` [`mod.rs:1095-1098`]: `unwrap_crate_local()` panics if accessed across the crate boundary -- diagnostic info doesn't need to survive metadata serialization.

### 2.3 Why `source_info: SourceInfo = { span, scope }`

The 86-line doc comment at [`mod.rs:967-1052`] is the most explicit rationale-laden block in the entire `Body` schema. The argument: a single `Span` is insufficient because lint scoping (where `#[allow(unused_mut)]` applies) and debugger scoping (where the user can refer to `x` by name) follow different rules. So rustc carries *two* scope trees: `LocalDecl.source_info.scope` is the lint scope; `VarDebugInfo.source_info.scope` is the debugger scope. The lint scope follows AST/HIR nesting; the debugger scope follows let-statement-suffix nesting (each let-statement nests its successors).

### 2.4 What shrinks for RegseqModel

- `mutability`: load-bearing for the post-runtime mutability classification (rustc validates "return place must be mutable" at `[mod.rs:434-437]`). **IN.**
- `local_info`: 9-variant diagnostic/borrowck classifier. RegseqModel does no borrowck (upstream-discharged) and emits no diagnostics tailored to binding form. **OUT** -- but its named-local payload (`User(BindingForm)` carries the user's name? no, `name` lives in `VarDebugInfo`, not `BindingForm`) is folded into `debug_name: Option<StringId>` -- a single optional name string, populated from `var_debug_info` per stage-8 D-8-3.
- `ty`: load-bearing for Place validation walk (D-5-2 rule 2 head-local lookup) and §3.3 Phase B validation generally. **IN**, as `TyId`.
- `user_ty`: borrowck-only [`mod.rs:967-971`]. **OUT.**
- `source_info`: the `span` half is kept; the `scope` half is RegseqModel-irrelevant (no DWARF emission, no two-scope-tree need).

**RegseqModel candidate: 4 fields** -- `{ ty: TyId, mutability, span, debug_name: Option<StringId> }`. Confirms kickoff D-11-2 alt (a).

### 2.5 RegseqModel implications (D-11-2)

Two design choices in rustc *do not* propagate:

1. **No `LocalInfo` enum equivalent.** Synthetic locals (Drop-as-Call's `_t_drop_arg` / `_t_drop_ret`; vtable-rewrite's `_t_recv` / `_t_meta` / `_t_fn` / `_t_data` / `_t_raw`) are distinguished from user-named locals by `debug_name = None`, not by a tag enum -- compositional with the README §2 "compositional behavior beats special-case flags" rule. Confirms kickoff D-11-2 hypothesis (a).
2. **No `LocalKind` enum field on LocalDecl.** Rustc's `LocalKind { Temp, Arg, ReturnPointer }` at [`mod.rs:858-867`] is a *helper*, computed from positional convention in `Body::local_kind` [`mod.rs:431-445`], not a stored field. RegseqModel inherits the positional convention from stage 1, so the helper computes the same way. Confirms kickoff D-11-2 (b) is unnecessary.

Two derived helpers transfer naturally: `args_iter` [`mod.rs:475-477`] = `(1..arg_count + 1).map(LocalId::new)`, and `vars_and_temps_iter` [`mod.rs:482-486`] = `(arg_count + 1..local_decls.len()).map(LocalId::new)`. INFERENCE: stage 11 likely lands these as `Body` methods, mirroring rustc's surface 1:1.

Notable absence: rustc has no `LocalDecl::debug_name` field. The named-local mapping lives at body level in `var_debug_info`. RegseqModel's lift -- folding it into per-LocalDecl -- is **structurally identical to what rustc itself does at codegen consumption time**: `FunctionCx.per_local_var_debug_info: Option<PerLocalVarDebugInfoIndexVec<'tcx, V>>` at [`rustc_codegen_ssa/src/mir/mod.rs:117-119`] *is* the per-Local partition of `var_debug_info`. INFERENCE: rustc stores a body-level `Vec` because the data has multiple consumers (codegen + DWARF emission + lint emission); RegseqModel has one consumer (renderers + C-emission), so storing per-LocalDecl is appropriate.

---

## 3. `FnSig` and the question of body-level signature caching (D-11-3)

Rustc's body container does **not** carry a `FnSig`. Argument types are reached via `local_decls[1..arg_count+1]`; return type via `Body::return_ty()` at [`rustc_middle/src/mir/mod.rs:506-510`]:

```rust
pub fn return_ty(&self) -> Ty<'tcx> {
    self.local_decls[RETURN_PLACE].ty
}
```

When rustc needs the *full* signature (not just typed locals), it queries `tcx.fn_sig(def_id)` -- a `TyCtxt`-level query that returns `EarlyBinder<PolyFnSig<'tcx>>`. The `PolyFnSig` itself is `rustc_type_ir::FnSig<I>` at [`rustc_type_ir/src/ty_kind.rs:728-744`]:

```rust
pub struct FnSig<I: Interner> {
    pub inputs_and_output: I::Tys,    // single contiguous list, output last
    pub c_variadic: bool,
    pub safety: I::Safety,
    pub abi: I::Abi,
}
```

### 3.1 Why no body-level `fn_sig` cache in rustc

Two reasons emerge from the design:

(a) **Redundancy avoidance.** Argument and return types live in `local_decls`; the abi / variadic / safety bits are tcx-cached by `def_id`, retrievable in `O(1)`. Caching them again in `Body` would mean two sources of truth and a synchronization burden under MIR transforms (especially `instantiate_mir_and_normalize_erasing_regions` at [`rustc_codegen_ssa/src/mir/mod.rs:131-135`]).

(b) **`Body` is `Clone`-cheap by design.** Body is cloned during inlining at [`rustc_codegen_ssa/src/mir/mod.rs:191-195`]; per-clone payload should be the actually-different parts (basic_blocks, local_decls). Static signature bits are def-keyed.

### 3.2 But RegseqModel has Q2 (eager translation)

Q2 forbids reaching back to rustc post-construction. So RegseqModel cannot defer the signature retrieval; if the bridge produces the body, it must produce the signature alongside.

That puts the question on stage 11: *where* does the signature go?

- D-11-3 alt (a)/(b)/(c): structural `fn_sig` field on Body (with or without `c_variadic` / `safety`). Mirrors stage-1's expectation; redundant with `local_decls` for input/output types.
- D-11-3 alt (d) (kickoff-surfaced): drop body-level `fn_sig`; reach types through `local_decls`; carry abi (and possibly `c_variadic`) as separate Body fields.

### 3.3 Pressure-test from rustc

Rustc's `Body::return_ty()` precedent **directly** validates D-11-3 alt (d) for the input/output portion: argument types and return type are reachable through `local_decls` in O(1) without redundant storage. The remaining pieces -- abi (always required for ABI dispatch in C codegen), `c_variadic` (relevant for C `printf`-style intrinsics), and `safety` (post-typeck; not C-relevant) -- can each be Body-level scalar fields.

INFERENCE: D-11-3 alt (d) is structurally cleaner than alt (a)/(b)/(c) -- it removes the fan-out of argument types (between `fn_sig.inputs[i]` and `local_decls[i+1].ty`) and the corresponding "are these the same?" invariant the validator would have to enforce. Confirms kickoff D-11-3 hypothesis lean -- with the refinement that alt (d) absorbs (b)'s `c_variadic` necessity by making `c_variadic` a Body-level field, not a sub-field of `fn_sig`.

### 3.4 RegseqModel implications (D-11-3)

The peer-compiler design supports the radical option (drop `fn_sig`). HYPOTHESIS: stage 11 will lock alt (d), with `Body { ..., abi: FnAbi, c_variadic: bool, ... }` and `Body::return_ty() -> TyId` / `Body::input_tys() -> &[TyId]` helpers. `safety` is OUT regardless (not C-relevant). The stage-1 expectation "FnSig is a structural field" is honored in spirit (signature data is structurally on Body), if not in literal form (no `fn_sig` field).

---

## 4. The `Local` newtype-index and the `newtype_index!` macro (D-11-4)

`Local` is defined at [`rustc_middle/src/mir/mod.rs:842-850`]:

```rust
rustc_index::newtype_index! {
    #[derive(HashStable)]
    #[encodable]
    #[orderable]
    #[debug_format = "_{}"]
    pub struct Local {
        const RETURN_PLACE = 0;
    }
}
```

Compare to `BasicBlock` at [`mod.rs:1262-1292`]:

```rust
rustc_index::newtype_index! {
    /// ...
    #[derive(HashStable)]
    #[encodable]
    #[orderable]
    #[debug_format = "bb{}"]
    pub struct BasicBlock {
        const START_BLOCK = 0;
    }
}
```

**Identical macro shape.** Five attributes plus the const-body grammar yielding one `pub const NAME: Type = Type::from_u32(VAL);` per declaration.

### 4.1 Where the const-body arm lives

The `newtype_index` is a proc-macro re-exported at [`rustc_index/src/lib.rs:16`]:

```rust
pub use rustc_index_macros::newtype_index;
```

The proc-macro source is at [`rustc_index_macros/src/newtype.rs`]. Key spots:

- Line 22: `let mut debug_format: Option<Lit> = None;` -- parses the `#[debug_format = "..."]` attribute.
- Line 24: `let mut consts = Vec::new();` -- accumulates `const NAME = INT;` declarations from the body.
- Line 87: `consts.push(quote! { #(#const_attrs)* #vis const #const_name: #name = #name::from_u32(#const_val); });` -- emits each const as `pub const NAME: Type = Type::from_u32(VAL);` using the macro-emitted constructor.
- Lines 162-175: emits `MAX_AS_U32` / `MAX` / `ZERO` plus `#(#consts)*` -- the user-supplied consts.
- Lines 180-246: emits `from_usize` / `from_u32` / `from_u16` / `index` / `as_u32` / `as_usize` -- standard typed-index machinery.

### 4.2 RegseqModel implications (D-11-4)

Stage-10's macro extension landed the const-body arm; the proc-macro shape is straightforward to mirror. RegseqModel's local `newtype_index!` lives in `crates/regseq-model/src/idx.rs`. Stage 11 reuses the const-body arm to declare:

```rust
newtype_index! {
    pub struct LocalId {
        const RETURN_PLACE = 0;
    }
}
```

This confirms kickoff D-11-4 alt (a) is the correct shape, structurally aligned with stage-10's `BlockId { const START_BLOCK = 0; }`.

The `#[debug_format = "_{}"]` and `#[debug_format = "bb{}"]` rendering hints in rustc's macro are now **out of stage-11 scope** per the rendering-deferred-to-stage-13 decision (D-11-K). Stage 11 lands the typed-index mechanics; stage 13 decides whether to use the macro's `debug_format` hint or a hand-written `render_local_id` helper.

---

## 5. BodyId vs ItemId resolution (D-11-5)

Rustc has **no `BodyId`**. Bodies are reached through `tcx.instance_mir(InstanceKind)` at [`rustc_middle/src/ty/mod.rs:1671-1703`]:

```rust
pub fn instance_mir(self, instance: ty::InstanceKind<'tcx>) -> &'tcx Body<'tcx> {
    match instance {
        ty::InstanceKind::Item(def) => {
            // dispatches to mir_for_ctfe(def) for const items, optimized_mir(def) otherwise
        }
        ty::InstanceKind::VTableShim(..) | ... | ty::InstanceKind::AsyncDropGlue(..) =>
            self.mir_shims(instance),
    }
}
```

The 12 `InstanceKind` variants [`mod.rs:1689-1702`] (`VTableShim`, `ReifyShim`, `Intrinsic`, `FnPtrShim`, `Virtual`, `ClosureOnceShim`, `ConstructCoroutineInClosureShim`, `FutureDropPollShim`, `DropGlue`, `CloneShim`, `ThreadLocalShim`, `FnPtrAddrShim`, `AsyncDropGlueCtorShim`, `AsyncDropGlue`) all dispatch to `mir_shims(instance)` -- that is, `InstanceKind` *itself* is the body-fetch key. There is no parallel ID space. The function symbol's `def_id` (for Item bodies) plus the variant tag (for shims) is the canonical body identity.

### 5.1 Why no BodyId

Two reasons surface:

(a) **InstanceKind already partitions the body space.** Adding a BodyId would either alias `(InstanceKind, args)` 1:1 (redundant) or partition it differently (creating two sources of truth).

(b) **Bodies are queried, not stored.** Rustc's body store is the `optimized_mir(def_id)` query plus the `mir_shims(InstanceKind)` query, both `tcx`-cached. `Body<'tcx>` instances are `&'tcx`-borrowed from the query cache; they are never owned outside the cache. Under Q2 (eager translation), RegseqModel must own bodies, but the no-BodyId rationale -- one fetch path keyed on instance identity -- still applies.

### 5.2 RegseqModel implications (D-11-5)

The peer evidence supports D-11-5 alt (c): bodies live in `Item.body: Option<Body>`. Composition:

- Q3 (single-level IR, no layered ladder) prefers fewer ID spaces.
- Stage 12's `Item` table is the natural anchor.
- `rustc_public::Instance::body() -> Option<Body>` (a stable-IR analog the kickoff cited) makes the same choice.

Items whose bodies are absent at the bridge boundary (e.g., `extern` declarations) carry `Item.body: None`. Items with synthesized bodies (the RegseqModel analog of rustc's `mir_shims` -- vtable shims, closure shims) get a body when stage 12's `SyntheticItemKind` machinery (per stage-4 D-4-3 + stage-9 D-9-2 / D-9-3) populates them.

HYPOTHESIS: this collapse produces a cleaner registry than rustc's Item-vs-shim split because RegseqModel's stage-12 `SyntheticItemKind` is already a unified Item-ish anchor for all synthesized-body cases. The "InstanceKind partitions body space" rationale carries through with `SyntheticItemKind` taking the role rustc gives `InstanceKind`.

Confirms kickoff D-11-5 hypothesis lean.

---

## 6. The body-level walk: codegen-SSA vs cranelift, and the `make_mir_visitor!` template (D-11-6)

Rustc has *two* body-walk patterns: a generic, multi-backend `BuilderMethods`-abstracted walk in `rustc_codegen_ssa`, and a flatter, Cranelift-specific walk in `rustc_codegen_cranelift`. The contrast is illuminating for RegseqModel's design.

### 6.1 The codegen-SSA walk

`codegen_mir` at [`rustc_codegen_ssa/src/mir/mod.rs:172-321`] is a 5-phase pattern:

1. **Fetch.** `tcx.instance_mir(instance.def)` at line 182.
2. **FunctionCx setup.** Lines 199-239: build `start_llbb`, `cleanup_kinds` (only if backend wants new EH instructions, line 210-211), `cached_llbbs` (lazy per-block creation, line 213-219), `landing_pads`, `funclets`, `cold_blocks`, the `locals` sidecar, `debug_context`.
3. **Per-local debuginfo lift.** Lines 246-249: `compute_per_local_var_debug_info` lifts `Body.var_debug_info` into a per-Local map.
4. **Pre-walk.** Line 250-251: `traversal_order = mono_reachable_reverse_postorder(mir, tcx, instance)` and `memory_locals = analyze::non_ssa_locals(&fx, &traversal_order)`. Two pre-walks.
5. **Per-local allocation + per-block dispatch.** Lines 254-296 allocate locals (RETURN_PLACE → args → vars_and_temps); lines 308-311 dispatch per-block via `for bb in traversal_order { fx.codegen_block(bb); }`.

### 6.2 The cranelift walk

`codegen_fn` at [`rustc_codegen_cranelift/src/base.rs:33-147`] is structurally identical, *but flatter*:

- Line 47: same `tcx.instance_mir(instance.def)`.
- Lines 73-77: predefine ALL blocks eagerly (`block_map: IndexVec<BasicBlock, Block> = (0..mir.basic_blocks.len()).map(|_| bcx.create_block()).collect()`), unlike SSA's lazy creation.
- Lines 78-118: build `FunctionCx` directly, no `cleanup_kinds` (cranelift has no MSVC-funclet model), no `cached_llbbs` enum (just the predefined `block_map`), no `cold_blocks`, no `non_ssa_locals` analysis.
- Line 120: `codegen_fn_body(&mut fx, start_block)` -- the entire walk.
- Lines 284-321: `codegen_fn_body` is `for (bb, bb_data) in fx.mir.basic_blocks.iter_enumerated() { ... }` -- iteration in **storage order**, with reachability checked per-block via `traversal::mono_reachable_as_bitset` (line 300).

### 6.3 Why two walks: the `BuilderMethods` abstraction

The codegen-SSA walk goes through `Bx: BuilderMethods<'a, 'tcx>` (the trait at `rustc_codegen_ssa/src/traits/`). This abstraction is what lets the same `codegen_mir` function emit LLVM IR (via `rustc_codegen_llvm`), GCC IR (via `rustc_codegen_gcc`), or other backends through a uniform interface. Cranelift bypasses this -- it has its own monolithic walk because there's only one cranelift backend.

The two walks differ in:

- **Block creation timing.** SSA: lazy (only if RPO reaches the block); Cranelift: eager.
- **Cleanup tracking.** SSA: cleanup_kinds + funclets + landing_pads (MSVC EH support); Cranelift: just `bb_data.is_cleanup` cold-block flag.
- **Local classification.** SSA: `non_ssa_locals` pre-walk to decide alloca-vs-immediate; Cranelift: simpler decision baked into `codegen_fn_prelude`.

### 6.4 The internal `make_mir_visitor!` template

[`rustc_middle/src/mir/visit.rs:68-160`] defines the visitor template via the `make_mir_visitor!` macro -- *the* canonical visitor pattern for body-level walks in rustc. The doc-comments at [`mod.rs:1-63`] are the most explicit visitor-design rationale anywhere in rustc:

1. **`visit_X` + `super_X` discipline.** Each MIR type gets both methods; `visit_X` defaults to `super_X`; `super_X` destructures and recurses. Override `visit_X` for selectivity, call `super_X` for default behavior. Standard double-dispatch.
2. **Exhaustive destructuring requirement.** `super_basic_block_data` destructures via `let BasicBlockData { statements, terminator, is_cleanup: _ } = *data;` *deliberately* rather than `data.statements`, "because if one adds a new field to `BasicBlockData`, one will be forced to revise this code, and hence one will (hopefully) invoke the correct visit methods."
3. **`_` allowed only for non-visit fields.** Even `_` is restricted -- can be used for fields the visitor doesn't recurse into (like `is_cleanup`), but not as catch-all for variants.

The visitor methods cover the full vocabulary: `visit_body`, `visit_basic_block_data`, `visit_source_scope_data`, `visit_statement_debuginfo`, `visit_statement`, `visit_assign`, `visit_terminator`, `visit_assert_message`, `visit_rvalue`, `visit_operand`, `visit_ascribe_user_ty`, plus per-Place / per-Local / per-Constant / per-VarDebugInfo hooks.

### 6.5 The pre-walk pattern (D-11-6.1)

[`rustc_codegen_ssa/src/mir/analyze.rs:17-56`] is the canonical "walk-once-to-classify" precedent. `non_ssa_locals(&fx, traversal_order)` does the following:

1. Pre-allocate `IndexVec<Local, LocalKind>` (line 23-31), with `ZST | Memory | Unused | SSA(DefLocation)` classifications.
2. Mark all args as `define`-d (lines 36-38).
3. Iterate `traversal_order` in topological-sort-of-dominance order (lines 43-46), calling `analyzer.visit_basic_block_data(bb, data)` -- a `Visitor` impl.
4. Collect locals classified `Memory` into a `DenseBitSet<Local>` (lines 48-53).

The pattern: a *lightweight* visitor pass, run once before the main walk, populates a sidecar that the main walk consults. The visitor is a `MirVisitor` impl on `LocalAnalyzer` -- using the very `make_mir_visitor!`-generated trait we just discussed.

### 6.6 RegseqModel implications (D-11-6 + D-11-6.1)

Rustc's two walks tell us:

- **Eager-vs-lazy block creation is a backend concern, not an IR concern.** RegseqModel's bridge produces a body; the body's block container is dense (per stage-10 D-10-2). Eager iteration in storage order (the cranelift pattern) IS RPO under stage-10's B-RPO invariant. RegseqModel does not need an SSA-style lazy block-creation cache. Confirms kickoff D-11-6 hypothesis (b): the validation walk is a Body-method or visitor over the existing `IndexVec<BlockId, BasicBlock>` -- no per-block backend allocation step.
- **The visitor template is well-validated.** `make_mir_visitor!`'s `visit_X` / `super_X` discipline is the right shape for stage-11's joint validation walk. The exhaustive-destructuring rule is the methodological discipline that closes invariant 3 of stage-10 (B-Successors-Valid) -- if a future stage adds a Terminator variant, the visitor's `super_terminator` will be a forcing function for revisiting `successors()`.
- **The pre-walk pattern resolves D-11-6.1.** Construction invariants 1 (TyId closure) and 9 (InstanceId closure) cleanly fold into the same visitor that runs the per-content checks (D-5-2, D-8-4 Part C, D-9-10 Part C). The visitor's `visit_local_decl` hook closes invariant 1; `visit_terminator` closes invariant 9. *No separate pass needed.* Confirms kickoff D-11-6.1 hypothesis (a). The non_ssa_locals precedent is informative only as proof that the pattern works -- not a structural model RegseqModel needs to copy.

The model-vs-analysis line per stage-10 §24.2.3 means **the visitor lives in `regseq-model`** (used by `RegseqModel::validate`), not in `regseq-analysis`. CFG analyses (predecessors, dominators, RPO iteration) are derivable downstream.

---

## 7. Body-level transformation pass timing: rustc deliberately does NOT rewrite for dyn-Trait dispatch (D-11-7)

The single highest-value rustc-internal finding for stage 11. Rustc's MIR keeps `<dyn Trait as Trait>::method` Calls as **direct** `TerminatorKind::Call` with `func: Operand` referring to a virtually-dispatched callee through the type system. The dispatch happens at codegen consumption time, not as a MIR rewrite.

### 7.1 The codegen-SSA Virtual-call path

[`rustc_codegen_ssa/src/mir/block.rs:1165-1211`] (inside `codegen_call_terminator`):

```rust
if let (0, Some(ty::InstanceKind::Virtual(_, idx))) = (i, instance.map(|i| i.def)) {
    match op.val {
        Pair(data_ptr, meta) => {
            // unwrap newtypes (Pin<&mut Self>, Rc<Self>) until we have *dyn Trait or &dyn Trait
            while !op.layout.ty.is_raw_ptr() && !op.layout.ty.is_ref() {
                let (idx, _) = op.layout.non_1zst_field(bx).expect(
                    "not exactly one non-1-ZST field in a `DispatchFromDyn` type",
                );
                op = op.extract_field(self, bx, idx.as_usize());
            }
            // split fat pointer, look up vtable slot, replace function-pointer
            llfn = Some(meth::VirtualIndex::from_index(idx).get_fn(
                bx, meta, op.layout.ty, fn_abi,
            ));
            llargs.push(data_ptr);
            continue 'make_args;
        }
        ...
    }
}
```

The pattern: at the `i==0` arg processing (the receiver), check whether the resolved `Instance::def` is `InstanceKind::Virtual(_, idx)`. If so, unwrap `DispatchFromDyn` newtypes (Pin / Rc / etc.) until we have a fat pointer; extract data + vtable; load the function pointer from `vtable[idx]`; pass data as args[0]. The MIR Call is unchanged -- the dispatch is purely backend-side.

### 7.2 The codegen-SSA dyn-Trait drop path

[`rustc_codegen_ssa/src/mir/block.rs:624-665`] (inside `codegen_drop_terminator`, for `ty::Dynamic(_, _)`):

```rust
ty::Dynamic(_, _) => {
    let virtual_drop = Instance {
        def: ty::InstanceKind::Virtual(drop_fn.def_id(), 0), // idx 0: drop function
        args: drop_fn.args,
    };
    let fn_abi = bx.fn_abi_of_instance(virtual_drop, ty::List::empty());
    let vtable = args[1];
    args = &args[..1];  // truncate vtable off args
    (
        true,
        meth::VirtualIndex::from_index(ty::COMMON_VTABLE_ENTRIES_DROPINPLACE)
            .get_optional_fn(bx, vtable, ty, fn_abi),
        fn_abi,
        virtual_drop,
    )
}
```

Same pattern: at `Drop`-becomes-codegen time, when the dropped type is `dyn Trait`, fabricate `InstanceKind::Virtual(drop_fn_def, 0)`, look up vtable slot 0 (`COMMON_VTABLE_ENTRIES_DROPINPLACE`), call indirectly.

### 7.3 Why rustc can do this

Rustc's codegen has direct access to `tcx`, can call `instance.fn_abi`, `vtable_entries`, etc. on the fly. The MIR doesn't need to spell out the vtable load because the backend re-derives it from the Instance shape.

### 7.4 Why RegseqModel cannot

C is the codegen target. There is no `tcx`-equivalent at C-emission time. The rewrite must happen *before* C-emission -- specifically, before the renderer / C backend sees the body. The vtable load must be spelled out as MIR (Statements + Operand-of-loaded-fn-pointer) so the renderer-and-C-backend chain has nothing dyn-shaped to reason about.

This is the **structural reason** RegseqModel needs the rewrite Charon and mrustc do (and rustc does *not*). Q2 (eager translation, no `'tcx` post-bridge) plus C-emission target == rewrite-before-emission is forced.

### 7.5 The rustc transformation-pass template (`MirPass`)

For passes that DO rewrite (drop elaboration, deref-temp erasure, simplify-cfg, etc.), rustc uses the `MirPass` trait. Example: `ElaborateDrops` at [`rustc_mir_transform/src/elaborate_drops.rs:50-91`]:

```rust
impl<'tcx> crate::MirPass<'tcx> for ElaborateDrops {
    fn run_pass(&self, tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) { ... }
    fn is_required(&self) -> bool { ... }
}
```

The `MirPass` interface is `fn run_pass(&self, tcx, body: &mut Body)` -- the pass mutates a `&mut Body` directly, in place. Passes are sequenced via the pass-manager at `rustc_mir_transform/src/pass_manager.rs` (the same file the stage-8 expectations cite for the `mir_enable_passes` whitelist).

### 7.6 RegseqModel implications (D-11-7)

The peer-compiler design is **anti-aligned** with RegseqModel's needs:

- Rustc's **non-rewrite** approach is a real choice surface. C-emission cannot do it; Charon's `transform_dyn_trait_calls` rewrite is the structurally correct peer.
- Rustc's `MirPass::run_pass(tcx, body: &mut Body)` shape is the right *implementation template* for the rewrite passes that stage 11 will own (vtable-rewrite + Drop-as-Call). Mutate the body in place; pass receives `&mut Body`; bridge sequences passes.
- HYPOTHESIS: kickoff D-11-7 alt (c) (hybrid timing -- Drop-as-Call at bridge construction time per stage-9 §23.4 step 4; vtable-rewrite as post-construction `MirPass`-style pass per Charon's `transform_dyn_trait_calls`) is the correct shape. The split is structural: Drop-as-Call's "pre-Call `&raw mut place` Statement" depends on per-terminator translation context (the bridge has it; a post-construction pass does not); vtable-rewrite needs the full Body to walk all Calls and read `Instance::flavor=Virtual + virtual_idx=Some(idx)` (a post-construction pass can do this; the bridge's per-terminator translation has the Instance but doesn't have the consolidated body view yet for fresh-LocalId allocation across multiple rewrite sites in the same body).

The rustc evidence does NOT settle the timing question by itself (rustc doesn't do the rewrite at all). But it confirms the `MirPass` interface shape is well-validated for body-mutation, and provides the LocalDecls-allocation discipline (§8) that stage 11 must mirror.

---

## 8. LocalDecls allocation discipline (D-11-8)

Rustc's `LocalDecl` is constructed via two paths:

(a) **Initial body construction.** Builders push to `IndexVec<Local, LocalDecl>` directly during MIR construction in `rustc_mir_build`. The `Body::new` constructor at [`rustc_middle/src/mir/mod.rs:338-381`] takes a fully-built `IndexVec<Local, LocalDecl<'tcx>>` -- the construction phase has already populated it; `Body::new` does not push.

(b) **Post-construction mutation by `MirPass`es.** A pass mutates `body.local_decls.push(LocalDecl::new(ty, span))` directly via the `pub` field. Example: `LocalDecl::new` at [`rustc_middle/src/mir/mod.rs:1173-1189`]:

```rust
pub fn new(ty: Ty<'tcx>, span: Span) -> Self {
    Self::with_source_info(ty, SourceInfo::outermost(span))
}
pub fn with_source_info(ty: Ty<'tcx>, source_info: SourceInfo) -> Self {
    LocalDecl {
        mutability: Mutability::Mut,
        local_info: ClearCrossCrate::Set(Box::new(LocalInfo::Boring)),
        ty,
        user_ty: None,
        source_info,
    }
}
```

A fresh temp defaults to `Mut` + `LocalInfo::Boring` + no user-ty + outermost-span scope. Conversion to immutable via `LocalDecl::immutable` at [`mod.rs:1192-1196`].

### 8.1 Why no public `push_local`

There is no `Body::push_local(LocalDecl) -> Local` method in rustc. Mutation goes through `body.local_decls.push(...)`, returning the new `Local`. The reason is structural: `local_decls` is `pub`, so passes that need to push do so directly. There is no encapsulation barrier.

### 8.2 RegseqModel implications (D-11-8)

Two implementation-time options surface:

- **alt (a) (kickoff)**: `Body::push_local(LocalDecl) -> LocalId` method on a mutable Body. Mirrors rustc's pub-field-push pattern but adds method-level encapsulation.
- **alt (b) (kickoff, builder)**: `BodyBuilder { local_decls, ... }` with `.push_local()`, freezes into Body via `.build()`. Cleaner under Q2 (eager translation, fully-owned IR), but adds a builder surface.
- **alt (c) (kickoff, direct field access)**: `body.local_decls.push(...)` (matches rustc's pub-field discipline).

INFERENCE: under D-11-7 alt (c) (vtable-rewrite as post-construction pass), the builder pattern is over-engineered: rewrite passes need `&mut Body` and want to push during the walk, not in a separate build phase. **alt (a) or alt (c) is the right shape.** alt (a) is slightly more discoverable (the method documents intent); alt (c) is what rustc does and is fine.

The Drop-as-Call collapse (per stage-9 D-9-2) happens at bridge-construction time; the bridge can use whichever API stage 11 chooses (it owns construction). The vtable-rewrite pass is the second consumer; it needs `&mut Body` and pushes during traversal.

HYPOTHESIS: stage 11 will lock D-11-8 alt (a) -- a single `Body::push_local` method serves both consumers and is method-level discoverable. This refines kickoff D-11-8 hypothesis lean.

---

## 9. The categorical rendering deferral (D-11-K)

Rustc's pretty-printer is at `rustc_middle/src/mir/pretty.rs`. Stage 11 of RegseqModel does not own pretty-printing; per the scope decision recorded at the top of stage-11/expectations.md (and cross-referenced from stage-13/expectations.md), all body-level renderer ownership transfers to stage 13 alongside the existing stage-3 D14 renderer-style decision.

### 9.1 RegseqModel implications (D-11-K)

The `#[debug_format = "_{}"]` and `#[debug_format = "bb{}"]` attributes on rustc's `Local` and `BasicBlock` typed-indexes (§4 above) are evidence that the *macro-emitted Debug impl* is the natural spot for sigil rendering, not a separate `render_local_id` helper. Whether RegseqModel's macro re-uses the `debug_format` mechanism or stage 13 lands a hand-written renderer is a stage-13 decision; stage 11 lands the typed-index without prejudice to rendering.

The categorical-deferral D-11-K closes the per-stage K-tag retargeting cascade (D-5-K, D-6-K, D-7-K, D-8-K, D-9-K, D-10-K plus the stage-1 `_N` sigil and the stage-10 `bbN` sigil) into a single audit-shape entry that the merger will note as `[REOPENED] -- retargeted to stage 13 per D-11-K`.

---

## 10. Summary: D-11 evidence map

| D-tag | Rustc evidence resolves cleanly? | Key file:line | Lean |
|-------|----------------------------------|---------------|------|
| D-11-1 Body field set | Yes -- 17→6 collapse derived from removing each rustc-only requirement | `mir/mod.rs:208-335` | alt (a): 6 fields |
| D-11-2 LocalDecl field set | Yes -- 4-field candidate aligns with stripping LocalInfo + user_ty + scope | `mir/mod.rs:951-1054` + `LocalInfo` at `:1063-1093` | alt (a): 4 fields |
| D-11-3 FnSig shape | Partially -- `Body::return_ty()` precedent supports alt (d) | `mir/mod.rs:506-510`; `ty_kind.rs:728-744` | alt (d): drop fn_sig field |
| D-11-4 LocalId definition | Yes -- direct mirror of rustc's `Local` macro shape | `mir/mod.rs:842-850`; `rustc_index_macros/src/newtype.rs:87` | alt (a): newtype_index! const-body |
| D-11-5 BodyId vs ItemId | Yes -- rustc has no BodyId; `instance_mir(InstanceKind)` is the only fetch path | `ty/mod.rs:1671-1703` | alt (c): no BodyId |
| D-11-6 Walk shape and ownership | Yes -- `make_mir_visitor!` template + cranelift's iter_enumerated() peer | `mir/visit.rs:1-160`; `cranelift/base.rs:284-321` | alt (b): visitor in regseq-model |
| D-11-6.1 Invariant 1+9 visitor integration | Yes -- `non_ssa_locals` precedent confirms the fold-in pattern | `codegen_ssa/mir/analyze.rs:17-56` | alt (a): folded into visitor |
| D-11-7 Pass timing | **Anti-aligned** -- rustc does NOT rewrite; reinforces RegseqModel's need for the rewrite Charon/mrustc do | `codegen_ssa/mir/block.rs:1165-1211, 624-665`; `MirPass` template at `mir_transform/elaborate_drops.rs:50-91` | alt (c): hybrid (Drop at bridge, vtable as pass) |
| D-11-8 LocalDecls allocation API | Partially -- rustc uses pub-field discipline, no encapsulating method | `mir/mod.rs:1173-1196` | alt (a): `Body::push_local` method |
| D-11-K Categorical rendering deferral | Yes -- supports a single retargeting tag rather than per-stage K cascade | `mir/mod.rs:842-850`'s debug_format precedent | All renderers to stage 13 |

The single highest-value finding: rustc's **non-rewrite** strategy for dyn-Trait dispatch (codegen-time `InstanceKind::Virtual` consumption + `meth::VirtualIndex` vtable load + arg-rebinding) is structurally inadmissible for a C-emission target like RegseqModel. This is not a "rustc happens to do it differently" detail -- it is the load-bearing reason RegseqModel needs the body-level rewrite Charon's `transform_dyn_trait_calls` and mrustc's `MIR_Cleanup_Virtualize` perform. D-11-7 inherits this structural constraint from the codegen target, not from a peer-mimicry choice.

Open questions for the spec-drafting phase:
- D-11-1 ordering: `basic_blocks` last vs first within Body? (style, not semantics; HYPOTHESIS: last reads more naturally given LocalId / FnSig forward references). Stage 11 to settle.
- D-11-7 sequencing: when the vtable-rewrite pass runs, do the visitor-walked invariants (B-3 / B-4 / B-6 from stage-9 D-9-10 -- args/destination type-equality) hold pre-rewrite or only post-rewrite? Stage-9 expectations.md "Post-rewrite invariant timing" says post-rewrite; stage 11 to confirm and document the sequencing in §3.3 Phase B.

---

## Appendix: Quick file-by-file index

- `rustc_middle/src/mir/mod.rs:208-335` -- `Body<'tcx>` definition
- `rustc_middle/src/mir/mod.rs:506-510` -- `Body::return_ty()` (D-11-3 alt (d) precedent)
- `rustc_middle/src/mir/mod.rs:842-850` -- `Local` newtype_index! definition
- `rustc_middle/src/mir/mod.rs:858-867` -- `LocalKind` enum
- `rustc_middle/src/mir/mod.rs:951-1054` -- `LocalDecl<'tcx>` definition
- `rustc_middle/src/mir/mod.rs:1063-1093` -- `LocalInfo<'tcx>` enum
- `rustc_middle/src/mir/mod.rs:1095-1196` -- `LocalDecl` impl block (helpers + constructors)
- `rustc_middle/src/mir/mod.rs:1199-1257` -- `VarDebugInfo<'tcx>` and contents
- `rustc_middle/src/mir/visit.rs:1-160` -- `make_mir_visitor!` template
- `rustc_middle/src/ty/mod.rs:1671-1703` -- `instance_mir` dispatch
- `rustc_codegen_ssa/src/mir/mod.rs:172-321` -- `codegen_mir` 5-phase walk
- `rustc_codegen_ssa/src/mir/mod.rs:46-119` -- `PerLocalVarDebugInfoIndexVec` + `FunctionCx` structure
- `rustc_codegen_ssa/src/mir/locals.rs:15-83` -- `Locals<V>` sidecar
- `rustc_codegen_ssa/src/mir/analyze.rs:17-56` -- `non_ssa_locals` pre-walk pattern
- `rustc_codegen_ssa/src/mir/block.rs:624-665` -- dyn-Trait drop dispatch (codegen-time)
- `rustc_codegen_ssa/src/mir/block.rs:1165-1211` -- Virtual-call dispatch (codegen-time, args[0] receiver-rewrite)
- `rustc_codegen_cranelift/src/base.rs:33-147` -- cranelift `codegen_fn`
- `rustc_codegen_cranelift/src/base.rs:284-321` -- cranelift `codegen_fn_body` (storage-order iter)
- `rustc_index/src/lib.rs:16` -- `pub use rustc_index_macros::newtype_index;`
- `rustc_index_macros/src/newtype.rs:24, 87` -- const-body grammar handling
- `rustc_mir_transform/src/elaborate_drops.rs:50-91` -- `MirPass::run_pass(tcx, body: &mut Body)` template
