# Stage 11 -- Prior Art: Charon primary source

Primary-source Charon (ULLBC) evidence for **MIR bodies**: the
function-level `Body`/`FunDecl` container, the `Locals`/`Local`
analog of LocalDecl, the `FunSig` shape, the `LocalId` typed-index
machinery, the body-level visitor traits, the body-level
transformation passes (especially `transform_dyn_trait_calls` and
`desugar_drops` as the load-bearing peers of stage-9 D-9-3 / D-9-2),
and the fresh-local allocation API used by those passes. Scoped to
stage 11's candidate decision namespace:

- `D-11-1` Body field set (which fields the `Body` container carries).
- `D-11-2` LocalDecl field set (per-local payload).
- `D-11-3` FnSig shape beyond the locked `abi: FnAbi` enum.
- `D-11-4` LocalId typed-index definition (newtype/macro;
  `RETURN_PLACE` constant; niche).
- `D-11-5` Body↔Item linkage (BodyId vs ItemId resolution).
- `D-11-6` Joint validation walk shape and ownership.
  - `D-11-6.1` Construction invariant 1 + 9 integration into the
    visitor.
- `D-11-7` Body-level transformation pass timing (vtable-indirect
  rewrite + Drop-as-Call collapse).
- `D-11-8` LocalDecls allocation API (fresh-local synthesis surface).
- `D-11-K` Categorical rendering deferral to stage 13.

Charon is INRIA's Rust front-end translator feeding the Aeneas
verification pipeline. The relevant layer for stage 11 is **ULLBC**
("Unstructured LLBC"), which is the basic-block-shaped IR retained
directly from rustc MIR; LLBC is the structurally-reconstructed
sibling and is *not* RegseqModel's target.

Repository: `https://github.com/AeneasVerif/charon` (main).
Local clone pinned at commit
`46ab49f627e1a8e72b17c437f05ae4d1c4c456df` (2025-11-27, "Merge pull
request #919 from Nadrieril/raw-consts"), same commit as the
stage-{6,7,8,9,10} Charon files.

Citation convention: every factual claim cites `[file:line]` in the
Charon source tree at this commit, paths relative to
`/Users/nyengele/Documents/sekekama/charon-1/charon/`. `HYPOTHESIS:`
flags speculation; `INFERENCE:` flags cross-site synthesis.

This file does not duplicate stage-{6,7,8,9,10}/prior-art/charon.md.
Operand / Rvalue / Statement / Terminator / per-block `BlockData` /
body-level basic-block container material lives there. Stage 11's
lens is the function-level container that *holds* the basic-block
table -- the wrapper around `BodyContents`, plus the per-local
table, signature, and the post-construction passes that mutate
those.

NOTE: Charon's source layout differs from earlier stages' notes:
the AST lives at `src/ast/` rather than `charon-lib/src/ast/`. The
content is the same; the path stripped one nesting level.

---

## 1. Body container shape (D-11-1)

Charon's Body is a two-level construction:

**Level 1: the per-IR-flavor body wrapper.** ULLBC's `ExprBody` is
defined as `pub type ExprBody = GExprBody<BodyContents>`
[`src/ast/ullbc_ast.rs:16`]; `BodyContents` is the basic-block
table, `pub type BodyContents = Vector<BlockId, BlockData>`
[`src/ast/ullbc_ast.rs:15`]. This matches stage-10 D-10-2's bare-
typed-IndexVec line for the basic-block table.

**Level 2: the IR-agnostic generic wrapper.** `GExprBody<T>`
[`src/ast/gast.rs:46-58`] carries:

```rust
pub struct GExprBody<T> {
    pub span: Span,
    pub locals: Locals,
    pub comments: Vec<(usize, Vec<String>)>,  // drive-skip; line-position-keyed
    pub body: T,
}
```

Four fields total:
- `span` -- whole-body diagnostic location.
- `locals` -- the LocalDecls table plus `arg_count` (see §2 below).
- `comments` -- `//` line comments harvested from source, keyed by
  line position; later attached to specific statements by the
  `recover_body_comments` pass [`src/transform/mod.rs:194`].
  RegseqModel does NOT carry source comments (out of scope per the
  C-shape rule); this field has no analog.
- `body` -- the parameterized payload (`BodyContents` for ULLBC;
  `Block` for LLBC).

**Level 3: the function-level enum.** Atop `GExprBody<T>` sits the
`Body` enum [`src/ast/gast.rs:64-83`] with six variants:

```rust
pub enum Body {
    Unstructured(ullbc_ast::ExprBody),
    Structured(llbc_ast::ExprBody),
    TraitMethodWithoutDefault,
    Opaque,
    Missing,
    Error(Error),
}
```

The non-`Unstructured`/`Structured` variants encode body absence
*structurally* in the type -- "trait method without default
implementation," "user-marked opaque," "extern symbol whose body is
not in the .rmeta," "translation error." RegseqModel rejects
externs upstream (per the C-semantics rule and stage-12's Item
classification); the absence cases collapse to "no body present"
which can live as `Item.body: Option<Body>` rather than a six-
variant enum on the body itself.

**TODO surfaces a design tension.** The comment at `[gast.rs:44-45]`
flags: "TODO: arg_count should be stored in GFunDecl below. But
then, the print is obfuscated and Aeneas may need some refactoring."
Charon kept `arg_count` on `Locals` (rather than on `FunDecl`) for
pretty-printing convenience -- a documented violation of the
"signature data should live with the signature" instinct. Stage-11
should NOTE this and consciously decide whether `arg_count` belongs
on `Body` (Charon's choice), on `Locals` (Charon's `Locals.arg_count`
is the same nesting level), on `FnSig`, or duplicated.

**Implication for D-11-1.** Charon's evidence reads as a 4-field
body wrapper (span / locals / payload / [optional comments]). Comments
drop, locals embed `arg_count`, payload is the IndexVec table. Add a
Body↔Item linkage if D-11-5 lands at "no implicit linkage" (see §5).
The kickoff pass's hypothesised six-field Body
(`{ basic_blocks, local_decls, arg_count, fn_sig, span, source }`) is
close: Charon-flavored evidence collapses `local_decls + arg_count`
into a `Locals` substructure, drops `source`, and keeps
`fn_sig` external on FunDecl. INFERENCE: a 3-field Body
(`{ basic_blocks, locals: Locals, span }`) plus `Item.body:
Option<Body>` and `Item.fn_sig: FnSig` (signature on the Item, not
on the Body) is the Charon-shaped reading.

---

## 2. LocalDecl field set (D-11-2)

Charon's LocalDecl analog is `Local` [`src/ast/gast.rs:13-23`]:

```rust
pub struct Local {
    pub index: LocalId,
    pub name: Option<String>,
    pub ty: Ty,
}
```

Three fields:
- `index: LocalId` -- redundant with the `Vector<LocalId, Local>`
  position; Charon stores it for ergonomic visitors that work on
  `Local` without surrounding context (e.g., `place_for_var`
  [`src/ast/gast_utils.rs:52-55`] reconstructs a `Place` from a
  `LocalId` by indexing into `Locals`).
- `name: Option<String>` -- direct analog of stage-8 D-8-3
  `LocalDecl.debug_name`. `None` for compiler-introduced temporaries.
- `ty: Ty` -- the local's type.

**Critically missing vs the kickoff candidate (b)/(c) for D-11-2:**

- No `mutability` field. Charon does not track per-local
  mutability; it lives on `Place`/`Rvalue::Ref` borrow-kind and
  does not need to be replicated on the local itself. This is a
  precedent that local-level mutability is redundant for an IR
  whose mutation surface is statement-level. RegseqModel could
  follow.
- No `span` field. Charon's per-local source location is not
  preserved; per-statement spans handle diagnostics. RegseqModel's
  kickoff candidate (a) included `span: Span` -- Charon's evidence
  argues against it as redundant.
- No kind tag (`Return`/`Arg`/`Temp`). Charon uses positional
  convention: index 0 is return; 1..=arg_count are inputs; rest are
  temps. The helper `Locals::is_return_or_arg(lid)` is at
  `[src/ast/gast_utils.rs:58-60]`. SAME convention as rustc and
  RegseqModel's stage-1 layout invariant.
- No `is_synthetic` flag for bridge-introduced locals. Charon's
  `desugar_drops` and `transform_dyn_trait_calls` create fresh
  locals with `name: Some("drop_arg")` etc., distinguishing them
  via the name string. Stage-9 expectations specifies similar
  `_t_recv`/`_t_meta`/...; the name string is the distinguisher,
  no separate flag needed. INFERENCE: this confirms compositional
  behavior beats special-case flags (README §2 rule).

**Implication for D-11-2.** Charon's 3-field `Local`
(`index, name, ty`) is the leanest evidence-supported shape.
The kickoff candidate (a)'s 4 fields drop to 3 if RegseqModel
follows Charon and drops `span` and `mutability`; or stay at 3 if
it drops `span` only. The `index` field is a convenience for
position-free visitors and is cheap to copy from the IndexVec
position; whether to keep it is an ergonomic call. HYPOTHESIS:
the choice surface for D-11-2 narrows to:
- (a') `{ ty: TyId, name: Option<StringId> }` (Charon-strict, drops
  index/mutability/span)
- (a'') `{ ty: TyId, name: Option<StringId>, mutability }` (preserve
  mutability for downstream codegen flag-passing without re-walking
  Places)
- (a''') `{ ty: TyId, name: Option<StringId>, span }` (preserve
  per-local diagnostic location; Charon's choice not to do this is
  documented as a stylistic call, not a deeply principled one)

Charon's silence on per-local mutability and span is the
dominant signal: neither is required by the IR, both are reachable
from other content.

---

## 3. FunSig shape (D-11-3)

Charon's FunSig is at `[src/ast/types.rs:953-962]`:

```rust
pub struct FunSig {
    pub is_unsafe: bool,
    pub generics: GenericParams,
    pub inputs: Vec<Ty>,
    pub output: Ty,
}
```

Four fields. Notable ABSENCES:
- **No `abi` field.** Searching the entire `src/ast/` tree confirms
  Charon has no Abi enum or `fn_abi` field anywhere. INFERENCE:
  Charon is verification-oriented (Aeneas) and does not generate
  native code; ABI distinctions are irrelevant to its output. This
  evidence is **not generalizable** to RegseqModel: the stage-3 D15
  lock of `FnAbi = { Rust, C }` is C-codegen-driven, and Charon's
  silence on ABI does not argue against it.
- **No `c_variadic` field.** No representation for variadic
  functions in Charon's IR. INFERENCE: variadic functions either
  pre-translation-reject (Charon doesn't model `extern "C"` C-FFI
  with variadics for a verifiable Rust subset) or fold into closure
  analyses. Not transferable.

The PRESENT `is_unsafe: bool` is a Charon-specific carry-through;
Aeneas may use it for unsafe-block scoping. Not load-bearing for
RegseqModel (which rejects unsafe at the DSL surface unless
intrinsic-tagged).

The `generics: GenericParams` field is present because Charon
operates on **generic, non-monomorphized IR** (the
`partial_monomorphization` pass is at SHARED_FINALIZING_PASSES line
197, near the end of the pipeline). RegseqModel is post-mono, so
this field is OUT regardless.

**Critically: Charon keeps FunSig as a structural field of FunDecl,
NOT redundant with Locals.** `FunDecl.signature: FunSig`
[`src/ast/gast.rs:163`] coexists with the `Locals` whose
indices `[0..=arg_count]` carry the same type information. Charon
does not collapse one into the other. This is **direct evidence
against D-11-3 alt (d)** (the kickoff's "drop fn_sig field, reach
data through local_decls" alternative). Three peers (rustc-internal,
rustc_public, Charon) all keep a separate signature on the
function declaration; alt (d) has no precedent.

INFERENCE on the precedent: the signature-field is for *callsite
type-checking and trait-method binders*, not for callee body
introspection. Even though `local_decls[0..=arg_count]` carries the
same types, callsites need access to the signature without loading
the body (e.g., for FnPtr type-equality at `Call.func` resolution).
RegseqModel has the same need: stage-9 D-9-1 invariant B-3
(Call destination type matches callee return type) and B-6 (per-arg
type match) require fetching the signature *of the callee* without
loading its body.

**Implication for D-11-3.** Charon's evidence strongly supports
keeping FunSig as a structural field. Inputs / output are mandatory.
ABI is RegseqModel-specific (per D15) and gets a separate
`abi: FnAbi` field. `c_variadic: bool` is the open Q (kickoff
candidate alt (b) vs alt (a)); Charon is silent on this, so the
choice falls to whether RegseqModel needs C variadic interop --
HYPOTHESIS: yes for `printf`-style intrinsics in a driver context.
`generics` and `is_unsafe` are dropped (post-mono and DSL-rejected,
respectively).

---

## 4. LocalId typed-index definition (D-11-4)

Charon's `LocalId` declaration is one line:
`generate_index_type!(LocalId, "");` [`src/ast/values.rs:15`].

The macro is at `[src/ids/mod.rs:14-47]`:

```rust
macro_rules! generate_index_type {
    ($name:ident) => { $crate::generate_index_type!($name, stringify!($name)); };
    ($name:ident, $pretty_name:expr) => {
        index_vec::define_index_type! {
            #[derive(derive_generic_visitor::Drive, ...)]
            #[drive(skip)]
            pub struct $name = usize;
            MAX_INDEX = std::u32::MAX as usize;
        }
        impl $name {
            pub const ZERO: Self = Self { _raw: 0 };
            pub fn is_zero(&self) -> bool { self.index() == 0 }
            pub fn to_pretty_string(self) -> String {
                format!("@{}{}", $pretty_name, self)
            }
        }
        impl std::fmt::Display for $name { ... }
    };
}
```

Four design choices visible:

1. **Backing storage is `usize` with `MAX_INDEX = u32::MAX`.** This
   gives a 4-byte effective range while leaving niche room. Charon's
   serialization assumes u32-fitting IDs.
2. **`::ZERO` constant is universal across all generated types.**
   Charon does not need a per-call const-body arm because every
   index gets `::ZERO` and `is_zero()` automatically. RegseqModel's
   stage-10 D-10-3.2 took a different approach (extending the
   `newtype_index!` macro with a per-call const-body arm); Charon's
   precedent says the universal-`ZERO` approach is also viable.
3. **The pretty-name parameter is decoupled from the type name.**
   Charon's `LocalId` is invoked with empty pretty-name (`""`), so
   its rendered form is `@{idx}`. `BlockId` uses
   `generate_index_type!(BlockId, "Block")` [`src/ast/ullbc_ast.rs:9`]
   so its rendered form is `@Block{idx}`. RegseqModel's stage-10
   chose `bb{idx}` for BlockId; the underscore-prefix `_{idx}`
   convention for LocalId is rustc's (and stage-1's deferred
   convention). Both are precedented.
4. **No `RETURN_PLACE` constant.** Charon hardcodes the convention
   `LocalId::new(0)` at `place_for_var` callsites
   [`src/ast/gast_utils.rs:64`]: `pub fn return_place(&self) -> Place
   { self.place_for_var(LocalId::new(0)) }`. The `::ZERO` constant
   is the only canonical zero. RegseqModel's stage-10 D-10-3.2
   added `START_BLOCK` as a named const for `BlockId(0)`; the
   parallel choice for `LocalId::RETURN_PLACE` is open.

**START_BLOCK_ID convention.** Charon also has a separate
`pub static START_BLOCK_ID: BlockId = BlockId::ZERO;`
[`src/ast/ullbc_ast.rs:12`] -- a top-level static rather than a
type-associated constant. This duplicates the `::ZERO` constant
under a more semantic name. Same idea, different implementation.

**Implication for D-11-4.** Charon's `generate_index_type!` macro
gives `::ZERO` for free, no const-body arm needed. RegseqModel's
stage-10 chose the const-body-arm path (more general, allows
arbitrary named constants). For `LocalId::RETURN_PLACE`, both
implementation paths are precedented; the kickoff candidate (a) is
consistent with stage-10's choice. Niche optimization for
`Option<LocalId>` is solved by `MAX_INDEX = u32::MAX` in Charon's
case; stage-1's existing assertion approach is parallel.

---

## 5. Body↔Item linkage (D-11-5)

Charon does NOT have a `BodyId`. Bodies live structurally inside
`FunDecl.body: Body` [`src/ast/gast.rs:172`]:

```rust
pub struct FunDecl {
    pub def_id: FunDeclId,
    pub item_meta: ItemMeta,
    pub signature: FunSig,
    pub src: ItemSource,
    pub is_global_initializer: Option<GlobalDeclId>,
    pub body: Body,
}
```

Five other fields are paired with `body`:
- `def_id` -- the `FunDeclId` typed-index for this function.
- `item_meta` -- name, attributes, lang-item label, source location.
- `signature` -- as discussed in §3.
- `src` -- the `ItemSource` classification (TopLevel / Closure /
  TraitDecl / TraitImpl / VTableTy / VTableInstance /
  VTableMethodShim) [`src/ast/gast.rs:108-152`].
- `is_global_initializer` -- back-link to a `GlobalDeclId` if this
  function is the initializer for a const or static.

**There is no separate `bodies: Vector<BodyId, Body>` table.** Body
lookup goes through `TranslatedCrate.fun_decls.get(id).body`. The
`TranslatedCrate` struct [`src/ast/krate.rs:163-208`] has
`fun_decls: Vector<FunDeclId, FunDecl>` and no parallel body table.

**This matches kickoff candidate (c) for D-11-5** ("no BodyId at
all -- bodies live in `Item.body: Option<Body>` directly"). Charon
is the strongest peer evidence: the body-as-field design is in
production use in a verification toolchain. The downside Charon
encountered (the `arg_count`-on-`Locals` TODO at gast.rs:44-45)
is mild and orthogonal.

**Aside: shim items get their own FunDecls.** Charon's `ItemSource`
distinguishes vtable shims, vtable instances, vtable types, etc. as
their own item-source categories [`src/ast/gast.rs:142-152`].
RegseqModel's stage-12 inbound for vtable items is parallel
(stage-12/expectations.md "From stage 3" Section A); the precedent
here is that synthesised items get FunDeclIds and own their bodies
the same as user-authored functions. INFERENCE: this aligns with
"every body lives in some Item" universally, with no special "shim
body" category.

**Implication for D-11-5.** Charon's evidence resolves the
question cleanly toward alt (c). FunDecl-as-anchor is a clean
single-level design; Q3 (single-level IR) and the stage-9
construction-invariant-9 closure (every Direct InstanceId resolves
in `model.instances`) are both consistent with this shape.

---

## 6. Joint validation walk (D-11-6, D-11-6.1)

Charon defines TWO visitor traits with distinct scopes:

**`AstVisitable` / `VisitAst`** [`src/ast/visitor.rs:38-96`]: full
crate-level visitor. Drives over types, generics, traits, items,
bodies, statements, terminators, ... -- everything reachable from
a `TranslatedCrate`. Uses the
`derive_generic_visitor::Drive[Mut]` derive for the actual
traversal.

**`BodyVisitable` / `VisitBody`** [`src/ast/visitor.rs:130-191`]:
smaller body-level visitor. Skips types (`skip(...AbortKind, BinOp,
... Ty, ...)`) and offers `override(...Operand, Place,
ProjectionElem, Rvalue, ... ullbc_terminator: ullbc_ast::Terminator,
ullbc_block_id: ullbc_ast::BlockId)`. This is the visitor used by
post-construction body passes.

**Critical design point:** Charon is intentionally bi-modal. A
type-level concern (e.g., generic parameter substitution) uses
`VisitAst`; a body-level concern (e.g., `transform_dyn_trait_calls`)
uses `VisitBody`. Body passes don't recurse into types except where
they want to (e.g., `Place::ty` lookups via the surrounding context).

The body visitor lists override hooks for: `AggregateKind, Call,
FnOperand, FnPtr, Operand, Place, ProjectionElem, Rvalue, Locals,
LocalId, llbc_block, llbc_statement, ullbc_statement,
ullbc_terminator, ullbc_block_id`
[`src/ast/visitor.rs:171-179`]. Note `Locals` and `LocalId` are
explicit override targets -- a visitor can hook into per-local-decl
visits, mirroring rustc_public's `visit_local_decl`.

**Implication for D-11-6.** Charon strongly precedents kickoff
candidate (b) -- a body-level visitor distinct from a full-IR
visitor, owned by `regseq-model` (since validation enforces the
invariants `regseq-model` is responsible for). The override hooks
needed for stage-5 + stage-8 + stage-9 + stage-10 walks all
correspond to existing Charon override targets, confirming the
shape composes. The stage-10 §24.4.3 joint DFS-RPO walk, however,
is structural (CFG-only) and slightly different from the per-
content visitor pattern -- Charon doesn't have an equivalent
because its `BodyContents` is a plain `Vector<BlockId, BlockData>`
without an explicit reachability/RPO check (though stage-10
already locked B-RPO as the storage layout, so RPO is a free byproduct).

**D-11-6.1 (construction-invariant integration).** Charon's
visitor pattern naturally absorbs cross-cutting checks via
override hooks. There is no separate "pre-pass to gather facts"
pattern; closure-walking happens inside the per-content visitor
hooks.

---

## 7. Body-level transformation pass timing (D-11-7)

This is the load-bearing section. Charon implements both the
vtable-indirect rewrite AND the Drop-as-Call collapse as
**post-construction passes**, NOT as bridge-construction-time
work.

### 7.1 Pass framework

`UllbcPass` trait [`src/transform/ctx.rs:23-59`]:

```rust
pub trait UllbcPass: Sync {
    fn transform_body(&self, _ctx: &mut TransformCtx,
                      _body: &mut ullbc_ast::ExprBody) {}
    fn transform_function(&self, ctx: &mut TransformCtx,
                          decl: &mut FunDecl) {
        if let Some(body) = decl.body.as_unstructured_mut() {
            self.transform_body(ctx, body)
        }
    }
    fn transform_ctx(&self, ctx: &mut TransformCtx) {
        ctx.for_each_fun_decl(|ctx, decl| {
            self.log_before_body(ctx, &decl.item_meta.name, &decl.body);
            self.transform_function(ctx, decl);
        });
    }
    ...
}
```

A pass is a plain trait-object that implements `transform_body` or
`transform_function`. The framework takes care of iterating across
all FunDecls and unwrapping the `Body::Unstructured` variant.

**Pipeline organization** [`src/transform/mod.rs:75-214`]: passes
are grouped into ordered phases:
- `INITIAL_CLEANUP_PASSES` (after raw rustc-MIR translation; both
  `desugar_drops` and `transform_dyn_trait_calls` run here)
- `ULLBC_PASSES` (further on cleaned-up basic-block IR)
- `LLBC_PASSES` (after ULLBC -> LLBC structural reconstruction)
- `SHARED_FINALIZING_PASSES`
- `FINAL_CLEANUP_PASSES`

### 7.2 `desugar_drops` -- the Drop-as-Call collapse

Located at `[src/transform/normalize/desugar_drops.rs:23-72]`. Key
points:

- **Pass scheduling:** registered in INITIAL_CLEANUP_PASSES
  [`src/transform/mod.rs:104`] via
  `UnstructuredBody(&normalize::desugar_drops::Transform)`.
- **Gating:** the pass is gated by
  `if !ctx.options.desugar_drops { return; }`
  [`src/transform/normalize/desugar_drops.rs:79-81`]; it runs only
  when the `--desugar-drops` CLI flag is set. Charon's default is
  to leave Drop preserved; users opt in to the collapse.
- **Translation shape (lines 23-72):** for each `TerminatorKind::Drop
  { kind: DropKind::Precise, place, tref, target, on_unwind }`:
  1. Check for noop destructor (line 33); if so, replace with
     `TerminatorKind::Goto { target }`.
  2. Allocate a fresh local for `&raw mut place`:
     `drop_arg = self.raw_borrow_to_new_var(place.clone(),
     RefKind::Mut, Some("drop_arg".into()))` (line 41-42).
  3. Resolve the `drop_in_place` method via the trait declaration
     (lines 44-54).
  4. Allocate a fresh unit-typed return slot:
     `drop_ret = self.fresh_var(Some("drop_ret".into()),
     Ty::mk_unit())` (line 56).
  5. Build the Call: `Call { func: FnOperand::Regular(fn_ptr),
     args: Vec::from([Operand::Move(drop_arg)]), dest: drop_ret }`
     (lines 57-65).
  6. Replace the terminator: `term.kind = TerminatorKind::Call {
     call, target: target.clone(), on_unwind: on_unwind.clone() }`
     (lines 66-70). `target` and `on_unwind` block IDs are
     preserved.

**This matches stage-9 §23.4 step 4 EXACTLY** in the structural
shape. The differences:
- Charon does it as a post-construction pass (here); stage-9
  expectations describes it as bridge-construction-time work
  ("This is bridge-construction-time work, not a post-construction
  pass" -- stage-11/expectations.md line 950).
- Charon makes it CLI-gated; RegseqModel makes it always-on per
  D-9-2.

### 7.3 `transform_dyn_trait_calls` -- the vtable-indirect rewrite

Located at
`[src/transform/normalize/transform_dyn_trait_calls.rs:31-122]`.
Key points:

- **Pass scheduling:** registered in INITIAL_CLEANUP_PASSES at
  [`src/transform/mod.rs:111`]. Comment at line 110: "This should
  be early to handle the calls before other transformations."
- **Always-on** (no CLI gating).
- **Translation shape:** for each `TerminatorKind::Call` whose
  `call.func = FnOperand::Regular(fn_ptr)` with
  `fn_ptr.kind = FnPtrKind::Trait(trait_ref, method_name, _)` and
  `trait_ref.kind = TraitRefKind::Dyn`:
  1. Look up the vtable struct's TypeDeclRef via
     `trait_decl.vtable` (lines 50-67).
  2. Look up the method field by name in the vtable struct's
     fields: `vtable_decl.get_field_by_name(None,
     &format!("method_{}", method_name))` (lines 76-89). Charon's
     vtable methods are field-named `method_<name>`; RegseqModel's
     stage-12 will have its own field-naming convention.
  3. Extract the receiver from `call.args[0]` -- panic on
     `Operand::Const(_)` (line 102).
  4. Build the method-field Place via projection chain:
     `dyn_trait_place.clone().project(ProjectionElem::PtrMetadata,
     ptr_to_vtable_ty).project(ProjectionElem::Deref,
     vtable_ty).project(ProjectionElem::Field(...,
     method_field_id), method_ty)` (lines 109-116).
  5. Replace `call.func`: `call.func =
     FnOperand::Move(method_field_place);` (line 119). NO fresh
     locals, NO new statements.

**Key divergence from RegseqModel's stage-9 plan.** Charon does the
rewrite **in-place** by composing place projections. RegseqModel's
stage-9 expectations.md describes a multi-step rewrite with five
fresh locals (`_t_recv`, `_t_meta`, `_t_fn`, `_t_data`, `_t_raw`)
and source normalization for `&T -> *const T` reference
receivers (stage-11/expectations.md lines 696-839). The reason for
the divergence: Charon's `Place` carries `PtrMetadata` and `Deref`
projection variants directly; RegseqModel's stage-5 D-5-1 locks a
6-variant `PlaceElem` (Deref / Field / Index / ConstantIndex /
Subslice / Downcast) -- NO `PtrMetadata` projection variant. So
RegseqModel cannot do the in-place projection trick; it must
materialize the projection chain into intermediate locals via
Statements.

INFERENCE: this is a real cross-stage friction signal. Stage-5's
D-5-1 closed without a `PtrMetadata` variant on grounds that the
rustc cleanup-pass-discharged set didn't include it. The vtable-
rewrite use case may surface as a "this would have been simpler if
PlaceElem had PtrMetadata" finding. Stage-11 should consciously
record that the multi-statement rewrite is what stage-5's locked
vocabulary forces, not a free design choice.

**Charon does NOT panic on `Operand::Const(_)` receivers in
practice;** the pass returns early on opaque vtable decls (lines
72-74). Stage-9 expectations specifies the same return-early
behavior for the constant case (line 754-762).

### 7.4 Implication for D-11-7

Charon's evidence is **strong for alt (b) post-construction
passes** for both the vtable-indirect rewrite AND Drop-as-Call. The
hybrid alt (c) (Drop-as-Call at bridge time; vtable-rewrite as
post-construction) is what stage-9's expectations.md prescribes;
Charon shows alt (b) (both post-construction) is also workable.
The tradeoff:

- **Alt (b) (Charon's choice):** simpler bridge construction. The
  bridge translates rustc terminators 1:1 -- including Drop -- and
  passes run on the resulting RegseqModel. Drop preservation
  requires a `Drop` variant in `TerminatorKind`, which RegseqModel
  stage-9 D-9-1.1 explicitly DROPPED (Drop is not in the locked
  6-variant set).
- **Alt (c) (stage-9's prescription):** keeps RegseqModel's
  Terminator vocabulary stricter (no Drop variant). The cost is
  that the bridge-construction-time Drop-collapse logic lives at
  the bridge boundary, not in a discrete pass.

The stage-9 D-9-1.1 lock means alt (b) is **not viable for
RegseqModel without reopening D-9-1.1**. The hybrid alt (c) is
forced by the existing vocabulary lock. INFERENCE: this is not a
true open question; the stage-9 lock has already preempted alt (b).
What stage-11 does decide is the *machinery shape* of the bridge-
side Drop collapse and the post-construction shape of the vtable-
rewrite pass.

---

## 8. LocalDecls allocation API (D-11-8)

Charon's fresh-local API is layered:

**Layer 1: `Locals::new_var`** [`src/ast/gast_utils.rs:42-49`]:

```rust
pub fn new_var(&mut self, name: Option<String>, ty: Ty) -> Place {
    let local_id = self.locals.push_with(|index| Local {
        index, name, ty: ty.clone(),
    });
    Place::new(local_id, ty)
}
```

Mutates `&mut Locals` directly. Returns a `Place` (not a `LocalId`)
for ergonomic chaining: callers can use `Place` directly in
operands and assignments without an extra `Place::from(local_id)`
construction. This is the leanest layer.

**Layer 2: `BodyTransformCtx::fresh_var`**
[`src/transform/ctx.rs:221-225`]:

```rust
fn fresh_var(&mut self, name: Option<String>, ty: Ty) -> Place {
    let var = self.get_locals_mut().new_var(name, ty);
    self.insert_storage_live_stmt(var.local_id().unwrap());
    var
}
```

Wraps `Locals::new_var` with auto-StorageLive insertion.
**RegseqModel does NOT carry StorageLive in stage-8's locked
StatementKind** (D-8-1 has 3 variants; StorageLive is in the
rejection table per D-8-1.1 cardinality). So this auto-StorageLive
wrapper does not transfer; RegseqModel's `fresh_var` would just
forward to the leaner `Locals::new_var` analog without
StorageLive insertion.

**Layer 3: domain-specific helpers.** Examples:
- `borrow_to_new_var` [`src/transform/ctx.rs:412-418`]: fresh-var +
  emit `Assign(target, Rvalue::Ref(...))`.
- `raw_borrow_to_new_var` [`src/transform/ctx.rs:420-431`]: fresh-
  var + emit `Assign(target, Rvalue::RawPtr(...))`. **Used by
  desugar_drops to create the `&raw mut place` argument.**
- `compute_subslice_end_idx`, `compute_place_metadata`, and others
  -- higher-level multi-statement synthesizers using the lower
  layers.

**Trait-based context.** `BodyTransformCtx`
[`src/transform/ctx.rs:206-213`]:

```rust
pub trait BodyTransformCtx: Sized {
    fn get_crate(&self) -> &TranslatedCrate;
    fn get_options(&self) -> &TranslateOptions;
    fn get_params(&self) -> &GenericParams;
    fn get_locals_mut(&mut self) -> &mut Locals;
    fn insert_storage_live_stmt(&mut self, local: LocalId);
    fn insert_storage_dead_stmt(&mut self, local: LocalId);
    fn insert_assn_stmt(&mut self, place: Place, rvalue: Rvalue);
    ...
}
```

Concrete implementations include `UllbcStatementTransformCtx<'a>`
[`src/transform/ctx.rs:434+`] which carries `ctx, params, locals,
span` plus statement-buffering state. The trait abstracts the
fresh-var allocation logic from the concrete pass shape.

**Implication for D-11-8.** Charon strongly precedents the **method-
on-Locals** approach (alt (a) flavor) with **higher-level helper
traits on transform contexts** for multi-statement synthesis. NOT
a `BodyBuilder { ... }.build()` pattern (kickoff alt (b)); NOT
direct field access (kickoff alt (c)). The layered design is the
key insight: leaf API is `Locals::new_var(name, ty) -> Place`, and
domain-specific helpers (raw-borrow-to-new-var, etc.) live on a
trait that abstracts the surrounding context. RegseqModel can
mirror this exactly, modulo the StorageLive auto-insertion which
does not apply.

---

## 9. Categorical rendering deferral (D-11-K)

Charon's pretty-printer is **co-located with the IR** (`src/pretty/`)
rather than deferred to a "rendering stage." The renderer is a
single trait `FmtWithCtx` with per-IR-type implementations
distributed alongside each IR module. There is no analog to
RegseqModel's stage-13 categorical separation.

This makes Charon a poor precedent for D-11-K. The decision to
defer all renderers to stage 13 is a RegseqModel-specific
methodology choice (spec-before-implementation discipline; renderer
ownership is a stage-13-owned concern). Charon's silence here is
not evidence against the deferral.

INFERENCE: D-11-K's audit-shape entry should cite stage-{5,6,7,8,9,
10}/discussions.md K-tags as the precedent (those K-tags were
per-renderer deferrals targeting stage 11; the categorical re-
targeting to stage 13 supersedes them). Charon evidence does NOT
add to the audit shape for D-11-K.

---

## 10. Synthesis: per-D-tag verdict

| D-tag | Charon evidence verdict | Choice surface narrowed |
|---|---|---|
| D-11-1 (Body field set) | RESOLVES PARTIALLY | 3 fields if Body↔Item linkage is implicit (kickoff alt (a) + (c) hybrid). Charon-shaped reading: `{ basic_blocks, locals: Locals, span }`, with `arg_count` *inside* `Locals`, FnSig on the Item, no `source` field. Forward-tension: Charon's TODO-flagged `arg_count`-on-`Locals` is a stylistic call. |
| D-11-2 (LocalDecl) | RESOLVES STRONGLY | 2-3 fields (`{ ty, name }` minimum; `mutability` and `span` optional and Charon-silent). Drops the kickoff candidate's per-local span and per-local mutability as redundant. |
| D-11-3 (FnSig shape) | RESOLVES STRONGLY (against alt (d)) | Keep FnSig as a structural field of the Item (NOT redundant with Locals). Inputs / output / abi mandatory. `c_variadic` open. Charon's `is_unsafe` and `generics` do not transfer. |
| D-11-4 (LocalId) | RESOLVES PARTIALLY | Two viable macro shapes: universal-`::ZERO` (Charon) vs per-call const-body arm (stage-10 path). Stage-10's choice already locked the per-call path; D-11-4 inherits. Niche optimization is solved by `MAX_INDEX = u32::MAX` in Charon. |
| D-11-5 (BodyId vs ItemId) | RESOLVES STRONGLY | Alt (c) -- no BodyId; bodies live in `Item.body`. Charon is the production peer for this design. |
| D-11-6 (validation walk) | RESOLVES STRONGLY | Alt (b) -- visitor trait owned by `regseq-model`, distinct from a wider crate-level visitor (Charon's bi-modal split: `VisitAst` vs `VisitBody`). Override hooks for Operand/Place/Rvalue/Statement/Terminator/Locals/LocalId. |
| D-11-6.1 (invariant 1+9 integration) | RESOLVES STRONGLY | Folds into the visitor's per-LocalDecl and per-Terminator hooks; no separate pre-pass. |
| D-11-7 (pass timing) | RESOLVES PARTIALLY | Charon's full alt (b) (both passes post-construction) is preempted by stage-9 D-9-1.1's rejection of a `Drop` variant. Forced into hybrid alt (c) (Drop-as-Call at bridge time; vtable-rewrite as post-construction). The vtable-rewrite *shape* matters: Charon's in-place projection-chain trick requires a `PtrMetadata` PlaceElem variant which RegseqModel stage-5 D-5-1 does NOT have, forcing the multi-statement form. |
| D-11-8 (LocalDecls API) | RESOLVES STRONGLY | Layered design: leaf `Locals::new_var(name, ty) -> Place` + higher-level domain helpers on a `BodyTransformCtx` trait. NOT a BodyBuilder pattern. |
| D-11-K (rendering deferral) | NOT APPLICABLE | Charon co-locates renderer with IR; no precedent for the categorical stage-13 deferral. RegseqModel-internal methodology choice. |

## 11. Open question Charon does NOT resolve

**The per-Local mutability question for D-11-2.** Charon dropped
mutability from `Local`; rustc's `LocalDecl.mutability` keeps it
[rustc kickoff §B]. The choice depends on whether RegseqModel's
codegen wants to consume `LocalDecl.mutability` directly (vs.
inferring mutability from Place/Rvalue patterns at codegen time).
Charon doesn't generate code, so its silence here is non-evidence.
RegseqModel's choice should be made on its own codegen needs
(stage-14 territory), not on Charon precedent.

**The `source: MirSource` analog in Body.** rustc's `Body.source`
carries `def_id + InstanceKind`-promoted info; Charon's FunDecl
already has `def_id`, `item_meta`, and `src: ItemSource` so the
information is structurally present, just on the FunDecl-side
rather than the Body-side. The kickoff candidate (a) for D-11-1
includes a body-level `source` field; Charon's evidence argues for
moving it to the Item-side. INFERENCE: this dovetails with D-11-5's
"no BodyId; bodies live in Item.body" -- if the body is owned by
the Item, item-side source info is reachable from the body via the
inverse (one Body's enclosing Item can be tracked separately if
needed, but the Item-as-anchor design makes this the natural
question).
