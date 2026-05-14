# regseq-model-88: Body-level building-block renderers (P1a-PP-03)

## Description

Implement the 9 stage-13 public body-level renderers plus the 1
internal helper from `pretty-printer.md` §7.1 - §7.10 on
`PrettyPrinter<'_>`:

1. `render_place(&Place, &Body) -> String` (§7.1)
2. `render_operand(&Operand, &Body, expected_ty: TyId) -> String`
   (§7.2; pair-form per stage-14 D-14-4.9)
3. `render_rvalue(&Rvalue, &Body, expected_ty: TyId) -> String` (§7.3;
   pair-form per stage-14 D-14-4.9)
4. `render_statement(&Statement, &Body) -> String` (§7.4)
5. `render_terminator(&Terminator, &Body) -> String` (§7.5)
6. `render_terminator_kind(&TerminatorKind, &Body) -> String` (§7.6
   internal helper)
7. `render_call_callee(&CallCallee, &Body) -> String` (§7.7)
8. `render_switch_targets(&SwitchTargets, &Body) -> String` (§7.8)
9. `render_assert_kind(&AssertKind, &Body) -> String` (§7.9)
10. `render_basic_block(&BasicBlock, &Body) -> String` (§7.10)

Also own the §4 span / debuginfo gating for the **body-level** source
positions (Statement.span, Terminator.span, LocalDecl.span -- per the
brief's per-task scope statement). Item-header span gating belongs to
P1a-PP-01; function-body envelope (locals declaration block + arg
locals) belongs to P1a-PP-05a.

The pair-form signatures on `render_operand` and `render_rvalue`
thread the consumer-context `TyId` so they compose cleanly with
`render_const(ConstId, TyId)` in P1a-PP-04a/b. This task may land
before P1a-PP-04a; the `Operand::Constant` arm of `render_operand`
calls `render_const(id, expected_ty)` (which may be `todo!()` until
P1a-PP-04a closes; the signature is fixed here).

## Input Context

- `.harness/SESSION.md`
- `.harness/spec-batches/regseqmodel-phase-1a.md` (per-task scope row
  P1a-PP-03)
- `docs/discussions/regseq-model/pretty-printer.md` §1 (Mission), §2
  (Master rendering style), §3.1 (Body envelope), §3.2 (Indentation),
  §3.3 (Sigil conventions), §3.4 (Field / variant name resolution
  via `place_ty` walk), §4 (Span-rendering and debuginfo modes;
  especially §4.2 / §4.3 / §4.4 for body-level span gating), §7.1 -
  §7.10 (Body-level rendering per-arm specs), §8 (Per-method
  ownership table at `pretty-printer.md:1148-1170`), §9.2 (`render_place`
  specimens), §9.3 (`render_operand` specimens), §9.4 (`render_rvalue`
  specimens), §9.5 (`render_statement` specimens), §9.6
  (`render_terminator` specimens), §9.7 (`render_assert_kind`
  specimens), §9.8 (`render_basic_block` specimens), §10.1.8
  (Type-context threading and the operand-callsite wiring contract;
  the table at `pretty-printer.md:3244-3251` defines the per-caller
  expected_ty resolution rules)
- `docs/discussions/regseq-model/spec.md` §11 (Layout schema; consumed
  by §3.4 name resolution), §19 (Place + PlaceElem 6-variant enum),
  §20 (Operand vocabulary), §21 (Rvalue 9-variant enum + CastKind +
  AggregateKind 7-variant enum + 26-variant BinOp + UnOp), §22
  (Statement 3-variant + NonDivergingIntrinsic), §23 (Terminator
  6-variant + CallCallee + SwitchTargets + 14-variant AssertKind),
  §24 (BasicBlock + B-NonEmpty invariant), §25 (Body schema, LocalDecl
  with `debug_name: Option<StringId>` and `span: Span`)
- `crates/regseq-model/src/pretty.rs` (the existing `PrettyPrinter`
  shell)
- The Place / PlaceElem types from regseq-model-DS-11 (P1a-DS-11)
- The Operand / LiteralValue from regseq-model-DS-12a (P1a-DS-12a)
- The Rvalue / CastKind / AggregateKind / BinOp / UnOp from
  regseq-model-DS-12b (P1a-DS-12b)
- The Statement / StatementKind / NonDivergingIntrinsic from
  regseq-model-DS-13a (P1a-DS-13a)
- The Terminator / TerminatorKind / CallCallee / SwitchTargets /
  AssertKind from regseq-model-DS-13b (P1a-DS-13b)
- The BasicBlock / BlockId from regseq-model-DS-14a (P1a-DS-14a)
- The Body / LocalDecl / LocalId from regseq-model-DS-14b
  (P1a-DS-14b)
- `crates/regseq-model/src/def_path.rs` and the §6 `render_ty`
  implementation from regseq-model-87 (P1a-PP-02)
- `.harness/tasks/regseq-model/24-place-and-projection.md` (size /
  shape reference)

## Implementation Notes

Edit `crates/regseq-model/src/pretty.rs`. The §8 ownership table at
`pretty-printer.md:1148-1170` is the canonical reference for the 10
methods this task owns.

### Public API (per §8 ownership table rows)

```rust
impl PrettyPrinter<'_> {
    pub fn render_place(&self, place: &Place, body: &Body) -> String;
    pub fn render_operand(&self, op: &Operand, body: &Body, expected_ty: TyId) -> String;
    pub fn render_rvalue(&self, rv: &Rvalue, body: &Body, expected_ty: TyId) -> String;
    pub fn render_statement(&self, stmt: &Statement, body: &Body) -> String;
    pub fn render_terminator(&self, term: &Terminator, body: &Body) -> String;
    pub fn render_call_callee(&self, callee: &CallCallee, body: &Body) -> String;
    pub fn render_switch_targets(&self, targets: &SwitchTargets, body: &Body) -> String;
    pub fn render_assert_kind(&self, msg: &AssertKind, body: &Body) -> String;
    pub fn render_basic_block(&self, block: &BasicBlock, body: &Body) -> String;

    // Internal helper (not public outside the renderer family):
    fn render_terminator_kind(&self, kind: &TerminatorKind, body: &Body) -> String;
}
```

### §7.1 `render_place` (rules at pretty-printer.md:759-794)

Head local: `_<local.index()>`. Each `PlaceElem` wraps the prior
result via the wrap table at pretty-printer.md:769-778:

| PlaceElem | Wrap |
|---|---|
| `Deref` | `(*X)` |
| `Field(idx, ty)` | `(X.<field_name>: <render_ty(ty)>)` |
| `Index(local)` | `X[_<local.index()>]` |
| `ConstantIndex { offset, min_length, from_end: false }` | `X[<offset> of <min_length>]` |
| `ConstantIndex { offset, min_length, from_end: true }` | `X[-<offset> of <min_length>]` |
| `Subslice { from, to, from_end: false }` | `X[<from>..<to>]` |
| `Subslice { from, to, from_end: true }` | `X[<from>..-<to>]` |
| `Downcast(variant_idx)` | `(X as <variant_name>)` |

Field / variant name resolution uses the §3.4 `place_ty` walk over
`body.local_decls[place.local].ty` plus the projection chain, with
layout dispatch through `Layout.kind` per `spec.md` §11. Per §3.4
both `FieldLayout.name` and `VariantLayout.name` are non-optional;
missing names are invalid IR (validation rejects).

### §7.2 `render_operand` (rules at pretty-printer.md:795-826)

Pair-form per stage-14 D-14-4.9 REOPEN of stage-13 D-13-6:

| Variant | Output |
|---|---|
| `Copy(p)` | `copy <render_place(p, body)>` (expected_ty ignored) |
| `Move(p)` | `move <render_place(p, body)>` (expected_ty ignored) |
| `Constant(id)` | `<render_const(id, expected_ty)>` |

`Copy` / `Move` ignore `expected_ty`; `Constant` threads it to
`render_const`. The signature uniformly requires `expected_ty` for
API symmetry per pretty-printer.md:801-812.

### §7.3 `render_rvalue` (table at pretty-printer.md:843-855)

Pair-form per D-14-4.9. The caller (`render_statement` Assign arm)
threads `expected_ty = place_ty(place, body)`. Each arm derives
per-operand expected Tys:

- `Use(operand)` -> transparent: `<render_operand(operand, body,
  expected_ty)>`.
- `Repeat(operand, count)` -> `[<render_operand(op, body, elem_ty)>;
  <count>]` where `elem_ty` derived from
  `expected_ty = Ty::Array(elem, count)`.
- `Ref(Provenance::SharedBorrow, place)` ->
  `&<render_place(place, body)>`.
- `Ref(Provenance::UniqueBorrow, place)` ->
  `&mut <render_place(place, body)>`.
- `RawPtr(Mutability::Not, place)` ->
  `&raw const <render_place(place, body)>`.
- `RawPtr(Mutability::Mut, place)` ->
  `&raw mut <render_place(place, body)>`.
- `Cast(kind, operand, ty)` -> `<render_operand(operand, body,
  source_ty)> as <render_ty(ty)>` plus optional
  ` (CastKind)` annotation per §7.3.1
  (pretty-printer.md:857-895). Annotation fires for
  `PointerCoercion(ReifyFnPointer / UnsafeFnPointer / ClosureFnPointer
  / Unsize)` and `Subtype`; all other CastKind variants render bare.
- `BinaryOp(op, lhs, rhs)` ->
  `<render_operand(lhs, body, op_input_ty)> <op_glyph> <render_operand(rhs, body, op_input_ty)>`.
  Comparison ops (`Eq / Lt / Le / Ne / Ge / Gt`) derive operand type
  via operand inspection (both operands share the same source-MIR
  type per `Rvalue::BinaryOp` invariants). Arithmetic / shift ops
  use `expected_ty`.
- `UnaryOp(op, operand)` ->
  `<op_glyph><render_operand(operand, body, expected_ty)>`. Operand
  type = expected_ty (UnaryOp preserves type).
- `Discriminant(place)` -> `discriminant(<render_place(place, body)>)`.
- `Aggregate(kind, operands)` -> per `AggregateKind` arm in §7.3.2
  (table at pretty-printer.md:910-924). Children render via
  `render_operand(op_i, body, per_field_ty_i)` where the per-field Ty
  resolves from the AggregateKind's structural payload + the
  destination type's layout. Cover all variants: Array, Tuple
  (arity 0 / 1 / 2+), Struct (tuple-variant + struct-variant per
  §3.4 field-name pattern dispatch), Enum (C-style + tuple-variant
  + struct-variant per `EnumShape` dispatch), Union (single active
  field), Closure (upvar names from the parallel ClosureCaptures
  struct's layout per §13.1), RawPtr (thin = unit `()` metadata;
  fat = full metadata operand).

### §7.4 `render_statement` (table at pretty-printer.md:971-977)

| StatementKind | Output |
|---|---|
| `Assign(place, rvalue)` | `<render_place(place, body)> = <render_rvalue(rvalue, body, place_ty)>;` |
| `SetDiscriminant(place, variant_idx)` | `discriminant(<render_place(place, body)>) = <variant_name>;` |
| `Intrinsic(NonDivergingIntrinsic::Assume(operand))` | `assume(<render_operand(operand, body, Ty::Bool)>);` |
| `Intrinsic(NonDivergingIntrinsic::CopyNonOverlapping{src, dst, count})` | `copy_nonoverlapping(<src>, <dst>, <count>);` with src expected_ty `*const ()`, dst `*mut ()`, count `usize` |

Trailing `;` always present.

### §7.5 / §7.6 `render_terminator` and `render_terminator_kind`

Output: `<render_terminator_kind(...)> -> [<successor_labels>]`. No
trailing `;` (the composer in §7.10 appends it).

Per `TerminatorKind` (table at pretty-printer.md:1000-1007):

- `Goto(b)` -> head `goto`, succ `[bb<N>]`.
- `SwitchInt { discr, targets }` -> head
  `switchInt(<render_operand(discr, body, discr_ty)>)`, succ
  `[<render_switch_targets(targets, body)>]`. Discr type via
  operand inspection.
- `Return` -> head `return`, succ `[]`.
- `Unreachable` -> head `unreachable`, succ `[]`.
- `Call { callee, args, destination, target, .. }` -> head
  `<render_place(destination, body)> = <render_call_callee(callee,
  body)>(<arg0>, <arg1>, ...)`. Each arg's expected_ty =
  `callee_fn_sig.inputs[i]` resolved via the Direct InstanceId's
  Function FnSig or the Indirect operand's `Ty::FnPtr` sig. Succ
  `[return: bb<N>]` when `target = Some(b)`; `[]` for diverging
  calls.
- `Assert { cond, expected, msg, target }` -> head
  `assert(<expected>, <render_operand(cond, body, Ty::Bool)>,
  <render_assert_kind(msg, body)>)`, succ `[success: bb<N>]`.

`<expected>` renders as `true` / `false` literal per the bool field.

### §7.7 `render_call_callee` (table at pretty-printer.md:1018-1022)

| CallCallee | Output |
|---|---|
| `Direct(id)` | `<render_def_path(instance.item)><render_generic_args(instance.generic_args, instance.item)>` |
| `Indirect(opand)` | `<render_operand(opand, body, Ty::FnPtr(sig_id))>` |

For `Direct`, `instance = model.instances[id]`; `instance.generic_args`
is carried at the Instance struct level per `spec.md` §9.2.

### §7.8 `render_switch_targets` (rules at pretty-printer.md:1031-1047)

```
<value0>: bb<N0>, <value1>: bb<N1>, ..., otherwise: bb<N_otherwise>
```

Values render as decimal `u128`. No surrounding brackets at this
helper level (the `[...]` brackets are added by `render_terminator`).

### §7.9 `render_assert_kind` (table at pretty-printer.md:1060-1075)

14 variants (see the §7.9 table). Each emits its fixed-template
message string with placeholders rendered via
`render_operand(_, body, expected_ty)` where expected_ty per the
per-placeholder derivation table at pretty-printer.md:1083-1104:

- `BoundsCheck { len, index }`: both `Ty::Uint(Usize)`.
- `Overflow(Add / Sub / Mul / Div / Rem)`: both `l` and `r` share the
  BinOp input numeric primitive (resolved via operand inspection).
- `Overflow(Shl / Shr)`: `l` numeric, `r` `Ty::Uint(_)` shift-amount.
- `OverflowNeg / DivisionByZero / RemainderByZero`: numeric primitive
  via operand inspection.
- `MisalignedPointerDereference { required, found }`: both
  `Ty::Uint(Usize)`.
- `InvalidEnumConstruction(found)`: discriminant scalar type per the
  source enum's tag.
- `NullPointerDereference`: no operand placeholders.

### §7.10 `render_basic_block` (rules at pretty-printer.md:1106-1138)

Envelope:

```
bb<N>: {
    <stmt_0>;
    <stmt_1>;
    ...
    <terminator> -> [<successor_labels>];
}
```

Composition:

- Header line `bb<N.index()>: {` at envelope indent (4 spaces;
  `INDENT = "    "` per §3.2).
- Per-statement lines: 8-space indent, each rendered via
  `render_statement` (which provides the trailing `;`).
- Terminator line: 8-space indent, rendered via `render_terminator`
  (which provides head text + `-> [labels]`); `;` appended here by
  `render_basic_block`.
- Closing line `}` at envelope indent (4 spaces).

A zero-statement block (terminator only, per the B-NonEmpty invariant
in `spec.md` §24) renders compactly per pretty-printer.md:1129-1138.

### §4 span gating (Statement / Terminator / LocalDecl source positions)

This task owns the span emission for the body-level constructs per
the brief's per-task scope. Rules per `pretty-printer.md` §4.2 +
§4.3 + §4.4:

- `PrettyPrintOptions::include_spans == true` (default off):
  - `render_statement` emits a trailing `// <render_span(stmt.span)>`
    comment aligned to column `ALIGN = 40` per §4.2.
  - `render_terminator` emits the same trailing comment for
    `terminator.span` after the `]` closing bracket.
  - LocalDecl span emission applies when this task renders a
    locals-declaration line; the function-body envelope's
    locals-declaration block is owned by P1a-PP-05a, but the per-line
    span gating rule (`// <render_span(local_decl.span)>`) is shared
    discipline. Per the brief's per-task scope: PP-03 owns the
    LocalDecl span gating; PP-05a composes the envelope and calls
    into a per-LocalDecl render helper exposed by this task.
  - In all three cases, suppress the comment unconditionally when
    `span.file == SourceFileId::SYNTHESIZED` per §4.3.
- `include_debug_names == true`:
  - This task does NOT render the `debug NAME => _N;` lines (those
    belong to the function-body envelope owned by P1a-PP-05a). The
    `debug_name: Option<StringId>` field on `LocalDecl` is consumed
    by the envelope, not by per-line body rendering.

### Pair-form composition contract

`render_operand` and `render_rvalue` consume `render_const(ConstId,
TyId)` (P1a-PP-04a/b). If P1a-PP-04a/b have not landed at this
task's start, the `Operand::Constant` arm of `render_operand` and
the `Rvalue::Use(Operand::Constant(_))` arm of `render_rvalue` may
defer the `render_const` call to its `todo!()` placeholder; the
signatures and dispatch shapes are fixed by this task.

## Non-Goals

- No `render_const` implementation. Owned by P1a-PP-04a/b.
- No `render_item` / attribute block / Item-header span gating.
  Owned by P1a-PP-01.
- No top-level `render(&self)` driver or PrettyPrintOptions struct
  definition. Owned by P1a-PP-06.
- No function-body envelope (the `fn $path(args) -> Ret { ... }`
  wrapping; locals declaration block; arg-locals; debug-name lines).
  Owned by P1a-PP-05a.
- No per-Instance ADT-definition rendering (Struct / Enum / Union
  body). Owned by P1a-PP-05b.
- No `render_ty` reimplementation (consumed via P1a-PP-02).
- No bridge-side translation (Place / Operand / Rvalue / etc.
  construction). Bridge-side work is in axis 3 (P1a-BR-*).

## Test Plan

Test discipline names `unit-in-src` (per-method dispatch logic;
especially `place_ty` walk for §3.4 name resolution and
`render_terminator_kind` internal helper) and `integration-in-tests`
(insta snapshots for §9.2 - §9.8 specimens). Per the brief: define
the API first with `todo!()` bodies, then write tests, then
implement.

### Unit tests (in `crates/regseq-model/src/pretty.rs`)

1. `render_place`: bare local (no projections) -> `_<N>`.
2. `render_place`: each PlaceElem variant in isolation (8 variants
   per the §7.1 wrap table).
3. `render_place`: pre/post-wrap walk preserves precedence
   (Specimen 26: `Deref` followed by `Field` produces
   `((*_3).<name>: <ty>)`).
4. `render_operand`: Copy / Move ignore `expected_ty`; Constant
   threads it.
5. `render_rvalue`: each of the 9 variants dispatches per §7.3 with
   the correct per-operand expected_ty derivation.
6. `render_rvalue` Cast: §7.3.1 annotation fires for the 5 ambiguous
   CastKind variants; bare for the others.
7. `render_rvalue` AggregateKind: each of the 7 variants per §7.3.2
   (Array, Tuple, Struct, Enum, Union, Closure, RawPtr).
8. `render_statement`: 4 cases (Assign, SetDiscriminant, Assume,
   CopyNonOverlapping).
9. `render_terminator`: 6 TerminatorKind variants + Call (Direct +
   Indirect) + Assert.
10. `render_call_callee`: Direct + Indirect.
11. `render_switch_targets`: empty-cases edge + multi-cases.
12. `render_assert_kind`: each of the 14 variants per the §7.9 table.
13. `render_basic_block`: empty-statements (Specimen 68) +
    multi-statement (Specimen 67); verify 4-space and 8-space indent
    discipline.
14. §4 span gating per construct (default off; on with
    non-synthesized; on with synthesized = suppressed).

### Integration tests (`crates/regseq-model/tests/pretty.rs`)

Add insta snapshot tests against the §9.2 - §9.8 worked-example
specimens. One snapshot per specimen, named after the specimen.

Specimens to cover:

- §9.2 `render_place` Specimen 26 (long projection chain).
- §9.3 `render_operand` Specimens 27 - 29.
- §9.4 `render_rvalue` Specimens 30 - 47c (covers Use, Repeat, Ref
  shared / unique, RawPtr const / mut, Cast unambiguous + ambiguous,
  BinaryOp, UnaryOp, Discriminant, Aggregate Array / Tuple / Struct
  / Enum tuple / Enum C-style / Union / Closure / RawPtr thin / RawPtr
  fat).
- §9.5 `render_statement` Specimens 48 - 51 (Assign,
  SetDiscriminant, Intrinsic Assume, Intrinsic CopyNonOverlapping).
- §9.6 `render_terminator` Specimens 52 - 58 (Goto, SwitchInt, Return,
  Unreachable, Call Direct, Call Indirect, Assert) + Specimens 58a -
  58c (`render_call_callee` Direct / Indirect; `render_switch_targets`
  standalone).
- §9.7 `render_assert_kind` Specimens 59 - 66b (BoundsCheck, Overflow
  Add / Shl, OverflowNeg, DivisionByZero, RemainderByZero,
  MisalignedPointerDereference, NullPointerDereference,
  InvalidEnumConstruction).
- §9.8 `render_basic_block` Specimens 67, 68.

Add an additional integration test exercising the §4 body-level span
gating across `render_statement` and `render_terminator` (default
off / on / suppressed-when-synthesized).

Add `insta` to `crates/regseq-model/Cargo.toml` `[dev-dependencies]`
if not already present.

### Test-discipline tags

- `unit-in-src` -- per-method dispatch logic and the
  `render_terminator_kind` internal helper.
- `integration-in-tests` -- insta snapshots for §9.2 - §9.8
  specimens and §4 body-level span gating.

## Review

After tests pass, request a Codex review using `codex_runner.py`:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-model-88 (P1a-PP-03) against its
task spec at .harness/tasks/regseq-model/88-render-body-level.md. ALL
CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-model/src/pretty.rs
- crates/regseq-model/tests/pretty.rs
- (any new insta snapshot files under crates/regseq-model/tests/snapshots/)

Focus on:
- Spec deviations: every rule in §7.1 - §7.10 (per-arm tables;
  CastKind §7.3.1; AggregateKind §7.3.2; AssertKind §7.9 templates).
- Pair-form D-14-4.9 signatures on `render_operand` and
  `render_rvalue` are present and threaded correctly.
- Per-operand `expected_ty` derivation per the §10.1.8 caller table
  (rendered correctly through Statement Assign, Rvalue dispatch,
  Call args, Assert message operands, Indirect callee).
- §3.4 name resolution: field names + variant names resolve via the
  `place_ty` walk through `Layout.kind`; non-optional name fields
  per the schema.
- §4 body-level span gating: Statement.span, Terminator.span,
  LocalDecl.span (when render_locals_decl-style helper exposed);
  §4.3 synthesized-span suppression.
- §3.1 envelope discipline: 4-space indent for bb<N>: { and }; 8-space
  for inner statements / terminator. Trailing `;` after terminator is
  appended by `render_basic_block`, not `render_terminator`.
- Test coverage vs spec Test Plan: every §9.2 - §9.8 specimen has a
  matching insta snapshot.
Do NOT flag anything explicitly marked out of scope (Non-Goals;
later task IDs P1a-PP-04a/b through P1a-PP-06; render_const stub
behavior).
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

- All 9 public renderers + 1 internal helper (`render_place`,
  `render_operand` pair-form, `render_rvalue` pair-form,
  `render_statement`, `render_terminator`, `render_terminator_kind`
  internal, `render_call_callee`, `render_switch_targets`,
  `render_assert_kind`, `render_basic_block`) implemented per
  pretty-printer.md §7.1 - §7.10 and the §8 ownership table.
- §3.4 field / variant name resolution via the `place_ty` walk and
  `Layout.kind` dispatch is functional.
- §4 body-level span gating (Statement, Terminator, LocalDecl per
  the brief's per-task scope) implemented; §4.3 synthesized-span
  suppression honored.
- All §9.2 - §9.8 worked-example specimens (Specimens 26 through 68)
  have matching insta snapshot tests; outputs match verbatim.
- Unit tests cover per-arm dispatch shape, pre/post-wrap walk for
  Place projections, AggregateKind 7-variant dispatch, AssertKind
  14-variant dispatch, and §4 span gating.
- `cargo test -p regseq-model`, `cargo fmt --all -- --check`, and
  `cargo clippy -p regseq-model -- -D warnings` all pass.
- Codex Task Review verdict APPROVED.
