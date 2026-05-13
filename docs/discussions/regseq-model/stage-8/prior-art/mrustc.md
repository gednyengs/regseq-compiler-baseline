# Stage 8 -- Prior Art: mrustc primary source

Primary-source mrustc evidence for the **MIR Statement
analog vocabulary**, scoped to what stage 8's candidate
decision namespace
(`D-8-1` Statement variant set + per-variant payload shapes,
with sub-decisions `D-8-1.1` variant cardinality, `D-8-1.2`
Assign payload, `D-8-1.3` SetDiscriminant payload,
`D-8-1.4` Intrinsic payload; `D-8-2` Statement interning vs
inline storage; `D-8-3` Statement wrapper / source-info;
`D-8-4` Statement-site invariants + rejection rules; `D-8-K`
pretty-printer deferral) needs.

mrustc is the closest existing peer: a Rust-to-C compiler
with its own HIR/MIR, written in C++. Its `MIR::Statement`
and surrounding types directly inform the same design space
as RegseqModel's `Statement`. The stage-6 and stage-7
prior-art mrustc files
(`./../../stage-6/prior-art/mrustc.md`,
`./../../stage-7/prior-art/mrustc.md`) already covered the
`LValue`, `Param`, `Constant`, `RValue`, `eBinOp` and
`eUniOp` surfaces; this file does not re-derive that
material. It focuses on the `Statement` tagged-union, its
construction site, validation, codegen-C consumer dispatch,
and pretty-printing -- topics those earlier files did not
exercise.

Repository: https://github.com/thepowersgang/mrustc

Master HEAD as of this pass: commit
`7392eca5bd4958cb184fb47cefbd4c7e4f43547b` (committer date
2026-04-13). All file:line citations refer to that tree.

Primary files inspected:

- `src/mir/mir.hpp` -- IR type definitions
  (`Statement`, `BasicBlock`, `Terminator`, `CallTarget`,
  `eDropKind`, `AsmParam`, `Function`).
- `src/mir/from_hir.hpp` -- `MirBuilder` declarations
  (`push_stmt_*` family, span handling at construction).
- `src/mir/from_hir.cpp` -- HIR-to-MIR construction
  (which Statement variants are emitted, where).
- `src/mir/check.cpp` -- `MIR_Validate` and
  `MIR_Validate_ValState` entry points; per-variant flat
  type checks; per-variant value-state checks.
- `src/mir/check_full.cpp` -- `MIR_Validate_Full` /
  `MIR_Validate_FullValState` validators (deeper move /
  drop-flag invariants).
- `src/mir/cleanup.cpp` -- `MIR_Cleanup` pass
  (does mrustc strip Statement variants the way rustc
  does in `RemoveStorageMarkers` / `CleanupPostBorrowck`?).
- `src/mir/dump.cpp` -- pretty-printer entry
  (`MIR_Dump`, `MIR_Dump_Fcn`).
- `src/mir/mir.cpp` -- `operator<<` for `Statement`,
  `MIR::Cloner::clone_stmt`.
- `src/trans/codegen_c.cpp` -- C-emit consumer
  (`emit_statement`, `emit_intrinsic_call`).

Every factual claim cites `[file:line]` in the mrustc tree.
Hypotheses are flagged **HYPOTHESIS:**. Cross-source
synthesis is flagged **INFERENCE:**.

Stage-context anchor: this file does not claim parity with
rustc, rustc_public, or charon -- those are sibling
prior-art passes. mrustc consumes a much older rustc and
predates significant post-MIR work in modern rustc; drift is
expected, and where mrustc made a different design choice,
that divergence is itself the evidence relevant to a
Rust-to-C peer compiler.

---

## 1. `Statement` tagged-union shape and full variant list

mrustc's `Statement` is an 8-variant tagged union
[`src/mir/mir.hpp:733-776`]:

```cpp
TAGGED_UNION(Statement, Asm,
    // Value assigment
    (Assign, struct {
        LValue  dst;
        RValue  src;
        }),
    // Inline assembly (`llvm_asm!`)
    (Asm, struct {
        ::std::string   tpl;
        ::std::vector< ::std::pair<::std::string,LValue> >  outputs;
        ::std::vector< ::std::pair<::std::string,LValue> >  inputs;
        ::std::vector< ::std::string>   clobbers;
        ::std::vector< ::std::string>   flags;
        }),
    // Inline assembly (stabilised)
    (Asm2, struct {
        AsmCommon::Options  options;
        std::vector<AsmCommon::Line>   lines;
        ::std::vector<AsmParam> params;
        }),
    // Update the state of a drop flag
    (SetDropFlag, struct {
        unsigned int idx;
        bool new_val;
        unsigned int other;
        }),
    // Save a drop flag to a bitset
    (SaveDropFlag, struct {
        LValue  slot;
        unsigned int    bit_index;
        unsigned int    idx;
        }),
    (LoadDropFlag, struct {
        unsigned int    idx;
        LValue  slot;
        unsigned int    bit_index;
        }),
    // Drop a value
    (Drop, struct {
        eDropKind   kind;
        LValue  slot;
        unsigned int flag_idx;
        }),
    (ScopeEnd, struct {
        ::std::vector<unsigned> slots;
        })
    );
```

Variant tally (8 arms): `Assign`, `Asm`, `Asm2`,
`SetDropFlag`, `SaveDropFlag`, `LoadDropFlag`, `Drop`,
`ScopeEnd`.

Notable absences vs rustc's 13-variant
`StatementKind<'tcx>` (all confirmed by header sweep
[`src/mir/mir.hpp:733-776`]):

- **No `SetDiscriminant`.** Discriminant writes fold
  into `RValue::EnumVariant{ path, index, vals }` (the
  rvalue-side aggregate constructor; stage-7 prior-art
  §10), surfaced via an ordinary `Assign{ dst,
  src=EnumVariant{..} }`. INFERENCE: mrustc never
  needs the rustc-style "update the tag of an
  already-initialized place" operation; whole-enum
  construction is the only path. Detail in §3.
- **No `StorageLive` / `StorageDead`.** No storage
  markers. mrustc's `Function` carries a single
  `::std::vector< ::HIR::TypeRef> locals`
  [`src/mir/mir.hpp:758-766`] (one slot per local
  function-wide); finer stack-allocation granularity is
  the C compiler's job.
- **No `FakeRead`, `AscribeUserType`, `PlaceMention`,
  `Retag`.** No NLL borrowck or Stacked-Borrows /
  Tree-Borrows hooks; mrustc's own
  `MIR_Validate_ValState` does move/init tracking
  [`src/mir/check.cpp:85`,
  `src/mir/check_full.cpp:707`] without these markers.
  Confirms the stage-7 finding
  (`./../../stage-7/prior-art/mrustc.md` §10) that
  rustc's `UserTypeAnnotationIndex` slot is dead by the
  time MIR reaches a peer Rust-to-C compiler.
- **No `Coverage`, `Nop`, `ConstEvalCounter`,
  `BackwardIncompatibleDropHint`.** Coverage is
  out-of-scope for Rust-to-C; the rest are rustc-
  internal optimisation hints.
- **No `Intrinsic` Statement variant.** Intrinsics
  always flow through `Terminator::Call(CallTarget::
  Intrinsic{ name, params })`
  [`src/mir/mir.hpp:686-719`,
  `src/trans/codegen_c.cpp:4840-4850`]. Detail in §4.

Notable presences vs rustc:

- **`Asm` and `Asm2` are two distinct Statement
  variants** [`src/mir/mir.hpp:738-754`] -- `Asm` for
  legacy `llvm_asm!()` (template + in/out lvalues +
  clobbers + flags), `Asm2` for stable `asm!()`
  (AsmCommon::Options + Line list + AsmParam list).
  rustc consolidates into a single `InlineAsm`
  terminator. INFERENCE: mrustc retains both for
  backwards compatibility -- `Asm` predates the
  stabilised `asm!`.
- **Drop-flag plumbing is first-class** as three
  Statement variants (`SetDropFlag`, `SaveDropFlag`,
  `LoadDropFlag`) [`src/mir/mir.hpp:751-768`]. rustc's
  drop elaboration compiles to Assign-to-bool + Switch;
  mrustc materialises flag ops directly, with a
  function-wide sidecar bitset (`Function::drop_flags:
  vec<bool>` [`src/mir/mir.hpp:760`]).
- **`Drop{ kind: eDropKind, slot: LValue, flag_idx:
  unsigned }`** [`src/mir/mir.hpp:769-773`]. `eDropKind`
  is `{SHALLOW, DEEP}` [`src/mir/mir.hpp:676-679`].
  `flag_idx == ~0u` is "unconditional"; rustc separates
  the conditional-drop guard across Statement +
  Terminator boundaries.
- **`ScopeEnd{ slots: vec<unsigned> }`**
  [`src/mir/mir.hpp:774-776`] -- explicit "these locals
  are dead now" marker; rustc computes equivalent
  scope information from `StorageLive`/`StorageDead`
  pairs and `SourceScope` debuginfo. The dump.cpp
  consumer renders it as a comment
  [`src/trans/codegen_c.cpp:3698-3700`] (§7).

The variant tally is **deliberately small** (8) compared
to rustc's 13. mrustc consolidates: discriminant-set into
aggregate-rvalue assignment, all storage tracking into
function-level locals, all borrowck markers absent, all
coverage / retag / nop absent. What survives is what a
runtime Rust-to-C transpiler actually needs to produce
working C code: assignment, two flavours of inline asm,
drop-flag manipulation, the drop call itself, and a scope
marker.

Maps to **D-8-1.1** (variant cardinality), **D-8-1.2**
(Assign payload), **D-8-1.3** (SetDiscriminant decision --
mrustc chose "fold into aggregate-rvalue assign"),
**D-8-1.4** (Intrinsic decision -- mrustc chose
"never a Statement; always a Call terminator"). Cross-
referenced with stage-7 prior-art's evidence on
`RValue::EnumVariant`.

---

## 2. `Assign` payload shape -- flat tuple, RValue inline,
no boxing

mrustc's `Assign` payload is a flat anonymous struct
[`src/mir/mir.hpp:734-737`]:

```cpp
(Assign, struct {
    LValue  dst;
    RValue  src;
    }),
```

Two members, both stored **by value, inline**, no
`Box<...>` / `unique_ptr<...>` indirection. `RValue` is
itself a 15-variant tagged union
[`src/mir/mir.hpp:628-677`] (stage-7 prior-art §1).

INFERENCE: the C++ tagged-union machinery imposes no
strict size budget analogous to rustc's
`static_assert_size!`; mrustc accepts the width.

Type-equality between `dst` and `src` is **not encoded
structurally** -- no shared type slot, no type witness.
It is enforced post-hoc by the validator
[`src/mir/check.cpp:984-995`]:

```cpp
auto check_types = [&](const auto& dst_ty, const auto& src_ty) {
    if( src_ty == ::HIR::TypeRef::new_diverge() ) { }
    else if( src_ty == dst_ty ) { }
    else {
        MIR_BUG(state, "Type mismatch:\n"
            << " dst : " << dst_ty << "\n"
            << " src : " << src_ty);
    }
};
```

The `new_diverge()` carve-out lets the never-type flow
through. For RegseqModel **D-8-1.2**: mrustc's choice
is "flat struct, no Box, no per-Statement type witness;
type equality is a validator-pass invariant." Direct
peer evidence for a flat `Assign { dst: Place, src:
Rvalue }` shape with no type witness.

Maps to **D-8-1.2** (Assign payload).

---

## 3. `SetDiscriminant` analog -- folded into
`RValue::EnumVariant`

mrustc has **no** `SetDiscriminant` Statement variant
[`src/mir/mir.hpp:733-776`]. Discriminant writes happen
via the rvalue-side `EnumVariant` aggregate constructor
[`src/mir/mir.hpp:673-678`, stage-7 prior-art §10]:

```cpp
// Create a new instance of an enum
(EnumVariant, struct {
    ::HIR::GenericPath  path;
    unsigned int index;
    ::std::vector<Param>   vals;
    }),
```

Lowering evidence: `from_hir.cpp` constructs enum-variant
assignments via `push_stmt_assign(sp, dst,
RValue::make_EnumVariant({path, index, vals}))`
[`src/mir/from_hir.cpp:642`,
`src/mir/from_hir.cpp:646`,
`src/mir/from_hir.cpp:708`,
`src/mir/from_hir.cpp:1403`]. There is no
`push_stmt_set_discriminant` or equivalent; the only
`push_stmt_*` API at the Statement level is the
generic-and-specific assign / drop / asm / drop-flag
family (§5 below).

This is a **structural divergence** from rustc:

| | rustc | mrustc |
|--|--|--|
| Whole-enum construction | `Assign(dst, Aggregate(Adt(.., variant_idx, ..), [fields]))` | `Assign(dst, EnumVariant{path, index, vals})` |
| Tag-only update on already-init enum | `SetDiscriminant{place, variant_index}` Statement | (no equivalent; mrustc lowers this to a fresh `Assign` of an `EnumVariant` rvalue with the new tag and the existing field reads recopied) |

INFERENCE: mrustc never needs to *update* the tag on
an already-initialized enum because lowering always
re-constructs the whole enum value (fields still moved
via `vals`); strictly less expressive than rustc's
surface, which has partial-init / `mem::replace`-the-tag
use cases. HYPOTHESIS: rustc retains `SetDiscriminant`
for some `match` exhaustiveness or
`enum::Niche::ZeroOrOne`-style lowerings; not verified
in this pass.

For RegseqModel **D-8-1.3**: mrustc demonstrates a peer
can elide `SetDiscriminant` by always going through
aggregate-rvalue construction. INFERENCE: stage 8 will
likely retain `SetDiscriminant` per the
information-preservation rule -- the bridge consumes
rustc's surface directly, and re-synthesising an
aggregate rvalue with field reads from the existing
place would be added lowering work mrustc's posture
absorbs but RegseqModel's translator-not-optimizer
posture would not.

Maps to **D-8-1.3** (SetDiscriminant payload / decision).

---

## 4. Intrinsic analog -- always a `Terminator::Call`,
never a Statement

mrustc has **no** `Intrinsic` Statement variant
[`src/mir/mir.hpp:733-776`]. Intrinsics flow exclusively
through the `Call` terminator's `CallTarget::Intrinsic`
arm [`src/mir/mir.hpp:686-719`,
`src/mir/mir.hpp:620-627`]:

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

```cpp
(Call, struct {
    BasicBlockId    ret_block;
    BasicBlockId    panic_block;
    LValue  ret_val;
    CallTarget  fcn;
    ::std::vector<Param>   args;
    })
```

The intrinsic name is an `RcString` (mrustc's reference-
counted string type) and the type parameters are an
`::HIR::PathParams`. Args go through the normal `Call`
args list (`vec<Param>`).

Codegen-side dispatch at
[`src/trans/codegen_c.cpp:4840-4850`]:

```cpp
TU_ARMA(Intrinsic, e2) {
    const auto& name = e2.name;
    const auto& params = e2.params;
    emit_intrinsic_call(name, params, e);
    if( has_zst )
    {
        indent.n --;
        m_of << indent << "}\n";
    }
    return ;
    }
```

`emit_intrinsic_call` [`src/trans/codegen_c.cpp:5859`]
is a per-intrinsic-name dispatch (a large switch over
intrinsic names; the section banner at line 7519 reads
"Platform Intrinsics (and SIMD)").

INFERENCE: mrustc's design choice is "intrinsics are
calls; calls are terminators; therefore intrinsics
terminate basic blocks" -- uniform with the
borrowck-and-init-tracking model (every call is a CFG
branch because of the panic-on-fail unwind path).

This contrasts with rustc's modern `StatementKind::
Intrinsic(Box<NonDivergingIntrinsic>)` for non-diverging
intrinsics (`assume`, `copy_nonoverlapping`) that don't
need a CFG break.

For RegseqModel: this is **direct peer evidence for
D-8-1.4**. Two real Rust-to-C designs:

1. **rustc-shape**: `Statement::Intrinsic(NonDiverging)` +
   `Terminator::Call(...)` for diverging intrinsics. Two
   surfaces.
2. **mrustc-shape**: `Terminator::Call(CallTarget::
   Intrinsic{name, params})` for everything; no Statement
   intrinsic at all. One surface.

The trade-off is: mrustc-shape is simpler (one variant
fewer at the Statement level, uniform Call dispatch in
the C backend), but inflates the basic-block count
because every `assume(x)` becomes its own block. rustc-
shape is closer to what rustc emits (so the bridge
translates 1:1) but requires an additional Statement
variant.

INFERENCE: stage 8 will likely follow rustc's shape
(retain `Statement::Intrinsic` for the non-diverging
subset) because the bridge consumes rustc's shape
directly, and RegseqModel is a translator. mrustc's
unifying shortcut is a peer alternative, not a
recommended path.

Maps to **D-8-1.4** (Intrinsic payload / decision).

---

## 5. Statement interning vs inline storage

mrustc stores Statements **inline by value** in
`BasicBlock::statements: std::vector<Statement>`
[`src/mir/mir.hpp:778-782`]:

```cpp
struct BasicBlock
{
    ::std::vector<Statement>    statements;
    Terminator  terminator;
};
```

No interner, no `StatementId` indirection, no
`Box<Statement>` / `unique_ptr<Statement>` per slot.
The vector is the only indirection; the tagged-union is
stored as a contiguous run of (tag + payload) cells.

Construction-side: `MirBuilder::push_stmt(...)` and
the specific `push_stmt_assign` / `push_stmt_drop` /
`push_stmt_asm` / `push_stmt_set_dropflag_*` family
[`src/mir/from_hir.hpp:~290`] consume arguments by
value and append-construct at the end of the current
block's `statements` vector. No deduplication.

INFERENCE: Statements are large (Assign carries a
15-variant RValue) and rarely identical across sites;
hash-cons would not pay for itself. Direct peer
evidence for **D-8-2**: inline `Vec<Statement>` storage,
no interning. Matches the stage-8 spec snapshot's
framing (`./../status.md` line 136).

Maps to **D-8-2** (no Statement interning).

---

## 6. `Statement` wrapper / source-info treatment

mrustc's `Statement` has **no wrapper struct**
[`src/mir/mir.hpp:733-776`]. The `Statement` tagged
union itself is what's stored in `BasicBlock::
statements: std::vector<Statement>`
[`src/mir/mir.hpp:778-782`]. There is no
`SourceInfo` / `Span` field.

`BasicBlock` also has no span field
[`src/mir/mir.hpp:778-782`]. `Function` has no
function-level source info field
[`src/mir/mir.hpp:758-766`].

Construction-side evidence: every `push_stmt_*` method
takes `const Span& sp` as its first argument
[`src/mir/from_hir.hpp:~290`]:

```cpp
void push_stmt_assign(const Span& sp, ::MIR::LValue dst,
                      ::MIR::RValue val, bool update_dest_state=true);
void push_stmt_drop(const Span& sp, ::MIR::LValue val,
                    unsigned int drop_flag=~0u);
void push_stmt_drop_shallow(const Span& sp, ::MIR::LValue val,
                            unsigned int drop_flag=~0u);
void push_stmt_asm(const Span& sp, ::MIR::Statement::Data_Asm data);
void push_stmt_set_dropflag_val(const Span& sp, unsigned int index,
                                bool value);
void push_stmt_set_dropflag_other(const Span& sp, unsigned int index,
                                  unsigned int other);
void push_stmt_set_dropflag_default(const Span& sp, unsigned int index);
void push_stmt(const Span& sp, ::MIR::Statement stmt);
```

But the `Span` is **consumed during construction and not
persisted**. There is no parallel `std::vector<Span>`
running alongside `BasicBlock::statements`; there is no
sidecar storage on `Function`; the Statement itself has
no slot for it. The Span is used for the diagnostic
context of construction-time failures (e.g., type-
inference crashes during HIR-to-MIR lowering) and then
dropped.

INFERENCE: mrustc deliberately throws away
Statement-level source spans. Diagnostics that need to
point at a specific Statement after the function is
fully lowered cannot do so; only function-level source
context (from the HIR side) is available downstream.
This is a strict simplification choice with direct cost:
post-lowering errors lose granular line-pointing.

This contrasts sharply with rustc, where every
`Statement` carries a `source_info: SourceInfo` (with
`span: Span` and `scope: SourceScope`). rustc treats
per-Statement spans as load-bearing for diagnostic and
debuginfo emission.

For RegseqModel **D-8-3**: two peer designs --
rustc-shape (per-Statement `SourceInfo`) vs mrustc-shape
(no IR-level spans; function-level context only).
Stage-8 spec snapshot (`./../status.md` line 48)
proposes a third design: Statement carries only its
structural payload, with a sidecar
`source_info_table` for diagnostic pointer-back.
mrustc's stance is direct peer evidence the structural
payload alone is workable; RegseqModel's sidecar design
trades mrustc's diagnostic loss for a less-coupled
storage scheme than rustc's inline SourceInfo.

Maps to **D-8-3** (Statement wrapper / source-info).

---

## 7. Validation pass behaviour on Statement

mrustc has two validators that exercise Statement-
shape invariants: `MIR_Validate` (basic shape /
type-equality) and `MIR_Validate_Full` /
`MIR_Validate_FullValState` (deep move / drop-flag
state tracking).

**`MIR_Validate`** [`src/mir/check.cpp:844`,
entry signature shown in the file's documentation
banner]. Per-Statement dispatch is a `switch`
[`src/mir/check.cpp:938-1341`] with arms for every tag:

```cpp
switch( stmt.tag() )
{
case ::MIR::Statement::TAGDEAD:
case ::MIR::Statement::TAG_SetDropFlag:
case ::MIR::Statement::TAG_SaveDropFlag:
case ::MIR::Statement::TAG_LoadDropFlag:
case ::MIR::Statement::TAG_Assign:
case ::MIR::Statement::TAG_Asm:
case ::MIR::Statement::TAG_Asm2:
case ::MIR::Statement::TAG_Drop:
case ::MIR::Statement::TAG_ScopeEnd:
}
```

Per-variant invariants observed:

- **`SetDropFlag`** [`src/mir/check.cpp:949-952`] --
  drop-flag index in bounds (`idx < fcn.drop_flags.size()`).
- **`SaveDropFlag` / `LoadDropFlag`**
  [`src/mir/check.cpp:953-969`] -- the slot lvalue's type
  must be `[u8; N]` (a byte array used as the bit-storage
  backing); the bit index must fit within `bytes * 8`
  ([`src/mir/check.cpp:968`]):

  ```cpp
  MIR_ASSERT(state, bit < bytes * 8,
      "Save/Load drop flag, bit index out of range");
  ```

- **`Assign`** [`src/mir/check.cpp:971-1217`] -- the
  centerpiece. Type-equality between `dst` and `src` is
  enforced by the `check_types` helper
  [`src/mir/check.cpp:984-995`]; per-RValue-variant
  shape constraints are also checked here (every RValue
  arm has its own block: Use, Constant, SizedArray,
  Borrow, Cast, BinOp, UniOp, DstMeta, DstPtr, MakeDst,
  Tuple, Array, UnionVariant, EnumVariant, Struct).
  Constant-type validation per Constant-variant
  [`src/mir/check.cpp:1002-1048`]; cast-legality
  [`src/mir/check.cpp:1090-1181`].
- **`Asm` / `Asm2`** [`src/mir/check.cpp:1219-1242`] --
  inputs are moved (use validation), outputs are marked
  valid post-write. No template-string parsing or
  param-count cross-check at this level.
- **`Drop`** [`src/mir/check.cpp:1243-1254`] --
  prohibits drop through a shared borrow (`MIR_ASSERT`
  the LValue is not a `&T` deref). Drop-kind /
  flag-index shape is left to the deeper validator.
- **`ScopeEnd`** [`src/mir/check.cpp:1255`] -- placeholder,
  no validation. Slot indices not bounds-checked here.

Rejection policy: every per-variant invariant violation
fires `MIR_BUG` (fatal compiler-internal error) or
`MIR_ASSERT` (assertion that becomes `MIR_BUG` on fail).
There is no soft-fail / diagnostic-only path; Statement-
shape violations are unrecoverable.

**`MIR_Validate_ValState`** [`src/mir/check.cpp:85`] is
the move-tracking pass. It walks each Statement and
updates a per-local validity vector:

- `Assign`: source operand validity per RValue arm,
  destination marked valid post-write
  [`src/mir/check.cpp:535` -- non-Copy values marked
  invalid after move].
- `Drop`: source slot validity, then marked invalid
  post-drop.
- `SetDropFlag` / `SaveDropFlag` / `LoadDropFlag`:
  flag-index bounds, no LValue mutation tracked.
- `Asm` / `Asm2`: outputs marked valid, inputs require
  valid lvalues.
- `ScopeEnd`: documented as "should mark slots invalid"
  but currently a no-op (per the deeper validator).

**`MIR_Validate_Full`** [`src/mir/check_full.cpp:857`]
and `MIR_Validate_FullValState`
[`src/mir/check_full.cpp:707`] add Box-aware
shallow-drop invariants
[`src/mir/check_full.cpp:799-810`]:

```cpp
MIR_ASSERT(mir_res, vs.index != ~0u,
  "Shallow drop on fully-valid value - " << se.slot);
MIR_ASSERT(mir_res, vs.is_composite(),
  "Shallow drop on non-composite state - " << se.slot ...);
MIR_ASSERT(mir_res, sub_states.size() == 2,
  "Shallow drop of slot with incorrect state shape ...");
MIR_ASSERT(mir_res, sub_states[0].is_valid(),
  "Shallow drop on deallocated Box - " << se.slot ...);
MIR_ASSERT(mir_res, !sub_states[1].is_valid(),
  "Shallow drop on populated Box - " << se.slot ...);
```

`LoadDropFlag` and `SaveDropFlag` in the deep validator
[`src/mir/check_full.cpp:818-822`] are unimplemented
(`MIR_TODO`).

For RegseqModel **D-8-4**: two layers of validation --
shape invariants (type-equality, bounds checks,
shared-borrow-drop prohibition) and move-state
invariants (deeper, more subtle: "shallow drop on
populated Box" is the exemplar). mrustc fires
`MIR_BUG` fatal-on-violation, consistent with stage 8's
framing of Statement-site invariants as bridge-rejection
rules. RegseqModel's analog invariants -- place
projection-prefix rules from D-5-3, operand rules from
D-6, rvalue rules from D-7, Assign type-equality from
D-8-1.2, SetDiscriminant target-type/variant-index
from D-8-1.3 -- mirror mrustc's at construct-time.

Maps to **D-8-4** (Statement-site invariants +
rejection rules).

---

## 8. Codegen-C consumer dispatch

mrustc's C codegen consumer dispatches per-Statement in
`CodeGenerator_C::emit_statement`
[`src/trans/codegen_c.cpp:3691-3791`] -- a `switch
(stmt.tag())` with one arm per variant. Per-variant
emission shape:

- **`Assign`** [`src/trans/codegen_c.cpp:3781-...`] --
  most complex arm. Inner `TU_MATCH_HDRA` over the 15
  RValue variants emits a C assignment
  (`<lvalue> = <rvalue>;`); ZST destinations are
  short-circuited with a `/* ZST assign */` comment
  [`src/trans/codegen_c.cpp:3788-3792`].
- **`SetDropFlag`** [`src/trans/codegen_c.cpp:3701-3709`]
  -- emits `dfN = <bool-expr>;` (either literal or
  `[!]dfM` indirection). Real C output.
- **`SaveDropFlag` / `LoadDropFlag`**
  [`src/trans/codegen_c.cpp:3710-3724`] -- bit-
  manipulation against the slot's `.DATA[byte_idx]`
  byte-array storage (`|=` set, `&= ~` clear, `&` mask
  for read). Real C output.
- **`Drop`** [`src/trans/codegen_c.cpp:3725-3756`] --
  conditional `if( dfN )` guard if `flag_idx != ~0u`,
  then `emit_box_drop` (SHALLOW) or
  `emit_destructor_call` (DEEP). Fires `MIR_BUG` if
  SHALLOW is requested on a non-Box type
  [`src/trans/codegen_c.cpp:3743`]. Real C output.
- **`Asm`** [`src/trans/codegen_c.cpp:3757-3768`] --
  per-target-compiler dispatch (`emit_asm_gcc` /
  `emit_asm_msvc`). Real C output (compiler-specific
  inline-asm syntax).
- **`Asm2`** [`src/trans/codegen_c.cpp:3769-3780`] --
  same per-compiler split for the stabilised asm form.
  Real C output.
- **`ScopeEnd`** [`src/trans/codegen_c.cpp:3698-3700`]
  -- emits a **comment only** (`// ScopeEnd: ...`); no
  C statement. **No-op for runtime semantics**.

INFERENCE: every variant except `ScopeEnd` produces
real C output. `ScopeEnd` is the only debug-marker-only
variant. Several arms (`Drop`, `Asm`, `Asm2`) emit a
trailing `// ^ <stmt>` comment for back-tracing C output
to the MIR Statement; incidental, not load-bearing.

For RegseqModel: informs **D-8-1.1** indirectly
(every retained variant needs a C-emit path; mrustc's
8 variants all do, with ScopeEnd as the no-op
exception). Also informs **D-8-K** (pretty-printer):
mrustc inlines C-emit per variant and uses
`Statement::operator<<` for debug rendering (§9
below); the C-emit dispatch is a separate backend
concern out of scope for stage 8.

Maps to **D-8-1.1** and informs **D-8-K**.

---

## 9. Pretty-printer / dump renderer

mrustc has two Statement renderers:

**`Statement::operator<<`** [`src/mir/mir.cpp:689-722`]
-- renders to any `std::ostream` via `TU_MATCH_HDRA`
over the 8 variants. Per-variant output (sketched):
`Assign` emits `<dst> = <src>`; `Asm` emits a
`llvm_asm!(...)` form with template, in/out lists,
clobbers, flags; remaining arms (Asm2, SetDropFlag,
SaveDropFlag, LoadDropFlag, Drop, ScopeEnd) similarly
each emit a single-line structural rendering.

**`MirDumper::dump_mir`**
[`src/mir/dump.cpp:~41-156`, entries `MIR_Dump`
[`src/mir/dump.cpp:371`] and `MIR_Dump_Fcn`
[`src/mir/dump.cpp:376`]] -- higher-level dump that
wraps the operator with block IDs and statement
indices. Per-variant arms: `Assign`: `<dst> = <src>;`;
`Asm`: `(...) = asm! "..."(...)`; `Asm2`: `asm2!(...)`;
`SetDropFlag`: `df$<idx> = ...;`; `SaveDropFlag`:
`SaveDropFlag(<slot> BIT <bit_idx>)`; `LoadDropFlag`:
`LoadDropFlag(df$<idx> = <slot>)`; `Drop`:
`drop(<slot> ...);`; `ScopeEnd`: `// Scope End: <slots>`.

INFERENCE: dump output emits no source-info / span
context (consistent with §6). Each Statement renders
purely from its structural payload -- exactly what a
"store, not walk" IR's pretty-printer needs.

For RegseqModel **D-8-K**: mrustc's discipline is
per-Statement `operator<<`, not centralised. Same
discipline as stage 7 D-7-K. Stage 8 follows the
stage-7 precedent of deferring
`pretty.rs::render_statement` to stage 14.

Maps to **D-8-K** (pretty-printer for statements).

---

## 10. Cleanup-pass behaviour: no Statement variants stripped

mrustc has a `MIR_Cleanup` pass
[`src/mir/cleanup.cpp:~1350`] -- not a variant-stripping
pass. Its TU_MATCH dispatch
[`src/mir/cleanup.cpp:~1385`] processes Statements
in-place: `Assign` does heavy work (constant
replacement, coercion fixup, type re-resolution);
`Drop` / `SaveDropFlag` / `LoadDropFlag` clean their
lvalues; `SetDropFlag` / `ScopeEnd` are no-ops; `Asm` /
`Asm2` clean per-param lvalues. **No Statements are
removed**; payloads change, variant shape doesn't. The
crate iterator `MIR_CleanupCrate`
[`src/mir/cleanup.cpp:~1563`] never erases or filters
the `statements` vector.

INFERENCE: mrustc has no analog of rustc's
`RemoveStorageMarkers` or `CleanupPostBorrowck` because
the variants those passes target (StorageLive/Dead,
FakeRead, AscribeUserType) never exist in mrustc's IR
in the first place. The Statement variant set is
closed-form by construction: HIR-to-MIR lowering
produces exactly the 8 variants, and nothing downstream
adds or removes any.

For RegseqModel: **direct peer evidence** that a
Rust-to-C IR can have a closed-form Statement variant
set without any cleanup-style stripping passes. The
bridge's job is to translate rustc's 13-variant input
to RegseqModel's chosen subset; no in-RegseqModel
cleanup is needed if the choice is well-made up front.

Maps to **D-8-1.1** (variant cardinality + closure).

---

## 11. Cross-D-tag findings (summary)

Compact mapping of mrustc evidence to stage-8 D-tags
(detail in the per-section "Maps to" lines above):

- **D-8-1.1** (variant cardinality): 8 variants
  [`src/mir/mir.hpp:733-776`]; closed-form by
  construction (§10). Stage-7 / -11 / -13 absorb the
  drop-flag and asm material.
- **D-8-1.2** (Assign payload): flat
  `{ dst: LValue, src: RValue }`
  [`src/mir/mir.hpp:734-737`], no Box, no type witness;
  type-equality is a validator invariant
  [`src/mir/check.cpp:984-995`].
- **D-8-1.3** (SetDiscriminant): folded into
  `RValue::EnumVariant` (§3); mrustc has no
  Statement-level discriminant-set. RegseqModel will
  likely preserve rustc's variant per
  information-preservation.
- **D-8-1.4** (Intrinsic): always
  `Terminator::Call(CallTarget::Intrinsic{name, params})`
  [`src/mir/mir.hpp:620-627`,
  `src/trans/codegen_c.cpp:4840-4850`]; mrustc has no
  `Statement::Intrinsic`. RegseqModel will likely retain
  rustc's NonDiverging Statement subset.
- **D-8-2** (interning): inline `vec<Statement>`
  [`src/mir/mir.hpp:778-782`], no interner.
- **D-8-3** (wrapper / source-info): Statement carries
  no span; `push_stmt_*` consumes Span for diagnostics
  only [`src/mir/from_hir.hpp:~290`].
- **D-8-4** (invariants): per-variant `MIR_BUG` /
  `MIR_ASSERT` checks
  [`src/mir/check.cpp:938-1341`], deeper move-state
  pass [`src/mir/check_full.cpp:857`]; fail-fast
  rejection.
- **D-8-K** (pretty-printer): per-variant
  `Statement::operator<<` [`src/mir/mir.cpp:689-722`]
  + higher-level `MIR_Dump`
  [`src/mir/dump.cpp:371-376`]; defer
  `pretty.rs::render_statement` to stage 14.

---

## 12. Cross-reference vs rustc_middle

Where mrustc made a different choice from rustc, those
divergences are themselves the evidence relevant to a
peer Rust-to-C compiler. Summary:

| Concern | rustc | mrustc | Divergence reason |
|--|--|--|--|
| Variant count | 13 | 8 | mrustc strips borrowck markers (FakeRead, AscribeUserType, PlaceMention, Retag), coverage, nop, storage markers; folds SetDiscriminant into aggregate-rvalue assigns; routes intrinsics through Call. |
| StorageLive/Dead | Yes | No | mrustc has function-wide locals; stack lifetime is C-compiler concern. |
| FakeRead / AscribeUserType / PlaceMention | Yes | No | mrustc has no NLL borrowchecker consuming these. |
| SetDiscriminant | Yes (Statement) | No (folded into RValue::EnumVariant) | mrustc constructs whole enums; never updates a tag in place. |
| Intrinsic location | Statement (NonDiverging) + Call | Call only | mrustc unifies all intrinsics through the Call terminator. |
| Inline asm | Single Terminator | Two Statement variants (legacy + stable) | mrustc historical: Asm predates Asm2; both retained. |
| Drop-flag plumbing | Assign-to-bool + Switch on bool | First-class SetDropFlag / SaveDropFlag / LoadDropFlag Statement variants | mrustc materialises drop-flag operations directly. |
| ScopeEnd marker | (not present at Statement level) | Present (debug-marker) | mrustc explicitly tracks scope-end at the Statement level. |
| Statement source info | `SourceInfo { span, scope }` per Statement | None at all | mrustc drops Statement-level spans. |
| Cleanup-pass stripping | `RemoveStorageMarkers`, `CleanupPostBorrowck` | None | mrustc's variant set is closed-form by construction; nothing to remove. |

INFERENCE: mrustc's design choices reflect a
post-MIR Rust-to-C transpiler that does not need
borrowck-internal markers, does not need rustc-internal
debug or coverage hooks, and is willing to trade
diagnostic granularity (Statement-level spans) for IR
simplicity. RegseqModel's design decisions sit between
mrustc and rustc: the bridge consumes rustc's full
13-variant surface, but the translator's storage choice
is closer to mrustc (small closed-form variant set).

---

## 13. Synthesis -- what mrustc says about a "store-not-walk" Rust-to-C IR

mrustc demonstrates that the Statement surface
surviving into a Rust-to-C C-emit target is small and
closed-form. Findings, condensed:

1. **Assign is irreducible** -- flat `{ dst: LValue,
   src: RValue }` is the minimum viable shape.
2. **SetDiscriminant is optional** -- mrustc folds it
   into rvalue-side `EnumVariant`; RegseqModel's
   information-preservation rule favours retention.
3. **Intrinsic-as-Statement is optional** -- mrustc
   routes everything through Call; rustc keeps a
   NonDiverging subset at the Statement level. Either
   is a valid peer choice.
4. **Storage markers, borrowck markers (FakeRead /
   AscribeUserType / PlaceMention / Retag), coverage,
   nop are all unnecessary** -- mrustc has none.
5. **Drop-flag plumbing can be first-class** as
   Statement variants (mrustc) or open-coded as
   Assign-to-bool + Switch (rustc); a stage-13
   drop-elaboration concern.
6. **ScopeEnd is a debug marker** -- mrustc uses it;
   the C consumer renders it as a comment.
7. **No Statement-level source info is workable** --
   mrustc accepts diagnostic loss; RegseqModel's
   sidecar-table proposal is a third design point.
8. **No Statement interning is correct** -- inline
   `vec<Statement>` works.
9. **Type-equality is a validator-pass concern**
   (mrustc's `MIR_Validate`), not a structural slot.
10. **No cleanup-style variant stripping needed** --
    closed-form by construction.

Collectively this supports stage 8 designing toward a
small (closer to mrustc's 8 than rustc's 13),
closed-form, inline, post-hoc-validated Statement set.
The choice points -- SetDiscriminant retention,
Intrinsic-as-Statement retention -- are bridge-input
shape decisions: RegseqModel preserves what rustc
emits unless there's a compelling reason to fold.
mrustc's evidence shows folded shapes are workable but
lossy; information preservation drives RegseqModel
toward more variants than mrustc, fewer than rustc.

---

## 14. What's NOT in this pass

- **`HIR::PathParams` / `HIR::Path` definitions** --
  HIR-namespace, not `mir.hpp`. The intrinsic name
  (`RcString`) and the per-name codegen dispatch
  [`src/trans/codegen_c.cpp:5859`] are evidence enough
  for D-8-1.4.
- **`AsmCommon::*` definitions** -- referenced by `Asm2`
  / `AsmParam` [`src/mir/mir.hpp:744-749`,
  `src/mir/mir.hpp:640-650`], defined in
  `../hir/asm.hpp`. The fact that mrustc splits asm
  into legacy `Asm` + stable `Asm2` Statement variants
  is the relevant evidence; param shape is a stage-11
  Terminator concern (rustc places InlineAsm there).
- **Per-RValue-arm validation in
  `MIR_Validate`'s Assign branch**
  [`src/mir/check.cpp:971-1217`] -- stage-7 RValue
  invariants, not Statement-shape; stage-7 prior-art
  covers it.
- **`emit_intrinsic_call`'s per-intrinsic body**
  [`src/trans/codegen_c.cpp:5859+`,
  banner at 7519: "Platform Intrinsics (and SIMD)"]
  -- per-name semantics are a stage-12+ concern.
- **Drop-flag bitset layout** -- a stage-13
  drop-elaboration concern. Evidence at
  [`src/mir/mir.hpp:760`,
  `src/trans/codegen_c.cpp:3711-3717`] suffices for
  Statement-vocabulary scoping.
- **Borrow-check pass behaviour on Statement** --
  `src/mir/borrow_check.cpp` not pursued. D-8-4 is
  about bridge-side Statement-shape invariants, not
  borrowck; mid-stage top-up can land it if needed.
