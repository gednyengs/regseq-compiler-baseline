# regseq-model-90: `render_const` for aggregate / enum / bytes / reloc variants (P1a-PP-04b)

## Description

Complete the stage-14 composer `render_const(ConstId, TyId) ->
String` by implementing the **recursive / structural** Const variants
left as `todo!()` placeholders by P1a-PP-04a (regseq-model-89):
`Aggregate`, `EnumValue`, `Bytes`, `NullPtr`, and `Reloc`. Per
D-14-4.9 LOCKED, recursive children render via
`render_const(child_id, per_field_ty)` where `per_field_ty` is
resolved by walking the parent consumer Ty's structural layout
(§10.1.8 per-sub-field Ty resolution table).

This task closes out the §10.1 dispatch surface together with
P1a-PP-04a; both tasks share the same single `render_const` method on
`PrettyPrinter<'_>`.

## Input Context

- `.harness/SESSION.md`
- `.harness/spec-batches/regseqmodel-phase-1a.md` (per-task scope row
  P1a-PP-04b)
- `docs/discussions/regseq-model/pretty-printer.md` §10.1 dispatch
  overview (pretty-printer.md:2610-2649), §10.1.3 `Aggregate`
  (pretty-printer.md:2950-3008), §10.1.4 `EnumValue`
  (pretty-printer.md:3009-3043), §10.1.5 `Bytes`
  (pretty-printer.md:3045-3078), §10.1.6 `NullPtr`
  (pretty-printer.md:3080-3128), §10.1.7 `Reloc`
  (pretty-printer.md:3129-3186), §10.1.8 type-context threading
  (pretty-printer.md:3187-3282 -- the per-sub-field Ty resolution
  table is the canonical source for child Ty derivation), §9.10
  specimens 91 - 109 (the recursive subset)
- `docs/discussions/regseq-model/spec.md` §9.7 (`Const` 11-variant
  enum + `Byte` 3-state enum + `InstanceRef` + canonical-form rules
  including the `Slice { data, meta }` -> `Aggregate([Reloc, Uint])`
  bridge dispatch), §11 (`Layout` + `LayoutKind` + `EnumShape` +
  `VariantLayout` + `FieldLayout` -- consumed by per-field Ty
  resolution and tuple-vs-named dispatch), §13.2 / §13.3 / §13.4
  (vtable + ConstMemoryStatic synthetic Items + their DefPath
  encodings)
- `crates/regseq-model/src/pretty.rs` (the existing `PrettyPrinter`
  shell amended by P1a-PP-04a)
- The `Const`, `Byte`, `InstanceRef`, `ConstInterner` types from
  regseq-model-DS-05 (P1a-DS-05)
- The synthesised vtable + ConstMemoryStatic Items from
  regseq-model-DS-08b (P1a-DS-08b) -- consumed by the Reloc dispatch
  via `model.item_synthetic_kinds[item]`
- `render_const` scalar / ZST arms from regseq-model-89 (P1a-PP-04a)
- `render_ty`, `render_def_path`, `render_generic_args` from
  P1a-PP-02 / stage 12
- `.harness/tasks/regseq-model/24-place-and-projection.md` (size /
  shape reference)

## Implementation Notes

Edit `crates/regseq-model/src/pretty.rs`. This task fills in the
`Aggregate / EnumValue / Bytes / NullPtr / Reloc` arms of the
`render_const` per-variant match laid out by P1a-PP-04a.

### §10.1.8 type-context threading -- per-sub-field Ty resolution (canonical)

Per D-14-4.9 LOCKED, the table at pretty-printer.md:3199-3205
specifies how recursion threads the consumer Ty:

| Parent consumer Ty | Per-child Ty resolution |
|---|---|
| `Ty::Array(elem, N)` | every child has Ty `elem` |
| `Ty::Tuple { fields }` | i-th child has Ty `fields[i]` |
| `Ty::Struct { layout: { kind: Struct { fields }, .. } }` | i-th child has Ty `fields[i].ty` |
| `Ty::Enum` `EnumShape::Single { variant: V, .. }` | i-th payload child has Ty `V.fields[i].ty` |
| `Ty::Enum` `EnumShape::Multiple { variants, .. }` | for variant_idx, i-th payload child has Ty `variants[variant_idx].fields[i].ty` |
| `Ty::Ref(_, pointee, _)` fat-pointer fallback | child0 = TyInterner lookup for `Ty::Ptr(pointee, Mutability::Not)`; child1 = TyInterner lookup for `Ty::Uint(UintKind::Usize)` -- both must already be interned by the bridge per the §9.7 fat-pointer well-formedness rule |

The renderer NEVER constructs non-interned TyIds; it looks up
existing TyIds in the closed `model.types` interner (the §10
closed-Ty-enum invariant). The fat-pointer fallback consumes the
bridge-side guarantee that the bridge interned both
`Ty::Ptr(pointee, Mutability::Not)` and `Ty::Uint(UintKind::Usize)`
when producing the `Aggregate([Reloc, Uint])` value per `spec.md`
§9.7's `Slice { data, meta }` dispatch.

### §10.1.3 `Aggregate(Vec<ConstId>)` -- table at pretty-printer.md:2960-2972

Per D-14-4.4 LOCKED. Children render via `render_const(c_i,
per_field_ty_i)` per the §10.1.8 resolution.

| Consumer Ty.kind() | Output |
|---|---|
| `Ty::Array(elem, N)` | `[<r(c_0, elem)>, ...]` (N children, bridge-validated) |
| `Ty::Tuple { fields: [T_0] }` | `(<r(c_0, T_0)>,)` (1-tuple trailing comma) |
| `Ty::Tuple { fields: [T_0, T_1, ...] }` (n >= 2) | `(<r(c_0, T_0)>, <r(c_1, T_1)>, ...)` |
| `Ty::Struct` empty fields | `<render_def_path(item)><render_generic_args(args, item)>` |
| `Ty::Struct` tuple-style | `<path><args>(<r(c_0, fields[0].ty)>, ...)` |
| `Ty::Struct` named-field | `<path><args> { fields[0].name: <r(c_0, fields[0].ty)>, ... }` |
| `Ty::Ref(_, Ty::Str, _)` + fat-pointer pre-conditions hold | `"<utf8-lossy decoded>"` |
| `Ty::Ref(_, Ty::Slice(<u8>), _)` + fat-pointer pre-conditions hold | `b"<escape-ascii decoded>"` |
| Other `Ty::Ref` fat-pointer (slice non-u8 / dyn / pre-conditions fail) | `(<r(c_0, *const inner)>, <r(c_1, usize)>) as <render_ty(consumer)>` (structural fallback per D-14-4.9) |
| `Ty::Enum / Ty::Union / scalars / FnDef / Closure / thin pointer / etc.` | Cannot occur -- panic |

#### Fat-pointer special-case pre-conditions (pretty-printer.md:2974-2994)

For both `&str` and `&[u8]` arms ALL must hold:

1. Aggregate has exactly 2 children.
2. `children[0]` resolves to `Const::Reloc(InstanceRef)`.
3. The InstanceRef resolves to a Static Instance with
   `model.item_synthetic_kinds[item] ==
   SyntheticItemKind::ConstMemoryStatic { .. }`.
4. The Static's `init` resolves to `Const::Bytes(vec)` of all
   `Byte::Value(b)` bytes (no `Uninit` / `Reloc`).
5. `children[1]` resolves to `Const::Uint(UintKind::Usize, len)`
   where `len == vec.len() as u128`.

For `&[u8]`: byte sequence renders as `b"<escape-ascii>"`. For
`&str`: byte sequence renders as `"<utf8-lossy decoded>"` (invalid
bytes become `U+FFFD` replacement character).

If any pre-condition fails, structural fallback per D-14-4.9 applies.

#### Tuple-vs-named-field dispatch (§3.4)

Synthetic `_<index>` field names signal tuple-style; non-synthetic
names signal named-field-style; empty fields signal unit-style. Same
rule as P1a-PP-04a's ZST arms.

### §10.1.4 `EnumValue { variant_idx, payload }` -- table at pretty-printer.md:3018-3029

Per D-14-4.5 LOCKED. Dispatches on consumer `Ty::Enum`'s `EnumShape`
to locate the named variant, then on the variant's `fields` to
determine the ctor form.

| Consumer Ty + variant shape | Output |
|---|---|
| `EnumShape::Single { variant: V, .. }` AND `V.fields.is_empty()` AND `payload.is_empty()` | `<enum_path>::<V.name>` |
| `EnumShape::Single { variant: V, .. }` AND `V.fields` tuple-synthetic | `<enum_path>::<V.name>(<r(p_0, V.fields[0].ty)>, ...)` |
| `EnumShape::Single { variant: V, .. }` AND `V.fields` named | `<enum_path>::<V.name> { V.fields[0].name: <r(p_0, V.fields[0].ty)>, ... }` |
| `EnumShape::Multiple { variants, .. }` AND `V = variants[variant_idx]` AND `V.fields.is_empty()` AND `payload.is_empty()` | `<enum_path>::<V.name>` (e.g., `Option::None`) |
| `EnumShape::Multiple` AND `V` tuple-synthetic | `<enum_path>::<V.name>(<r(p_0, V.fields[0].ty)>, ...)` (e.g., `Option::Some(42_i32)`) |
| `EnumShape::Multiple` AND `V` named | `<enum_path>::<V.name> { V.fields[0].name: <r(p_0, V.fields[0].ty)>, ... }` |
| `EnumShape::Empty` | Cannot occur -- panic |
| `variant_idx` out of bounds | Cannot occur (bridge-validated) -- panic |
| `payload.len() != V.fields.len()` | Cannot occur (bridge-validated) -- panic |
| Non-`Ty::Enum` consumer Ty | Cannot occur -- panic |

`enum_path = <render_def_path(item)><render_generic_args(args, item)>`.

Variant name lookup per §3.4: `EnumShape::Single.variant.name` or
`EnumShape::Multiple.variants[variant_idx].name`. Non-optional per
`spec.md` §11; missing names are bridge bugs.

### §10.1.5 `Bytes(Vec<Byte>)` -- table at pretty-printer.md:3051-3057

Per D-14-4.6 LOCKED. Dispatches with an all-uninit shortcut + a
byte-array case + a structural fallback.

| Bytes payload + consumer Ty | Output |
|---|---|
| All bytes are `Byte::Uninit` | `<uninit>` (rustc-mirrored shortcut; applies for any consumer Ty) |
| All-`Byte::Value(b)` AND `Ty::Array(<u8>, N)` AND `vec.len() == N` | `*b"<escape-ascii>"` (rustc-mirrored deref form) |
| Other consumer Tys with mixed bytes | `bytes![<byte_0>, <byte_1>, ..., <byte_{n-1}>]` (RegseqModel structural fallback) |
| `Ty::Ref(_, Ty::Str, _)` / `Ty::Ref(_, Ty::Slice(_), _)` | Cannot occur (bridge produces Aggregate per §9.7) -- panic |
| Scalar Ty (Bool / Int / Uint / Float / Char) | Cannot occur (bridge produces scalar variant) -- panic |

**All-uninit shortcut precedence**: evaluated FIRST, before the
byte-array case (`Const::Bytes([Uninit; N])` consumed at
`Ty::Array(<u8>, N)` renders as `<uninit>`, NOT
`*b"\xc0\xc0..."`).

**Structural fallback `bytes![...]` per-byte forms**:

- `Byte::Value(b)` -> `0xHH` (lowercase 2-digit hex).
- `Byte::Uninit` -> `<uninit>`.
- `Byte::Reloc(instance_ref)` -> `<reloc:
  <render_def_path(resolved_item)>>` with one token per maximal
  relocation run (per §9.7 well-formedness: relocation runs are
  pointer-shaped with `ptr_size` consecutive `Reloc(r)` bytes for
  one pointer).

Byte ordering renders in `Vec<Byte>` order (target-endian as the
bridge captured from `Allocation.bytes`). The renderer does NOT
re-order or re-interpret per consumer-Ty endianness.

### §10.1.6 `NullPtr(TyId)` -- table at pretty-printer.md:3104-3111

Per D-14-4.7 LOCKED. Renders in the rustc-mirrored typed-cast form
`0x0 as <render_ty(consumer_ty)>`, where `consumer_ty` is the
consumer-context pointer Ty (NOT the variant's `pointee_ty` payload
-- the payload is for canonical-equality only per `spec.md` §9.7).

| Consumer Ty.kind() | Output |
|---|---|
| `Ty::Ref(T, Provenance::SharedBorrow)` | `0x0 as &<render_ty(T)>` |
| `Ty::Ref(T, Provenance::UniqueBorrow)` | `0x0 as &mut <render_ty(T)>` |
| `Ty::Ptr(T, Mutability::Not)` | `0x0 as *const <render_ty(T)>` |
| `Ty::Ptr(T, Mutability::Mut)` | `0x0 as *mut <render_ty(T)>` |
| `Ty::FnPtr(sig_id)` | `0x0 as <render_ty(sig)>` (e.g., `0x0 as fn(u32) -> bool`) |
| Non-thin / non-pointer Ty | Cannot occur (fat pointers produce `Aggregate([NullPtr, NullPtr])` or similar; other consumer Tys are bridge bugs) -- panic |

The renderer delegates the cast-type form to
`render_ty(consumer_ty)` per stage-13 D-13-5.

### §10.1.7 `Reloc(InstanceRef)` -- table at pretty-printer.md:3143-3153

Per D-14-4.8 LOCKED. Dispatch sequence:

1. Resolve `instance_ref` to `InstanceId` (F4-2; bridge-validated).
2. Look up `instance = model.instances[InstanceId]`.
3. For Static instances, look up
   `model.item_synthetic_kinds[instance.item]` to distinguish
   source-level Statics from synth Items.
4. Dispatch on `(instance.kind, consumer Ty)`:

| Resolved instance.kind / synth | Consumer Ty | Output |
|---|---|---|
| `Function { .. }` | `Ty::FnDef(item, args)` matching `(instance.item, instance.generic_args)` | `<render_def_path(instance.item)><render_generic_args(instance.generic_args, instance.item)>` (no cast) |
| `Function { .. }` | `Ty::FnPtr(sig_id)` | `<path><args> as <render_ty(consumer)>` |
| `Static { .. }`, source-defined (`item_synthetic_kinds[item] = None`) | Any `Ty::Ref` or `Ty::Ptr` | `<render_def_path(instance.item)> as <render_ty(consumer)>` |
| `Static { .. }`, `VTableInstance { .. }` synth | Any pointer consumer Ty | `<render_def_path(instance.item)> as <render_ty(consumer)>` |
| `Static { .. }`, `ConstMemoryStatic { .. }` synth | `Ty::Ref(_, Ty::Array(<u8>, N), _)` AND b"..." pre-conditions hold | `b"<escape-ascii of init bytes>"` (rustc-mirrored) |
| `Static { .. }`, `ConstMemoryStatic { .. }` synth | Other thin-pointer Ty OR pre-conditions fail | `<render_def_path(instance.item)> as <render_ty(consumer)>` (structural fallback) |
| `Const / Struct / Enum / Union` | Any | Cannot occur -- panic |
| `Function { .. }` non-FnDef / non-FnPtr consumer | -- | Cannot occur -- panic |
| `Static { .. }` non-pointer consumer | -- | Cannot occur -- panic |

#### `b"..."` byte-string pre-conditions (pretty-printer.md:3155-3168)

ALL must hold:

1. Consumer Ty is `Ty::Ref(_, Ty::Array(u8_ty, N), _)` where `u8_ty`
   resolves to `Ty::Uint(UintKind::U8)`.
2. `instance_ref` resolves to a `Static { ty: byte_array_ty, init:
   Some(bytes_const_id) }`.
3. `model.item_synthetic_kinds[instance.item] ==
   SyntheticItemKind::ConstMemoryStatic { .. }`.
4. `model.consts[bytes_const_id] == Const::Bytes(vec)`.
5. Every byte is `Byte::Value(b)` (no Uninit / Reloc).
6. `vec.len() == N`.

If all hold, render as `b"<escape_ascii>"`. If any fails, structural
fallback (typed-cast) applies.

#### FnDef-vs-FnPtr dispatch

The bridge produces `Const::Reloc(fn_ref)` for both direct
function-value references (consumer Ty `Ty::FnDef`; no coercion) and
function-to-FnPtr coercion sites (consumer Ty `Ty::FnPtr`). The
renderer distinguishes by consumer Ty: bare path for FnDef,
typed-cast for FnPtr.

### Cross-references

- The §10.1.8 type-context-threading table is the canonical recursion
  rule; consumed by Aggregate, EnumValue, and the fat-pointer
  fallback paths.
- The §3.4 tuple-vs-named-field dispatch is shared with P1a-PP-04a's
  ZST arms and consumed identically here.

## Non-Goals

- No re-implementation of scalar / ZST arms (owned by P1a-PP-04a).
- No `render_ty`, `render_def_path`, or `render_generic_args`
  reimplementation (consumed via P1a-PP-02 / stage 12).
- No `render_instance` composition. Owned by P1a-PP-05a/b.
- No top-level `render(&self)` driver. Owned by P1a-PP-06.
- No bridge-side const synthesis (`ConstMemoryStatic` Item creation,
  Reloc target resolution, `Aggregate` / `Bytes` construction).
  Owned by axis 3 (P1a-BR-03a + P1a-BR-04).

## Test Plan

Test discipline names `unit-in-src` (per-arm dispatch logic;
fat-pointer pre-condition check; byte-rendering escape rules;
recursion termination through the §10.1.8 child Ty resolution) and
`integration-in-tests` (insta snapshots for §9.10 specimens 91-109
plus 96, 101, 102 fat-pointer specials).

### Unit tests (in `crates/regseq-model/src/pretty.rs`)

1. `Aggregate` over `Ty::Array` -- N-child bridge-validated count.
2. `Aggregate` over `Ty::Tuple` arity 1 (trailing comma) and arity
   2+.
3. `Aggregate` over `Ty::Struct` tuple-style and named-field with
   per-field Ty threading.
4. `Aggregate` `&str` special: all 5 pre-conditions hold ->
   `"<utf8-lossy>"`. Pre-condition fail -> structural fallback.
5. `Aggregate` `&[u8]` special: all 5 pre-conditions hold ->
   `b"<escape-ascii>"`. Pre-condition fail -> structural fallback.
6. `Aggregate` cannot-occur cases panic (Enum / Union / scalars /
   FnDef / etc.).
7. `EnumValue` `EnumShape::Single` and `EnumShape::Multiple` arms;
   unit / tuple / named variant ctor forms.
8. `EnumValue` `EnumShape::Empty` panics.
9. `EnumValue` payload-arity mismatch panics.
10. `Bytes` all-uninit shortcut precedence (evaluated before
    byte-array case).
11. `Bytes` byte-array `*b"..."` form when all-Value and consumer is
    `Ty::Array(<u8>, N)`.
12. `Bytes` structural fallback `bytes![0x00, <uninit>, 0xff,
    <reloc: ...>]` (Specimen 109).
13. `Bytes` cannot-occur cases (`Ty::Ref(_, Ty::Str)` / scalar Ty)
    panic.
14. `NullPtr` thin-pointer arms (Ref shared / unique, Ptr const /
    mut, FnPtr).
15. `NullPtr` non-thin / non-pointer consumer panics.
16. `Reloc` `Function -> Ty::FnDef` (no cast) vs `Function ->
    Ty::FnPtr` (typed cast).
17. `Reloc` source-defined Static vs `VTableInstance` synth (both
    typed-cast).
18. `Reloc` `ConstMemoryStatic` byte-string pre-conditions: all hold
    -> `b"..."`; any fail -> typed-cast fallback.
19. `Reloc` `Const / Struct / Enum / Union` instance.kind panics.

### Integration tests (`crates/regseq-model/tests/pretty.rs`)

Add insta snapshot tests for §9.10 specimens 91-109:

- Specimens 91, 92: NullPtr raw const pointer / shared reference.
- Specimens 93, 94: Reloc to FnDef (no cast) / FnDef -> FnPtr coercion
  (typed cast).
- Specimen 95: Reloc to source-defined Static (typed cast).
- Specimen 96: Reloc to ConstMemoryStatic with byte-string special
  case (`b"hello"`).
- Specimens 97 - 100: Aggregate Array, Aggregate 2-Tuple, Aggregate
  1-Tuple (trailing comma), Aggregate Struct (named-field).
- Specimens 101, 102: Aggregate `&str` and `&[u8]` literal special
  cases.
- Specimens 103 - 106: EnumValue `Option::None`, `Option::Some(42_u32)`,
  C-style, named-field variant.
- Specimens 107 - 109: Bytes all-uninit shortcut, byte-array
  `*b"hello"`, structural fallback `bytes![...]`.

### Test-discipline tags

- `unit-in-src` -- per-arm dispatch logic + fat-pointer pre-condition
  check + byte-rendering + Reloc dispatch tree.
- `integration-in-tests` -- insta snapshots for §9.10 specimens
  91 - 109.

## Review

After tests pass, request a Codex review using `codex_runner.py`:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-model-90 (P1a-PP-04b) against
its task spec at .harness/tasks/regseq-model/90-render-const-aggregate.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-model/src/pretty.rs
- crates/regseq-model/tests/pretty.rs
- (any new insta snapshot files under crates/regseq-model/tests/snapshots/)

Focus on:
- Spec deviations: §10.1.3 / §10.1.4 / §10.1.5 / §10.1.6 / §10.1.7
  per-arm tables; §10.1.8 per-sub-field Ty resolution table;
  fat-pointer pre-condition lists for `Aggregate` (&str / &[u8]) and
  `Reloc` (b"..." byte-string).
- Recursion shape: children call `render_const(c_i, per_field_ty_i)`
  with the correct child Ty per §10.1.8; no non-interned TyIds
  constructed at render time.
- Cannot-occur rows panic with informative messages, not silent
  fallback (per the brief's no-defensive-fallbacks rule).
- Correct rendering of `bytes![...]` per-byte forms, escape-ascii
  byte strings, UTF-8 lossy decode for &str.
- Test coverage vs spec Test Plan: every Specimen 91 - 109 has a
  matching insta snapshot.
Do NOT flag anything explicitly marked out of scope (scalar / ZST
arms which belong to P1a-PP-04a; later task IDs P1a-PP-05a/b through
P1a-PP-06).
</task>

<grounding_rules>
Cite file:line for every claim and every required fix. Do not
speculate about code you did not read. Mark hypotheses as hypotheses.
</grounding_rules>

<structured_output_contract>
First line: VERDICT: APPROVED | REQUIRES FIXES
If REQUIRES FIXES: numbered list, each entry exactly
  <file>:<line-range> -- <what is wrong> -- <what the spec requires>
If APPROVED: one paragraph (<=120 words) summarizing what was
verified and why any deliberate deviations are acceptable.
Total response <= 400 words.
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, form a verdict,
stop.
</default_follow_through_policy>
EOF
```

Iterate fix -> re-review until VERDICT: APPROVED.

## Quality Gate

```
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

## Acceptance Criteria

- `Aggregate / EnumValue / Bytes / NullPtr / Reloc` arms of
  `render_const(ConstId, TyId)` implemented per §10.1.3 - §10.1.7
  with the §10.1.8 per-sub-field Ty resolution and pair-form
  recursion.
- Fat-pointer pre-conditions enforced for both `Aggregate` (&str /
  &[u8] specials) and `Reloc` (`b"..."` byte-string special); failed
  pre-conditions fall through to structural fallback.
- Cannot-occur cases panic with informative assertion messages.
- All §9.10 specimens 91 - 109 have matching insta snapshot tests;
  outputs match verbatim.
- Unit tests cover per-arm dispatch shape, fat-pointer pre-condition
  evaluation, all-uninit shortcut precedence, and Reloc dispatch
  tree (Function vs Static vs synth).
- `cargo test -p regseq-model`, `cargo fmt --all -- --check`, and
  `cargo clippy -p regseq-model -- -D warnings` all pass.
- Codex Task Review verdict APPROVED.
