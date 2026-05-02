# CompCert -- Stage-3 Prior-Art Notes

Primary-source research for the RegseqModel `Ty` enum, sourced from
the CompCert Coq implementation on GitHub (AbsInt/CompCert, master
branch). This file deliberately reads CompCert through the lens of the
stage-3 themes A-G; it does not propose RegseqModel design choices.

CompCert is interesting to us as the canonical example of a multi-IR
pipeline with formally-defined type semantics: its high-level
front-end (`Csyntax` / `Clight`) and low-level back-end
(`Cminor` / `RTL` / ...) deliberately use very different type
vocabularies, and the type system collapses in a single, well-defined
step (Clight -> Cminor). It is also the canonical example of "C
semantics with information preservation" because every front-end
expression node carries its resolved type.

## Sources (primary)

All raw URLs were fetched on 2026-04-28.

- `cfrontend/Ctypes.v` --
  https://raw.githubusercontent.com/AbsInt/CompCert/master/cfrontend/Ctypes.v
- `cfrontend/Csyntax.v` --
  https://raw.githubusercontent.com/AbsInt/CompCert/master/cfrontend/Csyntax.v
- `cfrontend/Clight.v` --
  https://raw.githubusercontent.com/AbsInt/CompCert/master/cfrontend/Clight.v
- `cfrontend/Cop.v` --
  https://raw.githubusercontent.com/AbsInt/CompCert/master/cfrontend/Cop.v
- `cfrontend/PrintCsyntax.ml` --
  https://raw.githubusercontent.com/AbsInt/CompCert/master/cfrontend/PrintCsyntax.ml
- `backend/Cminor.v` --
  https://raw.githubusercontent.com/AbsInt/CompCert/master/backend/Cminor.v
- `backend/RTL.v` --
  https://raw.githubusercontent.com/AbsInt/CompCert/master/backend/RTL.v
- `common/AST.v` --
  https://raw.githubusercontent.com/AbsInt/CompCert/master/common/AST.v
- `common/Values.v` --
  https://raw.githubusercontent.com/AbsInt/CompCert/master/common/Values.v
- `common/Memory.v` --
  https://raw.githubusercontent.com/AbsInt/CompCert/master/common/Memory.v
- Project entrypoint --
  https://github.com/AbsInt/CompCert
- CompCert C manual (ISO C subset notes, `_Alignas`, volatile) --
  https://compcert.org/man/manual.pdf
- Backend paper, Leroy, JAR 2009 --
  https://xavierleroy.org/publi/compcert-backend.pdf

## A. Type-vocabulary completeness (per IR level)

CompCert is multi-tiered. There are effectively two distinct type
vocabularies:

1. **Front-end C types** -- `Ctypes.type`, used by both `Csyntax`
   (high-level) and `Clight` (lower-level pure-expression C). This is
   the rich, ISO-C-shaped vocabulary.
2. **Back-end machine types** -- `AST.typ` (a 6-constructor
   register-class tag) plus `AST.xtype` (a finer 13-constructor flavor
   used in function `signature`s) plus `AST.memory_chunk` (the
   load/store width tag). Used by `Cminor` and every IR after it.

### A.1 Front-end -- `Ctypes.type`

Single inductive, file `cfrontend/Ctypes.v`:

```coq
Inductive type : Type :=
  | Tvoid: type
  | Tint: intsize -> signedness -> attr -> type
  | Tlong: signedness -> attr -> type
  | Tfloat: floatsize -> attr -> type
  | Tpointer: type -> attr -> type
  | Tarray: type -> Z -> attr -> type
  | Tfunction: list type -> type -> calling_convention -> type
  | Tstruct: ident -> attr -> type
  | Tunion: ident -> attr -> type.
```

Supporting inductives:

```coq
Inductive signedness : Type := Signed | Unsigned.

Inductive intsize : Type := I8 | I16 | I32 | IBool.

Inductive floatsize : Type := F32 | F64.

Inductive struct_or_union : Type := Struct | Union.

Inductive member : Type :=
  | Member_plain (id: ident) (t: type)
  | Member_bitfield (id: ident) (sz: intsize) (sg: signedness)
                    (a: attr) (width: Z) (padding: bool).

Definition members : Type := list member.

Inductive composite_definition : Type :=
  Composite (id: ident) (su: struct_or_union) (m: members) (a: attr).
```

Attribute record (the only "modifier" channel):

```coq
Record attr : Type := mk_attr {
  attr_volatile: bool;
  attr_alignas: option N
}.
```

Notable absences from `Ctypes.type`:

- No 128-bit integer (no `__int128`).
- No `_Complex` types.
- No `_Atomic` qualifier (see theme D below).
- Aggregate types `Tstruct` / `Tunion` are referenced **by ident only**;
  the body is held in a separate `composite_env` (see theme C).
- Function types do **not** carry an `attr`; only the arrow-shape carries
  a `calling_convention`. Per `attr_of_type`, `Tfunction` returns
  `noattr`.

### A.2 Back-end -- `AST.typ` and friends

`common/AST.v`:

```coq
Inductive typ : Type :=
  | Tint                (**r 32-bit integers or pointers *)
  | Tfloat              (**r 64-bit double-precision floats *)
  | Tlong               (**r 64-bit integers *)
  | Tsingle             (**r 32-bit single-precision floats *)
  | Tany32              (**r any 32-bit value *)
  | Tany64.             (**r any 64-bit value, i.e. any value *)
```

Six constructors total. Signedness, struct shape, and array shape
are gone. Pointers are erased into `Tint` (32-bit target) or `Tlong`
(64-bit target); the discriminant lives in `Archi.ptr64` rather than
in the type itself.

Finer-grained extension used in function signatures only:

```coq
Inductive xtype : Type :=
  | Xbool               (**r Boolean value (0 or 1) *)
  | Xint8signed         (**r 8-bit signed integer *)
  | Xint8unsigned       (**r 8-bit unsigned integer *)
  | Xint16signed        (**r 16-bit signed integer *)
  | Xint16unsigned      (**r 16-bit unsigned integer *)
  | Xint                (**r 32-bit integers or pointers *)
  | Xfloat              (**r 64-bit double-precision floats *)
  | Xlong               (**r 64-bit integers *)
  | Xsingle             (**r 32-bit single-precision floats *)
  | Xptr                (**r pointers and pointer-sized integers *)
  | Xany32              (**r any 32-bit value *)
  | Xany64              (**r any 64-bit value, i.e. any value *)
  | Xvoid.              (**r no meaningful value *)
```

`xtype` is used in `signature`'s `sig_args` and `sig_res` fields. It
records sub-32-bit precision and pointer-vs-int distinction at the
ABI boundary, but the body of the function works in `typ`.

Companion projections:

```coq
Definition inj_type (t: typ) : xtype := ...    (* typ -> xtype lift *)
Definition proj_xtype (x: xtype) : typ := ...  (* xtype -> typ collapse *)
```

The semantic value domain (`common/Values.v`):

```coq
Inductive val: Type :=
  | Vundef: val
  | Vint: int -> val
  | Vlong: int64 -> val
  | Vfloat: float -> val
  | Vsingle: float32 -> val
  | Vptr: block -> ptrofs -> val.
```

Linked back to `typ` via `Val.has_type`:

```coq
Definition has_type (v: val) (t: typ) : Prop :=
  match v, t with
  | Vundef, _ => True
  | Vint _, Tint => True
  | Vlong _, Tlong => True
  | Vfloat _, Tfloat => True
  | Vsingle _, Tsingle => True
  | Vptr _ _, Tint => Archi.ptr64 = false
  | Vptr _ _, Tlong => Archi.ptr64 = true
  | _, _ => False
  end.
```

### A.3 The third tag: `memory_chunk`

`common/AST.v` also defines the load/store width tag:

```coq
Inductive memory_chunk : Type :=
  | Mbool           (**r 8-bit integer containing 0 or 1 *)
  | Mint8signed     (**r 8-bit signed integer *)
  | Mint8unsigned   (**r 8-bit unsigned integer *)
  | Mint16signed    (**r 16-bit signed integer *)
  | Mint16unsigned  (**r 16-bit unsigned integer *)
  | Mint32          (**r 32-bit integer, or pointer *)
  | Mint64          (**r 64-bit integer *)
  | Mfloat32        (**r 32-bit single-precision float *)
  | Mfloat64        (**r 64-bit double-precision float *)
  | Many32          (**r any value that fits in 32 bits *)
  | Many64.         (**r any value *)
```

`memory_chunk` is the operational answer to "how do I read this from
memory?" Per `Memory.v`, `load` and `store` are parameterized by a
`memory_chunk`, not a `typ`. So `typ` describes register class,
`memory_chunk` describes load-store width and signedness, and `xtype`
describes ABI precision; they are deliberately three separate
vocabularies even though they overlap.

## B. Layout / representation

CompCert performs layout once, in the front-end, and stores the
results. It does not pass alignment annotations through subsequent
passes.

### B.1 Where layout lives -- `composite`

```coq
Record composite : Type := {
  co_su: struct_or_union;
  co_members: members;
  co_attr: attr;
  co_sizeof: Z;
  co_alignof: Z;
  co_rank: nat;
  co_sizeof_pos: co_sizeof >= 0;
  co_alignof_two_p: exists n, co_alignof = two_power_nat n;
  co_sizeof_alignof: (co_alignof | co_sizeof)
}.

Definition composite_env : Type := PTree.t composite.
```

Every `Tstruct id _` / `Tunion id _` is a name-only handle; layout
results (`co_sizeof`, `co_alignof`) are looked up via
`composite_env` (a `PTree`, i.e. a positive-keyed trie). The record
also embeds proof obligations: sizeof must be non-negative, alignof
must be a power of two, and alignof divides sizeof. Composites are
thus carrying their own well-formedness witnesses.

Each `program` carries the env explicitly:

```coq
Record program : Type := {
  prog_defs: list (ident * globdef fundef type);
  prog_public: list ident;
  prog_main: ident;
  prog_types: list composite_definition;
  prog_comp_env: composite_env;
  prog_comp_env_eq: build_composite_env prog_types = OK prog_comp_env
}.
```

The proof field `prog_comp_env_eq` ties the env to the source list
of `composite_definition`s.

### B.2 Sizeof / alignof are env-parameterized

```coq
Fixpoint sizeof (env: composite_env) (t: type) : Z :=
  match t with
  | Tvoid => 1
  | Tint I8 _ _ => 1
  | Tint I16 _ _ => 2
  | Tint I32 _ _ => 4
  | Tint IBool _ _ => 1
  | Tlong _ _ => 8
  | Tfloat F32 _ => 4
  | Tfloat F64 _ => 8
  | Tpointer _ _ => if Archi.ptr64 then 8 else 4
  | Tarray t' n _ => sizeof env t' * Z.max 0 n
  | Tfunction _ _ _ => 1
  | Tstruct id _ | Tunion id _ =>
      match env!id with Some co => co_sizeof co | None => 0 end
  end.

Fixpoint alignof (env: composite_env) (t: type) : Z :=
  align_attr (attr_of_type t)
   (match t with
      | Tvoid => 1
      | Tint I8 _ _ => 1
      | ...
      | Tpointer _ _ => if Archi.ptr64 then 8 else 4
      | Tarray t' _ _ => alignof env t'
      | Tfunction _ _ _ => 1
      | Tstruct id _ | Tunion id _ =>
          match env!id with Some co => co_alignof co | None => 1 end
    end).
```

Pointer width and `int64` alignment are target-parameterized via
`Archi.ptr64` and `Archi.align_int64`; `Float64` alignment goes through
`Archi.align_float64`. Target-dependence is bundled in a single `Archi`
module rather than spread across the type vocabulary.

### B.3 `_Alignas` is one optional N (log2)

```coq
Definition align_attr (a: attr) (al: Z) : Z :=
  match attr_alignas a with
  | Some l => two_p (Z.of_N l)
  | None => al
  end.
```

`attr_alignas` is `option N`, where `N` is the log2 of the requested
alignment. So `_Alignas(16)` is stored as `Some 4`. The pretty-printer
unscales it on output:

```ocaml
let attributes a =
  let s1 = if a.attr_volatile then " volatile" else "" in
  match a.attr_alignas with
  | None -> s1
  | Some l -> sprintf " _Alignas(%Ld)%s" (Int64.shift_left 1L (N.to_int l)) s1
```

### B.4 Bitfields

Bitfields are first-class in the field representation:

```coq
Inductive member : Type :=
  | Member_plain (id: ident) (t: type)
  | Member_bitfield (id: ident) (sz: intsize) (sg: signedness)
                    (a: attr) (width: Z) (padding: bool).
```

The `padding: bool` flag distinguishes named bitfields from anonymous
padding bitfields. There is also a separate access tag for use sites:

```coq
Inductive bitfield : Type :=
  | Full
  | Bits (sz: intsize) (sg: signedness) (pos: Z) (width: Z).
```

`Full` is for ordinary l-value access; `Bits` carries the storage-unit
size, sign, bit position within the storage unit, and width. These
results come out of `next_field` / `layout_field` (referenced in the
file but not quoted here).

### B.5 Access modes

```coq
Inductive mode: Type :=
  | By_value: memory_chunk -> mode
  | By_reference: mode
  | By_copy: mode
  | By_nothing: mode.

Definition access_mode (ty: type) : mode := ...
```

`access_mode` answers "given a `Ctypes.type`, how do I access an
l-value of that type at runtime?" Scalar types resolve to
`By_value Mxxx` (here is where `Ctypes` connects to `memory_chunk`);
arrays resolve to `By_reference` (decay); struct/union to `By_copy`;
`Tvoid` and `Tfunction` to `By_nothing`. This is the bridge between
the rich front-end vocabulary and the operational memory model.

## C. Identity, interning, canonical form

CompCert relies entirely on Coq's structural equality of inductives,
plus a derived decidable-equality lemma:

```coq
Lemma type_eq: forall (ty1 ty2: type), {ty1=ty2} + {ty1<>ty2}.
```

This is a `{=}+{<>}` ("sumbool") -- a constructive decision procedure
returning either a proof of equality or a proof of disequality. It is
used at compile time wherever the implementation must compare types
(e.g., assignment compatibility, cast classification).

Important consequences:

- **No interning, no hash-consing.** Two `Ctypes.type` values are
  equal iff their inductive trees match; CompCert does not maintain a
  unique-pointer / id-based discipline for types.
- **Aggregates *are* indirected, but by name not by identity.** Struct
  and union types carry an `ident` (the C source name) and consult
  `composite_env` to look up the body. This is name equality, not
  structural equality across bodies. Two structs with identical fields
  but different names are not equal.
- **Coq inductives give canonicality "for free".** Because every type
  is a closed inductive value, there is no fresh-name or generation
  counter; the same constructor with the same arguments is the same
  term up to `eq`.
- **Attributes participate in equality.** `Tint I32 Signed
  {| attr_volatile := true; ... |}` is not equal to `Tint I32 Signed
  noattr`. Functions `change_attributes`, `remove_attributes`,
  `attr_of_type` exist precisely to manipulate attributes when
  exact-equality would otherwise be too strict:

```coq
Definition attr_of_type (ty: type) := ...
Definition change_attributes (f: attr -> attr) (ty: type) : type := ...
Definition remove_attributes (ty: type) : type := change_attributes (fun _ => noattr) ty.
```

## D. Special-semantics markers (volatile, atomic)

### D.1 Volatile

`attr_volatile: bool` is the only volatile channel. There is no
separate `Tvolatile t` constructor; volatility is a per-type-node
boolean carried in every `attr`. The pretty-printer emits the
`volatile` keyword from this bit (see B.3).

The CompCert C manual states that "for the purposes of the semantics
and the compiler, a type denotes a volatile access if it carries the
volatile attribute and it is accessed by value". So volatility is
both a static type-level attribute and a dynamic-semantics flag --
the same attribute determines whether a load/store goes through the
volatile-trace observable behavior.

CompCert's observable behavior trace explicitly records volatile
operations (per the manual and the CACM 2009 paper), which is why
volatile is a load-bearing primitive rather than an annotation that
can be erased.

### D.2 `_Atomic`

`_Atomic` is **not present** in `Ctypes.attr` or `Ctypes.type` on
master at the time of this fetch. The attribute record is exactly:

```coq
Record attr : Type := mk_attr {
  attr_volatile: bool;
  attr_alignas: option N
}.
```

CompCert's documentation has historically described support for
limited atomics through builtin/intrinsic functions (`__builtin_*`)
rather than through a type-level qualifier. Per the manual, `_Atomic`
as a type qualifier is not part of the verified subset.

### D.3 `restrict`, `const`

Also not represented in `attr`. `const` is a C-level qualifier without
runtime semantic effect in CompCert and is dropped at parse time;
`restrict` similarly does not survive into `Ctypes.type`.

## E. Post-monomorphization erasure

N/A. CompCert consumes C, which has no parametric polymorphism; there
is no monomorphization step, and therefore no erasure phase analogous
to Rust's MIR -> post-mono lowering. We skip this theme with this
note.

## F. Bridge / consumption shape (where the type vocabulary collapses)

CompCert has a sharp, identifiable point where types collapse: the
**Clight -> Cminor** transition (compiler pass `Cshmgen` then
`Cminorgen`).

### F.1 Csyntax (high-level)

`cfrontend/Csyntax.v`. Every expression node carries a `ty: type`.
For example:

```coq
Inductive expr : Type :=
  | Eval (v: val) (ty: type)
  | Evar (x: ident) (ty: type)
  | Efield (l: expr) (f: ident) (ty: type)
  | Evalof (l: expr) (ty: type)
  | Ederef (r: expr) (ty: type)
  | Eaddrof (l: expr) (ty: type)
  | Eunop (op: unary_operation) (r: expr) (ty: type)
  | Ebinop (op: binary_operation) (r1 r2: expr) (ty: type)
  | Ecast (r: expr) (ty: type)
  | Eseqand (r1 r2: expr) (ty: type)
  | Eseqor (r1 r2: expr) (ty: type)
  | Econdition (r1 r2 r3: expr) (ty: type)
  | Esizeof (ty': type) (ty: type)
  | Ealignof (ty': type) (ty: type)
  | Eassign (l: expr) (r: expr) (ty: type)
  | Eassignop (op: binary_operation) (l r: expr) (tyres ty: type)
  | Epostincr (id: incr_or_decr) (l: expr) (ty: type)
  | Ecomma (r1 r2: expr) (ty: type)
  | Ecall (r1: expr) (rargs: exprlist) (ty: type)
  | Ebuiltin (ef: external_function) (tyargs: list type)
              (rargs: exprlist) (ty: type)
  | Eloc (b: block) (ofs: ptrofs) (bf: bitfield) (ty: type)
  | Eparen (r: expr) (tycast: type) (ty: type).
```

Csyntax is "C with side-effecting expressions": assignment, increment,
function call, etc. are expressions, not statements. Statements:

```coq
Inductive statement : Type :=
  | Sskip | Sdo : expr -> statement
  | Ssequence : statement -> statement -> statement
  | Sifthenelse : expr -> statement -> statement -> statement
  | Swhile | Sdowhile | Sfor | Sbreak | Scontinue
  | Sreturn : option expr -> statement
  | Sswitch : expr -> labeled_statements -> statement
  | Slabel : label -> statement -> statement
  | Sgoto : label -> statement.

Record function : Type := mkfunction {
  fn_return: type;
  fn_callconv: calling_convention;
  fn_params: list (ident * type);
  fn_vars: list (ident * type);
  fn_body: statement
}.
```

### F.2 Clight (lower C, pure expressions)

`cfrontend/Clight.v`: "a simplified version of Compcert C where all
expressions are pure and assignments and function calls are
statements, not expressions." Same `Ctypes.type` vocabulary; same
"type on every node" rule:

```coq
Inductive expr : Type :=
  | Econst_int: int -> type -> expr
  | Econst_float: float -> type -> expr
  | Econst_single: float32 -> type -> expr
  | Econst_long: int64 -> type -> expr
  | Evar: ident -> type -> expr
  | Etempvar: ident -> type -> expr
  | Ederef: expr -> type -> expr
  | Eaddrof: expr -> type -> expr
  | Eunop: unary_operation -> expr -> type -> expr
  | Ebinop: binary_operation -> expr -> expr -> type -> expr
  | Ecast: expr -> type -> expr
  | Efield: expr -> ident -> type -> expr
  | Esizeof: type -> type -> expr
  | Ealignof: type -> type -> expr.
```

Statements gain assignment-as-statement (`Sassign`, `Sset`) and
explicit single-assignment loop forms:

```coq
Inductive statement : Type :=
  | Sskip
  | Sassign : expr -> expr -> statement
  | Sset : ident -> expr -> statement
  | Scall: option ident -> expr -> list expr -> statement
  | Sbuiltin: option ident -> external_function -> list type -> list expr -> statement
  | Ssequence | Sifthenelse | Sloop
  | Sbreak | Scontinue | Sreturn : option expr -> statement
  | Sswitch | Slabel | Sgoto.

Record function : Type := mkfunction {
  fn_return: type;
  fn_callconv: calling_convention;
  fn_params: list (ident * type);
  fn_vars: list (ident * type);
  fn_temps: list (ident * type);  (* new: SSA-ish temporaries *)
  fn_body: statement
}.
```

So Csyntax -> Clight does **not** drop type info; it normalizes the
expression form (pure expressions, side effects in statements) and
introduces temporaries `fn_temps`.

The classification machinery in `cfrontend/Cop.v` -- `classify_cast`,
`classify_binarith`, `classify_shift`, `classify_cmp`, etc. -- is what
consumes the per-node `ty` annotation. Front-end operators are
overloaded; the resolved meaning is computed by classifying the
operand types. Without per-node types this resolution would be
impossible; the design rule is that the front-end *preserves enough
information for classification to be local*.

### F.3 Cminor (the type collapse point)

`backend/Cminor.v`. The constructors no longer carry C types:

```coq
Inductive expr : Type :=
  | Evar : ident -> expr
  | Econst : constant -> expr
  | Eunop : unary_operation -> expr -> expr
  | Ebinop : binary_operation -> expr -> expr -> expr
  | Eload : memory_chunk -> expr -> expr.

Inductive stmt : Type :=
  | Sskip
  | Sassign : ident -> expr -> stmt
  | Sstore : memory_chunk -> expr -> expr -> stmt
  | Scall : option ident -> signature -> expr -> list expr -> stmt
  | Stailcall: signature -> expr -> list expr -> stmt
  | Sbuiltin : option ident -> external_function -> list expr -> stmt
  | Sseq | Sifthenelse | Sloop | Sblock | Sexit
  | Sswitch: bool -> expr -> list (Z * nat) -> nat -> stmt
  | Sreturn | Slabel | Sgoto.

Record function : Type := mkfunction {
  fn_sig: signature;
  fn_params: list ident;
  fn_vars: list ident;
  fn_stackspace: Z;
  fn_body: stmt
}.
```

Observations:

- Operators have been **monomorphized into their typed variants**.
  The Cminor `unary_operation` and `binary_operation` inductives are
  much wider than the front-end's: `Oadd / Osub / Omul / Odiv / Omod`
  for 32-bit signed, `Oaddl / Osubl / ... / Omodlu` for 64-bit,
  `Oaddf / Osubf / ...` for double, `Oaddfs / Osubfs / ...` for
  single, `Ocast8signed / Ocast8unsigned / Ocast16*` for narrowing
  casts, etc. The "what type was this" question is encoded in the
  *operator name* rather than in a per-node tag.
- Memory operations are tagged with `memory_chunk`. `Eload` and
  `Sstore` carry their access width directly.
- Function calls retain a `signature` (so ABI types survive). Locals
  and parameters do not.
- Pointer types are gone -- they are `Tint` or `Tlong` depending on
  `Archi.ptr64`, and even that distinction is implicit; addresses
  appear as `Vptr` values at runtime, statically untagged.

So Clight -> Cminor is the **collapse boundary**. The collapse is
not lossy in a problematic sense: all type information needed for
correctness is *baked into operator and chunk choices* before the
collapse, and the function-level `signature` retains ABI-shape info
in `xtype`.

### F.4 RTL (and beyond)

`backend/RTL.v`: "the first intermediate language after Cminor and
CminorSel."

```coq
Inductive instruction: Type :=
  | Inop: node -> instruction
  | Iop: operation -> list reg -> reg -> node -> instruction
  | Iload: memory_chunk -> addressing -> list reg -> reg -> node -> instruction
  | Istore: memory_chunk -> addressing -> list reg -> reg -> node -> instruction
  | Icall: signature -> reg + ident -> list reg -> reg -> node -> instruction
  | Itailcall: signature -> reg + ident -> list reg -> instruction
  | Ibuiltin: external_function -> list (builtin_arg reg) -> builtin_res reg -> node -> instruction
  | Icond: condition -> list reg -> node -> node -> instruction
  | Ijumptable: reg -> list node -> instruction
  | Ireturn: option reg -> instruction.

Record function: Type := mkfunction {
  fn_sig: signature;
  fn_params: list reg;
  fn_stacksize: Z;
  fn_code: code;
  fn_entrypoint: node
}.
```

Same pattern: `signature` at the call boundary, `memory_chunk` at
each load/store, no per-register type annotation. Pseudo-registers
hold `val` values; their typing is inferred globally from uses (a
separate analysis, `RTLtyping`, can recover register types but they
are not part of the IR).

### F.5 Summary of the bridge

| Stage         | Type vocabulary                | Per-node types | Carrier of "narrowness" |
|---------------|--------------------------------|----------------|-------------------------|
| Csyntax       | `Ctypes.type` (rich C types)   | yes            | `intsize` / `signedness` / `attr` |
| Clight        | `Ctypes.type`                  | yes            | same                    |
| Cminor        | `AST.typ` (in `signature`s)    | no             | operator name + `memory_chunk` |
| CminorSel     | `AST.typ` (in `signature`s)    | no             | machine-specific operators |
| RTL           | `AST.typ` (in `signature`s)    | no             | `memory_chunk` + `signature` + `condition`/`operation` |
| LTL/Linear/Mach/Asm | `AST.typ`, `xtype`        | no             | machine ops             |

The collapse happens once; before the collapse, types are
omnipresent; after the collapse, they live only in `signature`s and
in the names of operators / chunks.

## G. Renderer / pretty-printer format

CompCert ships per-IR pretty-printers in OCaml (the verified Coq core
calls into them through extraction). Relevant files include
`PrintCsyntax.ml` (Csyntax), `PrintClight.ml`, `PrintCminor.ml`,
`PrintRTL.ml`, etc. The `ccomp` driver exposes them through `-dparse`,
`-dc`, `-dclight`, `-dcminor`, `-dcminorsel`, `-drtl`, `-dltl`,
`-dmach`, `-dasm` switches.

Selected from `cfrontend/PrintCsyntax.ml`:

```ocaml
let name_type ty = name_cdecl "" ty
let name_optid id = if id = "" then "" else " " ^ id

let attributes a =
  let s1 = if a.attr_volatile then " volatile" else "" in
  match a.attr_alignas with
  | None -> s1
  | Some l -> sprintf " _Alignas(%Ld)%s" (Int64.shift_left 1L (N.to_int l)) s1
```

`name_cdecl` is the standard "C declaration" formatter that handles
the inside-out reading rule of C (pointers, arrays, functions). It
takes a type and a (possibly empty) declarator name and emits a
syntactically valid C declaration. This is the round-trip approach:
the front-end IRs print as legal C source, and the `clightgen` tool
in particular produces a Clight-equivalent C file that can be parsed
back.

For the back-end IRs (Cminor, RTL, ...), the printers emit a
custom textual form (S-expression-ish) since those IRs have no C
analogue. Operators print as the Coq constructor names (e.g.
`addl`, `subl`, `cast8signed`); `memory_chunk` prints as `int8s`,
`int8u`, `int16s`, ..., `int32`, `int64`, `float32`, `float64`,
`any32`, `any64`, `bool`.

The renderer pattern is therefore "two regimes": front-end IRs use a
real-C printer keyed off `Ctypes.type`, back-end IRs use a structural
printer keyed off the constructor names of `AST.typ`,
`memory_chunk`, and the per-IR operation inductives.

## Watch-outs and observations

The following are observations a stage-3 reader should keep in mind;
they are not RegseqModel design recommendations.

1. **Volatile is highly relevant for our driver use case.** CompCert
   treats `volatile` as a first-class observable: it is part of the
   trace semantics, and the verified compilation pipeline preserves
   the **trace** rather than just the post-state. For driver code,
   this means CompCert *cannot* fold or reorder volatile loads/stores,
   not merely *should not*. The design choice is to encode this as a
   bit on every `attr` in `Ctypes.type`, not as a `Tvolatile` wrapper.
   This is worth contrasting with rustc, which carries `Volatile` only
   inside intrinsics (`volatile_load` / `volatile_store`) rather than
   on type nodes.

2. **`_Atomic` is not in the verified subset.** If we are sourcing
   "what counts as a full C type system" from CompCert, atomics will
   *not* appear. The CompCert manual treats atomic operations as
   builtins, not as a type qualifier. Anything we want to do with C11
   atomics in the emitted C cannot lean on CompCert's `attr` design.

3. **Three vocabularies, not one, by design.** It is tempting to
   describe CompCert as "one type system that gets simpler"; that is
   not what the source says. `Ctypes.type` (front-end), `AST.typ`
   (back-end), and `AST.memory_chunk` (loads/stores) coexist as
   peers. `xtype` adds a fourth layer at the ABI boundary. The
   discipline is "use the narrowest vocabulary that suffices for the
   pass at hand".

4. **Composite indirection is by `ident`, not by interned type id.**
   `Tstruct id _` is just a name handle. There is no notion of
   "structural identity" or "interning" for C types in CompCert; two
   structs with identical field lists but different names are
   genuinely different types. The body lives in `composite_env`,
   which is a `PTree.t composite` (positive-keyed trie), and the
   `program` record carries a proof that the env is consistent with
   the source list of `composite_definition`s.

5. **Per-node type annotation in the front-end is load-bearing.** It
   is what allows `Cop.classify_*` to operate locally and resolve
   overloaded C operators to typed primitives, without having to walk
   the whole AST. The cost is significant duplication (every
   subexpression carries a `type`); the benefit is that the
   classification analysis is a simple match. This is the canonical
   "C semantics with information preservation" pattern.

6. **Layout is computed once, then stored.** `co_sizeof` and
   `co_alignof` are part of the `composite` record; the surrounding
   `Sigma` proof obligations make the record unforgeable. Subsequent
   passes never recompute layout; they look it up. Layout decisions
   thus have a single point of authority.

7. **`Ctypes.type_eq` is decidable structural equality.** The lemma
   returns a `{=} + {<>}` sumbool. This is significantly stronger
   than rustc's `Ty<'tcx>` pointer-equality (which is interning-based
   and only works within a `tcx`); CompCert's equality is a pure
   inductive comparison and is portable across compile sessions.

8. **The collapse from Clight to Cminor is sharp and well-localized.**
   This is exactly the bridging shape that motivates the stage-3
   methodology of "store resolved results, do not pass annotations
   through". Cminor's typed operators (`Oaddl`, `Oaddf`, `Oaddfs`,
   `Ocast8unsigned`, ...) are the resolved results; the front-end
   classification is the resolution step; the per-node `type`
   annotation is the annotation that gets *consumed* before being
   *dropped*.

9. **Bitfields are first-class but only in members, not in `type`.**
   `Member_bitfield` is a constructor of `member`, not of `type`. The
   `bitfield` inductive (`Full | Bits ...`) appears at *use sites*
   (e.g. `Eloc` in `Csyntax.expr`) but never as a type. So bitfield
   widths do not propagate through the type system; they live where
   layout lives (in members) and where access happens (in l-value
   designators).

10. **Pointer width is a target parameter, not a type parameter.**
    `Archi.ptr64` decides whether `Tpointer _ _` is 4 or 8 bytes.
    There is no `Tpointer32 / Tpointer64` distinction at the type
    level. This couples cleanly to the back-end where `Vptr` is
    untagged and is interpreted via the same `Archi.ptr64`.
