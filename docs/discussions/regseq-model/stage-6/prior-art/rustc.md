# Stage 6 -- Prior Art: rustc primary source

Primary-source rustc evidence for `mir::Operand<'tcx>` and
its consumer pipeline, scoped to what stage 6's decision
cluster (D-6-1 variant set, D-6-2 storage / interning,
D-6-3 invariants, D-6-K render deferral) needs. Fresh pass
per `../../README.md` §2 methodology rule (do not reuse
stage-0 summaries).

All paths below are relative to the pinned-toolchain root:

```
~/.rustup/toolchains/nightly-2026-02-01-aarch64-apple-darwin/lib/rustlib/rustc-src/rust/compiler/
```

(See `../../rustc-references.md` for the canonical statement
of this root path.)

Every factual claim cites a `[file:line]` in that pinned
source. Hypotheses are flagged. Inferences from one site to
another are labelled INFERENCE.

---

## 1. `Operand<'tcx>` enum shape

`Operand<'tcx>` is a 4-variant closed enum
[`rustc_middle/src/mir/syntax.rs:1308-1341`]:

```rust
#[derive(Clone, PartialEq, TyEncodable, TyDecodable, Hash,
         HashStable, TypeFoldable, TypeVisitable)]
pub enum Operand<'tcx> {
    /// Creates a value by loading the given place.
    ///
    /// Before drop elaboration, the type of the place must be
    /// `Copy`. After drop elaboration there is no such
    /// requirement.
    Copy(Place<'tcx>),

    /// Creates a value by performing loading the place, just
    /// like the `Copy` operand.
    ///
    /// This *may* additionally overwrite the place with
    /// `uninit` bytes, depending on how we decide in [UCG#188].
    /// You should not emit MIR that may attempt a subsequent
    /// second load of this place without first re-initializing
    /// it.
    Move(Place<'tcx>),

    /// Constants are already semantically values, and remain
    /// unchanged.
    Constant(Box<ConstOperand<'tcx>>),

    /// Query the compilation session of the current crate for
    /// a particular flag. This is not quite a const since its
    /// value can differ across crates within a single crate
    /// graph.
    RuntimeChecks(RuntimeChecks),
}
```

Key facts:

- **4 variants, closed.** No `Other` / `Opaque` arm.
- **Copy and Move are structurally identical** (both wrap a
  bare `Place<'tcx>`). The semantic distinction is documented
  in their docstrings, not in the variant payload.
- **Copy's pre-drop-elaboration type-`Copy` invariant is
  explicitly stated as gone post-drop-elaboration**
  (`syntax.rs:1312-1313`). RegseqModel reads `optimized_mir`,
  which is post-drop-elaboration, so this invariant does not
  apply at our boundary.
- **Constant uses `Box<ConstOperand>` indirection.** rustc
  boxes the constant payload to keep the enum size down: with
  inline `ConstOperand` (which carries a `Span` plus
  `Option<UserTypeAnnotationIndex>` plus a `Const<'tcx>`), the
  enum's worst-case width would be the constant payload, not
  the place payload.
- **RuntimeChecks** is documented as session-specific (not
  const) precisely because it can differ across crates in a
  graph (`syntax.rs:1338-1340`). The rustc MIR carries it as
  a 4th first-class operand variant rather than collapsing it
  to `Const` because const-eval would have to be told which
  crate's session to query.

---

## 2. `ConstOperand<'tcx>` wrapper

The wrapper around an MIR constant operand
[`rustc_middle/src/mir/syntax.rs:1343-1356`]:

```rust
#[derive(Clone, Copy, PartialEq, TyEncodable, TyDecodable,
         Hash, HashStable)]
#[derive(TypeFoldable, TypeVisitable)]
pub struct ConstOperand<'tcx> {
    pub span: Span,

    /// Optional user-given type: for something like
    /// `collect::<Vec<_>>`, this would be present and would
    /// indicate that `Vec<_>` was explicitly specified.
    ///
    /// Needed for NLL to impose user-given type constraints.
    pub user_ty: Option<UserTypeAnnotationIndex>,

    pub const_: Const<'tcx>,
}
```

Three fields, each with a distinct consumer:

- **`span`**: source location of the constant *use site*
  (distinct from the const value's definition span). Used
  primarily for diagnostics.
- **`user_ty`**: present when the user wrote an explicit type
  ascription on the constant (e.g., `collect::<Vec<_>>()`).
  The doc comment is explicit that this is "Needed for NLL to
  impose user-given type constraints." NLL (non-lexical
  lifetimes / borrowck) runs *before* MIR optimization passes;
  the field is dead by the time `optimized_mir` is produced.
  INFERENCE (high confidence): a downstream consumer of
  `optimized_mir` cannot reconstruct anything from `user_ty`
  that it couldn't already see in the resolved `const_` value
  -- the field is informational metadata for an upstream pass
  that has already run.
- **`const_`**: the actual `mir::Const<'tcx>` payload (note:
  this is *MIR*'s `Const`, not `ty::Const` -- see Section 3).

The wrapper is wrapped in `Box<...>` at the `Operand` level.
This is purely a memory-layout choice: keeping the variant
payload behind a single pointer keeps the parent `Operand`
enum at ~16 bytes instead of ~50+ bytes.

---

## 3. `mir::Const<'tcx>` (briefly)

The `const_: Const<'tcx>` field of `ConstOperand` is
`mir::Const<'tcx>` [`rustc_middle/src/mir/consts.rs:31-66`,
not re-quoted here in full -- stage 4's prior art covers it].
The relevant fact for stage 6 is that this is MIR's three-arm
const variant: `Ty(ty::Const)`, `Val(ConstValue, Ty)`,
`Unevaluated(UnevaluatedConst, Ty)`. By the time codegen runs
the collector has already evaluated everything to `Val`
(`rustc_codegen_ssa/src/mir/constant.rs:23` panic message:
"erroneous constant missed by mono item collection"). For
RegseqModel this means stage 4's `Const` enum -- which
already embodies post-evaluation value content -- is the
right interface; an `Operand::Constant(ConstId)` referencing
stage 4's `ConstInterner` is the natural shape, with no
parallel "unevaluated" arm.

---

## 4. `RuntimeChecks` enum

The `RuntimeChecks` variant carries one of three session
queries [`rustc_middle/src/mir/syntax.rs:1572-1593`]:

```rust
#[derive(Copy, Clone, Debug, PartialEq, Eq, TyEncodable,
         TyDecodable, Hash, HashStable)]
pub enum RuntimeChecks {
    /// Returns whether we should perform some UB-checking at
    /// runtime. See the `ub_checks` intrinsic docs for details.
    UbChecks,
    /// Returns whether we should perform contract-checking at
    /// runtime. See the `contract_checks` intrinsic docs for
    /// details.
    ContractChecks,
    /// Returns whether we should perform some overflow-checking
    /// at runtime. See the `overflow_checks` intrinsic docs for
    /// details.
    OverflowChecks,
}

impl RuntimeChecks {
    pub fn value(self, sess: &rustc_session::Session) -> bool {
        match self {
            Self::UbChecks => sess.ub_checks(),
            Self::ContractChecks => sess.contract_checks(),
            Self::OverflowChecks => sess.overflow_checks(),
        }
    }
}
```

Properties:

- **Three sub-variants only.** Resolves to `bool` via
  session flags: `sess.ub_checks()`, `sess.contract_checks()`,
  `sess.overflow_checks()`.
- **Per-crate compile-time constant.** The result is fixed
  once the session is fixed; it doesn't vary at runtime
  *within* a crate. The "Runtime" in the name refers to the
  fact that the consuming code *might* perform checks at
  runtime when the flag is true -- but the flag value itself
  is a build-time decision per crate.
- **Why not folded into `Const`?** rustc keeps it separate
  because the value depends on the *consumer* crate's session.
  In a multi-crate graph, the same `cfg!(ub_checks)` reference
  in a generic function could yield different booleans
  depending on which crate is monomorphizing it.
  INFERENCE: this is rustc accommodating its multi-crate
  reality. Per Q4 (`rustc_driver::Callbacks` integration),
  the RegseqModel bridge runs inside one rustc invocation
  per build with one consuming `Session` in scope. Q6's
  multi-crate strategy (regseq facade + regseq-aware
  dependency crates compiled separately under
  `RegseqAwareCallbacks`, with their MIR consumed via
  `tcx.optimized_mir(def_id)`) does not break this: the
  bridge is a single consumer regardless of how many crates
  contributed MIR to the build graph. The rustc multi-consumer
  rationale therefore collapses to a single resolution site
  at the bridge boundary -- mirroring rustc's standard
  codegen behavior, where the consumer's session determines
  the value. Lowering the construct to `Const::Bool` at
  bridge time (D-6-1.2 option C) is well-defined per build.

---

## 5. `codegen_operand` consumer shape

The canonical operand-consumer match
[`rustc_codegen_ssa/src/mir/operand.rs:1038-1096`]:

```rust
pub fn codegen_operand(
    &mut self,
    bx: &mut Bx,
    operand: &mir::Operand<'tcx>,
) -> OperandRef<'tcx, Bx::Value> {
    match *operand {
        mir::Operand::Copy(ref place)
        | mir::Operand::Move(ref place) => {
            let kind = match operand {
                mir::Operand::Move(_) => LangItem::CompilerMove,
                mir::Operand::Copy(_) => LangItem::CompilerCopy,
                _ => unreachable!(),
            };

            let move_annotation =
                self.move_copy_annotation_instance(
                    bx, place.as_ref(), kind);

            OperandRef {
                move_annotation,
                ..self.codegen_consume(bx, place.as_ref())
            }
        }

        mir::Operand::RuntimeChecks(checks) => {
            let layout = bx.layout_of(bx.tcx().types.bool);
            let BackendRepr::Scalar(scalar) =
                layout.backend_repr else {
                bug!("from_const: invalid ByVal layout: {:#?}",
                     layout);
            };
            let x = Scalar::from_bool(checks.value(bx.tcx().sess));
            let llval = bx.scalar_to_backend(
                x, scalar, bx.immediate_backend_type(layout));
            let val = OperandValue::Immediate(llval);
            OperandRef { val, layout, move_annotation: None }
        }

        mir::Operand::Constant(ref constant) => {
            let constant_ty = self.monomorphize(constant.ty());
            // SIMD specialization elided ...
            self.eval_mir_constant_to_operand(bx, constant)
        }
    }
}
```

Key observations:

- **Copy and Move share a code path** (`codegen_consume`).
  The only operational difference is the `move_annotation`
  field, which is debug-info attached when `compiler_move` /
  `compiler_copy` lang items are present and the size /
  representation conditions are met
  (`operand.rs:1108-1160`). The annotation does not change
  the emitted load -- it adds a debug scope.
- **Move's distinguishing operational feature is at `Call`
  sites**, not at general operand evaluation (per the
  `Operand::Move` doc comment, `syntax.rs:1322-1330`: "it has
  no effect at all unless it appears in an argument to
  `Call`; for `Call` it allows the argument to be passed to
  the callee 'in-place'"). At general operand sites, Move and
  Copy are codegen-equivalent in practice.
- **RuntimeChecks resolves at codegen time** via
  `checks.value(bx.tcx().sess)` -- that is, the value is
  baked in at the codegen pass, not at runtime, despite the
  variant name. The result becomes a `bool` immediate.
- **Constant flows through `eval_mir_constant_to_operand`**
  (`operand.rs:1093`), which calls
  `eval_mir_constant` (`mir/constant.rs:23`); by that point
  the collector has already evaluated the const, so this is
  a value lookup, not an evaluation.

---

## 6. `move_copy_annotation_instance` -- what's lost when Copy/Move collapses

The annotation instance is built only when *all* of the
following hold [`rustc_codegen_ssa/src/mir/operand.rs:1108-1160`]:

1. Debug info is enabled
   (`sess.opts.debuginfo != DebugInfo::None`).
2. The unstable opt `annotate_moves` is enabled.
3. The place's resolved type has a `BackendRepr::Memory`
   layout (i.e., goes through memory rather than living in
   registers).
4. The size exceeds a configured threshold and is non-zero.

When these conditions don't hold, the annotation is `None`
and the only difference between Copy and Move at the
codegen-operand level vanishes. INFERENCE: for the
RegseqModel C backend (which produces C source, not LLVM IR
or machine code), the annotation distinction is not
load-bearing -- C doesn't have a corresponding debug
construct emitted at this granularity. The Copy/Move
distinction is effectively *informational* for the C
backend.

However: the distinction may matter for backend-specific
*lowering decisions*. A C backend that wants to emit
`memcpy` for large moves and an inline copy for small ones,
or that wants to elide a final move into a return slot,
needs to know which is which. INFERENCE: preserving the
variant in RegseqModel keeps the option open for future
backend-specific specialization at zero spec cost.

---

## 7. `Operand` storage in MIR -- no interning

Operands are stored inline at use sites in MIR. The
containing types are `Statement`, `Rvalue`, `Terminator`:

- `Rvalue::Use(Operand<'tcx>)`, `Rvalue::Repeat(Operand, _)`,
  `Rvalue::BinaryOp(_, Box<(Operand, Operand)>)`,
  `Rvalue::UnaryOp(_, Operand)`, `Rvalue::Cast(_, Operand, Ty)`,
  etc. [`rustc_middle/src/mir/syntax.rs:1369-1568`].
- `TerminatorKind::Call { func: Operand, args: Vec<Spanned<Operand>>, ... }`
  [`rustc_middle/src/mir/syntax.rs:937-988`, schematic].
- `TerminatorKind::SwitchInt { discr: Operand<'tcx>, ... }`
  [`rustc_middle/src/mir/syntax.rs:851-863`].

There is no `OperandId` or operand-interner table at the
crate or body level. Operands are constructed inline,
moved into use sites, and have structural equality.

---

## 8. `rustc_public::mir::Operand` mirror

The owned IR mirror in the in-tree
SMIR-replacement crate `rustc_public`
[`rustc_public/src/mir/body.rs:671-707`]:

```rust
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize)]
pub enum Operand {
    Copy(Place),
    Move(Place),
    Constant(ConstOperand),
    RuntimeChecks(RuntimeChecks),
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize)]
pub struct ConstOperand {
    pub span: Span,
    pub user_ty: Option<UserTypeAnnotationIndex>,
    pub const_: MirConst,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize)]
pub enum RuntimeChecks {
    UbChecks,
    ContractChecks,
    OverflowChecks,
}
```

Observations:

- **The shape is preserved 1:1.** `rustc_public` -- the
  forthcoming "publishable" stable IR -- mirrors the same
  4-variant operand surface, with the same `ConstOperand`
  wrapper and the same `RuntimeChecks` variant.
- **`Box<ConstOperand>` indirection is dropped** in
  `rustc_public` (the variant carries `ConstOperand` by
  value). INFERENCE: rustc_public is `Serialize`-derived and
  the storage trade-off is different (no `Box` overhead
  considerations for ser/deser; willingness to pay the
  enum-size cost in exchange for simpler shape).
- **`user_ty` and `span` are preserved** in the
  publishable IR, which suggests the rustc maintainers
  consider those fields semantically meaningful enough to
  expose at the stable boundary -- even though `user_ty` is
  borrowck-only and dead post-`optimized_mir`. INFERENCE:
  the stable IR is a snapshot of MIR shape, not necessarily
  pruned of fields whose downstream consumers have already
  fired. RegseqModel is downstream of `optimized_mir`
  *consumers* (codegen-style), not a stable-IR republisher,
  so the pruning calculus is different.

---

## 9. Cross-stage anchors

`Operand` references content pinned in earlier stages:

- **`Place`** is pinned in stage 5 (§19 of top-level
  spec.md). Stored inline by-value (no `PlaceId`); structural
  equality over `(local: LocalId, projection: Box<[PlaceElem]>)`.
  `Operand::Copy(Place)` and `Operand::Move(Place)` carry
  `Place` directly.
- **`Const`** is pinned in stage 4 (§13 of top-level
  spec.md). Stored in a top-level `ConstInterner`; referenced
  by `ConstId` (NonMaxU32-backed). Stage 4 already uses
  `ConstId` for nested const references (e.g.,
  `Const::Aggregate(Vec<ConstId>)`,
  `Const::EnumValue { payload: Vec<ConstId>, ... }`).
  `Operand::Constant(ConstId)` extends this discipline to
  use-site references at the operand level.

There is no `Operand`-side dependency on stage 11 (`Body`) or
stage 12 (`Item`). Operand's content is fully expressible in
terms of stage-4 + stage-5 + stage-1 vocabulary.

---

## 10. Cross-D-tag findings

Mapping of evidence to D-6-K decisions:

### D-6-1 (variant set)

- **Copy and Move structurally identical** (Section 1) but
  documented with distinct semantic intent (Section 1, doc
  comments).
- **Copy/Move codegen-equivalent at general operand sites**
  (Section 5, 6); operational difference is debug-info-only
  in the SSA backend, and doesn't apply to a C backend at
  the same granularity (Section 6 INFERENCE).
- **Move's distinguishing feature is `Call`-site in-place
  passing** (Section 5, doc comment), which would matter for
  a backend that wants to model parameter passing precisely.
- **`RuntimeChecks` is per-consumer session-derived**
  (Section 4); separate from `Const` in rustc only because
  rustc supports multi-crate graphs where the consumer's
  session determines the result and consumers can vary.
  Q4 (`rustc_driver::Callbacks` integration) makes the
  bridge a single consumer per build with one consuming
  `Session` in scope; per-build the value is fixed even
  under Q6's multi-crate strategy (cross-crate MIR is
  consumed via `tcx.optimized_mir(def_id)` against the
  bridge-host's session). The bridge boundary is a single
  resolution site.
- **Constant flows from collector-pre-evaluated MIR Const**
  (Sections 3, 5); stage 4's `Const` already embodies
  post-evaluation values. `Operand::Constant(ConstId)`
  composes naturally with stage 4's `ConstInterner`.

Implication for D-6-1: keep Copy/Move (preserve information
rustc gives us; backend optionality at zero spec cost);
drop RuntimeChecks variant in favour of bridge-side
lowering to `Const::Bool` (D-6-1.2 option C; Q4's single
consuming session per build collapses the multi-consumer
rationale rustc has); use `Constant(ConstId)` (no wrapper,
no `Box`, no `user_ty`, no use-site `span`).

### D-6-2 (storage / interning)

- **MIR has no operand-interner** (Section 7); operands are
  constructed inline at use sites.
- **`rustc_public` follows the same pattern** (Section 8).
- **No downstream consumer of operand-content-uniqueness**
  exists. Contrast with `Const`, which has multiple
  downstream consumers (`GenericArg::Const(ConstId)` in
  stage 3, identity in stage 9 instance-key construction,
  etc.).

Implication for D-6-2: no interning; `Operand` lives inline
at use sites in stages 7-9 payloads.

### D-6-3 (invariants)

- **`Operand::Copy(p)`'s pre-drop-elaboration type-Copy
  invariant is gone post-drop-elaboration** (Section 1, doc
  comment). RegseqModel reads `optimized_mir` (post-drop-
  elaboration), so this invariant does not apply at our
  boundary.
- **No other rustc-side operand-construction invariants**
  surface in the syntax-level definition. Place-typing
  invariants are part of Place's surface (§19.7 of top-level
  spec.md) and reach operand sites via the place projection
  walk.

Implication for D-6-3: no type-Copy enforcement on
`Operand::Copy(p)`; the invariant rustc once carried is
already discharged by the MIR phase we read.

### D-6-K (rendering deferral)

- **No rustc-side operand pretty-printing constraint**
  surfaces in the operand machinery itself; operand
  formatting is an Output of `Display` impls in
  `rustc_middle/src/mir/pretty.rs` (not pursued in this
  pass; deferral target is render-time stage 11 alongside
  `render_place`).

Implication for D-6-K: defer `pretty.rs::render_operand` to
stage 11 alongside `render_place`. Operand rendering needs
the same field/variant name tables that `render_place`
reaches via `LocalDecl.ty`.

---

## 11. What's NOT in this pass

- **`Spanned<Operand>` (used in `TerminatorKind::Call::args`).**
  Schematically the per-argument source-location wrapping;
  irrelevant to operand variant choice. Picked up in stage 9
  when terminators are specced.
- **Operand-into-Rvalue / Operand-into-Statement embedding.**
  Stage 7 / stage 8 territory. Stage 6 only fixes the
  Operand vocabulary; the embedding sites are downstream
  consumers.
- **`monomorphize` / `Instance::resolve` cross-references for
  `Constant` operands.** Already handled at stage 4 (Const)
  and stage 9 (Instance). Operand carries `ConstId`; the
  resolution chain is stage-4-internal.
- **Pretty-printer style.** Per D-6-K deferral.
