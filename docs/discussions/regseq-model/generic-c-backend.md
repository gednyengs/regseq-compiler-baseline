# Generic-C backend

Top-level merger artifact for the generic-C backend codegen
spec. Introduced by stage-14 **D-14-10** (generic-C backend
codegen carve-out) per README §5.9 rule 7. Co-equal with
`spec.md` (producer-side IR structure) and `pretty-printer.md`
(consumer-side rendering surface); this file holds the
backend-side C-emission surface.

Audit-shape decision text for D-14-10 lives in
`discussions.md` under `## Stage 14`. This file is
**relocation content** -- the sections below were originally
specified in earlier stages (3, 5, 6, 7, 11, 14 per
per-section provenance) and lifted from `spec.md` to here at
stage-14 close per the §5.9 rule 7 dual-backend split.
Original audit trails remain in those stages' `discussions.md`
contributions; D-14-10 changes the canonical-spec location
only, not the content. Linux-flavored content was dropped per
"no forward-looking placeholders" and filed as inbound
expectations to phase-2 stages 15 / 16 / 17; see
`stage-15/expectations.md`, `stage-16/expectations.md`,
`stage-17/expectations.md`.

---

## 1. Mission and scope

This file specifies how RegseqModel constructs lower to C in
the **generic-C backend** (phase-1 implementation territory:
hosted userspace, libc stdio, `main()`, host-platform
integer conventions). The Linux kernel backend, Zephyr backend,
etc. each get their own merger artifact under the project
`README.md` §2 "Dual backend" rule.

**Scope boundary with `spec.md` and `pretty-printer.md`:**

- `spec.md` -- IR-side producer-surface: type universe,
  layout, place/operand vocabulary, terminator vocabulary,
  intrinsic recognition, validation diagnostics, bridge
  translation rules. The IR is the input to this backend.
- `pretty-printer.md` -- IR-rendering consumer-surface:
  textual rendering of the IR for snapshots and diagnostics.
- `generic-c-backend.md` (this file) -- C-emission
  consumer-surface: how each IR construct lowers to C source
  for the generic-C target. The output is C source code
  consumable by a host C toolchain (clang / gcc).

Per the project `README.md` §2 "Dual backend" rule, this
backend is **not** parameterised over an OS-integration table. The Linux backend
will *consult* the rules here as reference but maintain its
own separate file. Integer-type lowering, OS-integration
boundaries, and per-target call shapes differ enough that a
parameterised single-backend design is rejected by §2.

**Cross-references.** Each section below opens with a pointer
back to the corresponding `spec.md` section that defines the
IR construct. `spec.md` sections that previously carried
C-emission content now close with a pointer forward into this
file.

---

## 2. Per-`Ty` C lowering table

IR-side definition: `spec.md` §10 (the closed 19-variant `Ty`
enum). The generic-C target lowers each variant as follows:

| Variant | C lowering |
|---------|------------|
| `Bool` | `_Bool` / `uint8_t` |
| `Char` | `uint32_t` (Unicode scalar value) |
| `Int(IntKind)` | `intN_t` per width (`I8` -> `int8_t`, ..., `I128` -> `__int128`); `Isize` -> `intptr_t` |
| `Uint(UintKind)` | `uintN_t` per width; `Usize` -> `size_t` |
| `Float(FloatKind)` | `_Float16` / `float` / `double` / `_Float128` per kind |
| `Str` | only behind `Ref`/`Ptr`; fat pointer per §3 |
| `Never` | `void` + `__attribute__((noreturn))` in fn-return position; accepted as `Ref`/`Ptr` pointee (codegen MAY reject) |
| `Array(TyId, u64)` | C array `T[N]` in field/local context only (rejected at `FnAbi::C` parameter/return per `spec.md` §14.5; C arrays decay to pointers in those positions) |
| `Slice(TyId)` | only behind `Ref`/`Ptr`; fat pointer per §3 |
| `Ref(TyId, Provenance)` | `T*` / `const T*` / `T* restrict` per §3 |
| `Ptr(TyId, Mutability)` | `T*` / `const T*` (raw) per §3 |
| `FnDef(ItemId, GenericArgs)` | ZST (function-identity tag); no C value emission, no C type |
| `FnPtr(FnSigId)` | C function pointer `T (*)(args)` |
| `Dynamic { auto_traits, vtable_ty }` | only behind `Ref`/`Ptr`; fat pointer per §3 |
| `Closure(ItemId, GenericArgs)` | C `struct` (synthesised captures-struct's parallel `Ty::Struct` C lowering) |
| `Tuple { fields, layout }` | anonymous C struct with synthetic field names `_0`, `_1`, ... for non-empty tuples; the **empty tuple** `Ty::Tuple { fields: [] }` is Rust `()` -- ZST with `size: 0, align: 1`, lowered to C `void` in function return position (per `spec.md` §14.5) and treated as a value-less marker elsewhere |
| `Struct { item, args, layout }` | C `struct`; field placement reproduces the layout offsets per §14 |
| `Enum { item, args, layout }` | offset-faithful tagged / niche-encoded C `struct` per the layout's `EnumShape` / `TagEncoding` — general rule at §13 (the §9.8 / §9.9 examples are now special cases of §13) |
| `Union { item, args, layout }` | C `union` (field placement per §14) |

Opaque-handle pattern (e.g., `core::ffi::c_void` rewrites per
`spec.md` Appendix A.1): an `#[repr(C)] struct OpaqueCtx {
_private: [u8; 0] }` is structurally an ordinary `Ty::Struct`
and lowers to a forward-declared struct pointer (`struct
OpaqueCtx *`).

---

## 3. Reference and pointer C lowering

IR-side definition: `spec.md` §17.1 (reference and pointer
type lowering) + §17.2 (fat-pointer aggregates) + §17.2.3
(trait-object dispatch carve-out -- IR-side erasure rule).

### 3.1 Thin reference and pointer C lowering

For sized pointees:

- `Ref(t, SharedBorrow)` lowers to `const T*` if
  `!contains_unsafe_cell(t)`, else `T*`.
- `Ref(t, UniqueBorrow)` lowers to `T*` (mutable borrow already
  drops `const`; `restrict` is a separate per-context decision).
- `Ptr(t, Mutability::Not)` lowers to `const T*` (raw pointers
  do not consult `is_unsafe_cell`; raw-pointer framing).
- `Ptr(t, Mutability::Mut)` lowers to `T*`.

### 3.2 Fat-pointer C struct shapes

For unsized pointees (`Ty::Str`, `Ty::Slice(_)`,
`Ty::Dynamic{..}`), the `Ref` / `Ptr` value lowers as a
two-slot C struct, not as `T*`:

| Pointee + borrow                       | `data` field type                                                                                                                                              | Second slot                          |
| -------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------ |
| `Ref(Str, SharedBorrow)`               | `const uint8_t *`                                                                                                                                              | `len` -- target-width unsigned       |
| `Ref(Str, UniqueBorrow)`               | `uint8_t *`                                                                                                                                                    | `len` -- target-width unsigned       |
| `Ptr(Str, Mutability::Not)`            | `const uint8_t *`                                                                                                                                              | `len` -- target-width unsigned       |
| `Ptr(Str, Mutability::Mut)`            | `uint8_t *`                                                                                                                                                    | `len` -- target-width unsigned       |
| `Ref(Slice(T), SharedBorrow)`          | `const T*` if `!contains_unsafe_cell(T)`, else `T*`                                                                                                            | `len` -- target-width unsigned       |
| `Ref(Slice(T), UniqueBorrow)`          | `T*`                                                                                                                                                           | `len` -- target-width unsigned       |
| `Ptr(Slice(T), Mutability::Not)`       | `const T*`                                                                                                                                                     | `len` -- target-width unsigned       |
| `Ptr(Slice(T), Mutability::Mut)`       | `T*`                                                                                                                                                           | `len` -- target-width unsigned       |
| `Ref(Dynamic{..}, _)` or `Ptr(Dynamic{..}, _)` | `void *` (unqualified -- see §3.3)                                                                                                                            | `vtable` -- `const VTable *`         |

C typedef shapes per fat-pointer flavor:

- `struct rust_str { const uint8_t *data; size_t len; };`
  (mutability variation per the table; `len` field type is
  `size_t` on hosted-userspace; the abstract "target-width
  unsigned" gates the kernel/embedded variations).
- `struct rust_slice_<T> { const T *data; size_t len; };`
  (per-`T` expansion; mutability variation per the table).
- `struct trait_obj_<Trait> { void *data; const
  <Trait>_VTable *vtable; };` (data slot unqualified per §3.3).

See §9.2 worked example for concrete syntax.

### 3.3 Trait-object dispatch: `data` slot rendering

IR-side rule: `spec.md` §17.2.3 specifies that at the
trait-object boundary, the source mutability/`Provenance`
discriminant on the `data` slot is **erased** because the
`contains_unsafe_cell` predicate is undecidable on the
`Dynamic{..}` pointee (the concrete `Self` is not in the
type, and shared `&self` methods on impls whose `Self`
contains `UnsafeCell` may legitimately mutate through the
shared reference).

The generic-C rendering of this erasure: the `data` field is
emitted as **unqualified `void *`** regardless of the source
`Ref(Dynamic{..}, SharedBorrow)` / `Ref(Dynamic{..},
UniqueBorrow)` / `Ptr(Dynamic{..}, _)` flavor. The `const`
qualifier is dropped from the C type because promising
C `const` through type-erased dispatch would be C undefined
behaviour for UnsafeCell-containing impls. The vtable pointer
slot itself remains `const VTable *` -- vtable instances are
immutable statics.

The vtable shim's receiver is uniformly `*mut ()` (`void *`)
per `spec.md` §13.2; this section's `void *` rendering of the
`data` slot is consistent with that shim convention.

The Rust borrow flavor remains visible at the IR's
type-system level (the source `Ref(Dynamic{..}, SharedBorrow)`
is preserved in the IR even though the C-level qualifier is
dropped at emission).

### 3.4 C boundary rejection

The fat-pointer C lowerings above apply to **Rust-internal**
uses (Rust-ABI function calls, value-context locals, struct
fields). At `FnAbi::C` parameter/return positions, `Ref` /
`Ptr` to unsized pointees are **rejected** by the C ABI
admissibility predicate (`spec.md` §14.5). Driver code
crossing extern "C" passes the data pointer and length /
vtable pointer as separate parameters; the wide-pointer
aggregate is not idiomatic at the C boundary.

---

## 4. `PlaceElem` C lvalue forms

IR-side definition: `spec.md` §19 (`Place` and `PlaceElem`).

Each `PlaceElem` maps to a C lvalue construct:

| Variant | C lvalue form |
|---|---|
| `Deref` | unary `*p` |
| `Field(idx, _)` | `.fname` (field name from predecessor layout's `FieldLayout.name`; per-renderer dispatch in `pretty-printer.md`) |
| `Index(local)` | `[local_lvalue]` (where `local_lvalue` is the C lvalue of the `LocalId` payload) |
| `ConstantIndex { offset, ..., from_end: false }` | `[offset]` |
| `ConstantIndex { offset, ..., from_end: true }` | `[len - offset]` (with `len` being the predecessor slice's runtime length; `from_end: true` is slice-only per `spec.md` §19.4) |
| `Subslice { from, to, from_end: false }` | a slice expression with bounds `[from, to)` |
| `Subslice { from, to, from_end: true }` | a slice expression with bounds `[from, len - to)` |
| `Downcast(VariantIdx)` | tagged-union arm projection (form depends on the C representation chosen for tagged unions; pinned alongside the per-`Ty` table at §2) |

The lowering per variant is deterministic from the
projection-chain content; no special-case flags.

---

## 5. `Operand` C lowering

IR-side definition: `spec.md` §20 (`Operand`).

The generic-C backend lowers each variant as follows:

| Variant         | C lowering                                                  |
| --------------- | ----------------------------------------------------------- |
| `Copy(p)`       | C value-load from the C lvalue of `p` (per §4). The backend may emit a plain assignment, a struct-copy, or `memcpy` based on size and layout policy. |
| `Move(p)`       | Same C value-load as `Copy(p)`. The variant tag is informational at the C level; a backend that wants to specialise (e.g., emit `memcpy` for large moves while the source place becomes dead, or elide a final move into a return slot) may consult the variant. The bare load semantics are identical to `Copy`. |
| `Constant(id)`  | C literal or compound literal materialised from `model.consts.resolve(id)`. The per-`Const`-variant initializer forms are pinned at **§12**; they compose with the per-`Ty` table at §2 and the `Const` vocabulary defined in `spec.md` §9.7. |

Because the IR has no `RuntimeChecks` variant (per `spec.md`
§20.3.1), the C backend never sees a session-flag query at
operand site; it sees `Constant(ConstId)` referencing a
`Const::Bool(b)` exactly as if the source had written the
literal `true` / `false`.

---

## 6. Intrinsic C-emission

IR-side definition: `spec.md` §22 (statement vocabulary,
including `Statement::Intrinsic`).

C-emission shapes:

- `Assume(...)` -> `__builtin_assume(_5)` (clang) or
  `if (!_5) __builtin_unreachable()` (gcc fallback).
- `CopyNonOverlapping{...}` -> `memcpy(_1, _2, _3 * sizeof(T))`,
  where `T` is the pointee type fixed at bridge translation
  per the B-5 invariants in `spec.md` §22.

**Argument-order swap (memcpy vs `copy_nonoverlapping`).**
rustc's `copy_nonoverlapping(src, dst, count)` is `(_2, _1,
_3)` in body source order; C's `memcpy(dst, src, count)` is
`(_1, _2, _3 * sizeof T)`. The named `src` / `dst` fields on
the `CopyNonOverlapping` payload struct (IR-side; defined in
`spec.md` §22) preserve rustc's argument naming so that the
generic-C emission reads the fields by name and produces
`memcpy` arguments in the C order without relying on
positional accident.

---

## 7. Body-to-C function lowering

IR-side definition: `spec.md` §25 (`Body`).

Each `Body` lowers to a C function. The function signature is
determined by the owning Instance's
`InstanceKind::Function.fn_sig` (per `spec.md` §25.4). Body
content lowers as follows:

```c
/* signature from InstanceKind::Function.fn_sig */
ret_ty fn_name(arg_ty1 arg1, ..., arg_tyN argN) {
    /* parameter locals from local_decls[1..=arg_count] are
       the formal arguments; codegen emits them in the C
       signature, not as additional declarations. */

    /* internal locals from local_decls[arg_count+1..]: */
    local_ty_<N1> _N1;   /* or named per debug_name when Some */
    local_ty_<N2> _N2;
    ...

    /* basic blocks from body.basic_blocks (in RPO storage order): */
    bb_0: { stmt_0; ... ; terminator_0; }
    bb_1: { stmt_1; ... ; terminator_1; }
    ...
}
```

**Identifier emission policy.** _(task-11 canonical-doc fix: the
prior text claimed instance `emission_name` was unique and emitted
without re-mint — a wrong premise. The bridge mints
`Instance.emission_name = DefPath.rendered` keyed by the **item**,
so every monomorphization of one generic item shares it; it is also
generic-agnostic and possibly not a valid C identifier. The backend
therefore **derives** the final instance symbol the same way it
derives type names.)_ **Instance** symbols (functions, statics) are
*derived* and linkage-aware:
**external** linkage (`NoMangle` / `export_name` / `Foreign`) →
`sanitize(emission_name)` with **no** generic suffix, so the ABI
symbol is stable across monomorphizations (real external items are
non-generic); **internal** linkage (`Default` / `Weak`) →
`sanitize(emission_name) + generic_suffix(generic_args)`, so distinct
monomorphizations get distinct symbols. (The base is `sanitize`, not
the raw bytes: the bridge mints `DefPath.rendered` — e.g.
`crate::{extern#0}::foo` — for every linkage, not a bare ABI
identifier, and the behavioral harness sanitises identically on both
sides, so `sanitize` is the cross-TU contract; only a generic
*suffix* on an external symbol would break linkage.) Consts
materialise at use sites and have no standalone symbol. A
`_clash_<n>` net over the instance-symbol table (separate from the
type-tag table — C ordinary-identifier vs struct/union/enum tag
namespaces — and reserving the prelude `regseq_*` helpers + the
external symbols' final names) disambiguates any residual internal
collision. **Type** names (struct / enum / union / tuple / closure)
are *derived*:
`sanitize(DefPath.rendered)` (mapping `::` and any
non-`[A-Za-z0-9_]` to `_`) plus, for generic items, a canonical
`__<hex>` suffix from the interner's `ArgsKey` — so the name is
a pure function of the post-mono nominal identity `(DefPath,
ArgsKey)` and is build-equivalence-stable. Tuple typedefs are
`tuple_<hash-of-field-TyIds>` with fields `_0`, `_1`, ...; enum
variant payload structs are `<EnumName>__<VariantName>`. Fields
use the interned `FieldLayout.name` / `VariantLayout.name`
(sanitised). Named locals via `debug_name = Some` use the
recorded name; locals with `debug_name = None` use the synthetic
`_N` form; basic blocks are `bb_N` from `BlockId`. A final
`_clash_<n>` disambiguation pass over the name table is the
safety net (expected no-op given the identity keys). The
identifier-emission convention pinned for the snapshot
pretty-printer (`pretty-printer.md`) coincides with the
generic-C backend's; if a future renderer or backend diverges,
each artifact pins its own convention.

§7 pins the Body envelope (signature template, local decls,
basic-block labels). Per-statement C-emission for `Operand`,
`Rvalue`, and intrinsic statements is at §5 / §10 / §6;
per-terminator C-emission is pinned in §7.1 below. Statements:
`Assign(place, rvalue)` → `<place> = <rvalue>;` (place per §4,
rvalue per §10; struct-copy or `memcpy` for large aggregate
RHS); `SetDiscriminant(place, v)` → the tag write per §13;
`Intrinsic(..)` → §6.

### 7.1 Per-terminator C-emission

IR-side definition: `spec.md` §23 (the closed 6-variant
terminator vocabulary). Control flow is the `bb_N:` labels of
§7 plus `goto`. Each `TerminatorKind` lowers as:

- **`Goto(t)`** → `goto bb_<t>;`
- **`Return`** → `return _0;` (the return-place local
  `LocalId::RETURN_PLACE`, `spec.md` §25.1); for a unit-`()`
  (C `void`) output, `return;`. A `Never`-output body is
  `_Noreturn` and contains no `Return` (the bridge ends
  divergent bodies in `Unreachable` / a divergent `Call`); a
  `Return` there is model corruption.
- **`SwitchInt { discr, targets }`** → a C `switch` when `discr`
  lowers to an integer that fits C `switch` (≤ 64-bit — the
  common `bool` / small-`repr(int)` enum / integer cases):
  `switch (<discr>) { case <v_i>: goto bb_<t_i>; ... default:
  goto bb_<otherwise>; }`. When `discr` is 128-bit (the
  `SwitchTargets.cases` values are `u128`), emit an if-else
  chain instead.
- **`Call { callee, args, destination, target }`**:
  - **Diag-log interception (first).** If `callee` is
    `Direct(inst)` and `model.instances[inst]`'s source `item`
    carries `AttrData::Intrinsic(IntrinsicKind::Log(..))`
    (`spec.md` §26), emit the §8 diagnostic-logging stdio
    emission sequence **instead of** a call (the log-marked item
    is a recognition tag and emits no C definition — see §15),
    then `goto bb_<target>;`.
  - **Ordinary call.** `Direct(inst)` → `name(args...)` with
    `name = emission_name(inst)`; `Indirect(op)` →
    `(<op>)(args...)`.
  - **Destination.** If the callee's return `Ty` is unit / ZST,
    emit `<call>;` (no assignment); else
    `<destination> = <call>;` (`destination` is a `Place`, §4).
  - **Successor.** `target: Some(t)` → append `goto bb_<t>;`.
    `target: None` (divergent; the output is `Never` by
    invariant) → emit `<call>;` then `__builtin_unreachable();`
    with **no** destination assignment.
  - **Drop-as-`Call`** (`spec.md` §23.4): no special case — it
    is a `Direct` call to a drop-glue instance; emit per the
    `Direct` rule.
- **`Assert { cond, expected, msg, target }`** →
  `if ((<cond>) == <expected>) goto bb_<target>; else <panic
  for msg>;` (`expected` is a compile-time `bool`; the panic
  path is §16's `AssertKind` table).
- **`Unreachable`** → the §16 trap.

---

## 8. Diagnostic-logging backend contract

IR-side definition: `spec.md` §26 (diagnostic-logging
intrinsic vocabulary; D-14-2 cluster). Pipeline A uniformity
rule and `max_str_args_per_call` validation diagnostic D8
live IR-side in `spec.md` §26.8 and §26.9; this section
declares the generic-C target's bindings the bridge consults
when enforcing them.

### 8.1 Per-`LogLevel` C emission shape

Each diag-log call lowers to a deterministic **statement
sequence** writing to a single output stream (stdout), with
severity rendered as a leading bracketed tag:

| `LogLevel` | Leading tag |
|------------|-------------|
| `Error`    | `[ERROR] `  |
| `Warn`     | `[WARN] `   |
| `Info`     | `[INFO] `   |
| `Debug`    | `[DEBUG] `  |

**Uniform `fwrite` emission.** Every byte that reaches stdout
is written by `fwrite(ptr, 1, n, stdout)` with an **explicit
byte count** `n`:

- every **literal run** (the bracketed tag, the author
  template's literal text per `spec.md` §26.7, and the trailing
  `\n` appended by lowering) is written as a single
  `fwrite("<bytes>", 1, <decoded-byte-length>, stdout)` -- the
  count is the decoded byte length (an interior NUL rendered
  `\000` in the C string literal counts as one byte), not the
  escaped C-literal length;
- every **numeric conversion** is rendered into a fixed stack
  buffer with `snprintf` (the §8.2 specifier + cast), then the
  returned length is `fwrite`n;
- every **`&str` value** is written by `fwrite` of its full
  byte range (§8.3).

Adjacent literal text coalesces, so the tag merges into the
leading literal run and the `\n` into the trailing run; one
statement is emitted per piece, in template order, on the one
stdout stream (bytes land in call order).

**Rationale.** A Rust `&str` (and the format-template literal)
is length-prefixed and may contain an interior `\0` (valid
UTF-8). A NUL-terminated `printf` format string and the `%s`
conversion both stop at the first NUL, **silently truncating**
such content. `fwrite` with an explicit byte count reproduces
every byte faithfully, including interior NUL. (This replaces
the earlier single-`printf(fmt, args)` emitter, which carried
that latent truncation.)

### 8.2 Per-arg C specifier

The per-arg specifier is derived from each peeled operand's
RegseqModel `Ty`:

| Operand `Ty`               | C specifier |
|----------------------------|-------------|
| `Ty::Bool`                 | `%d` (lowered through `int`; printed `0` / `1`) |
| `Ty::Uint(UintKind::U8..U64)` | `%u` / `%u` / `%u` / `%llu` per width |
| `Ty::Int(IntKind::I8..I64)`  | `%d` / `%d` / `%d` / `%lld` per width |
| `Ty::Ref(Ty::Str, _)`      | written byte-faithfully via `fwrite` (§8.3); not a `printf`/`snprintf` specifier |

The numeric rows give the `snprintf` format specifier used to
render each numeric operand into the §8.1 stack buffer.
Modifier-specifiers from `spec.md` §26.7 compose with the
per-width specifier (e.g., `{:08x}` on a `U32`-typed operand
lowers to the `snprintf` format `%08x`).

### 8.3 `&str` byte-faithful emission

Rust `&str` is a length-prefixed slice (data pointer + byte
length), not a NUL-terminated C string. The generic-C backend
writes both static and non-static `&str` values with `fwrite`
of the **full** byte range -- exactly `len` bytes -- so the
content is reproduced verbatim, including any interior NUL.
There is **no NUL termination, no stack buffer, no length cap,
and no truncation**.

- **Static literals.** When the `&str` operand traces to a
  static string literal Constant (the `args[0]` template
  case, and any author-provided `Str` argument that resolves
  to a static literal at lowering time), the backend emits the
  CMS bytes as a C string literal and `fwrite`s them with their
  known decoded byte length: `fwrite("<bytes>", 1, <len>,
  stdout)`. An interior NUL renders `\000` in the C literal and
  is counted as one byte; the literal's implicit trailing `\0`
  is irrelevant because `fwrite` writes exactly `len` bytes.
- **Non-static `&str` operands.** For `&str` operands that do
  not trace to a static literal (e.g., author-provided `&str`
  arguments that borrow from a runtime-constructed string), the
  backend emits `fwrite((<operand>).data, 1, (<operand>).len,
  stdout)` against the §3.2 fat-pointer struct
  (`rust_str { const uint8_t *data; size_t len; }`). This
  writes exactly `.len` bytes from `.data` -- the fat pointer
  guarantees `.len` valid bytes, so there is no over-read -- and
  a `&str` of any length prints in full.

(Earlier revisions materialised a NUL-terminated copy in a
fixed-size stack buffer for the non-static case, with a 256-byte
floor and a deterministic truncation-at-`cap-1` contract. That
mechanism is **removed**: a NUL-terminated `%s` silently
truncated content at the first interior NUL, and the cap
truncated long strings. `fwrite` with an explicit byte count
needs neither a buffer nor a cap.)

### 8.4 `max_str_args_per_call` declaration

**Generic-C `max_str_args_per_call = 8`** (counted across
`args[1..]`; the format template at `args[0]` is excluded
from the count). A Log Call site whose post-DiagLogPeel
argument set contains more than 8 `Ty::Ref(Ty::Str, _)`
operands is rejected at bridge-side validation per `spec.md`
§26.8 D8. The bound counts **all** `&str` operands, not only
the non-static ones -- the bridge does not need to trace
operand provenance to decide D8; the count is read directly
off the fingerprint's `arg_tys` field. With the §8.3
byte-faithful `fwrite` emission no per-`&str` stack buffer is
allocated, so the bound is no longer a stack-use limit; it is
now purely a bridge-side argument-count ceiling (a sanity bound
on how many `&str` operands one call site may carry, read off
the fingerprint without provenance analysis). Authors needing
more than 8 `&str` operands at one call site compose multiple
`log!` calls.

### 8.5 Pipeline A consumption

The generic-C backend reads the post-DiagLogPeel flat-arg IR
shape (per `spec.md` §26.6) and emits the §8.1 uniform-`fwrite`
statement sequence over the peeled operands in template order:
numeric operands through §8.2's specifier + cast (rendered via
`snprintf`), `&str` operands and literal runs through `fwrite`
(§8.3 / §8.1). The backend **MUST NOT** reconstruct a `LogArg`
slice or any pre-DiagLogPeel carrier shape; the prohibition is
normative per `spec.md` §26.9 Pipeline A uniformity rule, and
the zero-runtime-cost-on-emit guarantee rests on it.

---

## 9. Worked C-lowering examples

These examples illustrate the live spec. IR-side definitions
are anchored per example. Each example is generic-C /
target-agnostic; phase-2 backends contribute their own
target-specific worked examples to their respective
`<target>-backend.md`.

> **Note.** §§10–17 below pin the normative per-construct rules these
> examples illustrate (Rvalue + operators, casts, `Const` initializers,
> enum representation, aggregate field offsets, declaration order, host
> panic, symbol policy). The §9.8 / §9.9 enum examples are special cases
> of the general rule at §13.

### 9.1 Reference and pointer types

**Rust:**
```rust
let a: &u32        = ...;   // shared borrow
let b: &mut u32    = ...;   // unique borrow
let c: *const u32  = ...;   // raw const
let d: *mut u32    = ...;   // raw mut
```

**Translated (post-bridge):**
```text
a: Ty::Ref(t_u32, Provenance::SharedBorrow)
b: Ty::Ref(t_u32, Provenance::UniqueBorrow)
c: Ty::Ptr(t_u32, Mutability::Not)
d: Ty::Ptr(t_u32, Mutability::Mut)
```

**C:**
```c
const uint32_t *a;            // SharedBorrow
uint32_t       *b;            // UniqueBorrow (codegen MAY add `restrict` in declaration contexts)
const uint32_t *c;            // raw const
uint32_t       *d;            // raw mut
```

### 9.2 Fat-pointer types: `&str`, `&[T]`, `&dyn Trait`

**Rust:**
```rust
let s: &str = "hello";
let xs: &[u32] = &[1, 2, 3];
let d: &dyn Driver = &my_driver;
```

**C:**
```c
typedef struct rust_str { const uint8_t *data; size_t len; } rust_str_t;

typedef struct rust_slice_u32 {
    const uint32_t *data;
    size_t len;
} rust_slice_u32_t;

typedef struct trait_obj_Driver {
    void *data;                          /* unqualified per `spec.md` §17.2.3 trait-object dispatch carve-out */
    const Driver_VTable *vtable;
} trait_obj_Driver_t;

static const uint8_t __str_lit_0[] = { 'h', 'e', 'l', 'l', 'o' };  // no NUL
static const rust_str_t HELLO = { __str_lit_0, 5 };
```

### 9.3 Niche-encoded enum: `Option<&u32>`

**Rust:**
```rust
let p: Option<&u32> = ...;   // 8 bytes; niche-encoded (None = 0, Some(p) = p)
```

**C:**
```c
typedef struct Option_ref_u32 {
    const uint32_t *p;   // 0 = None; nonzero = Some(p)
} Option_ref_u32_t;
```

### 9.4 `UnsafeCell` aliasing

**Rust:**
```rust
let x: &UnsafeCell<u32> = ...;
```

The `is_unsafe_cell` flag on the `UnsafeCell<u32>` `Item`
triggers `const`-drop on enclosing shared references.

**C:**
```c
uint32_t *x;   // NOT const, despite the source-side &T
```

### 9.5 Packed struct

**Rust:**
```rust
#[repr(packed)]
struct Frame {
    kind: u8,
    addr: u32,
}
```

**Layout:** `pack: Some(1)`, `size: 5`, `align: 1`.

**C:**
```c
struct Frame {
    uint8_t  kind;
    uint32_t addr;
} __attribute__((packed));
```

### 9.6 `&dyn Driver` lowering

**Rust:**
```rust
fn process(d: &dyn Driver) -> u32 { d.read() }
```

**C:**
```c
/* Vtable type: erased-receiver fn-pointer fields. Per `spec.md` §13.2,
   all receiver pointers are uniformly `*mut ()` (`void *`) regardless of
   the source borrow flavor -- see the trait-object dispatch carve-out
   in `spec.md` §17.2.3 for the UnsafeCell soundness rationale. */
typedef struct Driver_VTable {
    void (*__drop_in_place)(void *self);
    size_t __size;
    size_t __align;
    uint32_t (*read)(void *self);
} Driver_VTable_t;

/* Trait object: data field is unqualified `void *` per the
   trait-object dispatch carve-out (`spec.md` §17.2.3). The Rust
   SharedBorrow flavor is preserved at the type-system level
   but the C-level `const` is dropped at the erased boundary. */
typedef struct trait_obj_Driver {
    void *data;
    const Driver_VTable_t *vtable;
} trait_obj_Driver_t;

/* Method shim (`spec.md` §13.2). */
static uint32_t Driver_read__shim_PL011(void *self) {
    return PL011_read((PL011_t *)self);
}

/* Drop-in-place shim (`spec.md` §13.2). */
static void Driver__drop_shim_PL011(void *self) {
    PL011_drop_in_place((PL011_t *)self);
}

/* Vtable instance: both drop slot and method slot use shims. */
extern const Driver_VTable_t __vtable_PL011_for_Driver;
const Driver_VTable_t __vtable_PL011_for_Driver = {
    .__drop_in_place = Driver__drop_shim_PL011,
    .__size          = sizeof(PL011_t),
    .__align         = _Alignof(PL011_t),
    .read            = Driver_read__shim_PL011,
};

uint32_t process(trait_obj_Driver_t d) {
    return d.vtable->read(d.data);
}
```

**General rule (the vtable task generalised this worked example).** The vtable
*type* is the bridge-synthesised `vtable_ty` `Ty::Struct` itself -- emitted under
its own (discovered) name via the ordinary struct path; the trait-object fat
pointer's `vtable` field points at it **by that name** (not a separate `_VTable`
alias). Its fields follow rustc's `vtable_entries` order with bridge-derived
names: `__drop_in_place` (a `*mut ()`, null when the impl type needs no drop),
`__size` / `__align` (`size_t`), then one fn-pointer field per method by slot
index (the field name is the method name; a `<method>#<idx>` suffix breaks a
collision), then any supertrait-vptr / vacant slots (a vacant slot holds an
**uninit** cell, per rustc's `vtable_allocation_provider`). The `VTableInstance`
**static** is the ordinary `Const::Aggregate` of those slots (a `Const::Reloc`
to each shim, `sizeof` / `_Alignof`, null / uninit). Each method/drop **shim**
is an ordinary emitted function whose receiver is erased to `*mut ()` at the C
boundary (so every impl's vtable shares the layout); rustc's `VtableShim` body
forwards the (reinterpreted) erased receiver to the concrete method (the §15
`__abi` prologue marshalling reconciles the erased boundary receiver with the
concrete body receiver). A virtual call decomposes to `d.vtable->m(d.data, ..)`
(the bridge rewrites it to a `CallCallee::Indirect` through the loaded slot).

An `&T as &dyn Tr` **unsize** builds the §3.2 fat pointer `{ .data = (void *)src,
.vtable = &<the VTableInstance static> }` directly (a `Ty::Ref` value -- not a
`RawPtr` aggregate); the backend resolves the concrete impl's `VTableInstance`
from the dyn principal (recovered from `vtable_ty`) plus the source `Self` type.

### 9.7 Closure lowering

**Rust:**
```rust
struct Token { handle: u32 }
impl Drop for Token {
    fn drop(&mut self) { /* release the handle; details elided */ }
}
impl Token {
    fn into_handle(self) -> u32 {
        let h = self.handle;
        core::mem::forget(self);
        h
    }
}

fn make_consumer(t: Token) -> impl FnOnce() -> u32 {
    move || t.into_handle()
}

let c = make_consumer(Token { handle: 42 });
let value = c();
```

**C:**
```c
typedef struct Token { uint32_t handle; } Token_t;

void Token_drop(Token_t *self) {
    /* release the handle */
}

uint32_t Token_into_handle(Token_t self) {
    uint32_t h = self.handle;
    /* core::mem::forget suppresses Drop; no Token_drop call here. */
    return h;
}

typedef struct closure_make_consumer_0 {
    Token_t t;
} closure_make_consumer_0_t;

uint32_t closure_make_consumer_0__call_once(closure_make_consumer_0_t self) {
    return Token_into_handle(self.t);
}

void closure_make_consumer_0__drop(closure_make_consumer_0_t *self) {
    Token_drop(&self->t);
}

closure_make_consumer_0_t make_consumer(Token_t t) {
    return (closure_make_consumer_0_t){ .t = t };
}

closure_make_consumer_0_t c = make_consumer((Token_t){ .handle = 42 });
uint32_t value = closure_make_consumer_0__call_once(c);
```

### 9.8 `repr(u32)` enum with user-assigned tags

**Rust:**
```rust
#[repr(u32)]
pub enum Status {
    Idle   = 0,
    Active = 1,
    Error  = 0xFF,
    Reset  = 0xDEAD,
}

fn priority(s: Status) -> u32 {
    match s {
        Status::Idle   => 0,
        Status::Active => 1,
        Status::Error  => 100,
        Status::Reset  => 200,
    }
}
```

**Layout:** `EnumShape::Multiple` with `TagEncoding::Direct`,
`tag.scalar_ty: Uint(U32)`,
`valid_range: WrappingRange { start: 0, end: 0xDEAD }`. Each
`VariantLayout` carries `discriminant` (semantic) and
`tag_value: Some(discriminant)` (Direct encoding writes the
discriminant verbatim).

**C:**
```c
typedef struct Status {
    uint32_t tag;
} Status_t;

#define STATUS_IDLE   0u
#define STATUS_ACTIVE 1u
#define STATUS_ERROR  0xFFu
#define STATUS_RESET  0xDEADu

uint32_t priority(Status_t s) {
    switch (s.tag) {
        case STATUS_IDLE:   return 0;
        case STATUS_ACTIVE: return 1;
        case STATUS_ERROR:  return 100;
        case STATUS_RESET:  return 200;
    }
}

// SetDiscriminant(Status::Error) -> place->tag = STATUS_ERROR;
//   codegen reads VariantLayout.tag_value = Some(0xFF) and emits
//   0xFF as the tag value, NOT 2 (variant index).
```

### 9.9 Niche-encoded `Option<&u32>` with discriminant fields

**Rust:**
```rust
fn f(x: Option<&u32>) -> u32 {
    match x {
        Some(p) => *p,
        None    => 0,
    }
}
```

**Layout:** `EnumShape::Multiple` with `TagEncoding::Niche {
untagged_variant: Some, niche_variants: None..=None,
niche_start: 0 }`.

| Variant | `discriminant` | `tag_value` |
| ------- | -------------- | ----------- |
| `None`  | `0`            | `Some(0)`   |
| `Some`  | `1`            | **`None`**  |

`tag_value: None` for `Some` because `Some` is the untagged
variant -- the inner pointer's bit pattern IS the tag.

**C:**
```c
typedef struct Option_ref_u32 {
    const uint32_t *p;   // 0 = None; nonzero = Some(p)
} Option_ref_u32_t;

uint32_t f(Option_ref_u32_t x) {
    if (x.p == NULL) return 0;        // None
    return *x.p;                      // Some
}

// SetDiscriminant(Option::None) -> place->p = (const uint32_t *)0;
//   /* tag_value = Some(0) */
// SetDiscriminant(Option::Some) -> /* no-op */
//   /* tag_value = None: untagged variant; payload IS the tag. */
// Construction of Option::Some(p) -> place->p = p;
//   /* Separate MIR field-write statement, NOT a SetDiscriminant. */
```

---

## 10. `Rvalue` C-emission

IR-side definition: `spec.md` §21. `Rvalue` appears as the RHS of
`Assign` (§7). Each variant lowers as:

- `Use(op)` → the operand expression (§5).
- `Repeat(op, n)` → array fill into the destination
  (`for (size_t i = 0; i < n; i++) dst[i] = <op>;`); `n` is the
  post-mono constant.
- `Ref(_, place)` / `RawPtr(_, place)` → `&<place>` (§4); for an unsized
  pointee the result is the §3.2 fat pointer built per §11 `Unsize`.
- `Cast(k, op, ty)` → §11.
- `BinaryOp(op, a, b)` → the BinOp table below.
- `UnaryOp(op, a)` → `Not`: `!a` for `bool`, `~a` for integers; `Neg`:
  `-a`; `PtrMetadata`: the §3.2 fat-pointer metadata slot (`.len` /
  `.vtable`).
- `Discriminant(place)` → §13 (read the tag / niche) when the place's type
  is an enum; for a **non-enum** predecessor type the result is the
  constant `0` (`spec.md` §21.2.8).
- `Aggregate(kind, ops)` → struct / union / tuple / array compound literal
  with field placement per §14; enum per §13; closure → captures-struct
  compound literal (§9.7); `AggregateKind::RawPtr` → build the §3.2 fat
  pointer from `(data, metadata)`.

### 10.1 `BinOp`

The host toolchain **MUST** be configured for two's-complement wrapping of
signed `+`, `-`, `*` (e.g. `-fwrapv`); `-fwrapv` does **not** cover shift
UB, which §10.1 lowers explicitly.

| `BinOp` | C emission |
|---------|-----------|
| `Add` / `Sub` / `Mul` | `a + b` / `a - b` / `a * b` (wrapping) |
| `AddUnchecked` / `SubUnchecked` / `MulUnchecked` | same operators (defined-wrap is a sound superset of the IR's UB-on-overflow) |
| `AddWithOverflow` / `SubWithOverflow` / `MulWithOverflow` | `__builtin_{add,sub,mul}_overflow(a, b, &tmp)` → `struct { T v; _Bool overflow; }` (the builtin's return is `overflow`, `tmp` is `v`), matching the IR's `(T, bool)` |
| `Div` / `Rem` | `a / b` / `a % b` — sound because rustc always emits the guarding `Assert(DivisionByZero)` and `Assert(Overflow)` (INT_MIN/-1) terminators upstream; the bare op never sees the UB inputs |
| `BitXor` / `BitAnd` / `BitOr` | `a ^ b` / `a & b` / `a | b` |
| `Shl` (wrapping) | `(T)((U)a << (b & (BITS-1)))` — compute in the unsigned same-width type `U`, count masked |
| `Shr` (wrapping) | `a >> (b & (BITS-1))` — **arithmetic** for signed `a`, **logical** for unsigned `a` (host arithmetic right shift is a pinned host assumption) |
| `ShlUnchecked` / `ShrUnchecked` | as `Shl` / `Shr` but with the count **unmasked** (caller guarantees `b < BITS`) |
| `Eq` / `Ne` / `Lt` / `Le` / `Ge` / `Gt` | `a == b`, … → `_Bool` |
| `Cmp` | three-way `core::cmp::Ordering` as `i8`: `(int8_t)((a > b) - (a < b))` (−1 / 0 / 1) |
| `Offset` | `a + b` (C pointer arithmetic; `<*const T>::offset` semantics; `b` is `isize`) |

---

## 11. Cast C-emission (`CastKind` / `PointerCoercion`)

IR-side definition: `spec.md` §21.3.3.

| `CastKind` | C emission |
|------------|-----------|
| `IntToInt` / `FloatToFloat` / `IntToFloat` | `(T)x` (two's-complement integer narrowing is a pinned host assumption) |
| `FloatToInt` | **saturating helper, never a bare cast** (C is UB out-of-range; Rust `as` saturates, NaN→0). Per source-float × dest-int prelude helper: `if (isnan(x)) return 0; if (x <= (F)DI_MIN) return DI_MIN; if (x >= (F)DI_MAX) return DI_MAX; return (DI)x;` (bounds checked *before* the cast; ±inf handled by the comparisons) |
| `PtrToPtr` | `(T*)p` |
| `FnPtrToPtr` | `(T*)p` — function↔object pointer round-trip is a pinned **host-target assumption** (POSIX-guaranteed, not ISO C) |
| `PointerExposeProvenance` | `(uintptr_t)p` |
| `PointerWithExposedProvenance` | `(T*)n` |
| `Transmute` | bit reinterpret via `memcpy` between equal-`sizeof` objects (no union pun): `memcpy(&dst, &src, sizeof dst)` |
| `Subtype` | no-op (representation-identical; emit the operand) |

`PointerCoercion`:

- `Unsize` → build the §3.2 fat pointer: slice / array → `{ data, len }`;
  `str` → `{ data, len }`; `dyn` → `{ data, vtable = &<VTableInstance> }` (§9.6;
  the concrete impl's vtable static, resolved from the dyn principal + source
  `Self`). An already-fat `&dyn → &dyn` source is an identity copy.
- `ReifyFnPointer` → `&<fn>` (function designator → fn pointer).
- `ClosureFnPointer` → `&<the closure's `CallOnce` shim>` for the non-capturing
  closure. The bridge recovers the closure at the cast site and lowers the
  (otherwise type-erased `Const::ZST`) operand to a `Const::Reloc` to the
  `CallOnce` `ClosureCall` shim, whose by-value zero-sized receiver is elided at
  the C boundary so its signature is the bare `fn(..) -> ..`; this then shares
  the `ReifyFnPointer` `&<fn>` designator path.
- `UnsafeFnPointer` → no-op (safe→unsafe fn-ptr is C-identical).

`PtrMetadata` of a fat pointer reads its second slot: `.len` for slice / str,
`.vtable` for `dyn`. A `PtrToPtr` cast of a fat (`dyn`) source to a thin pointer
takes the `.data` slot (not the whole fat struct).

---

## 12. `Const` → C initializers

IR-side definition: `spec.md` §9.7 (the closed 11-variant `Const` +
3-state `Byte`). Reached via `Operand::Constant` (§5),
`InstanceKind::Static.init`, and `InstanceKind::Const.value`. Every form
below is materialisable both at value sites and as a static / `const`
initializer.

| `Const` | C initializer |
|---------|--------------|
| `Bool(b)` | `1` / `0` |
| `Char(u)` | `(uint32_t)<u>` |
| `Int(k, v)` | width-suffixed signed literal of the value (`v` is the sign-extended bit pattern; `LL` as needed; `I128` via `__int128` halves, host-dependent) |
| `Uint(k, v)` | unsigned literal (`U` / `ULL`) |
| `Float(k, bits)` | a **constant-expression** that is exact + bit-faithful (incl. NaN payloads): finite → C99 **hex-float literal**; ±∞ → `__builtin_inff()` / `__builtin_inf()`; NaN → `__builtin_nanf("0x<payload>")` (quiet) / `__builtin_nansf(...)` (signaling). These are GCC/Clang constant expressions (the primitives behind `<math.h>` `INFINITY` / `NAN`); a host-target assumption. **Not** a union member access (which is not a constant expression). `_Float16` / `_Float128` host-dependent. |
| `ZST` | no bytes — value position: no-op / `(void)0`; aggregate field: omitted (§14) |
| `Bytes(Vec<Byte>)` | reconstructed as a **typed** initializer for the consumer `Ty` (byte-image walker): scalars decode `size_of` bytes per `target.endian` into the matching literal (`Uninit`→`0`); `Tuple` / `Struct` / `Array` slice per `FieldLayout.offset` (source order, ZST omitted) and recurse; a complete pointer-width `Reloc(r)` run → `&<r>`; a **no-relocation** enum byte image initializes the §13 `_raw` member (the inline enum's promoted `_raw` or the multi-view `u._raw`), while a multi-view **tagged** (`Direct`) image carrying a `Reloc` routes the tag + relocation through the §13 `_c<idx>` typed full view (the reloc in a typed pointer slot). A `ConstMemoryStatic` is declared as its consumer **pointee storage type** (re-typing, §13 note), so `&X` is the right type + naturally aligned (a flat `unsigned char[]` is the special case of a `[u8; N]` consumer). A `Ty::Union` byte image initializes the byte-covering `_raw` union member (§14 Unions), `(union U){ ._raw = { <bytes> } }`, type-punned by an in-place member read; a `Byte::Reloc` in a union image is a Phase-1 limitation (a `char[]` cannot hold an address constant). |
| `Aggregate(children)` | dual role: the `[Reloc, Uint(meta)]` **reference carrier** over `Ty::Ref` (a sized pointee → the reloc designator cast to the pointer type; `Ty::Str` / `Ty::Slice` → the §3.2 `rust_str` / `rust_slice_<T>` fat struct, `.data` via the mutability-correct slice-data pointer type, `.len = (size_t)<meta>`) vs the typed valtree aggregate over `Tuple` / `Struct` / non-`u8` `Array` in §14 field order (a `Ty::Union` valtree is unsupported — `Const::Aggregate` carries no active field) |
| `EnumValue { variant_idx, payload }` | §13: a multi-view value serializes the tag + a **no-relocation** payload (scalar / aggregate / array / `Bytes` / `NullPtr` / nested `EnumValue`, recursively) to a `layout.size` byte image written into the `_raw` union member; a multi-view **tagged** (`Direct`) value whose payload carries a **relocation** routes the tag + reloc through the `_c<idx>` typed full view (`(struct E){ .u._c<v> = { .tag = <tv>, .<f> = &Y } }`); a multi-view **niche-untagged** value (no tag to co-write) instead initializes the existing `u._v<untagged>` typed view (so a relocation payload lands in a typed pointer field); an inline value uses the typed single view (its untagged payload / single tag member), or — for a **niched** variant — the promoted `_raw` byte image (so a bool-niche or a niche cell buried in a nested aggregate round-trips with no C-type normalization). A relocation payload of a niche-**tagged** (non-untagged) variant remains an external-rustc-invariant defensive fail-loud (rustc niche-fills only fieldless variants) |
| `NullPtr(ty)` | `(T*)0` |
| `Reloc(InstanceRef)` | `&<emission_name>` of the referenced instance |

---

## 13. Enum C representation

IR-side definition: `spec.md` §11 (`Layout` / `EnumShape` / `TagEncoding`).
Generalises the §9.8 (Direct, fieldless) and §9.9 (Niche, single-pointer)
examples to **all** enum shapes.

**Representation — offset-faithful (union-of-views).** An enum lowers to a
C `struct` built by the §14 field-offset technique (NOT a naive
`{ tag; union; }`, which falsely assumes tag-at-0 + contiguous payload).
The tag cell and each variant's payload are emitted as **full-object
views** — `struct`s whose fields sit at their **absolute** offsets from the
enum object's start, so a field's offset within its view *is* its absolute
offset (rustc already placed it at a naturally-aligned position, so **no
view needs packing**). All views share **one** `union`, wrapped in the
outer enum `struct` forced to `__attribute__((aligned(layout.align)))`:

- a **tag view** — present iff `Single{scalar:Some}` or `Multiple{Direct}`
  (**not** `Multiple{Niche}`) — holds `<TagScalar.scalar_ty>` at
  `TagScalar.offset`;
- a **variant view** per variant with ≥ 1 non-ZST field (fieldless variants
  place no bytes → no view);
- if exactly **one** view results, it is wrapped in an **anonymous**
  `union { struct { <view fields> }; unsigned char _raw[size]; }` —
  collapsing the §9.8 fieldless `Direct` enum to a tag-only struct and the
  §9.9 niche `Option<&T>` to a single-field struct, while gnu11 **member
  promotion** keeps the runtime access path bare (`e.<field>`, the access
  path **unchanged** from a plain inline struct) and the byte-covering `_raw`
  sibling serves the inline const path (below). The `_raw` name is
  collision-proof against the promoted view field names; **zero** views (an
  `Empty` enum, or a `Single{scalar:None}` whose sole variant is fieldless) →
  a size-faithful (possibly empty) `struct` with no `_raw`.

`_Static_assert(sizeof==size)`, `_Static_assert(_Alignof==align)`, and a
per-field `_Static_assert(offsetof(E, <view>.<field>)==<absolute offset>)`
(nested member designator — a gcc/clang host assumption) validate every
placement. Niche enums carry no tag view — the niche cell is a payload field
*in a variant view* whose bit pattern *is* the tag.

Every **multi-view** enum's shared `union u` also carries a full-object
`unsigned char _raw[size]` member, used **only** to const-initialize a
value-position enum const (§12 `EnumValue` / `Bytes`): a C designated
initializer cannot set two members of the same `union` (`.u._tag` and
`.u._v<v>` share offset 0 — a later union designator overrides the earlier),
so a fully-known **no-relocation** value is written as
`(struct E){ .u._raw = { <bytes> } }`. A `char[]` `_raw` cannot hold an
address constant, so a multi-view **tagged** (`Direct`) value whose payload
carries a **relocation** instead uses a **const-init-only** typed full-object
view `_c<idx>` — `struct { <tag>; <payload fields…>; }` at **absolute**
offsets, one per payload-bearing `Direct` variant, with its synthetic `tag`
sub-field name collision-proofed against the payload field names — so the tag
and the relocation go through **one** union designator with the reloc in a
typed pointer slot (`(struct E){ .u._c<v> = { .tag = <tv>, .<f> = &Y } }`).
Runtime reads stay **in-place** typed-view accesses (`x.u._tag.tag`,
`x.u._v<v>.<f>`) — gnu11 `union` type-punning, valid because the read never
takes the address of an inactive member and dereferences a typed pointer
later; the runtime navigator is **unchanged** (`_c<idx>` is written only by
the const path). Both `_raw` and each `_c<idx>` are layout-neutral: the views
are already full-object size, `_raw` has alignment 1, and the size / align
`_Static_assert`s are unchanged.

**Why not a single tag field + payload `union`?** A literal "tag field at
`TagScalar.offset` + a payload `union` placed at some `union_start` (fields
at `offset − union_start`)" is **not C-placeable** for shifted-alignment
`Direct` enums: `enum E { A(u8), B(u32) }` lays out tag @0, `A.0` @1,
`B.0` @4 — a C `union` containing a `uint32_t` has alignment 4 and **cannot
sit at the first payload byte (offset 1)**, and `B.0` at `4 − 1 = 3` would be
misaligned. The union-of-views uses **no `union_start`**: every field is
placed at its absolute (naturally-aligned) offset within a view, and the
`union` aliasing reproduces the exact byte layout.

**Tag operations.**

- `Discriminant(place)` — `Empty`: unreachable. `Single`: the single
  variant index (constant). `Multiple` (both `Direct` **and** `Niche`):
  read the tag / niche cell at `TagScalar.offset` and **compare against
  each variant's precomputed `VariantLayout.tag_value`**; the match selects
  the variant. Using the bridge-precomputed `tag_value` (which already
  encodes the **mod-2^scalar_width** storage and any **wrapping**
  `WrappingRange`) avoids re-deriving the niche formula. For `Niche`, the
  niched variants carry `Some(tag_value)` and the untagged variant carries
  `None`; **no match → `untagged_variant`**.
- `SetDiscriminant(place, v)` — `Direct` / `Single{scalar:Some}`: store
  `VariantLayout[v].tag_value` (`Some` by construction) into the tag cell.
  `Niche` niched variant: store its `tag_value`. `Niche` untagged variant:
  **no-op** (`tag_value: None`; payload is the tag). `Single{scalar:None}`:
  no-op.
- `Downcast(v)` then `Field(i, _)` → access variant `v`'s payload field `i`
  at its `FieldLayout.offset` (§14 access).
- `Aggregate::Enum(ty, v)` → write the active variant's payload fields at
  their offsets + the tag write (as `SetDiscriminant(v)`), as sequential
  statements through the typed views.
- `Const::EnumValue` (a value-position const) → an **inline** enum uses the
  typed single view (its untagged payload / single tag member); a **niched**
  inline variant serializes the niche value + any payload to the promoted
  `_raw` byte image (so a bool-niche or a niche cell buried in a nested
  aggregate round-trips with no C-type normalization). A **multi-view** enum
  cannot set the `_tag` and `_v<v>` union members in one initializer, so a
  fully-known value is **usually** serialized to a byte image and written into
  the `_raw` union member (`(struct E){ .u._raw = { ... } }`) — the tag plus a
  **no-relocation** payload (scalar / aggregate / array / `Bytes` / `NullPtr` /
  nested `EnumValue`, serialized recursively). **Except**: a multi-view
  **niche-untagged** value (no `tag_value` to co-write) initializes the
  existing `u._v<untagged>` typed view (`(struct E){ .u._v<untagged>.<f> = ... }`),
  letting a **relocation** payload land in a typed pointer field; and a
  multi-view **tagged** (`Direct`) value whose payload carries a **relocation**
  uses the `_c<idx>` typed full view (`(struct E){ .u._c<v> = { .tag = <tv>,
  .<f> = &Y } }`). A relocation payload of a niche-**tagged** (non-untagged)
  variant is an external-rustc-invariant defensive fail-loud (rustc niche-fills
  only fieldless variants). **The runtime representation is unchanged — the
  `_raw` / `_c<idx>` members are const-init-only.**

A readability optimisation (a named `{ tag; union; }` when the layout is
provably tag-prefix-compatible) is **deferred** — not Phase 1.

---

## 14. Aggregate field-offset reproduction

IR-side definition: `spec.md` §11. `FieldLayout` / `VariantLayout` field
vectors are in **source / MIR field order, not memory order**; `repr(Rust)`
reorders, so verbatim field emission does **not** reproduce `offset`. Every
aggregate (struct, union, tuple, enum payload, closure captures) is emitted
by:

1. Sort the emittable fields by `FieldLayout.offset` ascending.
2. **Omit ZST fields** but account for them: `PhantomData`
   (`is_phantom_data`, `spec.md` §14.5 `CFieldAdmissible`) and unit / ZST
   fields occupy 0 bytes; multiple same-offset ZSTs collapse.
3. Insert explicit `unsigned char _padN[k];` fillers to advance from the
   previous field's end to the next field's `offset`, and a trailing pad to
   `layout.size`.
4. Carry alignment: force the aggregate's C **type** alignment to
   `layout.align` via `__attribute__((aligned(layout.align)))` (`_Alignas` is
   **not** valid on a `struct`/`union` *tag definition* — it specifies
   objects/members, not a type). `pack: Some(_)` adds `packed` →
   `__attribute__((packed, aligned(layout.align)))`. The alignment is
   **always `layout.align`, never `pack`'s `n`** — under `repr(packed(n))` the
   true type alignment is `min(max_field_align, n) = layout.align`, which can
   be `< n`; the step-6 `_Alignof` guard asserts it.
5. **Unions** (`LayoutKind::Union`): all field offsets are 0 → a C `union`
   of the fields, plus an always-emitted byte-covering
   `unsigned char <raw>[size]` member (mirroring the §13 enum `_raw`). The
   `_raw` member spans `layout.size` (so it pins `sizeof`, subsuming the old
   conditional trailing `_pad` filler) and is const-init-addressable, so a
   `Const::Bytes` over the union initializes every byte through one designator
   (`(union U){ ._raw = { ... } }`, §12). Its name is **collision-proof**
   against the union's real (sanitized) field names (a field literally named
   `_raw` yields `_raw0` / ...); the body side spells the init designator only
   through the same source-of-truth lookup. Layout-neutral: `_raw` is
   alignment 1 and full-object size, so the union's `aligned` + size / align
   `_Static_assert`s are unchanged. Runtime reads stay in-place typed-member
   accesses (gnu11 `union` type-punning).
6. Emit `_Static_assert(sizeof(T)==size)`,
   `_Static_assert(_Alignof(T)==align)`, and per-field
   `_Static_assert(offsetof(T,f)==offset)` guards.
7. **Field access** (§4 `.fname`): direct member access for the natural
   case; `memcpy` to / from a typed temporary when accessing through enum
   byte storage or a packed / misaligned field (avoids alignment UB).

---

## 15. Declaration order, signatures & C-ABI prototypes

IR-side definition: `spec.md` §8 / §9 (items / instances), §14 (`FnSig` /
`FnAbi` / §14.5 C-ABI). Phase 1 emits a **single translation unit**; module
hierarchy lives only in the `DefPath`-qualified names (§7) so a later
multi-file split is rename-free. The TU is emitted in five ordered banks:

1. **Prelude** — fixed runtime header: includes (`<stdint.h>`, `<stddef.h>`,
   `<stdio.h>`, `<stdlib.h>`, `<string.h>`), the §3.2 fat-pointer typedefs,
   the §16 panic routines, and the §10 / §11 `*WithOverflow` /
   saturating-cast helpers.
2. **Type forward-decls** — tag forward-decls for every aggregate in F4
   order: `struct <T>;` for structs / tuples / enums / closures,
   `union <T>;` for unions.
3. **Type definitions** — full `struct` / `union` bodies (enums lower to
   structs per §13), **topologically sorted by by-value containment**: a
   type that contains another *by value* is emitted after it (C requires a
   complete type before by-value use — fields, array elements, by-value
   params/returns, `sizeof`/`offsetof` for the §14 `_Static_assert`
   guards). The containment graph (the **only** load-bearing emission
   order; everything else is forward-declared) is, for an aggregate `T`:

   - `T → U` iff `U` is the resolved `Ty` of any `FieldLayout.ty` of `T`'s
     struct / union / tuple fields, any enum `VariantLayout` payload field,
     or any closure capture field; **or** `U` is the **element type of a
     `Ty::Array`** reached that way (C `U[N]` needs `U` complete). Recurse
     through these edges.
   - **No** edge through `Ty::Ref` / `Ty::Ptr` / `Ty::FnPtr` (forward-decls
     in bank 2 break these — pointers to incomplete types are legal) or
     `Ty::FnDef` (a ZST; not emitted).
   - **Fat-pointer exception (the lowered struct, not the pointee).** A
     `Ref`/`Ptr` to an *unsized* pointee lowers to a **by-value** fat struct
     (§3.2): a containing aggregate therefore edges to that fat struct
     (`rust_slice_<T>` / `trait_obj_<Trait>`; `rust_str` is in the prelude).
     The fat struct's `data` slot is a pointer, so it normally needs only a
     forward-decl of the element — **except** for a slice whose element is a
     `Ty::Array(base, N)`: `data` is then `base (*)[N]`, and a C **array type
     needs its element complete even behind a pointer**, so the fat struct
     edges (ordering, complete-before) to `base`'s aggregate. (Discovery —
     naming + forward-decl — always edges to the element aggregate; only the
     `Ty::Array` element case adds the *ordering* edge. A genuinely cyclic
     array-element slice is not C-representable and is a hard emission error.)

   This is derivable purely from `FieldLayout.ty` + the `Ty` interner (no
   recomputation). F4 / instance kind-rank order is the deterministic
   **tie-breaker** only. By-value cycles are impossible for sized Rust
   types (rustc rejects them); a detected cycle is a hard error (model
   corruption).
4. **Declarations** — function prototypes for every function instance
   (including foreign `body: None`); and `extern` declarations for **every**
   static — both foreign (`init: None`) and locally-defined statics that may
   be referenced before their bank-5 definition (declare-before-use in the
   single TU: `extern [const] T X;` here, defined in bank 5).
5. **Definitions** — function bodies + static initializers, F4 order.

**C-ABI prototypes.** A function with `FnSig.abi == C` emits its signature
from the **C-ABI lowered** types: read `sig_c_abi_lowerings[sig]` (§14.5);
a position with `CAbiShape::Substituted { emit_as, .. }` emits `emit_as`'s
C type (not the original `FnSig` `TyId`); `Direct` uses the position's own
type. The backend **never re-derives** repr facts (they are not in the IR).
Nested `FnPtr` declarators recurse through the same table.

**C variadics.** `FnSig.c_variadic == true` (implies `abi == C` + ≥ 1 fixed
input) emits the fixed parameters followed by a trailing `, ...` in both the
prototype (bank 4) and the definition (bank 5).

**Diag-log exclusion.** An instance whose source `item` carries
`IntrinsicKind::Log(..)` is a recognition tag (§7.1, §8): it is **skipped**
in banks 4 / 5 — no prototype, no definition.

---

## 16. Host panic & `AssertKind` messages

IR-side definition: `spec.md` §23 (`Assert` / `AssertKind` /
`Unreachable`). The prelude (§15 bank 1) defines a `_Noreturn` panic
routine that prints to **stderr** (keeping the §8 diag-log **stdout** oracle
clean) and `abort()`s:

```c
_Noreturn static void regseq_panic(const char *msg) {
    fprintf(stderr, "panic: %s\n", msg);
    abort();
}
```

`Assert` failure (§7.1) and `Unreachable` route here; `Unreachable` prints
`"internal error: entered unreachable code"`. Each `AssertKind` prints a
fixed Rust-style message (operands formatted by their `Ty`):

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

A release-mode `Unreachable` MAY instead emit `__builtin_unreachable()`;
the Phase-1 default is the trap (fail loud for the host oracle).

---

## 17. Symbol & linkage policy

IR-side definition: `spec.md` §8 (`Item` linkage / section / inline
fields), §9 (`InstanceKind`).

- **`Linkage`**: `Default` → ordinary file-scope definition **without**
  `static` (cross-instance refs exist within the one TU); `NoMangle` /
  `export_name` → definition under the §7-derived external symbol
  (`sanitize(emission_name)`, **no** generic suffix -- not the raw
  `emission_name`); `Foreign` → **declaration only** (`extern`, bank 4),
  never defined; `Weak` → `__attribute__((weak))` on the definition.
- **`link_section: Some(s)`** → `__attribute__((section("<s>")))` on the
  definition.
- **`inline_hint`**: `Hint` → `inline`; `Always` → `inline
  __attribute__((always_inline))`; `Never` → `__attribute__((noinline))`;
  `None` → nothing.
- **`Static { mutability }`**: `Not` → `const`-qualified global; `Mut` →
  non-const global. `init: Some` → with initializer (§12); `init: None` →
  `extern`, no initializer.
- **`Const` item**: no standalone C symbol by default — materialised at use
  sites (§12). A `static const` is emitted only if a `Reloc` takes its
  address.
- **Function instances**: `body: Some` → prototype (bank 4) + definition
  (bank 5); `body: None` → prototype / `extern` only.
- **Shim flavors** (`FunctionInstanceFlavor`): reachable flavors all emit
  as ordinary C functions from their bridge-synthesised `body` (no
  per-flavor special case). `virtual_idx: Some(i)` participates only in
  **vtable** construction — slot `i` is initialised to `&<shim
  emission_name>` (§9.6), where `<shim emission_name>` is shorthand for the
  shim's **derived** final symbol per the §7 identifier-emission policy (the
  linkage-aware `sanitize` / `generic_suffix` rule, not the raw
  `emission_name`); the actual vtable-slot emission lands with the deferred
  `dyn`/vtable task. Coroutine / async flavors are unreachable
  (`spec.md` §15.1).
- `is_unsafe` / `FnSig.safety` are **not** lowered (no C equivalent).
