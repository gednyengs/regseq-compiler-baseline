# regseq-backends-17: Generic-C non-scalar-const carve-outs — the union `_raw` member, the recursive no-reloc enum-payload serializer, the niche-untagged typed-view route, + carve-out relabels (Phase-1 task d2.3a)

## Description

Close the **cheap, low-risk** subset of the six `task d2.3` non-scalar-`Const`
carve-outs d2.2 (`regseq-backends-14`, `0347d92`) left fail-loud — **without
perturbing the locked D9 enum representation or the d2.1 navigator**, and lower
every reachable shape **soundly** under the pinned `cc` flags (`-std=gnu11 -fwrapv
-Wall`, **no** `-fno-strict-aliasing`, `toolchain.rs:79-84`). This is **task d2.3a**
of the Phase-1 a→b→c1→c2→d1→d2→d2.1→d2.2→e→`-16`→**d2.3a** sequence; it is the
disposition of the **plan-first design doc**
`docs/discussions/generic-c-backend/phase-1/04-const-carve-out-resolution.md`
(Codex-reviewed, 3 rounds → APPROVED), **Package B**.

The design doc triaged the carve-outs (it is **six**, not five — the planning
record omitted `inline niche cell not a top-level field`) and split the work by
whether it perturbs the **Codex-hardened D9 enum representation / d2.1 navigator**.
This task implements the **no-D9-change** wins; the three rare representation-change
shapes (**#2-reloc**, **#3** inline bool-niche, **#4** inline nested-niche) stay
**fail-loud as sound, explicitly-documented Phase-1 limitations** (a follow-on
**d2.3b** under Package A would close them via the `_c<idx>` full view + niche-cell
retype — out of scope here).

After d2.3a, `emit_generic_c` lowers, compiles, and runs every non-`dyn`
`RegseqModel` the bridge produces **except** the three documented limitations and
the unreachable / vanishingly-rare guards (#5, #6).

### What ships (the five changes)

- **Change A — the union `_raw` member (#1, representation: a layout-neutral union
  extension).** Add a byte-covering, **collision-proof** `unsigned char <raw>[size]`
  member to every emitted C `union`, spelled through a single source of truth.
- **Change B — `Const::Bytes` over `Ty::Union` (#1).** Lower a union byte image as
  `(union U){ .<raw> = { b0,…,bN-1 } }`; a `Byte::Reloc` in a union image stays
  fail-loud (relabelled to #2-reloc).
- **Change C — the recursive no-reloc enum-payload serializer (#2-noreloc).**
  Upgrade `serialize_multiview_enum_value`'s payload write from scalar-only
  (`const_scalar_bits`) to a recursive `serialize_const_into` that byte-serializes
  any **no-relocation** payload child (scalar / `NullPtr` / `Bytes` / `Aggregate` /
  nested `EnumValue`) into the `_raw` image; **any relocation → fail-loud
  (#2-reloc).**
- **Change D — the niche-untagged typed-view route (M1-lite).** A **multi-view
  niche-untagged** enum value (no `tag_value` to co-write) routes through the
  **existing** `u._v<untagged>` typed view (reusing `lower_enum_payload` +
  `enum_value_payload_literal`), so its payload — **including a relocation** —
  lowers via a single typed designator, no `_raw`, no new member.
- **Change E — relabel #5 (unreachable → defensive) + sharpen #6.** Plus the
  bounded `generic-c-backend.md` §12/§13/§14 doc delta and the three-limitation
  record.

### Stays fail-loud — three Phase-1 limitations + two guards (NOT this task's to fix)

These remain a sound `EmitError::Unsupported` (hard error, **never** a silent
miscompile). The labels are **rewritten** to read as documented Phase-1 limitations
(not bare `task d2.3` deferrals), and the three limitations are **recorded** (Change
E):

- **#2-reloc** — a **multi-view tagged** enum value (a `Direct` payload variant, or
  a niched variant) whose payload carries a **relocation** (`Reloc` / a reference
  carrier / a `Byte::Reloc`, possibly nested): `_raw` (a `char[]`) cannot encode an
  address constant, and the union-of-views cannot co-init `_tag` + `_v<v>` in one
  initializer. Needs the `_c<idx>` typed full view (d2.3b). Label: `"Const enum
  value with a relocation payload: needs the typed full view (Phase-1 limitation;
  task d2.3b)"`.
- **#3 — inline bool-niche** (`const X: Option<bool> = None;`): an **inline** enum
  has no `union`/`_raw`, and the niche cell's logical C type `_Bool` normalizes
  `(_Bool){2}` → 1. Needs the niche-cell retype (d2.3b). Label: `"Const::EnumValue
  (inline bool-niche): needs niche-cell retype (Phase-1 limitation; task d2.3b)"`.
- **#4 — inline niche cell not a top-level field**: the niche is buried in a nested
  aggregate field, so no single top-level designator writes it; an inline enum has
  no `_raw`. Needs the nested-designator locator + retype (d2.3b). Label:
  `"Const::EnumValue (inline niche cell not a top-level field): needs nested
  designator (Phase-1 limitation; task d2.3b)"`.
- **#5 — `Const::Aggregate` over `Ty::Union`** (Change E): **unreachable** — a union
  const reaches the backend as `Const::Bytes` (the `translate_static_init` byte
  path), never as `Const::Aggregate` (the valtree path), because rustc does not
  represent a union const as a valtree; `pretty.rs:714-718` rejects it too. Keep the
  **defensive** guard (the unreachability is an external rustc invariant, not
  repo-proven); relabel away from `task d2.3`. Label: `"Const::Aggregate over a
  union: not expected (rustc emits no union valtree; unions arrive as Const::Bytes)"`.
- **#6 — `ConstMemoryStatic` at incompatible pointee types** (Change E): the
  content-hash dedup collision (`const_.rs:145-152`). **Vanishingly rare**; the
  fail-loud is sound. The clean fix is a **bridge** type-aware dedup key (a separate
  optional task), **not** a backend split. Sharpen the label to note the root cause.
  Label: `"ConstMemoryStatic deduped at incompatible pointee types (bridge
  content-hash dedup; sound fail-loud — see task d2.3-bridge-dedup)"`.

## Input Context

### Design source — the plan-first doc + the canonical spec

- **`docs/discussions/generic-c-backend/phase-1/04-const-carve-out-resolution.md`**
  — the resolution this task implements (Package B; §4 R1 = the union `_raw`; §5.1
  the reloc-presence #2 split; §5.2 M1-lite; §2/§3 the #5/#6 dispositions). **Read
  it first.**
- **`docs/discussions/regseq-model/generic-c-backend.md`**:
  - **§12 `Const` → C initializers** (lines 928-946): the `Bytes` row (944) +
    `Aggregate` row (945) + `EnumValue` row (946). Change A/B/C reconcile the union
    rows (the union byte image is no longer "unsupported"); the **no-reloc structured
    enum payload** now serializes into `_raw`; a **reloc payload** is a documented
    Phase-1 limitation.
  - **§13 Enum representation** (lines 958-1034, esp. the enum `_raw` member
    985-994): the union-of-views form. **Unchanged** — Change C reuses the existing
    enum `_raw`; Change D reuses the existing `u._v<v>` view. No new enum member, no
    `build_views` change.
  - **§14/§15 aggregate emission** (lines 1044-1096, esp. **§ Unions** ~1062):
    document the new byte-covering union `_raw` member (mirroring the §13 enum
    `_raw` prose).

### Code anchors (confirm at HEAD `cedf528`)

- `crates/regseq-backends/src/generic_c/layout_emit.rs` — the union body emitter:
  - `emit_union_body` (`:278-310`) emits each non-ZST union field at offset 0 + a
    trailing `unsigned char <pad>[size]` filler (via `PadNamer`) **only when** no
    real member spans `layout.size`. **Change A** replaces the conditional `_pad`
    filler with an **always-emitted, collision-proof** byte-covering member spelled
    through the new shared `union_raw_member` (so it both pins `sizeof` and is
    const-init-addressable).
  - `PadNamer` (`:11-49`) — the collision-proofing primitive (seed reserved set with
    the real sanitized member names; skip taken names). Change A's name generator
    reuses this discipline (a `_raw`-seeded namer), since "a field literally named
    `_raw` is a legal sanitized name."
  - `resolve_view` (`:86-135`) / `emit_aggregate` (`:174-219`) — the union/struct
    dispatch; `is_zst` (`:139`).
- `crates/regseq-backends/src/generic_c/types.rs`:
  - `declare(model, names, ty, decl)` (`:76`) — used to spell the union members; the
    `_raw` member is `unsigned char <name>[size]` (spelled directly, no `Ty`).
- `crates/regseq-backends/src/generic_c/body_emit.rs` — the const seam:
  - `bytes_to_typed_literal` (`:2408-2433`) — the byte-image walker; the `Ty::Union`
    arm (`:2427`) is **Change B** (currently `Unsupported("Const union byte-image:
    task d2.3")`).
  - `raw_byte_image(bytes)` (`:2668-2684`) — renders an `unsigned char[]` initializer
    `{ b0,…,bN-1 }` (`Uninit`→0; `Reloc`→ #2-reloc). **Reused** by Change B.
  - `enum_value_expr` (`:2994-3083`) — the value-position enum const dispatch; the
    multi-view branch (`:3056-3082`) is where **Change D** (M1-lite) inserts the
    niche-untagged route before `serialize_multiview_enum_value`.
  - `serialize_multiview_enum_value` (`:3093-3142`) — **Change C**: the per-payload
    write at `:3117-3131` (currently `const_scalar_bits` + `write_bits_into`) becomes
    a call to the new recursive `serialize_const_into`. The tag write (`:3110-3115`)
    is unchanged.
  - `const_scalar_bits` (`:3186-3193`) — scalar-only (`None` for `Reloc`/`NullPtr`/
    `Aggregate`/`Bytes`/`EnumValue`/ZST); **reused** by `serialize_const_into`'s
    scalar arm. `write_bits_into` (`:3199-3220`) — the endian byte writer; reused.
  - `lower_enum_payload` (`:3227-3252`) — lowers each payload child via `const_expr`
    at the `enum_variant_field` member path (`u._v<v>.<field>` multi-view); **reused
    verbatim** by Change D. `enum_value_payload_literal` (`:3280-3286`) — wraps the
    designators as `(struct E){ … }`; reused by Change D.
  - `enum_tag_location` (`:3144-3162`) — the (offset, width) of the tag cell; reused
    by Change C's tag write (unchanged) and `serialize_const_into`'s `EnumValue` arm.
  - `typed_valtree_aggregate` (`:2898-2972`) — the `Ty::Union` arm (`:2963`) is
    **#5** (Change E relabel). `aggregate_from_bytes` (`:2524-2559`) /
    `array_from_bytes` (`:2595-2634`) — Change C's `serialize_const_into` mirrors
    their `FieldLayout`-source-order / element-stride walks, **in reverse** (const →
    bytes).
- `crates/regseq-backends/src/generic_c/mod.rs`:
  - `record` (`:1410-1435`) — the CMS re-typing collision (`:1426-1429`) is **#6**
    (Change E label sharpen).
- `crates/regseq-backends/src/generic_c/enum_emit.rs` — read-only navigator:
  - `enum_shape` (`:289`), `enum_is_inline` (`:302`), `enum_variant_field` (`:324`),
    `enum_tag` (`:345`), `enum_raw_member` (`:312`, the enum `u._raw` path — **the
    template for Change A's `union_raw_member`**). **No change** (Change C/D reuse).
- IR (read-only — bridge certificates):
  - `crates/regseq-model/src/const_value.rs` — `Const::{Bytes(Vec<Byte>),
    Aggregate(Vec<ConstId>), EnumValue{variant_idx, payload}, NullPtr(TyId),
    Reloc(InstanceRef), Bool, Char, Int, Uint, Float}`; `Byte::{Uninit, Value(u8),
    Reloc}`; `:38-39` (no active `FieldIdx`).
  - `crates/regseq-model/src/target.rs` — `TargetInfo.endian`; `model.size_align_of`
    (`model.rs:1831`); `model.types`, `model.consts.resolve`.
  - `crates/regseq-model/src/{ty.rs, layout.rs}` — `Ty::{Union, Tuple, Struct,
    Closure, Array, Enum, Ptr, Ref, FnPtr, …}`; `FieldLayout{name, ty, offset}`;
    `LayoutKind::{Struct, Union, Enum}`; `VariantLayout`; `TagScalar`; `TagEncoding`.

### Prior tasks

- **d2.2** (`regseq-backends-14`, `0347d92`) — lowered all non-scalar `Const` except
  the six carve-outs; added the **enum** `_raw` member (Change A there) +
  `bytes_to_typed_literal` + `serialize_multiview_enum_value` + the CMS re-typing
  pre-pass. d2.3a reuses all of it; the **union** `_raw` (Change A here) mirrors the
  enum one.
- **d2.1** (`regseq-backends-13`, `c8cadaa`) — the enum navigator + the niche
  raw-storage discriminant read (`body_emit.rs:754`/`:816`, raw memcpy — **so Change
  D's typed-view route does not touch the discriminant read path**).
- **`-16`** (`dad9a1e`+`3f2a172`) — the diag-log `fwrite` redesign + the
  `fresh_temp_name` discipline (unrelated; the backend's most recent state).

### Test surface

- `crates/codegen-coverage-runner/tests/generic_c_bodies.rs` — the compile-**and-run**
  harness. d2.3a adds: a **union static** read back through a member (#1); a
  **multi-view `Direct` enum const with a no-reloc aggregate payload** (#2-noreloc,
  e.g. `enum E{A,B([u32;2])}`, read both elements back); a **multi-view
  niche-untagged reloc payload** (M1-lite — a hand-built niche enum whose untagged
  variant holds a `&STATIC`, read the pointee back).
- `crates/regseq-backends/src/generic_c/mod_tests.rs` — hand-built structural tests.
  d2.3a **flips** the current #1 / #2-noreloc negatives to positives, **updates** the
  five existing d2.3 negative tests (`mod_tests.rs:4220/4251/4637/4666/4694/4743`)
  to the **rewritten labels** (still fail-loud, new message text), and adds the
  union-`_raw` structural + collision-proofing tests. The union-emit text-assertion
  tests (size/align unchanged) update for the new `_raw` member.

## Implementation Notes

### Change A: the union `_raw` member + the `union_raw_member` source of truth

In `layout_emit.rs`, add `pub(super) fn union_raw_member(model, names, ty) ->
Option<String>` (the single source of truth for the union `_raw` member name,
mirroring `enum_emit::enum_raw_member`): for a `Ty::Union`, compute a
**collision-proof** name by seeding a `PadNamer`-style reserved set with the union's
sanitized field names and yielding the first free `_raw` / `_raw<n>`; `None` for a
non-union. Deterministic (the field-name set + a fixed `_raw` seed order).

In `emit_union_body` (`:278`), **always** emit `    unsigned char <raw>[<size>];`
(the `union_raw_member` name) as the union's byte-covering member, **replacing** the
conditional trailing `_pad[size]` filler (the `_raw` member already spans
`layout.size`, so it pins `sizeof`; the separate `_pad` filler is now redundant).
Its `offsetof` guard asserts `0` (every union member is at offset 0), like the real
members. Layout-neutral: alignment 1; the union's `aligned` + sizeof/`_Alignof`
`_Static_assert`s are unchanged.

**Soundness:** initializing `.<raw>` then reading a typed member is gnu11 union
type-punning — **sound** under the pinned flags (the read is through the union
object, never an inactive-member-address-then-typed-deref), identical to the enum
`_raw` argument d2.2 validated. The **collision-proof name is mandatory**: a union's
`_raw` is a sibling of the user's sanitized field names (a Rust union field named
`_raw` would clash and silently misdirect the init) — the same hazard `PadNamer`
already guards `_pad` against.

### Change B: `Const::Bytes` over `Ty::Union`

Replace the `bytes_to_typed_literal` `Ty::Union` arm (`:2427`) with: spell
`(union <U>){ .<raw> = <raw_byte_image(bytes)> }`, where `<raw>` is
`union_raw_member(model, names, ty)` (fail loud if `None` — a non-union reached this
arm) and `raw_byte_image` (reused, `:2670`) renders the `{ b0,… }` image and **defers
on a `Byte::Reloc`** (relabelled #2-reloc — a union holding a reference). The
compound-literal form is valid at both value and static-init positions (a union, like
a struct, can initialize from a compound literal), so **no `at_init` special-casing**
(matches `aggregate_from_bytes`). `<U>` is `names.agg_name(ty)`.

### Change C: the recursive no-reloc enum-payload serializer (`serialize_const_into`)

Add `fn serialize_const_into(model, image: &mut [u8], offset: usize, c: &Const, ty:
TyId, endian) -> Result<(), EmitError>` — the **const → byte-image** walker (the
inverse of `bytes_to_typed_literal`), purely positional + layout-driven:

- **scalar** (`Bool`/`Char`/`Int`/`Uint`/`Float`) → `const_scalar_bits(c)` +
  `write_bits_into(image, offset, size_of(ty), bits, endian)`.
- **`NullPtr`** → `write_bits_into(image, offset, size_of(ty), 0, endian)` (a null
  pointer is the zero bit pattern).
- **`Bytes(bytes)`** → write `bytes[i]` at `offset+i`: `Value(v)`→`v`, `Uninit`→`0`,
  **`Reloc` → fail loud (#2-reloc)**; bounds-checked against `size_of(ty)`.
- **`Aggregate(children)`** over `Tuple`/`Struct`/`Closure` → for each field `i` in
  **`FieldLayout` source order**, `serialize_const_into(child_i, FieldLayout[i].ty)`
  at `offset + FieldLayout[i].offset` (ZST fields: size 0, no write — but still
  recursed so a deferred shape fails loud). Over `Array(elem, n)` → each child at
  `offset + i*size_of(elem)`. Over `Ty::Ref`/`Ty::Union` → fail loud (a `Ref`
  carrier is a reloc → #2-reloc; a `Const::Aggregate` over a union is #5 — defensive).
- **`EnumValue { variant_idx, payload }`** (a nested enum payload) → recurse: write
  the nested enum's `tag_value` (when `Some`) at its `enum_tag_location` over the tag
  width, then each non-ZST payload field via `serialize_const_into` at its
  `FieldLayout.offset` (all `+ offset`). (Inline / multi-view is irrelevant — the
  byte image is representation-independent.)
- **`Reloc`** → fail loud (#2-reloc).

Then `serialize_multiview_enum_value` (`:3093`) keeps its tag write (`:3110-3115`)
and replaces the **payload loop** (`:3117-3131`) with, per non-ZST field `i`,
`serialize_const_into(model, &mut image, field.offset as usize, child, field.ty,
endian)`. The `_raw` wrap (`:3133-3141`) is unchanged. **This is the soundness-
critical surface** (an off-by-one in an offset/stride silently miscompiles a `static`
/ const) — the mandatory post-close Codex soundness pass targets it.

### Change D: the niche-untagged typed-view route (M1-lite)

In `enum_value_expr`'s **multi-view** branch (`:3056-3082`), **before** the
`serialize_multiview_enum_value` fall-through, add: if `shape` is niche-encoded and
`v` is the **untagged** variant (`variant_layout(shape, v).tag_value == None` — there
is **no tag to co-write**), route through the **existing typed view**:
`enum_value_payload_literal(&agg, &lower_enum_payload(model, names, syms, enum_ty, v,
payload)?)`. `lower_enum_payload` already spells `u._v<untagged>.<field>` (multi-view
`enum_variant_field` prefix) and lowers each child via `const_expr` — so a
**relocation** payload lowers to `.u._v<v>.<field> = &Y` (one union member, no
double-init, an address constant in a typed pointer field), and a scalar/aggregate
payload lowers through its normal `const_expr` arm. No `_raw`, no new member.

(The **niched** untagged-vs-tagged distinction is exact: only the niche
**untagged** variant has `tag_value == None` and so needs no tag co-write; a niched
**tagged** variant or a `Direct` payload variant still serializes to `_raw` via
Change C, deferring to #2-reloc on a relocation.)

### Change E: relabel #5, sharpen #6, the doc delta + the limitation record

- **#5** (`typed_valtree_aggregate` `Ty::Union` arm, `:2963`): rewrite the message
  to the defensive-guard text (§ Description). Keep the arm (do **not** delete — the
  unreachability is an external rustc invariant).
- **#6** (`mod.rs` `record`, `:1426-1429`): rewrite the message to name the bridge
  content-hash dedup root cause + the d2.3-bridge-dedup follow-up.
- **#2-reloc / #3 / #4**: rewrite the three labels (`body_emit.rs` `:2655`/`:2678`/
  `:3127` reloc; `:2721`/`:3043` bool-niche; `:3047` nested-niche) to the Phase-1-
  limitation text (§ Description). Still `EmitError::Unsupported` (fail-loud).
- **Doc delta** (`generic-c-backend.md`, bounded): §12 `Bytes` row (944) — the
  `Ty::Union` byte image now initializes the union `_raw` member (no-reloc; reloc =
  a Phase-1 limitation); §12 `EnumValue` row (946) — a no-reloc structured payload
  serializes into `_raw`, a niche-untagged value uses the `u._v<untagged>` view, a
  reloc payload is a Phase-1 limitation; **§13 the `Const::EnumValue` serialization
  prose (1028-1032)** — qualify it: a multi-view value is **usually** serialized to
  `_raw`, **except** a multi-view **niche-untagged** value, which may initialize the
  existing `u._v<untagged>` typed view (no `_tag` member is co-written), and a
  multi-view value with a **relocation** payload is a Phase-1 limitation (the
  **representation itself is unchanged** — no new member); §14 § Unions (~1062) —
  document the byte-covering union `_raw` member (mirror the §13 enum `_raw` prose).
  Keep it minimal; reword rows/prose, do not restructure.
- **Limitation record**: add the **three** Phase-1 const limitations (#2-reloc, #3,
  #4) to `docs/discussions/generic-c-backend/decisions-and-state.md` (the d2.3a close
  entry) so the gap is honest (the matrix tracks constructs, not const sub-shapes).

## Non-Goals

- **No D9 enum representation change, no d2.1 navigator change.** No `_c<idx>` full
  view (M1), no inline-niche `_raw`, no niche-cell retype (M2-a) — those are **d2.3b**
  (Package A). `build_views`, `member_prefix`, `enum_variant_field`, `enum_tag`,
  `enum_raw_member` are **read**, not modified.
- **No bridge / model change.** #6's clean fix (a type-aware dedup key) is a bridge
  task, explicitly deferred; the backend stays fail-loud. No `Const`/`Byte`/layout
  change.
- **The three limitations stay fail-loud.** #2-reloc, #3, #4 are **not** implemented
  here; they are relabelled + recorded, not closed.
- **No new `EmitError` variant** — reuse `Unsupported(&'static str)`; remove every
  bare `task d2.3` label (rewrite to the limitation / defensive / bridge-follow-up
  text).
- **No `types.rs` change beyond using `declare`** for the union members; the union
  `_raw` is spelled `unsigned char <name>[size]` directly (no `Ty` interning, no
  discovery/topo edge — `unsigned char[N]` needs no type definition).
- **No prelude / `regseq_main` / harness / matrix / ledger change.** (The ledger
  is regenerated by the runner; d2.3a adds no new green matrix row beyond what the
  new fixtures cover — and the const sub-shapes are not distinct matrix rows.)
- **No layout / C-ABI / endianness recomputation** — field offsets, tag values,
  scalar/pointer widths, the array length, `target.endian` are bridge certificates.

## Test Plan

### B1: build + workspace green
- `cargo build -p regseq-backends -p codegen-coverage-runner` clean.
- `RUST_TEST_THREADS=1 cargo test -p regseq-backends -p codegen-coverage-runner --
  --test-threads=1` green — every d1/c1/c2/d2/d2.1/d2.2/e/`-16` test passes (the
  union-emit text assertions **updated** for the `_raw` member; size/align unchanged;
  the five d2.3 negatives **updated** to the new labels).

### B2: end-to-end (compile-**and-run**)
- **Union static** (#1): a `repr(C)` (or Rust) union, `static U: MyUnion = …;`,
  read back through a member; proves Change A `_raw` + Change B byte image (the
  member type-puns the `_raw` bytes). Include a union whose largest member does
  **not** span the full size (so the old code path needed the `_pad` filler) to
  prove `_raw` pins `sizeof` correctly.
- **Multi-view `Direct` enum const, no-reloc aggregate payload** (#2-noreloc):
  `enum E { A, B([u32; 2]) }`, `const X: E = E::B([10, 20]);` (or the bridgeable
  shape); `regseq_main` matches it (`Discriminant` + `Downcast`+`Field`) and reads
  both array elements back, aborting on a wrong tag / element. Proves
  `serialize_const_into` recurses an array payload into `_raw`.
- **Multi-view niche-untagged reloc payload** (M1-lite, hand-built): a niche enum
  whose untagged variant holds a `&'static u32` and which is multi-view (a second
  variant has a payload view); a value `E::Untagged(&Y)`; read the pointee back.
  Proves Change D routes the reloc through `u._v<untagged>.<field>`.

### B3: structural
- Change A: an emitted `union` contains `unsigned char <raw>[<size>];`; a union with
  a real field named `_raw` gets a **collision-proof** name (`_raw0`/…) and the
  `union_raw_member` lookup returns it (the init designator matches the emitted
  member). The size/align `_Static_assert`s are unchanged.
- Change B: a union byte image lowers to `(union U){ .<raw> = { … } }` (the same
  `<raw>` name `union_raw_member` yields).
- Change C: a multi-view `Direct` enum const with a 2-element array payload serializes
  the elements at the right offsets into `_raw` (assert the byte image); a nested
  `EnumValue` payload serializes the nested tag + fields.
- Change D: a multi-view niche-untagged enum const emits `(struct E){ .u._v<v>.<f> =
  … }` (the typed view), **not** a `_raw` byte image.

### B4: deferred seams fail loud (the three limitations + two guards)
- **#2-reloc**: a multi-view `Direct` enum const with a **pointer** payload (`enum
  E{A,B(&'static u32)}`, `const X=E::B(&Y)`) → `Unsupported(_)` "relocation payload …
  task d2.3b"; a union byte image with a `Byte::Reloc` → same.
- **#3**: `const X: Option<bool> = None;` → `Unsupported(_)` "inline bool-niche …
  task d2.3b".
- **#4**: an inline niche enum whose niche cell is in a nested aggregate →
  `Unsupported(_)` "inline niche cell not a top-level field … task d2.3b".
- **#5**: a hand-built `Const::Aggregate` consumed by `Ty::Union` → `Unsupported(_)`
  "not expected (rustc emits no union valtree …)".
- **#6**: a CMS referenced at two incompatible pointee types → `Unsupported(_)`
  "incompatible pointee types (bridge content-hash dedup …)".
- Confirm **no bare `task d2.3` label** remains (all rewritten); the vtable-task
  labels are untouched.

### B5: determinism
- `emit_generic_c` byte-identical across two calls on a model with union statics
  (`_raw` byte order + the collision-proof name), multi-view aggregate-payload enum
  consts (serialized `_raw`), and niche-untagged reloc consts ([[codegen-ledger-determinism]]).

### B6: fmt + clippy + diff scope
- `cargo fmt --all -- --check` clean; `cargo clippy -p regseq-backends -p
  codegen-coverage-runner --all-targets -- -D warnings` clean (scoped per
  [[clippy-all-targets-test-drift]]).
- Diff confined to `crates/regseq-backends/src/generic_c/{layout_emit.rs, body_emit.rs,
  mod.rs}` (+ `mod_tests.rs`) + the runner behavioral test + the **bounded §12/§13/§14
  `generic-c-backend.md` delta** + the `decisions-and-state.md` limitation record +
  the `tasks.json` registration. **No** `spec.md` / bridge / model / matrix / ledger /
  `test-ir-fixtures` / harness / `types.rs` / `enum_emit.rs` change.

## Review

### Files to send to Codex
This spec;
`docs/discussions/generic-c-backend/phase-1/04-const-carve-out-resolution.md`;
`docs/discussions/regseq-model/generic-c-backend.md` §12/§13/§14;
`crates/regseq-backends/src/generic_c/{layout_emit.rs, body_emit.rs, enum_emit.rs,
mod.rs, types.rs}`; the IR `crates/regseq-model/src/{const_value.rs, layout.rs, ty.rs,
target.rs, model.rs}`; `crates/codegen-coverage-runner/{tests/generic_c_bodies.rs,
src/toolchain.rs}`.

### Reviewer focus
- **Union `_raw` (Change A).** Always-emitted, byte-covering, **collision-proof**
  name via a single source of truth (`union_raw_member`); subsumes the `_pad` filler;
  layout-neutral (sizeof/align asserts hold); gnu11 type-pun sound under the pinned
  flags. Does dropping the conditional `_pad` ever change a union's size/align?
- **`serialize_const_into` (Change C).** Every arm correct; **endianness from
  `target.endian`**; aggregate slicing via `FieldLayout.offset` + `size_of` in
  **source order**, ZST recursed-but-zero-width; `NullPtr` → zero; **`Reloc` /
  `Byte::Reloc` anywhere → fail loud (#2-reloc)**; the nested `EnumValue` tag write
  reads `enum_tag_location` / `tag_value` as certificates (no niche re-derivation).
  **No off-by-one in any offset/stride** (soundness-pass critical).
- **M1-lite (Change D).** Fires **only** for a multi-view niche **untagged** variant
  (`tag_value == None`); reuses `lower_enum_payload` (so the member path is
  `enum_variant_field`'s, not re-derived); a reloc payload lands in a typed pointer
  field; a `Direct` / niched-tagged variant still serializes to `_raw` (Change C) and
  defers to #2-reloc on a relocation. No double-init.
- **Scope discipline.** No D9 / d2.1-navigator / `enum_emit.rs` / bridge / model
  change; the three limitations (#2-reloc, #3, #4) stay fail-loud + are recorded; #5
  guard retained; #6 not split; only the bounded §12/§13/§14 doc delta.
- **Verification honesty.** Compile-and-run over the union static + the no-reloc
  aggregate-payload enum const + the niche-untagged reloc const; the fail-loud
  negatives prove every limitation/guard; determinism; no bare `task d2.3` label
  left; citation drift vs HEAD `cedf528`; scope creep.
- **Did d2.3a leave any silent-miscompile path?** (a wrong `serialize_const_into`
  offset; a union `_raw` that mis-sizes; an M1-lite route that drops the niche
  encoding; a relabel that turned a fail-loud into a fall-through).

### Adversarial prompt template
```
<task>
Review .harness/tasks/regseq-backends/17-generic-c-const-carve-outs.md — Phase-1 task
d2.3a of the generic-C backend, implementing PACKAGE B of the Codex-approved design
doc phase-1/04-const-carve-out-resolution.md. It closes the CHEAP, no-D9-representation
-change subset of the six d2.3 non-scalar-const carve-outs, soundly under -std=gnu11
-fwrapv -Wall, NO -fno-strict-aliasing. Five changes: A a byte-covering, COLLISION-PROOF
union `_raw` member (always emitted, single source of truth union_raw_member, subsumes
the _pad filler; gnu11 type-pun sound) + B Const::Bytes-over-Ty::Union -> (union U){
._raw = {bytes} } (reuse raw_byte_image; Byte::Reloc -> fail loud #2-reloc); C a
recursive serialize_const_into (const -> byte image: scalar via const_scalar_bits;
NullPtr -> zero; Bytes -> bytes (Reloc -> fail loud); Aggregate over Tuple/Struct/Array
recursing by FieldLayout.offset SOURCE order / element stride; nested EnumValue tag+
payload; Reloc -> fail loud) replacing serialize_multiview_enum_value's scalar-only
payload write, so a NO-RELOCATION structured enum const payload serializes into the
enum `_raw`; D M1-lite: a MULTI-VIEW NICHE-UNTAGGED enum value (tag_value None, no tag
to co-write) routes through the EXISTING u._v<untagged> typed view via lower_enum_payload
(so even a reloc payload lowers to one typed designator, no _raw); E relabel #5
(unreachable Const::Aggregate-over-union; external rustc invariant; keep defensive
guard) + sharpen #6 (CMS content-hash dedup collision; bridge follow-up) + the bounded
§12/§13/§14 generic-c-backend.md doc delta + record the THREE Phase-1 limitations.
DEFERRED (stay fail-loud, NOT this task): #2-reloc (multi-view tagged enum const with a
relocation payload — needs the _c<idx> typed full view), #3 inline bool-niche, #4 inline
nested-niche (both need the niche-cell retype) — all d2.3b/Package A. This is a SPEC
review: judge correctness, completeness, internal consistency, sound scoping, faithful
grounding vs HEAD cedf528. SPECIFICALLY verify: the union `_raw` is layout-neutral +
collision-proof; serialize_const_into has NO off-by-one and fails loud on EVERY reloc;
M1-lite fires only for the niche-untagged variant and never double-inits; the relabels
keep every deferred shape FAIL-LOUD (no fall-through to a silent miscompile); the doc
delta is bounded + truthful; no D9/d2.1/enum_emit/bridge/model change.
Identify: any silent-miscompile path d2.3a introduces; any reloc sub-case
serialize_const_into misses; any reachable shape wrongly left fail-loud or wrongly
claimed handled; drifted citations; scope creep.
</task>
<grounding_rules>
- Cite file:line for every claim. Read the files; don't speculate. Mark hypotheses.
</grounding_rules>
<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
PER-CHANGE (A/B/C/D/E): PASS/FAIL + citation
SOUNDNESS: serialize_const_into off-by-one / reloc-coverage; union _raw layout-neutral;
  M1-lite no-double-init; relabels stay fail-loud — PASS/FAIL + cite; any silent
  miscompile? YES/NO + cite
NEW ISSUES: numbered or "none"
REQUIRED FIXES: numbered (only if VERDICT != APPROVED)
</structured_output_contract>
<default_follow_through_policy>
Do not ask clarifying questions. Read the files, decide, stop.
</default_follow_through_policy>
```

## Quality Gate
- Build + fmt + clippy clean (scoped, per [[clippy-all-targets-test-drift]]).
- All backend unit + runner compile-AND-RUN tests green; the new B2 fixtures run +
  byte-exact; the B4 negatives fail loud with the rewritten labels.
- `emit_generic_c` byte-stable (B5); Mode A ledger byte-stable + verdict unchanged
  (`not-pass` — no new green matrix row; d2.3a adds const sub-shape coverage, not a
  new construct row).
- Diff confined to the Change-A–E set (B6); no `task d2.3` bare label remains.
- The mandatory **post-close Codex soundness pass** ([[assessment-soundness-pass]],
  6/6 streak) on `serialize_const_into` + the union `_raw` + the M1-lite route.

## Acceptance Criteria
1. Every emitted C `union` carries a byte-covering `unsigned char <raw>[size]`
   member spelled through `union_raw_member`, with a **collision-proof** name (a
   union field named `_raw` does not clash); the `_pad` filler is subsumed; size/align
   `_Static_assert`s unchanged.
2. A `Const::Bytes` over `Ty::Union` lowers to `(union U){ .<raw> = { bytes } }`
   (no-reloc), compiles, and a member reads the expected bytes; a `Byte::Reloc` in a
   union image fails loud (#2-reloc).
3. A multi-view enum const with a **no-relocation** structured payload (aggregate /
   array / `Bytes` / `NullPtr` / nested `EnumValue`) serializes into the enum `_raw`
   via `serialize_const_into`, compiles, and reads back correctly; **any** relocation
   in the payload fails loud (#2-reloc).
4. A multi-view **niche-untagged** enum value (incl. a relocation payload) lowers
   through the existing `u._v<untagged>` typed view (M1-lite), compiles, and reads
   back; it emits **no** `_raw` byte image and no double-init.
5. #5 is a retained **defensive** guard with the external-rustc-invariant message;
   #6 is fail-loud with the bridge-dedup root-cause message; **no bare `task d2.3`
   label remains**.
6. The three Phase-1 limitations (#2-reloc, #3, #4) are fail-loud with rewritten
   limitation labels **and recorded** in `decisions-and-state.md`.
7. The bounded `generic-c-backend.md` §12/§13/§14 delta reconciles the union rows +
   documents the union `_raw` member; no other doc/spec change.
8. Build/fmt/clippy clean; all unit + compile-AND-RUN tests green; `emit_generic_c`
   byte-stable; Mode A ledger byte-stable, verdict `not-pass`; diff scope per B6; the
   post-close soundness pass is SOUND.
