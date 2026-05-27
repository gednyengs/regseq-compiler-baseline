# Phase 1 · Doc 2 — Architecture decisions

**Status: FILLED (2026-05-26).** Each decision below opens with its
**locked direction** (agreed; not re-litigated) and resolves it to a
**concrete, pinned C-lowering rule** per the architect rule "settle here,
not in implementation." Decisions that change the IR→C contract are
flagged `Spec-delta: yes` and become edits to
`../../regseq-model/generic-c-backend.md`. The gap list in Doc 1
(`01-construct-surface-and-matrix.md`) is the agenda; every `unspecced`/
`deferred` row there is resolved by a decision here.

The genuinely open forks got **Codex first-contact** before drafting; its
corrections are folded in (notably: the enum C shape is offset-faithful,
not a naive `{tag; union;}` — D9; signed shifts need unsigned-domain
lowering, not just `-fwrapv` — D7).

> **Resolve-template per decision:** *Problem* → *Options* → **Decision**
> → *Rationale / consequences* → *Spec-delta?*

**Decision map → Doc-1 gaps**

| Decision | Resolves Doc-1 gap |
| -------- | ------------------ |
| D1 Naming | IN-1; type-name derivation |
| D2 Declaration order | gap C9 (IN-3, IN-7, IN-8); gap C12 (FS-5 `c_variadic`); FS C-ABI prototypes |
| D3 File layout + harness entry | — (verification plumbing; feeds Doc 3) |
| D4 Host panic / abort | gap A2 (AK-1..8); TM-4/TM-6 trap path |
| D5 Terminator emission | gap A1 (TM-1..10) |
| D6 Diag-log interception | DL-7 (the deferred call-site half) |
| D7 Rvalue + operators | gap B4 (RV/BO/UN/AG/Repeat); ST-1 |
| D8 Casts | gap B4 (CK-*, PC-*) |
| D9 Enum C representation | gap B5 (TY-18, LY-5/7..14, IN-10, IT-5, PL-8, RV-8, ST-2, CO-9, AG-4) |
| D10 Aggregate field-offset reproduction | gap B6 (LY-3/6, IN-9); union/tuple/struct |
| D11 Const → C initializers | gap B7 (CO-1..14, IN-7/8) |
| D12 Symbol & declaration policy | gap C8 (IT-2/7/8/9), gap C10 (IN-5/6) |

---

## D1 — Naming (identifiers for emitted C)

**Locked direction.** Lean on `Instance.emission_name` for
functions/statics/consts; interned `FieldLayout.name` for fields; derive
**type** names from `DefPath`. Deterministic across the
build-equivalence quad (golden-output tests).

**Problem.** Every emitted C entity needs a stable, collision-free C
identifier; type names are not pre-minted (only instance `emission_name`
is).

**Decision.**
1. **Instances** (fn / static / const): use `Instance.emission_name`
   (`StringId`) verbatim. The bridge guarantees uniqueness and
   determinism (instance.rs:139); the backend does not re-mint.
2. **Aggregate type names** (`Struct`/`Enum`/`Union`/`Closure`): derive
   `T = sanitize(DefPath.rendered) ++ generic_suffix(args)` where
   `sanitize` maps `::`→`_` and any non-`[A-Za-z0-9_]` to `_`, and
   `generic_suffix` is empty for non-generic items and otherwise the
   canonical args key rendered as `__<hex>` (the same `ArgsKey` the
   interner uses — args_key.rs — so it is post-mono-unique and
   build-stable). Collisions are impossible because `(DefPath, ArgsKey)`
   is the nominal identity key.
3. **Tuples**: anonymous `struct` typedef named
   `tuple_<hash-of-field-TyIds>`; fields `_0`, `_1`, … (gc §2).
4. **Fields**: interned `FieldLayout.name` / `VariantLayout.name`,
   `sanitize`d. Enum variant payload structs: `<EnumName>__<VariantName>`.
5. **Locals**: `debug_name` when `Some` (sanitized); else synthetic `_N`
   from the `LocalId` index (gc §7). Basic blocks: `bb_N` from `BlockId`.

*Rationale / consequences.* All names are pure functions of bridge
certificates (`emission_name`, `DefPath`, `ArgsKey`, interned names) →
deterministic across the build-equivalence quad. `sanitize` is
total and deterministic; a `_clash_<n>` disambiguation pass over the final
name table is the safety net (expected no-op given the identity keys).

*Spec-delta:* yes (gc §7 identifier policy gains the type-name +
sanitize rules).

---

## D2 — Declaration order (C declare-before-use)

**Locked direction.** Forward-declare everything up front, then
definitions in deterministic (F4) order; the only hard ordering
constraint is a topological sort of **type definitions** by by-value
containment.

**Problem.** C requires declare-before-use; the model is an unordered
graph of mutually-referential instances/types.

**Decision.** The single `.c` (D3) is emitted in five ordered banks:
1. **Prelude**: fixed runtime header — includes (`<stdint.h>`,
   `<stddef.h>`, `<stdio.h>`, `<stdlib.h>`, `<string.h>`), the fat-pointer
   typedefs (gc §3.2), the panic routines (D4), the saturating-cast and
   `*WithOverflow` helpers (D7/D8).
2. **Type forward-decls**: tag forward-decls for every aggregate type, in
   F4 order — `struct <T>;` for structs / tuples / enums / closures,
   `union <T>;` for unions (distinct C tag forms).
3. **Type definitions**: full `struct`/`union` bodies (enums lower to
   structs per D9), **topologically sorted by by-value containment** — a
   type that contains another *by value* must follow it (C needs a
   complete type before by-value use: fields, array elements, by-value
   params/returns, `sizeof`/`offsetof`). The containment graph (the only
   load-bearing emission order — everything else is forward-declared), for
   an aggregate `T`: an edge `T → U` iff `U` is the resolved `Ty` of any
   `FieldLayout.ty` (struct/union/tuple field, enum `VariantLayout` payload
   field, closure capture) **or** the **element type of a `Ty::Array`**
   reached that way (`U[N]` needs `U` complete); recurse. **No** edge
   through `Ty::Ref`/`Ty::Ptr`/`Ty::FnPtr` (broken by the bank-2
   forward-decls — pointers to incomplete types are legal) or `Ty::FnDef`
   (ZST, not emitted). Derivable purely from `FieldLayout.ty` + the `Ty`
   interner. F4/instance order (instance.rs:163-178 kind ranks) is the
   deterministic **tie-breaker** only. By-value cycles are impossible for
   sized Rust types (rustc rejects them); a detected by-value cycle is a
   hard error (model corruption).
4. **Declarations**: `extern` statics (foreign / forward refs), function
   prototypes for every function instance (including foreign `body:None`,
   IN-3), and `extern` foreign statics (`init:None`, IN-7).
5. **Definitions**: function bodies + static initializers, F4 order.

**C-ABI prototypes.** A function whose `FnSig.abi == C` emits its
prototype using the **C-ABI lowered** types: read
`model.sig_c_abi_lowerings[sig]` (model.rs:97); for each position with
`CAbiShape::Substituted { emit_as, .. }` emit `emit_as`'s C type, not the
original `FnSig` `TyId`; `Direct` uses the position's own type. The
backend **never re-derives** repr facts (spec §14.5; they are not in the
IR). Nested `FnPtr` declarators recurse through the same table.

**C variadics (FS-5 / gap C12).** A `FnSig` with `c_variadic == true`
(which implies `abi == C` and ≥ 1 fixed input, fn_sig.rs:27-31) emits the
fixed parameters followed by a trailing `, ...` in both the prototype
(bank 4) and the definition (bank 5): `ret name(<fixed params>, ...)`.
The backend trusts the bridge/validator's `c_variadic ⇒ C + ≥1 fixed`
invariant.

*Rationale / consequences.* Forward-declaring everything makes bank 5
order-independent except for the type-definition topo-sort, which is the
one true C constraint. Determinism comes from F4 order as the stable
tie-breaker.

*Spec-delta:* yes (gc gains a "§N declaration order" section).

---

## D3 — File layout / translation-unit partitioning + harness entry

**Locked direction.** Single translation unit first; encode module
hierarchy in `DefPath`-qualified names so a later multi-file split is
rename-free. Multi-file gated/deferred.

**Decision.**
1. **One `.c` file** per compiled model for Phase 1 (no `.h`). The five
   banks of D2 are its structure. Module hierarchy lives only in the
   `DefPath`-qualified names (D1), so a future `DefPath`-prefix partition
   into multiple `.c`+`.h` is a pure split with no renames.
2. **Harness entry.** The codegen-coverage harness (Doc 3) compiles the
   emitted `.c` **together with a generated `main.c`** that the harness
   writes. `main()` calls a **designated entry instance** — the fixture
   declares one `#[no_mangle] fn regseq_main()` (or the harness is told
   the entry `emission_name`); the generated `main` is:
   ```c
   int main(void) { regseq_main(); return 0; }
   ```
   Entry-signature policy: the entry takes no args and returns unit/`()`
   (lowered `void`) or an `i32`/`u32` exit code (returned from `main`).
   A non-unit, non-`{i,u}32` entry return is a harness-setup error
   (fixtures are authored to the entry contract). The entry is the place
   diag-logs fire; the harness captures its stdout.

*Rationale / consequences.* Single TU removes the include/guard story for
P1 and makes the golden C snapshot one file. The generated `main` keeps
the emitted TU free of a hand-written entry, so the snapshot is purely
the lowering output.

*Spec-delta:* no (backend/harness-internal; recorded here + Doc 3).

---

## D4 — Host panic / abort strategy

**Locked direction.** `Assert` failure, `Unreachable`, and `panic=abort`
lower to a host trap: print the message + `abort()` / `__builtin_trap()`.

**Decision.** The prelude (D2 bank 1) defines:
```c
_Noreturn static void regseq_panic(const char *msg) {
    fprintf(stderr, "panic: %s\n", msg);
    abort();
}
```
Panics go to **stderr**, not stdout, so they never pollute the diag-log
stdout oracle (Doc 3); the harness distinguishes a panic by non-zero exit
+ stderr text. Each `AssertKind` maps to a fixed message (Rust-style),
printed via a per-kind `fprintf` then `abort()` (operands formatted by
their `Ty`, per D7):

| `AssertKind` | message |
| ------------ | ------- |
| `BoundsCheck{len,index}` | `index out of bounds: the len is <len> but the index is <index>` |
| `Overflow(op,l,r)` | `attempt to <op> with overflow` |
| `OverflowNeg(v)` | `attempt to negate with overflow` |
| `DivisionByZero(v)` | `attempt to divide by zero` |
| `RemainderByZero(v)` | `attempt to calculate the remainder with a divisor of zero` |
| `MisalignedPointerDereference{required,found}` | `misaligned pointer dereference: address must be a multiple of <required> but is <found>` |
| `NullPointerDereference` | `null pointer dereference occurred` |
| `InvalidEnumConstruction(v)` | `constructed invalid enum discriminant <v>` |

`Unreachable` and `panic=abort` paths call `regseq_panic("internal error:
entered unreachable code")` (Unreachable). Release-mode `Unreachable` MAY
instead emit `__builtin_unreachable()`; Phase-1 default is the trap (fail
loud for the host oracle).

*Rationale / consequences.* stderr/stdout split keeps the dual oracle
clean (behavioral oracle reads stdout only). Fixed messages make panic
fixtures golden-testable.

*Spec-delta:* yes (gc §7 / a new panic section + the `AssertKind` table).

---

## D5 — Terminator C-emission  (spec §7 deferred all six — core P1 delta)

**Locked direction.** Fully pin the C lowering for every terminator.

**Decision (per `TerminatorKind`).** Bodies emit `bb_N:` labels (gc §7);
control flow is labels + `goto`.

- **`Goto(t)`** → `goto bb_<t>;`
- **`Return`** → if the body's output `Ty` is unit `()` (lowered `void`),
  `return;`; else `return _0;` (the return-place local,
  `LocalId::RETURN_PLACE`, spec §25.1). A body whose output `Ty` is
  `Never` is `_Noreturn`; a `Return` terminator cannot occur in it (the
  bridge guarantees divergent bodies end in `Unreachable`/divergent
  `Call`) — treat a `Return` there as model corruption.
- **`SwitchInt{discr, targets}`** → C `switch`:
  ```c
  switch (<discr>) {
      case <v0>: goto bb_<t0>;
      ...
      default:  goto bb_<otherwise>;
  }
  ```
  emitted when `discr`'s C type is an integer that fits C `switch`
  (≤ 64-bit; the common cases — `bool`, small `repr(int)` enums,
  integer scrutinees). When `discr` is 128-bit (`SwitchTargets.cases`
  are `u128`, terminator.rs:129), fall back to an if-else chain
  (`if (d==v0) goto..; else if ... else goto otherwise;`). Bool switches
  are the two-case form.
- **`Call{callee, args, destination, target}`**:
  - **Diag-log interception first** (D6): if `callee` is
    `Direct(inst)` and `model.instances[inst].item` carries
    `AttrData::Intrinsic(IntrinsicKind::Log(..))`, emit the §8 `printf`
    (per D6) **instead of** a call, then `goto bb_<target>;`.
  - **`Direct(inst)`** (ordinary) → `name(args...)` where `name =
    emission_name(inst)`. **`Indirect(op)`** → `(<op-expr>)(args...)`.
  - **Destination**: if the callee's return `Ty` is unit/ZST, emit
    `<call>;` (no assignment). Otherwise `<dest-lvalue> = <call>;`
    (`destination` is a `Place`, lowered per gc §4).
  - **Successor**: `target: Some(t)` → append `goto bb_<t>;`.
    `target: None` (divergent; the callee's output is `Never` by
    invariant) → emit `<call>;` then `__builtin_unreachable();` with **no**
    destination assignment (the result type is uninhabited).
  - **Drop-as-`Call`** (spec §23.4): no special case — it is a `Direct`
    call to a `DropGlue` instance; emit per the `Direct` rule.
- **`Assert{cond, expected, msg, target}`** →
  `if ((<cond>) == <expected>) goto bb_<target>; else <D4 panic for msg>;`
  (`expected` is a compile-time `bool`; the success path is `cond ==
  expected`).
- **`Unreachable`** → D4 trap.

*Rationale / consequences.* Labels + `goto` reproduce arbitrary MIR CFGs
without restructuring (the model is already RPO, gc §7). The unit/ZST and
divergent-`Call` carve-outs (Codex F1) avoid assigning uninhabited/absent
results.

*Spec-delta:* yes (this is the **core** delta — gc §7 terminator section).

---

## D6 — Diagnostic-log call interception

**Locked direction.** The log-marked item is a **recognition tag**:
intercept at each `Direct` call site, emit the §8 `printf`, emit **no** C
definition for the marked item. Args arrive peeled flat.

**Decision.**
1. **Exclusion from the emission walk.** During D2 bank 4/5, an instance
   whose `item` carries `Log(..)` is **skipped** (no prototype, no
   definition). It exists only as a recognition tag.
2. **Recognition** (at `Call`/`Direct(inst)`, inside D5): the predicate
   is `model.instances[inst].item`'s `domain_attrs.item_attrs` contains
   `AttrData::Intrinsic(IntrinsicKind::Log(level))` (model.rs:1928-1982;
   domain_attr.rs:62-76).
3. **Emission** (per gc §8): `args` is the peeled flat list
   `[fmt_const, a1, … aN]` (gc §8.5). Build the C format string =
   `"[<LEVEL>] "` ++ translated template ++ `"\n"`, where the template's
   `{...}` specifiers map to C specifiers by each arg's `Ty` (gc §8.2,
   §8.3 for `&str`); emit `printf(<fmt>, <a1>, … <aN>);`. `&str` args
   follow the §8.3 static-literal vs stack-buffer rule.
4. **D8/`max_str_args_per_call`** is bridge-enforced (gc §8.4); the
   backend trusts it.

*Rationale / consequences.* Interception lives inside D5's `Call`
handling, so diag-log end-to-end depends on D5 (Doc 1 Cross-dependencies);
this is the dual oracle's behavioral fixture (Doc 3).

*Spec-delta:* no (gc §8 already pins the printf shape; this records the
call-site mechanics + exclusion rule).

---

## D7 — Rvalue + operator C-emission  (gap B4 — the largest block)

**Locked direction.** None pre-existing; spec §21.3.4 commits only to the
variant set. Pin per-variant C emission.

**Decision.**

**`Rvalue` variants.**
- `Use(op)` → the operand expression (gc §5).
- `Repeat(op, n)` → array fill: a compound literal is not portable for
  runtime `n`, so emit a fill loop into the destination
  (`for (size_t i=0;i<n;i++) dst[i] = <op>;`); `n` is the post-mono
  constant (rvalue.rs:29).
- `Ref(_, place)` / `RawPtr(_, place)` → `&<place-lvalue>`. For an unsized
  pointee the result is a fat pointer built per D8 `Unsize` / §3.2.
- `Cast(k, op, ty)` → per D8.
- `BinaryOp(op, a, b)` → per the BinOp table below.
- `UnaryOp(op, a)` → `Not`: `!a` for `bool`, `~a` for integers; `Neg`:
  `-a`; `PtrMetadata`: read the fat-pointer metadata slot (`.len` or
  `.vtable`, §3.2).
- `Discriminant(place)` → per D9 (read the tag/niche).
- `Aggregate(kind, ops)` → per D9 (enum) / D10 (struct/union/tuple/array);
  closure → captures-struct compound literal (gc §9.7); `RawPtr` aggregate
  → build the §3.2 fat pointer from `(data, metadata)`.

**`BinOp` (host flag `-fwrapv`, pinned in Doc 3, defines signed +,-,*
overflow).**
- `Add/Sub/Mul` → `a + b` / `a - b` / `a * b` (wrapping; `-fwrapv` covers
  signed). `AddUnchecked/SubUnchecked/MulUnchecked` → same operators
  (defined-wrap is a sound superset of their UB-on-overflow).
- `Add/Sub/MulWithOverflow` → `__builtin_{add,sub,mul}_overflow(a, b,
  &tmp)` into a result aggregate `struct { T v; _Bool overflow; }` matching
  the IR's `(T, bool)` (spec §7535); the bool is the builtin's return, `v`
  is `tmp`.
- `Div/Rem` → `a / b` / `a % b`. **Sound because** rustc always inserts
  the guarding `Assert(DivisionByZero)` and `Assert(Overflow)` (INT_MIN/-1)
  terminators upstream (terminator.rs:150-160); the bare op never sees the
  UB inputs.
- `BitXor/BitAnd/BitOr` → `a ^ b` / `a & b` / `a | b` (works for `bool`
  and integers).
- **Shifts** (Codex F2 — `-fwrapv` does *not* cover shift UB):
  - `Shl` (wrapping) → compute in the unsigned domain with masked count:
    `(T)((U)a << (b & (BITS-1)))` where `U` is the unsigned same-width
    type. `Shr` (wrapping) → masked count `a >> (b & (BITS-1))`; the shift
    is **arithmetic** for signed `a` (cast through the signed type) and
    **logical** for unsigned `a` (host two's-complement arithmetic shift is
    relied on and pinned as a host-target assumption).
  - `ShlUnchecked/ShrUnchecked` → no mask; same unsigned-domain (`Shl`) /
    signedness-typed (`Shr`) form, count unmasked (caller guarantees
    `b < BITS`).
- `Eq/Ne/Lt/Le/Ge/Gt` → `a == b` etc., result `_Bool`.
- `Cmp` → three-way `core::cmp::Ordering` as `i8` (-1/0/1):
  `(int8_t)((a > b) - (a < b))` (spec §7548).
- `Offset` → pointer offset with `<*const T>::offset` semantics:
  `a + b` (C pointer arithmetic scales by `sizeof(*a)`; `b` is `isize`).

**`Assign(place, rvalue)`** → `<place-lvalue> = <rvalue-expr>;` for scalar
RHS. For an **aggregate** RHS / large value, struct assignment (`dst =
src;`) is used when both are the same C aggregate type; `memcpy(&dst,
&src, sizeof)` is the escape hatch for byte-blob/enum storage (D9). The
backend MAY choose `memcpy` by a size threshold; both are behaviorally
identical (gc §5).

*Rationale / consequences.* The wrapping/unchecked unification + the
shift unsigned-domain fix close the signed-UB hole (Codex top-risk #2).
Div/Rem rely on the upstream-Assert invariant — a load-bearing assumption
recorded for the validator.

*Spec-delta:* yes (a **new gc Rvalue section** + the BinOp/UnOp tables).

---

## D8 — Cast C-emission  (gap B4, `CastKind`/`PointerCoercion`)

**Decision (per `CastKind`).**
- `IntToInt` → `(T)x` (truncate/extend; two's-complement narrowing pinned
  as a host assumption). `FloatToFloat` → `(T)x`. `IntToFloat` → `(T)x`.
- `FloatToInt` → **saturating helper, never a bare cast** (C is UB
  out-of-range; Rust `as` saturates, `NaN`→0). Prelude emits per
  source-float × dest-int helper
  `static <DI> regseq_f<W>_to_<DI>(<F> x)`:
  `if (isnan(x)) return 0; if (x <= (F)DI_MIN) return DI_MIN; if (x >=
  (F)DI_MAX) return DI_MAX; return (DI)x;` (bounds checked *before* any
  cast; ±inf handled by the comparisons).
- `PtrToPtr` → `(T*)p`. `FnPtrToPtr` → `(T*)p` **as a documented hosted-
  target extension** (object↔function pointer round-trip is not ISO C but
  is guaranteed on POSIX hosts; pinned as a host-target assumption, not
  ISO-portable).
- `PointerExposeProvenance` → `(uintptr_t)p`.
  `PointerWithExposedProvenance` → `(T*)n`.
- `Transmute` → bit reinterpret via `memcpy` between equal-`sizeof`
  objects (no union pun; avoids type-punning aliasing UB): `memcpy(&dst,
  &src, sizeof dst)`.
- `Subtype` → no-op (representation-identical; emit the operand).
- `PointerCoercion(pc)`:
  - `Unsize` → build the §3.2 fat pointer: slice/array→ `{data=ptr,
    len=N}`; `str` → `{data, len}`; `dyn` → `{data=ptr, vtable=&VT}`
    (per-case, Codex F3).
  - `ReifyFnPointer` → `&<fn>` (function designator → fn pointer).
  - `ClosureFnPointer` → `&<synthesized-fn>` for the non-capturing
    closure's call shim.
  - `UnsafeFnPointer` → no-op (safe→unsafe fn-ptr is C-identical).

*Spec-delta:* yes (new gc cast section + the saturating-helper contract +
the two host-target assumptions, FnPtr↔ptr and arithmetic shift).

---

## D9 — Enum C representation  (gap B5 — Codex top-risk #1; was WRONG)

**Locked direction.** Lower per `EnumShape`/`TagEncoding` (gc §2, §9.8,
§9.9). Codex first-contact **refuted** the naive `{tag; union;}` shape.

**Problem.** `TagScalar.offset` is arbitrary and **variant payload field
offsets are absolute to the whole enum object, not the payload start**
(layout.rs:63-67, 146-154). A naive tag-at-0 + contiguous-union struct
does **not** reproduce rustc's layout; niche encoding has **no tag field**
at all and is **general** (not Option-only; layout.rs:168-182). Byte
fidelity is required wherever enum bytes are observed externally
(`Transmute`, `CopyNonOverlapping`, C-ABI).

**Decision — offset-faithful, uniform across all `EnumShape`
(union-of-views).** An enum lowers to a C `struct` built by the **D10
field-offset technique**, forced to `__attribute__((aligned(align)))` (not
`_Alignas`, invalid on a tag definition — same fix as D10). Its members
reproduce the exact recorded offsets via **full-object views**: the tag cell
and each variant's payload are each a `struct` whose fields sit at their
**absolute** offsets from the enum object's start (a field's view-relative
offset *is* its absolute offset → naturally aligned → no view needs
packing), all sharing **one** `union` —
- a **tag view** (when present: `Single{scalar:Some}` or `Multiple{Direct}`
  — **not** `Multiple{Niche}`) holds `<TagScalar.scalar_ty>` at
  `TagScalar.offset`;
- a **variant view** per variant with ≥ 1 non-ZST field (fieldless
  variants → no view);
- exactly **one** view → emitted **inline** (no `union`): §9.8 fieldless
  `Direct` → tag-only struct, §9.9 niche → single-field struct; **zero**
  views → a size-faithful struct;
- `_Static_assert(sizeof(E)==size)`, `_Static_assert(_Alignof(E)==align)`,
  and a per-field `_Static_assert(offsetof(E, <view>.<field>)==<absolute
  offset>)` (nested member designator, gcc/clang host assumption) validate
  every placement at compile time.

This is uniform: niche enums have no tag view; the niche cell is a payload
field *in a variant view* whose bit pattern *is* the tag. No carve-out
(Codex: a niche carve-out is unsound — niche is general and not
bridge-rejected).

**Why not a single tag field + payload `union` at a `union_start`?** (The
original D9 wording.) That form is **not C-placeable** for shifted-alignment
`Direct` enums — `enum E { A(u8), B(u32) }` is tag @0, `A.0` @1, `B.0` @4;
a C `union` holding a `uint32_t` (align 4) cannot sit at the first payload
byte (offset 1), and `B.0` at `4 − union_start = 3` is misaligned. The
union-of-views uses **no `union_start`** (every field at its absolute,
naturally-aligned offset within a view), so it is placeable for any layout.
(Surfaced by the task-c2 Codex review; gc §13 + this decision were updated
together.)

**Tag operations** (drive `Discriminant`/`SetDiscriminant`/`Downcast`):
- `Discriminant(place)` (RV-8):
  - `Empty` → unreachable (no values).
  - `Single` → the single variant index (constant; `scalar` irrelevant to
    the *read*).
  - `Multiple` (both `Direct` **and** `Niche`) → read the tag/niche cell
    at `TagScalar.offset`, then **compare the read value against each
    variant's precomputed `VariantLayout.tag_value`** and select the
    matching variant. This uses the bridge-precomputed `tag_value`s
    (layout.rs:122-135) rather than re-deriving the niche formula, so it
    is automatically correct for the **mod-2^scalar_width** storage and
    **wrapping** `WrappingRange` cases (layout.rs:168-182, 197-211). For
    `Niche`, exactly the niched variants carry `Some(tag_value)` and the
    untagged variant carries `None`; **no match → `untagged_variant`**
    (its payload is the tag). The compare set and fallthrough are emitted
    as an if-chain (or `switch` for `Direct` with small tag values).
- `SetDiscriminant(place, v)` (ST-2):
  - `Direct` / `Single{scalar:Some}` → store `VariantLayout[v].tag_value`
    (`Some` by construction) into the tag cell.
  - `Niche` niched variant → store its `tag_value` into the niche cell.
  - `Niche` untagged variant → **no-op** (`tag_value: None`; payload is the
    tag, gc §9.9).
  - `Single{scalar:None}` → no-op (no tag bytes).
- `Downcast(v)` then `Field(i,_)` (PL-8) → access variant `v`'s payload
  field `i` at its `FieldLayout.offset` (D10 access).
- `Aggregate::Enum(ty, v)` (AG-4) / `Const::EnumValue` (CO-9) → write the
  active variant's payload fields at their offsets + the tag write (as
  `SetDiscriminant(v)`).

*Rationale / consequences.* Offset-faithful emission is the only
representation correct for **all** shapes and for external byte
observation; it reuses D10 as its primitive. The cost is less "natural" C
structs, but the `_Static_assert` guards make the snapshot self-checking.
A readability optimization (named `{tag; union;}` when the layout proves
tag-prefix-compatible) is **deferred** — not Phase-1.

*Spec-delta:* yes (gc §2 enum row + a new enum-representation section
generalizing §9.8/§9.9; tag-op algorithms).

---

## D10 — Aggregate field-offset reproduction  (gap B6)

**Locked direction.** None pre-existing; Doc 1 surfaced that
`FieldLayout` is source/MIR order, not memory order.

**Problem.** `FieldLayout`/`VariantLayout` field vectors are in
source/MIR order (layout.rs:35-39); `repr(Rust)` reorders fields, so
emitting them verbatim does **not** reproduce `offset`.

**Decision.** Every aggregate (`struct`, `union`, tuple, enum payload,
closure captures) is emitted by this technique:
1. Sort the emittable fields by `FieldLayout.offset` ascending.
2. **Omit ZST fields** from emission but account for them: `PhantomData`
   (`is_phantom_data`, spec §14.5 / §5721-5733) and unit-tuple/ZST fields
   are dropped (they occupy 0 bytes); multiple same-offset ZSTs collapse.
3. Insert explicit `unsigned char _padN[k];` fillers to advance from the
   previous field's end to the next field's `offset`, and a trailing pad
   to `layout.size`.
4. Carry alignment: force the C **type** alignment to `layout.align` via
   `__attribute__((aligned(layout.align)))` (not `_Alignas`, which is invalid
   on a `struct`/`union` tag definition); `pack: Some(_)` adds `packed` →
   `__attribute__((packed, aligned(layout.align)))` (LY-2). The alignment is
   **always `layout.align`, never `pack`'s `n`** (`repr(packed(n))` true
   alignment = `min(max_field_align, n) = layout.align`; the step-6 `_Alignof`
   guard asserts it).
5. **Unions** (`LayoutKind::Union`): all field offsets are 0
   (layout.rs:41) → a C `union` of the fields (no padding between; trailing
   pad to `size`).
6. Emit `_Static_assert(sizeof(T)==size)`,
   `_Static_assert(_Alignof(T)==align)`, and per-field
   `_Static_assert(offsetof(T,f)==offset)` guards.
7. **Field access** (gc §4 `.fname`): direct member access for the natural
   case; `memcpy` to/from a typed temporary when accessing through enum
   byte storage or a packed/misaligned field (avoids alignment UB).

*Rationale / consequences.* Explicit padding + static-asserts reproduce
*any* rustc layout (repr(C) and repr(Rust)) and fail the build loudly on a
mismatch — strictly better than scoping guarantees to repr(C). This is the
shared primitive for D9.

*Spec-delta:* yes (gc §2 aggregate rows + a new field-offset section).

---

## D11 — `Const` → C initializers  (gap B7)

**Locked direction.** None pre-existing; gc §5 defers per-variant forms.

**Decision (per `Const`, const_value.rs:18-51).** Materialized at use
sites (`Operand::Constant`) and as static/const initializers:
- `Bool(b)` → `1`/`0`. `Char(u)` → `(uint32_t)<u>`.
- `Int(k,v)` → width-suffixed signed literal of the value (`v` is the
  sign-extended bit pattern; print the signed value, `LL`/`LLU` as
  needed; `I128` → `__int128` literal composed from two 64-bit halves as a
  host assumption). `Uint(k,v)` → unsigned literal (`U`/`ULL`).
- `Float(k,bits)` → a **compile-time constant expression** that is exact
  and bit-faithful, valid in **both** value sites and static/const
  (including nested-`Aggregate`) initializers — because each form below is
  a C constant expression, unlike a union member access:
  - **finite** (normal / denormal / ±0) → a C99 **hex-float literal**
    (`0xH.HHHHpE` with the per-kind suffix), which names any finite IEEE
    value exactly;
  - **±infinity** → `__builtin_inff()` / `__builtin_inf()` (per kind);
  - **NaN** → `__builtin_nanf("0x<payload>")` (quiet) /
    `__builtin_nansf("0x<payload>")` (signaling), payload-preserving.
  These are GCC/Clang **constant expressions** (the same primitives behind
  `<math.h>`'s `INFINITY` / `NAN`), pinned as a host-target assumption;
  `_Float16` / `_Float128` are host-dependent. No runtime union is needed,
  and the forms compose directly inside `Const::Aggregate` static
  initializers.
- `ZST` → no bytes; in value position a no-op / `(void)0`; as an aggregate
  field, omitted (D10).
- `Bytes(Vec<Byte>)` → `unsigned char[]` initializer: `Value(b)`→`b`,
  `Uninit`→`0` (uninit lowered to a defined zero for host determinism),
  `Reloc(r)`→ the address bytes of `r`'s symbol (emitted as a pointer slot,
  see `Reloc`).
- `Aggregate(children)` → C compound literal `(T){ <child inits> }` in
  D10 field order (padding via designated/zeroed init).
- `EnumValue{variant_idx, payload}` → per D9 (tag write + payload fields).
- `NullPtr(ty)` → `(T*)0`.
- `Reloc(InstanceRef)` → `&<emission_name of the referenced instance>`
  (address-of); for a `Byte::Reloc` slot inside `Bytes`, the pointer
  occupies one pointer-width slot.

*Rationale / consequences.* `Uninit`→0 trades Rust's "uninitialized" for
host determinism (the golden snapshot + behavioral oracle need defined
bytes); recorded as an intentional generic-C choice. Float consts use
exact **constant-expression** forms (hex-float + `__builtin_{inf,nan}`),
so they are bit-faithful (incl. NaN payloads) *and* legal inside static
aggregate initializers — no union member access, which is not a constant
expression.

*Spec-delta:* yes (gc §5 gains the per-`Const` initializer table).

---

## D12 — Symbol & declaration policy  (gaps C8, C10)

**Locked direction.** None pre-existing.

**Decision.**
- **`Linkage`** (item.rs:136-147): `Default` → ordinary definition (file-
  scope, no `static` since cross-instance refs exist in one TU — emit at
  file scope without `static`); `NoMangle`/`export_name` → definition with
  the verbatim `emission_name` (already minted); `Foreign` → **declaration
  only** (`extern`, D2 bank 4), never defined; `Weak` →
  `__attribute__((weak))` on the definition.
- **`link_section: Some(s)`** (IT-8) → `__attribute__((section("<s>")))`
  on the emitted def.
- **`inline_hint`** (IT-9): `Hint`→`inline`; `Always`→`inline
  __attribute__((always_inline))`; `Never`→`__attribute__((noinline))`;
  `None`→nothing.
- **`Static{mutability}`** (IT-2): `Mutability::Not` → `const`-qualified
  global; `Mut` → non-const global. `init: Some` → with initializer (D11);
  `init: None` (foreign) → `extern`, no initializer (IN-7).
- **`Const` item** (IN-8): does **not** get its own C symbol by default —
  the value is materialized at use sites (D11). (A `static const` may be
  emitted if a `Reloc` references the const item; then it gets a symbol.)
- **Function instances** (IN-2/3): `body: Some` → prototype (bank 4) +
  definition (bank 5); `body: None` (foreign) → prototype/`extern` only.
- **Shim flavors** (IN-5/6): reachable flavors (Item/Intrinsic/VTableShim/
  ReifyShim/FnPtrShim/Virtual/ClosureOnceShim/ThreadLocalShim/DropGlue/
  CloneShim/FnPtrAddrShim) all emit as ordinary C functions whose
  **bridge-synthesized body** is already in `FunctionInstanceData.body`;
  the backend lowers that body like any other (no per-flavor special
  case). `virtual_idx: Some(i)` participates only in **vtable instance**
  construction: the vtable static's slot `i` is initialized to `&<shim
  emission_name>` (gc §9.6). Coroutine/async flavors are unreachable
  (spec §15.1; Doc 1 IN-5b).

*Rationale / consequences.* Single-TU means file-scope-without-`static`
for `Default` linkage (every instance may be referenced by another in the
same TU). Const-items-inline avoids needless symbols; the `Reloc`
exception covers address-taken consts.

*Spec-delta:* yes (gc gains a symbol/linkage/section/inline section).

---

## Open-decision log

NEW forks surfaced while filling Doc 2 (resolved inline above; recorded so
nothing was silently decided in implementation):

1. **`Uninit` byte → defined `0`** (D11) — host-determinism choice over
   faithful "uninitialized"; needed for the golden oracle.
2. **Host-target assumptions** (D7/D8/D11) — arithmetic right-shift for
   signed integers (D7), function↔object pointer round-trip (POSIX, D8),
   and GCC/Clang float constant-expression builtins
   (`__builtin_inf`/`__builtin_nan`, D11). Pinned as generic-C host
   assumptions, not ISO-portable guarantees; recorded for the per-target
   backends that will revisit them.
3. **Readable `{tag; union;}` enum form** — deferred optimization over the
   offset-faithful canonical form (D9); not Phase-1.
4. **`-fwrapv` + unsigned-domain shifts** (D7) — the signed-overflow
   strategy is split (flag covers +,-,*; shifts lowered explicitly);
   Doc 3 pins the flag.

These feed the `generic-c-backend.md` spec delta (every `Spec-delta: yes`
above) and then `task.py` tasks.
