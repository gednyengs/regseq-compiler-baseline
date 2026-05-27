# regseq-backends-12: Generic-C structured data — places, aggregates & slice/str fat pointers (Phase-1 task d2)

## Description

Replace the **non-enum structured-data** `EmitError::Unsupported` seams the d1
body emitter left in `body_emit.rs` with real lowering: §4 place projections
(`Field` / `Index` / `ConstantIndex` / `Subslice`) + §14 aggregate field access,
the §10 `Aggregate` / `Repeat` rvalues, the §10 `UnaryOp::PtrMetadata` slot read,
the §11 slice/str fat-pointer **construction** (`Unsize` + `Ref`/`RawPtr` to an
unsized slice/str pointee + `AggregateKind::RawPtr`), and the now-unblocked §10.1
`*WithOverflow` result `(T, bool)`. This is **task d2** of the Phase-1
a→b→c1→c2→d1→**d2**→d2.1→d2.2→e sequence — the structured-data half of the original
"(d) bodies", split from d1 (user-approved 2026-05-27, mirroring c→c1/c2).

d2 is the **body value/place lowering** for sized structured data + runtime slices.
It does **not** add any const lowering beyond d1's scalars — all non-scalar const
work is task d2.2 (see deferral 2). The split has **three** deferrals, all
user-approved (2026-05-27):

1. **Enum tag-ops are task d2.1** (the next task). d2 does **not** touch the §13
   union-of-views: `Downcast(VariantIdx)` place projection (and the `Field` that the
   I-Downcast-followed-by-Field invariant guarantees follows it), `Discriminant(place)`
   on an **enum** predecessor, `StatementKind::SetDiscriminant`,
   `Rvalue::Aggregate(AggregateKind::Enum, ..)`, and `Const::EnumValue` stay
   `EmitError::Unsupported`. d2.1 is the layout-invariant-touching set and gets a
   dedicated Codex soundness pass ([[assessment-soundness-pass]]).
2. **All non-scalar const lowering is task d2.2.** The bridge represents **every
   non-scalar static / value const** as either `Const::Bytes` — a flat byte image,
   often carrying **embedded `Byte::Reloc`** pointer slots
   (`const_.rs:258-262`/`:307`/`:520-539`; a `[fn(); 2]` static → `Bytes([Reloc,
   Reloc])`, a struct static → mixed value+reloc bytes) — or, for reference-typed
   consts, a 2-element `Const::Aggregate([Reloc, Uint])` **carrier**
   (`const_.rs:390-405`) whose `Reloc` points at a `ConstMemoryStatic` **defined by a
   `Const::Bytes` init** (`const_.rs:203-230`/`:343-355`). So the carrier is
   **transitively coupled to a `Bytes` static definition** — it cannot be fully
   emitted (linked) without the byte-image static lowering. Lowering a flat byte
   image to a typed C initializer (reconstructing the consumer layout, mapping each
   pointer-width relocation to `&<symbol>`, handling alignment — even a pure-value
   struct `Bytes` is not a plain `unsigned char[N]` assignable to a `struct T`) is a
   substantial, self-contained sub-problem. **`Const::Bytes` (all forms) AND the
   `Const::Aggregate` reference carrier AND the `ConstMemoryStatic` / `Bytes`-init
   static *definitions* are the whole of task d2.2** and stay `EmitError::Unsupported`
   in d2. d2's run-fixtures exercise structured **bodies** (compute / index / slice
   over **runtime** values), so d2 needs no non-scalar const. (Consequence,
   acknowledged: a model containing a string/array/struct literal const, or a
   non-scalar static, does not fully emit until d2.2 — d2's fixtures avoid them.)
3. **`dyn` / vtable / virtual dispatch stays the later vtable task.** `Unsize` to a
   `dyn` pointee (`{ data, vtable = &VT }`), `PointerCoercion::ClosureFnPointer`
   (`&<synthesised shim>`), `PtrMetadata` of a `dyn` pointer (`.vtable`),
   `Ref`/`RawPtr`/`Deref` of a `dyn` pointer, and all vtable-static / `virtual_idx`-shim
   / `Indirect`-through-vtable emission remain `EmitError::Unsupported`. d2 builds
   **only** the slice/str fat pointer (`{ data, len }`).

After d2, `emit_generic_c` emits a complete, host-compilable-and-runnable TU for any
model whose bodies use the d1 scalar/pointer/control-flow subset **plus** sized
structured data (structs / tuples / arrays / closures / unions) and runtime slices —
everything except enums (d2.1), non-scalar consts (d2.2), and `dyn` (the vtable
task). Verification continues the d1 pattern: bridge a structured-data Rust function,
emit the TU, compile **and run** a `void regseq_main(void)` that computes a structured
value and `abort()`s on a wrong result — a clean exit (code 0) proves the
field-offset / aggregate / slice lowering reproduced rustc's layout and computed
correctly (the §14 `_Static_assert`s additionally guard layout at compile time). The
first *coverage-ledger* green fixture is still **task e** (it needs the §8 diag-log
stdout oracle + §7.1 call-site interception).

### Scope boundaries (flagged for review — see § Review)

1. **Non-enum structured data + runtime slice/str fat pointers.** d2 lowers: the §4
   `Field` (struct / tuple / closure / union predecessor) / `Index` / `ConstantIndex`
   / `Subslice` projections + §14 field access; `Rvalue::Aggregate` for
   `AggregateKind::{Array, Tuple, Struct, Union, Closure, RawPtr}`; `Rvalue::Repeat`;
   `UnaryOp::PtrMetadata` (slice/str `.len`); `Rvalue::Discriminant` on a **non-enum**
   predecessor (→ constant `0`, `spec.md` §21.2.8); `PointerCoercion::Unsize` (slice /
   array→slice / str); `Ref`/`RawPtr` to an unsized **slice/str** pointee (and a sized
   array `Subslice` under a `Ref`); the §10.1 `*WithOverflow` result `(T, bool)`.
   **No const work beyond d1's scalars.**
2. **Deferred to d2.1 (enums — stay fail-loud).** `Downcast(VariantIdx)` place
   projection (with the `Field` that follows it per the I-Downcast-followed-by-Field
   invariant, `place.rs:61-63`); `Discriminant(place)` on an **enum** predecessor;
   `SetDiscriminant`; `Rvalue::Aggregate(AggregateKind::Enum, ..)`; `Const::EnumValue`.
   **The `Field` arm never sees an enum predecessor in d2**: an enum field is always
   reached through a `Downcast`, which still errors first — so d2's `Field` lowering
   assumes a struct / tuple / closure / union predecessor (the §19.4 narrowing
   requires a prior `Downcast` for an enum-variant field, `body_ty.rs:88-117`).
3. **Deferred to d2.2 (non-scalar const — stays fail-loud).** Every `Const::Bytes`
   (value/uninit **and** relocation-bearing) and every `Const::Aggregate` (the
   reference carrier and any other shape) → `EmitError::Unsupported` at `const_expr`'s
   `Bytes` / `Aggregate` arms, relabelled to d2.2. The bank-5 *definition* of a
   `Static{init: Some(Const::Bytes(_))}` therefore fails loud **automatically** (it
   routes through the `Bytes` arm) — no new guard. A `Const::Reloc` to a
   `ConstMemoryStatic` is **not** itself a deferred seam: it emits its address
   normally via the **unchanged** `reloc_designator` (`body_emit.rs:928`); it is the
   referenced static's own `Bytes` *definition* that fails loud (d2.2).
4. **Deferred to the later dyn/vtable task (stays fail-loud).** `Unsize` to a `dyn`
   pointee; `PointerCoercion::ClosureFnPointer`; `PtrMetadata` of a `dyn` pointer;
   `Ref`/`RawPtr`/`Deref` of a `dyn` pointer; vtable-static / `virtual_idx`-shim /
   `Indirect`-through-vtable emission. d2 builds the slice/str fat pointer only.
5. **Fat-pointer `Deref` is `.data`, not `(*p)`** (a d1 latent gap, never exercised —
   a backend lowering rule, **boundary 5**, consistent with §3.2's fat struct + §4's
   `Deref`/`Index` rows; §4 line 198's `*p` is written for a *thin* pointer and is
   **not** edited by d2). A `Deref` of a **thin** pointer is `(*p)` (d1, unchanged); a
   `Deref` of a **fat** (slice/str) pointer yields the `.data` slot, combined with the
   trailing `Index` / `ConstantIndex` / `Subslice`. The `.len` slot is read for the
   `from_end` index forms and `PtrMetadata`. (`Deref` of a `dyn` pointer →
   `Unsupported`, vtable task.)
6. **C forbids array assignment.** An rvalue whose **destination type is an array**
   (`Aggregate(Array)`, `Repeat`) cannot lower to `dst = <expr>;` (C rejects array
   assignment); it lowers at the **assign-statement level** to a per-element loop or
   `memcpy` from a compound literal, mirroring the existing `Transmute` assign-site
   special case (`body_emit.rs:311`). A by-value array **operand** at a call site (C
   array decay) is a rare Rust-ABI edge — flag it; fail loud if it cannot be spelled
   soundly.
7. **Place-type narrowing — extend the backend's own walk; do NOT call the model's
   recovery.** `regseq_model::place_ty` exists (`body_ty.rs:67`) but is **not directly
   reusable**: its `unwrap_recovered` is `#[cfg(test)]` only (`body_ty.rs:46-55`), it
   returns a 3-variant `TypeRecovery`, and its array-`Subslice` result is `Ty::Array
   (elem, to-from)` **only when that shortened array is already interned** — otherwise
   it tracks an internal `derived_array_elem` and leaves `current` unchanged
   (`body_ty.rs:143-155`). So d2 **extends the backend's own `place_ty`/`place_lvalue`
   walk** (a single shared walk returning both the C-lvalue and the running type) and
   **mirrors the §19.4 narrowing table** (`body_ty.rs:75-168` is the reference to
   agree with, not a function to call). The backend does not need the interned
   shortened-array `TyId`: for an array `Subslice` it tracks the **element type** (for
   a follow-on `Index`) + the **`from` offset** (for the lvalue), which is all the
   lowering consumes.

## Input Context

### Design source — `docs/discussions/regseq-model/generic-c-backend.md`

- **§3.2 Fat-pointer C struct shapes** (lines 116–145): `struct rust_str { const
  uint8_t *data; size_t len; }` (prelude, monomorphic), `struct rust_slice_<T> {
  const T *data; size_t len; }` (per-element). Both ship from c1
  (`types::emit_slice_struct` at `types.rs:296-314`; `rust_str` in the prelude). d2
  builds **values** of these (`{ data, len }`); field names are `data` / `len`. The
  `trait_obj_<Trait>` (`dyn`) shape is emitted but its construction is the vtable task
  (boundary 4).
- **§4 `PlaceElem` C lvalue forms** (lines 190–208): `Field(idx, _)` → `.fname` (name
  from the predecessor layout's `FieldLayout.name`); `Index(local)` → `[<local
  lvalue>]`; `ConstantIndex{offset, from_end:false}` → `[offset]`, `{from_end:true}` →
  `[len - offset]` (slice runtime `len` = the fat pointer's `.len`); `Subslice` → a
  sub-slice expression. `Deref` → `*p` (written for a thin pointer; the **fat** case
  is the backend rule, boundary 5 — d2 does not edit §4). `Downcast` is d2.1.
- **§10 `Rvalue` C-emission** (lines 843–865): `Repeat(op, n)` → `for (size_t i = 0; i
  < n; i++) dst[i] = <op>;` (`n` post-mono constant); `UnaryOp::PtrMetadata` → the §3.2
  metadata slot (`.len` for slice/str; `.vtable` is the vtable task); `Discriminant
  (place)` → §13 for an enum (d2.1), **constant `0` for a non-enum** (`spec.md`
  §21.2.8); `Aggregate(kind, ops)` → struct / union / tuple / array compound literal
  with §14 placement, closure → captures literal, `AggregateKind::RawPtr` → the fat
  raw pointer from `(data, metadata)` (thin pointee → `data` only). **`AggregateKind::
  Enum` is d2.1.**
- **§10.1 `BinOp`** (lines 867–886): the `*WithOverflow` row — `__builtin_{add,sub,
  mul}_overflow(a, b, &tmp)` → the IR's `(T, bool)`. d1 deferred this (the result
  aggregate + its field access were unavailable); d2 unblocks it (Change G).
- **§11 Cast C-emission** (lines 889–911): `PointerCoercion::Unsize` → slice /
  array→`{data, len}`, str→`{data, len}` (d2), `dyn`→`{data, vtable}` (vtable task);
  `ClosureFnPointer` (vtable task). The §10 `Ref`/`RawPtr` to an unsized pointee
  "builds the §3.2 fat pointer per §11 Unsize" (line 853) — the same construction,
  reached from the place side.
- **§14 Aggregate field-offset reproduction** (lines 1009–1039): the seven-step
  technique c1/c2 built (`layout_emit.rs`); d2 reuses its **field-name + field-order**
  logic for both **access** (`.fname`) and **construction** (compound literals). Step
  7 field access: direct member access for the natural case; `memcpy` to / from a
  typed temporary "when accessing through enum byte storage (d2.1) or a **packed /
  misaligned field** (avoids alignment UB)". The construction side places fields in
  §14 order, ZST omitted, padding zero-filled — **designated initializers** (`(struct
  T){ ._0 = …, .f = … }`) make field order irrelevant and zero-fill padding
  deterministically (recommended; the implementer may pick the mechanism — the
  contract is byte-faithful §14 placement).
- **§12 `Const`** (lines 915–935) / **§15 Declaration order** (lines 1043–1111):
  unchanged by d2 — d1's scalar `Const` lowering stands; all non-scalar `Const` is
  d2.2 (deferral 2). d1 widened the discovery roots + broadened discovery through
  sized pointer edges, and the slice/str fat structs + their element aggregates are
  discovered + ordered by c1's machinery (the §15 fat-pointer exception, lines
  1073–1084). d2 adds **no** discovery root and **no** ordering edge.

### Code anchors (confirm line numbers at HEAD)

- `crates/regseq-backends/src/generic_c/body_emit.rs` — the d1 emitter d2 fills:
  - `place_lvalue` (364–393) — the `Deref` arm (368) is thin-only today; Change A
    splits thin vs. fat and implements the `Field` (369) / `Index` (372) /
    `ConstantIndex` (375) / `Subslice` (380) arms. `Downcast` (385) stays
    `Unsupported` (d2.1).
  - `place_ty` (147–158) — **extended** to the full non-enum narrowing (boundary 7);
    the call sites (`is_zst` / int-info / float-kind / `assign` / `Ref` / `call`, lines
    174/182/203/308/438/472/741) consume it.
  - `assign` (305–336) — the statement dispatch; the `Transmute`→`memcpy` special case
    (311–324) is the precedent for the array / `*WithOverflow` assign-site forms
    (Change C/D/G).
  - `rvalue_expr` (432–453) — the `Repeat` (448) / `Discriminant` (449) / `Aggregate`
    (452) seams (Change C/D/E); `Ref`/`RawPtr`-to-unsized (438–442, Change F).
  - `unary_op` (466–487) — `PtrMetadata` seam (485, Change E).
  - `binary_op` (491–553) — the `*WithOverflow` seam (512–524, Change G).
  - `cast_expr` (559–606) — the `Unsize` (591–593) seam (Change F); `ClosureFnPointer`
    (594–596) stays `Unsupported` (vtable task).
  - `const_expr` (876–906) — **untouched by d2** except relabelling the deferral
    strings: `Bytes` (902) / `Aggregate` (903) → d2.2, `EnumValue` (904) → d2.1.
  - `is_zst` (137–142), `is_unsized` (457–462), `operand_ty` (163–168) — predicates d2
    reuses; `is_unsized` distinguishes the fat pointees.
- `crates/regseq-backends/src/generic_c/layout_emit.rs`:
  - `resolve_view` (91–135) — the **field-name + field-order** source of truth: tuple
    field → `_<idx>`, struct/union → `sanitize(FieldLayout.name)`, closure → resolved
    through its parallel capture `Ty::Struct`. d2's field **access** and **construction**
    must spell **the same names** the definition emits, so factor a shared
    `field_c_name(model, predecessor_ty, field_idx) -> String` (or expose the naming
    slice of `resolve_view`) rather than re-deriving — a name mismatch is a compile
    error (`offsetof` guard / `.field` access disagree).
  - `ViewField` (55–62), `emit_offset_fields` (219–259), `is_zst` (139–141) — the §14
    primitive; d2's construction side mirrors its emittable-field filter (ZST omitted)
    + field order.
- `crates/regseq-backends/src/generic_c/types.rs`:
  - `emit_slice_struct` (296–314) — the `rust_slice_<T>` shape; note the `.data` slot
    is spelled by `ptr_to(model, names, elem, const_data, false, "data")` (307), i.e.
    the **element pointer type** (`const T *`), which d2's fat construction must cast to
    for the `.data` operand. `unsized_pointee_struct_name` (225–247) — resolves a
    `Ref`/`Ptr`-to-unsized to its fat-struct C name (`rust_str` / `rust_slice_<T>` /
    `trait_obj_<Trait>`); d2 reuses it to spell the fat struct + detect/reject `dyn`.
  - `c_type` / `declare` (64–78) — the element / aggregate spelling for compound
    literals + `Repeat` element types.
- `crates/regseq-backends/src/generic_c/prelude.rs`:
  - `helper_block` (124–181) — `struct regseq_overflow_<T> { <T> v; _Bool overflow; }`
    + `regseq_{add,sub,mul}_<T>(a, b)` (142–156). Change G calls the helper into a temp,
    then writes the `(T, bool)` result tuple's two fields (`._0 = tmp.v`, `._1 =
    tmp.overflow`).
- IR surface (read-only — trust as bridge certificates):
  - `crates/regseq-model/src/place.rs` — `PlaceElem::{Deref, Field(FieldIdx, TyId),
    Index(LocalId), ConstantIndex{offset, min_length, from_end}, Subslice{from, to,
    from_end}, Downcast(VariantIdx)}` (70–131); the **I-Deref-first** + **I-Downcast-
    followed-by-Field** invariants (56–63); `Field`'s inline `TyId` is the post-narrow
    field type (80–84) — the authoritative narrowing the backend walk uses for Field.
  - `crates/regseq-model/src/body_ty.rs` — `place_ty` (67) + the §19.4 narrowing the
    backend mirrors (Field 88–117, Index 118–127, ConstantIndex 128–142, Subslice
    143–161, Downcast 162–167); `TypeRecovery` (35–44) + the **test-only**
    `unwrap_recovered` (46–55) — read for the narrowing rules, **not called** (boundary
    7).
  - `crates/regseq-model/src/rvalue.rs` — `Rvalue::{Repeat, Discriminant(Place),
    Aggregate(AggregateKind, Box<[Operand]>)}` (19–75); `AggregateKind::{Array(TyId),
    Tuple(TyId), Struct(TyId), Enum(TyId, VariantIdx), Union(TyId, FieldIdx),
    Closure(TyId), RawPtr(TyId, Mutability)}` (185–222 — d2 = all but `Enum`); `UnOp::
    PtrMetadata` (180); `PointerCoercion::Unsize` (104–112).
  - `crates/regseq-model/src/ty.rs` / `layout.rs` — `Ty::{Array(TyId, n), Slice(TyId),
    Str, Dynamic{..}, …}` (the array length `n`); `FieldLayout { name, ty, offset }`,
    `Layout { size, align, pack, kind }`, `model.size_align_of(ty)`. Field offsets are
    **source/MIR order, not memory order** ([[methodology]]).
  - `crates/regseq-driver/src/bridge/const_.rs` (read-only — grounds the d2.2 deferral):
    `translate_static_init` (263–310, scalar → scalar `Const`, **else `Const::Bytes`**);
    `wrap_reference_const` (390–405, the `Aggregate([Reloc, Uint])` carrier);
    `synthesise_const_memory_static` (203–230) + the `Indirect` ref path (343–355, the
    carrier's `Reloc` → a `ConstMemoryStatic` with a `Bytes` init); `allocation_bytes`
    (520–539, embeds `Byte::Reloc`).

### Prior tasks

- `regseq-backends-10` (d1, `4a8e655`) / `-11` (d1.1, `d040b37`) — the §7 body
  envelope, the §7.1 terminators, the d1 value layer + the `EmitError::Unsupported`
  seams d2 fills, and the `InstanceSymbols` table d2's `&fn` construction reuses
  unchanged. d1 deferred `Const::Bytes`/`Aggregate` to "d2"; the d2.2 carve-out refines
  that — the body lowering is d2, the non-scalar const lowering is d2.2.
- `regseq-backends-06`/`08`/`09` (c1/c2/09) — the §14 field-offset primitive
  (`layout_emit`), the §13 enum representation (`enum_emit`, the union-of-views d2.1
  navigates), the `rust_slice_<T>` / `trait_obj` shapes, the discovery / topo-sort
  machinery. d2 **consumes** these; it adds no type-definition or discovery change.
- Doc-1 status reconciliation (`111da9b`): d2 flips **no** matrix status and adds
  **no** coverage-ledger green fixture (the first is **task e**). d2's behavioral test
  is a compile-and-**run** check in d1's home (`generic_c_bodies.rs`).

### Test surface

- `crates/codegen-coverage-runner/tests/generic_c_bodies.rs` — d1's behavioral harness
  (`try_bridge_fixture_path`; `toolchain::compile_and_run(tu, "regseq_main", cc)` →
  assert `exit_code == Some(0)`; the hand-built-model pattern). d2 adds structured-data
  fixtures + hand-built models here.
- `crates/regseq-backends/src/generic_c/mod_tests.rs` — the in-crate hand-built
  structural tests; d2 adds field-access / aggregate / slice-construction structural
  cases + the negative (d2.1 / d2.2 / vtable-task construct → `Unsupported`) cases.
  [[regseq-model-test-surfaces]]: the model's validator tests are out of scope.

## Implementation Notes

### Change A: place projections + fat-pointer `Deref` + extended narrowing (`body_emit.rs`)

Extend the backend's `place_ty` (147) and `place_lvalue` (364) to walk the full
non-enum projection chain — ideally a **single shared walk** returning both the
running C-lvalue string and the running type, so they cannot drift (the round-1
consistency concern). Mirror the §19.4 narrowing (`body_ty.rs:75-168`) without calling
it (boundary 7). Track, alongside the lvalue + type: the **fat-pointer base** (for
`.len`) and a pending **array-`Subslice` `from` offset** (for a follow-on index):

- **`Deref`** (I-Deref-first → always index 0, peels the head pointer):
  - **thin** pointer (to a *sized* pointee) → `(*<lv>)` (d1, unchanged); narrow to the
    pointee.
  - **fat** pointer (to `Str` / `Slice`) → the data slot `<lv>.data`; narrow to the
    unsized pointee. **Remember the fat base `<lv>`** for a trailing `from_end` index /
    `Subslice` `.len`.
  - **`Dynamic{..}`** → `EmitError::Unsupported("Deref of a dyn pointer (vtable task)")`.
- **`Field(idx, ty)`** on a struct / tuple / closure / union predecessor →
  `<lv>.<field_c_name(predecessor_ty, idx)>` (Change B); narrow to the carried `ty`
  (`place.rs:80-84`). An enum predecessor is unreachable in d2 (a prior `Downcast`
  errors first); assume non-enum.
- **`Index(local)`** → `<lv>[<local_c_name(index_local)>]` (the index `usize` local via
  `local_c_name`; add the pending array-`Subslice` `from` offset if set → `<lv>[from +
  i]`); narrow to the element type (array/slice element, or the tracked sub-array
  element).
- **`ConstantIndex{offset, from_end}`** → `[offset]` (`from_end:false`, + a pending
  array-Subslice `from`) or `[<base>.len - offset]` (`from_end:true`, slice — `<base>`
  the remembered fat base); narrow to the element type.
- **`Subslice{from, to, from_end}`** as a place lvalue (i.e. **not** under a `Ref`)
  occurs only on a **sized array** base (`from_end:false`): record the `from` offset and
  narrow the running element type to the array element (length `to-from`), so a
  follow-on `Index`/`ConstantIndex` offsets by `from`. (A **slice** `Subslice` produces
  an unsized place, materialised only under a `Ref` — Change F.)
- **`Downcast`** → `EmitError::Unsupported` (d2.1), unchanged.

### Change B: §14 field access + the shared field-name resolver (`body_emit.rs` + `layout_emit.rs`)

Add `pub(super) fn field_c_name(model, predecessor_ty, field_idx) -> String` to
`layout_emit` (or expose the naming slice of `resolve_view`) and call it from Change
A's `Field` arm, so **access spells the same field name the definition emits** (tuple
`_<idx>`, struct/union `sanitize(name)`, closure via the capture struct). §14 step-7:
direct member access (`.fname`) for the natural case. For a **packed / misaligned**
field where an aligned object is required (e.g. `Rvalue::Ref` of a packed-struct field
→ a misaligned pointer), use the §14.7 `memcpy`-to/from-typed-temporary, or fail loud
(`EmitError::Unsupported("address of a misaligned packed field (§14.7)")`) if it cannot
be spelled soundly — a plain value load/store of a packed member is sound directly
(gcc/clang honour `__attribute__((packed))`). Flag the exact §14.7 trigger for review.

### Change C: §10 `Aggregate` rvalue (`body_emit.rs`)

Lower `Rvalue::Aggregate(kind, ops)` for the non-enum kinds, placing each **non-ZST**
operand at its §14 field (ZST operands omitted, padding zero-filled) — **designated
initializers** recommended:

- **`Struct(ty)` / `Tuple(ty)` / `Closure(ty)`** → `(struct <agg_name>){ .<f0> = <op0>,
  … }` over the emittable fields (operands in MIR order; each mapped to its §14 name via
  Change B). Assignable by value → the `dst = <literal>;` path.
- **`Union(ty, field_idx)`** → `(union <agg_name>){ .<field> = <op> }` (the single
  active field).
- **`Array(ty)`** → C forbids array assignment (boundary 6): at the **assign site**,
  `memcpy(&dst, &(<elem>[N]){ <op0>, … }, sizeof dst)` or a per-element `dst[i] =
  <opi>;`. A ZST element writes nothing.
- **`RawPtr(ty, mut)`** → dispatch on the pointee:
  - **sized (thin)** → the raw pointer is `<ops[0]>` (the data pointer); the metadata
    operand is unit/ZST — drop it.
  - **slice/str (fat)** → `(struct <fat>){ .data = <ops[0]>, .len = <ops[1]> }`.
  - **`dyn`** → `EmitError::Unsupported` (vtable task).
- **`Enum(ty, v)`** → `EmitError::Unsupported` (d2.1).

Array-producing rvalues route through the assign-site dispatch (next to `Transmute`);
struct/tuple/union/closure/thin-rawptr/fat-rawptr literals are expressions on the `dst
= <expr>;` path.

### Change D: §10 `Repeat` (`body_emit.rs`)

`Rvalue::Repeat(op, n)` (array fill, `n` post-mono constant) → at the assign site: `for
(size_t <i> = 0; <i> < <n>; <i>++) <dst>[<i>] = <op>;`. A ZST element → no-op. The loop
variable name must not collide with a body local (a reserved prefix, e.g. `_ri`, or a
fresh name; document it). C forbids array assignment → an assign-statement form
(boundary 6), not an expression.

### Change E: §10 `PtrMetadata` + non-enum `Discriminant` (`body_emit.rs`)

- `UnaryOp::PtrMetadata` on a slice/str fat pointer → its `.len` slot (`<operand>.len`).
  On a `dyn` pointer → `EmitError::Unsupported` (vtable task). On a thin pointer →
  metadata is `()` (ZST destination, elided) — emit nothing (confirm the bridge only
  emits `PtrMetadata` on a fat pointer).
- `Rvalue::Discriminant(place)` on a **non-enum** predecessor → the constant `0`
  (`spec.md` §21.2.8); on an **enum** predecessor → `EmitError::Unsupported` (d2.1).
  Dispatch on the narrowed `place_ty`'s `Ty` (`Ty::Enum` → d2.1, else `0`).

### Change F: §11 slice/str fat-pointer construction (`body_emit.rs`)

`Unsize` and `Ref`/`RawPtr`-to-unsized build the **same** §3.2 fat pointer (spec line
853). The `.data` slot's C type is the **element pointer type** (`const T *` per
`emit_slice_struct`, `types.rs:307`) — cast the data operand to it (not to the fat
struct type):

- **`PointerCoercion::Unsize`** (`cast_expr`, 591) — operand is a thin pointer to a
  sized source; dest is fat:
  - `&[T; N]` → `&[T]`: `(struct <fat>){ .data = (<data-slot ptr type>)<op>, .len = <N>
    }` (`N` the array length).
  - `&[u8; N]` → `&str`: `(struct rust_str){ .data = (const uint8_t *)<op>, .len = <N> }`.
  - an already-fat operand being re-unsized → identity / copy (confirm the bridge shape).
  - `dyn` dest → `EmitError::Unsupported` (vtable task).
- **`Rvalue::Ref(_, place)` / `RawPtr(_, place)`** (`rvalue_expr`, 435) — currently
  errors on any unsized pointee (438). Replace:
  - place is a `Deref` of an existing fat pointer (re-borrow of a whole slice/str) →
    the existing fat pointer copied (`{ .data, .len }` identity).
  - place is a **slice** `Subslice{from, to, from_end}` → `(struct <fat>){ .data =
    (<data-slot ptr type>)(<base>.data + <from>), .len = <computed> }`, `<computed>` =
    `to - from` (`from_end:false`) or `<base>.len - <to> - <from>` (`from_end:true`,
    the slice case, `spec.md` §19.4 `[from .. len-to]`).
  - place is a **sized array** `Subslice` (the recovery narrows to a sized sub-array) →
    a **thin** pointer to the sub-array, `(<elem>(*)[<to-from>])&<base>[<from>]`.
  - place's pointee is `dyn` → `EmitError::Unsupported` (vtable task).

Reuse `unsized_pointee_struct_name` (`types.rs:225`) to spell `<fat>` + detect the
`dyn` reject. `ClosureFnPointer` (594) stays `Unsupported` (vtable task).

### Change G: §10.1 `*WithOverflow` result `(T, bool)` (`body_emit.rs`)

Replace the `binary_op` `*WithOverflow` `Unsupported` arm (512). The prelude
`regseq_{add,sub,mul}_<T>(a, b)` returns `struct regseq_overflow_<T> { T v; _Bool
overflow; }`; the destination is the Rust tuple `(T, bool)` (a C `struct tuple_<hash>`
with §14 fields `_0` = `T`, `_1` = `_Bool`). At the **assign site** (a multi-step
lowering, helper called **once**): compute the helper into a temporary, then write the
destination's two fields — `<dst>._0 = <tmp>.v; <dst>._1 = <tmp>.overflow;` (`_0`/`_1`
via the tuple §14 layout, `v`/`overflow` the helper struct's fields). `<T>` = the
operand's `IntInfo.ctype`. Use a fresh temp name that cannot collide with a body local.

### Out of scope (deferred seams that **stay** `EmitError::Unsupported`)

d2.1 (enums): `Downcast`, enum `Discriminant`, `SetDiscriminant`, `Aggregate::Enum`,
`Const::EnumValue`. d2.2 (non-scalar const): all `Const::Bytes`, all `Const::Aggregate`
(incl. the reference carrier), and the `Bytes`-init static *definitions* that route
through them (the `Const::Reloc` path is **unchanged** — only the referenced `Bytes`
static def fails loud). vtable task: `Unsize`-`dyn`, `ClosureFnPointer`, `PtrMetadata`-`dyn`,
`Ref`/`RawPtr`/`Deref`-`dyn`, vtable / shim / virtual-dispatch. Each keeps a distinct
static label so each later task replaces only its own arms.

## Non-Goals

- **No enum tag-ops (all d2.1)** — no `Downcast` access, enum `Discriminant`,
  `SetDiscriminant`, `Aggregate::Enum`, `Const::EnumValue`, §13 navigation;
  `enum_emit.rs` untouched.
- **No non-scalar const lowering (all d2.2)** — no `Const::Bytes` (value/uninit or
  relocation-bearing), no `Const::Aggregate` (incl. the reference carrier), no
  `Bytes`-init static *definitions* (they route through the deferred `Bytes` arm). d2
  does not touch `const_expr` beyond relabelling its existing fail-loud arms; the
  `Const::Reloc` path stays as d1 (unchanged).
- **No `dyn` / vtable / virtual dispatch (vtable task)** — no `Unsize`-`dyn`,
  `ClosureFnPointer`, `PtrMetadata`-`dyn`, `Ref`/`Deref`-`dyn`, vtable / shim / virtual
  dispatch.
- **No call to `regseq_model::place_ty`** (boundary 7) — the backend extends its own
  narrowing walk, mirroring the §19.4 table.
- **No type-definition / discovery / topo-sort / naming change** — d2 consumes the
  c1/c2/d1 names + shapes; no discovery root, no ordering edge. The shared
  `field_c_name` is a **read** of existing naming, not a new scheme.
- **No prelude / helper change** — the `*WithOverflow` / saturating-cast helpers ship
  from task b; Change G only wires the use-site.
- **No new `EmitError` variant** — reuse `Unsupported(&'static str)`; d2 removes the
  d2-scope arms and adds the d2.1 / d2.2 / vtable-task labels.
- **No spec / Doc change** — fat-pointer `Deref` is a backend lowering rule (boundary
  5), not a §4 edit.
- **No harness change** — verify through the existing `void regseq_main(void)` contract
  (`toolchain.rs:60-77`); `{i,u}32` exit-code stays deferred.
- **No layout / C-ABI recomputation** — trust `FieldLayout`, `Layout`, `size_align_of`,
  the array length `n`, the carried `Field` `TyId` as bridge certificates.
- **No deps; no bridge / model / matrix / ledger / `test-ir-fixtures` change.** d2 is
  confined to `crates/regseq-backends/src/generic_c/{body_emit.rs, layout_emit.rs}` +
  new behavioral / structural tests + the `tasks.json` registration.

## Test Plan

### B1: build + workspace green
- `cargo build -p regseq-backends -p codegen-coverage-runner` clean.
- `cargo test -p regseq-backends -p codegen-coverage-runner -- --test-threads=1` green —
  **all d1 / c1 / c2 tests still pass** (the field-name resolver is a read of existing
  naming; no type-definition output changes), plus the new d2 tests.

### B2: end-to-end structured-data behavior (compile-**and-run**)
- A `#[no_mangle] pub extern "C" fn regseq_main()` that (a) builds a struct / tuple and
  reads a field back computing a value, (b) builds + indexes a fixed array (`let a =
  [10u32, 20, 30]; if a[1] != 20 { panic!() }`), (c) takes a **runtime** slice (`let s:
  &[u32] = &a; if s[1] != 20 || s.len() != 3 { panic!() }`) — `panic!()` on any wrong
  result. `compile_and_run` → assert `exit_code == Some(0)`; the §14 `_Static_assert`s
  additionally fail the **compile** on a wrong layout. **Slice/str fixtures use runtime
  values (a `&[T]` borrow of a local array), not string/array literal consts** (those
  are `Const::Aggregate`/`Bytes` → d2.2).
- A `repr(Rust)` struct with **reordered** fields (source order ≠ memory order) — read
  each field; proves §14-ordered construction + access agree.
- A `checked_add`-style overflow-returning helper (Change G) → assert both `(T, bool)`
  components.
- Where the bridge cannot surface a shape (a `Subslice` / `from_end` `ConstantIndex`, a
  packed-field access, a `RawPtr` aggregate) → **hand-build the model** (the c1
  precedent) and assert the emitted TU compiles + the structural shape.

### B3: structural — names, order, fat pointers
- Field **access** names match the **definition** names for a tuple (`_0`/`_1`), a
  struct (`sanitize`), a closure (capture struct) — the same model emits both. A
  reordered `repr(Rust)` struct's access uses `.fname` (not a positional index).
- An `Unsize` of `&[u32; 4]` → `&[u32]` emits `(struct rust_slice_…){ .data = (const
  uint32_t *)…, .len = 4 }`; a `dyn` Unsize → `Unsupported`.
- **`str` construction (hand-built, no literal const).** A hand-built model whose body
  `Unsize`s a `&[u8; N]` → `&str` emits `(struct rust_str){ .data = (const uint8_t
  *)…, .len = <N> }`, and a `PtrMetadata` on a `&str` reads `<operand>.len`. A runtime
  `&str` without a string literal is awkward to bridge (a literal is a `Const::Aggregate`
  carrier → d2.2), so this `str` case is **hand-built** (the c1 precedent) — asserting
  the emitted TU compiles + the `rust_str` literal shape — rather than a bridged
  run-fixture.
- A fat-pointer `Deref`+`Index` emits `<base>.data[i]` (not `(*base)[i]`); a `from_end`
  `ConstantIndex` reads `<base>.len`.

### B4: the deferred seams fail loud
- One negative per family → `EmitError::Unsupported(_)` with the right label: **d2.1** —
  `Downcast` place, enum `Discriminant`, `SetDiscriminant`, `Aggregate::Enum`,
  `Const::EnumValue`. **d2.2** — a `Const::Bytes` (value-only and relocation-bearing), a
  `Const::Aggregate` (reference carrier), a `Bytes`-init static. **vtable task** —
  `Unsize`-`dyn`, `ClosureFnPointer`, `PtrMetadata`-`dyn`, `Ref`-`dyn`.

### B5: determinism
- `emit_generic_c` byte-identical across two calls on a structured-data model (field
  order, designated-initializer order, the `Repeat` loop var, the temp names all
  deterministic) ([[codegen-ledger-determinism]]).

### B6: fmt + clippy + diff scope
- `cargo fmt --all -- --check` clean; `cargo clippy -p regseq-backends -p
  codegen-coverage-runner --all-targets -- -D warnings` clean (scoped per
  [[clippy-all-targets-test-drift]]).
- Diff confined to `crates/regseq-backends/src/generic_c/{body_emit.rs, layout_emit.rs}`
  + the new runner behavioral test + the `mod_tests.rs` cases + the `tasks.json`
  registration. **No** spec / Doc / bridge / model / matrix / ledger / `test-ir-fixtures`
  / harness / type-definition / discovery / `const_expr`-logic change.

## Review

### Files to send to Codex
This spec; `docs/discussions/regseq-model/generic-c-backend.md` §3.2/§4/§10/§10.1/§11/
§12/§14/§15; `crates/regseq-backends/src/generic_c/{body_emit.rs, layout_emit.rs,
enum_emit.rs, types.rs, prelude.rs}`; the IR surface `crates/regseq-model/src/{place.rs,
body_ty.rs, rvalue.rs, const_value.rs, ty.rs, layout.rs}`; the bridge const lowering
`crates/regseq-driver/src/bridge/const_.rs`;
`crates/codegen-coverage-runner/tests/generic_c_bodies.rs`.

### Reviewer focus
- **Place walk + extended narrowing + fat `Deref` (boundary 4/5/7).** Does Change A
  extend the backend's own `place_ty`/`place_lvalue` (single shared walk) — NOT calling
  the test-only `unwrap_recovered` / the conditional model recovery — mirroring §19.4:
  thin (`(*p)`) vs fat (`.data`) Deref, the fat base remembered for `.len`, the array-
  `Subslice` `from` offset threaded into a follow-on index (tracking element + offset,
  not the interned shortened-array TyId)? Is `Field`-assumes-non-enum sound?
- **Field-name agreement (Change B).** Same names access ↔ definition via a shared
  resolver? §14.7 packed/misaligned handled or soundly failed loud?
- **Aggregate / array (Change C/D).** §14 order + ZST omission + zero-filled padding?
  Array-assignment prohibition at the assign site? **Thin** RawPtr (operand 0, metadata
  dropped) + **fat** RawPtr (`{data,len}`) + `dyn`-reject? Determinism of loop var / temp?
- **Fat-pointer construction (Change F).** `{data,len}` with the `.data` cast to the
  **element pointer type** (not the fat struct), correct lengths; **array** `Subslice`
  under a `Ref` = thin sub-array pointer (sized), **slice** `Subslice` = fat
  (`len-to-from`); reuses `unsized_pointee_struct_name`; rejects `dyn`?
- **`*WithOverflow` (Change G).** Helper once into a temp; both fields (`_0`=`v`,
  `_1`=`overflow`); `<T>` from the operand; no collision?
- **Scope discipline (d2 vs d2.1 vs d2.2 vs vtable task).** Exactly the right arms
  filled; **no const work in d2** (the carrier + `Bytes` + non-scalar statics all
  d2.2, since the carrier is transitively coupled to a `Bytes` static def); the three
  deferrals fail loud with distinct labels; no hidden coupling re-leaks (the slice
  fixtures use **runtime** `&[T]`, not literal consts)?
- **Verification honesty.** Compile + run a `void regseq_main` that aborts on a wrong
  result + the §14 `_Static_assert` guards — sound, and **free of any d2.2 const path**
  (no string/array literal const in the bridged fixtures)? Negatives prove fail-loud?
  No green ledger until e?
- Determinism; citation drift vs HEAD; scope creep vs Non-Goals.

### Adversarial prompt template
```
<task>
Review .harness/tasks/regseq-backends/12-generic-c-structured-data.md — Phase-1 task
d2 of the generic-C backend (REVISED twice; confirm fixes + look for anything new). d2
is the NON-ENUM, NON-CONST body structured-data lowering replacing d1's
EmitError::Unsupported seams in body_emit.rs: §4 Field/Index/ConstantIndex/Subslice
place projections + §14 field access (extending the backend's OWN place_ty walk,
mirroring §19.4 — NOT calling the test-only regseq_model::place_ty/unwrap_recovered);
§10 Aggregate (Array/Tuple/Struct/Union/Closure/RawPtr — NOT Enum) / Repeat; §10
PtrMetadata (slice/str .len) + non-enum Discriminant (→0); §11 slice/str fat-pointer
construction (Unsize + Ref/RawPtr-to-unsized + array/slice Subslice under a Ref); §10.1
*WithOverflow result (T,bool). NO const work beyond d1 scalars. THREE deferrals stay
fail-loud: ENUM tag-ops → d2.1; ALL non-scalar const (Const::Bytes + the
Const::Aggregate reference carrier + ConstMemoryStatic/Bytes-init static defs) → d2.2
(the carrier is transitively coupled to a Bytes static definition, so it can't link in
d2); dyn/vtable → the vtable task. Verification = bridge a structured-data fn → emit →
compile AND RUN a void regseq_main(void) that aborts on a wrong result → assert exit 0
(+ §14 _Static_assert guards); slice fixtures use RUNTIME &[T] (no literal consts).
This is a SPEC review: judge correctness, completeness, internal consistency, sound
scoping, faithful grounding. Flag wrong/unsound lowering, missing in-scope constructs,
drifted citations, leaky d2/d2.1/d2.2/vtable seams.
Identify:
- Place walk: extends the backend's OWN place_ty/place_lvalue (single shared walk),
  NOT calling test-only unwrap_recovered nor depending on the conditional model
  Subslice recovery; mirrors §19.4; thin (*p) vs fat .data Deref; fat base for .len;
  array-Subslice from-offset threaded into a follow-on index (tracks element+offset,
  not the interned shortened-array TyId); Field-assumes-non-enum sound.
- Field-name agreement: access==definition names via a shared layout_emit resolver;
  §14.7 packed handled or failed loud.
- Aggregate/array: §14 order/ZST/padding; array no-assign at the assign site; thin
  RawPtr (operand 0, metadata dropped) + fat RawPtr ({data,len}) + dyn-reject;
  determinism.
- Fat-pointer construction: {data,len} with .data cast to the ELEMENT pointer type
  (not the fat struct); lengths (N; to-from; len-to-from); array Subslice under Ref =
  thin sub-array ptr (sized) vs slice Subslice = fat; reuses unsized_pointee_struct_name;
  rejects dyn.
- *WithOverflow: helper once into temp, both (T,bool) fields (_0=v,_1=overflow), <T>
  from operand, no collision.
- Scope discipline d2 vs d2.1 vs d2.2 vs vtable task: NO const work in d2; right arms
  filled; three deferrals fail loud with distinct labels; no hidden coupling re-leaks
  (slice fixtures runtime &[T], not literal consts); verification free of any d2.2
  const path.
- Verification honesty (run + _Static_assert sound, no const-literal path); fail-loud
  negatives; no ledger green until e; determinism; scope creep vs Non-Goals; citation
  drift vs HEAD.
Read: docs/discussions/regseq-model/generic-c-backend.md §3.2/§4/§10/§10.1/§11/§12/§14/
§15; crates/regseq-backends/src/generic_c/{body_emit,layout_emit,enum_emit,types,
prelude}.rs; crates/regseq-model/src/{place,body_ty,rvalue,const_value,ty,layout}.rs;
crates/regseq-driver/src/bridge/const_.rs; crates/codegen-coverage-runner/tests/
generic_c_bodies.rs.
</task>

<grounding_rules>
- Cite file:line for every claim. Mark hypotheses as hypotheses.
- Don't speculate about code/spec you didn't read.
</grounding_rules>

<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
PLACE-WALK-AND-NARROWING (own walk, no test-only API; thin vs fat Deref; .len base; array-Subslice from-offset; Field assumes non-enum): PASS / FAIL with citation
FIELD-NAME-AGREEMENT (shared resolver; access==definition; §14.7 packed): PASS / FAIL with citation
AGGREGATE-AND-ARRAY (§14 order/ZST/padding; array no-assign; thin+fat+dyn RawPtr; determinism): PASS / FAIL with citation
FAT-POINTER-CONSTRUCTION (.data element-ptr cast; lengths; array Subslice thin vs slice fat; rejects dyn): PASS / FAIL with citation
WITH-OVERFLOW (helper-once temp; both fields; <T>; no collision): PASS / FAIL with citation
SCOPE-DISCIPLINE (NO const in d2; d2 vs d2.1 vs d2.2 vs vtable; distinct labels; no hidden coupling/leak): PASS / FAIL with citation
VERIFICATION-HONESTY (run + _Static_assert; no const-literal path; fail-loud negatives; no ledger green until e): PASS / FAIL with citation
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
  (all d1/c1/c2 tests unchanged + the new d2 tests).
- A bridged structured-data function emits a TU whose `void regseq_main` exits 0
  (correct field/aggregate/slice lowering) / aborts on a wrong result; the §14
  `_Static_assert`s hold; the deferred-construct negative tests return
  `EmitError::Unsupported` with the right label.
- `emit_generic_c` byte-stable; ledger verdict unchanged (`not-pass`); no Doc-1 flip.
- `git diff --stat` confined to the Change-A–G file set + the runner test + the
  `mod_tests.rs` cases + registration.
- Codex Task Review: APPROVED or APPROVED_WITH_MINOR_FIXES.

## Acceptance Criteria

1. **§4 places + extended narrowing + fat `Deref`** (`body_emit.rs`): the backend's own
   `place_ty`/`place_lvalue` extended (single shared walk mirroring §19.4, **not**
   calling the test-only `unwrap_recovered` / depending on the conditional model
   recovery); `Field` (struct/tuple/closure/union) / `Index` / `ConstantIndex` (both
   `from_end`) / `Subslice` lowered; a **fat** (slice/str) `Deref` emits the `.data`
   slot (`.len` for `from_end`), a **thin** Deref stays `(*p)`; an array-`Subslice` `from`
   offset threads into a follow-on index; the `Field` arm assumes non-enum (enum → d2.1).
2. **§14 field access + shared field-name resolver** (`layout_emit.rs` + `body_emit.rs`):
   access spells the same names the definition emits; §14.7 packed/misaligned handled or
   soundly failed loud.
3. **§10 `Aggregate`** (non-enum): `Struct`/`Tuple`/`Union`/`Closure` compound literals
   in §14 order (ZST omitted, padding zero-filled); `Array` + `Repeat` lowered at the
   assign site (C array-assignment prohibition); `RawPtr` — **thin** → operand 0 (metadata
   dropped), **slice/str** → `{data, len}`, **`dyn`** → `Unsupported` (vtable task).
   `Enum` → `Unsupported` (d2.1).
4. **§10 `PtrMetadata` (slice/str `.len`) + non-enum `Discriminant` (→ `0`)**;
   `dyn`-metadata → `Unsupported` (vtable task), enum `Discriminant` → `Unsupported`
   (d2.1).
5. **§11 slice/str fat-pointer construction**: `Unsize` + `Ref`/`RawPtr`-to-unsized-slice/str
   + **slice** `Subslice`-under-`Ref` build `{data, len}` with the `.data` cast to the
   **element pointer type** and the correct length; an **array** `Subslice` under a `Ref`
   is a thin sub-array pointer; `dyn` Unsize / `ClosureFnPointer` / `Ref`-`dyn` →
   `Unsupported` (vtable task).
6. **No const work beyond d1 scalars**: `const_expr` is untouched except relabelling its
   fail-loud arms — all `Const::Bytes` and all `Const::Aggregate` (incl. the reference
   carrier) → `EmitError::Unsupported` (d2.2), and a `Bytes`-init static *definition*
   fails loud automatically through the `Bytes` arm; `Const::EnumValue` → d2.1. The
   `Const::Reloc` path is unchanged from d1.
7. **§10.1 `*WithOverflow`** result `(T, bool)`: the prelude helper called once into a
   temp, both fields written (`_0`=`v`, `_1`=`overflow`), via the assign site.
8. **Fail-loud deferral seams**: every d2.1 construct (Downcast / enum-Discriminant /
   SetDiscriminant / Aggregate::Enum / EnumValue), every d2.2 construct (all
   `Const::Bytes` / all `Const::Aggregate` / `Bytes`-init static def), and every
   vtable-task construct (Unsize-`dyn` /
   ClosureFnPointer / PtrMetadata-`dyn` / Ref-`dyn` / Deref-`dyn`) returns
   `EmitError::Unsupported` with a distinct label; a negative test per family proves it.
9. **Verification**: a bridged structured-data function compiles **and runs** —
   `regseq_main` exits 0 iff the field/aggregate/slice lowering reproduced rustc's
   layout (the §14 `_Static_assert`s additionally guard at compile time); the bridged
   fixtures use **runtime** values only (no literal-const / d2.2 path); determinism
   holds; **no scope creep** — no enum / non-scalar-const / `dyn` / vtable lowering, no
   §13 / discovery / type-definition / prelude / harness / spec / Doc / bridge / model /
   matrix / ledger change; diff confined to the Quality-Gate file set.
