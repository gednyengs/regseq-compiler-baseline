# regseq-backends-13: Generic-C enum tag-ops — §13 union-of-views navigation (Phase-1 task d2.1)

## Description

Replace the **enum tag-op** `EmitError::Unsupported("…task d2.1")` seams the d2
body emitter left in `body_emit.rs` with real §13 lowering over the c2
**union-of-views** enum representation (`enum_emit.rs`): the §4 `Downcast(v)` →
`Field(i)` payload-field place projection, `Rvalue::Discriminant` on an **enum**
predecessor, `StatementKind::SetDiscriminant`, `Rvalue::Aggregate(AggregateKind::Enum,
..)` construction, and **single-member** `Const::EnumValue`. This is **task d2.1** of
the Phase-1 a→b→c1→c2→d1→d2→**d2.1**→d2.2→e sequence — the enum half of the original
"(d) bodies", split from d2 (user-approved 2026-05-27, mirroring c→c1/c2 and d→d1/d2).

d2.1 is the **layout-invariant-touching** set: every op navigates the §13/D9
union-of-views layout the c2 task built, so a navigation that disagrees with the
emitted struct (a wrong union member, a wrong field name, a wrong tag offset) is a
silent miscompile that the green compile-and-run fixtures can still pass. Per the
[[assessment-soundness-pass]] discipline (which already caught two holes the green d1
*and* d2 fixtures missed), **d2.1's close gets a dedicated Codex soundness pass** in
addition to the spec review.

The **central correctness lever** (the d2 `field_c_name` lesson, [[regseq-model-test-surfaces]]
generalised): the body-side accessors must spell **the exact same** C member path the
`enum_emit` definition emits — the same inline-vs-`union u` collapse decision, the same
`_tag` / `_v<idx>` union member names, the same `view_field_name` payload field names.
A mismatch is a C compile error (the `offsetof` guard / the `.member` access disagree)
in the lucky case and a wrong-bytes miscompile in the unlucky one. So d2.1 **factors the
view-list construction + the inline decision + the naming out of `enum_emit.rs` into one
`pub(super)` source of truth**, consumed by **both** `emit_enum` (the definition) and the
new body-side accessors (the use) — they cannot drift.

### EnumValue scope (user-approved 2026-05-27)

`Const::EnumValue` is an enum-typed **value const** (the bridge emits it via the valtree
walk, `const_.rs:91-103`: `EnumValue { variant_idx, payload }`, `payload` = the variant's
fields as const children). Unlike the runtime ops it is an **operand** (nests anywhere),
so it needs an **expression** form, and the union-of-views lets a single C compound
literal designate **at most one** `union` member. The user-approved split:

- **d2.1 lowers an `EnumValue` whose construction designates a single nameable member
  group**: (a) an **inline** enum (the §13 ≤1-view collapse) constructing a variant **written
  entirely through directly-designatable inline fields** — a fieldless-collapsed `Direct` tag,
  a `Single{scalar:None}` payload, or a **niche niched variant whose niche cell coincides
  exactly with one inline scalar field** (`field.offset == TagScalar.offset` **and** the
  field's type is `TagScalar.scalar_ty` — e.g. `Option<&T>` / `Option<NonZeroU32>`, where the
  niche cell *is* the single pointer / integer field); or (b) a **fieldless variant** of a
  multi-view `Direct` enum (writes only the `_tag` member: `(struct E){ .u._tag = { .tag =
  <tag_value> } }`).
- **d2.2 owns the rest**: a `Const::EnumValue` requiring **two** members (a payload-bearing
  `Direct` variant in a multi-view enum: tag member **and** `_v<v>` member); a niche-cell
  write in a multi-view enum; **or an inline niche niched variant whose niche cell does NOT
  coincide with one directly-designatable inline scalar field** (e.g. the niche is a sub-byte
  of a nested aggregate field, `Option<(u8, NonZeroU8)>` — the cell is not a top-level field)
  → stays fail-loud, relabelled **`Const::EnumValue` (multi-member): task d2.2**.
  (d2.2 reuses d2.1's navigator + its `Aggregate::Enum` constructor to build these.) A
  payload child that is itself a non-scalar const (`Bytes` / `Aggregate` / a deferred
  `EnumValue`) **transitively** defers — it fails loud at its own d2.2 arm when `const_expr`
  recurses into it (the d2 reference-carrier coupling, generalised).

After d2.1, `emit_generic_c` emits a complete, host-compilable-and-runnable TU for any
model whose bodies use the d1 + d2 subset **plus** enum match / construct / discriminant
over the **inhabited** §13 shapes (`Single{scalar:None|Some}` / `Multiple{Direct|Niche}`;
`Empty` is a zero-variant **uninhabited** enum — `Discriminant`/`Downcast` on it is
statically unreachable, never a runtime value, so it gets structural/unreachable coverage
only) —
everything except multi-member enum **consts** (d2.2), non-scalar consts (d2.2), and `dyn`
(the vtable task). Verification continues the d1/d2 pattern: bridge an enum-matching /
enum-constructing Rust function, emit the TU, compile **and run** a `void regseq_main(void)`
that matches/builds an enum value and `abort()`s on a wrong result — a clean exit (code 0)
proves the tag read/write + payload access reproduced rustc's discriminant semantics (the
§13 `_Static_assert`s additionally guard the layout at compile time). The first
*coverage-ledger* green fixture is still **task e**.

### Scope boundaries (flagged for review — see § Review)

1. **§13 enum tag-ops over the c2 union-of-views.** d2.1 lowers: `Downcast(v)` +
   the `Field(i, ty)` the I-Downcast-followed-by-Field invariant guarantees follows it
   (`place.rs:61-63`) → the variant payload field at its absolute offset; `Discriminant(place)`
   on an **enum** predecessor (`Single` → the constant discriminant; `Multiple{Direct}` →
   tag-cell read mapped to the matched variant's `discriminant`; `Multiple{Niche}` →
   niche-cell read with the `untagged_variant` fallback); `SetDiscriminant(place, v)`
   (`Direct`/`Single{scalar:Some}` → write `tag_value`; `Niche` niched → write the niche
   cell; `Niche` untagged / `Single{scalar:None}` → no-op); `Aggregate::Enum(ty, v)`
   construction (tag write + payload field writes, at the assign site); single-member
   `Const::EnumValue` (above). **No const work beyond d1 scalars + the single-member
   `EnumValue` literal.**
2. **The navigator-agreement requirement.** The body-side member paths (`.u._v<v>.<name>`
   / inline `.<name>`; `.u._tag.tag` / inline `.tag`) and the tag-value / discriminant
   constants come from the **shared** `enum_emit` view machinery, not a re-derivation —
   so they match the emitted definition + its `offsetof` guards exactly (boundary, Change A).
3. **The niche cell has no `_tag` view.** A `Niche` enum carries no `_tag` view
   (`enum_emit::tag_view_scalar` returns `None`, `enum_emit.rs:178-190`): the niche cell is
   a byte region at `TagScalar.offset` aliased inside a variant view. d2.1 reads / writes it
   via the §14.7 **byte-storage technique** — `memcpy` to / from a typed temporary at the
   absolute offset — **not** a typed-pointer cast (the pinned `cc` flags are
   `-std=gnu11 -fwrapv -Wall`, `toolchain.rs:84`, with **no** `-fno-strict-aliasing`, so a
   reinterpreting pointer load is UB; `memcpy` is the codebase's established aliasing-safe
   idiom — `Transmute`/`CopyNonOverlapping`/array-assign all use it). A niche `Discriminant`
   read therefore lowers at the **assign site** (memcpy the niche cell into a temp, then the
   ternary), mirroring the existing array / `*WithOverflow` / `Transmute` assign-site cases —
   **not** a GNU statement-expression.
4. **Deferred to d2.2 (stays fail-loud).** A multi-member `Const::EnumValue` (a
   payload-bearing `Direct` variant in a multi-view enum, or a niche-cell write in a
   multi-view enum) → `EmitError::Unsupported("Const::EnumValue (multi-member): task d2.2")`;
   all `Const::Bytes` / `Const::Aggregate` arms are **unchanged** (still d2.2). The
   `Const::Reloc` path is unchanged (d1).
5. **Deferred to the vtable task (stays fail-loud, unchanged).** No `dyn` / vtable / virtual
   dispatch work. (Enums do not interact with `dyn` here.)

**Payload access is direct — no §14.7 `memcpy` for payload fields.** The union-of-views
emits each variant payload as a **full-object view** whose fields sit at their absolute,
**naturally-aligned** offsets (no view needs packing, `enum_emit.rs:14-21`). So a
`Downcast`+`Field` payload access is a plain member access `<lv>.u._v<v>.<name>` (or inline)
— the §14.7 `memcpy`-through-byte-storage caveat applies **only** to the niche cell
(boundary 3), which has no view. This is the whole soundness payoff of the union-of-views
over a naive packed `{ tag; union; }`; d2.1 must not reintroduce a byte-storage access for
payload fields.

## Input Context

### Design source — `docs/discussions/regseq-model/generic-c-backend.md`

- **§13 Enum C representation** (lines 939–1005): the union-of-views form + the four **Tag
  operations** bullets (lines 982–1002) — the normative lowering d2.1 implements:
  - `Discriminant(place)`: `Empty` unreachable; `Single` → the single variant's
    `VariantLayout.discriminant` (a constant — §13 line 984's "the single variant index"
    phrasing means the index-0 variant; its **returned value** is its `discriminant`, which is
    `5` for `enum Only { A = 5 }`, not the index `0`, per `docs/discussions/regseq-model/spec.md:3957`);
    `Multiple` (Direct **and** Niche) → read the tag / niche cell at
    `TagScalar.offset` and **compare against each variant's precomputed `VariantLayout.tag_value`**
    (the match selects the variant); for `Niche`, niched variants carry `Some(tag_value)`,
    the untagged variant `None`, **no match → `untagged_variant`**. Using the
    bridge-precomputed `tag_value` (already mod-2^width + wrapping-encoded) **avoids
    re-deriving the niche formula**.
  - `SetDiscriminant(place, v)`: `Direct`/`Single{scalar:Some}` → store
    `VariantLayout[v].tag_value` (`Some` by construction) into the tag cell; `Niche` niched →
    store its `tag_value`; `Niche` untagged → **no-op** (`tag_value: None`); `Single{scalar:None}`
    → no-op.
  - `Downcast(v)` then `Field(i, _)` → access variant `v`'s payload field `i` at its
    `FieldLayout.offset` (§14 access).
  - `Aggregate::Enum(ty, v)` / `Const::EnumValue` → write the active variant's payload
    fields at their offsets + the tag write (as `SetDiscriminant(v)`). **Niche enums carry no
    tag view — the niche cell is a payload field in a variant view whose bit pattern is the
    tag** (line 969).
- **§13 result value** — the §11.1 table (`spec.md:3957-3961`): `discriminant` is **what
  `Discriminant(place)` returns** (the Rust-level semantic discriminant); `tag_value` is
  **what `SetDiscriminant` writes** (the layout-level tag bytes). They are **equal** for
  `Direct`, **differ** for `Niche` and explicit-discriminant enums — so the `Discriminant`
  read maps tag/niche-cell `== tag_value` to the matched variant's `discriminant`, it does
  **not** return the raw cell. Signed-discriminant rendering per `spec.md:3974-3986`
  (`TagScalar.scalar_ty` decides signed vs unsigned literal emission).
- **§14 Aggregate field-offset reproduction** (lines 1009–1039): step 7 — "direct member
  access for the natural case; `memcpy` to / from a typed temporary **when accessing through
  enum byte storage** or a packed / misaligned field". d2.1 maps "enum byte storage" to the
  **niche cell only** (boundary 3); payload views are the natural case (direct access).
- **§12 `Const`** (lines 915–935): unchanged except the `EnumValue` arm — d1's scalar
  `Const` stands; `Bytes` / `Aggregate` stay d2.2.

### Code anchors (confirm line numbers at HEAD)

- `crates/regseq-backends/src/generic_c/enum_emit.rs` — the c2 union-of-views **definition**;
  d2.1 factors its view machinery into a shared source of truth (Change A):
  - `build_views` (150–172) — the ordered view list (`_tag` view then `_v<idx>` per variant
    with ≥1 non-ZST field). **Currently private + returns a private `View{member, fields}`.**
  - the inline-vs-`union u` decision (`emit_enum`, 75–133): `[]` → size-faithful struct;
    `[view]` → **inline, no `union`, member_prefix `""`**; `_` (≥2) → `union { … } u;`, member
    prefix `u.<member>.`. **This collapse decision is the load-bearing thing the body
    accessors must reproduce** — factor it.
  - `tag_view_scalar` (178–190) — the `_tag` view presence (`Single{scalar:Some}` /
    `Multiple{Direct}` → `Some`; `Niche` / `Single{scalar:None}` / `Empty` → `None`).
  - `view_field_name` (227–234) — the payload field C name (struct-style `sanitize`,
    tuple-style `_<idx>`); **distinct from `layout_emit::field_c_name_ty`** (which returns
    `None` for an enum). Payload-field access must use **this**.
  - `shape_variants` (194–200), `variant_view_fields` (206–219).
- `crates/regseq-backends/src/generic_c/body_emit.rs` — the d2 emitter d2.1 fills:
  - `place_walk` (640–755) — the single shared lvalue+type walk d2 built. The `Field` arm
    (679–689) currently routes through `layout_emit::field_c_name_ty` (non-enum only); the
    `Downcast` arm (742–746) errors `task d2.1`. Change B: a `Downcast(v)` records a pending
    variant; the following `Field` arm, when a downcast is pending, spells the variant view
    member path (Change A navigator) instead.
  - `place_ty` (171) / `PlaceWalk` (141) — the walk used by `Discriminant` (824) + `assign`
    (329); Change B threads the pending-variant state.
  - `statement` (309–317) — `SetDiscriminant` seam (312–313, Change D).
  - `assign` (326–382) — the assign-site dispatch; the array (348) / struct-with-array (351) /
    `Repeat` (359) / `*WithOverflow` (362) special cases are the precedent for `Aggregate::Enum`
    (Change D) and the niche-`Discriminant` (Change C) assign-site forms.
  - `rvalue_expr` (809–833) — the `Discriminant` seam (821–830, Change C); `Aggregate` (831)
    dispatches to `aggregate_expr`.
  - `aggregate_expr` (1008–1025) — the `AggregateKind::Enum` arm (1023, Change D: stays a
    defensive error; the real lowering is at the assign site, mirroring `Array` at 1020).
  - `const_expr` (1642–1672) — the `EnumValue` arm (1670, Change E): single-member literal
    or relabel multi-member → d2.2; `Bytes` (1668) / `Aggregate` (1669) **unchanged**.
  - `fresh_temp_name` (396–411) — the deterministic collision-free temp for the niche-read
    temp + any construction temp; reuse unchanged.
  - `assign_with_overflow` (558) / `assign_struct_fields` (431) / `assign_array` (486) —
    multi-statement assign-site precedents (`memcpy` for array fields; `&(T){v}` compound
    literal for a temp address at 782) Change C/D mirror.
- IR surface (read-only — trust as bridge certificates):
  - `crates/regseq-model/src/layout.rs` — `EnumShape::{Empty, Single{variant, scalar},
    Multiple{tag, encoding, variants}}` (87–108); `VariantLayout{name, fields, uninhabited,
    discriminant, tag_value}` (115–136) — `discriminant` = Discriminant's result,
    `tag_value: Option<u128>` = SetDiscriminant's write (`None` for the niche untagged variant
    + `Single{scalar:None}`); `TagScalar{offset, scalar_ty, valid_range}` (146–157);
    `TagEncoding::{Direct, Niche{untagged_variant, niche_variants, niche_start}}` (164–183);
    `FieldLayout.offset` for variant payload fields is **absolute to the whole enum object**
    (53–68, `spec.md:3915-3921`).
  - `crates/regseq-model/src/place.rs` — `PlaceElem` (`:70`), `Field(FieldIdx, TyId)` (`:84`,
    the carried post-narrow field type), `Downcast(VariantIdx)` (`:130`); the
    **I-Downcast-followed-by-Field** invariant (`:61-63`): a `Downcast` is never last and is
    always immediately followed by a `Field` — so the walk's `Downcast` arm can assume the next
    elem is the payload `Field`, and a bare `Downcast`-as-last is malformed.
  - `crates/regseq-model/src/rvalue.rs` — `Rvalue::{Discriminant(Place), Aggregate(AggregateKind,
    Box<[Operand]>)}`; `AggregateKind::Enum(TyId, VariantIdx)` (`rvalue.rs:205`);
    `StatementKind::SetDiscriminant(Place, VariantIdx)` (`statement.rs:47`).
  - `crates/regseq-model/src/const_value.rs` — `Const::EnumValue{variant_idx: VariantIdx,
    payload: Vec<ConstId>}` (40–45); `payload` children resolved via `model.consts.resolve`.
  - `spec.md` §11.1/§11.2 (3932–4062) — `tag_value` semantics table + the **niche tag-value
    formula** (the bridge already applies it; the backend reads `tag_value`, **never**
    re-derives); §21 Rvalue (`Discriminant(Place)` variant `spec.md:7244`, §21.2.8 semantics
    `spec.md:7407-7414` — non-enum predecessor yields `0`), §22 Statement (`SetDiscriminant`).
  - `crates/regseq-driver/src/bridge/const_.rs:91-103` — where `Const::EnumValue` is produced
    (grounds the EnumValue scope: `payload` is the variant's fields, scalar or nested const).

### Prior tasks

- `regseq-backends-08` (c2, the union-of-views `enum_emit.rs`) — d2.1 **navigates** what c2
  emits; Change A factors c2's view machinery into the shared source of truth. c2's scope
  note (`enum_emit.rs:26-29`) already names the tag operations as "task d" — d2.1 is that work.
- `regseq-backends-12` (d2, `19f5b25`) — the `place_walk` single shared walk, the assign-site
  special-case pattern (`assign_array`/`assign_repeat`/`assign_with_overflow`), `field_c_name_ty`,
  `fresh_temp_name`, the `&(T){v}` temp-address idiom — all reused. d2 deferred every enum
  tag-op to d2.1 with distinct `task d2.1` labels; d2.1 replaces exactly those arms.
- d1 (`-10`/`-11`) — the scalar `Const` rendering (`render_int`/`render_uint` + the §11.1
  signed rendering) the `Discriminant` result + `tag_value` comparisons reuse.

### Test surface

- `crates/codegen-coverage-runner/tests/generic_c_bodies.rs` — d1/d2's behavioral harness
  (`try_bridge_fixture_path`; `toolchain::compile_and_run(tu, "regseq_main", cc)` → assert
  `exit_code == Some(0)`; the hand-built-model pattern). d2.1 adds enum match / construct
  fixtures + hand-built models here.
- `crates/regseq-backends/src/generic_c/mod_tests.rs` — the in-crate hand-built structural
  tests; d2.1 adds Downcast/Field, Discriminant (Direct + Niche), SetDiscriminant,
  Aggregate::Enum, single-member EnumValue structural cases + the negative (multi-member
  EnumValue → `Unsupported` d2.2) cases. [[regseq-model-test-surfaces]]: the model's
  validator tests are out of scope.

## Implementation Notes

### Change A: the shared enum view/tag navigator (`enum_emit.rs`)

Factor `enum_emit.rs`'s view-list construction + inline decision + naming into one
`pub(super)` source of truth, consumed by **both** `emit_enum` (refactored to call it — so
the definition is proven to use the same paths) and the new body-side accessors. The refactor
is **output-preserving** (every c2 enum_emit test stays byte-identical). Expose enough for the
body side to spell, for an enum `TyId`:

- **the inline flag** — `views.len() <= 1` → inline (no `union u`, member designator prefix
  `""`); `>= 2` → `union { … } u`, prefix `u.<member>.`; `== 0` → empty/size-faithful struct
  (an `Empty` enum — `Discriminant`/`Downcast` on it is statically unreachable).
- **the variant payload-field member path** — `enum_variant_field(model, names, enum_ty,
  variant_idx, field_idx) -> Option<(member_path, field_ty)>` where `member_path` is
  `u._v<variant>.<view_field_name>` (multi-view) or `<view_field_name>` (inline). `None` if
  the field is ZST (omitted from the view) or the variant has no view — both are caller-handled
  signals, not silent.
- **the tag cell** — `enum_tag(shape) -> Tag`: `Direct{member_path, scalar_ty}` where
  `member_path` is `u._tag.tag` / inline `tag`; `Niche{offset, scalar_ty}` (no view — byte
  storage); `None{}` (`Single{scalar:None}`).
- **per-variant constants** — `tag_value: Option<u128>`, `discriminant: u128`, and the
  encoding (`Direct` vs `Niche{untagged_variant, …}`), straight from `VariantLayout` /
  `TagEncoding` (no re-derivation).

Keep the navigator a thin read of the existing structs (`build_views`, `tag_view_scalar`,
`view_field_name`, `VariantLayout`); do **not** recompute any offset, tag value, or name.

### Change B: `Downcast` + `Field` place projection (`body_emit.rs` `place_walk`)

Thread a `pending_variant: Option<VariantIdx>` through `place_walk`:

- **`Downcast(v)`** → record `pending_variant = Some(v)`; the running `ty` stays the enum
  `Ty` (the §19.4 narrowing is realised by the following `Field`'s carried `TyId`). By
  I-Downcast-followed-by-Field the next elem **is** a `Field`; a `Downcast`-as-last is
  malformed (defensive `Unsupported`).
- **`Field(idx, field_ty)`** → if `pending_variant` is set, spell the variant payload field
  via Change A's `enum_variant_field(enum_ty, v, idx)` (→ `<lv>.u._v<v>.<name>` / inline
  `<lv>.<name>`); narrow `ty = field_ty`; clear `pending_variant`. Otherwise the existing
  non-enum path (`field_c_name_ty`) is unchanged. A ZST payload field is omitted from the
  view — its lvalue is never materialised (the d2 ZST-elision rule already drops ZST
  assignments), but reaching `enum_variant_field` for a ZST field returns `None`; treat it
  consistently with the existing ZST handling (the field's value is never read/written).

Payload field access is **direct** (boundary, "Payload access is direct"): the view fields
are naturally aligned, so **no** §14.7 `memcpy`. (The packed-field §14.7 concern from d2 does
not apply — enum views are never packed, `enum_emit.rs:14-21`.)

### Change C: `Discriminant` on an enum (`body_emit.rs`)

Replace the `rvalue_expr` `Discriminant`-on-enum error (825). The result is the matched
variant's **`discriminant`** (not the raw cell, not `tag_value`):

- **`Single{..}`** → the single variant's `discriminant` (a constant; rendered per the result
  type). **`Empty`** → statically unreachable; emit the §16 unreachable or a defensive `0`
  (flag the choice — no inhabited value reaches here).
- **`Multiple{Direct}`** → a ternary chain over the `_tag` view read: `(<tag-read> == <tv0>
  ? <d0> : <tag-read> == <tv1> ? <d1> : … : <d_last>)`, `tvN` = `VariantLayout[N].tag_value`
  rendered per `TagScalar.scalar_ty` signedness, `dN` = `discriminant` rendered per the
  result type. A **pure expression** (`<tag-read>` = the `_tag` member access). The trailing
  `: <d_last>` covers the last variant (all `Direct` variants have `tag_value: Some`).
- **`Multiple{Niche}`** → the niche cell has no `_tag` view (boundary 3) → lower **at the
  assign site** (peek in `assign`, like the array / overflow cases): emit `memcpy(&<tmp>,
  (const unsigned char *)&(<enum_lv>) + <TagScalar.offset>, sizeof(<tmp>));` with `<tmp>` of
  `scalar_ty`'s C type (`fresh_temp_name`), then `dst = (<tmp> == <tv_a> ? <d_a> : … :
  <d_untagged>);` — the ternary covers the **niched** variants (`tag_value: Some`) and falls
  through to the `untagged_variant`'s `discriminant` (the §13 "no match → untagged_variant"
  rule). Determinism: variant order + `fresh_temp_name`.

**Tag-scalar rendering — every admitted `TagScalar.scalar_ty`, not just integers.** `scalar_ty`
admits `Bool`, `Char`, `Int(_)`, `Uint(_)`, `Ptr(_, _)`, `Ref(_, _)` (`docs/discussions/regseq-model/spec.md:4025-4034`)
— niche enums commonly carry a **pointer/reference** tag (`Option<&T>`: the tag *is* the
pointer, `tag_value` `0` = null). So the niche-cell temp's C type is `types::c_type(scalar_ty)`,
and a tag-value literal is rendered by a **tag-scalar helper covering all six**, reusing the d1
scalar `Const` renderers keyed on `scalar_ty`: `Int(_)`/`Uint(_)` → `render_int`/`render_uint`
(signed vs unsigned per §11.1, `docs/discussions/regseq-model/spec.md:3974-3986`); `Bool` →
the d1 `Const::Bool` form; `Char` → `(uint32_t)…U`; `Ptr`/`Ref` → the d1 null/pointer literal
cast to `c_type(scalar_ty)` (e.g. `((const T *)<value>)`). The `discriminant` result is rendered
per the **result** integer type. **Flag the per-`scalar_ty` rendering (esp. pointer/ref tags +
signedness) for review.**

### Change D: `SetDiscriminant` + `Aggregate::Enum` construction (`body_emit.rs`)

**`SetDiscriminant(place, v)`** (`statement`, 312) → resolve the enum lvalue (`place_walk`),
then per Change A's tag descriptor + `VariantLayout[v].tag_value`:

- **`Direct` / `Single{scalar:Some}`** (`tag_value: Some(tv)`) → `<enum_lv>.u._tag.tag = <tv>;`
  (or inline `<enum_lv>.tag = <tv>;`), `<tv>` rendered per `scalar_ty`.
- **`Niche` niched** (`tag_value: Some(tv)`) → write the niche cell via §14.7 byte storage:
  `memcpy((unsigned char *)&(<enum_lv>) + <TagScalar.offset>, &(<scalar_c>){<tv>}, sizeof(<scalar_c>));`
  (the `&(T){v}` compound-literal temp-address idiom, `body_emit.rs:782`).
- **`Niche` untagged** (`tag_value: None`) / **`Single{scalar:None}`** → **no-op** →
  `Ok(None)` (the statement is elided; the payload is the tag / there are no tag bytes).

**`Aggregate::Enum(ty, v)`** — always an `Assign` RHS → a new assign-site helper
`assign_enum(dst, ty, v, ops)` (the `aggregate_expr` `Enum` arm at 1023 stays a defensive
error, mirroring `Array` at 1020). Emit, as a `;`-joined statement sequence:

1. **The tag write** (Change D `SetDiscriminant` logic for `v`, if any) — `Direct`/scalar:Some
   → the `_tag` write; `Niche` niched → the niche-cell `memcpy`; untagged / scalar:None → none.
2. **Each non-ZST payload field** at its view member: `<dst>.u._v<v>.<name> = <op>;` (or inline;
   array-typed field → `memcpy(&<dst>…, <src>, sizeof …)` per the d2 array rule, `assign_struct_fields`
   precedent). ZST operands skipped.

Tag and payload occupy **disjoint** byte ranges (the union-of-views tag prefix vs the variant
payload), aliased through different `union` members — the sequential writes are sound (the §13
"union aliasing reproduces the exact byte layout" assumption the representation already relies
on); tag-first for determinism. Padding is left as-is (matches rustc's undef padding). A
fieldless `Direct` variant → tag write only; a `Niche` untagged variant → payload writes only;
a `Niche` niched fieldless variant (e.g. `None`) → the niche-cell write only.

### Change E: single-member `Const::EnumValue` (`body_emit.rs` `const_expr`)

Replace the `const_expr` `EnumValue` error (1670). Lower to a single C compound literal **iff**
the construction designates one nameable member group (Change A); otherwise relabel to d2.2:

- **inline enum** (≤1 view) → `(struct <E>){ <designators over the inline view's fields> }`:
  for a fieldless-collapsed `Direct` enum the field is `tag` = `tag_value`; for a
  `Single{scalar:None}` / niche-**untagged** payload the fields are the payload children
  (`const_expr` recursion, ZST omitted).
- **inline niche, niched variant** → lower **only if** the niche cell coincides exactly with
  one inline scalar field — a field `F` of the inline view with `F.offset == TagScalar.offset`
  **and** `F.ty == TagScalar.scalar_ty` (e.g. `Option<&T>` / `Option<NonZeroU32>`): then
  `(struct <E>){ .<F> = <tag_value rendered as scalar_ty> }`. If **no** such single coinciding
  scalar field exists (the niche is a sub-byte of a nested aggregate field, e.g.
  `Option<(u8, NonZeroU8)>`) → **d2.2** (the cell is not directly designatable in a constant).
- **multi-view, fieldless `Direct` variant** → `(struct <E>){ .u._tag = { .tag = <tag_value> } }`.
- **otherwise** (a payload-bearing `Direct` variant in a multi-view enum → two members; **any**
  niche-cell write in a multi-view enum) → `EmitError::Unsupported("Const::EnumValue (multi-member): task d2.2")`.

Each payload child lowers via the **existing** `const_expr` recursion: a scalar child lowers
(d1); a non-scalar child (`Bytes` / `Aggregate` / a deferred `EnumValue`) **fails loud at its
own d2.2 arm** (transitive coupling — d2.1 adds no new const lowering beyond the single-member
`EnumValue` envelope + d1 scalars). `Const::Bytes` / `Const::Aggregate` arms **unchanged**.

### Out of scope (deferred seams that **stay** `EmitError::Unsupported`)

d2.2 (non-scalar const): all `Const::Bytes`, all `Const::Aggregate` (incl. the reference
carrier), `Bytes`-init static defs, **and** the multi-member `Const::EnumValue` cases (a
distinct `…(multi-member): task d2.2` label). vtable task: every `dyn` / vtable arm
(unchanged). Each keeps a distinct static label so each later task replaces only its own arms.

## Non-Goals

- **No enum-layout recomputation** — read `EnumShape` / `VariantLayout.tag_value` /
  `.discriminant` / `TagScalar.{offset, scalar_ty}` / `TagEncoding` as bridge certificates;
  **never** re-derive the niche formula (`spec.md:4036-4062` — the bridge already applied it).
- **No re-derivation of enum C member paths / field names** — the body accessors call Change
  A's shared navigator, which reads `enum_emit`'s own `build_views` / inline decision /
  `view_field_name`. A re-implementation that drifts from `emit_enum` is the failure mode.
- **No §14.7 `memcpy` for payload fields** — payload views are naturally aligned; direct
  member access. `memcpy` is used **only** for the niche cell (no view) + array-typed payload
  fields (the d2 array rule).
- **No GNU statement-expression** — the niche `Discriminant` read uses the assign-site
  memcpy-temp form (boundary 3); enum construction uses assign-site statements; the
  single-member `EnumValue` const uses a plain compound literal.
- **No multi-member `Const::EnumValue`** (d2.2), **no other non-scalar const** (`Bytes` /
  `Aggregate` arms untouched), **no `dyn` / vtable** (untouched).
- **No type-definition / discovery / topo-sort / naming change** — `enum_emit`'s **output**
  is byte-identical after Change A's factoring (a refactor, not a re-emit); d2.1 adds no
  discovery root, no ordering edge, no new aggregate.
- **No new `EmitError` variant** — reuse `Unsupported(&'static str)`; d2.1 removes the `task
  d2.1` arms and adds the `…(multi-member): task d2.2` label.
- **No prelude / helper / spec / Doc change** — §13 is a backend lowering of an existing spec;
  no canonical-doc edit. **No harness change** — verify through the existing `void
  regseq_main(void)` contract.
- **No deps; no bridge / model / matrix / ledger / `test-ir-fixtures` change.** d2.1 is
  confined to `crates/regseq-backends/src/generic_c/{body_emit.rs, enum_emit.rs}` + new
  behavioral / structural tests + the `tasks.json` registration.

## Test Plan

### B1: build + workspace green
- `cargo build -p regseq-backends -p codegen-coverage-runner` clean.
- `cargo test -p regseq-backends -p codegen-coverage-runner -- --test-threads=1` green —
  **all c2 / d1 / d2 tests still pass** (Change A is an output-preserving refactor of
  `enum_emit`; the emitted enum structs are byte-identical), plus the new d2.1 tests.

### B2: end-to-end enum behavior (compile-**and-run**)
- **`Direct` multi-variant match + construct.** A `#[no_mangle] pub extern "C" fn regseq_main()`
  that builds `enum E { A, B(u32), C { x: u8, y: u16 } }` values, matches on them (`match` →
  `Discriminant` + `Downcast` + `Field`), and `panic!()`s on a wrong discriminant or payload —
  exits 0 iff the tag read + payload access agree with rustc. Cover a **fieldless** variant
  (tag-only) and **payload-bearing** variants (tag + `_v<v>` payload).
- **`Niche` enum match + construct.** `Option<&u32>` (or `Option<NonZeroU32>`): construct
  `Some`/`None`, match (niche `Discriminant` → the assign-site memcpy-temp path), read the
  payload — `panic!()` on a wrong result. Proves the niche cell read/write + the
  `untagged_variant` fallback. (Use a **runtime** reference for `Some(&x)` of a local, not a
  literal const — a reference const is the d2.2 carrier.)
- **`Single{scalar:None}` / single-field collapse.** A single-variant enum with payload →
  inline struct; construct + read back.
- **`repr(int)` fieldless enum** (`Single{scalar:Some}` and `Multiple{Direct}` with explicit
  discriminants where `discriminant != ` storage index) → asserts the `Discriminant` ternary
  maps tag → the correct semantic discriminant, not the raw cell.
- Where the bridge cannot surface a shape → **hand-build the model** (the c1/d2 precedent) and
  assert the emitted TU compiles + the structural shape.

### B3: structural — navigator agreement
- The body `Downcast`+`Field` access spells **the same** member path `emit_enum` emits for the
  same model: a multi-view enum → `.u._v<v>.<name>`; an inline enum → `.<name>`; the `_tag`
  member → `.u._tag.tag` / inline `.tag`. (Emit the definition + a body that accesses it from
  the **same model**; assert the access substrings match the definition's member designators /
  `offsetof` guards.)
- `SetDiscriminant` writes `tag_value` (Direct → `_tag`; Niche → the niche-cell `memcpy`);
  untagged / `scalar:None` → elided (no statement).
- `Aggregate::Enum` of a `Direct` payload variant emits the tag write **and** the `_v<v>`
  payload writes; a niche niched variant emits only the niche-cell write.
- A single-member `EnumValue` (fieldless `Direct` variant; inline enum) emits a plain compound
  literal with the right designator.

### B4: the deferred seams fail loud
- **d2.2**: a multi-member `Const::EnumValue` (a payload-bearing `Direct` variant in a
  multi-view enum) → `EmitError::Unsupported(_)` labelled `…(multi-member): task d2.2`; **and**
  an inline niche `Const::EnumValue` whose niche cell does **not** coincide with one inline
  scalar field (e.g. `Option<(u8, NonZeroU8)>`) → same d2.2 label (the narrowed predicate,
  Change E); a `Const::Bytes` + a `Const::Aggregate` (reference carrier) still fail loud (d2.2,
  unchanged). Conversely a single-coinciding-cell inline niche const (`Option<&T>` `None`)
  **does** lower in d2.1 (a positive structural case).
- Confirm **no** `task d2.1` label remains anywhere in `body_emit.rs`.

### B5: determinism
- `emit_generic_c` byte-identical across two calls on an enum-bearing model (the ternary
  variant order, the niche-read temp name, the construction statement order all deterministic)
  ([[codegen-ledger-determinism]]).

### B6: fmt + clippy + diff scope
- `cargo fmt --all -- --check` clean; `cargo clippy -p regseq-backends -p
  codegen-coverage-runner --all-targets -- -D warnings` clean (scoped per
  [[clippy-all-targets-test-drift]]).
- Diff confined to `crates/regseq-backends/src/generic_c/{body_emit.rs, enum_emit.rs}` + the
  new runner behavioral test + the `mod_tests.rs` cases + the `tasks.json` registration. **No**
  spec / Doc / bridge / model / matrix / ledger / `test-ir-fixtures` / harness / type-definition
  / discovery change; `enum_emit`'s emitted output unchanged.

## Review

### Files to send to Codex
This spec; `docs/discussions/regseq-model/generic-c-backend.md` §13/§14/§12; `crates/regseq-backends/src/generic_c/{enum_emit.rs, body_emit.rs, layout_emit.rs, types.rs}`; the IR surface `crates/regseq-model/src/{layout.rs, place.rs, rvalue.rs, statement.rs, const_value.rs}` + `docs/discussions/regseq-model/spec.md` §11.1/§11.2 + §21/§22; the bridge `crates/regseq-driver/src/bridge/const_.rs:91-103`; `crates/codegen-coverage-runner/tests/generic_c_bodies.rs`; `crates/codegen-coverage-runner/src/toolchain.rs` (the pinned `cc` flags).

### Reviewer focus
- **Navigator agreement (Change A/B).** Does the body access spell the **exact** member path
  `emit_enum` emits — same inline-vs-`union u` collapse (`views.len() <= 1`), same `_tag` /
  `_v<idx>` member names, same `view_field_name` payload names — via **one shared** `enum_emit`
  source of truth (NOT a re-derivation)? Is `emit_enum` itself refactored onto it (output
  byte-identical)? Is the `Downcast`-pending-`Field` threading sound (I-Downcast-followed-by-Field)?
- **Discriminant (Change C).** Result = the matched variant's **`discriminant`** (not the raw
  cell, not `tag_value`)? `Direct` ternary over the `_tag` read; `Niche` at the assign site
  (memcpy the niche cell → temp → ternary with the **`untagged_variant` fallback**); `Single`
  constant; `Empty` unreachable? Signed-discriminant rendering per §11.1? **Reads `tag_value`,
  never re-derives the niche formula?**
- **SetDiscriminant + Aggregate::Enum (Change D).** Writes `VariantLayout.tag_value` to the
  right cell (Direct → `_tag` view; Niche niched → niche-cell `memcpy`; untagged / `scalar:None`
  → no-op)? Construction writes tag **and** disjoint payload fields through the views (union
  aliasing), ZST omitted, array fields via `memcpy`? Determinism?
- **Niche byte storage (boundary 3).** `memcpy` to/from a typed temp at `TagScalar.offset`,
  **not** a reinterpreting pointer cast — sound under `-std=gnu11 -fwrapv -Wall` with **no**
  `-fno-strict-aliasing`? Payload fields are **direct** access (no §14.7 memcpy)?
- **EnumValue scope (Change E).** Exactly the single-member cases lowered (inline enum —
  with a niche **niched** variant lowered **only** when its niche cell coincides exactly with
  one inline scalar field, else d2.2; multi-view fieldless `Direct` variant); multi-member →
  `…(multi-member): task d2.2`; a
  non-scalar payload child transitively defers via its own arm; `Bytes` / `Aggregate` arms
  unchanged?
- **Scope discipline (d2.1 vs d2.2 vs vtable).** Exactly the `task d2.1` arms filled; no
  `task d2.1` label left; the new d2.2 label distinct; no enum-layout / discovery /
  type-definition / spec / Doc / harness change; `enum_emit` output byte-identical?
- **Verification honesty.** Compile + run over every §13 shape (Direct fieldless + payload,
  Niche, Single, repr(int) with explicit discriminants) aborting on a wrong result + the §13
  `_Static_assert` guards — sound, free of any d2.2 const path (no literal-const enum/payload)?
  Negatives prove fail-loud? No ledger green until e? Determinism; citation drift vs HEAD;
  scope creep vs Non-Goals.

### Adversarial prompt template
```
<task>
Review .harness/tasks/regseq-backends/13-generic-c-enum-tag-ops.md — Phase-1 task d2.1 of the
generic-C backend. d2.1 is the §13 ENUM TAG-OP body lowering over the c2 union-of-views
representation (enum_emit.rs), replacing d2's EmitError::Unsupported("…task d2.1") seams in
body_emit.rs: Downcast(v)+Field(i) payload-field place access; Discriminant on an enum (Single
→ const discriminant; Multiple{Direct} → tag-read ternary mapping tag_value→discriminant;
Multiple{Niche} → niche-cell read at the ASSIGN SITE via memcpy→temp→ternary with the
untagged_variant fallback); SetDiscriminant (Direct/scalar:Some → write tag_value to the _tag
view; Niche niched → memcpy tag_value into the niche cell; Niche untagged / Single{scalar:None}
→ no-op); Aggregate::Enum construction at the assign site (tag write + disjoint payload field
writes through the views); SINGLE-MEMBER Const::EnumValue only (inline enum — a niche niched
variant only when its niche cell coincides with one inline scalar field, else d2.2; or a
fieldless Direct variant of a multi-view enum) → a plain compound literal. CENTRAL LEVER: the body
accessors must spell the EXACT member path emit_enum emits — factor enum_emit's view-list
construction + inline-vs-union collapse + view_field_name into ONE pub(super) source of truth
consumed by BOTH emit_enum AND the body accessors (output byte-identical; the d2 field_c_name
lesson). The niche cell has NO _tag view → §14.7 byte storage (memcpy to/from a typed temp at
TagScalar.offset), NOT a reinterpreting pointer cast — the pinned cc flags are
-std=gnu11 -fwrapv -Wall with NO -fno-strict-aliasing (toolchain.rs:84). Reads tag_value /
discriminant / TagScalar as bridge certificates — NEVER re-derives the niche formula. THREE
deferrals stay fail-loud: MULTI-MEMBER Const::EnumValue (payload-bearing Direct variant in a
multi-view enum, or a niche-cell write in a multi-view enum) → d2.2 (distinct label); all
Const::Bytes / Const::Aggregate → d2.2 (UNCHANGED arms); dyn/vtable → the vtable task
(unchanged). Verification = bridge an enum-matching/constructing fn → emit → compile AND RUN a
void regseq_main(void) that aborts on a wrong discriminant/payload → assert exit 0 (+ §13
_Static_assert guards); fixtures use RUNTIME values (no literal enum/reference consts). This is
a SPEC review: judge correctness, completeness, internal consistency, sound scoping, faithful
grounding. Flag wrong/unsound lowering, missing in-scope shapes, drifted citations, leaky
d2.1/d2.2/vtable seams, and especially any navigator drift between the definition and the
access.
Identify:
- Navigator agreement: body access == emit_enum's member path via ONE shared enum_emit source
  of truth (inline-vs-union collapse views.len()<=1, _tag/_v<idx> names, view_field_name); is
  emit_enum refactored onto it with byte-identical output; Downcast-pending-Field threading
  sound (I-Downcast-followed-by-Field).
- Discriminant: result = matched variant's DISCRIMINANT (not raw cell / not tag_value); Direct
  ternary over _tag read; Niche at the assign site (memcpy niche cell→temp→ternary, untagged
  fallback); Single const; Empty unreachable; signed rendering per §11.1; reads tag_value,
  never re-derives the niche formula.
- SetDiscriminant + Aggregate::Enum: writes tag_value to the right cell (Direct _tag / Niche
  niche-cell memcpy / untagged|scalar:None no-op); construction writes tag + disjoint payload
  via views (union aliasing), ZST omitted, array fields via memcpy; determinism.
- Niche byte storage: memcpy at TagScalar.offset (NOT a pointer-cast reinterpret) sound under
  the pinned flags (no -fno-strict-aliasing); payload fields direct (no §14.7 memcpy).
- EnumValue scope: exactly single-member lowered (inline — niche niched only when the cell
  coincides with one inline scalar field, else d2.2; multi-view fieldless Direct);
  multi-member → distinct d2.2 label; non-scalar payload child transitively defers; Bytes /
  Aggregate arms unchanged.
- Scope discipline d2.1 vs d2.2 vs vtable: right arms filled; no task-d2.1 label left; new d2.2
  label distinct; no enum-layout/discovery/type-def/spec/Doc/harness change; enum_emit output
  byte-identical.
- Verification honesty (run + _Static_assert over Direct/Niche/Single/repr-int shapes, no
  const-literal path); fail-loud negatives; no ledger green until e; determinism; scope creep
  vs Non-Goals; citation drift vs HEAD.
Read: docs/discussions/regseq-model/generic-c-backend.md §13/§14/§12; crates/regseq-backends/
src/generic_c/{enum_emit,body_emit,layout_emit,types}.rs; crates/regseq-model/src/{layout,
place,rvalue,statement,const_value}.rs + spec.md §11.1/§11.2 + §21/§22; crates/regseq-driver/
src/bridge/const_.rs:91-103; crates/codegen-coverage-runner/tests/generic_c_bodies.rs;
crates/codegen-coverage-runner/src/toolchain.rs.
</task>

<grounding_rules>
- Cite file:line for every claim. Mark hypotheses as hypotheses.
- Don't speculate about code/spec you didn't read.
</grounding_rules>

<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
NAVIGATOR-AGREEMENT (one shared enum_emit source of truth; access==definition member path; emit_enum refactored byte-identical; Downcast-pending-Field): PASS / FAIL with citation
DISCRIMINANT (matched-variant discriminant not raw cell; Direct ternary; Niche assign-site memcpy+untagged fallback; Single/Empty; signed render; no niche re-derivation): PASS / FAIL with citation
SETDISC-AND-CONSTRUCT (tag_value to the right cell; no-op cases; construction tag+disjoint payload via views; ZST/array; determinism): PASS / FAIL with citation
NICHE-BYTE-STORAGE (memcpy at offset not pointer-cast; sound under -std=gnu11 no -fno-strict-aliasing; payload direct): PASS / FAIL with citation
ENUMVALUE-SCOPE (exactly single-member lowered — inline niche niched only when field.offset==TagScalar.offset && field.ty==scalar_ty, else d2.2; multi-member→distinct d2.2 label; non-scalar child defers; Bytes/Aggregate unchanged): PASS / FAIL with citation
SCOPE-DISCIPLINE (right arms; no task-d2.1 left; distinct d2.2 label; no layout/discovery/spec/Doc/harness change; enum_emit byte-identical): PASS / FAIL with citation
VERIFICATION-HONESTY (run + _Static_assert over Direct/Niche/Single/repr-int; no const-literal path; fail-loud negatives; no ledger green until e): PASS / FAIL with citation
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
- `cargo fmt --all -- --check` + `cargo clippy -p regseq-backends -p codegen-coverage-runner
  --all-targets -- -D warnings` clean.
- `cargo test -p regseq-backends -p codegen-coverage-runner -- --test-threads=1` green (all
  c2/d1/d2 tests unchanged + the new d2.1 tests).
- A bridged enum-matching / enum-constructing function emits a TU whose `void regseq_main`
  exits 0 (correct tag read/write + payload access) / aborts on a wrong result; the §13
  `_Static_assert`s hold; the multi-member `Const::EnumValue` negative returns
  `EmitError::Unsupported` labelled d2.2.
- `emit_generic_c` byte-stable; `enum_emit` output byte-identical (Change A is a refactor);
  ledger verdict unchanged (`not-pass`); no Doc-1 flip.
- `git diff --stat` confined to `{body_emit.rs, enum_emit.rs}` + the runner test + the
  `mod_tests.rs` cases + registration.
- Codex Task Review: APPROVED or APPROVED_WITH_MINOR_FIXES.
- **Mandatory post-close Codex soundness pass** ([[assessment-soundness-pass]]) — d2.1 is
  layout-invariant-touching; the green compile-and-run fixtures can mask a navigator/offset
  hole (the d1 + d2 lesson).

## Acceptance Criteria

1. **Shared navigator** (`enum_emit.rs`): the view-list construction + inline-vs-`union u`
   collapse + `_tag`/`_v<idx>` member names + `view_field_name` payload names are **one
   `pub(super)` source of truth** consumed by **both** `emit_enum` (refactored, output
   byte-identical) and the body accessors; nothing re-derives a member path, offset, tag value,
   or name.
2. **`Downcast` + `Field`** (`place_walk`): a `Downcast(v)` threads a pending variant the
   following `Field(i, ty)` consumes → the variant payload member path (`.u._v<v>.<name>` /
   inline) via the navigator, narrowing to the carried `ty`; payload access is **direct** (no
   §14.7 `memcpy`); the non-enum `Field` path is unchanged.
3. **`Discriminant` on an enum**: returns the matched variant's **`discriminant`** — `Single`
   constant; `Multiple{Direct}` a ternary over the `_tag` read; `Multiple{Niche}` an
   assign-site `memcpy` of the niche cell into a temp then a ternary with the `untagged_variant`
   fallback; `Empty` unreachable. Compares against `tag_value` (read, **never** re-derived);
   signed rendering per §11.1.
4. **`SetDiscriminant`**: `Direct`/`Single{scalar:Some}` write `tag_value` to the `_tag` cell;
   `Niche` niched `memcpy`s `tag_value` into the niche cell at `TagScalar.offset`; `Niche`
   untagged / `Single{scalar:None}` are elided (no-op).
5. **`Aggregate::Enum` construction** (assign site): the tag write (per the variant's encoding)
   + each non-ZST payload field written through the variant view (disjoint union members; array
   fields via `memcpy`; ZST omitted); deterministic order.
6. **Single-member `Const::EnumValue`**: an inline enum (a niche **niched** variant **only**
   when its niche cell coincides exactly with one inline scalar field — `field.offset ==
   TagScalar.offset` and `field.ty == scalar_ty` — else d2.2) or a multi-view fieldless `Direct`
   variant lowers to a single compound literal; a multi-member case → `EmitError::Unsupported`
   labelled `…(multi-member): task d2.2`; a non-scalar payload child transitively defers via
   its own arm; `Const::Bytes` / `Const::Aggregate` arms unchanged.
7. **Niche byte storage**: the niche cell is accessed via `memcpy` to/from a typed temporary at
   `TagScalar.offset` (sound under `-std=gnu11 -fwrapv -Wall`, no `-fno-strict-aliasing`), not a
   reinterpreting pointer cast; no GNU statement-expression is introduced.
8. **Fail-loud deferral seams**: no `task d2.1` label remains; the multi-member `EnumValue`
   case + all `Bytes` / `Aggregate` arms fail loud with distinct d2.2 labels; the `dyn` / vtable
   arms are unchanged; a negative test per family proves it.
9. **Verification**: a bridged enum-matching / enum-constructing function compiles **and runs**
   — `regseq_main` exits 0 iff the tag read/write + payload access reproduced rustc's
   discriminant semantics across the **inhabited** shapes
   `Single{scalar:None|Some}`/`Multiple{Direct|Niche}` (incl. explicit `repr(int)` discriminants
   where `discriminant != tag storage index`); `EnumShape::Empty` (uninhabited) gets
   **structural/unreachable** coverage only — never a runtime value/match fixture; the
   §13 `_Static_assert`s hold; fixtures use **runtime** values only (no literal enum/reference
   const → d2.2); determinism holds; **no scope creep** — no multi-member-const / non-scalar-const
   / `dyn` / vtable lowering, no enum-layout / discovery / type-definition / spec / Doc / bridge /
   model / matrix / ledger / harness change, `enum_emit` output byte-identical; diff confined to
   the Quality-Gate file set.
```
