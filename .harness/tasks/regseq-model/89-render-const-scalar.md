# regseq-model-89: `render_const` for scalar / simple variants (P1a-PP-04a)

## Description

Implement the stage-14 composer `render_const(ConstId, TyId) ->
String` on `PrettyPrinter<'_>`, covering the **scalar / simple**
variants of the 11-variant closed `Const` enum: `Bool`, `Char`,
`Int`, `Uint`, `Float`, and `ZST`. The pair-form takes the consumer
context `TyId` (the type of the position holding the `ConstId`) per
stage-14 D-14-4.1 LOCKED.

Per the §8 ownership-table row: this is one of the three stage-14
composer renderers. The dispatch path implemented here is the
"key-on-variant-tag, ignore consumer Ty" branch for `Bool / Char /
Int / Uint / Float`, and the "key-on-consumer-Ty.kind()" recursive
branch for `ZST`. The aggregate / enum / bytes / reloc / nullptr
arms are owned by P1a-PP-04b (regseq-model-90); both tasks share the
same single `render_const` method, with this task laying the per-arm
dispatch frame and the simple arms.

## Input Context

- `.harness/SESSION.md`
- `.harness/spec-batches/regseqmodel-phase-1a.md` (per-task scope row
  P1a-PP-04a)
- `docs/discussions/regseq-model/pretty-printer.md` §1 (Mission), §2
  (Master rendering style), §3.3 (sigil convention for `ConstId`),
  §6 (`render_ty` -- consumed by recursion), §8 (Per-method ownership
  table at `pretty-printer.md:1148-1170` -- canonical
  `render_const(ConstId, TyId)` signature), §10.1 dispatch overview
  (pretty-printer.md:2610-2649), §10.1.1 scalar variants
  (pretty-printer.md:2651-2879), §10.1.2 ZST
  (pretty-printer.md:2881-2948), §10.1.8 type-context threading
  (pretty-printer.md:3187-3282 -- recursion + operand-wrapper
  contract), §9.10 specimens (the scalar / ZST subset: Specimens
  76 - 90 plus 91 - 92 NullPtr which are P1a-PP-04b)
- `docs/discussions/regseq-model/spec.md` §9.7 (`Const` 11-variant
  enum + per-variant byte-encoding tags + canonical-form rules), §10
  (`Ty` enum -- consumer `Ty.kind()` is the dispatch driver for ZST),
  §11 (`ScalarLayouts` for `Isize` / `Usize` width column when
  decoding sentinel constants per D-14-4.10)
- `crates/regseq-model/src/pretty.rs` (the existing `PrettyPrinter`
  shell)
- The `Const` 11-variant enum, `IntKind`, `UintKind`, `FloatKind`,
  `Byte` 3-state enum, `InstanceRef`, and `ConstInterner` from
  regseq-model-DS-05 (P1a-DS-05)
- The `Ty` enum + `Layout` machinery from regseq-model-DS-02 /
  P1a-DS-03
- `render_ty` (P1a-PP-02; regseq-model-87) -- consumed by the ZST
  composite recursion arms
- `render_def_path` (already shipped at stage 12) -- consumed by ZST
  arms over `Ty::Struct / Enum / Union / FnDef / Closure`
- `render_generic_args` (P1a-PP-02; regseq-model-87) -- consumed by
  ZST arms over generic-bearing ADTs / FnDef / Closure
- `.harness/tasks/regseq-model/24-place-and-projection.md` (size /
  shape reference)

## Implementation Notes

Edit `crates/regseq-model/src/pretty.rs`. The §8 ownership table at
`pretty-printer.md:1148-1170` is the canonical source for the
`render_const` signature; the per-arm dispatch is documented
exhaustively across §10.1.1 - §10.1.7. This task owns §10.1.1 and
§10.1.2; P1a-PP-04b owns §10.1.3 - §10.1.7.

### Public API (per §8 ownership table)

```rust
impl PrettyPrinter<'_> {
    /// Per-variant Const value (pair-form mirrors rustc per-variant
    /// dispatch; composes `render_ty` for type-context arms).
    /// Per pretty-printer.md §10.1; D-14-4.1 LOCKED pair-form takes
    /// `(ConstId, TyId)` where `TyId` is the consumer-context type
    /// at the use site.
    pub fn render_const(&self, id: ConstId, ty: TyId) -> String;
}
```

The single `render_const` method dispatches on `(model.consts[id], ty.kind())`:

- `Bool / Char / Int / Uint / Float` -- key on variant tag; ignore
  consumer Ty (this task).
- `ZST` -- key on consumer `Ty.kind()` and recurse for composite
  cases (this task).
- `Aggregate / EnumValue / Bytes / NullPtr / Reloc` -- key on
  consumer Ty for structural arms (P1a-PP-04b).

This task lays the per-variant dispatch frame; the variants P1a-PP-04b
owns may be implemented as `todo!()` placeholders if P1a-PP-04b lands
later, but the match shape covers all 11 variants from this task.

### §10.1.1 Scalar variants -- Bool / Char / Int / Uint / Float

Per D-14-4.2 LOCKED. Width self-determined by the variant payload tag
(`IntKind` / `UintKind` / `FloatKind`); consumer `TyId` ignored.

#### Bool / Char (table rows at pretty-printer.md:2663-2665)

- `Bool(false)` -> `false` (no suffix).
- `Bool(true)` -> `true` (no suffix).
- `Char(u32)` -> `'<ch>'` via Rust's `char::Debug` form (Specimens 85,
  86 cover ASCII printable + escape).

The `Char` arm constructs a `char` from the `u32` payload (bridge has
validated validity per `spec.md` §9.7) and emits `format!("{:?}",
ch)`, which gives the Rust char-literal Debug form per
pretty-printer.md:2783-2794.

#### Int / Uint -- D-14-4.10 sentinel collapsing FIRST

Per D-14-4.10 LOCKED. Before applying the underscore-decimal form,
test the decoded value against the width-bounds sentinel constants
table at pretty-printer.md:2710-2729:

| Variant + decoded value | Output | Sentinel |
|---|---|---|
| `Int(I8)` MIN / MAX | `i8::MIN` / `i8::MAX` | `-128` / `127` |
| `Int(I16)` MIN / MAX | `i16::MIN` / `i16::MAX` | `-32768` / `32767` |
| `Int(I32)` MIN / MAX | `i32::MIN` / `i32::MAX` | `-2147483648` / `2147483647` |
| `Int(I64)` MIN / MAX | `i64::MIN` / `i64::MAX` | `-9223372036854775808` / `9223372036854775807` |
| `Int(I128)` MIN / MAX | `i128::MIN` / `i128::MAX` | `-(2^127)` / `2^127 - 1` |
| `Int(Isize)` MIN / MAX | `isize::MIN` / `isize::MAX` | target-pointer-width-dependent |
| `Uint(U8 / U16 / U32 / U64 / U128 / Usize)` MAX | `u8::MAX` / ... / `usize::MAX` | width-dependent |

Notes (pretty-printer.md:2731-2754):

- **No `_` width suffix on sentinel forms**. The type-name prefix
  carries the width unambiguously.
- **No unsigned MIN collapsing.** `Uint(_, 0)` always renders as
  `0_u<width>` decimal.
- **Isize / Usize render symbolically regardless of target.** Use
  `target.scalar_layouts.int[Isize].size` / `uint[Usize].size` from
  `spec.md` §11 ScalarLayouts to determine the sentinel constant for
  comparison; the rendered output is the symbolic form (Specimen 81
  is the canonical proof: 32-bit target with `0xFFFFFFFF` ->
  `usize::MAX`, NOT the per-target decimal).

#### Int / Uint -- decimal form when no sentinel matches

Per the decimal-rendering precision rules at
pretty-printer.md:2759-2782:

- For `Int(k, v)`: re-interpret the low `width(k)` bits of `v` as
  signed two's-complement, then print using Rust's `Display` for the
  resulting signed type. Storage is sign-extended `u128` per
  `spec.md` §9.7; decoding is `(v as i128) >> (128 - width(k))` for
  width < 128, identity for `I128`. `Isize` uses target-pointer-width
  from `ScalarLayouts`.
- For `Uint(k, v)`: print the low `width(k)` bits as unsigned
  decimal.

Output form: `<dec>_<type-name>` (e.g., `42_i32`, `255_u8`,
`16_usize`). No leading-zero or digit-grouping conventions.

#### Float -- finite vs non-finite dispatch

Per D-14-4.2 + D-14-4.12 LOCKED.

Finite values (pretty-printer.md:2796-2818):

- `Float(F16, bits)` -> `<val>f16` -- shortest round-trippable
  decimal display of the IEEE-754 binary16 bit pattern.
- `Float(F32, bits)` -> `<val>f32` -- binary32.
- `Float(F64, bits)` -> `<val>f64` -- binary64.
- `Float(F128, bits)` -> `<val>f128` -- binary128.

Integer-valued finite floats render without fractional part (`0f64`,
`-1f32`, `42f64`); non-integer finite floats render with a fractional
part (`1.5f32`, `3.14f64`). Negative zero is finite (`-0f32` per the
finite rule; the underscore form does NOT apply per
pretty-printer.md:2876-2879).

Non-finite values (table at pretty-printer.md:2825-2838) per
D-14-4.12:

- +inf -> `inf_f<width>`
- -inf -> `-inf_f<width>`
- NaN (any bit pattern, quiet / signaling / payload-bearing /
  sign-set) -> `NaN_f<width>` (single-form collapse per
  pretty-printer.md:2864-2875).

Classification: reconstitute the IEEE-754 value at the variant's
FloatKind width per the finite-precision rule, then invoke the
apfloat-style classification (`is_finite()` -> finite branch; else
+inf / -inf / NaN).

The underscore is **load-bearing for non-finite values only**
(`inf_f32` reads as a non-finite float vs `inff32` which would parse
as a single identifier per Rust lexer rules). Finite values do NOT
use the underscore (`1.5f32`, `0f64`, `-0f32`, `42f64`).

### §10.1.2 ZST -- per-Ty.kind() recursive dispatch

Per D-14-4.3 LOCKED. `Const::ZST` is the canonical single
representative for all ZST values per `spec.md` §9.7. The renderer
dispatches on the consumer `Ty.kind()` across the 7 categories in
the table at pretty-printer.md:2900-2919:

| Consumer Ty.kind() | Output |
|---|---|
| `Ty::Tuple { fields: [] }` | `()` |
| `Ty::Tuple { fields: [T1] }` | `(<r(zst, T1)>,)` (1-tuple trailing comma) |
| `Ty::Tuple { fields: [T1, ..., Tn] }` (n >= 2) | `(<r(zst, T1)>, <r(zst, T2)>, ...)` |
| `Ty::Array(_, 0)` | `[]` |
| `Ty::Array(elem, N>0)` ZST | `[<r(zst, elem)>; N]` (N decimal usize) |
| `Ty::Struct { item, args, layout: { kind: Struct { fields: [] }, .. } }` | `<render_def_path(item)><render_generic_args(args, item)>` |
| `Ty::Struct` tuple-style with all-ZST fields | `<path><args>(<r(zst, f0.ty)>, ...)` |
| `Ty::Struct` named-field with all-ZST fields | `<path><args> { <f0.name>: <r(zst, f0.ty)>, ... }` |
| `Ty::Enum { ... EnumShape::Single { variant: V, .. } }` `V.fields.is_empty()` | `<enum_path>::<V.name>` |
| Single-variant tuple-variant enum, all-ZST | `<enum_path>::<V.name>(<r(zst, V.fields[0].ty)>, ...)` |
| Single-variant named-field-variant enum, all-ZST | `<enum_path>::<V.name> { <V.fields[0].name>: <r(zst, V.fields[0].ty)>, ... }` |
| `Ty::Union { item, args, .. }` | `<render_def_path(item)><render_generic_args(args, item)>` (no field disambiguation at value level) |
| `Ty::FnDef(item, args)` | `<render_def_path(item)><render_generic_args(args, item)>` (rustc-mirrored value-namespace path; distinct from the §6.4 type-rendering form) |
| `Ty::Closure(item, args)` (non-capturing) | `<render_def_path(item)><render_generic_args(args, item)>` |

Panic / cannot-occur cases (renderer panics with assertion message,
matching the brief's "no defensive fallbacks" rule):

- `Ty::Enum { ... EnumShape::Multiple { .. } }` -- multi-variant enum
  carries a discriminant tag of nonzero size; cannot be ZST.
- `Ty::Enum { ... EnumShape::Empty }` -- zero-variant uninhabited
  enum has no values.
- `Ty::Never` -- uninhabited.
- Any non-ZST `Ty.kind()` (Bool / Int / Uint / Float / Char / Ref /
  RawPtr / Dynamic / etc.) -- non-ZST types cannot consume
  `Const::ZST`; the bridge produces a non-ZST Const variant.

#### Recursion shape

`r(zst, T)` denotes `self.render_const(zst_const_id, T)` where
`zst_const_id` is the SAME `ConstId` representing the unique
`Const::ZST` canonical representative per `spec.md` §9.7. Termination
is guaranteed because each recursive step descends into a structurally
smaller consumer Ty and eventually bottoms out at a leaf ZST Ty
(`Ty::Tuple([])`, fieldless `Ty::Struct`, `Ty::FnDef`, `Ty::Closure`,
etc.).

#### Tuple-vs-named-field dispatch (§3.4)

Field names matching the synthetic pattern `_<index>` (`_0`, `_1`,
...) signal tuple-style; non-synthetic names signal named-field-style;
empty fields signal unit-style. Per pretty-printer.md:2934-2939, this
matches the §3.4 shared dispatch rule.

### Cross-references

- §10.1.8 type-context threading (pretty-printer.md:3187-3282)
  documents the per-sub-field Ty resolution mechanism (parent
  consumer Ty -> per-child Ty) consumed by the recursive ZST arms
  here and by P1a-PP-04b's Aggregate / EnumValue arms.

## Non-Goals

- No `Aggregate / EnumValue / Bytes / NullPtr / Reloc` per-arm
  rendering. Owned by P1a-PP-04b (regseq-model-90); the per-variant
  match arm in `render_const` may stay `todo!()` until P1a-PP-04b
  lands.
- No `render_ty`, `render_def_path`, or `render_generic_args`
  reimplementation. Consumed via P1a-PP-02 / stage 12.
- No `render_instance` composition. Owned by P1a-PP-05a/b; the
  Static `init` value and Const `value` rendering paths consume
  `render_const` from the composer side.
- No top-level `render(&self)` driver. Owned by P1a-PP-06.
- No PrettyPrintOptions struct or with_options constructor. Owned by
  P1a-PP-06.
- No bridge-side const synthesis (`ConstMemoryStatic` Item creation,
  `Const::Aggregate` / `Const::Bytes` / etc. construction). Owned by
  axis 3 (P1a-BR-03a + P1a-BR-04).
- No closure-captures Aggregate Closure rendering at the const layer
  (Aggregates are P1a-PP-04b's domain).

## Test Plan

Test discipline names `unit-in-src` (per-arm dispatch logic;
sentinel-collapsing branch; D-14-4.12 finite vs non-finite
classification; ZST recursion termination) and `integration-in-tests`
(insta snapshots for §9.10 specimens 76 - 90).

Per the brief: define the API first with `todo!()` bodies (the full
11-variant match), then write tests against the simple-arm subset,
then implement.

### Unit tests (in `crates/regseq-model/src/pretty.rs`)

1. `Bool` true / false render without suffix.
2. Each `IntKind` / `UintKind` / `FloatKind` with a representative
   non-sentinel decoded value renders in the underscore-decimal form
   (`42_i32`, `255_u8`, `1f32`, ...).
3. Each sentinel from the §10.1.1 table renders symbolically (no
   `_<width>` suffix).
4. `Uint(_, 0)` does NOT collapse to `_::MIN` (no unsigned MIN
   collapsing).
5. `Isize` / `Usize` MAX renders as `isize::MAX` / `usize::MAX`
   regardless of target pointer width (set up the model with both 32-
   and 64-bit `ScalarLayouts` and verify both render symbolically).
6. `Char` ASCII printable -> `'a'`; escape -> `'\n'`; non-ASCII ->
   `'\u{...}'` via `char::Debug`.
7. `Float` finite: integer-valued -> `0f64`, `-0f32`, `42f64`;
   non-integer -> `1.5f32`, `3.14f64`. Negative zero finite ->
   `-0f32`.
8. `Float` non-finite per D-14-4.12: +inf -> `inf_f<w>`; -inf ->
   `-inf_f<w>`; NaN (quiet, signaling, payload-bearing, sign-set) ->
   `NaN_f<w>` single-form collapse.
9. ZST dispatch per consumer Ty.kind() across all rows in the
   §10.1.2 table; verify panic (or assertion failure) for the cannot-
   occur rows (multi-variant enum / empty enum / Never / non-ZST
   primitive).
10. ZST composite recursion: `Ty::Tuple([(), (), ()])` -> `((), (),
    ())` with the same `ConstId` threaded through (Specimen 90).
11. ZST PhantomData<u32> -> `core::marker::PhantomData::<u32>`
    (Specimen 89; exercises generic-args composition through
    `render_generic_args`).

### Integration tests (`crates/regseq-model/tests/pretty.rs`)

Add insta snapshot tests against the §9.10 specimens covered by this
task:

- Specimens 76, 77: Bool true / false.
- Specimen 78: Int decimal non-sentinel.
- Specimens 79, 80: Int sentinel MAX / MIN.
- Specimen 81: Uint sentinel target-pointer-width-independent.
- Specimens 82, 83, 84: Float finite, +inf, NaN.
- Specimens 85, 86: Char ASCII, char escape.
- Specimens 87, 88, 89, 90: ZST unit, ZST FnDef, ZST PhantomData,
  ZST composite tuple recursion.

(Specimens 91 - 92 NullPtr and 93 - 96 Reloc and 97 - 109 Aggregate
/ EnumValue / Bytes belong to P1a-PP-04b.)

Add `insta` to `crates/regseq-model/Cargo.toml` `[dev-dependencies]`
if not already present.

### Test-discipline tags

- `unit-in-src` -- per-arm dispatch logic; sentinel-collapsing;
  D-14-4.12 finite/non-finite classification; ZST recursion
  termination.
- `integration-in-tests` -- insta snapshots for §9.10 specimens
  76 - 90.

## Review

After tests pass, request a Codex review using `codex_runner.py`:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-model-89 (P1a-PP-04a) against
its task spec at .harness/tasks/regseq-model/89-render-const-scalar.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-model/src/pretty.rs
- crates/regseq-model/tests/pretty.rs
- (any new insta snapshot files under crates/regseq-model/tests/snapshots/)

Focus on:
- Spec deviations: §10.1.1 scalar table; D-14-4.10 sentinel
  collapsing (no `_<width>` suffix on sentinel; no unsigned MIN
  collapsing; symbolic Isize/Usize); D-14-4.12 non-finite Float
  classification (NaN single-form collapse).
- §10.1.2 ZST per-Ty.kind() dispatch table is exhaustive; recursion
  threads the same `ConstId` per the canonical-form rule; cannot-
  occur rows panic.
- Pair-form signature `render_const(ConstId, TyId)` per D-14-4.1
  and the §8 ownership table.
- Test coverage vs spec Test Plan: every Specimen 76 - 90 has a
  matching insta snapshot.
Do NOT flag anything explicitly marked out of scope (Aggregate /
EnumValue / Bytes / NullPtr / Reloc arms which are P1a-PP-04b;
later task IDs P1a-PP-05a/b through P1a-PP-06).
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

- `render_const(ConstId, TyId) -> String` implemented per the §8
  ownership-table signature; the per-variant match covers all 11
  Const variants (Aggregate / EnumValue / Bytes / NullPtr / Reloc
  arms may stay `todo!()` until P1a-PP-04b lands).
- Bool / Char / Int / Uint / Float / ZST arms implemented per
  pretty-printer.md §10.1.1 / §10.1.2:
  - D-14-4.10 sentinel collapsing fires before decimal form.
  - D-14-4.12 non-finite Float collapses to `inf_f<w>` / `-inf_f<w>`
    / `NaN_f<w>`.
  - ZST consumer-Ty dispatch covers all rows in the §10.1.2 table;
    composite recursion threads the same `Const::ZST` ConstId.
  - Cannot-occur rows panic with informative messages.
- All §9.10 specimens 76 - 90 have matching insta snapshot tests;
  outputs match verbatim.
- Unit tests cover sentinel collapsing, non-unsigned-MIN, target-
  pointer-width independence for Isize/Usize, NaN single-form
  collapse, and ZST composite recursion.
- `cargo test -p regseq-model`, `cargo fmt --all -- --check`, and
  `cargo clippy -p regseq-model -- -D warnings` all pass.
- Codex Task Review verdict APPROVED.
