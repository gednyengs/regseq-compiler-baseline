# Stage 11 -- Prior Art: mrustc primary source

Primary-source mrustc evidence for the **MIR body container
vocabulary**, scoped to what stage 11's candidate decision
namespace
(`D-11-1` Body field set; `D-11-2` LocalDecl field set; `D-11-3`
FnSig shape (full, not just Abi); `D-11-4` LocalId typed-index
definition; `D-11-5` BodyId vs ItemId resolution; `D-11-6` joint
validation walk shape and ownership; `D-11-6.1` construction
invariant 1 + 9 integration into the visitor; `D-11-7` body-level
transformation pass timing (vtable rewrite + Drop-as-Call);
`D-11-8` LocalDecls allocation API; `D-11-K` categorical
rendering deferral to stage 13) needs.

mrustc is the closest existing peer: a Rust-to-C compiler with
its own HIR/MIR, written in C++. Its `MIR::Function` and
`MirBuilder`/`MirMutator` allocation APIs directly inform the
design space for RegseqModel's `Body`, `LocalDecl`, and the
construction-time vs transformation-time API split. The
stage-{6,7,8,9,10}/prior-art mrustc files
(`./../../stage-6/prior-art/mrustc.md`,
`./../../stage-7/prior-art/mrustc.md`,
`./../../stage-8/prior-art/mrustc.md`,
`./../../stage-9/prior-art/mrustc.md`,
`./../../stage-10/prior-art/mrustc.md`) already covered the
`LValue`, `Param`, `Constant`, `RValue`, `eBinOp`, `eUniOp`,
`Statement`, `Terminator`, and `BasicBlock` surfaces; this file
does not re-derive that material. It focuses on the **body-level
container** holding stage-10's basic-block table: the
`MIR::Function` class, the per-local representation, the
construction-time and post-construction allocation APIs, the
validation-walk shape, the HIR-layer signature anchor, and the
post-construction transformation passes (`MIR_Cleanup`,
`MIR_Cleanup_Virtualize`).

Repository: https://github.com/thepowersgang/mrustc

Master HEAD as of this pass: commit
`7392eca5bd4958cb184fb47cefbd4c7e4f43547b` (committer date
2026-04-13, "standalone_miri - Compile fixes"). Same commit as
stage-{9,10}. All file:line citations refer to that tree.

Primary files inspected:

- `src/mir/mir.hpp` -- IR type definitions (`Function` class,
  `BasicBlock`, `BasicBlockId` typedef, locals vector).
- `src/mir/mir_builder.cpp` -- construction-time allocation
  (`MirBuilder::new_temporary`, `define_variable`,
  `new_bb_unlinked`, `final_cleanup`).
- `src/mir/cleanup.cpp` -- post-construction transformation pass
  (`MirMutator` class, `MIR_Cleanup`, `MIR_Cleanup_Virtualize`,
  `MIR_CleanupCrate`).
- `src/mir/check.cpp` -- per-body validation walk
  (`MIR_Validate`).
- `src/mir/visit_crate_mir.hpp` -- crate-iteration visitor
  (`OuterVisitor`).
- `src/hir/hir.hpp` -- HIR-layer function signature
  (`HIR::Function`, `args_t` typedef).

Every factual claim cites `[file:line]` in the mrustc tree at
this commit. Hypotheses are flagged **HYPOTHESIS:**. Cross-
source synthesis is flagged **INFERENCE:**.

Stage-context anchor: this file does not claim parity with
rustc, rustc_public, or charon -- those are sibling prior-art
passes. mrustc consumes a much older rustc and predates
significant post-MIR work in modern rustc; drift is expected,
and where mrustc made a different design choice that divergence
is itself the evidence relevant to a Rust-to-C peer compiler.

---

## 1. `D-11-1` -- Body field set

`MIR::Function` [`src/mir/mir.hpp:1117-1130`]:

```cpp
class Function
{
public:
    ::std::vector< ::HIR::TypeRef>  locals;
    //::std::vector< RcString>   local_names;
    ::std::vector<bool> drop_flags;

    ::std::vector<BasicBlock>   blocks;

    // Cache filled/used by enumerate
    mutable EnumCachePtr trans_enum_state;
};
```

**Three load-bearing fields plus one transient cache.**
`locals: vector<HIR::TypeRef>` (line 1119), `drop_flags:
vector<bool>` (line 1121), `blocks: vector<BasicBlock>` (line
1123). `trans_enum_state` (line 1126) is a `mutable
EnumCachePtr` reachable for stage-internal enumerate-pass
caching, not part of the body's defining content.

**What is NOT on the body, contra rustc-internal `Body<'tcx>`:**

- No `arg_count` field. Argument count is passed sideband to
  consumers as `HIR::Function::args_t& args` -- see
  `MIR_Validate(... const ::HIR::Function::args_t& args, const
  ::HIR::TypeRef& ret_type)` [`src/mir/check.cpp:445`] and
  `MIR_Cleanup(... const ::HIR::Function::args_t& args, const
  ::HIR::TypeRef& ret_type)` [`src/mir/cleanup.cpp:~1053`].
- No `fn_sig` field. Signature lives at HIR layer
  (`HIR::Function`); the MIR `Function` is the body alone.
- No `span` field on the function itself.
- No `var_debug_info` analog. The commented-out
  `// ::std::vector< RcString> local_names` (line 1120) shows
  this was *considered and rejected* during mrustc's evolution
  -- a strong negative precedent for one of stage-8 D-8-3's
  alternative designs (per-local debug name as a parallel
  array vs. as a `LocalDecl` field). RegseqModel's chosen
  shape (`LocalDecl.debug_name` as a per-decl field, populated
  from rustc's body-level `var_debug_info`) is structurally
  different from mrustc's commented-out parallel-array form.
- No `coroutine`, `phase`, `pass_count`, `tainted_by_errors`
  -- these are rustc-specific bookkeeping with no mrustc
  analog.

**Implication for RegseqModel's `D-11-1` choice surface.** mrustc
demonstrates the *minimum* viable body-level container for a
Rust-to-C compiler: locals (typed) + blocks + drop-flag bits.
Even `arg_count` is omitted, with the partition reconstructed
from sideband HIR data. RegseqModel's candidate (a) (`{
basic_blocks, local_decls, arg_count, fn_sig, span, source }`)
is RICHER than mrustc's; the question is whether the extras
earn their cost. **HYPOTHESIS:** `span` and a Body-Item link
both earn their cost on diagnostic and rendering grounds (and
`fn_sig` may earn its cost only if a downstream consumer reads
the signature as a unit; see §3 D-11-3 below). `arg_count` earns
its cost trivially because rustc's positional-local convention
([0]=ret, [1..arg_count+1]=args, [arg_count+1..]=temps) has no
analog in mrustc's HIR-driven world (mrustc reconstructs args
from `HIR::Function::m_args`).

---

## 2. `D-11-2` -- LocalDecl field set

mrustc's per-local representation is a single `HIR::TypeRef`
[`src/mir/mir.hpp:1119`]. There is no `LocalDecl` struct or
class. The `drop_flags: vector<bool>` parallel array
[`src/mir/mir.hpp:1121`] carries one bit per local (HYPOTHESIS:
or per-drop-site -- the field name is ambiguous and the
construction-time use at `mir_builder.cpp` would clarify it).

**No mutability field per local.** Local mutability is *not
tracked* in mrustc's MIR; HIR-layer `Pattern` matching (the
`pat: Pattern` in each `args_t` entry, see §3 below) carries
binding-mode info, but that's lost by MIR-build time.

**No span field per local.** Spans live at the HIR-`ExprPtr`
level and are accessed through the `Span` argument threaded
through every MIR helper (e.g., `MIR_Cleanup_Virtualize(const
Span& sp, ...)` [`src/mir/cleanup.cpp:548`]).

**No debug name per local.** The commented-out
`// ::std::vector< RcString> local_names` line directly
addresses this: mrustc *considered* per-local naming and
*rejected* it. Codegen-C does not preserve named locals --
emitted C uses positional `_<N>` identifiers throughout
[INFERENCE: based on stage-{6,7,8,9,10} mrustc prior-art
references to `_<N>` rendering in `dump.cpp`].

**Implication for RegseqModel's `D-11-2` choice surface.**
mrustc's choice (single `TypeRef`) is the **lower bound** of
the choice space. RegseqModel's candidate (a)
`{ ty: TyId, mutability, span, debug_name: Option<StringId> }`
adds three fields beyond mrustc:

- `mutability` -- mrustc doesn't track it post-MIR-build;
  RegseqModel might also drop it if no downstream consumer
  reads it. **HYPOTHESIS:** RegseqModel keeps `mutability`
  because C-emit needs `const` qualifiers on parameter
  declarations. Pressure-test in the spec phase by checking
  whether C-emit reads it from `LocalDecl` or recovers it
  from rustc's `LocalDecl.mutability`.
- `span` -- mrustc threads spans sideband; RegseqModel's
  per-local span supports better diagnostics during the
  joint validation walk (D-11-6). The cost is one `Span`
  per LocalDecl.
- `debug_name` -- mrustc rejected this; RegseqModel keeps
  it for stage-8 D-8-3 reasons (pretty-printer readability,
  generated-C readability, debugger usability). Note this
  is a **deliberate divergence** from mrustc.

mrustc's `drop_flags` parallel array has no clean RegseqModel
analog: rustc's `optimized_mir` is post-drop-elaboration, so
explicit drop flags are no longer needed at the source MIR layer
that the bridge consumes. `drop_flags` exists in mrustc because
mrustc constructs MIR from HIR and threads drop-flag bits
through the construction; rustc has internalized that into the
pre-`optimized_mir` pipeline.

---

## 3. `D-11-3` -- FnSig shape

**mrustc keeps function-signature data at the HIR layer, not
the MIR layer.** The MIR `Function` has no signature field
([`mir.hpp:1117-1130`] cited above is exhaustive).

`HIR::Function` [`src/hir/hir.hpp:271-327`]:

```cpp
typedef ::std::vector< ::std::pair< ::HIR::Pattern, ::HIR::TypeRef> >   args_t;

class Function
{
public:
    enum class Receiver { Free, Value, BorrowOwned, BorrowUnique,
                         BorrowShared, Box, Custom, };

    Linkage m_linkage;
    Receiver m_receiver = Receiver::Free;
    RcString m_abi = RcString::new_interned(ABI_RUST);
    bool m_unsafe = false;
    bool m_const = false;
    GenericParams m_params;
    args_t m_args;
    bool m_variadic = false;
    TypeRef m_return;
    ExprPtr m_code;
    struct Markings { /* ... */ } m_markings;
};
```

**Five fields directly relevant to RegseqModel's `FnSig`
question:**

- `m_args: args_t` -- vector of `(Pattern, TypeRef)` pairs.
  Argument types AND patterns. RegseqModel does not carry
  patterns (post-monomorphization; bindings are resolved into
  positional locals).
- `m_return: TypeRef` -- return type.
- `m_abi: RcString` -- ABI as an open-vocabulary string. Note:
  RegseqModel D15 closed the ABI vocabulary to `{Rust, C}`, a
  STRICT subset of mrustc's open vocabulary.
- `m_variadic: bool` -- variadic-argument flag. **This
  validates the `c_variadic` half of D-11-3's alternative
  (b).** mrustc deemed variadic-flag worth carrying.
- `m_unsafe: bool` -- safety marker. mrustc keeps this for ABI
  reporting (e.g., emitting `unsafe extern "C" fn(...)` in
  diagnostic dumps). **HYPOTHESIS:** RegseqModel drops this
  per the C-semantics rule -- C has no `unsafe` distinction,
  and `unsafe` is post-typeck information whose retention
  doesn't change C-emit behavior.

**Sidesteppable as an MIR-layer choice.** mrustc proves a body
*can* function without an MIR-layer `FnSig` field: `MIR_Validate`
and `MIR_Cleanup` both accept `args_t&` and `TypeRef& ret_type`
as sideband parameters [`src/mir/check.cpp:445`,
`src/mir/cleanup.cpp:~1053`]. This is **a positive precedent for
RegseqModel's D-11-3 alternative (d)**: drop body-level `fn_sig`,
reach signature data through `local_decls[0..=arg_count]` (or via
the parent `Item.fn_sig` if RegseqModel keeps a Function-level
signature anchor at stage 12).

**Implication for RegseqModel's `D-11-3` choice surface.** Three
of mrustc's six relevant signature fields validate parts of the
choice space: variadic-flag IN (matches alt (b)), abi as enum
(matches D15 + ABI is one of the structural fields), no-MIR-
layer-fn_sig is workable (matches alt (d)). The fields mrustc
keeps but RegseqModel might drop (`m_unsafe`, `m_receiver`,
`m_linkage`) are rustc-internal-typeck artifacts with no C-emit
relevance. The fields mrustc has but RegseqModel's bridge
discards (`m_params: GenericParams`) reflect post-monomorphization
loss-of-information that is by design.

---

## 4. `D-11-4` -- LocalId typed-index definition

mrustc's local-index typing is **asymmetric**:

- `BasicBlockId` is typedef-typed: `typedef unsigned int
  BasicBlockId;` [`src/mir/mir.hpp:20`].
- `RegionId` is typedef-typed: `typedef unsigned int RegionId;`
  [`src/mir/mir.hpp:19`].
- **Locals are NOT typedef-typed** -- `unsigned int` directly,
  e.g., `MirBuilder::define_variable(unsigned int idx)`
  [`src/mir/mir_builder.cpp:207`].

This asymmetry is **a useful negative precedent for RegseqModel's
D-11-4 question.** mrustc's design implicitly classified
local-indices as "low-importance enough to skip the typedef" and
basic-block-indices as "important enough to typedef." The
asymmetry creates exactly the API drift the typedef would have
prevented: mrustc's signatures use bare `unsigned int` for
locals, allowing accidental cross-mixing with non-local indices.

**Implication for RegseqModel's `D-11-4` choice surface.**
RegseqModel's chosen path (typed `LocalId` via stage-1's
`newtype_index!`, with a const-body arm landed by stage 10)
**closes the asymmetry mrustc lives with**. The cost is a tiny
amount of macro-derived boilerplate; the benefit is type-system
prevention of cross-mixing across the entire stage-11 surface
(LocalId vs BlockId vs FieldIdx vs VariantIdx).

---

## 5. `D-11-5` -- BodyId vs ItemId resolution

**mrustc has no BodyId.** The body is reached as
`HIR::Function::m_code` -- an `ExprPtr` [`src/hir/hir.hpp:286`]
that owns the MIR Function. The HIR `ItemPath` (e.g., the
`HIR::ItemPath p` argument threaded through `MIR_Validate` and
`MIR_Cleanup`) is the sole identity anchor.

The body-fetch pattern is HIR-layer:

```cpp
::HIR::ExprPtr& expr_ptr = /* item.m_code or similar */;
expr_ptr.get_mir_or_error_mut(Span())
    -> ::MIR::Function&
```

(Pattern reconstructed from `MIR_CleanupCrate`
[`src/mir/cleanup.cpp:1236-1246`]:
`MIR_Cleanup(res, p, expr_ptr.get_mir_or_error_mut(Span()),
args, ty)`.)

**One-body-per-item.** Each HIR item that has executable code
(`HIR::Function`, plus statics with initializer expressions and
constants) owns exactly one MIR `Function` body via its
`m_code: ExprPtr`.

**Implication for RegseqModel's `D-11-5` choice surface.** mrustc
validates **option (c)** (no BodyId; bodies live in
`Item.body: Option<Body>`). The composition with Q3 (single-
level IR) and the Item-as-anchor pattern matches mrustc's HIR
layout. **HYPOTHESIS:** option (c) is the right choice for
RegseqModel; the inbound stage-1 expectation can be closed at
"no separate BodyId."

---

## 6. `D-11-6` -- Joint validation walk shape and ownership

`MIR_Validate(const StaticTraitResolve& resolve, const
::HIR::ItemPath& path, const ::MIR::Function& fcn, const
::HIR::Function::args_t& args, const ::HIR::TypeRef& ret_type)`
[`src/mir/check.cpp:445`].

**Three sequential phases:**

1. **Local type sizing pass** [`src/mir/check.cpp:459-473`].
   Iterates `fcn.locals`; verifies each is `Sized` unless the
   type is generic.
2. **CFA reachability sweep** [`src/mir/check.cpp:476-537`].
   Worklist-based DFS from BB0; populates a `visited_bbs`
   bitmap; checks that all reachable terminators are
   non-`TAGDEAD`.
3. **Flat per-statement / per-terminator checks**
   [`src/mir/check.cpp:540-1087`]. Nested `for` loop over
   `fcn.blocks`, `block.statements`, with `TU_MATCH` dispatch on
   statement and terminator tags. Type-equality, argument-arity,
   and condition-type checks live here.

**No visitor class within `MIR_Validate`.** The walk is
direct-loop. A visitor exists at the *crate-iteration* layer
only (`MIR::OuterVisitor` [`src/mir/visit_crate_mir.hpp:13-44`])
which iterates HIR items and dispatches per-function callbacks.
That visitor is at the HIR layer, NOT the MIR-body layer.

**Construction invariants live INLINE in the per-statement
walk.** Local-type lookup, place-type-narrowing, call argument-
arity, return-type matching: all dispatched via `TU_MATCH` in
the same nested loop [`src/mir/check.cpp:540-1087`].

**Implication for RegseqModel's `D-11-6` choice surface.** mrustc
demonstrates that **direct-loop validation works** -- but at the
cost of repeated boilerplate at the per-tag dispatch sites and
no extension point for stage-internal walks (e.g., the joint
DFS-RPO walk per stage-10 §24.4.3 has to be a *separate* walk in
mrustc, not composable with the main validation).

For RegseqModel, the rustc_public-style visitor with `super_*`
defaults is a stronger fit: it composes the deferred Place /
Statement / Terminator walks (D-5-2, D-8-4 Part C, D-9-10 Part C)
with the body-level DFS-RPO walk (stage-10 §24.4.3) into a single
traversal pass via visitor-trait extension. mrustc's direct-loop
shape would force separate walks.

The mrustc precedent is **NEGATIVE** for D-11-6 candidate (a)
(direct method on `Body`) and (c) (analysis-side pass): both
forfeit the visitor's composability. mrustc's experience supports
candidate (b): visitor in `regseq-model`, used by
`RegseqModel::validate`, with stage-internal extension hooks for
the deferred walks.

---

## 7. `D-11-6.1` -- Construction invariant 1 + 9 integration

mrustc folds invariant-style checks (per-local sizing, per-place
type-narrowing, per-call argument-arity, return-type match)
inline into the per-statement walk [`src/mir/check.cpp:540-1087`]
via `TU_MATCH` dispatch. There is **no separate "construction
invariants" pass.**

**Implication.** mrustc validates D-11-6.1 alternative (a)
(closure invariants fold into the per-content visit hooks).
Alternative (b) (separate pre-pass) would duplicate the dispatch
logic mrustc fuses into one walk.

---

## 8. `D-11-7` -- Body-level transformation pass timing

**The most load-bearing mrustc finding for stage 11.**

mrustc's vtable-rewrite is implemented as `MIR_Cleanup_Virtualize`
[`src/mir/cleanup.cpp:548-684`], invoked from the dispatch driver
`MIR_Cleanup` [`src/mir/cleanup.cpp:~1053`]. Pipeline shape:

1. **Construction phase** -- `MirBuilder` builds MIR from HIR
   [`src/mir/mir_builder.cpp`]; `final_cleanup` finishes
   construction [line 74].
2. **Validation phase** -- `MIR_Validate` walks the body
   [`src/mir/check.cpp:445`].
3. **Post-monomorphization cleanup phase** -- `MIR_CleanupCrate`
   [`src/mir/cleanup.cpp:1236-1246`] iterates all functions in
   the crate via `OuterVisitor`, invoking `MIR_Cleanup` followed
   by `MIR_Validate` on each.
4. **Cleanup is gated on `g_is_post_monomorph`**
   [`src/mir/cleanup.cpp:1111`] -- the entry-point flag confirms
   the pass runs after monomorphization is complete.

**`MIR_Cleanup_Virtualize` activation:**

Triggers when a `Call` terminator's function path matches:

- `path.m_data.is_UfcsKnown()` AND
- `path.m_data.as_UfcsKnown().type.data().is_TraitObject()`

[`src/mir/cleanup.cpp:1016-1031`].

**`MIR_Cleanup_Virtualize` operations:**

- Computes `vtable_lv` -- a temporary holding the vtable
  pointer [`src/mir/cleanup.cpp:571`].
- For by-value methods: wraps the receiver in `&move`
  [`src/mir/cleanup.cpp:566-570`].
- Inserts a vtable-metadata-extracting Statement via
  `DstMeta` [`src/mir/cleanup.cpp:682`].
- Rewrites `Call.fcn`: `e.fcn = mv$(tgt_lvalue)`
  [`src/mir/cleanup.cpp:685`] -- the new callee is a
  field-dereference into the loaded vtable at index
  `trait.get_vtable_value_index()` [line 556].

**The single-field rewrite at line 685 is structurally identical
to RegseqModel's stage-9 D-9-3(b) `CallCallee::Indirect(Operand)`
shape.** mrustc's `tgt_lvalue` (a vtable-indexed field-deref
LValue) is the moral equivalent of RegseqModel's `Operand::Move(
Place::from(_t_fn))` per stage-9 expectations §23.3 step 3.

**The pre-Call vtable-load Statements** mrustc inserts (vtable
load at line 571; DstMeta at line 682) are inserted via the
`MirMutator` API (cited in §9 below). This matches RegseqModel's
stage-9 expectations §23.3 step 2 sketch: a chain of
`PtrMetadata`, `Deref`, `Field(virtual_idx, fn_ptr_ty)` synthesis
preceding the Call rewrite.

**Implication for RegseqModel's `D-11-7` choice surface.**

- mrustc validates **option (b)** (post-construction pass) for
  vtable-rewrite specifically. The pass cannot run at MirBuilder
  time because the vtable load requires monomorphization to
  resolve (`get_vtable_value_index` is per-instance).
- mrustc does **NOT speak to** Drop-as-Call timing: mrustc
  constructs from HIR where Drop is a native construct, and its
  drop-glue is generated separately via `enumerate.cpp` /
  `auto_impls.cpp` [INFERENCE: based on stage-9 mrustc prior-art
  citations]. RegseqModel's bridge consumes rustc's
  `optimized_mir` which is post-drop-elaboration, so the
  Drop-elaboration timing question is rustc-side, not body-pass-
  side. **HYPOTHESIS:** D-11-7's hybrid option (c) (Drop-as-Call
  at bridge time + vtable-rewrite as post-construction pass) is
  the right shape, with mrustc's `MIR_Cleanup_Virtualize`
  precedent supporting the post-construction half and
  rustc-internal drop-elaboration discharging the bridge half.
- The **two-phase pipeline** (construct → validate → cleanup →
  re-validate) is itself a useful methodology precedent.
  RegseqModel's stage-11 walks compose with this pattern: the
  joint validation walk runs both before and after the vtable
  rewrite pass, with the second invocation enforcing the post-
  rewrite invariants (stage-9 §23.6.2 B-3 / B-4 / B-6 holding
  post-rewrite).

---

## 9. `D-11-8` -- LocalDecls allocation API

mrustc has **two distinct allocation APIs** for fresh locals,
indexed by the phase of the pipeline:

**Construction-time API: `MirBuilder::new_temporary`**

[`src/mir/mir_builder.cpp:232`]:

```cpp
::MIR::LValue MirBuilder::new_temporary(const ::HIR::TypeRef& ty)
```

The `MirBuilder` holds a reference to the in-progress
`m_output: MIR::Function` [`src/mir/mir_builder.cpp:13`] and
mutates `m_output.locals` directly. State tracking happens via
parallel vectors `m_slot_states` and `m_arg_states`. There is no
freeze step; `final_cleanup` [`src/mir/mir_builder.cpp:74`]
handles terminal-statement and drop-flag rewriting but does not
freeze the Function.

For block creation: `new_bb_unlinked()`
[`src/mir/mir_builder.cpp:389`] returns
`auto rv = m_output.blocks.size();`; `new_bb_linked()`
[`src/mir/mir_builder.cpp:383`] wraps `new_bb_unlinked` plus
linking from the current block.

**Cleanup-time API: `MirMutator`**

[`src/mir/cleanup.cpp:25-100`] (approximate):

```cpp
class MirMutator
{
    ::MIR::Function& m_fcn;
    unsigned int    cur_block;
    unsigned int    cur_stmt;
    mutable ::std::vector< ::MIR::Statement>    new_statements;

public:
    MirMutator(::MIR::Function& fcn, unsigned int bb,
               unsigned int stmt);
    void update_state(::MIR::TypeResolve& state);
    ::MIR::LValue new_temporary(::HIR::TypeRef ty);
    void push_statement(::MIR::Statement stmt);
    ::MIR::LValue in_temporary(::HIR::TypeRef ty, ::MIR::RValue val);
    decltype(new_statements.begin()) flush_stmt();
    void flush_block();
};
```

**Two key observations:**

- **Same `new_temporary` name across both APIs** -- mrustc
  preserves API symmetry between construction-time and
  transformation-time allocation. A call site that needs a
  fresh local says `mutator.new_temporary(ty)` whether the
  context is construction or cleanup.
- **`push_statement` buffers; `flush_stmt`/`flush_block` apply.**
  The `MirMutator` accumulates pending statements in
  `new_statements` and flushes them only at safe points
  (between original statements or at end-of-block) -- this is
  how the vtable-rewrite synthesises pre-Call Statements
  without invalidating in-progress iteration over
  `block.statements`.

**Implication for RegseqModel's `D-11-8` choice surface.**

- mrustc validates **option (a)** (mutable `Body::push_local`)
  during construction, BUT routes mutation through a builder
  (`MirBuilder`) rather than directly on the `Function`. This is
  closer to a hybrid of (a) and (b): a builder-level API that
  mutates the body (a) without freezing (so not pure (b)).
- For post-construction transformations (the vtable-rewrite
  pass), mrustc adds a parallel `MirMutator` API that *also*
  mutates the body but with traversal-aware buffering.
  **HYPOTHESIS:** RegseqModel needs both APIs for the same
  reasons: bridge-time `Body::push_local` (or
  `BodyBuilder::push_local`) for the Drop-as-Call collapse;
  pass-time mutator-style API for the vtable-rewrite. The two
  APIs can share a common trait if the bridge-time call site
  doesn't care about pending-statement buffering.

---

## 10. `D-11-K` -- Categorical rendering deferral to stage 13

mrustc does not defer rendering. `src/mir/dump.cpp` provides
per-statement / per-terminator / per-block rendering
implemented immediately during MIR construction (see
stage-{6,7,8,9,10}/prior-art/mrustc.md for cross-stage references
to `MirDumper::dump_mir` and the `bbN: { ... }` envelope). The
mrustc design philosophy is *interleave implementation with
construction*; RegseqModel's spec-before-implementation discipline
is the opposite.

**Implication for RegseqModel's `D-11-K` choice surface.** mrustc
is **silent** on the categorical deferral question. The
methodology divergence means mrustc's rendering placement is not
a useful precedent. The question of where renderers eventually
land (stage 11 vs stage 13) is a RegseqModel-internal design
choice that mrustc's interleaved approach simply does not
exercise.

---

## Synthesis: which D-tags mrustc evidence resolves cleanly

**Cleanly resolved (mrustc gives strong evidence):**

- `D-11-5` -- "no BodyId" pattern strongly supported. Bodies
  live as `HIR::Function::m_code: ExprPtr` reachable from the
  HIR `ItemPath`. Composes with RegseqModel's `Item.body:
  Option<Body>` design.
- `D-11-7` -- post-construction-pass timing for vtable-rewrite
  validated (`MIR_Cleanup_Virtualize` is the load-bearing
  precedent). Single-field `Call.fcn` rewrite at
  `cleanup.cpp:685` matches RegseqModel's stage-9 D-9-3(b)
  `CallCallee::Indirect(Operand)` shape exactly.
- `D-11-6.1` -- inline-fold of construction invariants into the
  per-content walk validated (mrustc's TU_MATCH dispatch).
- `D-11-3` partial -- variadic-flag earns its cost (mrustc keeps
  it); HIR-layer-only signature shows alternative (d)
  (no body-level FnSig) is workable.

**Resolved with deliberate divergence (mrustc shows a path,
RegseqModel takes a different one):**

- `D-11-2` -- mrustc's bare `vector<TypeRef>` is the lower
  bound; RegseqModel's 4-field `LocalDecl` is richer for the
  named-local-preservation reasons (stage-8 D-8-3) that mrustc
  explicitly rejected (commented-out `local_names` line).
- `D-11-4` -- mrustc's typedef-asymmetry (BlockId typed,
  LocalId untyped) is exactly the API drift typed-index design
  prevents. RegseqModel closes the asymmetry with consistent
  `newtype_index!`.
- `D-11-6` -- mrustc's direct-loop validation forfeits visitor
  composability. RegseqModel's planned `rustc_public`-style
  visitor is a deliberate divergence supported by the
  composability needs of stages 5/8/9/10's deferred walks.

**Open after mrustc evidence (other prior-art needed):**

- `D-11-1` -- mrustc's three-field minimal Body is a lower
  bound; the question of which extras (span, source, fn_sig,
  spread_arg) earn their cost in RegseqModel is not settled by
  mrustc alone. Charon's BodyContents and rustc_public's Body
  schema are the next sources.
- `D-11-3` full -- variadic-flag yes, but the `safety` /
  `c_variadic` / `unsafe` distinction needs cross-source
  triangulation. Charon's FnSig and rustc_public's FnSig are
  the targets.
- `D-11-8` -- two-API split (construction-time + pass-time)
  validated, but the surface shape (`Body::push_local` vs
  `BodyBuilder::push_local` vs traversal-aware Mutator) needs
  Charon and rustc_public cross-checks.
- `D-11-K` -- mrustc silent (interleaved rendering is the
  opposite philosophy).

**Highest-value finding.** `MIR_Cleanup_Virtualize` at
`src/mir/cleanup.cpp:548-684` is the closest existing peer
implementation of the body-level vtable-indirect rewrite pass.
Specifically: (a) timing is post-construction post-monomorph,
(b) the rewrite is single-field on `Call.fcn` matching
RegseqModel's `CallCallee::Indirect(Operand)` choice from
stage-9 D-9-3(b), (c) the pre-Call vtable-load synthesis uses a
traversal-aware `MirMutator` API for safe insertion, and (d) the
pipeline composes with re-validation (cleanup → validate cycle
in `MIR_CleanupCrate`). All four properties transfer directly
to RegseqModel's stage-11 transformation-pass design.
