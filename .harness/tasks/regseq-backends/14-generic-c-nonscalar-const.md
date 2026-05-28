# regseq-backends-14: Generic-C non-scalar consts — byte-image reconstruction, the enum `_raw` union member, the reference carrier & multi-member `Const::EnumValue` (Phase-1 task d2.2)

## Description

Replace the **non-scalar `Const` arms** d2's `const_expr` + d2.1's multi-member
`EnumValue` left as `EmitError::Unsupported("…task d2.2")` with real §12 lowering for
every shape the bridge produces, **soundly under the pinned `cc` flags**
(`-std=gnu11 -fwrapv -Wall`, **no** `-fno-strict-aliasing`, `toolchain.rs:79-84`):
`Const::Bytes(bytes)` over every consumer type (scalar / pointer / `Tuple` / `Struct`
/ `Union` / `Array` / `Enum`); the `Const::Aggregate([Reloc, Uint(meta)])` **reference
carrier** (thin and fat `&T` / `&str` / `&[T]`); `Const::Aggregate(children)` over the
valtree-rooted typed aggregates; the **multi-member `Const::EnumValue` cases d2.1
handed off**; and the **`ConstMemoryStatic` byte-image static definitions** the carrier
is transitively coupled to. This is **task d2.2** of the Phase-1
a→b→c1→c2→d1→d2→d2.1→**d2.2**→e sequence.

A **first Codex spec review of this task's prior draft found two architectural
soundness holes that this draft is rebuilt to avoid** (a Codex design consult then
validated the two primitives below):

1. **An enum's §13 union-of-views cannot be const-initialized with separate tag and
   payload.** `.u._tag` and `.u._v<v>` are two members of the **same** `union u`
   (`enum_emit.rs:98-127`), and a C designated initializer that names a second union
   member **overrides** the first (they share offset 0) — the tag is lost. The d2.1
   *runtime* path avoids this with sequential statements (`x.u._tag.tag = tv;
   x.u._v1._0 = …;`); a value-position const is one expression and cannot. **Fix —
   the `_raw` union member (Change A):** add `unsigned char _raw[<size>];` to every
   **multi-view** enum's `union u`, and lower a fully-known enum const value as
   `(struct E){ .u._raw = { b0, …, bN-1 } }`. Runtime reads stay in-place union
   accesses (`x.u._tag.tag`) — gnu11 union type-punning, **sound** under the pinned
   flags **provided reads never take the address of an inactive member and
   dereference a typed pointer later** (they don't — the d2.1 navigator reads
   in-place). The member is layout-neutral (the views are already full-object size
   `N`; `_raw[N]` has alignment 1; the outer `aligned(layout.align)` + the §13
   `_Static_assert(sizeof/_Alignof)` guards still hold).
2. **A `ConstMemoryStatic` of `uint8_t[N]` read through `(const T*)&X` for non-char
   `T` is strict-aliasing UB (and may be mis-aligned).** The bridge synthesises every
   `ConstMemoryStatic` as a `Static { ty: Array(u8, byte_len) }` (`const_.rs:221-230`).
   For `&str` / `&[u8]` the consumer reads `const uint8_t *` — char-to-char, **sound**.
   For `&[u32]` / `&Struct` / `&[fn();N]` the consumer reads a non-char type through a
   `uint8_t` array — UB. **Fix — re-type `ConstMemoryStatic` by its consumer pointee
   (Change F):** a backend pre-pass scans the model for each `ConstMemoryStatic`'s
   consumer(s), recovers the pointee storage as a **backend-local `StorageTy`
   descriptor** (a sized pointee `TyId`, or an `{elem TyId, count}` array backing — no
   model interning, since the bridge interns `Slice(elem)` but not `Array(elem, meta)`),
   and **declares the static from it** (1-byte `u8` / `i8` data — `&str` / `&[u8]` — as
   `uint8_t[N]`; `Ty::Char` data as `uint32_t[N]`; any other element / sized pointee as
   its real C type), so `&X` is the right type and the consumer cast is same-type (sound,
   naturally aligned). The
   content-hash dedup (`const_.rs:145-152`) can merge two reference consts with
   identical bytes but incompatible pointee types onto one `ConstMemoryStatic` →
   **fail-loud** (a distinct `task d2.3` label; a type-aware dedup key is a bridge
   change, out of scope).

Per the [[assessment-soundness-pass]] discipline (which caught holes the green
compile-and-run fixtures hid in d1, d2, **and** d2.1), **d2.2's close gets a dedicated
Codex soundness pass** in addition to the spec review. d2.2 is layout-invariant-touching
on two axes — byte-image decoding of every scalar / aggregate / enum / pointer shape
(an off-by-one in field-offset slicing silently miscompiles `static X: T = …`), and the
`ConstMemoryStatic` consumer-pointee recovery (a wrong storage type is a silent
miscompile + an alignment hazard).

After d2.2, `emit_generic_c` emits a complete, host-compilable-and-runnable TU for every
non-`dyn` model the bridge produces, **except** a narrow set of d2.3 carve-outs (below).
**Task e (diag-log → first green ledger fixture)** is unblocked: a `&str` literal lowers
as the carrier `(struct rust_str){ .data = (const uint8_t *)&CONST_MEM_NN, .len = <N> }`
over a `uint8_t[N]` `ConstMemoryStatic` (the 1-byte `u8` data reconstructs as
`uint8_t[N]` — same-type, sound).

### d2.3 carve-outs (stay fail-loud, distinct labels)

These shapes are genuinely hard under the pinned flags and are **rare**; each gets a
distinct `task d2.3` label so the follow-on task replaces only its own arms:

- **`Const::Bytes` / `Const::EnumValue` whose value carries a relocation that cannot be
  written into `_raw`** — a multi-view enum const (or an enum-containing aggregate
  const) with a **pointer/reloc payload**: `_raw = {bytes}` can't encode `&Y` at a
  sub-element. Label: `"Const enum/aggregate with relocation payload: task d2.3"`.
- **Inline bool-niche enum const** (`Option<bool>::None`-shaped): an **inline** enum
  has no `union` to write `_raw` through, and the single niche field's logical C type
  is `_Bool` (where `(_Bool){2}` normalizes). Label: `"Const::EnumValue (inline
  bool-niche): task d2.3"`. (A **multi-view** bool-niche const **is** handled — `_raw`
  writes byte `2` directly, and the d2.1 discriminant read uses raw storage.)
- **`ConstMemoryStatic` referenced at multiple incompatible pointee types** (the
  content-dedup collision): `"ConstMemoryStatic at incompatible pointee types: task
  d2.3"`.
- **`Bytes`-over-`Ty::Union`** (and, transitively, an aggregate const whose
  reconstruction recurses into a union field) — a C union const can initialize only
  one member, and **size equality alone is not byte-preserving**: a size-spanning
  *aggregate* member has internal padding the source-field designators don't cover, so
  the byte image isn't faithfully reproduced. Sound reconstruction needs a
  byte-covering member (a `_raw`-style union member) — deferred to keep
  `types.rs`/`layout_emit.rs` untouched (d2.3 may add a union `_raw` member the way
  Change A does for enums). Label: `"Const union byte-image: task d2.3"`.
- **Typed-valtree `Const::Aggregate` over `Ty::Union`** — `Const::Aggregate(Vec<ConstId>)`
  carries **no active `FieldIdx`** (`const_value.rs:38-39`); only the *runtime*
  `AggregateKind::Union(TyId, FieldIdx)` (`rvalue.rs:207-211`) records which member is
  live, and `pretty.rs:714-718` already **rejects** a const aggregate consumed by
  `Ty::Union` for exactly this reason. It cannot be lowered soundly without an IR/bridge
  shape change (out of scope). Label: `"Const::Aggregate over a union (no active field):
  task d2.3"`. (This is a **distinct label** from the `Bytes`-over-`Ty::Union` carve-out
  above — both defer, but for different reasons: this one lacks the active field, that
  one lacks a byte-covering member.)

### Out of scope (vtable task, unchanged)

`dyn` / vtable / virtual dispatch: a `Const::Aggregate` reference carrier to a
`Ty::Dynamic{..}` pointee → `Unsupported("Const::Aggregate reference carrier to a dyn
pointee: vtable task")`; every other `dyn` arm stays as d1/d2 left it.

## Input Context

### Design source — `docs/discussions/regseq-model/generic-c-backend.md`

- **§12 `Const` → C initializers** (lines 915–935): the per-variant lowering table.
  d2.2 implements the `Bytes`, `Aggregate`, and `EnumValue` rows; the scalar rows +
  `NullPtr` + `Reloc` stand from d1.
- **§13 Enum C representation** (lines 939–1005): the union-of-views form. Change A
  adds the `_raw` union member to multi-view enums; Changes B/E reuse the d2.1
  navigator (`enum_emit::{enum_shape, enum_is_inline, enum_variant_field, enum_tag}`).
- **§14 Aggregate field-offset reproduction** (lines 1009–1039): the §14-ordered,
  ZST-omitted, padding-filled compound-literal construction Change B (typed
  reconstruction) and Change D (typed valtree) follow. **§14 nuance**: `FieldLayout`
  vectors are **source/MIR order**, while physical emission is **offset-sorted** —
  Change B/D pair `children[i]` / byte-slices with `FieldLayout[i]` in **source
  order** and designate by the field's C name (so offset-sorting is irrelevant to
  pairing).
- **§15 Declaration order** (lines 1043–1111): bank-3 type defs are topo-sorted by
  by-value containment. Change F re-types `ConstMemoryStatic`s but adds **no new
  aggregate type** beyond what the pointee type already requires (the reconstructed
  pointee type is an existing model `Ty`, already discovered/ordered).

### Code anchors (confirm line numbers at HEAD `4d66bfa`)

- `crates/regseq-driver/src/bridge/const_.rs` — the bridge const path (read-only,
  the IR contract):
  - `:51-122` `translate_ty_const_value` (valtree → `Const::Aggregate` for
    tuple/array(non-u8)/struct, `Const::EnumValue` for enums, `Const::Bytes` for
    `[u8;N]`); `:263-310` `translate_static_init` (every non-scalar static →
    `Const::Bytes`, line 307); `:335-388` `translate_const_value`
    (`ConstValue::Slice`/`Indirect` → the carrier + `ConstMemoryStatic`);
    `:390-405` `wrap_reference_const` (the `[Reloc, Uint(meta)]` carrier, **only**
    for `TyKind::Ref(..)`; `:441-444` raw pointers stay bare `Const::Reloc`);
    `:520-573` `allocation_bytes` (pointer-size `Byte::Reloc` runs, `:550-554`);
    `:145-152` the **content-hash dedup** (Change F's collision source);
    `:203-235` `synthesise_const_memory_static` + the Phase-5 instance build
    (`ty: Array(u8, byte_len)`, `init: Some(Bytes)`, `:221-230`).
- `crates/regseq-backends/src/generic_c/body_emit.rs` — the const seam d2.2 fills:
  - `:1959-1967` `static_initializer` (bank-5 entry → `const_expr`); `:1975-2008`
    `const_expr` (the value-site dispatch; `Bytes`/`Aggregate` arms at `:2001-2002`,
    `EnumValue` at `:2003-2006`); `:2027-2105` `enum_value_expr` (d2.1's
    single-member dispatch; multi-member fall-through `:2099-2103`); `:2107-2134`
    `lower_enum_payload`; `:2176-2210` `niche_coinciding_field` (the bool-niche reject
    `:2193-2201`); `:2237-2248` `niche_storage_kind`; `:2258-2281` `render_tag_value`;
    `:2283-2297` `render_discriminant`; `:2319-2360` `reloc_designator` (**unchanged**;
    the consumer cast resolves correctly once Change F re-types the target).
  - the d1 scalar renderers `render_int` / `render_uint` / `render_float` (reused by
    Change B's scalar-from-bytes decode); the assign-site `assign_enum` (`:840-885`)
    serialization logic Change E mirrors for the `_raw` byte image.
- `crates/regseq-backends/src/generic_c/enum_emit.rs` — the d2.1 navigator + the
  union-of-views definition:
  - the multi-view `union { … } u;` emission (`:98-127`); the `_Static_assert`
    sizeof/_Alignof guards (`:131-138`). **Change A** inserts the `_raw` member
    after the view loop (`:124`, before `} u;` at `:126`) and adds a navigator entry
    `enum_raw_member(model, shape) -> Option<String>` (the `u._raw` member path, `None`
    for inline / single-view).
  - `enum_shape` (`:280`), `enum_is_inline` (`:293`), `enum_variant_field` (`:305`),
    `enum_tag` (`:326`) + the `EnumTag` enum (`:255`: `View{member_path, scalar_ty}`
    `:259`, `Niche{offset, scalar_ty}` `:268`, `None` `:275`) — reused by Changes B/E
    exclusively (the single-source-of-truth contract).
- `crates/regseq-backends/src/generic_c/layout_emit.rs`:
  - `field_c_name_ty(model, ty, idx) -> Option<(String, TyId)>` (`:152`) — the §14
    field-name + field-type resolver Change B/D reuse; `is_zst` (`:139`).
- `crates/regseq-backends/src/generic_c/types.rs`:
  - `c_type(model, names, ty)` (`:64`); `declare(model, names, ty, decl)` (`:76`);
    `slice_data_ptr_type(...)` (`:299`, the mutability/UnsafeCell-correct `.data`
    pointer spelling — **Change C uses this, not a hardcoded `const E *`**);
    `emit_slice_struct` (`:310`); `unsized_pointee_struct_name` (`:225`).
- `crates/regseq-backends/src/generic_c/mod.rs` — the banks:
  - `bank3_type_definitions` (`:699`); `bank4_declarations` (`:913-964`, the per-static
    `extern` at `:947-957`); `bank5_definitions` (`:982-1042`, the per-static `[const]
    T X = init;` at `:1017-1037`). **Change F** drives the declared type + init of each
    `ConstMemoryStatic` from the pre-pass map.
  - `discover` (`:353-410`) / `topo_sort` (`:418-526`) — **unchanged** (Change F
    re-types to existing model `Ty`s, already discovered/ordered).
- IR surface (read-only — bridge certificates):
  - `crates/regseq-model/src/const_value.rs` — `Const::{…, Bytes(Vec<Byte>),
    Aggregate(Vec<ConstId>), EnumValue{variant_idx, payload}, NullPtr(TyId),
    Reloc(InstanceRef)}` (`:20-51`); `Byte::{Uninit, Value(u8), Reloc(InstanceRef)}`
    (`:69-76`).
  - `crates/regseq-model/src/target.rs` — `TargetInfo.endian: Endian` (`:53`;
    `Endian::{Little, Big}` `:19-23`) — **Change B's scalar decode keys on this**, not
    hardcoded little-endian. `model.target` at `model.rs:61`.
  - `crates/regseq-model/src/model.rs` — `size_align_of(ty)` (`:1831`) for scalar /
    pointer widths; `model.target` (`:61`).
  - `crates/regseq-model/src/{ty.rs, layout.rs}` — `Ty::{…, Array(elem,n), Tuple,
    Struct, Union, Slice, Str, Enum, Ptr, Ref, FnPtr}`; `FieldLayout{name, ty,
    offset}`; `Layout{size, align, pack, kind, fields}`; the `VariantLayout` /
    `TagScalar` / `TagEncoding` (d2.1 anchors).
  - `docs/discussions/regseq-model/spec.md` §9.7 (the closed `Const` contract);
    §11.1/§11.2 (`tag_value` / niche formula — read as a certificate, never
    re-derived).

### Prior tasks

- **d2** (`regseq-backends-12`, `19f5b25`) — left `Const::Bytes` / `Const::Aggregate`
  arms fail-loud (`task d2.2`); the `Const::Reloc` body path (`reloc_designator`) is
  d1 + unchanged.
- **d2.1** (`regseq-backends-13`, `c8cadaa`) — §13 enum tag-ops + single-member
  `Const::EnumValue` + the bool-niche raw-storage soundness fix. d2.2 builds directly
  on the `enum_emit` navigator, `render_tag_value` / `render_discriminant`,
  `niche_storage_kind`, and the `assign_enum` tag-then-payload sequence (Change E
  mirrors it as a `_raw` byte image).
- **d1** (`-10`/`-11`) — the scalar `Const` renderers, `reloc_designator`,
  `InstanceSymbols`, `NullPtr`/`Reloc` lowering (all reused/unchanged).

### Test surface

- `crates/codegen-coverage-runner/tests/generic_c_bodies.rs` — the compile-**and-run**
  harness. d2.2 adds: a bridged `&str`-literal fixture (task-e shape); a bridged
  tuple-static fixture; a bridged struct-static fixture; a hand-built multi-view
  `Direct` `Const::EnumValue` fixture (the `_raw` path, scalar payload); a hand-built
  multi-view bool-niche enum const fixture (proves `_raw` writes byte 2 + the d2.1
  raw-storage discriminant read agree); a hand-built non-char `ConstMemoryStatic`
  fixture (e.g. a `&[u32; 4]` const — proves Change F re-typing end-to-end).
- `crates/regseq-backends/src/generic_c/mod_tests.rs` — the hand-built structural
  tests. d2.2 **flips** the three current d2.2 negatives to positives
  (`bytes_static_initializer_is_unsupported` `:2101`,
  `reloc_bearing_bytes_const_is_unsupported_d2_2` `:3185`,
  `const_aggregate_reference_carrier_is_unsupported_d2_2` `:3207`) and adds the d2.3
  carve-out negatives. The c2 / d2.1 enum-emit tests that assert the **exact** struct
  text are **updated** for the new `_raw` member (a deliberate representation
  extension — the size/align asserts are unchanged).

## Implementation Notes

### Change A: the `_raw` union member + navigator entry (`enum_emit.rs`)

Add `unsigned char _raw[<layout.size>];` as a member of every **multi-view** enum's
`union u` (the `views.len() >= 2` branch, after the view loop at `:124`, before `} u;`
at `:126`). This is the **only** representation change; it is layout-neutral (the views
are already full-object size `N`; `_raw` is alignment 1; the outer
`aligned(layout.align)` + the `_Static_assert(sizeof==size / _Alignof==align)` guards
are unchanged and still pass).

Add a navigator entry `pub(super) fn enum_raw_member(model, shape) -> Option<String>`:
`Some("u._raw")` for a multi-view enum (`!enum_is_inline`), `None` for an inline /
single-view / `Empty` enum (which has no union → no `_raw`; const init uses the
inline / d2.1 path or the d2.3 carve-out). **No other navigator change** — the body
side spells the `_raw` member only through this, never a re-derived name.

**Soundness (validated by a Codex design consult):** initializing `.u._raw` then
reading `.u._tag.tag` / `.u._v<v>.<f>` is gnu11 union type-punning, **allowed** under
`-fstrict-aliasing` because the read is **through the union object** (the d2.1
navigator reads members in place; it never takes `&x.u._v<v>.<f>` and dereferences a
typed pointer later). The c2 / d2.1 enum-emit text-assertion tests are updated for the
added member.

### Change B: the `Const::Bytes` byte-image walker (`body_emit.rs`)

Replace the `const_expr` `Bytes` arm (`:2001`) with `bytes_to_typed_literal(model,
names, syms, bytes, consumer_ty) -> Result<String>`, dispatching on
`model.types[consumer_ty]`:

- **Scalars** (`Bool` / `Char` / `Int(k)` / `Uint(k)` / `Float(k)`): decode
  `size_of(consumer_ty)` bytes per `model.target.endian` (little ⇒ LE, big ⇒ BE) into
  the bit pattern, then the d1 renderer (`render_int` sign-extends per width;
  `render_uint`; `render_float` hex-float; `Bool` → `0`/`1` from the byte; `Char` →
  the `uint32_t` literal). `Byte::Uninit` decodes as `0` (§12). A `Byte::Reloc` where
  a scalar is expected is malformed → `Unsupported`.
- **Pointers** (`Ptr` / `Ref` / `FnPtr`): if the `size_of(ptr)` bytes are a complete
  `Byte::Reloc(r)` run (all the same `r`) → `reloc_designator(r, consumer_ty)` (the
  d1 `&<target>` cast); else all-zero (and no `Reloc`) → the typed-`NullPtr` form
  (`((T)0)`); else (mixed / partial reloc) → `Unsupported`.
- **`Tuple` / `Struct` / non-`u8` `Array`** (typed reconstruction): walk
  `layout.fields` in **source/MIR order**, slice `bytes[f.offset .. f.offset +
  size_of(f.ty)]` per **non-ZST** field, recurse, and designate by the field's C name
  (`layout_emit::field_c_name_ty` for tuple/struct; `[i]` for an array element).
  ZST fields contribute no designator. Emit `(struct <agg>){ .<f0> = <recon0>, … }` for
  struct/tuple; for an array, `(<arr-typename>){ <recon0>, … }` where `<arr-typename>` is
  the **declarator-aware** array type-name `types::declare(model, names, elem, "[<n>]")`
  (**not** a bare `<c_type(elem)>[<n>]`, invalid C for an array-of-array / fn-pointer
  element); at a static-init position the cast is omitted (`= { … }`). (Array element
  size from `size_of(elem)`.)
- **`Union`** → `Unsupported("Const union byte-image: task d2.3")`. (Size equality
  alone is not byte-preserving — a size-spanning *aggregate* member has internal
  padding the source-field designators don't cover; sound reconstruction needs a
  byte-covering union member, deferred. An aggregate whose reconstruction recurses
  into a union field transitively defers here.)
- **`Enum`**: if **inline** (`enum_is_inline`) → reconstruct the inline fields (the
  d2.1 single-view path; the niche cell decodes as its scalar — defer inline
  bool-niche to d2.3); if **multi-view** → `(struct <E>){ .u._raw = { <bytes> } }`
  via `enum_raw_member` **iff** the bytes carry **no** `Byte::Reloc` (else
  `"Const enum/aggregate with relocation payload: task d2.3"`). The `_raw` byte image
  is the literal `bytes` (no tag decoding needed — the stored bytes already encode the
  active variant).

`size_of(ty)` / `pointer_size` come from `model.size_align_of(ty)` (cached). The walker
is purely positional + layout-driven; no tag/discriminant re-derivation.

### Change C: the `Const::Aggregate` reference carrier (`body_emit.rs`)

In `const_expr`, when `Const::Aggregate(children)` AND the consumer is `Ty::Ref(pointee,
_)`: expect `children == [Reloc(_), Uint(Usize, meta)]` (defensive `Unsupported` on any
other shape). Dispatch on `model.types[pointee]`:

- **sized pointee** → `reloc_designator(children[0], consumer_ty)` (the cast is to the
  `Ref`'s pointer type; the `Uint(0)` placeholder is dropped — but `children[1]` is
  still lowered through `const_expr` so a deferred shape fails loud);
- **`Ty::Str`** → `(struct rust_str){ .data = (<slice_data_ptr_type for u8>)<reloc>,
  .len = (size_t)<meta> }`;
- **`Ty::Slice(elem)`** → `(struct <fat>){ .data = (<slice_data_ptr_type for elem>)<reloc>,
  .len = (size_t)<meta> }`, `<fat>` from `unsized_pointee_struct_name`;
- **`Ty::Dynamic{..}`** → `Unsupported("Const::Aggregate reference carrier to a dyn
  pointee: vtable task")`.

The `.data` pointer type is **`slice_data_ptr_type`** (`types.rs:299`), not a hardcoded
`const E *` — it carries the correct mutability / `UnsafeCell` flavor. The `<reloc>`
points at the (Change-F-re-typed) `ConstMemoryStatic`, so the emitted address has the
right type and alignment.

### Change D: the `Const::Aggregate` typed valtree aggregate (`body_emit.rs`)

In `const_expr`, when `Const::Aggregate(children)` AND the consumer is **not**
`Ty::Ref(..)`: dispatch on `model.types[consumer_ty]`:

- **`Tuple` / `Struct`** → `(struct <agg>){ .<f0> = <c0>, … }`, pairing `children[i]`
  with `FieldLayout[i]` (**source order**), each non-ZST child lowered through
  `const_expr` at the field's `FieldLayout.ty`, designated by `field_c_name_ty`. ZST
  children recursed (fail-loud) but omitted from designators.
- **non-`u8` `Array(elem, n)`** → `(<arr-typename>){ <c0>, … }`, each child at `elem`,
  where `<arr-typename>` is the **declarator-aware** array type-name `types::declare(model,
  names, elem, "[<n>]")` (**not** a bare `<c_type(elem)>[<n>]`, invalid C for an
  array-of-array / fn-pointer element); the cast is omitted at a static-init position.
- **`Union`** → `Unsupported("Const::Aggregate over a union (no active field): task
  d2.3")` — `Const::Aggregate(Vec<ConstId>)` carries no active `FieldIdx`
  (`const_value.rs:38-39`); `pretty.rs:714-718` rejects this for the same reason. (A
  union STATIC arrives as `Const::Bytes`, which Change B **also** defers — `"Const union
  byte-image: task d2.3"`.)
- **`Enum`** consumer → defensive `Unsupported` (the bridge uses `EnumValue` for enums,
  `const_.rs:91-103`).
- **anything else** → defensive `Unsupported`.

### Change E: multi-member `Const::EnumValue` (`body_emit.rs`)

Extend `enum_value_expr`'s multi-member fall-through (`:2099-2103`). The single-member
cases d2.1 already handles (inline fieldless tag-only; inline niche-coinciding non-Bool;
multi-view fieldless `Direct`) stay on their current paths. For the handed-off cases:

- **Multi-view enum, fully byte-serializable value** (a `Direct` payload-bearing
  variant with **scalar** payload, or a niche niched/untagged variant — including
  **bool-niche**): **serialize the value to a `layout.size` byte image** (mirror
  `assign_enum`'s tag-then-payload logic, but into bytes):
  - write `VariantLayout[v].tag_value` (when `Some`) at `TagScalar.offset` over its
    raw storage width (`niche_storage_kind`), endian per `model.target.endian`;
  - write each non-ZST payload child's bytes at its `FieldLayout.offset` (each scalar
    child serialized via its bit pattern; a **non-scalar / `Reloc` / `Bytes` child** →
    `"Const enum/aggregate with relocation payload: task d2.3"`);
  - emit `(struct <E>){ .u._raw = { <byte image> } }` via `enum_raw_member`.
  This handles **bool-niche multi-view** (byte `2` written directly into `_raw`; the
  d2.1 discriminant read uses raw storage — sound).
- **Inline enum** (no union, so no `_raw`): the single-member cases are d2.1's; an
  inline bool-niche niched variant → `"Const::EnumValue (inline bool-niche): task
  d2.3"` (the existing `niche_coinciding_field` Bool reject, relabelled).
- **Multi-view enum with a non-serializable (pointer/`Reloc`) payload** → `"Const
  enum/aggregate with relocation payload: task d2.3"`.

### Change F: `ConstMemoryStatic` re-typing pre-pass (`mod.rs` + a new helper)

A backend **pre-pass** (run once before banks 3–5; a new
`const_memory_static_storage(model) -> Result<HashMap<InstanceId, StorageTy>>`,
deterministic) recovers each `ConstMemoryStatic`'s consumer **storage type**, expressed
as a **backend-local `StorageTy` descriptor** — *not* a model `TyId` — so it can spell a
shape (like `[u32; 4]`) that the model never interns:

```text
enum StorageTy {
    Sized(TyId),         // a sized pointee T (thin `&T` / `*const T`) — types::declare(T, ident)
    Array { elem: TyId, count: u64 },  // a slice/str backing `[elem; count]` —
                                       // types::declare(elem, "<ident>[<count>]") (declarator-aware)
}
```

- **Scan** every const reachable from instances (static inits, const values, function
  body operand constants), threading the consumer type the way `const_expr` does, and
  for each reference to a `ConstMemoryStatic` instance record the consumer **pointee
  storage type**:
  - a `Const::Aggregate([Reloc→CMS, Uint(meta)])` carrier over `Ty::Ref(pointee, _)`
    → `Sized(pointee)` if `pointee` is sized; for unsized `Str` → `Array{ elem: u8,
    count: meta }`; for unsized `Slice(elem)` → `Array{ elem, count: meta }`;
  - a bare `Const::Reloc→CMS` consumed by `*const T` / `*mut T` → `Sized(T)`;
  - a `Byte::Reloc→CMS` inside another (re-typed) static's reconstructed pointer field
    → `Sized(field's pointee type)`.
  The element `TyId` (`u8` / `elem` from `Ty::Slice(elem)`) **is** interned at HEAD
  (the bridge interns `Slice(elem)`, `const_.rs:474-477`); only the `Array(elem, meta)`
  *wrapper* is not — and the `StorageTy::Array` descriptor spells the C declaration via
  the **declarator-aware** `types::declare(model, names, elem, "<ident>[<count>]")` —
  exactly how `Ty::Array(elem, len)` spells *itself* (`types.rs:118-120` recurses
  `declare(elem, "{decl}[{len}]")`). The array brackets attach to the *declarator*, so a
  declarator-shaped element spells correctly — an array `[[u32;4]]` → `uint32_t X[N][4]`,
  a fn-pointer `[fn()]` → `<ret> (*X[N])(<args>)`; a bare string `<c_type(elem)>
  X[count]` would be **invalid C** for those (cf. the §15 slice-array-element case +
  `types.rs:317`). So **no array `TyId` is needed and the backend interns nothing**.
- **Resolve** per CMS: if all recorded descriptors are identical → use it; if they
  disagree (content-dedup collision, `const_.rs:145-152`) → the **whole emit** fails
  loud `"ConstMemoryStatic at incompatible pointee types: task d2.3"`.
- **Banks 3/4/5** (`bank4_declarations` `:947-957`, `bank5_definitions` `:1017-1037`):
  **always declare the `ConstMemoryStatic` from its resolved `StorageTy`** (instead of
  the bridge's `Array(u8, byte_len)`): `Sized(ty)` → `types::declare(model, names, ty,
  ident)`; `Array{elem, count}` → `types::declare(model, names, elem,
  "<ident>[<count>]")` (declarator-aware — **not** a bare `<c_type(elem)> X[count]`
  string, which is invalid C for an array-of-array / fn-pointer element). Reconstruct
  the init via Change B's walker at that storage shape (an `Array{elem, count}` walks
  `count` element slices of `size_of(elem)` bytes each, emitting `{ <recon0>, … }`). The
  reconstruction is **uniform**: a 1-byte `u8` / `i8` element → `uint8_t[N]` (so `&str`
  / `&[u8]` is unchanged and sound); a `Ty::Char` element (which renders as
  **`uint32_t`**, `types.rs:103-104`, `ty.rs:223-230`) → `uint32_t[count]`; any other
  element / sized pointee → its real C type. A non-CMS static (a user `static`) keeps
  its own declared `ty` and Change B reconstructs the init at that type. **No bank-3
  type addition, no interning** — `StorageTy` references only the already-interned
  element / pointee `TyId`s.

The reconstructed storage shape is naturally aligned (`uint32_t[4]` has alignment 4),
so `&CONST_MEM_NN` cast to the consumer pointer type is same-type **and** aligned —
**sound**, eliminating the prior draft's synthesised packed struct + manual alignment.

### Change G: §12/§13 Doc reconciliation (`generic-c-backend.md`)

The §12 `Bytes` row currently reads "`unsigned char[]` initializer: `Value(b)`→`b`;
`Uninit`→`0`; `Reloc(r)`→ the pointer-width address slot `&<r>`" (lines 931) — a flat
char-array lowering that is **unsound** for a non-`[u8;N]` consumer (a `uint8_t[N]`
initializer is not assignable to `struct T`, and the cast for a non-char consumer is
strict-aliasing UB). Update the §12 `Bytes` and `Aggregate` rows to describe the sound
lowering this task implements:

- **`Bytes(Vec<Byte>)`** → "reconstructed as a **typed** initializer for the consumer
  `Ty` (Change B byte-image walker): scalars decode per `model.target.endian`;
  aggregates slice per `FieldLayout.offset` and recurse; a `ConstMemoryStatic` is
  declared as its consumer pointee storage type (§ re-typing); a multi-view enum value
  initializes the §13 `_raw` union member; `Reloc` runs → `&<target>`. (A flat
  `unsigned char[]` is the special case of a `[u8; N]` consumer.)"
- **`Aggregate(children)`** → note the dual role: the `[Reloc, Uint]` **reference
  carrier** over `Ty::Ref` (thin / `rust_str` / `rust_slice_<T>` fat struct) vs the
  typed valtree aggregate over `Tuple` / `Struct` / non-`u8` `Array` (a union valtree
  is unsupported — no active field).
- **`EnumValue`** row + §13 "Tag operations" → note that a value-position enum const
  initializes the **`_raw` union member** of a multi-view enum (the union-of-views
  can't be const-initialized through separate `_tag`/`_v<v>` members).
- **§13** → document the `_raw` union member: every multi-view enum's `union u` carries
  `unsigned char _raw[size]` for const initialization; runtime reads use the typed
  views (gnu11 union type-punning). Layout-neutral.

Keep the delta **minimal** — reword the affected rows/notes, do not restructure §12/§13.

### Out of scope (stays fail-loud)

- d2.3 carve-outs (distinct labels, § "d2.3 carve-outs" above).
- vtable task: the `dyn`-pointee carrier + every existing `dyn` arm (unchanged).

## Non-Goals

- **No bridge / model change.** Change F is a backend pre-pass; the cleaner type-aware
  dedup key (`const_.rs:145-152`) is a **bridge** change, explicitly deferred (the
  collision fails loud).
- **A bounded §12/§13 Doc delta IS in scope (Change G).** The canonical spec's §12
  `Bytes` row currently describes the **unsound** flat `unsigned char[]` lowering
  (lines 923–933); d2.2 reconciles it to the typed-reconstruction + `_raw` reality
  (Change G). This is the only Doc edit; §13's "union-of-views" intent is unchanged
  (`_raw` is documented as the const-init emission detail). No `spec.md` (IR
  producer-surface) change.
- **No `types.rs` / `layout_emit.rs` change.** Unions get no `_raw` member (every
  `Bytes`-over-`Ty::Union` const defers to d2.3); the §14 field resolver +
  `slice_data_ptr_type` are **read**, not modified.
- **No re-derivation of enum member paths / tag values / niche formula** — Changes
  B/E call `enum_emit::{enum_shape, enum_is_inline, enum_variant_field, enum_tag,
  enum_raw_member}` and read `tag_value` / `TagScalar` / `FieldLayout` as
  certificates.
- **No discovery / topo-sort change, no type interning** — Change F's `StorageTy`
  descriptor references only already-interned element / pointee `TyId`s and spells the
  array case via the declarator-aware `types::declare(elem, "<ident>[<count>]")`, so it
  adds no model `Ty`, no discovery root, and no ordering edge.
- **No `may_alias`, no use-site `memcpy`** for the `ConstMemoryStatic` cast (per the
  Codex consult: `may_alias` is a non-portable extension that doesn't fix alignment;
  use-site `memcpy` can't yield a stable reference-const address). The sound mechanism
  is re-typing (Change F) + in-place union reads (Change A).
- **No new `EmitError` variant** — reuse `Unsupported(&'static str)`; remove every
  `task d2.2` label; add the distinct `task d2.3` + vtable-task labels.
- **No prelude / `regseq_main` / harness change.**
- **No layout / C-ABI recomputation** — field offsets, tag values, scalar/pointer
  widths, endianness, the array length `n` are bridge certificates.

## Test Plan

### B1: build + workspace green
- `cargo build -p regseq-backends -p codegen-coverage-runner` clean.
- `cargo test -p regseq-backends -p codegen-coverage-runner -- --test-threads=1` green
  — every d1 / c1 / c2 / d2 / d2.1 test passes (the c2/d2.1 enum-emit text assertions
  **updated** for the `_raw` member; size/align asserts unchanged) + the new d2.2
  tests.

### B2: end-to-end (compile-**and-run**)
- **`&str` literal** (bridged, task-e shape): `let s: &str = "hello"; if s.len() != 5
  || s.as_bytes()[1] != b'e' { panic!(); }`. Proves Change C carrier + the `u8`-byte
  `ConstMemoryStatic` (Change F reconstructs the 1-byte data as `uint8_t[N]`, sound).
- **Tuple-static** (bridged): `static PAIR: (u32, u64) = (1, 2);` read both back.
  Proves Change B typed reconstruction over a non-`u8` aggregate.
- **Struct-static, all-scalar** (bridged): a `repr(Rust)` struct (reordered fields)
  read field-by-field. Proves Change B §14-ordered reconstruction + access agree.
- **Non-char reference const** (bridged or hand-built): `static A: [u32; 4] = [10, 20,
  30, 40]; static R: &[u32; 4] = &A;` (or the closest bridgeable shape) → proves Change
  F re-types the `ConstMemoryStatic` to `uint32_t[4]` and the consumer reads u32s
  soundly + aligned. Assert `R[2] == 30`.
- **`Ty::Char` reference const** (hand-built — proves the `Ty::Char` ≠ `u8`
  reconstruction): a `&[char]` / `&char` pointing at a `ConstMemoryStatic` →
  proves Change F declares the storage as `uint32_t[n]` (4 bytes/elem), **not**
  `uint8_t[4n]`; assert a char element reads back correctly.
- **Declarator-shaped slice element** (hand-built — proves `StorageTy::Array`'s
  declarator-aware spelling): a `ConstMemoryStatic` backing `&[[u32; 4]]` (element is a
  C array) and/or `&[fn()]` (element is a fn pointer) → proves Change F emits a valid
  declaration (`uint32_t X[N][4]` / `<ret> (*X[N])(<args>)`), **not** the invalid
  `<c_type(elem)> X[N]` concat; assert it compiles and an element reads/calls correctly.
- **Declarator-shaped array CONST** (hand-built — proves Change B/D's array
  compound-literal type-name is declarator-aware, separate from the CMS case): a
  `static A: [[u32; 4]; 2] = …;` (Change B, Bytes-over-array-of-array) and/or a
  `[fn(); 2]` array const → proves the emitted array type-name is `uint32_t[2][4]` /
  `<ret> (*[2])(<args>)` via `types::declare(elem, "[<n>]")`, **not** the invalid
  `<c_type(elem)>[<n>]` concat; assert it compiles and an element reads/calls correctly.
- **Multi-view `Direct` `Const::EnumValue`, scalar payload** (hand-built): `enum E { A,
  B(u32) }`, `Const::EnumValue { variant_idx: 1, payload: [Uint(U32, 42)] }` → emits
  `(struct E){ .u._raw = { … } }`; a `regseq_main` matches it (`Discriminant` +
  `Downcast`+`Field`) and aborts on a wrong tag/payload.
- **Multi-view bool-niche enum const** (hand-built): a multi-view niche enum whose
  niche scalar is `Bool`, niched variant `tag_value = 2` → emits `_raw` with byte `2`;
  `regseq_main` reads the discriminant (d2.1 raw-storage path) and asserts it maps to
  the niched variant, not the `Some(true)` variant. (The d2.1 soundness-fix regression,
  now exercised through a const.)

### B3: structural
- Change A: a multi-view enum's emitted struct contains `unsigned char _raw[<size>];`
  inside `union u`; the size/align `_Static_assert`s are unchanged.
- Change B/D spell the **same** member names the aggregate definition emits
  (`field_c_name_ty` / `enum_variant_field`).
- Change C fat carrier: `(struct rust_str){ .data = <slice_data_ptr_type>, .len =
  (size_t)… }`; the `.data` cast type matches `slice_data_ptr_type` (incl. a `&mut`
  / `UnsafeCell` slice).
- Change F: a non-char `ConstMemoryStatic` is declared with its reconstructed pointee
  type (e.g. `uint32_t[4]`), not `uint8_t[16]`; a `&str` / `&[u8]` one reconstructs as
  `uint8_t[N]`; a `Ty::Char` one as `uint32_t[n]` (4 bytes/elem), never `uint8_t[4n]`.

### B4: deferred seams fail loud
- A multi-view enum const with a pointer payload → `Unsupported(_)` `task d2.3`
  (reloc payload).
- An inline bool-niche enum const (`Option<bool>::None`) → `Unsupported(_)` `task d2.3`
  (inline bool-niche).
- A typed-valtree `Const::Aggregate` consumed by a `Ty::Union` (hand-built — no active
  `FieldIdx`) → `Unsupported(_)` `task d2.3` (union, no active field).
- A `Bytes`-over-`Ty::Union` static (hand-built) → `Unsupported(_)` `task d2.3`
  (union byte-image).
- A `ConstMemoryStatic` referenced at two incompatible pointee types (hand-built) →
  `Unsupported(_)` `task d2.3` (incompatible pointee types).
- A `Const::Aggregate` carrier to a `dyn` pointee → `Unsupported(_)` vtable task.
- Confirm **no** `task d2.2` label remains in `body_emit.rs` / `mod.rs`.

### B5: determinism
- `emit_generic_c` byte-identical across two calls on a model with `ConstMemoryStatic`s
  (re-typed deterministically by the pre-pass), multi-view enum consts (`_raw` byte
  order), fat carriers, and typed aggregate consts ([[codegen-ledger-determinism]]).

### B6: fmt + clippy + diff scope
- `cargo fmt --all -- --check` clean; `cargo clippy -p regseq-backends -p
  codegen-coverage-runner --all-targets -- -D warnings` clean (scoped per
  [[clippy-all-targets-test-drift]]).
- Diff confined to `crates/regseq-backends/src/generic_c/{body_emit.rs, enum_emit.rs,
  mod.rs}` + the **bounded §12/§13 `generic-c-backend.md` delta** (Change G) + the
  runner behavioral test + the `mod_tests.rs` cases (flipped + new + the updated
  enum-emit text assertions) + the `tasks.json` registration. **No** `spec.md` (IR
  producer surface) / bridge / model / matrix / ledger / `test-ir-fixtures` / harness /
  `layout_emit.rs` / `types.rs` / `prelude.rs` change.

## Review

### Files to send to Codex
This spec; `docs/discussions/regseq-model/generic-c-backend.md` §12/§13/§14/§15;
`crates/regseq-backends/src/generic_c/{body_emit.rs, enum_emit.rs, mod.rs,
layout_emit.rs, types.rs, prelude.rs}`; the IR surface
`crates/regseq-model/src/{const_value.rs, layout.rs, ty.rs, target.rs, model.rs}` +
`docs/discussions/regseq-model/spec.md` §9.7/§11.1/§11.2;
`crates/regseq-driver/src/bridge/const_.rs:36-573`;
`crates/codegen-coverage-runner/{tests/generic_c_bodies.rs, src/toolchain.rs}`.

### Reviewer focus
- **`_raw` union member (Change A).** Layout-neutral (sizeof/_Alignof asserts hold);
  inserted only for multi-view enums; `enum_raw_member` is the single source of truth
  for the `u._raw` path; runtime reads stay **in-place** union accesses (no
  inactive-member-address-then-deref); the c2/d2.1 text assertions are updated. Is the
  gnu11 type-pun reasoning correct under the pinned flags (no `-fno-strict-aliasing`)?
- **Byte-image walker (Change B).** Every consumer arm correct; **endianness from
  `model.target.endian`** (not hardcoded); pointer-slot detection (full
  `pointer_size`-cell `Reloc(r)` run); aggregate slicing via `FieldLayout.offset` +
  `size_of(f.ty)` in **source order**, ZST omitted, uninit → 0; **`Ty::Union` →
  d2.3** (byte-image; size equality is not byte-preserving for a padded spanning
  member); enum via `_raw` (no-reloc) / inline. No off-by-one in the slice bounds
  (soundness-pass critical).
- **Reference carrier (Change C).** Carrier shape enforced; `.data` via
  **`slice_data_ptr_type`** (mutability/UnsafeCell-correct), not hardcoded `const`;
  `.len` `size_t`; sized/Str/Slice/dyn dispatch; the `<reloc>` points at the
  Change-F-re-typed target.
- **Typed valtree (Change D).** Source-order pairing (not offset-sorted); a
  `Ty::Union` consumer → d2.3 (no active `FieldIdx`, matching `pretty.rs:714-718`);
  enum-consumer rejected; ZST recursed but omitted.
- **Multi-member EnumValue (Change E).** Byte-image serialization mirrors `assign_enum`;
  bool-niche multi-view works via `_raw` (byte 2) + the d2.1 raw-storage read; reloc
  payload → d2.3; inline bool-niche → d2.3; reuses the navigator exclusively.
- **`ConstMemoryStatic` re-typing (Change F).** The pre-pass recovers the consumer
  pointee soundly across the carrier / bare-reloc / nested-byte-reloc cases; **uniform
  reconstruction** as the pointee storage type (1-byte `u8`/`i8` → `uint8_t[N]`;
  `Ty::Char` → `uint32_t[n]`; else the real type), sound + aligned; content-dedup
  collision → fail loud; **no bridge change, no new aggregate type, no discovery/topo
  edge**; deterministic.
- **Scope discipline (d2.2 vs d2.3 vs vtable).** Right arms filled; no `task d2.2`
  label left; the **five** distinct `task d2.3` labels + the vtable label; **only the
  bounded §12/§13 Doc delta (Change G)** — no `spec.md` / bridge / model / harness /
  `types.rs` / `layout_emit.rs` / `prelude.rs` change.
- **Verification honesty.** Compile + run over the `&str` literal + tuple/struct
  statics + the non-char reference const (Change F) + the `Ty::Char` CMS + the
  multi-view scalar + bool-niche enum consts; fail-loud negatives prove every carve-out
  (incl. `Bytes`-over-union + typed-valtree-union); no ledger green until task e;
  determinism; citation drift vs HEAD `4d66bfa`; scope creep.
- **Did the redesign fully close the two prior soundness holes?** (the union double-init
  and the non-char `ConstMemoryStatic` cast) — and did it introduce any new one
  (e.g. a `_raw` read path that escapes the union; a Change-F storage type that's still
  mis-aligned; a serialization endianness bug)?

### Adversarial prompt template
```
<task>
Review .harness/tasks/regseq-backends/14-generic-c-nonscalar-const.md — Phase-1 task
d2.2 of the generic-C backend, REDESIGNED after a first review found two architectural
soundness holes. d2.2 lowers all non-scalar §12 consts SOUNDLY under -std=gnu11 -fwrapv
-Wall with NO -fno-strict-aliasing (toolchain.rs:79-84). Two primitives (validated by a
prior Codex design consult): (1) a `_raw` union member — add `unsigned char
_raw[<size>]` to every MULTI-VIEW enum's `union u`, lower a fully-known enum const as
`(struct E){ .u._raw = {bytes} }`, runtime reads stay IN-PLACE union accesses
(`x.u._tag.tag`) = gnu11 type-punning, sound (this fixes the prior hole: you cannot set
.u._tag AND .u._v1 in one initializer — a later union designator overrides the earlier).
(2) re-type ConstMemoryStatic by its consumer pointee — the bridge makes every
ConstMemoryStatic a Static of Array(u8,byte_len) (const_.rs:221-230); reading it via
`(const T*)&X` for non-char T is strict-aliasing UB + misaligned, so a backend pre-pass
recovers each CMS's consumer pointee and UNIFORMLY declares the static as that
reconstructed storage type (1-byte u8/i8 → uint8_t[N], so &str/&[u8] is unchanged;
Ty::Char → uint32_t[n]; else the real type), sound+aligned; content-hash dedup
(const_.rs:145-152) collisions fail loud (task d2.3). Changes:
A `_raw` member + enum_raw_member navigator entry (enum_emit.rs); B the Const::Bytes
byte-image walker (scalars decoded per model.target.endian, NOT hardcoded LE; pointers
via pointer_size Reloc-run → &target; Tuple/Struct/non-u8-Array typed reconstruction
slicing by FieldLayout.offset in SOURCE order, ZST omitted; Ty::Union → d2.3 (byte-image,
size equality not byte-preserving); Enum via `.u._raw = {bytes}` multi-view / inline,
no-reloc only);
C the Const::Aggregate reference carrier ([Reloc,Uint(meta)] over Ty::Ref; .data via
slice_data_ptr_type NOT hardcoded const; Str/Slice fat struct; dyn → vtable task);
D typed valtree Const::Aggregate (Tuple/Struct/non-u8-Array, source-order pairing,
enum-consumer + Ty::Union-consumer rejected — Const::Aggregate has no active FieldIdx);
E multi-member Const::EnumValue (serialize tag+scalar payload to a byte image → `.u._raw`;
bool-niche MULTI-VIEW works via _raw byte 2 + d2.1 raw-storage read; reloc payload → d2.3;
inline bool-niche → d2.3); F the ConstMemoryStatic re-typing pre-pass (mod.rs banks
3/4/5; a backend-local StorageTy descriptor — Sized(TyId) or Array{elem TyId, count},
NO model interning since the bridge interns Slice(elem) but not Array(elem,meta);
UNIFORM reconstruction — 1-byte u8/i8 → uint8_t[N], Ty::Char → uint32_t[n], else real
type; NO char/u8 keep-shorthand); G a bounded §12/§13 generic-c-backend.md Doc delta
reconciling the §12 Bytes
row (currently the unsound flat unsigned char[]) to typed reconstruction + documenting
the §13 _raw member. CARVE-OUTS (distinct task d2.3 labels): reloc payload in
enum/aggregate const; inline bool-niche enum const; CMS at incompatible pointee types;
Bytes-over-Ty::Union (byte-image; size equality not byte-preserving); typed-valtree
Const::Aggregate over Ty::Union (no active FieldIdx). VTABLE TASK (unchanged): dyn carrier. This is a SPEC review: judge correctness, completeness, internal consistency,
sound scoping, faithful grounding. SPECIFICALLY verify the redesign fully closes BOTH
prior holes and introduces NO new one (a _raw read that escapes the union; a Change-F
storage type still misaligned; a byte serialization endianness bug; a slice off-by-one).
Flag wrong/unsound lowering, missing in-scope shapes, drifted citations, leaky
d2.2/d2.3/vtable seams.
Identify:
- _raw member (A): layout-neutral (sizeof/_Alignof asserts hold); multi-view only;
  enum_raw_member single source of truth; reads IN-PLACE through the union (no
  inactive-member-address-deref); gnu11 type-pun sound under the pinned flags; c2/d2.1
  text asserts updated.
- Byte-image walker (B): every consumer arm; endianness from model.target.endian;
  pointer Reloc-run detection; aggregate slicing FieldLayout.offset+size_of in SOURCE
  order, ZST/uninit; Ty::Union → d2.3 (byte-image); enum via _raw no-reloc; no off-by-one.
- Carrier (C): shape enforced; .data via slice_data_ptr_type (mut/UnsafeCell-correct)
  not hardcoded const; .len size_t; sized/Str/Slice/dyn; reloc → re-typed CMS.
- Typed valtree (D): source-order pairing; Ty::Union-consumer → d2.3 (no active
  FieldIdx); enum rejected; ZST recursed+omitted.
- Multi-member EnumValue (E): byte serialization mirrors assign_enum; bool-niche
  multi-view via _raw + d2.1 raw-storage read; reloc payload → d2.3; inline bool-niche
  → d2.3; navigator reuse only.
- ConstMemoryStatic re-typing (F): pre-pass recovers pointee as a backend-local
  StorageTy (Sized(TyId) / Array{elem TyId, count}, no model interning); UNIFORM
  reconstruction (u8/i8→uint8_t[N], Ty::Char→uint32_t[n], else real type; no
  keep-shorthand), sound+aligned; dedup collision fail-loud; no bridge/model/
  new-aggregate/interning/discovery change; deterministic.
- Doc delta (G): bounded §12/§13 reword (Bytes row → typed reconstruction; _raw
  documented); no spec.md (IR producer surface) change.
- Scope discipline d2.2 vs d2.3 vs vtable: right arms; no task-d2.2 left; distinct d2.3
  + vtable labels; ONLY the bounded §12/§13 Doc delta; no spec.md/bridge/model/harness/
  types.rs/layout_emit.rs/prelude change.
- Verification honesty (run over &str + tuple/struct statics + non-char reference const
  + Ty::Char CMS + multi-view scalar & bool-niche enum consts; fail-loud negatives per
  carve-out incl. union + typed-union; no ledger green until e); determinism; scope
  creep; citation drift vs HEAD 4d66bfa.
- Did the redesign fully close BOTH prior holes (union double-init; non-char CMS cast)
  and introduce no new one?
Read: docs/discussions/regseq-model/generic-c-backend.md §12/§13/§14/§15;
crates/regseq-backends/src/generic_c/{body_emit,enum_emit,mod,layout_emit,types,
prelude}.rs; crates/regseq-model/src/{const_value,layout,ty,target,model}.rs +
docs/discussions/regseq-model/spec.md §9.7/§11.1/§11.2; crates/regseq-driver/src/bridge/
const_.rs:36-573; crates/codegen-coverage-runner/{tests/generic_c_bodies.rs,
src/toolchain.rs}.
</task>

<grounding_rules>
- Cite file:line for every claim. Mark hypotheses as hypotheses.
- Don't speculate about code/spec you didn't read.
</grounding_rules>

<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
RAW-UNION-MEMBER (layout-neutral; multi-view only; single source of truth; in-place union reads; gnu11 type-pun sound; text asserts updated): PASS / FAIL with citation
BYTE-IMAGE-WALKER (every arm; endianness; pointer-slot; source-order slicing; ZST/uninit; Ty::Union → d2.3 byte-image; enum via _raw no-reloc; no off-by-one): PASS / FAIL with citation
REFERENCE-CARRIER (shape; .data slice_data_ptr_type not hardcoded const; .len size_t; sized/Str/Slice/dyn; re-typed CMS target): PASS / FAIL with citation
TYPED-VALTREE (source-order pairing; Ty::Union-consumer → d2.3 no-active-field; enum rejected; ZST recursed+omitted): PASS / FAIL with citation
MULTI-MEMBER-ENUMVALUE (byte serialization; bool-niche multi-view via _raw; reloc payload + inline bool-niche → d2.3; navigator reuse): PASS / FAIL with citation
CONSTMEMORYSTATIC-RETYPE (pre-pass recovers pointee as backend-local StorageTy Sized/Array no-interning; UNIFORM reconstruction u8/i8→uint8_t[N], Ty::Char→uint32_t[n], else real type; dedup collision fail-loud; no bridge/model/new-aggregate/discovery; deterministic): PASS / FAIL with citation
DOC-DELTA-G (bounded §12/§13 reword to typed reconstruction + _raw; no spec.md change): PASS / FAIL with citation
SCOPE-DISCIPLINE (d2.2 vs d2.3 vs vtable; no task-d2.2 left; distinct labels; ONLY the bounded §12/§13 Doc delta; no spec.md/bridge/model/harness/types/layout_emit/prelude change): PASS / FAIL with citation
VERIFICATION-HONESTY (run over &str + tuple/struct + non-char ref + Ty::Char CMS + multi-view scalar & bool-niche enum consts; fail-loud negatives incl. union + typed-union; no ledger green until e): PASS / FAIL with citation
PRIOR-HOLES-CLOSED (union double-init; non-char CMS cast both fully resolved; no new hole introduced): PASS / FAIL with citation
DETERMINISM: PASS / FAIL
SCOPE-CREEP: PASS / FAIL
CITATION DRIFT: list or "none"
NEW ISSUES: numbered or "none"
REQUIRED FIXES: numbered (only if VERDICT != APPROVED)
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, decide, stop.
</default_follow_through_policy>
```

## Quality Gate

- `cargo build -p regseq-backends -p codegen-coverage-runner` clean.
- `cargo fmt --all -- --check` + `cargo clippy -p regseq-backends -p
  codegen-coverage-runner --all-targets -- -D warnings` clean.
- `cargo test -p regseq-backends -p codegen-coverage-runner -- --test-threads=1` green
  (every d1/c1/c2/d2/d2.1 test passes; the c2/d2.1 enum-emit text assertions updated
  for `_raw`; the three d2.2 negatives flipped to positive; the five d2.3 negatives
  added).
- The B2 fixtures (`&str` literal, tuple/struct statics, non-char reference const,
  `Ty::Char` CMS, multi-view scalar + bool-niche enum consts) compile **and**
  `regseq_main` exits 0; the §13/§14 `_Static_assert`s hold; the d2.3 + vtable negatives
  return `EmitError::Unsupported` with the right distinct labels.
- `emit_generic_c` byte-stable; ledger verdict unchanged (`not-pass` — task e flips
  it); no Doc-1 flip.
- `git diff --stat` confined to `{body_emit.rs, enum_emit.rs, mod.rs}` + the bounded
  §12/§13 `generic-c-backend.md` delta + the runner test + the `mod_tests.rs` cases +
  registration.
- Codex Task Review: APPROVED or APPROVED_WITH_MINOR_FIXES.
- **Mandatory post-close Codex soundness pass** ([[assessment-soundness-pass]]) —
  d2.2 is layout-invariant-touching (byte-image decoding off-by-ones; the `_raw` /
  re-typing soundness); green compile-and-run fixtures can mask a field-slice or
  alignment hole.

## Acceptance Criteria

1. **`_raw` union member** (Change A, `enum_emit.rs`): every multi-view enum's `union
   u` carries `unsigned char _raw[<layout.size>];`; `enum_raw_member` is the single
   source of truth for the `u._raw` path (`None` for inline/single-view/`Empty`);
   layout-neutral (the §13 size/align `_Static_assert`s pass); runtime reads stay
   in-place union accesses; the c2/d2.1 enum-emit text assertions are updated.
2. **Byte-image walker** (Change B, `body_emit.rs`): `bytes_to_typed_literal`
   dispatches on the consumer `Ty` — scalars decode per `model.target.endian` into the
   d1 renderers; pointers match a `pointer_size` `Byte::Reloc(r)` run → `&<target>` /
   all-zero → typed null / else `Unsupported`; `Tuple`/`Struct`/non-`u8` `Array` slice
   per `FieldLayout.offset` (source order) and recurse, ZST omitted, uninit → 0;
   `Ty::Union` → d2.3 (`"Const union byte-image"` — size equality is not
   byte-preserving for a padded spanning member); `Ty::Enum` via `.u._raw = {bytes}`
   (multi-view, no-reloc) / inline. No layout/tag re-derivation.
3. **Reference carrier** (Change C): `[Reloc, Uint(meta)]` over `Ty::Ref(pointee)` —
   sized → reloc designator cast to the consumer pointer type; `Str`/`Slice` → the
   §3.2 fat struct with `.data` via `slice_data_ptr_type` and `.len` `size_t`;
   `Dynamic` → vtable-task label.
4. **Typed valtree** (Change D): `Const::Aggregate(children)` over a non-`Ref` aggregate
   → §14-ordered designated literal, **source-order child↔field pairing**, ZST recursed
   but omitted; a `Ty::Union` consumer → `task d2.3` (no active `FieldIdx` in
   `Const::Aggregate`); an enum-consumer defensively rejected.
5. **Multi-member `Const::EnumValue`** (Change E): a fully byte-serializable multi-view
   value (scalar `Direct` payload; niche niched/untagged — **including bool-niche**)
   serializes tag + payload to a `layout.size` byte image (mirroring `assign_enum`,
   endian per target) → `.u._raw = {bytes}`; a reloc/pointer payload → `task d2.3`; an
   inline bool-niche niched variant → `task d2.3`; the navigator is the only member-path
   source.
6. **`ConstMemoryStatic` re-typing** (Change F, `mod.rs` + pre-pass): a deterministic
   pre-pass recovers each `ConstMemoryStatic`'s consumer pointee storage as a
   **backend-local `StorageTy` descriptor** (`Sized(TyId)` for a sized pointee /
   `Array{elem: TyId, count}` for a slice/str backing — referencing only
   already-interned element/pointee `TyId`s, **never** interning an `Array(elem, meta)`
   `TyId`); the static is declared from it (`Sized` → `types::declare(ty, ident)`;
   `Array` → the **declarator-aware** `types::declare(elem, "<ident>[<count>]")`, not a
   bare `<c_type(elem)> X[count]` string) and its init reconstructed at that shape
   (uniform — a
   1-byte `u8`/`i8` element → `uint8_t[N]`, so `&str`/`&[u8]` is unchanged; `Ty::Char` →
   `uint32_t[count]`; any other → its real C type), sound + naturally aligned; a
   content-dedup multi-incompatible-type collision → the whole emit fails loud (`task
   d2.3`); no bridge/model change, no new aggregate type, no interning, no discovery/topo
   edge.
7. **Soundness**: the redesign closes **both** prior holes — no const initializes two
   members of an enum's `union u`, and no `ConstMemoryStatic` is read through a
   mismatched-type pointer (the storage type matches the consumer pointee); the `_raw`
   reads stay through the union object; the re-typed static is naturally aligned. The
   pinned `cc` flags (no `-fno-strict-aliasing`) are respected; no `may_alias` /
   use-site `memcpy`.
8. **Verification + scope**: the B2 fixtures compile **and** run (exit 0) — including the
   `Ty::Char` / `&[char]` `ConstMemoryStatic` reconstruction; the d2.3 (incl. the
   typed-valtree-union rejection) + vtable negatives fail loud with distinct labels; no
   `task d2.2` label remains; determinism holds; **no scope creep** beyond the bounded
   §12/§13 Doc delta (Change G) — no `spec.md` / bridge / model / harness / `types.rs` /
   `layout_emit.rs` / `prelude.rs` change; diff confined to the Quality-Gate file set.
