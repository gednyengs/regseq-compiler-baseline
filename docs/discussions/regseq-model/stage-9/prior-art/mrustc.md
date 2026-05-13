# Stage 9 -- Prior Art: mrustc primary source

Primary-source mrustc evidence for the **MIR Terminator
analog vocabulary**, scoped to what stage 9's candidate
decision namespace
(`D-9-1` closed `TerminatorKind` surface; `D-9-2`
Drop-as-Call collapse; `D-9-3` callee shape with vtable-
indirect dispatch; `D-9-4` per-arg span preservation;
`D-9-5` `AssertKind` subset; `D-9-6` `SwitchTargets`
storage shape; `D-9-7` no Terminator interning;
`D-9-8` wrapper struct shape; `D-9-9` cleanup-pass-
discharged rejection table; `D-9-10` invariants registry
+ deferred walk; `D-9-K` pretty-printer deferral) needs.

mrustc is the closest existing peer: a Rust-to-C compiler
with its own HIR/MIR, written in C++. Its
`MIR::Terminator` and surrounding types directly inform
the same design space as RegseqModel's `Terminator`. The
stage-6, stage-7, and stage-8 prior-art mrustc files
(`./../../stage-6/prior-art/mrustc.md`,
`./../../stage-7/prior-art/mrustc.md`,
`./../../stage-8/prior-art/mrustc.md`) already covered the
`LValue`, `Param`, `Constant`, `RValue`, `eBinOp`,
`eUniOp`, and `Statement` surfaces; this file does not
re-derive that material. It focuses on the `Terminator`
tagged union, its construction site, validation, codegen-C
consumer dispatch, cleanup-pass behaviour, and pretty-
printing -- topics those earlier files did not exercise.

Repository: https://github.com/thepowersgang/mrustc

Master HEAD as of this pass: commit
`7392eca5bd4958cb184fb47cefbd4c7e4f43547b` (committer date
2026-04-13). All file:line citations refer to that tree.

Primary files inspected:

- `src/mir/mir.hpp` -- IR type definitions
  (`Terminator`, `CallTarget`, `SwitchValues`,
  `BasicBlock`, `Function`, `Cloner::clone_term`).
- `src/mir/mir_builder.cpp` -- `MirBuilder::end_block`,
  drop-statement emission; the construction-side handle
  for terminators.
- `src/mir/from_hir.cpp` -- HIR-to-MIR construction,
  `Terminator::make_*` call sites for every variant.
- `src/mir/from_hir_match.cpp` -- match-lowering's heavy
  use of `Terminator::make_If` for range / guard
  comparisons.
- `src/mir/check.cpp` -- `MIR_Validate` per-Terminator
  rules (CFA reachability sweep + flat type checks +
  `MIR_Validate_ValState` move-tracking pass).
- `src/mir/check_full.cpp` -- `MIR_Validate_Full` deep
  per-Terminator state-tracking with leak detection.
- `src/mir/cleanup.cpp` -- `MIR_Cleanup` pass; vtable-
  call lowering (`MIR_Cleanup_Virtualize`) and Fn-trait
  lowering, both of which rewrite `Call.fcn`.
- `src/mir/dump.cpp` -- per-Terminator dump rendering
  (`MirDumper::dump_mir`).
- `src/mir/mir.cpp` -- `Terminator::operator<<`,
  `operator==`, `Cloner::clone_term`.
- `src/trans/codegen_c.cpp` -- C-emit consumer
  (`emit_terminator`-like dispatch starting around
  line 3236; `emit_term_call`, `emit_term_switch`,
  `emit_term_switchvalue`, `emit_destructor_call`).
- `src/trans/enumerate.cpp` -- drop-glue function
  enumeration; the `#drop_glue` synthetic path.
- `src/trans/auto_impls.cpp` -- drop-glue function body
  synthesis (the MIR for `<T>::#drop_glue`).

Every factual claim cites `[file:line]` in the mrustc
tree. Hypotheses are flagged **HYPOTHESIS:**. Cross-
source synthesis is flagged **INFERENCE:**.

Stage-context anchor: this file does not claim parity with
rustc, rustc_public, or charon -- those are sibling
prior-art passes. mrustc consumes a much older rustc and
predates significant post-MIR work in modern rustc; drift
is expected, and where mrustc made a different design
choice, that divergence is itself the evidence relevant to
a Rust-to-C peer compiler.

---

## 1. mrustc `MIR::Terminator` tagged union -- full variant table

mrustc's `Terminator` is a 9-variant tagged union
[`src/mir/mir.hpp:615-643`]:

```cpp
TAGGED_UNION(Terminator, Incomplete,
    (Incomplete, struct {}),    // Block isn't complete (ERROR in output)
    (Return, struct {}),    // Return clealy to caller
    (Diverge, struct {}),   // Continue unwinding up the stack
    (Goto, BasicBlockId),   // Jump to another block
    (Panic, struct { BasicBlockId dst; }),  // ?
    (If, struct { LValue cond; BasicBlockId bb_true; BasicBlockId bb_false; }),
    (Switch, struct { LValue val; ::std::vector<BasicBlockId> targets; }),
    (SwitchValue, struct {
        LValue  val;
        BasicBlockId    def_target;
        ::std::vector<BasicBlockId> targets;
        SwitchValues    values;
        }),
    (Call, struct {
        BasicBlockId    ret_block;
        BasicBlockId    panic_block;
        LValue  ret_val;
        CallTarget  fcn;
        ::std::vector<Param>   args;
        })
    );
```

Variant tally (9 arms) and payload semantics:

| Variant | Payload | Semantics |
|--|--|--|
| `Incomplete` | `struct {}` | Builder-bug sentinel; validator rejects (§7). C-emit: `for(;;);` [`src/trans/codegen_c.cpp:3238-3240`]. |
| `Return` | `struct {}` | Return to caller via implicit return slot `LValue::new_Return()`. |
| `Diverge` | `struct {}` | Continue unwinding up the call stack; emitted as `_Unwind_Resume();` [`src/trans/codegen_c.cpp:3249`]. |
| `Goto` | `BasicBlockId` (**bare integer, not struct-wrapped**) | Unconditional jump. |
| `Panic` | `struct { BasicBlockId dst; }` | Source-commented `// ?`. C-emit: `goto bb<n>; /* panic */` [`src/trans/codegen_c.cpp:3262`] -- semantically identical to Goto. |
| `If` | `{ LValue cond; BasicBlockId bb_true; BasicBlockId bb_false; }` | Two-way branch on a `bool`-typed LValue (validator enforces; §7). |
| `Switch` | `{ LValue val; vector<BasicBlockId> targets; }` | Branch on enum discriminant; `targets[i]` corresponds to variant index `i`. No default arm. |
| `SwitchValue` | `{ LValue val; BasicBlockId def_target; vector<BasicBlockId> targets; SwitchValues values; }` | Branch on integer / string / byte-string value, with explicit default. |
| `Call` | `{ BasicBlockId ret_block; BasicBlockId panic_block; LValue ret_val; CallTarget fcn; vector<Param> args; }` | Function call with both success (`ret_block`) and unwind (`panic_block`) successors. |

`Cloner::clone_term` exhaustively case-splits all 9
variants [`src/mir/mir.cpp:1023-1078`].

Notable differences vs rustc's modern
`mir::TerminatorKind`:

- **No `Drop` terminator.** Drop is a Statement
  [`src/mir/mir.hpp:710-715`, stage-8 prior-art §1].
  Detail in §2.
- **No `Assert` terminator.** mrustc lowers bounds /
  overflow checks to explicit `If` + `Diverge` during
  HIR-to-MIR [`src/mir/from_hir.cpp:1888-1896`]; the
  source comment at line 1895
  `//m_builder.end_block( ::MIR::Terminator::make_Panic({}) );`
  shows mrustc once used `Panic` and switched to
  `Diverge`. Detail in §6, §11.
- **No `Yield` / `GeneratorDrop` / `CoroutineDrop`.**
  Generators lowered to Switch-on-state-lvalue
  [`src/mir/from_hir.cpp:127`,
  `src/mir/from_hir.cpp:150`,
  `src/mir/from_hir.cpp:233`].
- **No `FalseEdge` / `FalseUnwind`.** No borrowck-
  internal control-flow markers.
- **No `InlineAsm` terminator.** Inline asm is two
  Statement variants
  [`src/mir/mir.hpp:673-684`, stage-8 prior-art §1].
- **No `UnwindResume` / `UnwindTerminate` distinction.**
  `Diverge` covers both.
- **`Incomplete`, `Panic`, `If` are mrustc-specific.**
  rustc's `SwitchInt(bool)` covers `If`'s role; rustc
  has no `Incomplete` (uses `bug!()` instead) and no
  first-class `Panic` (panic-edge attached to Call /
  Drop / Assert).

Closest rustc-Terminator mapping for each mrustc variant:

| mrustc | rustc analog |
|--|--|
| `Incomplete` | (none) |
| `Return` | `Return` |
| `Diverge` | `UnwindResume` |
| `Goto` | `Goto` |
| `Panic` | (panic edge of Call/Drop/Assert) |
| `If` | `SwitchInt` (2 targets) |
| `Switch` | `SwitchInt` (enum discriminant) |
| `SwitchValue` | `SwitchInt` (integer/string literal) |
| `Call` | `Call` (+ Drop, Assert, InlineAsm absorbed elsewhere) |

Maps to **D-9-1** (closed `TerminatorKind` surface) and
informs D-9-2 through D-9-6 per-variant comparisons.

---

## 2. Drop terminator + drop-glue -- mrustc keeps Drop as a Statement, codegen inlines destructor

mrustc has **no** `Drop` terminator. Drop is a Statement
variant
[`src/mir/mir.hpp:710-715`]:

```cpp
(Drop, struct {
    eDropKind   kind;   // NOTE: For the `box` primitive
    LValue  slot;
    unsigned int flag_idx;  // Valid if != ~0u
    }),
```

with `eDropKind` being `{SHALLOW, DEEP}`
[`src/mir/mir.hpp:662-665`]. Constructed via
`MirBuilder::push_stmt_drop`
[`src/mir/mir_builder.cpp:408-418`] and
`push_stmt_drop_shallow`
[`src/mir/mir_builder.cpp:419-426`] -- both
short-circuit for `Copy` types and emit a
`Statement::make_Drop(...)`, never a Terminator.

Codegen-C handles `Statement::Drop` **inline at the
statement level**
[`src/trans/codegen_c.cpp:3725-3756`]: a `flag_idx`-
guarded `if( df<n> ) { ... }` wrapper, then per-`eDropKind`
dispatch -- SHALLOW calls `emit_box_drop` for owned-Box
types (else `MIR_BUG`), DEEP calls `emit_destructor_call`.

Crucially: a Drop **does not become a `Call`**.
`emit_destructor_call`
[`src/trans/codegen_c.cpp:7783-7869`] **inlines a
direct C function call to the drop-glue mangle**
(`Trans_Mangle(p) << "(&" << slot << ");"` where
`p == HIR::Path(ty.clone(), "#drop_glue")`
[`src/trans/codegen_c.cpp:7819`,
`src/trans/codegen_c.cpp:7844`]). The destructor call
appears in C as a single `<mangle>(&slot);` line
embedded in the run of statements; control flow does
not break on the drop. `emit_destructor_call`
case-splits on the type (Path / Borrow / Array / Tuple
/ TraitObject / Slice) and recurses into compound types
to call inner destructors directly
[`src/trans/codegen_c.cpp:7791-7920`].

Drop-glue itself is a **synthesised function** with a
canonical path:

- Enumeration registers `HIR::Path(ty, "#drop_glue")`
  [`src/trans/enumerate.cpp:698`,
  `src/trans/enumerate.cpp:705-707`,
  `src/trans/enumerate.cpp:764-766`,
  `src/trans/auto_impls.cpp:51`,
  `src/trans/auto_impls.cpp:730`,
  `src/trans/auto_impls.cpp:802`,
  `src/trans/auto_impls.cpp:930`].
- Body synthesis: `auto_impls.cpp:933-961` constructs a
  `HIR::Function` taking a `&own ty` argument, then
  builds the MIR with `MirBuilder` -- using
  `Statement::make_Drop` for inner-field drops
  [`src/trans/auto_impls.cpp:952`,
  `src/trans/auto_impls.cpp:959`]. Recursion bottoms
  out via `type_needs_drop_glue` early-exit
  [`src/trans/codegen_c.cpp:7786`].

INFERENCE: mrustc's choice is "Drop is a Statement at
the IR level, the C backend inlines a
`<drop_glue_mangle>(&slot);` call at Statement-emit
time, and the drop glue itself is synthesised by
auto_impls before codegen." Drop is **not** a basic-
block boundary -- no panic edge, no return BB, no
out-arg LValue. Unwind-on-Drop-panic is absorbed
implicitly: a destructor that panics keeps panicking
through the C stack via its own `Diverge` ->
`_Unwind_Resume()` path.

Direct structural divergence from both rustc and D-9-2:

| | rustc | mrustc | RegseqModel D-9-2 plan |
|--|--|--|--|
| Drop location | Terminator | Statement | (after collapse) Call terminator |
| Drop has panic edge | Yes | No | Yes (inherits Call's) |
| Drop-glue lookup | Resolved at codegen | Resolved at enumerate (auto_impls) | At collapse-time bridge resolves Instance |
| C lowering | per-target codegen lowers Drop terminator | inline C destructor call inside Statement loop | Uniform Call codegen handles drop |

mrustc's evidence: a Rust-to-C peer **can** drop the
Drop variant from the Terminator surface (mrustc puts
it in Statements), **but** pays for that by making the
C backend treat Drop specially. RegseqModel's D-9-2
collapse goes the other way -- keep the C backend
uniform (sees only Calls), normalise in the bridge.
**HYPOTHESIS:** D-9-2 is cleaner for a
"store-not-walk" IR because it avoids encoding C-emit-
time policy in the IR semantics.

Maps to **D-9-2** (Drop-as-Call collapse). Direct peer
evidence that the alternative -- preserving Drop as a
Statement and inlining at emit time -- is workable but
pushes drop-glue policy into the backend.

---

## 3. Call terminator shape -- `CallTarget` 3-arm union; vtable handled by cleanup pass

mrustc's `CallTarget` is a 3-variant tagged union
[`src/mir/mir.hpp:595-602`]:

```cpp
TAGGED_UNION(CallTarget, Intrinsic,
    (Value, LValue),
    (Path,  ::HIR::Path),
    (Intrinsic, struct {
        RcString   name;
        ::HIR::PathParams   params;
        })
    );
```

Three callee-shape arms:

- **`Value(LValue)`** -- indirect call through a fn-ptr
  or vtable-loaded slot. Validator enforces
  `ty.data().is_Function()`
  [`src/mir/check.cpp:1255-1264`]; the inline comment
  "NOTE: VTable functions use this, and have a little
  bit of type shenanigans going on" confirms vtable-
  indirect dispatch reuses the same `Value(LValue)` arm.
  **No dedicated `Virtual` arm.**
- **`Path(::HIR::Path)`** -- direct call to a named
  function path. Validator enforces signature
  compatibility (arg-count, per-arg type equality, ret
  type) for `Generic` / `UfcsKnown` / `UfcsInherent`
  paths [`src/mir/check.cpp:1283-1335`]. Variadic-fn
  handled.
- **`Intrinsic{name: RcString, params: HIR::PathParams}`**
  -- compiler intrinsic (stage-8 prior-art §4 covered
  the routing rationale).

C-emit dispatch [`src/trans/codegen_c.cpp:4766-4851`]:
`Value` -> `(<lvalue>)(args)`; `Path` -> `<mangle>(args)`;
`Intrinsic` -> `emit_intrinsic_call(...)` early-return.

**Vtable lowering happens in the cleanup pass**, not at
construction or codegen. `MIR_Cleanup` rewrites
`Call.fcn` from `Path(<dyn Trait as Trait>::method)` to
`Value(<vtable_loaded_lvalue>)`
[`src/mir/cleanup.cpp:1357-1381`]. The detector keys on
`path.m_data.is_UfcsKnown() &&
path.m_data.as_UfcsKnown().type.data().is_TraitObject()`,
then calls `MIR_Cleanup_Virtualize` and assigns the
returned LValue to `e.fcn`.

`MIR_Cleanup_Virtualize`
[`src/mir/cleanup.cpp:548-684`] does the work:

1. Computes the vtable index
   `trait.get_vtable_value_index(pe.trait.m_path, pe.item)`
   [`src/mir/cleanup.cpp:562`].
2. Synthesises a vtable pointer type
   [`src/mir/cleanup.cpp:567`] and allocates a temporary
   local [`src/mir/cleanup.cpp:581`].
3. Synthesises an
   `LValue::new_Field(LValue::new_Deref(vtable_lv), vtable_idx)`
   loading the fn-ptr slot
   [`src/mir/cleanup.cpp:582`].
4. Fixes up the receiver (Box / `&dyn` / `&mut dyn` /
   by-value `&move` borrow)
   [`src/mir/cleanup.cpp:572-578`,
   `src/mir/cleanup.cpp:649-676`].
5. Pushes Statements that materialise the vtable
   pointer load
   (`RValue::make_DstMeta` to extract the metadata,
   stored into the temp)
   [`src/mir/cleanup.cpp:679-680`].
6. Returns the new `fcn_lval`; the caller flips
   `e.fcn` from `Path(...)` to `Value(fcn_lval)`
   [`src/mir/cleanup.cpp:1379`].

INFERENCE: mrustc's design choice is "callee
discriminator at the IR level is 3 arms (`Value` /
`Path` / `Intrinsic`); vtable dispatch reuses `Value` by
synthesising a vtable load before the call, in a
cleanup pass." No `Virtual` callee-arm, no
`InstanceKind::Virtual` analog -- the *call* is
ordinary indirect, the *resolution* is encoded as a
sequence of cleanup-pass-inserted Statements.

Direct peer evidence relevant to **D-9-3** (both
options):

| Concern | rustc | mrustc | D-9-3 (a) | D-9-3 (b) |
|--|--|--|--|--|
| Direct call shape | `func: Operand::Constant` | `Path(HIR::Path)` | `Direct(InstanceId)` | `Direct(InstanceId)` |
| Indirect call (fn ptr) | `func: Operand::Move(_lv)` | `Value(LValue)` | -- | `Indirect(Operand)` |
| Virtual call lowering | `InstanceDef::Virtual(idx)` -- vtable load at codegen | `Value(LValue)` after cleanup-pass vtable load | `Virtual(InstanceId, idx)` arm | `Indirect(Operand)` after IR-level vtable load |
| When vtable load happens | At codegen | At MIR cleanup | At post-bridge cleanup | At post-bridge cleanup |

mrustc's stance: vtable resolution is **a pre-codegen
MIR transform**, not a permanent IR shape. A
RegseqModel following mrustc would make `Call.callee` a
3-arm union analogue and put vtable-load synthesis in a
cleanup pass. D-9-3 (a)+(b) diverges by: (a) hoisting
"this call is virtual" into a stable IR-level shape
(`InstanceKind::Virtual`) rather than cleanup-discharged;
(b) using `Operand` rather than `LValue` for the
indirect-call value (Operand carries Move/Copy;
mrustc's `Value(LValue)` is implicitly move-by-not-
copying).

**HYPOTHESIS:** RegseqModel keeps Virtual as a stable
IR shape because the bridge consumes rustc's
monomorphised MIR (which already distinguishes virtual
calls via `InstanceDef::Virtual`); information-
preservation argues for retaining that distinction.

Note on Fn-trait lowering: mrustc also rewrites
`Fn`/`FnMut`/`FnOnce` calls in cleanup
[`src/mir/cleanup.cpp:1383-1449`] -- `Fn`/`FnMut` flip
`e.fcn` to `Value(Deref(closure))`; `FnOnce` flips to
`Value(closure)`; both redistribute the args tuple.
Same pattern as the vtable case: call-site rewrite, not
stable IR variant.

Maps to **D-9-3** (callee shape with vtable-indirect
dispatch). Direct peer evidence that vtable resolution
can be cleanup-discharged; RegseqModel's choice to
surface it as `InstanceKind::Virtual` is a deliberate
divergence.

---

## 4. Per-arg span / source-info -- absent at Terminator level

mrustc's `Call.args` is `std::vector<Param>`
[`src/mir/mir.hpp:641`], with `Param` a 3-arm union
(`LValue | Borrow{type,val} | Constant`)
[`src/mir/mir.hpp:498-514`]. There is **no
`Spanned<Param>` wrapping**, no sidecar `vector<Span>`
parallel to `args`, and `Terminator` /
`BasicBlock` carry no span fields
[`src/mir/mir.hpp:615-643`,
`src/mir/mir.hpp:726-730`]. Same posture as mrustc's
`Statement` (stage-8 prior-art §6).

Construction-side: `MirBuilder::end_block(::MIR::Terminator term)`
[`src/mir/mir_builder.cpp:795-804`] **takes no Span
argument**. The current span context is taken
implicitly from the calling HIR-walker but is not
persisted onto the Terminator; the `BUG(Span(), ...)`
failure path uses an empty span.

INFERENCE: mrustc deliberately drops Terminator-level
spans and therefore per-arg spans on Call.
Diagnostics that need to point at "argument 2 of this
Call" cannot do so post-construction.

For **D-9-4**: mrustc evidence shows *no* per-arg span
is workable in a Rust-to-C transpiler. RegseqModel's
proposed `Spanned<Operand>` per Call arg is strictly
more diagnostic information than mrustc retains, at
the cost of a slightly larger payload. Independent
design choice.

Maps to **D-9-4** (per-arg span preservation).

---

## 5. If / Switch / SwitchValue -- three switch-style terminators

mrustc has three switch-shaped terminators
[`src/mir/mir.hpp:621-635`]:

**`If { LValue cond; BasicBlockId bb_true, bb_false; }`**
-- two-target branch on a `bool` LValue. Validator
enforces `ty == ::HIR::CoreType::Bool`
[`src/mir/check.cpp:1241-1248`]. C-emit
[`src/trans/codegen_c.cpp:3264-3266`] is `if(<lv>) goto
bb<t>; else goto bb<f>;`.

**`Switch { LValue val; vector<BasicBlockId> targets; }`**
-- branch on enum discriminant; **single
`vector<BasicBlockId>` only**, no values vector. The
value at index `i` is implicitly variant index `i`.
Dense, one-target-per-variant; no default arm.
Validator is a comment-only TODO `// Check that the
condition is an enum`
[`src/mir/check.cpp:1249-1251`].

**`SwitchValue`** -- branch on integer / string /
byte-string literal; explicit default; with a separate
`SwitchValues` 4-arm union payload
[`src/mir/mir.hpp:603-613`,
`src/mir/mir.hpp:630-635`]:

```cpp
TAGGED_UNION_EX(SwitchValues, (), Unsigned, (
    (Unsigned, ::std::vector<uint64_t>),
    (Signed, ::std::vector<int64_t>),
    (String, ::std::vector<::std::string>),
    (ByteString, ::std::vector<::std::vector<uint8_t>>)
    ), ...);
(SwitchValue, struct {
    LValue  val;
    BasicBlockId    def_target;
    ::std::vector<BasicBlockId> targets;
    SwitchValues    values;
    }),
```

Storage: `targets: vector<BasicBlockId>` paired with
`values: SwitchValues` (4-arm union of typed literal
vectors); plus a separate `def_target`. The `targets`
and `values` vectors are **two separate parallel
vectors** with the invariant that `targets[i]`
corresponds to `values.{Unsigned,Signed,...}[i]`
(maintained by the dump renderer
[`src/mir/dump.cpp:172-211`] and C emit
[`src/trans/codegen_c.cpp:4624-4705`]). Validator is a
comment-only TODO `// Check that the condition's type
matches the values` [`src/mir/check.cpp:1252-1254`].

For **D-9-6** (SwitchTargets storage shape), mrustc
presents three different shapes:

1. `If` -- 2 BBs, no values list (bool implicit).
2. `Switch` -- N BBs, no values list (variant index
   implicit).
3. `SwitchValue` -- N BBs paired with N values, plus
   explicit default.

rustc's modern `SwitchInt` uses a single
`SwitchTargets` holding paired `(value, target)` for
non-default arms plus a default -- closer to mrustc's
`SwitchValue` than to `Switch`.

INFERENCE: mrustc's parallel-array choice has the
practical drawback that consistency must be maintained
by every walker
[`src/mir/dump.cpp:172-211`,
`src/trans/codegen_c.cpp:4624-4705`,
`src/mir/check.cpp:605-619`,
`src/mir/check_full.cpp:985-991`]. Every iterator
indexes `values` with the same `i` as `targets`.
Brittle but cheap; the paired-vec alternative is more
cache-friendly per-arm but bloats trivial-layout cases.

Maps to **D-9-6** (SwitchTargets storage shape).
mrustc evidence: parallel arrays work; the
`SwitchValues` 4-arm union externalises the literal-
type tag once for the whole switch instead of per-arm.

---

## 6. Panic / Diverge / Return / Goto / Incomplete -- the small terminators

**`Goto(BasicBlockId)`** [`src/mir/mir.hpp:619`] -- the
payload is the BB id *directly*, not wrapped in a
struct. Mrustc-idiosyncratic. C-emit
[`src/trans/codegen_c.cpp:3251-3260`] applies a
fall-through optimisation: if the goto target is the
next block in linear order, no `goto` is emitted (let C
control-flow drop into the next block).

**`Return`** [`src/mir/mir.hpp:617`] -- empty struct;
C-emit `return ;` (unit) or `return rv;`
[`src/trans/codegen_c.cpp:3241-3247`].

**`Diverge`** [`src/mir/mir.hpp:618`] -- empty struct;
mrustc's universal "unwind continues" terminator,
unifying rustc's `UnwindResume` and the unreachable-
code case. C-emit
[`src/trans/codegen_c.cpp:3248-3250`]: `_Unwind_Resume();`.

**`Panic{ BasicBlockId dst }`**
[`src/mir/mir.hpp:620`] -- C-emit
[`src/trans/codegen_c.cpp:3261-3263`]: `goto bb<n>; /*
panic */` -- semantically identical to `Goto`. The
validator's CFA reachability sweep treats it like
`Goto` [`src/mir/check.cpp:709-711`]. The
`from_hir.cpp:1895` commented-out
`make_Panic({})` superseded by `make_Diverge({})`
suggests `Panic` is at-most a debug-marker that mrustc
is migrating away from.

**`Incomplete`** [`src/mir/mir.hpp:616`] -- empty
struct; default value of a fresh `Terminator` (default
arm of the `TAGGED_UNION` macro). Sentinel for
"builder forgot to call `end_block`". Validator rejects
[`src/mir/check.cpp:697-699`,
`src/mir/check.cpp:666`]; C-emit `for(;;);`
[`src/trans/codegen_c.cpp:3238-3240`] as fail-stop
defensive output for bugs past the validator.

For RegseqModel:

- **`Return`**: direct match to D-9-1's `Return`.
- **`Diverge`** maps to D-9-1's `Unreachable`. mrustc
  emits `_Unwind_Resume()` (unwind continuation);
  RegseqModel's `Unreachable` should emit `abort()` or
  `__builtin_unreachable()` -- the unwind-resume role
  is absorbed by Call panic edges under D-9-2's drop
  collapse. **HYPOTHESIS:** the semantic split is
  intentional -- after RegseqModel's drop-elaboration
  collapses Drop into Call, the "no panic edge here"
  case is genuinely unreachable, not a resume point.
- **`Goto`**: direct match.
- **`Panic`**: dropped from D-9-1; mrustc itself is
  phasing it out in favour of `Diverge`.
- **`Incomplete`**: builder-specific; RegseqModel
  doesn't need it (the bridge constructs Terminators
  directly from rustc's monomorphised MIR; no
  incremental `end_block` API).

mrustc's 5 small terminators (Goto, Return, Diverge,
Panic, Incomplete) collapse to RegseqModel's 3 (Goto,
Return, Unreachable) once Panic and Incomplete are
dropped.

Maps to **D-9-1** (closed `TerminatorKind` surface).

---

## 7. Validator (`MIR_Validate`) per-Terminator rules

mrustc's `MIR_Validate` does three Terminator-relevant
sweeps:

**Sweep A: CFA reachability**
[`src/mir/check.cpp:670-737`]. Iterates BBs reachable
from BB0; rejects `Incomplete` outright; for every
other Terminator, pushes its successor BB(s) onto the
visit stack. The `PUSH_BB` macro
[`src/mir/check.cpp:690-695`] enforces
`idx < fcn.blocks.size()` for every successor reference.
Per-variant successor extraction
[`src/mir/check.cpp:696-731`]:

- `Incomplete` -> `MIR_BUG`.
- `Return` -> marks the function as returning.
- `Diverge`, `Panic` -> no/single successor.
- `Goto` -> push `e`.
- `If` -> push `bb_true`, `bb_false`.
- `Switch` -> push every entry of `targets`.
- `SwitchValue` -> push every `targets[i]` plus
  `def_target`.
- `Call` -> push `ret_block`, `panic_block`.

Per-variant invariants captured: all BB ids referenced
by any Terminator are in-bounds; `Incomplete` is
rejected; the function must have at least one
returning path. No structural check on
`Switch.targets.size() == enum-variant-count` here.

**Sweep B: flat per-Terminator type checks**
[`src/mir/check.cpp:1229-1338`]:

- `Incomplete`, `Return`, `Diverge`, `Goto`, `Panic`:
  no-op (bb-id validity covered by sweep A).
- `If`: condition-LValue type must equal
  `::HIR::CoreType::Bool`
  [`src/mir/check.cpp:1241-1248`].
- `Switch`: comment-only TODO `// Check that the
  condition is an enum`
  [`src/mir/check.cpp:1249-1251`].
- `SwitchValue`: comment-only TODO `// Check that the
  condition's type matches the values`
  [`src/mir/check.cpp:1252-1254`].
- `Call.fcn.is_Value()`: lvalue type must be
  `is_Function()` [`src/mir/check.cpp:1255-1264`].
  Arg-count and arg/ret-type checks for the Value arm
  are `#if 0`'d [`src/mir/check.cpp:1265-1281`] with
  the comment "VTable functions use this, and have a
  little bit of type shenanigans going on".
- `Call.fcn.is_Path()`: full signature check (arg-
  count, per-arg type equality, return-slot type
  equality) with `!`-type carve-out
  [`src/mir/check.cpp:1283-1335`]. Variadic-fn handled.

**Sweep C: value-state (move tracking)
`MIR_Validate_ValState`** [`src/mir/check.cpp:563-633`].
Per-variant transitions on the per-local validity
vector:

- `Return`: `ensure_valid(LValue::new_Return())`.
- `If` / `Switch` / `SwitchValue`: condition LValue
  must be valid; successor blocks visited with
  copy/move-distinguished state.
- `Call`: callee LValue (Value-arm) must be valid;
  every arg is `move_val`'d (validity-cleared if not
  Copy); panic-block sees pre-ret state; ret-block
  sees ret_val marked valid.
- `Diverge`, `Panic`: comment-only TODOs (`/* TODO:
  cleanup */`, `/* What should be done here? */`).

**Sweep D: full-state validation `MIR_Validate_Full`**
[`src/mir/check_full.cpp:937-1014`]:

- Drop-leak detector at `Return`
  [`src/mir/check_full.cpp:945-963`] (gated behind
  `ENABLE_LEAK_DETECTOR`): every non-Copy local at
  function exit must have been dropped.
- `Call`: panic_block-skip optimisation if
  panic_block is a single-statement `Diverge`
  [`src/mir/check_full.cpp:1005-1010`].

For **D-9-10** (invariants registry; walk deferred to
stage 11), mrustc's per-Terminator invariants form a
direct prior-art reference table:

| Terminator | Invariants enforced by mrustc validator |
|--|--|
| `Goto` | bb-in-bounds |
| `Return` | return-slot is valid; function has a returning path |
| `Diverge` (Unreachable analog) | (none currently; cleanup-TODO) |
| `If` (SwitchInt-bool analog) | cond is `bool`-typed; cond is valid; bb-in-bounds |
| `Switch` (SwitchInt-enum) | each target bb-in-bounds; cond is valid |
| `SwitchValue` (SwitchInt-int/str) | each target + default bb-in-bounds; cond is valid |
| `Call` | bb-in-bounds (ret + panic); callee Value-arm has `Function`-type; signature-match for Path-arm; args valid before move; ret_val valid after |
| `Assert` (not present) | -- |

Maps to **D-9-10** (invariants registry). RegseqModel's
stage-11 walker would mirror `MIR_Validate`'s per-
variant case-split.

---

## 8. Terminator interning and inline storage

mrustc stores the Terminator **inline by value** in
`BasicBlock::terminator`
[`src/mir/mir.hpp:726-730`]:

```cpp
struct BasicBlock
{
    ::std::vector<Statement>    statements;
    Terminator  terminator;
};
```

One Terminator per BasicBlock; tagged-union stored
inline (tag + payload union); no `Box<Terminator>`
indirection, no `TerminatorId` interning. Construction-
side: `MirBuilder::end_block(::MIR::Terminator term)`
[`src/mir/mir_builder.cpp:795-804`] move-assigns the
terminator into the BB's slot.

`Cloner::clone_term`
[`src/mir/mir.cpp:1023-1078`] returns Terminators by
value; `Function::blocks` is a
`std::vector<BasicBlock>` [`src/mir/mir.hpp:753`] which
moves the inline BB-and-terminator on resize.

For RegseqModel **D-9-7** (no Terminator interning):
mrustc's posture is direct peer evidence -- inline-by-
value, one per BasicBlock, no indirection. Matches the
Statement storage choice (stage-8 prior-art §5).
HYPOTHESIS: terminators are even less repetition-prone
than Statements (every BB has *exactly one* terminator;
`Goto(bb5)` repeats only as often as bb5 has multiple
predecessors), so interning saves nothing.

Maps to **D-9-7** (no Terminator interning).

---

## 9. C-emit consumer dispatch (`src/trans/codegen_c.cpp`)

mrustc's C codegen consumer dispatches per-Terminator
in two forms:

**Form A: linear emission**
[`src/trans/codegen_c.cpp:3236-3324`]. A
`TU_MATCH_HDRA` over the 9 variants. Per-variant
recipe:

| Variant | C output |
|--|--|
| `Incomplete` | `for(;;);` |
| `Return` | `return ;` (unit) or `return rv;` |
| `Diverge` | `_Unwind_Resume();` |
| `Goto` | `goto bb<n>;` (or fall through if next block in linear order) |
| `Panic` | `goto bb<n>; /* panic */` |
| `If` | `if(<lv>) goto bb<t>; else goto bb<f>;` |
| `Switch` | C `switch` with case-per-variant goto, with odd-arm 2-unique collapse to `if/else` [`src/trans/codegen_c.cpp:3267-3304`] |
| `SwitchValue` | C `switch` (Unsigned/Signed) or chained `if(strcmp/memcmp(...))` (String/ByteString) -- via `emit_term_switchvalue` |
| `Call` | `<ret_lv> = (<callee>)(<args>);` then optional `goto bb<ret_block>;` (fall-through optimised) |

Notable C-emit features:

- **Goto fall-through optimisation**
  [`src/trans/codegen_c.cpp:3252-3259`,
  `src/trans/codegen_c.cpp:3313-3320`]: if the goto
  target is the next block, emit no `goto`.
- **Switch odd-arm collapse**
  [`src/trans/codegen_c.cpp:3267-3304`]: 2-unique
  Switch becomes `if/else`.
- **Diverge -> `_Unwind_Resume()`**: real C call to
  `<unwind.h>` resume.

`emit_term_call` [`src/trans/codegen_c.cpp:4706+`] is
the heaviest sub-emitter:

- ZST-arg materialisation (temp vars for empty structs)
  [`src/trans/codegen_c.cpp:4711-4747`].
- High-alignment-arg copying
  [`src/trans/codegen_c.cpp:4731-4747`].
- Return-slot omission for `()` and `!` returns
  [`src/trans/codegen_c.cpp:4749-4764`].
- Per-`CallTarget` dispatch
  [`src/trans/codegen_c.cpp:4766-4851`]: `Value` ->
  `(<lv>)(args)`; `Path` -> `<mangle>(args)`;
  `Intrinsic` -> `emit_intrinsic_call` early-return.

`emit_term_switch` [`src/trans/codegen_c.cpp:4417+`]:
2-arm Switch -> `if/else`; N-arm -> C `switch` with
`case 0:` / `case 1:` / ... labels; discriminant-load
adapts to enum representation (tagged / niche /
raw).

`emit_term_switchvalue` [`src/trans/codegen_c.cpp:4624+`]:
per `SwitchValues` arm -- Unsigned/Signed C `switch`
with int cases; String / ByteString chained
`if(strcmp/memcmp(...))`.

**Form B: structured emission `emit_fcn_node`**
[`src/trans/codegen_c.cpp:3360-3438`]. Alternative
C-emit mode (`EMIT_STRUCTURED`) producing structured
control flow rather than goto-based; same per-
Terminator dispatch with structured-block
accumulation.

For RegseqModel: C-emit is **out of scope for stage 9**.
mrustc's evidence informs the post-stage codegen:

- Each retained Terminator needs a clear C-lowering
  recipe. mrustc's recipes are the reference table.
- The `goto BB<n>` baseline plus fall-through
  optimisation is sufficient for an unstructured C
  output.
- `emit_term_call`'s ABI-bridging machinery (ZST args,
  high-align args, `()`/`!` return omission) is direct
  prior art for RegseqModel-codegen.

Informs **D-9-1**'s "every retained variant needs a
C-emit path".

---

## 10. Pretty-printer (`MIR::Terminator::operator<<` / `MirDumper::dump_mir`)

mrustc has two Terminator renderers:

**`Terminator::operator<<`** [`src/mir/mir.cpp:419-489`].
Renders to any `std::ostream` via `TU_MATCHA` over the
9 variants. Per-variant output:

| Variant | Rendering |
|--|--|
| `Incomplete` | `Invalid` |
| `Return` | `Return` |
| `Diverge` | `Diverge` |
| `Goto` | `Goto(<bb>)` |
| `Panic` | `Panic(<bb>;)` (note the rogue `;`) |
| `If` | `If( <cond> : <bb_true>, <bb_false>)` |
| `Switch` | `Switch( <val> : 0 => bb<t0>, 1 => bb<t1>, ...)` |
| `SwitchValue` | `SwitchValue( <val> : <v0> => bb<t0>, ... else bb<def>)` |
| `Call` | `Call( <ret> = (callee)( <args>, ), bb<ret>, bb<panic>)` |

Where `<callee>` is per-`CallTarget` arm:

- `Value(lv)` -> `(<lv>)`.
- `Path(p)` -> `<p>` (HIR path operator<<).
- `Intrinsic{name, params}` -> `"<name>"::<params>`.

**`MirDumper::dump_mir`**
[`src/mir/dump.cpp:147-232`] -- higher-level dump used
by `MIR_Dump_Fcn`. Per-variant rendering uses a more
human-readable form:

| Variant | Rendering |
|--|--|
| `Incomplete` | `INVALID;` |
| `Return` | `return;` |
| `Diverge` | `diverge;` |
| `Goto` | `goto bb<n>;` |
| `Panic` | `panic bb<n>;` |
| `If` | `if <cond> { goto bb<t>; } else { goto bb<f>; }` |
| `Switch` | `switch <val> {0 => bb<t0>, 1 => bb<t1>, ...}` |
| `SwitchValue` | `switch <val> {<v0> => bb<t0>, ..., _ => bb<def>}` |
| `Call` | `<ret> = <callee>( <args>, ) goto bb<ret> else bb<panic>` |

`MirDumper` also pre-emits the BB header (`bb<n>: {`),
then iterates statements (rendered via
`Statement::operator<<` from stage-8 prior-art §9),
then emits the terminator on its own line, then closes
the block.

INFERENCE: mrustc has *two* pretty-printers because the
operator-form is for ad-hoc debug output (`DEBUG(...)`
macros embedded in the validator and codegen) and the
dump-form is for full IR dumps. Both omit source-info
context (consistent with §4); both are pure-structural.
Same discipline as stage-7 D-7-K and stage-8 D-8-K.

For RegseqModel **D-9-K** (pretty-printer deferral):
mrustc's discipline is per-Terminator `operator<<`,
plus a higher-level dump function. Stage-9 follows the
stage-7/-8 precedent of deferring
`pretty.rs::render_terminator` to stage 14.

Maps to **D-9-K** (pretty-printer deferral). Direct
peer evidence the Terminator-level pretty-printer is a
straightforward per-variant case-split with no source-
info context required.

---

## 11. Cleanup pass(es) -- `MIR_Cleanup` rewrites Call payloads, no variant stripping

mrustc's `MIR_Cleanup` pass
[`src/mir/cleanup.cpp:1326-1469`] does four kinds of
Terminator-relevant work:

**Form A: in-place LValue cleaning per Terminator**
[`src/mir/cleanup.cpp:1326-1354`]. A `TU_MATCH_HDRA`
over the 9 variants. Cleans `If.cond`, `Switch.val`,
`SwitchValue.val`, `Call.{ret_val, fcn(if Value), args}`
in-place (coercions, type re-resolution); no variant
changes.

**Form B: vtable-call lowering**
[`src/mir/cleanup.cpp:1357-1381`]. As detailed in §3,
rewrites `Call.fcn` from `Path(<dyn Trait as
Trait>::method)` to `Value(<vtable_loaded_lv>)`. The
Terminator stays a `Call`; only the `fcn` arm changes.

**Form C: Fn-trait lowering**
[`src/mir/cleanup.cpp:1383-1450`]. As detailed in §3,
rewrites `Fn`/`FnMut`/`FnOnce` calls; flips `Call.fcn`
from `Path` to `Value(LValue)` (with optional Deref
for Fn/FnMut), redistributes the args tuple. The
`Call` variant survives.

**Form D: Call-to-Goto downgrades**
[`src/mir/cleanup.cpp:1453-1466`,
`src/mir/cleanup.cpp:1431-1446`]. Specific Calls are
*replaced* by an Assign Statement (or Statement pair)
plus a `Goto(ret_block)` Terminator:

- Intrinsic `read_via_copy`: `Call` becomes `Goto`,
  with `Assign(ret_val, Deref(arg0))` prepended
  [`src/mir/cleanup.cpp:1453-1458`].
- Intrinsic `write_via_move`: `Call` becomes `Goto`,
  with `Assign(Deref(arg0), arg1)` and
  `Assign(ret_val, Tuple{})` prepended
  [`src/mir/cleanup.cpp:1459-1465`].
- `StructConstructor` callee: `Call` becomes `Goto`,
  with `Assign(ret_val, RValue::Struct{path, args})`
  prepended [`src/mir/cleanup.cpp:1431-1437`].
- `EnumConstructor` callee: `Call` becomes `Goto`,
  with `Assign(ret_val, RValue::EnumVariant{path, idx,
  args})` prepended [`src/mir/cleanup.cpp:1438-1446`].

INFERENCE: mrustc has no analog of rustc's
`ElaborateDrops` (Drop is already a Statement, §2),
`StateTransform`, or `CleanupPostBorrowck`. mrustc's
cleanup *does* rewrite Terminators, but only
**arm-flips within Call** (vtable, Fn-trait) or
**downgrades to Goto** (for the four cases above).
**No Terminator variants are removed from the surface**;
the surface is closed-form by construction (same
discipline as Statement, stage-8 prior-art §10).

For **D-9-9** (cleanup-pass-discharged rejection
table): mrustc handles four cases as cleanup
specialisations of a uniform input `Call`. The
analogous RegseqModel decision is whether the bridge
emits the post-cleanup shape directly or a placeholder
shape that cleanup specialises. mrustc evidence: a
cleanup pass that mutates Calls in-place is workable
but adds a new IR-walking surface and creates a "real
IR" vs "pre-cleanup IR" distinction.

Maps to **D-9-9** (cleanup-pass-discharged rejection
table). mrustc evidence: the closed-form Terminator
surface coexists with a cleanup pass that rewrites
*payloads*, not variants; the rejection-table
discipline lives at the bridge.

---

## 12. Stage-9 takeaways

D-tag-keyed summary:

- **D-9-1** (closed `TerminatorKind` surface): mrustc
  has 9 variants [`src/mir/mir.hpp:615-643`]. Mapping
  to RegseqModel's proposed 6: `Goto` <-> `Goto`,
  `SwitchInt` <-> `If` or `Switch` or `SwitchValue`
  (mrustc splits, RegseqModel unifies), `Return` <->
  `Return`, `Unreachable` <-> `Diverge` (semantic split:
  mrustc emits `_Unwind_Resume()`, RegseqModel should
  emit `abort()` / `__builtin_unreachable()`),
  `Call` <-> `Call`, `Assert` <-> (open-coded in mrustc).
  mrustc's `Incomplete` and `Panic` are dropped from
  the RegseqModel surface.
- **D-9-2** (Drop-as-Call collapse): mrustc keeps
  Drop as a Statement, codegen-inlines
  `<drop_glue>(&slot);`
  [`src/mir/mir.hpp:710-715`,
  `src/trans/codegen_c.cpp:3725-3756`,
  `src/trans/codegen_c.cpp:7783-7869`]. RegseqModel's
  collapse-to-Call goes the other way; mrustc evidence
  shows the alternative is workable but encodes drop-
  glue policy in the C backend.
- **D-9-3** (callee shape with vtable-indirect
  dispatch): mrustc has `CallTarget = Value(LValue) |
  Path(HIR::Path) | Intrinsic{name, params}`
  [`src/mir/mir.hpp:595-602`]; vtable dispatch reuses
  `Value` after cleanup-pass synthesised vtable load
  [`src/mir/cleanup.cpp:1357-1381`,
  `src/mir/cleanup.cpp:548-684`]. No `Virtual` arm.
  RegseqModel's option (a) (`InstanceKind::Virtual`)
  hoists this distinction into a stable IR shape;
  option (b) (`Indirect(Operand)`) widens Operand
  rather than InstanceKind.
- **D-9-4** (per-arg span preservation): mrustc has
  no spans on Terminator and no spans on
  `Call.args: vec<Param>`
  [`src/mir/mir.hpp:498-514`,
  `src/mir/mir.hpp:641`]; construction-side `Span`s
  are dropped at `end_block`
  [`src/mir/mir_builder.cpp:795-804`]. RegseqModel's
  `Spanned<Operand>` per-arg is strictly more
  diagnostic information than mrustc retains.
- **D-9-5** (`AssertKind` subset): mrustc has no
  `Assert` terminator
  [`src/mir/mir.hpp:615-643`]; bounds / overflow checks
  open-coded as `If` + `Diverge`
  [`src/mir/from_hir.cpp:1888-1896`]. Direct peer
  evidence that Assert is droppable, at the cost of BB
  count inflation and lost panic-message structure.
- **D-9-6** (`SwitchTargets` storage shape): mrustc
  uses parallel arrays in `SwitchValue`
  (`targets: vec<BasicBlockId>` paired with
  `values: SwitchValues` 4-arm union)
  [`src/mir/mir.hpp:603-635`]; rustc uses paired vec.
  mrustc's `SwitchValues` 4-arm union externalises
  the literal-type tag once per switch.
- **D-9-7** (no Terminator interning): mrustc stores
  `Terminator` inline by value at
  `BasicBlock.terminator`
  [`src/mir/mir.hpp:726-730`]; no `Box<Terminator>`,
  no interning. Direct match.
- **D-9-8** (wrapper struct shape): mrustc has no
  Terminator-level wrapper struct -- the tagged union
  itself is what's stored. No `SourceInfo`, no span.
- **D-9-9** (cleanup-pass-discharged rejection
  table): mrustc's `MIR_Cleanup` mutates Call
  payloads (vtable, Fn-trait), and downgrades Call to
  Goto+Statement for two intrinsics and constructor-
  callees [`src/mir/cleanup.cpp:1326-1469`]. The
  Terminator surface is closed-form by construction;
  cleanup rewrites only payloads or downgrades for
  specific cases.
- **D-9-10** (invariants registry; walk deferred):
  per-Terminator invariants spread across
  `MIR_Validate` (CFA + flat type checks
  [`src/mir/check.cpp:670-737`,
  `src/mir/check.cpp:1229-1338`]),
  `MIR_Validate_ValState`
  [`src/mir/check.cpp:563-633`], and `MIR_Validate_Full`
  [`src/mir/check_full.cpp:937-1014`]. RegseqModel's
  registry-then-stage-11-walker discipline mirrors
  this layering.
- **D-9-K** (pretty-printer deferral): mrustc has
  `Terminator::operator<<`
  [`src/mir/mir.cpp:419-489`] and `MirDumper`
  [`src/mir/dump.cpp:147-232`], both per-variant
  case-splits with no source-info context. Defer
  `pretty.rs::render_terminator` to stage 14.

**Notable peer-design lessons:**

1. **Drop is the central tension.** mrustc puts it in
   Statements and inlines at C-emit; rustc puts it in
   Terminators with a panic edge; RegseqModel D-9-2
   collapses to a synthetic Call. All three are
   workable; the choice trades IR simplicity (mrustc),
   rustc-shape preservation (rustc), and uniform
   C-emit (RegseqModel).
2. **Vtable dispatch can be a payload-level
   transform, not a stable IR variant.** mrustc
   resolves vtables in cleanup; RegseqModel's option
   (a) hoists "Virtual" into `InstanceKind`. mrustc's
   evidence: the stable-variant approach is **not**
   required to make the IR work.
3. **Panic-edge representation diverges across peers.**
   mrustc has `Panic` (debug-marker, phasing out) plus
   `Call.panic_block`; rustc uses `UnwindAction` on
   Call/Drop/Assert; RegseqModel D-9-2 (Call carries
   the panic edge for Drop) follows the rustc pattern.
   mrustc's `Call.panic_block` is direct prior art for
   panic-edge-on-Call.
4. **Validator per-Terminator case-split *is* the
   invariants registry.** mrustc's `MIR_Validate` is
   the literal reference structure D-9-10 calls for;
   stage-11's walker mirrors it variant-by-variant.
5. **Closed-form Terminator surface is non-negotiable.**
   mrustc demonstrates a peer can have a 9-variant
   surface that the bridge produces and codegen
   consumes, with cleanup mutating payloads and
   downgrading Call -> Goto in narrow cases -- without
   ever adding new variants at cleanup. RegseqModel's
   D-9-9 rejection table follows the same discipline.

---

## 13. What's NOT in this pass

- **Optimisation passes**
  (`src/mir/optimise.cpp`) -- inlining, constant-
  folding, CFG simplification touch Terminators but
  are out of scope for IR shape decisions.
- **Borrow-check pass behaviour on Terminator** --
  `src/mir/borrow_check.cpp` returned no `Terminator`
  hits; D-9-10 does not include borrow-check
  invariants.
- **Generator/coroutine state-machine lowering** --
  evidence at [`src/mir/from_hir.cpp:127`,
  `src/mir/from_hir.cpp:233`] shows lowering to
  Switch-on-state; mechanism details out of scope.
- **`emit_intrinsic_call` per-intrinsic body** at
  [`src/trans/codegen_c.cpp:5859+`] -- per-name
  intrinsic codegen is a stage-12+ concern.
- **Drop-glue function body synthesis** at
  [`src/trans/auto_impls.cpp:933-961`] -- body
  generation for `<T>::#drop_glue` is a stage-13
  drop-elaboration concern.
- **`MIR::Cloner::clone_term` arm-by-arm**
  [`src/mir/mir.cpp:1023-1078`] -- cited for variant
  tally but not detailed.
- **`emit_term_switch` / `emit_term_switchvalue`
  internals** [`src/trans/codegen_c.cpp:4417-4705`] --
  discriminant-load and case-label emission;
  stage-12 codegen, not stage-9 IR shape.
