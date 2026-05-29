# regseq-backends-18: Generic-C const full closure — the inline anonymous-union `_raw` + the M1 `_c<idx>` typed full view (Phase-1 task d2.3b)

## Description

Close the **three documented Phase-1 const limitations** d2.3a (`regseq-backends-17`,
`82930d5`) left fail-loud — **#2-reloc** (a multi-view tagged enum const with a
relocation payload), **#3** (an inline bool-niche), **#4** (an inline niche cell buried
in a nested aggregate) — so `emit_generic_c` lowers, compiles, and runs **every
reachable** enum/union `Const` shape under the pinned `cc` flags (`-std=gnu11 -fwrapv
-Wall`, **no** `-fno-strict-aliasing`, `toolchain.rs:84`). This is **task d2.3b** of the
Phase-1 a→…→d2.2→d2.3a→**d2.3b** sequence (Package A / full closure), the disposition of
the **plan-first design refinement**
`docs/discussions/generic-c-backend/phase-1/05-d2.3b-mechanism-refinement.md`
(Codex-reviewed, 2 rounds → APPROVED), which **supersedes** the M2-a/M2-b mechanism
sketch in `04-const-carve-out-resolution.md` §5.2.

The §05 refinement establishes (empirically, under the pinned flags) a **lighter, lower-
risk** mechanism than §04's M2-a (niche-cell retype) + named-union M2-b (de-inline):

- **An anonymous union+struct** gives a **single-view** inline enum a byte-covering
  `_raw` member while **member promotion keeps the runtime access path identical**
  (`e.field`) — so the **d2.1 navigator is untouched** — and the inline const path
  **serializes to `_raw` uniformly with multi-view enums**. This **subsumes M2-a
  entirely** (no niche-cell retype, no nested-designator locator, no
  `niche_coinciding_field`) and closes **all of #3 and #4** — top-level `_Bool`, nested
  non-bool, **and the nested `_Bool` M2-a could not** — because `_raw` is just bytes.
- **M1** (unchanged from §04): a **const-init-only** typed full-object view `_c<idx>`
  per payload-bearing variant of a **multi-view `Direct`** enum, so #2-reloc's tag +
  relocation go through **one** union designator (the reloc in a typed pointer slot;
  `_raw`, a `char[]`, cannot hold an address constant).

After d2.3b the **const exit gate is gap-free for every reachable enum/union shape**.
The remaining fail-louds are all **external-rustc-invariant defensive guards**, not
shipped gaps: a niche-**tagged** variant with a relocation payload (rustc niche-fills
only fieldless variants — §05 §3), plus the two retained guards **#5** (`Const::Aggregate`
over `Ty::Union`; rustc emits no union valtree) and **#6** (a `ConstMemoryStatic`
deduped at incompatible pointee types; bridge content-hash dedup — task
d2.3-bridge-dedup).

### What ships (five changes)

- **Change A — inline enums get an anonymous-union `_raw` (representation, layout-
  neutral).** Wrap a **single-view** inline enum's view in `union { struct { <view> };
  unsigned char <raw>[size]; };` (anonymous union + anonymous struct). Member promotion
  keeps `e.<field>` access **unchanged** (`member_prefix` stays `""`). The `<raw>` name
  is **collision-proof** against the promoted view field names, spelled through the
  **name-computing single source of truth** `enum_raw_member`.
- **Change B — the uniform inline const path.** An inline enum's **no-relocation** value
  (the niched variant of #3/#4, and any no-reloc inline byte image) serializes to `_raw`
  via the (generalized) `serialize_enum_value_to_raw` / `raw_byte_image` — exactly as a
  multi-view enum does. Drops the `niche_coinciding_field` special-casing and the #3/#4
  carve-outs. The untagged / `Single` / fieldless inline cases keep their existing
  (working) typed-designator paths; an inline **relocation** value keeps the typed
  single-view path (the single view *is* the whole object).
- **Change C — M1: the `_c<idx>` const-only typed full-object view.** For a **multi-view
  `Direct`** enum, `build_views` + `emit_enum` add, per payload-bearing variant, a
  `struct { <tag>; <payload fields…>; } _c<idx>;` union member at **absolute** offsets
  (the `tag` sub-field name collision-proofed). `offsetof`/sizeof/align guards;
  layout-neutral. The runtime `_tag`/`_v<idx>` views and the navigator are **unchanged**
  (`_c<idx>` is written only by the const path) — a new read-only navigator
  `enum_variant_typed_view` spells its member path.
- **Change D — route a multi-view `Direct` reloc payload to `_c<idx>` (#2-reloc).** A
  multi-view tagged enum const whose payload carries a relocation lowers to
  `(struct E){ .u._c<v> = { .<tag> = <tv>, <payload designators> } }` — both the
  `Const::EnumValue` (const/valtree) form and the `Const::Bytes`-with-`Byte::Reloc`
  (static) form. A **no-relocation** payload still serializes to `_raw` (Change C of
  d2.3a, unchanged).
- **Change E — relabels + doc delta + the closure record.** Sharpen the niche-tagged-
  reloc message to the external-invariant defensive-guard text (no new code); keep #5/#6;
  the bounded `generic-c-backend.md` **§12/§13** doc delta (§14 § Unions is **read-only**
  — the union `_raw` is d2.3a, unchanged); record the closure (and the one residual
  defensive guard) in `decisions-and-state.md`.

## Input Context

### Design source — the refinement + the prior design + the canonical spec

- **`docs/discussions/generic-c-backend/phase-1/05-d2.3b-mechanism-refinement.md`** —
  the mechanism this task implements (the anonymous-union `_raw`, the collision-proof
  naming, M1, the niche-tagged-reloc disposition, the code touchpoints §4). **Read it
  first** (it supersedes §04 §5.2).
- **`docs/discussions/generic-c-backend/phase-1/04-const-carve-out-resolution.md`** —
  the package decision + the #5/#6 dispositions (§2/§3) + the reloc-presence #2 split
  (§5.1). §5.2 M2-a/M2-b is **superseded** by §05.
- **`docs/discussions/regseq-model/generic-c-backend.md`**:
  - **§12 `Const` → C initializers** (the `EnumValue` row): a multi-view **tagged**
    enum const with a relocation payload now lowers through the `_c<idx>` typed full
    view; an inline enum const serializes to `_raw` (no-reloc) or the typed single view
    (reloc).
  - **§13 Enum representation** (the union-of-views form + the enum `_raw` member): add
    (a) the **inline anonymous-union `_raw`** (a single-view enum gains a promoted `_raw`
    sibling; access path unchanged) and (b) the **`_c<idx>` const-init-only typed full
    view** (per payload-bearing `Direct` variant; runtime navigator unchanged).
  - **§14 § Unions** — unchanged (the union `_raw` is d2.3a); the enum changes are §13.

### Code anchors (confirm at HEAD `6d2fe28`)

- `crates/regseq-backends/src/generic_c/enum_emit.rs`:
  - `emit_enum` (`:59-150`) — the view emitter; the inline-vs-`union` branch
    (`:81-137`): `union_wrap = views.len() >= 2` (`:98`); the multi-view `_raw` (`:134`);
    the 0-view `_pad0` (`:87`). **Change A** rewrites the **exactly-1-view** sub-case
    (currently `union_wrap == false`, fields emitted directly) to the anonymous
    union+struct + `_raw`; **Change C** adds the `_c<idx>` members to the multi-view
    (`union_wrap`) sub-case.
  - `build_views` (`:154-178`) — the tag view + one `_v<idx>` payload view per non-ZST
    variant. **Change C** adds the `_c<idx>` const views (a parallel list, or a `View`
    kind), consumed only by `emit_enum` + the new navigator.
  - `member_prefix` (`:251-257`) — `""` for ≤1 view, `u.<member>.` for ≥2. **Unchanged**
    (the anonymous union promotes, so 1-view access stays bare).
  - `enum_is_inline` (`:302`), `enum_variant_field` (`:324`), `enum_tag` (`:345`),
    `View`/`tag_view_scalar`/`shape_variants`/`variant_view_fields`/`view_field_name`
    (`:37-240`) — the navigator + view builders Change C extends.
  - `enum_raw_member` (`:312-314`) — **Change A**: becomes the **name-computing single
    source of truth**: a collision-proof bare name (1-view inline) / `u.<rawname>`
    (multi-view) / `None` (0-view); no hardcoded `"_raw"`.
- `crates/regseq-backends/src/generic_c/layout_emit.rs`:
  - `raw_member_name` (`:60-73`) — the collision-proof name primitive (seed the reserved
    set with the real member names; first free `_raw`/`_raw<n>`). **Change A** reuses it
    (make `pub(super)` and call from `enum_emit`, or factor a shared helper).
  - `emit_offset_fields` (`:272-312`) — the §14 field-body emitter (sort by offset, pad,
    `offsetof` guards via a `GuardDesignator`). **Reused** by Change A (the inline view
    struct) + Change C (each `_c<idx>` view). `ViewField` (`:94`), `GuardDesignator`
    (`:107`), `align_attr` (`:202`), `is_zst` (`:178`).
  - `union_raw_member` (`:83) / `emit_union_body` (`:327`) — the union `_raw` (d2.3a);
    the template Change A mirrors for enums. **Unchanged.**
- `crates/regseq-backends/src/generic_c/body_emit.rs` — the const seam:
  - `enum_value_expr` (`:3036-3157`) — the value-position dispatch. The **inline branch**
    (`:3053-3097`) is **Change B** (the Niche niched-variant arm → `_raw`; drop the #3/#4
    carve-outs `:3085`/`:3089`); the **multi-view branch** (`:3098-3156`) is **Change D**
    (route a `Direct` reloc payload to `_c<idx>` before the `serialize_multiview_enum_value`
    fall-through `:3155`). M1-lite (the niche-untagged route, `:3139-3150`) is unchanged.
  - `serialize_multiview_enum_value` (`:3335-3394`) — **Change B** generalizes it to
    `serialize_enum_value_to_raw` (it already works for any enum with a `_raw` + a tag
    location; the inline niched variant has both). The `tag_value.ok_or` (`:3357`) stays
    (a niched variant *has* a tag).
  - `serialize_const_into` (`:3185-3261`) / `serialize_aggregate_into` (`:3269`) — the
    const→byte-image walker (d2.3a). **Reused** verbatim by Change B's `_raw` path.
  - `enum_from_bytes` (`:2674-2700`) — **Change B** (a no-reloc inline byte image → `_raw`)
    + **Change D** (a multi-view reloc byte image → `_c<idx>` instead of the #2-reloc fail
    `:2687`). `reconstruct_inline_enum_from_bytes` (`:2728-2766`) / `reconstruct_inline_
    variant_fields` (`:2772-2811`) — Change B routes no-reloc through `_raw`, keeps these
    for the inline **reloc** byte image (a `Byte::Reloc` cannot enter `_raw`).
  - `niche_coinciding_field` (`:3577-3611`) — **removed** by Change B (the inline niche
    const now uses `_raw`; this becomes dead). `raw_byte_image` (`:2706`), `render_tag_value`
    (`:3659`), `enum_tag_location` (`:3401`), `lower_enum_payload` (`:3485`),
    `enum_value_payload_literal` (`:3563`), `const_carries_reloc` (`:3523`),
    `variant_layout` (`:3624`), `inline_payload_variant` (`:2816`) — read/reused.
  - `bytes_to_typed_literal` (`:2416`) — Change D's bytes path reconstructs each `_c<idx>`
    payload field through it (a reloc field → the reference carrier → `&Y`).
- IR (read-only — bridge certificates):
  - `crates/regseq-model/src/const_value.rs` — `Const::{Bytes, Aggregate, EnumValue,
    NullPtr, Reloc, Bool, Char, Int, Uint, Float}`; `Byte::{Uninit, Value, Reloc}`.
  - `crates/regseq-model/src/layout.rs` — `VariantLayout{ fields, tag_value, discriminant }`
    (`:115`), `TagScalar`, `TagEncoding::{Direct, Niche{untagged_variant}}`, `FieldLayout
    {name, ty, offset}`, `LayoutKind`. `layout_validator.rs:554` — the validator does
    **not** enforce "niched variants are fieldless" (so #2 niche-tagged-reloc stays a
    defensive guard).
  - `crates/regseq-model/src/{ty.rs, model.rs, target.rs}` — `Ty`, `model.size_align_of`,
    `model.types`, `model.consts.resolve`, `target.endian`.

### Prior tasks

- **d2.3a** (`regseq-backends-17`, `82930d5` + fixup `0ff35be`) — the union `_raw`, the
  recursive `serialize_const_into`, the **M1-lite** niche-untagged route. d2.3b reuses
  all of it; the inline `_raw` (Change A) mirrors the union `_raw`; the `_c<idx>` (Change
  C) is the multi-view analog of the inline single-view-is-the-whole-object property.
- **d2.2** (`regseq-backends-14`, `0347d92`) — the enum `_raw` member +
  `serialize_multiview_enum_value` + `bytes_to_typed_literal` + the CMS re-typing pre-pass.
- **d2.1** (`regseq-backends-13`) — the enum navigator + the niche raw-storage
  discriminant read (`body_emit.rs:765`/`:822`, raw memcpy — **so the inline `_raw` does
  not touch the discriminant read/write**, which is the soundness key for #3/#4).

### Test surface

- `crates/codegen-coverage-runner/tests/generic_c_bodies.rs` — the compile-**and-run**
  harness. d2.3b adds: **#2-reloc** (a multi-view `Direct` enum const holding a
  `&'static u32`, both `const` and `static` forms, deref the pointee back); **#3**
  (`Option<bool> = None` and `Some(true)`, match the discriminant + read the bool back);
  **#4** (an inline niche enum whose niche cell is a non-bool nested field, and one whose
  niche cell is a **nested `_Bool`** — the M2-a-impossible case — read the discriminant
  + the untagged payload back).
- `crates/regseq-backends/src/generic_c/mod_tests.rs` (+ any in-crate `*_tests.rs`) —
  structural tests. d2.3b **flips** the #2-reloc / #3 / #4 negatives (`mod_tests` sites
  for the d2.3b labels) to positives, adds the inline-`_raw` + `_c<idx>` structural +
  collision-proofing tests, and **re-blesses** the enum text-assertion goldens
  (single-view enums gain the anonymous-union wrap; multi-view `Direct` payload variants
  gain `_c<idx>`; size/align/`offsetof(field)` guards unchanged). Per
  [[regseq-model-test-surfaces]], grep the whole crate, not just `tests/`.

## Implementation Notes

### Change A: the inline anonymous-union `_raw` + `enum_raw_member` as source of truth

In `enum_emit.rs::emit_enum`, split the current `union_wrap = views.len() >= 2` branch
into three:

- **0 views** (`views.is_empty()`): unchanged (`_pad0[size]` / empty struct).
- **exactly 1 view**: emit
  ```c
  struct E {
      union {
          struct {
              <emit_offset_fields(view.fields, member_prefix "")>
          };
          unsigned char <raw>[size];
      };
  } __attribute__((aligned(align)));
  ```
  i.e. an **anonymous** `union` wrapping an **anonymous** `struct` of the single view's
  fields, plus the byte-covering `<raw>` member. `member_prefix` stays `""` (promotion),
  so the view's `offsetof` guards remain `offsetof(struct E, <field>)` (verified to hold
  under promotion). Add the guard `offsetof(struct E, <raw>) == 0`.
- **≥2 views** (`union_wrap`): unchanged shape (named `union { … } u;` + `_raw`) **plus**
  the Change-C `_c<idx>` members.

`enum_raw_member(model, shape)` becomes the **name-computing single source of truth**:
- 1-view inline → the **collision-proof** bare name = `raw_member_name(<the single
  view's field names>)` (make `layout_emit::raw_member_name` `pub(super)`, or factor a
  shared `crate::generic_c::raw_member_name`); the body side spells `.<raw>` (no prefix).
- ≥2-view → `u.<rawname>` where `rawname = raw_member_name(<the `u` member names:
  `_tag`/`_v<idx>`/`_c<idx>`>)` (these are generated, so `_raw` is free; compute for
  uniformity + future-proofing).
- 0-view → `None`.

`emit_enum` MUST spell the emitted `<raw>` member through the **same** `raw_member_name`
call (the same seed), so the definition and the const designator cannot drift. (This is
the §05 round-1 Codex fix: a Rust variant field named `_raw` promotes next to the array
member → `duplicate member '_raw'` without the reservation.)

**Soundness:** initializing `.<raw>` and reading a promoted typed member is gnu11 union
type-punning — sound under the pinned flags, **identical** to the d2.2/d2.3a `_raw`
argument (the read is through the union object, not an inactive-member-address deref).
Layout-neutral: `<raw>` is alignment 1 + full-object size; the anonymous union's
size/align equal the view's; sizeof/`_Alignof`/`offsetof` `_Static_assert`s hold
(promoted members are `offsetof`-addressable — compiler-verified, §05 §2).

### Change B: the uniform inline const path

- **`enum_value_expr` inline branch, the `EnumTag::Niche` arm (`:3069-3096`):** replace
  the `niche_coinciding_field` / bool-defer (#3) / nested-defer (#4) logic with:
  - **niched variant** (`variant.tag_value == Some(tv)`): serialize to `_raw` via
    `serialize_enum_value_to_raw(model, &agg, enum_ty, shape, v, payload)` (the
    generalized `serialize_multiview_enum_value`; it writes `tv` at `enum_tag_location`
    — the niche cell — and any payload via `serialize_const_into`, then wraps
    `(struct E){ .<raw> = { … } }` with `<raw>` from `enum_raw_member`). **A relocation
    in the payload still fails loud (#2-reloc)** via `serialize_const_into`.
  - **untagged variant** (`tag_value == None`): unchanged (`enum_value_payload_literal`
    over `lower_enum_payload` — the typed single view; a reloc lands in the typed field).
  The `EnumTag::View` (inline fieldless `Direct`) and `EnumTag::None` (`Single { scalar:
  None }`) arms are **unchanged** (they already lower correctly).
- **`enum_from_bytes` / `reconstruct_inline_enum_from_bytes`:** for an **inline** enum,
  route a **no-relocation** byte image to `(struct E){ .<raw> = raw_byte_image(bytes) }`
  (uniform with multi-view; closes #3-via-bytes + nested-bool-via-bytes); a byte image
  carrying a `Byte::Reloc` keeps `reconstruct_inline_variant_fields` (the typed single
  view; `_raw` cannot hold the reloc). Remove the #3 bool-niche bytes carve-out
  (`:2756`).
- **Remove `niche_coinciding_field`** (`:3577`) once it is orphaned (surgical: only if no
  other caller remains).

This is **soundness-critical** (a wrong `_raw` byte image silently miscompiles a
`static`/const) — the byte writes are the same bounds-checked `serialize_const_into` /
`write_bits_into` d2.3a hardened; the mandatory post-close soundness pass targets it.

### Change C: M1 — the `_c<idx>` const-only typed full-object view

For a **multi-view `Direct`** enum (one with a `_tag` view), add per **payload-bearing**
variant `idx` a union member `struct { <tag>; <payload fields…>; } _c<idx>;` whose fields
sit at their **absolute** enum-object offsets:

- In `build_views` (or a parallel `build_const_views`), for each variant whose `_v<idx>`
  view exists, build a `_c<idx>` view = the tag `ViewField` (name collision-proofed
  against the payload field names — seed a `raw_member_name`-style reservation with the
  payload names; the tag is typically `tag`, falling back to `tag0`/… on a clash) +ﾠthe
  variant's payload `ViewField`s (the **same** names/types/offsets `_v<idx>` uses).
- In `emit_enum`'s `union_wrap` sub-case, emit each `_c<idx>` as `struct { … } _c<idx>;`
  via `emit_offset_fields` (member_prefix `u._c<idx>.`), after the `_v<idx>` views and
  before `_raw`. The `offsetof` guards assert each field's absolute offset.
- Add `enum_variant_typed_view(model, enum_ty, variant_idx) -> Option<TypedView>` (the
  read-only navigator the body side spells `u._c<idx>.<tag>` + `u._c<idx>.<field>`
  through — never re-derived), mirroring `enum_variant_field`.

**Scope:** `_c<idx>` is emitted for every payload-bearing `Direct` variant (uniform,
const-init-only — a small text-test re-bless), **not** for niche enums (their reloc
cases are M1-lite (untagged, done) or the defensive guard (niched-tagged)). Layout-
neutral (another full-object-sized union member; sizeof/align unchanged). The runtime
`_tag`/`_v<idx>` views and `member_prefix`/`enum_variant_field`/`enum_tag` are
**unchanged** — `_c<idx>` is written only by Change D.

### Change D: route a multi-view `Direct` reloc payload to `_c<idx>` (#2-reloc)

- **`enum_value_expr` multi-view branch (`:3151-3155`):** before the
  `serialize_multiview_enum_value` (no-reloc) fall-through, add: if the variant is a
  `Direct` payload variant **and** the payload carries a relocation
  (`const_carries_reloc`), lower through the typed full view:
  `(struct E){ .u._c<v> = { .<tag> = <render_tag_value(tv)>, <designators> } }`, where
  the designators are `lower_enum_payload`-style (`const_expr(child, field_ty, at_init)`
  per non-ZST payload child, spelled through the `enum_variant_typed_view` member path).
  A relocation child lowers via `const_expr`'s reference-carrier / reloc arm (→ `&Y` /
  the fat struct), landing in the **typed** field — nested relocs land via nested
  designators. **One** union member (`_c<v>`), no double-init.
- **`enum_from_bytes` (`:2687`):** replace the multi-view reloc fail-loud with the bytes
  analog: determine the active variant `v` by **decoding the `Direct` tag slice**
  (`enum_tag_location`'s offset+width) from the byte image with the existing
  **target-endian** scalar-byte decoder (the d2.3a `scalar_from_bytes`, `:2446`) and
  matching it against each `variant.tag_value` — **fail loud** on a `Byte::Reloc` inside
  the tag slice, a slice shorter than the tag width, or **no** matching `tag_value` (a
  malformed image, never a silent default). Then emit `(struct E){ .u._c<v> = { .<tag> =
  <tv>, <fields> } }` where each payload field is `bytes_to_typed_literal(model, names,
  syms, &bytes[off..off+w], fty, true)` (a `Byte::Reloc` field → the reference carrier →
  `&Y`). A no-reloc byte image still uses `_raw` (d2.3a, unchanged).
- A multi-view **`Direct`** variant remains the **only** reloc routing; a niche
  multi-view enum's reloc is M1-lite (untagged) or the §05 §3 defensive guard
  (niched-tagged — unreachable; fails loud via `serialize_const_into` as today).

### Change E: relabels, doc delta, closure record

- **Niche-tagged-reloc message:** sharpen the fall-through `EmitError::Unsupported`
  (the one a niched-tagged reloc hits via `serialize_const_into` /
  `serialize_enum_value_to_raw`) to name the external rustc invariant (e.g. `"niche enum
  const with a relocation payload in a niched (non-untagged) variant: not expected
  (rustc niche-fills only fieldless variants); a Direct reloc uses the _c<idx> view"`).
  **No new code** — it is the existing reloc reject, renamed.
- **#5 / #6:** unchanged (retained guards; messages already external-invariant /
  bridge-follow-up from d2.3a).
- **Doc delta** (`generic-c-backend.md`, bounded): §12 `EnumValue` row — the reloc
  tagged case uses `_c<idx>`, the inline case uses `_raw`/typed-view; §13 — add the
  inline anonymous-union `_raw` prose (promotion → access unchanged) + the `_c<idx>`
  const-init-only typed full view (runtime navigator unchanged). Reword, don't
  restructure.
- **Closure record:** update `decisions-and-state.md` — the three Phase-1 const
  limitations are **closed** by d2.3b; the const exit gate is gap-free for reachable
  shapes; the one residual is the niche-tagged-reloc defensive guard (external invariant)
  + #5/#6. Update memory [[assessment-soundness-pass]] only on the close (the soundness
  result).

## Non-Goals

- **No d2.1 navigator / runtime access-path change.** The anonymous-union `_raw` relies
  on member promotion precisely so `member_prefix` / `enum_variant_field` / `enum_tag`
  and every runtime read stay identical. `_c<idx>` is **const-init-only** (a new
  read-only navigator; the runtime `_tag`/`_v<idx>` are untouched).
- **No bridge / model / `spec.md` change.** #6's clean fix (a bridge type-aware dedup
  key) stays a separate task; no `Const`/`Byte`/layout/validator change. The
  niche-tagged-reloc stays a defensive guard (no validator rule added).
- **No M2-a, no named-union M2-b.** No niche-cell retype, no nested-designator locator —
  subsumed by the inline `_raw` (§05). `niche_coinciding_field` is removed, not retyped.
- **No new `EmitError` variant** — reuse `Unsupported(&'static str)`; remove the #2-reloc
  / #3 / #4 limitation labels (the shapes are now lowered) and rename the residual
  niche-tagged-reloc reject.
- **No prelude / harness / matrix / ledger / `types.rs` change** beyond what emission
  needs. The const sub-shapes are not distinct matrix rows (the ledger verdict is
  unaffected; const coverage is tracked by the construct row + the fixtures).
- **No layout / C-ABI / endianness / tag-value recomputation** — all bridge certificates.

## Test Plan

### B1: build + workspace green
- `cargo build -p regseq-backends -p codegen-coverage-runner` clean.
- `RUST_TEST_THREADS=1 cargo test -p regseq-backends -p codegen-coverage-runner --
  --test-threads=1` green — every prior test passes (enum text assertions **re-blessed**
  for the anonymous-union wrap + `_c<idx>`; size/align/`offsetof(field)` guards unchanged;
  the #2-reloc/#3/#4 negatives **flipped** to positives).

### B2: end-to-end (compile-**and-run**) — the closures
- **#2-reloc `const`** (`enum E { A(u8), B(&'static u32) }`, `const X: E = E::B(&Y);`):
  `regseq_main` matches B (`Discriminant`+`Downcast`+`Field`) and derefs the pointee
  back (`== Y`), aborting on a wrong tag/value. Proves Change C/D `_c<idx>` (EnumValue).
- **#2-reloc `static`** (the same enum, `static X: E = E::B(&Y);`): same readback. Proves
  Change D's bytes-with-reloc → `_c<idx>` path.
- **#3** (`const N: Option<bool> = None;` and `const S: Option<bool> = Some(true);`):
  match the discriminant (None vs Some) and read the bool back. Proves Change A `_raw` +
  Change B (the niche byte 2 round-trips where `(_Bool){2}` could not).
- **#4 non-bool nested** (an inline niche enum whose niche cell is a non-bool field in a
  nested aggregate): discriminant + untagged payload readback.
- **#4 nested `_Bool`** (the M2-a-impossible case — a `_Bool` niche cell inside a shared
  nested struct): the niched-variant const + the untagged payload (`x`, `b`) read back.
  Proves the `_raw` byte image writes the nested niche byte with no normalization.

### B3: structural
- Change A: a single-view inline enum emits `union { struct { … }; unsigned char
  <raw>[size]; };`; `e.<field>` access is bare (promotion); a variant field literally
  named `_raw` gets a **collision-proof** `<raw>` and `enum_raw_member` returns it;
  size/align/`offsetof(field)` `_Static_assert`s unchanged + `offsetof(<raw>)==0`.
- Change B: an inline niched const (`Option<bool>::None`) emits `(struct E){ .<raw> =
  {2} }`; a nested-bool niche const emits the right byte image; an inline reloc const
  (`Option<&T>::Some(&Y)`) still uses the typed single view (`.<field> = &Y`).
- Change C: a multi-view `Direct` enum emits `struct { <tag>; <fields>; } _c<idx>;` per
  payload variant at absolute offsets, with `offsetof` guards; a payload field named
  `tag` does not clash with the `_c<idx>` tag sub-field.
- Change D: a multi-view `Direct` reloc const emits `(struct E){ .u._c<v> = { .<tag> =
  <tv>, … = &Y } }` (one union member); a no-reloc payload still emits `.u._raw = {…}`.

### B4: residual seams fail loud
- Niche-**tagged**-reloc (hand-built, since unreachable from rustc): a niche enum whose
  *niched* variant carries a relocation payload → `Unsupported(_)` with the external-
  invariant message. **#5** (`Const::Aggregate` over `Ty::Union`) and **#6** (CMS
  incompatible pointee) unchanged. Confirm **no** #2-reloc/#3/#4 limitation label remains
  (the shapes are lowered now).

### B5: determinism
- `emit_generic_c` byte-identical across two calls on a model exercising the inline
  `_raw` (incl. the collision-proof name), the `_c<idx>` reloc consts (both forms), and
  the inline niche consts ([[codegen-ledger-determinism]], [[golden-c-emit-process-determinism]]).

### B6: fmt + clippy + diff scope
- `cargo fmt --all -- --check` clean; `cargo clippy -p regseq-backends -p
  codegen-coverage-runner --all-targets -- -D warnings` clean (scoped per
  [[clippy-all-targets-test-drift]]).
- Diff confined to `crates/regseq-backends/src/generic_c/{enum_emit.rs, layout_emit.rs,
  body_emit.rs}` (+ the in-crate `*_tests.rs`) + the runner behavioral test + the bounded
  §12/§13 `generic-c-backend.md` delta + the `decisions-and-state.md` closure record +
  `tasks.json`. **No** `spec.md` / bridge / model / matrix / ledger / `test-ir-fixtures`
  / harness / `types.rs` change.

## Review

### Files to send to Codex
This spec;
`docs/discussions/generic-c-backend/phase-1/05-d2.3b-mechanism-refinement.md` (+ `04`);
`docs/discussions/regseq-model/generic-c-backend.md` §12/§13/§14;
`crates/regseq-backends/src/generic_c/{enum_emit.rs, layout_emit.rs, body_emit.rs,
types.rs, mod.rs}`; the IR `crates/regseq-model/src/{const_value.rs, layout.rs,
layout_validator.rs, ty.rs, model.rs, target.rs}`;
`crates/codegen-coverage-runner/{tests/generic_c_bodies.rs, src/toolchain.rs}`.

### Reviewer focus
- **Inline `_raw` (Change A).** Anonymous union+struct promotion is gnu11-valid; access
  path + `member_prefix` + every navigator stay identical; the `<raw>` name is
  collision-proof via the **shared** `raw_member_name` and `enum_raw_member` is the sole
  speller (no hardcoded `"_raw"`); layout-neutral (sizeof/align/`offsetof(field)` hold;
  promoted-member `offsetof` works). Does the 1-view wrap change any size/align?
- **Uniform inline const (Change B).** The niched-variant `_raw` write is byte-faithful
  (the niche value at the niche cell + payload; reloc → fail loud); the untagged/Single/
  fieldless arms unchanged; no inline shape regresses; `niche_coinciding_field` truly
  orphaned before removal; the bytes path splits no-reloc(`_raw`)/reloc(typed) correctly.
- **`_c<idx>` (Change C).** Tag + payload at **absolute** offsets; the `tag` sub-field
  name collision-proofed; emitted per payload-bearing `Direct` variant; `offsetof`
  guards correct; layout-neutral; runtime navigator + `_tag`/`_v<idx>` untouched; the new
  `enum_variant_typed_view` reads `build_views` output (not re-derived).
- **Reloc routing (Change D).** Fires only for a multi-view `Direct` variant with a
  relocation; **both** the EnumValue and the Bytes-with-`Byte::Reloc` forms route to
  `_c<idx>`; the bytes path determines the variant from the tag certificate (no
  niche re-derivation); nested relocs land via nested designators; one union member, no
  double-init; a no-reloc payload still uses `_raw`.
- **Scope + soundness.** No d2.1-navigator/runtime/bridge/model change; the niche-tagged-
  reloc stays a defensive fail-loud (external invariant, no validator rule); the bounded
  doc delta; **no off-by-one in any `_raw`/`_c<idx>` offset/stride**; every reloc path
  that cannot be expressed fails loud (never a silent miscompile).
- **Verification honesty.** Compile-and-run proves all three closures (incl. the nested-
  `_Bool` #4 case and both #2-reloc forms); determinism; citations vs HEAD `6d2fe28`;
  scope creep; any reachable shape still wrongly fail-loud or wrongly claimed handled.

### Adversarial prompt template
```
<task>
Review .harness/tasks/regseq-backends/18-generic-c-const-full-closure.md — Phase-1 task
d2.3b of the generic-C backend, PACKAGE A / full closure, implementing the Codex-approved
mechanism refinement phase-1/05-d2.3b-mechanism-refinement.md (which supersedes 04 §5.2's
M2-a/M2-b). It closes the three documented Phase-1 const limitations soundly under
-std=gnu11 -fwrapv -Wall, NO -fno-strict-aliasing. Five changes: A inline enums get an
ANONYMOUS union+struct byte-covering `_raw` (member promotion keeps e.field access
unchanged; collision-proof name via the SHARED raw_member_name; enum_raw_member the sole
speller) — layout-neutral; B the UNIFORM inline const path: a no-reloc inline value (the
#3/#4 niched variant, and any no-reloc inline byte image) serializes to `_raw` exactly as
multi-view, SUBSUMING M2-a (drop niche_coinciding_field + the #3/#4 carve-outs); the
untagged/Single/fieldless + the inline-reloc cases keep their existing typed paths; C M1:
a CONST-INIT-ONLY typed full-object view `_c<idx>` = { tag; payload fields } at ABSOLUTE
offsets per payload-bearing Direct variant of a multi-view enum (tag sub-field name
collision-proofed; offsetof guards; runtime _tag/_v<idx> + navigator UNCHANGED); D route a
multi-view Direct reloc payload to (struct E){ .u._c<v> = { .tag = tv, … = &Y } } — BOTH
the Const::EnumValue and the Const::Bytes-with-Byte::Reloc forms; a no-reloc payload still
uses `_raw`; E sharpen the niche-tagged-reloc message to an external-rustc-invariant
defensive guard (rustc niche-fills only fieldless variants; NO new code) + the bounded
§12/§13 doc delta + the closure record. This is a SPEC review: judge correctness,
completeness, internal consistency, sound scoping, faithful grounding vs HEAD 6d2fe28.
SPECIFICALLY verify: the anonymous-union `_raw` is layout-neutral + access-path-preserving
+ collision-proof; the inline `_raw` const path is byte-faithful and never normalizes
(closing nested-_Bool #4) and fails loud on a reloc; `_c<idx>` is at absolute offsets with
a collision-proof tag and no runtime-navigator change; reloc routing covers BOTH forms,
determines the bytes-form variant from the tag certificate, and never double-inits; the
relabel keeps the niched-tagged-reloc FAIL-LOUD (no silent miscompile); the doc delta is
bounded + truthful; no d2.1/runtime/bridge/model change.
Identify: any silent-miscompile path; any reloc/niche sub-case missed or wrongly handled;
any reachable shape still fail-loud; drifted citations; scope creep; any inline shape that
regresses from the uniform-`_raw` rewrite.
</task>
<grounding_rules>
- Cite file:line for every claim. Read the files; don't speculate. Mark hypotheses.
- The model is post-mono + rustc-free; layout/tag_value/offset/endian are bridge
  certificates (the backend does not recompute them).
</grounding_rules>
<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
PER-CHANGE (A/B/C/D/E): PASS/FAIL + citation
SOUNDNESS: inline `_raw` layout-neutral + access-preserving; const `_raw` byte-faithful /
  no normalization / reloc fail-loud; `_c<idx>` absolute offsets + collision-proof + no
  navigator change; reloc routing both forms + no double-init; relabel stays fail-loud —
  PASS/FAIL + cite; any silent miscompile? YES/NO + cite
NEW ISSUES: numbered or "none"
REQUIRED FIXES: numbered (only if VERDICT != APPROVED)
</structured_output_contract>
<default_follow_through_policy>
Do not ask clarifying questions. Read the files, decide, stop.
</default_follow_through_policy>
```

## Quality Gate
- Build + fmt + clippy clean (scoped, [[clippy-all-targets-test-drift]]).
- All backend unit + runner compile-AND-RUN tests green; the B2 fixtures run + byte-exact
  (both #2-reloc forms, #3, both #4 cases incl. nested `_Bool`); the B4 niche-tagged-reloc
  + #5 + #6 negatives fail loud.
- `emit_generic_c` byte-stable (B5); Mode A ledger byte-stable + verdict unchanged
  (`not-pass` — const sub-shapes are not distinct matrix rows).
- Diff confined to the Change-A–E set (B6); no #2-reloc/#3/#4 limitation label remains.
- The mandatory **post-close Codex soundness pass** ([[assessment-soundness-pass]], 7/7
  streak) on the inline `_raw` const path + the `_c<idx>` view + the reloc routing (both
  forms) — the byte-image / union-pun surfaces it has caught latent miscompiles on.

## Acceptance Criteria
1. A single-view inline enum carries a byte-covering `_raw` via an anonymous union+struct;
   `e.<field>` access is **unchanged** (promotion); the `<raw>` name is collision-proof
   (a variant field named `_raw` does not clash) and spelled solely through
   `enum_raw_member`; sizeof/align/`offsetof(field)` `_Static_assert`s unchanged.
2. **#3** closed: `Option<bool> = None` (and `Some(true)`) lowers via `_raw`, compiles,
   and the discriminant + bool read back correctly.
3. **#4** closed for **both** a non-bool nested niche cell **and** a nested `_Bool` niche
   cell (the M2-a-impossible case) — lowers via `_raw`, compiles, reads back.
4. **#2-reloc** closed: a multi-view `Direct` enum const holding a `&'static u32` lowers
   through `_c<idx>` in **both** the `const` (`Const::EnumValue`) and `static`
   (`Const::Bytes`+`Byte::Reloc`) forms, compiles, and the pointee reads back; the reloc
   sits in a typed pointer slot, one union member, no double-init; a no-reloc payload
   still uses `_raw`.
5. `_c<idx>` is emitted per payload-bearing `Direct` variant at absolute offsets with a
   collision-proof `tag` sub-field + `offsetof` guards; the runtime `_tag`/`_v<idx>`
   views + navigator + `member_prefix` are unchanged.
6. The niche-**tagged**-reloc residual is a fail-loud **defensive guard** with the
   external-rustc-invariant message; #5/#6 unchanged; **no #2-reloc/#3/#4 limitation
   label remains** (the shapes are lowered).
7. The bounded `generic-c-backend.md` §12/§13 delta documents the inline `_raw` + the
   `_c<idx>` view; `decisions-and-state.md` records the closure + the one residual guard;
   no other doc/spec change. M2-a is not implemented (`niche_coinciding_field` removed).
8. Build/fmt/clippy clean; all unit + compile-AND-RUN tests green; `emit_generic_c`
   byte-stable; Mode A ledger byte-stable, verdict `not-pass`; diff scope per B6; the
   post-close soundness pass is SOUND.
