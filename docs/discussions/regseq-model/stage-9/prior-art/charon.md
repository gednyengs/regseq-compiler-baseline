# Stage 9 -- Prior Art: Charon primary source

Primary-source Charon (ULLBC / LLBC) evidence for `TerminatorKind`,
its `Terminator` wrapper, the rustc-MIR -> Charon translation
behaviour governing what gets carried as a Terminator vs. lifted to
a Statement vs. dropped, the post-translation reshaping passes
touching terminator-level constructs, and -- most importantly --
the `transform_dyn_trait_calls` pass that rewrites
`<dyn Trait as Trait>::method` calls into indirect calls through
the vtable's loaded function pointer. Scoped to stage 9's decision
namespace: D-9-1 (closed `TerminatorKind` surface), D-9-2 (drop as
call), D-9-3 (call callee shape; Direct/Indirect split vs. uniform
Operand), D-9-4 (per-arg span), D-9-5 (`AssertKind` subset), D-9-6
(`SwitchTargets` storage), D-9-7 (no Terminator interning), D-9-8
(wrapper struct), D-9-9 (cleanup-pass-discharged rejection table),
D-9-10 (invariants registry), D-9-K (pretty-printer deferral).
Fresh pass per `../../README.md` §2 methodology rule; companion to
stage-{6,7,8} Charon prior-art -- this stage extends the same
sources to terminator-level decisions and avoids re-deriving
already-cited Operand/Rvalue/Statement findings.

Charon is INRIA's Rust front-end translator feeding the Aeneas
verification pipeline. ULLBC ("Unstructured LLBC") is the
basic-block-shaped IR retained directly out of MIR; LLBC ("Low-Level
Borrow Calculus") is the structurally-reconstructed (loop +
if/else + match) sibling that fuses Statement and Terminator.
**For stage 9 the relevant layer is ULLBC** -- ULLBC's
`TerminatorKind` is the direct rustc-MIR `TerminatorKind<'tcx>`
analog and the structurally-closer peer to RegseqModel's stage-9
candidate. LLBC's terminator lift (Switch + Loop become
`StatementKind` arms, `Call` and `Drop` likewise demoted) is
informational evidence about what the structured form keeps; it is
*not* RegseqModel's target.

Repository: `https://github.com/AeneasVerif/charon` (main).
Local clone pinned at commit
`46ab49f627e1a8e72b17c437f05ae4d1c4c456df` (2025-11-27, "Merge
pull request #919 from Nadrieril/raw-consts"), same commit as the
stage-{6,7,8} Charon files.

Citation convention: every factual claim cites `[file:line]` in
the Charon source tree at this commit, paths relative to
`/Users/nyengele/Documents/sekekama/charon-1/charon/`.
`HYPOTHESIS:` flags speculation. `INFERENCE:` flags cross-site
synthesis.

Stage-9 framing: **TerminatorKind** (RegseqModel) is the per-block
control-flow record at the end of a basic block -- analogous to
`rustc_middle::mir::TerminatorKind<'tcx>`. RegseqModel's
closed-surface candidate is a 6-variant
`TerminatorKind::{Goto, Switch, Call, Return, Unreachable,
Assert}` with an explicit Direct/Indirect callee split for
vtable-indirect dispatch (D-9-3 option (b)). Charon is the most
relevant non-rustc peer: (a) closed `Vec<Operand>` arg storage,
(b) Serde-derived for verification consumers, (c) consumes rustc
MIR via hax, (d) has had to make the same "what counts as a
Terminator, what gets demoted/dropped/collapsed" choices, and (e)
**ships a load-bearing `transform_dyn_trait_calls` pass** that
rewrites trait-method calls on `dyn Trait` receivers into indirect
calls through vtable function pointers -- direct precedent for the
D-9-3 (b)-shape recommendation.

Primary files inspected:

- `src/ast/ullbc_ast.rs` -- ULLBC IR types (`TerminatorKind`,
  `Terminator`, `SwitchTargets`, `BlockData`, `BodyContents`).
- `src/ast/llbc_ast.rs` -- LLBC IR types (informational peer).
- `src/ast/gast.rs` -- shared shapes: `Call`, `FnOperand`,
  `AbortKind`, `DropKind`, `Assert`, `Body`, `GExprBody`,
  `Locals`.
- `src/ast/expressions.rs` -- `FnPtr`, `FnPtrKind`, `FunId`,
  `BuiltinFunId`, `MaybeBuiltinFunDeclRef`, `TraitMethodRef`.
- `src/ast/ullbc_ast_utils.rs` -- `Terminator::new`,
  `BlockData::targets`, `SwitchTargets::get_targets`,
  `Terminator::is_error`, `BlockData::new_goto`.
- `src/bin/charon-driver/translate/translate_bodies.rs` --
  rustc-MIR -> ULLBC body translator (`translate_terminator`,
  `translate_function_call`, `translate_drop`,
  `translate_unwind_action`, `translate_switch_targets`,
  `translate_arguments`).
- `src/bin/charon-driver/translate/translate_crate.rs` --
  `translate_fn_ptr` (Direct vs. Trait callee discrimination).
- `src/transform/normalize/desugar_drops.rs` -- rewrites
  `TerminatorKind::Drop` into `TerminatorKind::Call` (the D-9-2
  precedent).
- `src/transform/normalize/transform_dyn_trait_calls.rs` --
  **central D-9-3 evidence**: rewrites the `Call.func` from
  `FnOperand::Regular(FnPtr{kind: Trait(Dyn, ...), ...})` into
  `FnOperand::Move(<vtable load place>)`.
- `src/transform/normalize/filter_unreachable_blocks.rs` --
  CFG cleanup post-rewrite.
- `src/transform/resugar/reconstruct_asserts.rs` -- folds
  `Switch -> {Abort | continue}` panic-branch idiom into
  `StatementKind::Assert` (the D-9-5 + Statement-vs-Terminator
  Assert evidence).
- `src/transform/resugar/inline_local_panic_functions.rs` --
  rewrites calls to expanded `panic_cold_explicit` shims into
  `TerminatorKind::Abort(AbortKind::Panic(...))`.
- `src/transform/resugar/reconstruct_intrinsics.rs` -- reshapes a
  `Call` terminator into `Goto` after lifting an offset_of call to
  an Assign statement.
- `src/transform/resugar/reconstruct_boxes.rs` -- intrinsics-driven
  call rewriting (referenced for context, not load-bearing).
- `src/transform/simplify_output/filter_trivial_drops.rs` --
  collapses no-op-destruct `Drop`s into `Goto`.
- `src/transform/control_flow/duplicate_return.rs`,
  `merge_goto_chains.rs` -- CFG-shape simplifications.
- `src/transform/control_flow/ullbc_to_llbc.rs` -- ULLBC->LLBC
  bridge (informational; shows how each ULLBC `TerminatorKind`
  arm is demoted to LLBC `StatementKind`).
- `src/transform/ctx.rs` -- `transform_ullbc_terminators` driver.
- `src/pretty/fmt_with_ctx.rs` -- `FmtWithCtx` impls for
  `Terminator`, `Call`, `FnOperand`, `FnPtr`, `AbortKind`,
  `Assert`.

---

## 1. ULLBC `TerminatorKind` enum shape

Charon's terminator surface is **8 variants** -- substantially
narrower than rustc's `TerminatorKind<'tcx>` (15 variants). The
enum is defined verbatim at [`src/ast/ullbc_ast.rs:81-113`]:

```rust
/// A raw terminator: a terminator without meta data.
#[derive(Debug, Clone, EnumIsA, EnumAsGetters, Serialize, Deserialize, Drive, DriveMut)]
pub enum TerminatorKind {
    Goto {
        target: BlockId,
    },
    Switch {
        discr: Operand,
        targets: SwitchTargets,
    },
    Call {
        call: Call,
        target: BlockId,
        on_unwind: BlockId,
    },
    /// Drop the value at the given place.
    Drop {
        #[drive(skip)]
        kind: DropKind,
        place: Place,
        tref: TraitRef,
        target: BlockId,
        on_unwind: BlockId,
    },
    /// Handles panics and impossible cases.
    Abort(AbortKind),
    Return,
    UnwindResume,
}
```

Cardinality: **7 syntactic arms** (`Goto`, `Switch`, `Call`,
`Drop`, `Abort`, `Return`, `UnwindResume`) plus `Abort` carrying a
3-variant `AbortKind`. Compared to RegseqModel's 6-variant
candidate (`Goto`, `Switch`, `Call`, `Return`, `Unreachable`,
`Assert`):

- Charon has **no `Assert` Terminator** -- assert checks live at
  the **Statement** level (§6).
- Charon has **no `Unreachable` Terminator** -- it folds rustc's
  `Unreachable` into `Abort(AbortKind::UndefinedBehavior)` (§7).
- Charon **keeps `Drop`** as a Terminator until the optional
  `--desugar-drops` pass collapses it into `Call` (§2).
- Charon **keeps `UnwindResume`** as a real Terminator, even
  though Aeneas elides unwinding semantically -- it's preserved as
  a CFG marker and the LLBC bridge folds it to
  `StatementKind::Abort(Panic(None))`
  [`src/transform/control_flow/ullbc_to_llbc.rs:1462-1465`].
- Charon **omits** `Yield`, `CoroutineDrop`, `TailCall`,
  `InlineAsm`, `FalseEdge`, `FalseUnwind`, `Assert`,
  `UnwindTerminate` (the latter folded into
  `Abort(AbortKind::UnwindTerminate)`) at the translator
  [`translate_bodies.rs:881-958`]; see §9.

Notable design choices visible from the type definition:

- **`Call` and `Drop` carry `target` AND `on_unwind` BlockIds**
  inline; both are mandatory `BlockId`, never `Option<BlockId>`.
  rustc's `Call` carries `Option<BasicBlock>` for `target` and a
  separate `UnwindAction` enum; Charon collapses both into
  in-CFG block ids (no-target -> synthesised abort block;
  unwind-action -> synthesised resume/abort block;
  unwind-action -> `Cleanup` block id directly). See §4 + §9.
- **No `targets: Box<TerminatorKind>` interning.** The variants
  are inline; only the inside of `Call` (a `Call` struct itself)
  carries field-level data and could in principle be Boxed but
  isn't -- no size budget enforced.
- **`Abort` is a single variant with a discriminator payload**
  (`AbortKind`), not three separate Terminator variants; this is
  the Charon-side parallel to D-9-9 ("collapse panic-shape
  arms").

Variant-by-variant payload:

| Variant | Payload | Translator origin |
|---|---|---|
| `Goto` | `{ target: BlockId }` | rustc `Goto` directly; also synthesised from `Assert`, `FalseEdge`, `FalseUnwind`, `Drop` (when noop-destruct) |
| `Switch` | `{ discr: Operand, targets: SwitchTargets }` | rustc `SwitchInt` (translator: `translate_switch_targets`) |
| `Call` | `{ call: Call, target: BlockId, on_unwind: BlockId }` | rustc `Call` (translator: `translate_function_call`) |
| `Drop` | `{ kind: DropKind, place: Place, tref: TraitRef, target: BlockId, on_unwind: BlockId }` | rustc `Drop` (translator: `translate_drop`) |
| `Abort(AbortKind)` | `Panic(Option<Name>)`, `UndefinedBehavior`, `UnwindTerminate` | rustc `Unreachable`, `UnwindTerminate`, panic lang-items, no-target `Call` |
| `Return` | (unit) | rustc `Return` |
| `UnwindResume` | (unit) | rustc `UnwindResume`; also synthesised in `Continue` arm of `translate_unwind_action` |

`AbortKind` is itself defined at [`src/ast/gast.rs:411-419`]:

```rust
pub enum AbortKind {
    /// A built-in panicking function.
    Panic(Option<Name>),
    /// Undefined behavior in the rust abstract machine.
    UndefinedBehavior,
    /// Unwind had to stop for Abi reasons or because cleanup code panicked again.
    UnwindTerminate,
}
```

`AbortKind::Panic` carries an optional fully-qualified `Name` of
the panic-emitting function, so Charon preserves "this came from
panic_fmt" vs. "this came from begin_panic" through to LLBC.
RegseqModel is `#![no_std]` device-driver-only and has no
panic-emit notion, so D-9-1's mapping is: `Abort(Panic(Some(_)))`
+ `Abort(Panic(None))` -> RegseqModel's prospective `Unreachable`;
`Abort(UndefinedBehavior)` -> RegseqModel's prospective
`Unreachable`; `Abort(UnwindTerminate)` -> reject (we have no
unwinding model).

Maps to D-9-1 (closed surface: Charon 7 syntactic arms vs.
RegseqModel candidate 6), D-9-9 (Charon's translator drops 8 of
rustc's 15 terminator variants; collapses several to existing arms
via panic-name preservation in `AbortKind::Panic`).

---

## 2. Drop handling -- preserved Terminator with optional Call collapse

Charon **preserves `Drop` as a first-class Terminator** in the
default ULLBC pipeline and offers an *opt-in*
`transform_dyn_trait_calls`-adjacent pass (`desugar_drops`) that
rewrites it into a `Call` terminator. This is direct precedent for
D-9-2 ("Drop-as-Call collapse") -- but with the inverted default:
Charon's default is "preserve `Drop`", and the collapse is gated
by `--desugar-drops`.

The translator emits `TerminatorKind::Drop` directly from rustc's
`Drop` [`translate_bodies.rs:1126-1148`]:

```rust
fn translate_drop(
    &mut self,
    span: Span,
    place: &hax::Place,
    impl_expr: &hax::ImplExpr,
    target: &hax::BasicBlock,
    unwind: &UnwindAction,
) -> Result<TerminatorKind, Error> {
    let place = self.translate_place(span, place)?;
    let tref = self.translate_trait_impl_expr(span, impl_expr)?;
    let target = self.translate_basic_block_id(*target);
    let on_unwind = self.translate_unwind_action(span, unwind);

    Ok(TerminatorKind::Drop {
        kind: self.drop_kind,
        place,
        tref,
        target,
        on_unwind,
    })
}
```

Two important specialisations of the `Drop` variant:

1. The `kind: DropKind` discriminator
   [`src/ast/gast.rs:424-448`] distinguishes `Precise` ("real
   drop, calls `<T as Destruct>::drop_in_place(&raw mut place)`,
   marks place as moved-out") from `Conditional` ("borrow-checker
   may or may not need to run drop code"). Charon picks one
   globally by MIR phase: `Built`/`Promoted` -> `Conditional`,
   `Elaborated`/`Optimized` -> `Precise`.
2. The `tref: TraitRef` field carries the destructor's trait-ref
   resolution -- Charon lifts the `Destruct::drop_in_place` trait
   instance into the IR rather than letting backends re-resolve
   it.

The `desugar_drops` pass at
[`src/transform/normalize/desugar_drops.rs:23-72`] performs the
Drop-to-Call rewrite **only** for `DropKind::Precise` and **only**
when `ctx.options.desugar_drops` is set
[`desugar_drops.rs:79-82`]:

```rust
fn transform_drop_to_call(&mut self, term: &mut Terminator) {
    if let TerminatorKind::Drop {
        kind: DropKind::Precise,
        place,
        tref,
        target,
        on_unwind,
    } = &mut term.kind
    {
        // check if this drop is noop
        if is_noop_destruct(tref) {
            term.kind = TerminatorKind::Goto { target: target.clone() };
            return;
        }

        // assign `&raw mut place` to a new variable
        let drop_arg = self.raw_borrow_to_new_var(place.clone(), RefKind::Mut, ...);
        let trait_id = tref.trait_decl_ref.skip_binder.id;
        let Some(tdecl) = self.ctx.translated.trait_decls.get(trait_id) else { return; };
        let method_name = TraitItemName("drop_in_place".into());
        let Some(bound_method) = tdecl.methods.iter().find(|m| m.name() == method_name) else {
            return;
        };
        let method_decl_id = bound_method.skip_binder.item.id;

        let drop_ret = self.fresh_var(Some("drop_ret".into()), Ty::mk_unit());
        let fn_ptr = FnPtr::new(
            FnPtrKind::Trait(tref.clone(), method_name, method_decl_id),
            GenericArgs::empty(),
        );
        let call = Call {
            func: FnOperand::Regular(fn_ptr),
            args: Vec::from([Operand::Move(drop_arg)]),
            dest: drop_ret,
        };
        term.kind = TerminatorKind::Call {
            call,
            target: target.clone(),
            on_unwind: on_unwind.clone(),
        };
    }
}
```

Three observations relevant to D-9-2:

- **The collapse is mechanical and total** for `Precise` drops
  with a known destructor trait: synthesise a `&raw mut place`
  borrow into a fresh local, build a `Call` terminator with that
  local as the sole arg and a fresh unit-typed return slot, copy
  the `target` and `on_unwind` from the `Drop`. RegseqModel's
  D-9-2 plan to fold drop into call at construction is structurally
  identical except: (a) RegseqModel can do it at lower level (the
  raw-borrow synthesis is the only non-trivial step), (b)
  RegseqModel does not need the `Conditional` distinction since
  device-driver specs operate on `Copy`-able registers and
  byte-buffer types.
- **`is_noop_destruct(tref)` short-circuits to `Goto`** -- a
  builtin no-op destructor (e.g., for primitive integer types)
  collapses the Drop directly to a `Goto`, removing the call
  entirely. RegseqModel's stage-3/D5 ban on user destructors makes
  this the *only* drop shape RegseqModel ever needs to emit; there
  is no `Drop`-vs-`Call` distinction to maintain.
- **`filter_trivial_drops` performs the same Goto collapse
  earlier** for any `Drop` whose `tref` is
  `BuiltinOrAuto{builtin_data: NoopDestruct, ..}`
  [`src/transform/simplify_output/filter_trivial_drops.rs:5-30`],
  independent of `desugar_drops`. The two passes are belt-and-
  braces.

INFERENCE: Charon's design choice is to keep `Drop` as a
first-class IR variant because Aeneas-side reasoning about drop
semantics benefits from the structural marker; the call-collapse
is offered as an option for backends that prefer uniform
call-shaped IR. RegseqModel has no Aeneas-style downstream and
no destructors, so D-9-2's "always collapse to Call" default is
strictly simpler than Charon's gated rewrite.

Maps to D-9-2 (drop-as-call collapse: Charon offers it as opt-in
pass; RegseqModel adopts it at construction; Charon's mechanical
rewrite -- `&raw mut`, fresh return-slot, copy target/unwind --
is the precedent recipe).

---

## 3. The `transform_dyn_trait_calls` pass -- D-9-3 load-bearing precedent

This is the single most important section for stage 9. Charon's
`transform_dyn_trait_calls` pass at
[`src/transform/normalize/transform_dyn_trait_calls.rs:1-133`]
rewrites the `func` field of a `Call` terminator from
`FnOperand::Regular(FnPtr{kind: Trait(Dyn, ...), ...})` -- the
"direct call to a trait method, with the receiver being a `dyn
Trait`" shape -- into `FnOperand::Move(<vtable load place>)` --
the "indirect call through a function pointer loaded from the
vtable" shape. **This is exactly D-9-3 option (b): a single
`Call` Terminator whose callee position is uniformly an
`Operand`/`FnOperand`, and whose direct-vs-indirect status is
determined by the `FnOperand` enum's two arms (`Regular(FnPtr)`
vs. `Move(Place)`), not by an `InstanceKind::Virtual` marker
inside a Direct callee.**

### 3.1 Pre-pass shape -- direct call carrying a trait-method ref

Before the pass runs, the call site for `x.method(args)` where `x:
&dyn Trait` is encoded as a `TerminatorKind::Call` whose `call.func`
is the `Regular`/`FnPtr` shape produced by the translator's
`translate_fn_ptr` [`src/bin/charon-driver/translate/translate_crate.rs:511-541`]:

```rust
let fun_id = match fun_item.trait_ref {
    None => FnPtrKind::Fun(fun_item.id),
    Some(trait_ref) => {
        let name = self.t_ctx.translate_trait_item_name(&item.def_id)?;
        let method_decl_id = *fun_item.id.as_regular()
            .expect("methods are not builtin functions");
        ...
        FnPtrKind::Trait(trait_ref.move_under_binder(), name, method_decl_id)
    }
};
```

The translator does **not** distinguish dyn-trait calls from
non-dyn trait-method calls at translation time -- both end up as
`FnPtrKind::Trait(trait_ref, name, method_decl_id)`. The
`TraitRefKind::Dyn` discriminator (in `trait_ref.kind`) carries
the "this is a `dyn Trait`" signal forward, set in
`src/bin/charon-driver/translate/translate_predicates.rs:310` for
the `ImplExprAtom::Dyn` case.

So the pre-pass call site looks like:

```text
@dest := <dyn Trait as Trait>::method(receiver_place, arg_1, ..., arg_n) -> bb_target (unwind: bb_unwind)
```

Encoded as:

```rust
TerminatorKind::Call {
    call: Call {
        func: FnOperand::Regular(FnPtr {
            kind: Box::new(FnPtrKind::Trait(
                TraitRef { kind: TraitRefKind::Dyn, trait_decl_ref, .. },
                method_name,
                method_decl_id,
            )),
            generics: ...,
        }),
        args: vec![Operand::Move(receiver_place), Operand::Move(arg_1), ...],
        dest: dest_place,
    },
    target: bb_target,
    on_unwind: bb_unwind,
}
```

### 3.2 The pass body -- detect-and-rewrite

The pass walks every terminator in every function body
[`transform_dyn_trait_calls.rs:124-133`]:

```rust
pub struct Transform;
impl UllbcPass for Transform {
    fn transform_function(&self, ctx: &mut TransformCtx, decl: &mut FunDecl) {
        decl.transform_ullbc_terminators(ctx, |ctx, term| {
            if let TerminatorKind::Call { call, .. } = &mut term.kind {
                let _ = transform_dyn_trait_call(ctx, call);
            }
        });
    }
}
```

The detection guard is a strict three-step pattern match
[`transform_dyn_trait_calls.rs:38-46`]:

```rust
let FnOperand::Regular(fn_ptr) = &call.func else {
    return Ok(()); // Not a regular function call
};
let FnPtrKind::Trait(trait_ref, method_name, _) = &fn_ptr.kind else {
    return Ok(()); // Not a trait method call
};
let TraitRefKind::Dyn = &trait_ref.kind else {
    return Ok(()); // Not a dyn trait trait call
};
```

A call is rewritten **iff** all three conditions hold:

1. `call.func` is `FnOperand::Regular(fn_ptr)` -- not already an
   indirect call.
2. `fn_ptr.kind` is `FnPtrKind::Trait(trait_ref, method_name, _)`
   -- a trait method, not a free function.
3. `trait_ref.kind` is `TraitRefKind::Dyn` -- the trait reference
   resolves through a `dyn Trait` predicate, not a concrete impl.

When all three fire, Charon resolves the vtable struct, finds the
field for the called method, and synthesises the place expression
that loads the method's function pointer
[`transform_dyn_trait_calls.rs:49-117`]:

```rust
// Get the type of the vtable struct.
let vtable_decl_ref: TypeDeclRef = {
    let Some(trait_decl) = ctx.ctx.translated.trait_decls.get(trait_pred.id) else {
        return Ok(()); // Unknown trait
    };
    let Some(vtable_ty) = &trait_decl.vtable else {
        raise_error!(... "Found a `dyn Trait` method call for non-dyn-compatible trait `{}`!", ...);
    };
    vtable_ty.clone().substitute_with_self(&trait_pred.generics, &trait_ref.kind)
};
let vtable_decl_id = *vtable_decl_ref.id.as_adt().unwrap();
let Some(vtable_decl) = ctx.ctx.translated.type_decls.get(vtable_decl_id) else {
    return Ok(()); // Missing data
};
if matches!(vtable_decl.kind, TypeDeclKind::Opaque) {
    return Ok(()); // Missing data
}

// Retreive the method field from the vtable struct definition.
let method_field_name = format!("method_{}", method_name);
let Some((method_field_id, method_field)) =
    vtable_decl.get_field_by_name(None, &method_field_name)
else {
    raise_error!(... "Could not determine method index for {} in vtable {}", ...);
};
let method_ty = method_field.ty.clone().substitute(&vtable_decl_ref.generics);

// Get the receiver (first argument).
if call.args.is_empty() {
    raise_error!(ctx.ctx, ctx.span, "Dyn trait call has no arguments!");
}
let dyn_trait_place = match &call.args[0] {
    Operand::Copy(place) | Operand::Move(place) => place,
    Operand::Const(_) => panic!("Unexpected constant as receiver for dyn trait method call"),
};

// Construct the `(*ptr.ptr_metadata).method_field` place.
let vtable_ty = TyKind::Adt(vtable_decl_ref).into_ty();
let ptr_to_vtable_ty = Ty::new(TyKind::RawPtr(vtable_ty.clone(), RefKind::Shared));
let method_field_place = dyn_trait_place
    .clone()
    .project(ProjectionElem::PtrMetadata, ptr_to_vtable_ty)
    .project(ProjectionElem::Deref, vtable_ty)
    .project(
        ProjectionElem::Field(FieldProjKind::Adt(vtable_decl_id, None), method_field_id),
        method_ty,
    );

// Transform the original call to use the function pointer
call.func = FnOperand::Move(method_field_place);
```

The synthesised place expression is read inside-out:

1. Start with `dyn_trait_place` -- the receiver `Place`. Type:
   `&dyn Trait` (a fat pointer = `(data_ptr, vtable_ptr)`).
2. Apply `ProjectionElem::PtrMetadata` with type `*const VTable`
   -- this picks the metadata half of the fat pointer (Charon's
   stage-7 prior-art covered this Rvalue/projection element). The
   place now denotes the vtable pointer.
3. Apply `ProjectionElem::Deref` with type `VTable` -- dereference
   the vtable pointer. The place now denotes the vtable struct
   itself.
4. Apply `ProjectionElem::Field(FieldProjKind::Adt(vtable_decl_id,
   None), method_field_id)` with type `method_ty` (the function
   pointer field's type, e.g., `fn(&dyn Trait, ...) -> Ret`). The
   place now denotes the *function-pointer field* of the vtable.

The pass's final step **mutates `call.func` in place**:

```rust
call.func = FnOperand::Move(method_field_place);
```

This is the load-bearing line. Post-pass, the call site looks
like:

```text
@dest := (move (*receiver.ptr_metadata).method_method_name)(receiver, arg_1, ..., arg_n) -> bb_target (unwind: bb_unwind)
```

Encoded as:

```rust
TerminatorKind::Call {
    call: Call {
        func: FnOperand::Move(method_field_place), // the synthesised projection
        args: ..., // unchanged
        dest: dest_place,
    },
    target: bb_target,
    on_unwind: bb_unwind,
}
```

### 3.3 IR-shape consequence -- direct/indirect by Operand-arm, not InstanceKind

The rewrite **does not change the `Call` AST node's `func` field's
static type** -- pre-pass and post-pass it is `FnOperand`
[`src/ast/gast.rs:393-398`]:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Drive, DriveMut)]
pub struct Call {
    pub func: FnOperand,
    pub args: Vec<Operand>,
    pub dest: Place,
}
```

The `FnOperand` enum is the load-bearing direct-vs-indirect
discriminator [`src/ast/gast.rs:381-391`]:

```rust
/// A function operand is used in function calls.
/// It either designates a top-level function, or a place in case
/// we are using function pointers stored in local variables.
#[derive(Debug, Clone, Serialize, Deserialize, Drive, DriveMut)]
#[charon::variants_prefix("FnOp")]
pub enum FnOperand {
    /// Regular case: call to a top-level function, trait method, etc.
    Regular(FnPtr),
    /// Use of a function pointer stored in a local variable
    Move(Place),
}
```

This is **exactly D-9-3 option (b)**: a `CallCallee::{Direct(FnPtr),
Indirect(Place)}` split. Charon names it `FnOperand`, the
RegseqModel-level analog is `CallCallee::{Direct(InstanceId),
Indirect(Operand)}`. The semantic identity holds:

- `FnOperand::Regular(FnPtr)` -- statically resolved callee
  (function decl id + generics + optional trait-ref). Backends
  emit a direct call instruction.
- `FnOperand::Move(Place)` -- function pointer loaded from a
  place. Backends emit an indirect call (load function pointer,
  then call).

There is **no `InstanceKind::Virtual` marker inside
`FnPtrKind`**. `FnPtrKind`'s two arms [`src/ast/expressions.rs:432-440`]:

```rust
pub enum FnPtrKind {
    #[charon::rename("FunId")]
    Fun(FunId),
    /// If a trait: the reference to the trait and the id of the trait method.
    #[charon::rename("TraitMethod")]
    Trait(TraitRef, TraitItemName, FunDeclId),
}
```

Are both **direct-call shapes** -- `Fun` is a free function, `Trait`
is a statically-resolved trait method (the trait_ref pins down which
impl). The `TraitRefKind::Dyn` variant is what flags "this trait
method has not yet been resolved to a concrete impl" -- and the
dynamic-dispatch pass *eliminates* that variant by rewriting the
call to indirect form. After `transform_dyn_trait_calls`, no
`FnOperand::Regular(FnPtr{kind: Trait(Dyn, ...), ...})` remains in
the IR; the pass is total when its preconditions hold (vtable
exists, method field found, receiver is a place).

This is the **definitive evidence** that Charon picks D-9-3
option (b): the entire dispatch decision is encoded in the
`FnOperand` enum's two arms, not in a `Direct`-side
"is-this-virtual" marker. The `Trait`/`Dyn` shape is a transient
input-form that gets rewritten away by the pass; it is *not* a
sustained "Direct callee referencing a dyn instance" form.

Maps to D-9-3 (call callee shape: Charon goes the (b)-shape route
via `transform_dyn_trait_calls`. The pass body lives at
`src/transform/normalize/transform_dyn_trait_calls.rs:31-122`; the
key mutation line is `transform_dyn_trait_calls.rs:119`. The
`FnOperand` enum's Regular/Move split is at
`src/ast/gast.rs:386-391`. This is the single load-bearing
prior-art reference for the recommended option).

---

## 4. Call terminator shape

The `Call` struct (carried inside `TerminatorKind::Call.call`) is
defined at [`src/ast/gast.rs:393-398`]:

```rust
pub struct Call {
    pub func: FnOperand,
    pub args: Vec<Operand>,
    pub dest: Place,
}
```

Three fields, no Box. Outside the struct, the `TerminatorKind::Call`
arm wraps it with `target: BlockId` and `on_unwind: BlockId`
[`src/ast/ullbc_ast.rs:91-95`].

Per-field analysis relevant to D-9-3 / D-9-4 / D-9-9:

- **`func: FnOperand`** -- see §3 above. Two-arm enum
  Regular/Move; the load-bearing direct/indirect split.
- **`args: Vec<Operand>`** -- **plain `Vec<Operand>`, no per-arg
  span**. Charon does **not** preserve rustc's
  `Vec<Spanned<Operand>>` shape. The translator throws away the
  per-arg `Spanned` wrapper [`translate_bodies.rs:1174-1186`]:

```rust
fn translate_arguments(
    &mut self,
    span: Span,
    args: &Vec<hax::Spanned<hax::Operand>>,
) -> Result<Vec<Operand>, Error> {
    let mut t_args: Vec<Operand> = Vec::new();
    for arg in args.iter().map(|x| &x.node) {
        // Translate
        let op = self.translate_operand(span, arg)?;
        t_args.push(op);
    }
    Ok(t_args)
}
```

  Note `args.iter().map(|x| &x.node)`: the per-arg span (`x.span`)
  is **discarded**, only `x.node` (the operand) is translated.
  This means Charon does **not** preserve per-arg spans for
  diagnostics or error messages -- per-arg span data is lost at
  the rustc-MIR -> ULLBC boundary. Maps to **D-9-4: Charon's
  precedent is to drop per-arg spans**; the call-level span (in
  the `Terminator.span` field) is the only span retained.

- **`dest: Place`** -- mandatory. There is no
  `Option<Place>`/diverging-call shape; rustc's no-target call
  (panic-shape) is folded to `Abort(AbortKind::Panic(_))`
  upstream in `translate_function_call`
  [`translate_bodies.rs:1066-1079`]:

```rust
let panic_lang_items = &["panic", "panic_fmt", "begin_panic"];
let panic_names = &[&["core", "panicking", "assert_failed"], EXPLICIT_PANIC_NAME];

if fun_def.lang_item.as_deref().is_some_and(|lang_it| panic_lang_items.iter().contains(&lang_it))
    || panic_names.iter().any(|panic| name.equals_ref_name(panic))
{
    assert!(target.is_none());
    return Ok(TerminatorKind::Abort(AbortKind::Panic(Some(name))));
}
```

  Non-panic calls with no target (which "shouldn't" happen per the
  comment but Charon defensively handles) get a synthesised abort
  block as their target [`translate_bodies.rs:1107-1114`]:

```rust
let target = match target {
    Some(target) => self.translate_basic_block_id(*target),
    None => {
        let abort = Terminator::new(span, TerminatorKind::Abort(AbortKind::UndefinedBehavior));
        self.blocks.push(abort.into_block())
    }
};
```

- **`target: BlockId`** -- mandatory `BlockId`, never
  `Option`. The "no-return" case is encoded by making the target
  block contain only an `Abort` terminator.
- **`on_unwind: BlockId`** -- mandatory `BlockId`. Synthesised
  from rustc's `UnwindAction` by `translate_unwind_action`
  [`translate_bodies.rs:1150-1170`]:

```rust
fn translate_unwind_action(&mut self, span: Span, unwind: &UnwindAction) -> BlockId {
    let on_unwind = match unwind {
        UnwindAction::Continue => {
            let unwind_continue = Terminator::new(span, TerminatorKind::UnwindResume);
            self.blocks.push(unwind_continue.into_block())
        }
        UnwindAction::Unreachable => {
            let abort = Terminator::new(span, TerminatorKind::Abort(AbortKind::UndefinedBehavior));
            self.blocks.push(abort.into_block())
        }
        UnwindAction::Terminate(..) => {
            let abort = Terminator::new(span, TerminatorKind::Abort(AbortKind::UnwindTerminate));
            self.blocks.push(abort.into_block())
        }
        UnwindAction::Cleanup(bb) => self.translate_basic_block_id(*bb),
    };
    on_unwind
}
```

  Each `UnwindAction` arm becomes either a fresh synthesised
  block with the appropriate `UnwindResume`/`Abort` terminator,
  or a real cleanup block id. Charon's `on_unwind` field is thus
  **always** a valid block id; there is no "this call cannot
  unwind" sentinel. RegseqModel can elide unwind handling
  entirely (no panics in device-driver IR) and so does not
  inherit Charon's belt-and-braces synthesis.

**`fn_span` -- the rustc-MIR field naming the function call site
itself (separate from the terminator's span) is dropped at the
translator** [`translate_bodies.rs:898-906`]:

```rust
TerminatorKind::Call {
    fun,
    args,
    destination,
    target,
    unwind,
    fn_span: _,    // <-- explicitly discarded
    ..
} => self.translate_function_call(...)?,
```

The `fn_span: _` binding shows Charon deliberately doesn't carry
this finer span. Only the outer `terminator.source_info` span
makes it through (translated at line 856-858).

Maps to D-9-3 (Call shape: Charon's Call carries `func`, `args`,
`dest` inline -- no Box -- with `target` and `on_unwind` adjacent
in the wrapping `TerminatorKind::Call` arm), D-9-4 (per-arg span:
Charon drops per-arg spans wholesale at the translator boundary;
Charon also drops `fn_span`), D-9-9 (no-target Call collapses to
Abort).

---

## 5. Switch -- SwitchTargets storage shape

Charon's `SwitchTargets` is a **separate enum**, not a single
flat shape, defined at [`src/ast/ullbc_ast.rs:59-79`]:

```rust
#[derive(
    Debug, Clone, EnumIsA, EnumAsGetters, VariantName,
    VariantIndexArity, Serialize, Deserialize, Drive, DriveMut,
)]
#[charon::rename("Switch")]
pub enum SwitchTargets {
    /// Gives the `if` block and the `else` block
    If(BlockId, BlockId),
    /// Gives the integer type, a map linking values to switch branches, and the
    /// otherwise block. Note that matches over enumerations are performed by
    /// switching over the discriminant, which is an integer.
    SwitchInt(LiteralTy, Vec<(Literal, BlockId)>, BlockId),
}
```

Two arms:

- `If(then: BlockId, else: BlockId)` -- the boolean two-way
  branch, picked by `translate_switch_targets` for `LiteralTy::Bool`
  [`translate_bodies.rs:975-983`]. Note **the targets are
  inverted**: `assert_eq!(val.data_le_bytes, [0; 16])`, then
  `if_block = otherwise`, `then_block = target`. The MIR
  representation tags the "false" arm as a numeric value `0`, and
  Charon swaps so `If(then, else)` follows the natural reading
  order.
- `SwitchInt(switch_ty, Vec<(value, target)>, otherwise)` --
  general n-way branch. Used for `Char`, signed/unsigned integer
  types, picked at [`translate_bodies.rs:984-1034`]. **The value
  type is `Literal`, not a bag of bytes** -- Charon translates
  rustc's `ScalarInt` into the typed `Literal::Scalar(...)` /
  `Literal::char_from_le_bytes(...)` form at translation time,
  preserving the discriminator type.

The storage shape for the n-way case is a `Vec<(Literal,
BlockId)>` plus a separate `otherwise: BlockId` -- **not** rustc's
parallel-vector `(values: Vec<u128>, targets: Vec<BasicBlock>)`
shape, **not** an `IndexMap`. This is the same layout
RegseqModel's D-9-6 candidate proposes.

`SwitchTargets::get_targets` flattens to a `Vec<BlockId>`
[`src/ast/ullbc_ast_utils.rs:6-22`]:

```rust
impl SwitchTargets {
    pub fn get_targets(&self) -> Vec<BlockId> {
        match self {
            SwitchTargets::If(then_tgt, else_tgt) => {
                vec![*then_tgt, *else_tgt]
            }
            SwitchTargets::SwitchInt(_, targets, otherwise) => {
                let mut all_targets = vec![];
                for (_, target) in targets {
                    all_targets.push(*target);
                }
                all_targets.push(*otherwise);
                all_targets
            }
        }
    }
}
```

LLBC's `Switch` enum [`src/ast/llbc_ast.rs:107-136`] is a 3-arm
restructuring: `If(Operand, Block, Block)`, `SwitchInt(Operand,
LiteralTy, Vec<(Vec<Literal>, Block)>, Block)` (note the **inner
`Vec<Literal>`** -- LLBC merges grouped match arms like `E::V1 |
E::V2 => ...`), and `Match(Place, Vec<(Vec<VariantId>, Block)>,
Option<Block>)` (the resugared ADT match). For ULLBC -- the
relevant layer for D-9-6 -- the per-arm value list is *not* yet
merged; each `(Literal, BlockId)` entry is a singleton.

LLBC does carry a per-arm-value list, however: this is INFERENCE
that **if RegseqModel ever pursues a structured-CFG IR layer
(stage 11+ control-flow lift?), grouped-arm storage becomes a
real concern**. For stage 9's basic-block-shaped IR, the simpler
`Vec<(Literal, BlockId)>` shape suffices.

Maps to D-9-6 (Switch storage: Charon's `SwitchTargets` matches
the candidate shape exactly -- separate `If` and `SwitchInt`
arms, `Vec<(Value, BlockId)> + otherwise: BlockId` for the
n-way case, no parallel vectors).

---

## 6. Assert -- a Statement, not a Terminator

Charon does **not** carry `Assert` as a `TerminatorKind` variant.
This is one of the most striking deviations from rustc and from
RegseqModel's stage-9 candidate. The reasoning chain:

1. The translator initially encounters rustc's `Assert`
   terminator and **lifts it to a Statement immediately**
   [`translate_bodies.rs:907-922`]:

```rust
TerminatorKind::Assert {
    cond,
    expected,
    msg: _,
    target,
    unwind: _, // We model unwinding as an effet, we don't represent it in control flow
} => {
    let assert = Assert {
        cond: self.translate_operand(span, cond)?,
        expected: *expected,
        on_failure: AbortKind::Panic(None),
    };
    statements.push(Statement::new(span, StatementKind::Assert(assert)));
    let target = self.translate_basic_block_id(*target);
    ullbc_ast::TerminatorKind::Goto { target }
}
```

  The terminator becomes `Goto { target }`; the assert is pushed
  as a `Statement` into the same block's statement list. **No
  Terminator-level Assert exists in Charon's ULLBC.**

2. Stage 8 prior-art covered the `StatementKind::Assert(Assert)`
   variant [`src/ast/ullbc_ast.rs:41`]; the `Assert` struct
   itself is at [`src/ast/gast.rs:457-466`]:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Drive, DriveMut)]
#[charon::rename("Assertion")]
pub struct Assert {
    pub cond: Operand,
    /// The value that the operand should evaluate to for the assert to succeed.
    #[drive(skip)]
    pub expected: bool,
    /// What kind of abort happens on assert failure.
    pub on_failure: AbortKind,
}
```

  Three fields: `cond: Operand`, `expected: bool` (the value the
  operand must evaluate to for success), `on_failure: AbortKind`
  (re-uses the `AbortKind` enum from `Abort` Terminator -- so
  asserts can fail with `Panic`, `UndefinedBehavior`, or
  `UnwindTerminate`).

3. Charon has a *second* path to producing `StatementKind::Assert`:
   the resugar pass `reconstruct_asserts` at
   [`src/transform/resugar/reconstruct_asserts.rs:33-63`] folds
   the `Switch -> {Abort | continue}` panic-branch idiom into a
   `StatementKind::Assert(Assert)` plus a `Goto` terminator:

```rust
match &block.terminator.kind {
    TerminatorKind::Switch {
        discr: _,
        targets: SwitchTargets::If(bid0, bid1),
    } => {
        let (nbid, expected, abort) = if let Some(abort) = panics.get(bid0) {
            (*bid1, false, abort)
        } else if let Some(abort) = panics.get(bid1) {
            (*bid0, true, abort)
        } else {
            continue;
        };

        let kind = std::mem::replace(
            &mut block.terminator.kind,
            TerminatorKind::Goto { target: nbid },
        );
        let (discr, _) = kind.as_switch().unwrap();
        block.statements.push(Statement::new(
            block.terminator.span,
            StatementKind::Assert(Assert {
                cond: discr.clone(),
                expected,
                on_failure: abort.clone(),
            }),
        ));
    }
    _ => (),
}
```

Implications for D-9-5 (`AssertKind` subset):

- Charon's `AssertKind` analog is **`AbortKind`** -- shared
  between `Abort` Terminator and `Assert` Statement. Three
  variants only: `Panic(Option<Name>)`,
  `UndefinedBehavior`, `UnwindTerminate`. **rustc's rich
  `AssertKind` (BoundsCheck, Overflow, OverflowNeg, DivisionBy
  Zero, RemainderByZero, ResumedAfterReturn, ResumedAfterPanic,
  MisalignedPointerDereference, NullPointerDereference) is
  collapsed wholesale to `AbortKind::Panic(None)` at the
  translator** [`translate_bodies.rs:917`: `on_failure:
  AbortKind::Panic(None)`].
- The `msg: _` field of rustc's `Assert` is **discarded**; the
  diagnostic reason is lost.
- Asserts get **eliminated** in a later pass
  (`reconstruct_fallible_operations`) because they're "implicit
  in the semantics of our array accesses etc." per the
  `Assert` doc-comment [`src/ast/gast.rs:451-456`] -- and
  reintroduced in `reconstruct_asserts` for genuine user
  `assert!()` macros.

**RegseqModel's stage-9 D-9-5 framing -- "AssertKind subset" -- is
therefore subtly different from Charon's**: Charon's analogous
`AbortKind` is shared with the panic-Terminator rather than being
a Terminator-only enum. If RegseqModel keeps `Assert` at the
Terminator level (stage-9 candidate), it is the *minority* design;
Charon's choice of moving it to Statement is the alternative
shape stage 9 should at least consider.

INFERENCE: Charon's choice of "Assert is a Statement" works for
Aeneas because the verification semantics fold "this branch
panics" into a side-condition rather than a control-flow choice
-- the panic block is a sink. RegseqModel emits C drivers where
asserts may need to compile to runtime checks (or be lowered to
`__builtin_unreachable()` after RTL-level optimisation); whether
the assert is a Terminator or a Statement matters less than
whether `AssertKind` carries enough type info for backends to
choose. **Charon's evidence supports either shape, but it does
favour collapsing the rich `AssertKind` to a small enum.**

Maps to D-9-5 (AssertKind subset: Charon collapses 9 rustc
AssertKind variants to 3 AbortKind variants -- effectively just
"this is a panic, no message preserved"; RegseqModel can adopt
the same minimal shape), D-9-1 (Assert-as-Terminator vs.
Assert-as-Statement: Charon has chosen Statement; RegseqModel's
candidate keeps it as Terminator, which is a deviation worth
flagging).

---

## 7. Return / Goto / Abort / UnwindResume -- the small terminators

The four payload-light terminators:

- **`Return`** -- unit variant [`src/ast/ullbc_ast.rs:111`]. No
  payload; the return value is always written to the
  reserved-for-return local (index 0 in `Locals.locals` per
  [`src/ast/gast.rs:30-41`]). Translator origin:
  [`translate_bodies.rs:885`].
- **`Goto { target: BlockId }`** [`ullbc_ast.rs:84-86`]. Single
  field. Synthesised in **at least seven distinct places**:
  rustc `Goto` directly [`translate_bodies.rs:863-866`], rustc
  `Assert` (after lifting to Statement) [`translate_bodies.rs:921`],
  rustc `FalseEdge` [`translate_bodies.rs:923-941`], rustc
  `FalseUnwind` [`translate_bodies.rs:942-949`],
  `desugar_drops` for noop destructors
  [`desugar_drops.rs:34-37`], `filter_trivial_drops`
  [`filter_trivial_drops.rs:23`], `inline_promoted_consts`
  [`src/transform/simplify_output/inline_promoted_consts.rs:76-77`],
  `reconstruct_intrinsics` after offset_of lift
  [`reconstruct_intrinsics.rs:50`], `reconstruct_asserts`
  after assert lift [`reconstruct_asserts.rs:49`]. The pattern
  is: **whenever a Terminator collapses to "do nothing
  control-flow-wise, just continue", it becomes a `Goto`**.
- **`Abort(AbortKind)`** [`ullbc_ast.rs:110`]. Three sub-variants
  via `AbortKind`. Synthesised from rustc `Unreachable` ->
  `Abort(UndefinedBehavior)` [`translate_bodies.rs:888-890`],
  rustc `UnwindTerminate` -> `Abort(UnwindTerminate)`
  [`translate_bodies.rs:882-884`], panic-shaped no-target Calls
  -> `Abort(Panic(Some(name)))` [`translate_bodies.rs:1079`],
  defensive no-target catchall -> `Abort(UndefinedBehavior)`
  [`translate_bodies.rs:1110-1112`], `inline_local_panic_functions`
  rewrites of `panic_cold_explicit` calls
  [`inline_local_panic_functions.rs:39, 62`].
- **`UnwindResume`** [`ullbc_ast.rs:112`]. Unit variant. Direct
  translation of rustc's `UnwindResume`
  [`translate_bodies.rs:881`] and synthesised by
  `translate_unwind_action` for `UnwindAction::Continue`
  [`translate_bodies.rs:1153-1156`].

**Charon's `Abort` vs. `Unreachable` distinction**: Charon does
**not** have a separate `Unreachable` Terminator. rustc's
`Unreachable` is folded into `Abort(AbortKind::UndefinedBehavior)`
[`translate_bodies.rs:888-890`]. The "this code path is
unreachable" semantic is encoded as a *kind* of abort, not as a
distinct terminator variant. The doc-comment on `AbortKind`
[`src/ast/gast.rs:407-411`] makes this explicit:

```text
/// (U)LLBC is a language with side-effects: a statement may abort in a way that isn't tracked by
/// control-flow. The two kinds of abort are:
/// - Panic (may unwind or not depending on compilation setting);
/// - Undefined behavior:
```

So Charon classifies "panic" (controlled abort, recoverable via
unwinding in some build modes) and "undefined behavior" (uncontrolled
abort, the `Unreachable` semantic) under one umbrella.
RegseqModel's stage-9 `Unreachable` proposal corresponds to
Charon's `Abort(UndefinedBehavior)`; Charon's `Abort(Panic(_))`
has no RegseqModel analog (no panic semantics in driver specs);
Charon's `Abort(UnwindTerminate)` likewise has no analog.

Maps to D-9-1 (Abort-vs-Unreachable: Charon's two-axis encoding
-- `AbortKind` discriminator -- collapses to RegseqModel's
single-variant `Unreachable` since neither Panic nor
UnwindTerminate apply).

---

## 8. Wrapper struct -- `Terminator { kind, span, comments_before }`

Charon wraps the raw kind in a per-terminator record exactly
parallel to its `Statement` wrapper [`src/ast/ullbc_ast.rs:115-123`]:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Drive, DriveMut)]
pub struct Terminator {
    pub span: Span,
    pub kind: TerminatorKind,
    /// Comments that precede this terminator.
    // This is filled in a late pass after all the control-flow manipulation.
    #[drive(skip)]
    pub comments_before: Vec<String>,
}
```

Three fields, identical schema to `Statement` (stage-8
prior-art §2):

- `span: Span` -- per-terminator span, owned/serde-serialisable.
- `kind: TerminatorKind` -- the discriminator + payload.
- `comments_before: Vec<String>` -- comments scraped from source
  text, finalised in the late `recover_body_comments` pass.

Construction is via a single helper [`src/ast/ullbc_ast_utils.rs:34-44`]:

```rust
impl Terminator {
    pub fn new(span: Span, kind: TerminatorKind) -> Self {
        Terminator {
            span,
            kind,
            comments_before: vec![],
        }
    }
    pub fn goto(span: Span, target: BlockId) -> Self {
        Self::new(span, TerminatorKind::Goto { target })
    }
    ...
}
```

**Note that ULLBC's `Terminator` does not carry a per-instance
id**, unlike LLBC's `Statement` (which has
`id: StatementId`). Terminators are positional: each `BlockData`
has exactly one `terminator` [`ullbc_ast.rs:125-130`], so
`BlockId` is the implicit identity. The LLBC layer fuses
Statement and Terminator [`llbc_ast.rs:21-72`] under
`StatementKind` and gains `StatementId` for in-IR identity.

Stage-9 implications for D-9-8:

- **Charon wraps `TerminatorKind` in `Terminator`** with span and
  comments distinct from the raw kind. Direct precedent for
  RegseqModel's wrapper-struct candidate over flat-enum.
- **Span is separately-allocated owned, not interned.** Same
  precedent as `Statement` (stage-8 §2).
- **`comments_before: Vec<String>` is novel** to Charon, motivated
  by Aeneas's source-fidelity needs (matches stage-8's note).
  RegseqModel can ignore this for stage 9 unless source-fidelity
  becomes a stage 11/12 driver.
- **Wrapper is `Box`-free.** `Terminator` lives inline as the
  `terminator: Terminator` field of `BlockData`
  [`ullbc_ast.rs:128`]. The only Box inside is **none** at the
  Terminator level -- not even on the heaviest variant
  (`Drop { kind, place, tref, target, on_unwind }`).

Maps to D-9-7 (no Terminator interning: Charon stores
`Terminator` inline as `BlockData.terminator: Terminator`, no
arena, no `Box`), D-9-8 (wrapper struct: span + kind + comments,
exactly the proposed shape).

---

## 9. Per-Terminator translation from rustc -- the cleanup-discharge table

The translator [`translate_bodies.rs:847-962`] is the canonical
source of evidence for D-9-9 (cleanup-pass-discharged rejection
table). For each rustc `TerminatorKind<'tcx>` variant, Charon's
policy is one of: **carry** (translate to a corresponding Charon
TerminatorKind), **collapse** (fold to an existing Charon variant
+ side-effecting Statement push), **synthesise** (translate the
whole terminator into an inline-synthesised CFG of multiple
blocks/terminators), **drop** (handled by translator + cleanup
pass, becomes Goto), or **reject** (`raise_error!`, unsupported).

| rustc variant | Charon policy | file:line | Result |
|---|---|---|---|
| `Goto` | carry | `translate_bodies.rs:863-866` | `TerminatorKind::Goto { target }` |
| `SwitchInt` | carry | `translate_bodies.rs:867-880` | `TerminatorKind::Switch { discr, targets }` |
| `UnwindResume` | carry | `translate_bodies.rs:881` | `TerminatorKind::UnwindResume` |
| `UnwindTerminate { .. }` | collapse | `translate_bodies.rs:882-884` | `TerminatorKind::Abort(AbortKind::UnwindTerminate)` |
| `Return` | carry | `translate_bodies.rs:885` | `TerminatorKind::Return` |
| `Unreachable` | collapse | `translate_bodies.rs:886-890` | `TerminatorKind::Abort(AbortKind::UndefinedBehavior)` |
| `Drop { ... }` | carry | `translate_bodies.rs:891-897, 1126-1148` | `TerminatorKind::Drop { kind, place, tref, target, on_unwind }` |
| `Call { ... }` | carry / collapse-on-panic | `translate_bodies.rs:898-906, 1044-1122` | `TerminatorKind::Call { call, target, on_unwind }` (default) or `TerminatorKind::Abort(AbortKind::Panic(Some(name)))` for panic lang-items |
| `Assert { ... }` | collapse | `translate_bodies.rs:907-922` | Statement push of `StatementKind::Assert(Assert)` + Terminator becomes `Goto { target }` |
| `FalseEdge { real_target, imaginary_target }` | drop | `translate_bodies.rs:923-941` | `TerminatorKind::Goto { target: real_target }` |
| `FalseUnwind { real_target, unwind: _ }` | drop | `translate_bodies.rs:942-949` | `TerminatorKind::Goto { target: real_target }` |
| `InlineAsm { .. }` | reject | `translate_bodies.rs:950-952` | `raise_error!("Inline assembly is not supported")` |
| `CoroutineDrop` | reject | `translate_bodies.rs:953-957` | `raise_error!("Unsupported terminator: ...")` |
| `TailCall { .. }` | reject | `translate_bodies.rs:953-957` | `raise_error!("Unsupported terminator: ...")` |
| `Yield { .. }` | reject | `translate_bodies.rs:953-957` | `raise_error!("Unsupported terminator: ...")` |

`UnwindAction` translation -- the per-Call/Drop unwind side --
[`translate_bodies.rs:1150-1170`]:

| rustc UnwindAction | Charon synthesis | Result block terminator |
|---|---|---|
| `Continue` | synthesise | new block with `TerminatorKind::UnwindResume` |
| `Unreachable` | synthesise | new block with `TerminatorKind::Abort(AbortKind::UndefinedBehavior)` |
| `Terminate(...)` | synthesise | new block with `TerminatorKind::Abort(AbortKind::UnwindTerminate)` |
| `Cleanup(bb)` | carry | existing block id |

For each rustc terminator variant in {`TailCall`, `Yield`,
`CoroutineDrop`, `FalseEdge`, `FalseUnwind`, `UnwindResume`,
`UnwindTerminate`, `InlineAsm`} that D-9-9 frames:

- **`TailCall`** -- **rejected** at translator
  [`translate_bodies.rs:953-957`]. RegseqModel can adopt the same
  policy.
- **`Yield`** -- **rejected** at translator
  [`translate_bodies.rs:953-957`]. RegseqModel adopts.
- **`CoroutineDrop`** -- **rejected** at translator
  [`translate_bodies.rs:953-957`]. RegseqModel adopts.
- **`FalseEdge`** -- **dropped (collapsed to Goto)** at translator
  [`translate_bodies.rs:923-941`]. RegseqModel can adopt: these
  are borrow-checker artefacts that have no semantic content
  after MIR is finalised.
- **`FalseUnwind`** -- **dropped (collapsed to Goto)** at
  translator [`translate_bodies.rs:942-949`]. RegseqModel adopts.
- **`UnwindResume`** -- **carried** as
  `TerminatorKind::UnwindResume` [`translate_bodies.rs:881`].
  Charon keeps it because Aeneas-side reasoning examines the
  unwind shape; RegseqModel without unwinding semantics can
  reject this variant outright (it should not appear in
  `#![no_std]` device-driver code under `panic="abort"`-style
  build).
- **`UnwindTerminate`** -- **collapsed** to
  `Abort(UnwindTerminate)` [`translate_bodies.rs:882-884`].
  RegseqModel without unwinding semantics: same situation as
  `UnwindResume`; reject upstream.
- **`InlineAsm`** -- **rejected** at translator
  [`translate_bodies.rs:950-952`]. RegseqModel adopts (driver
  specs use intrinsics for register I/O, not inline asm).

INFERENCE: RegseqModel's D-9-9 rejection table is a **strict
superset** of Charon's reject set: the four variants Charon
rejects ({`TailCall`, `Yield`, `CoroutineDrop`, `InlineAsm`}) are
all uncontroversial rejects for RegseqModel, plus RegseqModel can
additionally reject `UnwindResume` and `UnwindTerminate` because
the no-unwinding policy is stricter. Charon's pattern of
lowering `UnwindAction` -> synthesised abort/resume blocks is
direct precedent for how to keep the type signature
(`on_unwind: BlockId` mandatory) while semantically discharging
the unwind concern. RegseqModel can either adopt mandatory
`on_unwind` with always-Abort synthesis, or drop the field
entirely if no-unwinding is a stage-3 invariant.

Maps to D-9-9 (cleanup-pass-discharged rejection table: Charon's
reject set is {`TailCall`, `Yield`, `CoroutineDrop`, `InlineAsm`}
at the translator with `raise_error!`; the borrow-check artefact
variants {`FalseEdge`, `FalseUnwind`} are silently dropped to
Goto; RegseqModel can subset / superset this list directly.
File:line evidence in the table above).

---

## 10. Pretty-printer (`FmtWithCtx`) -- D-9-K reference

Charon's `Terminator` pretty-printer is at
[`src/pretty/fmt_with_ctx.rs:1563-1620`]:

```rust
impl<C: AstFormatter> FmtWithCtx<C> for Terminator {
    fn fmt_with_ctx(&self, ctx: &C, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let tab = ctx.indent();
        for line in &self.comments_before {
            writeln!(f, "{tab}// {line}")?;
        }
        write!(f, "{tab}")?;
        match &self.kind {
            TerminatorKind::Goto { target } => write!(f, "goto bb{target}"),
            TerminatorKind::Switch { discr, targets } => match targets {
                SwitchTargets::If(true_block, false_block) => write!(
                    f,
                    "if {} -> bb{} else -> bb{}",
                    discr.with_ctx(ctx),
                    true_block,
                    false_block
                ),
                SwitchTargets::SwitchInt(_ty, maps, otherwise) => {
                    let maps = maps
                        .iter()
                        .map(|(v, bid)| format!("{}: bb{}", v.to_string(), bid))
                        .chain([format!("otherwise: bb{otherwise}")])
                        .format(", ");
                    write!(f, "switch {} -> {}", discr.with_ctx(ctx), maps)
                }
            },
            TerminatorKind::Call { call, target, on_unwind } => {
                let call = call.with_ctx(ctx);
                write!(f, "{call} -> bb{target} (unwind: bb{on_unwind})",)
            }
            TerminatorKind::Drop { kind, place, tref, target, on_unwind } => {
                let kind = match kind {
                    DropKind::Precise => "drop",
                    DropKind::Conditional => "conditional_drop",
                };
                write!(
                    f,
                    "{kind}[{}] {} -> bb{target} (unwind: bb{on_unwind})",
                    tref.with_ctx(ctx),
                    place.with_ctx(ctx),
                )
            }
            TerminatorKind::Abort(kind) => write!(f, "{}", kind.with_ctx(ctx)),
            TerminatorKind::Return => write!(f, "return"),
            TerminatorKind::UnwindResume => write!(f, "unwind_continue"),
        }
    }
}
```

The render shape is **textually compact**, basic-block-shaped:

- `goto bb<n>` for `Goto`
- `if <discr> -> bb<then> else -> bb<else>` for `Switch::If`
- `switch <discr> -> <v0>: bb<n0>, <v1>: bb<n1>, ..., otherwise: bb<m>` for `Switch::SwitchInt`
- `<dest> := <fnop>(<args>) -> bb<target> (unwind: bb<unwind>)` for `Call` (where the inner Call render handles `<dest> := <fnop>(<args>)` at [`fmt_with_ctx.rs:270-277`])
- `drop[<tref>] <place> -> bb<target> (unwind: bb<unwind>)` (or `conditional_drop[...]`) for `Drop`
- `panic` / `panic(<name>)` / `undefined_behavior` / `unwind_terminate` for `Abort` (via `AbortKind` formatter at [`fmt_with_ctx.rs:69-83`])
- `return` for `Return`
- `unwind_continue` for `UnwindResume`

The `Call` inner render [`fmt_with_ctx.rs:270-277`]:

```rust
impl<C: AstFormatter> FmtWithCtx<C> for Call {
    fn fmt_with_ctx(&self, ctx: &C, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let dest = self.dest.with_ctx(ctx);
        let func = self.func.with_ctx(ctx);
        let args = self.args.iter().map(|x| x.with_ctx(ctx)).format(", ");
        write!(f, "{dest} := {func}({args})")
    }
}
```

The `FnOperand` inner render [`fmt_with_ctx.rs:427-434`]:

```rust
impl<C: AstFormatter> FmtWithCtx<C> for FnOperand {
    fn fmt_with_ctx(&self, ctx: &C, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FnOperand::Regular(func) => write!(f, "{}", func.with_ctx(ctx)),
            FnOperand::Move(p) => write!(f, "(move {})", p.with_ctx(ctx)),
        }
    }
}
```

Note the post-`transform_dyn_trait_calls` rendering: an indirect
call appears as `<dest> := (move (*<recv>.ptr_metadata).method_<m>)(<args>) -> bb<target> (unwind: bb<unwind>)` --
matching the doc-comment example at the top of
`transform_dyn_trait_calls.rs:13-18`.

Implications for D-9-K:

- Charon's pretty-printer is a **single trait impl per Terminator
  variant**, no double-dispatch / visitor pattern. Direct
  precedent for RegseqModel's `Display`-style render.
- The render is **comments-prefixed**: `comments_before` lines
  are emitted as `// <comment>` before the terminator body.
  RegseqModel can ignore this (no source comments to preserve in
  driver specs).
- The formatter context (`AstFormatter`) carries indent,
  generics, locals, and crate state -- letting `Place` render
  with local names. This is heavier than D-9-K's "deferred
  pretty-printer" framing suggests; Charon front-loaded the
  formatter infrastructure across stages.
- The `bb<n>` notation for `BlockId` is a deliberate echo of
  rustc's `bb0`/`bb1`/... convention. RegseqModel can use the
  same notation for symmetry.

INFERENCE: Charon's pretty-printer is part of the IR
infrastructure from day one, not a stage-deferred component. For
RegseqModel D-9-K's "defer to stage 14" framing, Charon is
neutral evidence -- the pretty-printer was always present, but
Charon is a single-target tool (Aeneas) where IR readability is
itself a deliverable.

Maps to D-9-K (pretty-printer deferral: Charon shows the
`FmtWithCtx`-style render is small per-variant -- ~50 LOC for
the whole `Terminator` impl -- and has no fundamental
infrastructure dependencies; deferral is feasible but the
implementation cost is small).

---

## 11. Validator / well-formedness invariants

Charon does **not** ship a single dedicated terminator validator
in the form of `Validator::check_terminator(t)`. Instead, the
invariants are *enforced by construction* across multiple sites,
and a few `EnumIsA`/`EnumAsGetters`-derived predicates make
introspection cheap. The relevant in-tree machinery:

- `Terminator::is_error` [`src/ast/ullbc_ast_utils.rs:46-54`]:

```rust
pub fn is_error(&self) -> bool {
    use TerminatorKind::*;
    match &self.kind {
        Abort(..) => true,
        Goto { .. } | Switch { .. } | Return | Call { .. } | Drop { .. } | UnwindResume => false,
    }
}
```

  Classifies `Abort(_)` as the unique "this terminator is an
  unconditional error" form. Used by passes that want to know
  "does this block sink?"

- `BlockData::targets` [`src/ast/ullbc_ast_utils.rs:73-89`] and
  `BlockData::targets_ignoring_unwind` [`ullbc_ast_utils.rs:91-104`]
  -- exhaustive match over `TerminatorKind` enumerating successor
  block ids. Both are total:

```rust
pub fn targets(&self) -> Vec<BlockId> {
    match &self.terminator.kind {
        TerminatorKind::Goto { target } => vec![*target],
        TerminatorKind::Switch { targets, .. } => targets.get_targets(),
        TerminatorKind::Call { target, on_unwind, .. }
        | TerminatorKind::Drop { target, on_unwind, .. } => vec![*target, *on_unwind],
        TerminatorKind::Abort(..) | TerminatorKind::Return | TerminatorKind::UnwindResume => vec![],
    }
}
```

  Implicit invariant: **every BlockId mentioned in any terminator
  must point into the body's `BodyContents` vector**. The
  `filter_unreachable_blocks` pass
  [`src/transform/normalize/filter_unreachable_blocks.rs:13-39`]
  does a DFS using `targets()` and renumbers blocks --
  re-establishing the invariant after passes that may leave
  dangling refs.

- The `EnumIsA` macro on `TerminatorKind`
  [`src/ast/ullbc_ast.rs:82`] auto-derives `is_goto()`,
  `is_switch()`, `is_call()`, `is_drop()`, `is_abort()`,
  `is_return()`, `is_unwind_resume()`. Used by passes like
  `merge_goto_chains` [`src/transform/control_flow/merge_goto_chains.rs:42`]
  and `duplicate_return` [`duplicate_return.rs:44`].

- `EnumAsGetters` derives `as_switch()`, `as_call()`, etc. Used
  by `reconstruct_asserts.rs:51` to extract the discriminant
  Operand after a Switch terminator has already been replaced
  with Goto.

Per-variant invariants implicit in the type definition + translator:

| Invariant | Enforcement |
|---|---|
| `Goto.target` is a valid block id | translator always passes `translate_basic_block_id(...)`; `filter_unreachable_blocks` renumbers |
| `Switch.discr.ty()` matches `Switch.targets`'s `LiteralTy` | translator types the LiteralTy against `discr.ty()` at `translate_bodies.rs:973` |
| `Switch::SwitchInt.targets` is non-empty | not statically enforced; rustc's MIR guarantees at least one arm |
| `Call.target`, `Call.on_unwind` are valid block ids | translator synthesis (no-target -> abort block) |
| `Call.dest.ty()` matches the callee's return type | not statically enforced; translator types are derived from rustc |
| `Drop.tref` is a `Destruct`-trait ref | translator's `translate_trait_impl_expr` resolves it |
| `Drop.kind` is consistent with MIR phase | set globally at `BodyTransCtx.drop_kind` per command-line option |
| `AbortKind::UnwindTerminate` only follows unwind paths | not statically enforced |

Interestingly, **Charon does not enforce that
`TerminatorKind::Call.func` is *not*
`FnOperand::Regular(FnPtr{kind: Trait(Dyn, ...), ...})`** as a
post-`transform_dyn_trait_calls` invariant -- there is no
validator pass. The pass is run as a normalization step;
downstream passes simply assume the dyn case has been handled.
RegseqModel could add a check (post-stage-9 invariant: "no
direct call to a dyn-resolved trait method").

Implications for D-9-10 (invariants registry; walk deferred):

- Charon's invariants are **enforced at translator/pass
  boundaries**, not by a dedicated validator. Direct precedent
  for "deferred walk" -- the invariants list is curated, but the
  enforcement is incidental to construction.
- `EnumIsA`/`EnumAsGetters` derive macros provide the
  introspection primitives needed for ad-hoc validation. The
  RegseqModel-side analog might be a `TerminatorKind::is_*` set
  of predicates.
- The successor-set computation (`targets`) is the canonical
  invariant carrier -- a `validate_terminator` walk would mostly
  re-use this.

Maps to D-9-10 (invariants registry; walk deferred: Charon
enforces invariants by construction, has no dedicated walk; the
`EnumIsA`/`EnumAsGetters` machinery + `BlockData::targets`
function are the de facto invariant primitives).

---

## 12. Stage-9 takeaways -- D-tag-keyed bullet summary

- **D-9-1 (closed `TerminatorKind` surface).** Charon's ULLBC
  surface is **7 syntactic arms**: `Goto`, `Switch`, `Call`,
  `Drop`, `Abort`, `Return`, `UnwindResume`. RegseqModel's
  candidate is 6 (`Goto`, `Switch`, `Call`, `Return`,
  `Unreachable`, `Assert`). The two key Charon-vs-RegseqModel
  differences: (a) Charon has **no `Assert` Terminator** -- it's
  a Statement (§6); (b) Charon has **no `Unreachable`** -- folded
  into `Abort(UndefinedBehavior)` (§7); (c) Charon **keeps
  `Drop`** as a Terminator (D-9-2 below); (d) Charon **keeps
  `UnwindResume`** because Aeneas examines unwind shapes.
  RegseqModel can be stricter (no UnwindResume, no UnwindTerminate)
  given the no-unwinding stage-3 invariant.
- **D-9-2 (Drop-as-Call collapse).** Charon's default is
  **preserve `Drop`** as a Terminator with a `--desugar-drops`
  opt-in pass that rewrites it into a `Call` with `&raw mut
  place` argument and a fresh unit-typed return slot
  [`desugar_drops.rs:20-72`]. RegseqModel adopts the rewrite
  unconditionally at construction (no destructors in driver
  specs); Charon's pass body is the precedent recipe.
- **D-9-3 (Call callee shape supporting vtable-indirect
  dispatch).** **Charon definitively favors option (b)**: a
  `FnOperand::{Regular(FnPtr), Move(Place)}` enum
  [`src/ast/gast.rs:386-391`] is the load-bearing
  direct-vs-indirect split. The `transform_dyn_trait_calls` pass
  [`src/transform/normalize/transform_dyn_trait_calls.rs:31-122`]
  rewrites `<dyn Trait as Trait>::method` calls (encoded as
  `FnOperand::Regular(FnPtr{kind: Trait(Dyn, ...), ...})`) into
  `FnOperand::Move(<vtable load place>)` via a chain of
  ProjectionElems (`PtrMetadata` -> `Deref` -> `Field`). The key
  mutation is at `transform_dyn_trait_calls.rs:119`:
  `call.func = FnOperand::Move(method_field_place);`.
  **Charon does NOT use an `InstanceKind::Virtual` marker** --
  there is no such variant in `FnPtrKind`; the `TraitRefKind::Dyn`
  flag inside the `TraitRef` is a transient input form that the
  pass eliminates. This is the single load-bearing prior-art
  reference for D-9-3 (b).
- **D-9-4 (per-arg span preservation).** Charon **drops** per-arg
  spans at the rustc-MIR -> ULLBC translator boundary
  [`translate_bodies.rs:1180`: `args.iter().map(|x| &x.node)`].
  Only the call-level `Terminator.span` is preserved.
  Additionally, rustc's `fn_span` is explicitly discarded
  [`translate_bodies.rs:903`: `fn_span: _`]. RegseqModel can
  adopt the same drop policy.
- **D-9-5 (`AssertKind` subset).** Charon collapses rustc's
  9-variant `AssertKind` to a 3-variant `AbortKind` (`Panic`,
  `UndefinedBehavior`, `UnwindTerminate`) at the translator
  [`translate_bodies.rs:917`: `on_failure: AbortKind::Panic(None)`].
  The diagnostic message field (`msg`) is dropped wholesale.
  RegseqModel's stage-9 candidate -- a small AssertKind subset --
  has clear precedent.
- **D-9-6 (`SwitchTargets` storage shape).** Charon uses **two
  enum arms** [`src/ast/ullbc_ast.rs:71-79`]: `If(BlockId,
  BlockId)` for boolean two-way and `SwitchInt(LiteralTy,
  Vec<(Literal, BlockId)>, BlockId)` for n-way. Storage is
  `Vec<(value, target)>` plus a separate `otherwise: BlockId`,
  matching RegseqModel's candidate. No parallel-vector
  `(values, targets)` shape (rustc's choice); no IndexMap.
- **D-9-7 (no Terminator interning).** Charon stores `Terminator`
  inline as `BlockData.terminator: Terminator`
  [`ullbc_ast.rs:128`]. No arena, no `Box`, no interning. Direct
  precedent for RegseqModel's owned-inline storage.
- **D-9-8 (Wrapper struct shape).** Charon's `Terminator { span,
  kind, comments_before }` [`ullbc_ast.rs:115-123`] matches
  RegseqModel's proposed shape exactly. `comments_before` is the
  Charon-novel field for Aeneas source-fidelity; RegseqModel can
  omit it.
- **D-9-9 (Cleanup-pass-discharged rejection table).** Charon
  rejects (`raise_error!`) at translator: `TailCall`, `Yield`,
  `CoroutineDrop`, `InlineAsm`
  [`translate_bodies.rs:950-957`]. Drops to `Goto` (silent):
  `FalseEdge`, `FalseUnwind`. Carries: `UnwindResume`,
  `UnwindTerminate` (collapsed into `Abort(UnwindTerminate)`).
  RegseqModel can additionally reject `UnwindResume` and
  `UnwindTerminate` under the no-unwinding policy.
- **D-9-10 (Invariants registry; walk deferred).** Charon
  enforces invariants by construction at translator/pass
  boundaries; there is no dedicated terminator validator. The
  `BlockData::targets` function and `EnumIsA`/`EnumAsGetters`
  macros are the de facto invariant primitives. RegseqModel can
  follow the same deferred-walk pattern with a curated invariants
  list separate from a single validator pass.
- **D-9-K (Pretty-printer deferral).** Charon's `Terminator`
  pretty-printer is ~50 LOC at
  [`src/pretty/fmt_with_ctx.rs:1563-1620`], a single
  `FmtWithCtx` impl with a per-variant match. No
  infrastructure dependencies beyond the `AstFormatter` context
  RegseqModel will need anyway. Deferral is feasible -- the
  implementation is small and self-contained.

---

End of stage-9 prior-art-Charon document. Next stage in the
prior-art series should be the rustc primary source for
`TerminatorKind<'tcx>` and the rustc validator
(`compiler/rustc_const_eval/src/check_consts/check.rs` or the
analogous MIR validator pass), to cross-check D-9-9's reject-list
against rustc's notion of "always-rejected for const_eval".
