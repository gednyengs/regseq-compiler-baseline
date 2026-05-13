# Stage 11 -- Prior Art: rustc_public primary source

`rustc_public` (formerly `rustc_smir`, the in-tree home of the
Stable-MIR project) is the rustc-internal crate that defines an
owned, `'tcx`-free, `serde::Serialize`-able mirror of `rustc_middle`'s
MIR. Its purpose is "the WIP public interface to rustc internals...
for third-party tools to invoke" -- documented at the crate root
[`rustc_public/src/lib.rs:1-3,13-17`].

Stages 5-10 already cover the per-construct surfaces (Place,
Operand, Rvalue, Statement, Terminator, BasicBlock); this file does
not re-derive that material. Its scope is the **body-level
container** (`Body`), the **per-local declarations** (`LocalDecl`),
the **typed-index for locals** (`Local`), the **body-level
visitor** (`MirVisitor` / `MutMirVisitor`), the **Body↔Item
linkage** (`Instance::body()`), the **body construction API**
(`Body::new`), the **body-level transformation surface** (which
does not exist in rustc_public), and the **FnSig question**
(D-11-3 alt (d): does the body carry an `fn_sig` field?).

All paths below are relative to the pinned-toolchain root:

```
~/.rustup/toolchains/nightly-2026-02-01-aarch64-apple-darwin/lib/rustlib/rustc-src/rust/compiler/
```

(See `../../rustc-references.md` for the canonical statement of this
root path.)

Every factual claim cites a `[file:line]` in that pinned source.
Hypotheses are flagged as **HYPOTHESIS:**. Cross-source synthesis
across two or more sites is flagged as **INFERENCE:**. Citations to
sibling prior-art files (rustc, mrustc, charon) are out of scope --
this file documents `rustc_public` only. Cross-references to
stage-{9,10}/prior-art/rustc_public.md are made only where their
material is load-bearing for a stage-11 D-tag.

Stage 11's D-tag namespace, restated for self-containment:

- **D-11-1** -- `Body` field set (which fields of rustc's
  `Body<'tcx>` does RegseqModel's `Body` carry).
- **D-11-2** -- `LocalDecl` field set (cardinality + per-field
  choices; named-local preservation question from stage-8 D-8-3).
- **D-11-3** -- `FnSig` shape and **whether the body carries an
  `fn_sig` field at all** (D-11-3 alt (d)).
- **D-11-4** -- `LocalId` typed-index definition (newtype-index
  vs bare-usize; RETURN_PLACE const; niche).
- **D-11-5** -- Body↔Item linkage (`BodyId == ItemId`;
  separate-BodyId; or no-BodyId-bodies-live-in-`Item`).
- **D-11-6** -- joint validation walk shape (visitor trait;
  consumer surface; ownership location).
  - **D-11-6.1** -- construction-invariant 1 + 9 integration
    into the visitor.
- **D-11-7** -- body-level transformation pass timing
  (vtable rewrite + Drop-as-Call: bridge-time vs
  post-construction-pass).
- **D-11-8** -- LocalDecls allocation API (`Body::push_local`
  vs builder pattern vs direct field mutation).
- **D-11-K** -- categorical rendering deferral to stage 13.

---

## 1. `Body` field set: six pub/pub(super) fields, NO `fn_sig`

`rustc_public::Body` is at [`rustc_public/src/mir/body.rs:13-38`]:

```rust
#[derive(Clone, Debug, Serialize)]
pub struct Body {
    pub blocks: Vec<BasicBlock>,

    /// Declarations of locals within the function.
    pub(super) locals: LocalDecls,

    /// The number of arguments this function takes.
    pub(super) arg_count: usize,

    /// Debug information pertaining to user variables, including captures.
    pub var_debug_info: Vec<VarDebugInfo>,

    /// Mark an argument (which must be a tuple) as getting passed as
    /// its individual components. This is used for the "rust-call" ABI
    /// such as closures.
    pub(super) spread_arg: Option<Local>,

    /// The span that covers the entire function body.
    pub span: Span,
}
```

Six fields, not seven. The **load-bearing observation for D-11-3**:

**There is no `fn_sig` field on `Body`**. The signature is not
cached at the body level. Argument-types and return-type are
reached through the `locals: LocalDecls` table via the
positional convention `[0]=return, [1..arg_count+1]=args,
[arg_count+1..]=temps` documented at lines 19-22 and exposed by
the helpers `Body::ret_local`, `Body::arg_locals`,
`Body::inner_locals` at [`body.rs:64-94`]. The full FnSig (with
`abi`, `c_variadic`, `safety`) is reached through a *separate*
query against the function definition: `FnDef::fn_sig() ->
PolyFnSig` at [`rustc_public/src/ty.rs:713-717`], which calls
`self.ty().kind().fn_sig().unwrap()` -- i.e., looks the signature
up from the function's type, not from the body.

**INFERENCE for D-11-3 alt (d):** rustc_public is the
load-bearing precedent for "drop the body-level fn_sig field;
reach signature data through `local_decls[0..=arg_count]` for
input/output Tys plus a separate query for ABI and other
attributes." The design splits responsibility: the Body owns the
local-table (which structurally encodes input/output types via
positional convention), and the function definition / ABI query
owns the binary-interface metadata. RegseqModel can mirror this
exactly, given that stage-1 already reaches function-level
metadata through `Item` (analog of `FnDef`).

The **counter-pressure** stage-1 expectations name is "FnSig is
a structural field of Body (no side-channel lookup) per Q2
eager translation." Two ways to read the divergence:

1. rustc_public is structurally compatible with Q2 (eager
   translation) because the FnSig's *data content* (input Tys
   via local_decls + abi via FnDef) is fully owned and not a
   `'tcx`-bearing query at consumption time -- the `with(|cx|
   ...)` lookup at [`mono.rs:81`] hits a thread-local
   `CompilerCtxt`, which is rustc-side, but the result `FnAbi`
   is owned. RegseqModel's analog: input Tys are owned in the
   `local_decls` table (no `'tcx`); abi can be owned at the
   `Item` level (no `'tcx`). Both fully comply with Q2.
2. The convenience question (one-stop FnSig view at the body
   site vs. multi-stop reconstruction) is independent of Q2
   compliance. It is a UX/composability question.

**For D-11-3, the upshot:** rustc_public's design is admissible
under Q2; RegseqModel can either (a) keep the body-level fn_sig
field per the stage-1 expectation framing, or (b) drop it and
reach signature data through `local_decls` + `Item.abi`. (b) is
the rustc_public pattern; (a) is the rustc_middle pattern.

### 1.1 Drops compared to rustc_middle's `Body<'tcx>`

The kickoff evidence pass already enumerated rustc's `Body<'tcx>`
at [`rustc_middle/src/mir/mod.rs:208-335`] (~17 fields).
rustc_public retains 6 of those (`blocks`, `local_decls` ->
`locals`, `arg_count`, `var_debug_info`, `spread_arg`, `span`)
and drops 11+ (`phase`, `pass_count`, `source`,
`source_scopes`, `coroutine`, `user_type_annotations`,
`required_consts`, `mentioned_items`, `is_polymorphic`,
`injection_phase`, `tainted_by_errors`, plus the coverage
group). This is roughly the same winnowing the kickoff
proposed for RegseqModel's `Body`.

The two interesting differences:

- rustc_public **keeps `var_debug_info` at body level** as a
  `Vec<VarDebugInfo>` field, NOT folded into per-`LocalDecl`
  fields. See §2.
- rustc_public **keeps `spread_arg`** as `Option<Local>`. Stage-3
  D15 locks RegseqModel's `FnAbi` to `{Rust, C}`; the rust-call
  ABI is not in scope. **HYPOTHESIS:** RegseqModel can drop
  `spread_arg` accordingly. If a future stage admits rust-call
  (e.g., for closures), the field re-emerges; until then it is
  unreachable.

### 1.2 What rustc_middle has that rustc_public drops -- relevant to D-11-1

The `source: MirSource<'tcx>` field (rustc_middle line 225)
carries `def_id` + `InstanceKind`-promoted info. **rustc_public
drops it at the Body level**: no Body-internal field links back
to the originating `FnDef` / `Instance`. The linkage is one-way
through `Instance::body()` instead. See §5 below for the D-11-5
treatment.

---

## 2. `LocalDecl` field set: three fields, no `debug_name`

```rust
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct LocalDecl {
    pub ty: Ty,
    pub span: Span,
    pub mutability: Mutability,
}
```

[`rustc_public/src/mir/body.rs:126-131`]

Three fields: `ty`, `span`, `mutability`. All `pub`; no
`pub(super)` encapsulation.

**No `debug_name` field.** The named-local mapping is preserved
*not* by lifting names into per-`LocalDecl` fields, but by
keeping the body-level `var_debug_info: Vec<VarDebugInfo>`
intact (line 29; see §1).

`VarDebugInfo` itself is at lines 710-748:

```rust
pub struct VarDebugInfo {
    pub name: Symbol,
    pub source_info: SourceInfo,
    pub composite: Option<VarDebugInfoFragment>,
    pub value: VarDebugInfoContents,
    pub argument_index: Option<u16>,
}
```

with the helper `VarDebugInfo::local() -> Option<Local>` at
[`body.rs:732-739`]:

```rust
pub fn local(&self) -> Option<Local> {
    match &self.value {
        VarDebugInfoContents::Place(place) if place.projection.is_empty() => Some(place.local),
        VarDebugInfoContents::Place(_) | VarDebugInfoContents::Const(_) => None,
    }
}
```

This is the **projection-empty case**: a `VarDebugInfo` whose
`value` is a `Place` with no projection elements directly names
a single Local; consumers reading the named-local mapping use
this helper to fold `var_debug_info` into a per-Local lookup.

**INFERENCE for D-11-2:** rustc_public's design is **diametric
to stage-8 D-8-3's chosen design** (which folds names into
`LocalDecl.debug_name: Option<StringId>` at construction time).
The two designs are equivalent in expressive power but differ
in *consumer ergonomics*:

- **rustc_public pattern:** body-level `var_debug_info` is
  the canonical store; per-Local lookup is a folding step the
  consumer performs. Preserves the rustc_middle structure
  unchanged.
- **stage-8 D-8-3 pattern:** body-construction-time folding;
  per-`LocalDecl.debug_name` is the canonical store. Loses
  fragment / composite / argument_index information unless
  separately preserved.

The stage-8 rationale (top-level `spec.md` §22.1.1 plus
`stage-8/discussions.md` D-8-3) emphasises C-shape readability:
named locals carry through pretty-printing AND C-backend
identifier emission, so the per-LocalDecl form is more
ergonomic for both consumers. rustc_public's audience
(third-party tools, possibly debug-info-aware) needs the full
VarDebugInfo richness; RegseqModel's audience (Codegen +
analysis) needs only the name. **The divergence is intentional
and load-bearing, not a methodology mismatch.**

For D-11-2 specifically, rustc_public confirms that a 3-field
`LocalDecl { ty, span, mutability }` is the structural floor; the
4th field (`debug_name`) is RegseqModel's add per stage-8 D-8-3.
Stage-11 D-11-2 should explicitly address whether to:

(a) keep VarDebugInfo at body level (rustc_public pattern;
diverges from D-8-3),

(b) fold names into LocalDecl.debug_name and drop body-level
VarDebugInfo entirely (D-8-3 pattern; full divergence from
rustc_public), or

(c) hybrid: fold names into LocalDecl.debug_name AND keep
body-level VarDebugInfo (preserves both consumer ergonomics
and structural richness; cost: one extra field).

**HYPOTHESIS:** stage-8 D-8-3 has already made the call (b);
the stage-11 task is to ratify and integrate it, not re-litigate.
rustc_public's design serves as evidence that (a) is viable, not
that it is preferred.

---

## 3. `Local`: bare `usize` type alias, `RETURN_LOCAL = 0` const

```rust
pub type Local = usize;
pub const RETURN_LOCAL: Local = 0;
```

[`rustc_public/src/mir/body.rs:850-852`]

Compare to rustc_middle's `pub struct Local { const RETURN_PLACE = 0; }`
newtype-index wrapper at [`rustc_middle/src/mir/mod.rs:842-850`]
(per the kickoff evidence pass). rustc_public chose the same
trade-off it made for `BasicBlockIdx` (stage-10 §3): bare type
alias, sacrificing type discipline / `_N` debug format / `Option`
niche optimisation in exchange for serializability and
"third-party-tool friendliness."

The trade-offs were exhaustively analyzed in
`stage-10/prior-art/rustc_public.md` §3 and need not be
re-derived here. The conclusion stands for stage 11: RegseqModel
keeps the rustc_middle pattern (typed `LocalId` newtype-index,
`RETURN_PLACE = 0` const, `Option<LocalId>` niche, `_N` debug
format if rendered) per stage-1's typed-ID convention and stage
10's symmetric `BlockId` decision.

**For D-11-4, rustc_public is silent prior art** -- it does not
support the typed-newtype choice; it merely demonstrates that
the bare-`usize` alternative is feasible at the cost of
discipline and niche.

### 3.1 The `Local` shape's interaction with the visitor

`MirVisitor::visit_local` at [`rustc_public/src/mir/visit.rs:83-85`]
is a no-op default:

```rust
fn visit_local(&mut self, local: &$($mutability)? Local, ptx: PlaceContext, location: Location) {
    let _ = (local, ptx, location);
}
```

The `super_*` chain reaches it through three sites:
- per-`Place` traversal at `super_place` (`visit.rs:476-484` for
  the immutable variant); each place traversal calls
  `visit_local(&place.local, ptx, location)`.
- `super_terminator`'s `Return` arm at lines 235-238: passes
  `RETURN_LOCAL`.
- `super_local_decl` at lines 147-151 receives the `local: Local`
  as the first argument but does not pass it down -- only
  `decl.ty` and `decl.span` get visited.

**For D-11-6**, this is structural evidence that visitor traversal
treats `Local` as a leaf-visited primitive (no destructuring
required); RegseqModel's `LocalId` design can do the same.

---

## 4. Body↔Item linkage: one-way `Instance::body() -> Option<Body>`

There is **no `BodyId` type** in rustc_public. There is **no
`Body.item: ItemId` or `Body.def_id: DefId` back-pointer**. The
linkage is entirely one-way:

```rust
pub fn body(&self) -> Option<Body> {
    with(|context| context.instance_body(self.def))
}
```

[`rustc_public/src/mir/mono.rs:53-55`]

The `Instance` (analog of RegseqModel's `Instance` from
stage-1/stage-9 §9.4) holds a `def: InstanceDef`, and `body()`
performs a `with(|cx| cx.instance_body(def))` lookup that
materialises the `Body` on demand. The `Body` itself does not
record which instance produced it.

`ClosureDef::body()` at [`rustc_public/src/ty.rs:751-755`] and
`CoroutineDef::body()` at lines 763-768 follow the same
pattern: each definition type holds the linkage; `Body` is the
result, owning no back-pointer.

**INFERENCE for D-11-5:** rustc_public is the load-bearing
precedent for **alt (c) "no BodyId at all -- bodies live in
`Item.body: Option<Body>`"**. The pattern is:

- one body per item (the `Option` distinguishes
  body-having-items from body-absent-items per
  `Instance::has_body` at [`mono.rs:66-68`]);
- forward linkage only (Item -> Body);
- no separate ID space for bodies;
- `Instance::body()` is the access point.

This composes cleanly with Q3 (single-level IR) -- no extra
indirection layer; no separate ID table; the `Body` value lives
inside its owning `Item`, and the `Item`'s identity (which is
stage-12's space) IS the body's identity.

**Counter-pressure:** rustc_public's `Body::new` constructor
takes ownership of all the body's components and produces a
fresh `Body` every time `Instance::body()` is called -- there
is no memoisation visible at the public surface. Whether the
underlying `cx.instance_body` memoises is a separate question;
the public type does not. RegseqModel's `Item.body:
Option<Body>` would be by-value-owned (no separate body table),
and the question of "is it cheap to clone" lives at the
serde / data-layout layer -- D-11-1's responsibility, not
D-11-5's.

---

## 5. `MirVisitor` template: super_*/visit_* with exhaustive destructuring

The visitor module is at [`rustc_public/src/mir/visit.rs:1-583`]
(~583 lines). The trait is defined via a macro
`make_mir_visitor!` so a single body produces both immutable
(`MirVisitor`, line 516) and mutable (`MutMirVisitor`, line 517)
variants.

### 5.1 Visit/super hook pairs

The trait has 17 `visit_*` hook pairs (lines 45-133), each
defaulting to its `super_*` counterpart. The hooks cover:

- **Body-level**: `visit_body`, `super_body` (lines 45-47, 135-137)
- **Block-level**: `visit_basic_block`, `super_basic_block`
  (lines 49-51, 139-145)
- **Local-decl-level (three flavours)**: `visit_ret_decl`,
  `visit_arg_decl`, `visit_local_decl` (lines 53-63, 147-159).
  All three default to the same `super_local_decl` body, which
  visits `decl.ty` (and `_ = decl.mutability` per pattern
  `let LocalDecl { ty, span, .. } = decl;` at line 149 -- note
  the `..` here is the documented exception for fields-not-needing-
  visit, see line 35-36).
- **Statement / terminator**: `visit_statement`, `super_statement`
  (lines 65-67, 161-200); `visit_terminator`, `super_terminator`
  (lines 69-71, 202-243). Both take a `Location` second arg.
- **Place / local primitives**: `visit_place`, `visit_local`,
  `super_place`, `super_projection_elem` (lines 77-86,
  476-512). `PlaceContext` is the access-mode marker
  (MUTATING / NON_MUTATING / NON_USE; lines 569-583).
- **Type-level**: `visit_ty`, `super_ty` (lines 99-102,
  308-310). Default no-ops (line 309: `let _ = ty;`) --
  consumers override only if doing per-Ty traversal. **For
  D-11-6.1's construction-invariant 1 (per-field TyId closure),
  this is the natural override site**: a consumer's
  `visit_ty` checks `model.types.contains_id(ty.id())`.
- **Misc**: `visit_span`, `visit_rvalue`, `visit_operand`,
  `visit_user_type_projection`, `visit_const_operand`,
  `visit_mir_const`, `visit_ty_const`, `visit_region`,
  `visit_args`, `visit_assert_msg`, `visit_var_debug_info`
  (lines 73-133 + 245-381).

### 5.2 The "exhaustive destructuring as documentation" idiom

The module preamble at [`visit.rs:23-36`] makes the discipline
binding:

> The code is written in a very deliberate style intended to
> minimize the chance of things being overlooked.
>
> Use pattern matching to reference fields and ensure that all
> matches are exhaustive.
>
> For this to work, ALL MATCHES MUST BE EXHAUSTIVE IN FIELDS AND
> VARIANTS. That means you never write `..` to skip over fields,
> nor do you write `_` to skip over variants in a `match`.
>
> The only place that `_` is acceptable is to match a field (or
> variant argument) that does not require visiting.

Concretely, `super_basic_block` at line 140 destructures
`let BasicBlock { statements, terminator } = bb;` (no `..`, no
`_`). `super_body` (the macro form at lines 410-432) does
`let Body { blocks, locals: _, arg_count, var_debug_info,
spread_arg: _, span } = $body;` (one `_` per documented
exception, no `..`). `super_terminator` at line 203 destructures
`let Terminator { kind, span } = term;`.

**INFERENCE for D-11-6:** RegseqModel's body-level visitor
should adopt this discipline. When a stage adds a Body field
(e.g., via stage-3's S0 framing), the visitor's destructuring
fails to compile until the new field is explicitly handled.
This is the structural enforcement that prevents stage-N
content from silently bypassing stage-N+1's walk machinery.

### 5.3 The `super_body!` macro: `Body` traversal order

The body-level traversal order (immutable variant, lines
409-432):

```
let Body { blocks, locals: _, arg_count, var_debug_info, spread_arg: _, span } = $body;

for bb in blocks {
    $self.visit_basic_block(bb);
}

$self.visit_ret_decl(RETURN_LOCAL, $body.ret_local());

for (idx, arg) in $body.arg_locals().iter().enumerate() {
    $self.visit_arg_decl(idx + 1, arg)
}

let local_start = arg_count + 1;
for (idx, arg) in $body.inner_locals().iter().enumerate() {
    $self.visit_local_decl(idx + local_start, arg)
}

for info in var_debug_info.iter() {
    $self.visit_var_debug_info(info);
}

$self.visit_span(span)
```

Order: blocks first, then ret_decl, then arg_decls (positional
`idx + 1`), then inner_locals (positional `idx + local_start`),
then var_debug_info entries, then the body span.

**INFERENCE for D-11-6:** RegseqModel's joint validation walk
can adopt the same order. Block-level invariants
(stages-5/8/9/10 deferred walks) hit during the `for bb in
blocks` loop. Local-decl invariants (stage-1's construction-
invariant 1 ID-closure for `LocalDecl.ty: TyId`) hit during the
ret/arg/inner local walks. The walk hits each construct once,
in a single linear sweep -- no separate pre-pass needed.

### 5.4 `Location` and `PlaceContext`

`Location(Span)` at [`visit.rs:528-536`] is a single-field
wrapper passed as the second argument to most `visit_*` hooks
(everything below the body level). It carries the source
location of the visited construct. The free functions
`statement_location(body, bb_idx, stmt_idx)` at lines 540-544
and `terminator_location(body, bb_idx)` at lines 548-552 build
`Location`s on demand; the visitor's `super_basic_block` at
lines 142-144 invokes them inline:
`self.visit_statement(stmt, Location(stmt.span))`.

`PlaceContext { is_mut: bool }` at [`visit.rs:569-583`] is the
access-mode marker passed alongside `Place` visits. Three
canonical values: `MUTATING` (write), `NON_MUTATING` (read),
`NON_USE` (e.g., `StorageLive`/`StorageDead` reference, but no
read or write). `is_mut` is private with a `is_mutating()`
accessor for forward-compat (line 571 comment: "Keep this
private so we can increment the type in a backward compatible
manner").

**INFERENCE for D-11-6:** RegseqModel can adopt the same
`PlaceContext` shape (or a richer enum if stage-9's terminator
walks need finer distinctions, e.g., `Move`-vs-`Copy`-aware
context). The privacy idiom is good design hygiene. The
load-bearing point: every `Place` visit carries its access
context, which is what stage-5 D-5-2's type-coherence walk
needs to dispatch on.

### 5.5 Construction invariant 1 + 9 integration: D-11-6.1

The visitor surfaces natural override sites for both:

- **Invariant 1 (TyId closure for `LocalDecl.ty` and any other
  body-side TyIds):** override `visit_ty` to check
  `model.types.contains(ty.id())`. The `super_local_decl`
  default already calls `self.visit_ty(ty, ...)` (line 150),
  so a single override hits all per-`LocalDecl` ty fields plus
  any embedded `Field(_, ty)` projection-elems
  (`super_projection_elem` at line 505 calls `visit_ty`),
  Cast-target tys (`super_rvalue::Cast` arm at line 264-266),
  Aggregate-Closure tys, etc. **One override; full closure.**
- **Invariant 9 (every Direct InstanceId resolves):** override
  `visit_terminator` (or a custom `visit_call_callee` if the
  RegseqModel visitor extends the rustc_public surface to
  expose CallCallee separately). The `super_terminator::Call`
  arm at lines 217-223 visits `func`, args, destination; a
  RegseqModel extension that destructures
  `CallCallee::Direct(id)` at this point checks
  `model.instances.contains(id)`. **One override; full
  closure.**

**INFERENCE for D-11-6.1:** rustc_public confirms that visitor-
override is the natural integration site for closure
invariants. No separate "walk-once-to-classify" pre-pass is
needed; the single body-level walk subsumes both content
validation and ID-closure validation.

---

## 6. `Body::new` construction API

```rust
pub fn new(
    blocks: Vec<BasicBlock>,
    locals: LocalDecls,
    arg_count: usize,
    var_debug_info: Vec<VarDebugInfo>,
    spread_arg: Option<Local>,
    span: Span,
) -> Self {
    assert!(
        locals.len() > arg_count,
        "A Body must contain at least a local for the return value and each of the function's arguments"
    );
    Self { blocks, locals, arg_count, var_debug_info, spread_arg, span }
}
```

[`rustc_public/src/mir/body.rs:42-62`]

This is the **single constructor** for `Body` outside the
crate; the `pub(super) locals` and `pub(super) arg_count`
fields force every external caller through this entry point.

The constructor:
- consumes complete `Vec` collections for `blocks`, `locals`,
  `var_debug_info` -- no incremental push API at the public
  surface;
- asserts only `locals.len() > arg_count`;
- enforces nothing about blocks (the cross-stage analog of
  stage-10 §5).

**Crucially: there is no `Body::push_local`, no
`Body::push_block`, no `BodyBuilder` type, no incremental
mutator at the public surface.** Mutation of an existing body
is private (see `ret_local_mut` at line 81, marked
`pub(crate)`); external consumers receive immutable bodies.

**INFERENCE for D-11-8:** rustc_public's design is
**construct-once-from-complete-data**. This pattern works for
rustc_public because the bridge translator produces the full
body in one pass from rustc-internal state -- there is no
post-construction mutation phase.

For RegseqModel, the question is whether stage-9's transformation
passes (Drop-as-Call collapse and the body-level vtable-rewrite
pass per D-9-2 / D-9-3) need an incremental allocation API:

- **Drop-as-Call collapse** synthesises two fresh LocalDecls
  per Drop site (`_t_drop_arg` and `_t_drop_ret` per stage-9
  expectations.md "From stage 9: Drop-as-Call bridge concerns:
  fresh LocalIds"). If the collapse runs at bridge-construction
  time (per the stage-9 entry's framing), the construction
  pass simply pushes onto a working `Vec<LocalDecl>` before
  finalising the Body -- no API needed. **rustc_public's
  pattern fits.**
- **Vtable-rewrite pass** synthesises 4-5 fresh LocalDecls
  per dyn-Trait Call site (`_t_recv`, `_t_meta`, `_t_fn`,
  `_t_data`, optionally `_t_raw` per stage-9 expectations.md).
  If it runs at bridge-construction time, same pattern. If it
  runs as a post-construction pass, the pass needs a way to
  append to `body.locals` and to mutate
  `body.blocks[*].statements` and `body.blocks[*].terminator`.
  **rustc_public's pattern does NOT fit a post-construction
  pass design.**

D-11-7 (transformation pass timing) drives D-11-8. If
D-11-7 lands at "all transformations at bridge time" (analog to
the rustc_public pattern), D-11-8 lands at "construct-once,
no incremental API." If D-11-7 lands at "vtable-rewrite is a
post-construction pass" (analog to Charon's
`transform_dyn_trait_calls`), D-11-8 lands at "builder pattern
or push-on-mutable-Body API."

**HYPOTHESIS:** the cleanest split is bridge-time for
Drop-as-Call (per stage-9 §23.4 step 4 framing) and
post-construction for vtable-rewrite (the pass needs the full
Body to walk all Calls and read their resolved Instances --
a property only available after the bridge populates
`model.instances`). Under that split, RegseqModel needs **both**
construction-time push (for the bridge) AND a builder
pattern or limited mutation API (for the rewrite pass).

---

## 7. Body-level transformation passes: rustc_public is silent

rustc_public has **no body-level transformation pass surface**.
It exposes the immutable `MirVisitor` and the `MutMirVisitor`
that allows mutation in-place, but no pre-defined transformation
passes (no equivalent of Charon's
`transform_dyn_trait_calls.rs`, no equivalent of mrustc's
`MIR_Cleanup_Virtualize`).

This is consistent with rustc_public's mission ("the WIP public
interface to rustc internals... for third-party tools to invoke"
at [`lib.rs:13-17`]): it provides the *data*, not the
*transformations*. Tools that want to rewrite bodies do so
themselves using `MutMirVisitor` as the traversal infrastructure.

**For D-11-7, rustc_public is silent prior art** -- it does not
take a position on bridge-time vs post-construction-pass
timing because it does not perform either kind of
transformation. The decision must be informed by Charon and
mrustc (where prior-art work is in progress in sibling files
this round) and by RegseqModel's own constraints (Q2 eager
translation; bridge ownership; analysis-side separation per
Q1).

What rustc_public *does* validate: the `MutMirVisitor` /
`MirVisitor` shape is sufficient infrastructure for either
timing choice. A bridge-time transformation runs while
populating per-block content; a post-construction
transformation runs through `MutMirVisitor`. RegseqModel can
keep either option open without changing the visitor design.

---

## 8. Pretty-printing: same crate, no deferral pattern

`rustc_public::mir::pretty::function_body` at
[`rustc_public/src/mir/pretty.rs:38-88`] is the body-level
pretty-printer. It lives in the same crate as the body schema;
there is no "deferral" pattern (no stub returning
`unimplemented!`; no later-stage population step).

`Body::dump(w, fn_name)` at [`body.rs:114-117`] is the public
entry point that delegates to `function_body`.

**For D-11-K, rustc_public is structurally distant prior art.**
RegseqModel's stage-11→stage-13 deferral is a stage-management
choice (which stage owns which renderer), not a structural
property the pretty-printer exposes. rustc_public's
pretty-printer is finished and shipped; RegseqModel's renderers
are in progress with deferral cascading from stage 5 onward.

The structural shape of `function_body` is interesting for
*stage 13* (which will eventually own the renderer
implementations), but is not load-bearing for the **D-11-K
deferral decision itself** (which is about whether stage 11
hosts renderer work or stage 13 does).

---

## 9. Summary: rustc_public evidence per D-11 tag

| D-tag | rustc_public says | Implication for RegseqModel |
|---|---|---|
| **D-11-1** | Six `pub`/`pub(super)` fields: `{ blocks, locals, arg_count, var_debug_info, spread_arg, span }` [`body.rs:13-38`]. NO `fn_sig` field; NO `source` / Body↔Item back-pointer. | Mirror the slimming. The fn_sig question (D-11-3) is independent (rustc_public drops it; rustc_middle keeps it). Stage-1 expectations frame fn_sig as a structural Body field, but rustc_public is the precedent that the alternative is admissible. |
| **D-11-2** | Three fields: `{ ty, span, mutability }` [`body.rs:126-131`]. NO per-LocalDecl `debug_name`; named-local mapping preserved at body-level via `var_debug_info: Vec<VarDebugInfo>` plus `VarDebugInfo::local()` helper [lines 732-739]. | Three fields is the structural floor; stage-8 D-8-3 adds `debug_name` per RegseqModel C-shape ergonomics. The divergence from rustc_public is intentional and load-bearing. |
| **D-11-3 alt (d)** | NO body-level `fn_sig` field. Signature data reached through `local_decls[0..=arg_count]` (input/output Tys) and `FnDef::fn_sig() -> PolyFnSig` [`ty.rs:713-717`] (full FnSig with abi, c_variadic, safety). | Confirms alt (d) is admissible under Q2 eager translation. The choice between (a) "body-level fn_sig" and (d) "no body-level fn_sig" is a UX/composability question, not a methodology question. Stage-11 must pick one. |
| **D-11-4** | `pub type Local = usize` [`body.rs:850`]. Bare alias; same trade-off as `BasicBlockIdx` at `body.rs:40` (analyzed in stage-10 prior-art §3): sacrifices type discipline / `_N` debug format / niche for serializability. | Silent prior art -- demonstrates the bare-usize alternative works. RegseqModel keeps the typed-newtype pattern per stage-1 + stage-10 precedent. |
| **D-11-5 alt (c)** | NO `BodyId` type. NO `Body.item` / `Body.def_id` back-pointer. Linkage is one-way through `Instance::body() -> Option<Body>` [`mono.rs:53-55`]. Same pattern for `ClosureDef::body()` [`ty.rs:751-755`] and `CoroutineDef::body()` [lines 763-768]. | Load-bearing precedent for D-11-5 alt (c) "no BodyId; bodies live in `Item.body: Option<Body>`". One body per item; forward linkage only. Composes with Q3 (single-level IR). |
| **D-11-6** | `MirVisitor` / `MutMirVisitor` traits via `make_mir_visitor!` macro [`visit.rs:42-517`]. 17 `visit_*`/`super_*` hook pairs. Exhaustive-destructuring discipline (no `..`, no catch-all `_`) per preamble [lines 23-36]. `Location(Span)` and `PlaceContext { is_mut }` shapes [lines 528-583]. | Load-bearing template. RegseqModel adopts the macro-generated dual-variant trait, exhaustive destructuring discipline, `Location` wrapper, and `PlaceContext`. The walk hits each construct once in linear order: blocks, ret_decl, arg_decls, inner locals, var_debug_info, span. |
| **D-11-6.1** | `visit_ty` is the natural override site for invariant 1 (per-field TyId closure); `visit_terminator::Call` arm is the natural override site for invariant 9 (Direct(InstanceId) closure). Both reached by the single body-level walk; no separate pre-pass needed. | Confirms visitor-override approach (no pre-pass required). Single override per invariant; both fold into the joint walk D-11-6 lands. |
| **D-11-7** | NO transformation passes at the rustc_public layer. `MutMirVisitor` provides infrastructure for in-place rewrite; rustc_public itself does no rewriting. | Silent prior art -- rustc_public takes no position on bridge-time vs post-construction-pass timing. Decision must be informed by Charon and mrustc (the "reference rewrite pass" cases). |
| **D-11-8** | Single `Body::new` constructor [`body.rs:47-62`] consuming complete `Vec` collections; `pub(super)` locals/arg_count force entry through `new`. NO incremental `push_local` / `BodyBuilder`. Mutation is `pub(crate)`. | Construct-once-from-complete-data is the rustc_public pattern. Fits bridge-time transformations (per D-11-7 (a) or hybrid (c) Drop-as-Call). Does NOT fit post-construction-pass transformations -- if D-11-7 lands at hybrid (c) with vtable-rewrite as post-construction, RegseqModel needs an incremental API beyond rustc_public's pattern. |
| **D-11-K** | Pretty-printer in same crate; no deferral pattern; `Body::dump(w, fn_name)` is the public entry point. | Structurally distant. The D-11-K decision (stage 11 vs stage 13 hosting renderer work) is a stage-management choice rustc_public's design does not constrain. |

---

## 10. Open questions surfaced

- **D-11-2 phrasing:** does the spec call `LocalDecl.debug_name`
  the canonical store with body-level VarDebugInfo dropped (full
  D-8-3), or is body-level VarDebugInfo retained as a richer
  store (rustc_public hybrid)? The kickoff evidence pass already
  preferred the D-8-3 design; this prior-art read confirms it
  is a divergence from rustc_public, not an inheritance.
- **D-11-3:** which alt -- (a) body-level fn_sig field per
  stage-1 framing, or (d) no body-level fn_sig per rustc_public
  pattern -- does stage 11 pick? Both are admissible under Q2.
  This is the **single most surfaced choice** in this prior-art
  read.
- **D-11-7 ↔ D-11-8 coupling:** the timing decision drives the
  allocation-API decision. rustc_public confirms construct-once
  works for bridge-time transformations; whether it suffices
  for RegseqModel depends on whether the vtable-rewrite pass
  is bridge-time (works) or post-construction (does not work
  with rustc_public's pattern; needs builder or mutator API).
