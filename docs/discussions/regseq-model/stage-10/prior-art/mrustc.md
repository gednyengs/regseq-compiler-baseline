# Stage 10 -- Prior Art: mrustc primary source

Primary-source mrustc evidence for the **MIR basic-block
container vocabulary**, scoped to what stage 10's candidate
decision namespace
(`D-10-1` per-block `BasicBlock` struct shape; `D-10-2`
body-level basic-block container shape; `D-10-3` `BlockId`
typed-index introduction; `D-10-4` block-ordering invariant
(RPO inheritance from `ReorderBasicBlocks`); `D-10-5`
invariants registry (rejection table + positive invariants
+ walk-impl deferral); `D-10-K` pretty-printer deferral and
`bbN` sigil) needs.

mrustc is the closest existing peer: a Rust-to-C compiler
with its own HIR/MIR, written in C++. Its `MIR::BasicBlock`
and `Function::blocks` directly inform the same design space
as RegseqModel's `BasicBlock` and the body-level
basic-block table. The stage-{6,7,8,9}/prior-art mrustc
files
(`./../../stage-6/prior-art/mrustc.md`,
`./../../stage-7/prior-art/mrustc.md`,
`./../../stage-8/prior-art/mrustc.md`,
`./../../stage-9/prior-art/mrustc.md`) already covered the
`LValue`, `Param`, `Constant`, `RValue`, `eBinOp`, `eUniOp`,
`Statement`, and `Terminator` surfaces; this file does not
re-derive that material. It focuses on the **container**
into which Statements (stage 8) and Terminators (stage 9)
are placed: the per-block `BasicBlock` struct, the body-
level `Function::blocks` vector, the `BasicBlockId` typed-
index, block-ordering discipline, validation reachability
sweeps, and pretty-printer sigil/structure.

Repository: https://github.com/thepowersgang/mrustc

Master HEAD as of this pass: commit
`7392eca5bd4958cb184fb47cefbd4c7e4f43547b` (committer date
2026-04-13, "standalone_miri - Compile fixes"). Same commit
as stage-9. All file:line citations refer to that tree.

Primary files inspected:

- `src/mir/mir.hpp` -- IR type definitions
  (`BasicBlockId` typedef, `BasicBlock` struct, `Function`
  class, `Cloner::map_bb_idx`).
- `src/mir/mir_builder.cpp` -- block construction:
  `MirBuilder::new_bb_unlinked`, `new_bb_linked`,
  `set_cur_block`, `end_block`, `pause_cur_block`.
- `src/mir/from_hir.cpp` -- the HIR-to-MIR walker that
  drives block creation order; `MirBuilder` callers across
  match-arm fan-out, loop heads, function-call ret/panic
  pairs.
- `src/mir/check.cpp` -- `MIR_Validate` per-block rules:
  CFA reachability sweep, terminator non-DEAD assertion,
  flat per-block per-statement type checks.
- `src/mir/dump.cpp` -- `MirDumper::dump_mir` per-block
  rendering: `bbN: { ... }` envelope.
- `src/mir/optimise.cpp` -- inspected for any reverse-
  postorder / topological reorder pass; none present.
- `src/trans/codegen_c.cpp` -- C-emit consumer: linear
  block iteration loop at line 3187, label-omission
  optimisations.
- `src/mir/mir.cpp` -- per-construct operator<< /
  operator==; inspected to confirm there is no `BasicBlock`-
  level structural operator defined.

Every factual claim cites `[file:line]` in the mrustc
tree at this commit. Hypotheses are flagged
**HYPOTHESIS:**. Cross-source synthesis is flagged
**INFERENCE:**.

Stage-context anchor: this file does not claim parity with
rustc, rustc_public, or charon -- those are sibling
prior-art passes. mrustc consumes a much older rustc and
predates significant post-MIR work in modern rustc; drift
is expected, and where mrustc made a different design
choice that divergence is itself the evidence relevant to
a Rust-to-C peer compiler.

---

## 1. mrustc `MIR::BasicBlock` -- 2-field struct, no cleanup flag, no debuginfo, no scope

mrustc's `BasicBlock` is a 2-field plain struct
[`src/mir/mir.hpp:726-730`]:

```cpp
struct BasicBlock
{
    ::std::vector<Statement>    statements;
    Terminator  terminator;
};
```

Field tally:

| Field | Type | Notes |
|--|--|--|
| `statements` | `std::vector<Statement>` | Sub-terminator instruction list; element type is the 8-variant tagged union from stage-8 prior-art §1. |
| `terminator` | `Terminator` (inline-by-value) | The 9-variant tagged union from stage-9 prior-art §1. Default-constructed as `Terminator::Incomplete{}` -- the validator rejects this default if it ever survives `MIR_Validate` [§4 below]. |

What mrustc's `BasicBlock` does **not** have:

- **No `is_cleanup` flag.** No equivalent of rustc's
  `BasicBlockData::is_cleanup`. mrustc encodes panic-edge
  state on the `Call` terminator (`panic_block:
  BasicBlockId` [`src/mir/mir.hpp:638`]) rather than on
  the destination block. INFERENCE: mrustc's stance is
  "the *terminator* knows which BB is its panic
  successor; the BB itself is just code." Any block can
  be reached as a panic block; the BB doesn't need to
  self-identify.
- **No debug/scope info.** No analog of rustc's
  `BasicBlockData::after_last_stmt_debuginfos:
  StmtDebugInfos<'tcx>` (introduced post-mrustc; see
  stage-8 rustc prior-art for the rustc shape). mrustc
  retains zero block-level DWARF emission targeting.
- **No span / source-info.** Spans live on individual
  Statements (where present) and not at the block level.
- **No loop / domination metadata.** No `is_loop_header`,
  no precomputed predecessors, no dominator info. These
  are recomputed by the validator on demand
  [§4 below].

INFERENCE: mrustc's `BasicBlock` is the structural minimum
to express "a sequence of stage-8 Statements followed by a
single stage-9 Terminator." Anything that can be derived
on-demand from the terminator's successor list is *not*
stored on the block. `is_cleanup` survives in rustc
because MSVC SEH codegen needs it; mrustc-the-Rust-to-C-
transpiler does not target SEH and so does not carry it.

Maps to **D-10-1** (per-block `BasicBlock` struct shape).
mrustc evidence: a Rust-to-C peer can be built with a
2-field `BasicBlock`. RegseqModel's stage-9 preview at
`spec.md:8448-8453` matches mrustc's shape exactly.

---

## 2. Body-level container `Function::blocks` -- bare `std::vector<BasicBlock>`, no cache

mrustc's body-level basic-block container is the
`Function::blocks` field [`src/mir/mir.hpp:746-757`]:

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

Salient observations:

- **`blocks: std::vector<BasicBlock>`** -- bare standard-
  library vector. Position in the vector *is* the
  `BasicBlockId`.
- **No wrapper struct around `blocks`.** No
  `BasicBlocks` analog of rustc's
  `rustc_middle::mir::BasicBlocks<'tcx>` (which wraps an
  `IndexVec<BasicBlock, BasicBlockData<'tcx>> +
  Arc<Cache>` with predecessors / RPO / dominators
  caches).
- **No CFG cache.** No precomputed predecessors,
  reverse-postorder, or dominators. The validator
  [§4 below] re-walks reachability from BB0 on every
  invocation.
- **`trans_enum_state`** is a translation-state cache,
  not a CFG cache; out of scope.
- **No body-level "entry block" field.** Entry is
  implicit: `blocks[0]` is the entry block, by
  convention.

The MirBuilder constructs `blocks` element-by-element via
`new_bb_unlinked()` [`src/mir/mir_builder.cpp:824-830`]:

```cpp
::MIR::BasicBlockId MirBuilder::new_bb_unlinked()
{
    auto rv = m_output.blocks.size();
    DEBUG("BB" << rv);
    m_output.blocks.push_back({});
    return rv;
}
```

Each new block is appended; its ID is its position in the
vector. `new_bb_linked()` [`src/mir/mir_builder.cpp:816-823`]
extends this by ending the current block with a `Goto` to
the new one before switching `m_current_block` to the new
ID. `end_block(Terminator term)`
[`src/mir/mir_builder.cpp:795-803`] writes the terminator
into `m_output.blocks.at(m_current_block).terminator`.

INFERENCE: mrustc's discipline is "the body-level
container is a bare growable vector, the index type is a
bare integer alias, and the entry block is whatever ends
up at position 0." Any analytical structure (predecessors,
domination, loop tree) is *not* in the IR -- it is
recomputed by passes that need it.

For **D-10-2** (body-level basic-block container shape),
mrustc presents the **maximally bare** point in the
design space: bare std-vec, no wrapper, no cache. Direct
peer evidence that a Rust-to-C transpiler can ship without
cached CFG metadata at the IR level.

The two RegseqModel-relevant alternatives:

| | rustc_middle | mrustc | rustc_public | RegseqModel candidate |
|--|--|--|--|--|
| Container | `BasicBlocks<'tcx>` wrapper | `std::vector<BasicBlock>` bare | `Vec<BasicBlock>` bare | `IndexVec<BlockId, BasicBlock>` |
| CFG cache | predecessors / RPO / dominators | none | none | none (deferred to `regseq-analysis`) |
| Index type | `BasicBlock` newtype_index | `BasicBlockId = unsigned int` typedef | `BasicBlockIdx = usize` typedef | `BlockId` newtype_index |
| Entry block | `START_BLOCK` const, encoded in newtype | implicit position 0 | implicit `START_BLOCK = 0` constant, no enforcement | `START_BLOCK = BlockId(0)` const + invariant |

Maps to **D-10-2** (body-level basic-block container shape).
mrustc + rustc_public agree on bare-container; rustc_middle
adds the cache. RegseqModel's choice is which pole to
target.

---

## 3. `BasicBlockId = unsigned int` -- bare typedef, no newtype wrapper

mrustc's basic-block index type is a bare integer typedef
[`src/mir/mir.hpp:24`]:

```cpp
typedef unsigned int    BasicBlockId;
```

Properties:

- **Bare typedef.** Implicitly convertible to/from
  `unsigned int`. No type-system distinction between
  `BasicBlockId` and any other `unsigned`. No `Idx`-
  trait analog (mrustc is C++; the convention works
  through naming, not types).
- **`size_t` width on most platforms via `unsigned
  int`.** rustc uses a `u32`-backed newtype. mrustc's
  `unsigned int` is platform-defined but uniformly 32-bit
  on the Linux/macOS targets mrustc supports.
- **Index is position in `Function::blocks`.** No
  separate "BB ID" -- the index *is* the ID. New BBs get
  `m_output.blocks.size()` as their ID
  [`src/mir/mir_builder.cpp:826`].

The `Cloner::map_bb_idx(BasicBlockId idx)` virtual method
[`src/mir/mir.hpp:765-767`] supports renumbering during
inlining / merging; default is identity. This is the only
point in mrustc where BB IDs get remapped en masse.

INFERENCE: mrustc deliberately accepts the type-safety
loss of bare integers in exchange for unified
integer-arithmetic operations on BB IDs (additions,
modular indexing, etc., used by the renumberer). RegseqModel
inherits stage-1's `newtype_index!` machinery and so
diverges from mrustc here -- but the mrustc evidence
confirms a peer can ship without typed indices and survive
the validator's bounds checks.

For **D-10-3** (`BlockId` typed-index introduction):
mrustc's evidence is the *negative* case -- a Rust-to-C
peer demonstrates that a bare `unsigned int` typedef is
sufficient for correctness. RegseqModel's choice to use
`newtype_index!` follows rustc/rustc_middle's stronger
discipline (and stage-1's already-locked convention),
which trades the renumbering convenience for stronger
type checking on `BlockId`-vs-`LocalId`-vs-other-u32-ID
mix-ups.

Sigil: mrustc renders `BasicBlockId` as `bb<N>` everywhere
[`src/mir/dump.cpp:43,158,164,169,177,...`,
`src/trans/codegen_c.cpp:3194,3221,3226`]. Direct match to
RegseqModel's `bbN` sigil decision.

Maps to **D-10-3** (BlockId typed-index). mrustc evidence:
bare-integer typedef survives, but the `bbN` sigil is
universal across the renderer / codegen.

---

## 4. Block-ordering invariant -- mrustc preserves construction order, no RPO reorder pass

mrustc has **no analog of rustc's `ReorderBasicBlocks`
pass**. Search of the optimisation pipeline:

- `src/mir/optimise.cpp` (6339 lines) -- inspected;
  contains inlining, constant-folding, dead-block
  elimination, branch / unify / simplify passes; no
  `reorder`, `reverse_post`, `topolog`, `RPO`, or
  `post.order` matches.
- `src/mir/check.cpp` -- inspected; reachability sweep
  is read-only (visits, doesn't reorder).
- `src/mir/mir_builder.cpp` -- inspected; no reorder
  call.

Block ordering in mrustc is therefore **HIR-walk
construction order**: whatever sequence of
`new_bb_unlinked()` calls
[`src/mir/mir_builder.cpp:824-830`] happens during HIR-to-
MIR lowering [`src/mir/from_hir.cpp` passim] determines
the IR's block layout. Examples from `from_hir.cpp`:

- Match-arm fan-out:
  `arm_targets.push_back(m_builder.new_bb_unlinked())`
  [`src/mir/from_hir.cpp:121,138,188,212`]. Arms get
  block IDs in the order they appear in source.
- Loop heads:
  `auto loop_next = m_builder.new_bb_unlinked()`
  [`src/mir/from_hir.cpp:911`]. Loop-back and exit blocks
  allocated lazily during loop body lowering.
- Function-call ret/panic pairs:
  `auto bb_ret = m_builder.new_bb_unlinked();
  auto bb_panic = m_builder.new_bb_unlinked()`
  [`src/mir/from_hir.cpp:800-801,821-822`]. ret-block
  always gets a smaller ID than panic-block; same call's
  panic block follows it immediately.

INFERENCE: mrustc's order is *approximately* RPO because
HIR is structured: an HIR-walk visits a syntactic
construct's body before its successors, naturally
producing forward-edge-dominant ordering. But this is
**not enforced**: a back-edge from a loop tail to the
loop head produces a back-edge in the BB-ID space too
(loop_next allocated after the loop body but referenced
by the body's terminator), and there is no pass that
canonicalises this.

What mrustc's C-emit consumer relies on is **NOT** a
reverse-postorder invariant -- it is the much weaker
"last-block ends with `Diverge` so any unreached label
is a no-op" property, plus a label-omission optimisation
[`src/trans/codegen_c.cpp:3215-3222`]:

```cpp
else if( bb_use_counts.at(i) == 1 )
{
    if( i > 0 && (TU_TEST1(code->blocks[i-1].terminator, Goto, == i)
                  || TU_TEST1(code->blocks[i-1].terminator, Call, .ret_block == i)) )
    {
        // Don't print the label, only use is previous block
    }
    else { m_of << "bb" << i << ":\n"; }
}
```

The optimisation triggers when the *previous* block (in
linear order) goto's the current block: emit no label,
let C fall through. This is order-sensitive **only** in
the sense that "block i-1 -> i fall-through saves a
label"; it does not require *all* successors of block i-1
to have higher IDs than i-1.

For **D-10-4** (block-ordering invariant -- RPO inheritance
from `ReorderBasicBlocks`):

- mrustc evidence: RPO is *not* an invariant. Construction
  order is roughly RPO due to HIR being structured, but
  this is not enforced or relied on by passes downstream.
- Direct peer divergence: RegseqModel inherits RPO from
  rustc's `ReorderBasicBlocks` *because the bridge
  consumes rustc MIR after that pass has run*. mrustc
  builds MIR from HIR directly, so it has no "incoming
  RPO" to preserve.

**HYPOTHESIS:** RegseqModel's choice to keep RPO as a
positive invariant (rather than dropping it like mrustc)
trades implementation simplicity for the diagnostic
benefit of "block IDs are deterministic across rebuilds
of the same input." mrustc accepts non-determinism here
because its IDs are not exposed as a stability surface.

Maps to **D-10-4** (RPO inheritance + START_BLOCK = 0
invariants). mrustc evidence: a Rust-to-C peer can ship
without RPO ordering; the price is non-deterministic
block IDs across rebuilds.

---

## 5. Validator (`MIR_Validate`) per-block rules + reachability sweep

mrustc's `MIR_Validate` runs three block-level checks
[`src/mir/check.cpp:636-737`]:

**Sweep A: terminator non-DEAD assertion**
[`src/mir/check.cpp:662-668`]:

```cpp
for(const auto& bb : fcn.blocks)
{
    state.set_cur_stmt_term(&bb - &fcn.blocks.front());
    MIR_ASSERT(state, bb.terminator.tag() != ::MIR::Terminator::TAGDEAD, "Moved terminator");
}
```

Every BB's terminator must have a live tag. The default-
constructed `Terminator::Incomplete{}` is *not* DEAD
(DEAD is the moved-from state of the tagged-union
machinery); `Incomplete` is rejected separately in sweep
B.

**Sweep B: CFA reachability**
[`src/mir/check.cpp:670-737`]. The comment is the
load-bearing structural invariant:

```
// [CFA] = Control Flow Analysis
// - [CFA] All code paths from bb0 must end with either a return or a diverge (or loop)
//  - Requires checking the links between basic blocks, with a bitmap to catch loops/multipath
```

Implementation:

```cpp
bool returns = false;
::std::vector<bool> visited_bbs( fcn.blocks.size() );
::std::vector<unsigned int> to_visit_blocks;
to_visit_blocks.push_back(0);
while( to_visit_blocks.size() > 0 )
{
    auto block = to_visit_blocks.back();
    to_visit_blocks.pop_back();
    MIR_ASSERT(state, block < fcn.blocks.size(),
        "Ended up with BB out of range (" << block << " >= " << fcn.blocks.size() << ")");
    if( visited_bbs[block] ) continue;
    visited_bbs[block] = true;
    // ...
    #define PUSH_BB(idx, desc)  do {\
        if( !(idx < fcn.blocks.size() ) )   MIR_BUG(state, ...);\
        if( visited_bbs[idx] == false ) { to_visit_blocks.push_back(idx); }\
    } while(0)
    TU_MATCH(::MIR::Terminator, (fcn.blocks[block].terminator), (e),
    (Incomplete, MIR_BUG(state, "Encounterd `Incomplete` block in control flow - BB" << block);),
    (Return,     returns = true;),
    (Diverge,    /* no successor */),
    (Goto,       PUSH_BB(e, "Goto");),
    (Panic,      PUSH_BB(e.dst, "Panic");),
    (If,         PUSH_BB(e.bb_true, "If true"); PUSH_BB(e.bb_false, "If false");),
    // ... (Switch, SwitchValue, Call all PUSH_BB their successors)
    )
}
// after loop: assert returns is true (or function returns `!`) -- CFA structural invariant
```

Explicit per-block invariants captured:

| Invariant | Site | What it enforces |
|--|--|--|
| Terminator-not-moved | `check.cpp:666` | No DEAD-tag in any BB's terminator slot. |
| Terminator-not-Incomplete | `check.cpp:697-699` | Builder must have called `end_block` for every reachable BB. |
| Successor-bb-in-range | `check.cpp:691` (`PUSH_BB`) | Every BB-id reference (Goto, Panic, If, Switch, SwitchValue, Call ret/panic) is `< fcn.blocks.size()`. Stage-9 prior-art §7 covered terminator-payload validity; this invariant is the *block-table-bounds* slice of that. |
| CFA structural | `check.cpp:670` (comment) + reachability sweep + `returns` check | All reachable paths from BB0 terminate (Return / Diverge / loop). Equivalent to "every reachable BB's terminator is non-DEAD and the terminator's successor-set is entirely in-bounds." |
| Reachable-from-BB0 walk anchor | `check.cpp:676` | BB0 is the entry. Implicitly: `START_BLOCK = 0`. |

**Sweep C: flat per-block per-statement type checks**
[`src/mir/check.cpp:739-1338`]. Iterates
`for(unsigned int bb_idx = 0; bb_idx < fcn.blocks.size();
bb_idx++)` and per-statement / per-terminator type checks.
Stage-8 prior-art §7 and stage-9 prior-art §7 covered the
per-statement / per-terminator details respectively. The
block-level contribution is *only* the iteration shape:
linear walk over `fcn.blocks` with `(bb_idx, stmt_idx)`
state.

Notably **absent** from `MIR_Validate`:

- **No "all blocks reachable" requirement.** Unreachable
  blocks (no path from BB0) are tolerated silently --
  the reachability sweep simply doesn't visit them. The
  flat sweep (C) still type-checks them, so they cannot
  be malformed; they just don't have to be reachable.
  HYPOTHESIS: this is intentional because monomorphisation-
  time const folding can leave dead branches of `if
  const { ... }` unreached, and mrustc doesn't run a
  "remove unreachable" pass before validation.
- **No `is_cleanup`-style structural check.** Without
  the field, no validation rule for it.
- **No "terminator successor-count matches expected for
  variant" check beyond per-terminator type rules from
  stage-9.**
- **No per-block ordering check.** The validator does
  not require RPO; it does not check that BB0 is the
  first block (that's an *implementation* assumption
  that BB0-as-entry is preserved by `MirBuilder`,
  enforced only by convention).

For **D-10-5** (per-stage invariants registry), mrustc's
`MIR_Validate` is direct prior art for the **walk-impl**
side of the registry. Mapping:

| RegseqModel candidate B-row | mrustc `MIR_Validate` analog |
|--|--|
| terminator non-None (post-construction) | `bb.terminator.tag() != TAGDEAD` + `Incomplete` rejection [`check.cpp:666,697-699`] |
| successors in 0..n_blocks | `PUSH_BB` bounds check [`check.cpp:691`] |
| START_BLOCK = BlockId(0) is the entry | implicit (`to_visit_blocks.push_back(0)` [`check.cpp:676`]) |
| CFA structural (every path returns/diverges) | comment-articulated structural invariant [`check.cpp:670`] enforced via `returns` flag + reachability sweep |
| RPO ordering of storage | ABSENT in mrustc (per §4) |
| All-blocks-stored regardless of reachability | implicit (validator tolerates unreachable; flat sweep type-checks them) |

| RegseqModel candidate A-row (rejection table) | mrustc handling |
|--|--|
| `is_cleanup=true` block (panic=abort discharge) | N/A in mrustc (no field) |
| `after_last_stmt_debuginfos` non-empty (DWARF discharge) | N/A in mrustc (no field) |
| `terminator: None` post-construction | `Incomplete` rejected [`check.cpp:697-699`] |

INFERENCE: mrustc's `MIR_Validate` is the literal prior-
art for stage-11's deferred walk implementation. Each
RegseqModel positive invariant is a per-block
case-split that mrustc has already done in the same
shape. RegseqModel adds the rejection-table discipline
(stages 5-9 precedent), which mrustc lacks.

Maps to **D-10-5** (per-stage invariants registry).
mrustc evidence: the registry-then-walker layering
matches stages-5-9; the per-block walker shape is
already-known.

---

## 6. Pretty-printer -- `MirDumper::dump_mir` block envelope

mrustc's `MirDumper::dump_mir`
[`src/mir/dump.cpp:26-48`] emits the body-level block
listing as:

```cpp
for(unsigned int i = 0; i < fcn.blocks.size(); i ++)
{
    const auto& block = fcn.blocks[i];
    m_os << indent() << "bb" << i << ": {\n";
    inc_indent();
    for(const auto& stmt : block.statements)
    {
        m_os << indent();
        // per-statement TU_MATCH_HDRA dispatch
        // ...
    }
    // per-terminator TU_MATCH dispatch on block.terminator
    // ...
    dec_indent();
    m_os << indent() << "}\n";
}
```

Block envelope shape (line-by-line):

```
bb<i>: {
    <statement-1>;
    <statement-2>;
    ...
    <terminator>;
}
```

Notes:

- **Sigil is `bb<i>`** [`src/mir/dump.cpp:43`]. Decimal,
  zero-padded only as wide as needed (no `bb000`-style
  zero-padding).
- **Brace-wrapped block** with `bb<i>: {` opening and `}`
  closing on their own lines, indentation increased
  inside.
- **Statements rendered per-Statement-variant via
  `TU_MATCH_HDRA`** [`src/mir/dump.cpp:49-145`]. Stage-8
  prior-art §9 covered the Statement-level rendering.
- **Terminator rendered last on its own line** without
  trailing semicolon [`src/mir/dump.cpp:147-232`]. Stage-9
  prior-art §10 covered the Terminator-level rendering.
  Examples:
  - `goto bb<n>;` [`dump.cpp:158`]
  - `panic bb<n>;` [`dump.cpp:161`]
  - `if <cond> { goto bb<t>; } else { goto bb<f>; }`
    [`dump.cpp:164`]
  - `<ret> = <callee>( <args>, ) goto bb<ret> else
    bb<panic>` [`dump.cpp:230`]
- **Block index is its position in `fcn.blocks`** -- no
  separate ID printed, just the position.
- **No source-info preamble** at block level (no `// from
  <span>` comment, no scope marker).

`MirDumper` also pre-renders `Function::locals` and
`Function::drop_flags` *before* the block listing
[`src/mir/dump.cpp:28-35`]:

```cpp
for(size_t i = 0; i < fcn.locals.size(); i ++)
    m_os << indent() << "let _$" << i << ": " << fcn.locals[i] << ";\n";
for(unsigned int i = 0; i < fcn.drop_flags.size(); i ++)
    m_os << indent() << "let df$" << i << " = " << fcn.drop_flags[i] << ";\n";
```

The body-level container is rendered as: locals header,
drop-flags header, then linear block listing.

For **D-10-K** (`render_basic_block` + `bbN` sigil
deferral): mrustc's discipline is direct peer evidence
that:

1. The `bbN` sigil with no zero-padding is sufficient
   (rustc, rustc_public, mrustc, charon all converge on
   this).
2. The `bbN: { ... }` block envelope is the canonical
   layout shape; statements then terminator, indented
   inside.
3. No source-info preamble at block level is needed.

Stage-1's pretty-printer scaffold already declared the
`bbN` sigil convention [`crates/regseq-model/src/pretty.rs:29`];
stage 10 inherits it. The `render_basic_block`
implementation defers to stage 11 alongside the §22.5 /
§23.7 deferrals (stage-8 D-8-K and stage-9 D-9-K
precedent).

Maps to **D-10-K** (pretty-printer deferral). mrustc
evidence: the renderer is a straightforward per-block
case-split with no source-info context required.

---

## 7. C-emit consumer block-iteration pattern

mrustc's C codegen iterates `Function::blocks` linearly
[`src/trans/codegen_c.cpp:3187-3320`]:

```cpp
for(unsigned int i = 0; i < code->blocks.size(); i ++)
{
    TRACE_FUNCTION_F(p << " bb" << i);

    // HACK: Ignore any blocks that only contain `diverge;`
    if( code->blocks[i].statements.size() == 0 && code->blocks[i].terminator.is_Diverge() ) {
        DEBUG("- Diverge only, omitting");
        m_of << "bb" << i << ": _Unwind_Resume(); // Diverge\n";
        continue ;
    }

    // Label-omission optimisation when use-count is 0 or 1 + previous-block falls through
    if( bb_use_counts.at(i) == 0 ) {
        if( i == 0 ) { /* First BB, don't print label */ }
        else { /* Unused BB (likely part of unsupported panic path) */ continue; }
    }
    else if( bb_use_counts.at(i) == 1 ) {
        if( i > 0 && (TU_TEST1(code->blocks[i-1].terminator, Goto, == i)
                      || TU_TEST1(code->blocks[i-1].terminator, Call, .ret_block == i)) )
        { /* Don't print the label, only use is previous block */ }
        else m_of << "bb" << i << ":\n";
    }
    else {
        m_of << "bb" << i << ":\n";
    }

    for(const auto& stmt : code->blocks[i].statements)
        emit_statement(mir_res, stmt);

    TU_MATCH_HDRA( (code->blocks[i].terminator), {)
    // ... per-terminator emission
    }
}
```

Salient observations:

- **Linear iteration over `code->blocks` by index**.
  No reverse-postorder traversal; the layout-as-stored
  *is* the C output order.
- **`bb_use_counts: vector<unsigned>`** precomputed
  [`src/trans/codegen_c.cpp:3067-3146`] -- a
  use-count of incoming BB-id references across all
  terminators in the function. Used for label-omission.
- **Diverge-only block omission** [3192-3196]: an empty
  block with `Diverge` terminator emits an inline
  `_Unwind_Resume();` at the label, not a full block.
  HYPOTHESIS: this saves a few bytes per panic-only BB
  in the C output but is not a correctness issue.
- **First-block (i=0) always emits without label**:
  it's the C function body's natural entry, no `bb0:`
  label needed. INFERENCE: this depends on the
  convention that `blocks[0]` is the entry and that the
  C compiler enters the function at its first
  statement.
- **Label-omission for unique-pred fall-through**: if
  block `i` has use-count 1 and the only use is from
  `blocks[i-1]`'s terminator, no label is emitted -- C's
  fall-through delivers the control transfer.

INFERENCE: mrustc's C-emit does *not* require RPO
ordering, but **does** require `blocks[0]` to be the
entry (relied on at line 3203). The "fall-through"
optimisation is opportunistic -- it kicks in when
construction order happens to align with control flow,
which is common but not guaranteed.

For RegseqModel: stage-10 doesn't own C-emit (that's
stage 14), but mrustc's iteration shape is direct prior
art for the post-stage codegen. Specifically:

- The "first block is the entry, no label" convention
  carries over to RegseqModel's `START_BLOCK =
  BlockId(0)` invariant.
- The label-omission optimisation is enabled by RPO
  ordering: with RPO, fall-through is *more* common,
  yielding shorter C output. This is an *advantage*
  RegseqModel can claim from inheriting `ReorderBasicBlocks`
  that mrustc forgoes.

Informs **D-10-4** (RPO ordering as a positive invariant
that codegen consumers can rely on for output quality).

---

## 8. Cloner / renumbering -- BB-id remapping during inlining

mrustc's `Cloner` base class [`src/mir/mir.hpp:759-797`]
provides a virtual `map_bb_idx` hook for renumbering
basic-block IDs during clones (e.g., during inlining or
function-merging passes):

```cpp
virtual ::MIR::BasicBlockId map_bb_idx(::MIR::BasicBlockId idx) const {
    return idx;
}
```

Default is identity. Subclasses (e.g., the inlining
cloner in `optimise.cpp`) override this to translate
source-function BB IDs into destination-function BB IDs.

For **D-10-3** (BlockId typed-index): mrustc's renumbering
discipline is *enabled* by the bare-integer typedef --
arithmetic on `BasicBlockId` is automatic. RegseqModel's
`newtype_index!`-typed `BlockId` would require explicit
conversions (`BlockId::from_u32` / `.as_u32()`) for the
same operation. HYPOTHESIS: this is a non-issue because
RegseqModel does not currently plan inlining or
function-merging passes; the cloner pattern is a stage-
14+ concern at most. The type-safety win on
"don't accidentally use a `BlockId` where a `LocalId`
is expected" outweighs the cost of explicit conversions
in the (currently absent) renumbering codepaths.

Note: mrustc's `Cloner` also has `map_local` and
`map_drop_flag` virtuals [`src/mir/mir.hpp:768-773`] for
the parallel local-renumbering operations. Same pattern.

Maps to **D-10-3** (BlockId introduction). mrustc evidence:
typed indices add explicit-conversion ceremony at
renumbering sites, which is a minor cost for the type-
safety win.

---

## 9. Stage-10 takeaways

D-tag-keyed summary:

- **D-10-1** (per-block `BasicBlock` struct shape):
  mrustc's struct is 2 fields -- `statements:
  std::vector<Statement>` + `terminator: Terminator`
  [`src/mir/mir.hpp:726-730`]. NO `is_cleanup`, NO
  debuginfo, NO scope, NO span. Direct peer evidence
  that a Rust-to-C transpiler can ship with the
  structural minimum. RegseqModel's stage-9 preview at
  `spec.md:8448-8453` matches mrustc's shape exactly.
- **D-10-2** (body-level basic-block container shape):
  mrustc uses bare `std::vector<BasicBlock>` at
  `Function::blocks` [`src/mir/mir.hpp:753`]. NO
  wrapper struct, NO CFG cache (no precomputed
  predecessors / RPO / dominators). Position in the
  vector *is* the BlockId. Together with rustc_public
  (also bare `Vec<BasicBlock>`), this is two-of-three
  peer evidence pointing at "bare container, no cache."
- **D-10-3** (`BlockId` typed-index introduction):
  mrustc uses `typedef unsigned int BasicBlockId`
  [`src/mir/mir.hpp:24`] -- bare typedef, no newtype
  wrapper. The `bbN` sigil is universal across
  validator, dump, and codegen renderings
  [`src/mir/dump.cpp:43,158,164,...`,
  `src/trans/codegen_c.cpp:3194,3221,3226`].
  RegseqModel's `newtype_index!`-typed `BlockId`
  diverges from mrustc on the typed-index axis but
  matches on the sigil.
- **D-10-4** (block-ordering invariant -- RPO inheritance
  from `ReorderBasicBlocks`): mrustc has NO RPO
  reorder pass. Block ordering is HIR-walk
  construction order, approximately RPO due to
  HIR-structure but not enforced or relied on
  downstream. Direct peer divergence: RegseqModel
  inherits RPO from rustc's pinned cleanup whitelist
  because the bridge consumes rustc MIR after
  `ReorderBasicBlocks` has run; mrustc builds MIR from
  HIR directly so has no incoming RPO to preserve.
- **D-10-5** (per-stage invariants registry):
  mrustc's `MIR_Validate` [`src/mir/check.cpp:636-737`]
  enforces: terminator non-DEAD [`check.cpp:666`],
  Incomplete-rejection [`check.cpp:697-699`], successor-
  bb-id-in-range [`check.cpp:691`], CFA structural
  (every reachable path returns or diverges)
  [`check.cpp:670` + reachability sweep], BB0-as-entry
  (implicit, [`check.cpp:676`]). NO is_cleanup check
  (no field), NO RPO check, NO all-blocks-reachable
  requirement. mrustc tolerates unreachable blocks
  silently. Direct prior art for stage-11's deferred
  walk implementation; the registry-then-walker
  layering matches stages-5-9.
- **D-10-K** (`render_basic_block` deferral and `bbN`
  sigil): mrustc's `MirDumper::dump_mir`
  [`src/mir/dump.cpp:26-48`] uses `bbN: { ... }` block
  envelope with statements then terminator, indented
  inside. Direct peer evidence the renderer is a
  straightforward per-block case-split with no
  source-info context required. RegseqModel inherits
  the `bbN` sigil convention from stage 1; the
  `render_basic_block` implementation defers to stage
  11 alongside stages 5-9's deferrals.

**Notable peer-design lessons from mrustc:**

1. **Two-field `BasicBlock` is sufficient.** mrustc, the
   only Rust-to-C peer in our prior-art set, ships with
   `{ statements, terminator }` and no other fields.
   The fields rustc carries (`is_cleanup`,
   `after_last_stmt_debuginfos`) exist for codegen-
   target reasons (MSVC SEH; DWARF emission) that don't
   apply to a C-emit transpiler.
2. **CFG metadata is not a permanent IR shape.**
   mrustc + rustc_public + RegseqModel-candidate-bare
   form a 3-way agreement that predecessors / RPO /
   dominators can be recomputed on demand by analysis
   passes; only rustc_middle caches them eagerly. The
   cache buys rustc query-system memoisation;
   RegseqModel's `regseq-analysis` crate is the
   architectural analog.
3. **RPO is incoming-data, not own-invariant.** mrustc
   has no incoming RPO and so doesn't claim one;
   RegseqModel inherits one from rustc's pinned
   `ReorderBasicBlocks` whitelist and so *can* claim
   it. The decision is whether to *make* the claim
   (positive invariant in §10.X) vs. leave it
   undocumented (use it for fall-through optimisation
   without naming it).
4. **CFA structural invariant is universal.** "Every
   reachable path from BB0 returns or diverges" is the
   one block-level invariant every Rust-MIR peer enforces.
   mrustc enforces it via reachability sweep + returns-
   flag check; rustc_middle relies on `terminator:
   Option<Terminator>` plus "always Some after
   construction" docstring; rustc_public makes terminator
   non-Optional structurally. All three converge on
   "CFA closes the body."
5. **Walk-impl per-block case-split is direct peer
   prior art.** `MIR_Validate`'s per-terminator
   case-split [`check.cpp:696-731`] is the literal
   reference structure D-10-5's stage-11 walker
   mirrors -- terminator-by-terminator successor
   pushing with bounds-check on every BB-id reference.

**Mapping mrustc's evidence to RegseqModel's six tags:**

| D-tag | mrustc shape | RegseqModel candidate | Divergence |
|--|--|--|--|
| D-10-1 | 2-field `{ statements, terminator }` | 2-field `{ statements, terminator }` | None -- direct match |
| D-10-2 | bare `std::vector<BasicBlock>` | `IndexVec<BlockId, BasicBlock>`, no cache | Same bare-container; RegseqModel uses typed-index storage |
| D-10-3 | `BasicBlockId = unsigned int` typedef | `BlockId` newtype_index | Type-safety divergence; sigil identical (`bbN`) |
| D-10-4 | No RPO invariant | RPO inherited from `ReorderBasicBlocks` | RegseqModel has one; mrustc has none |
| D-10-5 | `MIR_Validate` per-block sweeps | 4-part registry (rejection table + invariants + walk-deferral + F4/F4-2 OUT) | RegseqModel adds rejection-table discipline (stages 5-9 precedent) and explicit RPO + all-blocks-stored invariants; mrustc has neither |
| D-10-K | `bbN: { ... }` envelope, no source-info preamble | `bbN: { ... }` envelope, deferred to stage 11 | None -- direct match |

---

## 10. What's NOT in this pass

- **Optimisation passes** (`src/mir/optimise.cpp`,
  6339 lines) -- inlining, dead-block elimination,
  branch-unify, simplify-cfg touch BB structure but are
  out of scope for IR shape decisions.
- **Inlining cloner** specifics in
  `src/mir/optimise.cpp` -- the `Cloner::map_bb_idx`
  override site for inlining-time BB renumbering;
  cited at conceptual level (§8) but not detailed.
- **Borrow-check pass behaviour** on basic-block
  granularity -- mrustc's borrow-check is
  block-level-aware but the per-block invariants
  enforced are stage-14+ concerns.
- **Generator/coroutine state-machine lowering** --
  evidence at `src/mir/from_hir.cpp:127,233` shows
  lowering to Switch-on-state with dynamically-allocated
  per-state BBs; out of scope for stage-10 shape.
- **`MIR_Validate_ValState` move-tracking on a
  per-block basis** [`src/mir/check.cpp:563-633`] --
  cited in stage-9 §7 for terminator-level invariants;
  block-level state propagation (initial-state-per-BB
  bitvectors) is out of scope here.
- **`MIR_Validate_Full`** [`src/mir/check_full.cpp`] --
  per-statement / per-terminator deep state tracking
  with leak detection; out of scope for stage-10
  block-level shape.
- **C-emit `emit_statement` / `emit_term_*`
  internals** [`src/trans/codegen_c.cpp:3236+`] --
  per-construct C lowering; covered in stage-8 / stage-9
  prior-art.
- **HIR-walker block-creation order in detail**
  [`src/mir/from_hir.cpp` passim] -- §4 cited
  representative call sites for the construction-order
  claim; the full HIR-to-MIR algorithm is out of scope.
