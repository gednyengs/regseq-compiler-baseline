# Rustc prior art -- const rendering

Evidence collected for **D-14-4** (`render_const(ConstId) -> String`
composer renderer; per-arm specs across the closed 11-variant `Const`
enum).

Toolchain: `nightly-2026-02-01-aarch64-apple-darwin`. Paths are
relative to:
`~/.rustup/toolchains/nightly-2026-02-01-aarch64-apple-darwin/lib/rustlib/rustc-src/rust/compiler/`

The 11 variants under design (per `spec-snapshot.md` §9.7):
`Bool(bool)` / `Char(u32)` / `Int(IntKind, u128)` /
`Uint(UintKind, u128)` / `Float(FloatKind, u128)` / `ZST` /
`Bytes(Vec<Byte>)` / `Aggregate(Vec<ConstId>)` /
`EnumValue { variant_idx, payload }` / `NullPtr(TyId)` /
`Reloc(InstanceRef)`.

## Q1 -- API signature: is `Ty` required alongside `Const`?

**Finding.** Rustc's MIR const-rendering primary entry takes both:

```text
fn pretty_print_const_value_tcx<'tcx>(
    tcx: TyCtxt<'tcx>,
    ct: ConstValue,
    ty: Ty<'tcx>,
    fmt: &mut Formatter<'_>,
) -> fmt::Result
```

at `rustc_middle/src/mir/pretty.rs:1851`. The dispatch is a tuple
match on `(ct, ty.kind())` at line 1872, with several arms keyed
*solely* on `ty.kind()` (the `&str` / `&[u8]` / `[u8; N]` special
cases at 1874-1898 use the type to decide rendering shape, not the
const).

**The MIR pretty-printer's overall `Const` `Display` impl** (at
`pretty.rs:1269`-`1277`) wraps the value with `"const "` prefix
unless `ty.kind() == FnDef(..)`; the value rendering still threads
the `Ty` through.

**Stable-IR analog** (`rustc_public/src/mir/pretty.rs:339-345`):

```rust
fn pretty_mir_const(literal: &MirConst) -> String {
    with(|cx| cx.mir_const_pretty(literal))
}
fn pretty_ty_const(ct: &TyConst) -> String {
    with(|cx| cx.ty_const_pretty(ct.id))
}
```

The stable IR delegates back through the compiler interface to the
rustc-internal printer rather than reimplementing the dispatch
itself. That escape hatch is unavailable to RegseqModel (no `tcx`
at render time), so RegseqModel must implement the dispatch
self-contained.

**Implication for D-14-4.** The renderer needs the consumer's `Ty`
for at least these arms:

- `ZST`: type comes from consumer context (`spec.md` §9.7 line 2861
  is explicit on this).
- `Bytes(_)`: the same byte payload renders as `"hello"` / `b"hello"`
  / `*b"hello"` depending on whether the type is `&str` / `&[u8]` /
  `[u8; N]`; without the type, only a raw byte dump is possible.
- `Aggregate(_)`: child ordering is structural (vec-positional), but
  the *delimiters* differ (array `[...]` vs tuple `(...)` vs struct
  ctor `Path { f1: a, f2: b }`); the field-name rendering for
  struct/enum variants needs the layout from the consumer's Ty.
- `EnumValue { variant_idx, payload }`: the variant *name* (and
  field names if named-field) come from the consumer's
  `Ty::Enum.layout.kind`.
- `NullPtr(TyId)`: variant carries the pointee Ty, no additional
  context needed.
- `Reloc(InstanceRef)`: the InstanceRef resolves to a path
  (`render_def_path` via the resolved Instance); type provides the
  pointer-form decoration (`as *const T` / `as fn(...) -> T`).

The variants that carry their own type-determining tag
(`Bool` / `Char(u32)` / `Int(IntKind, _)` / `Uint(UintKind, _)` /
`Float(FloatKind, _)`) can render fully without external context.

**Recommended signature.** `render_const(ConstId, TyId) -> String`,
mirroring rustc's `(ct, ty)` pair. The TyId is the consumer-context
type at the use site (the `Ty` of the position holding the
`ConstId`: `InstanceKind::Static.init` Ty, `InstanceKind::Const.value`
Ty, an aggregate field's Ty for a recursive child, etc.).

Alternative signatures considered:

- `render_const(ConstId) -> String` (no Ty). Forces a type-erased
  rendering: `Bytes` always as raw bytes; `Aggregate` always as
  `[child, child, ...]` regardless of struct/tuple/array shape.
  Loses fidelity vs the existing specimen contract at
  `pretty-printer-snapshot.md` lines 1140-1287 (which already
  expects `42_i32`, `true`, `0xFFu8`, etc.). Rejected.
- `render_const(ConstId, &Ty)` (borrowed `Ty`). Borrow-friendly but
  requires the caller to have already resolved `TyId -> &Ty`. Stage
  13's `render_ty(TyId)` and `render_def_path(ItemId)` take ID
  forms, so consistency points to `TyId`. The `PrettyPrinter` has
  `&model` and can resolve internally.

## Q2 -- Per-variant rendering forms (from rustc)

Source: `rustc_middle/src/ty/print/pretty.rs:1706-1849` for scalar
forms; `rustc_middle/src/mir/pretty.rs:1851-1989` for the top-level
dispatch + aggregate forms; `rustc_middle/src/ty/consts/int.rs:51-138`
for `ConstInt` Debug (the integer rendering primitive).

### Bool

`rustc_middle/src/ty/print/pretty.rs:1784-1785`:

```rust
ty::Bool if int == ScalarInt::FALSE => write!(self, "false")?,
ty::Bool if int == ScalarInt::TRUE => write!(self, "true")?,
```

No type suffix.

### Int (signed) / Uint (unsigned)

`rustc_middle/src/ty/print/pretty.rs:1806-1810`:

```rust
ty::Uint(_) | ty::Int(_) => {
    let int = ConstInt::new(
        int,
        matches!(ty.kind(), ty::Int(_)),
        ty.is_ptr_sized_integral(),
    );
    if print_ty { write!(self, "{int:#?}")? } else { write!(self, "{int:?}")? }
}
```

`print_ty == true` in MIR pretty-printing, so the alternate form
applies. The alternate-form `ConstInt` Debug at
`rustc_middle/src/ty/consts/int.rs:89-99` / `123-133` produces:

- Underscore-prefixed-width suffix: `42_i32`, `42_u32`, `0_u8`,
  `42_isize`, `42_usize`, etc.
- Sentinel collapsing at the bounds: `i32::MAX`, `i32::MIN`,
  `u32::MAX`, `usize::MIN`, etc. No suffix on sentinels.

The bound-sentinel check is exact-equality on raw bits
(`raw == min` / `raw == max`).

### Float

`rustc_middle/src/ty/print/pretty.rs:1787-1804`:

```rust
ty::Float(fty) => match fty {
    ty::FloatTy::F16 => {
        let val = Half::try_from(int).unwrap();
        write!(self, "{}{}f16", val, if val.is_finite() { "" } else { "_" })?;
    }
    ty::FloatTy::F32 => { ... "{}{}f32" ... }
    ty::FloatTy::F64 => { ... "{}{}f64" ... }
    ty::FloatTy::F128 => { ... "{}{}f128" ... }
}
```

The forms:

- Finite: `1.5f32`, `1f32`, `0f64`, `-0f64`, `inff32` (Display impl
  of `Single` produces `inf` / `-inf` / `NaN` for non-finites).
- Non-finite: underscore between value and suffix: `inf_f32`,
  `-inf_f32`, `NaN_f32`. Reason: `1.5f32` parses as a Rust literal;
  `inff32` would too if not for the trailing `f32` colliding with
  the `inf` token, so the underscore forces a token break.

`rustc_apfloat` provides the `Display` impl that does the IEEE-754
decoding (the `Half` / `Single` / `Double` / `Quad` types).

### Char

`rustc_middle/src/ty/print/pretty.rs:1812-1814`:

```rust
ty::Char if char::try_from(int).is_ok() => {
    write!(self, "{:?}", char::try_from(int).unwrap())?;
}
```

`char` Debug produces a quoted char literal with Rust's standard
escape rules: `'a'`, `'\n'`, `'\\''`, `'\u{1f600}'`, etc.

If `char::try_from(int)` fails (invalid Unicode scalar), this arm
doesn't match and rendering falls through to the
"Nontrivial scalar" fallback (`transmute(0x...)` form). RegseqModel
won't hit this: the bridge validates `Char(u32)` payloads at
construction (per `spec-snapshot.md` §9.7 line 2836).

### ZST

`rustc_middle/src/mir/pretty.rs:1976-1982`:

```rust
(ConstValue::ZeroSized, ty::FnDef(d, s)) => {
    let mut p = FmtPrinter::new(tcx, Namespace::ValueNS);
    p.print_alloc_ids = true;
    p.pretty_print_value_path(*d, s)?;
    fmt.write_str(&p.into_buffer())?;
    return Ok(());
}
```

`ZST` over `FnDef` renders as just the function path (e.g.,
`std::option::Option::<T>::map`). No `const` prefix at the operand
layer either (line 1271-1273: `ty::FnDef(..) => {}` skips the
prefix).

`ZST` over other types falls through to the debug fallback at line
1988: `write!(fmt, "{ct:?}: {ty}")`. RegseqModel's `ZST` covers a
wider range (unit types, fieldless structs, `PhantomData<T>`,
etc.) and would render those as the value form expected at the
consumer site (e.g., `()` for unit, `Foo` for fieldless struct,
`PhantomData` for PhantomData -- TBD per D-14-4 sub-decision).

### Aggregate (Array / Tuple / Struct ctor / Enum variant)

`rustc_middle/src/mir/pretty.rs:1907-1966`. The dispatch:

```rust
(_, ty::Array(..) | ty::Tuple(..) | ty::Adt(..))
    if !ty.has_non_region_param() => {
    let contents = tcx.try_destructure_mir_constant_for_user_output(ct, ty);
    match *ty.kind() {
        ty::Array(..) => "[ comma_sep ]",
        ty::Tuple(..) => "( comma_sep )" + trailing-comma if len==1,
        ty::Adt(def, _) if def.variants().is_empty() => "{unreachable(): TY}",
        ty::Adt(def, args) => {
            // variant_def = def.variant(contents.variant.unwrap())
            // p.pretty_print_value_path(variant_def.def_id, args)
            match variant_def.ctor_kind() {
                Some(CtorKind::Const) => {}                                  // unit variant
                Some(CtorKind::Fn)    => "(" + comma_sep + ")",              // tuple variant
                None                  => " { f1: v1, f2: v2 }",              // named-field variant
            }
        }
    }
}
```

Forms:

- Array of len N: `[a, b, c, d]`.
- Tuple of len > 1: `(a, b, c)`.
- Tuple of len 1: `(a,)` (trailing comma -- Rust syntax for
  1-tuples).
- Tuple of len 0: `()`. (Rust unit; rustc renders via the same
  `(comma_sep)` path with empty fields.)
- Struct (unit variant, ctor=Const): just `Path` (the value path).
- Struct (tuple variant, ctor=Fn): `Path(a, b, c)`.
- Struct (named-field variant, ctor=None): `Path { f1: a, f2: b }`
  (with `{{` and `}}` escapes around braces in the rustc source,
  which produce literal braces).
- Enum (multi-variant): same per-variant shapes as struct, but the
  path includes the enum name and variant name (e.g.,
  `Option::Some(42_i32)`, `Result::Err { reason: "oops" }`).
- Enum (zero-variant `!`-like): `{unreachable(): TY}` (theoretical
  -- such a const value cannot exist).

Recursion via `comma_sep`
(`rustc_middle/src/mir/pretty.rs:1835-1849`):

```rust
fn comma_sep(tcx, fmt, elems: Vec<(ConstValue, Ty<'tcx>)>) {
    for (ct, ty) in elems {
        if !first { ", " }
        pretty_print_const_value_tcx(tcx, ct, ty, fmt)?;
    }
}
```

Each child is rendered with its own per-field type (resolved via
`try_destructure_mir_constant_for_user_output` which returns
`(child ConstValue, child Ty)` pairs).

### Bytes (byte string / slice / array specializations)

`rustc_middle/src/mir/pretty.rs:1874-1898` plus
`pretty.rs:1727-1754`:

- `&str` literal: `rustc_middle/src/mir/pretty.rs:1874-1879`:
  if type is `&_ str` and bytes recoverable:
  ```rust
  fmt.write_str(&format!("{:?}", String::from_utf8_lossy(data)))?;
  ```
  produces a Rust-debug-quoted string: `"hello"`, `"line1\nline2"`,
  `"with \"quotes\""`. Lossy UTF-8 substitutes the replacement
  character for ill-formed bytes.
- `&[u8]` literal: `rustc_middle/src/mir/pretty.rs:1880-1888`:
  if type is `&_ [u8]` and bytes recoverable:
  ```rust
  pretty_print_byte_str(fmt, data)?;
  ```
  where `pretty_print_byte_str` at line 1831 is:
  ```rust
  write!(fmt, "b\"{}\"", byte_str.escape_ascii())
  ```
  produces a Rust byte-literal: `b"hello"`, `b"\x00\x01\x02"`.
- `[u8; N]` literal (array, not reference):
  `rustc_middle/src/mir/pretty.rs:1889-1898`:
  ```rust
  fmt.write_str("*")?;
  pretty_print_byte_str(fmt, byte_str)?;
  ```
  produces `*b"hello"` -- the leading `*` is a Rust deref operator
  indicating the bytes are loaded from a pointer, used to
  disambiguate from the `&[u8]` form.

The "scalar pointer to byte string" path
(`pretty_print_const_scalar_ptr`, `pretty.rs:1719-1773`) covers
the `&[u8; N]` case (a thin pointer to a byte-array allocation):

- If `ty == Ref(_, Array(u8, N))` and provenance resolves to
  `GlobalAlloc::Memory`: `pretty_print_byte_str(byte_str)` ->
  `b"hello"`.
- If `ty == Ref(_, Array(u8, N))` and provenance resolves to other
  allocations: `<static({def_id:?})>`, `<function>`, `<vtable>`,
  `<typeid>`, `<dangling pointer>`.

All-uninit shortcut at `rustc_middle/src/mir/pretty.rs:1864-1869`:

```rust
if ct.all_bytes_uninit(tcx) {
    fmt.write_str("<uninit>")?;
    return Ok(());
}
```

Renders `<uninit>` for any const where every byte is uninit (e.g.,
`MaybeUninit<u8>::uninit()` or an aggregate where every field is
uninit). Saves dump verbosity.

### Fallback for unhandled Bytes shapes

Per the FIXME at `rustc_middle/src/mir/pretty.rs:1983-1985`:

> FIXME(oli-obk): also pretty print arrays and other aggregate
> constants by reading their fields instead of just dumping the
> memory.

The fallback at line 1988 is `write!(fmt, "{ct:?}: {ty}")` -- the
raw Debug shape of `ConstValue` (e.g.,
`Indirect { alloc_id: alloc7, offset: Size { raw: 0 } }: [i32; 4]`)
appended with `:` and the type. Not user-pretty.

RegseqModel can do better: by the time `Bytes(Vec<Byte>)` reaches
`render_const`, the bridge has already evaluated the allocation
into the three-state `Byte` form (Uninit / Value(u8) / Reloc), so
the renderer has full structural access. A skeletal byte-list form
(`bytes![0x00, 0x01, <uninit>, <reloc:path>]` or similar) is
possible as a fallback for `Bytes` over types that aren't
`&str`/`&[u8]`/`[u8; N]`.

### NullPtr / Reloc -- pointer forms

`rustc_middle/src/ty/print/pretty.rs:1816-1826` (scalar int over
pointer type):

```rust
ty::Ref(..) | ty::RawPtr(_, _) | ty::FnPtr(..) => {
    let data = int.to_bits(self.tcx().data_layout.pointer_size());
    self.typed_value(
        |this| write!(this, "0x{data:x}"),
        |this| this.print_type(ty),
        " as ",
    )?;
}
```

The `typed_value` helper produces forms like `0x0 as *const u32`.
For RegseqModel's `NullPtr(TyId)`, this corresponds to:
`0x0 as <render_ty(TyId)>` (a null-cast form).

`rustc_middle/src/ty/print/pretty.rs:1755-1768` (scalar ptr to fn):

```rust
ty::FnPtr(..) => {
    if let Some(GlobalAlloc::Function { instance, .. }) =
        self.tcx().try_get_global_alloc(prov.alloc_id()) {
        self.typed_value(
            |this| this.pretty_print_value_path(instance.def_id(), instance.args),
            |this| this.print_type(ty),
            " as ",
        )?;
    }
}
```

For RegseqModel's `Reloc(InstanceRef)` over fn-ptr type:
`<resolved path> as <render_ty(TyId)>` (the path + cast form).

Reloc to static, vtable, etc. uses `pretty_print_const_pointer` at
line 1772 (overridden in MIR mode at line 1851-1860 to print alloc
ids: `alloc7+0x10`). RegseqModel's `Reloc(InstanceRef)` over a
non-fn-ptr type would render as the resolved path + cast (the
InstanceRef points to a synthesised Static for `&str` / `&[T]` per
`spec-snapshot.md` §9.7 "Reference-typed const handling"; for a
direct fn-Instance the path is the function definition).

### Verbose-internals mode

`rustc_middle/src/mir/pretty.rs:1859-1862`:

```rust
if tcx.sess.verbose_internals() {
    fmt.write_str(&format!("ConstValue({ct:?}: {ty})"))?;
    return Ok(());
}
```

Toggle for raw-debug dumps. RegseqModel's `PrettyPrintOptions`
(stage-13 D-13-3) currently has `include_spans` + `include_debug_names`;
adding a `verbose_const_internals: bool` for the equivalent toggle
is **out of v1 scope** (per `spec-snapshot.md` §3.1's no-defensive
posture + this is purely diagnostic, not a load-bearing feature).

## Q3 -- Recursion / acyclicity

`comma_sep` (`rustc_middle/src/mir/pretty.rs:1835-1849`) recurses
into each child via `pretty_print_const_value_tcx`. No explicit
cycle-detection -- rustc relies on `ConstValue` being a tree
(allocations can't recursively reference themselves through const
evaluation).

RegseqModel mirror: `Aggregate(Vec<ConstId>)` /
`EnumValue { payload: Vec<ConstId> }` recurse through `ConstId`
edges. **`spec-snapshot.md` §9.7 line 830-836 already pins
const-graph acyclicity** as a validation invariant ("No `ConstId`
appears in its own transitive `Aggregate` / `EnumValue` closure").
Termination is guaranteed.

## Q4 -- Operand-level wrapper

`Display for ConstOperand` at `rustc_middle/src/mir/pretty.rs:1269-1277`:

```rust
match self.ty().kind() {
    ty::FnDef(..) => {}
    _ => write!(fmt, "const ")?,
}
Display::fmt(&self.const_, fmt)
```

Operand-level rendering adds `const ` prefix unless the type is
`FnDef`. For RegseqModel, this is **stage-13 D-13-6's territory**
(per the per-arm `render_operand` spec at
`pretty-printer-snapshot.md` §7.2). Stage-13's spec for
`Operand::Constant(id)` renders as bare `<render_const(id)>` with no
`const ` prefix (per the specimen at line 1140-1141:
`Operand::Constant(<ConstId for 42_i32>)` -> `42_i32`). This is a
deliberate stylistic divergence from rustc -- stage-14's
`render_const` produces the bare value form; the operand wrapper
does not add `const ` decoration.

## Q5 -- Path rendering inside Const arms

The Aggregate-over-Adt arms call `p.pretty_print_value_path(def_id, args)`
which produces the value-namespace path with generic args (e.g.,
`Option::<i32>::Some`). For RegseqModel, the equivalent is
`render_def_path(ItemId)` (stage-12 D-12-6) for the Item-level
path; if the Item is a variant constructor, the rendering needs to
combine the enum's path with the variant's name. The exact form is
a sub-decision for D-14-4 (Aggregate-EnumValue arm).

## Q6 -- Specimen contract already pinned

The existing `pretty-printer-snapshot.md` specimens already
constrain `render_const` outputs:

- `Operand::Constant(<ConstId for 42_i32>)` -> `42_i32`
  (`pretty-printer-snapshot.md:1140-1141`)
- `Operand::Constant(<ConstId for 0_u8>)` (within Repeat) ->
  `0_u8` (line 1152)
- `Operand::Constant(<ConstId for 1u8>)` -> `1u8` (line 1204; note:
  no underscore here -- inconsistency with line 1140's `42_i32`;
  flagged for D-14-4 to resolve)
- `Operand::Constant(<ConstId for 42i32>)` -> `42i32` (line 1212;
  same inconsistency)
- `Operand::Constant(<ConstId for true>)` -> `true` (line 1212)
- `Operand::Constant(<ConstId for 42u32>)` -> `42u32` (line 1226;
  same inconsistency)
- `Operand::Constant(<ConstId for 0xFFu8>)` -> `0xFFu8` (line 1241;
  note: hex literal -- but rustc's `ConstInt` Debug is always
  decimal, so this form would be a RegseqModel-specific
  divergence; flagged)
- `Operand::Constant(<ConstId for unit ()>)` -> `()` (line 1255)
- `Operand::Constant(<ConstId for 16usize>)` -> `16usize` (line
  1287)

**Inconsistency to resolve in D-14-4.** Some specimens use the
underscore separator (`42_i32`); others omit it (`1u8`, `42i32`,
`42u32`, `16usize`). Rustc consistently uses the underscore form
(`{ConstInt:#?}` always produces `_<width>`). **Recommend
standardising on the rustc form: `42_i32`, `1_u8`, `42_u32`,
`16_usize`.** The `0xFFu8` hex form is a more substantive
question -- rustc never produces hex; RegseqModel could either
match rustc (always decimal) or admit a hex form for byte-style
values (likely produced via a bridge hint or the consumer's source
style). **Recommend decimal-only for consistency**; the
`0xFFu8` specimen would be revised to `255_u8`.

The `unit ()` specimen (`ZST` over `()`) at line 1255 shows the
expected form for a ZST over unit type. Need to extend the spec
to cover ZST over fieldless struct / PhantomData / fn / etc.

## Q7 -- Reusable building blocks

RegseqModel components that `render_const` will compose:

- `render_ty(TyId)` (stage 13, D-13-5): per-arm Ty rendering for
  the type-suffix and type-cast forms (`as *const T`, `as fn(...)`).
- `render_def_path(ItemId)` (stage 12, D-12-6): path string for
  the `Reloc(InstanceRef)` arm (after `InstanceRef -> ItemId`
  resolution).
- `render_def_path_value(&DefPath)` (stage 1): value-level path
  helper (used internally for synthesized paths if needed).
- `render_generic_args(&GenericArgs)` (stage 13, D-13-5): generic
  args rendering for `Path::<T1, T2>` value-namespace forms.
- Field / variant name resolution via layout (per
  `pretty-printer-snapshot.md` §3.4) for the Aggregate-Struct /
  EnumValue arms.

`render_const` does NOT need to call back into rustc -- the
RegseqModel IR carries everything required (post-bridge).

## Summary -- D-14-4 candidate sub-decisions

Each is a candidate D-tag, to be allocated and locked at Phase A.
These are the sub-decisions surfaced by the rustc evidence pass;
prior-art research on Charon / mrustc / rustc_public will refine
where load-bearing.

1. **D-14-4.1 -- API signature.** `render_const(ConstId, TyId) -> String`
   (vs Ty-less form). Recommend: pair form to match rustc.
2. **D-14-4.2 -- Bool/Char/Int/Uint/Float per-arm.** Decimal value
   + Rustc-style underscore-prefixed suffix
   (`42_i32`, `42_u32`, `1.5f32`, `'a'`, `true`).
   Sentinel collapsing
   (`i32::MAX`, `i32::MIN`, etc.) yes-or-no -- recommend YES
   (matches rustc; reduces noise for max-value patterns common in
   driver register code). Float NaN/inf rendering via underscore
   break (`inf_f32`, `NaN_f32`).
3. **D-14-4.3 -- ZST per-arm.** Rendering rules by consumer Ty:
   - `()` -> `()`
   - `Ty::Struct` fieldless / `PhantomData` / `Ty::Closure`
     non-capturing -> `<render_def_path(ItemId)>` (just the path)
   - `Ty::FnDef(InstanceId, _)` -> `<render_def_path(item)>`
   - else: open question (debug-fallback or rejection?)
4. **D-14-4.4 -- Aggregate per-arm dispatch.** Dispatch on
   consumer `Ty.kind()`:
   - `Ty::Array(_, _)` -> `[c1, c2, ...]`
   - `Ty::Tuple(_)` len > 1 -> `(c1, c2, ...)`
   - `Ty::Tuple(_)` len == 1 -> `(c1,)`
   - `Ty::Tuple(_)` len == 0 -> impossible (ZST handles unit;
     reject if hit)
   - `Ty::Struct { layout, .. }`:
     - unit struct (no fields) -> `<path>`
     - tuple struct (fields with synthetic `_0` / `_1` / ... names
       per `pretty-printer-snapshot.md` §3.4) ->
       `<path>(c1, c2, ...)`
     - named-field struct -> `<path> { f1: c1, f2: c2 }`
5. **D-14-4.5 -- EnumValue per-arm.** Dispatch on the variant's
   field shape via `Ty::Enum.layout.kind`:
   - unit variant -> `<enum_path>::<variant_name>`
   - tuple variant -> `<enum_path>::<variant_name>(c1, c2)`
   - named-field variant ->
     `<enum_path>::<variant_name> { f1: c1, f2: c2 }`
6. **D-14-4.6 -- Bytes per-arm dispatch.** Dispatch on consumer
   `Ty.kind()`:
   - `Ty::Ref(_, inner, _)` where `inner == Ty::Str` -> `"hello"`
     (UTF-8 decode of payload Value bytes; uninit-or-reloc bytes
     fall through to fallback)
   - `Ty::Ref(_, inner, _)` where `inner == Ty::Slice(u8)` ->
     `b"hello"`
   - `Ty::Array(u8, N)` -> `*b"hello"` (rustc form)
   - else: structural byte-list form (e.g.,
     `bytes![0x00, 0x01, <uninit>, <reloc:path>]` -- to be pinned)
   - All-uninit shortcut: `<uninit>` (mirrors rustc)
7. **D-14-4.7 -- NullPtr per-arm.**
   `0x0 as <render_ty(TyId)>` form (rustc-mirrored).
8. **D-14-4.8 -- Reloc per-arm.** Dispatch on the InstanceRef's
   resolved Item kind:
   - Function -> `<render_def_path>` (no `as` cast since the path
     names a function value)
   - Static / vtable / synthesised Static (`ConstMemoryStatic`) ->
     `<render_def_path> as <render_ty(TyId)>` (path + cast to
     pointer type)
9. **D-14-4.9 -- Type-context threading.** How the caller threads
   the consumer Ty for recursive children. Two patterns:
   - Caller resolves per-field types from the parent Ty's layout
     and passes the per-child Ty in (matches rustc's
     `try_destructure_mir_constant_for_user_output` returning
     `(child ConstValue, child Ty)` pairs).
   - Renderer takes a `parent_ty` and walks the parent layout
     itself.
   Recommend: caller resolves and passes (cleaner contract; the
   PrettyPrinter has `&model` and can walk layouts).
10. **D-14-4.10 -- Sentinel collapsing scope.** Which ints/uints
    get `T::MAX` / `T::MIN` rendering? Rustc does ALL primitive
    int widths + isize/usize. Recommend: match rustc; emit MAX/MIN
    sentinels for all `IntKind` / `UintKind` variants. **Worth a
    look at driver code to confirm this is desirable** -- driver
    code often uses raw decimal values for register-bit patterns,
    and `u32::MAX` is rare except in mask constants. **Leave open
    for evidence-pass continuation.**
11. **D-14-4.11 -- Integer hex form admission.** The existing
    specimen `0xFFu8` (line 1241) admits hex; rustc never does.
    Recommend: standardise on decimal-only (revise specimen to
    `255_u8`); admit a future `IntegerRenderStyle::Hex` option if
    driver code needs it (out of v1 scope).
12. **D-14-4.12 -- Float infinity / NaN representation.** Rustc
    uses underscore-broken form (`inf_f32`, `NaN_f32`, `-inf_f32`).
    Recommend: match rustc (the form parses unambiguously and is
    diagnostically useful).

## What rustc evidence does NOT settle

These need prior-art from other projects (Charon, mrustc,
rustc_public) or design judgment during D-14-4 work:

- **`ZST` over non-`FnDef` / non-fieldless-struct types.** Rustc
  has a debug fallback; RegseqModel's stronger validation surface
  permits a richer rendering. Consider: do we admit `ZST` over
  arbitrary `Ty::Tuple(0)` -> `()`?
- **`Bytes` over non-special-cased types.** Rustc admits a debug
  fallback; RegseqModel should pin a structural byte-list form
  (the `Byte` enum's three-state shape gives full information).
- **Operand-level `const ` prefix.** Stage-13 D-13-6 specimens
  don't use it; rustc does. Already pinned -- no action needed.
- **Hex vs decimal integer rendering.** Driver code preference
  may differ from rustc convention. The current specimen
  inconsistency forces a choice.
- **Path-vs-cast for `Reloc` to vtable / synthesised static.**
  Need a specimen-level decision: do we render the synth-static
  for `&str` as `<mem_<hash>> as *const u8` or just the bytes
  inline? Spec §9.7 says "C codegen casts at use site" -- the
  rendering should probably preserve this layering.

These are the load-bearing sub-questions for D-14-4 Phase A
discussions, not blockers for the rustc evidence pass.
