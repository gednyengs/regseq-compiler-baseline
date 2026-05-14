# regseq-model-87: Type rendering -- `render_ty` per-arm specs (P1a-PP-02)

## Description

Implement the stage-13 building-block `render_ty(TyId) -> String` on
`PrettyPrinter<'_>`, dispatching across the closed 19-variant `Ty`
enum (`spec.md` §10). Implement the §6.7 helper
`render_generic_args(args: &GenericArgs, item: ItemId) -> String` with
the stage-14 D-14-4.9 signature (the `item: ItemId` parameter is
required to resolve the declared type of each `GenericArg::Const(c)`
arg from the enclosing Item's generic-parameter list per D-14-4.1's
pair-form).

The output is the Rust-source-style type rendering pinned by
`pretty-printer.md` §6.1 - §6.6. All TyIds resolve recursively via
`render_ty`; ADT / FnDef / Closure paths compose
`render_def_path(item) + render_generic_args(args, item)`; FnPtr
composes `[unsafe ][extern "ABI" ]fn(<args>) -> <ret>`; Dynamic
composes `dyn <vtable-DefPath>[ + <Auto>...]` per the §13.4
`<regseq_synth>::vtable::<Trait>::for::<ImplTy>` synthesised path.

## Input Context

- `.harness/SESSION.md`
- `.harness/spec-batches/regseqmodel-phase-1a.md` (per-task scope row
  P1a-PP-02)
- `docs/discussions/regseq-model/pretty-printer.md` §1 (Mission), §2
  (Master rendering style), §6.1 - §6.7 (Type rendering per-arm
  specs), §8 (Per-method ownership table at
  `pretty-printer.md:1148-1170` -- canonical signatures for
  `render_ty` and `render_generic_args`), §9.1 (`render_ty` specimens
  1-25)
- `docs/discussions/regseq-model/spec.md` §9.1 (`GenericArg` 2-variant
  enum: `Type(TyId)` / `Const(ConstId)`), §10 (`Ty` 19-variant enum at
  `spec.md:3643-3663`), §10.1 (auto-trait sort order), §10.3
  (wide-scalar admission), §13.2 / §13.4 (synthesised vtable struct +
  the `<regseq_synth>::vtable::<Trait>::for::<ImplTy>` DefPath
  encoding), §14.1 (`FnAbi` 2-variant enum), §14.3 (FnSig schema)
- `crates/regseq-model/src/pretty.rs` (replace the existing
  `render_ty` stub)
- The `Ty`, `IntKind`, `UintKind`, `FloatKind`, `Provenance`,
  `Mutability`, `AutoTrait`, `FnAbi`, `Safety` types from
  regseq-model-DS-02 (P1a-DS-02)
- The `Layout` machinery from regseq-model-DS-03 (P1a-DS-03; consumed
  by ADT arms via the `layout: Layout` carry on Tuple / Struct / Enum
  / Union variants per `spec.md:3805-3855`)
- The `TyInterner` and `GenericArgs` shape from regseq-model-DS-04
  (P1a-DS-04)
- The synthesised vtable Item shape from regseq-model-DS-08b
  (P1a-DS-08b)
- The closure-as-struct identity from regseq-model-DS-08a (P1a-DS-08a)
- `crates/regseq-model/src/def_path.rs` (`render_def_path` -- already
  shipped at stage 12)
- `.harness/tasks/regseq-model/24-place-and-projection.md` (size /
  shape reference)

## Implementation Notes

Edit `crates/regseq-model/src/pretty.rs`. The §8 ownership table at
`pretty-printer.md:1148-1170` is the canonical reference for the two
methods this task owns.

### Public API

Per the §8 ownership table rows:

```rust
impl PrettyPrinter<'_> {
    /// 19-variant `Ty` enum dispatch per pretty-printer.md §6.
    pub fn render_ty(&self, id: TyId) -> String;

    /// `<T1, T2, K1>` form; empty `String` when args is empty.
    /// Signature per stage-14 D-14-4.9: takes the enclosing `ItemId`
    /// alongside the args so `GenericArg::Const(c)` resolves the
    /// declared parameter type from `model.items[item]`'s
    /// generic-parameter list at the matching positional index.
    /// Per pretty-printer.md §6.7.
    pub fn render_generic_args(&self, args: &GenericArgs, item: ItemId) -> String;
}
```

### §6.1 Primitives (table at pretty-printer.md:610-633)

| Variant | Output |
|---|---|
| `Bool` | `bool` |
| `Char` | `char` |
| `Int(I8/I16/I32/I64/I128/Isize)` | `i8 / i16 / i32 / i64 / i128 / isize` |
| `Uint(U8/U16/U32/U64/U128/Usize)` | `u8 / u16 / u32 / u64 / u128 / usize` |
| `Float(F16/F32/F64/F128)` | `f16 / f32 / f64 / f128` |
| `Str` | `str` |
| `Never` | `!` |

### §6.2 Compound non-recursive (table at pretty-printer.md:635-650)

| Variant | Output |
|---|---|
| `Array(T, n)` | `[<render_ty(T)>; <n>]` (`n` decimal) |
| `Slice(T)` | `[<render_ty(T)>]` |
| `Ref(T, Provenance::SharedBorrow)` | `&<render_ty(T)>` |
| `Ref(T, Provenance::UniqueBorrow)` | `&mut <render_ty(T)>` |
| `Ptr(T, Mutability::Not)` | `*const <render_ty(T)>` |
| `Ptr(T, Mutability::Mut)` | `*mut <render_ty(T)>` |

`Ref` arms render WITHOUT any `'a` lifetime token; `Provenance` is
borrow flavor only per `spec.md` §10.1.

### §6.3 Tuple (table at pretty-printer.md:652-660)

- `Tuple { fields: [] }` -> `()`
- `Tuple { fields: [T] }` -> `(<render_ty(T)>,)` (explicit trailing
  comma for arity 1)
- `Tuple { fields: [T1, T2, ...] }` ->
  `(<render_ty(T1)>, <render_ty(T2)>, ...)`

### §6.4 ADT and FnDef / Closure (table at pretty-printer.md:662-679)

- `Struct { item, args, .. }`,
  `Enum { item, args, .. }`,
  `Union { item, args, .. }`,
  `Closure(item, args)`
  -> `<render_def_path(item)><render_generic_args(args, item)>`
- `FnDef(item, args)` ->
  `fn(<arg_tys>) -> <ret_ty> {<render_def_path(item)><render_generic_args(args, item)>}`

The `FnDef` arm looks up the matching Instance by walking
`model.instances` for `instance.item == item && instance.generic_args
== args`, then resolves `instance.kind = InstanceKind::Function {
fn_sig, .. }` to get the FnSigId, then renders arg types and return
type via `render_ty`. The curly-brace form is rustc-faithful (FnDef is
not expressible as a type expression).

### §6.5 Function pointer (table at pretty-printer.md:681-690)

- `FnPtr(fnsig)` ->
  `[unsafe ][extern "ABI" ]fn(<arg_tys>) -> <ret_ty>`

`unsafe ` prefix when `FnSig.safety == Safety::Unsafe` (suppressed
when `Safe`). `extern "ABI" ` prefix when `FnSig.abi != FnAbi::Rust`
(default `Rust` is suppressed; `C` -> `extern "C"`). Args separated
by `, `; return type always rendered including unit case `-> ()`.

### §6.6 Dynamic (rules at pretty-printer.md:692-711)

`Dynamic { auto_traits: Vec<AutoTrait>, vtable_ty: TyId }` renders as:

```
dyn <render_def_path(vtable_item)> + <Auto1> + <Auto2>
```

Where:

- `vtable_item` is recovered via `vtable_ty -> Ty::Struct { item, .. }
  -> item`. The DefPath encodes the principal trait + impl per
  `spec.md` §13.4 in the form
  `<regseq_synth>::vtable::<Trait>::for::<ImplTy>` (specimen 23
  exercises this exact form).
- Auto-traits render in `spec.md` §10.1 sorted order (Send < Sync <
  Unpin), separated by ` + `.
- Empty `auto_traits: []` omits the `+ ...` suffix; renders just
  `dyn <vtable-DefPath>` (specimen 24).

### §6.7 `render_generic_args(args, item)` helper (rules at pretty-printer.md:713-751)

Form: `<T1, T2, K1>` non-empty; empty `String` when `args.is_empty()`.
Args separated by `, `; the caller (FnDef / Closure / Struct / Enum /
Union arm; or `render_const` callers per §10.1.3 - §10.1.7) wraps in
`<...>`.

Per-arg dispatch:

- `GenericArg::Type(ty)` -> `self.render_ty(ty)`.
- `GenericArg::Const(c)` -> `self.render_const(c, param_ty)` where
  `param_ty` is the declared type of this const-generic parameter,
  looked up positionally from `model.items[item]`'s generic-parameter
  list at the matching index.

The `arg-position -> parameter-position` mapping is positional
(rustc-mirrored). The helper consumes `render_const` (P1a-PP-04a/b)
for the Const arm. If `render_const` is not yet available at this
task's landing time, the implementation may defer the Const-arm body
to its `todo!()` placeholder; the helper signature itself is fixed by
this task and the Const-arm wiring lands when P1a-PP-04a closes.

### Composition rules summarised

- `Ty::Struct/Enum/Union/Closure` and `Ty::FnDef` compose
  `<render_def_path(item)>` then `<render_generic_args(args, item)>`
  (concatenated; the helper returns the empty string for empty args
  so no extra punctuation needed).
- `Ty::FnDef`'s outer wrapping is
  `fn(<arg_tys>) -> <ret_ty> { <inner> }`.
- `Ty::FnPtr`'s prefix order: `unsafe ` (if `Unsafe`), then
  `extern "ABI" ` (if non-Rust ABI), then `fn(<args>) -> <ret>`.
- `Ty::Dynamic`'s output starts with `dyn ` then the synthesised
  vtable struct DefPath, then ` + ` joined sorted auto-traits.

### Cross-references

- §3.3 sigil-conventions table: `TyId` renders via `render_ty(TyId)`
  recursive on `Ty` variants per §6.
- §10.3 wide-scalar admission (`Int(I128)`, `Uint(U128)`, `Float(F16)`,
  `Float(F128)`): the type-rendering side renders all widths
  unconditionally; bridge / backend rejection is out of scope.

## Non-Goals

- No top-level `render(&self)` or per-Instance composition. Owned by
  P1a-PP-05a/b and P1a-PP-06.
- No `render_const` implementation. Owned by P1a-PP-04a/b. The §6.7
  helper's Const-arm wiring depends on `render_const`; if landing
  before P1a-PP-04a, the helper's Const-arm body may stay `todo!()`
  (the helper signature itself is fixed here).
- No body-level renderers (Place / Operand / Rvalue / Statement /
  Terminator / BasicBlock). Owned by P1a-PP-03.
- No Item-level rendering (`render_item` / attribute block). Owned by
  P1a-PP-01.
- No span / debuginfo gating: type rendering does not consume Spans.
- No FnSig / Layout schema additions; this task consumes the shapes
  defined by P1a-DS-02, P1a-DS-03, P1a-DS-06.
- No TyInterner mutations (no on-render interning). The renderer
  walks existing interned TyIds only.

## Test Plan

Test discipline names `unit-in-src` (per-arm dispatch logic) and
`integration-in-tests` (insta snapshots for the §9.1 specimens
1-25). Per the brief's cross-cutting rule: define the API first with
`todo!()` bodies, then write tests, then implement.

### Unit tests (in `crates/regseq-model/src/pretty.rs`'s `#[cfg(test)] mod tests`)

Cover per-arm dispatch shape:

1. Each primitive variant (`Bool`, `Char`, all 6 IntKind, all 6
   UintKind, all 4 FloatKind, `Str`, `Never`) renders to its §6.1
   row.
2. `Array` decimal `n` rendering.
3. `Slice` element-type recursion.
4. `Ref` and `Ptr` mutability dispatch (4 combinations).
5. Tuple arity 0 / 1 / 2 (specimens 14, 15, 16).
6. ADT + FnDef + Closure path composition with empty and non-empty
   args.
7. FnPtr prefix combinations: safe Rust, safe C, unsafe Rust, unsafe
   C.
8. Dynamic with empty auto-traits and with sorted auto-traits.
9. `render_generic_args` returns empty `String` for empty args; emits
   `<T1, T2>` for two Type args.

### Integration tests (`crates/regseq-model/tests/pretty.rs`)

Add insta snapshot tests against the §9.1 specimens 1 - 25
(pretty-printer.md:1191-1318). One snapshot per specimen, named
after the specimen (e.g., `render_ty_specimen_01_bool`,
`render_ty_specimen_23_dynamic_with_auto_traits`).

The 25 specimens to cover:

- Specimens 1 - 7: primitives (`Bool`, representative `Int`, `Uint`,
  `Float`, `Char`, `Str`, `Never`).
- Specimen 8: `Array(<u8>, 32)` -> `[u8; 32]`.
- Specimen 9: `Slice(<u8>)` -> `[u8]`.
- Specimens 10 - 11: `Ref` shared / unique.
- Specimens 12 - 13: `Ptr` const / mut.
- Specimens 14 - 16: tuple arity 0 / 1 / 2.
- Specimens 17 - 19: `Struct`, `Enum<u32>`, `Union`.
- Specimen 20: `FnDef` with curly-brace wrapping.
- Specimens 21 - 22: `FnPtr` safe Rust / unsafe extern "C".
- Specimen 23: `Dynamic` with auto-traits and synthesised vtable
  DefPath form `<regseq_synth>::vtable::Reader::for::<std::fs::File>`.
- Specimen 24: `Dynamic` without auto-traits.
- Specimen 25: `Closure` with generic args.

Add `insta` to `crates/regseq-model/Cargo.toml` `[dev-dependencies]`
if not already present.

### Test-discipline tags

- `unit-in-src` -- per-arm dispatch logic.
- `integration-in-tests` -- insta snapshots for §9.1 specimens 1-25.

## Review

After tests pass, request a Codex review using `codex_runner.py` per
`.harness/SESSION.md` Review section:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-model-87 (P1a-PP-02) against its
task spec at .harness/tasks/regseq-model/87-render-ty.md. ALL CHECKS
MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-model/src/pretty.rs
- crates/regseq-model/tests/pretty.rs
- (any new insta snapshot files under crates/regseq-model/tests/snapshots/)

Focus on:
- Spec deviations: every row in §6.1, §6.2, §6.3, §6.4, §6.5, §6.6,
  and the §6.7 helper signature (D-14-4.9).
- Correctness issues: Auto-trait sort order (§10.1); FnPtr prefix
  ordering (`unsafe ` then `extern "ABI" `); Dynamic vtable DefPath
  recovery (§13.4); FnDef instance lookup composition.
- Test coverage vs the spec's Test Plan: every §9.1 specimen
  (Specimens 1 - 25) has a matching insta snapshot.
Do NOT flag anything explicitly marked out of scope (Non-Goals, later
task IDs P1a-PP-03 through P1a-PP-06; the Const-arm wiring of
`render_generic_args` if P1a-PP-04a has not landed yet).
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

- `render_ty(TyId) -> String` implemented for all 19 closed variants
  per §6.1 - §6.6.
- `render_generic_args(args, item)` implemented per §6.7 with the
  D-14-4.9 signature; the Const-arm composition wires through
  `render_const` (or stays `todo!()` if P1a-PP-04a has not landed).
- All §9.1 worked-example specimens (Specimens 1 - 25) have matching
  insta snapshot tests; outputs match verbatim.
- Unit tests cover per-variant dispatch shape (primitives, compound,
  tuple, ADT/FnDef/Closure, FnPtr prefix combos, Dynamic auto-trait
  dispatch).
- `cargo test -p regseq-model`, `cargo fmt --all -- --check`, and
  `cargo clippy -p regseq-model -- -D warnings` all pass.
- Codex Task Review verdict APPROVED.
