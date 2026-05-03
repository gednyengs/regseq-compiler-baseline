# Stage-4 prior art: mrustc constants

mrustc is John Hodge's Rust→C translator. It is the cross-check
on **what const handling looks like without rustc's mature
const-eval**: mrustc evaluates Rust constants in-tree, producing
its own bytes-with-relocations form before C emission. Reading
mrustc tells us what stage 4 *avoids* by depending on rustc's
post-monomorphisation `optimized_mir`.

Source: `/tmp/regseq-stage4-research/mrustc/src/`. Citations are
of the form `path/to/file.cpp:N` against that checkout.

---

## 0. Source map

| Topic                                | File                                                | Lines        |
| ------------------------------------ | --------------------------------------------------- | ------------ |
| MIR-level `Constant` tagged union    | `src/mir/mir.hpp`                                   | 457-494      |
| MIR `Param` (constant-or-lvalue)     | `src/mir/mir.hpp`                                   | 498-514      |
| HIR `ConstGeneric` 4-variant union   | `src/hir/path.hpp`                                  | 45-67        |
| `EncodedLiteral` (post-eval form)    | `src/hir/encoded_literal.hpp`                       | 8-82         |
| `EncodedLiteralSlice`                | `src/hir/encoded_literal.hpp`                       | 84-125       |
| `HIR::Static.m_value_res`            | `src/hir/hir.hpp`                                   | 132          |
| `m_monomorph_cache` (per-path cache) | `src/hir/hir.hpp`                                   | 137          |
| `HIR::Evaluator` interface           | `src/hir_conv/constant_evaluation.hpp`              | 19-82        |
| Internal `Constant` (eval state)     | `src/hir_conv/constant_evaluation.cpp`              | 218-275      |
| Internal `Allocation` (eval state)   | `src/hir_conv/constant_evaluation.cpp`              | 277-442      |
| `StaticRef` (eval state)             | `src/hir_conv/constant_evaluation.cpp`              | 443-540      |
| `Allocation → EncodedLiteral`        | `src/hir_conv/constant_evaluation.cpp`              | 3086-3133    |
| `Evaluator::evaluate_constant`       | `src/hir_conv/constant_evaluation.cpp`              | 3135+        |
| Static-borrow const promotion        | `src/hir_expand/static_borrow_constants.cpp`        | 1118, 1162, 1366, 1405 |
| Trans-time monomorph const eval      | `src/trans/monomorphise.cpp`                        | 107-198      |
| Codegen `emit_constant` (MIR side)   | `src/trans/codegen_c.cpp`                           | 8104-8233    |
| Codegen `emit_static_local`          | `src/trans/codegen_c.cpp`                           | 2538-2630    |
| MIR validation entry point           | `src/mir/check.cpp`                                 | 96, 636, 1348|

---

## 1. Const-value type — two layers, not one

mrustc carries constants in **two distinct shapes**, with an
internal third shape used only inside the evaluator.

### 1.1 `MIR::Constant` (pre-evaluation, IR-level)

`src/mir/mir.hpp:457-494`. `TAGGED_UNION` with 10 variants:

```c++
TAGGED_UNION_EX(Constant, (), Int, (
    (Int,          struct { S128 v; HIR::CoreType t; }),
    (Uint,         struct { U128 v; HIR::CoreType t; }),
    (Float,        struct { double v; HIR::CoreType t; }),
    (Bool,         struct { bool v; }),
    (Bytes,        std::vector<uint8_t>),         // Byte string
    (StaticString, std::string),                  // String
    (Const,        struct { unique_ptr<HIR::Path> p; }),  // `const`
    (Generic,      HIR::GenericRef),
    (Function,     struct { unique_ptr<HIR::Path> p; }),  // ZST fn-type
    (ItemAddr,     unique_ptr<HIR::Path>)         // Address of a value
), (), (), (...));
```

The variant set splits into three groups:

- **Already-evaluated leaves** (Int / Uint / Float / Bool / Bytes
  / StaticString) — fully self-contained values, codegen-ready.
- **Path-bearing references** (Const / Function / ItemAddr) —
  references to other items that mrustc evaluates *later*. These
  are deferred-evaluation slots: `Constant::Const(path)` means
  "the value of this `const` item, look it up in the crate."
- **Generic carrier** (Generic) — pre-monomorphisation generic
  parameter reference; replaced during monomorphisation.

Crucially, **`MIR::Constant` carries `HIR::Path` references that
are not yet resolved to their values**. The lowering from HIR
emits them as path-bearing constructors (e.g.
`Constant::make_Const({box$(node.m_path.clone())})` at
`src/mir/from_hir.cpp:2727`). They get resolved later (see §7).

### 1.2 `EncodedLiteral` (post-evaluation, byte-buffer)

`src/hir/encoded_literal.hpp:49-82`:

```c++
struct EncodedLiteral {
    inline static const unsigned PTR_BASE = 0x1000;
    std::vector<uint8_t>  bytes;
    std::vector<Reloc>    relocations;
    // ...
};

struct Reloc {
    size_t  ofs;
    size_t  len;
    std::unique_ptr<HIR::Path>  p;
    std::string  bytes;  // alternative when p is null
};
```

This is the **fully-evaluated** form, used for:
- `HIR::Static.m_value_res` (the cached value of a static)
- `HIR::ConstGeneric::Evaluated(EncodedLiteralPtr)` (resolved
  generic-const arguments — `path.hpp:57`)
- `HIR::Static.m_monomorph_cache: map<HIR::Path, EncodedLiteral>`
  (per-monomorphisation cache — `hir.hpp:137`)

The shape is "byte buffer + relocation table" — flat
representation, no nested structure. Aggregates collapse into
the byte stream; pointers/references appear as relocation
entries. (See §3 and §4.)

`PTR_BASE = 0x1000` is a magic constant: pointer-bearing bytes
in the buffer carry the relocation's *additional offset* with
this base subtracted, so codegen can distinguish "address-of-X
plus offset 0" from "null". See `codegen_c.cpp:2580` (`v -=
EncodedLiteral::PTR_BASE`) and §5 below.

### 1.3 Internal evaluator types (not IR-visible)

The Evaluator runs MIR against three IValue subclasses
(`constant_evaluation.cpp:140-540`):

- `Constant` (immutable byte buffer for read-only static data,
  e.g. string literals).
- `Allocation` (mutable byte buffer with per-byte init mask and
  inline relocation list — see §6).
- `StaticRef` (path-handle pointing to a static).

`RelocPtr` is a 3-variant tagged union over these
(`constant_evaluation.cpp:161-216`). These types are NOT part of
the IR — they live only inside `Evaluator` while it interprets
MIR. The IR-visible shape is `EncodedLiteral` after
`allocation_to_encoded` (`constant_evaluation.cpp:3086`).

---

## 2. Type-tag placement

### 2.1 Inline in scalar variants

`MIR::Constant::Int / Uint / Float` carry the type tag inline as
`HIR::CoreType` (`mir.hpp:458-468`). This matches stage-4's
plan (`Int(IntKind, value)` form per spec.md §9.7).

### 2.2 Bool: implicit (variant tag = type)

`MIR::Constant::Bool { bool v }` (`mir.hpp:470-472`) has no `t`
field. The variant discriminant alone identifies the type.
Stage 4 plans the same shape.

### 2.3 Float storage width is always `double`

`mir.hpp:466-469`: `(Float, struct { double v; HIR::CoreType t; })`.
The storage is unconditional `double` (64-bit) regardless of
whether `t` is `F16`, `F32`, `F64`, or `F128`. The codegen path
at `codegen_c.cpp:8189-8190` calls `emit_float(c.v, c.t)` which
re-derives the C-level form from the type tag, with `F16` and
`F128` rejected via `f16_disabled()` / `f128_disabled()` stubs
at `codegen_c.cpp:2632-2636`.

**Stage-4 implication:** mrustc's storage choice is lossy for
`F16` (silently rounds 16-bit float values to nearest
representable `double` and back) and only *partially* lossless
for `F32` (round-trip-stable but architecturally awkward). Stage
4 should **NOT** adopt this. The rustc-prior-art file flags
that rustc's `Scalar` carries the value at the natural width per
type. Stage 4's spec.md §9.7 already plans
`Float(FloatKind, bits)` where `bits` is the natural-width
unsigned integer — adopt that.

### 2.4 EncodedLiteral has no type tag

The post-eval byte buffer at `encoded_literal.hpp:49-58` carries
**no type information**. The type is recovered from context
(`HIR::Static.m_type`, `HIR::Constant.m_type`) at consumption
sites. Codegen emits a `__attribute__((packed))` C struct view
over the raw bytes (`codegen_c.cpp:2552-2618`).

**Stage-4 implication:** stage 4's `Const` enum is the
identity-bearing IR object; consumers must be able to reason
about the value *without* an external type lookup. mrustc's
context-typed approach does not survive re-derivation under our
single-IR + interner framing.

---

## 3. Aggregate constants

### 3.1 No aggregate variant in `MIR::Constant`

`MIR::Constant` (10 variants above) has no `Aggregate` /
`Struct` / `Array` / `Tuple` form. Aggregate construction at MIR
level is `RValue::Aggregate { values: Vec<Param>, ... }` or
similar — but those are *r-values* (Statement RHS), not const
*values*. A `MIR::Constant` itself is always one of the leaf
shapes plus the four path-bearing variants.

This means: a Rust expression like `[1u32, 2u32, 3u32]`
appearing as a const-context value never lives as a
`MIR::Constant` — it's either an `RValue::Aggregate` (in
function bodies) or evaluates to an `EncodedLiteral` (when
referenced from a static / const-generic position).

### 3.2 Aggregates flatten to bytes-with-relocations

In `EncodedLiteral`, aggregates are flat byte buffers:
- field 0 occupies `bytes[0..size_of<F0>()]`
- field 1 starts at `padding_align(field_0_end, align_of<F1>())`
- … and so on, following the layout rules computed during
  `Allocation::write_*`.

There is NO internal structure preserved. Pointer-bearing
fields appear in `relocations`; everything else appears as raw
bytes.

`allocation_to_encoded` at `constant_evaluation.cpp:3086-3133`
shows the conversion. Notable behaviour:

- **Aggregate child pointers create new statics.** When the
  source `Allocation` contains a relocation pointing at another
  *writable* `Allocation` (a nested aggregate), mrustc
  recursively encodes the inner allocation (line 3099) and
  promotes it to a freshly-named static via
  `nvs.new_static(inner_type, inner_val)` (line 3106). The
  outer relocation then refers to that synthetic static by path.
  This is **eager promotion** — every nested aggregate that
  contains data becomes a top-level static.

- **String children are inlined.** When the inner allocation is
  *read-only* (a string literal), the bytes are copied into the
  outer `Reloc::bytes` field rather than promoted to a static
  (line 3115). At C emission time these become string literals
  via `print_escaped_string` (`codegen_c.cpp:2594`).

- **Children are NOT shared by structural equality.** Each
  aggregate gets fresh sub-allocations / promoted statics. Two
  identical inner aggregates produce two distinct synthetic
  statics. (This is the opposite of stage 4's planned
  `ConstId`-based child sharing.)

### 3.3 Codegen emits aggregates as `__attribute__((packed))`

`codegen_c.cpp:2538-2630` (`emit_static_local`) is the read-out
path. It emits `static T x = { .raw = { 0x..., 0x..., ... } };`
with `.raw` an array of `uintptr_t` words, with relocations
inserted as `(uintptr_t)&Trans_Mangle(path) + offset`
expressions. The `is_packed` flag drives whether to use the
word-array path or per-byte emission for non-pointer-aligned
data (line 2607-2617 — pure-bytes case rejects relocations).

**Stage-4 implication:** mrustc's design loses every piece of
aggregate structure information by the time the IR reaches
codegen. Stage 4's plan to keep aggregates as
`Aggregate(Vec<ConstId>)` preserves the structure as a
first-class IR property — supports introspection, structural
canonicalisation, and per-field rendering without consulting
the type. mrustc's choice is downstream-friendly (codegen just
emits bytes) but loses the affordances stage 4 needs for
deterministic identity.

---

## 4. Relocation handling

### 4.1 At MIR-Constant level: HIR::Path verbatim

`MIR::Constant::ItemAddr(unique_ptr<HIR::Path>)` (`mir.hpp:482`)
carries the relocation target as a **bare `HIR::Path`**:

- After monomorphisation, the path is fully concrete (all
  generics substituted).
- The path itself IS the relocation key. No layer of
  abstraction.
- No stable cross-build hash — the path is mrustc-internal
  identity, valid only within one compilation.

`MIR::Constant::Function(Path)` is similar but represents the
function's ZST type (the function value is `Trans_Mangle(path)`
without `&`).

### 4.2 At Allocation level: RelocPtr (refcounted handle)

`constant_evaluation.cpp:161-216` defines `RelocPtr` as a
3-variant pointer:
- `ConstantPtr` — pointer to an immutable byte buffer (string
  literal, etc.).
- `AllocationPtr` — pointer to a mutable byte buffer (nested
  aggregate during evaluation).
- `StaticRefPtr` — pointer to a named static (the path-bearing
  case).

Each variant is refcounted so the evaluator can share inner
allocations (e.g., when a copy is made via `MIR::RValue::Use`).

The relocation is stored at the byte-offset where the pointer
sits: `Allocation::Reloc { offset: size_t, ptr: RelocPtr }`
(`constant_evaluation.cpp:282-285`).

### 4.3 At EncodedLiteral level: Reloc

`encoded_literal.hpp:8-48` defines:

```c++
struct Reloc {
    size_t  ofs;
    size_t  len;
    std::unique_ptr<HIR::Path>  p;   // named target, OR
    std::string  bytes;              // inline bytes (string literal)
};
```

`p` is set for relocations to named statics/functions; `bytes`
is set for inline byte data (read-only strings written directly
into the relocation slot rather than promoted to a static).

`Reloc::ord` (`encoded_literal.hpp:31-44`) compares
lexicographically: offset, then length, then `p` (recursive
HIR::Path compare) or `bytes` (string compare).

### 4.4 The PTR_BASE = 0x1000 trick

`encoded_literal.hpp:50` and `codegen_c.cpp:2580`:

The byte buffer reserves bytes at relocation offsets to hold an
"additional offset" that codegen adds to the named target. The
value stored in the bytes is `PTR_BASE + additional_offset`, so
that:
- A "null pointer" reads as 0 (no relocation entry, raw bytes
  are 0 → C emits literal 0 → null).
- A "pointer to X with offset 0" has bytes[ofs..ofs+ptr_size] =
  encoded `PTR_BASE` and a relocation entry with `p=X`. Codegen
  reads the bytes, subtracts `PTR_BASE`, gets 0, emits `(uintptr_t)&X`.
- A "pointer to X with offset N" stores `PTR_BASE + N` in the
  bytes; codegen subtracts and emits `(uintptr_t)&X + N`.

This is a serialisation/deserialisation artefact: mrustc keeps
the offset in the byte buffer (rather than in the relocation
struct) because the bytes are also used directly during constant
folding within the evaluator, where pointer arithmetic operates
on the byte view.

### 4.5 No cross-build determinism

`HIR::Path` is mrustc-internal identity. There is no stable
hash, no `DefPathHash` analogue. Two builds of the same crate
with cosmetic source changes (whitespace, comment edits) MAY
produce different `HIR::Path` values in some corners. mrustc
makes no guarantee.

**Stage-4 implication:** mrustc's relocation scheme is the
"local-ID with inline offset" pattern; stage 4's planned 4-tuple
`(kind_rank, def_path_hash, args_key, shim_disambig)` with
inline byte-offset is the same idea elevated to stable identity.
The PTR_BASE trick is unnecessary for stage 4 (we have a
separate offset field if needed; we are not stuck encoding
through a uint8_t stream).

---

## 5. Null pointer typing

### 5.1 Implicit at every layer

Null pointers in mrustc are **never explicit**:

- `MIR::Constant` has no `Null` / `NullPtr` variant. Null
  pointer constants come from `RValue::Cast` of `Constant::Uint(0)`
  to a pointer type, or are implicit in the type's zero-init.
- `Allocation` represents null as bytes 0 with no relocation
  entry (init mask all-set; relocations empty for those bytes).
- `EncodedLiteral` does the same: bytes 0, no `Reloc` entry at
  that offset.

Codegen emits null as the literal `0` cast appropriately
(`codegen_c.cpp:2603`: `m_of << "0x" << std::hex << v << "ull"`
when no relocation matches the offset).

### 5.2 Type carried externally

The type of a null pointer is recovered from context:
- For static fields: from `HIR::Static.m_type` field layout at
  the relevant offset.
- For function-pointer slots: from the slot's declared type at
  the C-side.

### 5.3 The empty-`HIR::Path` ItemAddr

There is one cosmetic null-like form at MIR level:
`MIR::Constant::make_ItemAddr({})` with default-constructed
`HIR::Path`. From `from_hir.cpp:1598, 1713, 1718, 1726, 1756`,
this is constructed as the *metadata* slot of a `MakeDst` rvalue
(constructing a fat pointer). The empty-path payload is a
placeholder that codegen treats specially. This is NOT a
typed-null pointer — it's a sentinel for the metadata-vacant
case in fat-pointer construction.

**Stage-4 implication:** mrustc's implicit-null design fails
the "canonical equality between vacant-vtable-slot null and
string-data null" requirement that stage 4 spec.md §9.7
imposes. mrustc never has to make that distinction because
`EncodedLiteral` doesn't intern. Stage 4's planned
`NullPtr(TyId)` is the right divergence. mrustc's implicit-null
choice is downstream of its no-interner choice — the two fall
together.

---

## 6. Uninit / padding

### 6.1 Allocation tracks per-byte init mask

`constant_evaluation.cpp:287-301`:

```c++
class Allocation final: public IValue {
    unsigned    reference_count;
    unsigned    length;
    bool        is_readonly;
    HIR::TypeRef  m_type;
    std::vector<Reloc>  relocations;
    uint8_t data[1];   // length bytes, then ((length+7)/8) mask bytes

    Allocation(size_t len, const HIR::TypeRef& ty)
        // ...
        memset(data, 0, len + (len + 7) / 8);  // both data AND mask zeroed
    }
    // ...
};
```

The `data[1]` field is a flexible-array hack: the storage
trailing the struct holds `length` data bytes followed by
`(length + 7) / 8` mask bytes (one bit per data byte, 1 = init,
0 = uninit). `get_mask()` returns `data + length`.

`Allocation::get_bytes(ofs, len, check_mask)` at
`constant_evaluation.cpp:327-348` is the read API. With
`check_mask = true`, returns `nullptr` if any byte in the range
is uninit. Read sites that tolerate uninit pass `false`.

`ext_write_bytes(ofs, len)` at `constant_evaluation.cpp:387-405`
sets the mask bits for written bytes and clears any relocation
entries that fall in the written range.

### 6.2 EncodedLiteral DROPS the init mask

`allocation_to_encoded` at `constant_evaluation.cpp:3088-3092`:

```c++
//const auto* a_bytes = a.get_bytes(0, a.size(), true);
const auto* a_bytes = a.get_bytes(0, a.size(), false);
// NOTE: Read the uninitialised bytes (they _should_ be zeroes)
ASSERT_BUG(this->root_span, a_bytes, "Unable to get entire allocation - ...");
EncodedLiteral  rv;
rv.bytes.insert(rv.bytes.begin(), a_bytes, a_bytes + a.size());
```

The conversion **collapses uninit-as-zero**. The init mask
is not propagated to `EncodedLiteral`. The comment makes the
assumption explicit: uninitialised bytes "should be zeroes."

This is sound at the IR level *only because* `Allocation`'s
constructor `memset`s both data and mask to zero (line 300):
truly-uninit bytes are observably zero, so reading them
unmasked yields the all-zero pattern.

But **padding-vs-zero distinction is lost** at the IR boundary.
A struct with explicit zero padding and a struct with
unspecified padding map to identical `EncodedLiteral`s.

### 6.3 Codegen emits uninit ranges as zero bytes

`codegen_c.cpp:2603` is the no-relocation branch — it emits
`0x<hex>ull` from the byte values. There's no "uninit" syntax
in the C output; the bytes are zero on disk, so the C compiler
sees zero literals.

**Stage-4 implication:** mrustc collapses uninit. This is the
**lossy path** — and aligns with charon's `Byte::Uninit`
*absence* discussion. Stage 4's spec.md §9.7 line "padding /
uninitialised bytes (canonicalisation must distinguish 'uninit'
from any specific byte pattern)" calls for the *non*-collapsing
choice. mrustc does NOT support that requirement; charon's
three-state `Byte::{Uninit, Value(u8), Provenance(_, u8)}`
does. The mrustc design choice is consistent (no interner →
no canonical equality → no need to distinguish) but does NOT
provide a precedent stage 4 can adopt.

---

## 7. Validation phase

mrustc's validation surface for constants is **distributed** —
no single "const validation phase":

### 7.1 No const-specific checks at MIR level

`src/mir/check.cpp:96` (`MIR_Validate_ValState`),
`check.cpp:636` (`MIR_Validate`), `check.cpp:1348`
(`MIR_CheckCrate`) — none of these contain const-specific
validation. A `MIR::Constant` is treated as a typed leaf;
correctness of its payload is upstream-of-MIR concern.

### 7.2 Const evaluation runs in HIR expansion / monomorphisation

Three call sites for `Evaluator::evaluate_constant`:

- **`hir_expand/static_borrow_constants.cpp:1162, 1405`** — the
  static-borrow promotion pass. References like `&[1, 2, 3]` get
  promoted to anonymous statics; their initialisers are
  evaluated here.
- **`hir_conv/constant_evaluation.cpp:1110, 1322, 1351, 1483`**
  — the bulk of HIR-side const evaluation: array lengths,
  generic-const args, enum discriminants, top-level `const`
  items, top-level `static` initialisers.
- **`trans/monomorphise.cpp:107-198`** — generic-const
  arguments evaluated when monomorphising a generic function
  (the per-monomorphisation cache populated from this site).

The **MIR consumer guarantees** are:
- `MIR::Constant::Const(path)` is a bridge bug at codegen
  (`codegen_c.cpp:8207`: `MIR_BUG(*m_mir_res, "Unexpected
  Constant::Const - " << ve)`).
- `MIR::Constant::Generic(...)` is a bridge bug at codegen
  (`codegen_c.cpp:8210`: `"Generic value present at codegen"`).
- `MIR::Constant::Function(...)` is `MIR_TODO` (line 8213) —
  not yet wired through codegen for the function-as-ZST case.

So the implicit "phase" in mrustc is: by the time
`MIR::Constant` reaches codegen, every `Const`/`Generic` must
have been replaced with a leaf value or an `ItemAddr`.

### 7.3 No type-level vs value-level split

mrustc uses ONE evaluator for all const eval. Generic-const
arguments and aggregate static initialisers go through the same
`HIR::Evaluator::evaluate_constant` interface — no phase
split, no separate "type-level" vs "value-level" interner.

mrustc gets away with this because:
- It has no const interner (no `ConstId`, no canonical content
  hash).
- It does not validate const equality across types — each
  consumer of a `ConstGeneric::Evaluated` does its own byte-by-byte
  compare (`HIR::ConstGeneric::ord` in `path.hpp:65`).
- It runs eval lazily per-monomorphisation (`m_monomorph_cache`
  in `hir.hpp:137`), so there is no global ordering constraint.

**Stage-4 implication:** mrustc's no-phase-split design works
*because* it has no interner. Stage 4 has an interner (per spec.md
§9.7 ConstInterner), and the interner participates in
`GenericArg::Const(ConstId)` nominal-type identity. The
type-level / value-level split is forced by the interner's
participation in `TyInterner` validation (Phase B step 4 before
step 6). mrustc's experience does NOT validate the necessity of
the split — it sidesteps the question. Justification rests on
the F4-2 fingerprint determinism argument (per spec.md §3.3 and
§9.7).

---

## 8. Cross-build determinism

### 8.1 Not a stated mrustc goal

mrustc does NOT aim for cross-build identity stability.
`README.md` and project documentation (consulted briefly during
research) emphasise correctness of Rust→C translation and
self-hosting; deterministic identity across builds is not a
named requirement.

### 8.2 No StableHash analogue

Searches for `stable_hash`, `StableHash`, `DefPathHash`, and
similar produce no significant matches. The cross-build identity
machinery present in rustc (per the rustc-prior-art file) has no
counterpart here.

### 8.3 EncodedLiteral comparison is byte-equal-and-path-equal

`encoded_literal.hpp:76-81` (`EncodedLiteral::ord` /
`operator==`) compares:
- `bytes` lexicographically (cheap memcmp).
- `relocations` element-by-element via `Reloc::ord`.
- `Reloc::ord` (line 31-44) compares offset, length, `p`
  recursively (`HIR::Path::ord`), and `bytes`.

This is **deterministic within one build** (same crate, same
compilation): two evaluations of the same expression in the
same context produce equal `EncodedLiteral`s.

Across builds, `HIR::Path` equality may not hold (the path
encodes mrustc-internal name resolution state, not a stable
hash). mrustc serialises/deserialises `EncodedLiteral` for
incremental compilation (`hir/deserialise.cpp:1467+`) but the
incremental-cache key is the source crate + compilation context,
not a content hash.

**Stage-4 implication:** confirms the rustc-prior-art finding
that `HashStable for AllocId` is the load-bearing stable-hash
implementation in the prior-art. mrustc doesn't have an equivalent;
its `EncodedLiteral` is *deterministic-within-build* only.

---

## Stage-4 applicability

A focused mapping of mrustc's design choices to stage 4's D-4-1
decision space:

### What stage 4 should ADOPT from mrustc

- **Inline scalar type-tag** (`Int(IntKind, value)` etc.) is the
  same shape spec.md §9.7 already plans.
- **The bytes-with-relocations primitive** as a *building block*
  for value-level consts — mrustc, charon (RawMemory), and rustc
  (Allocation) all converge on this shape for the
  pointers-mixed-into-data case. Stage 4's plan to use
  structured `Aggregate(Vec<ConstId>)` does NOT cover the
  arbitrary-mixed-bytes case (e.g., a `repr(C)` struct with
  explicit padding initialised via `MaybeUninit::zeroed()`).
  Worth a sub-decision: do we admit a `RawBytes(Vec<Byte>)`
  variant *alongside* `Aggregate`, or force everything through
  `Aggregate`?

### What stage 4 should REJECT or DIVERGE from

- **Two-layer (path-bearing + evaluated) const representation.**
  mrustc carries `MIR::Constant::Const(path)` because it must
  evaluate the const itself. Stage 4 inherits rustc's post-mono
  guarantee — every const is fully evaluated by
  `optimized_mir`. The path-bearing variants
  (`Const`/`Generic`/`Function`) are EXACTLY the constructs the
  bridge rejects per the no-defensive-fallbacks methodology.
  Stage 4's `Const` enum has only the post-eval shapes.
- **Float storage as `double`.** mrustc's lossy `double` for
  all float types is a pragmatic shortcut. Stage 4 should use
  natural-width unsigned bits per `FloatKind` (already planned
  in spec.md §9.7).
- **Implicit null-pointer (bytes-zero, no reloc).** mrustc gets
  away with this because it has no interner; stage 4's
  canonical-equality requirement for vacant-vtable-slot nulls
  forces an explicit `NullPtr(TyId)` variant. The
  rustc-prior-art file similarly recommends inline-typed null.
- **Eager promotion of nested aggregates to fresh statics** (in
  `allocation_to_encoded`). mrustc creates synthetic statics for
  every nested aggregate; stage 4's `Aggregate(Vec<ConstId>)`
  with structural sharing avoids this — equal nested aggregates
  share a `ConstId`, no fresh static per occurrence.
- **PTR_BASE = 0x1000 byte-encoded offset.** A
  serialisation-hack artefact of mrustc's bytes-only
  representation. Stage 4 has structured relocations
  (`InstanceRef` with explicit offset field if needed), so the
  trick is unnecessary.
- **Uninit-as-zero collapse at the IR boundary.** mrustc's
  `Allocation` tracks per-byte init bits but
  `allocation_to_encoded` discards them. Stage 4's spec.md §9.7
  requires uninit-vs-zero distinguishability — must NOT collapse.
- **No const interner / no phase split.** mrustc's design
  works because it doesn't intern. Stage 4 interns; the
  validation phase split (type-level before value-level) is
  forced by the interner's participation in
  `GenericArg::Const(ConstId)` nominal-type identity.
- **HIR::Path as relocation key.** Local identity, not stable
  cross-build. Stage 4's 4-tuple `(kind_rank, def_path_hash,
  args_key, shim_disambig)` is the right elevation; mrustc
  provides no precedent for it.

### What mrustc's experience tells us about the post-mono guarantee

The cross-check finding from the meta-question:

> **Without rustc's post-mono const-eval guarantee, the IR needs
> two layers** — an unresolved-AST layer (`MIR::Constant` with
> path-bearing variants) AND an evaluated-byte-buffer layer
> (`EncodedLiteral`). The lower layer requires its own evaluator
> (~3000 lines of `constant_evaluation.cpp`).

Stage 4's dependency on rustc's post-mono guarantee (a
methodology rule already locked: "store the const-eval result,
not the const expression") means:

- **One IR layer suffices.** No path-bearing `Const` /
  `Generic` / `Function` variants are needed — the bridge
  rejects them.
- **No in-tree evaluator.** RegseqModel reads the bridge's
  `ConstValue` output directly; we do not interpret MIR for
  const evaluation.
- **Narrower validation surface.** mrustc's `Allocation`'s
  init-mask + relocation-list machinery is not in stage 4's
  scope — we accept the bridge's already-canonicalised content.
- **Trade-off explicitly named:** if rustc's `optimized_mir`
  contract ever changes such that unevaluated consts can survive
  to the post-mono stage, stage 4 raises a `BridgeError` rather
  than working around the change. The pinned-toolchain
  methodology rule absorbs the risk.

This is the answer to the framing question "what would we lose
if rustc's const-eval ever changes shape." We would lose the
ability to skip the entire AST-level layer and the evaluator
infrastructure mrustc carries (~3000 lines + ~700 lines of
internal value types). The pinned-toolchain methodology
explicitly accepts this dependency.

---

## Open questions surfaced for D-4-1

1. **`RawBytes` / `RawMemory` variant alongside `Aggregate`?**
   mrustc, charon, and rustc all admit a "byte buffer with
   relocations" primitive at SOME layer. Stage 4's planned
   `Aggregate(Vec<ConstId>)` does not directly cover the
   "explicit padding bytes initialised to specific values"
   case. Sub-decision for D-4-1: admit a RawBytes variant
   carrying `Vec<Byte>` (charon's three-state) or constrain
   `Aggregate` to handle this via `Uninit { size: u32 }` leaf
   children only?

2. **Promote-aggregates-to-statics vs structural-share-via-ConstId?**
   mrustc promotes nested aggregates to synthetic `HIR::Static`
   items; stage 4 plans structural sharing through `ConstId`.
   The promote-to-static approach pushes the equality problem
   into the Item table (which has its own deduplication
   machinery in stage 1). Stage 4's choice is the structural
   one, but mrustc's experience surfaces a sub-question: when
   a `Const` value is "the same as" an existing top-level static
   item, should it intern as `Reloc(static_item)` or as the
   underlying inlined `Aggregate`?

3. **Float storage — natural-width bits, not `double`.**
   Already-planned in spec.md §9.7. mrustc's lossy `double`
   storage is the wrong choice; the prior-art comparison
   confirms.

4. **Validation phase split — necessity rests on F4-2, not on
   prior art.** Neither mrustc, charon, nor LLVM has the
   type-level / value-level interner split. The justification
   for stage 4's split is internal: the type-level subset must
   be canonicalised before `TyInterner` because it
   participates in nominal-type identity. mrustc and charon
   sidestep by not interning at all; LLVM sidesteps by uniquing
   per-LLVMContext rather than across builds.

5. **Init-mask preservation.** Stage 4 spec.md §9.7 calls for
   uninit-vs-zero distinguishability. mrustc demonstrates the
   *lossy* path; charon's `Byte::Uninit` demonstrates the
   *lossless* path. The decision needs evidence that fixtures
   actually rely on the distinction (or that they don't). Forward
   to D-4-1 sub-decision.

6. **Function-as-ZST handling.** mrustc has
   `Constant::Function(path)` for the ZST case (the function
   *value*, not its address) but it's `MIR_TODO` in codegen.
   This suggests mrustc has not exercised the FnDef-value
   reaching const position. Stage 4's `Ty::FnDef(ItemId,
   GenericArgs)` is ZST per spec.md §10; the constant of a
   FnDef is logically the unit value (or omitted entirely).
   D-4-1 sub-decision: does `Const` need a separate ZST variant,
   or does `ZeroSized` (rustc's name) suffice?

---

## Hypotheses (marked)

- **Hypothesis (uncited):** mrustc's choice of `HIR::Path` as
  relocation key, rather than a content hash, is influenced by
  its self-hosting goal — the pre-existing rustc-1.x source it
  bootstraps does not produce stable hashes either. *Not
  load-bearing for stage-4 decisions; flagged as conjecture.*

- **Hypothesis (uncited):** mrustc's eager promotion of nested
  aggregates to fresh statics (rather than structural sharing)
  is downstream of the C target — every static gets a name in
  the linker symbol table, and the promote-to-static path
  reuses existing infrastructure for emitting names. RegseqModel's
  C target may face the same pressure at codegen, but stage 4
  (IR-level) is free to pick structural sharing because the
  C-emission stage downstream can re-promote when needed.
  *Worth revisiting at codegen-stage spec.*

---

## What is NOT covered here

Out of scope per directive:
- mrustc's borrow-checker, type-checker, full backend code
  emission.
- The HIR-level expression representation (only the literal /
  encoded-literal types were read).
- The MIR optimisation passes (`src/mir/optimise.cpp`,
  `src/mir/cleanup.cpp`).
- The MIR builder (`src/mir/mir_builder.cpp`) construction
  pattern.
- The deserialise / serialise format (only the structural
  schema was noted at `deserialise.cpp:1467` for cross-reference).
