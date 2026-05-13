# Stage 6 -- Prior Art: Charon primary source

Primary-source Charon (LLBC / ULLBC) evidence for `Operand`,
`ConstantExpr`, and the related translation-pass behaviour
governing the post-MIR shape of operands. Scoped to what
stage 6's decision cluster (D-6-1 variant set, D-6-2 storage /
interning, D-6-3 invariants, D-6-K render deferral) needs.
Fresh pass per `../../README.md` §2 methodology rule (do not
reuse stage-0 summaries).

Charon is INRIA's Rust front-end translator that feeds the
Aeneas verification pipeline by lowering rustc MIR (via the
`hax` extraction layer) into LLBC ("Low-Level Borrow
Calculus") and the more MIR-shaped intermediate ULLBC
("Unstructured LLBC"). LLBC is the headline IR named in the
ICFP 2022 paper "Aeneas: Rust Verification by Functional
Translation" (Ho & Protzenko); ULLBC is the basic-block-shaped
predecessor produced before control-flow reconstruction.

All paths below are relative to the Charon repository root:

```
https://github.com/AeneasVerif/charon (main branch)
```

Every factual claim cites a `[file:line]` (or `[file:Lstart-Lend]`)
in the Charon source tree at `main`. Where the Aeneas paper
contributes a rationale that the source does not fully spell
out, the reference is to the arxiv preprint (2206.07185).
Hypotheses are flagged. Inferences across two sites are
labelled INFERENCE.

---

## 1. `Operand` enum shape

`Operand` is a 3-variant closed enum in Charon's expression
AST [`charon/src/ast/expressions.rs:364-370`]:

```rust
pub enum Operand {
    Copy(Place),
    Move(Place),
    /// Constant value (including constant and static variables)
    #[charon::rename("Constant")]
    Const(Box<ConstantExpr>),
}
```

Key facts:

- **3 variants, closed.** No `RuntimeChecks` arm; no `Other`
  / `Opaque` arm.
- **Copy and Move are structurally identical** (both wrap a
  bare `Place`).
- **The Constant variant boxes a `ConstantExpr`**, exactly
  paralleling rustc's `Box<ConstOperand<'tcx>>` indirection.
  The serde rename `#[charon::rename("Constant")]` keeps the
  on-disk variant name aligned with rustc terminology even
  though the in-Rust ident is `Const`.
- **Operand is unified across LLBC and ULLBC.** ULLBC
  (basic-block form) and LLBC (structured-control-flow form)
  both `pub use super::expressions::*`
  [`charon/src/ast/ullbc_ast.rs:3`] and [`llbc_ast.rs:10`],
  so the operand vocabulary does not change between the two
  IR levels -- only statement / terminator layout does.

`Operand` is *not* `HashConsable` and is not wrapped in
`HashConsed<T>` anywhere in the AST surface (Section 7).

---

## 2. `ConstantExpr` wrapper

The constant payload carried by `Operand::Const`
[`charon/src/ast/expressions.rs:678-683`]:

```rust
pub struct ConstantExpr {
    pub kind: ConstantExprKind,
    pub ty: Ty,
}
```

Two-field struct: the kind (a tagged union of 13 variants;
see Section 3) and the constant's type.

Notable contrasts with rustc's `ConstOperand<'tcx>`:

- **No `span` field on `ConstantExpr`.** Source location
  lives one level up, on the `Statement` /
  `Terminator` wrapper (`statement.span`, `terminator.span`)
  at [`charon/src/ast/ullbc_ast.rs:67-75`,
  `:154-162`] and [`llbc_ast.rs:90-102`]. INFERENCE: the
  use-site source location is recoverable from the
  containing statement; per-operand span is not preserved.
- **No `user_ty` ascription field.** Type ascriptions for
  `collect::<Vec<_>>()` etc. (NLL-only metadata in rustc)
  are dropped at the `hax` -> Charon boundary.
- **The `ty` field is preserved** -- LLBC carries a typed
  constant, mirroring the typed Place (Section 8).

The wrapper is itself wrapped in `Box<...>` at the `Operand`
level. Same memory-layout rationale as rustc:
keep the parent `Operand` enum compact.

---

## 3. `ConstantExprKind` -- the 13-variant constant union

The constant payload's tagged union
[`charon/src/ast/expressions.rs:604-676`]:

```rust
pub enum ConstantExprKind {
    Literal(Literal),
    Adt(Option<VariantId>, Vec<ConstantExpr>),
    Array(Vec<ConstantExpr>),
    Global(GlobalDeclRef),                            // eliminated in micro-pass
    TraitConst(TraitRef, TraitItemName),
    VTableRef(TraitRef),
    Ref(Box<ConstantExpr>, Option<UnsizingMetadata>), // eliminated in micro-pass
    Ptr(RefKind, Box<ConstantExpr>, Option<UnsizingMetadata>),  // eliminated
    Var(ConstGenericDbVar),
    FnDef(FnPtr),
    FnPtr(FnPtr),                                     // eliminated in micro-pass
    PtrNoProvenance(u128),                            // eliminated in micro-pass
    RawMemory(Vec<Byte>),
    Opaque(String),
}
```

Several variants carry the comment "we eliminate this case
in a micro-pass" -- referring to the
`simplify_constants` pass (Section 6) which extracts
non-`Literal` constants into separate let-bindings.

INFERENCE: post-`simplify_constants`, the LLBC constant
payload reduces to a small subset (predominantly `Literal`,
plus residual `Var` / `FnDef` / `Opaque` / `RawMemory`
/ etc. that are not extracted). The downstream verification
target (Aeneas) consumes a normalised operand vocabulary
where most heterogeneity has been pushed into Place loads.

---

## 4. No `RuntimeChecks` operand variant -- lowered during translation

Charon does *not* preserve rustc's `Operand::RuntimeChecks`
variant. The translation function maps it into a different
construct [`charon/src/bin/charon-driver/translate/translate_bodies.rs:533-588`]:

```rust
fn translate_operand(
    &mut self,
    span: Span,
    operand: &mir::Operand<'tcx>,
) -> Result<Operand, Error> {
    Ok(match operand {
        mir::Operand::Copy(place) => { ... Operand::Copy(p) }
        mir::Operand::Move(place) => { ... Operand::Move(p) }
        mir::Operand::Constant(const_op) => { ... }
        mir::Operand::RuntimeChecks(check) => {
            let op = match check {
                mir::RuntimeChecks::UbChecks => NullOp::UbChecks,
                mir::RuntimeChecks::OverflowChecks => NullOp::OverflowChecks,
                mir::RuntimeChecks::ContractChecks => NullOp::ContractChecks,
            };
            let local = self.locals.new_var(None, Ty::mk_bool());
            self.statements.push(Statement {
                span, kind: StatementKind::StorageLive(local.as_local().unwrap()),
                comments_before: vec![],
            });
            self.statements.push(Statement {
                span, kind: StatementKind::Assign(
                    local.clone(),
                    Rvalue::NullaryOp(op, Ty::mk_bool()),
                ),
                comments_before: vec![],
            });
            Operand::Move(local)
        }
    })
}
```

What happens: when rustc hands Charon an
`Operand::RuntimeChecks(check)`, the translator
**materialises a fresh `bool` local**, emits a `StorageLive`
plus an `Assign(local, Rvalue::NullaryOp(op, bool))`
statement, and **returns `Operand::Move(local)`** as the
operand at the original use site. The three runtime-check
flavours become `NullOp::UbChecks` / `OverflowChecks` /
`ContractChecks` at the rvalue level
[`charon/src/ast/expressions.rs:197-204`]:

```rust
pub enum NullOp {
    SizeOf,
    AlignOf,
    OffsetOf(TypeDeclRef, Option<VariantId>, FieldId),
    UbChecks,
    OverflowChecks,
    ContractChecks,
}
```

Implications:

- **Runtime-check queries survive into LLBC**, but at the
  rvalue level (`Rvalue::NullaryOp`), not the operand level.
- **The operand grammar is simpler in Charon (3 variants)**
  precisely because the 4th rustc variant gets pushed down
  one level.
- **Charon does not reject** the construct -- it lowers it
  to the rvalue level (`Rvalue::NullaryOp` + `Move(local)`).
  RegseqModel's D-6-1.2 decision lowers the construct as
  well, but more aggressively at the operand level (directly
  to `Operand::Constant(ConstId)` referencing a
  `Const::Bool(b)` resolved at bridge time). The two
  designs converge on "do not reject" but diverge on the
  lowering aggressiveness gradient (rvalue-level for Charon's
  verification IR; operand-level for RegseqModel's codegen IR).

---

## 5. `codegen_operand`-equivalent: where operands are consumed

Charon does not have a "codegen" pass in the rustc sense --
it produces serialised LLBC, which is then consumed by
Aeneas. The closest analog is the rendering /
serialisation surface plus the verification-target
extraction. The operand-consumer call chain in Charon
itself is therefore:

- **Translation** (rustc MIR -> ULLBC): see Section 4.
- **Micro-passes on ULLBC** (e.g., `simplify_constants`,
  Section 6): canonicalise operand payloads.
- **Control-flow reconstruction** (ULLBC -> LLBC):
  preserves operand vocabulary unchanged.
- **Pretty-printing for human consumption**:
  Section 9.
- **Serialisation to JSON for Aeneas consumption**: the
  derive-`SerializeState` / `DeserializeState` machinery
  in the AST modules.

There is no "evaluate operand to backend value" step in
Charon analogous to rustc's `codegen_operand`. The whole
LLBC body is shipped to the verification framework as data,
and Aeneas does the semantic work in a separate pass.

---

## 6. `simplify_constants` micro-pass -- the operand-payload normalisation

The `simplify_constants` pass
[`charon/src/transform/simplify_output/simplify_constants.rs:1-12`,
header doc comment]:

```text
//! The MIR constant expressions lead to a lot of duplication: there are
//! for instance constant ADTs which duplicate the "regular" aggregated
//! ADTs in the operands, constant references, etc. This reduces the number
//! of cases to handle and eases the function translation in Aeneas.
//!
//! This pass removes all those occurrences so that only the
//! `ConstantExpression::Literal`. It does so by introducing intermediate statements.
```

What the pass does (header doc comment):

- For `ConstantExprKind::Adt(...)`, `Array(...)`, `Ref(...)`,
  `Ptr(...)`, `Global(...)`, `PtrNoProvenance(...)`,
  `FnPtr(...)`: introduce intermediate let-bindings that
  build the value via `Rvalue::Aggregate` /
  `Rvalue::Ref` / `Rvalue::Use` / etc., and replace the
  original `Operand::Const(non-literal)` with
  `Operand::Copy(local)` or `Operand::Move(local)` where
  `local` is the intermediate.
- After the pass, the *only* `ConstantExprKind` arms allowed
  in `Operand::Const` are `Literal`, plus residual cases the
  pass explicitly does not extract (`Var`, `FnDef`,
  `RawMemory`, `Opaque`, `TraitConst`, `VTableRef` --
  observed by inspecting the pass's match arms,
  not exhaustively cited).

**INFERENCE for D-6-3:** post-`simplify_constants`, Charon
enforces a strong post-pass invariant: most LLBC
`Operand::Const(...)` payloads are `Literal`. This is a
**construction-time invariant**, not a verification-time
one. RegseqModel's stage-4 `Const` enum already lives at the
post-evaluation layer (no `Unevaluated` arm), so the
analogous invariant comes "for free" via stage-4's design
rather than via a stage-6 pass.

---

## 7. No operand-level interning

Charon implements a generic hash-consing infrastructure
[`charon/src/ast/hash_cons.rs:14-20`]:

```rust
pub struct HashConsed<T>(HashByAddr<Arc<T>>);

pub trait HashConsable = Hash + PartialEq + Eq + Clone + Mappable;
```

`HashConsed<T>` wraps an `Arc<T>` whose pointer address is
the hash key, guaranteeing that two equal values share the
same allocation. The underlying interning table is a global
`LazyLock<RwLock<TypeMap<InternMapper>>>`
[`charon/src/ast/hash_cons.rs:69-75`].

Direct grep for `HashConsed<` against `expressions.rs` and
`values.rs` (the modules carrying `Operand`, `Place`,
`ConstantExpr`, `Literal`) finds **zero occurrences**.
INFERENCE (high confidence): Charon does not hash-cons
operands, places, or constant expressions; the hash-consing
machinery is reserved for type-level structures (`Ty` and
related) and a small set of bridge-side handles in
`charon-driver`. Operand storage at use sites is fully
inline; only the `Box<ConstantExpr>` indirection on
`Operand::Const` exists for memory-layout reasons.

This matches RegseqModel's current D-6-2 leaning (no
operand interning).

---

## 8. `Place` shape -- typed, recursive, NOT identical to rustc or RegseqModel

For context, since `Operand::Copy(Place)` /
`Operand::Move(Place)` carry `Place` directly,
[`charon/src/ast/expressions.rs:10-39`]:

```rust
pub struct Place {
    pub kind: PlaceKind,
    pub ty: Ty,
}

pub enum PlaceKind {
    Local(LocalId),
    Projection(Box<Place>, ProjectionElem),
    Global(GlobalDeclRef),
}
```

Structural notes (relevant context, not a stage-6 question
itself):

- **`Place` carries inline `ty`.** Charon stores the
  resolved type at every Place node. RegseqModel's stage-5
  `Place` does not carry a `ty` field at the head, but each
  `PlaceElem::Field(FieldIdx, TyId)` carries the post-narrow
  type at the projection site -- a different placement of
  the same information.
- **Projections are recursive (`Box<Place>`)**, not flat
  slices. Each `Projection` step wraps an inner `Place`, so
  `((local_3.f).g)[i]` becomes a 4-deep nested
  `PlaceKind::Projection(Box<...>, ...)` chain, not a
  `(local, [Field, Field, Index])` flat representation.
  rustc and RegseqModel use the flat form
  [`rustc_middle/src/mir/syntax.rs:1188-1194`,
  RegseqModel `spec.md` §19.1].
- **`PlaceKind::Global(GlobalDeclRef)`** is a Charon
  invention introduced by the `simplify_constants` pass --
  not present in rustc MIR. It lets a Place refer to a
  top-level `const` / `static` directly without going
  through a constant operand.
- **`ProjectionElem::Index { offset: Box<Operand>, from_end }`**
  [`charon/src/ast/expressions.rs:81-86`] -- Charon's index
  carries an `Operand` (not a `LocalId` as in stage 5 of
  RegseqModel). This means Charon's `Place` and `Operand`
  are mutually recursive. RegseqModel deliberately broke
  this cycle at stage 5 by having `PlaceElem::Index(LocalId)`
  -- see RegseqModel `spec.md` line 5294-5298
  ("`Index` carries a `LocalId`, not an `Operand` -- so
  `Place` has no circular dependency on stage-6 `Operand`").
  INFERENCE: Charon accepts the cycle and pays for it with
  recursive box ownership.
- **Charon-internal `Subslice { from: Box<Operand>, to: Box<Operand>, from_end: bool }`**
  [`charon/src/ast/expressions.rs:90-95`] -- same Operand-
  cycling design as `Index`. Note the comment "We
  **eliminate** this variant in a micro-pass for LLBC" on
  both `Index` and `Subslice` -- post-pass, ULLBC's variants
  with `Box<Operand>` get reduced (likely to function calls,
  per the `index_to_function_calls` pass listed in the
  `simplify_output` directory).

The `Place` shape difference does not bear directly on
stage 6's operand-variant decisions. It does mean
"`Operand::Copy(Place)`" carries different concrete content
between Charon and RegseqModel, but the variant-shape choice
(carry `Place` by value) is shared.

---

## 9. Pretty-printing -- Copy / Move / Const are textually distinct

Operand pretty-printer
[`charon/src/pretty/fmt_with_ctx.rs:1141-1147`]:

```rust
impl<C: AstFormatter> FmtWithCtx<C> for Operand {
    fn fmt_with_ctx(&self, ctx: &C, f: &mut Formatter<'_>) -> Result {
        match self {
            Operand::Copy(p) => write!(f, "copy {}", p.with_ctx(ctx)),
            Operand::Move(p) => write!(f, "move {}", p.with_ctx(ctx)),
            Operand::Const(c) => write!(f, "const {}", c.with_ctx(ctx)),
        }
    }
}
```

Key observations:

- **All three variants are textually distinct**. The
  `copy` / `move` / `const` keyword prefixes preserve the
  Copy/Move distinction at every render site.
- **No specialisation for empty / trivial cases.** Even a
  bare local-load gets the full `copy <local>` rendering;
  there is no operand-level shortcut.
- **The constant variant prints inline**, not by reference,
  because operands are not hash-consed (Section 7) -- there
  is no operand-level ID to reference.

This is one valid stylistic point for RegseqModel's
deferred D-6-K stage-11 `render_operand`: the keyword-prefix
pattern is well-established prior art and works without
sigils or symbol-heavy syntax.

---

## 10. Cross-stage anchors (in Charon's own structure)

`Operand` references content pinned in adjacent Charon
modules:

- **`Place`**: defined in
  [`charon/src/ast/expressions.rs:12-15`], in the same file.
  Stage 6's analog (`Operand`) and stage 5's analog
  (`Place`) live in the same Charon module; in RegseqModel
  they live in separate per-stage spec sections but have
  the same logical relationship.
- **`Literal`**: defined in
  [`charon/src/ast/values.rs:41-52`]. Sub-variants
  `Scalar(ScalarValue)`, `Float(FloatValue)`, `Bool(bool)`,
  `Char(char)`, `ByteStr(Vec<u8>)`, `Str(String)` --
  partially overlapping with RegseqModel stage-4's `Const`
  variants (which has Bool / Char / Int / Uint / Float / ZST /
  Bytes / Aggregate / EnumValue / NullPtr / Reloc).
- **`Ty`**: defined in `charon/src/ast/types.rs` (not
  fetched in this pass; cited by file path only).

Charon does not have stage-1 / stage-12 / stage-9
analogs at the same granularity as RegseqModel. The crate-
level interning happens at `Krate` / `TranslatedCrate` in
`krate.rs`. Operand-reachable cross-stage edges in
RegseqModel terms: stage 5 (`Place`), stage 4 (`Const`).

---

## 11. Cross-D-tag findings

Mapping of Charon evidence to D-6-K decisions:

### D-6-1 (variant set + payload shapes)

- **Charon's operand has 3 variants** (no `RuntimeChecks` at
  the operand level) -- matches RegseqModel's leaning to
  drop the variant.
- **Charon preserves Copy and Move as distinct variants**,
  visible at construction (`translate_operand`) and at
  rendering (`fmt_with_ctx`). Matches RegseqModel's
  leaning to keep the distinction for backend optionality.
  INFERENCE: that even a verification-only IR with no
  codegen back-end pressure preserves Copy/Move suggests
  the distinction has independent semantic value (or at
  least, Charon judged the cost of preservation lower than
  the cost of eliminating it).
- **Charon's `Operand::Const(Box<ConstantExpr>)` differs
  from RegseqModel's `Operand::Constant(ConstId)` in the
  storage discipline.** Charon inlines the constant
  payload (with a `Box` for layout); RegseqModel references
  an interned constant by ID. Both designs accept the
  trade-off in different directions: Charon's choice
  preserves ConstantExpr-specific kinds (Global, FnDef,
  Var, etc.) at the operand site for later micro-pass
  rewrites; RegseqModel's choice composes with stage-4's
  pre-existing interning machinery at no additional cost.

  RegseqModel's design is closer to the *post-*
  `simplify_constants` Charon shape (a normalised constant
  payload with most heterogeneity pushed down to Place
  loads), since stage-4's `Const` enum is already
  post-evaluation and excludes things like `Global` /
  `FnDef` (those are stage-12 `Item` / stage-9
  `InstanceKind` references in RegseqModel, not
  consts).

- **`RuntimeChecks` convergence: both lower, with
  different downstream shapes.** Charon's translator emits
  `StorageLive(local) + Assign(local, Rvalue::NullaryOp(...)) + Operand::Move(local)`
  as the rustc -> ULLBC translation for runtime-check
  operand uses (lowered to the rvalue level; `Operand` sees
  a `Move` of a synthesised local). RegseqModel's D-6-1.2
  decision lowers more aggressively: directly to
  `Operand::Constant(ConstId)` referencing a
  `Const::Bool(b)` interned via stage-4's `ConstInterner`,
  with `b = checks.value(tcx.sess)` resolved at bridge time
  against the bridge-host's session. Both designs agree
  that `RuntimeChecks` does not belong at the operand
  level; Charon preserves it as an rvalue (consistent with
  verification-IR rationale to track its origin), while
  RegseqModel collapses to a literal bool (consistent with
  the codegen-IR rationale that only the resolved value
  matters).

  RegseqModel's collapse is well-defined per Q4
  (`rustc_driver::Callbacks` integration -- one consuming
  `Session` per build) regardless of Q6's multi-crate
  reality; the bridge sees one session, and that session
  is what determines the bool. Charon's choice to keep the
  construct at the rvalue level reflects its different
  goal (functional verification of arbitrary Rust programs)
  and its different consumer (Aeneas's symbolic model,
  which may want to reason about the flag value
  parametrically).

  INFERENCE for RegseqModel: Charon's "lower, do not
  reject" choice is direct precedent for D-6-1.2 (option
  C). The aggressiveness gradient (rvalue-level lowering
  in Charon vs operand-level constant in RegseqModel) is
  appropriate to each project's downstream consumers.

### D-6-2 (storage / interning)

- **Charon's `Operand` is not hash-consed**
  (Section 7). Hash-consing infrastructure exists but is
  reserved for types.
- **Charon's `Place` is also not hash-consed**, despite
  being recursive (`Box<Place>`-projection chains could
  benefit from sharing). INFERENCE: even an IR designed for
  serialised export at scale prefers structural per-use
  storage of operands and places.
- Matches RegseqModel's leaning (no operand-level
  interning).

### D-6-3 (operand-site invariants)

- **No type-`Copy` invariant** is enforced on
  `Operand::Copy(p)` in LLBC. The Aeneas paper does not
  document such a constraint at the operand level
  (Section 1's arxiv-fetch attempt did not surface it; if
  the paper enforces such an invariant it is below the
  granularity that the public abstract / accessible PDF
  surface). INFERENCE (medium confidence, based on absence
  in source): Charon and Aeneas operate on a post-drop-
  elaboration shape, just like RegseqModel.
- **A post-`simplify_constants` invariant exists**
  (Section 6): `Operand::Const(c)` has `c.kind` mostly
  reduced to `Literal` after the pass. RegseqModel's
  stage-4 `Const` enum is structurally already at the
  reduced layer, so the equivalent invariant is enforced by
  type construction rather than a normalisation pass.
- Matches RegseqModel's leaning (no type-Copy enforcement;
  post-drop semantics inherited from `optimized_mir`).

### D-6-K (rendering deferral)

- **Charon prints `copy <p>` / `move <p>` / `const <c>`**
  with keyword prefixes (Section 9). Distinct prefix per
  variant; no symbol-heavy syntax.
- This is one valid prior-art point for RegseqModel's
  deferred stage-11 `render_operand`. RegseqModel's
  deferral target is unchanged by this finding -- the
  *deferral* is the decision; the *rendering style* is
  stage-11's call.

---

## 12. What's NOT in this pass

- **Aeneas paper deep dive.** The arxiv PDF (582 KB) was
  fetched but the surfaced content did not contain
  operand-grammar productions or operand-level
  well-formedness rules at the granularity stage 6 needs.
  The fact that the paper exists and publishes a "pure,
  functional semantics for LLBC" is corroborated; whether
  the paper enforces additional operand invariants is
  unknown from this pass.
- **`hax` boundary.** The actual MIR -> Charon translation
  goes through the `hax` extraction layer, which has its
  own representation
  (`hax::ConstOperandKind::Value(...)` /
  `Promoted(...)` visible in
  [`translate_bodies.rs:550-562`]). The `hax`-side shape
  is an upstream concern; stage 6 does not need to match
  it.
- **OCaml-side IR (`charon-ml`).** Charon ships an OCaml
  consumer library at `charon-ml/`, which mirrors the Rust
  AST in OCaml types
  ([`charon-ml/src/Print.ml`] referenced in the search hits
  for `RuntimeChecks` shows that the OCaml side also has
  the variant). Not investigated in detail; the Rust AST
  is canonical.
- **Verification-target rendering (F* / Coq / Lean).** The
  Aeneas backend is a separate codebase
  (https://github.com/AeneasVerif/aeneas), not Charon.
  Charon's job ends at LLBC serialisation. Operand
  rendering in F* / Coq / Lean is downstream of this
  prior-art pass.
- **Rvalue-level interactions.** `Rvalue::Use(Operand)`,
  `Rvalue::BinaryOp(_, Operand, Operand)`, etc.
  surfaces are stage-7 RegseqModel territory and were not
  pursued here.
- **Cranelift / MLIR / LLVM-IR comparisons.** Per
  parent-context decision, this prior-art pass is scoped to
  Charon only; Cranelift / MLIR / LLVM-IR are different
  paradigms (SSA-values-as-operands) and were
  deliberately excluded from stage-6's prior-art scope.
