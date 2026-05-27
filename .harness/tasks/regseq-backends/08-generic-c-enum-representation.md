# regseq-backends-08: Generic-C enum representation (§13/D9) (Phase-1 task c2)

## Description

Fill the **enum-body hook** in bank 3 of the generic-C 5-bank driver
(`mod.rs:443-451`, the `/* TODO(task c2) */` branch): emit the
**offset-faithful C `struct` representation** for every `InstanceKind::Enum`,
**reusing task c1's §14/D10 field-offset primitive** (sort-by-offset +
explicit padding + `_Static_assert` guards). This is **task c2** of the
Phase-1 a→b→c1→**c2**→d→e sequence — the enum half of the split "(c) types +
layout", flagged as the Doc-2 top-risk item ("was WRONG" on first contact:
the naive `{ tag; union; }` shape is unsound — see Doc 2 D9).

After this task, `emit_generic_c` emits compilable C type definitions for
**all enum shapes** (`EnumShape::{Empty, Single, Multiple}` ×
`TagEncoding::{Direct, Niche}`), each with `_Static_assert(sizeof / _Alignof /
offsetof)` layout guards — so compiling the emitted TU *is itself* the proof
that the lowering reproduces rustc's enum layout. Combined with c1, banks 2–3
now emit definitions for **every** sized aggregate (struct / union / tuple /
closure / array / slice / **enum**). **No green codegen-coverage fixture is
produced** (a green fixture needs a function body + diag-log → tasks d/e);
c2's verification is backend-internal (bridge an enum snippet → emit → the TU
compiles, i.e. the layout guards hold), the same pattern tasks b and c1 used.

### Scope boundaries (flagged for review — see § Review)

1. **Representation only — NOT tag *operations*.** c2 emits the enum C type
   *definition* + layout guards. The D9 "tag operations" —
   `Discriminant(place)` (RV-8), `SetDiscriminant(place, v)` (ST-2),
   `Downcast(v)`/payload field access (PL-8), `Aggregate::Enum` (AG-4),
   `Const::EnumValue` (CO-9) — are **use-site / body lowering = task d**
   (exactly symmetric with c1 deferring struct field *access* to d). c2
   touches no `Rvalue` / `Statement` / `Place` lowering.
2. **No tag-value constants / macros.** The per-variant tag-value
   `#define`/named constants the §9.8 example shows (`STATUS_IDLE`, …) are
   consumed by the tag operations (match arms, `SetDiscriminant` stores) in
   task d; emit them there. c2 reads `VariantLayout.tag_value` only to decide
   the *representation* (whether a `Direct` tag field is present); it emits
   no tag-value literals.
3. **Reuse (and a small refactor of) c1's §14 primitive is the only
   c1-code touch.** c2 factors `layout_emit`'s field-body builder so the enum
   variant payload structs + the outer tag/payload placement call it; c1's
   public `emit_aggregate` behavior (and its tests) stay unchanged. No other
   change to `naming` / `types` / discovery / topo-sort — the enum node is
   **already** discovered, named, and topo-ordered by c1 (its payload fields
   are in the §15 edge set, `mod.rs:174-181`); c2 only fills the body.

## Input Context

### Design source — `docs/discussions/regseq-model/generic-c-backend.md`

- **§13 Enum C representation** (lines 916–955): the offset-faithful contract.
  An enum lowers to a C `struct` built by the **§14 field-offset technique**
  (NOT a naive `{ tag; union; }`, which falsely assumes tag-at-0 +
  contiguous payload). A tag cell is present iff `Single{scalar:Some}` or
  `Multiple{encoding:Direct}` — the canonical §13 (line 925) says "`Single
  {scalar:Some}`, `Multiple`", but read together with its "Niche enums carry no
  tag field" clause (line 929) this is **`Direct`-only for `Multiple`**. The
  payload region is a `union` of per-variant payload structs, **each itself
  offset-faithful** (variant field offsets are **absolute to the whole enum
  object**, not the payload start). **Niche enums carry no tag field** — the
  niche cell is a payload field whose bit pattern *is* the tag (§9.9).
  `_Static_assert` guards validate every offset/`sizeof`. (c2's *placement* of
  these uses the union-of-views form — see Change A's doc-override note.)
  (The §13 "tag operations" paragraph drives `Discriminant`/`SetDiscriminant`/
  `Downcast` — those are **task d**, out of scope here; c2 emits only the
  representation those operations later index into.)
- **§14 Aggregate field-offset reproduction** (lines 959–989): the **reused
  primitive**. The 7-step technique (sort by `offset`; omit ZST fields but
  account for them; explicit `unsigned char _padN[k]` fillers + trailing pad;
  alignment via `__attribute__((aligned(layout.align)))`, +`packed` when
  `pack.is_some()`; unions = all-offset-0; the three `_Static_assert` forms).
  c2's per-variant payload structs and the outer enum struct are emitted by
  this same machinery (factored from c1's `layout_emit`).
- **§2 enum row + §9.8 / §9.9 worked examples** (lines 721–816): §9.8 = a
  `repr(u32)` fieldless `Direct` enum (a single `uint32_t tag` field); §9.9 =
  a niche-encoded `Option<&u32>` (no tag field; the inner pointer's bit
  pattern is the tag). c2 generalizes both to **payload-carrying** enums.
- **Doc 2 D9** (`02-architecture-decisions.md:419-487`): the locked
  offset-faithful decision + the refutation of the naive shape (Codex
  top-risk #1). D9's "tag operations" list is the task-d contract; c2
  implements the **representation** D9 pins (tag cell at offset + payload
  union of offset-faithful per-variant structs + the guards). **Doc 2 D10**
  (lines 491–531): the §14 primitive D9 reuses.

### Representation: a **union-of-views** (overrides §13/D9's literal form)

**§13/D9 describe a "tag field at `TagScalar.offset` + a payload `union` of
per-variant structs placed at `union_start`" (the variant fields at
`offset − union_start`). That literal form is NOT C-placeable for all `Direct`
layouts** — c2 uses the strictly-general **union-of-views** form below instead,
and **flags a §13/D9 doc fix** (see Change A; mirrors the c1 §14/§15 doc
overrides — the `_Static_assert` guards make the implementation self-correcting
regardless).

*Why the literal form breaks (the bug).* Tag and payload bytes never
interleave — for `Direct`, rustc lays the discriminant out as a **prefix**
(`rustc_abi/src/layout.rs:803-824`, `StructKind::Prefixed`; variant fields
start at `prefix_size.align_to(prefix_align)`, lines 1260-1264), so payload
fields sit at offsets ≥ the tag end. **But the payload region is not
relocatable to a single `union_start`.** Example: `enum E { A(u8), B(u32) }` —
tag `u8` @0, `A.0` @1, `B.0` @4, size 8, align 4. `union_start = min(1, 4) = 1`,
but a C `union` containing a `uint32_t` member has natural alignment 4 and
**cannot sit at offset 1**, and within it `B.0` would land at `4 − 1 = 3`
(misaligned) — the `_Static_assert(offsetof…)` would fail the compile. Packing
the synthesized union to force offset 1 is fragile (byte-wise access semantics,
brittle attribute tuning).

*The fix — union-of-views (no `union_start`, no packing).* Each variant's
payload **and** the tag are emitted as **full-object offset-faithful views**
(structs whose fields sit at their **absolute** offsets from the enum object's
start, padded to `layout.size`), all sharing **one** `union`, wrapped in the
outer `struct E`:

```c
struct E {
    union {
        struct { uint8_t tag; }                  _tag;  /* Direct/Single-Some */
        struct { unsigned char _p0[1]; uint8_t a; }  _v0;   /* A.0 @1 */
        struct { unsigned char _p0[4]; uint32_t b; } _v1;   /* B.0 @4 */
    } u;
} __attribute__((aligned(4)));
_Static_assert(sizeof(struct E) == 8, "E size");
_Static_assert(_Alignof(struct E) == 4, "E align");
_Static_assert(offsetof(struct E, u._tag.tag) == 0, "E tag");
_Static_assert(offsetof(struct E, u._v0.a) == 1, "E A.0");
_Static_assert(offsetof(struct E, u._v1.b) == 4, "E B.0");
```

Because each view starts at the enum object's offset 0, **a field's offset
within its view equals its absolute offset**, which rustc already placed at a
naturally-aligned position (rustc respects each field type's alignment) — so
**no packing is needed**, each view is a plain §14 struct, and the union
aliasing reproduces the exact byte layout (the tag view's bytes [0,1) and the
`_v1` view's bytes [4,8) are disjoint regions of the same object, read through
their respective views). This handles **any** tag/payload placement, any
alignment, `Direct` / `Niche` / `Single` / `Empty` uniformly. (`Niche`: no
`_tag` view — the niche cell is a payload field in the variant views.)

### Code anchors (IR surface — confirm line numbers at HEAD)

- `crates/regseq-model/src/layout.rs` (the enum IR c2 consumes):
  - `Layout { size, align, pack: Option<u32>, uninhabited, kind }`;
    `LayoutKind::Enum { shape: EnumShape }` (lines 46-50).
  - `EnumShape::{ Empty, Single { variant: VariantLayout, scalar:
    Option<TagScalar> }, Multiple { tag: TagScalar, encoding: TagEncoding,
    variants: Vec<VariantLayout> } }` (lines 87-108).
  - `VariantLayout { name: StringId, fields: Vec<FieldLayout>, uninhabited:
    bool, discriminant: u128, tag_value: Option<u128> }` (lines 115-136) —
    `fields` in **source/MIR order**; each `FieldLayout.offset` is **absolute
    to the whole enum object** (lines 63-67), *not* the payload start.
  - `TagScalar { offset: u64, scalar_ty: TyId, valid_range: WrappingRange }`
    (lines 146-157) — `scalar_ty` is one of `Bool`/`Char`/`Int`/`Uint`/`Ptr`/
    `Ref` (lines 150-154; spell it via `types::declare`).
  - `TagEncoding::{ Direct, Niche { untagged_variant, niche_variants,
    niche_start } }` (lines 164-183).
- `crates/regseq-model/src/ty.rs:294` — `Ty::Enum { item, args, layout }`
  (already spelled `struct <name>` by c1's `types::declare`, `types.rs:87`).
- `crates/regseq-backends/src/generic_c/mod.rs`:
  - `bank3_type_definitions` lines 430-469 — the enum branch at **444-448**
    (the `/* TODO(task c2) */` marker) is the **one hook c2 fills**.
  - `raw_field_tys` lines 147-183 + `discover`/`topo_sort` — already treat
    enums as graph nodes and include **every variant's payload fields** in the
    §15 by-value edge set (lines 174-181), so the enum node is discovered,
    named (`build_name_table`), and topo-ordered **before** c2 runs. c2 adds
    **no** discovery / naming / ordering code.
- `crates/regseq-backends/src/generic_c/layout_emit.rs` — the **§14 primitive
  to reuse / refactor**: `emit_aggregate` (lines 112-143), `emit_struct_body`
  (sort + `_padN` + per-field offsetof guard, lines 148-188), `align_attr`
  (95-107), `is_zst` (91-93). `resolve_view` returns `None` for
  `LayoutKind::Enum` today (line 67) — that's the c2 seam.
- `crates/regseq-backends/src/generic_c/{types.rs, naming.rs}` — `declare`
  (field-type spelling, used unchanged), `sanitize` (field names, unchanged).
- `crates/regseq-model/src/model.rs:1831` — `size_align_of(TyId)` for ZST
  detection + field sizes (read-only; **no layout recomputation**).
- `crates/codegen-coverage-runner/tests/generic_c_layout.rs` — c1's behavioral
  harness (the `try_bridge_fixture_path` + `toolchain::compile_and_run` +
  hand-built-model patterns c2's tests mirror).

### Prior tasks

- `regseq-backends-06` (`f410115`, done) — the c1 foundation (spelling +
  naming + the §14 primitive + discovery/topo-sort/banks 2-3). c2 fills the
  single enum hook and reuses the primitive.
- `regseq-backends-07` (pending) — c1 edge-case hardening (independent;
  includes the `_padN` / union-`_pad` collision-proof name fix for the §14
  primitive). c2 **does not depend on 07**: c2's only *new* synthesized member
  names (`tag` / `payload` / `vN`) live in synthesized-only scopes with **no
  real-field siblings**, so they cannot collide; the inner-struct `_padN`
  collision is the **shared §14 primitive's** concern, fixed by 07 — c2
  inherits that fix for free once 07 lands (no cross-task coupling needed).
- Doc-1 status reconciliation (`111da9b`): the rows c2 **enables** (TY-18
  enum `Ty`, LY-5/7..14 enum-layout rows, IN-10 enum instance, IT-5, the
  PL-8/RV-8/ST-2/CO-9/AG-4 *operation* rows that d will close) go **green only
  when a behavioral fixture exercises them** (d/e). c2 flips **no** matrix
  status and adds **no** green fixture.

## Implementation Notes

### Change A: the enum emitter (`generic_c/enum_emit.rs`, new module — §13/D9)

`emit_enum(model, names, ty, enum_name) -> String` builds the outer C
`struct E { union { …views… } u; } __attribute__((aligned(layout.align)))`
(the **union-of-views** form above), one **view** per:

- **Tag view** — present **iff** `Single { scalar: Some(tag) }` **or**
  `Multiple { encoding: Direct, tag, .. }`. A view holding
  `<types::declare(tag.scalar_ty)> tag;` at **absolute offset `tag.offset`**
  (via leading `_padN`). **Absent** for `Multiple { encoding: Niche }` (the
  niche cell is a payload field — §13, §9.9), `Single { scalar: None }`, and
  `Empty`.
- **Variant views** — one per variant **that has ≥1 non-ZST field** (fieldless
  variants place no bytes → **omit their view**; this makes §9.8 collapse to a
  tag-only struct and §9.9 to a single-field struct). Each view's non-ZST
  fields are placed at their **absolute** `FieldLayout.offset` via the §14
  technique (sort, `_padN` fillers, trailing pad to `layout.size`, ZST omit).

Then:

- **Collapse degenerate cases**: if exactly **one** view results, emit it
  inline as `struct E { …fields… }` (drop the `union`/view wrapper) — so a
  fieldless `Direct` enum is `struct Status { uint32_t tag; }` (§9.8) and
  `Option<&u32>` is `struct Option { const uint32_t *p; }` (§9.9). For **≥2**
  views, the `union { … } u;` wrapper. For **zero** views (an `Empty` enum, or a
  `Single { scalar: None }` whose sole variant is fieldless) emit a
  size-faithful struct: `struct E { unsigned char _pad[size]; }` (a size-0
  uninhabited enum → an empty `struct` under `-std=gnu11`).
- **Alignment**: force the **outer** `struct E` to
  `__attribute__((aligned(layout.align)))` (enums are never `repr(packed)`;
  `pack` is expected `None`, carried uniformly like c1). Each view is a plain
  (non-packed) §14 struct: because view-relative offset == absolute offset and
  rustc places fields at naturally-aligned absolute offsets, **no view needs
  packing** (this is what fixes the shifted-alignment bug — no `union_start`,
  no relative-offset packing).
- **Uninhabited variants** (`VariantLayout.uninhabited`) — emit their view
  structurally if they carry fields (a harmless `union` member never selected);
  offsets still come from the real layout, so the guards hold.

**Guards** (the self-checking proof): `_Static_assert(sizeof(struct E) ==
size)`, `_Static_assert(_Alignof(struct E) == align)`, and for the tag + every
non-ZST variant field, `_Static_assert(offsetof(struct E, <path>) ==
<absolute offset>)` where `<path>` is `u.<view>.<field>` (union-of-views form)
or `<field>` (collapsed single-view form). The **nested member designator** in
`offsetof` is a GCC/Clang feature — pinned as a **host-target assumption**,
consistent with the existing Doc-3 `-std=gnu11` + gcc/clang host assumptions.
A wrong tag or field offset fails the compile on the `_Static_assert` (stage 4).

**Doc-override note.** This **overrides** the §13/D9 literal "tag field +
payload `union` at `union_start`" wording
(`generic-c-backend.md:922-930`, `02-architecture-decisions.md:432-448`), which
is unsound for shifted-alignment `Direct` enums (the worked
`enum E { A(u8), B(u32) }` case in the representation note above). **Flag a
follow-up doc fix** to §13 + Doc-2 D9 to record the union-of-views form (the
same way the c1 task flagged the §14/§15 alignment + fat-pointer doc fixes).
The `_Static_assert` guards make the implementation correct regardless of the
doc wording.

### Change B: reuse / refactor the §14 primitive (`generic_c/layout_emit.rs`)

Factor the field-body builder out of `emit_struct_body` so c2's enum emitter
reuses **the same** sort-by-offset + `_padN`-filler + per-field-`offsetof`
logic for each view. Because each view's fields are at **absolute** offsets
(view starts at the enum object's offset 0), **no base-offset shifting is
needed** — the only parameter the enum use adds is a **member-path prefix** for
the `offsetof` guard (so a view's guards read `offsetof(struct E, u.<view>.f)`
while c1's still read `offsetof(struct T, f)`).

Keep `emit_aggregate`'s public signature + output byte-for-byte unchanged for
non-enum aggregates (c1's structural + behavioral tests must stay green —
verify by running them). Field *types* via `types::declare`; field *names* via
`naming::sanitize(FieldLayout.name)`; view member names `_tag` and `_vN`
(index-based, deterministic). The view member names live in synthesized-only
scopes (no real-field siblings), so they cannot collide with field names; the
inner-struct `_padN` collision is the shared §14 primitive's concern (07).

### Change C: wire bank 3 (`generic_c/mod.rs`)

Replace the enum branch at `mod.rs:444-448` (the `/* TODO(task c2): enum body
… */` marker) with `enum_emit::emit_enum(model, names, *ty, name)`. The enum
node is already discovered + named + topo-ordered by c1, so this is the only
wiring needed. No change to `discover`, `raw_field_tys`, `containment_target`,
`build_name_table`, `topo_sort`, bank 2, or banks 4/5.

### Change D: tests

- **Structural** (`regseq-backends`, hand-built and/or bridged models): assert
  the emitted TU contains — for a multi-view `Direct` enum, the `u._tag` view +
  per-variant `u._vN` views + the nested `offsetof(struct E, u._tag.tag…)` /
  `offsetof(struct E, u._vN.…)` guards; for `Option<&u32>` (`Niche`), the
  **collapsed single-field** `struct` (no tag, no union — §9.9); for a
  fieldless `Direct` enum (§9.8), the **collapsed tag-only** `struct`; for
  `Empty`, a size-faithful (possibly empty) struct. Assert **byte-stability**
  across two `emit_generic_c` calls.
- **Behavioral** (`codegen-coverage-runner/tests`, reusing
  `toolchain::compile_and_run`): bridge enum snippets — a `#[repr(u32)]`
  fieldless enum (§9.8 Direct, collapsed), `Option<&u32>` (§9.9 Niche,
  collapsed), a **multi-field-payload** `repr(Rust)` enum (e.g. `enum E { A(u8),
  B { x: u32, y: u16 }, C }`), **and — named explicitly so it cannot be missed
  — the shifted-alignment `enum Shift { A(u8), B(u32) }`** (tag `u8` @0, `A.0`
  @1, `B.0` @4; the case the literal §13/D9 form could not place: the test
  asserts the `B` view's `uint32_t` lands at absolute offset 4 and the TU
  compiles clean). Each: emit → compile the TU + a trivial `regseq_main` →
  assert it compiles **clean** (the §14 guards passing *is* the layout-fidelity
  proof) and exits 0. A **wrong-offset injection** (a hand-built enum `Layout`
  whose tag or a variant field claims an impossible offset) → `compile_and_run`
  returns `ToolchainError::CompileFailed` (the `_Static_assert` bites). Where
  the bridge cannot currently surface a shape (cf. c1's `repr(packed)` note in
  `generic_c_layout.rs:62-70`), hand-build the model.

## Non-Goals

- **No tag operations** — `Discriminant` / `SetDiscriminant` / `Downcast` /
  `Aggregate::Enum` / `Const::EnumValue` (RV-8/ST-2/PL-8/AG-4/CO-9) are
  **task d** (use-site/body lowering). c2 emits only the type representation.
- **No tag-value `#define`s / named constants** (§9.8 `STATUS_*`) — task d.
- **No field-access lowering** (`.fname` / `memcpy` through enum storage) —
  task d.
- **No bank 4/5** (prototypes, `extern` statics, bodies, static inits) —
  task d. c2 leaves those hooks empty.
- **No `dyn`/vtable** work; **no green codegen-coverage fixture / no Doc-1
  status flip** (a green fixture needs a body + diag-log → d/e).
- **No discovery / naming / topo-sort changes** — c1 already roots, names, and
  orders the enum node; c2 fills only the body.
- **No layout recomputation** — trust `Layout` / `EnumShape` / `VariantLayout`
  / `TagScalar` as bridge certificates; `size_align_of` is read-only.
- **No deps added; no harness changes.** Do **not** modify the spec, Docs 1–3,
  the bridge, the model, or `test-ir-fixtures`. The **only** c1-code touch is
  the `layout_emit` field-body refactor (Change B), within `regseq-backends`.

## Test Plan

### B1: build + workspace green
- `cargo build -p regseq-backends -p codegen-coverage-runner` clean.
- `cargo test -p regseq-backends -p codegen-coverage-runner -- --test-threads=1`
  green — **all c1 tests still pass** (the `layout_emit` refactor preserves
  non-enum output), plus the new c2 tests.

### B2: enum layout fidelity (Change D)
- Structural: multi-view `Direct` enum → `u._tag` + `u._vN` views + nested
  `offsetof(struct E, u.…)` guards; `Niche` (`Option<&u32>`) → collapsed
  single-field struct; fieldless `Direct` (§9.8) → collapsed tag-only struct;
  `Empty` → size-faithful struct.
- Behavioral (via `toolchain`): bridged §9.8 / §9.9 / multi-field-payload /
  **the shifted-alignment `enum Shift { A(u8), B(u32) }`** snippets → emit →
  **TU compiles clean** (layout guards hold) + exits 0. The wrong-offset
  injection → compile **fails** on the `_Static_assert`.

### B3: determinism
- `emit_generic_c` byte-identical across two calls on an enum-containing model
  ([[codegen-ledger-determinism]]).

### B4: scope + ledger
- Ledger verdict unchanged (`not-pass`); no green fixture; no Doc-1 flip.
- No tag-op / field-access / bank-4/5 code added (a diff-scope assertion in
  review, not a unit test).

### B5: fmt + clippy
- `cargo fmt --all -- --check` clean; `cargo clippy -p regseq-backends -p
  codegen-coverage-runner --all-targets -- -D warnings` clean (scoped per
  [[clippy-all-targets-test-drift]]).

### B6: diff scope
Confined to `crates/regseq-backends/src/generic_c/**` (new `enum_emit.rs`, the
`layout_emit.rs` field-body refactor, the `mod.rs:444-448` bank-3 wiring + the
`mod enum_emit;` declaration), the runner's new behavioral test, and the
`tasks.json` registration. **No** spec / Doc / bridge / model / matrix /
ledger / `test-ir-fixtures` changes.

## Review

### Files to send to Codex
This spec; `generic-c-backend.md` §13/§14 (+ §2 enum row, §9.8/§9.9); Doc 2
D9/D10; `regseq-model/src/layout.rs` (the enum types) + `ty.rs:294`;
`regseq-backends/src/generic_c/{mod.rs, layout_emit.rs, types.rs}`;
`codegen-coverage-runner/tests/generic_c_layout.rs` (the harness c2 mirrors).

### Reviewer focus
- **Representation fidelity across ALL `EnumShape` × `TagEncoding`**: `Empty`,
  `Single{scalar:Some/None}`, `Multiple{Direct}`, `Multiple{Niche}`. Does the
  **union-of-views** form (a `_tag` view for Direct/Single-Some + a per-variant
  `_vN` view, each a §14 struct with fields at **absolute** offsets, collapsed
  inline for a single view) reproduce every case — including the
  shifted-alignment `Direct` case (`enum E { A(u8), B(u32) }`: tag@0, A@1, B@4)
  the literal `union_start` form could not place? Is the **tag-is-always-a-
  prefix** grounding (rustc_abi layout.rs:803-824, 1260-1264) correct? Is
  **niche = no tag field** right?
- **Absolute-offset claim**: is "view-relative offset == absolute offset"
  sound (each view a distinct struct laid out from 0; the union puts all views
  at offset 0 of `u`; `u` at offset 0 of `struct E`), so each field lands at a
  naturally-aligned absolute offset with **no packing**? Do the nested
  `offsetof(struct E, u._vN.f)` guards reproduce the absolute offsets? Does
  `sizeof(struct E) == size` / `_Alignof == align` hold (views padded to size;
  outer `aligned(align)`; size already a multiple of align)?
- **Reuse of the §14 primitive**: is the `layout_emit` refactor clean, and does
  it preserve c1's non-enum output byte-for-byte (c1 tests green)?
- **Scope discipline (c2 vs d)**: representation only — no tag operations / no
  tag-value constants / no field access / no bank 4/5. Is the line drawn
  correctly, and does d have everything it needs (the struct layout + the
  `tag_value`s in the IR) to implement the operations later?
- **Determinism**: pure function of the model (variant order, names, member
  ordering all deterministic)?
- **Verification honesty**: is "compile + `_Static_assert` holds" a sound proof
  of enum-layout fidelity absent a behavioral fixture, and does the
  wrong-offset injection genuinely prove the guards bite for an enum?
- **The nested-`offsetof` host assumption** — acceptable under the Doc-3
  gcc/clang + gnu11 toolchain?
- Citation drift vs HEAD.

### Adversarial prompt template
```
<task>
Review .harness/tasks/regseq-backends/08-generic-c-enum-representation.md —
Phase-1 task c2 of the generic-C backend: the offset-faithful C enum
representation (§13/D9), reusing task c1's §14/D10 field-offset primitive.
c2 emits the enum type DEFINITION + _Static_assert layout guards only; the
tag OPERATIONS (Discriminant/SetDiscriminant/Downcast/Aggregate::Enum/
Const::EnumValue) and tag-value constants are task d. No green fixture
(needs a body+diag-log, d/e); verification is backend-internal (bridge an
enum snippet -> emit -> the _Static_assert layout guards hold on compile).
Identify:
- Representation fidelity across ALL EnumShape × TagEncoding (Empty / Single
  {scalar Some/None} / Multiple{Direct} / Multiple{Niche}). Does the
  union-of-views form (a _tag view for Direct/Single-Some + a per-variant _vN
  view, each a §14 struct with fields at ABSOLUTE offsets, collapsed inline
  for a single view) reproduce every layout, incl. the shifted-alignment
  Direct case enum E { A(u8), B(u32) } (tag@0,A@1,B@4)? Is the
  tag-is-always-a-prefix grounding (rustc_abi/src/layout.rs:803-824,1260-1264)
  correct? Is niche=no-tag-field right?
- Absolute-offset soundness (view-relative==absolute; no packing; naturally
  aligned); sizeof/_Alignof; the nested offsetof(struct E, u._vN.f) guard
  form (GCC/Clang host assumption under gnu11).
- Reuse of the §14 primitive (clean refactor, member-path prefix only / no
  base offset; preserves c1 output byte-for-byte / c1 tests green).
- Scope discipline c2 (representation) vs d (operations/constants/access/
  banks 4-5); does d have what it needs (layout + tag_value in the IR)?
- Determinism (pure function of the model).
- Whether "compile + _Static_assert holds" soundly proves enum-layout
  fidelity, and the wrong-offset injection proves the guards bite.
- Scope creep vs Non-Goals; citation drift vs HEAD.
Read: generic-c-backend.md §13/§14/§2/§9.8/§9.9; Doc 2 D9/D10;
regseq-model/src/layout.rs + ty.rs:294;
regseq-backends/src/generic_c/{mod,layout_emit,types}.rs;
codegen-coverage-runner/tests/generic_c_layout.rs.
</task>

<grounding_rules>
- Cite file:line for every claim. Mark hypotheses as hypotheses.
- Don't speculate about code/spec you didn't read.
</grounding_rules>

<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
REPRESENTATION-FIDELITY (all EnumShape×TagEncoding, incl. shifted-align): PASS / FAIL with citation
TAG-PREFIX-GROUNDING (no interleaving missed): PASS / FAIL with citation
ABSOLUTE-OFFSET + SIZEOF/ALIGNOF + NESTED-OFFSETOF: PASS / FAIL with citation
PRIMITIVE-REUSE (preserves c1 output): PASS / FAIL with citation
SCOPE-DISCIPLINE (c2 vs d): PASS / FAIL with citation
DETERMINISM: PASS / FAIL
VERIFICATION-HONESTY: PASS / FAIL with citation
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
- `cargo test -p regseq-backends -p codegen-coverage-runner -- --test-threads=1`
  green (all c1 tests unchanged + the new c2 tests).
- Bridged §9.8 / §9.9 / payload-carrying enum snippets emit a TU that compiles
  clean (layout guards hold); the wrong-offset injection fails the compile on
  the `_Static_assert`.
- `emit_generic_c` byte-stable; ledger verdict unchanged (`not-pass`).
- `git diff --stat` confined to the Change-A–D file set + registration.
- Codex Task Review: APPROVED or APPROVED_WITH_MINOR_FIXES.

## Acceptance Criteria

1. **Enum representation** (`enum_emit.rs`): every `EnumShape` lowers to an
   offset-faithful C `struct` via the **union-of-views** form — a `_tag` view
   (iff `Single{scalar:Some}` or `Multiple{Direct}`) + one `_vN` view per
   variant with ≥1 non-ZST field, each view's fields at their **absolute**
   offsets; `Multiple{Niche}` → **no `_tag` view** (the niche cell is a payload
   field); `Empty` → size-faithful struct. **No `union_start`, no packing.**
2. **View collapse**: exactly one view → emit inline (no `union` wrapper),
   collapsing §9.8 to a tag-only struct and §9.9 to a single-field struct; ≥2
   views → the `union { … } u;` wrapper; zero views → a size-faithful struct.
3. **§14 primitive reused**: each view goes through the (refactored)
   `layout_emit` field-body builder (member-path prefix only; no base-offset
   shift); c1's non-enum output is byte-for-byte unchanged (c1 tests green).
4. **Layout guards**: `_Static_assert` for `sizeof`, `_Alignof`, and the tag +
   every non-ZST variant field's absolute offset (`offsetof(struct E,
   u.<view>.f)`, or `offsetof(struct E, f)` in the collapsed form).
5. **Bank 3 wired**: `mod.rs:444-448` enum branch calls `emit_enum`; no
   discovery / naming / topo-sort / bank-2 / bank-4/5 change.
6. **Layout fidelity proven**: bridged §9.8 / §9.9 / payload-carrying enum
   snippets emit a TU that compiles clean; the wrong-offset injection fails
   the compile on the `_Static_assert`.
7. **Determinism**: `emit_generic_c` byte-identical across two calls.
8. **No scope creep**: no tag operations, no tag-value constants, no field
   access, no bank 4/5, no `dyn`, no green fixture, no Doc-1 flip; ledger
   verdict unchanged; diff confined to the Quality-Gate file set.
