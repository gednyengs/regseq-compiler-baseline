# CompCert prior-art for stage 4 (constants and literals)

Stage-4 prior-art research on **CompCert** -- the formally-verified C
compiler. CompCert is the confirmation tier in the stage-4 prior-art
set: its constant-value design has formal-verification pressure (every
operation justified by a Coq theorem) and reaches a small,
mathematically-clean primitive set. We use it as a sanity check on the
*minimum-viable* scalar surface and as a counter-example to the
"aggregate constants must be first-class" position taken by
charon / LLVM / stage-4-plan.

## Source map

All paths under `/tmp/regseq-stage4-research/CompCert/`. Citations
later in the file use repo-relative paths.

- `common/Values.v:34-40` -- the `val` type (run-time semantic values:
  `Vundef | Vint | Vlong | Vfloat | Vsingle | Vptr`).
- `common/Values.v:49-53` -- `Vnullptr`, `Vptrofs` definitions
  (architecture-dependent encodings).
- `common/Values.v:75-88, 154-201` -- `has_type`, `has_argtype`,
  `has_rettype` predicates (type judgements for values).
- `common/AST.v:31-43` -- `ident := positive`; `typ` enum (Tint /
  Tfloat / Tlong / Tsingle / Tany32 / Tany64).
- `common/AST.v:198-272` -- `memory_chunk` enum + per-chunk type and
  size mappings.
- `common/AST.v:278-298` -- `init_data` enum (8 variants) +
  `init_data_size`.
- `common/AST.v:320-325` -- `globvar` record carrying the
  `gvar_init: list init_data`.
- `common/Globalenvs.v:617-642` -- `store_init_data` + dispatch table:
  one `Mem.store` per `init_data` variant with the appropriate
  `memory_chunk`.
- `common/Globalenvs.v:649-666` -- `alloc_global`: zero-init then apply
  init_data list, then drop permission per `gvar_volatile`/`gvar_readonly`.
- `common/Globalenvs.v:910-924` -- `bytes_of_init_data`: lossy
  byte-level rendering (Init_space → zero bytes; failed Init_addrof →
  Undef memvals).
- `common/Globalenvs.v:1508-1544` -- `init_mem_inversion` /
  `store_init_data_list_free_idents` -- the *theorem*-level "validity
  check" for relocations.
- `backend/Cminor.v:37-43` -- `Cminor.constant`: 6 variants
  (`Ointconst`, `Ofloatconst`, `Osingleconst`, `Olongconst`,
  `Oaddrsymbol`, `Oaddrstack`).
- `backend/Cminor.v:125-130` -- `expr` containing `Econst : constant ->
  expr` (no aggregate).
- `cfrontend/Csyntax.v:31-32` -- `Eval (v: val) (ty: type)` -- the C
  source-level constant carrier (val + outer type annotation).
- `lib/Floats.v:31-32` -- `float = binary64`, `float32 = binary32`. No
  f16, no f128.

I read these files in full or in the cited regions; line numbers are
exact against the cloned tree (commit visible via `git -C
/tmp/regseq-stage4-research/CompCert rev-parse HEAD`). Coq syntax may
look unfamiliar; where I am inferring rather than reading
literally, the inference is marked.

## 1. The const-value type(s)

CompCert does **not** have a single `Const` enum. Constants appear in
**three distinct universes**, each with its own carrier:

### Universe A: `val` -- run-time semantic values

`common/Values.v:34-40`:

```coq
Inductive val: Type :=
  | Vundef: val
  | Vint: int -> val
  | Vlong: int64 -> val
  | Vfloat: float -> val
  | Vsingle: float32 -> val
  | Vptr: block -> ptrofs -> val.
```

6 variants. This is the **dynamic-semantics value type** -- what
expressions evaluate to. `int` is `Integers.int` (32-bit machine
integer), `int64` is 64-bit; `float`/`float32` are IEEE 754
binary64/binary32 (`lib/Floats.v:31-32`). `Vptr block ptrofs` is a
pointer with provenance (`block: positive` is an abstract memory
block; `ptrofs` is a target-pointer-width offset).

`Vundef` is "an arbitrary bit pattern, such as the value of an
uninitialized variable" (Values.v:30-31 docstring). It is a
first-class value, not an out-of-band marker.

Notable absences from `val`: no aggregate variant (no struct value,
no array value), no enum-discriminant value, no string. Aggregates
exist only as memory-block contents.

### Universe B: `init_data` -- compile-time global initializers

`common/AST.v:278-286`:

```coq
Inductive init_data: Type :=
  | Init_int8: int -> init_data
  | Init_int16: int -> init_data
  | Init_int32: int -> init_data
  | Init_int64: int64 -> init_data
  | Init_float32: float32 -> init_data
  | Init_float64: float -> init_data
  | Init_space: Z -> init_data
  | Init_addrof: ident -> ptrofs -> init_data.
```

8 variants. Used inside `globvar.gvar_init: list init_data`
(`AST.v:322`) to describe the initial bytes of a global variable.
This is the only universe where aggregate constants exist, and they
exist as a **flat sequence** over the global's memory layout, not
as a nested data structure.

`Init_space n` covers padding and explicit zero-init regions. `Init_addrof id ofs` is the only relocation form -- it stores
"address of symbol `id` plus offset `ofs`" at the current position.
Each variant has a known byte size (`init_data_size`,
`AST.v:288-298`).

### Universe C: `Cminor.constant` -- IR-operand-level scalar constants

`backend/Cminor.v:37-43`:

```coq
Inductive constant : Type :=
  | Ointconst: int -> constant
  | Ofloatconst: float -> constant
  | Osingleconst: float32 -> constant
  | Olongconst: int64 -> constant
  | Oaddrsymbol: ident -> ptrofs -> constant
  | Oaddrstack: ptrofs -> constant.
```

6 variants. This is the constant operand inside Cminor IR
expressions (`Cminor.v:125-130`: `expr` has `Econst : constant ->
expr` as one of five expression forms). **No aggregate variant.**
Each variant maps to a specific scalar shape; `Oaddrsymbol` is the
relocation form (mirrors `Init_addrof`); `Oaddrstack` is a
stack-pointer-relative offset constant (used for taking the
address of a stack-allocated local).

### Universe D: `Csyntax.Eval` -- C-source-level value literals

`cfrontend/Csyntax.v:31-32`:

```coq
Inductive expr : Type :=
  | Eval (v: val) (ty: type)                                  (**r constant *)
  | ...
```

C source expressions wrap a `val` together with an outer `type`
annotation. The `type` is full Compcert C `Ctypes.type` (richer
than the `typ` enum -- includes pointer / struct / union / array
shapes).

### Layering observation

The four universes are **deliberately stratified**:

- `val` is the run-time semantic primitive; it appears in expression
  reductions, memory loads, function arguments. It carries no type;
  the type comes from context (the language's typing predicate).
- `init_data` is the static-initializer language; it is *not* a
  subset of `val` -- it has explicit byte-width variants (Init_int8 …
  Init_int64) precisely because the global's memory layout demands
  byte-precise initialization. It also has `Init_space` (no `val`
  analogue) and `Init_addrof` (the relocation -- a `val`-level
  `Vptr` would require resolved provenance, which doesn't exist
  pre-link).
- `Cminor.constant` is the IR-operand scalar; it has no aggregate
  because aggregate-typed values don't appear in Cminor expressions
  (aggregates are accessed through pointer dereferences and copies).
- `Csyntax.Eval` is the C source-level literal carrier; it is `val +
  outer Ctype` precisely to separate semantic value from C type
  (necessary because C has integer-promotion / casting rules that
  reinterpret a `Vint` under different `Ctypes.type`s).

For comparison with the stage-4 plan: stage 4's `Const` enum is
expected to span universes A, B, and C of CompCert (a single carrier
covering value-semantic, static-init, and IR-operand roles). CompCert
deliberately keeps them distinct.

## 2. Type-tag placement

CompCert spreads the placement decision across the four universes:

| Universe          | Type-tag placement                                                                | Citation                  |
| ----------------- | --------------------------------------------------------------------------------- | ------------------------- |
| `val`             | **Fully external**. `Vint` carries `int`; type predicate comes from context.      | Values.v:34-40, 75-88     |
| `init_data`       | **Inline by name**. `Init_int8` vs `Init_int32` etc. encodes width + signedness.  | AST.v:278-286             |
| `Cminor.constant` | **Inline by name**. `Ointconst` (32-bit int) vs `Olongconst` (64-bit int) etc.    | Cminor.v:37-43            |
| `Csyntax.Eval`    | **Outer**. `Eval (v: val) (ty: type)` -- val + outer Ctype annotation.            | Csyntax.v:31-32           |

The `val` universe externalizes because it's a *semantic* primitive
-- the same machine integer represents a `Tint` value and a pointer
on 32-bit targets (Values.v:82 confirms: `Vptr _ _, Tint => Archi.ptr64
= false`). Inlining the type would force `Vint` and `Vptr` to be
distinct cases of the same scalar bit pattern, which would conflict
with the way `Vptrofs` and `Vnullptr` reuse `Vint`/`Vlong` (Values.v:49-53).

The `init_data` and `Cminor.constant` universes inline because each
variant maps to a distinct memory-store width / IR-operand shape (the
`store_init_data` dispatch at Globalenvs.v:617-631 illustrates this --
each variant chooses a specific `memory_chunk`). Without the inline
tag, the consumer cannot pick the chunk.

The `Csyntax.Eval` universe outer-annotates because C semantics
require richer type information (signed vs unsigned, pointer-to-T,
struct-typed) that doesn't fit the small `typ` enum.

### Stage-4 implication

CompCert validates **inline-tag-by-variant** for the IR-operand and
static-init levels (which is closest to stage 4's role). It validates
**externalized-tag** at the run-time-semantic level (which stage 4
does not have -- RegseqModel is bridge-built once, not stepped through
operationally). It validates **outer-tag** at the source-syntactic
level (which is also out-of-scope for stage 4 -- we don't emit a
post-translation source-AST).

The convergent finding across charon / MLIR / stage-4-plan / CompCert-
inner-universes is **inline-tag-by-variant** for IR-operand const
forms. LLVM's externalized form is the dissenting voice, motivated by
the operand-graph design (every `Constant` is also a `Value` with a
`Type *`).

## 3. Aggregate constants

CompCert's design here is the **strongest position any prior-art
project takes**: aggregate constants are *not* IR-level values at all.
They exist only at the static-init level, as flat sequences of
init_data. There is no `Cminor.constant` for an aggregate; there is
no `val` for an aggregate.

### Mechanism

A C global like `int xs[3] = {1, 2, 3};` lowers to:

```coq
gvar_init = [Init_int32 1; Init_int32 2; Init_int32 3]
```

A struct global lowers similarly: each field becomes one or more
`Init_*` entries, with `Init_space n` for trailing or interior padding
that the C ABI / repr requires. The list is **flat**, not nested.

For local aggregate initialisers (`int xs[3] = {1, 2, 3};` inside a
function), CompCert lowers them via the front-end translation:
elements are stored to the stack-allocated array one-at-a-time using
ordinary `Sstore` statements. There is no compile-time aggregate
const value held in IR; the IR holds individual scalar stores.

### How big aggregates work

`bytes_of_init_data` (Globalenvs.v:910-924) shows the byte-rendering:
each `Init_*` becomes its `inj_bytes` encoding; `Init_space n` becomes
`List.repeat (Byte Byte.zero) (Z.to_nat n)`; `Init_addrof` (post-
resolution) becomes `inj_value` of the resolved `Vptr`. The whole
init_data list is concatenated.

There is no sharing across globals. Two globals that happen to have
identical `gvar_init` lists are independent IR entities; there's no
content-keyed interning. Byte-level rendering is deterministic given
the resolved symbol-to-block map, so two compilations of the same
source produce the same bytes (within a single CompCert run).

### Rationale (inferred)

CompCert is justified-by-construction. Adding aggregate constants
at the value level would require:
- proving operational semantics for aggregate values in the value
  reduction relations (creates a large proof-engineering burden)
- justifying every transformation (constant folding, register
  allocation, etc.) preserves aggregate-value identity (often
  these transformations don't preserve aggregate identity in any
  useful sense)

Pushing aggregates entirely to memory layout collapses the proof
obligation: aggregate "equality" becomes byte-level memory equality,
which is already proven sound under CompCert's memory model.

### Stage-4 implication

CompCert is the **counter-example** to stage-4's planned
`Aggregate(Vec<ConstId>)` variant. It demonstrates that aggregate
constants *can* be deferred entirely to a static-init lowering phase
without losing expressiveness for a C-target compiler.

For stage 4, the question becomes: **what consumer requires
value-level aggregate constants in v1?**

- Generic-const args: scalar primitives only in v1 per spec.md §9.7.
  No aggregate need.
- Static initialisers: could go to a "static-init" sub-vocabulary
  (CompCert/mrustc shape) rather than as `Const::Aggregate`.
- Vtable instances (§13.2): aggregates with relocations. These are
  the strongest demand. CompCert handles them via flat init_data
  lists; charon handles them via inline `Vec<ConstantExpr>`; LLVM
  handles them via `ConstantStruct/Array`. Stage 4's plan mirrors
  charon/LLVM here.
- Const items (`InstanceKind::Const { value: ConstId }`): if the
  const item has an aggregate value, it needs aggregate
  representation. But could be deferred similarly to a static-init
  shape if v1 fixtures don't need const-aggregate-by-value uses.

The CompCert evidence does not refute stage-4's plan, but it
surfaces a viable alternative: **bridge-demote aggregate constants
to synthesized Static items at construction time.** The cost would
be one extra Item per aggregate-valued const; the benefit would be a
flatter `Const` enum (no `Aggregate` / `Enum` variants) and no
deep-canonicalisation requirement on aggregate trees.

## 4. Relocation handling

CompCert's relocation form is the simplest of the projects examined:

`init_data` level: `Init_addrof: ident -> ptrofs -> init_data`
(AST.v:286). Stores the address of symbol `ident` plus offset
`ptrofs`.

`Cminor.constant` level: `Oaddrsymbol: ident -> ptrofs -> constant`
(Cminor.v:42). Same shape.

### Resolution timing

Both forms carry **`ident` (a positive)** as the symbol identity.
`ident` is build-local: parser/elaborator assigns positives to symbol
names, with no cross-build stability claim.

Resolution happens at memory-initialization time (`store_init_data`,
Globalenvs.v:617-631):

```coq
| Init_addrof symb ofs =>
    match find_symbol ge symb with
    | None => None
    | Some b' => Mem.store Mptr m b p (Vptr b' ofs)
    end
```

If the symbol resolves to a block (`Vptr b' ofs`), store it; if
not, the entire `alloc_global` returns `None` and link fails. So
unresolved relocations are a hard error at the `init_mem` level.

### Validity theorem

`init_mem_inversion` (Globalenvs.v:1524-1544) proves: for every
program `p` whose `init_mem p` succeeds, every `Init_addrof` in any
global's `gvar_init` resolves to a `find_symbol` block. This is the
**theorem-level analogue of stage-4's planned construction
invariant**: it's not "checked at run time" but proven at proof
time. The dual at link time is that `init_mem` simply fails when an
ident doesn't resolve.

### What CompCert does NOT do

- **No content-keyed identity for relocations.** Two `Init_addrof
  foo 0` entries are not interned; they're separate `init_data`
  list nodes.
- **No stable cross-build encoding.** `ident` is a local positive;
  recompiling assigns fresh positives. CompCert has no analogue of
  stage-4's planned 4-tuple `(kind_rank, def_path_hash,
  canonical_generic_args, fn_shim_disambig)`.
- **No relocation-arithmetic.** Unlike LLVM's `ConstantExpr`,
  CompCert's relocation is a single ident + offset pair.
  Operations like "address of foo plus address of bar" are not
  expressible as a constant -- they would have to be computed at
  run time.

### Stage-4 implication

CompCert provides **no precedent** for stage-4's cross-build stable
relocation encoding. The 4-tuple plan in spec.md §9.7 remains novel
relative to CompCert. CompCert + charon + LLVM + MLIR all confirmed-
negative for cross-build determinism in the const universe; the only
positive precedent remains rustc's `HashStable for AllocId` (cited
in the sibling rustc.md).

CompCert *does* validate the **lazy ident-keyed shape** as a
working precedent for relocations within a single build, with
resolution deferred to a link-time phase. Stage 4's spec already
takes this position (the bridge resolves `InstanceId` references at
construction; cross-build stability is a separate canonical-key
concern).

## 5. Null pointer typing

CompCert has **no typed null pointer**. The null pointer is an
architecture-dependent integer value:

`common/Values.v:49-50`:

```coq
Definition Vnullptr :=
  if Archi.ptr64 then Vlong Int64.zero else Vint Int.zero.
```

On 64-bit targets, null is `Vlong 0`; on 32-bit, `Vint 0`. **Never
`Vptr`** -- `Vptr` requires a memory `block`, and "no block" is
expressed by encoding the pointer-width zero as a plain integer
value.

The same encoding shows up in `Vptrofs`:

`Values.v:52-53`:

```coq
Definition Vptrofs (n: ptrofs) :=
  if Archi.ptr64 then Vlong (Ptrofs.to_int64 n) else Vint (Ptrofs.to_int n).
```

Pointer offsets / pointer-as-integer values are also `Vint/Vlong`,
not `Vptr`. The two value worlds (integers and pointers) overlap at
the zero / arithmetic-only points.

### Why this works for CompCert

C semantics treats null as integer-cast-to-pointer. CompCert's
`has_type` predicate accepts `Vint` / `Vlong` as `Tptr` exactly when
the architecture's pointer width matches (`Values.v:82-83`). The
typing context determines pointer-vs-integer interpretation; the
value itself doesn't carry that distinction.

Two null pointers of different declared types (e.g., `int *` and
`char *`) reduce to the same `val` (`Vint 0` on 32-bit). They are
**equal** at the value level. The C-side typing system is the only
thing that distinguishes them.

### Stage-4 implication

This is a **divergence point** for stage 4. CompCert's "untyped null"
strategy fails the stage-4 vacant-vtable-slot canonicalisation
requirement: spec.md §13.2 pins the canonical vacant-slot type as
`Ty::FnPtr(canonical_vacant_sig_id)`, and `Ty::Dynamic` for `dyn
Trait<u32>` vs `dyn Trait<u64>` produces vtables whose vacant slots
share a `null` value but differ in slot signature. If the bridge
canonicalised "all nulls are `Vint 0`", the vtable canonicalisation
would collapse different-width vacant slots and break vtable
identity.

The stage-4 plan `Const::NullPtr(TyId)` (one variant per pointee
type) preserves the distinction and matches LLVM's
`ConstantPointerNull::get(PointerType*)` rather than CompCert's
plain integer encoding. The decision is forced by stage-4-specific
consumer pressure (vtable canonicalisation) rather than by general
elegance arguments.

That said, CompCert's evidence is useful as a *negative result*: an
untyped-null design *can* work for a C compiler, just not for one
where post-mono vtable canonicalisation is a hard requirement.

## 6. Uninit / padding

CompCert has **two distinct treatments** of uninit, depending on
universe:

### At the `val` level: first-class `Vundef`

`Vundef` is a `val` variant (Values.v:35). The docstring says it
denotes "an arbitrary bit pattern, such as the value of an
uninitialized variable" (Values.v:30-31).

`Vundef` is *truly* first-class -- many operations have explicit
`Vundef` cases (e.g., `Val.has_type Vundef _ => True` in Values.v:77,
i.e. `Vundef` has every type because nothing is known about it).
Stack-allocated locals that haven't been written are `Vundef`
(Cminor.v:198: `set_params` of unprovided arguments yields `Vundef`).

Equivalent role to LLVM's `undef`. **No analogue of LLVM's
`poison`** -- CompCert has only the one form.

### At the `init_data` level: lossy zero-rendering

`Init_space n` (AST.v:285) is the only uninit variant. It declares
"`n` bytes of uninitialised space."

But: `bytes_of_init_data` (Globalenvs.v:910-924) renders `Init_space`
as **literal zeros**:

```coq
| Init_space n => List.repeat (Byte Byte.zero) (Z.to_nat n)
```

And `store_init_data` for `Init_space` is a no-op
(Globalenvs.v:630: `| Init_space n => Some m`). The mechanism is:
`alloc_global` first calls `store_zeros m1 b 0 sz` (Globalenvs.v:658)
to zero the entire allocation, then runs `store_init_data_list`
which leaves the `Init_space` regions untouched (already zero).

So at the byte level, **`Init_space` is indistinguishable from an
all-zero `Init_int*` sequence**. The lossy collapse is built into
the language: globals are zero-initialized.

The only place actual `Undef` memvals appear is the
`bytes_of_init_data` failure path for unresolved `Init_addrof`
(Globalenvs.v:922), which is an error case rather than a designed-in
uninit shape.

### Stage-4 implication

CompCert + mrustc both take the **lossy uninit-as-zero path** at the
static-init level. The lossless path is staked out by **charon
(`Byte::Uninit`)** and **rustc (`InitMask`)**.

For stage 4, the question is whether the lossless path is
*required*. spec.md §9.7 lists "padding/uninitialised bytes if they
appear in constants (canonicalisation must distinguish `uninit` from
any specific byte pattern)" as a stage-4 requirement. The CompCert
evidence shows this requirement is *not* universal -- many real
compilers ship without it.

Justification candidates for the stage-4 lossless path:
- **Niche optimisation distinguishability.** Spec.md §11 D11/D12
  preserves rustc's niche encoding via `TagEncoding::Niche`; if a
  niche-bearing aggregate has `Uninit` in a payload-byte position
  vs `0` in the same position, those are semantically distinct
  values that a downstream codegen must distinguish. (Hypothesis;
  not directly cited in spec.md but consistent with the
  niche-preservation principle.)
- **Bit-for-bit certificate.** If RegseqModel is the bridge's
  attestation that a given const-eval'd value has a specific byte
  pattern, lossy rendering would mean two distinct rustc-side
  values map to the same `Const`, breaking the certificate.
- **F4-2 fingerprint determinism.** If `ConstId` content uniqueness
  must reflect the full canonical const-eval result, lossy
  rendering loses information that influences `ArgsKey` for nominal
  type identity. Two builds whose const-eval results agree on
  uninit positions could otherwise hash to the same `ConstId` while
  having different bytes -- a violation of the build-equivalence
  quad.

If none of those bind, the CompCert/mrustc lossy-zero path is
viable. Stage 4 should make this explicit: under what concrete
fixture does `Uninit ≠ 0` matter?

## 7. Validation phase

CompCert is verified-by-construction in Coq. There is **no run-time
validation phase** in the rustc / stage-4 sense. The closest analogues
are:

### Type predicates
`Val.has_type`, `Val.has_argtype`, `Val.has_rettype`
(Values.v:75, 154, 184) are Coq propositions over `(val, type)`
pairs. Each language's small-step semantics asserts the appropriate
predicate at every transition; consistency is proven once, in Coq.

### Init-data validity theorems

`init_mem_inversion` (Globalenvs.v:1524-1544) proves the relocation
validity theorem: every reachable `Init_addrof` resolves. This is
the per-program version of stage-4's "every `InstanceId` reference
in a `Const` resolves" invariant. Theorem-proven, not run-time-
checked.

### Linking constraints

`Linking.v` (not read in detail) handles separate-compilation
linking constraints; consistency theorems prove that linking two
verified compilation units produces a verified result. Linking
acts as a phase boundary similar to stage-4's Phase B validation,
but the boundary is between programs (not between IR sub-tables
within a single program).

### What CompCert does NOT have

- **No interner.** Types are inductive Coq types; constants are
  inductive Coq types. No content-keyed deduplication; no
  identity-key derivation. Two structurally-equal constants are
  *the same Coq term* under definitional equality, but there is no
  shared `ConstId`.
- **No phase split** type-level vs value-level. The dependency
  cycle stage-4 must resolve (constants reach types via
  `GenericArg::Const`, types reach constants via aggregate field
  types) doesn't arise: there's no cross-table reference because
  there's no table.

### Stage-4 implication

**CompCert provides no precedent for stage-4's two-phase validation
discipline.** Combined with charon (no const interner, sidesteps the
question), MLIR (per-context uniquer, no phase split), LLVM (no
phase split, types-before-constants implicit by construction order),
and mrustc (no const interner), the consistent finding is:

> **No prior-art project validates the necessity of a two-phase
> const-validation discipline. Stage 4's split rests on F4-2
> fingerprint determinism alone.**

The justification for the split has to come from RegseqModel-internal
requirements:

1. `GenericArg::Const(ConstId)` participates in nominal-type identity
   (`Ty::FnDef(item, args)`, `Ty::Closure(item, args)`,
   `Ty::Struct(item, args, layout)`). For F4-2 to hold, type identity
   must be canonicalisable before instance ordering, which means
   `ConstId`s reachable from `GenericArg::Const` must be content-
   uniqued before `TyInterner` finalises.
2. Value-level consts (statics, vtable instances, aggregates with
   relocations) need `TyId` (typed-null canonicalisation) and
   `InstanceId` (relocation encoding), so they can't be content-
   uniqued before `TyInterner` and `instances`.

These two constraints cannot both be satisfied without a phase split
*because RegseqModel chooses to intern at all*. CompCert's design
shows the alternative: don't intern, work with structural equality.
The cost of that alternative (no F4-2 cross-build alignment, no
content-keyed dedup, no `ConstId`s) is too high for stage-4's
bridge-output-as-deterministic-artifact charter, but the fact that
the alternative exists is worth recording.

## 8. Cross-build determinism

**CompCert does not aim for cross-build determinism.** `ident :=
positive` (AST.v:31) is assigned at parse time; recompiling assigns
fresh positives. There is no analogue of rustc's `DefPathHash` or
`StableHash`.

CompCert *does* support separate compilation with linking
(`Linking.v`), but linking preserves identifier identity within a
session, not across sessions. Two recompilations of the same source
produce two independently-numbered `program`s.

Within a single run, CompCert's lack of structural sharing means
"deterministic" reduces to "deterministic source → deterministic
parser positives → deterministic theorem proofs." That's
deterministic enough for verified-compilation, but not for the
stage-4 use case (snapshot equivalence across builds).

### Stage-4 implication

**Negative result, confirming the prior-art picture.** The four
non-rustc projects examined (charon, LLVM, MLIR, mrustc, CompCert =
five if we count) all confirm-negative for cross-build deterministic
const hashing. Only rustc's `HashStable for AllocId` (sibling
`rustc.md`, finding 8) provides positive prior art, and that
implementation hashes the *target* of the AllocId (the `GlobalAlloc`
content), not the AllocId integer itself.

Stage 4's planned 4-tuple `(kind_rank, def_path_hash, canonical_args,
shim_disambig)` is **operationally equivalent to rustc's
HashStable-for-AllocId pattern** (both resolve the local handle to
its target identity and hash the target). It is **novel relative to
all five non-rustc projects examined.**

## Stage-4 applicability summary

CompCert is the **minimum-viable confirmation tier**. Mapping CompCert's
choices onto stage-4's D-4-1 candidate variant set:

| Stage-4 variant (Option A plan)    | CompCert position            | Verdict for stage 4                                                            |
| ---------------------------------- | ---------------------------- | ------------------------------------------------------------------------------ |
| `Bool(bool)`                       | Subset of `Vint` (0 or 1)    | Stage-4 inline-tag form is finer-grained than CompCert's `val` but matches `init_data` flavour. |
| `Char(char)`                       | Subset of `Vint`             | Stage-4 inline form is finer-grained; necessary for typed canonical equality.  |
| `Int(IntKind, u128)`               | `Vint` (32-bit) / `Vlong` (64-bit) | Stage-4's I8/I16/I32/I64/I128/Isize is finer-grained than CompCert's two widths. CompCert pads everything below `Vlong` into `Vint`; stage 4 distinguishes for nominal-type-identity reasons. |
| `Uint(UintKind, u128)`             | Same as `Vint`/`Vlong`       | Same.                                                                          |
| `Float(FloatKind, u128)`           | `Vfloat` (binary64) / `Vsingle` (binary32) | CompCert has only F32/F64; **does not cover F16 or F128**. Stage 4 needs all four (per spec.md §10). CompCert is NOT a confirmation for the wider set; if stage 4 needs F16/F128, the precedent has to come from elsewhere (likely rustc's `Float(FloatTy::F128)` directly). |
| `ZST`                              | No analogue                  | CompCert has no ZST treatment at the const level. Stage-4 ZST canonicalisation is novel relative to CompCert. |
| `Bytes(Vec<u8>)`                   | Spread across `Init_int8` list | CompCert's flat-init-data form is the prior art; stage-4's `Bytes` packs them. Both work. |
| `Aggregate(Vec<ConstId>)`          | **No value-level analogue**  | CompCert flatly contradicts -- aggregate constants are static-init-only. Stage 4 should explicitly justify why value-level aggregates are required (see Open question 1). |
| `Enum { variant_idx, payload }`    | No analogue                  | CompCert has no enum-discriminant constants. Novel for stage 4.                |
| `Reloc(InstanceRef)`               | `Init_addrof(ident, ofs)` / `Oaddrsymbol(ident, ofs)` | CompCert validates the lazy-ident-keyed shape but not the cross-build stable encoding. Stage 4's 4-tuple is novel.|
| `NullPtr(TyId)`                    | `Vnullptr` (untyped)         | CompCert's untyped form **fails** stage-4's vacant-vtable-slot canonicalisation requirement. Diverge from CompCert. |
| `Uninit { size }`                  | `Init_space n` (lossy zero) at init level; `Vundef` at val level | CompCert/mrustc take the lossy path. Stage 4's lossless path needs a concrete justification (see Open question 3 in §6 above). |

### Big-picture take

CompCert is a **calibration point**, not a design source. It shows
how *little* a const vocabulary needs if the design accepts the
trade-offs CompCert accepts:

- accept that aggregate constants live only at static-init level;
- accept lossy zero-rendering for uninit;
- accept untyped null pointers (relying on context for typing);
- accept theorem-level validity rather than runtime invariants;
- forgo cross-build deterministic identity.

Each accept above is a place where stage-4-specific consumer pressure
(vtable canonicalisation, F4-2 fingerprint determinism, niche-
preservation, the bridge-as-attestation model) pushes us beyond the
CompCert minimum. The pushes are *individually* well-justified by
spec.md §9.7 and §13.2; CompCert's evidence usefully sharpens the
question of which pushes are absolutely required vs which are
speculative scope expansion.

## Open questions for D-4-1

1. **Aggregate constants at value level vs static-init only.**
   CompCert + mrustc both push aggregates to static-init lists;
   charon + LLVM + stage-4-plan keep them first-class. What v1
   fixture strictly requires value-level aggregate constants? If
   the answer is "vtable instances only" (per spec.md §13.2),
   could stage 4 limit `Aggregate` to vtable-instance synthesis
   sites and not expose it as a general-purpose `Const` variant?
   This would simplify the canonicaliser at the cost of adding a
   special bridge-synthesis path.

2. **Float-kind support beyond F32/F64.** Spec.md §10's `Float`
   covers F16/F32/F64/F128. CompCert covers only F32/F64. If F16 /
   F128 const-eval doesn't appear in v1 driver fixtures, stage 4
   could match the CompCert minimum here and route the wider set
   to a "future" sub-decision. The trade-off is whether
   future-proofing is cheap (just two more variants) or expensive
   (forces wider integer payload from `u64` to `u128`).

3. **Untyped null vs typed null.** CompCert collapses null to
   `Vint 0` / `Vlong 0`; stage 4's plan keeps `NullPtr(TyId)`. The
   pressure for typed-null is the vacant-vtable-slot
   canonicalisation requirement. If vacant-slot-typing is moved
   from `Const` to `FnSig` (the slot's `FnPtr` type carries the
   signature; the const value is just a single null marker
   independent of pointee), the CompCert minimum applies and
   stage 4 could collapse all nulls to `Const::NullScalar`. This
   is structurally cleaner but moves bookkeeping into the
   per-vtable layout / `FnSig` interner. Worth weighing.

4. **Two-phase validation justification (recap from §7).** No
   prior-art project validates the necessity of a two-phase
   discipline. Stage 4's split must rest on the F4-2 +
   `GenericArg::Const` reachability argument alone. If the value-
   level subset of consts can be canonicalised *without* `TyId` /
   `InstanceId` references (e.g., by using the stable 4-tuple for
   relocations + structural keys for aggregates), the cycle
   collapses and a single-phase discipline suffices. Worth
   examining whether the `TyId` reference in `NullPtr(TyId)` and
   the `InstanceRef` in relocations actually create a dependency
   on `TyInterner` / `instances` finalisation, or whether the
   identity tuples can be derived from rustc state without
   needing the local IDs to be assigned.

5. **Demote-to-Static as alternative aggregate strategy.**
   mrustc eagerly promotes nested aggregate consts to fresh
   synthetic statics (per the sibling `mrustc.md` finding 3).
   CompCert's static-init-only design is structurally similar:
   any aggregate const becomes a global with an init_data list.
   Stage 4 could adopt this pattern: a const expression with an
   aggregate value gets bridge-synthesized as a `Static` Item,
   and the original constexpr position holds a relocation to that
   static. This would allow `Const::Aggregate` to be dropped
   entirely from the variant set; the cost is one synthetic Item
   per aggregate constant. Worth weighing against the
   interner-complexity savings.

Stop.