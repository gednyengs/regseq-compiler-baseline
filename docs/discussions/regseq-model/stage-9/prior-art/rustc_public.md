# Stage 9 -- Prior Art: rustc_public primary source

`rustc_public` (formerly `rustc_smir`, the in-tree home of the
Stable-MIR project) is the rustc-internal crate that defines an owned,
`'tcx`-free, `serde::Serialize`-able mirror of `rustc_middle`'s MIR.
Its purpose is "the WIP public interface to rustc internals... for
third-party tools to invoke" -- documented at the crate root
[`rustc_public/src/lib.rs:1-3,13-17`]:

```rust
//! The WIP public interface to rustc internals.
//!
//! For more information see <https://github.com/rust-lang/project-stable-mir>
...
//! This crate shall contain all type definitions and APIs that we
//! expect third-party tools to invoke to interact with the compiler.
//!
//! The goal is to eventually be published on
//! [crates.io](https://crates.io).
```

This makes `rustc_public` the **closest existing analog to
RegseqModel's body-level IR shape** -- both are owned, `'tcx`-free,
serde-serializable mirrors of `mir::Body<'tcx>` derived through a
hand-written translator from rustc internals. Its design choices for
the **terminator surface** (`TerminatorKind`, the `Terminator`
wrapper, `AssertMessage`, `UnwindAction`, `SwitchTargets`,
`InstanceKind`, plus the bridge translator that maps rustc's 15-variant
`TerminatorKind<'tcx>` down to rustc_public's 10-variant
`TerminatorKind`) are direct prior art for stage 9.

This file is the rustc_public prior-art evidence pass for stage 9
(`Terminator`). Stages 5 (`Place`), 6 (`Operand`), 7 (`Rvalue`), and
8 (`Statement`) are closed; stage 9 specifies the variant set
(`D-9-1`), the Drop-as-Call collapse (`D-9-2`), the call-callee shape
under vtable-indirect dispatch (`D-9-3`), per-arg span preservation
(`D-9-4`), the `AssertKind` subset (`D-9-5`), `SwitchTargets` storage
(`D-9-6`), interning policy (`D-9-7`), the wrapper struct
(`D-9-8`), the cleanup-pass-discharged rejection table (`D-9-9`),
the invariants registry (`D-9-10`), and the renderer (`D-9-K`).

All paths below are relative to the pinned-toolchain root:

```
~/.rustup/toolchains/nightly-2026-02-01-aarch64-apple-darwin/lib/rustlib/rustc-src/rust/compiler/
```

(See `../../rustc-references.md` for the canonical statement of this
root path.)

Every factual claim cites a `[file:line]` in that pinned source.
Hypotheses are flagged as **HYPOTHESIS:**. Cross-source synthesis
across two or more sites is flagged as **INFERENCE:**. Citations to
sibling prior-art files (mrustc, charon) are out of scope here -- this
file documents `rustc_public` only.

Stage 9's candidate D-tag namespace, restated for self-containment:

- **D-9-1** -- closed `TerminatorKind` surface (Regseq candidate: 6
  variants `Goto | SwitchInt | Return | Unreachable | Call | Assert`).
- **D-9-2** -- Drop-as-Call collapse at the bridge (no `Drop` in the
  stage-9 surface; rustc-side `Drop { place }` rewritten to
  `Call(drop_in_place::<T>(&mut place))`).
- **D-9-3** -- `Call.callee` shape supporting vtable-indirect
  dispatch. (a) extend `InstanceKind::Virtual { idx }`; (b) widen
  `Call.callee` to `CallCallee::{Direct(InstanceId), Indirect(Operand)}`.
- **D-9-4** -- per-arg span preservation (`Spanned<Operand>` at
  Call/TailCall level). (a) inline `ArgWithSpan { value, span }`;
  (b) parallel slabs `args: Vec<Operand>` + `arg_spans: Vec<Span>`.
- **D-9-5** -- `AssertKind` subset (8 vs 11).
- **D-9-6** -- `SwitchTargets` storage shape.
- **D-9-7** -- no Terminator interning.
- **D-9-8** -- wrapper struct `Terminator { kind, span }` (drop
  `source_info.scope`; drop Call-internal `fn_span`/`call_source`).
- **D-9-9** -- cleanup-pass-discharged rejection table (which
  rustc-side variants are rejected at the bridge, with what idiom).
- **D-9-10** -- invariants registry; walk deferred to stage 11.
- **D-9-K** -- `render_terminator` deferred to stage 11.

---

## 1. `TerminatorKind` 10-variant collapse

`rustc_public::TerminatorKind` is a closed 10-variant enum
[`rustc_public/src/mir/body.rs:153-193`]:

```rust
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum TerminatorKind {
    Goto { target: BasicBlockIdx },
    SwitchInt { discr: Operand, targets: SwitchTargets },
    Resume,
    Abort,
    Return,
    Unreachable,
    Drop {
        place: Place,
        target: BasicBlockIdx,
        unwind: UnwindAction,
    },
    Call {
        func: Operand,
        args: Vec<Operand>,
        destination: Place,
        target: Option<BasicBlockIdx>,
        unwind: UnwindAction,
    },
    Assert {
        cond: Operand,
        expected: bool,
        msg: AssertMessage,
        target: BasicBlockIdx,
        unwind: UnwindAction,
    },
    InlineAsm {
        template: String,
        operands: Vec<InlineAsmOperand>,
        options: String,
        line_spans: String,
        destination: Option<BasicBlockIdx>,
        unwind: UnwindAction,
    },
}
```

Side-by-side with rustc_middle's 15-variant `TerminatorKind<'tcx>`
[`rustc_middle/src/mir/syntax.rs:703-981`]:

| #  | rustc_middle variant     | line       | rustc_public variant | line       | Drift call |
|----|--------------------------|------------|----------------------|------------|------------|
| 1  | `Goto { target }`        | 705        | `Goto { target }`    | 155        | Renumbered (`BasicBlock` -> `BasicBlockIdx = usize`) |
| 2  | `SwitchInt { discr, targets }` | 715  | `SwitchInt { discr, targets }` | 158 | Same; `SwitchTargets` reshaped (see `body.rs:874-908`, §5) |
| 3  | `UnwindResume`           | 727        | `Resume`             | 162        | **Renamed** |
| 4  | `UnwindTerminate(reason)`| 733        | `Abort`              | 163        | **Renamed + payload dropped** (`UnwindTerminateReason` discarded) |
| 5  | `Return`                 | 746        | `Return`             | 164        | Identical |
| 6  | `Unreachable`            | 751        | `Unreachable`        | 165        | Identical |
| 7  | `Drop { place, target, unwind, replace, drop, async_fut }` | 790 | `Drop { place, target, unwind }` | 166 | **Payload simplified** -- 3 of 6 fields dropped |
| 8  | `Call { func, args: Box<[Spanned<Operand>]>, destination, target, unwind, call_source, fn_span }` | 811 | `Call { func, args: Vec<Operand>, destination, target, unwind }` | 171 | **Spanned -> bare Operand**; 2 fields dropped (`call_source`, `fn_span`) |
| 9  | `TailCall { func, args, fn_span }` | 850 | --                | --         | **Dropped** (translated to `todo!()`, see §7) |
| 10 | `Assert { cond, expected, msg: Box<AssertMessage>, target, unwind }` | 875 | `Assert { cond, expected, msg, target, unwind }` | 178 | Lossless (unbox `msg`) |
| 11 | `Yield { value, resume, resume_arg, drop }` | 900 | -- | -- | **Dropped** (`unreachable!()`, see §7) |
| 12 | `CoroutineDrop`          | 921        | --                   | --         | **Dropped** (`unreachable!()`, see §7) |
| 13 | `FalseEdge { real_target, imaginary_target }` | 929 | -- | -- | **Dropped** (`unreachable!()`, see §7) |
| 14 | `FalseUnwind { real_target, unwind }` | 944 | --       | --         | **Dropped** (`unreachable!()`, see §7) |
| 15 | `InlineAsm { asm_macro, template, operands, options, line_spans, targets, unwind }` | 957 | `InlineAsm { template: String, operands, options: String, line_spans: String, destination, unwind }` | 185 | **Lossy stringification** (`asm_macro` dropped; `template`/`options`/`line_spans` rendered as debug strings) |

Carving summary: **5 fully dropped** (`TailCall`, `Yield`,
`CoroutineDrop`, `FalseEdge`, `FalseUnwind`); **2 simplified-renamed**
(`UnwindResume -> Resume`, `UnwindTerminate(_) -> Abort` losing the
`UnwindTerminateReason`); **3 with simplified payloads** (`Drop`
loses 3 of 6 fields, `Call` loses 2 fields and demotes
`Spanned<Operand>` to bare `Operand`, `InlineAsm` collapses to
debug strings); **5 nearly-identical** (`Goto`, `SwitchInt`, `Return`,
`Unreachable`, `Assert`). 15 -> 10.

Key facts about the rustc_public surface:

- **10 variants, closed.** No `Other` / `Opaque` arm.
- **No Hash derive on `TerminatorKind`.** `derive(Clone, Debug, Eq,
  PartialEq, Serialize)` only [`body.rs:153`]. Same discipline as
  `StatementKind` -- terminators are positionally identified by
  `(BasicBlockIdx, "the terminator slot")`, not by content. This
  bears on **D-9-7** (no interning).
- **No `#[non_exhaustive]`** [`body.rs:153,139`]. rustc_middle's
  `TerminatorKind<'tcx>` is also not `non_exhaustive`
  [`rustc_middle/src/mir/syntax.rs:703`], so this is parity rather
  than divergence.
- **No size assertion.** `static_assert_size!` is not applied to
  `TerminatorKind`. Same payload-flattening freedom as `StatementKind`
  (see stage-8 prior-art §1).

Maps to **D-9-1** (variant set), **D-9-7** (no interning),
**D-9-9** (5 dropped, 2 simplified-renamed at the bridge).

---

## 2. `Terminator { kind, span }` wrapper

The `Terminator` wrapper is a flat 2-field struct
[`rustc_public/src/mir/body.rs:139-143`]:

```rust
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Terminator {
    pub kind: TerminatorKind,
    pub span: Span,
}

impl Terminator {
    pub fn successors(&self) -> Successors {
        self.kind.successors()
    }
}
```

Compare rustc_middle's wrapper
[`rustc_middle/src/mir/terminator.rs:435-439`]:

```rust
#[derive(Clone, Debug, TyEncodable, TyDecodable, HashStable, TypeFoldable, TypeVisitable)]
pub struct Terminator<'tcx> {
    pub source_info: SourceInfo,
    pub kind: TerminatorKind<'tcx>,
}
```

Two drift points:

1. **`source_info: SourceInfo` -> `span: Span`.** The `SourceInfo`
   shape is `{ span, scope }` where `scope: SourceScope` is a
   debuginfo-only index [`rustc_middle/src/mir/mod.rs:822-830`,
   matching the same shape rustc_public preserves for `VarDebugInfo`
   at `body.rs:752-756`]. The bridge translation site is the smoking
   gun [`rustc_public/src/unstable/convert/stable/mir.rs:689-702`]:

   ```rust
   impl<'tcx> Stable<'tcx> for mir::Terminator<'tcx> {
       type T = crate::mir::Terminator;
       fn stable<'cx>(...) -> Self::T {
           use crate::mir::Terminator;
           Terminator {
               kind: self.kind.stable(tables, cx),
               span: self.source_info.span.stable(tables, cx),
           }
       }
   }
   ```

   The `source_info.scope` field is **explicitly dropped on the floor
   at the terminator boundary**. This is the same carve rustc_public
   makes at the statement boundary
   (`stable/mir.rs:68-79`, see stage-8 prior-art §2): per-element
   lexical scope is debuginfo metadata, not load-bearing for an
   owned-IR consumer.

2. **No `debuginfos` field.** rustc_middle's `Statement` carries a
   trailing `debuginfos: StmtDebugInfos`
   [`rustc_middle/src/mir/statement.rs:1014-1015`]; rustc_middle's
   `Terminator` does not (see field list at
   `rustc_middle/src/mir/terminator.rs:436-439`). So unlike statements,
   terminators have no debuginfo-list to drop. INFERENCE: rustc_public
   inherits the rustc_middle parity at this point -- no extra carving
   was needed.

Direct prior art for **D-9-8**: a 2-field `{ kind, span }` wrapper is
sufficient for an owned-IR consumer; both `source_info.scope` (per-
terminator) and the Call-internal `fn_span`/`call_source` (per-Call;
see §1, table row 8) are dropped at the bridge.

Maps to **D-9-8**.

---

## 3. `InstanceKind` 4-variant narrowing -- the **Virtual** decision

rustc_public exposes `InstanceKind` as a public 4-variant enum
[`rustc_public/src/mir/mono.rs:29-40`]:

```rust
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize)]
pub enum InstanceKind {
    /// A user defined item.
    Item,
    /// A compiler intrinsic function.
    Intrinsic,
    /// A virtual function definition stored in a VTable.
    /// The `idx` field indicates the position in the VTable for this instance.
    Virtual { idx: usize },
    /// A compiler generated shim.
    Shim,
}
```

Critically, **`Virtual { idx: usize }` is a public, first-class
variant** of the public-IR `InstanceKind`. It is not relegated to the
callee-side widening of `Call`. The translation site
[`rustc_public/src/unstable/convert/stable/ty.rs:937-967`] shows the
13-variant rustc-internal `ty::InstanceKind` collapse:

```rust
let kind = match self.def {
    ty::InstanceKind::Item(..) => crate::mir::mono::InstanceKind::Item,
    ty::InstanceKind::Intrinsic(..) => crate::mir::mono::InstanceKind::Intrinsic,
    ty::InstanceKind::Virtual(_def_id, idx) => {
        crate::mir::mono::InstanceKind::Virtual { idx }
    }
    ty::InstanceKind::VTableShim(..)
    | ty::InstanceKind::ReifyShim(..)
    | ty::InstanceKind::FnPtrAddrShim(..)
    | ty::InstanceKind::ClosureOnceShim { .. }
    | ty::InstanceKind::ConstructCoroutineInClosureShim { .. }
    | ty::InstanceKind::ThreadLocalShim(..)
    | ty::InstanceKind::DropGlue(..)
    | ty::InstanceKind::CloneShim(..)
    | ty::InstanceKind::FnPtrShim(..)
    | ty::InstanceKind::FutureDropPollShim(..)
    | ty::InstanceKind::AsyncDropGlue(..)
    | ty::InstanceKind::AsyncDropGlueCtorShim(..) => crate::mir::mono::InstanceKind::Shim,
};
```

13 rustc-internal variants -> 4 public variants. `Virtual` is
preserved with payload (`idx`); 10 shim variants are collapsed into
the `Shim` umbrella with no payload distinguisher; `Item` and
`Intrinsic` are 1:1.

The `idx` field is the vtable slot index, and rustc_public's own use
sites confirm it is the operationally meaningful payload
[`rustc_public/src/mir/mono.rs:111-118`]:

```rust
pub fn intrinsic_name(&self) -> Option<Symbol> {
    match self.kind {
        InstanceKind::Intrinsic => {
            Some(with(|context| context.intrinsic(self.def.def_id()).unwrap().fn_name()))
        }
        InstanceKind::Item | InstanceKind::Virtual { .. } | InstanceKind::Shim => None,
    }
}
```

(See also `mono.rs:166`: `self.kind == InstanceKind::Shim && ...`.)

INFERENCE: rustc_public's design choice for vtable-indirect dispatch
is **to lift `Virtual` to the InstanceKind sum**, not to widen the
callee field of `Call`. A `Call.func: Operand` of type
`fn(...) -> ...` whose underlying `Instance` has `kind ==
InstanceKind::Virtual { idx }` is the encoding -- the dispatch path
is determined by inspecting the resolved instance, not by branching at
the `Call` site itself.

This is **direct prior art for D-9-3 option (a)**. The rustc_public
team chose path (a) -- `InstanceKind::Virtual { idx }` as a public
variant -- not path (b) (callee-side `CallCallee::{Direct, Indirect}`
widening). Stage 9 will need to weigh:

- Stage-1's grandfathered 6-variant Regseq `InstanceKind` (per
  `../../stage-1/spec-snapshot.md`) **omits** `Virtual`. If stage 9
  follows rustc_public's path (a), stage 1 must be re-opened to add
  `Virtual { idx }` (or a similar payload). This is the **option (a)
  consequence**.
- Path (b) keeps stage 1 closed and does the widening at the
  `Call` site instead. rustc_public does **not** model this; charon
  and mrustc prior-art (out of scope here) would need to be consulted
  for path-(b) precedent.

INFERENCE: rustc_public's choice is structurally cleaner because
**`Instance` is the unit of resolution** -- a `Virtual { idx }`
instance cannot be confused with a `Direct` call site at any other
layer (codegen, layout, ABI). Pushing the indirect-vs-direct
distinction up to `Call` adds branch points everywhere `Call` is
inspected. **HYPOTHESIS:** this is why rustc_public picked (a) over
(b).

Maps to **D-9-3**.

---

## 4. `AssertMessage` -- 11 variants, full preservation

rustc_public's `AssertMessage` is an 11-variant enum
[`rustc_public/src/mir/body.rs:260-273`]:

```rust
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum AssertMessage {
    BoundsCheck { len: Operand, index: Operand },
    Overflow(BinOp, Operand, Operand),
    OverflowNeg(Operand),
    DivisionByZero(Operand),
    RemainderByZero(Operand),
    ResumedAfterReturn(CoroutineKind),
    ResumedAfterPanic(CoroutineKind),
    ResumedAfterDrop(CoroutineKind),
    MisalignedPointerDereference { required: Operand, found: Operand },
    NullPointerDereference,
    InvalidEnumConstruction(Operand),
}
```

Compare rustc_middle's `AssertKind<O>` 11-variant generic
[`rustc_middle/src/mir/syntax.rs:1064-1078`]:

```rust
pub enum AssertKind<O> {
    BoundsCheck { len: O, index: O },
    Overflow(BinOp, O, O),
    OverflowNeg(O),
    DivisionByZero(O),
    RemainderByZero(O),
    ResumedAfterReturn(CoroutineKind),
    ResumedAfterPanic(CoroutineKind),
    ResumedAfterDrop(CoroutineKind),
    MisalignedPointerDereference { required: O, found: O },
    NullPointerDereference,
    InvalidEnumConstruction(O),
}
pub type AssertMessage<'tcx> = AssertKind<Operand<'tcx>>;
```

(rustc_middle uses `AssertMessage<'tcx>` as a type alias for
`AssertKind<Operand<'tcx>>` at `syntax.rs:1114`. rustc_public flattens
the alias and uses the variant name `AssertMessage` directly.)

The bridge translator is structurally 1:1 across all 11 variants
[`rustc_public/src/unstable/convert/stable/mir.rs:520-568`]:

```rust
impl<'tcx> Stable<'tcx> for mir::AssertMessage<'tcx> {
    type T = crate::mir::AssertMessage;
    fn stable<'cx>(...) -> Self::T {
        use rustc_middle::mir::AssertKind;
        match self {
            AssertKind::BoundsCheck { len, index } => crate::mir::AssertMessage::BoundsCheck { ... },
            AssertKind::Overflow(bin_op, op1, op2) => crate::mir::AssertMessage::Overflow(...),
            AssertKind::OverflowNeg(op) => crate::mir::AssertMessage::OverflowNeg(...),
            AssertKind::DivisionByZero(op) => crate::mir::AssertMessage::DivisionByZero(...),
            AssertKind::RemainderByZero(op) => crate::mir::AssertMessage::RemainderByZero(...),
            AssertKind::ResumedAfterReturn(coroutine) => ...ResumedAfterReturn(coroutine.stable(...)),
            AssertKind::ResumedAfterPanic(coroutine) => ...ResumedAfterPanic(coroutine.stable(...)),
            AssertKind::ResumedAfterDrop(coroutine) => ...ResumedAfterDrop(coroutine.stable(...)),
            AssertKind::MisalignedPointerDereference { required, found } => ...,
            AssertKind::NullPointerDereference => crate::mir::AssertMessage::NullPointerDereference,
            AssertKind::InvalidEnumConstruction(source) => ...InvalidEnumConstruction(...),
        }
    }
}
```

Every rustc_middle variant maps to a rustc_public variant of the same
name, same shape, same field ordering. **No collapse, no opacity, no
rejection.** The unbox of `Box<AssertMessage<'tcx>>` happens at the
`Assert` translation site
[`rustc_public/src/unstable/convert/stable/mir.rs:758-766`] -- the
inner enum is preserved verbatim.

The 11-variant surface includes 3 coroutine-relevant variants
(`ResumedAfterReturn`/`ResumedAfterPanic`/`ResumedAfterDrop`) that
discriminate over a `CoroutineKind` [`body.rs:402-422`,
3-variant: `Desugared(CoroutineDesugaring, CoroutineSource) |
Coroutine(Movability)`]. These are **the candidates for the D-9-5
"8 vs 11" gap**: under RegseqModel's no-coroutines configuration (per
README.md, "no coroutines"), a coroutine `AssertKind` cannot reach
the bridge -- it would only originate inside a coroutine body whose
`Yield`/`CoroutineDrop` terminators have already been rejected (see
§7). So the 3 coroutine asserts are unreachable under the pinned
toolchain configuration.

INFERENCE: rustc_public's choice to preserve all 11 variants is a
**representational choice** ("preserve the rustc surface") rather than
a **drop choice** ("explicitly remove things we don't model"). The
team's stance is that even if a consumer never observes a variant,
the type still mirrors rustc; carving is left to the consumer's
phase-and-config audit. RegseqModel's stage-9 candidate of 8 (drop the
3 coroutine variants) is an explicit-policy choice that diverges from
rustc_public's preserve-all stance, justified by the
no-coroutine configuration. Path (a)-equivalent here is "8 variants,
explicit drop"; path (b)-equivalent is "11 variants, runtime
unreachable" (rustc_public's path).

Note: the `description()` method
[`rustc_public/src/mir/body.rs:275-350`] returns a static
panic-message string for each variant; the coroutine arms produce
strings like `"coroutine resumed after completion"` /
`"async fn resumed after completion"`. Even the *rendering* path
preserves the coroutine variants -- this is a strong "preserve
everything" stance.

Maps to **D-9-5**.

---

## 5. `SwitchTargets` and `successors()` impl

`SwitchTargets` storage shape
[`rustc_public/src/mir/body.rs:872-908`]:

```rust
/// The possible branch sites of a [TerminatorKind::SwitchInt].
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SwitchTargets {
    /// The conditional branches where the first element represents the value
    /// that guards this branch, and the second element is the branch target.
    branches: Vec<(u128, BasicBlockIdx)>,
    /// The `otherwise` branch which will be taken in case none of the
    /// conditional branches are satisfied.
    otherwise: BasicBlockIdx,
}

impl SwitchTargets {
    pub fn all_targets(&self) -> Successors { ... }
    pub fn otherwise(&self) -> BasicBlockIdx { ... }
    pub fn branches(&self) -> impl Iterator<Item = (u128, BasicBlockIdx)> { ... }
    pub fn len(&self) -> usize { ... }
    pub fn new(branches: Vec<(u128, BasicBlockIdx)>, otherwise: BasicBlockIdx) -> SwitchTargets { ... }
}
```

Three relevant design choices:

1. **Two parallel slots, not one fused vec.** `branches:
   Vec<(u128, BasicBlockIdx)>` is a list of `(value, target)` pairs;
   `otherwise: BasicBlockIdx` is a single trailing target. This
   carving treats the otherwise-branch as a structurally distinct
   entity, not "the last element of the targets vector." Compare
   rustc_middle's encoding [`rustc_middle/src/mir/syntax.rs:1018-1031`]:

   ```rust
   pub struct SwitchTargets {
       pub(super) values: SmallVec<[Pu128; 1]>,
       pub(super) targets: SmallVec<[BasicBlock; 2]>,
   }
   ```

   rustc_middle uses two parallel `SmallVec`s with the convention that
   `targets` has length `values.len() + 1` and the trailing target is
   the otherwise-branch (the `+1` slot). rustc_public **rejects this
   convention** in favour of a `branches: Vec<(u128, BasicBlock)>` +
   explicit `otherwise: BasicBlock` shape, which makes the
   otherwise-branch a named field rather than an off-by-one
   convention. INFERENCE: this is the same kind of "tuple -> named
   record" preference rustc_public exercises on `AscribeUserType`
   (see stage-8 prior-art §4.7) -- when the tuple form encodes an
   invariant by convention, promote it to a named-field record.

2. **Bridge translation flattens rustc's `iter()` API into
   `SwitchTargets::new`** [`rustc_public/src/unstable/convert/stable/mir.rs:716-725`]:

   ```rust
   mir::TerminatorKind::SwitchInt { discr, targets } => TerminatorKind::SwitchInt {
       discr: discr.stable(tables, cx),
       targets: {
           let branches = targets.iter().map(|(val, target)| (val, target.as_usize()));
           crate::mir::SwitchTargets::new(
               branches.collect(),
               targets.otherwise().as_usize(),
           )
       },
   },
   ```

   `targets.iter()` is rustc_middle's `(value, target)` iterator;
   `targets.otherwise()` is the otherwise-branch accessor. The bridge
   reads them through rustc's public methods rather than the underlying
   `values`/`targets` SmallVec storage, giving rustc_public stable
   construction even if rustc_middle's storage shape changes.

3. **Successor enumeration via `all_targets()`**
   [`body.rs:883-887`]:

   ```rust
   pub fn all_targets(&self) -> Successors {
       self.branches.iter().map(|(_, target)| *target).chain(Some(self.otherwise)).collect()
   }
   ```

   The successor list is `branches[].1 ++ [otherwise]` -- otherwise
   appended last. This is the order consumed by `TerminatorKind::successors()`
   [`body.rs:223`: `SwitchInt { ref targets, .. } => targets.all_targets()`].

For RegseqModel **D-9-6**, the rustc_public shape is direct
prior art for the "explicit `otherwise` field" path. The alternative
"otherwise is the last element of a fused targets vec" path is
rustc_middle's. Owned-IR-with-no-size-budget favours rustc_public's
shape; size-budgeted-IR with `SmallVec` favours rustc_middle's.

Maps to **D-9-6**.

The full `TerminatorKind::successors()` impl
[`body.rs:195-225`]:

```rust
impl TerminatorKind {
    pub fn successors(&self) -> Successors {
        use self::TerminatorKind::*;
        match *self {
            Call { target: Some(t), unwind: UnwindAction::Cleanup(u), .. }
            | Drop { target: t, unwind: UnwindAction::Cleanup(u), .. }
            | Assert { target: t, unwind: UnwindAction::Cleanup(u), .. }
            | InlineAsm { destination: Some(t), unwind: UnwindAction::Cleanup(u), .. } => {
                vec![t, u]
            }
            Goto { target: t }
            | Call { target: None, unwind: UnwindAction::Cleanup(t), .. }
            | Call { target: Some(t), unwind: _, .. }
            | Drop { target: t, unwind: _, .. }
            | Assert { target: t, unwind: _, .. }
            | InlineAsm { destination: None, unwind: UnwindAction::Cleanup(t), .. }
            | InlineAsm { destination: Some(t), unwind: _, .. } => { vec![t] }
            Return | Resume | Abort | Unreachable
            | Call { target: None, unwind: _, .. }
            | InlineAsm { destination: None, unwind: _, .. } => { vec![] }
            SwitchInt { ref targets, .. } => targets.all_targets(),
        }
    }
}
```

INFERENCE: this is the canonical "successors" walk for a
terminator surface that includes `Call`/`Drop`/`Assert`/`InlineAsm`
with cleanup edges. RegseqModel's stage-9 candidate (no `Drop`, no
`InlineAsm`) collapses this to roughly:

- `Goto { t }` -> `[t]`
- `SwitchInt { targets, .. }` -> `targets.all_targets()`
- `Return | Unreachable` -> `[]`
- `Call { target: Some(t), .. }` -> `[t]`
- `Call { target: None, .. }` -> `[]`
- `Assert { target, .. }` -> `[target]` (panic=abort: no
  cleanup edge)

The stage-9 successors() impl is materially simpler than rustc_public's
because the dropped variants (`Drop`, `InlineAsm`) and the
panic-abort discharge of `unwind: Cleanup(_)` (which never appears
under panic=abort) eliminate most of the multi-target branches.

---

## 6. `UnwindAction` 4-variant + reason-discard

rustc_public's `UnwindAction`
[`rustc_public/src/mir/body.rs:252-258`]:

```rust
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize)]
pub enum UnwindAction {
    Continue,
    Unreachable,
    Terminate,
    Cleanup(BasicBlockIdx),
}
```

Compare rustc_middle [`rustc_middle/src/mir/syntax.rs:1036-1050`]:

```rust
pub enum UnwindAction {
    Continue,
    Unreachable,
    Terminate(UnwindTerminateReason),
    Cleanup(BasicBlock),
}
```

The drift is **Terminate(reason) -> Terminate** -- the
`UnwindTerminateReason` (2 variants `Abi | InCleanup`,
[`syntax.rs:1055-1061`]) is dropped at the bridge. Translation site
[`rustc_public/src/unstable/convert/stable/mir.rs:481-492`]:

```rust
impl<'tcx> Stable<'tcx> for mir::UnwindAction {
    type T = crate::mir::UnwindAction;
    fn stable(...) -> Self::T {
        use rustc_middle::mir::UnwindAction;
        match self {
            UnwindAction::Continue => crate::mir::UnwindAction::Continue,
            UnwindAction::Unreachable => crate::mir::UnwindAction::Unreachable,
            UnwindAction::Terminate(_) => crate::mir::UnwindAction::Terminate,
            UnwindAction::Cleanup(bb) => crate::mir::UnwindAction::Cleanup(bb.as_usize()),
        }
    }
}
```

The `Terminate(_)` arm explicitly pattern-binds and discards the
reason. This is **minor information loss** -- consumers can no longer
distinguish `Abi` from `InCleanup` -- but neither variant changes
runtime semantics: both terminate the process.

Direct prior art for **D-9-9** in two ways:

1. **Reason-discard at the bridge is acceptable when the discarded
   payload is diagnostic-tier.** Both `UnwindTerminateReason`
   variants and the `UnwindTerminate(reason)` -> `Abort` rename
   in `TerminatorKind` (see §1, table row 4) drop reason payloads
   without flagging it as a problem.
2. **Under `panic=abort`** (RegseqModel's pinned toolchain config),
   `UnwindAction` reduces further: `Cleanup(_)` is not generated;
   `Continue`/`Unreachable` are similarly discharge-able.
   rustc_public preserves all four variants because some consumers
   care about pre-cleanup MIR; RegseqModel can carve harder.

Maps to **D-9-9** (rejection / discharge framing).

---

## 7. The Bridge translator -- variant-by-variant decisions

The full `TerminatorKind` translation site lives at
[`rustc_public/src/unstable/convert/stable/mir.rs:704-790`]. This is
the **load-bearing prior art for D-9-9** -- it is the executable
specification of which rustc-side variants survive into rustc_public,
which are simplified, and which are rejected (with what idiom).

The complete match arm-by-arm:

```rust
impl<'tcx> Stable<'tcx> for mir::TerminatorKind<'tcx> {
    type T = crate::mir::TerminatorKind;
    fn stable<'cx>(...) -> Self::T {
        use crate::mir::TerminatorKind;
        match self {
            // ---- KEPT (5 nearly-identical) ----

            mir::TerminatorKind::Goto { target } => {                       // line 713
                TerminatorKind::Goto { target: target.as_usize() }
            }
            mir::TerminatorKind::SwitchInt { discr, targets } => TerminatorKind::SwitchInt {
                discr: discr.stable(tables, cx),                            // line 716
                targets: { ... SwitchTargets::new(...) },
            },
            mir::TerminatorKind::Return => TerminatorKind::Return,          // line 728
            mir::TerminatorKind::Unreachable => TerminatorKind::Unreachable,// line 729
            mir::TerminatorKind::Assert { cond, expected, msg, target, unwind } => {  // line 758
                TerminatorKind::Assert {
                    cond: cond.stable(tables, cx),
                    expected: *expected,
                    msg: msg.stable(tables, cx),    // unbox Box<AssertMessage>
                    target: target.as_usize(),
                    unwind: unwind.stable(tables, cx),
                }
            }

            // ---- KEPT WITH SIMPLIFICATION (3) ----

            mir::TerminatorKind::Drop {                                     // line 730
                place,
                target,
                unwind,
                replace: _,        // dropped
                drop: _,           // dropped (async drop)
                async_fut: _,      // dropped (async drop)
            } => TerminatorKind::Drop {
                place: place.stable(tables, cx),
                target: target.as_usize(),
                unwind: unwind.stable(tables, cx),
            },

            mir::TerminatorKind::Call {                                     // line 742
                func,
                args,
                destination,
                target,
                unwind,
                call_source: _,    // dropped
                fn_span: _,        // dropped
            } => TerminatorKind::Call {
                func: func.stable(tables, cx),
                args: args.iter().map(|arg| arg.node.stable(tables, cx)).collect(),
                                    //  ^^^^^^^^ Spanned<Operand> -> bare Operand
                destination: destination.stable(tables, cx),
                target: target.map(|t| t.as_usize()),
                unwind: unwind.stable(tables, cx),
            },

            mir::TerminatorKind::InlineAsm {                                // line 767
                asm_macro: _,        // dropped (asm! vs naked_asm! distinguisher)
                template,
                operands,
                options,
                line_spans,
                targets,
                unwind,
            } => TerminatorKind::InlineAsm {
                template: format!("{template:?}"),       // debug-string stub
                operands: operands.iter().map(|operand| operand.stable(tables, cx)).collect(),
                options: format!("{options:?}"),         // debug-string stub
                line_spans: format!("{line_spans:?}"),   // debug-string stub
                // FIXME: Figure out how to do labels in SMIR
                destination: targets.first().map(|d| d.as_usize()),  // first target only
                unwind: unwind.stable(tables, cx),
            },

            // ---- RENAMED + REASON DROPPED (2) ----

            mir::TerminatorKind::UnwindResume => TerminatorKind::Resume,    // line 726
            mir::TerminatorKind::UnwindTerminate(_) => TerminatorKind::Abort,// line 727

            // ---- TODO!() (1) ----

            mir::TerminatorKind::TailCall { func: _, args: _, fn_span: _ } => todo!(), // line 757

            // ---- UNREACHABLE!() (4) ----

            mir::TerminatorKind::Yield { .. }                               // line 784
            | mir::TerminatorKind::CoroutineDrop                            // line 785
            | mir::TerminatorKind::FalseEdge { .. }                         // line 786
            | mir::TerminatorKind::FalseUnwind { .. } => unreachable!(),    // line 787
        }
    }
}
```

The five drop decisions split into two idiom classes:

1. **`unreachable!()` -- 4 variants**. `Yield`, `CoroutineDrop`,
   `FalseEdge`, `FalseUnwind` are explicitly classified by rustc_public
   as "should never reach the public surface." The implicit assumption
   is that these are pre-runtime / pre-cleanup-pass-discharge artefacts:

   - `Yield` and `CoroutineDrop` -- rustc_middle says of `Yield` "Not
     permitted in bodies that are not coroutine bodies, or after
     coroutine lowering" [`syntax.rs:897`]. After
     `StateTransform` runs, coroutine bodies are lowered to
     state-machines and these variants no longer appear. rustc_public
     consumers reading optimized_mir don't see them.
   - `FalseEdge` and `FalseUnwind` -- "Disallowed after drop
     elaboration" [`syntax.rs:928,943`]. They exist for borrowck's
     drop-flag analysis and are eliminated before runtime MIR.

2. **`todo!()` -- 1 variant**. `TailCall` is left as a runtime
   panic, indicating the rustc_public team has **not yet implemented**
   the translation, not that they have rejected it. INFERENCE: tail
   calls are still an experimental MIR feature (the
   `explicit_tail_calls` feature gate is unstable); the rustc_public
   team is waiting for the rustc-side surface to stabilize before
   committing to a public translation. This is **a different
   discipline from `unreachable!()`**: `unreachable!()` is a
   correctness-precondition assertion; `todo!()` is an incomplete
   implementation marker.

For **D-9-9**, RegseqModel's rejection table can borrow the
classification:

| rustc-side variant   | rustc_public idiom | Stage-9 discharge        |
|----------------------|--------------------|--------------------------|
| `Yield { .. }`       | `unreachable!()`   | no-coroutines policy: bridge `unsupported_construct!` |
| `CoroutineDrop`      | `unreachable!()`   | no-coroutines policy: bridge `unsupported_construct!` |
| `FalseEdge { .. }`   | `unreachable!()`   | drop-elaboration discharge: `unreachable!()` |
| `FalseUnwind { .. }` | `unreachable!()`   | drop-elaboration discharge: `unreachable!()` |
| `TailCall { .. }`    | `todo!()`          | bridge `unsupported_construct!` (no `become` in driver specs) |
| `InlineAsm { .. }`   | preserved (lossy)  | bridge `unsupported_construct!` (no inline asm in driver specs) |

The rustc_public team's choice is "preserve InlineAsm with debug-string
payloads"; RegseqModel's stage-9 candidate is "reject InlineAsm at the
bridge." The rustc_public team's choice is "preserve TailCall as
`todo!()`"; RegseqModel rejects it. **HYPOTHESIS:** these divergences
are justified by RegseqModel's tighter input regime (driver-spec
crates with no inline asm, no tail calls) and tighter consumer
contract (a C-emitting backend that has no encoding for either).

Note: **`Drop` is preserved by rustc_public** but is rejected by
RegseqModel's stage-9 candidate via the **D-9-2 Drop-as-Call collapse**.
rustc_public has no equivalent of D-9-2; its 10-variant surface
includes `Drop` because consumers may still want to inspect the
drop-glue dispatch separately from a `core::mem::drop_in_place` call.
RegseqModel's choice to collapse to `Call` is a stage-9 design
decision **not** anticipated by rustc_public.

Maps to **D-9-2** (Drop preserved by rustc_public; Regseq diverges)
and **D-9-9** (rejection table for the 5 dropped variants and the
rejection-but-preservable variants).

---

## 8. `Tables<'tcx, B>` and the ID-pattern

The bridge maintains 8 interned families
[`rustc_public_bridge/src/lib.rs:42-54`]:

```rust
pub trait Cacheable = Copy + Debug + PartialEq + IndexedVal;

pub struct Tables<'tcx, B: Bridge> {
    pub def_ids: IndexMap<DefId, B::DefId>,
    pub alloc_ids: IndexMap<AllocId, B::AllocId>,
    pub spans: IndexMap<rustc_span::Span, B::Span>,
    pub types: IndexMap<Ty<'tcx>, B::Ty>,
    pub instances: IndexMap<ty::Instance<'tcx>, B::InstanceDef>,
    pub ty_consts: IndexMap<ty::Const<'tcx>, B::TyConstId>,
    pub mir_consts: IndexMap<mir::Const<'tcx>, B::MirConstId>,
    pub layouts: IndexMap<rustc_abi::Layout<'tcx>, B::Layout>,
}
```

**No terminator family.** Same discipline as `StatementKind` (see
stage-8 prior-art §3): terminators are **positionally** identified by
`(BasicBlockIdx, "the terminator slot")`, not by content-hash. The 8
interned families are all identity-bearing or
canonicalization-bearing data (DefIds, AllocIds, Spans, Types,
Instances, TyConsts, MirConsts, Layouts) -- terminator-shaped data
isn't.

The dispatch wiring is the `with(|cx| ...)` thread-local pattern
[`rustc_public/src/compiler_interface.rs:844,852-867`]:

```rust
scoped_tls::scoped_thread_local!(static TLV: Cell<*const ()>);
...
pub(crate) fn with<R>(f: impl for<'tcx>
    FnOnce(&CompilerInterface<'tcx>) -> R) -> R
{
    assert!(TLV.is_set());
    TLV.with(|tlv| {
        let ptr = tlv.get();
        assert!(!ptr.is_null());
        f(unsafe { *(ptr as *const &CompilerInterface<'_>) })
    })
}
```

The `TLV` (thread-local value) carries the active `CompilerInterface`
which holds `tables: RefCell<Tables<'tcx, BridgeTys>>` and `cx:
RefCell<CompilerCtxt<'tcx, BridgeTys>>`
[`rustc_public/src/compiler_interface.rs:75-78`]. The
`CompilerCtxt<'tcx, B>` itself
[`rustc_public_bridge/src/context/mod.rs:23-26`] is a thin
`{ tcx: TyCtxt<'tcx>, _marker: PhantomData<B> }` -- the ID-lookup
context.

INFERENCE: the **opaque-IDs-with-CompilerCtxt-lookup pattern** is
the structural shape of the bridge:

- Owned IR data (`Statement`, `Terminator`, `BasicBlock`, `Body`,
  `TerminatorKind` variants) lives inline in serializable structures
  that hold opaque IDs (`B::DefId`, `B::Ty`, ...).
- Operations that need to inspect the underlying rustc data (resolve
  a `DefId` to a `Ty`, evaluate a const, get an `FnAbi`) call
  `with(|cx| cx.foo(id))` to enter a `'tcx`-scoped callback that
  reads `tables` / `cx`.

RegseqModel takes the **eager-translation path** (Q2, per stages 5-8
context): instead of preserving opaque IDs and doing late lookup, it
resolves everything at construction time so the resulting structure
needs no `'tcx` context to interpret. The two patterns are
complementary -- rustc_public's pattern is precedent for the
**owned-IR-with-deferred-resolution** path; RegseqModel diverges by
choosing eager.

Note that for **terminator-level data specifically**, the only opaque
IDs reaching the public surface through `TerminatorKind` are:

- `Operand` (carried in `SwitchInt.discr`, `Call.func`/`args`,
  `Assert.cond`/`msg.*`) -- reaches `MirConst` IDs and Place /
  Local references. (See stage-6 prior-art for `Operand` shape.)
- `Place` (`Drop.place`, `Call.destination`, `InlineAsmOperand.out_place`).
- `BasicBlockIdx` (every target field) -- this is `usize`, not an
  opaque ID.

So the terminator surface itself doesn't add new families to
`Tables`; it borrows the families established by the operand- and
place-level IRs.

Maps to **D-9-7** (no Terminator interning) and confirms the
structural separation between owned-data and interned-handle halves.

---

## 9. InlineAsm collapse -- "preserve and lossily render"

rustc_public's `InlineAsm` arm
[`rustc_public/src/mir/body.rs:185-192`]:

```rust
InlineAsm {
    template: String,
    operands: Vec<InlineAsmOperand>,
    options: String,
    line_spans: String,
    destination: Option<BasicBlockIdx>,
    unwind: UnwindAction,
},
```

with the `InlineAsmOperand` payload struct
[`rustc_public/src/mir/body.rs:243-250`]:

```rust
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct InlineAsmOperand {
    pub in_value: Option<Operand>,
    pub out_place: Option<Place>,
    // This field has a raw debug representation of MIR's InlineAsmOperand.
    // For now we care about place/operand + the rest in a debug format.
    pub raw_rpr: String,
}
```

Three observations:

1. **`template`/`options`/`line_spans` are debug-formatted strings.**
   The bridge wraps with `format!("{x:?}")`
   [`rustc_public/src/unstable/convert/stable/mir.rs:776-779`]:

   ```rust
   template: format!("{template:?}"),
   ...
   options: format!("{options:?}"),
   line_spans: format!("{line_spans:?}"),
   ```

   The structured rustc-side payloads (`&'tcx [InlineAsmTemplatePiece]`,
   `InlineAsmOptions`, `&'tcx [Span]`) are flattened to debug strings.
   This is a lossy preservation -- re-parsing the structured form is
   not possible.

2. **`asm_macro` is dropped entirely.** The rustc-side
   `asm_macro: InlineAsmMacro` field [`syntax.rs:959`] (which
   distinguishes `asm!` from `naked_asm!`) is pattern-bound as
   `asm_macro: _` and discarded. INFERENCE: rustc_public consumers
   are not expected to need the macro distinction.

3. **`targets` collapses to `destination: Option<BasicBlockIdx>`,
   first target only**, with the inline FIXME
   [`stable/mir.rs:780-781`]:

   ```rust
   // FIXME: Figure out how to do labels in SMIR
   destination: targets.first().map(|d| d.as_usize()),
   ```

   rustc_middle's `targets: Box<[BasicBlock]>` is the full label list
   for `asm!`'s `label = ...` operands; rustc_public takes only the
   first (the fall-through destination). This is also a lossy
   preservation, with the FIXME flagging that the rustc_public team
   knows it needs proper label support but hasn't built it yet.

The `InlineAsmOperand` translation
[`stable/mir.rs:662-687`] is structurally similar -- it preserves
`in_value`/`out_place` from `In`/`Out`/`InOut` operand kinds, but
collapses `Const`/`SymFn`/`SymStatic`/`Label` operand kinds to
`(None, None)` while stashing the full debug string in `raw_rpr`:

```rust
let (in_value, out_place) = match self {
    InlineAsmOperand::In { value, .. } => (Some(value.stable(tables, cx)), None),
    InlineAsmOperand::Out { place, .. } => (None, place.map(|place| place.stable(tables, cx))),
    InlineAsmOperand::InOut { in_value, out_place, .. } => (...),
    InlineAsmOperand::Const { .. } | InlineAsmOperand::SymFn { .. }
        | InlineAsmOperand::SymStatic { .. } | InlineAsmOperand::Label { .. }
        => (None, None),
};
crate::mir::InlineAsmOperand { in_value, out_place, raw_rpr: format!("{self:?}") }
```

INFERENCE: rustc_public's "preserve and lossily render" stance for
`InlineAsm` is the **opposite design choice** from RegseqModel's
planned outright rejection at **D-9-9**. The rustc_public team
preserves the variant because some consumers might want to know
"there's some inline asm here" for diagnostics or analysis purposes;
RegseqModel rejects because no driver-spec body contains inline asm
under the no_std + Rust-only policy, and the C backend has no
encoding for it anyway.

Maps to **D-9-1** (variant set: rustc_public has 10, RegseqModel has 6
-- the 4 dropped are `Resume`/`Abort`/`Drop`/`InlineAsm`) and
**D-9-9** (rejection idiom for InlineAsm).

---

## 10. No Terminator interning

`Body` and `BasicBlock` carry terminators inline, not via a separate
interner [`rustc_public/src/mir/body.rs:13-38, 133-137`]:

```rust
#[derive(Clone, Debug, Serialize)]
pub struct Body {
    pub blocks: Vec<BasicBlock>,
    pub(super) locals: LocalDecls,
    pub(super) arg_count: usize,
    pub var_debug_info: Vec<VarDebugInfo>,
    pub(super) spread_arg: Option<Local>,
    pub span: Span,
}
...
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct BasicBlock {
    pub statements: Vec<Statement>,
    pub terminator: Terminator,
}
```

**Each `BasicBlock` owns exactly one `Terminator` directly** --
`pub terminator: Terminator`, not `pub terminator: TerminatorId` or
`pub terminator: Box<Terminator>` or `pub terminator: Arc<Terminator>`.
The terminator lives **inline** with the block.

This is consistent with:

- **No `Hash` derive on `Terminator` or `TerminatorKind`**
  [`body.rs:139,153`]: `Clone, Debug, Eq, PartialEq, Serialize` only.
  Same discipline as `Statement` / `StatementKind` (stage-8 prior-art
  §3): you need `Hash` to intern, and rustc_public doesn't intern
  terminators, so it doesn't need `Hash`.
- **No terminator slot in `Tables`** (§8): the 8 interned families
  do not include terminators.
- **No size assertion**: `static_assert_size!(TerminatorKind<'_>, ...)`
  is **not** mirrored in rustc_public. (The corresponding rustc_middle
  `TerminatorKind` does not appear to carry a size assert in
  `syntax.rs` either.)

INFERENCE: a positional location (block index + "the terminator slot")
is sufficient identity for a terminator. Terminator *position* matters
(execution edge identity, CFG); terminator *content equality*
does not (no consumer needs to dedup terminators). Same discipline as
statements, but with the additional structural fact that **each block
has exactly one terminator** -- terminators are even more positionally
unique than statements.

Maps directly to **D-9-7**: RegseqModel inherits the rustc_public
choice -- inline `Terminator` in `BasicBlock`, no
`TerminatorInterner`, no `Hash` derive.

---

## 11. Pretty-printer surface (`pretty_terminator`)

rustc_public has a pretty-printer for terminators at
[`rustc_public/src/mir/pretty.rs:128-227`] that is **substantially
more complete** than the corresponding `pretty_statement`
(stage-8 prior-art §6, which had `// FIX-ME: Make them pretty.`
markers for 3 variants).

The full `pretty_terminator` orchestration
[`pretty.rs:128-169`]:

```rust
fn pretty_terminator<W: Write>(writer: &mut W, terminator: &TerminatorKind) -> io::Result<()> {
    pretty_terminator_head(writer, terminator)?;
    let successors = terminator.successors();
    let successor_count = successors.len();
    let labels = pretty_successor_labels(terminator);

    let show_unwind = !matches!(terminator.unwind(), None | Some(UnwindAction::Cleanup(_)));
    let fmt_unwind = |w: &mut W| -> io::Result<()> {
        write!(w, "unwind ")?;
        match terminator.unwind() {
            None | Some(UnwindAction::Cleanup(_)) => unreachable!(),
            Some(UnwindAction::Continue) => write!(w, "continue"),
            Some(UnwindAction::Unreachable) => write!(w, "unreachable"),
            Some(UnwindAction::Terminate) => write!(w, "terminate"),
        }
    };

    match (successor_count, show_unwind) {
        (0, false) => {}
        (0, true) => { write!(writer, " -> ")?; fmt_unwind(writer)?; }
        (1, false) => write!(writer, " -> bb{:?}", successors[0])?,
        _ => {
            write!(writer, " -> [")?;
            for (i, target) in successors.iter().enumerate() { ... }
            if show_unwind { write!(writer, ", ")?; fmt_unwind(writer)?; }
            write!(writer, "]")?;
        }
    };

    writeln!(writer, ";")
}
```

The renderer composes three pieces:

1. **`pretty_terminator_head`** [`pretty.rs:171-202`] -- emits the
   per-variant lead-in:

   ```
   goto                          // Goto
   switchInt(<discr>)            // SwitchInt
   resume                        // Resume
   abort                         // Abort
   return                        // Return
   unreachable                   // Unreachable
   drop(<place>)                 // Drop
   <dest> = <func>(<args>)       // Call
   assert(<cond>, <msg>)         // Assert
   InlineAsm                     // InlineAsm (placeholder only)
   ```

   All 10 variants are handled. Only `InlineAsm` is rendered as a
   bare placeholder (no operand or template printout).

2. **`pretty_successor_labels`** [`pretty.rs:204-227`] -- emits the
   per-edge labels (`return`/`unwind`/`success`/`goto`/`otherwise`/
   the SwitchInt value).

3. **Edge orchestration** -- composes the lead-in + successor list
   with `-> bb{N}` or `-> [label1: bbX, label2: bbY, unwind ...]`
   syntax, suppressing the unwind sub-clause when it is
   `None | Cleanup(_)` (i.e., when the unwind edge is already in the
   successor list).

INFERENCE: unlike `pretty_statement`'s 3 partially-implemented
variants, **`pretty_terminator` is essentially complete** -- only the
`InlineAsm` lead-in is a stub. This is a significant maturity
difference between the statement renderer and the terminator
renderer. Stage 11's render scope (per `D-9-K` deferral) inherits a
clearer reference: the rustc_public team has worked out a stable
external-text shape for terminators that statements don't yet have.

The signature is `fn pretty_terminator<W: Write>(writer: &mut W,
terminator: &TerminatorKind)`, taking `&TerminatorKind` rather than
`&Terminator` -- the wrapping span is dropped at the rendering level.
This is the same discipline `pretty_statement` follows (stage-8
prior-art §6, observation 3): `span` is metadata, not a renderable
concept in MIR text format.

Maps to **D-9-K** (deferred to stage 11; rustc_public is the
reference shape).

---

## 12. Synthesis: stage-9 takeaways keyed by D-tag

What rustc_public's `TerminatorKind` design says about each stage-9
question:

- **D-9-1 (closed surface, 6 variants).** rustc_public closed at 10;
  RegseqModel's candidate is 6. The 4-variant gap
  (`Resume`/`Abort`/`Drop`/`InlineAsm`) is fully consequential: each
  has explicit prior-art treatment in §1's table. `Resume`/`Abort` are
  unwind-machinery variants discharged by `panic=abort`;
  `Drop` is collapsed to `Call` per D-9-2; `InlineAsm` is rejected as
  out-of-scope for driver specs. All four diverge from rustc_public's
  preserve-and-render stance via input-regime + consumer-contract
  tightening. **6-variant closed surface is the right shape for
  RegseqModel's input.**

- **D-9-2 (Drop-as-Call collapse).** rustc_public **does not** make
  this collapse -- it preserves `Drop` as a distinct variant
  [`body.rs:166-170`]. RegseqModel's stage-9 candidate diverges. The
  rustc-side documentation [`rustc_middle/src/mir/syntax.rs:753-757`]
  explicitly states that "After drop elaboration: `Drop` terminators
  are a complete nop for types that have no drop glue. For other
  types, `Drop` terminators behave exactly like a call to
  `core::mem::drop_in_place` with a pointer to the given place." This
  *justifies* RegseqModel's collapse but does not constitute prior
  art for it -- rustc_public could have collapsed but didn't, because
  preserving the distinction is useful for analysis-tier consumers.
  **RegseqModel's D-9-2 collapse is a fresh design choice, not an
  inheritance from rustc_public.**

- **D-9-3 (Call callee shape).** rustc_public picks **path (a)** --
  `InstanceKind::Virtual { idx: usize }` lifted to a public variant
  of `InstanceKind` [`mono.rs:36-37`], not callee-side widening of
  `Call`. The vtable index lives at the instance-resolution layer,
  not the call-site layer. **Strong prior art for path (a).** Stage 9
  should follow rustc_public's path (a); the cost is reopening
  stage 1's grandfathered `InstanceKind` to add `Virtual { idx }`.

- **D-9-4 (per-arg span).** rustc_public **does NOT preserve**
  per-arg spans. The bridge translation
  [`stable/mir.rs:752`] writes
  `args: args.iter().map(|arg| arg.node.stable(tables, cx)).collect()`
  -- `arg.node` strips the `Spanned<Operand>` wrapping and only the
  bare `Operand` survives. The `Spanned`'s `span` field is dropped on
  the floor at the bridge. **rustc_public is prior art for "drop
  per-arg spans"** (a third option not in D-9-4's framing of (a)
  inline `ArgWithSpan` vs (b) parallel slabs). If RegseqModel decides
  to preserve per-arg spans, it diverges from rustc_public; if it
  decides to drop them, it inherits the rustc_public stance verbatim.
  Note: rustc_public's drop is consistent with its broader carve at
  §2 (the wrapper `source_info.scope` is also dropped), suggesting
  the rustc_public team's stance is that **only the terminator-level
  `span` is load-bearing for an owned-IR consumer**, not finer-grained
  spans.

- **D-9-5 (AssertKind subset).** rustc_public preserves all 11
  variants verbatim [`body.rs:260-273`, `stable/mir.rs:520-568`].
  RegseqModel's candidate is 8 (drop the 3 coroutine variants under
  the no-coroutines policy). **rustc_public is anti-precedent here**
  -- it explicitly chose to preserve over carve. RegseqModel's 8-
  variant carve is consistent with the no-coroutines configuration
  but represents an explicit-policy carve rustc_public deliberately
  declined to make.

- **D-9-6 (SwitchTargets storage).** rustc_public uses
  `branches: Vec<(u128, BasicBlockIdx)>` + explicit `otherwise:
  BasicBlockIdx` [`body.rs:872-908`], **not** rustc_middle's
  parallel-SmallVec convention. The `all_targets()` method enumerates
  branches first, otherwise last [`body.rs:885-887`]. **Direct prior
  art for explicit-otherwise shape**; RegseqModel can adopt verbatim.

- **D-9-7 (no Terminator interning).** rustc_public stores
  terminators inline (`pub terminator: Terminator` in `BasicBlock`,
  [`body.rs:135-137`]). No `Hash` derive on `Terminator` /
  `TerminatorKind`. No `terminator` family in `Tables`. **Direct
  prior art for inline storage**; RegseqModel inherits.

- **D-9-8 (wrapper struct).** rustc_public's wrapper is `Terminator
  { kind: TerminatorKind, span: Span }` [`body.rs:139-143`], with the
  bridge dropping `source_info.scope` at construction
  [`stable/mir.rs:689-702`]. Inside `Call`, `call_source` and
  `fn_span` are also dropped [`stable/mir.rs:748-749`]. **Direct
  prior art for the 2-field wrapper + Call-internal field drops**;
  RegseqModel inherits.

- **D-9-9 (cleanup-pass-discharged rejection).** rustc_public's
  `unreachable!()` arm
  [`stable/mir.rs:784-787`] rejects `Yield`/`CoroutineDrop`/
  `FalseEdge`/`FalseUnwind` outright at the bridge. The `todo!()` arm
  [`stable/mir.rs:757`] rejects `TailCall`. The `UnwindResume ->
  Resume` and `UnwindTerminate(_) -> Abort` renames discard
  reason payloads. The `Drop`-arm `replace`/`drop`/`async_fut`
  field-drops [`stable/mir.rs:730-741`] and the `Call`-arm
  `call_source`/`fn_span` field-drops [`stable/mir.rs:742-756`] are
  silent omissions. **Direct prior art for at least 5 rejection
  decisions.** RegseqModel's stage-9 rejection table can borrow the
  classification (`unreachable!()` for borrowck-phase artefacts;
  `unsupported_construct!`-equivalent for explicit policy
  rejections like InlineAsm).

- **D-9-10 (invariants registry; walk).** rustc_public's
  `super_terminator` visitor
  [`visit.rs:202-243`] is the canonical walk shape for the
  10-variant surface. It destructures `Terminator { kind, span }`
  literally, visits the span, then matches on `TerminatorKind`. The
  body-level walk site [`visit.rs:140-145`] passes
  `Location(terminator.span)` -- terminator location is identified
  by span, mirroring statements (stage-8 prior-art §5). Stage 11
  inherits this walk shape.

- **D-9-K (renderer).** rustc_public's `pretty_terminator`
  [`pretty.rs:128-227`] is **substantially more complete** than
  `pretty_statement` (which had FIXMEs). All 10 variants have
  per-variant lead-ins, full successor-label rendering, and edge
  orchestration with unwind-clause suppression. Only `InlineAsm`
  is a bare placeholder. **Direct prior art for stage-11's
  `render_terminator`**; the rustc_public renderer's structural
  decomposition (head + successor labels + edge orchestration) is
  reusable.

| Tag        | rustc_public evidence anchor | RegseqModel position |
|------------|------------------------------|----------------------|
| **D-9-1**  | 10-variant enum [`body.rs:153-193`] | 6-variant subset (drop `Resume`, `Abort`, `Drop`, `InlineAsm`) |
| **D-9-2**  | Preserves `Drop` [`body.rs:166-170`] | Diverge: collapse to `Call` |
| **D-9-3**  | `InstanceKind::Virtual { idx }` [`mono.rs:36-37`] | Adopt path (a); reopen stage 1 |
| **D-9-4**  | Strips `Spanned<Operand>` to bare `Operand` [`stable/mir.rs:752`] | Stage 9 must decide: preserve (diverge) or drop (inherit) |
| **D-9-5**  | All 11 variants [`body.rs:260-273`] | Diverge: 8-variant subset under no-coroutines policy |
| **D-9-6**  | `branches + otherwise` shape [`body.rs:872-908`] | Adopt shape verbatim |
| **D-9-7**  | Inline `Terminator` in `BasicBlock` [`body.rs:135-137`]; no `Tables` slot [`bridge/lib.rs:42-54`] | Adopt inline storage |
| **D-9-8**  | `Terminator { kind, span }` [`body.rs:139-143`]; drops scope/fn_span/call_source | Adopt 2-field shape |
| **D-9-9**  | `unreachable!()` for 4 [`stable/mir.rs:784-787`]; `todo!()` for 1 [`stable/mir.rs:757`]; reason-discard for 2 [`stable/mir.rs:726-727`] | Adopt classification; extend with explicit `unsupported_construct!` for `InlineAsm` and `TailCall` |
| **D-9-10** | `super_terminator` walk [`visit.rs:202-243`] | Defer to stage 11; reuse walk shape |
| **D-9-K**  | `pretty_terminator` complete except InlineAsm [`pretty.rs:128-227`] | Defer to stage 11; rustc_public is the reference |

This rustc_public.md is the kickoff evidence pass; mid-stage deeper
reads on specific D-tags will land additional citations as needed
(e.g., for `D-9-3` corroboration via charon/mrustc on path (a)
vs (b), for `D-9-4` exact `Spanned` vs `arg_spans` shape parity with
the eager-resolution discipline of stages 5-7, for `D-9-9`
`unsupported_construct!` site parity with stages 5-8).
