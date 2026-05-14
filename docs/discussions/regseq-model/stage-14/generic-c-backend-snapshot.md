# Stage 14 generic-C backend spec snapshot

> **Snapshot at close of stage 14 (2026-05-13).** This file is
> frozen as a historical artifact preserving the stage-14
> contribution. The current generic-C backend spec lives in
> top-level `generic-c-backend.md`.

Stage-14 working snapshot for the new `generic-c-backend.md`
top-level merger artifact introduced by **D-14-10** (generic-C
backend codegen carve-out). Co-equal with `spec-snapshot.md`
(producer-side IR structure) and `pretty-printer-snapshot.md`
(consumer-side rendering surface); this file holds the
backend-side C-emission surface.

Audit-shape decision text for D-14-10 lives in
`discussions.md`. This file is **relocation content** -- the
sections below are lifted from `spec-snapshot.md` and were
originally specified in earlier stages (stages 3, 5, 6, 7, 11,
14 per per-section provenance). Their original audit trails
remain in those stages' `discussions.md`; D-14-10 changes the
canonical-spec location only, not the content. Linux-flavored
content was dropped per "no forward-looking placeholders" and
filed as inbound expectations to phase-2 stages 15 / 16 / 17;
see `stage-15/expectations.md`, `stage-16/expectations.md`,
`stage-17/expectations.md`.

---

## 1. Mission and scope

This file specifies how RegseqModel constructs lower to C in
the **generic-C backend** (phase-1 implementation territory:
hosted userspace, libc, `printf`, `main()`, host-platform
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
| `Struct { item, args, layout }` | C `struct` |
| `Enum { item, args, layout }` | tagged C struct or niche-encoded per the layout's `EnumShape` / `TagEncoding` (see §9.8, §9.9 for worked examples) |
| `Union { item, args, layout }` | C `union` |

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
| `Constant(id)`  | C literal or compound literal materialised from `model.consts.resolve(id)`. Concrete per-`Const`-variant C lowerings (literal vs compound vs address-of for `Reloc`, etc.) compose with the per-`Ty` table at §2 and the `Const` vocabulary defined in `spec.md` §9.7. |

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

**Identifier emission policy.** Named locals via
`debug_name = Some` use the recorded name; locals with
`debug_name = None` use the synthetic `_N` form. The
identifier-emission convention pinned for the snapshot
pretty-printer (`pretty-printer.md`) coincides with the
generic-C backend's; if a future renderer or backend diverges,
each artifact pins its own convention.

§7 pins only the Body envelope (signature template, local
decls, basic-block labels). Per-statement C-emission for
`Operand` and intrinsic statements is at §5 / §6 above;
per-terminator C-emission is **not specified by stage 14** --
the terminator vocabulary (`Return` / `Goto` / `SwitchInt` /
`Call` / `Drop` / `Unreachable`) is IR-side closed at
`spec.md` §23, but the per-target C lowering for each
terminator surfaces with the phase-1 implementation sweep
(post stage-14 close) and lands in this file alongside the
implementation. Backends consuming the IR before that sweep
can derive each terminator's C emission directly from the
IR-side construct (per the project `README.md` §2
"C semantics with information preservation" principle --
"store the result, not the input"; see also `spec.md`
§17.3).

---

## 8. Diagnostic-logging backend contract

IR-side definition: `spec.md` §26 (diagnostic-logging
intrinsic vocabulary; D-14-2 cluster). Pipeline A uniformity
rule and `max_str_args_per_call` validation diagnostic D8
live IR-side in `spec.md` §26.8 and §26.9; this section
declares the generic-C target's bindings the bridge consults
when enforcing them.

### 8.1 Per-`LogLevel` C call shape

| `LogLevel` | Lowered C call shape |
|------------|----------------------|
| `Error`    | `printf("[ERROR] " <fmt> "\n", <args>...)` |
| `Warn`     | `printf("[WARN] " <fmt> "\n", <args>...)`  |
| `Info`     | `printf("[INFO] " <fmt> "\n", <args>...)`  |
| `Debug`    | `printf("[DEBUG] " <fmt> "\n", <args>...)` |

Single emitter (`printf`); single output stream (stdout);
severity rendered as a leading bracketed tag; explicit `\n`
appended by lowering. The format string emitted to the C
code is the concatenation of the bracketed severity tag, the
author template (per `spec.md` §26.7), and a literal `\n`.

### 8.2 Per-arg C specifier

The per-arg specifier is derived from each peeled operand's
RegseqModel `Ty`:

| Operand `Ty`               | C specifier |
|----------------------------|-------------|
| `Ty::Bool`                 | `%d` (lowered through `int`; printed `0` / `1`) |
| `Ty::Uint(UintKind::U8..U64)` | `%u` / `%u` / `%u` / `%llu` per width |
| `Ty::Int(IntKind::I8..I64)`  | `%d` / `%d` / `%d` / `%lld` per width |
| `Ty::Ref(Ty::Str, _)`      | `%s` (see §8.3 `&str` NUL-termination contract) |

Modifier-specifiers from `spec.md` §26.7 compose with the
per-width specifier (e.g., `{:08x}` on a `U32`-typed operand
lowers to `%08x` in the emitted C format string).

### 8.3 `&str` NUL-termination contract

Rust `&str` is a length-prefixed slice (data pointer + byte
length), not a NUL-terminated C string. The generic-C backend
MUST pass a NUL-terminated `const char *` to `printf`'s `%s`
specifier; the backend materialises the NUL-terminated form
from the `&str` data pointer + length pair at the lowering
site. Two materialisation strategies are admissible:

- **Static literals.** When the `&str` operand traces to a
  static string literal Constant (the `args[0]` template
  case, and any author-provided `Str` argument that resolves
  to a static literal at lowering time), the backend may emit
  the C string literal directly with an implicit trailing
  `\0` (C string literals are NUL-terminated by the C
  standard); no temporary buffer is needed.
- **Non-static `&str` operands.** For `&str` operands that
  do not trace to a static literal (e.g., author-provided
  `&str` arguments that borrow from a runtime-constructed
  string), the backend emits a temporary fixed-size stack
  buffer at the call site, copies the byte range, appends a
  `\0`, and passes the buffer pointer to `printf`. The
  **normative overflow contract** (binding, observable
  behaviour):
  - **Deterministic truncation algorithm** (applied
    unconditionally for every non-static `&str` operand):
    copy `min(source.len(), cap - 1)` bytes from the source
    into the buffer, then write `\0` at index
    `min(source.len(), cap - 1)`. **Truncation occurs
    whenever `source.len() >= cap`** (when source length
    equals or exceeds the cap, the algorithm copies
    `cap - 1` bytes and the buffer is full minus the
    terminator). No panic; no read past `source.len()`; no
    write past `cap - 1`.
  - **Guaranteed NUL termination** in all cases (truncated
    output included; the trailing byte of the buffer is
    always `\0`).
  - **No truncation indicator** in the emitted output -- the
    lowering does NOT append `"..."` or a similar marker.
    Authors needing truncation-aware behaviour bound their
    `&str` arguments at the source.
  - **Minimum buffer size: 256 bytes** (`cap >= 256`).
    Implementations MAY use a larger cap; this floor gives
    authors a portable lower bound on `&str` argument length
    that won't truncate.
  - **Per-`%s`-site buffer.** A call with multiple `&str`
    args gets independent buffers per `%s` site, not a
    single shared buffer. (Two `&str` args of length `cap`
    each must both fit; the buffers are stack-allocated and
    independent.)
  - The numeric cap value above the 256-byte floor is an
    implementation constant; the phase-1 implementation
    sweep picks the concrete cap, with the above contract
    binding the observable behaviour.

### 8.4 `max_str_args_per_call` declaration

**Generic-C `max_str_args_per_call = 8`** (counted across
`args[1..]`; the format template at `args[0]` is excluded
from the count). A Log Call site whose post-DiagLogPeel
argument set contains more than 8 `Ty::Ref(Ty::Str, _)`
operands is rejected at bridge-side validation per `spec.md`
§26.8 D8. The bound counts **all** `&str` operands, not only
the non-static ones -- the bridge does not need to trace
operand provenance to decide D8; the count is read directly
off the fingerprint's `arg_tys` field. The 8-arg ceiling
bounds per-call stack use at `8 * cap` bytes (`>= 2 KB` at
the 256-byte cap floor; comfortably below realistic stack
budgets while accommodating typical driver-logging patterns
of 0-3 `&str` args per call). The bound is strictly tighter
than necessary -- static-literal `&str` args don't allocate
buffers in practice -- but the simpler formulation avoids
bridge-side provenance analysis. Authors needing more than 8
`&str` operands at one call site compose multiple `log!`
calls.

### 8.5 Pipeline A consumption

The generic-C backend reads the post-DiagLogPeel flat-arg IR
shape (per `spec.md` §26.6) and emits the `printf` call with
each peeled operand rendered element-by-element through §8.2's
per-arg specifier. The backend **MUST NOT** reconstruct a
`LogArg` slice or any pre-DiagLogPeel carrier shape; the
prohibition is normative per `spec.md` §26.9 Pipeline A
uniformity rule, and the zero-runtime-cost-on-emit guarantee
rests on it.

---

## 9. Worked C-lowering examples

These examples illustrate the live spec. IR-side definitions
are anchored per example. Each example is generic-C /
target-agnostic; phase-2 backends contribute their own
target-specific worked examples to their respective
`<target>-backend.md`.

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
    void (*drop_in_place)(void *self);
    size_t size;
    size_t align;
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
    .drop_in_place = Driver__drop_shim_PL011,
    .size          = sizeof(PL011_t),
    .align         = _Alignof(PL011_t),
    .read          = Driver_read__shim_PL011,
};

uint32_t process(trait_obj_Driver_t d) {
    return d.vtable->read(d.data);
}
```

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
