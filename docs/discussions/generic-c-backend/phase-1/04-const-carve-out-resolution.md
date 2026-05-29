# Phase 1 — d2.3 non-scalar-const carve-out resolution (plan-first design)

A focused **plan-first design doc** (pre-spec, per `.harness/generic-c/methodology.md`)
resolving the representation fork the **task d2.3** carve-outs raise, before any
spec is written. d2.2 (`regseq-backends-14`, `0347d92`) lowered *all* non-scalar
`Const` shapes the bridge produces **except** a set of fail-loud carve-outs; this
doc decides, per carve-out, **whether it is reachable, how hard a sound lowering
is, and the recommended disposition** (implement / no-code-defensive /
documented-limitation), so the d2.3 spec(s) consume a settled design rather than
making the call in-spec.

> **Method (binding, per `phase-1/README.md`):** settle in the doc, not in
> implementation. This doc → Codex → spec delta into
> `../../regseq-model/generic-c-backend.md` (§12/§13) → task(s).

## 0. Correction to the planning record: it is **six** carve-outs, not five

`decisions-and-state.md` and the handoffs list **five** d2.3 carve-outs. The live
code carries **six** distinct `task d2.3` labels (`body_emit.rs` / `mod.rs`). The
omitted one is **#4** below (`inline niche cell not a top-level field`). All six
are enumerated and dispositioned here; the project record is corrected by this doc.

## 1. The carve-outs, reachability-triaged

Reachability was triaged against the bridge const path (`crates/regseq-driver/
src/bridge/const_.rs`) + the IR (`const_value.rs`, `pretty.rs`). Verdicts:

| # | Label (verbatim) | Site | Reachable? | Realistic Rust source |
|---|---|---|---|---|
| 1 | `Const union byte-image: task d2.3` | `body_emit.rs:2427` | **Yes** | `static U: MyUnion = …;` (a union static → `Const::Bytes`, `const_.rs` static-init path) |
| 2 | `Const enum/aggregate with relocation payload: task d2.3` | `body_emit.rs:2655/2678/3127` | **Yes** (broader than the label) | a **multi-view** enum const whose payload child is **anything `const_scalar_bits` rejects** — a reference/reloc, *or* a no-reloc aggregate/array/bytes/null-ptr/nested-enum — `enum E{A,B(&'static u32)}` / `enum E{A,B([u32;2])}` |
| 3 | `Const::EnumValue (inline bool-niche): task d2.3` | `body_emit.rs:2721/3043` | **Yes** | `const X: Option<bool> = None;` |
| 4 | `Const::EnumValue (inline niche cell not a top-level field): task d2.3` | `body_emit.rs:3047` | **Yes** (rare) | a niche-encoded inline enum whose niche cell is buried in a nested aggregate field |
| 5 | `Const::Aggregate over a union (no active field): task d2.3` | `body_emit.rs:2964` | **No — unreachable / defensive** | none — see §2 |
| 6 | `ConstMemoryStatic at incompatible pointee types: task d2.3` | `mod.rs:1428` | **Yes** (vanishingly rare) | two byte-identical reference consts at incompatible pointee types, content-hash-deduped onto one CMS |

**The crucial structural fact (`enum_emit.rs:154-178`, `build_views`):** an enum's
union-of-views holds, per payload variant, a `_v<idx>` view of **only that
variant's payload fields** — *not* the tag — and a separate `_tag` view of *only*
the tag cell. A C designated initializer can name **one** union member (a second
designator at the same offset overrides the first), so a value-position enum const
must carry its whole value through **one** member. d2.2's `_raw` (an
`unsigned char[size]` byte image, Change A) is that one member, and it faithfully
carries any **no-relocation** byte image. The deferrals are therefore of three
kinds: (a) values `_raw` **cannot** express — a **relocation** payload (a char
array can't hold an address constant); (b) values `_raw` **can** express but the
current emitter does not yet **serialize** into it (the no-reloc non-scalar
`const`/valtree payloads, §5.1 #2-noreloc — a serializer gap, not a `_raw`
limitation); and (c) the **inline** enums that have **no union at all** (the
≤1-view inline collapse, `enum_emit.rs:98`), so no `_raw` exists to write.

**Second structural fact (the #2 label undersells its trigger):** the multi-view
serializer `serialize_multiview_enum_value` writes each payload child via
`const_scalar_bits` (`body_emit.rs:3186`), which returns `None` — and so defers
under the **"relocation payload"** label — for **every** non-scalar child:
`Reloc` / `NullPtr` / `Aggregate` / `Bytes` / `EnumValue` / ZST (its own doc
comment, `:3182-3185`). So #2 fires not only on a reloc payload but on **any
non-scalar payload** — a no-relocation aggregate / array / bytes / **null pointer**
/ **nested enum** included. The right axis is **reloc presence** (§5.1): a payload
carrying *no* relocation is `_raw`-serializable (cheap); only a *reloc-bearing*
payload is the hard case. The d2.3 work and the relabel must split on that axis.

## 2. #5 — `Const::Aggregate` over `Ty::Union`: **unreachable, keep defensive (no code)**

`Const::Aggregate(Vec<ConstId>)` carries **no active `FieldIdx`**
(`const_value.rs:38-39`); only the *runtime* `AggregateKind::Union(TyId, FieldIdx)`
records the live member (`rvalue.rs`). The only bridge path that produces
`Const::Aggregate` is the **valtree** path (`translate_ty_const_value`), whose
non-enum-ADT arm (`const_.rs:104`) would, *in repo code*, aggregate **any**
non-enum ADT — including a union — if rustc handed it such a valtree. The
unreachability therefore rests on an **external rustc invariant** (not something
repo code enforces): **rustc does not structurally represent a `union` const as a
valtree** — a union const/static instead reaches the backend as `Const::Bytes`
via `translate_static_init` (→ carve-out **#1**). The bridge's own pretty-printer
also rejects an `Aggregate`-over-union (`pretty.rs:714-718`), corroborating that no
upstream path is expected to feed one in.

**Disposition:** **no implementation.** The arm stays a **defensive guard** —
precisely *because* its unreachability is an external-invariant assumption rather
than a repo-proven impossibility, the fail-loud must remain (it is sound: a hard
error, never a miscompile). d2.3 only **sharpens the message** to name the
invariant it rests on (e.g. `"Const::Aggregate over a union: not expected (rustc
does not emit a union valtree; unions arrive as Const::Bytes)"`) with a one-line
grounding comment. *Zero representation work; the guard is retained, not removed.*

## 3. #6 — `ConstMemoryStatic` at incompatible pointee types: **keep fail-loud; optional bridge follow-up**

The collision is real but its **root cause is in the bridge**: the
`ConstMemoryStatic` dedup keys on a **content hash only** (`const_.rs:145-152`), so
two reference consts with byte-identical images but incompatible pointee types
(e.g. `&[u8;4]` and a `&[u32]` sharing a byte image) can be merged onto one CMS
`InstanceId`; the backend's re-typing pre-pass then records two conflicting
`StorageTy`s for that id and fails loud (`mod.rs:1410-1435`, `record`). The defer
is **sound** (fail-loud, no miscompile), and the trigger is **vanishingly rare**
(byte-identical images at incompatible pointee types in one TU).

Two sound resolutions, neither a representation change:
- **(R6-a, cleanest, out of d2.3's backend scope):** a **bridge** type-aware dedup
  key — include the (canonical) pointee type in the dedup hash so the two consts
  get **separate** CMS statics; the collision then cannot arise and the backend
  defer becomes dead. This is a small, localized **bridge** task.
- **(R6-b, rejected):** a **backend split** — emit N differently-typed statics
  from one CMS `InstanceId`. Rejected: one `InstanceId` maps to one emission
  symbol; splitting requires synthesizing fresh symbols *and* rewriting every
  consumer's reloc to the right split — invasive symbol surgery for a
  vanishingly-rare case.

**Disposition:** **keep the backend fail-loud** (sound) in d2.3; **file R6-a as a
separate optional bridge task** (or accept it as a documented Phase-1 limitation).
d2.3 may **sharpen the label** but does no backend split. *Out of the enum/union
representation work.*

## 4. #1 — `Const::Bytes` over `Ty::Union`: **implement (clean, backend-local)**

A union static's byte image must initialize **all** bytes (a size-spanning
aggregate member has interior padding the typed source-field designators do not
cover — *size equality alone is not byte-preserving*, which is why d2.2 deferred a
*typed* union member). The byte-faithful answer is the **same primitive d2.2 used
for enums (Change A): a byte-covering `_raw` union member.**

**Resolution (R1):** add a byte-covering `unsigned char <raw>[<layout.size>];`
member to **every** emitted `union` (`layout_emit::emit_union_body`, `:278`),
subsuming the unnamed trailing `_pad[size]` filler with a **stably-named**
member. Lower a `Const::Bytes` over `Ty::Union` as `(union U){ .<raw> = {
b0,…,bN-1 } }` (the `bytes_to_typed_literal` `Ty::Union` arm, `body_emit.rs:2427`).
Runtime reads stay typed in-place union accesses (gnu11 union type-punning — sound
under the pinned `-std=gnu11`, no `-fno-strict-aliasing`, exactly as the enum
`_raw` argument in d2.2). Layout-neutral: alignment 1, the union's `aligned`/sizeof
`_Static_assert`s are unchanged.

- **The member name MUST be collision-proof, not a hardcoded `_raw`.** Unlike the
  *enum* `_raw` (whose `union u` holds only generated `_tag`/`_v<idx>` siblings,
  so `_raw` can never clash), a *union*'s `_raw` is a sibling of the **user's
  sanitized field names** (`layout_emit.rs:112`), and a Rust union field literally
  named `_raw` sanitizes to `_raw` → collision → the init writes the wrong member.
  This is the same hazard the `_padN` filler already guards against via `PadNamer`
  (seeded with the real member names, `layout_emit.rs:11-23`). R1 must (a) generate
  the raw member name through the **same collision-proofing** (a `_raw`-seeded
  namer, or reuse `PadNamer`'s reservation), and (b) expose it through a **single
  source of truth** the body side reads (mirroring how `enum_raw_member`,
  `enum_emit.rs:312`, is the sole speller of the enum `u._raw`), so the init
  designator can never drift from the emitted member name.
- **Touches:** `layout_emit.rs` (union body + the shared name lookup) +
  `body_emit.rs` (the `Ty::Union` arm) + the union text-assertion tests (a new raw
  member; size/align asserts unchanged). **No** bridge / model change.
- A `Byte::Reloc` inside a union byte image (a union holding a reference) →
  fail-loud (a char array cannot hold an address constant — same constraint as
  #2-reloc; not in #1's scope).

This is a clean, realistic win (`static U: MyUnion = …` is ordinary) and is
recommended **in scope for d2.3** regardless of the §5 decision.

## 5. #2 / #3 / #4 — the enum-const representation family (the real fork)

The *representation*-hard members of this family are blocked by one root cause: **a
value-position enum const cannot carry discriminant + payload through a single C
initializer** when (a) the payload holds a relocation (`_raw`, a `char[]`, cannot
encode `&Y`) — **#2-reloc**, or (b) the enum is **inline** (no union, so no `_raw`)
and the niche bit-pattern cannot be written through a typed top-level field —
**#3** (a `_Bool` niche cell: `(_Bool){2}` normalizes 2→1) / **#4** (the niche cell
is nested, so no single designator). (The third member, **#2-noreloc**, is *not*
a representation problem — it is a serializer gap, closed cheaply per §5.1/§5.3.)

### 5.1 What already works (so the *deferred* set is narrow)

Confirmed by tracing `enum_value_expr` (`body_emit.rs:2994`):
- **`Option<&T>`** — `Some(&Y)` and `None` **both lower today** (inline niche;
  pointer cell coincides with the single top-level field). The common
  reference-bearing option is *not* deferred.
- **`Option<bool>`** — `Some(true)` lowers; only **`None`** defers (#3).
- Multi-view **no-relocation** enum consts lower via `_raw` (d2.2).

So the deferred cases are genuinely the tail. **#2 splits on one axis — does the
payload (transitively) carry a relocation?**
- **#2-scalar** — a multi-view enum const with an all-scalar payload: **already
  works** (d2.2 `_raw` serialization).
- **#2-noreloc** — a multi-view enum const whose payload is non-scalar but carries
  **no relocation**: an aggregate / array / `Const::Bytes` / **null pointer**
  (`NullPtr`, all-zero bytes) / **nested enum** (`EnumValue`) — e.g.
  `enum E{A,B([u32;2])}`, `const X=E::B([1,2])`. Such a value **can** go in `_raw`
  (no address constant to encode). The **`static` form already works**: a static
  arrives as `Const::Bytes` over the enum and lowers via `enum_from_bytes` → `_raw`
  directly (`body_emit.rs:2640-2666`, which defers *only* on a `Byte::Reloc`). Only
  the **`const`/valtree form** (a `Const::EnumValue` with a non-scalar child)
  defers, because the multi-view serializer handles only scalar children
  (`const_scalar_bits`). The fix is a **recursive const→byte-image walker** for the
  payload (it serializes a `NullPtr` to zero bytes and recurses into
  `Aggregate`/`Bytes`/`EnumValue` children) — `body_emit` only, **no
  representation change**.
- **#2-reloc** — a multi-view enum const whose payload carries a **relocation**
  anywhere (a reference / reloc, possibly nested): `enum E{A,B(&'static u32)}`,
  `const X=E::B(&Y)`. This is the one `_raw` **fundamentally** cannot express (a
  char array can't hold an address constant) → it needs the typed full view **M1**.

Plus **#3** (bool-niche `None`) and **#4** (nested-niche) for inline enums.

### 5.2 Candidate mechanisms

- **M1 — typed full-object per-variant view (`_c<idx>`), for #2-reloc.** Add, per
  payload-bearing variant of a **multi-view** enum, a union member
  `struct { <tag field>; <payload fields…>; } _c<idx>;` whose fields sit at their
  absolute enum-object offsets (so the reloc lands in a **typed pointer field**:
  `(struct E){ .u._c1 = { .tag = <tv>, .ptr = &Y } }` — one union member, no
  double-init, an address constant in a pointer field). It is the **typed**
  alternative to `_raw`, so it actually serves *any* payload field type (it would
  also cover #2-noreloc) — but #2-noreloc is cheaper to close with the
  const→byte-image serializer extension (no new member), so M1's *unique* job is
  the **reloc** case `_raw` cannot encode. The **runtime navigator is unchanged**
  (`_tag`/`_v<idx>` still drive reads; `_c<idx>` is const-init-only).
  - *Sub-win (M1-lite):* a **niche untagged** variant with a reloc payload (no
    `tag_value` to write) can route through the **existing** `_v<untagged>` view
    (`.u._v<v>.<field> = &Y`) with **no new member** — a small `body_emit` change.
    (Reachable only when such an enum is multi-view; rare.)
  - *Cost:* a new union member per payload variant of every multi-view enum;
    `offsetof` guards; the c2/d2.1 enum-emit **text-assertion** tests update
    (size/align unchanged). Layout-neutral. Moderate `enum_emit.rs` +
    `body_emit.rs` change; **extends the locked D9 representation.**
- **M2 — an inline-niche raw-write path, for #3/#4.** Two sub-options:
  - **M2-a (retype the niche cell):** emit the niche-bearing inline scalar field
    as its **raw integer storage type** (e.g. `uint8_t`) instead of `_Bool`, so
    `.<field> = 2` stores byte 2 faithfully; for #4 also emit a **nested
    designator path** (`.<f0>.<…>.<leaf> = <bits>`) located by walking the layout
    to the niche offset. *Cost (corrected):* the discriminant **read/write does
    not ripple** — niche tag access already goes through **raw-integer memcpy**,
    not the `_Bool` field (`body_emit.rs:754`, `:816`). The actual ripple is (i)
    the enum **field C-type emission** for the retyped cell, (ii) the **payload
    access / assignment** paths that read/write that field as its value type
    (`body_emit.rs:862`, `:992`), and (iii) the enum-emit **text-assertion tests**;
    plus a niche-locator walk for #4. Surgical, contained, but non-trivial.
  - **M2-b (inline `_raw` via union-wrap):** give inline enums a `_raw` by wrapping
    the inline view in a `union { <inline view>; unsigned char _raw[size]; } u;`.
    *Cost:* **de-inlines** §9.8/§9.9 — every inline-enum runtime access (the whole
    d2.1 inline path, all inline-enum tests) changes from `<field>` to
    `u.<view>.<field>`. **Heavy**; rejected as disproportionate to the rare const
    shapes it serves.

### 5.3 Cost vs. value, and the two packages

Every defer is **sound** (a hard `Unsupported`, never a silent miscompile —
confirmed across all six sites). The fixes divide by the **risk axis that matters
here**: do they perturb the **locked, Codex-hardened D9 enum representation and/or
the d2.1 navigator**? That split — not a #2-vs-#3-vs-#4 split — defines the two
packages, and is the one decision this doc surfaces (§6):

**No D9-enum / d2.1-navigator change (the cheap, low-risk wins):**
- **#1** — the union `_raw` member (R1, §4). A **layout-neutral union
  representation extension** (a byte-covering member, sizeof/align unchanged), *not*
  a D9-enum or navigator change. Realistic (`static U: MyUnion`).
- **#2-noreloc** — the recursive const→byte-image serializer extension (§5.1): a
  `body_emit` walker that serializes a no-reloc `Aggregate`/`Bytes`/`NullPtr`/
  nested-`EnumValue` payload child into the existing `_raw` image. No new member.
- **M1-lite** — route a **multi-view niche-untagged** reloc payload through the
  existing `_v<untagged>` view (no `tag_value` to write, so one designator
  suffices). No new member.
- **#5** relabel + **#6** label sharpen (no code).

**Extends the D9 enum representation / d2.1 navigator (the costly tail; rare shapes):**
- **#2-reloc** — M1 (the `_c<idx>` typed full view) for a multi-view variant with a
  reloc-bearing payload.
- **#3 / #4** — M2-a (retype the inline niche cell + the #4 nested-designator
  locator).

So:

- **Package A — full coverage.** The cheap wins **plus** M1 (#2-reloc) + M2-a
  (#3/#4). Every reachable enum/union const lowers; the Phase-1 exit gate ("every
  construct lowers, compiles, runs") is met for `Const` with no documented gaps.
  Best split: **d2.3a** = the cheap wins; **d2.3b** = M1 + M2-a (the D9-extending
  part — re-blesses the enum-emit text tests; `offsetof`/sizeof guards prove
  layout-neutrality).
- **Package B — sound-but-scoped.** Ship **only the cheap wins** as d2.3a and keep
  the costly tail **fail-loud as explicitly-documented Phase-1 limitations**:
  **#2-reloc** (multi-view tagged enum const with a reference payload), **#3**
  (inline bool-niche `None`), **#4** (inline nested-niche const). All rare; all
  sound hard-errors. The codegen matrix tracks *constructs*, not every const
  sub-shape, so the gate can read `pass` with these three recorded as sound
  scope-outs (the ledger / a known-limitations note must list **all three** — not
  just "#2-reloc" — so the gap is honest).

**Author's recommendation:** **Package B**, *if* the architect accepts documenting
those **three** tail shapes as sound Phase-1 limitations; **Package A (d2.3a +
d2.3b)** *if* a gap-free `Const` exit gate is required. Rationale: the cheap wins
already close #1 + #2-noreloc + the niche-untagged reloc sub-case at zero
representation risk; the remaining tail (#2-reloc, #3, #4) is genuinely rare, and
M1 + M2-a perturb the **locked, Codex-hardened D9 representation** and the enum
field-type emission for marginal real-world coverage. Each defer is sound, so
Package B trades only *completeness*, never *correctness*.

## 6. The single decision this doc surfaces

Everything else is resolved above (implement #1; #2-noreloc + M1-lite as cheap
no-representation-change wins; #5 = no code; #6 = keep fail-loud + optional bridge
follow-up; the M1/M2-a mechanisms are designed). The one open call is **§5.3:
Package A (also close the costly tail — M1 for #2-reloc + M2-a for #3/#4, as a
D9-extending d2.3b) vs. Package B (ship only the cheap wins as d2.3a; document
#2-reloc, #3, #4 as three sound Phase-1 limitations).** Recommendation: **Package
B** unless a gap-free `Const` exit gate is wanted.

## 7. Spec hand-off (once §6 is decided)

- **d2.3a (both packages):** R1 (#1 union `_raw`, collision-proof name) +
  #2-noreloc (the const→byte-image payload serializer) + M1-lite
  (niche-untagged reloc routing) + the #5 relabel + the #6 label sharpen.
  8-section spec → Codex → `task.py add-task --depends-on regseq-backends-14`.
  **No D9-enum / d2.1-navigator change** (only the layout-neutral union `_raw`
  extension) — confine the diff to `body_emit.rs` + `layout_emit.rs` (union body) +
  tests.
- **d2.3b (Package A only):** M1 (`_c<idx>` full view, #2-reloc) + M2-a (retype the
  inline niche cell + the #4 nested-designator locator). Its own 8-section spec →
  Codex; depends on d2.3a. **Extends D9** — re-bless the enum-emit text tests;
  size/align/`offsetof` guards prove layout-neutrality.
- Either path keeps the **mandatory post-close Codex soundness pass**
  ([[assessment-soundness-pass]], 6/6 streak) — the byte-image / `_raw` / niche
  serialization arms are exactly the off-by-one-prone surfaces it has caught before.

## 8. Grounding anchors

- Bridge const path: `crates/regseq-driver/src/bridge/const_.rs` (static-init →
  `Const::Bytes`; valtree → `Const::Aggregate`/`EnumValue`; CMS dedup `:145-152`).
- IR: `crates/regseq-model/src/const_value.rs` (`:38-39` no `FieldIdx`),
  `pretty.rs:714-718` (the union-aggregate reject).
- Backend: `body_emit.rs` (the const seam `:2380-3142`; the six labels),
  `enum_emit.rs` (`build_views` `:154`, `_raw` `:312`, the inline collapse `:98`),
  `layout_emit.rs:278` (union body), `mod.rs:1410` (CMS re-typing `record`).
- Canonical spec to delta: `../../regseq-model/generic-c-backend.md` §12/§13.
- Locked decision this extends: D9 (`02-architecture-decisions.md`; the
  union-of-views enum representation).
